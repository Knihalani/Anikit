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
    "type", "name", "count", "seed", "emit", "velocity", "spread", "normal_speed", "life", "flow", "attractors", "gravity", "drag", "target", "collide", "bounce", "friction", "size", "size_var", "color", "color_by", "speed_range", "opacity", "emissive", "blend", "lit", "start", "end", "substeps", "stretch", "shadows", "force",
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
    /// Custom: position (and optionally velocity) as expressions of `i`, `age` (the spawn cycle), `t`.
    Expr { pos: Arc<[PExpr; 3]>, vel: Option<Arc<[PExpr; 3]>> },
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
    /// Absorbing particles shadow each other and the scene.
    pub shadows: bool,
    /// Hash of the description (disk cache key).
    key: u64,
    /// Custom acceleration field, size, colour and opacity per particle.
    force: Option<Arc<[PExpr; 3]>>,
    size_e: Option<PExpr>,
    color_e: Option<Arc<[PExpr; 3]>>,
    opacity_e: Option<PExpr>,
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
    // Custom spawn rule: "pos": "=[…]" expressions.
    if let Some(Value::String(ps)) = m.get("pos").or(m.get("position")) {
        spec::check_keys(m, &["pos", "position", "vel", "velocity"], path)?;
        let vel = m.get("vel").or(m.get("velocity")).map(|x| pexpr3(x, &format!("{path}.vel"))).transpose()?.map(Arc::new);
        return Ok(Emit::Expr { pos: Arc::new(pexpr3(&Value::String(ps.clone()), &format!("{path}.pos"))?), vel });
    }
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
    if od.parts.len() == 1 && matches!(shapes.list[od.parts[0].0 as usize], Shape::Sphere) {
        return Ok(Emit::Sphere { c: xf.t, r: xf.smax(), surface: true });
    }
    let mut tris: Vec<[V3; 3]> = Vec::new();
    for p in &od.parts {
        if let Shape::Mesh(b) = &shapes.list[p.0 as usize] {
            tris.extend(b.tris.iter().map(|t| [xf.point(t.v0), xf.point(add(t.v0, t.e1)), xf.point(add(t.v0, t.e2))]));
        }
    }
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
    let color_e = match m.get("color") {
        Some(Value::String(c)) if c.trim_start().starts_with('=') => Some(Arc::new(pexpr3(&Value::String(c.clone()), &format!("{path}.color"))?)),
        _ => None,
    };
    let colors = match m.get("color") {
        None => vec![v3(1.0, 1.0, 1.0)],
        Some(_) if color_e.is_some() => vec![v3(1.0, 1.0, 1.0)],
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
        size: match m.get("size") {
            Some(Value::String(_)) => 0.004,
            _ => gf(m, "size", 0.004, path)?.max(1e-5),
        },
        size_var: gf(m, "size_var", 0.5, path)?.clamp(0.0, 1.0),
        colors,
        color_by,
        speed_range: sr,
        opacity: match m.get("opacity") {
            Some(Value::String(_)) => 1.0,
            _ => gf(m, "opacity", 1.0, path)?.clamp(0.0, 1.0),
        },
        emissive: gf(m, "emissive", if blend == Blend::Add { 1.0 } else { 0.0 }, path)?,
        blend,
        lit: m.get("lit").and_then(|x| x.as_bool()).unwrap_or(blend == Blend::Over),
        start: gf(m, "start", 0.0, path)?,
        end: gf(m, "end", f32::INFINITY, path)?,
        substeps: gf(m, "substeps", 2.0, path)?.clamp(1.0, 16.0) as u32,
        stretch: gf(m, "stretch", 1.0, path)?.max(0.0),
        shadows: m.get("shadows").and_then(|x| x.as_bool()).unwrap_or(blend == Blend::Over),
        force: m.get("force").map(|x| pexpr3(x, &format!("{path}.force"))).transpose()?.map(Arc::new),
        size_e: match m.get("size") {
            Some(Value::String(x)) => Some(PExpr::compile(x, &format!("{path}.size"))?),
            _ => None,
        },
        color_e,
        opacity_e: match m.get("opacity") {
            Some(Value::String(x)) => Some(PExpr::compile(x, &format!("{path}.opacity"))?),
            _ => None,
        },
        key: {
            // An image emitter's file can change under the same spec.
            let stamp = emit_v.get("image").and_then(|x| x.as_str()).and_then(|f| std::fs::metadata(f).ok()).and_then(|m| m.modified().ok()).map(|t| format!("{t:?}")).unwrap_or_default();
            crate::components::style::fnv(&[b"anikit-particles-v1", v.to_string().as_bytes(), emit_v.to_string().as_bytes(), stamp.as_bytes()])
        },
    })
}

// ------------------------------------------------------------------ expressions

/// A per-particle expression. Variables: `x`, `y`, `z` (position, m), `vx`,
/// `vy`, `vz` (velocity, m/s), `t` (seconds), `age` (0–1 over a life, or
/// seconds / 4 without `life`), `i` (particle id), plus every expression
/// function (`sin`, `noise`, `rand`, `clamp`, `mix` …). In emitters, `i` is the
/// particle and `age` the spawn cycle.
#[derive(Clone)]
pub struct PExpr(Arc<crate::eval::expr::Expr>);

/// Particle variables onto the expression engine's slots.
const PVARS: &[(&str, &str)] = &[("vx", "T"), ("vy", "gt"), ("vz", "p"), ("age", "n"), ("x", "w"), ("y", "h"), ("z", "fps")];

impl PExpr {
    fn compile(src: &str, path: &str) -> Res<PExpr> {
        let src = src.strip_prefix('=').unwrap_or(src);
        let b = src.as_bytes();
        let mut out = String::with_capacity(src.len());
        let mut i = 0;
        'scan: while i < b.len() {
            let start_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
            if start_ok {
                for (from, to) in PVARS {
                    let e = i + from.len();
                    if src[i..].starts_with(from) && b.get(e).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_')) {
                        out.push_str(to);
                        i = e;
                        continue 'scan;
                    }
                }
            }
            out.push(b[i] as char);
            i += 1;
        }
        crate::eval::expr::Expr::compile(&out, 1.0, 1920.0, 1080.0).map(|e| PExpr(Arc::new(e))).map_err(|e| format!("{path}: expression `{src}`: {e}"))
    }

    #[inline]
    fn at(&self, p: V3, v: V3, t: f32, age: f32, i: usize) -> f32 {
        let vars = Vars { t: t as f64, scene_t: v.x as f64, gt: v.y as f64, p: v.z as f64, i: i as f64, n: age as f64, w: p.x as f64, h: p.y as f64, fps: p.z as f64, ..Vars::default() };
        self.0.eval(&vars) as f32
    }
}

/// `"=[a, b, c]"`: three per-particle expressions.
fn pexpr3(v: &Value, path: &str) -> Res<[PExpr; 3]> {
    let s = v.as_str().ok_or_else(|| format!("{path}: expected \"=[x, y, z]\" (expressions of x, y, z, vx, vy, vz, t, age, i)"))?;
    let body = s.strip_prefix('=').unwrap_or(s).trim();
    let inner = body.strip_prefix('[').and_then(|b| b.strip_suffix(']')).ok_or_else(|| format!("{path}: expected \"=[x, y, z]\""))?;
    let mut parts = Vec::new();
    let (mut depth, mut cur) = (0, String::new());
    for c in inner.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                cur.push(c)
            }
            ')' | ']' => {
                depth -= 1;
                cur.push(c)
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    if parts.len() != 3 {
        return Err(format!("{path}: expected 3 expressions, got {}", parts.len()));
    }
    Ok([PExpr::compile(&parts[0], path)?, PExpr::compile(&parts[1], path)?, PExpr::compile(&parts[2], path)?])
}

#[inline]
fn eval3p(e: &[PExpr; 3], p: V3, v: V3, t: f32, age: f32, i: usize) -> V3 {
    v3(e[0].at(p, v, t, age, i), e[1].at(p, v, t, age, i), e[2].at(p, v, t, age, i))
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
        Emit::Expr { pos, .. } => (eval3p(pos, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0), 0.0, k as f32, i as usize), v3(0.0, 1.0, 0.0), None),
    }
}

/// Resolve image / composition emitters (needs the spec's compositions).
fn resolve(e: &Emit, assets: Option<&dyn crate::components::Assets>) -> Res<Emit> {
    let Emit::Pending(m) = e else { return Ok(e.clone()) };
    let path = "particles.emit";
    let (w, h, px): (usize, usize, Vec<[f32; 4]>) = if let Some(file) = m.get("image").and_then(|x| x.as_str()) {
        let img = image::open(file).map_err(|err| format!("{path}.image: can't read `{file}`: {err}"))?.to_rgba32f();
        let (w, h) = (img.width() as usize, img.height() as usize);
        (w, h, img.pixels().map(|p| p.0).collect())
    } else {
        let name = m.get("composition").and_then(|x| x.as_str()).unwrap_or("").trim_start_matches('#');
        let info = assets.and_then(|a| a.composition_info(name)).ok_or_else(|| format!("{path}.composition: no composition `{name}`"))?;
        let ct = m.get("frame").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32 / info.fps.max(1.0);
        let pm = assets.and_then(|a| a.composition_frame(name, ct, 0)).ok_or_else(|| format!("{path}.composition: can't render `{name}`"))?;
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
    let xf = Xf::new(c, rot, v3(1.0, 1.0, 1.0));
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
        let v = &self.v[..FN * FN * FN];
        let mut o = [0.0f32; 3];
        // SAFETY: every index is masked to the grid (x, y, z < FN).
        let at = |k: usize| unsafe { *v.get_unchecked(k) };
        let (a, b, c, d) = (at(z0 + y0 + x0), at(z0 + y0 + x1), at(z0 + y1 + x0), at(z0 + y1 + x1));
        let (e, f, g, h) = (at(z1 + y0 + x0), at(z1 + y0 + x1), at(z1 + y1 + x0), at(z1 + y1 + x1));
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
    /// Particle ids (slot `j` holds particle `id[j]`; slots are kept in spatial
    /// order). Shared with the live state until the next re-sort.
    pub id: Arc<Vec<u32>>,
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
        self.pos.len() * 22
    }
}

struct Live {
    frame: i64,
    id: Arc<Vec<u32>>,
    px: Vec<f32>,
    py: Vec<f32>,
    pz: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    vz: Vec<f32>,
}

