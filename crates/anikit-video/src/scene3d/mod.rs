//! `scene3d`: ray-traced, physically based 3D for product films.
//!
//! A software renderer built for quality: chrome, aluminium and glass reflect
//! a procedural HDR studio (a bright white cyclorama by default, or dark
//! studio, night, neon and sunset presets) and each other; the key softbox
//! casts soft traced shadows; glass refracts with Fresnel and tinted edges;
//! any spec composition can be a live texture on a 3D surface, also across
//! thousands of instances at per-instance frame offsets; and the image goes
//! through denoising, depth of field, motion blur, bloom and tone mapping.
//!
//! Modules: [`math`] (vectors, transforms, sampling), [`bvh`] (triangle and
//! instance hierarchies), [`geom`] (meshes), [`env`] (environments),
//! [`spec`] (props → scene description), [`trace`] (shading and the primary
//! pass), [`post`] (the image pipeline). This file is the component.
//!
//! Spec (as component props; keys may be camelCase):
//! ```json
//! { "env": "cyc", "camera": "orbit",
//!   "objects": [ { "text": "ANIKIT", "material": "chrome" },
//!                { "type": "slab", "count": 40, "pos": "=[1.7*i, 0.47, 0]", "material": "glass",
//!                  "texture": { "composition": "film", "frame": "=i" } } ] }
//! ```

pub mod bvh;
pub mod env;
pub mod geom;
pub mod math;
pub mod post;
pub mod spec;
pub mod trace;

use crate::components::style::fnv;
use crate::components::{Component, Ctx, Error, Prepare};
use crate::dl::Op;
use crate::eval::Evaluator;
use crate::pipeline::textures::CompTextures;
use math::*;
use serde_json::{Map, Value};
use spec::{CamSpec, ObjDef};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use trace::{Floor, FloorVis, Inst, Material, Quality, Scene, Shape, Tex};

const TOP_KEYS: &[&str] = &[
    "text", "font", "weight", "env", "material", "primitives", "camera", "objects", "materials", "floor", "aperture", "shutter", "bloom", "exposure", "tonemap", "quality", "vignette", "denoise",
];

/// The `scene3d` component (kind names `scene3d`, `scene-3d`, `3d.scene`).
#[derive(serde::Deserialize)]
#[serde(try_from = "Value")]
pub struct Scene3d {
    raw: Map<String, Value>,
    prepared: Option<Prepared>,
}

impl TryFrom<Value> for Scene3d {
    type Error = String;
    fn try_from(v: Value) -> Result<Self, String> {
        match v {
            Value::Object(raw) => Ok(Scene3d { raw, prepared: None }),
            Value::Null => Ok(Scene3d { raw: Map::new(), prepared: None }),
            _ => Err("scene3d props must be an object".into()),
        }
    }
}

struct Prepared {
    shapes: Vec<Shape>,
    mats: Vec<Material>,
    objs: Vec<ObjDef>,
    env: env::Env,
    floor: Option<FloorParams>,
    floor_vis: Option<Arc<FloorVis>>,
    backdrop: (V3, V3),
    glow: (V3, V3, f32),
    haze: (f32, V3),
    camera: CamSpec,
    default_aperture: f32,
    shutter: Option<f32>,
    bloom: f32,
    exposure: f32,
    vignette: f32,
    tonemap: post::Tonemap,
    quality: Quality,
    denoise: u32,
    key: u64,
    /// Composition textures (shared by every frame and thread).
    textures: OnceLock<CompTextures>,
    /// Average colour per (composition, frame), for far panes.
    averages: Mutex<HashMap<(usize, u32), [f32; 4]>>,
}

#[derive(Clone)]
struct FloorParams {
    y: f32,
    color: V3,
    roughness: f32,
    mirror: f32,
    reflect: f32,
    pool: (f32, f32, f32),
    fade: (f32, f32),
}

fn num_or(m: &Map<String, Value>, k: &str, d: f32) -> Result<f32, String> {
    match m.get(k) {
        None | Some(Value::Null) => Ok(d),
        Some(v) => v.as_f64().map(|x| x as f32).filter(|x| x.is_finite()).ok_or_else(|| format!("`{k}` must be a number")),
    }
}

