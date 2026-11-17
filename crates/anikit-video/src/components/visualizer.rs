//! Audio visualiser synced to a music `src`: spectrum bars, a mirrored wave
//! or radial bars around a ring (with an optional image in the middle).
//!
//! The audio is decoded once in `prepare`, analysed with an FFT at 60 Hz into
//! a compact `u8` table of band levels (already smoothed and with peak-hold
//! precomputed), so `eval` only interpolates two rows of the table. Without a
//! readable `src` the component analyses a deterministic synthetic groove
//! (kick, snare, hats, bass and a pad at 120 BPM, looping every 8 s), so it
//! still looks alive in previews.

use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::audio::Pcm;
use crate::dl::{Op, Paint, Rgba, IDENTITY};
use serde::Deserialize;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

/// Analysis rate (table rows per second).
const RATE: f32 = 60.0;
/// Analysis sample rate (plenty for a visualiser, and cheap to FFT).
const SR: u32 = 22_050;
const N: usize = 2048;

/// Precomputed band levels.
struct Table {
    bands: usize,
    frames: usize,
    /// `frames × bands` smoothed levels, 0..255.
    level: Vec<u8>,
    /// `frames × bands` peak-hold levels, 0..255.
    peak: Vec<u8>,
    /// Beat times (s).
    beats: Vec<f32>,
    /// Loops (synthetic signal) instead of falling silent at the end.
    looped: bool,
}

impl Table {
    /// Interpolated level of band `b` at time `t` (s); `peak` selects the peak table.
    #[inline]
    fn at(&self, data: &[u8], b: usize, t: f32) -> f32 {
        if self.frames == 0 {
            return 0.0;
        }
        let mut f = t.max(0.0) * RATE;
        let len = self.frames as f32;
        if self.looped {
            f %= len;
        } else if f >= len - 1.0 {
            return 0.0;
        }
        let i = f as usize;
        let j = if i + 1 >= self.frames { 0 } else { i + 1 };
        let k = f - i as f32;
        let a = data[i * self.bands + b] as f32;
        let c = data[j * self.bands + b] as f32;
        (a + (c - a) * k) * (1.0 / 255.0)
    }

