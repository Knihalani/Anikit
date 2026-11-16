//! A physically based Schwarzschild black-hole renderer.
//!
//! Units: G = c = M = 1 (lengths are in black-hole masses). See
//! `docs/black-hole.md` for the physics and citations.
//!
//! Architecture (fast and exact):
//!
//! * **Geodesic table.** Every photon orbit lies in a plane through the hole
//!   and, for a given observer, depends on one number: the angle ψ between
//!   the viewing direction and the direction to the hole (measured in the
//!   observer's own frame, so relativistic aberration of an infalling
//!   observer is built in). Once per distance change we integrate the Binet
//!   equation `u'' + u = 3u²` (`u = 1/r`) for `ROWS` values of ψ with RK4 and
//!   store `u(φ)` on a fixed φ grid, plus the asymptotic escape angle φ∞.
//! * **Per-pixel geometry** (on camera change). A pixel's orbit plane is
//!   spanned by the observer direction ĉ and a unit vector ŝ; its disk-plane
//!   crossings are at φ0 + kπ, which are fixed column offsets in the table.
//!   Crossings k = 0, 1, 2 give the direct image, the lensed image that arcs
//!   over and under the shadow, and the photon ring. Each crossing that lands
//!   on the disk becomes a *sample* with its redshift-coloured base radiance.
//! * **Per frame** only the animated parts run: a flowing disk-density
//!   texture in (ln r, azimuth) built from sheared turbulence and orbiting
//!   hot spots, one bilinear lookup per sample, lensed star splats, bloom,
//!   ACES-style tone mapping and grain.
use crate::math::{floorf, sqrtf};
use crate::simd::V4;
use alloc::vec;
use alloc::vec::Vec;
use core::f32::consts::PI;

const TAU: f32 = 2.0 * PI;
/// Table rows (viewing angles ψ).
pub const ROWS: usize = 1536;
/// Table columns per half turn of φ (the disk is crossed every π).
pub const COLS_PER_PI: usize = 128;
/// Table columns (φ from 0 to 4π).
pub const COLS: usize = COLS_PER_PI * 4 + 2;
const H_PHI: f32 = PI / COLS_PER_PI as f32;
/// Value stored after a ray has been captured (r = 0.5M: never the disk).
const U_CAPTURED: f32 = 2.0;
/// Value stored after a ray has escaped (negative: never the disk).
const U_ESCAPED: f32 = -0.25;
/// Disk texture: radial texels (ln r) and azimuthal texels.
pub const TEX_X: usize = 256;
pub const TEX_A: usize = 1024;
const TEX_R0: f32 = 4.5;
const BB_N: usize = 1024;
const BB_T0: f32 = 600.0;
const BB_T1: f32 = 100_000.0;
const PANO_W: usize = 1024;
const PANO_H: usize = 512;
const SRGB_N: usize = 4096;
/// Orbit-plane angle bins.
const TH_BINS: usize = 16384;

/// Innermost stable circular orbit of a Schwarzschild hole.
pub const ISCO: f32 = 6.0;
/// Critical impact parameter: the photon sphere's shadow, b = 3√3 M.
pub const B_CRIT: f32 = 5.196_152;

/// Per-frame render parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Observer radius in M.
    pub distance: f32,
    /// Angle between the disk normal and the observer, degrees (90 = edge-on).
    pub inclination: f32,
    /// Vertical field of view, degrees.
    pub fov: f32,
    /// Seconds.
    pub time: f32,
    pub exposure: f32,
    pub bloom: f32,
    pub grain: f32,
    /// Hot-spot strength (0 = none).
    pub hot_spots: f32,
    /// Jet glow strength (0 = none).
    pub jet: f32,
    /// Camera orbit angle around the disk axis, degrees.
    pub azimuth: f32,
    /// Look away from the hole, degrees (0 = at it, 180 = straight back).
    pub look: f32,
    /// Camera roll, degrees.
    pub roll: f32,
    /// Observer velocity as a fraction of free fall from rest at infinity
    /// (0 = static, 1 = Painlevé–Gullstrand raindrop). Inside the horizon the
    /// observer is always a raindrop.
    pub fall: f32,
    /// 0 = physical (Doppler colour, g⁴ beaming), 1 = cinematic.
    pub mode: u32,
    /// Peak disk temperature, kelvin.
    pub temperature: f32,
    /// Outer disk radius, M.
    pub disk_outer: f32,
    /// Star brightness.
    pub stars: f32,
    /// Disk flow speed multiplier.
    pub speed: f32,
    /// Background nebula brightness.
    pub nebula: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            distance: 30.0,
            inclination: 80.0,
            fov: 40.0,
            time: 0.0,
            exposure: 1.0,
            bloom: 1.0,
            grain: 1.0,
            hot_spots: 1.0,
            jet: 0.0,
            azimuth: 0.0,
            look: 0.0,
            roll: 0.0,
            fall: 0.0,
            mode: 0,
            temperature: 7000.0,
            disk_outer: 22.0,
            stars: 1.0,
            speed: 1.0,
            nebula: 1.0,
        }
    }
}

/// Counters from the last frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Disk samples per image order (direct, secondary, photon ring).
    pub samples: [u32; 3],
    /// Star images splatted.
    pub stars: u32,
    /// 1 if the geodesic table was rebuilt this frame.
    pub table_built: u32,
    /// 1 if the per-pixel geometry was rebuilt this frame.
    pub geometry_built: u32,
    /// Angular radius of the shadow edge (radians, observer frame), or -1.
    pub shadow: f32,
}

/// Disk samples of one image order, 16 bytes each.
#[derive(Default)]
struct Layer {
    /// Pixel index.
    pix: Vec<u32>,
    /// Disk-texture coordinate: x in 8.8 fixed point (low 16 bits), azimuth
    /// in 10.6 fixed point (high 16 bits).
    txa: Vec<u32>,
    /// Colour-table index (low 16 bits) and opacity × 65535 (high 16 bits).
    ca: Vec<u32>,
    /// Radiance multiplier.
    gain: Vec<f32>,
}

impl Layer {
    fn clear(&mut self) {
        self.pix.clear();
        self.txa.clear();
        self.ca.clear();
        self.gain.clear();
    }
}

struct Star {
    dir: [f32; 3],
    flux: f32,
    temp: f32,
}

/// One frame's observer basis.
#[derive(Clone, Copy)]
struct Camera {
    c: [f32; 3],
    fwd: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    /// Basis of the plane perpendicular to ĉ (the camera before `look`).
    pr: [f32; 3],
    pu: [f32; 3],
    tan_half: f32,
}

/// The renderer: owns its tables and pixel buffers.
pub struct BlackHole {
    pub w: usize,
    pub h: usize,
    bw: usize,
    bh: usize,
    // Geodesic table for the current observer.
    table: Vec<f32>,
    esc: Vec<f32>,
    imp: Vec<f32>,
    dobs: Vec<f32>,
    psi_max: f32,
    first_escape: usize,
    table_key: [f32; 3],
    geom_key: Option<(Params, usize, usize)>,
    // Per-pixel viewing angle ψ and orbit-plane angle θ (bins), cached per
    // size / field of view / look.
    scr_psi: Vec<f32>,
    scr_th: Vec<u16>,
    scr_key: Option<(usize, usize, f32, f32)>,
    scr_psi_max: f32,
    // Low-resolution bloom accumulators (filled by the tone-map pass).
    ar: Vec<f32>,
    ag: Vec<f32>,
    ab: Vec<f32>,
    at: Vec<f32>,
    // Static resources.
    noise: Vec<f32>,
    pano: Vec<[f32; 3]>,
    bb: Vec<[f32; 3]>,
    srgb: Vec<u8>,
    stars: Vec<Star>,
    // Per-frame disk texture.
    disk: Vec<f32>,
    layers: [Layer; 3],
    // Low-resolution sky lookup coordinates (panorama u, v, gain).
    sky_u: Vec<f32>,
    sky_v: Vec<f32>,
    sky_k: Vec<f32>,
    // Full-resolution HDR planes and transmittance.
    hr: Vec<f32>,
    hg: Vec<f32>,
    hb: Vec<f32>,
    trans: Vec<f32>,
    // Low-resolution planes for bloom.
    lr: Vec<f32>,
    lg: Vec<f32>,
    lb: Vec<f32>,
    lt: Vec<f32>,
    tmp: Vec<f32>,
    acc: Vec<f32>,
    grain_tab: Vec<f32>,
    wide: [Vec<f32>; 3],
    up_x0: Vec<u32>,
    up_fx: Vec<f32>,
    row_a: Vec<f32>,
    row_b: Vec<f32>,
    /// RGBA8 output.
    pub out: Vec<u8>,
    pub stats: Stats,
    frame: u32,
    cam: Option<Camera>,
    /// Eye adaptation: exposure gain that compensates the redshifted sky
    /// seen looking back out while falling in.
    adapt: f32,
}

// ---------------------------------------------------------------------------
// Small math helpers (f32, branch-light)
// ---------------------------------------------------------------------------

#[inline(always)]
fn atan_unit(x: f32) -> f32 {
    // |x| <= 1, max error ~1e-5 rad.
    let x2 = x * x;
    x * (0.999_866 + x2 * (-0.330_299_5 + x2 * (0.180_141 + x2 * (-0.085_133 + x2 * 0.020_835_1))))
}

/// Fast `atan2` (error ~1e-5 rad).
#[inline(always)]
pub fn atan2f(y: f32, x: f32) -> f32 {
    let ax = x.abs();
    let ay = y.abs();
    let mx = ax.max(ay);
    if mx == 0.0 {
        return 0.0;
    }
    let a = atan_unit(ax.min(ay) / mx);
    let a = if ay > ax { PI / 2.0 - a } else { a };
    let a = if x < 0.0 { PI - a } else { a };
    if y < 0.0 {
        -a
    } else {
        a
    }
}

#[inline(always)]
fn acosf(x: f32) -> f32 {
    let x = x.clamp(-1.0, 1.0);
    atan2f(sqrtf((1.0 - x * x).max(0.0)), x)
}

/// Fast (sin x, cos x), error ~2e-6.
#[inline(always)]
fn sincosf(x: f32) -> (f32, f32) {
    // Reduce to [-π, π], then to [-π/2, π/2] by reflection.
    let k = floorf(x * (1.0 / TAU) + 0.5);
    let x = x - k * TAU;
    let s = |x: f32| {
        let x2 = x * x;
        x * (1.0 + x2 * (-1.0 / 6.0 + x2 * (1.0 / 120.0 + x2 * (-1.0 / 5040.0 + x2 * (1.0 / 362_880.0 - x2 * (1.0 / 39_916_800.0))))))
    };
    let refl = |x: f32| if x > PI / 2.0 { PI - x } else if x < -PI / 2.0 { -PI - x } else { x };
    let c = x + PI / 2.0;
    let c = if c > PI { c - TAU } else { c };
    (s(refl(x)), s(refl(c)))
}

