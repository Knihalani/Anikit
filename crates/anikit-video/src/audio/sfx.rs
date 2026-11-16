//! Procedural UI sound effects: deterministic, asset-free, synthesised from
//! shaped noise, resonant filters, FM/sine voices and a small stereo reverb.
//!
//! Names: `click`, `tick`, `pop`, `whoosh`, `swoosh` (reverse whoosh), `rise`
//! (alias `riser`), `drop` / `impact`, `ding`, `chime`, `typing`, `success`.
//! Every effect is normalised to a −1 dBFS peak.

use super::{Pcm, DEFAULT_RATE};
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Every built-in effect name.
pub const SFX_NAMES: &[&str] = &[
    "click", "tick", "pop", "whoosh", "swoosh", "rise", "riser", "drop", "impact", "ding", "chime",
    "typing", "success",
];

/// Parameters shared by all effects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SfxParams {
    /// Length in seconds for effects with a length (`whoosh`, `swoosh`,
    /// `rise`, `typing`); ignored by one-shots.
    pub duration: Option<f64>,
    /// Pitch multiplier (default 1).
    pub pitch: f64,
    /// Variation seed (same seed → same samples).
    pub seed: u64,
}

impl Default for SfxParams {
    fn default() -> Self {
        SfxParams {
            duration: None,
            pitch: 1.0,
            seed: 0,
        }
    }
}

/// Synthesise effect `name` at [`DEFAULT_RATE`].
pub fn sfx(name: &str, params: &SfxParams) -> Result<Pcm, String> {
    sfx_at(name, params, DEFAULT_RATE)
}

/// Synthesise effect `name` at `rate`.
pub fn sfx_at(name: &str, params: &SfxParams, rate: u32) -> Result<Pcm, String> {
    let key = name.trim().to_ascii_lowercase();
    let mut g = Gen {
        sr: rate as f32,
        p: params.pitch.clamp(0.25, 4.0) as f32,
        rng: Rng::new(params.seed ^ hash(&key)),
        dur: params.duration.map(|d| d.clamp(0.05, 60.0) as f32),
    };
    let out = match key.as_str() {
        "click" => g.click(),
        "tick" => g.tick(),
        "pop" => g.pop(),
        "whoosh" => g.whoosh(false),
        "swoosh" => g.whoosh(true),
        "rise" | "riser" => g.rise(),
        "drop" | "impact" => g.impact(),
        "ding" => g.ding(false),
        "chime" => g.ding(true),
        "typing" => g.typing(),
        "success" => g.success(),
        _ => {
            let hint = suggest(&key)
                .map(|s| format!(" (did you mean \"{s}\"?)"))
                .unwrap_or_default();
            return Err(format!(
                "audio: unknown sfx \"{name}\"{hint}; available: {}",
                SFX_NAMES.join(", ")
            ));
        }
    };
    Ok(Pcm {
        rate,
        channels: 2,
        samples: out,
    })
}

fn suggest(name: &str) -> Option<&'static str> {
    SFX_NAMES
        .iter()
        .map(|c| (levenshtein(name, c), *c))
        .filter(|(d, c)| *d <= 2.max(c.len() / 3) || c.starts_with(name) && !name.is_empty())
        .min_by_key(|x| x.0)
        .map(|x| x.1)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + (ca != cb) as usize).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    })
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    /// Uniform in −1…1.
    fn noise(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
    /// Uniform in 0…1.
    fn unit(&mut self) -> f32 {
        0.5 * (self.noise() + 1.0)
    }
}

/// RBJ biquad (transposed direct form II); coefficients can change per sample.
#[derive(Default, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

#[derive(Clone, Copy)]
enum Kind {
    Low,
    High,
    Band,
}

