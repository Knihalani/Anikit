/**
 * Physics: `spring()` (retargetable, velocity-preserving) and `inertia()`
 * (momentum with bounds and snapping). Both run in the core with exact,
 * frame-rate independent closed-form solutions.
 *
 * ```ts
 * // Follow the pointer: every call retargets the running spring.
 * addEventListener('pointermove', (e) => spring(ball, { x: e.clientX, y: e.clientY }, { stiffness: 300, damping: 25 }));
 * // Fling with bounds and snapping.
 * inertia(card, { x: { velocity: vx, min: 0, max: 600, snap: 100 } });
 * ```
 */
import { core, whenReady, F64, f64Buf } from './wasm';
import { register, unregister, wake, NodeRecord, EV_FINISH, prefersReduced } from './core';
import {
  Target, Targets, toTargets, classify, Kind, readValue, writer, convertUnit, defaultUnit, isElement,
  claim, unclaim, Owner, transformUnit,
} from './props';
import { parseValue, parseColor, r3 } from './values';
import { Binding, bind, apply, custom } from './bind';
import { track } from './context';
import { describe, checkKeys, checkSeconds, checkTargets } from './debug';
import type { OptionKeys } from './debug';
import { snapshot } from './props';

export interface PhysicsOptions {
  /** Spring constant. Default 100; higher is snappier (pointer followers: ~300). */
  stiffness?: number;
  /** Friction. Default 10; higher settles with less overshoot (critically damped ≈ 2·√(stiffness·mass)). */
  damping?: number;
  /** Mass. Default 1. */
  mass?: number;
  /** Perceptual alternative to stiffness/damping: approximate settle time in seconds (not ms)… */
  duration?: number;
  /** …and bounciness, 0 (none) to 1 (very bouncy). Default 0.25 when either is given. */
  bounce?: number;
  /** Initial velocity (units/s); a number or per-property map. Defaults to the current velocity. */
  velocity?: number | Record<string, number>;
  /** Distance from target considered at rest. */
  restDelta?: number;
  /** Speed considered at rest (units/s). */
  restSpeed?: number;
  /** Every frame while moving. */
  onUpdate?: () => void;
  /** Came to rest (the controller is also awaitable: `await spring(…)`). */
  onComplete?: () => void;
}

const SPRING_KEYS: OptionKeys<PhysicsOptions> = {
  stiffness: 0, damping: 0, mass: 0, duration: 0, bounce: 0, velocity: 0, restDelta: 0, restSpeed: 0, onUpdate: 0, onComplete: 0,
};
const INERTIA_KEYS: OptionKeys<Omit<InertiaOptions, keyof PhysicsOptions>> = { power: 0, timeConstant: 0, bounceStiffness: 0, bounceDamping: 0 };
const BALLISTIC_KEYS: OptionKeys<BallisticOptions> = { friction: 0, restitution: 0, life: 0, onUpdate: 0, onComplete: 0 };

function params(o: PhysicsOptions): [number, number, number] {
  if (o.bounce !== undefined || (o.duration !== undefined && o.stiffness === undefined)) {
    const d = Math.max(0.001, o.duration ?? 0.5);
    const zeta = Math.min(1, Math.max(0.05, 1 - (o.bounce ?? 0.25)));
    const w0 = (2 * Math.PI) / (d * 1.2);
    return [w0 * w0, 2 * zeta * w0, 1];
  }
  return [o.stiffness ?? 100, o.damping ?? 10, o.mass ?? 1];
}

/** One running simulation for one (target, property). */
class Sim implements NodeRecord, Owner {
  h = 0;
  at = 0;
  done: Array<() => void> = [];
  listeners: PhysicsOptions[] = [];
  constructor(
    public target: Target,
    public prop: string,
    public ch: number,
    public b: Binding,
  ) {}
  render(): void {
    apply(this.b, this.at);
    const ls = this.listeners;
    for (let i = 0; i < ls.length; i++) ls[i].onUpdate?.();
  }
  event(kind: number): void {
    if (kind === EV_FINISH) {
      const d = this.done;
      this.done = [];
      const ls = this.listeners;
      this.listeners = [];
      for (const l of ls) l.onComplete?.();
      for (const f of d) f();
    }
  }
  release(): void {
    this.dispose();
  }
  info(): Record<string, unknown> {
    return { type: 'physics', target: describe(this.target), props: [this.prop] };
  }
  dispose(): void {
    if (!this.h) return;
    unclaim(this.target, this.prop, this);
    unregister(this);
    core().ak_release(this.h);
    sims.get(this.target)?.delete(this.prop);
    this.h = 0;
    this.event(EV_FINISH);
  }
}

