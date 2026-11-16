//! 3D product shots: a lit primitive (rounded box, sphere, torus, knot,
//! cylinder, cube, extruded logo or an inline OBJ) spinning, orbiting and
//! floating over a soft contact shadow.
//!
//! The mesh is built once in [`Prepare`]. Each frame the model is drawn by
//! `anikit-core`'s software rasteriser into an image covering only the
//! model's on-screen bounds, supersampled and box-filtered down to straight
//! RGBA (coverage from the depth buffer gives the alpha, so the model sits on
//! whatever is behind it). Images are keyed by parameters and frame-quantised
//! time, so repeated frames are free. The shadow is vector.

use super::style::*;
use super::{rgba, Component, Ctx, Error, Prepare};
use crate::dl::{Op, Rgba};
use anikit_core::raster::{self, Lighting, Material, Mesh, Raster, M4};
use anikit_core::space;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

/// Largest side of the rendered image (before supersampling).
const MAX_SIDE: f32 = 2048.0;
/// Vertical field of view: a long, product-photography lens.
const FOV: f32 = 28.0;
/// Camera elevation above the model's equator (degrees).
const ELEVATION: f32 = 15.0;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Model {
    /// `"rounded-box"`, `"sphere"`, `"torus"`, `"knot"`, `"cylinder"`,
    /// `"cube"` or `"logo"` (extrudes `points`). Ignored when `obj` is set.
    pub shape: String,
    /// Inline Wavefront OBJ text (`v`, `vn`, `f`); replaces `shape`.
    pub obj: Option<String>,
    /// Logo outline `[[x, y], ...]` (y down, any units), extruded by `depth`.
    pub points: Vec<[f32; 2]>,
    /// Logo thickness as a fraction of its larger dimension.
    pub depth: f32,
    /// Rounded-box size `[w, h, d]` (proportions) and corner radius
    /// (fraction of the smallest side).
    pub size: [f32; 3],
    pub radius: f32,
    /// Base colour (defaults to the theme accent).
    pub color: Option<Color>,
    /// 0 = plastic/ceramic, 1 = polished metal.
    pub metallic: f32,
    /// 0 = mirror-sharp highlights, 1 = matte.
    pub roughness: f32,
    /// Turntable speed around the model's vertical axis (degrees per second).
    pub spin: f32,
    /// Starting turntable angle (degrees).
    pub angle: f32,
    /// Tilt towards the camera (degrees).
    pub tilt: f32,
    /// Camera orbit around the model (degrees per second).
    pub orbit: f32,
    /// Gentle bob amplitude (fraction of the model's size).
    pub float: f32,
    /// Size multiplier (1 fills about three quarters of the box).
    pub scale: f32,
    /// Seconds before the model springs in.
    pub delay: f32,
    /// Contact shadow strength (0 or `false` = none).
    #[serde(deserialize_with = "num_or_bool")]
    pub shadow: f32,
    /// Render scale (1 = native box pixels).
    pub resolution: f32,
    #[serde(skip)]
    mesh: Mesh,
    /// Cull back faces (closed meshes with known winding).
    #[serde(skip)]
    cull: bool,
    #[serde(skip)]
    key: u64,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            shape: "rounded-box".into(),
            obj: None,
            points: Vec::new(),
            depth: 0.22,
            size: [1.0, 1.0, 1.0],
            radius: 0.22,
            color: None,
            metallic: 0.3,
            roughness: 0.3,
            spin: 28.0,
            angle: -30.0,
            tilt: 16.0,
            orbit: 0.0,
            float: 0.035,
            scale: 1.0,
            delay: 0.0,
            shadow: 0.6,
            resolution: 1.0,
            mesh: Mesh::default(),
            cull: true,
            key: 0,
        }
    }
}

fn num_or_bool<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Bool(b) => Ok(if b { 0.6 } else { 0.0 }),
        serde_json::Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) as f32),
        v => Err(serde::de::Error::custom(format!("`shadow` must be a number or a boolean, got {v}"))),
    }
}

const SHAPES: &[&str] = &["rounded-box", "sphere", "torus", "knot", "cylinder", "cube", "logo"];

