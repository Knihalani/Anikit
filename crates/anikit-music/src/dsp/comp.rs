//! Dynamics: a stereo-linked feed-forward [`Compressor`] (soft knee, peak or
//! RMS detection, external sidechain, look-ahead, parallel `mix`), a
//! beat-triggered [`Ducker`] (the classic sidechain pump, sample-accurate,
//! look-ahead capable) and a key-driven [`KeyDucker`] for voice-over.

use serde::Deserialize;

use super::biquad::{Biquad, FilterKind};
use super::util::{db_to_gain, fast_db_to_gain, fast_gain_to_db, flush, DelayLine};
use super::Processor;

/// Level detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Detector {
    /// Instantaneous peak of `max(|l|, |r|)`.
    #[default]
    Peak,
    /// Mean-square over `rmsMs` of the summed channels.
    Rms,
}

/// Compressor parameters (times in ms, levels in dB).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct CompressorParams {
    pub threshold: f32,
    pub ratio: f32,
    /// Knee width in dB (0 = hard knee).
    pub knee: f32,
    pub attack: f32,
    pub release: f32,
    /// Make-up gain in dB.
    pub makeup: f32,
    pub detector: Detector,
    /// RMS window (ms) for `detector: "rms"`.
    pub rms_ms: f32,
    /// Look-ahead (ms): the audio is delayed so gain reduction starts early.
    pub lookahead: f32,
    /// High-pass on the detector (Hz), so bass does not pump the whole mix.
    pub sc_hp: Option<f32>,
    /// Wet/dry for parallel ("New York") compression: 1 = fully compressed.
    pub mix: f32,
}

impl Default for CompressorParams {
    fn default() -> Self {
        CompressorParams {
            threshold: -18.0,
            ratio: 4.0,
            knee: 6.0,
            attack: 10.0,
            release: 120.0,
            makeup: 0.0,
            detector: Detector::Peak,
            rms_ms: 10.0,
            lookahead: 0.0,
            sc_hp: None,
            mix: 1.0,
        }
    }
}

fn coef(ms: f32, rate: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * rate)).exp()
    }
}

/// Feed-forward stereo-linked compressor with log-domain smoothing.
#[derive(Debug, Clone)]
pub struct Compressor {
    p: CompressorParams,
    att: f32,
    rel: f32,
    rms_c: f32,
    ms: f32,
    gr: f32,
    makeup: f32,
    sc_hp: Option<Biquad>,
    dl: DelayLine,
    dr: DelayLine,
    look: usize,
    /// Deepest gain reduction seen since the last [`Compressor::take_max_reduction`] (dB, ≤ 0).
    max_gr: f32,
}

impl Compressor {
    pub fn new(rate: f32, p: &CompressorParams) -> Compressor {
        let look = (p.lookahead.max(0.0) * 0.001 * rate).round() as usize;
        Compressor {
            att: coef(p.attack, rate),
            rel: coef(p.release, rate),
            rms_c: coef(p.rms_ms.max(0.1), rate),
            ms: 0.0,
            gr: 0.0,
            makeup: db_to_gain(p.makeup),
            sc_hp: p.sc_hp.filter(|f| *f > 0.0).map(|f| Biquad::new(FilterKind::HighPass, rate, f, 0.7071, 0.0)),
            dl: DelayLine::new(look + 1),
            dr: DelayLine::new(look + 1),
            look,
            max_gr: 0.0,
            p: p.clone(),
        }
    }

    /// Static curve: output level (dB) for a steady input level (dB).
    pub fn static_curve(&self, x: f32) -> f32 {
        static_curve(x, self.p.threshold, self.p.ratio, self.p.knee)
    }

    /// Current gain reduction in dB (≤ 0).
    pub fn reduction(&self) -> f32 {
        self.gr
    }

    /// Deepest reduction since the last call (dB, ≤ 0); resets the tracker.
    pub fn take_max_reduction(&mut self) -> f32 {
        std::mem::replace(&mut self.max_gr, 0.0)
    }

    /// Compress `l`/`r`, detecting on `sl`/`sr` (an external sidechain).
    pub fn process_sidechain(&mut self, l: &mut [f32], r: &mut [f32], sl: &[f32], sr: &[f32]) {
        self.run(l, r, Some((sl, sr)));
    }

