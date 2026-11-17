//! Throughput of every DSP processor on 60 s of stereo at 48 kHz (× real time),
//! processed in engine-sized blocks, plus offline convolution, metering,
//! mastering and SFX rendering.
//!
//! `cargo run --release -p anikit-music --example dsp_bench`

use std::time::Instant;

use anikit_music::dsp::*;
use anikit_music::sfx::{sfx, SfxParams, SFX};
use anikit_music::Rng;

const FS: f32 = 48_000.0;
const SECS: f32 = 60.0;

/// A busy test mix: kick, bass, chords, hats and noise.
fn mix() -> (Vec<f32>, Vec<f32>) {
    let n = (SECS * FS) as usize;
    let mut rng = Rng::new(5);
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for i in 0..n {
        let t = i as f32 / FS;
        let tb = t % 0.5;
        let kick = (std::f32::consts::TAU * (50.0 + 120.0 * (-tb * 30.0).exp()) * tb).sin() * (-tb * 9.0).exp() * 0.5;
        let bass = (std::f32::consts::TAU * 55.0 * t).sin() * 0.15;
        let pad = ((std::f32::consts::TAU * 220.0 * t).sin() + (std::f32::consts::TAU * 330.0 * t).sin()) * 0.05;
        let hat = rng.bi() * (-((t + 0.25) % 0.5) * 60.0).exp() * 0.12;
        l[i] = kick + bass + pad + hat * 0.8;
        r[i] = kick + bass + pad * 0.9 + hat;
    }
    (l, r)
}

fn bench(name: &str, src: &(Vec<f32>, Vec<f32>), p: &mut dyn Processor) {
    let (mut l, mut r) = src.clone();
    // Warm-up pass on a short slice.
    p.process(&mut l[..48_000].to_vec(), &mut r[..48_000].to_vec());
    p.reset();
    let t = Instant::now();
    for (a, b) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
        p.process(a, b);
    }
    report(name, t.elapsed().as_secs_f64());
    assert!(l.iter().all(|x| x.is_finite()), "{name}: non-finite output");
}

fn report(name: &str, secs: f64) {
    println!("{name:<34} {:>8.1} ms {:>9.0}× real time", secs * 1000.0, SECS as f64 / secs);
}

const BLOCK: usize = 128;

/// Stereo 1 kHz sine segments (seconds, dBFS peak).
fn tone(freq: f64, segs: &[(f64, f64)]) -> Vec<f32> {
    let mut v = Vec::new();
    let mut i = 0usize;
    for &(sec, db) in segs {
        let a = 10f64.powf(db / 20.0);
        for _ in 0..(sec * 48_000.0).round() as usize {
            v.push((a * (std::f64::consts::TAU * freq * i as f64 / 48_000.0).sin()) as f32);
            i += 1;
        }
    }
    v
}

/// Meter readings for the synthesisable EBU Tech 3341 / 3342 cases.
fn validation() {
    println!("\nEBU Tech 3341 integrated loudness (expected ±0.1 LU):");
    let cases: [(&str, &[(f64, f64)], f64); 5] = [
        ("1: −23 dBFS 20 s", &[(20.0, -23.0)], -23.0),
        ("2: −33 dBFS 20 s", &[(20.0, -33.0)], -33.0),
        ("3: −36/−23/−36", &[(10.0, -36.0), (60.0, -23.0), (10.0, -36.0)], -23.0),
        ("4: −72/−36/−23/−36/−72", &[(10.0, -72.0), (10.0, -36.0), (60.0, -23.0), (10.0, -36.0), (10.0, -72.0)], -23.0),
        ("5: −26/−20/−26", &[(20.0, -26.0), (20.1, -20.0), (20.0, -26.0)], -23.0),
    ];
    for (name, segs, want) in cases {
        let x = tone(1000.0, segs);
        let m = measure(&x, &x, FS);
        println!("  case {name:<26} {:>8.3} LUFS (want {want})", m.integrated);
    }
    let x = tone(1000.0, &[(10.0, -20.0)]);
    let z = vec![0.0; x.len()];
    println!("  1 kHz −20 dBFS both channels: {:.3} LUFS; one channel: {:.3} LUFS", measure(&x, &x, FS).integrated, measure(&x, &z, FS).integrated);
    let x = tone(997.0, &[(10.0, 0.0)]);
    let z = vec![0.0; x.len()];
    println!("  997 Hz 0 dBFS one channel: {:.3} LUFS (BS.1770: −3.01)", measure(&x, &z, FS).integrated);
    println!("EBU Tech 3342 loudness range (expected ±1 LU):");
    let lra: [(&[(f64, f64)], f64); 4] = [
        (&[(20.0, -20.0), (20.0, -30.0)], 10.0),
        (&[(20.0, -20.0), (20.0, -15.0)], 5.0),
        (&[(20.0, -40.0), (20.0, -20.0)], 20.0),
        (&[(20.0, -50.0), (20.0, -35.0), (20.0, -20.0), (20.0, -35.0), (20.0, -50.0)], 15.0),
    ];
    for (i, (segs, want)) in lra.iter().enumerate() {
        let x = tone(1000.0, segs);
        println!("  case {}: {:>6.2} LU (want {want})", i + 1, measure(&x, &x, FS).lra);
    }
    println!("EBU Tech 3341 true peak (expected +0.2/−0.4 dB):");
    for (i, (f, ph, db)) in [(0.25f64, 0.0f64, -6.0f64), (0.25, 45.0, -6.0), (1.0 / 6.0, 60.0, -6.0), (0.125, 67.5, -6.0), (0.25, 45.0, 0.0)].iter().enumerate() {
        let a = 10f64.powf(db / 20.0);
        let x: Vec<f32> =
            (0..48_000).map(|k| (a * (k as f64 / 2400.0).min(1.0) * (std::f64::consts::TAU * f * k as f64 + ph.to_radians()).sin()) as f32).collect();
        let m = measure(&x, &x, FS);
        println!("  case {}: {:>7.3} dBTP (want {db}), sample peak {:.2} dBFS", 15 + i, m.true_peak, m.sample_peak);
    }
}

