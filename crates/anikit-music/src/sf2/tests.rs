//! Sampler correctness tests (synthetic fonts and the bundled bank).

use super::engine::SUB;
use super::interp::Interp;
use super::parse::gen;
use super::sampler::SoundFont;
use crate::voice::{NoteOn, Voice, BLOCK};
use std::sync::Arc;

/// A sample for the synthetic font builder.
pub struct TSample {
    pub data: Vec<i16>,
    pub rate: u32,
    pub root: u8,
    pub lp: (u32, u32),
}

/// An instrument zone for the builder.
pub struct TZone {
    pub keys: (u8, u8),
    pub vels: (u8, u8),
    pub sample: usize,
    pub gens: Vec<(u16, i16)>,
}

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], p: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(p.len() as u32).to_le_bytes());
    out.extend_from_slice(p);
    if p.len() & 1 == 1 {
        out.push(0);
    }
}

fn name20(s: &str) -> [u8; 20] {
    let mut n = [0u8; 20];
    n[..s.len().min(19)].copy_from_slice(&s.as_bytes()[..s.len().min(19)]);
    n
}

/// Build a one-preset SF2 (bank 0, program 0) with one instrument.
pub fn build_sf2(samples: &[TSample], zones: &[TZone]) -> Vec<u8> {
    let mut smpl = Vec::new();
    let mut shdr = Vec::new();
    let mut pos = 0u32;
    for (i, s) in samples.iter().enumerate() {
        for v in &s.data {
            smpl.extend_from_slice(&v.to_le_bytes());
        }
        smpl.extend(std::iter::repeat_n(0u8, 46 * 2));
        let start = pos;
        let end = pos + s.data.len() as u32;
        shdr.extend_from_slice(&name20(&format!("s{i}")));
        for v in [start, end, start + s.lp.0, start + s.lp.1, s.rate] {
            shdr.extend_from_slice(&v.to_le_bytes());
        }
        shdr.push(s.root);
        shdr.push(0);
        shdr.extend_from_slice(&0u16.to_le_bytes());
        shdr.extend_from_slice(&1u16.to_le_bytes());
        pos = end + 46;
    }
    shdr.extend_from_slice(&[0u8; 46]);

    let mut igen = Vec::new();
    let mut ibag = Vec::new();
    let mut ng = 0u16;
    for z in zones {
        ibag.extend_from_slice(&ng.to_le_bytes());
        ibag.extend_from_slice(&0u16.to_le_bytes());
        let mut g = vec![(43u16, [z.keys.0, z.keys.1]), (44u16, [z.vels.0, z.vels.1])];
        for &(op, v) in &z.gens {
            g.push((op, v.to_le_bytes()));
        }
        g.push((53, (z.sample as u16).to_le_bytes()));
        for (op, v) in g {
            igen.extend_from_slice(&op.to_le_bytes());
            igen.extend_from_slice(&v);
            ng += 1;
        }
    }
    ibag.extend_from_slice(&ng.to_le_bytes());
    ibag.extend_from_slice(&0u16.to_le_bytes());
    igen.extend_from_slice(&[0u8; 4]);
    let mut inst = Vec::new();
    inst.extend_from_slice(&name20("inst"));
    inst.extend_from_slice(&0u16.to_le_bytes());
    inst.extend_from_slice(&name20("EOI"));
    inst.extend_from_slice(&(zones.len() as u16).to_le_bytes());

    let mut phdr = Vec::new();
    for (name, bag) in [("preset", 0u16), ("EOP", 1)] {
        phdr.extend_from_slice(&name20(name));
        phdr.extend_from_slice(&0u16.to_le_bytes());
        phdr.extend_from_slice(&0u16.to_le_bytes());
        phdr.extend_from_slice(&bag.to_le_bytes());
        phdr.extend_from_slice(&[0u8; 12]);
    }
    let pbag = [0u8, 0, 0, 0, 1, 0, 0, 0];
    let pgen = [41u8, 0, 0, 0, 0, 0, 0, 0];

    let mut info = b"INFO".to_vec();
    chunk(&mut info, b"ifil", &[2, 0, 1, 0]);
    chunk(&mut info, b"INAM", b"test\0");
    let mut sdta = b"sdta".to_vec();
    chunk(&mut sdta, b"smpl", &smpl);
    let mut pdta = b"pdta".to_vec();
    chunk(&mut pdta, b"phdr", &phdr);
    chunk(&mut pdta, b"pbag", &pbag);
    chunk(&mut pdta, b"pmod", &[0u8; 10]);
    chunk(&mut pdta, b"pgen", &pgen);
    chunk(&mut pdta, b"inst", &inst);
    chunk(&mut pdta, b"ibag", &ibag);
    chunk(&mut pdta, b"imod", &[0u8; 10]);
    chunk(&mut pdta, b"igen", &igen);
    chunk(&mut pdta, b"shdr", &shdr);
    let mut body = b"sfbk".to_vec();
    chunk(&mut body, b"LIST", &info);
    chunk(&mut body, b"LIST", &sdta);
    chunk(&mut body, b"LIST", &pdta);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// A looped sine sample with an exact number of periods in the loop.
pub fn sine_sample(freq_cycles: u32, period: u32, rate: u32, root: u8, amp: f32) -> TSample {
    let lp_len = freq_cycles * period;
    let n = lp_len * 4;
    let data = (0..n)
        .map(|i| ((i as f64 * 2.0 * std::f64::consts::PI / period as f64).sin() * amp as f64 * 32767.0).round() as i16)
        .collect();
    TSample { data, rate, root, lp: (lp_len, 2 * lp_len) }
}

pub fn note(key: f32, vel: f32) -> NoteOn {
    NoteOn { key, vel, rate: 48_000.0, seed: 1, rr: 0, len: f32::INFINITY }
}

/// Render `secs` seconds (releasing at `release_at`), in blocks of `block`.
pub fn render(v: &mut dyn Voice, secs: f32, release_at: f32, block: usize) -> (Vec<f32>, Vec<f32>, bool) {
    let n = (secs * 48_000.0) as usize;
    let rel = (release_at * 48_000.0) as usize;
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    let mut off = 0;
    let mut alive = true;
    let mut released = false;
    while off < n && alive {
        if !released && off >= rel {
            v.release();
            released = true;
        }
        let m = block.min(n - off).min(if released { usize::MAX } else { rel - off }).max(1);
        alive = v.render(&mut l[off..off + m], &mut r[off..off + m]);
        off += m;
    }
    (l, r, alive)
}

/// Frequency of the strongest partial near `guess` (Hann window, golden-section refinement of the DTFT).
pub fn measure_freq(x: &[f32], guess: f64, rate: f64) -> f64 {
    let n = x.len();
    let w: Vec<f64> = (0..n).map(|i| x[i] as f64 * (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos())).collect();
    let mag = |f: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        let dw = 2.0 * std::f64::consts::PI * f / rate;
        let (c, s) = (dw.cos(), dw.sin());
        let (mut cr, mut ci) = (1.0f64, 0.0f64);
        for &v in &w {
            re += v * cr;
            im += v * ci;
            let t = cr * c - ci * s;
            ci = cr * s + ci * c;
            cr = t;
        }
        re * re + im * im
    };
    // coarse scan ±3 %
    let mut best = guess;
    let mut bm = 0.0;
    for k in -60..=60 {
        let f = guess * (1.0 + k as f64 * 0.0005);
        let m = mag(f);
        if m > bm {
            bm = m;
            best = f;
        }
    }
    let (mut a, mut b) = (best * 0.9995, best * 1.0005);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..40 {
        let c = b - g * (b - a);
        let d = a + g * (b - a);
        if mag(c) > mag(d) {
            b = d;
        } else {
            a = c;
        }
    }
    0.5 * (a + b)
}

