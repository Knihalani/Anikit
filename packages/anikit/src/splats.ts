/**
 * Real-time 3D Gaussian splatting on WebGL2, animated by anikit.
 *
 * Splat sets come from files (`.ply` from 3DGS training, antimatter15
 * `.splat`) or are made procedurally (`fromPoints`, `fromMesh`, `fromText`,
 * `fromSDF`), so a scene needs no downloads. Each splat is packed into two
 * `RGBA32UI` texels (centre, colour, 3D covariance as halves), projected to a
 * 2D covariance in the vertex shader and drawn as an instanced quad with
 * Gaussian falloff, premultiplied, back to front. The depth sort is a 16-bit
 * counting sort in the Rust core running in a Web Worker (main-thread
 * fallback), re-run only when the view moves enough.
 *
 * Every effect is a uniform: `layer.fx` and `layer.camera`/`layer.transform`
 * are plain objects, so anikit tweens, springs, timelines and scroll drive
 * them with zero per-frame allocation.
 *
 * ```ts
 * const layer = splats('#c', splats.fromText('anikit', { count: 120_000 }));
 * layer.morph(splats.fromSDF((x, y, z) => Math.hypot(x, y, z) - 1), { duration: 2 });
 * layer.explode({ strength: 1.5, duration: 1.2 });
 * ```
 */
import { core, ready as coreReady, wasmModule } from './wasm';
import { engine } from './core';
import { animate, type Animation } from './animate';
import type { AnimateOptions } from './animate';
import { track } from './context';
import { checkKeys, warn, type OptionKeys } from './debug';
import { parseColor } from './values';
import { textPoints } from './cloud';

/** Floats per splat in a {@link SplatSet}: `x y z sx sy sz qw qx qy qz r g b a`. */
export const SPLAT_STRIDE = 14;

/**
 * A set of Gaussian splats in canonical form: 14 floats per splat
 * (`x y z`, linear scale `sx sy sz`, rotation quaternion `qw qx qy qz`,
 * colour `r g b` and opacity `a` in 0..1).
 */
export interface SplatSet {
  /** Number of splats. */
  readonly count: number;
  /** `count * 14` floats (see {@link SPLAT_STRIDE}). */
  readonly data: Float32Array;
}

/** Colour input for procedural splats: CSS colour, `[r, g, b]` in 0..1, or a function of the splat. */
export type SplatColor = string | [number, number, number] | ((i: number, x: number, y: number, z: number) => [number, number, number]);

/** Options shared by the procedural sources. */
export interface SplatSourceOptions {
  /** Splat radius (one standard deviation) in world units, or a function of the index (default: from the point density). */
  size?: number | ((i: number) => number);
  /** Colour (default `'#8aa2ff'`). */
  color?: SplatColor;
  /** Opacity 0..1 (default 0.9). */
  opacity?: number;
  /** Seed for the random sampling (default 1). */
  seed?: number;
}

/** Live effect uniforms; tween them with `animate(layer.fx, …)` or the helpers. */
export interface SplatFx {
  /** Morph progress 0..1 toward the target set (see `morph()`). */
  morph: number;
  /** 0 = every splat morphs together; higher = rippling, staggered morph (default 0.6). */
  spread: number;
  /** Explosion amount (0 = assembled; 1 ≈ one scene radius outward). */
  explode: number;
  /** Swirl around the vertical axis, in turns. */
  swirl: number;
  /** Dissolve 0..1 (1 = gone). */
  dissolve: number;
  /** Reveal from the bottom up, 0..1 (default 1 = all visible). */
  reveal: number;
  /** Splat size multiplier (default 1). */
  size: number;
  /** Global opacity multiplier (default 1). */
  opacity: number;
}

/** A look-at camera: `x y z` position, `tx ty tz` target, vertical `fov` in degrees. */
export interface SplatCamera {
  /** Camera position x. */
  x: number;
  /** Camera position y. */
  y: number;
  /** Camera position z. */
  z: number;
  /** Look-at target x. */
  tx: number;
  /** Look-at target y. */
  ty: number;
  /** Look-at target z. */
  tz: number;
  /** Vertical field of view in degrees. */
  fov: number;
  /** Near plane (default 0.05). */
  near: number;
  /** Far plane (default 1000). */
  far: number;
}

/** Whole-cloud transform (applied before the camera); rotations in degrees. */
export interface SplatTransform {
  /** Translation x. */
  x: number;
  /** Translation y. */
  y: number;
  /** Translation z. */
  z: number;
  /** Rotation about x, degrees. */
  rotateX: number;
  /** Rotation about y, degrees. */
  rotateY: number;
  /** Rotation about z, degrees. */
  rotateZ: number;
  /** Uniform scale. */
  scale: number;
}

export interface SplatsOptions {
  /** Initial camera (default: looking at the set's centre from 2.6 radii away on +z, fov 45). */
  camera?: Partial<SplatCamera>;
  /** Initial transform. */
  transform?: Partial<SplatTransform>;
  /** Initial effect values. */
  fx?: Partial<SplatFx>;
  /** Clear colour (CSS); default transparent. */
  background?: string | null;
  /** Render automatically every engine frame (default true when the layer owns its canvas). */
  autoRender?: boolean;
  /** Device pixel ratio cap (default `min(devicePixelRatio, 2)`). */
  pixelRatio?: number;
  /** Sort in a Web Worker (default true; falls back to the main thread). */
  worker?: boolean;
  /** View change that triggers a re-sort, as a fraction of the scene radius / radians (default 0.002). */
  sortThreshold?: number;
  /** Drag to orbit, wheel to zoom (default false). */
  orbit?: boolean;
  /** Largest on-screen splat radius in CSS pixels (one standard deviation; default 48): keeps near splats from smearing. */
  maxRadius?: number;
  /**
   * Quality governor: when frames run slower than `fps` (default 50), draw a
   * random subset of the splats (down to `min`, default 0.25), growing the
   * kept ones to cover the gaps, and recover when there is headroom.
   * Default on for layers that render themselves; `false` turns it off.
   */
  governor?: boolean | { fps?: number; min?: number };
}

const OPTION_KEYS: OptionKeys<SplatsOptions> = {
  camera: 0, transform: 0, fx: 0, background: 0, autoRender: 0, pixelRatio: 0, worker: 0, sortThreshold: 0, orbit: 0, maxRadius: 0, governor: 0,
};

/** Morph / effect tween options. */
export interface SplatMorphOptions {
  /** Seconds (default 1.6). */
  duration?: number;
  /** Easing (default `'inOutCubic'`). */
  ease?: AnimateOptions['ease'];
  /** Delay in seconds. */
  delay?: number;
  /** Stagger spread for this morph (default: keep `fx.spread`). */
  spread?: number;
}

/** Options of `explode()`, `dissolve()`, `swirl()` and `reveal()`. */
export interface SplatEffectOptions {
  /** Target amount (explode default 1, dissolve/reveal default 1, swirl default 1 turn). */
  to?: number;
  /** Seconds (default 1.2). */
  duration?: number;
  /** Easing (default `'outCubic'`). */
  ease?: AnimateOptions['ease'];
  /** Delay in seconds. */
  delay?: number;
}

/** Render statistics. */
export interface SplatStats {
  /** Splats in the layer. */
  count: number;
  /** Completed depth sorts. */
  sorts: number;
  /** Duration of the last sort in ms (inside the worker when one is used). */
  sortMs: number;
  /** Frames drawn. */
  frames: number;
  /** Whether sorting runs in a Web Worker. */
  worker: boolean;
  /** Fraction of splats drawn (1 unless the governor is shedding load). */
  quality: number;
}

