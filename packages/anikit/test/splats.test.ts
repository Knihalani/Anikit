import { describe, it, expect, beforeAll } from 'vitest';
import { ready } from '../src/index';
import { splats, packSplats, SPLAT_STRIDE } from '../src/splats';
import { core } from '../src/wasm';

beforeAll(async () => {
  await ready;
});

function splatFile(records: Array<{ p: number[]; s: number[]; rgba: number[]; rot: number[] }>): Uint8Array {
  const buf = new ArrayBuffer(records.length * 32);
  const dv = new DataView(buf);
  records.forEach((r, i) => {
    const o = i * 32;
    [...r.p, ...r.s].forEach((v, k) => dv.setFloat32(o + k * 4, v, true));
    r.rgba.forEach((v, k) => dv.setUint8(o + 24 + k, v));
    r.rot.forEach((v, k) => dv.setUint8(o + 28 + k, v));
  });
  return new Uint8Array(buf);
}

function plyFile(n: number, props: string[], row: (i: number) => number[]): Uint8Array {
  const header = `ply\nformat binary_little_endian 1.0\nelement vertex ${n}\n${props.map((p) => `property float ${p}`).join('\n')}\nend_header\n`;
  const h = new TextEncoder().encode(header);
  const out = new Uint8Array(h.length + n * props.length * 4);
  out.set(h);
  const dv = new DataView(out.buffer, h.length);
  for (let i = 0; i < n; i++) row(i).forEach((v, k) => dv.setFloat32((i * props.length + k) * 4, v, true));
  return out;
}

describe('splats: parsing', () => {
  it('parses .splat records', () => {
    const s = splats.parse(splatFile([
      { p: [1, 2, 3], s: [0.1, 0.2, 0.3], rgba: [255, 0, 0, 128], rot: [255, 128, 128, 128] },
      { p: [-1, 0, 0], s: [1, 1, 1], rgba: [0, 255, 0, 255], rot: [128, 128, 128, 255] },
    ]), 'splat');
    expect(s.count).toBe(2);
    expect(s.data.length).toBe(2 * SPLAT_STRIDE);
    expect(Array.from(s.data.subarray(0, 3))).toEqual([1, 2, 3]);
    expect(s.data[3]).toBeCloseTo(0.1);
    expect(s.data[6]).toBeCloseTo(1, 5); // w
    expect(s.data[13]).toBeCloseTo(128 / 255);
    expect(s.data[SPLAT_STRIDE + 9]).toBeCloseTo(1, 5); // pure z rotation
  });

  it('parses 3DGS .ply (log scales, logit opacity, SH DC colour) and sniffs the format', () => {
    const props = ['x', 'y', 'z', 'f_dc_0', 'f_dc_1', 'f_dc_2', 'opacity', 'scale_0', 'scale_1', 'scale_2', 'rot_0', 'rot_1', 'rot_2', 'rot_3', 'f_rest_0'];
    const s = splats.parse(plyFile(3, props, (i) => [i, i * 2, -i, 0, 1, -1, 0, Math.log(0.5), 0, 0, 0, 0, 0, 2, 9]));
    expect(s.count).toBe(3);
    expect(Array.from(s.data.subarray(SPLAT_STRIDE * 2, SPLAT_STRIDE * 2 + 3))).toEqual([2, 4, -2]);
    expect(s.data[3]).toBeCloseTo(0.5);
    expect(s.data[4]).toBeCloseTo(1);
    expect(Array.from(s.data.subarray(6, 10))).toEqual([0, 0, 0, 1]);
    expect(s.data[10]).toBeCloseTo(0.5);
    expect(s.data[11]).toBeCloseTo(0.5 + 0.2820948);
    expect(s.data[13]).toBeCloseTo(0.5);
  });

  it('rejects bad files with a clear error', () => {
    expect(() => splats.parse(new TextEncoder().encode('ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nend_header\n'))).toThrow(/binary little-endian/);
    expect(() => splats.parse(plyFile(1, ['a', 'b'], () => [0, 0]))).toThrow(/x\/y\/z/);
    expect(() => splats.parse(new Uint8Array(4), 'ksplat')).toThrow(/ksplat/);
  });
});

