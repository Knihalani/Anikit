//! Two-operator FM pairs (up to four, i.e. a 4–8 operator patch), DX-style:
//! each pair is a modulator → carrier with its own decaying index, ratio,
//! detune, level and pan. Used for electric pianos, FM bells and lo-fi keys.

use super::env::Lfo;
use super::filter::OnePole;
use super::util::{fract_pos, prewarp, sin2pi, sin2pi_unit, t60_coef, tanh, vel_gain, Noise, Out};
use crate::voice::{key_to_freq, Instrument, NoteOn, Rng, Voice};

/// One modulator → carrier pair.
#[derive(Clone, Copy, Debug)]
pub struct FmPair {
    /// Carrier frequency ratio.
    pub carrier: f32,
    /// Modulator frequency ratio.
    pub modr: f32,
    /// Modulation index at velocity 0 and the extra at velocity 1 (applied as vel²).
    pub index: f32,
    pub index_vel: f32,
    /// Index envelope: −60 dB time of the decaying part (s), and the sustained fraction.
    pub index_t60: f32,
    pub index_sus: f32,
    /// Modulator self-feedback (0..1).
    pub fb: f32,
    pub level: f32,
    /// Level change per octave above middle C (dB).
    pub level_key: f32,
    pub cents: f32,
    pub pan: f32,
    /// Fixed-ratio modulators lose index on high keys (dB per octave).
    pub index_key: f32,
}

impl FmPair {
    pub const fn new(
        carrier: f32,
        modr: f32,
        index: f32,
        index_vel: f32,
        index_t60: f32,
        index_sus: f32,
        level: f32,
    ) -> FmPair {
        FmPair {
            carrier,
            modr,
            index,
            index_vel,
            index_t60,
            index_sus,
            fb: 0.0,
            level,
            level_key: 0.0,
            cents: 0.0,
            pan: 0.0,
            index_key: 0.0,
        }
    }
    pub const fn cents(mut self, c: f32) -> FmPair {
        self.cents = c;
        self
    }
    pub const fn pan(mut self, p: f32) -> FmPair {
        self.pan = p;
        self
    }
    pub const fn fb(mut self, f: f32) -> FmPair {
        self.fb = f;
        self
    }
    pub const fn level_key(mut self, d: f32) -> FmPair {
        self.level_key = d;
        self
    }
    pub const fn index_key(mut self, d: f32) -> FmPair {
        self.index_key = d;
        self
    }
}

/// An FM patch.
#[derive(Clone, Debug)]
pub struct FmPatch {
    pub pairs: Vec<FmPair>,
    /// Attack (s).
    pub attack: f32,
    /// Fast decay (−60 dB time, s) and the level share of the slow decay.
    pub t60_fast: f32,
    pub slow_mix: f32,
    pub t60_slow: f32,
    /// Decay scale per octave above middle C.
    pub key_decay: f32,
    /// Release (−60 dB, s).
    pub release: f32,
    /// Stereo auto-pan (Hz, depth 0..1).
    pub trem: (f32, f32),
    /// Pitch wobble: wow (cents, Hz) and flutter (cents, Hz).
    pub wow: (f32, f32),
    pub flutter: (f32, f32),
    /// Low-pass (Hz, 0 = off), raised by velocity (octaves).
    pub lp: f32,
    pub lp_vel: f32,
    pub drive: f32,
    /// Hiss level (lo-fi).
    pub hiss: f32,
    pub gain: f32,
}

/// An FM instrument.
pub struct FmSynth(pub FmPatch);

impl Instrument for FmSynth {
    fn voice(&self, n: &NoteOn) -> Box<dyn Voice> {
        Box::new(FmVoice::new(&self.0, n))
    }
}

const MAXP: usize = 4;

struct FmVoice {
    rate: f32,
    // Per-pair state (arrays so the inner loop is fixed-size).
    pc: [f32; MAXP],
    pm: [f32; MAXP],
    dc: [f32; MAXP],
    dm: [f32; MAXP],
    i_sus: [f32; MAXP],
    i_dec: [f32; MAXP],
    i_coef: [f32; MAXP],
    fb: [f32; MAXP],
    prev: [f32; MAXP],
    gl: [f32; MAXP],
    gr: [f32; MAXP],
    base: [f32; MAXP],
    base_m: [f32; MAXP],
    f0: f32,
    att: f32,
    att_step: f32,
    e_fast: f32,
    e_slow: f32,
    c_fast: f32,
    c_slow: f32,
    c_rel: f32,
    released: bool,
    trem: Lfo,
    trem_depth: f32,
    wow: Lfo,
    flutter: Lfo,
    wow_c: f32,
    flutter_c: f32,
    lp: [[OnePole; 2]; 2],
    lp_g: f32,
    drive: f32,
    hiss: f32,
    noise: Noise,
    out: Out,
}

