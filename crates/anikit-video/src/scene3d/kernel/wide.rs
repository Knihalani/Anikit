//! 8-wide bounding-volume hierarchies and their traversal.
//!
//! A binary SAH tree is collapsed into nodes of up to eight children whose
//! boxes are stored lane-wise (`[f32; 8]` per bound), so one node visit is
//! six 8-lane slab tests: one AVX2 instruction each (two SSE ones without
//! AVX2). Hit children are pushed far-to-near onto a fixed, uninitialised
//! stack, so the nearest pops first and the far ones are culled by the
//! shrinking `tmax`. Triangles live in blocks of four (structure of arrays)
//! and are tested four at a time.
//!
//! Traversal is written once ([`traverse`], `#[inline(always)]`) and compiled
//! twice by its callers: in an `#[target_feature(enable = "avx2,fma")]`
//! wrapper, picked at run time by [`avx2`], and for the baseline. The
//! arithmetic is the same IEEE operations in the same order either way (no
//! fused multiply-adds are formed), so both paths give identical results.

use super::super::math::*;
use std::mem::MaybeUninit;

/// Whether the CPU has AVX2 + FMA (checked once).
#[inline]
pub fn avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        use std::sync::atomic::{AtomicU8, Ordering};
        static F: AtomicU8 = AtomicU8::new(0);
        match F.load(Ordering::Relaxed) {
            1 => false,
            2 => true,
            _ => {
                let on = std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") && std::env::var_os("ANIKIT_NO_AVX2").is_none();
                F.store(if on { 2 } else { 1 }, Ordering::Relaxed);
                on
            }
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// A binary node: bounds plus either two children (`count == 0`, left child
/// at `first`, right at `first + 1`) or a run of `count` items.
#[derive(Clone, Copy, Debug)]
pub struct BNode {
    pub lo: V3,
    pub hi: V3,
    pub first: u32,
    pub count: u32,
}

/// Eight children, bounds lane-wise. Lane `l` is used when bit `l` of
/// `valid` is set; it is the node `kid[l]` when `cnt[l] == 0`, otherwise a
/// leaf of `cnt[l]` items (triangle blocks or instances) from `kid[l]`.
#[derive(Clone, Copy)]
#[repr(C, align(32))]
pub struct Node8 {
    pub lo: [[f32; 8]; 3],
    pub hi: [[f32; 8]; 3],
    pub kid: [u32; 8],
    pub cnt: [u32; 8],
    pub valid: u32,
}

impl Node8 {
    fn empty() -> Node8 {
        Node8 { lo: [[0.0; 8]; 3], hi: [[0.0; 8]; 3], kid: [0; 8], cnt: [0; 8], valid: 0 }
    }
}

fn area(n: &BNode) -> f32 {
    let d = sub(n.hi, n.lo);
    d.x * d.y + d.y * d.z + d.z * d.x
}

/// Collapse a binary tree into an 8-wide one: each node adopts up to eight
/// descendants, opening the largest inner child first. `leaf(first, count)`
/// maps a binary leaf's item range to the wide leaf's `(kid, cnt)`.
pub fn collapse(nodes: &[BNode], mut leaf: impl FnMut(u32, u32) -> (u32, u32)) -> Vec<Node8> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Node8> = vec![Node8::empty()];
    let mut todo = vec![(0usize, 0usize)];
    while let Some((bi, slot)) = todo.pop() {
        let root = &nodes[bi];
        let mut kids: Vec<usize> = if root.count > 0 { vec![bi] } else { vec![root.first as usize, root.first as usize + 1] };
        while kids.len() < 8 {
            let pick = kids.iter().enumerate().filter(|(_, &k)| nodes[k].count == 0).max_by(|a, b| area(&nodes[*a.1]).total_cmp(&area(&nodes[*b.1])));
            let Some((i, &k)) = pick else { break };
            kids.remove(i);
            kids.push(nodes[k].first as usize);
            kids.push(nodes[k].first as usize + 1);
        }
        let mut n8 = Node8::empty();
        for (l, &k) in kids.iter().enumerate() {
            let c = &nodes[k];
            for ax in 0..3 {
                n8.lo[ax][l] = get(c.lo, ax);
                n8.hi[ax][l] = get(c.hi, ax);
            }
            n8.valid |= 1 << l;
            if c.count > 0 {
                let (kid, cnt) = leaf(c.first, c.count);
                n8.kid[l] = kid;
                n8.cnt[l] = cnt;
            } else {
                let s = out.len();
                out.push(Node8::empty());
                n8.kid[l] = s as u32;
                n8.cnt[l] = 0;
                todo.push((k, s));
            }
        }
        out[slot] = n8;
    }
    out
}

