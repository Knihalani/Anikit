//! Export helpers: turn any easing (springs, bounces, custom functions) into
//! native CSS so the browser compositor can run it off the main thread.

use alloc::string::String;
use alloc::vec::Vec;

use crate::easing::{Easing, StepPosition};
use crate::util::push_num;

/// Ramer–Douglas–Peucker simplification of a sampled curve.
pub fn simplify(points: &[(f64, f64)], tolerance: f64) -> Vec<(f64, f64)> {
    let n = points.len();
    if n <= 2 {
        return points.to_vec();
    }
    let mut keep = alloc::vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = alloc::vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (ax, ay) = points[a];
        let (bx, by) = points[b];
        // Vertical distance: easings are functions of x, so measure error in y.
        let mut worst = (0.0, a);
        for (i, &(x, y)) in points.iter().enumerate().take(b).skip(a + 1) {
            let t = if bx > ax { (x - ax) / (bx - ax) } else { 0.0 };
            let d = crate::math::abs(y - (ay + (by - ay) * t));
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > tolerance {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    points.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// Approximate an easing by a minimal set of `(x, y)` points within `tolerance`.
pub fn linear_points(e: &Easing, tolerance: f64) -> Vec<(f64, f64)> {
    let n = 2048;
    let pts: Vec<(f64, f64)> = (0..=n).map(|i| {
        let x = i as f64 / n as f64;
        (x, e.ease(x))
    }).collect();
    simplify(&pts, tolerance.max(1e-6))
}

/// The most compact native CSS `<easing-function>` for an easing.
pub fn to_css(e: &Easing, tolerance: f64) -> String {
    let mut s = String::new();
    match e {
        Easing::Linear => s.push_str("linear"),
        Easing::Bezier(b) => {
            s.push_str("cubic-bezier(");
            for (i, v) in [b.x1, b.y1, b.x2, b.y2].iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                push_num(&mut s, *v, 4);
            }
            s.push(')');
        }
        Easing::Steps(n, p) => {
            s.push_str("steps(");
            crate::util::push_uint(&mut s, *n as u64);
            s.push_str(match p {
                StepPosition::JumpStart => ",jump-start)",
                StepPosition::JumpEnd => ")",
                StepPosition::JumpNone => ",jump-none)",
                StepPosition::JumpBoth => ",jump-both)",
            });
        }
        _ => {
            let pts = linear_points(e, tolerance);
            s.push_str("linear(");
            let last = pts.len().saturating_sub(1);
            for (i, &(x, y)) in pts.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                push_num(&mut s, y, 4);
                if i != 0 && i != last {
                    s.push(' ');
                    push_num(&mut s, x * 100.0, 2);
                    s.push('%');
                }
            }
            s.push(')');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spring_to_linear_roundtrip() {
        let e = Easing::parse("spring(200, 12)").unwrap();
        let css = to_css(&e, 0.002);
        assert!(css.starts_with("linear(0,"));
        let back = Easing::parse(&css).unwrap();
        let err = crate::easing::max_error(&e, &back, 1000);
        assert!(err < 0.01, "err {err} css {css}");
        assert!(css.len() < 1200, "{}", css.len());
    }
    #[test]
    fn natives() {
        assert_eq!(to_css(&Easing::parse("ease").unwrap(), 0.001), "cubic-bezier(0.25,0.1,0.25,1)");
        assert_eq!(to_css(&Easing::parse("steps(3, jump-none)").unwrap(), 0.001), "steps(3,jump-none)");
        let q = to_css(&Easing::parse("outQuad").unwrap(), 0.001);
        assert!(q.starts_with("linear("));
    }
}