impl FmVoice {
    fn new(p: &FmPatch, n: &NoteOn) -> FmVoice {
        let rate = n.rate;
        let mut rng = Rng::new(n.seed);
        let f0 = key_to_freq(n.key);
        let oct = (n.key - 60.0) / 12.0;
        let kd = p.key_decay.powf(oct);
        let v2 = n.vel * n.vel;
        let mut s = FmVoice {
            rate,
            pc: [0.0; MAXP],
            pm: [0.0; MAXP],
            dc: [0.0; MAXP],
            dm: [0.0; MAXP],
            i_sus: [0.0; MAXP],
            i_dec: [0.0; MAXP],
            i_coef: [0.0; MAXP],
            fb: [0.0; MAXP],
            prev: [0.0; MAXP],
            gl: [0.0; MAXP],
            gr: [0.0; MAXP],
            base: [0.0; MAXP],
            base_m: [0.0; MAXP],
            f0,
            att: 0.0,
            att_step: 1.0 / (p.attack.max(1e-4) * rate),
            e_fast: 1.0 - p.slow_mix,
            e_slow: p.slow_mix,
            c_fast: t60_coef(p.t60_fast * kd, rate),
            c_slow: t60_coef(p.t60_slow * kd, rate),
            c_rel: t60_coef(p.release * kd.sqrt(), rate),
            released: false,
            trem: Lfo::new(p.trem.0, super::env::LfoShape::Sine, 0.0, 0.001, rng.f32()),
            trem_depth: p.trem.1,
            wow: Lfo::new(p.wow.1, super::env::LfoShape::Sine, 0.0, 0.001, rng.f32()),
            flutter: Lfo::new(
                p.flutter.1,
                super::env::LfoShape::Sine,
                0.0,
                0.001,
                rng.f32(),
            ),
            wow_c: p.wow.0,
            flutter_c: p.flutter.0,
            lp: [[OnePole::default(); 2]; 2],
            lp_g: if p.lp > 0.0 {
                OnePole::coef(prewarp(p.lp * (p.lp_vel * (n.vel - 0.7)).exp2(), rate))
            } else {
                0.0
            },
            drive: p.drive,
            hiss: p.hiss,
            noise: Noise::new(n.seed ^ 0x5151),
            out: Out::new(vel_gain(n.vel) * p.gain, 0.0),
        };
        for (k, q) in p.pairs.iter().take(MAXP).enumerate() {
            let det = (q.cents / 1200.0).exp2();
            s.base[k] = q.carrier * det;
            s.base_m[k] = q.modr * det;
            let ik = super::util::db(-q.index_key * oct.max(0.0));
            let idx = (q.index + q.index_vel * v2) * ik;
            s.i_sus[k] = idx * q.index_sus;
            s.i_dec[k] = idx * (1.0 - q.index_sus);
            s.i_coef[k] = t60_coef(q.index_t60 * kd.sqrt(), rate);
            s.fb[k] = q.fb;
            let lv = q.level * super::util::db(q.level_key * oct);
            let (a, b) = crate::voice::pan_gains(q.pan);
            s.gl[k] = lv * a * std::f32::consts::SQRT_2;
            s.gr[k] = lv * b * std::f32::consts::SQRT_2;
            // DX-style: carriers start in sine phase 0 (consistent attack), modulators random-free.
            s.pc[k] = 0.0;
            s.pm[k] = 0.0;
        }
        s.set_freq(0.0);
        s
    }

    fn set_freq(&mut self, cents: f32) {
        let f = self.f0 * (cents / 1200.0).exp2() / self.rate;
        for k in 0..MAXP {
            self.dc[k] = (f * self.base[k]).min(0.49);
            self.dm[k] = (f * self.base_m[k]).min(0.49);
        }
    }
}

impl Voice for FmVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if crate::synth::util::avx() {
            // SAFETY: AVX support was checked at run time.
            return unsafe { self.render_avx(l, r) };
        }
        self.render_body(l, r)
    }

    fn release(&mut self) {
        self.released = true;
    }

    fn kill(&mut self) {
        self.released = true;
        self.out.kill(self.rate);
    }
}

impl FmVoice {
    /// The render body compiled with AVX (8-wide vector loops).
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx")]
    fn render_avx(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        self.render_body(l, r)
    }

