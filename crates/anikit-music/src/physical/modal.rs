//! Modal synthesis: a struck object as a bank of exponentially decaying
//! sinusoids. Each mode is a complex one-pole "rotator" `z ← z·r·e^{jω}`;
//! modes are stored in groups of four (structure of arrays) so the per-sample
//! loop is a handful of 4-wide multiply-adds per group.
//!
//! Partial ratios and decays are physically based (tuned bars 1:4:10 and 1:3,
//! free-free bars, clamped tines, minor-third church-bell partials, Rayleigh
//! tubular-bell partials, steelpan/handpan tuned notes, glass). The mallet's
//! contact time (from velocity: harder = shorter = brighter) weights the modes;
//! a short contact noise adds the strike. A tuned resonator tube appears as a
//! slowly rising mode at the fundamental. Doublets (degenerate modes split by
//! asymmetry) give bells their slow beating.

use crate::synth::filter::{Svf, SvfCoef};
use crate::synth::util::{prewarp, sin2pi, vel_gain, Noise, Out, SILENCE};
use crate::voice::{Instrument, NoteOn, Rng, Voice, BLOCK};
use std::f64::consts::PI;

/// Maximum modes per voice.
pub const MAX_MODES: usize = 16;
const G: usize = MAX_MODES / 4;

/// One mode: frequency ratio (negative = absolute Hz), relative amplitude,
/// decay multiplier (× the instrument's t60 at that frequency).
pub type Mode = (f32, f32, f32);

/// Parameters of a struck / plucked modal instrument.
#[derive(Clone, Debug)]
pub struct ModalSpec {
    pub modes: &'static [Mode],
    /// Frequency of ratio 1 relative to the key frequency (e.g. 0.224 for tubular bells
    /// so that the strike tone, half of mode 4, lands on the key).
    pub tune: f32,
    /// −60 dB decay at 261.6 Hz (s) and its scaling with frequency (`(f/261.6)^−slope`).
    pub t60: f32,
    pub slope: f32,
    /// Mallet contact time at velocity 0 and 1 (ms).
    pub contact: (f32, f32),
    /// Strike noise level, band centre (Hz) and length (ms).
    pub noise: (f32, f32, f32),
    /// Resonator tube: level, −60 dB time (s), rise time (s); 0 level = none.
    pub tube: (f32, f32, f32),
    /// Tremolo (Hz, depth 0..1).
    pub trem: (f32, f32),
    /// Doublet split (cents) for beating; 0 = none.
    pub beat: f32,
    /// Random per-mode detune (cents) — hand-made instruments.
    pub detune: f32,
    /// Damping at note-off (−60 dB s); 0 = let ring.
    pub release_t60: f32,
    /// Stereo spread of upper modes and pan spread across the keyboard.
    pub width: f32,
    pub spread: f32,
    pub gain: f32,
}

/// A modal instrument.
pub struct Modal(pub ModalSpec);

impl Instrument for Modal {
    fn voice(&self, n: &NoteOn) -> Box<dyn Voice> {
        Box::new(ModalVoice::new(&self.0, n))
    }
}

struct ModalVoice {
    rate: f32,
    re: [[f32; 4]; G],
    im: [[f32; 4]; G],
    cr: [[f32; 4]; G],
    ci: [[f32; 4]; G],
    wl: [[f32; 4]; G],
    wr: [[f32; 4]; G],
    radius: [[f32; 4]; G],
    groups: usize,
    noise: Vec<f32>,
    npos: usize,
    trem: (f32, f32),
    trem_ph: f32,
    r_rel: f32,
    released: bool,
    out: Out,
}

