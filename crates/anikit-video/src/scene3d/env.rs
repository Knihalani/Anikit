//! Procedural HDR studio environments.
//!
//! An environment is a dark gradient with a soft horizon band plus a set of
//! rectangular softboxes (and strips) placed on the sky sphere. It is
//! evaluated analytically, so mirror reflections stay razor sharp at any
//! resolution, and it is pre-filtered analytically too: a reflection with
//! roughness `r` widens every softbox edge by the GGX lobe's angular width,
//! which keeps each box's integral (its energy) while blurring it, the
//! split-sum "pre-filtered environment" without any mip maps or noise.
//!
//! One softbox can be marked as the key: it is left out of the diffuse
//! irradiance (spherical harmonics) and lit explicitly with traced soft
//! shadows instead, so objects cast real shadows from it.

use super::math::*;

#[derive(Clone, Debug)]
pub struct Softbox {
    /// Unit direction to the box centre, and the box's tangent axes.
    pub c: V3,
    pub u: V3,
    pub v: V3,
    /// Half extents in tangent-plane units (tan of the half angle).
    pub hu: f32,
    pub hv: f32,
    /// Edge softness (tangent-plane units).
    pub soft: f32,
    pub radiance: V3,
    /// Brightness falloff from the box centre to its edge along `v`
    /// (0 = flat, 0.5 = the edge is half as bright), like a real diffuser.
    pub falloff: f32,
}

impl Softbox {
    /// A softbox centred on direction `dir`, `w` × `h` degrees across, rolled
    /// by `roll` degrees around its axis.
    pub fn new(dir: V3, w_deg: f32, h_deg: f32, roll_deg: f32, radiance: V3) -> Softbox {
        let c = norm(dir);
        let up = if c.y.abs() > 0.95 { v3(0.0, 0.0, -1.0) } else { v3(0.0, 1.0, 0.0) };
        let u0 = norm(cross(up, c));
        let v0 = cross(c, u0);
        let r = roll_deg.to_radians();
        let u = add(scale(u0, r.cos()), scale(v0, r.sin()));
        let v = add(scale(v0, r.cos()), scale(u0, -r.sin()));
        Softbox { c, u, v, hu: (w_deg * 0.5).to_radians().tan(), hv: (h_deg * 0.5).to_radians().tan(), soft: 0.012, radiance, falloff: 0.25 }
    }

    /// Radiance towards direction `d` with extra edge blur `blur`.
    #[inline]
    pub fn eval(&self, d: V3, blur: f32) -> V3 {
        let k = dot(d, self.c);
        if k <= 0.05 {
            return v3(0.0, 0.0, 0.0);
        }
        let x = dot(d, self.u) / k;
        let y = dot(d, self.v) / k;
        let s = self.soft + blur;
        let lim = 1.0 + s * 2.0;
        if x.abs() > self.hu * lim + s || y.abs() > self.hv * lim + s {
            return v3(0.0, 0.0, 0.0);
        }
        let m = boxf(x, self.hu, s) * boxf(y, self.hv, s);
        if m <= 0.0 {
            return v3(0.0, 0.0, 0.0);
        }
        let g = 1.0 - self.falloff * (y / self.hv.max(1e-4)).clamp(-1.0, 1.0).powi(2);
        scale(self.radiance, m * g)
    }

    /// A point on the box (tangent coordinates in -1..1) as a direction.
    #[inline]
    pub fn dir_at(&self, a: f32, b: f32) -> V3 {
        norm(add(add(self.c, scale(self.u, a * self.hu)), scale(self.v, b * self.hv)))
    }

    /// Approximate solid angle (steradians).
    pub fn solid_angle(&self) -> f32 {
        let n = 32;
        let mut s = 0.0;
        for i in 0..n {
            for j in 0..n {
                let x = ((i as f32 + 0.5) / n as f32 * 2.0 - 1.0) * self.hu;
                let y = ((j as f32 + 0.5) / n as f32 * 2.0 - 1.0) * self.hv;
                s += 1.0 / (1.0 + x * x + y * y).powf(1.5);
            }
        }
        s * 4.0 * self.hu * self.hv / (n * n) as f32
    }
}

/// A box of half width `h` convolved with a smooth kernel of half width `s`
/// (its integral stays `2h` however wide the blur).
#[inline]
fn boxf(x: f32, h: f32, s: f32) -> f32 {
    let ax = x.abs();
    if s <= 1e-6 {
        return if ax < h { 1.0 } else { 0.0 };
    }
    smoothstep(-h - s, -h + s, ax) - smoothstep(h - s, h + s, ax)
}

