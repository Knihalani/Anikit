//! The mixer: dry parts → auto-levelled, processed tracks → reverb / delay
//! buses → sound effects → master bus (−14 LUFS, −1 dBTP) and stems.
//!
//! Balance is set by loudness, not raw gain: each track is measured
//! (K-weighted, gated, over the time it plays) and brought to
//! `REF + level` LUFS, where `level` comes from the style (or the track's
//! `gain`). Instruments from different sources (samples, models, synths)
//! therefore sit right in the mix without per-instrument trims.

use crate::dsp::{self, Processor};
use crate::render::{RenderOpts, Stem, Timings};
use crate::spec::{Arrangement, TrackMix};
use rayon::prelude::*;
use std::time::Instant;

/// Reference loudness of a level-0 track (drums), LUFS.
const REF: f64 = -20.0;

pub struct MixOut {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub stems: Vec<Stem>,
    pub report: serde_json::Value,
}

/// Stem group of a role.
pub fn stem_of(role: &str) -> &'static str {
    match role {
        "drums" | "perc" | "percussion" | "timpani" => "drums",
        "bass" | "808" | "sub" | "low" | "basses" => "bass",
        _ => "music",
    }
}

struct Processed {
    l: Vec<f32>,
    r: Vec<f32>,
    reverb: f32,
    delay: f32,
    stem: &'static str,
}

pub fn mix(a: &Arrangement, dry: Vec<(Vec<f32>, Vec<f32>)>, rate: u32, frames: usize, opts: &RenderOpts, tm: &mut Timings) -> Result<MixOut, String> {
    let t = Instant::now();
    // Sidechain triggers (kicks) in seconds.
    let kicks = a.kicks.clone();
    let beat = a.grid.beat();
    let tracks: Vec<Processed> = dry
        .into_par_iter()
        .zip(a.tracks.par_iter())
        .map(|((mut l, mut r), t)| {
            process_track(&mut l, &mut r, &t.mix, rate, &kicks, beat)?;
            Ok(Processed { l, r, reverb: t.mix.reverb, delay: t.mix.delay, stem: stem_of(&t.role) })
        })
        .collect::<Result<_, String>>()?;
    let prof = std::env::var_os("ANIKIT_MUSIC_PROFILE").is_some();
    if prof {
        eprintln!("mix: tracks {:.3}s", t.elapsed().as_secs_f64());
    }
    // Sound effects, aligned by their hit point.
    let fx = render_sfx(a, rate, frames)?;
    if prof {
        eprintln!("mix: sfx {:.3}s", t.elapsed().as_secs_f64());
    }
    // Groups: one set of buses per stem when stems are wanted (so stems sum
    // exactly to the mix), otherwise one shared set.
    let groups: Vec<&'static str> = if opts.stems { vec!["drums", "bass", "music"] } else { vec!["all"] };
    if !dsp::REVERB_PRESETS.contains(&a.reverb.as_str()) {
        return Err(format!("music: reverb preset '{}' (use {})", a.reverb, dsp::REVERB_PRESETS.join(", ")));
    }
    let reverb = dsp::ReverbParams { preset: Some(a.reverb.clone()), wet: Some(1.0), dry: Some(0.0), ..Default::default() };
    let delay_time = beat * 0.75; // dotted 8th
    let stems: Vec<Stem> = groups
        .par_iter()
        .map(|&g| {
            let mut l = vec![0.0f32; frames];
            let mut r = vec![0.0f32; frames];
            let mut rl = vec![0.0f32; frames];
            let mut rr = vec![0.0f32; frames];
            let mut dl = vec![0.0f32; frames];
            let mut dr = vec![0.0f32; frames];
            let mut any_r = false;
            let mut any_d = false;
            for p in tracks.iter().filter(|p| g == "all" || p.stem == g) {
                add(&mut l, &p.l, 1.0);
                add(&mut r, &p.r, 1.0);
                if p.reverb > 0.0 {
                    add(&mut rl, &p.l, p.reverb);
                    add(&mut rr, &p.r, p.reverb);
                    any_r = true;
                }
                if p.delay > 0.0 {
                    add(&mut dl, &p.l, p.delay);
                    add(&mut dr, &p.r, p.delay);
                    any_d = true;
                }
            }
            if any_d {
                let mut d = dsp::Delay::new(rate as f32, &dsp::DelayParams { time: Some(delay_time as f32), feedback: 0.35, ping_pong: true, wet: 1.0, dry: 0.0, ..Default::default() });
                d.process(&mut dl, &mut dr);
                add(&mut l, &dl, 0.5);
                add(&mut r, &dr, 0.5);
                // A little of the echoes into the reverb.
                add(&mut rl, &dl, 0.3);
                add(&mut rr, &dr, 0.3);
                any_r = true;
            }
            if any_r {
                let mut rv = dsp::Reverb::new(rate as f32, &reverb);
                rv.process(&mut rl, &mut rr);
                let g = 10f32.powf(a.master.reverb_gain as f32 / 20.0) * 0.6;
                add(&mut l, &rl, g);
                add(&mut r, &rr, g);
            }
            Stem { name: g.to_string(), l, r }
        })
        .collect();
    let mut stems = stems;
    if let Some((fl, fr)) = fx {
        if opts.stems {
            stems.push(Stem { name: "fx".into(), l: fl, r: fr });
        } else {
            add(&mut stems[0].l, &fl, 1.0);
            add(&mut stems[0].r, &fr, 1.0);
        }
    }
    tm.mix = t.elapsed().as_secs_f64();
    // Master.
    let t = Instant::now();
    let mut l = vec![0.0f32; frames];
    let mut r = vec![0.0f32; frames];
    for s in &stems {
        add(&mut l, &s.l, 1.0);
        add(&mut r, &s.r, 1.0);
    }
    fades(a, &mut l, &mut r, rate);
    let rep = dsp::master(&mut l, &mut r, rate, a.master.lufs as f32, a.master.ceiling as f32);
    // Stems carry the master's static gain (not its limiter) so they line up with the mix.
    let g = 10f32.powf(rep.gain_db as f32 / 20.0);
    let mut stems = if opts.stems { stems } else { Vec::new() };
    for s in stems.iter_mut() {
        s.l.iter_mut().chain(s.r.iter_mut()).for_each(|x| *x *= g);
        fades(a, &mut s.l, &mut s.r, rate);
    }
    let report = dsp::analyze(&l, &r, rate as f32);
    tm.master = t.elapsed().as_secs_f64();
    let mut report = serde_json::to_value(&report).unwrap_or_default();
    report["lufs"] = serde_json::json!(rep.output.integrated);
    report["truePeak"] = serde_json::json!(rep.output.true_peak);
    report["master"] = serde_json::to_value(&rep).unwrap_or_default();
    Ok(MixOut { l, r, stems, report })
}

