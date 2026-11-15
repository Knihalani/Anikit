/**
 * `animate()` — the main entry point.
 *
 * ```ts
 * animate('.box', { x: 200, rotate: 90, backgroundColor: '#f0f' }, { duration: 0.8, ease: 'outBack' })
 * animate(el, { opacity: [0, 1], y: [40, 0] }, { delay: stagger(0.05) })
 * animate(path, { d: star(50, 50, 5, 40) })                    // shape morph
 * animate(dot, { path: '#track', rotate: true }, { duration: 3 }) // motion path
 * animate(obj, { value: 100 }, { onUpdate: () => draw(obj.value) })
 * ```
 */
import { core, F64, LINEAR, f64Buf, whenReady, views } from './wasm';
import {
  register, unregister, wake, flush, easingId, easingDuration, NodeRecord, EasingInput, prefersReduced,
  FL_FILL_BACKWARDS, FL_FILL_FORWARDS, FL_IMMEDIATE,
} from './core';
import {
  buildTrack, TrackDef, TRACK_NUMBER, TRACK_PATH, TRACK_MATRIX, COLOR_SPACES, ColorSpaceName, r3,
} from './values';
import {
  Target, Targets, toTargets, classify, Kind, readValue, writer, convertUnit, defaultUnit, isElement,
  claim, unclaim, Owner, stateOf, markDirty, transformUnit, TargetState, snapshot, composeTransform, kebab, computed, camel,
} from './props';
import { Playable, Callbacks, Linked } from './playable';
import { warn, didYouMean, describe, cssPropertyNames, suggest, warningsOn, checkSeconds, checkTargets } from './debug';
import type { OptionKeys } from './debug';
import { Binding, bind, apply, custom, transformSlot, M_PROP, M_TRANSFORM, M_TRANSFORM2, M_STYLE_NUM, M_STYLE_FMT, M_VAR, M_CUSTOM } from './bind';
import { pathData, PathSource, writeMorph, elementLength, morphSequence } from './svg';
import { parseTransform, toMatrix3d } from './scene';
import { writeStr } from './wasm';

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/** A value, or a function producing one per target: `(target, index, count)`. */
export type FnValue<T> = T | ((target: any, index: number, count: number) => T);
export type Scalar = number | string;
export type PropValue =
  | FnValue<Scalar | null | Array<Scalar | null>>
  | Array<FnValue<Scalar | null>>
  | { to?: FnValue<Scalar>; from?: FnValue<Scalar>; value?: Array<FnValue<Scalar | null>>; ease?: EasingInput | EasingInput[] }
  | MotionPathOptions
  | TextOptions
  | Element;

export interface MotionPathOptions {
  /** Path data, a CSS selector or an SVG shape element. */
  path: PathSource;
  /** Progress along the path to travel from / to (0–1). Default 0 → 1. */
  start?: number;
  end?: number;
  /** Rotate the target to follow the path tangent; a number adds an angle offset. */
  rotate?: boolean | number;
}

/**
 * The properties anikit treats specially (documented here so editor hover
 * explains them). Any other key is a CSS property in camelCase
 * (`backgroundColor`), a CSS variable (`'--hue'`), or a property of a plain
 * object target.
 *
 * Every value may be: a single end value (`100`), keyframes (`[0, 100]`,
 * `[null, 50, 0]` where `null` is the current value), a relative value
 * (`'+=50'`, `'-=50'`, `'*=2'`), `'random(0, 100)'`, or a function
 * `(target, index, count) => value` evaluated per target (it may return keyframes too: `(el, i) => [0, i * 10]`).
 */
export interface TransformProps {
  /** translateX. Number = px; any length or `%` of the element (`'50%'`, `'2rem'`). Prefer over `left`. */
  x?: PropValue;
  /** translateY. Number = px; any length or `%`. Prefer over `top`. */
  y?: PropValue;
  /** translateZ in px (needs `perspective` on the element or a parent to be visible). */
  z?: PropValue;
  /** 2D rotation. Number = degrees; also `'1turn'`, `'0.5rad'`. Relative: `'+=90'`. */
  rotate?: PropValue;
  /** 3D rotation around X. Number = degrees. */
  rotateX?: PropValue;
  /** 3D rotation around Y. Number = degrees. */
  rotateY?: PropValue;
  /** Same as `rotate`. Number = degrees. */
  rotateZ?: PropValue;
  /** Uniform scale factor (1 = natural size; unitless). */
  scale?: PropValue;
  /** Horizontal scale factor (unitless). */
  scaleX?: PropValue;
  /** Vertical scale factor (unitless). */
  scaleY?: PropValue;
  /** Skew on both axes. Number = degrees. */
  skew?: PropValue;
  /** Horizontal skew. Number = degrees. */
  skewX?: PropValue;
  /** Vertical skew. Number = degrees. */
  skewY?: PropValue;
  /** translateX as a percentage of the element's own width (`xPercent: -50` centres it). */
  xPercent?: PropValue;
  /** translateY as a percentage of the element's own height. */
  yPercent?: PropValue;
  /** Perspective (px) applied in this element's own transform, for its 3D rotations. */
  perspective?: PropValue;
  /** Opacity, 0–1 (not 0–100). */
  opacity?: PropValue;
  /** A full CSS transform list, interpolated like browsers do: `'rotateY(180deg) translateZ(40px)'`. */
  transform?: PropValue;
  /** SVG shape morph: path data, a `<path>`/shape element or a selector. Arrays morph through several shapes. */
  d?: PropValue;
  /** Motion along a path: path data, a selector, an element, or `{ path, start, end, rotate }`. */
  path?: PropValue;
  /** Visible stroke segment as fractions of the length: `[0, 1]` draws a line in; `['0 0', '0 1']` explicit. */
  draw?: PropValue;
  /** Attributes: `attr: { r: 20, cx: 50, viewBox: '0 0 10 10' }`. */
  attr?: Record<string, PropValue>;
  /** Typewriter / scramble text: `'Hello'` or `{ value: 'Hello', type: 'scramble', chars: 'upper' }`. */
  text?: PropValue;
  /** Number counter written as text; combine with the `format` / `round` options. */
  textContent?: PropValue;
}

/** Properties to animate: see {@link TransformProps} for the special keys. Any CSS property, CSS variable or object key also works. */
export type Props = TransformProps & Record<string, PropValue | Record<string, PropValue>>;
export type Keyframe = Record<string, Scalar | null | undefined> & { offset?: number; ease?: EasingInput };