impl Clone for Live {
    fn clone(&self) -> Live {
        Live { frame: self.frame, id: self.id.clone(), px: self.px.clone(), py: self.py.clone(), pz: self.pz.clone(), vx: self.vx.clone(), vy: self.vy.clone(), vz: self.vz.clone() }
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
    /// Target points, arrival delays, and surface normals (assembled particles shade like the surface).
    tgt: Option<Targets>,
    flows: Option<(FlowGrid, FlowGrid)>,
    floor_y: Option<f32>,
    /// Scene objects particles bounce off (`"collide": true`).
    colliders: Vec<ObjDef>,
    /// Disk checkpoint key: the description, fps and floor.
    key: u64,
    state: Mutex<SimState>,
    busy: std::sync::atomic::AtomicBool,
    /// Every `stride`-th particle is simulated (previews); see `set_stride`.
    pub stride: usize,
}

type Targets = (Vec<[f32; 3]>, Vec<f32>, Vec<[f32; 3]>);

const CHECK_EVERY: i64 = 60;
const SORT_EVERY: i64 = 30;
const SNAP_BUDGET: usize = 3 << 30;
const CHECK_BUDGET: usize = 2 << 30;

impl Sim {
    pub fn new(def: &Def, fps: f32, floor_y: Option<f32>, assets: Option<&dyn crate::components::Assets>) -> Res<Sim> {
        let emit = resolve(&def.emit, assets)?;
        let target = def.target.as_ref().map(|t| resolve(&t.emit, assets)).transpose()?;
        let n = def.count;
        let src_col = match &emit {
            Emit::Image { .. } => Some((0..n).map(|i| sample(&emit, def.seed, i as u32, 0).2.unwrap_or([1.0; 3])).collect()),
            _ => None,
        };
        let tgt = target.as_ref().map(|t| {
            use rayon::prelude::*;
            let (pts, nrm): (Vec<[f32; 3]>, Vec<[f32; 3]>) = (0..n).into_par_iter().map(|i| {
                let (p, q, _) = sample(t, def.seed ^ 0x7a3c, i as u32, 0);
                ([p.x, p.y, p.z], [q.x, q.y, q.z])
            }).unzip();
            // Staggered arrival: lower target points settle first.
            let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
            let delay = pts.iter().enumerate().map(|(i, p)| 0.55 * (p[1] - lo) / (hi - lo).max(1e-6) + 0.15 * hu(def.seed, i as u32, 99)).collect();
            (pts, delay, nrm)
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
            colliders: Vec::new(),
            key: crate::components::style::fnv(&[&def.key.to_le_bytes(), &fps.to_bits().to_le_bytes(), &floor_y.unwrap_or(f32::NAN).to_bits().to_le_bytes()]),
            state: Mutex::new(SimState { live: None, snaps: HashMap::new(), order: Vec::new(), checkpoints: Vec::new(), steps: 0, sim_ms: 0.0 }),
            busy: std::sync::atomic::AtomicBool::new(false),
            stride: 1,
        })
    }

    /// Preview: simulate every `k`-th particle only (exactly the same paths:
    /// particles don't interact), each drawn with `k` times the weight. Call
    /// before the first frame.
    pub fn set_stride(&mut self, k: usize) {
        let k = k.max(1);
        if k != self.stride {
            self.stride = k;
            self.key = crate::components::style::fnv(&[&self.key.to_le_bytes(), &(k as u64).to_le_bytes()]);
        }
    }

    /// Particles actually simulated.
    fn n_sim(&self) -> usize {
        self.def.count.div_ceil(self.stride)
    }

    /// Objects particles collide with: spheres exactly, everything else as
    /// its oriented bounding box. Animated objects push particles along.
    pub fn set_colliders(&mut self, objs: Vec<ObjDef>) {
        if self.def.collide {
            self.colliders = objs;
        }
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
        let mut v = add(add(d.vel, scale(n, d.normal_speed)), scale(v3(r(11), r(12), r(13)), d.spread));
        if let Emit::Expr { vel: Some(e), .. } = &self.emit {
            v = add(v, eval3p(e, p, v3(0.0, 0.0, 0.0), self.birth(i), k as f32, i));
        }
        (p, v)
    }

    fn init(&self) -> Live {
        use rayon::prelude::*;
        let (n, k) = (self.n_sim(), self.stride);
        let pv: Vec<(V3, V3)> = (0..n).into_par_iter().map(|i| self.spawn(i * k, 0)).collect();
        let mut live = Live {
            frame: 0,
            id: Arc::new((0..n).map(|i| (i * k) as u32).collect()),
            px: pv.iter().map(|q| q.0.x).collect(),
            py: pv.iter().map(|q| q.0.y).collect(),
            pz: pv.iter().map(|q| q.0.z).collect(),
            vx: pv.iter().map(|q| q.1.x).collect(),
            vy: pv.iter().map(|q| q.1.y).collect(),
            vz: pv.iter().map(|q| q.1.z).collect(),
        };
        self.sort(&mut live);
        live
    }

