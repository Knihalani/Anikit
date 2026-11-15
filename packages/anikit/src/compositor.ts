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
import { easingId, easingDuration, EasingInput, LINEAR } from './core';
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
  constructor() {
    this._done = new Promise((r) => (this._resolve = r));
  }
  /** @internal */
  _settle(list: globalThis.Animation[]): void {
    this.animations = list;
    Promise.all(list.map((a) => a.finished.catch(() => undefined))).then(() => this._resolve());
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
  kill(): void {
    this.animations.forEach((a) => a.cancel());
    this._resolve();
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
    els.forEach((el, i) => {
      const res = <T,>(v: FnValue<T>): T => (typeof v === 'function' ? (v as any)(el, i, n) : v);
      const s = stateOf(el);
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
        duration: dur * 1000,
        delay: res(options.delay ?? 0) * 1000,
        endDelay: (options.endDelay ?? 0) * 1000,
        iterations: repeat + 1,
        direction: options.direction ? DIR[options.direction] : options.yoyo ? 'alternate' : 'normal',
        easing,
        fill: 'both',
      });
      if (options.speed) anim.playbackRate = options.speed;
      const last = temp[count - 1];
      anim.finished.then(() => {
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
