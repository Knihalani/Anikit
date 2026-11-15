//! Particle emitter: spawn, simulate and age particles in 3D.
//!
//! Continuous emission (`rate` per second) and one-shot bursts from a point,
//! sphere, box or ring, with velocity spread around a direction, gravity,
//! drag, a cheap curl-ish turbulence field, an optional attractor, and
//! size/colour over life. Dead particles are removed by swap-remove, so the
//! live set is always the first `len()` slots — renderers read contiguous
//! arrays with no gaps. Output feeds [`crate::space::Projector`] (positions)
//! and [`crate::splat::Splat`] (per-particle colours).

use crate::math::sqrtf;
use alloc::vec::Vec;

/// Where new particles appear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpawnShape {
    Point,
    /// Inside a sphere of radius `size[0]`.
    Sphere,
    /// Inside a box of half-extents `size`.
    Box,
    /// On a ring of radius `size[0]` in the XZ plane.
    Ring,
}

#[derive(Clone, Copy, Debug)]
pub struct EmitterParams {
    /// Particles per second (continuous emission).
    pub rate: f32,
    /// Lifetime in seconds and its random variation (± fraction).
    pub life: f32,
    pub life_var: f32,
    /// Spawn position and shape.
    pub origin: [f32; 3],
    pub shape: SpawnShape,
    pub size: [f32; 3],
    /// Mean launch direction (normalised internally), speed and its variation.
    pub direction: [f32; 3],
    pub speed: f32,
    pub speed_var: f32,
    /// Cone half-angle around `direction` in radians (π = all directions).
    pub spread: f32,
    pub gravity: [f32; 3],
    /// Velocity damping per second.
    pub drag: f32,
    /// Turbulence amplitude (units/s²) and spatial frequency.
    pub turbulence: f32,
    pub frequency: f32,
    /// Attractor position and strength (0 = off; negative repels).
    pub attractor: [f32; 3],
    pub attract: f32,
    /// Size at birth and death (multiplier for renderers).
    pub size_start: f32,
    pub size_end: f32,
}

impl Default for EmitterParams {
    fn default() -> Self {
        Self {
            rate: 1000.0,
            life: 2.0,
            life_var: 0.3,
            origin: [0.0; 3],
            shape: SpawnShape::Point,
            size: [0.0; 3],
            direction: [0.0, 1.0, 0.0],
            speed: 200.0,
            speed_var: 0.4,
            spread: 0.4,
            gravity: [0.0, -300.0, 0.0],
            drag: 0.4,
            turbulence: 0.0,
            frequency: 0.01,
            attractor: [0.0; 3],
            attract: 0.0,
            size_start: 1.0,
            size_end: 0.0,
        }
    }
}

/// A colour gradient over normalised age: up to 4 RGB stops at 0, ⅓, ⅔, 1.
pub type Gradient = [[f32; 3]; 4];

#[derive(Debug)]
pub struct Emitter {
    pub params: EmitterParams,
    /// Maximum live particles.
    pub capacity: usize,
    /// Live positions `[x, y, z] * len`.
    pub pos: Vec<f32>,
    vel: Vec<f32>,
    age: Vec<f32>,
    life: Vec<f32>,
    /// Per-particle `[r, g, b] * len` after [`Emitter::shade`].
    pub colors: Vec<f32>,
    /// Per-particle size multiplier after [`Emitter::shade`].
    pub sizes: Vec<f32>,
    carry: f32,
    rng: u32,
    time: f32,
}

#[inline(always)]
fn fsin(x: f32) -> f32 {
    // Same polynomial sine as the point cloud (max error ≈ 0.001).
    const TAU: f32 = core::f32::consts::TAU;
    let x = x - TAU * crate::math::floorf(x * (1.0 / TAU) + 0.5);
    let b = 4.0 / core::f32::consts::PI;
    let c = -4.0 / (core::f32::consts::PI * core::f32::consts::PI);
    let ax = if x < 0.0 { -x } else { x };
    let y = b * x + c * x * ax;
    let ay = if y < 0.0 { -y } else { y };
    0.225 * (y * ay - y) + y
}

