// Regression tests for two core bugs: reduced motion 'instant' made a
// scroll-driven (progress-driven) animate() ignore progress, and animate()
// ignored an inline transform written as a function list (translateX(...)).
import { describe, it, expect, beforeAll } from 'vitest';
import { animate, engine, flush, scroll } from '../src/index';
import { setup, advance, box } from './setup';

beforeAll(setup);

const tf = (el: HTMLElement) => el.style.transform;

describe("reduced motion 'instant' and progress", () => {
  it('a paused animation still follows progress (what scroll() sets)', () => {
    engine.reducedMotion = 'always';
    try {
      const o = { x: 0 };
      const a = animate(o, { x: [0, 100] }, { duration: 2, ease: 'linear' });
      a.pause();
      a.progress = 0.25;
      expect(o.x).toBeCloseTo(25, 6);
      a.progress = 0.75;
      expect(o.x).toBeCloseTo(75, 6);
      a.kill();
      // Playing still jumps straight to the end, as before.
      const el = box();
      animate(el, { x: 100 }, { duration: 5, delay: 1, repeat: Infinity });
      advance(0.02);
      expect(tf(el)).toBe('translate(100px,0px)');
    } finally {
      engine.reducedMotion = 'never';
    }
  });

  it('scroll() drives it through its range', () => {
    engine.reducedMotion = 'always';
    const c = document.createElement('div');
    document.body.appendChild(c);
    Object.defineProperty(c, 'scrollHeight', { value: 1100, configurable: true });
    Object.defineProperty(c, 'clientHeight', { value: 100, configurable: true });
    let top = 0;
    Object.defineProperty(c, 'scrollTop', { get: () => top, set: (v) => { top = v; }, configurable: true });
    const o = { x: 0 };
    const a = animate(o, { x: [0, 100] }, { duration: 1, ease: 'linear', autoplay: false });
    const off = scroll(a, { container: c });
    try {
      top = 500;
      c.dispatchEvent(new Event('scroll'));
      advance(0.02);
      flush();
      expect(o.x).toBeCloseTo(50, 6);
    } finally {
      off();
      a.kill();
      engine.reducedMotion = 'never';
    }
  });
});

describe('inline transform function lists', () => {
  it('animate() starts from an inline translateX and keeps the other functions', () => {
    const el = box();
    el.style.transform = 'translateX(50px) rotate(10deg)';
    animate(el, { x: 150 }, { duration: 1, ease: 'linear' });
    advance(0.5, 500);
    expect(tf(el)).toBe('translate(100px,0px) rotate(10deg)');
  });

  it('reads translate / translate3d / scale / units, and leaves unknown lists to the matrix path', () => {
    const a = box();
    a.style.transform = 'translate(10px, 20px) scale(2)';
    animate(a, { y: 120 }, { duration: 1, ease: 'linear' });
    advance(0.5, 500);
    expect(tf(a)).toBe('translate(10px,70px) scale(2,2)');
    const b = box();
    b.style.transform = 'translate3d(0px, 0px, 30px) rotate(0.25turn)';
    animate(b, { rotate: 180 }, { duration: 1, ease: 'linear' });
    advance(1, 500);
    expect(tf(b)).toContain('rotate(180deg)');
    expect(tf(b)).toContain('30px');
    const c = box();
    c.style.transform = 'translateX(25%)';
    animate(c, { y: 10 }, { duration: 1, ease: 'linear' });
    advance(1, 500);
    expect(tf(c)).toContain('25%');
  });
});
