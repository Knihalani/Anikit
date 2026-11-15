//! Damped harmonic oscillator, solved in closed form.
//!
//! Instead of numerically integrating (which drifts and explodes at large time
//! steps), every step uses the exact analytic solution expressed as a 2×2
//! state-transition matrix:
//!
//! ```text
//! [x(t+dt)]   [a b] [x(t)]
//! [v(t+dt)] = [c d] [v(t)]
//! ```
//!
//! where `x` is the displacement from the target. The matrix depends only on
//! the spring parameters and `dt`, so the engine computes it once per distinct
//! spring configuration per frame and then advances every channel of every
//! spring using it with four multiply-adds.

use crate::math::{abs, exp, ln, sincos, sqrt, PI};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringParams {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
}

impl Default for SpringParams {
    fn default() -> Self {
        SpringParams { stiffness: 100.0, damping: 10.0, mass: 1.0 }
    }
}

impl SpringParams {
    pub fn new(stiffness: f64, damping: f64, mass: f64) -> Self {
        SpringParams {
            stiffness: if stiffness > 0.0 { stiffness } else { 1e-6 },
            damping: if damping >= 0.0 { damping } else { 0.0 },
            mass: if mass > 0.0 { mass } else { 1e-6 },
        }
    }

    /// Perceptual parameterisation: `duration` is the approximate time (in
    /// seconds) for the spring to visually reach its target; `bounce` in
    /// `[0, 1)` controls overshoot (0 = critically damped, no bounce).
    pub fn from_duration_bounce(duration: f64, bounce: f64) -> Self {
        let duration = if duration > 0.001 { duration } else { 0.001 };
        let zeta = crate::math::clamp(1.0 - bounce, 0.05, 1.0);
        // Natural frequency chosen so the response's 1st crossing (or ~98% for
        // critical damping) lands near `duration`.
        let omega = 2.0 * PI / (duration * 1.2);
        SpringParams { stiffness: omega * omega, damping: 2.0 * zeta * omega, mass: 1.0 }
    }

    /// Damping ratio ζ.
    pub fn zeta(&self) -> f64 {
        self.damping / (2.0 * sqrt(self.stiffness * self.mass))
    }
    /// Undamped angular frequency ω₀ (rad/s).
    pub fn omega0(&self) -> f64 {
        sqrt(self.stiffness / self.mass)
    }

    /// State transition matrix `[a, b, c, d]` for a step of `dt` seconds.
    pub fn matrix(&self, dt: f64) -> [f64; 4] {
        let w0 = self.omega0();
        let z = self.zeta();
        if dt <= 0.0 {
            return [1.0, 0.0, 0.0, 1.0];
        }
        if abs(z - 1.0) < 1e-6 {
            // Critically damped.
            let e = exp(-w0 * dt);
            [e * (1.0 + w0 * dt), e * dt, -e * w0 * w0 * dt, e * (1.0 - w0 * dt)]
        } else if z < 1.0 {
            let alpha = z * w0;
            let wd = w0 * sqrt(1.0 - z * z);
            let e = exp(-alpha * dt);
            // One argument reduction for both (bit-identical to sin + cos).
            let (sn, cs) = sincos(wd * dt);
            [
                e * (cs + alpha / wd * sn),
                e * sn / wd,
                -e * sn * w0 * w0 / wd,
                e * (cs - alpha / wd * sn),
            ]
        } else {
            let s = sqrt(z * z - 1.0);
            let r1 = -w0 * (z - s);
            let r2 = -w0 * (z + s);
            let e1 = exp(r1 * dt);
            let e2 = exp(r2 * dt);
            let inv = 1.0 / (r1 - r2);
            [
                (r1 * e2 - r2 * e1) * inv,
                (e1 - e2) * inv,
                r1 * r2 * (e2 - e1) * inv,
                (r1 * e1 - r2 * e2) * inv,
            ]
        }
    }

    /// Position (0 → 1) of a spring released from 0 towards 1 with initial
    /// velocity `v0` (in units of "distance per second" where distance = 1),
    /// evaluated at time `t` seconds.
    #[inline]
    pub fn response(&self, t: f64, v0: f64) -> f64 {
        let m = self.matrix(t);
        // x0 = -1 (displacement from target).
        1.0 - m[0] + m[1] * v0
    }