    /// Beat pulse 0..1 (sharp attack, ~0.25 s decay).
    fn pulse(&self, t: f32) -> f32 {
        if self.beats.is_empty() {
            return 0.0;
        }
        let t = if self.looped { t.rem_euclid(self.frames as f32 / RATE) } else { t };
        let i = self.beats.partition_point(|&b| b <= t);
        if i == 0 {
            return 0.0;
        }
        let a = t - self.beats[i - 1];
        (-a * 9.0).exp() * clamp01(a * 40.0)
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Visualizer {
    /// Audio file (mp3, wav, ogg, flac, m4a).
    pub src: Option<String>,
    /// `bars`, `wave` or `circle`.
    pub style: String,
    /// Number of bands (bars); default 48 for bars, 96 for circle, 32 for wave.
    pub bands: Option<u32>,
    /// Gradient colours (low → high); default: a ramp of the theme accent.
    pub colors: Vec<Color>,
    /// Release smoothing 0..1 (higher = slower fall).
    pub smoothing: f32,
    /// Peak-hold caps (bars style, not mirrored).
    pub peaks: bool,
    /// Pulse on detected beats.
    pub beats: bool,
    /// Level multiplier.
    pub gain: f32,
    /// Bars style: mirror about the horizontal centre line.
    pub mirror: bool,
    /// Bars style: faded reflection below (0 = none).
    pub reflection: f32,
    /// Gap between bars as a fraction of the slot.
    pub gap: f32,
    /// Circle style: ring radius as a fraction of the half-size of the box.
    pub radius: f32,
    /// Circle style: image in the middle (avatar or logo).
    pub image: Option<String>,
    /// Seconds into the audio at local time 0.
    pub offset: f32,
    pub delay: f32,
    /// Line width (wave style).
    pub line: f32,
    #[serde(skip)]
    table: Option<Arc<Table>>,
}

impl Default for Visualizer {
    fn default() -> Self {
        Visualizer {
            src: None,
            style: "bars".into(),
            bands: None,
            colors: Vec::new(),
            smoothing: 0.6,
            peaks: true,
            beats: false,
            gain: 1.0,
            mirror: false,
            reflection: 0.25,
            gap: 0.38,
            radius: 0.42,
            image: None,
            offset: 0.0,
            delay: 0.0,
            line: 3.0,
            table: None,
        }
    }
}

impl Visualizer {
    fn table(&self) -> &Table {
        self.table.as_deref().expect("prepared")
    }
    fn n(&self) -> usize {
        self.table().bands
    }
    fn colors(&self, ctx: &Ctx) -> Vec<Rgba> {
        if self.colors.is_empty() {
            vec![shade(ctx.theme.accent, -0.25), ctx.theme.accent, shade(ctx.theme.accent, 0.5)]
        } else {
            self.colors.iter().map(|c| c.0).collect()
        }
    }
}

/// Colour at `x` (0..1) along a gradient of evenly spaced stops.
fn ramp(cols: &[Rgba], x: f32) -> Rgba {
    if cols.len() == 1 {
        return cols[0];
    }
    let f = clamp01(x) * (cols.len() - 1) as f32;
    let i = (f as usize).min(cols.len() - 2);
    mix(cols[i], cols[i + 1], f - i as f32)
}

fn stops(cols: &[Rgba], a: f32) -> Vec<(f32, Rgba)> {
    let n = (cols.len() - 1).max(1) as f32;
    cols.iter().enumerate().map(|(i, c)| (i as f32 / n, alpha(*c, a))).collect()
}

impl Prepare for Visualizer {
    fn prepare(&mut self) -> Result<(), Error> {
        let default_bands = match self.style.as_str() {
            "bars" => 48,
            "wave" => 32,
            "circle" => 96,
            s => {
                let hint = closest(s, &["bars", "wave", "circle"]).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(Error(format!("style must be bars, wave or circle, not {s:?}{hint}")));
            }
        };
        let bands = self.bands.unwrap_or(default_bands).clamp(4, 256) as usize;
        if !(self.gain.is_finite() && self.offset.is_finite() && self.delay.is_finite()) {
            return Err(Error("`gain`, `offset` and `delay` must be finite".into()));
        }
        // Several visualizers usually share one track: decode and analyse it
        // once per (file, bands, smoothing) for the whole process.
        // Keyed by the file's size and modification time too, so a rewritten
        // file (MCP server, --watch) is analysed again.
        type Key = (Option<String>, u64, u128, usize, u32);
        static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<Key, Arc<Table>>>> = std::sync::OnceLock::new();
        let smoothing = clamp01(self.smoothing);
        let (len, mtime) = self
            .src
            .as_deref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos())))
            .unwrap_or((0, 0));
        let key: Key = (self.src.clone(), len, mtime, bands, smoothing.to_bits());
        let cache = CACHE.get_or_init(Default::default);
        if let Some(t) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            self.table = Some(t.clone());
            return Ok(());
        }
        let decoded = self.src.as_deref().and_then(|s| crate::audio::decode_file_at(s, SR).ok()).filter(|p| p.frames() > N);
        let (pcm, looped) = match decoded {
            Some(p) => (p, false),
            None => (synth(), true),
        };
        let t = Arc::new(analyse(&pcm, bands, smoothing, looped));
        let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
        // Bounded: a long-lived process may see many tracks.
        if c.len() > 32 {
            c.clear();
        }
        c.insert(key, t.clone());
        self.table = Some(t);
        Ok(())
    }
}

/// Band edges: log-spaced from 40 Hz to 14 kHz, at least one FFT bin wide.
fn band_bins(bands: usize) -> Vec<(usize, usize)> {
    let (lo, hi) = (40.0f32, 14_000.0f32);
    let bin = SR as f32 / N as f32;
    let mut out = Vec::with_capacity(bands);
    let mut prev = (lo / bin) as usize;
    for b in 0..bands {
        let f1 = lo * (hi / lo).powf((b + 1) as f32 / bands as f32);
        let e = ((f1 / bin) as usize).max(prev + 1).min(N / 2);
        out.push((prev.min(e - 1), e));
        prev = e;
    }
    out
}

