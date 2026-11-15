/**
 * Value parsing and track building.
 *
 * Every animatable value is reduced to numeric channels the core can
 * interpolate: numbers (+unit), colours (RGBA, interpolated in OKLab by
 * default), "complex" strings (any text with numbers/colours inside, e.g.
 * `box-shadow`, `filter`, `clip-path`, gradients) and discrete values.
 */
import { core, writeStr, F64, views } from './wasm';

export const TRACK_NUMBER = 0;
export const TRACK_COLOR = 1;
export const TRACK_DISCRETE = 2;
export const TRACK_PATH = 3;

export type ColorSpaceName = 'srgb' | 'linear' | 'oklab' | 'oklch' | 'hsl';
export const COLOR_SPACES: Record<ColorSpaceName, number> = { srgb: 0, linear: 1, oklab: 2, oklch: 3, hsl: 4 };

/** Round for output: 3 decimals keeps strings short and sub-pixel accurate. */
export const r3 = (v: number): number => Math.round(v * 1000) / 1000;

// ---------------------------------------------------------------------------
// Parsed values
// ---------------------------------------------------------------------------

export type Rel = '' | '+=' | '-=' | '*=';
export interface PNum {
  k: 'num';
  n: number;
  unit: string;
  rel: Rel;
}
export interface PColor {
  k: 'color';
  c: [number, number, number, number];
}
export interface PComplex {
  k: 'complex';
  /** Static text pieces; numbers/colours go between them. */
  parts: string[];
  /** Slot kinds: 0 number, 1 colour (4 channels). */
  slots: number[];
  nums: number[];
}
export interface PStr {
  k: 'str';
  s: string;
}
export type Parsed = PNum | PColor | PComplex | PStr;

const NUM_RE = /^([+\-*]=)?\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)([a-z%]*)$/i;
const colorCache = new Map<string, [number, number, number, number] | null>();

/** Parse a CSS colour via the core (cached). */
export function parseColor(s: string): [number, number, number, number] | null {
  let c = colorCache.get(s);
  if (c !== undefined) return c;
  const w = core();
  if (w.ak_color_parse(writeStr(s))) {
    views();
    const at = w.ak_f64_buf(0) >>> 3;
    c = [F64[at], F64[at + 1], F64[at + 2], F64[at + 3]];
  } else c = null;
  if (colorCache.size > 2000) colorCache.clear();
  colorCache.set(s, c);
  return c;
}

const COLOR_TOKEN =
  /#[0-9a-f]{3,8}\b|(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\([^()]*\)|\b(?:transparent|[a-z]{3,20})\b|[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/gi;

export function parseValue(v: unknown): Parsed {
  if (typeof v === 'number') return { k: 'num', n: v, unit: '', rel: '' };
  const s = String(v).trim();
  const m = NUM_RE.exec(s);
  if (m) return { k: 'num', n: parseFloat(m[2]), unit: m[3], rel: (m[1] as Rel) || '' };
  const c = parseColor(s);
  if (c) return { k: 'color', c };
  // Complex: scan for numbers and colours.
  const parts: string[] = [];
  const slots: number[] = [];
  const nums: number[] = [];
  let lastIndex = 0;
  COLOR_TOKEN.lastIndex = 0;
  let mt: RegExpExecArray | null;
  while ((mt = COLOR_TOKEN.exec(s))) {
    const tok = mt[0];
    const ch = tok.charCodeAt(0);
    const isNum = (ch >= 48 && ch <= 57) || ch === 45 || ch === 43 || ch === 46;
    if (isNum) {
      // Avoid splitting identifiers like "translate3d" or "h1".
      const prev = mt.index > 0 ? s.charCodeAt(mt.index - 1) : 32;
      const isIdent = (prev >= 65 && prev <= 90) || (prev >= 97 && prev <= 122) || prev === 95;
      if (isIdent) continue;
      parts.push(s.slice(lastIndex, mt.index));
      slots.push(0);
      nums.push(parseFloat(tok));
      lastIndex = mt.index + tok.length;
    } else {
      const col = parseColor(tok);
      if (!col) continue;
      parts.push(s.slice(lastIndex, mt.index));
      slots.push(1);
      nums.push(col[0], col[1], col[2], col[3]);
      lastIndex = mt.index + tok.length;
    }
  }
  if (slots.length === 0) return { k: 'str', s };
  parts.push(s.slice(lastIndex));
  return { k: 'complex', parts, slots, nums };
}

