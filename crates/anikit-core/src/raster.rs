//! A small, fast software 3D renderer: z-buffered, lit, instanced triangles.
//!
//! Meshes (positions, normals, UVs, triangle indices) are drawn many times
//! per call with per-instance affine transforms and colours. Triangles are
//! clipped against the near plane, then shaded *per pixel* (perspective-
//! correct normal, position and UV) after the depth test: Blinn–Phong key
//! and fill lights, a fresnel rim, chrome reflections of a procedural studio
//! environment, a thin-film sheen, glowing UV grid lines and light pulses that
//! travel across the surface, and depth fog. Colours are tone-mapped so hot
//! values roll off towards white instead of clipping per channel.

use alloc::vec;
use alloc::vec::Vec;

/// Column-major 4×4 matrix (same layout as [`crate::space::Mat4`], in f32).
pub type M4 = [f32; 16];

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub ambient: f32,
    pub diffuse: f32,
    pub specular: f32,
    pub shininess: f32,
    /// Fresnel-style edge light.
    pub rim: f32,
    /// Self-illumination: base colour × emissive is added unlit (glows under bloom).
    pub emissive: f32,
    /// Mirror reflection of the procedural environment (0 = matte, 1 = chrome).
    pub metal: f32,
    /// Thin-film rainbow sheen that shifts with viewing angle (tints reflections), and its hue offset.
    pub iridescence: f32,
    pub hue: f32,
    /// Glowing grid lines in the mesh's UV space: intensity, colour, cells along u and v,
    /// and half-width in cells. Antialiased; dense grids fade to their average.
    pub lines: f32,
    pub line_color: [f32; 3],
    pub grid: [f32; 2],
    pub line_width: f32,
    /// Light pulses travelling along u (in `line_color`): intensity, pulses per unit of u,
    /// phase (animate it), and a per-instance phase offset `dot(position, pulse_dir)` so a
    /// pulse can sweep through a field of instances as a wave.
    pub pulse: f32,
    pub pulse_count: f32,
    pub pulse_phase: f32,
    pub pulse_dir: [f32; 3],
}

impl Default for Material {
    fn default() -> Self {
        Self {
            ambient: 0.22,
            diffuse: 0.85,
            specular: 0.5,
            shininess: 40.0,
            rim: 0.35,
            emissive: 0.0,
            metal: 0.0,
            iridescence: 0.0,
            hue: 0.0,
            lines: 0.0,
            line_color: [1.0, 1.0, 1.0],
            grid: [8.0, 8.0],
            line_width: 0.04,
            pulse: 0.0,
            pulse_count: 1.0,
            pulse_phase: 0.0,
            pulse_dir: [0.0; 3],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Lighting {
    /// Direction *towards* the key light (normalised internally) and its colour.
    pub key: [f32; 3],
    pub key_color: [f32; 3],
    /// Fill light direction and colour (cheap second light).
    pub fill: [f32; 3],
    pub fill_color: [f32; 3],
    pub rim_color: [f32; 3],
    /// Fog: start and end distance from the camera, and colour (0…1).
    pub fog: [f32; 2],
    pub fog_color: [f32; 3],
    /// Environment for reflections: sky (up), horizon band, ground (down).
    pub sky: [f32; 3],
    pub horizon: [f32; 3],
    pub ground: [f32; 3],
    /// Colour of sharp studio light strips in the environment (0 = none).
    pub strips: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            key: [0.4, 0.8, 0.6],
            key_color: [1.0, 1.0, 1.0],
            fill: [-0.6, -0.2, 0.4],
            fill_color: [0.25, 0.28, 0.35],
            rim_color: [1.0, 1.0, 1.0],
            fog: [0.0, f32::INFINITY],
            fog_color: [0.0; 3],
            sky: [0.25, 0.3, 0.55],
            horizon: [1.0, 0.85, 0.7],
            ground: [0.05, 0.05, 0.07],
            strips: [0.0; 3],
        }
    }
}

/// One mesh: `pos`/`nrm` are xyz per vertex, `uv` two per vertex (may be
/// empty), `idx` three per triangle.
#[derive(Debug, Default, Clone)]
pub struct Mesh {
    pub pos: Vec<f32>,
    pub nrm: Vec<f32>,
    pub uv: Vec<f32>,
    pub idx: Vec<u32>,
}

/// Per-instance data: a 3×4 affine (column-major, `[r00 r10 r20 r01 r11 r21 r02 r12 r22 tx ty tz]`) and RGB.
pub const INSTANCE_STRIDE: usize = 15;

/// Varyings per vertex: world normal (3), world position (3), uv (2).
const NV: usize = 8;

/// A screen-space vertex: pixel x/y, NDC depth, 1/w and varyings × 1/w.
#[derive(Clone, Copy, Default, Debug)]
struct Sv {
    x: f32,
    y: f32,
    z: f32,
    q: f32,
    a: [f32; NV],
}

#[derive(Debug, Default)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    /// RGBA words (little endian), `width * height`.
    pub color: Vec<u32>,
    depth: Vec<f32>,
    // Scratch per mesh draw.
    clip: Vec<[f32; 4]>,
    var: Vec<[f32; NV]>,
    sv: Vec<Sv>,
    order: Vec<u32>,
    keys: Vec<f32>,
    /// Triangles drawn by the last call (diagnostics).
    pub drawn: usize,
    queue: Queue,
    /// Pixels shaded by the last call (diagnostics).
    pub shaded: usize,
    pub tested: usize,
    pub rows: usize,
    /// Run a depth-only pre-pass before shading opaque draws, so each pixel is
    /// shaded once. Front-to-back instance order already keeps overdraw low,
    /// so this only pays off for heavily self-overlapping single meshes.
    pub prepass: bool,
}

/// Everything the pixel shader needs for one instance.
struct Shade<'a> {
    m: &'a Material,
    l: &'a Lighting,
    key: [f32; 3],
    fill: [f32; 3],
    eye: [f32; 3],
    base: [f32; 3],
    phase: f32,
    fog_on: bool,
    fog_inv: f32,
    env_on: bool,
    strips_on: bool,
    pattern_on: bool,
    /// Per-instance lighting terms: base×(ambient+emissive), base×diffuse×key, base×diffuse×fill.
    lit: [[f32; 3]; 3],
}

