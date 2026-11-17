//! The subtractive (virtual-analog) synth voice: up to three oscillator banks
//! (unison / supersaw lanes), sub and noise, a ladder / SVF / formant filter
//! with its own envelope, an amp envelope, LFO (vibrato, filter, tremolo,
//! PWM), pitch envelope, analog drift, ensemble and mono glide.

use super::env::{Adsr, Glide, GlideLink, GlideState, Lfo, LfoShape};
use super::filter::{Ladder, LadderCoef, OnePole, Svf, SvfCoef};
use super::osc::{Drift, Lanes, Wave, LANES};
use super::util::{prewarp, semis, sin2pi, tanh, vel_gain, Noise, Out};
use crate::voice::{key_to_freq, Instrument, NoteOn, Rng, Voice};
use std::sync::Arc;

/// Control-rate sub-block (samples); halved while the pitch is moving
/// (pitch envelope, glide) so sweeps stay smooth.
const CTRL: usize = 64;
const CTRL_FAST: usize = 16;

/// One oscillator bank.
#[derive(Clone, Copy, Debug)]
pub struct Osc {
    pub wave: Wave,
    /// Unison lanes (1..=8); ignored for supersaw.
    pub voices: usize,
    /// Unison spread (± cents).
    pub cents: f32,
    /// Stereo width of the lanes (0..1).
    pub width: f32,
    /// Transpose in semitones.
    pub semis: f32,
    pub level: f32,
    /// Pulse width.
    pub pw: f32,
    /// Supersaw (detune, mix) instead of plain unison.
    pub supersaw: Option<(f32, f32)>,
}

impl Osc {
    pub const fn new(wave: Wave, voices: usize, cents: f32, level: f32) -> Osc {
        Osc {
            wave,
            voices,
            cents,
            width: 0.0,
            semis: 0.0,
            level,
            pw: 0.5,
            supersaw: None,
        }
    }
    pub const fn width(mut self, w: f32) -> Osc {
        self.width = w;
        self
    }
    pub const fn semis(mut self, s: f32) -> Osc {
        self.semis = s;
        self
    }
    pub const fn pw(mut self, pw: f32) -> Osc {
        self.pw = pw;
        self
    }
    pub const fn supersaw(detune: f32, mix: f32, width: f32, level: f32) -> Osc {
        Osc {
            wave: Wave::Saw,
            voices: 7,
            cents: 0.0,
            width,
            semis: 0.0,
            level,
            pw: 0.5,
            supersaw: Some((detune, mix)),
        }
    }
}

/// Filter type.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FilterKind {
    /// 4-pole ladder low-pass.
    Ladder,
    /// 2-pole SVF low-pass.
    Lp,
    /// 4-pole low-pass from two cascaded SVFs (Butterworth Qs; lighter than the ladder).
    Lp4,
    /// 2-pole SVF band-pass.
    Bp,
    /// Three-formant vowel filter ("ah" ↔ "oh" morph by the LFO).
    Formant,
}

/// ADSR times.
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

pub const fn env(a: f32, d: f32, s: f32, r: f32) -> Env {
    Env { a, d, s, r }
}

/// A subtractive patch.
#[derive(Clone, Debug)]
pub struct Patch {
    pub osc: [Option<Osc>; 3],
    /// Sine sub-oscillator one octave down.
    pub sub: f32,
    pub noise: f32,
    pub filter: FilterKind,
    /// Base cutoff (Hz) at middle C, velocity 0.7, envelope 0.
    pub cutoff: f32,
    pub res: f32,
    /// Ladder drive (1 = clean-ish, 3 = gritty).
    pub drive: f32,
    /// Cutoff key tracking (0..1).
    pub track: f32,
    /// Octaves of cutoff per unit velocity (around 0.7).
    pub vel_cut: f32,
    /// Filter envelope depth (octaves).
    pub env_amt: f32,
    pub fenv: Env,
    pub amp: Env,
    /// Amp decay/release scale per octave above middle C (<1 shortens high notes).
    pub key_decay: f32,
    /// How strongly velocity scales level (0 = organ-like, 1 = full).
    pub vel_amt: f32,
    pub lfo_rate: f32,
    pub lfo_shape: LfoShape,
    pub lfo_delay: f32,
    pub vib: f32,
    pub lfo_cut: f32,
    pub trem: f32,
    pub pwm: f32,
    /// Pitch envelope: start offset (semitones) and time constant (s).
    pub pitch_env: (f32, f32),
    pub glide: f32,
    /// Glide on every note (portamento) instead of legato only.
    pub glide_always: bool,
    pub mono: bool,
    /// Analog drift (cents).
    pub drift: f32,
    /// Ensemble chorus: per-lane pitch wobble depth (cents).
    pub ensemble: f32,
    /// Process left/right separately (wide pads) instead of a mono path.
    pub stereo: bool,
    /// Post high-pass (Hz, 0 = off).
    pub hp: f32,
    /// Post saturation amount (0 = off).
    pub sat: f32,
    /// 303-style accent on loud notes (velocity ≥ 0.8).
    pub accent: bool,
    /// Allow 2× oversampled oscillators for high notes (bright, open-filter
    /// patches; dark patches filter their aliases away anyway).
    pub oversample: bool,
    pub gain: f32,
    pub pan: f32,
}

