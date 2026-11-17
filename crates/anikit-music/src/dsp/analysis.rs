//! Listening-proxy metrics for tests and QA: loudness, peaks, crest factor,
//! clipping, spectral balance, stereo correlation and DC offset, as one
//! serialisable [`Report`].

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::Serialize;

use super::loudness::measure;

/// Energy per band in dB relative to the total (all bands sum to 0 dB).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Bands {
    /// < 60 Hz.
    pub sub: f64,
    /// 60–250 Hz.
    pub low: f64,
    /// 250 Hz–2 kHz.
    pub low_mid: f64,
    /// 2–6 kHz.
    pub high_mid: f64,
    /// > 6 kHz.
    pub air: f64,
}

/// QA report for a stereo buffer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub duration: f64,
    pub integrated_lufs: f64,
    pub lra: f64,
    pub max_short_term_lufs: f64,
    pub true_peak_dbtp: f64,
    pub sample_peak_dbfs: f64,
    pub rms_dbfs: f64,
    /// Sample peak minus RMS (dB).
    pub crest_db: f64,
    /// Percentage of samples at or above full scale (|x| ≥ 0.9999).
    pub clipped_pct: f64,
    pub bands: Bands,
    /// Spectral centroid (Hz).
    pub centroid_hz: f64,
    /// Pearson correlation of L and R (1 = mono, 0 = unrelated, −1 = out of phase).
    pub correlation: f64,
    /// Mean of each channel.
    pub dc_offset: [f64; 2],
}

const EDGES: [f64; 4] = [60.0, 250.0, 2000.0, 6000.0];

/// Welch power spectrum of both channels (Hann, 8192-point, 50 % overlap):
/// returns (bin width Hz, power per bin summed over channels).
pub fn power_spectrum(l: &[f32], r: &[f32], rate: f32) -> (f64, Vec<f64>) {
    let n = 8192usize;
    let len = l.len().min(r.len());
    let fft = FftPlanner::<f32>::new().plan_fft_forward(n);
    let win: Vec<f32> = (0..n).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()).collect();
    let mut pow = vec![0.0f64; n / 2 + 1];
    let mut buf = vec![Complex::<f32>::default(); n];
    let mut scratch = vec![Complex::<f32>::default(); fft.get_inplace_scratch_len()];
    let hop = n / 2;
    let mut start = 0usize;
    loop {
        for (i, b) in buf.iter_mut().enumerate() {
            let j = start + i;
            *b = if j < len { Complex::new(l[j] * win[i], r[j] * win[i]) } else { Complex::default() };
        }
        fft.process_with_scratch(&mut buf, &mut scratch);
        for (k, p) in pow.iter_mut().enumerate() {
            let a = buf[k];
            let c = buf[(n - k) % n].conj();
            let xl = (a + c) * 0.5;
            let xr = (a - c) * 0.5;
            *p += (xl.norm_sqr() + xr.norm_sqr()) as f64;
        }
        start += hop;
        if start + hop >= len.max(1) {
            break;
        }
    }
    (rate as f64 / n as f64, pow)
}

/// Analyse a stereo buffer.
pub fn analyze(l: &[f32], r: &[f32], rate: f32) -> Report {
    let n = l.len().min(r.len());
    let (l, r) = (&l[..n], &r[..n]);
    let loud = measure(l, r, rate);
    let (mut sl, mut sr, mut slr, mut ml, mut mr, mut clip) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0usize);
    for (&a, &b) in l.iter().zip(r) {
        let (a, b) = (a as f64, b as f64);
        sl += a * a;
        sr += b * b;
        slr += a * b;
        ml += a;
        mr += b;
        clip += (a.abs() >= 0.9999) as usize + (b.abs() >= 0.9999) as usize;
    }
    let nn = n.max(1) as f64;
    let rms = ((sl + sr) / (2.0 * nn)).sqrt();
    let db = |x: f64| if x > 0.0 { 20.0 * x.log10() } else { -200.0 };
    let (bw, pow) = power_spectrum(l, r, rate);
    let mut band = [0.0f64; 5];
    let (mut total, mut moment) = (0.0f64, 0.0f64);
    for (k, p) in pow.iter().enumerate().skip(1) {
        let f = k as f64 * bw;
        let i = EDGES.iter().take_while(|e| f >= **e).count();
        band[i] += p;
        total += p;
        moment += p * f;
    }
    let rel = |e: f64| if total > 0.0 && e > 0.0 { 10.0 * (e / total).log10() } else { -200.0 };
    Report {
        duration: n as f64 / rate as f64,
        integrated_lufs: loud.integrated,
        lra: loud.lra,
        max_short_term_lufs: loud.max_short_term,
        true_peak_dbtp: loud.true_peak,
        sample_peak_dbfs: loud.sample_peak,
        rms_dbfs: db(rms),
        crest_db: if rms > 0.0 { loud.sample_peak - db(rms) } else { 0.0 },
        clipped_pct: 100.0 * clip as f64 / (2.0 * nn),
        bands: Bands { sub: rel(band[0]), low: rel(band[1]), low_mid: rel(band[2]), high_mid: rel(band[3]), air: rel(band[4]) },
        centroid_hz: if total > 0.0 { moment / total } else { 0.0 },
        correlation: if sl > 0.0 && sr > 0.0 { slr / (sl * sr).sqrt() } else { 0.0 },
        dc_offset: [ml / nn, mr / nn],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_report() {
        let fs = 48_000.0;
        let a = 0.5f32;
        let l: Vec<f32> = (0..96_000).map(|i| a * (std::f64::consts::TAU * 1000.0 * i as f64 / fs as f64).sin() as f32).collect();
        let rep = analyze(&l, &l, fs);
        assert!((rep.crest_db - 3.01).abs() < 0.05, "{}", rep.crest_db);
        assert!(rep.bands.low_mid > -0.01 && rep.bands.air < -60.0, "{:?}", rep.bands);
        assert!((rep.correlation - 1.0).abs() < 1e-9);
        assert!((rep.centroid_hz - 1000.0).abs() < 20.0);
        assert!(rep.dc_offset[0].abs() < 1e-4);
        assert_eq!(rep.clipped_pct, 0.0);
        let inv: Vec<f32> = l.iter().map(|v| -v).collect();
        assert!((analyze(&l, &inv, fs).correlation + 1.0).abs() < 1e-9);
        let json = serde_json::to_string(&rep).unwrap();
        assert!(json.contains("\"integratedLufs\"") && json.contains("\"highMid\""));
    }

    #[test]
    fn bands_split_correctly() {
        let fs = 48_000.0;
        // Equal-amplitude tones at 40, 120, 1000, 4000, 10000 Hz → each band ≈ −7 dB.
        let x: Vec<f32> = (0..96_000)
            .map(|i| {
                [40.0, 120.0, 1000.0, 4000.0, 10_000.0].iter().map(|f| (std::f64::consts::TAU * f * i as f64 / fs as f64).sin() as f32 * 0.1).sum()
            })
            .collect();
        let b = analyze(&x, &x, fs).bands;
        for v in [b.sub, b.low, b.low_mid, b.high_mid, b.air] {
            assert!((v + 6.99).abs() < 0.2, "{b:?}");
        }
    }
}
