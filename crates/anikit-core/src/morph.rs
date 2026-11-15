//! Shape morphing between two arbitrary SVG paths.
//!
//! Both paths are normalised (once, at creation time) to the same structure —
//! same number of subpaths, same number of cubic segments per subpath, with
//! the closed subpaths rotated/reversed to minimise point travel. After that a
//! morph is a plain numeric interpolation of control points, which the engine
//! evaluates with a tight, auto-vectorised loop.

use alloc::string::String;
use alloc::vec::Vec;

use crate::path::{cubic_length, split_cubic, write_subpath, Path, Seg, SubPath};

#[derive(Clone, Debug, PartialEq)]
pub struct MorphPlan {
    /// Per subpath: (segment count, closed).
    pub template: Vec<(usize, bool)>,
    /// Flattened channels: per subpath `[sx, sy, (c1x c1y c2x c2y x y) * n]`.
    pub from: Vec<f64>,
    pub to: Vec<f64>,
}

impl MorphPlan {
    pub fn channels(&self) -> usize {
        self.from.len()
    }

    /// Build the path string for already-interpolated channel values.
    pub fn write(&self, values: &[f64], decimals: u32) -> String {
        write_template(&self.template, values, decimals)
    }

    /// Interpolate and serialise at `t`.
    pub fn at(&self, t: f64) -> String {
        let v: Vec<f64> = self.from.iter().zip(&self.to).map(|(a, b)| a + (b - a) * t).collect();
        self.write(&v, 3)
    }

    /// Template encoded as `[n_sub, (nseg, closed) * n_sub]` (used by bindings).
    pub fn template_flat(&self) -> Vec<f64> {
        let mut v = Vec::with_capacity(1 + self.template.len() * 2);
        v.push(self.template.len() as f64);
        for &(n, c) in &self.template {
            v.push(n as f64);
            v.push(if c { 1.0 } else { 0.0 });
        }
        v
    }
}

pub fn write_template(template: &[(usize, bool)], values: &[f64], decimals: u32) -> String {
    let mut s = String::with_capacity(values.len() * 6);
    let mut at = 0;
    let mut segs: Vec<Seg> = Vec::new();
    for &(n, closed) in template {
        if at + 2 + n * 6 > values.len() {
            break;
        }
        let start = [values[at], values[at + 1]];
        at += 2;
        segs.clear();
        for _ in 0..n {
            let mut seg = [0.0; 6];
            seg.copy_from_slice(&values[at..at + 6]);
            segs.push(seg);
            at += 6;
        }
        write_subpath(&mut s, start, &segs, closed, decimals);
    }
    s
}

fn centroid(sp: &SubPath) -> [f64; 2] {
    let mut x = sp.start[0];
    let mut y = sp.start[1];
    for s in &sp.segs {
        x += s[4];
        y += s[5];
    }
    let n = (sp.segs.len() + 1) as f64;
    [x / n, y / n]
}

fn point_subpath(p: [f64; 2], closed: bool) -> SubPath {
    SubPath { start: p, segs: alloc::vec![[p[0], p[1], p[0], p[1], p[0], p[1]]], closed }
}

/// Subdivide segments so the subpath ends up with exactly `target` segments,
/// distributing the splits proportionally to segment length.
fn subdivide(sp: &SubPath, target: usize) -> SubPath {
    let n = sp.segs.len();
    if n >= target || n == 0 {
        return sp.clone();
    }
    let mut lens = Vec::with_capacity(n);
    let mut p0 = sp.start;
    for s in &sp.segs {
        lens.push(cubic_length(p0, s, 1.0).max(1e-9));
        p0 = [s[4], s[5]];
    }
    let total: f64 = lens.iter().sum();
    // Largest-remainder apportionment of `target` pieces (each >= 1).
    let extra = target - n;
    let mut pieces: Vec<usize> = alloc::vec![1; n];
    let mut rema: Vec<(f64, usize)> = Vec::with_capacity(n);
    let mut assigned = 0;
    for i in 0..n {
        let share = lens[i] / total * extra as f64;
        let whole = share as usize;
        pieces[i] += whole;
        assigned += whole;
        rema.push((share - whole as f64, i));
    }
    rema.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(core::cmp::Ordering::Equal));
    for k in 0..(extra - assigned) {
        pieces[rema[k % n].1] += 1;
    }
    let mut out = Vec::with_capacity(target);
    let mut p0 = sp.start;
    for (i, s) in sp.segs.iter().enumerate() {
        let k = pieces[i];
        let mut rest = *s;
        let mut start = p0;
        for j in 0..k - 1 {
            // Split off 1/(k-j) of the remaining curve.
            let t = 1.0 / (k - j) as f64;
            let (a, mid, b) = split_cubic(start, &rest, t);
            out.push(a);
            start = mid;
            rest = b;
        }
        out.push(rest);
        p0 = [s[4], s[5]];
    }
    SubPath { start: sp.start, segs: out, closed: sp.closed }
}