// ---------------------------------------------------------------------------
// Parsing and packing (core)
// ---------------------------------------------------------------------------

const ERRORS = ['', 'not a PLY file', 'unsupported PLY format (binary little-endian only)', 'PLY has no x/y/z vertex properties', 'PLY is truncated'];

function bytesOf(src: ArrayBuffer | ArrayBufferView): Uint8Array {
  return src instanceof Uint8Array ? src : ArrayBuffer.isView(src) ? new Uint8Array(src.buffer, src.byteOffset, src.byteLength) : new Uint8Array(src);
}

/**
 * Parse `.ply` (3DGS or coloured point cloud, binary little-endian) or
 * `.splat` bytes. `format` defaults to sniffing the `ply` magic.
 */
function parse(src: ArrayBuffer | ArrayBufferView, format?: 'ply' | 'splat' | 'ksplat'): SplatSet {
  const bytes = bytesOf(src);
  const fmt = format ?? (bytes[0] === 0x70 && bytes[1] === 0x6c && bytes[2] === 0x79 ? 'ply' : 'splat');
  if (fmt === 'ksplat') throw new Error('anikit: splats: .ksplat is not supported yet; convert it to .ply or .splat.');
  const w = core();
  const p = w.ak_u8_buf(bytes.length);
  new Uint8Array(w.memory.buffer, p, bytes.length).set(bytes);
  const n = w.ak_gs_parse(fmt === 'ply' ? 0 : 1, bytes.length);
  if (n < 0) throw new Error(`anikit: splats.parse(): ${ERRORS[-n] ?? 'bad file'}.`);
  const ptr = w.ak_gs_data(n);
  return { count: n, data: new Float32Array(w.memory.buffer, ptr, n * SPLAT_STRIDE).slice() };
}

