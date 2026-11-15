/**
 * Interaction: draggable elements with momentum, a unified input observer
 * (wheel / touch / pointer intents), and hover / press gestures.
 *
 * ```ts
 * draggable('.card', { bounds: '.board', inertia: true, snap: { x: 120, y: 120 } });
 * draggable('.knob', { type: 'rotate', inertia: true, onDrag: (d) => setVolume(d.rotation) });
 * observer({ target: window, type: 'wheel,touch', onDown: next, onUp: prev, tolerance: 20 });
 * hover('.btn', (el) => { const a = animate(el, { scale: 1.05 }); return () => animate(el, { scale: 1 }); });
 * ```
 */
import { Targets, toTargets, stateOf, composeTransform, TargetState } from './props';
import { inertia, PhysicsAnimation } from './spring';
import { VelocityTracker } from './utils';
import { track } from './context';

// ---------------------------------------------------------------------------
// Draggable
// ---------------------------------------------------------------------------

export interface Bounds {
  minX?: number;
  maxX?: number;
  minY?: number;
  maxY?: number;
  minRotation?: number;
  maxRotation?: number;
}

type SnapSpec = number | number[] | ((v: number) => number);

export interface DraggableOptions {
  /** `'x,y'` (default), `'x'`, `'y'`, or `'rotate'`. */
  type?: 'x,y' | 'x' | 'y' | 'rotate';
  /** Keep within an element (by its box) or explicit limits (in transform px / degrees). */
  bounds?: Element | string | Bounds;
  /** Resistance past the bounds while dragging: 0 (hard stop) – 1 (no resistance). Default 0.35. */
  edgeResistance?: number;
  /** Glide with momentum after release. */
  inertia?: boolean;
  /** Snap the resting position: number step, list or function — per axis via `{ x, y, rotation }`. */
  snap?: SnapSpec | { x?: SnapSpec; y?: SnapSpec; rotation?: SnapSpec };
  /** Only start dragging from this element (default: the target). */
  handle?: Element | string;
  /** Lock to the dominant axis after a few pixels. */
  lockAxis?: boolean;
  /** Minimum movement (px) before a drag starts — lets clicks through. Default 3. */
  minimumMovement?: number;
  cursor?: string;
  onPress?: (d: Draggable, e: PointerEvent) => void;
  onDragStart?: (d: Draggable, e: PointerEvent) => void;
  onDrag?: (d: Draggable, e: PointerEvent) => void;
  onDragEnd?: (d: Draggable, e: PointerEvent) => void;
  onRelease?: (d: Draggable, e: PointerEvent) => void;
  onThrowUpdate?: (d: Draggable) => void;
  onThrowComplete?: (d: Draggable) => void;
}

const resolveSnap = (s: SnapSpec | undefined, v: number): number =>
  s === undefined ? v
    : typeof s === 'function' ? s(v)
    : Array.isArray(s) ? s.reduce((a, b) => (Math.abs(b - v) < Math.abs(a - v) ? b : a))
    : Math.round(v / s) * s;

export class Draggable {
  readonly target: HTMLElement;
  /** Current transform position / rotation. */
  x = 0;
  y = 0;
  rotation = 0;
  /** Movement since the press. */
  deltaX = 0;
  deltaY = 0;
  isDragging = false;
  isPressed = false;
  enabled = true;
  private o: DraggableOptions;
  private s: TargetState;
  private vx = new VelocityTracker();
  private vy = new VelocityTracker();
  private vr = new VelocityTracker();
  private start = { px: 0, py: 0, x: 0, y: 0, r: 0, angle: 0, cx: 0, cy: 0 };
  private lock: 'x' | 'y' | null = null;
  private throwAnim: PhysicsAnimation | null = null;
  private handleEl: Element;
  private limits: Bounds = {};

