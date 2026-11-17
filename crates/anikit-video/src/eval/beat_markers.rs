//! Beat markers: `"@beat17"`, `"@bar5"`, `"@bar5+0.25"` resolve to the beat
//! grid detected in the spec's music (the first top-level `audio` cue with a
//! `src` that is not a voice-over), so cuts and reveals land on the beat
//! without hand-typed times. Beats and bars count from 1; times are on the
//! global timeline (the cue's `at` and `trimStart` are applied). The audio is
//! decoded and analysed once, and only when such a marker is used.
//!
//! The grid is [`refine`]d first: beats snap to the nearest onset within
//! 60 ms (the tempo estimate drifts a little over a long track), and beats
//! and bars before the first detected one are extrapolated back to 0, so a
//! track that starts on the beat has `@beat1` = `@bar1` ≈ 0.

use crate::audio::{analyze_pcm, decode_file, Beats};
use crate::spec::{Num, Spec};
use std::path::{Path, PathBuf};

pub struct BeatMarkers {
    src: Option<(PathBuf, f64, f64)>,
    beats: Option<Result<Beats, String>>,
    level: Option<Vec<f32>>,
}

/// The music as expressions see it (`beat`, `bar`, `level`, `bpm`), on the global timeline.
pub struct Music {
    beats: Vec<f64>,
    bars: Vec<f64>,
    bpm: f64,
    /// Loudness 0..1 every 1/100 s from global time `start`.
    level: Vec<f32>,
    start: f64,
}

/// Fractional count along a sorted grid: 0 at the first mark, 1 at the second…
/// extrapolated at the ends with the mean spacing.
fn along(grid: &[f64], t: f64) -> f64 {
    match grid.len() {
        0 => 0.0,
        1 => t - grid[0],
        n => {
            let i = grid.partition_point(|&g| g <= t);
            if i == 0 || i == n {
                let step = (grid[n - 1] - grid[0]) / (n - 1) as f64;
                let (k, g) = if i == 0 { (0, grid[0]) } else { (n - 1, grid[n - 1]) };
                k as f64 + (t - g) / step.max(1e-6)
            } else {
                (i - 1) as f64 + (t - grid[i - 1]) / (grid[i] - grid[i - 1]).max(1e-6)
            }
        }
    }
}

impl Music {
    /// (beat, bar, level, bpm) at global time `t`.
    pub fn at(&self, t: f64) -> (f64, f64, f64, f64) {
        let k = ((t - self.start) * 100.0).floor();
        let level = if k < 0.0 { 0.0 } else { self.level.get(k as usize).copied().unwrap_or(0.0) as f64 };
        (along(&self.beats, t), along(&self.bars, t), level, self.bpm)
    }
}

fn secs(n: &Option<Num>) -> f64 {
    match n {
        Some(Num::Num(v)) => *v,
        Some(Num::Str(s)) => s.trim().trim_end_matches('s').parse().unwrap_or(0.0),
        None => 0.0,
    }
}

/// `beat12` → ("beat", 12); `bar3` → ("bar", 3).
fn parse(name: &str) -> Option<(&'static str, usize)> {
    for kind in ["beat", "bar"] {
        if let Some(n) = name.strip_prefix(kind).and_then(|r| r.parse::<usize>().ok()) {
            return Some((kind, n));
        }
    }
    None
}

impl BeatMarkers {
    pub fn new(spec: &Spec, base: &Path) -> Self {
        let src = spec.audio.iter().flatten().find(|c| c.src.is_some() && c.voice != Some(true)).map(|c| {
            (base.join(c.src.as_deref().unwrap_or_default()), secs(&c.at), secs(&c.trim_start))
        });
        BeatMarkers { src, beats: None, level: None }
    }

    /// Beat grid and loudness envelope for expressions (decoded and analysed once).
    pub fn music(&mut self) -> Result<std::sync::Arc<Music>, String> {
        let Some((path, at, trim)) = self.src.clone() else {
            return Err("add an `audio` track with a `src` to use beat, bar, level or bpm".into());
        };
        let pcm = decode_file(&path)?;
        if self.beats.is_none() {
            self.beats = Some(Ok(refine(&analyze_pcm(&pcm))));
        }
        let b = self.beats.as_ref().unwrap().as_ref().map_err(|e| e.clone())?;
        // RMS per 10 ms, normalised by the 95th percentile, quick attack and slower release.
        let mono = pcm.mono();
        let hop = (pcm.rate as usize / 100).max(1);
        let skip = (trim * pcm.rate as f64) as usize;
        let mut rms: Vec<f32> = mono.get(skip..).unwrap_or(&[]).chunks(hop).map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt()).collect();
        let mut sorted = rms.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95 = sorted.get(sorted.len() * 95 / 100).copied().unwrap_or(1.0).max(1e-4);
        let mut env = 0.0f32;
        for r in &mut rms {
            let x = (*r / p95).min(1.0);
            env = if x > env { env + (x - env) * 0.6 } else { env + (x - env) * 0.15 };
            *r = env;
        }
        self.level = Some(rms.clone());
        let shift = |v: &Vec<f64>| v.iter().map(|t| t - trim + at).collect::<Vec<f64>>();
        Ok(std::sync::Arc::new(Music { beats: shift(&b.beats), bars: shift(&b.downbeats), bpm: b.bpm, level: rms, start: at }))
    }

    /// `None` when `name` is not a beat marker; otherwise its global time or an error.
    pub fn resolve(&mut self, name: &str) -> Option<Result<f32, String>> {
        let (kind, n) = parse(name)?;
        let Some((path, at, trim)) = &self.src else {
            return Some(Err(format!("'@{name}' needs music: add an `audio` track with a `src`")));
        };
        let beats = self.beats.get_or_insert_with(|| decode_file(path).map(|p| refine(&analyze_pcm(&p))));
        let b = match beats {
            Ok(b) => b,
            Err(e) => return Some(Err(e.clone())),
        };
        let list = if kind == "beat" { &b.beats } else { &b.downbeats };
        Some(match n.checked_sub(1).and_then(|i| list.get(i)) {
            Some(t) => Ok((t - trim + at) as f32),
            None => Err(format!("'@{name}': the music has {} {kind}s (counting from 1)", list.len())),
        })
    }
}

