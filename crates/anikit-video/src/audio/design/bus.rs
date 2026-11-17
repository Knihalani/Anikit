//! The sound-design mix bus.
//!
//! ```text
//!  tracks (music · voice · file sfx) ──► stems ─┐
//!  cues ─► render (parallel) ─► pan ─► SFX bus ─┼─► + ─► master (EQ · glue · TP limiter → −14 LUFS / −1 dBTP)
//!                         └─► send ─► shared reverb (the scene's space) ─┘
//!  ambience bed ─────────────────────────────────┘
//!  music stem ducked a few dB under each hit (look-ahead: it dips *before* the transient)
//! ```

use super::{render, DesignParams};
use crate::audio::mix::{load_sources, mix_stems, Track};
use crate::audio::Pcm;
use anikit_music::dsp::reverb::{Reverb, ReverbParams};
use anikit_music::dsp::{master_with, Downsampler2, MasterParams, Processor, Upsampler2};
use rayon::prelude::*;
use std::path::Path;

/// One placed sound-design effect.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    /// Effect name (see [`super::all_names`]).
    pub name: String,
    /// Timeline seconds where the effect's *hit* lands (its transient, the
    /// loudest point of a whoosh, the end of a riser).
    pub at: f64,
    pub params: DesignParams,
    /// Level trim in dB.
    pub gain_db: f32,
    /// Reverb send 0..1 into the shared space.
    pub send: f32,
    /// How far the music dips under this hit (dB, 0 = not at all).
    pub duck_db: f32,
    /// `params.size` was set by the author (keep the effect's own tail even in a shared space).
    pub own_size: bool,
    /// Why the cue exists (`"scenes[0].layers[2]: lands"`), for reports.
    pub why: String,
}

impl Cue {
    pub fn new(name: &str, at: f64) -> Cue {
        Cue {
            name: name.into(),
            at,
            params: DesignParams::default(),
            gain_db: 0.0,
            send: 0.2,
            duck_db: 0.0,
            own_size: false,
            why: String::new(),
        }
    }
}

/// Bus settings.
#[derive(Debug, Clone, PartialEq)]
pub struct BusOpts {
    /// Integrated loudness target (LUFS); `None` leaves the level alone (limiter only).
    pub target_lufs: Option<f32>,
    /// True-peak ceiling (dBTP).
    pub true_peak: f32,
    /// Shared reverb: preset (`room`, `hall`, `plate`, `chamber`, `cathedral`,
    /// `ambience`) and size; `None` = dry.
    pub space: Option<(String, f32)>,
    /// Ambience bed: name, level trim (dB), seed.
    pub ambience: Option<(String, f32, u64)>,
    /// Overall SFX trim (dB).
    pub sfx_db: f32,
    /// Duck the music under cues' `duck_db`.
    pub duck: bool,
}

impl Default for BusOpts {
    fn default() -> Self {
        BusOpts {
            target_lufs: Some(-14.0),
            true_peak: -1.0,
            space: Some(("room".into(), 0.6)),
            ambience: None,
            sfx_db: 0.0,
            duck: true,
        }
    }
}

/// What the bus did.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixReport {
    pub cues: usize,
    pub integrated_lufs: f64,
    pub true_peak_dbtp: f64,
    pub lra: f64,
    pub max_music_duck_db: f32,
    pub millis: f64,
}

/// Effects whose generators place themselves in the stereo field (moving
/// sounds and the music engine's library take `pan` / `motion` directly).
fn self_panning(name: &str) -> bool {
    matches!(
        name,
        "air" | "swish" | "whoosh-deep" | "whoosh-air" | "passby" | "droplet" | "blip" | "ticker"
    ) || anikit_music::sfx::SFX.contains(&name)
        || name == "downer"
}