/// The phase-1 lookdev scene: a chrome wordmark with four supporting objects.
fn lookdev_objects(m: &Map<String, Value>) -> Vec<Value> {
    let text = m.get("text").cloned().unwrap_or(Value::from("ANIKIT"));
    let mut word = serde_json::json!({ "type": "text", "text": text, "height": 1.0, "depth": 0.34, "bevel": 0.045, "material": m.get("material").cloned().unwrap_or(Value::from("chrome")) });
    for k in ["font", "weight"] {
        if let Some(v) = m.get(k) {
            word[k] = v.clone();
        }
    }
    let mut v = vec![word];
    if m.get("primitives").and_then(|p| p.as_bool()).unwrap_or(true) {
        v.extend([
            serde_json::json!({ "type": "sphere", "radius": 0.62, "pos": [-3.3, 0.62, -1.9], "material": "black" }),
            serde_json::json!({ "type": "torus", "radius": 0.7, "tube": 0.112, "pos": [3.15, 0.82, -2.3], "rot": [0, -32, 0], "material": { "extends": "gold", "roughness": 0.16, "base": [1.0, 0.74, 0.42] } }),
            serde_json::json!({ "type": "sphere", "radius": 0.26, "pos": [-1.85, 0.26, 2.3], "material": "chrome" }),
            serde_json::json!({ "type": "sphere", "radius": 0.34, "pos": [2.75, 0.34, 0.9], "material": { "base": [0.8, 0.79, 0.77], "roughness": 0.38 } }),
        ]);
    }
    v
}

fn lookdev_camera() -> Value {
    serde_json::json!([
        { "t": 0.0, "pos": [-4.9, 1.35, 9.4], "target": [-1.3, 0.42, 1.2], "fov": 28.0, "focus": 7.9 },
        { "t": 1.6, "pos": [-4.1, 1.28, 9.2], "target": [-1.0, 0.45, 0.9], "fov": 28.0, "focus": 7.9 },
        { "t": 3.8, "pos": [-2.2, 1.15, 9.0], "target": [-0.1, 0.5, 0.0], "fov": 27.5 },
        { "t": 9.0, "pos": [2.7, 1.0, 8.9], "target": [0.2, 0.52, -0.2], "fov": 27.5 }
    ])
}