    /// Reorder slots along a Morton curve of position, so particles near each
    /// other in space (and on screen) are near each other in memory: the
    /// splatter's writes and the flow lookups then stay in cache (1.4–1.7×
    /// faster at 10M). Deterministic: ties break on the particle id.
    fn sort(&self, live: &mut Live) {
        use rayon::prelude::*;
        let n = live.px.len();
        if n < 2 {
            return;
        }
        let inf = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        let (lo, hi) = (0..n).into_par_iter().with_min_len(1 << 14).map(|i| {
            let p = v3(live.px[i], live.py[i], live.pz[i]);
            (p, p)
        }).reduce(|| inf, |(a0, a1), (b0, b1)| (vmin(a0, b0), vmax(a1, b1)));
        let ext = sub(hi, lo);
        let k = v3(1023.999 / ext.x.max(1e-6), 1023.999 / ext.y.max(1e-6), 1023.999 / ext.z.max(1e-6));
        let spread = |v: u64| -> u64 {
            let mut x = v & 0x3ff;
            x = (x | (x << 16)) & 0x0300_00ff;
            x = (x | (x << 8)) & 0x0300_f00f;
            x = (x | (x << 4)) & 0x030c_30c3;
            (x | (x << 2)) & 0x0924_9249
        };
        let mut keys: Vec<(u32, u32)> = (0..n).into_par_iter().with_min_len(1 << 14).map(|i| {
            let q = |v: f32, o: f32, s: f32| (((v - o) * s).max(0.0) as u64).min(1023);
            let m = spread(q(live.px[i], lo.x, k.x)) | spread(q(live.py[i], lo.y, k.y)) << 1 | spread(q(live.pz[i], lo.z, k.z)) << 2;
            (m as u32, i as u32)
        }).collect();
        let id = &live.id;
        keys.par_sort_unstable_by_key(|&(m, i)| (m, id[i as usize]));
        let gather = |a: &Vec<f32>| -> Vec<f32> { keys.par_iter().with_min_len(1 << 14).map(|&(_, i)| a[i as usize]).collect() };
        let (px, py, pz, vx, vy, vz) = (gather(&live.px), gather(&live.py), gather(&live.pz), gather(&live.vx), gather(&live.vy), gather(&live.vz));
        live.id = Arc::new(keys.par_iter().with_min_len(1 << 14).map(|&(_, i)| live.id[i as usize]).collect());
        (live.px, live.py, live.pz, live.vx, live.vy, live.vz) = (px, py, pz, vx, vy, vz);
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
        // Collider instances per substep.
        enum PCol {
            Ball(V3, f32),
            Obb(Xf, V3, V3),
        }
        let cols: Vec<Vec<PCol>> = (0..sub)
            .map(|s| {
                let t = t0 + (s as f32 + 1.0) * h;
                let mut v = Vec::new();
                for o in &self.colliders {
                    for lane in 0..o.lanes {
                        for i in 0..o.count {
                            let xf = o.xf_at(t, i, lane);
                            v.push(match &o.col {
                                spec::Col::Ball => PCol::Ball(xf.t, xf.smax()),
                                spec::Col::Cuboid { c, h } => PCol::Obb(xf, *c, *h),
                                spec::Col::Cylinder { c, hh, r } => PCol::Obb(xf, *c, v3(*r, *hh, *r)),
                                spec::Col::Hull(pts) => {
                                    let (lo, hi) = pts.iter().fold((v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN)), |(a, b), q| (vmin(a, *q), vmax(b, *q)));
                                    PCol::Obb(xf, scale(add(lo, hi), 0.5), scale(super::math::sub(hi, lo), 0.5))
                                }
                            });
                        }
                    }
                }
                v
            })
            .collect();
        let att: Vec<Vec<(V3, f32, f32, f32)>> = (0..sub)
            .map(|s| {
                let t = t0 + (s as f32 + 0.5) * h;
                let vars = Vars { t: t as f64, gt: t as f64, ..Vars::default() };
                d.attractors.iter().map(|a| (spec::eval3(&a.pos, &vars), a.strength, a.swirl, a.radius)).collect()
            })
            .collect();
        let Live { id, px, py, pz, vx, vy, vz, .. } = live;
        // The two flow fields cross-faded per substep, once for all particles.
        let blended: Vec<FlowGrid> = match &self.flows {
            Some((fa, fb)) => (0..sub)
                .map(|k| {
                    let t = t0 + k as f32 * h;
                    let a = t * d.flow.map(|f| f.2).unwrap_or(0.0) * std::f32::consts::FRAC_PI_2;
                    let (wa, wb) = (a.cos(), a.sin());
                    FlowGrid { v: fa.v.par_iter().zip(&fb.v).map(|(x, y)| [x[0] * wa + y[0] * wb, x[1] * wa + y[1] * wb, x[2] * wa + y[2] * wb]).collect() }
                })
                .collect(),
            None => Vec::new(),
        };
        let (fs, fscale, _fevo, fdrift) = d.flow.unwrap_or((0.0, 1.0, 0.0, v3(0.0, 0.0, 0.0)));
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
            .zip(id.par_chunks(chunk))
            .for_each(|((((((px, py), pz), vx), vy), vz), ids)| {
                for s in 0..sub {
                    let t = t0 + s as f32 * h;
                    let te = t + h;
                    let flow = blended.get(s as usize);
                    let off = scale(fdrift, -t);
                    let dampf = (-drag * h).exp();
                    let spring = target.map(|tg| ((t - tg.from) / (tg.to - tg.from)).clamp(-1.0, 2.0));
                    for j in 0..px.len() {
                        let i = ids[j] as usize;
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
                        if let Some(fe) = &d.force {
                            let age = match life {
                                Some(l) => ((t - (d.start + hu(d.seed, i as u32, 17) * l)) / l).rem_euclid(1.0),
                                None => ((t - d.start) / 4.0).clamp(0.0, 0.999),
                            };
                            let f = eval3p(fe, v3(x, y, z), v3(vx[j], vy[j], vz[j]), t, age, i);
                            ax += f.x;
                            ay += f.y;
                            az += f.z;
                        }
                        let (mut ux, mut uy, mut uz) = (vx[j], vy[j], vz[j]);
                        if let Some(fg) = flow {
                            let a = fg.sample((x + off.x) * inv, (y + off.y) * inv, (z + off.z) * inv);
                            // Relax towards the flow velocity.
                            let k = (1.0 - dampf) * 4.0;
                            ux += (a[0] * fs - ux) * k;
                            uy += (a[1] * fs - uy) * k;
                            uz += (a[2] * fs - uz) * k;
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
                        if let (Some(sp), Some((tp, delay, _))) = (spring, tgt) {
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
                        for c in &cols[s as usize] {
                            // Push out along the surface normal; bounce and slide.
                            let (n, pen) = match c {
                                PCol::Ball(cc, r) => {
                                    let dv = super::math::sub(v3(nx, ny, nz), *cc);
                                    let l = len(dv);
                                    let rr = r + d.size;
                                    if l >= rr || l < 1e-9 {
                                        continue;
                                    }
                                    (scale(dv, 1.0 / l), rr - l)
                                }
                                PCol::Obb(xf, cc, hh) => {
                                    let lp = super::math::sub(xf.point_inv(v3(nx, ny, nz)), *cc);
                                    let (dx, dy, dz) = ((hh.x - lp.x.abs()) * xf.s.x, (hh.y - lp.y.abs()) * xf.s.y, (hh.z - lp.z.abs()) * xf.s.z);
                                    if dx <= 0.0 || dy <= 0.0 || dz <= 0.0 {
                                        continue;
                                    }
                                    let (ln, pen) = if dx <= dy && dx <= dz {
                                        (v3(lp.x.signum(), 0.0, 0.0), dx)
                                    } else if dy <= dz {
                                        (v3(0.0, lp.y.signum(), 0.0), dy)
                                    } else {
                                        (v3(0.0, 0.0, lp.z.signum()), dz)
                                    };
                                    (norm(xf.rot(ln)), pen)
                                }
                            };
                            let q = madd(v3(nx, ny, nz), n, pen + d.size * 0.5);
                            (nx, ny, nz) = (q.x, q.y, q.z);
                            let u = v3(ux, uy, uz);
                            let vn = dot(u, n);
                            if vn < 0.0 {
                                let tang = scale(super::math::sub(u, scale(n, vn)), 1.0 - d.friction);
                                let u2 = add(tang, scale(n, -vn * d.bounce));
                                (ux, uy, uz) = (u2.x, u2.y, u2.z);
                            }
                        }
                        (px[j], py[j], pz[j], vx[j], vy[j], vz[j]) = (nx, ny, nz, ux, uy, uz);
                    }
                }
            });
        live.frame += 1;
        if live.frame % SORT_EVERY == 0 {
            let c = std::time::Instant::now();
            self.sort(live);
            if std::env::var_os("ANIKIT_PARTICLES_PROFILE").is_some() {
                eprintln!("particles: sort {:.0} ms", c.elapsed().as_secs_f64() * 1e3);
            }
        }
    }

    fn snap(&self, live: &Live) -> Snap {
        use rayon::prelude::*;
        let n = live.px.len();
        const CH: usize = 1 << 15;
        let m = live.vx.par_chunks(CH).zip(live.vy.par_chunks(CH)).zip(live.vz.par_chunks(CH)).map(|((a, b), c)| a.iter().zip(b).zip(c).fold(1e-6f32, |m, ((x, y), z)| m.max(x.abs()).max(y.abs()).max(z.abs()))).reduce(|| 1e-6f32, f32::max);
        let vs = m / 32767.0;
        let iv = 1.0 / vs;
        let q = |v: f32| (v * iv).round().clamp(-32767.0, 32767.0) as i16;
        Snap {
            id: live.id.clone(),
            pos: (0..n).into_par_iter().with_min_len(CH).map(|i| [live.px[i], live.py[i], live.pz[i]]).collect(),
            vel: (0..n).into_par_iter().with_min_len(CH).map(|i| [q(live.vx[i]), q(live.vy[i]), q(live.vz[i])]).collect(),
            vscale: vs,
            t: self.def.start + live.frame as f32 * self.dt,
        }
    }

    /// State at video time `t` (rounded to a frame of this system's fps).
    pub fn at(&self, t: f32) -> Arc<Snap> {
        let f = ((t - self.def.start) / self.dt).round().max(0.0) as i64;
        // Wait for the thread that is stepping. Don't help with rayon work
        // meanwhile (`rayon::yield_now`): a stolen task can be another frame
        // calling `at` on this same Sim. The stepping itself runs on the
        // simulation pool (`scene3d::sim_blocking`), so it always progresses.
        while self.busy.swap(true, std::sync::atomic::Ordering::Acquire) {
            std::thread::yield_now();
        }
        let r = self.at_locked(f);
        self.busy.store(false, std::sync::atomic::Ordering::Release);
        r
    }

    fn at_locked(&self, f: i64) -> Arc<Snap> {
        let mut guard = self.state.lock().unwrap();
        let st: &mut SimState = &mut guard;
        // The parallel stepping runs on the simulation pool while the lock is
        // held (see `scene3d::sim_pool`).
        super::sim_blocking(move || self.at_locked_in(st, f))
    }

    fn at_locked_in(&self, st: &mut SimState, f: i64) -> Arc<Snap> {
        if let Some(s) = st.snaps.get(&f) {
            return s.clone();
        }
        let clock = std::time::Instant::now();
        let mut live = match st.live.take() {
            Some(l) if l.frame <= f => l,
            ahead => {
                // Behind the live state (keep it for later frames): restart
                // from the latest checkpoint at or before `f`, in memory or on disk.
                st.live = ahead;
                let mem = st.checkpoints.iter().filter(|c| c.frame <= f).max_by_key(|c| c.frame).cloned();
                let from_disk = self.disk_checkpoint(f, mem.as_ref().map(|c| c.frame).unwrap_or(0));
                match (mem, from_disk) {
                    (_, Some(d)) => d,
                    (Some(c), None) => c,
                    (None, None) => self.init(),
                }
            }
        };
        let s0 = st.steps;
        let prof = std::env::var_os("ANIKIT_PARTICLES_PROFILE").is_some();
        let (mut t_step, mut t_snap) = (0.0f64, 0.0f64);
        while live.frame < f {
            let c0 = std::time::Instant::now();
            self.step_frame(&mut live);
            t_step += c0.elapsed().as_secs_f64();
            st.steps += 1;
            if live.frame % CHECK_EVERY == 0 && !st.checkpoints.iter().any(|c| c.frame == live.frame) {
                if (st.checkpoints.len() + 1) * self.n_sim() * 24 < CHECK_BUDGET {
                    st.checkpoints.push(live.clone());
                }
                self.save_checkpoint(&live);
            }
            // Frames close behind the target are likely asked for next (render batches).
            if f - live.frame <= 24 && live.frame < f {
                let c1 = std::time::Instant::now();
                let s = Arc::new(self.snap(&live));
                t_snap += c1.elapsed().as_secs_f64();
                st.snaps.insert(live.frame, s);
                st.order.push(live.frame);
            }
        }
        let c1 = std::time::Instant::now();
        let s = Arc::new(self.snap(&live));
        t_snap += c1.elapsed().as_secs_f64();
        if prof {
            eprintln!("particles: step {:.0} ms, snapshots {:.0} ms", t_step * 1e3, t_snap * 1e3);
        }
        st.snaps.insert(f, s.clone());
        st.order.push(f);
        if st.live.as_ref().is_none_or(|l| l.frame <= live.frame) {
            st.live = Some(live);
        }
        // Evict the oldest snapshots over budget.
        let per = s.bytes().max(1);
        while st.order.len() * per > SNAP_BUDGET && st.order.len() > 1 {
            let k = st.order.remove(0);
            st.snaps.remove(&k);
        }
        st.sim_ms += clock.elapsed().as_secs_f64() * 1e3;
        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() && st.steps > s0 {
            eprintln!("particles: {} × {} frames in {:.0} ms", self.n_sim(), st.steps - s0, clock.elapsed().as_secs_f64() * 1e3);
        }
        s
    }

    fn checkpoint_path(&self, frame: i64) -> Option<std::path::PathBuf> {
        Some(super::sim_cache_dir()?.join(format!("parts2-{:016x}-{frame}.bin", self.key)))
    }

    /// Write a checkpoint to the disk cache (exact state; written once, renamed into place).
    fn save_checkpoint(&self, live: &Live) {
        let Some(p) = self.checkpoint_path(live.frame) else { return };
        if p.exists() {
            return;
        }
        let n = live.px.len();
        let mut b = Vec::with_capacity(16 + n * 28);
        b.extend_from_slice(b"AKPART02");
        b.extend_from_slice(&(n as u64).to_le_bytes());
        for v in live.id.iter() {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for a in [&live.px, &live.py, &live.pz, &live.vx, &live.vy, &live.vz] {
            for v in a.iter() {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        super::prune_sim_cache();
        let tmp = p.with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&tmp, &b).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }

    /// The latest disk checkpoint at or before `f` and after `after`.
    fn disk_checkpoint(&self, f: i64, after: i64) -> Option<Live> {
        let mut k = f - f.rem_euclid(CHECK_EVERY);
        while k > after {
            if let Some(b) = self.checkpoint_path(k).and_then(|p| std::fs::read(p).ok()) {
                let n = self.n_sim();
                if b.len() == 16 + n * 28 && &b[..8] == b"AKPART02" && u64::from_le_bytes(b[8..16].try_into().unwrap()) as usize == n {
                    let id: Vec<u32> = b[16..16 + n * 4].chunks_exact(4).map(|x| u32::from_le_bytes(x.try_into().unwrap())).collect();
                    let col = |c: usize| -> Vec<f32> { b[16 + (c + 1) * n * 4..16 + (c + 2) * n * 4].chunks_exact(4).map(|x| f32::from_le_bytes(x.try_into().unwrap())).collect() };
                    if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
                        eprintln!("particles: checkpoint at frame {k} from the cache");
                    }
                    return Some(Live { frame: k, id: Arc::new(id), px: col(0), py: col(1), pz: col(2), vx: col(3), vy: col(4), vz: col(5) });
                }
            }
            k -= CHECK_EVERY;
        }
        None
    }

    /// Opacity of particle `i` at `t` (for shadows).
    #[inline]
    fn look_alpha(&self, i: usize, t: f32) -> (f32, f32) {
        let age = self.age(i, t);
        if age < 0.0 {
            return (age, 0.0);
        }
        let a = match self.def.life {
            Some(_) => smoothstep(0.0, 0.08, age) * (1.0 - smoothstep(0.75, 1.0, age)),
            None => 1.0,
        };
        (age, a * self.def.opacity)
    }

    /// Colour (linear) and opacity of particle `i` at `t`, given its speed.
    #[inline]
    fn look(&self, i: usize, t: f32, p: V3, vel: V3) -> ([f32; 3], f32) {
        let d = &self.def;
        let age = self.age(i, t);
        if age < 0.0 {
            return ([0.0; 3], 0.0);
        }
        let speed = len(vel);
        let x = match d.color_by {
            ColorBy::Random => hu(d.seed, i as u32, 41),
            ColorBy::Age => age,
            ColorBy::Speed => ((speed - d.speed_range.0) / (d.speed_range.1 - d.speed_range.0).max(1e-6)).clamp(0.0, 1.0),
            ColorBy::Height => (p.y / 3.0).clamp(0.0, 1.0),
            ColorBy::Source => 0.0,
        };
        let c = if let Some(e) = &d.color_e {
            let c = eval3p(e, p, vel, t, age, i);
            [c.x.max(0.0), c.y.max(0.0), c.z.max(0.0)]
        } else if let (ColorBy::Source, Some(sc)) = (d.color_by, &self.src_col) {
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
        let op = match &d.opacity_e {
            Some(e) => e.at(p, vel, t, age, i).clamp(0.0, 1.0),
            None => d.opacity,
        };
        (c, a * op)
    }
}

// ------------------------------------------------------------------ rendering

/// Particle light for one frame: emission, and (for `over`) premultiplied
/// colour and optical depth of absorbing particles. Composited by `post`
/// before bloom.
pub struct Layer {
    pub w: usize,
    pub h: usize,
    /// Floats per pixel, and where the emission (rgb) and the absorbing
    /// particles' premultiplied colour plus optical depth (rgba) start.
    pub c: usize,
    pub eo: Option<usize>,
    pub ab: Option<usize>,
    pub buf: Vec<f32>,
}

impl Layer {
    /// Composite over the scene's HDR image.
    #[inline]
    pub fn apply(&self, i: usize, c: [f32; 3]) -> [f32; 3] {
        let b = &self.buf[i * self.c..(i + 1) * self.c];
        let mut o = c;
        if let Some(a) = self.ab {
            let od = b[a + 3];
            if od > 1e-6 {
                let tr = (-od).exp();
                let k = (1.0 - tr) / od;
                o = [c[0] * tr + b[a] * k, c[1] * tr + b[a + 1] * k, c[2] * tr + b[a + 2] * k];
            }
        }
        if let Some(e) = self.eo {
            o = [o[0] + b[e], o[1] + b[e + 1], o[2] + b[e + 2]];
        }
        o
    }

    /// Total emitted light (tests).
    pub fn emitted(&self) -> f64 {
        match self.eo {
            Some(e) => self.buf.chunks_exact(self.c).map(|q| q[e] as f64).sum(),
            None => 0.0,
        }
    }
}

/// Pyramid levels, `c` floats per texel.
struct Pyr {
    c: usize,
    lv: Vec<(usize, usize, Vec<f32>)>,
    /// Levels anything was splatted into (empty ones skip the collapse).
    used: [bool; 8],
    /// Defocus bins (see `BINS`), allocated when first used.
    bins: Vec<Option<Vec<f32>>>,
    /// Rows (full resolution) this pyramid owns; level l owns rows `band >> l`.
    /// Threads split the image into bands, so every texel is summed by one
    /// thread in particle order: byte-identical at any thread count.
    band: (i32, i32),
    /// First row held in each level's buffer (band pyramids hold only their rows).
    off: [i32; 8],
}

/// Defocus bins: disc radius (px at full resolution) and the pyramid level it
/// lives at, so the disc is 2–2.8 texels across there. Particles splat a
/// point into the bins bracketing their blur; each used bin is convolved with
/// its disc once, so out-of-focus particles get real lens bokeh (discs, not
/// Gaussian blobs) at the cost of a point splat.
const BINS: [(f32, usize); 17] = [
    (4.0, 1), (4.757, 1), (5.657, 1), (6.727, 1),
    (8.0, 2), (9.514, 2), (11.31, 2), (13.45, 2),
    (16.0, 3), (19.03, 3), (22.63, 3), (26.91, 3),
    (32.0, 4), (38.05, 4), (45.25, 4), (53.82, 4), (64.0, 4),
];

/// Stamps for small footprints: a disc of radius r (0..4 px in 1/16 steps)
/// seen through the tent pixel filter, at 8×8 sub-pixel phases, as 11×11
/// weights summing to 1. Exact small particles cost one table lookup.
const ST_R: usize = 65;
const ST_P: usize = 16;
const ST_N: usize = 11;
const ST_C: usize = ST_N / 2;

fn stamps() -> &'static (Vec<f32>, Vec<[u8; 4]>) {
    static T: std::sync::OnceLock<(Vec<f32>, Vec<[u8; 4]>)> = std::sync::OnceLock::new();
    // Built sequentially: a rayon loop inside `get_or_init` deadlocks when a
    // worker waiting on this lock steals a task that also needs it (critique
    // and multi-frame renders call this from rayon workers).
    T.get_or_init(|| {
        let per = ST_P * ST_P * ST_N * ST_N;
        let mut t = vec![0.0f32; ST_R * per];
        t.chunks_mut(per).enumerate().for_each(|(ri, t)| {
        {
            let r = ri as f32 / 16.0;
            for py in 0..ST_P {
                for px in 0..ST_P {
                    // Particle centre inside the stamp's middle pixel, at this phase.
                    let (cx, cy) = (ST_C as f32 + (px as f32 + 0.5) / ST_P as f32, ST_C as f32 + (py as f32 + 0.5) / ST_P as f32);
                    let o = (py * ST_P + px) * ST_N * ST_N;
                    let step = (r / 12.0).max(0.02);
                    let m = (r / step).ceil() as i32;
                    let mut tot = 0.0;
                    for j in -m..=m {
                        for i in -m..=m {
                            let (dx, dy) = (i as f32 * step, j as f32 * step);
                            if dx * dx + dy * dy > r * r && !(i == 0 && j == 0) {
                                continue;
                            }
                            // Tent filter: the point's share of each pixel centre.
                            let (qx, qy) = (cx + dx - 0.5, cy + dy - 0.5);
                            let (ix, iy) = (qx.floor(), qy.floor());
                            let (tx, ty) = (qx - ix, qy - iy);
                            for (ox, oy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                                let (x, y) = (ix as i32 + ox, iy as i32 + oy);
                                if (0..ST_N as i32).contains(&x) && (0..ST_N as i32).contains(&y) {
                                    t[o + y as usize * ST_N + x as usize] += wt;
                                }
                            }
                            tot += 1.0;
                        }
                    }
                    for v in &mut t[o..o + ST_N * ST_N] {
                        *v /= tot;
                    }

                }
            }
        }
        });
        // Tight bounds of each stamp's non-zero weights.
        let boxes = (0..ST_R * ST_P * ST_P)
            .map(|k| {
                let o = k * ST_N * ST_N;
                let (mut b, mut q) = ([ST_N as u8, 0, ST_N as u8, 0], false);
                for y in 0..ST_N {
                    for x in 0..ST_N {
                        if t[o + y * ST_N + x] > 1e-7 {
                            b = [b[0].min(x as u8), b[1].max(x as u8), b[2].min(y as u8), b[3].max(y as u8)];
                            q = true;
                        }
                    }
                }
                if q { b } else { [ST_C as u8, ST_C as u8, ST_C as u8, ST_C as u8] }
            })
            .collect();
        (t, boxes)
    })
}

/// Disc radius (texels at its level) of bin `b`: smaller than nominal by the
/// blur its level adds (the point splat and the upsampling tents), so the
/// total matches a disc of the nominal radius through the pixel filter.
fn bin_rho(b: usize) -> f32 {
    let (r, l) = BINS[b];
    let lv = (4f32.powi(l as i32) + (4f32.powi(l as i32 + 1) - 4.0) / 3.0) / 6.0;
    let rd = (r * r - 4.0 * (lv - 1.0 / 6.0)).max(0.25).sqrt();
    rd / (1u32 << l) as f32
}

/// Normalised disc kernel of radius `rho` texels (anti-aliased edge).
fn disc_kernel(rho: f32) -> Vec<(i32, i32, f32)> {
    let m = (rho + 1.0).ceil() as i32;
    let mut k = Vec::new();
    let mut sum = 0.0;
    for dy in -m..=m {
        for dx in -m..=m {
            // Coverage of the texel by the disc, by 4×4 sampling.
            let mut c = 0.0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let (x, y) = (dx as f32 + (sx as f32 + 0.5) / 4.0 - 0.5, dy as f32 + (sy as f32 + 0.5) / 4.0 - 0.5);
                    if x * x + y * y <= rho * rho {
                        c += 1.0 / 16.0;
                    }
                }
            }
            if c > 0.0 {
                k.push((dx, dy, c));
                sum += c;
            }
        }
    }
    k.iter().map(|&(x, y, c)| (x, y, c / sum)).collect()
}

impl Pyr {
    #[inline(always)]
    fn rows(&self, l: usize) -> (i32, i32) {
        (self.band.0 >> l, if self.band.1 == i32::MAX { i32::MAX } else { self.band.1 >> l })
    }