impl Shade<'_> {
    #[inline(always)]
    fn set_base(&mut self, b: [f32; 3]) {
        let (m, l) = (self.m, self.l);
        self.base = b;
        self.lit = [
            core::array::from_fn(|k| b[k] * (m.ambient + m.emissive)),
            core::array::from_fn(|k| b[k] * m.diffuse * l.key_color[k]),
            core::array::from_fn(|k| b[k] * m.diffuse * l.fill_color[k]),
        ];
    }
}

#[inline(always)]
fn norm3(v: [f32; 3]) -> [f32; 3] {
    let r = 1.0 / crate::math::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(1e-9);
    [v[0] * r, v[1] * r, v[2] * r]
}
#[inline(always)]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

use crate::simd::{dot3, V4};

/// ≈ 0.5 + 0.5·cos(2πx) per lane, without a software cosine (smoothstep of a triangle wave).
#[inline(always)]
fn wave4(x: V4) -> V4 {
    let y = (2.0 * x.fract() - 1.0).abs();
    y * y * (3.0 - 2.0 * y)
}
/// A soft peak of width ~1/k around 0 (rational, no exp).
#[inline(always)]
fn peak4(x: V4, k: f32) -> V4 {
    let t = x * k;
    let b = 1.0 / (1.0 + t * t);
    b * b
}

/// Antialiased grid-line coverage at `t × cells`, with a per-lane screen
/// footprint `fw` (UV units per pixel). Lines thinner than a pixel dim rather
/// than shimmer, and grids denser than ~2 px per cell fade to their mean.
#[inline(always)]
fn grid4(t: V4, cells: f32, hw: f32, fw: V4) -> V4 {
    let fw = fw * cells;
    let aa = fw.maxf(1e-4);
    let inv = 1.0 / aa;
    let fade = (2.0 - fw * 4.0).clamp(0.0, 1.0);
    let gain = (2.0 * hw * inv).minf(1.0) * fade;
    let f = (t * cells).fract();
    let d = f.min(1.0 - f);
    ((hw - d) * inv + 0.5).clamp(0.0, 1.0) * gain + (1.0 - fade) * (2.0 * hw).min(1.0)
}

/// Pixels waiting to be shaded: passing pixels from successive rows and
/// triangles are packed into full groups of four before shading, so small
/// triangles don't waste SIMD lanes.
#[derive(Debug, Default)]
struct Queue {
    n: usize,
    idx: [usize; 4],
    /// 1/w and varyings/w (divided at flush).
    q: [f32; 4],
    a: [[f32; 4]; NV],
    fw: [[f32; 4]; 2],
}

/// Tone-map and pack four pixels: luminance above 0.6 rolls off smoothly
/// towards 1, and channels that still exceed 1 burn towards white instead
/// of clipping per channel.
#[inline(always)]
fn pack4(c: [V4; 3]) -> [u32; 4] {
    let (r, g, b) = (c[0].maxf(0.0), c[1].maxf(0.0), c[2].maxf(0.0));
    let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let x = (l - 0.6).maxf(0.0);
    let s = V4::select(l.gtf(0.6), (0.6 + x / (1.0 + x * 2.5)) / l.maxf(1e-6), V4::splat(1.0));
    let (r, g, b) = (r * s, g * s, b * s);
    let m = r.max(g).max(b).maxf(1.0);
    let w = ((m - 1.0) * 0.2).minf(0.75);
    let inv = 1.0 / m;
    let t = |v: V4| {
        let v = v * inv;
        v + (1.0 - v) * w
    };
    V4::pack_rgb(t(r), t(g), t(b))
}

#[cfg(test)]
fn pack(c: [f32; 3]) -> u32 {
    pack4([V4::splat(c[0]), V4::splat(c[1]), V4::splat(c[2])])[0]
}

