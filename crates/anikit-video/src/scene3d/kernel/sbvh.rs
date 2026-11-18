//! Mesh hierarchy build: binned SAH over all three axes with spatial splits
//! (SBVH, Stich et al. 2009).
//!
//! Long thin triangles (extruded text, slabs, bevel strips) have boxes far
//! larger than their surface, so object splits alone leave siblings that
//! overlap and rays visit both. A spatial split cuts the node at a plane
//! and sends a straddling triangle to both sides with its box clipped to
//! each side, so siblings stop overlapping. The same triangle can sit in
//! several leaves; hits are unchanged (the same triangle data is tested).
//! Duplication is capped at [`DUP_BUDGET`] × the triangle count.

use super::super::math::*;
use super::wide::BNode;

/// Bins per axis for object splits.
const BINS: usize = 32;
/// Bins per axis for spatial splits.
const SBINS: usize = 32;
/// Spatial splits are tried when the best object split's children overlap
/// by more than this fraction of the root's surface area.
const ALPHA: f32 = 1e-5;
/// Extra references allowed by spatial splits, as a fraction of the triangles.
const DUP_BUDGET: f32 = 1.0;
/// Leaves hold at most this many triangles (one or two `Tri4` blocks).
const LEAF_MAX: usize = 8;

#[derive(Clone, Copy)]
struct Ref {
    lo: V3,
    hi: V3,
    tri: u32,
}

#[derive(Clone, Copy)]
struct Aabb {
    lo: V3,
    hi: V3,
}

impl Aabb {
    const EMPTY: Aabb = Aabb { lo: V3 { x: f32::MAX, y: f32::MAX, z: f32::MAX }, hi: V3 { x: f32::MIN, y: f32::MIN, z: f32::MIN } };
    #[inline]
    fn grow(&mut self, lo: V3, hi: V3) {
        self.lo = vmin(self.lo, lo);
        self.hi = vmax(self.hi, hi);
    }
    #[inline]
    fn area(&self) -> f32 {
        if self.lo.x > self.hi.x {
            return 0.0;
        }
        let d = sub(self.hi, self.lo);
        d.x * d.y + d.y * d.z + d.z * d.x
    }
    #[inline]
    fn and(&self, o: &Aabb) -> Aabb {
        Aabb { lo: vmax(self.lo, o.lo), hi: vmin(self.hi, o.hi) }
    }
}

enum Split {
    Object { axis: usize, bin: usize, lo: f32, ext: f32 },
    Spatial { axis: usize, pos: f32 },
}

/// The bounds of the parts of triangle `v` on either side of the plane
/// `axis = pos`, each clipped to the reference's box `b`.
fn split_tri(v: &[V3; 3], b: &Aabb, axis: usize, pos: f32) -> (Aabb, Aabb) {
    let (mut l, mut r) = (Aabb::EMPTY, Aabb::EMPTY);
    for i in 0..3 {
        let (p, q) = (v[i], v[(i + 1) % 3]);
        let (a, c) = (get(p, axis), get(q, axis));
        if a <= pos {
            l.grow(p, p);
        }
        if a >= pos {
            r.grow(p, p);
        }
        if (a < pos && c > pos) || (a > pos && c < pos) {
            let t = ((pos - a) / (c - a)).clamp(0.0, 1.0);
            let mut x = add(p, scale(sub(q, p), t));
            set(&mut x, axis, pos);
            l.grow(x, x);
            r.grow(x, x);
        }
    }
    let (mut l, mut r) = (l.and(b), r.and(b));
    // The plane bounds each side exactly.
    let (lh, rl) = (get(l.hi, axis).min(pos), get(r.lo, axis).max(pos));
    set(&mut l.hi, axis, lh);
    set(&mut r.lo, axis, rl);
    (l, r)
}

#[inline]
fn set(v: &mut V3, axis: usize, x: f32) {
    match axis {
        0 => v.x = x,
        1 => v.y = x,
        _ => v.z = x,
    }
}

/// Build over triangles given as three vertices each: binary nodes whose
/// leaves index runs of the returned order (triangle ids, possibly repeated).
/// Large meshes build their subtrees in parallel on the render pool; the
/// result doesn't depend on scheduling.
pub fn build(verts: &[[V3; 3]]) -> (Vec<BNode>, Vec<u32>) {
    let n = verts.len();
    if n == 0 {
        return (vec![BNode { lo: v3(0.0, 0.0, 0.0), hi: v3(0.0, 0.0, 0.0), first: 0, count: 0 }], Vec::new());
    }
    let refs: Vec<Ref> = verts
        .iter()
        .enumerate()
        .map(|(i, v)| Ref { lo: vmin(v[0], vmin(v[1], v[2])), hi: vmax(v[0], vmax(v[1], v[2])), tri: i as u32 })
        .collect();
    let mut root = Aabb::EMPTY;
    for r in &refs {
        root.grow(r.lo, r.hi);
    }
    let cx = Cx { verts, root_area: root.area().max(1e-30) };
    let budget = (n as f32 * DUP_BUDGET) as usize;
    let t = if n >= PAR_MIN && super::par::parallel() { super::par::isolated(|| cx.node(refs, root, 0, budget)) } else { cx.node(refs, root, 0, budget) };
    (t.nodes, t.order)
}

