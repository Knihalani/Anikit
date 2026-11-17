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

#![allow(clippy::too_many_arguments, clippy::needless_range_loop, clippy::type_complexity, clippy::question_mark, clippy::new_without_default)]

pub mod authoring;
pub mod bvh;
pub mod env;
pub mod geom;
pub mod gltf;
pub mod kernel;
pub mod math;
pub mod particles;
pub mod physics;
pub use kernel::post;
pub mod spec;
pub mod trace;

use crate::components::style::fnv;
use crate::components::{Component, Ctx, Error, Prepare};
use crate::dl::Op;
use crate::components::Assets;
use math::*;
use serde_json::{Map, Value};
use spec::{CamSpec, ObjDef};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use trace::{Floor, FloorVis, Inst, Material, Quality, Scene, Shape, Tex};

/// Spec shorthand, applied before parsing:
/// - a layer `{ "scene3d": {...}, ...layer keys }` becomes
///   `{ "component": "scene3d", "props": {...}, ...layer keys }`;
/// - a scene `{ "scene3d": {...}, ... }` gets that layer first in its `layers`
///   (full frame, behind everything else).
pub fn shorthand(mut v: Value) -> Value {
    fn layer(l: &mut Value) {
        if let Value::Object(m) = l {
            if let Some(props) = m.remove("scene3d") {
                m.insert("component".into(), Value::from("scene3d"));
                m.insert("props".into(), props);
            }
            if let Some(Value::Array(ls)) = m.get_mut("layers") {
                ls.iter_mut().for_each(layer);
            }
            if let Some(Value::Array(ls)) = m.get_mut("children") {
                ls.iter_mut().for_each(layer);
            }
        }
    }
    fn scene(s: &mut Value) {
        if let Value::Object(m) = s {
            if let Some(props) = m.remove("scene3d") {
                let l = serde_json::json!({ "component": "scene3d", "props": props });
                match m.get_mut("layers") {
                    Some(Value::Array(ls)) => ls.insert(0, l),
                    _ => {
                        m.insert("layers".into(), Value::Array(vec![l]));
                    }
                }
            }
            if let Some(Value::Array(ls)) = m.get_mut("layers") {
                ls.iter_mut().for_each(layer);
            }
            if let Some(Value::Array(ss)) = m.get_mut("scenes") {
                ss.iter_mut().for_each(scene);
            }
        }
    }
    scene(&mut v);
    if let Some(Value::Object(cs)) = v.get_mut("compositions") {
        cs.values_mut().for_each(scene);
    }
    v
}