fn rotate_closed(sp: &SubPath, r: usize) -> SubPath {
    let n = sp.segs.len();
    if r == 0 || n == 0 {
        return sp.clone();
    }
    let prev = &sp.segs[r - 1];
    let start = [prev[4], prev[5]];
    let mut segs = Vec::with_capacity(n);
    for k in 0..n {
        segs.push(sp.segs[(r + k) % n]);
    }
    SubPath { start, segs, closed: true }
}

fn on_curve(sp: &SubPath, i: usize) -> [f64; 2] {
    if i == 0 {
        sp.start
    } else {
        let s = &sp.segs[i - 1];
        [s[4], s[5]]
    }
}

/// For a closed `b`, find the rotation (and direction) that best matches `a`.
fn align(a: &SubPath, b: &SubPath) -> SubPath {
    let n = b.segs.len();
    if !a.closed || !b.closed || !(2..=4096).contains(&n) {
        return b.clone();
    }
    let mut rev = b.clone();
    rev.segs = {
        let mut r = Path { subpaths: alloc::vec![b.clone()] };
        r.reverse();
        r.subpaths.pop().map(|s| s.segs).unwrap_or_default()
    };
    rev.start = b.segs.last().map(|s| [s[4], s[5]]).unwrap_or(b.start);
    let mut best = (f64::INFINITY, 0usize, false);
    for (cand, reversed) in [(b, false), (&rev, true)] {
        // On-curve points of the candidate (n points for a closed loop).
        let pts: Vec<[f64; 2]> = (0..n).map(|i| on_curve(cand, i)).collect();
        for r in 0..n {
            let mut cost = 0.0;
            for i in 0..n {
                let pa = on_curve(a, i);
                let pb = pts[(i + r) % n];
                cost += (pa[0] - pb[0]) * (pa[0] - pb[0]) + (pa[1] - pb[1]) * (pa[1] - pb[1]);
                if cost >= best.0 {
                    break;
                }
            }
            if cost < best.0 {
                best = (cost, r, reversed);
            }
        }
    }
    let base = if best.2 { &rev } else { b };
    rotate_closed(base, best.1)
}

/// Build a morph plan between two paths.
pub fn prepare(a: &Path, b: &Path) -> MorphPlan {
    let mut sa = a.subpaths.clone();
    let mut sb = b.subpaths.clone();
    if sa.is_empty() {
        sa.push(point_subpath([0.0, 0.0], false));
    }
    if sb.is_empty() {
        sb.push(point_subpath([0.0, 0.0], false));
    }
    // Match subpath counts by collapsing extras into points at their centroid.
    while sa.len() < sb.len() {
        let other = &sb[sa.len()];
        sa.push(point_subpath(centroid(other), other.closed));
    }
    while sb.len() < sa.len() {
        let other = &sa[sb.len()];
        sb.push(point_subpath(centroid(other), other.closed));
    }
    let mut plan = MorphPlan { template: Vec::new(), from: Vec::new(), to: Vec::new() };
    for (pa, pb) in sa.iter().zip(sb.iter()) {
        let mut pa = pa.clone();
        let mut pb = pb.clone();
        if pa.segs.is_empty() {
            pa = point_subpath(pa.start, pb.closed);
        }
        if pb.segs.is_empty() {
            pb = point_subpath(pb.start, pa.closed);
        }
        let n = pa.segs.len().max(pb.segs.len());
        let pa = subdivide(&pa, n);
        let pb = subdivide(&pb, n);
        let closed = pa.closed && pb.closed;
        let pb = if closed { align(&pa, &pb) } else { pb };
        plan.template.push((n, closed || pa.closed || pb.closed));
        for (sp, out) in [(&pa, &mut plan.from), (&pb, &mut plan.to)] {
            out.push(sp.start[0]);
            out.push(sp.start[1]);
            for s in &sp.segs {
                out.extend_from_slice(s);
            }
        }
    }
    plan
}

/// A morph through any number of shapes, all normalised to one structure.
#[derive(Clone, Debug, PartialEq)]
pub struct MorphSeq {
    pub template: Vec<(usize, bool)>,
    /// One channel vector per shape (all the same length).
    pub frames: Vec<Vec<f64>>,
}

impl MorphSeq {
    pub fn channels(&self) -> usize {
        self.frames.first().map(|f| f.len()).unwrap_or(0)
    }
    pub fn template_flat(&self) -> Vec<f64> {
        MorphPlan { template: self.template.clone(), from: Vec::new(), to: Vec::new() }.template_flat()
    }
    /// Path data at shape-sequence progress `t ∈ [0, 1]` (linear between shapes).
    pub fn at(&self, t: f64) -> String {
        let n = self.frames.len();
        if n == 0 {
            return String::new();
        }
        if n == 1 {
            return write_template(&self.template, &self.frames[0], 3);
        }
        let f = t.clamp(0.0, 1.0) * (n - 1) as f64;
        let i = (f as usize).min(n - 2);
        let k = f - i as f64;
        let v: Vec<f64> = self.frames[i].iter().zip(&self.frames[i + 1]).map(|(a, b)| a + (b - a) * k).collect();
        write_template(&self.template, &v, 3)
    }
}

