//! Open authoring: geometry agents describe themselves instead of picking a
//! preset. Every builder returns a mesh with smooth normals and UVs, which
//! the scene then BVH-traces like any other object (materials, keys,
//! expressions and `count` all apply).
//!
//! - `extrude`: any SVG path (`d`) as a solid with a rounded bevel.
//! - `lathe`: a profile `[[r, y], ...]` revolved about Y (vases, lenses, columns).
//! - `tube`: a swept circle along `points` or a path expression in `s`, with a
//!   radius that may vary along it (`"=0.05*(1-s)"`).
//! - `surface`: a parametric surface `x, y, z` as expressions of `u, v`.
//! - `heightfield`: `y = f(x, z)` over a rectangle.
//! - `mesh`: raw `vertices` and `faces`.

use super::math::*;
use super::spec::{compile, eval3, f, Num, Res};
use crate::eval::expr::Vars;
use anikit_core::raster::Mesh;
use serde_json::{Map, Value};

/// Compile an expression, renaming free variables (`s` → the evaluator's `p`, …).
fn compile_as(src: &str, map: &[(&str, &str)]) -> Res<Num> {
    let src = src.strip_prefix('=').unwrap_or(src);
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    'outer: while i < b.len() {
        for (from, to) in map {
            let w = from.as_bytes();
            let starts = b[i..].starts_with(w);
            let before = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
            let after = b.get(i + w.len()).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_' || *c == b'('));
            if starts && before && after {
                out.push_str(to);
                i += w.len();
                continue 'outer;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    compile(&out)
}

/// A number or an expression in the given variables.
fn num_as(v: &Value, map: &[(&str, &str)], path: &str) -> Res<Num> {
    match v {
        Value::Number(n) => Ok(Num::C(n.as_f64().unwrap_or(0.0) as f32)),
        Value::String(s) => compile_as(s, map).map_err(|e| format!("{path}: {e}")),
        _ => Err(format!("{path}: expected a number or an \"=expression\"")),
    }
}

/// `"=[a, b, c]"` or `["=a", "=b", "=c"]` in the given variables.
fn vec3_as(v: &Value, map: &[(&str, &str)], path: &str) -> Res<[Num; 3]> {
    match v {
        Value::Array(a) if a.len() == 3 => Ok([num_as(&a[0], map, path)?, num_as(&a[1], map, path)?, num_as(&a[2], map, path)?]),
        Value::String(s) => {
            let body = s.trim().strip_prefix('=').unwrap_or(s).trim();
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
                return Err(format!("{path}: expected 3 values"));
            }
            Ok([compile_as(&parts[0], map)?, compile_as(&parts[1], map)?, compile_as(&parts[2], map)?])
        }
        _ => Err(format!("{path}: expected \"=[x, y, z]\" or three values")),
    }
}

fn pairs(v: &Value, path: &str) -> Res<Vec<[f32; 2]>> {
    v.as_array()
        .ok_or_else(|| format!("{path}: expected [[a, b], ...]"))?
        .iter()
        .map(|p| {
            let a = p.as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("{path}: expected [a, b] pairs"))?;
            Ok([f(&a[0], path)?, f(&a[1], path)?])
        })
        .collect()
}

fn triples(v: &Value, path: &str) -> Res<Vec<V3>> {
    let a = v.as_array().ok_or_else(|| format!("{path}: expected [[x, y, z], ...]"))?;
    if a.first().is_some_and(|x| x.is_number()) {
        if a.len() % 3 != 0 {
            return Err(format!("{path}: a flat list needs 3 numbers per point"));
        }
        return a.chunks_exact(3).map(|c| Ok(v3(f(&c[0], path)?, f(&c[1], path)?, f(&c[2], path)?))).collect();
    }
    a.iter()
        .map(|p| {
            let q = p.as_array().filter(|q| q.len() == 3).ok_or_else(|| format!("{path}: expected [x, y, z] points"))?;
            Ok(v3(f(&q[0], path)?, f(&q[1], path)?, f(&q[2], path)?))
        })
        .collect()
}

/// Smooth vertex normals from face areas (for builders without analytic normals).
pub fn smooth_normals(m: &mut Mesh) {
    let nv = m.pos.len() / 3;
    let mut acc = vec![v3(0.0, 0.0, 0.0); nv];
    let p = |i: u32| v3(m.pos[i as usize * 3], m.pos[i as usize * 3 + 1], m.pos[i as usize * 3 + 2]);
    for t in m.idx.chunks_exact(3) {
        let f = cross(sub(p(t[1]), p(t[0])), sub(p(t[2]), p(t[0])));
        for &i in t {
            acc[i as usize] = add(acc[i as usize], f);
        }
    }
    m.nrm = acc.into_iter().flat_map(|n| {
        let n = norm(n);
        [n.x, n.y, n.z]
    }).collect();
}

/// A `(nu+1) × (nv+1)` grid of points into a mesh (two triangles per cell).
fn grid(pts: Vec<V3>, nu: usize, nv: usize, close_u: bool) -> Mesh {
    let mut m = Mesh::default();
    for (k, p) in pts.iter().enumerate() {
        m.pos.extend_from_slice(&[p.x, p.y, p.z]);
        let (i, j) = (k % (nu + 1), k / (nu + 1));
        m.uv.extend_from_slice(&[i as f32 / nu as f32, j as f32 / nv as f32]);
    }
    let row = (nu + 1) as u32;
    for j in 0..nv as u32 {
        for i in 0..nu as u32 {
            let a = j * row + i;
            let (b, c, d) = (a + 1, a + row, a + row + 1);
            m.idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    smooth_normals(&mut m);
    if close_u {
        // Weld the seam: average normals of the first and last column.
        for j in 0..=nv {
            let (a, b) = (j * (nu + 1), j * (nu + 1) + nu);
            for k in 0..3 {
                let v = (m.nrm[a * 3 + k] + m.nrm[b * 3 + k]) * 0.5;
                m.nrm[a * 3 + k] = v;
                m.nrm[b * 3 + k] = v;
            }
        }
    }
    m
}

/// Whether an open-geometry description reads time (`t`), so its mesh is
/// rebuilt every frame.
pub fn is_dynamic(m: &Map<String, Value>) -> bool {
    ["x", "y", "z", "path", "radius", "expr"].iter().any(|k| match m.get(*k) {
        Some(Value::String(s)) => {
            let b = s.as_bytes();
            (0..b.len()).any(|i| {
                b[i] == b't' && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) && b.get(i + 1).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_' || *c == b'('))
            })
        }
        _ => false,
    })
}

/// Build the mesh of an open-geometry object type, or `None` for other types.
pub fn build(ty: &str, m: &Map<String, Value>, path: &str, t: f32) -> Res<Option<Mesh>> {
    let t = t as f64;
    let gf = |k: &str, d: f32| -> Res<f32> { m.get(k).map(|x| f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
    let res = |k: &str, d: usize| -> Res<usize> { Ok(gf(k, d as f32)?.clamp(2.0, 2048.0) as usize) };
    Ok(Some(match ty {
        "extrude" => {
            let d = m.get("d").or(m.get("path")).and_then(|x| x.as_str()).ok_or_else(|| format!("{path}: extrude needs `d` (an SVG path)"))?;
            let cs = crate::components::text3d::parse_svg_path(d).map_err(|e| format!("{path}.d: {e}"))?;
            let cs = crate::components::text3d::normalise_logo(cs).map_err(|e| format!("{path}.d: {}", e.0))?;
            let cs: Vec<Vec<[f32; 2]>> = cs.into_iter().filter_map(crate::components::text3d::clean).collect();
            if cs.is_empty() {
                return Err(format!("{path}.d: the path has no area"));
            }
            let h = gf("height", 1.0)?;
            let depth = gf("depth", 0.2 * h)? / h.max(1e-6);
            let bevel = gf("bevel", 0.02 * h)? / h.max(1e-6);
            let mut mesh = crate::components::text3d::extrude_segs(&cs, depth * 0.5, bevel.min(depth * 0.45), 0.0, 6);
            // normalise_logo gives about one unit tall: scale to `height`, stand on y = 0.
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for q in mesh.pos.chunks_exact(3) {
                lo = lo.min(q[1]);
                hi = hi.max(q[1]);
            }
            let k = h / (hi - lo).max(1e-6);
            for q in mesh.pos.chunks_exact_mut(3) {
                q[0] *= k;
                q[1] = (q[1] - lo) * k;
                q[2] *= k;
            }
            mesh.uv = mesh.pos.chunks_exact(3).flat_map(|q| [q[0] / h + 0.5, 1.0 - q[1] / h]).collect();
            mesh
        }
        "lathe" => {
            let prof = pairs(m.get("points").ok_or_else(|| format!("{path}: lathe needs `points` [[radius, y], ...]"))?, &format!("{path}.points"))?;
            if prof.len() < 2 {
                return Err(format!("{path}.points: at least 2 points"));
            }
            let seg = res("segments", 96)?;
            // Smooth profiles (default): a centripetal-free Catmull-Rom through the
            // points, 10 steps per span, so a few points make a turned shape, not
            // a faceted one. `"smooth": false` keeps hard corners.
            let prof = if m.get("smooth").and_then(|x| x.as_bool()).unwrap_or(true) && prof.len() >= 3 {
                let n = prof.len();
                let at = |i: isize| prof[i.clamp(0, n as isize - 1) as usize];
                let mut out = Vec::with_capacity((n - 1) * 10 + 1);
                for i in 0..n - 1 {
                    let (p0, p1, p2, p3) = (at(i as isize - 1), at(i as isize), at(i as isize + 1), at(i as isize + 2));
                    for k in 0..10 {
                        let t = k as f32 / 10.0;
                        let c = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t + (-a + 3.0 * b - 3.0 * c + d) * t * t * t);
                        out.push([c(p0[0], p1[0], p2[0], p3[0]).max(0.0), c(p0[1], p1[1], p2[1], p3[1])]);
                    }
                }
                out.push(prof[n - 1]);
                out
            } else {
                prof
            };
            let mut pts = Vec::new();
            for p in &prof {
                for i in 0..=seg {
                    let a = i as f32 / seg as f32 * std::f32::consts::TAU;
                    pts.push(v3(p[0] * a.cos(), p[1], p[0] * a.sin()));
                }
            }
            grid(pts, seg, prof.len() - 1, true)
        }
        "tube" => {
            let sides = res("sides", 24)?;
            let smap = [("s", "p")];
            let (pts, radii): (Vec<V3>, Vec<f32>) = match (m.get("points"), m.get("path")) {
                (Some(p), _) => {
                    let pts = triples(p, &format!("{path}.points"))?;
                    let rs = match m.get("radius") {
                        Some(Value::String(e)) => {
                            let n = compile_as(e, &smap)?;
                            (0..pts.len()).map(|k| n.at(&Vars { t, p: k as f64 / (pts.len().max(2) - 1) as f64, ..Vars::default() })).collect()
                        }
                        _ => vec![gf("radius", 0.05)?; pts.len()],
                    };
                    (pts, rs)
                }
                (None, Some(p)) => {
                    let e = vec3_as(p, &smap, &format!("{path}.path"))?;
                    let n = res("segments", 256)?;
                    let rn = m.get("radius").map(|r| num_as(r, &smap, &format!("{path}.radius"))).transpose()?.unwrap_or(Num::C(0.05));
                    (0..=n)
                        .map(|k| {
                            let vars = Vars { t, p: k as f64 / n as f64, ..Vars::default() };
                            (eval3(&e, &vars), rn.at(&vars))
                        })
                        .unzip()
                }
                _ => return Err(format!("{path}: tube needs `points` [[x, y, z], ...] or `path` \"=[x(s), y(s), z(s)]\"")),
            };
            if pts.len() < 2 {
                return Err(format!("{path}: a tube needs at least 2 points"));
            }
            let flat: Vec<f32> = pts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
            let mut mesh = anikit_core::raster::tube_path(&flat, &radii, 1.0, sides);
            if mesh.idx.is_empty() {
                return Err(format!("{path}: degenerate tube"));
            }
            if mesh.nrm.len() != mesh.pos.len() {
                smooth_normals(&mut mesh);
            }
            // Flat end caps (rings are `sides + 1` vertices each, in path order).
            let ring = sides + 1;
            let rings = mesh.pos.len() / 3 / ring;
            if mesh.uv.len() != mesh.pos.len() / 3 * 2 {
                mesh.uv.resize(mesh.pos.len() / 3 * 2, 0.0);
            }
            for (r, c, d) in [(0, pts[0], sub(pts[0], pts[1])), (rings - 1, pts[pts.len() - 1], sub(pts[pts.len() - 1], pts[pts.len() - 2]))] {
                if radii[if r == 0 { 0 } else { radii.len() - 1 }] <= 1e-6 {
                    continue;
                }
                let nrm = norm(d);
                let base = (mesh.pos.len() / 3) as u32;
                mesh.pos.extend_from_slice(&[c.x, c.y, c.z]);
                for k in 0..ring {
                    let q = (r * ring + k) * 3;
                    let v = [mesh.pos[q], mesh.pos[q + 1], mesh.pos[q + 2]];
                    mesh.pos.extend_from_slice(&v);
                }
                for _ in 0..=ring {
                    mesh.nrm.extend_from_slice(&[nrm.x, nrm.y, nrm.z]);
                    mesh.uv.extend_from_slice(&[0.5, 0.5]);
                }
                for k in 0..sides as u32 {
                    let (a, b) = (base + 1 + k, base + 2 + k);
                    // Wind so the face points along `nrm`.
                    let pa = v3(mesh.pos[a as usize * 3], mesh.pos[a as usize * 3 + 1], mesh.pos[a as usize * 3 + 2]);
                    let pb = v3(mesh.pos[b as usize * 3], mesh.pos[b as usize * 3 + 1], mesh.pos[b as usize * 3 + 2]);
                    if dot(cross(sub(pa, c), sub(pb, c)), nrm) >= 0.0 {
                        mesh.idx.extend_from_slice(&[base, a, b]);
                    } else {
                        mesh.idx.extend_from_slice(&[base, b, a]);
                    }
                }
            }
            mesh
        }
        "surface" => {
            let uvmap = [("u", "p"), ("v", "gt")];
            let need = |k: &str| m.get(k).ok_or_else(|| format!("{path}: a surface needs `{k}` (an expression of u and v)"));
            let (ex, ey, ez) = (num_as(need("x")?, &uvmap, &format!("{path}.x"))?, num_as(need("y")?, &uvmap, &format!("{path}.y"))?, num_as(need("z")?, &uvmap, &format!("{path}.z"))?);
            let range = |k: &str| -> Res<(f32, f32, usize)> {
                match m.get(k) {
                    None => Ok((0.0, 1.0, 64)),
                    Some(Value::Array(a)) if a.len() == 3 => Ok((f(&a[0], path)?, f(&a[1], path)?, (f(&a[2], path)?.clamp(2.0, 1024.0)) as usize)),
                    Some(_) => Err(format!("{path}.{k}: expected [from, to, steps]")),
                }
            };
            let ((u0, u1, nu), (v0b, v1b, nvb)) = (range("u")?, range("v")?);
            let mut pts = Vec::with_capacity((nu + 1) * (nvb + 1));
            for j in 0..=nvb {
                for i in 0..=nu {
                    let u = u0 + (u1 - u0) * i as f32 / nu as f32;
                    let v = v0b + (v1b - v0b) * j as f32 / nvb as f32;
                    let vars = Vars { t, p: u as f64, gt: v as f64, ..Vars::default() };
                    pts.push(v3(ex.at(&vars), ey.at(&vars), ez.at(&vars)));
                }
            }
            grid(pts, nu, nvb, false)
        }
        "cloth" => super::cloth::mesh_at(m, path, t as f32)?,
        "heightfield" => {
            let xz = [("x", "p"), ("z", "gt")];
            let e = num_as(m.get("expr").or(m.get("y")).ok_or_else(|| format!("{path}: a heightfield needs `expr` (y as an expression of x and z)"))?, &xz, &format!("{path}.expr"))?;
            let (w, d) = match m.get("size") {
                Some(Value::Array(a)) if a.len() >= 2 => (f(&a[0], path)?, f(&a[a.len() - 1], path)?),
                Some(x) => {
                    let s = f(x, &format!("{path}.size"))?;
                    (s, s)
                }
                None => (4.0, 4.0),
            };
            let n = res("res", 128)?;
            let mut pts = Vec::with_capacity((n + 1) * (n + 1));
            for j in 0..=n {
                for i in 0..=n {
                    let x = (i as f32 / n as f32 - 0.5) * w;
                    let z = (j as f32 / n as f32 - 0.5) * d;
                    pts.push(v3(x, e.at(&Vars { t, p: x as f64, gt: z as f64, ..Vars::default() }), z));
                }
            }
            grid(pts, n, n, false)
        }
        "mesh" => {
            let vs = triples(m.get("vertices").ok_or_else(|| format!("{path}: a mesh needs `vertices`"))?, &format!("{path}.vertices"))?;
            let fs = m.get("faces").and_then(|x| x.as_array()).ok_or_else(|| format!("{path}: a mesh needs `faces` ([[a, b, c], ...] or a flat list)"))?;
            let mut idx = Vec::new();
            for face in fs {
                match face {
                    Value::Number(n) => idx.push(n.as_u64().unwrap_or(0) as u32),
                    Value::Array(a) => {
                        // Polygons fan-triangulate.
                        let ids: Vec<u32> = a.iter().map(|x| x.as_u64().unwrap_or(0) as u32).collect();
                        for k in 1..ids.len().saturating_sub(1) {
                            idx.extend_from_slice(&[ids[0], ids[k], ids[k + 1]]);
                        }
                    }
                    _ => return Err(format!("{path}.faces: expected vertex indices")),
                }
            }
            if idx.len() % 3 != 0 || idx.iter().any(|&i| i as usize >= vs.len()) {
                return Err(format!("{path}.faces: indices must come in threes and be < {}", vs.len()));
            }
            let mut mesh = Mesh { pos: vs.iter().flat_map(|p| [p.x, p.y, p.z]).collect(), nrm: Vec::new(), uv: Vec::new(), idx };
            if m.get("flat").and_then(|x| x.as_bool()).unwrap_or(false) {
                // Unshare vertices so every face keeps its own normal.
                let mut flat = Mesh::default();
                for (k, t) in mesh.idx.chunks_exact(3).enumerate() {
                    for &i in t {
                        flat.pos.extend_from_slice(&mesh.pos[i as usize * 3..i as usize * 3 + 3]);
                    }
                    flat.idx.extend_from_slice(&[k as u32 * 3, k as u32 * 3 + 1, k as u32 * 3 + 2]);
                }
                mesh = flat;
            }
            smooth_normals(&mut mesh);
            mesh.uv = vec![0.0; mesh.pos.len() / 3 * 2];
            mesh
        }
        _ => return Ok(None),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mesh(ty: &str, v: Value) -> Mesh {
        build(ty, v.as_object().unwrap(), "o", 0.0).unwrap().unwrap()
    }

    #[test]
    fn builders_make_closed_smooth_meshes() {
        let e = mesh("extrude", json!({ "d": "M0 0 L10 0 L10 10 L0 10 Z M3 3 L7 3 L7 7 L3 7 Z", "height": 0.5, "depth": 0.1 }));
        let ys: Vec<f32> = e.pos.chunks_exact(3).map(|p| p[1]).collect();
        assert!(ys.iter().cloned().fold(f32::MAX, f32::min).abs() < 1e-4 && (ys.iter().cloned().fold(f32::MIN, f32::max) - 0.5).abs() < 1e-3);
        let l = mesh("lathe", json!({ "points": [[0.0, 0.0], [0.3, 0.1], [0.2, 0.6], [0.0, 0.7]], "segments": 32, "smooth": false }));
        assert_eq!(l.pos.len() / 3, 33 * 4);
        let t = mesh("tube", json!({ "path": "=[cos(s*6.283), s, sin(s*6.283)]", "radius": "=0.05*(1-s)+0.01", "segments": 64 }));
        assert!(t.idx.len() > 64 * 6);
        let s = mesh("surface", json!({ "x": "=u", "y": "=sin(u*3)*0.2", "z": "=v", "u": [0, 1, 16], "v": [0, 2, 8] }));
        assert_eq!(s.pos.len() / 3, 17 * 9);
        assert!((s.pos[s.pos.len() - 1] - 2.0).abs() < 1e-5, "v runs to 2");
        let h = mesh("heightfield", json!({ "expr": "=0.1*sin(x*4)*cos(z*4)", "size": [2, 2], "res": 8 }));
        assert_eq!(h.idx.len(), 8 * 8 * 6);
        let m = mesh("mesh", json!({ "vertices": [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]], "faces": [[0, 1, 2, 3]] }));
        assert_eq!(m.idx, vec![0, 1, 2, 0, 2, 3]);
        let w = json!({ "expr": "=0.1*sin(x*4+t*3)", "size": 2, "res": 4 });
        assert!(is_dynamic(w.as_object().unwrap()) && !is_dynamic(json!({ "expr": "=sqrt(x)" }).as_object().unwrap()));
        let (a, b) = (build("heightfield", w.as_object().unwrap(), "o", 0.0).unwrap().unwrap(), build("heightfield", w.as_object().unwrap(), "o", 0.5).unwrap().unwrap());
        assert_ne!(a.pos, b.pos);
        assert!((m.nrm[2] - 1.0).abs() < 1e-5);
        assert!(build("mesh", json!({ "vertices": [[0, 0, 0]], "faces": [[0, 1, 2]] }).as_object().unwrap(), "o", 0.0).is_err());
    }
}