fn main() {
    let src = mix();
    println!("60 s stereo @ 48 kHz, {BLOCK}-sample blocks, single thread\n");
    let eq: EqParams = serde_json::from_str(r#"{"low": -2, "lowFreq": 120, "mid": [[400, -3, 1.0]], "high": 2, "highFreq": 8000, "hp": 30, "lp": 18000}"#).unwrap();
    bench("eq (5 sections)", &src, &mut Eq::new(FS, &eq));
    bench("svf low-pass", &src, &mut Svf::new(FS, SvfMode::LowPass, 1200.0, 2.0));
    bench("compressor (peak)", &src, &mut Compressor::new(FS, &CompressorParams::default()));
    bench(
        "compressor (rms, sc-hp, lookahead)",
        &src,
        &mut Compressor::new(FS, &CompressorParams { detector: Detector::Rms, sc_hp: Some(100.0), lookahead: 2.0, ..Default::default() }),
    );
    let beats: Vec<f64> = (0..120).map(|i| i as f64 * 0.5).collect();
    bench("ducker (120 triggers)", &src, &mut Ducker::new(FS, &DuckerParams::default(), &beats));
    for preset in REVERB_PRESETS {
        bench(
            &format!("reverb {preset}"),
            &src,
            &mut Reverb::new(FS, &ReverbParams { preset: Some(preset.to_string()), ..Default::default() }),
        );
    }
    bench("delay (ping-pong 1/8.)", &src, &mut Delay::new(FS, &DelayParams { note: Some("1/8.".into()), ping_pong: true, ..Default::default() }));
    bench("chorus", &src, &mut Chorus::new(FS, &ChorusParams::default()));
    bench("ensemble", &src, &mut Chorus::new(FS, &ChorusParams { mode: ChorusMode::Ensemble, ..Default::default() }));
    for os in [1, 2, 4] {
        bench(&format!("saturator tape {os}×"), &src, &mut Saturator::new(FS, &SaturatorParams { oversample: os, ..Default::default() }));
    }
    bench("widener (mono below 120)", &src, &mut Widener::new(FS, &WidenerParams { width: 1.3, mono_below: Some(120.0) }));
    bench("transient shaper", &src, &mut TransientShaper::new(FS, &TransientParams::default()));
    bench("limiter (true peak, 5 ms)", &src, &mut Limiter::new(FS, &LimiterParams::default()));

    // Convolution: 3 s synthetic stereo IR.
    let t = Instant::now();
    let ir = Ir::synthetic(&SynthIrParams { decay: 2.7, length: Some(3.0), ..Default::default() }, 48_000);
    report("synthetic IR (3 s) generation", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let (cl, _cr) = convolve(&src.0, &src.1, &ir);
    report("convolve offline (3 s IR)", t.elapsed().as_secs_f64());
    assert!(cl.iter().all(|x| x.is_finite()));
    bench("convolver streaming (3 s IR, 512)", &src, &mut Convolver::new(&ir, 512));

    // Metering and mastering.
    let t = Instant::now();
    let m = measure(&src.0, &src.1, FS);
    report("loudness meter (with true peak)", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let _ = loudness::integrated_lufs(&src.0, &src.1, FS);
    report("integrated LUFS only", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let a = analyze(&src.0, &src.1, FS);
    report("analysis report", t.elapsed().as_secs_f64());
    let (mut l, mut r) = src.clone();
    let t = Instant::now();
    let rep = master(&mut l, &mut r, 48_000, -14.0, -1.0);
    report("master() to −14 LUFS / −1 dBTP", t.elapsed().as_secs_f64());
    println!(
        "\ninput  {:.2} LUFS, {:.2} dBTP; output {:.2} LUFS, {:.2} dBTP, LRA {:.1}; gain {:+.2} dB, glue {:.2} dB, limiter {:.2} dB, {} iteration(s)",
        m.integrated, m.true_peak, rep.output.integrated, rep.output.true_peak, rep.output.lra, rep.gain_db, rep.glue_reduction_db, rep.limiter_reduction_db, rep.iterations
    );
    println!("analysis: crest {:.1} dB, correlation {:.2}, bands {:?}", a.crest_db, a.correlation, a.bands);

    validation();

    // SFX.
    let t = Instant::now();
    let mut audio = 0.0;
    for name in SFX {
        let s = sfx(name, &SfxParams::default(), 48_000).unwrap();
        audio += s.duration(48_000) as f64;
    }
    let secs = t.elapsed().as_secs_f64();
    println!("\nall {} SFX: {:.1} ms for {:.1} s of audio ({:.0}× real time)", SFX.len(), secs * 1000.0, audio, audio / secs);
}