fn add(dst: &mut [f32], src: &[f32], g: f32) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d += s * g;
    }
}

/// Auto-level, filters, EQ, compression, drive, width, pan, sidechain pump.
fn process_track(l: &mut [f32], r: &mut [f32], m: &TrackMix, rate: u32, kicks: &[f64], beat: f64) -> Result<(), String> {
    // Loudness while playing → target.
    let loud = dsp::measure(l, r, rate as f32).integrated;
    if loud.is_finite() {
        let g = 10f32.powf(((REF + m.gain) - loud) as f32 / 20.0);
        l.iter_mut().for_each(|x| *x *= g);
        r.iter_mut().for_each(|x| *x *= g);
    }
    if m.hp > 0.0 || m.lp > 0.0 {
        let p = dsp::EqParams { hp: if m.hp > 0.0 { Some(m.hp) } else { None }, lp: if m.lp > 0.0 { Some(m.lp) } else { None }, ..Default::default() };
        dsp::Eq::new(rate as f32, &p).process(l, r);
    }
    if let Some(eq) = &m.eq {
        let p: dsp::EqParams = serde_json::from_value(eq.clone()).map_err(|e| format!("eq: {e}"))?;
        dsp::Eq::new(rate as f32, &p).process(l, r);
    }
    if let Some(c) = &m.comp {
        let p: dsp::CompressorParams = match c {
            serde_json::Value::Bool(true) => Default::default(),
            serde_json::Value::Bool(false) => return Ok(()),
            v => serde_json::from_value(v.clone()).map_err(|e| format!("comp: {e}"))?,
        };
        dsp::Compressor::new(rate as f32, &p).process(l, r);
    }
    if m.drive > 0.0 {
        dsp::Saturator::new(rate as f32, &dsp::SaturatorParams { drive: m.drive, ..Default::default() }).process(l, r);
    }
    if (m.width - 1.0).abs() > 1e-3 {
        dsp::Widener::new(rate as f32, &dsp::WidenerParams { width: m.width, ..Default::default() }).process(l, r);
    }
    if m.pan.abs() > 1e-3 {
        // Balance-style pan for stereo sources.
        let (gl, gr) = crate::voice::pan_gains(m.pan);
        let s = std::f32::consts::SQRT_2;
        l.iter_mut().for_each(|x| *x *= gl * s);
        r.iter_mut().for_each(|x| *x *= gr * s);
    }
    if m.duck > 0.0 && !kicks.is_empty() {
        let d = dsp::DuckerParams { depth: -m.duck.abs(), attack: 4.0, hold: 20.0, release: (beat * 550.0) as f32, ..Default::default() };
        dsp::Ducker::new(rate as f32, &d, kicks).process(l, r);
    }
    Ok(())
}

