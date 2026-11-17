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

/// The part of a triangle traversal touches (36 bytes, cache friendly);
/// normals and UVs live in [`Bvh::tris`] and are read only for the hit.
#[derive(Clone, Copy)]
pub struct Hot {
    pub v0: V3,
    pub e1: V3,
    pub e2: V3,
}

/// Four children per node, bounds stored lane-wise so the four slab tests
/// run side by side (LLVM vectorises the lane loops). `cnt[k] == 0` means
/// child `k` is the node `kid[k]`; otherwise it is a leaf of `cnt[k]`
/// triangles starting at `kid[k]`; `kid[k] == u32::MAX` is an empty lane.
#[derive(Clone, Copy)]
#[repr(C, align(32))]
struct Node4 {
    lo: [[f32; 4]; 3],
    hi: [[f32; 4]; 3],
    kid: [u32; 4],
    cnt: [u32; 4],
}

pub struct Bvh {
    /// The binary tree the 4-wide one was collapsed from (tests traverse it as a reference).
    #[allow(dead_code)]
    nodes: Vec<Node>,
    nodes4: Vec<Node4>,
    pub tris: Vec<Tri>,
    hot: Vec<Hot>,
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
        let hot: Vec<Hot> = tris.iter().map(|t| Hot { v0: t.v0, e1: t.e1, e2: t.e2 }).collect();
        let (lo, hi) = if n == 0 { (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0)) } else { (nodes[0].lo, nodes[0].hi) };
        let nodes4 = if n == 0 { Vec::new() } else { collapse(&nodes) };
        Bvh { nodes, nodes4, tris, hot, lo, hi }
    }

    /// Closest hit along `o + t·d` with `t` in `(tmin, tmax)`.
    #[inline]
    pub fn intersect(&self, o: V3, d: V3, tmin: f32, mut tmax: f32) -> Option<Hit> {
        if self.nodes4.is_empty() {
            return None;
        }
        let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
        let org = [o.x, o.y, o.z];
        let mut best: Option<Hit> = None;
        let mut stack = [(0u32, 0u32, 0.0f32); 96];
        let mut sp = 0usize;
        // (child, count, tnear); count 0 = inner node.
        stack[0] = (0, 0, tmin);
        sp += 1;
        while sp > 0 {
            sp -= 1;
            let (kid, cnt, tn) = stack[sp];
            if tn > tmax {
                continue;
            }
            if cnt > 0 {
                for k in kid..kid + cnt {
                    if let Some((t, u, v)) = tri_hit(&self.hot[k as usize], o, d, tmin, tmax) {
                        tmax = t;
                        best = Some(Hit { t, tri: k, u, v });
                    }
                }
                continue;
            }
            let n = &self.nodes4[kid as usize];
            let (near, hit) = slab4(n, org, inv, tmin, tmax);
            // Push hit children far-to-near so the nearest pops first.
            let mut order = [0usize, 1, 2, 3];
            order.sort_unstable_by(|a, b| near[*b].total_cmp(&near[*a]));
            for &l in &order {
                if hit[l] && sp < stack.len() {
                    stack[sp] = (n.kid[l], n.cnt[l], near[l]);
                    sp += 1;
                }
            }
        }
        best
    }

    /// Whether anything is hit along the segment (shadow and AO rays).
    #[inline]
    pub fn occluded(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> bool {
        if self.nodes4.is_empty() {
            return false;
        }
        let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
        let org = [o.x, o.y, o.z];
        let mut stack = [(0u32, 0u32); 96];
        let mut sp = 1usize;
        while sp > 0 {
            sp -= 1;
            let (kid, cnt) = stack[sp];
            if cnt > 0 {
                for k in kid..kid + cnt {
                    if tri_hit(&self.hot[k as usize], o, d, tmin, tmax).is_some() {
                        return true;
                    }
                }
                continue;
            }
            let n = &self.nodes4[kid as usize];
            let (_, hit) = slab4(n, org, inv, tmin, tmax);
            for l in 0..4 {
                if hit[l] && sp < stack.len() {
                    stack[sp] = (n.kid[l], n.cnt[l]);
                    sp += 1;
                }
            }
        }
        false
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
                    if let Some((t, _, _)) = tri_hit(&self.hot[k as usize], o, d, tmin, tmax) {
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

/// Four slab tests at once: per-lane entry distance and hit mask.
#[inline(always)]
fn slab4(n: &Node4, o: [f32; 3], inv: [f32; 3], tmin: f32, tmax: f32) -> ([f32; 4], [bool; 4]) {
    let mut a = [tmin; 4];
    let mut b = [tmax; 4];
    for ax in 0..3 {
        for l in 0..4 {
            let t1 = (n.lo[ax][l] - o[ax]) * inv[ax];
            let t2 = (n.hi[ax][l] - o[ax]) * inv[ax];
            a[l] = fmax(a[l], fmin(t1, t2));
            b[l] = fmin(b[l], fmax(t1, t2));
        }
    }
    let mut hit = [false; 4];
    for l in 0..4 {
        hit[l] = a[l] <= b[l] && n.kid[l] != u32::MAX;
    }
    (a, hit)
}

/// Collapse a binary BVH into a 4-wide one: each node adopts up to four
/// descendants, opening the largest inner child first.
fn collapse(nodes: &[Node]) -> Vec<Node4> {
    let area = |n: &Node| {
        let d = sub(n.hi, n.lo);
        d.x * d.y + d.y * d.z + d.z * d.x
    };
    let mut out: Vec<Node4> = Vec::new();
    // (binary node index, slot in `out`)
    let mut todo = vec![(0usize, 0usize)];
    out.push(Node4 { lo: [[0.0; 4]; 3], hi: [[0.0; 4]; 3], kid: [u32::MAX; 4], cnt: [0; 4] });
    while let Some((bi, slot)) = todo.pop() {
        let root = &nodes[bi];
        let mut kids: Vec<usize> = if root.count > 0 { vec![bi] } else { vec![root.first as usize, root.first as usize + 1] };
        while kids.len() < 4 {
            // Open the largest inner child.
            let pick = kids.iter().enumerate().filter(|(_, &k)| nodes[k].count == 0).max_by(|a, b| area(&nodes[*a.1]).total_cmp(&area(&nodes[*b.1])));
            let Some((i, &k)) = pick else { break };
            kids.remove(i);
            kids.push(nodes[k].first as usize);
            kids.push(nodes[k].first as usize + 1);
        }
        let mut n4 = Node4 { lo: [[f32::MAX; 4]; 3], hi: [[f32::MIN; 4]; 3], kid: [u32::MAX; 4], cnt: [0; 4] };
        for (l, &k) in kids.iter().enumerate() {
            let c = &nodes[k];
            for ax in 0..3 {
                n4.lo[ax][l] = get(c.lo, ax);
                n4.hi[ax][l] = get(c.hi, ax);
            }
            if c.count > 0 {
                n4.kid[l] = c.first;
                n4.cnt[l] = c.count;
            } else {
                let s = out.len();
                out.push(Node4 { lo: [[0.0; 4]; 3], hi: [[0.0; 4]; 3], kid: [u32::MAX; 4], cnt: [0; 4] });
                n4.kid[l] = s as u32;
                n4.cnt[l] = 0;
                todo.push((k, s));
            }
        }
        out[slot] = n4;
    }
    out
}

/// Top-level hierarchy over world-space boxes (one per object instance),
/// rebuilt every frame (cheap: median splits over a few thousand boxes).
pub struct Tlas {
    nodes4: Vec<Node4>,
    pub items: Vec<u32>,
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
        let nodes4 = if n == 0 { Vec::new() } else { collapse(&nodes) };
        Tlas { nodes4, items }
    }

    /// Visit items whose boxes the ray enters before `tmax` (nearest first);
    /// `f(item, tmax) -> new tmax` (return a smaller value on a hit, or
    /// `-1.0` to stop early for shadow rays).
    #[inline]
    pub fn visit(&self, o: V3, d: V3, tmin: f32, mut tmax: f32, mut f: impl FnMut(u32, f32) -> f32) {
        if self.nodes4.is_empty() {
            return;
        }
        let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
        let org = [o.x, o.y, o.z];
        let mut stack = [(0u32, 0u32, 0.0f32); 96];
        stack[0] = (0, 0, tmin);
        let mut sp = 1usize;
        while sp > 0 {
            sp -= 1;
            let (kid, cnt, tn) = stack[sp];
            if tn > tmax {
                continue;
            }
            if cnt > 0 {
                for k in kid..kid + cnt {
                    tmax = f(self.items[k as usize], tmax);
                    if tmax < 0.0 {
                        return;
                    }
                }
                continue;
            }
            let n = &self.nodes4[kid as usize];
            let (near, hit) = slab4(n, org, inv, tmin, tmax);
            let mut order = [0usize, 1, 2, 3];
            order.sort_unstable_by(|a, b| near[*b].total_cmp(&near[*a]));
            for &l in &order {
                if hit[l] && sp < stack.len() {
                    stack[sp] = (n.kid[l], n.cnt[l], near[l]);
                    sp += 1;
                }
            }
        }
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
fn tri_hit(tr: &Hot, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<(f32, f32, f32)> {
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
            for t in &b.hot {
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
}
