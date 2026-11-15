/**
 * Property layer: classify what a property name means for a given target,
 * read its current value, convert units and write values back with minimal
 * DOM work. Transforms are composed per element and flushed once per frame.
 */
import { onFlush } from './core';
import { r3 } from './values';

export type Target = Element | Record<string, any>;

// ---------------------------------------------------------------------------
// Transforms
// ---------------------------------------------------------------------------

/** Transform component → [state slot, default unit]. */
const TRANSFORM: Record<string, [number, string]> = {
  x: [0, 'px'], translateX: [0, 'px'],
  y: [1, 'px'], translateY: [1, 'px'],
  z: [2, 'px'], translateZ: [2, 'px'],
  rotate: [3, 'deg'], rotateZ: [3, 'deg'], rotation: [3, 'deg'],
  rotateX: [4, 'deg'], rotateY: [5, 'deg'],
  skewX: [6, 'deg'], skewY: [7, 'deg'],
  scaleX: [8, ''], scaleY: [9, ''], scaleZ: [10, ''],
  perspective: [11, 'px'],
};
/** `scale` and `skew` set two slots at once. */
const DOUBLE: Record<string, [number, number, string]> = { scale: [8, 9, ''], skew: [6, 7, 'deg'] };

const IDENTITY = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0];

/** State slots written by a transform property (`-1` when only one). */
export function transformSlots(p: string): [number, number] {
  const t = TRANSFORM[p];
  if (t) return [t[0], -1];
  const d = DOUBLE[p];
  return d ? [d[0], d[1]] : [0, -1];
}

export function isTransform(p: string): boolean {
  return p in TRANSFORM || p in DOUBLE;
}
export function transformUnit(p: string): string {
  const t = TRANSFORM[p];
  return t ? t[1] : DOUBLE[p] ? DOUBLE[p][2] : 'px';
}

/** Per-target bookkeeping (transform state + property ownership). */
export class TargetState {
  /** Transform slots (see TRANSFORM). */
  t: number[] = IDENTITY.slice();
  /** Units of x / y / z (px, %, em, ...). */
  u: string[] = ['px', 'px', 'px'];
  dirty = false;
  owners = new Map<string, Owner>();
  constructor(public el: Target) {}
}

/** Something that can be told it lost a property to a newer animation. */
export interface Owner {
  release(prop: string): void;
}

const states = new WeakMap<object, TargetState>();
const dirty: TargetState[] = [];

export function stateOf(t: Target): TargetState {
  let s = states.get(t);
  if (!s) {
    s = new TargetState(t);
    if (isElement(t)) readTransform(t, s);
    states.set(t, s);
  }
  return s;
}

/** Claim `prop` on `target`, evicting the previous owner. */
export function claim(target: Target, prop: string, owner: Owner): void {
  const s = stateOf(target);
  const prev = s.owners.get(prop);
  if (prev && prev !== owner) prev.release(prop);
  s.owners.set(prop, owner);
}
export function unclaim(target: Target, prop: string, owner: Owner): void {
  const s = states.get(target);
  if (s && s.owners.get(prop) === owner) s.owners.delete(prop);
}

export function isElement(t: unknown): t is Element {
  return typeof Element !== 'undefined' && t instanceof Element;
}

/** Decompose an element's computed transform into the state (once per element). */
function readTransform(el: Element, s: TargetState): void {
  const style = (el as HTMLElement).style;
  const raw = style && style.transform ? style.transform : computed(el, 'transform');
  if (!raw || raw === 'none') return;
  const m = /matrix(3d)?\(([^)]+)\)/.exec(raw);
  if (!m) return;
  const v = m[2].split(',').map(parseFloat);
  let a: number, b: number, c: number, d: number, e: number, f: number, z = 0;
  if (m[1]) {
    [a, b, c, d, e, f, z] = [v[0], v[1], v[4], v[5], v[12], v[13], v[14]];
  } else {
    [a, b, c, d, e, f] = v;
  }
  // 2D decomposition into translate · rotate · skewX · scale (CSS "unmatrix").
  let sx = Math.hypot(a, b);
  if (sx) {
    a /= sx;
    b /= sx;
  }
  let k = a * c + b * d;
  c -= a * k;
  d -= b * k;
  const sy = Math.hypot(c, d);
  if (sy) k /= sy;
  if (a * (d / (sy || 1)) < b * (c / (sy || 1))) {
    a = -a;
    b = -b;
    k = -k;
    sx = -sx;
  }
  const deg = 180 / Math.PI;
  s.t[0] = e;
  s.t[1] = f;
  s.t[2] = z || 0;
  s.t[3] = r3(Math.atan2(b, a) * deg);
  s.t[6] = r3(Math.atan(k) * deg);
  s.t[8] = r3(sx);
  s.t[9] = r3(sy);
}

