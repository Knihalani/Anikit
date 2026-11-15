//! Offline audio analysis for syncing animation to music.
//!
//! [`analyze`] runs a short-time FFT over mono samples and returns a loudness
//! envelope, a spectral-flux onset curve, onset times, a tempo estimate and
//! a beat grid fitted to the onsets. Animations and video cuts can then be
//! placed on beats instead of guessed by ear.

use crate::math::{floorf, sqrtf};
use alloc::vec;
use alloc::vec::Vec;

const FRAME: usize = 1024;
const HOP: usize = 512;

#[derive(Debug, Default, Clone)]
pub struct Analysis {
    /// Seconds between consecutive envelope / flux values.
    pub hop: f32,
    /// RMS loudness per hop, normalised to 0…1.
    pub envelope: Vec<f32>,
    /// Spectral flux (onset strength) per hop, normalised to 0…1.
    pub flux: Vec<f32>,
    /// Onset times in seconds.
    pub onsets: Vec<f32>,
    /// Tempo in beats per minute (0 when no steady pulse was found).
    pub bpm: f32,
    /// Beat times in seconds.
    pub beats: Vec<f32>,
}

/// In-place iterative radix-2 FFT (`re`, `im` of power-of-two length).
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -core::f32::consts::TAU / len as f32;
        let (wr, wi) = (libm::cosf(ang), libm::sinf(ang));
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (i + k, i + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
            i += len;
        }
        len <<= 1;
    }
}

fn normalise(v: &mut [f32]) {
    let m = v.iter().cloned().fold(0.0f32, f32::max);
    if m > 0.0 {
        v.iter_mut().for_each(|x| *x /= m);
    }
}