const sims = new WeakMap<object, Map<string, Sim>>();

interface Resolved {
  values: number[];
  b: Binding;
}

/** Resolve a numeric/colour value + writer for a target property. */
function resolveProp(target: Target, prop: string, value: unknown): Resolved & { current: number[] } {
  const kind = classify(target, prop);
  const cur = parseValue(readValue(target, prop, kind));
  const el = isElement(target) ? target : null;
  if (typeof value === 'string' && parseColor(value)) {
    const to = parseColor(value)!;
    const from = cur.k === 'color' ? cur.c : to;
    const w = writer(target, prop, kind, '');
    return {
      values: to.slice(),
      current: from.slice(),
      b: custom(target, prop, 0, 0, (at) => {
        const a = F64[at + 3];
        w(`rgba(${Math.round(F64[at])},${Math.round(F64[at + 1])},${Math.round(F64[at + 2])},${r3(Math.min(1, Math.max(0, a)))})`);
      }),
    };
  }
  const p = parseValue(value);
  let unit = p.k === 'num' ? p.unit : '';
  if (!unit) unit = kind === Kind.Transform ? transformUnit(prop) : defaultUnit(prop, kind);
  if (kind === Kind.Transform && transformUnit(prop) !== 'px') unit = transformUnit(prop);
  let to = p.k === 'num' ? p.n : 0;
  if (p.k === 'num' && p.rel) {
    const base = cur.k === 'num' ? cur.n : 0;
    to = p.rel === '+=' ? base + p.n : p.rel === '-=' ? base - p.n : base * p.n;
  }
  if (p.k === 'num' && p.unit && p.unit !== unit) to = convertUnit(el, prop, to, p.unit, unit);
  let from = cur.k === 'num' ? cur.n : 0;
  if (cur.k === 'num' && cur.unit && unit && cur.unit !== unit) from = convertUnit(el, prop, from, cur.unit, unit);
  const fmt = (at: number) => r3(F64[at]) + unit;
  return {
    values: [to],
    current: [from],
    b: bind(target, prop, kind, 0, true, unit, fmt),
  };
}

/** Handle returned by {@link spring} and {@link inertia}. */
export class PhysicsAnimation implements PromiseLike<void> {
  /** @internal */
  _sims: Sim[] = [];
  private _done: Promise<void>;
  private _resolve!: () => void;
  constructor() {
    this._done = new Promise((r) => (this._resolve = r));
    track(this);
  }
  /** @internal */
  _track(list: Sim[]): void {
    this._sims = list;
    for (const sm of list) this._restore.push(snapshot(sm.target, sm.prop, classify(sm.target, sm.prop)));
    let left = list.length;
    if (!left) this._resolve();
    for (const s of list) s.done.push(() => --left === 0 && this._resolve());
  }
  get finished(): Promise<void> {
    return this._done;
  }
  then<A = void, B = never>(ok?: ((v: void) => A | PromiseLike<A>) | null, err?: ((e: unknown) => B | PromiseLike<B>) | null): Promise<A | B> {
    return this._done.then(ok, err);
  }
  /** Current velocity per property (units/s) of the first target. */
  get velocity(): Record<string, number> {
    const out: Record<string, number> = {};
    for (const s of this._sims) {
      if (!s.h) continue;
      const p = core().ak_sim_velocity_ptr(s.h) >>> 3;
      if (!(s.prop in out)) out[s.prop] = F64[p];
    }
    return out;
  }
  /** Stop and restore the animated properties to their values before this started. */
  revert(): void {
    whenReady(() => {
      for (const s of this._sims) s.dispose();
      for (const r of this._restore.splice(0).reverse()) r();
    });
  }
  /** Alias of {@link stop}. */
  kill(): void {
    this.stop();
  }
  /** @internal */
  _restore: Array<() => void> = [];
  /** Stop all simulations where they are. */
  stop(): void {
    whenReady(() => {
      for (const s of this._sims) s.dispose();
    });
  }
}

/**
 * Animate with a physical spring. Calling again for the same target/property
 * retargets the running simulation, preserving momentum.
 */