export interface AnimateOptions extends Callbacks<Animation> {
  /**
   * Length of one iteration, in **seconds** (not ms). Default `0.5`; with a
   * spring easing, the spring's natural settle time. Can be a function
   * `(target, index, count) => seconds`.
   * @example animate('.box', { x: 100 }, { duration: 0.8 })
   */
  duration?: FnValue<number>;
  /**
   * Wait before starting, in **seconds**. Use `stagger(0.05)` to offset each target.
   * @example animate('.item', { opacity: [0, 1] }, { delay: stagger(0.05, { from: 'center' }) })
   */
  delay?: FnValue<number>;
  /** Seconds to hold after the last iteration before `finished` resolves. Default 0. */
  endDelay?: number;
  /**
   * Easing. Default `'outQuad'`. Names (`'outCubic'`, `'power2.inOut'`, `'ease-in-out'`),
   * functions (`'cubic-bezier(.2,.8,.2,1)'`, `'steps(4)'`, `'back.out(1.7)'`),
   * springs (`'spring(bounce: 0.3)'`, `{ stiffness: 200, damping: 20 }`), `[x1, y1, x2, y2]`
   * or `(t) => number`. An array gives one easing per keyframe segment.
   */
  ease?: EasingInput | EasingInput[];
  /** Extra iterations after the first (`1` plays twice). `Infinity` or `-1` loops forever. Default 0. */
  repeat?: number;
  /** Pause between iterations, in seconds. Default 0. */
  repeatDelay?: number;
  /** Alternate direction every iteration (same as `direction: 'alternate'`). Default false. */
  yoyo?: boolean;
  /** Iteration direction, as in CSS. Default `'normal'`. */
  direction?: 'normal' | 'reverse' | 'alternate' | 'alternate-reverse';
  /** Playback rate: `2` is twice as fast, negative plays backwards. Default 1. */
  speed?: number;
  /** Start playing immediately. Default true; pass `false` for scroll triggers and manual control. */
  autoplay?: boolean;
  /** Keyframe offsets (0–1) for array values, e.g. `{ x: [0, 100, 50] }` with `times: [0, 0.3, 1]`. */
  times?: number[];
  /** Colour interpolation space. Default `'oklab'` (perceptually even, no grey midpoints). */
  colorSpace?: ColorSpaceName;
  /** Which values apply outside the active period. Default `'both'`. (Named `fillMode` so `fill` stays the SVG property.) */
  fillMode?: 'both' | 'forwards' | 'backwards' | 'none';
  /** Take the animated properties over from animations already running on the same targets. Default true. */
  overwrite?: boolean;
  /** Render the first keyframe right away, before the delay ends. Default: on for delayed animations with explicit start values. */
  immediateRender?: boolean;
  /** Snap numeric values to a step (or the nearest of a list); a number applies to every property: `{ snap: { x: 50 } }`. */
  snap?: number | Record<string, number | number[]>;
  /** Round to integers: `true` for every property, or a list of property names. */
  round?: boolean | string[];
  /** Transform each value right before it is written, e.g. `{ x: (v) => v % 500 }` for wrapping. */
  modifiers?: Record<string, (value: any, target: any) => any>;
  /**
   * Format numbers written as text (counters): `Intl.NumberFormat` options or a function.
   * @example animate('#price', { textContent: 12499 }, { format: { style: 'currency', currency: 'USD' } })
   */
  format?: Intl.NumberFormatOptions | ((value: number) => string);
  /** Locale used by `format`. Default: the user's. */
  locale?: string;
}

/** Every `AnimateOptions` key (compiler-checked; used for "unknown option" warnings). */
export const ANIMATE_KEYS: OptionKeys<AnimateOptions> = {
  duration: 0, delay: 0, endDelay: 0, ease: 0, repeat: 0, repeatDelay: 0, yoyo: 0, direction: 0, speed: 0, autoplay: 0,
  times: 0, colorSpace: 0, fillMode: 0, overwrite: 0, immediateRender: 0, onStart: 0, onUpdate: 0, onComplete: 0,
  onRepeat: 0, onReverseComplete: 0, snap: 0, round: 0, modifiers: 0, format: 0, locale: 0,
};
const OPTION_KEYS = new Set(Object.keys(ANIMATE_KEYS));

const DIRECTIONS = { normal: 0, reverse: 1, alternate: 2, 'alternate-reverse': 3 } as const;
/** Options accepted for compatibility / by wrappers (not warned about). */
const EXTRA_OPTION_KEYS = new Set(['stagger', 'distance', 'from', 'intensity', 'loop', 'axis', 'at', 'charDelay', 'start', 'to', 'chars', 'delimiter', 'text', 'size', 'color']);
const TRANSFORM_PROPS = ['x', 'y', 'z', 'rotate', 'rotateX', 'rotateY', 'rotateZ', 'scale', 'scaleX', 'scaleY', 'scaleZ', 'skew', 'skewX', 'skewY', 'perspective', 'xPercent', 'yPercent'];
const SPECIAL_PROPS = ['path', 'draw', 'd', 'attr', 'text', 'transform'];
const FILL_MODES = ['both', 'forwards', 'backwards', 'none'];
const ANGLE_UNITS = ['', 'deg', 'rad', 'turn', 'grad'];

/** Construction-time validation of animation options (only while warnings are on). */
function checkAnimateOptions(given: AnimateOptions, o: AnimateOptions): void {
  for (const k of Object.keys(given)) {
    if (!OPTION_KEYS.has(k) && !EXTRA_OPTION_KEYS.has(k)) warn(`unknown animation option "${k}"; it is ignored.${didYouMean(k, OPTION_KEYS)}`);
  }
  checkSeconds('animate()', 'duration', o.duration);
  checkSeconds('animate()', 'delay', o.delay);
  checkSeconds('animate()', 'repeatDelay', o.repeatDelay);
  checkSeconds('animate()', 'endDelay', o.endDelay);
  if (o.colorSpace !== undefined && !(o.colorSpace in COLOR_SPACES)) {
    warn(`unknown colorSpace "${o.colorSpace}"; using "oklab". Use one of: ${Object.keys(COLOR_SPACES).join(', ')}.${didYouMean(String(o.colorSpace), Object.keys(COLOR_SPACES))}`);
  }
  if (o.direction !== undefined && !(o.direction in DIRECTIONS)) {
    warn(`unknown direction "${o.direction}"; using "normal". Use one of: ${Object.keys(DIRECTIONS).join(', ')}.${didYouMean(String(o.direction), Object.keys(DIRECTIONS))}`);
  }
  if (o.fillMode !== undefined && !FILL_MODES.includes(o.fillMode)) {
    warn(`unknown fillMode "${o.fillMode}"; using "both". Use one of: ${FILL_MODES.join(', ')}.`);
  }
  if (typeof o.repeat === 'number' && o.repeat > 0 && o.repeat !== Infinity && !Number.isInteger(o.repeat)) {
    warn(`repeat is ${o.repeat}; it counts extra iterations and should be a whole number (or Infinity / -1 to loop).`);
  }
}