impl Prepare for Model {
    fn prepare(&mut self) -> Result<(), Error> {
        for (name, v) in [("depth", self.depth), ("radius", self.radius), ("metallic", self.metallic), ("roughness", self.roughness), ("scale", self.scale), ("resolution", self.resolution), ("shadow", self.shadow)] {
            if !v.is_finite() || v < 0.0 {
                return Err(Error(format!("`{name}` must be a non-negative number, got {v}")));
            }
        }
        for (name, v) in [("spin", self.spin), ("angle", self.angle), ("tilt", self.tilt), ("orbit", self.orbit), ("float", self.float), ("delay", self.delay)] {
            if !v.is_finite() {
                return Err(Error(format!("`{name}` must be a finite number")));
            }
        }
        if self.size.iter().any(|s| !s.is_finite() || *s <= 0.0) {
            return Err(Error("`size` must be three positive numbers".into()));
        }
        let mut mesh = if let Some(src) = &self.obj {
            self.cull = false;
            parse_obj(src).map_err(Error)?
        } else {
            match self.shape.as_str() {
                "rounded-box" => {
                    let r = self.radius.min(0.5) * self.size.iter().cloned().fold(f32::INFINITY, f32::min);
                    rounded_box(self.size.map(|s| s * 0.5), r, 7)
                }
                "sphere" => raster::sphere(72, 144),
                "torus" => {
                    // Lean the ring back so it reads as a ring, not a disc edge.
                    let mut m = raster::torus(1.0, 0.4, 128, 56);
                    rotate_x(&mut m, -62f32.to_radians());
                    m
                }
                "knot" => raster::torus_knot(2.0, 3.0, 1.0, 0.42, 0.2, 720, 40),
                "cylinder" => {
                    let mut m = raster::cylinder(128, true);
                    for p in m.pos.chunks_exact_mut(3) {
                        p[0] *= 0.62;
                        p[1] *= 1.25;
                        p[2] *= 0.62;
                    }
                    m
                }
                "cube" => raster::cube(),
                "logo" => {
                    let pts = if self.points.is_empty() { default_logo() } else { self.points.clone() };
                    extrude(&pts, self.depth).map_err(Error)?
                }
                s => {
                    return Err(Error(match closest(s, SHAPES) {
                        Some(c) => format!("unknown shape `{s}`; did you mean `{c}`?"),
                        None => format!("unknown shape `{s}`; expected one of {}", SHAPES.join(", ")),
                    }))
                }
            }
        };
        normalise(&mut mesh);
        orient(&mut mesh);
        self.mesh = mesh;
        let mut h = Vec::new();
        h.extend_from_slice(self.shape.as_bytes());
        h.extend_from_slice(self.obj.as_deref().unwrap_or("").as_bytes());
        for v in self.points.iter().flatten().chain(&self.size).chain(&[
            self.depth,
            self.radius,
            self.metallic,
            self.roughness,
            self.spin,
            self.angle,
            self.tilt,
            self.orbit,
            self.float,
            self.scale,
            self.delay,
            self.resolution,
        ]) {
            h.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        self.key = fnv(&[b"model", &h]);
        Ok(())
    }
}

impl Component for Model {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let fps = ctx.fps.max(1.0);
        let frame = (t * fps).round();
        let t = frame / fps;
        let local = t - self.delay;
        if local < 0.0 || ctx.width < 2.0 || ctx.height < 2.0 || self.scale <= 0.0 {
            return Vec::new();
        }
        let opacity = ease_out(seg(local, 0.0, 0.45));
        let grow = 0.8 + 0.2 * spring(local, 0.9);
        let (w, h) = (ctx.width, ctx.height);
        let aspect = w / h;
        let half = (FOV * 0.5).to_radians().tan();

        // Camera: framed so the unit bounding sphere fills ~72 % of the short side.
        let fit = 0.72 * aspect.min(1.0);
        let dist = 1.0 / (fit * half) + 0.35;
        let orbit = (self.orbit * t).to_radians();
        let el = ELEVATION.to_radians();
        let eye = [dist * el.cos() * orbit.sin(), dist * el.sin(), dist * el.cos() * orbit.cos()];
        let view = space::look_at(eye.map(|v| v as f64), [0.0; 3], [0.0, 1.0, 0.0]);
        let proj = space::perspective(FOV as f64, aspect as f64, (dist * 0.2) as f64, (dist * 3.0) as f64);
        let vp_full = space::mul(&proj, &view);

        // Model transform: turntable spin, then tilt towards the camera, then bob.
        let s = self.scale * grow;
        let yaw = (self.angle + self.spin * t).to_radians();
        let pitch = self.tilt.to_radians();
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        // R = Rx(pitch) · Ry(yaw), columns.
        let c0 = [cy, sp * sy, -cp * sy];
        let c1 = [0.0, cp, sp];
        let c2 = [sy, -sp * cy, cp * cy];
        let phase = (t * std::f32::consts::TAU / 3.6).sin();
        let bob = self.float * phase * s;
        let rot = [c0, c1, c2];