    fn run(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = l.len().min(r.len());
        let (t, ratio, knee) = (self.p.threshold, self.p.ratio, self.p.knee);
        let rms = self.p.detector == Detector::Rms;
        let mix = self.p.mix.clamp(0.0, 1.0);
        let mut bl = [0.0f32; 128];
        let mut br = [0.0f32; 128];
        let mut start = 0;
        while start < n {
            let m = (n - start).min(128);
            match key {
                Some((sl, sr)) => {
                    bl[..m].copy_from_slice(&sl[start..start + m]);
                    br[..m].copy_from_slice(&sr[start..start + m]);
                }
                None => {
                    bl[..m].copy_from_slice(&l[start..start + m]);
                    br[..m].copy_from_slice(&r[start..start + m]);
                }
            }
            if let Some(hp) = self.sc_hp.as_mut() {
                hp.process(&mut bl[..m], &mut br[..m]);
            }
            for i in 0..m {
                let (a, b) = (bl[i], br[i]);
                let level = if rms {
                    self.ms = flush(self.rms_c * self.ms + (1.0 - self.rms_c) * 0.5 * (a * a + b * b));
                    0.5 * fast_gain_to_db(self.ms)
                } else {
                    fast_gain_to_db(a.abs().max(b.abs()))
                };
                let target = static_curve(level, t, ratio, knee) - level;
                let c = if target < self.gr { self.att } else { self.rel };
                self.gr = target + c * (self.gr - target);
                self.max_gr = self.max_gr.min(self.gr);
                let g = fast_db_to_gain(self.gr) * self.makeup;
                let g = mix * g + (1.0 - mix);
                let j = start + i;
                let (xl, xr) = if self.look > 0 {
                    self.dl.push(l[j]);
                    self.dr.push(r[j]);
                    (self.dl.tap(self.look), self.dr.tap(self.look))
                } else {
                    (l[j], r[j])
                };
                l[j] = xl * g;
                r[j] = xr * g;
            }
            start += m;
        }
    }
}

/// Soft-knee static curve (Giannoulis, Massberg & Reiss 2012).
#[inline(always)]
pub fn static_curve(x: f32, t: f32, ratio: f32, knee: f32) -> f32 {
    let slope = 1.0 / ratio.max(1.0) - 1.0;
    let d = x - t;
    let w = knee.max(1e-6);
    let soft = x + slope * (d + 0.5 * w) * (d + 0.5 * w) / (2.0 * w);
    let hard = t + d / ratio.max(1.0);
    if 2.0 * d < -w {
        x
    } else if 2.0 * d > w {
        hard
    } else {
        soft
    }
}

impl Processor for Compressor {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.run(l, r, None);
    }
    fn reset(&mut self) {
        self.ms = 0.0;
        self.gr = 0.0;
        self.max_gr = 0.0;
        self.dl.reset();
        self.dr.reset();
        if let Some(h) = self.sc_hp.as_mut() {
            h.reset();
        }
    }
    fn latency(&self) -> usize {
        self.look
    }
}

/// Beat-triggered ducker parameters (times in ms).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct DuckerParams {
    /// Gain at full duck, in dB (negative).
    pub depth: f32,
    /// Ramp down time (raised cosine).
    pub attack: f32,
    /// Time held at full depth.
    pub hold: f32,
    /// Recovery time.
    pub release: f32,
    /// Start ducking this long *before* each trigger so the music is already
    /// down when the kick lands (0 = duck starts at the trigger).
    pub lookahead: f32,
    /// Release shape: 1 = linear in dB; >1 recovers fast then eases (compressor-like);
    /// <1 stays down then swells back.
    pub curve: f32,
}

impl Default for DuckerParams {
    fn default() -> Self {
        DuckerParams { depth: -9.0, attack: 4.0, hold: 20.0, release: 200.0, lookahead: 0.0, curve: 1.5 }
    }
}

/// Sidechain-style pumping driven by trigger times (e.g. kick hits), without
/// needing the kick audio. Sample-accurate; envelopes of overlapping triggers
/// combine by maximum depth so the gain is continuous.
#[derive(Debug, Clone)]
pub struct Ducker {
    starts: Vec<f64>,
    att: f64,
    hold: f64,
    rel: f64,
    depth: f32,
    curve: f32,
    pos: u64,
    first: usize,
}