/** Unit / range sanity checks for one property's values (first target only). */
function checkValues(prop: string, kind: Kind, raw: unknown[], target: unknown): void {
  if (kind === Kind.Transform) {
    const canonical = transformUnit(prop);
    for (const v of raw) {
      if (typeof v !== 'string') continue;
      const unit = /^\s*[-+*]?=?\s*-?[\d.]+(?:e[-+]?\d+)?\s*([a-z%]*)\s*$/i.exec(v)?.[1]?.toLowerCase();
      if (unit === undefined || unit === '') continue;
      const angleUnit = ANGLE_UNITS.includes(unit);
      if (canonical === 'deg' && !angleUnit) warn(`"${prop}" is an angle, but got "${v}". Use a number (degrees) or deg / rad / turn.`);
      else if (canonical === '' && unit !== '%') warn(`"${prop}" is a unitless factor (1 = 100%), but got "${v}". Use e.g. ${prop}: 1.5.`);
      else if ((canonical === 'px' || canonical === '%') && angleUnit) warn(`"${prop}" is a length, but got "${v}". Use a number (px) or a length unit; for rotation use "rotate".`);
    }
  } else if (prop === 'opacity' && isElement(target)) {
    const big = raw.find((v) => typeof v === 'number' && v > 1);
    if (big !== undefined) warn(`opacity goes from 0 to 1, but got ${big} on ${describe(target)}. Did you mean ${+((big as number) / 100).toFixed(3)}?`);
  }
}

