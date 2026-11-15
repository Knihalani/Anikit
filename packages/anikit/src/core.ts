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

import { didYouMean, EASING_NAMES } from './debug';

export interface NodeRecord {
  /** Description for `engine.inspect()`. */
  info?(): Record<string, unknown>;
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
let rendering = false;

export function flush(): void {
  const w = W;
  if (!w) return;
  // Re-entrancy: callbacks fired while rendering (e.g. a spring's onUpdate
  // seeking another animation) append to the same core list; the loop below
  // picks those up, so a nested call has nothing to do.
  if (rendering) return;
  rendering = true;
  try {
    let i = 0;
    for (;;) {
      const n = w.ak_updated_len();
      if (i >= n) break;
      views();
      const up = w.ak_updated_ptr() >>> 2;
      for (; i < n; i++) {
        const h = U32[up + i];
        const r = records[h & INDEX_MASK];
        if (r !== undefined && r.h === h && r.render !== undefined) r.render();
      }
    }
  } finally {
    rendering = false;
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
/** Recorders (e.g. `anikit-record`) set `__ANIKIT_MANUAL__` before load to start in manual time. */
let manual = !!(globalThis as any).__ANIKIT_MANUAL__;
export const isManual = (): boolean => manual;
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

let rate = 1;

function step(dt: number): void {
  const w = core();
  w.ak_tick(dt);
  // Frame callbacks (worlds, clouds, user loops) see engine time, so
  // `engine.speed` slows everything down together.
  const sdt = dt * rate;
  clock += sdt;
  flush();
  if (frameCallbacks.size) for (const cb of frameCallbacks) cb(clock, sdt);
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

let reduced: 'never' | 'always' | 'user' = 'never';
let reducedMode: 'instant' | 'fade' = 'instant';
/** How reduced motion is applied (see `engine.reducedMotionMode`). */
export function reducedMotionMode(): 'instant' | 'fade' {
  return reducedMode;
}
/** True when animations should be reduced (see `engine.reducedMotion`). */
export function prefersReduced(): boolean {
  if (reduced === 'always') return true;
  if (reduced === 'never') return false;
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

export const engine = {
  /**
   * Accessibility: `'user'` follows the OS "reduce motion" setting, `'always'`
   * forces it. When reduced, animations jump to their end state (no motion,
   * no infinite loops) while callbacks and promises still fire.
   */
  get reducedMotion(): 'never' | 'always' | 'user' {
    return reduced;
  },
  set reducedMotion(v: 'never' | 'always' | 'user') {
    reduced = v;
  },
  /**
   * What "reduced" means. `'instant'` (default): every animation jumps to its
   * end. `'fade'`: only movement is removed — transforms, motion paths, shape
   * morphs and layout properties (sizes, offsets, margins) jump to their end
   * values, while opacity, colours, filters and the like still animate with
   * their normal timing. Fades and colour changes don't trigger vestibular
   * symptoms and keep interfaces readable, so this is often the better choice.
   *
   * ```ts
   * engine.reducedMotion = 'user';
   * engine.reducedMotionMode = 'fade';
   * ```
   */
  get reducedMotionMode(): 'instant' | 'fade' {
    return reducedMode;
  },
  set reducedMotionMode(v: 'instant' | 'fade') {
    reducedMode = v;
  },
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
  /**
   * Global time scale (1 = normal, 0.1 = bullet time). Applies to every
   * animation, physics world, point cloud and `onFrame` callback's `dt`.
   */
  set speed(r: number) {
    rate = Math.max(0, r);
    whenReady(() => core().ak_set_global_rate(rate));
  },
  get speed(): number {
    return rate;
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
  /**
   * Run `cb(time, dt)` every frame after animations are applied (`dt` in ms,
   * scaled by `engine.speed`). Returns an unsubscribe fn.
   */
  onFrame(cb: FrameCallback): () => void {
    frameCallbacks.add(cb);
    wake();
    return () => frameCallbacks.delete(cb);
  },
  /**
   * JSON-serialisable snapshot of everything the engine is running — what is
   * animating which target, its state and progress. Handy for debugging (and
   * for coding agents driving a headless browser).
   */
  inspect(): Array<Record<string, unknown>> {
    const out: Array<Record<string, unknown>> = [];
    if (!W) return out;
    const states = ['idle', 'running', 'paused', 'finished'];
    for (const r of records) {
      if (!r || !r.info) continue;
      const h = r.h;
      if (!W.ak_alive(h)) continue;
      const dur = W.ak_duration(h);
      out.push({
        id: h & INDEX_MASK,
        state: states[W.ak_state(h)] ?? 'unknown',
        time: Math.round(W.ak_time(h)) / 1000,
        duration: Number.isFinite(dur) ? Math.round(dur) / 1000 : 'infinite',
        progress: Math.round((W.ak_progress(h) || 0) * 1000) / 1000,
        ...r.info(),
      });
    }
    return out;
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
/**
 * A spring used as an easing. Give either physical parameters
 * (`stiffness`/`damping`/`mass`) or perceptual ones (`bounce`/`duration`).
 */
export interface SpringOptions {
  /** Spring constant. Default 100; higher is snappier. */
  stiffness?: number;
  /** Friction. Default 10; higher settles with less oscillation. */
  damping?: number;
  /** Mass. Default 1; heavier is slower and swings further. */
  mass?: number;
  /** Initial velocity, in units of the whole distance per second. Default 0. */
  velocity?: number;
  /** Perceptual: approximate visual duration in seconds. */
  duration?: number;
  /** Perceptual: 0 (no bounce) … 1 (very bouncy). */
  bounce?: number;
}
type EaseCurve = 'Quad' | 'Cubic' | 'Quart' | 'Quint' | 'Sine' | 'Expo' | 'Circ' | 'Back' | 'Elastic' | 'Bounce';
type GsapCurve = 'power1' | 'power2' | 'power3' | 'power4' | 'sine' | 'expo' | 'circ' | 'back' | 'elastic' | 'bounce';
type SpringPreset = 'gentle' | 'wobbly' | 'stiff' | 'slow' | 'molasses' | 'snappy' | 'bouncy';
/**
 * Easing names with editor autocomplete. Any string the parser accepts also
 * works (`EasingInput` allows every string): function forms such as
 * `'cubic-bezier(.2,.8,.2,1)'`, `'steps(4, jump-start)'`, `'back.out(1.7)'`,
 * `'spring(bounce: 0.3, duration: 0.6)'`, `'custom(M0,0 C…)'`, `'mirror(outBack)'`.
 * Unknown names throw with a "did you mean" suggestion.
 */
export type EasingName =
  | 'linear' | 'ease' | 'ease-in' | 'ease-out' | 'ease-in-out' | 'step-start' | 'step-end'
  | `${'in' | 'out' | 'inOut' | 'outIn'}${EaseCurve}`
  | `ease${'In' | 'Out' | 'InOut'}${EaseCurve}`
  | GsapCurve | `${GsapCurve}.${'in' | 'out' | 'inOut'}`
  | 'spring' | SpringPreset | `spring.${SpringPreset}`
  | 'anticipate' | 'wiggle' | 'shake' | 'slowmo' | 'rough';
/**
 * Anything accepted as an easing: a name or function string (see {@link EasingName}),
 * a `(t) => number` function, a cubic-bezier `[x1, y1, x2, y2]`, or spring
 * options `{ stiffness, damping, mass }` / `{ bounce, duration }`.
 */
export type EasingInput = EasingName | (string & {}) | EasingFn | [number, number, number, number] | SpringOptions | undefined;

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
      if (id === EASING_ERROR) {
        const head = e.replace(/\(.*$/, '').trim();
        throw new Error(`anikit: unknown easing "${e}".${didYouMean(head, EASING_NAMES)} See docs/api.md#easings for every accepted syntax.`);
      }
      easingCache.set(e, id);
    }
    return id;
  }
  if (typeof e === 'function') {
    let id = fnCache.get(e);
    if (id === undefined) {
      const n = 1024;
      // Sample first: user code must not run while the scratch buffer is reserved.
      const samples = new Float64Array(n);
      for (let i = 0; i < n; i++) samples[i] = e(i / (n - 1));
      const at = f64Buf(n);
      F64.set(samples, at);
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

/** Forget every record (used by `testing.reset()` after the core is cleared). */
export function clearRecords(): void {
  records.length = 0;
}

// Global handle for tools (anikit-record CLI, devtools, coding agents):
// `__ANIKIT__.tick(ms)`, `__ANIKIT__.inspect()`, `__ANIKIT__.manual()`.
(globalThis as any).__ANIKIT__ ??= engine;