fn cents(f: f64, r: f64) -> f64 {
    1200.0 * (f / r).log2()
}

fn sine_font() -> Arc<SoundFont> {
    // A 100-frame period at 44 kHz is exactly 440 Hz (root key 69).
    let s = sine_sample(20, 100, 44_000, 69, 0.5);
    let z = TZone { keys: (0, 127), vels: (0, 127), sample: 0, gens: vec![(gen::SAMPLE_MODES as u16, 1)] };
    SoundFont::from_bytes(build_sf2(&[s], &[z])).unwrap()
}

#[test]
fn sf2_pitch_a4_synthetic() {
    let f = sine_font();
    let inst = f.instrument(0, 0).unwrap();
    for interp in [Interp::Cubic, Interp::Sinc] {
        f.set_interp(interp);
        let mut v = inst.voice(&note(69.0, 1.0));
        let (l, _, _) = render(v.as_mut(), 1.5, 10.0, BLOCK);
        let hz = measure_freq(&l[24_000..], 440.0, 48_000.0);
        assert!(cents(hz, 440.0).abs() < 0.1, "{interp:?}: {hz} Hz");
        // A5 and a microtonal key
        let mut v = inst.voice(&note(81.25, 1.0));
        let (l, _, _) = render(v.as_mut(), 1.5, 10.0, BLOCK);
        let hz = measure_freq(&l[24_000..], 880.0 * 2f64.powf(0.25 / 12.0), 48_000.0);
        assert!(cents(hz, 880.0 * 2f64.powf(0.25 / 12.0)).abs() < 0.1, "{hz}");
    }
}

