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
pub const SUB: usize = 32;
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

/// Zero-delay-feedback state-variable filter (Simper / TPT form).
#[derive(Clone, Copy, Default)]
struct Svf {
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
    ic1: [f32; 2],
    ic2: [f32; 2],
}

impl Svf {
    fn set(&mut self, fc_hz: f32, q: f32, rate: f32) {
        let fc = fc_hz.clamp(5.0, 0.45 * rate);
        let g = (std::f32::consts::PI * fc / rate).tan();
        let k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
        self.k = k;
    }
    #[inline]
    fn run(&mut self, kind: FilterKind, ch: usize, x: &mut [f32]) {
        let (a1, a2, a3, k) = (self.a1, self.a2, self.a3, self.k);
        let (mut ic1, mut ic2) = (self.ic1[ch], self.ic2[ch]);
        if kind == FilterKind::Off {
            return;
        }
        for s in x.iter_mut() {
            let v0 = *s;
            let v3 = v0 - ic2;
            let v1 = a1 * ic1 + a2 * v3;
            let v2 = ic2 + a2 * ic1 + a3 * v3;
            ic1 = 2.0 * v1 - ic1;
            ic2 = 2.0 * v2 - ic2;
            *s = match kind {
                FilterKind::Lp => v2,
                FilterKind::Hp => v0 - k * v1 - v2,
                FilterKind::Bp => v1,
                _ => v0 - k * v1,
            };
        }
        // denormal guard
        if ic1.abs() < 1e-20 {
            ic1 = 0.0;
        }
        if ic2.abs() < 1e-20 {
            ic2 = 0.0;
        }
        self.ic1[ch] = ic1;
        self.ic2[ch] = ic2;
    }
}

/// One sounding sample layer.
pub struct Osc {
    p: OscParams,
    interp: Interp,
    pos: u64,
    inc: u64,
    bucket: usize,
    stitch: [[f32; 2 * ST]; 2],
    vol: VolEnv,
    menv: ModEnv,
    t: f32, // voice time in seconds at the next control update
    sub_left: usize,
    g: f32, // gain at the start of the current control period
    g_step: f32,
    gl: f32,
    gr: f32,
    svf: Svf,
    fc_last: f32,
    released: bool,
    kill_n: f32, // frames since kill() (exact integers in f32)
    kill_step: f32,
    ended: bool,
    static_pitch: bool,
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
        let (gl, gr) = crate::voice::pan_gains(p.pan);
        let mut svf = Svf::default();
        if p.filter != FilterKind::Off {
            svf.set(cents_to_hz(p.fc), p.q, p.rate);
        }
        Osc {
            pos: ((p.start as u64) + PAD as u64) << 32,
            inc,
            bucket: interp::bucket(p.ratio),
            interp,
            stitch,
            vol: VolEnv::new(p.vol_env),
            menv: ModEnv::new(p.mod_env),
            t: 0.0,
            sub_left: 0,
            g: 0.0,
            g_step: 0.0,
            gl,
            gr,
            svf,
            fc_last: p.fc,
            released: false,
            kill_n: 0.0,
            kill_step: 0.0,
            ended: false,
            static_pitch,
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

    fn control(&mut self) {
        let dt = SUB as f32 / self.p.rate;
        let tmid = self.t + 0.5 * dt;
        self.vol.advance(dt);
        self.menv.advance(dt);
        self.t += dt;
        let menv = self.menv.v;
        let mlfo = lfo_value(&self.p.mod_lfo, tmid);
        let vlfo = lfo_value(&self.p.vib_lfo, tmid);

        if !self.static_pitch {
            let cents = menv * self.p.mod_env_pitch + mlfo * self.p.mod_lfo_pitch + vlfo * self.p.vib_lfo_pitch;
            let r = self.p.ratio * ((cents / 1200.0) as f64).exp2();
            self.inc = Self::to_fixed(r);
            self.bucket = interp::bucket(r);
        }
        if self.p.filter != FilterKind::Off {
            let fc = self.p.fc + menv * self.p.mod_env_fc + mlfo * self.p.mod_lfo_fc;
            if fc != self.fc_last {
                self.fc_last = fc;
                self.svf.set(cents_to_hz(fc), self.p.q, self.p.rate);
            }
        }
        let mut cb = self.p.atten - mlfo * self.p.mod_lfo_vol;
        cb = cb.max(-200.0);
        let mut target = self.p.gain * self.vol.value() * cb_to_amp(cb);
        if self.vol.stage == Stage::Done {
            target = 0.0;
        }
        self.g_step = (target - self.g) / SUB as f32;
    }

    /// Gain at the end of the current control period.
    #[inline]
    fn g_end(&self) -> f32 {
        self.g + self.g_step * SUB as f32
    }

    fn done(&self) -> bool {
        self.ended
            || (self.vol.stage == Stage::Done && self.g <= 1e-9 && self.sub_left == 0)
            || self.kill_step * self.kill_n >= 1.0
    }

    /// Fill `out[c][..n]` with interpolated sample frames; returns frames
    /// produced (fewer than `n` when the sample ends).
    fn read(&mut self, out: &mut [[f32; SUB]; 2], n: usize) -> usize {
        let nch = self.p.sample.ch.len().min(2);
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
            for c in 0..nch {
                let (data, pos): (&[f32], u64) = if use_stitch {
                    (&self.stitch[c][..], self.pos - (base << 32))
                } else {
                    (&self.p.sample.ch[c][..], self.pos)
                };
                let o = &mut out[c][done..done + m];
                let r = match self.interp {
                    Interp::Cubic => interp::cubic(data, pos, self.inc, o),
                    Interp::Sinc => interp::sinc(data, &interp::tables()[self.bucket], pos, self.inc, o),
                };
                np = if use_stitch { r + (base << 32) } else { r };
            }
            self.pos = np;
            done += m;
        }
        done
    }