impl ModalVoice {
    fn new(s: &ModalSpec, n: &NoteOn) -> ModalVoice {
        let rate = n.rate as f64;
        let v = n.vel.clamp(0.0, 1.0) as f64;
        let mut rng = Rng::new(n.seed);
        let f1 = n.freq() as f64 * s.tune as f64;
        let tc = (s.contact.0 as f64 + (s.contact.1 as f64 - s.contact.0 as f64) * v) * 1e-3;
        // Candidate modes: (freq, amp, t60, pan).
        let mut cand: Vec<(f64, f64, f64, f32)> = Vec::with_capacity(MAX_MODES * 2);
        let t60_at = |f: f64| s.t60 as f64 * (f / 261.63).powf(-(s.slope as f64));
        for (k, &(ratio, amp, dm)) in s.modes.iter().enumerate() {
            let mut f = if ratio < 0.0 {
                -ratio as f64
            } else {
                f1 * ratio as f64
            };
            if s.detune > 0.0 && k > 0 {
                f *= (rng.bi() as f64 * s.detune as f64 / 1200.0).exp2();
            }
            if f >= 0.45 * rate || amp <= 0.0 {
                continue;
            }
            // Mallet spectrum: smooth low-pass with corner ≈ 1/contact time.
            let x = f * tc;
            let w = 1.0 / (1.0 + x * x * x * x).sqrt();
            let t60 = t60_at(f) * dm as f64;
            let pan = if k == 0 {
                0.0
            } else {
                rng.bi() * s.width * (f as f32 / 1500.0).min(1.0)
            };
            if s.beat > 0.0 && k < 6 {
                let d = (s.beat as f64 * (0.6 + 0.8 * rng.f32() as f64) / 2400.0).exp2();
                cand.push((f * d, amp as f64 * w * 0.5, t60, pan + 0.15 * s.width));
                cand.push((f / d, amp as f64 * w * 0.5, t60 * 0.9, pan - 0.15 * s.width));
            } else {
                cand.push((f, amp as f64 * w, t60, pan));
            }
        }
        // Tube resonator: a rising envelope = two same-frequency modes of opposite sign.
        let (ta, tt, trise) = s.tube;
        if ta > 0.0 && f1 < 0.45 * rate {
            let ff = n.freq() as f64;
            cand.push((ff, ta as f64, tt as f64, 0.0));
            cand.push((ff, -(ta as f64), trise as f64, 0.0));
        }
        // Keep the loudest MAX_MODES.
        if cand.len() > MAX_MODES {
            cand.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
            cand.truncate(MAX_MODES);
        }
        // Longest decays first, so the last groups die first and can be dropped.
        cand.sort_by(|a, b| b.2.total_cmp(&a.2));
        let groups = cand.len().div_ceil(4).max(1);
        let mut m = ModalVoice {
            rate: rate as f32,
            re: [[0.0; 4]; G],
            im: [[0.0; 4]; G],
            cr: [[0.0; 4]; G],
            ci: [[0.0; 4]; G],
            wl: [[0.0; 4]; G],
            wr: [[0.0; 4]; G],
            radius: [[0.0; 4]; G],
            groups,
            noise: Vec::new(),
            npos: 0,
            trem: (s.trem.0 / rate as f32, s.trem.1),
            trem_ph: rng.f32(),
            r_rel: if s.release_t60 > 0.0 {
                10f64.powf(-3.0 / (s.release_t60 as f64 * rate)) as f32
            } else {
                1.0
            },
            released: false,
            out: Out::new(0.0, 0.0),
        };
        let total: f64 = cand.iter().map(|c| c.1.abs()).sum::<f64>().max(1e-9);
        for (i, &(f, a, t60, pan)) in cand.iter().enumerate() {
            let (g, k) = (i / 4, i % 4);
            let r = 10f64.powf(-3.0 / (t60.max(0.005) * rate));
            let w = 2.0 * PI * f / rate;
            m.radius[g][k] = r as f32;
            m.cr[g][k] = (r * w.cos()) as f32;
            m.ci[g][k] = (r * w.sin()) as f32;
            // Start in sine phase (zero displacement, struck velocity).
            m.re[g][k] = (a / total.sqrt()) as f32;
            let (pl, pr) = crate::voice::pan_gains(pan);
            m.wl[g][k] = pl * std::f32::consts::SQRT_2;
            m.wr[g][k] = pr * std::f32::consts::SQRT_2;
        }
        // Strike noise.
        let (nl, nf, nms) = s.noise;
        if nl > 0.0 {
            let len = (nms as f64 * 1e-3 * rate) as usize;
            let mut nz = Noise::new(n.seed ^ 0x3A11);
            let mut f = Svf::default();
            let c = SvfCoef::new(prewarp(nf * (0.5 + v as f32), rate as f32), 0.9);
            let hard = (0.3 + 0.7 * v * v) as f32;
            m.noise = (0..len)
                .map(|i| {
                    let e = (-(i as f32) / (len as f32 * 0.25)).exp();
                    f.bp(nz.next(), &c) * e * nl * hard
                })
                .collect();
        }
        let key_pan = ((n.key - 66.0) / 36.0).clamp(-1.0, 1.0) * s.spread;
        m.out = Out::new(vel_gain(n.vel) * s.gain, key_pan);
        m
    }