fn analyse(pcm: &Pcm, bands: usize, smoothing: f32, looped: bool) -> Table {
    let mono = pcm.mono();
    let hop = SR as f32 / RATE;
    let frames = ((mono.len() as f32) / hop) as usize;
    let win: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (TAU * i as f32 / N as f32).cos()).collect();
    let edges = band_bins(bands);
    let mut db = vec![0f32; frames * bands];
    let (mut re, mut im) = (vec![0f32; N], vec![0f32; N]);
    for f in 0..frames {
        let c = (f as f32 * hop) as isize - (N / 2) as isize;
        for i in 0..N {
            let k = c + i as isize;
            let s = if k >= 0 && (k as usize) < mono.len() {
                mono[k as usize]
            } else if looped {
                mono[k.rem_euclid(mono.len() as isize) as usize]
            } else {
                0.0
            };
            re[i] = s * win[i];
            im[i] = 0.0;
        }
        anikit_core::audio::fft(&mut re, &mut im);
        for (b, &(s, e)) in edges.iter().enumerate() {
            let mut acc = 0.0;
            for k in s..e {
                acc += re[k] * re[k] + im[k] * im[k];
            }
            // Mean power, tilted +3 dB/oct so treble reads as well as bass.
            let p = acc / (e - s) as f32 * (1.0 + b as f32 / bands as f32 * 3.0);
            db[f * bands + b] = 10.0 * (p + 1e-10).log10();
        }
    }
    // Normalise: a ceiling per band (its 97th percentile, blended with the
    // global one) maps to 1, 42 dB below it maps to 0.
    let pct = |v: &mut Vec<f32>| -> f32 {
        if v.is_empty() {
            return 0.0;
        }
        let k = ((v.len() - 1) as f32 * 0.97) as usize;
        *v.select_nth_unstable_by(k, f32::total_cmp).1
    };
    let global = pct(&mut db.clone());
    let ceil: Vec<f32> = (0..bands)
        .map(|b| {
            let mut col: Vec<f32> = (0..frames).map(|f| db[f * bands + b]).collect();
            0.5 * pct(&mut col) + 0.5 * global
        })
        .collect();
    let range = 42.0;
    // Fast attack, slow release; peaks hold 0.4 s then fall with gravity.
    let release = lerp(0.55, 0.92, smoothing);
    let mut level = vec![0u8; frames * bands];
    let mut peak = vec![0u8; frames * bands];
    let mut cur = vec![0f32; bands];
    let mut pk = vec![(0f32, 0f32); bands]; // (value, seconds since set)
    // Looped tables run twice so the start continues smoothly from the end.
    let passes = if looped { 2 } else { 1 };
    for pass in 0..passes {
        for f in 0..frames {
            for b in 0..bands {
                let raw = clamp01((db[f * bands + b] - (ceil[b] - range)) / range);
                let raw = raw * raw * (3.0 - 2.0 * raw); // a little contrast
                let v = if raw > cur[b] { lerp(cur[b], raw, 0.75) } else { cur[b] * release + raw * (1.0 - release) };
                cur[b] = v;
                let (pv, age) = pk[b];
                let age = age + 1.0 / RATE;
                let held = pv - if age > 0.4 { 1.6 * (age - 0.4) * (age - 0.4) } else { 0.0 };
                pk[b] = if v >= held { (v, 0.0) } else { (pv, age) };
                let shown = held.max(v);
                if pass + 1 == passes {
                    level[f * bands + b] = (v * 255.0).round() as u8;
                    peak[f * bands + b] = (shown * 255.0).round() as u8;
                }
            }
        }
    }
    let beats = if looped {
        (0..16).map(|i| i as f32 * 0.5).collect()
    } else {
        crate::audio::analyze_pcm(pcm).beats.iter().map(|&b| b as f32).collect()
    };
    Table { bands, frames, level, peak, beats, looped }
}