impl Shade<'_> {
    #[inline(always)]
    fn env4(&self, r: &[V4; 3]) -> [V4; 3] {
        let l = self.l;
        let up = r[1].clamp(-1.0, 1.0);
        let band = peak4(up, 3.0);
        let above = up.gtf(0.0);
        let mut e: [V4; 3] = core::array::from_fn(|k| {
            let sg = V4::select(above, V4::splat(l.sky[k]), V4::splat(l.ground[k]));
            sg + (l.horizon[k] - sg) * band
        });
        if self.strips_on {
            // Studio softboxes: a high horizontal strip and two vertical panels.
            let side = peak4(r[0] - 0.72, 10.0) + 0.6 * peak4(r[0] + 0.55, 16.0);
            let st = peak4(up - 0.55, 14.0) + V4::select(up.gtf(-0.25), side, V4::splat(0.0));
            for (k, ek) in e.iter_mut().enumerate() {
                *ek = *ek + st * l.strips[k];
            }
        }
        e
    }

    /// Shade four pixels at once.
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn px4(&self, n: [V4; 3], p: [V4; 3], u: V4, v: V4, fwu: V4, fwv: V4) -> [V4; 3] {
        let (m, l) = (self.m, self.l);
        let view = [self.eye[0] - p[0], self.eye[1] - p[1], self.eye[2] - p[2]];
        let d2 = dot3(&view, &view).maxf(1e-12);
        let rd = 1.0 / d2.sqrt();
        let vd = [view[0] * rd, view[1] * rd, view[2] * rd];
        let rn = 1.0 / dot3(&n, &n).sqrt().maxf(1e-9);
        let ndv0 = dot3(&n, &vd) * rn;
        // Normalise and flip back faces (two-sided lighting) with one scale.
        let sc = V4::select(ndv0.ltf(0.0), -rn, rn);
        let n = [n[0] * sc, n[1] * sc, n[2] * sc];
        let ndv = ndv0.abs();
        let key = [V4::splat(self.key[0]), V4::splat(self.key[1]), V4::splat(self.key[2])];
        let dk = dot3(&n, &key).maxf(0.0);
        let df = (n[0] * self.fill[0] + n[1] * self.fill[1] + n[2] * self.fill[2]).maxf(0.0);
        let h = [vd[0] + self.key[0], vd[1] + self.key[1], vd[2] + self.key[2]];
        let x = (dot3(&n, &h) / dot3(&h, &h).sqrt().maxf(1e-9)).maxf(0.0);
        // Schlick's cheap power: x / (s − s·x + x) ≈ x^s.
        let ss = m.shininess;
        let sp = V4::select(dk.gtf(0.0), x / (ss - ss * x + x) * m.specular, V4::splat(0.0));
        let fr = 1.0 - ndv;
        let rim = fr * fr * fr * m.rim;
        let (ka, kk, kf) = (&self.lit[0], &self.lit[1], &self.lit[2]);
        let mut c: [V4; 3] = core::array::from_fn(|k| ka[k] + dk * kk[k] + df * kf[k] + sp * l.key_color[k] + rim * l.rim_color[k]);
        if self.env_on {
            let r = [2.0 * ndv * n[0] - vd[0], 2.0 * ndv * n[1] - vd[1], 2.0 * ndv * n[2] - vd[2]];
            let mut e = self.env4(&r);
            if m.iridescence > 0.0 {
                // Thin film: tint the reflection with a palette indexed by the viewing angle.
                let t = m.iridescence;
                let xx = fr * 1.6 + m.hue;
                let pal = [wave4(xx), wave4(xx + 0.33), wave4(xx + 0.67)];
                for k in 0..3 {
                    e[k] = e[k] * ((1.0 - t) + pal[k] * (t * 1.7));
                    c[k] = c[k] + pal[k] * fr * (t * 0.3);
                }
            }
            if m.metal > 0.0 {
                let b = self.base;
                for k in 0..3 {
                    let refl = e[k] * (0.3 + 0.7 * b[k]) + sp * l.key_color[k];
                    c[k] = c[k] + (refl - c[k]) * m.metal;
                }
            } else {
                // Dielectric: a fresnel-weighted reflection on top.
                let f2 = fr * fr;
                let f = (0.04 + 0.96 * f2 * f2 * fr) * m.iridescence;
                for k in 0..3 {
                    c[k] = c[k] + e[k] * f;
                }
            }
        }
        if self.pattern_on {
            let band = if m.pulse > 0.0 {
                let t = (u * m.pulse_count - self.phase).fract();
                let t2 = t * t;
                let t4 = t2 * t2;
                t4 * t4 * t
            } else {
                V4::splat(0.0)
            };
            let zero = V4::splat(0.0);
            let lu = if m.lines > 0.0 && m.grid[0] > 0.0 { grid4(u, m.grid[0], m.line_width, fwu) } else { zero };
            let lv = if m.lines > 0.0 && m.grid[1] > 0.0 { grid4(v, m.grid[1], m.line_width, fwv) } else { zero };
            let e = lu.max(lv) * m.lines * (1.0 + 3.0 * band) + band * m.pulse;
            for k in 0..3 {
                c[k] = c[k] + e * m.line_color[k];
            }
        }
        if self.fog_on {
            let f = ((d2 * rd - l.fog[0]) * self.fog_inv).clamp(0.0, 1.0);
            for k in 0..3 {
                c[k] = c[k] + (l.fog_color[k] - c[k]) * f;
            }
        }
        c
    }
}

impl Raster {
    pub fn new(w: usize, h: usize) -> Self {
        let mut r = Self::default();
        r.resize(w, h);
        r
    }
    pub fn resize(&mut self, w: usize, h: usize) {
        self.width = w.max(1);
        self.height = h.max(1);
        self.color = vec![0; self.width * self.height];
        self.depth = vec![f32::INFINITY; self.width * self.height];
    }
    /// Clear colour (0xRRGGBB, or alpha in the top byte for a transparent clear) and depth.
    pub fn clear(&mut self, rgb: u32, alpha: u8) {
        let c = ((rgb >> 16) & 255) | (rgb & 0xff00) | ((rgb & 255) << 16) | ((alpha as u32) << 24);
        self.color.iter_mut().for_each(|p| *p = c);
        self.depth.iter_mut().for_each(|d| *d = f32::INFINITY);
    }

    #[inline(always)]
    fn to_screen(&self, c: [f32; 4], a: &[f32; NV]) -> Sv {
        let (hw, hh) = (self.width as f32 * 0.5, self.height as f32 * 0.5);
        let q = 1.0 / c[3];
        Sv { x: hw + c[0] * q * hw, y: hh - c[1] * q * hh, z: c[2] * q, q, a: core::array::from_fn(|k| a[k] * q) }
    }