    /// Time (seconds) after which the normalised response stays within
    /// `epsilon` of the target. Capped at 60s.
    pub fn settle_time(&self, v0: f64, epsilon: f64) -> f64 {
        let w0 = self.omega0();
        let z = self.zeta();
        // Analytic estimate from the decay envelope, then refine by sampling.
        let decay = if z < 1.0 { z * w0 } else { w0 * (z - sqrt(z * z - 1.0)) };
        if decay <= 0.0 {
            return 60.0;
        }
        let amp = 1.0 + abs(v0) / w0.max(1e-9);
        let guess = (ln(amp / epsilon) / decay).clamp(0.0, 60.0);
        // Walk backwards to find the last time the response left the band.
        let dt = (guess / 400.0).max(1.0 / 2000.0);
        let step = self.matrix(dt);
        let mut x = -1.0;
        let mut v = v0;
        let mut t = 0.0;
        let mut last_out = 0.0;
        while t < guess * 1.5 + dt {
            let nx = step[0] * x + step[1] * v;
            let nv = step[2] * x + step[3] * v;
            x = nx;
            v = nv;
            t += dt;
            if abs(x) > epsilon {
                last_out = t;
            }
        }
        last_out.min(60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integrate(p: &SpringParams, x0: f64, v0: f64, t: f64) -> (f64, f64) {
        // Reference RK4 integration with a tiny step.
        let n = 200_000;
        let h = t / n as f64;
        let (mut x, mut v) = (x0, v0);
        let f = |x: f64, v: f64| -> f64 { (-p.stiffness * x - p.damping * v) / p.mass };
        for _ in 0..n {
            let k1x = v;
            let k1v = f(x, v);
            let k2x = v + 0.5 * h * k1v;
            let k2v = f(x + 0.5 * h * k1x, v + 0.5 * h * k1v);
            let k3x = v + 0.5 * h * k2v;
            let k3v = f(x + 0.5 * h * k2x, v + 0.5 * h * k2v);
            let k4x = v + h * k3v;
            let k4v = f(x + h * k3x, v + h * k3v);
            x += h / 6.0 * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
            v += h / 6.0 * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
        }
        (x, v)
    }

    #[test]
    fn matrix_matches_integration_all_regimes() {
        for p in [
            SpringParams::new(100.0, 10.0, 1.0), // under
            SpringParams::new(100.0, 20.0, 1.0), // critical
            SpringParams::new(100.0, 50.0, 1.0), // over
            SpringParams::new(300.0, 2.0, 2.0),
        ] {
            let m = p.matrix(0.37);
            let (x, v) = integrate(&p, 1.3, -2.0, 0.37);
            let mx = m[0] * 1.3 + m[1] * -2.0;
            let mv = m[2] * 1.3 + m[3] * -2.0;
            assert!((x - mx).abs() < 1e-6, "{p:?} x {x} vs {mx}");
            assert!((v - mv).abs() < 1e-5, "{p:?} v {v} vs {mv}");
        }
    }

    #[test]
    fn composition_is_exact() {
        let p = SpringParams::new(170.0, 26.0, 1.0);
        let a = p.matrix(0.1);
        let b = p.matrix(0.2);
        // a·a == b
        let aa = [
            a[0] * a[0] + a[1] * a[2],
            a[0] * a[1] + a[1] * a[3],
            a[2] * a[0] + a[3] * a[2],
            a[2] * a[1] + a[3] * a[3],
        ];
        for i in 0..4 {
            assert!((aa[i] - b[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn settle() {
        let p = SpringParams::new(100.0, 10.0, 1.0);
        let t = p.settle_time(0.0, 0.001);
        assert!(t > 0.5 && t < 2.0, "{t}");
        assert!((p.response(t + 0.5, 0.0) - 1.0).abs() < 0.001);
        let pb = SpringParams::from_duration_bounce(0.5, 0.0);
        assert!((pb.zeta() - 1.0).abs() < 1e-9);
    }
}