/** Fetch and parse a `.ply` / `.splat` URL. */
async function load(url: string | URL, init?: RequestInit): Promise<SplatSet> {
  const res = await fetch(url, init);
  if (!res.ok) throw new Error(`anikit: splats.load(): ${res.status} ${res.statusText} for ${String(url)}`);
  const buf = await res.arrayBuffer();
  await coreReady;
  const path = String(url).split(/[?#]/)[0].toLowerCase();
  return parse(buf, path.endsWith('.ksplat') ? 'ksplat' : path.endsWith('.splat') ? 'splat' : undefined);
}

/** Pack a set into 8 `u32` words per splat, padded to `rows` rows of 2048 texels. */
export function packSplats(set: SplatSet, n = set.count): Uint32Array {
  const w = core();
  const m = Math.max(set.count, 1);
  const ptr = w.ak_gs_data(n);
  const dst = new Float32Array(w.memory.buffer, ptr, n * SPLAT_STRIDE);
  if (set.count === n) dst.set(set.data);
  else {
    // Remap so a morph between different counts keeps the overall shape.
    for (let i = 0; i < n; i++) {
      const j = set.count ? Math.min(m - 1, Math.floor((i * m) / n)) : 0;
      if (set.count) dst.set(set.data.subarray(j * SPLAT_STRIDE, (j + 1) * SPLAT_STRIDE), i * SPLAT_STRIDE);
    }
  }
  const out = w.ak_gs_pack();
  const rows = Math.max(1, Math.ceil((n * 2) / TEX_W));
  const packed = new Uint32Array(rows * TEX_W * 4);
  packed.set(new Uint32Array(w.memory.buffer, out, n * 8));
  return packed;
}

// ---------------------------------------------------------------------------
// Procedural sources
// ---------------------------------------------------------------------------

function rng(seed: number): () => number {
  let s = (seed >>> 0) || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return (s >>> 0) / 4294967296;
  };
}

function colorFn(c: SplatColor | undefined): (i: number, x: number, y: number, z: number) => [number, number, number] {
  if (typeof c === 'function') return c;
  let rgb: [number, number, number] = [0.54, 0.64, 1];
  if (Array.isArray(c)) rgb = c;
  else if (typeof c === 'string') {
    const p = parseColor(c);
    if (p) rgb = [p[0] / 255, p[1] / 255, p[2] / 255];
    else warn(`splats: cannot parse color "${c}".`);
  }
  return () => rgb;
}

/** Quaternion (w x y z) rotating +z onto unit vector n. */
function quatFromZ(nx: number, ny: number, nz: number, out: Float32Array, o: number): void {
  if (nz < -0.999999) {
    out[o] = 0; out[o + 1] = 1; out[o + 2] = 0; out[o + 3] = 0;
    return;
  }
  // q = normalize(1 + z·n, z × n)
  const w = 1 + nz;
  const x = -ny;
  const y = nx;
  const l = Math.hypot(w, x, y) || 1;
  out[o] = w / l; out[o + 1] = x / l; out[o + 2] = y / l; out[o + 3] = 0;
}

function build(
  count: number,
  o: SplatSourceOptions,
  pos: (i: number, out: Float32Array, k: number) => void,
  flat?: (i: number) => [number, number, number] | null,
): SplatSet {
  const data = new Float32Array(count * SPLAT_STRIDE);
  const col = colorFn(o.color);
  const size = o.size ?? 0.015;
  const op = o.opacity ?? 0.9;
  for (let i = 0; i < count; i++) {
    const k = i * SPLAT_STRIDE;
    pos(i, data, k);
    const s = typeof size === 'function' ? size(i) : size;
    const n = flat?.(i);
    if (n) {
      data[k + 3] = s; data[k + 4] = s; data[k + 5] = s * 0.15;
      quatFromZ(n[0], n[1], n[2], data, k + 6);
    } else {
      data[k + 3] = s; data[k + 4] = s; data[k + 5] = s;
      data[k + 6] = 1;
    }
    const c = col(i, data[k], data[k + 1], data[k + 2]);
    data[k + 10] = c[0]; data[k + 11] = c[1]; data[k + 12] = c[2]; data[k + 13] = op;
  }
  return { count, data };
}

/** Isotropic splats at `x y z` triples. */
function fromPoints(points: ArrayLike<number>, o: SplatSourceOptions = {}): SplatSet {
  const n = Math.floor(points.length / 3);
  if (o.size === undefined && n > 1) {
    // Default scale from the point density: about half the mean spacing of a
    // surface sampled by n points over the bounding box, so clouds look crisp.
    let lo = Infinity, hi = -Infinity;
    for (let i = 0; i < n * 3; i++) { const v = points[i]; if (v < lo) lo = v; if (v > hi) hi = v; }
    const ext = hi - lo || 1;
    o = { ...o, size: Math.min(ext * 0.05, Math.max(ext * 1e-4, (ext * 0.5) / Math.sqrt(n))) };
  }
  return build(n, o, (i, d, k) => {
    d[k] = points[i * 3]; d[k + 1] = points[i * 3 + 1]; d[k + 2] = points[i * 3 + 2];
  });
}

/**
 * Sample `count` flat splats on a triangle mesh's surface (area-weighted),
 * oriented along each face. `indices` default to consecutive triangles.
 */
function fromMesh(positions: ArrayLike<number>, indices: ArrayLike<number> | null, o: SplatSourceOptions & { count?: number } = {}): SplatSet {
  const tris = indices ? Math.floor(indices.length / 3) : Math.floor(positions.length / 9);
  const idx = (t: number, c: number) => (indices ? indices[t * 3 + c] : t * 3 + c);
  const cdf = new Float64Array(tris);
  const normals = new Float32Array(tris * 3);
  let total = 0;
  for (let t = 0; t < tris; t++) {
    const a = idx(t, 0) * 3, b = idx(t, 1) * 3, c = idx(t, 2) * 3;
    const ux = positions[b] - positions[a], uy = positions[b + 1] - positions[a + 1], uz = positions[b + 2] - positions[a + 2];
    const vx = positions[c] - positions[a], vy = positions[c + 1] - positions[a + 1], vz = positions[c + 2] - positions[a + 2];
    const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
    const l = Math.hypot(nx, ny, nz);
    total += l / 2;
    cdf[t] = total;
    if (l > 0) { normals[t * 3] = nx / l; normals[t * 3 + 1] = ny / l; normals[t * 3 + 2] = nz / l; }
  }
  const count = o.count ?? 50_000;
  if (!tris || total <= 0) return { count: 0, data: new Float32Array(0) };
  const r = rng(o.seed ?? 1);
  const face = new Int32Array(count);
  return build(
    count,
    { size: Math.sqrt(total / count) * 0.6, ...o },
    (i, d, k) => {
      const x = r() * total;
      let lo = 0, hi = tris - 1;
      while (lo < hi) { const m = (lo + hi) >> 1; if (cdf[m] < x) lo = m + 1; else hi = m; }
      face[i] = lo;
      let u = r(), v = r();
      if (u + v > 1) { u = 1 - u; v = 1 - v; }
      const a = idx(lo, 0) * 3, b = idx(lo, 1) * 3, c = idx(lo, 2) * 3;
      for (let j = 0; j < 3; j++) d[k + j] = positions[a + j] + u * (positions[b + j] - positions[a + j]) + v * (positions[c + j] - positions[a + j]);
    },
    (i) => [normals[face[i] * 3], normals[face[i] * 3 + 1], normals[face[i] * 3 + 2]],
  );
}

/**
 * Splats filling text (rendered with a 2D canvas; empty outside the DOM).
 * `width` is the text width in world units (default 2), `depth` its thickness.
 */
function fromText(text: string, o: SplatSourceOptions & { count?: number; width?: number; depth?: number; font?: string; weight?: number | string } = {}): SplatSet {
  const count = o.count ?? 60_000;
  const width = o.width ?? 2;
  const pts = textPoints(text, count, { width: 1000, depth: ((o.depth ?? 0.1) * 1000) / width, font: o.font, weight: o.weight });
  const k = width / 1000;
  for (let i = 0; i < pts.length; i++) pts[i] *= k;
  return fromPoints(pts, { size: Math.max(0.004, (width / Math.sqrt(count)) * 0.18), ...o });
}

/**
 * Splats on the zero level set of a signed distance function, flattened
 * along its gradient. `bounds` is the half-size of the sampling cube
 * (default 1.5), centred on the origin.
 */
function fromSDF(sdf: (x: number, y: number, z: number) => number, o: SplatSourceOptions & { count?: number; bounds?: number } = {}): SplatSet {
  const count = o.count ?? 50_000;
  const B = o.bounds ?? 1.5;
  const r = rng(o.seed ?? 1);
  const pos = new Float32Array(count * 3);
  const nrm = new Float32Array(count * 3);
  const e = B * 1e-4;
  const tol = B * 2e-3;
  let found = 0;
  for (let tries = 0; found < count && tries < count * 50; tries++) {
    let x = (r() * 2 - 1) * B, y = (r() * 2 - 1) * B, z = (r() * 2 - 1) * B;
    let gx = 0, gy = 0, gz = 0, f = 0;
    for (let it = 0; it < 6; it++) {
      f = sdf(x, y, z);
      gx = (sdf(x + e, y, z) - f) / e; gy = (sdf(x, y + e, z) - f) / e; gz = (sdf(x, y, z + e) - f) / e;
      const g2 = gx * gx + gy * gy + gz * gz;
      if (!(g2 > 1e-12)) break;
      x -= (f * gx) / g2; y -= (f * gy) / g2; z -= (f * gz) / g2;
    }
    f = sdf(x, y, z);
    if (!(Math.abs(f) < tol) || Math.abs(x) > B || Math.abs(y) > B || Math.abs(z) > B) continue;
    const l = Math.hypot(gx, gy, gz) || 1;
    pos.set([x, y, z], found * 3);
    nrm.set([gx / l, gy / l, gz / l], found * 3);
    found++;
  }
  if (found < count) warn(`splats.fromSDF(): found ${found} of ${count} surface points; check that the surface lies within ±${B}.`);
  return build(
    found,
    { size: B * 0.9 / Math.sqrt(Math.max(found, 1)), ...o },
    (i, d, k) => { d[k] = pos[i * 3]; d[k + 1] = pos[i * 3 + 1]; d[k + 2] = pos[i * 3 + 2]; },
    (i) => [nrm[i * 3], nrm[i * 3 + 1], nrm[i * 3 + 2]],
  );
}

/** Concatenate sets. */
function merge(...sets: SplatSet[]): SplatSet {
  const count = sets.reduce((a, s) => a + s.count, 0);
  const data = new Float32Array(count * SPLAT_STRIDE);
  let o = 0;
  for (const s of sets) { data.set(s.data.subarray(0, s.count * SPLAT_STRIDE), o); o += s.count * SPLAT_STRIDE; }
  return { count, data };
}

// ---------------------------------------------------------------------------
// Sorting: Web Worker running the core's counting sort
// ---------------------------------------------------------------------------

const WORKER_SRC = `
let W, id, ready;
onmessage = (e) => {
  const m = e.data;
  if (m.module) { ready = WebAssembly.instantiate(m.module, {}).then((i) => { W = i.exports; id = W.ak_gs_sorter_create(); }); return; }
  ready.then(() => {
    if (m.centers) {
      const p = W.ak_gs_sorter_centers(id, m.n, m.set);
      if (m.n) new Float32Array(W.memory.buffer, p, m.n * 3).set(m.centers);
      return;
    }
    const t0 = performance.now();
    const p = W.ak_gs_sort(id, m.v[0], m.v[1], m.v[2], m.v[3], m.mix);
    const out = m.buf && m.buf.byteLength === m.n * 4 ? new Uint32Array(m.buf) : new Uint32Array(m.n);
    out.set(new Uint32Array(W.memory.buffer, p, m.n));
    postMessage({ order: out, ms: performance.now() - t0, seq: m.seq }, [out.buffer]);
  });
};`;

interface SortBackend {
  centers(c: Float32Array, n: number, set: 0 | 1): void;
  /** Request a sort; `done` gets the order (furthest first). */
  sort(v: Float32Array, mix: number, n: number, done: (order: Uint32Array, ms: number) => void): void;
  readonly busy: boolean;
  readonly worker: boolean;
  destroy(): void;
}

function mainSorter(): SortBackend {
  const w = core();
  const id = w.ak_gs_sorter_create();
  let out: Uint32Array | null = null;
  return {
    worker: false,
    busy: false,
    centers(c, n, set) {
      const p = w.ak_gs_sorter_centers(id, n, set);
      if (n) new Float32Array(w.memory.buffer, p, n * 3).set(c.subarray(0, n * 3));
    },
    sort(v, mix, n, done) {
      const t0 = performance.now();
      const p = w.ak_gs_sort(id, v[0], v[1], v[2], v[3], mix);
      if (!out || out.length !== n) out = new Uint32Array(n);
      out.set(new Uint32Array(w.memory.buffer, p, n));
      done(out, performance.now() - t0);
    },
    destroy() { w.ak_gs_sorter_free(id); },
  };
}

function workerSorter(fallback: () => SortBackend): SortBackend {
  const mod = wasmModule();
  if (!mod || typeof Worker === 'undefined' || typeof Blob === 'undefined' || typeof URL.createObjectURL !== 'function') return fallback();
  let wk: Worker;
  let url = '';
  try {
    url = URL.createObjectURL(new Blob([WORKER_SRC], { type: 'text/javascript' }));
    wk = new Worker(url);
    wk.postMessage({ module: mod });
  } catch {
    if (url) URL.revokeObjectURL(url);
    return fallback();
  }
  let pending: ((order: Uint32Array, ms: number) => void) | null = null;
  let spare: ArrayBuffer | null = null;
  let failed: SortBackend | null = null;
  const replay: Array<[Float32Array, number, 0 | 1]> = [];
  const api: SortBackend = {
    worker: true,
    get busy() { return failed ? failed.busy : pending !== null; },
    centers(c, n, set) {
      if (failed) return failed.centers(c, n, set);
      const copy = c.slice(0, n * 3);
      replay[set] = [copy, n, set];
      wk.postMessage({ centers: copy, n, set });
    },
    sort(v, mix, n, done) {
      if (failed) return failed.sort(v, mix, n, done);
      pending = done;
      const buf = spare && spare.byteLength === n * 4 ? spare : null;
      spare = null;
      wk.postMessage({ v: [v[0], v[1], v[2], v[3]], mix, n, buf }, buf ? [buf] : []);
    },
    destroy() { wk.terminate(); URL.revokeObjectURL(url); failed?.destroy(); },
  };
  wk.onmessage = (e: MessageEvent) => {
    const cb = pending;
    pending = null;
    cb?.(e.data.order, e.data.ms);
  };
  wk.onerror = (e) => {
    e.preventDefault?.();
    // e.g. a CSP without blob: workers: sort on the main thread instead.
    failed = fallback();
    for (const r of replay) if (r) failed.centers(r[0], r[1], r[2]);
    pending = null;
  };
  // Recycle the transferred order buffer after the caller has uploaded it.
  (api as SortBackend & { recycle(b: ArrayBuffer): void }).recycle = (b) => { spare = b; };
  return api;
}

// ---------------------------------------------------------------------------
// WebGL2 renderer
// ---------------------------------------------------------------------------

const TEX_W = 2048;

const VS = `#version 300 es
precision highp float;
precision highp int;
precision highp usampler2D;
uniform usampler2D u_a, u_b;
uniform mat4 u_view, u_proj;
uniform vec2 u_viewport;
uniform vec4 u_bounds; // centre xyz, radius
uniform vec2 u_yrange;
uniform float u_morph, u_spread, u_explode, u_swirl, u_dissolve, u_reveal, u_size, u_opacity, u_keep, u_maxr;
in uint a_index;
out vec4 v_color;
out vec2 v_pos;

float hash(uint x) {
  x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16;
  return float(x) / 4294967295.0;
}
vec4 rgba(uint w) { return vec4((uvec4(w) >> uvec4(0u, 8u, 16u, 24u)) & 0xffu) / 255.0; }
ivec2 at(uint t) { return ivec2(int(t & 2047u), int(t >> 11)); }
mat3 cov(uvec4 t) {
  vec2 a = unpackHalf2x16(t.x), b = unpackHalf2x16(t.y), c = unpackHalf2x16(t.z);
  return mat3(a.x, a.y, b.x, a.y, b.y, c.x, b.x, c.x, c.y);
}
void main() {
  uint i = a_index;
  // Governor: skip a random subset early (cheap), grow the rest to cover.
  if (hash(i ^ 0x9e3779b9u) > u_keep) { gl_Position = vec4(0.0, 0.0, 2.0, 1.0); return; }
  uvec4 a0 = texelFetch(u_a, at(i * 2u), 0), a1 = texelFetch(u_a, at(i * 2u + 1u), 0);
  vec3 p = uintBitsToFloat(a0.xyz);
  vec4 col = rgba(a0.w);
  mat3 S = cov(a1);
  float h = hash(i);
  if (u_morph > 0.0) {
    uvec4 b0 = texelFetch(u_b, at(i * 2u), 0), b1 = texelFetch(u_b, at(i * 2u + 1u), 0);
    float t = clamp(u_morph * (1.0 + u_spread) - h * u_spread, 0.0, 1.0);
    t = t * t * (3.0 - 2.0 * t);
    vec3 pb = uintBitsToFloat(b0.xyz);
    vec3 lift = (vec3(hash(i + 7u), hash(i + 13u), hash(i + 29u)) - 0.5) * u_bounds.w * 0.35 * sin(t * 3.14159265) * u_spread;
    p = mix(p, pb, t) + lift;
    col = mix(col, rgba(b0.w), t);
    mat3 Sb = cov(b1);
    S = mat3(mix(S[0], Sb[0], t), mix(S[1], Sb[1], t), mix(S[2], Sb[2], t));
  }
  float R = u_bounds.w;
  if (u_swirl != 0.0) {
    vec3 q = p - u_bounds.xyz;
    float ang = u_swirl * 6.2831853 * (0.6 + 0.8 * h) * (1.0 - 0.5 * clamp(length(q.xz) / R, 0.0, 1.0));
    float c = cos(ang), s = sin(ang);
    p = u_bounds.xyz + vec3(c * q.x + s * q.z, q.y, -s * q.x + c * q.z);
  }
  if (u_explode != 0.0) {
    vec3 r3 = vec3(hash(i + 3u), hash(i + 5u), hash(i + 11u)) - 0.5;
    vec3 d = normalize(p - u_bounds.xyz + r3 * R * 0.6 + 1e-6);
    p += d * u_explode * R * (0.4 + 1.2 * h);
    col.a /= 1.0 + 0.6 * abs(u_explode);
  }
  float vis = clamp((h - u_dissolve * 1.1 + 0.05) * 20.0, 0.0, 1.0);
  p.y += (1.0 - vis) * R * 0.25;
  float yt = mix(u_yrange.x - R * 0.1, u_yrange.y + R * 0.1, u_reveal);
  vis *= clamp((yt - p.y) / (R * 0.04) + 0.5, 0.0, 1.0);
  col.a *= vis * u_opacity;
  vec4 cam = u_view * vec4(p, 1.0);
  vec4 clip = u_proj * cam;
  if (col.a < 0.004 || cam.z > -1e-4 || abs(clip.x) > 1.3 * clip.w || abs(clip.y) > 1.3 * clip.w) {
    gl_Position = vec4(0.0, 0.0, 2.0, 1.0);
    return;
  }
  mat3 W = mat3(u_view);
  mat3 Sv = W * (S * (u_size * u_size / u_keep)) * transpose(W);
  vec2 f = vec2(u_proj[0][0], u_proj[1][1]) * u_viewport * 0.5;
  float z = cam.z, z2 = z * z;
  mat3 J = mat3(-f.x / z, 0.0, 0.0, 0.0, -f.y / z, 0.0, f.x * cam.x / z2, f.y * cam.y / z2, 0.0);
  mat3 C = J * Sv * transpose(J);
  float a = C[0][0] + 0.3, b = C[0][1], c = C[1][1] + 0.3;
  float mid = 0.5 * (a + c), rad = length(vec2(0.5 * (a - c), b));
  float l1 = mid + rad, l2 = max(mid - rad, 0.1);
  vec2 dir = normalize(vec2(b, l1 - a) + vec2(1e-9, 0.0));
  vec2 major = min(sqrt(2.0 * l1), u_maxr) * dir;
  vec2 minor = min(sqrt(2.0 * l2), u_maxr) * vec2(dir.y, -dir.x);
  vec2 corner = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1)) * 3.5355339 - 1.7677670;
  v_pos = corner;
  v_color = col;
  vec2 ndc = clip.xy / clip.w + (corner.x * major + corner.y * minor) * 2.0 / u_viewport;
  gl_Position = vec4(ndc, clamp(clip.z / clip.w, -1.0, 1.0), 1.0);
}`;

const FS = `#version 300 es
precision highp float;
in vec4 v_color;
in vec2 v_pos;
out vec4 o;
void main() {
  float A = -dot(v_pos, v_pos);
  if (A < -3.125) discard; // 2.5 sigma
  float B = exp(A) * v_color.a;
  o = vec4(v_color.rgb * B, B);
}`;

function compile(gl: WebGL2RenderingContext): WebGLProgram {
  const sh = (type: number, src: string) => {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS) && !gl.isContextLost()) throw new Error(`anikit: splats shader: ${gl.getShaderInfoLog(s)}`);
    return s;
  };
  const p = gl.createProgram()!;
  gl.attachShader(p, sh(gl.VERTEX_SHADER, VS));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, FS));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS) && !gl.isContextLost()) throw new Error(`anikit: splats program: ${gl.getProgramInfoLog(p)}`);
  return p;
}

