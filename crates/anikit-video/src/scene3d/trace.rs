//! The `scene3d` renderer: a ray tracer with physically based shading.
//!
//! * **Geometry:** shapes (triangle BVHs or analytic spheres) placed by
//!   instances (rigid transform + uniform scale); a top-level hierarchy over
//!   the instances' world boxes is rebuilt per frame, so thousands of moving
//!   instances stay cheap.
//! * **Visibility:** one primary ray per pixel, then a second pass adds
//!   jittered samples only where neighbours disagree (object edges, sharp
//!   reflections). Samples resolve in tone-mapped weighted space so bright
//!   highlights don't alias.
//! * **Shading:** GGX/Schlick split-sum specular against the analytic,
//!   pre-filtered environment ([`super::env`]); reflections of the scene are
//!   traced. Glass is a dielectric with Fresnel reflection, refraction and
//!   Beer–Lambert absorption (thick edges go deep green). Thin glass (panes)
//!   lets primary rays continue straight through, so what's behind stays in
//!   the denoised G-buffer instead of becoming noisy secondary shading.
//! * **Diffuse light** for primary surfaces is split into albedo × (ambient ×
//!   AO + key × visibility); AO and key visibility come from few traced rays
//!   and are denoised edge-aware in [`super::post`]. The floor can use a
//!   baked visibility texture instead ([`FloorVis`]).
//! * **Floor:** a plane whose reflection lives in its own buffer and is blurred
//!   by the distance to what it reflects; it fades into the backdrop.
//! * **Haze:** exponential fog towards the haze colour.

use super::bvh::{Bvh, Tlas};
use super::env::Env;
use super::math::*;
use std::sync::Arc;
use vello_cpu::Pixmap;

#[derive(Clone, Debug)]
pub struct Material {
    /// Linear base colour.
    pub base: V3,
    pub metallic: f32,
    pub roughness: f32,
    /// Dielectric reflectance scale (1 = F0 0.04).
    pub specular: f32,
    /// Emitted radiance (linear, already multiplied by strength).
    pub emissive: V3,
    /// Clear lacquer on top (0..1).
    pub clearcoat: f32,
    /// 0 = opaque, 1 = clear dielectric (glass).
    pub transmission: f32,
    pub ior: f32,
    /// Absorption per metre inside the material (Beer–Lambert, linear RGB).
    pub absorb: V3,
    /// How much a texture glows on its own (0 = only lit, 1 = a screen).
    pub tex_glow: f32,
}

impl Material {
    pub fn chrome() -> Material {
        Material { base: v3(0.94, 0.94, 0.96), metallic: 1.0, roughness: 0.06, specular: 1.0, emissive: v3(0.0, 0.0, 0.0), clearcoat: 0.0, transmission: 0.0, ior: 1.5, absorb: v3(0.0, 0.0, 0.0), tex_glow: 0.7 }
    }
}

/// An image texture (premultiplied sRGB RGBA8), or a flat average for far LODs.
#[derive(Clone)]
pub enum Tex {
    Image(Arc<Pixmap>),
    /// Linear premultiplied RGBA.
    Flat([f32; 4]),
}

/// Which faces show the texture (object-space normal direction).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Face {
    Front,
    Back,
    Both,
}

