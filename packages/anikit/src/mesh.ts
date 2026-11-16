/**
 * Solid 3D: lit, z-buffered, instanced meshes rendered in WebAssembly.
 *
 * ```ts
 * const cube = mesh.cube();
 * const cubes = instances(5000);
 * for (let i = 0; i < 5000; i++) cubes.set(i, { x, y, z, rx, ry, scale: 12, color: '#ff3b14' });
 * const r3 = renderer3d(canvas);                 // or renderer3d({ width, height })
 * engine.onFrame(() => r3.render(scene, [{ mesh: cube, instances: cubes }], { background: '#050505', fog: [800, 4000] }));
 * ```
 *
 * Per-vertex Blinn–Phong with a key light, a fill light, a fresnel rim and
 * depth fog; per-pixel depth test; back-face culling. Tens of thousands of
 * triangles per frame at 1080p on the CPU — no WebGL.
 */
import { core, F64, f64Buf, W } from './wasm';
import { parseColor } from './values';
import { track } from './context';
import type { Scene3D } from './scene';
import { addPass, created } from './profile';

type V3 = [number, number, number];
const STRIDE = 15;

function rgb(c: string | V3 | undefined, d: V3 = [1, 1, 1]): V3 {
  if (!c) return d;
  if (Array.isArray(c)) return c;
  const p = parseColor(c);
  return p ? [p[0] / 255, p[1] / 255, p[2] / 255] : d;
}

/** An image living in WebAssembly memory, mipmapped, for `material.texture`. */
export class Texture3D {
  constructor(readonly id: number, readonly width: number, readonly height: number) {}
  free(): void {
    core().ak_texture_free(this.id);
  }
}

type ImageLike = { width: number; height: number; data: ArrayLike<number> };

/**
 * Upload an image for 3D materials: `ImageData`, `{ width, height, data }`
 * (RGBA bytes), or a canvas (its current pixels). Row 0 is the top (v = 0).
 *
 * ```ts
 * const screen = texture(uiCanvas);
 * { mesh: mesh.plane(), instances, material: { texture: screen, textureMode: 'emissive', textureGain: 1.4 } }
 * ```
 */
export function texture(src: ImageLike | CanvasLike): Texture3D {
  let img: ImageLike;
  if ('data' in src) img = src;
  else {
    const ctx = src.getContext('2d');
    if (!ctx) throw new Error('anikit: texture() needs ImageData, { width, height, data } or a canvas with a 2d context');
    img = ctx.getImageData(0, 0, src.width, src.height);
  }
  const n = img.width * img.height * 4;
  if (img.data.length < n) throw new Error(`anikit: texture() got ${img.data.length} bytes, expected ${n} (width × height × 4)`);
  const w = core();
  const ptr = w.ak_u8_buf(n);
  new Uint8Array(W!.memory.buffer, ptr, n).set(img.data.length === n ? img.data : Array.prototype.slice.call(img.data, 0, n));
  return new Texture3D(w.ak_texture_create(img.width, img.height), img.width, img.height);
}

/** A mesh living in WebAssembly memory. */
export class Mesh3D {
  /** Depth/width ratio for meshes built from images (`mesh.text`): use it as the instance's `sz`/`sy` factor. */
  aspect = 1;
  constructor(readonly id: number) {
    created.meshes++;
  }
  get vertices(): number {
    return core().ak_mesh_ptr(this.id, 3);
  }
  get triangles(): number {
    return core().ak_mesh_ptr(this.id, 4);
  }
  free(): void {
    core().ak_mesh_free(this.id);
  }
}

/** Bounding sphere `[cx, cy, cz, r]` per mesh (it only steers `sort: 'near'`, never the pixels). */
const boundsCache = new WeakMap<Mesh3D, number[]>();
function meshBounds(m: Mesh3D): number[] {
  let b = boundsCache.get(m);
  if (!b) {
    const at = f64Buf(4);
    b = core().ak_mesh_bounds(m.id) ? Array.from(F64.subarray(at, at + 4)) : [0, 0, 0, 0];
    boundsCache.set(m, b);
  }
  return b;
}

const builtin = (kind: number, params: number[] = []) => {
  const at = f64Buf(Math.max(1, params.length));
  F64.set(params, at);
  return new Mesh3D(core().ak_mesh_builtin(kind));
};

