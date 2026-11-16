import { describe, it, expect, beforeAll } from 'vitest';
import { marquee, context, engine, testing } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);

/** happy-dom has no layout: give items and the container fixed sizes. */
function row(n: number, item = 100, view = 300) {
  const el = document.createElement('div');
  el.style.color = 'red';
  for (let i = 0; i < n; i++) {
    const it = document.createElement('span');
    it.textContent = `item ${i}`;
    it.id = `m${i}`;
    el.appendChild(it);
  }
  document.body.appendChild(el);
  Object.defineProperty(el, 'offsetWidth', { configurable: true, get: () => view });
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, get(this: HTMLElement) { return this.parentElement?.hasAttribute('data-anikit-marquee') ? item : view; } });
  return el;
}
const offset = (el: HTMLElement) => -parseFloat((el.firstElementChild as HTMLElement).style.transform.match(/translate3d\((-?[\d.]+)px/)![1]);

describe('marquee', () => {
  it('clones the content to cover the container twice, moves at speed and wraps seamlessly', () => {
    const el = row(2); // one repeat = 200px; view 300 → 3 copies more
    const m = marquee(el, { speed: 100, smoothing: 0.001 });
    const track = el.firstElementChild as HTMLElement;
    expect(track.hasAttribute('data-anikit-marquee')).toBe(true);
    expect(track.children.length).toBe(2 + 2 * 3);
    const clones = track.querySelectorAll('[data-anikit-clone]');
    expect([...clones].every((c) => c.getAttribute('aria-hidden') === 'true' && !c.id)).toBe(true);
    advance(0.5);
    expect(offset(el)).toBeCloseTo(50, 0);
    advance(1.6); // 210 px travelled → wrapped once
    expect(m.offset).toBeGreaterThanOrEqual(0);
    expect(m.offset).toBeLessThan(200);
    expect(m.offset).toBeCloseTo(10, -1);
    m.revert();
  });

  it('skips style writes off screen and catches up on entry; unchanged resizes keep the clones', () => {
    const G = globalThis as any;
    const prevIO = G.IntersectionObserver, prevRO = G.ResizeObserver;
    const io: Array<(e: Array<{ isIntersecting: boolean }>) => void> = [];
    const ro: Array<() => void> = [];
    G.IntersectionObserver = class { constructor(cb: any) { io.push(cb); } observe() {} disconnect() {} };
    G.ResizeObserver = class { constructor(cb: any) { ro.push(cb); } observe() {} disconnect() {} };
    try {
      const el = row(2);
      const m = marquee(el, { speed: 100, smoothing: 0.001 });
      const track = el.firstElementChild as HTMLElement;
      const first = track.lastElementChild;
      ro[0](); // the observer's first report: same sizes, so no new clones
      expect(track.lastElementChild).toBe(first);
      advance(0.5);
      expect(offset(el)).toBeCloseTo(50, 0);
      io[0]([{ isIntersecting: false }]);
      advance(0.5);
      expect(offset(el)).toBeCloseTo(50, 0); // not written while off screen…
      expect(m.offset).toBeCloseTo(100, 0); // …but still moving
      io[0]([{ isIntersecting: true }]);
      expect(offset(el)).toBeCloseTo(100, 0); // shown at once on entry
      m.measure(); // an explicit measure still re-clones
      expect(track.lastElementChild).not.toBe(first);
      io[0]([{ isIntersecting: false }]);
      advance(0.2);
      m.kill(); // killed off screen: the track shows where it had got to
      expect(offset(el)).toBeCloseTo(m.offset, 1);
      m.revert();
    } finally {
      G.IntersectionObserver = prevIO;
      G.ResizeObserver = prevRO;
    }
  });

  it('reverses with negative speed, eases to a stop on pause, and reverts the markup', () => {
    const el = row(3);
    const m = marquee(el, { speed: -100, smoothing: 0.001 });
    advance(0.25);
    expect(m.offset).toBeGreaterThan(250); // moving backwards from 0 wraps to the end
    m.pause();
    advance(0.2);
    const at = m.offset;
    advance(0.5);
    expect(m.offset).toBeCloseTo(at, 3);
    m.revert();
    expect(el.children.length).toBe(3);
    expect(el.querySelector('[data-anikit-marquee]')).toBeNull();
    expect(el.getAttribute('style')).toBe('color: red;');
  });

  it('is cleaned up by context().revert() and warns about unknown options', () => {
    const el = row(2);
    const warnings = testing.captureWarnings();
    const ctx = context(() => { marquee(el, { speed: 50, sped: 3 } as never); });
    expect(warnings.some((w) => /marquee\(\).*"sped".*speed/.test(w))).toBe(true);
    ctx.revert();
    expect(el.querySelector('[data-anikit-marquee]')).toBeNull();
    expect(engine.inspect()).toBeDefined();
  });
});
