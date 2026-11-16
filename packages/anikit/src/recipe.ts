/**
 * Recipes: one call for a polished, common pattern, composed from the
 * primitives (`animate`, `timeline`, `splitText`, `scrollTrigger`,
 * `magnetic`, `parallax`, `inView`). Good-looking defaults, a handful of
 * options, and one handle that reverts everything it created.
 *
 * ```ts
 * recipe('hero-reveal', '.hero');
 * recipe('scroll-story', 'main', { children: 'section' });
 * recipe('magnetic-buttons', '.cta');
 * recipe('text-scramble', '#title', { text: 'HELLO WORLD' });
 * recipe('parallax-gallery', '.gallery');
 * ```
 */
import { animate } from './animate';
import { timeline } from './timeline';
import { stagger } from './stagger';
import { splitText } from './text';
import { magnetic, parallax } from './pointerfx';
import { inView } from './scroll';
import { scrollTriggerBatch } from './scrolltrigger';
import { Context, track } from './context';
import { toTargets, Targets, isElement } from './props';
import { checkKeys, checkTargets, didYouMean, docLink } from './debug';
import type { OptionKeys } from './debug';
import type { EasingInput } from './core';
import type { Playable } from './playable';

/** Options shared by every recipe (each recipe ignores those it doesn't use, and warns about unknown keys). */
export interface RecipeOptions {
  /** Overall duration in seconds of each piece's motion (each recipe has its own default). */
  duration?: number;
  /** Seconds before the recipe starts (default 0). */
  delay?: number;
  /** Seconds between consecutive pieces (characters, words, items). */
  stagger?: number;
  /** Easing (default `'expo.out'`, or a spring for pointer recipes). */
  ease?: EasingInput;
  /** Selector for the pieces inside the element (`'scroll-story'`: `'section'`; `'parallax-gallery'`: `'img'`; `'stagger-in'`: children). */
  children?: string;
  /** `'hero-reveal'`: selector of the headline inside the element (default `'h1, h2'`). */
  headline?: string;
  /** `'hero-reveal'`, `'text-scramble'`: split the headline by `'words'` (default) or `'chars'`. */
  by?: 'words' | 'chars';
  /** `'text-scramble'`: final text (default: the element's current text). */
  text?: string;
  /** `'text-scramble'`: glyph set (`'upper' | 'lower' | 'digits' | 'symbols' | 'binary'` or your own characters). */
  chars?: string;
  /** Travel distance in px for slide-in motion (default 40). */
  distance?: number;
  /** `'magnetic-buttons'`: pull strength 0–1 (default 0.35). `'parallax-gallery'`: max speed (default 0.25). */
  strength?: number;
  /** Start when scrolled into view instead of immediately (default: true for scroll recipes, false for hero/scramble). */
  onScroll?: boolean;
  /** Only animate the first time (scroll recipes, default true). */
  once?: boolean;
}

/** Valid keys of `RecipeOptions` (drives the "unknown option" warnings). */
export const RECIPE_KEYS: OptionKeys<RecipeOptions> = {
  duration: 0, delay: 0, stagger: 0, ease: 0, children: 0, headline: 0, by: 0, text: 0, chars: 0, distance: 0,
  strength: 0, onScroll: 0, once: 0,
};

/** Names of the built-in recipes. */
export type RecipeName =
  | 'hero-reveal' | 'scroll-story' | 'magnetic-buttons' | 'text-scramble' | 'parallax-gallery'
  | 'stagger-in' | 'draw-svg' | 'count-up' | 'tilt-cards';

/** What `recipe()` returns: undo everything it created in one call. */
export interface RecipeHandle {
  /** The recipe's name. */
  readonly name: RecipeName;
  /** The animations started so far (scroll recipes add theirs as elements enter). */
  readonly animations: Playable[];
  /** Resolves when every animation started synchronously has finished (scroll recipes resolve at once). */
  readonly finished: Promise<void>;
  /** Stop and restore the original styles and markup. */
  revert(): void;
  /** Stop, leaving everything where it is. */
  kill(): void;
}

type Build = (els: HTMLElement[], o: RecipeOptions, out: Playable[]) => void;

