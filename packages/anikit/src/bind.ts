/**
 * Bindings connect one output channel range to one target property.
 *
 * Performance note: every binding is an instance of the same class and is
 * applied by one function with a `switch` on its mode. This keeps the hot
 * loop monomorphic so the JIT can inline it — per-binding closures would make
 * every call site megamorphic (measured ~5x slower).
 */
import { F64 } from './wasm';
import { Target, Kind, TargetState, stateOf, markDirty, camel, transformSlots } from './props';
import { r3 } from './values';

export const M_PROP = 0;
export const M_TRANSFORM = 1;
export const M_TRANSFORM2 = 2;
export const M_STYLE_NUM = 3;
export const M_STYLE_FMT = 4;
export const M_VAR = 5;
export const M_ATTR = 6;
export const M_PROP_FMT = 7;
export const M_CUSTOM = 8;

type Fmt = (at: number) => string | number;

export class Binding {
  on = true;
  constructor(
    public target: any,
    public prop: string,
    public track: number,
    public off: number,
    public mode: number,
    /** Style declaration (style modes) or TargetState (transform modes). */
    public obj: any,
    public key: string,
    public unit: string,
    public slot: number,
    public slot2: number,
    public fmt: Fmt | null,
    public fn: ((at: number) => void) | null,
  ) {}
}

/** Write one binding's value(s) from F64[at]. */
export function apply(b: Binding, at: number): void {
  switch (b.mode) {
    case M_PROP:
      b.target[b.key] = F64[at];
      return;
    case M_TRANSFORM: {
      const s = b.obj as TargetState;
      s.t[b.slot] = F64[at];
      if (!s.dirty) markDirty(s);
      return;
    }
    case M_TRANSFORM2: {
      const s = b.obj as TargetState;
      s.t[b.slot] = s.t[b.slot2] = F64[at];
      if (!s.dirty) markDirty(s);
      return;
    }
    case M_STYLE_NUM:
      b.obj[b.key] = r3(F64[at]) + b.unit;
      return;
    case M_STYLE_FMT:
      b.obj[b.key] = b.fmt!(at);
      return;
    case M_VAR:
      (b.obj as CSSStyleDeclaration).setProperty(b.key, String(b.fmt!(at)));
      return;
    case M_ATTR:
      (b.target as Element).setAttribute(b.key, String(b.fmt!(at)));
      return;
    case M_PROP_FMT:
      b.target[b.key] = b.fmt!(at);
      return;
    default:
      b.fn!(at);
  }
}

/**
 * Create the binding for `(target, prop)`.
 * `numeric`: the track is a single plain number channel; `unit` its unit.
 */
export function bind(
  target: Target, prop: string, kind: Kind, track: number, numeric: boolean, unit: string, fmt: Fmt,
): Binding {
  switch (kind) {
    case Kind.Transform: {
      const s = stateOf(target);
      const [a, b] = transformSlots(prop);
      if (a <= 2) s.u[a] = unit || 'px';
      return new Binding(target, prop, track, 0, b >= 0 ? M_TRANSFORM2 : M_TRANSFORM, s, prop, unit, a, b, null, null);
    }
    case Kind.Style: {
      const style = (target as HTMLElement).style;
      const key = camel(prop);
      return numeric
        ? new Binding(target, prop, track, 0, M_STYLE_NUM, style, key, unit, 0, -1, null, null)
        : new Binding(target, prop, track, 0, M_STYLE_FMT, style, key, unit, 0, -1, fmt, null);
    }
    case Kind.Var:
      return new Binding(target, prop, track, 0, M_VAR, (target as HTMLElement).style, prop, unit, 0, -1, fmt, null);
    case Kind.Attr:
      return new Binding(target, prop, track, 0, M_ATTR, null, prop, unit, 0, -1, fmt, null);
    default:
      return numeric && !unit
        ? new Binding(target, prop, track, 0, M_PROP, null, prop, '', 0, -1, null, null)
        : new Binding(target, prop, track, 0, M_PROP_FMT, null, prop, unit, 0, -1, fmt, null);
  }
}

/** A binding running arbitrary code (motion paths, morphs, stroke drawing). */
export function custom(target: Target, prop: string, track: number, off: number, fn: (at: number) => void): Binding {
  return new Binding(target, prop, track, off, M_CUSTOM, null, prop, '', 0, -1, null, fn);
}

/** Bind a transform slot directly (used by motion paths). */
export function transformSlot(target: Target, prop: string, track: number, off: number, slot: number): Binding {
  return new Binding(target, prop, track, off, M_TRANSFORM, stateOf(target), prop, '', slot, -1, null, null);
}
