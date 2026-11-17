//! The sample-playback voice engine shared by the SF2/SF3 and SFZ front ends.
//!
//! A [`SamplerVoice`] holds one or more [`Osc`]s (one per matching region /
//! layer). Each oscillator reads a decoded [`SampleBuf`] through the selected
//! interpolation kernel, runs a TPT state-variable filter, and is shaped by a
//! DAHDSR volume envelope evaluated in the dB domain (like FluidSynth), a
//! modulation envelope and two triangle LFOs. Control values are updated
//! every [`SUB`] frames of *voice time* (so output does not depend on how the
//! caller splits blocks) and gains are ramped linearly in between.

use super::interp::{self, Interp, ONE};
use crate::voice::Voice;
use std::sync::Arc;

/// Control-rate period in frames.
pub const SUB: usize = 64;
/// Zero frames before and after every sample buffer.
pub const PAD: usize = 16;
/// Half the size of a loop-stitch window.
const ST: usize = 32;

/// A decoded sample: planar channels, each `PAD + len + PAD` long.
#[derive(Debug)]
pub struct SampleBuf {
    /// One (mono) or two (stereo) channels.
    pub ch: Vec<Box<[f32]>>,
    /// Frames of audio (excluding padding).
    pub len: usize,
    /// Sample rate.
    pub rate: u32,
    /// Loop points embedded in the source file (WAV `smpl`), frames.
    pub file_loop: Option<(u32, u32)>,
}

impl SampleBuf {
    /// Build from planar channel data.
    pub fn new(chans: Vec<Vec<f32>>, rate: u32) -> SampleBuf {
        let len = chans.first().map_or(0, |c| c.len());
        let ch = chans
            .into_iter()
            .map(|c| {
                let mut v = vec![0f32; PAD + len + PAD];
                v[PAD..PAD + len].copy_from_slice(&c[..len]);
                v.into_boxed_slice()
            })
            .collect();
        SampleBuf { ch, len, rate: rate.max(1), file_loop: None }
    }
}

/// Loop behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play start → end once (envelope still applies).
    None,
    /// Play to the end ignoring note-off (SFZ `one_shot`).
    OneShot,
    /// Loop until the voice has faded out.
    Continuous,
    /// Loop while held, then play on to the end (SF2 mode 3, SFZ `loop_sustain`).
    UntilRelease,
}

/// DAHDSR timing in seconds. `sustain` is an attenuation in cB for the volume
/// envelope, a level 0..1 for the modulation envelope.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Env {
    /// Delay before the attack.
    pub delay: f32,
    /// Attack (linear rise).
    pub attack: f32,
    /// Hold at peak.
    pub hold: f32,
    /// Decay: time for a full-scale fall (100 dB for volume, 1 → 0 for modulation).
    pub decay: f32,
    /// Sustain (cB attenuation for volume, level for modulation).
    pub sustain: f32,
    /// Release: time for a full-scale fall.
    pub release: f32,
}

impl Env {
    /// A flat "gate" envelope with a short release.
    pub const GATE: Env = Env { delay: 0.0, attack: 0.001, hold: 0.0, decay: 0.0, sustain: 0.0, release: 0.005 };
}

/// Triangle LFO: delay (s), frequency (Hz).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lfo {
    /// Delay before the LFO starts, seconds.
    pub delay: f32,
    /// Frequency in Hz.
    pub freq: f32,
}

/// Filter response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    /// Bypassed.
    Off,
    /// 2-pole low-pass.
    Lp,
    /// 2-pole high-pass.
    Hp,
    /// 2-pole band-pass.
    Bp,
    /// 2-pole band-reject (notch).
    Br,
}