/// A deterministic 8 s groove at 120 BPM: kick, snare, hats, bass and a pad.
fn synth() -> Pcm {
    let sr = SR as f32;
    let len = (8.0 * sr) as usize;
    let mut out = vec![0f32; len];
    let mut rng = Rng::new(7);
    let noise: Vec<f32> = (0..len).map(|_| rng.f() * 2.0 - 1.0).collect();
    let beat = 0.5;
    let bass = [55.0, 55.0, 65.41, 49.0];
    let chords = [[220.0, 261.63, 329.63], [220.0, 261.63, 329.63], [261.63, 329.63, 392.0], [196.0, 246.94, 293.66]];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let bar = ((t / 2.0) as usize) % 4;
        let tb = t % beat;
        // Kick on every beat (plus a pickup), pitch-swept sine.
        let mut s = (-tb * 18.0).exp() * (TAU * (48.0 * tb + 9.0 * (1.0 - (-tb * 30.0).exp()))).sin() * 0.9;
        let t8 = t % 2.0;
        if (1.75..2.0).contains(&t8) {
            let k = t8 - 1.75;
            s += (-k * 20.0).exp() * (TAU * (48.0 * k + 9.0 * (1.0 - (-k * 30.0).exp()))).sin() * 0.6;
        }
        // Snare on 2 and 4.
        let tbar = t % 1.0;
        if tbar >= 0.5 {
            let k = tbar - 0.5;
            s += (-k * 14.0).exp() * (noise[i] * 0.45 + (TAU * 190.0 * k).sin() * 0.3);
        }
        // Hats on eighths (differenced noise is bright).
        let k = t % 0.25;
        let hp = noise[i] - noise[i.saturating_sub(1)];
        s += (-k * 45.0).exp() * hp * if (t % 0.5) < 0.25 { 0.12 } else { 0.2 };
        // Bass (rounded saw) and pad (sines with a slow swell).
        let f = bass[bar] * if (t % 1.0) < 0.75 { 1.0 } else { 2.0 };
        let ph = (t * f).fract();
        s += (ph * 2.0 - 1.0) * 0.18 * (1.0 - (-((t % 0.25) * 30.0)).exp()) * (-(t % 0.25) * 4.0).exp();
        let swell = 0.5 + 0.5 * (TAU * t / 2.0 - PI * 0.5).sin();
        for (j, &cf) in chords[bar].iter().enumerate() {
            s += (TAU * cf * t).sin() * 0.05 * swell + (TAU * cf * 2.0 * t + j as f32).sin() * 0.02;
        }
        // A sparkly arpeggio high up.
        let step = ((t * 8.0) as usize) % 8;
        let af = chords[bar][step % 3] * 4.0;
        s += (TAU * af * t).sin() * 0.04 * (-(t % 0.125) * 20.0).exp();
        *o = s * 0.6;
    }
    Pcm::from_mono(SR, &out)
}

impl Component for Visualizer {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let at = t - self.delay;
        if at < 0.0 {
            return Vec::new();
        }
        let grow = ease_out(seg(at, 0.0, 0.7));
        let ta = at + self.offset;
        let tb = self.table();
        let pulse = if self.beats { tb.pulse(ta) } else { 0.0 };
        let k = grow * self.gain * (1.0 + 0.18 * pulse);
        match self.style.as_str() {
            "wave" => self.wave(ta, k, pulse, ctx),
            "circle" => self.circle(ta, k, pulse, grow, ctx),
            _ => self.bars(ta, k, pulse, ctx),
        }
    }

    /// The audio's length (none for the looping synthetic signal).
    fn duration(&self) -> Option<f32> {
        let tb = self.table();
        (!tb.looped).then(|| (self.delay + tb.frames as f32 / RATE - self.offset).max(0.0))
    }
}