impl Prepare for Scene3d {
    fn prepare(&mut self) -> Result<(), Error> {
        let m = &self.raw;
        for k in m.keys() {
            if !TOP_KEYS.contains(&k.as_str()) {
                let hint = crate::components::style::closest(k, TOP_KEYS).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(Error(format!("unknown key `{k}`{hint}")));
            }
        }
        let lookdev = !m.contains_key("objects");
        // The lookdev shot keeps its dark studio unless told otherwise.
        let env_v = m.get("env").cloned().unwrap_or(if lookdev { Value::from("studio") } else { Value::Null });
        let es = spec::env(&env_v, "env").map_err(Error)?;
        let bright = matches!(es.name.as_str(), "cyc" | "sunset");

        let mut named: HashMap<String, Material> = HashMap::new();
        if let Some(ms) = m.get("materials") {
            let o = ms.as_object().ok_or_else(|| Error("`materials` must be an object of name → material".into()))?;
            for (name, v) in o {
                let mat = spec::material(v, &named, &format!("materials.{name}")).map_err(Error)?;
                named.insert(name.clone(), mat);
            }
        }
        let objs_v: Vec<Value> = match m.get("objects") {
            None => lookdev_objects(m),
            Some(Value::Array(a)) => a.clone(),
            Some(_) => return Err(Error("`objects` must be an array".into())),
        };
        let mut shapes = spec::Shapes::new();
        let mut mats = Vec::new();
        let mut objs = Vec::new();
        for (i, o) in objs_v.iter().enumerate() {
            objs.push(spec::object(o, &mut shapes, &mut mats, &named, &format!("objects[{i}]")).map_err(Error)?);
        }
        let shapes = shapes.list;

        // Floor.
        let floor = match m.get("floor") {
            Some(Value::Bool(false)) => None,
            v => {
                let mut fp = if bright {
                    FloorParams { y: 0.0, color: scale(es.tint, 0.92), roughness: 0.35, mirror: 0.0, reflect: 0.35, pool: (0.0, 0.0, 0.0), fade: (0.0, 0.0) }
                } else {
                    FloorParams { y: 0.0, color: v3(0.035, 0.035, 0.04), roughness: 0.09, mirror: 0.06, reflect: 1.0, pool: (0.0, 0.0, 5.0), fade: (7.0, 16.0) }
                };
                if let Some(Value::Object(fm)) = v {
                    for k in fm.keys() {
                        if !["color", "roughness", "reflect", "mirror", "y"].contains(&k.as_str()) {
                            return Err(Error(format!("floor: unknown key `{k}` (color, roughness, reflect, mirror, y)")));
                        }
                    }
                    if let Some(c) = fm.get("color") {
                        fp.color = spec::color(c, "floor.color").map_err(Error)?;
                    }
                    fp.roughness = num_or(fm, "roughness", fp.roughness).map_err(Error)?;
                    fp.reflect = num_or(fm, "reflect", fp.reflect).map_err(Error)?;
                    fp.mirror = num_or(fm, "mirror", fp.mirror).map_err(Error)?;
                    fp.y = num_or(fm, "y", fp.y).map_err(Error)?;
                }
                Some(fp)
            }
        };

        // Scene bounds at t = 0 (camera presets).
        let mut probe = Scene::new(shapes.clone(), mats.clone(), Vec::new(), es.env.clone(), None);
        probe.insts = instances(&objs, 0.0, 0.0);
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for i in &probe.insts {
            let (a, b) = probe.world_box(i);
            lo = vmin(lo, a);
            hi = vmax(hi, b);
        }
        let bounds = if probe.insts.is_empty() { (v3(0.0, 0.5, 0.0), 1.0) } else { (scale(add(lo, hi), 0.5), len(sub(hi, lo)) * 0.5) };
        let cam_v = m.get("camera").cloned().unwrap_or(if lookdev { lookdev_camera() } else { Value::Null });
        let camera = spec::camera(&cam_v, bounds, "camera").map_err(Error)?;

        // Backdrop, haze and exposure.
        let up = v3(0.0, 1.0, 0.0);
        let (backdrop, glow, haze, auto_exp, tonemap) = if bright {
            // Seamless cyc: the backdrop and haze match the floor's far brightness.
            let kd = es.env.key_box().map(|k| scale(k.radiance, es.env.key_solid_angle * dot(up, k.c).max(0.0) * (1.0 - k.falloff / 3.0) / std::f32::consts::PI)).unwrap_or(v3(0.0, 0.0, 0.0));
            let far = mul(floor.as_ref().map(|f| f.color).unwrap_or(scale(es.tint, 0.92)), add(es.env.diffuse(up), kd));
            let exp = 0.86 / luma(far).max(1e-3);
            ((far, scale(far, 0.9)), (v3(0.0, 0.0, -1.0), v3(0.0, 0.0, 0.0), 0.1), (es.haze, far), exp, post::Tonemap::Neutral)
        } else {
            let hz = (es.haze, v3(0.006, 0.006, 0.008));
            ((v3(0.006, 0.006, 0.008), v3(0.0015, 0.0015, 0.002)), (norm(v3(0.0, 0.12, -1.0)), v3(0.05, 0.045, 0.06), 0.09), hz, 1.0, post::Tonemap::Aces)
        };
        let tonemap = match m.get("tonemap").and_then(|t| t.as_str()) {
            None => tonemap,
            Some("aces") => post::Tonemap::Aces,
            Some("neutral") => post::Tonemap::Neutral,
            Some(o) => return Err(Error(format!("unknown tonemap `{o}`; expected aces or neutral"))),
        };
        let quality_s = m.get("quality").and_then(|q| q.as_str()).unwrap_or("standard");
        let (quality, denoise, bake) = match quality_s {
            "draft" => (Quality { edge_spp: 3, ao_rays: 1, shadow_rays: 1, max_depth: 2, max_glass: 6, contrast: 0.08 }, 3, (384, 24)),
            "standard" => (Quality::default(), 4, (768, 64)),
            "high" => (Quality { edge_spp: 16, ao_rays: 4, shadow_rays: 4, max_depth: 4, max_glass: 14, contrast: 0.035 }, 4, (1024, 128)),
            o => return Err(Error(format!("unknown quality `{o}`; expected draft, standard or high"))),
        };
        let denoise = num_or(m, "denoise", denoise as f32).map_err(Error)? as u32;

        // Static layouts get a baked, noise-free floor.
        let total: u64 = objs.iter().map(|o| o.count as u64 * o.lanes as u64).sum();
        let floor_vis = if floor.is_some() && total <= 400 && objs.iter().all(|o| !o.dynamic()) {
            let fp = floor.clone().unwrap();
            let mut s = Scene::new(shapes.clone(), mats.clone(), instances(&objs, 0.0, 0.0), es.env.clone(), Some(make_floor(&fp, None)));
            s.bake_floor(bake.0, bake.1);
            s.floor.and_then(|f| f.vis)
        } else {
            None
        };

        let key = fnv(&[b"scene3d-v2", Value::Object(m.clone()).to_string().as_bytes()]);
        self.prepared = Some(Prepared {
            shapes,
            mats,
            objs,
            env: es.env,
            floor,
            floor_vis,
            backdrop,
            glow,
            haze,
            camera,
            default_aperture: num_or(m, "aperture", if lookdev { 26.0 } else { 0.0 }).map_err(Error)?,
            shutter: m.get("shutter").and_then(|s| s.as_f64()).map(|s| s as f32),
            bloom: num_or(m, "bloom", if bright { 0.035 } else { 0.06 }).map_err(Error)?,
            exposure: num_or(m, "exposure", 1.0).map_err(Error)? * auto_exp,
            vignette: num_or(m, "vignette", if bright { 0.12 } else { 0.22 }).map_err(Error)?,
            tonemap,
            quality,
            denoise,
            key,
            textures: OnceLock::new(),
            averages: Mutex::new(HashMap::new()),
        });
        Ok(())
    }
}

