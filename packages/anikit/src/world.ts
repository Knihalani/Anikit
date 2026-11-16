/**
 * `world()` — a 2D physics world running in the WebAssembly core: thousands
 * of colliding bodies, ropes, cloth, static boxes, pointer forces and
 * dragging. Bind DOM elements to bodies, or render the raw `Float32Array`
 * yourself (canvas / WebGL) with zero per-body JS overhead.
 *
 * ```ts
 * const w = world({ container: '#stage', gravity: [0, 1400], restitution: 0.3 });
 * w.bind('.ball');                       // DOM elements become circular bodies
 * w.pointer({ mode: 'repel', radius: 140 });
 * w.dragging();                          // grab and throw bodies
 * const rope = w.rope([[100, 40], [400, 40]], { segments: 24, pinStart: true });
 * ```
 */
import { core, whenReady, F64, f64Buf, W, views } from './wasm';
import { engine } from './core';
import { Targets, toTargets, stateOf, markDirty, flushTransforms, TargetState } from './props';
import { track } from './context';
import { warn, checkKeys, selectorHint } from './debug';
import type { OptionKeys } from './debug';

export interface WorldOptions {
  /** px/s² (default `[0, 980]`). */
  gravity?: [number, number];
  /** Linear drag per second (default 0.05). */
  damping?: number;
  /** Solver sub-steps per frame (default 4; raise for stiff stacks / cloth). */
  substeps?: number;
  /** Constraint iterations per sub-step (default 2). */
  iterations?: number;
  /** Body–body bounciness 0–1 (default 0.2). */
  restitution?: number;
  /** Bounciness against walls/boxes 0–1 (default 0.3). */
  wallRestitution?: number;
  /** Contact friction 0–1 (default 0.05). */
  friction?: number;
  /** Element whose box is the world's coordinate space and bounds. */
  container?: Element | string;
  /** Explicit bounds `[x0, y0, x1, y1]`, `'container'` (default with a container) or `false`. */
  bounds?: [number, number, number, number] | 'container' | false;
  /** Step automatically every frame (default true). */
  autoStep?: boolean;
}

const WORLD_KEYS: OptionKeys<WorldOptions> = {
  gravity: 0, damping: 0, substeps: 0, iterations: 0, restitution: 0, wallRestitution: 0, friction: 0, container: 0, bounds: 0, autoStep: 0,
};

export interface BodyInit {
  /** Centre x (px, world space). */
  x: number;
  /** Centre y (px, world space). */
  y: number;
  /** Radius (px). */
  r: number;
  /** <= 0 → static. Default: area-proportional. */
  mass?: number;
}

interface Bound {
  i: number;
  s: TargetState;
  ox: number;
  oy: number;
  rotate: boolean;
}

export class World {
  readonly id: number;
  /** Number of bodies. */
  count = 0;
  private bound: Bound[] = [];
  private radii: number[] = [];
  private attractors: number[] = [];
  private pointerAttr: { mode: number; strength: number; radius: number; x: number; y: number; on: boolean } | null = null;
  private offFrame: (() => void) | null = null;
  private listeners: Array<() => void> = [];
  private stepCbs: Array<(w: World, dt: number) => void> = [];
  private view: Float32Array = new Float32Array(0);
  private buf: ArrayBuffer | null = null;
  private container: Element | null;
  private paused = false;
  /** Constraint pairs `[a, b]` (for drawing ropes/cloth). */
  readonly links: Array<[number, number]> = [];

  constructor(private o: WorldOptions = {}) {
    checkKeys('world()', o, WORLD_KEYS);
    const w = core();
    const g = o.gravity ?? [0, 980];
    this.id = w.ak_world_create(g[0], g[1], o.damping ?? 0.05, o.substeps ?? 4, o.iterations ?? 2, o.restitution ?? 0.2, o.wallRestitution ?? 0.3, o.friction ?? 0.05);
    const c = o.container;
    this.container = typeof c === 'string' ? document.querySelector(c) : c ?? null;
    if (typeof c === 'string' && !this.container) warn(`world(): container "${c}" matched no element, so the world has no DOM coordinate space or bounds.${selectorHint(c)}`);
    this.refreshBounds();
    if (this.container && typeof window !== 'undefined') {
      const onResize = () => this.refreshBounds();
      window.addEventListener('resize', onResize);
      this.listeners.push(() => window.removeEventListener('resize', onResize));
    }
    if (o.autoStep !== false) this.offFrame = engine.onFrame((_t, dt) => !this.paused && this.step(dt / 1000));
    track(this);
  }