impl Biquad {
    fn new(kind: Kind, f: f32, q: f32, sr: f32) -> Biquad {
        let mut b = Biquad::default();
        b.set(kind, f, q, sr);
        b
    }
    fn set(&mut self, kind: Kind, f: f32, q: f32, sr: f32) {
        let w = TAU * f.clamp(10.0, sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        let (b0, b1, b2) = match kind {
            Kind::Low => ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0),
            Kind::High => ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0),
            Kind::Band => (alpha, 0.0, -alpha),
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * c / a0;
        self.a2 = (1.0 - alpha) / a0;
    }
    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Linear attack then exponential decay.
#[inline]
fn ad(t: f32, attack: f32, tau: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t < attack {
        t / attack
    } else {
        (-(t - attack) / tau).exp()
    }
}

struct Gen {
    sr: f32,
    p: f32,
    rng: Rng,
    dur: Option<f32>,
}

impl Gen {
    fn len(&self, secs: f32) -> usize {
        (secs * self.sr) as usize
    }

    fn click(&mut self) -> Vec<f32> {
        let mut x = vec![0.0; self.len(0.04)];
        let mut bp = Biquad::new(Kind::Band, 3200.0 * self.p, 1.4, self.sr);
        let mut hp = Biquad::new(Kind::High, 900.0, 0.7, self.sr);
        for (i, v) in x.iter_mut().enumerate() {
            let t = i as f32 / self.sr;
            let n = bp.run(self.rng.noise()) * ad(t, 0.0004, 0.004) * 2.5;
            let tone = (TAU * 1750.0 * self.p * t).sin() * ad(t, 0.0004, 0.009) * 0.5;
            *v = hp.run(n + tone);
        }
        self.finish(x, None, 0.08, 0.2, 0.08)
    }

    fn tick(&mut self) -> Vec<f32> {
        let mut x = vec![0.0; self.len(0.03)];
        let mut hp = Biquad::new(Kind::High, 6000.0, 0.7, self.sr);
        for (i, v) in x.iter_mut().enumerate() {
            let t = i as f32 / self.sr;
            let tone = (TAU * 5200.0 * self.p * t).sin() * ad(t, 0.0002, 0.005);
            *v = tone + hp.run(self.rng.noise()) * ad(t, 0.0001, 0.0015) * 0.7;
        }
        self.finish(x, None, 0.05, 0.15, 0.06)
    }

    fn pop(&mut self) -> Vec<f32> {
        let mut x = vec![0.0; self.len(0.12)];
        let mut lp = Biquad::new(Kind::Low, 4000.0, 0.7, self.sr);
        let mut ph = 0.0f32;
        for (i, v) in x.iter_mut().enumerate() {
            let t = i as f32 / self.sr;
            // A bubble: a quick upward chirp.
            let f = self.p * (380.0 + 1150.0 * (1.0 - (-t / 0.014).exp()));
            ph = (ph + TAU * f / self.sr) % TAU;
            let body = ph.sin() * ad(t, 0.001, 0.028);
            *v = body + lp.run(self.rng.noise()) * ad(t, 0.0002, 0.002) * 0.35;
        }
        self.finish(x, None, 0.12, 0.3, 0.12)
    }

    fn whoosh(&mut self, reverse: bool) -> Vec<f32> {
        let d = self.dur.unwrap_or(0.6);
        let n = self.len(d);
        let mut x = vec![0.0; n];
        let mut pan = vec![0.0; n];
        let mut bp = Biquad::default();
        let mut lp = Biquad::default();
        let pk = 0.55;
        for i in 0..n {
            let u = i as f32 / n as f32;
            let a = if u < pk {
                let s = u / pk;
                s * s * (3.0 - 2.0 * s)
            } else {
                ((1.0 - u) / (1.0 - pk)).powf(1.6)
            };
            bp.set(Kind::Band, self.p * (300.0 + 2600.0 * a), 1.3, self.sr);
            lp.set(Kind::Low, self.p * (250.0 + 1100.0 * a), 0.9, self.sr);
            let w = self.rng.noise();
            x[i] = (bp.run(w) * 1.2 + lp.run(w) * 0.6) * a;
            pan[i] = -0.6 + 1.2 * u;
        }
        if reverse {
            x.reverse();
            pan.reverse();
        }
        self.finish(x, Some(pan), 0.22, 0.5, 0.35)
    }

