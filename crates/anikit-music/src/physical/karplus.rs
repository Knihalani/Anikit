//! Extended Karplus-Strong / digital-waveguide plucked strings.
//!
//! The loop is a delay line closed by:
//! - a first-order Thiran allpass for the fractional part of the period;
//! - a one-pole loss filter whose DC gain and pole are solved at note-on so
//!   the fundamental decays in exactly `t60(f0)` seconds and the upper
//!   partials in `t60_hi` (frequency-dependent loss, decay set in seconds);
//! - optional cascaded first-order allpasses for stiffness dispersion
//!   (stretched, slightly sharp upper partials of metal strings).
//!
//! The phase delay of every filter at f0 is included when the integer and
//! fractional delays are solved, so the fundamental is in tune to a small
//! fraction of a cent.
//!
//! The pluck (a velocity-shaped pulse plus filtered noise, comb-filtered at the
//! pick position) is convolved with a modal body (commuted synthesis: guitar
//! top/air modes, harp soundboard, violin body, banjo head) at note-on and then
//! injected into the loop, so the body costs nothing per sample. Electric
//! instruments read a pickup comb and pass a light amp + cabinet stage.
//! Note-off damps the string (finger / palm mute) to `release_t60`.

use crate::synth::filter::{OnePole, Svf, SvfCoef};
use crate::synth::util::{peak, prewarp, soft, vel_gain, Noise, Out, Quiet};
use crate::voice::{Instrument, NoteOn, Rng, Voice, BLOCK};
use std::f64::consts::PI;

/// A modal body resonance: (frequency Hz, −60 dB time s, gain).
pub type BodyMode = (f32, f32, f32);

/// Parameters of a plucked-string instrument.
#[derive(Clone, Debug)]
pub struct StringSpec {
    /// −60 dB decay of the fundamental at middle C (s).
    pub t60: f32,
    /// Register scaling: t60(f) = t60 · (f / 261.6)^(−slope).
    pub slope: f32,
    /// −60 dB decay around 4 kHz (s): brightness of the sustain.
    pub t60_hi: f32,
    /// Stiffness: allpass coefficient (−0.7..0; 0 = none) and stage count (≤ 4).
    pub disp: f32,
    pub disp_stages: usize,
    /// Pluck position as a fraction of the string length (0.5 = middle).
    pub pick: f32,
    /// Excitation low-pass at velocity 0 and 1 (Hz).
    pub soft: f32,
    pub hard: f32,
    /// Excitation pulse width at velocity 0 and 1 (ms): finger flesh vs nail/plectrum.
    pub pulse_ms: (f32, f32),
    /// Share of noise in the excitation (0..1).
    pub noise: f32,
    /// Body modes (commuted into the excitation).
    pub body: &'static [BodyMode],
    pub body_mix: f32,
    /// Pickup position (fraction, 0 = acoustic output).
    pub pickup: f32,
    /// Amp: drive (0 = no amp stage), cabinet low-pass (Hz), high-pass (Hz).
    pub amp: (f32, f32, f32),
    /// Damping applied at note-off (−60 dB s).
    pub release_t60: f32,
    /// Pick / nail attack noise (level, band centre Hz).
    pub click: (f32, f32),
    /// Stereo width (0..1) and pan spread across the keyboard.
    pub width: f32,
    pub spread: f32,
    pub gain: f32,
    /// Tension modulation: a hard pluck starts this many cents sharp (at
    /// velocity 1) and settles within ≈ 60 ms.
    pub bend: f32,
}

/// A plucked-string instrument.
pub struct PluckedString(pub StringSpec);

impl Instrument for PluckedString {
    fn voice(&self, n: &NoteOn) -> Box<dyn Voice> {
        Box::new(StringVoice::new(&self.0, n))
    }
}

/// Phase delay (samples) of a first-order allpass `(a + z⁻¹)/(1 + a z⁻¹)` at `w`.
fn ap_delay(a: f64, w: f64) -> f64 {
    let (s, c) = w.sin_cos();
    let arg = (-s).atan2(a + c) - (-a * s).atan2(1.0 + a * c);
    -arg / w
}