    fn new(w: usize, h: usize, levels: usize, c: usize) -> Pyr {
        let mut lv = Vec::new();
        let (mut cw, mut ch) = (w, h);
        for _ in 0..levels {
            lv.push((cw, ch, vec![0.0f32; cw * ch * c]));
            cw = cw.div_ceil(2).max(1);
            ch = ch.div_ceil(2).max(1);
        }
        Pyr { c, lv, used: [false; 8], bins: vec![None; BINS.len()], band: (0, i32::MAX), off: [0; 8] }
    }

    /// A pyramid holding only the rows of `band` at each level.
    fn new_band(w: usize, h: usize, levels: usize, c: usize, band: (i32, i32)) -> Pyr {
        let mut p = Pyr { c, lv: Vec::new(), used: [false; 8], bins: vec![None; BINS.len()], band, off: [0; 8] };
        let (mut cw, mut ch) = (w, h);
        for l in 0..levels {
            let (r0, r1) = p.rows(l);
            let (r0, r1) = (r0.clamp(0, ch as i32), r1.clamp(0, ch as i32));
            p.off[l] = r0;
            p.lv.push((cw, ch, vec![0.0f32; (r1 - r0) as usize * cw * c]));
            cw = cw.div_ceil(2).max(1);
            ch = ch.div_ceil(2).max(1);
        }
        p
    }