  /** Update settings at runtime. */
  set(o: Omit<WorldOptions, 'container' | 'autoStep'>): this {
    Object.assign(this.o, o);
    const c = this.o;
    const g = c.gravity ?? [0, 980];
    core().ak_world_config(this.id, g[0], g[1], c.damping ?? 0.05, c.substeps ?? 4, c.iterations ?? 2, c.restitution ?? 0.2, c.wallRestitution ?? 0.3, c.friction ?? 0.05);
    if (o.bounds !== undefined) this.refreshBounds();
    return this;
  }

  /** Re-read the container size (called on resize). */
  refreshBounds(): void {
    const b = this.o.bounds;
    const w = core();
    if (Array.isArray(b)) w.ak_world_bounds(this.id, b[0], b[1], b[2], b[3]);
    else if (b !== false && this.container) {
      const r = this.container.getBoundingClientRect();
      w.ak_world_bounds(this.id, 0, 0, r.width, r.height);
    } else w.ak_world_bounds(this.id, NaN, NaN, NaN, NaN);
  }

  /** Add one body; returns its index. */
  add(x: number, y: number, r: number, mass?: number): number {
    return this.addMany([{ x, y, r, mass }]);
  }

  /** Add many bodies in one call; returns the first index. */
  addMany(bodies: BodyInit[] | ArrayLike<number>): number {
    const flat: number[] = [];
    if (Array.isArray(bodies) && typeof bodies[0] === 'object') {
      for (const b of bodies as BodyInit[]) flat.push(b.x, b.y, b.r, b.mass ?? Math.PI * b.r * b.r * 0.01);
    } else {
      const a = bodies as ArrayLike<number>;
      for (let i = 0; i < a.length; i++) flat.push(a[i]);
    }
    const n = flat.length / 4;
    const at = f64Buf(flat.length);
    F64.set(flat, at);
    const first = core().ak_world_add(this.id, n);
    for (let k = 0; k < n; k++) this.radii[first + k] = flat[k * 4 + 2];
    this.count += n;
    return first;
  }

  /**
   * Turn elements into circular bodies (radius from their size). Their
   * transforms follow the simulation; `rotate` makes them roll.
   */
  bind(targets: Targets, options: {
    /** Body radius in px (default: half the element's smaller side), or per element. */
    radius?: number | ((el: Element) => number);
    /** Mass (default: by area; <= 0 is static). */
    mass?: number;
    /** Roll the element with the body (default false). */
    rotate?: boolean;
  } = {}): number[] {
    const els = toTargets(targets) as HTMLElement[];
    const cr = this.container?.getBoundingClientRect() ?? { left: 0, top: 0 };
    const ids: number[] = [];
    for (const el of els) {
      const s = stateOf(el);
      const r = el.getBoundingClientRect();
      // Layout position (without our transform) of the element's centre.
      const cx = r.left - cr.left + r.width / 2 - s.t[0];
      const cy = r.top - cr.top + r.height / 2 - s.t[1];
      const rad = typeof options.radius === 'function' ? options.radius(el) : options.radius ?? Math.min(r.width, r.height) / 2;
      const i = this.add(cx + s.t[0], cy + s.t[1], rad, options.mass);
      this.bound.push({ i, s, ox: cx, oy: cy, rotate: options.rotate ?? false });
      ids.push(i);
    }
    return ids;
  }

  /** A static axis-aligned box obstacle. */
  box(x0: number, y0: number, x1: number, y1: number): this {
    core().ak_world_box(this.id, x0, y0, x1, y1);
    return this;
  }
  /** Make elements static obstacles (by their boxes). */
  obstacles(targets: Targets): this {
    const cr = this.container?.getBoundingClientRect() ?? { left: 0, top: 0 };
    for (const el of toTargets(targets) as Element[]) {
      const r = el.getBoundingClientRect();
      this.box(r.left - cr.left, r.top - cr.top, r.right - cr.left, r.bottom - cr.top);
    }
    return this;
  }

  /** Distance constraint (spring when `stiffness < 1`); `length` defaults to the current distance. */
  link(a: number, b: number, options: {
    /** Rest length in px (default: the current distance). */
    length?: number;
    /** 0…1, 1 = rigid (default 1). */
    stiffness?: number;
    /** Break when stretched beyond this factor of the rest length (default 0 = never). */
    tear?: number;
  } = {}): this {
    core().ak_world_constraint(this.id, a, b, options.length ?? -1, options.stiffness ?? 1, options.tear ?? 0);
    this.links.push([a, b]);
    return this;
  }