export const mesh = {
  /** Unit cube centred on the origin, flat-shaded. */
  cube: () => builtin(0),
  /** Unit sphere. */
  sphere: (rings = 16, segments = 24) => builtin(1, [rings, segments]),
  /** A (p, q) torus knot tube. */
  torusKnot: (o: { p?: number; q?: number; radius?: number; spread?: number; tube?: number; segments?: number; sides?: number } = {}) =>
    builtin(2, [o.p ?? 2, o.q ?? 5, o.radius ?? 300, o.spread ?? 120, o.tube ?? 40, o.segments ?? 360, o.sides ?? 16]),
  /** A torus in the XY plane. */
  torus: (radius = 300, tube = 60, segments = 96, sides = 24) => builtin(3, [radius, tube, segments, sides]),
  /**
   * A flat grid in the XZ plane (−0.5…0.5, scale it with the instance), facing +Y,
   * with `cells` subdivisions; UVs span 0–1. Floors and horizons: pair with
   * `lines` + `grid` for a glowing grid that runs to the horizon.
   */
  plane: (cells: number | [number, number] = 1) => {
    const [x, z] = Array.isArray(cells) ? cells : [cells, cells];
    return builtin(4, [x, z]);
  },
  /**
   * An open tube along a path of points (light trails, energy streams, cables), with
   * twist-free frames. `u` runs along it (0–1 by length, so `pulse` travels down it),
   * `v` around. `taper` (or a per-point `radii` array) thins the ends.
   *
   * ```ts
   * const trail = mesh.tube(path.points(200), { radius: 12, taper: [0, 1] });
   * items.push({ mesh: trail, blend: 'additive', material: { emissive: 1, pulse: 3, pulsePhase: t * 4 } });
   * ```
   */
  tube(points: ArrayLike<number> | [number, number, number][], o: {
    /** Tube radius (default 10). */
    radius?: number;
    /** Sides around the tube (default 8). */
    sides?: number;
    /** Radius scale per point (overrides `taper`). */
    radii?: ArrayLike<number>;
    /** Radius scale at the start and end, eased along the tube (e.g. `[0, 1]` for a comet). */
    taper?: [number, number];
  } = {}): Mesh3D {
    const flat: number[] = Array.isArray(points[0]) ? (points as [number, number, number][]).flat() : Array.from(points as ArrayLike<number>);
    const n = Math.floor(flat.length / 3);
    const radii = o.radii ? Array.from(o.radii) : o.taper ? Array.from({ length: n }, (_, i) => { const k = i / Math.max(1, n - 1); const e = k * k * (3 - 2 * k); return o.taper![0] + (o.taper![1] - o.taper![0]) * e; }) : null;
    const vals = [n, o.radius ?? 10, o.sides ?? 8, radii ? 1 : 0, ...flat.slice(0, n * 3), ...(radii ?? [])];
    const at = f64Buf(vals.length);
    F64.set(vals, at);
    return new Mesh3D(core().ak_mesh_builtin(7));
  },
  /** Capped cylinder, radius 1, height 1 (Y −0.5…0.5). `u` runs around, `v` along. */
  cylinder: (segments = 32, caps = true) => builtin(5, [segments, caps ? 1 : 0]),
  /**
   * A heightfield in the XZ plane (−0.5…0.5) from a `width × depth` grid of
   * heights (row-major), with smooth normals. Cells whose corners are all
   * `<= skip` are cut out (use `skip: 0` to keep only raised areas).
   *
   * ```ts
   * const hills = mesh.heightfield(64, 64, (x, z) => noise2(x * 4, z * 4) * 0.1);
   * ```
   */
  heightfield(width: number, depth: number, heights: ArrayLike<number> | ((x: number, z: number) => number), o: { skip?: number } = {}): Mesh3D {
    const n = width * depth;
    // Resolve user code before reserving the scratch buffer (AGENTS.md scratch-buffer rule).
    const hs = typeof heights === 'function' ? Float64Array.from({ length: n }, (_, i) => heights((i % width) / (width - 1), Math.floor(i / width) / (depth - 1))) : heights;
    const at = f64Buf(3 + n);
    F64.set([width, depth, o.skip ?? -Infinity], at);
    F64.set(hs as ArrayLike<number>, at + 3);
    return new Mesh3D(core().ak_mesh_builtin(6));
  },
  /**
   * Solid 3D text in any font: the text is rasterised to a canvas, softened
   * (which gives the edges a bevel) and raised into a heightfield, so the
   * letters stand `depth` tall out of the XZ plane. Rotate the instance
   * (`rx: 90`) to stand it up facing the camera. Width is 1 unit; scale it.
   *
   * ```ts
   * const word = mesh.text('anikit', { font: '900 120px Inter', depth: 0.06 });
   * items.push({ mesh: word, instances: instances(1, () => ({ rx: 90, scale: 1200 })) });
   * ```
   */
  text(s: string, o: { font?: string; resolution?: number; depth?: number; bevel?: number; padding?: number } = {}): Mesh3D {
    const font = o.font ?? '900 120px sans-serif';
    const doc = (globalThis as { document?: { createElement(t: string): HTMLCanvasElement } }).document;
    if (!doc) throw new Error('anikit: mesh.text() needs a canvas (a browser, or anikit/node\'s loadAnikit in Node)');
    const probe = doc.createElement('canvas').getContext('2d')!;
    probe.font = font;
    const m = probe.measureText(s);
    const pad = o.padding ?? 8;
    const tw = Math.ceil(m.width) + pad * 2;
    const asc = m.actualBoundingBoxAscent || parseFloat(font.match(/(\d+)px/)?.[1] ?? '100') * 0.8;
    const desc = m.actualBoundingBoxDescent || 0;
    const th = Math.ceil(asc + desc) + pad * 2;
    // Sample grid: `resolution` columns across the text (default 360).
    const cols = Math.max(16, Math.round(o.resolution ?? 360));
    const scale = cols / tw;
    const W = cols, H = Math.max(4, Math.round(th * scale));
    const c = doc.createElement('canvas');
    c.width = W; c.height = H;
    const ctx = c.getContext('2d')!;
    ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, H);
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    ctx.font = font; ctx.fillStyle = '#fff'; ctx.textBaseline = 'alphabetic';
    const bevel = o.bevel ?? 1.2;
    if (bevel > 0 && 'filter' in ctx) ctx.filter = `blur(${bevel / scale}px)`;
    ctx.fillText(s, pad, pad + asc);
    const px = ctx.getImageData(0, 0, W, H).data;
    // A bevel even without canvas filters: a 3×3 box blur.
    const raw = new Float32Array(W * H);
    for (let i = 0; i < W * H; i++) raw[i] = px[i * 4] / 255;
    const hs = new Float64Array(W * H);
    const depth = o.depth ?? 0.06;
    for (let z = 0; z < H; z++) for (let x = 0; x < W; x++) {
      let acc = 0, k = 0;
      for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) {
        const xx = x + dx, zz = z + dz;
        if (xx >= 0 && zz >= 0 && xx < W && zz < H) { acc += raw[zz * W + xx]; k++; }
      }
      const v = acc / k;
      hs[z * W + x] = v * v * (3 - 2 * v) * depth * (W / Math.max(W, H));
    }
    // Aspect: the heightfield spans −0.5…0.5 in both X and Z; squash Z via the instance (sz = H / W).
    const mesh3 = mesh.heightfield(W, H, hs, { skip: 1e-4 * depth });
    mesh3.aspect = H / W;
    return mesh3;
  },
  /** Your own geometry: xyz positions and normals per vertex, three indices per triangle, optional uv pairs. */
  custom(positions: ArrayLike<number>, normals: ArrayLike<number>, indices: ArrayLike<number>, uvs?: ArrayLike<number>): Mesh3D {
    const w = core();
    const id = w.ak_mesh_custom(positions.length / 3, indices.length / 3);
    const mem = W!.memory.buffer;
    new Float32Array(mem, w.ak_mesh_ptr(id, 0), positions.length).set(positions);
    new Float32Array(mem, w.ak_mesh_ptr(id, 1), normals.length).set(normals);
    new Uint32Array(mem, w.ak_mesh_ptr(id, 2), indices.length).set(indices);
    if (uvs) new Float32Array(mem, w.ak_mesh_ptr(id, 5), (positions.length / 3) * 2).set(uvs);
    return new Mesh3D(id);
  },
};

