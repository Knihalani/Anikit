//! Point clouds that flow between shapes.
//!
//! A [`Cloud`] holds `n` particles and any number of target shapes (each `n`
//! xyz positions). Every step, each particle's target is a blend between two
//! shapes, offset per particle so the transition ripples through the cloud,
//! and the particle follows it with its own damped spring plus a cheap
//! turbulence field. The result is organic, interruptible morphing: scrub the
//! blend from a scroll position, change shapes mid-flight, or blast the
//! particles with [`Cloud::explode`] and they find their way home.
//!
//! The hot loop is branch-light f32 math with a polynomial sine; 100k
//! particles step in well under a millisecond natively.

use alloc::vec;
use alloc::vec::Vec;
use crate::math::{floorf, sqrtf};

/// Tunables for [`Cloud::step`].
#[derive(Clone, Copy, Debug)]
pub struct CloudParams {
    /// Spring stiffness toward the target (1/s², default 60).
    pub stiffness: f32,
    /// Velocity damping (1/s, default 9).
    pub damping: f32,
    /// How much the transition is spread across particles (0 = all at once, default 0.6).
    pub spread: f32,
    /// Resting turbulence amplitude in world units (default 2).
    pub noise: f32,
    /// Extra turbulence while a particle is mid-transition (default 40).
    pub swirl: f32,
    /// Spatial frequency of the turbulence (default 0.01).
    pub frequency: f32,
}

impl Default for CloudParams {
    fn default() -> Self {
        Self { stiffness: 60.0, damping: 9.0, spread: 0.6, noise: 2.0, swirl: 40.0, frequency: 0.01 }
    }
}

/// A particle cloud morphing between target shapes.
#[derive(Debug)]
pub struct Cloud {
    n: usize,
    /// Current positions, `[x, y, z] * n`.
    pub pos: Vec<f32>,
    vel: Vec<f32>,
    /// Per-particle random in [0, 1): transition order and stiffness jitter.
    seed: Vec<f32>,
    shapes: Vec<Vec<f32>>,
    pub params: CloudParams,
    time: f32,
}

/// Fast sine: wraps to [-π, π] and uses a refined parabola (max error ≈ 0.001).
#[inline(always)]
fn fsin(x: f32) -> f32 {
    const TAU: f32 = core::f32::consts::TAU;
    const INV_TAU: f32 = 1.0 / TAU;
    let x = x - TAU * floorf(x * INV_TAU + 0.5);
    let b = 4.0 / core::f32::consts::PI;
    let c = -4.0 / (core::f32::consts::PI * core::f32::consts::PI);
    let y = b * x + c * x * if x < 0.0 { -x } else { x };
    0.225 * (y * if y < 0.0 { -y } else { y } - y) + y
}

#[inline(always)]
fn smooth(t: f32) -> f32 {
    // inOutCubic
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u * 0.5
    }
}