fn srgb_lut() -> &'static [f32; 256] {
    static L: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    L.get_or_init(|| {
        std::array::from_fn(|i| {
            let v = i as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}

impl Tex {
    /// Linear premultiplied RGBA at `uv` (bilinear, clamped).
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        match self {
            Tex::Flat(c) => *c,
            Tex::Image(pm) => {
                let (w, h) = (pm.width() as usize, pm.height() as usize);
                let data = pm.data_as_u8_slice();
                let x = (u.clamp(0.0, 1.0) * w as f32 - 0.5).clamp(0.0, w as f32 - 1.0);
                let y = (v.clamp(0.0, 1.0) * h as f32 - 0.5).clamp(0.0, h as f32 - 1.0);
                let (ix, iy) = (x as usize, y as usize);
                let (tx, ty) = (x - ix as f32, y - iy as f32);
                let (ix1, iy1) = ((ix + 1).min(w - 1), (iy + 1).min(h - 1));
                let lut = srgb_lut();
                let texel = |i: usize| -> [f32; 4] {
                    let o = i * 4;
                    let a = data[o + 3];
                    if a == 0 {
                        return [0.0; 4];
                    }
                    let af = a as f32 / 255.0;
                    let un = |c: u8| lut[((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as usize] * af;
                    [un(data[o]), un(data[o + 1]), un(data[o + 2]), af]
                };
                let (a, b, c, d) = (texel(iy * w + ix), texel(iy * w + ix1), texel(iy1 * w + ix), texel(iy1 * w + ix1));
                std::array::from_fn(|k| (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty)
            }
        }
    }

    /// Average colour (for far LODs).
    pub fn average(pm: &Pixmap) -> [f32; 4] {
        let lut = srgb_lut();
        let data = pm.data_as_u8_slice();
        let mut acc = [0.0f64; 4];
        let step = (data.len() / 4 / 4096).max(1);
        let mut n = 0.0f64;
        for px in data.chunks_exact(4).step_by(step) {
            let a = px[3];
            if a > 0 {
                let af = a as f32 / 255.0;
                for k in 0..3 {
                    acc[k] += (lut[((px[k] as u32 * 255 + a as u32 / 2) / a as u32).min(255) as usize] * af) as f64;
                }
                acc[3] += af as f64;
            }
            n += 1.0;
        }
        std::array::from_fn(|k| (acc[k] / n.max(1.0)) as f32)
    }
}

/// Object geometry, in object space.
#[derive(Clone)]
pub enum Shape {
    Mesh(Arc<Bvh>),
    /// The unit sphere, intersected analytically (perfectly round at any size).
    Sphere,
}

/// A hit in object space: distance, shading normal, geometric normal, UV.
type ObjHit = (f32, V3, V3, [f32; 2]);

impl Shape {
    pub fn bounds(&self) -> (V3, V3) {
        match self {
            Shape::Mesh(b) => (b.lo, b.hi),
            Shape::Sphere => (v3(-1.0, -1.0, -1.0), v3(1.0, 1.0, 1.0)),
        }
    }
    /// Closest hit distance and where (triangle and barycentrics; unused for
    /// spheres), without the surface attributes ([`Shape::surface`]).
    #[inline]
    fn hit_t(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<(f32, u32, f32, f32)> {
        match self {
            Shape::Mesh(b) => b.intersect(o, d, tmin, tmax).map(|h| (h.t, h.tri, h.u, h.v)),
            Shape::Sphere => sphere_t(o, d, tmin, tmax).map(|t| (t, 0, 0.0, 0.0)),
        }
    }

    /// The attributes of a hit found by [`Shape::hit_t`] (same values as [`Shape::hit`]).
    #[inline]
    fn surface(&self, o: V3, d: V3, h: (f32, u32, f32, f32)) -> ObjHit {
        let (t, tri, u, v) = h;
        match self {
            Shape::Mesh(b) => {
                let tr = &b.tris[tri as usize];
                let w = 1.0 - u - v;
                let uv = [tr.uv[0][0] * w + tr.uv[1][0] * u + tr.uv[2][0] * v, tr.uv[0][1] * w + tr.uv[1][1] * u + tr.uv[2][1] * v];
                (t, add(add(scale(tr.n[0], w), scale(tr.n[1], u)), scale(tr.n[2], v)), cross(tr.e1, tr.e2), uv)
            }
            Shape::Sphere => {
                let n = madd(o, d, t);
                let uv = [0.5 + n.x.atan2(n.z) / std::f32::consts::TAU, 0.5 - n.y.clamp(-1.0, 1.0).asin() / std::f32::consts::PI];
                (t, n, n, uv)
            }
        }
    }

    #[inline]
    #[allow(dead_code)]
    fn hit(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<ObjHit> {
        match self {
            Shape::Mesh(b) => b.intersect(o, d, tmin, tmax).map(|h| {
                let tr = &b.tris[h.tri as usize];
                let w = 1.0 - h.u - h.v;
                let uv = [tr.uv[0][0] * w + tr.uv[1][0] * h.u + tr.uv[2][0] * h.v, tr.uv[0][1] * w + tr.uv[1][1] * h.u + tr.uv[2][1] * h.v];
                (h.t, add(add(scale(tr.n[0], w), scale(tr.n[1], h.u)), scale(tr.n[2], h.v)), cross(tr.e1, tr.e2), uv)
            }),
            Shape::Sphere => sphere_t(o, d, tmin, tmax).map(|t| {
                let n = madd(o, d, t);
                let uv = [0.5 + n.x.atan2(n.z) / std::f32::consts::TAU, 0.5 - n.y.clamp(-1.0, 1.0).asin() / std::f32::consts::PI];
                (t, n, n, uv)
            }),
        }
    }
    #[inline]
    fn occluded(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> bool {
        match self {
            Shape::Mesh(b) => b.occluded(o, d, tmin, tmax),
            Shape::Sphere => sphere_t(o, d, tmin, tmax).is_some(),
        }
    }
}

#[inline]
fn sphere_t(o: V3, d: V3, tmin: f32, tmax: f32) -> Option<f32> {
    let a = dot(d, d);
    let b = dot(o, d);
    let c = dot(o, o) - 1.0;
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let t0 = (-b - sq) / a;
    if t0 > tmin && t0 < tmax {
        return Some(t0);
    }
    let t1 = (-b + sq) / a;
    (t1 > tmin && t1 < tmax).then_some(t1)
}

/// One placed object.
#[derive(Clone)]
pub struct Inst {
    pub shape: u32,
    pub xf: Xf,
    /// Transform one frame earlier (for motion vectors).
    pub prev: Xf,
    pub mat: u32,
    /// Texture prints, each on its faces (e.g. one front, another back).
    pub tex: Vec<(Tex, Face)>,
    /// Thin glass: primary rays pass straight through (no lateral offset).
    pub thin: bool,
    /// Casts shadows and occludes.
    pub shadow: bool,
    /// Seen by camera rays (false for reflector cards: they only show in
    /// reflections and light the scene).
    pub cam: bool,
    /// Which object, instance index and lane this came from (for textures).
    pub obj: u32,
    pub i: u32,
    pub lane: u32,
}

/// Baked floor lighting: key-light visibility and ambient occlusion over a
/// rectangle of the floor.
pub struct FloorVis {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
    pub nx: usize,
    pub nz: usize,
    /// (key visibility, ambient occlusion) per texel.
    pub data: Vec<[f32; 2]>,
}

impl FloorVis {
    #[inline]
    pub fn sample(&self, x: f32, z: f32) -> [f32; 2] {
        let fx = (x - self.x0) / (self.x1 - self.x0) * self.nx as f32 - 0.5;
        let fz = (z - self.z0) / (self.z1 - self.z0) * self.nz as f32 - 0.5;
        if fx < -1.0 || fz < -1.0 || fx > self.nx as f32 || fz > self.nz as f32 {
            return [1.0, 1.0];
        }
        let (ix, iz) = (fx.floor(), fz.floor());
        let (tx, tz) = (fx - ix, fz - iz);
        let at = |i: i32, k: i32| -> [f32; 2] {
            if i < 0 || k < 0 || i >= self.nx as i32 || k >= self.nz as i32 {
                [1.0, 1.0]
            } else {
                self.data[k as usize * self.nx + i as usize]
            }
        };
        let (i, k) = (ix as i32, iz as i32);
        let (a, b, c, d) = (at(i, k), at(i + 1, k), at(i, k + 1), at(i + 1, k + 1));
        let l = |j: usize| (a[j] * (1.0 - tx) + b[j] * tx) * (1.0 - tz) + (c[j] * (1.0 - tx) + d[j] * tx) * tz;
        [l(0), l(1)]
    }
}

pub struct Floor {
    pub y: f32,
    pub color: V3,
    pub roughness: f32,
    /// Extra mirror reflectance on top of Fresnel (0 = plain lacquer).
    pub mirror: f32,
    /// Overall reflection strength (0 = matte paper, 1 = lacquer).
    pub reflect: f32,
    /// Light pool: the key's light on the floor falls off around this centre
    /// (radius 0 = no falloff).
    pub pool: (f32, f32, f32),
    /// The floor fades into the backdrop between these distances from the
    /// pool centre (equal = never).
    pub fade: (f32, f32),
    pub vis: Option<Arc<FloorVis>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: V3,
    pub target: V3,
    /// Vertical field of view (degrees).
    pub fov: f32,
    /// Focus distance (world units) and aperture (blur-circle radius in
    /// pixels at 1080p for points at infinity).
    pub focus: f32,
    pub aperture: f32,
    pub roll: f32,
    /// Shutter as a fraction of the frame interval.
    pub shutter: f32,
}

impl Camera {
    pub fn basis(&self) -> (V3, V3, V3) {
        let f = norm(sub(self.target, self.pos));
        let r0 = norm(cross(f, v3(0.0, 1.0, 0.0)));
        let u0 = cross(r0, f);
        let a = self.roll.to_radians();
        let r = add(scale(r0, a.cos()), scale(u0, a.sin()));
        let u = add(scale(u0, a.cos()), scale(r0, -a.sin()));
        (f, r, u)
    }
}

/// A camera's per-frame constants for one image size: rays and projection
/// without per-sample trigonometry (bit-identical to [`Scene::cam_ray`]).
#[derive(Clone, Copy)]
pub struct Rig {
    pub pos: V3,
    pub f: V3,
    pub r: V3,
    pub u: V3,
    /// tan(fov / 2), and that times the aspect ratio.
    pub th: f32,
    pub tha: f32,
    pub aspect: f32,
    pub w: f32,
    pub h: f32,
}

impl Rig {
    pub fn new(cam: &Camera, w: usize, h: usize) -> Rig {
        let (f, r, u) = cam.basis();
        let th = (cam.fov * 0.5).to_radians().tan();
        let aspect = w as f32 / h as f32;
        Rig { pos: cam.pos, f, r, u, th, tha: th * aspect, aspect, w: w as f32, h: h as f32 }
    }

    /// The ray through image point `(sx, sy)` (pixels).
    #[inline]
    pub fn ray(&self, sx: f32, sy: f32) -> V3 {
        let nx = (sx / self.w * 2.0 - 1.0) * self.th * self.aspect;
        let ny = (1.0 - sy / self.h * 2.0) * self.th;
        norm(add(self.f, add(scale(self.r, nx), scale(self.u, ny))))
    }

    /// Where world point `p` lands on the image (pixels).
    #[inline]
    pub fn project(&self, p: V3) -> (f32, f32) {
        let q = sub(p, self.pos);
        let z = dot(q, self.f).max(1e-4);
        ((dot(q, self.r) / z / self.tha + 1.0) * 0.5 * self.w, (1.0 - dot(q, self.u) / z / self.th) * 0.5 * self.h)
    }
}

pub struct Scene {
    pub shapes: Vec<Shape>,
    pub mats: Vec<Material>,
    pub insts: Vec<Inst>,
    pub tlas: Tlas,
    pub env: Env,
    pub floor: Option<Floor>,
    /// Backdrop colours (linear): at the horizon and at the top of frame.
    pub backdrop: (V3, V3),
    /// A soft glow on the backdrop: direction, colour, angular width (1 - cos).
    pub glow: (V3, V3, f32),
    /// Exponential haze: density per metre, and colour (linear).
    pub haze: (f32, V3),
    /// Local point and spot lights.
    pub lights: Vec<Light>,
}

#[derive(Clone, Copy)]
pub struct Quality {
    /// Extra samples where a pixel disagrees with its neighbours.
    pub edge_spp: u32,
    pub ao_rays: u32,
    pub shadow_rays: u32,
    pub max_depth: u32,
    /// Glass interfaces a ray may cross.
    pub max_glass: u32,
    /// Luminance contrast (tone-mapped) that triggers refinement.
    pub contrast: f32,
    /// Shadow and AO rays at secondary hits (0 = unoccluded, noise-free).
    pub secondary_rays: u32,
    /// Reference mode: every pixel takes `edge_spp` extra samples and no
    /// speed-only shortcut applies (the quality harness's ground truth).
    pub full: bool,
}

impl Default for Quality {
    fn default() -> Self {
        Quality { edge_spp: 6, ao_rays: 1, shadow_rays: 1, max_depth: 3, max_glass: 10, contrast: 0.06, secondary_rays: 0, full: false }
    }
}

/// A local point or spot light (inverse-square falloff, hard traced shadow).
#[derive(Clone, Debug)]
pub struct Light {
    pub pos: V3,
    /// Colour × intensity: radiance factor at 1 m.
    pub power: V3,
    /// Spot axis and cone (cosines of the outer and inner half angles);
    /// a point light has `cos_outer` = -1.
    pub dir: V3,
    pub cos_outer: f32,
    pub cos_inner: f32,
    pub shadow: bool,
}

impl Scene {
    /// Light arriving at `p` (normal `n`) from the local lights: (diffuse
    /// irradiance factor for albedo 1, GGX specular toward `v`).
    fn local_lights(&self, p: V3, n: V3, v: V3, rough: f32, f0: V3) -> (V3, V3) {
        let mut dif = v3(0.0, 0.0, 0.0);
        let mut spec = v3(0.0, 0.0, 0.0);
        for l in &self.lights {
            let to = sub(l.pos, p);
            let d2 = dot(to, to).max(1e-4);
            let dist = d2.sqrt();
            let ld = scale(to, 1.0 / dist);
            let nl = dot(n, ld);
            if nl <= 0.0 {
                continue;
            }
            let mut k = 1.0 / d2;
            if l.cos_outer > -1.0 {
                let c = dot(scale(ld, -1.0), l.dir);
                k *= smoothstep(l.cos_outer, l.cos_inner.max(l.cos_outer + 1e-4), c);
            }
            if k <= 1e-7 {
                continue;
            }
            if l.shadow && self.transmit(madd(p, n, 2e-3), ld, dist - 2e-3, false) <= 0.0 {
                continue;
            }
            let e = scale(l.power, k * nl);
            dif = add(dif, e);
            // GGX highlight (roughness floored so small lights don't sparkle).
            let a = rough.max(0.08) * rough.max(0.08);
            let hv = norm(add(ld, v));
            let nh = dot(n, hv).max(0.0);
            let nv = dot(n, v).max(1e-4);
            let dd = nh * nh * (a * a - 1.0) + 1.0;
            let dterm = a * a / (std::f32::consts::PI * dd * dd);
            let kk = a * 0.5;
            let g = (nl / (nl * (1.0 - kk) + kk)) * (nv / (nv * (1.0 - kk) + kk));
            let vh = dot(v, hv).max(0.0);
            let fr = madd(f0, sub(v3(1.0, 1.0, 1.0), f0), (1.0 - vh).powi(5));
            spec = add(spec, scale(mul(fr, e), dterm * g / (4.0 * nv)));
        }
        (dif, spec)
    }
}

impl Scene {
    /// Light scattered toward the camera by fog lit by the key and the local
    /// lights (single scattering, Henyey–Greenstein g = 0.35), ray-marched
    /// with traced visibility at half resolution: god rays and light cones.
    /// `depth` is the full-resolution view depth; returns `w/2 × h/2` radiance.
    pub fn volumetrics(&self, cam: &Camera, w: usize, h: usize, depth: &[f32], density: f32, steps: u32) -> Vec<[f32; 3]> {
        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
        let (f, _, _) = cam.basis();
        let g = 0.35f32;
        let hg = |c: f32| (1.0 - g * g) / (4.0 * std::f32::consts::PI * (1.0 + g * g - 2.0 * g * c).max(1e-4).powf(1.5));
        let key = self.env.key_box().map(|k| (k.c, scale(k.radiance, self.env.key_solid_angle)));
        let row = |y: usize| -> Vec<[f32; 3]> {
            (0..hw)
                .map(|x| {
                    let d = Self::cam_ray(cam, hw, hh, x as f32 + 0.5, y as f32 + 0.5);
                    // Nearest surface in this 2×2 block (march no further).
                    let mut dv = f32::MAX;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let (sx, sy) = ((2 * x + dx).min(w - 1), (2 * y + dy).min(h - 1));
                        dv = dv.min(depth[sy * w + sx]);
                    }
                    let tmax = (dv / dot(d, f).max(1e-3)).min(60.0);
                    let jit = u01(hash2(x as u32 * 7 + 3, y as u32 * 13 + 1));
                    let mut acc = v3(0.0, 0.0, 0.0);
                    // The key (directional) lights the whole ray.
                    if let Some((kd, kl)) = key {
                        let dt = tmax / steps as f32;
                        let ph = hg(dot(kd, d));
                        for i in 0..steps {
                            let t = (i as f32 + jit) * dt;
                            if self.transmit(madd(cam.pos, d, t), kd, 1e6, false) > 0.0 {
                                acc = madd(acc, kl, (-density * t).exp() * density * dt * ph);
                            }
                        }
                    }
                    // Each local light is marched only where it matters: the
                    // part of the ray inside its sphere of influence.
                    for l in &self.lights {
                        let reach = if l.cos_outer > -1.0 { 8.0f32.max(luma(l.power).sqrt() * 1.2) } else { luma(l.power).sqrt() * 1.5 };
                        let oc = sub(cam.pos, l.pos);
                        let b = dot(oc, d);
                        let c = dot(oc, oc) - reach * reach;
                        let disc = b * b - c;
                        if disc <= 0.0 {
                            continue;
                        }
                        let sq = disc.sqrt();
                        let (t0, t1) = ((-b - sq).max(0.0), (-b + sq).min(tmax));
                        if t1 <= t0 {
                            continue;
                        }
                        let dt = (t1 - t0) / steps as f32;
                        for i in 0..steps {
                            let t = t0 + (i as f32 + jit) * dt;
                            let x3 = madd(cam.pos, d, t);
                            let to = sub(l.pos, x3);
                            let d2 = dot(to, to).max(1e-3);
                            let dist = d2.sqrt();
                            let ld = scale(to, 1.0 / dist);
                            let mut k = 1.0 / d2;
                            if l.cos_outer > -1.0 {
                                k *= smoothstep(l.cos_outer, l.cos_inner.max(l.cos_outer + 1e-4), dot(scale(ld, -1.0), l.dir));
                            }
                            if k < 1e-6 {
                                continue;
                            }
                            if l.shadow && self.transmit(x3, ld, dist, false) <= 0.0 {
                                continue;
                            }
                            acc = madd(acc, l.power, (-density * t).exp() * density * dt * k * hg(dot(ld, d)) * 4.0 * std::f32::consts::PI);
                        }
                    }
                    [acc.x, acc.y, acc.z]
                })
                .collect()
        };
        use rayon::prelude::*;
        let rows: Vec<Vec<[f32; 3]>> = if rayon::current_thread_index().is_some() { (0..hh).map(row).collect() } else { (0..hh).into_par_iter().map(row).collect() };
        let raw: Vec<[f32; 3]> = rows.into_iter().flatten().collect();
        // Two 5×5 binomial passes remove the jitter (shafts are smooth).
        let mut out = raw.clone();
        let mut raw = raw;
        for _pass in 0..2 {
        for y in 0..hh {
            for x in 0..hw {
                let mut a = [0.0f32; 3];
                let mut ws = 0.0;
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let (sx, sy) = (x as i32 + dx, y as i32 + dy);
                        if sx < 0 || sy < 0 || sx >= hw as i32 || sy >= hh as i32 {
                            continue;
                        }
                        let wt = [1.0, 4.0, 6.0, 4.0, 1.0][(dx + 2) as usize] * [1.0, 4.0, 6.0, 4.0, 1.0][(dy + 2) as usize];
                        let v = raw[sy as usize * hw + sx as usize];
                        for k in 0..3 {
                            a[k] += v[k] * wt;
                        }
                        ws += wt;
                    }
                }
                out[y * hw + x] = a.map(|v| v / ws);
            }
        }
        raw.clone_from(&out);
        }
        out
    }
}

/// What a ray hit.
#[derive(Clone, Copy)]
struct Surf {
    t: f32,
    p: V3,
    ng: V3,
    ns: V3,
    uv: [f32; 2],
    /// Object-space geometric normal (for texture faces).
    on: V3,
    /// Instance index, or `FLOOR`.
    obj: usize,
}

const FLOOR: usize = usize::MAX;

/// Ray counters for `ANIKIT_SCENE3D_PROFILE` (closest-hit, shadow, primary samples).
pub static RAYS: [std::sync::atomic::AtomicU64; 6] = [const { std::sync::atomic::AtomicU64::new(0) }; 6];
#[inline]
fn count(k: usize) {
    if cfg!(debug_assertions) || PROFILE.load(std::sync::atomic::Ordering::Relaxed) {
        // Per-thread tallies, flushed in batches (a shared atomic per ray
        // would serialise the threads and distort the profile).
        thread_local!(static LOCAL: std::cell::Cell<[u32; 6]> = const { std::cell::Cell::new([0; 6]) });
        LOCAL.with(|c| {
            let mut v = c.get();
            v[k] += 1;
            if v[k] >= 256 {
                RAYS[k].fetch_add(v[k] as u64, std::sync::atomic::Ordering::Relaxed);
                v[k] = 0;
            }
            c.set(v);
        });
    }
}
/// Profiling: wall µs of the centre pass and the refinement pass, refined pixels.
pub static TIMES: [std::sync::atomic::AtomicU64; 3] = [const { std::sync::atomic::AtomicU64::new(0) }; 3];
pub static PROFILE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const EPS: f32 = 1e-4;

/// Secondary rays whose whole-path weight (the luminance factor their
/// radiance is scaled by on its way to the pixel) is below this take the
/// pre-filtered environment instead of a traced ray: the error is bounded by
/// the weight times the radiance difference, under one 8-bit level here.
const W_MIN: f32 = 0.003;

thread_local!(static PATH_W: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) });

/// The current path weight (see [`W_MIN`]).
#[inline]
fn path_w() -> f32 {
    PATH_W.with(|c| c.get())
}

/// Run `f` with the path weight multiplied by `k`.
#[inline]
fn weighted<R>(k: f32, f: impl FnOnce() -> R) -> R {
    PATH_W.with(|c| {
        let old = c.get();
        c.set(old * k);
        let r = f();
        c.set(old);
        r
    })
}

/// Diffuse lighting split for denoising: albedo, unoccluded ambient,
/// unshadowed key, and the AO / key visibility to apply to them.
#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Diffuse {
    alb: V3,
    amb: V3,
    key: V3,
    ao: f32,
    vk: f32,
    /// Environment specular (already weighted) that AO should occlude.
    spec: V3,
}

impl Diffuse {
    #[inline]
    fn light(&self) -> V3 {
        add(mul(self.alb, add(scale(self.amb, self.ao), scale(self.key, self.vk))), scale(self.spec, self.ao))
    }
}

/// Per-pixel output of the primary pass.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Px {
    /// Everything except primary diffuse light and the floor reflection.
    pub col: [f32; 3],
    /// Diffuse albedo (with throughput) and its unoccluded light.
    pub alb: [f32; 3],
    pub amb: [f32; 3],
    pub key: [f32; 3],
    pub ao: f32,
    pub vk: f32,
    /// Environment specular to occlude with the denoised AO.
    pub spec: [f32; 3],
    pub refl: [f32; 3],
    /// Weighted floor coverage and reflection blur radius (pixels).
    pub cov: f32,
    pub rad: f32,
    /// Floor reflection fade × haze (applied after the blur).
    pub rf: f32,
    /// Coverage of floor samples that traced their reflection.
    pub rcov: f32,
    /// Weight of samples whose visibility was traced (0 = fill from neighbours).
    pub vw: f32,
    /// View depth (distance along the camera axis) and motion (pixels per frame).
    pub depth: f32,
    pub vel: [f32; 2],
    /// Normal (for edge-aware denoising) and hit id.
    pub nrm: [f32; 3],
    pub id: u32,
}

// SAFETY: `Px` is `repr(C)` with only f32/u32 fields (no padding, any bits valid).
unsafe impl super::kernel::pool::Pod for Px {}

#[derive(Clone, Copy)]
#[repr(C)]
struct Sample {
    col: V3,
    dif: Diffuse,
    refl: V3,
    /// 1 for a directly seen floor sample.
    floor: u32,
    rad: f32,
    /// Floor reflection fade × haze, applied after the blur.
    rf: f32,
    /// 1 if this floor sample traced its reflection (0: checkerboard gap).
    rw: f32,
    /// 1 if shadow/AO visibility was traced here (0: checkerboard gap).
    vw: f32,
    depth: f32,
    vel: (f32, f32),
    nrm: V3,
    _pad: u32,
    id: usize,
}

// SAFETY: `repr(C)`, only f32/u32/usize fields laid out without padding.
unsafe impl super::kernel::pool::Pod for Sample {}

impl Sample {
    #[inline]
    fn total(&self) -> V3 {
        add(add(self.col, self.refl), self.dif.light())
    }
}

#[inline]
fn fresnel_dielectric(cos_i: f32, eta: f32) -> f32 {
    // eta = n_i / n_t
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i).max(0.0);
    if sin2_t >= 1.0 {
        return 1.0;
    }
    let cos_t = (1.0 - sin2_t).sqrt();
    let rs = (eta * cos_i - cos_t) / (eta * cos_i + cos_t);
    let rp = (cos_i - eta * cos_t) / (cos_i + eta * cos_t);
    0.5 * (rs * rs + rp * rp)
}

