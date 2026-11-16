/**
 * Reactive values: a number you can subscribe to, animate, spring, derive
 * other values from and bind to element styles. Drive many properties from one
 * source of truth (a scroll position, a drag, a slider) without wiring every
 * update by hand.
 *
 * ```ts
 * const progress = motionValue(0);
 * const opacity = progress.map([0, 0.5, 1], [0, 1, 0.4]);
 * const x = progress.map([0, 1], [0, 300]);
 * bindValues('.card', { opacity, x });              // writes styles when they change
 * progress.to(1, { duration: 1.2, ease: 'inOutCubic' });
 * addEventListener('pointermove', (e) => progress.spring(e.clientX / innerWidth));
 * ```
 */
import { animate, AnimateOptions, Animation } from './animate';
import { spring, PhysicsAnimation, PhysicsOptions } from './spring';
import { interpolate } from './utils';
import type { EasingInput } from './core';
import { quickSetter } from './quick';
import { snapshot, classify } from './props';
import { toTargets, Targets } from './props';
import { track } from './context';
import { checkTargets, warn } from './debug';
import { scroll, ScrollOptions } from './scroll';

type Listener = (v: number) => void;

/** A reactive number: `get`/`set`, subscribe with `on`, animate with `to`/`spring`, derive with `map`/`derived()`. */
export class MotionValue {
  private v: number;
  private subs = new Set<Listener>();
  /** The animated box `animate()`/`spring()` write into. */
  private box: { v: number };
  private anim: Animation | PhysicsAnimation | null = null;
  private sopts: PhysicsOptions = {};
  private last = { t: 0, v: 0, vel: 0 };
  private notifying = false;

  constructor(initial: number) {
    this.v = initial;
    this.box = { v: initial };
    this.last = { t: now(), v: initial, vel: 0 };
  }
  /** The current value. */
  get(): number {
    return this.v;
  }
  /** Set the value now (stops a running `to()`/`spring()`) and notify subscribers. */
  set(v: number): void {
    this.stop();
    this.write(v);
  }
  /** Units per second: from the running spring, else from the last changes. */
  get velocity(): number {
    if (this.anim instanceof PhysicsAnimation) {
      const v = this.anim.velocity.v;
      if (v !== undefined) return v;
    }
    return now() - this.last.t > 100 ? 0 : this.last.vel;
  }
  /** Call `fn(value)` on every change; returns an unsubscribe function. */
  on(fn: Listener): () => void {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  }
  /** Tween to `target` (any `animate()` options); returns the animation. */
  to(target: number, options: AnimateOptions = {}): Animation {
    this.stop();
    this.box.v = this.v;
    const user = options.onUpdate;
    const a = animate(this.box, { v: target }, { ...options, onUpdate: (...args: unknown[]) => { this.write(this.box.v); (user as ((...a: unknown[]) => void) | undefined)?.(...args); } });
    this.anim = a;
    return a;
  }
  /**
   * Spring to `target`, keeping the current velocity when re-targeted (call it
   * on every pointer move). Options as `spring()`.
   */
  spring(target: number, options: PhysicsOptions = {}): PhysicsAnimation {
    if (!(this.anim instanceof PhysicsAnimation)) {
      this.stop();
      this.box.v = this.v;
    }
    // One options object per value, so re-targeting doesn't pile up listeners.
    for (const k of Object.keys(this.sopts) as (keyof PhysicsOptions)[]) if (k !== 'onUpdate') delete this.sopts[k];
    Object.assign(this.sopts, options, { onUpdate: () => { this.write(this.box.v); options.onUpdate?.(); } });
    const s = spring(this.box, { v: target }, this.sopts);
    this.anim = s;
    return s;
  }
  /** Stop a running `to()`/`spring()` where it is. */
  stop(): void {
    if (!this.anim) return;
    const a = this.anim;
    this.anim = null;
    a.kill();
  }
  /** A value that follows this one through `interpolate()` stops: `x.map([0, 1], [0, 300])`. */
  map(input: number[], output: number[], options: {
    /** Hold the end values outside the input range (default true). */
    clamp?: boolean;
    /** Easing for every segment, or one per segment. */
    ease?: EasingInput | EasingInput[];
  } = {}): MotionValue {
    const f = interpolate(input, output, options) as (v: number) => number;
    return derived([this], (v) => f(v));
  }
  /** Stop animating, drop every subscriber and any source (e.g. `scrollProgress()`'s listener). */
  destroy(): void {
    this.stop();
    this.subs.clear();
    this._cleanup?.();
    this._cleanup = undefined;
  }
  /** @internal Disconnect from a source on destroy(). */
  _cleanup?: () => void;

