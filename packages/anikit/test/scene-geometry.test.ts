import { describe, it, expect } from 'vitest';
import {
  box, roundedBox, sphere, cylinder, capsule, torus, plane, lathe, extrude, tube, triangulate, smoothNormals, subdivide,
  sdfMesh, sdSphere, sdBox, smoothUnion, material, linearColor, testing,
} from '../src/index';
import type { Geometry } from '../src/index';

/** Every normal is unit length and every triangle faces the same way as its normals. */
function check(g: Geometry, outward?: (x: number, y: number, z: number) => [number, number, number]) {
  const p = g.positions, n = g.normals, ix = g.indices;
  expect(n.length).toBe(p.length);
  expect(g.uvs.length).toBe((p.length / 3) * 2);
  for (let i = 0; i < n.length; i += 3) expect(Math.hypot(n[i], n[i + 1], n[i + 2])).toBeCloseTo(1, 4);
  for (const i of ix) expect(i).toBeLessThan(p.length / 3);
  let agree = 0;
  for (let t = 0; t < ix.length; t += 3) {
    const a = ix[t] * 3, b = ix[t + 1] * 3, c = ix[t + 2] * 3;
    const ux = p[b] - p[a], uy = p[b + 1] - p[a + 1], uz = p[b + 2] - p[a + 2];
    const vx = p[c] - p[a], vy = p[c + 1] - p[a + 1], vz = p[c + 2] - p[a + 2];
    const fx = uy * vz - uz * vy, fy = uz * vx - ux * vz, fz = ux * vy - uy * vx;
    if (Math.hypot(fx, fy, fz) < 1e-12) { agree++; continue; }
    const cx = (p[a] + p[b] + p[c]) / 3, cy = (p[a + 1] + p[b + 1] + p[c + 1]) / 3, cz = (p[a + 2] + p[b + 2] + p[c + 2]) / 3;
    const [ox, oy, oz] = outward ? outward(cx, cy, cz) : [n[a] + n[b] + n[c], n[a + 1] + n[b + 1] + n[c + 1], n[a + 2] + n[b + 2] + n[c + 2]];
    if (fx * ox + fy * oy + fz * oz >= 0) agree++;
  }
  expect(agree / (ix.length / 3)).toBeGreaterThan(0.99);
}
const radial = (x: number, y: number, z: number): [number, number, number] => [x, y, z];

describe('scene geometry', () => {
  it('box: 24 vertices, 12 triangles, outward faces', () => {
    const g = box(2, 1, 1);
    expect(g.positions.length / 3).toBe(24);
    expect(g.indices.length / 3).toBe(12);
    check(g, radial);
    expect(Math.max(...g.positions.filter((_, i) => i % 3 === 0))).toBeCloseTo(1);
  });
  it('sphere: counts, radius and radial normals', () => {
    const g = sphere(2, 16);
    expect(g.positions.length / 3).toBe(17 * 9);
    expect(g.indices.length / 3).toBe(16 * 8 * 2);
    for (let i = 0; i < g.positions.length; i += 3) {
      expect(Math.hypot(g.positions[i], g.positions[i + 1], g.positions[i + 2])).toBeCloseTo(2, 5);
      expect(g.normals[i]).toBeCloseTo(g.positions[i] / 2, 5);
    }
    check(g, radial);
  });
  it('roundedBox stays inside its bounds with smooth unit normals', () => {
    const g = roundedBox(2, 1, 1, 0.2, 4);
    for (let i = 0; i < g.positions.length; i += 3) {
      expect(Math.abs(g.positions[i])).toBeLessThanOrEqual(1 + 1e-6);
      expect(Math.abs(g.positions[i + 1])).toBeLessThanOrEqual(0.5 + 1e-6);
    }
    check(g, radial);
  });
  it('cylinder, capsule, torus, plane, lathe, tube are consistent', () => {
    check(cylinder(0.5, 0.5, 2, 24));
    check(capsule(0.3, 1.5, 24), radial);
    const t = torus(1, 0.25, 32, 12);
    expect(t.positions.length / 3).toBe(33 * 13);
    check(t);
    const pl = plane(2, 2, 4);
    expect(pl.indices.length / 3).toBe(32);
    for (let i = 1; i < pl.normals.length; i += 3) expect(pl.normals[i]).toBe(1);
    check(pl);
    check(lathe([[0, 0], [1, 0.5], [0.5, 1], [0, 1.2]], 20));
    check(tube((t) => [Math.cos(t * 6), t, Math.sin(t * 6)], 0.1, 40, 8));
  });
  it('extrude triangulates concave outlines and closes the sides', () => {
    const L: Array<[number, number]> = [[0, 0], [2, 0], [2, 1], [1, 1], [1, 2], [0, 2]];
    expect(triangulate(L).length / 3).toBe(4);
    const g = extrude(L, 0.5);
    expect(g.indices.length / 3).toBe(4 * 2 + 6 * 2);
    check(g);
  });
  it('subdivide quadruples triangles and smoothNormals welds seams', () => {
    const g = subdivide(box(1, 1, 1), 1, 0);
    expect(g.indices.length / 3).toBe(48);
    const s = smoothNormals(sphere(1, 12));
    check(s, radial);
  });
  it('sdfMesh: surface nets mesh lies on the zero set', () => {
    const f = (x: number, y: number, z: number) => smoothUnion(sdSphere(x - 0.3, y, z, 0.5), sdBox(x + 0.3, y, z, 0.3, 0.3, 0.3, 0.05), 0.2);
    const g = sdfMesh(f, { resolution: 32, bounds: 1.2 });
    expect(g.indices.length).toBeGreaterThan(1000);
    for (let i = 0; i < g.positions.length; i += 3) expect(Math.abs(f(g.positions[i], g.positions[i + 1], g.positions[i + 2]))).toBeLessThan(0.08);
    check(g);
  });
  it('materials: presets, linear colours and unknown-key warnings', () => {
    const m = material('gold');
    expect(m.metalness).toBe(1);
    expect(linearColor('#ffffff')).toEqual([1, 1, 1]);
    expect(linearColor('#808080')[0]).toBeCloseTo(0.2159, 3);
    const w = testing.captureWarnings();
    material({ roughnes: 0.2 } as never);
    expect(w.join()).toMatch(/roughness/);
  });
});