#[inline]
fn refract(d: V3, n: V3, eta: f32) -> Option<V3> {
    let c = -dot(d, n);
    let k = 1.0 - eta * eta * (1.0 - c * c);
    (k >= 0.0).then(|| norm(add(scale(d, eta), scale(n, eta * c - k.sqrt()))))
}

impl Scene {
    pub fn new(shapes: Vec<Shape>, mats: Vec<Material>, insts: Vec<Inst>, env: Env, floor: Option<Floor>) -> Scene {
        let mut s = Scene {
            shapes,
            mats,
            insts,
            tlas: Tlas::new(&[]),
            env,
            floor,
            backdrop: (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)),
            glow: (v3(0.0, 0.0, -1.0), v3(0.0, 0.0, 0.0), 0.1),
            haze: (0.0, v3(0.0, 0.0, 0.0)),
            lights: Vec::new(),
        };
        s.rebuild();
        s
    }

    /// Rebuild the top-level hierarchy after instances moved.
    pub fn rebuild(&mut self) {
        let boxes: Vec<(V3, V3)> = self.insts.iter().map(|i| self.world_box(i)).collect();
        self.tlas = Tlas::new(&boxes);
    }

    pub fn world_box(&self, i: &Inst) -> (V3, V3) {
        let (blo, bhi) = self.shapes[i.shape as usize].bounds();
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for k in 0..8 {
            let c = v3(if k & 1 == 0 { blo.x } else { bhi.x }, if k & 2 == 0 { blo.y } else { bhi.y }, if k & 4 == 0 { blo.z } else { bhi.z });
            let p = i.xf.point(c);
            lo = vmin(lo, p);
            hi = vmax(hi, p);
        }
        (lo, hi)
    }

    fn trace(&self, o: V3, d: V3, tmax: f32) -> Option<Surf> {
        self.trace_f(o, d, tmax, false)
    }

    /// Closest hit; with `camera`, objects hidden from the camera (cards) are skipped.
    fn trace_f(&self, o: V3, d: V3, tmax: f32, camera: bool) -> Option<Surf> {
        count(0);
        let mut best: Option<Surf> = None;
        let mut tm = tmax;
        if let Some(f) = &self.floor {
            if d.y < -1e-7 {
                let t = (f.y - o.y) / d.y;
                if t > EPS && t < tm {
                    tm = t;
                    let up = v3(0.0, 1.0, 0.0);
                    best = Some(Surf { t, p: madd(o, d, t), ng: up, ns: up, uv: [0.0; 2], on: up, obj: FLOOR });
                }
            }
        }
        // The nearest object hit so far: instance, object-space ray, hit.
        // Surface attributes are computed once, for the final hit only.
        let mut obj: Option<(u32, V3, V3, (f32, u32, f32, f32))> = None;
        self.tlas.visit(o, d, EPS, tm, |k, tm| {
            let ob = &self.insts[k as usize];
            if camera && !ob.cam {
                return tm;
            }
            let lo = ob.xf.point_inv(o);
            let ld = ob.xf.dir_inv(d);
            if let Some(hh) = self.shapes[ob.shape as usize].hit_t(lo, ld, EPS, tm) {
                obj = Some((k, lo, ld, hh));
                hh.0
            } else {
                tm
            }
        });
        if let Some((k, lo, ld, hh)) = obj {
            let ob = &self.insts[k as usize];
            let (t, sn, gn, uv) = self.shapes[ob.shape as usize].surface(lo, ld, hh);
            best = Some(Surf { t, p: madd(o, d, t), ng: ob.xf.normal(gn), ns: ob.xf.normal(sn), uv, on: norm(gn), obj: k as usize });
        }
        best
    }

    /// Light getting through along a segment (0 = blocked); glass lets most through.
    fn transmit(&self, o: V3, d: V3, tmax: f32, floor: bool) -> f32 {
        count(1);
        if floor {
            if let Some(f) = &self.floor {
                if d.y < -1e-7 {
                    let t = (f.y - o.y) / d.y;
                    if t > EPS && t < tmax {
                        return 0.0;
                    }
                }
            }
        }
        let mut vis = 1.0f32;
        self.tlas.visit(o, d, EPS, tmax, |k, tm| {
            let ob = &self.insts[k as usize];
            if !ob.shadow {
                return tm;
            }
            let lo = ob.xf.point_inv(o);
            let ld = ob.xf.dir_inv(d);
            if self.shapes[ob.shape as usize].occluded(lo, ld, EPS, tm) {
                let m = &self.mats[ob.mat as usize];
                if m.transmission > 0.0 {
                    // Glass: about 85% through each pane (two interfaces).
                    vis *= (1.0 - 0.15 * m.transmission.min(1.0)) * m.transmission.min(1.0);
                    if vis < 0.02 {
                        vis = 0.0;
                        return -1.0;
                    }
                    tm
                } else {
                    vis = 0.0;
                    -1.0
                }
            } else {
                tm
            }
        });
        vis
    }

    /// The backdrop seen by rays that miss everything.
    #[inline]
    pub fn backdrop(&self, d: V3) -> V3 {
        let base = lerp3(self.backdrop.0, self.backdrop.1, smoothstep(-0.05, 0.6, d.y));
        let (g, c, w) = self.glow;
        madd(base, c, (-(1.0 - dot(d, g)) / w.max(1e-4)).exp())
    }

    /// Haze between a ray origin and a hit at distance `t`.
    #[inline]
    fn hazed(&self, c: V3, t: f32) -> V3 {
        if self.haze.0 <= 0.0 {
            return c;
        }
        lerp3(c, self.haze.1, 1.0 - (-self.haze.0 * t).exp())
    }

    /// Key softbox visibility (0..1) and ambient occlusion (0..1) at `p`.
    fn visibility(&self, p: V3, n: V3, rays_k: u32, rays_ao: u32, rot: (f32, f32), ao_dist: f32) -> (f32, f32) {
        let o = madd(p, n, 2e-3);
        let mut vk = 1.0;
        if let Some(k) = self.env.key_box() {
            if rays_k > 0 && dot(n, k.c) > -0.3 {
                let mut lit = 0.0;
                for i in 0..rays_k {
                    let (a, b) = r2(i, rot);
                    let d = k.dir_at(a * 2.0 - 1.0, b * 2.0 - 1.0);
                    lit += if dot(d, n) <= 0.0 { 1.0 } else { self.transmit(o, d, 1e6, false) };
                }
                vk = lit / rays_k as f32;
            }
        }
        let mut ao = 1.0;
        if rays_ao > 0 {
            let mut sum = 0.0;
            let rot2 = (rot.1, rot.0);
            for i in 0..rays_ao {
                let (a, b) = r2(i, rot2);
                let d = cosine_dir(n, a, b);
                sum += 1.0 - self.transmit(o, d, ao_dist, true);
            }
            ao = 1.0 - sum / rays_ao as f32;
        }
        (vk, ao)
    }

    /// Diffuse light from the key softbox for normal `n` (radiance for albedo 1, unshadowed).
    #[inline]
    fn key_diffuse(&self, n: V3) -> V3 {
        match self.env.key_box() {
            Some(k) => {
                let c = dot(n, k.c).max(0.0);
                scale(k.radiance, self.env.key_solid_angle * c * (1.0 - k.falloff / 3.0) / std::f32::consts::PI)
            }
            None => v3(0.0, 0.0, 0.0),
        }
    }

    /// Environment seen by a secondary ray that escapes.
    #[inline]
    fn sky(&self, d: V3, rough: f32) -> V3 {
        self.env.radiance(d, rough)
    }

    /// Radiance arriving along a secondary ray (diffuse resolved inline).
    fn radiance(&self, o: V3, d: V3, depth: u32, glass: u32, q: &Quality, rough: f32, rot: (f32, f32)) -> V3 {
        if !q.full && path_w() < W_MIN {
            return self.sky(d, rough);
        }
        count(3);
        match self.trace(o, d, 1e6) {
            Some(s) => {
                let c = if s.obj == FLOOR {
                    let (dif, r, f, _) = self.shade_floor(&s, d, depth, q, rot, true, false, false);
                    lerp3(add(r, dif.light()), self.backdrop(d), f)
                } else {
                    let (c, dif) = self.shade_obj(&s, d, depth, glass, q, rot, true);
                    add(c, dif.light())
                };
                self.hazed(c, s.t)
            }
            None => self.sky(d, rough),
        }
    }

    /// Floor: (diffuse split, reflection, fade to backdrop, reflected hit distance).
    fn shade_floor(&self, s: &Surf, d: V3, depth: u32, q: &Quality, rot: (f32, f32), inline_vis: bool, skip_refl: bool, skip_vis: bool) -> (Diffuse, V3, f32, f32) {
        let f = self.floor.as_ref().unwrap();
        let p = s.p;
        let up = v3(0.0, 1.0, 0.0);
        let (vk, ao) = match &f.vis {
            Some(v) => {
                let a = v.sample(p.x, p.z);
                (a[0], a[1])
            }
            None if skip_vis => (1.0, 1.0),
            None => {
                let (nk, na) = if inline_vis { (q.secondary_rays, q.secondary_rays) } else { (q.shadow_rays, q.ao_rays) };
                self.visibility(p, up, nk, na, rot, 1.2)
            }
        };
        let dx = p.x - f.pool.0;
        let dz = p.z - f.pool.1;
        let r2d = dx * dx + dz * dz;
        let pool = if f.pool.2 > 0.0 { (-r2d / (f.pool.2 * f.pool.2)).exp() } else { 1.0 };
        let (ll, _) = if self.lights.is_empty() { (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)) } else { self.local_lights(p, up, scale(d, -1.0), 1.0, v3(0.0, 0.0, 0.0)) };
        let dif = Diffuse { alb: f.color, amb: add(self.env.diffuse(up), ll), key: scale(self.key_diffuse(up), pool), ao, vk: vk * ao.sqrt(), spec: v3(0.0, 0.0, 0.0) };
        let fade = if f.fade.1 > f.fade.0 { smoothstep(f.fade.0, f.fade.1, r2d.sqrt()) } else { 0.0 };
        // Reflection: Fresnel lacquer plus an optional mirror boost.
        let nv = (-d.y).clamp(1e-4, 1.0);
        let fr = 0.04 + 0.96 * (1.0 - nv).powi(5);
        // (The fade to backdrop is applied by the caller, after any blur.)
        let k = (fr + f.mirror * (1.0 - fr)) * f.reflect;
        let mut refl = v3(0.0, 0.0, 0.0);
        let mut dist = 0.0;
        if k > 1e-3 {
            let r = v3(d.x, -d.y, d.z);
            let miss = |r: V3| self.backdrop(r);
            // A faint reflection (matte floor seen from above) isn't worth a ray.
            if skip_refl {
                // Traced by the neighbours (checkerboard): post fills it in.
                dist = 30.0;
            } else if depth < q.max_depth && k > 0.025 && (q.full || path_w() * k >= W_MIN) {
                count(4);
                match self.trace(madd(p, up, 1e-3), r, 1e6) {
                    Some(h) if h.obj != FLOOR => {
                        dist = h.t;
                        let (c, dd) = weighted(k, || self.shade_obj(&h, r, depth + 1, 0, q, rot, true));
                        refl = self.hazed(add(c, dd.light()), h.t);
                    }
                    _ => {
                        dist = 1e3;
                        refl = miss(r);
                    }
                }
            } else {
                refl = miss(r);
                dist = 1e3;
            }
            refl = scale(refl, k);
        }
        // What the floor reflects, it doesn't scatter (so at grazing angles a
        // white floor turns into exactly the backdrop: a seamless cyc).
        let mut dif = dif;
        dif.alb = scale(dif.alb, 1.0 - k.min(1.0));
        (dif, refl, fade, dist)
    }

    #[inline]
    fn print(&self, ob: &Inst, s: &Surf) -> Option<[f32; 4]> {
        let front = s.on.z > 0.5;
        let back = s.on.z < -0.5;
        ob.tex
            .iter()
            .find(|(_, face)| match face {
                Face::Front => front,
                Face::Back => back,
                Face::Both => front || back,
            })
            .map(|(t, _)| t.sample(s.uv[0], s.uv[1]))
    }

    /// Radiance of a texture print: lit like paper plus its own glow.
    #[inline]
    fn print_light(&self, t: [f32; 4], n: V3, glow: f32) -> V3 {
        let c = v3(t[0], t[1], t[2]);
        let lit = add(self.env.diffuse(n), self.key_diffuse(n));
        add(scale(c, glow * 1.6), scale(mul(c, lit), 1.0 - glow))
    }

    /// An object surface: (non-diffuse light, diffuse split). With
    /// `inline_vis`, visibility is traced here with few rays (secondary hits);
    /// otherwise the caller fills `ao`/`vk`.
    fn shade_obj(&self, s: &Surf, d: V3, depth: u32, glass: u32, q: &Quality, rot: (f32, f32), inline_vis: bool) -> (V3, Diffuse) {
        let ob = &self.insts[s.obj];
        let m = &self.mats[ob.mat as usize];
        let entering = dot(s.ng, d) < 0.0;
        let (mut ng, mut n) = (s.ng, s.ns);
        if !entering {
            ng = scale(ng, -1.0);
        }
        if dot(n, ng) < 0.0 {
            n = scale(n, -1.0);
        }
        let v = scale(d, -1.0);
        let mut nv = dot(n, v);
        if nv < 0.02 {
            n = norm(madd(n, v, 0.02 - nv));
            nv = dot(n, v).max(1e-4);
        }
        let mut out = v3(0.0, 0.0, 0.0);
        let mut cover = 0.0;
        if let Some(t) = self.print(ob, s) {
            out = self.print_light(t, n, m.tex_glow);
            cover = t[3];
            if cover >= 0.999 {
                return (add(out, m.emissive), Diffuse::default());
            }
        }
        let keep = 1.0 - cover;

        if m.transmission > 0.0 {
            // Dielectric interface.
            let eta = if entering { 1.0 / m.ior } else { m.ior };
            let fr = fresnel_dielectric(nv.min(1.0), if ob.thin { 1.0 / m.ior } else { eta });
            let rough = m.roughness.clamp(0.0, 1.0);
            let r = reflect(d, n);
            // Only the first glass interface along a path traces its reflection:
            // otherwise reflection × transmission branching grows as 2^layers.
            let refl_l = if depth < q.max_depth.min(2) && entering && glass == 0 { weighted(fr * keep, || self.radiance(madd(s.p, ng, 1e-4), r, depth + 1, glass + 1, q, rough, rot)) } else { self.sky(r, rough.max(0.05)) };
            let mut c = scale(refl_l, fr);
            if glass < q.max_glass {
                let dir = if ob.thin { Some(d) } else { refract(d, n, eta) };
                match dir {
                    Some(t) => {
                        let tl = weighted((1.0 - fr) * m.transmission.clamp(0.0, 1.0) * keep, || self.radiance(madd(s.p, ng, -1e-4), t, depth, glass + 1, q, rough, rot));
                        c = madd(c, tl, (1.0 - fr) * m.transmission.clamp(0.0, 1.0));
                    }
                    None => c = madd(c, refl_l, 1.0 - fr),
                }
            }
            let opaque = (1.0 - m.transmission).clamp(0.0, 1.0);
            if opaque > 0.0 && entering {
                let lit = add(self.env.diffuse(n), self.key_diffuse(n));
                c = madd(c, mul(m.base, lit), opaque * (1.0 - fr));
            }
            // Beer–Lambert over the path inside (exiting: s.t was inside).
            if !entering {
                c = mul(c, absorb(m.absorb, s.t));
            }
            return (madd(add(out, m.emissive), c, keep), Diffuse::default());
        }

        let rough = m.roughness.clamp(0.02, 1.0);
        let f0 = lerp3(v3(0.04 * m.specular, 0.04 * m.specular, 0.04 * m.specular), m.base, m.metallic);
        let ab = env_brdf(rough, nv);
        let spec_w = madd(scale(f0, ab.0), v3(1.0, 1.0, 1.0), ab.1);
        let diff_col = scale(m.base, 1.0 - m.metallic);
        let needs_diffuse = luma(diff_col) > 0.005;
        let rotp = (rot.1 * 0.37 + fract(s.p.x * 13.7), rot.0 * 0.61 + fract(s.p.z * 7.3));
        // Secondary hits (seen in reflections or through glass) aren't denoised:
        // a few rays would sparkle, so standard quality skips them.
        let (vk, ao) = if inline_vis && q.secondary_rays > 0 && (needs_diffuse || rough > 0.3) { self.visibility(s.p, ng, q.secondary_rays, q.secondary_rays, rotp, 0.8) } else { (1.0, 1.0) };
        // Specular: trace the reflection; the pre-filtered environment takes
        // over for rough surfaces. On primary hits the environment part is
        // occluded later by the denoised AO (so rough metal stays clean).
        let mut r = reflect(d, n);
        let rg = dot(r, ng);
        if rg < 0.01 {
            r = norm(madd(r, ng, 0.01 - rg));
        }
        let rough_mix = smoothstep(0.18, 0.5, rough);
        // (Weighted by `rough_mix`: smooth metal never needs the lookup.)
        let env_raw = if rough_mix > 0.0 { self.sky(r, rough) } else { v3(0.0, 0.0, 0.0) };
        let traced = if depth < q.max_depth && rough_mix < 1.0 { weighted(luma(spec_w) * (1.0 - rough_mix) * keep, || self.radiance(madd(s.p, ng, 1e-4), r, depth + 1, glass, q, rough, rot)) } else { v3(0.0, 0.0, 0.0) };
        let (traced, env_spec) = if inline_vis {
            (lerp3(traced, scale(env_raw, ao.powf(0.5 + rough)), rough_mix), v3(0.0, 0.0, 0.0))
        } else {
            (scale(traced, 1.0 - rough_mix), scale(mul(env_raw, spec_w), rough_mix * keep))
        };
        let mut c = mul(traced, spec_w);
        let (ll, lspec) = if self.lights.is_empty() { (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)) } else { self.local_lights(s.p, n, v, rough, f0) };
        c = add(c, scale(lspec, keep));
        let kd = 1.0 - luma(spec_w).min(1.0);
        let mut dif = Diffuse::default();
        if needs_diffuse {
            dif = Diffuse { alb: scale(diff_col, kd * keep), amb: add(self.env.diffuse(n), ll), key: self.key_diffuse(n), ao, vk, spec: v3(0.0, 0.0, 0.0) };
        }
        dif.spec = env_spec;
        if !inline_vis && luma(env_spec) > 0.0 {
            dif.ao = ao;
        }
        if m.clearcoat > 0.0 {
            let fc = (0.04 + 0.96 * (1.0 - nv).powi(5)) * m.clearcoat;
            c = madd(scale(c, 1.0 - fc), self.sky(r, 0.03), fc);
            dif.alb = scale(dif.alb, 1.0 - fc);
            dif.spec = scale(dif.spec, 1.0 - fc);
        }
        (madd(add(out, m.emissive), c, keep), dif)
    }

    #[inline]
    pub fn cam_ray(cam: &Camera, w: usize, h: usize, sx: f32, sy: f32) -> V3 {
        Rig::new(cam, w, h).ray(sx, sy)
    }

    fn primary(&self, rig: &Rig, prev: &Rig, sx: f32, sy: f32, q: &Quality, rot: (f32, f32), skip: u8) -> Sample {
        // Checkerboard bits: 1 = skip the floor reflection ray, 2 = skip shadow/AO
        // rays (the denoiser fills both from the traced neighbours).
        let skip_refl = skip & 1 != 0;
        let skip_vis = skip & 2 != 0;
        count(2);
        let d = rig.ray(sx, sy);
        let (f, th, h) = (rig.f, rig.th, rig.h);
        let cam = rig;
        let project_prev = |p: V3| prev.project(p);
        // Walk through thin glass: each pane adds its reflection (and print)
        // to `col` and attenuates what's behind it, which stays "primary".
        let mut o = cam.pos;
        let mut thr = v3(1.0, 1.0, 1.0);
        let mut col = v3(0.0, 0.0, 0.0);
        let mut travelled = 0.0;
        // (view depth, normal, id, motion) of the first surface.
        let mut first: Option<(f32, V3, usize, (f32, f32))> = None;
        let mut inside_since: Option<f32> = None;
        let none = v3(0.0, 0.0, 0.0);
        for layer in 0..=q.max_glass {
            let Some(s) = self.trace_f(o, d, 1e6, true) else {
                // Fog covers the backdrop too (as if it were 200 m away).
                let bd = if self.haze.0 > 0.0 { lerp3(self.backdrop(d), self.haze.1, 1.0 - (-self.haze.0 * 200.0).exp()) } else { self.backdrop(d) };
                let b = mul(bd, thr);
                let (depth, nrm, id, vel) = first.unwrap_or((1e4, none, usize::MAX - 1, (0.0, 0.0)));
                return Sample { col: add(col, b), dif: Diffuse::default(), refl: none, floor: 0, rad: 0.0, rf: 1.0, rw: 0.0, vw: 1.0, depth, vel, nrm, _pad: 0, id };
            };
            let dist = travelled + s.t;
            let haze_f = if self.haze.0 > 0.0 { (-self.haze.0 * dist).exp() } else { 1.0 };
            let depth_v = dot(sub(s.p, cam.pos), f);
            let prev_p = if s.obj == FLOOR {
                s.p
            } else {
                let ob = &self.insts[s.obj];
                ob.prev.point(ob.xf.point_inv(s.p))
            };
            let (ppx, ppy) = project_prev(prev_p);
            let here = (depth_v, s.ns, s.obj, (sx - ppx, sy - ppy));
            let (fd, fnrm, fid, fvel) = first.unwrap_or(here);
            if s.obj != FLOOR {
                let ob = &self.insts[s.obj];
                let m = &self.mats[ob.mat as usize];
                if ob.thin && m.transmission > 0.0 && layer < q.max_glass {
                    // A thin pane's interface.
                    let entering = dot(s.ng, d) < 0.0;
                    if first.is_none() {
                        first = Some(here);
                    }
                    let ngf = if entering { s.ng } else { scale(s.ng, -1.0) };
                    let mut n = s.ns;
                    if dot(n, ngf) < 0.0 {
                        n = scale(n, -1.0);
                    }
                    let nv = dot(n, scale(d, -1.0)).clamp(1e-4, 1.0);
                    // A thin parallel slab: both faces reflect like the entry
                    // face (the ray isn't bent inside, so evaluating the exit
                    // with the outside angle would wrongly hit total internal
                    // reflection).
                    let fr = fresnel_dielectric(nv, 1.0 / m.ior);
                    let mut a = 0.0;
                    if let Some(t) = self.print(ob, &s) {
                        col = add(col, scale(mul(self.print_light(t, n, m.tex_glow), thr), haze_f));
                        a = t[3];
                    }
                    let r = reflect(d, n);
                    let rl = if entering && layer < 2 && fr * (1.0 - a) * luma(thr) > 0.004 { weighted(fr * (1.0 - a) * luma(thr) * haze_f, || self.radiance(madd(s.p, ngf, 1e-4), r, 1, layer + 1, q, m.roughness, rot)) } else { self.sky(r, m.roughness.max(0.05)) };
                    col = add(col, scale(mul(rl, thr), fr * (1.0 - a) * haze_f));
                    // Partly transmissive glass: the rest is a lit surface of its base colour.
                    let opaque = (1.0 - m.transmission).clamp(0.0, 1.0);
                    if opaque > 0.0 && entering {
                        let lit = add(self.env.diffuse(n), self.key_diffuse(n));
                        col = add(col, scale(mul(mul(m.base, lit), thr), opaque * (1.0 - fr) * (1.0 - a) * haze_f));
                    }
                    let mut tthr = scale(thr, (1.0 - fr) * (1.0 - a) * m.transmission.clamp(0.0, 1.0));
                    if entering {
                        inside_since = Some(dist);
                    } else {
                        let inside = inside_since.map(|t0| dist - t0).unwrap_or(s.t);
                        tthr = mul(tthr, absorb(m.absorb, inside));
                        inside_since = None;
                    }
                    thr = tthr;
                    if luma(thr) < 0.004 {
                        return Sample { col, dif: Diffuse::default(), refl: none, floor: 0, rad: 0.0, rf: 1.0, rw: 0.0, vw: 1.0, depth: fd, vel: fvel, nrm: fnrm, _pad: 0, id: fid };
                    }
                    o = madd(s.p, d, 1e-4);
                    travelled = dist;
                    continue;
                }
            }
            // An opaque (or thick glass) surface: the primary hit proper.
            let haze_add = scale(self.haze.1, 1.0 - haze_f);
            if s.obj == FLOOR {
                let (mut dif, refl, fade, rdist) = weighted(luma(thr) * haze_f, || self.shade_floor(&s, d, 0, q, rot, false, skip_refl, skip_vis));
                let bd = self.backdrop(d);
                let keepf = (1.0 - fade) * haze_f;
                dif.alb = mul(dif.alb, scale(thr, keepf));
                let col = add(col, mul(thr, add(scale(bd, fade * haze_f), haze_add)));
                let fl = self.floor.as_ref().unwrap();
                let focal = h * 0.5 / th;
                let rad = (fl.roughness * rdist.min(50.0) / depth_v.max(1e-3) * focal * 0.5).min(48.0);
                let direct = first.is_none();
                // The reflection is blurred in post before its fade/haze factor
                // applies (so far, hazed floor can't gather near reflections).
                // Seen through glass, it isn't blurred separately.
                let (refl, col) = if direct { (mul(refl, thr), col) } else { (none, add(col, mul(scale(refl, keepf), thr))) };
                return Sample { col, dif, refl, floor: direct as u32, rad, rf: keepf, rw: if skip_refl { 0.0 } else { 1.0 }, vw: if skip_vis && self.floor.as_ref().is_some_and(|f| f.vis.is_none()) { 0.0 } else { 1.0 }, depth: fd, vel: fvel, nrm: fnrm, _pad: 0, id: fid };
            }
            let (c, mut dif) = weighted(luma(thr) * haze_f, || self.shade_obj(&s, d, 0, layer, q, rot, false));
            if (luma(dif.alb) > 0.0 || luma(dif.spec) > 0.0) && !skip_vis {
                let ng = if dot(s.ng, d) < 0.0 { s.ng } else { scale(s.ng, -1.0) };
                let nk = if luma(dif.alb) > 0.0 { q.shadow_rays } else { 0 };
                let (vk, ao) = self.visibility(s.p, ng, nk, q.ao_rays, rot, 0.8);
                dif.vk = vk;
                dif.ao = ao;
            }
            dif.alb = mul(dif.alb, scale(thr, haze_f));
            dif.spec = mul(dif.spec, scale(thr, haze_f));
            let col = add(col, mul(thr, add(scale(c, haze_f), haze_add)));
            return Sample { col, dif, refl: none, floor: 0, rad: 0.0, rf: 1.0, rw: 0.0, vw: if skip_vis { 0.0 } else { 1.0 }, depth: fd, vel: fvel, nrm: fnrm, _pad: 0, id: fid };
        }
        let (depth, nrm, id, vel) = first.unwrap_or((1e4, none, usize::MAX - 1, (0.0, 0.0)));
        Sample { col, dif: Diffuse::default(), refl: none, floor: 0, rad: 0.0, rf: 1.0, rw: 0.0, vw: 1.0, depth, vel, nrm, _pad: 0, id }
    }

    /// Tone-mapped perceptual luminance for the refinement test. Diffuse light
    /// is taken unshadowed: AO and key visibility come from one noisy ray each
    /// and are denoised in post, so their noise must not trigger supersampling
    /// (it covered whole matte surfaces); geometric and shading edges still do.
    #[inline]
    fn contrast_luma(s: &Sample) -> f32 {
        let d = &s.dif;
        let lit = add(mul(d.alb, add(d.amb, d.key)), d.spec);
        let l = luma(add(add(s.col, s.refl), lit)).max(0.0);
        (l / (1.0 + l)).powf(1.0 / 2.2)
    }

    /// The primary pass over the whole frame, in two row-parallel phases: one
    /// centre sample per pixel, then `edge_spp` more where a pixel disagrees
    /// with a neighbour, resolved into `out` (`w × h`). Fine-grained rows keep
    /// the cores evenly loaded, and no centre sample is computed twice.
    pub fn render_frame(&self, cam: &Camera, prev: &Camera, w: usize, h: usize, q: &Quality, out: &mut [Px]) {
        use super::kernel::{par, pool::Buf};
        let (rig, prev) = (&Rig::new(cam, w, h), &Rig::new(prev, w, h));
        let clock = std::time::Instant::now();
        let mut centre: Buf<Sample> = Buf::uninit(w * h);
        let mut tms: Buf<f32> = Buf::uninit(w * h);
        let tp = par::Shared(tms.as_mut_ptr());
        let rough_floor = self.floor.as_ref().is_some_and(|f| f.roughness >= 0.2);
        par::rows(&mut centre, w, |y, row| {
            for x in 0..w {
                let hs = hash2(x as u32, y as u32);
                let rot = (u01(hs), u01(hash2(hs, 0x9e37)));
                // Rough floors are blurred anyway: trace their reflections on a checkerboard.
                let odd = !q.full && (x + y) & 1 == 1;
                let skip = if odd { (rough_floor as u8) | 2 } else { 0 };
                let s = self.primary(rig, prev, x as f32 + 0.5, y as f32 + 0.5, q, rot, skip);
                // SAFETY: pixel (x, y) belongs to this row's task only.
                unsafe { *tp.at(y * w + x) = Self::contrast_luma(&s) };
                row[x] = s;
            }
        });
        let t_centre = clock.elapsed();
        let refined = std::sync::atomic::AtomicU64::new(0);
        let (centre, tms) = (&centre, &tms);
        par::rows(out, w, |y, row| {
            let mut samples: Vec<Sample> = Vec::with_capacity(q.edge_spp as usize + 1);
            let mut nref = 0u64;
            for x in 0..w {
                let ci = y * w + x;
                let c = &centre[ci];
                let mut refine = q.full;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                        continue;
                    }
                    let ni = ny as usize * w + nx as usize;
                    if centre[ni].id != c.id || (tms[ni] - tms[ci]).abs() > q.contrast {
                        refine = true;
                        break;
                    }
                }
                samples.clear();
                samples.push(*c);
                if refine {
                    nref += 1;
                    // Two stages: half the samples, then the rest only where they
                    // still disagree (object edges always get all of them).
                    let hs = hash2(x as u32, y as u32);
                    let rot = (u01(hs), u01(hash2(hs, 0x5bd1)));
                    let half = (q.edge_spp / 2).max(1);
                    let mut i = 0;
                    while i < q.edge_spp {
                        let (a, b) = r2(i, rot);
                        let srot = (u01(hash2(hs, i * 2 + 1)), u01(hash2(hs, i * 2 + 2)));
                        samples.push(self.primary(rig, prev, x as f32 + a, y as f32 + b, q, srot, 0));
                        i += 1;
                        if i == half {
                            let id0 = samples[0].id;
                            let mixed = samples.iter().any(|s| s.id != id0);
                            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                            for s in &samples {
                                let l = luma(s.total()).max(0.0);
                                let v = (l / (1.0 + l)).powf(1.0 / 2.2);
                                lo = lo.min(v);
                                hi = hi.max(v);
                            }
                            if !mixed && hi - lo < q.contrast * 1.5 {
                                break;
                            }
                        }
                    }
                }
                row[x] = resolve(&samples);
            }
            refined.fetch_add(nref, std::sync::atomic::Ordering::Relaxed);
        });
        if PROFILE.load(std::sync::atomic::Ordering::Relaxed) {
            use std::sync::atomic::Ordering::Relaxed;
            TIMES[0].fetch_add(t_centre.as_micros() as u64, Relaxed);
            TIMES[1].fetch_add((clock.elapsed() - t_centre).as_micros() as u64, Relaxed);
            TIMES[2].fetch_add(refined.load(Relaxed), Relaxed);
        }
    }

    /// Micro-benchmarks of the building blocks over every pixel (profiling aid).
    pub fn micro(&self, cam: &Camera, w: usize, h: usize) {
        use rayon::prelude::*;
        let time = |name: &str, f: &(dyn Fn(usize, usize) -> f32 + Sync)| {
            let c = std::time::Instant::now();
            let s: f32 = (0..h).into_par_iter().map(|y| (0..w).map(|x| f(x, y)).sum::<f32>()).sum();
            let ms = c.elapsed().as_secs_f64() * 1e3;
            eprintln!("  micro {name}: {ms:.1} ms ({:.0} ns/px cpu) [{s:.1}]", ms * 1e6 * rayon::current_num_threads() as f64 / (w * h) as f64);
        };
        time("cam_ray", &|x, y| Self::cam_ray(cam, w, h, x as f32 + 0.5, y as f32 + 0.5).x);
        time("trace primary", &|x, y| {
            let d = Self::cam_ray(cam, w, h, x as f32 + 0.5, y as f32 + 0.5);
            self.trace(cam.pos, d, 1e6).map(|s| s.t).unwrap_or(0.0)
        });
        time("tlas only", &|x, y| {
            let d = Self::cam_ray(cam, w, h, x as f32 + 0.5, y as f32 + 0.5);
            let mut n = 0.0;
            self.tlas.visit(cam.pos, d, EPS, 1e6, |_, tm| {
                n += 1.0;
                tm
            });
            n
        });
        let q = Quality::default();
        time("primary sample", &|x, y| {
            let hs = hash2(x as u32, y as u32);
            let rot = (u01(hs), u01(hash2(hs, 0x9e37)));
            let skip = (x + y) & 1 == 1 && self.floor.as_ref().is_some_and(|f| f.roughness >= 0.2);
            let rig = Rig::new(cam, w, h);
            self.primary(&rig, &rig, x as f32 + 0.5, y as f32 + 0.5, &q, rot, if skip { 1 } else { 0 }).col.x
        });
        time("env radiance", &|x, y| {
            let d = Self::cam_ray(cam, w, h, x as f32 + 0.5, y as f32 + 0.5);
            self.env.radiance(d, 0.1).x
        });
        time("backdrop", &|x, y| {
            let d = Self::cam_ray(cam, w, h, x as f32 + 0.5, y as f32 + 0.5);
            self.backdrop(d).x
        });
        time("env diffuse", &|x, y| {
            let d = Self::cam_ray(cam, w, h, x as f32 + 0.5, y as f32 + 0.5);
            self.env.diffuse(d).x
        });
    }

    /// Bake key visibility and AO over the floor around the objects (for
    /// static layouts: noise-free contact shadows).
    pub fn bake_floor(&mut self, res: usize, rays: u32) {
        let Some(f) = &self.floor else { return };
        if self.insts.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for o in &self.insts {
            let (a, b) = self.world_box(o);
            lo = vmin(lo, a);
            hi = vmax(hi, b);
        }
        let m = (hi.y - f.y).max(0.5) * 1.6;
        let (x0, x1, z0, z1) = (lo.x - m, hi.x + m, lo.z - m, hi.z + m);
        let aspect = (x1 - x0) / (z1 - z0);
        let (nx, nz) = if aspect >= 1.0 { (res, ((res as f32 / aspect) as usize).max(8)) } else { (((res as f32 * aspect) as usize).max(8), res) };
        let y = f.y;
        let me: &Scene = self;
        let up = v3(0.0, 1.0, 0.0);
        let row = |k: usize| -> Vec<[f32; 2]> {
            (0..nx)
                .map(|i| {
                    let p = v3(x0 + (i as f32 + 0.5) / nx as f32 * (x1 - x0), y, z0 + (k as f32 + 0.5) / nz as f32 * (z1 - z0));
                    let hs = hash2(i as u32 + 7, k as u32 + 13);
                    let (a, b) = me.visibility(p, up, rays, rays, (u01(hs), u01(hash2(hs, 5))), (hi.y - y).max(0.3) * 1.2);
                    [a, b]
                })
                .collect()
        };
        use rayon::prelude::*;
        let rows: Vec<Vec<[f32; 2]>> = if rayon::current_thread_index().is_some() { (0..nz).map(row).collect() } else { (0..nz).into_par_iter().map(row).collect() };
        let raw: Vec<[f32; 2]> = rows.into_iter().flatten().collect();
        let mut data = raw.clone();
        for k in 0..nz {
            for i in 0..nx {
                let mut s = [0.0f32; 2];
                let mut wsum = 0.0;
                for dk in -2i32..=2 {
                    for di in -2i32..=2 {
                        let (a, b) = (i as i32 + di, k as i32 + dk);
                        if a < 0 || b < 0 || a >= nx as i32 || b >= nz as i32 {
                            continue;
                        }
                        let wt = (-((di * di + dk * dk) as f32) / 2.5).exp();
                        let v = raw[b as usize * nx + a as usize];
                        s[0] += v[0] * wt;
                        s[1] += v[1] * wt;
                        wsum += wt;
                    }
                }
                data[k * nx + i] = [s[0] / wsum, s[1] / wsum];
            }
        }
        let vis = FloorVis { x0, z0, x1, z1, nx, nz, data };
        self.floor.as_mut().unwrap().vis = Some(Arc::new(vis));
    }
}

