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

  it('retargeted controllers resolve at rest, awaited before or after', async () => {
    const o = { x: 0, y: 0 };
    const first = spring(o, { x: 50, y: 10 });
    const early = first.finished; // asked for before any frame
    const done: string[] = [];
    void early.then(() => done.push('early'));
    advance(0.05);
    const mid = spring(o, { x: 80 }, { stiffness: 300, damping: 30 }); // retargets x only
    advance(0.05);
    void mid.then(() => done.push('mid')); // asked for while still moving
    advance(4);
    await Promise.resolve();
    await Promise.resolve();
    expect(o.x).toBe(80);
    expect(done.sort()).toEqual(['early', 'mid']);
    // Already at rest: a controller of a finished spring resolves when asked.
    const rested = spring(o, { x: 80 });
    advance(1);
    await rested;
    // Many retargets that nobody awaits leave nothing behind on the simulation.
    for (let i = 0; i < 100; i++) spring(o, { x: i });
    const last = spring(o, { x: 5 });
    advance(4);
    await last;
    expect(o.x).toBe(5);
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