const UNIFORMS = ['u_a', 'u_b', 'u_view', 'u_proj', 'u_viewport', 'u_bounds', 'u_yrange', 'u_morph', 'u_spread', 'u_explode', 'u_swirl', 'u_dissolve', 'u_reveal', 'u_size', 'u_opacity', 'u_keep', 'u_maxr'] as const;

// Column-major 4×4 helpers writing into preallocated arrays.
function perspective(out: Float32Array, fovDeg: number, aspect: number, near: number, far: number): void {
  const f = 1 / Math.tan((fovDeg * Math.PI) / 360);
  out.fill(0);
  out[0] = f / aspect; out[5] = f; out[10] = (far + near) / (near - far); out[11] = -1; out[14] = (2 * far * near) / (near - far);
}

function lookAt(out: Float32Array, c: SplatCamera): void {
  let zx = c.x - c.tx, zy = c.y - c.ty, zz = c.z - c.tz;
  let l = Math.hypot(zx, zy, zz) || 1;
  zx /= l; zy /= l; zz /= l;
  // up = +y (fall back to +z when looking straight up/down)
  let ux = 0, uy = 1, uz = 0;
  if (Math.abs(zy) > 0.999) { uy = 0; uz = 1; }
  let xx = uy * zz - uz * zy, xy = uz * zx - ux * zz, xz = ux * zy - uy * zx;
  l = Math.hypot(xx, xy, xz) || 1;
  xx /= l; xy /= l; xz /= l;
  const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
  out[0] = xx; out[4] = xy; out[8] = xz; out[12] = -(xx * c.x + xy * c.y + xz * c.z);
  out[1] = yx; out[5] = yy; out[9] = yz; out[13] = -(yx * c.x + yy * c.y + yz * c.z);
  out[2] = zx; out[6] = zy; out[10] = zz; out[14] = -(zx * c.x + zy * c.y + zz * c.z);
  out[3] = 0; out[7] = 0; out[11] = 0; out[15] = 1;
}