/** Compose the transform string, omitting identity parts. */
export function composeTransform(s: TargetState): string {
  const t = s.t;
  let out = '';
  if (t[11]) out += 'perspective(' + r3(t[11]) + 'px) ';
  if (t[0] || t[1] || t[2]) {
    out += t[2]
      ? 'translate3d(' + r3(t[0]) + s.u[0] + ',' + r3(t[1]) + s.u[1] + ',' + r3(t[2]) + s.u[2] + ') '
      : 'translate(' + r3(t[0]) + s.u[0] + ',' + r3(t[1]) + s.u[1] + ') ';
  }
  if (t[3]) out += 'rotate(' + r3(t[3]) + 'deg) ';
  if (t[4]) out += 'rotateX(' + r3(t[4]) + 'deg) ';
  if (t[5]) out += 'rotateY(' + r3(t[5]) + 'deg) ';
  if (t[6] || t[7]) out += 'skew(' + r3(t[6]) + 'deg,' + r3(t[7]) + 'deg) ';
  if (t[8] !== 1 || t[9] !== 1) out += t[10] !== 1
    ? 'scale3d(' + r3(t[8]) + ',' + r3(t[9]) + ',' + r3(t[10]) + ') '
    : 'scale(' + r3(t[8]) + ',' + r3(t[9]) + ') ';
  else if (t[10] !== 1) out += 'scaleZ(' + r3(t[10]) + ') ';
  return out === '' ? 'none' : out.slice(0, -1);
}

export function markDirty(s: TargetState): void {
  if (!s.dirty) {
    s.dirty = true;
    dirty.push(s);
  }
}

/** Write all pending transforms (one style write per element per frame). */
export function flushTransforms(): void {
  for (let i = 0; i < dirty.length; i++) {
    const s = dirty[i];
    s.dirty = false;
    const el = s.el as HTMLElement;
    if (el.style) el.style.transform = composeTransform(s);
  }
  dirty.length = 0;
}
onFlush(flushTransforms);

// ---------------------------------------------------------------------------
// Reading values & units
// ---------------------------------------------------------------------------

const camelRe = /-([a-z])/g;
const kebabRe = /[A-Z]/g;
export const camel = (s: string) => s.replace(camelRe, (_, c) => c.toUpperCase());
export const kebab = (s: string) => (s.startsWith('--') ? s : s.replace(kebabRe, (c) => '-' + c.toLowerCase()));

export function computed(el: Element, prop: string): string {
  if (typeof getComputedStyle !== 'function') return '';
  return getComputedStyle(el).getPropertyValue(kebab(prop)).trim();
}

/** How a property is applied to a target. */
export const enum Kind {
  Transform,
  Style,
  Var,
  Attr,
  Prop,
}

export function classify(target: Target, prop: string): Kind {
  if (!isElement(target)) return Kind.Prop;
  if (isTransform(prop)) return Kind.Transform;
  if (prop.startsWith('--')) return Kind.Var;
  const style = (target as HTMLElement).style;
  if (style && (prop in style || camel(prop) in style)) return Kind.Style;
  if (target.hasAttribute(prop) || typeof SVGElement !== 'undefined' && target instanceof SVGElement) return Kind.Attr;
  if (prop in target) return Kind.Prop;
  return Kind.Style;
}

/** Current value of a property (for implicit `from` values). */
export function readValue(target: Target, prop: string, kind: Kind): unknown {
  switch (kind) {
    case Kind.Transform: {
      const s = stateOf(target);
      const d = DOUBLE[prop];
      if (d) return s.t[d[0]] + d[2];
      const [slot, unit] = TRANSFORM[prop];
      return s.t[slot] + (slot <= 2 ? s.u[slot] : unit);
    }
    case Kind.Style: {
      const el = target as HTMLElement;
      const inline = el.style ? (el.style as any)[camel(prop)] : '';
      const v = inline || computed(el, prop);
      return v === '' || v === 'auto' || v === 'normal' ? (isColorProp(prop) ? 'rgba(0,0,0,0)' : 0) : v;
    }
    case Kind.Var: {
      const v = computed(target as Element, prop) || (target as HTMLElement).style?.getPropertyValue(prop);
      return v === '' ? 0 : v;
    }
    case Kind.Attr: {
      const v = (target as Element).getAttribute(prop);
      return v === null ? 0 : v;
    }
    default: {
      const v = (target as any)[prop];
      return v === undefined || v === null ? 0 : v;
    }
  }
}

function isColorProp(p: string): boolean {
  return /color|fill|stroke|background$/i.test(p);
}

const ANGLE: Record<string, number> = { deg: 1, rad: 180 / Math.PI, turn: 360, grad: 0.9 };