/// Combine a pixel's samples with tone-mapped weights (bright highlights
/// don't alias). AO and key visibility average weighted by albedo, so the
/// denoiser can filter them as scalars.
fn resolve(samples: &[Sample]) -> Px {
    let mut px = Px::default();
    let mut wsum = 0.0;
    let mut best_d = f32::MAX;
    let mut fl_w = 0.0;
    let mut vis_w = 0.0;
    let mut amb = v3(0.0, 0.0, 0.0);
    let mut key = v3(0.0, 0.0, 0.0);
    let mut alb_l = 0.0;
    for s in samples {
        let wt = 1.0 / (1.0 + luma(s.total()));
        wsum += wt;
        let d = &s.dif;
        for k in 0..3 {
            px.col[k] += get(s.col, k) * wt;
            px.refl[k] += get(s.refl, k) * wt;
            px.alb[k] += get(d.alb, k) * wt;
            px.spec[k] += get(d.spec, k) * wt;
        }
        let la = (luma(d.alb) + luma(d.spec)) * wt;
        amb = madd(amb, d.amb, la);
        key = madd(key, d.key, la);
        alb_l += la;
        px.ao += d.ao * la * s.vw;
        px.vk += d.vk * la * s.vw;
        vis_w += la * s.vw;
        px.vw += la;
        if s.floor != 0 {
            px.cov += wt;
            px.rad += s.rad * wt;
            px.rf += s.rf * wt;
            px.rcov += s.rw * wt;
            fl_w += wt;
        }
        if s.depth < best_d {
            best_d = s.depth;
            px.vel = [s.vel.0, s.vel.1];
            px.nrm = [s.nrm.x, s.nrm.y, s.nrm.z];
            px.id = s.id as u32;
        }
    }
    for k in 0..3 {
        px.col[k] /= wsum;
        px.refl[k] /= wsum;
        px.alb[k] /= wsum;
        px.spec[k] /= wsum;
    }
    if alb_l > 0.0 {
        px.amb = [amb.x / alb_l, amb.y / alb_l, amb.z / alb_l];
        px.key = [key.x / alb_l, key.y / alb_l, key.z / alb_l];
    }
    if vis_w > 0.0 {
        px.ao /= vis_w;
        px.vk /= vis_w;
        px.vw = (vis_w / px.vw.max(1e-9)).min(1.0);
    } else {
        px.ao = 1.0;
        px.vk = 1.0;
        px.vw = if px.vw > 0.0 { 0.0 } else { 1.0 };
    }
    if fl_w > 0.0 {
        px.rad /= fl_w;
        px.rf /= fl_w;
    }
    px.cov /= wsum;
    px.rcov /= wsum;
    px.depth = best_d;
    px
}

#[inline]
fn absorb(sigma: V3, dist: f32) -> V3 {
    v3((-sigma.x * dist).exp(), (-sigma.y * dist).exp(), (-sigma.z * dist).exp())
}

/// Split-sum environment BRDF (scale, bias) for F0 (Karis' analytic fit).
#[inline]
fn env_brdf(rough: f32, nv: f32) -> (f32, f32) {
    let c0 = [-1.0, -0.0275, -0.572, 0.022];
    let c1 = [1.0, 0.0425, 1.04, -0.04];
    let r = [rough * c0[0] + c1[0], rough * c0[1] + c1[1], rough * c0[2] + c1[2], rough * c0[3] + c1[3]];
    let a004 = (r[0] * r[0]).min((-9.28 * nv).exp2()) * r[0] + r[1];
    (a004 * -1.04 + r[2], a004 * 1.04 + r[3])
}