fn make_floor(f: &FloorParams, vis: Option<Arc<FloorVis>>) -> Floor {
    Floor { y: f.y, color: f.color, roughness: f.roughness, mirror: f.mirror, reflect: f.reflect, pool: f.pool, fade: f.fade, vis }
}

/// Every object's instances at time `t` (with transforms at `t - dt` for motion).
fn instances(objs: &[ObjDef], t: f32, dt: f32) -> Vec<Inst> {
    let mut out = Vec::new();
    for o in objs {
        if t < o.start || t >= o.end {
            continue;
        }
        for lane in 0..o.lanes {
            for i in 0..o.count {
                let xf = o.xf_at(t, i, lane);
                let prev = if dt > 0.0 { o.xf_at(t - dt, i, lane) } else { xf };
                out.push(Inst { shape: o.shape, xf, prev, mat: o.mat, tex: None, thin: o.thin, shadow: o.shadow });
            }
        }
    }
    out
}

impl Scene3d {
    fn p(&self) -> &Prepared {
        self.prepared.as_ref().expect("scene3d: prepare() not called")
    }

    /// Render one frame at `w × h` (RGBA8). `root` resolves composition textures.
    pub fn render(&self, t: f32, dt: f32, w: usize, h: usize, root: Option<&Evaluator>) -> Vec<u8> {
        let p = self.p();
        let prof = std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some();
        let clock = std::time::Instant::now();
        let aspect = w as f32 / h as f32;
        let mut cam = p.camera.at(t, aspect, p.default_aperture);
        if let Some(s) = p.shutter {
            cam.shutter = s;
        }
        let prev = p.camera.at(t - dt, aspect, p.default_aperture);
        let mut scene = Scene::new(p.shapes.clone(), p.mats.clone(), Vec::new(), p.env.clone(), p.floor.as_ref().map(|f| make_floor(f, p.floor_vis.clone())));
        scene.backdrop = p.backdrop;
        scene.glow = p.glow;
        scene.haze = p.haze;
        let mut insts = instances(&p.objs, t, dt);
        // Textures need world boxes, so build the hierarchy first.
        scene.insts = std::mem::take(&mut insts);
        let n_tex = self.assign_textures(&mut scene, &cam, t, w, h, root);
        scene.rebuild();
        let t_setup = clock.elapsed();

        let mut px = vec![trace::Px::default(); w * h];
        let band = 8usize;
        let q = p.quality;
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
        let t_trace = clock.elapsed();
        let pp = post::Post { focus: cam.focus, aperture: cam.aperture, shutter: cam.shutter, bloom: p.bloom, exposure: p.exposure, vignette: p.vignette, tonemap: p.tonemap, denoise: p.denoise };
        let out = post::process(&px, w, h, &pp);
        if prof {
            let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3;
            eprintln!(
                "scene3d t={t:.3} {w}x{h}: setup {:.0} ms ({} instances, {n_tex} textured), trace {:.0} ms, post {:.0} ms",
                ms(t_setup),
                scene.insts.len(),
                ms(t_trace - t_setup),
                ms(clock.elapsed() - t_trace)
            );
        }
        out
    }