/// log2 for positive normal floats (error < 1e-5).
#[inline(always)]
fn fast_log2(x: f32) -> f32 {
    let b = x.to_bits();
    let e = ((b >> 23) & 0xff) as i32 - 127;
    let m = f32::from_bits((b & 0x007f_ffff) | 0x3f80_0000);
    // log2(m), m in [1, 2): atanh series in t = (m − 1)/(m + 1), |t| <= 1/3.
    let t = (m - 1.0) / (m + 1.0);
    let t2 = t * t;
    let p = t * (2.885_39 + t2 * (0.961_797 + t2 * (0.577_078 + t2 * 0.412_199)));
    e as f32 + p
}

/// e^-x for x >= 0 (cheap, smooth, ~1e-3 accurate).
#[inline(always)]
fn exp_neg(x: f32) -> f32 {
    let t = (1.0 - x * (1.0 / 16.0)).max(0.0);
    let t = t * t;
    let t = t * t;
    let t = t * t;
    t * t
}

#[inline(always)]
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline(always)]
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = sqrtf(dot(a, a)).max(1e-20);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn hashf(a: u32, b: u32, c: u32) -> f32 {
    (hash(a ^ hash(b ^ hash(c))) >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Periodic 2D value noise on an `nx × ny` lattice (wraps in both axes).
fn value_noise(x: f32, y: f32, nx: u32, ny: u32, seed: u32) -> f32 {
    let xf = floorf(x);
    let yf = floorf(y);
    let fx = x - xf;
    let fy = y - yf;
    let (x0, y0) = ((xf as i32).rem_euclid(nx as i32) as u32, (yf as i32).rem_euclid(ny as i32) as u32);
    let (x1, y1) = ((x0 + 1) % nx, (y0 + 1) % ny);
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = hashf(x0, y0, seed);
    let b = hashf(x1, y0, seed);
    let c = hashf(x0, y1, seed);
    let d = hashf(x1, y1, seed);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sy
}

/// Periodic fbm, 0..1.
fn fbm(x: f32, y: f32, nx: u32, ny: u32, oct: u32, seed: u32) -> f32 {
    let mut s = 0.0;
    let mut amp = 0.5;
    let mut tot = 0.0;
    let mut f = 1u32;
    for o in 0..oct {
        s += amp * value_noise(x * f as f32, y * f as f32, nx * f, ny * f, seed.wrapping_add(o * 977));
        tot += amp;
        amp *= 0.5;
        f *= 2;
    }
    s / tot
}

// ---------------------------------------------------------------------------
// Physics
// ---------------------------------------------------------------------------

/// Initial conditions for a ray seen at angle `psi` from the hole's direction
/// by an observer at `u0 = 1/r`, moving radially inward at `fall` × the
/// free-fall speed √(2M/r) relative to static observers (always a raindrop
/// inside the horizon). Returns `(du/dφ, b, D)`, where `b` is the impact
/// parameter L/E and `D = ε_obs/E` converts Killing energy to the energy the
/// observer measures; `None` if the photon has non-positive Killing energy
/// (it cannot have come from outside the hole).
pub fn ray_init(psi: f32, u0: f32, fall: f32) -> Option<(f32, f32, f32)> {
    let (s, c) = (libm::sinf(psi).max(1e-6), libm::cosf(psi));
    let beta = sqrtf(2.0 * u0);
    if u0 >= 0.5 || fall >= 0.9999 {
        // Painlevé–Gullstrand raindrop: E = ε(1 − β cos ψ), L = ε r sin ψ.
        let e = 1.0 - beta * c;
        if e <= 1e-6 {
            return None;
        }
        Some((u0 * (c - beta) / s, s / (u0 * e), 1.0 / e))
    } else {
        // Static frame angle α via aberration, then the static-observer ray.
        let v = (fall * beta).clamp(0.0, 0.9999);
        let den = 1.0 - v * c;
        let ca = (c - v) / den;
        let sa = sqrtf(1.0 - v * v) * s / den;
        let lapse = sqrtf(1.0 - 2.0 * u0);
        Some((u0 * lapse * ca / sa.max(1e-7), sa / (u0 * lapse), sqrtf(1.0 - v * v) / (den * lapse)))
    }
}

/// Redshift factor g = ν_obs/ν_emit for gas on a circular Keplerian orbit at
/// `r` (r ≥ 3M), seen in a photon with Killing-energy impact parameter
/// `lz` = L_z/E (positive = co-rotating), by an observer with energy factor
/// `d_obs` (1 for a static observer at infinity).
#[inline(always)]
pub fn redshift(r: f32, lz: f32, d_obs: f32) -> f32 {
    let u = 1.0 / r;
    let omega = u * sqrtf(u);
    d_obs * sqrtf((1.0 - 3.0 * u).max(1e-6)) / (1.0 - omega * lz).max(1e-3)
}

/// Page–Thorne (Novikov–Thorne) flux profile for a Schwarzschild hole, up to
/// a constant: zero at the ISCO, peaking near r ≈ 9.55 M.
pub fn page_thorne_flux(r: f64) -> f64 {
    if r <= 6.0 {
        return 0.0;
    }
    let s3 = crate::math::sqrt(3.0);
    let s6 = crate::math::sqrt(6.0);
    let sr = crate::math::sqrt(r);
    let l = crate::math::ln((sr + s3) * (s6 - s3) / ((sr - s3) * (s6 + s3)));
    let br = 1.0 - crate::math::sqrt(6.0 / r) + 0.5 * s3 / sr * l;
    br / (r * r * (r - 3.0))
}

/// CIE 1931 colour matching (Wyman, Sloan & Shirley 2013 multi-lobe fit).
fn cie(l: f64) -> (f64, f64, f64) {
    let g = |x: f64, mu: f64, s1: f64, s2: f64| {
        let t = (x - mu) / if x < mu { s1 } else { s2 };
        crate::math::exp(-0.5 * t * t)
    };
    let x = 1.056 * g(l, 599.8, 37.9, 31.0) + 0.362 * g(l, 442.0, 16.0, 26.7) - 0.065 * g(l, 501.1, 20.4, 26.2);
    let y = 0.821 * g(l, 568.8, 46.9, 40.5) + 0.286 * g(l, 530.9, 16.3, 31.1);
    let z = 1.217 * g(l, 437.0, 11.8, 36.0) + 0.681 * g(l, 459.0, 26.0, 13.8);
    (x, y, z)
}

/// Linear-sRGB radiance of a blackbody at `t` kelvin, scaled so a 6500 K
/// body has luminance 1. Out-of-gamut (negative) channels are desaturated.
pub fn blackbody(t: f64) -> [f32; 3] {
    fn raw(t: f64) -> [f64; 3] {
        let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
        let mut l = 380.0;
        while l <= 780.0 {
            let lm = l * 1e-9;
            let p = 1.0 / (lm * lm * lm * lm * lm * (crate::math::exp((1.438_776_9e-2 / (lm * t)).min(700.0)) - 1.0));
            let (cx, cy, cz) = cie(l);
            x += p * cx;
            y += p * cy;
            z += p * cz;
            l += 5.0;
        }
        let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
        let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
        let b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
        // Desaturate toward the luminance until every channel is >= 0.
        let m = r.min(g).min(b);
        if m < 0.0 {
            let k = y / (y - m);
            [y + (r - y) * k, y + (g - y) * k, y + (b - y) * k]
        } else {
            [r, g, b]
        }
    }
    let n = raw(6500.0);
    let y65 = 0.2126 * n[0] + 0.7152 * n[1] + 0.0722 * n[2];
    let v = raw(t);
    [(v[0] / y65) as f32, (v[1] / y65) as f32, (v[2] / y65) as f32]
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

impl BlackHole {
    pub fn new(w: usize, h: usize, seed: u32) -> Self {
        let mut bh = Self {
            w: 0,
            h: 0,
            bw: 0,
            bh: 0,
            table: vec![0.0; ROWS * COLS],
            esc: vec![-1.0; ROWS],
            imp: vec![0.0; ROWS],
            dobs: vec![1.0; ROWS],
            psi_max: 1.0,
            first_escape: ROWS,
            table_key: [-1.0; 3],
            geom_key: None,
            scr_psi: Vec::new(),
            scr_th: Vec::new(),
            scr_key: None,
            scr_psi_max: 1.0,
            ar: Vec::new(),
            ag: Vec::new(),
            ab: Vec::new(),
            at: Vec::new(),
            noise: Vec::new(),
            pano: Vec::new(),
            bb: Vec::new(),
            srgb: Vec::new(),
            stars: Vec::new(),
            disk: vec![0.0; TEX_X * (TEX_A + 1)],
            layers: [Layer::default(), Layer::default(), Layer::default()],
            sky_u: Vec::new(),
            sky_v: Vec::new(),
            sky_k: Vec::new(),
            hr: Vec::new(),
            hg: Vec::new(),
            hb: Vec::new(),
            trans: Vec::new(),
            lr: Vec::new(),
            lg: Vec::new(),
            lb: Vec::new(),
            lt: Vec::new(),
            tmp: Vec::new(),
            acc: Vec::new(),
            grain_tab: Vec::new(),
            wide: [Vec::new(), Vec::new(), Vec::new()],
            up_x0: Vec::new(),
            up_fx: Vec::new(),
            row_a: Vec::new(),
            row_b: Vec::new(),
            out: Vec::new(),
            stats: Stats::default(),
            frame: 0,
            cam: None,
            adapt: 1.0,
        };
        bh.build_static(seed);
        bh.resize(w, h);
        bh
    }

    fn build_static(&mut self, seed: u32) {
        // Disk turbulence: anisotropic periodic fbm — finer radially than in
        // azimuth, so shear draws it into filaments.
        let mut noise = vec![0.0f32; TEX_X * (TEX_A + 1)];
        for x in 0..TEX_X {
            for a in 0..TEX_A {
                let fx = x as f32 / TEX_X as f32 * 24.0;
                let fa = a as f32 / TEX_A as f32 * 14.0;
                // Clumpy fine structure on top of broad lanes and gaps.
                let n = fbm(fx, fa, 24, 14, 4, seed);
                let m = fbm(fx * 0.25 + 3.1, fa * 0.2, 8, 8, 3, seed ^ 0x55);
                let v = (n * 2.3 - 0.68 + (m - 0.5) * 0.7).max(0.0);
                noise[x * (TEX_A + 1) + a] = v * sqrtf(v) * 1.35;
            }
            noise[x * (TEX_A + 1) + TEX_A] = noise[x * (TEX_A + 1)];
        }
        self.noise = noise;
        // Blackbody colours on a log-temperature grid.
        let lt0 = libm::logf(BB_T0);
        let lt1 = libm::logf(BB_T1);
        self.bb = (0..BB_N).map(|i| blackbody(libm::expf(lt0 + (lt1 - lt0) * i as f32 / (BB_N - 1) as f32) as f64)).collect();
        // Tone-mapped value (0..1) → sRGB byte.
        self.srgb = (0..SRGB_N)
            .map(|i| {
                let v = i as f64 / (SRGB_N - 1) as f64;
                let s = if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * crate::math::pow(v, 1.0 / 2.4) - 0.055 };
                (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
            })
            .collect();
        // Star catalogue: a magnitude-limited sample with a Milky-Way band.
        let n = 9000;
        let mut stars = Vec::with_capacity(n);
        let band_n = norm([0.35, -0.55, 0.76]);
        let band_a = norm([band_n[1], -band_n[0], 0.0]);
        let band_b = [
            band_n[1] * band_a[2] - band_n[2] * band_a[1],
            band_n[2] * band_a[0] - band_n[0] * band_a[2],
            band_n[0] * band_a[1] - band_n[1] * band_a[0],
        ];
        for i in 0..n as u32 {
            let r = |k: u32| hashf(i, k, seed ^ 0xa11ce);
            let dir = if r(0) < 0.45 {
                // Band: gaussian latitude around a tilted great circle.
                let lon = r(1) * TAU;
                let g = (r(2) + r(3) + r(4) - 1.5) * 0.22;
                let (sl, cl) = sincosf(lon);
                norm([
                    (band_a[0] * cl + band_b[0] * sl) + band_n[0] * g,
                    (band_a[1] * cl + band_b[1] * sl) + band_n[1] * g,
                    (band_a[2] * cl + band_b[2] * sl) + band_n[2] * g,
                ])
            } else {
                let z = r(1) * 2.0 - 1.0;
                let p = r(2) * TAU;
                let s = sqrtf(1.0 - z * z);
                let (sp, cp) = sincosf(p);
                [s * cp, s * sp, z]
            };
            // N(<m) ∝ 10^(0.5 m) between m = -1 and 8.5.
            let m = 8.5 + 2.0 * libm::log10f(r(5).max(1e-6));
            let m = m.max(-1.0);
            let flux = libm::powf(10.0, -0.4 * m);
            let c = r(6);
            let temp = if c < 0.55 {
                3200.0 + r(7) * 2300.0
            } else if c < 0.85 {
                5500.0 + r(7) * 2500.0
            } else {
                8000.0 + r(7) * r(7) * 22_000.0
            };
            stars.push(Star { dir, flux, temp });
        }
        self.stars = stars;
        // Sky panorama in the disk frame (lon about +z, lat): faint nebula,
        // a dusty Milky-Way band.
        let mut pano = vec![[0.0f32; 3]; PANO_W * PANO_H];
        for y in 0..PANO_H {
            let lat = (0.5 - (y as f32 + 0.5) / PANO_H as f32) * PI;
            let (sla, cla) = sincosf(lat);
            for x in 0..PANO_W {
                let lon = (x as f32 + 0.5) / PANO_W as f32 * TAU;
                let (slo, clo) = sincosf(lon);
                let d = [cla * clo, cla * slo, sla];
                let bl = dot(d, band_n);
                let band = exp_neg(bl * bl * 90.0);
                let u = x as f32 / PANO_W as f32;
                let v = y as f32 / PANO_H as f32;
                let n1 = fbm(u * 16.0, v * 8.0, 16, 8, 5, seed ^ 0xbeef);
                let n2 = fbm(u * 8.0 + 0.3, v * 4.0, 8, 4, 4, seed ^ 0xf00d);
                let n3 = fbm(u * 32.0, v * 16.0, 32, 16, 4, seed ^ 0xd057);
                let dust = smooth(0.45, 0.7, n3) * band;
                let glow = band * (0.35 + 0.65 * n1) * (1.0 - 0.85 * dust);
                let neb = smooth(0.55, 0.85, n2) * 0.5;
                let neb2 = smooth(0.6, 0.9, n1) * 0.3;
                // Fade structure out toward the poles (no equirect pinch).
                let pole = smooth(0.82, 0.97, sla.abs());
                let (glow, neb, neb2) = (glow * (1.0 - pole), neb * (1.0 - pole), neb2 * (1.0 - pole));
                let k = 0.03;
                pano[y * PANO_W + x] = [
                    k * (glow * 1.0 + neb * 0.9 + neb2 * 0.2),
                    k * (glow * 0.85 + neb * 0.35 + neb2 * 0.35),
                    k * (glow * 0.7 + neb * 0.5 + neb2 * 0.9),
                ];
            }
        }
        self.pano = pano;
        self.grain_tab = (0..4100u32).map(|i| hashf(i, 7, seed) - 0.5).collect();
    }

    /// Change the output size (pixels).
    pub fn resize(&mut self, w: usize, h: usize) {
        let (w, h) = (w.max(4), h.max(4));
        if w == self.w && h == self.h {
            return;
        }
        self.w = w;
        self.h = h;
        self.bw = w.div_ceil(4);
        self.bh = h.div_ceil(4);
        let n = w * h;
        self.hr = vec![0.0; n];
        self.hg = vec![0.0; n];
        self.hb = vec![0.0; n];
        self.trans = vec![1.0; n];
        let m = self.bw * self.bh;
        self.lr = vec![0.0; m];
        self.lg = vec![0.0; m];
        self.lb = vec![0.0; m];
        self.lt = vec![0.0; m];
        self.ar = vec![0.0; m];
        self.ag = vec![0.0; m];
        self.ab = vec![0.0; m];
        self.at = vec![0.0; m];
        self.scr_key = None;
        self.tmp = vec![0.0; m.max(self.bw.max(self.bh))];
        self.acc = vec![0.0; self.bw];
        self.wide = [vec![0.0; m], vec![0.0; m], vec![0.0; m]];
        self.sky_u = vec![0.0; m];
        self.sky_v = vec![0.0; m];
        self.sky_k = vec![0.0; m];
        self.out = vec![0; n * 4];
        self.up_x0 = Vec::with_capacity(w);
        self.up_fx = Vec::with_capacity(w);
        for x in 0..w {
            let lx = ((x as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (self.bw - 1) as f32);
            let x0 = (lx as usize).min(self.bw.saturating_sub(2));
            self.up_x0.push(x0 as u32);
            self.up_fx.push((lx - x0 as f32).min(1.0));
        }
        self.row_a = vec![0.0; w * 3];
        self.row_b = vec![0.0; w * 3];
        self.geom_key = None;
    }

    // -- geodesic table ----------------------------------------------------

    /// Integrate the table for an observer at `distance` with infall `fall`,
    /// for viewing angles up to `psi_max`.
    pub fn build_table(&mut self, distance: f32, fall: f32, psi_max: f32) {
        let key = [distance, fall, psi_max];
        if key == self.table_key {
            return;
        }
        self.table_key = key;
        self.stats.table_built = 1;
        self.psi_max = psi_max;
        let u0 = 1.0 / distance;
        let h = H_PHI;
        let f = |u: f32| 3.0 * u * u - u;
        self.first_escape = ROWS;
        for j in 0..ROWS {
            let psi = self.row_psi(j as f32);
            let row = &mut self.table[j * COLS..(j + 1) * COLS];
            let Some((w0, b, d)) = ray_init(psi, u0, fall) else {
                row.fill(U_CAPTURED);
                self.esc[j] = -1.0;
                self.imp[j] = 0.0;
                self.dobs[j] = 1.0;
                continue;
            };
            self.imp[j] = b;
            self.dobs[j] = d;
            let (mut u, mut w) = (u0, w0);
            row[0] = u;
            let mut esc = -1.0;
            let mut k = 1;
            while k < COLS {
                // RK4 on (u, w = du/dφ), w' = 3u² − u.
                let k1u = w;
                let k1w = f(u);
                let k2u = w + 0.5 * h * k1w;
                let k2w = f(u + 0.5 * h * k1u);
                let k3u = w + 0.5 * h * k2w;
                let k3w = f(u + 0.5 * h * k2u);
                let k4u = w + h * k3w;
                let k4w = f(u + h * k3u);
                let un = u + h / 6.0 * (k1u + 2.0 * k2u + 2.0 * k3u + k4u);
                let wn = w + h / 6.0 * (k1w + 2.0 * k2w + 2.0 * k3w + k4w);
                if un <= 0.0 {
                    // Reached r = ∞ between samples: interpolate φ∞.
                    esc = (k as f32 - 1.0 + u / (u - un)) * h;
                    row[k..].fill(U_ESCAPED);
                    break;
                }
                if un > 0.5 && wn > 0.0 && (un > u0 || u0 < 0.5) {
                    row[k..].fill(U_CAPTURED);
                    break;
                }
                row[k] = un;
                u = un;
                w = wn;
                k += 1;
            }
            self.esc[j] = esc;
            if esc >= 0.0 && self.first_escape == ROWS {
                self.first_escape = j;
            }
        }
        // Escape angle must be monotonic over the escaping rows for the
        // star inversion; rows beyond the first escape that were captured
        // (grazing numerical noise) inherit the neighbour's value.
        for j in self.first_escape + 1..ROWS {
            if self.esc[j] < 0.0 {
                self.esc[j] = self.esc[j - 1];
            }
        }
    }

    #[inline(always)]
    fn row_psi(&self, row: f32) -> f32 {
        (row + 0.5) / ROWS as f32 * self.psi_max
    }

    /// Angular radius (observer frame) of the shadow edge, from the table.
    pub fn shadow_radius(&self) -> f32 {
        if self.first_escape >= ROWS {
            -1.0
        } else {
            self.row_psi(self.first_escape as f32 - 0.5)
        }
    }

    /// Bilinear table lookup: u at (row, col) in table coordinates.
    #[inline(always)]
    fn lookup(&self, row: f32, col: f32) -> f32 {
        let r0 = (row as usize).min(ROWS - 2);
        let fr = (row - r0 as f32).clamp(0.0, 1.0);
        let c0 = (col as usize).min(COLS - 2);
        let fc = col - c0 as f32;
        let t = &self.table;
        let a = t[r0 * COLS + c0];
        let b = t[r0 * COLS + c0 + 1];
        let c = t[(r0 + 1) * COLS + c0];
        let d = t[(r0 + 1) * COLS + c0 + 1];
        let ab = a + (b - a) * fc;
        let cd = c + (d - c) * fc;
        ab + (cd - ab) * fr
    }

    /// r(φ) along the ray seen at viewing angle `psi` (for tests).
    pub fn orbit_u(&self, psi: f32, phi: f32) -> f32 {
        self.lookup(psi / self.psi_max * ROWS as f32 - 0.5, phi / H_PHI)
    }

    fn camera(p: &Params) -> Camera {
        let inc = p.inclination.clamp(0.0, 180.0) * (PI / 180.0);
        let (si, ci) = sincosf(inc);
        let c = [si, 0.0, ci];
        let fwd0 = [-si, 0.0, -ci];
        let up0 = [-ci, 0.0, si];
        // right = fwd × up
        let right0 = [
            fwd0[1] * up0[2] - fwd0[2] * up0[1],
            fwd0[2] * up0[0] - fwd0[0] * up0[2],
            fwd0[0] * up0[1] - fwd0[1] * up0[0],
        ];
        let (sr, cr) = sincosf(p.roll * (PI / 180.0));
        let up1 = [up0[0] * cr + right0[0] * sr, up0[1] * cr + right0[1] * sr, up0[2] * cr + right0[2] * sr];
        let right1 = [right0[0] * cr - up0[0] * sr, right0[1] * cr - up0[1] * sr, right0[2] * cr - up0[2] * sr];
        let (sl, cl) = sincosf(p.look * (PI / 180.0));
        let fwd = [fwd0[0] * cl + up1[0] * sl, fwd0[1] * cl + up1[1] * sl, fwd0[2] * cl + up1[2] * sl];
        let up = [up1[0] * cl - fwd0[0] * sl, up1[1] * cl - fwd0[1] * sl, up1[2] * cl - fwd0[2] * sl];
        let tan_half = libm::tanf(p.fov.clamp(1.0, 170.0) * (PI / 360.0));
        Camera { c, fwd, right: right1, up, pr: right1, pu: up1, tan_half }
    }

    /// World direction of the ray through screen point (sx, sy) in units of
    /// the half-height (y up).
    #[inline(always)]
    fn ray_dir(cam: &Camera, sx: f32, sy: f32) -> [f32; 3] {
        let a = sx * cam.tan_half;
        let b = sy * cam.tan_half;
        norm([
            cam.fwd[0] + cam.right[0] * a + cam.up[0] * b,
            cam.fwd[1] + cam.right[1] * a + cam.up[1] * b,
            cam.fwd[2] + cam.right[2] * a + cam.up[2] * b,
        ])
    }

    /// ψ, ŝ for a world ray direction.
    #[inline(always)]
    fn orbit_plane(cam: &Camera, d: [f32; 3]) -> (f32, [f32; 3]) {
        let cp = -dot(d, cam.c);
        let s = [d[0] + cp * cam.c[0], d[1] + cp * cam.c[1], d[2] + cp * cam.c[2]];
        let l = sqrtf(dot(s, s));
        let s = if l < 1e-7 { cam.up } else { [s[0] / l, s[1] / l, s[2] / l] };
        (acosf(cp), s)
    }

    fn temperature(p: &Params, r: f32) -> f32 {
        if p.mode == 1 {
            // Cinematic: a hot inner rim instead of the zero-torque falloff.
            p.temperature * libm::powf(ISCO / r, 0.75) * (0.55 + 0.45 * smooth(ISCO * 0.92, ISCO * 1.25, r))
        } else {
            // Page–Thorne, normalised to the peak (at r ≈ 9.55M).
            let f = page_thorne_flux(r as f64) / page_thorne_flux(9.554);
            p.temperature * libm::powf(f.max(0.0) as f32, 0.25)
        }
    }

    #[inline(always)]
    fn bb_at(&self, t: f32) -> [f32; 3] {
        let lt0 = 9.228_819; // log2(600)
        let lt1 = 16.609_64; // log2(100000)
        let x = ((fast_log2(t.max(BB_T0)) - lt0) / (lt1 - lt0) * (BB_N - 1) as f32).clamp(0.0, (BB_N - 1) as f32);
        let i = (x as usize).min(BB_N - 2);
        let f = x - i as f32;
        let a = self.bb[i];
        let b = self.bb[i + 1];
        [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]
    }

    /// Per-pixel ψ and θ for the current screen (size, fov, look).
    fn build_screen(&mut self, cam: &Camera) {
        let (w, h) = (self.w, self.h);
        let half = h as f32 * 0.5;
        let aspect = w as f32 / h as f32;
        let mut psi_max = 0.0f32;
        for (sx, sy) in [(-aspect, -1.0), (aspect, -1.0), (-aspect, 1.0), (aspect, 1.0), (0.0, 1.0), (0.0, -1.0), (aspect, 0.0), (-aspect, 0.0)] {
            psi_max = psi_max.max(Self::orbit_plane(cam, Self::ray_dir(cam, sx, sy)).0);
        }
        self.scr_psi_max = (psi_max * 1.02 + 0.002).min(PI);
        self.scr_psi.resize(w * h, 0.0);
        self.scr_th.resize(w * h, 0);
        let th_k = TH_BINS as f32 / TAU;
        let straight = dot(cam.fwd, cam.c) < -0.999_999;
        for y in 0..h {
            let sy = (half - y as f32 - 0.5) / half;
            for x in 0..w {
                let sx = (x as f32 + 0.5 - w as f32 * 0.5) / half;
                let (psi, th) = if straight {
                    // Looking at the hole: ψ is radial, θ the screen angle.
                    let rho = sqrtf(sx * sx + sy * sy) * cam.tan_half;
                    (atan2f(rho, 1.0), atan2f(sy, sx))
                } else {
                    let d = Self::ray_dir(cam, sx, sy);
                    let (psi, sv) = Self::orbit_plane(cam, d);
                    (psi, atan2f(dot(sv, cam.pu), dot(sv, cam.pr)))
                };
                let mut b = th * th_k + 0.5;
                if b < 0.0 {
                    b += TH_BINS as f32;
                }
                self.scr_psi[y * w + x] = psi;
                self.scr_th[y * w + x] = (b as usize % TH_BINS) as u16;
            }
        }
    }

    /// Rebuild the per-pixel lensing geometry for a camera.
    fn build_geometry(&mut self, p: &Params) {
        self.stats.geometry_built = 1;
        let cam = Self::camera(p);
        self.cam = Some(cam);
        let (w, h) = (self.w, self.h);
        let skey = (w, h, p.fov, p.look);
        if self.scr_key != Some(skey) {
            self.build_screen(&cam);
            self.scr_key = Some(skey);
        }
        let fall = p.fall.clamp(0.0, 1.0);
        self.build_table(p.distance.max(1.0), fall, self.scr_psi_max);
        for l in self.layers.iter_mut() {
            l.clear();
        }
        let ci = cam.c[2];
        let rout = p.disk_outer.max(ISCO + 1.0);
        let u_lo = 1.0 / (rout * 1.05);
        let u_hi = 1.0 / TEX_R0;
        let lx0 = libm::logf(TEX_R0);
        let lx1 = libm::logf(rout * 1.06);
        let xs = (TEX_X - 2) as f32 / (lx1 - lx0);
        let cinematic = p.mode == 1;
        // Exposure is relative to the disk's peak (rest-frame) luminance.
        let peak = self.bb_at(p.temperature.max(BB_T0));
        let norm_k = if cinematic { 0.5 } else { 0.1 } / (0.2126 * peak[0] + 0.7152 * peak[1] + 0.0722 * peak[2]).max(1e-6);
        // Radial profile over u: temperature, edge fade, texture x.
        const UN: usize = 2048;
        // [T, fade, texture x, √(1 − 3u), Ω]
        let mut ulut = vec![[0.0f32; 5]; UN + 2];
        for (i, e) in ulut.iter_mut().enumerate() {
            let u = u_lo + (u_hi - u_lo) * i as f32 / UN as f32;
            let r = 1.0 / u;
            let fade = smooth(rout, rout * 0.62, r) * smooth(ISCO * 0.86, ISCO * 1.02, r);
            let tx = ((libm::logf(r) - lx0) * xs).clamp(0.0, (TEX_X - 2) as f32 - 0.01);
            let uc = u.min(1.0 / 3.01);
            *e = [Self::temperature(p, r), fade, tx, sqrtf(1.0 - 3.0 * uc), uc * sqrtf(uc)];
        }
        let u_k = UN as f32 / (u_hi - u_lo);
        // Orbit-plane quantities per θ bin: φ0 column, azimuth (texels), L_z/b.
        let mut tlut = vec![[0.0f32; 3]; TH_BINS];
        for (i, e) in tlut.iter_mut().enumerate() {
            let (st, ct) = sincosf(i as f32 / TH_BINS as f32 * TAU);
            let sv = [ct * cam.pr[0] + st * cam.pu[0], ct * cam.pr[1] + st * cam.pu[1], ct * cam.pr[2] + st * cam.pu[2]];
            let mut phi0 = atan2f(-ci, sv[2]);
            if phi0 < 0.0 {
                phi0 += PI;
            }
            if phi0 >= PI {
                phi0 -= PI;
            }
            let (sp, cp) = sincosf(phi0);
            let az0 = atan2f(cp * cam.c[1] + sp * sv[1], cp * cam.c[0] + sp * sv[0]);
            let qz = -(cam.c[0] * sv[1] - cam.c[1] * sv[0]);
            *e = [phi0 / H_PHI, az0 * (TEX_A as f32 / TAU), qz];
        }
        let lt0 = libm::log2f(BB_T0);
        let bb_k = (BB_N - 1) as f32 / (libm::log2f(BB_T1) - lt0);
        let row_scale = ROWS as f32 / self.psi_max;
        // One pixel's angular size, in table rows.
        let px_rows = 2.0 * cam.tan_half / h as f32 * row_scale;
        // The thin images hug the shadow edge.
        let edge_row = self.first_escape as f32;
        let alpha_q = if cinematic { 0.92 * 65535.0 } else { 0.97 * 65535.0 };
        // Walk the screen in 32×32 tiles: neighbouring pixels read
        // neighbouring table rows, which keeps the 3 MB table in cache.
        const TILE: usize = 32;
        let tiles = (0..h.div_ceil(TILE)).flat_map(|ty| (0..w.div_ceil(TILE)).map(move |tx| (tx, ty)));
        for pix in tiles.flat_map(|(tx, ty)| {
            (ty * TILE..((ty + 1) * TILE).min(h)).flat_map(move |y| (tx * TILE..((tx + 1) * TILE).min(w)).map(move |x| y * w + x))
        }) {
            let row = self.scr_psi[pix] * row_scale - 0.5;
            if row >= (ROWS - 1) as f32 {
                continue;
            }
            let row = row.max(0.0);
            let [col0, az_t, qz] = tlut[self.scr_th[pix] as usize];
            let ri = (row as usize).min(ROWS - 2);
            let fr = row - ri as f32;
            let bimp = self.imp[ri] + (self.imp[ri + 1] - self.imp[ri]) * fr;
            let dob = self.dobs[ri] + (self.dobs[ri + 1] - self.dobs[ri]) * fr;
            let lz = bimp * qz;
            // The crossings are whole half-turns apart: same column fraction,
            // columns 128 apart, on the same two table rows.
            let ci0 = (col0 as usize).min(COLS - 2 - 2 * COLS_PER_PI);
            let fc = col0 - ci0 as f32;
            let r0 = &self.table[ri * COLS + ci0..ri * COLS + COLS];
            let r1 = &self.table[(ri + 1) * COLS + ci0..(ri + 1) * COLS + COLS];
            for k in 0..3 {
                let o = k * COLS_PER_PI;
                let a = r0[o] + (r0[o + 1] - r0[o]) * fc;
                let b = r1[o] + (r1[o + 1] - r1[o]) * fc;
                let mut u = a + (b - a) * fr;
                // The lensed images (k ≥ 1) are often thinner than a pixel:
                // take the pixel's coverage from three ψ sub-samples.
                let mut cover = 1.0;
                if k > 0 && (row - edge_row).abs() < 10.0 * px_rows {
                    let mut hits = 0u32;
                    let mut first = -1.0f32;
                    for d in [-0.33f32, 0.0, 0.33] {
                        let rr = (row + d * px_rows).clamp(0.0, (ROWS - 1) as f32 - 1e-3);
                        let us = self.lookup(rr, col0 + o as f32);
                        if us > u_lo && us < u_hi {
                            hits += 1;
                            if first < 0.0 || d == 0.0 {
                                first = us;
                            }
                        }
                    }
                    if hits > 0 {
                        u = first;
                        cover = hits as f32 / 3.0;
                    }
                }
                if !(u > u_lo && u < u_hi) {
                    // Escaped or captured before this crossing: no later ones.
                    if u <= 0.0 || u >= 1.0 {
                        break;
                    }
                    continue;
                }
                let ue = ulut[((u - u_lo) * u_k) as usize];
                let (temp, fade, tx) = (ue[0], ue[1], ue[2]);
                if temp < 1.0 || fade <= 0.0 {
                    continue;
                }
                // g = D·√(1 − 3u)/(1 − Ω L_z/E), see `redshift`.
                let g = dob * ue[3] / (1.0 - ue[4] * lz).max(1e-3);
                let (t_obs, gain) = if cinematic {
                    // Interstellar: no Doppler colour, beaming softened.
                    let gg = g.clamp(0.2, 3.0);
                    (temp, gg * sqrtf(gg) * 1.6)
                } else {
                    // I_ν(ν) = B_ν(ν; gT): a blackbody at g·T, ∝ g⁴T⁴.
                    (temp * g, 1.0)
                };
                let ci_ = ((fast_log2(t_obs.max(BB_T0)) - lt0) * bb_k + 0.5).min((BB_N - 1) as f32) as u32;
                let mut ta = az_t + (k * TEX_A / 2) as f32;
                if ta >= TEX_A as f32 {
                    ta -= TEX_A as f32;
                }
                if ta < 0.0 {
                    ta += TEX_A as f32;
                }
                let txa = ((tx * 256.0) as u32).min(0xffff) | (((ta * 64.0) as u32).min(TEX_A as u32 * 64 - 1) << 16);
                let l = &mut self.layers[k];
                l.pix.push(pix as u32);
                l.txa.push(txa);
                l.ca.push(ci_ | (((fade * cover * alpha_q) as u32) << 16));
                l.gain.push(gain * fade * cover * norm_k);
            }
        }
        // Eye adaptation from the energy factor of the outermost escaping ray.
        let d_back = self.dobs[ROWS - 1].max(0.05);
        self.adapt = if self.first_escape < ROWS { (1.0 / (d_back * d_back * d_back)).clamp(1.0, 10.0) } else { 1.0 };
        // Low-resolution sky coordinates.
        let (bw, bhh) = (self.bw, self.bh);
        for by in 0..bhh {
            for bx in 0..bw {
                let i = by * bw + bx;
                let pix = (by * 4 + 2).min(h - 1) * w + (bx * 4 + 2).min(w - 1);
                let row = (self.scr_psi[pix] * row_scale - 0.5).max(0.0);
                let ri = (row as usize).min(ROWS - 2);
                let fr = row - ri as f32;
                let (e0, e1) = (self.esc[ri], self.esc[ri + 1]);
                if e0 < 0.0 || e1 < 0.0 || ri < self.first_escape {
                    self.sky_k[i] = 0.0;
                    continue;
                }
                let (st, ct) = sincosf(self.scr_th[pix] as f32 / TH_BINS as f32 * TAU);
                let sv = [ct * cam.pr[0] + st * cam.pu[0], ct * cam.pr[1] + st * cam.pu[1], ct * cam.pr[2] + st * cam.pu[2]];
                let phi = e0 + (e1 - e0) * fr;
                let dob = self.dobs[ri] + (self.dobs[ri + 1] - self.dobs[ri]) * fr;
                let (sp, cp) = sincosf(phi);
                let dir = [cp * cam.c[0] + sp * sv[0], cp * cam.c[1] + sp * sv[1], cp * cam.c[2] + sp * sv[2]];
                let lon = atan2f(dir[1], dir[0]);
                let lat = atan2f(dir[2], sqrtf(dir[0] * dir[0] + dir[1] * dir[1]));
                self.sky_u[i] = lon * (PANO_W as f32 / TAU);
                self.sky_v[i] = ((0.5 - lat / PI) * PANO_H as f32 - 0.5).clamp(0.0, (PANO_H - 2) as f32);
                self.sky_k[i] = (dob * dob * dob).min(40.0);
            }
        }
    }

    /// Build the flowing disk texture for time `t` (M) and camera azimuth.
    fn build_disk(&mut self, p: &Params, t: f32, az_off: f32) {
        let rout = p.disk_outer.max(ISCO + 1.0);
        let lx0 = libm::logf(TEX_R0);
        let lx1 = libm::logf(rout * 1.06);
        let period = 70.0;
        let cyc = t / period;
        let tex_per_rad = TEX_A as f32 / TAU;
        let mut ph = [(0.0f32, 0.0f32, 0u32); 2];
        for (i, slot) in ph.iter_mut().enumerate() {
            let c = cyc + i as f32 * 0.5;
            let cf = floorf(c);
            let tau = c - cf;
            let wgt = 1.0 - (2.0 * tau - 1.0).abs();
            *slot = (tau * period, wgt, hash(cf as i32 as u32 ^ (i as u32 * 0x9e37)));
        }
        for x in 0..TEX_X {
            let r = libm::expf(lx0 + (lx1 - lx0) * x as f32 / (TEX_X - 2) as f32);
            let om = 1.0 / (r * sqrtf(r));
            let row = &mut self.disk[x * (TEX_A + 1)..(x + 1) * (TEX_A + 1)];
            row[..TEX_A].fill(0.0);
            for &(age, wgt, seed) in ph.iter() {
                // Sample N(a + Δ − Ω·age + random offset per cycle).
                let shift = (az_off - om * age) * tex_per_rad + (seed % TEX_A as u32) as f32;
                let mut s = shift - floorf(shift / TEX_A as f32) * TEX_A as f32;
                if s >= TEX_A as f32 {
                    s -= TEX_A as f32;
                }
                let si = s as usize;
                let f = s - si as f32;
                let src = &self.noise[x * (TEX_A + 1)..(x + 1) * (TEX_A + 1)];
                let (w0, w1) = (wgt * (1.0 - f), wgt * f);
                let n1 = TEX_A - si;
                // a in [0, n1): j = a + si; a in [n1, TEX_A): j = a + si − TEX_A.
                for (o, (a, b)) in row[..n1].iter_mut().zip(src[si..TEX_A].iter().zip(src[si + 1..].iter())) {
                    *o += w0 * a + w1 * b;
                }
                for (o, (a, b)) in row[n1..TEX_A].iter_mut().zip(src[..si].iter().zip(src[1..].iter())) {
                    *o += w0 * a + w1 * b;
                }
            }
            // The two phases are independent: restore contrast lost to mixing.
            let wsum2 = ph[0].1 * ph[0].1 + ph[1].1 * ph[1].1;
            let k = 1.0 / sqrtf(wsum2.max(0.25));
            for v in row[..TEX_A].iter_mut() {
                *v = ((*v - 0.5) * k + 0.5).max(0.0);
            }
        }
        // Hot spots: compact flares on Keplerian orbits that shear as they go.
        let n_spots = if p.hot_spots > 0.0 { 5 } else { 0 };
        for i in 0..n_spots {
            let life = 150.0;
            let ph0 = i as f32 / n_spots as f32;
            let c = t / life + ph0;
            let cf = floorf(c);
            let age = (c - cf) * life;
            let hs = |k: u32| hashf(i, cf as i32 as u32, k);
            let rs = 6.6 * libm::powf(2.0, hs(1) * 1.1);
            if rs > rout * 0.8 {
                continue;
            }
            let amp = p.hot_spots * 1.6 * {
                let s = libm::sinf(PI * (c - cf));
                s * s
            } * (0.5 + hs(3));
            let az0 = hs(2) * TAU;
            let xs = (libm::logf(rs) - lx0) / (lx1 - lx0) * (TEX_X - 2) as f32;
            let sig_x = 0.7 / rs / (lx1 - lx0) * (TEX_X - 2) as f32;
            let sig_a = 0.22 * tex_per_rad;
            let x_lo = ((xs - 3.0 * sig_x).max(0.0)) as usize;
            let x_hi = ((xs + 3.0 * sig_x) as usize + 1).min(TEX_X - 1);
            for x in x_lo..=x_hi {
                let dx = (x as f32 - xs) / sig_x;
                let wx = exp_neg(dx * dx);
                let r = libm::expf(lx0 + (lx1 - lx0) * x as f32 / (TEX_X - 2) as f32);
                let om = 1.0 / (r * sqrtf(r));
                let mut ac = (az0 + om * age - az_off) * tex_per_rad;
                ac -= floorf(ac / TEX_A as f32) * TEX_A as f32;
                let span = (3.0 * sig_a) as i32 + 1;
                let row = &mut self.disk[x * (TEX_A + 1)..(x + 1) * (TEX_A + 1)];
                for da in -span..=span {
                    let a = (ac as i32 + da).rem_euclid(TEX_A as i32) as usize;
                    let d = (floorf(ac) + da as f32 - ac) / sig_a;
                    row[a] += amp * wx * exp_neg(d * d);
                }
            }
        }
        // Infalling gas streams: two trailing spirals that wind in from the
        // outer disk, rotating with the gas at their radius, with clumps
        // sliding inward along them; each fades in and out over its life.
        if p.hot_spots > 0.0 {
            for i in 0..2u32 {
                let life = 420.0;
                let c = t / life + i as f32 * 0.5;
                let cf = floorf(c);
                let hs = |k: u32| hashf(i + 17, cf as i32 as u32, k);
                let env = {
                    let s = libm::sinf(PI * (c - cf));
                    s * s
                };
                let amp = p.hot_spots * 0.9 * env;
                let az0 = hs(1) * TAU;
                let pitch = 2.2 + hs(2) * 1.2;
                let age = (c - cf) * life;
                let sig_a = 0.16 * tex_per_rad;
                let span = (3.0 * sig_a) as i32 + 1;
                for x in 0..TEX_X - 1 {
                    let lr = lx0 + (lx1 - lx0) * x as f32 / (TEX_X - 2) as f32;
                    let r = libm::expf(lr);
                    if r < ISCO {
                        continue;
                    }
                    let om = 1.0 / (r * sqrtf(r));
                    // Trailing spiral: azimuth grows inward; the whole stream
                    // is carried around at the local orbital speed.
                    let along = lx1 - lr;
                    let clump = 0.55 + 0.45 * libm::sinf(along * 9.0 + age * 0.06);
                    let w = amp * clump * smooth(lx1, lx1 - 0.25, lr) * smooth(libm::logf(ISCO), libm::logf(ISCO) + 0.3, lr);
                    if w <= 0.0 {
                        continue;
                    }
                    let mut ac = (az0 + pitch * along + om * age - az_off) * tex_per_rad;
                    ac -= floorf(ac / TEX_A as f32) * TEX_A as f32;
                    let row = &mut self.disk[x * (TEX_A + 1)..(x + 1) * (TEX_A + 1)];
                    for da in -span..=span {
                        let a = (ac as i32 + da).rem_euclid(TEX_A as i32) as usize;
                        let d = (floorf(ac) + da as f32 - ac) / sig_a;
                        row[a] += w * exp_neg(d * d);
                    }
                }
            }
        }
        for x in 0..TEX_X {
            let o = x * (TEX_A + 1);
            self.disk[o + TEX_A] = self.disk[o];
        }
    }

    /// Render one frame into `out` (RGBA8).
    pub fn render(&mut self, p: &Params) {
        self.stats = Stats::default();
        self.frame = self.frame.wrapping_add(1);
        let key = Params { time: 0.0, exposure: 0.0, bloom: 0.0, grain: 0.0, hot_spots: 0.0, jet: 0.0, azimuth: 0.0, stars: 0.0, speed: 0.0, nebula: 0.0, ..*p };
        if self.geom_key != Some((key, self.w, self.h)) {
            self.build_geometry(p);
            self.geom_key = Some((key, self.w, self.h));
        }
        self.stats.shadow = self.shadow_radius();
        let cam = self.cam.unwrap_or_else(|| Self::camera(p));
        // ~11.5 M per second: the ISCO orbit (92 M) takes ~8 s at speed 1.
        let t = p.time * 11.5 * p.speed;
        let az_off = p.azimuth * (PI / 180.0);
        self.build_disk(p, t, az_off);
        self.shade_disk();
        self.draw_stars(p, &cam, az_off);
        self.bloom_and_sky(p, &cam, az_off);
        self.tonemap(p);
    }

    /// Run one pipeline stage alone (0 disk texture, 1 shading, 2 stars,
    /// 3 bloom and sky, 4 tone map, 5 geometry), for profiling from a host
    /// with a clock.
    #[doc(hidden)]
    pub fn stage(&mut self, p: &Params, k: u32) {
        let cam = self.cam.unwrap_or_else(|| Self::camera(p));
        let t = p.time * 11.5 * p.speed;
        let az = p.azimuth * (PI / 180.0);
        match k {
            0 => self.build_disk(p, t, az),
            1 => self.shade_disk(),
            2 => self.draw_stars(p, &cam, az),
            3 => self.bloom_and_sky(p, &cam, az),
            4 => self.tonemap(p),
            _ => self.build_geometry(p),
        }
    }

    /// Stage timings in ms (disk texture, shading, stars, bloom, tone map).
    #[cfg(feature = "std")]
    pub fn profile(&mut self, p: &Params) -> [f64; 5] {
        let cam = self.cam.unwrap_or_else(|| Self::camera(p));
        let t = p.time * 11.5 * p.speed;
        let az = p.azimuth * (PI / 180.0);
        let mut out = [0.0; 5];
        let mut clock = std::time::Instant::now();
        let mut lap = |i: usize, out: &mut [f64; 5]| {
            out[i] = clock.elapsed().as_secs_f64() * 1000.0;
            clock = std::time::Instant::now();
        };
        self.build_disk(p, t, az);
        lap(0, &mut out);
        self.shade_disk();
        lap(1, &mut out);
        self.draw_stars(p, &cam, az);
        lap(2, &mut out);
        self.bloom_and_sky(p, &cam, az);
        lap(3, &mut out);
        self.tonemap(p);
        lap(4, &mut out);
        out
    }

    fn shade_disk(&mut self) {
        let disk = &self.disk;
        let bb = &self.bb;
        let (hr, hg, hb, tr) = (&mut self.hr, &mut self.hg, &mut self.hb, &mut self.trans);
        const ROW: usize = TEX_A + 1;
        for (k, l) in self.layers.iter().enumerate() {
            self.stats.samples[k] = l.pix.len() as u32;
            for (((&p, &txa), &ca), &gain) in l.pix.iter().zip(&l.txa).zip(&l.ca).zip(&l.gain) {
                let ix = ((txa & 0xffff) >> 8) as usize;
                let fx = (txa & 0xff) as f32 * (1.0 / 256.0);
                let ia = (txa >> 22) as usize;
                let fa = ((txa >> 16) & 63) as f32 * (1.0 / 64.0);
                let t = ix * ROW + ia;
                let (a, b, c, d) = (disk[t], disk[t + 1], disk[t + ROW], disk[t + ROW + 1]);
                let ab = a + (b - a) * fa;
                let cd = c + (d - c) * fa;
                let v = ab + (cd - ab) * fx;
                let p = p as usize;
                let tt = tr[p];
                let e = tt * v * gain;
                let col = bb[(ca & 0xffff) as usize];
                hr[p] += e * col[0];
                hg[p] += e * col[1];
                hb[p] += e * col[2];
                tr[p] = tt * (1.0 - (ca >> 16) as f32 * (1.0 / 65535.0) * v.min(1.0));
            }
        }
    }

    /// Invert the lens for each catalogue star: find the viewing angle whose
    /// ray escapes toward the star (φ∞ = θ, 2π − θ, 2π + θ) by bisection on
    /// the monotonic escape angle, then splat with the solid-angle
    /// magnification and the observer's Doppler factor.
    fn draw_stars(&mut self, p: &Params, cam: &Camera, az_off: f32) {
        if p.stars <= 0.0 || self.first_escape >= ROWS - 2 {
            return;
        }
        let (w, h) = (self.w as f32, self.h as f32);
        let half = h * 0.5;
        let fs = half / cam.tan_half;
        let (sa, ca) = sincosf(-az_off);
        let lo = self.first_escape;
        let mut drawn = 0;
        for si in 0..self.stars.len() {
            let st = &self.stars[si];
            let d = [st.dir[0] * ca - st.dir[1] * sa, st.dir[0] * sa + st.dir[1] * ca, st.dir[2]];
            let ct = dot(d, cam.c);
            let theta = acosf(ct);
            let sp = [d[0] - ct * cam.c[0], d[1] - ct * cam.c[1], d[2] - ct * cam.c[2]];
            let sl = sqrtf(dot(sp, sp)).max(1e-6);
            let sp = [sp[0] / sl, sp[1] / sl, sp[2] / sl];
            let sin_t = sl.max(2e-3);
            for img in 0..3 {
                let (target, sgn) = match img {
                    0 => (theta, 1.0),
                    1 => (TAU - theta, -1.0),
                    _ => (TAU + theta, 1.0),
                };
                // esc is decreasing in j over [lo, ROWS).
                if target > self.esc[lo] || target < self.esc[ROWS - 1] {
                    continue;
                }
                let (mut a, mut b) = (lo, ROWS - 1);
                while b - a > 1 {
                    let m = (a + b) / 2;
                    if self.esc[m] >= target {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                let (ea, eb) = (self.esc[a], self.esc[b]);
                let de = ea - eb;
                if de <= 0.0 {
                    continue;
                }
                let f = (ea - target) / de;
                let psi = self.row_psi(a as f32 + f);
                let dpsi = self.psi_max / ROWS as f32;
                let (spsi, cpsi) = sincosf(psi);
                let mu = (spsi * dpsi / (de * sin_t)).min(40.0);
                let dob = self.dobs[a] + (self.dobs[b] - self.dobs[a]) * f;
                // Screen position of direction cosψ·(−ĉ) + sinψ·(±ŝ).
                let dir = [
                    -cpsi * cam.c[0] + sgn * spsi * sp[0],
                    -cpsi * cam.c[1] + sgn * spsi * sp[1],
                    -cpsi * cam.c[2] + sgn * spsi * sp[2],
                ];
                let z = dot(dir, cam.fwd);
                if z <= 1e-3 {
                    continue;
                }
                let px = w * 0.5 + dot(dir, cam.right) / z * fs - 0.5;
                let py = half - dot(dir, cam.up) / z * fs - 0.5;
                if !(px >= 0.0 && py >= 0.0 && px < w - 1.0 && py < h - 1.0) {
                    continue;
                }
                let d2 = dob * dob;
                let k = p.stars * 16.0 * st.flux * mu * (d2 * d2).min(400.0);
                if k < 2e-3 {
                    // Below one grey level after tone mapping.
                    continue;
                }
                let col = self.bb_at(st.temp * dob);
                let lum = 0.2126 * col[0] + 0.7152 * col[1] + 0.0722 * col[2];
                let k = k / lum.max(1e-6);
                let (x0, y0) = (px as usize, py as usize);
                let (fx, fy) = (px - x0 as f32, py - y0 as f32);
                let wts = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
                let idx = [y0 * self.w + x0, y0 * self.w + x0 + 1, (y0 + 1) * self.w + x0, (y0 + 1) * self.w + x0 + 1];
                for q in 0..4 {
                    let i = idx[q];
                    let e = wts[q] * k * self.trans[i];
                    self.hr[i] += e * col[0];
                    self.hg[i] += e * col[1];
                    self.hb[i] += e * col[2];
                }
                drawn += 1;
            }
        }
        self.stats.stars = drawn;
    }

    /// Box blur (radius `r`) along rows of `src` (w × h) into `dst`.
    fn box_rows(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
        let k = 1.0 / (2 * r + 1) as f32;
        for y in 0..h {
            let row = &src[y * w..(y + 1) * w];
            let out = &mut dst[y * w..(y + 1) * w];
            let mut acc = row[0] * (r + 1) as f32;
            for i in 1..=r {
                acc += row[i.min(w - 1)];
            }
            for x in 0..w {
                out[x] = acc * k;
                acc += row[(x + r + 1).min(w - 1)] - row[x.saturating_sub(r)];
            }
        }
    }

    fn box_cols(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, acc: &mut [f32]) {
        let k = 1.0 / (2 * r + 1) as f32;
        let acc = &mut acc[..w];
        for (a, s) in acc.iter_mut().zip(&src[..w]) {
            *a = s * (r + 1) as f32;
        }
        for i in 1..=r {
            let o = i.min(h - 1) * w;
            for (a, s) in acc.iter_mut().zip(&src[o..o + w]) {
                *a += s;
            }
        }
        for y in 0..h {
            for (d, a) in dst[y * w..(y + 1) * w].iter_mut().zip(acc.iter()) {
                *d = a * k;
            }
            let add = (y + r + 1).min(h - 1) * w;
            let sub = y.saturating_sub(r) * w;
            for x in 0..w {
                acc[x] += src[add + x] - src[sub + x];
            }
        }
    }

    fn blur(buf: &mut [f32], tmp: &mut [f32], acc: &mut [f32], w: usize, h: usize, r: usize, passes: usize) {
        for _ in 0..passes {
            Self::box_rows(buf, tmp, w, h, r);
            Self::box_cols(tmp, buf, w, h, r, acc);
        }
    }

    fn bloom_and_sky(&mut self, p: &Params, cam: &Camera, az_off: f32) {
        let (w, h, bw, bh) = (self.w, self.h, self.bw, self.bh);
        // Bloom source: the previous frame's 4×4-downsampled HDR, which the
        // tone-map pass accumulated while it read the pixels anyway.
        let _ = (w, h);
        core::mem::swap(&mut self.lr, &mut self.ar);
        core::mem::swap(&mut self.lg, &mut self.ag);
        core::mem::swap(&mut self.lb, &mut self.ab);
        core::mem::swap(&mut self.lt, &mut self.at);
        self.ar.fill(0.0);
        self.ag.fill(0.0);
        self.ab.fill(0.0);
        self.at.fill(0.0);
        let k = 1.0 / 16.0;
        // Bloom: a tight and a wide lobe (iterated box ≈ gaussian).
        let strength = p.bloom.max(0.0);
        for c in 0..3 {
            let buf = match c {
                0 => &mut self.lr,
                1 => &mut self.lg,
                _ => &mut self.lb,
            };
            for v in buf.iter_mut() {
                *v *= k;
            }
            let wide = &mut self.wide[c];
            wide.copy_from_slice(buf);
            Self::blur(buf, &mut self.tmp, &mut self.acc, bw, bh, 1, 1);
            Self::blur(wide, &mut self.tmp, &mut self.acc, bw, bh, 8, 2);
            for (b, wv) in buf.iter_mut().zip(wide.iter()) {
                *b = strength * (0.05 * *b + 0.07 * wv);
            }
        }
        // Lensed sky (nebula and Milky-Way glow), occluded by the disk.
        let neb = p.nebula.max(0.0);
        if neb > 0.0 {
            let du = az_off * (PANO_W as f32 / TAU);
            for i in 0..bw * bh {
                let kk = self.sky_k[i] * neb * self.lt[i] * k / sqrtf(self.adapt);
                if kk <= 0.0 {
                    continue;
                }
                let mut u = self.sky_u[i] + du;
                u -= floorf(u / PANO_W as f32) * PANO_W as f32;
                let u0 = (u as usize).min(PANO_W - 1);
                let u1 = if u0 + 1 == PANO_W { 0 } else { u0 + 1 };
                let fu = u - u0 as f32;
                let v = self.sky_v[i];
                let v0 = v as usize;
                let fv = v - v0 as f32;
                let a = self.pano[v0 * PANO_W + u0];
                let b = self.pano[v0 * PANO_W + u1];
                let c = self.pano[(v0 + 1) * PANO_W + u0];
                let d = self.pano[(v0 + 1) * PANO_W + u1];
                let wa = (1.0 - fu) * (1.0 - fv);
                let wb = fu * (1.0 - fv);
                let wc = (1.0 - fu) * fv;
                let wd = fu * fv;
                self.lr[i] += kk * (a[0] * wa + b[0] * wb + c[0] * wc + d[0] * wd);
                self.lg[i] += kk * (a[1] * wa + b[1] * wb + c[1] * wc + d[1] * wd);
                self.lb[i] += kk * (a[2] * wa + b[2] * wb + c[2] * wc + d[2] * wd);
            }
        }
        // Relativistic jet: a faint bipolar glow along the spin axis.
        if p.jet > 0.0 {
            let half = h as f32 * 0.5;
            let fs = half / cam.tan_half;
            let proj = |pt: [f32; 3]| -> Option<(f32, f32)> {
                let rel = [pt[0] - cam.c[0] * p.distance, pt[1] - cam.c[1] * p.distance, pt[2] - cam.c[2] * p.distance];
                let z = dot(rel, cam.fwd);
                if z <= 0.1 {
                    return None;
                }
                Some(((w as f32 * 0.5 + dot(rel, cam.right) / z * fs) / 4.0, (half - dot(rel, cam.up) / z * fs) / 4.0))
            };
            let pulse = 0.85 + 0.15 * libm::sinf(p.time * 1.7);
            for sgn in [1.0f32, -1.0] {
                let mut prev = proj([0.0, 0.0, sgn * 3.0]);
                for s in 1..48 {
                    let zz = 3.0 + s as f32 * s as f32 * 0.09;
                    let cur = proj([0.0, 0.0, sgn * zz]);
                    if let (Some(a), Some(b)) = (prev, cur) {
                        let width = (0.6 + zz * 0.12) * fs / (p.distance + 1.0) / 4.0;
                        let inten = p.jet * 0.035 * pulse * exp_neg(zz * 0.05) * (1.0 + 0.4 * libm::sinf(zz * 0.35 - p.time * 3.0));
                        Self::stroke(&mut self.lr, &mut self.lg, &mut self.lb, bw, bh, a, b, width.max(0.7), inten);
                    }
                    prev = cur;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stroke(lr: &mut [f32], lg: &mut [f32], lb: &mut [f32], bw: usize, bh: usize, a: (f32, f32), b: (f32, f32), wd: f32, inten: f32) {
        let x0 = (a.0.min(b.0) - wd * 2.0).max(0.0) as usize;
        let x1 = ((a.0.max(b.0) + wd * 2.0).max(0.0) as usize).min(bw - 1);
        let y0 = (a.1.min(b.1) - wd * 2.0).max(0.0) as usize;
        let y1 = ((a.1.max(b.1) + wd * 2.0).max(0.0) as usize).min(bh - 1);
        if x0 > x1 || y0 > y1 || (x1 - x0) * (y1 - y0) > 40_000 {
            return;
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = (dx * dx + dy * dy).max(1e-6);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 - a.0, y as f32 - a.1);
                let t = ((px * dx + py * dy) / l2).clamp(0.0, 1.0);
                let (ex, ey) = (px - t * dx, py - t * dy);
                let d2 = (ex * ex + ey * ey) / (wd * wd);
                let v = inten * exp_neg(d2) * sqrtf(l2) / wd.max(1.0) * 0.5;
                let i = y * bw + x;
                lr[i] += v * 0.55;
                lg[i] += v * 0.7;
                lb[i] += v * 1.0;
            }
        }
    }

    fn upsample_row(&self, ly: usize, dst: &mut [f32]) {
        let (bw, w) = (self.bw, self.w);
        let o = ly * bw;
        let (dr, rest) = dst.split_at_mut(w);
        let (dg, db) = rest.split_at_mut(w);
        let (lr, lg, lb) = (&self.lr[o..o + bw], &self.lg[o..o + bw], &self.lb[o..o + bw]);
        for x in 0..w {
            let x0 = self.up_x0[x] as usize;
            let f = self.up_fx[x];
            let x1 = (x0 + 1).min(bw - 1);
            dr[x] = lr[x0] + (lr[x1] - lr[x0]) * f;
            dg[x] = lg[x0] + (lg[x1] - lg[x0]) * f;
            db[x] = lb[x0] + (lb[x1] - lb[x0]) * f;
        }
    }

    fn tonemap(&mut self, p: &Params) {
        let (w, h, bh) = (self.w, self.h, self.bh);
        let mut row_a = core::mem::take(&mut self.row_a);
        let mut row_b = core::mem::take(&mut self.row_b);
        let mut cached = (usize::MAX, usize::MAX);
        let ex = V4::splat(p.exposure.max(0.0) * 1.6 * self.adapt);
        let grain = p.grain.max(0.0) * 0.035;
        let seed = hash(self.frame.wrapping_mul(0x9e37_79b9));
        let srgb = &self.srgb[..];
        let gtab = &self.grain_tab[..];
        let sn = (SRGB_N - 1) as f32;
        let aces = |x: V4| ((x * (x * 2.51 + 0.03)) / (x * (x * 2.43 + 0.59) + 0.14)).clamp(0.0, 1.0);
        for y in 0..h {
            let ly = ((y as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (bh - 1) as f32);
            let l0 = (ly as usize).min(bh.saturating_sub(2));
            let l1 = (l0 + 1).min(bh - 1);
            let fy = V4::splat(ly - l0 as f32);
            if cached != (l0, l1) {
                if cached.1 == l0 {
                    core::mem::swap(&mut row_a, &mut row_b);
                } else {
                    self.upsample_row(l0, &mut row_a);
                }
                self.upsample_row(l1, &mut row_b);
                cached = (l0, l1);
            }
            let s = y * w;
            let (ar, rest) = row_a.split_at(w);
            let (ag, ab) = rest.split_at(w);
            let (br, rest) = row_b.split_at(w);
            let (bg, bb) = rest.split_at(w);
            let hr = &mut self.hr[s..s + w];
            let hg = &mut self.hg[s..s + w];
            let hb = &mut self.hb[s..s + w];
            let out = &mut self.out[s * 4..(s + w) * 4];
            let go = (hash(y as u32 ^ seed) & 4095) as usize;
            let lo = (y >> 2) * self.bw;
            let (acr, acg, acb, act) = (&mut self.ar[lo..lo + self.bw], &mut self.ag[lo..lo + self.bw], &mut self.ab[lo..lo + self.bw], &mut self.at[lo..lo + self.bw]);
            let trow = &mut self.trans[s..s + w];
            // The bloom source samples one row in four (it is blurred anyway).
            let acc_row = (y & 3) == 1 || (y + 1 == h && (y & 3) < 1);
            let g = V4::splat(grain);
            let mut x = 0;
            while x + 4 <= w {
                let v = |pl: &[f32]| V4::from_array([pl[x], pl[x + 1], pl[x + 2], pl[x + 3]]);
                let (vr, vg, vb) = (v(hr), v(hg), v(hb));
                let sum = |q: V4| {
                    let a = q.to_array();
                    (a[0] + a[1]) + (a[2] + a[3])
                };
                if acc_row {
                    let lx = x >> 2;
                    acr[lx] += sum(vr) * 4.0;
                    acg[lx] += sum(vg) * 4.0;
                    acb[lx] += sum(vb) * 4.0;
                    act[lx] += sum(v(trow)) * 4.0;
                }
                let r = aces((vr + v(ar) + (v(br) - v(ar)) * fy) * ex);
                let gg = aces((vg + v(ag) + (v(bg) - v(ag)) * fy) * ex);
                let b = aces((vb + v(ab) + (v(bb) - v(ab)) * fy) * ex);
                let gi = (go + x) & 4095;
                let n = V4::from_array([gtab[gi], gtab[gi + 1], gtab[gi + 2], gtab[gi + 3]]) * g;
                // Film grain, strongest in the mid-tones, then the sRGB curve
                // (1.138√v − 0.138v, with the linear toe; within ~1/255 above v = 0.02).
                let enc = |c: V4| {
                    let c = (c + n * ((c - c * c) * 3.0 + 0.15)).clamp(0.0, 1.0);
                    (c.sqrt() * 1.138 - c * 0.138).min(c * 12.92)
                };
                let q = V4::pack_rgb(enc(r), enc(gg), enc(b));
                let o = &mut out[x * 4..x * 4 + 16];
                for (lane, w) in q.iter().enumerate() {
                    o[lane * 4..lane * 4 + 4].copy_from_slice(&w.to_le_bytes());
                }
                x += 4;
            }
            let fy = fy.to_array()[0];
            while x < w {
                let vals = [hr[x] + ar[x] + (br[x] - ar[x]) * fy, hg[x] + ag[x] + (bg[x] - ag[x]) * fy, hb[x] + ab[x] + (bb[x] - ab[x]) * fy];
                let t = aces(V4::from_array([vals[0], vals[1], vals[2], 0.0]) * ex).to_array();
                for c in 0..3 {
                    out[x * 4 + c] = srgb[(t[c] * sn) as usize];
                }
                out[x * 4 + 3] = 255;
                x += 1;
            }
            hr.fill(0.0);
            hg.fill(0.0);
            hb.fill(0.0);
            trow.fill(1.0);
        }
        self.row_a = row_a;
        self.row_b = row_b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn far(psi_max: f32) -> BlackHole {
        let mut bh = BlackHole::new(8, 8, 1);
        bh.build_table(1000.0, 0.0, psi_max);
        bh
    }

    #[test]
    fn photon_sphere_and_shadow_edge() {
        // Far away, the shadow edge sits at b = 3√3 M: ψ_c ≈ b/r.
        let bh = far(0.02);
        let psi_c = bh.shadow_radius();
        let b = psi_c * 1000.0;
        assert!((b - B_CRIT).abs() < 0.02, "shadow edge b = {b}");
        // Just outside the critical ray the orbit skims r ≈ 3M.
        let j = bh.first_escape;
        let mut umax = 0.0f32;
        for c in 0..COLS {
            umax = umax.max(bh.table[j * COLS + c]);
        }
        assert!((1.0 / umax - 3.0).abs() < 0.15, "periapsis {}", 1.0 / umax);
    }

    #[test]
    fn shadow_at_finite_distance() {
        // Static observer at r: sin ψ_c = (b_c / r) √(1 − 2/r) (Synge 1966).
        for r in [10.0f32, 30.0] {
            let mut bh = BlackHole::new(8, 8, 1);
            bh.build_table(r, 0.0, 1.0);
            let exact = libm::asinf(B_CRIT / r * sqrtf(1.0 - 2.0 / r));
            assert!((bh.shadow_radius() - exact).abs() < 2e-3, "r={r}: {} vs {exact}", bh.shadow_radius());
        }
    }

    #[test]
    fn isco_and_flux_profile() {
        assert_eq!(page_thorne_flux(6.0), 0.0);
        assert!(page_thorne_flux(6.01) > 0.0);
        // Peak of the Page–Thorne flux near 9.55 M.
        let mut best = (0.0, 0.0);
        let mut r = 6.0;
        while r < 30.0 {
            let f = page_thorne_flux(r);
            if f > best.1 {
                best = (r, f);
            }
            r += 0.01;
        }
        assert!((best.0 - 9.55).abs() < 0.1, "peak at {}", best.0);
    }

    #[test]
    fn redshift_matches_analytic() {
        // Face-on (L_z = 0): pure gravitational + transverse Doppler √(1 − 3M/r).
        for r in [6.0f32, 10.0, 20.0] {
            assert!((redshift(r, 0.0, 1.0) - sqrtf(1.0 - 3.0 / r)).abs() < 1e-5);
        }
        // Edge-on, photon emitted along the orbit at large distance: b = ±r/√(1−2/r)… use the
        // exact form g = √(1−3/r)/(1 − Ω b) with Ω = r^(−3/2).
        let r = 10.0f32;
        let b = 7.0;
        let g = redshift(r, b, 1.0);
        let exact = (1.0 - 3.0 / r).sqrt() / (1.0 - b / r.powf(1.5));
        assert!((g - exact).abs() < 1e-5);
        assert!(redshift(r, -b, 1.0) < redshift(r, 0.0, 1.0));
        assert!(redshift(r, b, 1.0) > redshift(r, 0.0, 1.0));
    }

    #[test]
    fn raindrop_matches_static_plus_aberration() {
        // fall = 1 just outside the horizon: PG formulas agree with the
        // static-frame formulas boosted by v = √(2M/r).
        let u0 = 1.0 / 5.0;
        for psi in [0.3f32, 1.0, 2.0] {
            let a = ray_init(psi, u0, 1.0).unwrap();
            let b = ray_init(psi, u0, 0.9998).unwrap();
            assert!((a.0 - b.0).abs() < 2e-3 && (a.1 - b.1).abs() < 2e-2 && (a.2 - b.2).abs() < 2e-3, "{a:?} {b:?}");
        }
        // Inside the horizon, looking straight down: negative Killing energy.
        assert!(ray_init(0.01, 1.0 / 1.5, 1.0).is_none());
        // Looking straight up: the outside sky, blueshifted.
        let (w, _, d) = ray_init(3.1, 1.0 / 1.5, 1.0).unwrap();
        assert!(w < 0.0 && d > 0.0);
    }

    #[test]
    fn fast_log2_is_continuous_and_accurate() {
        let mut x = 0.01f32;
        while x < 1.0e6 {
            assert!((fast_log2(x) - libm::log2f(x)).abs() < 2e-5, "{x}");
            x *= 1.003;
        }
        for (a, b) in [(0.9999, 1.0001), (8191.9, 8192.1)] {
            assert!((fast_log2(b) - fast_log2(a)).abs() < 1e-3);
        }
        let (s, c) = sincosf(2.5);
        assert!((s - libm::sinf(2.5)).abs() < 1e-5 && (c - libm::cosf(2.5)).abs() < 1e-5);
        let (s, c) = sincosf(-7.0);
        assert!((s - libm::sinf(-7.0)).abs() < 1e-5 && (c - libm::cosf(-7.0)).abs() < 1e-5);
    }

    #[test]
    fn blackbody_colours() {
        let c = blackbody(6500.0);
        let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!((y - 1.0).abs() < 1e-3);
        let red = blackbody(2500.0);
        assert!(red[0] > red[2] * 3.0);
        let blue = blackbody(20000.0);
        assert!(blue[2] > blue[0]);
    }

    #[test]
    fn turbulence_is_continuous_in_time() {
        let mut bh = BlackHole::new(8, 8, 3);
        let p = Params::default();
        bh.build_disk(&p, 100.0, 0.0);
        let a = bh.disk.clone();
        bh.build_disk(&p, 100.005, 0.0);
        let mut max = 0.0f32;
        for (x, y) in a.iter().zip(bh.disk.iter()) {
            max = max.max((x - y).abs());
        }
        assert!(max < 0.05, "max change {max}");
        // ...and crosses a turbulence reset smoothly too.
        let mut prev = {
            bh.build_disk(&p, 34.0, 0.0);
            bh.disk.clone()
        };
        let mut t = 34.0;
        while t < 36.5 {
            t += 0.005;
            bh.build_disk(&p, t, 0.0);
            let m = prev.iter().zip(bh.disk.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
            assert!(m < 0.05, "jump {m} at t={t}");
            prev = bh.disk.clone();
        }
    }

    #[test]
    fn renders_shadow_and_disk() {
        let mut bh = BlackHole::new(160, 100, 1);
        let p = Params { distance: 30.0, inclination: 80.0, fov: 40.0, ..Default::default() };
        bh.render(&p);
        assert!(bh.stats.samples[0] > 1000 && bh.stats.samples[1] > 50, "{:?}", bh.stats);
        // The centre of the shadow is black.
        let c = (36 * 160 + 80) * 4;
        assert!(bh.out[c] < 12 && bh.out[c + 1] < 12, "{:?}", &bh.out[c..c + 4]);
        let lit = bh.out.chunks(4).filter(|p| p[0] > 100).count();
        assert!(lit > 500, "lit {lit}");
        // Second frame reuses the geometry.
        bh.render(&Params { time: 1.0, ..p });
        assert_eq!(bh.stats.geometry_built, 0);
        // Inside the horizon, looking back out.
        bh.render(&Params { distance: 1.5, fall: 1.0, look: 180.0, fov: 90.0, ..p });
        assert!(bh.out.chunks(4).any(|p| p[0] > 30));
    }
}