/// NaN-oblivious min/max (compare + select, vectorises to minps/maxps).
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

/// Eight slab tests: per-lane entry distance and the hit mask (portable).
#[inline(always)]
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
fn slab8_scalar(n: &Node8, o: &[f32; 3], inv: &[f32; 3], tmin: f32, tmax: f32) -> ([f32; 8], u32) {
    let mut a = [tmin; 8];
    let mut b = [tmax; 8];
    for ax in 0..3 {
        let (lo, hi) = (&n.lo[ax], &n.hi[ax]);
        for l in 0..8 {
            let t1 = (lo[l] - o[ax]) * inv[ax];
            let t2 = (hi[l] - o[ax]) * inv[ax];
            a[l] = fmax(a[l], fmin(t1, t2));
            b[l] = fmin(b[l], fmax(t1, t2));
        }
    }
    let mut m = 0u32;
    for l in 0..8 {
        m |= ((a[l] <= b[l]) as u32) << l;
    }
    (a, m & n.valid)
}

// `min_ps(a, b)` is `a < b ? a : b` and `max_ps(a, b)` is `a > b ? a : b`:
// exactly `fmin`/`fmax` above, so every path computes the same bits.

/// [`slab8_scalar`] with SSE2 (two 4-lane halves).
#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn slab8_sse(n: &Node8, o: &[f32; 3], inv: &[f32; 3], tmin: f32, tmax: f32) -> ([f32; 8], u32) {
    use std::arch::x86_64::*;
    // SAFETY: SSE2 is part of the x86_64 baseline; `Node8` is 32-byte aligned
    // so every `[f32; 4]` half is 16-byte aligned.
    unsafe {
        let mut near = [0.0f32; 8];
        let mut m = 0u32;
        for h in 0..2 {
            let mut a = _mm_set1_ps(tmin);
            let mut b = _mm_set1_ps(tmax);
            for ax in 0..3 {
                let (oo, ii) = (_mm_set1_ps(o[ax]), _mm_set1_ps(inv[ax]));
                let lo = _mm_load_ps(n.lo[ax].as_ptr().add(h * 4));
                let hi = _mm_load_ps(n.hi[ax].as_ptr().add(h * 4));
                let t1 = _mm_mul_ps(_mm_sub_ps(lo, oo), ii);
                let t2 = _mm_mul_ps(_mm_sub_ps(hi, oo), ii);
                a = _mm_max_ps(a, _mm_min_ps(t1, t2));
                b = _mm_min_ps(b, _mm_max_ps(t1, t2));
            }
            _mm_storeu_ps(near.as_mut_ptr().add(h * 4), a);
            m |= (_mm_movemask_ps(_mm_cmple_ps(a, b)) as u32) << (h * 4);
        }
        (near, m & n.valid)
    }
}

