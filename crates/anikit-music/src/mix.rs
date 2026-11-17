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
    // The sidechain pump's shape is shared by every ducked track: compute it
    // once (as ln of the gain at 1 dB depth), in parallel chunks.
    let pump: Option<Vec<f32>> = (!kicks.is_empty() && a.tracks.iter().any(|t| t.mix.duck > 0.0)).then(|| {
        let d = dsp::DuckerParams { depth: -1.0, attack: 4.0, hold: 20.0, release: (beat * 550.0) as f32, ..Default::default() };
        let mut env = vec![1.0f32; frames];
        env.par_chunks_mut(1 << 16).enumerate().for_each(|(k, c)| {
            let mut du = dsp::Ducker::new(rate as f32, &d, &kicks);
            du.seek((k << 16) as u64);
            let mut c2 = vec![1.0f32; c.len()];
            du.process(c, &mut c2);
            c.iter_mut().for_each(|x| *x = x.ln());
        });
        env
    });
    let tracks: Vec<Processed> = dry
        .into_par_iter()
        .zip(a.tracks.par_iter())
        .map(|((mut l, mut r), t)| {
            let t0 = Instant::now();
            process_track(&mut l, &mut r, &t.mix, rate, pump.as_deref())?;
            if std::env::var_os("ANIKIT_MUSIC_PROFILE").is_some() {
                eprintln!("  track {} ({}): {:.3}s", t.name, t.inst, t0.elapsed().as_secs_f64());
            }
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
    let a_gain = a.master.reverb_gain as f32;
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
                let ir = reverb_ir(rate, &reverb);
                let ((a, b), (c, d)) = rayon::join(|| dsp::convolve(&rl, &rl, &ir.0), || dsp::convolve(&rr, &rr, &ir.1));
                let g = 10f32.powf(a_gain / 20.0) * 0.6;
                add(&mut l, &a[..frames], g);
                add(&mut l, &c[..frames], g);
                add(&mut r, &b[..frames], g);
                add(&mut r, &d[..frames], g);
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

/// The reverb as a true-stereo impulse response (L→LR, R→LR), rendered once per
/// preset and rate from the algorithmic reverb, then applied by FFT convolution
/// (several times faster than running the FDN over the whole track).
pub(crate) fn reverb_ir(rate: u32, p: &dsp::ReverbParams) -> std::sync::Arc<(dsp::Ir, dsp::Ir)> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    type Cache = Mutex<HashMap<(String, u32), Arc<(dsp::Ir, dsp::Ir)>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (format!("{p:?}"), rate);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(ir) = cache.lock().unwrap().get(&key) {
        return ir.clone();
    }
    // On-disk cache: the IR depends only on the preset and the rate.
    let file = {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in key.0.bytes().chain(rate.to_le_bytes()).chain(*b"ir-v1") {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
        crate::sf2::packs::cache_dir().join(format!("reverb-{h:016x}.f32"))
    };
    if let Ok(b) = std::fs::read(&file) {
        let v: Vec<f32> = b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        if v.len().is_multiple_of(4) && !v.is_empty() {
            let n = v.len() / 4;
            let ir = Arc::new((dsp::Ir { l: v[..n].to_vec(), r: v[n..2 * n].to_vec(), rate }, dsp::Ir { l: v[2 * n..3 * n].to_vec(), r: v[3 * n..].to_vec(), rate }));
            cache.lock().unwrap().insert(key, ir.clone());
            return ir;
        }
    }
    // Render in 0.5 s blocks until the tail has died away (max 12 s).
    let one = |left: bool| {
        let blk = rate as usize / 2;
        let mut rv = dsp::Reverb::new(rate as f32, p);
        let (mut l, mut r) = (Vec::new(), Vec::new());
        let mut peak = 0.0f32;
        for k in 0..24 {
            let (mut bl, mut br) = (vec![0.0f32; blk], vec![0.0f32; blk]);
            if k == 0 {
                if left {
                    bl[0] = 1.0;
                } else {
                    br[0] = 1.0;
                }
            }
            rv.process(&mut bl, &mut br);
            let m = bl.iter().chain(&br).fold(0.0f32, |m, x| m.max(x.abs()));
            peak = peak.max(m);
            l.extend_from_slice(&bl);
            r.extend_from_slice(&br);
            if k > 1 && m < peak * 1e-5 {
                break;
            }
        }
        (l, r)
    };
    let ((ll, lr), (rl, rr)) = rayon::join(|| one(true), || one(false));
    // Trim the inaudible tail (below −100 dB of the peak).
    let peak = ll.iter().chain(&lr).chain(&rl).chain(&rr).fold(0.0f32, |m, x| m.max(x.abs()));
    let floor = peak * 1e-5;
    let last = [&ll, &lr, &rl, &rr].iter().map(|v| v.iter().rposition(|x| x.abs() > floor).unwrap_or(0)).max().unwrap_or(0) + 1;
    let cut = |mut v: Vec<f32>| {
        v.resize(last, 0.0);
        // Short fade so the truncation is silent.
        let f = (rate as usize / 20).min(last);
        for i in 0..f {
            v[last - 1 - i] *= i as f32 / f as f32;
        }
        v
    };
    let ir = Arc::new((dsp::Ir { l: cut(ll), r: cut(lr), rate }, dsp::Ir { l: cut(rl), r: cut(rr), rate }));
    let bytes: Vec<u8> = [&ir.0.l, &ir.0.r, &ir.1.l, &ir.1.r].iter().flat_map(|v| v.iter().flat_map(|x| x.to_le_bytes())).collect();
    if std::fs::create_dir_all(file.parent().unwrap_or(std::path::Path::new("."))).is_ok() {
        // Write then rename, so a concurrent reader never sees a partial file.
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&tmp, &bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &file);
        }
    }
    cache.lock().unwrap().insert(key, ir.clone());
    ir
}

