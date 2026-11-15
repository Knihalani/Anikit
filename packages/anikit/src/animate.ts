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
  register, unregister, wake, flush, easingId, easingDuration, NodeRecord, EasingInput, prefersReduced, reducedMotionMode,
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
import { warn, didYouMean, describe, cssPropertyNames } from './debug';
import { Binding, bind, apply, custom, transformSlot, M_PROP, M_TRANSFORM, M_TRANSFORM2, M_STYLE_NUM, M_STYLE_FMT, M_VAR, M_CUSTOM } from './bind';
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
  | FnValue<Scalar | null>
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

export type Props = Record<string, PropValue>;
export type Keyframe = Record<string, Scalar | null | undefined> & { offset?: number; ease?: EasingInput };

export interface AnimateOptions extends Callbacks<Animation> {
  /** Seconds (default 0.5; spring easings default to their natural duration). */
  duration?: FnValue<number>;
  /** Seconds before starting (accepts `stagger()`). */
  delay?: FnValue<number>;
  /** Seconds to hold after finishing (before `finished` resolves). */
  endDelay?: number;
  /** Easing for each keyframe segment, or one per segment. */
  ease?: EasingInput | EasingInput[];
  /** Extra iterations (`Infinity` or `-1` loops forever). */
  repeat?: number;
  /** Pause between iterations (seconds). */
  repeatDelay?: number;
  /** Alternate direction every iteration. */
  yoyo?: boolean;
  direction?: 'normal' | 'reverse' | 'alternate' | 'alternate-reverse';
  /** Playback rate (default 1). */
  speed?: number;
  /** Start immediately (default true). */
  autoplay?: boolean;
  /** Keyframe offsets (0–1) for array values. */
  times?: number[];
  /** Colour interpolation space (default 'oklab'). */
  colorSpace?: ColorSpaceName;
  /** Which values to keep outside the active period (default 'both'). */
  fillMode?: 'both' | 'forwards' | 'backwards' | 'none';
  /** Take properties over from running animations (default true). */
  overwrite?: boolean;
  /** Render the first keyframe immediately even when delayed. */
  immediateRender?: boolean;
  /** Snap numeric values to a step (or list of values); a number applies to all properties. */
  snap?: number | Record<string, number | number[]>;
  /** Round to integers: `true` for all properties or a list of property names. */
  round?: boolean | string[];
  /** Transform each value right before it is applied, e.g. `{ x: (v) => v % 500 }`. */
  modifiers?: Record<string, (value: any, target: any) => any>;
  /** Format numbers written as text (counters): Intl options or a function. */
  format?: Intl.NumberFormatOptions | ((value: number) => string);
  /** Locale used by `format` (default: the user's). */
  locale?: string;
}

const OPTION_KEYS = new Set([
  'duration', 'delay', 'endDelay', 'ease', 'repeat', 'repeatDelay', 'yoyo', 'direction', 'speed', 'autoplay',
  'times', 'colorSpace', 'fillMode', 'overwrite', 'immediateRender', 'onStart', 'onUpdate', 'onComplete',
  'onRepeat', 'onReverseComplete', 'snap', 'round', 'modifiers', 'format', 'locale',
]);

const DIRECTIONS = { normal: 0, reverse: 1, alternate: 2, 'alternate-reverse': 3 } as const;
/** Options accepted for compatibility / by wrappers (not warned about). */
const EXTRA_OPTION_KEYS = new Set(['stagger', 'distance', 'from', 'intensity', 'loop', 'axis', 'at', 'charDelay', 'start', 'to', 'chars', 'delimiter', 'text', 'size', 'color']);
const TRANSFORM_PROPS = ['x', 'y', 'z', 'rotate', 'rotateX', 'rotateY', 'rotateZ', 'scale', 'scaleX', 'scaleY', 'scaleZ', 'skew', 'skewX', 'skewY', 'perspective', 'xPercent', 'yPercent'];
const SPECIAL_PROPS = ['path', 'draw', 'd', 'attr', 'text', 'transform'];
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
    const [props, opts] = normalize(keyframes, options);
    super(opts);
    for (const k of Object.keys(options)) {
      if (!OPTION_KEYS.has(k) && !EXTRA_OPTION_KEYS.has(k)) warn(`unknown animation option "${k}".${didYouMean(k, OPTION_KEYS)}`);
    }
    this._times = opts.times;
    whenReady(() => this._build(targets, props, opts, ctx));
  }

  private _build(targetsIn: Targets, props: Map<string, PropSpec>, o: AnimateOptions, ctx: BuildContext): void {
    const w = core();
    const targets = (this.targets = toTargets(targetsIn));
    const n = targets.length;
    if (n === 0 && typeof targetsIn === 'string') warn(`selector "${targetsIn}" matched no elements, so nothing was animated.`);
    else if (n === 0 && targetsIn != null) warn('animate() received an empty target list.');
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

    for (let i = 0; i < n; i++) {
      const target = targets[i];
      const part = new Part(this, claims);
      part.upd = !!o.onUpdate;
      const tracks: TrackDef[] = [];
      let hasExplicitFrom = false;
      // One transform snapshot per target covers every transform component.
      let snappedTransform = false;

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
          t.easings = id === LINEAR ? [] : [id];
        }
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

      for (const [prop, spec] of props) {
        if (spec.raw !== undefined) {
          this._special(prop, spec.raw, target, i, n, part, tracks, addTrack, cs, ctx);
          continue;
        }
        const kind = classify(target, prop);
        if (kind === Kind.Style && i === 0 && !isKnownStyle(target as HTMLElement, prop)) {
          warn(`"${prop}" is not a CSS property of ${describe(target)}; it will be written as an inline style anyway.${didYouMean(prop, [...cssPropertyNames(), ...TRANSFORM_PROPS, ...SPECIAL_PROPS])}`);
        }
        const raw = spec.values.map((v) => resolve(v, target, i, n));
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
            for (let k = 0; k < raw.length; k++) {
              if (raw[k] === 'auto' || (k === 0 && inlineAuto && (raw[k] === null || raw[k] === undefined))) raw[k] = px + 'px';
            }
          }
        }
        const chained = ctx.chain?.get(target)?.get(prop);
        const read = () => (chained !== undefined ? chained : readValue(target, prop, kind));
        const el = isElement(target) ? target : null;
        let du = defaultUnit(prop, kind);
        const conv = (v: number, f: string, t: string) => convertUnit(el, prop, v, f, t);
        let track = buildTrack(raw, read, conv, cs, du);
        if (kind === Kind.Transform) {
          if (track.kind !== TRACK_NUMBER || track.ch !== 1 || track.complex) continue;
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
        if (kind !== Kind.Transform || !snappedTransform) this._restore.push(snapshot(target, prop, kind));
        if (kind === Kind.Transform) snappedTransform = true;
        const post = postFor(prop, o);
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
            const s = Math.abs(v - ap) < 1e-3 ? kw : r3(v) + 'px';
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
        style.strokeDasharray = r3((e - s) * len) + 'px ' + r3(len + 1) + 'px';
        style.strokeDashoffset = r3(-s * len) + 'px';
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