impl Ducker {
    /// `triggers` are in seconds from the start of the stream.
    pub fn new(rate: f32, p: &DuckerParams, triggers: &[f64]) -> Ducker {
        let r = rate as f64;
        let ms = |v: f32| (v.max(0.0) as f64) * 0.001 * r;
        let att = ms(p.attack).max(1.0);
        let look = ms(p.lookahead);
        let mut starts: Vec<f64> = triggers.iter().map(|t| t * r - look).collect();
        starts.sort_by(|a, b| a.total_cmp(b));
        Ducker {
            starts,
            att,
            hold: ms(p.hold) + (look - att).max(0.0),
            rel: ms(p.release).max(1.0),
            depth: p.depth.min(0.0),
            curve: p.curve.max(0.05),
            pos: 0,
            first: 0,
        }
    }

    /// Jump to an absolute sample position.
    pub fn seek(&mut self, pos: u64) {
        self.pos = pos;
        self.first = 0;
    }

    /// Duck amount 0..1 for an envelope that started `u` samples ago.
    #[inline]
    fn amount(&self, u: f64) -> f32 {
        if u < 0.0 {
            0.0
        } else if u < self.att {
            (0.5 - 0.5 * (std::f64::consts::PI * u / self.att).cos()) as f32
        } else if u < self.att + self.hold {
            1.0
        } else {
            let v = ((u - self.att - self.hold) / self.rel).min(1.0) as f32;
            (1.0 - v).powf(self.curve)
        }
    }

    fn span(&self) -> f64 {
        self.att + self.hold + self.rel
    }

    /// Gain (linear) at absolute sample `pos`.
    pub fn gain_at(&self, pos: u64) -> f32 {
        let p = pos as f64;
        let mut d = 0.0f32;
        for &s in &self.starts {
            if s > p {
                break;
            }
            d = d.max(self.amount(p - s));
        }
        fast_db_to_gain(self.depth * d)
    }
}

impl Processor for Ducker {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let span = self.span();
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let p = self.pos as f64;
            while self.first < self.starts.len() && self.starts[self.first] + span <= p {
                self.first += 1;
            }
            let mut d = 0.0f32;
            let mut i = self.first;
            while i < self.starts.len() && self.starts[i] <= p {
                d = d.max(self.amount(p - self.starts[i]));
                i += 1;
            }
            let g = fast_db_to_gain(self.depth * d);
            *a *= g;
            *b *= g;
            self.pos += 1;
        }
    }
    fn reset(&mut self) {
        self.seek(0);
    }
}

/// Voice-over ducking keyed from another signal's level.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct KeyDuckParams {
    /// Key level (dBFS RMS) where ducking starts.
    pub threshold: f32,
    /// Key level span (dB) above threshold over which ducking reaches full depth.
    pub range: f32,
    /// Gain at full duck (dB, negative).
    pub depth: f32,
    /// Times in ms.
    pub attack: f32,
    pub hold: f32,
    pub release: f32,
    /// Offline only ([`key_duck_gains`]): start ducking this early.
    pub lookahead: f32,
}

impl Default for KeyDuckParams {
    fn default() -> Self {
        KeyDuckParams { threshold: -45.0, range: 8.0, depth: -10.0, attack: 60.0, hold: 250.0, release: 500.0, lookahead: 80.0 }
    }
}

/// Streaming key ducker: `process_key(l, r, key_l, key_r)`.
#[derive(Debug, Clone)]
pub struct KeyDucker {
    p: KeyDuckParams,
    rms_c: f32,
    att: f32,
    rel: f32,
    hold: u32,
    ms: f32,
    d: f32,
    held: u32,
}

impl KeyDucker {
    pub fn new(rate: f32, p: &KeyDuckParams) -> KeyDucker {
        KeyDucker {
            rms_c: coef(15.0, rate),
            att: coef(p.attack, rate),
            rel: coef(p.release, rate),
            hold: (p.hold.max(0.0) * 0.001 * rate) as u32,
            ms: 0.0,
            d: 0.0,
            held: 0,
            p: p.clone(),
        }
    }

