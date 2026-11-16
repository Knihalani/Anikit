/**
 * `anikit/react`: hooks and a `<Presence>` component. Everything a hook
 * creates is collected in a `context()` and reverted on unmount, so there is
 * no cleanup to write.
 *
 * ```tsx
 * const scope = useAnikit(() => { animate('.box', { x: 100 }); }); // '.box' inside <div ref={scope}>
 * const [ref, animate] = useAnimate();      // animate(ref.current, …) on events, reverted on unmount
 * useRecipe('hero-reveal', heroRef);
 * const x = useSpring(target);              // MotionValue that springs to `target`
 * const { scrollYProgress } = useScroll();
 * <Presence exit={{ opacity: 0, y: -8 }}>{open && <Toast key="t" />}</Presence>
 * ```
 */
import {
  useRef, useEffect, useLayoutEffect, useState, useMemo, createElement, Children, isValidElement, cloneElement,
  type RefObject, type ReactNode, type ReactElement, type DependencyList,
} from 'react';
import { animate as baseAnimate, AnimateOptions, Props, Animation } from './animate';
import { Context, context } from './context';
import { motionValue, MotionValue, scrollProgress } from './motionvalue';
import type { PhysicsOptions } from './spring';
import type { ScrollOptions } from './scroll';
import type { Targets } from './props';
import { recipe, RecipeName, RecipeOptions } from './recipe';

const useIsoLayoutEffect = typeof window !== 'undefined' ? useLayoutEffect : useEffect;

/**
 * Run `setup` inside a `context()` scoped to the returned ref's element after
 * mount (selectors only match inside it); everything it creates is reverted on
 * unmount or when `deps` change. `setup` may return a cleanup function.
 */