  constructor(target: HTMLElement, options: DraggableOptions = {}) {
    this.target = target;
    this.o = options;
    this.s = stateOf(target);
    this.x = this.s.t[0];
    this.y = this.s.t[1];
    this.rotation = this.s.t[3];
    const h = options.handle;
    this.handleEl = (typeof h === 'string' ? target.querySelector(h) : h) ?? target;
    const hs = (this.handleEl as HTMLElement).style;
    hs.touchAction = options.type === 'x' ? 'pan-y' : options.type === 'y' ? 'pan-x' : 'none';
    hs.cursor = options.cursor ?? 'grab';
    hs.userSelect = 'none';
    this.handleEl.addEventListener('pointerdown', this.down as EventListener);
    track(this);
  }

  private computeBounds(): Bounds {
    const b = this.o.bounds;
    if (!b) return {};
    if (typeof b === 'object' && !(b instanceof Element)) return b as Bounds;
    const el = typeof b === 'string' ? document.querySelector(b) : b;
    if (!el) return {};
    // Element bounds: how far the target can move (in transform space).
    const r = this.target.getBoundingClientRect();
    const c = el.getBoundingClientRect();
    return {
      minX: this.x + (c.left - r.left),
      maxX: this.x + (c.right - r.right),
      minY: this.y + (c.top - r.top),
      maxY: this.y + (c.bottom - r.bottom),
    };
  }

  private resist(v: number, min = -Infinity, max = Infinity): number {
    const k = this.o.edgeResistance ?? 0.35;
    if (v < min) return min - (min - v) * k;
    if (v > max) return max + (v - max) * k;
    return v;
  }

  private render(): void {
    const t = this.s.t;
    t[0] = this.x;
    t[1] = this.y;
    t[3] = this.rotation;
    this.s.dirty = false;
    this.target.style.transform = composeTransform(this.s);
  }

  private down = (e: PointerEvent): void => {
    if (!this.enabled || (e.button !== undefined && e.button > 0)) return;
    this.throwAnim?.stop();
    this.x = this.s.t[0];
    this.y = this.s.t[1];
    this.rotation = this.s.t[3];
    this.limits = this.computeBounds();
    const r = this.target.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    this.start = { px: e.clientX, py: e.clientY, x: this.x, y: this.y, r: this.rotation, angle: Math.atan2(e.clientY - cy, e.clientX - cx), cx, cy };
    this.isPressed = true;
    this.lock = null;
    this.vx.reset();
    this.vy.reset();
    this.vr.reset();
    try {
      (this.handleEl as HTMLElement).setPointerCapture?.(e.pointerId);
    } catch {
      /* synthetic events */
    }
    window.addEventListener('pointermove', this.move);
    window.addEventListener('pointerup', this.up);
    window.addEventListener('pointercancel', this.up);
    this.o.onPress?.(this, e);
  };

  private move = (e: PointerEvent): void => {
    if (!this.isPressed) return;
    const dx = e.clientX - this.start.px;
    const dy = e.clientY - this.start.py;
    if (!this.isDragging) {
      if (Math.hypot(dx, dy) < (this.o.minimumMovement ?? 3)) return;
      this.isDragging = true;
      (this.handleEl as HTMLElement).style.cursor = this.o.cursor ?? 'grabbing';
      this.o.onDragStart?.(this, e);
    }
    const type = this.o.type ?? 'x,y';
    const L = this.limits;
    if (type === 'rotate') {
      const a = Math.atan2(e.clientY - this.start.cy, e.clientX - this.start.cx);
      let d = ((a - this.start.angle) * 180) / Math.PI;
      if (d > 180) d -= 360;
      if (d < -180) d += 360;
      this.rotation = this.resist(this.start.r + d, L.minRotation, L.maxRotation);
      this.start.r = this.rotation;
      this.start.angle = a;
      this.vr.add(this.rotation, e.timeStamp || undefined);
    } else {
      if (this.o.lockAxis && !this.lock && Math.hypot(dx, dy) > 6) this.lock = Math.abs(dx) > Math.abs(dy) ? 'x' : 'y';
      if (type !== 'y' && this.lock !== 'y') this.x = this.resist(this.start.x + dx, L.minX, L.maxX);
      if (type !== 'x' && this.lock !== 'x') this.y = this.resist(this.start.y + dy, L.minY, L.maxY);
      this.vx.add(this.x, e.timeStamp || undefined);
      this.vy.add(this.y, e.timeStamp || undefined);
    }
    this.deltaX = dx;
    this.deltaY = dy;
    this.render();
    this.o.onDrag?.(this, e);
  };

