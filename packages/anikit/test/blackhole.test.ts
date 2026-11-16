import { describe, it, expect, beforeAll } from 'vitest';
import { ready, blackHole, testing } from '../src/index';

beforeAll(async () => {
  await ready;
});

describe('blackHole', () => {
  it('renders a shadow, a lensed disk and stars', () => {
    const bh = blackHole({ width: 160, height: 100 });
    const px = bh.render({ distance: 30, inclination: 80, fov: 40, time: 1 });
    expect(px.length).toBe(160 * 100 * 4);
    const s = bh.stats;
    expect(s.geometryBuilt).toBe(true);
    expect(s.samples[0]).toBeGreaterThan(1000);
    expect(s.samples[1]).toBeGreaterThan(50);
    // Static observer at 30M: sin ψ = 3√3/30 · √(1 − 2/30).
    expect(s.shadowRadius).toBeCloseTo(Math.asin((Math.sqrt(27) / 30) * Math.sqrt(1 - 2 / 30)), 2);
    const c = (36 * 160 + 80) * 4;
    expect(px[c]).toBeLessThan(15);
    // Animating time reuses the geometry.
    bh.render({ distance: 30, inclination: 80, fov: 40, time: 1.1 });
    expect(bh.stats.geometryBuilt).toBe(false);
    bh.destroy();
  });

  it('falls through the horizon and looks back out', () => {
    const bh = blackHole({ width: 64, height: 48, mode: 'cinematic' });
    const px = bh.render({ distance: 1.5, fall: 1, look: 180, fov: 90 });
    let lit = 0;
    for (let i = 0; i < px.length; i += 4) if (px[i] > 30) lit++;
    expect(lit).toBeGreaterThan(0);
    bh.resize(32, 24);
    expect(bh.render().length).toBe(32 * 24 * 4);
    bh.destroy();
  });

  it('warns about unknown options', () => {
    const w = testing.captureWarnings();
    const bh = blackHole({ width: 16, height: 16, mdoe: 'physical' } as any);
    bh.render({ distnce: 10 } as any);
    expect(w.join('\n')).toMatch(/mdoe.*mode/s);
    expect(w.join('\n')).toMatch(/distnce.*distance/s);
    bh.destroy();
  });
});
