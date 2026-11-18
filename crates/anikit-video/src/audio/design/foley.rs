//! Procedural foley and motion sounds: materials struck with modal resonator
//! banks (glass, metal, wood, plastic, stone, ceramic), granular textures
//! (paper, cloth, shatter, sparkle) and motion-driven air (whoosh, swish,
//! passby) whose spectrum and length follow on-screen speed.
//!
//! Each returns a stereo buffer and its hit point (seconds from the start).

use super::synth::*;
use super::DesignParams;
use std::f32::consts::TAU;

pub(super) struct Ctx<'a> {
    pub sr: f32,
    pub p: &'a DesignParams,
    pub n: Noise,
}

impl Ctx<'_> {
    /// Pitch factor from `pitch` semitones.
    fn pf(&self) -> f32 {
        (self.p.pitch / 12.0).exp2()
    }
    fn i(&self) -> f32 {
        self.p.intensity.clamp(0.0, 1.0)
    }
    /// `f` snapped to the tuning root's pitch class (when tuned), then pitched.
    fn tuned(&self, f: f32) -> f32 {
        let f = match self.p.tune {
            Some(root) if root > 0.0 => {
                let k = (12.0 * (f / root).log2()).round();
                root * (k / 12.0).exp2()
            }
            _ => f,
        };
        f * self.pf()
    }
    /// A pentatonic note near `f` (major pentatonic over the tuning root, or A).
    fn penta(&mut self, lo: f32, hi: f32) -> f32 {
        const STEPS: [f32; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];
        let root = self.p.tune.unwrap_or(440.0);
        let f = self.n.range(lo, hi);
        let semis = 12.0 * (f / root).log2();
        let oct = (semis / 12.0).floor();
        let within = semis - oct * 12.0;
        let step = STEPS
            .iter()
            .copied()
            .min_by(|a, b| (a - within).abs().total_cmp(&(b - within).abs()))
            .unwrap();
        root * ((oct * 12.0 + step) / 12.0).exp2() * self.pf()
    }
    fn dur(&self, def: f32) -> f32 {
        self.p.duration.unwrap_or(def).clamp(0.03, 60.0)
    }
}

/// A struck material: modes `(ratio, amp, t60)` over a base frequency.
fn struck(
    c: &mut Ctx,
    base: f32,
    modes: &[(f32, f32, f32)],
    strike_ms: f32,
    bright: f32,
    detune: f32,
) -> St {
    let sr = c.sr;
    let longest = modes.iter().map(|m| m.2).fold(0.0f32, f32::max);
    let len = frames(longest * 1.1 + 0.05, sr);
    let exc = strike(&mut c.n, strike_ms, bright, sr);
    // Slight per-channel mode detune gives a natural stereo shimmer.
    let ml: Vec<_> = modes.iter().map(|&(r, a, t)| (base * r, a, t)).collect();
    let mr: Vec<_> = modes
        .iter()
        .map(|&(r, a, t)| (base * r * (1.0 + detune), a, t * 0.97))
        .collect();
    let mut st = St {
        l: modal(&exc, &ml, sr, len),
        r: modal(&exc, &mr, sr, len),
    };
    // The strike click itself (the transient layer).
    let mut click = exc.clone();
    highpass(&mut click, 1500.0, sr);
    st.add_mono(0, &click, 0.0, 0.25);
    st
}

pub(super) fn glass(c: &mut Ctx) -> (St, f32) {
    let i = c.i();
    let base = {
        let f = c.n.range(1400.0, 2200.0);
        c.tuned(f)
    };
    let ring = 0.6 + 1.2 * i;
    let modes = [
        (1.0, 1.0, ring),
        (2.32, 0.55, ring * 0.6),
        (4.25, 0.35, ring * 0.35),
        (6.63, 0.22, ring * 0.2),
        (9.38, 0.12, ring * 0.12),
    ];
    let mut st = struck(c, base, &modes, 0.8, 9000.0, 0.0035);
    reverb(&mut st, "plate", 0.6, 0.55 * c.p.size, 0.8, c.sr);
    (st, 0.0)
}