    #[inline(always)]
    fn run<const NG: usize>(&mut self, xl: &mut [f32], xr: &mut [f32]) {
        let (mut re, mut im) = (self.re, self.im);
        let (cr, ci, wl, wr) = (self.cr, self.ci, self.wl, self.wr);
        for (a, b) in xl.iter_mut().zip(xr.iter_mut()) {
            let mut sl = [0.0f32; 4];
            let mut sr = [0.0f32; 4];
            for g in 0..NG {
                for k in 0..4 {
                    let (x, y) = (re[g][k], im[g][k]);
                    let nx = x * cr[g][k] - y * ci[g][k];
                    let ny = x * ci[g][k] + y * cr[g][k];
                    re[g][k] = nx;
                    im[g][k] = ny;
                    sl[k] += ny * wl[g][k];
                    sr[k] += ny * wr[g][k];
                }
            }
            *a = (sl[0] + sl[1]) + (sl[2] + sl[3]);
            *b = (sr[0] + sr[1]) + (sr[2] + sr[3]);
        }
        self.re = re;
        self.im = im;
    }
}

impl Voice for ModalVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if crate::synth::util::avx() {
            // SAFETY: AVX support was checked at run time.
            return unsafe { self.render_avx(l, r) };
        }
        self.render_body(l, r)
    }

    fn release(&mut self) {
        if self.released || self.r_rel >= 1.0 {
            return;
        }
        self.released = true;
        for g in 0..self.groups {
            for k in 0..4 {
                let r = self.radius[g][k];
                if r > self.r_rel {
                    let s = self.r_rel / r;
                    self.cr[g][k] *= s;
                    self.ci[g][k] *= s;
                }
            }
        }
    }

    fn kill(&mut self) {
        self.out.kill(self.rate);
    }
}

impl ModalVoice {
    /// The render body compiled with AVX (8-wide vector loops).
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx")]
    fn render_avx(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        self.render_body(l, r)
    }

