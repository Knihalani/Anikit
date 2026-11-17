//! Image-based environments: an equirectangular HDRI as lighting, reflections
//! and background (`"env": {"hdri": "polyhaven:hdri/kloppenheim_06"}`).
//!
//! - The map is kept as a chain of levels, each half the size of the last and
//!   blurred to about two of its own pixels (horizontally widened by
//!   1/cos(latitude) so blur is even on the sphere): a roughness lookup blends
//!   the two levels whose blur brackets the GGX lobe width, the same
//!   split-sum pre-filtering the procedural environments use, with no noise.
//! - The brightest compact region (the sun, or a studio's key lamp) is lifted
//!   out of the map and becomes the environment's key softbox, so it casts
//!   real traced shadows and its energy is not counted twice.
//! - Diffuse lighting is the map's 9-term spherical-harmonic projection
//!   (computed by [`super::env::Env`] from [`EnvMap::lookup`]).

use super::env::Softbox;
use super::math::*;
use std::f32::consts::{PI, TAU};
use std::path::Path;

#[derive(Clone)]
struct Level {
    w: usize,
    h: usize,
    px: Vec<V3>,
    /// Angular blur (radians, about one sigma) of this level.
    sigma: f32,
}

/// What camera rays that miss everything see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Background {
    /// Lighting only: the scene's own backdrop shows.
    Off,
    /// The HDRI, blurred by this angle (radians; 0 = sharp).
    Map(f32),
}

#[derive(Clone)]
pub struct EnvMap {
    levels: Vec<Level>,
    /// Rotation about +Y (sin, cos).
    yaw: (f32, f32),
    /// Exposure multiplier.
    pub gain: f32,
    pub background: Background,
    /// Brightness multiplier for the background only.
    pub bg_gain: f32,
    /// What was extracted as the key (for docs/tests): energy fraction.
    pub key_fraction: f32,
}

impl std::fmt::Debug for EnvMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EnvMap({}×{}, {} levels)", self.levels[0].w, self.levels[0].h, self.levels.len())
    }
}

/// Options for [`EnvMap::load`].
#[derive(Clone, Copy, Debug)]
pub struct Opts {
    /// Degrees about +Y.
    pub rotate: f32,
    /// Stops.
    pub exposure: f32,
    pub background: Background,
    pub bg_exposure: f32,
    /// Lift the brightest compact light out as a traced key.
    pub extract_key: bool,
    /// Longest side kept (texels); 2048 is plenty for reflections at 1080p.
    pub max_width: usize,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { rotate: 0.0, exposure: 0.0, background: Background::Map(0.0), bg_exposure: 0.0, extract_key: true, max_width: 2048 }
    }
}

/// Direction for equirect texel centre `(x, y)` of a `w`×`h` map (no rotation).
#[inline]
fn texel_dir(x: f32, y: f32, w: usize, h: usize) -> V3 {
    let phi = (x / w as f32 - 0.5) * TAU;
    let th = y / h as f32 * PI;
    v3(th.sin() * phi.sin(), th.cos(), -th.sin() * phi.cos())
}

