/**
 * Multi-touch gestures: pan, pinch-zoom and two-finger rotation on an element
 * (photo viewers, maps, canvases, sticker editors), with trackpad pinch
 * (Ctrl/⌘ + wheel), rubber-banded limits, momentum and a spring back into
 * range on release.
 *
 * ```ts
 * gesture('#photo', { minScale: 1, maxScale: 6, bounds: { minX: -400, maxX: 400, minY: -300, maxY: 300 } });
 * gesture(sticker, { type: 'pinch,rotate', onChange: (g) => save(g.scale, g.rotation) });
 * gesture(canvasEl, { apply: false, onChange: ({ x, y, scale }) => view.set(x, y, scale) });  // drive anything
 * ```
 *
 * The point under the fingers stays under the fingers: zooming and rotating
 * happen about the gesture's centre, not the element's.
 */
import { Targets, toTargets, stateOf, composeTransform, TargetState } from './props';
import { VelocityTracker } from './utils';
import { inertia, spring, PhysicsAnimation } from './spring';
import { track } from './context';
import type { Bounds } from './interaction';
import { prefersReduced } from './core';

export interface GestureOptions {
  /** Comma separated: 'pan', 'pinch', 'rotate' (default all three). */
  type?: string;
  /** Write the transform to the element (default true); otherwise only report through callbacks. */
  apply?: boolean;
  /** Scale limits; past them the scale is rubber-banded and springs back on release. */
  minScale?: number;
  maxScale?: number;
  /** Translation limits (`minX`, `maxX`, `minY`, `maxY`, in px), rubber-banded the same way. */
  bounds?: Bounds;
  /** Pull past a limit: 0 = hard stop … 1 = no resistance (default 0.35). */
  edgeResistance?: number;
  /** Keep gliding after a fling (default true). */
  inertia?: boolean;
  /** Trackpad pinch (Ctrl/⌘ + wheel) zooms about the pointer (default true). */
  wheel?: boolean;
  /** Wheel zoom speed (default 0.01 per wheel pixel, exponential). */
  wheelSpeed?: number;
  onStart?: (g: Gesture) => void;
  onChange?: (g: Gesture) => void;
  /** Fingers lifted (momentum or a spring back may follow). */
  onEnd?: (g: Gesture) => void;
  /** Everything has come to rest. */
  onRest?: (g: Gesture) => void;
}

type Pt = { x: number; y: number };

/** Gesture state and controller; see {@link gesture}. */
export class Gesture {
  readonly target: HTMLElement;
  /** Translation (px), scale and rotation (deg) of the element. */
  x = 0;
  y = 0;
  scale = 1;
  rotation = 0;
  /** Number of pointers down. */
  pointers = 0;
  isActive = false;
  enabled = true;
  private o: GestureOptions;
  private s: TargetState;
  private pts = new Map<number, Pt>();
  /** Gesture start: transform, layout centre, finger centroid / distance / angle. */
  private g0 = { x: 0, y: 0, scale: 1, rotation: 0, cx: 0, cy: 0, px: 0, py: 0, dist: 1, angle: 0 };
  private vx = new VelocityTracker();
  private vy = new VelocityTracker();
  private vr = new VelocityTracker();
  private anims: PhysicsAnimation[] = [];
  private types: Set<string>;
  private last: Pt = { x: 0, y: 0 };

  constructor(target: HTMLElement, options: GestureOptions = {}) {
    this.target = target;
    this.o = options;
    this.types = new Set((options.type ?? 'pan,pinch,rotate').split(/[\s,]+/));
    this.s = stateOf(target);
    this.x = this.s.t[0];
    this.y = this.s.t[1];
    this.scale = this.s.t[8];
    this.rotation = this.s.t[3];
    const st = target.style;
    st.touchAction = 'none';
    st.userSelect = 'none';
    target.addEventListener('pointerdown', this.down);
    if (options.wheel !== false) target.addEventListener('wheel', this.onWheel, { passive: false });
    track(this);
  }