describe('splats: procedural sources', () => {
  it('fromPoints keeps positions, size and colour', () => {
    const s = splats.fromPoints([0, 0, 0, 1, 2, 3], { size: 0.1, color: '#ff0000', opacity: 0.5 });
    expect(s.count).toBe(2);
    expect(Array.from(s.data.subarray(SPLAT_STRIDE, SPLAT_STRIDE + 3))).toEqual([1, 2, 3]);
    expect(s.data[3]).toBeCloseTo(0.1);
    expect(Array.from(s.data.subarray(10, 14))).toEqual([1, 0, 0, 0.5]);
  });

  it('fromSDF puts flat splats on the surface, oriented along the normal', () => {
    const s = splats.fromSDF((x, y, z) => Math.hypot(x, y, z) - 1, { count: 500 });
    expect(s.count).toBe(500);
    for (let i = 0; i < s.count; i++) {
      const k = i * SPLAT_STRIDE;
      expect(Math.hypot(s.data[k], s.data[k + 1], s.data[k + 2])).toBeCloseTo(1, 2);
      expect(s.data[k + 5]).toBeLessThan(s.data[k + 3]); // flattened along the normal
    }
  });

  it('fromMesh samples the surface of triangles', () => {
    // A unit square in the z = 2 plane.
    const s = splats.fromMesh([0, 0, 2, 1, 0, 2, 1, 1, 2, 0, 1, 2], [0, 1, 2, 0, 2, 3], { count: 300 });
    expect(s.count).toBe(300);
    for (let i = 0; i < s.count; i++) {
      const k = i * SPLAT_STRIDE;
      expect(s.data[k + 2]).toBeCloseTo(2, 5);
      expect(s.data[k]).toBeGreaterThanOrEqual(-1e-6);
      expect(s.data[k]).toBeLessThanOrEqual(1 + 1e-6);
    }
  });

  it('merge concatenates; fromText is empty without a 2D canvas but well-formed', () => {
    const a = splats.fromPoints([0, 0, 0]);
    const m = splats.merge(a, a, a);
    expect(m.count).toBe(3);
    const t = splats.fromText('hi', { count: 10 });
    expect(t.count).toBe(10);
    expect(t.data.length).toBe(10 * SPLAT_STRIDE);
  });
});

describe('splats: packing and sorting', () => {
  it('packs 8 words per splat into 2048-texel rows (centre bits, rgba8, half covariance)', () => {
    const s = splats.fromPoints([1, -2, 3], { size: 1, color: [1, 0, 0], opacity: 1 });
    const p = packSplats(s);
    expect(p.length).toBe(2048 * 4);
    const f = new Float32Array(p.buffer, 0, 3);
    expect(Array.from(f)).toEqual([1, -2, 3]);
    expect(p[3]).toBe((0xff | (0xff << 24)) >>> 0);
    expect(p[4]).toBe(0x3c00); // cov xx = 1, xy = 0
    // Resampling to a larger count repeats splats.
    const p2 = packSplats(s, 3);
    expect(new Float32Array(p2.buffer, 32 * 2, 3)[1]).toBe(-2);
  });

  it('the core depth sort orders back to front', () => {
    const w = core();
    const id = w.ak_gs_sorter_create();
    const zs = [5, -10, 0, 3, -2];
    const ptr = w.ak_gs_sorter_centers(id, zs.length, 0);
    const c = new Float32Array(w.memory.buffer, ptr, zs.length * 3);
    zs.forEach((z, i) => (c[i * 3 + 2] = z));
    const out = w.ak_gs_sort(id, 0, 0, 1, -20, 0);
    expect(Array.from(new Uint32Array(w.memory.buffer, out, zs.length))).toEqual([1, 4, 2, 3, 0]);
    w.ak_gs_sorter_free(id);
  });
});
