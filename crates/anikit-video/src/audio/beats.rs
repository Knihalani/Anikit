//! Beat analysis for cutting on the beat: tempo, beat grid, downbeats,
//! onsets and a loudness envelope, built on [`anikit_core::audio::analyze`].

use super::Pcm;
use serde::{Deserialize, Serialize};

/// Hop of [`Beats::energy`] in seconds (100 values per second).
pub const ENERGY_HOP: f64 = 0.01;

/// The result of [`analyze_pcm`]; serialises as
/// `{ "bpm", "beats", "downbeats", "onsets", "energy", "hop" }`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Beats {
    /// Tempo (0 when no steady pulse was found).
    pub bpm: f64,
    /// Beat times in seconds.
    pub beats: Vec<f64>,
    /// Bar starts (every 4th beat, phase chosen by accent), in seconds.
    pub downbeats: Vec<f64>,
    /// Onset (transient) times in seconds.
    pub onsets: Vec<f64>,
    /// RMS loudness normalised to 0…1, one value every `hop` seconds.
    pub energy: Vec<f32>,
    /// Seconds between `energy` values ([`ENERGY_HOP`]).
    pub hop: f64,
}

/// Analysis frame length in samples used by the core analyser; its times
/// refer to frame starts, so events are reported this much early.
const FRAME: f64 = 1024.0;

/// Analyse `pcm` (mono downmix).
pub fn analyze_pcm(pcm: &Pcm) -> Beats {
    if pcm.rate == 0 || pcm.samples.is_empty() {
        return Beats {
            hop: ENERGY_HOP,
            ..Default::default()
        };
    }
    let rate = pcm.rate as f64;
    let mono = pcm.mono();
    let a = anikit_core::audio::analyze(&mono, pcm.rate as f32);
    let hop = a.hop as f64;
    // Core times are frame starts, so a transient is reported while it is
    // still ahead in the window (measured: ≈0.65 frame early on click tracks
    // at 44.1 and 48 kHz, ±1 ms). Shift times onto the event itself.
    let latency = 0.65 * FRAME / rate;
    let end = pcm.duration();
    let shift = |t: f32| (t as f64 + latency).clamp(0.0, end);
    let onsets: Vec<f64> = a.onsets.iter().map(|&t| shift(t)).collect();
    let beats: Vec<f64> = a
        .beats
        .iter()
        .map(|&t| t as f64 + latency)
        .filter(|t| (0.0..end).contains(t))
        .collect();

    // Downbeats: the 4-beat phase with the most onset strength and loudness.
    let at = |v: &[f32], t: f64| -> f32 {
        let i = ((t - latency) / hop).round();
        if i < 0.0 {
            return 0.0;
        }
        v.get(i as usize).copied().unwrap_or(0.0)
    };
    let mut best = (0usize, f32::MIN);
    for phase in 0..4.min(beats.len()) {
        let s: f32 = beats
            .iter()
            .skip(phase)
            .step_by(4)
            .map(|&t| at(&a.flux, t) + 0.5 * at(&a.envelope, t))
            .sum::<f32>()
            / beats.iter().skip(phase).step_by(4).count().max(1) as f32;
        if s > best.1 + 1e-6 {
            best = (phase, s);
        }
    }
    let downbeats = beats.iter().skip(best.0).step_by(4).copied().collect();

    // Energy resampled to a fixed hop; envelope value f describes the window
    // centred at f·hop + FRAME/2.
    let n = (end / ENERGY_HOP).ceil() as usize;
    let centre = FRAME / 2.0 / rate;
    let env = &a.envelope;
    let energy = (0..n)
        .map(|k| {
            if env.is_empty() {
                return 0.0;
            }
            let x = ((k as f64 * ENERGY_HOP - centre) / hop).max(0.0);
            let i = x.floor() as usize;
            if i + 1 >= env.len() {
                return env[env.len() - 1];
            }
            let f = (x - i as f64) as f32;
            env[i] * (1.0 - f) + env[i + 1] * f
        })
        .collect();

    Beats {
        bpm: a.bpm as f64,
        beats,
        downbeats,
        onsets,
        energy,
        hop: ENERGY_HOP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clicks every 60/bpm s from `first`, with an accent every 4th beat.
    fn click_track(bpm: f64, first: f64, secs: f64, rate: u32) -> Pcm {
        let n = (secs * rate as f64) as usize;
        let mut mono = vec![0.0f32; n];
        let mut seed = 0x1234_5678u32;
        let mut k = 0;
        loop {
            let t = first + k as f64 * 60.0 / bpm;
            if t >= secs {
                break;
            }
            let at = (t * rate as f64).round() as usize;
            let amp = if k % 4 == 0 { 1.0 } else { 0.6 };
            let len = rate as usize / 100;
            for i in 0..len.min(n - at) {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
                mono[at + i] = amp * noise * (1.0 - i as f32 / len as f32).powi(2);
            }
            k += 1;
        }
        Pcm::from_mono(rate, &mono)
    }

    #[test]
    fn finds_120_bpm_beats() {
        let first = 0.3;
        let pcm = click_track(120.0, first, 12.0, 48_000);
        let b = analyze_pcm(&pcm);
        assert!((b.bpm - 120.0).abs() <= 2.0, "bpm {}", b.bpm);
        assert!(b.beats.len() >= 20, "{} beats", b.beats.len());
        let mut worst = 0.0f64;
        for &t in &b.beats {
            let k = ((t - first) / 0.5).round();
            worst = worst.max((t - (first + k * 0.5)).abs());
        }
        assert!(worst <= 0.005, "worst beat error {:.1} ms", worst * 1000.0);
        // Downbeats on the accented clicks (0.3, 2.3, 4.3, …).
        assert!(b.downbeats.len() >= 5);
        for &t in &b.downbeats {
            let k = ((t - first) / 2.0).round();
            assert!((t - (first + k * 2.0)).abs() < 0.03, "downbeat {t}");
        }
        assert!(b.onsets.len() >= 20);
        assert_eq!(b.energy.len(), 1200);
        assert!(b.energy.iter().all(|e| (0.0..=1.0).contains(e)));
        let json = serde_json::to_value(&b).unwrap();
        assert!(json["beats"].is_array() && json["hop"].as_f64() == Some(0.01));
    }

    #[test]
    fn silence_has_no_beats() {
        let b = analyze_pcm(&Pcm::silence(48_000, 48_000));
        assert!(b.beats.is_empty() && b.downbeats.is_empty());
        assert_eq!(analyze_pcm(&Pcm::default()).beats.len(), 0);
    }
}
