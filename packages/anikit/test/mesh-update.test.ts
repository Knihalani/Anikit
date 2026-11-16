import { describe, it, expect, beforeAll } from 'vitest';
import { ready, composition, mesh, instances, renderer3d, scene3d, testing } from '../src/index';

beforeAll(async () => { await ready; });

const cam = () => scene3d({ width: 48, height: 32, camera: { z: 300 } });
const tri = () => ({ pos: new Float32Array([-60, -40, 0, 60, -40, 0, 0, 50, 0]), nrm: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1]) });
const hash = (px: ArrayLike<number>) => { let h = 2166136261; for (let i = 0; i < px.length; i++) h = Math.imul(h ^ px[i], 16777619); return h >>> 0; };

describe('mesh.update / free / tube({ into })', () => {
  it('update() moves vertices in place: same pixels as a fresh mesh with those vertices', () => {
    const { pos, nrm } = tri();
    const m = mesh.custom(pos, nrm, [0, 1, 2]);
    const r = renderer3d({ width: 48, height: 32 }), c = cam();
    const first = hash(r.render(c, [{ mesh: m, cull: false }]));
    pos[7] = -10; // pull the top vertex down
    m.update(pos);
    const moved = hash(r.render(c, [{ mesh: m, cull: false }]));
    const fresh = mesh.custom(pos, nrm, [0, 1, 2]);
    expect(moved).toBe(hash(r.render(c, [{ mesh: fresh, cull: false }])));
    expect(moved).not.toBe(first);
    m.free(); fresh.free();
  });

  it('update() checks sizes and refuses freed meshes', () => {
    const { pos, nrm } = tri();
    const m = mesh.custom(pos, nrm, [0, 1, 2]);
    expect(() => m.update(new Float32Array(6))).toThrow(/expected 9/);
    expect(() => m.update(pos, new Float32Array(3))).toThrow(/normal/);
    expect(() => m.update(pos, nrm, new Float32Array(5))).toThrow(/uv/);
    m.free();
    expect(m.freed).toBe(true);
    expect(() => m.update(pos)).toThrow(/freed/);
  });

  it('update(other) takes another mesh of any size and frees it; sort: near sees the new bounds', () => {
    const small = mesh.sphere(4, 6), big = mesh.sphere(12, 16);
    const nBig = big.triangles;
    big.aspect = 0.3; // e.g. from mesh.text()
    small.update(big);
    expect(small.triangles).toBe(nBig);
    expect(small.aspect).toBe(0.3);
    expect(big.freed).toBe(true);
    expect(() => small.update(big)).toThrow(/freed/);
    const r = renderer3d({ width: 48, height: 32 }), c = cam();
    const inst = instances(1, () => ({ scale: 40 }));
    expect(hash(r.render(c, [{ mesh: small, instances: inst }], { sort: 'near' }))).toBe(hash(r.render(c, [{ mesh: small, instances: inst }])));
    small.free();
  });

  it('free() twice does not free a mesh that reused the slot; a freed mesh is not drawn', () => {
    const a = mesh.cube();
    a.free();
    const b = mesh.cube(); // likely reuses a's slot
    a.free(); // must not free b
    expect(b.triangles).toBe(12);
    const r = renderer3d({ width: 48, height: 32 }), c = cam();
    const inst = instances(1, () => ({ scale: 80 }));
    const empty = hash(r.render(c, []));
    expect(hash(r.render(c, [{ mesh: a, instances: inst }]))).toBe(empty);
    expect(hash(r.render(c, [{ mesh: a, instances: inst }], { shadows: { extent: 200, size: 64 } }))).toBe(hash(r.render(c, [], { shadows: { extent: 200, size: 64 } })));
    expect(hash(r.render(c, [{ mesh: b, instances: inst }]))).not.toBe(empty);
    b.free();
  });

  it('tube({ into }) rebuilds in place (any point count) with the same pixels as a new tube', () => {
    const pts = (k: number, n: number) => Array.from({ length: n }, (_, i): [number, number, number] => [i * 20 - 60, Math.sin(i + k) * 20, 0]);
    const hair = mesh.tube(pts(0, 6), { radius: 6 });
    const same = mesh.tube(pts(1, 9), { radius: 6, into: hair });
    expect(same).toBe(hair);
    const ref = mesh.tube(pts(1, 9), { radius: 6 });
    expect(hair.triangles).toBe(ref.triangles);
    const r = renderer3d({ width: 48, height: 32 }), c = cam();
    expect(hash(r.render(c, [{ mesh: hair }], { sort: 'near' }))).toBe(hash(r.render(c, [{ mesh: ref }], { sort: 'near' })));
    hair.free(); ref.free();
  });

  it('no churn warning for a film that animates geometry in place, or frees what it replaces', async () => {
    const warnings = testing.captureWarnings();
    const cv: any = { width: 4, height: 2, getContext: () => ({ fillRect() {}, getImageData: () => ({ data: new Uint8ClampedArray(32) }) }) };
    const { pos, nrm } = tri();
    const sail = mesh.custom(pos, nrm, [0, 1, 2]);
    const hair = mesh.tube([[0, 0, 0], [1, 1, 1]]);
    let old = mesh.cube();
    const c = composition({ width: 4, height: 2, fps: 10, duration: 0.6 });
    c.scene('inplace', 0, 0.6, {
      draw: ({ t }) => {
        pos[1] = t; sail.update(pos);
        mesh.tube([[0, 0, 0], [1, t, 1], [2, 0, 0]], { into: hair });
        old.free(); old = mesh.cube();
      },
    });
    await c.render({ canvas: cv, onFrame: () => {} });
    expect(warnings.filter((w) => /in every draw/.test(w))).toHaveLength(0);
    sail.free(); hair.free(); old.free();
  });
});
