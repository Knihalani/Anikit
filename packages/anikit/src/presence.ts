/**
 * Presence: enter and exit animations for elements that come and go.
 *
 * The DOM has no "about to be removed" phase, so removing an element with an
 * exit animation means animating first and removing afterwards — and coping
 * with the element coming back halfway. `presence(container)` manages that
 * for a container's children, and (by default) animates the siblings into
 * their new places with {@link flip} while elements enter and leave.
 *
 * ```ts
 * const list = presence('#todos', { enter: { opacity: [0, 1], y: [-8, 0] }, exit: { opacity: 0, scale: 0.9 } });
 * list.add(item);                 // inserted, then animated in; siblings slide
 * list.remove(item);              // animated out in place, then removed
 * list.replace(oldView, newView, { mode: 'wait' }); // out, then in
 * exit('.toast');                 // standalone: fade out, then remove
 * ```
 */
import { animate, AnimateOptions, Animation, Props } from './animate';
import { flip, FlipOptions, cancelLeave, isLeaving } from './flip';
import { Playable } from './playable';
import { track } from './context';
import { Targets, toTargets, isElement } from './props';

export interface PresenceOptions extends AnimateOptions {
  /** Keyframes for entering children (default `{ opacity: [0, 1] }`). */
  enter?: Props;
  /** Keyframes for leaving children (default `{ opacity: 0 }`). */
  exit?: Props;
  /**
   * Animate siblings into their new places as children come and go (default
   * true). Leaving children then leave the flow at once (they're pinned where
   * they were while they animate out).
   */
  layout?: boolean;
  /** After the exit: remove the element (default) or hide it with the `hidden` attribute. */
  after?: 'remove' | 'hide';
}

/** Exits in progress for elements animated out in the flow (no layout mode). */
const exiting = new WeakMap<Element, Animation>();

function finishExit(el: Element, how: 'remove' | 'hide'): void {
  if (how === 'hide') (el as HTMLElement).hidden = true;
  else el.remove();
}

/** Turn a running exit around. Returns the animation, or null if there was none. */
function turnAround(el: Element): Playable | null {
  const ex = exiting.get(el);
  if (!ex) return null;
  exiting.delete(el);
  ex.reverse();
  return ex;
}

/**
 * Animate targets out, then remove them (or hide them with `after: 'hide'`).
 * Calling {@link Presence.add} (or `enter()`) on one before it's gone turns the
 * exit around instead.
 *
 * ```ts
 * exit('.toast', { opacity: 0, y: 20 }, { duration: 0.3 });
 * ```
 */
export function exit(targets: Targets, keyframes: Props = { opacity: 0 }, options: AnimateOptions & { after?: 'remove' | 'hide' } = {}): Animation {
  const { after = 'remove', ...opts } = options;
  const els = toTargets(targets).filter(isElement);
  const user = opts.onComplete;
  const a: Animation = animate(els, keyframes, {
    duration: 0.3, ease: 'inQuad', ...opts,
    onComplete: (self) => {
      for (const el of els) {
        if (exiting.get(el) !== a) continue;
        exiting.delete(el);
        finishExit(el, after);
      }
      user?.(self);
    },
  });
  for (const el of els) exiting.set(el, a);
  return a;
}

/**
 * Show targets with an entrance: un-hides them (`hidden` attribute), or turns
 * a running {@link exit} around.
 *
 * ```ts
 * enter('.panel', { opacity: [0, 1], y: [12, 0] });
 * ```
 */
export function enter(targets: Targets, keyframes: Props = { opacity: [0, 1] }, options: AnimateOptions = {}): Playable {
  const fresh: Element[] = [];
  let back: Playable | null = null;
  for (const el of toTargets(targets).filter(isElement)) {
    const t = turnAround(el) ?? null;
    if (t) back = t;
    else {
      cancelLeave(el);
      (el as HTMLElement).hidden = false;
      fresh.push(el);
    }
  }
  if (!fresh.length && back) return back;
  return animate(fresh, keyframes, { duration: 0.4, ease: 'outQuad', immediateRender: true, ...options });
}

/** Enter/exit manager for a container's children; see {@link presence}. */
export class Presence {
  readonly container: Element;
  private o: PresenceOptions;
  private live = new Set<Playable>();
  constructor(container: Element, options: PresenceOptions = {}) {
    this.container = container;
    this.o = options;
    track(this);
  }