impl Patch {
    /// A neutral starting point.
    pub const fn base() -> Patch {
        Patch {
            osc: [None, None, None],
            sub: 0.0,
            noise: 0.0,
            filter: FilterKind::Ladder,
            cutoff: 2000.0,
            res: 0.1,
            drive: 1.0,
            track: 0.5,
            vel_cut: 1.0,
            env_amt: 0.0,
            fenv: env(0.005, 0.3, 0.0, 0.3),
            amp: env(0.005, 1.0, 1.0, 0.3),
            key_decay: 1.0,
            vel_amt: 1.0,
            lfo_rate: 5.0,
            lfo_shape: LfoShape::Sine,
            lfo_delay: 0.0,
            vib: 0.0,
            lfo_cut: 0.0,
            trem: 0.0,
            pwm: 0.0,
            pitch_env: (0.0, 0.0),
            glide: 0.0,
            glide_always: false,
            mono: false,
            drift: 1.2,
            ensemble: 0.0,
            stereo: false,
            hp: 0.0,
            sat: 0.0,
            accent: false,
            oversample: false,
            gain: 1.0,
            pan: 0.0,
        }
    }
}

/// A subtractive synth instrument.
pub struct SubSynth {
    pub patch: Patch,
    glide: Arc<GlideState>,
}

impl SubSynth {
    pub fn new(patch: Patch) -> SubSynth {
        SubSynth {
            patch,
            glide: Arc::new(GlideState::default()),
        }
    }
}

/// Run a per-channel filter closure over one or both channels; stereo runs
/// both channels in one loop so their recurrences overlap.
#[inline(always)]
fn filt2<F: Fn(&mut [Svf; 3], f32, usize) -> f32>(
    sl: &mut [[Svf; 3]; 2],
    bl: &mut [f32],
    br: &mut [f32],
    stereo: bool,
    f: F,
) {
    let [a, b] = sl;
    if stereo {
        for i in 0..bl.len() {
            bl[i] = f(a, bl[i], i);
            br[i] = f(b, br[i], i);
        }
    } else {
        for (i, x) in bl.iter_mut().enumerate() {
            *x = f(a, *x, i);
        }
    }
}

impl SubSynth {
    /// A voice playing `patch` instead of this synth's own (per-note
    /// automation), sharing this synth's glide state.
    pub fn voice_with(&self, patch: &Patch, note: &NoteOn) -> Box<dyn Voice> {
        Box::new(SubVoice::new(patch, note, if patch.mono || patch.glide > 0.0 { Some(&self.glide) } else { None }))
    }
}

impl Instrument for SubSynth {
    fn voice(&self, note: &NoteOn) -> Box<dyn Voice> {
        Box::new(SubVoice::new(
            &self.patch,
            note,
            if self.patch.mono || self.patch.glide > 0.0 {
                Some(&self.glide)
            } else {
                None
            },
        ))
    }
    fn mono(&self) -> bool {
        self.patch.mono
    }
    fn ordered_voices(&self) -> bool {
        self.patch.mono || self.patch.glide > 0.0
    }
}

const VOWEL_A: [(f32, f32, f32); 3] =
    [(800.0, 1.0, 9.0), (1150.0, 0.5, 11.0), (2900.0, 0.12, 14.0)];
const VOWEL_O: [(f32, f32, f32); 3] =
    [(450.0, 1.0, 9.0), (800.0, 0.35, 11.0), (2830.0, 0.08, 14.0)];