  // --- geometry -------------------------------------------------------------

  /** Centre of the element's untransformed box (screen px). */
  private centre(): Pt {
    const r = this.target.getBoundingClientRect();
    // Transforms are about the centre, so the box centre moved by exactly (x, y).
    return { x: r.left + r.width / 2 - this.x, y: r.top + r.height / 2 - this.y };
  }

  /** Translation that keeps the local point now under `from` (at the gesture start) under `to`. */
  private anchor(from: Pt, to: Pt, scale: number, rotation: number): Pt {
    const g = this.g0;
    const a0 = (g.rotation * Math.PI) / 180;
    const a1 = (rotation * Math.PI) / 180;
    // Local offset of the anchor from the centre at the start (undo R0·S0).
    const dx = from.x - g.cx - g.x;
    const dy = from.y - g.cy - g.y;
    const c0 = Math.cos(-a0), s0 = Math.sin(-a0);
    const lx = (dx * c0 - dy * s0) / (g.scale || 1);
    const ly = (dx * s0 + dy * c0) / (g.scale || 1);
    // Where that local point lands with the new rotation/scale.
    const c1 = Math.cos(a1), s1 = Math.sin(a1);
    const wx = (lx * c1 - ly * s1) * scale;
    const wy = (lx * s1 + ly * c1) * scale;
    return { x: to.x - g.cx - wx, y: to.y - g.cy - wy };
  }

  private resist(v: number, min = -Infinity, max = Infinity): number {
    const k = this.o.edgeResistance ?? 0.35;
    if (v < min) return min - (min - v) * k;
    if (v > max) return max + (v - max) * k;
    return v;
  }
  /** Scale limits are resisted in log space (zooming feels even both ways). */
  private resistScale(s: number): number {
    const k = this.o.edgeResistance ?? 0.35;
    const min = this.o.minScale ?? 0;
    const max = this.o.maxScale ?? Infinity;
    if (s > max) return max * Math.pow(s / max, k);
    if (min > 0 && s < min) return min * Math.pow(s / min, k);
    return Math.max(s, 1e-4);
  }

  private render(): void {
    if (this.o.apply !== false) {
      const t = this.s.t;
      t[0] = this.x;
      t[1] = this.y;
      t[3] = this.rotation;
      t[8] = t[9] = this.scale;
      this.s.dirty = false;
      this.target.style.transform = composeTransform(this.s);
    }
    this.o.onChange?.(this);
  }

  private stopAnims(): void {
    for (const a of this.anims) a.stop();
    this.anims = [];
  }

  // --- pointer handling -----------------------------------------------------

  private begin(): void {
    // (Re)base the gesture on the current pointers.
    const p = Array.from(this.pts.values());
    const c = this.centre();
    let px = p[0].x, py = p[0].y, dist = 1, angle = 0;
    if (p.length >= 2) {
      px = (p[0].x + p[1].x) / 2;
      py = (p[0].y + p[1].y) / 2;
      dist = Math.hypot(p[1].x - p[0].x, p[1].y - p[0].y) || 1;
      angle = Math.atan2(p[1].y - p[0].y, p[1].x - p[0].x);
    }
    this.g0 = { x: this.x, y: this.y, scale: this.scale, rotation: this.rotation, cx: c.x, cy: c.y, px, py, dist, angle };
  }

  private down = (e: PointerEvent): void => {
    if (!this.enabled || (e.button !== undefined && e.button > 0)) return;
    this.syncFromState();
    this.stopAnims();
    this.pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
    this.pointers = this.pts.size;
    try {
      this.target.setPointerCapture?.(e.pointerId);
    } catch {
      /* synthetic events */
    }
    if (this.pts.size === 1) {
      window.addEventListener('pointermove', this.move);
      window.addEventListener('pointerup', this.up);
      window.addEventListener('pointercancel', this.up);
      this.vx.reset();
      this.vy.reset();
      this.vr.reset();
      this.isActive = true;
      this.o.onStart?.(this);
    }
    this.begin();
  };

