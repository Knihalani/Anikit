//! Synthesised trailer / motion-graphics sound effects, ready to land on a beat.
//!
//! [`sfx`] renders a named effect into a stereo buffer and reports its
//! perceptual hit point ([`Sfx::hit`], seconds from the buffer start): place
//! the buffer at `beat_time − hit` and the hit lands exactly on the beat.
//!
//! | kind | hit point |
//! |---|---|
//! | impacts, hits, clicks, stabs, bells, drops | the transient onset (≈ 0) |
//! | `whoosh`, `swoosh` | the loudest point of the pass-by |
//! | `whip` | the crack |
//! | `riser`, `reverse-cymbal`, `tape-stop` | the end (the peak / the stop) |
//! | `swell` | the peak of the crescendo |
//!
//! Every effect is deterministic for a given [`SfxParams::seed`], layered
//! (noise, tonal, sub, transient), stereo, and carries its own short tail.
//! Output true peak never exceeds −1 dBTP; levels are balanced per kind
//! (cinematic hits at −1 dBTP, UI sounds lower) and `gain` trims from there.

use serde::Deserialize;

use crate::dsp::biquad::{Biquad, FilterKind, Svf, SvfMode};
use crate::dsp::reverb::{closest, Reverb, ReverbParams};
use crate::dsp::util::{db_to_gain, fast_tanh, flush};
use crate::dsp::master::{Limiter, LimiterParams};
use crate::dsp::{process_aligned, Processor};
use crate::voice::{pan_gains, Rng};

/// Effect names accepted by [`sfx`].
pub const SFX: &[&str] = &[
    "whoosh",
    "swoosh",
    "whip",
    "impact",
    "boom",
    "hit",
    "riser",
    "downlifter",
    "reverse-cymbal",
    "braam",
    "glitch",
    "tick",
    "click",
    "pop",
    "shutter",
    "snap",
    "swell",
    "sub-drop",
    "zap",
    "ding",
    "chime",
    "typing",
    "heartbeat",
    "tape-stop",
];

/// Effect parameters (all optional).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SfxParams {
    /// Main length in seconds (the tail may extend past it); default per effect.
    pub duration: Option<f32>,
    /// Transposition in **semitones** (+12 = an octave up). Shifts pitches and filter sweeps.
    pub pitch: f32,
    /// 0..1: drive, brightness and how many layers come in.
    pub intensity: f32,
    /// 0..1: size of the built-in reverb tail (0 = dry).
    pub size: f32,
    /// 0..1: stereo width.
    pub width: f32,
    /// −1..1: pan position.
    pub pan: f32,
    /// −1..1: stereo travel for moving sounds (whoosh, riser, …): −1 sweeps right→left.
    pub motion: Option<f32>,
    /// Output trim in dB (peaks still stay ≤ −1 dBTP).
    pub gain: f32,
    /// Event count: keystrokes for `typing`, beats for `heartbeat`, notes for `chime`.
    pub count: Option<u32>,
    /// Tempo for `heartbeat` (beats per minute) and the `glitch` grid.
    pub bpm: Option<f32>,
    pub seed: u64,
}

impl Default for SfxParams {
    fn default() -> Self {
        SfxParams {
            duration: None,
            pitch: 0.0,
            intensity: 0.7,
            size: 0.5,
            width: 0.8,
            pan: 0.0,
            motion: None,
            gain: 0.0,
            count: None,
            bpm: None,
            seed: 1,
        }
    }
}

/// A rendered effect.
#[derive(Debug, Clone, PartialEq)]
pub struct Sfx {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    /// Perceptual hit point in seconds from the start of the buffer.
    pub hit: f32,
}

impl Sfx {
    /// Length in seconds.
    pub fn duration(&self, rate: u32) -> f32 {
        self.l.len() as f32 / rate as f32
    }
}

/// Canonical effect name: lower case, `_`/space → `-`, camelCase split.
fn canonical(name: &str) -> String {
    let mut s = String::new();
    for (i, c) in name.trim().chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 && !s.ends_with('-') {
            s.push('-');
        }
        s.push(match c {
            '_' | ' ' => '-',
            c => c.to_ascii_lowercase(),
        });
    }
    s
}

/// Render effect `name`. Unknown names fail with a "did you mean" suggestion.
pub fn sfx(name: &str, p: &SfxParams, rate: u32) -> Result<Sfx, String> {
    let c = canonical(name);
    let key = SFX
        .iter()
        .find(|n| **n == c || n.replace('-', "") == c.replace('-', ""))
        .ok_or_else(|| {
            format!("unknown sfx {name:?} (did you mean {:?}?); available: {}", closest(&c, SFX).unwrap_or("whoosh"), SFX.join(", "))
        })?;
    if let Some(d) = p.duration {
        if !(d.is_finite() && d > 0.0 && d <= 60.0) {
            return Err(format!("sfx {key}: duration must be in (0, 60] seconds, got {d}"));
        }
    }
    let mut g = G::new(rate as f32, p, key);
    let (mut l, mut r, hit, peak_db) = match *key {
        "whoosh" => g.whoosh(1.2, 0.62, false),
        "swoosh" => g.whoosh(0.5, 0.45, true),
        "whip" => g.whip(),
        "impact" => g.impact(false),
        "boom" => g.impact(true),
        "hit" => g.hit(),
        "riser" => g.riser(),
        "downlifter" => g.downlifter(),
        "reverse-cymbal" => g.reverse_cymbal(),
        "braam" => g.braam(),
        "glitch" => g.glitch(),
        "tick" => g.tick(),
        "click" => g.click(),
        "pop" => g.pop(),
        "shutter" => g.shutter(),
        "snap" => g.snap(),
        "swell" => g.swell(),
        "sub-drop" => g.sub_drop(),
        "zap" => g.zap(),
        "ding" => g.ding(),
        "chime" => g.chime(),
        "typing" => g.typing(),
        "heartbeat" => g.heartbeat(),
        "tape-stop" => g.tape_stop(),
        _ => unreachable!(),
    };
    // Big hits are driven into a true-peak limiter for density (dB of drive).
    let punch = match *key {
        "impact" => 6.0,
        "hit" => 4.0,
        "boom" | "braam" => 3.0,
        "sub-drop" | "riser" | "downlifter" => 1.5,
        _ => 0.0,
    };
    finish(&mut l, &mut r, g.fs, peak_db + p.gain, hit, punch);
    Ok(Sfx { l, r, hit })
}

/// DC-block, normalise the true peak to `peak_db` (≤ −1 dBTP; with `punch`
/// dB driven into a look-ahead true-peak limiter first), trim the silent tail.
fn finish(l: &mut Vec<f32>, r: &mut Vec<f32>, fs: f32, peak_db: f32, hit: f32, punch: f32) {
    let mut dc = Biquad::new(FilterKind::HighPass, fs, 18.0, std::f32::consts::FRAC_1_SQRT_2, 0.0);
    dc.process(l, r);
    let target = peak_db.min(-1.0);
    let tp = crate::dsp::loudness::true_peak_db(l, r) as f32;
    if tp > -150.0 {
        let g = db_to_gain(target + punch - tp - 0.02);
        l.iter_mut().chain(r.iter_mut()).for_each(|x| *x *= g);
        if punch > 0.0 {
            let lp = LimiterParams { ceiling: target - 0.05, lookahead: 3.0, release: 80.0, hold: 6.0, true_peak: true };
            process_aligned(&mut Limiter::new(fs, &lp), l, r);
        }
    }
    // Trim below −90 dBFS after the hit point, with a short fade.
    let min_len = ((hit * fs) as usize + 1).min(l.len());
    let floor = db_to_gain(-90.0);
    let end = l.iter().zip(r.iter()).rposition(|(a, b)| a.abs().max(b.abs()) > floor).map_or(0, |i| i + 1).max(min_len);
    l.truncate(end);
    r.truncate(end);
    let f = ((0.005 * fs) as usize).min(end.saturating_sub(min_len));
    for i in 0..f {
        let g = i as f32 / f as f32;
        l[end - 1 - i] *= g;
        r[end - 1 - i] *= g;
    }
}