impl EnvMap {
    /// Load an `.hdr` (Radiance) or any image the `image` crate reads (LDR maps are linearised).
    pub fn load(path: &Path, o: Opts) -> Result<(EnvMap, Option<Softbox>), String> {
        let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let hdr = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("hdr") || e.eq_ignore_ascii_case("exr"));
        let rgb = img.to_rgb32f();
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        if w < 8 || h < 4 {
            return Err(format!("{}: too small for an environment map", path.display()));
        }
        let lin = |v: f32| if hdr { v.max(0.0) } else { crate::render::post::srgb_to_linear(v) };
        let mut px: Vec<V3> = rgb.pixels().map(|p| v3(lin(p[0]), lin(p[1]), lin(p[2]))).collect();
        // Cap the size by box-halving.
        let (mut w, mut h) = (w, h);
        while w > o.max_width.max(64) {
            let (nw, nh) = (w / 2, h / 2);
            px = half(&px, w, h);
            w = nw;
            h = nh;
        }
        Ok(Self::from_pixels(px, w, h, o))
    }

    /// Build from linear equirect texels.
    pub fn from_pixels(mut px: Vec<V3>, w: usize, h: usize, o: Opts) -> (EnvMap, Option<Softbox>) {
        let mut key = None;
        let mut key_fraction = 0.0;
        if o.extract_key {
            if let Some((b, f)) = extract_key(&mut px, w, h) {
                key = Some(b);
                key_fraction = f;
            }
        }
        let mut levels = vec![Level { w, h, sigma: 0.5 * TAU / w as f32, px }];
        while levels.last().unwrap().w > 8 {
            let l = levels.last().unwrap();
            let (nw, nh) = (l.w / 2, (l.h / 2).max(2));
            let mut p = half(&l.px, l.w, l.h);
            // Blur to ~2 texels of this level (minus what halving already did).
            let want = 2.0f32;
            let have = l.sigma / (TAU / nw as f32);
            let s = (want * want - have * have).max(0.25).sqrt();
            blur(&mut p, nw, nh, s);
            levels.push(Level { w: nw, h: nh, px: p, sigma: want * TAU / nw as f32 });
        }
        let r = o.rotate.to_radians();
        let gain = 2f32.powf(o.exposure);
        if let Some(k) = &mut key {
            // The key lives in world space: rotate it like the map.
            let rot = |d: V3| v3(d.x * r.cos() + d.z * r.sin(), d.y, -d.x * r.sin() + d.z * r.cos());
            *k = Softbox { c: rot(k.c), u: rot(k.u), v: rot(k.v), radiance: scale(k.radiance, gain), ..k.clone() };
        }
        (EnvMap { levels, yaw: (r.sin(), r.cos()), gain, background: o.background, bg_gain: 2f32.powf(o.bg_exposure), key_fraction }, key)
    }

    #[inline]
    fn to_map(&self, d: V3) -> V3 {
        // Inverse yaw.
        let (s, c) = self.yaw;
        v3(d.x * c - d.z * s, d.y, d.x * s + d.z * c)
    }

    #[inline]
    fn sample(&self, k: usize, d: V3) -> V3 {
        let l = &self.levels[k];
        let phi = d.x.atan2(-d.z);
        let u = phi / TAU + 0.5;
        let v = d.y.clamp(-1.0, 1.0).acos() / PI;
        let x = u * l.w as f32 - 0.5;
        let y = (v * l.h as f32 - 0.5).clamp(0.0, (l.h - 1) as f32);
        let x0 = ffloor(x);
        let fx = x - x0;
        let xi = (x0 as i64).rem_euclid(l.w as i64) as usize;
        let x1 = if xi + 1 == l.w { 0 } else { xi + 1 };
        let y0 = y as usize;
        let y1 = (y0 + 1).min(l.h - 1);
        let fy = y - y0 as f32;
        let r0 = &l.px[y0 * l.w..];
        let r1 = &l.px[y1 * l.w..];
        let a = lerp3(r0[xi], r0[x1], fx);
        let b = lerp3(r1[xi], r1[x1], fx);
        lerp3(a, b, fy)
    }

    /// Radiance from direction `d` blurred by about `sigma` radians (key excluded).
    #[inline]
    pub fn lookup(&self, d: V3, sigma: f32) -> V3 {
        let d = self.to_map(d);
        let n = self.levels.len();
        if sigma <= self.levels[0].sigma {
            return scale(self.sample(0, d), self.gain);
        }
        // Levels' sigma doubles each step: fractional level = log2(sigma / sigma0).
        let lv = (sigma / self.levels[0].sigma).log2().min((n - 1) as f32);
        let k = (lv as usize).min(n - 1);
        let t = lv - k as f32;
        let a = self.sample(k, d);
        let c = if t > 1e-3 && k + 1 < n { lerp3(a, self.sample(k + 1, d), t) } else { a };
        scale(c, self.gain)
    }

    /// Pre-filtered radiance for GGX roughness (same lobe width as the procedural envs).
    #[inline]
    pub fn radiance(&self, d: V3, rough: f32) -> V3 {
        let a = rough * rough;
        self.lookup(d, 1.6 * a + 0.35 * a.sqrt() * rough)
    }

    /// What a camera ray that misses sees, or `None` for the scene's own backdrop.
    #[inline]
    pub fn background(&self, d: V3) -> Option<V3> {
        match self.background {
            Background::Off => None,
            Background::Map(s) => Some(scale(self.lookup(d, s), self.bg_gain)),
        }
    }

    /// Mean radiance (for exposure hints).
    pub fn mean(&self) -> V3 {
        let l = self.levels.last().unwrap();
        let mut s = v3(0.0, 0.0, 0.0);
        let mut wsum = 0.0;
        for y in 0..l.h {
            let wgt = ((y as f32 + 0.5) / l.h as f32 * PI).sin();
            for x in 0..l.w {
                s = madd(s, l.px[y * l.w + x], wgt);
                wsum += wgt;
            }
        }
        scale(s, self.gain / wsum.max(1e-6))
    }
}

