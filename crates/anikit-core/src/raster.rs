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
    /// How the draw's [`Texture`] (if any) is used: 0 none, 1 albedo (multiplies the lit
    /// colour), 2 emissive (added unlit, × `tex_gain`: screens, signs). UVs × `tex_repeat`.
    pub tex_mode: u8,
    pub tex_repeat: [f32; 2],
    pub tex_gain: f32,
    /// What `lines` draws: 0 grid lines (default), 1 hexagons, 2 dots, 3 circuit traces.
    pub pattern: u8,
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
            tex_mode: 0,
            tex_repeat: [1.0, 1.0],
            tex_gain: 1.0,
            pattern: 0,
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
    /// Ground fog: full below `[0]`, none above `[1]` (world y), reaching full
    /// strength over `[2]` units of distance (0 = off). Uses `fog_color`.
    pub ground_fog: [f32; 3],
    /// Point lights: position (3), colour (3), radius, intensity; the first
    /// `point_count` are used (up to 8). Light falls off as r² / (r² + d²) and
    /// fades out by 4·radius; they add diffuse and specular light.
    pub point_lights: [[f32; 8]; 8],
    pub point_count: u8,
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
            ground_fog: [0.0, 0.0, 0.0],
            point_lights: [[0.0; 8]; 8],
            point_count: 0,
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

/// An RGBA image with a mip chain, sampled bilinearly (wrapping) at the mip
/// level matching each pixel's screen footprint, so minified textures don't
/// shimmer.
#[derive(Debug, Default, Clone)]
pub struct Texture {
    /// (width, height, packed RGBA words) per level, largest first.
    levels: Vec<(usize, usize, Vec<u32>)>,
}

impl Texture {
    /// From 8-bit RGBA pixels, row 0 at the top (v = 0).
    pub fn from_rgba(w: usize, h: usize, data: &[u8]) -> Self {
        let (w, h) = (w.max(1), h.max(1));
        let mut base = vec![0u32; w * h];
        for (i, px) in data.chunks_exact(4).take(w * h).enumerate() {
            base[i] = px[0] as u32 | (px[1] as u32) << 8 | (px[2] as u32) << 16 | (px[3] as u32) << 24;
        }
        let mut levels = vec![(w, h, base)];
        while levels.len() < 14 {
            let (pw, ph, prev) = levels.last().unwrap();
            let (pw, ph) = (*pw, *ph);
            if pw == 1 && ph == 1 {
                break;
            }
            let (nw, nh) = ((pw / 2).max(1), (ph / 2).max(1));
            let mut next = vec![0u32; nw * nh];
            for y in 0..nh {
                for x in 0..nw {
                    let mut acc = [0u32; 4];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let c = prev[(y * 2 + dy).min(ph - 1) * pw + (x * 2 + dx).min(pw - 1)];
                        for (k, a) in acc.iter_mut().enumerate() {
                            *a += (c >> (8 * k)) & 255;
                        }
                    }
                    next[y * nw + x] = (0..4).fold(0, |o, k| o | ((acc[k] + 2) / 4) << (8 * k));
                }
            }
            levels.push((nw, nh, next));
        }
        Self { levels }
    }

    pub fn width(&self) -> usize {
        self.levels.first().map_or(0, |l| l.0)
    }
    pub fn height(&self) -> usize {
        self.levels.first().map_or(0, |l| l.1)
    }

    /// Bilinear sample (0…1 RGBA) at `u, v` (wrapping) with a footprint of
    /// `fw` base-level texels per pixel.
    #[inline]
    fn sample(&self, u: f32, v: f32, fw: f32) -> [f32; 4] {
        let Some(top) = self.levels.first() else { return [1.0; 4] };
        // floor(log2(fw)) from the float exponent.
        let lv = if fw > 1.0 { (((fw.to_bits() >> 23) & 255) as i32 - 127).clamp(0, self.levels.len() as i32 - 1) as usize } else { 0 };
        let (w, h, px) = (&self.levels[lv].0, &self.levels[lv].1, &self.levels[lv].2);
        let _ = top;
        let fx = (u - crate::math::floorf(u)) * *w as f32 - 0.5;
        let fy = (v - crate::math::floorf(v)) * *h as f32 - 0.5;
        let (x0f, y0f) = (crate::math::floorf(fx), crate::math::floorf(fy));
        let (tx, ty) = (fx - x0f, fy - y0f);
        let wrap = |i: isize, n: usize| i.rem_euclid(n as isize) as usize;
        let (x0, y0) = (wrap(x0f as isize, *w), wrap(y0f as isize, *h));
        let (x1, y1) = ((x0 + 1) % *w, (y0 + 1) % *h);
        let (a, b, c, d) = (px[y0 * w + x0], px[y0 * w + x1], px[y1 * w + x0], px[y1 * w + x1]);
        core::array::from_fn(|k| {
            let ch = |q: u32| ((q >> (8 * k)) & 255) as f32;
            let top = ch(a) + (ch(b) - ch(a)) * tx;
            let bot = ch(c) + (ch(d) - ch(c)) * tx;
            (top + (bot - top) * ty) * (1.0 / 255.0)
        })
    }
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
    /// Reflection strength for [`Blend::Gloss`] draws (0…1).
    pub gloss: f32,
    /// Cull front faces instead of back faces (mirrored cameras flip winding).
    pub flip_winding: bool,
    /// Scratch for [`Raster::motion_blur`]: frame copy, velocity + view depth, tile maxima.
    blur_src: Vec<u32>,
    vel: Vec<u32>,
    tiles: Vec<[f32; 2]>,
    /// Per-tile 16.16 sample step and start offset for the motion-blur cleanup pass.
    tile_step: Vec<[i32; 4]>,
    alpha: Vec<f32>,
    row_px: Vec<u32>,
    /// Half-resolution packed RGB written by the blur passes for [`Raster::blend_half`].
    half_px: Vec<u32>,
    /// Half-resolution target for a planar reflection pass (see [`Raster::reflection_begin`]).
    reflection: Option<alloc::boxed::Box<Raster>>,
    reflecting: bool,
    /// Depth-of-field scratch: circle of confusion per half-res pixel, and
    /// integer tap offsets per quantised radius.
    coc: Vec<f32>,
    /// Sky: which half-res grid samples border an uncovered pixel.
    sky_need: Vec<u8>,
    dof_taps: Vec<(i32, i32, f32)>,
    /// Shadow map from the key light (see [`Raster::shadow_begin`]).
    shadow_depth: Vec<f32>,
    shadow_size: usize,
    shadow_vp: M4,
    shadow_bias: f32,
    shadow_strength: f32,
    shadows_on: bool,
    /// Additive light accumulation for [`Raster::points`] (RGB f32, allocated on first use).
    light: Vec<f32>,
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
    tex: Option<&'a Texture>,
    /// Shadow map: depth, size, light view-projection, bias, strength.
    shadow: Option<(&'a [f32], usize, M4, f32, f32)>,
    /// Point lights that can reach the current instance (see `cull_lights`).
    pl: [[f32; 8]; 8],
    npl: usize,
}

impl Shade<'_> {
    /// Keep only the point lights whose reach (4 × radius, where the falloff
    /// window ends) touches the instance's bounding sphere `(c, r)`.
    #[inline]
    fn cull_lights(&mut self, c: [f32; 3], r: f32) {
        let l = self.l;
        self.npl = 0;
        for pl in &l.point_lights[..(l.point_count as usize).min(8)] {
            let (dx, dy, dz) = (pl[0] - c[0], pl[1] - c[1], pl[2] - c[2]);
            let reach = 4.0 * pl[6] + r;
            if dx * dx + dy * dy + dz * dz < reach * reach {
                self.pl[self.npl] = *pl;
                self.npl += 1;
            }
        }
    }
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