/// Phase delay of the loss filter `g(1−b)/(1 − b z⁻¹)` at `w`.
fn loss_delay(b: f64, w: f64) -> f64 {
    let (s, c) = w.sin_cos();
    (b * s).atan2(1.0 - b * c) / w
}

/// |1 − b e^{−jw}|.
fn loss_den(b: f64, w: f64) -> f64 {
    let (s, c) = w.sin_cos();
    ((1.0 - b * c).powi(2) + (b * s).powi(2)).sqrt()
}

/// Allpass coefficient giving phase delay `d` at `w` (bisection; d in ≈0.1..2).
fn thiran_for(d: f64, w: f64) -> f64 {
    let (mut lo, mut hi) = (-0.9999, 0.9999);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        // Delay decreases as the coefficient grows.
        if ap_delay(mid, w) > d {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Delay-line length (power of two): periods up to ≈ 2900 samples (≥ 16.5 Hz).
const DELAY: usize = 4096;

struct StringVoice {
    rate: f32,
    d0: f32,
    eta_corr: f32,
    bend: f32,
    bend_coef: f32,
    buf: Box<[f32; DELAY]>,
    w: usize,
    n_int: usize,
    side_tap: usize,
    pick_tap: usize,
    eta: f32,
    ap_z: f32,
    lg: f32,
    lb: f32,
    lz: f32,
    g_rel: f32,
    da: f32,
    dz: [f32; 4],
    stages: usize,
    exc: Vec<f32>,
    epos: usize,
    click: Vec<f32>,
    cpos: usize,
    width: f32,
    pickup: bool,
    amp_drive: f32,
    amp_post: f32,
    cab: Svf,
    cab_c: SvfCoef,
    hp: OnePole,
    hp_g: f32,
    out: Out,
    quiet: Quiet,
    released: bool,
}

impl StringVoice {
    fn new(s: &StringSpec, n: &NoteOn) -> StringVoice {
        let rate = n.rate as f64;
        let f0 = n.freq() as f64;
        let w0 = 2.0 * PI * f0 / rate;
        let period = rate / f0;
        let v = n.vel.clamp(0.0, 1.0) as f64;
        let mut rng = Rng::new(n.seed);

        // --- loss filter: DC gain g and pole b from the two decay times ---
        let t60 = (s.t60 as f64) * (f0 / 261.63).powf(-(s.slope as f64));
        let g0 = 10f64.powf(-3.0 / (t60 * f0));
        let fh = (4000.0f64).max(3.0 * f0).min(0.45 * rate);
        let wh = 2.0 * PI * fh / rate;
        let gh = 10f64.powf(-3.0 / ((s.t60_hi as f64).min(t60) * f0));
        let rho = gh / g0;
        let (mut lo, mut hi) = (0.0f64, 0.97f64);
        if rho < 1.0 {
            for _ in 0..50 {
                let b = 0.5 * (lo + hi);
                let ratio = loss_den(b, w0) / loss_den(b, wh);
                if ratio > rho {
                    lo = b;
                } else {
                    hi = b;
                }
            }
        } else {
            hi = 0.0;
            lo = 0.0;
        }
        let b = 0.5 * (lo + hi);
        // |H(w0)| = g(1−b)/|1 − b e^{−jw0}| = g0.
        let lg = g0 * loss_den(b, w0) / (1.0 - b);
        let t60_rel = (s.release_t60 as f64).min(t60);
        let g_rel = lg * 10f64.powf(-3.0 / (t60_rel * f0)) / g0;

        // --- dispersion + fractional delay, tuned at f0 ---
        let mut stages = s.disp_stages.min(4);
        let da = s.disp as f64;
        // Keep the longest period (plus taps) inside the delay line.
        let period = period.min((DELAY as f64 - 8.0) / 1.4);
        // With a pitch bend the fractional delay sits in [1, 2) so it can shrink.
        let bend_on = s.bend > 0.0;
        let d_lo = if bend_on { 1.0 } else { 0.3 };
        let (n_int, eta, d0) = loop {
            let fixed =
                loss_delay(b, w0) + stages as f64 * if stages > 0 { ap_delay(da, w0) } else { 0.0 };
            let rest = period - fixed;
            let ni = (rest - d_lo).floor();
            if ni >= 2.0 || stages == 0 {
                let ni = ni.max(1.0);
                let d = rest - ni;
                break (ni as usize, thiran_for(d, w0), d);
            }
            stages -= 1;
        };
        // Bend: loop shortening (samples) at the start, limited to what the
        // fractional delay can absorb; decays with a 25 ms time constant.
        let bend_samples = if bend_on {
            let c = s.bend as f64 * v * v;
            (period * (1.0 - (-c / 1200.0).exp2()))
                .min(d0 - 0.15)
                .max(0.0)
        } else {
            0.0
        };
        // η(d) ≈ (1 − d)/(1 + d) plus the exact correction at d0.
        let eta_corr = eta - (1.0 - d0) / (1.0 + d0);

        // --- excitation: pulse + noise, low-passed by velocity, pick-position comb ---
        let p_len = (period.ceil() as usize).max(4);
        let fc = s.soft as f64 + (s.hard as f64 - s.soft as f64) * v * v;
        let fc = fc.min(0.45 * rate);
        let pw_ms = s.pulse_ms.0 as f64 + (s.pulse_ms.1 as f64 - s.pulse_ms.0 as f64) * v;
        let pw = ((pw_ms * 1e-3 * rate) as usize).clamp(1, p_len);
        let mut e = vec![0.0f32; p_len + pw];
        for (i, x) in e.iter_mut().enumerate().take(pw) {
            // Raised-cosine force pulse.
            let ph = (i as f64 + 0.5) / pw as f64;
            *x = (0.5 - 0.5 * (2.0 * PI * ph).cos()) as f32;
        }
        let pulse_sum: f32 = e.iter().sum::<f32>().max(1e-6);
        for x in e.iter_mut().take(pw) {
            *x /= pulse_sum / (pw as f32).sqrt();
        }
        let mut nz = Noise::new(n.seed ^ 0x5157);
        let nmix = s.noise.clamp(0.0, 1.0);
        for (i, x) in e.iter_mut().enumerate() {
            let fade = 1.0 - i as f32 / (p_len + pw) as f32;
            *x = *x * (1.0 - nmix) + nz.next() * nmix * fade * 0.5;
        }
        // Two one-pole low-passes at fc.
        let g = OnePole::coef(prewarp(fc as f32, rate as f32));
        let (mut a1, mut a2) = (OnePole::default(), OnePole::default());
        for x in e.iter_mut() {
            *x = a2.lp(a1.lp(*x, g), g);
        }
        // Pick position comb.
        let pick = (s.pick as f64 + rng.bi() as f64 * 0.01).clamp(0.02, 0.5);
        let m = ((pick * period).round() as usize).max(1);
        let mut ec = vec![0.0f32; e.len() + m];
        for i in 0..ec.len() {
            let a = if i < e.len() { e[i] } else { 0.0 };
            let bv = if i >= m && i - m < e.len() {
                e[i - m]
            } else {
                0.0
            };
            ec[i] = a - bv;
        }
        // Commuted body: run the pluck through the body modes and mix.
        let exc = if !s.body.is_empty() && s.body_mix > 0.0 {
            let tmax = s.body.iter().fold(0.0f32, |a, m| a.max(m.1));
            let len = ec.len() + (tmax * 0.75 * rate as f32) as usize;
            let mut out = vec![0.0f32; len];
            for (i, o) in out.iter_mut().enumerate() {
                *o = if i < ec.len() {
                    ec[i] * (1.0 - s.body_mix * 0.5)
                } else {
                    0.0
                };
            }
            // All body modes advance together (independent lanes: vectorises).
            const BM: usize = 12;
            let (mut c1, mut c2, mut gain) = ([0.0f32; BM], [0.0f32; BM], [0.0f32; BM]);
            for (k, &(bf, bt, bg)) in s.body.iter().take(BM).enumerate() {
                if bf as f64 >= 0.45 * rate {
                    continue;
                }
                let r = 10f64.powf(-3.0 / (bt as f64 * rate));
                let th = 2.0 * PI * bf as f64 / rate;
                c1[k] = (2.0 * r * th.cos()) as f32;
                c2[k] = (-r * r) as f32;
                // Normalised so each body resonance peaks at `bg · body_mix`.
                gain[k] = ((bg * s.body_mix) as f64
                    * (1.0 - r)
                    * (1.0 - 2.0 * r * (2.0 * th).cos() + r * r).sqrt())
                    as f32;
            }
            let (mut y1, mut y2) = ([0.0f32; BM], [0.0f32; BM]);
            for (i, o) in out.iter_mut().enumerate() {
                let x = if i < ec.len() { ec[i] } else { 0.0 };
                let mut sum = 0.0;
                for k in 0..BM {
                    let y = x * gain[k] + c1[k] * y1[k] + c2[k] * y2[k];
                    y2[k] = y1[k];
                    y1[k] = y;
                    sum += y;
                }
                *o += sum;
            }
            out
        } else {
            ec
        };
        let pk = peak(&exc).max(1e-9);
        let exc: Vec<f32> = exc.iter().map(|x| x / pk).collect();

        // Pick / nail click (outside the loop).
        let mut click = Vec::new();
        if s.click.0 > 0.0 {
            let len = (0.004 * rate) as usize;
            let mut f = Svf::default();
            let c = SvfCoef::new(
                prewarp(s.click.1 * (0.7 + 0.6 * v as f32), rate as f32),
                1.0,
            );
            let mut nz = Noise::new(n.seed ^ 0xC71C);
            for i in 0..len {
                let env = (-(i as f32) / (0.0006 * rate as f32)).exp();
                click.push(f.bp(nz.next(), &c) * env * s.click.0 * (v * v) as f32);
            }
        }

        let side_tap = ((0.37 * period) as usize).max(1);
        let pick_tap = if s.pickup > 0.0 {
            ((s.pickup as f64 * period) as usize).max(1)
        } else {
            1
        };
        let key_pan = ((n.key - 60.0) / 36.0).clamp(-1.0, 1.0) * s.spread;
        let level = vel_gain(n.vel) * s.gain;
        StringVoice {
            rate: rate as f32,
            buf: Box::new([0.0; DELAY]),
            w: 0,
            n_int,
            side_tap,
            pick_tap,
            eta: eta as f32,
            ap_z: 0.0,
            lg: lg as f32,
            lb: b as f32,
            lz: 0.0,
            g_rel: g_rel as f32,
            da: da as f32,
            dz: [0.0; 4],
            stages,
            exc,
            epos: 0,
            click,
            cpos: 0,
            width: s.width,
            pickup: s.pickup > 0.0,
            amp_drive: s.amp.0,
            amp_post: if s.amp.0 > 0.0 {
                1.0 / soft(s.amp.0)
            } else {
                1.0
            },
            cab: Svf::default(),
            cab_c: SvfCoef::new(
                prewarp(if s.amp.1 > 0.0 { s.amp.1 } else { 20000.0 }, rate as f32),
                0.9,
            ),
            hp: OnePole::default(),
            hp_g: if s.amp.2 > 0.0 {
                OnePole::coef(prewarp(s.amp.2, rate as f32))
            } else {
                0.0
            },
            out: Out::new(level, key_pan),
            d0: d0 as f32,
            eta_corr: eta_corr as f32,
            bend: bend_samples as f32,
            bend_coef: (-(BLOCK as f64) / (0.025 * rate)).exp() as f32,
            quiet: Quiet::new(n_int as u32 + 2 * BLOCK as u32),
            released: false,
        }
    }

    /// The waveguide loop for one block; `D` dispersion stages, `EXC` = excitation still feeding.
    #[inline(always)]
    fn run<const D: usize, const EXC: bool, const AMP: bool>(
        &mut self,
        mid: &mut [f32],
        side: &mut [f32],
    ) {
        let buf = &mut *self.buf;
        let mask = DELAY - 1;
        let (eta, lg, lb, da) = (self.eta, self.lg, self.lb, self.da);
        let (mut apz, mut lz, mut dz) = (self.ap_z, self.lz, self.dz);
        let mut w = self.w;
        let (ni, st, pt) = (self.n_int, self.side_tap, self.pick_tap);
        let pickup = self.pickup;
        let exc = &self.exc;
        let mut ep = self.epos;
        let lgb = lg * (1.0 - lb);
        let (mut hp, mut cab, cab_c, hg) = (self.hp, self.cab, self.cab_c, self.hp_g);
        let (drive, post) = (self.amp_drive, self.amp_post);
        for i in 0..mid.len() {
            let x = buf[w.wrapping_sub(ni) & mask];
            // Fractional delay (Thiran allpass).
            let y = eta * x + apz;
            apz = x - eta * y;
            // Loss filter.
            lz = lgb * y + lb * lz;
            let mut z = lz;
            // Dispersion.
            for s in dz.iter_mut().take(D) {
                let o = da * z + *s;
                *s = z - da * o;
                z = o;
            }
            if EXC {
                z += exc[ep];
                ep += 1;
            }
            buf[w & mask] = z;
            let tap_s = buf[w.wrapping_sub(st) & mask];
            let mut o = if pickup {
                z - buf[w.wrapping_sub(pt) & mask]
            } else {
                z
            };
            if AMP {
                // Amp: high-pass, soft clip, cabinet low-pass (in the same loop so
                // the independent recurrences overlap).
                let h = hp.hp(o, hg);
                o = cab.lp(soft(h * drive) * post, &cab_c);
            }
            mid[i] = o;
            side[i] = z - tap_s;
            w = w.wrapping_add(1);
        }
        self.ap_z = apz;
        self.lz = lz;
        self.dz = dz;
        self.w = w;
        self.epos = ep;
        self.hp = hp;
        self.cab = cab;
    }

    #[inline(always)]
    fn run_d<const D: usize>(&mut self, exc: bool, amp: bool, a: &mut [f32], b: &mut [f32]) {
        match (exc, amp) {
            (false, false) => self.run::<D, false, false>(a, b),
            (false, true) => self.run::<D, false, true>(a, b),
            (true, false) => self.run::<D, true, false>(a, b),
            (true, true) => self.run::<D, true, true>(a, b),
        }
    }
}

impl Voice for StringVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        let mut mid = [0.0f32; BLOCK];
        let mut side = [0.0f32; BLOCK];
        if self.bend > 0.0 {
            // Tension modulation: shorten the fractional delay, then relax.
            self.bend *= self.bend_coef.powf(n as f32 / BLOCK as f32);
            if self.bend < 1e-4 {
                self.bend = 0.0;
            }
            let d = self.d0 - self.bend;
            self.eta = (1.0 - d) / (1.0 + d) + self.eta_corr;
        }
        let mut done = 0;
        while done < n {
            // Split where the excitation ends so both loops stay branch-free.
            let left = self.exc.len() - self.epos;
            let m = if left > 0 {
                left.min(n - done)
            } else {
                n - done
            };
            let exc = left > 0;
            let (a, b) = (&mut mid[done..done + m], &mut side[done..done + m]);
            let amp = self.amp_drive > 0.0;
            match self.stages {
                0 => self.run_d::<0>(exc, amp, a, b),
                1 => self.run_d::<1>(exc, amp, a, b),
                2 => self.run_d::<2>(exc, amp, a, b),
                3 => self.run_d::<3>(exc, amp, a, b),
                _ => self.run_d::<4>(exc, amp, a, b),
            }
            done += m;
        }
        let (mid, side) = (&mut mid[..n], &mut side[..n]);
        if self.cpos < self.click.len() {
            let end = (self.cpos + n).min(self.click.len());
            for (i, c) in self.click[self.cpos..end].iter().enumerate() {
                mid[i] += c;
            }
            self.cpos = end;
        }
        // Mid/side → left/right.
        let wd = if self.pickup {
            self.width * 0.5
        } else {
            self.width
        } * 0.5;
        for i in 0..n {
            let (m, s) = (mid[i], side[i] * wd);
            side[i] = m - s;
            mid[i] = m + s;
        }
        let pk = peak(mid).max(peak(side)) * self.out.g;
        self.out.stereo(mid, side, l, r);
        let silent = self.quiet.feed(pk, n);
        !(silent || self.out.dead())
    }

    fn release(&mut self) {
        if !self.released {
            self.released = true;
            self.lg = self.g_rel;
        }
    }

    fn kill(&mut self) {
        self.release();
        self.out.kill(self.rate);
    }
}

