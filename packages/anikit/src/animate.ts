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
import { core, F64, LINEAR, f64Buf, whenReady, readyNow, views } from './wasm';
import {
  register, unregister, wake, flush, easingId, easingDuration, NodeRecord, Runner, INDEX_MASK, EasingInput, prefersReduced, reducedMotionMode,
  FL_FILL_BACKWARDS, FL_FILL_FORWARDS, FL_IMMEDIATE,
} from './core';
import {
  buildTrack, numberTrack, TrackDef, TRACK_NUMBER, TRACK_PATH, TRACK_MATRIX, COLOR_SPACES, ColorSpaceName, r3, s3,
} from './values';
import {
  Target, Targets, toTargets, classify, Kind, readValue, writer, convertUnit, defaultUnit, isElement,
  claimAll, unclaimAll, Owner, stateOf, TEXT_PROPS, markDirty, transformUnit, TargetState, snapshot, composeTransform, kebab, computed, camel,
} from './props';
import { Playable, Callbacks, Linked } from './playable';
import { warn, didYouMean, describe, cssPropertyNames, suggest, warningsOn, checkSeconds, checkTargets } from './debug';
import { stagger as staggerFn } from './stagger';
import type { StaggerOptions } from './stagger';
import type { OptionKeys } from './debug';
import { Binding, bind, apply, custom, transformSlot, M_PROP, M_PROP_FMT, M_TRANSFORM, M_TRANSFORM2, M_STYLE_NUM, M_STYLE_FMT, M_VAR, M_CUSTOM } from './bind';
import { pathData, PathSource, writeMorph, elementLength, morphSequence, transformPath } from './svg';
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
  /** Progress along the path to stop at (0–1). Default 1. */
  end?: number;
  /** Rotate the target to follow the path tangent; a number adds an angle offset. */
  rotate?: boolean | number;
  /**
   * Put the target exactly on the path as drawn on the page: the path's
   * coordinates (its SVG user space, viewBox and all) are mapped to the
   * target's. `true` uses the path element itself; pass an SVG element when
   * `path` is raw path data in that element's coordinates.
   */
  align?: boolean | Element | string;
  /** Point of the target that sits on the path, as fractions of its box (default `[0.5, 0.5]` with `align`). */
  alignOrigin?: [number, number];
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
  /**
   * Shorthand for `delay: stagger(each)` (GSAP style). The object form takes the
   * stagger options plus `each` / `amount`: `{ each: 0.05, from: 'center' }`.
   */
  stagger?: number | (StaggerOptions & { each?: number; amount?: number });
  /** `true` loops forever (same as `repeat: Infinity`). */
  loop?: boolean;
}

/** Every `AnimateOptions` key (compiler-checked; used for "unknown option" warnings). */
export const ANIMATE_KEYS: OptionKeys<AnimateOptions> = {
  duration: 0, delay: 0, endDelay: 0, ease: 0, repeat: 0, repeatDelay: 0, yoyo: 0, direction: 0, speed: 0, autoplay: 0,
  times: 0, colorSpace: 0, fillMode: 0, overwrite: 0, immediateRender: 0, onStart: 0, onUpdate: 0, onComplete: 0,
  onRepeat: 0, onReverseComplete: 0, snap: 0, round: 0, modifiers: 0, format: 0, locale: 0, stagger: 0, loop: 0,
};
const OPTION_KEYS = new Set(Object.keys(ANIMATE_KEYS));

const DIRECTIONS = { normal: 0, reverse: 1, alternate: 2, 'alternate-reverse': 3 } as const;
/** Options accepted for compatibility / by wrappers (not warned about). */
const EXTRA_OPTION_KEYS = new Set(['distance', 'from', 'intensity', 'axis', 'at', 'charDelay', 'start', 'to', 'chars', 'delimiter', 'text', 'size', 'color']);
const TRANSFORM_PROPS = ['x', 'y', 'z', 'rotate', 'rotateX', 'rotateY', 'rotateZ', 'scale', 'scaleX', 'scaleY', 'scaleZ', 'skew', 'skewX', 'skewY', 'perspective', 'xPercent', 'yPercent'];
const SPECIAL_PROPS = ['path', 'draw', 'd', 'attr', 'text', 'transform'];
/** Option names from other libraries (Web Animations, anime.js, GSAP, Motion) → the anikit form. */
const OPTION_ALIASES: Record<string, string> = {
  easing: 'ease', iterations: 'repeat (extra iterations: iterations - 1; Infinity loops)', fill: 'fillMode', paused: 'autoplay: false',
  playbackRate: 'speed', reversed: "direction: 'reverse'", onEnd: 'onComplete', onFinish: 'onComplete', onBegin: 'onStart',
  onPlay: 'onStart', onLoop: 'onRepeat', startDelay: 'delay', iterationStart: 'seek()', keyframes: 'the second argument (keyframes)',
};
const FILL_MODES = ['both', 'forwards', 'backwards', 'none'];
/** Elements whose position was already checked (one style read per element, not per call). */
const positionChecked = new WeakSet<Element>();
const ANGLE_UNITS = ['', 'deg', 'rad', 'turn', 'grad'];

/** Construction-time validation of animation options (only while warnings are on). */
function checkAnimateOptions(given: AnimateOptions, o: AnimateOptions): void {
  for (const k of Object.keys(given)) {
    if (OPTION_KEYS.has(k) || EXTRA_OPTION_KEYS.has(k)) continue;
    const alias = OPTION_ALIASES[k];
    warn(`unknown animation option "${k}"; it is ignored.${alias ? ` Use ${alias} instead.` : didYouMean(k, OPTION_KEYS)}`);
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
  } else if ((prop === 'left' || prop === 'top' || prop === 'right' || prop === 'bottom') && isElement(target) && !positionChecked.has(target) && positionChecked.add(target) && computed(target, 'position') === 'static') {
    warn(`"${prop}" has no effect on ${describe(target)} because it is position: static. Animate x / y (transforms, faster) instead, or give it position: relative.`);
  } else if (prop === 'opacity' && isElement(target)) {
    const big = raw.find((v) => typeof v === 'number' && v > 1);
    if (big !== undefined) warn(`opacity goes from 0 to 1, but got ${big} on ${describe(target)}. Did you mean ${+((big as number) / 100).toFixed(3)}?`);
  }
}

function isKnownStyle(el: HTMLElement, prop: string): boolean {
  const st = el.style as any;
  return !st || prop in st || camel(prop) in st || prop.startsWith('--');
}