pub(super) fn metal(c: &mut Ctx) -> (St, f32) {
    let i = c.i();
    let base = {
        let f = c.n.range(320.0, 520.0);
        c.tuned(f)
    };
    let ring = 0.8 + 1.8 * i;
    // Free-bar partials plus plate-like inharmonic clang.
    let modes = [
        (1.0, 1.0, ring),
        (1.59, 0.5, ring * 0.7),
        (2.14, 0.45, ring * 0.6),
        (2.756, 0.6, ring * 0.5),
        (3.16, 0.3, ring * 0.4),
        (5.404, 0.35, ring * 0.3),
        (8.933, 0.2, ring * 0.15),
    ];
    let mut st = struck(c, base, &modes, 1.5, 7000.0, 0.004);
    reverb(&mut st, "hall", 0.7, 0.55 * c.p.size, 1.2, c.sr);
    (st, 0.0)
}

pub(super) fn wood(c: &mut Ctx) -> (St, f32) {
    let base = {
        let f = c.n.range(220.0, 340.0);
        c.tuned(f)
    };
    let d = 0.05 + 0.08 * c.i();
    let modes = [
        (1.0, 1.0, d * 1.6),
        (2.57, 0.7, d),
        (4.87, 0.4, d * 0.6),
        (7.15, 0.2, d * 0.4),
    ];
    let mut st = struck(c, base, &modes, 2.5, 3500.0, 0.002);
    // Body thump.
    let sr = c.sr;
    let thump = sine_glide(
        frames(0.09, sr),
        sr,
        base * 0.45,
        base * 0.3,
        frames(0.05, sr),
        |k| env_ad(k, 30, 0.02 * sr),
    );
    st.add_mono(0, &thump, 0.0, 0.5);
    reverb(&mut st, "room", 0.5, 0.4 * c.p.size, 0.4, sr);
    (st, 0.0)
}

pub(super) fn plastic(c: &mut Ctx) -> (St, f32) {
    let base = {
        let f = c.n.range(900.0, 1400.0);
        c.tuned(f)
    };
    let modes = [
        (1.0, 1.0, 0.035),
        (1.71, 0.7, 0.025),
        (2.93, 0.4, 0.018),
        (4.3, 0.25, 0.012),
    ];
    let mut st = struck(c, base, &modes, 0.6, 8000.0, 0.003);
    reverb(&mut st, "room", 0.4, 0.28 * c.p.size, 0.3, c.sr);
    (st, 0.0)
}

pub(super) fn ceramic(c: &mut Ctx) -> (St, f32) {
    let base = {
        let f = c.n.range(1800.0, 2600.0);
        c.tuned(f)
    };
    let modes = [
        (1.0, 1.0, 0.25),
        (1.53, 0.6, 0.18),
        (2.48, 0.45, 0.12),
        (3.9, 0.3, 0.08),
    ];
    let mut st = struck(c, base, &modes, 0.7, 10000.0, 0.003);
    reverb(&mut st, "room", 0.5, 0.45 * c.p.size, 0.4, c.sr);
    (st, 0.0)
}

pub(super) fn stone(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let base = c.n.range(110.0, 170.0) * c.pf();
    let modes = [
        (1.0, 1.0, 0.07),
        (1.47, 0.8, 0.05),
        (2.09, 0.6, 0.04),
        (2.83, 0.4, 0.03),
        (3.6, 0.3, 0.02),
    ];
    let mut st = struck(c, base, &modes, 6.0, 1200.0, 0.01);
    let mut grit: Vec<f32> = (0..frames(0.06, sr))
        .map(|k| c.n.white() * env_ad(k, 10, 0.012 * sr))
        .collect();
    bandpass(&mut grit, 2500.0, 0.8, sr);
    st.add_mono(0, &grit, 0.0, 0.4);
    reverb(&mut st, "room", 0.6, 0.35 * c.p.size, 0.4, sr);
    (st, 0.0)
}

