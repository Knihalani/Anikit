//! Level-independent transient shaper: boosts or softens attacks and
//! sustains by comparing fast and slow envelope followers (in dB).

use serde::Deserialize;

use super::util::{fast_db_to_gain, fast_gain_to_db, flush};
use super::Processor;

/// Transient shaper parameters (dB of change at full detection).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct TransientParams {
    /// Attack emphasis in dB (+ = punchier, − = softer).
    pub attack: f32,
    /// Sustain emphasis in dB (+ = more body/room, − = tighter).
    pub sustain: f32,
}

impl Default for TransientParams {
    fn default() -> Self {
        TransientParams { attack: 4.0, sustain: 0.0 }
    }
}

#[derive(Debug, Clone, Copy)]
struct Follower {
    a: f32,
    r: f32,
    y: f32,
}

impl Follower {
    fn new(rate: f32, att_ms: f32, rel_ms: f32) -> Follower {
        let c = |ms: f32| (-1.0 / (ms * 0.001 * rate)).exp();
        Follower { a: c(att_ms), r: c(rel_ms), y: 0.0 }
    }
    #[inline(always)]
    fn step(&mut self, x: f32) -> f32 {
        let c = if x > self.y { self.a } else { self.r };
        self.y = flush(x + c * (self.y - x));
        self.y
    }
}

/// Envelope-difference transient shaper (stereo-linked). A fast peak
/// envelope is compared (in dB) with an attack-limited copy of itself
/// (transients) and with a slow-release copy (sustain); a 1.5 dB dead zone
/// keeps steady tones untouched.
#[derive(Debug, Clone)]
pub struct TransientShaper {
    p: TransientParams,
    base: Follower,
    slow: Follower,
    hold: Follower,
    smooth: f32,
    g: f32,
}

impl TransientShaper {
    pub fn new(rate: f32, p: &TransientParams) -> TransientShaper {
        TransientShaper {
            p: p.clone(),
            base: Follower::new(rate, 0.1, 60.0),
            slow: Follower::new(rate, 20.0, 60.0),
            hold: Follower::new(rate, 0.1, 350.0),
            smooth: (-1.0 / (0.001 * rate)).exp(),
            g: 0.0,
        }
    }
}

const DEAD: f32 = 1.5;
const SPAN: f32 = 10.0;

impl Processor for TransientShaper {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let x = a.abs().max(b.abs()) + 1e-9;
            let e = self.base.step(x);
            let ed = fast_gain_to_db(e);
            let att = ((ed - fast_gain_to_db(self.slow.step(e)) - DEAD) / SPAN).clamp(0.0, 1.0);
            let sus = ((fast_gain_to_db(self.hold.step(e)) - ed - DEAD) / SPAN).clamp(0.0, 1.0);
            let target = self.p.attack * att + self.p.sustain * sus;
            self.g = flush(target + self.smooth * (self.g - target));
            let g = fast_db_to_gain(self.g);
            *a *= g;
            *b *= g;
        }
    }
    fn reset(&mut self) {
        for f in [&mut self.base, &mut self.slow, &mut self.hold] {
            f.y = 0.0;
        }
        self.g = 0.0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boosts_attack_only() {
        // A decaying 200 Hz "pluck": the first 10 ms gets louder, the steady part is unchanged.
        let n = 48_000;
        let x: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / 48_000.0;
                0.5 * (2.0 * std::f32::consts::PI * 200.0 * t).sin() * (0.3 + 0.7 * (-t * 20.0).exp())
            })
            .collect();
        let mut s = TransientShaper::new(48_000.0, &TransientParams { attack: 6.0, sustain: 0.0 });
        let (mut l, mut r) = (x.clone(), x.clone());
        s.process(&mut l, &mut r);
        let pk = |v: &[f32]| v.iter().fold(0.0f32, |m, a| m.max(a.abs()));
        assert!(pk(&l[..480]) > pk(&x[..480]) * 1.3);
        assert!((pk(&l[40_000..]) / pk(&x[40_000..]) - 1.0).abs() < 0.03);
    }
}