  private up = (e: PointerEvent): void => {
    window.removeEventListener('pointermove', this.move);
    window.removeEventListener('pointerup', this.up);
    window.removeEventListener('pointercancel', this.up);
    const wasDragging = this.isDragging;
    this.isPressed = false;
    this.isDragging = false;
    (this.handleEl as HTMLElement).style.cursor = this.o.cursor ?? 'grab';
    this.o.onRelease?.(this, e);
    if (!wasDragging) return;
    this.o.onDragEnd?.(this, e);
    const L = this.limits;
    const snap = this.o.snap;
    const per = (axis: 'x' | 'y' | 'rotation'): SnapSpec | undefined =>
      snap === undefined ? undefined : typeof snap === 'object' && !Array.isArray(snap) ? (snap as any)[axis] : (snap as SnapSpec);
    const type = this.o.type ?? 'x,y';
    const props: Record<string, any> = {};
    const clampSnap = (axis: 'x' | 'y' | 'rotation', min?: number, max?: number) => (v: number) => {
      const r = resolveSnap(per(axis), v);
      return Math.min(max ?? Infinity, Math.max(min ?? -Infinity, r));
    };
    if (type === 'rotate') {
      props.rotate = { velocity: this.o.inertia ? this.vr.velocity : 0, min: L.minRotation, max: L.maxRotation, snap: per('rotation') !== undefined ? clampSnap('rotation', L.minRotation, L.maxRotation) : undefined };
    } else {
      if (type !== 'y') props.x = { velocity: this.o.inertia ? this.vx.velocity : 0, min: L.minX, max: L.maxX, snap: per('x') !== undefined ? clampSnap('x', L.minX, L.maxX) : undefined };
      if (type !== 'x') props.y = { velocity: this.o.inertia ? this.vy.velocity : 0, min: L.minY, max: L.maxY, snap: per('y') !== undefined ? clampSnap('y', L.minY, L.maxY) : undefined };
    }
    const outOfBounds =
      this.x < (L.minX ?? -Infinity) || this.x > (L.maxX ?? Infinity) ||
      this.y < (L.minY ?? -Infinity) || this.y > (L.maxY ?? Infinity) ||
      this.rotation < (L.minRotation ?? -Infinity) || this.rotation > (L.maxRotation ?? Infinity);
    if (this.o.inertia || snap !== undefined || outOfBounds) {
      this.throwAnim = inertia(this.target, props, {
        onUpdate: () => {
          this.x = this.s.t[0];
          this.y = this.s.t[1];
          this.rotation = this.s.t[3];
          this.o.onThrowUpdate?.(this);
        },
        onComplete: () => this.o.onThrowComplete?.(this),
      });
    }
  };

  /** Move programmatically (respects nothing — use for resets). */
  setPosition(x: number, y: number, rotation = this.rotation): void {
    this.x = x;
    this.y = y;
    this.rotation = rotation;
    this.render();
  }
  enable(): void {
    this.enabled = true;
  }
  disable(): void {
    this.enabled = false;
  }
  kill(): void {
    this.throwAnim?.stop();
    this.handleEl.removeEventListener('pointerdown', this.down as EventListener);
    window.removeEventListener('pointermove', this.move);
    window.removeEventListener('pointerup', this.up);
  }
  revert(): void {
    this.kill();
    this.setPosition(0, 0, 0);
  }
}

/** Make elements draggable. Returns one {@link Draggable} per element. */
export function draggable(targets: Targets, options?: DraggableOptions): Draggable[] {
  return (toTargets(targets) as HTMLElement[]).map((el) => new Draggable(el, options));
}

// ---------------------------------------------------------------------------
// Observer
// ---------------------------------------------------------------------------