  /** A rope between points; returns body indices. */
  rope(
    points: Array<[number, number]>,
    o: {
      /** Number of links (default 20). */
      segments?: number;
      /** Radius of each body in px (default 3). */
      radius?: number;
      /** 0…1 (default 1). */
      stiffness?: number;
      /** Fix the first point in place. */
      pinStart?: boolean;
      /** Fix the last point in place. */
      pinEnd?: boolean;
      /** Break factor, as in `link()` (default never). */
      tear?: number;
    } = {},
  ): number[] {
    const seg = o.segments ?? 20;
    const pts: Array<[number, number]> = [];
    for (let k = 0; k <= seg; k++) {
      const t = k / seg;
      const f = t * (points.length - 1);
      const i = Math.min(points.length - 2, Math.floor(f));
      const u = f - i;
      pts.push([points[i][0] + (points[i + 1][0] - points[i][0]) * u, points[i][1] + (points[i + 1][1] - points[i][1]) * u]);
    }
    const r = o.radius ?? 3;
    const first = this.addMany(pts.map(([x, y], k) => ({ x, y, r, mass: (k === 0 && o.pinStart) || (k === seg && o.pinEnd) ? 0 : 1 })));
    const ids = pts.map((_, k) => first + k);
    for (let k = 0; k < seg; k++) this.link(ids[k], ids[k + 1], { stiffness: o.stiffness ?? 1, tear: o.tear });
    return ids;
  }

  /** A cloth grid; returns body indices row-major. */
  cloth(
    x: number, y: number, cols: number, rows: number, spacing: number,
    o: {
      /** Body radius in px (default spacing / 4). */
      radius?: number;
      /** Pin the top row: true (default) = every point, n = every nth, false = none. */
      pinTop?: boolean | number;
      /** 0…1 (default 1). */
      stiffness?: number;
      /** Break factor, as in `link()` (default never). */
      tear?: number;
    } = {},
  ): number[] {
    const bodies: BodyInit[] = [];
    const pinEvery = typeof o.pinTop === 'number' ? o.pinTop : o.pinTop === false ? 0 : 1;
    for (let rI = 0; rI < rows; rI++) {
      for (let c = 0; c < cols; c++) {
        const pinned = rI === 0 && pinEvery > 0 && c % pinEvery === 0;
        bodies.push({ x: x + c * spacing, y: y + rI * spacing, r: o.radius ?? spacing * 0.25, mass: pinned ? 0 : 1 });
      }
    }
    const first = this.addMany(bodies);
    const id = (c: number, rI: number) => first + rI * cols + c;
    for (let rI = 0; rI < rows; rI++) {
      for (let c = 0; c < cols; c++) {
        if (c + 1 < cols) this.link(id(c, rI), id(c + 1, rI), { stiffness: o.stiffness ?? 1, tear: o.tear });
        if (rI + 1 < rows) this.link(id(c, rI), id(c, rI + 1), { stiffness: o.stiffness ?? 1, tear: o.tear });
      }
    }
    return Array.from({ length: rows * cols }, (_, k) => first + k);
  }

  /** Add a persistent attractor (negative strength repels). Returns its index. */
  attract(x: number, y: number, strength = 3000, radius = 200): number {
    this.attractors.push(x, y, strength, radius);
    this.pushAttractors();
    return this.attractors.length / 4 - 1;
  }
  /** Move / change an attractor. */
  setAttractor(i: number, x: number, y: number, strength?: number, radius?: number): void {
    const a = this.attractors;
    a[i * 4] = x;
    a[i * 4 + 1] = y;
    if (strength !== undefined) a[i * 4 + 2] = strength;
    if (radius !== undefined) a[i * 4 + 3] = radius;
    this.pushAttractors();
  }
  clearAttractors(): void {
    this.attractors.length = 0;
    this.pushAttractors();
  }
  private pushAttractors(): void {
    // Runs on every pointer move: write straight into the buffer, no copy.
    const list = this.attractors;
    const p = this.pointerAttr;
    const on = !!(p && p.on);
    const n = list.length + (on ? 4 : 0);
    const at = f64Buf(n || 1);
    for (let i = 0; i < list.length; i++) F64[at + i] = list[i];
    if (on) {
      const k = at + list.length;
      F64[k] = p!.x;
      F64[k + 1] = p!.y;
      F64[k + 2] = p!.strength * p!.mode;
      F64[k + 3] = p!.radius;
    }
    core().ak_world_attractors(this.id, n / 4);
  }

