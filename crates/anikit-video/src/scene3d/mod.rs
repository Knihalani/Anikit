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
pub mod rig;
pub mod sets;
pub mod env;
pub mod geom;
pub mod gltf;
pub mod hdri;
pub mod pbr;
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

/// Run `f` (which may use rayon) while the caller holds a simulation lock
/// (particle stepping, rapier, building particle sims), without letting the
/// caller run other work meanwhile. A rayon worker that waits on its own
/// pool, or even in `install` on another pool, steals other frames' tasks,
/// and one of those can need the lock it holds: a deadlock. So `f` runs on a
/// separate pool, started from a plain scoped thread that the caller simply
/// joins (blocking, not stealing). Same thread count as the global pool,
/// so results match.
pub(crate) fn sim_blocking<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    std::thread::scope(|s| s.spawn(|| sim_pool().install(f)).join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
}

fn sim_pool() -> &'static rayon::ThreadPool {
    static P: OnceLock<rayon::ThreadPool> = OnceLock::new();
    P.get_or_init(|| rayon::ThreadPoolBuilder::new().num_threads(rayon::current_num_threads()).thread_name(|i| format!("anikit-sim-{i}")).build().expect("scene3d: can't start the simulation thread pool"))
}

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
    "text", "font", "weight", "env", "material", "primitives", "camera", "objects", "materials", "floor", "aperture", "shutter", "bloom", "exposure", "tonemap", "quality", "resolution", "vignette", "denoise", "lights", "fog", "physics", "glass_layers", "look", "outline", "rig", "set",
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
    /// Underwater caustics strength (0 = off).
    caustics: f32,
    gobo: rig::Gobo,
    /// An animated rig: (rig, the env it was applied to, camera view, key level).
    rig_anim: Option<Arc<(rig::Rig, env::Env, V3, f32)>>,
    /// Ink outlines: (width in px at 1080p, colour sRGB).
    outline: Option<(f32, [u8; 3])>,
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
    /// PBR texture set (`floor.texture`), 0 = none.
    pbr: u32,
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
        let bright = matches!(es.name.as_str(), "cyc" | "sunset" | "overcast" | "interior" | "underwater");

        let mut named: HashMap<String, Material> = HashMap::new();
        if let Some(ms) = m.get("materials") {
            let o = ms.as_object().ok_or_else(|| Error("`materials` must be an object of name → material".into()))?;
            for (name, v) in o {
                let mat = spec::material(v, &named, &format!("materials.{name}")).map_err(Error)?;
                named.insert(name.clone(), mat);
            }
        }
        let objs_v: Vec<Value> = match m.get("objects") {
            // A physics-only scene is not the lookdev demo: no stray "ANIKIT" wordmark.
            None if m.contains_key("physics") => Vec::new(),
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
        // `look`: a style for the whole scene: the default material of every
        // object without one, plus ink outlines for the graphic looks.
        let look = m.get("look").and_then(|l| l.as_str()).map(crate::components::kebab);
        let (look_mat, look_outline) = match look.as_deref() {
            None | Some("photo") | Some("pbr") => (None, false),
            Some("toon") | Some("cel") => (Some("toon"), true),
            Some("ink") => (Some("hatch"), true),
            Some("hatch") => (Some("hatch"), true),
            Some("clay") | Some("plasticine") => (Some("clay"), false),
            Some("paper") => (Some("paper"), true),
            Some("cardboard") => (Some("cardboard"), false),
            Some("gouache") | Some("painted") => (Some("gouache"), false),
            Some(o) => {
                let names = ["photo", "toon", "ink", "hatch", "clay", "paper", "cardboard", "gouache"];
                let hint = crate::components::style::closest(o, &names).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(Error(format!("look: unknown look `{o}`{hint}")));
            }
        };
        let objs_v: Vec<Value> = match look_mat {
            Some(lm) => objs_v
                .into_iter()
                .map(|mut o| {
                    if let Value::Object(om) = &mut o {
                        let card = om.get("type").and_then(|t| t.as_str()) == Some("card");
                        if !card && !om.contains_key("material") {
                            // `color` tints the look's material.
                            let mat = match om.remove("color") {
                                Some(c) => serde_json::json!({ "extends": lm, "base": c }),
                                None => Value::String(lm.into()),
                            };
                            om.insert("material".into(), mat);
                        }
                    }
                    o
                })
                .collect(),
            None => objs_v,
        };
        // `set`: a stage built around the subject (plinth, steps, field, ...).
        // (Built after the objects below, so a plinth fits the subject.)
        let mut set: Option<sets::Built> = None;
        let outline = match m.get("outline") {
            None if look_outline => Some((1.6, [22u8, 20, 24])),
            None | Some(Value::Bool(false)) => None,
            Some(Value::Bool(true)) => Some((1.6, [22u8, 20, 24])),
            Some(Value::Number(n)) => Some((n.as_f64().unwrap_or(1.6) as f32, [22u8, 20, 24])),
            Some(Value::Object(o)) => {
                let w = num_or(o, "width", 1.6).map_err(Error)?;
                let c = o.get("color").map(|c| spec::color(c, "outline.color")).transpose().map_err(Error)?.unwrap_or(v3(0.008, 0.007, 0.009));
                let g = |x: f32| (x.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8;
                Some((w, [g(c.x), g(c.y), g(c.z)]))
            }
            Some(_) => return Err(Error("`outline` must be true, a width in px, or { width, color }".into())),
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
        if let Some(sv) = m.get("set") {
            // The subject's footprint (half-width on the floor) at t = 0.
            let mut probe = Scene::new(shapes.list.clone(), mats.clone(), Vec::new(), es.env.clone(), None);
            probe.insts = instances(&objs, 0.0, 0.0);
            let mut fit: f32 = 0.0;
            for i in &probe.insts {
                let (a, b) = probe.world_box(i);
                fit = fit.max(a.x.abs()).max(b.x.abs()).max(a.z.abs()).max(b.z.abs());
            }
            let built = sets::build(sv, "set", (fit > 0.0).then_some(fit)).map_err(Error)?;
            for (k, o) in built.objects.iter().enumerate() {
                objs.push(spec::object(o, &mut shapes, &mut mats, &named, &format!("set[{k}]")).map_err(Error)?);
            }
            set = Some(built);
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
            None if set.as_ref().is_some_and(|b| b.no_floor) => None,
            v => {
                let mut fp = if es.name == "hdri" {
                    // A ground that catches shadows, with the albedo that makes it
                    // match the HDRI's own ground under the HDRI's light, so it
                    // melts into the photographed horizon.
                    let up = v3(0.0, 1.0, 0.0);
                    let lit = add(es.env.diffuse(up), es.env.key_box().map(|k| scale(k.radiance, es.env.key_solid_angle * dot(up, k.c).max(0.0) / std::f32::consts::PI)).unwrap_or(v3(0.0, 0.0, 0.0)));
                    let g = es.env.map.as_ref().map(|m| m.ground()).unwrap_or(v3(0.1, 0.1, 0.1));
                    let alb = |a: f32, b: f32| (a / b.max(1e-5)).clamp(0.03, 0.85);
                    let color = v3(alb(g.x, lit.x), alb(g.y, lit.y), alb(g.z, lit.z));
                    FloorParams { y: 0.0, color, roughness: 0.6, mirror: 0.0, reflect: 0.12, pool: (0.0, 0.0, 0.0), fade: (4.0, 18.0), pbr: 0 }
                } else if bright {
                    let color = match es.name.as_str() {
                        "overcast" => v3(0.2, 0.27, 0.12),
                        "interior" => v3(0.32, 0.19, 0.1),
                        "underwater" => v3(0.5, 0.45, 0.33),
                        _ => scale(es.tint, 0.92),
                    };
                    let (roughness, reflect) = match es.name.as_str() {
                        "overcast" | "underwater" => (0.9, 0.0),
                        "interior" => (0.3, 0.5),
                        _ => (0.35, 0.35),
                    };
                    FloorParams { y: 0.0, color, roughness, mirror: 0.0, reflect, pool: (0.0, 0.0, 0.0), fade: (25.0, 250.0), pbr: 0 }
                } else {
                    FloorParams { y: 0.0, color: v3(0.035, 0.035, 0.04), roughness: 0.09, mirror: 0.06, reflect: 1.0, pool: (0.0, 0.0, 5.0), fade: (7.0, 16.0), pbr: 0 }
                };
                if let Some(y) = set.as_ref().and_then(|b| b.floor_y) {
                    fp.y = y;
                }
                if let Some(Value::Object(fm)) = v {
                    for k in fm.keys() {
                        if !["color", "roughness", "reflect", "mirror", "y", "texture", "scale", "texture_rotate"].contains(&k.as_str()) {
                            return Err(Error(format!("floor: unknown key `{k}` (color, roughness, reflect, mirror, y, texture, scale, textureRotate)")));
                        }
                    }
                    if let Some(t) = fm.get("texture") {
                        // A ground material: the set's albedo times `color` (white unless given).
                        let src = t.as_str().ok_or_else(|| Error("floor.texture: expected an asset ref (`ambientcg:WoodFloor051`) or a folder of maps".into()))?;
                        let o = pbr::SetOpts {
                            scale: num_or(fm, "scale", 1.0).map_err(Error)?.max(1e-3),
                            tint: v3(1.0, 1.0, 1.0),
                            mapping: pbr::Mapping::Triplanar,
                            rough_mul: 1.0,
                            normal_strength: 1.0,
                            keep_rough: false,
                            keep_metal: true,
                            rotate: num_or(fm, "texture_rotate", 0.0).map_err(Error)?,
                            max: 2048,
                        };
                        fp.pbr = pbr::load(src, &o).map_err(|e| Error(format!("floor.texture: {e}")))?;
                        if !fm.contains_key("color") {
                            fp.color = v3(1.0, 1.0, 1.0);
                        }
                        if !fm.contains_key("reflect") {
                            fp.reflect = 0.6;
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
        // `rig`: a lighting setup placed around the camera's view at t = 0.
        let rig = m.get("rig").map(|r| rig::parse(r, "rig")).transpose().map_err(Error)?;
        let mut gobo = rig::Gobo::default();
        let mut rig_anim_base: Option<(env::Env, V3, f32)> = None;
        if let Some(r) = &rig {
            let c0 = camera.at(0.0, 16.0 / 9.0, 0.0);
            let view = norm(sub(c0.target, c0.pos));
            // Keep the env's key level so exposure and the look stay calibrated.
            let level = es.env.key_box().map(|k| luma(k.radiance) * es.env.key_solid_angle * (1.0 - k.falloff / 3.0) / std::f32::consts::PI).unwrap_or(1.0).max(0.2);
            let (e, g) = r.apply(&es.env, view, level);
            if r.anim != 0 {
                rig_anim_base = Some((es.env.clone(), view, level));
            }
            es.env = e;
            gobo = g;
        }
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
        // Non-studio environments: a sky, a room wall, open water. Their floors
        // are dark (grass, wood, sand), so exposure follows the light, not the floor.
        let floor_l = floor.as_ref().map(|f| luma(f.color)).unwrap_or(0.8).max(0.02);
        let (backdrop, haze, auto_exp) = match es.name.as_str() {
            "overcast" | "interior" | "underwater" => {
                let irr = luma(backdrop.0).max(1e-4) / floor_l;
                let e = match es.name.as_str() { "overcast" => 1.15, "interior" => 0.8, _ => 0.62 } / irr;
                let at = |c: V3, l: f32| scale(c, l / luma(c).max(1e-4) / e);
                match es.name.as_str() {
                    "overcast" => {
                        let sky = (at(v3(0.86, 0.88, 0.9), 0.9), at(v3(0.62, 0.7, 0.8), 0.72));
                        (sky, (0.012, sky.0), e)
                    }
                    "interior" => ((at(v3(0.9, 0.86, 0.8), 0.62), at(v3(0.86, 0.82, 0.76), 0.55)), (0.0, backdrop.0), e),
                    _ => {
                        let deep = at(v3(0.05, 0.38, 0.46), 0.3);
                        ((deep, at(v3(0.14, 0.62, 0.68), 0.55)), (0.09, deep), e)
                    }
                }
            }
            "city" => (backdrop, (0.012, v3(0.01, 0.012, 0.03)), auto_exp),
            "hdri" => {
                // HDRIs are scene-referred at whatever level they were shot:
                // expose so a white card facing a typical direction (the mean
                // over eight around-the-horizon normals, tilted up 20°)
                // reads light grey. `env.exposure` offsets this in stops.
                let kd = |n: V3| {
                    es.env.key_box().map(|k| scale(k.radiance, es.env.key_solid_angle * dot(n, k.c).max(0.0) * (1.0 - k.falloff / 3.0) / std::f32::consts::PI)).unwrap_or(v3(0.0, 0.0, 0.0))
                };
                // (Up itself is left out: an overhead noon sun would expose
                // for the tops of things and leave every face in the dark.)
                let mut irr = 0.0;
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::TAU / 8.0;
                    let n = norm(v3(a.cos(), 0.36, a.sin()));
                    irr += luma(add(es.env.diffuse(n), kd(n)));
                }
                irr /= 8.0;
                let e = 0.9 / irr.max(1e-4);
                (backdrop, (es.haze, es.env.map.as_ref().map(|m| m.mean()).unwrap_or(v3(0.5, 0.5, 0.5))), e)
            }
            _ => (backdrop, haze, auto_exp),
        };
        let caustics = if es.name == "underwater" { 1.0 } else { 0.0 };
        // Night city: coloured practicals around the subject unless lights are given.
        if es.name == "city" && local_lights.is_empty() {
            let (c, r) = bounds;
            let r = r.max(0.5);
            for (k, (dx, dy, dz, col)) in [(-1.6, 1.1, 0.6, v3(3.0, 0.35, 2.0)), (1.7, 1.4, 0.2, v3(0.3, 2.2, 3.4)), (0.4, 2.2, -1.8, v3(3.2, 1.7, 0.5)), (-0.8, 0.5, -1.4, v3(3.0, 0.3, 0.3))].into_iter().enumerate() {
                let _ = k;
                local_lights.push(trace::Light { pos: add(c, v3(dx * r, dy * r, dz * r)), power: scale(col, r * r * 2.2), dir: v3(0.0, -1.0, 0.0), cos_outer: -1.0, cos_inner: -1.0, shadow: true });
            }
        }
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
        // A rig with `beam`: haze so the key reads as a volumetric beam.
        let haze = if rig.as_ref().is_some_and(|r| r.beam) && !m.contains_key("fog") {
            shafts = true;
            (0.04, if bright { haze.1 } else { v3(0.004, 0.004, 0.005) })
        } else {
            haze
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
        let floor_vis = if floor.is_some() && total <= 400 && objs.iter().all(|o| !o.dynamic()) && rig_anim_base.is_none() {
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
            caustics,
            gobo,
            rig_anim: match (rig, rig_anim_base) {
                (Some(r), Some((e, v, l))) => Some(Arc::new((r, e, v, l))),
                _ => None,
            },
            outline,
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
    Floor { y: f.y, color: f.color, roughness: f.roughness, mirror: f.mirror, reflect: f.reflect, pool: f.pool, fade: f.fade, vis, pbr: f.pbr }
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

/// Look warnings for `check`: combinations that render, but badly.
pub fn lint(props: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let env = match props.get("env") {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Object(m)) if m.contains_key("hdri") => "hdri",
        Some(Value::Object(m)) => m.get("preset").and_then(|p| p.as_str()).unwrap_or("cyc"),
        _ => "cyc",
    };
    let env = if crate::assetlib::is_ref(env) || env.ends_with(".hdr") { "hdri" } else { env };
    let objs = props.get("objects").and_then(|o| o.as_array()).cloned().unwrap_or_default();
    let mat_of = |o: &Value| -> String {
        match o.get("material").or(props.get("material")) {
            Some(Value::String(s)) => s.to_lowercase(),
            Some(Value::Object(m)) => m.get("extends").and_then(|e| e.as_str()).unwrap_or("").to_lowercase(),
            _ => if o.get("type").and_then(|t| t.as_str()).is_some_and(|t| t == "slab" || t == "pane") { "glass".into() } else { "chrome".into() },
        }
    };
    let warm = ["gold", "copper"];
    if matches!(env, "sunset" | "neon") {
        for (i, o) in objs.iter().enumerate() {
            let m = mat_of(o);
            if warm.contains(&m.as_str()) {
                out.push((format!(".objects[{i}].material"), format!("`{m}` under env `{env}` washes out into the coloured light; use `chrome` or `aluminium`, or env `studio`/`night` for warm metals")));
            }
        }
    }
    if env == "cyc" && props.get("rig").is_none() && props.get("look").and_then(|l| l.as_str()).is_none_or(|l| l == "photo" || l == "pbr") {
        let dark_card = objs.iter().any(|o| o.get("type").and_then(|t| t.as_str()) == Some("card") && o.get("intensity").and_then(|k| k.as_f64()).is_some_and(|k| k < 0.2));
        let chrome = objs.iter().position(|o| matches!(mat_of(o).as_str(), "chrome" | "mirror") && o.get("type").and_then(|t| t.as_str()) != Some("card"));
        if let (Some(i), false) = (chrome, dark_card) {
            out.push((format!(".objects[{i}].material"), "chrome on the white `cyc` reflects only white and reads flat; add a black card ({ \"type\": \"card\", \"color\": \"#000\", \"intensity\": 0, \"pos\": [0, 0.5, 2.5], \"rot\": [0, 180, 0] }) or use env `studio`".into()));
        }
    }
    look_lint(props, &objs, &mut out);
    out
}

/// Craft checks (docs/video/craft/3d-design.md §6): the tells that make CG read
/// cheap, caught from the spec alone.
fn look_lint(props: &Value, objs: &[Value], out: &mut Vec<(String, String)>) {
    let num = |v: Option<&Value>| v.and_then(|x| x.as_f64()).map(|x| x as f32);
    let has_rig = props.get("rig").is_some();
    let has_lights = props.get("lights").and_then(|l| l.as_array()).is_some_and(|a| !a.is_empty());
    let has_metal_or_glass = objs.iter().any(|o| {
        let m = o.get("material").map(|m| m.to_string().to_lowercase()).unwrap_or_default();
        ["chrome", "mirror", "glass", "aluminium", "gold", "copper", "titanium"].iter().any(|k| m.contains(k))
    });
    if !has_rig && !has_lights && !objs.is_empty() && props.get("env").is_none() {
        let hint = if has_metal_or_glass { "`product` (strips and flags draw edges on metal and glass)" } else { "`rembrandt` (4:1) or `loop` (3:1)" };
        out.push(("".into(), format!("default lighting reads flat and generic; set a `rig`, e.g. {hint}")));
    }
    // Camera.
    let cam = props.get("camera");
    if let Some(Value::Object(c)) = cam {
        if let Some(p) = c.get("preset").and_then(|p| p.as_str()) {
            let deg = num(c.get("degrees")).unwrap_or(70.0);
            let dur = num(c.get("duration")).unwrap_or(5.0).max(0.1);
            if p == "orbit" && deg / dur > 20.0 {
                out.push((".camera".into(), format!("orbit at {:.0}°/s feels like a turntable demo; keep it under 15°/s (e.g. degrees {:.0} over {dur} s)", deg / dur, 15.0 * dur)));
            }
        }
        if let Some(keys) = c.get("keys").and_then(|k| k.as_array()) {
            if c.get("interp").and_then(|i| i.as_str()) == Some("linear") && keys.len() <= 3 && c.get("ease").and_then(|e| e.as_bool()) != Some(true) {
                out.push((".camera.interp".into(), "a linear move starts and stops dead; use `smooth` (eased) unless it's a dense tracked path".into()));
            }
            let still = keys.len() == 1 || keys.windows(2).all(|w| w[0].get("pos") == w[1].get("pos") && w[0].get("target") == w[1].get("target"));
            if still && c.get("shake").is_none() {
                out.push((".camera.shake".into(), "a locked-off CG camera looks dead; add `\"shake\": \"breath\"` (0.15% of the frame)".into()));
            }
            for (i, k) in keys.iter().enumerate() {
                if num(k.get("fstop")).is_some_and(|n| n < 1.4) {
                    out.push((format!(".camera.keys[{i}].fstop"), "below f/1.4 everything but a sliver is mush; product work sits at f/2.8–8".into()));
                }
                if num(k.get("focal")).is_some_and(|f| f < 20.0) {
                    out.push((format!(".camera.keys[{i}].focal"), "under 20 mm distorts products; use 24 only for scale shots, 50–135 for heroes".into()));
                }
            }
        }
    }
    if num(props.get("bloom")).is_some_and(|b| b > 0.15) {
        out.push((".bloom".into(), "heavy bloom is the classic cheap-CG tell; keep it under 0.1 and let emissives carry it".into()));
    }
    // Floating spheres (an easy, common mistake: contact sells the scale).
    for (i, o) in objs.iter().enumerate() {
        if o.get("type").and_then(|t| t.as_str()) == Some("sphere") && o.get("keys").is_none() && o.get("count").is_none() {
            let r = num(o.get("radius")).unwrap_or(0.5);
            if let Some(y) = o.get("pos").and_then(|p| p.as_array()).and_then(|a| a.get(1)).and_then(|y| y.as_f64()) {
                if (y as f32) - r > 0.03 && (y as f32) - r < 1.0 {
                    out.push((format!(".objects[{i}].pos"), format!("the sphere floats {:.2} m above the floor; set y = radius ({r}) for contact, or lift it clearly", y as f32 - r)));
                }
            }
        }
    }
    // Palette: more than three hues of colour fight each other.
    let mut hues: Vec<f32> = Vec::new();
    for o in objs {
        let c = o.get("color").or_else(|| o.get("material").and_then(|m| m.get("base"))).and_then(|c| c.as_str());
        if let Some(c) = c.and_then(|c| spec::color(&Value::String(c.into()), "").ok()) {
            let (mx, mn) = (c.x.max(c.y).max(c.z), c.x.min(c.y).min(c.z));
            if mx - mn > 0.08 * mx.max(1e-3) && mx > 0.02 {
                let h = if mx == c.x { ((c.y - c.z) / (mx - mn)).rem_euclid(6.0) } else if mx == c.y { (c.z - c.x) / (mx - mn) + 2.0 } else { (c.x - c.y) / (mx - mn) + 4.0 } * 60.0;
                if !hues.iter().any(|&q| ((q - h + 540.0) % 360.0 - 180.0).abs() < 25.0) {
                    hues.push(h);
                }
            }
        }
    }
    if hues.len() > 3 {
        out.push((".objects".into(), format!("{} distinct hues; studio palettes hold to one or two plus neutrals, with the accent on the hero only", hues.len())));
    }
}

/// Ink outlines from the primary pass: silhouettes (object id), creases
/// (normals) and depth steps, thicker for near objects, anti-aliased by
/// coverage. The floor and backdrop never draw lines between themselves.
fn outlines(out: &mut [u8], px: &post::Planes, w: usize, h: usize, width: f32, ink: [u8; 3]) {
    let scale_px = h as f32 / 1080.0;
    let bg = |id: u32| id >= u32::MAX - 2;
    let n_at = |i: usize| v3(px.nx[i], px.ny[i], px.nz[i]);
    // Median depth of objects sets "near" (thicker) and "far" (thinner) lines.
    let mut ds: Vec<f32> = (0..w * h).step_by(97).filter(|&i| !bg(px.id[i])).map(|i| px.depth[i]).collect();
    ds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let dmed = ds.get(ds.len() / 2).copied().unwrap_or(5.0).max(0.1);
    let edge = |i: usize, j: usize| -> bool {
        let (a, b) = (px.id[i], px.id[j]);
        if bg(a) && bg(b) {
            return false;
        }
        if a != b {
            return true;
        }
        let (da, db) = (px.depth[i], px.depth[j]);
        if (da - db).abs() > 0.04 * da.min(db) {
            return true;
        }
        dot(n_at(i), n_at(j)) < 0.6
    };
    use rayon::prelude::*;
    // Edge pixels first (each pixel tests its right and lower neighbour), then
    // a distance-limited dilation over that mask.
    let mut mask = vec![0u8; w * h];
    mask.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            let mut e = (x + 1 < w && edge(i, i + 1)) || (y + 1 < h && edge(i, i + w));
            e |= (x > 0 && edge(i, i - 1)) || (y > 0 && edge(i, i - w));
            // Keep the line on the nearer side of a depth step.
            if e && !bg(px.id[i]) {
                row[x] = 1;
            } else if e {
                row[x] = 2;
            }
        }
    });
    let mut cov = vec![0f32; w * h];
    cov.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            let d = if bg(px.id[i]) { dmed } else { px.depth[i] };
            let r = (width * scale_px * (dmed / d.max(0.05)).sqrt().clamp(0.6, 1.8)).max(0.6) * 0.65;
            let ri = r.ceil() as i32;
            let mut best = f32::MAX;
            for dy in -ri..=ri {
                let yy = y as i32 + dy;
                if yy < 0 || yy >= h as i32 {
                    continue;
                }
                for dx in -ri..=ri {
                    let xx = x as i32 + dx;
                    if xx < 0 || xx >= w as i32 || mask[yy as usize * w + xx as usize] == 0 {
                        continue;
                    }
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    best = best.min(dist);
                }
            }
            row[x] = if best == f32::MAX { 0.0 } else { (r + 0.5 - best).clamp(0.0, 1.0) };
        }
    });
    for (i, &c) in cov.iter().enumerate() {
        if c > 0.0 {
            for k in 0..3 {
                let o = &mut out[i * 4 + k];
                *o = (*o as f32 + (ink[k] as f32 - *o as f32) * c).round() as u8;
            }
        }
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
        // An animated rig rebuilds the lights for this frame (before the scene,
        // which derives its floor lighting from them).
        let env_t = match &p.rig_anim {
            Some(ra) => ra.0.apply_at(&ra.1, ra.2, ra.3, t).0,
            None => p.env.clone(),
        };
        let mut scene = Scene::new(shapes, p.mats.clone(), Vec::new(), env_t, p.floor.as_ref().map(|f| make_floor(f, p.floor_vis.clone())));
        scene.backdrop = p.backdrop;
        scene.glow = p.glow;
        scene.haze = p.haze;
        scene.lights = p.lights.clone();
        scene.caustics = p.caustics;
        scene.gobo = p.gobo;
        scene.time = t;
        // One pixel's angle: picks texture mips (vertical fov over output rows).
        scene.texel_angle = (2.0 * (cam.fov.to_radians() * 0.5).tan() / h.max(1) as f32).max(1e-5);
        let mut insts = instances(&p.objs, t, dt);
        // Textures need world boxes, so build the hierarchy first.
        scene.insts = std::mem::take(&mut insts);
        // Focus pulls onto named objects (rack focus between keys).
        if p.camera.focuses_on_objects() {
            let centre_of = |name: &str, prev: bool| -> Option<V3> {
                let oi = p.objs.iter().position(|o| o.name.as_deref() == Some(name))?;
                let inst = scene.insts.iter().find(|i| i.obj as usize == oi)?;
                let mut i2 = inst.clone();
                if prev {
                    i2.xf = inst.prev;
                }
                let (lo, hi) = scene.world_box(&i2);
                Some(scale(add(lo, hi), 0.5))
            };
            let shutter = cam.shutter;
            cam = p.camera.at_with(t, aspect, p.default_aperture, &|n| centre_of(n, false));
            cam.shutter = shutter;
            prev_cam = p.camera.at_with(t - dt, aspect, p.default_aperture, &|n| centre_of(n, true));
        }
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

        let mut px = post::Planes::new(w * h);
        let mut q = p.quality;
        // Converged sampling is opt-in (scene3d_bench --reference). qbench's
        // reference mode doesn't switch it on: its committed baseline was
        // measured against standard sampling (see PERF before changing this).
        if std::env::var_os("ANIKIT_SCENE3D_REFERENCE").is_some() {
            // Converged sampling with the same image model: 1 + 24 samples per
            // pixel everywhere, 4 shadow and AO rays per sample.
            q.full = true;
            q.edge_spp = 24;
            q.ao_rays = q.ao_rays.max(4);
            q.shadow_rays = q.shadow_rays.max(4);
        }
        // Drafts (`quality: draft` or `--quality draft`) are for iterating on
        // composition, light direction and colour: one sample per pixel (no
        // edge supersampling), one traced reflection bounce, few glass layers.
        // Standard and high quality are untouched.
        let draft = (p.draft || crate::components::draft()) && !q.full;
        if draft {
            q.edge_spp = 0;
            q.max_depth = q.max_depth.min(1);
            q.max_glass = q.max_glass.min(4);
            q.ao_rays = q.ao_rays.min(1);
            q.shadow_rays = q.shadow_rays.min(1);
            q.secondary_rays = 0;
        }
        // Trace, particles and post run on the render pool (see `kernel::par::isolated`).
        let (out, t_trace, t_parts) = kernel::par::isolated(|| {
            scene.render_frame(&cam, &prev, w, h, &q, &mut px);
            let t_trace = clock.elapsed();
            let layer = self.particles(t, dt, &cam, &prev, &mut px, w, h, assets);
            let t_parts = clock.elapsed();
            let mut pp = post::Post { focus: cam.focus, aperture: cam.aperture, shutter: cam.shutter, bloom: p.bloom, exposure: p.exposure, vignette: p.vignette, tonemap: p.tonemap, denoise: p.denoise };
            if draft {
                // Drafts: a lighter denoise and no motion blur (focus blur stays: it is composition).
                pp.denoise = pp.denoise.min(2);
                pp.shutter = 0.0;
            }
            if let Some(path) = std::env::var_os("ANIKIT_SCENE3D_DUMP") {
                // Development aid: the primary pass and post settings, for examples/post_bench.
                post::dump(std::path::Path::new(&path), &px, w, h, &pp);
            }
            let out = {
                let vol = if p.shafts && p.haze.0 > 0.0 {
                    let depth: &[f32] = &px.depth;
                    let steps = match p.quality.edge_spp { 0..=3 => 8, 4..=8 => 16, _ => 32 };
                    Some(scene.volumetrics(&cam, w, h, depth, p.haze.0, steps))
                } else {
                    None
                };
                let mut out = post::process_planes(&px, w, h, &pp, vol.as_ref().map(|v| post::Volume { px: v, w: w.div_ceil(2), h: h.div_ceil(2) }), layer.as_ref());
                if let Some((width, ink)) = p.outline {
                    outlines(&mut out, &px, w, h, width, ink);
                }
                out
            };
            (out, t_trace, t_parts)
        });
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
    fn particles(&self, t: f32, dt: f32, cam: &trace::Camera, prev: &trace::Camera, px: &mut post::Planes, w: usize, h: usize, assets: Option<&dyn Assets>) -> Option<particles::Layer> {
        let p = self.p();
        if p.parts.is_empty() {
            return None;
        }
        let sims = p.psims.get_or_init(|| sim_blocking(|| {
            let floor_y = p.floor.as_ref().map(|f| f.y);
            let colliders: Vec<spec::ObjDef> = p.objs.iter().filter(|o| o.collide && o.phys.is_none()).cloned().collect();
            p.parts
                .iter()
                .map(|d| {
                    particles::Sim::new(d, 1.0 / dt.max(1e-4), floor_y, assets).map(|mut s| {
                        s.set_colliders(colliders.clone());
                        // Small previews simulate an exact subset of the particles,
                        // so each pixel sees about the density it would at 1080p.
                        if h <= 400 {
                            s.set_stride(((1080.0 / h.max(1) as f32).powi(2) as usize).clamp(1, 16));
                        }
                        s
                    })
                })
                .collect()
        }));
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
        let up = v3(0.0, 1.0, 0.0);
        let key = p.env.key_box().map(|k| (k.c, scale(k.radiance, p.env.key_solid_angle))).unwrap_or((up, v3(0.0, 0.0, 0.0)));
        // Particle shadows on the scene: dim the key light by the particles' transmittance.
        let c0 = std::time::Instant::now();
        let shadow = particles::ShadowMap::build(&live, key.0, t);
        let c1 = c0.elapsed();
        if let Some(sm) = &shadow {
            let (fwd, _, _) = cam.basis();
            let rig = trace::Rig::new(cam, w, h);
            let depth = &px.depth;
            kernel::par::rows(&mut px.vk, w, |y, row| {
                for (x, vk) in row.iter_mut().enumerate() {
                    let z = depth[y * w + x];
                    if z > 1e5 || *vk <= 0.0 {
                        continue;
                    }
                    let d = rig.ray(x as f32 + 0.5, y as f32 + 0.5);
                    let wp = madd(cam.pos, d, z / dot(d, fwd).max(1e-4));
                    *vk *= sm.trans(wp);
                }
            });
        }
        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() && shadow.is_some() {
            eprintln!("  particle shadows: map {:.0} ms, scene lookups {:.0} ms", c1.as_secs_f64() * 1e3, (c0.elapsed() - c1).as_secs_f64() * 1e3);
        }
        let view = particles::View { cam, prev, w, h, depth: &px.depth, focus: cam.focus, aperture: cam.aperture, shutter: cam.shutter, key, amb: (p.env.diffuse(up), p.env.diffuse(scale(up, -1.0))), shadow: shadow.as_ref(), draft: p.draft };
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
        // full size at standard/high quality and 0.3 of it in drafts (`--quality draft`,
        // one sample per pixel: composition, light and colour), so iterating is fast and
        // final renders lose nothing.
        let default_res = if crate::components::draft() || p.draft { 0.3 } else { 1.0 };
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
    fn camera_lens_shake_and_rack_focus() {
        let b = (v3(0.0, 0.5, 0.0), 1.0);
        // A lens on a preset keeps the framing: longer lens, further away.
        let c50 = spec::camera(&json!({ "preset": "static", "lens": "normal50" }), b, "camera").unwrap();
        let c135 = spec::camera(&json!({ "preset": "static", "lens": "tele135" }), b, "camera").unwrap();
        let d = |c: &spec::CamSpec| len(sub(c.keys[0].pos, c.keys[0].target));
        assert!(d(&c135) > d(&c50) * 2.3, "{} {}", d(&c135), d(&c50));
        // Handheld noise: deterministic, small (well under 5% of the frame), and alive.
        let h = spec::camera(&json!({ "preset": "static", "shake": "handheld" }), b, "camera").unwrap();
        let (a0, a1) = (h.at(1.0, 16.0 / 9.0, 0.0), h.at(1.0, 16.0 / 9.0, 0.0));
        assert_eq!(a0.target.x.to_bits(), a1.target.x.to_bits());
        let base = c50.at(1.0, 16.0 / 9.0, 0.0);
        let _ = base;
        let moved: Vec<f32> = (0..20).map(|k| len(sub(h.at(k as f32 * 0.1, 16.0 / 9.0, 0.0).target, h.keys[0].target))).collect();
        let dist = len(sub(h.keys[0].pos, h.keys[0].target));
        assert!(moved.iter().any(|&m| m > 1e-4) && moved.iter().all(|&m| m / dist < 0.05), "{moved:?}");
        // Rack focus between two named objects.
        let r = spec::camera(&json!({ "keys": [ { "t": 0, "pos": [0, 1, 5], "target": [0, 0.5, 0], "focus": "near", "fstop": 2 }, { "t": 1, "pos": [0, 1, 5], "target": [0, 0.5, 0], "focus": "far", "fstop": 2 } ] }), b, "camera").unwrap();
        assert!(r.focuses_on_objects());
        let res = |n: &str| match n { "near" => Some(v3(0.0, 1.0, 3.0)), _ => Some(v3(0.0, 1.0, -5.0)) };
        let (f0, f1) = (r.at_with(0.0, 16.0 / 9.0, 0.0, &res).focus, r.at_with(1.0, 16.0 / 9.0, 0.0, &res).focus);
        assert!((f0 - 2.0).abs() < 1e-3 && (f1 - 10.0).abs() < 1e-3, "{f0} {f1}");
        assert!(spec::camera(&json!({ "preset": "static", "shake": "handheldd" }), b, "camera").err().unwrap().contains("did you mean `handheld`"));
    }

    #[test]
    fn errors_suggest_fixes() {
        let m = spec::material(&json!({ "extends": "chrome", "imperfections": 0.5, "dust": 0.2 }), &HashMap::new(), "m").unwrap();
        assert_eq!(m.imperf, [0.5, 0.3, 0.25, 0.2]);
        scene(json!({ "objects": [ { "type": "box", "material": { "extends": "lacquer", "smudges": 0.8, "scratches": 0.5 } } ] })).unwrap();
        for set in ["plinth", "podium", "steps", "sweep", "field", "arches", "terrain"] {
            scene(json!({ "set": set, "objects": [ { "type": "sphere", "radius": 0.3, "pos": [0, 0.3, 0] } ] })).unwrap();
        }
        let e = scene(json!({ "set": "plint", "objects": [] })).err().unwrap();
        assert!(e.contains("did you mean `plinth`"), "{e}");
        for look in ["toon", "ink", "clay", "paper", "cardboard", "gouache"] {
            scene(json!({ "look": look, "objects": [ { "type": "sphere", "color": "#E4572E" } ] })).unwrap();
        }
        for env in ["overcast", "interior", "city", "underwater"] {
            scene(json!({ "env": env, "objects": [ { "type": "sphere", "material": "clay" } ] })).unwrap();
        }
        let e = scene(json!({ "look": "tooon", "objects": [] })).err().unwrap();
        assert!(e.contains("did you mean `toon`"), "{e}");
        assert_eq!(lint(&json!({ "env": "sunset", "objects": [ { "text": "A", "material": "gold" } ] })).len(), 1);
        assert_eq!(lint(&json!({ "env": "cyc", "objects": [ { "text": "A" } ] })).len(), 1, "chrome on cyc without a flag");
        assert!(lint(&json!({ "env": "cyc", "objects": [ { "text": "A" }, { "type": "card", "intensity": 0 } ] })).is_empty());
        // Craft checks: default light, turntable orbit, dead camera, floating sphere, palette, bloom.
        let w = lint(&json!({ "objects": [ { "type": "sphere", "radius": 0.3, "pos": [0, 0.6, 0] }, { "type": "box", "color": "#E4572E" }, { "type": "box", "color": "#29335C" }, { "type": "box", "color": "#A8C686" }, { "type": "box", "color": "#F3A712" }, { "type": "box", "color": "#7B2CBF" } ], "camera": { "preset": "orbit", "degrees": 360, "duration": 4 }, "bloom": 0.3 }));
        let all = w.iter().map(|(p, m)| format!("{p}: {m}")).collect::<Vec<_>>().join("\n");
        for needle in ["`rig`", "turntable", "floats", "hues", "bloom"] {
            assert!(all.contains(needle), "missing {needle}: {all}");
        }
        let still = lint(&json!({ "env": "studio", "camera": { "keys": [ { "t": 0, "pos": [0, 1, 4], "target": [0, 0, 0] } ] }, "objects": [] }));
        assert!(still.iter().any(|(_, m)| m.contains("breath")));
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

/// HDRI environments and PBR texture sets end to end, from local files (no network).
#[cfg(test)]
mod asset_tests {
    use super::*;
    use serde_json::json;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("anikit-s3d-assets-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A 128×64 sky: blue above, brown below, a small 4000-nit sun.
    fn write_hdr(path: &std::path::Path) {
        let (w, h) = (128u32, 64u32);
        let img = image::Rgb32FImage::from_fn(w, h, |x, y| {
            if (60..62).contains(&x) && (14..16).contains(&y) {
                image::Rgb([4000.0, 3800.0, 3400.0])
            } else if y < h / 2 {
                image::Rgb([0.35, 0.5, 0.8])
            } else {
                image::Rgb([0.12, 0.09, 0.06])
            }
        });
        image::DynamicImage::ImageRgb32F(img).save(path).unwrap();
    }

    fn mean(px: &[u8]) -> [f32; 3] {
        let n = (px.len() / 4) as f32;
        let mut m = [0.0; 3];
        for p in px.chunks_exact(4) {
            for k in 0..3 {
                m[k] += p[k] as f32 / n;
            }
        }
        m
    }

    #[test]
    fn hdri_env_lights_reflects_and_backs_the_scene() {
        let d = dir("hdri");
        let hdr = d.join("sky.hdr");
        write_hdr(&hdr);
        let props = |bg: Value| json!({
            "env": { "hdri": hdr.to_string_lossy(), "background": bg },
            "camera": { "keys": [ { "t": 0, "pos": [0, 0.6, 3], "target": [0, 0.5, 0], "focal": 40 } ] },
            "objects": [ { "type": "sphere", "radius": 0.5, "pos": [0, 0.5, 0], "material": "chrome" } ]
        });
        let mut s = Scene3d::try_from(props(json!(true))).unwrap();
        s.prepare().map_err(|e| e.0).unwrap();
        let p = s.p();
        assert!(p.env.map.is_some());
        assert!(p.env.key_box().is_some(), "the sun becomes the traced key");
        let px = s.render(0.0, 1.0 / 30.0, 64, 36, None);
        // The top of the frame is the blue sky.
        let top = mean(&px[..64 * 4 * 4]);
        assert!(top[2] > top[0] + 10.0, "sky should be blue: {top:?}");
        // Without the background, rays that miss see the dark studio backdrop instead.
        let mut s2 = Scene3d::try_from(props(json!(false))).unwrap();
        s2.prepare().map_err(|e| e.0).unwrap();
        let top2 = mean(&s2.render(0.0, 1.0 / 30.0, 64, 36, None)[..64 * 4 * 4]);
        assert!(top2[2] + 20.0 < top[2], "{top2:?} vs {top:?}");
        // Unknown keys and bad values are reported.
        let bad = Scene3d::try_from(json!({ "env": { "hdri": hdr.to_string_lossy(), "rotation": 3 } })).and_then(|mut s| s.prepare().map_err(|e| e.0));
        let msg = bad.err().unwrap_or_default();
        assert!(msg.contains("rotation"), "{msg}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn texture_sets_tint_and_roughen_a_material() {
        let d = dir("pbr");
        // Red albedo, mid roughness, flat normal.
        image::RgbImage::from_pixel(32, 32, image::Rgb([200, 30, 30])).save(d.join("Red_Color.jpg")).unwrap();
        image::GrayImage::from_pixel(32, 32, image::Luma([200])).save(d.join("Red_Roughness.png")).unwrap();
        image::RgbImage::from_pixel(32, 32, image::Rgb([128, 128, 255])).save(d.join("Red_NormalGL.png")).unwrap();
        let s = Scene3d::try_from(json!({
            "env": "studio",
            "objects": [ { "type": "sphere", "radius": 0.5, "pos": [0, 0.5, 0], "material": { "texture": d.to_string_lossy(), "scale": 2 } } ]
        }))
        .and_then(|mut s| s.prepare().map_err(|e| e.0).map(|_| s))
        .unwrap();
        let m = &s.p().mats[s.p().objs.iter().find_map(|o| o.parts.first().map(|x| x.1 as usize)).unwrap_or(0)];
        assert!(m.pbr > 0);
        let px = s.render(0.0, 1.0 / 30.0, 48, 48, None);
        // The sphere in the middle reads red.
        let c = &px[(24 * 48 + 24) * 4..(24 * 48 + 24) * 4 + 3];
        assert!(c[0] as i32 > c[1] as i32 + 30, "{c:?}");
        // The same set on the ground: the floor reads red too.
        let g = Scene3d::try_from(json!({
            "env": "studio", "floor": { "texture": d.to_string_lossy() },
            "camera": { "keys": [ { "t": 0, "pos": [0, 2, 2], "target": [0, 0, 0], "focal": 30 } ] },
            "objects": [ { "type": "sphere", "radius": 0.1, "pos": [5, 0.1, 5] } ]
        }))
        .and_then(|mut s| s.prepare().map_err(|e| e.0).map(|_| s))
        .unwrap();
        let px = g.render(0.0, 1.0 / 30.0, 32, 32, None);
        let c = &px[(16 * 32 + 16) * 4..(16 * 32 + 16) * 4 + 3];
        assert!(c[0] as i32 > c[1] as i32 + 20, "floor {c:?}");
        // A folder without a colour map is an error that says so.
        let empty = dir("pbr-empty");
        let e = Scene3d::try_from(json!({ "objects": [ { "type": "sphere", "material": { "texture": empty.to_string_lossy() } } ] })).and_then(|mut s| s.prepare().map_err(|e| e.0));
        assert!(e.unwrap_err().contains("albedo"));
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&empty);
    }
}