struct SubVoice {
    p: Patch,
    rate: f32,
    vel: f32,
    banks: Vec<Lanes>,
    noise: Noise,
    noise_amt: f32,
    drift: Drift,
    lfo: Lfo,
    ens_phase: [f32; LANES],
    glide: Glide,
    link: Option<GlideLink>,
    pe: f32,
    pe_coef: f32,
    aenv: Adsr,
    fenv: Adsr,
    fl: [Ladder; 2],
    sl: [[Svf; 3]; 2],
    hp: [OnePole; 2],
    hp_g: f32,
    lc: LadderCoef,
    sc: [SvfCoef; 3],
    sc_init: bool,
    sc_gain: [f32; 3],
    base_cut: f32,
    env_amt: f32,
    /// Ladder drive, eased off for high notes (saturating their upper
    /// harmonics would alias).
    drive: f32,
    out: Out,
}

impl SubVoice {
    fn new(p: &Patch, n: &NoteOn, glide: Option<&Arc<GlideState>>) -> SubVoice {
        let rate = n.rate;
        let mut rng = Rng::new(n.seed);
        let mut banks = Vec::with_capacity(4);
        for o in p.osc.iter().flatten() {
            let mut b = match o.supersaw {
                Some((d, m)) => Lanes::supersaw(d, m, o.width, o.level, &mut rng),
                None => Lanes::unison(
                    o.wave,
                    o.voices,
                    o.cents,
                    o.width,
                    semis(o.semis),
                    o.level,
                    &mut rng,
                ),
            };
            b.pw = o.pw;
            if !p.oversample {
                b.oversample(false);
            }
            banks.push(b);
        }
        if p.sub > 0.0 {
            let mut b = Lanes::unison(Wave::Sine, 1, 0.0, 0.0, 0.5, p.sub, &mut rng);
            b.oversample(false);
            banks.push(b);
        }
        let accent = p.accent && n.vel >= 0.8;
        let oct = (n.key - 60.0) / 12.0;
        let kd = p.key_decay.powf(oct);
        let mut amp = p.amp;
        amp.d *= kd;
        amp.r *= kd.sqrt();
        let mut fe = p.fenv;
        if accent {
            fe.d *= 0.55;
        }
        let (from, link) = match glide {
            Some(g) => {
                let (f, l) = GlideState::start(g, n.key, p.glide_always);
                (f, Some(l))
            }
            None => (None, None),
        };
        let gl = Glide::new(
            from.unwrap_or(n.key),
            n.key,
            if from.is_some() { p.glide } else { 0.0 },
            rate,
            1,
        );
        let f0 = key_to_freq(n.key);
        let base_cut = p.cutoff * (f0 / 261.63).powf(p.track) * (p.vel_cut * (n.vel - 0.7)).exp2();
        let env_amt = p.env_amt
            * if accent {
                1.6
            } else {
                0.6 + 0.4 * n.vel.min(1.0)
            };
        // High notes pass more harmonics through key-tracked filters: keep them level.
        let key_level = if p.filter == FilterKind::Ladder {
            (523.25 / f0).sqrt().min(1.0)
        } else {
            1.0
        };
        let level = (1.0 - p.vel_amt + p.vel_amt * vel_gain(n.vel))
            * p.gain
            * key_level
            * if accent { 1.18 } else { 1.0 };
        let mut v = SubVoice {
            rate,
            vel: n.vel,
            banks,
            noise: Noise::new(n.seed ^ 0xABCD),
            noise_amt: p.noise,
            drift: Drift::new(n.seed, p.drift),
            lfo: Lfo::new(
                p.lfo_rate,
                p.lfo_shape,
                p.lfo_delay,
                p.lfo_delay.max(0.05) * 1.5,
                rng.f32(),
            ),
            ens_phase: core::array::from_fn(|k| k as f32 / LANES as f32 + rng.f32() * 0.1),
            glide: gl,
            link,
            pe: p.pitch_env.0 * if accent { 1.3 } else { 1.0 },
            pe_coef: super::util::pole(p.pitch_env.1, rate),
            aenv: Adsr::new(amp.a, amp.d, amp.s, amp.r, rate),
            fenv: Adsr::new(fe.a, fe.d, fe.s, fe.r, rate),
            fl: [Ladder::default(); 2],
            sl: [[Svf::default(); 3]; 2],
            hp: [OnePole::default(); 2],
            hp_g: if p.hp > 0.0 {
                OnePole::coef(prewarp(p.hp, rate))
            } else {
                0.0
            },
            lc: LadderCoef::default(),
            sc: [SvfCoef::default(); 3],
            sc_init: false,
            sc_gain: [0.0; 3],
            base_cut,
            env_amt,
            drive: p.drive * (700.0 / f0).powf(0.9).clamp(0.3, 1.0),
            out: Out::new(level, p.pan),
            p: p.clone(),
        };
        v.lc = v.ladder_coef(0.0);
        v
    }

