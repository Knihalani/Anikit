/**
 * Scroll-linked animation and viewport detection.
 *
 * ```ts
 * scroll(animate('.bar', { scaleX: [0, 1] }, { ease: 'linear' }));          // page progress bar
 * scroll(animate(img, { y: [-50, 50] }), { target: img, smooth: 0.1 });    // smoothed parallax
 * scroll(compositor('.hero', { scale: [1, 1.2], opacity: [1, 0] }), { target: hero, offset: ['start start', 'end start'] }); // runs on the compositor
 * inView('.card', (el) => { animate(el, { opacity: [0, 1] }); });
 * ```
 */
import { Playable } from './playable';
import { Targets, toTargets } from './props';
import { spring, PhysicsAnimation } from './spring';
import { whenReady } from './wasm';
import { CompositorAnimation } from './compositor';

type Edge = string | number;
export interface ScrollOptions {
  /** Element whose position within the container drives progress (default: the container's own scroll). */
  target?: Element;
  /** Scroll container (default: the page). */
  container?: Element | Window;
  axis?: 'x' | 'y';
  /**
   * `[startOffset, endOffset]`, each `"<target edge> <container edge>"`. Edges:
   * `start | center | end`, a fraction `0.25`, `"40%"` or `"120px"`.
   * Default `['start end', 'end start']` (enter → exit).
   */
  offset?: [string, string];
  /** Smooth the progress with a spring; value ≈ lag in seconds (0 = off). */
  smooth?: number;
  /**
   * For {@link compositor} animations: hand them to a native `ScrollTimeline`
   * so the browser scrubs them off the main thread (default true; ignored
   * with `smooth`). Without support they're scrubbed from the scroll event.
   */
  native?: boolean;
}

const EDGES: Record<string, number> = { start: 0, center: 0.5, end: 1 };

function edge(v: string, size: number): number {
  if (v in EDGES) return EDGES[v] * size;
  if (v.endsWith('px')) return parseFloat(v);
  if (v.endsWith('%')) return (parseFloat(v) / 100) * size;
  return parseFloat(v) * size;
}

function parseOffset(o: string): [string, string] {
  const p = o.trim().split(/\s+/);
  return [p[0] ?? 'start', p[1] ?? p[0] ?? 'start'];
}

/**
 * Drive an animation's progress (or call a function with progress 0–1) from
 * scroll position. Returns a cleanup function.
 *
 * {@link compositor} animations are attached to a native `ScrollTimeline`
 * where supported: the browser then scrubs them on the compositor thread and
 * no JavaScript runs while scrolling.
 */
export function scroll(subject: Playable | CompositorAnimation | ((progress: number) => void), options: ScrollOptions = {}): () => void {
  if (typeof window === 'undefined') return () => {};
  const container = options.container ?? window;
  const isWin = container === window;
  const axis = options.axis ?? 'y';
  const scroller = (isWin ? document.scrollingElement || document.documentElement : container) as Element;
  const [o0, o1] = (options.offset ?? ['start end', 'end start']).map(parseOffset);

  let start = 0;
  let end = 1;
  const measure = () => {
    const size = axis === 'y'
      ? (isWin ? window.innerHeight : scroller.clientHeight)
      : (isWin ? window.innerWidth : scroller.clientWidth);
    const t = options.target;
    if (!t) {
      start = 0;
      end = axis === 'y' ? scroller.scrollHeight - size : scroller.scrollWidth - size;
      return;
    }
    const r = t.getBoundingClientRect();
    const cr = isWin ? { top: 0, left: 0 } : scroller.getBoundingClientRect();
    const pos = axis === 'y' ? r.top - cr.top + scroller.scrollTop : r.left - cr.left + scroller.scrollLeft;
    const len = axis === 'y' ? r.height : r.width;
    start = pos + edge(o0[0], len) - edge(o0[1], size);
    end = pos + edge(o1[0], len) - edge(o1[1], size);
  };

  if (subject instanceof CompositorAnimation) {
    subject._linked = true;
    const G = globalThis as any;
    if (options.native !== false && !options.smooth && typeof G.ScrollTimeline === 'function') {
      measure();
      return nativeScroll(subject, scroller, axis, measure, () => [start, end]);
    }
  }
  const apply: (p: number) => void =
    subject instanceof Playable || subject instanceof CompositorAnimation
      ? (p) => {
          subject.progress = p;
        }
      : subject;
  if (subject instanceof Playable) subject.pause();

  // Optional smoothing through a spring on a plain object.
  const proxy = { p: 0 };
  let smoother: PhysicsAnimation | null = null;
  const read = () => {
    const s = axis === 'y' ? scroller.scrollTop : scroller.scrollLeft;
    const span = end - start;
    return span > 0 ? Math.min(1, Math.max(0, (s - start) / span)) : s >= end ? 1 : 0;
  };
  const springOpts = {
    duration: Math.max(0.01, options.smooth ?? 0) * 4,
    bounce: 0,
    restDelta: 1e-4,
    restSpeed: 1e-3,
    onUpdate: () => apply(proxy.p),
  };
  const update = () => {
    const p = read();
    if (options.smooth) smoother = spring(proxy, { p }, springOpts);
    else apply(p);
  };
  const onResize = () => {
    measure();
    update();
  };
  measure();
  proxy.p = read();
  whenReady(() => apply(proxy.p));
  const evTarget: EventTarget = isWin ? window : scroller;
  evTarget.addEventListener('scroll', update, { passive: true });
  window.addEventListener('resize', onResize, { passive: true });
  return () => {
    evTarget.removeEventListener('scroll', update);
    window.removeEventListener('resize', onResize);
    smoother?.stop();
  };
}