const MODEL = new Float64Array(16);

/** out = view · model(t) (no allocation). */
function modelView(out: Float32Array, view: ArrayLike<number>, t: SplatTransform): void {
  const r = Math.PI / 180;
  const cx = Math.cos(t.rotateX * r), sx = Math.sin(t.rotateX * r);
  const cy = Math.cos(t.rotateY * r), sy = Math.sin(t.rotateY * r);
  const cz = Math.cos(t.rotateZ * r), sz = Math.sin(t.rotateZ * r);
  const s = t.scale;
  // R = Rz · Ry · Rx (columns)
  const m00 = cz * cy, m10 = sz * cy, m20 = -sy;
  const m01 = cz * sy * sx - sz * cx, m11 = sz * sy * sx + cz * cx, m21 = cy * sx;
  const m02 = cz * sy * cx + sz * sx, m12 = sz * sy * cx - cz * sx, m22 = cy * cx;
  const M = MODEL;
  M[0] = m00 * s; M[1] = m10 * s; M[2] = m20 * s; M[3] = 0;
  M[4] = m01 * s; M[5] = m11 * s; M[6] = m21 * s; M[7] = 0;
  M[8] = m02 * s; M[9] = m12 * s; M[10] = m22 * s; M[11] = 0;
  M[12] = t.x; M[13] = t.y; M[14] = t.z; M[15] = 1;
  for (let c = 0; c < 4; c++) {
    for (let rr = 0; rr < 4; rr++) {
      out[c * 4 + rr] = view[rr] * M[c * 4] + view[4 + rr] * M[c * 4 + 1] + view[8 + rr] * M[c * 4 + 2] + view[12 + rr] * M[c * 4 + 3];
    }
  }
}

function bounds(set: SplatSet): [number, number, number, number, number, number] {
  let x = 0, y = 0, z = 0, lo = Infinity, hi = -Infinity;
  const d = set.data;
  const n = set.count;
  for (let i = 0; i < n; i++) {
    const k = i * SPLAT_STRIDE;
    x += d[k]; y += d[k + 1]; z += d[k + 2];
    if (d[k + 1] < lo) lo = d[k + 1];
    if (d[k + 1] > hi) hi = d[k + 1];
  }
  if (!n) return [0, 0, 0, 1, -1, 1];
  x /= n; y /= n; z /= n;
  let r2 = 0;
  for (let i = 0; i < n; i++) {
    const k = i * SPLAT_STRIDE;
    const q = (d[k] - x) ** 2 + (d[k + 1] - y) ** 2 + (d[k + 2] - z) ** 2;
    if (q > r2) r2 = q;
  }
  return [x, y, z, Math.sqrt(r2) || 1, lo, hi];
}

function centersOf(set: SplatSet, n: number): Float32Array {
  const c = new Float32Array(n * 3);
  const m = Math.max(set.count, 1);
  for (let i = 0; i < n; i++) {
    const j = set.count === n ? i : Math.min(m - 1, Math.floor((i * m) / n));
    const k = j * SPLAT_STRIDE;
    c[i * 3] = set.data[k] ?? 0; c[i * 3 + 1] = set.data[k + 1] ?? 0; c[i * 3 + 2] = set.data[k + 2] ?? 0;
  }
  return c;
}

/**
 * A WebGL2 Gaussian splat layer. Create with `splats(canvas, set)`.
 * `camera`, `transform` and `fx` are plain objects to animate directly.
 */
