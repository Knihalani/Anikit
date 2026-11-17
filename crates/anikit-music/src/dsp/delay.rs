//! Tempo-syncable stereo / ping-pong delay with filtered (and optionally
//! saturated) feedback and gentle tape-style modulation.

use serde::Deserialize;

use super::biquad::{Biquad, FilterKind};
use super::util::{fast_tanh, flush, DelayLine};
use super::Processor;

/// Seconds for a note value at `bpm` (4/4: a whole note is four beats).
/// Accepts `"1/4"`, `"1/8"`, `"3/16"`, `"1"`, dotted `"1/8."` or `"1/8d"`,
/// triplet `"1/8t"`.
pub fn note_seconds(note: &str, bpm: f32) -> Result<f32, String> {
    let s = note.trim().to_ascii_lowercase();
    let (body, mult) = if let Some(b) = s.strip_suffix('.').or_else(|| s.strip_suffix('d')) {
        (b, 1.5)
    } else if let Some(b) = s.strip_suffix('t') {
        (b, 2.0 / 3.0)
    } else {
        (s.as_str(), 1.0)
    };
    let (num, den) = match body.split_once('/') {
        Some((a, b)) => (a.trim().parse::<f32>(), b.trim().parse::<f32>()),
        None => (body.parse::<f32>(), Ok(1.0)),
    };
    match (num, den) {
        (Ok(n), Ok(d)) if n > 0.0 && d > 0.0 && bpm > 0.0 => Ok(n / d * 4.0 * 60.0 / bpm * mult),
        _ => Err(format!("invalid note value {note:?}: use e.g. \"1/4\", \"1/8.\" (dotted), \"1/8t\" (triplet)")),
    }
}

/// Delay parameters.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct DelayParams {
    /// Delay time in seconds (overrides `note`).
    pub time: Option<f32>,
    /// Tempo-synced note value, e.g. `"1/8."`.
    pub note: Option<String>,
    pub bpm: f32,
    /// Feedback 0..0.98.
    pub feedback: f32,
    pub wet: f32,
    pub dry: f32,
    /// Ping-pong: the (mono-summed) input bounces L → R → L.
    pub ping_pong: bool,
    /// Extra right-channel delay in ms (stereo mode), for width.
    pub spread: f32,
    /// Feedback low-pass (Hz): each repeat gets darker.
    pub high_cut: f32,
    /// Feedback high-pass (Hz): repeats stay out of the bass.
    pub low_cut: f32,
    /// Tape-style wow depth (ms) and rate (Hz).
    pub mod_depth: f32,
    pub mod_rate: f32,
    /// Saturation in the feedback loop (0 = clean).
    pub drive: f32,
}

impl Default for DelayParams {
    fn default() -> Self {
        DelayParams {
            time: None,
            note: None,
            bpm: 120.0,
            feedback: 0.35,
            wet: 0.3,
            dry: 1.0,
            ping_pong: false,
            spread: 0.0,
            high_cut: 6000.0,
            low_cut: 150.0,
            mod_depth: 0.12,
            mod_rate: 0.6,
            drive: 0.0,
        }
    }
}

impl DelayParams {
    /// Resolved delay time in seconds (`time`, else `note` at `bpm`, else 1/8).
    pub fn seconds(&self) -> Result<f32, String> {
        match (self.time, &self.note) {
            (Some(t), _) if t > 0.0 => Ok(t),
            (Some(t), _) => Err(format!("delay time must be positive, got {t}")),
            (None, Some(n)) => note_seconds(n, self.bpm),
            (None, None) => note_seconds("1/8", self.bpm),
        }
    }
}

/// Stereo / ping-pong feedback delay.
#[derive(Debug, Clone)]
pub struct Delay {
    p: DelayParams,
    lines: [DelayLine; 2],
    d: [f32; 2],
    lp: Biquad,
    hp: Biquad,
    depth: f32,
    ph: (f32, f32),
    rot: (f32, f32),
}

impl Delay {
    /// Invalid `note` strings fall back to 1/8 (validate with [`DelayParams::seconds`]).
    pub fn new(rate: f32, p: &DelayParams) -> Delay {
        let secs = p.seconds().unwrap_or(60.0 / p.bpm.max(1.0) / 2.0).clamp(0.001, 10.0);
        let depth = p.mod_depth.max(0.0) * 0.001 * rate;
        // Lines are read before the push, so a read of `d` delays by `d + 1`.
        let d0 = secs * rate - 1.0;
        let d1 = if p.ping_pong { d0 } else { d0 + p.spread.max(0.0) * 0.001 * rate };
        let max = (d0.max(d1) + depth + 4.0) as usize;
        let w = 2.0 * std::f32::consts::PI * p.mod_rate.max(0.0) / rate;
        Delay {
            lines: [DelayLine::new(max), DelayLine::new(max)],
            d: [d0.max(1.0 + depth), d1.max(1.0 + depth)],
            lp: Biquad::new(FilterKind::LowPass, rate, p.high_cut.max(200.0), 0.6, 0.0),
            hp: Biquad::new(FilterKind::HighPass, rate, p.low_cut.max(10.0), 0.6, 0.0),
            depth,
            ph: (1.0, 0.0),
            rot: (w.cos(), w.sin()),
            p: p.clone(),
        }
    }
}