        // Lowest point of the rotated mesh (for the shadow's ground plane).
        let mut low = 0.0f32;
        for p in self.mesh.pos.chunks_exact(3).step_by(3) {
            low = low.min(c0[1] * p[0] + c1[1] * p[1] + c2[1] * p[2]);
        }
        let ground = low * s - 0.03 * s;

        let mut out = Vec::new();
        let to_px = |m: &space::Mat4, p: [f32; 3]| -> Option<(f32, f32, f32)> {
            let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
            let cw = m[3] * x + m[7] * y + m[11] * z + m[15];
            if cw <= 1e-6 {
                return None;
            }
            let cx = (m[0] * x + m[4] * y + m[8] * z + m[12]) / cw;
            let cy = (m[1] * x + m[5] * y + m[9] * z + m[13]) / cw;
            Some(((cx as f32 + 1.0) * 0.5 * w, (1.0 - cy as f32) * 0.5 * h, cw as f32))
        };

        // Contact shadow: a wide soft pool and a tighter, darker core.
        if self.shadow > 0.0 {
            if let Some((sx, sy, depth)) = to_px(&vp_full, [0.0, ground, 0.0]) {
                let focal = h * 0.5 / half;
                let rx = 0.9 * s * focal / depth;
                let lift = phase * (self.float * 6.0).clamp(-1.0, 1.0);
                let spread = 1.0 + 0.25 * lift;
                let a = self.shadow * opacity * (1.0 - 0.35 * lift).clamp(0.3, 1.2);
                let flat = (el.sin() + 0.1).min(0.5);
                for (k, (rr, aa)) in [(1.3 * spread, 0.5), (0.7 * spread, 0.75)].into_iter().enumerate() {
                    let r = rx * rr;
                    let stops = if k == 0 {
                        vec![(0.0, rgba(0.0, 0.0, 0.0, a * aa)), (0.45, rgba(0.0, 0.0, 0.0, a * aa * 0.45)), (1.0, rgba(0.0, 0.0, 0.0, 0.0))]
                    } else {
                        vec![(0.0, rgba(0.0, 0.0, 0.0, a * aa)), (0.6, rgba(0.0, 0.0, 0.0, a * aa * 0.35)), (1.0, rgba(0.0, 0.0, 0.0, 0.0))]
                    };
                    out.push(Op::Fill {
                        path: circle_path(0.0, 0.0, r),
                        paint: radial(0.0, 0.0, r, stops),
                        rule: crate::dl::FillRule::NonZero,
                        transform: [1.0, 0.0, 0.0, flat, sx, sy + r * flat * 0.3],
                    });
                }
            }
        }

        // Screen bounds of the bounding sphere (radius s around (0, bob, 0)).
        let mut lo = [f32::INFINITY; 2];
        let mut hi = [f32::NEG_INFINITY; 2];
        for i in 0..8 {
            let p = [if i & 1 == 0 { -s } else { s }, bob + if i & 2 == 0 { -s } else { s }, if i & 4 == 0 { -s } else { s }];
            if let Some((x, y, _)) = to_px(&vp_full, p) {
                lo = [lo[0].min(x), lo[1].min(y)];
                hi = [hi[0].max(x), hi[1].max(y)];
            } else {
                lo = [0.0, 0.0];
                hi = [w, h];
            }
        }
        let x0 = (lo[0] - 2.0).floor().clamp(0.0, w);
        let y0 = (lo[1] - 2.0).floor().clamp(0.0, h);
        let x1 = (hi[0] + 2.0).ceil().clamp(0.0, w.floor());
        let y1 = (hi[1] + 2.0).ceil().clamp(0.0, h.floor());
        if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
            return out;
        }
        let (rw, rh) = (x1 - x0, y1 - y0);
        let res = self.resolution.max(0.05).min(MAX_SIDE / rw.max(rh));
        let iw = (rw * res).round().max(1.0) as u32;
        let ih = (rh * res).round().max(1.0) as u32;

        // Off-centre projection onto the sub-rectangle.
        let (cxn, hxn) = ((x0 + x1) / w - 1.0, rw / w);
        let (cyn, hyn) = (1.0 - (y0 + y1) / h, rh / h);
        let mut sub = [0.0f64; 16];
        sub[0] = 1.0 / hxn as f64;
        sub[5] = 1.0 / hyn as f64;
        sub[10] = 1.0;
        sub[15] = 1.0;
        sub[12] = -(cxn / hxn) as f64;
        sub[13] = -(cyn / hyn) as f64;
        let m = space::mul(&sub, &vp_full);
        let vp: M4 = std::array::from_fn(|i| m[i] as f32);

