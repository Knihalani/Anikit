//! Cinematic post and film look, applied to the finished frame in linear light.
//!
//! Order:
//! 1. lens distortion and chromatic aberration (one resample);
//! 2. bloom (soft-knee bright pass, wide multi-scale pyramid) and halation
//!    (warm glow around highlights, as film's red layer scatters);
//! 3. exposure, white balance, lift/gamma/gain, contrast and saturation;
//! 4. tone map (a filmic shoulder, or ACES);
//! 5. vignette;
//! 6. animated film grain;
//! 7. dithered 8-bit output, so dark gradients don't band.
//!
//! Presets: `none`, `clean`, `cinematic`, `dreamy`, `punchy`, `noir`,
//! `vintage`. A spec gives either a preset name or an object with
//! `preset` plus any field to override (see [`PostSpec`]).

use serde::{Deserialize, Serialize};

/// Tone-mapping curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tonemap {
    /// Clip at white.
    None,
    /// Identity up to white; values pushed past white (bloom) roll off towards white.
    #[default]
    Soft,
    /// ACES filmic fit (Narkowicz): richer contrast, desaturated highlights.
    Aces,
}

/// Every post parameter. All effects are off at their zero/neutral values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PostSettings {
    /// Bloom strength (0 = off, 0.3 subtle, 1 strong).
    pub bloom: f32,
    /// Luminance (linear, 0..1+) where bloom starts.
    pub bloom_threshold: f32,
    /// Soft knee width around the threshold.
    pub bloom_knee: f32,
    /// Bloom spread as a fraction of the frame height (0.05 tight … 0.4 wide).
    pub bloom_radius: f32,
    /// Bloom colour multiplier.
    pub bloom_tint: [f32; 3],
    /// Halation: warm red-orange glow around bright edges (0 = off, 0.3 subtle).
    pub halation: f32,
    /// Chromatic aberration: red/blue split at the frame corners, in pixels at 1080p.
    pub aberration: f32,
    /// Lens distortion: barrel (> 0) or pincushion (< 0), 0.05 subtle.
    pub distortion: f32,
    /// Exposure in stops.
    pub exposure: f32,
    /// White balance: warm (> 0) / cool (< 0), and green (> 0) / magenta (< 0).
    pub temperature: f32,
    pub tint: f32,
    /// ASC-CDL-style lift (shadows offset), gamma (midtones), gain (highlights), per channel.
    pub lift: [f32; 3],
    pub gamma: [f32; 3],
    pub gain: [f32; 3],
    /// Contrast around mid grey (1 = none) and saturation (1 = none).
    pub contrast: f32,
    pub saturation: f32,
    pub tonemap: Tonemap,
    /// Vignette strength (0..1) and softness (0.2 hard … 1 soft).
    pub vignette: f32,
    pub vignette_softness: f32,
    /// Animated film grain amount (0 = off, 0.04 subtle, 0.1 heavy) and grain size in pixels.
    pub grain: f32,
    pub grain_size: f32,
    /// Dither the 8-bit output (on by default: removes banding).
    pub dither: bool,
    /// A 3D LUT applied after tone mapping: `anikit:lut/<look>` (`assets list --type lut`),
    /// another library ref, a `.cube` path, or the film model's own parameters
    /// (`{"base": "teal-orange", "contrast": 0.6, "shadows": [0, 0.01, 0.03], …}`).
    pub lut: Option<serde_json::Value>,
    /// How much of the LUT (0..1).
    pub lut_mix: f32,
}

impl Default for PostSettings {
    fn default() -> Self {
        Self {
            bloom: 0.0,
            bloom_threshold: 0.75,
            bloom_knee: 0.25,
            bloom_radius: 0.15,
            bloom_tint: [1.0, 1.0, 1.0],
            halation: 0.0,
            aberration: 0.0,
            distortion: 0.0,
            exposure: 0.0,
            temperature: 0.0,
            tint: 0.0,
            lift: [0.0; 3],
            gamma: [1.0; 3],
            gain: [1.0; 3],
            contrast: 1.0,
            saturation: 1.0,
            tonemap: Tonemap::Soft,
            vignette: 0.0,
            vignette_softness: 0.6,
            grain: 0.0,
            grain_size: 1.3,
            dither: true,
            lut: None,
            lut_mix: 1.0,
        }
    }
}

/// Preset names, for errors and docs.
pub const PRESETS: &[&str] = &["none", "clean", "cinematic", "dreamy", "punchy", "noir", "vintage"];

impl PostSettings {
    /// A named look.
    pub fn preset(name: &str) -> Option<Self> {
        let d = Self::default();
        Some(match name {
            "none" => Self { tonemap: Tonemap::None, dither: false, ..d },
            // Just the quality fixes: dither and a soft highlight shoulder.
            "clean" => d,
            "cinematic" => Self {
                bloom: 0.35,
                bloom_threshold: 0.7,
                bloom_radius: 0.18,
                halation: 0.18,
                aberration: 1.2,
                distortion: 0.02,
                lift: [0.004, 0.006, 0.012],
                gain: [1.02, 1.0, 0.97],
                contrast: 1.06,
                saturation: 1.04,
                vignette: 0.32,
                grain: 0.035,
                ..d
            },
            "dreamy" => Self {
                bloom: 0.7,
                bloom_threshold: 0.55,
                bloom_knee: 0.35,
                bloom_radius: 0.3,
                bloom_tint: [1.0, 0.92, 1.0],
                halation: 0.3,
                lift: [0.008, 0.006, 0.012],
                contrast: 0.96,
                saturation: 0.95,
                vignette: 0.2,
                grain: 0.02,
                ..d
            },
            "punchy" => Self {
                bloom: 0.25,
                bloom_threshold: 0.8,
                bloom_radius: 0.12,
                contrast: 1.18,
                saturation: 1.2,
                exposure: 0.1,
                vignette: 0.2,
                grain: 0.015,
                ..d
            },
            "noir" => Self {
                bloom: 0.2,
                halation: 0.0,
                contrast: 1.3,
                saturation: 0.0,
                vignette: 0.55,
                vignette_softness: 0.5,
                grain: 0.07,
                grain_size: 1.6,
                tonemap: Tonemap::Aces,
                exposure: 0.3,
                ..d
            },
            "vintage" => Self {
                bloom: 0.3,
                halation: 0.35,
                aberration: 1.8,
                distortion: 0.04,
                temperature: 0.35,
                lift: [0.012, 0.01, 0.003],
                gain: [1.0, 0.97, 0.88],
                contrast: 0.95,
                saturation: 0.8,
                vignette: 0.45,
                grain: 0.06,
                grain_size: 1.8,
                ..d
            },
            _ => return None,
        })
    }

    /// Whether this changes anything at all (for an 8-bit frame).
    pub fn is_identity(&self) -> bool {
        self.bloom == 0.0
            && self.halation == 0.0
            && self.aberration == 0.0
            && self.distortion == 0.0
            && self.exposure == 0.0
            && self.temperature == 0.0
            && self.tint == 0.0
            && self.lift == [0.0; 3]
            && self.gamma == [1.0; 3]
            && self.gain == [1.0; 3]
            && self.contrast == 1.0
            && self.saturation == 1.0
            && self.tonemap != Tonemap::Aces
            && self.vignette == 0.0
            && self.grain == 0.0
            && (self.lut.is_none() || self.lut_mix <= 0.0)
        // An 8-bit input stays within 0..1 and on the 8-bit grid without the
        // effects above, so the soft shoulder and dither change nothing.
    }
}