/// Subtrees with at least this many references build their children in parallel.
const PAR_MIN: usize = 4096;

struct Cx<'a> {
    verts: &'a [[V3; 3]],
    root_area: f32,
}

/// A subtree: its root is `nodes[0]`; node and order indices are local.
struct Sub {
    nodes: Vec<BNode>,
    order: Vec<u32>,
}

impl Cx<'_> {
    fn node(&self, refs: Vec<Ref>, bounds: Aabb, depth: u32, budget: usize) -> Sub {
        let count = refs.len();
        let leaf = |refs: &[Ref]| Sub { nodes: vec![BNode { lo: bounds.lo, hi: bounds.hi, first: 0, count: refs.len() as u32 }], order: refs.iter().map(|r| r.tri).collect() };
        if count <= 4 || depth >= 60 {
            return leaf(&refs);
        }
        let (cost, split, overlap) = object_split(&refs);
        let mut best = (cost, split);
        if overlap / self.root_area > ALPHA && budget > 0 {
            if let Some((c, s)) = spatial_split(&refs, &bounds, self.verts) {
                if c < best.0 {
                    best = (c, Some(s));
                }
            }
        }
        let leaf_cost = count as f32 * bounds.area();
        let Some(split) = best.1 else { return leaf(&refs) };
        if best.0 >= leaf_cost && count <= LEAF_MAX {
            return leaf(&refs);
        }
        let (mut l, mut r) = (Vec::with_capacity(count / 2 + 4), Vec::with_capacity(count / 2 + 4));
        match split {
            Split::Object { axis, bin, lo, ext } => {
                for x in refs {
                    if obj_bin(&x, axis, lo, ext) < bin {
                        l.push(x)
                    } else {
                        r.push(x)
                    }
                }
            }
            Split::Spatial { axis, pos } => {
                for x in refs {
                    if get(x.hi, axis) <= pos {
                        l.push(x);
                    } else if get(x.lo, axis) >= pos {
                        r.push(x);
                    } else {
                        let (a, b) = split_tri(&self.verts[x.tri as usize], &Aabb { lo: x.lo, hi: x.hi }, axis, pos);
                        let ok = |b: &Aabb| b.lo.x <= b.hi.x && b.lo.y <= b.hi.y && b.lo.z <= b.hi.z;
                        match (ok(&a), ok(&b)) {
                            (true, true) => {
                                l.push(Ref { lo: a.lo, hi: a.hi, tri: x.tri });
                                r.push(Ref { lo: b.lo, hi: b.hi, tri: x.tri });
                            }
                            (true, false) => l.push(Ref { lo: a.lo, hi: a.hi, tri: x.tri }),
                            _ => r.push(Ref { lo: b.lo, hi: b.hi, tri: x.tri }),
                        }
                    }
                }
            }
        }
        let budget = budget.saturating_sub((l.len() + r.len()).saturating_sub(count));
        if l.is_empty() || r.is_empty() {
            // Degenerate partition: split the list in half.
            let mut all = l;
            all.append(&mut r);
            let m = all.len() / 2;
            r = all.split_off(m);
            l = all;
        }
        let bb = |v: &[Ref]| {
            let mut b = Aabb::EMPTY;
            for x in v {
                b.grow(x.lo, x.hi);
            }
            b
        };
        let (lb, rb) = (bb(&l), bb(&r));
        // The remaining duplication budget, shared in proportion to size.
        let bl = (budget as u64 * l.len() as u64 / (l.len() + r.len()) as u64) as usize;
        let br = budget - bl;
        let (ls, rs) = if l.len() + r.len() >= PAR_MIN && super::par::parallel() {
            rayon::join(|| self.node(l, lb, depth + 1, bl), || self.node(r, rb, depth + 1, br))
        } else {
            (self.node(l, lb, depth + 1, bl), self.node(r, rb, depth + 1, br))
        };
        // Layout: [root, left root, right root, rest of left, rest of right].
        let mut nodes = Vec::with_capacity(ls.nodes.len() + rs.nodes.len() + 1);
        nodes.push(BNode { lo: bounds.lo, hi: bounds.hi, first: 1, count: 0 });
        let (lrest, rrest) = (3, 3 + ls.nodes.len() - 1);
        let lo_ord = ls.order.len() as u32;
        let place = |sub: &Sub, rest: usize, ord: u32, k: usize| {
            let mut n = sub.nodes[k];
            if n.count > 0 {
                n.first += ord;
            } else {
                // Children `c` and `c + 1` of a subtree land at `rest + c - 1` (c ≥ 1).
                n.first = (rest + n.first as usize - 1) as u32;
            }
            n
        };
        nodes.push(place(&ls, lrest, 0, 0));
        nodes.push(place(&rs, rrest, lo_ord, 0));
        nodes.extend((1..ls.nodes.len()).map(|k| place(&ls, lrest, 0, k)));
        nodes.extend((1..rs.nodes.len()).map(|k| place(&rs, rrest, lo_ord, k)));
        let mut order = ls.order;
        order.extend_from_slice(&rs.order);
        Sub { nodes, order }
    }
}