export interface ObserverEvent {
  deltaX: number;
  deltaY: number;
  velocityX: number;
  velocityY: number;
  event: Event;
}

export interface ObserverOptions {
  target?: Element | Window | string;
  /** Comma separated: 'wheel', 'touch', 'pointer', 'scroll'. Default 'wheel,touch,pointer'. */
  type?: string;
  /** Minimum accumulated movement before firing directional callbacks. Default 10. */
  tolerance?: number;
  /** Prevent default browser handling (e.g. page scroll) for wheel/touch. */
  preventDefault?: boolean;
  /** Invert wheel direction semantics. */
  wheelSpeed?: number;
  onUp?: (e: ObserverEvent) => void;
  onDown?: (e: ObserverEvent) => void;
  onLeft?: (e: ObserverEvent) => void;
  onRight?: (e: ObserverEvent) => void;
  onChange?: (e: ObserverEvent) => void;
  onPress?: (e: ObserverEvent) => void;
  onRelease?: (e: ObserverEvent) => void;
  onDrag?: (e: ObserverEvent) => void;
  /** Fires once input has been quiet for 150ms. */
  onStop?: (e: ObserverEvent) => void;
}

/**
 * Normalise wheel, touch, pointer and scroll input into intent callbacks
 * (`onUp`/`onDown`/`onLeft`/`onRight`) with deltas and velocities — the basis
 * for full-page section transitions, carousels and custom scroll experiences.
 * Direction naming follows the content: scrolling/swiping to see what's
 * below is `onDown`.
 */
export function observer(options: ObserverOptions) {
  const target: Element | Window =
    typeof options.target === 'string' ? document.querySelector(options.target) ?? window : options.target ?? window;
  const types = new Set((options.type ?? 'wheel,touch,pointer').split(/[\s,]+/));
  const tol = options.tolerance ?? 10;
  let ax = 0;
  let ay = 0;
  const vx = new VelocityTracker();
  const vy = new VelocityTracker();
  let px = 0;
  let py = 0;
  let pressed = false;
  let lastScroll = target === window ? window.scrollY : (target as Element).scrollTop;
  let stopTimer: ReturnType<typeof setTimeout> | null = null;
  let posX = 0;
  let posY = 0;

  const emit = (dx: number, dy: number, event: Event) => {
    posX += dx;
    posY += dy;
    vx.add(posX);
    vy.add(posY);
    ax += dx;
    ay += dy;
    const ev: ObserverEvent = { deltaX: dx, deltaY: dy, velocityX: vx.velocity, velocityY: vy.velocity, event };
    options.onChange?.(ev);
    if (Math.abs(ay) >= tol) {
      (ay > 0 ? options.onDown : options.onUp)?.(ev);
      ay = 0;
    }
    if (Math.abs(ax) >= tol) {
      (ax > 0 ? options.onRight : options.onLeft)?.(ev);
      ax = 0;
    }
    if (stopTimer) clearTimeout(stopTimer);
    stopTimer = setTimeout(() => {
      ax = ay = 0;
      options.onStop?.(ev);
    }, 150);
  };

  const onWheel = (e: Event) => {
    const w = e as WheelEvent;
    if (options.preventDefault) w.preventDefault();
    const k = (w.deltaMode === 1 ? 40 : w.deltaMode === 2 ? 800 : 1) * (options.wheelSpeed ?? 1);
    emit(w.deltaX * k, w.deltaY * k, e);
  };
  const point = (e: Event): [number, number] => {
    const t = (e as TouchEvent).touches?.[0] ?? (e as TouchEvent).changedTouches?.[0];
    const p = (t ?? e) as { clientX: number; clientY: number };
    return [p.clientX, p.clientY];
  };
  const onDownEv = (e: Event) => {
    pressed = true;
    [px, py] = point(e);
    options.onPress?.({ deltaX: 0, deltaY: 0, velocityX: 0, velocityY: 0, event: e });
  };
  const onMoveEv = (e: Event) => {
    if (!pressed) return;
    if (options.preventDefault && e.cancelable) e.preventDefault();
    const [x, y] = point(e);
    // Dragging content upward reveals what's below → "down".
    const dx = px - x;
    const dy = py - y;
    px = x;
    py = y;
    options.onDrag?.({ deltaX: -dx, deltaY: -dy, velocityX: vx.velocity, velocityY: vy.velocity, event: e });
    emit(dx, dy, e);
  };
  const onUpEv = (e: Event) => {
    if (!pressed) return;
    pressed = false;
    options.onRelease?.({ deltaX: 0, deltaY: 0, velocityX: vx.velocity, velocityY: vy.velocity, event: e });
  };
  const onScroll = (e: Event) => {
    const s = target === window ? window.scrollY : (target as Element).scrollTop;
    emit(0, s - lastScroll, e);
    lastScroll = s;
  };

  const listeners: Array<[EventTarget, string, EventListener, AddEventListenerOptions?]> = [];
  const on = (t: EventTarget, ev: string, fn: EventListener, opt?: AddEventListenerOptions) => {
    t.addEventListener(ev, fn, opt);
    listeners.push([t, ev, fn, opt]);
  };
  const passive = { passive: !options.preventDefault };
  if (types.has('wheel')) on(target, 'wheel', onWheel, passive);
  if (types.has('touch')) {
    on(target, 'touchstart', onDownEv, { passive: true });
    on(target, 'touchmove', onMoveEv, passive);
    on(target, 'touchend', onUpEv);
  }
  if (types.has('pointer')) {
    on(target, 'pointerdown', (e) => (e as PointerEvent).pointerType === 'mouse' && onDownEv(e));
    on(window, 'pointermove', (e) => (e as PointerEvent).pointerType === 'mouse' && onMoveEv(e));
    on(window, 'pointerup', (e) => (e as PointerEvent).pointerType === 'mouse' && onUpEv(e));
  }
  if (types.has('scroll')) on(target, 'scroll', onScroll, { passive: true });
  const api = {
    kill() {
      for (const [t, ev, fn, opt] of listeners.splice(0)) t.removeEventListener(ev, fn, opt);
      if (stopTimer) clearTimeout(stopTimer);
    },
    revert() {
      api.kill();
    },
  };
  track(api);
  return api;
}