/// Procedural glowing patterns in cell space (`x`, `y` = uv × cells), with
/// footprint `fw` (cells per pixel) for antialiasing and half-width `hw`:
/// 1 hexagon edges, 2 dots, 3 circuit traces with pads.
#[inline(always)]
fn pattern4(kind: u8, x: V4, y: V4, fw: V4, hw: f32) -> V4 {
    let aa = fw.maxf(1e-4);
    // Coverage of a band of half-width hw at distance d.
    let band = |d: V4| ((V4::splat(hw) - d) / aa + 0.5).clamp(0.0, 1.0);
    // Patterns denser than ~2 px per cell fade to their mean instead of shimmering.
    let fade = (2.0 - fw * 4.0).clamp(0.0, 1.0);
    let cov = match kind {
        1 => {
            // Hexagons: nearest of two offset lattices, distance to the cell edge.
            let (sx, sy) = (1.0f32, 1.732_050_8f32);
            let (ax, ay) = ((x / sx).fract() * sx - sx * 0.5, (y / sy).fract() * sy - sy * 0.5);
            let (bxx, byy) = (((x - sx * 0.5) / sx).fract() * sx - sx * 0.5, ((y - sy * 0.5) / sy).fract() * sy - sy * 0.5);
            let use_a = (ax * ax + ay * ay).lt(bxx * bxx + byy * byy);
            let (gx, gy) = (V4::select(use_a, ax, bxx).abs(), V4::select(use_a, ay, byy).abs());
            let e = (gx * 0.5 + gy * 0.866_025_4).max(gx);
            band(0.5 - e)
        }
        2 => {
            let (fx, fy) = (x.fract() - 0.5, y.fract() - 0.5);
            let d = (fx * fx + fy * fy).sqrt();
            band(d - hw * 2.0)
        }
        _ => {
            // Circuit: each cell hashes to a horizontal or vertical trace through
            // its centre (which joins the neighbours'), some cells get a pad.
            let (cx, cy) = (x.floor(), y.floor());
            let h = ((cx * 0.1031 + cy * 0.113_69) * (cx * 0.0973 + cy * 0.1099 + 33.33)).fract();
            let (fx, fy) = ((x - cx - 0.5).abs(), (y - cy - 0.5).abs());
            let horiz = h.ltf(0.5);
            let trace = band(V4::select(horiz, fy, fx));
            let pad = band((fx * fx + fy * fy).sqrt() - hw * 2.5);
            trace.max(V4::select(h.gtf(0.8), pad, V4::splat(0.0)))
        }
    };
    cov * fade + (1.0 - fade) * (2.0 * hw).min(1.0) * 0.5
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

    /// Sample the texture for four lanes (RGBA as V4s).
    #[inline(always)]
    fn sample4(&self, t: &Texture, u: V4, v: V4, fwu: V4, fwv: V4) -> [V4; 4] {
        let m = self.m;
        let (ru, rv) = (m.tex_repeat[0], m.tex_repeat[1]);
        let (tw, th) = (t.width() as f32 * ru, t.height() as f32 * rv);
        let (u, v, fu, fv) = ((u * ru).to_array(), (v * rv).to_array(), (fwu * tw).to_array(), (fwv * th).to_array());
        let s: [[f32; 4]; 4] = core::array::from_fn(|l| t.sample(u[l], v[l], fu[l].max(fv[l])));
        core::array::from_fn(|k| V4::new(s[0][k], s[1][k], s[2][k], s[3][k]))
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
        let mut dk = dot3(&n, &key).maxf(0.0);
        if let Some((sd, size, svp, bias, strength)) = self.shadow {
            // Light-space position → shadow map texel; 2×2 PCF.
            let lw = p[0] * svp[3] + p[1] * svp[7] + p[2] * svp[11] + svp[15];
            let ilw = 1.0 / lw;
            let lx = (p[0] * svp[0] + p[1] * svp[4] + p[2] * svp[8] + svp[12]) * ilw;
            let ly = (p[0] * svp[1] + p[1] * svp[5] + p[2] * svp[9] + svp[13]) * ilw;
            let lz = (p[0] * svp[2] + p[1] * svp[6] + p[2] * svp[10] + svp[14]) * ilw - bias;
            let hs = size as f32 * 0.5;
            let (tx, ty, tz) = ((lx * hs + hs - 0.5).to_array(), (hs - ly * hs - 0.5).to_array(), lz.to_array());
            let lit: [f32; 4] = core::array::from_fn(|l| {
                let (fx, fy) = (tx[l], ty[l]);
                if !(fx > -1.0 && fy > -1.0 && fx < size as f32 && fy < size as f32) {
                    return 1.0;
                }
                let (x0, y0) = (fx.max(0.0) as usize, fy.max(0.0) as usize);
                let (x1, y1) = ((x0 + 1).min(size - 1), (y0 + 1).min(size - 1));
                let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
                let t = |x: usize, y: usize| if tz[l] <= sd[y * size + x] { 1.0 } else { 0.0 };
                let top = t(x0, y0) + (t(x1, y0) - t(x0, y0)) * ax.clamp(0.0, 1.0);
                let bot = t(x0, y1) + (t(x1, y1) - t(x0, y1)) * ax.clamp(0.0, 1.0);
                top + (bot - top) * ay.clamp(0.0, 1.0)
            });
            let k = 1.0 - (1.0 - V4::from_array(lit)) * strength;
            dk = dk * k;
        }
        let dk = dk;
        let df = (n[0] * self.fill[0] + n[1] * self.fill[1] + n[2] * self.fill[2]).maxf(0.0);
        let h = [vd[0] + self.key[0], vd[1] + self.key[1], vd[2] + self.key[2]];
        let x = (dot3(&n, &h) / dot3(&h, &h).sqrt().maxf(1e-9)).maxf(0.0);
        // Schlick's cheap power: x / (s − s·x + x) ≈ x^s.
        let ss = m.shininess;
        let sp = V4::select(dk.gtf(0.0), x / (ss - ss * x + x) * m.specular, V4::splat(0.0));
        let fr = 1.0 - ndv;
        let rim = fr * fr * fr * m.rim;
        let (ka, kk, kf) = (&self.lit[0], &self.lit[1], &self.lit[2]);
        let texel = match self.tex {
            Some(t) if m.tex_mode != 0 => Some(self.sample4(t, u, v, fwu, fwv)),
            _ => None,
        };
        let mut c: [V4; 3] = match (texel, m.tex_mode) {
            // Albedo: the texture tints the lit (diffuse + ambient) colour.
            (Some(tx), 1) => core::array::from_fn(|k| tx[k] * (dk * kk[k] + df * kf[k] + ka[k]) + sp * l.key_color[k] + rim * l.rim_color[k]),
            _ => core::array::from_fn(|k| ka[k] + dk * kk[k] + df * kf[k] + sp * l.key_color[k] + rim * l.rim_color[k]),
        };
        if self.npl > 0 {
            // Local lights: diffuse (tinted by the base / texture) + Schlick specular.
            let b = self.base;
            for pl in &self.pl[..self.npl] {
                let lv = [pl[0] - p[0], pl[1] - p[1], pl[2] - p[2]];
                let d2 = dot3(&lv, &lv).maxf(1e-6);
                let r2 = pl[6] * pl[6];
                // All four pixels beyond the falloff window: nothing to add.
                if d2.to_array().iter().all(|d| *d >= 16.0 * r2) {
                    continue;
                }
                let win = (1.0 - d2 * (1.0 / (16.0 * r2))).clamp(0.0, 1.0);
                let att = r2 / (d2 + r2) * win * win * pl[7];
                let il = 1.0 / d2.sqrt();
                let ld = [lv[0] * il, lv[1] * il, lv[2] * il];
                let ndl = dot3(&n, &ld).maxf(0.0);
                let hp = [ld[0] + vd[0], ld[1] + vd[1], ld[2] + vd[2]];
                let xh = (dot3(&n, &hp) / dot3(&hp, &hp).sqrt().maxf(1e-9)).maxf(0.0);
                let spec = V4::select(ndl.gtf(0.0), xh / (ss - ss * xh + xh) * m.specular, V4::splat(0.0));
                let dif = ndl * att * m.diffuse;
                let sat = spec * att;
                for k in 0..3 {
                    let alb = match (texel, m.tex_mode) {
                        (Some(tx), 1) => tx[k] * b[k],
                        _ => V4::splat(b[k]),
                    };
                    c[k] = c[k] + (alb * dif + sat) * pl[3 + k];
                }
            }
        }
        if let (Some(tx), 2) = (texel, m.tex_mode) {
            // Emissive: added unlit (screens, signs), scaled by alpha.
            for k in 0..3 {
                c[k] = c[k] + tx[k] * tx[3] * m.tex_gain;
            }
        }
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
            let lm = if m.lines <= 0.0 {
                zero
            } else if m.pattern == 0 {
                let lu = if m.grid[0] > 0.0 { grid4(u, m.grid[0], m.line_width, fwu) } else { zero };
                let lv = if m.grid[1] > 0.0 { grid4(v, m.grid[1], m.line_width, fwv) } else { zero };
                lu.max(lv)
            } else {
                pattern4(m.pattern, u * m.grid[0].max(1e-3), v * m.grid[1].max(1e-3), (fwu * m.grid[0]).max(fwv * m.grid[1]), m.line_width)
            };
            let e = lm * m.lines * (1.0 + 3.0 * band) + band * m.pulse;
            for k in 0..3 {
                c[k] = c[k] + e * m.line_color[k];
            }
        }
        if self.fog_on || l.ground_fog[2] > 0.0 {
            let dist = d2 * rd;
            let mut f = if self.fog_on { ((dist - l.fog[0]) * self.fog_inv).clamp(0.0, 1.0) } else { V4::splat(0.0) };
            if l.ground_fog[2] > 0.0 {
                let gf = l.ground_fog;
                let low = ((gf[1] - p[1]) * (1.0 / (gf[1] - gf[0]).max(1e-3))).clamp(0.0, 1.0);
                let g = low * low * (dist * (1.0 / gf[2])).minf(1.0);
                f = f + (1.0 - f) * g;
            }
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
        self.shadows_on = false;
    }

    /// Start a shadow map for the key light: `light_vp` is the light's
    /// (orthographic) view-projection, `size` the map's resolution. Draw the
    /// shadow casters with [`Raster::shadow_draw`], then draw the scene as
    /// usual: the key light and its highlights are dimmed by `strength` (0…1)
    /// where occluded (4-tap PCF, depth `bias` in NDC). [`Raster::clear`] ends it.
    pub fn shadow_begin(&mut self, light_vp: &M4, size: usize, bias: f32, strength: f32) {
        let size = size.clamp(16, 8192);
        self.shadow_size = size;
        self.shadow_depth.clear();
        self.shadow_depth.resize(size * size, f32::INFINITY);
        self.shadow_vp = *light_vp;
        self.shadow_bias = bias;
        self.shadow_strength = strength.clamp(0.0, 1.0);
        self.shadows_on = true;
    }

    /// Render `mesh` instances into the shadow map (depth only).
    pub fn shadow_draw(&mut self, mesh: &Mesh, instances: &[f32], cull_back: bool) {
        if !self.shadows_on {
            return;
        }
        let (w, h) = (self.width, self.height);
        let s = self.shadow_size;
        core::mem::swap(&mut self.depth, &mut self.shadow_depth);
        self.width = s;
        self.height = s;
        let vp = self.shadow_vp;
        self.draw_with(mesh, instances, &vp, [0.0; 3], &Material::default(), &Lighting::default(), cull_back, Blend::DepthOnly, None);
        self.width = w;
        self.height = h;
        core::mem::swap(&mut self.depth, &mut self.shadow_depth);
    }

    /// Reset depth only (keep the colour): draw a mirrored pass, then the real scene over it.
    pub fn clear_depth(&mut self) {
        self.depth.iter_mut().for_each(|d| *d = f32::INFINITY);
    }

    /// Start a planar reflection pass rendered at half resolution: until
    /// [`Raster::reflection_end`], [`Raster::target`] returns a half-size raster
    /// cleared to `rgb`. Reflections are seen through a glossy surface, so the
    /// softer half-res image looks right and costs a quarter of the shading.
    pub fn reflection_begin(&mut self, rgb: u32, alpha: u8) {
        let (hw, hh) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let sub = self.reflection.get_or_insert_with(Default::default);
        if sub.width != hw || sub.height != hh {
            sub.resize(hw, hh);
        }
        sub.clear(rgb, alpha);
        self.reflecting = true;
    }
    /// The raster draws should go to: the half-res reflection target during a
    /// reflection pass, otherwise this one.
    pub fn target(&mut self) -> &mut Raster {
        match (self.reflecting, self.reflection.is_some()) {
            (true, true) => self.reflection.as_deref_mut().expect("checked above"),
            _ => self,
        }
    }
    /// End the reflection pass: upsample it over the whole frame (bilinear) and
    /// clear depth, ready for the glossy surface and the scene itself.
    pub fn reflection_end(&mut self) {
        if !self.reflecting {
            return;
        }
        self.reflecting = false;
        let Some(sub) = self.reflection.take() else { return };
        let (bw, bh) = (sub.width, sub.height);
        if self.width >= 2 && self.height >= 2 {
            self.half_px.clear();
            self.half_px.extend(sub.color.iter().map(|c| c & 0x00ff_ffff));
            let mut alpha = core::mem::take(&mut self.alpha);
            alpha.clear();
            alpha.resize(bw * bh, 1.0);
            self.blend_half(bw, bh, &alpha);
            self.alpha = alpha;
        }
        self.reflection = Some(sub);
        self.clear_depth();
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
        self.draw_with(mesh, instances, vp, eye, mat, light, cull_back, Blend::Opaque, None);
    }

    /// [`Raster::draw`] with a blend mode.
    ///
    /// Opaque draws are shaded front to back (optionally after a depth
    /// pre-pass, see [`Raster::prepass`]), four pixels at a time. Additive draws are depth-tested against what is
    /// already drawn but don't write depth.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::needless_range_loop)]
    pub fn draw_with(&mut self, mesh: &Mesh, instances: &[f32], vp: &M4, eye: [f32; 3], mat: &Material, light: &Lighting, cull_back: bool, blend: Blend, tex: Option<&Texture>) {
        // The shadow map is read while the frame is written: move it out for the call.
        let shadow_depth = core::mem::take(&mut self.shadow_depth);
        self.draw_inner(mesh, instances, vp, eye, mat, light, cull_back, blend, tex, &shadow_depth);
        self.shadow_depth = shadow_depth;
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::needless_range_loop)]
    fn draw_inner(&mut self, mesh: &Mesh, instances: &[f32], vp: &M4, eye: [f32; 3], mat: &Material, light: &Lighting, cull_back: bool, blend: Blend, tex: Option<&Texture>, shadow_depth: &[f32]) {
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
            tex,
            shadow: if self.shadows_on && blend != Blend::DepthOnly { Some((shadow_depth, self.shadow_size, self.shadow_vp, self.shadow_bias, self.shadow_strength)) } else { None },
            pl: [[0.0; 8]; 8],
            npl: 0,
        };
        // Front-to-back: nearer instances fill the depth buffer first, so the
        // depth test rejects hidden pixels early.
        let count = instances.len() / INSTANCE_STRIDE;
        if count == 0 || nv == 0 {
            return;
        }
        self.order.clear();
        // Frustum-cull whole instances by bounding sphere (Gribb–Hartmann planes from vp).
        let (bc, br) = bounds(mesh);
        let row = |r: usize| [vp[r], vp[4 + r], vp[8 + r], vp[12 + r]];
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        let planes: [[f32; 4]; 6] = core::array::from_fn(|k| {
            let (a, sgn) = match k {
                0 => (r0, 1.0),
                1 => (r0, -1.0),
                2 => (r1, 1.0),
                3 => (r1, -1.0),
                4 => (r2, 1.0),
                _ => (r2, -1.0),
            };
            let p = [r3[0] + sgn * a[0], r3[1] + sgn * a[1], r3[2] + sgn * a[2], r3[3] + sgn * a[3]];
            let l = 1.0 / crate::math::sqrtf(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).max(1e-12);
            [p[0] * l, p[1] * l, p[2] * l, p[3] * l]
        });
        for i in 0..count {
            let m = &instances[i * INSTANCE_STRIDE..i * INSTANCE_STRIDE + 12];
            let c = [
                m[0] * bc[0] + m[3] * bc[1] + m[6] * bc[2] + m[9],
                m[1] * bc[0] + m[4] * bc[1] + m[7] * bc[2] + m[10],
                m[2] * bc[0] + m[5] * bc[1] + m[8] * bc[2] + m[11],
            ];
            let s2 = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).max(m[3] * m[3] + m[4] * m[4] + m[5] * m[5]).max(m[6] * m[6] + m[7] * m[7] + m[8] * m[8]);
            let r = br * crate::math::sqrtf(s2);
            if planes.iter().all(|p| p[0] * c[0] + p[1] * c[1] + p[2] * c[2] + p[3] >= -r) {
                self.order.push(i as u32);
            }
        }
        let count = self.order.len();
        if count == 0 {
            return;
        }
        let lights_on = light.point_count > 0 && blend != Blend::DepthOnly;
        let sphere = |ii: usize| {
            let m = &instances[ii * INSTANCE_STRIDE..ii * INSTANCE_STRIDE + 12];
            let c = [m[0] * bc[0] + m[3] * bc[1] + m[6] * bc[2] + m[9], m[1] * bc[0] + m[4] * bc[1] + m[7] * bc[2] + m[10], m[2] * bc[0] + m[5] * bc[1] + m[8] * bc[2] + m[11]];
            let s2 = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).max(m[3] * m[3] + m[4] * m[4] + m[5] * m[5]).max(m[6] * m[6] + m[7] * m[7] + m[8] * m[8]);
            (c, br * crate::math::sqrtf(s2))
        };
        if count > 1 && blend == Blend::Opaque {
            let d2 = |i: u32| {
                let b = i as usize * INSTANCE_STRIDE;
                let (dx, dy, dz) = (instances[b + 9] - eye[0], instances[b + 10] - eye[1], instances[b + 11] - eye[2]);
                dx * dx + dy * dy + dz * dz
            };
            // Keys indexed by instance (culled ones are never looked up).
            self.keys.clear();
            self.keys.resize(instances.len() / INSTANCE_STRIDE, 0.0);
            for &i in &self.order {
                self.keys[i as usize] = d2(i);
            }
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
            Blend::DepthOnly => {
                for oi in 0..count {
                    self.tris::<DEPTH>(mesh, oi * nv, cull_back, &sh);
                }
            }
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
                    if lights_on {
                        let (c, r) = sphere(ii);
                        sh.cull_lights(c, r);
                    }
                    if pre {
                        self.tris::<SHADE_EQ>(mesh, oi * nv, cull_back, &sh);
                    } else {
                        self.tris::<SHADE_LT>(mesh, oi * nv, cull_back, &sh);
                    }
                }
            }
            Blend::Gloss => {
                for oi in 0..count {
                    let ii = self.order[oi] as usize;
                    let inst = &instances[ii * INSTANCE_STRIDE..(ii + 1) * INSTANCE_STRIDE];
                    sh.set_base([inst[12], inst[13], inst[14]]);
                    sh.phase = mat.pulse_phase + dot([inst[9], inst[10], inst[11]], mat.pulse_dir);
                    if lights_on {
                        let (c, r) = sphere(ii);
                        sh.cull_lights(c, r);
                    }
                    self.tris::<GLOSS>(mesh, oi * nv, cull_back, &sh);
                }
            }
            Blend::Additive => {
                for oi in 0..count {
                    let ii = self.order[oi] as usize;
                    let inst = &instances[ii * INSTANCE_STRIDE..(ii + 1) * INSTANCE_STRIDE];
                    sh.set_base([inst[12], inst[13], inst[14]]);
                    sh.phase = mat.pulse_phase + dot([inst[9], inst[10], inst[11]], mat.pulse_dir);
                    if lights_on {
                        let (c, r) = sphere(ii);
                        sh.cull_lights(c, r);
                    }
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
            flush::<MODE>(&mut self.queue, &mut self.color, sh, (self.gloss.clamp(0.0, 1.0) * 256.0) as u32);
        }
    }

    #[inline]
    #[allow(clippy::needless_range_loop)]
    fn tri<const MODE: u8>(&mut self, p0: &Sv, p1: &Sv, p2: &Sv, cull_back: bool, sh: &Shade) {
        let (x0, y0, x1, y1, x2, y2) = (p0.x, p0.y, p1.x, p1.y, p2.x, p2.y);
        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if area.is_nan() || area.abs() < 1e-6 || (cull_back && (area > 0.0) != self.flip_winding) {
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
            let gl = (self.gloss.clamp(0.0, 1.0) * 256.0) as u32;
            let (depth, color, qu) = (&mut self.depth[a0..=a1], &mut self.color, &mut self.queue);
            let len = depth.len();
            let mut k = 0;
            // Groups of four: when all four pass the depth test (large faces,
            // no overdraw) shade them straight from registers.
            while k + 4 <= len {
                let kf = k as f32;
                let zs = [z0 + dz * kf, z0 + dz * (kf + 1.0), z0 + dz * (kf + 2.0), z0 + dz * (kf + 3.0)];
                let d4 = &mut depth[k..k + 4];
                let pass: [bool; 4] = core::array::from_fn(|l| match MODE {
                    SHADE_EQ => zs[l] <= d4[l],
                    _ => zs[l] < d4[l],
                });
                if !(pass[0] || pass[1] || pass[2] || pass[3]) {
                    k += 4;
                    continue; // hidden: the common case under overdraw
                }
                if pass[0] && pass[1] && pass[2] && pass[3] {
                    if MODE == SHADE_LT || MODE == GLOSS {
                        d4.copy_from_slice(&zs);
                    }
                    // A queued pixel from an earlier (farther) triangle must not land on top later.
                    let (g0, g1) = (a0 + k, a0 + k + 4);
                    if (0..qu.n).any(|l| (g0..g1).contains(&qu.idx[l])) {
                        self.shaded += qu.n;
                        flush::<MODE>(qu, color, sh, gl);
                    }
                    let kk = V4::new(kf, kf + 1.0, kf + 2.0, kf + 3.0);
                    let iw = 1.0 / (kk * dq + q0);
                    let a: [V4; NV] = core::array::from_fn(|c| (kk * da[c] + s0[c]) * iw);
                    let px = pack4(sh.px4([a[0], a[1], a[2]], [a[3], a[4], a[5]], a[6], a[7], V4::splat(fwu), V4::splat(fwv)));
                    let dst = &mut color[a0 + k..a0 + k + 4];
                    for l in 0..4 {
                        put::<MODE>(&mut dst[l], px[l], gl);
                    }
                    self.shaded += 4;
                } else {
                    for l in 0..4 {
                        if !pass[l] {
                            continue;
                        }
                        if MODE == SHADE_LT || MODE == GLOSS {
                            d4[l] = zs[l];
                        }
                        let kf = (k + l) as f32;
                        let qi = qu.n & 3;
                        qu.idx[qi] = a0 + k + l;
                        qu.q[qi] = q0 + dq * kf;
                        for c in 0..NV {
                            qu.a[c][qi] = s0[c] + da[c] * kf;
                        }
                        qu.fw[0][qi] = fwu;
                        qu.fw[1][qi] = fwv;
                        qu.n = qi + 1;
                        if qi == 3 {
                            self.shaded += 4;
                            flush::<MODE>(qu, color, sh, gl);
                        }
                    }
                }
                k += 4;
            }
            // Tail (and short spans): queue the passing pixels.
            for (kk, dp) in depth[k..].iter_mut().enumerate() {
                let kf = (k + kk) as f32;
                let z = z0 + dz * kf;
                let ok = match MODE {
                    SHADE_EQ => z <= *dp,
                    _ => z < *dp,
                };
                if !ok {
                    continue;
                }
                if MODE == SHADE_LT || MODE == GLOSS {
                    *dp = z;
                }
                let l = qu.n & 3;
                qu.idx[l] = a0 + k + kk;
                qu.q[l] = q0 + dq * kf;
                for c in 0..NV {
                    qu.a[c][l] = s0[c] + da[c] * kf;
                }
                qu.fw[0][l] = fwu;
                qu.fw[1][l] = fwv;
                qu.n = l + 1;
                if l == 3 {
                    self.shaded += 4;
                    flush::<MODE>(qu, color, sh, gl);
                }
            }
        }
    }
}