impl Visualizer {
    fn bars(&self, ta: f32, k: f32, pulse: f32, ctx: &Ctx) -> Vec<Op> {
        let tb = self.table();
        let n = self.n();
        let (w, h) = (ctx.width, ctx.height);
        let cols = self.colors(ctx);
        let slot = w / n as f32;
        let bw = (slot * (1.0 - clamp01(self.gap))).max(1.0);
        let r = bw * 0.5;
        let refl = if self.mirror { 0.0 } else { clamp01(self.reflection) };
        // Baseline and the height available above it.
        let head = if self.peaks && !self.mirror { (bw * 0.42).clamp(2.0, 8.0) * 2.4 } else { 0.0 };
        let (base, avail) = if self.mirror {
            (h * 0.5, h * 0.5)
        } else {
            let b = h / (1.0 + refl * 0.6);
            (b, b - head)
        };
        let top = base - avail;
        // One gradient for every bar, so tall bars reach the hot colour.
        let grad = |a: f32| {
            if self.mirror {
                let mut c: Vec<Rgba> = cols.iter().rev().copied().collect();
                c.extend(cols.iter().skip(1));
                Paint::Linear { p0: [0.0, top], p1: [0.0, h], stops: stops(&c, a) }
            } else {
                Paint::Linear { p0: [0.0, base], p1: [0.0, top], stops: stops(&cols, a) }
            }
        };
        let mut o = Vec::with_capacity(n * 3 + 4);
        if pulse > 0.01 {
            o.push(rect(0.0, top, w, avail * 2.0, 0.0, radial(w * 0.5, base, w * 0.6, vec![(0.0, alpha(cols[0], 0.18 * pulse)), (1.0, alpha(cols[0], 0.0))])));
        }
        let hs: Vec<f32> = (0..n).map(|b| (tb.at(&tb.level, b, ta) * k).min(1.0) * (avail - bw) + bw).collect();
        if refl > 0.0 {
            let fade = Paint::Linear { p0: [0.0, base], p1: [0.0, base + avail * refl], stops: vec![(0.0, alpha(cols[0], 0.32)), (1.0, alpha(cols[0], 0.0))] };
            for (b, &bh) in hs.iter().enumerate() {
                let x = b as f32 * slot + (slot - bw) * 0.5;
                o.push(rect(x, base + bw * 0.4, bw, (bh * refl).max(bw), r, fade.clone()));
            }
        }
        let paint = grad(1.0);
        for (b, &bh) in hs.iter().enumerate() {
            let x = b as f32 * slot + (slot - bw) * 0.5;
            let y = if self.mirror { base - bh * 0.5 } else { base - bh };
            o.push(rect(x, y, bw, bh, r, paint.clone()));
        }
        if self.peaks && !self.mirror {
            let cap = (bw * 0.42).clamp(2.0, 8.0);
            for b in 0..n {
                let p = (tb.at(&tb.peak, b, ta) * k).min(1.0) * (avail - bw) + bw;
                if p <= hs[b] + cap * 0.5 {
                    continue;
                }
                let x = b as f32 * slot + (slot - bw) * 0.5;
                let c = shade(ramp(&cols, p / avail), 0.35);
                o.push(rect(x, base - p - cap * 1.6, bw, cap, cap * 0.5, solid(alpha(c, 0.9))));
            }
        }
        o
    }

