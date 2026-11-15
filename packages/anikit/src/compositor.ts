/**
 * Compositor offload: hand transform/opacity/filter animations to the
 * browser's compositor thread via the Web Animations API, so they stay smooth
 * even while the main thread is busy. Any Anikit easing — including springs,
 * bounces and custom functions — is converted by the core into a minimal
 * native CSS `linear()` easing.
 *
 * ```ts
 * compositor('.card', { y: [40, 0], opacity: [0, 1] }, { ease: 'spring(bounce: 0.3)', delay: stagger(0.04) });
 * ```
 */
import { core, readStr, whenReady } from './wasm';
import { easingId, easingDuration, EasingInput, LINEAR, prefersReduced } from './core';
import { track } from './context';
import { AnimateOptions, Props, FnValue, Scalar } from './animate';
import { Targets, toTargets, classify, Kind, readValue, stateOf, composeTransform, TargetState, isTransform, transformUnit, kebab } from './props';
import { parseValue } from './values';

const cssCache = new Map<number, string>();
let linearSupported: boolean | null = null;

/** Native CSS easing string for an easing (springs → `linear()`). */
export function cssEasing(e: EasingInput, tolerance = 0.002): string {
  const id = easingId(e);
  if (id === LINEAR) return 'linear';
  let s = cssCache.get(id);
  if (s === undefined) {
    s = readStr(core().ak_easing_css(id, tolerance));
    cssCache.set(id, s);
  }
  if (s.startsWith('linear(')) {
    if (linearSupported === null) {
      linearSupported = typeof CSS !== 'undefined' && typeof CSS.supports === 'function' && CSS.supports('transition-timing-function', 'linear(0, 1)');
    }
    if (!linearSupported) return 'ease-out';
  }
  return s;
}

const DIR = { normal: 'normal', reverse: 'reverse', alternate: 'alternate', 'alternate-reverse': 'alternate-reverse' } as const;
const SLOT: Record<string, number[]> = {
  x: [0], translateX: [0], y: [1], translateY: [1], z: [2], translateZ: [2], rotate: [3], rotateZ: [3], rotation: [3],
  rotateX: [4], rotateY: [5], skewX: [6], skewY: [7], skew: [6, 7], scaleX: [8], scaleY: [9], scaleZ: [10], scale: [8, 9],
  perspective: [11], xPercent: [12], yPercent: [13],
};

/** Controller for compositor animations (mirrors the core Animation API). */
export class CompositorAnimation implements PromiseLike<void> {
  animations: globalThis.Animation[] = [];
  private _done: Promise<void>;
  private _resolve!: () => void;
  /** Driven by scroll (a scroll timeline or `progress`): never auto-committed on finish. */
  _linked = false;
  /** Restore the inline values that were there before (`revert()`). */
  _restore: Array<() => void> = [];
  private _ready: Array<() => void> | null = [];
  constructor() {
    this._done = new Promise((r) => (this._resolve = r));
    track(this);
  }
  /** Run `fn` once the WAAPI animations exist (they're created when the core is ready). */
  _whenBuilt(fn: () => void): void {
    if (this._ready) this._ready.push(fn);
    else fn();
  }
  /** @internal */
  _settle(list: globalThis.Animation[]): void {
    this.animations = list;
    Promise.all(list.map((a) => a.finished.catch(() => undefined))).then(() => this._resolve());
    const r = this._ready;
    this._ready = null;
    if (r) for (const fn of r) fn();
  }
  get finished(): Promise<void> {
    return this._done;
  }
  then<A = void, B = never>(ok?: ((v: void) => A | PromiseLike<A>) | null, err?: ((e: unknown) => B | PromiseLike<B>) | null): Promise<A | B> {
    return this._done.then(ok, err);
  }
  play(): this {
    this.animations.forEach((a) => a.play());
    return this;
  }
  pause(): this {
    this.animations.forEach((a) => a.pause());
    return this;
  }
  reverse(): this {
    this.animations.forEach((a) => a.reverse());
    return this;
  }
  /** Seek to `time` seconds. */
  seek(time: number): this {
    this.animations.forEach((a) => (a.currentTime = time * 1000));
    return this;
  }
  get speed(): number {
    return this.animations[0]?.playbackRate ?? 1;
  }
  set speed(r: number) {
    this.animations.forEach((a) => a.updatePlaybackRate(r));
  }
  get progress(): number {
    const a = this.animations[0];
    if (!a) return 0;
    const t = a.effect?.getComputedTiming();
    return Number(t?.progress ?? 0);
  }
  /** Seek every animation to overall progress `p` (0–1) and hold there (pauses). */
  set progress(p: number) {
    this._whenBuilt(() => {
      for (const a of this.animations) {
        const end = Number(a.effect?.getComputedTiming().endTime ?? 0);
        if (a.playState !== 'paused') a.pause();
        a.currentTime = Math.min(1, Math.max(0, p)) * end;
      }
    });
  }
  /** Total duration in seconds (longest animation, delays included). */
  get duration(): number {
    let d = 0;
    for (const a of this.animations) d = Math.max(d, Number(a.effect?.getComputedTiming().endTime ?? 0));
    return Number.isFinite(d) ? d / 1000 : Infinity;
  }
  kill(): void {
    this._whenBuilt(() => {
      this.animations.forEach((a) => a.cancel());
      this._resolve();
    });
  }
  /** Cancel and put back the inline styles from before the animation. */
  revert(): void {
    this._whenBuilt(() => {
      this.animations.forEach((a) => a.cancel());
      for (const r of this._restore.splice(0).reverse()) r();
      this._resolve();
    });
  }
}