export class SplatLayer {
  /** The canvas drawn into. */
  readonly canvas: HTMLCanvasElement | OffscreenCanvas;
  /** The WebGL2 context. */
  readonly gl: WebGL2RenderingContext;
  /** Look-at camera (animatable). */
  readonly camera: SplatCamera;
  /** Whole-cloud transform (animatable). */
  readonly transform: SplatTransform;
  /** Effect uniforms (animatable). */
  readonly fx: SplatFx;
  /** Render statistics. */
  readonly stats: SplatStats;
  /** The current splat set. */
  set: SplatSet;
  /** Resolves after the first depth sort has been applied. */
  readonly sorted: Promise<void>;

  private prog: WebGLProgram;
  private loc: Record<(typeof UNIFORMS)[number], WebGLUniformLocation | null>;
  private vao: WebGLVertexArrayObject;
  private indexBuf: WebGLBuffer;
  private texA: WebGLTexture;
  private texB: WebGLTexture;
  private packedA: Uint32Array = new Uint32Array(0);
  private packedB: Uint32Array = new Uint32Array(0);
  private centersA: Float32Array = new Float32Array(0);
  private posSent = 0;
  private posTimer: ReturnType<typeof setTimeout> | null = null;
  private hasB = false;
  private target: SplatSet | null = null;
  private sorter: SortBackend;
  private view = new Float32Array(16);
  private mv = new Float32Array(16);
  private proj = new Float32Array(16);
  private lastV = new Float32Array([NaN, 0, 0, 0]);
  private lastMix = -1;
  private sortV = new Float32Array(4);
  private sortN = 0;
  private dirty = true;
  private b: [number, number, number, number, number, number];
  private off: (() => void) | null = null;
  private own: boolean;
  private bg: [number, number, number, number] | null = null;
  private ratio: number;
  private threshold: number;
  private orbitOff: (() => void) | null = null;
  private resolveSorted!: () => void;
  private morphAnim: Animation | null = null;
  private dead = false;
  private maxR: number;
  private gov: { fps: number; min: number } | null;
  private govLast = 0;
  private govAvg = 0;
  private govHold = 0;
  private onDone: (order: Uint32Array, ms: number) => void;

  constructor(target: HTMLCanvasElement | OffscreenCanvas | WebGL2RenderingContext, set: SplatSet, o: SplatsOptions = {}) {
    checkKeys('splats()', o, OPTION_KEYS);
    const isGl = typeof WebGL2RenderingContext !== 'undefined' && target instanceof WebGL2RenderingContext;
    this.own = !isGl;
    const gl = isGl
      ? (target as WebGL2RenderingContext)
      : ((target as HTMLCanvasElement).getContext('webgl2', { antialias: false, premultipliedAlpha: true, alpha: true, depth: false, preserveDrawingBuffer: true }) as WebGL2RenderingContext | null);
    if (!gl) throw new Error('anikit: splats(): WebGL2 is not available.');
    this.gl = gl;
    this.canvas = gl.canvas;
    this.set = set;
    this.b = bounds(set);
    const [cx, cy, cz, R] = this.b;
    this.camera = { x: cx, y: cy, z: cz + R * 2.6, tx: cx, ty: cy, tz: cz, fov: 45, near: R * 0.01, far: R * 100, ...o.camera };
    this.transform = { x: 0, y: 0, z: 0, rotateX: 0, rotateY: 0, rotateZ: 0, scale: 1, ...o.transform };
    this.fx = { morph: 0, spread: 0.6, explode: 0, swirl: 0, dissolve: 0, reveal: 1, size: 1, opacity: 1, ...o.fx };
    this.ratio = o.pixelRatio ?? Math.min(typeof devicePixelRatio === 'number' ? devicePixelRatio : 1, 2);
    this.threshold = o.sortThreshold ?? 0.002;
    this.maxR = o.maxRadius ?? 48;
    const gv = o.governor ?? (o.autoRender ?? !isGl);
    this.gov = gv ? { fps: 50, min: 0.25, ...(typeof gv === 'object' ? gv : {}) } : null;
    if (o.background) {
      const c = parseColor(o.background);
      if (c) this.bg = [c[0] / 255, c[1] / 255, c[2] / 255, c[3] ?? 1];
      else warn(`splats(): cannot parse background "${o.background}".`);
    }
    this.prog = compile(gl);
    this.loc = Object.fromEntries(UNIFORMS.map((u) => [u, gl.getUniformLocation(this.prog, u)])) as SplatLayer['loc'];
    this.texA = gl.createTexture()!;
    this.texB = gl.createTexture()!;
    this.indexBuf = gl.createBuffer()!;
    this.vao = gl.createVertexArray()!;
    gl.bindVertexArray(this.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.indexBuf);
    const li = gl.getAttribLocation(this.prog, 'a_index');
    gl.enableVertexAttribArray(li);
    gl.vertexAttribIPointer(li, 1, gl.UNSIGNED_INT, 0, 0);
    gl.vertexAttribDivisor(li, 1);
    gl.bindVertexArray(null);
    this.sorted = new Promise((r) => (this.resolveSorted = r));
    this.sorter = o.worker === false ? mainSorter() : workerSorter(mainSorter);
    this.stats = { count: set.count, sorts: 0, frames: 0, sortMs: 0, worker: this.sorter.worker, quality: 1 };
    this.onDone = (order, ms) => {
      if (this.dead) return;
      if (order.length === this.set.count) {
        const g = this.gl;
        g.bindBuffer(g.ARRAY_BUFFER, this.indexBuf);
        g.bufferSubData(g.ARRAY_BUFFER, 0, order);
      }
      this.stats.sorts++;
      this.stats.sortMs = ms;
      (this.sorter as { recycle?: (b: ArrayBuffer) => void }).recycle?.(order.buffer as ArrayBuffer);
      this.resolveSorted();
      if (this.dirty) this.requestRender();
    };
    this.upload(set);
    if (o.orbit) this.orbit();
    if (o.autoRender ?? this.own) this.off = engine.onFrame(() => { this.govern(); this.render(); });
    track(this);
  }

