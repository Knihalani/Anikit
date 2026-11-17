//! `scene3d`: ray-traced, physically based 3D for product shots.
//!
//! A software renderer built for quality rather than speed: chrome and glossy
//! materials reflect a procedural HDR studio (softboxes, strips, horizon) and
//! each other, the key softbox casts soft traced shadows, the floor is a
//! glossy mirror with distance-blurred reflections, and the image goes
//! through depth of field, motion blur, bloom and an ACES tone map.
//!
//! Modules: [`math`] (vectors, transforms, sampling), [`bvh`] (triangle
//! hierarchy), [`env`] (studio environments), [`trace`] (shading and the
//! primary pass), [`post`] (the post stack). This file is the component.

pub mod bvh;
pub mod env;
pub mod math;
pub mod post;
pub mod trace;

use crate::components::style::fnv;
use crate::components::text3d::{clean, extrude_segs, glyph_outlines};
use crate::components::{Component, Ctx, Error, Prepare};
use crate::dl::Op;
use bvh::Bvh;
use env::Env;
use math::*;
use serde::Deserialize;
use std::sync::Arc;
use trace::{Camera, Floor, Material, Object, Quality, Scene};

/// A camera keyframe.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CamKey {
    pub at: f32,
    pub pos: [f32; 3],
    pub target: [f32; 3],
    #[serde(default = "d_fov")]
    pub fov: f32,
    /// Focus distance; `null` focuses on the target.
    #[serde(default)]
    pub focus: Option<f32>,
}
fn d_fov() -> f32 {
    30.0
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scene3d {
    pub text: String,
    pub font: String,
    pub weight: f32,
    /// Environment preset: `studio`, `night`, `neon`, `sunset`.
    pub env: String,
    /// Wordmark material: `chrome`, `gold`, `black`, `glass` (approximate).
    pub material: String,
    pub primitives: bool,
    pub camera: Vec<CamKey>,
    /// Blur radius (px at 1080p) for points at infinity.
    pub aperture: f32,
    pub shutter: f32,
    pub bloom: f32,
    pub exposure: f32,
    /// `draft`, `standard` or `high`.
    pub quality: String,
    #[serde(skip)]
    scene: Option<Arc<Scene>>,
    #[serde(skip)]
    key: u64,
}

impl Default for Scene3d {
    fn default() -> Self {
        Scene3d {
            text: "ANIKIT".into(),
            font: "Inter".into(),
            weight: 800.0,
            env: "studio".into(),
            material: "chrome".into(),
            primitives: true,
            camera: Vec::new(),
            aperture: 26.0,
            shutter: 0.5,
            bloom: 0.06,
            exposure: 1.0,
            quality: "standard".into(),
            scene: None,
            key: 0,
        }
    }
}

fn srgb_lin(c: [f32; 3]) -> V3 {
    let f = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    v3(f(c[0]), f(c[1]), f(c[2]))
}

/// Default lookdev camera: a slow arc and dolly, focus racking from the
/// foreground sphere to the wordmark.
fn default_camera() -> Vec<CamKey> {
    vec![
        CamKey { at: 0.0, pos: [-4.9, 1.35, 9.4], target: [-1.3, 0.42, 1.2], fov: 28.0, focus: Some(7.9) },
        CamKey { at: 1.6, pos: [-4.1, 1.28, 9.2], target: [-1.0, 0.45, 0.9], fov: 28.0, focus: Some(7.9) },
        CamKey { at: 3.8, pos: [-2.2, 1.15, 9.0], target: [-0.1, 0.5, 0.0], fov: 27.5, focus: None },
        CamKey { at: 9.0, pos: [3.0, 0.98, 8.2], target: [0.3, 0.52, 0.0], fov: 26.5, focus: None },
    ]
}

impl Prepare for Scene3d {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.camera.is_empty() {
            self.camera = default_camera();
        }
        self.camera.sort_by(|a, b| a.at.total_cmp(&b.at));
        let accent = srgb_lin([0.486, 0.361, 1.0]);
        let accent2 = srgb_lin([0.133, 0.827, 0.933]);
        let env = Env::preset(&self.env, accent, accent2).ok_or_else(|| Error(format!("unknown env `{}`; expected studio, night, neon or sunset", self.env)))?;
        let mat = match self.material.as_str() {
            "chrome" => Material::chrome(),
            "gold" => Material { base: v3(1.0, 0.71, 0.38), metallic: 1.0, roughness: 0.12, ..Material::chrome() },
            "black" => Material { base: v3(0.01, 0.01, 0.012), metallic: 0.0, roughness: 0.08, specular: 1.2, ..Material::chrome() },
            other => return Err(Error(format!("unknown material `{other}`; expected chrome, gold or black"))),
        };
        // Wordmark: letters extruded with a smooth 8-segment bevel, one unit tall.
        let letters = glyph_outlines(&self.text, &self.font, self.weight, 0.02);
        let shapes: Vec<Vec<Vec<[f32; 2]>>> = letters.into_iter().map(|cs| cs.into_iter().filter_map(clean).collect::<Vec<_>>()).filter(|c: &Vec<_>| !c.is_empty()).collect();
        if shapes.is_empty() {
            return Err(Error("nothing to extrude".into()));
        }
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in shapes.iter().flatten().flatten() {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let cap = (hi[1] - lo[1]).max(1e-3);
        let k = 1.0 / cap;
        let cx = (lo[0] + hi[0]) * 0.5;
        let mut pos = Vec::new();
        let mut nrm = Vec::new();
        let mut idx = Vec::new();
        for cs in &shapes {
            // Centred vertically so the cap normals' "pillow" tilt is symmetric:
            // the top of each face catches the front softbox, the bottom the
            // dark floor, a vertical chrome sweep instead of one flat grey.
            let cs: Vec<Vec<[f32; 2]>> = cs.iter().map(|c| c.iter().map(|p| [(p[0] - cx) * k, (p[1] - lo[1]) * k - 0.5]).collect()).collect();
            let mut m = extrude_segs(&cs, 0.17, 0.045, 0.55, 10);
            for p in m.pos.chunks_exact_mut(3) {
                p[1] += 0.5;
            }
            let base = (pos.len() / 3) as u32;
            pos.extend_from_slice(&m.pos);
            nrm.extend_from_slice(&m.nrm);
            idx.extend(m.idx.iter().map(|i| i + base));
        }
        let word = Arc::new(Bvh::new(&pos, &nrm, &idx));
        let mut objects = vec![Object { bvh: word, xf: Xf::new(v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0), 1.0), prev: Xf::IDENTITY, mat }];
        if self.primitives {
            let sphere = {
                let m = anikit_core::raster::sphere(96, 192);
                Arc::new(Bvh::new(&m.pos, &m.nrm, &m.idx))
            };
            let torus = {
                let m = anikit_core::raster::torus(1.0, 0.16, 192, 64);
                Arc::new(Bvh::new(&m.pos, &m.nrm, &m.idx))
            };
            // Glossy black ceramic sphere, back left.
            objects.push(Object {
                bvh: sphere.clone(),
                xf: Xf::new(v3(-3.3, 0.62, -1.9), v3(0.0, 0.0, 0.0), 0.62),
                prev: Xf::IDENTITY,
                mat: Material { base: v3(0.012, 0.012, 0.014), metallic: 0.0, roughness: 0.05, specular: 1.3, ..Material::chrome() },
            });
            // Brushed gold ring standing on its edge, back right.
            objects.push(Object {
                bvh: torus,
                xf: Xf::new(v3(3.5, 0.82, -2.3), v3(0.0, -32.0, 0.0), 0.7),
                prev: Xf::IDENTITY,
                mat: Material { base: v3(1.0, 0.74, 0.42), metallic: 1.0, roughness: 0.16, ..Material::chrome() },
            });
            // Small chrome sphere in the foreground (the opening focus).
            objects.push(Object { bvh: sphere.clone(), xf: Xf::new(v3(-1.45, 0.26, 2.1), v3(0.0, 0.0, 0.0), 0.26), prev: Xf::IDENTITY, mat: Material::chrome() });
            // Satin white sphere, front right.
            objects.push(Object {
                bvh: sphere,
                xf: Xf::new(v3(2.75, 0.34, 0.9), v3(0.0, 0.0, 0.0), 0.34),
                prev: Xf::IDENTITY,
                mat: Material { base: v3(0.8, 0.79, 0.77), metallic: 0.0, roughness: 0.38, ..Material::chrome() },
            });
        }
        for o in objects.iter_mut() {
            o.prev = o.xf;
        }
        let floor = Floor { y: 0.0, color: v3(0.035, 0.035, 0.04), roughness: 0.09, mirror: 0.06, pool: (0.0, 0.0, 5.0), fade: (7.0, 16.0), vis: None };
        let mut scene = Scene { objects, env, floor: Some(floor), backdrop: (v3(0.006, 0.006, 0.008), v3(0.0015, 0.0015, 0.002)), glow: (norm(v3(0.0, 0.12, -1.0)), v3(0.05, 0.045, 0.06), 0.09), exposure: self.exposure };
        let (res, rays) = match self.quality.as_str() {
            "draft" => (384, 24),
            "high" => (1024, 128),
            _ => (768, 64),
        };
        scene.bake_floor(res, rays);
        self.scene = Some(Arc::new(scene));
        let mut h = Vec::new();
        h.extend_from_slice(self.text.as_bytes());
        h.extend_from_slice(self.env.as_bytes());
        h.extend_from_slice(self.material.as_bytes());
        h.extend_from_slice(self.quality.as_bytes());
        for v in [self.aperture, self.shutter, self.bloom, self.exposure, self.weight] {
            h.extend_from_slice(&v.to_le_bytes());
        }
        for c in &self.camera {
            for v in [c.at, c.pos[0], c.pos[1], c.pos[2], c.target[0], c.target[1], c.target[2], c.fov, c.focus.unwrap_or(-1.0)] {
                h.extend_from_slice(&v.to_le_bytes());
            }
        }
        self.key = fnv(&[b"scene3d", &h]);
        Ok(())
    }
}