  private write(v: number): void {
    if (v === this.v) return;
    const t = now(), last = this.last, dt = (t - last.t) / 1000;
    if (dt > 0) {
      // In place: one record per value, not one object per change.
      last.vel = (v - last.v) / dt;
      last.t = t;
      last.v = v;
    }
    this.v = v;
    if (this.notifying) return; // a subscriber set us again: the outer loop already has the latest value
    this.notifying = true;
    try {
      for (const fn of this.subs) fn(this.v);
    } finally {
      this.notifying = false;
    }
  }
  /** @internal Set without stopping animations (derived values). */
  _derive(v: number): void {
    this.write(v);
  }
}

const now = () => (typeof performance !== 'undefined' ? performance.now() : Date.now());

/** A reactive number starting at `initial`. */
export function motionValue(initial = 0): MotionValue {
  return new MotionValue(initial);
}

/**
 * A value computed from others, recomputed whenever any of them changes:
 * `derived([w, h], (w, h) => w * h)`.
 */
export function derived(sources: MotionValue[], fn: (...values: number[]) => number): MotionValue {
  // One or two sources (the common cases, and every `map()`): no array and
  // spread per update.
  const a = sources[0], b = sources[1];
  const read =
    sources.length === 1 ? () => fn(a.get())
    : sources.length === 2 ? () => fn(a.get(), b.get())
    : () => fn(...sources.map((s) => s.get()));
  const out = new MotionValue(read());
  for (const s of sources) s.on(() => out._derive(read()));
  return out;
}

/** What `bindValues()` returns: stop the binding, or stop and restore the styles from before. */
export interface ValueBinding {
  /** Stop updating and restore each element's inline style from before the binding. */
  revert(): void;
  /** Stop updating, keep the current styles. */
  kill(): void;
}

/**
 * Write values to element properties whenever they change (any property
 * `animate()` understands: `x`, `opacity`, `backgroundColor`, CSS variables…).
 * The current values are written straight away. Registered with `context()`.
 */
export function bindValues(targets: Targets, props: Record<string, MotionValue>): ValueBinding {
  const els = toTargets(targets);
  checkTargets('bindValues()', targets, els.length);
  const saved = els.map((el) => (el as HTMLElement).getAttribute?.('style') ?? null);
  // Also the engine's cached transform, or a later animation would write the
  // bound values back.
  const restores: Array<() => void> = [];
  for (const el of els) for (const prop of Object.keys(props)) restores.push(snapshot(el, prop, classify(el, prop)));
  const offs: Array<() => void> = [];
  for (const [prop, val] of Object.entries(props)) {
    if (!(val instanceof MotionValue)) {
      warn(`bindValues(): "${prop}" is not a motionValue (got ${typeof val}); wrap it: motionValue(${JSON.stringify(val)})`);
      continue;
    }
    const set = quickSetter(els, prop);
    set(val.get());
    offs.push(val.on(set));
  }
  const kill = () => { for (const off of offs.splice(0)) off(); };
  return track({
    kill,
    revert() {
      kill();
      for (const r of restores.splice(0).reverse()) r();
      els.forEach((el, i) => {
        const e = el as HTMLElement;
        if (!e.setAttribute) return;
        if (saved[i] === null) e.removeAttribute('style');
        else e.setAttribute('style', saved[i]!);
      });
    },
  });
}

/**
 * Scroll progress as a motion value (0 → 1 through `options`' range, same
 * options as `scroll()`: `target`, `offset`, `container`, `axis`, `smooth`).
 * Its `velocity` is the scroll speed in progress per second.
 *
 * ```ts
 * const p = scrollProgress({ target: hero, offset: ['start start', 'end start'] });
 * bindValues(hero, { opacity: p.map([0, 1], [1, 0]), scale: p.map([0, 1], [1, 0.9]) });
 * ```
 */
export function scrollProgress(options: ScrollOptions = {}): MotionValue {
  const v = new MotionValue(0);
  v._cleanup = scroll((p) => v._derive(p), options);
  return v;
}
