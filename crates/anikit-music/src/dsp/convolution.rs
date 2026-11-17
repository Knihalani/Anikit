//! Convolution reverb: a uniformly-partitioned overlap-save FFT convolver
//! ([`Convolver`], stereo packed into one complex FFT), an offline
//! whole-buffer [`convolve`], impulse responses ([`Ir`]) loaded from
//! WAV / FLAC / Ogg (symphonia, resampled to the engine rate) or synthesised
//! ([`Ir::synthetic`]: exponentially decaying noise with frequency-dependent
//! RT60, build-up, early reflections and adjustable width).

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::Deserialize;

use super::Processor;
use crate::voice::Rng;

type C32 = Complex<f32>;

/// A stereo impulse response.
#[derive(Debug, Clone, PartialEq)]
pub struct Ir {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub rate: u32,
}

/// Synthetic impulse-response parameters (times in seconds unless noted).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SynthIrParams {
    /// Mid-band RT60 (s).
    pub decay: f32,
    /// Low / high RT60 as multiples of `decay`.
    pub low_ratio: f32,
    pub high_ratio: f32,
    /// Crossovers (Hz).
    pub low_cross: f32,
    pub high_cross: f32,
    /// Pre-delay (ms).
    pub predelay: f32,
    /// Early-reflection level (0..1) and spread (room size 0.1..2).
    pub early: f32,
    pub size: f32,
    /// Stereo width 0..1 (1 = fully decorrelated channels).
    pub width: f32,
    /// Spectral roll-off of the whole response (Hz).
    pub high_cut: f32,
    /// Length (s); default 1.1 × the longest RT60, at most 12 s.
    pub length: Option<f32>,
    pub seed: u64,
}

impl Default for SynthIrParams {
    fn default() -> Self {
        SynthIrParams {
            decay: 2.2,
            low_ratio: 1.2,
            high_ratio: 0.45,
            low_cross: 300.0,
            high_cross: 4000.0,
            predelay: 15.0,
            early: 0.5,
            size: 1.0,
            width: 1.0,
            high_cut: 14_000.0,
            length: None,
            seed: 1,
        }
    }
}