#[derive(Clone, Debug)]
pub struct Env {
    pub boxes: Vec<Softbox>,
    /// Index of the key softbox (lit with traced shadows), if any.
    pub key: Option<usize>,
    pub zenith: V3,
    pub horizon: V3,
    pub ground: V3,
    /// Horizon band brightness (the chrome "horizon line").
    pub band: V3,
    /// Diffuse irradiance without the key, as 9 SH coefficients per channel.
    sh: [V3; 9],
    pub key_solid_angle: f32,
}

impl Env {
    pub fn new(boxes: Vec<Softbox>, key: Option<usize>, zenith: V3, horizon: V3, ground: V3, band: V3) -> Env {
        let mut e = Env { boxes, key, zenith, horizon, ground, band, sh: [v3(0.0, 0.0, 0.0); 9], key_solid_angle: 0.0 };
        e.key_solid_angle = key.map(|k| e.boxes[k].solid_angle()).unwrap_or(0.0);
        e.project_sh();
        e
    }

    /// Background gradient (no softboxes).
    #[inline]
    fn sky(&self, d: V3, blur: f32) -> V3 {
        let y = d.y;
        let base = if y >= 0.0 { lerp3(self.horizon, self.zenith, smoothstep(0.0, 0.7, y)) } else { lerp3(self.horizon, self.ground, smoothstep(0.0, 0.25, -y)) };
        let w = 0.035 + blur;
        let b = (-(y - 0.02) * (y - 0.02) / (w * w)).exp() * (0.035 / w).min(1.0);
        madd(base, self.band, b)
    }

    /// Radiance towards `d`, pre-filtered for GGX roughness `rough`.
    #[inline]
    pub fn radiance(&self, d: V3, rough: f32) -> V3 {
        let a = rough * rough;
        let blur = 1.6 * a + 0.35 * a.sqrt() * rough;
        let mut c = self.sky(d, blur);
        for b in &self.boxes {
            c = add(c, b.eval(d, blur));
        }
        c
    }

    fn radiance_nokey(&self, d: V3) -> V3 {
        let mut c = self.sky(d, 0.0);
        for (i, b) in self.boxes.iter().enumerate() {
            if Some(i) != self.key {
                c = add(c, b.eval(d, 0.0));
            }
        }
        c
    }

