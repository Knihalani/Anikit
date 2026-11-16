//! Path trimming by arc length (`Stroke::trim`), for draw-on effects.

use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg, Point};

const ACCURACY: f64 = 0.02;

/// Write the part of `src` between fractions `t0..t1` of its total length into `out`.
/// `lens` is reusable scratch.
pub fn trim_path(src: &BezPath, t0: f32, t1: f32, out: &mut BezPath, lens: &mut Vec<f64>) {
    out.truncate(0);
    let fix = |v: f32, d: f32| if v.is_nan() { d as f64 } else { v.clamp(0.0, 1.0) as f64 };
    let (t0, t1) = (fix(t0, 0.0), fix(t1, 1.0));
    if t1 <= t0 {
        return;
    }
    lens.clear();
    lens.extend(src.segments().map(|s| s.arclen(ACCURACY)));
    let total: f64 = lens.iter().sum();
    if total <= 0.0 {
        return;
    }
    let (a, b) = (t0 * total, t1 * total);
    let mut acc = 0.0;
    let mut last_end: Option<(usize, Point)> = None;
    for (i, seg) in src.segments().enumerate() {
        let len = lens[i];
        let (s0, s1) = (acc, acc + len);
        acc = s1;
        if s1 <= a || s0 >= b || len <= 0.0 {
            continue;
        }
        let p0 = if a > s0 { seg.inv_arclen(a - s0, ACCURACY) } else { 0.0 };
        let p1 = if b < s1 { seg.inv_arclen(b - s0, ACCURACY) } else { 1.0 };
        let sub = seg.subsegment(p0..p1);
        let start = sub.start();
        let contiguous = matches!(last_end, Some((j, p)) if j + 1 == i && p.distance(start) < 1e-6);
        if !contiguous {
            out.move_to(start);
        }
        match sub {
            PathSeg::Line(l) => out.line_to(l.p1),
            PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
            PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
        }
        last_end = Some((i, sub.end()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_half_a_line_path() {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((100.0, 0.0));
        p.line_to((100.0, 100.0));
        let mut out = BezPath::new();
        trim_path(&p, 0.25, 0.75, &mut out, &mut Vec::new());
        let segs: Vec<_> = out.segments().collect();
        assert_eq!(segs.len(), 2);
        assert!((segs[0].start().x - 50.0).abs() < 1e-6);
        assert!((segs[1].end().y - 50.0).abs() < 1e-6);
    }
}