export function spring(targets: Targets, props: Record<string, number | string>, options: PhysicsOptions = {}): PhysicsAnimation {
  checkKeys('spring()', options, SPRING_KEYS);
  checkSeconds('spring()', 'duration', options.duration);
  const ctl = new PhysicsAnimation();
  whenReady(() => {
    const w = core();
    const [k, c, m] = params(options);
    const list: Sim[] = [];
    const ts = toTargets(targets);
    checkTargets('spring()', targets, ts.length);
    for (const target of ts) {
      let map = sims.get(target);
      if (!map) sims.set(target, (map = new Map()));
      for (const prop of Object.keys(props)) {
        const r = resolveProp(target, prop, props[prop]);
        const ch = r.values.length;
        const vOpt = options.velocity;
        const vel = typeof vOpt === 'number' ? vOpt : vOpt?.[prop];
        let sim = map.get(prop);
        if (sim && sim.h && sim.ch === ch) {
          // Retarget, keeping position and velocity.
          const at = f64Buf(ch);
          for (let i = 0; i < ch; i++) F64[at + i] = r.values[i];
          w.ak_sim_target(sim.h, ch);
          w.ak_sim_params(sim.h, k, c, m);
          if (vel !== undefined) {
            const b = f64Buf(ch * 2);
            const pos = core().ak_values_ptr(sim.h) >>> 3;
            for (let i = 0; i < ch; i++) {
              F64[b + i] = F64[pos + i];
              F64[b + ch + i] = vel;
            }
            w.ak_sim_position(sim.h, ch, 1);
          }
        } else {
          sim?.dispose();
          const at = f64Buf(ch * 3);
          for (let i = 0; i < ch; i++) {
            F64[at + i] = r.current[i];
            F64[at + ch + i] = vel ?? 0;
            F64[at + 2 * ch + i] = r.values[i];
          }
          sim = new Sim(target, prop, ch, r.b);
          sim.h = w.ak_spring_create(ch, k, c, m);
          register(sim);
          claim(target, prop, sim);
          map.set(prop, sim);
        }
        if (prefersReduced()) {
          // Reduced motion: arrive at once (at rest on the target).
          const b = f64Buf(ch * 2);
          for (let i = 0; i < ch; i++) {
            F64[b + i] = r.values[i];
            F64[b + ch + i] = 0;
          }
          w.ak_sim_position(sim.h, ch, 1);
        }
        const unitScale = ch === 4 ? 1 : 1;
        w.ak_sim_rest(sim.h, options.restDelta ?? 0.01 * unitScale, options.restSpeed ?? 0.1 * unitScale);
        if ((options.onUpdate || options.onComplete) && !sim.listeners.includes(options)) sim.listeners.push(options);
        list.push(sim);
      }
    }
    ctl._track(list);
    wake();
  });
  return ctl;
}

export interface InertiaProp {
  /** Initial velocity in units per second (e.g. a pointer's release velocity). */
  velocity: number;
  /** Lower bound: the value springs back if it glides past. */
  min?: number;
  /** Upper bound: the value springs back if it glides past. */
  max?: number;
  /** Snap the resting point: a step, a list of points, or a function. */
  snap?: number | number[] | ((natural: number) => number);
}

export interface InertiaOptions extends PhysicsOptions {
  /** Multiplier on how far momentum carries (default 0.8). */
  power?: number;
  /** Decay time constant in seconds (default 0.35). */
  timeConstant?: number;
  /** Spring used when hitting a bound. */
  bounceStiffness?: number;
  /** Damping of that bound spring (default 40). */
  bounceDamping?: number;
}

/** Glide with momentum, slowing down naturally; bounce off bounds; snap. */
export function inertia(targets: Targets, props: Record<string, InertiaProp | number>, options: InertiaOptions = {}): PhysicsAnimation {
  checkKeys('inertia()', options, SPRING_KEYS, INERTIA_KEYS);
  const ctl = new PhysicsAnimation();
  whenReady(() => {
    const w = core();
    const power = options.power ?? 0.8;
    const tc = (options.timeConstant ?? 0.35) * 1000;
    const list: Sim[] = [];
    const ts = toTargets(targets);
    checkTargets('inertia()', targets, ts.length);
    for (const target of ts) {
      let map = sims.get(target);
      if (!map) sims.set(target, (map = new Map()));
      for (const prop of Object.keys(props)) {
        const spec0 = typeof props[prop] === 'number' ? { velocity: props[prop] as number } : (props[prop] as InertiaProp);
        // Reduced motion: no gliding (bounds and snapping still apply).
        const spec = prefersReduced() ? { ...spec0, velocity: 0 } : spec0;
        const kind = classify(target, prop);
        const cur = parseValue(readValue(target, prop, kind));
        const pos = cur.k === 'num' ? cur.n : 0;
        const unit = kind === Kind.Transform ? transformUnit(prop) : defaultUnit(prop, kind);
        const b = bind(target, prop, kind, 0, true, unit, (at: number) => r3(F64[at]) + unit);
        let target_: number | undefined;
        if (spec.snap !== undefined) {
          const natural = pos + power * spec.velocity * (tc / 1000);
          const sn = spec.snap;
          target_ =
            typeof sn === 'function' ? sn(natural)
            : Array.isArray(sn) ? sn.reduce((a, b) => (Math.abs(b - natural) < Math.abs(a - natural) ? b : a))
            : Math.round(natural / sn) * sn;
          if (spec.min !== undefined) target_ = Math.max(spec.min, target_);
          if (spec.max !== undefined) target_ = Math.min(spec.max, target_);
        }
        map.get(prop)?.dispose();
        const has = [target_ !== undefined, spec.min !== undefined, spec.max !== undefined];
        let at = f64Buf(5);
        F64[at++] = pos;
        F64[at++] = spec.velocity;
        if (has[0]) F64[at++] = target_!;
        if (has[1]) F64[at++] = spec.min!;
        if (has[2]) F64[at++] = spec.max!;
        const sim = new Sim(target, prop, 1, b);
        sim.h = w.ak_decay_create(
          1, power, tc, options.bounceStiffness ?? 500, options.bounceDamping ?? 40, 1,
          +has[0], +has[1], +has[2],
        );
        register(sim);
        claim(target, prop, sim);
        map.set(prop, sim);
        w.ak_sim_rest(sim.h, options.restDelta ?? 0.01, options.restSpeed ?? 0.1);
        if (options.onUpdate || options.onComplete) sim.listeners.push(options);
        list.push(sim);
      }
    }
    ctl._track(list);
    wake();
  });
  return ctl;
}