    /// Give each textured instance its composition frame at a level of detail
    /// matching its size on screen (a flat average when tiny or off screen).
    fn assign_textures(&self, scene: &mut Scene, cam: &trace::Camera, t: f32, w: usize, h: usize, root: Option<&Evaluator>) -> usize {
        let p = self.p();
        let mut k = 0usize;
        let mut n = 0usize;
        let (fwd, right, up) = cam.basis();
        let th = (cam.fov * 0.5).to_radians().tan();
        let aspect = w as f32 / h as f32;
        let focal_px = h as f32 * 0.5 / th;
        for o in &p.objs {
            if t < o.start || t >= o.end {
                continue;
            }
            let count = (o.lanes * o.count) as usize;
            let comp = o.tex.as_ref().and_then(|ts| root.and_then(|r| r.composition(&ts.comp)).map(|c| (ts, c)));
            let Some((ts, comp)) = comp else {
                k += count;
                continue;
            };
            let cache = p.textures.get_or_init(|| CompTextures::new(1 << 30));
            let fps = comp.fps.max(1.0);
            for lane in 0..o.lanes {
                for i in 0..o.count {
                    let (lo, hi) = scene.world_box(&scene.insts[k]);
                    let s = scene.insts[k].xf.s;
                    let ct = match &ts.frame {
                        Some(fr) => {
                            let vars = crate::eval::expr::Vars { t: t as f64, scene_t: lane as f64, gt: t as f64, i: i as f64, n: o.count as f64, ..Default::default() };
                            (fr.at(&vars) + 0.25) / fps
                        }
                        None => crate::eval::CompTime { trim: ts.offset, speed: ts.speed, looped: ts.looped }.map(t, comp.duration),
                    };
                    let ct = if ts.looped && comp.duration > 0.0 { ct.rem_euclid(comp.duration) } else { ct.clamp(0.0, comp.duration.max(0.0)) };
                    // On-screen size → LOD.
                    let c = scale(add(lo, hi), 0.5);
                    let rel = sub(c, cam.pos);
                    let z = dot(rel, fwd);
                    let r = len(sub(hi, lo)) * 0.5;
                    let on_screen = z > -r && {
                        let zz = z.max(1e-3);
                        let sx = dot(rel, right) / zz / (th * aspect);
                        let sy = dot(rel, up) / zz / th;
                        let m = r / zz / th;
                        sx.abs() < 1.0 + m * aspect.max(1.0) && sy.abs() < 1.0 + m
                    };
                    let px_w = if z > 1e-3 { o.extent * 2.0 * s * focal_px / z } else { 0.0 };
                    let tex = if on_screen && px_w >= 6.0 {
                        let lod = ((comp.width as f32 / px_w.max(1.0)).log2().floor().max(0.0) as u8).min(4);
                        n += 1;
                        Tex::Image(cache.get(comp, ct, lod))
                    } else {
                        // Tiny or off screen (only seen in reflections): average colour.
                        let id = (Arc::as_ptr(comp) as usize, CompTextures::frame_index(comp, ct));
                        let cached = p.averages.lock().unwrap().get(&id).copied();
                        let avg = cached.unwrap_or_else(|| {
                            let a = Tex::average(&cache.get(comp, ct, 4));
                            p.averages.lock().unwrap().insert(id, a);
                            a
                        });
                        Tex::Flat(avg)
                    };
                    scene.insts[k].tex = Some((tex, ts.face));
                    k += 1;
                }
            }
        }
        n
    }
}

impl Component for Scene3d {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let p = self.p();
        let (w, h) = (ctx.width.round().max(2.0) as u32, ctx.height.round().max(2.0) as u32);
        let fps = ctx.fps.max(1.0);
        let frame = (t * fps).round();
        let t = frame / fps;
        let key = fnv(&[&p.key.to_le_bytes(), &frame.to_bits().to_le_bytes(), &fps.to_bits().to_le_bytes(), &w.to_le_bytes(), &h.to_le_bytes()]);
        let root = ctx.root;
        let image = ctx.assets.image(key, w, h, &mut || self.render(t, 1.0 / fps, w as usize, h as usize, root));
        vec![Op::Image { image, dst: [0.0, 0.0, ctx.width, ctx.height], opacity: 1.0, transform: crate::dl::IDENTITY }]
    }
}