/// A soft landing: a pitched-down sine body, a short low-passed click and a
/// faint material ring. The UI "settle" sound.
pub(super) fn thump(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let i = c.i();
    let f0 = 95.0 * c.pf();
    let len = frames(0.18 + 0.15 * i, sr);
    let body = sine_glide(len, sr, f0 * 1.6, f0, frames(0.04, sr), |k| {
        env_ad(k, frames(0.002, sr), (0.045 + 0.06 * i) * sr)
    });
    let mut click: Vec<f32> = (0..frames(0.012, sr))
        .map(|k| c.n.white() * env_ad(k, 4, 0.003 * sr))
        .collect();
    lowpass(&mut click, 2500.0 + 3000.0 * i, sr);
    // Phone speakers start around 150–200 Hz: a little saturation puts
    // harmonics of the 60–150 Hz body where they can be heard.
    let body: Vec<f32> = body.iter().map(|x| 0.7 * x + 0.3 * soft(4.0 * x)).collect();
    let mut st = St::new(len);
    st.add_mono(0, &body, 0.0, 0.9);
    st.add_mono(0, &click, 0.0, 0.5);
    (st, 0.0)
}

/// Paper: crinkle grains over a light slide noise; `page` adds a flap.
pub(super) fn paper(c: &mut Ctx, page: bool) -> (St, f32) {
    let sr = c.sr;
    let dur = c.dur(if page { 0.45 } else { 0.6 });
    let len = frames(dur, sr);
    let mut st = St::new(len + frames(0.05, sr));
    // Slide bed.
    let bed = swept_noise(
        &mut c.n,
        len,
        sr,
        0.7,
        false,
        |_| 3200.0,
        |k| bell(k, len, 0.5, 1.0) * 0.12,
    );
    st.add_mono(0, &bed, 0.0, 1.0);
    // Crinkle: Poisson grains, density following a bell.
    let rate = 90.0 + 160.0 * c.i();
    let mut k = 0usize;
    while k < len {
        let dens = bell(k, len, if page { 0.75 } else { 0.4 }, 0.8);
        k += frames(-((c.n.f().max(1e-4)).ln()) / (rate * dens.max(0.05)), sr).max(1);
        if k >= len {
            break;
        }
        let glen = frames(c.n.range(0.0006, 0.004), sr).max(4);
        let f = c.n.range(1800.0, 7000.0);
        let mut g: Vec<f32> = (0..glen)
            .map(|j| c.n.white() * env_ad(j, 2, glen as f32 * 0.3))
            .collect();
        bandpass(&mut g, f, 1.2, sr);
        let a = c.n.f().powf(2.0) * dens;
        let pan = c.n.range(-0.4, 0.4);
        st.add_mono(k, &g, pan, a * 1.4);
    }
    if page {
        // The flap: a short low whoosh near the end.
        let fl = frames(0.12, sr);
        let flap = swept_noise(
            &mut c.n,
            fl,
            sr,
            1.2,
            true,
            |j| 600.0 + 900.0 * j as f32 / fl as f32,
            |j| bell(j, fl, 0.6, 1.5) * 0.6,
        );
        st.add_mono(len.saturating_sub(fl + frames(0.03, sr)), &flap, 0.1, 1.0);
    }
    highpass_st(&mut st, 250.0, sr);
    (st, dur * if page { 0.8 } else { 0.4 })
}

/// Cloth rustle / swish: mid-band noise with a fast random flutter.
pub(super) fn cloth(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let dur = c.dur(0.45);
    let len = frames(dur, sr);
    let mut fl = Wander::new(14.0, sr);
    let mut fl2 = Wander::new(14.0, sr);
    let mut n2 = Noise::new(c.p.seed ^ 0x55);
    let mut st = St::new(len);
    for (ch, (wander, nz)) in [(&mut fl, &mut c.n), (&mut fl2, &mut n2)]
        .into_iter()
        .enumerate()
    {
        let mut x = Vec::with_capacity(len);
        for k in 0..len {
            let flutter = 0.55 + 0.45 * wander.next(nz);
            x.push(nz.pink() * bell(k, len, 0.45, 1.2) * flutter.max(0.0));
        }
        bandpass(&mut x, 1100.0, 0.6, sr);
        highpass(&mut x, 280.0, sr);
        let dst = if ch == 0 { &mut st.l } else { &mut st.r };
        for (d, s) in dst.iter_mut().zip(x) {
            *d += s;
        }
    }
    (st, dur * 0.45)
}