fn gauss(rng: &mut Rng) -> f32 {
    // Box-Muller (deterministic).
    let u = rng.f32().max(1e-12);
    let v = rng.f32();
    (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
}

impl Ir {
    /// Length in samples.
    pub fn len(&self) -> usize {
        self.l.len().max(self.r.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Scale to unit mean energy per channel (white noise in → same power out).
    pub fn normalize(&mut self) {
        let e: f64 = self.l.iter().chain(self.r.iter()).map(|x| (*x as f64).powi(2)).sum::<f64>() * 0.5;
        if e > 0.0 {
            let g = (1.0 / e.sqrt()) as f32;
            self.l.iter_mut().chain(self.r.iter_mut()).for_each(|x| *x *= g);
        }
    }

    /// Resample to `rate` (band-limited).
    pub fn resampled(&self, rate: u32) -> Ir {
        Ir { l: super::util::resample(&self.l, self.rate, rate), r: super::util::resample(&self.r, self.rate, rate), rate }
    }

    /// Exponentially decaying filtered noise with frequency-dependent decay,
    /// built frame by frame in the frequency domain (Hann, 75 % overlap), plus
    /// a diffuse build-up and sparse stereo early reflections. Normalised.
    pub fn synthetic(p: &SynthIrParams, rate: u32) -> Ir {
        let fs = rate as f32;
        let decay = p.decay.clamp(0.05, 30.0);
        let longest = decay * p.low_ratio.max(p.high_ratio).max(1.0);
        let len_s = p.length.unwrap_or(longest * 1.1).clamp(0.05, 12.0);
        let pre = (p.predelay.max(0.0) * 0.001 * fs) as usize;
        let n = pre + (len_s * fs) as usize;
        let mut out = [vec![0.0f32; n], vec![0.0f32; n]];
        let fft_n = 1024;
        let hop = fft_n / 4;
        let ifft = FftPlanner::<f32>::new().plan_fft_inverse(fft_n);
        let win: Vec<f32> = (0..fft_n).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / fft_n as f32).cos()).collect();
        // Per-bin decay rate (nepers/s) and static spectral shape.
        let bins = fft_n / 2 + 1;
        let mut rate_k = vec![0.0f32; bins];
        let mut shape = vec![0.0f32; bins];
        for k in 0..bins {
            let f = (k as f32 * fs / fft_n as f32).max(1.0);
            let wl = 1.0 / (1.0 + (f / p.low_cross.max(10.0)).powi(2));
            let wh = 1.0 / (1.0 + (p.high_cross.max(100.0) / f).powi(2));
            let rt = decay * (wl * p.low_ratio.max(0.05).ln() + wh * p.high_ratio.max(0.05).ln()).exp();
            rate_k[k] = 6.907_755 / rt;
            let hc = 1.0 / (1.0 + (f / p.high_cut.max(1000.0)).powi(4)).sqrt();
            let lc = (f / 35.0).powi(2) / (1.0 + (f / 35.0).powi(2));
            shape[k] = hc * lc;
        }
        let build = 0.012 + 0.02 * p.size.clamp(0.1, 2.0);
        let mut rng = Rng::new(p.seed ^ 0x1A2B_3C4D);
        let mut buf = vec![C32::new(0.0, 0.0); fft_n];
        let mut scratch = vec![C32::new(0.0, 0.0); ifft.get_inplace_scratch_len()];
        // Hann² summed at 75 % overlap = 1.5; frames are independent, so variances add.
        let norm = 1.0 / (1.5f32.sqrt() * (fft_n as f32).sqrt());
        let frames = (n - pre) / hop + 4;
        for ch in out.iter_mut() {
            for m in 0..frames {
                let start = m as isize * hop as isize - fft_n as isize / 2;
                let t = (m * hop) as f32 / fs;
                for k in 0..bins {
                    let a = shape[k] * (-rate_k[k] * t).exp();
                    let z = C32::new(gauss(&mut rng), gauss(&mut rng)) * a;
                    buf[k] = z;
                    if k > 0 && k < fft_n / 2 {
                        buf[fft_n - k] = z.conj();
                    }
                }
                buf[0].im = 0.0;
                buf[fft_n / 2].im = 0.0;
                ifft.process_with_scratch(&mut buf, &mut scratch);
                for (i, v) in buf.iter().enumerate() {
                    let j = start + i as isize;
                    if j >= 0 && (j as usize) < n - pre {
                        let tt = j as f32 / fs;
                        let env = 1.0 - (-tt / build).exp();
                        ch[pre + j as usize] += v.re * win[i] * norm * env;
                    }
                }
            }
        }
        // Early reflections: sparse, panned, decaying, slightly smoothed.
        if p.early > 0.0 {
            let span = 0.08 * p.size.clamp(0.1, 2.0).sqrt();
            let taps = 20;
            let tail_rms = (out[0][pre..(pre + (0.05 * fs) as usize).min(n)].iter().map(|x| x * x).sum::<f32>()
                / (0.05 * fs))
                .sqrt();
            for k in 0..taps {
                let u = (k as f32 + rng.range(0.0, 1.0)) / taps as f32;
                let t = 0.002 + span * u * u.sqrt();
                let i = pre + (t * fs) as usize;
                if i + 2 >= n {
                    continue;
                }
                let g = p.early * 6.0 * tail_rms.max(1e-4) * (1.0 - 0.7 * u) * if rng.f32() < 0.5 { -1.0 } else { 1.0 };
                let (gl, gr) = crate::voice::pan_gains(rng.range(-0.9, 0.9));
                for (d, w) in [0.25f32, 0.5, 0.25].iter().enumerate() {
                    out[0][i + d] += g * gl * w * 2.0;
                    out[1][i + d] += g * gr * w * 2.0;
                }
            }
        }
        // Width (mid/side).
        let w = p.width.clamp(0.0, 1.0);
        if w < 1.0 {
            let (a, b) = out.split_at_mut(1);
            for (x, y) in a[0].iter_mut().zip(b[0].iter_mut()) {
                let m = 0.5 * (*x + *y);
                let s = 0.5 * (*x - *y) * w;
                *x = m + s;
                *y = m - s;
            }
        }
        let [l, r] = out;
        let mut ir = Ir { l, r, rate };
        ir.normalize();
        ir
    }

    /// Load an IR from WAV / FLAC / Ogg Vorbis bytes and resample to `rate`.
    /// Mono files feed both channels; extra channels are ignored.
    pub fn from_bytes(bytes: &[u8], rate: u32) -> Result<Ir, String> {
        use symphonia::core::audio::SampleBuffer;
        use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
        use symphonia::core::errors::Error;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;

        let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
        let probed = symphonia::default::get_probe()
            .format(&Hint::new(), mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| format!("impulse response: unsupported or corrupt audio ({e})"))?;
        let mut format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("impulse response: no audio track")?
            .clone();
        let src_rate = track.codec_params.sample_rate.ok_or("impulse response: unknown sample rate")?;
        let mut dec = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| format!("impulse response: {e}"))?;
        let (mut l, mut r) = (Vec::new(), Vec::new());
        loop {
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(Error::ResetRequired) => break,
                Err(e) => return Err(format!("impulse response: {e}")),
            };
            if packet.track_id() != track.id {
                continue;
            }
            let decoded = match dec.decode(&packet) {
                Ok(d) => d,
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(format!("impulse response: {e}")),
            };
            let spec = *decoded.spec();
            let ch = spec.channels.count().max(1);
            let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            sb.copy_interleaved_ref(decoded);
            for f in sb.samples().chunks_exact(ch) {
                l.push(f[0]);
                r.push(if ch > 1 { f[1] } else { f[0] });
            }
        }
        if l.is_empty() {
            return Err("impulse response: file has no samples".into());
        }
        let ir = Ir { l, r, rate: src_rate };
        Ok(if src_rate == rate { ir } else { ir.resampled(rate) })
    }

    /// Load an IR file (see [`Ir::from_bytes`]).
    pub fn from_file(path: impl AsRef<std::path::Path>, rate: u32) -> Result<Ir, String> {
        let p = path.as_ref();
        let bytes = std::fs::read(p).map_err(|e| format!("impulse response {}: {e}", p.display()))?;
        Ir::from_bytes(&bytes, rate)
    }
}