// ---------------------------------------------------------------------------
// Gestures
// ---------------------------------------------------------------------------

type Gesture = (el: Element, e: PointerEvent) => void | ((e: PointerEvent) => void);

function gesture(targets: Targets, startEv: string, endEvs: string[], fn: Gesture, filter: (e: PointerEvent) => boolean) {
  const offs: Array<() => void> = [];
  for (const el of toTargets(targets) as Element[]) {
    let end: ((e: PointerEvent) => void) | void;
    let active = false;
    const onStart = (e: Event) => {
      const pe = e as PointerEvent;
      if (!filter(pe) || active) return;
      active = true;
      end = fn(el, pe);
      const finish = (ev: Event) => {
        active = false;
        for (const name of endEvs) (name === 'pointerleave' ? el : window).removeEventListener(name, finish);
        if (typeof end === 'function') end(ev as PointerEvent);
      };
      for (const name of endEvs) (name === 'pointerleave' ? el : window).addEventListener(name, finish);
    };
    el.addEventListener(startEv, onStart);
    offs.push(() => el.removeEventListener(startEv, onStart));
  }
  const api = { kill: () => offs.splice(0).forEach((f) => f()), revert: () => api.kill() };
  track(api);
  return api;
}

/** Hover (mouse/pen only — no sticky touch hovers). `fn` may return an end callback. */
export function hover(targets: Targets, fn: Gesture) {
  return gesture(targets, 'pointerenter', ['pointerleave'], fn, (e) => e.pointerType !== 'touch');
}

/** Press (all pointer types). `fn` may return a release callback. */
export function press(targets: Targets, fn: Gesture) {
  return gesture(targets, 'pointerdown', ['pointerup', 'pointercancel'], fn, (e) => !e.button);
}