/// [`slab8_scalar`] with AVX: one instruction per step for all eight lanes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[inline]
unsafe fn slab8_avx2(n: &Node8, o: &[f32; 3], inv: &[f32; 3], tmin: f32, tmax: f32) -> ([f32; 8], u32) {
    use std::arch::x86_64::*;
    // SAFETY: the caller has AVX2; `Node8` bounds are 32-byte aligned.
    unsafe {
        let mut a = _mm256_set1_ps(tmin);
        let mut b = _mm256_set1_ps(tmax);
        for ax in 0..3 {
            let (oo, ii) = (_mm256_set1_ps(o[ax]), _mm256_set1_ps(inv[ax]));
            let lo = _mm256_load_ps(n.lo[ax].as_ptr());
            let hi = _mm256_load_ps(n.hi[ax].as_ptr());
            let t1 = _mm256_mul_ps(_mm256_sub_ps(lo, oo), ii);
            let t2 = _mm256_mul_ps(_mm256_sub_ps(hi, oo), ii);
            a = _mm256_max_ps(a, _mm256_min_ps(t1, t2));
            b = _mm256_min_ps(b, _mm256_max_ps(t1, t2));
        }
        let mut near = [0.0f32; 8];
        _mm256_storeu_ps(near.as_mut_ptr(), a);
        let m = _mm256_movemask_ps(_mm256_cmp_ps::<_CMP_LE_OQ>(a, b)) as u32;
        (near, m & n.valid)
    }
}

#[inline(always)]
fn slab8<const AVX: bool>(n: &Node8, o: &[f32; 3], inv: &[f32; 3], tmin: f32, tmax: f32) -> ([f32; 8], u32) {
    #[cfg(target_arch = "x86_64")]
    {
        if AVX {
            // SAFETY: `AVX` traversals only run inside AVX2 functions.
            unsafe { slab8_avx2(n, o, inv, tmin, tmax) }
        } else {
            slab8_sse(n, o, inv, tmin, tmax)
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        slab8_scalar(n, o, inv, tmin, tmax)
    }
}

const STACK: usize = 192;

/// Visit the leaves whose boxes the ray enters before `tmax`, nearest
/// first: `leaf(kid, cnt, tmax) -> tmax` returns the new `tmax` (smaller
/// after a hit, negative to stop).
#[inline(always)]
pub fn traverse<const AVX: bool>(nodes: &[Node8], o: V3, d: V3, tmin: f32, mut tmax: f32, mut leaf: impl FnMut(u32, u32, f32) -> f32) {
    if nodes.is_empty() {
        return;
    }
    let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
    let org = [o.x, o.y, o.z];
    // (child, count, entry distance); uninitialised: only pushed entries are read.
    let mut stack: [MaybeUninit<(u32, u32, f32)>; STACK] = [const { MaybeUninit::uninit() }; STACK];
    let mut sp = 0usize;
    // The entry being visited: descend straight into the nearest hit child
    // (no stack round trip); the others wait on the stack, far-to-near.
    let mut cur = (0u32, 0u32, tmin);
    'walk: loop {
        let (kid, cnt, tn) = cur;
        'visit: {
            if tn > tmax {
                break 'visit;
            }
            if cnt > 0 {
                tmax = leaf(kid, cnt, tmax);
                if tmax < 0.0 {
                    return;
                }
                break 'visit;
            }
            // SAFETY: inner-node indices come from `collapse`.
            let n = unsafe { nodes.get_unchecked(kid as usize) };
            let (near, mut m) = slab8::<AVX>(n, &org, &inv, tmin, tmax);
            if m == 0 {
                break 'visit;
            }
            let l0 = m.trailing_zeros() as usize;
            m &= m - 1;
            if m == 0 {
                cur = (n.kid[l0], n.cnt[l0], near[l0]);
                continue 'walk;
            }
            let l1 = m.trailing_zeros() as usize;
            m &= m - 1;
            if m == 0 {
                // Two hits: push the farther, visit the nearer.
                let (a, b) = if near[l1] < near[l0] { (l1, l0) } else { (l0, l1) };
                if sp < STACK {
                    stack[sp].write((n.kid[b], n.cnt[b], near[b]));
                    sp += 1;
                }
                cur = (n.kid[a], n.cnt[a], near[a]);
                continue 'walk;
            }
            // Three or more: insertion-sort far-to-near, push all but the nearest.
            let mut ord: [(f32, u32); 8] = [(0.0, 0); 8];
            ord[0] = (near[l0], l0 as u32);
            let mut k = 1usize;
            let add = |l: usize, ord: &mut [(f32, u32); 8], k: &mut usize| {
                let t = near[l];
                let mut j = *k;
                while j > 0 && ord[j - 1].0 < t {
                    ord[j] = ord[j - 1];
                    j -= 1;
                }
                ord[j] = (t, l as u32);
                *k += 1;
            };
            add(l1, &mut ord, &mut k);
            while m != 0 {
                let l = m.trailing_zeros() as usize;
                m &= m - 1;
                add(l, &mut ord, &mut k);
            }
            for &(t, l) in &ord[..k - 1] {
                if sp < STACK {
                    stack[sp].write((n.kid[l as usize], n.cnt[l as usize], t));
                    sp += 1;
                }
            }
            let (t, l) = ord[k - 1];
            cur = (n.kid[l as usize], n.cnt[l as usize], t);
            continue 'walk;
        }
        if sp == 0 {
            return;
        }
        sp -= 1;
        // SAFETY: entries below the old `sp` were written.
        cur = unsafe { stack[sp].assume_init() };
    }
}

