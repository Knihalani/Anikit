import { describe, it, expect, beforeAll } from 'vitest';
import { ready, flightPath, scene3d, mesh, instances, renderer3d, shatter, texture } from '../src/index';

beforeAll(async () => { await ready; });

describe('flightPath', () => {
  it('adds a deterministic handheld shake to flights', () => {
    const path = flightPath([[0, 0, 0], [0, 0, -1000], [0, 0, -2000]], { lookAhead: 200 });
    const cam = scene3d({ width: 64, height: 36 });
    path.apply(cam, 500);
    const still = { ...cam.camera };
    path.apply(cam, 500, 0, { shake: 20 });
    const a = { ...cam.camera };
    path.apply(cam, 500, 0, { shake: 20 });
    expect(cam.camera).toEqual(a); // same input, same pose
    const d = Math.hypot(a.x - still.x, a.y - still.y, a.z - still.z);
    expect(d).toBeGreaterThan(0.5);
    expect(d).toBeLessThan(40);
    path.apply(cam, 520, 0, { shake: 20 });
    expect(Math.hypot(cam.camera.x - a.x, cam.camera.y - a.y)).toBeLessThan(10); // smooth: nearby distances, nearby poses
  });

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

  it('shatters a mesh into shards that start intact and fly apart deterministically', () => {
    const src = mesh.cube();
    const a = shatter(src, { transform: { scale: 100 }, force: 500 });
    expect(a.count).toBe(12);
    // At t = 0 every shard sits on its source triangle: the first vertex of shard 0 lies on the cube surface.
    const d = a.instances.data;
    expect(Math.max(Math.abs(d[9]), Math.abs(d[10]), Math.abs(d[11]))).toBeCloseTo(50, 3);
    a.update(1);
    const moved = Math.hypot(d[9], d[10], d[11]);
    expect(moved).toBeGreaterThan(150);
    const b = shatter(src, { transform: { scale: 100 }, force: 500 }).update(1);
    expect(Array.from(b.instances.data)).toEqual(Array.from(d));
  });

  it('maps textures onto meshes (emissive screens)', () => {
    const data: number[] = [];
    for (let i = 0; i < 64; i++) data.push(...(i < 32 ? [255, 0, 0, 255] : [0, 0, 255, 255]));
    const tex = texture({ width: 64, height: 1, data });
    expect([tex.width, tex.height]).toEqual([64, 1]);
    const r = renderer3d({ width: 32, height: 32 });
    const cam = scene3d({ width: 32, height: 32, camera: { x: 0, y: 400, z: 1 }, target: { x: 0, y: 0, z: 0 } });
    const px = r.render(cam, [{ mesh: mesh.plane(), instances: instances(1, () => ({ scale: 2000, color: [0, 0, 0] })), material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: tex, textureMode: 'emissive' } }], {});
    let red = 0, blue = 0;
    for (let i = 0; i < px.length; i += 4) { if (px[i] > 150 && px[i + 2] < 80) red++; if (px[i + 2] > 150 && px[i] < 80) blue++; }
    expect(red).toBeGreaterThan(20);
    expect(blue).toBeGreaterThan(20);
    expect(() => texture({ width: 4, height: 4, data: [1, 2, 3] })).toThrow(/expected 64/);
  });

  it('reflects the scene in a glossy mirror surface', () => {
    const r = renderer3d({ width: 48, height: 48 });
    const cam = scene3d({ width: 48, height: 48, camera: { x: 0, y: 60, z: 400 }, target: { x: 0, y: 0, z: 0 } });
    const items = [
      { mirror: true, mesh: mesh.plane(), instances: instances(1, () => ({ y: -50, scale: 4000, color: [0.02, 0.02, 0.02] })), material: { specular: 0, rim: 0 } },
      { mesh: mesh.cube(), instances: instances(1, () => ({ y: 20, scale: 80, color: [1, 0, 0] })), material: { emissive: 1 } },
    ];
    const red = (px: Uint8ClampedArray) => { let n = 0; for (let y = 36; y < 48; y++) for (let x = 0; x < 48; x++) if (px[(y * 48 + x) * 4] > 60) n++; return n; };
    const plain = red(r.render(cam, items, {}).slice());
    const mirrored = red(r.render(cam, items, { mirror: { y: -50, strength: 0.6 } }).slice());
    expect(mirrored).toBeGreaterThan(plain + 5); // the red cube shows up in the floor below it
  });

  it('builds open tubes along paths (trails)', () => {
    const path = flightPath([[0, 0, 0], [100, 0, -500], [0, 50, -1000]]);
    const pts = path.points(40);
    expect(pts.length).toBe(40);
    const tube = mesh.tube(pts, { radius: 5, sides: 6, taper: [0, 1] });
    expect(tube.triangles).toBe(39 * 6 * 2);
    expect(mesh.tube([[0, 0, 0], [0, 0, -10]]).vertices).toBe(2 * 9);
  });

  it('paints the sky, casts shadows, blurs out of focus, lights with point lights', () => {
    const W = 48, H = 48;
    const r = renderer3d({ width: W, height: H });
    const cam = scene3d({ width: W, height: H, camera: { x: 0, y: 300, z: 600 }, target: { x: 0, y: 0, z: 0 } });
    const floor = { mesh: mesh.plane(), instances: instances(1, () => ({ y: -50, scale: 3000, color: [0.8, 0.8, 0.8] })), material: { specular: 0, rim: 0 } };
    const box = { mesh: mesh.cube(), instances: instances(1, () => ({ y: 60, scale: 100, color: [0.8, 0.8, 0.8] })), material: { specular: 0, rim: 0 } };
    const sum = (px: Uint8ClampedArray, ch = 0) => { let s = 0; for (let i = ch; i < px.length; i += 4) s += px[i]; return s; };
    // Sky fills the background only.
    const plain = r.render(cam, [floor, box], { background: '#000000' }).slice();
    const sky = r.render(cam, [floor, box], { background: '#000000', skybox: true, sky: [0, 0, 1], horizon: [0, 0, 1], ground: [0, 0, 1] }).slice();
    expect(sum(sky, 2)).toBeGreaterThan(sum(plain, 2));
    expect(sky[(40 * W + 24) * 4]).toBe(plain[(40 * W + 24) * 4]); // floor pixel unchanged
    // Shadows darken the floor under the box.
    const lit = sum(r.render(cam, [floor, box], { key: [0, 1, 0] }).slice());
    const shaded = sum(r.render(cam, [floor, box], { key: [0, 1, 0], shadows: { extent: 600, center: [0, 0, 0] } }).slice());
    expect(shaded).toBeLessThan(lit);
    // Point lights add coloured light.
    const dark = { ...floor, material: { ambient: 0, diffuse: 1, specular: 0, rim: 0 } };
    const noLight = sum(r.render(cam, [dark], { key: [0, -1, 0], fill: [0, -1, 0] }).slice(), 1);
    const withLight = sum(r.render(cam, [dark], { key: [0, -1, 0], fill: [0, -1, 0], lights: [{ position: [0, 50, 0], color: [0, 1, 0], radius: 300, intensity: 2 }] }).slice(), 1);
    expect(withLight).toBeGreaterThan(noLight + 1000);
    // Depth of field lowers contrast when nothing is in focus.
    const lines = { ...floor, material: { lines: 3, grid: [40, 40] as [number, number], lineWidth: 0.1 } };
    const edges = (px: Uint8ClampedArray) => { let e = 0; for (let y = 0; y < H; y++) for (let x = 0; x + 1 < W; x++) e += Math.abs(px[(y * W + x) * 4] - px[(y * W + x + 1) * 4]); return e; };
    const sharp = edges(r.render(cam, [lines], {}).slice());
    const soft = edges(r.render(cam, [lines], { dof: { focus: 30000, aperture: 30, max: 8 } }).slice());
    expect(sharp).toBeGreaterThan(0);
    expect(soft).toBeLessThan(sharp);
  });

  it('applies camera motion blur only when the camera moved', () => {
    const r = renderer3d({ width: 48, height: 48 });
    const cam = scene3d({ width: 48, height: 48, camera: { x: 0, y: 0, z: 400 }, target: { x: 0, y: 0, z: 0 } });
    const items = [{ mesh: mesh.cube(), instances: instances(1, () => ({ scale: 120, color: [1, 1, 1] })), material: { ambient: 1, diffuse: 0, specular: 0, rim: 0, lines: 2, grid: [4, 4] as [number, number] } }];
    const still = r.render(cam, items, {}).slice();
    const prevCamera = cam.viewProjection().matrix;
    expect(Array.from(r.render(cam, items, { motionBlur: true, prevCamera }))).toEqual(Array.from(still));
    cam.camera.x = 60; cam.target.x = 60;
    const moved = r.render(cam, items, {}).slice();
    const blurred = r.render(cam, items, { motionBlur: { shutter: 1 }, prevCamera }).slice();
    expect(Array.from(blurred)).not.toEqual(Array.from(moved));
  });
});
