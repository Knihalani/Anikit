/**
 * Engine glue: node records, the frame loop, event dispatch and easing
 * registration. One `ak_tick` call per frame; afterwards only the nodes the
 * core reports as updated are touched.
 */
import { W, F64, U32, views, core, whenReady, writeStr, f64Buf, LINEAR, EASING_ERROR } from './wasm';

export const INDEX_MASK = (1 << 20) - 1;

/** Event kinds (mirror `anikit_core::engine::event`). */
export const EV_START = 1;
export const EV_REPEAT = 2;
export const EV_COMPLETE = 3;
export const EV_REVERSE_COMPLETE = 4;
export const EV_CALLBACK = 5;
export const EV_FINISH = 6;

/** Node flags (mirror `anikit_core::engine::flags`). */
export const FL_FILL_BACKWARDS = 1;
export const FL_FILL_FORWARDS = 2;
export const FL_IMMEDIATE = 4;
export const FL_AUTO_RELEASE = 8;

export interface NodeRecord {
  /** Engine handle. */
  h: number;
  /** F64 index of the node's output values (0 if none). */
  at: number;
  /** Apply freshly computed values. */
  render?(): void;
  /** Discrete engine events. */
  event?(kind: number): void;
}

const records: Array<NodeRecord | undefined> = [];

export function register(rec: NodeRecord): void {
  records[rec.h & INDEX_MASK] = rec;
  if (rec.at === 0 && rec.render) {
    const p = core().ak_values_ptr(rec.h);
    rec.at = p >>> 3;
  }
}

export function unregister(rec: NodeRecord): void {
  const i = rec.h & INDEX_MASK;
  if (records[i] === rec) records[i] = undefined;
}

// ---------------------------------------------------------------------------
// Post-render hooks (transform flushing etc.) and deferred user callbacks
// ---------------------------------------------------------------------------

const flushHooks: Array<() => void> = [];
/** Register work that runs after all values of a frame are applied. */
export function onFlush(fn: () => void): void {
  flushHooks.push(fn);
}

let deferred: Array<() => void> = [];
/** Queue a user callback to run after this frame's DOM writes. */
export function defer(fn: () => void): void {
  deferred.push(fn);
}

const evScratch: number[] = [];

/**
 * Apply everything the core produced since the last flush: values of updated
 * nodes, then post-render hooks, then events and user callbacks.
 */
export function flush(): void {
  const w = W;
  if (!w) return;
  views();
  const n = w.ak_updated_len();
  if (n > 0) {
    const up = w.ak_updated_ptr() >>> 2;
    for (let i = 0; i < n; i++) {
      const h = U32[up + i];
      const r = records[h & INDEX_MASK];
      if (r !== undefined && r.h === h && r.render !== undefined) r.render();
    }
  }
  for (let i = 0; i < flushHooks.length; i++) flushHooks[i]();
  // Copy events before dispatching: callbacks may create or seek animations.
  const ne = w.ak_events_len();
  let count = 0;
  if (ne > 0) {
    views();
    const ep = w.ak_events_ptr() >>> 2;
    for (let i = 0; i < ne; i++) evScratch[i] = U32[ep + i];
    count = ne;
  }
  w.ak_clear_frame();
  for (let i = 0; i < count; i += 2) {
    const h = evScratch[i];
    const r = records[h & INDEX_MASK];
    if (r !== undefined && r.h === h && r.event !== undefined) r.event(evScratch[i + 1]);
  }
  if (deferred.length) {
    const d = deferred;
    deferred = [];
    for (let i = 0; i < d.length; i++) d[i]();
  }
}

// ---------------------------------------------------------------------------
// Ticker
// ---------------------------------------------------------------------------

type FrameCallback = (time: number, dt: number) => void;
const frameCallbacks = new Set<FrameCallback>();
const raf: (cb: (t: number) => void) => number =
  typeof requestAnimationFrame === 'function'
    ? (cb) => requestAnimationFrame(cb)
    : (cb) => setTimeout(() => cb(now()), 16) as unknown as number;
const now = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

let running = false;
let manual = false;
let last = 0;
let clock = 0;
let maxFps = 0;

function loop(ts: number): void {
  if (manual) {
    running = false;
    return;
  }
  let dt = ts - last;
  if (dt < 0) dt = 0;
  if (maxFps > 0 && dt < 1000 / maxFps - 1) {
    raf(loop);
    return;
  }
  last = ts;
  step(dt);
  if (W!.ak_active_count() > 0 || frameCallbacks.size > 0 || pendingWake) {
    pendingWake = false;
    raf(loop);
  } else {
    running = false;
  }
}

