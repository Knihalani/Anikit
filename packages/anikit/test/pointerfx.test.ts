import { describe, it, expect, beforeAll } from 'vitest';
import { magnetic, parallax, context, testing } from '../src/index';
import { setup, advance, box } from './setup';

beforeAll(setup);

const tx = (el: HTMLElement) => {
  const m = el.style.transform.match(/translate\((-?[\d.]+)px,\s*(-?[\d.]+)px\)/);
  return m ? [parseFloat(m[1]), parseFloat(m[2])] : [0, 0];
};

describe('magnetic', () => {
  it('leans towards a nearby pointer and springs back when it leaves', () => {
    const el = box();
    el.getBoundingClientRect = () => ({ left: 100, top: 100, width: 100, height: 40, right: 200, bottom: 140, x: 100, y: 100, toJSON() {} }) as DOMRect;
    const fx = magnetic(el, { strength: 0.5, radius: 50 });
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 190, clientY: 120 }));
    advance(1);
    const [x, y] = tx(el);
    expect(x).toBeCloseTo(20, 0); // half of the 40 px offset from the centre (150, 120)
    expect(y).toBeCloseTo(0, 0);
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 600, clientY: 600 }));
    advance(1.5);
    expect(Math.abs(tx(el)[0])).toBeLessThan(0.5);
    fx.revert();
  });

  it('is cleaned up by context() and warns on unknown options', () => {
    const el = box();
    const warnings = testing.captureWarnings();
    const ctx = context(() => { magnetic(el, { strenght: 1 } as never); });
    expect(warnings.some((w) => /magnetic\(\).*"strenght".*strength/.test(w))).toBe(true);
    ctx.revert();
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 0, clientY: 0 }));
    advance(0.5);
    expect(tx(el)).toEqual([0, 0]);
  });
});

describe('parallax', () => {
  it('creates a scroll-linked drift and reverts it', () => {
    const el = box();
    const fx = parallax(el, { speed: -0.5 });
    advance(0.1);
    expect(el.style.transform).toMatch(/translate/);
    fx.revert();
    expect(el.style.transform).toBe('');
  });
});