  private local(e: { clientX: number; clientY: number }): [number, number] {
    const r = this.container?.getBoundingClientRect() ?? { left: 0, top: 0 };
    return [e.clientX - r.left, e.clientY - r.top];
  }

  /** The pointer attracts or repels bodies. */
  pointer(o: {
    /** Default `'repel'`. */
    mode?: 'attract' | 'repel';
    /** Force (default 4000). */
    strength?: number;
    /** Reach in px (default 150). */
    radius?: number;
  } = {}): this {
    this.pointerAttr = { mode: o.mode === 'attract' ? 1 : -1, strength: o.strength ?? 4000, radius: o.radius ?? 150, x: 0, y: 0, on: false };
    const target = (this.container ?? window) as HTMLElement;
    const move = (e: PointerEvent) => {
      const r = this.container?.getBoundingClientRect();
      const a = this.pointerAttr!;
      a.x = e.clientX - (r ? r.left : 0);
      a.y = e.clientY - (r ? r.top : 0);
      a.on = true;
      this.pushAttractors();
    };
    const leave = () => {
      this.pointerAttr!.on = false;
      this.pushAttractors();
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerleave', leave);
    this.listeners.push(() => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerleave', leave);
    });
    return this;
  }

  /** Grab bodies with the pointer and throw them. */
  dragging(o: {
    /** Extra px around bodies that still grab them (helps with small bodies on touch). */
    hitSlop?: number;
  } = {}): this {
    const target = (this.container ?? window) as HTMLElement;
    let held = -1;
    // Updated in place on every move.
    const last: [number, number, number] = [0, 0, 0];
    const vel: [number, number] = [0, 0];
    const down = (e: PointerEvent) => {
      const [x, y] = this.local(e);
      let i = core().ak_world_pick(this.id, x, y) >>> 0;
      if (i === 0xffffffff && o.hitSlop) {
        for (const [dx, dy] of [[o.hitSlop, 0], [-o.hitSlop, 0], [0, o.hitSlop], [0, -o.hitSlop]]) {
          i = core().ak_world_pick(this.id, x + dx, y + dy) >>> 0;
          if (i !== 0xffffffff) break;
        }
      }
      if (i === 0xffffffff) return;
      held = i;
      last[0] = x;
      last[1] = y;
      last[2] = e.timeStamp;
      vel[0] = vel[1] = 0;
      core().ak_world_set_pinned(this.id, i, 1);
      (e.target as Element).setPointerCapture?.(e.pointerId);
      e.preventDefault();
    };
    const move = (e: PointerEvent) => {
      if (held < 0) return;
      const r = this.container?.getBoundingClientRect();
      const x = e.clientX - (r ? r.left : 0);
      const y = e.clientY - (r ? r.top : 0);
      const dt = Math.max(1, e.timeStamp - last[2]) / 1000;
      vel[0] = (x - last[0]) / dt;
      vel[1] = (y - last[1]) / dt;
      last[0] = x;
      last[1] = y;
      last[2] = e.timeStamp;
      core().ak_world_set_position(this.id, held, x, y, 0);
    };
    const up = () => {
      if (held < 0) return;
      core().ak_world_set_pinned(this.id, held, 0);
      core().ak_world_set_velocity(this.id, held, vel[0], vel[1]);
      held = -1;
    };
    target.addEventListener('pointerdown', down);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    this.listeners.push(() => {
      target.removeEventListener('pointerdown', down);
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    });
    return this;
  }

  /** Radial velocity kick. */
  impulse(x: number, y: number, radius = 200, strength = 800): this {
    core().ak_world_impulse(this.id, x, y, radius, strength);
    return this;
  }
  setPosition(i: number, x: number, y: number, keepVelocity = false): this {
    core().ak_world_set_position(this.id, i, x, y, keepVelocity ? 1 : 0);
    return this;
  }
  setVelocity(i: number, vx: number, vy: number): this {
    core().ak_world_set_velocity(this.id, i, vx, vy);
    return this;
  }
  velocity(i: number): [number, number] {
    const p = core().ak_world_velocity(this.id, i) >>> 3;
    views();
    return [F64[p], F64[p + 1]];
  }
  pin(i: number, pinned = true): this {
    core().ak_world_set_pinned(this.id, i, pinned ? 1 : 0);
    return this;
  }
  /** Remove a body from the simulation (its index stays reserved). */
  remove(i: number): this {
    core().ak_world_set_active(this.id, i, 0);
    return this;
  }
  setRadius(i: number, r: number): this {
    core().ak_world_set_radius(this.id, i, r);
    return this;
  }
  /** Index of the body under a point, or -1. */
  pick(x: number, y: number): number {
    const i = core().ak_world_pick(this.id, x, y) >>> 0;
    return i === 0xffffffff ? -1 : i;
  }
  /** Which constraints are still intact (after tearing). */
  linksAlive(): boolean[] {
    const n = core().ak_world_constraints(this.id);
    views();
    const at = core().ak_f64_buf(0) >>> 3;
    return Array.from({ length: n }, (_, k) => F64[at + k * 3 + 2] === 1);
  }