/// How a spec writes `post`: a preset name, or a preset plus overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PostSpec {
    Preset(String),
    Custom(Box<PostCustom>),
}

/// `{"preset": "cinematic", "bloom": 0.5, …}`: any [`PostSettings`] field overrides the preset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct PostCustom {
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl PostSpec {
    /// Resolve to settings, or an error naming the unknown preset or field.
    pub fn resolve(&self) -> Result<PostSettings, String> {
        let base = |name: &str| {
            PostSettings::preset(name).ok_or_else(|| format!("unknown post preset \"{name}\" (presets: {})", PRESETS.join(", ")))
        };
        match self {
            PostSpec::Preset(name) => base(name),
            PostSpec::Custom(c) => {
                let start = base(c.preset.as_deref().unwrap_or("clean"))?;
                let mut v = serde_json::to_value(start).map_err(|e| e.to_string())?;
                let obj = v.as_object_mut().ok_or("post settings")?;
                for (k, val) in &c.fields {
                    if !obj.contains_key(k) {
                        return Err(format!("unknown post field \"{k}\""));
                    }
                    obj.insert(k.clone(), val.clone());
                }
                let s: PostSettings = serde_json::from_value(v).map_err(|e| format!("post: {e}"))?;
                if let Some(l) = &s.lut {
                    crate::assetlib::lut::load_value(l).map_err(|e| format!("post.lut: {e}"))?;
                }
                Ok(s)
            }
        }
    }
}

// ------------------------------------------------------------------ processing

pub(crate) fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Linear 0..1 → sRGB 0..255 through a shared table (sqrt-indexed for dark precision).
#[inline(always)]
pub(crate) fn encode_srgb255(v: f32) -> f32 {
    static T: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let lut = T.get_or_init(|| {
        (0..=SRGB_LUT)
            .map(|i| {
                let t = i as f32 / SRGB_LUT as f32;
                linear_to_srgb(t * t) * 255.0
            })
            .collect()
    });
    PostProcessor::encode(lut, v)
}

/// Triangular dither in −1..1 LSB, fixed per pixel.
#[inline(always)]
pub(crate) fn tri_dither(x: u32, y: u32) -> f32 {
    hash01(x, y, 0x1234_5678) + hash01(x, y, 0x9abc_def0) - 1.0
}

/// Integer hash → uniform 0..1.
#[inline(always)]
fn hash01(x: u32, y: u32, s: u32) -> f32 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ s.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Scratch for [`PostProcessor::apply`], reused across frames.
#[derive(Default)]
pub struct PostProcessor {
    to_lin: Vec<f32>,
    to_srgb: Vec<f32>,
    img: Vec<[f32; 3]>,
    tmp: Vec<[f32; 3]>,
    pyr: Vec<(usize, usize, Vec<[f32; 3]>)>,
    grade: [Vec<f32>; 3],
    /// Bloom + halation at half resolution (empty when off), and its size.
    glow: Vec<[f32; 3]>,
    glow_dims: (usize, usize),
    /// `tri_dither` for every pixel, and the size it was built for.
    dith: Vec<f32>,
    dith_dims: (usize, usize),
}

const SRGB_LUT: usize = 4096;
const GRADE_LUT: usize = 4096;
/// Largest linear value the grade table covers (bloom overshoot).
const GRADE_MAX: f32 = 16.0;


impl PostProcessor {
    fn luts(&mut self) {
        if self.to_lin.is_empty() {
            self.to_lin = (0..256).map(|i| srgb_to_linear(i as f32 / 255.0)).collect();
            // Linear 0..1 → sRGB 0..255, sampled densely (sqrt-spaced for dark precision).
            self.to_srgb = (0..=SRGB_LUT).map(|i| {
                let t = i as f32 / SRGB_LUT as f32;
                linear_to_srgb(t * t) * 255.0
            }).collect();
        }
    }

    #[inline(always)]
    fn encode(lut: &[f32], v: f32) -> f32 {
        // Index by sqrt(v) so dark values get more table entries.
        let s = v.clamp(0.0, 1.0).sqrt() * SRGB_LUT as f32;
        let i = (s as usize).min(SRGB_LUT - 1);
        let f = s - i as f32;
        lut[i] + (lut[i + 1] - lut[i]) * f
    }