    fn wave(&self, ta: f32, k: f32, pulse: f32, ctx: &Ctx) -> Vec<Op> {
        let tb = self.table();
        let n = self.n();
        let (w, h) = (ctx.width, ctx.height);
        let cy = h * 0.5;
        let cols = self.colors(ctx);
        let mut o = Vec::with_capacity(8);
        // Three layers: a wide soft one behind (lagging), a mid fill, and the crisp line.
        for (layer, (lag, amp, fill_a, line_a)) in [(0.15, 1.2, 0.12, 0.0), (0.06, 1.06, 0.22, 0.3), (0.0, 1.0, 0.0, 1.0)].into_iter().enumerate() {
            // Band levels, blurred across neighbours so the outline stays smooth.
            let raw: Vec<f32> = (0..n).map(|b| tb.at(&tb.level, b, ta - lag)).collect();
            let vals: Vec<f32> = (0..n)
                .map(|b| {
                    // Reflect at band 0: the middle of the wave is mirrored.
                    let g = |d: isize| raw[(b as isize + d).unsigned_abs().min(n - 1)];
                    (g(-3) + 6.0 * g(-2) + 15.0 * g(-1) + 20.0 * g(0) + 15.0 * g(1) + 6.0 * g(2) + g(3)) / 64.0
                })
                .collect();
            let m = 36usize;
            let mut up = Vec::with_capacity(m + 1);
            for i in 0..=m {
                let u = i as f32 / m as f32;
                // Lows in the middle, highs towards both ends.
                let bf = (u * 2.0 - 1.0).abs() * (n - 1) as f32;
                let bi = (bf as usize).min(n - 1);
                let v = lerp(vals[bi], vals[(bi + 1).min(n - 1)], bf.fract());
                let win = (PI * u).sin().max(0.0).powf(1.2);
                let wob = 0.8 + 0.2 * (u * 7.0 + ta * (1.5 + layer as f32 * 0.7) + layer as f32 * 2.0).sin();
                let a = (v * k).min(1.1) * h * 0.44 * amp * win * wob + 1.5;
                up.push((u * w, cy - a));
            }
            let top = smooth_path(&up, 1.0);
            let dn: Vec<(f32, f32)> = up.iter().rev().map(|&(x, y)| (x, 2.0 * cy - y)).collect();
            let bottom = smooth_path(&dn, 1.0);
            let horiz = |a: f32| Paint::Linear { p0: [0.0, cy], p1: [w, cy], stops: stops(&[cols[0], *cols.last().unwrap(), cols[0]], a) };
            if fill_a > 0.0 {
                let mut p = top.clone();
                for el in bottom.elements().iter().skip(1) {
                    p.push(*el);
                }
                p.close_path();
                o.push(fill(p, horiz(fill_a * (1.0 + pulse * 0.6))));
            }
            if line_a > 0.0 {
                let lw = self.line * if layer == 2 { 1.0 } else { 0.6 };
                o.push(Op::Stroke { path: top, paint: horiz(line_a), stroke: line(lw, [0.0, 1.0]), transform: IDENTITY });
                o.push(Op::Stroke { path: bottom, paint: horiz(line_a), stroke: line(lw, [0.0, 1.0]), transform: IDENTITY });
            }
        }
        // Bright core line through the middle.
        o.push(rect(0.0, cy - 0.75, w, 1.5, 0.0, Paint::Linear { p0: [0.0, cy], p1: [w, cy], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.0])), (0.5, Rgba([1.0, 1.0, 1.0, 0.35])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))] }));
        o
    }

    fn circle(&self, ta: f32, k: f32, pulse: f32, grow: f32, ctx: &Ctx) -> Vec<Op> {
        let tb = self.table();
        let n = self.n();
        let (w, h) = (ctx.width, ctx.height);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let half = w.min(h) * 0.5;
        let r0 = half * self.radius.clamp(0.1, 0.9) * (1.0 + 0.03 * pulse);
        let len = (half - r0) * 0.92;
        let cols = self.colors(ctx);
        let circ = TAU * r0;
        let bw = (circ / n as f32 * (1.0 - clamp01(self.gap))).max(1.5);
        let mut o = Vec::with_capacity(n + 8);
        // Soft glow behind the ring follows overall loudness.
        let mut loud = 0.0;
        for b in 0..n.min(8) {
            loud += tb.at(&tb.level, b, ta);
        }
        loud = loud / n.min(8) as f32 * grow;
        o.push(circle(cx, cy, r0 + len, radial(cx, cy, r0 + len, vec![(0.0, alpha(cols[0], 0.0)), (r0 / (r0 + len), alpha(cols[0], 0.25 + 0.25 * loud + 0.2 * pulse)), (1.0, alpha(cols[0], 0.0))])));
        // Radial bars, spectrum mirrored left/right so the ring is symmetric.
        for i in 0..n {
            let u = i as f32 / n as f32;
            let m = 1.0 - (u * 2.0 - 1.0).abs(); // 0 at top, 1 at bottom
            let bf = m * (n - 1) as f32;
            let b = bf as usize;
            let v = lerp(tb.at(&tb.level, b, ta), tb.at(&tb.level, (b + 1).min(n - 1), ta), bf.fract());
            let l = (v * k).min(1.0) * len * 0.9 + bw;
            let ang = u * TAU - PI * 0.5;
            let c = ramp(&cols, (v * k).min(1.0) * 0.6 + m * 0.4);
            let m6 = rotate_about(ang, 0.0, 0.0);
            let tr = crate::dl::mul(&translate(cx, cy), &m6);
            o.push(Op::RoundRect { x: r0 + bw * 0.8, y: -bw * 0.5, w: l, h: bw, radius: bw * 0.5, paint: solid(c), transform: tr });
        }
        // The ring itself.
        o.push(Op::Stroke { path: circle_path(cx, cy, r0), paint: solid(alpha(Rgba([1.0, 1.0, 1.0, 1.0]), 0.18 + 0.4 * pulse)), stroke: line(2.0, [0.0, 1.0]), transform: IDENTITY });
        // Centre: the image (cover-fit in a circle) or a gradient disc.
        let ri = r0 * 0.9;
        if let Some((id, iw, ih)) = self.image.as_deref().and_then(|s| ctx.assets.media(s, ta)) {
            o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: Vec::new(), clip: Some((circle_path(cx, cy, ri), IDENTITY)), cache_key: None });
            let s = (2.0 * ri / iw as f32).max(2.0 * ri / ih as f32);
            let (dw, dh) = (iw as f32 * s, ih as f32 * s);
            o.push(Op::Image { image: id, dst: [cx - dw * 0.5, cy - dh * 0.5, dw, dh], opacity: 1.0, transform: IDENTITY });
            o.push(Op::PopLayer);
        } else {
            o.push(circle(cx, cy, ri, Paint::Linear { p0: [cx - ri, cy - ri], p1: [cx + ri, cy + ri], stops: stops(&[shade(cols[0], -0.2), shade(*cols.last().unwrap(), -0.45)], 1.0) }));
            o.push(circle(cx, cy, ri, radial(cx - ri * 0.3, cy - ri * 0.4, ri * 1.2, vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.22])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))])));
        }
        if grow < 1.0 {
            let mut o2 = o;
            super::transform_ops(&mut o2, &scale_about(lerp(0.85, 1.0, grow), cx, cy));
            return super::with_opacity(o2, grow);
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use serde_json::json;

    fn shot(name: &str, p: serde_json::Value, t: f32, w: f32, h: f32) {
        let th = Theme::default();
        let a = preview::host();
        let c = build("visualizer", &p).unwrap();
        for tt in [0.0, t * 0.5, t, 60.0] {
            preview::validate(&c.eval(tt, w, h, 30.0, &th, a)).unwrap();
        }
        preview::snapshot(name, w as u32, h as u32, th.background, c.eval(t, w, h, 30.0, &th, a), a);
    }

    #[test]
    fn styles() {
        let a = preview::host();
        // A test "logo": radial gradient disc.
        let s = 128u32;
        let mut px = Vec::new();
        for y in 0..s {
            for x in 0..s {
                let (dx, dy) = (x as f32 / s as f32 - 0.5, y as f32 / s as f32 - 0.5);
                let d = (dx * dx + dy * dy).sqrt();
                px.extend_from_slice(&[(255.0 * (1.0 - d)) as u8, (120.0 + 200.0 * dx).clamp(0.0, 255.0) as u8, 200, 255]);
            }
        }
        a.add_media("viz-logo.png", s, s, px);
        shot("visualizer-bars", json!({"beats": true}), 2.3, 960.0, 400.0);
        shot("visualizer-mirror", json!({"mirror": true, "bands": 64}), 3.1, 960.0, 400.0);
        shot("visualizer-wave", json!({"style": "wave"}), 2.6, 960.0, 400.0);
        shot("visualizer-circle", json!({"style": "circle", "image": "viz-logo.png", "beats": true}), 2.02, 600.0, 600.0);
    }

    #[test]
    fn real_audio_and_errors() {
        // Write a click track and check beats + silence after the end.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/components");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("viz-test.wav");
        let sr = 22_050;
        let mono: Vec<f32> = (0..sr * 3).map(|i| {
            let t = i as f32 / sr as f32;
            let k = t % 0.5;
            (-k * 30.0).exp() * (std::f32::consts::TAU * 80.0 * t).sin()
        }).collect();
        crate::audio::write_wav(&path, &crate::audio::Pcm::from_mono(sr as u32, &mono), crate::audio::WavFormat::Pcm16).unwrap();
        let c = build("visualizer", &json!({"src": path.to_str().unwrap(), "beats": true})).unwrap();
        let th = Theme::default();
        let ops = c.eval(1.0, 800.0, 300.0, 30.0, &th, preview::host());
        preview::validate(&ops).unwrap();
        let d = c.component.duration().expect("decoded, so it has a length");
        assert!((d - 3.0).abs() < 0.1, "{d}");
        assert!(build("visualizer", &json!({"src": "/nonexistent.mp3"})).unwrap().component.duration().is_none());
        assert!(build("visualizer", &json!({"style": "barz"})).err().unwrap().0.contains("did you mean `bars`"));
    }
}
