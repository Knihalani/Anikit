import { describe, it, expect, beforeAll } from 'vitest';
import { ready, flightPath, scene3d, mesh, instances, renderer3d } from '../src/index';

beforeAll(async () => { await ready; });

describe('flightPath', () => {
  it('passes through its points with arc-length parameterisation', () => {
    const p = flightPath([[0, 0, 0], [0, 0, -1000], [0, 0, -2000]]);
    expect(p.length).toBeCloseTo(2000, 0);
    expect(p.point(0)[2]).toBeCloseTo(0, 3);
    expect(p.point(1000)[2]).toBeCloseTo(-1000, 0);
    expect(p.point(5000)[2]).toBeCloseTo(-2000, 3); // clamped
    const pose = p.at(500, { lookAhead: 200 });
    expect(pose.target[2]).toBeCloseTo(-700, 0);
    expect(pose.roll).toBeCloseTo(0, 5); // straight: level
  });

  it('banks into turns and times smoothly and monotonically', () => {
    const right = flightPath([[0, 0, 0], [0, 0, -1000], [1000, 0, -2000], [2000, 0, -2000]]);
    const left = flightPath([[0, 0, 0], [0, 0, -1000], [-1000, 0, -2000], [-2000, 0, -2000]]);
    const r = right.at(1500).roll, l = left.at(1500).roll;
    expect(Math.abs(r)).toBeGreaterThan(5);
    expect(Math.sign(r)).toBe(-Math.sign(l));
    const s = right.timing([[0, 0], [1, 0.2], [2, 0.9], [3, 1]]);
    let prev = -1;
    for (let t = 0; t <= 3; t += 0.01) { const v = s(t); expect(v).toBeGreaterThanOrEqual(prev - 1e-9); prev = v; }
    expect(s(3)).toBeCloseTo(right.length, 6);
    const cam = scene3d({ width: 100, height: 100 });
    right.apply(cam, 100, 30);
    expect(cam.camera.z).toBeCloseTo(-100, 0);
    expect(cam.roll).toBeCloseTo(30 + right.at(100).roll, 6);
  });
});

describe('mesh builders', () => {
  it('builds planes, cylinders and heightfields that render', () => {
    expect(mesh.plane([4, 2]).triangles).toBe(16);
    expect(mesh.cylinder(16, false).triangles).toBe(32);
    expect(mesh.cylinder(16, true).triangles).toBe(64);
    const hf = mesh.heightfield(5, 5, (x, z) => (x > 0.4 && z > 0.4 ? 0.2 : 0), { skip: 0 });
    expect(hf.triangles).toBeGreaterThan(0);
    expect(hf.triangles).toBeLessThan(32); // flat cells cut out
    const r = renderer3d({ width: 32, height: 32 });
    const cam = scene3d({ width: 32, height: 32, camera: { x: 0, y: 400, z: 1 }, target: { x: 0, y: 0, z: 0 } });
    const px = r.render(cam, [{ mesh: mesh.plane(4), instances: instances(1, () => ({ scale: 2000, color: [1, 1, 1] })) }], {});
    expect(px[(16 * 32 + 16) * 4]).toBeGreaterThan(40);
  });

  it('adds light with additive blending', () => {
    const r = renderer3d({ width: 16, height: 16 });
    const cam = scene3d({ width: 16, height: 16, camera: { z: 300 } });
    const inst = instances(1, () => ({ scale: 400, color: [0.3, 0, 0] }));
    const base = r.render(cam, [], { background: '#002000' }).slice();
    const lit = r.render(cam, [{ mesh: mesh.cube(), instances: inst, blend: 'additive', material: { ambient: 1, diffuse: 0, specular: 0, rim: 0 } }], { background: '#002000' });
    const c = (8 * 16 + 8) * 4;
    expect(lit[c + 1]).toBe(base[c + 1]); // background green kept
    expect(lit[c]).toBeGreaterThan(50); // red added
  });

  it('draws light points that streak with camera motion', () => {
    const r = renderer3d({ width: 64, height: 64 });
    const cam = scene3d({ width: 64, height: 64, camera: { x: 0, y: 0, z: 300 }, target: { x: 0, y: 0, z: 0 } });
    const pts = { points: [0, 0, 0], size: 1, intensity: 3 };
    const lit = (px: Uint8ClampedArray) => { let n = 0; for (let i = 0; i < px.length; i += 4) if (px[i] > 0) n++; return n; };
    const still = lit(r.render(cam, [pts], {}).slice());
    expect(still).toBeGreaterThan(0);
    cam.camera.x = 40; cam.target.x = 40;
    const moving = lit(r.render(cam, [{ ...pts, streak: 1 }], {}).slice());
    expect(moving).toBeGreaterThan(still * 2);
  });
});
