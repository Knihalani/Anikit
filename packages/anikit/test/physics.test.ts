import { describe, it, expect, beforeAll } from 'vitest';
import { spring, inertia } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);

describe('physics', () => {
  it('springs settle exactly and retarget with momentum', async () => {
    const o = { x: 0 };
    const s = spring(o, { x: 100 }, { stiffness: 200, damping: 20 });
    advance(0.1);
    const v = s.velocity.x;
    expect(v).toBeGreaterThan(100);
    spring(o, { x: -100 }, { stiffness: 200, damping: 20 });
    const before = o.x;
    advance(1 / 60);
    expect(o.x).toBeGreaterThan(before); // still moving forward
    advance(3);
    expect(o.x).toBe(-100);
    await s;
  });

  it('perceptual springs (bounce/duration) overshoot', () => {
    const o = { x: 0 };
    spring(o, { x: 100 }, { duration: 0.4, bounce: 0.5 });
    let max = 0;
    for (let i = 0; i < 120; i++) {
      advance(1 / 60);
      max = Math.max(max, o.x);
    }
    expect(max).toBeGreaterThan(105);
    expect(o.x).toBeCloseTo(100, 1);
  });

  it('inertia respects bounds and snapping', () => {
    const o = { x: 0 };
    inertia(o, { x: { velocity: 2000, max: 300 } });
    advance(3);
    expect(o.x).toBe(300);
    const p = { x: 0 };
    inertia(p, { x: { velocity: 500, snap: 100 } });
    advance(5);
    expect(p.x).toBe(100);
  });
});