// ---------------------------------------------------------------- bodies

/// Acoustic guitar: Helmholtz air mode, top-plate modes and higher plate modes.
pub const GUITAR_BODY: &[BodyMode] = &[
    (98.0, 0.22, 1.0),
    (196.0, 0.16, 0.9),
    (226.0, 0.12, 0.55),
    (288.0, 0.11, 0.4),
    (395.0, 0.09, 0.45),
    (512.0, 0.07, 0.3),
    (730.0, 0.06, 0.28),
    (1050.0, 0.045, 0.22),
    (1480.0, 0.035, 0.18),
    (2300.0, 0.025, 0.14),
    (3300.0, 0.02, 0.1),
];

/// Concert harp soundboard.
pub const HARP_BODY: &[BodyMode] = &[
    (150.0, 0.3, 0.8),
    (310.0, 0.22, 0.6),
    (520.0, 0.16, 0.45),
    (880.0, 0.1, 0.3),
    (1500.0, 0.06, 0.2),
    (2600.0, 0.04, 0.12),
];

/// Koto (paulownia body).
pub const KOTO_BODY: &[BodyMode] = &[
    (135.0, 0.25, 0.7),
    (262.0, 0.2, 0.6),
    (470.0, 0.12, 0.45),
    (840.0, 0.08, 0.35),
    (1450.0, 0.05, 0.3),
    (2700.0, 0.03, 0.2),
];