  private anim(): AnimateOptions {
    const { enter: _e, exit: _x, layout: _l, after: _a, ...rest } = this.o;
    void _e; void _x; void _l; void _a;
    return rest;
  }
  private flipOpts(extra: AnimateOptions = {}): FlipOptions {
    return {
      ...this.anim(), ...extra,
      targets: this.container.children,
      enter: this.o.enter ?? { opacity: [0, 1] },
      leave: this.o.exit ?? { opacity: 0 },
    };
  }
  private keep<T extends Playable>(p: T): T {
    this.live.add(p);
    p.then(() => this.live.delete(p));
    return p;
  }

  /** Insert `el` (before `before`, default at the end) and animate it in. */
  add(el: Element, before: Node | null = null): Playable {
    const back = turnAround(el);
    if (back) return back;
    const c = this.container;
    const insert = () => {
      if (isLeaving(el) && el.parentNode === c) {
        // Still animating out from an earlier removal: re-inserting it (in
        // place unless told otherwise) tells flip() it is wanted back.
        c.insertBefore(el, before ?? el.nextSibling);
      } else if (el.parentNode !== c || (before !== null && el.nextSibling !== before)) c.insertBefore(el, before);
      if ((el as HTMLElement).hidden) (el as HTMLElement).hidden = false;
    };
    if (this.o.layout === false) {
      cancelLeave(el);
      insert();
      return this.keep(animate(el, this.o.enter ?? { opacity: [0, 1] }, { duration: 0.4, ease: 'outQuad', immediateRender: true, ...this.anim() }));
    }
    return this.keep(flip(Array.from(c.children), insert, this.flipOpts()));
  }

  /** Animate `el` out, then remove it (or hide it with `after: 'hide'`). */
  remove(el: Element): Playable {
    const how = this.o.after ?? 'remove';
    if (this.o.layout === false) {
      return this.keep(exit(el, this.o.exit ?? { opacity: 0 }, { ...this.anim(), after: how }));
    }
    const kids = Array.from(this.container.children);
    if (!kids.includes(el)) kids.push(el);
    return this.keep(flip(kids, () => (how === 'hide' ? ((el as HTMLElement).hidden = true) : el.remove()), this.flipOpts()));
  }

  /** Show or hide a child (hidden children stay in the DOM). */
  toggle(el: Element, show: boolean): Playable {
    if (show) return this.add(el, el.parentNode === this.container ? el.nextSibling : null);
    const prev = this.o.after;
    this.o.after = 'hide';
    try {
      return this.remove(el);
    } finally {
      this.o.after = prev;
    }
  }

  /**
   * Swap `oldEl` for `newEl`. `mode: 'sync'` (default) runs exit and entrance
   * together; `'wait'` lets the exit finish first.
   */
  async replace(oldEl: Element, newEl: Element, options: { mode?: 'sync' | 'wait' } = {}): Promise<void> {
    const next = oldEl.nextSibling;
    if (options.mode === 'wait') {
      await this.remove(oldEl);
      await this.add(newEl, next && next.parentNode === this.container ? next : null);
      return;
    }
    if (this.o.layout === false) {
      const a = this.remove(oldEl);
      const b = this.add(newEl, next);
      await Promise.all([a, b]);
      return;
    }
    const kids = Array.from(this.container.children);
    await this.keep(flip(kids, () => {
      this.container.insertBefore(newEl, oldEl);
      if (this.o.after === 'hide') (oldEl as HTMLElement).hidden = true;
      else oldEl.remove();
    }, this.flipOpts()));
  }

  /** Jump every running enter/exit to its end (exits complete their removal). */
  kill(): void {
    for (const p of this.live) p.complete();
    this.live.clear();
  }
  /** Same as {@link kill}: the DOM changes requested so far are kept. */
  revert(): void {
    this.kill();
  }
}

/**
 * Manage enter/exit animations for the children of `container`.
 * See {@link Presence} for `add`, `remove`, `toggle` and `replace`.
 */
export function presence(container: Element | string, options?: PresenceOptions): Presence {
  const el = typeof container === 'string' ? document.querySelector(container) : container;
  if (!el) throw new Error(`anikit: presence() container "${String(container)}" not found.`);
  return new Presence(el, options);
}
