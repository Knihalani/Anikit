/**
 * View Transitions: page-level and shared-element transitions animated by the
 * browser on the compositor, timed with any Anikit easing (springs included,
 * through native `linear()`), with a FLIP fallback where the API is missing.
 *
 * ```ts
 * await viewTransition(() => showDetail(id), {
 *   ease: 'spring(bounce: 0.2, duration: 0.5)',
 *   names: { hero: [thumb, () => document.querySelector('.detail img')] },   // shared element
 * });
 * viewTransition(() => navigate(), { old: { opacity: [1, 0] }, new: { transform: ['translateY(24px)', 'none'], opacity: [0, 1] } });
 * ```
 */
import { cssEasing } from './compositor';
import { EasingInput, prefersReduced } from './core';
import { flip } from './flip';

type ElementRef = Element | string | (() => Element | null | undefined);

export interface ViewTransitionOptions {
  /** Seconds for every group (default: the browser's 0.25 s). */
  duration?: number;
  /** Any Anikit easing; springs and custom curves become `linear()` (default: the browser's `ease`). */
  ease?: EasingInput;
  /** Seconds before the animations start. */
  delay?: number;
  /**
   * Elements that get their own `view-transition-name` for this transition
   * only. A pair `[before, after]` names `before` in the old state and `after`
   * in the new one: the browser morphs one into the other.
   */
  names?: Record<string, ElementRef | [ElementRef, ElementRef]>;
  /** Keyframes (WAAPI form) for the outgoing snapshot of `scope` (default scope: 'root'). */
  old?: Keyframe[] | PropertyIndexedKeyframes;
  /** Keyframes (WAAPI form) for the incoming snapshot of `scope`. */
  new?: Keyframe[] | PropertyIndexedKeyframes;
  /** Which transition group `old`/`new` apply to: 'root' or one of `names`. */
  scope?: string;
  /** View transition types (`:active-view-transition-type()`), where supported. */
  types?: string[];
  /**
   * Without View Transitions support (or with reduced motion): `'flip'`
   * (default) animates the named elements with {@link flip}; `'none'` just
   * runs the update.
   */
  fallback?: 'flip' | 'none';
}

/** Handle mirroring the native `ViewTransition`; awaiting it waits for `finished`. */
export interface ViewTransitionHandle extends PromiseLike<void> {
  /** The update has run and the new state is in the DOM. */
  updateCallbackDone: Promise<void>;
  /** Snapshots are taken and the animation is about to start. */
  ready: Promise<void>;
  /** The transition is over (also after a skip or a fallback). */
  finished: Promise<void>;
  /** Jump to the end state. */
  skipTransition(): void;
  /** True when the native API ran the transition. */
  native: boolean;
}

function resolve(ref: ElementRef): Element | null {
  if (typeof ref === 'function') return ref() ?? null;
  if (typeof ref === 'string') return typeof document !== 'undefined' ? document.querySelector(ref) : null;
  return ref;
}

/** Give `el` a transition name; returns the undo. */
function nameIt(el: Element | null, name: string): () => void {
  if (!el) return () => {};
  const st = (el as HTMLElement).style;
  const prev = st.getPropertyValue('view-transition-name');
  st.setProperty('view-transition-name', name);
  return () => (prev ? st.setProperty('view-transition-name', prev) : st.removeProperty('view-transition-name'));
}

let uid = 0;

/**
 * Run `update` (which changes the DOM) as a View Transition. Returns a handle
 * that is awaitable and mirrors `ViewTransition`.
 */