/// Uniformly-partitioned overlap-save convolver. Both channels share one
/// complex FFT (left in the real part, right in the imaginary part). Latency
/// is one block ([`Processor::latency`]); [`convolve`] removes it offline.
pub struct Convolver {
    b: usize,
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    /// Partition spectra, bins 0..=b.
    hl: Vec<Vec<C32>>,
    hr: Vec<Vec<C32>>,
    /// Frequency-domain delay line of input spectra.
    xl: Vec<Vec<C32>>,
    xr: Vec<Vec<C32>>,
    head: usize,
    /// Last 2b input samples (packed).
    inp: Vec<C32>,
    fill: usize,
    buf: Vec<C32>,
    acc_l: Vec<C32>,
    acc_r: Vec<C32>,
    scratch: Vec<C32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    wet: f32,
    dry: f32,
}

impl std::fmt::Debug for Convolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Convolver").field("block", &self.b).field("partitions", &self.hl.len()).finish()
    }
}

impl Convolver {
    /// `block` is rounded up to a power of two (≥ 32). Output is wet only
    /// (see [`Convolver::set_mix`]).
    pub fn new(ir: &Ir, block: usize) -> Convolver {
        let b = block.max(32).next_power_of_two();
        let n = 2 * b;
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n);
        let ifft = planner.plan_fft_inverse(n);
        let parts = ir.len().div_ceil(b).max(1);
        let scratch_len = fft.get_inplace_scratch_len().max(ifft.get_inplace_scratch_len());
        let mut scratch = vec![C32::default(); scratch_len];
        let mut hl = Vec::with_capacity(parts);
        let mut hr = Vec::with_capacity(parts);
        let mut buf = vec![C32::default(); n];
        for p in 0..parts {
            buf.iter_mut().for_each(|c| *c = C32::default());
            for i in 0..b {
                let j = p * b + i;
                buf[i] = C32::new(ir.l.get(j).copied().unwrap_or(0.0), ir.r.get(j).copied().unwrap_or(0.0));
            }
            fft.process_with_scratch(&mut buf, &mut scratch);
            let (sl, sr) = split(&buf, b);
            hl.push(sl);
            hr.push(sr);
        }
        Convolver {
            b,
            fft,
            ifft,
            xl: vec![vec![C32::default(); b + 1]; parts],
            xr: vec![vec![C32::default(); b + 1]; parts],
            hl,
            hr,
            head: 0,
            inp: vec![C32::default(); n],
            fill: 0,
            buf,
            acc_l: vec![C32::default(); b + 1],
            acc_r: vec![C32::default(); b + 1],
            scratch,
            out_l: vec![0.0; b],
            out_r: vec![0.0; b],
            wet: 1.0,
            dry: 0.0,
        }
    }

    /// Wet and dry gains (the dry path is delayed to match the latency).
    pub fn set_mix(&mut self, wet: f32, dry: f32) {
        self.wet = wet;
        self.dry = dry;
    }

    /// Block size (= latency).
    pub fn block(&self) -> usize {
        self.b
    }

    fn run_block(&mut self) {
        let b = self.b;
        let n = 2 * b;
        self.buf.copy_from_slice(&self.inp);
        self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);
        let parts = self.hl.len();
        self.head = if self.head == 0 { parts - 1 } else { self.head - 1 };
        {
            let (xl, xr) = (&mut self.xl[self.head], &mut self.xr[self.head]);
            for k in 0..=b {
                let a = self.buf[k];
                let c = self.buf[(n - k) % n].conj();
                xl[k] = (a + c) * 0.5;
                // (a − c) / 2i
                let d = (a - c) * 0.5;
                xr[k] = C32::new(d.im, -d.re);
            }
        }
        self.acc_l.iter_mut().for_each(|c| *c = C32::default());
        self.acc_r.iter_mut().for_each(|c| *c = C32::default());
        for p in 0..parts {
            let slot = (self.head + p) % parts;
            mac(&mut self.acc_l, &self.xl[slot], &self.hl[p]);
            mac(&mut self.acc_r, &self.xr[slot], &self.hr[p]);
        }
        // Y = Yl + i·Yr over the full spectrum (Yl, Yr Hermitian).
        for k in 0..=b {
            let (yl, yr) = (self.acc_l[k], self.acc_r[k]);
            self.buf[k] = yl + C32::new(-yr.im, yr.re);
            if k > 0 && k < b {
                let (cl, cr) = (yl.conj(), yr.conj());
                self.buf[n - k] = cl + C32::new(-cr.im, cr.re);
            }
        }
        self.ifft.process_with_scratch(&mut self.buf, &mut self.scratch);
        let s = 1.0 / n as f32;
        for i in 0..b {
            let v = self.buf[b + i];
            self.out_l[i] = v.re * s;
            self.out_r[i] = v.im * s;
        }
        // Slide the input window.
        self.inp.copy_within(b.., 0);
    }
}