impl Emitter {
    pub fn new(capacity: usize, seed: u32) -> Self {
        Self {
            params: EmitterParams::default(),
            capacity,
            pos: Vec::with_capacity(capacity * 3),
            vel: Vec::with_capacity(capacity * 3),
            age: Vec::with_capacity(capacity),
            life: Vec::with_capacity(capacity),
            colors: Vec::new(),
            sizes: Vec::new(),
            carry: 0.0,
            rng: seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453) | 1,
            time: 0.0,
        }
    }

    pub fn len(&self) -> usize {
        self.age.len()
    }
    pub fn is_empty(&self) -> bool {
        self.age.is_empty()
    }

    #[inline]
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Spawn `n` particles now (a burst), up to capacity.
    pub fn burst(&mut self, n: usize) {
        let p = self.params;
        let d = p.direction;
        let dl = sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).max(1e-6);
        let dir = [d[0] / dl, d[1] / dl, d[2] / dl];
        // An orthonormal basis around `dir` for the cone.
        let up = if dir[1] < 0.9 && dir[1] > -0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let mut t1 = [up[1] * dir[2] - up[2] * dir[1], up[2] * dir[0] - up[0] * dir[2], up[0] * dir[1] - up[1] * dir[0]];
        let tl = sqrtf(t1[0] * t1[0] + t1[1] * t1[1] + t1[2] * t1[2]).max(1e-6);
        t1 = [t1[0] / tl, t1[1] / tl, t1[2] / tl];
        let t2 = [dir[1] * t1[2] - dir[2] * t1[1], dir[2] * t1[0] - dir[0] * t1[2], dir[0] * t1[1] - dir[1] * t1[0]];
        let cos_max = libm::cosf(p.spread.clamp(0.0, core::f32::consts::PI));
        for _ in 0..n {
            if self.len() >= self.capacity {
                break;
            }
            // Position.
            let (u1, u2, u3) = (self.rand(), self.rand(), self.rand());
            let off = match p.shape {
                SpawnShape::Point => [0.0; 3],
                SpawnShape::Sphere => {
                    let z = 2.0 * u1 - 1.0;
                    let r = sqrtf(1.0 - z * z);
                    let a = u2 * core::f32::consts::TAU;
                    let k = p.size[0] * libm::cbrtf(u3);
                    [r * libm::cosf(a) * k, z * k, r * libm::sinf(a) * k]
                }
                SpawnShape::Box => [(u1 * 2.0 - 1.0) * p.size[0], (u2 * 2.0 - 1.0) * p.size[1], (u3 * 2.0 - 1.0) * p.size[2]],
                SpawnShape::Ring => {
                    let a = u1 * core::f32::consts::TAU;
                    [libm::cosf(a) * p.size[0], (u2 - 0.5) * p.size[1], libm::sinf(a) * p.size[0]]
                }
            };
            // Direction uniformly inside the cone.
            let cz = 1.0 - self.rand() * (1.0 - cos_max);
            let sr = sqrtf((1.0 - cz * cz).max(0.0));
            let phi = self.rand() * core::f32::consts::TAU;
            let (cx, cy) = (sr * libm::cosf(phi), sr * libm::sinf(phi));
            let v = p.speed * (1.0 + (self.rand() * 2.0 - 1.0) * p.speed_var);
            for k in 0..3 {
                self.pos.push(p.origin[k] + off[k]);
                self.vel.push((dir[k] * cz + t1[k] * cx + t2[k] * cy) * v);
            }
            self.age.push(0.0);
            let l = p.life * (1.0 + (self.rand() * 2.0 - 1.0) * p.life_var);
            self.life.push(l.max(1e-3));
        }
    }

    /// Advance `dt` seconds: emit, integrate, age and remove the dead.
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        let p = self.params;
        // Continuous emission with fractional carry.
        self.carry += p.rate.max(0.0) * dt;
        let n = self.carry as usize;
        self.carry -= n as f32;
        self.burst(n);
        let damp = 1.0 - (p.drag * dt).min(1.0);
        let (f, t) = (p.frequency, self.time);
        let mut i = 0;
        while i < self.age.len() {
            self.age[i] += dt;
            if self.age[i] >= self.life[i] {
                // Swap-remove keeps the live set contiguous.
                let last = self.age.len() - 1;
                self.age.swap_remove(i);
                self.life.swap_remove(i);
                for k in (0..3).rev() {
                    self.pos.swap(i * 3 + k, last * 3 + k);
                    self.vel.swap(i * 3 + k, last * 3 + k);
                }
                self.pos.truncate(last * 3);
                self.vel.truncate(last * 3);
                continue;
            }
            let o = i * 3;
            let (x, y, z) = (self.pos[o], self.pos[o + 1], self.pos[o + 2]);
            let mut a = p.gravity;
            if p.turbulence != 0.0 {
                a[0] += p.turbulence * fsin(f * y + t * 1.3 + i as f32 * 0.37);
                a[1] += p.turbulence * fsin(f * z + t * 1.1 + i as f32 * 0.61);
                a[2] += p.turbulence * fsin(f * x + t * 0.9 + i as f32 * 0.17);
            }
            if p.attract != 0.0 {
                let (dx, dy, dz) = (p.attractor[0] - x, p.attractor[1] - y, p.attractor[2] - z);
                let d = sqrtf(dx * dx + dy * dy + dz * dz).max(20.0);
                let s = p.attract / d;
                a[0] += dx * s;
                a[1] += dy * s;
                a[2] += dz * s;
            }
            for (k, ak) in a.iter().enumerate() {
                self.vel[o + k] = (self.vel[o + k] + ak * dt) * damp;
                self.pos[o + k] += self.vel[o + k] * dt;
            }
            i += 1;
        }
    }

    /// Compute per-particle colour and size from normalised age.
    pub fn shade(&mut self, gradient: &Gradient) {
        let n = self.len();
        self.colors.resize(n * 3, 0.0);
        self.sizes.resize(n, 0.0);
        let p = self.params;
        for i in 0..n {
            let u = (self.age[i] / self.life[i]).clamp(0.0, 1.0);
            let s = u * 3.0;
            let k = (s as usize).min(2);
            let f = s - k as f32;
            let (a, b) = (gradient[k], gradient[k + 1]);
            for c in 0..3 {
                self.colors[i * 3 + c] = a[c] + (b[c] - a[c]) * f;
            }
            self.sizes[i] = p.size_start + (p.size_end - p.size_start) * u;
        }
    }

    /// Remove every particle.
    pub fn clear(&mut self) {
        self.pos.clear();
        self.vel.clear();
        self.age.clear();
        self.life.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_ages_and_dies() {
        let mut e = Emitter::new(10_000, 1);
        e.params.rate = 600.0;
        e.params.life = 0.5;
        e.params.life_var = 0.0;
        for _ in 0..30 {
            e.step(1.0 / 60.0);
        }
        // Steady state ≈ rate × life.
        assert!((e.len() as i32 - 300).abs() < 20, "{}", e.len());
        assert_eq!(e.pos.len(), e.len() * 3);
        e.params.rate = 0.0;
        for _ in 0..40 {
            e.step(1.0 / 60.0);
        }
        assert!(e.is_empty());
    }

    #[test]
    fn burst_respects_capacity_cone_and_gravity() {
        let mut e = Emitter::new(100, 2);
        e.params.rate = 0.0;
        e.params.spread = 0.0;
        e.params.direction = [1.0, 0.0, 0.0];
        e.params.speed_var = 0.0;
        e.params.gravity = [0.0, -100.0, 0.0];
        e.params.drag = 0.0;
        e.burst(500);
        assert_eq!(e.len(), 100);
        e.step(0.1);
        // All moved along +x; gravity pulled them down.
        assert!(e.pos.chunks(3).all(|p| p[0] > 19.0 && p[1] < 0.0));
        e.shade(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0]]);
        assert_eq!(e.colors.len(), 300);
        assert!(e.sizes.iter().all(|s| *s <= 1.0 && *s >= 0.0));
    }
}
