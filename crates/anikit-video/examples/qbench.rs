//! Quality-locked performance harness for Anikit Video.
//!
//! `cargo run --release -p anikit-video --example qbench [-- options]`
//!
//! For every spec in `crates/anikit-video/qbench/corpus.json`:
//! 1. **Default render**, timed: exactly what `anikit-video render` does
//!    (ak264, `--quality standard`, full resolution), `--runs` times; median wall,
//!    fps and the per-stage profile.
//! 2. **Reference render**: every speed-only approximation off
//!    ([`Quality::Reference`]: exact glyph/image phases, no sprites, caches,
//!    decimated blurs, fused YUV, incremental regions or held frames; 96-sample
//!    motion blur; components in reference mode), every frame from scratch,
//!    converted to YUV in floating point.
//! 3. **Render fidelity**: the default pipeline's frames, tapped exactly as they
//!    go into the encoder, vs the reference: PSNR, SSIM, CIE ΔE (p99.9 and
//!    max), a max-ΔE heatmap, and temporal stability (frame-to-frame change of
//!    the error, which catches flicker, stepping and popping).
//! 4. **Delivered fidelity**: the decoded MP4 vs the reference (encoder loss
//!    included), checked against the committed baseline.
//!
//! Options: `--only a,b` · `--runs N` (default 3) · `--gate` (exit 1 on any
//! failure) · `--update-baseline` · `--out DIR` (default `target/qbench`) ·
//! `--json`.
//!
//! See `docs/video/perf.md` for thresholds and how to read the table.

use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::{self, Codec, Quality, RenderOptions};
use anikit_video::render::{Quality as RenderQuality, Renderer};
use anikit_video::spec::{self, Severity};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ---------------------------------------------------------------- thresholds

/// Render fidelity (default pipeline before encoding vs reference), per frame.
/// A frame below any of these fails the harness.
const MIN_SSIM: f64 = 0.995;
const MIN_PSNR_Y: f64 = 42.0;
/// CIE76 ΔE: 2.3 is one just-noticeable difference.
const MAX_DE_P999: f64 = 2.3;
/// Frame-to-frame change of the luma error (8-bit levels), 99.9th percentile:
/// a pattern that changes between frames (flicker, stepping, popping).
const MAX_FLICKER_P999: f64 = 4.0;
/// Reported (⚑), not failed: single pixels above this ΔE.
const FLAG_DE_MAX: f64 = 5.0;
/// Delivered fidelity (decoded MP4 vs reference) may not fall below the
/// baseline by more than this.
const DELIVERED_SSIM_SLACK: f64 = 0.002;
const DELIVERED_PSNR_SLACK: f64 = 0.3;
/// Reported (⚑), not failed: grain and noise are expensive for any lossy encoder.
const DELIVERED_FLAG_SSIM: f64 = 0.97;

// ---------------------------------------------------------------- corpus

#[derive(Deserialize)]
struct Corpus {
    entries: Vec<Entry>,
}

#[derive(Deserialize, Clone)]
struct Entry {
    name: String,
    kind: String,
    spec: String,
    from: Option<f32>,
    to: Option<f32>,
    /// Timed runs for this entry (overrides `--runs`; for very slow content).
    runs: Option<usize>,
    /// Time the whole spec rather than `from..to` (short clips time too noisily).
    #[serde(default)]
    time_full: bool,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Baseline {
    /// name → (delivered SSIM min, delivered PSNR-Y mean)
    delivered: std::collections::BTreeMap<String, (f64, f64)>,
}

// ---------------------------------------------------------------- frames

/// A tight 4:2:0 frame.
#[derive(Clone, Default)]
struct Frame {
    w: usize,
    h: usize,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl Frame {
    fn from_buffer(b: &ak264::YuvBuffer) -> Frame {
        Frame { w: b.width as usize, h: b.height as usize, y: b.y.clone(), u: b.u.clone(), v: b.v.clone() }
    }
}

/// Premultiplied RGBA (over black) → BT.709 limited-range 4:2:0, in double
/// precision, cropped to `w × h`. Chroma is the 2×2 average, as the renderer's.
fn rgba_to_frame(px: &[u8], stride_w: usize, w: usize, h: usize) -> Frame {
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut f = Frame { w, h, y: vec![0; w * h], u: vec![0; cw * ch], v: vec![0; cw * ch] };
    let q = |v: f64| (v + 0.5).floor().clamp(0.0, 255.0) as u8;
    for y in 0..h {
        for x in 0..w {
            let p = &px[(y * stride_w + x) * 4..];
            let (r, g, b) = (p[0] as f64, p[1] as f64, p[2] as f64);
            f.y[y * w + x] = q(16.0 + 219.0 / 255.0 * (0.2126 * r + 0.7152 * g + 0.0722 * b));
        }
    }
    for cy in 0..ch {
        for cx in 0..cw {
            let (mut r, mut g, mut b, mut n) = (0.0, 0.0, 0.0, 0.0);
            for y in cy * 2..(cy * 2 + 2).min(h) {
                for x in cx * 2..(cx * 2 + 2).min(w) {
                    let p = &px[(y * stride_w + x) * 4..];
                    r += p[0] as f64;
                    g += p[1] as f64;
                    b += p[2] as f64;
                    n += 1.0;
                }
            }
            let (r, g, b) = (r / n, g / n, b / n);
            f.u[cy * cw + cx] = q(128.0 + 224.0 / 255.0 * (-0.114572 * r - 0.385428 * g + 0.5 * b));
            f.v[cy * cw + cx] = q(128.0 + 224.0 / 255.0 * (0.5 * r - 0.454153 * g - 0.045847 * b));
        }
    }
    f
}

// ---------------------------------------------------------------- spec loading

fn build(path: &Path) -> Result<Evaluator, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let low = path.to_string_lossy().to_ascii_lowercase();
    let parsed = if low.ends_with(".yaml") || low.ends_with(".yml") { spec::parse_yaml(&src) } else { spec::parse_json(&src) };
    let parsed = parsed.map_err(|d| d.iter().map(|x| format!("{}: {}", x.path, x.message)).collect::<Vec<_>>().join("; "))?;
    let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let fonts = anikit_video::text::FontStore::new();
    if let Some(f) = &parsed.spec.fonts {
        for file in f.values() {
            fonts.add_font_file(base.join(file)).map_err(|e| format!("font {file}: {e}"))?;
        }
    }
    let opts = Options { base_dir: base, fonts: Some(Arc::new(fonts)), load_assets: true, assets: None };
    let (ev, d) = Evaluator::build(&parsed.spec, &opts);
    let errs: Vec<String> = d.iter().filter(|x| x.severity == Severity::Error).map(|x| format!("{}: {}", x.path, x.message)).collect();
    match ev {
        Some(ev) if errs.is_empty() => Ok(ev),
        _ => Err(errs.join("; ")),
    }
}

fn range(ev: &Evaluator, e: &Entry) -> (u32, u32) {
    let total = ev.frames();
    let f0 = e.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total);
    let f1 = e.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0, total);
    (f0, f1)
}