    #[inline(always)]
    fn render_body(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        let mut xl = [0.0f32; BLOCK];
        let mut xr = [0.0f32; BLOCK];
        let (bl, br) = (&mut xl[..n], &mut xr[..n]);
        match self.groups {
            1 => self.run::<1>(bl, br),
            2 => self.run::<2>(bl, br),
            3 => self.run::<3>(bl, br),
            _ => self.run::<4>(bl, br),
        }
        if self.npos < self.noise.len() {
            let end = (self.npos + n).min(self.noise.len());
            for (i, v) in self.noise[self.npos..end].iter().enumerate() {
                bl[i] += v;
                br[i] += v;
            }
            self.npos = end;
        }
        if self.trem.1 > 0.0 {
            // Motor-driven resonator discs: opposed phases left/right.
            let d = self.trem.1 * 0.5;
            let p0 = self.trem_ph;
            let p1 = p0 + self.trem.0 * n as f32;
            self.trem_ph = crate::synth::util::fract(p1);
            let (a0, a1) = (sin2pi(p0), sin2pi(p1));
            let (b0, b1) = (sin2pi(p0 + 0.2), sin2pi(p1 + 0.2));
            for i in 0..n {
                let t = i as f32 / n as f32;
                bl[i] *= 1.0 - d + d * (a0 + (a1 - a0) * t);
                br[i] *= 1.0 - d + d * (b0 + (b1 - b0) * t);
            }
        }
        self.out.stereo(bl, br, l, r);
        // Flush dead modes (−140 dB; also the denormal guard), drop dead
        // trailing groups, and estimate the remaining level.
        let mut bound = 0.0f32;
        for g in 0..self.groups {
            for k in 0..4 {
                let m = self.re[g][k].abs() + self.im[g][k].abs();
                if m < 1e-7 {
                    self.re[g][k] = 0.0;
                    self.im[g][k] = 0.0;
                }
                bound += m;
            }
        }
        while self.groups > 1
            && self.re[self.groups - 1]
                .iter()
                .chain(&self.im[self.groups - 1])
                .all(|&v| v == 0.0)
        {
            self.groups -= 1;
        }
        let rest = if self.npos < self.noise.len() {
            1.0
        } else {
            0.0
        };
        !((bound + rest) * self.out.g < SILENCE || self.out.dead())
    }
}

// ---------------------------------------------------------------- partials

/// Free-free bar (glockenspiel, untuned bars): β² ratios.
const FREE_BAR: [f32; 6] = [1.0, 2.756, 5.404, 8.933, 13.344, 18.638];
/// Clamped-free bar / tine (kalimba, music box): 1 : 6.267 : 17.55 : 34.39.
const TINE: [f32; 4] = [1.0, 6.267, 17.547, 34.386];