    #[inline(always)]
    fn render_body(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        let wob = self.wow_c * self.wow.step(n, self.rate)
            + self.flutter_c * self.flutter.step(n, self.rate);
        if self.wow_c != 0.0 || self.flutter_c != 0.0 {
            self.set_freq(wob);
        }
        let mut bl = [0.0f32; crate::voice::BLOCK];
        let mut br = [0.0f32; crate::voice::BLOCK];
        let inv_n = 1.0 / n as f32;
        // Per pair, one vectorisable pass over the block: phases are computed
        // directly from the block start, the index envelope is interpolated
        // linearly across the block (it decays by < 20 % per block).
        let mut sum = [0.0f32; crate::voice::BLOCK];
        for k in 0..MAXP {
            let (gl, gr) = (self.gl[k], self.gr[k]);
            if gl == 0.0 && gr == 0.0 {
                continue;
            }
            let (tc0, dc, tm0, dm) = (self.pc[k], self.dc[k], self.pm[k], self.dm[k]);
            let i0 = (self.i_sus[k] + self.i_dec[k]) * (1.0 / std::f32::consts::TAU);
            let dec1 = self.i_dec[k] * self.i_coef[k].powi(n as i32);
            let i1 = (self.i_sus[k] + dec1) * (1.0 / std::f32::consts::TAU);
            let di = (i1 - i0) * inv_n;
            let s = &mut sum[..n];
            if self.fb[k] == 0.0 {
                for (i, o) in s.iter_mut().enumerate() {
                    let f = (i + 1) as f32;
                    let m = sin2pi_unit(fract_pos(tm0 + f * dm));
                    // +8 keeps the argument positive for indices up to ≈ 50 rad.
                    *o = sin2pi_unit(fract_pos(tc0 + f * dc + m * (i0 + di * f) + 8.0));
                }
            } else {
                let fb = self.fb[k];
                let mut prev = self.prev[k];
                for (i, o) in s.iter_mut().enumerate() {
                    let f = (i + 1) as f32;
                    let m = sin2pi(tm0 + f * dm + fb * prev);
                    prev = m;
                    *o = sin2pi_unit(fract_pos(tc0 + f * dc + m * (i0 + di * f) + 8.0));
                }
                self.prev[k] = prev;
            }
            for i in 0..n {
                bl[i] += s[i] * gl;
                br[i] += s[i] * gr;
            }
            self.pc[k] = fract_pos(tc0 + n as f32 * dc);
            self.pm[k] = fract_pos(tm0 + n as f32 * dm);
            self.i_dec[k] = if dec1.abs() < 1e-12 { 0.0 } else { dec1 };
        }
        // Amplitude: two decays (interpolated per block) and a linear attack.
        let (cf, cs) = if self.released {
            (self.c_rel, self.c_rel)
        } else {
            (self.c_fast, self.c_slow)
        };
        let e0 = self.e_fast + self.e_slow;
        self.e_fast = super::util::flush(self.e_fast * cf.powi(n as i32));
        self.e_slow = super::util::flush(self.e_slow * cs.powi(n as i32));
        let de = (self.e_fast + self.e_slow - e0) * inv_n;
        let (a0, ast) = (self.att, self.att_step);
        for i in 0..n {
            let f = (i + 1) as f32;
            let e = (e0 + de * f) * super::util::fmin(a0 + ast * f, 1.0);
            bl[i] *= e;
            br[i] *= e;
        }
        self.att = (a0 + ast * n as f32).min(1.0);
        let (ef, es) = (self.e_fast, self.e_slow);
        let (bl, br) = (&mut bl[..n], &mut br[..n]);
        // Stereo auto-pan / tremolo.
        if self.trem_depth > 0.0 {
            let t = self.trem.step(n, self.rate) * self.trem_depth;
            let (a, b) = (1.0 + t, 1.0 - t);
            for i in 0..n {
                bl[i] *= a;
                br[i] *= b;
            }
        }
        if self.hiss > 0.0 {
            let h = self.hiss * (ef + es).min(1.0).sqrt();
            for i in 0..n {
                let z = self.noise.next() * h;
                bl[i] += z;
                br[i] += z;
            }
        }
        if self.drive > 0.0 {
            let d = self.drive;
            let post = 1.0 / tanh(d);
            for x in bl.iter_mut().chain(br.iter_mut()) {
                *x = tanh(*x * d) * post;
            }
        }
        if self.lp_g > 0.0 {
            let g = self.lp_g;
            for (c, buf) in [&mut *bl, &mut *br].into_iter().enumerate() {
                let [a, b] = &mut self.lp[c];
                for x in buf.iter_mut() {
                    *x = b.lp(a.lp(*x, g), g);
                }
            }
        }
        self.out.stereo(bl, br, l, r);
        let level = (self.e_fast + self.e_slow) * self.out.g;
        !(level < 1e-5 || self.out.dead())
    }
}