/// Analyse mono `samples` at `rate` Hz.
pub fn analyze(samples: &[f32], rate: f32) -> Analysis {
    let hop_s = HOP as f32 / rate;
    let frames = if samples.len() >= FRAME { (samples.len() - FRAME) / HOP + 1 } else { 0 };
    let window: Vec<f32> = (0..FRAME).map(|i| 0.5 - 0.5 * libm::cosf(core::f32::consts::TAU * i as f32 / FRAME as f32)).collect();
    let (mut re, mut im) = (vec![0.0f32; FRAME], vec![0.0f32; FRAME]);
    let mut prev = vec![0.0f32; FRAME / 2];
    let mut envelope = Vec::with_capacity(frames);
    let mut flux = Vec::with_capacity(frames);
    for f in 0..frames {
        let s = &samples[f * HOP..f * HOP + FRAME];
        let mut e = 0.0;
        for i in 0..FRAME {
            re[i] = s[i] * window[i];
            im[i] = 0.0;
            e += s[i] * s[i];
        }
        envelope.push(sqrtf(e / FRAME as f32));
        fft(&mut re, &mut im);
        let mut fl = 0.0;
        for k in 1..FRAME / 2 {
            let mag = libm::logf(1.0 + 100.0 * sqrtf(re[k] * re[k] + im[k] * im[k]));
            let d = mag - prev[k];
            if d > 0.0 {
                fl += d;
            }
            prev[k] = mag;
        }
        flux.push(if f == 0 { 0.0 } else { fl });
    }
    normalise(&mut envelope);
    normalise(&mut flux);

    // Onsets: local maxima above an adaptive threshold.
    let mut onsets = Vec::new();
    let w = 8usize;
    let mut last = -1.0f32;
    for i in 1..flux.len().saturating_sub(1) {
        let (a, b) = (i.saturating_sub(w), (i + w).min(flux.len() - 1));
        let mean = flux[a..=b].iter().sum::<f32>() / (b - a + 1) as f32;
        let t = i as f32 * hop_s;
        if flux[i] > mean + 0.08 && flux[i] >= flux[i - 1] && flux[i] > flux[i + 1] && t - last > 0.07 {
            onsets.push(t);
            last = t;
        }
    }

    // Tempo: autocorrelation of the flux over 60–200 BPM.
    let mut bpm = 0.0;
    let mut beats = Vec::new();
    let (lo, hi) = ((60.0 / 200.0 / hop_s) as usize, (1.0 / hop_s) as usize);
    if flux.len() > hi * 2 && lo > 0 {
        let mut best = (0usize, 0.0f32);
        for lag in lo..=hi {
            let mut s = 0.0;
            for i in lag..flux.len() {
                s += flux[i] * flux[i - lag];
            }
            // Mild preference for 90–150 BPM, where most music sits.
            let b = 60.0 / (lag as f32 * hop_s);
            let wgt = 1.0 / (1.0 + ((b - 120.0) / 60.0) * ((b - 120.0) / 60.0));
            let s = s / (flux.len() - lag) as f32 * wgt;
            if s > best.1 {
                best = (lag, s);
            }
        }
        if best.0 > 0 {
            let period = best.0 as f32;
            // Phase: the offset whose comb collects the most onset strength.
            let mut phase = (0.0f32, -1.0f32);
            let steps = period as usize;
            for o in 0..steps {
                let mut s = 0.0;
                let mut x = o as f32;
                while (x as usize) < flux.len() {
                    s += flux[x as usize];
                    x += period;
                }
                if s > phase.1 {
                    phase = (o as f32, s);
                }
            }
            // Refine period and phase with a least-squares fit to the onsets
            // near the grid (an integer-hop period drifts over long tracks).
            let (mut p_s, mut t0) = (period * hop_s, phase.0 * hop_s);
            for _ in 0..3 {
                let (mut n, mut sk, mut st, mut skk, mut skt) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
                for &t in &onsets {
                    let k = libm::roundf((t - t0) / p_s);
                    let err = t - (t0 + k * p_s);
                    if (if err < 0.0 { -err } else { err }) < p_s * 0.25 {
                        n += 1.0;
                        sk += k;
                        st += t;
                        skk += k * k;
                        skt += k * t;
                    }
                }
                let den = n * skk - sk * sk;
                if n < 4.0 || den.abs() < 1e-6 {
                    break;
                }
                p_s = (n * skt - sk * st) / den;
                t0 = (st - p_s * sk) / n;
            }
            bpm = 60.0 / p_s;
            let end = flux.len() as f32 * hop_s;
            let mut x = t0 - floorf(t0 / p_s) * p_s;
            while x < end {
                beats.push(x);
                x += p_s;
            }
        }
    }
    Analysis { hop: hop_s, envelope, flux, onsets, bpm, beats }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_finds_a_sine() {
        let n = 256;
        let mut re: Vec<f32> = (0..n).map(|i| libm::sinf(core::f32::consts::TAU * 8.0 * i as f32 / n as f32)).collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let mags: Vec<f32> = (0..n / 2).map(|k| sqrtf(re[k] * re[k] + im[k] * im[k])).collect();
        let peak = mags.iter().enumerate().fold((0, 0.0f32), |a, (i, &m)| if m > a.1 { (i, m) } else { a }).0;
        assert_eq!(peak, 8);
    }

    #[test]
    fn detects_a_120_bpm_click_track() {
        let rate = 22050.0;
        let secs = 8.0;
        let n = (rate * secs) as usize;
        let mut s = vec![0.0f32; n];
        // A short noisy click every 0.5 s (120 BPM), starting at 0.25 s.
        let mut seed = 1u32;
        let mut k = 0.25;
        while k < secs {
            let at = (k * rate) as usize;
            for i in 0..400.min(n - at) {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
                s[at + i] = noise * (1.0 - i as f32 / 400.0);
            }
            k += 0.5;
        }
        let a = analyze(&s, rate);
        assert!((a.bpm - 120.0).abs() < 4.0, "bpm {}", a.bpm);
        assert!(a.onsets.len() >= 14, "{}", a.onsets.len());
        // Beats land within ~1 hop of the clicks.
        let near = a.beats.iter().filter(|b| ((**b - 0.25) / 0.5 - libm::roundf((**b - 0.25) / 0.5)).abs() * 0.5 < 0.04).count();
        assert!(near * 10 >= a.beats.len() * 8, "{near}/{}", a.beats.len());
    }
}