#[inline]
fn obj_bin(r: &Ref, axis: usize, lo: f32, ext: f32) -> usize {
    let c = (get(r.lo, axis) + get(r.hi, axis)) * 0.5;
    (((c - lo) / ext * BINS as f32) as usize).min(BINS - 1)
}

/// Best binned object split over all three axes: (SAH cost, split, overlap
/// area of the two children).
fn object_split(refs: &[Ref]) -> (f32, Option<Split>, f32) {
    let mut cb = Aabb::EMPTY;
    for r in refs {
        let c = scale(add(r.lo, r.hi), 0.5);
        cb.grow(c, c);
    }
    let mut best = (f32::MAX, None, 0.0);
    for axis in 0..3 {
        let (lo, ext) = (get(cb.lo, axis), get(cb.hi, axis) - get(cb.lo, axis));
        if ext < 1e-12 {
            continue;
        }
        let mut bins = [(0u32, Aabb::EMPTY); BINS];
        for r in refs {
            let b = &mut bins[obj_bin(r, axis, lo, ext)];
            b.0 += 1;
            b.1.grow(r.lo, r.hi);
        }
        let mut right = [(0u32, Aabb::EMPTY); BINS];
        let mut acc = (0u32, Aabb::EMPTY);
        for s in (1..BINS).rev() {
            acc.0 += bins[s].0;
            acc.1.grow(bins[s].1.lo, bins[s].1.hi);
            right[s] = acc;
        }
        let mut left = (0u32, Aabb::EMPTY);
        for s in 1..BINS {
            left.0 += bins[s - 1].0;
            left.1.grow(bins[s - 1].1.lo, bins[s - 1].1.hi);
            let (rn, rb) = right[s];
            if left.0 == 0 || rn == 0 {
                continue;
            }
            let c = left.0 as f32 * left.1.area() + rn as f32 * rb.area();
            if c < best.0 {
                best = (c, Some(Split::Object { axis, bin: s, lo, ext }), left.1.and(&rb).area());
            }
        }
    }
    best
}

/// Best spatial split over all three axes: (SAH cost, split).
fn spatial_split(refs: &[Ref], bounds: &Aabb, verts: &[[V3; 3]]) -> Option<(f32, Split)> {
    let mut best: Option<(f32, Split)> = None;
    for axis in 0..3 {
        let lo = get(bounds.lo, axis);
        let ext = get(bounds.hi, axis) - lo;
        if ext < 1e-9 {
            continue;
        }
        let w = ext / SBINS as f32;
        let bin_at = |x: f32| (((x - lo) / w) as usize).min(SBINS - 1);
        let mut bins = [Aabb::EMPTY; SBINS];
        let mut enter = [0u32; SBINS];
        let mut exit = [0u32; SBINS];
        for r in refs {
            let (b0, b1) = (bin_at(get(r.lo, axis)), bin_at(get(r.hi, axis)));
            enter[b0] += 1;
            exit[b1] += 1;
            let mut cur = Aabb { lo: r.lo, hi: r.hi };
            for b in b0..b1 {
                let (a, c) = split_tri(&verts[r.tri as usize], &cur, axis, lo + w * (b + 1) as f32);
                bins[b].grow(a.lo, a.hi);
                cur = c;
            }
            bins[b1].grow(cur.lo, cur.hi);
        }
        let mut right = [(0u32, Aabb::EMPTY); SBINS];
        let mut acc = (0u32, Aabb::EMPTY);
        for s in (1..SBINS).rev() {
            acc.0 += exit[s];
            acc.1.grow(bins[s].lo, bins[s].hi);
            right[s] = acc;
        }
        let mut left = (0u32, Aabb::EMPTY);
        for s in 1..SBINS {
            left.0 += enter[s - 1];
            left.1.grow(bins[s - 1].lo, bins[s - 1].hi);
            let (rn, rb) = right[s];
            if left.0 == 0 || rn == 0 {
                continue;
            }
            let c = left.0 as f32 * left.1.area() + rn as f32 * rb.area();
            if best.as_ref().is_none_or(|b| c < b.0) {
                best = Some((c, Split::Spatial { axis, pos: lo + w * s as f32 }));
            }
        }
    }
    best
}
