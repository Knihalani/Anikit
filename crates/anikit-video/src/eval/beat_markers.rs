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
    /// Generated music (`music` key): its exact grid and start time — no analysis needed.
    music: Option<Result<(anikit_music::Grid, f64), String>>,
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
        let music = spec.music.as_ref().map(|v| {
            let v = match v {
                serde_json::Value::String(s) => serde_json::json!({ "style": s }),
                v => v.clone(),
            };
            let m = anikit_music::MusicSpec::from_json(&v.to_string())?;
            Ok((anikit_music::beat_grid(&m)?, m.at.unwrap_or(0.0)))
        });
        BeatMarkers { src, beats: None, music }
    }

    /// `None` when `name` is not a beat marker; otherwise its global time or an error.
    pub fn resolve(&mut self, name: &str) -> Option<Result<f32, String>> {
        let (kind, n) = parse(name)?;
        if let Some(m) = &self.music {
            return Some(match m {
                Ok((g, at)) => {
                    let unit = if kind == "bar" { g.beats_per_bar } else { 1.0 };
                    Ok((at + g.secs(n.saturating_sub(1) as f64 * unit)) as f32)
                }
                Err(e) => Err(format!("'@{name}': {e}")),
            });
        }
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

    #[test]
    fn generated_music_gives_an_exact_grid() {
        let spec: Spec = serde_json::from_str(r#"{ "scenes": [ { "layers": [] } ], "music": { "style": "house", "bpm": 120, "at": 0.5 } }"#).unwrap();
        let mut m = BeatMarkers::new(&spec, Path::new("."));
        assert_eq!(m.resolve("beat1").unwrap().unwrap(), 0.5);
        assert_eq!(m.resolve("beat3").unwrap().unwrap(), 1.5);
        assert_eq!(m.resolve("bar2").unwrap().unwrap(), 2.5);
        // Hits nudge the tempo; markers follow the nudged grid exactly.
        let spec: Spec = serde_json::from_str(r#"{ "scenes": [ { "layers": [] } ], "music": { "style": "house", "bpm": 128, "hits": ["@2.0"] } }"#).unwrap();
        let mut m = BeatMarkers::new(&spec, Path::new("."));
        let b = (1..20).map(|n| m.resolve(&format!("beat{n}")).unwrap().unwrap()).find(|t| (t - 2.0).abs() < 1e-4);
        assert!(b.is_some(), "a beat lands exactly on the 2.0 s hit");
    }
}