    /// Copy a band pyramid's rows into this full one.
    fn absorb(&mut self, b: Pyr) {
        let c = self.c;
        for (l, (cw, _, buf)) in b.lv.into_iter().enumerate() {
            if b.used[l] {
                self.used[l] = true;
                let o = b.off[l] as usize * cw * c;
                self.lv[l].2[o..o + buf.len()].copy_from_slice(&buf);
            }
        }
        for (k, bin) in b.bins.into_iter().enumerate() {
            if let Some(buf) = bin {
                let l = BINS[k].1;
                let (cw, ch) = (self.lv[l].0, self.lv[l].1);
                let dst = self.bins[k].get_or_insert_with(|| vec![0.0; cw * ch * c]);
                let o = b.off[l] as usize * cw * c;
                dst[o..o + buf.len()].copy_from_slice(&buf);
            }
        }
    }
    /// Reference splat: a disc of radius `r` (px) at full resolution seen
    /// through the tent pixel filter (the fast path's reconstruction), by
    /// dense sampling of the disc.
    fn disc(&mut self, x: f32, y: f32, r: f32, v: &[f32; 8]) {
        let (r0, r1) = self.rows(0);
        let off = self.off[0];
        let c = self.c;
        self.used[0] = true;
        let (w, h, buf) = &mut self.lv[0];
        let (w, h) = (*w as i32, *h as i32);
        // Points over the disc (a fine grid clipped to it; the centre for a point).
        let step = (r / 12.0).clamp(0.02, 0.25);
        let m = (r / step).ceil() as i32;
        let mut cells: std::collections::HashMap<(i32, i32), f32> = std::collections::HashMap::new();
        let mut total = 0.0f32;
        for j in -m..=m {
            for i in -m..=m {
                let (dx, dy) = (i as f32 * step, j as f32 * step);
                if dx * dx + dy * dy > r * r && !(i == 0 && j == 0) {
                    continue;
                }
                let (qx, qy) = (x + dx - 0.5, y + dy - 0.5);
                let (ix, iy) = (qx.floor(), qy.floor());
                let (tx, ty) = (qx - ix, qy - iy);
                for (ox, oy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                    *cells.entry((ix as i32 + ox, iy as i32 + oy)).or_insert(0.0) += wt;
                }
                total += 1.0;
            }
        }
        for ((px, py), n) in cells {
            if px < 0 || py < r0.max(0) || px >= w || py >= h.min(r1) {
                continue;
            }
            let wt = n / total;
            let o = &mut buf[((py - off) * w + px) as usize * c..][..c];
            for k in 0..c {
                o[k] += v[k] * wt;
            }
        }
    }

    /// An exact small disc (radius < 4 px) from the stamp table, full
    /// resolution, bilinear between the four nearest of 16×16 sub-pixel
    /// phases, over the union of their tight bounds.
    #[inline(always)]
    fn stamp(&mut self, x: f32, y: f32, r: f32, v: &[f32; 8]) {
        match self.c {
            3 => self.stamp_c::<3>(x, y, r, v),
            4 => self.stamp_c::<4>(x, y, r, v),
            _ => self.stamp_c::<7>(x, y, r, v),
        }
    }

    #[inline(always)]
    fn stamp_c<const C: usize>(&mut self, x: f32, y: f32, r: f32, v: &[f32; 8]) {
        let (r0, r1) = self.rows(0);
        let off = self.off[0];
        let c = C;
        self.used[0] = true;
        let (tab, boxes) = stamps();
        let (w, h, buf) = &mut self.lv[0];
        let (w, h) = (*w as i32, *h as i32);
        let (fx, fy) = (x.floor(), y.floor());
        let ph = |f: f32| {
            let u = f * ST_P as f32 - 0.5;
            let p0 = (u.floor() as i32).clamp(0, ST_P as i32 - 2) as usize;
            (p0, u - p0 as f32)
        };
        let ((px, tx), (py, ty)) = (ph(x - fx), ph(y - fy));
        let ri = ((r * 16.0).round() as usize).min(ST_R - 1);
        let kk = |py: usize, px: usize| (ri * ST_P + py) * ST_P + px;
        let ks = [kk(py, px), kk(py, px + 1), kk(py + 1, px), kk(py + 1, px + 1)];
        let ws = [(1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty];
        let mut bx = [ST_N as u8, 0, ST_N as u8, 0];
        for &k in &ks {
            let b = boxes[k];
            bx = [bx[0].min(b[0]), bx[1].max(b[1]), bx[2].min(b[2]), bx[3].max(b[3])];
        }
        let st = |k: usize| &tab[k * ST_N * ST_N..(k + 1) * ST_N * ST_N];
        let (s0, s1, s2, s3) = (st(ks[0]), st(ks[1]), st(ks[2]), st(ks[3]));
        let (ox, oy) = (fx as i32 - ST_C as i32, fy as i32 - ST_C as i32);
        let (xa, xb) = ((bx[0] as i32).max(-ox), (bx[1] as i32).min(w - 1 - ox));
        if xa > xb {
            return;
        }
        for sy in bx[2] as i32..=bx[3] as i32 {
            let yy = oy + sy;
            if yy < r0.max(0) || yy >= h.min(r1) {
                continue;
            }
            let base = sy as usize * ST_N;
            let row = &mut buf[((yy - off) * w + ox + xa) as usize * c..((yy - off) * w + ox + xb + 1) as usize * c];
            let vv: [f32; C] = std::array::from_fn(|j| v[j]);
            for (k, o) in row.chunks_exact_mut(C).enumerate() {
                let i = base + xa as usize + k;
                let wt = s0[i] * ws[0] + s1[i] * ws[1] + s2[i] * ws[2] + s3[i] * ws[3];
                for j in 0..C {
                    o[j] += vv[j] * wt;
                }
            }
        }
    }

    /// Bilinear point splat into defocus bin `b`.
    #[inline(always)]
    fn splat_bin(&mut self, b: usize, x: f32, y: f32, v: &[f32; 8]) {
        let (r0, r1) = self.rows(BINS[b].1);
        let c = self.c;
        let l = BINS[b].1;
        let (w, h) = (self.lv[l].0, self.lv[l].1);
        let (b0, b1) = (r0.clamp(0, h as i32), r1.clamp(0, h as i32));
        let off = b0;
        let buf = self.bins[b].get_or_insert_with(|| vec![0.0; (b1 - b0) as usize * w * c]);
        let f = 1.0 / (1u32 << l) as f32;
        let (fx, fy) = (x * f - 0.5, y * f - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i32, iy as i32);
        let (w, h) = (w as i32, h as i32);
        for (dx, dy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            let (xx, yy) = (ix + dx, iy + dy);
            if xx < 0 || yy < r0.max(0) || xx >= w || yy >= h.min(r1) {
                continue;
            }
            let o = &mut buf[((yy - off) * w + xx) as usize * c..][..c];
            for k in 0..c {
                o[k] += v[k] * wt;
            }
        }
    }

    /// Convolve every used bin with its disc and add it into its level.
    fn resolve_bins(&mut self, par: bool) {
        use rayon::prelude::*;
        let c = self.c;
        for b in 0..BINS.len() {
            let Some(src) = self.bins[b].take() else { continue };
            let l = BINS[b].1;
            let ker = disc_kernel(bin_rho(b));
            let (w, h, dst) = &mut self.lv[l];
            let (w, h) = (*w as i32, *h as i32);
            self.used[l] = true;
            let row = |(y, out): (usize, &mut [f32])| {
                for x in 0..w {
                    let o = &mut out[x as usize * c..(x as usize + 1) * c];
                    for &(dx, dy, k) in &ker {
                        let (sx, sy) = (x + dx, y as i32 + dy);
                        if sx < 0 || sy < 0 || sx >= w || sy >= h {
                            continue;
                        }
                        let s = &src[(sy * w + sx) as usize * c..][..c];
                        for j in 0..c {
                            o[j] += s[j] * k;
                        }
                    }
                }
            };
            if par {
                dst.par_chunks_mut(w as usize * c).enumerate().for_each(row);
            } else {
                dst.chunks_mut(w as usize * c).enumerate().for_each(row);
            }
        }
    }

    /// Bilinear splat at level `l`, pixel coordinates of level 0.
    #[inline(always)]
    #[allow(dead_code)]
    fn splat(&mut self, l: usize, x: f32, y: f32, v: &[f32; 8]) {
        match self.c {
            3 => self.splat_c::<3>(l, x, y, v),
            4 => self.splat_c::<4>(l, x, y, v),
            _ => self.splat_c::<7>(l, x, y, v),
        }
    }

    #[inline(always)]
    fn splat_c<const C: usize>(&mut self, l: usize, x: f32, y: f32, v: &[f32; 8]) {
        let (r0, r1) = self.rows(l);
        let off = self.off[l];
        let c = C;
        self.used[l] = true;
        let (w, h, buf) = &mut self.lv[l];
        let f = 1.0 / (1u32 << l) as f32;
        let (fx, fy) = (x * f - 0.5, y * f - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i32, iy as i32);
        let (w, h) = (*w as i32, *h as i32);
        for (dx, dy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            let (xx, yy) = (ix + dx, iy + dy);
            if xx < 0 || yy < r0.max(0) || xx >= w || yy >= h.min(r1) {
                continue;
            }
            let o = &mut buf[((yy - off) * w + xx) as usize * c..][..C];
            for k in 0..C {
                o[k] += v[k] * wt;
            }
        }
    }
}

/// An opacity shadow map from the key light: optical depth of absorbing
/// particles, accumulated in slices along the light direction. Particles
/// read it for self-shadowing (dense clouds darken inside, like volumes) and
/// the scene reads it for particle shadows on the floor and objects.
pub struct ShadowMap {
    a: V3,
    b: V3,
    l: V3,
    lo: V3,
    k: V3,
    n: usize,
    s: usize,
    /// Optical depth from the light down to the *start* of each slice, per texel.
    cum: Vec<f32>,
}

const SM_N: usize = 384;
const SM_S: usize = 64;

impl ShadowMap {
    /// Build from the absorbing systems; `l` points towards the light.
    pub fn build(systems: &[(&Sim, Arc<Snap>)], l: V3, t: f32) -> Option<ShadowMap> {
        use rayon::prelude::*;
        let casters: Vec<&(&Sim, Arc<Snap>)> = systems.iter().filter(|(s, _)| s.def.blend == Blend::Over && s.def.shadows).collect();
        if casters.is_empty() {
            return None;
        }
        let l = norm(l);
        let (a, b) = basis(l);
        let ext = |p: &[f32; 3]| {
            let q = v3(p[0], p[1], p[2]);
            v3(dot(q, a), dot(q, b), dot(q, l))
        };
        let inf = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        let (mut lo, mut hi) = inf;
        for (_, snap) in &casters {
            let (x, y) = snap.pos.par_iter().with_min_len(1 << 14).map(|p| { let e = ext(p); (e, e) }).reduce(|| inf, |(a0, a1), (b0, b1)| (vmin(a0, b0), vmax(a1, b1)));
            lo = vmin(lo, x);
            hi = vmax(hi, y);
        }
        if lo.x > hi.x {
            return None;
        }
        // Square texels in the light plane.
        let span = (hi.x - lo.x).max(hi.y - lo.y).max(1e-3) * 1.04;
        let c = scale(add(lo, hi), 0.5);
        let lo = v3(c.x - span * 0.5, c.y - span * 0.5, lo.z - 1e-3);
        let depth = (hi.z - lo.z).max(1e-3) * 1.02;
        let (n, sl) = (SM_N, SM_S);
        let k = v3(n as f32 / span, n as f32 / span, sl as f32 / depth);
        let texel_area = (span / n as f32).powi(2);
        let mut grid = vec![0.0f32; n * n * sl];
        for (sim, snap) in &casters {
            let d = &sim.def;
            for (j, p) in snap.pos.iter().enumerate() {
                let i = snap.id[j] as usize;
                let (_, op) = sim.look_alpha(i, t);
                if op <= 0.0 {
                    continue;
                }
                let size = match &d.size_e {
                    Some(e) => e.at(v3(p[0], p[1], p[2]), v3(0.0, 0.0, 0.0), t, sim.age(i, t), i).max(0.0),
                    None => d.size * (1.0 + d.size_var * (hu(d.seed, i as u32, 7) - 0.5) * 2.0),
                };
                let od = op * std::f32::consts::PI * size * size / texel_area * sim.stride as f32;
                let e = ext(p);
                let (fx, fy) = ((e.x - lo.x) * k.x - 0.5, (e.y - lo.y) * k.y - 0.5);
                // Tent in depth too, so a surface crossing slices doesn't band.
                let fz = ((e.z - lo.z) * k.z - 0.5).clamp(0.0, sl as f32 - 1.0);
                let (z0, tz) = (fz as usize, fz.fract());
                let z1 = (z0 + 1).min(sl - 1);
                let (ix, iy) = (fx.floor(), fy.floor());
                let (tx, ty) = (fx - ix, fy - iy);
                let (ix, iy) = (ix as i32, iy as i32);
                for (dx, dy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                    let (x, y) = (ix + dx, iy + dy);
                    if x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n {
                        let c = (y as usize * n + x as usize) * sl;
                        grid[c + z0] += od * wt * (1.0 - tz);
                        grid[c + z1] += od * wt * tz;
                    }
                }
            }
        }
        // Accumulate from the light side (highest slice) down, then soften
        // across texels (a particle is far smaller than a texel).
        grid.par_chunks_mut(sl).for_each(|col| {
            let mut acc = 0.0;
            for z in (0..sl).rev() {
                let v = col[z];
                col[z] = acc + v * 0.5;
                acc += v;
            }
        });
        let mut cum = grid.clone();
        cum.par_chunks_mut(n * sl).enumerate().for_each(|(y, row)| {
            for x in 0..n {
                for z in 0..sl {
                    let mut acc = 0.0;
                    let mut ws = 0.0;
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                            if xx >= 0 && yy >= 0 && (xx as usize) < n && (yy as usize) < n {
                                let w = if dx == 0 && dy == 0 { 4.0 } else if dx == 0 || dy == 0 { 2.0 } else { 1.0 };
                                acc += grid[(yy as usize * n + xx as usize) * sl + z] * w;
                                ws += w;
                            }
                        }
                    }
                    row[x * sl + z] = acc / ws;
                }
            }
        });
        Some(ShadowMap { a, b, l, lo, k, n, s: sl, cum })
    }

