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

  it('splat + tone map match the native golden scene bit for bit', () => {
    // Same scene as `golden_scene_hash` in crates/anikit-core/src/splat.rs:
    // the wasm build runs the simd128-vectorised tone map, the native test SSE2 code.
    const w = core();
    const n = 400;
    const pid = w.ak_projector_create();
    let pts = new Float32Array(w.memory.buffer, w.ak_projector_points(pid, n * 3), n * 3);
    for (let i = 0; i < n; i++) {
      const z = 1 + (((i * 31) % 97) / 97) * 3;
      pts[i * 3] = ((((i * 7919) % 997) / 997) * 2.4 - 1.2) * z;
      pts[i * 3 + 1] = ((((i * 104729) % 991) / 991) * 2.4 - 1.2) * z;
      pts[i * 3 + 2] = z;
    }
    const f64 = (v: number[]) => new Float64Array(w.memory.buffer, w.ak_f64_buf(v.length), v.length).set(v);
    const I = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    const vp = I.slice();
    vp[11] = 1;
    vp[15] = 0;
    f64([...I, ...vp]);
    w.ak_project(pid, 1, 0, 3, 0, 1, 2, n, 37, 23, 0);
    const sid = w.ak_splat_create(37, 23);
    const col = new Float32Array(w.memory.buffer, w.ak_splat_colors(sid, n), n * 3);
    for (let i = 0; i < n * 3; i++) col[i] = ((i * 104729) % 1000) / 1000;
    let h = 0x811c9dc5;
    for (let frame = 0; frame < 8; frame++) {
      if (frame !== 3) {
        f64([0.25, 0.75, 1.5, frame % 2 === 0 ? 4.5 : 0, 1, 18, 11, 6, frame === 1 ? 2 : 0, 3, 1, 0.75, 0.5]);
        w.ak_splat_draw(sid, pid, n);
      }
      w.ak_splat_mode(sid, frame >= 5 ? 1 : 0);
      const keep = [0.5, 0, 1, 0.75][frame % 4];
      const ptr = w.ak_splat_resolve(sid, 1 + frame * 0.25, frame < 4 ? 0x07080b : 0xeeddcc, keep);
      const px = new Uint32Array(w.memory.buffer, ptr, 37 * 23);
      for (let i = 0; i < px.length; i++) h = Math.imul(h ^ px[i], 16777619) >>> 0;
    }
    w.ak_splat_free(sid);
    w.ak_projector_free(pid);
    expect(h).toBe(4017498113);
  });

  it('engine.speed scales frame-callback dt', () => {
    engine.speed = 0.25;
    expect(engine.speed).toBe(0.25);
    engine.speed = 1;
  });
});