/// Shade the queued pixels (up to four) in one SIMD group and store them.
#[inline(always)]
#[allow(clippy::needless_range_loop)]
fn flush<const MODE: u8>(q: &mut Queue, color: &mut [u32], sh: &Shade, gl: u32) {
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
            put::<MODE>(dst, px[l], gl);
        }
    }
    q.n = 0;
}

impl Raster {
    /// Camera motion blur from the depth buffer.
    ///
    /// Each pixel's world position is reconstructed with the inverse of
    /// `vp` and reprojected with `prev_vp` (the previous frame's camera),
    /// giving a screen velocity × `shutter` (0…1 of a frame), capped at
    /// `max_px`. The blur is reconstructed at half resolution along the
    /// largest velocity of each tile neighbourhood, with depth-aware weights
    /// (McGuire et al.) so near geometry smears over what is behind it, then
    /// blended over the sharp frame where things actually move. Object motion
    /// within a frame isn't captured.
    #[allow(clippy::needless_range_loop)]
    pub fn motion_blur(&mut self, vp: &M4, prev_vp: &M4, shutter: f32, max_px: f32) {
        let m64: crate::space::Mat4 = core::array::from_fn(|i| vp[i] as f64);
        let Some(inv) = crate::space::invert(&m64) else { return };
        let inv: M4 = core::array::from_fn(|i| inv[i] as f32);
        let (w, h) = (self.width, self.height);
        if w < 4 || h < 4 {
            return;
        }
        let (fw, fh) = (w as f32 * 0.5, h as f32 * 0.5);
        // Half-resolution working set.
        let (bw, bh) = (w / 2, h / 2);
        let max_b = max_px * 0.5;
        // 1. Half-res colour (2×2 mean), speed + view distance packed in 4 bytes
        //    (bf16 distance | 8.8 speed), and 8×8 tile maxima of the velocity.
        const T: usize = 8;
        let (tw, th) = (bw.div_ceil(T), bh.div_ceil(T));
        self.tiles.clear();
        self.tiles.resize(tw * th, [0.0; 2]);
        self.vel.clear();
        self.vel.resize(bw * bh, 0);
        self.blur_src.clear();
        self.blur_src.resize(bw * bh, 0);
        // Packed: bf16 view distance | 1/speed in 1.15 fixed point (speed ≥ 0.5 px → 1/speed ≤ 2).
        let pack = |lin: f32, len: f32| (lin.to_bits() & 0xffff_0000) | (((1.0 / len.clamp(0.5, 255.0)) * 32767.0) as u32).min(0xffff);
        // Reprojection in one matrix: an NDC point (nx, ny, nz, 1) is the world
        // point inv·ndc / cw, so its previous clip position is (prev·inv)·ndc / cw
        // and its current view distance is 1 / cw. The cw cancels in the divide.
        let p64: crate::space::Mat4 = core::array::from_fn(|i| prev_vp[i] as f64);
        let inv64 = crate::space::invert(&m64).unwrap_or(m64);
        let c64 = crate::space::mul(&p64, &inv64);
        let cm: M4 = core::array::from_fn(|i| c64[i] as f32);
        let mut any = false;
        let (sx, sy) = (fw * 0.5 * shutter, fh * 0.5 * shutter);
        for by in 0..bh {
            let y = by * 2;
            let ny = 1.0 - (y as f32 + 1.0) / fh;
            // Row constants: each projected coordinate is affine in (nx, nz) along a row.
            let kw = inv[7] * ny + inv[15];
            let (kx, ky, kq) = (cm[4] * ny + cm[12], cm[5] * ny + cm[13], cm[7] * ny + cm[15]);
            let row = y * w;
            let trow = (by / T) * tw;
            for bx in 0..bw {
                let x = bx * 2;
                let i = row + x;
                let q = [self.color[i], self.color[i + 1], self.color[i + w], self.color[i + w + 1]];
                let ch = |s: u32| ((q[0] >> s & 255) + (q[1] >> s & 255) + (q[2] >> s & 255) + (q[3] >> s & 255) + 2) >> 2;
                self.blur_src[by * bw + bx] = ch(0) | ch(8) << 8 | ch(16) << 16;
                // Nearest of the four depths: edges belong to the foreground.
                let d = self.depth[i].min(self.depth[i + 1]).min(self.depth[i + w]).min(self.depth[i + w + 1]);
                let nz = if d.is_finite() { d } else { 0.9999 };
                let nx = (x as f32 + 1.0) / fw - 1.0;
                let cw = inv[3] * nx + inv[11] * nz + kw;
                if cw.abs() < 1e-12 {
                    continue;
                }
                let lin = 1.0 / cw;
                let qw = cm[3] * nx + cm[11] * nz + kq;
                // Behind the previous camera (w ≤ 0 after the cw divide): treat as still.
                if qw * lin <= 1e-6 {
                    self.vel[by * bw + bx] = pack(lin, 0.5);
                    continue;
                }
                let iq = 1.0 / qw;
                let px = (cm[0] * nx + cm[8] * nz + kx) * iq;
                let py = (cm[1] * nx + cm[9] * nz + ky) * iq;
                // Velocity in half-res pixels.
                let (mut vx, mut vy) = ((nx - px) * sx, (py - ny) * sy);
                let l2 = vx * vx + vy * vy;
                let mut len = crate::math::sqrtf(l2);
                if len > max_b {
                    vx *= max_b / len;
                    vy *= max_b / len;
                    len = max_b;
                }
                any |= len > 0.5;
                self.vel[by * bw + bx] = pack(lin, len);
                let t = &mut self.tiles[trow + bx / T];
                if len * len > t[0] * t[0] + t[1] * t[1] {
                    *t = [vx, vy];
                }
            }
        }
        if !any {
            return;
        }
        // 2. Max over each tile's 3×3 neighbourhood.
        let tmax = self.tiles.clone();
        for ty in 0..th {
            for tx in 0..tw {
                let mut best = [0.0f32; 2];
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = (tx as i32 + dx, ty as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= tw as i32 || ny >= th as i32 {
                            continue;
                        }
                        let v = tmax[ny as usize * tw + nx as usize];
                        if v[0] * v[0] + v[1] * v[1] > best[0] * best[0] + best[1] * best[1] {
                            best = v;
                        }
                    }
                }
                self.tiles[ty * tw + tx] = best;
            }
        }
        // 3. Half-res reconstruction into `light` (reused as an RGB f32 buffer).
        self.half_px.clear();
        self.half_px.resize(bw * bh, 0);
        // (view distance, 1/speed)
        let unpack = |v: u32| (f32::from_bits(v & 0xffff_0000), (v & 0xffff) as f32 * (1.0 / 32767.0));
        let (src, vel) = (&self.blur_src, &self.vel);
        let (wi, hi) = (bw as isize - 1, bh as isize - 1);
        const N: usize = 8;
        let lanes = V4::new(0.0, 1.0, 2.0, 3.0);
        for by in 0..bh {
            for bx in 0..bw {
                let j = by * bw + bx;
                let c0 = src[j];
                let mut hs = (bx as u32).wrapping_mul(0x9E37_79B1) ^ (by as u32).wrapping_mul(0x85EB_CA77);
                hs ^= hs >> 15;
                hs = hs.wrapping_mul(0x2C1B_3C6D);
                let jx = ((hs >> 4) & 7) as isize - 4;
                let jy = ((hs >> 12) & 7) as isize - 4;
                let tx = ((bx as isize + jx).clamp(0, wi) as usize) / T;
                let ty = ((by as isize + jy).clamp(0, hi) as usize) / T;
                let vn = self.tiles[ty * tw + tx];
                let ln2 = vn[0] * vn[0] + vn[1] * vn[1];
                if ln2 < 0.25 {
                    self.half_px[j] = c0 & 0x00ff_ffff;
                    continue;
                }
                let ln = crate::math::sqrtf(ln2);
                let (zc, ilc) = unpack(vel[j]);
                let kz = 1.0 / (0.02 * zc.abs().max(1.0));
                let jit = (hs >> 8) as f32 * (1.0 / 16_777_216.0) - 0.5;
                // Taps spaced ≤ ~2.5 half-res px apart (jittered), 3 to N of them.
                let n = ((ln * 0.4) as usize + 3).min(N);
                let inv_n = 1.0 / n as f32;
                // Taps march along the tile velocity in 16.16 fixed point: one
                // float→int conversion per pixel instead of two per tap.
                let h0 = (0.5 + jit * 0.95) * inv_n - 0.5;
                let fx0 = ((bx as f32 + 0.5 + vn[0] * h0) * 65536.0) as i32;
                let fy0 = ((by as f32 + 0.5 + vn[1] * h0) * 65536.0) as i32;
                let (dfx, dfy) = ((vn[0] * inv_n * 65536.0) as i32, (vn[1] * inv_n * 65536.0) as i32);
                let (dd, d0) = (inv_n * ln, h0 * ln);
                let (mut fx, mut fy) = (fx0, fy0);
                // Four taps per step: scalar gathers, weights and accumulation
                // in SIMD lanes (one lane per tap; lanes past n weigh zero).
                let (zcv, kzv, ilcv) = (V4::splat(zc), V4::splat(kz), V4::splat(ilc));
                let mut acc4 = [V4::splat(0.0); 3];
                let mut tot4 = V4::splat(0.0);
                let mut k0 = 0;
                while k0 < n {
                    let mut vq = [0u32; 4];
                    let mut cq = [0u32; 4];
                    for l in 0..4 {
                        let m = (fy >> 16).clamp(0, hi as i32) as usize * bw + (fx >> 16).clamp(0, wi as i32) as usize;
                        fx = fx.wrapping_add(dfx);
                        fy = fy.wrapping_add(dfy);
                        vq[l] = vel[m];
                        cq[l] = src[m];
                    }
                    let kv = lanes + k0 as f32;
                    let dist = (kv * dd + d0).abs();
                    let zs = V4::from_bits_masked(vq, 0xffff_0000);
                    let ils = V4::from_u32_masked(vq, 0xffff) * (1.0 / 32767.0);
                    let (ds, dc) = (dist * ils, dist * ilcv);
                    let fg = ((zcv - zs) * kzv + 0.5).clamp(0.0, 1.0);
                    let wgt = fg * (1.0 - ds).maxf(0.0) + (1.0 - fg) * (1.0 - dc).maxf(0.0) + (10.5 - ds * 10.0).clamp(0.0, 1.0) * (10.5 - dc * 10.0).clamp(0.0, 1.0) * 2.0;
                    let wgt = V4::select(kv.ltf(n as f32), wgt, V4::splat(0.0));
                    let [r, g, b] = V4::unpack_rgb(cq);
                    acc4 = [acc4[0] + r * wgt, acc4[1] + g * wgt, acc4[2] + b * wgt];
                    tot4 = tot4 + wgt;
                    k0 += 4;
                }
                let acc = [(c0 & 255) as f32 * ilc + acc4[0].sum(), ((c0 >> 8) & 255) as f32 * ilc + acc4[1].sum(), ((c0 >> 16) & 255) as f32 * ilc + acc4[2].sum()];
                let tot = ilc + tot4.sum();
                let it = 1.0 / tot;
                let q = |v: f32| ((v * it + 0.5) as u32).min(255);
                self.half_px[j] = q(acc[0]) | q(acc[1]) << 8 | q(acc[2]) << 16;
            }
        }
        // 3b. The jittered taps leave grain inside long smears (taps there are
        //     several pixels apart). Average four samples of the result spread
        //     over 1.5 tap spacings along the tile velocity: the smear is
        //     already that long, so this only removes the noise. Packed SWAR.
        {
            const RB: u32 = 0x00ff_00ff;
            // Per tile: the 16.16 step between samples and the first sample's offset (0 step = still).
            self.tile_step.clear();
            self.tile_step.extend(self.tiles.iter().map(|vn| {
                let ln2 = vn[0] * vn[0] + vn[1] * vn[1];
                if ln2 < 0.25 {
                    return [0; 4];
                }
                let s = 0.375 / ((crate::math::sqrtf(ln2) * 0.4) as usize + 3).min(N) as f32;
                let (sx, sy) = (vn[0] * s, vn[1] * s);
                [(sx * 65536.0) as i32, (sy * 65536.0) as i32, ((0.5 - sx * 3.5) * 65536.0) as i32, ((0.5 - sy * 3.5) * 65536.0) as i32]
            }));
            let (hp, out, steps) = (&self.half_px, &mut self.blur_src, &self.tile_step);
            for by in 0..bh {
                let trow = (by / T) * tw;
                let (orow, hrow) = (&mut out[by * bw..(by + 1) * bw], &hp[by * bw..(by + 1) * bw]);
                for (bx, (o, &c)) in orow.iter_mut().zip(hrow).enumerate() {
                    let [sx, sy, ox, oy] = steps[trow + bx / T];
                    if sx == 0 && sy == 0 {
                        *o = c;
                        continue;
                    }
                    let (mut fx, mut fy) = (((bx as i32) << 16) + ox, ((by as i32) << 16) + oy);
                    let (mut arb, mut ag) = (0u32, 0u32);
                    // Clamp per sample only when the footprint leaves the image.
                    let (ex, ey) = (fx.wrapping_add(sx * 7), fy.wrapping_add(sy * 7));
                    let inside = fx.min(ex) >= 0 && fy.min(ey) >= 0 && (fx.max(ex) >> 16) <= wi as i32 && (fy.max(ey) >> 16) <= hi as i32;
                    if inside {
                        for _ in 0..8 {
                            let v = hp[(fy >> 16) as usize * bw + (fx >> 16) as usize];
                            arb += v & RB;
                            ag += (v >> 8) & 255;
                            fx += sx;
                            fy += sy;
                        }
                    } else {
                        for _ in 0..8 {
                            let v = hp[(fy >> 16).clamp(0, hi as i32) as usize * bw + (fx >> 16).clamp(0, wi as i32) as usize];
                            arb += v & RB;
                            ag += (v >> 8) & 255;
                            fx = fx.wrapping_add(sx);
                            fy = fy.wrapping_add(sy);
                        }
                    }
                    *o = ((arb + 0x0004_0004) >> 3 & RB) | ((ag + 4) >> 3) << 8;
                }
            }
            core::mem::swap(&mut self.half_px, &mut self.blur_src);
        }
        // 4. Blend the upsampled blur over the sharp frame by local speed
        //    (sharp where still, fully blurred from ~3 px of motion).
        let mut alpha = core::mem::take(&mut self.alpha);
        alpha.clear();
        alpha.extend((0..bw * bh).map(|j| {
            let t = self.tiles[((j / bw) / T).min(th - 1) * tw + ((j % bw) / T).min(tw - 1)];
            ((crate::math::sqrtf(t[0] * t[0] + t[1] * t[1]) * 2.0 - 0.75) / 2.25).clamp(0.0, 1.0)
        }));
        self.blend_half(bw, bh, &alpha);
        self.alpha = alpha;
    }
}