    /// Apply `p` to an opaque RGBA8 frame (`w × h`) in place. `frame` seeds
    /// the animated grain (use the frame index, so output is deterministic).
    pub fn apply(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &PostSettings, frame: u64) {
        if p.is_identity() || w == 0 || h == 0 {
            return;
        }
        self.luts();
        let n = w * h;
        // 1. Decode to linear.
        {
            use rayon::prelude::*;
            self.img.resize(n, [0.0; 3]);
            let lin = &self.to_lin;
            self.img.par_chunks_mut(w).zip(rgba.par_chunks(w * 4)).for_each(|(d, s)| {
                for (d, px) in d.iter_mut().zip(s.chunks_exact(4)) {
                    *d = [lin[px[0] as usize], lin[px[1] as usize], lin[px[2] as usize]];
                }
            });
        }
        let scale1080 = h as f32 / 1080.0;
        // 2. Lens distortion + chromatic aberration.
        if p.distortion != 0.0 || p.aberration != 0.0 {
            self.lens(w, h, p.distortion, p.aberration * scale1080);
        }
        // 3. Bloom + halation (into `glow`, added in the final pass).
        self.glow.clear();
        if p.bloom > 0.0 || p.halation > 0.0 {
            self.bloom(w, h, p);
        }
        // 4–7: grade, tone map, vignette, grain, dither, encode.
        // The per-channel part of the grade (exposure, white balance,
        // lift/gain, gamma, contrast) is one 1-D curve per channel: tabulated.
        let ev = 2f32.powf(p.exposure);
        let wb = [1.0 + 0.12 * p.temperature - 0.03 * p.tint, 1.0 + 0.06 * p.tint, 1.0 - 0.12 * p.temperature - 0.03 * p.tint];
        let inv_gamma = p.gamma.map(|g| 1.0 / g.max(0.05));
        let curve = |k: usize, v: f32| -> f32 {
            let v = v * ev * wb[k];
            let v = (p.gain[k] * (v + p.lift[k] * (1.0 - v))).max(0.0);
            let v = if inv_gamma[k] != 1.0 { v.powf(inv_gamma[k]) } else { v };
            if p.contrast != 1.0 { 0.18 * (v.max(1e-6) / 0.18).powf(p.contrast) } else { v }
        };
        // Domain 0..GRADE_MAX (bloom can push past white), indexed by sqrt.
        for k in 0..3 {
            self.grade[k].clear();
            self.grade[k].extend((0..=GRADE_LUT).map(|i| {
                let t = i as f32 / GRADE_LUT as f32;
                curve(k, t * t * GRADE_MAX)
            }));
        }
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let inv_diag = 1.0 / (cx * cx + cy * cy).sqrt();
        let soft = p.vignette_softness.clamp(0.05, 2.0);
        let seed = (frame as u32).wrapping_mul(0x9e37_79b9) ^ 0x51ed_270b;
        let gsize = p.grain_size.max(1.0);
        let grain_amp = p.grain * 255.0;
        // Static per-pixel tables, rebuilt only when the size changes: the fixed
        // dither and the glow grid's column taps (same values as computing them
        // per pixel).
        if self.dith_dims != (w, h) {
            self.dith.clear();
            self.dith.extend((0..h).flat_map(|y| (0..w).map(move |x| tri_dither(x as u32, y as u32))));
            self.dith_dims = (w, h);
        }
        let (glow, (gw, gh)) = (&self.glow, self.glow_dims);
        let gcols: Vec<(usize, usize, f32)> = if glow.is_empty() {
            Vec::new()
        } else {
            (0..w)
                .map(|x| {
                    let sx = ((x as f32 + 0.5) * gw as f32 / w as f32 - 0.5).clamp(0.0, (gw - 1) as f32);
                    let x0 = sx as usize;
                    (x0, (x0 + 1).min(gw - 1), sx - x0 as f32)
                })
                .collect()
        };
        let (lut, grade, img, dith) = (&self.to_srgb[..], &self.grade, &self.img, &self.dith);
        let (sat, tonemap, vig, dither) = (p.saturation, p.tonemap, p.vignette, p.dither);
        // Checked when the spec was resolved; a LUT that fails now is skipped.
        let grade_lut = p.lut.as_ref().filter(|_| p.lut_mix > 0.0).and_then(|l| crate::assetlib::lut::load_value(l).ok());
        let lut_mix = p.lut_mix.clamp(0.0, 1.0);
        let enc = |v: f32| -> f32 {
            let s = v.clamp(0.0, 1.0).sqrt() * SRGB_LUT as f32;
            let i = (s as usize).min(SRGB_LUT - 1);
            let f = s - i as f32;
            // SAFETY: i + 1 ≤ SRGB_LUT and the table has SRGB_LUT + 1 entries.
            let (a, b) = unsafe { (*lut.get_unchecked(i), *lut.get_unchecked(i + 1)) };
            a + (b - a) * f
        };
        let look = |t: &[f32], v: f32| -> f32 {
            let s = (v.max(0.0) / GRADE_MAX).sqrt().min(1.0) * GRADE_LUT as f32;
            let i = (s as usize).min(GRADE_LUT - 1);
            let f = s - i as f32;
            // SAFETY: i + 1 ≤ GRADE_LUT and the table has GRADE_LUT + 1 entries.
            let (a, b) = unsafe { (*t.get_unchecked(i), *t.get_unchecked(i + 1)) };
            a + (b - a) * f
        };
        let (g0, g1, g2) = (&grade[0][..], &grade[1][..], &grade[2][..]);
        use rayon::prelude::*;
        if grade_lut.is_none() && !SCALAR_FINAL.with(|c| c.get()) {
            // The common case: planar, chunked stages that LLVM vectorises. Same
            // arithmetic, in the same order, as the per-pixel loop below (bit-exact;
            // `final_pass_matches_scalar` checks it).
            let fc = FinalCtx { w, h, gw, gh, glow, gcols: &gcols, img, dith, lut, grade: [g0, g1, g2], sat, tonemap, vig, soft, cx, cy, inv_diag, grain_amp, gsize, seed, dither };
            rgba.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| final_row(&fc, y, row));
            return;
        }
        rgba.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            let src = &img[y * w..(y + 1) * w];
            let drow = &dith[y * w..(y + 1) * w];
            let dy = (y as f32 + 0.5 - cy) * inv_diag;
            // Glow rows for this output row (same arithmetic as `sample_grid`).
            let grow = (!glow.is_empty()).then(|| {
                let sy = ((y as f32 + 0.5) * gh as f32 / h as f32 - 0.5).clamp(0.0, (gh - 1) as f32);
                let y0 = sy as usize;
                (&glow[y0 * gw..(y0 + 1) * gw], &glow[(y0 + 1).min(gh - 1) * gw..((y0 + 1).min(gh - 1) + 1) * gw], sy - y0 as f32)
            });
            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                let mut v = src[x];
                if let Some((ra, rb, ty)) = grow {
                    let (x0, x1, tx) = gcols[x];
                    let (a, b, c, d) = (ra[x0], ra[x1], rb[x0], rb[x1]);
                    for k in 0..3 {
                        let top = a[k] + (b[k] - a[k]) * tx;
                        let bot = c[k] + (d[k] - c[k]) * tx;
                        v[k] += top + (bot - top) * ty;
                    }
                }
                let mut c = [look(g0, v[0]), look(g1, v[1]), look(g2, v[2])];
                if sat != 1.0 {
                    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                    for v in &mut c {
                        *v = (l + (*v - l) * sat).max(0.0);
                    }
                }
                match tonemap {
                    Tonemap::None => {}
                    Tonemap::Soft => {
                        // Identity up to white; overshoot (bloom) rolls off towards
                        // white, hue-preserving, instead of clipping per channel.
                        let m = c[0].max(c[1]).max(c[2]);
                        if m > 1.0 {
                            let excess = (1.0 - 1.0 / m) * 0.6;
                            for v in &mut c {
                                let n = *v / m;
                                *v = n + (1.0 - n) * excess;
                            }
                        }
                    }
                    Tonemap::Aces => {
                        for v in &mut c {
                            // Narkowicz fit, pre-scaled so mid grey stays put.
                            let x = *v * 0.8;
                            *v = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
                        }
                    }
                }
                if vig > 0.0 {
                    let dx = (x as f32 + 0.5 - cx) * inv_diag;
                    let r = (dx * dx + dy * dy).sqrt() * std::f32::consts::SQRT_2;
                    let t = ((r - (1.0 - soft)) / soft).clamp(0.0, 1.0);
                    let m = 1.0 - vig * t * t * (3.0 - 2.0 * t);
                    for v in &mut c {
                        *v *= m;
                    }
                }
                // Encode, then grain (strongest in the midtones) and dither in 8-bit units.
                let mut o = [enc(c[0]), enc(c[1]), enc(c[2])];
                if let Some(l) = &grade_lut {
                    let g = l.sample([o[0] / 255.0, o[1] / 255.0, o[2] / 255.0]);
                    for k in 0..3 {
                        o[k] += (g[k] * 255.0 - o[k]) * lut_mix;
                    }
                }
                if grain_amp > 0.0 {
                    let (gx, gy) = ((x as f32 / gsize) as u32, (y as f32 / gsize) as u32);
                    let g = (hash01(gx, gy, seed) + hash01(gx, gy, seed ^ 0x68e3_1da4) - 1.0) * grain_amp;
                    let l = (0.2126 * o[0] + 0.7152 * o[1] + 0.0722 * o[2]) / 255.0;
                    let wgt = 0.35 + 2.6 * l * (1.0 - l);
                    for v in &mut o {
                        *v += g * wgt;
                    }
                }
                if dither {
                    // Triangular noise, ±1 LSB, fixed per pixel (static regions stay static).
                    let d = drow[x];
                    for v in &mut o {
                        *v += d;
                    }
                }
                px[0] = (o[0] + 0.5).clamp(0.0, 255.0) as u8;
                px[1] = (o[1] + 0.5).clamp(0.0, 255.0) as u8;
                px[2] = (o[2] + 0.5).clamp(0.0, 255.0) as u8;
                px[3] = 255;
            }
        });
        let _ = n;
    }


    /// Barrel distortion and radial chromatic aberration in one bilinear resample.
    fn lens(&mut self, w: usize, h: usize, k: f32, ca_px: f32) {
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let inv = 1.0 / (cx * cx + cy * cy).sqrt();
        // Zoom so the corners still sample inside the frame.
        let zoom = 1.0 / (1.0 + k.max(0.0));
        let ca = ca_px * inv; // fraction of the half-diagonal at the corner
        // Every pixel is written below: only the size matters.
        self.tmp.resize(w * h, [0.0; 3]);
        let img = &self.img[..w * h];
        let (wm, hm) = ((w - 1) as f32, (h - 1) as f32);
        // Bilinear sample of channel `ch`, clamped to the frame. Same arithmetic
        // as a checked version; the clamps keep every index in range.
        #[inline(always)]
        fn sample(img: &[[f32; 3]], w: usize, wm: f32, hm: f32, sx: f32, sy: f32, ch: usize) -> f32 {
            let sx = sx.clamp(0.0, wm);
            let sy = sy.clamp(0.0, hm);
            let (x0, y0) = (sx as usize, sy as usize);
            let x1 = if (x0 as f32) < wm { x0 + 1 } else { x0 };
            let y1 = if (y0 as f32) < hm { y0 + 1 } else { y0 };
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            // SAFETY: x0, x1 ≤ w − 1 and y0, y1 ≤ h − 1 (clamped above); img holds w·h pixels.
            let (p00, p01, p10, p11) = unsafe {
                (
                    img.get_unchecked(y0 * w + x0)[ch],
                    img.get_unchecked(y0 * w + x1)[ch],
                    img.get_unchecked(y1 * w + x0)[ch],
                    img.get_unchecked(y1 * w + x1)[ch],
                )
            };
            let a = p00 + (p01 - p00) * fx;
            let b = p10 + (p11 - p10) * fx;
            a + (b - a) * fy
        }
        use rayon::prelude::*;
        if !SCALAR_FINAL.with(|c| c.get()) {
            let lc = LensCtx { img, w, wm, hm, cx, cy, inv, k, zoom, ca };
            self.tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| lens_row(&lc, y, row));
            std::mem::swap(&mut self.img, &mut self.tmp);
            return;
        }
        self.tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let dy = y as f32 + 0.5 - cy;
            for (x, out) in row.iter_mut().enumerate() {
                let dx = x as f32 + 0.5 - cx;
                let r2 = (dx * dx + dy * dy) * inv * inv;
                let f = (1.0 + k * r2) * zoom;
                let cr = ca * r2.sqrt();
                for (ch, s) in [(0usize, 1.0 + cr), (1, 1.0), (2, 1.0 - cr)] {
                    let m = f * s;
                    out[ch] = sample(img, w, wm, hm, cx + dx * m - 0.5, cy + dy * m - 0.5, ch);
                }
            }
        });
        std::mem::swap(&mut self.img, &mut self.tmp);
    }

    /// Soft-knee bright pass at half resolution, a 2× pyramid down to the bloom
    /// radius, and a tent-filtered upsample chain; adds bloom and halation.
    fn bloom(&mut self, w: usize, h: usize, p: &PostSettings) {
        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
        let (t, knee) = (p.bloom_threshold.max(0.0), p.bloom_knee.max(1e-4));
        use rayon::prelude::*;
        let mut base = vec![[0f32; 3]; hw * hh];
        let img = &self.img;
        if !SCALAR_FINAL.with(|c| c.get()) {
            base.par_chunks_mut(hw).enumerate().for_each(|(y, row)| bright_row(img, w, h, y, t, knee, row));
        } else {
            base.par_chunks_mut(hw).enumerate().for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                let mut acc = [0f32; 3];
                for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + ox).min(w - 1), (2 * y + oy).min(h - 1));
                    let c = img[sy * w + sx];
                    for k in 0..3 {
                        acc[k] += c[k] * 0.25;
                    }
                }
                let l = 0.2126 * acc[0] + 0.7152 * acc[1] + 0.0722 * acc[2];
                let s = (l - t + knee).clamp(0.0, 2.0 * knee);
                let s = s * s / (4.0 * knee);
                let c = s.max(l - t) / l.max(1e-5);
                *out = acc.map(|v| v * c.max(0.0));
            }
            });
        }
        // Pyramid: levels until the blur reaches the radius.
        let target = (p.bloom_radius.max(0.01) * h as f32).max(4.0);
        let levels = ((target / 2.0).log2().ceil() as usize).clamp(2, 9);
        self.pyr.clear();
        self.pyr.push((hw, hh, base));
        for _ in 1..levels {
            let (pw, ph, prev) = self.pyr.last().unwrap();
            let (pw, ph) = (*pw, *ph);
            if pw < 4 || ph < 4 {
                break;
            }
            let (nw, nh) = (pw.div_ceil(2), ph.div_ceil(2));
            let mut next = vec![[0f32; 3]; nw * nh];
            next.par_chunks_mut(nw).enumerate().for_each(|(y, row)| {
                for (x, out) in row.iter_mut().enumerate() {
                    // 4×4 tent-ish downsample (taps at ±1.5 px around the centre).
                    let mut acc = [0f32; 3];
                    let mut wsum = 0.0;
                    for oy in -1i32..=2 {
                        for ox in -1i32..=2 {
                            let wgt = if (0..=1).contains(&ox) { 3.0 } else { 1.0 } * if (0..=1).contains(&oy) { 3.0 } else { 1.0 };
                            let sx = (2 * x as i32 + ox).clamp(0, pw as i32 - 1) as usize;
                            let sy = (2 * y as i32 + oy).clamp(0, ph as i32 - 1) as usize;
                            let c = prev[sy * pw + sx];
                            for k in 0..3 {
                                acc[k] += c[k] * wgt;
                            }
                            wsum += wgt;
                        }
                    }
                    *out = acc.map(|v| v / wsum);
                }
            });
            self.pyr.push((nw, nh, next));
        }
        // Upsample chain: each level += bilinear(up(next)); keep a mid level for halation.
        let up = |src: &[[f32; 3]], sw: usize, sh: usize, x: usize, y: usize| -> [f32; 3] {
            let fx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (sw - 1) as f32);
            let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (sh - 1) as f32);
            let (x0, y0) = (fx as usize, fy as usize);
            let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            let mut o = [0f32; 3];
            for k in 0..3 {
                let a = src[y0 * sw + x0][k] + (src[y0 * sw + x1][k] - src[y0 * sw + x0][k]) * tx;
                let b = src[y1 * sw + x0][k] + (src[y1 * sw + x1][k] - src[y1 * sw + x0][k]) * tx;
                o[k] = a + (b - a) * ty;
            }
            o
        };
        let nl = self.pyr.len();
        let hal_level = (nl / 2).max(1).min(nl - 1);
        let mut halo: Option<(usize, usize, Vec<[f32; 3]>)> = None;
        for i in (0..nl - 1).rev() {
            let (lo, hi) = self.pyr.split_at_mut(i + 1);
            let (dw, _dh, dst) = &mut lo[i];
            let (sw, sh, small) = (&hi[0].0, &hi[0].1, &hi[0].2);
            let (sw, sh, dw) = (*sw, *sh, *dw);
            if !SCALAR_FINAL.with(|c| c.get()) {
                // Same taps as `up`, computed once per column and per row.
                let cols = taps(dw, |x| ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (sw - 1) as f32), sw);
                dst.par_chunks_mut(dw).enumerate().for_each(|(y, row)| {
                    let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (sh - 1) as f32);
                    let y0 = fy as usize;
                    let y1 = (y0 + 1).min(sh - 1);
                    lerp_add_row(&small[y0 * sw..(y0 + 1) * sw], &small[y1 * sw..(y1 + 1) * sw], fy - y0 as f32, &cols, row);
                });
            } else {
                dst.par_chunks_mut(dw).enumerate().for_each(|(y, row)| {
                    for (x, d) in row.iter_mut().enumerate() {
                        let u = up(small, sw, sh, x, y);
                        for k in 0..3 {
                            d[k] += u[k];
                        }
                    }
                });
            }
            if i == hal_level {
                halo = Some((dw, lo[i].1, lo[i].2.clone()));
            }
        }
        let norm = 1.0 / nl as f32;
        let (bw, bh) = (self.pyr[0].0, self.pyr[0].1);
        let hal_tint = [1.0f32, 0.32, 0.12];
        // Combine bloom and halation at the base (half) resolution; the final
        // pass adds it with one bilinear lookup per pixel.
        let mut glow = std::mem::take(&mut self.glow);
        glow.clear();
        glow.resize(bw * bh, [0.0; 3]);
        let base = &self.pyr[0].2;
        if !SCALAR_FINAL.with(|c| c.get()) {
            let hal = halo.as_ref().filter(|_| p.halation > 0.0);
            let cols = hal.map(|(hw2, _, _)| taps(bw, |x| ((x as f32 + 0.5) * *hw2 as f32 / bw as f32 - 0.5).clamp(0.0, (*hw2 - 1) as f32), *hw2)).unwrap_or_default();
            let bscale: [f32; 3] = std::array::from_fn(|k| p.bloom_tint[k]);
            glow.par_chunks_mut(bw).enumerate().for_each(|(y, row)| {
                let b = &base[y * bw..(y + 1) * bw];
                for (g, b) in row.iter_mut().zip(b) {
                    for k in 0..3 {
                        g[k] = b[k] * norm * p.bloom * bscale[k] * 2.0;
                    }
                }
                if let Some((hw2, hh2, hv)) = hal {
                    let (hw2, hh2) = (*hw2, *hh2);
                    let sy = ((y as f32 + 0.5) * hh2 as f32 / bh as f32 - 0.5).clamp(0.0, (hh2 - 1) as f32);
                    let y0 = sy as usize;
                    let y1 = (y0 + 1).min(hh2 - 1);
                    let (ra, rb, ty) = (&hv[y0 * hw2..(y0 + 1) * hw2], &hv[y1 * hw2..(y1 + 1) * hw2], sy - y0 as f32);
                    for (g, &(x0, x1, tx)) in row.iter_mut().zip(&cols) {
                        // SAFETY: `taps` clamps x0, x1 to hw2 − 1.
                        let (a, b, c, d) = unsafe { (*ra.get_unchecked(x0 as usize), *ra.get_unchecked(x1 as usize), *rb.get_unchecked(x0 as usize), *rb.get_unchecked(x1 as usize)) };
                        let hb: [f32; 3] = std::array::from_fn(|k| {
                            let top = a[k] + (b[k] - a[k]) * tx;
                            let bot = c[k] + (d[k] - c[k]) * tx;
                            top + (bot - top) * ty
                        });
                        let l = 0.2126 * hb[0] + 0.7152 * hb[1] + 0.0722 * hb[2];
                        for k in 0..3 {
                            g[k] += l * hal_tint[k] * p.halation * 3.0 * norm;
                        }
                    }
                }
            });
            self.glow = glow;
            self.glow_dims = (bw, bh);
            return;
        }
        glow.par_chunks_mut(bw).enumerate().for_each(|(y, row)| {
            for (x, g) in row.iter_mut().enumerate() {
                let b = base[y * bw + x];
                for k in 0..3 {
                    g[k] = b[k] * norm * p.bloom * p.bloom_tint[k] * 2.0;
                }
                if p.halation > 0.0 {
                    if let Some((hw2, hh2, hv)) = &halo {
                        let hb = sample_grid(hv, *hw2, *hh2, x, y, bw, bh);
                        let l = 0.2126 * hb[0] + 0.7152 * hb[1] + 0.0722 * hb[2];
                        for k in 0..3 {
                            g[k] += l * hal_tint[k] * p.halation * 3.0 * norm;
                        }
                    }
                }
            }
        });
        self.glow = glow;
        self.glow_dims = (bw, bh);
    }
}

