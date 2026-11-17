//! The shared instrument interface. Every sound source (sampler, physical
//! model, synth, drum machine) implements [`Instrument`]; each note it plays
//! is a [`Voice`] that renders additively into planar stereo blocks.
//!
//! Rules for implementors (they keep rendering fast and deterministic):
//! - `render` must not allocate; keep per-sample loops branch-free over
//!   planar `f32` slices so LLVM vectorises them;
//! - keep decaying state out of denormal range (flush below ~1e-20);
//! - all randomness comes from [`NoteOn::seed`] (use [`Rng`]).

/// Internal sample rate of the engine.
pub const RATE: u32 = 48_000;
/// Maximum frames per `render` call.
pub const BLOCK: usize = 128;

/// A note to start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteOn {
    /// MIDI key, fractional for microtuning (60 = middle C, 69 = A4 = 440 Hz).
    pub key: f32,
    /// 0..1 (MIDI velocity / 127).
    pub vel: f32,
    /// Output sample rate.
    pub rate: f32,
    /// Deterministic per-note seed (humanisation, round robin, noise).
    pub seed: u64,
    /// How many times this instrument has played this key before (round-robin index).
    pub rr: u32,
    /// Planned length in seconds (note-on to note-off); voices may use it for
    /// legato/articulation choices. `f32::INFINITY` when unknown.
    pub len: f32,
}

impl NoteOn {
    /// Frequency in Hz (A4 = 440).
    #[inline]
    pub fn freq(&self) -> f32 {
        key_to_freq(self.key)
    }
}

/// `440 · 2^((key − 69) / 12)`.
#[inline]
pub fn key_to_freq(key: f32) -> f32 {
    440.0 * ((key - 69.0) / 12.0).exp2()
}

/// A sounding note.
pub trait Voice: Send {
    /// Add `l.len()` (≤ [`BLOCK`]) frames into `l` and `r`. Return `false` once
    /// the voice is silent for good (it is then dropped).
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool;
    /// Note-off: enter the release phase.
    fn release(&mut self);
    /// Exclusive class (e.g. closed hi-hat chokes open hi-hat); 0 = none.
    fn exclusive(&self) -> u32 {
        0
    }
    /// Stop quickly (≈5 ms fade), used for chokes and voice stealing.
    fn kill(&mut self) {
        self.release();
    }
}

/// Something that can play notes.
pub trait Instrument: Send + Sync {
    /// Start a note.
    fn voice(&self, note: &NoteOn) -> Box<dyn Voice>;
    /// Monophonic instruments (basses, leads) release the previous note when a new one starts.
    fn mono(&self) -> bool {
        false
    }
}

/// Small, fast deterministic RNG (SplitMix64 seeding + xorshift*).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Rng((z ^ (z >> 31)) | 1)
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1).
    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / (1u64 << 24) as f32)
    }
    /// Uniform in [−1, 1).
    #[inline]
    pub fn bi(&mut self) -> f32 {
        self.f32() * 2.0 - 1.0
    }
    /// Uniform in [lo, hi).
    #[inline]
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    /// Approximately normal (mean 0, sd 1).
    #[inline]
    pub fn gauss(&mut self) -> f32 {
        (self.f32() + self.f32() + self.f32() + self.f32() - 2.0) * 1.732
    }
    /// Uniform integer in [0, n).
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_u64() >> 32) * n as u64 >> 32) as usize
    }
}

/// Constant-power pan: `pan` in −1 (left)…1 (right) → (gl, gr).
#[inline]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}

/// Flush tiny values to zero (denormal guard).
#[inline]
pub fn undenorm(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}