/// 2×2 box downsample (odd sizes drop the last row/column).
fn half(px: &[V3], w: usize, h: usize) -> Vec<V3> {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = Vec::with_capacity(nw * nh);
    for y in 0..nh {
        let (y0, y1) = ((2 * y).min(h - 1), (2 * y + 1).min(h - 1));
        for x in 0..nw {
            let (x0, x1) = ((2 * x).min(w - 1), (2 * x + 1).min(w - 1));
            let s = add(add(px[y0 * w + x0], px[y0 * w + x1]), add(px[y1 * w + x0], px[y1 * w + x1]));
            out.push(scale(s, 0.25));
        }
    }
    out
}

/// Gaussian-ish blur (three box passes) of `sigma` texels, wrapping in x and
/// widened by 1/cos(latitude) so the blur is even on the sphere.
fn blur(px: &mut [V3], w: usize, h: usize, sigma: f32) {
    let mut tmp = vec![v3(0.0, 0.0, 0.0); w.max(h)];
    for y in 0..h {
        let lat = ((y as f32 + 0.5) / h as f32 - 0.5) * PI;
        let sx = (sigma / lat.cos().max(1e-3)).min(w as f32 * 0.5);
        let r = box_radius(sx);
        let row = &mut px[y * w..(y + 1) * w];
        for _ in 0..3 {
            box_pass(row, &mut tmp[..w], r, true);
        }
    }
    let r = box_radius(sigma);
    let mut col = vec![v3(0.0, 0.0, 0.0); h];
    for x in 0..w {
        for y in 0..h {
            col[y] = px[y * w + x];
        }
        for _ in 0..3 {
            box_pass(&mut col, &mut tmp[..h], r, false);
        }
        for y in 0..h {
            px[y * w + x] = col[y];
        }
    }
}

/// Box radius whose three passes approximate a Gaussian of `sigma`.
fn box_radius(sigma: f32) -> usize {
    // Variance of a box of width 2r+1 is ((2r+1)²−1)/12; three passes triple it.
    (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) * 0.5).round().max(0.0) as usize
}