fn add(dst: &mut [f32], src: &[f32], g: f32) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d += s * g;
    }
}

/// Auto-level, filters, EQ, compression, drive, width, pan, sidechain pump.
fn process_track(l: &mut [f32], r: &mut [f32], m: &TrackMix, rate: u32, pump: Option<&[f32]>) -> Result<(), String> {
    // Loudness while playing → target.
    let loud = dsp::loudness::integrated_lufs(l, r, rate as f32);
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
    if let (true, Some(p)) = (m.duck > 0.0, pump) {
        // gain = (gain at 1 dB)^depth
        let k = m.duck.abs();
        for ((x, y), e) in l.iter_mut().zip(r.iter_mut()).zip(p) {
            // exp(x) as (1 + x/256)^256: branch-free and vectorisable; |x| ≤ 2.8 here,
            // relative error < 2e-4 (≈ 0.002 dB).
            let mut g = 1.0 + e * k * (1.0 / 256.0);
            for _ in 0..8 {
                g *= g;
            }
            *x *= g;
            *y *= g;
        }
    }
    Ok(())
}

/// Place every SFX so its hit point lands on its time.
type StereoBuf = (Vec<f32>, Vec<f32>);

fn render_sfx(a: &Arrangement, rate: u32, frames: usize) -> Result<Option<StereoBuf>, String> {
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

/// Timing accuracy of hits against their expected times (ms): for each
/// expected time, the onset is the steepest rise of a 1 ms energy envelope
/// within ±30 ms (robust to the tails of earlier hits still ringing).
pub fn onset_error(l: &[f32], r: &[f32], rate: u32, times: &[f64]) -> serde_json::Value {
    let n = l.len().min(r.len());
    let w = (0.001 * rate as f64).round() as usize; // 1 ms
    // Energy in 1 ms frames, hop 0.25 ms.
    let hop = (w / 4).max(1);
    let frames = n / hop;
    let mut e = vec![0.0f32; frames];
    for (k, v) in e.iter_mut().enumerate() {
        let a = k * hop;
        let b = (a + w).min(n);
        *v = (a..b).map(|i| l[i] * l[i] + r[i] * r[i]).sum::<f32>();
    }
    let mut errs = Vec::new();
    for &t in times {
        let c = t * rate as f64 / hop as f64;
        let span = 0.03 * rate as f64 / hop as f64;
        let (a, b) = (((c - span).max(4.0)) as usize, ((c + span) as usize).min(frames));
        if a + 8 >= b {
            continue;
        }
        // Steepest rise in dB: log energy difference over 1 ms.
        let lg = |k: usize| 10.0 * (e[k] + 1e-12).log10();
        let lag = w / hop;
        let best = (a..b).max_by(|&i, &j| (lg(i) - lg(i - lag)).total_cmp(&(lg(j) - lg(j - lag))));
        if let Some(k) = best {
            if lg(k) - lg(k - lag) < 6.0 {
                continue; // no clear onset
            }
            // The rise ends at frame k (window k..k+w ends ~1 ms later): the onset is near its start.
            let onset = (k as f64 - lag as f64 * 0.5) * hop as f64 / rate as f64;
            errs.push((onset - t) * 1000.0);
        }
    }
    let m = errs.len().max(1) as f64;
    serde_json::json!({
        "hits": errs.len(),
        "meanMs": errs.iter().sum::<f64>() / m,
        "meanAbsMs": errs.iter().map(|e| e.abs()).sum::<f64>() / m,
        "maxAbsMs": errs.iter().map(|e| e.abs()).fold(0.0, f64::max),
    })
}