/// Motion air: a whoosh whose length, brightness and weight follow `speed`
/// (0..1 of the frame per … see [`DesignParams::speed`]) and whose stereo
/// position travels with `motion`. Doppler-like: the band rises on approach
/// and falls about a fifth on the way out; the loudest point is the hit.
pub(super) fn air(c: &mut Ctx, kind: Air) -> (St, f32) {
    let sr = c.sr;
    let speed = c.p.speed.clamp(0.0, 1.0);
    let i = c.i();
    let (def, lo, hi, q, body, peak) = match kind {
        Air::Soft => (
            0.6 - 0.25 * speed,
            250.0,
            1400.0 + 1600.0 * speed,
            0.9,
            0.35,
            0.6,
        ),
        Air::Swish => (
            0.22 + 0.1 * (1.0 - speed),
            900.0,
            3500.0 + 3500.0 * speed,
            1.6,
            0.15,
            0.55,
        ),
        Air::Whoosh => (
            0.75 - 0.3 * speed,
            300.0,
            1800.0 + 2600.0 * speed,
            1.2,
            0.6,
            0.62,
        ),
        Air::Deep => (
            1.1 - 0.3 * speed,
            120.0,
            900.0 + 900.0 * speed,
            1.0,
            1.0,
            0.65,
        ),
        Air::Passby => (
            1.4 - 0.5 * speed,
            200.0,
            1500.0 + 2500.0 * speed,
            1.4,
            0.7,
            0.5,
        ),
    };
    let dur = c.dur(def.max(0.12));
    let len = frames(dur, sr);
    let pf = c.pf() * (0.85 + 0.3 * c.p.tone);
    let centre = |k: usize| {
        let x = k as f32 / len as f32;
        let e = bell(k, len, peak, 1.0);
        let f = lo + (hi - lo) * e.powf(1.3);
        // Recede: a fifth lower after the peak.
        let after = if x > peak {
            1.0 - 0.33 * ((x - peak) / (1.0 - peak))
        } else {
            1.0
        };
        f * after * pf
    };
    let amp = |k: usize| bell(k, len, peak, 1.6 - 0.6 * speed);
    let a = swept_noise(&mut c.n, len, sr, q, true, centre, amp);
    let b = swept_noise(&mut c.n, len, sr, q, true, |k| centre(k) * 1.07, amp);
    let travel = c.p.motion.clamp(-1.0, 1.0) * 0.8;
    let mut st = St::new(len);
    st.add_moving(0, &a, c.p.pan - travel, c.p.pan + travel, 0.8);
    st.add_moving(0, &b, c.p.pan - travel * 0.6, c.p.pan + travel * 0.6, 0.6);
    if body > 0.0 {
        // Low body for weight.
        let lb = swept_noise(
            &mut c.n,
            len,
            sr,
            0.8,
            true,
            |k| (centre(k) * 0.3).max(60.0),
            |k| bell(k, len, peak, 2.0),
        );
        st.add_moving(
            0,
            &lb,
            c.p.pan - travel * 0.3,
            c.p.pan + travel * 0.3,
            body * (0.6 + 0.6 * i),
        );
    }
    if matches!(kind, Air::Whoosh | Air::Passby) && i > 0.5 {
        // A whistle partial for fast, hard objects.
        let w = swept_noise(
            &mut c.n,
            len,
            sr,
            9.0,
            false,
            |k| centre(k) * 2.2,
            |k| bell(k, len, peak, 3.0),
        );
        st.add_moving(0, &w, c.p.pan - travel, c.p.pan + travel, 0.18 * (i - 0.5));
    }
    (st, dur * peak)
}

#[derive(Clone, Copy)]
pub(super) enum Air {
    Soft,
    Swish,
    Whoosh,
    Deep,
    Passby,
}