/// Fit one steady grid to the onsets nearest the detected beats (least
/// squares, then twice more without outliers), extend it back
/// to 0, and take bars as every 4th beat in the detected downbeat phase.
pub fn refine(b: &Beats) -> Beats {
    if b.beats.len() < 4 {
        return b.clone();
    }
    // Observations: the onset nearest each detected beat (within 60 ms).
    let nearest = |t: f64| {
        let i = b.onsets.partition_point(|&o| o < t);
        [i.wrapping_sub(1), i]
            .iter()
            .filter_map(|&j| b.onsets.get(j).copied())
            .filter(|o| (o - t).abs() <= 0.06)
            .min_by(|x, y| (x - t).abs().total_cmp(&(y - t).abs()))
    };
    let mut pts: Vec<(f64, f64)> = b.beats.iter().enumerate().filter_map(|(k, &t)| nearest(t).map(|o| (k as f64, o))).collect();
    if pts.len() < 4 {
        pts = b.beats.iter().enumerate().map(|(k, &t)| (k as f64, t)).collect();
    }
    let fit = |p: &[(f64, f64)]| {
        let n = p.len() as f64;
        let (mx, my) = (p.iter().map(|q| q.0).sum::<f64>() / n, p.iter().map(|q| q.1).sum::<f64>() / n);
        let sxy: f64 = p.iter().map(|q| (q.0 - mx) * (q.1 - my)).sum();
        let sxx: f64 = p.iter().map(|q| (q.0 - mx).powi(2)).sum();
        let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
        (my - slope * mx, slope)
    };
    let (mut c, mut period) = fit(&pts);
    // Drop outliers twice with a tightening window (soft transients read early).
    for tol in [0.03, 0.015] {
        let kept: Vec<_> = pts.iter().copied().filter(|q| (c + period * q.0 - q.1).abs() <= tol).collect();
        if kept.len() >= 4 {
            (c, period) = fit(&kept);
        }
    }
    if period <= 0.05 {
        return b.clone();
    }
    // Grid index of t = 0 (allow a beat a little before 0, clamped).
    let k0 = (-(c + 0.06) / period).ceil();
    let end = b.beats.last().copied().unwrap_or(0.0) + 0.5 * period;
    let beats: Vec<f64> = (0..).map(|j| c + period * (k0 + j as f64)).take_while(|&t| t <= end).map(|t| t.max(0.0)).collect();
    let phase = b
        .downbeats
        .first()
        .and_then(|&d| beats.iter().enumerate().min_by(|x, y| (x.1 - d).abs().total_cmp(&(y.1 - d).abs())).map(|(i, _)| i % 4))
        .unwrap_or(0);
    let downbeats = beats.iter().skip(phase).step_by(4).copied().collect();
    Beats { bpm: 60.0 / period, beats, downbeats, ..b.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{write_wav, Pcm, WavFormat};

    #[test]
    fn beat_and_bar_markers_follow_the_music() {
        // 120 BPM clicks: a beat every 0.5 s, accented every 4th.
        let rate = 44_100;
        let mut mono = vec![0.0f32; rate * 8];
        for k in 0..16 {
            let i = k * rate / 2;
            let amp = if k % 4 == 0 { 1.0 } else { 0.5 };
            for j in 0..400 {
                mono[i + j] = amp * (1.0 - j as f32 / 400.0) * if j % 20 < 10 { 1.0 } else { -1.0 };
            }
        }
        let dir = std::env::temp_dir().join(format!("ak-beatmarks-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_wav(dir.join("m.wav"), &Pcm::from_mono(rate as u32, &mono), WavFormat::Pcm16).unwrap();
        let spec: Spec = serde_json::from_str(r#"{ "scenes": [ { "layers": [] } ], "audio": [ { "src": "m.wav", "at": 1 } ] }"#).unwrap();
        let mut m = BeatMarkers::new(&spec, &dir);
        assert!(m.resolve("drop").is_none());
        let b5 = m.resolve("beat5").unwrap().unwrap();
        let b6 = m.resolve("beat6").unwrap().unwrap();
        assert!(((b6 - b5) - 0.5).abs() < 0.03, "beat spacing {}", b6 - b5);
        let bar2 = m.resolve("bar2").unwrap().unwrap();
        assert!(bar2 > 1.0, "the cue's `at` shifts markers: {bar2}");
        assert!(m.resolve("beat999").unwrap().is_err());
        // The grid starts at the music's first beat (`at` = 1 s).
        let b1 = m.resolve("beat1").unwrap().unwrap();
        assert!((b1 - 1.0).abs() < 0.03, "beat1 = {b1}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