/** Size in px of one `unit` for `prop` on `el`. */
function unitPx(el: Element | null, prop: string, unit: string): number {
  switch (unit) {
    case 'px':
    case '':
      return 1;
    case 'in':
      return 96;
    case 'cm':
      return 96 / 2.54;
    case 'mm':
      return 96 / 25.4;
    case 'pt':
      return 4 / 3;
    case 'pc':
      return 16;
    case 'vw':
      return (typeof innerWidth === 'number' ? innerWidth : 1000) / 100;
    case 'vh':
      return (typeof innerHeight === 'number' ? innerHeight : 1000) / 100;
    case 'vmin':
      return Math.min(unitPx(el, prop, 'vw'), unitPx(el, prop, 'vh'));
    case 'vmax':
      return Math.max(unitPx(el, prop, 'vw'), unitPx(el, prop, 'vh'));
    case 'rem':
      return parseFloat(typeof document !== 'undefined' ? computed(document.documentElement, 'font-size') : '') || 16;
    case 'em':
      return (el && parseFloat(computed(el, 'font-size'))) || 16;
    case '%': {
      if (!el) return 1;
      const h = el as HTMLElement;
      if (prop === 'x' || prop === 'translateX') return (h.offsetWidth || 0) / 100;
      if (prop === 'y' || prop === 'translateY') return (h.offsetHeight || 0) / 100;
      const parent = h.parentElement;
      if (!parent) return 1;
      if (/height|top|bottom/i.test(prop)) return parent.clientHeight / 100;
      if (/font/i.test(prop)) return (parseFloat(computed(parent, 'font-size')) || 16) / 100;
      return parent.clientWidth / 100;
    }
    default:
      return 1;
  }
}

export function convertUnit(el: Element | null, prop: string, v: number, from: string, to: string): number {
  if (from === to || v === 0) return v;
  if (from in ANGLE || to in ANGLE) return (v * (ANGLE[from] ?? 1)) / (ANGLE[to] ?? 1);
  if ((from === 's' && to === 'ms') || (from === 'ms' && to === 's')) return from === 's' ? v * 1000 : v / 1000;
  const px = v * unitPx(el, prop, from);
  const f = unitPx(el, prop, to);
  return f ? px / f : px;
}

/** Default unit for unitless numbers written to a CSS property. */
const UNITLESS = new Set([
  'opacity', 'zIndex', 'fontWeight', 'lineHeight', 'flexGrow', 'flexShrink', 'order', 'zoom', 'scale',
  'fillOpacity', 'strokeOpacity', 'stopOpacity', 'floodOpacity', 'strokeMiterlimit', 'columnCount',
  'aspectRatio', 'orphans', 'widows', 'animationIterationCount', 'tabSize', 'gridRow', 'gridColumn',
]);
export function defaultUnit(prop: string, kind: Kind): string {
  if (kind === Kind.Transform) return transformUnit(prop);
  if (kind === Kind.Style && !UNITLESS.has(camel(prop))) return 'px';
  return '';
}

// ---------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------

export type Writer = (v: string | number) => void;

/** Build the fastest writer for a (target, property) pair. */
export function writer(target: Target, prop: string, kind: Kind, unit: string): Writer {
  switch (kind) {
    case Kind.Transform: {
      const s = stateOf(target);
      const d = DOUBLE[prop];
      if (d) {
        const [a, b] = d;
        return (v) => {
          s.t[a] = s.t[b] = v as number;
          markDirty(s);
        };
      }
      const slot = TRANSFORM[prop][0];
      if (slot <= 2) s.u[slot] = unit || 'px';
      return (v) => {
        s.t[slot] = v as number;
        markDirty(s);
      };
    }
    case Kind.Style: {
      const style = (target as HTMLElement).style as any;
      const key = camel(prop);
      return (v) => {
        style[key] = v;
      };
    }
    case Kind.Var: {
      const style = (target as HTMLElement).style;
      return (v) => style.setProperty(prop, String(v));
    }
    case Kind.Attr:
      return (v) => (target as Element).setAttribute(prop, String(v));
    default:
      return (v) => {
        (target as any)[prop] = v;
      };
  }
}

// ---------------------------------------------------------------------------
// Targets
// ---------------------------------------------------------------------------

export type Targets = string | Target | ArrayLike<Target> | Iterable<Target> | null | undefined;

/** Normalise a selector / element / list / object into an array of targets. */
export function toTargets(t: Targets): Target[] {
  if (t === null || t === undefined) return [];
  if (typeof t === 'string') {
    if (typeof document === 'undefined') return [];
    return Array.from(document.querySelectorAll(t));
  }
  if (isElement(t)) return [t];
  if (Array.isArray(t)) {
    const out: Target[] = [];
    for (const x of t) out.push(...toTargets(x as Targets));
    return out;
  }
  if (typeof (t as any)[Symbol.iterator] === 'function' && !(typeof (t as any).length === 'number' && !(t as any).item)) {
    return Array.from(t as Iterable<Target>);
  }
  if (typeof (t as any).length === 'number' && typeof (t as any).item === 'function') {
    return Array.from(t as ArrayLike<Target>);
  }
  return [t as Target];
}