/// Bilinear lookup of grid `src` (`sw × sh`) at the centre of pixel (`x`, `y`) of a `dw × dh` grid.
#[inline(always)]
fn sample_grid(src: &[[f32; 3]], sw: usize, sh: usize, x: usize, y: usize, dw: usize, dh: usize) -> [f32; 3] {
    let sx = ((x as f32 + 0.5) * sw as f32 / dw as f32 - 0.5).clamp(0.0, (sw - 1) as f32);
    let sy = ((y as f32 + 0.5) * sh as f32 / dh as f32 - 0.5).clamp(0.0, (sh - 1) as f32);
    let (x0, y0) = (sx as usize, sy as usize);
    let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
    let (tx, ty) = (sx - x0 as f32, sy - y0 as f32);
    let (a, b, c, d) = (src[y0 * sw + x0], src[y0 * sw + x1], src[y1 * sw + x0], src[y1 * sw + x1]);
    std::array::from_fn(|k| {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bot = c[k] + (d[k] - c[k]) * tx;
        top + (bot - top) * ty
    })
}

thread_local! {
    /// Tests: force the per-pixel final pass, to check the fast one against it.
    static SCALAR_FINAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Bilinear taps along one axis: for each of `n` output positions, the two
/// source indices (clamped to `len − 1`) and the fraction, from the source
/// coordinate `f(i)` (already clamped to `0..=len − 1`).
fn taps(n: usize, f: impl Fn(usize) -> f32, len: usize) -> Vec<(u32, u32, f32)> {
    (0..n)
        .map(|i| {
            let s = f(i);
            let i0 = s as usize;
            (i0 as u32, (i0 + 1).min(len - 1) as u32, s - i0 as f32)
        })
        .collect()
}

/// `row[x] += bilinear(ra, rb)` with column taps `cols` and row fraction `ty`
/// (the same arithmetic as `up`).
fn lerp_add_row(ra: &[[f32; 3]], rb: &[[f32; 3]], ty: f32, cols: &[(u32, u32, f32)], row: &mut [[f32; 3]]) {
    for (d, &(x0, x1, tx)) in row.iter_mut().zip(cols) {
        // SAFETY: `taps` clamps x0, x1 below the source width, the length of ra and rb.
        let (p00, p01, p10, p11) = unsafe { (*ra.get_unchecked(x0 as usize), *ra.get_unchecked(x1 as usize), *rb.get_unchecked(x0 as usize), *rb.get_unchecked(x1 as usize)) };
        for k in 0..3 {
            let a = p00[k] + (p01[k] - p00[k]) * tx;
            let b = p10[k] + (p11[k] - p10[k]) * tx;
            d[k] += a + (b - a) * ty;
        }
    }
}

/// One row of the bloom's bright pass at half resolution: a 2×2 box average
/// and the soft-knee threshold (the same arithmetic as the per-pixel loop).
fn bright_row(img: &[[f32; 3]], w: usize, h: usize, y: usize, t: f32, knee: f32, row: &mut [[f32; 3]]) {
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 is available; no multiply-adds are fused.
            unsafe { bright_row_avx2(img, w, h, y, t, knee, row) };
            return;
        }
    }
    bright_row_impl(img, w, h, y, t, knee, row)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn bright_row_avx2(img: &[[f32; 3]], w: usize, h: usize, y: usize, t: f32, knee: f32, row: &mut [[f32; 3]]) {
    bright_row_impl(img, w, h, y, t, knee, row)
}