const reveal = (el: Element, o: RecipeOptions, i = 0) =>
  animate(el, { opacity: [0, 1], y: [o.distance ?? 40, 0] }, {
    duration: o.duration ?? 0.9, ease: o.ease ?? 'expo.out', delay: (o.delay ?? 0) + i * (o.stagger ?? 0.08),
  });

/** Run `start` now, or when each element first scrolls into view. */
function whenVisible(els: Element[], o: RecipeOptions, scrollDefault: boolean, start: (el: Element, i: number) => Playable | void, out: Playable[]) {
  if (!(o.onScroll ?? scrollDefault)) {
    els.forEach((el, i) => { const a = start(el, i); if (a) out.push(a); });
    return;
  }
  const off = inView(els, (el) => {
    const a = start(el, 0);
    if (a) out.push(a);
  }, { once: o.once ?? true, amount: 0.2 });
  track({ revert: off, kill: off });
}

const RECIPES: Record<RecipeName, Build> = {
  'hero-reveal'(els, o, out) {
    for (const el of els) {
      const head = (el.matches('h1, h2, h3') ? el : el.querySelector(o.headline ?? 'h1, h2')) as HTMLElement | null;
      const rest = Array.from(el.children).filter((c) => c !== head && !c.contains(head));
      const by = o.by ?? 'words';
      const tl = timeline({ delay: o.delay ?? 0 });
      if (head) {
        const split = splitText(head, { type: by === 'chars' ? 'words,chars' : 'words', mask: by });
        tl.add(animate(by === 'chars' ? split.chars : split.words, { y: ['110%', '0%'], opacity: [0, 1] }, {
          duration: o.duration ?? 1, ease: o.ease ?? 'expo.out', delay: stagger(o.stagger ?? (by === 'chars' ? 0.02 : 0.06)),
        }));
      }
      if (rest.length) {
        tl.add(animate(rest, { opacity: [0, 1], y: [o.distance ?? 24, 0] }, {
          duration: 0.8, ease: o.ease ?? 'expo.out', delay: stagger(0.08),
        }), head ? '-=0.6' : 0);
      }
      out.push(tl);
    }
  },
  'scroll-story'(els, o, out) {
    for (const el of els) {
      const parts = Array.from(el.querySelectorAll(o.children ?? 'section, [data-story]'));
      if (!parts.length) parts.push(el);
      if (o.onScroll === false) parts.forEach((p, i) => out.push(reveal(p, o, i)));
      else {
        scrollTriggerBatch(parts, {
          start: 'top 85%', toggleActions: o.once === false ? 'play none none reverse' : 'play none none none',
          animation: (p) => {
            const a = animate(p, { opacity: [0, 1], y: [o.distance ?? 60, 0] }, { duration: o.duration ?? 1, ease: o.ease ?? 'expo.out', autoplay: false });
            out.push(a);
            return a;
          },
        });
      }
    }
  },
  'magnetic-buttons'(els, o) {
    const btns = els.flatMap((el) => el.matches('button, a, [role=button], .btn') ? [el] : Array.from(el.querySelectorAll<HTMLElement>(o.children ?? 'button, a, [role=button], .btn')));
    magnetic(btns.length ? btns : els, { strength: o.strength ?? 0.35, radius: o.distance ?? 80 });
  },
  'text-scramble'(els, o, out) {
    whenVisible(els, o, false, (el, i) => animate(el, {
      text: { value: o.text ?? el.textContent ?? '', type: 'scramble', chars: o.chars ?? 'upper' },
    }, { duration: o.duration ?? 1.2, ease: o.ease ?? 'linear', delay: (o.delay ?? 0) + i * (o.stagger ?? 0) }), out);
  },
  'parallax-gallery'(els, o) {
    const s = o.strength ?? 0.25;
    for (const el of els) {
      const items = Array.from(el.querySelectorAll<HTMLElement>(o.children ?? 'img, figure, [data-parallax]'));
      // Alternate depths so neighbours separate as the page scrolls.
      items.forEach((it, i) => parallax(it, { speed: [-s, s * 0.4, -s * 0.6][i % 3] }));
    }
  },
  'stagger-in'(els, o, out) {
    whenVisible(els, o, true, (el) => {
      const kids = o.children ? Array.from(el.querySelectorAll(o.children)) : Array.from(el.children);
      return animate(kids.length ? kids : el, { opacity: [0, 1], y: [o.distance ?? 30, 0], scale: [0.96, 1] }, {
        duration: o.duration ?? 0.7, ease: o.ease ?? 'expo.out', delay: stagger(o.stagger ?? 0.06, { start: o.delay ?? 0 }),
      });
    }, out);
  },
  'draw-svg'(els, o, out) {
    whenVisible(els, o, true, (el) => {
      const paths = el.matches('path, line, polyline, polygon, circle, ellipse, rect') ? [el] : Array.from(el.querySelectorAll(o.children ?? 'path, line, polyline, polygon, circle, ellipse, rect'));
      return animate(paths, { draw: [0, 1] }, { duration: o.duration ?? 1.6, ease: o.ease ?? 'power2.inOut', delay: stagger(o.stagger ?? 0.12, { start: o.delay ?? 0 }) });
    }, out);
  },
  'count-up'(els, o, out) {
    whenVisible(els, o, true, (el) => {
      const to = parseFloat((o.text ?? el.textContent ?? '0').replace(/[^\d.-]/g, '')) || 0;
      const box = { v: 0 };
      return animate(box, { v: [0, to] }, {
        duration: o.duration ?? 1.6, ease: o.ease ?? 'expo.out', delay: o.delay ?? 0,
        onUpdate: () => { el.textContent = Math.round(box.v).toLocaleString(); },
      });
    }, out);
  },
  'tilt-cards'(els, o) {
    const cards = els.flatMap((el) => o.children ? Array.from(el.querySelectorAll<HTMLElement>(o.children)) : [el]);
    const max = (o.strength ?? 0.5) * 20;
    for (const card of cards) {
      const move = (e: PointerEvent) => {
        const r = card.getBoundingClientRect();
        const px = r.width ? (e.clientX - r.left) / r.width - 0.5 : 0, py = r.height ? (e.clientY - r.top) / r.height - 0.5 : 0;
        animate(card, { rotateY: px * max, rotateX: -py * max, perspective: 800 }, { duration: 0.4, ease: 'expo.out' });
      };
      const leave = () => animate(card, { rotateX: 0, rotateY: 0 }, { duration: 0.8, ease: 'spring(bounce: 0.4)' });
      card.addEventListener('pointermove', move);
      card.addEventListener('pointerleave', leave);
      const off = () => { card.removeEventListener('pointermove', move); card.removeEventListener('pointerleave', leave); };
      track({ revert: () => { off(); card.style.transform = ''; }, kill: off });
    }
  },
};