impl Raster {
    /// Fill every pixel no geometry covered (depth still clear) with the
    /// environment: a sky/horizon/ground gradient seen along each view ray,
    /// plus a sun disc with a soft halo towards `light.key` (`sun` = its
    /// angular size factor, 0 = none). Call after the opaque draws.
    #[allow(clippy::needless_range_loop)]
    pub fn sky(&mut self, vp: &M4, light: &Lighting, sun: f32) {
        let m64: crate::space::Mat4 = core::array::from_fn(|i| vp[i] as f64);
        let Some(inv) = crate::space::invert(&m64) else { return };
        let inv: M4 = core::array::from_fn(|i| inv[i] as f32);
        let (w, h) = (self.width, self.height);
        let key = norm3(light.key);
        // View rays are linear in screen space for a perspective camera: the
        // far-plane point minus the near-plane point at three corners spans them.
        let un = |nx: f32, ny: f32, nz: f32| {
            let cw = inv[3] * nx + inv[7] * ny + inv[11] * nz + inv[15];
            let iw = 1.0 / cw;
            [(inv[0] * nx + inv[4] * ny + inv[8] * nz + inv[12]) * iw, (inv[1] * nx + inv[5] * ny + inv[9] * nz + inv[13]) * iw, (inv[2] * nx + inv[6] * ny + inv[10] * nz + inv[14]) * iw]
        };
        let ray = |nx: f32, ny: f32| {
            let (a, b) = (un(nx, ny, -1.0), un(nx, ny, 1.0));
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
        };
        let (r00, r10, r01) = (ray(-1.0, 1.0), ray(1.0, 1.0), ray(-1.0, -1.0));
        let dx: [f32; 3] = core::array::from_fn(|k| (r10[k] - r00[k]) / w as f32);
        let dy: [f32; 3] = core::array::from_fn(|k| (r01[k] - r00[k]) / h as f32);
        let fog = light.fog_color;
        let fog_k = if light.fog[1].is_finite() { 0.6 } else { 0.0 };
        let sun_k = 400.0 / (sun * sun).max(1e-6);
        // The sky is smooth (only the sun's rim has detail, and bloom softens
        // it), so shade it on a half-resolution grid of samples at even pixels
        // and interpolate. Only samples next to an uncovered pixel are shaded.
        let (gw, gh) = (w / 2 + 2, h / 2 + 2);
        let mut need = core::mem::take(&mut self.sky_need);
        need.clear();
        need.resize(gw * gh, 0);
        let mut any = false;
        for y in 0..h {
            let j = y / 2;
            for (x, d) in self.depth[y * w..(y + 1) * w].iter().enumerate() {
                if !d.is_finite() {
                    let i = j * gw + x / 2;
                    need[i] = 1;
                    need[i + 1] = 1;
                    need[i + gw] = 1;
                    need[i + gw + 1] = 1;
                    any = true;
                }
            }
        }
        if !any {
            self.sky_need = need;
            return;
        }
        let mut grid = core::mem::take(&mut self.half_px);
        grid.clear();
        grid.resize(gw * gh, 0);
        let lanes = V4::new(0.5, 2.5, 4.5, 6.5);
        for j in 0..gh {
            let fy = (2 * j) as f32 + 0.5;
            let base: [f32; 3] = core::array::from_fn(|k| r00[k] + dy[k] * fy);
            let mut i = 0;
            while i < gw {
                let n = (gw - i).min(4);
                let row = j * gw + i;
                if need[row..row + n].iter().all(|v| *v == 0) {
                    i += 4;
                    continue;
                }
                let fx = lanes + (2 * i) as f32;
                let d = [fx * dx[0] + base[0], fx * dx[1] + base[1], fx * dx[2] + base[2]];
                let inv_len = 1.0 / dot3(&d, &d).sqrt().maxf(1e-12);
                let d = [d[0] * inv_len, d[1] * inv_len, d[2] * inv_len];
                let up = d[1].clamp(-1.0, 1.0);
                let band = peak4(up, 3.0);
                let above = up.gtf(0.0);
                let mut c: [V4; 3] = core::array::from_fn(|k| {
                    let sg = V4::select(above, V4::splat(light.sky[k]), V4::splat(light.ground[k]));
                    let v = sg + (light.horizon[k] - sg) * band;
                    // Towards the horizon the sky meets the fog colour, so distant geometry dissolves into it.
                    v + (fog[k] - v) * band * fog_k
                });
                if sun > 0.0 {
                    let cs = (d[0] * key[0] + d[1] * key[1] + d[2] * key[2]).maxf(0.0);
                    let t = (1.0 - cs) * sun_k;
                    let disc = (1.0 - t).clamp(0.0, 1.0);
                    let halo = 0.25 / (t * 0.6 + 1.0);
                    for k in 0..3 {
                        c[k] = c[k] + (disc * 3.0 + halo) * light.key_color[k];
                    }
                }
                let px = pack4(c);
                grid[row..row + n].copy_from_slice(&px[..n]);
                i += 4;
            }
        }
        // Fill uncovered pixels: even pixels take their sample, odd ones the
        // mean of the two or four around them (packed-integer averages).
        const RB: u32 = 0x00ff_00ff;
        for y in 0..h {
            let (j, oy) = (y / 2, y & 1);
            let (g0, g1) = (&grid[j * gw..(j + 1) * gw], &grid[(j + oy) * gw..(j + oy + 1) * gw]);
            let (crow, drow) = (&mut self.color[y * w..(y + 1) * w], &self.depth[y * w..(y + 1) * w]);
            for (x, (c, d)) in crow.iter_mut().zip(drow).enumerate() {
                if d.is_finite() {
                    continue;
                }
                let (i, ox) = (x / 2, x & 1);
                let q = [g0[i], g0[i + ox], g1[i], g1[i + ox]];
                let rb = ((q[0] & RB) + (q[1] & RB) + (q[2] & RB) + (q[3] & RB) + 0x0002_0002) >> 2 & RB;
                let g = (((q[0] >> 8) & 255) + ((q[1] >> 8) & 255) + ((q[2] >> 8) & 255) + ((q[3] >> 8) & 255) + 2) >> 2;
                *c = 0xff00_0000 | rb | g << 8;
            }
        }
        self.half_px = grid;
        self.sky_need = need;
    }
}