export interface InstanceInit {
  x?: number;
  y?: number;
  z?: number;
  /** Euler rotation in degrees (applied X, then Y, then Z). */
  rx?: number;
  ry?: number;
  rz?: number;
  scale?: number;
  sx?: number;
  sy?: number;
  sz?: number;
  color?: string | V3;
}

/** Per-instance transforms and colours for one draw. */
export class Instances {
  readonly data: Float32Array;
  constructor(public count: number) {
    created.instances++;
    this.data = new Float32Array(count * STRIDE);
    for (let i = 0; i < count; i++) this.set(i, {});
  }
  /** Set instance `i` (unspecified fields default: origin, no rotation, scale 1, white). */
  set(i: number, o: InstanceInit): this {
    const d = this.data, b = i * STRIDE, r = Math.PI / 180;
    const [cx, sx_] = [Math.cos((o.rx ?? 0) * r), Math.sin((o.rx ?? 0) * r)];
    const [cy, sy_] = [Math.cos((o.ry ?? 0) * r), Math.sin((o.ry ?? 0) * r)];
    const [cz, sz_] = [Math.cos((o.rz ?? 0) * r), Math.sin((o.rz ?? 0) * r)];
    const s = o.scale ?? 1, kx = (o.sx ?? 1) * s, ky = (o.sy ?? 1) * s, kz = (o.sz ?? 1) * s;
    // R = Rz · Ry · Rx (columns), then scale per axis.
    const m00 = cz * cy, m10 = sz_ * cy, m20 = -sy_;
    const m01 = cz * sy_ * sx_ - sz_ * cx, m11 = sz_ * sy_ * sx_ + cz * cx, m21 = cy * sx_;
    const m02 = cz * sy_ * cx + sz_ * sx_, m12 = sz_ * sy_ * cx - cz * sx_, m22 = cy * cx;
    d[b] = m00 * kx; d[b + 1] = m10 * kx; d[b + 2] = m20 * kx;
    d[b + 3] = m01 * ky; d[b + 4] = m11 * ky; d[b + 5] = m21 * ky;
    d[b + 6] = m02 * kz; d[b + 7] = m12 * kz; d[b + 8] = m22 * kz;
    d[b + 9] = o.x ?? 0; d[b + 10] = o.y ?? 0; d[b + 11] = o.z ?? 0;
    const c = rgb(o.color);
    d[b + 12] = c[0]; d[b + 13] = c[1]; d[b + 14] = c[2];
    return this;
  }
}

/** Create an instance buffer (one instance = identity, white, unless set). */
export function instances(count: number, init?: (i: number) => InstanceInit): Instances {
  const n = new Instances(count);
  if (init) for (let i = 0; i < count; i++) n.set(i, init(i));
  return n;
}

