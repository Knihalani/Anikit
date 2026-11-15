/**
 * FLIP layout animations: animate elements between two layouts using only
 * transforms (compositor-friendly), no matter how the layout changed.
 *
 * ```ts
 * flip('.item', () => list.classList.toggle('grid'), { duration: 0.6, ease: 'spring(bounce: 0.2)' });
 * // or capture manually
 * const state = flip.capture('.item'); reorder(); flip.from(state);
 * ```
 */
import { Animation, AnimateOptions, Props } from './animate';
import { Targets, toTargets, stateOf, flushTransforms } from './props';

export interface FlipState {
  els: Element[];
  rects: DOMRect[];
  opacity: string[];
}

export interface FlipOptions extends AnimateOptions {
  /** Animate size changes with scale (default true). */
  scale?: boolean;
  /** Fade elements that appear/disappear (when given in `capture` but not visible after). */
  fade?: boolean;
}

function capture(targets: Targets): FlipState {
  const els = toTargets(targets) as Element[];
  return {
    els,
    rects: els.map((e) => e.getBoundingClientRect()),
    opacity: els.map((e) => (e as HTMLElement).style.opacity),
  };
}

function fromState(state: FlipState, options: FlipOptions = {}): Animation {
  const { scale = true, ...anim } = options;
  const kf: Props[] = [];
  const els: Element[] = [];
  state.els.forEach((el, i) => {
    const s = stateOf(el);
    // Measure the new layout without our current transform.
    const html = el as HTMLElement;
    const prevTransform = html.style.transform;
    html.style.transform = 'none';
    const last = el.getBoundingClientRect();
    html.style.transform = prevTransform;
    const first = state.rects[i];
    const t = s.t;
    const sx = scale && last.width ? first.width / last.width : 1;
    const sy = scale && last.height ? first.height / last.height : 1;
    // Element transforms use the centre as origin.
    const dx = first.left + first.width / 2 - (last.left + last.width / 2);
    const dy = first.top + first.height / 2 - (last.top + last.height / 2);
    if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5 && Math.abs(sx - 1) < 1e-3 && Math.abs(sy - 1) < 1e-3) return;
    kf.push({
      x: [t[0] + dx, t[0]],
      y: [t[1] + dy, t[1]],
      ...(scale ? { scaleX: [t[8] * sx, t[8]], scaleY: [t[9] * sy, t[9]] } : {}),
    });
    els.push(el);
  });
  const keys = ['x', 'y', ...(scale ? ['scaleX', 'scaleY'] : [])];
  // Function values are resolved per target at build time.
  const resolved: Props = {};
  for (const k of keys) {
    resolved[k] = [
      ((_t: unknown, i: number) => (kf[i][k] as number[])[0]) as any,
      ((_t: unknown, i: number) => (kf[i][k] as number[])[1]) as any,
    ];
  }
  const a = new Animation(els, resolved, { duration: 0.5, ease: 'outCubic', immediateRender: true, ...anim });
  // Render the inverted state before the next paint.
  a.seek(0);
  flushTransforms();
  return a;
}

/** Record positions, run `mutate`, then animate from the old layout to the new one. */
export function flip(targets: Targets, mutate: () => void, options?: FlipOptions): Animation {
  const state = capture(targets);
  mutate();
  return fromState({ ...state, els: state.els }, options);
}
flip.capture = capture;
flip.from = fromState;