impl Raster {
    /// Depth of field: a thin-lens blur whose circle of confusion is
    /// `aperture · |d − focus| / d` pixels (d = view distance from the depth
    /// buffer; background counts as infinitely far), capped at `max_px`.
    /// Gathered at half resolution with a 16-tap golden-angle disc, taps
    /// weighted by their own blur so sharp foreground doesn't bleed into the
    /// background, then blended over the sharp frame by each pixel's blur.
    #[allow(clippy::needless_range_loop)]
    pub fn depth_of_field(&mut self, vp: &M4, focus: f32, aperture: f32, max_px: f32) {
        let m64: crate::space::Mat4 = core::array::from_fn(|i| vp[i] as f64);
        let Some(inv) = crate::space::invert(&m64) else { return };
        let inv: M4 = core::array::from_fn(|i| inv[i] as f32);
        let (w, h) = (self.width, self.height);
        if w < 4 || h < 4 {
            return;
        }
        let (bw, bh) = (w / 2, h / 2);
        let (fw, fh) = (w as f32 * 0.5, h as f32 * 0.5);
        // CoC in half-res pixels per half-res pixel + half-res colour.
        self.blur_src.clear();
        self.blur_src.resize(bw * bh, 0);
        let mut coc = core::mem::take(&mut self.coc);
        coc.clear();
        coc.resize(bw * bh, 0.0);
        let cmax = max_px * 0.5;
        let mut any = false;
        for by in 0..bh {
            let y = by * 2;
            let ny = 1.0 - (y as f32 + 1.0) / fh;
            let kw = inv[7] * ny + inv[15];
            for bx in 0..bw {
                let x = bx * 2;
                let i = y * w + x;
                let q = [self.color[i], self.color[i + 1], self.color[i + w], self.color[i + w + 1]];
                let ch = |s: u32| ((q[0] >> s & 255) + (q[1] >> s & 255) + (q[2] >> s & 255) + (q[3] >> s & 255) + 2) >> 2;
                self.blur_src[by * bw + bx] = ch(0) | ch(8) << 8 | ch(16) << 16;
                let d = self.depth[i].min(self.depth[i + 1]).min(self.depth[i + w]).min(self.depth[i + w + 1]);
                let c = if d.is_finite() {
                    // View distance of an NDC point is 1 / (inv·ndc).w (vp·inv = I).
                    let nx = (x as f32 + 1.0) / fw - 1.0;
                    let lin = (1.0 / (inv[3] * nx + inv[11] * d + kw)).max(1e-3);
                    aperture * 0.5 * (lin - focus).abs() / lin
                } else {
                    aperture * 0.5
                };
                let c = c.min(cmax);
                any |= c > 0.5;
                coc[by * bw + bx] = c;
            }
        }
        if !any {
            self.coc = coc;
            return;
        }
        // Gather over a golden-angle disc. Radii are quantised to ¼ px so the
        // integer tap offsets come from a table (no float→int per tap); small
        // circles use the first half of a 7-tap disc.
        self.half_px.clear();
        self.half_px.resize(bw * bh, 0);
        const N: usize = 14;
        const NS: usize = 7;
        const Q: f32 = 4.0;
        let golden = 2.399_963;
        let disc = |k: usize, n: usize| {
            let r = crate::math::sqrtf((k as f32 + 0.5) / n as f32);
            let a = k as f32 * golden;
            (r * libm::cosf(a), r * libm::sinf(a), r)
        };
        let levels = (cmax * Q) as usize + 2;
        let mut tab = core::mem::take(&mut self.dof_taps);
        tab.clear();
        for l in 0..levels {
            let rq = l as f32 / Q;
            let n = if rq < 3.0 { NS } else { N };
            for k in 0..N {
                let (tx, ty, tr) = if k < n { disc(k, n) } else { (0.0, 0.0, 0.0) };
                tab.push((crate::math::floorf(0.5 + tx * rq) as i32, crate::math::floorf(0.5 + ty * rq) as i32, rq * tr));
            }
        }
        let (wi, hi) = (bw as i32 - 1, bh as i32 - 1);
        let src = &self.blur_src;
        for by in 0..bh {
            for bx in 0..bw {
                let j = by * bw + bx;
                let c0 = src[j];
                let rc = coc[j];
                if rc < 0.5 {
                    self.half_px[j] = c0 & 0x00ff_ffff;
                    continue;
                }
                let lv = ((rc * Q + 0.5) as usize).min(levels - 1);
                let n = if (lv as f32) < 3.0 * Q { NS } else { N };
                let mut acc = [(c0 & 255) as f32, ((c0 >> 8) & 255) as f32, ((c0 >> 16) & 255) as f32];
                let mut tot = 1.0;
                for &(ox, oy, dist) in &tab[lv * N..lv * N + n] {
                    let m = (by as i32 + oy).clamp(0, hi) as usize * bw + (bx as i32 + ox).clamp(0, wi) as usize;
                    // A tap only contributes if it is itself blurry enough to reach us.
                    let wgt = ((coc[m] - dist) + 1.0).clamp(0.0, 1.0);
                    if wgt <= 0.0 {
                        continue;
                    }
                    let c = src[m];
                    acc[0] += (c & 255) as f32 * wgt;
                    acc[1] += ((c >> 8) & 255) as f32 * wgt;
                    acc[2] += ((c >> 16) & 255) as f32 * wgt;
                    tot += wgt;
                }
                let it = 1.0 / tot;
                let q = |v: f32| ((v * it + 0.5) as u32).min(255);
                self.half_px[j] = q(acc[0]) | q(acc[1]) << 8 | q(acc[2]) << 16;
            }
        }
        self.dof_taps = tab;
        // Blend over the sharp frame by blur amount.
        let mut alpha = core::mem::take(&mut self.alpha);
        alpha.clear();
        alpha.extend(coc.iter().map(|c| ((c - 0.25) * 1.5).clamp(0.0, 1.0)));
        self.blend_half(bw, bh, &alpha);
        self.alpha = alpha;
        self.coc = coc;
    }
}

