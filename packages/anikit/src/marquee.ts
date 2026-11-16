/**
 * Infinite marquees: a row (or column) of items that scrolls forever without
 * a seam — logos, tickers, testimonials. The content is cloned until it
 * covers the container twice, then the track moves and wraps.
 *
 * ```ts
 * const m = marquee('.logos', { speed: 60, pauseOnHover: true, scrollBoost: 0.4 });
 * m.speed = -120;            // reverse, faster
 * m.pause(); m.play();
 * m.revert();                // remove the clones, restore the markup
 * ```
 */
import { engine, prefersReduced } from './core';
import { track } from './context';
import { checkKeys, checkTargets, warn } from './debug';
import type { OptionKeys } from './debug';
import { toTargets, Targets, isElement } from './props';

export interface MarqueeOptions {
  /** Pixels per second; negative reverses (default 80). */
  speed?: number;
  /** `'x'` (default, moves left for positive speed) or `'y'` (moves up). */
  axis?: 'x' | 'y';
  /** Space between items and between repeats, px (default 0: use CSS `gap` on the container instead). */
  gap?: number;
  /** Slow to a stop while the pointer is over it (default false). */
  pauseOnHover?: boolean;
  /** Speed multiplier while hovered when not pausing (default 1). */
  hoverSpeed?: number;
  /**
   * Speed up with page scrolling: speed × (1 + scrollBoost × px/s of scroll / 1000),
   * following the scroll direction (default 0 = off).
   */
  scrollBoost?: number;
  /** Seconds to ease between speeds (hover, scroll, `m.speed = …`; default 0.4). */
  smoothing?: number;
}
const MARQUEE_KEYS: OptionKeys<MarqueeOptions> = { speed: 0, axis: 0, gap: 0, pauseOnHover: 0, hoverSpeed: 0, scrollBoost: 0, smoothing: 0 };

/** A running marquee: change `speed`, `pause()`/`play()`, `revert()` to restore the markup. */
export class Marquee {
  /** Target speed in px/s (negative reverses). Changes ease in over `smoothing`. */
  speed: number;
  /** Current offset in px within one repeat (0 … repeat length). */
  offset = 0;
  /** The current, eased speed in px/s. */
  velocity = 0;
  private paused = false;
  private hovered = false;
  private boost = 1;
  private off: Array<() => void> = [];
  private track: HTMLElement;
  private clones: HTMLElement[] = [];
  private originalStyle: string | null;
  private length = 0;
  /** On screen (or unknown): off-screen frames move the offset but skip the style write. */
  private visible = true;
  /** The offset the track's transform shows (NaN: not written yet). */
  private shown = NaN;
  /** Container box at the last measure: the resize observer's reports of that same size skip re-cloning. */
  private measured = '';

  constructor(readonly el: HTMLElement, private o: MarqueeOptions) {
    this.speed = o.speed ?? 80;
    this.velocity = this.speed;
    this.originalStyle = el.getAttribute('style');
    const doc = el.ownerDocument;
    // Move the children into a track that can be translated as one.
    this.track = doc.createElement('div');
    this.track.setAttribute('data-anikit-marquee', '');
    const ax = o.axis ?? 'x';
    Object.assign(this.track.style, { display: 'flex', flexDirection: ax === 'x' ? 'row' : 'column', width: ax === 'x' ? 'max-content' : '', willChange: 'transform' });
    if (o.gap) this.track.style.gap = `${o.gap}px`;
    while (el.firstChild) this.track.appendChild(el.firstChild);
    el.appendChild(this.track);
    el.style.overflow = 'hidden';
    this.measure();
    const onEnter = () => (this.hovered = true), onLeave = () => (this.hovered = false);
    if (o.pauseOnHover || (o.hoverSpeed ?? 1) !== 1) {
      el.addEventListener('pointerenter', onEnter);
      el.addEventListener('pointerleave', onLeave);
      this.off.push(() => { el.removeEventListener('pointerenter', onEnter); el.removeEventListener('pointerleave', onLeave); });
    }
    if (o.scrollBoost && typeof window !== 'undefined') {
      let last = window.scrollY, lastT = performance.now();
      const onScroll = () => {
        const t = performance.now(), dt = Math.max(1, t - lastT);
        const v = ((window.scrollY - last) / dt) * 1000;
        last = window.scrollY; lastT = t;
        this.boost = 1 + (o.scrollBoost! * v) / 1000;
      };
      window.addEventListener('scroll', onScroll, { passive: true });
      this.off.push(() => window.removeEventListener('scroll', onScroll));
    }
    if (typeof ResizeObserver !== 'undefined') {
      // The first callback (on observe) reports the size measure() just
      // saw: no new clones for that.
      const ro = new ResizeObserver(() => {
        if (this.box() !== this.measured) this.measure();
      });
      ro.observe(el);
      this.off.push(() => ro.disconnect());
    }
    if (typeof IntersectionObserver !== 'undefined') {
      // The margin flips `visible` before the marquee scrolls into view (the
      // callback comes a frame late), so it never shows a stale offset.
      const io = new IntersectionObserver((entries) => {
        this.visible = entries[entries.length - 1].isIntersecting;
        if (this.visible) this.render();
      }, { rootMargin: '50%' });
      io.observe(el);
      this.off.push(() => io.disconnect());
    }
    this.off.push(engine.onFrame((_t, dt) => this.frame(dt / 1000)));
  }

