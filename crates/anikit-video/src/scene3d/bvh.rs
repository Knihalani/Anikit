//! Triangle bounding-volume hierarchy for the `scene3d` ray tracer.
//!
//! Built once per mesh in object space (binned SAH with spatial splits, see
//! [`super::kernel::sbvh`]). Objects move rigidly, so a moving object only changes the
//! object-to-world transform its rays are mapped through; the hierarchy is
//! never rebuilt per frame.

use super::kernel::sbvh;
use super::kernel::wide::{BNode as Node, Boxes8, Mesh8};
use super::math::*;

/// A triangle ready for Möller–Trumbore: a vertex and two edges, plus the
/// three vertex normals for smooth shading.
#[derive(Clone, Copy)]
pub struct Tri {
    pub v0: V3,
    pub e1: V3,
    pub e2: V3,
    pub n: [V3; 3],
    /// Texture coordinates per vertex.
    pub uv: [[f32; 2]; 3],
}

/// Closest hit in object space.
#[derive(Clone, Copy)]
pub struct Hit {
    pub t: f32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
}

pub struct Bvh {
    /// The binary tree the 8-wide one was collapsed from (tests traverse it as a reference).
    #[allow(dead_code)]
    nodes: Vec<Node>,
    /// The traversal structure (see [`super::kernel::wide`]).
    wide: Mesh8,
    pub tris: Vec<Tri>,
    pub lo: V3,
    pub hi: V3,
}

impl Bvh {
    /// Build from an indexed mesh (`pos`/`nrm` xyz per vertex).
    pub fn new(pos: &[f32], nrm: &[f32], idx: &[u32]) -> Bvh {
        Bvh::with_uv(pos, nrm, &[], idx)
    }

    /// Build from an indexed mesh with texture coordinates (`uv`: two per vertex, may be empty).
    pub fn with_uv(pos: &[f32], nrm: &[f32], uv: &[f32], idx: &[u32]) -> Bvh {
        let tc = |i: u32| if uv.len() >= (i as usize + 1) * 2 { [uv[i as usize * 2], uv[i as usize * 2 + 1]] } else { [0.0, 0.0] };
        let p = |i: u32| v3(pos[i as usize * 3], pos[i as usize * 3 + 1], pos[i as usize * 3 + 2]);
        let nn = |i: u32| {
            if nrm.len() >= pos.len() {
                norm(v3(nrm[i as usize * 3], nrm[i as usize * 3 + 1], nrm[i as usize * 3 + 2]))
            } else {
                v3(0.0, 0.0, 0.0)
            }
        };
        let mut tris: Vec<Tri> = Vec::with_capacity(idx.len() / 3);
        for t in idx.chunks_exact(3) {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let e1 = sub(b, a);
            let e2 = sub(c, a);
            let g = cross(e1, e2);
            if dot(g, g) < 1e-20 {
                continue;
            }
            let gn = norm(g);
            let fix = |n: V3| if dot(n, n) < 0.5 { gn } else { n };
            tris.push(Tri { v0: a, e1, e2, n: [fix(nn(t[0])), fix(nn(t[1])), fix(nn(t[2]))], uv: [tc(t[0]), tc(t[1]), tc(t[2])] });
        }
        let n = tris.len();
        let verts: Vec<[V3; 3]> = tris.iter().map(|t| [t.v0, add(t.v0, t.e1), add(t.v0, t.e2)]).collect();
        let (nodes, order) = sbvh::build(&verts);
        let tris: Vec<Tri> = order.iter().map(|&i| tris[i as usize]).collect();
        let (lo, hi) = if n == 0 { (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)) } else { (nodes[0].lo, nodes[0].hi) };
        let geo: Vec<(V3, V3, V3)> = tris.iter().map(|t| (t.v0, t.e1, t.e2)).collect();
        let wide = if n == 0 { Mesh8::new(&[], &[]) } else { Mesh8::new(&nodes, &geo) };
        Bvh { nodes, wide, tris, lo, hi }
    }

    /// Closest hit along `o + t·d` with `t` in `(tmin, tmax)`.
    #[inline]
    pub fn intersect(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<Hit> {
        self.wide.closest(o, d, tmin, tmax).map(|h| Hit { t: h.t, tri: h.tri, u: h.u, v: h.v })
    }

    /// Whether anything is hit along the segment (shadow and AO rays).
    #[inline]
    pub fn occluded(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> bool {
        self.wide.occluded(o, d, tmin, tmax)
    }

    /// Reference traversal of the binary tree (tests compare against it).
    #[cfg(test)]
    fn intersect2(&self, o: V3, d: V3, tmin: f32, mut tmax: f32) -> Option<f32> {
        let inv = v3(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let mut best = None;
        let mut stack = vec![0u32];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni as usize];
            if slab(node, o, inv, tmin, tmax).is_none() {
                continue;
            }
            if node.count > 0 {
                for k in node.first..node.first + node.count {
                    if let Some((t, _, _)) = tri_hit(&self.tris[k as usize], o, d, tmin, tmax) {
                        tmax = t;
                        best = Some(t);
                    }
                }
            } else {
                stack.push(node.first);
                stack.push(node.first + 1);
            }
        }
        best
    }
}