/// Marimba bar: tuned 1 : 4 : 9.9 (undercut arch), plus torsional/lateral modes.
pub const MARIMBA: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (3.99, 0.35, 0.45),
    (9.9, 0.12, 0.25),
    (2.38, 0.03, 0.4),
    (15.6, 0.03, 0.15),
];
/// Vibraphone (aluminium, 1 : 4 : 10, long sustain).
pub const VIBRAPHONE: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (4.0, 0.3, 0.5),
    (10.0, 0.08, 0.3),
    (2.71, 0.02, 0.4),
    (16.2, 0.02, 0.2),
];
/// Xylophone (rosewood, tuned 1 : 3 : 6.1 → bright twelfth).
pub const XYLOPHONE: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (3.0, 0.5, 0.55),
    (6.1, 0.2, 0.35),
    (9.8, 0.08, 0.25),
    (2.5, 0.03, 0.4),
];
/// Glockenspiel (steel free-free bars).
pub const GLOCKENSPIEL: &[Mode] = &[
    (FREE_BAR[0], 1.0, 1.0),
    (FREE_BAR[1], 0.45, 0.6),
    (FREE_BAR[2], 0.25, 0.4),
    (FREE_BAR[3], 0.12, 0.3),
    (FREE_BAR[4], 0.05, 0.2),
];
/// Celesta (hammered steel plates over resonators: sweet, mostly fundamental).
pub const CELESTA: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (2.0, 0.08, 0.5),
    (3.98, 0.2, 0.35),
    (10.4, 0.05, 0.2),
    (2.76, 0.04, 0.4),
];
/// Kalimba tine plus box resonances (negative ratio = Hz).
pub const KALIMBA: &[Mode] = &[
    (TINE[0], 1.0, 1.0),
    (TINE[1], 0.22, 0.3),
    (TINE[2], 0.06, 0.15),
    (2.0, 0.015, 0.6),
    (-260.0, 0.05, 0.06),
    (-610.0, 0.03, 0.05),
];
/// Music-box comb tooth.
pub const MUSIC_BOX: &[Mode] = &[
    (TINE[0], 1.0, 1.0),
    (TINE[1], 0.3, 0.35),
    (TINE[2], 0.1, 0.2),
    (TINE[3], 0.04, 0.12),
    (2.0, 0.02, 0.5),
];
/// Tubular bell (Rayleigh: free-free tube; strike tone ≈ half of mode 4).
pub const TUBULAR: &[Mode] = &[
    (FREE_BAR[0], 0.25, 1.4),
    (FREE_BAR[1], 0.5, 1.2),
    (FREE_BAR[2], 0.8, 1.0),
    (FREE_BAR[3], 1.0, 1.0),
    (FREE_BAR[4], 0.8, 0.9),
    (FREE_BAR[5], 0.6, 0.8),
    (24.66, 0.3, 0.6),
    (31.4, 0.15, 0.5),
];
/// Church bell (minor-third bell): hum, prime, tierce, quint, nominal, deciem,
/// undeciem, duodeciem, upper octave — relative to the prime.
pub const CHURCH_BELL: &[Mode] = &[
    (0.5, 0.7, 1.7),
    (1.0, 0.55, 1.3),
    (1.2, 0.6, 1.0),
    (1.5, 0.25, 0.8),
    (2.0, 1.0, 0.9),
    (2.5, 0.35, 0.55),
    (2.67, 0.25, 0.5),
    (3.01, 0.3, 0.45),
    (4.0, 0.2, 0.35),
    (5.33, 0.08, 0.25),
];
/// Bright modal bell (handbell-like, strong fundamental).
pub const BELL: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (2.0, 0.25, 0.8),
    (2.76, 0.45, 0.6),
    (3.92, 0.3, 0.5),
    (5.4, 0.25, 0.4),
    (6.8, 0.15, 0.3),
    (8.2, 0.12, 0.25),
    (10.3, 0.06, 0.2),
];
/// Handpan note field: fundamental, octave, compound fifth, plus weak inharmonics.
pub const HANDPAN: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (2.0, 0.5, 0.75),
    (3.0, 0.3, 0.6),
    (0.5, 0.0, 1.0),
    (4.21, 0.05, 0.3),
    (5.6, 0.03, 0.25),
    (-210.0, 0.02, 0.4),
];
/// Steelpan note: 1, 2, 3, 4 tuned, strong octave.
pub const STEEL_DRUM: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (2.0, 0.75, 0.6),
    (3.0, 0.35, 0.45),
    (4.0, 0.2, 0.35),
    (5.02, 0.08, 0.25),
    (6.1, 0.05, 0.2),
];
/// Wood block (hollowed hardwood, short and knocky).
pub const WOOD_BLOCK: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (1.58, 0.45, 0.7),
    (2.56, 0.3, 0.5),
    (3.9, 0.15, 0.4),
    (5.3, 0.06, 0.3),
];
/// Crystal / glass (wine-glass ratios, long ring).
pub const CRYSTAL: &[Mode] = &[
    (1.0, 1.0, 1.0),
    (2.32, 0.4, 0.7),
    (4.25, 0.25, 0.5),
    (6.63, 0.12, 0.35),
    (9.38, 0.06, 0.25),
];