impl Scene3d {
    /// Camera at time `t`: Catmull-Rom through the keys, eased in and out
    /// over the whole move.
    fn camera_at(&self, t: f32) -> Camera {
        let ks = &self.camera;
        let (t0, t1) = (ks[0].at, ks[ks.len() - 1].at);
        // Ease the whole move (slow start, slow settle).
        let u = if t1 > t0 { ((t - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 0.0 };
        let eased = t0 + (t1 - t0) * (u * u * (3.0 - 2.0 * u) * 0.6 + u * 0.4);
        let t = eased;
        let mut i = 0;
        while i + 2 < ks.len() && ks[i + 1].at <= t {
            i += 1;
        }
        let a = &ks[i];
        let b = &ks[(i + 1).min(ks.len() - 1)];
        let s = if b.at > a.at { ((t - a.at) / (b.at - a.at)).clamp(0.0, 1.0) } else { 0.0 };
        let pm = &ks[i.saturating_sub(1)];
        let pn = &ks[(i + 2).min(ks.len() - 1)];
        let cr = |p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]| -> V3 {
            let f = |k: usize| {
                let (a, b, c, d) = (p0[k], p1[k], p2[k], p3[k]);
                0.5 * (2.0 * b + (-a + c) * s + (2.0 * a - 5.0 * b + 4.0 * c - d) * s * s + (-a + 3.0 * b - 3.0 * c + d) * s * s * s)
            };
            v3(f(0), f(1), f(2))
        };
        let pos = cr(pm.pos, a.pos, b.pos, pn.pos);
        let target = cr(pm.target, a.target, b.target, pn.target);
        let fov = a.fov + (b.fov - a.fov) * s;
        let dist_t = len(sub(target, pos));
        let fa = a.focus.unwrap_or(dist_t);
        let fb = b.focus.unwrap_or(dist_t);
        // Rack focus with a smooth ease (focus pulls feel mechanical when linear).
        let ss = s * s * (3.0 - 2.0 * s);
        let focus = fa + (fb - fa) * ss;
        Camera { pos, target, fov, focus, aperture: self.aperture, roll: 0.0 }
    }

    /// Render one frame at `w × h` (RGBA8).
    pub fn render(&self, t: f32, dt: f32, w: usize, h: usize) -> Vec<u8> {
        let scene = self.scene.as_ref().expect("prepared");
        let cam = self.camera_at(t);
        let prev = self.camera_at(t - dt);
        let q = match self.quality.as_str() {
            "draft" => Quality { min_spp: 1, max_spp: 4, ao_rays: 2, shadow_rays: 2, max_depth: 2 },
            "high" => Quality { min_spp: 6, max_spp: 32, ao_rays: 8, shadow_rays: 6, max_depth: 4 },
            _ => Quality::default(),
        };
        let clock = std::time::Instant::now();
        let mut px = vec![trace::Px::default(); w * h];
        let band = 8usize;
        let body = |(k, rows): (usize, &mut [trace::Px])| {
            let y0 = k * band;
            let y1 = (y0 + rows.len() / w).min(h);
            scene.render_rows(&cam, &prev, w, h, y0, y1, &q, rows);
        };
        use rayon::prelude::*;
        if rayon::current_thread_index().is_some() {
            px.chunks_mut(band * w).enumerate().for_each(body);
        } else {
            px.par_chunks_mut(band * w).enumerate().for_each(body);
        }
        let p = post::Post { focus: cam.focus, aperture: cam.aperture, shutter: self.shutter, bloom: self.bloom, exposure: scene.exposure, vignette: 0.22 };
        let traced = clock.elapsed();
        let out = post::process(&px, w, h, &p);
        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
            eprintln!("scene3d t={t:.3}: trace {:.0} ms, post {:.0} ms", traced.as_secs_f64() * 1e3, (clock.elapsed() - traced).as_secs_f64() * 1e3);
        }
        out
    }
}

impl Component for Scene3d {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width.round().max(2.0) as u32, ctx.height.round().max(2.0) as u32);
        let fps = ctx.fps.max(1.0);
        let frame = (t * fps).round();
        let t = frame / fps;
        let key = fnv(&[&self.key.to_le_bytes(), &frame.to_bits().to_le_bytes(), &fps.to_bits().to_le_bytes(), &w.to_le_bytes(), &h.to_le_bytes()]);
        let image = ctx.assets.image(key, w, h, &mut || self.render(t, 1.0 / fps, w as usize, h as usize));
        vec![Op::Image { image, dst: [0.0, 0.0, ctx.width, ctx.height], opacity: 1.0, transform: crate::dl::IDENTITY }]
    }
}