/// Everything an oscillator needs, resolved at note-on.
#[derive(Clone)]
pub struct OscParams {
    /// Decoded sample.
    pub sample: Arc<SampleBuf>,
    /// First frame played.
    pub start: u32,
    /// One past the last frame.
    pub end: u32,
    /// Loop start frame.
    pub loop_start: u32,
    /// Loop end frame (exclusive: `loop_end` follows `loop_end − 1` with `loop_start`).
    pub loop_end: u32,
    /// Loop mode.
    pub loop_mode: LoopMode,
    /// Input frames per output frame at rest (pitch × rate conversion).
    pub ratio: f64,
    /// Output sample rate.
    pub rate: f32,
    /// Static attenuation in cB (≥ 0 quieter; velocity included).
    pub atten: f32,
    /// Extra linear gain.
    pub gain: f32,
    /// Pan −1..1 (balance for stereo samples).
    pub pan: f32,
    /// Volume envelope (sustain in cB).
    pub vol_env: Env,
    /// Modulation envelope (sustain level 0..1).
    pub mod_env: Env,
    /// Mod envelope → pitch, cents.
    pub mod_env_pitch: f32,
    /// Mod envelope → filter cut-off, cents.
    pub mod_env_fc: f32,
    /// Modulation LFO.
    pub mod_lfo: Lfo,
    /// Mod LFO → pitch, cents.
    pub mod_lfo_pitch: f32,
    /// Mod LFO → cut-off, cents.
    pub mod_lfo_fc: f32,
    /// Mod LFO → volume, cB (positive excursion = louder).
    pub mod_lfo_vol: f32,
    /// Vibrato LFO.
    pub vib_lfo: Lfo,
    /// Vibrato LFO → pitch, cents.
    pub vib_lfo_pitch: f32,
    /// Filter response.
    pub filter: FilterKind,
    /// Cut-off in absolute cents (8.176 Hz · 2^(c/1200)).
    pub fc: f32,
    /// Linear resonance (Q; 0.707 = flat).
    pub q: f32,
    /// Exclusive class.
    pub exclusive: u32,
    /// Interpolation kernel.
    pub interp: Interp,
}

/// Absolute cents → Hz.
#[inline]
pub fn cents_to_hz(c: f32) -> f32 {
    8.175_799 * (c / 1200.0).exp2()
}