#[inline(always)]
// Index loops over planar chunk arrays are what vectorises here.
#[allow(clippy::needless_range_loop)]
fn bright_row_impl(img: &[[f32; 3]], w: usize, h: usize, y: usize, t: f32, knee: f32, row: &mut [[f32; 3]]) {
    let r0 = &img[(2 * y).min(h - 1) * w..][..w];
    let r1 = &img[(2 * y + 1).min(h - 1) * w..][..w];
    let mut acc = [[0f32; FCH]; 3];
    let mut cm = [0f32; FCH];
    let hw = row.len();
    let mut x0 = 0;
    while x0 < hw {
        let n = FCH.min(hw - x0);
        for i in 0..n {
            let x = x0 + i;
            let (sa, sb) = ((2 * x).min(w - 1), (2 * x + 1).min(w - 1));
            // Tap order (0,0), (1,0), (0,1), (1,1), as in the per-pixel loop.
            let (c00, c10, c01, c11) = (r0[sa], r0[sb], r1[sa], r1[sb]);
            for k in 0..3 {
                acc[k][i] = 0.0 + c00[k] * 0.25 + c10[k] * 0.25 + c01[k] * 0.25 + c11[k] * 0.25;
            }
        }
        for i in 0..n {
            let l = 0.2126 * acc[0][i] + 0.7152 * acc[1][i] + 0.0722 * acc[2][i];
            let s = (l - t + knee).clamp(0.0, 2.0 * knee);
            let s = s * s / (4.0 * knee);
            let c = s.max(l - t) / l.max(1e-5);
            cm[i] = c.max(0.0);
        }
        for i in 0..n {
            row[x0 + i] = [acc[0][i] * cm[i], acc[1][i] * cm[i], acc[2][i] * cm[i]];
        }
        x0 += n;
    }
}

