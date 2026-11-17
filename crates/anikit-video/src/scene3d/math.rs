//! Small vector and transform helpers for the `scene3d` renderer.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[inline]
pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}
#[inline]
pub fn add(a: V3, b: V3) -> V3 {
    v3(a.x + b.x, a.y + b.y, a.z + b.z)
}
#[inline]
pub fn sub(a: V3, b: V3) -> V3 {
    v3(a.x - b.x, a.y - b.y, a.z - b.z)
}
#[inline]
pub fn mul(a: V3, b: V3) -> V3 {
    v3(a.x * b.x, a.y * b.y, a.z * b.z)
}
#[inline]
pub fn scale(a: V3, s: f32) -> V3 {
    v3(a.x * s, a.y * s, a.z * s)
}
#[inline]
pub fn madd(a: V3, b: V3, s: f32) -> V3 {
    v3(a.x + b.x * s, a.y + b.y * s, a.z + b.z * s)
}
#[inline]
pub fn dot(a: V3, b: V3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    v3(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}
#[inline]
pub fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}
#[inline]
pub fn norm(a: V3) -> V3 {
    let l = len(a);
    if l > 1e-20 {
        scale(a, 1.0 / l)
    } else {
        v3(0.0, 1.0, 0.0)
    }
}
#[inline]
pub fn vmin(a: V3, b: V3) -> V3 {
    v3(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z))
}
#[inline]
pub fn vmax(a: V3, b: V3) -> V3 {
    v3(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))
}
#[inline]
pub fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    madd(a, sub(b, a), t)
}
#[inline]
pub fn get(a: V3, k: usize) -> f32 {
    match k {
        0 => a.x,
        1 => a.y,
        _ => a.z,
    }
}
#[inline]
pub fn luma(c: V3) -> f32 {
    0.2126 * c.x + 0.7152 * c.y + 0.0722 * c.z
}
#[inline]
pub fn reflect(d: V3, n: V3) -> V3 {
    madd(d, n, -2.0 * dot(d, n))
}
#[inline]
pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// An orthonormal basis around `n` (Duff et al. 2017).
#[inline]
pub fn basis(n: V3) -> (V3, V3) {
    let s = if n.z >= 0.0 { 1.0 } else { -1.0 };
    let a = -1.0 / (s + n.z);
    let b = n.x * n.y * a;
    (v3(1.0 + s * n.x * n.x * a, s * b, -s * n.x), v3(b, s + n.y * n.y * a, -n.y))
}

/// A rigid transform with uniform scale: `p' = R·(s·p) + t`.
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    /// Rotation columns.
    pub r: [V3; 3],
    pub s: f32,
    pub t: V3,
}

impl Xf {
    pub const IDENTITY: Xf = Xf { r: [v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0)], s: 1.0, t: v3(0.0, 0.0, 0.0) };

    /// Rotation from Euler angles in degrees (applied Y·X·Z: yaw, pitch, roll).
    pub fn new(pos: V3, rot_deg: V3, s: f32) -> Xf {
        let (x, y, z) = (rot_deg.x.to_radians(), rot_deg.y.to_radians(), rot_deg.z.to_radians());
        let rx = [v3(1.0, 0.0, 0.0), v3(0.0, x.cos(), x.sin()), v3(0.0, -x.sin(), x.cos())];
        let ry = [v3(y.cos(), 0.0, -y.sin()), v3(0.0, 1.0, 0.0), v3(y.sin(), 0.0, y.cos())];
        let rz = [v3(z.cos(), z.sin(), 0.0), v3(-z.sin(), z.cos(), 0.0), v3(0.0, 0.0, 1.0)];
        let m = mat_mul(&ry, &mat_mul(&rx, &rz));
        Xf { r: m, s, t: pos }
    }
    #[inline]
    pub fn rot(&self, v: V3) -> V3 {
        add(add(scale(self.r[0], v.x), scale(self.r[1], v.y)), scale(self.r[2], v.z))
    }
    #[inline]
    pub fn rot_inv(&self, v: V3) -> V3 {
        v3(dot(self.r[0], v), dot(self.r[1], v), dot(self.r[2], v))
    }
    #[inline]
    pub fn point(&self, p: V3) -> V3 {
        add(self.rot(scale(p, self.s)), self.t)
    }
    #[inline]
    pub fn point_inv(&self, p: V3) -> V3 {
        scale(self.rot_inv(sub(p, self.t)), 1.0 / self.s)
    }
}

pub fn mat_mul(a: &[V3; 3], b: &[V3; 3]) -> [V3; 3] {
    let ap = |v: V3| add(add(scale(a[0], v.x), scale(a[1], v.y)), scale(a[2], v.z));
    [ap(b[0]), ap(b[1]), ap(b[2])]
}

/// Fractional part via truncation (std's `floor` is a software routine
/// without SSE4.1).
#[inline(always)]
pub fn fract(x: f32) -> f32 {
    let t = x - (x as i32) as f32;
    if t < 0.0 {
        t + 1.0
    } else {
        t
    }
}

/// A small, fast hash for per-pixel sample decorrelation.
#[inline]
pub fn hash2(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(0x8da6b343) ^ y.wrapping_mul(0xd8163841);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a2d39);
    h ^ (h >> 15)
}

#[inline]
pub fn u01(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Point `i` of the R2 low-discrepancy sequence (Roberts), offset by `rot`.
#[inline]
pub fn r2(i: u32, rot: (f32, f32)) -> (f32, f32) {
    const A1: f32 = 0.754_877_7;
    const A2: f32 = 0.569_840_3;
    let a = rot.0 + A1 * i as f32;
    let b = rot.1 + A2 * i as f32;
    (fract(a), fract(b))
}

/// Cosine-weighted hemisphere direction around `n`.
#[inline]
pub fn cosine_dir(n: V3, u: f32, v: f32) -> V3 {
    let (t, b) = basis(n);
    let r = u.sqrt();
    let phi = std::f32::consts::TAU * v;
    let (x, y) = (r * phi.cos(), r * phi.sin());
    let z = (1.0 - u).max(0.0).sqrt();
    add(add(scale(t, x), scale(b, y)), scale(n, z))
}