#[inline]
fn mac(acc: &mut [C32], x: &[C32], h: &[C32]) {
    for ((a, x), h) in acc.iter_mut().zip(x).zip(h) {
        *a += x * h;
    }
}

/// Split the spectrum of `l + i·r` into the half spectra of `l` and `r`.
fn split(buf: &[C32], b: usize) -> (Vec<C32>, Vec<C32>) {
    let n = 2 * b;
    let mut sl = vec![C32::default(); b + 1];
    let mut sr = vec![C32::default(); b + 1];
    for k in 0..=b {
        let a = buf[k];
        let c = buf[(n - k) % n].conj();
        sl[k] = (a + c) * 0.5;
        let d = (a - c) * 0.5;
        sr[k] = C32::new(d.im, -d.re);
    }
    (sl, sr)
}

impl Processor for Convolver {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let b = self.b;
        for (a, c) in l.iter_mut().zip(r.iter_mut()) {
            let (xl, xr) = (*a, *c);
            // Dry is delayed by one block: it is the sample leaving the window.
            let d = self.inp[self.fill];
            self.inp[b + self.fill] = C32::new(xl, xr);
            *a = self.out_l[self.fill] * self.wet + d.re * self.dry;
            *c = self.out_r[self.fill] * self.wet + d.im * self.dry;
            self.fill += 1;
            if self.fill == b {
                self.fill = 0;
                self.run_block();
            }
        }
    }
    fn reset(&mut self) {
        for v in self.xl.iter_mut().chain(self.xr.iter_mut()) {
            v.iter_mut().for_each(|c| *c = C32::default());
        }
        self.inp.iter_mut().for_each(|c| *c = C32::default());
        self.out_l.iter_mut().for_each(|x| *x = 0.0);
        self.out_r.iter_mut().for_each(|x| *x = 0.0);
        self.fill = 0;
        self.head = 0;
    }
    fn latency(&self) -> usize {
        self.b
    }
}