/// The modal presets.
pub fn spec(name: &str) -> Option<ModalSpec> {
    let base = ModalSpec {
        modes: MARIMBA,
        tune: 1.0,
        t60: 2.0,
        slope: 0.8,
        contact: (3.0, 0.6),
        noise: (0.0, 2000.0, 4.0),
        tube: (0.0, 1.0, 0.02),
        trem: (0.0, 0.0),
        beat: 0.0,
        detune: 0.0,
        release_t60: 0.0,
        width: 0.4,
        spread: 0.5,
        gain: 0.5,
    };
    Some(match name {
        "marimba" => ModalSpec {
            modes: MARIMBA,
            t60: 2.2,
            slope: 1.1,
            contact: (4.0, 0.9),
            noise: (0.05, 1200.0, 6.0),
            tube: (0.35, 1.6, 0.015),
            gain: 0.631,
            ..base
        },
        "vibraphone" => ModalSpec {
            modes: VIBRAPHONE,
            t60: 9.0,
            slope: 0.6,
            contact: (2.5, 0.5),
            noise: (0.04, 3000.0, 4.0),
            tube: (0.3, 3.0, 0.02),
            trem: (5.2, 0.55),
            release_t60: 0.6,
            gain: 0.674,
            ..base
        },
        "xylophone" => ModalSpec {
            modes: XYLOPHONE,
            t60: 1.2,
            slope: 1.0,
            contact: (1.2, 0.25),
            noise: (0.12, 4000.0, 3.0),
            tube: (0.15, 0.6, 0.01),
            gain: 0.752,
            ..base
        },
        "glockenspiel" => ModalSpec {
            modes: GLOCKENSPIEL,
            t60: 9.0,
            slope: 0.5,
            contact: (0.6, 0.12),
            noise: (0.08, 7000.0, 2.0),
            gain: 0.507,
            ..base
        },
        "celesta" => ModalSpec {
            modes: CELESTA,
            t60: 4.0,
            slope: 0.7,
            contact: (1.5, 0.5),
            noise: (0.03, 2500.0, 5.0),
            tube: (0.2, 1.5, 0.01),
            release_t60: 0.35,
            gain: 0.658,
            ..base
        },
        "kalimba" => ModalSpec {
            modes: KALIMBA,
            t60: 3.0,
            slope: 0.7,
            contact: (1.5, 0.35),
            noise: (0.06, 2500.0, 4.0),
            detune: 4.0,
            width: 0.3,
            gain: 0.638,
            ..base
        },
        "music-box" => ModalSpec {
            modes: MUSIC_BOX,
            t60: 3.5,
            slope: 0.6,
            contact: (0.4, 0.12),
            noise: (0.05, 6000.0, 2.0),
            detune: 6.0,
            width: 0.3,
            gain: 0.608,
            ..base
        },
        "tubular-bells" => ModalSpec {
            modes: TUBULAR,
            tune: 2.0 / FREE_BAR[3],
            t60: 7.0,
            slope: 0.35,
            contact: (1.2, 0.3),
            noise: (0.05, 3000.0, 4.0),
            beat: 3.0,
            release_t60: 2.0,
            spread: 0.3,
            gain: 0.391,
            ..base
        },
        "church-bell" => ModalSpec {
            modes: CHURCH_BELL,
            t60: 7.5,
            slope: 0.4,
            contact: (1.5, 0.5),
            noise: (0.05, 1500.0, 8.0),
            beat: 6.0,
            detune: 3.0,
            width: 0.5,
            spread: 0.2,
            gain: 0.488,
            ..base
        },
        "bell" => ModalSpec {
            modes: BELL,
            t60: 6.0,
            slope: 0.5,
            contact: (0.8, 0.2),
            noise: (0.04, 5000.0, 3.0),
            beat: 4.0,
            width: 0.5,
            gain: 0.567,
            ..base
        },
        "handpan" => ModalSpec {
            modes: HANDPAN,
            t60: 5.0,
            slope: 0.6,
            contact: (4.0, 1.6),
            noise: (0.03, 900.0, 8.0),
            beat: 3.0,
            detune: 2.0,
            width: 0.4,
            gain: 0.658,
            ..base
        },
        "steel-drum" => ModalSpec {
            modes: STEEL_DRUM,
            t60: 1.6,
            slope: 0.6,
            contact: (1.5, 0.45),
            noise: (0.06, 2500.0, 4.0),
            beat: 4.0,
            detune: 3.0,
            gain: 0.611,
            ..base
        },
        "wood-block" => ModalSpec {
            modes: WOOD_BLOCK,
            t60: 0.25,
            slope: 0.5,
            contact: (0.6, 0.2),
            noise: (0.25, 3000.0, 3.0),
            spread: 0.2,
            gain: 0.685,
            ..base
        },
        "crystal" => ModalSpec {
            modes: CRYSTAL,
            t60: 8.0,
            slope: 0.5,
            contact: (1.5, 0.6),
            noise: (0.0, 2000.0, 4.0),
            beat: 2.5,
            release_t60: 2.5,
            width: 0.6,
            gain: 0.561,
            ..base
        },
        _ => return None,
    })
}
