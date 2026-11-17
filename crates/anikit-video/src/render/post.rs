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
                serde_json::from_value(v).map_err(|e| format!("post: {e}"))
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

/// sRGB8 → linear, as a table.
pub(crate) fn to_linear_lut() -> &'static [f32; 256] {
    static T: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|i| srgb_to_linear(i as f32 / 255.0)))
}

/// Linear 0..1 → sRGB in 8-bit units (unrounded), by a sqrt-indexed table:
/// within 0.002 of [`linear_to_srgb`] · 255, without a `powf` per channel.
#[inline]
pub(crate) fn encode_srgb(v: f32) -> f32 {
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
        let (lut, grade, img) = (&self.to_srgb, &self.grade, &self.img);
        let (glow, (gw, gh)) = (&self.glow, self.glow_dims);
        use rayon::prelude::*;
        rgba.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            let src = &img[y * w..(y + 1) * w];
            let dy = (y as f32 + 0.5 - cy) * inv_diag;
            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                let mut v = src[x];
                if !glow.is_empty() {
                    let g = sample_grid(glow, gw, gh, x, y, w, h);
                    for k in 0..3 {
                        v[k] += g[k];
                    }
                }
                let mut c = [0f32; 3];
                for k in 0..3 {
                    c[k] = Self::lookup(&grade[k], v[k]);
                }
                if p.saturation != 1.0 {
                    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                    for v in &mut c {
                        *v = (l + (*v - l) * p.saturation).max(0.0);
                    }
                }
                match p.tonemap {
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
                if p.vignette > 0.0 {
                    let dx = (x as f32 + 0.5 - cx) * inv_diag;
                    let r = (dx * dx + dy * dy).sqrt() * std::f32::consts::SQRT_2;
                    let t = ((r - (1.0 - soft)) / soft).clamp(0.0, 1.0);
                    let m = 1.0 - p.vignette * t * t * (3.0 - 2.0 * t);
                    for v in &mut c {
                        *v *= m;
                    }
                }
                // Encode, then grain (strongest in the midtones) and dither in 8-bit units.
                let mut o = [Self::encode(lut, c[0]), Self::encode(lut, c[1]), Self::encode(lut, c[2])];
                if grain_amp > 0.0 {
                    let (gx, gy) = ((x as f32 / gsize) as u32, (y as f32 / gsize) as u32);
                    let g = (hash01(gx, gy, seed) + hash01(gx, gy, seed ^ 0x68e3_1da4) - 1.0) * grain_amp;
                    let l = (0.2126 * o[0] + 0.7152 * o[1] + 0.0722 * o[2]) / 255.0;
                    let wgt = 0.35 + 2.6 * l * (1.0 - l);
                    for v in &mut o {
                        *v += g * wgt;
                    }
                }
                if p.dither {
                    // Triangular noise, ±1 LSB, fixed per pixel (static regions stay static).
                    let d = tri_dither(x as u32, y as u32);
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

    #[inline(always)]
    fn lookup(lut: &[f32], v: f32) -> f32 {
        let s = (v.max(0.0) / GRADE_MAX).sqrt().min(1.0) * GRADE_LUT as f32;
        let i = (s as usize).min(GRADE_LUT - 1);
        let f = s - i as f32;
        lut[i] + (lut[i + 1] - lut[i]) * f
    }

    /// Barrel distortion and radial chromatic aberration in one bilinear resample.
    fn lens(&mut self, w: usize, h: usize, k: f32, ca_px: f32) {
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let inv = 1.0 / (cx * cx + cy * cy).sqrt();
        // Zoom so the corners still sample inside the frame.
        let zoom = 1.0 / (1.0 + k.max(0.0));
        let ca = ca_px * inv; // fraction of the half-diagonal at the corner
        self.tmp.clear();
        self.tmp.resize(w * h, [0.0; 3]);
        let img = &self.img;
        let sample = |sx: f32, sy: f32, ch: usize| -> f32 {
            let sx = sx.clamp(0.0, (w - 1) as f32);
            let sy = sy.clamp(0.0, (h - 1) as f32);
            let (x0, y0) = (sx as usize, sy as usize);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let a = img[y0 * w + x0][ch] + (img[y0 * w + x1][ch] - img[y0 * w + x0][ch]) * fx;
            let b = img[y1 * w + x0][ch] + (img[y1 * w + x1][ch] - img[y1 * w + x0][ch]) * fx;
            a + (b - a) * fy
        };
        use rayon::prelude::*;
        self.tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let r2 = (dx * dx + dy * dy) * inv * inv;
                let f = (1.0 + k * r2) * zoom;
                for (ch, s) in [(0usize, 1.0 + ca * r2.sqrt()), (1, 1.0), (2, 1.0 - ca * r2.sqrt())] {
                    let m = f * s;
                    out[ch] = sample(cx + dx * m - 0.5, cy + dy * m - 0.5, ch);
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
            dst.par_chunks_mut(dw).enumerate().for_each(|(y, row)| {
                for (x, d) in row.iter_mut().enumerate() {
                    let u = up(small, sw, sh, x, y);
                    for k in 0..3 {
                        d[k] += u[k];
                    }
                }
            });
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