function sameTemplate(a: PComplex, b: PComplex): boolean {
  if (a.slots.length !== b.slots.length) return false;
  for (let i = 0; i < a.slots.length; i++) if (a.slots[i] !== b.slots[i]) return false;
  for (let i = 0; i < a.parts.length; i++) {
    if (a.parts[i].replace(/\s+/g, ' ').trim() !== b.parts[i].replace(/\s+/g, ' ').trim()) return false;
  }
  return true;
}

/** A neutral version of a template ("none" → zeros, transparent colours). */
function zeroLike(t: PComplex): PComplex {
  const nums = t.nums.slice();
  let at = 0;
  for (const s of t.slots) {
    if (s === 0) nums[at++] = 0;
    else {
      nums[at + 3] = 0;
      at += 4;
    }
  }
  return { k: 'complex', parts: t.parts, slots: t.slots, nums };
}

function fmtColor(o: number[] | Float64Array, i: number): string {
  const a = o[i + 3];
  const r = Math.round(o[i]);
  const g = Math.round(o[i + 1]);
  const b = Math.round(o[i + 2]);
  return a >= 1 ? 'rgb(' + r + ',' + g + ',' + b + ')' : 'rgba(' + r + ',' + g + ',' + b + ',' + r3(a) + ')';
}

// ---------------------------------------------------------------------------
// Tracks
// ---------------------------------------------------------------------------

export interface TrackDef {
  kind: number;
  /** Colour space (colour tracks) or LUT id (path tracks). */
  arg: number;
  /** Channels per keyframe. */
  ch: number;
  /** Output channels. */
  out: number;
  values: number[];
  offsets?: number[];
  easings?: number[];
  /** Format output values at F64 index `at` (for string-valued targets). */
  format: (at: number) => string | number;
  /** Numeric unit when kind is number. */
  unit: string;
  /** Final value as written (for chaining and `complete()`). */
  last: string | number;
  first: string | number;
}

export type Converter = (value: number, from: string, to: string) => number;

/** Units that need no conversion between each other. */
const sameUnit = (a: string, b: string) => a === b || a === '' || b === '';

/**
 * Build a track from raw keyframe values. `current` is the live value used
 * for a missing/`null` first keyframe and as the base of relative values.
 */