fn options(e: &Entry, profile: bool) -> RenderOptions {
    RenderOptions { codec: Codec::Ak264, quality: Quality::Good, from: e.from, to: e.to, scale: 1.0, progress: false, audio: true, profile }
}

// ---------------------------------------------------------------- reference

fn reference_frames(ev: &Evaluator, f0: u32, n: usize, w: usize, h: usize) -> Vec<Frame> {
    anikit_video::components::set_draft(false);
    anikit_video::components::set_reference(true);
    let make = |ev: &Evaluator| {
        let mut r = Renderer::new(ev.assets.snapshot_fonts());
        let a = ev.assets.clone();
        r.set_image_source(Arc::new(move |id| a.pixmap(id)));
        r.set_quality(RenderQuality::Reference);
        (ev.assets.font_count(), r)
    };
    let out: Vec<Frame> = (0..n)
        .into_par_iter()
        .map_init(
            || None::<(usize, Renderer)>,
            |slot, k| {
                let i = f0 + k as u32;
                let t = pipeline::frame_time(ev, i);
                let dl = ev.eval(t);
                // Evaluating can add font faces: (re)make the renderer after it.
                if slot.as_ref().is_none_or(|(c, _)| *c != ev.assets.font_count()) {
                    *slot = Some(make(ev));
                }
                let r = &mut slot.as_mut().unwrap().1;
                let (post, blur) = pipeline::frame_look(ev, Quality::Good, i);
                r.set_post(post, i as u64);
                let pm = if blur {
                    let mut eval = |t: f32| ev.eval(t);
                    r.render_motion_blur(&mut eval, t, ev.shutter)
                } else {
                    r.render(&dl)
                };
                rgba_to_frame(pm.data_as_u8_slice(), pm.width() as usize, w, h)
            },
        )
        .collect();
    anikit_video::components::set_reference(false);
    out
}

// ---------------------------------------------------------------- metrics

#[derive(Clone, Default, Serialize)]
struct FrameStats {
    psnr_y: f64,
    psnr: f64,
    ssim: f64,
    /// Spatial ΔE (S-CIELAB-style pooling, see [`spatial`]): 99.9th percentile and max.
    de_p999: f64,
    de_max: f64,
    /// Pixels with spatial ΔE above one JND.
    de_over: u64,
    /// Spatially pooled CIEDE2000: 99.9th percentile (1.0 ≈ one JND).
    de00_p999: f64,
    /// Plain per-pixel CIE76 ΔE (no pooling): counts ±1 LSB dither as "visible"; informational.
    raw_de_p999: f64,
    raw_de_max: f64,
    flicker_p999: f64,
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let se: u64 = a.iter().zip(b).map(|(x, y)| ((*x as i32 - *y as i32).pow(2)) as u64).sum();
    mse_to_psnr(se as f64 / a.len().max(1) as f64)
}

fn mse_to_psnr(mse: f64) -> f64 {
    if mse <= 0.0 {
        99.0
    } else {
        (10.0 * (255.0f64 * 255.0 / mse).log10()).min(99.0)
    }
}