  private upload(set: SplatSet): void {
    const gl = this.gl;
    const n = set.count;
    this.packedA = packSplats(set);
    this.texture(this.texA, this.packedA);
    this.centersA = centersOf(set, n);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.indexBuf);
    const id = new Uint32Array(n);
    for (let i = 0; i < n; i++) id[i] = i;
    gl.bufferData(gl.ARRAY_BUFFER, id, gl.DYNAMIC_DRAW);
    this.sorter.centers(this.centersA, n, 0);
    this.sorter.centers(new Float32Array(0), 0, 1);
    this.hasB = false;
    this.target = null;
    this.stats.count = n;
    this.lastV[0] = NaN;
    this.dirty = true;
  }

  private texture(t: WebGLTexture, packed: Uint32Array): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32UI, TEX_W, packed.length / 4 / TEX_W, 0, gl.RGBA_INTEGER, gl.UNSIGNED_INT, packed);
  }

  /**
   * Move splats every frame: `xyz` holds `x y z` centres for splats
   * `start … start + count` (default: the whole layer). Only the changed
   * texture rows are uploaded; the depth sort picks up the new centres,
   * throttled to one re-send per 100 ms. Drive it from springs, physics or
   * `pointCloud().positions`. Allocation-free per call.
   */
  positions(xyz: ArrayLike<number>, start = 0, count = Math.floor(xyz.length / 3)): this {
    const n = this.set.count;
    start = Math.max(0, Math.min(n, start | 0));
    const end = Math.min(n, start + Math.max(0, count | 0));
    if (end <= start || this.dead) return this;
    const P = this.packedA;
    const f = new Float32Array(P.buffer, P.byteOffset, P.length);
    const c = this.centersA;
    for (let i = start, k = 0; i < end; i++, k += 3) {
      const x = xyz[k], y = xyz[k + 1], z = xyz[k + 2];
      f[i * 8] = x; f[i * 8 + 1] = y; f[i * 8 + 2] = z;
      c[i * 3] = x; c[i * 3 + 1] = y; c[i * 3 + 2] = z;
    }
    const gl = this.gl;
    const r0 = (start * 2 / TEX_W) | 0;
    const r1 = (((end - 1) * 2 + 1) / TEX_W) | 0;
    gl.bindTexture(gl.TEXTURE_2D, this.texA);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, r0, TEX_W, r1 - r0 + 1, gl.RGBA_INTEGER, gl.UNSIGNED_INT, P, r0 * TEX_W * 4);
    this.dirty = true;
    const now = performance.now();
    if (now - this.posSent >= 100) this.sendCenters();
    else if (!this.posTimer) this.posTimer = setTimeout(() => this.sendCenters(), 100 - (now - this.posSent));
    return this;
  }

  private sendCenters(): void {
    if (this.posTimer) clearTimeout(this.posTimer);
    this.posTimer = null;
    if (this.dead) return;
    this.posSent = performance.now();
    this.sorter.centers(this.centersA, this.set.count, 0);
    this.lastV[0] = NaN; // force a re-sort on the next frame
    this.requestRender();
  }

  /** Replace the splat set (count may change). Ends any morph. */
  setSplats(set: SplatSet): this {
    this.morphAnim?.kill();
    this.set = set;
    this.b = bounds(set);
    this.fx.morph = 0;
    this.upload(set);
    return this;
  }

  /**
   * Morph toward another set: positions, shapes and colours interpolate on
   * the GPU (staggered by `fx.spread`). Counts may differ; the target is
   * resampled to this layer's count. When done, the target becomes the set.
   */
  morph(other: SplatSet, o: SplatMorphOptions = {}): Animation {
    this.morphAnim?.kill();
    if (this.hasB && this.target && this.fx.morph >= 1) this.adopt();
    const n = this.set.count;
    this.packedB = packSplats(other, n);
    this.texture(this.texB, this.packedB);
    this.sorter.centers(centersOf(other, n), n, 1);
    this.hasB = true;
    this.target = other;
    this.fx.morph = 0;
    if (o.spread !== undefined) this.fx.spread = o.spread;
    this.dirty = true;
    const a = animate(this.fx, { morph: 1 }, {
      duration: o.duration ?? 1.6, ease: o.ease ?? 'inOutCubic', delay: o.delay ?? 0,
      onComplete: () => { if (this.morphAnim === a) this.adopt(); },
    });
    this.morphAnim = a;
    return a;
  }

  /** Make the morph target the current set (keeps the count). */
  private adopt(): void {
    const t = this.target;
    if (!t) return;
    const n = this.set.count;
    const set = t.count === n ? t : { count: n, data: remap(t, n) };
    const tp = this.packedA;
    this.packedA = this.packedB;
    this.packedB = tp;
    this.centersA = centersOf(set, n);
    const tmp = this.texA;
    this.texA = this.texB;
    this.texB = tmp;
    this.set = set;
    this.b = bounds(set);
    this.sorter.centers(this.centersA, n, 0);
    this.sorter.centers(new Float32Array(0), 0, 1);
    this.hasB = false;
    this.target = null;
    this.fx.morph = 0;
    this.lastV[0] = NaN;
    this.dirty = true;
  }

  private fxTo(key: keyof SplatFx, def: number, o: SplatEffectOptions): Animation {
    return animate(this.fx, { [key]: o.to ?? def }, { duration: o.duration ?? 1.2, ease: o.ease ?? 'outCubic', delay: o.delay ?? 0 });
  }

  /** Blow the cloud apart (`to` default 1 ≈ one radius); `explode({ to: 0 })` reassembles. */
  explode(o: SplatEffectOptions = {}): Animation {
    return this.fxTo('explode', 1, o);
  }

  /** Dissolve splats away in random order, drifting up (`to: 0` brings them back). */
  dissolve(o: SplatEffectOptions = {}): Animation {
    return this.fxTo('dissolve', 1, o);
  }

  /** Swirl around the vertical axis by `to` turns (default 1). */
  swirl(o: SplatEffectOptions = {}): Animation {
    return this.fxTo('swirl', 1, o);
  }

  /** Reveal bottom-up from 0 to `to` (default 1). Bind `fx.reveal` to scroll for scroll reveals. */
  reveal(o: SplatEffectOptions = {}): Animation {
    this.fx.reveal = 0;
    return this.fxTo('reveal', 1, o);
  }

  /** Drag to orbit around the camera target, wheel to zoom. Returns a function that removes it. */
  orbit(): () => void {
    const el = this.canvas as HTMLCanvasElement;
    if (this.orbitOff || typeof el.addEventListener !== 'function') return this.orbitOff ?? (() => {});
    let down = false, px = 0, py = 0;
    const c = this.camera;
    const sph = () => {
      const dx = c.x - c.tx, dy = c.y - c.ty, dz = c.z - c.tz;
      const r = Math.hypot(dx, dy, dz);
      return [r, Math.atan2(dx, dz), Math.asin(Math.max(-1, Math.min(1, dy / (r || 1))))];
    };
    const put = (r: number, yaw: number, pitch: number) => {
      pitch = Math.max(-1.5, Math.min(1.5, pitch));
      c.x = c.tx + r * Math.sin(yaw) * Math.cos(pitch);
      c.y = c.ty + r * Math.sin(pitch);
      c.z = c.tz + r * Math.cos(yaw) * Math.cos(pitch);
    };
    const pd = (e: PointerEvent) => { down = true; px = e.clientX; py = e.clientY; el.setPointerCapture?.(e.pointerId); };
    const pm = (e: PointerEvent) => {
      if (!down) return;
      const [r, yaw, pitch] = sph();
      put(r, yaw - (e.clientX - px) * 0.008, pitch + (e.clientY - py) * 0.008);
      px = e.clientX; py = e.clientY;
    };
    const pu = () => { down = false; };
    const wh = (e: WheelEvent) => {
      e.preventDefault();
      const [r, yaw, pitch] = sph();
      put(r * Math.exp(e.deltaY * 0.001), yaw, pitch);
    };
    el.addEventListener('pointerdown', pd);
    el.addEventListener('pointermove', pm);
    el.addEventListener('pointerup', pu);
    el.addEventListener('pointercancel', pu);
    el.addEventListener('wheel', wh, { passive: false });
    el.style.touchAction = 'none';
    this.orbitOff = () => {
      el.removeEventListener('pointerdown', pd);
      el.removeEventListener('pointermove', pm);
      el.removeEventListener('pointerup', pu);
      el.removeEventListener('pointercancel', pu);
      el.removeEventListener('wheel', wh);
      this.orbitOff = null;
    };
    return this.orbitOff;
  }

  private requestRender(): void {
    if (!this.off && !this.dead && this.own) this.render();
  }

  /** Resize the drawing buffer to the canvas's CSS size × pixel ratio (automatic in `render()`). */
  resize(): void {
    const c = this.canvas as HTMLCanvasElement;
    if (!this.own || typeof c.clientWidth !== 'number') return;
    const w = Math.max(1, Math.round((c.clientWidth || c.width) * this.ratio));
    const h = Math.max(1, Math.round((c.clientHeight || c.height) * this.ratio));
    if (c.clientWidth && (c.width !== w || c.height !== h)) {
      c.width = w;
      c.height = h;
    }
  }

  /** Render one frame into the layer's own canvas (automatic unless `autoRender: false`). */
  render(): void {
    if (this.dead) return;
    const gl = this.gl;
    this.resize();
    const w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
    lookAt(this.view, this.camera);
    perspective(this.proj, this.camera.fov, w / h, this.camera.near, this.camera.far);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, w, h);
    if (this.bg) gl.clearColor(this.bg[0] * this.bg[3], this.bg[1] * this.bg[3], this.bg[2] * this.bg[3], this.bg[3]);
    else gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    this.draw(this.view, this.proj, w, h);
  }

  /**
   * Draw into whatever framebuffer and viewport are bound on this layer's
   * context, with the given column-major view and projection matrices: the
   * hook for rendering splats inside another WebGL2 scene. The layer's
   * `transform` is applied on top of `view`. `width`/`height` are the
   * viewport size in pixels (default: the drawing buffer).
   */
  draw(view: ArrayLike<number>, proj: ArrayLike<number>, width = this.gl.drawingBufferWidth, height = this.gl.drawingBufferHeight): void {
    if (this.dead) return;
    const gl = this.gl;
    const n = this.set.count;
    this.stats.frames++;
    this.dirty = false;
    modelView(this.mv, view, this.transform);
    this.maybeSort();
    if (!n) return;
    const L = this.loc;
    const f = this.fx;
    gl.useProgram(this.prog);
    gl.uniformMatrix4fv(L.u_view, false, this.mv);
    gl.uniformMatrix4fv(L.u_proj, false, proj as Float32Array);
    gl.uniform2f(L.u_viewport, width, height);
    const b = this.b;
    gl.uniform4f(L.u_bounds, b[0], b[1], b[2], b[3]);
    gl.uniform2f(L.u_yrange, b[4], b[5]);
    gl.uniform1f(L.u_morph, this.hasB ? f.morph : 0);
    gl.uniform1f(L.u_spread, f.spread);
    gl.uniform1f(L.u_explode, f.explode);
    gl.uniform1f(L.u_swirl, f.swirl);
    gl.uniform1f(L.u_dissolve, f.dissolve);
    gl.uniform1f(L.u_reveal, f.reveal);
    gl.uniform1f(L.u_size, f.size);
    gl.uniform1f(L.u_opacity, f.opacity);
    gl.uniform1f(L.u_keep, this.stats.quality);
    gl.uniform1f(L.u_maxr, this.maxR * Math.sqrt(2) * (this.own ? this.ratio : 1));
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texA);
    gl.uniform1i(L.u_a, 0);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.hasB ? this.texB : this.texA);
    gl.uniform1i(L.u_b, 1);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.CULL_FACE);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.bindVertexArray(this.vao);
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, n);
    gl.bindVertexArray(null);
  }

  private maybeSort(): void {
    const n = this.set.count;
    if (!n || this.sorter.busy) return;
    const m = this.mv;
    // Third row of view·model: depth along the view axis (larger = nearer).
    const v = this.sortV;
    v[0] = m[2]; v[1] = m[6]; v[2] = m[10]; v[3] = m[14];
    const mix = this.hasB ? Math.min(1, Math.max(0, this.fx.morph)) : 0;
    const L = this.lastV;
    const sc = Math.hypot(v[0], v[1], v[2]) || 1;
    const R = this.b[3] * sc;
    const moved = Number.isNaN(L[0]) || this.sortN !== n
      || Math.abs(v[0] - L[0]) + Math.abs(v[1] - L[1]) + Math.abs(v[2] - L[2]) > this.threshold * sc
      || Math.abs(v[3] - L[3]) > this.threshold * R
      || Math.abs(mix - this.lastMix) > 0.02;
    if (!moved) return;
    L.set(v);
    this.lastMix = mix;
    this.sortN = n;
    this.sorter.sort(v, mix, n, this.onDone);
  }

  /** Adjust `stats.quality` from the measured frame interval (governor). */
  private govern(): void {
    const g = this.gov;
    if (!g) return;
    const now = performance.now();
    const dt = this.govLast ? now - this.govLast : 0;
    this.govLast = now;
    if (!dt || dt > 250) return; // first frame, or the tab was hidden
    this.govAvg = this.govAvg ? this.govAvg * 0.9 + dt * 0.1 : dt;
    if (this.govHold > 0) { this.govHold--; return; }
    const budget = 1000 / g.fps;
    const q = this.stats.quality;
    if (this.govAvg > budget * 1.15 && q > g.min) {
      this.stats.quality = Math.max(g.min, q * 0.8);
      this.govHold = 20;
    } else if (this.govAvg < budget * 0.75 && q < 1) {
      this.stats.quality = Math.min(1, q * 1.1);
      this.govHold = 30;
    }
  }

  /** Remove the layer: stop rendering, free GPU resources and the worker. */
  kill(): void {
    if (this.dead) return;
    this.dead = true;
    this.off?.();
    this.off = null;
    this.orbitOff?.();
    this.morphAnim?.kill();
    if (this.posTimer) clearTimeout(this.posTimer);
    this.sorter.destroy();
    const gl = this.gl;
    gl.deleteTexture(this.texA);
    gl.deleteTexture(this.texB);
    gl.deleteBuffer(this.indexBuf);
    gl.deleteVertexArray(this.vao);
    gl.deleteProgram(this.prog);
    if (this.own) {
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
    }
  }

  /** Same as `kill()` (context cleanup). */
  revert(): void {
    this.kill();
  }
}

