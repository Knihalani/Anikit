//! `anikit-video analyze`: reverse-engineer a reference video.
//!
//! Decodes the clip once through ffmpeg (MP4, WebM, GIF, MOV or a frame
//! sequence) at 640 px wide, keeps a 160 px copy of every frame and a 640 px
//! frame every quarter second, and measures:
//!
//! - shots: hard cuts (colour-histogram jumps against a local median) and
//!   gradual transitions (a long-range jump with no single-frame cut), each
//!   classified as cut, crossfade, fade through black/white, flash, whip,
//!   zoom or wipe;
//! - per shot: an OKLab k-means palette, the luminance key and contrast,
//!   camera motion (a similarity model fitted to block motion), a motion
//!   energy curve, depth of field, a flat-2D vs rendered/photographic guess,
//!   text regions, grain, vignette, glow and a technique guess;
//! - audio (when present): tempo, beat grid, downbeats, onsets, hits,
//!   loudness, and how many cuts land on a beat.
//!
//! Everything is a heuristic with a stated meaning; the skill
//! (`skills/anikit-reverse-video/SKILL.md`) pairs it with an agent looking
//! at the contact sheet.

mod board;
mod draft;
mod image_ops;
mod sheet;

pub use board::{board, board_sheet, collect as collect_images, Board};
pub use draft::draft_spec;
pub use sheet::contact_sheet;

use image_ops::*;
use serde::Serialize;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

/// Width of the stored key frames (and of the decode).
pub const KEY_W: usize = 640;
/// Width of the per-frame analysis copy.
pub const SMALL_W: usize = 160;

/// Analysis options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Frame rate to analyse at (the source rate when lower). Default 30.
    pub max_fps: f64,
    /// Frame rate of an image sequence (a directory or a `%04d` pattern). Default 30.
    pub sequence_fps: f64,
    /// Analyse the audio track. Default true.
    pub audio: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { max_fps: 30.0, sequence_fps: 30.0, audio: true }
    }
}