    fn rise(&mut self) -> Vec<f32> {
        let d = self.dur.unwrap_or(2.0);
        let n = self.len(d);
        let mut x = vec![0.0; n];
        let mut bp = Biquad::default();
        let mut ph = [0.0f32; 3];
        let det = [0.993, 1.0, 1.007];
        for (i, v) in x.iter_mut().enumerate() {
            let u = i as f32 / n as f32;
            bp.set(
                Kind::Band,
                self.p * 200.0 * 30f32.powf(u),
                1.5 + 4.0 * u,
                self.sr,
            );
            let air = bp.run(self.rng.noise()) * u.powf(2.2) * 1.6;
            // Three detuned band-limited saws (8 harmonics) sliding up two octaves.
            let f0 = self.p * 110.0 * 4f32.powf(u);
            let mut tone = 0.0;
            for (k, p) in ph.iter_mut().enumerate() {
                *p = (*p + TAU * f0 * det[k] / self.sr) % TAU;
                for h in 1..=8 {
                    if f0 * h as f32 > self.sr * 0.4 {
                        break;
                    }
                    tone += (*p * h as f32).sin() / h as f32;
                }
            }
            let end = ((n - i) as f32 / (0.015 * self.sr)).min(1.0);
            *v = (air + tone * 0.12 * u * u) * end;
        }
        self.finish(x, None, 0.25, 0.6, 0.8)
    }

    fn impact(&mut self) -> Vec<f32> {
        let mut x = vec![0.0; self.len(1.4)];
        let mut lp = Biquad::new(Kind::Low, 900.0 * self.p, 0.8, self.sr);
        let mut hp = Biquad::new(Kind::High, 3000.0, 0.7, self.sr);
        let mut ph = 0.0f32;
        for (i, v) in x.iter_mut().enumerate() {
            let t = i as f32 / self.sr;
            let f = self.p * (38.0 + 30.0 * (-t / 0.25).exp());
            ph = (ph + TAU * f / self.sr) % TAU;
            let sub = ph.sin() * ad(t, 0.002, 0.38);
            let thump = lp.run(self.rng.noise()) * ad(t, 0.001, 0.06) * 1.4;
            let crack = hp.run(self.rng.noise()) * ad(t, 0.0002, 0.004) * 0.5;
            *v = (1.6 * (sub + thump + crack)).tanh();
        }
        self.finish(x, None, 0.3, 0.85, 1.0)
    }

    /// An FM bell at `f` added into `x` from `at` seconds.
    fn bell(&self, x: &mut [f32], at: f32, f: f32, amp: f32, decay: f32) {
        let s0 = (at * self.sr) as usize;
        for (i, v) in x.iter_mut().enumerate().skip(s0) {
            let t = (i - s0) as f32 / self.sr;
            let index = 2.2 * (-t / 0.2).exp();
            let fm = (TAU * f * 1.4 * t).sin() * index;
            let a = ad(t, 0.0015, decay);
            *v += amp
                * a
                * ((TAU * f * t + fm).sin()
                    + 0.35 * (TAU * f * 2.76 * t).sin() * (-t / (0.35 * decay)).exp()
                    + 0.18 * (TAU * f * 5.4 * t).sin() * (-t / (0.15 * decay)).exp());
        }
    }

    fn ding(&mut self, chime: bool) -> Vec<f32> {
        let mut x = vec![0.0; self.len(1.6)];
        if chime {
            self.bell(&mut x, 0.0, 987.77 * self.p, 0.7, 0.45);
            self.bell(&mut x, 0.12, 1318.5 * self.p, 0.8, 0.6);
        } else {
            self.bell(&mut x, 0.0, 1318.5 * self.p, 1.0, 0.5);
        }
        self.finish(x, None, 0.3, 0.7, 0.9)
    }