    /// Draw `mesh` once per instance (opaque). `vp` is view × projection,
    /// `eye` the camera position (for specular, rim, reflections and fog).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, mesh: &Mesh, instances: &[f32], vp: &M4, eye: [f32; 3], mat: &Material, light: &Lighting, cull_back: bool) {
        self.draw_with(mesh, instances, vp, eye, mat, light, cull_back, Blend::Opaque);
    }

    /// [`Raster::draw`] with a blend mode.
    ///
    /// Opaque draws are shaded front to back (optionally after a depth
    /// pre-pass, see [`Raster::prepass`]), four pixels at a time. Additive draws are depth-tested against what is
    /// already drawn but don't write depth.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::needless_range_loop)]
    pub fn draw_with(&mut self, mesh: &Mesh, instances: &[f32], vp: &M4, eye: [f32; 3], mat: &Material, light: &Lighting, cull_back: bool, blend: Blend) {
        let nv = mesh.pos.len() / 3;
        let has_uv = mesh.uv.len() >= nv * 2;
        self.drawn = 0;
        self.shaded = 0;
        self.tested = 0;
        self.rows = 0;
        let mut sh = Shade {
            m: mat,
            l: light,
            key: norm3(light.key),
            fill: norm3(light.fill),
            eye,
            base: [0.0; 3],
            phase: 0.0,
            fog_on: light.fog[1].is_finite(),
            fog_inv: 1.0 / (light.fog[1] - light.fog[0]).max(1e-3),
            env_on: mat.metal > 0.0 || mat.iridescence > 0.0,
            strips_on: light.strips[0] + light.strips[1] + light.strips[2] > 0.0,
            pattern_on: mat.lines > 0.0 || mat.pulse > 0.0,
            lit: [[0.0; 3]; 3],
        };
        // Front-to-back: nearer instances fill the depth buffer first, so the
        // depth test rejects hidden pixels early.
        let count = instances.len() / INSTANCE_STRIDE;
        if count == 0 || nv == 0 {
            return;
        }
        self.order.clear();
        self.order.extend(0..count as u32);
        if count > 1 && blend == Blend::Opaque {
            let d2 = |i: u32| {
                let b = i as usize * INSTANCE_STRIDE;
                let (dx, dy, dz) = (instances[b + 9] - eye[0], instances[b + 10] - eye[1], instances[b + 11] - eye[2]);
                dx * dx + dy * dy + dz * dz
            };
            self.keys.clear();
            self.keys.extend(self.order.iter().map(|&i| d2(i)));
            let keys = &self.keys;
            self.order.sort_unstable_by(|a, b| keys[*a as usize].partial_cmp(&keys[*b as usize]).unwrap_or(core::cmp::Ordering::Equal));
        }
        // Transform and project every vertex of every instance once; both
        // passes rasterise from these.
        let total = count * nv;
        self.clip.resize(total, [0.0; 4]);
        self.var.resize(total, [0.0; NV]);
        self.sv.resize(total, Sv::default());
        for oi in 0..count {
            let ii = self.order[oi] as usize;
            let m = &instances[ii * INSTANCE_STRIDE..ii * INSTANCE_STRIDE + 12];
            let off = oi * nv;
            for v in 0..nv {
                let (px, py, pz) = (mesh.pos[v * 3], mesh.pos[v * 3 + 1], mesh.pos[v * 3 + 2]);
                let wx = m[0] * px + m[3] * py + m[6] * pz + m[9];
                let wy = m[1] * px + m[4] * py + m[7] * pz + m[10];
                let wz = m[2] * px + m[5] * py + m[8] * pz + m[11];
                let (nx, ny, nz) = (mesh.nrm[v * 3], mesh.nrm[v * 3 + 1], mesh.nrm[v * 3 + 2]);
                let (uu, vv) = if has_uv { (mesh.uv[v * 2], mesh.uv[v * 2 + 1]) } else { (0.0, 0.0) };
                let var = [
                    m[0] * nx + m[3] * ny + m[6] * nz,
                    m[1] * nx + m[4] * ny + m[7] * nz,
                    m[2] * nx + m[5] * ny + m[8] * nz,
                    wx,
                    wy,
                    wz,
                    uu,
                    vv,
                ];
                let c = [
                    vp[0] * wx + vp[4] * wy + vp[8] * wz + vp[12],
                    vp[1] * wx + vp[5] * wy + vp[9] * wz + vp[13],
                    vp[2] * wx + vp[6] * wy + vp[10] * wz + vp[14],
                    vp[3] * wx + vp[7] * wy + vp[11] * wz + vp[15],
                ];
                self.clip[off + v] = c;
                self.var[off + v] = var;
                if c[2] >= -c[3] && c[3] > 0.0 {
                    self.sv[off + v] = self.to_screen(c, &var);
                }
            }
        }
        match blend {
            Blend::Opaque => {
                let pre = self.prepass;
                if pre {
                    for oi in 0..count {
                        self.tris::<DEPTH>(mesh, oi * nv, cull_back, &sh);
                    }
                }
                for oi in 0..count {
                    let ii = self.order[oi] as usize;
                    let inst = &instances[ii * INSTANCE_STRIDE..(ii + 1) * INSTANCE_STRIDE];
                    sh.set_base([inst[12], inst[13], inst[14]]);
                    sh.phase = mat.pulse_phase + dot([inst[9], inst[10], inst[11]], mat.pulse_dir);
                    if pre {
                        self.tris::<SHADE_EQ>(mesh, oi * nv, cull_back, &sh);
                    } else {
                        self.tris::<SHADE_LT>(mesh, oi * nv, cull_back, &sh);
                    }
                }
            }
            Blend::Additive => {
                for oi in 0..count {
                    let ii = self.order[oi] as usize;
                    let inst = &instances[ii * INSTANCE_STRIDE..(ii + 1) * INSTANCE_STRIDE];
                    sh.set_base([inst[12], inst[13], inst[14]]);
                    sh.phase = mat.pulse_phase + dot([inst[9], inst[10], inst[11]], mat.pulse_dir);
                    self.tris::<ADD>(mesh, oi * nv, cull_back, &sh);
                }
            }
        }
    }

    /// Rasterise one instance's triangles (vertices at `off`), clipping
    /// against the near plane.
    #[inline]
    fn tris<const MODE: u8>(&mut self, mesh: &Mesh, off: usize, cull_back: bool, sh: &Shade) {
        for t in mesh.idx.chunks_exact(3) {
            let ix = [off + t[0] as usize, off + t[1] as usize, off + t[2] as usize];
            // Signed distance to the near plane (z = −w in clip space).
            let d = ix.map(|i| self.clip[i][2] + self.clip[i][3]);
            let inside = d.map(|x| x >= 0.0 && x.is_finite());
            if inside[0] && inside[1] && inside[2] {
                let (a, b, c) = (self.sv[ix[0]], self.sv[ix[1]], self.sv[ix[2]]);
                self.tri::<MODE>(&a, &b, &c, cull_back, sh);
                continue;
            }
            if !inside[0] && !inside[1] && !inside[2] {
                continue;
            }
            // Sutherland–Hodgman against the near plane: 1 or 2 triangles.
            let mut poly: [([f32; 4], [f32; NV]); 4] = [([0.0; 4], [0.0; NV]); 4];
            let mut np = 0;
            for e in 0..3 {
                let (i, j) = (e, (e + 1) % 3);
                let (ci, cj) = (self.clip[ix[i]], self.clip[ix[j]]);
                let (vi, vj) = (self.var[ix[i]], self.var[ix[j]]);
                if inside[i] {
                    poly[np] = (ci, vi);
                    np += 1;
                }
                if inside[i] != inside[j] {
                    let s = d[i] / (d[i] - d[j]);
                    poly[np] = (core::array::from_fn(|k| ci[k] + (cj[k] - ci[k]) * s), core::array::from_fn(|k| vi[k] + (vj[k] - vi[k]) * s));
                    np += 1;
                }
            }
            if np < 3 || poly[..np].iter().any(|p| p.0[3] <= 1e-6) {
                continue;
            }
            let s0 = self.to_screen(poly[0].0, &poly[0].1);
            for k in 1..np - 1 {
                let (s1, s2) = (self.to_screen(poly[k].0, &poly[k].1), self.to_screen(poly[k + 1].0, &poly[k + 1].1));
                self.tri::<MODE>(&s0, &s1, &s2, cull_back, sh);
            }
        }
        // The shading inputs (base colour, pulse phase) change per instance.
        if MODE != DEPTH {
            self.shaded += self.queue.n;
            flush::<MODE>(&mut self.queue, &mut self.color, sh);
        }
    }

    #[inline]
    #[allow(clippy::needless_range_loop)]
    fn tri<const MODE: u8>(&mut self, p0: &Sv, p1: &Sv, p2: &Sv, cull_back: bool, sh: &Shade) {
        let (x0, y0, x1, y1, x2, y2) = (p0.x, p0.y, p1.x, p1.y, p2.x, p2.y);
        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if area.is_nan() || area.abs() < 1e-6 || (cull_back && area > 0.0) {
            return;
        }
        let (w, h) = (self.width as isize, self.height as isize);
        let minx = (crate::math::floorf(x0.min(x1).min(x2)).max(-1.0) as isize).max(0);
        let maxx = (crate::math::ceilf(x0.max(x1).max(x2)).min(w as f32) as isize).min(w - 1);
        let miny = (crate::math::floorf(y0.min(y1).min(y2)).max(-1.0) as isize).max(0);
        let maxy = (crate::math::ceilf(y0.max(y1).max(y2)).min(h as f32) as isize).min(h - 1);
        if minx > maxx || miny > maxy {
            return;
        }
        if MODE != DEPTH {
            self.drawn += 1;
        }
        let inv = 1.0 / area;
        // Edge functions; per row, solve for the covered span directly, then
        // walk it with incrementally interpolated depth (and, when shading,
        // 1/w and varyings/w).
        let e = |ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32| (bx - ax) * (py - ay) - (by - ay) * (px - ax);
        let sgn = if area > 0.0 { 1.0 } else { -1.0 };
        let d = [-(y2 - y1) * sgn, -(y0 - y2) * sgn, -(y1 - y0) * sgn];
        let gx = [d[0] * sgn * inv, d[1] * sgn * inv, d[2] * sgn * inv];
        let lerp = |g: &[f32; 3], a: f32, b: f32, c: f32| g[0] * a + g[1] * b + g[2] * c;
        let dz = lerp(&gx, p0.z, p1.z, p2.z);
        let (mut dq, mut da, mut fwu, mut fwv) = (0.0, [0.0f32; NV], 0.0, 0.0);
        if MODE != DEPTH {
            let gy = [(x2 - x1) * inv, (x0 - x2) * inv, (x1 - x0) * inv];
            dq = lerp(&gx, p0.q, p1.q, p2.q);
            da = core::array::from_fn(|k| lerp(&gx, p0.a[k], p1.a[k], p2.a[k]));
            // UV footprint per pixel (affine estimate, constant over the triangle) for antialiasing.
            let (u0, u1, u2) = (p0.a[6] / p0.q, p1.a[6] / p1.q, p2.a[6] / p2.q);
            let (v0, v1, v2) = (p0.a[7] / p0.q, p1.a[7] / p1.q, p2.a[7] / p2.q);
            fwu = lerp(&gx, u0, u1, u2).abs() + lerp(&gy, u0, u1, u2).abs();
            fwv = lerp(&gx, v0, v1, v2).abs() + lerp(&gy, v0, v1, v2).abs();
        }
        let span = (maxx - minx) as f32;
        for py in miny..=maxy {
            let fy = py as f32 + 0.5;
            let fx = minx as f32 + 0.5;
            let wv = [e(x1, y1, x2, y2, fx, fy) * sgn, e(x2, y2, x0, y0, fx, fy) * sgn, e(x0, y0, x1, y1, fx, fy) * sgn];
            let (mut lo, mut hi) = (0.0f32, span);
            let mut empty = false;
            for k in 0..3 {
                if d[k] == 0.0 {
                    if wv[k] < 0.0 {
                        empty = true;
                    }
                } else if d[k] > 0.0 {
                    lo = lo.max(crate::math::ceilf(-wv[k] / d[k]));
                } else {
                    hi = hi.min(crate::math::floorf(-wv[k] / d[k]));
                }
            }
            if empty || lo > hi {
                continue;
            }
            let (b0, b1, b2) = ((wv[0] + d[0] * lo) * sgn * inv, (wv[1] + d[1] * lo) * sgn * inv, (wv[2] + d[2] * lo) * sgn * inv);
            let z0 = b0 * p0.z + b1 * p1.z + b2 * p2.z;
            let row = py as usize * self.width + minx as usize;
            let (a0, a1) = (row + lo as usize, row + hi as usize);
            // Depth along the span is evaluated as z0 + dz·k in every mode, so a
            // pre-pass and the shading pass agree bit for bit.
            if MODE == DEPTH {
                for (k, dp) in self.depth[a0..=a1].iter_mut().enumerate() {
                    let z = z0 + dz * k as f32;
                    if z < *dp {
                        *dp = z;
                    }
                }
                continue;
            }
            let q0 = b0 * p0.q + b1 * p1.q + b2 * p2.q;
            let s0: [f32; NV] = core::array::from_fn(|k| b0 * p0.a[k] + b1 * p1.a[k] + b2 * p2.a[k]);
            // Split borrows so the span loop keeps everything in locals.
            let (depth, color, qu) = (&mut self.depth[a0..=a1], &mut self.color, &mut self.queue);
            for (k, dp) in depth.iter_mut().enumerate() {
                let kf = k as f32;
                let z = z0 + dz * kf;
                let ok = match MODE {
                    SHADE_EQ => z <= *dp,
                    _ => z < *dp,
                };
                if !ok {
                    continue;
                }
                if MODE == SHADE_LT {
                    *dp = z;
                }
                let l = qu.n & 3;
                qu.idx[l] = a0 + k;
                qu.q[l] = q0 + dq * kf;
                for c in 0..NV {
                    qu.a[c][l] = s0[c] + da[c] * kf;
                }
                qu.fw[0][l] = fwu;
                qu.fw[1][l] = fwv;
                qu.n = l + 1;
                if l == 3 {
                    self.shaded += 4;
                    flush::<MODE>(qu, color, sh);
                }
            }
        }
    }
}