/// Sparkle: pentatonic sine grains (tuned to the key when given) with a
/// rising density, a high shimmer band and a plate tail. `magic` adds an
/// upward swell under it.
pub(super) fn sparkle(c: &mut Ctx, magic: bool) -> (St, f32) {
    let sr = c.sr;
    let dur = c.dur(if magic { 1.2 } else { 0.7 });
    let len = frames(dur, sr);
    let count = c.p.count.unwrap_or(if magic { 26 } else { 14 }) as usize;
    let mut st = St::new(len + frames(0.4, sr));
    for g in 0..count {
        // Grains bunch towards the start for sparkle, rise for magic.
        let u = (g as f32 + c.n.f()) / count as f32;
        let at = if magic { u.powf(0.7) } else { u.powf(1.6) } * dur * 0.85;
        let f = c.penta(2000.0, 6500.0);
        let gl = frames(c.n.range(0.06, 0.25), sr);
        let att = frames(0.0015, sr).max(2);
        let tau = gl as f32 * 0.3;
        let x: Vec<f32> = {
            let mut ph = c.n.f() * TAU;
            (0..gl)
                .map(|k| {
                    ph += TAU * f / sr;
                    (ph.sin() + 0.25 * (2.0 * ph).sin()) * env_ad(k, att, tau)
                })
                .collect()
        };
        let a = 0.25 + 0.75 * c.n.f();
        let pan = c.n.range(-0.8, 0.8);
        st.add_mono(frames(at, sr), &x, pan, a * 0.3);
    }
    // Shimmer band.
    let sh = swept_noise(
        &mut c.n,
        len,
        sr,
        3.0,
        false,
        |k| 7000.0 + 3000.0 * k as f32 / len as f32,
        |k| bell(k, len, 0.4, 1.0) * 0.12,
    );
    st.add_mono(0, &sh, 0.0, 1.0);
    if magic {
        let sw = swept_noise(
            &mut c.n,
            len,
            sr,
            2.0,
            true,
            |k| 400.0 * (8.0f32).powf(k as f32 / len as f32),
            |k| bell(k, len, 0.85, 1.5) * 0.5,
        );
        st.add_moving(0, &sw, -0.3, 0.3, 1.0);
    }
    reverb(&mut st, "plate", 0.8, 0.93 * c.p.size, 1.2, sr);
    (st, if magic { dur * 0.85 } else { 0.0 })
}

/// Shatter: a low hit, a bright noise burst and a shower of glass grains with
/// decaying density.
pub(super) fn shatter(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let dur = c.dur(0.9);
    let len = frames(dur, sr);
    let mut st = St::new(len + frames(0.3, sr));
    let burst: Vec<f32> = (0..frames(0.08, sr))
        .map(|k| c.n.white() * env_ad(k, 8, 0.02 * sr))
        .collect();
    let mut b2 = burst.clone();
    highpass(&mut b2, 2500.0, sr);
    st.add_mono(0, &b2, 0.0, 0.8);
    let body = sine_glide(frames(0.25, sr), sr, 140.0, 55.0, frames(0.12, sr), |k| {
        env_ad(k, 20, 0.06 * sr)
    });
    st.add_mono(0, &body, 0.0, 0.7 * (0.5 + c.i()));
    let n = (50.0 + 60.0 * c.i()) as usize;
    for _ in 0..n {
        let at = c.n.f().powf(2.5) * dur;
        let base = c.n.range(2500.0, 9000.0) * c.pf();
        let ring = c.n.range(0.03, 0.25);
        let exc = strike(&mut c.n, 0.3, 12000.0, sr);
        let g = modal(
            &exc,
            &[
                (base, 1.0, ring),
                (base * 2.32, 0.5, ring * 0.6),
                (base * 1.41, 0.4, ring * 0.7),
            ],
            sr,
            frames(ring * 1.2, sr),
        );
        let a = (1.0 - at / dur).powf(1.5) * c.n.range(0.2, 1.0);
        let pan = c.n.range(-0.9, 0.9);
        st.add_mono(frames(at, sr), &g, pan, a * 0.5);
    }
    reverb(&mut st, "room", 0.7, 0.5 * c.p.size, 0.5, sr);
    (st, 0.0)
}

