//! Rendering and analysis helpers shared by the instrument tests and the
//! demo/level examples.
#![allow(dead_code)]

use anikit_music::{Instrument, NoteOn, BLOCK, RATE};
use rustfft::{num_complex::Complex, FftPlanner};
use std::f64::consts::PI;

pub fn note(key: f32, vel: f32, seed: u64) -> NoteOn {
    NoteOn {
        key,
        vel,
        rate: RATE as f32,
        seed,
        rr: 0,
        len: f32::INFINITY,
    }
}

/// Rendered note: left/right, and the frame at which the voice ended (if it did).
pub struct Render {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub ended: Option<usize>,
}

/// Render one note for `secs` seconds, releasing at `release` seconds.
/// Blocks are of irregular size to exercise partial blocks.
pub fn render(inst: &dyn Instrument, n: &NoteOn, secs: f32, release: f32) -> Render {
    let total = (secs * RATE as f32) as usize;
    let rel = (release * RATE as f32) as usize;
    let mut l = vec![0.0f32; total];
    let mut r = vec![0.0f32; total];
    let mut v = inst.voice(n);
    let mut pos = 0;
    let mut released = false;
    let mut ended = None;
    let mut i = 0usize;
    while pos < total {
        let mut m = if i % 7 == 3 { 37 } else { BLOCK };
        m = m.min(total - pos);
        if !released && pos + m > rel {
            m = (rel - pos).max(1).min(m);
        }
        if !v.render(&mut l[pos..pos + m], &mut r[pos..pos + m]) {
            ended = Some(pos + m);
            break;
        }
        pos += m;
        if !released && pos >= rel {
            v.release();
            released = true;
        }
        i += 1;
    }
    Render { l, r, ended }
}

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

pub fn dbfs(x: f32) -> f32 {
    20.0 * x.max(1e-12).log10()
}

/// Mono sum.
pub fn mono(r: &Render) -> Vec<f32> {
    r.l.iter().zip(&r.r).map(|(a, b)| 0.5 * (a + b)).collect()
}

/// Power spectrum of a Hann-windowed signal zero-padded to at least `min_n`
/// points; returns (power per bin, Hz per bin).
pub fn spectrum(x: &[f32], min_n: usize) -> (Vec<f64>, f64) {
    spectrum_win(x, min_n, false)
}

/// Like [`spectrum`]; `bh` selects a 4-term Blackman-Harris window (−92 dB
/// side lobes) for measurements that need a deep floor (aliasing).
pub fn spectrum_win(x: &[f32], min_n: usize, bh: bool) -> (Vec<f64>, f64) {
    let n = x.len().max(min_n).next_power_of_two();
    let len = x.len() as f64;
    let mut buf: Vec<Complex<f64>> = (0..n)
        .map(|i| {
            if i < x.len() {
                let p = 2.0 * PI * i as f64 / len;
                let w = if bh {
                    0.35875 - 0.48829 * p.cos() + 0.14128 * (2.0 * p).cos()
                        - 0.01168 * (3.0 * p).cos()
                } else {
                    0.5 - 0.5 * p.cos()
                };
                Complex::new(x[i] as f64 * w, 0.0)
            } else {
                Complex::new(0.0, 0.0)
            }
        })
        .collect();
    FftPlanner::new().plan_fft_forward(n).process(&mut buf);
    (
        buf[..n / 2].iter().map(|c| c.norm_sqr()).collect(),
        RATE as f64 / n as f64,
    )
}