function isKnownStyle(el: HTMLElement, prop: string): boolean {
  const st = el.style as any;
  return !st || prop in st || camel(prop) in st || prop.startsWith('--');
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------


/** Per-target engine node. */
class Part implements NodeRecord, Owner, Linked {
  h = 0;
  at = 0;
  b: Binding[] = [];
  ctl: Playable | null;
  constructor(ctl: Playable, public claims: boolean) {
    this.ctl = ctl;
  }
  /** Whether the controller wants per-frame `onUpdate` notifications. */
  upd = false;
  /** Timing used to create the node: [delayMs, durationMs, iterations, direction]. */
  tm: [number, number, number, number] = [0, 0, 1, 0];
  render(): void {
    const at = this.at;
    const b = this.b;
    const f = F64;
    for (let i = 0; i < b.length; i++) {
      const x = b[i];
      if (!x.on) continue;
      // Inline the two hottest modes; everything else goes through `apply`.
      const m = x.mode;
      if (m === M_TRANSFORM) {
        const s = x.obj as TargetState;
        const v = f[at + x.off];
        if (s.t[x.slot] !== v) {
          s.t[x.slot] = v;
          if (!s.dirty) markDirty(s);
        }
      } else if (m === M_PROP) {
        x.target[x.key] = f[at + x.off];
      } else apply(x, at + x.off);
    }
    if (this.upd) {
      const c = this.ctl;
      if (c !== null) c._touch();
    }
  }
  event?: (kind: number) => void;
  info(): Record<string, unknown> {
    return {
      type: 'tween',
      target: this.b[0] ? describe(this.b[0].target) : undefined,
      props: Array.from(new Set(this.b.filter((x) => x.on).map((x) => x.prop))),
    };
  }
  /** Another animation took `prop`: stop writing it. */
  release(prop: string): void {
    let live = 0;
    for (const x of this.b) {
      if (x.prop === prop) {
        x.on = false;
        if (!this.b.some((y) => y.on && y.track === x.track)) core().ak_track_enabled(this.h, x.track, 0);
      }
      if (x.on) live++;
    }
    if (live === 0) this.onEmpty?.();
  }
  onEmpty?: () => void;
  dispose(): void {
    for (const x of this.b) if (this.claims) unclaim(x.target, x.prop, this);
    unregister(this);
  }
}

/** Per-timeline/animation chain of last written values (timeline sequencing). */
export type Chain = WeakMap<object, Map<string, Scalar>>;

export interface BuildContext {
  /** Built as a timeline child (no autoplay, no overwrite claims). */
  child?: boolean;
  chain?: Chain;
  /** Force zero-duration "set" semantics. */
  set?: boolean;
  /** Don't warn about an empty target list (e.g. flip() when nothing moved). */
  quiet?: boolean;
}

const RANDOM_RE = /^\s*random\((.*)\)\s*$/;

/** Resolve `"random(min, max[, snap])"` / `"random([a, b, c])"` value strings. */
export function resolveRandom(v: unknown): unknown {
  if (typeof v !== 'string') return v;
  const m = RANDOM_RE.exec(v);
  if (!m) return v;
  const body = m[1].trim();
  if (body.startsWith('[')) {
    const items = body.slice(1, body.lastIndexOf(']')).split(',').map((x) => x.trim()).filter(Boolean);
    const pick = items[Math.floor(Math.random() * items.length)];
    const n = Number(pick);
    return Number.isFinite(n) && pick !== '' ? n : pick.replace(/^['"]|['"]$/g, '');
  }
  const parts = body.split(',').map((x) => x.trim());
  const unit = /[a-z%]+$/i.exec(parts[0])?.[0] ?? '';
  const [lo, hi, step] = parts.map(parseFloat);
  let r = lo + Math.random() * ((hi ?? 1) - lo);
  if (step) r = Math.round(r / step) * step;
  return unit ? r + unit : r;
}

function resolve<T>(v: FnValue<T>, t: unknown, i: number, n: number): T {
  return resolveRandom(typeof v === 'function' ? (v as any)(t, i, n) : v) as T;
}

/** Post-processing pipeline for one property (snap/round/modifier/format), or null. */
function postFor(prop: string, o: AnimateOptions): ((v: any, target: any) => any) | null {
  const snapOpt = o.snap === undefined ? undefined : typeof o.snap === 'number' ? o.snap : o.snap[prop];
  const round = o.round === true || (Array.isArray(o.round) && o.round.includes(prop));
  const mod = o.modifiers?.[prop];
  let fmt: ((v: number) => string) | undefined;
  if (o.format) {
    fmt = typeof o.format === 'function' ? o.format : ((nf) => (v: number) => nf.format(v))(new Intl.NumberFormat(o.locale, o.format));
  }
  if (snapOpt === undefined && !round && !mod && !fmt) return null;
  return (v: any, target: any) => {
    if (typeof v === 'number') {
      if (snapOpt !== undefined) {
        v = Array.isArray(snapOpt)
          ? snapOpt.reduce((a, b) => (Math.abs(b - v) < Math.abs(a - v) ? b : a))
          : Math.round(v / snapOpt) * snapOpt;
      }
      if (round) v = Math.round(v);
    }
    if (mod) v = mod(v, target);
    if (fmt && typeof v === 'number') v = fmt(v);
    return v;
  };
}

const SCRAMBLE_SETS: Record<string, string> = {
  upper: 'ABCDEFGHIJKLMNOPQRSTUVWXYZ',
  lower: 'abcdefghijklmnopqrstuvwxyz',
  digits: '0123456789',
  symbols: '!<>-_\\/[]{}—=+*^?#',
  binary: '01',
};

/** Options for the `text` property. */
export interface TextOptions {
  /** Text to end with. */
  value: string;
  /** Text to start from (default: current text). */
  from?: string;
  /** 'type' replaces characters progressively; 'scramble' reveals through random glyphs. */
  type?: 'type' | 'scramble';
  /** Animate per word (`' '`) instead of per character. */
  delimiter?: string;
  /** Scramble character set: 'upper' | 'lower' | 'digits' | 'symbols' | 'binary' or custom characters. */
  chars?: string;
  /** Scramble refresh rate (0–1, default 1 = every frame). */
  speed?: number;
}

function isPlainObject(v: unknown): v is Record<string, any> {
  return typeof v === 'object' && v !== null && !Array.isArray(v) && !isElement(v);
}

interface PropSpec {
  values: Array<FnValue<Scalar | null>>;
  offsets?: number[];
  ease?: EasingInput | EasingInput[];
  raw?: unknown;
}

/** Normalise the keyframes argument into per-property specs + extracted options. */
function normalize(kf: Props | Keyframe[], opts: AnimateOptions): [Map<string, PropSpec>, AnimateOptions] {
  const out = new Map<string, PropSpec>();
  const o: AnimateOptions = { ...opts };
  if (Array.isArray(kf)) {
    // WAAPI-style keyframe list.
    const n = kf.length;
    const offs: (number | undefined)[] = kf.map((k) => (typeof k.offset === 'number' ? k.offset : undefined));
    if (offs[0] === undefined) offs[0] = 0;
    if (n > 1 && offs[n - 1] === undefined) offs[n - 1] = 1;
    for (let i = 1; i < n; i++) {
      if (offs[i] === undefined) {
        let j = i;
        while (offs[j] === undefined) j++;
        const a = offs[i - 1]!;
        const b = offs[j]!;
        for (let k = i; k < j; k++) offs[k] = a + ((b - a) * (k - i + 1)) / (j - i + 1);
        i = j;
      }
    }
    kf.forEach((frame, i) => {
      for (const key of Object.keys(frame)) {
        if (key === 'offset' || key === 'ease') continue;
        if (OPTION_KEYS.has(key)) {
          (o as any)[key] = frame[key];
          continue;
        }
        let s = out.get(key);
        if (!s) out.set(key, (s = { values: [], offsets: [], ease: [] }));
        s.values.push(frame[key] as Scalar);
        s.offsets!.push(offs[i]!);
        (s.ease as EasingInput[]).push(frame.ease ?? (Array.isArray(o.ease) ? undefined : o.ease));
      }
    });
    for (const s of out.values()) {
      // Missing at the edges: start from current value / hold the last value.
      if (s.offsets![0] > 0) {
        s.values.unshift(null);
        s.offsets!.unshift(0);
        (s.ease as EasingInput[]).unshift(o.ease as EasingInput);
      }
      if (s.offsets![s.offsets!.length - 1] < 1) {
        s.values.push(s.values[s.values.length - 1]);
        s.offsets!.push(1);
      }
      (s.ease as EasingInput[]).length = s.values.length - 1;
      if (s.values.length === 1) s.values.unshift(null);
    }
    return [out, o];
  }
  for (const key of Object.keys(kf)) {
    const v = (kf as any)[key];
    if (OPTION_KEYS.has(key)) {
      (o as any)[key] = v;
      continue;
    }
    if (key === 'path' || key === 'draw' || key === 'd' || key === 'attr' || key === 'text' || key === 'transform') {
      out.set(key, { values: [], raw: v });
      continue;
    }
    if (Array.isArray(v)) out.set(key, { values: v, offsets: opts.times });
    else if (isPlainObject(v) && ('to' in v || 'from' in v || 'value' in v)) {
      const values = v.value ?? [v.from ?? null, v.to ?? null];
      out.set(key, { values, ease: v.ease, offsets: opts.times });
    } else out.set(key, { values: [null, v] });
  }
  return [out, o];
}

function isSpringEase(e: unknown): boolean {
  return (typeof e === 'string' && /^\s*spring/i.test(e)) || (isPlainObject(e) && !Array.isArray(e));
}

// ---------------------------------------------------------------------------
// The Animation controller
// ---------------------------------------------------------------------------

export class Animation extends Playable {
  /** The resolved targets. */
  targets: Target[] = [];
  _parts: Part[] = [];
  private _group = 0;
  private _immediate = false;
  /** Restorers for `revert()` (captured before the first write). */
  _restore: Array<() => void> = [];
  private _times: number[] | undefined;
  /** Seconds; available once built. */
  _end = 0;

  constructor(targets: Targets, keyframes: Props | Keyframe[], options: AnimateOptions = {}, ctx: BuildContext = {}) {
    if (typeof options === 'number') {
      // `animate(el, props, 0.5)`: anime.js / jQuery habit. Accept it as the duration.
      warn(`animate(): the third argument is an options object; got the number ${options}. Treating it as { duration: ${options} } (seconds).`);
      options = { duration: options };
    }
    if (keyframes === null || typeof keyframes !== 'object') {
      warn(`animate(): the second argument must be an object of properties ({ x: 100, opacity: 0 }) or a keyframe list; got ${typeof keyframes === 'string' ? `"${keyframes}"` : String(keyframes)}.${typeof keyframes === 'string' ? ' For presets, use effects.' + keyframes + '(targets).' : ''}`);
      keyframes = {};
    }
    const [props, opts] = normalize(keyframes, options ?? {});
    super(opts);
    if (warningsOn()) checkAnimateOptions(options ?? {}, opts);
    this._times = opts.times;
    whenReady(() => this._build(targets, props, opts, ctx));
  }

  private _build(targetsIn: Targets, props: Map<string, PropSpec>, o: AnimateOptions, ctx: BuildContext): void {
    const w = core();
    const targets = (this.targets = toTargets(targetsIn));
    const n = targets.length;
    if (!ctx.quiet) checkTargets('animate()', targetsIn, n);
    const claims = !ctx.child && o.overwrite !== false;
    const cs = COLOR_SPACES[o.colorSpace ?? 'oklab'] ?? 2;
    const reduce = prefersReduced();
    const repeat = reduce || o.repeat === undefined ? 0 : o.repeat < 0 ? Infinity : o.repeat;
    const direction =
      o.direction !== undefined ? DIRECTIONS[o.direction] ?? 0 : o.yoyo ? DIRECTIONS.alternate : 0;
    let fill = FL_FILL_BACKWARDS | FL_FILL_FORWARDS;
    if (o.fillMode === 'forwards') fill = FL_FILL_FORWARDS;
    else if (o.fillMode === 'backwards') fill = FL_FILL_BACKWARDS;
    else if (o.fillMode === 'none') fill = 0;
    const easeList = Array.isArray(o.ease) && !(o.ease.length === 4 && typeof o.ease[0] === 'number') ? o.ease : null;
    const baseEase = easeList ? undefined : (o.ease as EasingInput);

    for (let i = 0; i < n; i++) {
      const target = targets[i];
      const part = new Part(this, claims);
      part.upd = !!o.onUpdate;
      const tracks: TrackDef[] = [];
      let hasExplicitFrom = false;

      const addTrack = (t: TrackDef, ease: EasingInput | EasingInput[] | undefined, offsets?: number[]) => {
        const nk = t.values.length / t.ch;
        if (offsets && offsets.length === nk) t.offsets = offsets;
        const e = ease ?? (easeList ? easeList : baseEase);
        if (Array.isArray(e) && !(e.length === 4 && typeof e[0] === 'number')) {
          const ids: number[] = [];
          for (let k = 0; k < nk - 1; k++) ids.push(easingId((e as EasingInput[])[k] ?? baseEase));
          t.easings = ids;
        } else {
          const id = easingId(e as EasingInput);
          t.easings = id === LINEAR ? [] : [id];
        }
        tracks.push(t);
        return tracks.length - 1;
      };

      for (const [prop, spec] of props) {
        if (spec.raw !== undefined) {
          this._special(prop, spec.raw, target, i, n, part, tracks, addTrack, cs, ctx);
          continue;
        }
        const kind = classify(target, prop);
        if (kind === Kind.Style && i === 0 && !isKnownStyle(target as HTMLElement, prop)) {
          const s = suggest(prop, [...cssPropertyNames(), ...TRANSFORM_PROPS, ...SPECIAL_PROPS, ...OPTION_KEYS]);
          const hint = !s ? '' : OPTION_KEYS.has(s) ? ` Did you mean the option "${s}"? (Options can go in the third argument or among the properties.)` : ` Did you mean "${s}"?`;
          warn(`"${prop}" is not a CSS property of ${describe(target)}; it will be written as an inline style anyway.${hint}`);
        }
        let raw = spec.values.map((v) => resolve(v, target, i, n));
        // A function may return keyframes for its target: `y: (el, i) => [0, i * 10]`.
        if (raw.length === 2 && raw[0] === null && Array.isArray(raw[1])) raw = raw[1] as Array<Scalar | null>;
        if (i === 0 && warningsOn()) checkValues(prop, kind, raw, target);
        if (raw[0] !== null && raw[0] !== undefined && raw.length > 1) hasExplicitFrom = true;
        const chained = ctx.chain?.get(target)?.get(prop);
        const read = () => (chained !== undefined ? chained : readValue(target, prop, kind));
        const el = isElement(target) ? target : null;
        let du = defaultUnit(prop, kind);
        const conv = (v: number, f: string, t: string) => convertUnit(el, prop, v, f, t);
        let track = buildTrack(raw, read, conv, cs, du);
        if (kind === Kind.Transform) {
          if (track.kind !== TRACK_NUMBER || track.ch !== 1) {
            if (i === 0) warn(`"${prop}" needs a single number (optionally with a unit, e.g. 100 or '50%'); got ${raw.map((v) => JSON.stringify(v)).join(' → ')}, so it was skipped. For a full transform string use the "transform" property.`);
            continue;
          }
          // Angles/scales always use their canonical unit; x/y/z keep any length unit.
          du = transformUnit(prop);
          if (du !== 'px' && track.unit !== du) {
            const from = track.unit;
            track.values = track.values.map((v) => convertUnit(el, prop, v, from, du));
            track.unit = du;
          }
        }
        const idx = addTrack(track, spec.ease, spec.offsets);
        const numeric = track.kind === TRACK_NUMBER && track.ch === 1;
        this._restore.push(snapshot(target, prop, kind));
        const post = postFor(prop, o);
        if (post) {
          const write = writer(target, prop, kind, track.unit);
          const unit = track.unit;
          const raw = kind === Kind.Transform || (kind === Kind.Prop && !unit);
          const fmt = track.format;
          part.b.push(custom(target, prop, idx, 0, numeric
            ? (at) => {
                const v = post(F64[at], target);
                write(typeof v === 'number' && !raw ? r3(v) + unit : v);
              }
            : (at) => write(post(fmt(at), target))));
        } else {
          part.b.push(bind(target, prop, kind, idx, numeric, track.unit, track.format));
        }
        if (ctx.chain) chainSet(ctx.chain, target, prop, track.last);
      }

      if (tracks.length === 0) continue;

      // Output offsets per track.
      let off = 0;
      const offsets: number[] = [];
      for (const t of tracks) {
        offsets.push(off);
        off += t.out;
      }
      for (const b of part.b) b.off += offsets[b.track];

      // Timing.
      const easeForDur = easeList ? easeList[0] : baseEase;
      let dur: number;
      if (ctx.set || reduce) dur = 0;
      else if (o.duration !== undefined) dur = resolve(o.duration, target, i, n);
      else if (isSpringEase(easeForDur)) {
        const d = easingDuration(easingId(easeForDur as EasingInput));
        dur = d > 0 ? d : 0.5;
      } else dur = 0.5;
      const delay = ctx.set || reduce ? 0 : resolve(o.delay ?? 0, target, i, n);
      if (i > 0 && i === n - 1 && typeof o.delay === 'function' && delay >= 100) warn(`animate(): the delay function gives the last target a delay of ${delay} seconds. Times in anikit are in seconds: use stagger(0.05), not stagger(50).`);
      let durMs = Math.max(0, dur * 1000);
      const rd = (o.repeatDelay ?? 0) * 1000;
      if (rd > 0 && durMs > 0) {
        // Hold segment appended to each iteration.
        const k = durMs / (durMs + rd);
        for (const t of tracks) {
          const nk = t.values.length / t.ch;
          const offs = t.offsets ?? Array.from({ length: nk }, (_, j) => (nk === 1 ? 1 : j / (nk - 1)));
          t.offsets = offs.map((x) => x * k).concat(1);
          t.values = t.values.concat(t.values.slice(-t.ch));
          if (t.easings && t.easings.length > 1) t.easings.push(LINEAR);
        }
        durMs += rd;
      }
      let flags = fill;
      const immediate = o.immediateRender ?? (!ctx.child && hasExplicitFrom && delay > 0);
      if (immediate) {
        flags |= FL_IMMEDIATE;
        this._immediate = true;
      }

      part.tm = [delay * 1000, durMs, repeat + 1, direction];
      part.h = createAnim(
        delay * 1000, durMs, repeat + 1, direction, (o.endDelay ?? 0) * 1000, flags, tracks,
      );
      if (!part.h) throw new Error('anikit: failed to create animation (invalid keyframes)');
      register(part);
      if (claims) for (const b of part.b) claim(target, b.prop, part);
      part.onEmpty = () => this._partEmpty(part);
      this._parts.push(part);
      this._links.push(part);
    }

    // Root node: the single part, or a group of all parts.
    if (this._parts.length === 1) {
      const p = this._parts[0];
      this._h = p.h;
      p.event = (k) => p.ctl?._event(k);
    } else if (this._parts.length > 1) {
      this._group = w.ak_group_create(0, 0, 1, 0, 0, -1, FL_FILL_BACKWARDS | FL_FILL_FORWARDS, 1);
      for (const p of this._parts) w.ak_group_add(this._group, p.h, 0);
      this._h = this._group;
      const rec: NodeRecord & Linked = {
        h: this._group, at: 0, ctl: this,
        event: (k: number) => rec.ctl?._event(k),
      };
      register(rec);
      this._links.push(rec);
    } else {
      this._finish();
      return;
    }
    this._end = w.ak_duration(this._h) / 1000;
    if (o.speed !== undefined && o.speed !== 1) w.ak_set_rate(this._h, o.speed);
    if (ctx.set) {
      w.ak_seek(this._h, 0);
      flush();
      this.kill();
      return;
    }
    if (!ctx.child && o.autoplay !== false) {
      w.ak_play(this._h);
      // Immediate render of start values (e.g. staggered `from` values).
      if (this._immediate) {
        w.ak_seek(this._h, 0);
        flush();
      }
      wake();
    }
  }

  /** Special properties: motion `path`, morph `d`, stroke `draw`, `attr`. */
  private _special(
    prop: string, rawIn: unknown, target: Target, i: number, n: number, part: Part, tracks: TrackDef[],
    addTrack: (t: TrackDef, e: EasingInput | EasingInput[] | undefined, o?: number[]) => number,
    cs: number, ctx: BuildContext,
  ): void {
    const raw = resolve(rawIn as FnValue<any>, target, i, n);
    if (prop === 'transform') {
      // Whole transform lists, interpolated like browsers do (matrix
      // decomposition + quaternion slerp in the core).
      const isEl = isElement(target);
      const vals = Array.isArray(raw) ? raw.map((v) => resolve(v, target, i, n)) : [null, raw];
      const current = () =>
        isEl ? ((target as HTMLElement).style.transform || computed(target as Element, 'transform') || 'none') : String((target as any).transform ?? 'none');
      const mats: number[] = [];
      for (const v of vals) mats.push(...parseTransform(String(v ?? current())));
      if (isEl) this._restore.push(snapshot(target, 'transform', Kind.Style));
      else this._restore.push(snapshot(target, 'transform', Kind.Prop));
      const t: TrackDef = {
        kind: TRACK_MATRIX, arg: 0, ch: 16, out: 16, values: mats, unit: '',
        format: (at) => toMatrix3d(F64, at), first: toMatrix3d(mats, 0), last: toMatrix3d(mats, mats.length - 16),
      };
      const idx = addTrack(t, undefined, this._times);
      const style = isEl ? (target as HTMLElement).style : null;
      part.b.push(custom(target, 'transform', idx, 0, style ? (at) => (style.transform = toMatrix3d(F64, at)) : (at) => ((target as any).transform = toMatrix3d(F64, at))));
      return;
    }
    if (prop === 'text') {
      const spec: TextOptions = isPlainObject(raw) && 'value' in raw ? (raw as TextOptions) : { value: String(raw) };
      const isEl = isElement(target);
      const textProp = isEl ? 'textContent' : 'text';
      this._restore.push(snapshot(target, textProp, Kind.Prop));
      const fromText = spec.from ?? String((target as any)[textProp] ?? '');
      const delim = spec.delimiter ?? '';
      const a = delim ? fromText.split(delim) : Array.from(fromText);
      const b = delim ? String(spec.value).split(delim) : Array.from(String(spec.value));
      const t: TrackDef = {
        kind: TRACK_NUMBER, arg: 0, ch: 1, out: 1, values: [0, 1], unit: '',
        format: () => 0, first: fromText, last: String(spec.value),
      };
      const idx = addTrack(t, undefined);
      const set = (v: string) => ((target as any)[textProp] = v);
      if (spec.type === 'scramble') {
        const pool = spec.chars ? SCRAMBLE_SETS[spec.chars] ?? spec.chars : SCRAMBLE_SETS.upper + SCRAMBLE_SETS.digits + SCRAMBLE_SETS.symbols;
        const speed = Math.min(1, Math.max(0.01, spec.speed ?? 1));
        let cache: string[] = [];
        let tick = 0;
        part.b.push(custom(target, 'text', idx, 0, (at) => {
          const p = Math.min(1, Math.max(0, F64[at]));
          if (p >= 1) return set(b.join(delim));
          const revealed = Math.floor(p * b.length);
          const len = Math.round(a.length + (b.length - a.length) * p);
          if ((tick += speed) >= 1 || cache.length !== len) {
            tick = 0;
            cache = Array.from({ length: len }, () => pool[Math.floor(Math.random() * pool.length)]);
          }
          const out: string[] = [];
          for (let k = 0; k < len; k++) {
            const real = k < revealed ? b[k] : undefined;
            if (real !== undefined) out.push(real);
            else {
              const target_ = b[k] ?? a[k] ?? '';
              out.push(/^\s+$/.test(target_) ? target_ : cache[k]);
            }
          }
          set(out.join(delim));
        }));
      } else {
        const total = Math.max(a.length, b.length);
        part.b.push(custom(target, 'text', idx, 0, (at) => {
          const k = Math.round(Math.min(1, Math.max(0, F64[at])) * total);
          set(b.slice(0, k).concat(a.slice(k)).join(delim));
        }));
      }
      return;
    }
    if (prop === 'attr' && isPlainObject(raw)) {
      for (const key of Object.keys(raw)) {
        this._restore.push(snapshot(target, key, Kind.Attr));
        const vals = Array.isArray(raw[key]) ? raw[key] : [null, raw[key]];
        const read = () => readValue(target, key, Kind.Attr);
        const t = buildTrack(vals.map((v: any) => resolve(v, target, i, n)), read, (v) => v, cs);
        const idx = addTrack(t, undefined);
        const b = bind(target, key, Kind.Attr, idx, false, t.unit, t.format);
        b.prop = 'attr:' + key;
        part.b.push(b);
      }
      return;
    }
    if (prop === 'path') {
      this._restore.push(snapshot(target, 'x', Kind.Transform));
      const spec: MotionPathOptions =
        isPlainObject(raw) && 'path' in raw ? (raw as MotionPathOptions) : { path: raw as PathSource };
      const w = core();
      const lut = w.ak_path_lut(writeStr(pathData(spec.path)), 0);
      const t: TrackDef = {
        kind: TRACK_PATH, arg: lut, ch: 1, out: 3, values: [spec.start ?? 0, spec.end ?? 1], unit: '',
        format: () => 0, first: 0, last: 0,
      };
      const idx = addTrack(t, undefined);
      // The LUT is copied into the animation; free the registry slot after creation.
      queueMicrotask(() => w.ak_path_lut_free(lut));
      const s = stateOf(target);
      const rot = spec.rotate;
      part.b.push(transformSlot(target, 'x', idx, 0, 0));
      part.b.push(transformSlot(target, 'y', idx, 1, 1));
      if (rot !== undefined && rot !== false) {
        const add = typeof rot === 'number' ? rot : 0;
        part.b.push(custom(target, 'rotate', idx, 2, (at) => {
          s.t[3] = F64[at] + add;
          markDirty(s);
        }));
      }
      return;
    }
    if (prop === 'd') {
      const el = target as Element;
      const vals = Array.isArray(raw) ? raw.map((v) => resolve(v, target, i, n)) : [null, raw];
      const chained = ctx.chain?.get(target)?.get('d');
      const from = vals[0] ?? chained ?? (isElement(el) ? el.getAttribute('d') : (target as any).d) ?? '';
      const to = vals[vals.length - 1];
      void to;
      // Any number of shapes: normalise them all to one structure (core).
      const shapes = [from, ...vals.slice(1).map((v: unknown) => v ?? from)] as PathSource[];
      const seq = morphSequence(shapes);
      const tmpl = seq.template;
      const values: number[] = [];
      for (const f of seq.frames) values.push(...f);
      const ch = seq.frames[0].length;
      const t: TrackDef = {
        kind: TRACK_NUMBER, arg: 0, ch, out: ch, values, unit: '',
        format: (at) => writeMorph(tmpl, F64, at),
        first: writeMorph(tmpl, seq.frames[0]), last: writeMorph(tmpl, seq.frames[seq.frames.length - 1]),
      };
      if (isElement(el)) this._restore.push(snapshot(target, 'd', Kind.Attr));
      const idx = addTrack(t, undefined, this._times);
      const write = isElement(el) ? (v: string) => el.setAttribute('d', v) : (v: string) => ((target as any).d = v);
      const mb = custom(target, 'd', idx, 0, (at) => write(writeMorph(tmpl, F64, at)));
      ((this as any)._dFmt ??= new Map()).set(mb, (at: number) => writeMorph(tmpl, F64, at));
      part.b.push(mb);
      if (ctx.chain) chainSet(ctx.chain, target, 'd', t.last);
      return;
    }
    if (prop === 'draw') {
      // draw: visible segment of a stroke as "start end" fractions (or a single end fraction).
      const el = target as SVGElement;
      this._restore.push(snapshot(target, 'strokeDasharray', Kind.Style), snapshot(target, 'strokeDashoffset', Kind.Style));
      const len = elementLength(el);
      const parse = (v: unknown): [number, number] => {
        if (v === null || v === undefined) return [0, 1];
        if (typeof v === 'number') return [0, v];
        const p = String(v).trim().split(/[\s,]+/).map(parseFloat);
        return p.length > 1 ? [p[0], p[1]] : [0, p[0]];
      };
      const vals = Array.isArray(raw) ? raw : [null, raw];
      const pairs = vals.map((v) => parse(resolve(v, target, i, n)));
      if (vals[0] === null || vals[0] === undefined) {
        const cur = (el as any).__akDraw as [number, number] | undefined;
        if (cur) pairs[0] = cur;
      }
      const t: TrackDef = {
        kind: TRACK_NUMBER, arg: 0, ch: 2, out: 2, values: pairs.flat(), unit: '',
        format: () => 0, first: 0, last: 0,
      };
      const idx = addTrack(t, undefined);
      const style = (el as any).style as CSSStyleDeclaration;
      part.b.push(custom(target, 'draw', idx, 0, (at) => {
        const a = F64[at];
        const b = F64[at + 1];
        const s = Math.min(a, b);
        const e = Math.max(a, b);
        (el as any).__akDraw = [a, b];
        style.strokeDasharray = r3((e - s) * len) + 'px ' + r3(len + 1) + 'px';
        style.strokeDashoffset = r3(-s * len) + 'px';
      }));
      return;
    }
  }

  /** All properties of a part were taken over: free it. */
  private _partEmpty(part: Part): void {
    if (this._parts.every((p) => p.b.every((b) => !b.on))) {
      this.kill();
      return;
    }
    void part;
  }

  /**
   * Bake this animation into native CSS: `@keyframes` plus a rule using it.
   * Values are sampled from the engine (any easing, spring, colour space or
   * keyframe set works); transforms, styles and CSS variables are included.
   * One block per target (`name-0`, `name-1`, …) when there are several.
   */
  toCSS(options: { name?: string; selector?: string; samples?: number } = {}): string {
    const w = core();
    const name = options.name ?? 'anikit';
    const samples = Math.max(2, options.samples ?? 30);
    const dirs = ['normal', 'reverse', 'alternate', 'alternate-reverse'];
    let css = '';
    this._parts.forEach((part, pi) => {
      const nm = this._parts.length > 1 ? name + '-' + pi : name;
      css += '@keyframes ' + nm + ' {\n';
      for (let k = 0; k < samples; k++) {
        const p = k / (samples - 1);
        const base = w.ak_sample(part.h, p) >>> 3;
        views();
        const decls: string[] = [];
        let ts: TargetState | null = null;
        for (const b of part.b) {
          if (!b.on) continue;
          const at = base + b.off;
          if (b.mode === M_TRANSFORM || b.mode === M_TRANSFORM2) {
            if (!ts) {
              const src = b.obj as TargetState;
              ts = new TargetState(src.el);
              ts.t = src.t.slice();
              ts.u = src.u.slice();
            }
            ts.t[b.slot] = F64[at];
            if (b.mode === M_TRANSFORM2) ts.t[b.slot2] = F64[at];
          } else if (b.mode === M_STYLE_NUM) decls.push(kebab(b.key) + ': ' + r3(F64[at]) + b.unit);
          else if (b.mode === M_STYLE_FMT || b.mode === M_VAR) decls.push(kebab(b.key) + ': ' + b.fmt!(at));
          else if (b.prop === 'd' && b.mode === M_CUSTOM) {
            const d = (this as any)._dFmt?.get(b) as ((at: number) => string) | undefined;
            if (d) decls.push(`d: path("${d(at)}")`);
          }
        }
        if (ts) decls.unshift('transform: ' + composeTransform(ts));
        css += '  ' + r3(p * 100) + '% { ' + decls.join('; ') + ' }\n';
      }
      css += '}\n';
      if (options.selector) {
        const [delay, dur, it, dir] = part.tm;
        const sel = this._parts.length > 1 ? `${options.selector}:nth-of-type(${pi + 1})` : options.selector;
        css += `${sel} { animation: ${nm} ${r3(dur / 1000)}s linear ${r3(delay / 1000)}s ${it === Infinity ? 'infinite' : it} ${dirs[dir]} both; }\n`;
      }
    });
    return css;
  }

  /** Kill and restore every animated property to its value before this animation. */
  override revert(): void {
    whenReady(() => {
      this.kill();
      for (const r of this._restore.splice(0).reverse()) r();
    });
  }

  protected _kill(): void {
    const w = core();
    for (const p of this._parts) p.dispose();
    if (this._group) unregister({ h: this._group, at: 0 });
    // Releasing the root frees the whole subtree (group + parts).
    if (this._h) w.ak_release(this._h);
    this._parts = [];
    this._h = 0;
  }

  protected _cleanup(): () => void {
    const parts = this._parts;
    const group = this._group;
    const root = this._h;
    return () => {
      const w = core();
      for (const p of parts) p.dispose();
      if (group) unregister({ h: group, at: 0 });
      if (root) w.ak_release(root);
    };
  }
}

function chainSet(chain: Chain, target: object, prop: string, v: Scalar): void {
  let m = chain.get(target);
  if (!m) chain.set(target, (m = new Map()));
  m.set(prop, v);
}


/** Encode tracks into the scratch buffer and create the animation in the core. */
export function createAnim(
  delay: number, duration: number, iterations: number, direction: number, endDelay: number,
  flags: number, tracks: TrackDef[],
): number {
  let len = 8;
  for (const t of tracks) {
    const nk = t.values.length / t.ch;
    len += 5 + (t.offsets ? nk : 0) + t.values.length + 1 + (t.easings ? t.easings.length : 0);
  }
  let at = f64Buf(len);
  const f = F64;
  f[at++] = delay;
  f[at++] = duration;
  f[at++] = iterations;
  f[at++] = direction;
  f[at++] = endDelay;
  f[at++] = -1;
  f[at++] = flags;
  f[at++] = tracks.length;
  for (const t of tracks) {
    const nk = t.values.length / t.ch;
    f[at++] = t.kind;
    f[at++] = t.arg;
    f[at++] = t.ch;
    f[at++] = nk;
    f[at++] = t.offsets ? 1 : 0;
    if (t.offsets) for (let k = 0; k < nk; k++) f[at++] = t.offsets[k];
    for (let k = 0; k < t.values.length; k++) f[at++] = t.values[k];
    const e = t.easings ?? [];
    f[at++] = e.length;
    for (let k = 0; k < e.length; k++) f[at++] = e[k] === LINEAR ? -1 : e[k];
  }
  return core().ak_anim_create(len);
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/** Animate targets to (or through) values. Returns a controllable, awaitable {@link Animation}. */
export function animate(targets: Targets, keyframes: Props | Keyframe[], options?: AnimateOptions): Animation {
  return new Animation(targets, keyframes, options);
}

/** Animate from the given values to the current ones. */
export function from(targets: Targets, values: Props, options: AnimateOptions = {}): Animation {
  const kf: Props = {};
  for (const k of Object.keys(values)) {
    const v = values[k];
    kf[k] = OPTION_KEYS.has(k) || k === 'path' || k === 'attr' ? v : ([v, null] as PropValue);
  }
  return new Animation(targets, kf, { immediateRender: true, ...options });
}

/** Animate between explicit start and end values. */
export function fromTo(targets: Targets, fromValues: Props, toValues: Props, options: AnimateOptions = {}): Animation {
  const kf: Props = {};
  for (const k of Object.keys(toValues)) {
    kf[k] = OPTION_KEYS.has(k) || !(k in fromValues) ? toValues[k] : ([fromValues[k], toValues[k]] as PropValue);
  }
  return new Animation(targets, kf, { immediateRender: true, ...options });
}

/** Set values instantly (no animation). Takes properties over from running animations. */
export function set(targets: Targets, values: Props): void {
  new Animation(targets, values, {}, { set: true });
}
