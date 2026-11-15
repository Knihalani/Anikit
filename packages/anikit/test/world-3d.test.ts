import { describe, it, expect, beforeAll } from 'vitest';
import { world, scene3d, batch, animate, parseTransform, mixTransforms, toMatrix3d, engine } from '../src/index';
import { setup, advance, box } from './setup';
import { core } from '../src/wasm';

beforeAll(setup);

describe('physics world', () => {
  it('drops bodies onto the floor and keeps them apart', () => {
    const w = world({ bounds: [0, 0, 200, 200], autoStep: false });
    const first = w.addMany(Array.from({ length: 50 }, (_, i) => ({ x: 20 + (i % 10) * 18, y: 20 + Math.floor(i / 10) * 18, r: 6 })));
    expect(first).toBe(0);
    for (let k = 0; k < 600; k++) w.step(1 / 60);
    const d = w.data;
    expect(d.length).toBe(150);
    for (let i = 0; i < 50; i++) {
      expect(d[i * 3 + 1]).toBeLessThanOrEqual(194.01);
      expect(d[i * 3]).toBeGreaterThanOrEqual(5.99);
    }
    expect(Math.max(...Array.from({ length: 50 }, (_, i) => w.y(i)))).toBeGreaterThan(190); // resting on the floor
    w.destroy();
  });

  it('ropes, pins, picking, impulses and auto-stepping', () => {
    const w = world({ gravity: [0, 500], damping: 1 });
    const ids = w.rope([[0, 0], [100, 0]], { segments: 10, pinStart: true });
    expect(ids.length).toBe(11);
    expect(w.links.length).toBe(10);
    advance(3);
    expect(w.x(ids[0])).toBe(0);
    expect(w.y(ids[10])).toBeGreaterThan(60);
    expect(w.pick(w.x(ids[5]), w.y(ids[5]))).toBe(ids[5]);
    expect(w.pick(-500, -500)).toBe(-1);
    const b = w.add(500, 500, 5, 1);
    w.set({ gravity: [0, 0] });
    w.impulse(490, 500, 50, 400);
    advance(1 / 60);
    expect(w.velocity(b)[0]).toBeGreaterThan(100);
    expect(w.linksAlive().every(Boolean)).toBe(true);
    w.destroy();
  });

  it('cloth tears under load', () => {
    const w = world({ gravity: [0, 20000], autoStep: false, iterations: 1, substeps: 1 });
    w.cloth(0, 0, 8, 6, 10, { tear: 1.05 });
    for (let k = 0; k < 120; k++) w.step(1 / 60);
    expect(w.linksAlive().some((a) => !a)).toBe(true);
    w.destroy();
  });
});

describe('3D', () => {
  it('wasm projection + depth sort match the native golden scene bit for bit', () => {
    // Same scene as `projector_golden_hash` in crates/anikit-core/src/space.rs:
    // the wasm build takes the SIMD projection path, the native test the scalar one.
    const w = core();
    const n = 1001;
    const pid = w.ak_projector_create();
    const pts = new Float32Array(w.memory.buffer, w.ak_projector_points(pid, n * 3), n * 3);
    const f = (i: number, k: number, m: number) => ((i * k) % m) / m;
    for (let i = 0; i < n; i++) {
      pts[i * 3] = (f(i, 7919, 997) - 0.5) * 900;
      pts[i * 3 + 1] = (f(i, 104729, 991) - 0.5) * 700;
      pts[i * 3 + 2] = i % 50 === 7 ? 995 : i % 61 === 3 ? 1500 : (f(i, 31, 89) - 0.5) * 600;
    }
    pts[15] = NaN;
    pts[29] = Infinity;
    const model = [0.94, 0, -0.34, 0, 0, 1, 0, 0, 0.34, 0, 0.94, 0, 15, 0, 0, 1];
    const vp = [1.7171274, 0, 0, 0, 0, 2.7474039, 0, 0, 0, 0, -1.0033389, -1, 0, 0, 983.3055, 1000];
    const buf = [...model, ...vp];
    new Float64Array(w.memory.buffer, w.ak_f64_buf(32), 32).set(buf);
    const out = w.ak_project(pid, 1, 0, 3, 0, 1, 2, n, 1440, 900, 1);
    const words = [...new Uint32Array(w.memory.buffer, out, n * 4), ...new Uint32Array(w.memory.buffer, w.ak_projector_order(pid), n)];
    let h = 0x811c9dc5;
    for (const v of words) h = Math.imul(h ^ v, 16777619) >>> 0;
    w.ak_projector_free(pid);
    expect(h).toBe(1261226152);
  });

  it('parses, mixes and animates transforms like browsers', () => {
    const m = parseTransform('translate(10px, 20px)');
    expect(m[12]).toBe(10);
    expect(m[13]).toBe(20);
    expect(toMatrix3d(m)).toBe('matrix3d(1,0,0,0,0,1,0,0,0,0,1,0,10,20,0,1)');
    const mid = mixTransforms('rotateY(0deg)', 'rotateY(90deg)', 0.5);
    const c = Math.round(Math.cos(Math.PI / 4) * 1e4) / 1e4;
    expect(mid.startsWith(`matrix3d(${c},0,${-c}`)).toBe(true);
    const el = box();
    animate(el, { transform: ['rotateY(0deg)', 'rotateY(180deg) translateZ(100px)'] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(el.style.transform).toMatch(/^matrix3d\(/);
    advance(1);
    const end = el.style.transform.slice(9, -1).split(',').map(Number);
    expect(end[0]).toBeCloseTo(-1, 3);
    expect(end[14]).toBeCloseTo(-100, 3);
  });

  it('projects and depth-sorts batches and arrays', () => {
    const scene = scene3d({ width: 200, height: 200, fov: 60, camera: { z: 10 } });
    const p = scene.project([0, 0, 0, 0, 0, 5, 0, 0, -5]);
    expect(p.count).toBe(3);
    expect(p.data[0]).toBeCloseTo(100, 3);
    expect(Array.from(p.order)).toEqual([2, 0, 1]);
    const b = batch(100, { x: (i) => [i, i], y: [0, 0], z: (i) => [-i, -i] }, { duration: 1 });
    const q = scene.project(b);
    expect(q.count).toBe(100);
    expect(q.order[0]).toBe(99); // farthest first
    scene.rotation.y = 90;
    const r = scene.project([1, 0, 0]);
    expect(r.data[2]).toBeCloseTo(11, 3); // rotated to z = -1 → depth 11
    scene.destroy();
    b.kill();
  });
});