/** Names of every recipe, for listing or validation. */
export function recipeNames(): RecipeName[] {
  return Object.keys(RECIPES) as RecipeName[];
}

/**
 * A polished pattern in one call. `name` is one of `recipeNames()`; `targets`
 * the container (or elements) it applies to. Everything created is tracked:
 * `revert()` restores styles and markup, and an enclosing `context()` reverts
 * it too. Unknown names throw with a suggestion; unknown options warn.
 */
export function recipe(name: RecipeName, targets: Targets, options: RecipeOptions = {}): RecipeHandle {
  const build = RECIPES[name];
  if (!build) {
    throw new Error(`anikit: unknown recipe "${name}".${didYouMean(String(name), Object.keys(RECIPES))} Available: ${recipeNames().join(', ')}. See ${docLink('recipe()')}`);
  }
  checkKeys(`recipe('${name}')`, options, RECIPE_KEYS);
  const els = toTargets(targets).filter(isElement) as HTMLElement[];
  checkTargets(`recipe('${name}')`, targets, els.length);
  const animations: Playable[] = [];
  const ctx = new Context(() => build(els, options, animations));
  const finished = Promise.all(animations.slice().map((a) => a.finished)).then(() => undefined);
  const handle: RecipeHandle = {
    name, animations, finished,
    revert: () => ctx.revert(),
    kill: () => ctx.kill(),
  };
  track(handle);
  return handle;
}
