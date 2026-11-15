import { describe, it, expect, beforeAll } from 'vitest';
import { ready, pointCloud, scene3d, engine } from '../src/index';
import { core } from '../src/wasm';

beforeAll(async () => {
  await ready;
});

describe('pointCloud', () => {
  it('settles on a shape and morphs to another', () => {
    const c = pointCloud(200, {
      autoStep: false,
      noise: 0,
      swirl: 0,
      shapes: [(i) => [i, 0, 0], (i) => [0, i, 100]],
    });
    expect(c.positions[3]).toBe(1);
    c.morph(1);
    c.mix = 1;
    for (let k = 0; k < 900; k++) c.step(1 / 60);
    expect(c.positions[3]).toBeCloseTo(0, 1);
    expect(c.positions[4]).toBeCloseTo(1, 1);
    expect(c.positions[5]).toBeCloseTo(100, 1);
    expect(c.speed).toBeLessThan(0.05);
    c.explode(0, 0, 0, 500);
    expect(c.speed).toBeGreaterThan(1);
    c.destroy();
  });

  it('projects through a scene and splats to pixels', () => {
    const c = pointCloud(1000, { autoStep: false, shapes: [(i) => [Math.cos(i) * 50, Math.sin(i) * 50, 0]] });
    const s = scene3d({ width: 64, height: 48, camera: { z: 300 } });
    const p = s.project(c, { sort: false });
    expect(p.count).toBe(1000);
    expect(p.data[3]).toBeGreaterThan(0);
    const w = core();
    const id = w.ak_splat_create(64, 48);
    w.ak_splat_draw(id, s._pid, p.count);
    w.ak_splat_free(id);
    c.destroy();
  });

  it('engine.speed scales frame-callback dt', () => {
    engine.speed = 0.25;
    expect(engine.speed).toBe(0.25);
    engine.speed = 1;
  });
});
