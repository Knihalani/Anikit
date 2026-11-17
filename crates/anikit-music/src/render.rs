//! Spec → audio: compose, render every part in parallel, mix and master.

use crate::instruments;
use crate::mix;
use crate::seq::{self, Part};
use crate::spec::{Arrangement, MusicSpec, TrackMix, TrackPlan};
use crate::style;
use rayon::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

/// Render options.
#[derive(Debug, Clone, Default)]
pub struct RenderOpts {
    /// Output sample rate (default 48 kHz).
    pub rate: u32,
    /// Length to fill when the spec has none (the video's duration).
    pub video_length: Option<f64>,
    /// Directory relative paths (MIDI, SFZ, SF2, IRs) resolve against.
    pub base_dir: Option<PathBuf>,
    /// Also return stems (drums, bass, music, fx) that sum to the pre-master mix.
    pub stems: bool,
}

/// A stem: post-fader audio of a group of tracks (including their reverb and delay).
#[derive(Debug, Clone)]
pub struct Stem {
    pub name: String,
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

/// Wall-clock timings of a render (seconds).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Timings {
    pub compose: f64,
    pub instruments: f64,
    pub voices: f64,
    pub mix: f64,
    pub master: f64,
    pub total: f64,
    /// Seconds of audio per second of wall clock.
    pub realtime: f64,
    pub notes: usize,
}

/// A rendered track.
pub struct Rendered {
    pub rate: u32,
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub arrangement: Arrangement,
    pub stems: Vec<Stem>,
    /// Listening-proxy metrics of the master (loudness, true peak, spectrum…).
    pub report: serde_json::Value,
    pub timings: Timings,
}

impl Rendered {
    /// Interleaved stereo.
    pub fn interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.l.len() * 2);
        for (a, b) in self.l.iter().zip(&self.r) {
            out.push(*a);
            out.push(*b);
        }
        out
    }
    pub fn duration(&self) -> f64 {
        self.l.len() as f64 / self.rate as f64
    }
}

/// Compose a spec into an arrangement (no audio). Cheap: use it for beat grids.
pub fn arrange(spec: &MusicSpec, opts: &RenderOpts) -> Result<Arrangement, String> {
    let mut a = style::compose(spec, opts.video_length)?;
    if let Some(m) = &spec.midi {
        let path = match &opts.base_dir {
            Some(b) if std::path::Path::new(m).is_relative() => b.join(m),
            _ => PathBuf::from(m),
        };
        let bytes = std::fs::read(&path).map_err(|e| format!("music: midi '{}': {e}", path.display()))?;
        let song = crate::midi::import(&bytes)?;
        if spec.bpm.is_none() && a.style == "custom" {
            a.grid.bpm = song.bpm;
            a.grid.beats_per_bar = song.beats_per_bar;
        }
        let mut end: f64 = 0.0;
        for t in song.tracks {
            let inst = if t.channel == 9 { "drums".to_string() } else { format!("gm:{}", t.program.unwrap_or(0)) };
            end = end.max(t.notes.iter().map(|n| n.t + n.len).fold(0.0, f64::max));
            a.tracks.push(TrackPlan {
                name: t.name,
                role: if t.channel == 9 { "drums".into() } else { "music".into() },
                inst,
                notes: t.notes,
                mix: TrackMix { reverb: 0.15, ..Default::default() },
                humanize: 0.0,
                straight: true,
                fx: None,
                automate: None,
            });
        }
        if spec.length.is_none() && opts.video_length.is_none() {
            a.length = a.length.max(a.grid.secs(end) + 2.0);
        }
    }
    Ok(a)
}

/// Set flush-to-zero and denormals-are-zero on this thread (x86): decaying
/// filter and reverb states otherwise crawl through the subnormal range,
/// where arithmetic is ~100× slower. Deterministic either way.
pub fn ftz() {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    #[allow(deprecated)]
    unsafe {
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::{_mm_getcsr, _mm_setcsr};
        #[cfg(target_arch = "x86")]
        use std::arch::x86::{_mm_getcsr, _mm_setcsr};
        // SAFETY: only sets the FTZ (bit 15) and DAZ (bit 6) flags of MXCSR.
        _mm_setcsr(_mm_getcsr() | 0x8040);
    }
}

