/**
 * High-frequency helpers for pointer-driven / per-frame updates.
 *
 * ```ts
 * const setX = quickSetter(el, 'x');           // immediate, no animation
 * const toX = quickTo(el, 'x', { duration: 0.4, ease: 'outCubic' });
 * addEventListener('pointermove', (e) => toX(e.clientX));
 * ```
 */
import { animate, AnimateOptions, Animation } from './animate';
import { classify, Kind, writer, stateOf, composeTransform, defaultUnit, Target, toTargets, Targets } from './props';
import { r3 } from './values';

/** A function that writes a property immediately (transforms composed correctly). */
export function quickSetter(targets: Targets, prop: string, unit?: string): (value: number | string) => void {
  const list = toTargets(targets);
  const setters = list.map((t: Target) => {
    const kind = classify(t, prop);
    const u = unit ?? defaultUnit(prop, kind);
    const w = writer(t, prop, kind, u);
    if (kind === Kind.Transform) {
      const s = stateOf(t);
      return (v: number | string) => {
        w(typeof v === 'number' ? v : parseFloat(v));
        s.dirty = false;
        (t as HTMLElement).style.transform = composeTransform(s);
      };
    }
    const numeric = kind === Kind.Prop && !u;
    return (v: number | string) => w(typeof v === 'number' && !numeric ? r3(v) + u : v);
  });
  return (v) => {
    for (const s of setters) s(v);
  };
}

/**
 * A function that re-targets a tween each call, continuing smoothly from the
 * current value (for following pointers, sliders, etc.). For momentum-
 * preserving following use `spring()` instead.
 */
export function quickTo(targets: Targets, prop: string, options: AnimateOptions = {}): ((value: number | string) => Animation) & { kill(): void } {
  let current: Animation | null = null;
  const fn = ((value: number | string) => {
    current = animate(targets, { [prop]: value }, { duration: 0.5, ease: 'outCubic', ...options });
    return current;
  }) as ((value: number | string) => Animation) & { kill(): void };
  fn.kill = () => current?.kill();
  return fn;
}