/// Luma SSIM over 8×8 windows with stride 4 (as x264 and libvpx measure it).
fn ssim(a: &[u8], b: &[u8], w: usize, h: usize) -> f64 {
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let (mut sum, mut n) = (0.0, 0usize);
    let mut y = 0;
    while y + 8 <= h {
        let mut x = 0;
        while x + 8 <= w {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0u32, 0u32, 0u64, 0u64, 0u64);
            for yy in y..y + 8 {
                let (ra, rb) = (&a[yy * w + x..yy * w + x + 8], &b[yy * w + x..yy * w + x + 8]);
                for k in 0..8 {
                    let (p, q) = (ra[k] as u32, rb[k] as u32);
                    sa += p;
                    sb += q;
                    saa += (p * p) as u64;
                    sbb += (q * q) as u64;
                    sab += (p * q) as u64;
                }
            }
            let m = 64.0;
            let (ma, mb) = (sa as f64 / m, sb as f64 / m);
            let va = saa as f64 / m - ma * ma;
            let vb = sbb as f64 / m - mb * mb;
            let cov = sab as f64 / m - ma * mb;
            sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
            x += 4;
        }
        y += 4;
    }
    if n == 0 {
        1.0
    } else {
        sum / n as f64
    }
}

struct Lab {
    lin: [f32; 256],
}

impl Lab {
    fn new() -> Lab {
        let mut lin = [0f32; 256];
        for (i, l) in lin.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *l = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        }
        Lab { lin }
    }
    /// BT.709 limited Y'CbCr → sRGB8 → CIELAB (D65).
    fn lab(&self, y: u8, u: u8, v: u8) -> [f32; 3] {
        self.labf(y as f32, u as f32, v as f32)
    }
    fn labf(&self, y: f32, u: f32, v: f32) -> [f32; 3] {
        let (y, u, v) = ((y - 16.0) * (255.0 / 219.0), (u - 128.0) * (255.0 / 224.0), (v - 128.0) * (255.0 / 224.0));
        let r = y + 1.5748 * v;
        let g = y - 0.187324 * u - 0.468124 * v;
        let b = y + 1.8556 * u;
        // Interpolated sRGB → linear (pooled values are fractional).
        let c = |x: f32| {
            let x = x.clamp(0.0, 255.0);
            let i = (x as usize).min(254);
            let f = x - i as f32;
            self.lin[i] * (1.0 - f) + self.lin[i + 1] * f
        };
        let (r, g, b) = (c(r), c(g), c(b));
        let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
        let yy = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
        let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
        let (fx, fy, fz) = (f(x), f(yy), f(z));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }
}