/// The analysis report (`analysis.json`).
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub source: String,
    pub global: Global,
    pub shots: Vec<Shot>,
    pub transitions: Vec<Transition>,
    /// Single-frame flashes that are not cuts (the shot continues after).
    pub flashes: Vec<f64>,
    pub audio: Option<AudioInfo>,
    /// Seconds spent analysing, by stage.
    pub timing: Timing,
    /// What each measure means and how much to trust it.
    pub notes: Vec<&'static str>,
    #[serde(skip)]
    pub(crate) keyframes: Vec<KeyFrame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Global {
    pub width: u32,
    pub height: u32,
    /// `16:9`, `9:16`, `1:1`, `4:5`, `21:9`, or `W:H`.
    pub aspect: String,
    pub fps: f64,
    /// Rate the analysis ran at.
    pub analysed_fps: f64,
    pub duration: f64,
    pub shot_count: usize,
    pub median_shot: f64,
    pub mean_shot: f64,
    pub longest_shot: f64,
    pub shortest_shot: f64,
    pub cuts_per_minute: f64,
    /// `frantic`, `fast`, `steady` or `calm` (the `intent.pace` scale of `docs pacing`).
    pub pace: String,
    /// Palette over the whole clip, by screen share.
    pub palette: Vec<Swatch>,
    /// Transition types and counts.
    pub transition_mix: Vec<(String, usize)>,
    /// Shots per technique guess.
    pub technique_mix: Vec<(String, usize)>,
    pub mean_luma: f32,
    pub grain: f32,
    pub vignette: f32,
    pub glow: f32,
    /// One paragraph: what the clip looks like and how it is cut.
    pub style_summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Swatch {
    pub hex: String,
    /// Share of the sampled pixels, 0…1.
    pub share: f32,
    /// OKLab lightness and chroma.
    pub l: f32,
    pub c: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Shot {
    pub index: usize,
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    /// How this shot comes in (`start` for the first).
    pub transition_in: String,
    pub palette: Vec<Swatch>,
    /// Mean OKLab lightness (0 black … 1 white).
    pub luma: f32,
    /// Std of OKLab lightness.
    pub contrast: f32,
    /// Mean OKLab chroma.
    pub saturation: f32,
    /// `low-key`, `mid-key` or `high-key`.
    pub key: String,
    pub camera: Camera,
    /// Motion energy, 4 values per second: mean block motion in % of frame width per frame.
    pub motion: Vec<f32>,
    pub motion_mean: f32,
    /// Depth cues on the key frame.
    pub depth: Depth,
    /// `flat-2d` (hard edges between fills), `rendered-3d` (smooth shading, clean),
    /// `photographic` (smooth shading with grain) or `minimal` (nearly empty frame);
    /// `look_score` 0 = hard-edged vector … 1 = all smooth shading.
    pub look: String,
    pub look_score: f32,
    pub text: Vec<TextRegion>,
    /// Share of the frame covered by text boxes.
    pub text_coverage: f32,
    /// Std of the high-pass residual in the flattest tiles (0.004 clean … 0.03 heavy).
    pub grain: f32,
    /// Corner darkening vs centre, 0…1 (content can mimic it; trust it when most shots agree).
    pub vignette: f32,
    /// Halo brightness around clipped highlights, 0…1.
    pub glow: f32,
    /// Small isolated bright points on the key frame (particles, stars, sparks).
    pub points: usize,
    /// Best guess at how to build this shot in Anikit, most likely first.
    pub technique: Vec<String>,
    /// Time of the key frame (on the sheet).
    pub key_time: f64,
    /// Tone and colour of the grade.
    pub grade: Grade,
    /// Composition of the key frame.
    pub layout: Layout,
    /// How the motion is timed over the shot: `snap-then-settle` (ease-out),
    /// `builds` (ease-in), `steady` (linear drift), `bursts` (hits) or `still`.
    pub motion_character: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Camera {
    /// Total pan of the content over the shot, in frame widths / heights (+x right, +y down).
    pub pan_x: f32,
    pub pan_y: f32,
    /// Content scale at the end vs the start (> 1 = pushing in).
    pub zoom: f32,
    /// Content rotation over the shot, degrees.
    pub rotate: f32,
    /// Frame-to-frame jitter left after the smooth move (% of width): shake or handheld.
    pub shake: f32,
    /// `static`, `pan left`, `push in`, `pull out`, `roll`, `handheld`, …
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Grade {
    /// OKLab lightness at the 2nd, 50th and 98th percentiles: the black point (lift), mid (gamma) and white point (gain).
    pub black: f32,
    pub mid: f32,
    pub white: f32,
    /// Mean OKLab b (+ warm/yellow, − cool/blue) and a (+ magenta, − green) of the mid-tones.
    pub warmth: f32,
    pub tint: f32,
    /// e.g. `lifted blacks, soft highlights, warm, desaturated`.
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Layout {
    /// Left-right mirror similarity of the key frame, 0…1 (1 = symmetric).
    pub symmetry: f32,
    /// Centre of visual weight (contrast against the mean), 0…1 in x and y.
    pub weight_x: f32,
    pub weight_y: f32,
    /// `centred`, `left-weighted`, `right-weighted`, `top-heavy`, `bottom-heavy`.
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Depth {
    /// Sharpest tile's mean |Laplacian| on the 640 px key frame.
    pub sharpness: f32,
    /// Share of textured tiles much softer than the sharpest (defocus).
    pub defocus_share: f32,
    /// `shallow`, `deep` or `soft` (everything blurred).
    pub dof: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextRegion {
    /// Box in 0…1 frame units: x, y (top-left), w, h.
    pub bbox: [f32; 4],
    /// Line height relative to frame height (×100 = the `vh` size to use).
    pub height: f32,
    /// `left`, `center` or `right` (from the box position).
    pub align: String,
    /// `top`, `middle` or `bottom` third.
    pub position: String,
    /// Edge density in the box: higher means more confidently text.
    pub confidence: f32,
    /// `light-on-dark` or `dark-on-light`.
    pub ink: String,
    /// Share of the box covered by ink: ≈ 0.12 light/thin, 0.2 regular, ≥ 0.3 bold/black.
    pub ink_share: f32,
    /// `thin`, `regular` or `bold` from `ink_share`.
    pub weight: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Transition {
    /// Time of the cut (middle of a gradual transition).
    pub at: f64,
    pub duration: f64,
    /// `cut`, `crossfade`, `fadeBlack`, `fadeWhite`, `flash`, `whip`, `zoom`, `wipe`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `left`/`right`/`up`/`down` for whips and wipes.
    pub direction: Option<String>,
    /// Cut strength (histogram distance, 0…1).
    pub strength: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioInfo {
    pub bpm: f64,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
    pub onsets: Vec<f64>,
    /// The strongest onsets (impacts): the loudest 10% of jumps.
    pub hits: Vec<f64>,
    /// RMS loudness, 0…1, 4 values per second.
    pub loudness: Vec<f32>,
    /// Share of cuts within 2 frames (or 70 ms) of a beat.
    pub cuts_on_beat: f32,
    /// Share of cuts on a downbeat.
    pub cuts_on_downbeat: f32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Timing {
    pub decode: f64,
    pub analyse: f64,
    pub audio: f64,
    pub total: f64,
}

/// A stored 640 px RGB frame.
#[derive(Debug, Clone)]
pub(crate) struct KeyFrame {
    pub t: f64,
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

/// Per-frame measures on the 160 px copy.
struct FrameStat {
    hist: [f32; 64],
    luma: f32,
    /// Normalised luma plane.
    l: Vec<f32>,
    rgb: Vec<u8>,
    sharp: f32,
}

#[derive(Clone, Copy, Default)]
struct Motion {
    tx: f32,
    ty: f32,
    /// Scale - 1 per frame.
    a: f32,
    /// Rotation per frame (radians).
    th: f32,
    /// Mean block vector magnitude (px at 160).
    mag: f32,
}

struct Probe {
    w: u32,
    h: u32,
    fps: f64,
    audio: bool,
}

fn input_args(path: &Path, o: &Options) -> Result<Vec<String>, String> {
    let s = path.to_string_lossy().to_string();
    if path.is_dir() {
        let mut exts: Vec<String> = std::fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok()?.path().extension().map(|x| x.to_string_lossy().to_lowercase()))
            .filter(|x| ["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"].contains(&x.as_str()))
            .collect();
        exts.sort();
        exts.dedup();
        let ext = exts.first().ok_or_else(|| format!("{s}: no image frames in this directory"))?;
        return Ok(vec!["-framerate".into(), format!("{}", o.sequence_fps), "-pattern_type".into(), "glob".into(), "-i".into(), format!("{}/*.{ext}", s.trim_end_matches('/'))]);
    }
    if s.contains('%') {
        return Ok(vec!["-framerate".into(), format!("{}", o.sequence_fps), "-i".into(), s]);
    }
    if !path.exists() {
        return Err(format!("{s}: no such file"));
    }
    Ok(vec!["-i".into(), s])
}

fn probe(args: &[String]) -> Result<Probe, String> {
    // ffprobe takes the same input options as ffmpeg.
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-show_format", "-of", "json"])
        .args(args.iter().filter(|a| *a != "-i"))
        .output()
        .map_err(|e| format!("analyze needs ffmpeg/ffprobe on PATH: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffprobe: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let vs = streams.iter().find(|s| s["codec_type"] == "video").ok_or("no video stream")?;
    let rate = |k: &str| -> Option<f64> {
        let r = vs[k].as_str()?;
        let (a, b) = r.split_once('/')?;
        let (a, b): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
        (b > 0.0 && a > 0.0).then_some(a / b)
    };
    let fps = rate("avg_frame_rate").filter(|f| *f < 240.0).or(rate("r_frame_rate")).unwrap_or(30.0);
    Ok(Probe {
        w: vs["width"].as_u64().unwrap_or(0) as u32,
        h: vs["height"].as_u64().unwrap_or(0) as u32,
        fps,
        audio: streams.iter().any(|s| s["codec_type"] == "audio"),
    })
}

/// Analyse a video file, a GIF, a directory of frames, or a `frame%04d.png` pattern.
pub fn analyze(path: &Path, o: &Options) -> Result<Analysis, String> {
    let t0 = std::time::Instant::now();
    let args = input_args(path, o)?;
    let p = probe(&args)?;
    if p.w == 0 || p.h == 0 {
        return Err("can't read the video size".into());
    }
    let fps = p.fps.min(o.max_fps).max(1.0);
    let kh = (((KEY_W as f64 * p.h as f64 / p.w as f64) / 4.0).round() as usize * 4).max(8);
    let kw = KEY_W;
    let (sw, sh) = (kw / 4, kh / 4);
    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-threads", "0"])
        .args(&args)
        .args(["-an", "-vf", &format!("fps={fps},scale={kw}:{kh}:flags=area,format=rgb24"), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("analyze needs ffmpeg on PATH: {e}"))?;
    let mut stdout = child.stdout.take().ok_or("no ffmpeg stdout")?;
    // Audio decodes on a thread while frames stream in.
    let audio_job = (o.audio && p.audio).then(|| {
        let a = args.clone();
        std::thread::spawn(move || decode_audio(&a))
    });
    let fsz = kw * kh * 3;
    let mut buf = vec![0u8; fsz];
    let mut frames: Vec<FrameStat> = Vec::new();
    let mut keys: Vec<KeyFrame> = Vec::new();
    let key_every = (fps / 4.0).round().max(1.0) as usize;
    let mut n = 0usize;
    // Decode and reduce in batches so the per-frame maths runs in parallel.
    let mut batch: Vec<(usize, Vec<u8>)> = Vec::new();
    while stdout.read_exact(&mut buf).is_ok() {
        if n.is_multiple_of(key_every) {
            keys.push(KeyFrame { t: n as f64 / fps, w: kw, h: kh, rgb: buf.clone() });
        }
        batch.push((n, buf.clone()));
        n += 1;
        if batch.len() == 64 {
            frames.extend(reduce_batch(std::mem::take(&mut batch), kw, kh));
        }
    }
    frames.extend(reduce_batch(batch, kw, kh));
    let st = child.wait_with_output().map_err(|e| e.to_string())?;
    if frames.is_empty() {
        return Err(format!("ffmpeg decoded no frames: {}", String::from_utf8_lossy(&st.stderr).trim()));
    }
    let t_decode = t0.elapsed().as_secs_f64();
    let t1 = std::time::Instant::now();

    // Frame-to-frame motion (parallel).
    use rayon::prelude::*;
    let motion: Vec<Motion> = (0..frames.len()).into_par_iter().map(|i| if i == 0 { Motion::default() } else { global_motion(&frames[i - 1].l, &frames[i].l, sw, sh) }).collect();

    let (transitions, flashes) = detect_transitions(&frames, &motion, fps, sw);
    let duration = frames.len() as f64 / fps;
    // Shot boundaries: cut times.
    let mut bounds = vec![0.0];
    bounds.extend(transitions.iter().map(|t| t.at));
    bounds.push(duration);
    let shots: Vec<Shot> = (0..bounds.len() - 1)
        .into_par_iter()
        .map(|i| {
            let tin = if i == 0 { "start".to_string() } else { transitions[i - 1].kind.clone() };
            // Exclude the transition windows from the shot's own measures.
            let pad_a = if i == 0 { 0.0 } else { transitions[i - 1].duration / 2.0 };
            let pad_b = if i + 1 < bounds.len() - 1 { transitions[i].duration / 2.0 } else { 0.0 };
            analyse_shot(i, bounds[i], bounds[i + 1], pad_a, pad_b, tin, &frames, &motion, &keys, fps, sw, sh)
        })
        .collect();
    let t_analyse = t1.elapsed().as_secs_f64();

    let t2 = std::time::Instant::now();
    let audio = match audio_job {
        Some(j) => match j.join().map_err(|_| "audio thread panicked")? {
            Ok(Some(pcm)) => Some(audio_info(&pcm, &transitions, fps)),
            _ => None,
        },
        None => None,
    };
    let t_audio = t2.elapsed().as_secs_f64();

    let global = summarise(&p, fps, duration, &shots, &transitions, &audio);
    let key_ts: Vec<f64> = shots.iter().map(|s| s.key_time).collect();
    let keyframes = key_ts.iter().map(|&t| nearest_key(&keys, t).clone()).collect();
    Ok(Analysis {
        source: path.to_string_lossy().to_string(),
        global,
        shots,
        transitions,
        flashes,
        audio,
        timing: Timing { decode: round3(t_decode), analyse: round3(t_analyse), audio: round3(t_audio), total: round3(t0.elapsed().as_secs_f64()) },
        notes: vec![
            "Heuristics, not ground truth: confirm every guess by looking at the sheet and key frames.",
            "transition types: cut, crossfade, fadeBlack, fadeWhite, flash (bright frame at the cut), whip (blurred fast pan at the cut), zoom, wipe.",
            "camera: a similarity model (pan, zoom, roll) fitted to 8×8 block motion at 160 px; pan_x/pan_y in frame widths/heights over the shot.",
            "look: flat-2d = hard edges between flat fills (vector motion graphics); rendered-3d = smooth shading, clean pixels; photographic = smooth shading with grain; minimal = a nearly empty frame.",
            "text: boxes of dense high-contrast strokes; no OCR. height ×100 ≈ the vh size to use.",
            "cuts inside very fast action (every frame changes a lot) can be missed: a long shot with high `motion` may hide several; check its frames.",
            "grain/vignette/glow: relative measures; compare shots against each other and against your recreation's analysis.",
        ],
        keyframes,
    })
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn nearest_key(keys: &[KeyFrame], t: f64) -> &KeyFrame {
    keys.iter().min_by(|a, b| (a.t - t).abs().total_cmp(&(b.t - t).abs())).expect("at least one key frame")
}

fn reduce_batch(batch: Vec<(usize, Vec<u8>)>, kw: usize, kh: usize) -> Vec<FrameStat> {
    use rayon::prelude::*;
    batch.into_par_iter().map(|(_, px)| reduce(&px, kw, kh)).collect()
}

fn reduce(px: &[u8], kw: usize, kh: usize) -> FrameStat {
    let (sw, sh) = (kw / 4, kh / 4);
    let mut rgb = vec![0u8; sw * sh * 3];
    let mut l = vec![0f32; sw * sh];
    let mut hist = [0f32; 64];
    let mut sum = 0.0;
    for y in 0..sh {
        for x in 0..sw {
            let mut acc = [0u32; 3];
            for dy in 0..4 {
                let row = ((y * 4 + dy) * kw + x * 4) * 3;
                for dx in 0..4 {
                    for c in 0..3 {
                        acc[c] += px[row + dx * 3 + c] as u32;
                    }
                }
            }
            let o = y * sw + x;
            let c = [(acc[0] / 16) as u8, (acc[1] / 16) as u8, (acc[2] / 16) as u8];
            rgb[o * 3..o * 3 + 3].copy_from_slice(&c);
            let lv = (0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32) / 255.0;
            l[o] = lv;
            sum += lv;
            // Soft (trilinear) binning: a slow colour change moves the histogram smoothly.
            let bin = |v: u8| {
                let p = (v as f32 / 64.0 - 0.5).clamp(0.0, 3.0);
                let lo = (p as usize).min(2);
                let t = p - lo as f32;
                (lo, t)
            };
            let (r, g, bb) = (bin(c[0]), bin(c[1]), bin(c[2]));
            for (dr, wr) in [(0, 1.0 - r.1), (1, r.1)] {
                for (dg, wg) in [(0, 1.0 - g.1), (1, g.1)] {
                    for (db, wb) in [(0, 1.0 - bb.1), (1, bb.1)] {
                        hist[(r.0 + dr) * 16 + (g.0 + dg) * 4 + bb.0 + db] += wr * wg * wb;
                    }
                }
            }
        }
    }
    let n = (sw * sh) as f32;
    for h in &mut hist {
        *h /= n;
    }
    let sharp = mean_abs_laplacian(&l, sw, sh);
    FrameStat { hist, luma: sum / n, l, rgb, sharp }
}

fn hist_dist(a: &[f32; 64], b: &[f32; 64]) -> f32 {
    0.5 * a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>()
}

fn mad(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len().max(1) as f32
}

/// Block motion (8×8 blocks, two-level search) fitted with a similarity model.
fn global_motion(prev: &[f32], cur: &[f32], w: usize, h: usize) -> Motion {
    let (hw, hh) = (w / 2, h / 2);
    let half = |p: &[f32]| {
        let mut o = vec![0f32; hw * hh];
        for y in 0..hh {
            for x in 0..hw {
                o[y * hw + x] = 0.25 * (p[2 * y * w + 2 * x] + p[2 * y * w + 2 * x + 1] + p[(2 * y + 1) * w + 2 * x] + p[(2 * y + 1) * w + 2 * x + 1]);
            }
        }
        o
    };
    let (p2, c2) = (half(prev), half(cur));
    let b = 8usize;
    let sad = |p: &[f32], c: &[f32], ww: usize, hh2: usize, bx: usize, by: usize, bs: usize, dx: i32, dy: i32| -> f32 {
        let mut s = 0.0;
        for y in 0..bs {
            let sy = (by + y) as i32 + dy;
            if sy < 0 || sy >= hh2 as i32 {
                return f32::MAX;
            }
            for x in 0..bs {
                let sx = (bx + x) as i32 + dx;
                if sx < 0 || sx >= ww as i32 {
                    return f32::MAX;
                }
                s += (c[(by + y) * ww + bx + x] - p[sy as usize * ww + sx as usize]).abs();
            }
        }
        s
    };
    let mut vecs: Vec<(f32, f32, f32, f32, f32)> = Vec::new(); // x, y (centred), vx, vy, weight
    // Skip the outer ring of blocks: their matches fall off the frame.
    let mut by = b;
    while by + 2 * b <= h {
        let mut bx = b;
        while bx + 2 * b <= w {
            // Texture: skip flat blocks (no reliable motion).
            let mut mn = f32::MAX;
            let mut mx = f32::MIN;
            for y in 0..b {
                for x in 0..b {
                    let v = cur[(by + y) * w + bx + x];
                    mn = mn.min(v);
                    mx = mx.max(v);
                }
            }
            if mx - mn > 0.06 {
                // Coarse ±5 at half size, then ±2 at full size.
                let (cbx, cby) = (bx / 2, by / 2);
                let mut best = (f32::MAX, 0i32, 0i32);
                for dy in -5..=5 {
                    for dx in -5..=5 {
                        let s = sad(&p2, &c2, hw, hh, cbx, cby, b / 2, -dx, -dy);
                        if s < best.0 {
                            best = (s, dx, dy);
                        }
                    }
                }
                let zero = sad(&p2, &c2, hw, hh, cbx, cby, b / 2, 0, 0);
                if zero <= best.0 * 1.02 {
                    best = (zero, 0, 0);
                }
                let (cx, cy) = (best.1 * 2, best.2 * 2);
                let mut fine = (f32::MAX, cx, cy);
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        let s = sad(prev, cur, w, h, bx, by, b, -(cx + dx), -(cy + dy));
                        if s < fine.0 {
                            fine = (s, cx + dx, cy + dy);
                        }
                    }
                }
                let x = bx as f32 + b as f32 / 2.0 - w as f32 / 2.0;
                let y = by as f32 + b as f32 / 2.0 - h as f32 / 2.0;
                vecs.push((x, y, fine.1 as f32, fine.2 as f32, (mx - mn).min(0.5)));
            }
            bx += b;
        }
        by += b;
    }
    if vecs.len() < 6 {
        return Motion::default();
    }
    let mag = vecs.iter().map(|v| (v.2 * v.2 + v.3 * v.3).sqrt()).sum::<f32>() / vecs.len() as f32;
    let mut m = fit_similarity(&vecs);
    // One robust pass: drop blocks far from the model (moving objects).
    let res: Vec<f32> = vecs.iter().map(|v| residual(&m, v)).collect();
    let mut sorted = res.clone();
    sorted.sort_by(f32::total_cmp);
    let cut = sorted[sorted.len() * 7 / 10].max(1.0);
    let kept: Vec<_> = vecs.iter().zip(&res).filter(|(_, r)| **r <= cut * 1.5).map(|(v, _)| *v).collect();
    if kept.len() >= 6 {
        m = fit_similarity(&kept);
    }
    m.mag = mag;
    m
}

fn residual(m: &Motion, v: &(f32, f32, f32, f32, f32)) -> f32 {
    let px = m.tx + m.a * v.0 - m.th * v.1;
    let py = m.ty + m.a * v.1 + m.th * v.0;
    ((px - v.2).powi(2) + (py - v.3).powi(2)).sqrt()
}

/// Least squares for vx = tx + a·x − θ·y, vy = ty + a·y + θ·x.
fn fit_similarity(v: &[(f32, f32, f32, f32, f32)]) -> Motion {
    let mut ata = [[0f64; 4]; 4];
    let mut atb = [0f64; 4];
    for &(x, y, vx, vy, w) in v {
        let (x, y, vx, vy, w) = (x as f64, y as f64, vx as f64, vy as f64, w as f64);
        for (row, rhs) in [([1.0, 0.0, x, -y], vx), ([0.0, 1.0, y, x], vy)] {
            for i in 0..4 {
                for j in 0..4 {
                    ata[i][j] += w * row[i] * row[j];
                }
                atb[i] += w * row[i] * rhs;
            }
        }
    }
    for (i, r) in ata.iter_mut().enumerate() {
        r[i] += 1e-6;
    }
    let s = solve4(ata, atb).unwrap_or([0.0; 4]);
    Motion { tx: s[0] as f32, ty: s[1] as f32, a: s[2] as f32, th: s[3] as f32, mag: 0.0 }
}

fn solve4(mut a: [[f64; 4]; 4], mut b: [f64; 4]) -> Option<[f64; 4]> {
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in 0..4 {
            if r != c {
                let f = a[r][c] / a[c][c];
                let pivot = a[c];
                for (x, p) in a[r].iter_mut().zip(pivot) {
                    *x -= f * p;
                }
                b[r] -= f * b[c];
            }
        }
    }
    Some([b[0] / a[0][0], b[1] / a[1][1], b[2] / a[2][2], b[3] / a[3][3]])
}

fn median(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    s[s.len() / 2]
}

/// Hard cuts, gradual transitions and isolated flashes.
fn detect_transitions(f: &[FrameStat], m: &[Motion], fps: f64, sw: usize) -> (Vec<Transition>, Vec<f64>) {
    let n = f.len();
    let mut d = vec![0f32; n];
    let mut px = vec![0f32; n];
    for i in 1..n {
        // Colour histogram jump plus pixel difference: both must move for a cut.
        px[i] = (mad(&f[i].l, &f[i - 1].l) * 4.0).min(1.0);
        d[i] = 0.6 * hist_dist(&f[i].hist, &f[i - 1].hist) + 0.4 * px[i];
    }
    if let Ok(r) = std::env::var("ANIKIT_ANALYZE_DEBUG") {
        if let Some((a, b)) = r.split_once('-').and_then(|(a, b)| Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?))) {
            for i in ((a * fps) as usize).max(1)..((b * fps) as usize).min(n) {
                eprintln!("{:.3} d {:.3} hist {:.3} px {:.3}", i as f64 / fps, d[i], hist_dist(&f[i].hist, &f[i - 1].hist), px[i]);
            }
        }
    }
    // A cut changes the pixels in one frame; a dissolve spreads it out.
    let spike = |i: usize| px[i] > 1.4 * px[i - 1].max(if i + 1 < n { px[i + 1] } else { 0.0 }) + 0.02;
    let win = (fps * 0.5).round().max(3.0) as usize;
    let mut out: Vec<Transition> = Vec::new();
    let mut flashes = Vec::new();
    let mut i = 1;
    while i < n {
        let lo = i.saturating_sub(win);
        let hi = (i + win + 1).min(n);
        let local: Vec<f32> = (lo..hi).filter(|&j| j != i && j > 0).map(|j| d[j]).collect();
        let base = median(&local);
        if d[i] > 0.28 && d[i] > base * 3.0 + 0.08 && d[i] >= d[i.saturating_sub(1)] && (i + 1 >= n || d[i] >= d[i + 1]) && (spike(i) || hist_dist(&f[i].hist, &f[i - 1].hist) > 0.3) {
            // A bright frame that returns to the same picture is a flash, not a cut.
            let bright = f[i].luma > f[i - 1].luma + 0.25 && f[i].luma > 0.7;
            if bright {
                let back = (i + 1..(i + 5).min(n)).find(|&j| f[j].luma < f[i].luma - 0.2);
                if let Some(j) = back {
                    let t = i as f64 / fps;
                    if hist_dist(&f[j].hist, &f[i - 1].hist) < 0.3 {
                        flashes.push(round3(t));
                    } else {
                        out.push(Transition { at: round3(t), duration: round3((j - i) as f64 / fps), kind: "flash".into(), direction: None, strength: d[i] });
                    }
                    i = j + 1;
                    continue;
                }
            }
            let (kind, dir) = classify_cut(f, m, i, fps, sw);
            out.push(Transition { at: round3(i as f64 / fps), duration: 0.0, kind, direction: dir, strength: d[i] });
            i += 2;
            continue;
        }
        i += 1;
    }
    // Gradual transitions: a long-range jump with no hard cut inside it.
    let k = (fps * 0.3).round().max(2.0) as usize;
    let mut j = k;
    let gmed = median(&px[1..]);
    while j + k < n {
        let near_cut = out.iter().any(|t| ((t.at * fps) as isize - j as isize).unsigned_abs() <= 2 * k);
        let dl = hist_dist(&f[j - k].hist, &f[j + k].hist);
        let steps_max = (j - k + 1..=j + k).map(|q| px[q]).fold(0.0, f32::max);
        let steps_mean = (j - k + 1..=j + k).map(|q| px[q]).sum::<f32>() / (2 * k) as f32;
        let pan = (j - k + 1..=j + k).map(|q| m[q].tx.hypot(m[q].ty)).sum::<f32>() / (2 * k) as f32;
        let zoom = (j - k + 1..=j + k).map(|q| m[q].a.abs()).sum::<f32>() / (2 * k) as f32;
        // A peak of the long-range distance (a steady pan is high everywhere).
        let dq = |q: isize| if q >= k as isize && (q as usize) + k < n { hist_dist(&f[q as usize - k].hist, &f[q as usize + k].hist) } else { 0.0 };
        let around = dq(j as isize - 2 * k as isize).max(dq((j + 2 * k) as isize));
        if !near_cut && dl > 0.28 && steps_max < 0.28 && dl > around * 1.6 + 0.04 && steps_mean > gmed * 0.5 {
            // Find the peak inside the window.
            let mut best = (dl, j);
            for q in j..(j + k).min(n - k) {
                let v = hist_dist(&f[q - k].hist, &f[q + k].hist);
                if v > best.0 {
                    best = (v, q);
                }
            }
            let c = best.1;
            let (a, b, mid) = (&f[c - k], &f[c + k], &f[c]);
            let (kind, dir) = if mid.luma < 0.06 && a.luma > mid.luma + 0.08 && b.luma > mid.luma + 0.08 {
                ("fadeBlack", None)
            } else if mid.luma > 0.92 && a.luma < mid.luma - 0.08 && b.luma < mid.luma - 0.08 {
                ("fadeWhite", None)
            } else if blend_residual(a, b, mid) < 0.4 {
                ("crossfade", None)
            } else if let Some(dir) = wipe_dir(a, b, mid, sw) {
                ("wipe", Some(dir))
            } else if zoom > 0.02 {
                ("zoom", None)
            } else if pan > 2.5 {
                ("whip", Some(dir_of(m, c - k, c + k)))
            } else {
                ("crossfade", None)
            };
            out.push(Transition { at: round3(c as f64 / fps), duration: round3(2.0 * k as f64 / fps), kind: kind.into(), direction: dir, strength: best.0 });
            j = c + 3 * k;
            continue;
        }
        j += 1;
    }
    out.sort_by(|a, b| a.at.total_cmp(&b.at));
    // Drop near-duplicates (within 3 frames).
    let mut dedup: Vec<Transition> = Vec::new();
    for t in out {
        if dedup.last().is_some_and(|l| t.at - l.at < (3.0 / fps).max(0.25)) {
            continue;
        }
        dedup.push(t);
    }
    (dedup, flashes)
}

/// How well the middle frame is a mix of the two ends: residual of the best
/// `(1 − k)·a + k·b` fit, relative to |a − b| (0 = a perfect dissolve).
fn blend_residual(a: &FrameStat, b: &FrameStat, mid: &FrameStat) -> f32 {
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for i in 0..a.l.len() {
        let d = (b.l[i] - a.l[i]) as f64;
        num += (mid.l[i] - a.l[i]) as f64 * d;
        den += d * d;
    }
    if den < 1e-6 {
        return 1.0;
    }
    let k = (num / den).clamp(0.0, 1.0) as f32;
    let (mut res, mut diff) = (0.0f32, 0.0f32);
    for i in 0..a.l.len() {
        let pred = a.l[i] + k * (b.l[i] - a.l[i]);
        res += (mid.l[i] - pred).abs();
        diff += (b.l[i] - a.l[i]).abs();
    }
    res / diff.max(1e-6)
}

fn dir_of(m: &[Motion], a: usize, b: usize) -> String {
    let (sx, sy) = (a..=b.min(m.len() - 1)).fold((0.0, 0.0), |acc, q| (acc.0 + m[q].tx, acc.1 + m[q].ty));
    // Content moving left = camera moving right; name the camera direction.
    if sx.abs() >= sy.abs() {
        if sx < 0.0 { "right" } else { "left" }.into()
    } else if sy < 0.0 {
        "down".into()
    } else {
        "up".into()
    }
}

/// Mid-transition pixels closer to the end frame than the start: a straight
/// split across columns or rows is a wipe.
fn wipe_dir(a: &FrameStat, b: &FrameStat, mid: &FrameStat, sw: usize) -> Option<String> {
    let sh = a.l.len() / sw;
    let mut cols = vec![(0f32, 0f32); sw];
    let mut rows = vec![(0f32, 0f32); sh];
    for (y, row) in rows.iter_mut().enumerate() {
        for (x, col) in cols.iter_mut().enumerate() {
            let i = y * sw + x;
            let (da, db) = ((mid.l[i] - a.l[i]).abs(), (mid.l[i] - b.l[i]).abs());
            if (a.l[i] - b.l[i]).abs() < 0.08 {
                continue;
            }
            let v = if db < da { 1.0 } else { 0.0 };
            col.0 += v;
            col.1 += 1.0;
            row.0 += v;
            row.1 += 1.0;
        }
    }
    let step = |p: &[(f32, f32)]| -> Option<(f32, bool)> {
        let fr: Vec<f32> = p.iter().filter(|c| c.1 > 2.0).map(|c| c.0 / c.1).collect();
        if fr.len() < 8 {
            return None;
        }
        let decided = fr.iter().filter(|v| **v > 0.85 || **v < 0.15).count() as f32 / fr.len() as f32;
        let ends = fr.iter().filter(|v| **v > 0.85).count() as f32 / fr.len() as f32;
        // The "new" side: is it at the start or the end of the axis?
        let first = fr[..fr.len() / 2].iter().sum::<f32>() > fr[fr.len() / 2..].iter().sum::<f32>();
        (decided > 0.85 && ends > 0.15 && ends < 0.85).then_some((decided, first))
    };
    match (step(&cols), step(&rows)) {
        (Some((c, first)), r) if r.is_none_or(|r| c >= r.0) => Some(if first { "right" } else { "left" }.into()),
        (_, Some((_, first))) => Some(if first { "down" } else { "up" }.into()),
        _ => None,
    }
}

fn classify_cut(f: &[FrameStat], m: &[Motion], i: usize, fps: f64, sw: usize) -> (String, Option<String>) {
    let n = f.len();
    let r = (fps * 0.1).round().max(2.0) as usize;
    let lo = i.saturating_sub(r).max(1);
    let hi = (i + r).min(n - 1);
    // Fast global motion with soft frames right at the cut = a whip.
    let near: Vec<usize> = (lo..=hi).filter(|&q| q != i).collect();
    let speed = near.iter().map(|&q| m[q].tx.hypot(m[q].ty)).fold(0.0, f32::max);
    let soft = near.iter().map(|&q| f[q].sharp).fold(f32::MAX, f32::min);
    let wide_lo = i.saturating_sub((fps * 0.6) as usize);
    let wide_hi = (i + (fps * 0.6) as usize).min(n);
    let typical = median(&(wide_lo..wide_hi).map(|q| f[q].sharp).collect::<Vec<_>>());
    if speed > sw as f32 * 0.04 && soft < typical * 0.55 {
        return ("whip".into(), Some(dir_of(m, lo, hi)));
    }
    let zoom = near.iter().map(|&q| m[q].a.abs()).fold(0.0, f32::max);
    if zoom > 0.05 {
        return ("zoom".into(), None);
    }
    ("cut".into(), None)
}

#[allow(clippy::too_many_arguments)]
fn analyse_shot(index: usize, start: f64, end: f64, pad_a: f64, pad_b: f64, transition_in: String, f: &[FrameStat], m: &[Motion], keys: &[KeyFrame], fps: f64, sw: usize, sh: usize) -> Shot {
    let n = f.len();
    let fa = (((start + pad_a) * fps).round() as usize).min(n - 1);
    let fb = (((end - pad_b) * fps).round() as usize).clamp(fa + 1, n);
    let range = fa..fb;
    let len = range.len();
    // Palette: k-means in OKLab over up to 6 frames.
    let mut samples = Vec::new();
    let step = (len / 6).max(1);
    for q in range.clone().step_by(step).take(6) {
        let px = &f[q].rgb;
        for p in (0..px.len() / 3).step_by(3) {
            samples.push(srgb_to_oklab([px[p * 3], px[p * 3 + 1], px[p * 3 + 2]]));
        }
    }
    let palette = kmeans_palette(&samples, 5);
    let (mut ls, mut l2, mut cs) = (0.0f64, 0.0f64, 0.0f64);
    for s in &samples {
        ls += s[0] as f64;
        l2 += (s[0] * s[0]) as f64;
        cs += s[1].hypot(s[2]) as f64;
    }
    let ns = samples.len().max(1) as f64;
    let luma = (ls / ns) as f32;
    let contrast = ((l2 / ns - (ls / ns).powi(2)).max(0.0).sqrt()) as f32;
    let saturation = (cs / ns) as f32;
    let key = if luma < 0.38 { "low-key" } else if luma > 0.72 { "high-key" } else { "mid-key" }.to_string();
    let grade = grade_of(&samples, saturation);

    // Camera: integrate the similarity model over the shot (skip the first frame: it straddles the cut).
    let (mut px, mut py, mut lz, mut rot) = (0f32, 0f32, 0f32, 0f32);
    let mut txs = Vec::new();
    for q in range.clone().skip(1) {
        px += m[q].tx;
        py += m[q].ty;
        lz += (1.0 + m[q].a).max(0.5).ln();
        rot += m[q].th;
        txs.push((m[q].tx, m[q].ty));
    }
    let shake = if txs.len() > 3 {
        let mx = txs.iter().map(|v| v.0).sum::<f32>() / txs.len() as f32;
        let my = txs.iter().map(|v| v.1).sum::<f32>() / txs.len() as f32;
        // Jitter: frame-to-frame change of the velocity.
        let j = txs.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum::<f32>() / (txs.len() - 1) as f32;
        let _ = (mx, my);
        j / sw as f32 * 100.0
    } else {
        0.0
    };
    let camera = camera_label(px / sw as f32, py / sh as f32, lz.exp(), rot.to_degrees(), shake, (len as f64 / fps) as f32);
    // Motion energy: 4 per second, in % of width per frame.
    let per = (fps / 4.0).round().max(1.0) as usize;
    let motion: Vec<f32> = range.clone().collect::<Vec<_>>().chunks(per).map(|c| r2(c.iter().map(|&q| m[q].mag).sum::<f32>() / c.len() as f32 / sw as f32 * 100.0)).collect();
    let motion_mean = r2(motion.iter().sum::<f32>() / motion.len().max(1) as f32);
    let motion_character = motion_character(&motion);

    // Vignette: corners vs centre over the shot's frames.
    let mut vig = Vec::new();
    for q in range.clone().step_by(step) {
        vig.push(vignette(&f[q].l, sw, sh));
    }
    let vignette = r2(median(&vig));

    // Key frame: the middle of the shot.
    let key_time = (start + end) / 2.0;
    let k = nearest_key(keys, key_time);
    let kl = luma_plane(&k.rgb, k.w, k.h);
    let depth = depth_cues(&kl, k.w, k.h);
    let grain = r3(grain(&kl, k.w, k.h));
    let glow = r2(glow(&kl, k.w, k.h));
    let points = count_points(&kl, k.w, k.h);
    let text = text_regions(&kl, k.w, k.h);
    let layout = layout_of(&kl, k.w, k.h);
    let text_coverage = r2(text.iter().map(|t| t.bbox[2] * t.bbox[3]).sum::<f32>().min(1.0));
    let (busy, hard) = flatness(&kl, k.w, k.h);
    let look_score = r2(1.0 - (hard / 0.15).min(1.0));
    let look = if busy < 0.04 {
        "minimal"
    } else if hard >= 0.08 && busy < 0.45 {
        "flat-2d"
    } else if grain > 0.012 {
        "photographic"
    } else {
        "rendered-3d"
    }
    .to_string();
    let mut technique = Vec::new();
    // Type on a plain ground (a title card) or large type over anything.
    if text_coverage > 0.012 && (busy < 0.25 || text.iter().any(|t| t.height > 0.07)) {
        technique.push("kinetic-type".to_string());
    }
    if points > 120 {
        technique.push("particles".into());
    }
    match look.as_str() {
        "rendered-3d" => {
            if saturation > 0.09 && depth.dof != "shallow" && text_coverage < 0.05 {
                technique.push("shader".into());
            }
            technique.push("scene3d".into());
        }
        "photographic" => technique.push("footage-or-scene3d".into()),
        "minimal" if hard < 0.1 && luma < 0.3 => technique.push("scene3d".into()),
        _ => technique.push("2d-motion-graphics".into()),
    }
    if technique.is_empty() {
        technique.push("2d-motion-graphics".into());
    }
    Shot {
        index,
        start: round3(start),
        end: round3(end),
        duration: round3(end - start),
        transition_in,
        palette,
        luma: r2(luma),
        contrast: r2(contrast),
        saturation: r3(saturation),
        key,
        camera,
        motion,
        motion_mean,
        depth,
        look,
        look_score,
        text,
        text_coverage,
        grain,
        vignette,
        glow,
        points,
        technique,
        key_time: round3(k.t),
        grade,
        layout,
        motion_character,
    }
}

fn grade_of(lab: &[[f32; 3]], sat: f32) -> Grade {
    if lab.is_empty() {
        return Grade::default();
    }
    let mut l: Vec<f32> = lab.iter().map(|c| c[0]).collect();
    l.sort_by(f32::total_cmp);
    let p = |q: f32| l[((l.len() - 1) as f32 * q) as usize];
    let (black, mid, white) = (p(0.02), p(0.5), p(0.98));
    let mids: Vec<&[f32; 3]> = lab.iter().filter(|c| c[0] > 0.25 && c[0] < 0.8).collect();
    let n = mids.len().max(1) as f32;
    let warmth = mids.iter().map(|c| c[2]).sum::<f32>() / n;
    let tint = mids.iter().map(|c| c[1]).sum::<f32>() / n;
    let mut parts = Vec::new();
    parts.push(if black > 0.18 { "lifted (milky) blacks" } else if black < 0.04 { "crushed blacks" } else { "clean blacks" });
    parts.push(if white < 0.8 { "soft, rolled-off highlights" } else if white > 0.97 { "bright, clipped highlights" } else { "clean highlights" });
    parts.push(if white - black < 0.5 { "low contrast" } else if white - black > 0.85 { "high contrast" } else { "medium contrast" });
    parts.push(if warmth > 0.015 { "warm" } else if warmth < -0.015 { "cool" } else { "neutral" });
    if tint > 0.012 {
        parts.push("magenta tint");
    } else if tint < -0.012 {
        parts.push("green tint");
    }
    parts.push(if sat < 0.03 { "near monochrome" } else if sat < 0.07 { "desaturated" } else if sat > 0.13 { "saturated" } else { "natural saturation" });
    Grade { black: r2(black), mid: r2(mid), white: r2(white), warmth: r3(warmth), tint: r3(tint), label: parts.join(", ") }
}

fn layout_of(l: &[f32], w: usize, h: usize) -> Layout {
    let mean = l.iter().sum::<f32>() / l.len().max(1) as f32;
    let (mut sym, mut dev) = (0.0f32, 0.0f32);
    let (mut wx, mut wy, mut ws) = (0.0f32, 0.0f32, 0.0f32);
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let v = l[y * w + x];
            sym += (v - l[y * w + (w - 1 - x)]).abs();
            let d = (v - mean).abs();
            dev += d;
            wx += d * x as f32;
            wy += d * y as f32;
            ws += d;
        }
    }
    let symmetry = if dev > 1e-3 { (1.0 - sym / (2.0 * dev)).clamp(0.0, 1.0) } else { 1.0 };
    let (bx, by) = if ws > 1e-3 { (wx / ws / w as f32, wy / ws / h as f32) } else { (0.5, 0.5) };
    let label = if (bx - 0.5).abs() < 0.06 && (by - 0.5).abs() < 0.08 {
        "centred"
    } else if (bx - 0.5).abs() >= (by - 0.5).abs() {
        if bx < 0.5 { "left-weighted" } else { "right-weighted" }
    } else if by < 0.5 {
        "top-heavy"
    } else {
        "bottom-heavy"
    };
    Layout { symmetry: r2(symmetry), weight_x: r2(bx), weight_y: r2(by), label: label.into() }
}

fn motion_character(m: &[f32]) -> String {
    let n = m.len();
    let mean = m.iter().sum::<f32>() / n.max(1) as f32;
    if n < 2 || mean < 0.15 {
        return "still".into();
    }
    let peak = m.iter().cloned().fold(0.0, f32::max);
    let third = (n / 3).max(1);
    let a = m[..third].iter().sum::<f32>() / third as f32;
    let c = m[n - third..].iter().sum::<f32>() / third as f32;
    if peak > mean * 2.5 && n >= 4 {
        "bursts".into()
    } else if a > c * 1.6 {
        "snap-then-settle".into()
    } else if c > a * 1.6 {
        "builds".into()
    } else {
        "steady".into()
    }
}

fn r2(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}
fn r3(x: f32) -> f32 {
    (x * 1000.0).round() / 1000.0
}

fn camera_label(px: f32, py: f32, zoom: f32, rot: f32, shake: f32, secs: f32) -> Camera {
    let mut parts = Vec::new();
    if zoom > 1.04 {
        parts.push("push in");
    } else if zoom < 0.96 {
        parts.push("pull out");
    }
    // Content moving left = camera panning right.
    if px.abs() > 0.06 && px.abs() >= py.abs() {
        parts.push(if px < 0.0 { "pan right" } else { "pan left" });
    } else if py.abs() > 0.06 {
        parts.push(if py < 0.0 { "tilt down" } else { "tilt up" });
    }
    if rot.abs() > 8.0 {
        parts.push("roll");
    }
    if shake > 0.35 {
        parts.push("handheld/shake");
    }
    let _ = secs;
    Camera { pan_x: r3(px), pan_y: r3(py), zoom: r3(zoom), rotate: r2(rot), shake: r2(shake), label: if parts.is_empty() { "static".into() } else { parts.join(" + ") } }
}

fn decode_audio(args: &[String]) -> Result<Option<crate::audio::Pcm>, String> {
    let out = Command::new("ffmpeg").args(["-v", "error"]).args(args).args(["-vn", "-ac", "2", "-ar", "44100", "-f", "f32le", "-"]).output().map_err(|e| e.to_string())?;
    if !out.status.success() || out.stdout.len() < 8 {
        return Ok(None);
    }
    let samples: Vec<f32> = out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    Ok(Some(crate::audio::Pcm { rate: 44100, channels: 2, samples }))
}

fn audio_info(pcm: &crate::audio::Pcm, cuts: &[Transition], fps: f64) -> AudioInfo {
    let b = crate::eval::beat_markers::refine(&crate::audio::analyze_pcm(pcm));
    let per = (0.25 / b.hop).round().max(1.0) as usize;
    let loudness: Vec<f32> = b.energy.chunks(per).map(|c| r2(c.iter().copied().fold(0.0, f32::max))).collect();
    // Hits: onsets with the largest loudness jump.
    let at = |t: f64| b.energy.get((t / b.hop) as usize).copied().unwrap_or(0.0);
    let mut jumps: Vec<(f32, f64)> = b.onsets.iter().map(|&t| (at(t + 0.05) - at((t - 0.05).max(0.0)), t)).collect();
    jumps.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut hits: Vec<f64> = jumps.iter().take((jumps.len() / 10).clamp(1, 24)).filter(|j| j.0 > 0.1).map(|j| round3(j.1)).collect();
    hits.sort_by(f64::total_cmp);
    let tol = (2.0 / fps).max(0.07);
    let hard: Vec<f64> = cuts.iter().map(|c| c.at).collect();
    let on = |grid: &[f64]| {
        if hard.is_empty() || grid.is_empty() {
            return 0.0;
        }
        r2(hard.iter().filter(|c| grid.iter().any(|g| (*g - **c).abs() <= tol)).count() as f32 / hard.len() as f32)
    };
    AudioInfo {
        bpm: (b.bpm * 10.0).round() / 10.0,
        beats: b.beats.iter().map(|x| round3(*x)).collect(),
        downbeats: b.downbeats.iter().map(|x| round3(*x)).collect(),
        onsets: b.onsets.iter().map(|x| round3(*x)).collect(),
        hits,
        loudness,
        cuts_on_beat: on(&b.beats),
        cuts_on_downbeat: on(&b.downbeats),
    }
}

fn aspect_name(w: u32, h: u32) -> String {
    let r = w as f64 / h as f64;
    for (n, v) in [("16:9", 16.0 / 9.0), ("9:16", 9.0 / 16.0), ("1:1", 1.0), ("4:5", 0.8), ("4:3", 4.0 / 3.0), ("21:9", 2.37), ("2.39:1", 2.39)] {
        if (r - v).abs() / v < 0.02 {
            return n.into();
        }
    }
    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    let g = gcd(w, h).max(1);
    format!("{}:{}", w / g, h / g)
}

fn summarise(p: &Probe, fps: f64, duration: f64, shots: &[Shot], tr: &[Transition], audio: &Option<AudioInfo>) -> Global {
    let lens: Vec<f32> = shots.iter().map(|s| s.duration as f32).collect();
    let med = median(&lens) as f64;
    let longest = lens.iter().copied().fold(0.0, f32::max) as f64;
    let shortest = lens.iter().copied().fold(f32::MAX, f32::min) as f64;
    let mean = lens.iter().sum::<f32>() as f64 / lens.len().max(1) as f64;
    let pace = if longest <= 1.3 && med <= 0.8 {
        "frantic"
    } else if med <= 1.6 && longest <= 3.5 {
        "fast"
    } else if med <= 3.8 {
        "steady"
    } else {
        "calm"
    }
    .to_string();
    // Global palette: shot swatches weighted by shot length, merged.
    let mut samples = Vec::new();
    for s in shots {
        for sw in &s.palette {
            let lab = hex_to_oklab(&sw.hex);
            let reps = ((sw.share * s.duration as f32 * 40.0).round() as usize).max(1);
            samples.extend(std::iter::repeat_n(lab, reps));
        }
    }
    let palette = kmeans_palette(&samples, 6);
    let mut mix: Vec<(String, usize)> = Vec::new();
    for t in tr {
        match mix.iter_mut().find(|m| m.0 == t.kind) {
            Some(m) => m.1 += 1,
            None => mix.push((t.kind.clone(), 1)),
        }
    }
    mix.sort_by_key(|m| std::cmp::Reverse(m.1));
    let mut tech: Vec<(String, usize)> = Vec::new();
    for s in shots {
        let t = &s.technique[0];
        match tech.iter_mut().find(|m| &m.0 == t) {
            Some(m) => m.1 += 1,
            None => tech.push((t.clone(), 1)),
        }
    }
    tech.sort_by_key(|m| std::cmp::Reverse(m.1));
    let wmean = |f: &dyn Fn(&Shot) -> f32| shots.iter().map(|s| f(s) * s.duration as f32).sum::<f32>() / duration.max(1e-6) as f32;
    let mean_luma = r2(wmean(&|s| s.luma));
    let grain = r3(median(&shots.iter().map(|s| s.grain).collect::<Vec<_>>()));
    let vignette = r2(median(&shots.iter().map(|s| s.vignette).collect::<Vec<_>>()));
    let glow = r2(median(&shots.iter().map(|s| s.glow).collect::<Vec<_>>()));
    let moving = shots.iter().filter(|s| s.camera.label != "static").count();
    let text_shots = shots.iter().filter(|s| s.text_coverage > 0.02).count();
    let mut sum = format!(
        "{} {} clip, {:.1} s, {} shot{} (median {:.2} s, longest {:.2} s): {} pace. ",
        aspect_name(p.w, p.h),
        if mean_luma < 0.4 { "dark" } else if mean_luma > 0.7 { "bright" } else { "mid-tone" },
        duration,
        shots.len(),
        if shots.len() == 1 { "" } else { "s" },
        med,
        longest,
        pace
    );
    if !mix.is_empty() {
        sum += &format!("Transitions: {}. ", mix.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", "));
    }
    sum += &format!("Palette led by {}. ", palette.iter().take(3).map(|s| s.hex.clone()).collect::<Vec<_>>().join(", "));
    sum += &format!("Camera moves in {}/{} shots; text in {}/{}. ", moving, shots.len(), text_shots, shots.len());
    sum += &format!("Looks like: {}. ", tech.iter().map(|(k, n)| format!("{k} ×{n}")).collect::<Vec<_>>().join(", "));
    let mut fx = Vec::new();
    if grain > 0.012 {
        fx.push("film grain");
    }
    if vignette > 0.3 {
        fx.push("vignette");
    }
    if glow > 0.15 {
        fx.push("bloom/glow");
    }
    if !fx.is_empty() {
        sum += &format!("Finish: {}. ", fx.join(", "));
    }
    if let Some(a) = audio {
        if a.bpm > 0.0 {
            sum += &format!("Music ≈ {:.0} BPM; {:.0}% of cuts on a beat.", a.bpm, a.cuts_on_beat * 100.0);
        }
    }
    Global {
        width: p.w,
        height: p.h,
        aspect: aspect_name(p.w, p.h),
        fps: (p.fps * 1000.0).round() / 1000.0,
        analysed_fps: fps,
        duration: round3(duration),
        shot_count: shots.len(),
        median_shot: round3(med),
        mean_shot: round3(mean),
        longest_shot: round3(longest),
        shortest_shot: round3(shortest),
        cuts_per_minute: ((tr.len() as f64 / duration.max(1e-6) * 60.0) * 10.0).round() / 10.0,
        pace,
        palette,
        transition_mix: mix,
        technique_mix: tech,
        mean_luma,
        grain,
        vignette,
        glow,
        style_summary: sum.trim().to_string(),
    }
}

#[cfg(test)]
mod tests;