export function useAnikit<E extends Element = HTMLDivElement>(setup: (ctx: Context) => void | (() => void), deps: DependencyList = []): RefObject<E | null> {
  const scope = useRef<E | null>(null);
  useIsoLayoutEffect(() => {
    const ctx = context(setup, scope.current);
    return () => ctx.revert();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return scope;
}

/** The scoped `animate` returned by `useAnimate()`: tracked, and reverted on unmount. */
export type ScopedAnimate = (targets: Targets, keyframes: Props, options?: AnimateOptions) => Animation;

/**
 * Motion-style `useAnimate`: `[scope, animate]`. Call `animate` from events or
 * effects; selectors resolve inside `scope.current`, and every animation it
 * starts is reverted on unmount.
 */
export function useAnimate<E extends Element = HTMLDivElement>(): [RefObject<E | null>, ScopedAnimate] {
  const scope = useRef<E | null>(null);
  const ctx = useMemo(() => new Context(), []);
  useEffect(() => () => ctx.revert(), [ctx]);
  const run = useMemo<ScopedAnimate>(() => (targets, keyframes, options) => {
    ctx.scope = scope.current;
    return ctx.add(() => baseAnimate(targets, keyframes, options));
  }, [ctx]);
  return [scope, run];
}

/** Apply `recipe(name, ref.current, options)` after mount; reverted on unmount. */
export function useRecipe(name: RecipeName, ref: RefObject<Element | null>, options?: RecipeOptions, deps: DependencyList = []): void {
  useIsoLayoutEffect(() => {
    if (!ref.current) return;
    const h = recipe(name, ref.current, options);
    return () => h.revert();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [name, ...deps]);
}

/** A `MotionValue` that lives as long as the component. */
export function useMotionValue(initial = 0): MotionValue {
  const v = useMemo(() => motionValue(initial), []); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => v.destroy(), [v]);
  return v;
}

/**
 * A `MotionValue` that springs to `target`: a number (re-rendered props) or
 * another `MotionValue` to follow.
 */
export function useSpring(target: number | MotionValue, options: PhysicsOptions = {}): MotionValue {
  const start = typeof target === 'number' ? target : target.get();
  const v = useMotionValue(start);
  const opts = useRef(options);
  opts.current = options;
  useEffect(() => {
    if (typeof target === 'number') {
      v.spring(target, opts.current);
      return;
    }
    return target.on((n) => v.spring(n, opts.current));
  }, [target, v]);
  return v;
}

/** `value` mapped through `interpolate()` stops: `useTransform(x, [0, 1], [0, 300])`. */
export function useTransform(value: MotionValue, input: number[], output: number[]): MotionValue {
  const v = useMemo(() => value.map(input, output), [value, input.join(), output.join()]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => v.destroy(), [v]);
  return v;
}

/**
 * Scroll progress (0 → 1) of the page, or of `target` through the viewport
 * (same options as `scroll()`; `target`/`container` may be refs).
 */
export function useScroll(options: Omit<ScrollOptions, 'target' | 'container'> & {
  /** Element (or ref) whose passage through the viewport is measured. */
  target?: RefObject<Element | null> | Element;
  /** Scroll container (or ref); default the page. */
  container?: RefObject<Element | null> | Element;
} = {}): { scrollYProgress: MotionValue; scrollXProgress: MotionValue } {
  const y = useMotionValue(0);
  const x = useMotionValue(0);
  const deref = (r: RefObject<Element | null> | Element | undefined) => (r && 'current' in r ? r.current ?? undefined : r);
  useEffect(() => {
    const base = { ...options, target: deref(options.target), container: deref(options.container) };
    const py = scrollProgress({ ...base, axis: 'y' }), px = scrollProgress({ ...base, axis: 'x' });
    const offs = [py.on((n) => y.set(n)), px.on((n) => x.set(n))];
    return () => { offs.forEach((f) => f()); py.destroy(); px.destroy(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return { scrollYProgress: y, scrollXProgress: x };
}

/** Props of `<Presence>`. */
export interface PresenceProps {
  /** Keyed children; a child whose key disappears animates out before it's removed. */
  children?: ReactNode;
  /** Keyframes for entering children (default `{ opacity: [0, 1] }`). */
  enter?: Props;
  /** Keyframes for leaving children (default `{ opacity: 0 }`). */
  exit?: Props;
  /** Animation options for both (duration, ease…). */
  options?: AnimateOptions;
  /** Animate the children present on first render too (default false). */
  initial?: boolean;
}

type Item = { key: string; el: ReactElement; leaving: boolean };

/**
 * AnimatePresence-style wrapper: keyed children animate in when added and out
 * when removed (kept in the tree until their exit finishes). Each child is
 * wrapped in a `display: contents` element that carries the animation target lookup.
 */
export function Presence({ children, enter = { opacity: [0, 1] }, exit = { opacity: 0 }, options = {}, initial = false }: PresenceProps): ReactElement {
  const current = Children.toArray(children).filter(isValidElement) as ReactElement[];
  const [, force] = useState(0);
  const items = useRef<Item[] | null>(null);
  const first = useRef(true);
  const nodes = useRef(new Map<string, HTMLElement>());
  const anims = useRef(new Map<string, Animation>());
  const keyOf = (c: ReactElement) => String(c.key);

  // Merge: keep leaving items in place, add new ones, mark vanished ones as leaving.
  const prev = items.current ?? [];
  const now = new Set(current.map(keyOf));
  const merged: Item[] = [];
  let ci = 0;
  for (const it of prev) {
    if (now.has(it.key)) {
      while (ci < current.length && keyOf(current[ci]) !== it.key) {
        if (!prev.some((p) => p.key === keyOf(current[ci]))) merged.push({ key: keyOf(current[ci]), el: current[ci], leaving: false });
        ci++;
      }
      merged.push({ key: it.key, el: current[ci] ?? it.el, leaving: false });
      ci++;
    } else merged.push({ ...it, leaving: true });
  }
  for (; ci < current.length; ci++) if (!merged.some((m) => m.key === keyOf(current[ci]))) merged.push({ key: keyOf(current[ci]), el: current[ci], leaving: false });
  items.current = merged;

  useIsoLayoutEffect(() => {
    const animateIn = !first.current || initial;
    first.current = false;
    for (const it of merged) {
      const node = nodes.current.get(it.key)?.firstElementChild;
      if (!node) continue;
      const running = anims.current.get(it.key);
      if (it.leaving && !(running as any)?._leaving) {
        running?.cancel();
        const a = baseAnimate(node, exit, options);
        (a as any)._leaving = true;
        anims.current.set(it.key, a);
        a.then(() => {
          if (anims.current.get(it.key) !== a) return;
          anims.current.delete(it.key);
          items.current = (items.current ?? []).filter((m) => m.key !== it.key);
          force((n) => n + 1);
        });
      } else if (!it.leaving && (running as any)?._leaving) {
        running!.cancel();
        anims.current.set(it.key, baseAnimate(node, enter, options));
      } else if (!it.leaving && !running && animateIn && !(node as any).__akSeen) {
        anims.current.set(it.key, baseAnimate(node, enter, options));
      }
      (node as any).__akSeen = true;
    }
  });
  useEffect(() => () => anims.current.forEach((a) => a.cancel()), []);

  return createElement('div', { style: { display: 'contents' } },
    merged.map((it) => createElement('div', {
      key: it.key, style: { display: 'contents' }, 'data-presence': it.leaving ? 'exit' : 'present',
      ref: (n: HTMLElement | null) => { if (n) nodes.current.set(it.key, n); else nodes.current.delete(it.key); },
    }, cloneElement(it.el))));
}