    #[inline]
    fn step(&mut self, a: f32, b: f32) -> f32 {
        self.ms = flush(self.rms_c * self.ms + (1.0 - self.rms_c) * 0.5 * (a * a + b * b));
        let lvl = 0.5 * fast_gain_to_db(self.ms);
        let u = ((lvl - self.p.threshold) / self.p.range.max(0.1)).clamp(0.0, 1.0);
        let target = u * u * (3.0 - 2.0 * u);
        if target >= self.d {
            self.d = target + self.att * (self.d - target);
            self.held = self.hold;
        } else if self.held > 0 {
            self.held -= 1;
        } else {
            self.d = target + self.rel * (self.d - target);
        }
        fast_db_to_gain(self.p.depth.min(0.0) * self.d)
    }

    /// Duck `l`/`r` by the level of `kl`/`kr`.
    pub fn process_key(&mut self, l: &mut [f32], r: &mut [f32], kl: &[f32], kr: &[f32]) {
        for (((a, b), &x), &y) in l.iter_mut().zip(r.iter_mut()).zip(kl).zip(kr) {
            let g = self.step(x, y);
            *a *= g;
            *b *= g;
        }
    }

    pub fn reset(&mut self) {
        self.ms = 0.0;
        self.d = 0.0;
        self.held = 0;
    }
}