#[test]
fn sf2_pitch_a4_bundled_bank() {
    // A4 on the bundled bank's sine wave and flute must sound at 440 Hz ±1 cent.
    // For every instrument (the piano too, which GeneralUser stretch-tunes)
    // the output must match the raw sample's measured pitch transposed by the
    // bank's declared root key, tuning and correction, ±1 cent.
    let g = super::bank::gm();
    for (bank, prog, name) in [(8u16, 80u16, "sine"), (0, 73, "flute"), (0, 0, "piano")] {
        let p = g.sf.preset(bank, prog).unwrap();
        let r = p.regions.iter().find(|r| r.matches(69, 102)).unwrap();
        let h = &g.sf.samples[r.sample];
        let buf = g.sample(r.sample).unwrap();
        let raw = &buf.ch[0][super::engine::PAD..super::engine::PAD + buf.len];
        // Tile the loop (single-cycle waveforms are only a few hundred frames).
        let (ls, le) = (h.loop_start as usize, h.loop_end as usize);
        let mut sig: Vec<f32> = raw[..ls].to_vec();
        while sig.len() < h.rate as usize * 2 {
            sig.extend_from_slice(&raw[ls..le]);
        }
        let raw_hz = measure_freq(&sig[sig.len() / 4..], 440.0 * 2f64.powf((h.root as f64 - 69.0) / 12.0), h.rate as f64);
        let cents_total = h.correction as f64
            + r.gens[gen::FINE_TUNE] as f64
            + 100.0 * r.gens[gen::COARSE_TUNE] as f64
            + r.gens[gen::SCALE_TUNING] as f64 * (69.0 - h.root as f64);
        let want = raw_hz * 2f64.powf(cents_total / 1200.0);
        let inst = g.instrument(bank, prog).unwrap();
        let mut v = inst.voice(&note(69.0, 0.8));
        let (l, r, _) = render(v.as_mut(), 2.0, 10.0, BLOCK);
        let m: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a + b).collect();
        let hz = measure_freq(&m[9_600..], 440.0, 48_000.0);
        assert!(cents(hz, want).abs() < 1.0, "{name}: {hz} Hz vs {want} ({:.2} cents)", cents(hz, want));
        if name != "piano" {
            assert!(cents(hz, 440.0).abs() < 1.0, "{name}: {hz} Hz ({:.2} cents)", cents(hz, 440.0));
        }
    }
}

#[test]
fn sf2_loop_is_click_free() {
    let f = sine_font();
    let inst = f.instrument(0, 0).unwrap();
    for interp in [Interp::Cubic, Interp::Sinc] {
        f.set_interp(interp);
        // Pitched so the loop wraps at fractional positions.
        let mut v = inst.voice(&note(62.3, 1.0));
        let (l, _, _) = render(v.as_mut(), 3.0, 10.0, BLOCK);
        let w = 2.0 * std::f64::consts::PI * 440.0 * 2f64.powf((62.3 - 69.0) / 12.0) / 48_000.0;
        let peak = l[48_000..].iter().fold(0f32, |m, v| m.max(v.abs())) as f64;
        let bound = peak * w * w * 1.05;
        let worst = l[48_000..].windows(3).map(|s| (s[0] - 2.0 * s[1] + s[2]).abs() as f64).fold(0.0, f64::max);
        assert!(worst <= bound, "{interp:?}: second difference {worst} > {bound}");
    }
}

#[test]
fn sf2_release_ends_voice() {
    let s = sine_sample(20, 100, 44_000, 69, 0.5);
    // release 0.2 s (timecents = 1200·log2(0.2) ≈ −2786)
    let z = TZone { keys: (0, 127), vels: (0, 127), sample: 0, gens: vec![(54, 1), (38, -2786)] };
    let f = SoundFont::from_bytes(build_sf2(&[s], &[z])).unwrap();
    let mut v = f.instrument(0, 0).unwrap().voice(&note(60.0, 1.0));
    let (l, _, alive) = render(v.as_mut(), 1.0, 0.25, BLOCK);
    assert!(!alive, "voice still alive 0.75 s after a 0.2 s release");
    let tail = l[(0.46 * 48_000.0) as usize..].iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(tail < 1e-3, "{tail}");
    let held = l[(0.2 * 48_000.0) as usize..(0.24 * 48_000.0) as usize].iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(held > 0.2);
}