export function buildTrack(
  raw: unknown[],
  current: () => unknown,
  convert: Converter,
  colorSpace: number,
  defaultUnit = '',
): TrackDef {
  const vals = raw.slice();
  if (vals.length === 1) vals.unshift(null);
  let cur: Parsed | null = null;
  const parsed: Parsed[] = vals.map((v, i) => {
    if (v === null || v === undefined) {
      if (cur === null) cur = parseValue(current());
      return cur;
    }
    const p = parseValue(v);
    if (i === 0 && p.k === 'num' && p.rel) {
      if (cur === null) cur = parseValue(current());
    }
    return p;
  });

  // Resolve relative values against the previous keyframe (or current value).
  for (let i = 0; i < parsed.length; i++) {
    const p = parsed[i];
    if (p.k === 'num' && p.rel) {
      const base = i > 0 ? parsed[i - 1] : (cur ?? parseValue(current()));
      const bn = base.k === 'num' ? base.n : 0;
      const n = p.rel === '+=' ? bn + p.n : p.rel === '-=' ? bn - p.n : bn * p.n;
      parsed[i] = { k: 'num', n, unit: p.unit || (base.k === 'num' ? base.unit : ''), rel: '' };
    }
  }

  const all = (k: Parsed['k']) => parsed.every((p) => p.k === k);
  const n = parsed.length;

  if (all('num')) {
    const nums = parsed as PNum[];
    // Target unit: the last explicit unit (or the default for unitless numbers).
    let unit = '';
    for (let i = n - 1; i >= 0; i--) if (nums[i].unit) {
      unit = nums[i].unit;
      break;
    }
    if (!unit) unit = defaultUnit;
    const values = nums.map((p) => (sameUnit(p.unit, unit) ? p.n : convert(p.n, p.unit, unit)));
    const fmt = unit ? (at: number) => r3(F64[at]) + unit : (at: number) => r3(F64[at]);
    return {
      kind: TRACK_NUMBER, arg: 0, ch: 1, out: 1, values, format: fmt, unit,
      first: unit ? values[0] + unit : values[0], last: unit ? values[n - 1] + unit : values[n - 1],
    };
  }

  if (parsed.every((p) => p.k === 'color')) {
    const values: number[] = [];
    for (const p of parsed as PColor[]) values.push(p.c[0], p.c[1], p.c[2], p.c[3]);
    return {
      kind: TRACK_COLOR, arg: colorSpace, ch: 4, out: 4, values, unit: '',
      format: (at) => fmtColor(F64, at),
      first: fmtColor(values, 0), last: fmtColor(values, (n - 1) * 4),
    };
  }

  // Complex strings (possibly mixed with "none"/numbers/colours that can be
  // lifted into the template).
  const tmpl = parsed.find((p) => p.k === 'complex') as PComplex | undefined;
  if (tmpl) {
    const lifted: (PComplex | null)[] = parsed.map((p) => {
      if (p.k === 'complex') return sameTemplate(p, tmpl) ? p : null;
      if (p.k === 'str' && (p.s === 'none' || p.s === '' || p.s === 'auto' || p.s === 'normal')) return zeroLike(tmpl);
      if (p.k === 'num' && tmpl.slots.length === 1 && tmpl.slots[0] === 0) {
        return { k: 'complex', parts: tmpl.parts, slots: tmpl.slots, nums: [p.n] };
      }
      if (p.k === 'color' && tmpl.slots.length === 1 && tmpl.slots[0] === 1) {
        return { k: 'complex', parts: tmpl.parts, slots: tmpl.slots, nums: p.c.slice() };
      }
      return null;
    });
    if (lifted.every((x) => x !== null)) {
      const ch = tmpl.nums.length;
      const values: number[] = [];
      for (const p of lifted as PComplex[]) values.push(...p.nums);
      const { parts, slots } = tmpl;
      const build = (src: number[] | Float64Array, at: number): string => {
        let s = parts[0];
        let i = at;
        for (let k = 0; k < slots.length; k++) {
          if (slots[k] === 0) s += r3(src[i++]);
          else {
            s += fmtColor(src, i);
            i += 4;
          }
          s += parts[k + 1];
        }
        return s;
      };
      return {
        kind: TRACK_NUMBER, arg: 0, ch, out: ch, values, unit: '',
        format: (at) => build(F64, at),
        first: build(values, 0), last: build(values, (n - 1) * ch),
      };
    }
  }

  // Discrete fallback: switch between raw strings.
  const strs = parsed.map((p) =>
    p.k === 'str' ? p.s : p.k === 'num' ? p.n + p.unit : p.k === 'color' ? fmtColor(p.c, 0) : buildFromComplex(p),
  );
  return {
    kind: TRACK_DISCRETE, arg: 0, ch: 1, out: 1, values: strs.map((_, i) => i), unit: '',
    format: (at) => strs[F64[at] | 0],
    first: strs[0], last: strs[n - 1],
  };
}

function buildFromComplex(p: PComplex): string {
  let s = p.parts[0];
  let i = 0;
  for (let k = 0; k < p.slots.length; k++) {
    if (p.slots[k] === 0) s += p.nums[i++];
    else {
      s += fmtColor(p.nums, i);
      i += 4;
    }
    s += p.parts[k + 1];
  }
  return s;
}