  private move = (e: PointerEvent): void => {
    const p = this.pts.get(e.pointerId);
    if (!p) return;
    p.x = e.clientX;
    p.y = e.clientY;
    const g = this.g0;
    const list = Array.from(this.pts.values());
    let scale = g.scale;
    let rotation = g.rotation;
    let cx = list[0].x, cy = list[0].y;
    if (list.length >= 2) {
      cx = (list[0].x + list[1].x) / 2;
      cy = (list[0].y + list[1].y) / 2;
      const dist = Math.hypot(list[1].x - list[0].x, list[1].y - list[0].y) || 1;
      const angle = Math.atan2(list[1].y - list[0].y, list[1].x - list[0].x);
      if (this.types.has('pinch')) scale = this.resistScale((g.scale * dist) / g.dist);
      if (this.types.has('rotate')) {
        let d = ((angle - g.angle) * 180) / Math.PI;
        if (d > 180) d -= 360;
        if (d < -180) d += 360;
        rotation = g.rotation + d;
      }
    }
    // Pan follows the centroid; without 'pan', zoom/rotate about it in place.
    const to = this.types.has('pan') ? { x: cx, y: cy } : { x: g.px, y: g.py };
    const t = this.anchor({ x: g.px, y: g.py }, to, scale, rotation);
    const B = this.o.bounds ?? {};
    this.x = this.resist(t.x, B.minX, B.maxX);
    this.y = this.resist(t.y, B.minY, B.maxY);
    this.scale = scale;
    this.rotation = rotation;
    this.last = { x: cx, y: cy };
    const ts = e.timeStamp || undefined;
    this.vx.add(this.x, ts);
    this.vy.add(this.y, ts);
    this.vr.add(this.rotation, ts);
    this.render();
  };

  private up = (e: PointerEvent): void => {
    if (!this.pts.delete(e.pointerId)) return;
    this.pointers = this.pts.size;
    if (this.pts.size > 0) {
      // A finger lifted: carry on with the remaining one(s) from here.
      this.begin();
      return;
    }
    window.removeEventListener('pointermove', this.move);
    window.removeEventListener('pointerup', this.up);
    window.removeEventListener('pointercancel', this.up);
    this.isActive = false;
    this.o.onEnd?.(this);
    this.settle(this.last, this.o.inertia !== false && !prefersReduced());
  };

  private onWheel = (e: WheelEvent): void => {
    if (!this.enabled || !(e.ctrlKey || e.metaKey) || !this.types.has('pinch')) return;
    e.preventDefault();
    this.syncFromState();
    this.stopAnims();
    const at = { x: e.clientX, y: e.clientY };
    const c = this.centre();
    this.g0 = { x: this.x, y: this.y, scale: this.scale, rotation: this.rotation, cx: c.x, cy: c.y, px: at.x, py: at.y, dist: 1, angle: 0 };
    const k = this.o.wheelSpeed ?? 0.01;
    const scale = this.resistScale(this.scale * Math.exp(-e.deltaY * k));
    const t = this.anchor(at, at, scale, this.rotation);
    this.scale = scale;
    this.x = t.x;
    this.y = t.y;
    this.render();
    // Wheel events have no "up": settle limits after a short pause.
    clearTimeout(this.wheelTimer);
    this.wheelTimer = setTimeout(() => this.settle(at, false), 150) as unknown as number;
  };
  private wheelTimer = 0;

