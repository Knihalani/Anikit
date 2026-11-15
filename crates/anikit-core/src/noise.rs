//! Seeded simplex noise (2D / 3D) and fractal Brownian motion.
//!
//! Smooth, organic randomness for wiggles, drifting particles, procedural
//! motion and "alive" idle animations. Output range ≈ `[-1, 1]`.

use crate::math::{floor, Rng};

const GRAD3: [[f64; 3]; 12] = [
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, -1.0], [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0], [0.0, -1.0, 1.0], [0.0, 1.0, -1.0], [0.0, -1.0, -1.0],
];

/// Seeded permutation-table noise generator.
#[derive(Clone)]
pub struct Noise {
    perm: [u8; 512],
    seed: u32,
}

impl core::fmt::Debug for Noise {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Noise({})", self.seed)
    }
}

impl Noise {
    pub fn new(seed: u32) -> Noise {
        let mut p = [0u8; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        let mut rng = Rng(seed ^ 0x9E37_79B9);
        for i in (1..256).rev() {
            let j = (rng.next_u32() as usize) % (i + 1);
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Noise { perm, seed }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    #[inline]
    fn hash(&self, i: i64) -> usize {
        self.perm[(i & 255) as usize] as usize
    }

    /// 2D simplex noise.
    pub fn noise2(&self, x: f64, y: f64) -> f64 {
        const F2: f64 = 0.366_025_403_784_438_6; // (sqrt(3) - 1) / 2
        const G2: f64 = 0.211_324_865_405_187_1; // (3 - sqrt(3)) / 6
        let s = (x + y) * F2;
        let i = floor(x + s);
        let j = floor(y + s);
        let t = (i + j) * G2;
        let x0 = x - (i - t);
        let y0 = y - (j - t);
        let (i1, j1): (i64, i64) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + G2;
        let y1 = y0 - j1 as f64 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;
        let (ii, jj) = (i as i64, j as i64);
        let corner = |gi: usize, x: f64, y: f64| -> f64 {
            let t = 0.5 - x * x - y * y;
            if t < 0.0 {
                0.0
            } else {
                let g = GRAD3[gi % 12];
                let t2 = t * t;
                t2 * t2 * (g[0] * x + g[1] * y)
            }
        };
        let g0 = self.hash(ii + self.hash(jj) as i64);
        let g1 = self.hash(ii + i1 + self.hash(jj + j1) as i64);
        let g2 = self.hash(ii + 1 + self.hash(jj + 1) as i64);
        70.0 * (corner(g0, x0, y0) + corner(g1, x1, y1) + corner(g2, x2, y2))
    }

    /// 3D simplex noise (use the third axis as time for evolving 2D fields).
    pub fn noise3(&self, x: f64, y: f64, z: f64) -> f64 {
        const F3: f64 = 1.0 / 3.0;
        const G3: f64 = 1.0 / 6.0;
        let s = (x + y + z) * F3;
        let i = floor(x + s);
        let j = floor(y + s);
        let k = floor(z + s);
        let t = (i + j + k) * G3;
        let x0 = x - (i - t);
        let y0 = y - (j - t);
        let z0 = z - (k - t);
        let (i1, j1, k1, i2, j2, k2): (i64, i64, i64, i64, i64, i64) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };
        let offs = [
            (x0, y0, z0, 0i64, 0i64, 0i64),
            (x0 - i1 as f64 + G3, y0 - j1 as f64 + G3, z0 - k1 as f64 + G3, i1, j1, k1),
            (x0 - i2 as f64 + 2.0 * G3, y0 - j2 as f64 + 2.0 * G3, z0 - k2 as f64 + 2.0 * G3, i2, j2, k2),
            (x0 - 1.0 + 3.0 * G3, y0 - 1.0 + 3.0 * G3, z0 - 1.0 + 3.0 * G3, 1, 1, 1),
        ];
        let (ii, jj, kk) = (i as i64, j as i64, k as i64);
        let mut n = 0.0;
        for &(x, y, z, a, b, c) in &offs {
            let t = 0.6 - x * x - y * y - z * z;
            if t > 0.0 {
                let gi = self.hash(ii + a + self.hash(jj + b + self.hash(kk + c) as i64) as i64) % 12;
                let g = GRAD3[gi];
                let t2 = t * t;
                n += t2 * t2 * (g[0] * x + g[1] * y + g[2] * z);
            }
        }
        32.0 * n
    }

    /// Fractal Brownian motion over 2D simplex noise.
    pub fn fbm2(&self, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves.clamp(1, 12) {
            sum += amp * self.noise2(x * freq, y * freq);
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        sum / norm
    }

    /// Fractal Brownian motion over 3D simplex noise.
    pub fn fbm3(&self, x: f64, y: f64, z: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves.clamp(1, 12) {
            sum += amp * self.noise3(x * freq, y * freq, z * freq);
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        sum / norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_smoothness_determinism() {
        let n = Noise::new(42);
        let m = Noise::new(42);
        let o = Noise::new(7);
        let mut lo: f64 = 0.0;
        let mut hi: f64 = 0.0;
        let mut diff = 0.0;
        for i in 0..2000 {
            let x = i as f64 * 0.037;
            let v = n.noise2(x, x * 0.7);
            let w = n.noise3(x, -x, x * 0.3);
            assert!(v.abs() <= 1.0 && w.abs() <= 1.0);
            assert_eq!(v, m.noise2(x, x * 0.7));
            diff += (v - o.noise2(x, x * 0.7)).abs();
            lo = lo.min(v);
            hi = hi.max(v);
            // Continuity: tiny step → tiny change.
            assert!((n.noise2(x + 1e-4, x * 0.7) - v).abs() < 0.01);
        }
        assert!(lo < -0.5 && hi > 0.5);
        assert!(diff > 100.0); // different seeds differ
        assert!(n.fbm2(0.3, 0.4, 5, 2.0, 0.5).abs() <= 1.0);
    }
}