/// Pitch of the spectral cluster around `expected` Hz (±`span` cents):
/// (peak cents, power-weighted centroid cents) relative to `expected`.
pub fn pitch_cents(x: &[f32], expected: f64, span: f64) -> (f64, f64) {
    // ≥ 8 bins per cent-span-width: fine enough for parabolic peak interpolation.
    let (p, hz) = spectrum(x, 1 << 18);
    let lo = ((expected * (-span / 1200.0).exp2()) / hz).floor() as usize;
    let hi = ((expected * (span / 1200.0).exp2()) / hz).ceil() as usize;
    let (mut bi, mut bp) = (lo, 0.0);
    for (k, &v) in p.iter().enumerate().take(hi + 1).skip(lo) {
        if v > bp {
            bp = v;
            bi = k;
        }
    }
    // Parabolic interpolation on log power.
    let (a, b, c) = (
        p[bi - 1].max(1e-300).ln(),
        bp.max(1e-300).ln(),
        p[bi + 1].max(1e-300).ln(),
    );
    let d = 0.5 * (a - c) / (a - 2.0 * b + c);
    let fpk = (bi as f64 + d) * hz;
    let floor = bp * 0.01;
    let (mut sw, mut sc) = (0.0, 0.0);
    for (k, &v) in p.iter().enumerate().take(hi + 1).skip(lo) {
        if v > floor {
            let c = 1200.0 * (k as f64 * hz / expected).log2();
            sw += v - floor;
            sc += (v - floor) * c;
        }
    }
    (1200.0 * (fpk / expected).log2(), sc / sw.max(1e-300))
}

/// Centroid pitch error (cents) of a held note's nominal partial (`ratio` × key
/// frequency), averaged over `seeds` so detuned unison lanes, drift and
/// vibrato phases average out the way the ear averages them.
pub fn measure_pitch_seeds(inst: &dyn Instrument, key: f32, ratio: f64, seeds: &[u64]) -> f64 {
    let mut acc = 0.0;
    for &s in seeds {
        let r = render(inst, &note(key, 0.8, s), 2.8, 2.7);
        let m = mono(&r);
        let f = 440.0 * ((key as f64 - 69.0) / 12.0).exp2() * ratio;
        let start = (0.15 * RATE as f32) as usize;
        let seg = &m[start..(start + (2.5 * RATE as f32) as usize).min(m.len())];
        acc += pitch_cents(seg, f, 80.0).1;
    }
    acc / seeds.len() as f64
}

/// [`measure_pitch_seeds`] over six seeds.
pub fn measure_pitch(inst: &dyn Instrument, key: f32, ratio: f64) -> f64 {
    measure_pitch_seeds(inst, key, ratio, &[7, 11, 23, 42, 77, 99])
}

/// Write a 32-bit float stereo WAV.
pub fn write_wav(path: &std::path::Path, l: &[f32], r: &[f32]) -> std::io::Result<()> {
    let n = l.len();
    let mut out = Vec::with_capacity(44 + n * 8);
    let data_len = (n * 8) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 8).to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..n {
        out.extend_from_slice(&l[i].to_le_bytes());
        out.extend_from_slice(&r[i].to_le_bytes());
    }
    std::fs::write(path, out)
}

/// Spectral energy (fraction) above `hz`.
pub fn energy_above(x: &[f32], hz: f64) -> f64 {
    let (p, bin) = spectrum(x, 0);
    let tot: f64 = p.iter().sum();
    let hi: f64 = p
        .iter()
        .enumerate()
        .filter(|(k, _)| *k as f64 * bin > hz)
        .map(|(_, v)| v)
        .sum();
    hi / tot.max(1e-300)
}

/// Spectral centroid (Hz).
pub fn centroid(x: &[f32]) -> f64 {
    let (p, bin) = spectrum(x, 0);
    let tot: f64 = p.iter().sum();
    p.iter()
        .enumerate()
        .map(|(k, v)| k as f64 * bin * v)
        .sum::<f64>()
        / tot.max(1e-300)
}

/// Energy (relative to the total) more than `rel_tol` × f away from every
/// harmonic of `f0`, between `lo` and `hi` Hz — i.e. aliasing for a periodic
/// waveform. Returned in dB.
pub fn alias_db(x: &[f32], f0: f64, rel_tol: f64, lo: f64, hi: f64) -> f64 {
    let (p, bin) = spectrum_win(x, 0, true);
    let tot: f64 = p.iter().sum();
    let mut off = 0.0;
    for (k, &v) in p.iter().enumerate().skip(1) {
        let f = k as f64 * bin;
        if f < lo || f > hi {
            continue;
        }
        let h = (f / f0).round().max(1.0);
        if (f - h * f0).abs() > (rel_tol * h * f0).max(4.0 * RATE as f64 / x.len() as f64) {
            off += v;
        }
    }
    10.0 * (off / tot.max(1e-300)).max(1e-30).log10()
}
