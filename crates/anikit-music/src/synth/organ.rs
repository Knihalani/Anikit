//! Drawbar tonewheel organ: nine tempered tonewheel sines (with high-key
//! foldback), key click, harmonic percussion, a rotary-speaker style stereo
//! modulation and optional overdrive. Also drives the M1-style house stab.

use super::env::Adsr;
use super::filter::{OnePole, Svf, SvfCoef};
use super::util::{fract, fract_pos, prewarp, sin2pi, sin2pi_unit, tanh, vel_gain, Noise, Out};
use crate::voice::{key_to_freq, Instrument, NoteOn, Rng, Voice};

/// Drawbar footages as tempered tonewheel ratios: 16′ 5⅓′ 8′ 4′ 2⅔′ 2′ 1⅗′ 1⅓′ 1′.
pub const FOOTAGES: [f32; 9] = [
    0.5, 1.498_307, 1.0, 2.0, 2.996_614, 4.0, 5.039_684, 5.993_228, 8.0,
];

/// An organ patch.
#[derive(Clone, Debug)]
pub struct OrganPatch {
    /// Drawbar settings 0..8.
    pub drawbars: [f32; 9],
    /// Percussion: drawbar index (3 = 4′ "second", 4 = 2⅔′ "third"), level, decay (−60 dB s).
    pub perc: (usize, f32, f32),
    /// Key-click level.
    pub click: f32,
    /// Rotary: speed (Hz) and depth (0..1).
    pub rotary: (f32, f32),
    pub amp: super::patch::Env,
    pub vel_amt: f32,
    pub drive: f32,
    /// Low-pass (Hz, 0 = off).
    pub lp: f32,
    /// Velocity → brightness: octaves of low-pass and extra drive per unit velocity
    /// (0 for a real organ, which has no touch sensitivity).
    pub vel_bright: f32,
    /// Detune between left and right tonewheel sets (cents, for chorus).
    pub chorus: f32,
    pub gain: f32,
}

/// An organ instrument.
pub struct Organ(pub OrganPatch);

impl Instrument for Organ {
    fn voice(&self, n: &NoteOn) -> Box<dyn Voice> {
        Box::new(OrganVoice::new(&self.0, n))
    }
}

const L: usize = 12;

struct OrganVoice {
    rate: f32,
    ph: [f32; L],
    ph2: [f32; L],
    dt: [f32; L],
    amp: [f32; L],
    perc_lane: usize,
    perc_coef: f32,
    click: Vec<f32>,
    click_pos: usize,
    click_gain: f32,
    rot: f32,
    rot_dt: f32,
    rot_depth: f32,
    env: Adsr,
    drive: f32,
    lp: [OnePole; 2],
    lp_g: f32,
    out: Out,
    chorus: f32,
}

impl OrganVoice {
    fn new(p: &OrganPatch, n: &NoteOn) -> OrganVoice {
        let rate = n.rate;
        let mut rng = Rng::new(n.seed);
        let f0 = key_to_freq(n.key);
        let mut ph = [0.0; L];
        let mut dt = [0.0; L];
        let mut amp = [0.0; L];
        let total: f32 = p
            .drawbars
            .iter()
            .map(|d| (d / 8.0).powf(1.5))
            .sum::<f32>()
            .max(1.0);
        for k in 0..9 {
            let mut f = f0 * FOOTAGES[k];
            // Tonewheel foldback: the top wheels repeat their last octave.
            while f > 5900.0 {
                f *= 0.5;
            }
            dt[k] = f / rate;
            // Drawbar steps are ≈3 dB apart.
            amp[k] = if p.drawbars[k] > 0.0 {
                super::util::db(-3.0 * (8.0 - p.drawbars[k])) / total.sqrt()
            } else {
                0.0
            };
            ph[k] = rng.f32();
        }
        // Percussion on its own lane (phase-locked to its drawbar's wheel).
        let pl = 9;
        let (pi, plev, pdec) = p.perc;
        if plev > 0.0 {
            dt[pl] = dt[pi.min(8)];
            ph[pl] = ph[pi.min(8)];
            amp[pl] = plev * (0.6 + 0.4 * n.vel) / total.sqrt();
        }
        // Key click: a short burst of filtered noise plus a contact transient.
        let len = (0.006 * rate) as usize;
        let mut click = Vec::with_capacity(len);
        let mut nz = Noise::new(n.seed ^ 0xC11C);
        let mut bp = Svf::default();
        let bc = SvfCoef::new(prewarp(rng.range(1800.0, 3200.0), rate), 1.2);
        for i in 0..len {
            let e = (1.0 - i as f32 / len as f32).powi(3);
            click.push(bp.bp(nz.next(), &bc) * e);
        }
        let level = (1.0 - p.vel_amt + p.vel_amt * vel_gain(n.vel)) * p.gain;
        let mut ph2 = ph;
        for p in &mut ph2 {
            *p = fract(*p + rng.f32());
        }
        OrganVoice {
            rate,
            ph,
            ph2,
            dt,
            amp,
            perc_lane: pl,
            perc_coef: super::util::t60_coef(pdec.max(0.01), rate / crate::voice::BLOCK as f32),
            click,
            click_pos: 0,
            click_gain: p.click,
            rot: rng.f32(),
            rot_dt: p.rotary.0 / rate,
            rot_depth: p.rotary.1,
            env: Adsr::new(p.amp.a, p.amp.d, p.amp.s, p.amp.r, rate),
            drive: p.drive * (1.0 + p.vel_bright * 0.5 * (n.vel - 0.7)).max(0.2),
            lp: [OnePole::default(); 2],
            lp_g: if p.lp > 0.0 {
                OnePole::coef(prewarp(p.lp * (p.vel_bright * (n.vel - 0.7)).exp2(), rate))
            } else {
                0.0
            },
            out: Out::new(level, 0.0),
            chorus: p.chorus,
        }
    }
}