impl Processor for Delay {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let fb = self.p.feedback.clamp(0.0, 0.98);
        let (wet, dry) = (self.p.wet, self.p.dry);
        let drive = self.p.drive.max(0.0);
        let pp = self.p.ping_pong;
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let (c, s) = self.ph;
            self.ph = (c * self.rot.0 - s * self.rot.1, c * self.rot.1 + s * self.rot.0);
            let m = self.depth * (1.0 + c) * 0.5;
            let m2 = self.depth * (1.0 + s) * 0.5;
            let yl = self.lines[0].read(self.d[0] - m);
            let yr = self.lines[1].read(self.d[1] - m2);
            let mut fl = self.hp.tick(0, self.lp.tick(0, yl));
            let mut fr = self.hp.tick(1, self.lp.tick(1, yr));
            if drive > 0.0 {
                let k = 1.0 + drive;
                fl = fast_tanh(fl * k) / k;
                fr = fast_tanh(fr * k) / k;
            }
            if pp {
                self.lines[0].push(flush(0.5 * (*a + *b) + fb * fr));
                self.lines[1].push(flush(fb * fl));
            } else {
                self.lines[0].push(flush(*a + fb * fl));
                self.lines[1].push(flush(*b + fb * fr));
            }
            *a = *a * dry + yl * wet;
            *b = *b * dry + yr * wet;
        }
        // Keep the LFO rotator on the unit circle.
        let (c, s) = self.ph;
        let n = 1.0 / (c * c + s * s).sqrt();
        self.ph = (c * n, s * n);
    }
    fn reset(&mut self) {
        self.lines.iter_mut().for_each(|x| x.reset());
        self.lp.reset();
        self.hp.reset();
        self.ph = (1.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_values() {
        assert_eq!(note_seconds("1/4", 120.0).unwrap(), 0.5);
        assert_eq!(note_seconds("1/8", 120.0).unwrap(), 0.25);
        assert!((note_seconds("1/8.", 120.0).unwrap() - 0.375).abs() < 1e-6);
        assert!((note_seconds("1/8d", 120.0).unwrap() - 0.375).abs() < 1e-6);
        assert!((note_seconds("1/4t", 120.0).unwrap() - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(note_seconds("1", 60.0).unwrap(), 4.0);
        assert!(note_seconds("x/4", 120.0).is_err());
    }

    #[test]
    fn echoes_land_on_time() {
        let p: DelayParams =
            serde_json::from_str(r#"{"note": "1/8", "bpm": 120, "feedback": 0.5, "wet": 1, "dry": 0, "modDepth": 0, "highCut": 20000, "lowCut": 10}"#).unwrap();
        let mut d = Delay::new(48_000.0, &p);
        let mut l = vec![0.0f32; 48_000];
        l[0] = 1.0;
        let mut r = l.clone();
        d.process(&mut l, &mut r);
        let argmax = |v: &[f32]| v.iter().enumerate().fold((0, 0.0f32), |m, (i, x)| if x.abs() > m.1 { (i, x.abs()) } else { m }).0;
        assert!((argmax(&l[..18_000]) as i64 - 12_000).abs() <= 2);
        assert!((argmax(&l[18_000..30_000]) as i64 + 18_000 - 24_000).abs() <= 2);
        // Ping-pong: first echo left only, second right only.
        let p = DelayParams { ping_pong: true, ..p };
        let mut d = Delay::new(48_000.0, &p);
        let mut l = vec![0.0f32; 48_000];
        l[0] = 1.0;
        let mut r = vec![0.0f32; 48_000];
        d.process(&mut l, &mut r);
        let e = |v: &[f32], a: usize| v[a - 50..a + 50].iter().map(|x| x * x).sum::<f32>();
        assert!(e(&l, 12_000) > 100.0 * e(&r, 12_000));
        assert!(e(&r, 24_000) > 100.0 * e(&l, 24_000));
        assert!(l.iter().chain(r.iter()).all(|x| x.is_finite()));
    }
}
