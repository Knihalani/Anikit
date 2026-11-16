import { describe, it, expect, beforeAll } from 'vitest';
import { motionValue, derived, bindValues, scrollProgress, MotionValue, context, testing } from '../src/index';
import { setup, advance, box } from './setup';

beforeAll(setup);

describe('motionValue', () => {
  it('gets, sets and notifies subscribers', () => {
    const v = motionValue(2);
    const seen: number[] = [];
    const off = v.on((x) => seen.push(x));
    v.set(5);
    v.set(5); // unchanged: no notification
    v.set(7);
    off();
    v.set(9);
    expect(v.get()).toBe(9);
    expect(seen).toEqual([5, 7]);
  });

  it('tweens with to() and springs with spring(), keeping velocity on retarget', () => {
    const v = motionValue(0);
    const seen: number[] = [];
    v.on((x) => seen.push(x));
    v.to(100, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(v.get()).toBeGreaterThan(40);
    expect(v.get()).toBeLessThan(60);
    advance(0.6);
    expect(v.get()).toBe(100);
    expect(seen.length).toBeGreaterThan(20);

    v.spring(200, { stiffness: 120, damping: 14 });
    advance(0.2);
    const mid = v.get(), vel = v.velocity;
    expect(mid).toBeGreaterThan(100);
    expect(vel).toBeGreaterThan(0);
    v.spring(300); // retarget mid-flight: continues from where it is
    advance(0.016);
    expect(v.get()).toBeGreaterThan(mid);
    advance(3);
    expect(v.get()).toBeCloseTo(300, 0);
  });

  it('set() stops a running animation', () => {
    const v = motionValue(0);
    v.to(100, { duration: 1 });
    advance(0.2);
    v.set(-5);
    advance(0.5);
    expect(v.get()).toBe(-5);
  });

  it('derives values with map() and derived()', () => {
    const p = motionValue(0);
    const opacity = p.map([0, 0.5, 1], [0, 1, 0]);
    const w = motionValue(2), h = motionValue(3);
    const area = derived([w, h], (a, b) => a * b);
    expect(opacity.get()).toBe(0);
    p.set(0.25);
    expect(opacity.get()).toBeCloseTo(0.5);
    p.set(2); // clamped
    expect(opacity.get()).toBe(0);
    expect(area.get()).toBe(6);
    w.set(4);
    expect(area.get()).toBe(12);
    // Three or more sources (and none) take the general path.
    const d = motionValue(5);
    const vol = derived([w, h, d], (a, b, c) => a * b * c);
    expect(vol.get()).toBe(60);
    h.set(1);
    expect(vol.get()).toBe(20);
    expect(area.get()).toBe(4);
    expect(derived([], () => 7).get()).toBe(7);
  });

  it('binds values to element styles, and reverts', () => {
    const el = box({ color: 'red' });
    const x = motionValue(10), opacity = motionValue(0.5);
    const b = bindValues(el, { x, opacity });
    expect(el.style.transform).toContain('10px');
    expect(el.style.opacity).toBe('0.5');
    x.set(40);
    expect(el.style.transform).toContain('40px');
    b.revert();
    expect(el.style.opacity).toBe('');
    expect(el.style.color).toBe('red');
    x.set(80);
    expect(el.style.transform).toBe('');
  });

  it('is cleaned up by context().revert() and warns on non-values', () => {
    const el = box();
    const v = motionValue(0.3);
    const ctx = context(() => { bindValues(el, { opacity: v }); });
    expect(el.style.opacity).toBe('0.3');
    ctx.revert();
    v.set(0.9);
    expect(el.style.opacity).toBe('');
    const warnings = testing.captureWarnings();
    bindValues(el, { opacity: 0.5 as never });
    expect(warnings.some((w) => /"opacity" is not a motionValue/.test(w))).toBe(true);
  });

  it('scrollProgress() is a motion value fed by scroll()', () => {
    const p = scrollProgress({ offset: ['start start', 'end end'] });
    expect(p).toBeInstanceOf(MotionValue);
    expect(p.get()).toBeGreaterThanOrEqual(0);
    expect(() => p.destroy()).not.toThrow();
  });
});