/// Add the sounding tonewheels into `out`, one wheel at a time (vectorises across samples).
#[inline(always)]
fn wheels(ph: &mut [f32; L], dt: &[f32; L], amp: &[f32; L], det: f32, out: &mut [f32]) {
    let n = out.len();
    for k in 0..L {
        if amp[k] == 0.0 {
            continue;
        }
        let (t0, d, a) = (ph[k], dt[k] * det, amp[k]);
        for (i, o) in out.iter_mut().enumerate() {
            *o += sin2pi_unit(fract_pos(t0 + (i + 1) as f32 * d)) * a;
        }
        ph[k] = fract_pos(t0 + n as f32 * d);
    }
}

impl Voice for OrganVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if crate::synth::util::avx() {
            // SAFETY: AVX support was checked at run time.
            return unsafe { self.render_avx(l, r) };
        }
        self.render_body(l, r)
    }

    fn release(&mut self) {
        if !self.env.releasing() {
            // Release click (contacts opening), quieter than the attack click.
            self.click_pos = self.click.len() / 3;
            self.click_gain *= 0.4;
        }
        self.env.release();
    }

    fn kill(&mut self) {
        self.env.release();
        self.out.kill(self.rate);
    }
}

impl OrganVoice {
    /// The render body compiled with AVX (8-wide vector loops).
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx")]
    fn render_avx(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        self.render_body(l, r)
    }

    #[inline(always)]
    fn render_body(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        let mut x = [0.0f32; crate::voice::BLOCK];
        let mut y = [0.0f32; crate::voice::BLOCK];
        // Chorus: left and right read the wheels detuned by ±chorus/2 cents.
        let det = 1.0 + 0.5 * self.chorus * (std::f32::consts::LN_2 / 1200.0);
        let det_l = 2.0 - det;
        wheels(&mut self.ph, &self.dt, &self.amp, det_l, &mut x[..n]);
        if self.chorus > 0.0 {
            wheels(&mut self.ph2, &self.dt, &self.amp, det, &mut y[..n]);
        } else {
            y[..n].copy_from_slice(&x[..n]);
        }
        self.amp[self.perc_lane] = super::util::flush(
            self.amp[self.perc_lane] * self.perc_coef.powf(n as f32 / crate::voice::BLOCK as f32),
        );
        // Click.
        if self.click_pos < self.click.len() {
            let c = self.click_gain;
            let end = (self.click_pos + n).min(self.click.len());
            for (i, v) in self.click[self.click_pos..end].iter().enumerate() {
                x[i] += v * c;
                y[i] += v * c;
            }
            self.click_pos = end;
        }
        // Amp envelope + drive.
        let mut e = [0.0f32; crate::voice::BLOCK];
        self.env.fill(&mut e[..n]);
        let d = self.drive;
        let post = if d > 0.0 { 1.0 / tanh(d) } else { 1.0 };
        for i in 0..n {
            if d > 0.0 {
                x[i] = tanh(x[i] * d) * post;
                y[i] = tanh(y[i] * d) * post;
            }
            x[i] *= e[i];
            y[i] *= e[i];
        }
        if self.lp_g > 0.0 {
            let g = self.lp_g;
            for i in 0..n {
                x[i] = self.lp[0].lp(x[i], g);
                y[i] = self.lp[1].lp(y[i], g);
            }
        }
        // Rotary: opposing amplitude modulation (horn left/right).
        if self.rot_depth > 0.0 {
            let a = sin2pi(self.rot) * self.rot_depth;
            self.rot = fract(self.rot + self.rot_dt * n as f32);
            let b = sin2pi(self.rot) * self.rot_depth;
            for i in 0..n {
                let m = a + (b - a) * (i as f32 / n as f32);
                x[i] *= 1.0 + m;
                y[i] *= 1.0 - m;
            }
        }
        self.out.stereo(&x[..n], &y[..n], l, r);
        !(self.env.done() || self.out.dead())
    }
}