export interface PhysicsProp {
  /** Initial velocity (units/s). */
  velocity?: number;
  /** Constant acceleration (units/s²), e.g. gravity `980`. */
  acceleration?: number;
  /** Bounds that the value bounces off (see `restitution`). */
  min?: number;
  /** Upper bound the value bounces off. */
  max?: number;
}

export interface BallisticOptions {
  /** Linear drag (1/s); 0 = none. */
  friction?: number;
  /** Bounciness at bounds, 0 (stop) – 1 (perfectly elastic). Default 0.5. */
  restitution?: number;
  /** Stop after this many seconds. */
  life?: number;
  /** Every frame while moving. */
  onUpdate?: () => void;
  /** Came to rest (or `life` ran out). */
  onComplete?: () => void;
}

/**
 * Ballistic motion: velocity, gravity/wind, drag and bouncing bounds — solved
 * exactly in the core (confetti, throws, falling objects, particle bursts).
 *
 * ```ts
 * physics(ball, { x: { velocity: 300 }, y: { velocity: -600, acceleration: 1500, max: floorY } }, { restitution: 0.6 });
 * ```
 */
export function physics(targets: Targets, props: Record<string, PhysicsProp | number>, options: BallisticOptions = {}): PhysicsAnimation {
  checkKeys('physics()', options, BALLISTIC_KEYS);
  checkSeconds('physics()', 'life', options.life);
  const ctl = new PhysicsAnimation();
  whenReady(() => {
    const w = core();
    const list: Sim[] = [];
    const ts = toTargets(targets);
    checkTargets('physics()', targets, ts.length);
    for (const target of ts) {
      let map = sims.get(target);
      if (!map) sims.set(target, (map = new Map()));
      for (const prop of Object.keys(props)) {
        const spec: PhysicsProp = typeof props[prop] === 'number' ? { velocity: props[prop] as number } : (props[prop] as PhysicsProp);
        const kind = classify(target, prop);
        const cur = parseValue(readValue(target, prop, kind));
        const unit = kind === Kind.Transform ? transformUnit(prop) : defaultUnit(prop, kind);
        const b = bind(target, prop, kind, 0, true, unit, (at: number) => r3(F64[at]) + unit);
        map.get(prop)?.dispose();
        const hasMin = spec.min !== undefined;
        const hasMax = spec.max !== undefined;
        let at = f64Buf(5);
        F64[at++] = cur.k === 'num' ? cur.n : 0;
        F64[at++] = spec.velocity ?? 0;
        F64[at++] = spec.acceleration ?? 0;
        if (hasMin) F64[at++] = spec.min!;
        if (hasMax) F64[at++] = spec.max!;
        const sim = new Sim(target, prop, 1, b);
        sim.h = w.ak_physics_create(1, options.friction ?? 0, options.restitution ?? 0.5, (options.life ?? 0) * 1000, +hasMin, +hasMax);
        register(sim);
        claim(target, prop, sim);
        map.set(prop, sim);
        if (options.onUpdate || options.onComplete) sim.listeners.push(options);
        list.push(sim);
      }
    }
    ctl._track(list);
    wake();
  });
  return ctl;
}
