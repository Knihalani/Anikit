//! `scene3d` physics: thousands of rigid bodies (boxes, panes, spheres,
//! convex meshes) simulated once per spec with rapier and recorded, so any
//! frame can be rendered in any order on any thread.
//!
//! ```json
//! "physics": {
//!   "gravity": -9.8,
//!   "bodies": [ { "type": "pane", "count": 10000, "layout": "dominoes", "size": [0.9, 1.6, 0.03] } ],
//!   "impulses": [ { "t": 2.0, "at": [0, 0, -6], "strength": 6, "radius": 4 } ]
//! }
//! ```
//!
//! The simulation runs lazily: the first frame that needs time `t` steps the
//! world up to `t` (one thread steps, the others wait), recording every body's
//! pose at `record` Hz of video time. Frames between records interpolate.
//! Bodies at rest go to sleep and cost nothing; once every body sleeps, the
//! last record is shared instead of copied.
//!
//! Rapier is built with `enhanced-determinism`, so the same spec gives the
//! same motion on every machine, which keeps split and resumed renders seamless.

use super::math::*;
use super::spec::{self, Col, Num, ObjDef, Res};
use crate::eval::expr::Vars;
use rapier3d::dynamics::{RigidBodyBuilder, RigidBodyHandle, RigidBodyType};
use rapier3d::geometry::{ColliderBuilder, SharedShape};
use rapier3d::math::{Pose, Rotation, Vector};
use rapier3d::pipeline::PhysicsWorld;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A body pose: position and rotation quaternion (x, y, z, w).
pub type Pose7 = [f32; 7];

const PHYS_KEYS: &[&str] = &["gravity", "rate", "iterations", "time_scale", "start", "ground", "friction", "restitution", "record", "bodies", "impulses", "sleep", "freeze"];

/// Keys of a body group besides the usual object keys.
const BODY_KEYS: &[&str] = &[
    "instance", "layout", "spacing", "gap", "angle", "path", "heading", "origin", "grid", "region", "velocity", "angular", "density", "friction", "restitution", "trigger", "ccd", "seed", "turns", "jitter", "awake", "stagger", "pitch",
];

const LAYOUTS: &[&str] = &["dominoes", "grid", "wall", "pile", "rain", "expr"];

/// One rigid body's set-up.
#[derive(Clone)]
struct BodyInit {
    pose: Pose7,
    linvel: V3,
    angvel: V3,
    scale: f32,
    group: u32,
    awake: bool,
}

#[derive(Clone)]
struct GroupDef {
    col: Col,
    density: f32,
    friction: f32,
    restitution: f32,
    ccd: bool,
}

#[derive(Clone, Debug)]
struct Impulse {
    t: f32,
    at: V3,
    strength: f32,
    radius: f32,
    /// Extra upward share of the push (0 = purely radial).
    up: f32,
}

/// A fixed or kinematic collider that follows a scene object.
#[derive(Clone)]
struct Mover {
    obj: ObjDef,
    kinematic: bool,
}

/// The live rapier world, `at` records into the simulation.
struct World {
    world: PhysicsWorld,
    handles: Vec<RigidBodyHandle>,
    movers: Vec<Vec<RigidBodyHandle>>,
    at: usize,
    fired: usize,
    /// Seconds (simulated) each body has been still; `< 0` = frozen.
    still: Vec<f32>,
    frozen: usize,
}

struct State {
    /// Records `0..`: record `k` is video time `start + k / record`. Some may
    /// come from the disk cache, ahead of the live world.
    frames: Vec<Arc<Vec<Pose7>>>,
    world: Option<World>,
    steps: u64,
    sim_secs: f64,
    disk: Option<std::path::PathBuf>,
}

/// The simulation of one spec: set-up plus its lazily grown recording.
pub struct Sim {
    gravity: V3,
    rate: f32,
    iterations: usize,
    time_scale: Num,
    start: f32,
    ground: Option<f32>,
    record: f32,
    groups: Vec<GroupDef>,
    bodies: Vec<BodyInit>,
    impulses: Vec<Impulse>,
    movers: Vec<Mover>,
    sleep: bool,
    init: Arc<Vec<Pose7>>,
    /// Bounding radius per body (waking frozen bodies near movers).
    radius: Vec<f32>,
    /// Seconds a body must be still before it's frozen (0 = never).
    freeze: f32,
    /// Hash of everything that determines the motion (disk cache key).
    key: u64,
    state: Mutex<Option<State>>,
}

/// What `parse` hands back to the component.
pub struct Parsed {
    pub sim: Arc<Sim>,
    /// One render object per body group (transforms from the sim).
    pub objs: Vec<ObjDef>,
    /// Indices into the scene's objects used as `instance` templates (not drawn on their own).
    pub templates: Vec<usize>,
}

fn quat_of(r: &[V3; 3]) -> [f32; 4] {
    // Shepperd's method on the rotation matrix with columns r[0..3].
    let (m00, m11, m22) = (r[0].x, r[1].y, r[2].z);
    let tr = m00 + m11 + m22;
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [(r[1].z - r[2].y) / s, (r[2].x - r[0].z) / s, (r[0].y - r[1].x) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (r[1].x + r[0].y) / s, (r[2].x + r[0].z) / s, (r[1].z - r[2].y) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(r[1].x + r[0].y) / s, 0.25 * s, (r[2].y + r[1].z) / s, (r[2].x - r[0].z) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(r[2].x + r[0].z) / s, (r[2].y + r[1].z) / s, 0.25 * s, (r[0].y - r[1].x) / s]
    };
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt().max(1e-12);
    q.map(|v| v / n)
}

/// Rotation columns of a unit quaternion (x, y, z, w).
pub fn mat_of(q: [f32; 4]) -> [V3; 3] {
    let [x, y, z, w] = q;
    [
        v3(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)),
        v3(2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)),
        v3(2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)),
    ]
}

fn pose7(xf: &Xf) -> Pose7 {
    let q = quat_of(&xf.r);
    [xf.t.x, xf.t.y, xf.t.z, q[0], q[1], q[2], q[3]]
}