    fn project_sh(&mut self) {
        let (nt, np) = (128, 256);
        let mut sh = [v3(0.0, 0.0, 0.0); 9];
        for i in 0..nt {
            let th = (i as f32 + 0.5) / nt as f32 * std::f32::consts::PI;
            let dw = th.sin() * (std::f32::consts::PI / nt as f32) * (std::f32::consts::TAU / np as f32);
            for j in 0..np {
                let ph = (j as f32 + 0.5) / np as f32 * std::f32::consts::TAU;
                let d = v3(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
                let l = self.radiance_nokey(d);
                let y = sh_basis(d);
                for k in 0..9 {
                    sh[k] = madd(sh[k], l, y[k] * dw);
                }
            }
        }
        // Convolve with the clamped cosine (Ramamoorthi & Hanrahan), so
        // evaluating gives irradiance / π (outgoing radiance for albedo 1).
        let a = [1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25];
        for k in 0..9 {
            self.sh[k] = scale(sh[k], a[k]);
        }
    }

    /// Diffuse radiance (irradiance / π) for normal `n`, key excluded.
    #[inline]
    pub fn diffuse(&self, n: V3) -> V3 {
        let y = sh_basis(n);
        let mut c = v3(0.0, 0.0, 0.0);
        for k in 0..9 {
            c = madd(c, self.sh[k], y[k]);
        }
        vmax(c, v3(0.0, 0.0, 0.0))
    }

    pub fn key_box(&self) -> Option<&Softbox> {
        self.key.map(|k| &self.boxes[k])
    }

    /// Built-in presets: `studio`, `night`, `neon`, `sunset`.
    pub fn preset(name: &str, accent: V3, accent2: V3) -> Option<Env> {
        let w = |s: f32| v3(s, s, s);
        Some(match name {
            "studio" => {
                let mut key = Softbox::new(v3(-0.25, 1.0, 0.35), 70.0, 46.0, 0.0, w(4.2));
                key.falloff = 0.35;
                key.soft = 0.03;
                let mut front = Softbox::new(v3(0.05, 0.42, 1.0), 120.0, 26.0, 0.0, w(1.1));
                front.falloff = 0.6;
                front.soft = 0.05;
                let mut strip_l = Softbox::new(v3(-1.0, 0.12, 0.28), 7.0, 80.0, 0.0, w(5.5));
                strip_l.falloff = 0.15;
                let mut strip_r = Softbox::new(v3(1.0, 0.18, 0.05), 6.0, 84.0, 0.0, w(4.5));
                strip_r.falloff = 0.15;
                let rim_a = Softbox::new(v3(-0.75, 0.22, -1.0), 10.0, 60.0, 0.0, scale(accent, 3.0));
                let rim_b = Softbox::new(v3(0.8, 0.3, -1.0), 10.0, 60.0, 0.0, scale(accent2, 2.4));
                let mut top = Softbox::new(v3(0.35, 1.0, -0.4), 16.0, 90.0, 35.0, w(2.6));
                top.soft = 0.02;
                Env::new(vec![key, front, strip_l, strip_r, rim_a, rim_b, top], Some(0), w(0.004), w(0.03), w(0.008), w(0.12))
            }
            "night" => {
                let key = Softbox::new(v3(0.2, 1.0, 0.25), 50.0, 30.0, 0.0, v3(1.4, 1.6, 2.4));
                let strip = Softbox::new(v3(-1.0, 0.15, 0.2), 5.0, 70.0, 0.0, v3(2.0, 2.4, 3.6));
                let rim = Softbox::new(v3(0.8, 0.25, -1.0), 8.0, 60.0, 0.0, scale(accent2, 2.4));
                Env::new(vec![key, strip, rim], Some(0), v3(0.002, 0.003, 0.008), v3(0.01, 0.014, 0.03), v3(0.004, 0.005, 0.01), v3(0.02, 0.03, 0.07))
            }
            "neon" => {
                let key = Softbox::new(v3(0.0, 1.0, 0.3), 60.0, 34.0, 0.0, w(1.4));
                let a = Softbox::new(v3(-1.0, 0.15, 0.1), 6.0, 90.0, 0.0, scale(accent, 6.0));
                let b = Softbox::new(v3(1.0, 0.15, 0.1), 6.0, 90.0, 0.0, scale(accent2, 6.0));
                let c = Softbox::new(v3(0.0, 0.3, -1.0), 90.0, 4.0, 0.0, scale(add(accent, accent2), 2.0));
                let f = Softbox::new(v3(0.0, 0.4, 1.0), 100.0, 20.0, 0.0, w(0.5));
                Env::new(vec![key, a, b, c, f], Some(0), w(0.002), v3(0.02, 0.01, 0.03), w(0.004), v3(0.05, 0.02, 0.08))
            }
            "sunset" => {
                let mut sun = Softbox::new(v3(-0.9, 0.22, -0.5), 9.0, 9.0, 0.0, v3(40.0, 22.0, 9.0));
                sun.soft = 0.03;
                sun.falloff = 0.0;
                let sky = Softbox::new(v3(0.3, 1.0, 0.4), 100.0, 60.0, 0.0, v3(0.5, 0.55, 0.8));
                let warm = Softbox::new(v3(-0.6, 0.12, 0.8), 80.0, 18.0, 0.0, v3(1.6, 0.8, 0.4));
                Env::new(vec![sun, sky, warm], Some(0), v3(0.06, 0.08, 0.16), v3(0.5, 0.28, 0.14), v3(0.04, 0.03, 0.03), v3(0.6, 0.3, 0.1))
            }
            _ => return None,
        })
    }
}

#[inline]
fn sh_basis(d: V3) -> [f32; 9] {
    let (x, y, z) = (d.x, d.y, d.z);
    [
        0.282_095,
        0.488_603 * y,
        0.488_603 * z,
        0.488_603 * x,
        1.092_548 * x * y,
        1.092_548 * y * z,
        0.315_392 * (3.0 * z * z - 1.0),
        1.092_548 * x * z,
        0.546_274 * (x * x - y * y),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_keeps_energy() {
        // Integral of a softbox over its tangent plane stays the same when blurred.
        let b = Softbox::new(v3(0.0, 0.0, 1.0), 20.0, 20.0, 0.0, v3(1.0, 1.0, 1.0));
        let integ = |blur: f32| {
            let mut s = 0.0;
            let n = 400;
            for i in 0..n {
                for j in 0..n {
                    let x = (i as f32 / n as f32 - 0.5) * 1.6;
                    let y = (j as f32 / n as f32 - 0.5) * 1.6;
                    s += b.eval(norm(v3(x, y, 1.0)), blur).x * (1.0 + 0.25 * (y / b.hv).clamp(-1.0, 1.0).powi(2) * 0.0);
                }
            }
            s
        };
        let (a, c) = (integ(0.0), integ(0.08));
        assert!((a - c).abs() / a < 0.08, "{a} vs {c}");
    }

    #[test]
    fn diffuse_is_positive_and_brighter_up() {
        let e = Env::preset("studio", v3(0.5, 0.3, 1.0), v3(0.1, 0.8, 0.9)).unwrap();
        let up = e.diffuse(v3(0.0, 1.0, 0.0));
        let down = e.diffuse(v3(0.0, -1.0, 0.0));
        assert!(luma(up) > luma(down));
        assert!(e.key_solid_angle > 0.1);
    }
}