/// CIEDE2000 colour difference (kL = kC = kH = 1).
fn de2000(a: [f32; 3], b: [f32; 3]) -> f64 {
    let (l1, a1, b1) = (a[0] as f64, a[1] as f64, a[2] as f64);
    let (l2, a2, b2) = (b[0] as f64, b[1] as f64, b[2] as f64);
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = (a1 * (1.0 + g), a2 * (1.0 + g));
    let (c1p, c2p) = ((a1p * a1p + b1 * b1).sqrt(), (a2p * a2p + b2 * b2).sqrt());
    let hp = |b: f64, a: f64| if a == 0.0 && b == 0.0 { 0.0 } else { b.atan2(a).to_degrees().rem_euclid(360.0) };
    let (h1p, h2p) = (hp(b1, a1p), hp(b2, a2p));
    let dlp = l2 - l1;
    let dcp = c2p - c1p;
    let dhp = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhp_big = 2.0 * (c1p * c2p).sqrt() * (dhp.to_radians() / 2.0).sin();
    let lm = (l1 + l2) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hm - 30.0).to_radians().cos() + 0.24 * (2.0 * hm).to_radians().cos() + 0.32 * (3.0 * hm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hm - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cmp.powi(7) / (cmp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let rt = -(2.0 * dtheta.to_radians()).sin() * rc;
    ((dlp / sl).powi(2) + (dcp / sc).powi(2) + (dhp_big / sh).powi(2) + rt * (dcp / sc) * (dhp_big / sh)).sqrt()
}

/// Percentile from a histogram of values quantised at `step`.
fn hist_pct(hist: &[u64], step: f64, p: f64) -> f64 {
    let total: u64 = hist.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let target = ((total as f64) * p).ceil() as u64;
    let mut acc = 0;
    for (i, c) in hist.iter().enumerate() {
        acc += c;
        if acc >= target {
            return i as f64 * step;
        }
    }
    (hist.len() - 1) as f64 * step
}

/// Separable Gaussian blur of a `w × h` plane (σ in pixels, clamped edges).
fn gauss(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil() as isize;
    let k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f32 = k.iter().sum();
    let k: Vec<f32> = k.iter().map(|v| v / sum).collect();
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        let row = &src[y * w..y * w + w];
        for x in 0..w {
            let mut a = 0.0;
            for (j, kv) in k.iter().enumerate() {
                let xx = (x as isize + j as isize - r).clamp(0, w as isize - 1) as usize;
                a += kv * row[xx];
            }
            tmp[y * w + x] = a;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        for (j, kv) in k.iter().enumerate() {
            let yy = (y as isize + j as isize - r).clamp(0, h as isize - 1) as usize;
            let (o, t) = (&mut out[y * w..y * w + w], &tmp[yy * w..yy * w + w]);
            for x in 0..w {
                o[x] += kv * t[x];
            }
        }
    }
    out
}

/// Spatially pooled planes (S-CIELAB-style): the eye integrates luma over about
/// a minute of arc and colour over several. At 1080p seen from 3 picture
/// heights (~60 px/°) we use σ = 1 px for luma and σ = 1 chroma px (2 luma px)
/// for colour — less pooling than S-CIELAB itself, so stricter. Per-pixel
/// dither and sub-LSB rounding pool away; anything with spatial extent stays.
fn spatial(f: &Frame) -> [Vec<f32>; 3] {
    let (cw, ch) = (f.w.div_ceil(2), f.h.div_ceil(2));
    let c = |p: &[u8]| p.iter().map(|v| *v as f32).collect::<Vec<f32>>();
    [gauss(&c(&f.y), f.w, f.h, 1.0), gauss(&c(&f.u), cw, ch, 1.0), gauss(&c(&f.v), cw, ch, 1.0)]
}

/// Compare `test` with `refr`: stats, the per-pixel spatial ΔE (for the heatmap) and the signed luma error.
fn compare(lab: &Lab, test: &Frame, refr: &Frame) -> (FrameStats, Vec<f32>, Vec<f32>) {
    let (w, h, cw) = (refr.w, refr.h, refr.w.div_ceil(2));
    let py = psnr(&test.y, &refr.y);
    let (pu, pv) = (psnr(&test.u, &refr.u), psnr(&test.v, &refr.v));
    let mse = |p: f64| 255.0f64 * 255.0 / 10f64.powf(p / 10.0);
    let all = mse_to_psnr((6.0 * mse(py) + mse(pu) + mse(pv)) / 8.0);
    let identical = test.y == refr.y && test.u == refr.u && test.v == refr.v;
    let mut de = vec![0f32; w * h];
    let mut hist = vec![0u64; 10001];
    let mut raw_hist = vec![0u64; 10001];
    let mut h00 = vec![0u64; 10001];
    let (mut max, mut raw_max, mut over) = (0f64, 0f64, 0u64);
    let err: Vec<f32> = test.y.iter().zip(&refr.y).map(|(a, b)| *a as f32 - *b as f32).collect();
    if identical {
        hist[0] = (w * h) as u64;
        h00[0] = (w * h) as u64;
        raw_hist[0] = (w * h) as u64;
    } else {
        let (st, sr) = (spatial(test), spatial(refr));
        let d76 = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() as f64;
        for y in 0..h {
            for x in 0..w {
                let (i, c) = (y * w + x, (y / 2) * cw + x / 2);
                // Raw per-pixel ΔE.
                let rd = if test.y[i] == refr.y[i] && test.u[c] == refr.u[c] && test.v[c] == refr.v[c] {
                    0.0
                } else {
                    d76(lab.lab(test.y[i], test.u[c], test.v[c]), lab.lab(refr.y[i], refr.u[c], refr.v[c]))
                };
                raw_hist[((rd * 100.0) as usize).min(10000)] += 1;
                raw_max = raw_max.max(rd);
                // Pooled.
                let q = |p: &[Vec<f32>; 3]| lab.labf(p[0][i], p[1][c], p[2][c]);
                let (la, lb) = (q(&st), q(&sr));
                let d = d76(la, lb);
                let d00 = if d < 0.05 { d } else { de2000(la, lb) };
                h00[((d00 * 100.0) as usize).min(10000)] += 1;
                de[i] = d as f32;
                hist[((d * 100.0) as usize).min(10000)] += 1;
                max = max.max(d);
                over += (d > 2.3) as u64;
            }
        }
    }
    let st = FrameStats {
        psnr_y: py,
        psnr: all,
        ssim: ssim(&test.y, &refr.y, w, h),
        de_p999: hist_pct(&hist, 0.01, 0.999),
        de_max: max,
        de_over: over,
        raw_de_p999: hist_pct(&raw_hist, 0.01, 0.999),
        de00_p999: hist_pct(&h00, 0.01, 0.999),
        raw_de_max: raw_max,
        flicker_p999: 0.0,
    };
    (st, de, err)
}

/// Temporal stability: 99.9th percentile of the frame-to-frame change of the
/// luma error, pooled like [`spatial`] (σ = 1 px), in 8-bit levels. A
/// stable approximation has a still error; flicker, stepped motion and popping
/// make it change between frames.
fn flicker(e0: &[f32], e1: &[f32], w: usize, h: usize) -> f64 {
    let d: Vec<f32> = e0.iter().zip(e1).map(|(a, b)| a - b).collect();
    if d.iter().all(|v| *v == 0.0) {
        return 0.0;
    }
    let d = gauss(&d, w, h, 1.0);
    let mut hist = vec![0u64; 2561];
    for v in d {
        hist[((v.abs() * 10.0) as usize).min(2560)] += 1;
    }
    hist_pct(&hist, 0.1, 0.999)
}

#[derive(Clone, Default, Serialize)]
struct Fidelity {
    frames: usize,
    psnr_y_mean: f64,
    psnr_y_min: f64,
    ssim_mean: f64,
    ssim_min: f64,
    ssim_min_frame: usize,
    de_p999_max: f64,
    de_max: f64,
    de_max_frame: usize,
    raw_de_p999_max: f64,
    raw_de_max: f64,
    de00_p999_max: f64,
    flicker_p999_max: f64,
    flicker_frame: usize,
    /// Frames failing a threshold.
    failing: Vec<usize>,
}

/// Compare a sequence against the reference; writes the max-ΔE heatmap to `heat` if given.
fn fidelity(lab: &Lab, test: &[Frame], refs: &[Frame], heat: Option<&Path>, gate: bool) -> (Fidelity, Vec<FrameStats>) {
    let per: Vec<(FrameStats, Vec<f32>, Vec<f32>)> = test.par_iter().zip(refs.par_iter()).map(|(t, r)| compare(lab, t, r)).collect();
    let mut stats: Vec<FrameStats> = per.iter().map(|p| p.0.clone()).collect();
    let fl: Vec<f64> = (1..per.len()).into_par_iter().map(|k| flicker(&per[k - 1].2, &per[k].2, refs[k].w, refs[k].h)).collect();
    for (k, f) in fl.into_iter().enumerate() {
        stats[k + 1].flicker_p999 = f;
    }
    if let (Some(path), Some(r)) = (heat, refs.first()) {
        let mut m = vec![0f32; r.w * r.h];
        for p in &per {
            for (a, b) in m.iter_mut().zip(&p.1) {
                *a = a.max(*b);
            }
        }
        write_heat(path, &m, r.w, r.h, &refs[refs.len() / 2]);
    }
    let n = stats.len().max(1) as f64;
    let mut f = Fidelity { frames: stats.len(), psnr_y_min: 99.0, ssim_min: 1.0, ..Default::default() };
    for (k, s) in stats.iter().enumerate() {
        f.psnr_y_mean += s.psnr_y / n;
        f.ssim_mean += s.ssim / n;
        if s.psnr_y < f.psnr_y_min {
            f.psnr_y_min = s.psnr_y;
        }
        if s.ssim < f.ssim_min {
            f.ssim_min = s.ssim;
            f.ssim_min_frame = k;
        }
        f.de_p999_max = f.de_p999_max.max(s.de_p999);
        f.raw_de_p999_max = f.raw_de_p999_max.max(s.raw_de_p999);
        f.raw_de_max = f.raw_de_max.max(s.raw_de_max);
        f.de00_p999_max = f.de00_p999_max.max(s.de00_p999);
        if s.de_max > f.de_max {
            f.de_max = s.de_max;
            f.de_max_frame = k;
        }
        if s.flicker_p999 > f.flicker_p999_max {
            f.flicker_p999_max = s.flicker_p999;
            f.flicker_frame = k;
        }
        if gate && (s.ssim < MIN_SSIM || s.psnr_y < MIN_PSNR_Y || s.de_p999 > MAX_DE_P999 || s.flicker_p999 > MAX_FLICKER_P999) {
            f.failing.push(k);
        }
    }
    (f, stats)
}

/// Heatmap: the reference frame in dim grey, overlaid with max ΔE over the clip
/// (blue < 1 < green < 2.3 (JND) < yellow < 5 < red).
fn write_heat(path: &Path, de: &[f32], w: usize, h: usize, under: &Frame) {
    let mut img = image::RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let d = de[y * w + x];
            let g = (under.y[y * w + x] as f32 - 16.0).max(0.0) * 0.25;
            let c = if d <= 0.0 {
                [g, g, g]
            } else if d < 1.0 {
                [g, g, g + 120.0 * d]
            } else if d < 2.3 {
                [0.0, 200.0, 60.0]
            } else if d < 5.0 {
                [255.0, 220.0, 0.0]
            } else {
                [255.0, 30.0, 30.0]
            };
            img.put_pixel(x as u32, y as u32, image::Rgb(c.map(|v| v.clamp(0.0, 255.0) as u8)));
        }
    }
    let _ = img.save(path);
}