fn to_pose(p: &Pose7) -> Pose {
    Pose::from_parts(Vector::new(p[0], p[1], p[2]), Rotation::from_xyzw(p[3], p[4], p[5], p[6]).normalize())
}

/// Interpolate two poses (lerp + normalised quaternion lerp).
#[inline]
pub fn lerp_pose(a: &Pose7, b: &Pose7, u: f32) -> Pose7 {
    let d = a[3] * b[3] + a[4] * b[4] + a[5] * b[5] + a[6] * b[6];
    let sg = if d < 0.0 { -1.0 } else { 1.0 };
    let mut o = [0.0f32; 7];
    for k in 0..3 {
        o[k] = a[k] + (b[k] - a[k]) * u;
    }
    let mut n = 0.0;
    for k in 3..7 {
        o[k] = a[k] + (b[k] * sg - a[k]) * u;
        n += o[k] * o[k];
    }
    let n = n.sqrt().max(1e-12);
    for v in &mut o[3..7] {
        *v /= n;
    }
    o
}

#[inline]
pub fn xf_of(p: &Pose7, s: f32) -> Xf {
    Xf { r: mat_of([p[3], p[4], p[5], p[6]]), s, t: v3(p[0], p[1], p[2]) }
}

fn hash_u(seed: u32, i: u32, k: u32) -> f32 {
    u01(hash2(hash2(seed, i), k))
}

fn vec3c(v: &Value, path: &str) -> Res<V3> {
    match v {
        Value::Array(a) if a.len() == 3 => Ok(v3(spec::f(&a[0], path)?, spec::f(&a[1], path)?, spec::f(&a[2], path)?)),
        Value::Number(_) => {
            let x = spec::f(v, path)?;
            Ok(v3(x, x, x))
        }
        _ => Err(format!("{path}: expected [x, y, z]")),
    }
}