/// Four triangles, structure of arrays (vertex 0 and the two edges).
#[derive(Clone, Copy)]
#[repr(C, align(16))]
pub struct Tri4 {
    pub v0: [[f32; 4]; 3],
    pub e1: [[f32; 4]; 3],
    pub e2: [[f32; 4]; 3],
}

impl Tri4 {
    /// Four degenerate triangles (never hit).
    pub fn empty() -> Tri4 {
        Tri4 { v0: [[0.0; 4]; 3], e1: [[0.0; 4]; 3], e2: [[0.0; 4]; 3] }
    }

    pub fn set(&mut self, l: usize, v0: V3, e1: V3, e2: V3) {
        for ax in 0..3 {
            self.v0[ax][l] = get(v0, ax);
            self.e1[ax][l] = get(e1, ax);
            self.e2[ax][l] = get(e2, ax);
        }
    }

    /// Möller–Trumbore on four lanes: per-lane `(t, u, v)` and the hit mask
    /// (`t` in `(tmin, tmax)`).
    #[inline(always)]
    pub fn hit(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> ([f32; 4], [f32; 4], [f32; 4], u32) {
        #[cfg(target_arch = "x86_64")]
        {
            self.hit_sse(o, d, tmin, tmax)
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            self.hit_scalar(o, d, tmin, tmax)
        }
    }

    /// The same Möller–Trumbore, one SSE op per step for the four lanes
    /// (same operations in the same order as [`Tri4::hit_scalar`]).
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn hit_sse(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> ([f32; 4], [f32; 4], [f32; 4], u32) {
        use std::arch::x86_64::*;
        // SAFETY: SSE2 is baseline on x86_64; `Tri4` rows are 16-byte aligned.
        unsafe {
            let ld = |r: &[f32; 4]| _mm_load_ps(r.as_ptr());
            let (e1x, e1y, e1z) = (ld(&self.e1[0]), ld(&self.e1[1]), ld(&self.e1[2]));
            let (e2x, e2y, e2z) = (ld(&self.e2[0]), ld(&self.e2[1]), ld(&self.e2[2]));
            let (dx, dy, dz) = (_mm_set1_ps(d.x), _mm_set1_ps(d.y), _mm_set1_ps(d.z));
            let (mul, sub, add) = (|a, b| _mm_mul_ps(a, b), |a, b| _mm_sub_ps(a, b), |a, b| _mm_add_ps(a, b));
            let px = sub(mul(dy, e2z), mul(dz, e2y));
            let py = sub(mul(dz, e2x), mul(dx, e2z));
            let pz = sub(mul(dx, e2y), mul(dy, e2x));
            let det = add(add(mul(e1x, px), mul(e1y, py)), mul(e1z, pz));
            let id = _mm_div_ps(_mm_set1_ps(1.0), det);
            let sx = sub(_mm_set1_ps(o.x), ld(&self.v0[0]));
            let sy = sub(_mm_set1_ps(o.y), ld(&self.v0[1]));
            let sz = sub(_mm_set1_ps(o.z), ld(&self.v0[2]));
            let u = mul(add(add(mul(sx, px), mul(sy, py)), mul(sz, pz)), id);
            let qx = sub(mul(sy, e1z), mul(sz, e1y));
            let qy = sub(mul(sz, e1x), mul(sx, e1z));
            let qz = sub(mul(sx, e1y), mul(sy, e1x));
            let v = mul(add(add(mul(dx, qx), mul(dy, qy)), mul(dz, qz)), id);
            let t = mul(add(add(mul(e2x, qx), mul(e2y, qy)), mul(e2z, qz)), id);
            let adet = _mm_andnot_ps(_mm_set1_ps(-0.0), det);
            let zero = _mm_setzero_ps();
            let one = _mm_set1_ps(1.0);
            let mut ok = _mm_cmpge_ps(adet, _mm_set1_ps(1e-12));
            ok = _mm_and_ps(ok, _mm_cmpge_ps(u, zero));
            ok = _mm_and_ps(ok, _mm_cmple_ps(u, one));
            ok = _mm_and_ps(ok, _mm_cmpge_ps(v, zero));
            ok = _mm_and_ps(ok, _mm_cmple_ps(add(u, v), one));
            ok = _mm_and_ps(ok, _mm_cmpgt_ps(t, _mm_set1_ps(tmin)));
            ok = _mm_and_ps(ok, _mm_cmplt_ps(t, _mm_set1_ps(tmax)));
            let m = _mm_movemask_ps(ok) as u32;
            if m == 0 {
                return ([0.0; 4], [0.0; 4], [0.0; 4], 0);
            }
            let (mut ts, mut us, mut vs) = ([0.0f32; 4], [0.0f32; 4], [0.0f32; 4]);
            _mm_storeu_ps(ts.as_mut_ptr(), t);
            _mm_storeu_ps(us.as_mut_ptr(), u);
            _mm_storeu_ps(vs.as_mut_ptr(), v);
            (ts, us, vs, m)
        }
    }

    /// Möller–Trumbore on four lanes: per-lane `(t, u, v)` and the hit mask
    /// (`t` in `(tmin, tmax)`).
    #[inline(always)]
    #[cfg_attr(target_arch = "x86_64", allow(dead_code))]
    fn hit_scalar(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> ([f32; 4], [f32; 4], [f32; 4], u32) {
        let (e1, e2, v0) = (&self.e1, &self.e2, &self.v0);
        let (mut ts, mut us, mut vs) = ([0.0f32; 4], [0.0f32; 4], [0.0f32; 4]);
        let mut m = 0u32;
        for l in 0..4 {
            let px = d.y * e2[2][l] - d.z * e2[1][l];
            let py = d.z * e2[0][l] - d.x * e2[2][l];
            let pz = d.x * e2[1][l] - d.y * e2[0][l];
            let det = e1[0][l] * px + e1[1][l] * py + e1[2][l] * pz;
            let id = 1.0 / det;
            let (sx, sy, sz) = (o.x - v0[0][l], o.y - v0[1][l], o.z - v0[2][l]);
            let u = (sx * px + sy * py + sz * pz) * id;
            let qx = sy * e1[2][l] - sz * e1[1][l];
            let qy = sz * e1[0][l] - sx * e1[2][l];
            let qz = sx * e1[1][l] - sy * e1[0][l];
            let v = (d.x * qx + d.y * qy + d.z * qz) * id;
            let t = (e2[0][l] * qx + e2[1][l] * qy + e2[2][l] * qz) * id;
            let ok = (det.abs() >= 1e-12) & (0.0..=1.0).contains(&u) & (v >= 0.0) & (u + v <= 1.0) & (t > tmin) & (t < tmax);
            ts[l] = t;
            us[l] = u;
            vs[l] = v;
            m |= (ok as u32) << l;
        }
        (ts, us, vs, m)
    }
}

/// A triangle mesh hierarchy: 8-wide nodes over blocks of four triangles.
pub struct Mesh8 {
    pub nodes: Vec<Node8>,
    pub blocks: Vec<Tri4>,
    /// The triangle (index into the caller's list) in each block lane.
    pub ids: Vec<[u32; 4]>,
}

/// Closest hit: distance, triangle id, barycentrics.
#[derive(Clone, Copy, Debug)]
pub struct TriHit {
    pub t: f32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
}

/// How many nearest hits [`Near`] keeps.
pub const NEAR_K: usize = 4;

/// The nearest few hits along a ray, ascending by distance; hits at exactly
/// the same distance are kept once (a triangle referenced by two leaves, or
/// two triangles meeting at an edge, where a sequential trace sees one).
#[derive(Clone, Copy)]
pub struct Near<T: Copy> {
    pub n: usize,
    pub h: [(f32, T); NEAR_K],
}

impl<T: Copy> Near<T> {
    pub fn new(fill: T) -> Near<T> {
        Near { n: 0, h: [(f32::MAX, fill); NEAR_K] }
    }
    /// Hits beyond this can't enter the list.
    #[inline(always)]
    pub fn far(&self, tmax: f32) -> f32 {
        if self.n == NEAR_K {
            self.h[NEAR_K - 1].0.min(tmax)
        } else {
            tmax
        }
    }
    #[inline]
    pub fn insert(&mut self, t: f32, x: T) {
        if self.n == NEAR_K && t >= self.h[NEAR_K - 1].0 {
            return;
        }
        let mut i = self.n.min(NEAR_K - 1);
        // Find the slot: shift farther hits right.
        let mut pos = self.n;
        for (j, e) in self.h[..self.n].iter().enumerate() {
            if e.0 == t {
                return;
            }
            if e.0 > t {
                pos = j;
                break;
            }
        }
        if pos >= NEAR_K {
            return;
        }
        while i > pos {
            self.h[i] = self.h[i - 1];
            i -= 1;
        }
        self.h[pos] = (t, x);
        self.n = (self.n + 1).min(NEAR_K);
    }
}

impl Mesh8 {
    /// From a binary tree over `tris` (vertex, edge 1, edge 2), whose leaves
    /// index runs of `tris` (already in leaf order).
    pub fn new(nodes: &[BNode], tris: &[(V3, V3, V3)]) -> Mesh8 {
        let mut blocks = Vec::new();
        let mut ids = Vec::new();
        let nodes8 = collapse(nodes, |first, count| {
            let b0 = blocks.len() as u32;
            for c in (first..first + count).step_by(4) {
                let mut b = Tri4::empty();
                let mut id = [u32::MAX; 4];
                for l in 0..4 {
                    let k = c + l as u32;
                    if k < first + count {
                        let (v0, e1, e2) = tris[k as usize];
                        b.set(l, v0, e1, e2);
                        id[l] = k;
                    }
                }
                blocks.push(b);
                ids.push(id);
            }
            (b0, blocks.len() as u32 - b0)
        });
        Mesh8 { nodes: nodes8, blocks, ids }
    }

    #[inline(always)]
    fn closest_impl<const AVX: bool>(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<TriHit> {
        let mut best: Option<TriHit> = None;
        traverse::<AVX>(&self.nodes, o, d, tmin, tmax, |b0, nb, mut tm| {
            for b in b0..b0 + nb {
                // SAFETY: leaf ranges come from `new`.
                let blk = unsafe { self.blocks.get_unchecked(b as usize) };
                let (ts, us, vs, mut m) = blk.hit(o, d, tmin, tm);
                // In lane order, keep strictly nearer hits (as a sequential loop would).
                while m != 0 {
                    let l = m.trailing_zeros() as usize;
                    m &= m - 1;
                    if ts[l] < tm {
                        tm = ts[l];
                        best = Some(TriHit { t: ts[l], tri: self.ids[b as usize][l], u: us[l], v: vs[l] });
                    }
                }
            }
            tm
        });
        best
    }

    #[inline(always)]
    fn occluded_impl<const AVX: bool>(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> bool {
        let mut hit = false;
        traverse::<AVX>(&self.nodes, o, d, tmin, tmax, |b0, nb, tm| {
            for b in b0..b0 + nb {
                // SAFETY: leaf ranges come from `new`.
                let blk = unsafe { self.blocks.get_unchecked(b as usize) };
                if blk.hit(o, d, tmin, tmax).3 != 0 {
                    hit = true;
                    return -1.0;
                }
            }
            tm
        });
        hit
    }

    /// Closest hit along `o + t·d`, `t` in `(tmin, tmax)`.
    #[inline(never)]
    pub fn closest(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<TriHit> {
        #[cfg(target_arch = "x86_64")]
        if avx2() {
            // SAFETY: the CPU has AVX2 and FMA.
            return unsafe { self.closest_avx2(o, d, tmin, tmax) };
        }
        self.closest_impl::<false>(o, d, tmin, tmax)
    }

    /// Whether anything is hit along the segment.
    #[inline(never)]
    pub fn occluded(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> bool {
        #[cfg(target_arch = "x86_64")]
        if avx2() {
            // SAFETY: the CPU has AVX2 and FMA.
            return unsafe { self.occluded_avx2(o, d, tmin, tmax) };
        }
        self.occluded_impl::<false>(o, d, tmin, tmax)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn closest_avx2(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> Option<TriHit> {
        self.closest_impl::<true>(o, d, tmin, tmax)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn occluded_avx2(&self, o: V3, d: V3, tmin: f32, tmax: f32) -> bool {
        self.occluded_impl::<true>(o, d, tmin, tmax)
    }

    #[inline(always)]
    fn nearest_impl<const AVX: bool>(&self, o: V3, d: V3, tmin: f32, tmax: f32, near: &mut Near<TriHit>) {
        traverse::<AVX>(&self.nodes, o, d, tmin, near.far(tmax), |b0, nb, _| {
            for b in b0..b0 + nb {
                // SAFETY: leaf ranges come from `new`.
                let blk = unsafe { self.blocks.get_unchecked(b as usize) };
                let (ts, us, vs, mut m) = blk.hit(o, d, tmin, near.far(tmax));
                while m != 0 {
                    let l = m.trailing_zeros() as usize;
                    m &= m - 1;
                    near.insert(ts[l], TriHit { t: ts[l], tri: self.ids[b as usize][l], u: us[l], v: vs[l] });
                }
            }
            near.far(tmax)
        });
    }

    /// Merge the nearest [`NEAR_K`] hits in `(tmin, tmax)` into `near`
    /// (each with the same distance and barycentrics [`Mesh8::closest`] gives).
    #[inline(never)]
    pub fn nearest(&self, o: V3, d: V3, tmin: f32, tmax: f32, near: &mut Near<TriHit>) {
        #[cfg(target_arch = "x86_64")]
        if avx2() {
            // SAFETY: the CPU has AVX2 and FMA.
            return unsafe { self.nearest_avx2(o, d, tmin, tmax, near) };
        }
        self.nearest_impl::<false>(o, d, tmin, tmax, near)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn nearest_avx2(&self, o: V3, d: V3, tmin: f32, tmax: f32, near: &mut Near<TriHit>) {
        self.nearest_impl::<true>(o, d, tmin, tmax, near)
    }
}

/// An 8-wide hierarchy over boxes (instances); leaves index `items`.
pub struct Boxes8 {
    pub nodes: Vec<Node8>,
    pub items: Vec<u32>,
}

impl Boxes8 {
    /// From a binary tree whose leaves index runs of `items`.
    pub fn new(nodes: &[BNode], items: Vec<u32>) -> Boxes8 {
        Boxes8 { nodes: collapse(nodes, |f, c| (f, c)), items }
    }

    #[inline(always)]
    fn visit_impl<const AVX: bool>(&self, o: V3, d: V3, tmin: f32, tmax: f32, mut f: impl FnMut(u32, f32) -> f32) {
        traverse::<AVX>(&self.nodes, o, d, tmin, tmax, |first, n, mut tm| {
            for k in first..first + n {
                // SAFETY: leaf ranges come from `new`.
                tm = f(unsafe { *self.items.get_unchecked(k as usize) }, tm);
                if tm < 0.0 {
                    return tm;
                }
            }
            tm
        });
    }

    /// Visit items whose boxes the ray enters before `tmax` (nearest first);
    /// `f(item, tmax) -> new tmax` (smaller on a hit, `-1.0` to stop).
    #[inline]
    pub fn visit(&self, o: V3, d: V3, tmin: f32, tmax: f32, f: impl FnMut(u32, f32) -> f32) {
        #[cfg(target_arch = "x86_64")]
        if avx2() {
            // SAFETY: the CPU has AVX2 and FMA.
            return unsafe { self.visit_avx2(o, d, tmin, tmax, f) };
        }
        self.visit_impl::<false>(o, d, tmin, tmax, f)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn visit_avx2(&self, o: V3, d: V3, tmin: f32, tmax: f32, f: impl FnMut(u32, f32) -> f32) {
        self.visit_impl::<true>(o, d, tmin, tmax, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(seed: &mut u32) -> f32 {
        *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (*seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }

    /// The SIMD paths compute exactly the portable reference's bits.
    #[test]
    fn simd_paths_match_scalar() {
        let mut s = 7u32;
        for _ in 0..2000 {
            let mut n = Node8::empty();
            for l in 0..8 {
                for ax in 0..3 {
                    let (a, b) = (rng(&mut s) * 3.0, rng(&mut s) * 3.0);
                    n.lo[ax][l] = a.min(b);
                    n.hi[ax][l] = a.max(b);
                }
            }
            n.valid = (s >> 3) & 0xff;
            let o = [rng(&mut s) * 4.0, rng(&mut s) * 4.0, rng(&mut s) * 4.0];
            let d = norm(v3(rng(&mut s), rng(&mut s), if s & 1 == 0 { 0.0 } else { rng(&mut s) }));
            let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
            let r = slab8_scalar(&n, &o, &inv, 1e-4, 5.0);
            #[cfg(target_arch = "x86_64")]
            {
                let a = slab8_sse(&n, &o, &inv, 1e-4, 5.0);
                assert_eq!(a.1, r.1);
                if avx2() {
                    let b = unsafe { slab8_avx2(&n, &o, &inv, 1e-4, 5.0) };
                    assert_eq!(b.1, r.1);
                }
            }
            let mut t = Tri4::empty();
            for l in 0..3 {
                t.set(l, v3(rng(&mut s), rng(&mut s), rng(&mut s)), v3(rng(&mut s), rng(&mut s), rng(&mut s)), v3(rng(&mut s), rng(&mut s), rng(&mut s)));
            }
            let (oo, dd) = (v3(o[0], o[1], o[2]), d);
            let a = t.hit_scalar(oo, dd, 1e-4, 9.0);
            let b = t.hit(oo, dd, 1e-4, 9.0);
            assert_eq!(a.3, b.3);
            for l in 0..4 {
                if a.3 >> l & 1 == 1 {
                    assert_eq!(a.0[l].to_bits(), b.0[l].to_bits());
                    assert_eq!(a.1[l].to_bits(), b.1[l].to_bits());
                }
            }
        }
    }
}