    fn typing(&mut self) -> Vec<f32> {
        let d = self.dur.unwrap_or(1.5);
        let mut x = vec![0.0; self.len(d)];
        let mut t = 0.02;
        while t < d - 0.06 {
            let space = self.rng.unit() < 0.08;
            let amp = 0.55 + 0.45 * self.rng.unit();
            let f = 2400.0 + 2000.0 * self.rng.unit();
            let body = if space {
                140.0
            } else {
                190.0 + 80.0 * self.rng.unit()
            };
            self.key(&mut x, t, f, body, amp, space);
            // Key-up: a quieter click 40–80 ms later.
            let up = t + 0.04 + 0.04 * self.rng.unit();
            self.key(&mut x, up, f * 1.2, 0.0, amp * 0.3, false);
            t += 0.055 + 0.1 * self.rng.unit() + if self.rng.unit() < 0.06 { 0.22 } else { 0.0 };
        }
        self.finish(x, None, 0.06, 0.2, 0.1)
    }

    fn key(&mut self, x: &mut [f32], at: f32, f: f32, body: f32, amp: f32, space: bool) {
        let s0 = (at * self.sr) as usize;
        let n = self.len(if space { 0.06 } else { 0.035 });
        let mut bp = Biquad::new(
            Kind::Band,
            if space { f * 0.5 } else { f } * self.p,
            1.8,
            self.sr,
        );
        for (i, v) in x.iter_mut().skip(s0).take(n).enumerate() {
            let t = i as f32 / self.sr;
            let clack = bp.run(self.rng.noise()) * ad(t, 0.0003, 0.003) * 3.0;
            let thock = if body > 0.0 {
                (TAU * body * self.p * t).sin()
                    * ad(t, 0.0005, if space { 0.02 } else { 0.01 })
                    * 0.45
            } else {
                0.0
            };
            *v += amp * (clack + thock);
        }
    }

    fn success(&mut self) -> Vec<f32> {
        let mut x = vec![0.0; self.len(1.3)];
        let notes = [
            (0.0, 1046.5, 0.22),
            (0.085, 1318.5, 0.22),
            (0.17, 1568.0, 0.5),
        ];
        for (at, f, decay) in notes {
            let f = f * self.p;
            let s0 = (at * self.sr) as usize;
            for (i, v) in x.iter_mut().enumerate().skip(s0) {
                let t = (i - s0) as f32 / self.sr;
                let a = ad(t, 0.002, decay);
                // Marimba-ish: fundamental, a fast-decaying 4th partial, a little FM warmth.
                let fm = (TAU * f * 2.0 * t).sin() * 0.6 * (-t / 0.05).exp();
                *v += 0.6
                    * a
                    * ((TAU * f * t + fm).sin()
                        + 0.25 * (TAU * f * 4.0 * t).sin() * (-t / 0.03).exp());
            }
        }
        self.finish(x, None, 0.25, 0.6, 0.7)
    }