export interface MaterialOptions {
  ambient?: number;
  diffuse?: number;
  specular?: number;
  shininess?: number;
  rim?: number;
  /** Unlit self-illumination (×base colour): glows under the film grade's bloom. */
  emissive?: number;
  /** Chrome: mirror reflection of the sky/horizon/ground environment (0–1). */
  metal?: number;
  /** Thin-film rainbow sheen by viewing angle (0–1; tints reflections), and its hue offset. */
  iridescence?: number;
  hue?: number;
  /**
   * Glowing grid lines in the mesh's UV space (antialiased): intensity, colour,
   * cells along `[u, v]` (0 = no lines that way; u runs along tubes/knots, v around them; each cube face is 0–1),
   * and half-width in cells. Pair with a dark body and the film grade's bloom.
   */
  lines?: number;
  /** What `lines` draws: `'grid'` (default), `'hex'` cells, `'dots'`, or `'circuit'` traces with pads — in `grid` cells. */
  pattern?: 'grid' | 'hex' | 'dots' | 'circuit';
  lineColor?: string | V3;
  grid?: [number, number];
  lineWidth?: number;
  /**
   * Light pulses travelling along u in `lineColor`: intensity, pulses per unit u,
   * and phase (animate it, e.g. `pulsePhase: t * 0.8`). `pulseDir` offsets each
   * instance's phase by `dot(position, pulseDir)`, so a pulse sweeps through a
   * field of instances as a wave.
   */
  pulse?: number;
  pulseCount?: number;
  pulsePhase?: number;
  pulseDir?: V3;
  /**
   * An image from `texture()`, mapped by the mesh's UVs (× `textureRepeat`).
   * `'albedo'` (default) tints the lit colour; `'emissive'` adds it unlit ×
   * `textureGain` — screens, signs, holograms.
   */
  texture?: Texture3D;
  textureMode?: 'albedo' | 'emissive';
  textureRepeat?: [number, number];
  textureGain?: number;
  /**
   * Texture filtering. `'linear'` (default): one bilinear tap from the mip level that
   * matches the larger side of each pixel's footprint. `'aniso'`: anisotropic — the mip
   * follows the smaller side (up to 8× finer) and up to 4 taps are averaged along the
   * longer one, so ground and roads seen at grazing angles stay sharp instead of
   * blurring. Costs more per pixel where the footprint is stretched.
   */
  textureFilter?: 'linear' | 'aniso';
  /**
   * How much the render's `fog` / `groundFog` applies to this material (default 1;
   * 0 = none): sky domes and backdrops painted with their own haze.
   */
  fog?: number;
}
export interface LightOptions {
  key?: V3;
  keyColor?: string | V3;
  fill?: V3;
  fillColor?: string | V3;
  rimColor?: string | V3;
  /** Fog from `[near, far]` distance to `fogColor` (default: the background). */
  fog?: [number, number];
  fogColor?: string | V3;
  /** Environment reflected by metal materials. */
  sky?: string | V3;
  horizon?: string | V3;
  ground?: string | V3;
  /** Colour of sharp studio light strips in the environment: gives chrome crisp highlights. */
  strips?: string | V3;
  /** Ground fog in `fogColor`: full below `bottom`, none above `top` (world y), reaching full strength over `range` units of distance. */
  groundFog?: { bottom: number; top: number; range: number };
  /**
   * Point lights (up to 8): coloured pools of light from neon signs, trails, sparks.
   * `radius` sets the falloff (r² / (r² + d²), faded out by 4·radius), `intensity` its strength.
   */
  lights?: { position: V3; color?: string | V3; radius?: number; intensity?: number }[];
}
/**
 * Light points drawn into the 3D frame: stars, dust, sparks, data. They add
 * light, are hidden behind solid geometry drawn before them, and can streak
 * along their screen motion (warp-speed stars) using the camera from the
 * previous render.
 */
export interface PointsItem {
  /** xyz per point (world space), or a live `emitter()` / `pointCloud()` (its positions, and colours when it has them). */
  points: ArrayLike<number> | { positions: Float32Array; colors?: Float32Array; shade?: () => void };
  /** rgb per point (0–1, may exceed 1 for hot points); default white. */
  colors?: ArrayLike<number>;
  /** World-space radius (default 2). */
  size?: number;
  /** Brightness (default 1). */
  intensity?: number;
  /** Streak length × the screen motion since the previous render (0 = round; 1 = shutter-accurate; 4 = warp). */
  streak?: number;
  /** Fade out between these camera distances. */
  fog?: [number, number];
}

export interface DrawItem {
  mesh: Mesh3D;
  /** Instances, or omit for one untransformed white instance. */
  instances?: Instances;
  material?: MaterialOptions;
  /** Back-face culling (default true). */
  cull?: boolean;
  /**
   * `'opaque'` (default) or `'additive'`: added to the frame as light (beams,
   * holograms, glowing wire), depth-tested against what is drawn but not
   * occluding. Draw additive items after the opaque ones.
   */
  blend?: 'opaque' | 'additive';
  /** This item is the glossy surface for the render's `mirror` option (e.g. the floor plane). */
  mirror?: boolean;
  /** Leave this item out of the mirrored pass (default: everything opaque reflects). */
  reflect?: boolean;
  /** Cast shadows when the render has `shadows` (default true for opaque items). */
  castShadow?: boolean;
}
type CanvasLike = { width: number; height: number; getContext(t: '2d'): CanvasRenderingContext2D | null };

