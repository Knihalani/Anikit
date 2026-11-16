/**
 * Variants (Motion-style): name the states of an element once, then move
 * between them by name, with per-state transitions and parent → children
 * orchestration (`delayChildren`, `staggerChildren`, `when`).
 *
 * ```ts
 * const menu = variants('.menu', {
 *   closed: { opacity: 0, y: -8, transition: { duration: 0.2 } },
 *   open:   { opacity: 1, y: 0, transition: { staggerChildren: 0.04, delayChildren: 0.05 } },
 * }, {
 *   initial: 'closed',
 *   children: variants('.menu li', { closed: { opacity: 0, x: -12 }, open: { opacity: 1, x: 0 } }, { initial: 'closed' }),
 * });
 * button.onclick = () => menu.animate(menu.current === 'open' ? 'closed' : 'open');
 * ```
 */
import { animate, set, AnimateOptions, Props, ANIMATE_KEYS } from './animate';
import { timeline, Timeline } from './timeline';
import { track } from './context';
import { toTargets, Targets } from './props';
import { checkKeys, checkTargets, warn, didYouMean } from './debug';
import type { OptionKeys } from './debug';
import type { Playable } from './playable';

/** How a variant animates in, and how it orchestrates its children. */
export interface VariantTransition extends AnimateOptions {
  /** Seconds before the children start (default 0). */
  delayChildren?: number;
  /** Seconds between consecutive children (default 0). */
  staggerChildren?: number;
  /** Run the children `'beforeChildren'` / `'afterChildren'` this element's animation, or together (default). */
  when?: 'beforeChildren' | 'afterChildren' | false;
}
const TRANSITION_KEYS: OptionKeys<VariantTransition> = { ...ANIMATE_KEYS, delayChildren: 0, staggerChildren: 0, when: 0 };

/** One named state: properties (as for `animate()`, end values or keyframes) plus an optional `transition`. */
export type Variant = Props & {
  /** Options for animating into this state. */
  transition?: VariantTransition;
};

/** A variant, or a function of each element and its index returning one. */
export type VariantDef = Variant | ((el: Element, index: number) => Variant);

/** Options of `variants()`. */
export interface VariantsOptions {
  /** State applied immediately, without animating. */
  initial?: string;
  /** Default transition for every state (a state's own `transition` wins). */
  transition?: VariantTransition;
  /** Controllers (or one) that follow this one's state changes, orchestrated by the state's transition. */
  children?: Variants | Variants[];
}
const VARIANTS_KEYS: OptionKeys<VariantsOptions> = { initial: 0, transition: 0, children: 0 };

/** Controller returned by `variants()`. */
export class Variants {
  /** The current (or last requested) state name, if any. */
  current: string | undefined;
  private els: Element[];
  private children: Variants[];
  private running: Playable | null = null;
  private start = new Map<Element, string | null>();
  constructor(targets: Targets, private defs: Record<string, VariantDef>, private options: VariantsOptions = {}) {
    checkKeys('variants()', options, VARIANTS_KEYS);
    this.els = toTargets(targets).filter((t): t is Element => typeof Element !== 'undefined' && t instanceof Element);
    checkTargets('variants()', targets, this.els.length);
    for (const el of this.els) this.start.set(el, el.getAttribute('style'));
    this.children = options.children ? ([] as Variants[]).concat(options.children) : [];
    for (const [name, d] of Object.entries(defs)) if (typeof d === 'object' && d.transition) checkKeys(`variants() state "${name}" transition`, d.transition, TRANSITION_KEYS);
    if (options.initial !== undefined) this.set(options.initial);
    track(this);
  }

  /** Names of the defined states. */
  get names(): string[] {
    return Object.keys(this.defs);
  }

  private resolve(name: string): ((el: Element, i: number) => Variant) | null {
    const d = this.defs[name];
    if (!d) {
      warn(`variants(): unknown state "${name}".${didYouMean(name, this.names)} Defined: ${this.names.join(', ')}. See docs/api.md#variants-variants`);
      return null;
    }
    return typeof d === 'function' ? d : () => d;
  }

  /** Jump to a state without animating (children too). */
  set(name: string): this {
    const f = this.resolve(name);
    this.current = name;
    if (f) {
      this.running?.cancel();
      this.els.forEach((el, i) => {
        const { transition, ...props } = f(el, i);
        void transition;
        const end: Props = {};
        for (const [k, v] of Object.entries(props)) end[k] = Array.isArray(v) ? v[v.length - 1] : v;
        set(el, end);
      });
    }
    for (const c of this.children) c.set(name);
    return this;
  }

  /**
   * Animate to a state, then (or alongside, per `when`) the children. Returns a
   * timeline (awaitable); a running transition of this controller is replaced.
   */
  animate(name: string, overrides: AnimateOptions = {}): Timeline {
    const f = this.resolve(name);
    this.current = name;
    this.running?.cancel();
    let tr: VariantTransition = { ...this.options.transition };
    const own: Playable[] = [];
    if (f) {
      this.els.forEach((el, i) => {
        const v = f(el, i);
        tr = { ...this.options.transition, ...v.transition };
        own.push(this.animateOne(el, i, name, overrides)!);
      });
    }
    const kids: Playable[] = [];
    for (const c of this.children) {
      c.els.forEach((el, j) => { const k = c.animateOne(el, j, name, overrides); if (k) kids.push(k); });
      c.current = name;
    }
    const end = (list: Playable[], at = (i: number) => 0) => list.reduce((m, p, i) => Math.max(m, at(i) + p.duration), 0);
    const step = tr.staggerChildren ?? 0, delayKids = tr.delayChildren ?? 0;
    const ownStart = tr.when === 'afterChildren' && kids.length ? end(kids, (i) => delayKids + i * step) : 0;
    const kidsStart = (tr.when === 'beforeChildren' ? end(own) : 0) + delayKids;
    const tl = timeline();
    for (const p of own) tl.add(p, ownStart);
    kids.forEach((k, i) => tl.add(k, kidsStart + i * step));
    this.running = tl;
    return tl;
  }

  /** One element's animation into `name` (used when a parent orchestrates this controller). */
  private animateOne(el: Element, i: number, name: string, overrides: AnimateOptions): Playable | null {
    const f = this.resolve(name);
    if (!f) return null;
    const { transition, ...props } = f(el, i);
    const { delayChildren, staggerChildren, when, ...anim } = { ...this.options.transition, ...transition };
    void delayChildren; void staggerChildren; void when;
    return animate(el, props as Props, { ...anim, ...overrides, autoplay: false });
  }

  /** Stop and restore the elements' original inline styles. */
  revert(): void {
    this.running?.cancel();
    this.running = null;
    for (const [el, s] of this.start) {
      if (s === null) el.removeAttribute('style');
      else el.setAttribute('style', s);
    }
  }

  /** Stop, leaving the elements where they are. */
  kill(): void {
    this.running?.cancel();
    this.running = null;
  }
}

/** Named states for `targets`, animated between with `.animate(name)` (see `Variants`). */
export function variants(targets: Targets, defs: Record<string, VariantDef>, options?: VariantsOptions): Variants {
  return new Variants(targets, defs, options);
}
