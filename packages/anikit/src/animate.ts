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
import { core, F64, LINEAR, f64Buf, whenReady } from './wasm';
import {
  register, unregister, wake, flush, easingId, easingDuration, NodeRecord, EasingInput,
  FL_FILL_BACKWARDS, FL_FILL_FORWARDS, FL_IMMEDIATE,
} from './core';
import {
  buildTrack, TrackDef, TRACK_NUMBER, TRACK_PATH, COLOR_SPACES, ColorSpaceName, r3,
} from './values';
import {
  Target, Targets, toTargets, classify, Kind, readValue, writer, convertUnit, defaultUnit, isElement,
  claim, unclaim, Owner, stateOf, markDirty, transformUnit, TargetState,
} from './props';
import { Playable, Callbacks, Linked } from './playable';
import { Binding, bind, apply, custom, transformSlot, M_PROP, M_TRANSFORM } from './bind';
import { pathData, PathSource, morphPlan, writeMorph, elementLength } from './svg';
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
}

const OPTION_KEYS = new Set([
  'duration', 'delay', 'endDelay', 'ease', 'repeat', 'repeatDelay', 'yoyo', 'direction', 'speed', 'autoplay',
  'times', 'colorSpace', 'fillMode', 'overwrite', 'immediateRender', 'onStart', 'onUpdate', 'onComplete',
  'onRepeat', 'onReverseComplete',
]);

const DIRECTIONS = { normal: 0, reverse: 1, alternate: 2, 'alternate-reverse': 3 } as const;

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
        s.t[x.slot] = f[at + x.off];
        if (!s.dirty) markDirty(s);
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

function resolve<T>(v: FnValue<T>, t: unknown, i: number, n: number): T {
  return typeof v === 'function' ? (v as any)(t, i, n) : v;
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
    if (key === 'path' || key === 'draw' || key === 'd' || key === 'attr') {
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
  /** Seconds; available once built. */
  _end = 0;

  constructor(targets: Targets, keyframes: Props | Keyframe[], options: AnimateOptions = {}, ctx: BuildContext = {}) {
    const [props, opts] = normalize(keyframes, options);
    super(opts);
    whenReady(() => this._build(targets, props, opts, ctx));
  }

  private _build(targetsIn: Targets, props: Map<string, PropSpec>, o: AnimateOptions, ctx: BuildContext): void {
    const w = core();
    const targets = (this.targets = toTargets(targetsIn));
    const n = targets.length;
    const claims = !ctx.child && o.overwrite !== false;
    const cs = COLOR_SPACES[o.colorSpace ?? 'oklab'] ?? 2;
    const repeat = o.repeat === undefined ? 0 : o.repeat < 0 ? Infinity : o.repeat;
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
        const raw = spec.values.map((v) => resolve(v, target, i, n));
        if (raw[0] !== null && raw[0] !== undefined && raw.length > 1) hasExplicitFrom = true;
        const chained = ctx.chain?.get(target)?.get(prop);
        const read = () => (chained !== undefined ? chained : readValue(target, prop, kind));
        const el = isElement(target) ? target : null;
        let du = defaultUnit(prop, kind);
        const conv = (v: number, f: string, t: string) => convertUnit(el, prop, v, f, t);
        let track = buildTrack(raw, read, conv, cs, du);
        if (kind === Kind.Transform) {
          if (track.kind !== TRACK_NUMBER || track.ch !== 1) continue;
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
        part.b.push(bind(target, prop, kind, idx, numeric, track.unit, track.format));
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
      if (ctx.set) dur = 0;
      else if (o.duration !== undefined) dur = resolve(o.duration, target, i, n);
      else if (isSpringEase(easeForDur)) {
        const d = easingDuration(easingId(easeForDur as EasingInput));
        dur = d > 0 ? d : 0.5;
      } else dur = 0.5;
      const delay = ctx.set ? 0 : resolve(o.delay ?? 0, target, i, n);
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
    if (prop === 'attr' && isPlainObject(raw)) {
      for (const key of Object.keys(raw)) {
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
      const plan = morphPlan(from as PathSource, to as PathSource);
      const tmpl = plan.template;
      const t: TrackDef = {
        kind: TRACK_NUMBER, arg: 0, ch: plan.from.length, out: plan.from.length,
        values: plan.from.concat(plan.to), unit: '',
        format: (at) => writeMorph(tmpl, F64, at),
        first: writeMorph(tmpl, plan.from), last: writeMorph(tmpl, plan.to),
      };
      const idx = addTrack(t, undefined);
      const write = isElement(el) ? (v: string) => el.setAttribute('d', v) : (v: string) => ((target as any).d = v);
      part.b.push(custom(target, 'd', idx, 0, (at) => write(writeMorph(tmpl, F64, at))));
      if (ctx.chain) chainSet(ctx.chain, target, 'd', t.last);
      return;
    }
    if (prop === 'draw') {
      // draw: visible segment of a stroke as "start end" fractions (or a single end fraction).
      const el = target as SVGElement;
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