/// Shade the queued pixels (up to four) in one SIMD group and store them.
#[inline(always)]
#[allow(clippy::needless_range_loop)]
fn flush<const MODE: u8>(q: &mut Queue, color: &mut [u32], sh: &Shade) {
    let n = q.n;
    if n == 0 {
        return;
    }
    // Perspective divide for all four lanes at once.
    let iw = 1.0 / V4::from_array(q.q);
    let a: [V4; NV] = core::array::from_fn(|k| V4::from_array(q.a[k]) * iw);
    let c = sh.px4([a[0], a[1], a[2]], [a[3], a[4], a[5]], a[6], a[7], V4::from_array(q.fw[0]), V4::from_array(q.fw[1]));
    let px = pack4(c);
    for l in 0..n.min(4) {
        let i = q.idx[l];
        if let Some(dst) = color.get_mut(i) {
            *dst = if MODE == ADD { add_packed(*dst, px[l]) } else { px[l] };
        }
    }
    q.n = 0;
}

const DEPTH: u8 = 0;
const SHADE_EQ: u8 = 1;
const ADD: u8 = 2;
const SHADE_LT: u8 = 3;

/// Saturating per-channel add of two packed RGBA words (alpha kept from `dst`).
#[inline(always)]
fn add_packed(dst: u32, src: u32) -> u32 {
    let ch = |s: u32| (((dst >> s) & 255) + ((src >> s) & 255)).min(255) << s;
    ch(0) | ch(8) | ch(16) | (dst & 0xff00_0000)
}