    #[inline]
    fn cutoff(&self, lfo: f32) -> f32 {
        let oct = self.env_amt * self.fenv.level + self.p.lfo_cut * lfo;
        (self.base_cut * oct.exp2()).clamp(20.0, 0.45 * self.rate)
    }

    #[inline]
    fn ladder_coef(&self, lfo: f32) -> LadderCoef {
        LadderCoef::new(prewarp(self.cutoff(lfo), self.rate), self.p.res)
    }
}

impl Voice for SubVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if crate::synth::util::avx() {
            // SAFETY: AVX support was checked at run time.
            return unsafe { self.render_avx(l, r) };
        }
        self.render_body(l, r)
    }

    fn release(&mut self) {
        self.aenv.release();
        self.fenv.release();
        if let Some(link) = &self.link {
            link.release();
        }
    }

    fn kill(&mut self) {
        self.release();
        self.out.kill(self.rate);
    }
}

impl SubVoice {
    /// The render body compiled with AVX (8-wide vector loops).
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx")]
    fn render_avx(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        self.render_body(l, r)
    }

    #[inline(always)]
    fn render_body(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        if let Some(link) = &self.link {
            link.rendered();
        }
        let mut off = 0;
        let stereo = self.p.stereo;
        while off < n {
            let moving = self.pe.abs() > 0.02 || (self.glide.key - self.glide.target).abs() > 0.02;
            let m = if moving { CTRL_FAST } else { CTRL }.min(n - off);
            // --- control rate ---
            let lfo = self.lfo.step(m, self.rate);
            let key = self.glide.step(m) + self.pe + self.p.vib / 100.0 * lfo;
            self.pe = super::util::flush(self.pe * self.pe_coef.powi(m as i32));
            let f = key_to_freq(key);
            let mut lm = [1.0f32; LANES];
            self.drift.step(&mut lm);
            if self.p.ensemble > 0.0 {
                let dt = m as f32 / self.rate;
                for k in 0..LANES {
                    // Two incommensurate wobbles per lane (string-machine ensemble).
                    self.ens_phase[k] =
                        super::util::fract(self.ens_phase[k] + dt * (0.63 + 0.11 * k as f32));
                    let w = sin2pi(self.ens_phase[k]) + 0.4 * sin2pi(self.ens_phase[k] * 9.7);
                    lm[k] *= 1.0 + w * self.p.ensemble * (std::f32::consts::LN_2 / 1200.0);
                }
            }
            for b in &mut self.banks {
                b.set_freq(f, self.rate, &lm);
                b.pw = (b.pw + 0.0).clamp(0.05, 0.95);
            }
            if self.p.pwm != 0.0 {
                let pw0 = self
                    .p
                    .osc
                    .iter()
                    .flatten()
                    .map(|o| o.pw)
                    .next()
                    .unwrap_or(0.5);
                for b in &mut self.banks {
                    if b.wave == Wave::Pulse {
                        b.pw = (pw0 + self.p.pwm * lfo).clamp(0.05, 0.95);
                    }
                }
            }
            // --- oscillators ---
            let mut xl = [0.0f32; CTRL];
            let mut xr = [0.0f32; CTRL];
            let (bl, br) = (&mut xl[..m], &mut xr[..m]);
            for b in &mut self.banks {
                if stereo {
                    b.render(bl, br);
                } else {
                    b.render_mono(bl);
                }
            }
            if self.noise_amt > 0.0 {
                let a = self.noise_amt;
                for i in 0..m {
                    bl[i] += self.noise.next() * a;
                }
                if stereo {
                    for i in 0..m {
                        br[i] += self.noise.next() * a;
                    }
                }
            }
            // --- filter envelope (advanced at audio rate, applied per sub-block) ---
            self.fenv.skip(m);
            let chans = if stereo { 2 } else { 1 };
            match self.p.filter {
                FilterKind::Ladder => {
                    let c1 = self.ladder_coef(lfo);
                    let c0 = self.lc;
                    let drive = self.drive;
                    let inv_m = 1.0 / m as f32;
                    let [f0, f1] = &mut self.fl;
                    if stereo {
                        // Both channels in one loop: two independent recurrences overlap.
                        for i in 0..m {
                            let cc = c0.lerp(&c1, (i + 1) as f32 * inv_m);
                            bl[i] = f0.tick(bl[i], &cc, drive);
                            br[i] = f1.tick(br[i], &cc, drive);
                        }
                    } else {
                        for (i, x) in bl.iter_mut().enumerate() {
                            let cc = c0.lerp(&c1, (i + 1) as f32 * inv_m);
                            *x = f0.tick(*x, &cc, drive);
                        }
                    }
                    self.lc = c1;
                }
                FilterKind::Lp | FilterKind::Bp | FilterKind::Lp4 => {
                    // Coefficients interpolated per sample from the previous sub-block.
                    let g = prewarp(self.cutoff(lfo), self.rate);
                    let res = self.p.res;
                    let (n1, n2) = if self.p.filter == FilterKind::Lp4 {
                        // Two cascaded 2-poles with 4th-order Butterworth Qs; resonance on the second.
                        (
                            SvfCoef::new(g, 0.541_2),
                            SvfCoef::new(g, 1.306_6 + res * 6.0),
                        )
                    } else {
                        (SvfCoef::new(g, 0.5 + res * 8.0), SvfCoef::default())
                    };
                    let (o1, o2) = if self.sc_init {
                        (self.sc[0], self.sc[1])
                    } else {
                        (n1, n2)
                    };
                    self.sc = [n1, n2, SvfCoef::default()];
                    self.sc_init = true;
                    let inv_m = 1.0 / m as f32;
                    let co = |i: usize| -> (SvfCoef, SvfCoef) {
                        let t = (i + 1) as f32 * inv_m;
                        (o1.lerp(&n1, t), o2.lerp(&n2, t))
                    };
                    match self.p.filter {
                        FilterKind::Lp => {
                            filt2(&mut self.sl, bl, br, stereo, |f, x, i| f[0].lp(x, &co(i).0))
                        }
                        FilterKind::Bp => {
                            filt2(&mut self.sl, bl, br, stereo, |f, x, i| f[0].bp(x, &co(i).0))
                        }
                        _ => filt2(&mut self.sl, bl, br, stereo, |f, x, i| {
                            let (c1, c2) = co(i);
                            let y = f[0].lp(x, &c1);
                            f[1].lp(y, &c2)
                        }),
                    }
                }
                FilterKind::Formant => {
                    let t = 0.5 + 0.5 * lfo;
                    for k in 0..3 {
                        let (fa, ga, qa) = VOWEL_A[k];
                        let (fo, go, _) = VOWEL_O[k];
                        let fc = fa + (fo - fa) * t;
                        self.sc[k] = SvfCoef::new(prewarp(fc, self.rate), qa);
                        self.sc_gain[k] = (ga + (go - ga) * t) * 2.5 * qa / VOWEL_A[0].2;
                    }
                    let (sc, sg) = (self.sc, self.sc_gain);
                    let k0 = sc[0].k;
                    filt2(&mut self.sl, bl, br, stereo, |fs, v, _| {
                        (fs[0].bp(v, &sc[0]) * sg[0]
                            + fs[1].bp(v, &sc[1]) * sg[1]
                            + fs[2].bp(v, &sc[2]) * sg[2])
                            * k0
                            + v * 0.04
                    });
                }
            }
            // --- post: high-pass, saturation, amp envelope, tremolo ---
            let trem = 1.0 + self.p.trem * lfo;
            let sat = self.p.sat;
            let hg = self.hp_g;
            for (c, buf) in [&mut *bl, &mut *br].into_iter().enumerate().take(chans) {
                if hg > 0.0 {
                    let h = &mut self.hp[c];
                    for x in buf.iter_mut() {
                        *x = h.hp(*x, hg);
                    }
                }
                if sat > 0.0 {
                    let pre = 1.0 + sat * 3.0;
                    let post = 1.0 / tanh(pre).max(0.1) / (1.0 + sat);
                    for x in buf.iter_mut() {
                        *x = tanh(*x * pre) * post;
                    }
                }
            }
            let mut ae = [0.0f32; CTRL];
            self.aenv.fill(&mut ae[..m]);
            for i in 0..m {
                bl[i] *= ae[i] * trem;
            }
            if stereo {
                for i in 0..m {
                    br[i] *= ae[i] * trem;
                }
                self.out
                    .stereo(bl, br, &mut l[off..off + m], &mut r[off..off + m]);
            } else {
                self.out
                    .mono(bl, &mut l[off..off + m], &mut r[off..off + m]);
            }
            off += m;
        }
        for f in &mut self.fl {
            f.flush();
        }
        let _ = self.vel;
        !(self.aenv.done() || self.out.dead())
    }
}
