/**
 * Minimal glTF 2.0 loader for `scene()`: `.gltf` (embedded or external
 * buffers) and `.glb`; triangle meshes (POSITION, NORMAL, indices), PBR
 * metal/roughness factors, the node hierarchy, and TRS animations (linear /
 * step; rotations slerped). Textures and skinning are not loaded.
 *
 * ```ts
 * const model = await loadGLTF(s, '/duck.glb');
 * model.root.scale.set(0.01);
 * model.play(0, { loop: true });
 * ```
 */
import { engine } from './core';
import type { Geometry } from './geometry';
import { smoothNormals } from './geometry';
import { Material, Object3D, Scene } from './gl';

/** A loaded glTF model. */
export interface GltfModel {
  /** Group holding the model (add transforms here). */
  root: Object3D;
  /** Animation names (index = position). */
  animations: string[];
  /** Play an animation by index or name; returns a stop fn. */
  play(which?: number | string, options?: { loop?: boolean; speed?: number }): () => void;
}

/* eslint-disable @typescript-eslint/no-explicit-any */
type Json = any;
const COMP: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };

function accessor(g: Json, bufs: ArrayBuffer[], i: number): Float32Array | Uint32Array {
  const a = g.accessors[i], v = g.bufferViews[a.bufferView], n = COMP[a.type], count = a.count * n;
  const off = (v.byteOffset ?? 0) + (a.byteOffset ?? 0), buf = bufs[v.buffer];
  const ctor: any = { 5126: Float32Array, 5125: Uint32Array, 5123: Uint16Array, 5121: Uint8Array, 5122: Int16Array, 5120: Int8Array }[a.componentType as number];
  const stride = v.byteStride ?? 0, size = ctor.BYTES_PER_ELEMENT * n;
  let src: ArrayLike<number>;
  if (stride && stride !== size) {
    const out = new ctor(count), dv = new ctor(buf);
    for (let k = 0; k < a.count; k++) for (let c = 0; c < n; c++) out[k * n + c] = dv[(off + k * stride) / ctor.BYTES_PER_ELEMENT + c];
    src = out;
  } else src = new ctor(buf, off, count);
  return a.componentType === 5126 ? Float32Array.from(src) : Uint32Array.from(src);
}

function trs(o: Float32Array, t: ArrayLike<number>, q: ArrayLike<number>, s: ArrayLike<number>): void {
  const [x, y, z, w] = [q[0], q[1], q[2], q[3]];
  const x2 = x + x, y2 = y + y, z2 = z + z, xx = x * x2, xy = x * y2, xz = x * z2, yy = y * y2, yz = y * z2, zz = z * z2, wx = w * x2, wy = w * y2, wz = w * z2;
  o[0] = (1 - (yy + zz)) * s[0]; o[1] = (xy + wz) * s[0]; o[2] = (xz - wy) * s[0]; o[3] = 0;
  o[4] = (xy - wz) * s[1]; o[5] = (1 - (xx + zz)) * s[1]; o[6] = (yz + wx) * s[1]; o[7] = 0;
  o[8] = (xz + wy) * s[2]; o[9] = (yz - wx) * s[2]; o[10] = (1 - (xx + yy)) * s[2]; o[11] = 0;
  o[12] = t[0]; o[13] = t[1]; o[14] = t[2]; o[15] = 1;
}