    /// Transmittance of the key light reaching `p`.
    #[inline]
    pub fn trans(&self, p: V3) -> f32 {
        // Bias towards the light by two slices, so a surface isn't shadowed by its own layer.
        let p = madd(p, self.l, 2.0 / self.k.z);
        let (u, v, w) = ((dot(p, self.a) - self.lo.x) * self.k.x - 0.5, (dot(p, self.b) - self.lo.y) * self.k.y - 0.5, (dot(p, self.l) - self.lo.z) * self.k.z - 0.5);
        let n = self.n as f32;
        if u < -1.0 || v < -1.0 || u > n || v > n {
            return 1.0;
        }
        let sl = self.s;
        let z = w.clamp(0.0, sl as f32 - 1.0);
        let (iz, tz) = (z as usize, z.fract());
        let iz1 = (iz + 1).min(sl - 1);
        let (ix, iy) = (u.floor(), v.floor());
        let (tx, ty) = (u - ix, v - iy);
        let (ix, iy) = (ix as i32, iy as i32);
        let mut od = 0.0;
        for (dx, dy, wt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            let (x, y) = (ix + dx, iy + dy);
            if x >= 0 && y >= 0 && (x as usize) < self.n && (y as usize) < self.n {
                let c = &self.cum[(y as usize * self.n + x as usize) * sl..];
                // Below the bottom slice: everything above has been passed.
                let val = if w > sl as f32 - 0.5 { 0.0 } else if w < -0.5 { c[0] + 0.0 } else { c[iz] + (c[iz1] - c[iz]) * tz };
                od += val * wt;
            }
        }
        (-od).exp()
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
    pub shadow: Option<&'a ShadowMap>,
    /// Draft: in-focus particles as plain tent splats (about 2× faster, softer).
    pub draft: bool,
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

static REFERENCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Reference mode: particles splat as exact box-filtered discs (size and
/// defocus) at 32 instants over the shutter, instead of the pyramid and
/// adaptive streak samples. Slow; for measuring the fast path's error.
pub fn set_reference(on: bool) {
    REFERENCE.store(on, std::sync::atomic::Ordering::Relaxed);
}

#[inline]
fn reference() -> bool {
    REFERENCE.load(std::sync::atomic::Ordering::Relaxed) || std::env::var_os("ANIKIT_SCENE3D_REFERENCE").is_some()
}

/// acos to about 1e-4 (Abramowitz & Stegun 4.4.45).
#[inline(always)]
fn fast_acos(x: f32) -> f32 {
    let a = x.abs();
    let r = (1.0 - a).max(0.0).sqrt() * (1.570_728_8 + a * (-0.212_114_4 + a * (0.074_261_0 + a * -0.018_729_3)));
    if x < 0.0 {
        std::f32::consts::PI - r
    } else {
        r
    }
}

/// Splat every live system into one layer.
pub fn render(systems: &[(&Sim, Arc<Snap>)], v: &View, t: f32) -> Layer {
    let clock = std::time::Instant::now();
    let refm = reference();
    let (w, h) = (v.w, v.h);
    let levels = 6;
    let s = h as f32 / 1080.0;
    let pj = Proj::new(v.cam, w, h);
    let pp = Proj::new(v.prev, w, h);
    // Blur variance (px², per axis) of a splat at level l after upsampling to
    // full resolution: a tent of half-width 2^l, plus one tent per upsample.
    let lvar: Vec<f32> = (0..=levels).map(|l| (4f32.powi(l as i32) + (4f32.powi(l as i32 + 1) - 4.0) / 3.0) / 6.0).collect();
    // A disc of radius r seen through the tent pixel filter has variance r²/4 + 1/6:
    // split the particle between the two levels whose variances bracket it.
    // Kernels in order of blur: a point at full resolution, then the defocus
    // bins (disc plus the splat/upsample blur of their level).
    // Bins are sized so disc + level blur has the variance of a disc of
    // their nominal radius through the tent filter (see `bin_rho`).
    let kvar: Vec<f32> = std::iter::once(16.0 * 0.25 + 1.0 / 6.0).chain(BINS.iter().map(|&(r, _)| r * r * 0.25 + 1.0 / 6.0)).collect();
    let kern_of = |r: f32| -> (usize, f32) {
        let v = r * r * 0.25 + 1.0 / 6.0;
        let mut k = 0;
        while k + 1 < kvar.len() && kvar[k + 1] <= v {
            k += 1;
        }
        if k + 1 >= kvar.len() {
            return (kvar.len() - 1, 0.0);
        }
        (k, ((v - kvar[k]) / (kvar[k + 1] - kvar[k])).clamp(0.0, 1.0))
    };
    let _ = &lvar;
    // Channel layout: emission only for glowing systems, absorption only for `over` ones.
    let emits = systems.iter().any(|(s, _)| s.def.blend == Blend::Add || s.def.emissive > 0.0);
    let absorbs = systems.iter().any(|(s, _)| s.def.blend == Blend::Over);
    let (eo, ab, c) = match (emits, absorbs) {
        (true, true) => (Some(0), Some(3), 7),
        (false, true) => (None, Some(0), 4),
        _ => (Some(0), None, 3),
    };
    // Bands are fixed (64 rows), so the output never depends on the thread count;
    // nested inside a render worker this still spreads over idle cores.
    let threads = rayon::current_num_threads().max(1);
    let mut pyrs: Vec<Pyr> = Vec::new();
    for (sim, snap) in systems {
        let d = &sim.def;
        let n = snap.pos.len();
        let spread2 = if sim.stride > 1 { sim.stride as f32 / std::f32::consts::PI } else { 0.0 };
        // Bands of rows (multiples of 64, so every level splits exactly).
        // Threaded: 64-row bands. Pass 1 finds the bands each particle can
        // touch; pass 2 renders each band from just those particles, in order.
        const BAND: usize = 64;
        let nb = h.div_ceil(BAND);
        let ranges: Option<Vec<(u8, u8)>> = (threads > 1).then(|| {
            use rayon::prelude::*;
            (0..n)
                .into_par_iter()
                .with_min_len(1 << 14)
                .map(|j| {
                    let i = snap.id[j] as usize;
                    let p = v3(snap.pos[j][0], snap.pos[j][1], snap.pos[j][2]);
                    let (sx, sy, z) = pj.at(p);
                    if z < 0.05 {
                        return (255, 0);
                    }
                    let size = match &d.size_e {
                        Some(e) => e.at(p, snap.vel(j), t, sim.age(i, t), i).max(0.0),
                        None => d.size * (1.0 + d.size_var * (hu(d.seed, i as u32, 7) - 0.5) * 2.0),
                    };
                    let r_px = size * pj.k / z;
                    let coc = if v.aperture > 0.05 { (v.aperture * s * (1.0 - v.focus / z.min(v.focus * 8.0)).abs()).min(28.0 * s) } else { 0.0 };
                    // A preview particle stands for `stride` of them: spread it over about that many pixels.
                let foot = (r_px * r_px + coc * coc + spread2).sqrt();
                    let margin = foot + 4.0;
                    if sx < -margin || sy < -margin || sx > w as f32 + margin || sy > h as f32 + margin {
                        return (255, 0);
                    }
                    let vel = snap.vel(j);
                    let back = madd(p, vel, -v.shutter * sim.dt * d.stretch);
                    let by = if v.shutter > 0.0 { pp.at(back).1 } else { sy };
                    let reach = foot + 7.0 + (sy - by).abs() * 0.5;
                    let lo = ((sy - reach).max(0.0) as usize / BAND).min(nb - 1);
                    let hi = (((sy + reach).max(0.0) as usize) / BAND).min(nb - 1);
                    (lo as u8, hi as u8)
                })
                .collect()
        });
        let work = |band: (i32, i32), bi: Option<u8>| -> Pyr {
            let mut py = if band == (0, i32::MAX) { Pyr::new(w, h, levels, c) } else { Pyr::new_band(w, h, levels, c, band) };
            for j in 0..n {
                if let (Some(b), Some(rg)) = (bi, &ranges) {
                    let (lo, hi) = rg[j];
                    if b < lo || b > hi {
                        continue;
                    }
                }
                let i = snap.id[j] as usize;
                let p = v3(snap.pos[j][0], snap.pos[j][1], snap.pos[j][2]);
                let (sx, sy, z) = pj.at(p);
                if z < 0.05 {
                    continue;
                }
                let size = match &d.size_e {
                    Some(e) => e.at(p, snap.vel(j), t, sim.age(i, t), i).max(0.0),
                    None => d.size * (1.0 + d.size_var * (hu(d.seed, i as u32, 7) - 0.5) * 2.0),
                };
                let r_px = size * pj.k / z;
                let coc = if v.aperture > 0.05 { (v.aperture * s * (1.0 - v.focus / z.min(v.focus * 8.0)).abs()).min(28.0 * s) } else { 0.0 };
                // A preview particle stands for `stride` of them: spread it over about that many pixels.
                let foot = (r_px * r_px + coc * coc + spread2).sqrt();
                let margin = foot + 4.0;
                if sx < -margin || sy < -margin || sx > w as f32 + margin || sy > h as f32 + margin {
                    continue;
                }
                // Depth test against the scene at the particle's centre.
                let (ix, iy) = (sx as i32, sy as i32);
                if ix >= 0 && iy >= 0 && (ix as usize) < w && (iy as usize) < h && v.depth[iy as usize * w + ix as usize] < z - size * 2.0 {
                    continue;
                }
                // Threads own bands of rows (pass 1 already picked the particles that can reach this one).
                let banded = py.band != (0, i32::MAX);
                let vel = snap.vel(j);
                // Screen motion over the shutter: where the point was half a shutter ago.
                let back = madd(p, vel, -v.shutter * sim.dt * d.stretch);
                let (bx, by, _) = pp.at(back);
                let (bx, by) = (sx + (bx - sx) * if v.shutter > 0.0 { 1.0 } else { 0.0 }, sy + (by - sy) * if v.shutter > 0.0 { 1.0 } else { 0.0 });
                let (mx, my) = (sx - bx, sy - by);
                let ml = (mx * mx + my * my).sqrt();
                let _ = banded;
                let (col, a) = sim.look(i, t, p, vel);
                if a <= 0.0 {
                    continue;
                }
                let (kk, lw) = if foot < 4.0 { (usize::MAX, 0.0) } else { kern_of(foot) };
                let l = if kk == 0 || kk == usize::MAX { 0 } else { BINS[kk - 1].1 };
                let tex = (1u32 << l) as f32;
                // Streak samples at most half a texel apart (a texel in draft).
                let ns = if v.draft { ((ml / tex).ceil() as usize).clamp(1, 24) } else { ((ml * 2.0 / tex).ceil() as usize).clamp(1, 48) };
                // Energy: projected disc area (world size) — in focus it's smaller than a pixel.
                let area = std::f32::consts::PI * r_px * r_px;
                // Lit colour.
                let lc = if d.lit {
                    let to_cam = norm(sub(v.cam.pos, p));
                    let ca = dot(to_cam, v.key.0).clamp(-1.0, 1.0);
                    // Lambert sphere phase: (sin α + (π − α) cos α) / π, with π − α = acos(−cos α).
                    let phase = ((1.0 - ca * ca).max(0.0).sqrt() + fast_acos(-ca) * ca) / std::f32::consts::PI;
                    let amb = lerp3(v.amb.1, v.amb.0, 0.5 + 0.5 * to_cam.y.max(0.0));
                    let vis = v.shadow.map(|m| m.trans(p)).unwrap_or(1.0);
                    let keyc = scale(v.key.1, vis);
                    let mut l = add(scale(keyc, phase * 0.66), amb);
                    // Particles that have arrived on a target surface shade like it (two-sided).
                    if let (Some(tg), Some((_, delay, nrm))) = (&d.target, &sim.tgt) {
                        let wv = ((((t - tg.from) / (tg.to - tg.from)) - delay[i]) * 2.5).clamp(0.0, 1.0);
                        if wv > 0.0 {
                            let mut n = v3(nrm[i][0], nrm[i][1], nrm[i][2]);
                            if dot(n, to_cam) < 0.0 {
                                n = scale(n, -1.0);
                            }
                            let ls = add(scale(keyc, dot(n, v.key.0).max(0.0)), lerp3(v.amb.1, v.amb.0, 0.5 + 0.5 * n.y));
                            l = lerp3(l, ls, wv);
                        }
                    }
                    [col[0] * l.x, col[1] * l.y, col[2] * l.z]
                } else {
                    col
                };
                let mut val = [0.0f32; 8];
                match d.blend {
                    Blend::Add => {
                        let e = d.emissive * a * area.min(4.0);
                        let o = eo.unwrap_or(0);
                        val[o] = lc[0] * e;
                        val[o + 1] = lc[1] * e;
                        val[o + 2] = lc[2] * e;
                    }
                    Blend::Over => {
                        // Optical depth: covered fraction of a pixel.
                        let od = a * area.min(4.0);
                        let o = ab.unwrap_or(0);
                        val[o] = lc[0] * od;
                        val[o + 1] = lc[1] * od;
                        val[o + 2] = lc[2] * od;
                        val[o + 3] = od;
                        if let Some(o) = eo {
                            let e = d.emissive * od;
                            val[o] = col[0] * e;
                            val[o + 1] = col[1] * e;
                            val[o + 2] = col[2] * e;
                        }
                    }
                }
                if sim.stride > 1 {
                    // Each simulated particle stands for `stride` of them.
                    let k = sim.stride as f32;
                    for x in &mut val {
                        *x *= k;
                    }
                }
                if refm {
                    // Exact: the full footprint at 32–128 instants (at most 0.5 px apart).
                    let nt = if ml > 0.01 { ((ml * 2.0).ceil() as usize).clamp(32, 128) } else { 1 };
                    let vv = val.map(|x| x / nt as f32);
                    for k in 0..nt {
                        let u = if nt == 1 { 0.0 } else { (k as f32 + 0.5) / nt as f32 - 0.5 };
                        py.disc(sx + mx * u, sy + my * u, foot, &vv);
                    }
                    continue;
                }
                let inv = 1.0 / ns as f32;
                let (v0, v1) = (val.map(|x| x * inv * (1.0 - lw)), val.map(|x| x * inv * lw));
                for k in 0..ns {
                    let u = if ns == 1 { 0.0 } else { (k as f32 + 0.5) * inv - 0.5 };
                    let (x, y) = (sx + mx * u, sy + my * u);
                    if kk == usize::MAX {
                        if foot < 0.25 || v.draft {
                            // Disc ⊗ tent ≈ the tent itself.
                            py.splat(0, x, y, &v0);
                        } else {
                            py.stamp(x, y, foot, &v0);
                        }
                        continue;
                    }
                    if lw < 0.999 {
                        if kk == 0 {
                            py.stamp(x, y, 3.99, &v0);
                        } else {
                            py.splat_bin(kk - 1, x, y, &v0);
                        }
                    }
                    if lw > 0.001 {
                        py.splat_bin(kk, x, y, &v1);
                    }
                }
            }
            py
        };
        if threads == 1 {
            pyrs.push(work((0, i32::MAX), None));
        } else {
            use rayon::prelude::*;
            let parts: Vec<Pyr> = (0..nb).into_par_iter().map(|b| work(((b * BAND) as i32, if b + 1 == nb { i32::MAX } else { ((b + 1) * BAND) as i32 }), Some(b as u8))).collect();
            let mut full = Pyr::new(w, h, levels, c);
            for p in parts {
                full.absorb(p);
            }
            pyrs.push(full);
        }
    }
    let t_splat = clock.elapsed();
    // Reduce the per-thread pyramids, then collapse levels (coarse to fine, bilinear up).
    use rayon::prelude::*;
    let par = threads > 1;
    let mut acc = match pyrs.pop() {
        Some(p) => p,
        None => Pyr::new(w, h, levels, c),
    };
    for p in pyrs {
        for l in 0..levels {
            acc.used[l] |= p.used[l];
        }
        for (a, b) in acc.bins.iter_mut().zip(p.bins) {
            match (a.as_mut(), b) {
                (Some(x), Some(y)) => x.iter_mut().zip(&y).for_each(|(x, y)| *x += y),
                (None, Some(y)) => *a = Some(y),
                _ => {}
            }
        }
        for (a, b) in acc.lv.iter_mut().zip(p.lv) {
            if par {
                a.2.par_chunks_mut(1 << 16).zip(b.2.par_chunks(1 << 16)).for_each(|(x, y)| x.iter_mut().zip(y).for_each(|(x, y)| *x += y));
            } else {
                a.2.iter_mut().zip(&b.2).for_each(|(x, y)| *x += y);
            }
        }
    }
    let t_reduce = clock.elapsed();
    acc.resolve_bins(par);
    let t_bins = clock.elapsed();
    for l in (1..levels).rev() {
        if !acc.used[l] {
            continue;
        }
        acc.used[l - 1] = true;
        let (lo, hi) = acc.lv.split_at_mut(l);
        let (cw, ch, src) = (hi[0].0, hi[0].1, &hi[0].2);
        let (fw, _, dst) = &mut lo[l - 1];
        let fw = *fw;
        let row = |(y, out): (usize, &mut [f32])| {
            let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, ch as f32 - 1.0);
            let (iy, ty) = (fy as usize, fy - fy.floor());
            let iy1 = (iy + 1).min(ch - 1);
            let (r0, r1) = (&src[iy * cw * c..(iy + 1) * cw * c], &src[iy1 * cw * c..(iy1 + 1) * cw * c]);
            for x in 0..fw {
                let fx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, cw as f32 - 1.0);
                let (ix, tx) = (fx as usize, fx - fx.floor());
                let ix1 = (ix + 1).min(cw - 1);
                // Values are per texel; a coarse texel spreads over 4 fine ones.
                let (w00, w10, w01, w11) = ((1.0 - tx) * (1.0 - ty) * 0.25, tx * (1.0 - ty) * 0.25, (1.0 - tx) * ty * 0.25, tx * ty * 0.25);
                let o = &mut out[x * c..(x + 1) * c];
                for k in 0..c {
                    o[k] += r0[ix * c + k] * w00 + r0[ix1 * c + k] * w10 + r1[ix * c + k] * w01 + r1[ix1 * c + k] * w11;
                }
            }
        };
        if par {
            dst.par_chunks_mut(fw * c).enumerate().for_each(row);
        } else {
            dst.chunks_mut(fw * c).enumerate().for_each(row);
        }
    }
    let prof = std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some();
    if prof {
        eprintln!("  splat: points {:.0} ms, reduce {:.0} ms, bins {:.0} ms, collapse {:.0} ms", t_splat.as_secs_f64() * 1e3, (t_reduce - t_splat).as_secs_f64() * 1e3, (t_bins - t_reduce).as_secs_f64() * 1e3, (clock.elapsed() - t_bins).as_secs_f64() * 1e3);
    }
    let buf = std::mem::take(&mut acc.lv[0].2);
    Layer { w, h, c, eo, ab, buf }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim(v: Value) -> Sim {
        std::env::set_var("ANIKIT_CACHE", "off");
        let mut shapes = spec::Shapes::new();
        let d = parse(&v, &mut shapes, &mut Vec::new(), &HashMap::new(), &[], "p").unwrap();
        Sim::new(&d, 30.0, Some(0.0), None).unwrap()
    }

    #[test]
    fn same_frames_in_any_order() {
        let v = serde_json::json!({ "count": 5000, "flow": 1.0, "life": 1.0, "gravity": -1.0 });
        let a = sim(v.clone());
        let b = sim(v);
        let x = a.at(1.0);
        let _ = b.at(1.5);
        let y = b.at(1.0);
        assert_eq!(x.pos, y.pos);
    }

    #[test]
    fn flow_is_divergence_free() {
        let g = FlowGrid::new(3);
        let (mut div, mut mag) = (0.0f32, 0.0f32);
        for k in 0..200 {
            let p = [u01(hash2(k, 1)) * 64.0, u01(hash2(k, 2)) * 64.0, u01(hash2(k, 3)) * 64.0];
            let e = 0.5;
            let dx = g.sample(p[0] + e, p[1], p[2])[0] - g.sample(p[0] - e, p[1], p[2])[0];
            let dy = g.sample(p[0], p[1] + e, p[2])[1] - g.sample(p[0], p[1] - e, p[2])[1];
            let dz = g.sample(p[0], p[1], p[2] + e)[2] - g.sample(p[0], p[1], p[2] - e)[2];
            div += (dx + dy + dz).abs();
            mag += dx.abs() + dy.abs() + dz.abs();
        }
        assert!(div < mag * 0.15, "divergence {div} vs gradient {mag}");
    }

    #[test]
    fn particles_stay_out_of_colliders() {
        let mut s = sim(serde_json::json!({ "count": 4000, "emit": { "shape": "box", "pos": [0, 3, 0], "size": [0.6, 0.2, 0.6] }, "gravity": -9.8, "size": 0.005 }));
        let mut shapes = spec::Shapes::new();
        let ball = spec::object(&serde_json::json!({ "type": "sphere", "radius": 0.5, "pos": [0, 1, 0], "collide": true }), &mut shapes, &mut Vec::new(), &HashMap::new(), "b").unwrap();
        let cube = spec::object(&serde_json::json!({ "type": "box", "size": 0.6, "pos": [0, 0.3, 1.5], "rot": [0, 30, 0], "collide": true }), &mut shapes, &mut Vec::new(), &HashMap::new(), "c").unwrap();
        s.set_colliders(vec![ball, cube]);
        for f in [10, 20, 30, 45, 60] {
            let q = s.at(f as f32 / 30.0);
            for p in &q.pos {
                let d = len(super::super::math::sub(v3(p[0], p[1], p[2]), v3(0.0, 1.0, 0.0)));
                assert!(d > 0.49, "particle inside the sphere at frame {f}: {d}");
            }
        }
    }

    #[test]
    fn splats_are_identical_at_any_thread_count() {
        let s = sim(serde_json::json!({ "count": 30000, "emit": { "shape": "ball", "radius": 0.4, "pos": [0, 1, 0] }, "size": 0.004, "velocity": [1.5, 0, 0], "blend": "over" }));
        let snap = s.at(0.1);
        let (w, h) = (320usize, 180usize);
        let depth = vec![1e9f32; w * h];
        let cam = Camera { pos: v3(0.0, 1.0, 3.0), target: v3(0.0, 1.0, 0.0), fov: 30.0, focus: 2.0, aperture: 8.0, roll: 0.0, shutter: 0.5 };
        // The camera moves too (its motion streaks particles across bands).
        let prev = Camera { pos: v3(0.6, 0.6, 3.2), ..cam };
        let v = View { cam: &cam, prev: &prev, w, h, depth: &depth, focus: 2.0, aperture: 8.0, shutter: 0.5, key: (v3(0.0, 1.0, 0.0), v3(1.0, 1.0, 1.0)), amb: (v3(0.2, 0.2, 0.2), v3(0.1, 0.1, 0.1)), shadow: None, draft: false };
        let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| render(&[(&s, snap.clone())], &v, 0.1).buf);
        // From a plain thread the splatter splits the frame over the global pool's threads.
        let many = std::thread::scope(|sc| sc.spawn(|| render(&[(&s, snap.clone())], &v, 0.1).buf).join().unwrap());
        assert!(rayon::current_num_threads() < 2 || one.iter().zip(&many).all(|(a, b)| a.to_bits() == b.to_bits()), "layers differ between 1 and {} threads", rayon::current_num_threads());
    }