/// Hz → absolute cents.
#[inline]
pub fn hz_to_cents(hz: f32) -> f32 {
    1200.0 * (hz.max(1e-3) / 8.175_799).log2()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
enum Stage {
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
    Done,
}

/// Volume envelope state, in the dB domain (attenuation in cB) except for
/// the attack, which rises linearly in amplitude.
#[derive(Clone, Copy)]
struct VolEnv {
    stage: Stage,
    t: f32,  // seconds spent in the stage
    cb: f32, // attenuation in decay/sustain/release
    amp: f32,
    p: Env,
}

const SILENT_CB: f32 = 1000.0;

#[inline]
fn cb_to_amp(cb: f32) -> f32 {
    (cb * (-std::f32::consts::LN_10 / 200.0)).exp()
}

impl VolEnv {
    fn new(p: Env) -> VolEnv {
        VolEnv { stage: Stage::Delay, t: 0.0, cb: 0.0, amp: 0.0, p }
    }
    /// Advance by `dt` seconds.
    fn advance(&mut self, mut dt: f32) {
        while dt > 0.0 {
            match self.stage {
                Stage::Delay => {
                    let left = self.p.delay - self.t;
                    if dt < left {
                        self.t += dt;
                        return;
                    }
                    dt -= left.max(0.0);
                    self.stage = Stage::Attack;
                    self.t = 0.0;
                }
                Stage::Attack => {
                    let left = self.p.attack - self.t;
                    if dt < left {
                        self.t += dt;
                        self.amp = self.t / self.p.attack;
                        return;
                    }
                    dt -= left.max(0.0);
                    self.amp = 1.0;
                    self.cb = 0.0;
                    self.stage = Stage::Hold;
                    self.t = 0.0;
                }
                Stage::Hold => {
                    let left = self.p.hold - self.t;
                    if dt < left {
                        self.t += dt;
                        return;
                    }
                    dt -= left.max(0.0);
                    self.stage = Stage::Decay;
                    self.t = 0.0;
                }
                Stage::Decay => {
                    let rate = SILENT_CB / self.p.decay.max(1e-4);
                    let target = self.p.sustain.min(SILENT_CB);
                    let need = (target - self.cb).max(0.0) / rate;
                    if dt < need {
                        self.cb += rate * dt;
                        return;
                    }
                    dt -= need;
                    self.cb = target;
                    self.stage = if target >= SILENT_CB { Stage::Done } else { Stage::Sustain };
                }
                Stage::Sustain => return,
                Stage::Release => {
                    let rate = SILENT_CB / self.p.release.max(1e-4);
                    self.cb += rate * dt;
                    if self.cb >= SILENT_CB {
                        self.stage = Stage::Done;
                    }
                    return;
                }
                Stage::Done => return,
            }
        }
    }
    /// Amplitude multiplier.
    fn value(&self) -> f32 {
        match self.stage {
            Stage::Delay => 0.0,
            Stage::Attack => self.amp,
            Stage::Done => 0.0,
            _ => cb_to_amp(self.cb),
        }
    }
    fn release(&mut self) {
        match self.stage {
            Stage::Release | Stage::Done => {}
            Stage::Delay => self.stage = Stage::Done,
            Stage::Attack => {
                self.cb = if self.amp > 1e-5 { -200.0 * self.amp.log10() } else { SILENT_CB };
                self.stage = Stage::Release;
            }
            _ => self.stage = Stage::Release,
        }
        self.t = 0.0;
    }
}

/// Modulation envelope: linear segments between 0 and 1.
#[derive(Clone, Copy)]
struct ModEnv {
    stage: Stage,
    t: f32,
    v: f32,
    p: Env,
}

impl ModEnv {
    fn new(p: Env) -> ModEnv {
        ModEnv { stage: Stage::Delay, t: 0.0, v: 0.0, p }
    }
    fn advance(&mut self, mut dt: f32) {
        while dt > 0.0 {
            match self.stage {
                Stage::Delay => {
                    let left = self.p.delay - self.t;
                    if dt < left {
                        self.t += dt;
                        return;
                    }
                    dt -= left.max(0.0);
                    self.stage = Stage::Attack;
                    self.t = 0.0;
                }
                Stage::Attack => {
                    let rate = 1.0 / self.p.attack.max(1e-5);
                    let need = (1.0 - self.v) / rate;
                    if dt < need {
                        self.v += rate * dt;
                        return;
                    }
                    dt -= need;
                    self.v = 1.0;
                    self.stage = Stage::Hold;
                    self.t = 0.0;
                }
                Stage::Hold => {
                    let left = self.p.hold - self.t;
                    if dt < left {
                        self.t += dt;
                        return;
                    }
                    dt -= left.max(0.0);
                    self.stage = Stage::Decay;
                }
                Stage::Decay => {
                    let rate = 1.0 / self.p.decay.max(1e-5);
                    let need = (self.v - self.p.sustain).max(0.0) / rate;
                    if dt < need {
                        self.v -= rate * dt;
                        return;
                    }
                    self.v = self.p.sustain;
                    self.stage = Stage::Sustain;
                    return;
                }
                Stage::Sustain | Stage::Done => return,
                Stage::Release => {
                    let rate = 1.0 / self.p.release.max(1e-5);
                    self.v = (self.v - rate * dt).max(0.0);
                    if self.v <= 0.0 {
                        self.stage = Stage::Done;
                    }
                    return;
                }
            }
        }
    }
    fn release(&mut self) {
        if self.stage != Stage::Done {
            self.stage = Stage::Release;
        }
    }
}

/// Triangle in −1..1 starting at 0 and rising, after a delay.
#[inline]
fn lfo_value(l: &Lfo, t: f32) -> f32 {
    if t < l.delay || l.freq <= 0.0 {
        return 0.0;
    }
    let ph = ((t - l.delay) * l.freq).fract();
    // 0→0, 0.25→1, 0.75→−1
    let x = ph * 4.0;
    if x < 1.0 {
        x
    } else if x < 3.0 {
        2.0 - x
    } else {
        x - 4.0
    }
}

/// Zero-delay-feedback state-variable filter (Simper / TPT form), run
/// [`SB`] samples at a time in state-space form so the per-sample feedback
/// chain does not bound throughput: for a block, `y = P·s + H·x` and
/// `s' = A^SB·s + G·x`, with the matrices derived from the one-sample
/// recurrence in `f64` (`H` is the Toeplitz matrix of the impulse response).
/// Output lanes are independent, so LLVM vectorises them; the arithmetic
/// order is fixed, so results are identical with or without SIMD.
#[derive(Clone, Copy, Default)]
struct Svf {
    p0: [f32; SB],
    p1: [f32; SB],
    /// `[0; SB−1]` followed by the impulse response `h[0..SB]`.
    imp: [f32; 2 * SB - 1],
    m: [[f32; 2]; 2],
    g: [[f32; 2]; SB],
    ic: [[f32; 2]; 2],
}

/// Filter block length.
const SB: usize = 8;

/// Sum of 8 products in a fixed tree order.
#[inline(always)]
fn dot8(a: &[f32], x: &[f32; 8]) -> f32 {
    ((a[0] * x[0] + a[1] * x[1]) + (a[2] * x[2] + a[3] * x[3])) + ((a[4] * x[4] + a[5] * x[5]) + (a[6] * x[6] + a[7] * x[7]))
}

impl Svf {
    fn set(&mut self, kind: FilterKind, fc_hz: f32, q: f32, rate: f32) {
        let fc = (fc_hz as f64).clamp(5.0, 0.45 * rate as f64);
        let g = (std::f64::consts::PI * fc / rate as f64).tan();
        let k = 1.0 / (q as f64).max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        // s' = A s + B x ; y = C s + D x   (s = [ic1, ic2])
        let (a00, a01, a10, a11) = (2.0 * a1 - 1.0, -2.0 * a2, 2.0 * a2, 1.0 - 2.0 * a3);
        let (b0, b1) = (2.0 * a2, 2.0 * a3);
        let (c, d) = match kind {
            FilterKind::Lp => ([a2, 1.0 - a3], a3),
            FilterKind::Bp => ([a1, -a2], a2),
            FilterKind::Hp => ([-k * a1 - a2, k * a2 - (1.0 - a3)], 1.0 - k * a2 - a3),
            _ => ([-k * a1, k * a2], 1.0 - k * a2),
        };
        // row vector r = C·A^k and column vector v = A^k·B, k = 0..SB
        let mut r = c;
        let mut v = [b0, b1];
        let mut vs = [[0f64; 2]; SB];
        self.imp = [0.0; 2 * SB - 1];
        self.imp[SB - 1] = d as f32;
        for kk in 0..SB {
            self.p0[kk] = r[0] as f32;
            self.p1[kk] = r[1] as f32;
            if kk + 1 < SB {
                // h[kk+1] = C·A^kk·B
                self.imp[SB + kk] = (r[0] * b0 + r[1] * b1) as f32;
            }
            vs[kk] = v;
            r = [r[0] * a00 + r[1] * a10, r[0] * a01 + r[1] * a11];
            v = [a00 * v[0] + a01 * v[1], a10 * v[0] + a11 * v[1]];
        }
        // g[j] = A^(SB−1−j)·B
        for j in 0..SB {
            let w = vs[SB - 1 - j];
            self.g[j] = [w[0] as f32, w[1] as f32];
        }
        // A^SB by repeated squaring (SB = 8)
        let sq = |m: [f64; 4]| {
            [
                m[0] * m[0] + m[1] * m[2],
                m[0] * m[1] + m[1] * m[3],
                m[2] * m[0] + m[3] * m[2],
                m[2] * m[1] + m[3] * m[3],
            ]
        };
        let an = sq(sq(sq([a00, a01, a10, a11])));
        self.m = [[an[0] as f32, an[2] as f32], [an[1] as f32, an[3] as f32]];
    }

    #[inline(always)]
    fn run(&mut self, ch: usize, x: &mut [f32; SUB]) {
        let mut s = self.ic[ch];
        let (p0, p1, imp, m, g) = (&self.p0, &self.p1, &self.imp, &self.m, &self.g);
        let gc0 = [g[0][0], g[1][0], g[2][0], g[3][0], g[4][0], g[5][0], g[6][0], g[7][0]];
        let gc1 = [g[0][1], g[1][1], g[2][1], g[3][1], g[4][1], g[5][1], g[6][1], g[7][1]];
        for blk in x.chunks_exact_mut(SB) {
            let xs: [f32; SB] = blk.try_into().unwrap();
            // reversed input so that y[k] = Σ_j imp[SB−1+k−j]·x[j] reads imp forwards
            let xr = [xs[7], xs[6], xs[5], xs[4], xs[3], xs[2], xs[1], xs[0]];
            let mut y = [0f32; SB];
            for k in 0..SB {
                y[k] = (p0[k] * s[0] + p1[k] * s[1]) + dot8(&imp[k..k + SB], &xr);
            }
            let n0 = (m[0][0] * s[0] + m[1][0] * s[1]) + dot8(&gc0, &xs);
            let n1 = (m[0][1] * s[0] + m[1][1] * s[1]) + dot8(&gc1, &xs);
            s = [n0, n1];
            blk.copy_from_slice(&y);
        }
        // denormal guard
        for v in s.iter_mut() {
            if v.abs() < 1e-20 {
                *v = 0.0;
            }
        }
        self.ic[ch] = s;
    }
}

/// One sounding sample layer. Audio is produced one control period
/// ([`SUB`] frames of voice time) at a time into an internal buffer, so the
/// output never depends on how the caller splits blocks and every inner loop
/// runs over a fixed-size array.
pub struct Osc {
    p: OscParams,
    interp: Interp,
    nch: usize,
    pos: u64,
    inc: u64,
    bucket: usize,
    stitch: [[f32; 2 * ST]; 2],
    vol: VolEnv,
    menv: ModEnv,
    t: f32, // voice time in seconds at the next control update
    g: f32, // gain at the end of the last period
    gl: f32,
    gr: f32,
    svf: Svf,
    fc_last: f32,
    released: bool,
    kill_n: f32, // frames since kill() (exact integers in f32)
    kill_step: f32,
    ended: bool,
    finished: bool,
    static_pitch: bool,
    out: [[f32; SUB]; 2],
    rd: usize,
}

const FADE: f32 = 0.005;

impl Osc {
    /// Prepare an oscillator.
    pub fn new(mut p: OscParams) -> Osc {
        let len = p.sample.len as u32;
        p.end = p.end.min(len);
        p.start = p.start.min(p.end);
        let interp = p.interp;
        let looped = matches!(p.loop_mode, LoopMode::Continuous | LoopMode::UntilRelease);
        if looped && (p.loop_end > p.end || p.loop_start + 1 >= p.loop_end) {
            // Unusable loop: play straight through.
            p.loop_mode = LoopMode::None;
        }
        let mut stitch = [[0f32; 2 * ST]; 2];
        if matches!(p.loop_mode, LoopMode::Continuous | LoopMode::UntilRelease) {
            let (ls, le) = (p.loop_start as i64, p.loop_end as i64);
            let l = le - ls;
            for (c, st) in stitch.iter_mut().enumerate().take(p.sample.ch.len()) {
                let d = &p.sample.ch[c];
                for (k, s) in st.iter_mut().enumerate() {
                    let mut v = le - ST as i64 + k as i64;
                    if v >= le {
                        v = ls + (v - le) % l;
                    }
                    let i = v + PAD as i64;
                    *s = if i >= 0 && (i as usize) < d.len() { d[i as usize] } else { 0.0 };
                }
            }
        }
        let static_pitch = p.mod_env_pitch == 0.0 && p.mod_lfo_pitch == 0.0 && p.vib_lfo_pitch == 0.0;
        let inc = Self::to_fixed(p.ratio);
        let nch = p.sample.ch.len().clamp(1, 2);
        let (mut gl, mut gr) = crate::voice::pan_gains(p.pan);
        if nch == 2 {
            // stereo samples: pan acts as balance, unity at centre
            gl *= std::f32::consts::SQRT_2;
            gr *= std::f32::consts::SQRT_2;
        }
        let mut svf = Svf::default();
        if p.filter != FilterKind::Off {
            svf.set(p.filter, cents_to_hz(p.fc), p.q, p.rate);
        }
        Osc {
            pos: ((p.start as u64) + PAD as u64) << 32,
            inc,
            bucket: interp::bucket(p.ratio),
            interp,
            nch,
            stitch,
            vol: VolEnv::new(p.vol_env),
            menv: ModEnv::new(p.mod_env),
            t: 0.0,
            g: 0.0,
            gl,
            gr,
            svf,
            fc_last: p.fc,
            released: false,
            kill_n: 0.0,
            kill_step: 0.0,
            ended: false,
            finished: false,
            static_pitch,
            out: [[0.0; SUB]; 2],
            rd: SUB,
            p,
        }
    }

    #[inline]
    fn to_fixed(r: f64) -> u64 {
        ((r.clamp(1e-6, 1024.0)) * ONE as f64).round() as u64
    }

    /// Note-off.
    pub fn release(&mut self) {
        if !self.released {
            self.released = true;
            if self.p.loop_mode != LoopMode::OneShot {
                self.vol.release();
            }
            self.menv.release();
        }
    }

    /// Fast fade (≈5 ms).
    pub fn kill(&mut self) {
        self.released = true;
        if self.kill_step == 0.0 {
            self.kill_step = 1.0 / (FADE * self.p.rate).max(1.0);
        }
    }

    /// Exclusive class.
    pub fn exclusive(&self) -> u32 {
        self.p.exclusive
    }

    /// Advance the control state by one period; returns the gain target.
    fn control(&mut self) -> f32 {
        let dt = SUB as f32 / self.p.rate;
        let tmid = self.t + 0.5 * dt;
        self.vol.advance(dt);
        self.menv.advance(dt);
        self.t += dt;
        let menv = self.menv.v;
        let mlfo = if self.p.mod_lfo.freq > 0.0 { lfo_value(&self.p.mod_lfo, tmid) } else { 0.0 };
        if !self.static_pitch {
            let vlfo = lfo_value(&self.p.vib_lfo, tmid);
            let cents = menv * self.p.mod_env_pitch + mlfo * self.p.mod_lfo_pitch + vlfo * self.p.vib_lfo_pitch;
            let r = self.p.ratio * ((cents / 1200.0) as f64).exp2();
            self.inc = Self::to_fixed(r);
            self.bucket = interp::bucket(r);
        }
        if self.p.filter != FilterKind::Off {
            let fc = self.p.fc + menv * self.p.mod_env_fc + mlfo * self.p.mod_lfo_fc;
            if (fc - self.fc_last).abs() >= 3.0 {
                self.fc_last = fc;
                self.svf.set(self.p.filter, cents_to_hz(fc), self.p.q, self.p.rate);
            }
        }
        if self.vol.stage == Stage::Done {
            return 0.0;
        }
        let cb = (self.p.atten - mlfo * self.p.mod_lfo_vol).max(-200.0);
        self.p.gain * self.vol.value() * cb_to_amp(cb)
    }

    /// Fill `out[c][..n]` with interpolated sample frames; returns frames
    /// produced (fewer than `n` when the sample ends).
    fn read(&mut self, out: &mut [[f32; SUB]; 2], n: usize) -> usize {
        let nch = self.nch;
        let lft = interp::left(self.interp) as u64;
        let rgt = interp::right(self.interp) as u64;
        let pad = PAD as u64;
        let mut done = 0;
        while done < n {
            let looping = match self.p.loop_mode {
                LoopMode::Continuous => true,
                LoopMode::UntilRelease => !self.released,
                _ => false,
            };
            let idx = self.pos >> 32;
            let (target, base, use_stitch) = if looping {
                let le = self.p.loop_end as u64 + pad;
                let w = lft + 1;
                if idx >= le + w {
                    let l = (self.p.loop_end - self.p.loop_start) as u64;
                    let over = (idx - (le + w)) / l + 1;
                    self.pos -= (over * l) << 32;
                    continue;
                }
                if idx + rgt < le {
                    (le - rgt, 0u64, false)
                } else {
                    (le + w, le - ST as u64, true)
                }
            } else {
                let end = self.p.end as u64 + pad;
                if idx >= end {
                    self.ended = true;
                    break;
                }
                (end, 0, false)
            };
            let dist = (target << 32) - self.pos;
            let k = dist.div_ceil(self.inc) as usize;
            let m = k.min(n - done).max(1);
            let mut np = self.pos;
            for (c, oc) in out.iter_mut().enumerate().take(nch) {
                let (data, pos): (&[f32], u64) = if use_stitch {
                    (&self.stitch[c][..], self.pos - (base << 32))
                } else {
                    (&self.p.sample.ch[c][..], self.pos)
                };
                let o = &mut oc[done..done + m];
                let r = match self.interp {
                    Interp::Cubic => interp::cubic(data, pos, self.inc, o),
                    Interp::Sinc => interp::sinc(data, interp::table(self.bucket), pos, self.inc, o),
                };
                np = if use_stitch { r + (base << 32) } else { r };
            }
            self.pos = np;
            done += m;
        }
        done
    }

    /// Produce the next control period into `self.out`.
    fn produce(&mut self) {
        #[cfg(target_arch = "x86_64")]
        if interp::has_avx2() {
            // SAFETY: the CPU supports AVX2 (checked at run time). No FMA is
            // enabled, so results are identical to the baseline build.
            unsafe { self.produce_avx2() };
            return;
        }
        self.produce_impl();
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn produce_avx2(&mut self) {
        self.produce_impl();
    }

    #[inline(always)]
    fn produce_impl(&mut self) {
        let target = self.control();
        let mut buf = [[0f32; SUB]; 2];
        let got = self.read(&mut buf, SUB);
        let nch = self.nch;
        if self.p.filter != FilterKind::Off {
            for (c, b) in buf.iter_mut().enumerate().take(nch) {
                self.svf.run(c, b);
            }
        }
        // linear gain ramp from the previous period's end to the new target
        let g0 = self.g;
        let gs = (target - g0) * (1.0 / SUB as f32);
        let mut ramp = [0f32; SUB];
        for (i, v) in ramp.iter_mut().enumerate() {
            *v = g0 + gs * (i + 1) as f32;
        }
        if self.kill_step > 0.0 {
            let (kn, ks) = (self.kill_n, self.kill_step);
            for (i, v) in ramp.iter_mut().enumerate() {
                *v *= (1.0 - ks * (kn + (i + 1) as f32)).max(0.0);
            }
            self.kill_n = kn + SUB as f32;
        }
        for (b, o) in buf.iter().zip(self.out.iter_mut()).take(nch) {
            for ((o, x), g) in o.iter_mut().zip(b.iter()).zip(ramp.iter()) {
                *o = x * g;
            }
        }
        self.g = target;
        self.rd = 0;
        if got < SUB
            || self.ended
            || (self.vol.stage == Stage::Done && target == 0.0)
            || self.kill_step * self.kill_n >= 1.0
        {
            self.finished = true;
        }
    }

    /// Render additively into `l`/`r`. Returns `false` once finished.
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        let mut off = 0;
        while off < n {
            if self.rd == SUB {
                if self.finished {
                    return false;
                }
                self.produce();
            }
            let m = (SUB - self.rd).min(n - off);
            let (gl, gr) = (self.gl, self.gr);
            let lo = &mut l[off..off + m];
            let ro = &mut r[off..off + m];
            let src0 = &self.out[0][self.rd..self.rd + m];
            if self.nch == 2 {
                let src1 = &self.out[1][self.rd..self.rd + m];
                for (o, x) in lo.iter_mut().zip(src0) {
                    *o += x * gl;
                }
                for (o, x) in ro.iter_mut().zip(src1) {
                    *o += x * gr;
                }
            } else {
                for ((ol, or), x) in lo.iter_mut().zip(ro.iter_mut()).zip(src0) {
                    *ol += x * gl;
                    *or += x * gr;
                }
            }
            self.rd += m;
            off += m;
        }
        !(self.finished && self.rd == SUB)
    }
}

/// A note made of one or more oscillators, plus release-triggered layers
/// that start on note-off.
pub struct SamplerVoice {
    oscs: Vec<Osc>,
    on_release: Vec<Osc>,
    live: Vec<bool>,
    exclusive: u32,
    released: bool,
}

impl SamplerVoice {
    /// Build a voice from its layers. `on_release` layers start at note-off.
    pub fn new(oscs: Vec<Osc>, on_release: Vec<Osc>) -> SamplerVoice {
        let exclusive = oscs.iter().chain(on_release.iter()).map(|o| o.exclusive()).find(|&e| e != 0).unwrap_or(0);
        let n = oscs.len() + on_release.len();
        let mut oscs = oscs;
        oscs.reserve(on_release.len());
        SamplerVoice { live: vec![true; n], oscs, on_release, exclusive, released: false }
    }
}

impl Voice for SamplerVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let mut any = false;
        for (o, live) in self.oscs.iter_mut().zip(self.live.iter_mut()) {
            if *live {
                *live = o.render(l, r);
                any |= *live;
            }
        }
        any || (!self.released && !self.on_release.is_empty())
    }
    fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        for o in &mut self.oscs {
            o.release();
        }
        // Capacity was reserved at note-on: this moves without allocating.
        self.oscs.append(&mut self.on_release);
    }
    fn exclusive(&self) -> u32 {
        self.exclusive
    }
    fn kill(&mut self) {
        self.released = true;
        self.on_release.clear();
        for o in &mut self.oscs {
            o.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_volenv_db_domain() {
        let mut e = VolEnv::new(Env { delay: 0.0, attack: 0.01, hold: 0.0, decay: 1.0, sustain: 200.0, release: 1.0 });
        e.advance(0.005);
        assert!((e.value() - 0.5).abs() < 1e-3);
        e.advance(0.005 + 0.1);
        // 0.1 s into a 1 s / 100 dB decay → −10 dB
        assert!((e.value() - cb_to_amp(100.0)).abs() < 1e-3, "{}", e.value());
        e.advance(10.0);
        assert!((e.value() - cb_to_amp(200.0)).abs() < 1e-4);
        e.release();
        e.advance(0.5);
        assert!((e.value() - cb_to_amp(700.0)).abs() < 1e-4);
        e.advance(0.31);
        assert_eq!(e.stage, Stage::Done);
    }
}