/// Parse `physics` (`objects` are the scene's raw objects, for `instance` templates).
pub fn parse(v: &Value, raw_objects: &[Value], shapes: &mut spec::Shapes, mats: &mut Vec<super::trace::Material>, named: &HashMap<String, super::trace::Material>, scene_objs: &[ObjDef], floor_y: Option<f32>) -> Res<Parsed> {
    let m = v.as_object().ok_or("physics: expected an object like { \"bodies\": [ … ] }")?;
    spec::check_keys(m, PHYS_KEYS, "physics")?;
    let gf = |k: &str, d: f32| -> Res<f32> { m.get(k).map(|x| spec::f(x, &format!("physics.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
    let gravity = match m.get("gravity") {
        None => v3(0.0, -9.81, 0.0),
        Some(Value::Number(_)) => v3(0.0, gf("gravity", -9.81)?, 0.0),
        Some(x) => vec3c(x, "physics.gravity")?,
    };
    let rate_v = m.get("rate").map(|x| spec::f(x, "physics.rate")).transpose()?;
    let iterations = gf("iterations", 4.0)?.clamp(1.0, 50.0) as usize;
    let time_scale = match m.get("time_scale") {
        None => Num::C(1.0),
        Some(x) => spec::num(x, "physics.timeScale")?,
    };
    let start = gf("start", 0.0)?;
    let record = gf("record", 60.0)?.clamp(10.0, 480.0);
    let ground = match m.get("ground") {
        None | Some(Value::Bool(true)) => Some(floor_y.unwrap_or(0.0)),
        Some(Value::Bool(false)) => None,
        Some(x) => Some(spec::f(x, "physics.ground")?),
    };
    let friction = gf("friction", 0.5)?;
    let restitution = gf("restitution", 0.05)?;
    let sleep = m.get("sleep").and_then(|x| x.as_bool()).unwrap_or(true);

    let mut impulses = Vec::new();
    if let Some(a) = m.get("impulses") {
        let a = a.as_array().ok_or("physics.impulses: expected an array")?;
        for (j, x) in a.iter().enumerate() {
            let p = format!("physics.impulses[{j}]");
            let o = x.as_object().ok_or_else(|| format!("{p}: expected {{ \"t\", \"at\", \"strength\", \"radius\" }}"))?;
            spec::check_keys(o, &["t", "at", "strength", "radius", "up"], &p)?;
            let g = |k: &str, d: f32| -> Res<f32> { o.get(k).map(|x| spec::f(x, &format!("{p}.{k}"))).transpose().map(|v| v.unwrap_or(d)) };
            impulses.push(Impulse {
                t: g("t", 0.0)?,
                at: o.get("at").map(|x| vec3c(x, &format!("{p}.at"))).transpose()?.unwrap_or(v3(0.0, 0.0, 0.0)),
                strength: g("strength", 5.0)?,
                radius: g("radius", 3.0)?.max(1e-3),
                up: g("up", 0.3)?,
            });
        }
        impulses.sort_by(|a, b| a.t.total_cmp(&b.t));
    }

    let bodies_v = m.get("bodies").and_then(|b| b.as_array()).ok_or("physics: missing `bodies` (an array of body groups)")?;
    let mut groups = Vec::new();
    let mut bodies = Vec::new();
    let mut objs = Vec::new();
    let mut templates = Vec::new();
    for (gi, g) in bodies_v.iter().enumerate() {
        let path = format!("physics.bodies[{gi}]");
        let gm = g.as_object().ok_or_else(|| format!("{path}: expected an object"))?;
        // Object keys plus body keys.
        let mut allowed: Vec<&str> = spec::OBJ_KEYS.to_vec();
        allowed.extend_from_slice(BODY_KEYS);
        spec::check_keys(gm, &allowed, &path)?;
        // Template: `instance` names an object in `objects`, or is a type.
        let mut om: Map<String, Value> = Map::new();
        if let Some(inst) = gm.get("instance").and_then(|x| x.as_str()) {
            let found = raw_objects.iter().position(|o| o.get("name").and_then(|n| n.as_str()) == Some(inst));
            match found {
                Some(k) => {
                    if !templates.contains(&k) {
                        templates.push(k);
                    }
                    om = raw_objects[k].as_object().cloned().unwrap_or_default();
                    for k in ["count", "lanes", "pos", "position", "rot", "rotation", "keys", "spin", "name", "collide"] {
                        om.remove(k);
                    }
                }
                None => {
                    om.insert("type".into(), Value::from(inst));
                }
            }
        }
        let layout = gm.get("layout").and_then(|x| x.as_str()).unwrap_or(if gm.contains_key("pos") || gm.contains_key("position") { "expr" } else { "pile" });
        if !LAYOUTS.contains(&layout) {
            let hint = crate::components::style::closest(layout, LAYOUTS).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(format!("{path}.layout: unknown layout `{layout}`{hint}"));
        }
        // Domino panes default to a standing portrait slab.
        if layout == "dominoes" && !gm.contains_key("size") && !om.contains_key("size") {
            om.insert("size".into(), serde_json::json!([0.9, 1.6, 0.03]));
        }
        for (k, x) in gm {
            if spec::OBJ_KEYS.contains(&k.as_str()) && k != "keys" && k != "spin" {
                om.insert(k.clone(), x.clone());
            }
        }
        if !om.contains_key("type") && !om.contains_key("text") {
            om.insert("type".into(), Value::from("box"));
        }
        let count = gm.get("count").map(|x| spec::f(x, &format!("{path}.count"))).transpose()?.unwrap_or(1.0).max(0.0) as u32;
        if count > 200_000 {
            return Err(format!("{path}: {count} bodies is too many (max 200000)"));
        }
        om.insert("count".into(), Value::from(count));
        if layout != "expr" {
            om.remove("pos");
            om.remove("position");
            om.remove("rot");
            om.remove("rotation");
        }
        let mut obj = spec::object(&Value::Object(om), shapes, mats, named, &path)?;
        obj.lanes = 1;
        let gfl = |k: &str, d: f32| -> Res<f32> { gm.get(k).map(|x| spec::f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
        let mat_glass = obj.parts.iter().any(|p| mats[p.1 as usize].transmission > 0.5);
        let col = obj.col.clone();
        let (lo, hi) = col_bounds(&col);
        let gd = GroupDef {
            col: col.clone(),
            density: gfl("density", if mat_glass { 2.5 } else { 1.0 })?,
            friction: gfl("friction", friction)?,
            restitution: gfl("restitution", restitution)?,
            ccd: gm.get("ccd").and_then(|x| x.as_bool()).unwrap_or(false),
        };
        let seed = gfl("seed", gi as f32 + 1.0)? as u32;
        let first = bodies.len() as u32;
        let vel = gm.get("velocity").map(|x| spec::vec3(x, &format!("{path}.velocity"))).transpose()?;
        let ang = gm.get("angular").map(|x| spec::vec3(x, &format!("{path}.angular"))).transpose()?;
        let awake_all = gm.get("awake").and_then(|x| x.as_bool());
        let base_y = ground.unwrap_or(0.0);
        let poses = layout_poses(layout, gm, &obj, count, (lo, hi), base_y, seed, &path)?;
        let trigger = match gm.get("trigger") {
            Some(Value::Bool(false)) => None,
            None => (layout == "dominoes").then_some(1.6),
            Some(x) => Some(spec::f(x, &format!("{path}.trigger"))?),
        };
        for (i, (pose, s)) in poses.into_iter().enumerate() {
            let vars = Vars { t: 0.0, i: i as f64, n: count as f64, ..Vars::default() };
            let linvel = vel.as_ref().map(|v| spec::eval3(v, &vars)).unwrap_or(v3(0.0, 0.0, 0.0));
            let mut angvel = ang.as_ref().map(|v| scale(spec::eval3(v, &vars), std::f32::consts::PI / 180.0)).unwrap_or(v3(0.0, 0.0, 0.0));
            if i == 0 {
                if let Some(w) = trigger {
                    // Tip the first domino forward: its picture side (local +Z) faces back, so the top goes to local -Z.
                    let r = mat_of([pose[3], pose[4], pose[5], pose[6]]);
                    angvel = add(angvel, scale(r[0], -w));
                }
            }
            // Dominoes wait asleep until hit; free-falling layouts start awake.
            let awake = awake_all.unwrap_or(layout != "dominoes" || i == 0) || len(linvel) > 0.0 || len(angvel) > 0.0;
            bodies.push(BodyInit { pose, linvel, angvel, scale: s, group: gi as u32, awake });
        }
        groups.push(gd);
        obj.pos = [Num::C(0.0), Num::C(0.0), Num::C(0.0)];
        objs.push((obj, first));
    }

    // Thin bodies need smaller steps not to pass through each other.
    let thinnest = groups.iter().map(|g| {
        let (lo, hi) = col_bounds(&g.col);
        let d = sub(hi, lo);
        d.x.min(d.y).min(d.z)
    }).fold(f32::MAX, f32::min);
    let rate = rate_v.unwrap_or(if thinnest < 0.02 { 240.0 } else { 120.0 }).clamp(30.0, 2000.0);
    let key = {
        let colliders: Vec<&Value> = raw_objects.iter().filter(|o| o.get("collide").and_then(|c| c.as_bool()) == Some(true)).collect();
        let tpl: Vec<&Value> = templates.iter().map(|&k| &raw_objects[k]).collect();
        crate::components::style::fnv(&[b"anikit-physics-v1 rapier 0.36", v.to_string().as_bytes(), serde_json::to_string(&colliders).unwrap_or_default().as_bytes(), serde_json::to_string(&tpl).unwrap_or_default().as_bytes(), &floor_y.unwrap_or(f32::NAN).to_bits().to_le_bytes()])
    };
    let movers: Vec<Mover> = scene_objs.iter().filter(|o| o.collide).map(|o| Mover { obj: o.clone(), kinematic: o.dynamic() }).collect();
    let init = Arc::new(bodies.iter().map(|b| b.pose).collect());
    let radius = bodies.iter().map(|b| {
        let (lo, hi) = col_bounds(&groups[b.group as usize].col);
        len(sub(hi, lo)) * 0.5 * b.scale
    }).collect();
    let freeze = match m.get("freeze") {
        Some(Value::Bool(false)) => 0.0,
        Some(x) if !x.is_boolean() => spec::f(x, "physics.freeze")?.max(0.0),
        _ => 0.4,
    };
    let sim = Arc::new(Sim { gravity, rate, iterations, time_scale, start, ground, record, groups, bodies, impulses, movers, sleep, init, radius, freeze, key, state: Mutex::new(None) });
    let objs = objs
        .into_iter()
        .map(|(mut o, first)| {
            o.phys = Some((sim.clone(), first));
            o
        })
        .collect();
    Ok(Parsed { sim, objs, templates })
}

fn col_bounds(c: &Col) -> (V3, V3) {
    match c {
        Col::Cuboid { c, h } => (sub(*c, *h), add(*c, *h)),
        Col::Ball => (v3(-1.0, -1.0, -1.0), v3(1.0, 1.0, 1.0)),
        Col::Cylinder { c, hh, r } => (sub(*c, v3(*r, *hh, *r)), add(*c, v3(*r, *hh, *r))),
        Col::Hull(p) => p.iter().fold((v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN)), |(a, b), q| (vmin(a, *q), vmax(b, *q))),
    }
}

/// Initial poses (and scales) for a body group.
#[allow(clippy::too_many_arguments)]
fn layout_poses(layout: &str, gm: &Map<String, Value>, obj: &ObjDef, count: u32, (lo, hi): (V3, V3), base_y: f32, seed: u32, path: &str) -> Res<Vec<(Pose7, f32)>> {
    let gfl = |k: &str, d: f32| -> Res<f32> { gm.get(k).map(|x| spec::f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
    let sc = |i: u32| -> f32 {
        let vars = Vars { i: i as f64, n: count as f64, ..Vars::default() };
        (obj.scale.at(&vars) * obj.scale_mul).max(1e-6)
    };
    let s0 = sc(0);
    let size = scale(sub(hi, lo), s0);
    let ctr = scale(add(lo, hi), 0.5 * s0);
    let origin = gm.get("origin").map(|x| vec3c(x, &format!("{path}.origin"))).transpose()?;
    let n = count as usize;
    let mut out = Vec::with_capacity(n);
    let pose_at = |p: V3, yaw: f32, pitch: f32, roll: f32, s: f32| -> (Pose7, f32) {
        let xf = Xf::new(p, v3(pitch, yaw, roll), 1.0);
        (pose7(&Xf { s, ..xf }), s)
    };
    match layout {
        "dominoes" => {
            // Standing panes: local X across, Y up, Z the thin axis, pointing back the way the wave comes (the picture side faces the trigger).
            let h = size.y;
            let spacing = gfl("spacing", h * 0.55)?;
            let gap = gfl("gap", size.x * 0.12)?;
            let heading = gfl("heading", 180.0)?;
            let path_kind = gm.get("path").and_then(|x| x.as_str()).unwrap_or("fan");
            let o = origin.unwrap_or(v3(0.0, 0.0, 0.0));
            let y = base_y + h * 0.5 - ctr.y + 0.0005;
            let hd = heading.to_radians();
            // Travel direction for heading 180° is -Z (away from a camera on +Z).
            let dir = |a: f32| v3((hd + a).sin(), 0.0, (hd + a).cos());
            match path_kind {
                "fan" => {
                    // Rows are arcs `spacing` apart; each row holds as many panes as fit.
                    let ang = gfl("angle", 90.0)?.to_radians().clamp(0.05, std::f32::consts::TAU);
                    let mut row = 0u32;
                    while out.len() < n {
                        let r = row as f32 * spacing;
                        let per = ((ang * r) / (size.x + gap)).floor().max(1.0) as usize;
                        let per = per.min(n - out.len());
                        let step = (size.x + gap) / r.max(1e-6);
                        for j in 0..per {
                            let a = (j as f32 - (per as f32 - 1.0) * 0.5) * step;
                            let p = add(o, scale(dir(a), r));
                            out.push(pose_at(v3(p.x, y, p.z), heading + 180.0 + a.to_degrees(), 0.0, 0.0, s0));
                        }
                        row += 1;
                    }
                }
                "line" | "spiral" | "snake" => {
                    let turns = gfl("turns", 3.0)?;
                    // Arc-length placements along the curve.
                    let total = spacing * (n.max(1) - 1) as f32;
                    let curve = |s: f32| -> (V3, f32) {
                        match path_kind {
                            "line" => (scale(dir(0.0), s), 0.0),
                            "snake" => {
                                // A sine path: amplitude grows with distance.
                                let amp = gfl("angle", 4.0).unwrap_or(4.0);
                                let k = turns * std::f32::consts::TAU / total.max(1e-3);
                                let lat = amp * (k * s).sin();
                                let dlat = amp * k * (k * s).cos();
                                let f = dir(0.0);
                                let side = v3(f.z, 0.0, -f.x);
                                (add(scale(f, s), scale(side, lat)), dlat.atan())
                            }
                            _ => {
                                // Archimedean spiral outwards: r = b·θ, arc length ≈ b θ² / 2.
                                let b = gfl("pitch", h * 1.2).unwrap_or(h * 1.2) / std::f32::consts::TAU;
                                let th = (2.0 * s / b).sqrt() + 1.5;
                                let r = b * th;
                                let a = th;
                                let p = v3(r * a.cos(), 0.0, r * a.sin());
                                // Tangent of (b θ cos θ, b θ sin θ).
                                let t = v3(a.cos() - th * a.sin(), 0.0, a.sin() + th * a.cos());
                                (p, t.x.atan2(t.z) - hd)
                            }
                        }
                    };
                    let _ = turns;
                    for i in 0..n {
                        let s = i as f32 * spacing;
                        let (p, a) = curve(s);
                        let p = add(o, p);
                        out.push(pose_at(v3(p.x, y, p.z), heading + 180.0 + a.to_degrees(), 0.0, 0.0, s0));
                    }
                }
                other => return Err(format!("{path}.path: unknown domino path `{other}` (fan, line, spiral, snake)")),
            }
        }
        "grid" | "wall" => {
            let g = match gm.get("grid") {
                Some(x) => vec3c(x, &format!("{path}.grid"))?,
                None => {
                    let k = (n as f32).cbrt().ceil();
                    v3(k, k, k)
                }
            };
            let (nx, ny, nz) = (g.x.max(1.0) as usize, g.y.max(1.0) as usize, g.z.max(1.0) as usize);
            let sp = match gm.get("spacing") {
                Some(x) => vec3c(x, &format!("{path}.spacing"))?,
                None => scale(size, 1.02),
            };
            let o = origin.unwrap_or(v3(-(nx as f32 - 1.0) * sp.x * 0.5, 0.0, -(nz as f32 - 1.0) * sp.z * 0.5));
            let jit = gfl("jitter", 0.0)?;
            'fill: for iy in 0..ny {
                for iz in 0..nz {
                    for ix in 0..nx {
                        let i = out.len() as u32;
                        if out.len() >= n {
                            break 'fill;
                        }
                        let off = if layout == "wall" && iy % 2 == 1 { 0.5 } else { 0.0 };
                        let p = v3(o.x + (ix as f32 + off) * sp.x, base_y + o.y + size.y * 0.5 - ctr.y + iy as f32 * sp.y + 0.0005, o.z + iz as f32 * sp.z);
                        let yaw = (hash_u(seed, i, 1) - 0.5) * jit;
                        out.push(pose_at(p, yaw, 0.0, 0.0, sc(i)));
                    }
                }
            }
            while out.len() < n {
                // More bodies than grid cells: stack extra layers.
                let i = out.len();
                let p = v3(o.x + (i % nx) as f32 * sp.x, base_y + size.y * 0.5 + (ny + i / (nx * nz)) as f32 * sp.y, o.z + ((i / nx) % nz) as f32 * sp.z);
                out.push(pose_at(p, 0.0, 0.0, 0.0, sc(i as u32)));
            }
        }
        "pile" | "rain" => {
            // A jittered lattice filling `region` from the bottom up, random orientations.
            let d = len(size) * 1.08;
            let (rlo, rhi) = match gm.get("region") {
                Some(Value::Object(r)) => (
                    r.get("min").map(|x| vec3c(x, &format!("{path}.region.min"))).transpose()?.unwrap_or(v3(-2.0, 1.0, -2.0)),
                    r.get("max").map(|x| vec3c(x, &format!("{path}.region.max"))).transpose()?.unwrap_or(v3(2.0, 6.0, 2.0)),
                ),
                Some(_) => return Err(format!("{path}.region: expected {{ \"min\": [x, y, z], \"max\": [x, y, z] }}")),
                None => {
                    let side = ((n as f32).sqrt() * d * 0.7).max(d);
                    let c = origin.unwrap_or(v3(0.0, 0.0, 0.0));
                    (v3(c.x - side * 0.5, base_y + 1.0 + c.y, c.z - side * 0.5), v3(c.x + side * 0.5, base_y + 1.0 + c.y + side, c.z + side * 0.5))
                }
            };
            let nx = ((rhi.x - rlo.x) / d).floor().max(1.0) as usize;
            let nz = ((rhi.z - rlo.z) / d).floor().max(1.0) as usize;
            let jit = gfl("jitter", 0.3)?.clamp(0.0, 1.0) * d * 0.04;
            let stagger = gfl("stagger", 0.0)?;
            for i in 0..n {
                let (ix, iz, iy) = (i % nx, (i / nx) % nz, i / (nx * nz));
                let u = |k: u32| hash_u(seed, i as u32, k);
                let p = v3(
                    rlo.x + (ix as f32 + 0.5) * d + (u(1) - 0.5) * jit * 10.0,
                    rlo.y + (iy as f32 + 0.5) * d + stagger * u(5),
                    rlo.z + (iz as f32 + 0.5) * d + (u(2) - 0.5) * jit * 10.0,
                );
                let rnd = layout == "pile" || gm.get("jitter").is_some();
                let (yaw, pitch, roll) = if rnd { (u(3) * 360.0, u(4) * 360.0, u(6) * 360.0) } else { (0.0, 0.0, 0.0) };
                out.push(pose_at(p, yaw, pitch, roll, sc(i as u32)));
            }
        }
        _ => {
            // `expr`: the object's own pos/rot expressions of `i`.
            for i in 0..count {
                let xf = obj.xf_at(0.0, i, 0);
                out.push((pose7(&xf), xf.s));
            }
        }
    }
    out.truncate(n);
    Ok(out)
}

fn shared_shape(c: &Col, s: f32) -> Option<(SharedShape, V3)> {
    Some(match c {
        Col::Cuboid { c, h } => (SharedShape::cuboid(h.x * s, h.y * s, h.z * s), scale(*c, s)),
        Col::Ball => (SharedShape::ball(s), v3(0.0, 0.0, 0.0)),
        Col::Cylinder { c, hh, r } => (SharedShape::cylinder(hh * s, r * s), scale(*c, s)),
        Col::Hull(p) => {
            let pts: Vec<Vector> = p.iter().map(|q| Vector::new(q.x * s, q.y * s, q.z * s)).collect();
            (SharedShape::convex_hull(&pts)?, v3(0.0, 0.0, 0.0))
        }
    })
}

impl Sim {
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// The initial pose of body `b` (no simulation needed).
    pub fn initial(&self, b: usize) -> (Pose7, f32) {
        (self.bodies[b].pose, self.bodies[b].scale)
    }

    pub fn scale_of(&self, b: usize) -> f32 {
        self.bodies[b].scale
    }

    fn build(&self) -> World {
        let mut world = PhysicsWorld { gravity: Vector::new(self.gravity.x, self.gravity.y, self.gravity.z), ..PhysicsWorld::default() };
        world.integration_parameters.dt = 1.0 / self.rate;
        world.integration_parameters.num_solver_iterations = self.iterations;
        if let Some(y) = self.ground {
            world.insert(RigidBodyBuilder::fixed().translation(Vector::new(0.0, y - 50.0, 0.0)), ColliderBuilder::cuboid(5000.0, 50.0, 5000.0).friction(0.6).restitution(0.05));
        }
        let mut cache: HashMap<(u32, u32), Option<(SharedShape, V3)>> = HashMap::new();
        let mut handles = Vec::with_capacity(self.bodies.len());
        for b in &self.bodies {
            let g = &self.groups[b.group as usize];
            let shp = cache.entry((b.group, b.scale.to_bits())).or_insert_with(|| shared_shape(&g.col, b.scale)).clone();
            let rb = RigidBodyBuilder::dynamic()
                .pose(to_pose(&b.pose))
                .linvel(Vector::new(b.linvel.x, b.linvel.y, b.linvel.z))
                .angvel(Vector::new(b.angvel.x, b.angvel.y, b.angvel.z))
                .can_sleep(self.sleep)
                .sleeping(!b.awake && self.sleep)
                .ccd_enabled(g.ccd);
            let h = match shp {
                Some((shape, off)) => {
                    let col = ColliderBuilder::new(shape).translation(Vector::new(off.x, off.y, off.z)).density(g.density).friction(g.friction).restitution(g.restitution);
                    world.insert(rb, col).0
                }
                None => world.bodies.insert(rb),
            };
            handles.push(h);
        }
        let mut movers = Vec::new();
        for mv in &self.movers {
            let o = &mv.obj;
            let mut hs = Vec::new();
            for lane in 0..o.lanes {
                for i in 0..o.count {
                    let xf = o.xf_at(self.start, i, lane);
                    let Some((shape, off)) = shared_shape(&o.col, xf.s) else { continue };
                    let p = to_pose(&pose7(&xf));
                    let rb = if mv.kinematic { RigidBodyBuilder::kinematic_position_based() } else { RigidBodyBuilder::fixed() }.pose(p);
                    let col = ColliderBuilder::new(shape).translation(Vector::new(off.x, off.y, off.z)).friction(0.5).restitution(0.1);
                    hs.push(world.insert(rb, col).0);
                }
            }
            movers.push(hs);
        }
        let n = handles.len();
        World { world, handles, movers, at: 0, fired: 0, still: vec![0.0; n], frozen: 0 }
    }

    fn record(&self, st: &World) -> Vec<Pose7> {
        st.handles
            .iter()
            .map(|h| {
                let b = &st.world.bodies[*h];
                let t = b.translation();
                let q = b.rotation();
                [t.x, t.y, t.z, q.x, q.y, q.z, q.w]
            })
            .collect()
    }

    /// Step the live world one record; keep the record if it's new.
    fn advance(&self, state: &mut State) {
        if state.world.is_none() {
            let clock = std::time::Instant::now();
            state.world = Some(self.build());
            if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
                eprintln!("physics: built {} bodies in {:.0} ms", self.bodies.len(), clock.elapsed().as_secs_f64() * 1e3);
            }
        }
        let st = state.world.as_mut().unwrap();
        let k = st.at + 1;
        let hz = self.record;
        let t0 = self.start + (k - 1) as f32 / hz;
        let t1 = self.start + k as f32 / hz;
        // Impulses due in (t0, t1].
        while st.fired < self.impulses.len() && self.impulses[st.fired].t <= t1 {
            let im = &self.impulses[st.fired];
            for (k, h) in st.handles.iter().enumerate() {
                let b = &mut st.world.bodies[*h];
                if st.still[k] < 0.0 {
                    let p = b.translation();
                    if len(sub(v3(p.x, p.y, p.z), im.at)) < im.radius {
                        b.set_body_type(RigidBodyType::Dynamic, true);
                        st.still[k] = 0.0;
                        st.frozen -= 1;
                    } else {
                        continue;
                    }
                }
                let p = b.translation();
                let d = sub(v3(p.x, p.y, p.z), im.at);
                let l = len(d);
                if l < im.radius {
                    let dir = norm(add(scale(d, 1.0 / l.max(1e-3)), v3(0.0, im.up, 0.0)));
                    let mag = im.strength * (1.0 - l / im.radius) * b.mass();
                    b.apply_impulse(Vector::new(dir.x * mag, dir.y * mag, dir.z * mag), true);
                }
            }
            st.fired += 1;
        }
        let vars = |t: f32| Vars { t: t as f64, gt: t as f64, ..Vars::default() };
        let ts = self.time_scale.at(&vars((t0 + t1) * 0.5)).max(0.0);
        let sim_dt = ts * (t1 - t0);
        let n = (sim_dt * self.rate).ceil() as usize;
        if n > 0 {
            st.world.integration_parameters.dt = sim_dt / n as f32;
            for s in 0..n {
                let tv = t0 + (t1 - t0) * (s + 1) as f32 / n as f32;
                for (mv, hs) in self.movers.iter().zip(&st.movers) {
                    if !mv.kinematic {
                        continue;
                    }
                    let mut j = 0;
                    for lane in 0..mv.obj.lanes {
                        for i in 0..mv.obj.count {
                            if let Some(h) = hs.get(j) {
                                let xf = mv.obj.xf_at(tv, i, lane);
                                st.world.bodies[*h].set_next_kinematic_position(to_pose(&pose7(&xf)));
                            }
                            j += 1;
                        }
                    }
                }
                st.world.step();
                state.steps += 1;
            }
            state.sim_secs += sim_dt as f64;
        }
        if n > 0 && self.freeze > 0.0 {
            self.freeze_settled(st, sim_dt, t1);
        }
        st.at = k;
        if k < state.frames.len() {
            // Re-simulating records the disk cache already had.
            return;
        }
        let asleep = st.world.islands.active_bodies().next().is_none();
        let rec = if asleep && n > 0 && st.movers.iter().all(|m| m.is_empty()) {
            state.frames.last().unwrap().clone()
        } else {
            Arc::new(self.record(st))
        };
        if let Some(path) = &state.disk {
            disk::append(path, k, state.frames.last().unwrap(), &rec, self.record);
        }
        state.frames.push(rec);
    }

    /// Settled bodies become fixed, so piles of fallen bodies stop costing
    /// solver time (a whole leaning pile is one island and would never
    /// sleep while the wave front moves). Bodies near a moving kinematic
    /// collider are woken back up.
    fn freeze_settled(&self, st: &mut World, dt: f32, t: f32) {
        let (v_eps, w_eps) = (0.015f32, 0.04f32);
        let active: Vec<RigidBodyHandle> = st.world.islands.active_bodies().collect();
        let index: HashMap<RigidBodyHandle, usize> = if active.is_empty() { HashMap::new() } else { st.handles.iter().enumerate().map(|(k, h)| (*h, k)).collect() };
        for h in active {
            let Some(&k) = index.get(&h) else { continue };
            let b = &mut st.world.bodies[h];
            if !b.is_dynamic() {
                continue;
            }
            let (v, w) = (b.linvel(), b.angvel());
            if v.length() < v_eps && w.length() < w_eps {
                st.still[k] += dt;
                if st.still[k] > self.freeze {
                    b.set_linvel(Vector::ZERO, false);
                    b.set_angvel(Vector::ZERO, false);
                    b.set_body_type(RigidBodyType::Fixed, false);
                    st.still[k] = -1.0;
                    st.frozen += 1;
                }
            } else {
                st.still[k] = 0.0;
            }
        }
        // Wake frozen bodies that a kinematic collider is about to reach.
        if st.frozen > 0 {
            for (mv, hs) in self.movers.iter().zip(&st.movers) {
                if !mv.kinematic || hs.is_empty() {
                    continue;
                }
                let (lo, hi) = col_bounds(&mv.obj.col);
                let mut j = 0;
                for lane in 0..mv.obj.lanes {
                    for i in 0..mv.obj.count {
                        let xf = mv.obj.xf_at(t + 0.25, i, lane);
                        let c = xf.point(scale(add(lo, hi), 0.5));
                        let r = len(sub(hi, lo)) * 0.5 * xf.s + 0.5;
                        for (k, h) in st.handles.iter().enumerate() {
                            if st.still[k] >= 0.0 {
                                continue;
                            }
                            let p = st.world.bodies[*h].translation();
                            if len(sub(v3(p.x, p.y, p.z), c)) < r + self.radius[k] {
                                st.world.bodies[*h].set_body_type(RigidBodyType::Dynamic, true);
                                st.still[k] = 0.0;
                                st.frozen -= 1;
                            }
                        }
                        j += 1;
                    }
                }
                let _ = j;
            }
        }
    }

    /// The two records around video time `t` and the blend between them.
    pub fn at(&self, t: f32) -> (Arc<Vec<Pose7>>, Arc<Vec<Pose7>>, f32) {
        if t <= self.start {
            return (self.init.clone(), self.init.clone(), 0.0);
        }
        let u = ((t - self.start) * self.record).max(0.0);
        let k = u.floor() as usize;
        let mut g = self.state.lock().unwrap();
        if g.is_none() {
            let disk = disk::path(self.key);
            let frames = disk.as_ref().map(|p| disk::load(p, &self.init, self.record)).unwrap_or_else(|| vec![self.init.clone()]);
            if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() && frames.len() > 1 {
                eprintln!("physics: {} records from the cache", frames.len());
            }
            *g = Some(State { frames, world: None, steps: 0, sim_secs: 0.0, disk });
        }
        let st = g.as_mut().unwrap();
        let before = (st.frames.len(), std::time::Instant::now(), st.steps);
        while st.frames.len() <= k + 1 {
            self.advance(st);
        }
        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() && st.frames.len() > before.0 {
            let w = st.world.as_ref().unwrap();
            eprintln!(
                "physics: records {}..{} ({} steps) in {:.0} ms; {} awake, {} frozen",
                before.0,
                st.frames.len(),
                st.steps - before.2,
                before.1.elapsed().as_secs_f64() * 1e3,
                w.world.islands.active_bodies().filter(|h| w.world.bodies[*h].is_dynamic()).count(),
                w.frozen
            );
        }
        (st.frames[k].clone(), st.frames[k + 1].clone(), u - k as f32)
    }

    /// How far the toppling has spread from the first body (m): the farthest
    /// body tipped more than 30°. For framing domino shots.
    pub fn front(&self, t: f32) -> f32 {
        let (a, _, _) = self.at(t);
        let o = &self.init[0];

        a.iter()
            .filter(|q| mat_of([q[3], q[4], q[5], q[6]])[1].y < 0.87)
            .map(|q| ((q[0] - o[0]).powi(2) + (q[2] - o[2]).powi(2)).sqrt())
            .fold(0.0, f32::max)
    }

    /// Simulate up to video time `t` and report (steps, simulated seconds).
    pub fn run_to(&self, t: f32) -> (u64, f64) {
        self.at(t);
        let g = self.state.lock().unwrap();
        let st = g.as_ref().unwrap();
        (st.steps, st.sim_secs)
    }
}

/// The on-disk record cache: `$ANIKIT_CACHE/sim/phys-<key>.bin` (default
/// `~/.cache/anikit`; `ANIKIT_CACHE=off` disables it). A header, then one
/// block per record holding only the bodies that moved since the previous
/// one, exact `f32`, each block checksummed. Appended as records are made, so
/// a later `still` of the same spec reads its frames instead of simulating.
mod disk {
    use super::Pose7;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    const MAGIC: &[u8; 8] = b"AKPHYS01";

    pub fn path(key: u64) -> Option<PathBuf> {
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
        Some(dir.join(format!("phys-{key:016x}.bin")))
    }

    fn sum(b: &[u8]) -> u32 {
        let mut h = 0x811c_9dc5u32;
        for &x in b {
            h = (h ^ x as u32).wrapping_mul(0x0100_0193);
        }
        h
    }

    pub fn load(p: &Path, init: &Arc<Vec<Pose7>>, hz: f32) -> Vec<Arc<Vec<Pose7>>> {
        let mut out = vec![init.clone()];
        let Ok(b) = std::fs::read(p) else { return out };
        let n = init.len();
        if b.len() < 16 || &b[..8] != MAGIC || u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize != n || f32::from_le_bytes(b[12..16].try_into().unwrap()) != hz {
            return out;
        }
        let rd = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let mut o = 16;
        while o + 8 <= b.len() {
            let (k, m) = (rd(o) as usize, rd(o + 4) as usize);
            let end = o + 8 + m * 32;
            if end + 4 > b.len() || sum(&b[o..end]) != rd(end) {
                break;
            }
            if k == out.len() {
                let mut rec: Vec<Pose7> = out.last().unwrap().as_ref().clone();
                let mut ok = true;
                for j in 0..m {
                    let q = o + 8 + j * 32;
                    let i = rd(q) as usize;
                    if i >= n {
                        ok = false;
                        break;
                    }
                    for c in 0..7 {
                        rec[i][c] = f32::from_le_bytes(b[q + 4 + c * 4..q + 8 + c * 4].try_into().unwrap());
                    }
                }
                if !ok {
                    break;
                }
                // Unchanged records share the previous one.
                if m == 0 {
                    let last = out.last().unwrap().clone();
                    out.push(last);
                } else {
                    out.push(Arc::new(rec));
                }
            } else if k > out.len() {
                break;
            }
            o = end + 4;
        }
        out
    }

    pub fn append(p: &Path, k: usize, prev: &[Pose7], rec: &[Pose7], hz: f32) {
        let mut blk = Vec::with_capacity(16);
        blk.extend_from_slice(&(k as u32).to_le_bytes());
        blk.extend_from_slice(&0u32.to_le_bytes());
        let mut m = 0u32;
        for (i, (a, b)) in prev.iter().zip(rec).enumerate() {
            if a != b {
                m += 1;
                blk.extend_from_slice(&(i as u32).to_le_bytes());
                for v in b {
                    blk.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        blk[4..8].copy_from_slice(&m.to_le_bytes());
        let s = sum(&blk);
        blk.extend_from_slice(&s.to_le_bytes());
        let fresh = std::fs::metadata(p).map(|m| m.len() == 0).unwrap_or(true);
        let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) else { return };
        if fresh {
            let mut h = MAGIC.to_vec();
            h.extend_from_slice(&(rec.len() as u32).to_le_bytes());
            h.extend_from_slice(&hz.to_le_bytes());
            let _ = f.write_all(&h);
        }
        let _ = f.write_all(&blk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim(v: Value) -> Parsed {
        // Tests always simulate.
        std::env::set_var("ANIKIT_CACHE", "off");
        let mut shapes = spec::Shapes::new();
        parse(&v, &[], &mut shapes, &mut Vec::new(), &HashMap::new(), &[], Some(0.0)).unwrap()
    }

    #[test]
    fn sphere_comes_to_rest_on_the_ground() {
        let p = sim(serde_json::json!({ "bodies": [ { "type": "sphere", "radius": 0.25, "layout": "expr", "pos": [0, 3, 0] } ] }));
        let (a, _, _) = p.sim.at(4.0);
        assert!((a[0][1] - 0.25).abs() < 0.02, "rest height {}", a[0][1]);
    }

    #[test]
    fn fast_thin_panes_dont_tunnel_through_the_ground() {
        let p = sim(serde_json::json!({ "bodies": [ { "type": "pane", "size": [0.5, 0.5, 0.01], "count": 20, "layout": "rain", "velocity": [0, -12, 0] } ] }));
        for k in 1..120 {
            let (a, _, _) = p.sim.at(k as f32 / 30.0);
            for q in a.iter() {
                assert!(q[1] > -0.02, "body below the ground at frame {k}: {}", q[1]);
            }
        }
    }

    #[test]
    fn deterministic_whatever_the_order_frames_are_asked_in() {
        let spec = serde_json::json!({ "bodies": [ { "type": "box", "size": 0.2, "count": 200, "layout": "pile" } ] });
        let a = sim(spec.clone());
        let b = sim(spec);
        let x = a.sim.at(2.0).0;
        let _ = b.sim.at(0.5);
        let _ = b.sim.at(1.2);
        let y = b.sim.at(2.0).0;
        assert_eq!(x.as_slice(), y.as_slice());
    }

    #[test]
    fn a_line_of_dominoes_all_falls() {
        let p = sim(serde_json::json!({ "bodies": [ { "type": "pane", "count": 30, "layout": "dominoes", "path": "line" } ] }));
        let (a, _, _) = p.sim.at(12.0);
        for (i, q) in a.iter().enumerate() {
            // Upright panes have their local Y along world Y.
            let up = mat_of([q[3], q[4], q[5], q[6]])[1].y;
            assert!(up < 0.8, "domino {i} still standing (up·y = {up})");
        }
    }

    #[test]
    fn standing_dominoes_wait_until_hit() {
        let p = sim(serde_json::json!({ "bodies": [ { "type": "pane", "count": 50, "layout": "dominoes", "path": "line" } ] }));
        let (a, _, _) = p.sim.at(0.4);
        let up = mat_of([a[49][3], a[49][4], a[49][5], a[49][6]])[1].y;
        assert!(up > 0.999 && (a[49][1] - p.sim.init[49][1]).abs() < 2e-3, "up {up}, y {} vs {}", a[49][1], p.sim.init[49][1]);
    }

    #[test]
    fn disk_cache_round_trips_exactly() {
        let dir = std::env::temp_dir().join(format!("anikit-phys-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.bin");
        let a = sim(serde_json::json!({ "bodies": [ { "type": "box", "size": 0.2, "count": 50, "layout": "pile" } ] }));
        let init = a.sim.init.clone();
        let recs: Vec<Arc<Vec<Pose7>>> = (0..40).map(|k| a.sim.at(k as f32 / 60.0 + 1e-4).0).collect();
        for k in 1..recs.len() {
            disk::append(&p, k, &recs[k - 1], &recs[k], 60.0);
        }
        let back = disk::load(&p, &init, 60.0);
        assert_eq!(back.len(), recs.len());
        for (x, y) in back.iter().zip(&recs) {
            assert_eq!(x.as_slice(), y.as_slice());
        }
        // A different rate is a different cache.
        assert_eq!(disk::load(&p, &init, 30.0).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quaternion_round_trip() {
        let xf = Xf::new(v3(1.0, 2.0, 3.0), v3(30.0, -70.0, 145.0), 1.0);
        let q = quat_of(&xf.r);
        let r = mat_of(q);
        for k in 0..3 {
            assert!(len(sub(r[k], xf.r[k])) < 1e-5);
        }
    }
}