const TOP_KEYS: &[&str] = &[
    "text", "font", "weight", "env", "material", "primitives", "camera", "objects", "materials", "floor", "aperture", "shutter", "bloom", "exposure", "tonemap", "quality", "resolution", "vignette", "denoise", "lights", "fog", "physics", "glass_layers",
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
    lights: Vec<trace::Light>,
    /// Volumetric light shafts in the fog.
    shafts: bool,
    camera: CamSpec,
    default_aperture: f32,
    shutter: Option<f32>,
    bloom: f32,
    exposure: f32,
    vignette: f32,
    tonemap: post::Tonemap,
    quality: Quality,
    denoise: u32,
    /// `quality: draft`: cheaper particle splats.
    draft: bool,
    key: u64,
    /// Average colour per (composition, frame), for far panes; frame
    /// `u32::MAX` is the composition's fallback (its first frame).
    averages: Mutex<HashMap<(String, u32), [f32; 4]>>,
    /// Particle systems, and their simulations (made on the first frame, which knows the fps).
    parts: Vec<particles::Def>,
    psims: OnceLock<Result<Vec<particles::Sim>, String>>,
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
        let clock = std::time::Instant::now();
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
        let mut es = spec::env(&env_v, "env").map_err(Error)?;
        let mut local_lights = Vec::new();
        if let Some(l) = m.get("lights") {
            let (e, loc) = spec::lights(l, &es.env, "lights").map_err(Error)?;
            es.env = e;
            local_lights = loc;
        }
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
            Some(Value::Array(a)) => {
                // `{ "script": "..." }` items expand to the objects they return.
                let mut out = Vec::with_capacity(a.len());
                for (i, o) in a.iter().enumerate() {
                    match o.get("script") {
                        Some(code) if o.as_object().is_some_and(|m| m.len() == 1) => {
                            let gen = crate::spec::script::items(code, &serde_json::Map::new(), &format!("objects[{i}]")).map_err(|d| Error(format!("objects[{i}].script: {}", d.message)))?;
                            out.extend(gen);
                        }
                        _ => out.push(o.clone()),
                    }
                }
                out
            }
            Some(_) => return Err(Error("`objects` must be an array".into())),
        };
        let mut shapes = spec::Shapes::new();
        let mut mats = Vec::new();
        let mut objs = Vec::new();
        let mut parts = Vec::new();
        for (i, o) in objs_v.iter().enumerate() {
            if o.get("type").and_then(|t| t.as_str()) == Some("particles") {
                parts.push(particles::parse(o, &mut shapes, &mut mats, &named, &objs_v, &format!("objects[{i}]")).map_err(Error)?);
                continue;
            }
            objs.push(spec::object(o, &mut shapes, &mut mats, &named, &format!("objects[{i}]")).map_err(Error)?);
        }

        // Physics: body groups become objects whose transforms come from the simulation.
        if let Some(pv) = m.get("physics") {
            let floor_y = match m.get("floor") {
                Some(Value::Bool(false)) => None,
                Some(Value::Object(fm)) => Some(num_or(fm, "y", 0.0).map_err(Error)?),
                _ => Some(0.0),
            };
            let parsed = physics::parse(pv, &objs_v, &mut shapes, &mut mats, &named, &objs, floor_y).map_err(Error)?;
            objs = objs.into_iter().enumerate().filter(|(i, _)| !parsed.templates.contains(i)).map(|(_, o)| o).collect();
            objs.extend(parsed.objs);
        }
        let shapes = shapes.list;

        // Floor.
        let floor = match m.get("floor") {
            Some(Value::Bool(false)) => None,
            v => {
                let mut fp = if bright {
                    FloorParams { y: 0.0, color: scale(es.tint, 0.92), roughness: 0.35, mirror: 0.0, reflect: 0.35, pool: (0.0, 0.0, 0.0), fade: (25.0, 250.0) }
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
        if let Some(n) = &camera.target_obj {
            if !objs.iter().any(|o| o.name.as_deref() == Some(n)) {
                let names: Vec<&str> = objs.iter().filter_map(|o| o.name.as_deref()).collect();
                let hint = crate::components::style::closest(n, &names).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(Error(format!("camera.target: no object named `{n}`{hint}")));
            }
        }

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
        // `fog`: { "density": per metre, "color", "shafts" } overrides the env's haze.
        let mut shafts = false;
        let haze = match m.get("fog") {
            None => haze,
            Some(Value::Number(d)) => (d.as_f64().unwrap_or(0.0) as f32, haze.1),
            Some(Value::Object(f)) => {
                for k in f.keys() {
                    if !["density", "color", "shafts"].contains(&k.as_str()) {
                        return Err(Error(format!("fog: unknown key `{k}` (density, color, shafts)")));
                    }
                }
                shafts = f.get("shafts").and_then(|x| x.as_bool()).unwrap_or(false);
                let d = num_or(f, "density", haze.0).map_err(Error)?;
                let c = f.get("color").map(|c| spec::color(c, "fog.color")).transpose().map_err(Error)?.unwrap_or(haze.1);
                (d, c)
            }
            Some(_) => return Err(Error("`fog` must be a density (per metre) or { density, color }".into())),
        };
        let tonemap = match m.get("tonemap").and_then(|t| t.as_str()) {
            None => tonemap,
            Some("aces") => post::Tonemap::Aces,
            Some("neutral") => post::Tonemap::Neutral,
            Some(o) => return Err(Error(format!("unknown tonemap `{o}`; expected aces or neutral"))),
        };
        let quality_s = m.get("quality").and_then(|q| q.as_str()).unwrap_or("standard");
        let draft = quality_s == "draft";
        let (quality, denoise, bake) = match quality_s {
            "draft" => (Quality { edge_spp: 3, ao_rays: 1, shadow_rays: 1, max_depth: 2, max_glass: 6, contrast: 0.1, secondary_rays: 0, full: false }, 5, (384, 24)),
            "standard" => (Quality::default(), 5, (768, 64)),
            "high" => (Quality { edge_spp: 16, ao_rays: 3, shadow_rays: 3, max_depth: 4, max_glass: 14, contrast: 0.035, secondary_rays: 4, full: false }, 5, (1024, 128)),
            o => return Err(Error(format!("unknown quality `{o}`; expected draft, standard or high"))),
        };
        let denoise = num_or(m, "denoise", denoise as f32).map_err(Error)? as u32;
        // `glassLayers`: how many panes a ray may pass through (2 interfaces each).
        let mut quality = quality;
        if let Some(g) = m.get("glass_layers") {
            let g = g.as_f64().filter(|g| *g >= 0.0).ok_or_else(|| Error("`glassLayers` must be a non-negative number".into()))?;
            quality.max_glass = (g as u32 * 2).min(64);
        }

        // Static layouts get a baked, noise-free floor.
        let total: u64 = objs.iter().map(|o| o.count as u64 * o.lanes as u64).sum();
        let t_objs = clock.elapsed();
        let floor_vis = if floor.is_some() && total <= 400 && objs.iter().all(|o| !o.dynamic()) {
            let fp = floor.clone().unwrap();
            // The bake is deterministic, so it is cached on disk by the scene's
            // description (plus model files' size and mtime).
            let mut srcs = Vec::new();
            fn walk(v: &Value, out: &mut Vec<u8>) {
                match v {
                    Value::Object(m) => {
                        if let Some(Value::String(src)) = m.get("src") {
                            if let Ok(md) = std::fs::metadata(src) {
                                out.extend_from_slice(format!("{src}|{}|{:?}", md.len(), md.modified().ok()).as_bytes());
                            }
                        }
                        m.values().for_each(|x| walk(x, out));
                    }
                    Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
                    _ => {}
                }
            }
            walk(&Value::Object(m.clone()), &mut srcs);
            let bkey = fnv(&[b"floor-bake-v1", Value::Object(m.clone()).to_string().as_bytes(), &srcs, &(bake.0 as u64).to_le_bytes(), &(bake.1 as u64).to_le_bytes()]);
            let file = sim_cache_dir().map(|d| d.join(format!("floor-{bkey:016x}.bin")));
            match file.as_ref().and_then(|f| read_floor_vis(f)) {
                Some(v) => Some(Arc::new(v)),
                None => {
                    let mut s = Scene::new(shapes.clone(), mats.clone(), instances(&objs, 0.0, 0.0), es.env.clone(), Some(make_floor(&fp, None)));
                    s.bake_floor(bake.0, bake.1);
                    let vis = s.floor.and_then(|f| f.vis);
                    if let (Some(f), Some(v)) = (&file, &vis) {
                        write_floor_vis(f, v);
                    }
                    vis
                }
            }
        } else {
            None
        };

        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
            eprintln!("scene3d prepare: objects {:.0} ms, floor bake {:.0} ms", t_objs.as_secs_f64() * 1e3, (clock.elapsed() - t_objs).as_secs_f64() * 1e3);
        }
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
            lights: local_lights,
            shafts,
            camera,
            default_aperture: num_or(m, "aperture", if lookdev { 26.0 } else { 0.0 }).map_err(Error)?,
            shutter: m.get("shutter").and_then(|s| s.as_f64()).map(|s| s as f32),
            bloom: num_or(m, "bloom", if bright { 0.035 } else { 0.06 }).map_err(Error)?,
            exposure: num_or(m, "exposure", 1.0).map_err(Error)? * auto_exp,
            vignette: num_or(m, "vignette", if bright { 0.12 } else { 0.22 }).map_err(Error)?,
            tonemap,
            quality,
            denoise,
            draft,
            key,
            parts,
            psims: OnceLock::new(),
            averages: Mutex::new(HashMap::new()),
        });
        Ok(())
    }
}