/// Top-level hierarchy over world-space boxes (one per object instance),
/// rebuilt every frame (cheap: median splits over a few thousand boxes).
pub struct Tlas {
    wide: Boxes8,
}

impl Tlas {
    pub fn new(boxes: &[(V3, V3)]) -> Tlas {
        let n = boxes.len();
        let mut items: Vec<u32> = (0..n as u32).collect();
        let mut nodes = vec![Node { lo: v3(0.0, 0.0, 0.0), hi: v3(0.0, 0.0, 0.0), first: 0, count: n as u32 }];
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let (first, count) = (nodes[ni].first as usize, nodes[ni].count as usize);
            let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
            let (mut clo, mut chi) = (lo, hi);
            for &i in &items[first..first + count] {
                let (a, b) = boxes[i as usize];
                lo = vmin(lo, a);
                hi = vmax(hi, b);
                let c = scale(add(a, b), 0.5);
                clo = vmin(clo, c);
                chi = vmax(chi, c);
            }
            nodes[ni].lo = lo;
            nodes[ni].hi = hi;
            if count <= 2 {
                continue;
            }
            let ext = sub(chi, clo);
            let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
            let key = |i: u32| {
                let (a, b) = boxes[i as usize];
                get(a, axis) + get(b, axis)
            };
            let slice = &mut items[first..first + count];
            let m = count / 2;
            slice.select_nth_unstable_by(m, |a, b| key(*a).total_cmp(&key(*b)));
            let l = nodes.len();
            nodes.push(Node { lo, hi, first: first as u32, count: m as u32 });
            nodes.push(Node { lo, hi, first: (first + m) as u32, count: (count - m) as u32 });
            nodes[ni].first = l as u32;
            nodes[ni].count = 0;
            stack.push(l);
            stack.push(l + 1);
        }
        Tlas { wide: if n == 0 { Boxes8::new(&[], items) } else { Boxes8::new(&nodes, items) } }
    }

    /// Visit items whose boxes the ray enters before `tmax` (nearest first);
    /// `f(item, tmax) -> new tmax` (return a smaller value on a hit, or
    /// `-1.0` to stop early for shadow rays).
    #[inline]
    pub fn visit(&self, o: V3, d: V3, tmin: f32, tmax: f32, f: impl FnMut(u32, f32) -> f32) {
        self.wide.visit(o, d, tmin, tmax, f)
    }
}

