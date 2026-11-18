//! Cloth-lite: a grid of particles held together by distance constraints.
//!
//! Verlet integration with position-based constraints (Jakobsen 2001; Müller
//! et al. 2007): stretch along the grid, shear across the diagonals and bend
//! across two cells, solved in a fixed order (deterministic at any thread
//! count). Gravity, wind with gusts (pushing along the surface normal, so the
//! cloth billows), pins, sphere colliders and the floor. Frames are cached
//! with checkpoints, so they render in any order.
//!
//! It's an open-geometry object (`"type": "cloth"`): its mesh is rebuilt for
//! every frame from the simulation, in world space (leave `pos` unset; place
//! it with `center`).

use anikit_core::raster::Mesh;
use super::math::*;
use super::spec::{f, Res};
use anikit_core::noise::Noise;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Simulation steps per second.
const HZ: f32 = 240.0;
const CHECK_EVERY: u64 = 240;

pub const KEYS: &[&str] = &["pin", "hang", "wind", "gust", "gravity", "stiffness", "bend", "damping", "colliders", "floor", "center", "iterations", "seed", "thickness", "friction"];

struct Params {
    nu: usize,
    nv: usize,
    rest: (f32, f32),
    pins: Vec<usize>,
    p0: Vec<V3>,
    gravity: f32,
    wind: V3,
    gust: f32,
    stiffness: f32,
    bend: f32,
    damping: f32,
    spheres: Vec<(V3, f32)>,
    floor: Option<f32>,
    iterations: u32,
    thickness: f32,
    friction: f32,
    seed: u32,
}

#[derive(Clone)]
struct State {
    step: u64,
    x: Vec<V3>,
    prev: Vec<V3>,
}

struct Cloth {
    p: Params,
    state: Mutex<(Vec<State>, Vec<State>, HashMap<u64, Arc<Vec<V3>>>)>,
}

fn v3a(x: &Value, path: &str) -> Res<V3> {
    let a = x.as_array().filter(|a| a.len() == 3).ok_or_else(|| format!("{path}: expected [x, y, z]"))?;
    Ok(v3(f(&a[0], path)?, f(&a[1], path)?, f(&a[2], path)?))
}