/** Animate on the compositor thread (transform, opacity, filter, colours, …). */
export function compositor(targets: Targets, props: Props, options: AnimateOptions = {}): CompositorAnimation {
  const ctl = new CompositorAnimation();
  whenReady(() => {
    const list: globalThis.Animation[] = [];
    const els = toTargets(targets).filter((t): t is Element => typeof Element !== 'undefined' && t instanceof Element);
    const n = els.length;
    const ease = (Array.isArray(options.ease) && typeof options.ease[0] !== 'number' ? options.ease[0] : options.ease) as EasingInput;
    const easing = cssEasing(ease ?? 'outQuad');
    const reduce = prefersReduced();
    els.forEach((el, i) => {
      const res = <T,>(v: FnValue<T>): T => (typeof v === 'function' ? (v as any)(el, i, n) : v);
      const s = stateOf(el);
      // Inline values to restore on revert().
      {
        const st = (el as HTMLElement).style;
        const keys = new Set<string>();
        for (const k of Object.keys(props)) keys.add(isTransform(k) ? 'transform' : k.startsWith('--') ? k : kebab(k));
        const t0 = s.t.slice();
        const u0 = s.u.slice();
        for (const key of keys) {
          const v = st.getPropertyValue(key);
          const pr = st.getPropertyPriority(key);
          ctl._restore.push(() => {
            if (v) st.setProperty(key, v, pr);
            else st.removeProperty(key);
            if (key === 'transform') {
              s.t = t0.slice();
              s.u = u0.slice();
            }
          });
        }
      }
      const frames: Array<Record<string, string | number>> = [];
      // Per-property keyframe values.
      let count = 2;
      const values: Record<string, Array<Scalar | null>> = {};
      for (const k of Object.keys(props)) {
        const v = props[k];
        const arr = Array.isArray(v) ? v.map((x) => res(x as FnValue<Scalar | null>)) : [null, res(v as FnValue<Scalar>)];
        values[k] = arr;
        count = Math.max(count, arr.length);
      }
      const temp: TargetState[] = [];
      for (let f = 0; f < count; f++) {
        const st = new TargetState(el);
        st.t = s.t.slice();
        st.u = s.u.slice();
        temp.push(st);
        frames.push({});
      }
      for (const k of Object.keys(values)) {
        const arr = values[k];
        const pick = (f: number) => arr[Math.round((f * (arr.length - 1)) / (count - 1))];
        if (isTransform(k)) {
          for (let f = 0; f < count; f++) {
            const raw = pick(f);
            if (raw === null || raw === undefined) continue;
            const p = parseValue(raw);
            if (p.k !== 'num') continue;
            for (const slot of SLOT[k]) {
              temp[f].t[slot] = p.n;
              if (slot <= 2 && p.unit) temp[f].u[slot] = p.unit;
            }
            void transformUnit;
          }
        } else {
          const kind = classify(el, k);
          for (let f = 0; f < count; f++) {
            const raw = pick(f);
            let v: Scalar = raw === null || raw === undefined ? (readValue(el, k, kind) as Scalar) : raw;
            if (typeof v === 'number' && kind === Kind.Style && !/opacity|zIndex|scale/.test(k)) v = v + 'px';
            frames[f][k.startsWith('--') ? k : kebab(k).replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = v;
          }
        }
      }
      if (Object.keys(values).some(isTransform)) {
        for (let f = 0; f < count; f++) frames[f].transform = composeTransform(temp[f]);
      }
      const repeat = options.repeat === undefined ? 0 : options.repeat < 0 ? Infinity : options.repeat;
      let dur = options.duration !== undefined ? res(options.duration) : 0.5;
      if (options.duration === undefined && typeof ease === 'string' && /^\s*spring/.test(ease)) {
        const d = easingDuration(easingId(ease));
        if (d > 0) dur = d;
      }
      const anim = el.animate(frames as Keyframe[], {
        // Reduced motion: jump to the end state (committed like a normal finish).
        duration: reduce ? 0 : dur * 1000,
        delay: reduce ? 0 : res(options.delay ?? 0) * 1000,
        endDelay: reduce ? 0 : (options.endDelay ?? 0) * 1000,
        iterations: reduce ? 1 : repeat + 1,
        direction: options.direction ? DIR[options.direction] : options.yoyo ? 'alternate' : 'normal',
        easing,
        fill: 'both',
      });
      if (options.speed) anim.playbackRate = options.speed;
      const last = temp[count - 1];
      anim.finished.then(() => {
        // Scroll-linked animations finish whenever the scroll reaches the end:
        // they must stay attached.
        if (ctl._linked) return;
        // Persist the final values as inline styles and free the effect.
        try {
          anim.commitStyles();
        } catch {
          /* element detached */
        }
        anim.cancel();
        s.t = last.t.slice();
        s.u = last.u.slice();
      }).catch(() => undefined);
      list.push(anim);
    });
    if (options.autoplay === false) list.forEach((a) => a.pause());
    ctl._settle(list);
  });
  return ctl;
}