/// Offline stereo convolution of whole buffers (left with `ir.l`, right with
/// `ir.r`). Returns the full result, `len + ir.len() − 1` samples. The block
/// size is chosen for throughput.
pub fn convolve(l: &[f32], r: &[f32], ir: &Ir) -> (Vec<f32>, Vec<f32>) {
    let n = l.len().min(r.len());
    if n == 0 || ir.is_empty() {
        return (vec![0.0; n], vec![0.0; n]);
    }
    let total = n + ir.len() - 1;
    let block = (ir.len() / 8).next_power_of_two().clamp(256, 16_384);
    let mut c = Convolver::new(ir, block);
    let mut ol = vec![0.0f32; total];
    let mut or = vec![0.0f32; total];
    ol[..n].copy_from_slice(&l[..n]);
    or[..n].copy_from_slice(&r[..n]);
    super::process_aligned(&mut c, &mut ol, &mut or);
    (ol, or)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct(x: &[f32], h: &[f32]) -> Vec<f32> {
        let mut y = vec![0.0f32; x.len() + h.len() - 1];
        for (i, xi) in x.iter().enumerate() {
            for (j, hj) in h.iter().enumerate() {
                y[i + j] += xi * hj;
            }
        }
        y
    }

    #[test]
    fn matches_direct_convolution() {
        let mut rng = Rng::new(7);
        let x: Vec<f32> = (0..3000).map(|_| rng.bi()).collect();
        let y: Vec<f32> = (0..3000).map(|_| rng.bi()).collect();
        let ir = Ir { l: (0..1000).map(|i| rng.bi() * (-(i as f32) / 200.0).exp()).collect(), r: (0..700).map(|_| rng.bi() * 0.1).collect(), rate: 48_000 };
        let (ol, or) = convolve(&x, &y, &ir);
        let mut rr = ir.r.clone();
        rr.resize(1000, 0.0);
        let (dl, dr) = (direct(&x, &ir.l), direct(&y, &rr));
        assert_eq!(ol.len(), dl.len());
        let err = ol.iter().zip(&dl).chain(or.iter().zip(&dr)).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(err < 1e-3, "max err {err}");
        // Streaming with odd block lengths gives the same samples (delayed by one block).
        let mut c = Convolver::new(&ir, 64);
        let (mut sl, mut sr) = (x.clone(), y.clone());
        let mut s = 0;
        while s < sl.len() {
            let m = (sl.len() - s).min(37);
            c.process(&mut sl[s..s + m], &mut sr[s..s + m]);
            s += m;
        }
        for i in 64..3000 {
            assert!((sl[i] - dl[i - 64]).abs() < 1e-3);
        }
    }

    #[test]
    fn synthetic_ir_decays_as_specified() {
        let p = SynthIrParams { decay: 1.5, low_ratio: 1.0, high_ratio: 1.0, early: 0.0, predelay: 0.0, ..Default::default() };
        let ir = Ir::synthetic(&p, 48_000);
        assert!(ir.l.iter().chain(ir.r.iter()).all(|x| x.is_finite()));
        // Energy in 100 ms windows at 0.2 s and 0.7 s: 0.5 s apart → 20 dB at RT60 1.5 s.
        let e = |a: usize| ir.l[a..a + 4800].iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
        let drop = 10.0 * (e(9_600) / e(33_600)).log10();
        assert!((drop - 20.0).abs() < 2.5, "drop {drop}");
        // Decorrelated channels.
        let c: f64 = ir.l.iter().zip(&ir.r).map(|(a, b)| *a as f64 * *b as f64).sum();
        assert!(c.abs() < 0.1);
        // Deterministic.
        assert_eq!(ir, Ir::synthetic(&p, 48_000));
    }

    #[test]
    fn loads_wav() {
        // Minimal 16-bit stereo WAV at 44.1 kHz.
        let frames = 441;
        let mut bytes = Vec::new();
        let data_len = frames * 4;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&44_100u32.to_le_bytes());
        bytes.extend_from_slice(&(44_100u32 * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
        for i in 0..frames {
            let v = if i == 10 { 16_384i16 } else { 0 };
            bytes.extend_from_slice(&v.to_le_bytes());
            bytes.extend_from_slice(&(-v).to_le_bytes());
        }
        let ir = Ir::from_bytes(&bytes, 48_000).unwrap();
        assert_eq!(ir.rate, 48_000);
        assert_eq!(ir.len(), 480);
        let pk = ir.l.iter().enumerate().fold((0, 0.0f32), |m, (i, v)| if *v > m.1 { (i, *v) } else { m });
        assert!((pk.0 as i32 - 11).abs() <= 1 && (pk.1 - 0.5).abs() < 0.1, "{pk:?}");
        assert!(ir.r.iter().zip(&ir.l).all(|(a, b)| (a + b).abs() < 1e-6));
        assert!(Ir::from_bytes(b"nope", 48_000).is_err());
    }
}
