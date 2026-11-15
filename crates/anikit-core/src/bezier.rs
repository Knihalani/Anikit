//! CSS `cubic-bezier()` timing function.
//!
//! Solving `x(t) = input` uses a precomputed sample table to get a good initial
//! guess, followed by Newton–Raphson (or bisection when the slope is flat).
//! Typical cost: one table lookup + 1–2 Newton iterations.

use crate::math::abs;

const SAMPLES: usize = 11;
const STEP: f64 = 1.0 / (SAMPLES as f64 - 1.0);
const NEWTON_ITERS: usize = 4;
const NEWTON_MIN_SLOPE: f64 = 0.001;
const SUBDIV_PRECISION: f64 = 1e-7;
const SUBDIV_MAX: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    // Polynomial coefficients for x(t) and y(t).
    ax: f64,
    bx: f64,
    cx: f64,
    ay: f64,
    by: f64,
    cy: f64,
    table: [f64; SAMPLES],
    linear: bool,
}

impl CubicBezier {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        let x1 = crate::math::clamp(x1, 0.0, 1.0);
        let x2 = crate::math::clamp(x2, 0.0, 1.0);
        let cx = 3.0 * x1;
        let bx = 3.0 * (x2 - x1) - cx;
        let ax = 1.0 - cx - bx;
        let cy = 3.0 * y1;
        let by = 3.0 * (y2 - y1) - cy;
        let ay = 1.0 - cy - by;
        let mut b = CubicBezier {
            x1,
            y1,
            x2,
            y2,
            ax,
            bx,
            cx,
            ay,
            by,
            cy,
            table: [0.0; SAMPLES],
            linear: x1 == y1 && x2 == y2,
        };
        for i in 0..SAMPLES {
            b.table[i] = b.sample_x(i as f64 * STEP);
        }
        b
    }

    #[inline(always)]
    fn sample_x(&self, t: f64) -> f64 {
        ((self.ax * t + self.bx) * t + self.cx) * t
    }
    #[inline(always)]
    fn sample_y(&self, t: f64) -> f64 {
        ((self.ay * t + self.by) * t + self.cy) * t
    }
    #[inline(always)]
    fn slope_x(&self, t: f64) -> f64 {
        (3.0 * self.ax * t + 2.0 * self.bx) * t + self.cx
    }

    fn t_for_x(&self, x: f64) -> f64 {
        // Locate the table interval.
        let mut start = 0.0;
        let mut i = 1;
        while i < SAMPLES - 1 && self.table[i] <= x {
            start += STEP;
            i += 1;
        }
        i -= 1;
        let span = self.table[i + 1] - self.table[i];
        let dist = if span > 0.0 { (x - self.table[i]) / span } else { 0.0 };
        let mut t = start + dist * STEP;
        let slope = self.slope_x(t);
        if slope >= NEWTON_MIN_SLOPE {
            for _ in 0..NEWTON_ITERS {
                let s = self.slope_x(t);
                if s == 0.0 {
                    break;
                }
                let cur = self.sample_x(t) - x;
                t -= cur / s;
            }
            t
        } else if slope == 0.0 {
            t
        } else {
            let mut a = start;
            let mut b = start + STEP;
            for _ in 0..SUBDIV_MAX {
                t = a + (b - a) * 0.5;
                let cur = self.sample_x(t) - x;
                if abs(cur) <= SUBDIV_PRECISION {
                    break;
                }
                if cur > 0.0 {
                    b = t;
                } else {
                    a = t;
                }
            }
            t
        }
    }

    #[inline]
    pub fn ease(&self, x: f64) -> f64 {
        if self.linear {
            return x;
        }
        if x <= 0.0 {
            // Linear extrapolation using the start tangent (CSS Easing Level 2).
            return if self.x1 > 0.0 {
                x * self.y1 / self.x1
            } else if self.y1 == 0.0 && self.x2 > 0.0 {
                x * self.y2 / self.x2
            } else {
                0.0
            };
        }
        if x >= 1.0 {
            return if self.x2 < 1.0 {
                1.0 + (x - 1.0) * (self.y2 - 1.0) / (self.x2 - 1.0)
            } else if self.y2 == 1.0 && self.x1 < 1.0 {
                1.0 + (x - 1.0) * (self.y1 - 1.0) / (self.x1 - 1.0)
            } else {
                1.0
            };
        }
        self.sample_y(self.t_for_x(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ease_matches_reference() {
        // CSS `ease` = cubic-bezier(0.25, 0.1, 0.25, 1)
        let b = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
        assert!((b.ease(0.5) - 0.8024033877399112).abs() < 1e-5);
        assert_eq!(b.ease(0.0), 0.0);
        assert_eq!(b.ease(1.0), 1.0);
        let l = CubicBezier::new(0.3, 0.3, 0.7, 0.7);
        assert_eq!(l.ease(0.42), 0.42);
        // Monotone sweep.
        let mut prev = -1.0;
        for i in 0..=1000 {
            let y = b.ease(i as f64 / 1000.0);
            assert!(y >= prev - 1e-9);
            prev = y;
        }
    }
}