let pendingWake = false;

function step(dt: number): void {
  const w = core();
  clock += dt;
  w.ak_tick(dt);
  flush();
  if (frameCallbacks.size) for (const cb of frameCallbacks) cb(clock, dt);
}

/** Make sure the frame loop is running (no-op in manual mode). */
export function wake(): void {
  if (manual) return;
  if (running) {
    pendingWake = true;
    return;
  }
  running = true;
  last = now();
  raf(loop);
}

// ---------------------------------------------------------------------------
// Public engine controls
// ---------------------------------------------------------------------------

export const engine = {
  /**
   * Drive the engine yourself (tests, SSR, video export, custom loops).
   * `engine.tick(ms)` then advances time.
   */
  manual(on = true): void {
    manual = on;
    if (!on) wake();
  },
  /** Advance time by `ms` milliseconds and apply results (manual mode). */
  tick(ms = 1000 / 60): void {
    step(ms);
  },
  /** Global time scale (1 = normal, 0.5 = slow motion). */
  set speed(r: number) {
    whenReady(() => core().ak_set_global_rate(r));
  },
  /**
   * Treat frame gaps above `threshold` ms (tab switch, debugger) as `adjusted`
   * ms. `lagSmoothing(0)` disables it.
   */
  lagSmoothing(threshold = 500, adjusted = 33): void {
    whenReady(() => core().ak_set_lag_smoothing(threshold, adjusted));
  },
  /** Cap the frame rate (0 = display rate). */
  set fps(v: number) {
    maxFps = v > 0 ? v : 0;
  },
  /** Run `cb(time, dt)` every frame after animations are applied. Returns an unsubscribe fn. */
  onFrame(cb: FrameCallback): () => void {
    frameCallbacks.add(cb);
    wake();
    return () => frameCallbacks.delete(cb);
  },
  /** Live engine statistics. */
  stats(): { active: number; nodes: number; time: number } {
    if (!W) return { active: 0, nodes: 0, time: clock };
    return { active: W.ak_active_count(), nodes: W.ak_node_count(), time: clock };
  },
  /** Elapsed engine time in ms. */
  get time(): number {
    return clock;
  },
};

// ---------------------------------------------------------------------------
// Easings
// ---------------------------------------------------------------------------

export type EasingFn = (t: number) => number;
export interface SpringOptions {
  stiffness?: number;
  damping?: number;
  mass?: number;
  velocity?: number;
  /** Perceptual: approximate visual duration in seconds. */
  duration?: number;
  /** Perceptual: 0 (no bounce) … 1 (very bouncy). */
  bounce?: number;
}
export type EasingInput = string | EasingFn | [number, number, number, number] | SpringOptions | undefined;

const easingCache = new Map<string, number>();
const fnCache = new WeakMap<EasingFn, number>();

/** Resolve any easing input to a core easing id. */
export function easingId(e: EasingInput, fallback = 'outQuad'): number {
  if (e === undefined || e === null) e = fallback;
  if (typeof e === 'string') {
    let id = easingCache.get(e);
    if (id === undefined) {
      const w = core();
      id = w.ak_easing_parse(writeStr(e)) >>> 0;
      if (id === EASING_ERROR) throw new Error(`anikit: unknown easing "${e}"`);
      easingCache.set(e, id);
    }
    return id;
  }
  if (typeof e === 'function') {
    let id = fnCache.get(e);
    if (id === undefined) {
      const n = 1024;
      const at = f64Buf(n);
      for (let i = 0; i < n; i++) F64[at + i] = e(i / (n - 1));
      id = core().ak_easing_table(n) >>> 0;
      fnCache.set(e, id);
    }
    return id;
  }
  if (Array.isArray(e)) return core().ak_easing_bezier(e[0], e[1], e[2], e[3]) >>> 0;
  const s = e as SpringOptions;
  if (s.bounce !== undefined || (s.duration !== undefined && s.stiffness === undefined)) {
    return core().ak_easing_spring_db(s.duration ?? 0.5, s.bounce ?? 0.25, s.velocity ?? 0) >>> 0;
  }
  return core().ak_easing_spring(s.stiffness ?? 100, s.damping ?? 10, s.mass ?? 1, s.velocity ?? 0) >>> 0;
}

/** Natural duration (s) of a spring easing, or -1. */
export function easingDuration(id: number): number {
  return id === LINEAR ? -1 : core().ak_easing_duration(id);
}

export { LINEAR };