/// Mix `tracks` (files, voice, music, built-in sfx), sound-design `cues` and
/// an ambience bed into `duration` seconds at `rate`, then master.
pub fn mix_design(
    tracks: &[Track],
    cues: &[Cue],
    opts: &BusOpts,
    duration: f64,
    rate: u32,
    base: Option<&Path>,
) -> Result<(Pcm, MixReport), String> {
    let t0 = std::time::Instant::now();
    let prof = std::env::var_os("ANIKIT_SOUND_PROFILE").is_some();
    let lap = |what: &str| {
        if prof {
            eprintln!(
                "  sound bus {what}: {:.1} ms",
                t0.elapsed().as_secs_f64() * 1e3
            );
        }
    };
    let sr = rate as f32;
    let n = (duration.max(0.0) * rate as f64).round() as usize;
    // 1. Stems from the ordinary tracks.
    let sources = load_sources(tracks, rate, base)?;
    let [mut music, voice, fsfx] = mix_stems(tracks, &sources, duration, rate);
    lap("stems");
    // 2. Render every cue (in parallel; summed in order, so deterministic).
    let dry_space = opts.space.is_some();
    let render_cues = || -> Vec<Result<anikit_music::sfx::Sfx, String>> {
        cues.par_iter()
            .map(|c| {
                let mut p = c.params.clone();
                let key = super::canonical(&c.name);
                let own_pan = self_panning(&key);
                if !own_pan {
                    p.pan = 0.0;
                }
                // One space: with a shared reverb, effects come dry (their `size`
                // is the cue's own choice only when it set one explicitly).
                if dry_space && !c.own_size {
                    p.size = 0.0;
                }
                let mut s = render(&key, &p, rate)?;
                if !own_pan && c.params.pan != 0.0 {
                    let (gl, gr) = super::synth::pan_gains(c.params.pan * 0.85);
                    s.l.iter_mut().for_each(|x| *x *= gl);
                    s.r.iter_mut().for_each(|x| *x *= gr);
                }
                Ok(s)
            })
            .collect()
    };
    let render_amb = || -> Result<Option<anikit_music::sfx::Sfx>, String> {
        let Some((name, level, seed)) = &opts.ambience else {
            return Ok(None);
        };
        let p = DesignParams {
            duration: Some(duration as f32),
            seed: *seed,
            ..Default::default()
        };
        let mut s = render(name, &p, rate)?;
        let g = 10f32.powf(level / 20.0);
        s.l.iter_mut().chain(s.r.iter_mut()).for_each(|x| *x *= g);
        Ok(Some(s))
    };
    let (rendered, amb) = rayon::join(render_cues, render_amb);
    let amb = amb?;
    let mut dry_l = vec![0.0f32; n];
    let mut dry_r = vec![0.0f32; n];
    let mut send_l = vec![0.0f32; n];
    let mut send_r = vec![0.0f32; n];
    let sfx_gain = 10f32.powf(opts.sfx_db / 20.0);
    for (c, s) in cues.iter().zip(rendered) {
        let s =
            s.map_err(|e| format!("{}: {e}", if c.why.is_empty() { &c.name } else { &c.why }))?;
        let start = ((c.at - s.hit as f64) * rate as f64).round() as i64;
        let g = 10f32.powf(c.gain_db / 20.0) * sfx_gain;
        let send = c.send.clamp(0.0, 1.0);
        for k in 0..s.l.len() {
            let o = start + k as i64;
            if o < 0 {
                continue;
            }
            let o = o as usize;
            if o >= n {
                break;
            }
            let (l, r) = (s.l[k] * g, s.r[k] * g);
            dry_l[o] += l;
            dry_r[o] += r;
            send_l[o] += l * send;
            send_r[o] += r * send;
        }
    }
    lap("cues placed");
    // 3. The shared space.
    if let Some((preset, size)) = &opts.space {
        if send_l.iter().any(|x| *x != 0.0) {
            let p = ReverbParams {
                preset: Some(preset.clone()),
                size: Some(size.clamp(0.1, 2.0)),
                ..Default::default()
            };
            // Only where there is signal, plus the tail; at half rate (a room's
            // tail above 12 kHz is mostly absorbed anyway): half-band filters
            // down, the FDN at 24 kHz, half-band up. ≈2× faster.
            let first = send_l
                .iter()
                .zip(&send_r)
                .position(|(a, b)| *a != 0.0 || *b != 0.0)
                .unwrap_or(0);
            let last = send_l
                .iter()
                .zip(&send_r)
                .rposition(|(a, b)| *a != 0.0 || *b != 0.0)
                .unwrap_or(0);
            let end = (last + (4.0 * sr) as usize).min(n);
            let half = (end - first) / 2;
            let mut rv = Reverb::try_new(sr / 2.0, &p)?;
            rv.set_mix(1.0, 0.0);
            let mut dl = vec![0.0f32; half];
            let mut dr = vec![0.0f32; half];
            Downsampler2::new(true).process(&send_l[first..first + 2 * half], &mut dl);
            Downsampler2::new(true).process(&send_r[first..first + 2 * half], &mut dr);
            rv.process(&mut dl, &mut dr);
            Upsampler2::new(true).process(&dl, &mut send_l[first..first + 2 * half]);
            Upsampler2::new(true).process(&dr, &mut send_r[first..first + 2 * half]);
            for k in first..first + 2 * half {
                dry_l[k] += send_l[k] * 0.5;
                dry_r[k] += send_r[k] * 0.5;
            }
        }
    }
    // 4. Duck the music under hits: −duck_db reached at the hit (ramping in
    // over 30 ms before it), held 80 ms, released with a 250 ms time constant.
    let mut max_duck = 0.0f32;
    if opts.duck && cues.iter().any(|c| c.duck_db > 0.0) {
        const B: usize = 64;
        let blocks = n / B + 1;
        let mut red = vec![0.0f32; blocks];
        let bs = B as f64 / rate as f64;
        for c in cues.iter().filter(|c| c.duck_db > 0.0) {
            let d = c.duck_db.min(12.0);
            let hit = c.at / bs;
            let a0 = (hit - 0.03 / bs).floor().max(0.0) as usize;
            let tail = (1.5 / bs) as usize;
            for (b, rb) in red
                .iter_mut()
                .enumerate()
                .skip(a0)
                .take(tail + (0.03 / bs) as usize + 1)
            {
                let t = b as f64 - hit; // blocks after the hit
                let v = if t < 0.0 {
                    d * (1.0 + t * bs / 0.03).clamp(0.0, 1.0) as f32
                } else if t * bs < 0.08 {
                    d
                } else {
                    d * (-((t * bs - 0.08) / 0.25)).exp() as f32
                };
                if v > *rb {
                    *rb = v;
                }
            }
        }
        // Smooth across blocks (no zipper), then apply per sample.
        let mut g = 1.0f32;
        let k = 1.0 - (-1.0 / 3.0f32).exp();
        for (b, &r) in red.iter().enumerate() {
            max_duck = max_duck.max(r);
            let target = 10f32.powf(-r / 20.0);
            for i in b * B..((b + 1) * B).min(n) {
                g += (target - g) * k * 0.05;
                music[2 * i] *= g;
                music[2 * i + 1] *= g;
            }
        }
    }
    lap("duck");
    lap("duck");
    // 6. Sum.
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for k in 0..n {
        l[k] = music[2 * k] + voice[2 * k] + fsfx[2 * k] + dry_l[k];
        r[k] = music[2 * k + 1] + voice[2 * k + 1] + fsfx[2 * k + 1] + dry_r[k];
    }
    if let Some(a) = &amb {
        for k in 0..n.min(a.l.len()) {
            l[k] += a.l[k];
            r[k] += a.r[k];
        }
    }
    lap("sum");
    // 7. Master.
    let mp = MasterParams {
        target: opts.target_lufs.unwrap_or(-14.0),
        ceiling: opts.true_peak,
        ..Default::default()
    };
    let rep = if opts.target_lufs.is_some() {
        Some(master_with(&mut l, &mut r, rate, &mp))
    } else {
        // Level as mixed: only keep true peaks under the ceiling.
        let lin = anikit_music::dsp::loudness::measure(&l, &r, sr);
        if lin.true_peak > opts.true_peak as f64 {
            let g = 10f32.powf((opts.true_peak - lin.true_peak as f32) / 20.0);
            l.iter_mut().chain(r.iter_mut()).for_each(|x| *x *= g);
        }
        None
    };
    lap("master");
    let out = anikit_music::dsp::loudness::measure(&l, &r, sr);
    lap("measure");
    let mut samples = Vec::with_capacity(n * 2);
    for k in 0..n {
        samples.push(l[k]);
        samples.push(r[k]);
    }
    let _ = rep;
    Ok((
        Pcm {
            rate,
            channels: 2,
            samples,
        },
        MixReport {
            cues: cues.len(),
            integrated_lufs: out.integrated,
            true_peak_dbtp: out.true_peak,
            lra: out.lra,
            max_music_duck_db: max_duck,
            millis: t0.elapsed().as_secs_f64() * 1e3,
        },
    ))
}