        let base = self.color.map(|c| c.0).unwrap_or(ctx.theme.accent);
        let dark = ctx.theme.dark;
        let key = fnv(&[
            &self.key.to_le_bytes(),
            &frame.to_bits().to_le_bytes(),
            &fps.to_bits().to_le_bytes(),
            &[w.to_bits(), h.to_bits(), x0.to_bits(), y0.to_bits(), iw, ih].map(u32::to_le_bytes).concat(),
            &base.0.map(f32::to_bits).map(u32::to_le_bytes).concat(),
            &[dark as u8],
        ]);
        let inst: [f32; 15] = [
            rot[0][0] * s, rot[0][1] * s, rot[0][2] * s,
            rot[1][0] * s, rot[1][1] * s, rot[1][2] * s,
            rot[2][0] * s, rot[2][1] * s, rot[2][2] * s,
            0.0, bob, 0.0,
            base.0[0], base.0[1], base.0[2],
        ];
        let (mat, light) = self.look(base, dark);
        let cull = self.cull;
        let mesh = &self.mesh;
        let image = ctx.assets.image(key, iw, ih, &mut || render(mesh, &inst, &vp, eye, &mat, &light, cull, iw as usize, ih as usize));
        out.push(Op::Image { image, dst: [x0, y0, rw, rh], opacity, transform: crate::dl::IDENTITY });
        out
    }
}

impl Model {
    /// Material and studio lighting: warm key upper-left, cool fill right,
    /// a cool rim from behind, softbox strips for the reflections.
    fn look(&self, base: Rgba, dark: bool) -> (Material, Lighting) {
        let rough = self.roughness.clamp(0.0, 1.0);
        let metal = self.metallic.clamp(0.0, 1.0);
        let gloss = 1.0 - rough;
        let mat = Material {
            ambient: 0.16 + 0.06 * rough,
            diffuse: 0.95 - 0.55 * metal,
            specular: (0.25 + 0.95 * gloss * gloss) * (1.0 + 0.6 * metal),
            shininess: 6.0 + 260.0 * gloss * gloss * gloss,
            rim: 0.32,
            metal: (metal * (0.35 + 0.65 * gloss) + 0.16 * gloss * gloss).min(0.95),
            fog: 0.0,
            ..Material::default()
        };
        let b = base.0;
        let tint = |k: usize, a: f32| 1.0 + (b[k] - 0.5) * a;
        let floor = if dark { 0.025 } else { 0.18 };
        let light = Lighting {
            key: [-0.55, 0.72, 0.62],
            key_color: [1.08, 1.02, 0.95],
            fill: [0.8, 0.05, 0.4],
            fill_color: [0.3 * tint(0, 0.4), 0.33 * tint(1, 0.4), 0.42 * tint(2, 0.4)],
            rim_color: [0.72, 0.8, 1.0],
            sky: [0.3, 0.32, 0.38],
            horizon: [0.46, 0.47, 0.52],
            ground: [floor, floor, floor * 1.1],
            strips: [1.25, 1.25, 1.32],
            point_lights: {
                let mut pl = [[0.0; 8]; 8];
                // Kicker behind, above and to the right.
                pl[0] = [2.2, 2.0, -2.6, 0.85, 0.9, 1.0, 3.0, 1.1];
                pl
            },
            point_count: 1,
            ..Lighting::default()
        };
        (mat, light)
    }
}