impl Cloud {
    /// `n` particles at the origin, `shapes` empty targets.
    pub fn new(n: usize, shapes: usize, seed: u32) -> Self {
        let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
        let seeds = (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s >> 8) as f32 / (1u32 << 24) as f32
            })
            .collect();
        Self {
            n,
            pos: vec![0.0; n * 3],
            vel: vec![0.0; n * 3],
            seed: seeds,
            shapes: (0..shapes).map(|_| vec![0.0; n * 3]).collect(),
            params: CloudParams::default(),
            time: 0.0,
        }
    }

    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    /// Mutable access to shape `k` (`[x, y, z] * n`), growing the list if needed.
    pub fn shape_mut(&mut self, k: usize) -> &mut [f32] {
        while self.shapes.len() <= k {
            self.shapes.push(vec![0.0; self.n * 3]);
        }
        &mut self.shapes[k]
    }

    /// Snap every particle onto shape `k` with zero velocity.
    pub fn snap(&mut self, k: usize) {
        if let Some(s) = self.shapes.get(k) {
            self.pos.copy_from_slice(s);
            self.vel.iter_mut().for_each(|v| *v = 0.0);
        }
    }

    /// Advance `dt` seconds following the blend `from → to` at `mix ∈ [0, 1]`.
    pub fn step(&mut self, dt: f32, from: usize, to: usize, mix: f32) {
        let ns = self.shapes.len();
        if ns == 0 || self.n == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 1.0 / 20.0);
        self.time += dt;
        let (from, to) = (from.min(ns - 1), to.min(ns - 1));
        let p = self.params;
        let (a, b) = (&self.shapes[from], &self.shapes[to]);
        let spread = p.spread.max(0.0);
        let m = mix.clamp(0.0, 1.0) * (1.0 + spread);
        let t = self.time;
        let f = p.frequency;
        let h = dt;
        let damp = 1.0 - p.damping * h;
        let (t1, t2, t3) = (t * 1.3, t * 1.1, t * 0.9);
        let iter = self
            .pos
            .chunks_exact_mut(3)
            .zip(self.vel.chunks_exact_mut(3))
            .zip(a.chunks_exact(3).zip(b.chunks_exact(3)))
            .zip(self.seed.iter());
        for (((pp, vv), (pa, pb)), &sd) in iter {
            let local = (m - sd * spread).clamp(0.0, 1.0);
            let e = smooth(local);
            let tx = pa[0] + (pb[0] - pa[0]) * e;
            let ty = pa[1] + (pb[1] - pa[1]) * e;
            let tz = pa[2] + (pb[2] - pa[2]) * e;
            // Turbulence peaks mid-transition.
            let amp = p.noise + p.swirl * 4.0 * local * (1.0 - local);
            let ph = sd * 40.0;
            let ox = amp * fsin(f * ty + t1 + ph);
            let oy = amp * fsin(f * tz + t2 + ph * 1.7);
            let oz = amp * fsin(f * tx + t3 + ph * 2.3);
            let sh = p.stiffness * (0.55 + 0.9 * sd) * h;
            // Semi-implicit Euler (stable: stiffness·dt² ≪ 1 at the clamped dt).
            let vx = vv[0] * damp + (tx + ox - pp[0]) * sh;
            let vy = vv[1] * damp + (ty + oy - pp[1]) * sh;
            let vz = vv[2] * damp + (tz + oz - pp[2]) * sh;
            pp[0] += vx * h;
            pp[1] += vy * h;
            pp[2] += vz * h;
            vv[0] = vx;
            vv[1] = vy;
            vv[2] = vz;
        }
    }

    /// Radial velocity kick away from `(x, y, z)`; negative strength pulls in.
    pub fn explode(&mut self, x: f32, y: f32, z: f32, strength: f32) {
        for i in 0..self.n {
            let k = i * 3;
            let (dx, dy, dz) = (self.pos[k] - x, self.pos[k + 1] - y, self.pos[k + 2] - z);
            let d = sqrtf(dx * dx + dy * dy + dz * dz).max(1.0);
            let s = strength * (0.4 + 1.2 * self.seed[i]) / d;
            self.vel[k] += dx * s;
            self.vel[k + 1] += dy * s;
            self.vel[k + 2] += dz * s;
        }
    }

    /// Mean speed (for "settled" checks).
    pub fn mean_speed(&self) -> f32 {
        if self.n == 0 {
            return 0.0;
        }
        let mut s = 0.0;
        for v in self.vel.chunks_exact(3) {
            s += sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        }
        s / self.n as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_sine_is_accurate() {
        let mut x = -20.0f32;
        while x < 20.0 {
            assert!((fsin(x) - libm::sinf(x)).abs() < 0.002, "{x}");
            x += 0.013;
        }
    }

    #[test]
    fn settles_on_target_and_morphs() {
        let mut c = Cloud::new(500, 2, 7);
        for (i, v) in c.shape_mut(0).iter_mut().enumerate() {
            *v = (i % 17) as f32;
        }
        for (i, v) in c.shape_mut(1).iter_mut().enumerate() {
            *v = 100.0 + (i % 5) as f32;
        }
        c.params.noise = 0.0;
        c.params.swirl = 0.0;
        for _ in 0..600 {
            c.step(1.0 / 60.0, 0, 1, 0.0);
        }
        let err: f32 = c.pos.iter().zip(&c.shapes[0]).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(err < 0.05, "err {err}");
        for _ in 0..900 {
            c.step(1.0 / 60.0, 0, 1, 1.0);
        }
        let err: f32 = c.pos.iter().zip(&c.shapes[1]).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(err < 0.05, "err {err}");
        assert!(c.mean_speed() < 0.01);
    }

    #[test]
    fn explode_then_return() {
        let mut c = Cloud::new(200, 1, 1);
        c.params.noise = 0.0;
        c.snap(0);
        c.explode(0.0, 0.0, -10.0, 500.0);
        assert!(c.mean_speed() > 10.0);
        for _ in 0..900 {
            c.step(1.0 / 60.0, 0, 0, 0.0);
        }
        assert!(c.pos.iter().all(|v| v.abs() < 0.05));
    }
}