fn box_pass(v: &mut [V3], tmp: &mut [V3], r: usize, wrap: bool) {
    let n = v.len();
    if r == 0 || n < 2 {
        return;
    }
    let r = r.min(n - 1);
    let at = |i: isize| -> V3 {
        if wrap {
            v[i.rem_euclid(n as isize) as usize]
        } else {
            v[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let mut s = v3(0.0, 0.0, 0.0);
    for i in -(r as isize)..=(r as isize) {
        s = add(s, at(i));
    }
    let inv = 1.0 / (2 * r + 1) as f32;
    for i in 0..n {
        tmp[i] = scale(s, inv);
        s = add(sub(s, at(i as isize - r as isize)), at(i as isize + r as isize + 1));
    }
    v.copy_from_slice(tmp);
}

/// Find the brightest compact light (sun or key lamp), remove it from the map
/// and return it as a softbox with the same power, plus its share of the
/// map's total energy. `None` when the light is spread out (overcast, interiors).
fn extract_key(px: &mut [V3], w: usize, h: usize) -> Option<(Softbox, f32)> {
    let dw = |y: usize| ((y as f32 + 0.5) / h as f32 * PI).sin() * (PI / h as f32) * (TAU / w as f32);
    let mut total = 0.0f64;
    let (mut peak, mut pi) = (0.0f32, 0usize);
    for y in 0..h {
        let a = dw(y) as f64;
        for x in 0..w {
            let l = luma(px[y * w + x]);
            total += l as f64 * a;
            if l > peak {
                peak = l;
                pi = y * w + x;
            }
        }
    }
    if total <= 0.0 {
        return None;
    }
    let mean = total as f32 / (4.0 * PI);
    // A compact light is far brighter than the average sky.
    if peak < 30.0 * mean {
        return None;
    }
    let pd = texel_dir((pi % w) as f32 + 0.5, (pi / w) as f32 + 0.5, w, h);
    let cos_max = 22f32.to_radians().cos();
    let thr = (peak * 0.02).max(8.0 * mean);
    let mut region: Vec<usize> = Vec::new();
    let (mut e, mut cen, mut col, mut omega) = (0.0f64, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0), 0.0f32);
    let y_lo = ((pi / w) as f32 - 24.0 / 180.0 * h as f32).max(0.0) as usize;
    let y_hi = (((pi / w) as f32 + 24.0 / 180.0 * h as f32) as usize).min(h - 1);
    for y in y_lo..=y_hi {
        let a = dw(y);
        for x in 0..w {
            let i = y * w + x;
            let l = luma(px[i]);
            if l < thr {
                continue;
            }
            let d = texel_dir(x as f32 + 0.5, y as f32 + 0.5, w, h);
            if dot(d, pd) < cos_max {
                continue;
            }
            region.push(i);
            e += (l * a) as f64;
            cen = madd(cen, d, l * a);
            col = madd(col, px[i], a);
            omega += a;
        }
    }
    let frac = (e / total) as f32;
    if frac < 0.12 || region.is_empty() {
        return None;
    }
    // Fill the hole with the surrounding ring's mean.
    let ring_cos = (22f32 + 6.0).to_radians().cos();
    let (mut rs, mut rn) = (v3(0.0, 0.0, 0.0), 0.0f32);
    for y in y_lo.saturating_sub((4.0 / 180.0 * h as f32) as usize)..=(y_hi + (4.0 / 180.0 * h as f32) as usize).min(h - 1) {
        for x in 0..w {
            let i = y * w + x;
            let d = texel_dir(x as f32 + 0.5, y as f32 + 0.5, w, h);
            let c = dot(d, pd);
            if c < cos_max && c > ring_cos && luma(px[i]) < thr {
                rs = add(rs, px[i]);
                rn += 1.0;
            }
        }
    }
    let fill = if rn > 0.0 { scale(rs, 1.0 / rn) } else { v3(0.0, 0.0, 0.0) };
    for &i in &region {
        px[i] = vmin(px[i], fill);
    }
    let dir = norm(cen);
    // A disc of the region's solid angle, at least half a degree across.
    let half_angle = (omega / PI).sqrt().max(0.25f32.to_radians());
    let deg = (2.0 * half_angle.to_degrees()).clamp(0.5, 40.0);
    let mut b = Softbox::new(dir, deg, deg, 0.0, v3(1.0, 1.0, 1.0));
    b.falloff = 0.0;
    b.soft = b.hu * 0.25;
    let sa = b.solid_angle().max(1e-7);
    // Same power as what was removed (in the region's colour).
    let c_l = luma(col).max(1e-9);
    b.radiance = scale(col, (e as f32 / sa) / c_l);
    Some((b, frac))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sky(w: usize, h: usize, sun: Option<(usize, usize, f32)>) -> Vec<V3> {
        let mut px = vec![v3(0.3, 0.4, 0.6); w * h];
        for y in h / 2..h {
            for x in 0..w {
                px[y * w + x] = v3(0.1, 0.09, 0.08);
            }
        }
        if let Some((sx, sy, l)) = sun {
            for dy in 0..2 {
                for dx in 0..2 {
                    px[(sy + dy) * w + sx + dx] = v3(l, l * 0.95, l * 0.85);
                }
            }
        }
        px
    }

    #[test]
    fn sun_becomes_the_key_and_energy_is_kept() {
        let (w, h) = (256, 128);
        let px = sky(w, h, Some((64, 30, 5000.0)));
        let total = |px: &[V3]| -> f32 { (0..h).map(|y| ((y as f32 + 0.5) / h as f32 * PI).sin() * (PI / h as f32) * (TAU / w as f32) * (0..w).map(|x| luma(px[y * w + x])).sum::<f32>()).sum() };
        let before = total(&px);
        let (m, key) = EnvMap::from_pixels(px.clone(), w, h, Opts::default());
        let k = key.expect("a 5000-nit sun is a key");
        assert!(m.key_fraction > 0.5);
        // Its direction is where the sun was.
        let sd = texel_dir(65.0, 31.0, w, h);
        assert!(dot(k.c, sd) > 0.995, "{:?} vs {sd:?}", k.c);
        // Power kept: map remainder + key ≈ original.
        let after = total(&m.levels[0].px) + luma(k.radiance) * k.solid_angle();
        assert!((after / before - 1.0).abs() < 0.05, "{after} vs {before}");
        // Overcast (no sun): no key.
        let (_, none) = EnvMap::from_pixels(sky(w, h, None), w, h, Opts::default());
        assert!(none.is_none());
    }

    #[test]
    fn rough_lookups_are_smooth_and_rotation_turns_the_world() {
        let (w, h) = (128, 64);
        let px = sky(w, h, None);
        let (m, _) = EnvMap::from_pixels(px, w, h, Opts { extract_key: false, ..Default::default() });
        // Sharp lookup sees sky above and ground below; a rough one blends them.
        let up = m.radiance(v3(0.0, 0.3, -1.0), 0.0);
        let dn = m.radiance(v3(0.0, -0.3, -1.0), 0.0);
        assert!(up.z > 0.5 && dn.z < 0.1);
        let hz = m.radiance(norm(v3(0.0, 0.02, -1.0)), 1.0);
        assert!(hz.z > 0.15 && hz.z < 0.5, "{hz:?}");
        // A bright spot at u = 0.75 (+x) moves to −z after 90° of rotation.
        let mut px = vec![v3(0.0, 0.0, 0.0); w * h];
        px[32 * w + 96] = v3(100.0, 100.0, 100.0);
        let (m0, _) = EnvMap::from_pixels(px.clone(), w, h, Opts { extract_key: false, ..Default::default() });
        let (m90, _) = EnvMap::from_pixels(px, w, h, Opts { extract_key: false, rotate: 90.0, ..Default::default() });
        let a = m0.lookup(v3(1.0, 0.0, 0.0), 0.05).x;
        let b = m90.lookup(v3(0.0, 0.0, 1.0), 0.05).x.max(m90.lookup(v3(0.0, 0.0, -1.0), 0.05).x);
        assert!(a > 0.0 && (a - b).abs() / a < 0.2, "{a} {b}");
    }
}