    /// Pan (equal power, −1…1), add a stereo reverb tail of `tail` seconds,
    /// normalise to −1 dBFS and return interleaved stereo.
    fn finish(
        &self,
        mut dry: Vec<f32>,
        pan: Option<Vec<f32>>,
        wet: f32,
        room: f32,
        tail: f32,
    ) -> Vec<f32> {
        let n_dry = dry.len();
        dry.resize(n_dry + self.len(tail), 0.0);
        let (wl, wr) = reverb(&dry, self.sr, room);
        let mut out = Vec::with_capacity(dry.len() * 2);
        for (i, &d) in dry.iter().enumerate() {
            let p = pan.as_ref().and_then(|p| p.get(i)).copied().unwrap_or(0.0);
            let a = (p.clamp(-1.0, 1.0) + 1.0) * 0.25 * PI;
            let (gl, gr) = (
                a.cos() * std::f32::consts::SQRT_2,
                a.sin() * std::f32::consts::SQRT_2,
            );
            out.push(d * gl + wl[i] * wet);
            out.push(d * gr + wr[i] * wet);
        }
        // Trim inaudible tail, fade the last 3 ms, normalise.
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if peak > 0.0 {
            let floor = peak * 1e-4;
            let last = out.iter().rposition(|s| s.abs() > floor).unwrap_or(0) / 2 + 1;
            let keep = last.max(n_dry).min(out.len() / 2);
            out.truncate(keep * 2);
            let fade = self.len(0.003).min(out.len() / 2);
            let n = out.len() / 2;
            for k in 0..fade {
                let g = (FRAC_PI_2 * k as f32 / fade as f32).sin();
                out[2 * (n - 1 - k)] *= g;
                out[2 * (n - 1 - k) + 1] *= g;
            }
            let g = 0.891 / peak; // −1 dBFS
            out.iter_mut().for_each(|s| *s *= g);
        }
        out
    }
}

/// A Freeverb-style stereo reverb (8 damped combs + 4 allpasses per side),
/// `room` 0…1. Returns the wet signal only.
fn reverb(x: &[f32], sr: f32, room: f32) -> (Vec<f32>, Vec<f32>) {
    const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
    const ALLPASS: [usize; 4] = [556, 441, 341, 225];
    const SPREAD: usize = 23;
    let scale = sr / 44_100.0 * (0.35 + 0.65 * room);
    let feedback = 0.7 + 0.27 * room;
    let damp = 0.35;
    let side = |offset: usize| -> Vec<f32> {
        let mut out = vec![0.0f32; x.len()];
        for &d in &COMBS {
            let len = (((d + offset) as f32) * scale) as usize + 1;
            let (mut buf, mut idx, mut store) = (vec![0.0f32; len], 0usize, 0.0f32);
            for (o, &s) in out.iter_mut().zip(x) {
                let y = buf[idx];
                store = y * (1.0 - damp) + store * damp;
                buf[idx] = s * 0.015 + store * feedback;
                *o += y;
                idx = if idx + 1 == len { 0 } else { idx + 1 };
            }
        }
        for &d in &ALLPASS {
            let len = (((d + offset) as f32) * sr / 44_100.0) as usize + 1;
            let (mut buf, mut idx) = (vec![0.0f32; len], 0usize);
            for o in out.iter_mut() {
                let b = buf[idx];
                let y = -*o + b;
                buf[idx] = *o + b * 0.5;
                *o = y;
                idx = if idx + 1 == len { 0 } else { idx + 1 };
            }
        }
        out.iter_mut().for_each(|v| *v *= 3.0);
        out
    };
    (side(0), side(SPREAD))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sfx_is_finite_audible_and_bounded() {
        for name in SFX_NAMES {
            let p = sfx(name, &SfxParams::default()).unwrap();
            assert_eq!(p.rate, DEFAULT_RATE);
            assert!(p.frames() > 100, "{name} too short");
            assert!(p.samples.iter().all(|s| s.is_finite()), "{name} not finite");
            let peak = p.peak();
            assert!(peak <= 1.0 && peak > 0.5, "{name} peak {peak}");
            let rms =
                (p.samples.iter().map(|s| s * s).sum::<f32>() / p.samples.len() as f32).sqrt();
            assert!(rms > 1e-3, "{name} rms {rms}");
        }
    }

    #[test]
    fn deterministic_and_parametric() {
        let a = sfx(
            "typing",
            &SfxParams {
                duration: Some(2.0),
                ..Default::default()
            },
        )
        .unwrap();
        let b = sfx(
            "typing",
            &SfxParams {
                duration: Some(2.0),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(a, b);
        assert!(a.duration() >= 2.0);
        let c = sfx(
            "typing",
            &SfxParams {
                duration: Some(2.0),
                seed: 7,
                ..Default::default()
            },
        )
        .unwrap();
        assert_ne!(a, c);
        let r = sfx_at(
            "rise",
            &SfxParams {
                duration: Some(1.0),
                ..Default::default()
            },
            22_050,
        )
        .unwrap();
        assert_eq!(r.rate, 22_050);
        assert!(r.duration() >= 1.0);
    }

    #[test]
    fn unknown_names_suggest() {
        let e = sfx("woosh", &SfxParams::default()).unwrap_err();
        assert!(e.contains("did you mean \"whoosh\""), "{e}");
        let e = sfx("sucess", &SfxParams::default()).unwrap_err();
        assert!(e.contains("\"success\""), "{e}");
        assert!(!sfx("xyzzyplugh", &SfxParams::default())
            .unwrap_err()
            .contains("did you mean"));
    }
}