/// Render at 2× and box-filter down to straight RGBA; alpha is coverage.
/// Horizontal bands render in parallel, each with its own off-centre
/// projection (the vertex work per band is small next to the shading).
fn render(mesh: &Mesh, inst: &[f32; 15], vp: &M4, eye: [f32; 3], mat: &Material, light: &Lighting, cull: bool, w: usize, h: usize) -> Vec<u8> {
    use rayon::prelude::*;
    const SS: usize = 2;
    let band = h.div_ceil(rayon::current_num_threads().max(1) * 3).max(24);
    let mut out = vec![0u8; w * h * 4];
    out.par_chunks_mut(band * w * 4).enumerate().for_each(|(k, dst)| {
        let rows = dst.len() / (w * 4);
        let (y0, y1) = ((k * band) as f32, (k * band + rows) as f32);
        // Squeeze NDC y in [1 - 2·y1/h, 1 - 2·y0/h] onto [-1, 1].
        let (cy, hy) = (1.0 - (y0 + y1) / h as f32, (y1 - y0) / h as f32);
        let mut bvp = *vp;
        for c in 0..4 {
            bvp[4 * c + 1] = (vp[4 * c + 1] - cy * vp[4 * c + 3]) / hy;
        }
        let (sw, sh) = (w * SS, rows * SS);
        let mut r = Raster::new(sw, sh);
        r.clear(0, 0);
        r.draw(mesh, inst, &bvp, eye, mat, light, cull);
        for y in 0..rows {
            for x in 0..w {
                let mut acc = [0u32; 3];
                let mut n = 0u32;
                for dy in 0..SS {
                    let row = (y * SS + dy) * sw + x * SS;
                    for i in row..row + SS {
                        if r.depth[i].is_finite() {
                            let c = r.color[i];
                            acc[0] += c & 255;
                            acc[1] += (c >> 8) & 255;
                            acc[2] += (c >> 16) & 255;
                            n += 1;
                        }
                    }
                }
                if n == 0 {
                    continue;
                }
                let o = (y * w + x) * 4;
                dst[o] = ((acc[0] + n / 2) / n) as u8;
                dst[o + 1] = ((acc[1] + n / 2) / n) as u8;
                dst[o + 2] = ((acc[2] + n / 2) / n) as u8;
                dst[o + 3] = ((n * 255 + (SS * SS) as u32 / 2) / (SS * SS) as u32) as u8;
            }
        }
    });
    out
}

// ------------------------------------------------------------------ meshes

/// Centre the mesh and scale it to a unit bounding sphere.
fn normalise(m: &mut Mesh) {
    let (c, r) = raster::bounds(m);
    let k = 1.0 / r.max(1e-6);
    for p in m.pos.chunks_exact_mut(3) {
        for i in 0..3 {
            p[i] = (p[i] - c[i]) * k;
        }
    }
}

/// Make triangles counter-clockwise around their normals (front faces for
/// the rasteriser's back-face culling), flipping the winding when most
/// triangles disagree with their vertex normals.
fn orient(m: &mut Mesh) {
    let p = |i: u32| -> [f32; 3] { let i = i as usize * 3; [m.pos[i], m.pos[i + 1], m.pos[i + 2]] };
    let mut score = 0.0f64;
    for t in m.idx.chunks_exact(3) {
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let j = t[0] as usize * 3;
        score += (n[0] * m.nrm[j] + n[1] * m.nrm[j + 1] + n[2] * m.nrm[j + 2]).signum() as f64;
    }
    if score < 0.0 {
        for t in m.idx.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
    }
}