fn params(m: &Map<String, Value>, path: &str) -> Res<Params> {
    let gf = |k: &str, d: f32| -> Res<f32> { m.get(k).map(|x| f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
    let (w, h) = match m.get("size") {
        Some(Value::Array(a)) if a.len() >= 2 => (f(&a[0], path)?, f(&a[1], path)?),
        Some(x) => {
            let s = f(x, &format!("{path}.size"))?;
            (s, s)
        }
        None => (2.0, 1.5),
    };
    let (w, h) = (w.max(0.01), h.max(0.01));
    let nu = gf("res", 48.0)?.clamp(2.0, 160.0) as usize;
    let nv = ((nu as f32 * h / w).round() as usize).clamp(2, 160);
    let center = m.get("center").map(|x| v3a(x, &format!("{path}.center"))).transpose()?.unwrap_or(v3(0.0, 1.5, 0.0));
    let hang = m.get("hang").and_then(|x| x.as_str()).unwrap_or("vertical");
    if !["vertical", "horizontal"].contains(&hang) {
        return Err(format!("{path}.hang: unknown `{hang}` (vertical: a curtain or flag in the XY plane; horizontal: a sheet lying flat)"));
    }
    let row = nu + 1;
    let mut p0 = Vec::with_capacity(row * (nv + 1));
    for j in 0..=nv {
        for i in 0..=nu {
            let (a, b) = (i as f32 / nu as f32 - 0.5, j as f32 / nv as f32 - 0.5);
            p0.push(if hang == "vertical" { v3(center.x + a * w, center.y - b * h, center.z) } else { v3(center.x + a * w, center.y, center.z + b * h) });
        }
    }
    let pin = m.get("pin").and_then(|x| x.as_str()).unwrap_or(if hang == "vertical" { "top" } else { "none" });
    let pins: Vec<usize> = match pin {
        "top" => (0..row).collect(),
        "corners" => vec![0, nu],
        "left" => (0..=nv).map(|j| j * row).collect(),
        "none" => Vec::new(),
        o => return Err(format!("{path}.pin: unknown `{o}` (top, corners, left, none)")),
    };
    let mut spheres = Vec::new();
    if let Some(cs) = m.get("colliders") {
        let a = cs.as_array().ok_or_else(|| format!("{path}.colliders: expected [{{ \"pos\": [x, y, z], \"radius\": r }}]"))?;
        for (k, c) in a.iter().enumerate() {
            let p = format!("{path}.colliders[{k}]");
            let o = c.as_object().ok_or_else(|| format!("{p}: expected {{ \"pos\", \"radius\" }}"))?;
            super::spec::check_keys(o, &["pos", "radius"], &p)?;
            let pos = o.get("pos").map(|x| v3a(x, &format!("{p}.pos"))).transpose()?.ok_or_else(|| format!("{p}: missing `pos`"))?;
            let r = o.get("radius").map(|x| f(x, &format!("{p}.radius"))).transpose()?.unwrap_or(0.5);
            spheres.push((pos, r.max(0.0)));
        }
    }
    let floor = match m.get("floor") {
        None | Some(Value::Bool(true)) => Some(0.0),
        Some(Value::Bool(false)) | Some(Value::Null) => None,
        Some(x) => Some(f(x, &format!("{path}.floor"))?),
    };
    Ok(Params {
        nu,
        nv,
        rest: (w / nu as f32, h / nv as f32),
        pins,
        p0,
        gravity: gf("gravity", -9.8)?,
        wind: m.get("wind").map(|x| v3a(x, &format!("{path}.wind"))).transpose()?.unwrap_or(v3(0.0, 0.0, 0.0)),
        gust: gf("gust", 0.5)?.max(0.0),
        stiffness: gf("stiffness", 1.0)?.clamp(0.0, 1.0),
        bend: gf("bend", 0.15)?.clamp(0.0, 1.0),
        damping: gf("damping", 0.01)?.clamp(0.0, 0.5),
        spheres,
        floor,
        iterations: gf("iterations", 8.0)?.clamp(1.0, 64.0) as u32,
        thickness: gf("thickness", 0.01)?.max(0.0),
        friction: gf("friction", 0.5)?.clamp(0.0, 1.0),
        seed: gf("seed", 1.0)? as u32,
    })
}

/// Vertex normals of the grid (area-weighted face normals).
fn normals(x: &[V3], nu: usize, nv: usize) -> Vec<V3> {
    let row = nu + 1;
    let mut n = vec![v3(0.0, 0.0, 0.0); x.len()];
    for j in 0..nv {
        for i in 0..nu {
            let a = j * row + i;
            let (b, c, d) = (a + 1, a + row, a + row + 1);
            let f1 = cross(sub(x[c], x[a]), sub(x[b], x[a]));
            let f2 = cross(sub(x[c], x[b]), sub(x[d], x[b]));
            for k in [a, b, c] {
                n[k] = add(n[k], f1);
            }
            for k in [b, c, d] {
                n[k] = add(n[k], f2);
            }
        }
    }
    n.iter().map(|v| if dot(*v, *v) > 1e-20 { norm(*v) } else { v3(0.0, 0.0, 1.0) }).collect()
}

impl Cloth {
    fn init(&self) -> State {
        State { step: 0, x: self.p.p0.clone(), prev: self.p.p0.clone() }
    }

    fn step(&self, s: &mut State, noise: &Noise) {
        let p = &self.p;
        let dt = 1.0 / HZ;
        let t = s.step as f32 * dt;
        let (nu, nv) = (p.nu, p.nv);
        let row = nu + 1;
        let mut pinned = vec![false; s.x.len()];
        for &k in &p.pins {
            pinned[k] = true;
        }
        // Wind pushes along the surface normal (the cloth catches it and billows).
        let nrm = if p.wind != v3(0.0, 0.0, 0.0) || p.gust > 0.0 { normals(&s.x, nu, nv) } else { Vec::new() };
        for k in 0..s.x.len() {
            if pinned[k] {
                continue;
            }
            let x = s.x[k];
            let v = scale(sub(x, s.prev[k]), HZ);
            let mut a = v3(0.0, p.gravity, 0.0);
            if !nrm.is_empty() {
                let g = |o: f64| noise.noise3(x.x as f64 * 0.6 + o, x.y as f64 * 0.6, t as f64 * 0.8 + x.z as f64 * 0.6) as f32;
                let wl = len(p.wind).max(1.0);
                let w = add(p.wind, scale(v3(g(0.0), g(17.0) * 0.5, g(31.0)), p.gust * wl));
                let rel = sub(w, v);
                a = add(a, scale(nrm[k], dot(nrm[k], rel) * 1.2));
            }
            let nx = add(x, add(scale(sub(x, s.prev[k]), 1.0 - p.damping), scale(a, dt * dt)));
            s.prev[k] = x;
            s.x[k] = nx;
        }
        // Constraints, in a fixed order: stretch, shear, bend.
        let (rx, ry) = p.rest;
        let rd = (rx * rx + ry * ry).sqrt();
        let solve = |x: &mut Vec<V3>, a: usize, b: usize, rest: f32, k: f32| {
            let (wa, wb) = (if pinned[a] { 0.0 } else { 1.0 }, if pinned[b] { 0.0 } else { 1.0 });
            let ws = wa + wb;
            if ws == 0.0 {
                return;
            }
            let d = sub(x[b], x[a]);
            let l = len(d);
            if l < 1e-9 {
                return;
            }
            let c = scale(d, (l - rest) / l * k / ws);
            x[a] = add(x[a], scale(c, wa));
            x[b] = sub(x[b], scale(c, wb));
        };
        for _ in 0..p.iterations {
            for j in 0..=nv {
                for i in 0..=nu {
                    let a = j * row + i;
                    if i < nu {
                        solve(&mut s.x, a, a + 1, rx, p.stiffness);
                    }
                    if j < nv {
                        solve(&mut s.x, a, a + row, ry, p.stiffness);
                    }
                    if i < nu && j < nv {
                        solve(&mut s.x, a, a + row + 1, rd, p.stiffness * 0.5);
                        solve(&mut s.x, a + 1, a + row, rd, p.stiffness * 0.5);
                    }
                    if p.bend > 0.0 {
                        if i + 2 <= nu {
                            solve(&mut s.x, a, a + 2, rx * 2.0, p.bend);
                        }
                        if j + 2 <= nv {
                            solve(&mut s.x, a, a + 2 * row, ry * 2.0, p.bend);
                        }
                    }
                }
            }
            // Collisions: out of spheres and above the floor, with friction.
            for k in 0..s.x.len() {
                if pinned[k] {
                    continue;
                }
                let mut hit = false;
                for &(c, r) in &p.spheres {
                    let d = sub(s.x[k], c);
                    let l = len(d);
                    let rr = r + p.thickness;
                    if l < rr {
                        s.x[k] = add(c, scale(d, rr / l.max(1e-6)));
                        hit = true;
                    }
                }
                if let Some(fy) = p.floor {
                    if s.x[k].y < fy + p.thickness {
                        s.x[k].y = fy + p.thickness;
                        hit = true;
                    }
                }
                if hit && p.friction > 0.0 {
                    // Friction: lose some of this step's motion.
                    s.prev[k] = add(s.prev[k], scale(sub(s.x[k], s.prev[k]), p.friction));
                }
            }
        }
        s.step += 1;
    }

    fn at(&self, t: f32) -> Arc<Vec<V3>> {
        let target = (t.max(0.0) * HZ).round() as u64;
        let mut g = self.state.lock().unwrap();
        let (lives, cps, frames) = &mut *g;
        if let Some(fr) = frames.get(&target) {
            return fr.clone();
        }
        let best = lives.iter().enumerate().filter(|(_, l)| l.step <= target).max_by_key(|(_, l)| l.step).map(|(k, l)| (k, l.step));
        let cp = cps.iter().filter(|c| c.step <= target).map(|c| c.step).max();
        let mut s = match (best, cp) {
            (Some((k, ls)), c) if c.is_none_or(|c| ls >= c) => lives.swap_remove(k),
            (_, Some(c)) => cps.iter().find(|x| x.step == c).cloned().unwrap(),
            _ => self.init(),
        };
        let noise = Noise::new(self.p.seed);
        let (from, clock) = (s.step, std::time::Instant::now());
        while s.step < target {
            self.step(&mut s, &noise);
            if s.step % CHECK_EVERY == 0 && !cps.iter().any(|c| c.step == s.step) && cps.len() < 256 {
                cps.push(s.clone());
            }
        }
        if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() && target > from {
            eprintln!("cloth: {} points, steps {from}..{target} in {:.0} ms", s.x.len(), clock.elapsed().as_secs_f64() * 1e3);
        }
        let fr = Arc::new(s.x.clone());
        frames.insert(target, fr.clone());
        if frames.len() > 64 {
            let mut keys: Vec<u64> = frames.keys().copied().collect();
            keys.sort_unstable();
            for k in keys.iter().take(keys.len() - 64) {
                frames.remove(k);
            }
        }
        lives.push(s);
        if lives.len() > 8 {
            let k = lives.iter().enumerate().min_by_key(|(_, l)| l.step).map(|(k, _)| k).unwrap();
            lives.swap_remove(k);
        }
        fr
    }
}

fn cache() -> &'static Mutex<HashMap<u64, Arc<Cloth>>> {
    static C: OnceLock<Mutex<HashMap<u64, Arc<Cloth>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The cloth's mesh at `t` (world space).
pub fn mesh_at(m: &Map<String, Value>, path: &str, t: f32) -> Res<Mesh> {
    let key = crate::components::style::fnv(&[b"cloth", serde_json::to_string(m).unwrap_or_default().as_bytes()]);
    let cloth = {
        let mut c = cache().lock().unwrap();
        match c.get(&key) {
            Some(x) => x.clone(),
            None => {
                let x = Arc::new(Cloth { p: params(m, path)?, state: Mutex::new((Vec::new(), Vec::new(), HashMap::new())) });
                c.insert(key, x.clone());
                x
            }
        }
    };
    let x = cloth.at(t);
    let (nu, nv) = (cloth.p.nu, cloth.p.nv);
    let row = nu + 1;
    let mut mesh = Mesh::default();
    for (k, p) in x.iter().enumerate() {
        mesh.pos.extend_from_slice(&[p.x, p.y, p.z]);
        mesh.uv.extend_from_slice(&[(k % row) as f32 / nu as f32, (k / row) as f32 / nv as f32]);
    }
    for n in normals(&x, nu, nv) {
        mesh.nrm.extend_from_slice(&[n.x, n.y, n.z]);
    }
    for j in 0..nv as u32 {
        for i in 0..nu as u32 {
            let a = j * row as u32 + i;
            let (b, c, d) = (a + 1, a + row as u32, a + row as u32 + 1);
            mesh.idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(v: serde_json::Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn hangs_from_its_pins_and_keeps_its_length() {
        let c = m(serde_json::json!({ "type": "cloth", "size": [1, 1], "res": 16, "wind": [0, 0, 2], "seed": 3 }));
        let a = mesh_at(&c, "c", 2.0).unwrap();
        // The top row stays put; the rest sags and swings but doesn't stretch much.
        assert!((a.pos[1] - 2.0).abs() < 1e-5, "pinned corner at y {}", a.pos[1]);
        let p = |k: usize| v3(a.pos[k * 3], a.pos[k * 3 + 1], a.pos[k * 3 + 2]);
        let mut worst: f32 = 0.0;
        for j in 0..16 {
            worst = worst.max(len(sub(p((j + 1) * 17), p(j * 17))) / (1.0 / 16.0));
        }
        assert!(worst < 1.15, "stretched {worst}×");
    }

    #[test]
    fn drapes_over_a_sphere_and_frames_in_any_order() {
        let c = m(serde_json::json!({ "type": "cloth", "hang": "horizontal", "size": 2, "res": 20, "center": [0, 1.4, 0], "colliders": [{ "pos": [0, 0.6, 0], "radius": 0.6 }] }));
        let late = mesh_at(&c, "c", 2.5).unwrap();
        // The middle rests on the sphere's top; the edges hang below it.
        let mid = (10 * 21 + 10) * 3;
        assert!((late.pos[mid + 1] - 1.21).abs() < 0.05, "middle at y {}", late.pos[mid + 1]);
        assert!(late.pos[1] < 1.0, "corner at y {}", late.pos[1]);
        // A fresh simulation asked for an earlier frame first agrees exactly.
        let c2 = m(serde_json::json!({ "type": "cloth", "hang": "horizontal", "size": 2, "res": 20, "center": [0, 1.4, 0], "colliders": [{ "pos": [0, 0.6, 0], "radius": 0.6 }], "seed": 1 }));
        let _ = mesh_at(&c2, "c", 1.0).unwrap();
        let again = mesh_at(&c2, "c", 2.5).unwrap();
        assert_eq!(late.pos, again.pos);
    }
}