  /** Live `[x, y, angle] * count` view (zero-copy). */
  get data(): Float32Array {
    const w = W!;
    const b = w.memory.buffer as ArrayBuffer;
    const n = this.count * 3;
    if (b !== this.buf || this.view.length !== n || this.view.byteOffset !== w.ak_world_ptr(this.id)) {
      this.buf = b;
      this.view = new Float32Array(b, w.ak_world_ptr(this.id), n);
    }
    return this.view;
  }
  x(i: number): number {
    return this.data[i * 3];
  }
  y(i: number): number {
    return this.data[i * 3 + 1];
  }
  angle(i: number): number {
    return this.data[i * 3 + 2];
  }
  get energy(): number {
    return core().ak_world_energy(this.id);
  }
  /** Broadphase pair tests in the last step (diagnostics). */
  get pairTests(): number {
    return core().ak_world_pair_tests(this.id);
  }

  /** Called after every step (draw here). */
  onStep(cb: (w: World, dt: number) => void): () => void {
    this.stepCbs.push(cb);
    return () => (this.stepCbs = this.stepCbs.filter((c) => c !== cb));
  }

  /** Advance by `dt` seconds (automatic unless `autoStep: false`). */
  step(dt: number): void {
    core().ak_world_step(this.id, dt);
    if (this.bound.length) {
      const d = this.data;
      for (const b of this.bound) {
        const s = b.s;
        s.t[0] = d[b.i * 3] - b.ox;
        s.t[1] = d[b.i * 3 + 1] - b.oy;
        if (b.rotate) s.t[3] = (d[b.i * 3 + 2] * 180) / Math.PI;
        if (!s.dirty) markDirty(s);
      }
      flushTransforms();
    }
    for (const cb of this.stepCbs) cb(this, dt);
  }

  /** Draw bodies as circles on a 2D canvas (quick default renderer). */
  draw(ctx: CanvasRenderingContext2D, o: {
    /** Fill colour, or per body index (default `'#6ea8ff'`). */
    color?: string | ((i: number) => string);
    /** Stroke colour for links (default: links not drawn). */
    links?: string;
    /** Clear the canvas first (default true). */
    clear?: boolean;
  } = {}): void {
    const d = this.data;
    if (o.clear !== false) ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
    if (o.links && this.links.length) {
      ctx.strokeStyle = o.links;
      ctx.beginPath();
      for (const [a, b] of this.links) {
        ctx.moveTo(d[a * 3], d[a * 3 + 1]);
        ctx.lineTo(d[b * 3], d[b * 3 + 1]);
      }
      ctx.stroke();
    }
    const fixed = typeof o.color === 'string' ? o.color : null;
    if (fixed) ctx.fillStyle = fixed;
    for (let i = 0; i < this.count; i++) {
      if (!fixed) ctx.fillStyle = (o.color as (i: number) => string)?.(i) ?? '#6ea8ff';
      ctx.beginPath();
      ctx.arc(d[i * 3], d[i * 3 + 1], this.radius(i), 0, Math.PI * 2);
      ctx.fill();
    }
  }
  /** Radius of body `i`. */
  radius(i: number): number {
    return this.radii[i] ?? 0;
  }

  pause(): this {
    this.paused = true;
    return this;
  }
  play(): this {
    this.paused = false;
    return this;
  }
  destroy(): void {
    this.offFrame?.();
    for (const off of this.listeners.splice(0)) off();
    whenReady(() => core().ak_world_free(this.id));
  }
  kill(): void {
    this.destroy();
  }
  revert(): void {
    this.destroy();
    for (const b of this.bound) {
      b.s.t[0] = b.s.t[1] = 0;
      if (b.rotate) b.s.t[3] = 0;
      if (!b.s.dirty) markDirty(b.s);
    }
    flushTransforms();
  }
}

/** Create a physics world (requires the core to be ready: `await ready` first in async setups). */
export function world(options?: WorldOptions): World {
  return new World(options);
}