/// Offline key ducking: the per-sample gain curve for a whole key signal,
/// shifted earlier by `lookahead` so the music dips just before speech starts.
pub fn key_duck_gains(kl: &[f32], kr: &[f32], rate: f32, p: &KeyDuckParams) -> Vec<f32> {
    let n = kl.len().min(kr.len());
    let mut kd = KeyDucker::new(rate, p);
    let mut g: Vec<f32> = (0..n).map(|i| kd.step(kl[i], kr[i])).collect();
    let look = ((p.lookahead.max(0.0) * 0.001 * rate) as usize).min(n);
    // Dip early (look-ahead) but never release early: min of the curve and
    // its shifted copy.
    for i in 0..n.saturating_sub(look) {
        g[i] = g[i].min(g[i + look]);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_curve_points() {
        let c = Compressor::new(48_000.0, &CompressorParams { threshold: -20.0, ratio: 4.0, knee: 0.0, ..Default::default() });
        assert!((c.static_curve(-30.0) + 30.0).abs() < 1e-4);
        assert!((c.static_curve(-20.0) + 20.0).abs() < 1e-3);
        assert!((c.static_curve(0.0) + 15.0).abs() < 1e-4);
        let k = Compressor::new(48_000.0, &CompressorParams { threshold: -20.0, ratio: 4.0, knee: 10.0, ..Default::default() });
        // Knee midpoint: x = T → T + (1/R − 1)(W/2)²/(2W) = −20 − 0.75·25/20.
        assert!((k.static_curve(-20.0) - (-20.0 - 0.9375)).abs() < 1e-4);
        assert!((k.static_curve(-26.0) + 26.0).abs() < 1e-4);
        assert!((k.static_curve(-10.0) + 17.5).abs() < 1e-3);
    }

    #[test]
    fn steady_state_matches_static_curve() {
        // A 1 kHz sine with peak −6 dBFS through a peak compressor, T −20, R 4:
        // the steady-state peak should sit near −20 + 14/4 = −16.5 dBFS.
        let p = CompressorParams { threshold: -20.0, ratio: 4.0, knee: 0.0, attack: 1.0, release: 500.0, ..Default::default() };
        let mut c = Compressor::new(48_000.0, &p);
        let n = 96_000;
        let a = db_to_gain(-6.0);
        let mut l: Vec<f32> = (0..n).map(|i| a * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin()).collect();
        let mut r = l.clone();
        c.process(&mut l, &mut r);
        let pk = l[n - 4800..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((super::super::util::gain_to_db(pk) + 16.5).abs() < 0.3, "{}", super::super::util::gain_to_db(pk));
        // RMS detector: the sine's RMS is −9 dBFS → −20 + 11/4 = −17.25 dB RMS, peak +3 dB.
        let mut c = Compressor::new(48_000.0, &CompressorParams { detector: Detector::Rms, rms_ms: 50.0, ..p });
        let mut l: Vec<f32> = (0..n).map(|i| a * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin()).collect();
        let mut r = l.clone();
        c.process(&mut l, &mut r);
        let pk = l[n - 4800..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((super::super::util::gain_to_db(pk) + 14.24).abs() < 0.4, "{}", super::super::util::gain_to_db(pk));
    }

    #[test]
    fn parallel_mix_and_sidechain() {
        let p = CompressorParams { threshold: -40.0, ratio: 20.0, knee: 0.0, attack: 0.1, mix: 0.5, ..Default::default() };
        let mut c = Compressor::new(48_000.0, &p);
        let mut l = vec![0.5f32; 4800];
        let mut r = l.clone();
        let quiet = vec![0.0f32; 4800];
        c.process_sidechain(&mut l, &mut r, &quiet, &quiet);
        assert!((l[4799] - 0.5).abs() < 1e-4, "silent key leaves signal alone");
        let loud = vec![1.0f32; 4800];
        let mut l = vec![0.5f32; 4800];
        let mut r = l.clone();
        c.process_sidechain(&mut l, &mut r, &loud, &loud);
        // Half dry (0.25) plus a heavily reduced wet.
        assert!(l[4799] > 0.25 && l[4799] < 0.26, "{}", l[4799]);
    }

    #[test]
    fn lookahead_latency_and_alignment() {
        let p = CompressorParams { lookahead: 5.0, ..Default::default() };
        let mut c = Compressor::new(48_000.0, &p);
        assert_eq!(c.latency(), 240);
        let mut l = vec![0.0f32; 1000];
        l[100] = 0.01;
        let mut r = l.clone();
        super::super::process_aligned(&mut c, &mut l, &mut r);
        assert!((l[100] - 0.01).abs() < 1e-6);
    }

    #[test]
    fn ducker_shape() {
        let p = DuckerParams { depth: -12.0, attack: 5.0, hold: 10.0, release: 100.0, lookahead: 0.0, curve: 1.0 };
        let mut d = Ducker::new(48_000.0, &p, &[0.5, 1.0]);
        let n = 72_000;
        let mut l = vec![1.0f32; n];
        let mut r = l.clone();
        // Process in odd-sized blocks to check block independence.
        let mut s = 0;
        while s < n {
            let m = (n - s).min(97);
            d.process(&mut l[s..s + m], &mut r[s..s + m]);
            s += m;
        }
        let db = |i: usize| super::super::util::gain_to_db(l[i]);
        assert!(db(23_999).abs() < 1e-3);
        assert!((db(24_000 + 240 + 100) + 12.0).abs() < 0.01, "full depth during hold");
        assert!((db(24_000 + 240 + 480 + 2400) + 6.0).abs() < 0.05, "linear-in-dB release midpoint");
        assert!(db(24_000 + 240 + 480 + 4800 + 10).abs() < 0.01);
        assert!((db(48_000 + 400) + 12.0).abs() < 0.01);
        for i in 0..n {
            assert_eq!(l[i], d.gain_at(i as u64), "gain_at agrees at {i}");
        }
        // Look-ahead: fully down at the trigger.
        let p = DuckerParams { lookahead: 5.0, ..p };
        let d = Ducker::new(48_000.0, &p, &[0.5]);
        assert!((super::super::util::gain_to_db(d.gain_at(24_000)) + 12.0).abs() < 0.01);
        assert!(super::super::util::gain_to_db(d.gain_at(24_000 - 241)).abs() < 0.01);
    }

    #[test]
    fn key_ducker() {
        let rate = 48_000.0;
        let n = 96_000;
        let mut key = vec![0.0f32; n];
        for (i, k) in key.iter_mut().enumerate().take(72_000).skip(24_000) {
            *k = 0.3 * (i as f32 * 0.05).sin();
        }
        let g = key_duck_gains(&key, &key, rate, &KeyDuckParams::default());
        assert!(super::super::util::gain_to_db(g[10_000]).abs() < 0.01);
        assert!((super::super::util::gain_to_db(g[48_000]) + 10.0).abs() < 0.1);
        // Look-ahead: already dipping before the speech starts.
        assert!(g[24_000] < 0.9);
        assert!(g.iter().all(|x| x.is_finite() && *x > 0.0 && *x <= 1.0));
    }
}