/// Inputs of the lens resample.
struct LensCtx<'a> {
    img: &'a [[f32; 3]],
    w: usize,
    wm: f32,
    hm: f32,
    cx: f32,
    cy: f32,
    inv: f32,
    k: f32,
    zoom: f32,
    ca: f32,
}

/// One row of the lens resample: coordinates and weights in vector stages,
/// the four taps per channel gathered in a scalar loop, the blend vectorised.
/// Bit-exact with the per-pixel loop in `lens`.
fn lens_row(c: &LensCtx, y: usize, row: &mut [[f32; 3]]) {
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 is available; no multiply-adds are fused.
            unsafe { lens_row_avx2(c, y, row) };
            return;
        }
    }
    lens_row_impl(c, y, row)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn lens_row_avx2(c: &LensCtx, y: usize, row: &mut [[f32; 3]]) {
    lens_row_impl(c, y, row)
}

#[inline(always)]
// Index loops over planar chunk arrays are what vectorises here.
#[allow(clippy::needless_range_loop)]
fn lens_row_impl(c: &LensCtx, y: usize, row: &mut [[f32; 3]]) {
    let (w, wm, hm) = (c.w, c.wm, c.hm);
    let dy = y as f32 + 0.5 - c.cy;
    let mut m = [[0f32; FCH]; 3];
    let mut i00 = [[0u32; FCH]; 3];
    let mut dxo = [[0u32; FCH]; 3];
    let mut dyo = [[0u32; FCH]; 3];
    let mut fx = [[0f32; FCH]; 3];
    let mut fy = [[0f32; FCH]; 3];
    let mut p = [[[0f32; FCH]; 4]; 3];
    let mut x0 = 0;
    while x0 < w {
        let n = FCH.min(w - x0);
        for i in 0..n {
            let dx = (x0 + i) as f32 + 0.5 - c.cx;
            let r2 = (dx * dx + dy * dy) * c.inv * c.inv;
            let f = (1.0 + c.k * r2) * c.zoom;
            let cr = c.ca * r2.sqrt();
            m[0][i] = f * (1.0 + cr);
            m[1][i] = f * 1.0;
            m[2][i] = f * (1.0 - cr);
        }
        for ch in 0..3 {
            for i in 0..n {
                let dx = (x0 + i) as f32 + 0.5 - c.cx;
                let sx = (c.cx + dx * m[ch][i] - 0.5).max(0.0).min(wm);
                let sy = (c.cy + dy * m[ch][i] - 0.5).max(0.0).min(hm);
                // SAFETY: 0 ≤ sx ≤ w − 1 and 0 ≤ sy ≤ h − 1.
                let (xi, yi) = unsafe { (sx.to_int_unchecked::<i32>(), sy.to_int_unchecked::<i32>()) };
                let (xf, yf) = (xi as f32, yi as f32);
                dxo[ch][i] = (xf < wm) as u32;
                dyo[ch][i] = if yf < hm { w as u32 } else { 0 };
                i00[ch][i] = yi as u32 * w as u32 + xi as u32;
                fx[ch][i] = sx - xf;
                fy[ch][i] = sy - yf;
            }
            for i in 0..n {
                let (a, ox, oy) = (i00[ch][i] as usize, dxo[ch][i] as usize, dyo[ch][i] as usize);
                // SAFETY: a, a + ox, a + oy and a + ox + oy are pixels of the frame
                // (ox and oy are 0 on the last column and row).
                unsafe {
                    p[ch][0][i] = c.img.get_unchecked(a)[ch];
                    p[ch][1][i] = c.img.get_unchecked(a + ox)[ch];
                    p[ch][2][i] = c.img.get_unchecked(a + oy)[ch];
                    p[ch][3][i] = c.img.get_unchecked(a + oy + ox)[ch];
                }
            }
            for i in 0..n {
                let [p00, p01, p10, p11] = [p[ch][0][i], p[ch][1][i], p[ch][2][i], p[ch][3][i]];
                let a = p00 + (p01 - p00) * fx[ch][i];
                let b = p10 + (p11 - p10) * fx[ch][i];
                row[x0 + i][ch] = a + (b - a) * fy[ch][i];
            }
        }
        x0 += n;
    }
}

/// Inputs of the final pass (grade, tone map, vignette, grain, dither, encode).
struct FinalCtx<'a> {
    w: usize,
    h: usize,
    gw: usize,
    gh: usize,
    glow: &'a [[f32; 3]],
    gcols: &'a [(usize, usize, f32)],
    img: &'a [[f32; 3]],
    dith: &'a [f32],
    lut: &'a [f32],
    grade: [&'a [f32]; 3],
    sat: f32,
    tonemap: Tonemap,
    vig: f32,
    soft: f32,
    cx: f32,
    cy: f32,
    inv_diag: f32,
    grain_amp: f32,
    gsize: f32,
    seed: u32,
    dither: bool,
}