#[test]
fn sf2_deterministic_and_block_independent() {
    let inst = super::bank::gm_instrument(0);
    let run = |block: usize| {
        let mut v = inst.voice(&note(64.0, 0.7));
        render(v.as_mut(), 1.0, 0.6, block)
    };
    let a = run(BLOCK);
    let b = run(BLOCK);
    let c = run(37);
    assert!(a.0.iter().any(|v| v.abs() > 1e-3));
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    // control updates follow voice time, so any block split gives the same samples
    // (release lands on a SUB boundary in both cases here)
    assert_eq!((0.6 * 48_000.0) as usize % SUB, 0);
    assert_eq!(a.0, c.0);
}

#[test]
fn sf2_velocity_layers() {
    // soft layer: 440 Hz, loud layer: 660 Hz (same root key)
    let soft = sine_sample(20, 100, 44_000, 69, 0.5);
    let loud = TSample { root: 69, ..sine_sample(30, 100, 66_000, 69, 0.5) };
    let zones = [
        TZone { keys: (0, 127), vels: (0, 63), sample: 0, gens: vec![(54, 1)] },
        TZone { keys: (0, 127), vels: (64, 127), sample: 1, gens: vec![(54, 1)] },
    ];
    let f = SoundFont::from_bytes(build_sf2(&[soft, loud], &zones)).unwrap();
    let inst = f.instrument(0, 0).unwrap();
    for (vel, want) in [(0.3f32, 440.0), (0.49, 440.0), (0.51, 660.0), (1.0, 660.0)] {
        let mut v = inst.voice(&note(69.0, vel));
        let (l, _, _) = render(v.as_mut(), 0.5, 10.0, BLOCK);
        let hz = measure_freq(&l[4_800..], want, 48_000.0);
        assert!(cents(hz, want).abs() < 1.0, "vel {vel}: {hz}");
    }
}

#[test]
fn sf2_velocity_attenuation_follows_spec() {
    let f = sine_font();
    let inst = f.instrument(0, 0).unwrap();
    let rms = |vel: f32| {
        let mut v = inst.voice(&note(69.0, vel));
        let (l, _, _) = render(v.as_mut(), 0.5, 10.0, BLOCK);
        (l[4_800..].iter().map(|x| (x * x) as f64).sum::<f64>() / (l.len() - 4_800) as f64).sqrt()
    };
    // vel 64/127 → −40·log10(64/127) = 11.9 dB (+ the default vel→filter cut is
    // far above 440 Hz so it does not matter here).
    let db = 20.0 * (rms(1.0) / rms(64.0 / 127.0)).log10();
    assert!((db - 11.9).abs() < 0.3, "{db}");
}

#[test]
fn sf2_drum_kit_exclusive_hihat() {
    let kit = super::bank::gm_drums(0);
    let closed = kit.voice(&note(42.0, 0.8));
    let open = kit.voice(&note(46.0, 0.8));
    assert!(closed.exclusive() != 0);
    assert_eq!(closed.exclusive(), open.exclusive());
    // a one-shot drum ends on its own
    let mut v = kit.voice(&note(38.0, 0.9));
    let (l, _, alive) = render(v.as_mut(), 4.0, 0.1, BLOCK);
    assert!(l.iter().any(|v| v.abs() > 0.01));
    assert!(!alive, "snare should finish within 4 s");
}

#[test]
fn sf2_kill_fades_in_5ms() {
    let inst = super::bank::gm_instrument(48);
    let mut v = inst.voice(&note(60.0, 1.0));
    let (mut l, mut r) = (vec![0f32; 9_600], vec![0f32; 9_600]);
    for k in 0..75 {
        v.render(&mut l[k * 128..k * 128 + 128], &mut r[k * 128..k * 128 + 128]);
    }
    v.kill();
    let mut alive = true;
    let mut frames = 0;
    let mut buf = ([0f32; 128], [0f32; 128]);
    while alive && frames < 4_800 {
        alive = v.render(&mut buf.0, &mut buf.1);
        frames += 128;
    }
    assert!(!alive && frames <= 512, "{frames}");
}

#[test]
fn sf2_every_gm_preset_plays() {
    let g = super::bank::gm();
    g.preload();
    for (b, p, name) in g.presets() {
        let inst = g.instrument(b, p).unwrap();
        let key = if b >= 120 { 38.0 } else { 60.0 };
        let mut v = inst.voice(&note(key, 0.8));
        let (l, r, _) = render(v.as_mut(), 0.3, 0.2, BLOCK);
        let peak = l.iter().chain(r.iter()).fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak.is_finite() && peak < 8.0, "{name}: peak {peak}");
        if !matches!(name.as_str(), "Reverse Cymbal" | "Fl. Key Click" | "Helicopter" | "Applause" | "Seashore")
            && b < 120
        {
            assert!(peak > 1e-4, "{b}/{p} {name} is silent");
        }
    }
}