/** Attach compositor animations to a native scroll timeline over [start, end] px. */
function nativeScroll(
  subject: CompositorAnimation, scroller: Element, axis: 'x' | 'y', measure: () => void, range: () => [number, number],
): () => void {
  const G = globalThis as any;
  const timeline = new G.ScrollTimeline({ source: scroller, axis: axis === 'y' ? 'block' : 'inline' });
  const attach = () => {
    const [a, b] = range();
    for (const anim of subject.animations) {
      const x = anim as any;
      if (x.timeline !== timeline) x.timeline = timeline;
      // Lengths from the start of the scroll range (the string form: Chromium
      // reads a bare CSSUnitValue here as 0%).
      x.rangeStart = Math.max(0, a) + 'px';
      x.rangeEnd = Math.max(a + 0.01, b) + 'px';
      if (anim.playState !== 'running') anim.play();
    }
  };
  subject._whenBuilt(attach);
  // Ranges are in px: re-measure when the layout changes size.
  const onResize = () => {
    measure();
    attach();
  };
  window.addEventListener('resize', onResize, { passive: true });
  return () => {
    window.removeEventListener('resize', onResize);
    subject._whenBuilt(() => {
      for (const anim of subject.animations) {
        // Keep the current look: same progress, back on the document timeline.
        const p = Number(anim.effect?.getComputedTiming().progress ?? 0) || 0;
        (anim as any).timeline = document.timeline;
        anim.pause();
        const end = Number(anim.effect?.getComputedTiming().endTime ?? 0);
        if (Number.isFinite(end)) anim.currentTime = p * end;
      }
    });
  };
}

export interface InViewOptions {
  root?: Element | Document;
  /** Root margin, e.g. `"0px 0px -20% 0px"`. */
  margin?: string;
  /** Fraction visible required: 'some' (default), 'all' or 0–1. */
  amount?: 'some' | 'all' | number;
  /** Only fire once per element. */
  once?: boolean;
}

/**
 * Call `onEnter(element, entry)` when targets enter the viewport. If it
 * returns a function, that is called when the element leaves. Returns a
 * cleanup function.
 */
export function inView(
  targets: Targets,
  onEnter: (el: Element, entry: IntersectionObserverEntry) => void | ((entry: IntersectionObserverEntry) => void),
  options: InViewOptions = {},
): () => void {
  if (typeof IntersectionObserver === 'undefined') return () => {};
  const threshold = options.amount === 'all' ? 1 : options.amount === 'some' || options.amount === undefined ? 0 : options.amount;
  const leaves = new WeakMap<Element, (e: IntersectionObserverEntry) => void>();
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        const leave = leaves.get(e.target);
        if (e.isIntersecting && !leave) {
          const r = onEnter(e.target, e);
          if (typeof r === 'function') leaves.set(e.target, r);
          else leaves.set(e.target, () => {});
          if (options.once) io.unobserve(e.target);
        } else if (!e.isIntersecting && leave) {
          leave(e);
          leaves.delete(e.target);
        }
      }
    },
    { root: options.root as Element | null | undefined, rootMargin: options.margin, threshold },
  );
  for (const el of toTargets(targets)) io.observe(el as Element);
  return () => io.disconnect();
}