  /** After release: spring scale back into range (about `at`), or glide with momentum. */
  private settle(at: Pt, glide: boolean): void {
    const min = this.o.minScale ?? 0;
    const max = this.o.maxScale ?? Infinity;
    const B = this.o.bounds ?? {};
    const clampS = Math.min(max, Math.max(min, this.scale));
    const done = () => this.o.onRest?.(this);
    if (clampS !== this.scale) {
      // Spring the scale back, keeping the point under `at` fixed.
      const c = this.centre();
      this.g0 = { x: this.x, y: this.y, scale: this.scale, rotation: this.rotation, cx: c.x, cy: c.y, px: at.x, py: at.y, dist: 1, angle: 0 };
      const proxy = { s: this.scale };
      const a = spring(proxy, { s: clampS }, {
        stiffness: 300, damping: 30,
        onUpdate: () => {
          const t = this.anchor(at, at, proxy.s, this.rotation);
          this.scale = proxy.s;
          this.x = Math.min(B.maxX ?? Infinity, Math.max(B.minX ?? -Infinity, t.x));
          this.y = Math.min(B.maxY ?? Infinity, Math.max(B.minY ?? -Infinity, t.y));
          this.render();
        },
        onComplete: done,
      });
      this.anims.push(a);
      return;
    }
    const vx = glide ? this.vx.velocity : 0;
    const vy = glide ? this.vy.velocity : 0;
    const vr = glide && this.types.has('rotate') ? this.vr.velocity : 0;
    const outside = this.x < (B.minX ?? -Infinity) || this.x > (B.maxX ?? Infinity) || this.y < (B.minY ?? -Infinity) || this.y > (B.maxY ?? Infinity);
    if (!outside && Math.hypot(vx, vy) < 30 && Math.abs(vr) < 20) {
      done();
      return;
    }
    if (this.o.apply === false) {
      // Nothing to animate on the element: glide a proxy and report it.
      const proxy = { x: this.x, y: this.y, r: this.rotation };
      const a = inertia(proxy, {
        x: { velocity: vx, min: B.minX, max: B.maxX }, y: { velocity: vy, min: B.minY, max: B.maxY }, r: { velocity: vr },
      }, {
        onUpdate: () => {
          this.x = proxy.x;
          this.y = proxy.y;
          this.rotation = proxy.r;
          this.o.onChange?.(this);
        },
        onComplete: done,
      });
      this.anims.push(a);
      return;
    }
    const props: Record<string, { velocity: number; min?: number; max?: number }> = {
      x: { velocity: vx, min: B.minX, max: B.maxX },
      y: { velocity: vy, min: B.minY, max: B.maxY },
    };
    if (vr) props.rotate = { velocity: vr };
    const a = inertia(this.target, props, {
      onUpdate: () => {
        this.syncFromState();
        this.o.onChange?.(this);
      },
      onComplete: done,
    });
    this.anims.push(a);
  }

  private syncFromState(): void {
    if (this.o.apply === false) return;
    const t = this.s.t;
    this.x = t[0];
    this.y = t[1];
    this.rotation = t[3];
    this.scale = t[8];
  }

  // --- control --------------------------------------------------------------

  /** Set the transform directly (e.g. a "reset zoom" button: animate it with `animate(el, …)` instead). */
  set(values: { x?: number; y?: number; scale?: number; rotation?: number }): void {
    this.stopAnims();
    if (values.x !== undefined) this.x = values.x;
    if (values.y !== undefined) this.y = values.y;
    if (values.scale !== undefined) this.scale = values.scale;
    if (values.rotation !== undefined) this.rotation = values.rotation;
    this.render();
  }
  enable(): void {
    this.enabled = true;
  }
  disable(): void {
    this.enabled = false;
  }
  /** Stop listening (the element keeps its transform). */
  kill(): void {
    this.stopAnims();
    clearTimeout(this.wheelTimer);
    this.target.removeEventListener('pointerdown', this.down);
    this.target.removeEventListener('wheel', this.onWheel);
    window.removeEventListener('pointermove', this.move);
    window.removeEventListener('pointerup', this.up);
    window.removeEventListener('pointercancel', this.up);
  }
  /** Stop listening and reset the transform. */
  revert(): void {
    this.kill();
    this.set({ x: 0, y: 0, scale: 1, rotation: 0 });
  }
}

/** Pan / pinch / rotate gestures on each target. Returns one {@link Gesture} per element. */
export function gesture(targets: Targets, options?: GestureOptions): Gesture[] {
  return (toTargets(targets) as HTMLElement[]).map((el) => new Gesture(el, options));
}