/** Layout properties whose animation moves things (held still under reduced motion 'fade'). */
const MOTION_STYLE = /^(width|height|(min|max)(Width|Height)|blockSize|inlineSize|top|left|right|bottom|inset.*|margin.*|padding.*|translate|rotate|scale|offset(Distance|Path|Rotate)?|backgroundPosition.*|objectPosition|transform|perspective|gap|rowGap|columnGap|letterSpacing|wordSpacing|lineHeight|fontSize)$/;

const MOTION_ATTR = /^(x|y|cx|cy|r|rx|ry|x1|x2|y1|y2|dx|dy|width|height|points|transform|d|offset)$/;

/** Collapse a track onto its last keyframe (values stay, motion doesn't). */
function stillTrack(t: TrackDef): void {
  const ch = t.ch;
  const last = t.values.slice(-ch);
  const nk = t.values.length / ch;
  const v: number[] = [];
  for (let k = 0; k < nk; k++) v.push(...last);
  t.values = v;
}

/** Size properties that accept `'auto'` as an animation value. */
const AUTO_SIZE = new Set(['height', 'width', 'maxHeight', 'maxWidth', 'blockSize', 'inlineSize']);

/** The used size (px) of `prop` when set to `auto`, measured without leaving a trace. */
export function measureAuto(el: HTMLElement, prop: string): number {
  const style = el.style as any;
  const key = camel(prop);
  const prev = style[key];
  const max = key.replace(/^(width|height)$/, (m) => 'max' + m[0].toUpperCase() + m.slice(1));
  const prevMax = max !== key ? style[max] : undefined;
  style[key] = /^max/.test(key) ? 'none' : 'auto';
  if (prevMax !== undefined) style[max] = 'none';
  const vertical = /height|block/i.test(key);
  let px = parseFloat(computed(el, key.replace(/^max(.)/, (_, c) => c.toLowerCase())));
  if (!Number.isFinite(px)) {
    // No layout engine (or a non-px computed value): fall back to the content size.
    px = vertical ? el.scrollHeight : el.scrollWidth;
  }
  style[key] = prev;
  if (prevMax !== undefined) style[max] = prevMax;
  return px || 0;
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------


/**
 * Writer for one property set: `t["x"] = f[a + 0]; t["y"] = f[a + 1]; …`
 * compiled once per distinct key/offset signature and shared by every Part
 * with that signature. Constant keys make each store a monomorphic named
 * store, where the generic `t[k[i]] = …` loop is a megamorphic keyed store
 * (V8 boxes the double and probes the stub cache): ~3x faster at 50k objects.
 * Same values, same order, so results are identical to the loop.
 */
type Writer = (t: any, f: Float64Array, at: number) => void;
/** `undefined`: not probed yet; `false`: code generation is blocked (CSP) or disabled. */
let codegen: boolean | undefined;
/** Testing hook: force the generic loop (`false`) or re-probe (`undefined`); returns the previous state. */
export function _setCodegen(on: boolean | undefined): boolean | undefined {
  const prev = codegen;
  codegen = on;
  compactCache.clear();
  lastCompact = null;
  return prev;
}
/**
 * Dense tables of a key set, by node index: the handle of the Part writing
 * through that key set's runner there (0 if none), its target and its output
 * offset. Only Parts on the compact path without `onUpdate` are entered
 * (`Part.slot`), and they leave on `release` and `dispose`, so the runner
 * checks one handle and writes one target per record without loading the
 * record or the Part (the handle's generation guards reused indices).
 */
interface Slots {
  h: number[];
  t: unknown[];
  a: number[];
}
function makeWriter(keys: string[], offs: Int32Array): [Writer, Runner, Slots | null] | null {
  if (codegen === false) return null;
  // Strict, like this module: writing to a frozen target or a getter-only
  // property throws here exactly as it does in the generic loop.
  let stores = '';
  for (let i = 0; i < keys.length; i++) {
    if (typeof keys[i] !== 'string') return null;
    stores += `t[${JSON.stringify(keys[i])}]=f[a+${offs[i]}];`;
  }
  try {
    const fn = new Function('t', 'f', 'a', '"use strict";' + stores) as Writer;
    // The same stores as a loop over consecutive updated Parts that use
    // this writer (see `flush`): what `Part.render` does for each of them
    // in the compact case (`pm === 1`, no `onUpdate`), without a call per
    // Part and with one store site per key for this property set only.
    // A single property at offset 0 reads the engine's first-output array
    // instead (same number, `Engine::first_outputs`): consecutive targets
    // of one call are consecutive there, not one buffer each. Both loops
    // stop if a setter grew memory (detached views) or, for that array,
    // created animations (it may have moved: `ak_out0_epoch_ptr`). That loop
    // also reads the Parts from this key set's dense tables (`Slots`).
    const one = keys.length === 1 && offs[0] === 0;
    const slots: Slots = { h: [], t: [], a: [] };
    const run = (
      one
        ? new Function(
          'H', 'T',
          'return function run(R,U,up,i,n,f,w){"use strict";' +
            'const v0=w.ak_out0_ptr()>>>3,ep=w.ak_out0_epoch_ptr()>>>2,e0=U[ep];' +
            `for(;i<n;i++){if(U[ep]!==e0)break;const h=U[up+i]|0,x=h&${INDEX_MASK};if(H[x]!==h)break;` +
            `T[x][${JSON.stringify(keys[0])}]=f[v0+x];}return i;}`,
        )(slots.h, slots.t)
        : new Function(
          'H', 'T', 'A',
          'return function run(R,U,up,i,n,f,w){"use strict";' +
            'for(;i<n;i++){if(f.length===0)break;' +
            `const h=U[up+i]|0,x=h&${INDEX_MASK};if(H[x]!==h)break;` +
            `const t=T[x],a=A[x];${stores}}return i;}`,
        )(slots.h, slots.t, slots.a)
    ) as Runner;
    codegen = true;
    return [fn, run, slots];
  } catch {
    // Content-Security-Policy without 'unsafe-eval': probe once, then use the loop.
    codegen = false;
    return null;
  }
}

/**
 * Testing hook: entries of the compact writers' dense tables (`Slots`) whose
 * node is no longer alive. Always 0 unless a Part outlived its node without
 * leaving its table (a stale entry is written through if the slot's handle
 * comes round again).
 */
export function _staleSlots(): number {
  const w = core();
  let n = 0;
  compactCache.forEach((c) => {
    const s = c[4];
    if (s !== null) for (let x = 0; x < s.h.length; x++) if (s.h[x] !== 0 && !w.ak_alive(s.h[x])) n++;
  });
  return n;
}

/** `false`: formatted single-property Parts take the general binding loop (tests compare both). */
let compactFmt = true;
/** Testing hook: turn path 3 of `Part` off/on; returns the previous state. Applies to Parts prepared afterwards. */
export function _setCompactFmt(on: boolean): boolean {
  const prev = compactFmt;
  compactFmt = on;
  return prev;
}

/** Key/offset arrays (and their writer and runner) shared by Parts with the same property set (see `Part.prep`). */
type Compact = [string[], Int32Array, Writer | null, Runner | null, Slots | null];
const compactCache = new Map<string, Compact>();
/** The entry used last: consecutive Parts (the targets of one call) usually share it, so no signature string is built for them. */
let lastCompact: Compact | null = null;
function sameCompact(b: Binding[], c: Compact): boolean {
  const keys = c[0], offs = c[1];
  if (keys.length !== b.length) return false;
  for (let i = 0; i < b.length; i++) if (b[i].key !== keys[i] || b[i].off !== offs[i]) return false;
  return true;
}

/** `Part.event` of an animation's only Part (its root): one shared function, called with the Part as `this`. */
function rootEvent(this: Part, k: number): void {
  this.ctl?._event(k);
}

/** `Part.tm` before the node is created. */
const NO_TIMING: readonly [number, number, number, number] = Object.freeze([0, 0, 1, 0] as [number, number, number, number]);

/** Per-target engine node. */
class Part implements NodeRecord, Owner, Linked {
  h = 0;
  at = 0;
  b: Binding[] = [];
  ctl: Playable | null;
  /**
   * The animation told when every binding was released (`_partEmpty`). A
   * field, not a closure per Part: `ctl` is cleared when the controller
   * detaches, this stays (as the closure it replaces did).
   */
  own: Animation | null;
  constructor(ctl: Playable, public claims: boolean, own: Animation | null) {
    this.ctl = ctl;
    this.own = own;
  }
  /** Whether the controller wants per-frame `onUpdate` notifications. */
  upd = false;
  /** Timing used to create the node: [delayMs, durationMs, iterations, direction] (shared, read-only). */
  tm: readonly [number, number, number, number] = NO_TIMING;
  /**
   * Compact form when every binding writes a plain property of one target
   * (the common case for objects): the target plus key/offset arrays shared
   * by every Part with the same property set, so the hot loop touches the
   * Part, the target and two arrays that stay in cache — not one Binding
   * object per property (measured ~3x faster at 50k objects).
   */
  pt: any = null;
  pk: string[] | null = null;
  po: Int32Array | null = null;
  pw: Writer | null = null;
  /** The writer's batch form, used by `flush` (see `makeWriter`). */
  run: Runner | null = null;
  /**
   * One formatted plain property (`pm === 3`: colour, `'12px'`, complex
   * strings on objects): its key, output offset, formatter and the value
   * last written, copied from the binding so the frame loop touches the Part
   * and the target only, not the bindings array and the Binding (at 50k
   * targets those dependent loads were ~half of the frame). `last` is the
   * binding's `ls` while this path is on and goes back to it on `release`.
   */
  pk1 = '';
  po1 = 0;
  pf: ((at: number) => string | number) | null = null;
  last: unknown = undefined;
  /** 0 = not checked yet, 1 = compact path, 2 = general path, 3 = one formatted property. */
  pm = 0;
  private prep(): void {
    const b = this.b;
    this.pm = 2;
    if (b.length === 0 || b.length > 16) return;
    const t = b[0].target;
    if (b.length === 1 && compactFmt) {
      const x = b[0];
      if (x.on && x.mode === M_PROP_FMT) {
        this.pt = t;
        this.pk1 = x.key;
        this.po1 = x.off;
        this.pf = x.fmt;
        this.last = x.ls;
        this.pm = 3;
        return;
      }
    }
    for (let i = 0; i < b.length; i++) {
      const x = b[i];
      if (!x.on || x.mode !== M_PROP || x.target !== t) return;
    }
    let shared = lastCompact;
    if (shared === null || !sameCompact(b, shared)) {
      const sig = b.map((x) => x.key + ':' + x.off).join(',');
      let c = compactCache.get(sig);
      if (c === undefined) {
        const keys = b.map((x) => x.key), offs = Int32Array.from(b, (x) => x.off);
        const w = makeWriter(keys, offs);
        c = [keys, offs, w && w[0], w && w[1], w && w[2]];
        compactCache.set(sig, c);
      }
      lastCompact = shared = c;
    }
    this.pt = t;
    this.pk = shared[0];
    this.po = shared[1];
    this.pw = shared[2];
    this.run = shared[3];
    this.pm = 1;
    const s = shared[4];
    if (s !== null && !this.upd) {
      const x = this.h & INDEX_MASK;
      // Grow the tables densely (holes would make them sparse arrays).
      while (s.h.length <= x) {
        s.h.push(0);
        s.t.push(undefined);
        s.a.push(0);
      }
      s.h[x] = this.h;
      s.t[x] = t;
      s.a[x] = this.at;
      this.slot = s;
    }
  }
  /** The dense tables this Part is entered in (see `Slots`), if any. */
  slot: Slots | null = null;
  /** Leave `slot` (before any change to the compact path, and on dispose). */
  private unslot(): void {
    const s = this.slot;
    if (s !== null) {
      const x = this.h & INDEX_MASK;
      if (s.h[x] === this.h) {
        s.h[x] = 0;
        s.t[x] = undefined;
      }
      this.slot = null;
    }
  }
  render(): void {
    const at = this.at;
    const f = F64;
    if (this.pm !== 2) {
      if (this.pm === 0) this.prep();
      if (this.pm === 1) {
        const w = this.pw;
        if (w !== null) w(this.pt, f, at);
        else {
          const t = this.pt, k = this.pk!, o = this.po!;
          for (let i = 0; i < k.length; i++) t[k[i]] = f[at + o[i]];
        }
        if (this.upd) {
          const c = this.ctl;
          if (c !== null) c._touch();
        }
        return;
      }
      if (this.pm === 3) {
        // Same as `apply` for an M_PROP_FMT binding.
        const v = this.pf!(at + this.po1);
        if (v !== this.last) {
          this.last = v;
          this.pt[this.pk1] = v;
        }
        if (this.upd) {
          const c = this.ctl;
          if (c !== null) c._touch();
        }
        return;
      }
    }
    const b = this.b;
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
    // Hand the last written value back to the binding before leaving path 3.
    if (this.pm === 3) this.b[0].ls = this.last;
    this.unslot();
    this.pm = 0; // re-check the compact path
    let live = 0;
    for (const x of this.b) {
      if (x.prop === prop) {
        x.on = false;
        if (!this.b.some((y) => y.on && y.track === x.track)) core().ak_track_enabled(this.h, x.track, 0);
      }
      if (x.on) live++;
    }
    if (live === 0) this.own?._partEmpty(this);
  }
  /** Every binding was released (counted once by the animation). */
  gone = false;
  dispose(): void {
    if (this.claims) unclaimAll(this.b, this);
    this.unslot();
    unregister(this);
  }
}

/**
 * One shared `easings` array per easing id (and one empty one for linear):
 * tracks only read them, and the one place that appends (`repeatDelay`)
 * only touches lists of more than one easing.
 */
const NO_EASINGS: number[] = [];
const singleEasings = new Map<number, number[]>();
function singleEasing(id: number): number[] {
  if (id === LINEAR) return NO_EASINGS;
  let a = singleEasings.get(id);
  if (a === undefined) singleEasings.set(id, (a = [id]));
  return a;
}

/** "Not read yet" marker for the current value in `_build`. */
const UNREAD: unknown = {};
/** Whether `buildTrack` reads the current value for these keyframes (a lone value or a `null` one). */
function usesCurrent(raw: readonly unknown[]): boolean {
  if (raw.length === 1) return true;
  for (let i = 0; i < raw.length; i++) if (raw[i] === null || raw[i] === undefined) return true;
  return false;
}
/** `false`: always use the general `buildTrack` (tests compare both paths). */
let numberTracks = true;
/** Testing hook: turn the plain-number track path off/on; returns the previous state. */
export function _setNumberTracks(on: boolean): boolean {
  const prev = numberTracks;
  numberTracks = on;
  return prev;
}
/** `false`: never create a multi-target animation's nodes in one batch (tests compare both). */
let batchCreate = true;
/** `_fast`'s value scratch (JS memory, not the core's), and whether a call is using it. */
let fastVals = new Float64Array(64);
let fastBusy = false;
/** Testing hook: turn batched creation off/on; returns the previous state. */
export function _setBatchCreate(on: boolean): boolean {
  const prev = batchCreate;
  batchCreate = on;
  return prev;
}
/** `false`: never pack a multi-target animation's group in the core (tests compare both). */
let packGroups = true;
/** Testing hook: turn group packing off/on for animations created afterwards; returns the previous state. */
export function _setPackGroups(on: boolean): boolean {
  const prev = packGroups;
  packGroups = on;
  return prev;
}


/**
 * `Animation._fast`'s reads: each target's current values (and `revert()`
 * values into `rp`), and every target's keyframe values into `vals`.
 * False if a target is an element or a needed current value is not a number.
 */
function readFast(
  targets: Target[], keys: string[], specs: PropSpec[], cur: boolean[], rp: unknown[], vals: Float64Array,
  chain: Chain | undefined,
): boolean {
  const n = targets.length, np = keys.length;
  for (let i = 0, r = 0, at = 0; i < n; i++) {
    const t = targets[i] as any;
    if (isElement(t)) return false;
    // A timeline's earlier steps set where this one starts.
    const ch = chain !== undefined ? chain.get(t) : undefined;
    for (let pi = 0; pi < np; pi++) {
      const key = keys[pi];
      const v = t[key];
      rp[r] = t;
      rp[r + 1] = key;
      rp[r + 2] = v;
      r += 3;
      let cn = 0;
      if (cur[pi]) {
        const c = ch !== undefined ? ch.get(key) : undefined;
        const x = c !== undefined ? c : v;
        if (typeof x !== 'number') return false;
        cn = x;
      }
      const raw = specs[pi].values as Array<number | null>;
      if (raw.length === 1) {
        vals[at++] = cn;
        vals[at++] = raw[0] === null || raw[0] === undefined ? cn : raw[0];
      } else {
        for (let k = 0; k < raw.length; k++) {
          const x = raw[k];
          vals[at++] = x === null || x === undefined ? cn : x;
        }
      }
    }
  }
  return true;
}

const seen = new Set<unknown>();
/** Whether a target list holds one target more than once. */
function hasDuplicates(targets: Target[]): boolean {
  let dup = false;
  for (let i = 0; i < targets.length && !dup; i++) {
    if (seen.has(targets[i])) dup = true;
    else seen.add(targets[i]);
  }
  seen.clear();
  return dup;
}

/** `Animation._fast`: each value's last keyframe is where the timeline's next step starts. */
function fastChain(chain: Chain, targets: Target[], keys: string[], nks: number[], vals: Float64Array): void {
  for (let i = 0, v = 0; i < targets.length; i++) {
    for (let pi = 0; pi < keys.length; pi++) {
      v += nks[pi];
      chainSet(chain, targets[i], keys[pi], vals[v - 1]);
    }
  }
}

/** `Animation._fast`: one Part per target, handles read from the f64 buffer at `base`. */
function fastParts(
  an: Animation, targets: Target[], keys: string[], base: number, tm: readonly [number, number, number, number],
  claims: boolean, upd: boolean,
): void {
  const n = targets.length, np = keys.length;
  const parts = an._parts;
  for (let i = 0; i < n; i++) {
    const target = targets[i];
    const part = new Part(an, claims, an);
    part.upd = upd;
    // (Packed, like the general path's arrays: a holey one makes every
    // loop over bindings polymorphic.)
    const b: Binding[] = [];
    for (let pi = 0; pi < np; pi++) b.push(new Binding(target, keys[pi], pi, pi, M_PROP, null, keys[pi], '', 0, -1, null, null));
    part.b = b;
    part.tm = tm;
    // (int32, as wasm returns handles: keeps the field a small integer.)
    part.h = F64[base + i] | 0;
    parts.push(part);
  }
}

/** `Animation._fast`: pack the group, then read every part's output location. */
function fastPack(w: ReturnType<typeof core>, g: number, parts: Part[], base: number): void {
  const n = parts.length;
  // Output locations, after packing (the pack moves them).
  if (packGroups) w.ak_group_pack(g);
  for (let i = 0; i < n; i++) F64[base + i] = parts[i].h >>> 0;
  w.ak_values_ptrs(n);
  views();
  for (let i = 0; i < n; i++) parts[i].at = F64[base + i] >>> 3;
}

/** `Animation._fast`: register the parts and claim their properties (may release older owners). */
function fastLink(an: Animation, targets: Target[], claims: boolean): void {
  const parts = an._parts;
  for (let i = 0; i < parts.length; i++) {
    const part = parts[i];
    register(part);
    if (claims) claimAll(targets[i], part.b, part);
    an._live++;
    an._links.push(part);
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

/** The default (empty) build context, shared. */
const NO_CTX: BuildContext = Object.freeze({});

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
  /** Parts that still write at least one property (see `_partEmpty`). */
  _live = 0;
  /** The group's node record (registered while the group exists). */
  private _grec: NodeRecord | null = null;
  private _immediate = false;
  /** Set while `_build` runs (see `_partEmpty`). */
  private _building = false;
  /** Restorers for `revert()` (captured before the first write). */
  _restore: Array<() => void> = [];
  /**
   * Plain-property values for `revert()`, flat: target, key, value. Kept
   * apart from `_restore`; both are captured before any write, and they
   * never cover the same target and key with different values.
   */
  _rp: unknown[] | null = null;
  private _times: number[] | undefined;
  /** Seconds; available once built. */
  _end = 0;

  constructor(targets: Targets, keyframes: Props | Keyframe[], options: AnimateOptions = {}, ctx: BuildContext = NO_CTX) {
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
    const st = opts.stagger;
    if (st !== undefined && opts.delay === undefined) {
      if (typeof st === 'number') opts.delay = staggerFn(st);
      else {
        const { each, amount, ...rest } = st;
        opts.delay = staggerFn(each ?? 0.1, { ...rest, total: rest.total ?? amount });
      }
    }
    if (opts.loop === true && opts.repeat === undefined) opts.repeat = Infinity;
    super(opts);
    if (warningsOn()) checkAnimateOptions(options ?? {}, opts);
    this._times = opts.times;
    // (No closure when the core is already there, the usual case.)
    if (readyNow()) this._build(targets, props, opts, ctx);
    else whenReady(() => this._build(targets, props, opts, ctx));
  }

  private _build(targetsIn: Targets, props: Map<string, PropSpec>, o: AnimateOptions, ctx: BuildContext): void {
    this._building = true;
    try {
      this._build1(targetsIn, props, o, ctx);
    } finally {
      this._building = false;
    }
    // Every part lost its properties to a later part of the same call.
    if (this._parts.length > 0 && this._live <= 0) this.kill();
  }

  private _build1(targetsIn: Targets, props: Map<string, PropSpec>, o: AnimateOptions, ctx: BuildContext): void {
    const w = core();
    const targets = (this.targets = toTargets(targetsIn));
    const n = targets.length;
    if (!ctx.quiet) checkTargets(ctx.child ? 'timeline.add()' : ctx.set ? 'set()' : 'animate()', targetsIn, n);
    const claims = !ctx.child && o.overwrite !== false;
    const cs = COLOR_SPACES[o.colorSpace ?? 'oklab'] ?? 2;
    // Reduced motion: everything jumps ('instant'), or only movement does ('fade').
    const reducedAny = prefersReduced();
    const reduce = reducedAny && reducedMotionMode() === 'instant';
    const still = reducedAny && !reduce;
    const repeat = reduce || o.repeat === undefined ? 0 : o.repeat < 0 ? Infinity : o.repeat;
    const direction =
      o.direction !== undefined ? DIRECTIONS[o.direction] ?? 0 : o.yoyo ? DIRECTIONS.alternate : 0;
    let fill = FL_FILL_BACKWARDS | FL_FILL_FORWARDS;
    if (o.fillMode === 'forwards') fill = FL_FILL_FORWARDS;
    else if (o.fillMode === 'backwards') fill = FL_FILL_BACKWARDS;
    else if (o.fillMode === 'none') fill = 0;
    const easeList = Array.isArray(o.ease) && !(o.ease.length === 4 && typeof o.ease[0] === 'number') ? o.ease : null;
    const baseEase = easeList ? undefined : (o.ease as EasingInput);
    // Per-property work that does not depend on the target, done once.
    // (Two arrays filled by `forEach`: spreading the Map was a sixth of `_build` for one target.)
    const keys: string[] = [], specs: PropSpec[] = [];
    props.forEach((spec, key) => {
      keys.push(key);
      specs.push(spec);
    });
    const np = keys.length;
    const posts: Array<((v: any, target: any) => any) | null> = [], dynamic: boolean[] = [];
    for (let pi = 0; pi < np; pi++) {
      const spec = specs[pi];
      posts.push(spec.raw === undefined ? postFor(keys[pi], o) : null);
      // Values with no per-target functions or `random()` strings resolve to themselves.
      dynamic.push(spec.values.some((v) => typeof v === 'function' || (typeof v === 'string' && RANDOM_RE.test(v))));
    }
    let tm: readonly [number, number, number, number] = NO_TIMING;
    // `_rp` while this call fills it, and its used length.
    let rp: unknown[] | null = null;
    let rpn = 0;
    // The current target's track list (one `addTrack` per call, not per target).
    let curTracks: TrackDef[] = [];
    const trackOffs: number[] = [];
    const addTrack = (t: TrackDef, ease: EasingInput | EasingInput[] | undefined, offsets?: number[], motion = false) => {
      if (still && motion) {
        // Hold the end value for the whole animation: no movement, same timing.
        stillTrack(t);
        if (t.segs) t.segs.forEach(stillTrack);
      }
      const nk = t.values.length / t.ch;
      if (offsets && offsets.length === nk) t.offsets = offsets;
      const e = ease ?? (easeList ? easeList : baseEase);
      if (Array.isArray(e) && !(e.length === 4 && typeof e[0] === 'number')) {
        const ids: number[] = [];
        for (let k = 0; k < nk - 1; k++) ids.push(easingId((e as EasingInput[])[k] ?? baseEase));
        t.easings = ids;
      } else {
        const id = easingId(e as EasingInput);
        t.easings = singleEasing(id);
      }
      const tracks = curTracks;
      tracks.push(t);
      const idx = tracks.length - 1;
      // Segments of a complex value follow it (same timing, contiguous outputs).
      if (t.segs) {
        for (const sg of t.segs) {
          if (t.offsets) sg.offsets = t.offsets.slice();
          sg.easings = t.easings ? t.easings.slice() : undefined;
          tracks.push(sg);
        }
      }
      return idx;
    };

    const fast = batchCreate && n > 0 && !ctx.set && (o.repeatDelay ?? 0) <= 0 &&
      this._fast(targets, keys, specs, posts, dynamic, o, ctx, claims, reduce, repeat, direction, fill, easeList, baseEase);
    if (!fast) for (let i = 0; i < n; i++) {
      const target = targets[i];
      const part = new Part(this, claims, this);
      part.upd = !!o.onUpdate;
      const tracks: TrackDef[] = (curTracks = []);
      let hasExplicitFrom = false;
      // One transform snapshot per target covers every transform component.
      let snappedTransform = false;


      for (let pi = 0; pi < np; pi++) {
        const prop = keys[pi], spec = specs[pi];
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
        let raw = dynamic[pi] ? spec.values.map((v) => resolve(v, target, i, n)) : (spec.values as Array<Scalar | null>);
        // A function may return keyframes for its target: `y: (el, i) => [0, i * 10]`.
        if (raw.length === 2 && raw[0] === null && Array.isArray(raw[1])) raw = raw[1] as Array<Scalar | null>;
        if (i === 0 && warningsOn()) checkValues(prop, kind, raw, target);
        if (raw[0] !== null && raw[0] !== undefined && raw.length > 1) hasExplicitFrom = true;
        // `height: 'auto'` (and width, min/max sizes): animate to the measured
        // size, then hand the property back to layout as `auto`.
        let autoPx: number[] | null = null;
        if (kind === Kind.Style && AUTO_SIZE.has(camel(prop))) {
          const el = target as HTMLElement;
          const inlineAuto = el.style && (el.style as any)[camel(prop)] === 'auto';
          if (raw.some((v, k) => v === 'auto' || (inlineAuto && k === 0 && (v === null || v === undefined)))) {
            const px = measureAuto(el, prop);
            autoPx = [px];
            raw = raw.slice(); // (may be the shared spec values)
            for (let k = 0; k < raw.length; k++) {
              if (raw[k] === 'auto' || (k === 0 && inlineAuto && (raw[k] === null || raw[k] === undefined))) raw[k] = px + 'px';
            }
          }
        }
        const chained = ctx.chain?.get(target)?.get(prop);
        const el = isElement(target) ? target : null;
        let du = defaultUnit(prop, kind);
        // Plain numbers (the common case) skip the general parser and its
        // closures; the current value is read at most once either way.
        let cur: unknown = UNREAD;
        let track: TrackDef | null = null;
        if (numberTracks) {
          if (usesCurrent(raw)) cur = chained !== undefined ? chained : readValue(target, prop, kind);
          track = numberTrack(raw, cur, du);
        }
        if (track === null) {
          const read = cur !== UNREAD ? () => cur : () => (chained !== undefined ? chained : readValue(target, prop, kind));
          const conv = (v: number, f: string, t: string) => convertUnit(el, prop, v, f, t);
          track = buildTrack(raw, read, conv, cs, du);
        }
        if (kind === Kind.Transform) {
          if (track.kind !== TRACK_NUMBER || track.ch !== 1 || track.complex) {
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
        const idx = addTrack(track, spec.ease, spec.offsets, kind === Kind.Transform || (kind === Kind.Style && MOTION_STYLE.test(camel(prop))));
        // (A one-number template such as `blur(4px)` is not a plain number.)
        const numeric = track.kind === TRACK_NUMBER && track.ch === 1 && !track.complex;
        if (kind === Kind.Prop) {
          // Plain properties: target, key and value, flat (no closure per property).
          // (Sized once for the call: growing by push was a fifth of `_build`.)
          if (rp === null) rp = this._rp = new Array(3 * n * np);
          rp[rpn] = target;
          rp[rpn + 1] = prop;
          rp[rpn + 2] = (target as any)[prop];
          rpn += 3;
        } else if (kind !== Kind.Transform || !snappedTransform) this._restore.push(snapshot(target, prop, kind));
        if (kind === Kind.Transform) snappedTransform = true;
        const post = posts[pi];
        if (autoPx && numeric && track.unit === 'px') {
          // Write `auto` whenever the value sits on the measured size (start
          // or end): identical on screen, and the element stays responsive.
          const style = (target as HTMLElement).style as any;
          const key = camel(prop);
          const ap = autoPx[0];
          const kw = /^max/.test(key) ? 'none' : 'auto';
          const b = custom(target, prop, idx, 0, (at) => {
            let v = F64[at];
            if (post) v = post(v, target);
            const s = Math.abs(v - ap) < 1e-3 ? kw : s3(v) + 'px';
            if (style[key] !== s) style[key] = s;
          });
          part.b.push(b);
        } else if (post) {
          const write = writer(target, prop, kind, track.unit);
          const unit = track.unit;
          const raw = kind === Kind.Transform || (kind === Kind.Prop && !unit);
          const fmt = track.format;
          part.b.push(custom(target, prop, idx, 0, numeric
            ? (at) => {
                const v = post(F64[at], target);
                write(typeof v === 'number' && !raw ? s3(v) + unit : v);
              }
            : (at) => write(post(fmt(at), target))));
        } else {
          part.b.push(bind(target, prop, kind, idx, numeric, track.unit, track.format));
        }
        if (ctx.chain) chainSet(ctx.chain, target, prop, track.last);
      }

      if (tracks.length === 0) continue;
      // Exact-size copy: the pushes above leave spare capacity the Part would keep.
      if (part.b.length < 8) part.b = part.b.slice();

      // Output offsets per track.
      // (One array per call, overwritten per target.)
      let off = 0;
      const offsets = trackOffs;
      for (let k = 0; k < tracks.length; k++) {
        offsets[k] = off;
        off += tracks[k].out;
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

      // One timing array for every Part of the call that has the same timing.
      const dm = delay * 1000;
      if (tm[0] !== dm || tm[1] !== durMs || tm[2] !== repeat + 1 || tm[3] !== direction) tm = [dm, durMs, repeat + 1, direction];
      part.tm = tm;
      part.h = createAnim(
        delay * 1000, durMs, repeat + 1, direction, (o.endDelay ?? 0) * 1000, flags, tracks,
      );
      if (!part.h) throw new Error('anikit: failed to create animation (invalid keyframes)');
      register(part);
      if (claims) claimAll(target, part.b, part);
      this._parts.push(part);
      const pb = part.b;
      for (let k = 0; k < pb.length; k++) {
        if (pb[k].on) {
          this._live++;
          break;
        }
      }
      this._links.push(part);
    }

    if (rp !== null && rpn !== rp.length) rp.length = rpn;

    // Root node: the single part, or a group of all parts.
    if (this._parts.length === 1) {
      const p = this._parts[0];
      this._h = p.h;
      p.event = rootEvent;
    } else if (this._parts.length > 1) {
      // (`_fast` creates its group, adds and packs its children itself.)
      if (!fast) {
        this._group = w.ak_group_create(0, 0, 1, 0, 0, -1, FL_FILL_BACKWARDS | FL_FILL_FORWARDS, 1);
        for (const p of this._parts) w.ak_group_add(this._group, p.h, 0);
        // Children of one layout and timing are evaluated together; their
        // outputs then live in the pack.
        if (packGroups && w.ak_group_pack(this._group)) for (const p of this._parts) p.at = w.ak_values_ptr(p.h) >>> 3;
      }
      this._h = this._group;
      const rec: NodeRecord & Linked = {
        h: this._group, at: 0, ctl: this,
        event: (k: number) => rec.ctl?._event(k),
      };
      register(rec);
      this._grec = rec;
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

  /**
   * Batched creation for the common multi-target case: plain objects, plain
   * numbers (no per-target values, units or post-processing), one timing.
   * Reads every current value first (getters are user code; see the
   * scratch-buffer rule), then creates every target's node in one
   * `ak_anim_create_many` call, without per-target track objects. Returns
   * false, having changed nothing, when the call does not qualify.
   */
  private _fast(
    targets: Target[], keys: string[], specs: PropSpec[], posts: Array<unknown>, dynamic: boolean[],
    o: AnimateOptions, ctx: BuildContext, claims: boolean, reduce: boolean, repeat: number, direction: number, fill: number,
    easeList: readonly unknown[] | null, baseEase: EasingInput | undefined,
  ): boolean {
    const n = targets.length, np = keys.length;
    if (np === 0 || np > 16) return false;
    if ((o.duration !== undefined && typeof o.duration !== 'number') || (o.delay !== undefined && typeof o.delay !== 'number')) return false;
    // Per property: its keyframes (numbers or null), count, offsets, easings.
    const nks: number[] = [], cur: boolean[] = [];
    let stride = 0, hasExplicitFrom = false;
    for (let pi = 0; pi < np; pi++) {
      const spec = specs[pi];
      if (spec.raw !== undefined || posts[pi] !== null || dynamic[pi] || TEXT_PROPS.has(keys[pi])) return false;
      const raw = spec.values;
      if (raw.length === 0) return false;
      for (let k = 0; k < raw.length; k++) {
        const v = raw[k];
        if (v !== null && v !== undefined && typeof v !== 'number') return false;
      }
      const nk = raw.length === 1 ? 2 : raw.length;
      nks.push(nk);
      cur.push(usesCurrent(raw));
      if (raw[0] !== null && raw[0] !== undefined && raw.length > 1) hasExplicitFrom = true;
      stride += nk;
    }
    // A target listed twice in a timeline step starts its second part where
    // the first ends (the general path chains them one by one).
    if (ctx.chain && n > 1 && hasDuplicates(targets)) return false;
    // Current values (and the values `revert()` restores), read once.
    // (`vals` is shared between calls unless a getter re-enters.)
    const rp: unknown[] = new Array(3 * n * np);
    const busy = fastBusy;
    let vals = fastVals;
    if (busy) vals = new Float64Array(n * stride);
    else if (vals.length < n * stride) vals = fastVals = new Float64Array(Math.max(n * stride, 2 * vals.length));
    fastBusy = true;
    let ok: boolean;
    try {
      ok = readFast(targets, keys, specs, cur, rp, vals, ctx.chain);
    } finally {
      fastBusy = busy;
    }
    if (!ok) return false;
    if (warningsOn()) for (let pi = 0; pi < np; pi++) checkValues(keys[pi], Kind.Prop, specs[pi].values, targets[0]);
    // Offsets and easing ids per property (as `addTrack` sets them).
    const offs: Array<number[] | undefined> = [], eases: number[][] = [];
    let len = 8;
    for (let pi = 0; pi < np; pi++) {
      const spec = specs[pi], nk = nks[pi];
      const of = spec.offsets && spec.offsets.length === nk ? spec.offsets : undefined;
      offs.push(of);
      const e = spec.ease ?? (easeList ? easeList : baseEase);
      let ids: number[];
      if (Array.isArray(e) && !(e.length === 4 && typeof e[0] === 'number')) {
        ids = [];
        for (let k = 0; k < nk - 1; k++) ids.push(easingId((e as EasingInput[])[k] ?? baseEase));
      } else ids = singleEasing(easingId(e as EasingInput));
      eases.push(ids);
      len += 5 + (of ? nk : 0) + nk + 1 + ids.length;
    }
    // Timing (the same for every target).
    const easeForDur = easeList ? easeList[0] : baseEase;
    let dur: number;
    if (reduce) dur = 0;
    else if (o.duration !== undefined) dur = o.duration as number;
    else if (isSpringEase(easeForDur)) {
      const d = easingDuration(easingId(easeForDur as EasingInput));
      dur = d > 0 ? d : 0.5;
    } else dur = 0.5;
    const delay = reduce ? 0 : ((o.delay as number | undefined) ?? 0);
    const durMs = Math.max(0, dur * 1000);
    let flags = fill;
    if (o.immediateRender ?? (!ctx.child && hasExplicitFrom && delay > 0)) {
      flags |= FL_IMMEDIATE;
      this._immediate = true;
    }
    // Encode: the first target in full, then the others' values only.
    // (Nothing below calls user code until the call has consumed the buffer.)
    const base = f64Buf(len + (n - 1) * stride);
    let at = base;
    const f = F64;
    f[at++] = delay * 1000;
    f[at++] = durMs;
    f[at++] = repeat + 1;
    f[at++] = direction;
    f[at++] = (o.endDelay ?? 0) * 1000;
    f[at++] = -1;
    f[at++] = flags;
    f[at++] = np;
    for (let pi = 0, v = 0; pi < np; pi++) {
      const nk = nks[pi], of = offs[pi], ids = eases[pi];
      f[at++] = TRACK_NUMBER;
      f[at++] = 0;
      f[at++] = 1;
      f[at++] = nk;
      f[at++] = of ? 1 : 0;
      if (of) for (let k = 0; k < nk; k++) f[at++] = of[k];
      for (let k = 0; k < nk; k++) f[at++] = vals[v++];
      f[at++] = ids.length;
      for (let k = 0; k < ids.length; k++) f[at++] = ids[k] === LINEAR ? -1 : ids[k];
    }
    if (n > 1) f.set(vals.subarray(stride, n * stride), at);
    const w = core();
    // Several targets are created straight into the call's group (see the
    // root node in `_build`).
    const g = n > 1 ? w.ak_group_create(0, 0, 1, 0, 0, -1, FL_FILL_BACKWARDS | FL_FILL_FORWARDS, 1) : 0;
    if (w.ak_anim_create_many(len, n, stride, g) !== n) {
      if (g) w.ak_release(g);
      throw new Error('anikit: failed to create animation (invalid keyframes)');
    }
    this._group = g;
    views();
    const tm: readonly [number, number, number, number] = [delay * 1000, durMs, repeat + 1, direction];
    // (The per-target loops live in functions of their own: optimised during
    // one large call, the method would keep that code for every small one.)
    if (ctx.chain) fastChain(ctx.chain, targets, keys, nks, vals);
    fastParts(this, targets, keys, base, tm, claims, !!o.onUpdate);
    if (n > 1) fastPack(w, g, this._parts, base);
    fastLink(this, targets, claims);
    this._rp = rp;
    return true;
  }

  /** Special properties: motion `path`, morph `d`, stroke `draw`, `attr`. */
  private _special(
    prop: string, rawIn: unknown, target: Target, i: number, n: number, part: Part, tracks: TrackDef[],
    addTrack: (t: TrackDef, e: EasingInput | EasingInput[] | undefined, o?: number[], motion?: boolean) => number,
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
      const idx = addTrack(t, undefined, this._times, true);
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
        const idx = addTrack(t, undefined, undefined, MOTION_ATTR.test(key));
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
      let data = pathData(spec.path);
      if (spec.align && isElement(target)) data = this._alignPath(data, spec, target as HTMLElement);
      const w = core();
      const lut = w.ak_path_lut(writeStr(data), 0);
      const t: TrackDef = {
        kind: TRACK_PATH, arg: lut, ch: 1, out: 3, values: [spec.start ?? 0, spec.end ?? 1], unit: '',
        format: () => 0, first: 0, last: 0,
      };
      const idx = addTrack(t, undefined, undefined, true);
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
      const idx = addTrack(t, undefined, this._times, true);
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
        style.strokeDasharray = s3((e - s) * len) + 'px ' + s3(len + 1) + 'px';
        style.strokeDashoffset = s3(-s * len) + 'px';
      }));
      return;
    }
  }

  /** Map path data from its SVG user space into `target`'s translate space. */
  private _alignPath(data: string, spec: MotionPathOptions, target: HTMLElement): string {
    const ref =
      spec.align === true
        ? typeof spec.path === 'string'
          ? /^\s*[mM]/.test(spec.path) ? null : document.querySelector(spec.path)
          : spec.path
        : typeof spec.align === 'string'
          ? document.querySelector(spec.align)
          : (spec.align as Element);
    const ctm = ref && typeof (ref as SVGGraphicsElement).getScreenCTM === 'function' ? (ref as SVGGraphicsElement).getScreenCTM() : null;
    const [ox, oy] = spec.alignOrigin ?? [0.5, 0.5];
    // Untransformed box of the target (one write, one read, restore).
    const st = target.style;
    const prev = st.transform;
    st.transform = 'none';
    const r = target.getBoundingClientRect();
    st.transform = prev;
    if (spec.rotate !== undefined && spec.rotate !== false) {
      this._restore.push(snapshot(target, 'transformOrigin', Kind.Style));
      st.transformOrigin = ox * 100 + '% ' + oy * 100 + '%';
    }
    const m = ctm ?? { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
    return transformPath(data, [m.a, m.b, m.c, m.d, m.e - (r.left + ox * r.width), m.f - (r.top + oy * r.height)]);
  }

  /**
   * All properties of a part were taken over: once no part writes anything,
   * free the animation. Counted (bindings only ever turn off), not rescanned:
   * a new animation taking over all n targets of this one used to scan the
   * parts from the start for each of them, O(n²).
   */
  _partEmpty(part: Part): void {
    if (part.gone) return;
    part.gone = true;
    // While building, a later target of the same call can take a property
    // from an earlier one (a target listed twice): `_build` checks at the end.
    if (--this._live <= 0 && !this._building) this.kill();
  }

  /**
   * Bake this animation into native CSS: `@keyframes` plus a rule using it.
   * Values are sampled from the engine (any easing, spring, colour space or
   * keyframe set works); transforms, styles and CSS variables are included.
   * One block per target (`name-0`, `name-1`, …) when there are several.
   */
  toCSS(options: {
    /** `@keyframes` name (default `'anikit'`; `-0`, `-1`… per target). */
    name?: string;
    /** Selector for the rule that runs it (default `.<name>`). */
    selector?: string;
    /** Keyframes sampled from the engine (default 30). */
    samples?: number;
  } = {}): string {
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
      const rp = this._rp;
      this._rp = null;
      if (rp !== null) for (let k = rp.length - 3; k >= 0; k -= 3) (rp[k] as any)[rp[k + 1] as string] = rp[k + 2];
    });
  }

  protected _kill(): void {
    const w = core();
    for (const p of this._parts) p.dispose();
    // (`unregister` matches the record itself: the group's own record, so a
    // killed animation is not kept alive by the record table.)
    if (this._grec) unregister(this._grec);
    this._grec = null;
    // Releasing the root frees the whole subtree (group + parts).
    if (this._h) w.ak_release(this._h);
    this._parts = [];
    this._h = 0;
  }

  protected _cleanup(): () => void {
    const parts = this._parts;
    const grec = this._grec;
    const root = this._h;
    return () => {
      const w = core();
      for (const p of parts) p.dispose();
      if (grec) unregister(grec);
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
    const e = t.easings ?? NO_EASINGS;
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