/** Parse glTF JSON + binary buffers (already fetched) into a model. */
export function parseGLTF(s: Scene, g: Json, bufs: ArrayBuffer[]): GltfModel {
  const mats = (g.materials ?? []).map((m: Json) => {
    const p = m.pbrMetallicRoughness ?? {}, c = p.baseColorFactor ?? [1, 1, 1, 1];
    const mat = new Material({ metalness: p.metallicFactor ?? 1, roughness: p.roughnessFactor ?? 1, opacity: m.alphaMode === 'BLEND' ? c[3] : 1 });
    mat.color = [c[0], c[1], c[2]];
    if (m.emissiveFactor) { mat.emissive = m.emissiveFactor.slice(0, 3); mat.emissiveIntensity = m.extensions?.KHR_materials_emissive_strength?.emissiveStrength ?? 1; }
    return mat;
  });
  const fallback = new Material({ preset: 'plastic' });
  const meshes: Array<Array<{ geo: Geometry; mat: Material }>> = (g.meshes ?? []).map((m: Json) => m.primitives
    .filter((p: Json) => (p.mode ?? 4) === 4 && p.attributes.POSITION !== undefined)
    .map((p: Json) => {
      const positions = accessor(g, bufs, p.attributes.POSITION) as Float32Array;
      const n = positions.length / 3;
      const indices = p.indices !== undefined ? (accessor(g, bufs, p.indices) as Uint32Array) : Uint32Array.from({ length: n }, (_, i) => i);
      let geo: Geometry = { positions, normals: new Float32Array(n * 3), uvs: new Float32Array(n * 2), indices };
      if (p.attributes.NORMAL !== undefined) geo.normals = accessor(g, bufs, p.attributes.NORMAL) as Float32Array;
      else geo = smoothNormals(geo);
      return { geo, mat: p.material !== undefined ? mats[p.material] : fallback };
    }));
  const root = s.add(null);
  const nodes: Object3D[] = [];
  const state: Array<{ t: number[]; r: number[]; s: number[] }> = [];
  (g.nodes ?? []).forEach((nd: Json, i: number) => {
    const o = s.add(null);
    o.remove();
    o.matrix = new Float32Array(16);
    const st = { t: nd.translation ?? [0, 0, 0], r: nd.rotation ?? [0, 0, 0, 1], s: nd.scale ?? [1, 1, 1] };
    if (nd.matrix) o.matrix.set(nd.matrix); else trs(o.matrix, st.t, st.r, st.s);
    state[i] = st; nodes[i] = o;
    if (nd.mesh !== undefined) for (const p of meshes[nd.mesh]) { const m = s.add(p.geo, { material: p.mat }); m.remove(); o.add(m); m.scene = s; }
  });
  (g.nodes ?? []).forEach((nd: Json, i: number) => { for (const c of nd.children ?? []) nodes[i].add(nodes[c]); });
  const sceneNodes: number[] = g.scenes?.[g.scene ?? 0]?.nodes ?? nodes.map((_, i) => i).filter((i) => !nodes[i].parent);
  for (const i of sceneNodes) root.add(nodes[i]);
  const setScene = (o: Object3D) => { o.scene = s; o.children.forEach(setScene); };
  setScene(root);

  const anims = (g.animations ?? []).map((a: Json) => ({
    name: a.name ?? '',
    channels: a.channels.filter((c: Json) => c.target.node !== undefined && c.target.path !== 'weights').map((c: Json) => {
      const sm = a.samplers[c.sampler];
      return { node: c.target.node as number, path: c.target.path as 'translation' | 'rotation' | 'scale', interp: sm.interpolation ?? 'LINEAR',
        times: accessor(g, bufs, sm.input) as Float32Array, values: accessor(g, bufs, sm.output) as Float32Array };
    }),
  }));
  return {
    root,
    animations: anims.map((a: Json) => a.name),
    play(which = 0, o = {}) {
      const a = typeof which === 'number' ? anims[which] : anims.find((x: Json) => x.name === which);
      if (!a) return () => {};
      let end = 0;
      for (const c of a.channels) end = Math.max(end, c.times[c.times.length - 1]);
      let time = 0;
      const tmp = [0, 0, 0, 0];
      return engine.onFrame((_t, dt) => {
        time += (dt / 1000) * (o.speed ?? 1);
        if (o.loop && end > 0) time %= end; else time = Math.min(time, end);
        for (const c of a.channels) {
          const T = c.times, n = c.path === 'rotation' ? 4 : 3, st = state[c.node];
          let k = 0;
          while (k < T.length - 2 && T[k + 1] <= time) k++;
          const f = c.interp === 'STEP' || T.length < 2 ? 0 : Math.max(0, Math.min(1, (time - T[k]) / (T[k + 1] - T[k] || 1)));
          const cubic = c.interp === 'CUBICSPLINE' ? 3 : 1, off = cubic === 3 ? n : 0;
          const A = (k * cubic) * n + off, B = (Math.min(k + 1, T.length - 1) * cubic) * n + off, V = c.values;
          if (n === 4) {
            let d = 0; for (let j = 0; j < 4; j++) d += V[A + j] * V[B + j];
            const sgn = d < 0 ? -1 : 1;
            let l = 0;
            for (let j = 0; j < 4; j++) { tmp[j] = V[A + j] + (V[B + j] * sgn - V[A + j]) * f; l += tmp[j] * tmp[j]; }
            l = Math.sqrt(l) || 1;
            for (let j = 0; j < 4; j++) st.r[j] = tmp[j] / l;
          } else {
            const dst = c.path === 'translation' ? st.t : st.s;
            for (let j = 0; j < 3; j++) dst[j] = V[A + j] + (V[B + j] - V[A + j]) * f;
          }
          trs(nodes[c.node].matrix!, st.t, st.r, st.s);
        }
      });
    },
  };
}

/** Fetch and load a `.gltf` or `.glb` into the scene (see {@link GltfModel}). */
export async function loadGLTF(s: Scene, url: string): Promise<GltfModel> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`anikit: loadGLTF(): ${res.status} for ${url}`);
  const bin = await res.arrayBuffer();
  const dv = new DataView(bin);
  let json: Json, bufs: ArrayBuffer[] = [];
  if (dv.getUint32(0, true) === 0x46546c67) {
    let off = 12;
    while (off < bin.byteLength) {
      const len = dv.getUint32(off, true), type = dv.getUint32(off + 4, true);
      if (type === 0x4e4f534a) json = JSON.parse(new TextDecoder().decode(new Uint8Array(bin, off + 8, len)));
      else if (type === 0x004e4942) bufs.push(bin.slice(off + 8, off + 8 + len));
      off += 8 + len;
    }
  } else json = JSON.parse(new TextDecoder().decode(bin));
  const base = new URL(url, typeof location !== 'undefined' ? location.href : 'http://localhost/');
  bufs = await Promise.all((json.buffers ?? []).map(async (b: Json, i: number) => {
    if (b.uri === undefined) return bufs[i];
    const r = await fetch(new URL(b.uri, base).href);
    return r.arrayBuffer();
  }));
  return parseGLTF(s, json, bufs);
}