/// One output row of the final pass, in chunks of [`FCH`] pixels and one stage
/// at a time over planar arrays, so the arithmetic vectorises (the table
/// lookups stay scalar). Bit-exact with the per-pixel loop in `apply`.
fn final_row(c: &FinalCtx, y: usize, row: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 is available. Rust never fuses multiply-adds, so the
            // results are the same as the baseline build's.
            unsafe { final_row_avx2(c, y, row) };
            return;
        }
    }
    final_row_impl(c, y, row)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn final_row_avx2(c: &FinalCtx, y: usize, row: &mut [u8]) {
    final_row_impl(c, y, row)
}

const FCH: usize = 64;

#[inline(always)]
// Index loops over planar chunk arrays are what vectorises here.
// max/min rather than clamp: NaN must become a number before the unchecked casts.
#[allow(clippy::needless_range_loop, clippy::manual_clamp)]
fn final_row_impl(c: &FinalCtx, y: usize, row: &mut [u8]) {
    let w = c.w;
    let src = &c.img[y * w..(y + 1) * w];
    let drow = &c.dith[y * w..(y + 1) * w];
    let dy = (y as f32 + 0.5 - c.cy) * c.inv_diag;
    let grow = (!c.glow.is_empty()).then(|| {
        let sy = ((y as f32 + 0.5) * c.gh as f32 / c.h as f32 - 0.5).clamp(0.0, (c.gh - 1) as f32);
        let y0 = sy as usize;
        let y1 = (y0 + 1).min(c.gh - 1);
        (&c.glow[y0 * c.gw..(y0 + 1) * c.gw], &c.glow[y1 * c.gw..(y1 + 1) * c.gw], sy - y0 as f32)
    });
    let mut v = [[0f32; FCH]; 3];
    let mut si = [[0u32; FCH]; 3];
    let mut sf = [[0f32; FCH]; 3];
    let gy = (y as f32 / c.gsize.max(1.0)) as u32;
    let mut x0 = 0;
    while x0 < w {
        let n = FCH.min(w - x0);
        // Linear colour, plus the glow (bilinear from the half-res grid).
        for i in 0..n {
            let s = src[x0 + i];
            v[0][i] = s[0];
            v[1][i] = s[1];
            v[2][i] = s[2];
        }
        if let Some((ra, rb, ty)) = grow {
            for i in 0..n {
                let (gx0, gx1, tx) = c.gcols[x0 + i];
                let (a, b, cc, d) = (ra[gx0], ra[gx1], rb[gx0], rb[gx1]);
                for k in 0..3 {
                    let top = a[k] + (b[k] - a[k]) * tx;
                    let bot = cc[k] + (d[k] - cc[k]) * tx;
                    v[k][i] += top + (bot - top) * ty;
                }
            }
        }
        // Grade curves: table index and fraction (vector), lookup (scalar).
        for k in 0..3 {
            for i in 0..n {
                let s = (v[k][i].max(0.0) / GRADE_MAX).sqrt().min(1.0) * GRADE_LUT as f32;
                // SAFETY: 0 ≤ s ≤ GRADE_LUT, so the conversion is in range.
                let ii = unsafe { s.to_int_unchecked::<i32>() }.min(GRADE_LUT as i32 - 1);
                si[k][i] = ii as u32;
                sf[k][i] = s - ii as f32;
            }
            let t = c.grade[k];
            for i in 0..n {
                let ii = si[k][i] as usize;
                // SAFETY: ii + 1 ≤ GRADE_LUT and the table has GRADE_LUT + 1 entries.
                let (a, b) = unsafe { (*t.get_unchecked(ii), *t.get_unchecked(ii + 1)) };
                v[k][i] = a + (b - a) * sf[k][i];
            }
        }
        if c.sat != 1.0 {
            let sat = c.sat;
            for i in 0..n {
                let l = 0.2126 * v[0][i] + 0.7152 * v[1][i] + 0.0722 * v[2][i];
                for k in 0..3 {
                    v[k][i] = (l + (v[k][i] - l) * sat).max(0.0);
                }
            }
        }
        match c.tonemap {
            Tonemap::None => {}
            Tonemap::Soft => {
                for i in 0..n {
                    let m = v[0][i].max(v[1][i]).max(v[2][i]);
                    let excess = (1.0 - 1.0 / m) * 0.6;
                    let over = m > 1.0;
                    for k in 0..3 {
                        let nn = v[k][i] / m;
                        let r = nn + (1.0 - nn) * excess;
                        v[k][i] = if over { r } else { v[k][i] };
                    }
                }
            }
            Tonemap::Aces => {
                for k in 0..3 {
                    for i in 0..n {
                        let x = v[k][i] * 0.8;
                        v[k][i] = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
                    }
                }
            }
        }
        if c.vig > 0.0 {
            let (vig, soft) = (c.vig, c.soft);
            for i in 0..n {
                let dx = ((x0 + i) as f32 + 0.5 - c.cx) * c.inv_diag;
                let r = (dx * dx + dy * dy).sqrt() * std::f32::consts::SQRT_2;
                let t = ((r - (1.0 - soft)) / soft).clamp(0.0, 1.0);
                let m = 1.0 - vig * t * t * (3.0 - 2.0 * t);
                for k in 0..3 {
                    v[k][i] *= m;
                }
            }
        }
        // Encode to sRGB 0..255.
        for k in 0..3 {
            for i in 0..n {
                let s = v[k][i].max(0.0).min(1.0).sqrt() * SRGB_LUT as f32;
                // SAFETY: 0 ≤ s ≤ SRGB_LUT.
                let ii = unsafe { s.to_int_unchecked::<i32>() }.min(SRGB_LUT as i32 - 1);
                si[k][i] = ii as u32;
                sf[k][i] = s - ii as f32;
            }
            for i in 0..n {
                let ii = si[k][i] as usize;
                // SAFETY: ii + 1 ≤ SRGB_LUT and the table has SRGB_LUT + 1 entries.
                let (a, b) = unsafe { (*c.lut.get_unchecked(ii), *c.lut.get_unchecked(ii + 1)) };
                v[k][i] = a + (b - a) * sf[k][i];
            }
        }
        if c.grain_amp > 0.0 {
            let (amp, gsize, seed) = (c.grain_amp, c.gsize, c.seed);
            for i in 0..n {
                // SAFETY: x / gsize is finite, non-negative and below 2^31.
                let gx = unsafe { ((x0 + i) as f32 / gsize).to_int_unchecked::<i32>() } as u32;
                let g = (hash01(gx, gy, seed) + hash01(gx, gy, seed ^ 0x68e3_1da4) - 1.0) * amp;
                let l = (0.2126 * v[0][i] + 0.7152 * v[1][i] + 0.0722 * v[2][i]) / 255.0;
                let wgt = 0.35 + 2.6 * l * (1.0 - l);
                for k in 0..3 {
                    v[k][i] += g * wgt;
                }
            }
        }
        if c.dither {
            for k in 0..3 {
                for i in 0..n {
                    v[k][i] += drow[x0 + i];
                }
            }
        }
        let out = &mut row[x0 * 4..(x0 + n) * 4];
        for (i, px) in out.chunks_exact_mut(4).enumerate() {
            // max/min, not clamp: same values, and NaN becomes 0 (as `as u8` did).
            for k in 0..3 {
                let q = (v[k][i] + 0.5).max(0.0).min(255.0);
                // SAFETY: 0 ≤ q ≤ 255.
                px[k] = unsafe { q.to_int_unchecked::<i32>() } as u8;
            }
            px[3] = 255;
        }
        x0 += n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: usize, h: usize) -> Vec<u8> {
        let mut v = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let bright = (x as i32 - 32).abs() < 4 && (y as i32 - 24).abs() < 4;
                let g = if bright { 255 } else { (x * 40 / w) as u8 + 10 };
                v[i..i + 4].copy_from_slice(&[g, g, g, 255]);
            }
        }
        v
    }

    #[test]
    fn lut_grades_and_mix_zero_is_off() {
        let base = PostSettings { dither: false, ..Default::default() };
        let mono = PostSettings { lut: Some("anikit:lut/mono-silver".into()), ..base.clone() };
        // The film model's own parameters work too (a preset is only a start).
        let custom = PostSettings { lut: Some(serde_json::json!({ "base": "mono-silver", "contrast": 0.2, "mono": [1.1, 1.0, 0.85] })), ..base.clone() };
        let mut cp = [200u8, 60, 40, 255].repeat(16);
        PostProcessor::default().apply(&mut cp, 4, 4, &custom, 0);
        assert!(cp[0] > cp[2] + 10, "toned mono: {:?}", &cp[..4]);
        let bad = PostSettings { lut: Some(serde_json::json!({ "contrst": 1 })), ..base.clone() };
        assert!(crate::assetlib::lut::load_value(bad.lut.as_ref().unwrap()).unwrap_err().contains("contrast"));
        assert!(!mono.is_identity());
        let mut px = [200u8, 60, 40, 255].repeat(16);
        PostProcessor::default().apply(&mut px, 4, 4, &mono, 0);
        assert!((px[0] as i32 - px[1] as i32).abs() <= 2 && (px[1] as i32 - px[2] as i32).abs() <= 2, "{:?}", &px[..4]);
        let off = PostSettings { lut_mix: 0.0, ..mono };
        assert!(off.is_identity());
        // Unknown LUTs fail when the spec resolves, naming the built-ins.
        let bad = PostSpec::Custom(Box::new(PostCustom { preset: None, fields: serde_json::from_str(r#"{"lut": "anikit:lut/teal-ornge"}"#).unwrap() }));
        assert!(bad.resolve().unwrap_err().contains("teal-orange"));
    }

    #[test]
    fn presets_resolve_and_override() {
        for p in PRESETS {
            assert!(PostSettings::preset(p).is_some(), "{p}");
        }
        let s: PostSpec = serde_json::from_str(r#""cinematic""#).unwrap();
        assert_eq!(s.resolve().unwrap().bloom, 0.35);
        let s: PostSpec = serde_json::from_str(r#"{"preset":"cinematic","bloom":0.9,"grain":0}"#).unwrap();
        let r = s.resolve().unwrap();
        assert_eq!((r.bloom, r.grain, r.vignette), (0.9, 0.0, 0.32));
        let bad: PostSpec = serde_json::from_str(r#"{"preset":"cinematic","blom":1}"#).unwrap();
        assert!(bad.resolve().unwrap_err().contains("blom"));
        assert!(PostSpec::Preset("nope".into()).resolve().unwrap_err().contains("cinematic"));
    }

    #[test]
    fn clean_preset_is_near_identity_and_deterministic() {
        let (w, h) = (64, 48);
        let src = frame(w, h);
        let mut a = src.clone();
        let mut pp = PostProcessor::default();
        pp.apply(&mut a, w, h, &PostSettings::preset("clean").unwrap(), 3);
        assert_eq!(a, src, "clean is the identity on 8-bit frames");
        let mut b = src.clone();
        PostProcessor::default().apply(&mut b, w, h, &PostSettings::preset("clean").unwrap(), 3);
        assert_eq!(a, b);
    }

    #[test]
    fn bloom_glows_around_highlights() {
        let (w, h) = (64, 48);
        let src = frame(w, h);
        let mut a = src.clone();
        let p = PostSettings { bloom: 1.0, bloom_threshold: 0.5, dither: false, tonemap: Tonemap::None, ..Default::default() };
        PostProcessor::default().apply(&mut a, w, h, &p, 0);
        // Near the bright square (outside it), brighter than before.
        let i = (24 * w + 40) * 4;
        assert!(a[i] > src[i] + 5, "{} vs {}", a[i], src[i]);
        // Far away, about the same.
        let j = (5 * w + 5) * 4;
        assert!(a[j].abs_diff(src[j]) <= 3);
    }

    /// The chunked final pass must match the per-pixel one bit for bit, for
    /// every preset and tone map, with and without glow, at odd sizes.
    #[test]
    fn final_pass_matches_scalar() {
        let mut looks: Vec<PostSettings> = PRESETS.iter().map(|n| PostSettings::preset(n).unwrap()).collect();
        let c = PostSettings::preset("cinematic").unwrap();
        looks.push(PostSettings { tonemap: Tonemap::Aces, grain_size: 2.5, ..c.clone() });
        looks.push(PostSettings { tonemap: Tonemap::None, bloom: 0.0, halation: 0.0, saturation: 1.0, vignette: 0.0, ..c });
        for (w, h) in [(203usize, 117usize), (64, 9), (130, 66)] {
            let mut src = vec![0u8; w * h * 4];
            let mut r = 0x1234_5678u32;
            for (i, b) in src.iter_mut().enumerate() {
                r ^= r << 13;
                r ^= r >> 17;
                r ^= r << 5;
                // Smooth ramps, noise and some saturated highlights for the bloom.
                *b = if i % 4 == 3 || (i / 4) % 37 < 4 { 255 } else { ((i / 4 % w) * 255 / w) as u8 ^ (r as u8 & 15) };
            }
            for (k, p) in looks.iter().enumerate() {
                let mut fast = src.clone();
                PostProcessor::default().apply(&mut fast, w, h, p, 7 + k as u64);
                let mut slow = src.clone();
                SCALAR_FINAL.with(|c| c.set(true));
                PostProcessor::default().apply(&mut slow, w, h, p, 7 + k as u64);
                SCALAR_FINAL.with(|c| c.set(false));
                assert!(fast == slow, "look {k} at {w}×{h}: fast final pass differs");
            }
        }
    }

    #[test]
    fn grain_animates_but_dither_is_static() {
        let (w, h) = (32, 32);
        let src = vec![128u8; w * h * 4];
        let p = PostSettings { grain: 0.05, ..Default::default() };
        let (mut a, mut b) = (src.clone(), src.clone());
        let mut pp = PostProcessor::default();
        pp.apply(&mut a, w, h, &p, 1);
        pp.apply(&mut b, w, h, &p, 2);
        assert_ne!(a, b);
        let q = PostSettings::default();
        let (mut c, mut d) = (src.clone(), src);
        pp.apply(&mut c, w, h, &q, 1);
        pp.apply(&mut d, w, h, &q, 2);
        assert_eq!(c, d);
    }
}