/// Place every SFX so its hit point lands on its time.
fn render_sfx(a: &Arrangement, rate: u32, frames: usize) -> Result<Option<(Vec<f32>, Vec<f32>)>, String> {
    if a.sfx.is_empty() {
        return Ok(None);
    }
    let rendered: Vec<(f64, f64, crate::sfx::Sfx)> = a
        .sfx
        .par_iter()
        .map(|s| {
            let p: crate::sfx::SfxParams = serde_json::from_value(s.params.clone()).map_err(|e| format!("sfx '{}': {e}", s.name))?;
            Ok((s.at, s.gain, crate::sfx::sfx(&s.name, &p, rate)?))
        })
        .collect::<Result<_, String>>()?;
    let mut l = vec![0.0f32; frames];
    let mut r = vec![0.0f32; frames];
    for (at, gain, fx) in rendered {
        let g = 10f32.powf(gain as f32 / 20.0) * 0.9;
        let start = ((at - fx.hit as f64) * rate as f64).round() as i64;
        for (i, (x, y)) in fx.l.iter().zip(&fx.r).enumerate() {
            let k = start + i as i64;
            if k < 0 {
                continue;
            }
            if k as usize >= frames {
                break;
            }
            l[k as usize] += x * g;
            r[k as usize] += y * g;
        }
    }
    Ok(Some((l, r)))
}

/// Fades: a short de-click at the end always; `fade` fades the last bar(s); master fade options.
fn fades(a: &Arrangement, l: &mut [f32], r: &mut [f32], rate: u32) {
    let n = l.len();
    let rf = rate as f64;
    let out = match (a.master.fade_out, a.end.as_str()) {
        (Some(s), _) => s,
        (None, "fade") => (a.grid.bar() * 2.0).min(a.length * 0.3).max(1.0),
        (None, "cut") => 0.01,
        _ => (a.length * 0.12).clamp(0.05, 1.2),
    };
    let fo = ((out * rf) as usize).min(n);
    for i in 0..fo {
        let x = (i as f32 + 0.5) / fo as f32; // 0 at the end … 1 at the fade start
        let g = (x * std::f32::consts::FRAC_PI_2).sin().powi(2);
        l[n - 1 - i] *= g;
        r[n - 1 - i] *= g;
    }
    if let Some(s) = a.master.fade_in {
        let fi = ((s * rf) as usize).min(n);
        for i in 0..fi {
            let g = ((i as f32 + 0.5) / fi as f32 * std::f32::consts::FRAC_PI_2).sin().powi(2);
            l[i] *= g;
            r[i] *= g;
        }
    }
}

/// Timing accuracy: mean and max |onset − expected| (ms) for each expected
/// hit time, detecting the onset as the first sample within ±30 ms where the
/// signal rises above 25 % of the local peak.
pub fn onset_error(l: &[f32], r: &[f32], rate: u32, times: &[f64]) -> serde_json::Value {
    let w = (0.03 * rate as f64) as i64;
    let mut errs = Vec::new();
    for &t in times {
        let c = (t * rate as f64).round() as i64;
        let (a, b) = ((c - w).max(0) as usize, ((c + w) as usize).min(l.len()));
        if a >= b {
            continue;
        }
        // Energy before the window start must be lower (a real onset).
        let peak = (a..b).map(|i| l[i].abs().max(r[i].abs())).fold(0.0f32, f32::max);
        if peak < 1e-3 {
            continue;
        }
        if let Some(i) = (a..b).find(|&i| l[i].abs().max(r[i].abs()) > 0.25 * peak) {
            errs.push((i as f64 - c as f64) / rate as f64 * 1000.0);
        }
    }
    let n = errs.len().max(1) as f64;
    serde_json::json!({
        "hits": errs.len(),
        "meanMs": errs.iter().sum::<f64>() / n,
        "meanAbsMs": errs.iter().map(|e| e.abs()).sum::<f64>() / n,
        "maxAbsMs": errs.iter().map(|e| e.abs()).fold(0.0, f64::max),
    })
}