/// Droplet: a damped sine whose pitch rises quickly (bubble resonance).
pub(super) fn droplet(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let f0 = {
        let f = c.n.range(600.0, 900.0);
        c.tuned(f)
    };
    let len = frames(0.08, sr);
    let x = sine_glide(len, sr, f0, f0 * 2.4, frames(0.025, sr), |k| {
        env_ad(k, 8, 0.018 * sr)
    });
    let mut st = St::new(len);
    st.add_mono(0, &x, c.p.pan, 0.9);
    reverb(&mut st, "room", 0.4, 0.45 * c.p.size, 0.3, sr);
    (st, 0.0)
}

/// Digital blip: a short tuned square-ish beep with a pitch drop (UI confirm).
pub(super) fn blip(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let f0 = c.tuned(1320.0);
    let len = frames(0.07, sr);
    let mut ph = 0.0f32;
    let x: Vec<f32> = (0..len)
        .map(|k| {
            let f = f0 * (1.0 - 0.25 * (k as f32 / len as f32));
            ph += TAU * f / sr;
            soft(2.5 * ph.sin()) * env_ad(k, 20, 0.02 * sr)
        })
        .collect();
    let mut st = St::new(len);
    st.add_mono(0, &x, c.p.pan, 0.8);
    lowpass_st(&mut st, 6000.0, sr);
    (st, 0.0)
}

/// Data / ticker: a quick burst of soft clicks (counters, data streams).
pub(super) fn ticker(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let dur = c.dur(0.5);
    let n = c.p.count.unwrap_or(10).clamp(1, 120) as usize;
    let len = frames(dur, sr);
    let mut st = St::new(len + frames(0.03, sr));
    for k in 0..n {
        let at = len * k / n;
        let f = c.tuned(2400.0) * (1.0 + 0.04 * c.n.white());
        let cl = sine_glide(frames(0.006, sr), sr, f, f * 0.8, frames(0.006, sr), |j| {
            env_ad(j, 2, 0.0015 * sr)
        });
        st.add_mono(at, &cl, c.p.pan, 0.5 + 0.2 * c.n.f());
    }
    (st, 0.0)
}

/// Shepard riser: octave-spaced partials glide up together under a fixed
/// bell over log-frequency, so it climbs without ever arriving, with filtered
/// noise rising under it. The hit is the end (cut it on the reveal).
pub(super) fn shepard(c: &mut Ctx) -> (St, f32) {
    let sr = c.sr;
    let dur = c.dur(3.0);
    let len = frames(dur, sr);
    let octaves_up = 1.0 + 1.5 * c.i(); // how far each partial climbs
    let base = 55.0 * c.pf();
    let n_p = 8;
    let mut st = St::new(len);
    for ch in 0..2 {
        let mut x = vec![0.0f32; len];
        for p in 0..n_p {
            let mut ph = (p as f32 * 1.7 + ch as f32 * 0.3) % TAU;
            for (k, o) in x.iter_mut().enumerate() {
                let u = k as f32 / len as f32;
                // Position in a cycle of `n_p` octaves.
                let pos = (p as f32 + octaves_up * u) % n_p as f32;
                let f = base * pos.exp2();
                ph += TAU * f / sr;
                if ph > TAU {
                    ph -= TAU;
                }
                let w = (std::f32::consts::PI * pos / n_p as f32).sin().powi(2);
                *o += ph.sin() * w;
            }
        }
        let dst = if ch == 0 { &mut st.l } else { &mut st.r };
        for (k, (d, s)) in dst.iter_mut().zip(x).enumerate() {
            let u = k as f32 / len as f32;
            // Swell in, peak at the end.
            *d = s * (0.15 + 0.85 * u * u) * 0.25;
        }
    }
    let air = swept_noise(&mut c.n, len, sr, 1.5, true, |k| 300.0 * (20.0f32).powf(k as f32 / len as f32), |k| {
        let u = k as f32 / len as f32;
        u * u * 0.5
    });
    st.add_moving(0, &air, -0.4, 0.4, 1.0);
    st.fade_end(frames(0.01, sr));
    (st, dur)
}