function remap(set: SplatSet, n: number): Float32Array {
  const out = new Float32Array(n * SPLAT_STRIDE);
  const m = Math.max(set.count, 1);
  for (let i = 0; i < n && set.count; i++) {
    const j = Math.min(m - 1, Math.floor((i * m) / n));
    out.set(set.data.subarray(j * SPLAT_STRIDE, (j + 1) * SPLAT_STRIDE), i * SPLAT_STRIDE);
  }
  return out;
}

function createSplats(target: string | HTMLCanvasElement | OffscreenCanvas | WebGL2RenderingContext, set: SplatSet, options?: SplatsOptions): SplatLayer {
  let t = target;
  if (typeof t === 'string') {
    const el = typeof document !== 'undefined' ? document.querySelector(t) : null;
    if (!el) throw new Error(`anikit: splats(): no element matches "${t}".`);
    if (el instanceof HTMLCanvasElement) t = el;
    else {
      const c = document.createElement('canvas');
      c.style.cssText = 'display:block;width:100%;height:100%';
      el.appendChild(c);
      t = c;
    }
  }
  return new SplatLayer(t, set, options);
}

/**
 * Real-time 3D Gaussian splatting (WebGL2). `splats(canvasOrSelector, set, options)`
 * creates a {@link SplatLayer}; sources: `splats.parse`, `splats.load`,
 * `splats.fromPoints`, `splats.fromMesh`, `splats.fromText`, `splats.fromSDF`,
 * `splats.merge`.
 */
export const splats = /* @__PURE__ */ Object.assign(createSplats, { parse, load, fromPoints, fromMesh, fromText, fromSDF, merge, pack: packSplats });