impl Raster {
    /// Blend the half-resolution packed RGB image in `self.half_px` over the frame,
    /// upsampled 2× bilinearly with fixed phase weights (output pixel 2i + k
    /// sits at i − 0.25 + k/2), by the half-res `alpha`.
    #[allow(clippy::needless_range_loop)]
    fn blend_half(&mut self, bw: usize, bh: usize, alpha: &[f32]) {
        let (w, h) = (self.width, self.height);
        // Packed-integer (SWAR) from here on: red+blue share one u32 in 16-bit
        // lanes, green another, so each tap is two multiply-adds, not three.
        const RB: u32 = 0x00ff_00ff;
        self.row_px.clear();
        self.row_px.resize(bw, 0);
        let hp = &self.half_px;
        for y in 0..h.min(bh * 2) {
            let by = y / 2;
            let ny = if y & 1 == 0 { by.saturating_sub(1) } else { (by + 1).min(bh - 1) };
            let arow = &alpha[by * bw..(by + 1) * bw];
            if arow.iter().all(|a| *a <= 0.0) {
                continue;
            }
            // Vertical 0.75 / 0.25 phase.
            let (r0, r1) = (&hp[by * bw..(by + 1) * bw], &hp[ny * bw..(ny + 1) * bw]);
            for ((o, &a), &b) in self.row_px.iter_mut().zip(r0).zip(r1) {
                let rb = (((a & RB) * 3 + (b & RB) + 0x0002_0002) >> 2) & RB;
                let g = ((((a >> 8) & 255) * 3 + ((b >> 8) & 255) + 2) >> 2) << 8;
                *o = rb | g;
            }
            let rb = &self.row_px;
            let out = &mut self.color[y * w..y * w + bw * 2];
            // Output pixels come in pairs that share a source pixel and alpha:
            // the left one leans towards bx − 1, the right one towards bx + 1.
            let mix = |c: u32, p0: u32, p1: u32, ai: u32| {
                let b_rb = (((p0 & RB) * 3 + (p1 & RB) + 0x0002_0002) >> 2) & RB;
                let b_g = (((p0 >> 8) & 255) * 3 + ((p1 >> 8) & 255) + 2) >> 2;
                let o_rb = (((c & RB) * (256 - ai) + b_rb * ai + 0x0080_0080) >> 8) & RB;
                let o_g = ((((c >> 8) & 255) * (256 - ai) + b_g * ai + 128) >> 8) & 255;
                (c & 0xff00_0000) | o_rb | o_g << 8
            };
            if arow.iter().all(|a| *a >= 1.0) {
                // Fully covered row (a reflection): the mix is a plain store.
                let lerp = |p0: u32, p1: u32| ((((p0 & RB) * 3 + (p1 & RB) + 0x0002_0002) >> 2) & RB) | (((((p0 >> 8) & 255) * 3 + ((p1 >> 8) & 255) + 2) >> 2) << 8);
                for (bx, pair) in out.chunks_exact_mut(2).enumerate() {
                    let p0 = rb[bx];
                    let (pl, pr) = (rb[bx.saturating_sub(1)], rb[(bx + 1).min(bw - 1)]);
                    pair[0] = (pair[0] & 0xff00_0000) | lerp(p0, pl);
                    pair[1] = (pair[1] & 0xff00_0000) | lerp(p0, pr);
                }
                continue;
            }
            for (bx, (pair, &a)) in out.chunks_exact_mut(2).zip(arow).enumerate() {
                if a <= 0.0 {
                    continue;
                }
                let ai = (a.min(1.0) * 256.0) as u32;
                let p0 = rb[bx];
                let (pl, pr) = (rb[bx.saturating_sub(1)], rb[(bx + 1).min(bw - 1)]);
                pair[0] = mix(pair[0], p0, pl, ai);
                pair[1] = mix(pair[1], p0, pr, ai);
            }
        }
    }
}

/// Light points drawn into a [`Raster`] (stars, dust, sparks): additive,
/// depth-tested against the solid geometry already drawn, and optionally
/// streaked along their screen motion since the previous camera.
#[derive(Clone, Copy, Debug)]
pub struct PointStyle {
    /// World-space radius of each point.
    pub size: f32,
    /// Brightness multiplier.
    pub intensity: f32,
    /// Streak length as a multiple of the screen motion between `prev_vp`
    /// and `vp` (0 = round points).
    pub streak: f32,
    /// Fade out between these camera distances.
    pub fog: [f32; 2],
}

impl Default for PointStyle {
    fn default() -> Self {
        Self { size: 2.0, intensity: 1.0, streak: 0.0, fog: [0.0, f32::INFINITY] }
    }
}

impl Raster {
    /// Draw `pos` (xyz per point) with `rgb` colours (three per point, or
    /// empty for white) as additive light. `prev_vp` (the previous frame's
    /// view-projection) gives motion streaks when `style.streak > 0`.
    #[allow(clippy::too_many_arguments)]
    pub fn points(&mut self, pos: &[f32], rgb: &[f32], vp: &M4, prev_vp: Option<&M4>, eye: [f32; 3], style: &PointStyle) {
        let n = pos.len() / 3;
        let (w, h) = (self.width, self.height);
        let (hw, hh) = (w as f32 * 0.5, h as f32 * 0.5);
        self.light.clear();
        self.light.resize(w * h * 3, 0.0);
        // Focal length in NDC: the y row of P·V has the camera's cot(fov/2) as its length.
        let focal = crate::math::sqrtf(vp[1] * vp[1] + vp[5] * vp[5] + vp[9] * vp[9]);
        let fog_on = style.fog[1].is_finite();
        let fog_inv = 1.0 / (style.fog[1] - style.fog[0]).max(1e-3);
        let has_rgb = rgb.len() >= n * 3;
        let proj = |m: &M4, x: f32, y: f32, z: f32| {
            let cw = m[3] * x + m[7] * y + m[11] * z + m[15];
            let iw = 1.0 / cw;
            ((m[0] * x + m[4] * y + m[8] * z + m[12]) * iw, (m[1] * x + m[5] * y + m[9] * z + m[13]) * iw, (m[2] * x + m[6] * y + m[10] * z + m[14]) * iw, cw)
        };
        for i in 0..n {
            let (x, y, z) = (pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]);
            let (nx, ny, nz, cw) = proj(vp, x, y, z);
            if cw <= 1e-3 || !(-1.0..=1.0).contains(&nz) {
                continue;
            }
            let (sx, sy) = (hw + nx * hw, hh - ny * hh);
            let r = style.size * focal * hh / cw;
            if sx < -r - 64.0 || sy < -r - 64.0 || sx > w as f32 + r + 64.0 || sy > h as f32 + r + 64.0 {
                continue;
            }
            let mut e = style.intensity;
            if fog_on {
                let dist = crate::math::sqrtf((x - eye[0]) * (x - eye[0]) + (y - eye[1]) * (y - eye[1]) + (z - eye[2]) * (z - eye[2]));
                e *= 1.0 - ((dist - style.fog[0]) * fog_inv).clamp(0.0, 1.0);
                if e <= 0.0 {
                    continue;
                }
            }
            let c = if has_rgb { [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]] } else { [1.0; 3] };
            // Energy of a disc of radius r (in px²), spread over at least one pixel.
            let area = (r * r * core::f32::consts::PI).max(0.0);
            // Streak start: where the point was under the previous camera.
            let (mut ox, mut oy) = (sx, sy);
            if style.streak > 0.0 {
                if let Some(pm) = prev_vp {
                    let (px, py, _, pw) = proj(pm, x, y, z);
                    if pw > 1e-3 {
                        let (qx, qy) = (hw + px * hw, hh - py * hh);
                        ox = sx + (qx - sx) * style.streak;
                        oy = sy + (qy - sy) * style.streak;
                    }
                }
            }
            let len = crate::math::sqrtf((ox - sx) * (ox - sx) + (oy - sy) * (oy - sy)).min(w as f32);
            let steps = (len.max(0.0) as usize) + 1;
            // Total energy is conserved along the streak; the head carries a little extra.
            let per = e * area.max(1.0) / steps as f32;
            for k in 0..steps {
                let t = if steps == 1 { 0.0 } else { k as f32 / (steps - 1) as f32 };
                let (px, py) = (sx + (ox - sx) * t, sy + (oy - sy) * t);
                let fade = 1.0 - t * 0.85;
                self.splat_light(px, py, nz, c, per * fade * if k == 0 { 1.6 } else { 1.0 }, r);
            }
        }
        // Resolve: tone-map the light and add it to the frame.
        for (i, px) in self.color.iter_mut().enumerate() {
            let l = &self.light[i * 3..i * 3 + 3];
            if l[0] + l[1] + l[2] <= 0.0 {
                continue;
            }
            // Soft knee so dense clusters saturate gracefully.
            let t = |v: f32| (v / (1.0 + v * 0.35) * 255.0) as u32;
            let add = t(l[0]).min(255) | (t(l[1]).min(255) << 8) | (t(l[2]).min(255) << 16);
            *px = add_packed(*px, add);
        }
    }

    /// Add energy `e` of colour `c` around (x, y): bilinear for sub-pixel
    /// points, a small disc for larger ones; depth-tested at the centre.
    #[inline]
    fn splat_light(&mut self, x: f32, y: f32, z: f32, c: [f32; 3], e: f32, r: f32) {
        let (w, h) = (self.width as isize, self.height as isize);
        if r <= 1.0 {
            let (fx, fy) = (x - 0.5, y - 0.5);
            let (ix, iy) = (crate::math::floorf(fx) as isize, crate::math::floorf(fy) as isize);
            let (tx, ty) = (fx - ix as f32, fy - iy as f32);
            for (dx, dy, wgt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                let (px, py) = (ix + dx, iy + dy);
                if px < 0 || py < 0 || px >= w || py >= h {
                    continue;
                }
                let i = (py * w + px) as usize;
                if z > self.depth[i] {
                    continue;
                }
                let k = e * wgt;
                self.light[i * 3] += c[0] * k;
                self.light[i * 3 + 1] += c[1] * k;
                self.light[i * 3 + 2] += c[2] * k;
            }
            return;
        }
        let ri = r.min(64.0);
        let (x0, x1) = (((x - ri) as isize).max(0), ((x + ri) as isize + 1).min(w - 1));
        let (y0, y1) = (((y - ri) as isize).max(0), ((y + ri) as isize + 1).min(h - 1));
        let norm = e / (ri * ri * 1.6);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let (dx, dy) = (px as f32 + 0.5 - x, py as f32 + 0.5 - y);
                let q = 1.0 - (dx * dx + dy * dy) / (ri * ri);
                if q <= 0.0 {
                    continue;
                }
                let i = (py * w + px) as usize;
                if z > self.depth[i] {
                    continue;
                }
                let k = norm * q * q;
                self.light[i * 3] += c[0] * k;
                self.light[i * 3 + 1] += c[1] * k;
                self.light[i * 3 + 2] += c[2] * k;
            }
        }
    }
}