    /// Render additively into `l`/`r`. Returns `false` once finished.
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        let mut off = 0;
        let mut buf = [[0f32; SUB]; 2];
        while off < n {
            if self.done() {
                return false;
            }
            if self.sub_left == 0 {
                self.control();
                self.sub_left = SUB;
            }
            let m = self.sub_left.min(n - off);
            let got = self.read(&mut buf, m);
            if self.p.filter != FilterKind::Off {
                for c in 0..self.p.sample.ch.len().min(2) {
                    self.svf.run(self.p.filter, c, &mut buf[c][..got]);
                }
            }
            // gain ramp × pan (× kill fade); indices count from the start of
            // the control period so any block split gives identical samples
            let j = (SUB - self.sub_left) as f32;
            let (g0, gs) = (self.g, self.g_step);
            let (kn, ks) = (self.kill_n, self.kill_step);
            let lo = &mut l[off..off + got];
            let ro = &mut r[off..off + got];
            let (gl, gr) = (self.gl, self.gr);
            if self.p.sample.ch.len() >= 2 {
                let (bl, br) = buf.split_at(1);
                for i in 0..got {
                    let fi = (i + 1) as f32;
                    let g = (g0 + gs * (j + fi)) * (1.0 - ks * (kn + fi)).max(0.0);
                    lo[i] += bl[0][i] * g * gl * std::f32::consts::SQRT_2;
                    ro[i] += br[0][i] * g * gr * std::f32::consts::SQRT_2;
                }
            } else {
                for i in 0..got {
                    let fi = (i + 1) as f32;
                    let g = (g0 + gs * (j + fi)) * (1.0 - ks * (kn + fi)).max(0.0);
                    let x = buf[0][i] * g;
                    lo[i] += x * gl;
                    ro[i] += x * gr;
                }
            }
            if ks > 0.0 {
                self.kill_n = kn + m as f32;
            }
            self.sub_left -= m;
            if self.sub_left == 0 {
                self.g = self.g_end();
            }
            off += m;
            if got < m {
                self.ended = true;
                return false;
            }
        }
        !self.done()
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