fn rotate_x(m: &mut Mesh, a: f32) {
    let (s, c) = a.sin_cos();
    for v in m.pos.chunks_exact_mut(3).chain(m.nrm.chunks_exact_mut(3)) {
        let (y, z) = (v[1], v[2]);
        v[1] = c * y - s * z;
        v[2] = s * y + c * z;
    }
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// Box with half extents `he` and edges rounded by `r`, `k` segments per
/// eighth of a turn at each edge (angles evenly spaced, so highlights glide).
fn rounded_box(he: [f32; 3], r: f32, k: usize) -> Mesh {
    let coords = |hk: f32| -> Vec<f32> {
        let inner = hk - r;
        let mut v: Vec<f32> = (0..=k).rev().map(|i| -inner - r * (i as f32 / k as f32 * std::f32::consts::FRAC_PI_4).tan()).collect();
        v.extend((0..=k).map(|i| inner + r * (i as f32 / k as f32 * std::f32::consts::FRAC_PI_4).tan()));
        v
    };
    let faces: [([f32; 3], usize, usize); 6] = [
        ([1.0, 0.0, 0.0], 1, 2),
        ([-1.0, 0.0, 0.0], 2, 1),
        ([0.0, 1.0, 0.0], 2, 0),
        ([0.0, -1.0, 0.0], 0, 2),
        ([0.0, 0.0, 1.0], 0, 1),
        ([0.0, 0.0, -1.0], 1, 0),
    ];
    let mut m = Mesh::default();
    for (n, ua, va) in faces {
        let (cu, cv) = (coords(he[ua]), coords(he[va]));
        let b = (m.pos.len() / 3) as u32;
        for &v in &cv {
            for &u in &cu {
                let mut p = [n[0] * he[0], n[1] * he[1], n[2] * he[2]];
                p[ua] = u;
                p[va] = v;
                let q: [f32; 3] = std::array::from_fn(|i| p[i].clamp(-(he[i] - r), he[i] - r));
                let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
                let nn = if d.iter().all(|x| x.abs() < 1e-7) { n } else { norm(d) };
                m.pos.extend_from_slice(&[q[0] + nn[0] * r, q[1] + nn[1] * r, q[2] + nn[2] * r]);
                m.nrm.extend_from_slice(&nn);
                m.uv.extend_from_slice(&[0.0, 0.0]);
            }
        }
        let nu = cu.len() as u32;
        for j in 0..cv.len() as u32 - 1 {
            for i in 0..nu - 1 {
                let a = b + j * nu + i;
                m.idx.extend_from_slice(&[a, a + 1, a + 1 + nu, a, a + 1 + nu, a + nu]);
            }
        }
    }
    m
}

/// A lightning bolt, the default logo.
fn default_logo() -> Vec<[f32; 2]> {
    vec![[58.0, 0.0], [12.0, 62.0], [44.0, 62.0], [32.0, 112.0], [86.0, 40.0], [52.0, 40.0], [70.0, 0.0]]
}

fn area2(p: &[[f32; 2]]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a
}

/// Ear-clipping triangulation of a simple counter-clockwise polygon.
fn triangulate(p: &[[f32; 2]]) -> Vec<u32> {
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let mut idx: Vec<usize> = (0..p.len()).collect();
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < p.len() * p.len() {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (a, b, c) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            if cross(p[a], p[b], p[c]) <= 1e-9 {
                continue;
            }
            let inside = idx.iter().any(|&j| {
                j != a && j != b && j != c && cross(p[a], p[b], p[j]) >= 0.0 && cross(p[b], p[c], p[j]) >= 0.0 && cross(p[c], p[a], p[j]) >= 0.0
            });
            if !inside {
                out.extend_from_slice(&[a as u32, b as u32, c as u32]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // Degenerate input: drop a vertex rather than loop forever.
            idx.remove(0);
        }
    }
    if idx.len() == 3 {
        out.extend(idx.iter().map(|&i| i as u32));
    }
    out
}

/// Extrude a polygon outline (y down) to a slab `depth` × its larger side
/// thick, with smooth side normals across shallow corners.
fn extrude(points: &[[f32; 2]], depth: f32) -> Result<Mesh, String> {
    let mut p: Vec<[f32; 2]> = points.iter().map(|q| [q[0], -q[1]]).collect();
    if p.len() > 1 && p[0] == p[p.len() - 1] {
        p.pop();
    }
    p.dedup();
    if p.len() < 3 || p.iter().flatten().any(|v| !v.is_finite()) {
        return Err("`points` needs at least three finite [x, y] pairs".into());
    }
    if area2(&p).abs() < 1e-9 {
        return Err("`points` encloses no area".into());
    }
    if area2(&p) < 0.0 {
        p.reverse();
    }
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for q in &p {
        for k in 0..2 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    let p: Vec<[f32; 2]> = p.iter().map(|q| [(q[0] - (lo[0] + hi[0]) * 0.5) * 2.0 / span, (q[1] - (lo[1] + hi[1]) * 0.5) * 2.0 / span]).collect();
    let hz = depth.max(0.01);
    let tris = triangulate(&p);
    let n = p.len();
    let mut m = Mesh::default();
    // Caps.
    for (z, nz) in [(hz, 1.0f32), (-hz, -1.0)] {
        let b = (m.pos.len() / 3) as u32;
        for q in &p {
            m.pos.extend_from_slice(&[q[0], q[1], z]);
            m.nrm.extend_from_slice(&[0.0, 0.0, nz]);
            m.uv.extend_from_slice(&[0.0, 0.0]);
        }
        for t in tris.chunks_exact(3) {
            if nz > 0.0 {
                m.idx.extend_from_slice(&[b + t[0], b + t[1], b + t[2]]);
            } else {
                m.idx.extend_from_slice(&[b + t[0], b + t[2], b + t[1]]);
            }
        }
    }
    // Sides: per-edge outward normals, averaged with a neighbour when the corner is shallow.
    let en: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = (dx * dx + dy * dy).sqrt().max(1e-9);
            [dy / l, -dx / l]
        })
        .collect();
    let smooth = |a: [f32; 2], b: [f32; 2]| -> [f32; 2] {
        if a[0] * b[0] + a[1] * b[1] > 0.82 {
            let (x, y) = (a[0] + b[0], a[1] + b[1]);
            let l = (x * x + y * y).sqrt().max(1e-9);
            [x / l, y / l]
        } else {
            a
        }
    };
    for i in 0..n {
        let j = (i + 1) % n;
        let na = smooth(en[i], en[(i + n - 1) % n]);
        let nb = smooth(en[i], en[j]);
        let b = (m.pos.len() / 3) as u32;
        for (q, nn) in [(p[i], na), (p[j], nb)] {
            for z in [hz, -hz] {
                m.pos.extend_from_slice(&[q[0], q[1], z]);
                m.nrm.extend_from_slice(&[nn[0], nn[1], 0.0]);
                m.uv.extend_from_slice(&[0.0, 0.0]);
            }
        }
        // b: a front, b+1: a back, b+2: b front, b+3: b back.
        m.idx.extend_from_slice(&[b + 1, b + 3, b + 2, b + 1, b + 2, b]);
    }
    Ok(m)
}

/// Minimal Wavefront OBJ: `v`, `vn` and polygonal `f` (fan-triangulated,
/// negative indices allowed). Missing normals are smoothed from the faces.
fn parse_obj(src: &str) -> Result<Mesh, String> {
    let mut v: Vec<[f32; 3]> = Vec::new();
    let mut vn: Vec<[f32; 3]> = Vec::new();
    let mut corners: Vec<(usize, Option<usize>)> = Vec::new();
    let mut faces: Vec<[usize; 3]> = Vec::new();
    let resolve = |s: &str, len: usize, line: usize| -> Result<usize, String> {
        let i: i64 = s.parse().map_err(|_| format!("obj line {line}: bad index `{s}`"))?;
        let r = if i < 0 { len as i64 + i } else { i - 1 };
        if r < 0 || r as usize >= len {
            return Err(format!("obj line {line}: index {i} out of range"));
        }
        Ok(r as usize)
    };
    for (ln, line) in src.lines().enumerate() {
        let mut it = line.split_whitespace();
        let nums = |it: std::str::SplitWhitespace| -> Result<[f32; 3], String> {
            let f: Vec<f32> = it.take(3).map(|x| x.parse::<f32>()).collect::<Result<_, _>>().map_err(|_| format!("obj line {}: bad number", ln + 1))?;
            if f.len() < 3 || f.iter().any(|x| !x.is_finite()) {
                return Err(format!("obj line {}: expected three numbers", ln + 1));
            }
            Ok([f[0], f[1], f[2]])
        };
        match it.next() {
            Some("v") => v.push(nums(it)?),
            Some("vn") => vn.push(nums(it)?),
            Some("f") => {
                let mut poly = Vec::new();
                for c in it {
                    let mut parts = c.split('/');
                    let vi = resolve(parts.next().unwrap_or(""), v.len(), ln + 1)?;
                    let ni = match parts.nth(1) {
                        Some(s) if !s.is_empty() => Some(resolve(s, vn.len(), ln + 1)?),
                        _ => None,
                    };
                    corners.push((vi, ni));
                    poly.push(corners.len() - 1);
                }
                if poly.len() < 3 {
                    return Err(format!("obj line {}: a face needs three corners", ln + 1));
                }
                for k in 1..poly.len() - 1 {
                    faces.push([poly[0], poly[k], poly[k + 1]]);
                }
            }
            _ => {}
        }
    }
    if faces.is_empty() {
        return Err("obj: no faces".into());
    }
    // Smooth normals per position for corners without one.
    let mut acc = vec![[0.0f32; 3]; v.len()];
    for f in &faces {
        let [a, b, c] = f.map(|i| v[corners[i].0]);
        let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let fnrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        for i in f {
            let s = &mut acc[corners[*i].0];
            for k in 0..3 {
                s[k] += fnrm[k];
            }
        }
    }
    let mut m = Mesh::default();
    let mut map: HashMap<(usize, Option<usize>), u32> = HashMap::new();
    for f in &faces {
        for &ci in f {
            let c = corners[ci];
            let id = *map.entry(c).or_insert_with(|| {
                let id = (m.pos.len() / 3) as u32;
                m.pos.extend_from_slice(&v[c.0]);
                m.nrm.extend_from_slice(&norm(c.1.map(|n| vn[n]).unwrap_or(acc[c.0])));
                m.uv.extend_from_slice(&[0.0, 0.0]);
                id
            });
            m.idx.push(id);
        }
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::super::preview::{snapshot, validate};
    use super::super::{build, Theme};
    use super::*;
    use crate::dl::FillRule;
    use kurbo::Shape;

    fn backdrop(w: f32, h: f32) -> Op {
        Op::Fill {
            path: kurbo::Rect::new(0.0, 0.0, w as f64, h as f64).into_path(0.1),
            paint: radial(w * 0.5, h * 0.42, w * 0.7, vec![(0.0, rgba(0.13, 0.13, 0.17, 1.0)), (1.0, rgba(0.03, 0.03, 0.045, 1.0))]),
            rule: FillRule::NonZero,
            transform: crate::dl::IDENTITY,
        }
    }

    const OCTA: &str = "v 1 0 0\nv -1 0 0\nv 0 1 0\nv 0 -1 0\nv 0 0 1\nv 0 0 -1\nf 1 3 5\nf 3 2 5\nf 2 4 5\nf 4 1 5\nf 3 1 6\nf 2 3 6\nf 4 2 6\nf 1 4 6\n";

    fn cases() -> Vec<(&'static str, serde_json::Value)> {
        use serde_json::json;
        vec![
            ("rounded-box", json!({"shape": "rounded-box"})),
            ("sphere", json!({"shape": "sphere", "metallic": 0.1, "roughness": 0.15})),
            ("torus", json!({"shape": "torus", "color": "#ff6fb1"})),
            ("knot", json!({"shape": "knot", "metallic": 0.9, "roughness": 0.15, "color": "#c9ccd6"})),
            ("cylinder", json!({"shape": "cylinder", "color": "#2ad4e8"})),
            ("cube", json!({"shape": "cube", "color": "#ffb547", "roughness": 0.6})),
            ("logo", json!({"shape": "logo", "color": "#ffb547", "metallic": 0.6})),
            ("obj", json!({"obj": OCTA, "color": "#3ddc97"})),
        ]
    }

    #[test]
    fn shapes_render() {
        let theme = Theme::default();
        let assets = super::super::preview::host();
        let (w, h) = (640.0, 400.0);
        for (name, params) in cases() {
            let p = build("model", &params).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(p.component.duration().is_none());
            let mut seen = 0;
            for t in [0.0, 1.0, 2.5] {
                let ops = p.eval(t, w, h, 30.0, &theme, assets);
                validate(&ops).unwrap_or_else(|e| panic!("{name} at {t}: {e}"));
                assert!(ops.iter().any(|o| matches!(o, Op::Image { .. })), "{name}: no image at {t}");
                if t == 2.5 {
                    let mut all = vec![backdrop(w, h)];
                    all.extend(ops);
                    let px = snapshot(&format!("three-{name}"), w as u32, h as u32, theme.background, all, assets);
                    // Model pixels are lit and not the backdrop.
                    seen = px.chunks_exact(4).filter(|c| c[0].max(c[1]).max(c[2]) > 90).count();
                }
            }
            assert!(seen > 2000, "{name}: only {seen} bright pixels");
        }
    }

    #[test]
    fn entrance_and_caching() {
        let theme = Theme::default();
        let assets = super::super::preview::host();
        let p = build("model", &serde_json::json!({"shape": "sphere", "delay": 0.5, "spin": 0})).unwrap();
        assert!(p.eval(0.2, 320.0, 240.0, 30.0, &theme, assets).is_empty());
        let a = p.eval(0.6, 320.0, 240.0, 30.0, &theme, assets);
        let op = a.iter().find_map(|o| if let Op::Image { opacity, .. } = o { Some(*opacity) } else { None }).unwrap();
        assert!(op > 0.0 && op < 1.0);
        let n = assets.image_count();
        p.eval(0.6, 320.0, 240.0, 30.0, &theme, assets);
        assert_eq!(assets.image_count(), n, "same frame reuses the image");
    }

    #[test]
    fn errors() {
        let e = build("model", &serde_json::json!({"shape": "torsu"})).err().unwrap();
        assert!(e.0.contains("did you mean `torus`"), "{e}");
        assert!(build("model", &serde_json::json!({"shape": "logo", "points": [[0, 0], [1, 1]]})).is_err());
        assert!(build("model", &serde_json::json!({"obj": "v 0 0 0\nf 1 2 3"})).is_err());
        assert!(build("model", &serde_json::json!({"shadow": false})).is_ok());
    }

    /// `cargo test --release -p anikit-video three::tests::timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn timing() {
        let theme = Theme::default();
        for (name, params) in cases() {
            let assets = super::super::preview::host();
            let p = build("model", &params).unwrap();
            let n = 30;
            let t0 = std::time::Instant::now();
            for i in 0..n {
                p.eval(i as f32 / 30.0, 1280.0, 720.0, 30.0, &theme, assets);
            }
            println!("{name:12} {:.2} ms/frame", t0.elapsed().as_secs_f64() * 1e3 / n as f64);
        }
    }
}