export class Renderer3D {
  width: number;
  height: number;
  /** Triangles rasterised by the last render. */
  triangles = 0;
  /** Milliseconds the last render took. */
  lastMs = 0;
  /** Milliseconds per pass of the last render: `setup` (clear + shadow map), `mirror`, `opaque`, `sky`, `light` (additive + points), `dof`, `motionBlur`, `output`. Where to optimise. */
  stats: Record<string, number> = {};
  private id: number;
  private img: ImageData | null = null;
  private single = new Instances(1);

  constructor(private target: CanvasLike | { width: number; height: number }) {
    this.width = target.width;
    this.height = target.height;
    this.id = core().ak_raster_create(this.width, this.height);
    track(this);
  }

  resize(w: number, h: number): this {
    this.width = w;
    this.height = h;
    core().ak_raster_resize(this.id, w, h);
    this.img = null;
    return this;
  }

  /**
   * Render `items` through `scene`'s camera. Draws into the target canvas
   * (when it has one) and returns the RGBA pixels (a view into WebAssembly
   * memory, valid until the next call).
   */
  render(scene: Scene3D, items: (DrawItem | PointsItem)[], o: LightOptions & {
    background?: string;
    transparent?: boolean;
    /**
     * A mirror floor at height `y`: the scene is drawn reflected, then items marked
     * `mirror: true` are blended over it with `strength` (0–1, default 0.5). The
     * reflection renders at half resolution (`resolution: 'full'` for sharp ones).
     */
    mirror?: { y: number; strength?: number; resolution?: 'half' | 'full' };
    /**
     * Camera motion blur from the depth buffer, against the camera of the previous
     * `render()`: smooth, ghost-free blur for fast flights in one pass. `shutter` 0–1
     * of a frame (default 0.5), `max` caps the blur in px (default 6% of the width).
     * Use it with composition `motionBlur: 1` (one render per frame); call
     * `resetMotion()` after a cut.
     */
    motionBlur?: { shutter?: number; max?: number } | boolean;
    /**
     * The camera one frame earlier, as `scene.viewProjection().matrix`. Motion blur and
     * point streaks use it instead of the previous `render()` call — needed when not every
     * frame is drawn (previews, `--review`, parallel slices).
     */
    prevCamera?: number[];
    /**
     * `'near'`: draw opaque items nearest first (by the far side of each item's bounds,
     * so a sky dome around the camera goes last), the mirror pass too. Hidden pixels are
     * then rejected by the depth test instead of shaded and overwritten. The output is
     * identical to list order, exact depth ties included (the item listed first still wins).
     */
    sort?: 'near';
    /**
     * Paint the background with the environment (`sky` / `horizon` / `ground` gradient, meeting the
     * fog at the horizon) instead of a flat colour; `sun` adds a sun disc + halo towards `key`
     * (angular size, ~1–4). Drawn after opaque geometry, before additive items and points.
     */
    skybox?: boolean | { sun?: number };
    /**
     * Depth of field: `focus` = distance from the camera that is sharp, `aperture` = blur in
     * px for things at infinity (≈ 8–40), `max` caps the blur (default 3% of the width).
     * Applied after everything is drawn (points too), before motion blur.
     */
    dof?: { focus: number; aperture?: number; max?: number };
    /**
     * Key-light shadows: opaque items cast shadows (a shadow map along `key`, covering a
     * box of half-size `extent` around `center`, default the camera target). `size`
     * texels (default 2048), `strength` 0–1 (default 0.8), depth `bias` (default 0.0015).
     * Items with `castShadow: false` don't cast.
     */
    shadows?: { extent: number; center?: V3; size?: number; strength?: number; bias?: number };
  } = {}): Uint8ClampedArray {
    const t0 = performance.now();
    const stats: Record<string, number> = (this.stats = {});
    let lap = t0;
    const pass = (name: string) => { const now = performance.now(); stats[name] = (stats[name] ?? 0) + now - lap; addPass(name, now - lap); lap = now; };
    const w = core();
    const bg = rgb(o.background, [0, 0, 0]);
    const hex = (Math.round(bg[0] * 255) << 16) | (Math.round(bg[1] * 255) << 8) | Math.round(bg[2] * 255);
    w.ak_raster_clear(this.id, hex, o.transparent ? 0 : 255);
    const sw = scene.width, sh = scene.height;
    scene.width = this.width; scene.height = this.height;
    const { matrix, eye } = scene.viewProjection();
    scene.width = sw; scene.height = sh;
    this.triangles = 0;
    const drawMesh = (it: DrawItem, mat: number[], ey: number[], extra: number, gloss = 0.5) => {
      const inst = it.instances ?? this.single;
      const ptr = w.ak_raster_instances(this.id, inst.count);
      new Float32Array(W!.memory.buffer, ptr, inst.count * STRIDE).set(inst.data.subarray(0, inst.count * STRIDE));
      const m = it.material ?? {};
      const kc = rgb(o.keyColor), fc = rgb(o.fillColor, [0.25, 0.28, 0.35]), rc = rgb(o.rimColor), fog = o.fog ?? [0, 0], fogc = rgb(o.fogColor, bg);
      const vals = [...mat, ...ey, m.ambient ?? 0.22, m.diffuse ?? 0.85, m.specular ?? 0.5, m.shininess ?? 40, m.rim ?? 0.35,
        ...(o.key ?? [0.4, 0.8, 0.6]), ...kc, ...(o.fill ?? [-0.6, -0.2, 0.4]), ...fc, ...rc, fog[0], fog[1], ...fogc,
        m.emissive ?? 0, m.metal ?? 0, m.iridescence ?? 0, m.hue ?? 0, ...rgb(o.sky, [0.25, 0.3, 0.55]), ...rgb(o.horizon, [1, 0.85, 0.7]), ...rgb(o.ground, [0.05, 0.05, 0.07]),
        m.lines ?? 0, ...rgb(m.lineColor, [1, 1, 1]), ...(m.grid ?? [8, 8]), m.lineWidth ?? 0.04,
        m.pulse ?? 0, m.pulseCount ?? 1, m.pulsePhase ?? 0, ...(m.pulseDir ?? [0, 0, 0]), ...rgb(o.strips, [0, 0, 0]),
        m.texture ? m.texture.id + 1 : 0, m.texture ? (m.textureMode === 'emissive' ? 2 : 1) + (m.textureFilter === 'aniso' ? 4 : 0) : 0, ...(m.textureRepeat ?? [1, 1]), m.textureGain ?? 1, gloss,
        o.groundFog?.bottom ?? 0, o.groundFog?.top ?? 0, o.groundFog?.range ?? 0,
        ({ grid: 0, hex: 1, dots: 2, circuit: 3 } as const)[m.pattern ?? 'grid'] ?? 0,
        m.fog ?? 1,
        ...pointLights];
      const at = f64Buf(vals.length); // may grow memory: reserve before touching F64
      F64.set(vals, at);
      const flags = (it.cull === false ? 0 : 1) | (it.blend === 'additive' ? 2 : 0) | extra;
      this.triangles += w.ak_raster_draw(this.id, it.mesh.id, inst.count, flags, vals.length);
    };
    const meshes = items.filter((it): it is DrawItem => !('points' in it));
    /**
     * Opaque items in draw order: list order, or nearest first with `sort: 'near'`
     * (flag bit 4 + list index: the core resolves depth ties by list index, so the
     * pixels match list order).
     */
    const ordered = (list: DrawItem[], ey: number[]): Array<[DrawItem, number]> => {
      const out = list.map((it): [DrawItem, number] => [it, 0]);
      if (o.sort !== 'near' || list.length < 2 || items.length >= 0xfffe) return out;
      const keys = new Map<DrawItem, number>();
      for (const it of list) keys.set(it, farSide(it, ey));
      for (const e of out) e[1] = 16 | (items.indexOf(e[0]) << 8);
      return out.sort((a, b) => keys.get(a[0])! - keys.get(b[0])!);
    };
    const farSide = (it: DrawItem, ey: number[]): number => {
      const b = meshBounds(it.mesh);
      const inst = it.instances ?? this.single, d = inst.data;
      let far = 0;
      for (let i = 0, o = 0; i < inst.count; i++, o += STRIDE) {
        const x = d[o] * b[0] + d[o + 3] * b[1] + d[o + 6] * b[2] + d[o + 9] - ey[0];
        const y = d[o + 1] * b[0] + d[o + 4] * b[1] + d[o + 7] * b[2] + d[o + 10] - ey[1];
        const z = d[o + 2] * b[0] + d[o + 5] * b[1] + d[o + 8] * b[2] + d[o + 11] - ey[2];
        const s2 = Math.max(d[o] * d[o] + d[o + 1] * d[o + 1] + d[o + 2] * d[o + 2], d[o + 3] * d[o + 3] + d[o + 4] * d[o + 4] + d[o + 5] * d[o + 5], d[o + 6] * d[o + 6] + d[o + 7] * d[o + 7] + d[o + 8] * d[o + 8]);
        const f = Math.sqrt(x * x + y * y + z * z) + b[3] * Math.sqrt(s2);
        if (f > far) far = f;
      }
      return far;
    };
    const pls = (o.lights ?? []).slice(0, 8);
    const pointLights: number[] = [pls.length];
    for (const pl of pls) pointLights.push(...pl.position, ...rgb(pl.color), pl.radius ?? 500, pl.intensity ?? 1);
    if (o.shadows) {
      // Orthographic light camera looking along −key at the centre.
      const sh = o.shadows, e = sh.extent;
      const k = o.key ?? [0.4, 0.8, 0.6], kl = Math.hypot(k[0], k[1], k[2]) || 1, d = [k[0] / kl, k[1] / kl, k[2] / kl];
      const c = sh.center ?? [scene.target.x, scene.target.y, scene.target.z];
      const ey = [c[0] + d[0] * e * 2, c[1] + d[1] * e * 2, c[2] + d[2] * e * 2];
      const up = Math.abs(d[1]) > 0.99 ? [0, 0, 1] : [0, 1, 0];
      // View basis: f = −d (towards the centre), r = f × up, u = r × f.
      const f = [-d[0], -d[1], -d[2]];
      let r = [f[1] * up[2] - f[2] * up[1], f[2] * up[0] - f[0] * up[2], f[0] * up[1] - f[1] * up[0]];
      const rl = Math.hypot(r[0], r[1], r[2]) || 1; r = r.map((v) => v / rl);
      const u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
      const dotv = (a: number[], b: number[]) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
      // Ortho: x,y in [−e, e], view depth 0…4e → NDC (column-major, GL conventions).
      const near = 0, far = e * 4, sx = 1 / e, sz = -2 / (far - near), tz = -(far + near) / (far - near);
      const lvp = [
        sx * r[0], sx * u[0] * 0 + u[0] * sx, sz * -f[0], 0,
        sx * r[1], u[1] * sx, sz * -f[1], 0,
        sx * r[2], u[2] * sx, sz * -f[2], 0,
        -sx * dotv(r, ey), -sx * dotv(u, ey), sz * dotv(f, ey) + tz, 1,
      ];
      // (Row 1 is u·sx; row 2 maps view depth −f·(p − eye) to NDC.)
      for (let col = 0; col < 3; col++) lvp[col * 4 + 1] = u[col] * sx;
      const at = f64Buf(16);
      F64.set(lvp, at);
      w.ak_raster_shadow_begin(this.id, sh.size ?? 2048, sh.bias ?? 0.0015, sh.strength ?? 0.8);
      for (const it of meshes) {
        if (it.castShadow === false || it.blend === 'additive' || it.mirror) continue;
        const inst = it.instances ?? this.single;
        const ptr = w.ak_raster_instances(this.id, inst.count);
        new Float32Array(W!.memory.buffer, ptr, inst.count * STRIDE).set(inst.data.subarray(0, inst.count * STRIDE));
        w.ak_raster_shadow_draw(this.id, it.mesh.id, inst.count);
      }
    }
    pass('setup');
    const light = (it: DrawItem | PointsItem) => 'points' in it || it.blend === 'additive';
    // Half-res reflections render after the opaque scene, into the pixels it leaves
    // uncovered only (the same picture as reflecting first and drawing over it).
    const after = !!o.mirror && o.mirror.resolution !== 'full';
    const reflect = (mirror: { y: number }) => {
      // The scene reflected about y = mirror.y (winding flips) into a half-res target,
      // or (full resolution) into the frame before the scene, then a depth clear;
      // the glossy surface is blended over the reflection where it shows.
      const Y = mirror.y;
      const mr = matrix.slice();
      for (let r = 0; r < 4; r++) { mr[4 + r] = -matrix[4 + r]; mr[12 + r] = matrix[12 + r] + 2 * Y * matrix[4 + r]; }
      const er = [eye[0], 2 * Y - eye[1], eye[2]];
      if (after) w.ak_raster_reflection_after(this.id, hex, o.transparent ? 0 : 255);
      for (const [it, flags] of ordered(meshes.filter((it) => !it.mirror && it.reflect !== false && it.blend !== 'additive'), er)) drawMesh(it, mr, er, 8 | flags);
      if (after) w.ak_raster_reflection_end(this.id);
      else w.ak_raster_clear_depth(this.id);
      pass('mirror');
    };
    if (o.mirror && !after) reflect(o.mirror);
    const opaque = meshes.filter((it) => !light(it) && !(o.mirror && it.mirror));
    for (const [it, flags] of ordered(opaque, eye)) drawMesh(it, matrix, eye, flags);
    pass('opaque');
    if (o.mirror && after) reflect(o.mirror);
    if (o.mirror) {
      // The glossy surface after the opaque scene: the depth test skips floor
      // pixels the scene covers, so only visible floor is shaded and blended.
      for (const it of meshes) if (it.mirror) drawMesh(it, matrix, eye, 4, o.mirror.strength ?? 0.5);
      pass('mirror');
    }
    if (o.skybox) {
      const sb = o.skybox === true ? {} : o.skybox;
      const vals = [...matrix, ...(o.key ?? [0.4, 0.8, 0.6]), ...rgb(o.keyColor), ...rgb(o.sky, [0.25, 0.3, 0.55]), ...rgb(o.horizon, [1, 0.85, 0.7]), ...rgb(o.ground, [0.05, 0.05, 0.07]), ...rgb(o.fogColor, bg), ...(o.fog ?? [0, 0])];
      const at = f64Buf(vals.length);
      F64.set(vals, at);
      w.ak_raster_sky(this.id, sb.sun ?? 0);
      pass('sky');
    }
    // Light last: additive geometry and points add onto everything (incl. the sky).
    for (const item of items) {
      if (!light(item)) continue;
      if ('points' in item) this.drawPoints(item, matrix, eye, o.prevCamera);
      else drawMesh(item, matrix, eye, 0);
    }
    pass('light');
    if (o.dof) {
      const at = f64Buf(16);
      F64.set(matrix, at);
      w.ak_raster_dof(this.id, o.dof.focus, o.dof.aperture ?? 16, o.dof.max ?? this.width * 0.03);
      pass('dof');
    }
    const prev = o.prevCamera ?? this.prevMatrix;
    if (o.motionBlur && prev) {
      const mb = o.motionBlur === true ? {} : o.motionBlur;
      const at = f64Buf(32);
      F64.set([...matrix, ...prev], at);
      w.ak_raster_motion_blur(this.id, mb.shutter ?? 0.5, mb.max ?? this.width * 0.06);
      pass('motionBlur');
    }
    this.prevMatrix = matrix;
    const px = new Uint8ClampedArray(W!.memory.buffer, w.ak_raster_ptr(this.id), this.width * this.height * 4);
    const ctx = 'getContext' in this.target ? this.target.getContext('2d') : null;
    if (ctx) {
      if (!this.img || this.img.data.buffer !== W!.memory.buffer || (this.img.data as Uint8ClampedArray).byteOffset !== px.byteOffset) this.img = new ImageData(px, this.width, this.height);
      // Re-wrap each frame for canvases that copy (native Node canvases).
      else this.img = new ImageData(px, this.width, this.height);
      ctx.putImageData(this.img, 0, 0);
    }
    pass('output');
    this.lastMs = performance.now() - t0;
    return px;
  }