fn make_floor(f: &FloorParams, vis: Option<Arc<FloorVis>>) -> Floor {
    Floor { y: f.y, color: f.color, roughness: f.roughness, mirror: f.mirror, reflect: f.reflect, pool: f.pool, fade: f.fade, vis }
}

fn read_floor_vis(path: &std::path::Path) -> Option<FloorVis> {
    let b = std::fs::read(path).ok()?;
    let u = |k: usize| -> Option<u64> { Some(u64::from_le_bytes(b.get(k..k + 8)?.try_into().ok()?)) };
    let g = |k: usize| -> Option<f32> { Some(f32::from_le_bytes(b.get(k..k + 4)?.try_into().ok()?)) };
    let (nx, nz) = (u(0)? as usize, u(8)? as usize);
    let n = nx.checked_mul(nz)?;
    if b.len() != 32 + n * 8 {
        return None;
    }
    let data = (0..n).map(|i| [g(32 + i * 8).unwrap(), g(36 + i * 8).unwrap()]).collect();
    Some(FloorVis { x0: g(16)?, z0: g(20)?, x1: g(24)?, z1: g(28)?, nx, nz, data })
}

fn write_floor_vis(path: &std::path::Path, v: &FloorVis) {
    let mut b = Vec::with_capacity(32 + v.data.len() * 8);
    b.extend_from_slice(&(v.nx as u64).to_le_bytes());
    b.extend_from_slice(&(v.nz as u64).to_le_bytes());
    for x in [v.x0, v.z0, v.x1, v.z1] {
        b.extend_from_slice(&x.to_le_bytes());
    }
    for d in &v.data {
        b.extend_from_slice(&d[0].to_le_bytes());
        b.extend_from_slice(&d[1].to_le_bytes());
    }
    // Write then rename, so parallel renders never read a partial file.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, &b).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Where simulations are cached: `$ANIKIT_CACHE/sim` (default
/// `$XDG_CACHE_HOME/anikit/sim` or `~/.cache/anikit/sim`); `ANIKIT_CACHE=off`
/// disables caching.
pub fn sim_cache_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let base = match std::env::var("ANIKIT_CACHE") {
        Ok(v) if v == "off" || v == "0" => return None,
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => match std::env::var_os("XDG_CACHE_HOME") {
            Some(x) => PathBuf::from(x).join("anikit"),
            None => PathBuf::from(std::env::var_os("HOME")?).join(".cache").join("anikit"),
        },
    };
    let dir = base.join("sim");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Keep the simulation cache under `ANIKIT_CACHE_MAX_GB` (default 8 GB) by
/// deleting the least recently written files.
pub fn prune_sim_cache() {
    let Some(dir) = sim_cache_dir() else { return };
    let max = std::env::var("ANIKIT_CACHE_MAX_GB").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(8.0) * 1e9;
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, len, path) in files {
        if (total as f64) <= max {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// Every object's instances at time `t` (with transforms at `t - dt` for motion).
fn instances(objs: &[ObjDef], t: f32, dt: f32) -> Vec<Inst> {
    let mut out = Vec::new();
    for (oi, o) in objs.iter().enumerate() {
        if t < o.start || t >= o.end {
            continue;
        }
        if let Some((sim, first)) = &o.phys {
            let (a, b, u) = sim.at(t);
            let (pa, pb, pu) = if dt > 0.0 { sim.at(t - dt) } else { (a.clone(), b.clone(), u) };
            let first = *first as usize;
            for i in 0..o.count as usize {
                let s = sim.scale_of(first + i);
                let xf = physics::xf_of(&physics::lerp_pose(&a[first + i], &b[first + i], u), s);
                let prev = physics::xf_of(&physics::lerp_pose(&pa[first + i], &pb[first + i], pu), s);
                for &(shape, mat) in &o.parts {
                    out.push(Inst { shape, xf, prev, mat, tex: Vec::new(), thin: o.thin, shadow: o.shadow, cam: o.cam, obj: oi as u32, i: i as u32, lane: 0 });
                }
            }
            continue;
        }
        for lane in 0..o.lanes {
            for i in 0..o.count {
                let xf = o.xf_at(t, i, lane);
                // Scale ≈ 0 hides an instance: it costs nothing (no texture, not traced).
                if xf.smin() <= 1e-4 * o.scale_mul.max(1e-6) {
                    continue;
                }
                let prev = if dt > 0.0 { o.xf_at(t - dt, i, lane) } else { xf };
                match &o.anim {
                    // Animated glTF: each part follows its node.
                    Some(a) => {
                        let now = a.parts_at(&xf, t);
                        let before = a.parts_at(&prev, t - dt);
                        for (j, &(shape, mat)) in o.parts.iter().enumerate() {
                            out.push(Inst { shape, xf: now[j], prev: before[j], mat, tex: Vec::new(), thin: o.thin, shadow: o.shadow, cam: o.cam, obj: oi as u32, i, lane });
                        }
                    }
                    None => {
                        for &(shape, mat) in &o.parts {
                            out.push(Inst { shape, xf, prev, mat, tex: Vec::new(), thin: o.thin, shadow: o.shadow, cam: o.cam, obj: oi as u32, i, lane });
                        }
                    }
                }
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
    pub fn render(&self, t: f32, dt: f32, w: usize, h: usize, assets: Option<&dyn Assets>) -> Vec<u8> {
        let p = self.p();
        let prof = std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some();
        if prof {
            trace::PROFILE.store(true, std::sync::atomic::Ordering::Relaxed);
            for r in trace::RAYS.iter().chain(&trace::TIMES) {
                r.store(0, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let clock = std::time::Instant::now();
        let aspect = w as f32 / h as f32;
        let mut cam = p.camera.at(t, aspect, p.default_aperture);
        if let Some(s) = p.shutter {
            cam.shutter = s;
        }
        let mut prev_cam = p.camera.at(t - dt, aspect, p.default_aperture);
        // Open geometry that reads `t` is re-tessellated for this frame.
        let mut shapes = p.shapes.clone();
        for o in &p.objs {
            if let Some(r) = &o.regen {
                if let Ok(Some(m)) = authoring::build(&r.0, &r.1, &r.2, t) {
                    shapes[r.3 as usize] = spec::mesh_shape(m);
                }
            }
        }
        let mut scene = Scene::new(shapes, p.mats.clone(), Vec::new(), p.env.clone(), p.floor.as_ref().map(|f| make_floor(f, p.floor_vis.clone())));
        scene.backdrop = p.backdrop;
        scene.glow = p.glow;
        scene.haze = p.haze;
        scene.lights = p.lights.clone();
        let mut insts = instances(&p.objs, t, dt);
        // Textures need world boxes, so build the hierarchy first.
        scene.insts = std::mem::take(&mut insts);
        // A camera aimed at (or following) a named object.
        if let Some(name) = &p.camera.target_obj {
            if let Some(oi) = p.objs.iter().position(|o| o.name.as_deref() == Some(name)) {
                let centre = |prev: bool| -> Option<V3> {
                    let inst = scene.insts.iter().find(|i| i.obj as usize == oi)?;
                    let mut i2 = inst.clone();
                    if prev {
                        i2.xf = inst.prev;
                    }
                    let (lo, hi) = scene.world_box(&i2);
                    Some(scale(add(lo, hi), 0.5))
                };
                let explicit_focus = p.camera.keys.iter().any(|k| k.focus.is_some());
                for (c, prev) in [(&mut cam, false), (&mut prev_cam, true)] {
                    if let Some(o) = centre(prev) {
                        c.target = add(c.target, o);
                        if p.camera.follow {
                            c.pos = add(c.pos, o);
                        }
                        if !explicit_focus {
                            c.focus = len(sub(c.target, c.pos));
                        }
                    }
                }
            }
        }
        let prev = prev_cam;
        let n_tex = self.assign_textures(&mut scene, &cam, t, w, h, assets);
        scene.rebuild();
        let t_setup = clock.elapsed();
        if prof && std::env::var("ANIKIT_SCENE3D_PROFILE").as_deref() == Ok("2") {
            scene.micro(&cam, w, h);
        }

        let mut px: kernel::pool::Buf<trace::Px> = kernel::pool::Buf::uninit(w * h);
        let mut q = p.quality;
        if crate::components::reference() || std::env::var_os("ANIKIT_SCENE3D_REFERENCE").is_some() {
            // Converged sampling with the same image model: 1 + 24 samples per
            // pixel everywhere, 4 shadow and AO rays per sample.
            q.full = true;
            q.edge_spp = 24;
            q.ao_rays = q.ao_rays.max(4);
            q.shadow_rays = q.shadow_rays.max(4);
        }
        scene.render_frame(&cam, &prev, w, h, &q, &mut px);
        let t_trace = clock.elapsed();
        let layer = self.particles(t, dt, &cam, &prev, &mut px, w, h, assets);
        let t_parts = clock.elapsed();
        let pp = post::Post { focus: cam.focus, aperture: cam.aperture, shutter: cam.shutter, bloom: p.bloom, exposure: p.exposure, vignette: p.vignette, tonemap: p.tonemap, denoise: p.denoise };
        if let Some(path) = std::env::var_os("ANIKIT_SCENE3D_DUMP") {
            // Development aid: the primary pass and post settings, for examples/post_bench.
            post::dump(std::path::Path::new(&path), &px, w, h, &pp);
        }
        let vol = if p.shafts && p.haze.0 > 0.0 {
            let depth: Vec<f32> = px.iter().map(|q| q.depth).collect();
            let steps = match p.quality.edge_spp { 0..=3 => 8, 4..=8 => 16, _ => 32 };
            Some(scene.volumetrics(&cam, w, h, &depth, p.haze.0, steps))
        } else {
            None
        };
        let out = post::process(&px, w, h, &pp, vol.as_ref().map(|v| post::Volume { px: v, w: w.div_ceil(2), h: h.div_ceil(2) }), layer.as_ref());
        if prof {
            let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3;
            eprintln!(
                "scene3d t={t:.3} {w}x{h}: setup {:.0} ms ({} instances, {n_tex} textured), trace {:.0} ms, particles {:.0} ms, post {:.0} ms",
                ms(t_setup),
                scene.insts.len(),
                ms(t_trace - t_setup),
                ms(t_parts - t_trace),
                ms(clock.elapsed() - t_parts)
            );
            let tm: Vec<u64> = trace::TIMES.iter().map(|a| a.load(std::sync::atomic::Ordering::Relaxed)).collect();
            eprintln!("  wall: centre pass {:.0} ms, refinement {:.0} ms ({:.1}% of pixels refined)", tm[0] as f64 / 1e3, tm[1] as f64 / 1e3, tm[2] as f64 * 100.0 / (w * h) as f64);
            let r: Vec<u64> = trace::RAYS.iter().map(|a| a.load(std::sync::atomic::Ordering::Relaxed)).collect();
            eprintln!("  rays: {:.2}M closest ({:.2}M radiance, {:.2}M floor reflections), {:.2}M shadow/AO, {:.2} primary samples per pixel", r[0] as f64 / 1e6, r[3] as f64 / 1e6, r[4] as f64 / 1e6, r[1] as f64 / 1e6, r[2] as f64 / (w * h) as f64);
        }
        out
    }

    /// Simulate and splat the particle systems live at `t`.
    #[allow(clippy::too_many_arguments)]
    fn particles(&self, t: f32, dt: f32, cam: &trace::Camera, prev: &trace::Camera, px: &mut [trace::Px], w: usize, h: usize, assets: Option<&dyn Assets>) -> Option<particles::Layer> {
        let p = self.p();
        if p.parts.is_empty() {
            return None;
        }
        let sims = p.psims.get_or_init(|| {
            let floor_y = p.floor.as_ref().map(|f| f.y);
            let colliders: Vec<spec::ObjDef> = p.objs.iter().filter(|o| o.collide && o.phys.is_none()).cloned().collect();
            p.parts
                .iter()
                .map(|d| {
                    particles::Sim::new(d, 1.0 / dt.max(1e-4), floor_y, assets).map(|mut s| {
                        s.set_colliders(colliders.clone());
                        s
                    })
                })
                .collect()
        });
        let sims = match sims {
            Ok(s) => s,
            Err(e) => {
                eprintln!("scene3d particles: {e}");
                return None;
            }
        };
        let live: Vec<(&particles::Sim, Arc<particles::Snap>)> = sims.iter().filter(|s| t >= s.def.start && t < s.def.end).map(|s| (s, s.at(t))).collect();
        if live.is_empty() {
            return None;
        }
        let depth: Vec<f32> = px.iter().map(|q| q.depth).collect();
        let up = v3(0.0, 1.0, 0.0);
        let key = p.env.key_box().map(|k| (k.c, scale(k.radiance, p.env.key_solid_angle))).unwrap_or((up, v3(0.0, 0.0, 0.0)));
        // Particle shadows on the scene: dim the key light by the particles' transmittance.
        let c0 = std::time::Instant::now();
        let shadow = particles::ShadowMap::build(&live, key.0, t);
        let c1 = c0.elapsed();
        if let Some(sm) = &shadow {
            let (fwd, _, _) = cam.basis();
            let body = |(y, row): (usize, &mut [trace::Px])| {
                for (x, q) in row.iter_mut().enumerate() {
                    if q.depth > 1e5 || q.vk <= 0.0 {
                        continue;
                    }
                    let d = Scene::cam_ray(cam, w, h, x as f32 + 0.5, y as f32 + 0.5);
                    let wp = madd(cam.pos, d, q.depth / dot(d, fwd).max(1e-4));
                    q.vk *= sm.trans(wp);
                }
            };
            use rayon::prelude::*;
            if rayon::current_thread_index().is_some() {
                px.chunks_mut(w).enumerate().for_each(body);
            } else {
                px.par_chunks_mut(w).enumerate().for_each(body);
            }
        }
        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() && shadow.is_some() {
            eprintln!("  particle shadows: map {:.0} ms, scene lookups {:.0} ms", c1.as_secs_f64() * 1e3, (c0.elapsed() - c1).as_secs_f64() * 1e3);
        }
        let view = particles::View { cam, prev, w, h, depth: &depth, focus: cam.focus, aperture: cam.aperture, shutter: cam.shutter, key, amb: (p.env.diffuse(up), p.env.diffuse(scale(up, -1.0))), shadow: shadow.as_ref(), draft: p.draft };
        Some(particles::render(&live, &view, t))
    }

    /// Give each textured instance its composition frame at a level of detail
    /// matching its size on screen. Tiny or off-screen instances (seen only in
    /// reflections) take an average colour and never render a frame of their
    /// own: the average of an already-rendered frame, or the composition's
    /// fallback average (one tiny render per composition).
    fn assign_textures(&self, scene: &mut Scene, cam: &trace::Camera, t: f32, w: usize, h: usize, assets: Option<&dyn Assets>) -> usize {
        let p = self.p();
        let Some(assets) = assets else { return 0 };
        let (fwd, right, up) = cam.basis();
        let th = (cam.fov * 0.5).to_radians().tan();
        let aspect = w as f32 / h as f32;
        let focal_px = h as f32 * 0.5 / th;
        let mut infos: HashMap<String, Option<crate::components::CompInfo>> = HashMap::new();
        let mut n = 0usize;
        for k in 0..scene.insts.len() {
            let inst = scene.insts[k].clone();
            let o = &p.objs[inst.obj as usize];
            for ts in &o.tex {
            let info = *infos.entry(ts.comp.clone()).or_insert_with(|| assets.composition_info(&ts.comp));
            let Some(info) = info else { continue };
            let fps = info.fps.max(1.0);
            let dur = info.duration.max(0.0);
            let ct = match &ts.frame {
                Some(fr) => {
                    let vars = crate::eval::expr::Vars { t: t as f64, scene_t: inst.lane as f64, gt: t as f64, i: inst.i as f64, n: o.count as f64, ..Default::default() };
                    (fr.at(&vars) + 0.25) / fps
                }
                None => ts.offset + t * ts.speed,
            };
            let ct = if ts.looped && dur > 0.0 { ct.rem_euclid(dur) } else { ct.clamp(0.0, (dur - 1e-4).max(0.0)) };
            let frame = ((ct * fps + 1e-3).floor().max(0.0)) as u32;
            // On-screen size → LOD.
            let (lo, hi) = scene.world_box(&inst);
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
            let px_w = if z > 1e-3 { o.extent * 2.0 * inst.xf.smax() * focal_px / z } else { 0.0 };
            let tex = if on_screen && px_w >= 6.0 {
                let lod = ((info.width as f32 / px_w.max(1.0)).log2().floor().max(0.0) as u8).min(5);
                match assets.composition_frame(&ts.comp, ct, lod) {
                    Some(pm) => {
                        n += 1;
                        let avg = Tex::average(&pm);
                        p.averages.lock().unwrap().entry((ts.comp.clone(), frame)).or_insert(avg);
                        Tex::Image(pm)
                    }
                    None => continue,
                }
            } else {
                let known = { p.averages.lock().unwrap().get(&(ts.comp.clone(), frame)).copied() };
                let avg = known.or_else(|| p.averages.lock().unwrap().get(&(ts.comp.clone(), u32::MAX)).copied()).unwrap_or_else(|| {
                    let a = assets.composition_frame(&ts.comp, 0.0, 5).map(|pm| Tex::average(&pm)).unwrap_or([0.5, 0.5, 0.5, 1.0]);
                    p.averages.lock().unwrap().insert((ts.comp.clone(), u32::MAX), a);
                    a
                });
                Tex::Flat(avg)
            };
            scene.insts[k].tex.push((tex, ts.face));
            }
        }
        n
    }
}

impl Component for Scene3d {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let p = self.p();
        // Previews and reviews (`still --width`, `critique --render`) trace at the size they show.
        // `resolution` (0.25–1): trace at a fraction of the layer size and upscale. Unset, it is
        // full size at standard/high quality and half size in drafts (`--quality draft`), so
        // iterating is fast and final renders lose nothing.
        let default_res = if crate::components::draft() { 0.5 } else { 1.0 };
        let res = self.raw.get("resolution").and_then(|r| r.as_f64()).unwrap_or(default_res).clamp(0.25, 1.0) as f32;
        let ps = crate::eval::shader::preview_scale() * res;
        let (w, h) = ((ctx.width * ps).round().max(2.0) as u32, (ctx.height * ps).round().max(2.0) as u32);
        let fps = ctx.fps.max(1.0);
        let frame = (t * fps).round();
        let t = frame / fps;
        let key = fnv(&[&p.key.to_le_bytes(), &frame.to_bits().to_le_bytes(), &fps.to_bits().to_le_bytes(), &w.to_le_bytes(), &h.to_le_bytes()]);
        let assets = ctx.assets;
        let image = ctx.assets.image(key, w, h, &mut || self.render(t, 1.0 / fps, w as usize, h as usize, Some(assets)));
        vec![Op::Image { image, dst: [0.0, 0.0, ctx.width, ctx.height], opacity: 1.0, transform: crate::dl::IDENTITY }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scene(v: Value) -> Result<Scene3d, String> {
        let mut s = Scene3d::try_from(v)?;
        s.prepare().map_err(|e| e.0)?;
        Ok(s)
    }

    #[test]
    fn renders_cyc_objects_deterministically() {
        let s = scene(json!({
            "env": "cyc", "quality": "draft",
            "camera": { "keys": [ { "t": 0, "pos": [0, 1, 4], "target": [0, 0.4, 0], "focal": 35 } ] },
            "objects": [
                { "text": "AK", "height": 0.5, "material": "chrome" },
                { "type": "slab", "count": 3, "pos": "=[i - 1, 0.47, -1]", "material": "glass" },
                { "type": "sphere", "radius": 0.2, "pos": [1, 0.2, 0.5], "material": "matte" }
            ]
        }))
        .unwrap();
        let a = s.render(0.0, 1.0 / 30.0, 96, 54, None);
        let b = s.render(0.0, 1.0 / 30.0, 96, 54, None);
        assert_eq!(a.len(), 96 * 54 * 4);
        assert_eq!(a, b, "frames must be deterministic");
        // A bright cyc: the mean is light, and the objects add contrast.
        let lum: Vec<f32> = a.chunks_exact(4).map(|p| (p[0] as f32 + p[1] as f32 + p[2] as f32) / 765.0).collect();
        let mean = lum.iter().sum::<f32>() / lum.len() as f32;
        let min = lum.iter().cloned().fold(1.0f32, f32::min);
        assert!(mean > 0.6, "cyc should be bright, mean {mean}");
        assert!(min < 0.45, "objects should show, min {min}");
    }

    #[test]
    fn lookdev_props_still_work() {
        let s = scene(json!({ "text": "ANIKIT", "quality": "draft", "primitives": false })).unwrap();
        let px = s.render(1.0, 1.0 / 30.0, 64, 36, None);
        assert!(px.chunks_exact(4).any(|p| p[0] > 40));
    }

    #[test]
    fn errors_suggest_fixes() {
        // Scripts expand inside `objects`; open geometry parses.
        scene(json!({ "objects": [ { "script": "let o = []; for k in 0..3 { o.push(#{ type: \"lathe\", points: [[0.0, 0.0], [0.2, 0.3], [0.0, 0.6]], pos: [k * 0.5, 0.0, 0.0] }); } o" } ] })).unwrap();
        let e = scene(json!({ "objects": [ { "type": "sphere", "material": "crome" } ] })).err().unwrap();
        assert!(e.contains("did you mean `chrome`"), "{e}");
        let e = scene(json!({ "objects": [ { "type": "spere" } ] })).err().unwrap();
        assert!(e.contains("did you mean `sphere`"), "{e}");
        let e = scene(json!({ "env": "cyk", "objects": [] })).err().unwrap();
        assert!(e.contains("did you mean `cyc`"), "{e}");
        let e = scene(json!({ "objects": [ { "type": "box", "postion": [0, 0, 0] } ] })).err().unwrap();
        assert!(e.contains("did you mean `position`"), "{e}");
        let e = scene(json!({ "objects": [], "camra": "orbit" })).err().unwrap();
        assert!(e.contains("did you mean `camera`"), "{e}");
    }

    #[test]
    fn instances_follow_expressions_and_keys() {
        let s = scene(json!({
            "objects": [
                { "type": "box", "count": 4, "lanes": 2, "pos": "=[1.7*i, 0.5, -4*lane]" },
                { "type": "sphere", "keys": [ { "t": 0, "pos": [0, 0, 0] }, { "t": 2, "pos": [2, 0, 0], "ease": "linear" } ] }
            ]
        }))
        .unwrap();
        let p = s.p();
        let insts = instances(&p.objs, 1.0, 0.0);
        assert_eq!(insts.len(), 9);
        assert!((insts[3].xf.t.x - 5.1).abs() < 1e-4);
        assert!((insts[4].xf.t.z + 4.0).abs() < 1e-4);
        assert!((insts[8].xf.t.x - 1.0).abs() < 1e-4, "linear key halfway");
    }

    #[test]
    fn imports_gltf_models() {
        let dir = std::env::temp_dir().join(format!("scene3d-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("tri.gltf");
        gltf::tests::write_triangle(&p);
        let s = scene(json!({ "objects": [ { "type": "model", "src": p.to_str().unwrap(), "height": 2.0, "count": 2, "pos": "=[i * 3, 0, 0]" } ] })).unwrap();
        let insts = instances(&s.p().objs, 0.0, 0.0);
        assert_eq!(insts.len(), 2);
        // glTF materials come through: red, non-metal.
        assert!(s.p().mats[insts[0].mat as usize].base.x > 0.99 && s.p().mats[insts[0].mat as usize].metallic == 0.0);
        // Node animation: the part moves from x = 0 to x = 2 over 1 s.
        let pa = dir.join("anim.gltf");
        gltf::tests::write_animated_triangle(&pa);
        let s = scene(json!({ "objects": [ { "type": "model", "src": pa.to_str().unwrap() } ] })).unwrap();
        assert!(s.p().objs[0].anim.is_some() && s.p().objs[0].dynamic());
        let a = instances(&s.p().objs, 0.0, 0.0)[0].xf.t.x;
        let b = instances(&s.p().objs, 0.5, 0.0)[0].xf.t.x;
        assert!((b - a - 1.0).abs() < 1e-3, "moved {}", b - a);
        let e = scene(json!({ "objects": [ { "src": "missing.glb" } ] })).err().unwrap();
        assert!(e.contains("can't read"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cards_scale_lights_and_texture_lists() {
        let s = scene(json!({
            "env": "cyc", "quality": "draft",
            "lights": [ { "dir": [1, 1, 1], "size": [10, 40], "intensity": 6 } ],
            "fog": { "density": 0.01, "color": "#ffffff" },
            "camera": { "keys": [ { "t": 0, "pos": [0, 1, 4], "target": [0, 0.5, 0], "fov": 40 } ] },
            "objects": [
                { "type": "card", "size": [6, 4], "pos": [0, 1, 2], "rot": [0, 180, 0], "color": "#000" },
                { "type": "box", "size": [1, 1, 1], "scale": [2, 0.5, 1], "pos": [0, 0.25, 0], "material": "matte",
                  "texture": [ { "composition": "a", "face": "front" }, { "composition": "b", "face": "back" } ] },
                { "type": "sphere", "scale": "=[1, 0, 1]" },
                { "text": "AK", "align": "left", "height": 0.3 }
            ]
        }))
        .unwrap();
        let p = s.p();
        let insts = instances(&p.objs, 0.0, 0.0);
        assert_eq!(insts.len(), 3, "the zero-scale sphere is hidden");
        assert!(!insts[0].cam && insts[1].cam, "cards are invisible to the camera");
        assert_eq!(p.objs[1].tex.len(), 2);
        assert!((insts[1].xf.s.x - 2.0).abs() < 1e-6 && (insts[1].xf.s.y - 0.5).abs() < 1e-6);
        assert_eq!(p.env.boxes.len(), env::Env::preset("cyc", v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)).unwrap().boxes.len() + 1);
        // The card covers the whole view but isn't seen: the box and cyc are.
        let px = s.render(0.0, 1.0 / 30.0, 64, 36, None);
        let mean = px.chunks_exact(4).map(|q| q[0] as f32).sum::<f32>() / (64.0 * 36.0);
        assert!(mean > 120.0, "card must not black out the frame: {mean}");
        let spot = scene(json!({ "objects": [], "lights": [ { "pos": [0, 3, 0], "target": [0, 0, 0], "angle": 40, "intensity": 20 }, { "pos": [1, 1, 1] } ] })).unwrap();
        assert_eq!(spot.p().lights.len(), 2);
        assert!(spot.p().lights[0].cos_outer > 0.9 && spot.p().lights[1].cos_outer == -1.0);
        let e = scene(json!({ "objects": [], "lights": [ { "dir": [0, 0, 0] } ] })).err().unwrap();
        assert!(e.contains("must not be zero"), "{e}");
    }

    #[test]
    fn camera_follows_named_objects() {
        let s = scene(json!({
            "quality": "draft",
            "camera": { "target": "car", "follow": true, "keys": [ { "t": 0, "pos": [0, 1, 5] } ] },
            "objects": [ { "type": "box", "name": "car", "pos": "=[2*t, 0.5, 0]" } ]
        }))
        .unwrap();
        let _ = s.render(1.0, 1.0 / 30.0, 32, 18, None);
        let e = scene(json!({ "camera": { "target": "cra", "keys": [ { "t": 0, "pos": [0, 1, 5] } ] }, "objects": [ { "type": "box", "name": "car" } ] })).err().unwrap();
        assert!(e.contains("did you mean `car`"), "{e}");
    }

    #[test]
    fn shorthand_expands_layers_and_scenes() {
        let v = shorthand(json!({ "scenes": [ { "duration": 2, "scene3d": { "env": "cyc" }, "layers": [ "Hello" ] }, { "layers": [ { "scene3d": { "env": "neon" }, "opacity": 0.5 } ] } ] }));
        assert_eq!(v["scenes"][0]["layers"][0]["component"], "scene3d");
        assert_eq!(v["scenes"][0]["layers"][0]["props"]["env"], "cyc");
        assert_eq!(v["scenes"][0]["layers"][1], "Hello");
        assert_eq!(v["scenes"][1]["layers"][0]["props"]["env"], "neon");
        assert_eq!(v["scenes"][1]["layers"][0]["opacity"], 0.5);
    }

    #[test]
    fn camera_lens_and_linear_track() {
        let cs = spec::camera(&json!({ "interp": "linear", "keys": [
            { "t": 0, "pos": [0, 1, 0], "target": [10, 1, 0], "focal": 35, "fstop": 2.8, "shutter": 0.5 },
            { "t": 6, "pos": [612, 1, 0], "target": [622, 1, 0], "focal": 35, "fstop": 2.8, "shutter": 0.0 } ] }), (v3(0.0, 0.0, 0.0), 1.0), "camera")
        .unwrap();
        let c = cs.at(3.0, 16.0 / 9.0, 0.0);
        assert!((c.pos.x - 306.0).abs() < 1e-3, "exactly linear: {}", c.pos.x);
        assert!((c.shutter - 0.25).abs() < 1e-4);
        // 35 mm on a 36 mm-wide sensor: about 54.4° horizontal.
        let hf = 2.0 * ((c.fov.to_radians() * 0.5).tan() * 16.0 / 9.0).atan();
        assert!((hf.to_degrees() - 54.43).abs() < 0.1, "{}", hf.to_degrees());
        assert!(c.aperture > 0.5, "f/2.8 at 10 m focus blurs the far field");
    }
}