// ---------------------------------------------------------------- helpers

#[inline]
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[inline]
fn lerp_exp(a: f32, b: f32, t: f32) -> f32 {
    a * (b / a).powf(t.clamp(0.0, 1.0))
}

/// PolyBLEP residual for band-limited discontinuities.
#[inline]
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

/// Band-limited sawtooth oscillator.
#[derive(Clone, Copy)]
struct Saw {
    ph: f32,
}

impl Saw {
    #[inline]
    fn tick(&mut self, f: f32, fs: f32) -> f32 {
        let dt = (f / fs).clamp(0.0, 0.45);
        let v = 2.0 * self.ph - 1.0 - blep(self.ph, dt);
        self.ph += dt;
        if self.ph >= 1.0 {
            self.ph -= 1.0;
        }
        v
    }
}

fn add(dst: &mut [f32], src: &[f32], g: f32) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d += s * g;
    }
}

/// Filter `x` in place with a state-variable filter whose cutoff follows `f(t)`.
fn svf(x: &mut [f32], fs: f32, mode: SvfMode, q: f32, f: impl Fn(f32) -> f32) {
    let mut s = Svf::new(fs, mode, f(0.0), q);
    for (k, blk) in x.chunks_mut(16).enumerate() {
        s.set(f((k * 16) as f32 / fs), q);
        for v in blk.iter_mut() {
            *v = s.tick(0, *v);
        }
    }
}

fn biquad(x: &mut [f32], fs: f32, kind: FilterKind, f: f32, q: f32) {
    Biquad::new(kind, fs, f, q, 0.0).process_mono(x);
}

/// Peaking EQ band.
fn peak(x: &mut [f32], fs: f32, f: f32, q: f32, db: f32) {
    Biquad::new(FilterKind::Peak, fs, f, q, db).process_mono(x);
}

/// Sine with time-varying frequency and amplitude.
fn sine(n: usize, fs: f32, f: impl Fn(f32) -> f32, a: impl Fn(f32) -> f32) -> Vec<f32> {
    let mut ph = 0.0f64;
    (0..n)
        .map(|i| {
            let t = i as f32 / fs;
            let v = (ph as f32 * std::f32::consts::TAU).sin() * a(t);
            ph = (ph + (f(t) / fs) as f64).fract();
            v
        })
        .collect()
}

/// Exponential decay with a short linear attack.
#[inline]
fn ad(t: f32, att: f32, tau: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t < att {
        t / att
    } else {
        (-(t - att) / tau).exp()
    }
}

/// Generator state shared by all effects.
struct G {
    fs: f32,
    rng: Rng,
    ps: f32,
    it: f32,
    size: f32,
    width: f32,
    pan: f32,
    motion: Option<f32>,
    duration: Option<f32>,
    count: Option<u32>,
    bpm: Option<f32>,
}

type Out = (Vec<f32>, Vec<f32>, f32, f32);

impl G {
    fn new(fs: f32, p: &SfxParams, key: &str) -> G {
        // Mix the effect name into the seed so effects with equal seeds differ.
        let h = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        G {
            fs,
            rng: Rng::new(p.seed ^ h),
            ps: (p.pitch.clamp(-36.0, 36.0) / 12.0).exp2(),
            it: p.intensity.clamp(0.0, 1.0),
            size: p.size.clamp(0.0, 1.0),
            width: p.width.clamp(0.0, 1.0),
            pan: p.pan.clamp(-1.0, 1.0),
            motion: p.motion.map(|m| m.clamp(-1.0, 1.0)),
            duration: p.duration,
            count: p.count,
            bpm: p.bpm,
        }
    }

    fn dur(&self, default: f32) -> f32 {
        self.duration.unwrap_or(default)
    }

    fn n(&self, secs: f32) -> usize {
        (secs.max(0.0) * self.fs).round() as usize
    }