  private prevMatrix: number[] | null = null;
  private dist: Float32Array | null = null;

  /**
   * The depth buffer of the last `render()`: one value per pixel, `Infinity` where
   * nothing was drawn (sky). Without arguments it is NDC z (−1 near … 1 far), a view
   * into WebAssembly memory valid until the next call. Pass the scene that was
   * rendered to get the distance along the view direction in world units instead
   * (a buffer reused between calls). Drive post effects by depth: outlines,
   * fog, transitions that sweep from the horizon towards the camera.
   *
   * ```ts
   * const dist = r3.depth(cam);   // after r3.render(cam, items)
   * const centre = dist[(r3.height >> 1) * r3.width + (r3.width >> 1)];
   * ```
   */
  depth(scene?: Scene3D): Float32Array {
    const n = this.width * this.height;
    const z = new Float32Array(W!.memory.buffer, core().ak_raster_depth_ptr(this.id), n);
    if (!scene) return z;
    if (!this.dist || this.dist.length !== n) this.dist = new Float32Array(n);
    const out = this.dist, near = scene.near, far = scene.far;
    const a = 2 * far * near, b = far + near, c = far - near;
    for (let i = 0; i < n; i++) {
      const v = z[i];
      out[i] = v === Infinity ? Infinity : a / (b - v * c);
    }
    return out;
  }