/// Bounding sphere of a mesh (box centre, max distance), in model space.
fn bounds(mesh: &Mesh) -> ([f32; 3], f32) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in mesh.pos.chunks_exact(3) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if lo[0] > hi[0] {
        return ([0.0; 3], 0.0);
    }
    let c = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
    let mut r2 = 0.0f32;
    for p in mesh.pos.chunks_exact(3) {
        r2 = r2.max((p[0] - c[0]) * (p[0] - c[0]) + (p[1] - c[1]) * (p[1] - c[1]) + (p[2] - c[2]) * (p[2] - c[2]));
    }
    (c, crate::math::sqrtf(r2) * 1.0001 + 1e-6)
}

const DEPTH: u8 = 0;
const SHADE_EQ: u8 = 1;
const ADD: u8 = 2;
const SHADE_LT: u8 = 3;
const GLOSS: u8 = 4;

/// Write a shaded pixel according to the mode.
#[inline(always)]
fn put<const MODE: u8>(dst: &mut u32, px: u32, gloss: u32) {
    *dst = match MODE {
        ADD => add_packed(*dst, px),
        GLOSS => {
            // px·(1 − g) + dst·g per channel, g in 1/256ths.
            let ch = |s: u32| ((((px >> s) & 255) * (256 - gloss) + ((*dst >> s) & 255) * gloss) >> 8) << s;
            ch(0) | ch(8) | ch(16) | 0xff00_0000
        }
        _ => px,
    }
}

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
    /// Opaque, but blended over what is already in the colour buffer by
    /// [`Raster::gloss`]: a glossy floor over a mirrored render of the scene.
    Gloss,
    /// Depth only (shadow maps, occluders).
    DepthOnly,
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

