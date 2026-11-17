//! Envelopes, LFOs and mono glide.

use super::util::sin2pi;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

/// ADSR with analog-style exponential segments. Times in seconds
/// (attack: to full level; decay/release: to −60 dB).
#[derive(Clone, Copy, Debug)]
pub struct Adsr {
    pub level: f32,
    stage: u8,
    ca: f32,
    cd: f32,
    cr: f32,
    sus: f32,
}

const ATTACK: u8 = 0;
const DECAY: u8 = 1;
const RELEASE: u8 = 2;
const DONE: u8 = 3;

impl Adsr {
    pub fn new(a: f32, d: f32, s: f32, r: f32, rate: f32) -> Adsr {
        // Attack approaches 1.3 so it reaches 1 in `a` seconds with an analog curve.
        let ca = if a <= 0.0 {
            1.0
        } else {
            1.0 - (-1.466 / (a * rate)).exp()
        };
        Adsr {
            level: 0.0,
            stage: ATTACK,
            ca,
            cd: super::util::t60_coef(d.max(1e-4), rate),
            cr: super::util::t60_coef(r.max(1e-4), rate),
            sus: s.clamp(0.0, 1.0),
        }
    }
    /// Change the release time (e.g. velocity- or key-dependent).
    pub fn set_release(&mut self, r: f32, rate: f32) {
        self.cr = super::util::t60_coef(r.max(1e-4), rate);
    }
    #[inline]
    pub fn release(&mut self) {
        if self.stage < RELEASE {
            self.stage = RELEASE;
        }
    }
    #[inline]
    pub fn done(&self) -> bool {
        self.stage == DONE
    }
    #[inline]
    pub fn releasing(&self) -> bool {
        self.stage >= RELEASE
    }
    /// Next sample.
    #[inline(always)]
    pub fn tick(&mut self) -> f32 {
        match self.stage {
            ATTACK => {
                self.level += (1.3 - self.level) * self.ca;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = DECAY;
                }
            }
            DECAY => {
                self.level = self.sus + (self.level - self.sus) * self.cd;
                // A decay to silence ends the envelope (plucks without note-off).
                if self.sus == 0.0 && self.level < 1e-5 {
                    self.level = 0.0;
                    self.stage = DONE;
                }
            }
            RELEASE => {
                self.level *= self.cr;
                if self.level < 1e-5 {
                    self.level = 0.0;
                    self.stage = DONE;
                }
            }
            _ => {}
        }
        self.level
    }
    /// Advance `n` samples without output (closed form outside the attack).
    #[inline]
    pub fn skip(&mut self, n: usize) {
        match self.stage {
            DECAY => {
                self.level = self.sus + (self.level - self.sus) * self.cd.powi(n as i32);
                if self.sus == 0.0 && self.level < 1e-5 {
                    self.level = 0.0;
                    self.stage = DONE;
                }
            }
            RELEASE => {
                self.level *= self.cr.powi(n as i32);
                if self.level < 1e-5 {
                    self.level = 0.0;
                    self.stage = DONE;
                }
            }
            DONE => {}
            _ => {
                for _ in 0..n {
                    self.tick();
                }
            }
        }
    }
    /// Fill `out` with the next `out.len()` samples.
    #[inline(always)]
    pub fn fill(&mut self, out: &mut [f32]) {
        for o in out.iter_mut() {
            *o = self.tick();
        }
    }
}

/// LFO shapes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LfoShape {
    Sine,
    Triangle,
}

/// Low-frequency oscillator with delay and fade-in (for delayed vibrato).
#[derive(Clone, Copy, Debug)]
pub struct Lfo {
    pub phase: f32,
    pub rate: f32,
    pub shape: LfoShape,
    delay: f32,
    fade: f32,
    t: f32,
}

impl Lfo {
    /// `rate` Hz; vibrato-style `delay` and `fade` in seconds; start `phase` 0..1.
    pub fn new(rate: f32, shape: LfoShape, delay: f32, fade: f32, phase: f32) -> Lfo {
        Lfo {
            phase,
            rate,
            shape,
            delay,
            fade: fade.max(1e-3),
            t: 0.0,
        }
    }
    /// Advance by `n` samples; returns the value in [−1, 1] (scaled by the fade-in).
    #[inline]
    pub fn step(&mut self, n: usize, rate: f32) -> f32 {
        let dt = n as f32 / rate;
        self.t += dt;
        self.phase = super::util::fract(self.phase + self.rate * dt);
        let v = match self.shape {
            LfoShape::Sine => sin2pi(self.phase),
            LfoShape::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
        };
        let amt = ((self.t - self.delay) / self.fade).clamp(0.0, 1.0);
        v * amt
    }
}

/// Shared state for mono glide: remembers the last key and whether that note
/// is still held (legato), so a new voice can slide from it.
#[derive(Debug, Default)]
pub struct GlideState {
    /// f32 bits of the last key.
    key: AtomicU32,
    /// (voice id << 2) | flags (bit 0: held, bit 1: released but not yet rendered).
    held: AtomicU64,
}

/// Per-voice handle into [`GlideState`].
#[derive(Debug)]
pub struct GlideLink {
    state: Arc<GlideState>,
    id: u64,
}

impl GlideState {
    /// Register a new note; returns the key to glide from (if the previous note
    /// was still held, i.e. legato) and the link the voice keeps.
    pub fn start(state: &Arc<GlideState>, key: f32, always: bool) -> (Option<f32>, GlideLink) {
        let prev = state.held.load(Ordering::Relaxed);
        let last = f32::from_bits(state.key.load(Ordering::Relaxed));
        let id = (prev >> 2) + 1;
        let from = if prev & 3 != 0 || (always && prev != 0) {
            Some(last)
        } else {
            None
        };
        state.key.store(key.to_bits(), Ordering::Relaxed);
        state.held.store((id << 2) | 1, Ordering::Relaxed);
        (
            from,
            GlideLink {
                state: state.clone(),
                id,
            },
        )
    }
}

impl GlideLink {
    /// The voice was released (note-off or mono steal).
    pub fn release(&self) {
        let _ = self.state.held.compare_exchange(
            (self.id << 2) | 1,
            (self.id << 2) | 2,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
    /// The voice rendered a block: a pending release becomes final.
    pub fn rendered(&self) {
        let _ = self.state.held.compare_exchange(
            (self.id << 2) | 2,
            self.id << 2,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

/// Exponential pitch glide (in semitones) towards a target key.
#[derive(Clone, Copy, Debug)]
pub struct Glide {
    pub key: f32,
    pub target: f32,
    coef: f32,
}

impl Glide {
    /// Glide from `from` to `to`, reaching it (≈ −40 dB of the interval) in `time` seconds.
    pub fn new(from: f32, to: f32, time: f32, rate: f32, block: usize) -> Glide {
        let coef = if time <= 0.0 {
            0.0
        } else {
            (-4.6 * block as f32 / (time * rate)).exp()
        };
        Glide {
            key: from,
            target: to,
            coef,
        }
    }
    /// Advance `n` steps (of the `block` given to [`Glide::new`]); returns the current key.
    #[inline]
    pub fn step(&mut self, n: usize) -> f32 {
        self.key = self.target + (self.key - self.target) * self.coef.powi(n as i32);
        self.key
    }
}