// ---------------------------------------------------------------- delivered

/// Decode an MP4 to tight 4:2:0 frames with ffmpeg.
fn decode(mp4: &Path, w: usize, h: usize) -> Result<Vec<Frame>, String> {
    let o = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(mp4)
        .args(["-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).into());
    }
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let fs = w * h + 2 * cw * ch;
    Ok(o.stdout
        .chunks_exact(fs)
        .map(|c| Frame { w, h, y: c[..w * h].to_vec(), u: c[w * h..w * h + cw * ch].to_vec(), v: c[w * h + cw * ch..].to_vec() })
        .collect())
}

// ---------------------------------------------------------------- main

#[derive(Serialize, Default)]
struct Row {
    name: String,
    kind: String,
    skipped: Option<String>,
    frames: usize,
    /// Frames in the timed renders (the whole spec with `time_full`).
    timed_frames: usize,
    size: String,
    startup_ms: f64,
    first_frame_ms: f64,
    wall_s: f64,
    fps: f64,
    profile: Option<pipeline::StageProfile>,
    bytes: u64,
    reference_s: f64,
    render: Fidelity,
    delivered: Fidelity,
    delivered_regressed: bool,
    /// Two default renders of the same clip disagreed.
    nondeterministic: Option<String>,
    pass: bool,
    flags: Vec<String>,
}

fn to_rgb(f: &Frame) -> image::RgbImage {
    let cw = f.w.div_ceil(2);
    image::RgbImage::from_fn(f.w as u32, f.h as u32, |x, y| {
        let (x, y) = (x as usize, y as usize);
        let (yy, u, v) = (f.y[y * f.w + x] as f32, f.u[(y / 2) * cw + x / 2] as f32, f.v[(y / 2) * cw + x / 2] as f32);
        let (yy, u, v) = ((yy - 16.0) * (255.0 / 219.0), (u - 128.0) * (255.0 / 224.0), (v - 128.0) * (255.0 / 224.0));
        let c = |x: f32| (x + 0.5).clamp(0.0, 255.0) as u8;
        image::Rgb([c(yy + 1.5748 * v), c(yy - 0.187324 * u - 0.468124 * v), c(yy + 1.8556 * u)])
    })
}

/// Debug: test, reference, ×16 luma error and ×16 change of error vs the previous frame.
fn dump(stem: &Path, t: &Frame, r: &Frame, prev: Option<(&Frame, &Frame)>) {
    let _ = to_rgb(t).save(stem.with_extension("test.png"));
    let _ = to_rgb(r).save(stem.with_extension("ref.png"));
    let e = |a: &Frame, b: &Frame, i: usize| a.y[i] as i32 - b.y[i] as i32;
    let img = image::GrayImage::from_fn(t.w as u32, t.h as u32, |x, y| image::Luma([(128 + 16 * e(t, r, y as usize * t.w + x as usize)).clamp(0, 255) as u8]));
    let _ = img.save(stem.with_extension("err.png"));
    if let Some((pt, pr)) = prev {
        let img = image::GrayImage::from_fn(t.w as u32, t.h as u32, |x, y| {
            let i = y as usize * t.w + x as usize;
            image::Luma([(128 + 16 * (e(t, r, i) - e(pt, pr, i))).clamp(0, 255) as u8])
        });
        let _ = img.save(stem.with_extension("flicker.png"));
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let has = |k: &str| args.iter().any(|a| a == k);
    let gate = has("--gate");
    let runs: usize = get("--runs").and_then(|v| v.parse().ok()).unwrap_or(3).max(1);
    let only: Option<Vec<String>> = get("--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    // Repo root: two levels up from this crate.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let out_dir = get("--out").map(PathBuf::from).unwrap_or_else(|| root.join("target/qbench"));
    std::fs::create_dir_all(&out_dir).unwrap();
    let corpus: Corpus = serde_json::from_str(&std::fs::read_to_string(root.join("crates/anikit-video/qbench/corpus.json")).unwrap()).unwrap();
    let base_path = root.join("crates/anikit-video/qbench/baseline.json");
    let baseline: Baseline = std::fs::read_to_string(&base_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut new_base = baseline.clone();
    let lab = Lab::new();
    let mut rows = Vec::new();
    for e in &corpus.entries {
        if only.as_ref().is_some_and(|o| !o.contains(&e.name)) {
            continue;
        }
        eprintln!("== {} ({})", e.name, e.kind);
        let mut row = Row { name: e.name.clone(), kind: e.kind.clone(), ..Default::default() };
        let spec_path = root.join(&e.spec);
        // Startup: parse + compile, then the first frame (eval + full render), as an agent's still would.
        let t = Instant::now();
        let ev = match build(&spec_path) {
            Ok(ev) => ev,
            Err(err) => {
                eprintln!("   skipped: {err}");
                row.skipped = Some(err.chars().take(120).collect());
                row.pass = true;
                rows.push(row);
                continue;
            }
        };
        row.startup_ms = t.elapsed().as_secs_f64() * 1e3;
        let (f0, f1) = range(&ev, e);
        let t = Instant::now();
        let _ = pipeline::render_rgba(&ev, pipeline::frame_time(&ev, f0), 1.0);
        row.first_frame_ms = t.elapsed().as_secs_f64() * 1e3;
        let n = (f1 - f0) as usize;
        let (w, h) = ((ev.width as usize) & !1, (ev.height as usize) & !1);
        row.frames = n;
        row.size = format!("{w}×{h}@{}", ev.fps);
        let mp4 = out_dir.join(format!("{}.mp4", e.name));
        let wav = out_dir.join(format!("{}.wav", e.name));
        let t_audio0 = e.from.unwrap_or(0.0);
        let dur = e.to.unwrap_or(ev.duration) - t_audio0;
        // 1. Timed default renders (fresh evaluator each time, so no warm caches carry over).
        anikit_video::components::set_draft(false);
        let mut walls = Vec::new();
        let runs = e.runs.unwrap_or(runs).min(runs);
        for run in 0..runs {
            let ev = build(&spec_path).unwrap();
            let ev = &ev;
            let t = Instant::now();
            let audio = pipeline::mix_audio(ev, t_audio0, dur, &wav).unwrap_or(None);
            let mut o = options(e, false);
            if e.time_full {
                (o.from, o.to) = (None, None);
            }
            let s = pipeline::render_ak264(ev, &o, &mp4, audio.as_deref()).expect("render");
            walls.push(t.elapsed().as_secs_f64());
            if audio.is_some() {
                let _ = std::fs::remove_file(&wav);
            }
            row.bytes = s.bytes;
            if run == runs / 2 || row.profile.is_none() {
                row.profile = s.profile;
            }
        }
        row.wall_s = median(walls);
        row.timed_frames = if e.time_full { ev.frames() as usize } else { n };
        row.fps = row.timed_frames as f64 / row.wall_s;
        if e.time_full {
            // The quality passes need the clip's own MP4.
            let ev = build(&spec_path).unwrap();
            let audio = pipeline::mix_audio(&ev, t_audio0, dur, &wav).unwrap_or(None);
            pipeline::render_ak264(&ev, &options(e, false), &mp4, audio.as_deref()).expect("render");
            if audio.is_some() {
                let _ = std::fs::remove_file(&wav);
            }
        }
        eprintln!("   default: {:.3} s, {:.0} fps", row.wall_s, row.fps);
        // 2. Reference.
        let t = Instant::now();
        let ev_r = build(&spec_path).unwrap();
        let refs = reference_frames(&ev_r, f0, n, w, h);
        drop(ev_r);
        row.reference_s = t.elapsed().as_secs_f64();
        eprintln!("   reference: {:.1} s", row.reference_s);
        // 3. Render fidelity: tap the default pipeline's frames.
        let ev_t = build(&spec_path).unwrap();
        let slots: Mutex<Vec<Option<Frame>>> = Mutex::new(vec![None; n]);
        let tap = |k: usize, b: &ak264::YuvBuffer| {
            let f = Frame::from_buffer(b);
            slots.lock().unwrap()[k] = Some(f);
        };
        let tmp = out_dir.join(format!("{}.tap.mp4", e.name));
        pipeline::render_ak264_tap(&ev_t, &options(e, false), &tmp, None, Some(&tap)).expect("tapped render");
        let test: Vec<Frame> = slots.into_inner().unwrap().into_iter().map(|f| f.expect("frame not tapped")).collect();
        // Determinism: a second default render of the clip must give the same frames.
        {
            let ev2 = build(&spec_path).unwrap();
            let slots2: Mutex<Vec<Option<Frame>>> = Mutex::new(vec![None; n]);
            let tap2 = |k: usize, b: &ak264::YuvBuffer| {
                slots2.lock().unwrap()[k] = Some(Frame::from_buffer(b));
            };
            pipeline::render_ak264_tap(&ev2, &options(e, false), &tmp, None, Some(&tap2)).expect("tapped render");
            let again: Vec<Frame> = slots2.into_inner().unwrap().into_iter().map(|f| f.unwrap()).collect();
            let diff: Vec<usize> = (0..n).filter(|&k| test[k].y != again[k].y || test[k].u != again[k].u || test[k].v != again[k].v).collect();
            if let Some(&k) = diff.first() {
                let (a, b) = (&test[k], &again[k]);
                let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
                for (i, (p, q)) in a.y.iter().zip(&b.y).enumerate() {
                    if p != q {
                        let (x, y) = (i % a.w, i / a.w);
                        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                    }
                }
                row.nondeterministic = Some(format!("{} frames differ between two renders, first {k} (luma box {x0},{y0}–{x1},{y1})", diff.len()));
                eprintln!("   NONDETERMINISTIC: {}", row.nondeterministic.as_ref().unwrap());
                if has("--dump-nondet") {
                    dump(&out_dir.join(format!("{}-nondet{k}", e.name)), a, b, None);
                }
            }
        }
        let _ = std::fs::remove_file(&tmp);
        for k in get("--dump").into_iter().flat_map(|v| v.split(',').filter_map(|x| x.parse::<usize>().ok()).collect::<Vec<_>>()) {
            if k < n {
                dump(&out_dir.join(format!("{}-f{k}", e.name)), &test[k], &refs[k], k.checked_sub(1).map(|j| (&test[j], &refs[j])));
            }
        }
        let heat = out_dir.join(format!("{}.heat.png", e.name));
        let (rf, rstats) = fidelity(&lab, &test, &refs, Some(&heat), true);
        drop(test);
        // 4. Delivered fidelity.
        let dec = decode(&mp4, w, h).unwrap_or_default();
        let (df, _) = if dec.len() == refs.len() { fidelity(&lab, &dec, &refs, None, false) } else { (Fidelity::default(), vec![]) };
        if dec.len() != refs.len() {
            row.flags.push(format!("decoded {} frames, expected {}", dec.len(), refs.len()));
        }
        let regress = baseline.delivered.get(&e.name).is_some_and(|(s, p)| df.ssim_min < s - DELIVERED_SSIM_SLACK || df.psnr_y_mean < p - DELIVERED_PSNR_SLACK);
        row.delivered_regressed = regress;
        if df.ssim_min < DELIVERED_FLAG_SSIM {
            row.flags.push(format!("delivered SSIM {:.4} (grain/noise vs QP)", df.ssim_min));
        }
        new_base.delivered.insert(e.name.clone(), ((df.ssim_min * 1e4).floor() / 1e4, (df.psnr_y_mean * 100.0).floor() / 100.0));
        if rf.de_max > FLAG_DE_MAX {
            row.flags.push(format!("max ΔE {:.1} at frame {} (see {})", rf.de_max, rf.de_max_frame, heat.file_name().unwrap().to_string_lossy()));
        }
        let worst = rstats.iter().map(|s| s.de_over).max().unwrap_or(0);
        if worst > 0 {
            row.flags.push(format!("{worst} px > 1 JND in the worst frame"));
        }
        row.pass = rf.failing.is_empty() && !row.delivered_regressed && row.nondeterministic.is_none();
        if let Some(nd) = &row.nondeterministic {
            row.flags.push(format!("NONDETERMINISTIC: {nd}"));
        }
        row.render = rf;
        row.delivered = df;
        let _ = std::fs::write(out_dir.join(format!("{}.frames.json", e.name)), serde_json::to_string(&rstats).unwrap());
        eprintln!("   render SSIM min {:.5}, PSNR-Y min {:.2}, ΔE76 p99.9 {:.2}, ΔE00 p99.9 {:.2}, flicker {:.1}; delivered SSIM min {:.4}", row.render.ssim_min, row.render.psnr_y_min, row.render.de_p999_max, row.render.de00_p999_max, row.render.flicker_p999_max, row.delivered.ssim_min);
        rows.push(row);
    }
    print_table(&rows);
    let _ = std::fs::write(out_dir.join("results.json"), serde_json::to_string_pretty(&rows).unwrap());
    if has("--update-baseline") {
        std::fs::write(&base_path, serde_json::to_string_pretty(&new_base).unwrap() + "\n").unwrap();
        eprintln!("baseline written: {}", base_path.display());
    }
    let failed: Vec<&str> = rows.iter().filter(|r| !r.pass).map(|r| r.name.as_str()).collect();
    if failed.is_empty() {
        eprintln!("qbench: PASS ({} specs)", rows.len());
    } else {
        eprintln!("qbench: FAIL: {}", failed.join(", "));
        if gate {
            std::process::exit(1);
        }
    }
}

fn print_table(rows: &[Row]) {
    println!("\n| spec | content | frames (timed) | wall s | fps | ms/frame: eval · render · copy · encode · idle | startup ms · 1st frame ms | render SSIM min | PSNR-Y min | ΔE p99.9 / max | raw ΔE p99.9 | flicker p99.9 | delivered SSIM min / PSNR-Y | result |");
    println!("|---|---|---:|---:|---:|---|---|---:|---:|---:|---:|---:|---|---|");
    for r in rows {
        if let Some(s) = &r.skipped {
            println!("| {} | {} | – | – | – | – | – | – | – | – | – | – | – | skipped: {} |", r.name, r.kind, s.replace('|', "/"));
            continue;
        }
        let p = r.profile.clone().unwrap_or_default();
        let res = if r.pass { "PASS".to_string() } else { format!("**FAIL** ({} frames{})", r.render.failing.len(), if r.delivered_regressed { ", delivered regressed" } else { "" }) };
        let flags = if r.flags.is_empty() { String::new() } else { format!(" ⚑ {}", r.flags.join("; ")) };
        println!(
            "| {} | {} | {} ({}) | {:.2} | {:.0} | {:.2} · {:.2} · {:.2} · {:.2} · {:.2} | {:.0} · {:.0} | {:.5} | {:.2} | {:.2} / {:.1} | {:.1} | {:.1} | {:.4} / {:.2} | {}{} |",
            r.name,
            r.kind,
            r.frames,
            r.timed_frames,
            r.wall_s,
            r.fps,
            p.eval_ms,
            p.render_ms,
            p.copy_ms,
            p.encode_ms,
            p.idle_ms,
            r.startup_ms,
            r.first_frame_ms,
            r.render.ssim_min,
            r.render.psnr_y_min,
            r.render.de_p999_max,
            r.render.de_max,
            r.render.raw_de_p999_max,
            r.render.flicker_p999_max,
            r.delivered.ssim_min,
            r.delivered.psnr_y_mean,
            res,
            flags
        );
    }
}