/// The engine's thread pool: one worker per core, FTZ/DAZ on every worker.
fn pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let n = std::env::var("ANIKIT_MUSIC_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or_else(rayon::current_num_threads);
        rayon::ThreadPoolBuilder::new().num_threads(n).thread_name(|i| format!("anikit-music-{i}")).start_handler(|_| ftz()).build().expect("thread pool")
    })
}

/// Render a spec to mastered stereo.
pub fn render(spec: &MusicSpec, opts: &RenderOpts) -> Result<Rendered, String> {
    pool().install(|| {
        ftz();
        render_inner(spec, opts)
    })
}

fn render_inner(spec: &MusicSpec, opts: &RenderOpts) -> Result<Rendered, String> {
    let t0 = Instant::now();
    let a = arrange(spec, opts)?;
    let compose = t0.elapsed().as_secs_f64();
    let mut r = render_arrangement(a, opts)?;
    r.timings.compose = compose;
    r.timings.total = t0.elapsed().as_secs_f64();
    r.timings.realtime = r.duration() / r.timings.total.max(1e-9);
    Ok(r)
}

/// Render an arrangement.
pub fn render_arrangement(a: Arrangement, opts: &RenderOpts) -> Result<Rendered, String> {
    let rate = if opts.rate == 0 { crate::RATE } else { opts.rate };
    let mut tm = Timings::default();
    let t = Instant::now();
    // Instruments: custom definitions (with per-note `synth.*` automation) or names.
    let mut customs: Vec<Option<std::sync::Arc<crate::custom::CustomSynth>>> = Vec::with_capacity(a.tracks.len());
    let mut insts: Vec<std::sync::Arc<dyn crate::Instrument>> = Vec::with_capacity(a.tracks.len());
    for tr in &a.tracks {
        let err = |e: String| format!("music: track '{}': {e}", tr.name);
        let per_note = match &tr.automate {
            Some(v) => crate::custom::TrackAuto::parse(v, &a.grid, &format!("tracks.{}.automate", tr.name)).map_err(err)?.per_note,
            None => Vec::new(),
        };
        if let Some(def) = a.defines.get(&tr.inst) {
            let is_synth = def.get("synth").is_some();
            if !is_synth && !per_note.is_empty() {
                return Err(err("`synth.*` automation needs a custom synth instrument (define it with \"synth\")".into()));
            }
            if is_synth {
                let p = crate::custom::synth_patch(&def["synth"], &format!("define.{}.synth", tr.inst)).map_err(err)?;
                let c = std::sync::Arc::new(crate::custom::CustomSynth { synth: crate::synth::patch::SubSynth::new(p), per_note });
                insts.push(c.clone());
                customs.push(Some(c));
            } else {
                insts.push(crate::custom::build(&tr.inst, def, opts.base_dir.as_deref(), Vec::new()).map_err(err)?);
                customs.push(None);
            }
        } else {
            if !per_note.is_empty() {
                return Err(err(format!("`synth.*` automation needs a custom synth: define \"{}\" under \"define\"", tr.inst)));
            }
            let inst = instruments::resolve(&tr.inst, opts.base_dir.as_deref()).map_err(err)?;
            // Kits get a drummer's stereo image (one track holds the whole kit).
            insts.push(if tr.role == "drums" { std::sync::Arc::new(instruments::KitStage(inst)) } else { inst });
            customs.push(None);
        }
    }
    tm.instruments = t.elapsed().as_secs_f64();
    let frames = (a.length * rate as f64).round() as usize;
    let t = Instant::now();
    tm.notes = a.tracks.iter().map(|t| t.notes.len()).sum();
    let dry: Vec<(Vec<f32>, Vec<f32>)> = a
        .tracks
        .par_iter()
        .zip(insts.par_iter().zip(customs.par_iter()))
        .enumerate()
        .map(|(i, (tr, (inst, custom)))| {
            let part = Part {
                name: tr.name.clone(),
                inst: inst.clone(),
                notes: tr.notes.clone(),
                humanize: tr.humanize,
                seed: a.seed.wrapping_add(i as u64 * 0x2545_F491),
                straight: tr.straight,
                custom: custom.clone(),
            };
            seq::render_part(&part, &a.grid, rate, frames)
        })
        .collect();
    tm.voices = t.elapsed().as_secs_f64();
    let out = mix::mix(&a, dry, rate, frames, opts, &mut tm)?;
    Ok(Rendered { rate, l: out.l, r: out.r, stems: out.stems, report: out.report, arrangement: a, timings: tm })
}
