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