/// NaN-oblivious min/max (plain compare + select; `f32::min` adds NaN handling).
#[inline(always)]
fn fmin(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}
#[inline(always)]
fn fmax(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

#[inline(always)]
#[cfg_attr(not(test), allow(dead_code))]
fn slab(n: &Node, o: V3, inv: V3, tmin: f32, tmax: f32) -> Option<f32> {
    let t1 = (n.lo.x - o.x) * inv.x;
    let t2 = (n.hi.x - o.x) * inv.x;
    let t3 = (n.lo.y - o.y) * inv.y;
    let t4 = (n.hi.y - o.y) * inv.y;
    let t5 = (n.lo.z - o.z) * inv.z;
    let t6 = (n.hi.z - o.z) * inv.z;
    let a = fmax(fmax(fmax(fmin(t1, t2), fmin(t3, t4)), fmin(t5, t6)), tmin);
    let b = fmin(fmin(fmin(fmax(t1, t2), fmax(t3, t4)), fmax(t5, t6)), tmax);
    (a <= b).then_some(a)
}

#[inline]
#[cfg_attr(not(test), allow(dead_code))]
fn tri_hit(tr: &Tri, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<(f32, f32, f32)> {
    let p = cross(d, tr.e2);
    let det = dot(tr.e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let id = 1.0 / det;
    let s = sub(o, tr.v0);
    let u = dot(s, p) * id;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, tr.e1);
    let v = dot(d, q) * id;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(tr.e2, q) * id;
    (t > tmin && t < tmax).then_some((t, u, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_match_brute_force() {
        let m = anikit_core::raster::sphere(12, 24);
        let b = Bvh::new(&m.pos, &m.nrm, &m.idx);
        let mut seed = 1u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        for _ in 0..500 {
            let o = v3(rnd() * 3.0, rnd() * 3.0, 3.0);
            let d = norm(v3(rnd() * 0.5, rnd() * 0.5, -1.0));
            let fast = b.intersect(o, d, 1e-4, 1e9).map(|h| h.t);
            let mut best: Option<f32> = None;
            for t in &b.tris {
                if let Some((t, _, _)) = tri_hit(t, o, d, 1e-4, best.unwrap_or(1e9)) {
                    best = Some(t);
                }
            }
            assert_eq!(fast.is_some(), best.is_some());
            if let (Some(a), Some(c)) = (fast, best) {
                assert!((a - c).abs() < 1e-4);
            }
            assert_eq!(b.occluded(o, d, 1e-4, 1e9), best.is_some());
            let two = b.intersect2(o, d, 1e-4, 1e9);
            assert_eq!(two.is_some(), best.is_some());
        }
    }

    /// Long thin triangles take spatial splits (references duplicated):
    /// hits must still match a brute-force search over the original mesh.
    #[test]
    fn spatial_splits_match_brute_force() {
        // A fan of long slivers across a thin slab, plus a few small ones.
        let (mut pos, mut idx) = (Vec::new(), Vec::new());
        let mut seed = 7u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        for _ in 0..400 {
            let (y, z) = (rnd() * 0.2, rnd() * 0.05);
            let base = (pos.len() / 3) as u32;
            pos.extend_from_slice(&[-2.0, y, z, 2.0, y + rnd() * 0.01, z, rnd() * 4.0 - 2.0, y + 0.01, z + rnd() * 0.02]);
            idx.extend_from_slice(&[base, base + 1, base + 2]);
        }
        let b = Bvh::new(&pos, &[], &idx);
        let orig: Vec<Tri> = idx
            .chunks_exact(3)
            .map(|t| {
                let p = |i: u32| v3(pos[i as usize * 3], pos[i as usize * 3 + 1], pos[i as usize * 3 + 2]);
                let a = p(t[0]);
                Tri { v0: a, e1: sub(p(t[1]), a), e2: sub(p(t[2]), a), n: [a; 3], uv: [[0.0; 2]; 3] }
            })
            .collect();
        assert!(b.tris.len() > orig.len(), "expected spatial splits to duplicate some triangles");
        for _ in 0..2000 {
            let o = v3(rnd() * 4.0 - 2.0, rnd() * 0.4 - 0.1, 1.0);
            let d = norm(v3(rnd() * 0.4 - 0.2, rnd() * 0.2 - 0.1, -1.0));
            let mut best: Option<f32> = None;
            for t in &orig {
                if let Some((t, _, _)) = tri_hit(t, o, d, 1e-4, best.unwrap_or(1e9)) {
                    best = Some(t);
                }
            }
            let fast = b.intersect(o, d, 1e-4, 1e9);
            assert_eq!(fast.map(|h| h.t), best);
            assert_eq!(b.occluded(o, d, 1e-4, 1e9), best.is_some());
            assert_eq!(b.intersect2(o, d, 1e-4, 1e9), best);
        }
    }
}