/// An open tube of radius `r` along a polyline (`pts`, xyz triples), with
/// parallel-transport frames (no twisting) and a per-point radius scale
/// (`radii`, optional) so trails can taper. `u` runs along the tube (0…1 by
/// arc length), `v` around it.
#[allow(clippy::needless_range_loop)]
pub fn tube_path(pts: &[f32], radii: &[f32], r: f32, sides: usize) -> Mesh {
    let n = pts.len() / 3;
    let sides = sides.max(3);
    let mut m = Mesh::default();
    if n < 2 {
        return m;
    }
    let p = |i: usize| [pts[i * 3], pts[i * 3 + 1], pts[i * 3 + 2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let tangent = |i: usize| {
        let (a, b) = (p(i.saturating_sub(1)), p((i + 1).min(n - 1)));
        norm3([b[0] - a[0], b[1] - a[1], b[2] - a[2]])
    };
    // Arc length for u.
    let mut acc = vec![0.0f32; n];
    for i in 1..n {
        let (a, b) = (p(i - 1), p(i));
        acc[i] = acc[i - 1] + crate::math::sqrtf((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]) + (b[2] - a[2]) * (b[2] - a[2]));
    }
    let total = acc[n - 1].max(1e-6);
    let t0 = tangent(0);
    let seed = if t0[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let mut nrm = norm3(cross(cross(t0, seed), t0));
    let tau = core::f32::consts::TAU;
    for i in 0..n {
        let t = tangent(i);
        // Parallel transport: remove the tangent component from the previous normal.
        let d = dot(nrm, t);
        let v = [nrm[0] - d * t[0], nrm[1] - d * t[1], nrm[2] - d * t[2]];
        if dot(v, v) > 1e-10 {
            nrm = norm3(v);
        }
        let bn = cross(t, nrm);
        let rr = r * radii.get(i).copied().unwrap_or(1.0);
        let c = p(i);
        for j in 0..=sides {
            let a = (j % sides) as f32 / sides as f32 * tau;
            let (ca, sa) = (libm::cosf(a), libm::sinf(a));
            let dir = [nrm[0] * ca + bn[0] * sa, nrm[1] * ca + bn[1] * sa, nrm[2] * ca + bn[2] * sa];
            m.pos.extend_from_slice(&[c[0] + dir[0] * rr, c[1] + dir[1] * rr, c[2] + dir[2] * rr]);
            m.nrm.extend_from_slice(&dir);
            m.uv.extend_from_slice(&[acc[i] / total, j as f32 / sides as f32]);
        }
    }
    let row = (sides + 1) as u32;
    for i in 0..(n - 1) as u32 {
        for j in 0..sides as u32 {
            let (a, b, c, d) = (i * row + j, (i + 1) * row + j, (i + 1) * row + j + 1, i * row + j + 1);
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
    // A vertex is flat when its four neighbours share its height (its normal
    // is exactly vertical); a cell is flat when its four corners are flat at
    // one height. Runs of flat cells along a row merge into one quad: same
    // surface, same shading, far fewer triangles (text tops, plateaus).
    let flat_v = |x: usize, z: usize| {
        let y = at(x, z);
        at(x + 1, z) == y && at(x.saturating_sub(1), z) == y && at(x, z + 1) == y && at(x, z.saturating_sub(1)) == y
    };
    let full = heights.len() >= w * h;
    for z in 0..h - 1 {
        let mut x = 0;
        while x < w - 1 {
            if full && at(x, z) <= skip && at(x + 1, z) <= skip && at(x, z + 1) <= skip && at(x + 1, z + 1) <= skip {
                x += 1;
                continue;
            }
            let y = at(x, z);
            let flat_cell = |x: usize| flat_v(x, z) && flat_v(x + 1, z) && flat_v(x, z + 1) && flat_v(x + 1, z + 1) && at(x + 1, z) == y && at(x, z + 1) == y && at(x + 1, z + 1) == y;
            let mut x1 = x;
            if full && flat_cell(x) {
                while x1 + 1 < w - 1 && flat_cell(x1 + 1) {
                    x1 += 1;
                }
            }
            let (a, b, c, d) = ((z * w + x) as u32, (z * w + x1 + 1) as u32, ((z + 1) * w + x1 + 1) as u32, ((z + 1) * w + x) as u32);
            // Counter-clockwise seen from +Y.
            m.idx.extend_from_slice(&[a, c, b, a, d, c]);
            x = x1 + 1;
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
        let path: Vec<f32> = (0..20).flat_map(|i| [i as f32, libm::sinf(i as f32 * 0.4), 0.0]).collect();
        for m in [tube_path(&path, &[], 0.2, 6), cube(), sphere(8, 12), tube(|t| [libm::cosf(t * 6.283), libm::sinf(t * 6.283), 0.0], 0.2, 32, 8), plane(4, 3), cylinder(12, true), heightfield(8, 8, &hf, 0.0)] {
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

    #[test]
    fn points_are_occluded_and_streak() {
        let mut r = Raster::new(64, 64);
        r.clear(0, 255);
        let eye = [0.0, 0.0, 5.0];
        // A wall covering the left half, in front of the points.
        let mut wall = [1.0f32, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.1, -1.0, 0.0, 1.0].to_vec();
        wall.extend_from_slice(&[0.0, 0.0, 0.0]);
        let mat = Material { ambient: 0.0, diffuse: 0.0, specular: 0.0, rim: 0.0, ..Material::default() };
        r.draw(&cube(), &wall, &vp(), eye, &mat, &Lighting::default(), true);
        let pts = [-0.9f32, 0.0, 0.0, 0.6, 0.0, 0.0];
        let style = PointStyle { size: 0.05, intensity: 4.0, ..PointStyle::default() };
        r.points(&pts, &[], &vp(), None, eye, &style);
        let lum = |r: &Raster, x0: usize, x1: usize| (x0..x1).flat_map(|x| (28..36).map(move |y| (x, y))).map(|(x, y)| r.color[y * 64 + x] & 255).sum::<u32>();
        assert!(lum(&r, 36, 50) > 100, "right point visible");
        assert_eq!(lum(&r, 14, 28), 0, "left point hidden behind the wall");
        // Streaks: a moving camera smears a point along its screen motion.
        let mut r2 = Raster::new(64, 64);
        r2.clear(0, 255);
        let m = mul(&perspective(45.0, 1.0, 1.0, 100.0), &look_at([0.6, 0.0, 5.0], [0.6, 0.0, 0.0], [0.0, 1.0, 0.0]));
        let prev: M4 = core::array::from_fn(|i| m[i] as f32);
        r2.points(&pts[3..], &[], &vp(), Some(&prev), eye, &PointStyle { streak: 1.0, ..style });
        let lit = r2.color.iter().filter(|c| **c & 255 > 0).count();
        assert!(lit > 8, "streak covers several pixels: {lit}");
    }

    #[test]
    fn culls_instances_outside_the_frustum() {
        let mut r = Raster::new(32, 32);
        r.clear(0, 255);
        let mut inst = Vec::new();
        for (x, z) in [(0.0f32, 0.0f32), (0.0, 20.0), (50.0, 0.0)] {
            inst.extend_from_slice(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x, 0.0, z, 1.0, 1.0, 1.0]);
        }
        r.draw(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &Material::default(), &Lighting::default(), true);
        // Only the cube in view is rasterised (behind the camera and far to the side are culled).
        assert!(r.drawn >= 2 && r.drawn <= 6, "{}", r.drawn);
    }

    #[test]
    fn textures_tint_and_glow() {
        // 2×2 texture: red, green / blue, white.
        let t = Texture::from_rgba(2, 2, &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]);
        assert_eq!((t.width(), t.height()), (2, 2));
        let mut r = Raster::new(32, 32);
        r.clear(0, 255);
        let mut inst = ID.to_vec();
        inst.extend_from_slice(&[1.0, 1.0, 1.0]);
        let mat = Material { ambient: 0.0, diffuse: 0.0, specular: 0.0, rim: 0.0, tex_mode: 2, tex_gain: 1.0, ..Material::default() };
        r.draw_with(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &mat, &Lighting::default(), true, Blend::Opaque, Some(&t));
        // The front face shows all four colours (unlit, emissive).
        let mut seen = [false; 3];
        for c in &r.color {
            let (red, green, blue) = (c & 255, (c >> 8) & 255, (c >> 16) & 255);
            if red > 150 && green < 80 && blue < 80 { seen[0] = true; }
            if green > 150 && red < 80 && blue < 80 { seen[1] = true; }
            if blue > 150 && red < 80 && green < 80 { seen[2] = true; }
        }
        assert!(seen.iter().all(|s| *s), "{seen:?}");
    }

    #[test]
    fn camera_motion_blur_smears_along_the_move() {
        let mut r = Raster::new(64, 64);
        r.clear(0, 255);
        let mut inst = ID.to_vec();
        inst.extend_from_slice(&[1.0, 1.0, 1.0]);
        let mat = Material { ambient: 1.0, diffuse: 0.0, specular: 0.0, rim: 0.0, ..Material::default() };
        r.draw(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &mat, &Lighting::default(), true);
        let lit = |r: &Raster| r.color.iter().filter(|c| **c & 255 > 10).count();
        let before = lit(&r);
        // The previous camera was 1 unit to the left: the cube smears horizontally.
        let m = mul(&perspective(45.0, 1.0, 1.0, 100.0), &look_at([-1.0, 0.0, 5.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
        let prev: M4 = core::array::from_fn(|i| m[i] as f32);
        r.motion_blur(&vp(), &prev, 1.0, 40.0);
        assert!(lit(&r) > before + 30, "{} vs {}", lit(&r), before);
        // A static camera changes nothing.
        let snapshot = r.color.clone();
        r.motion_blur(&vp(), &vp(), 1.0, 40.0);
        assert_eq!(r.color, snapshot);
    }

    #[test]
    fn sky_fills_only_the_background() {
        let mut r = Raster::new(32, 32);
        r.clear(0, 255);
        let mut inst = ID.to_vec();
        inst.extend_from_slice(&[0.0, 0.0, 0.0]);
        let mat = Material { ambient: 0.0, diffuse: 0.0, specular: 0.0, rim: 0.0, ..Material::default() };
        r.draw(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &mat, &Lighting::default(), true);
        let light = Lighting { sky: [0.0, 0.0, 1.0], horizon: [1.0, 0.0, 0.0], ground: [0.0, 1.0, 0.0], ..Lighting::default() };
        r.sky(&vp(), &light, 0.0);
        assert_eq!(r.color[16 * 32 + 16] & 0xffffff, 0, "geometry untouched");
        assert!((r.color[0] >> 16) & 255 > 0 || r.color[0] & 255 > 0, "sky above");
        assert!((r.color[31 * 32] >> 8) & 255 > 0 || r.color[31 * 32] & 255 > 0, "ground below");
    }

    #[test]
    fn depth_of_field_blurs_out_of_focus_only() {
        let mut r = Raster::new(64, 64);
        r.clear(0, 255);
        // Stripes on a near cube: in focus they stay crisp, out of focus they blur.
        let mut inst = ID.to_vec();
        inst.extend_from_slice(&[1.0, 1.0, 1.0]);
        let mat = Material { ambient: 0.0, diffuse: 0.0, specular: 0.0, rim: 0.0, lines: 4.0, line_color: [1.0; 3], grid: [6.0, 0.0], line_width: 0.15, ..Material::default() };
        r.draw(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &mat, &Lighting::default(), true);
        let row = |r: &Raster| (16..48).map(|x| (r.color[32 * 64 + x] & 255) as i32).collect::<Vec<_>>();
        let contrast = |v: &[i32]| v.iter().max().unwrap() - v.iter().min().unwrap();
        let sharp = contrast(&row(&r));
        let mut a = Raster::new(64, 64);
        a.color = r.color.clone();
        a.depth = r.depth.clone();
        a.depth_of_field(&vp(), 4.5, 0.0, 10.0);
        assert_eq!(contrast(&row(&a)), sharp, "no aperture, no blur");
        r.depth_of_field(&vp(), 40.0, 40.0, 10.0);
        assert!(contrast(&row(&r)) < sharp, "out of focus blurs");
    }

    #[test]
    fn procedural_patterns_cover_part_of_each_cell() {
        // Sample a cell densely: every pattern lights some of it, none lights all of it.
        for kind in 1..=3u8 {
            let mut lit = 0;
            let n = 64;
            for i in 0..n {
                for j in 0..n {
                    let (x, y) = (2.0 + i as f32 / n as f32, 5.0 + j as f32 / n as f32);
                    let c = pattern4(kind, V4::splat(x), V4::splat(y), V4::splat(0.01), 0.06).to_array()[0];
                    assert!((0.0..=1.0).contains(&c));
                    if c > 0.5 {
                        lit += 1;
                    }
                }
            }
            assert!(lit > 50 && lit < n * n * 3 / 4, "pattern {kind}: {lit}");
        }
    }

    #[test]
    fn key_light_casts_shadows() {
        // Camera looks down at a floor; a cube floats above it; the light shines straight down.
        let m = mul(&perspective(60.0, 1.0, 0.5, 100.0), &look_at([0.0, 8.0, 0.01], [0.0; 3], [0.0, 1.0, 0.0]));
        let vp: M4 = core::array::from_fn(|i| m[i] as f32);
        // Orthographic light from above: x → x, z → y, y → depth (column-major).
        let e = 4.0f32;
        let lvp: M4 = [1.0 / e, 0.0, 0.0, 0.0, 0.0, 0.0, -0.1, 0.0, 0.0, -1.0 / e, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let floor = [6.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 6.0, 0.0, -1.0, 0.0, 0.8, 0.8, 0.8];
        let block = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.5, 1.0, 0.0, 0.8, 0.8, 0.8];
        let light = Lighting { key: [0.0, 1.0, 0.0], ..Lighting::default() };
        let mat = Material { ambient: 0.1, diffuse: 1.0, specular: 0.0, rim: 0.0, ..Material::default() };
        let mut r = Raster::new(64, 64);
        r.clear(0, 255);
        r.shadow_begin(&lvp, 256, 0.002, 1.0);
        r.shadow_draw(&cube(), &block, false);
        r.shadow_draw(&plane(1, 1), &floor, false);
        r.draw(&plane(1, 1), &floor, &vp, [0.0, 8.0, 0.0], &mat, &light, true);
        let lit = r.color.iter().map(|c| c & 255).max().unwrap();
        let dark = r.color.iter().filter(|c| **c & 255 > 0 && **c & 255 < lit / 3).count();
        // The block's shadow is a dark patch on the lit floor (the block itself is drawn from below as floor only).
        assert!(dark > 20 && dark < 64 * 64 / 4, "shadowed pixels: {dark} (lit {lit})");
    }

    #[test]
    fn point_lights_pool_light_locally() {
        // A dark floor seen from above with one red light near one side.
        let m = mul(&perspective(60.0, 1.0, 0.5, 100.0), &look_at([0.0, 8.0, 0.01], [0.0; 3], [0.0, 1.0, 0.0]));
        let vp: M4 = core::array::from_fn(|i| m[i] as f32);
        let floor = [6.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 6.0, 0.0, -1.0, 0.0, 1.0, 1.0, 1.0];
        let mut light = Lighting { key: [0.0, -1.0, 0.0], fill: [0.0, -1.0, 0.0], ..Lighting::default() };
        light.point_lights[0] = [2.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 3.0];
        light.point_count = 1;
        let mat = Material { ambient: 0.0, diffuse: 1.0, specular: 0.0, rim: 0.0, ..Material::default() };
        let mut r = Raster::new(64, 64);
        r.clear(0, 255);
        r.draw(&plane(1, 1), &floor, &vp, [0.0, 8.0, 0.0], &mat, &light, true);
        let red = r.color.iter().filter(|c| **c & 255 > 60).count();
        let green = r.color.iter().filter(|c| (**c >> 8) & 255 > 20).count();
        assert!(red > 20 && red < 64 * 64 / 2, "a pool, not a flood: {red}");
        assert_eq!(green, 0, "the light is red");
    }

    #[test]
    fn heightfield_merges_flat_runs() {
        // A 40×20 field: a raised plateau (rows 4..16, cols 5..35) on a flat base.
        let (w, h) = (40, 20);
        let hs: Vec<f32> = (0..w * h).map(|i| if (4..16).contains(&(i / w)) && (5..35).contains(&(i % w)) { 1.0 } else { 0.0 }).collect();
        let m = heightfield(w, h, &hs, -1.0);
        let tris = m.idx.len() / 3;
        assert!(tris * 2 < (w - 1) * (h - 1) * 2, "flat runs merge: {tris} triangles");
        // Same surface: the triangles still tile the whole 1×1 footprint.
        let area: f32 = m.idx.chunks(3).map(|t| {
            let p = |k: usize| [m.pos[t[k] as usize * 3], m.pos[t[k] as usize * 3 + 2]];
            let (a, b, c) = (p(0), p(1), p(2));
            ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() * 0.5
        }).sum();
        assert!((area - 1.0).abs() < 1e-4, "covers the footprint exactly once: {area}");
    }

    #[test]
    fn half_res_reflection_upsamples_into_the_frame() {
        let m = mul(&perspective(60.0, 1.0, 0.5, 100.0), &look_at([0.0, 0.0, 6.0], [0.0; 3], [0.0, 1.0, 0.0]));
        let vp: M4 = core::array::from_fn(|i| m[i] as f32);
        let cube_at = [2.0f32, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0, 0.1, 0.1];
        let mat = Material { ambient: 1.0, diffuse: 0.0, specular: 0.0, rim: 0.0, ..Material::default() };
        let mut r = Raster::new(64, 48);
        r.clear(0x000010, 255);
        r.reflection_begin(0x000010, 255);
        r.target().draw(&cube(), &cube_at, &vp, [0.0, 0.0, 6.0], &mat, &Lighting::default(), true);
        assert!(r.depth.iter().all(|d| d.is_infinite()), "draws went to the half-res target");
        r.reflection_end();
        let c = r.color[24 * 64 + 32];
        assert!(c & 255 > 150 && (c >> 8) & 255 < 60, "centre is the red cube: {c:08x}");
        assert_eq!(r.color[0] & 0x00ff_ffff, 0x100000, "background kept outside it");
        assert_eq!(r.color[0] >> 24, 255, "alpha kept");
        assert!(r.depth.iter().all(|d| d.is_infinite()), "depth cleared for the glossy surface");
        assert!(core::ptr::eq(r.target(), &r as *const Raster), "back to the frame");
    }
}