/// Violin family body (A0, CBR, B1−, B1+, bridge hill) for pizzicato.
pub const VIOLIN_BODY: &[BodyMode] = &[
    (275.0, 0.14, 1.0),
    (460.0, 0.1, 0.7),
    (540.0, 0.09, 0.9),
    (640.0, 0.08, 0.6),
    (1000.0, 0.05, 0.45),
    (2400.0, 0.03, 0.55),
    (3100.0, 0.025, 0.35),
];

/// Banjo head (bright, short membrane modes).
pub const BANJO_BODY: &[BodyMode] = &[
    (310.0, 0.07, 0.8),
    (590.0, 0.06, 0.7),
    (940.0, 0.05, 0.6),
    (1480.0, 0.04, 0.55),
    (2350.0, 0.03, 0.5),
    (3500.0, 0.025, 0.4),
];

/// The string presets.
pub fn spec(name: &str) -> Option<StringSpec> {
    let base = StringSpec {
        t60: 4.0,
        slope: 0.6,
        t60_hi: 0.5,
        disp: 0.0,
        disp_stages: 0,
        pick: 0.15,
        soft: 1500.0,
        hard: 9000.0,
        pulse_ms: (1.2, 0.25),
        noise: 0.25,
        body: &[],
        body_mix: 0.0,
        pickup: 0.0,
        amp: (0.0, 0.0, 0.0),
        release_t60: 0.25,
        click: (0.0, 3000.0),
        width: 0.5,
        spread: 0.3,
        gain: 0.5,
        bend: 0.0,
    };
    Some(match name {
        "nylon-guitar" => StringSpec {
            t60: 4.5,
            slope: 0.7,
            t60_hi: 0.35,
            pick: 0.16,
            soft: 1200.0,
            hard: 6000.0,
            pulse_ms: (1.6, 0.5),
            noise: 0.15,
            body: GUITAR_BODY,
            body_mix: 0.55,
            release_t60: 0.3,
            click: (0.05, 2500.0),
            width: 0.6,
            gain: 0.562,
            bend: 2.5,
            ..base
        },
        "steel-guitar" => StringSpec {
            t60: 6.0,
            slope: 0.7,
            t60_hi: 0.9,
            disp: -0.12,
            disp_stages: 2,
            pick: 0.11,
            soft: 2500.0,
            hard: 13000.0,
            pulse_ms: (0.8, 0.12),
            noise: 0.3,
            body: GUITAR_BODY,
            body_mix: 0.5,
            release_t60: 0.25,
            click: (0.12, 4500.0),
            width: 0.6,
            gain: 0.56,
            bend: 4.0,
            ..base
        },
        "electric-guitar" => StringSpec {
            t60: 7.0,
            slope: 0.6,
            t60_hi: 1.4,
            disp: -0.1,
            disp_stages: 2,
            pick: 0.12,
            soft: 2500.0,
            hard: 12000.0,
            pulse_ms: (0.7, 0.12),
            noise: 0.25,
            pickup: 0.2,
            amp: (1.2, 4800.0, 90.0),
            release_t60: 0.15,
            click: (0.06, 4000.0),
            width: 0.35,
            gain: 0.515,
            bend: 3.0,
            ..base
        },
        "muted-guitar" => StringSpec {
            t60: 0.32,
            slope: 0.3,
            t60_hi: 0.06,
            disp: -0.1,
            disp_stages: 2,
            pick: 0.12,
            soft: 1500.0,
            hard: 7000.0,
            pulse_ms: (0.9, 0.2),
            noise: 0.2,
            pickup: 0.22,
            amp: (1.4, 4200.0, 100.0),
            release_t60: 0.06,
            click: (0.08, 3000.0),
            width: 0.3,
            gain: 0.546,
            bend: 3.0,
            ..base
        },
        "pluck-bass" => StringSpec {
            t60: 9.0,
            slope: 0.9,
            t60_hi: 0.35,
            disp: -0.28,
            disp_stages: 2,
            pick: 0.18,
            soft: 700.0,
            hard: 4500.0,
            pulse_ms: (2.0, 0.6),
            noise: 0.1,
            pickup: 0.12,
            amp: (0.6, 3500.0, 35.0),
            release_t60: 0.12,
            click: (0.03, 2000.0),
            width: 0.1,
            spread: 0.0,
            gain: 0.527,
            bend: 5.0,
            ..base
        },
        "harp" => StringSpec {
            t60: 7.0,
            slope: 0.8,
            t60_hi: 0.6,
            pick: 0.48,
            soft: 1000.0,
            hard: 5000.0,
            pulse_ms: (2.0, 0.7),
            noise: 0.08,
            body: HARP_BODY,
            body_mix: 0.45,
            release_t60: 4.0,
            width: 0.7,
            spread: 0.6,
            gain: 0.505,
            bend: 1.5,
            ..base
        },
        "koto" => StringSpec {
            t60: 4.0,
            slope: 0.6,
            t60_hi: 0.5,
            disp: -0.08,
            disp_stages: 1,
            pick: 0.07,
            soft: 2500.0,
            hard: 11000.0,
            pulse_ms: (0.6, 0.12),
            noise: 0.3,
            body: KOTO_BODY,
            body_mix: 0.5,
            release_t60: 1.5,
            click: (0.15, 3500.0),
            width: 0.5,
            gain: 0.582,
            bend: 22.0,
            ..base
        },
        "pizz" => StringSpec {
            t60: 0.9,
            slope: 0.4,
            t60_hi: 0.12,
            pick: 0.22,
            soft: 1200.0,
            hard: 5000.0,
            pulse_ms: (2.0, 0.8),
            noise: 0.1,
            body: VIOLIN_BODY,
            body_mix: 0.7,
            release_t60: 0.25,
            width: 0.6,
            spread: 0.5,
            gain: 0.538,
            bend: 4.0,
            ..base
        },
        "banjo" => StringSpec {
            t60: 1.6,
            slope: 0.4,
            t60_hi: 0.4,
            disp: -0.1,
            disp_stages: 1,
            pick: 0.08,
            soft: 3000.0,
            hard: 14000.0,
            pulse_ms: (0.5, 0.1),
            noise: 0.35,
            body: BANJO_BODY,
            body_mix: 0.65,
            release_t60: 0.2,
            click: (0.18, 5000.0),
            width: 0.5,
            gain: 0.588,
            bend: 10.0,
            ..base
        },
        _ => return None,
    })
}