/// How a draw call writes pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    /// Depth-tested and depth-writing; shaded once per visible pixel.
    #[default]
    Opaque,
    /// Added to the frame (light: beams, holograms, glowing wire); depth-tested, no depth write.
    Additive,
}

// ---------------------------------------------------------------------------
// Mesh builders
// ---------------------------------------------------------------------------

/// Unit cube (−0.5…0.5) with flat per-face normals.
#[allow(clippy::needless_range_loop)]
pub fn cube() -> Mesh {
    let mut m = Mesh::default();
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    for (n, u, v) in faces {
        let b = (m.pos.len() / 3) as u32;
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            for k in 0..3 {
                m.pos.push(n[k] * 0.5 + u[k] * 0.5 * su + v[k] * 0.5 * sv);
            }
            m.nrm.extend_from_slice(&n);
            m.uv.extend_from_slice(&[(su + 1.0) * 0.5, (sv + 1.0) * 0.5]);
        }
        m.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    m
}

/// A tube of radius `r` swept along a closed curve given by `f(t)`, t ∈ [0, 1).
#[allow(clippy::needless_range_loop)]
pub fn tube(f: impl Fn(f32) -> [f32; 3], r: f32, segments: usize, sides: usize) -> Mesh {
    let mut m = Mesh::default();
    let tau = core::f32::consts::TAU;
    let pts: Vec<[f32; 3]> = (0..segments).map(|i| f(i as f32 / segments as f32)).collect();
    // Parallel-transport frames, then spread the closing twist evenly along
    // the curve so the last ring lines up with the first (no seam).
    let tangent = |i: usize| {
        let q = pts[(i + 1) % segments];
        let o = pts[(i + segments - 1) % segments];
        norm3([q[0] - o[0], q[1] - o[1], q[2] - o[2]])
    };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let transport = |n: [f32; 3], t: [f32; 3]| {
        let d = dot(n, t);
        let v = [n[0] - d * t[0], n[1] - d * t[1], n[2] - d * t[2]];
        if dot(v, v) < 1e-8 { norm3([t[1], -t[0], 0.0]) } else { norm3(v) }
    };
    let t0 = tangent(0);
    let seed_n = if t0[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let mut normals = Vec::with_capacity(segments);
    let mut n = transport(seed_n, t0);
    for i in 0..segments {
        if i > 0 {
            n = transport(n, tangent(i));
        }
        normals.push(n);
    }
    // Angle between the frame carried once around the loop and the first frame.
    let carried = transport(normals[segments - 1], t0);
    let b0 = cross(t0, normals[0]);
    let twist = libm::atan2f(dot(carried, b0), dot(carried, normals[0]));
    // Rings 0..=segments and sides 0..=sides: the seams are duplicated so
    // UVs run 0…1 without wrapping (u along the curve, v around it).
    for ii in 0..=segments {
        let i = ii % segments;
        let t = tangent(i);
        let n0 = normals[i];
        let b = cross(t, n0);
        let a = -twist * i as f32 / segments as f32;
        let (ca, sa) = (libm::cosf(a), libm::sinf(a));
        let n = [n0[0] * ca + b[0] * sa, n0[1] * ca + b[1] * sa, n0[2] * ca + b[2] * sa];
        let bn = cross(t, n);
        let p = pts[i];
        for jj in 0..=sides {
            let ang = (jj % sides) as f32 / sides as f32 * tau;
            let (cj, sj) = (libm::cosf(ang), libm::sinf(ang));
            let dir = [n[0] * cj + bn[0] * sj, n[1] * cj + bn[1] * sj, n[2] * cj + bn[2] * sj];
            for k in 0..3 {
                m.pos.push(p[k] + dir[k] * r);
            }
            m.nrm.extend_from_slice(&dir);
            m.uv.extend_from_slice(&[ii as f32 / segments as f32, jj as f32 / sides as f32]);
        }
    }
    let row = sides + 1;
    for i in 0..segments {
        for j in 0..sides {
            let (a, b, c, d) = ((i * row + j) as u32, ((i + 1) * row + j) as u32, ((i + 1) * row + j + 1) as u32, (i * row + j + 1) as u32);
            m.idx.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    m
}

/// A (p, q) torus knot as a tube.
pub fn torus_knot(p: f32, q: f32, big_r: f32, r: f32, tube_r: f32, segments: usize, sides: usize) -> Mesh {
    tube(
        move |t| {
            let a = t * core::f32::consts::TAU;
            let k = big_r + r * libm::cosf(q * a);
            [k * libm::cosf(p * a), k * libm::sinf(p * a), r * 1.6 * libm::sinf(q * a)]
        },
        tube_r,
        segments,
        sides,
    )
}

/// A torus in the XY plane.
pub fn torus(big_r: f32, r: f32, segments: usize, sides: usize) -> Mesh {
    tube(move |t| { let a = t * core::f32::consts::TAU; [big_r * libm::cosf(a), big_r * libm::sinf(a), 0.0] }, r, segments, sides)
}

/// UV sphere of radius 1.
pub fn sphere(rings: usize, segments: usize) -> Mesh {
    let mut m = Mesh::default();
    let pi = core::f32::consts::PI;
    for i in 0..=rings {
        let th = i as f32 / rings as f32 * pi;
        for j in 0..=segments {
            let ph = j as f32 / segments as f32 * 2.0 * pi;
            let n = [libm::sinf(th) * libm::cosf(ph), libm::cosf(th), libm::sinf(th) * libm::sinf(ph)];
            m.pos.extend_from_slice(&n);
            m.nrm.extend_from_slice(&n);
            m.uv.extend_from_slice(&[j as f32 / segments as f32, i as f32 / rings as f32]);
        }
    }
    let s = segments + 1;
    for i in 0..rings {
        for j in 0..segments {
            let (a, b) = ((i * s + j) as u32, ((i + 1) * s + j) as u32);
            m.idx.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
    }
    m
}

/// A flat grid in the XZ plane (−0.5…0.5), facing +Y, with `nx × nz` cells
/// (UVs span 0…1 across it): floors, water, terrain bases.
pub fn plane(nx: usize, nz: usize) -> Mesh {
    heightfield(nx + 1, nz + 1, &[], f32::NEG_INFINITY)
}

/// A heightfield in the XZ plane (−0.5…0.5) from `w × h` samples (row-major,
/// row = z), heights in Y. Smooth normals from central differences. Cells
/// whose four corners are all `<= skip` are omitted (cut-outs, e.g. text
/// embossed from a bitmap). Empty `heights` gives a flat plane.
#[allow(clippy::needless_range_loop)]
pub fn heightfield(w: usize, h: usize, heights: &[f32], skip: f32) -> Mesh {
    let (w, h) = (w.max(2), h.max(2));
    let at = |x: usize, z: usize| heights.get(z.min(h - 1) * w + x.min(w - 1)).copied().unwrap_or(0.0);
    let (sx, sz) = (1.0 / (w - 1) as f32, 1.0 / (h - 1) as f32);
    let mut m = Mesh::default();
    for z in 0..h {
        for x in 0..w {
            let y = at(x, z);
            m.pos.extend_from_slice(&[x as f32 * sx - 0.5, y, z as f32 * sz - 0.5]);
            let dx = (at(x + 1, z) - at(x.saturating_sub(1), z)) / (sx * (if x == 0 || x == w - 1 { 1.0 } else { 2.0 }));
            let dz = (at(x, z + 1) - at(x, z.saturating_sub(1))) / (sz * (if z == 0 || z == h - 1 { 1.0 } else { 2.0 }));
            m.nrm.extend_from_slice(&norm3([-dx, 1.0, -dz]));
            m.uv.extend_from_slice(&[x as f32 * sx, z as f32 * sz]);
        }
    }
    for z in 0..h - 1 {
        for x in 0..w - 1 {
            if heights.len() >= w * h && at(x, z) <= skip && at(x + 1, z) <= skip && at(x, z + 1) <= skip && at(x + 1, z + 1) <= skip {
                continue;
            }
            let (a, b, c, d) = ((z * w + x) as u32, (z * w + x + 1) as u32, ((z + 1) * w + x + 1) as u32, ((z + 1) * w + x) as u32);
            // Counter-clockwise seen from +Y.
            m.idx.extend_from_slice(&[a, c, b, a, d, c]);
        }
    }
    m
}

/// A capped cylinder of radius 1 and height 1 (Y −0.5…0.5): pillars, beams,
/// discs. `u` runs around, `v` along the side.
pub fn cylinder(segments: usize, caps: bool) -> Mesh {
    let n = segments.max(3);
    let tau = core::f32::consts::TAU;
    let mut m = Mesh::default();
    for j in 0..=1 {
        for i in 0..=n {
            let a = i as f32 / n as f32 * tau;
            let (c, s) = (libm::cosf(a), libm::sinf(a));
            m.pos.extend_from_slice(&[c, j as f32 - 0.5, s]);
            m.nrm.extend_from_slice(&[c, 0.0, s]);
            m.uv.extend_from_slice(&[i as f32 / n as f32, j as f32]);
        }
    }
    let row = (n + 1) as u32;
    for i in 0..n as u32 {
        let (a, b) = (i, i + 1);
        m.idx.extend_from_slice(&[a, a + row, b + row, a, b + row, b]);
    }
    if caps {
        for (y, ny) in [(-0.5f32, -1.0f32), (0.5, 1.0)] {
            let c0 = (m.pos.len() / 3) as u32;
            m.pos.extend_from_slice(&[0.0, y, 0.0]);
            m.nrm.extend_from_slice(&[0.0, ny, 0.0]);
            m.uv.extend_from_slice(&[0.5, 0.5]);
            for i in 0..=n {
                let a = i as f32 / n as f32 * tau;
                let (c, s) = (libm::cosf(a), libm::sinf(a));
                m.pos.extend_from_slice(&[c, y, s]);
                m.nrm.extend_from_slice(&[0.0, ny, 0.0]);
                m.uv.extend_from_slice(&[0.5 + c * 0.5, 0.5 + s * 0.5]);
            }
            for i in 0..n as u32 {
                let (a, b) = (c0 + 1 + i, c0 + 2 + i);
                if ny > 0.0 {
                    m.idx.extend_from_slice(&[c0, b, a]);
                } else {
                    m.idx.extend_from_slice(&[c0, a, b]);
                }
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{look_at, mul, perspective};

    fn vp() -> M4 {
        let m = mul(&perspective(45.0, 1.0, 1.0, 100.0), &look_at([0.0, 0.0, 5.0], [0.0; 3], [0.0, 1.0, 0.0]));
        core::array::from_fn(|i| m[i] as f32)
    }
    const ID: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

    #[test]
    fn draws_a_lit_cube_with_depth() {
        let mut r = Raster::new(64, 64);
        r.clear(0x000000, 255);
        let mut inst = ID.to_vec();
        inst.extend_from_slice(&[1.0, 0.0, 0.0]);
        r.draw(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &Material::default(), &Lighting::default(), true);
        let centre = r.color[32 * 64 + 32];
        assert!(centre & 0xff > 40, "lit red face {:x}", centre);
        assert_eq!(r.color[0] & 0xffffff, 0, "background stays clear");
        assert!(r.drawn >= 2 && r.drawn <= 6, "back faces culled: {}", r.drawn);
        // A second cube behind the first is hidden by the depth test.
        let before = r.color[32 * 64 + 32];
        let mut back = ID.to_vec();
        back[11] = -2.0;
        back.extend_from_slice(&[0.0, 1.0, 0.0]);
        r.draw(&cube(), &back, &vp(), [0.0, 0.0, 5.0], &Material::default(), &Lighting::default(), true);
        assert_eq!(r.color[32 * 64 + 32], before);
    }

    #[test]
    fn mesh_builders_are_consistent() {
        let hf: Vec<f32> = (0..64).map(|i| if i % 3 == 0 { 0.0 } else { 0.2 }).collect();
        for m in [cube(), sphere(8, 12), tube(|t| [libm::cosf(t * 6.283), libm::sinf(t * 6.283), 0.0], 0.2, 32, 8), plane(4, 3), cylinder(12, true), heightfield(8, 8, &hf, 0.0)] {
            assert_eq!(m.pos.len(), m.nrm.len());
            assert_eq!(m.uv.len() * 3, m.pos.len() * 2);
            assert_eq!(m.idx.len() % 3, 0);
            assert!(m.idx.iter().all(|&i| (i as usize) < m.pos.len() / 3));
        }
    }

    #[test]
    fn clips_at_the_near_plane() {
        // Camera inside a big cube: every triangle crosses or sits near the
        // near plane, yet the walls must still cover the view.
        let mut r = Raster::new(32, 32);
        r.clear(0x000000, 255);
        let m = mul(&perspective(90.0, 1.0, 0.1, 100.0), &look_at([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]));
        let vp: M4 = core::array::from_fn(|i| m[i] as f32);
        let mut inst: Vec<f32> = [3.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.3].to_vec();
        inst.extend_from_slice(&[0.5, 0.5, 0.5]);
        r.draw(&cube(), &inst, &vp, [0.0; 3], &Material::default(), &Lighting::default(), false);
        let covered = r.color.iter().filter(|c| **c & 0xffffff != 0).count();
        assert_eq!(covered, 32 * 32, "inside walls cover the screen");
    }

    #[test]
    fn grid_lines_and_tone_mapping() {
        let mut r = Raster::new(64, 64);
        r.clear(0x000000, 255);
        let mut inst = ID.to_vec();
        inst.extend_from_slice(&[0.05, 0.05, 0.05]);
        let mat = Material { lines: 3.0, line_color: [0.2, 0.6, 1.0], grid: [4.0, 4.0], line_width: 0.06, ..Material::default() };
        r.draw(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &mat, &Lighting::default(), true);
        let blues = r.color.iter().filter(|c| (**c >> 16) & 255 > 200).count();
        let darks = r.color.iter().filter(|c| **c & 0xffffff != 0 && (**c >> 16) & 255 < 60).count();
        assert!(blues > 40 && darks > 100, "lines {blues}, body {darks}");
        // Over-bright colours keep their hue ordering instead of clipping to equal channels.
        let p = pack([4.0, 1.0, 0.2]);
        assert!((p & 255) == 255 && ((p >> 8) & 255) < 250 && ((p >> 16) & 255) < ((p >> 8) & 255));
    }

    #[test]
    fn plane_and_cylinder_face_outwards() {
        // Seen from above, the plane is front-facing (not culled).
        let mut r = Raster::new(32, 32);
        r.clear(0, 255);
        let m = mul(&perspective(60.0, 1.0, 0.1, 100.0), &look_at([0.0, 3.0, 0.01], [0.0; 3], [0.0, 1.0, 0.0]));
        let vp: M4 = core::array::from_fn(|i| m[i] as f32);
        let mut inst = [4.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0].to_vec();
        inst.extend_from_slice(&[1.0, 1.0, 1.0]);
        r.draw(&plane(2, 2), &inst, &vp, [0.0, 3.0, 0.0], &Material::default(), &Lighting::default(), true);
        assert!(r.color[16 * 32 + 16] & 0xff > 40);
        // Cylinder side and caps survive back-face culling from the side and from above.
        for eye in [[0.0, 0.0, 4.0], [0.0, 4.0, 0.01]] {
            let mut r = Raster::new(32, 32);
            r.clear(0, 255);
            let m = mul(&perspective(60.0, 1.0, 0.1, 100.0), &look_at([eye[0] as f64, eye[1] as f64, eye[2] as f64], [0.0; 3], [0.0, 1.0, 0.0]));
            let vp: M4 = core::array::from_fn(|i| m[i] as f32);
            let mut inst = ID.to_vec();
            inst.extend_from_slice(&[1.0, 1.0, 1.0]);
            r.draw(&cylinder(24, true), &inst, &vp, eye, &Material::default(), &Lighting::default(), true);
            assert!(r.color[16 * 32 + 16] & 0xff > 20, "visible from {eye:?}");
        }
    }
}