    #[test]
    fn simulation_is_identical_at_any_thread_count() {
        let v = serde_json::json!({ "count": 70000, "emit": { "text": "AB", "height": 1.0 }, "flow": { "strength": 0.8, "scale": 0.8 }, "attractors": [{ "pos": [0, 1, 0], "strength": 0.5, "swirl": 1 }], "gravity": 0.15, "drag": 0.4, "life": 2 });
        let run = |threads: usize| rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
            let s = sim(v.clone());
            let q = s.at(1.5);
            (q.id.as_ref().clone(), q.pos.clone())
        });
        let (a, b) = (run(1), run(4));
        assert_eq!(a.0, b.0, "ids differ");
        assert!(a.1.iter().zip(&b.1).all(|(x, y)| x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits())), "positions differ between 1 and 4 threads");
    }

    #[test]
    fn draft_and_exact_splats_emit_the_same_light() {
        let s = sim(serde_json::json!({ "count": 20000, "emit": { "shape": "ball", "radius": 0.3, "pos": [0, 1, 0] }, "size": 0.002, "size_var": 0 }));
        let snap = s.at(0.0);
        let (w, h) = (640usize, 360usize);
        let depth = vec![1e9f32; w * h];
        let cam = Camera { pos: v3(0.0, 1.0, 4.0), target: v3(0.0, 1.0, 0.0), fov: 30.0, focus: 4.0, aperture: 0.0, roll: 0.0, shutter: 0.0 };
        let total = |draft: bool| {
            let v = View { cam: &cam, prev: &cam, w, h, depth: &depth, focus: 4.0, aperture: 0.0, shutter: 0.0, key: (v3(0.0, 1.0, 0.0), v3(1.0, 1.0, 1.0)), amb: (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)), shadow: None, draft };
            render(&[(&s, snap.clone())], &v, 0.0).emitted()
        };
        let (a, b) = (total(false), total(true));
        assert!((a - b).abs() / a < 0.01, "exact {a} vs draft {b}");
    }

    #[test]
    fn particles_bounce_off_the_floor() {
        let s = sim(serde_json::json!({ "count": 2000, "emit": { "shape": "box", "pos": [0, 1, 0], "size": 1 }, "gravity": -9.8, "velocity": [0, -5, 0] }));
        let q = s.at(2.0);
        assert!(q.pos.iter().all(|p| p[1] >= 0.0));
    }

    #[test]
    fn splats_conserve_light_in_and_out_of_focus() {
        let s = sim(serde_json::json!({ "count": 20000, "emit": { "shape": "ball", "radius": 0.3, "pos": [0, 1, 0] }, "size": 0.003, "size_var": 0 }));
        let snap = s.at(0.0);
        let (w, h) = (320usize, 180usize);
        let depth = vec![1e9f32; w * h];
        let total = |focus: f32, ap: f32| {
            let cam = Camera { pos: v3(0.0, 1.0, 4.0), target: v3(0.0, 1.0, 0.0), fov: 30.0, focus, aperture: ap, roll: 0.0, shutter: 0.0 };
            let v = View { cam: &cam, prev: &cam, w, h, depth: &depth, focus, aperture: ap, shutter: 0.0, key: (v3(0.0, 1.0, 0.0), v3(1.0, 1.0, 1.0)), amb: (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)), shadow: None, draft: false };
            render(&[(&s, snap.clone())], &v, 0.0).emitted()
        };
        let (sharp, blurred) = (total(4.0, 0.0), total(1.0, 30.0));
        assert!(sharp > 0.0 && (sharp - blurred).abs() / sharp < 0.03, "sharp {sharp} vs blurred {blurred}");
    }
}