  /** Re-measure and re-clone (after content or size changes; resizes are observed automatically). */
  measure(): void {
    for (const c of this.clones) c.remove();
    this.clones = [];
    const ax = this.o.axis ?? 'x';
    const size = (e: Element) => (ax === 'x' ? (e as HTMLElement).offsetWidth : (e as HTMLElement).offsetHeight);
    const items = Array.from(this.track.children) as HTMLElement[];
    this.shown = NaN;
    const gap = this.o.gap ?? (parseFloat(getComputedStyle(this.el)[ax === 'x' ? 'columnGap' : 'rowGap']) || 0);
    if (gap && !this.o.gap) this.track.style.gap = `${gap}px`;
    // One repeat = the items plus one gap, so the wrap is seamless.
    this.length = items.reduce((a, e) => a + size(e), 0) + gap * items.length;
    const view = size(this.el);
    this.measured = this.box();
    if (!(this.length > 0)) return;
    const copies = Math.max(1, Math.ceil((view * 2) / this.length));
    for (let k = 0; k < copies; k++) {
      for (const it of items) {
        const c = it.cloneNode(true) as HTMLElement;
        c.setAttribute('aria-hidden', 'true');
        c.removeAttribute('id');
        c.setAttribute('data-anikit-clone', '');
        this.track.appendChild(c);
        this.clones.push(c);
      }
    }
    this.render();
  }
  /** The container's box (read with the other sizes, before any clone is added). */
  private box(): string {
    return `${this.el.offsetWidth}x${this.el.offsetHeight}`;
  }
  /** Stop moving (eases to a stop over `smoothing`). */
  pause(): this {
    this.paused = true;
    return this;
  }
  /** Start moving again. */
  play(): this {
    this.paused = false;
    return this;
  }
  /** Stop updating; keep the current markup and position. */
  kill(): void {
    for (const f of this.off.splice(0)) f();
    // Killed off screen: leave the track where the marquee had got to.
    this.visible = true;
    this.render();
  }
  /** Stop, remove the clones and the track, and restore the container. */
  revert(): void {
    this.kill();
    for (const c of this.clones) c.remove();
    this.clones = [];
    while (this.track.firstChild) this.el.insertBefore(this.track.firstChild, this.track);
    this.track.remove();
    if (this.originalStyle === null) this.el.removeAttribute('style');
    else this.el.setAttribute('style', this.originalStyle);
  }

  private frame(dt: number): void {
    if (!this.length || dt <= 0) return;
    const hover = this.hovered ? (this.o.pauseOnHover ? 0 : this.o.hoverSpeed ?? 1) : 1;
    const reduced = prefersReduced();
    const target = this.paused || reduced ? 0 : this.speed * hover * this.boost;
    const k = 1 - Math.exp(-dt / Math.max(1e-3, (this.o.smoothing ?? 0.4) / 3));
    // Reduced motion stops at once: easing down would still move it.
    this.velocity = reduced ? 0 : this.velocity + (target - this.velocity) * k;
    this.boost += (1 - this.boost) * (1 - Math.exp(-dt * 4)); // scroll boost dies away
    this.offset = (((this.offset + this.velocity * dt) % this.length) + this.length) % this.length;
    this.render();
  }
  private render(): void {
    if (!this.visible || this.offset === this.shown) return;
    this.shown = this.offset;
    const ax = this.o.axis ?? 'x';
    this.track.style.transform = ax === 'x' ? `translate3d(${-this.offset}px,0,0)` : `translate3d(0,${-this.offset}px,0)`;
  }
}

/**
 * Make each target an endless marquee of its children. Returns the controller
 * (or an array for several targets). Registered with `context()`.
 */
export function marquee(targets: Targets, options: MarqueeOptions = {}): Marquee {
  checkKeys('marquee()', options, MARQUEE_KEYS);
  const els = toTargets(targets).filter(isElement) as HTMLElement[];
  checkTargets('marquee()', targets, els.length);
  if (els.length > 1) warn(`marquee(): ${els.length} elements matched; only the first is used. Call marquee() once per row.`);
  return track(new Marquee(els[0], options));
}
