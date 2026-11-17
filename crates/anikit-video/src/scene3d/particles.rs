//! `scene3d` particles: one to ten million points driven by curl-noise flow,
//! attractors, gravity and drag, emitted from shapes, text or images, able to
//! assemble into a target shape, bouncing off the floor, and drawn as lit or
//! glowing sprites with depth of field and motion streaks.
//!
//! ```json
//! { "type": "particles", "count": 2000000,
//!   "emit": { "text": "ANIKIT", "height": 1.2 },
//!   "flow": { "strength": 1.2, "scale": 1.5 }, "gravity": -0.4,
//!   "size": 0.004, "color": ["#ffffff", "#7fd4ff"], "colorBy": "speed", "blend": "add" }
//! ```
//!
//! **Simulation.** Positions and velocities live in flat `f32` arrays and
//! step in parallel chunks. The flow field is a divergence-free curl of a
//! periodic noise potential, precomputed once on a 64³ grid (two of them,
//! cross-faded so the flow evolves), so a particle's flow costs two
//! trilinear lookups. The sim advances frame by frame; frames requested out
//! of order inside a render batch are served from a small snapshot cache,
//! and older frames restart from periodic checkpoints.
//!
//! **Rendering.** Every particle is projected and splatted (2×2 bilinear,
//! energy-preserving) into a pyramid of buffers, at the level where its
//! footprint (size, defocus, or both) is about one texel, so a particle
//! costs the same in focus or blurred. Fast particles splat several times
//! along their screen motion (motion blur). Levels are summed back up with
//! smooth upsampling. Additive particles add light; `over` particles absorb
//! (Beer–Lambert over the summed density) and add their own lit colour, so
//! dense clouds read as volumes without sorting. Each particle is depth
//! tested against the traced scene.