  /** Forget the previous camera (after a cut), so motion blur and point streaks don't smear across it. */
  resetMotion(): this {
    this.prevMatrix = null;
    return this;
  }

  private drawPoints(it: PointsItem, matrix: number[], eye: number[], prevCamera?: number[]): void {
    const w = core();
    let src = it.points, cols = it.colors;
    if (!('length' in src)) {
      src.shade?.();
      cols ??= src.colors;
      src = src.positions;
    }
    // Views into wasm memory would detach if reserving the point buffer grows memory: copy them first.
    const own = (a: ArrayLike<number> | undefined) => (a instanceof Float32Array && a.buffer === W!.memory.buffer ? a.slice() : a);
    const pts = own(src)!, colors = own(cols);
    const n = Math.floor(pts.length / 3);
    if (!n) return;
    const ptr = w.ak_raster_point_buf(this.id, n);
    const mem = new Float32Array(W!.memory.buffer, ptr, n * 6);
    mem.set(pts.length === n * 3 ? pts : Array.prototype.slice.call(pts, 0, n * 3), 0);
    if (colors && colors.length >= n * 3) mem.set(colors.length === n * 3 ? colors : Array.prototype.slice.call(colors, 0, n * 3), n * 3);
    else mem.fill(1, n * 3, n * 6);
    const prev = prevCamera ?? this.prevMatrix ?? matrix;
    const fog = it.fog ?? [0, 0];
    const vals = [...matrix, ...prev, eye[0], eye[1], eye[2], it.size ?? 2, it.intensity ?? 1, it.streak ?? 0, fog[0], fog[1]];
    const at = f64Buf(vals.length);
    F64.set(vals, at);
    w.ak_raster_points(this.id, n, prevCamera || this.prevMatrix ? 1 : 0);
  }

  destroy(): void {
    if (this.id >= 0) core().ak_raster_free(this.id);
    this.id = -1;
  }
  kill(): void {
    this.destroy();
  }
  revert(): void {
    this.destroy();
  }
}

/** A solid 3D renderer for a canvas (or an offscreen `{ width, height }` target). */
export function renderer3d(target: CanvasLike | { width: number; height: number }): Renderer3D {
  return new Renderer3D(target);
}