/// Normalise several shapes to a common structure so they can be used as
/// keyframes of one morph. Each shape is aligned to the previous one.
pub fn prepare_many(paths: &[Path]) -> MorphSeq {
    if paths.is_empty() {
        return MorphSeq { template: Vec::new(), frames: Vec::new() };
    }
    let mut subs: Vec<Vec<SubPath>> = paths
        .iter()
        .map(|p| if p.subpaths.is_empty() { alloc::vec![point_subpath([0.0, 0.0], false)] } else { p.subpaths.clone() })
        .collect();
    let nsub = subs.iter().map(|s| s.len()).max().unwrap_or(1);
    // Pad subpath counts with points collapsed at a neighbour's centroid.
    for k in 0..subs.len() {
        while subs[k].len() < nsub {
            let idx = subs[k].len();
            let donor = subs.iter().find(|s| s.len() > idx).map(|s| s[idx].clone());
            let (c, closed) = donor.map(|d| (centroid(&d), d.closed)).unwrap_or(([0.0, 0.0], false));
            subs[k].push(point_subpath(c, closed));
        }
    }
    let mut seq = MorphSeq { template: Vec::new(), frames: alloc::vec![Vec::new(); paths.len()] };
    for j in 0..nsub {
        let column: Vec<SubPath> = subs
            .iter()
            .map(|s| if s[j].segs.is_empty() { point_subpath(s[j].start, s[j].closed) } else { s[j].clone() })
            .collect();
        let n = column.iter().map(|c| c.segs.len()).max().unwrap_or(1);
        let all_closed = column.iter().all(|c| c.closed);
        let any_closed = column.iter().any(|c| c.closed);
        let mut prev: Option<SubPath> = None;
        for (k, c) in column.iter().enumerate() {
            let mut d = subdivide(c, n);
            if all_closed {
                if let Some(p) = &prev {
                    d = align(p, &d);
                }
            }
            let out = &mut seq.frames[k];
            out.push(d.start[0]);
            out.push(d.start[1]);
            for s in &d.segs {
                out.extend_from_slice(s);
            }
            prev = Some(d);
        }
        seq.template.push((n, any_closed));
    }
    seq
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morph_many() {
        let shapes = [
            Path::parse("M0 0 H100 V100 H0 Z"),
            Path::ellipse(50.0, 50.0, 50.0, 50.0),
            Path::star(50.0, 50.0, 7, 50.0, 20.0, 0.0),
            Path::parse("M0 0 L10 0 Z M50 50 L60 60 L50 60 Z"),
        ];
        let seq = prepare_many(&shapes);
        assert_eq!(seq.frames.len(), 4);
        let ch = seq.channels();
        assert!(seq.frames.iter().all(|f| f.len() == ch));
        assert_eq!(seq.template.len(), 2);
        let first = Path::parse(&seq.at(0.0));
        assert!((first.length() - 400.0).abs() < 1e-2);
        let star = Path::parse(&seq.at(2.0 / 3.0));
        assert!((star.length() - shapes[2].length()).abs() < 1e-2);
    }

    #[test]
    fn morph_square_to_circle() {
        let sq = Path::parse("M0 0 H100 V100 H0 Z");
        let ci = Path::ellipse(50.0, 50.0, 50.0, 50.0);
        let plan = prepare(&sq, &ci);
        assert_eq!(plan.from.len(), plan.to.len());
        assert_eq!(plan.template.len(), 1);
        // Endpoints reproduce the inputs' geometry.
        let start = Path::parse(&plan.at(0.0));
        assert!((start.length() - 400.0).abs() < 1e-2, "{}", start.length());
        let end = Path::parse(&plan.at(1.0));
        assert!((end.length() - ci.length()).abs() < 1e-2);
    }

    #[test]
    fn different_subpath_counts() {
        let a = Path::parse("M0 0 L10 0 L10 10 Z M20 20 L30 20 L30 30 Z");
        let b = Path::parse("M0 0 L10 0 L10 10 L0 10 Z");
        let plan = prepare(&a, &b);
        assert_eq!(plan.template.len(), 2);
        let mid = Path::parse(&plan.at(0.5));
        assert_eq!(mid.subpaths.len(), 2);
    }

    #[test]
    fn alignment_minimises_travel() {
        let a = Path::parse("M0 0 L10 0 L10 10 L0 10 Z");
        // Same square, different start point and direction.
        let b = Path::parse("M10 10 L10 0 L0 0 L0 10 Z");
        let plan = prepare(&a, &b);
        let travel: f64 = plan.from.iter().zip(&plan.to).map(|(x, y)| (x - y).abs()).sum();
        assert!(travel < 1e-9, "travel {travel}");
    }
}