use super::math::*;
use super::spec::{self, Num, ObjDef, Res};
use super::trace::{Camera, Material, Shape};
use crate::eval::expr::Vars;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const KEYS: &[&str] = &[
    "type", "name", "count", "seed", "emit", "velocity", "spread", "normal_speed", "life", "flow", "attractors", "gravity", "drag", "target", "collide", "bounce", "friction", "size", "size_var", "color", "color_by", "speed_range", "opacity", "emissive", "blend", "lit", "start", "end", "substeps", "stretch",
];
const EMIT_KEYS: &[&str] = &["shape", "pos", "position", "radius", "size", "surface", "axis", "image", "composition", "frame", "threshold", "rot", "rotation"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Blend {
    Add,
    Over,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ColorBy {
    Random,
    Age,
    Speed,
    Height,
    Source,
}

/// Where particles are born: a list of weighted sample points (with normal
/// and colour), or an analytic primitive.
#[derive(Clone)]
enum Emit {
    Point(V3),
    Sphere { c: V3, r: f32, surface: bool },
    Box { c: V3, h: V3 },
    Disc { c: V3, r: f32, axis: V3 },
    /// Triangles (world space) with cumulative areas, for uniform surface sampling.
    Mesh { tris: Arc<Vec<[V3; 3]>>, cdf: Arc<Vec<f32>> },
    /// Image pixels as points on a plane: (position, colour), cumulative weights.
    Image { pts: Arc<Vec<(V3, [f32; 3])>>, cdf: Arc<Vec<f32>>, texel: f32, normal: V3 },
    /// An image or composition, resolved on first render.
    Pending(Arc<Map<String, Value>>),
}

#[derive(Clone)]
struct Attractor {
    pos: [Num; 3],
    strength: f32,
    swirl: f32,
    radius: f32,
}

#[derive(Clone)]
struct Target {
    emit: Emit,
    from: f32,
    to: f32,
}

/// One particle system, as parsed.
#[derive(Clone)]
pub struct Def {
    pub count: usize,
    seed: u32,
    emit: Emit,
    vel: V3,
    spread: f32,
    normal_speed: f32,
    life: Option<f32>,
    flow: Option<(f32, f32, f32, V3)>,
    attractors: Vec<Attractor>,
    gravity: V3,
    drag: f32,
    target: Option<Target>,
    collide: bool,
    bounce: f32,
    friction: f32,
    pub size: f32,
    size_var: f32,
    colors: Vec<V3>,
    color_by: ColorBy,
    speed_range: (f32, f32),
    opacity: f32,
    emissive: f32,
    pub blend: Blend,
    lit: bool,
    pub start: f32,
    pub end: f32,
    substeps: u32,
    stretch: f32,
}

fn gf(m: &Map<String, Value>, k: &str, d: f32, path: &str) -> Res<f32> {
    m.get(k).map(|x| spec::f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d))
}

fn v3c(v: &Value, path: &str) -> Res<V3> {
    match v {
        Value::Array(a) if a.len() == 3 => Ok(v3(spec::f(&a[0], path)?, spec::f(&a[1], path)?, spec::f(&a[2], path)?)),
        Value::Number(_) => {
            let x = spec::f(v, path)?;
            Ok(v3(0.0, x, 0.0))
        }
        _ => Err(format!("{path}: expected [x, y, z]")),
    }
}

/// Parse an emitter or target description.
fn emit(v: &Value, shapes: &mut spec::Shapes, mats: &mut Vec<Material>, named: &HashMap<String, Material>, objects: &[Value], path: &str) -> Res<Emit> {
    if let Value::String(name) = v {
        // A named scene object.
        let o = objects.iter().find(|o| o.get("name").and_then(|n| n.as_str()) == Some(name)).ok_or_else(|| format!("{path}: no object named `{name}` in `objects`"))?;
        return emit(o, shapes, mats, named, objects, path);
    }
    let m = v.as_object().ok_or_else(|| format!("{path}: expected an emitter like {{ \"shape\": \"sphere\", \"radius\": 1 }} or an object like {{ \"text\": \"ANIKIT\" }}"))?;
    let pos = m.get("pos").or(m.get("position")).map(|x| v3c(x, &format!("{path}.pos"))).transpose()?;
    if m.contains_key("image") || m.contains_key("composition") {
        spec::check_keys(m, EMIT_KEYS, path)?;
        return Ok(Emit::Pending(Arc::new(m.clone())));
    }
    if let Some(s) = m.get("shape").and_then(|s| s.as_str()) {
        spec::check_keys(m, EMIT_KEYS, path)?;
        let c = pos.unwrap_or(v3(0.0, 1.0, 0.0));
        return Ok(match s {
            "point" => Emit::Point(c),
            "sphere" | "ball" => Emit::Sphere { c, r: gf(m, "radius", 1.0, path)?, surface: m.get("surface").and_then(|x| x.as_bool()).unwrap_or(s == "sphere") },
            "box" => {
                let sz = m.get("size").map(|x| v3c(x, &format!("{path}.size"))).transpose()?.unwrap_or(v3(1.0, 1.0, 1.0));
                Emit::Box { c, h: scale(sz, 0.5) }
            }
            "disc" | "ring" => Emit::Disc { c, r: gf(m, "radius", 1.0, path)?, axis: m.get("axis").map(|x| v3c(x, &format!("{path}.axis"))).transpose()?.map(norm).unwrap_or(v3(0.0, 1.0, 0.0)) },
            o => return Err(format!("{path}.shape: unknown emitter shape `{o}` (point, sphere, ball, box, disc)")),
        });
    }
    // Any scene object (text, box, torus …): sample its surface at t = 0.
    let mut om = m.clone();
    om.remove("count");
    let od: ObjDef = spec::object(&Value::Object(om), shapes, mats, named, path)?;
    let xf = od.xf_at(0.0, 0, 0);
    let tris: Vec<[V3; 3]> = match &shapes.list[od.shape as usize] {
        Shape::Mesh(b) => b.tris.iter().map(|t| [xf.point(t.v0), xf.point(add(t.v0, t.e1)), xf.point(add(t.v0, t.e2))]).collect(),
        Shape::Sphere => return Ok(Emit::Sphere { c: xf.t, r: xf.s, surface: true }),
    };
    let mut cdf = Vec::with_capacity(tris.len());
    let mut acc = 0.0f32;
    for t in &tris {
        acc += len(cross(sub(t[1], t[0]), sub(t[2], t[0]))) * 0.5;
        cdf.push(acc);
    }
    if acc <= 0.0 {
        return Err(format!("{path}: the shape has no area to emit from"));
    }
    Ok(Emit::Mesh { tris: Arc::new(tris), cdf: Arc::new(cdf) })
}

pub fn parse(v: &Value, shapes: &mut spec::Shapes, mats: &mut Vec<Material>, named: &HashMap<String, Material>, objects: &[Value], path: &str) -> Res<Def> {
    let m = v.as_object().ok_or_else(|| format!("{path}: expected an object"))?;
    spec::check_keys(m, KEYS, path)?;
    let count = gf(m, "count", 100_000.0, path)?.max(0.0) as usize;
    if count > 20_000_000 {
        return Err(format!("{path}.count: {count} particles is too many (max 20000000)"));
    }
    let emit_v = m.get("emit").cloned().unwrap_or(serde_json::json!({ "shape": "sphere", "radius": 1.0, "pos": [0, 1, 0] }));
    let emit_ = emit(&emit_v, shapes, mats, named, objects, &format!("{path}.emit"))?;
    let flow = match m.get("flow") {
        None | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some((1.0, 1.5, 0.15, v3(0.0, 0.0, 0.0))),
        Some(Value::Number(n)) => Some((n.as_f64().unwrap_or(1.0) as f32, 1.5, 0.15, v3(0.0, 0.0, 0.0))),
        Some(Value::Object(f)) => {
            let p = format!("{path}.flow");
            spec::check_keys(f, &["strength", "scale", "evolve", "drift"], &p)?;
            Some((gf(f, "strength", 1.0, &p)?, gf(f, "scale", 1.5, &p)?.max(1e-3), gf(f, "evolve", 0.15, &p)?, f.get("drift").map(|x| v3c(x, &format!("{p}.drift"))).transpose()?.unwrap_or(v3(0.0, 0.0, 0.0))))
        }
        Some(_) => return Err(format!("{path}.flow: expected {{ \"strength\", \"scale\", \"evolve\" }}")),
    };
    let mut attractors = Vec::new();
    if let Some(a) = m.get("attractors") {
        for (j, x) in a.as_array().ok_or_else(|| format!("{path}.attractors: expected an array"))?.iter().enumerate() {
            let p = format!("{path}.attractors[{j}]");
            let o = x.as_object().ok_or_else(|| format!("{p}: expected {{ \"pos\", \"strength\" }}"))?;
            spec::check_keys(o, &["pos", "position", "strength", "swirl", "radius"], &p)?;
            attractors.push(Attractor {
                pos: o.get("pos").or(o.get("position")).map(|x| spec::vec3(x, &format!("{p}.pos"))).transpose()?.unwrap_or([Num::C(0.0), Num::C(1.0), Num::C(0.0)]),
                strength: gf(o, "strength", 2.0, &p)?,
                swirl: gf(o, "swirl", 0.0, &p)?,
                radius: gf(o, "radius", 0.5, &p)?.max(1e-3),
            });
        }
    }
    let target = match m.get("target") {
        None => None,
        Some(t) => {
            let p = format!("{path}.target");
            let mut tm = t.as_object().cloned().ok_or_else(|| format!("{p}: expected an emitter plus \"from\"/\"to\" seconds"))?;
            let from = tm.remove("from").map(|x| spec::f(&x, &format!("{p}.from"))).transpose()?.unwrap_or(1.0);
            let to = tm.remove("to").map(|x| spec::f(&x, &format!("{p}.to"))).transpose()?.unwrap_or(from + 2.0);
            Some(Target { emit: emit(&Value::Object(tm), shapes, mats, named, objects, &p)?, from, to: to.max(from + 1e-3) })
        }
    };
    let colors = match m.get("color") {
        None => vec![v3(1.0, 1.0, 1.0)],
        Some(Value::Array(a)) if a.iter().all(|x| x.is_string()) => a.iter().enumerate().map(|(k, x)| spec::color(x, &format!("{path}.color[{k}]"))).collect::<Res<Vec<_>>>()?,
        Some(x) => vec![spec::color(x, &format!("{path}.color"))?],
    };
    let blend = match m.get("blend").and_then(|b| b.as_str()) {
        None | Some("add") | Some("additive") | Some("glow") => Blend::Add,
        Some("over") | Some("alpha") | Some("normal") => Blend::Over,
        Some(o) => return Err(format!("{path}.blend: unknown blend `{o}` (add, over)")),
    };
    let color_by = match m.get("color_by").and_then(|b| b.as_str()) {
        None => {
            if matches!(emit_, Emit::Pending(_)) {
                ColorBy::Source
            } else {
                ColorBy::Random
            }
        }
        Some("random") => ColorBy::Random,
        Some("age") => ColorBy::Age,
        Some("speed") => ColorBy::Speed,
        Some("height") => ColorBy::Height,
        Some("source") | Some("image") => ColorBy::Source,
        Some(o) => return Err(format!("{path}.colorBy: unknown `{o}` (random, age, speed, height, source)")),
    };
    let gravity = match m.get("gravity") {
        None => v3(0.0, 0.0, 0.0),
        Some(x) => v3c(x, &format!("{path}.gravity"))?,
    };
    let sr = match m.get("speed_range") {
        Some(Value::Array(a)) if a.len() == 2 => (spec::f(&a[0], path)?, spec::f(&a[1], path)?),
        _ => (0.0, 2.0),
    };
    let life = match m.get("life") {
        None | Some(Value::Null) => None,
        Some(x) => Some(spec::f(x, &format!("{path}.life"))?.max(0.05)),
    };
    Ok(Def {
        count,
        seed: gf(m, "seed", 1.0, path)? as u32,
        emit: emit_,
        vel: m.get("velocity").map(|x| v3c(x, &format!("{path}.velocity"))).transpose()?.unwrap_or(v3(0.0, 0.0, 0.0)),
        spread: gf(m, "spread", 0.0, path)?,
        normal_speed: gf(m, "normal_speed", 0.0, path)?,
        life,
        flow,
        attractors,
        gravity,
        drag: gf(m, "drag", 0.2, path)?.max(0.0),
        target,
        collide: m.get("collide").and_then(|x| x.as_bool()).unwrap_or(true),
        bounce: gf(m, "bounce", 0.3, path)?,
        friction: gf(m, "friction", 0.3, path)?.clamp(0.0, 1.0),
        size: gf(m, "size", 0.004, path)?.max(1e-5),
        size_var: gf(m, "size_var", 0.5, path)?.clamp(0.0, 1.0),
        colors,
        color_by,
        speed_range: sr,
        opacity: gf(m, "opacity", 1.0, path)?.clamp(0.0, 1.0),
        emissive: gf(m, "emissive", if blend == Blend::Add { 1.0 } else { 0.0 }, path)?,
        blend,
        lit: m.get("lit").and_then(|x| x.as_bool()).unwrap_or(blend == Blend::Over),
        start: gf(m, "start", 0.0, path)?,
        end: gf(m, "end", f32::INFINITY, path)?,
        substeps: gf(m, "substeps", 2.0, path)?.clamp(1.0, 16.0) as u32,
        stretch: gf(m, "stretch", 1.0, path)?.max(0.0),
    })
}

// ------------------------------------------------------------------ emitters

#[inline]
fn hu(seed: u32, i: u32, k: u32) -> f32 {
    u01(hash2(hash2(seed ^ 0x9e37_79b9, i), k))
}

fn pick(cdf: &[f32], u: f32) -> usize {
    let x = u * cdf.last().copied().unwrap_or(0.0);
    cdf.partition_point(|&c| c < x).min(cdf.len().saturating_sub(1))
}

/// Sample emitter `e` for particle `i` (cycle `k`): position, normal, colour.
fn sample(e: &Emit, seed: u32, i: u32, k: u32) -> (V3, V3, Option<[f32; 3]>) {
    let s = hash2(seed, k.wrapping_mul(0x85eb_ca6b));
    let u = |j: u32| hu(s, i, j);
    let sph = |a: f32, b: f32| {
        let z = 1.0 - 2.0 * a;
        let r = (1.0 - z * z).max(0.0).sqrt();
        let ph = b * std::f32::consts::TAU;
        v3(r * ph.cos(), z, r * ph.sin())
    };
    match e {
        Emit::Point(c) => (*c, sph(u(1), u(2)), None),
        Emit::Sphere { c, r, surface } => {
            let n = sph(u(1), u(2));
            let rr = if *surface { *r } else { r * u(3).cbrt() };
            (madd(*c, n, rr), n, None)
        }
        Emit::Box { c, h } => (v3(c.x + (u(1) * 2.0 - 1.0) * h.x, c.y + (u(2) * 2.0 - 1.0) * h.y, c.z + (u(3) * 2.0 - 1.0) * h.z), sph(u(4), u(5)), None),
        Emit::Disc { c, r, axis } => {
            let (a, b) = basis(*axis);
            let rr = r * u(1).sqrt();
            let ph = u(2) * std::f32::consts::TAU;
            (add(*c, add(scale(a, rr * ph.cos()), scale(b, rr * ph.sin()))), *axis, None)
        }
        Emit::Mesh { tris, cdf } => {
            let t = &tris[pick(cdf, u(1))];
            let (mut a, mut b) = (u(2), u(3));
            if a + b > 1.0 {
                a = 1.0 - a;
                b = 1.0 - b;
            }
            let p = add(t[0], add(scale(sub(t[1], t[0]), a), scale(sub(t[2], t[0]), b)));
            (p, norm(cross(sub(t[1], t[0]), sub(t[2], t[0]))), None)
        }
        Emit::Image { pts, cdf, texel, normal } => {
            let (p, c) = pts[pick(cdf, u(1))];
            let (a, b) = basis(*normal);
            let j = add(scale(a, (u(2) - 0.5) * texel), scale(b, (u(3) - 0.5) * texel));
            (add(p, j), *normal, Some(c))
        }
        Emit::Pending(_) => (v3(0.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), None),
    }
}

/// Resolve image / composition emitters (needs the spec's compositions).
fn resolve(e: &Emit, root: Option<&crate::eval::Evaluator>, cache: &crate::pipeline::textures::CompTextures) -> Res<Emit> {
    let Emit::Pending(m) = e else { return Ok(e.clone()) };
    let path = "particles.emit";
    let (w, h, px): (usize, usize, Vec<[f32; 4]>) = if let Some(file) = m.get("image").and_then(|x| x.as_str()) {
        let img = image::open(file).map_err(|err| format!("{path}.image: can't read `{file}`: {err}"))?.to_rgba32f();
        let (w, h) = (img.width() as usize, img.height() as usize);
        (w, h, img.pixels().map(|p| p.0).collect())
    } else {
        let name = m.get("composition").and_then(|x| x.as_str()).unwrap_or("").trim_start_matches('#');
        let comp = root.and_then(|r| r.composition(name)).ok_or_else(|| format!("{path}.composition: no composition `{name}`"))?;
        let ct = m.get("frame").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32 / comp.fps.max(1.0);
        let pm = cache.get(comp, ct, 0);
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let data = pm.data_as_u8_slice();
        let px = data.chunks_exact(4).map(|c| {
            let a = c[3] as f32 / 255.0;
            let un = |v: u8| if a > 0.0 { (v as f32 / 255.0 / a).min(1.0) } else { 0.0 };
            [un(c[0]), un(c[1]), un(c[2]), a]
        }).collect();
        (w, h, px)
    };
    let size = match m.get("size") {
        Some(Value::Array(a)) if a.len() == 2 => (spec::f(&a[0], path)?, spec::f(&a[1], path)?),
        Some(x) => {
            let s = spec::f(x, &format!("{path}.size"))?;
            (s, s * h as f32 / w.max(1) as f32)
        }
        None => (4.0, 4.0 * h as f32 / w.max(1) as f32),
    };
    let c = m.get("pos").or(m.get("position")).map(|x| v3c(x, path)).transpose()?.unwrap_or(v3(0.0, size.1 * 0.5 + 0.2, 0.0));
    let rot = m.get("rot").or(m.get("rotation")).map(|x| v3c(x, path)).transpose()?.unwrap_or(v3(0.0, 0.0, 0.0));
    let xf = Xf::new(c, rot, 1.0);
    let thr = m.get("threshold").and_then(|x| x.as_f64()).unwrap_or(0.05) as f32;
    let mut pts = Vec::new();
    let mut cdf = Vec::new();
    let mut acc = 0.0;
    let to_lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    for y in 0..h {
        for x in 0..w {
            let p = px[y * w + x];
            let l = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
            // Weight: alpha, and brightness when the image is opaque.
            let wgt = p[3] * if px.iter().take(64).all(|q| q[3] > 0.99) { l } else { 1.0 };
            if wgt <= thr {
                continue;
            }
            let lp = v3(((x as f32 + 0.5) / w as f32 - 0.5) * size.0, (0.5 - (y as f32 + 0.5) / h as f32) * size.1, 0.0);
            acc += wgt;
            cdf.push(acc);
            pts.push((xf.point(lp), [to_lin(p[0]), to_lin(p[1]), to_lin(p[2])]));
        }
    }
    if pts.is_empty() {
        return Err(format!("{path}: the image has no pixels above the threshold"));
    }
    Ok(Emit::Image { pts: Arc::new(pts), cdf: Arc::new(cdf), texel: size.0 / w as f32, normal: xf.rot(v3(0.0, 0.0, 1.0)) })
}

// ------------------------------------------------------------------ flow field

const FN: usize = 64;

/// A periodic, divergence-free velocity field on an `FN`³ grid (unit RMS speed).
pub struct FlowGrid {
    v: Vec<[f32; 3]>,
}

fn lattice(seed: u32, x: i32, y: i32, z: i32, p: i32) -> f32 {
    let (x, y, z) = (x.rem_euclid(p) as u32, y.rem_euclid(p) as u32, z.rem_euclid(p) as u32);
    u01(hash2(hash2(hash2(seed, x), y), z)) * 2.0 - 1.0
}

fn value_noise(seed: u32, x: f32, y: f32, z: f32, p: i32) -> f32 {
    let (ix, iy, iz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let q = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (fx, fy, fz) = (q(x - ix as f32), q(y - iy as f32), q(z - iz as f32));
    let l = |a, b, c| lattice(seed, ix + a, iy + b, iz + c, p);
    let lx = |b, c| l(0, b, c) + (l(1, b, c) - l(0, b, c)) * fx;
    let ly = |c| lx(0, c) + (lx(1, c) - lx(0, c)) * fy;
    ly(0) + (ly(1) - ly(0)) * fz
}

impl FlowGrid {
    pub fn new(seed: u32) -> FlowGrid {
        use rayon::prelude::*;
        let n = FN;
        // Vector potential: two octaves of periodic noise (4 and 8 lattice cells per period).
        let pot: Vec<[f32; 3]> = (0..n * n * n)
            .into_par_iter()
            .map(|i| {
                let (x, y, z) = ((i % n) as f32, ((i / n) % n) as f32, (i / (n * n)) as f32);
                let mut o = [0.0f32; 3];
                for (c, oc) in o.iter_mut().enumerate() {
                    let s = seed.wrapping_mul(31).wrapping_add(c as u32 * 7919);
                    for (oct, (p, amp)) in [(4i32, 1.0f32), (8, 0.45)].iter().enumerate() {
                        let k = *p as f32 / n as f32;
                        *oc += amp * value_noise(s.wrapping_add(oct as u32 * 104729), x * k, y * k, z * k, *p);
                    }
                }
                o
            })
            .collect();
        let at = |x: usize, y: usize, z: usize| pot[(z % n) * n * n + (y % n) * n + (x % n)];
        let mut v: Vec<[f32; 3]> = (0..n * n * n)
            .into_par_iter()
            .map(|i| {
                let (x, y, z) = (i % n, (i / n) % n, i / (n * n));
                let d = |a: [f32; 3], b: [f32; 3], c: usize| (a[c] - b[c]) * 0.5;
                let (xp, xm, yp, ym, zp, zm) = (at(x + 1, y, z), at(x + n - 1, y, z), at(x, y + 1, z), at(x, y + n - 1, z), at(x, y, z + 1), at(x, y, z + n - 1));
                // curl ψ = (∂ψz/∂y − ∂ψy/∂z, ∂ψx/∂z − ∂ψz/∂x, ∂ψy/∂x − ∂ψx/∂y)
                [d(yp, ym, 2) - d(zp, zm, 1), d(zp, zm, 0) - d(xp, xm, 2), d(xp, xm, 1) - d(yp, ym, 0)]
            })
            .collect();
        let rms = (v.iter().map(|q| q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sum::<f32>() / v.len() as f32).sqrt().max(1e-9);
        for q in &mut v {
            *q = q.map(|c| c / rms);
        }
        FlowGrid { v }
    }

    /// Trilinear lookup at grid coordinates (wrapping).
    #[inline(always)]
    pub fn sample(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
        let (tx, ty, tz) = (x - fx, y - fy, z - fz);
        let m = FN as i32 - 1;
        let (x0, y0, z0) = ((fx as i32) & m, (fy as i32) & m, (fz as i32) & m);
        let (x1, y1, z1) = ((x0 + 1) & m, (y0 + 1) & m, (z0 + 1) & m);
        let (x0, y0, z0, x1, y1, z1) = (x0 as usize, y0 as usize * FN, z0 as usize * FN * FN, x1 as usize, y1 as usize * FN, z1 as usize * FN * FN);
        let v = &self.v;
        let mut o = [0.0f32; 3];
        let (a, b, c, d) = (v[z0 + y0 + x0], v[z0 + y0 + x1], v[z0 + y1 + x0], v[z0 + y1 + x1]);
        let (e, f, g, h) = (v[z1 + y0 + x0], v[z1 + y0 + x1], v[z1 + y1 + x0], v[z1 + y1 + x1]);
        for k in 0..3 {
            let ab = a[k] + (b[k] - a[k]) * tx;
            let cd = c[k] + (d[k] - c[k]) * tx;
            let ef = e[k] + (f[k] - e[k]) * tx;
            let gh = g[k] + (h[k] - g[k]) * tx;
            let l0 = ab + (cd - ab) * ty;
            let l1 = ef + (gh - ef) * ty;
            o[k] = l0 + (l1 - l0) * tz;
        }
        o
    }
}

// ------------------------------------------------------------------ simulation

/// Particle state at one frame, compact: positions, and velocity quantised to i16.
pub struct Snap {
    pub pos: Vec<[f32; 3]>,
    pub vel: Vec<[i16; 3]>,
    pub vscale: f32,
    pub t: f32,
}

impl Snap {
    #[inline]
    pub fn vel(&self, i: usize) -> V3 {
        let v = self.vel[i];
        v3(v[0] as f32 * self.vscale, v[1] as f32 * self.vscale, v[2] as f32 * self.vscale)
    }
    fn bytes(&self) -> usize {
        self.pos.len() * 18
    }
}

struct Live {
    frame: i64,
    px: Vec<f32>,
    py: Vec<f32>,
    pz: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    vz: Vec<f32>,
}

impl Clone for Live {
    fn clone(&self) -> Live {
        Live { frame: self.frame, px: self.px.clone(), py: self.py.clone(), pz: self.pz.clone(), vx: self.vx.clone(), vy: self.vy.clone(), vz: self.vz.clone() }
    }
}

struct SimState {
    live: Option<Live>,
    snaps: HashMap<i64, Arc<Snap>>,
    order: Vec<i64>,
    checkpoints: Vec<Live>,
    steps: u64,
    sim_ms: f64,
}

/// A particle system bound to a frame rate, ready to step.
pub struct Sim {
    pub def: Def,
    dt: f32,
    emit: Emit,
    /// Per-particle colour from an image emitter (linear RGB).
    src_col: Option<Vec<[f32; 3]>>,
    tgt: Option<(Vec<[f32; 3]>, Vec<f32>)>,
    flows: Option<(FlowGrid, FlowGrid)>,
    floor_y: Option<f32>,
    state: Mutex<SimState>,
    busy: std::sync::atomic::AtomicBool,
}

const CHECK_EVERY: i64 = 60;
const SNAP_BUDGET: usize = 3 << 30;
const CHECK_BUDGET: usize = 2 << 30;

impl Sim {
    pub fn new(def: &Def, fps: f32, floor_y: Option<f32>, root: Option<&crate::eval::Evaluator>, cache: &crate::pipeline::textures::CompTextures) -> Res<Sim> {
        let emit = resolve(&def.emit, root, cache)?;
        let target = def.target.as_ref().map(|t| resolve(&t.emit, root, cache)).transpose()?;
        let n = def.count;
        let src_col = match &emit {
            Emit::Image { .. } => Some((0..n).map(|i| sample(&emit, def.seed, i as u32, 0).2.unwrap_or([1.0; 3])).collect()),
            _ => None,
        };
        let tgt = target.as_ref().map(|t| {
            use rayon::prelude::*;
            let pts: Vec<[f32; 3]> = (0..n).into_par_iter().map(|i| {
                let p = sample(t, def.seed ^ 0x7a3c, i as u32, 0).0;
                [p.x, p.y, p.z]
            }).collect();
            // Staggered arrival: lower target points settle first.
            let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
            let delay = pts.iter().enumerate().map(|(i, p)| 0.55 * (p[1] - lo) / (hi - lo).max(1e-6) + 0.15 * hu(def.seed, i as u32, 99)).collect();
            (pts, delay)
        });
        let flows = def.flow.map(|_| {
            let (a, b) = rayon::join(|| FlowGrid::new(def.seed.wrapping_mul(3)), || FlowGrid::new(def.seed.wrapping_mul(3) + 1));
            (a, b)
        });
        Ok(Sim {
            def: def.clone(),
            dt: 1.0 / fps.max(1.0),
            emit,
            src_col,
            tgt,
            flows,
            floor_y: if def.collide { floor_y } else { None },
            state: Mutex::new(SimState { live: None, snaps: HashMap::new(), order: Vec::new(), checkpoints: Vec::new(), steps: 0, sim_ms: 0.0 }),
            busy: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Birth time and cycle length of particle `i` (`life` systems recycle).
    #[inline]
    pub fn birth(&self, i: usize) -> f32 {
        match self.def.life {
            Some(l) => self.def.start + hu(self.def.seed, i as u32, 17) * l,
            None => self.def.start,
        }
    }

    /// Age in [0, 1) of particle `i` at `t` (negative before its first birth).
    #[inline]
    pub fn age(&self, i: usize, t: f32) -> f32 {
        let b = self.birth(i);
        match self.def.life {
            Some(l) => {
                if t < b {
                    -1.0
                } else {
                    ((t - b) / l).fract()
                }
            }
            None => ((t - b) / 4.0).clamp(0.0, 0.999),
        }
    }

    fn spawn(&self, i: usize, k: u32) -> (V3, V3) {
        let d = &self.def;
        let (p, n, _) = sample(&self.emit, d.seed, i as u32, k);
        let r = |j: u32| hu(d.seed ^ k.wrapping_mul(0x27d4_eb2f), i as u32, j) * 2.0 - 1.0;
        let v = add(add(d.vel, scale(n, d.normal_speed)), scale(v3(r(11), r(12), r(13)), d.spread));
        (p, v)
    }

    fn init(&self) -> Live {
        use rayon::prelude::*;
        let n = self.def.count;
        let pv: Vec<(V3, V3)> = (0..n).into_par_iter().map(|i| self.spawn(i, 0)).collect();
        Live {
            frame: 0,
            px: pv.iter().map(|q| q.0.x).collect(),
            py: pv.iter().map(|q| q.0.y).collect(),
            pz: pv.iter().map(|q| q.0.z).collect(),
            vx: pv.iter().map(|q| q.1.x).collect(),
            vy: pv.iter().map(|q| q.1.y).collect(),
            vz: pv.iter().map(|q| q.1.z).collect(),
        }
    }

    /// Advance `live` by one frame.
    fn step_frame(&self, live: &mut Live) {
        use rayon::prelude::*;
        let d = &self.def;
        let sub = d.substeps.max(1);
        let h = self.dt / sub as f32;
        let t0 = d.start + live.frame as f32 * self.dt;
        let chunk = 1 << 14;
        // Attractor positions per substep.
        let att: Vec<Vec<(V3, f32, f32, f32)>> = (0..sub)
            .map(|s| {
                let t = t0 + (s as f32 + 0.5) * h;
                let vars = Vars { t: t as f64, gt: t as f64, ..Vars::default() };
                d.attractors.iter().map(|a| (spec::eval3(&a.pos, &vars), a.strength, a.swirl, a.radius)).collect()
            })
            .collect();
        let Live { px, py, pz, vx, vy, vz, .. } = live;
        let flows = self.flows.as_ref();
        let (fs, fscale, fevo, fdrift) = d.flow.unwrap_or((0.0, 1.0, 0.0, v3(0.0, 0.0, 0.0)));
        let inv = FN as f32 / (fscale * 4.0);
        let drag = d.drag;
        let g = d.gravity;
        let floor = self.floor_y.map(|y| y + d.size);
        let tgt = self.tgt.as_ref();
        let target = d.target.as_ref();
        let life = d.life;
        px.par_chunks_mut(chunk)
            .zip(py.par_chunks_mut(chunk))
            .zip(pz.par_chunks_mut(chunk))
            .zip(vx.par_chunks_mut(chunk))
            .zip(vy.par_chunks_mut(chunk))
            .zip(vz.par_chunks_mut(chunk))
            .enumerate()
            .for_each(|(c, (((((px, py), pz), vx), vy), vz))| {
                let base = c * chunk;
                for s in 0..sub {
                    let t = t0 + s as f32 * h;
                    let te = t + h;
                    let (wa, wb) = {
                        let a = t * fevo * std::f32::consts::FRAC_PI_2;
                        (a.cos(), a.sin())
                    };
                    let off = scale(fdrift, -t);
                    let dampf = (-drag * h).exp();
                    let spring = target.map(|tg| ((t - tg.from) / (tg.to - tg.from)).clamp(-1.0, 2.0));
                    for j in 0..px.len() {
                        let i = base + j;
                        // Recycle at the end of each life.
                        if let Some(l) = life {
                            let b = d.start + hu(d.seed, i as u32, 17) * l;
                            let (k0, k1) = (((t - b) / l).floor(), ((te - b) / l).floor());
                            if k1 != k0 && te >= b {
                                let (p, v) = self.spawn(i, k1.max(0.0) as u32 + 1);
                                (px[j], py[j], pz[j], vx[j], vy[j], vz[j]) = (p.x, p.y, p.z, v.x, v.y, v.z);
                                continue;
                            }
                            if te < b {
                                continue;
                            }
                        }
                        let (x, y, z) = (px[j], py[j], pz[j]);
                        let (mut ax, mut ay, mut az) = (g.x, g.y, g.z);
                        let (mut ux, mut uy, mut uz) = (vx[j], vy[j], vz[j]);
                        if let Some((fa, fb)) = flows {
                            let (sx, sy, sz) = ((x + off.x) * inv, (y + off.y) * inv, (z + off.z) * inv);
                            let a = fa.sample(sx, sy, sz);
                            let b = fb.sample(sx + 17.0, sy + 31.0, sz + 5.0);
                            let k = 1.0 - dampf;
                            // Relax towards the flow velocity.
                            ux += ((a[0] * wa + b[0] * wb) * fs - ux) * k * 4.0;
                            uy += ((a[1] * wa + b[1] * wb) * fs - uy) * k * 4.0;
                            uz += ((a[2] * wa + b[2] * wb) * fs - uz) * k * 4.0;
                        }
                        for &(c, st, sw, r) in &att[s as usize] {
                            let (dx, dy, dz) = (c.x - x, c.y - y, c.z - z);
                            let d2 = dx * dx + dy * dy + dz * dz;
                            let inv_l = 1.0 / (d2 + r * r).sqrt();
                            let f = st * inv_l * inv_l * (d2.sqrt() * inv_l);
                            ax += dx * inv_l * f * (d2 + r * r).sqrt();
                            ay += dy * inv_l * f * (d2 + r * r).sqrt();
                            az += dz * inv_l * f * (d2 + r * r).sqrt();
                            if sw != 0.0 {
                                // Swirl about the vertical axis through the attractor.
                                ax += -dz * inv_l * sw;
                                az += dx * inv_l * sw;
                            }
                        }
                        ux = (ux + ax * h) * dampf;
                        uy = (uy + ay * h) * dampf;
                        uz = (uz + az * h) * dampf;
                        // Assemble: a critically damped spring to the target, per-particle delayed.
                        if let (Some(sp), Some((tp, delay))) = (spring, tgt) {
                            let w = ((sp - delay[i]) * 2.5).clamp(0.0, 1.0);
                            if w > 0.0 {
                                let q = tp[i];
                                let k = 60.0 * w * w;
                                let c2 = 2.0 * k.sqrt();
                                let e = 1.0 / (1.0 + c2 * h + k * h * h);
                                ux = (ux + k * h * (q[0] - x)) * e;
                                uy = (uy + k * h * (q[1] - y)) * e;
                                uz = (uz + k * h * (q[2] - z)) * e;
                            }
                        }
                        let (mut nx, mut ny, mut nz) = (x + ux * h, y + uy * h, z + uz * h);
                        if let Some(fy) = floor {
                            if ny < fy {
                                ny = fy + (fy - ny) * d.bounce;
                                if uy < 0.0 {
                                    uy = -uy * d.bounce;
                                }
                                ux *= 1.0 - d.friction;
                                uz *= 1.0 - d.friction;
                            }
                        }
                        let _ = (&mut nx, &mut nz);
                        (px[j], py[j], pz[j], vx[j], vy[j], vz[j]) = (nx, ny, nz, ux, uy, uz);
                    }
                }
            });
        live.frame += 1;
    }

    fn snap(&self, live: &Live) -> Snap {
        let n = live.px.len();
        let mut m = 1e-6f32;
        for i in 0..n {
            m = m.max(live.vx[i].abs()).max(live.vy[i].abs()).max(live.vz[i].abs());
        }
        let vs = m / 32767.0;
        let q = |v: f32| (v / vs).round().clamp(-32767.0, 32767.0) as i16;
        Snap {
            pos: (0..n).map(|i| [live.px[i], live.py[i], live.pz[i]]).collect(),
            vel: (0..n).map(|i| [q(live.vx[i]), q(live.vy[i]), q(live.vz[i])]).collect(),
            vscale: vs,
            t: self.def.start + live.frame as f32 * self.dt,
        }
    }

    /// State at video time `t` (rounded to a frame of this system's fps).
    pub fn at(&self, t: f32) -> Arc<Snap> {
        let f = ((t - self.def.start) / self.dt).round().max(0.0) as i64;
        // Wait for a stepping thread without blocking rayon: help with its work.
        loop {
            if !self.busy.swap(true, std::sync::atomic::Ordering::Acquire) {
                break;
            }
            if rayon::yield_now().is_none() {
                std::thread::yield_now();
            }
        }
        let r = self.at_locked(f);
        self.busy.store(false, std::sync::atomic::Ordering::Release);
        r
    }

    fn at_locked(&self, f: i64) -> Arc<Snap> {
        let mut st = self.state.lock().unwrap();
        if let Some(s) = st.snaps.get(&f) {
            return s.clone();
        }
        let clock = std::time::Instant::now();
        let mut live = match st.live.take() {
            Some(l) if l.frame <= f => l,
            other => {
                // Behind: restart from the latest checkpoint at or before `f`.
                let cp = st.checkpoints.iter().filter(|c| c.frame <= f).max_by_key(|c| c.frame).cloned();
                match cp {
                    Some(c) => c,
                    None => {
                        drop(other);
                        self.init()
                    }
                }
            }
        };
        let s0 = st.steps;
        while live.frame < f {
            self.step_frame(&mut live);
            st.steps += 1;
            if live.frame % CHECK_EVERY == 0 && !st.checkpoints.iter().any(|c| c.frame == live.frame) && (st.checkpoints.len() + 1) * self.def.count * 24 < CHECK_BUDGET {
                st.checkpoints.push(live.clone());
            }
            // Frames close behind the target are likely asked for next (render batches).
            if f - live.frame <= 24 && live.frame < f {
                let s = Arc::new(self.snap(&live));
                st.snaps.insert(live.frame, s);
                st.order.push(live.frame);
            }
        }
        let s = Arc::new(self.snap(&live));
        st.snaps.insert(f, s.clone());
        st.order.push(f);
        st.live = Some(live);
        // Evict the oldest snapshots over budget.
        let per = s.bytes().max(1);
        while st.order.len() * per > SNAP_BUDGET && st.order.len() > 1 {
            let k = st.order.remove(0);
            st.snaps.remove(&k);
        }
        st.sim_ms += clock.elapsed().as_secs_f64() * 1e3;
        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() && st.steps > s0 {
            eprintln!("particles: {} × {} frames in {:.0} ms", self.def.count, st.steps - s0, clock.elapsed().as_secs_f64() * 1e3);
        }
        s
    }

    /// Colour (linear) and opacity of particle `i` at `t`, given its speed.
    #[inline]
    fn look(&self, i: usize, t: f32, p: V3, speed: f32) -> ([f32; 3], f32) {
        let d = &self.def;
        let age = self.age(i, t);
        if age < 0.0 {
            return ([0.0; 3], 0.0);
        }
        let x = match d.color_by {
            ColorBy::Random => hu(d.seed, i as u32, 41),
            ColorBy::Age => age,
            ColorBy::Speed => ((speed - d.speed_range.0) / (d.speed_range.1 - d.speed_range.0).max(1e-6)).clamp(0.0, 1.0),
            ColorBy::Height => (p.y / 3.0).clamp(0.0, 1.0),
            ColorBy::Source => 0.0,
        };
        let c = if let (ColorBy::Source, Some(sc)) = (d.color_by, &self.src_col) {
            sc[i]
        } else if d.colors.len() == 1 {
            [d.colors[0].x, d.colors[0].y, d.colors[0].z]
        } else {
            let f = x * (d.colors.len() - 1) as f32;
            let k = (f.floor() as usize).min(d.colors.len() - 2);
            let c = lerp3(d.colors[k], d.colors[k + 1], f - k as f32);
            [c.x, c.y, c.z]
        };
        let a = match d.life {
            Some(_) => smoothstep(0.0, 0.08, age) * (1.0 - smoothstep(0.75, 1.0, age)),
            None => 1.0,
        };
        (c, a * d.opacity)
    }
}

// ------------------------------------------------------------------ rendering

/// Particle light for one frame: emission, and (for `over`) absorbed
/// density with its colour. Composited by `post` before bloom.
pub struct Layer {
    pub w: usize,
    pub h: usize,
    /// Emitted light (linear RGB).
    pub emit: Vec<[f32; 3]>,
    /// Premultiplied colour and optical depth of absorbing particles.
    pub col: Vec<[f32; 4]>,
    pub any_over: bool,
}

impl Layer {
    /// Composite over the scene's HDR image.
    #[inline]
    pub fn apply(&self, i: usize, c: [f32; 3]) -> [f32; 3] {
        let e = self.emit[i];
        if !self.any_over {
            return [c[0] + e[0], c[1] + e[1], c[2] + e[2]];
        }
        let q = self.col[i];
        if q[3] <= 1e-6 {
            return [c[0] + e[0], c[1] + e[1], c[2] + e[2]];
        }
        let tr = (-q[3]).exp();
        let k = (1.0 - tr) / q[3];
        [c[0] * tr + q[0] * k + e[0], c[1] * tr + q[1] * k + e[1], c[2] * tr + q[2] * k + e[2]]
    }
}

/// Pyramid level buffers: 8 floats per texel (emit rgb, premult rgb, density, pad).
struct Pyr {
    lv: Vec<(usize, usize, Vec<[f32; 8]>)>,
}

impl Pyr {
    fn new(w: usize, h: usize, levels: usize) -> Pyr {
        let mut lv = Vec::new();
        let (mut cw, mut ch) = (w, h);
        for _ in 0..levels {
            lv.push((cw, ch, vec![[0.0f32; 8]; cw * ch]));
            cw = cw.div_ceil(2).max(1);
            ch = ch.div_ceil(2).max(1);
        }
        Pyr { lv }
    }
    /// Bilinear splat at level `l`, pixel coords of level 0.
    #[inline(always)]
    fn splat(&mut self, l: usize, x: f32, y: f32, v: &[f32; 8]) {
        let (w, h, buf) = &mut self.lv[l];
        let f = 1.0 / (1u32 << l) as f32;
        let (fx, fy) = (x * f - 0.5, y * f - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i32, iy as i32);
        let (w, h) = (*w as i32, *h as i32);
        for (dx, dy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            let (xx, yy) = (ix + dx, iy + dy);
            if xx < 0 || yy < 0 || xx >= w || yy >= h {
                continue;
            }
            let o = &mut buf[(yy * w + xx) as usize];
            for k in 0..7 {
                o[k] += v[k] * wt;
            }
        }
    }
}

/// Everything the splatter needs from the frame.
pub struct View<'a> {
    pub cam: &'a Camera,
    pub prev: &'a Camera,
    pub w: usize,
    pub h: usize,
    /// Scene depth per pixel (distance along the camera axis).
    pub depth: &'a [f32],
    /// DOF: focus distance, aperture (px at 1080p for infinity).
    pub focus: f32,
    pub aperture: f32,
    pub shutter: f32,
    /// Lighting for lit sprites: key direction and irradiance, ambient (up / down).
    pub key: (V3, V3),
    pub amb: (V3, V3),
}

struct Proj {
    f: V3,
    r: V3,
    u: V3,
    pos: V3,
    k: f32,
    cx: f32,
    cy: f32,
}

impl Proj {
    fn new(c: &Camera, w: usize, h: usize) -> Proj {
        let (f, r, u) = c.basis();
        let th = (c.fov * 0.5).to_radians().tan();
        Proj { f, r, u, pos: c.pos, k: h as f32 * 0.5 / th, cx: w as f32 * 0.5, cy: h as f32 * 0.5 }
    }
    /// Screen position (px) and view depth.
    #[inline(always)]
    fn at(&self, p: V3) -> (f32, f32, f32) {
        let d = sub(p, self.pos);
        let z = dot(d, self.f);
        let iz = 1.0 / z.max(1e-4);
        (self.cx + dot(d, self.r) * self.k * iz, self.cy - dot(d, self.u) * self.k * iz, z)
    }
}

/// Splat every live system into one layer.
pub fn render(systems: &[(&Sim, Arc<Snap>)], v: &View, t: f32) -> Layer {
    let (w, h) = (v.w, v.h);
    let levels = 6;
    let s = h as f32 / 1080.0;
    let pj = Proj::new(v.cam, w, h);
    let pp = Proj::new(v.prev, w, h);
    let lvl_of = |r: f32| -> usize { (r.max(1.0).log2().floor() as usize).min(levels - 1) };
    let mut any_over = false;
    let threads = if rayon::current_thread_index().is_some() { 1 } else { rayon::current_num_threads().max(1) };
    let mut pyrs: Vec<Pyr> = Vec::new();
    for (sim, snap) in systems {
        let d = &sim.def;
        any_over |= d.blend == Blend::Over;
        let n = snap.pos.len();
        let per = n.div_ceil(threads).max(1);
        let work = |range: std::ops::Range<usize>| -> Pyr {
            let mut py = Pyr::new(w, h, levels);
            for i in range {
                let p = v3(snap.pos[i][0], snap.pos[i][1], snap.pos[i][2]);
                let (sx, sy, z) = pj.at(p);
                if z < 0.05 {
                    continue;
                }
                let size = d.size * (1.0 + d.size_var * (hu(d.seed, i as u32, 7) - 0.5) * 2.0);
                let r_px = size * pj.k / z;
                let coc = if v.aperture > 0.05 { (v.aperture * s * (1.0 - v.focus / z.min(v.focus * 8.0)).abs()).min(28.0 * s) } else { 0.0 };
                let foot = (r_px * r_px + coc * coc).sqrt();
                let margin = foot + 4.0;
                if sx < -margin || sy < -margin || sx > w as f32 + margin || sy > h as f32 + margin {
                    continue;
                }
                // Depth test against the scene at the particle's centre.
                let (ix, iy) = (sx as i32, sy as i32);
                if ix >= 0 && iy >= 0 && (ix as usize) < w && (iy as usize) < h && v.depth[iy as usize * w + ix as usize] < z - size * 2.0 {
                    continue;
                }
                let vel = snap.vel(i);
                let speed = len(vel);
                let (col, a) = sim.look(i, t, p, speed);
                if a <= 0.0 {
                    continue;
                }
                // Screen motion over the shutter: where the point was half a shutter ago.
                let back = madd(p, vel, -v.shutter * sim.dt * d.stretch);
                let (bx, by, _) = pp.at(back);
                let (bx, by) = (sx + (bx - sx) * if v.shutter > 0.0 { 1.0 } else { 0.0 }, sy + (by - sy) * if v.shutter > 0.0 { 1.0 } else { 0.0 });
                let (mx, my) = (sx - bx, sy - by);
                let ml = (mx * mx + my * my).sqrt();
                let l = lvl_of(foot);
                let tex = (1u32 << l) as f32;
                let ns = ((ml / tex).ceil() as usize).clamp(1, 24);
                // Energy: projected disc area (world size) — in focus it's smaller than a pixel.
                let area = std::f32::consts::PI * r_px * r_px;
                // Lit colour.
                let lc = if d.lit {
                    let to_cam = norm(sub(v.cam.pos, p));
                    let ca = dot(to_cam, v.key.0).clamp(-1.0, 1.0);
                    let al = ca.acos();
                    let phase = (al.sin() + (std::f32::consts::PI - al) * ca) / std::f32::consts::PI;
                    let amb = lerp3(v.amb.1, v.amb.0, 0.5 + 0.5 * to_cam.y.max(0.0));
                    let l = add(scale(v.key.1, phase * 0.66), amb);
                    [col[0] * l.x, col[1] * l.y, col[2] * l.z]
                } else {
                    col
                };
                let mut val = [0.0f32; 8];
                match d.blend {
                    Blend::Add => {
                        let e = d.emissive * a * area.min(4.0);
                        val[0] = (col[0] * d.emissive * 0.0 + lc[0]) * e;
                        val[1] = lc[1] * e;
                        val[2] = lc[2] * e;
                    }
                    Blend::Over => {
                        // Optical depth: covered fraction of a pixel.
                        let od = a * area.min(4.0);
                        val[3] = lc[0] * od;
                        val[4] = lc[1] * od;
                        val[5] = lc[2] * od;
                        val[6] = od;
                        let e = d.emissive * od;
                        val[0] = col[0] * e;
                        val[1] = col[1] * e;
                        val[2] = col[2] * e;
                    }
                }
                let inv = 1.0 / ns as f32;
                let vv = val.map(|x| x * inv);
                for k in 0..ns {
                    let u = if ns == 1 { 0.0 } else { (k as f32 + 0.5) * inv - 0.5 };
                    py.splat(l, sx + mx * u - mx * 0.0, sy + my * u, &vv);
                }
            }
            py
        };
        if threads == 1 {
            pyrs.push(work(0..n));
        } else {
            use rayon::prelude::*;
            let parts: Vec<Pyr> = (0..threads).into_par_iter().map(|k| work(k * per..((k + 1) * per).min(n))).collect();
            pyrs.extend(parts);
        }
    }
    // Reduce the per-thread pyramids, then collapse levels (coarse to fine, bilinear up).
    let mut acc = match pyrs.pop() {
        Some(p) => p,
        None => Pyr::new(w, h, levels),
    };
    for p in pyrs {
        for (a, b) in acc.lv.iter_mut().zip(p.lv) {
            for (x, y) in a.2.iter_mut().zip(b.2) {
                for k in 0..7 {
                    x[k] += y[k];
                }
            }
        }
    }
    for l in (1..levels).rev() {
        let (cw, ch, src) = {
            let (cw, ch, b) = &acc.lv[l];
            (*cw, *ch, b.clone())
        };
        let (fw, fh, dst) = &mut acc.lv[l - 1];
        for y in 0..*fh {
            let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, ch as f32 - 1.0);
            let (iy, ty) = (fy as usize, fy - fy.floor());
            let iy1 = (iy + 1).min(ch - 1);
            for x in 0..*fw {
                let fx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, cw as f32 - 1.0);
                let (ix, tx) = (fx as usize, fx - fx.floor());
                let ix1 = (ix + 1).min(cw - 1);
                let (a, b, c, e) = (&src[iy * cw + ix], &src[iy * cw + ix1], &src[iy1 * cw + ix], &src[iy1 * cw + ix1]);
                let o = &mut dst[y * *fw + x];
                // Values are per texel; a coarse texel spreads over 4 fine ones.
                for k in 0..7 {
                    o[k] += ((a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + e[k] * tx) * ty) * 0.25;
                }
            }
        }
    }
    let base = &acc.lv[0].2;
    Layer {
        w,
        h,
        emit: base.iter().map(|q| [q[0], q[1], q[2]]).collect(),
        col: base.iter().map(|q| [q[3], q[4], q[5], q[6]]).collect(),
        any_over,
    }
}