    fn white(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.rng.bi() * 1.732).collect()
    }

    /// Pink noise (Paul Kellet), ≈ unit RMS.
    fn pink(&mut self, n: usize) -> Vec<f32> {
        let mut b = [0.0f32; 7];
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let w = self.rng.bi() * 1.732;
            b[0] = 0.99886 * b[0] + w * 0.055_517_9;
            b[1] = 0.99332 * b[1] + w * 0.075_075_9;
            b[2] = 0.969 * b[2] + w * 0.153_852;
            b[3] = 0.8665 * b[3] + w * 0.310_485_6;
            b[4] = 0.55 * b[4] + w * 0.532_952_2;
            b[5] = -0.7616 * b[5] - w * 0.016_898;
            let v = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
            b[6] = w * 0.115_926;
            out.push(v * 0.19);
        }
        out
    }

    /// Two partially correlated noise channels (correlation falls with width).
    fn noise2(&mut self, n: usize, pink: bool) -> (Vec<f32>, Vec<f32>) {
        let gen = |s: &mut Self| if pink { s.pink(n) } else { s.white(n) };
        let c = gen(self);
        let a = gen(self);
        let b = gen(self);
        let w = self.width;
        let (gc, gs) = ((1.0 - 0.5 * w * w).max(0.0).sqrt(), w * std::f32::consts::FRAC_1_SQRT_2);
        let l = c.iter().zip(&a).map(|(c, a)| c * gc + a * gs).collect();
        let r = c.iter().zip(&b).map(|(c, b)| c * gc + b * gs).collect();
        (l, r)
    }

    /// Mix a mono layer into the stereo bus with a (possibly moving) pan.
    fn place(&self, l: &mut [f32], r: &mut [f32], x: &[f32], g: f32, pan: impl Fn(f32) -> f32) {
        for (i, ((a, b), v)) in l.iter_mut().zip(r.iter_mut()).zip(x).enumerate() {
            let (gl, gr) = pan_gains((self.pan + pan(i as f32 / self.fs)).clamp(-1.0, 1.0));
            *a += v * g * gl * std::f32::consts::SQRT_2;
            *b += v * g * gr * std::f32::consts::SQRT_2;
        }
    }

    fn center(&self, l: &mut [f32], r: &mut [f32], x: &[f32], g: f32) {
        self.place(l, r, x, g, |_| 0.0);
    }

    /// Apply the global pan to a stereo layer.
    fn balance(&self, l: &mut [f32], r: &mut [f32]) {
        if self.pan != 0.0 {
            let (gl, gr) = pan_gains(self.pan);
            l.iter_mut().for_each(|x| *x *= gl * std::f32::consts::SQRT_2);
            r.iter_mut().for_each(|x| *x *= gr * std::f32::consts::SQRT_2);
        }
    }

    /// Append a reverb tail: `preset` scaled by `size`, `wet` at size 1.
    fn verb(&self, l: &mut Vec<f32>, r: &mut Vec<f32>, preset: &str, wet: f32, tail: f32) {
        let wet = wet * self.size;
        if wet <= 0.0 {
            return;
        }
        let p = ReverbParams {
            preset: Some(preset.into()),
            width: Some(self.width.max(0.3)),
            wet: Some(wet),
            dry: Some(1.0),
            ..Default::default()
        };
        let mut rv = Reverb::new(self.fs, &p);
        let extra = self.n(tail * (0.5 + self.size));
        l.resize(l.len() + extra, 0.0);
        r.resize(r.len() + extra, 0.0);
        rv.process(l, r);
    }

    // ------------------------------------------------------------ effects

    /// Whoosh / swoosh: filtered noise pass-by peaking at `peak` of the length.
    fn whoosh(&mut self, default: f32, peak: f32, bright: bool) -> Out {
        let d = self.dur(default);
        let n = self.n(d);
        let pk = peak * d;
        let fs = self.fs;
        let env = move |t: f32| {
            if t < pk {
                (0.5 - 0.5 * (std::f32::consts::PI * t / pk).cos()).powf(1.6)
            } else {
                (0.5 + 0.5 * (std::f32::consts::PI * ((t - pk) / (d - pk)).min(1.0)).cos()).powf(1.3)
            }
        };
        let (lo, hi) = if bright { (1200.0, 7500.0) } else { (380.0, 3600.0) };
        let hi = hi * (0.65 + 0.7 * self.it);
        let ps = self.ps;
        let fc = move |t: f32| lerp_exp(lo, hi, env(t)) * ps;
        let motion = self.motion.unwrap_or(0.5) * if bright { 1.2 } else { 1.0 };
        let mpan = move |t: f32| motion * (2.0 * t / d - 1.0).clamp(-1.0, 1.0);
        let (mut l, mut r) = self.noise2(n, true);
        let (wl, wr) = self.noise2(n, false);
        add(&mut l, &wl, 0.35);
        add(&mut r, &wr, 0.35);
        let q = if bright { 1.6 } else { 1.1 };
        svf(&mut l, fs, SvfMode::BandPass, q, fc);
        svf(&mut r, fs, SvfMode::BandPass, q, move |t| fc(t) * 1.06);
        // Body: low-passed pink rumble following the envelope.
        let mut body = self.pink(n);
        svf(&mut body, fs, SvfMode::LowPass, 0.8, move |t| (160.0 + 900.0 * env(t)) * ps);
        // Whistle: a resonant band an octave and a half above the sweep.
        let mut whistle = self.white(n);
        svf(&mut whistle, fs, SvfMode::BandPass, 9.0, move |t| fc(t) * 1.9);
        let mut ol = vec![0.0f32; n];
        let mut or = vec![0.0f32; n];
        for i in 0..n {
            let t = i as f32 / fs;
            let e = env(t);
            let (gl, gr) = pan_gains(mpan(t));
            ol[i] = l[i] * e * gl * 1.4;
            or[i] = r[i] * e * gr * 1.4;
            body[i] *= e * if bright { 0.25 } else { 0.7 };
            whistle[i] *= e * e * 0.35 * self.it;
        }
        self.place(&mut ol, &mut or, &body, 1.0, mpan);
        self.place(&mut ol, &mut or, &whistle, 1.0, mpan);
        self.balance(&mut ol, &mut or);
        self.verb(&mut ol, &mut or, "hall", 0.25, 1.2);
        (ol, or, pk, -3.0)
    }

    fn whip(&mut self) -> Out {
        let d = self.dur(0.4);
        let crack = 0.62 * d;
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let (mut l, mut r) = self.noise2(n, false);
        let env = move |t: f32| if t < crack { (t / crack).powi(3) } else { (-(t - crack) / 0.03).exp() };
        svf(&mut l, fs, SvfMode::BandPass, 1.4, move |t| lerp_exp(900.0, 6500.0, t / crack) * ps);
        svf(&mut r, fs, SvfMode::BandPass, 1.4, move |t| lerp_exp(950.0, 6800.0, t / crack) * ps);
        let mut ol = vec![0.0f32; n];
        let mut or = vec![0.0f32; n];
        let motion = self.motion.unwrap_or(0.4);
        for i in 0..n {
            let t = i as f32 / fs;
            let (gl, gr) = pan_gains(motion * (2.0 * t / crack - 1.0).clamp(-1.0, 1.0));
            ol[i] = l[i] * env(t) * gl * 0.9;
            or[i] = r[i] * env(t) * gr * 0.9;
        }
        // The crack: a sharp broadband burst plus a resonant snap.
        let c0 = self.n(crack);
        let mut burst = self.white(self.n(0.06));
        for (i, v) in burst.iter_mut().enumerate() {
            *v *= (-(i as f32 / fs) / 0.004).exp();
        }
        biquad(&mut burst, fs, FilterKind::HighPass, 1500.0 * ps, 0.7);
        let mut snap = burst.clone();
        biquad(&mut snap, fs, FilterKind::BandPass, 2600.0 * ps, 6.0);
        let mut cl = vec![0.0f32; burst.len()];
        let mut cr = cl.clone();
        self.place(&mut cl, &mut cr, &burst, 2.2, |_| motion);
        self.place(&mut cl, &mut cr, &snap, 3.0, |_| motion);
        for i in 0..cl.len().min(n.saturating_sub(c0)) {
            ol[c0 + i] += cl[i];
            or[c0 + i] += cr[i];
        }
        self.balance(&mut ol, &mut or);
        self.verb(&mut ol, &mut or, "room", 0.3, 0.6);
        (ol, or, crack, -3.0)
    }

    /// Cinematic impact (or a deeper, more distant boom).
    fn impact(&mut self, boom: bool) -> Out {
        let d = self.dur(if boom { 4.0 } else { 3.0 });
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let it = self.it;
        let (f_end, f_start, glide, sub_tau) = if boom { (32.0, 85.0, 0.12, 1.4) } else { (44.0, 115.0, 0.055, 0.85) };
        let sub_tau = sub_tau * (d / if boom { 4.0 } else { 3.0 }).min(2.0);
        let mut sub = sine(n, fs, |t| (f_end + (f_start - f_end) * (-t / glide).exp()) * ps, |t| ad(t, 0.002, sub_tau));
        for v in sub.iter_mut() {
            *v = fast_tanh(1.8 * *v) / fast_tanh(1.8);
        }
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &sub, if boom { 1.0 } else { 0.9 });
        // Body: low noise, its low-pass closing.
        let (mut bl, mut br) = self.noise2(n, true);
        let (b_hi, b_lo, b_tau) = if boom { (500.0, 70.0, 0.6) } else { (1400.0, 140.0, 0.22) };
        for (bx, k) in [(&mut bl, 1.0f32), (&mut br, 1.05)] {
            svf(bx, fs, SvfMode::LowPass, 0.9, move |t| lerp_exp(b_hi, b_lo, (t / 0.5).min(1.0)) * ps * k);
            for (i, v) in bx.iter_mut().enumerate() {
                *v *= ad(i as f32 / fs, 0.001, b_tau) * if boom { 0.55 } else { 0.9 };
            }
        }
        add(&mut l, &bl, 1.0);
        add(&mut r, &br, 1.0);
        // Transient: crack + click (softer and darker for a boom).
        let tn = self.n(0.08).min(n);
        let (mut tl, mut tr) = self.noise2(tn, false);
        for bx in [&mut tl, &mut tr] {
            for (i, v) in bx.iter_mut().enumerate() {
                *v *= (-(i as f32 / fs) / if boom { 0.012 } else { 0.007 }).exp();
            }
            if boom {
                biquad(bx, fs, FilterKind::LowPass, 2200.0 * ps, 0.7);
            } else {
                biquad(bx, fs, FilterKind::HighPass, 900.0 * ps, 0.7);
                peak(bx, fs, 3200.0 * ps, 0.8, 6.0);
            }
        }
        let tg = (0.6 + 0.6 * it) * if boom { 0.6 } else { 1.0 };
        add(&mut l[..tn], &tl, tg);
        add(&mut r[..tn], &tr, tg);
        // Metallic ring (inharmonic partials) for the cinematic "clang".
        if !boom && it > 0.3 {
            let ratios = [1.0f32, 1.594, 2.136, 2.653, 3.17, 4.07];
            for (k, &ra) in ratios.iter().enumerate() {
                let f = 173.0 * ra * ps;
                let tau = 0.5 / (1.0 + 0.4 * k as f32);
                let p = sine(n, fs, |_| f, |t| ad(t, 0.001, tau));
                let side = if k % 2 == 0 { -0.5 } else { 0.5 };
                self.place(&mut l, &mut r, &p, 0.05 * (it - 0.3) / (1.0 + k as f32 * 0.3), |_| side * self.width);
            }
        }
        // Shape the start: 0.3 ms ramp so the onset is sharp but click-free.
        let ramp = self.n(0.0003).max(1);
        for i in 0..ramp.min(n) {
            let g = i as f32 / ramp as f32;
            l[i] *= g;
            r[i] *= g;
        }
        self.balance(&mut l, &mut r);
        let preset = if boom { "cathedral" } else { "hall" };
        self.verb(&mut l, &mut r, preset, if boom { 0.45 } else { 0.35 }, if boom { 3.0 } else { 2.0 });
        (l, r, 0.0, -1.0)
    }

    /// Punchy hit: kick body + snare/clap crack + short room.
    fn hit(&mut self) -> Out {
        let d = self.dur(0.8);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let mut body = sine(n, fs, |t| (52.0 + 130.0 * (-t / 0.035).exp()) * ps, |t| ad(t, 0.001, 0.16));
        for v in body.iter_mut() {
            *v = fast_tanh(2.2 * *v) / fast_tanh(2.2);
        }
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &body, 0.85);
        let (mut nl, mut nr) = self.noise2(n, false);
        for bx in [&mut nl, &mut nr] {
            biquad(bx, fs, FilterKind::BandPass, 1800.0 * ps, 0.6);
            for (i, v) in bx.iter_mut().enumerate() {
                let t = i as f32 / fs;
                // Clap-like triple attack, then a noise decay.
                let clap = [0.0f32, 0.009, 0.019].iter().map(|o| ad(t - o, 0.0005, 0.006)).fold(0.0, f32::max);
                *v *= 0.45 * ad(t, 0.0008, 0.09) + 0.6 * clap * self.it;
            }
        }
        add(&mut l, &nl, 0.9);
        add(&mut r, &nr, 0.9);
        let mut thump = sine(n, fs, |_| 210.0 * ps, |t| ad(t, 0.001, 0.05));
        thump.iter_mut().for_each(|v| *v *= 0.25);
        self.center(&mut l, &mut r, &thump, 1.0);
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "room", 0.35, 0.8);
        (l, r, 0.0, -1.0)
    }

    /// Riser: noise + supersaw pitch rise, peaking exactly at the end.
    fn riser(&mut self) -> Out {
        let d = self.dur(4.0);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let it = self.it;
        let (mut l, mut r) = self.noise2(n, false);
        svf(&mut l, fs, SvfMode::BandPass, 2.2, move |t| lerp_exp(250.0, 9000.0, (t / d).powf(1.4)) * ps);
        svf(&mut r, fs, SvfMode::BandPass, 2.2, move |t| lerp_exp(265.0, 9400.0, (t / d).powf(1.4)) * ps);
        let motion = self.motion.unwrap_or(0.0);
        for i in 0..n {
            let p = i as f32 / n as f32;
            let e = p.powf(2.2);
            let (gl, gr) = pan_gains(motion * (2.0 * p - 1.0));
            l[i] *= e * 1.2 * gl * std::f32::consts::SQRT_2;
            r[i] *= e * 1.2 * gr * std::f32::consts::SQRT_2;
        }
        // Supersaw chord (root + fifth + octave) rising an octave, opening up.
        let f0 = 110.0 * ps;
        let notes = [1.0f32, 1.5, 2.0];
        let detune = [-0.11f32, -0.06, -0.02, 0.0, 0.02, 0.06, 0.11];
        let mut saws = vec![[Saw { ph: 0.0 }; 7]; notes.len()];
        for row in saws.iter_mut() {
            for s in row.iter_mut() {
                s.ph = self.rng.f32();
            }
        }
        let mut tl = vec![0.0f32; n];
        let mut tr = vec![0.0f32; n];
        for i in 0..n {
            let p = i as f32 / n as f32;
            let rise = (p * p).exp2();
            let trem_rate = 2.0 + 14.0 * p * p;
            let trem = 1.0 - 0.3 * it * (0.5 + 0.5 * (std::f32::consts::TAU * trem_rate * p * d).sin());
            for (k, nf) in notes.iter().enumerate() {
                for (j, dt) in detune.iter().enumerate() {
                    let f = f0 * nf * rise * (dt / 12.0).exp2();
                    let v = saws[k][j].tick(f, fs);
                    if j % 2 == 0 {
                        tl[i] += v;
                    } else {
                        tr[i] += v;
                    }
                }
            }
            let a = p.powf(1.8) * trem * 0.08;
            tl[i] *= a;
            tr[i] *= a;
        }
        svf(&mut tl, fs, SvfMode::LowPass, 1.0, move |t| lerp_exp(400.0, 9000.0, t / d) * ps.sqrt());
        svf(&mut tr, fs, SvfMode::LowPass, 1.0, move |t| lerp_exp(420.0, 9300.0, t / d) * ps.sqrt());
        add(&mut l, &tl, 0.6 + 0.4 * it);
        add(&mut r, &tr, 0.6 + 0.4 * it);
        // Cut cleanly at the end (2 ms).
        let f = self.n(0.002).min(n);
        for i in 0..f {
            let g = i as f32 / f as f32;
            l[n - 1 - i] *= g;
            r[n - 1 - i] *= g;
        }
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "hall", 0.3, 1.5);
        (l, r, d, -1.0)
    }

    fn downlifter(&mut self) -> Out {
        let d = self.dur(3.0);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let (mut l, mut r) = self.noise2(n, false);
        svf(&mut l, fs, SvfMode::BandPass, 1.8, move |t| lerp_exp(8000.0, 180.0, (t / d).powf(0.7)) * ps);
        svf(&mut r, fs, SvfMode::BandPass, 1.8, move |t| lerp_exp(8400.0, 190.0, (t / d).powf(0.7)) * ps);
        let mut tone = vec![0.0f32; n];
        let mut saws = [Saw { ph: 0.0 }, Saw { ph: 0.37 }, Saw { ph: 0.71 }];
        for (i, v) in tone.iter_mut().enumerate() {
            let p = i as f32 / n as f32;
            let f = 220.0 * ps * (-2.0 * p.powf(0.8)).exp2();
            *v = (saws[0].tick(f, fs) + saws[1].tick(f * 1.006, fs) + saws[2].tick(f * 0.994, fs)) * 0.2;
        }
        svf(&mut tone, fs, SvfMode::LowPass, 0.9, move |t| lerp_exp(6000.0, 300.0, t / d) * ps);
        let sub = sine(n, fs, |t| lerp_exp(90.0, 30.0, t / d) * ps, |t| ad(t, 0.003, d * 0.35));
        let motion = self.motion.unwrap_or(0.0);
        for i in 0..n {
            let p = i as f32 / n as f32;
            let e = (1.0 - p).powf(2.0) * smooth(p / 0.01);
            let (gl, gr) = pan_gains(-motion * (2.0 * p - 1.0));
            l[i] *= e * 1.2 * gl;
            r[i] *= e * 1.2 * gr;
            tone[i] *= e;
        }
        self.center(&mut l, &mut r, &tone, 0.5 + 0.5 * self.it);
        self.center(&mut l, &mut r, &sub, 0.6);
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "hall", 0.35, 1.5);
        (l, r, 0.0, -1.0)
    }

    /// Crash cymbal (metallic partials + noise), with its reverb, played backwards.
    fn reverse_cymbal(&mut self) -> Out {
        let d = self.dur(2.5);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let (mut l, mut r) = self.noise2(n, false);
        for bx in [&mut l, &mut r] {
            biquad(bx, fs, FilterKind::HighPass, 4200.0 * ps, 0.7);
            peak(bx, fs, 9000.0 * ps.sqrt(), 0.8, 4.0);
        }
        // Ring-modulated square partials (808-style metal).
        let fr = [205.3f32, 304.4, 369.6, 522.7, 540.0, 800.0];
        let mut metal = vec![0.0f32; n];
        let mut ph = [0.0f32; 6];
        for (i, v) in metal.iter_mut().enumerate() {
            let mut s = 0.0;
            for k in 0..6 {
                ph[k] = (ph[k] + fr[k] * 2.6 * ps / fs).fract();
                s += if ph[k] < 0.5 { 1.0 } else { -1.0 };
            }
            let _ = i;
            *v = s / 6.0;
        }
        biquad(&mut metal, fs, FilterKind::HighPass, 3500.0 * ps, 0.7);
        biquad(&mut metal, fs, FilterKind::HighPass, 3500.0 * ps, 0.7);
        let tau = d * 0.38;
        for i in 0..n {
            let t = i as f32 / fs;
            let e = ad(t, 0.0005, tau) * (0.75 + 0.25 * (-t / 0.05).exp());
            l[i] *= e * 0.7;
            r[i] *= e * 0.7;
            metal[i] *= e * 0.5 * self.it;
        }
        self.center(&mut l, &mut r, &metal, 1.0);
        // Reverb inside the length, then reverse so the swell leads into the hit.
        let mut rv = Reverb::new(fs, &ReverbParams { preset: Some("hall".into()), wet: Some(0.35 * self.size), dry: Some(1.0), ..Default::default() });
        rv.process(&mut l, &mut r);
        l.reverse();
        r.reverse();
        let f = self.n(0.0015).min(n);
        for i in 0..f {
            let g = i as f32 / f as f32;
            l[n - 1 - i] *= g;
            r[n - 1 - i] *= g;
        }
        self.balance(&mut l, &mut r);
        (l, r, d, -2.0)
    }

    /// Huge brassy cinematic stab.
    fn braam(&mut self) -> Out {
        let d = self.dur(3.5);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let it = self.it;
        let root = 55.0 * ps;
        let notes = [1.0f32, 2.0, 3.0, 4.0];
        let gains = [1.0f32, 0.8, 0.55, 0.35];
        let detune = [-0.14f32, -0.08, -0.03, 0.0, 0.03, 0.08, 0.14];
        let mut saws: Vec<[Saw; 7]> = (0..notes.len()).map(|_| [Saw { ph: 0.0 }; 7]).collect();
        for row in saws.iter_mut() {
            for s in row.iter_mut() {
                s.ph = self.rng.f32();
            }
        }
        let rel = (0.9f32).min(d * 0.4);
        let amp = move |t: f32| {
            let a = smooth(t / 0.045);
            let tail = if t > d - rel { (-(t - (d - rel)) / (rel * 0.3)).exp() } else { 1.0 };
            a * (0.75 + 0.25 * (-t / 0.4).exp()) * tail
        };
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        for i in 0..n {
            let t = i as f32 / fs;
            // Brass "rip": starts a semitone flat; slow growl modulation.
            let bend = (-(1.0 - smooth(t / 0.09)) / 12.0).exp2();
            let growl = 1.0 + 0.004 * (std::f32::consts::TAU * 5.5 * t).sin();
            for (k, nf) in notes.iter().enumerate() {
                for (j, dt) in detune.iter().enumerate() {
                    let v = saws[k][j].tick(root * nf * bend * growl * (dt / 12.0).exp2(), fs) * gains[k];
                    let pan = (j as f32 / 6.0 * 2.0 - 1.0) * self.width;
                    let (gl, gr) = pan_gains(pan);
                    l[i] += v * gl;
                    r[i] += v * gr;
                }
            }
            let a = amp(t) * 0.07;
            l[i] *= a;
            r[i] *= a;
        }
        let cut = move |t: f32| {
            let open = 180.0 + (2500.0 + 4000.0 * it) * smooth(t / 0.06) * (0.35 + 0.65 * (-t / 0.7).exp());
            open * ps.sqrt()
        };
        svf(&mut l, fs, SvfMode::LowPass, 1.3, cut);
        svf(&mut r, fs, SvfMode::LowPass, 1.3, cut);
        // Distortion (brass bite), then tame the fizz.
        let drive = 2.0 + 4.0 * it;
        for v in l.iter_mut().chain(r.iter_mut()) {
            *v = fast_tanh(*v * drive) / fast_tanh(drive) * 0.8;
        }
        for bx in [&mut l, &mut r] {
            biquad(bx, fs, FilterKind::LowPass, 9000.0, 0.7);
            peak(bx, fs, 300.0, 1.0, -2.5);
            peak(bx, fs, 1150.0, 0.9, 4.0);
        }
        let sub = sine(n, fs, |_| root, amp);
        self.center(&mut l, &mut r, &sub, 0.3);
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "hall", 0.4, 2.5);
        (l, r, 0.0, -1.0)
    }

    /// Stutter / bit-crush / buffer-repeat glitch.
    fn glitch(&mut self) -> Out {
        let d = self.dur(1.0);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        // Source: a bright arpeggio over noise blips.
        let src_n = n + self.n(0.2);
        let mut src = vec![0.0f32; src_n];
        let scale = [0.0f32, 3.0, 7.0, 10.0, 12.0, 15.0, 19.0];
        let step = self.n(0.045).max(1);
        let mut saw = Saw { ph: 0.0 };
        let mut note = 0.0f32;
        for (i, v) in src.iter_mut().enumerate() {
            if i % step == 0 {
                note = scale[self.rng.below(scale.len())];
            }
            let tt = (i % step) as f32 / fs;
            let f = 330.0 * ps * (note / 12.0).exp2();
            let sq = if (i as f32 * f / fs).fract() < 0.5 { 1.0 } else { -1.0 };
            *v = (0.5 * saw.tick(f, fs) + 0.3 * sq) * ad(tt, 0.001, 0.03);
        }
        let mut blips = self.white(src_n);
        biquad(&mut blips, fs, FilterKind::BandPass, 4000.0 * ps, 1.5);
        for (i, v) in blips.iter_mut().enumerate() {
            *v *= if (i / (step / 2 + 1)).is_multiple_of(5) { 0.5 } else { 0.0 };
        }
        add(&mut src, &blips, 1.0);
        // Slice on a 1/32 grid and mangle each slice.
        let bpm = self.bpm.unwrap_or(120.0).clamp(40.0, 300.0);
        let slice = self.n(60.0 / bpm / 8.0).max(64);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let fade = self.n(0.0015).max(1);
        let mut pos = 0;
        while pos < n {
            let len = slice.min(n - pos);
            let mode = self.rng.below(6);
            let pan = self.rng.range(-0.8, 0.8) * self.width;
            let (gl, gr) = pan_gains(pan);
            let rs = self.rng.range(0.008, 0.03);
            let rep = self.n(rs).max(16);
            let bits = 3.0 + self.rng.below(4) as f32;
            let hold = 2 + self.rng.below(12);
            for i in 0..len {
                let x = match mode {
                    0 => src[pos + i],
                    1 => src[pos + i % rep],
                    2 => {
                        let q = (2.0f32).powf(bits);
                        (src[pos + (i / hold) * hold] * q).round() / q
                    }
                    3 => src[pos + len - 1 - i],
                    4 => src[(pos + 2 * i).min(src_n - 1)],
                    _ => src[pos + i % rep] * (1.0 - i as f32 / len as f32),
                };
                let e = smooth(i as f32 / fade as f32) * smooth((len - i) as f32 / fade as f32);
                l[pos + i] = x * e * gl;
                r[pos + i] = x * e * gr;
            }
            pos += len;
        }
        // Make sure the first slice starts loud (the hit is at 0).
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "room", 0.2, 0.5);
        (l, r, 0.0, -3.0)
    }

    /// A struck resonator bank excited by a short noise burst.
    fn resonate(&mut self, n: usize, burst: f32, bands: &[(f32, f32, f32)]) -> Vec<f32> {
        let fs = self.fs;
        let mut ex = self.white(n);
        for (i, v) in ex.iter_mut().enumerate() {
            *v *= (-(i as f32 / fs) / burst).exp();
        }
        let mut out = vec![0.0f32; n];
        for &(f, q, g) in bands {
            let mut b = ex.clone();
            biquad(&mut b, fs, FilterKind::BandPass, f * self.ps, q);
            add(&mut out, &b, g);
        }
        out
    }

    fn tick(&mut self) -> Out {
        let n = self.n(self.dur(0.08));
        let x = self.resonate(n, 0.0006, &[(3200.0, 14.0, 1.0), (6100.0, 10.0, 0.6), (950.0, 5.0, 0.5)]);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &x, 1.0);
        self.verb(&mut l, &mut r, "ambience", 0.15, 0.2);
        (l, r, 0.0, -6.0)
    }

    fn click(&mut self) -> Out {
        let n = self.n(self.dur(0.05));
        let mut x = self.resonate(n, 0.0004, &[(2300.0, 3.0, 1.0), (6500.0, 2.0, 0.5)]);
        let tock = sine(n, self.fs, |_| 420.0 * self.ps, |t| ad(t, 0.0005, 0.006));
        add(&mut x, &tock, 0.12 * self.it);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &x, 1.0);
        (l, r, 0.0, -6.0)
    }

    fn pop(&mut self) -> Out {
        let n = self.n(self.dur(0.15));
        let fs = self.fs;
        let ps = self.ps;
        let mut x = sine(n, fs, |t| lerp_exp(320.0, 950.0, t / 0.03) * ps, |t| ad(t, 0.001, 0.035));
        let mut c = self.white(self.n(0.004).min(n));
        biquad(&mut c, fs, FilterKind::HighPass, 2000.0, 0.7);
        add(&mut x, &c, 0.15);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &x, 1.0);
        self.verb(&mut l, &mut r, "ambience", 0.2, 0.2);
        (l, r, 0.0, -6.0)
    }

    fn shutter(&mut self) -> Out {
        let d = self.dur(0.35);
        let n = self.n(d);
        let fs = self.fs;
        let mut x = vec![0.0f32; n];
        let first = self.resonate(self.n(0.05), 0.0012, &[(1800.0, 8.0, 0.8), (3700.0, 10.0, 1.0), (7200.0, 6.0, 0.5)]);
        add(&mut x, &first, 1.0);
        let slap = sine(self.n(0.04), fs, |_| 300.0 * self.ps, |t| ad(t, 0.001, 0.008));
        add(&mut x, &slap, 0.5);
        // Mirror / curtain whirr.
        let w0 = self.n(0.012);
        let wn = self.n(0.06).min(n - w0);
        let mut whirr = self.white(wn);
        biquad(&mut whirr, fs, FilterKind::BandPass, 5500.0 * self.ps, 1.5);
        for (i, v) in whirr.iter_mut().enumerate() {
            *v *= (std::f32::consts::PI * i as f32 / wn as f32).sin() * 0.25;
        }
        add(&mut x[w0..w0 + wn], &whirr, 1.0);
        let c2 = self.n(0.085 * d / 0.35).min(n.saturating_sub(self.n(0.05)));
        let second = self.resonate(self.n(0.05), 0.001, &[(2100.0, 8.0, 0.7), (4300.0, 10.0, 0.9), (8000.0, 6.0, 0.4)]);
        add(&mut x[c2..], &second, 0.8);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &x, 1.0);
        self.verb(&mut l, &mut r, "ambience", 0.2, 0.2);
        (l, r, 0.0, -6.0)
    }

    fn snap(&mut self) -> Out {
        let n = self.n(self.dur(0.25));
        let mut x = self.resonate(n, 0.0025, &[(2500.0, 2.2, 1.0), (1400.0, 6.0, 0.6), (5200.0, 3.0, 0.3)]);
        let thud = sine(n, self.fs, |_| 180.0 * self.ps, |t| ad(t, 0.001, 0.012));
        add(&mut x, &thud, 0.06);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &x, 1.0);
        self.verb(&mut l, &mut r, "room", 0.25, 0.4);
        (l, r, 0.0, -5.0)
    }

    /// Cinematic crescendo: pad chord + air rising to a peak.
    fn swell(&mut self) -> Out {
        let d = self.dur(3.0);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let peak = 0.92 * d;
        let env = move |t: f32| if t < peak { (t / peak).powf(2.0) } else { (-(t - peak) / (0.25 * (d - peak))).exp() };
        let f0 = 130.81 * ps;
        let notes = [1.0f32, 1.5, 2.0, 2.25, 3.0];
        let mut saws: Vec<[Saw; 4]> = (0..notes.len()).map(|_| [Saw { ph: 0.0 }; 4]).collect();
        for row in saws.iter_mut() {
            for s in row.iter_mut() {
                s.ph = self.rng.f32();
            }
        }
        let det = [-0.09f32, -0.03, 0.03, 0.09];
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        for i in 0..n {
            let t = i as f32 / fs;
            for (k, nf) in notes.iter().enumerate() {
                for (j, dt) in det.iter().enumerate() {
                    let v = saws[k][j].tick(f0 * nf * (dt / 12.0).exp2(), fs);
                    if (j + k) % 2 == 0 {
                        l[i] += v;
                    } else {
                        r[i] += v;
                    }
                }
            }
            let e = env(t) * 0.07;
            l[i] *= e;
            r[i] *= e;
        }
        let cut = move |t: f32| lerp_exp(300.0, 5500.0, (t / peak).min(1.0)) * ps.sqrt();
        svf(&mut l, fs, SvfMode::LowPass, 0.8, cut);
        svf(&mut r, fs, SvfMode::LowPass, 0.8, cut);
        // Partly blend the two voice sets so the pad stays mono-compatible.
        let blend = 0.5 * (1.0 - 0.5 * self.width);
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let (x, y) = (*a, *b);
            *a = x + blend * y;
            *b = y + blend * x;
        }
        let (mut al, mut ar) = self.noise2(n, true);
        for bx in [&mut al, &mut ar] {
            svf(bx, fs, SvfMode::BandPass, 0.9, move |t| lerp_exp(1500.0, 7000.0, (t / peak).min(1.0)));
            for (i, v) in bx.iter_mut().enumerate() {
                *v *= env(i as f32 / fs) * 0.25 * self.it;
            }
        }
        add(&mut l, &al, 1.0);
        add(&mut r, &ar, 1.0);
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "hall", 0.35, 2.0);
        (l, r, peak, -1.0)
    }

    fn sub_drop(&mut self) -> Out {
        let d = self.dur(2.5);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let tau = d * 0.45;
        let mut x = sine(n, fs, |t| (27.0 + 85.0 * (-t / (0.14 * d)).exp()) * ps, |t| ad(t, 0.003, tau));
        // Harmonics (2nd via bias, 3rd via tanh) so it reads on small speakers.
        let drive = 1.5 + 2.0 * self.it;
        for v in x.iter_mut() {
            *v = fast_tanh(drive * *v + 0.15) - fast_tanh(0.15);
        }
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &x, 1.0);
        (l, r, 0.0, -1.0)
    }

    fn zap(&mut self) -> Out {
        let d = self.dur(0.4);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        let (mut pc, mut pm) = (0.0f32, 0.0f32);
        let mut x = vec![0.0f32; n];
        for (i, v) in x.iter_mut().enumerate() {
            let t = i as f32 / fs;
            let fc = lerp_exp(3200.0, 160.0, (t / (0.85 * d)).min(1.0)) * ps;
            let idx = 3.0 * (-t / (0.3 * d)).exp() * (0.5 + self.it);
            pm = (pm + 0.5 * fc / fs).fract();
            pc = (pc + fc / fs).fract();
            let s = (std::f32::consts::TAU * pc + idx * (std::f32::consts::TAU * pm).sin()).sin();
            *v = fast_tanh(2.0 * s) * ad(t, 0.002, 0.3 * d);
        }
        biquad(&mut x, fs, FilterKind::LowPass, 9000.0, 0.7);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let m = self.motion.unwrap_or(0.0);
        self.place(&mut l, &mut r, &x, 1.0, move |t| m * (2.0 * t / d - 1.0));
        self.verb(&mut l, &mut r, "room", 0.3, 0.5);
        (l, r, 0.0, -3.0)
    }

    /// Bell partials with slight L/R beating; `(f0, amp, decay scale, pan)`.
    fn bell(&self, l: &mut [f32], r: &mut [f32], at: usize, (f0, amp, decay, pan): (f32, f32, f32, f32)) {
        const P: [(f32, f32, f32); 9] = [
            (0.5, 0.35, 1.0),
            (1.0, 1.0, 0.8),
            (1.19, 0.45, 0.6),
            (1.56, 0.4, 0.45),
            (2.0, 0.35, 0.4),
            (2.51, 0.22, 0.3),
            (2.66, 0.18, 0.25),
            (3.01, 0.14, 0.2),
            (4.1, 0.08, 0.12),
        ];
        let fs = self.fs;
        let n = l.len().saturating_sub(at);
        let (gl, gr) = pan_gains(pan);
        for &(ra, a, tau) in &P {
            let f = f0 * ra;
            if f > fs * 0.45 {
                continue;
            }
            let tau = tau * decay;
            let (wl, wr) = (std::f32::consts::TAU * (f - 0.12) / fs, std::f32::consts::TAU * (f + 0.12) / fs);
            let k = (-1.0 / (tau * fs)).exp();
            let (mut e, mut pl, mut pr) = (amp * a, 0.0f32, 0.0f32);
            let att = (0.0015 * fs) as usize;
            for i in 0..n {
                let g = if i < att { i as f32 / att as f32 } else { 1.0 };
                l[at + i] += pl.sin() * e * g * gl;
                r[at + i] += pr.sin() * e * g * gr;
                pl = (pl + wl) % std::f32::consts::TAU;
                pr = (pr + wr) % std::f32::consts::TAU;
                e = flush(e * k);
            }
        }
    }

    fn ding(&mut self) -> Out {
        let d = self.dur(2.5);
        let n = self.n(d);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.bell(&mut l, &mut r, 0, (1318.5 * self.ps, 0.3, d / 1.6, 0.0));
        let mut strike = self.white(self.n(0.004).min(n));
        biquad(&mut strike, self.fs, FilterKind::HighPass, 3000.0, 0.7);
        add(&mut l, &strike, 0.05);
        add(&mut r, &strike, 0.05);
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "plate", 0.3, 1.5);
        (l, r, 0.0, -4.0)
    }

    fn chime(&mut self) -> Out {
        let d = self.dur(3.0);
        let n = self.n(d);
        let count = self.count.unwrap_or(5).clamp(1, 16) as usize;
        let steps = [0.0f32, 4.0, 7.0, 9.0, 12.0, 16.0, 19.0, 21.0, 24.0];
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let gap = (0.07f32).min(d * 0.5 / count as f32);
        for k in 0..count {
            let jitter = if k > 0 { self.rng.range(0.0, 0.006) } else { 0.0 };
            let at = self.n(gap * k as f32 + jitter);
            if at >= n {
                break;
            }
            let semis = steps[k % steps.len()] + 12.0 * (k / steps.len()) as f32;
            let f = 1318.5 * self.ps * (semis / 12.0).exp2();
            let pan = (k as f32 / (count.max(2) - 1) as f32 * 2.0 - 1.0) * 0.7 * self.width;
            self.bell(&mut l, &mut r, at, (f, 0.22 * (1.0 - 0.08 * k as f32), d / 2.4, pan));
        }
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "plate", 0.45, 2.0);
        (l, r, 0.0, -4.0)
    }

    fn typing(&mut self) -> Out {
        let d = self.dur(2.0);
        let n = self.n(d);
        let fs = self.fs;
        let count = self.count.unwrap_or((d * 8.5).round().max(1.0) as u32) as usize;
        let mut times = vec![0.0f32];
        while times.len() < count {
            let last = *times.last().unwrap();
            let pause = if self.rng.f32() < 0.12 { self.rng.range(0.15, 0.3) } else { 0.0 };
            times.push(last + self.rng.range(0.06, 0.16) + pause);
        }
        // Stretch to fit the requested duration when counts are explicit.
        let span = times.last().copied().unwrap_or(0.0).max(1e-3);
        let limit = (d - 0.12).max(0.0);
        if span > limit && count > 1 {
            times.iter_mut().for_each(|t| *t *= limit / span);
        }
        let mut l = vec![0.0f32; n + self.n(0.2)];
        let mut r = l.clone();
        for (k, &t) in times.iter().enumerate() {
            let space = k % 7 == 5;
            let (hi, lo) = if space { (1600.0, 190.0) } else { (self.rng.range(1900.0, 3300.0), self.rng.range(260.0, 420.0)) };
            let pan = self.rng.range(-0.4, 0.4) * self.width;
            let kn = self.n(0.06);
            let down = self.resonate(kn, 0.0012, &[(hi, 2.5, 1.0), (hi * 2.1, 3.0, 0.5), (5200.0, 0.7, 0.35)]);
            let thock = sine(kn, fs, |_| lo * self.ps, |t| ad(t, 0.0008, if space { 0.03 } else { 0.015 }));
            let up = self.resonate(kn, 0.0008, &[(hi * 1.1, 2.5, 0.4), (5600.0, 0.8, 0.15)]);
            let at = self.n(t);
            let ud = self.rng.range(0.055, 0.1);
            let up_at = at + self.n(ud);
            let g = self.rng.range(0.75, 1.0);
            let (gl, gr) = pan_gains(pan);
            for i in 0..kn {
                if at + i < l.len() {
                    let v = (down[i] + thock[i] * if space { 0.45 } else { 0.15 }) * g;
                    l[at + i] += v * gl;
                    r[at + i] += v * gr;
                }
                if up_at + i < l.len() {
                    l[up_at + i] += up[i] * g * gl;
                    r[up_at + i] += up[i] * g * gr;
                }
            }
        }
        self.balance(&mut l, &mut r);
        self.verb(&mut l, &mut r, "ambience", 0.25, 0.3);
        (l, r, 0.0, -7.0)
    }

    fn heartbeat(&mut self) -> Out {
        let bpm = self.bpm.unwrap_or(70.0).clamp(30.0, 220.0);
        let period = 60.0 / bpm;
        let beats = match (self.count, self.duration) {
            (Some(c), _) => c.max(1) as usize,
            (None, Some(d)) => ((d / period).floor() as usize).max(1),
            (None, None) => 2,
        };
        let d = self.duration.unwrap_or(beats as f32 * period);
        let n = self.n(d.max((beats - 1) as f32 * period + 0.6));
        let fs = self.fs;
        let ps = self.ps;
        let mut x = vec![0.0f32; n];
        let dub_gap = (0.3 * period).min(0.32);
        for b in 0..beats {
            for (off, f_hi, f_lo, tau, g) in [(0.0, 80.0, 48.0, 0.07, 1.0f32), (dub_gap, 95.0, 60.0, 0.05, 0.7)] {
                let at = self.n(b as f32 * period + off);
                let len = self.n(0.35).min(n.saturating_sub(at));
                let s = sine(len, fs, |t| lerp_exp(f_hi, f_lo, t / 0.06) * ps, |t| ad(t, 0.008, tau));
                add(&mut x[at..at + len], &s, g);
                let mut th = self.pink(len);
                for (i, v) in th.iter_mut().enumerate() {
                    *v *= ad(i as f32 / fs, 0.004, tau * 0.6) * 0.3;
                }
                biquad(&mut th, fs, FilterKind::LowPass, 140.0 * ps, 0.7);
                add(&mut x[at..at + len], &th, g);
            }
        }
        let drive = 1.2 + self.it;
        for v in x.iter_mut() {
            *v = fast_tanh(drive * *v) / fast_tanh(drive);
        }
        biquad(&mut x, fs, FilterKind::LowPass, 380.0, 0.7);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        self.center(&mut l, &mut r, &x, 1.0);
        (l, r, 0.0, -2.0)
    }

    /// A chord played on "tape" that grinds to a halt; the hit is the stop.
    fn tape_stop(&mut self) -> Out {
        let d = self.dur(1.2);
        let n = self.n(d);
        let fs = self.fs;
        let ps = self.ps;
        // Source: bass + chord stabs, long enough for the whole read.
        let src_n = n + 16;
        let mut src_l = vec![0.0f32; src_n];
        let mut src_r = vec![0.0f32; src_n];
        let f0 = 65.41 * ps;
        let mut saws = [Saw { ph: 0.0 }, Saw { ph: 0.3 }, Saw { ph: 0.6 }, Saw { ph: 0.1 }, Saw { ph: 0.8 }];
        let ratios = [1.0f32, 2.0, 3.0, 4.0, 5.04];
        for i in 0..src_n {
            let t = i as f32 / fs;
            let stab = 0.55 + 0.45 * (-(t % 0.25) / 0.12).exp();
            for (k, s) in saws.iter_mut().enumerate() {
                let v = s.tick(f0 * ratios[k] * (1.0 + 0.002 * k as f32), fs) * stab / (1.0 + k as f32 * 0.4);
                if k % 2 == 0 {
                    src_l[i] += v;
                } else {
                    src_r[i] += v;
                }
                src_l[i] += v * 0.3;
                src_r[i] += v * 0.3;
            }
        }
        biquad(&mut src_l, fs, FilterKind::LowPass, 3500.0, 0.7);
        biquad(&mut src_r, fs, FilterKind::LowPass, 3500.0, 0.7);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let mut pos = 0.0f64;
        let mut lp = Svf::new(fs, SvfMode::LowPass, 8000.0, 0.7);
        for i in 0..n {
            let p = i as f32 / n as f32;
            let speed = (1.0 - p).max(0.0).powf(1.6);
            let j = pos as usize;
            let fr = (pos - j as f64) as f32;
            let at = |b: &[f32], k: usize| b[k.min(src_n - 1)];
            let vl = at(&src_l, j) + (at(&src_l, j + 1) - at(&src_l, j)) * fr;
            let vr = at(&src_r, j) + (at(&src_r, j + 1) - at(&src_r, j)) * fr;
            if i % 16 == 0 {
                lp.set((8000.0 * speed).max(60.0), 0.7);
            }
            let a = 0.12 * speed.sqrt().max(0.0);
            l[i] = lp.tick(0, vl) * a;
            r[i] = lp.tick(1, vr) * a;
            pos += speed as f64;
        }
        self.balance(&mut l, &mut r);
        (l, r, d, -2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: u32 = 48_000;

    /// 10 ms RMS envelope.
    fn envelope(s: &Sfx) -> Vec<f32> {
        let w = 480;
        s.l.chunks(w)
            .zip(s.r.chunks(w))
            .map(|(a, b)| (a.iter().chain(b).map(|x| x * x).sum::<f32>() / (2 * a.len()) as f32).sqrt())
            .collect()
    }

    #[test]
    fn all_render_clean_and_deterministic() {
        for name in SFX {
            let p = SfxParams { seed: 3, ..Default::default() };
            let a = sfx(name, &p, FS).unwrap();
            assert!(!a.l.is_empty() && a.l.len() == a.r.len(), "{name}");
            assert!(a.l.iter().chain(a.r.iter()).all(|x| x.is_finite()), "{name}: non-finite");
            let tp = crate::dsp::loudness::true_peak_db(&a.l, &a.r);
            assert!(tp <= -1.0 + 1e-3 && tp > -12.0, "{name}: true peak {tp}");
            assert!(a.hit >= 0.0 && a.hit <= a.duration(FS) + 1e-4, "{name}: hit {}", a.hit);
            let b = sfx(name, &p, FS).unwrap();
            assert_eq!(a, b, "{name}: not deterministic");
            let c = sfx(name, &SfxParams { seed: 4, ..p.clone() }, FS).unwrap();
            if !["sub-drop", "zap", "ding", "pop", "heartbeat", "tape-stop"].contains(name) {
                assert_ne!(a.l, c.l, "{name}: seed has no effect");
            }
            let dc = a.l.iter().map(|x| *x as f64).sum::<f64>() / a.l.len() as f64;
            assert!(dc.abs() < 2e-3, "{name}: dc {dc}");
        }
    }

    #[test]
    fn hit_points() {
        for name in SFX {
            let s = sfx(name, &SfxParams::default(), FS).unwrap();
            let env = envelope(&s);
            let peak = env.iter().cloned().fold(0.0f32, f32::max);
            let argmax = env.iter().position(|v| *v == peak).unwrap() as f32 * 0.01;
            let onset = env.iter().position(|v| *v > 0.1 * peak).unwrap() as f32 * 0.01;
            match *name {
                "whoosh" | "swoosh" => assert!((argmax - s.hit).abs() < 0.12, "{name}: hit {} vs loudest {argmax}", s.hit),
                "riser" | "reverse-cymbal" => {
                    assert!((s.hit - argmax).abs() < 0.1, "{name}: hit {} vs loudest {argmax}", s.hit);
                }
                "tape-stop" => assert!(s.hit > 1.0),
                "whip" => assert!((onset - s.hit).abs() < 0.3 && (argmax - s.hit).abs() < 0.03, "{name}: {} {argmax}", s.hit),
                "swell" => assert!((argmax - s.hit).abs() < 0.15, "{name}: {} {argmax}", s.hit),
                _ => {
                    assert_eq!(s.hit, 0.0, "{name}");
                    assert!(onset <= 0.03, "{name}: onset {onset}");
                }
            }
        }
        // The riser ends at its loudest and the hit is exactly the duration.
        let r = sfx("riser", &SfxParams { duration: Some(2.0), ..Default::default() }, FS).unwrap();
        assert_eq!(r.hit, 2.0);
    }

    #[test]
    fn extreme_params_are_safe() {
        let cases = [
            SfxParams { duration: Some(0.03), pitch: 24.0, intensity: 1.0, size: 0.0, width: 0.0, pan: -1.0, count: Some(1), ..Default::default() },
            SfxParams { duration: Some(8.0), pitch: -24.0, intensity: 0.0, size: 1.0, width: 1.0, pan: 1.0, count: Some(30), bpm: Some(200.0), ..Default::default() },
            SfxParams { motion: Some(-1.0), gain: 12.0, ..Default::default() },
        ];
        for p in &cases {
            for name in SFX {
                let s = sfx(name, p, FS).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(s.l.iter().chain(s.r.iter()).all(|x| x.is_finite()), "{name}");
                let tp = crate::dsp::loudness::true_peak_db(&s.l, &s.r);
                assert!(tp <= -1.0 + 1e-3, "{name}: {tp}");
            }
        }
    }

    #[test]
    fn names_and_errors() {
        assert!(sfx("reverse_cymbal", &SfxParams::default(), FS).is_ok());
        assert!(sfx("ReverseCymbal", &SfxParams::default(), FS).is_ok());
        assert!(sfx("subdrop", &SfxParams::default(), FS).is_ok());
        let e = sfx("whosh", &SfxParams::default(), FS).unwrap_err();
        assert!(e.contains("did you mean \"whoosh\""), "{e}");
        assert!(sfx("impact", &SfxParams { duration: Some(-1.0), ..Default::default() }, FS).is_err());
        assert!(serde_json::from_str::<SfxParams>(r#"{"pitch": 2, "sed": 1}"#).is_err());
        let p: SfxParams = serde_json::from_str(r#"{"pitch": -5, "duration": 2.5, "seed": 9, "intensity": 1}"#).unwrap();
        assert!(sfx("impact", &p, FS).is_ok());
    }

    #[test]
    fn spectra_match_their_character() {
        let bands = |name: &str| {
            let s = sfx(name, &SfxParams::default(), FS).unwrap();
            crate::dsp::analyze(&s.l, &s.r, FS as f32).bands
        };
        let sub = bands("sub-drop");
        assert!(sub.sub > -3.0, "sub-drop energy is sub: {sub:?}");
        let b = bands("impact");
        assert!(b.sub.max(b.low) > -6.0, "impact has weight: {b:?}");
        let t = bands("tick");
        assert!(t.high_mid > t.low && t.sub < -30.0, "tick is bright: {t:?}");
        let c = bands("reverse-cymbal");
        assert!(c.air > -3.0, "cymbal is air: {c:?}");
    }
}
