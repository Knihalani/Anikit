//! Triangle bounding-volume hierarchy for the `scene3d` ray tracer.
//!
//! Built once per mesh in object space (binned SAH, 12 bins, ≤ 4 triangles
//! per leaf). Objects move rigidly, so a moving object only changes the
//! object-to-world transform its rays are mapped through; the hierarchy is
//! never rebuilt per frame.

use super::math::*;

/// One BVH node: bounds plus either two children (`count == 0`, left child
/// at `first`, right at `first + 1`) or a run of `count` triangles.
#[derive(Clone, Copy)]
struct Node {
    lo: V3,
    hi: V3,
    first: u32,
    count: u32,
}

/// A triangle ready for Möller–Trumbore: a vertex and two edges, plus the
/// three vertex normals for smooth shading.
#[derive(Clone, Copy)]
pub struct Tri {
    pub v0: V3,
    pub e1: V3,
    pub e2: V3,
    pub n: [V3; 3],
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
    nodes: Vec<Node>,
    pub tris: Vec<Tri>,
    pub lo: V3,
    pub hi: V3,
}

impl Bvh {
    /// Build from an indexed mesh (`pos`/`nrm` xyz per vertex).
    pub fn new(pos: &[f32], nrm: &[f32], idx: &[u32]) -> Bvh {
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
            tris.push(Tri { v0: a, e1, e2, n: [fix(nn(t[0])), fix(nn(t[1])), fix(nn(t[2]))] });
        }
        let n = tris.len();
        let cent: Vec<V3> = tris.iter().map(|t| add(t.v0, scale(add(t.e1, t.e2), 1.0 / 3.0))).collect();
        let tlo: Vec<V3> = tris.iter().map(|t| vmin(t.v0, vmin(add(t.v0, t.e1), add(t.v0, t.e2)))).collect();
        let thi: Vec<V3> = tris.iter().map(|t| vmax(t.v0, vmax(add(t.v0, t.e1), add(t.v0, t.e2)))).collect();
        let mut order: Vec<u32> = (0..n as u32).collect();
        let mut nodes = vec![Node { lo: v3(0.0, 0.0, 0.0), hi: v3(0.0, 0.0, 0.0), first: 0, count: n as u32 }];
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let (first, count) = (nodes[ni].first as usize, nodes[ni].count as usize);
            let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
            let (mut clo, mut chi) = (lo, hi);
            for &i in &order[first..first + count] {
                lo = vmin(lo, tlo[i as usize]);
                hi = vmax(hi, thi[i as usize]);
                clo = vmin(clo, cent[i as usize]);
                chi = vmax(chi, cent[i as usize]);
            }
            nodes[ni].lo = lo;
            nodes[ni].hi = hi;
            if count <= 4 {
                continue;
            }
            // Binned SAH over the longest centroid axis.
            let ext = sub(chi, clo);
            let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
            let (amin, aext) = (get(clo, axis), get(ext, axis));
            if aext < 1e-9 {
                continue;
            }
            const B: usize = 12;
            let mut bins = [(0u32, v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN)); B];
            let bin_of = |i: u32| (((get(cent[i as usize], axis) - amin) / aext * B as f32) as usize).min(B - 1);
            for &i in &order[first..first + count] {
                let b = &mut bins[bin_of(i)];
                b.0 += 1;
                b.1 = vmin(b.1, tlo[i as usize]);
                b.2 = vmax(b.2, thi[i as usize]);
            }
            let area = |lo: V3, hi: V3| {
                let d = sub(hi, lo);
                d.x * d.y + d.y * d.z + d.z * d.x
            };
            let mut best = (f32::MAX, 0usize);
            for s in 1..B {
                let (mut ln, mut ll, mut lh) = (0u32, v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
                for b in &bins[..s] {
                    ln += b.0;
                    if b.0 > 0 {
                        ll = vmin(ll, b.1);
                        lh = vmax(lh, b.2);
                    }
                }
                let (mut rn, mut rl, mut rh) = (0u32, v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
                for b in &bins[s..] {
                    rn += b.0;
                    if b.0 > 0 {
                        rl = vmin(rl, b.1);
                        rh = vmax(rh, b.2);
                    }
                }
                if ln == 0 || rn == 0 {
                    continue;
                }
                let c = ln as f32 * area(ll, lh) + rn as f32 * area(rl, rh);
                if c < best.0 {
                    best = (c, s);
                }
            }
            let leaf_cost = count as f32 * area(lo, hi);
            if best.0 >= leaf_cost && count <= 16 {
                continue;
            }
            let split = if best.0 == f32::MAX { B / 2 } else { best.1 };
            let slice = &mut order[first..first + count];
            let mut m = 0;
            for k in 0..slice.len() {
                if bin_of(slice[k]) < split {
                    slice.swap(k, m);
                    m += 1;
                }
            }
            if m == 0 || m == count {
                m = count / 2;
            }
            let l = nodes.len();
            nodes.push(Node { lo, hi, first: first as u32, count: m as u32 });
            nodes.push(Node { lo, hi, first: (first + m) as u32, count: (count - m) as u32 });
            nodes[ni].first = l as u32;
            nodes[ni].count = 0;
            stack.push(l);
            stack.push(l + 1);
        }
        let tris: Vec<Tri> = order.iter().map(|&i| tris[i as usize]).collect();
        let (lo, hi) = if n == 0 { (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)) } else { (nodes[0].lo, nodes[0].hi) };
        Bvh { nodes, tris, lo, hi }
    }

    /// Closest hit along `o + t·d` with `t` in `(tmin, tmax)`.
    #[inline]
    pub fn intersect(&self, o: V3, d: V3, tmin: f32, mut tmax: f32) -> Option<Hit> {
        if self.tris.is_empty() {
            return None;
        }
        let inv = v3(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let mut best: Option<Hit> = None;
        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        let mut ni = 0u32;
        if slab(&self.nodes[0], o, inv, tmin, tmax).is_none() {
            return None;
        }
        loop {
            let node = &self.nodes[ni as usize];
            if node.count > 0 {
                for k in node.first..node.first + node.count {
                    let tr = &self.tris[k as usize];
                    if let Some((t, u, v)) = tri_hit(tr, o, d, tmin, tmax) {
                        tmax = t;
                        best = Some(Hit { t, tri: k, u, v });
                    }
                }
            } else {
                let (a, b) = (node.first, node.first + 1);
                let ta = slab(&self.nodes[a as usize], o, inv, tmin, tmax);
                let tb = slab(&self.nodes[b as usize], o, inv, tmin, tmax);
                match (ta, tb) {
                    (Some(x), Some(y)) => {
                        let (near, far) = if x <= y { (a, b) } else { (b, a) };
                        if sp < 64 {
                            stack[sp] = far;
                            sp += 1;
                        }
                        ni = near;
                        continue;
                    }
                    (Some(_), None) => {
                        ni = a;
                        continue;
                    }
                    (None, Some(_)) => {
                        ni = b;
                        continue;
                    }
                    _ => {}
                }
            }
            if sp == 0 {
                break;
            }
            sp -= 1;
            ni = stack[sp];
        }
        best
    }

    /// Whether anything is hit along the segment (shadow and AO rays).
    #[inline]
    pub fn occluded(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> bool {
        if self.tris.is_empty() {
            return false;
        }
        let inv = v3(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        let mut ni = 0u32;
        if slab(&self.nodes[0], o, inv, tmin, tmax).is_none() {
            return false;
        }
        loop {
            let node = &self.nodes[ni as usize];
            if node.count > 0 {
                for k in node.first..node.first + node.count {
                    if tri_hit(&self.tris[k as usize], o, d, tmin, tmax).is_some() {
                        return true;
                    }
                }
            } else {
                let (a, b) = (node.first, node.first + 1);
                let ta = slab(&self.nodes[a as usize], o, inv, tmin, tmax).is_some();
                let tb = slab(&self.nodes[b as usize], o, inv, tmin, tmax).is_some();
                if ta && tb {
                    if sp < 64 {
                        stack[sp] = b;
                        sp += 1;
                    }
                    ni = a;
                    continue;
                } else if ta {
                    ni = a;
                    continue;
                } else if tb {
                    ni = b;
                    continue;
                }
            }
            if sp == 0 {
                return false;
            }
            sp -= 1;
            ni = stack[sp];
        }
    }
}

#[inline]
fn slab(n: &Node, o: V3, inv: V3, tmin: f32, tmax: f32) -> Option<f32> {
    let t1 = (n.lo.x - o.x) * inv.x;
    let t2 = (n.hi.x - o.x) * inv.x;
    let t3 = (n.lo.y - o.y) * inv.y;
    let t4 = (n.hi.y - o.y) * inv.y;
    let t5 = (n.lo.z - o.z) * inv.z;
    let t6 = (n.hi.z - o.z) * inv.z;
    let a = t1.min(t2).max(t3.min(t4)).max(t5.min(t6)).max(tmin);
    let b = t1.max(t2).min(t3.max(t4)).min(t5.max(t6)).min(tmax);
    (a <= b).then_some(a)
}

#[inline]
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
        }
    }
}
