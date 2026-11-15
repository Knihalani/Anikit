import { describe, it, expect, beforeAll, vi } from 'vitest';
import { batch, stagger, engine } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);

describe('batch', () => {
  it('animates instances into a contiguous Float32Array', async () => {
    const onUpdate = vi.fn();
    const b = batch(4, { x: (i) => [0, (i + 1) * 10], y: [0, 100] }, { duration: 1, ease: 'linear', delay: stagger(0.5), onUpdate });
    expect(b.stride).toBe(2);
    expect(b.data.length).toBe(8);
    advance(0.5);
    expect(Array.from(b.data.slice(0, 4))).toEqual([5, 50, 0, 0]);
    expect(onUpdate).toHaveBeenCalled();
    advance(3);
    expect(Array.from(b.data)).toEqual([10, 100, 20, 100, 30, 100, 40, 100]);
    expect(b.get(3, 'x')).toBe(40);
    await b;
    const nodes = engine.stats().nodes;
    b.kill();
    expect(engine.stats().nodes).toBe(nodes - 1);
  });

  it('supports keyframes, easing and seeking', () => {
    const b = batch(2, { v: [0, 10, 0] }, { duration: 1, ease: 'linear', autoplay: false });
    b.seek(0.25);
    expect(Array.from(b.data)).toEqual([5, 5]);
    b.progress = 0.5;
    expect(b.get(1, 'v')).toBe(10);
  });
});