export function viewTransition(update: () => void | Promise<void>, options: ViewTransitionOptions = {}): ViewTransitionHandle {
  const doc: any = typeof document !== 'undefined' ? document : null;
  const names = options.names ?? {};
  const undo: Array<() => void> = [];
  const cleanup = () => {
    for (const u of undo.splice(0).reverse()) u();
  };

  // Old-state names (and the elements a FLIP fallback would track).
  const tracked: Element[] = [];
  const nameOf = new Map<Element, string>();
  for (const [name, ref] of Object.entries(names)) {
    const before = resolve(Array.isArray(ref) ? ref[0] : ref);
    if (before) {
      tracked.push(before);
      nameOf.set(before, name);
    }
    undo.push(nameIt(before, name));
  }
  const rename = () => {
    // Pairs: the name moves to the "after" element in the new state.
    for (const [name, ref] of Object.entries(names)) {
      if (!Array.isArray(ref)) continue;
      const before = resolve(ref[0]);
      if (before) (before as HTMLElement).style.removeProperty('view-transition-name');
      const after = resolve(ref[1]);
      if (after) {
        nameOf.set(after, name);
        undo.push(nameIt(after, name));
      }
    }
  };
  const wrapped = async () => {
    await update();
    rename();
  };

  const supported = !!doc && typeof doc.startViewTransition === 'function';
  if (!supported || prefersReduced()) {
    // Fallback: run the update; optionally FLIP the named elements.
    const flipIt = options.fallback !== 'none' && !supported && !prefersReduced() && tracked.length > 0;
    const state = flipIt ? flip.capture(tracked, { id: (el) => nameOf.get(el) }) : null;
    let tl: PromiseLike<void> | null = null;
    let skip: (() => void) | null = null;
    const updated = (async () => {
      await update();
      rename();
      if (state) {
        const after = Object.values(names).map((r) => resolve(Array.isArray(r) ? r[1] : r)).filter((e): e is Element => !!e);
        const t = flip.from(state, {
          targets: after, id: (el) => nameOf.get(el), crossfade: false,
          duration: options.duration ?? 0.25, ease: options.ease ?? 'ease', delay: options.delay,
        });
        tl = t;
        skip = () => t.complete();
      }
      cleanup();
    })();
    const finished = updated.then(() => tl ?? undefined).then(() => undefined);
    return {
      native: false, updateCallbackDone: updated, ready: updated, finished,
      skipTransition: () => skip?.(),
      then: (x, y) => finished.then(x, y),
    };
  }

  // Timing for every group / snapshot, as a temporary stylesheet.
  let style: HTMLStyleElement | null = null;
  const id = 'anikit-vt-' + ++uid;
  const decl: string[] = [];
  if (options.duration !== undefined) decl.push(`animation-duration:${options.duration}s`);
  if (options.delay !== undefined) decl.push(`animation-delay:${options.delay}s`);
  const easing = options.ease !== undefined ? easingString(options.ease) : null;
  if (easing) decl.push(`animation-timing-function:${easing}`);
  if (decl.length) {
    style = doc.createElement('style') as HTMLStyleElement;
    style.id = id;
    style.textContent =
      `::view-transition-group(*),::view-transition-old(*),::view-transition-new(*),::view-transition-image-pair(*){${decl.join(';')}}`;
    doc.head.appendChild(style);
    undo.push(() => style!.remove());
  }

  let vt: any;
  try {
    vt = options.types ? doc.startViewTransition({ update: wrapped, types: options.types }) : doc.startViewTransition(wrapped);
  } catch {
    // Older implementations take only a callback.
    vt = doc.startViewTransition(wrapped);
  }
  const scope = options.scope ?? 'root';
  const custom = (which: 'old' | 'new') => {
    const kf = options[which];
    if (!kf) return;
    const root = doc.documentElement as HTMLElement;
    root.animate(kf, {
      pseudoElement: `::view-transition-${which}(${scope})`,
      duration: (options.duration ?? 0.25) * 1000,
      delay: (options.delay ?? 0) * 1000,
      easing: easing ?? 'ease',
      fill: 'both',
    });
  };
  const ready: Promise<void> = Promise.resolve(vt.ready).then(() => {
    custom('old');
    custom('new');
  });
  ready.catch(() => undefined);
  const finished: Promise<void> = Promise.resolve(vt.finished).then(cleanup, (e: unknown) => {
    cleanup();
    throw e;
  });
  return {
    native: true,
    updateCallbackDone: Promise.resolve(vt.updateCallbackDone),
    ready,
    finished,
    skipTransition: () => vt.skipTransition(),
    then: (a, b) => finished.then(a, b),
  };
}

function easingString(e: EasingInput): string {
  // `cssEasing` needs the core (springs are sampled there); a View Transition
  // starts synchronously, so fall back to the CSS keyword until it's ready.
  let s = typeof e === 'string' && /^(linear|ease(-in|-out|-in-out)?|step-(start|end)|steps\(|cubic-bezier\()/.test(e) ? e : 'ease';
  try {
    s = cssEasing(e);
  } catch {
    /* core not loaded yet */
  }
  return s;
}
