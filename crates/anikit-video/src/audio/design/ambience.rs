//! Synthesised ambience beds: room tone, studio, city, nature, wind, ocean,
//! rain, night, office, cafe, space. Each fills `duration` seconds, is
//! decorrelated left/right, has no audible loop point (nothing repeats), and
//! is normalised to a bed level (RMS) the mix bus then trims.

use super::synth::*;
use super::DesignParams;
use std::f32::consts::TAU;

/// Bed names.
pub const AMBIENCES: &[&str] = &[
    "room", "studio", "city", "nature", "forest", "wind", "ocean", "rain", "night", "office",
    "cafe", "space", "crowd", "fire", "vinyl", "tape",
];

/// Bed RMS in dBFS before the bus level (quiet rooms sit lower).
fn bed_rms(name: &str) -> f32 {
    match name {
        "studio" => -52.0,
        "room" => -46.0,
        "office" | "space" => -40.0,
        // Texture beds sit under everything, as a finish rather than a place.
        "vinyl" | "tape" => -44.0,
        _ => -36.0,
    }
}

pub(super) fn bed(name: &str, p: &DesignParams, sr: f32) -> Option<St> {
    let dur = p.duration.unwrap_or(10.0).clamp(0.5, 600.0);
    let len = frames(dur, sr);
    let seed = p.seed;
    let mut st = match name {
        "room" | "studio" => room(len, sr, seed),
        "wind" => wind(len, sr, seed, 1.0),
        "ocean" => ocean(len, sr, seed),
        "rain" => rain(len, sr, seed),
        "city" => city(len, sr, seed),
        "nature" | "forest" => nature(len, sr, seed),
        "night" => night(len, sr, seed),
        "office" => office(len, sr, seed),
        "cafe" => cafe(len, sr, seed),
        "space" => space(len, sr, seed),
        "crowd" => crowd(len, sr, seed),
        "fire" => fire(len, sr, seed),
        "vinyl" => vinyl(len, sr, seed),
        "tape" => tape(len, sr, seed),
        _ => return None,
    };
    st.l.truncate(len);
    st.r.truncate(len);
    st.normalize_rms(bed_rms(name));
    // Fade the edges so a bed never starts or stops with a click.
    let f = frames(0.25, sr).min(len / 4);
    for k in 0..f {
        let g = eqp(k as f32 / f as f32);
        st.l[k] *= g;
        st.r[k] *= g;
    }
    st.fade_end(f);
    Some(st)
}

/// Two decorrelated channels from a mono generator.
fn stereo(len: usize, seed: u64, mut gen: impl FnMut(&mut Noise, usize, usize) -> f32) -> St {
    let mut st = St::new(len);
    for ch in 0..2 {
        let mut n = Noise::new(seed.wrapping_mul(31).wrapping_add(ch as u64 * 0x9E37));
        let dst = if ch == 0 { &mut st.l } else { &mut st.r };
        for (k, d) in dst.iter_mut().enumerate() {
            *d = gen(&mut n, k, ch);
        }
    }
    st
}

fn room(len: usize, sr: f32, seed: u64) -> St {
    let mut st = stereo(len, seed, |n, _, _| n.pink());
    lowpass_st(&mut st, 900.0, sr);
    highpass_st(&mut st, 45.0, sr);
    // A whisper of air-handling hum (very low, broad).
    let hum = stereo(len, seed ^ 7, |n, _, _| n.brown());
    let mut hum = hum;
    lowpass_st(&mut hum, 180.0, sr);
    st.add(0, &hum, 0.5);
    st
}

fn wind(len: usize, sr: f32, seed: u64, amount: f32) -> St {
    let mut st = St::new(len);
    for ch in 0..2 {
        let mut n = Noise::new(seed ^ (0xA1 + ch as u64));
        let mut gust = Wander::new(0.12, sr);
        let mut wob = Wander::new(0.6, sr);
        let x = swept_noise_ctl(&mut n, len, sr, 2.2, |n, _| {
            let g = gust.next(n);
            let w = wob.next(n);
            let c = 380.0 * (1.0 + 0.9 * g + 0.2 * w).max(0.25);
            let a = (0.55 + 0.45 * g).max(0.05) * amount;
            (c, a)
        });
        let dst = if ch == 0 { &mut st.l } else { &mut st.r };
        dst.copy_from_slice(&x);
    }
    // Rumble.
    let mut rum = stereo(len, seed ^ 0x33, |n, _, _| n.brown());
    lowpass_st(&mut rum, 120.0, sr);
    st.add(0, &rum, 0.6 * amount);
    st
}

/// Band-passed pink noise whose centre and amplitude come from `ctl` (updated every 32 samples).
fn swept_noise_ctl(
    n: &mut Noise,
    len: usize,
    sr: f32,
    q: f32,
    mut ctl: impl FnMut(&mut Noise, usize) -> (f32, f32),
) -> Vec<f32> {
    use anikit_music::dsp::biquad::{Svf, SvfMode};
    let mut f = Svf::new(sr, SvfMode::BandPass, 500.0, q);
    let mut ctrl_noise = Noise::new(0xC0FFEE);
    let mut a = 0.0;
    let mut out = Vec::with_capacity(len);
    for k in 0..len {
        if k % 32 == 0 {
            let (c, a2) = ctl(&mut ctrl_noise, k);
            a = a2;
            f.set(c.clamp(30.0, sr * 0.45), q);
        }
        out.push(f.tick(0, n.pink() * 3.0) * a);
    }
    out
}

fn ocean(len: usize, sr: f32, seed: u64) -> St {
    let mut st = St::new(len);
    let mut n = Noise::new(seed ^ 0x0CEA);
    // Waves: overlapping swells every 5–10 s.
    let mut at = -(frames(n.range(1.0, 4.0), sr) as i64);
    while at < len as i64 {
        let wl = frames(n.range(6.0, 10.0), sr);
        let crest = n.range(0.35, 0.5);
        let pan = n.range(-0.5, 0.5);
        let level = n.range(0.6, 1.0);
        let wave = swept_noise(
            &mut n,
            wl,
            sr,
            0.6,
            true,
            |k| 250.0 + 2600.0 * bell(k, wl, crest, 2.0),
            |k| bell(k, wl, crest, 1.4) * level,
        );
        let start = at.max(0) as usize;
        let skip = (start as i64 - at) as usize;
        if skip < wave.len() {
            st.add_moving(start, &wave[skip..], pan - 0.2, pan + 0.2, 1.0);
        }
        at += frames(n.range(4.5, 8.0), sr) as i64;
    }
    let mut rum = stereo(len, seed ^ 0x77, |n, _, _| n.brown());
    lowpass_st(&mut rum, 200.0, sr);
    st.ensure(len);
    st.add(0, &rum, 0.35);
    st
}

fn rain(len: usize, sr: f32, seed: u64) -> St {
    let mut st = stereo(len, seed, |n, _, _| n.pink());
    highpass_st(&mut st, 900.0, sr);
    lowpass_st(&mut st, 9000.0, sr);
    st.scale(0.5);
    // Drops: Poisson clicks, ~350/s, panned wide.
    let mut n = Noise::new(seed ^ 0xD20);
    let mut k = 0usize;
    while k < len {
        k += frames(-(n.f().max(1e-4)).ln() / 350.0, sr).max(1);
        if k >= len {
            break;
        }
        let f = n.range(1800.0, 6500.0);
        let dl = frames(0.006, sr);
        let mut d: Vec<f32> = (0..dl)
            .map(|j| n.white() * env_ad(j, 2, dl as f32 * 0.2))
            .collect();
        bandpass(&mut d, f, 2.0, sr);
        let a = n.f().powf(3.0) * 0.8;
        let pan = n.range(-1.0, 1.0);
        st.add_mono(k, &d, pan, a);
    }
    st
}

fn city(len: usize, sr: f32, seed: u64) -> St {
    let mut st = stereo(len, seed, |n, _, _| n.brown());
    lowpass_st(&mut st, 260.0, sr);
    // Distant murmur band.
    let mut mur = stereo(len, seed ^ 0x11, |n, _, _| n.pink());
    highpass_st(&mut mur, 400.0, sr);
    lowpass_st(&mut mur, 1800.0, sr);
    st.add(0, &mur, 0.25);
    // Traffic passes every 2–7 s.
    let mut n = Noise::new(seed ^ 0xCA2);
    let mut at = 0usize;
    while at < len {
        let pl = frames(n.range(3.0, 6.0), sr);
        let dir = if n.f() < 0.5 { -1.0 } else { 1.0 };
        let lvl = n.range(0.3, 1.0);
        let car = swept_noise(
            &mut n,
            pl,
            sr,
            1.0,
            true,
            |k| 520.0 - 260.0 * (k as f32 / pl as f32),
            |k| bell(k, pl, 0.5, 2.0) * lvl,
        );
        st.ensure(at + pl);
        st.add_moving(at, &car, -0.8 * dir, 0.8 * dir, 0.9);
        at += frames(n.range(2.0, 7.0), sr);
    }
    st
}

/// A bird call: a trill of rising/falling FM chirps.
fn chirps(n: &mut Noise, sr: f32) -> Vec<f32> {
    let count = 2 + (n.f() * 6.0) as usize;
    let f0 = n.range(2400.0, 4200.0);
    let span = n.range(-0.5, 0.8);
    let cl = frames(n.range(0.04, 0.09), sr);
    let gap = frames(n.range(0.03, 0.09), sr);
    let mut out = vec![0.0f32; count * (cl + gap)];
    for c in 0..count {
        let mut ph = 0.0f32;
        let base = f0 * (1.0 + 0.05 * c as f32 * span.signum());
        for k in 0..cl {
            let x = k as f32 / cl as f32;
            let f = base * (1.0 + span * x) * (1.0 + 0.06 * (TAU * 35.0 * k as f32 / sr).sin());
            ph += TAU * f / sr;
            out[c * (cl + gap) + k] += ph.sin() * bell(k, cl, 0.3, 1.0);
        }
    }
    out
}

fn nature(len: usize, sr: f32, seed: u64) -> St {
    // Leaves: high, gusty, quiet.
    let mut st = wind(len, sr, seed ^ 0x1EAF, 0.35);
    let mut leaves = stereo(len, seed ^ 0x2EAF, |n, _, _| n.white());
    highpass_st(&mut leaves, 2500.0, sr);
    lowpass_st(&mut leaves, 7000.0, sr);
    let mut n = Noise::new(seed ^ 0x3EAF);
    let mut g = Wander::new(0.2, sr);
    for k in 0..len {
        let a = (0.4 + 0.6 * g.next(&mut n)).max(0.0) * 0.12;
        leaves.l[k] *= a;
        leaves.r[k] *= a;
    }
    st.add(0, &leaves, 1.0);
    // Birds.
    let mut at = frames(n.range(0.3, 1.5), sr);
    while at < len {
        let call = chirps(&mut n, sr);
        let pan = n.range(-0.85, 0.85);
        let lvl = n.range(0.04, 0.12);
        st.add_mono(at, &call, pan, lvl);
        at += frames(n.range(0.8, 4.0), sr);
    }
    reverb(&mut st, "hall", 1.2, 0.15, 0.0, sr);
    st
}

fn night(len: usize, sr: f32, seed: u64) -> St {
    let mut st = wind(len, sr, seed ^ 0x4167, 0.2);
    let mut n = Noise::new(seed ^ 0x2167);
    // Crickets: 3 voices, pulse trains at ~30 Hz in chirps of 3–4.
    for v in 0..3 {
        let f = n.range(4200.0, 4900.0);
        let pan = -0.7 + 0.7 * v as f32 + n.range(-0.1, 0.1);
        let lvl = n.range(0.02, 0.05);
        let period = frames(n.range(0.4, 0.7), sr);
        let mut at = frames(n.range(0.0, 0.5), sr);
        let pl = frames(0.012, sr);
        let pg = frames(0.033, sr);
        while at < len {
            let pulses = 3 + (n.f() * 2.0) as usize;
            for p in 0..pulses {
                let s = at + p * pg;
                let x: Vec<f32> = (0..pl)
                    .map(|k| (TAU * f * k as f32 / sr).sin() * bell(k, pl, 0.4, 1.0))
                    .collect();
                st.add_mono(s, &x, pan, lvl);
            }
            at += period;
        }
    }
    st
}

fn office(len: usize, sr: f32, seed: u64) -> St {
    let mut st = room(len, sr, seed);
    st.scale(3.0);
    let mut n = Noise::new(seed ^ 0x0FF1);
    // Distant typing bursts.
    let mut at = frames(n.range(0.5, 2.0), sr);
    while at < len {
        let keys = 4 + (n.f() * 14.0) as usize;
        let pan = n.range(-0.7, 0.7);
        let mut k = at;
        for _ in 0..keys {
            let cl: Vec<f32> = (0..frames(0.01, sr))
                .map(|j| n.white() * env_ad(j, 3, 0.002 * sr))
                .collect();
            let mut cl = cl;
            bandpass(&mut cl, n.range(1500.0, 3000.0), 1.5, sr);
            st.add_mono(k, &cl, pan, 0.05);
            k += frames(n.range(0.07, 0.2), sr);
        }
        at = k + frames(n.range(1.5, 5.0), sr);
    }
    st
}

fn cafe(len: usize, sr: f32, seed: u64) -> St {
    let mut st = room(len, sr, seed);
    // Murmur: eight "voices" of formant-filtered noise gated at syllable rate.
    let mut n = Noise::new(seed ^ 0xCAFE);
    for _ in 0..8 {
        let mut syl = Wander::new(5.0, sr);
        let mut phrase = Wander::new(0.25, sr);
        let pan = n.range(-0.8, 0.8);
        let lvl = n.range(0.3, 0.8);
        let f1 = n.range(400.0, 900.0);
        let mut x = Vec::with_capacity(len);
        for _ in 0..len {
            let gate = (syl.next(&mut n) * 1.8).max(0.0) * (phrase.next(&mut n) + 0.3).max(0.0);
            x.push(n.pink() * gate);
        }
        let mut y = x.clone();
        bandpass(&mut x, f1, 2.5, sr);
        bandpass(&mut y, f1 * 2.6, 3.0, sr);
        for (a, b) in x.iter_mut().zip(&y) {
            *a += 0.6 * b;
        }
        lowpass(&mut x, 2200.0, sr);
        st.add_mono(0, &x, pan, lvl * 0.4);
    }
    // Occasional cup clinks.
    let mut at = frames(n.range(1.0, 3.0), sr);
    while at < len {
        let base = n.range(2200.0, 3200.0);
        let exc = strike(&mut n, 0.5, 9000.0, sr);
        let c = modal(
            &exc,
            &[
                (base, 1.0, 0.3),
                (base * 1.53, 0.5, 0.2),
                (base * 2.48, 0.3, 0.1),
            ],
            sr,
            frames(0.35, sr),
        );
        st.add_mono(at, &c, n.range(-0.7, 0.7), 0.03);
        at += frames(n.range(2.0, 6.0), sr);
    }
    reverb(&mut st, "room", 0.8, 0.25, 0.0, sr);
    st
}

fn space(len: usize, sr: f32, seed: u64) -> St {
    let mut n = Noise::new(seed ^ 0x5BACE);
    let mut st = St::new(len);
    // Slowly beating low partials.
    let partials = [
        (41.2, 1.0),
        (61.8, 0.6),
        (82.4, 0.5),
        (123.6, 0.25),
        (164.8, 0.12),
    ];
    for (ch, dst) in [&mut st.l, &mut st.r].into_iter().enumerate() {
        for &(f, a) in &partials {
            let df = f * (1.0 + n.range(-0.004, 0.004)) + ch as f32 * 0.07;
            let mut ph = n.f() * TAU;
            let lfo_f = n.range(0.03, 0.09);
            for (k, d) in dst.iter_mut().enumerate() {
                ph += TAU * df / sr;
                if ph > TAU {
                    ph -= TAU;
                }
                let lfo = 0.7 + 0.3 * (TAU * lfo_f * k as f32 / sr).sin();
                *d += ph.sin() * a * lfo * 0.3;
            }
        }
    }
    let mut air = stereo(len, seed ^ 0x99, |n, _, _| n.pink());
    let mut w = Wander::new(0.05, sr);
    for k in 0..len {
        let a = 0.5 + 0.5 * w.next(&mut n);
        air.l[k] *= a;
        air.r[k] *= a;
    }
    bandpass_st(&mut air, 700.0, 0.8, sr);
    st.add(0, &air, 0.25);
    reverb(&mut st, "cathedral", 1.5, 0.3, 0.0, sr);
    st
}

fn bandpass_st(st: &mut St, hz: f32, q: f32, sr: f32) {
    bandpass(&mut st.l, hz, q, sr);
    bandpass(&mut st.r, hz, q, sr);
}

/// A crowd: many murmuring voices, denser and wider than `cafe`, no clinks.
fn crowd(len: usize, sr: f32, seed: u64) -> St {
    let mut st = St::new(len);
    let mut n = Noise::new(seed ^ 0xC20D);
    for _ in 0..20 {
        let mut syl = Wander::new(n.range(3.5, 6.0), sr);
        let mut phrase = Wander::new(0.3, sr);
        let pan = n.range(-1.0, 1.0);
        let f1 = n.range(350.0, 900.0);
        let mut x = Vec::with_capacity(len);
        for _ in 0..len {
            let gate = (syl.next(&mut n) * 1.6).max(0.0) * (phrase.next(&mut n) + 0.4).max(0.0);
            x.push(n.pink() * gate);
        }
        let mut y = x.clone();
        bandpass(&mut x, f1, 2.2, sr);
        bandpass(&mut y, f1 * 2.4, 2.8, sr);
        for (a, b) in x.iter_mut().zip(&y) {
            *a += 0.5 * b;
        }
        lowpass(&mut x, 2500.0, sr);
        st.add_mono(0, &x, pan, n.range(0.2, 0.5));
    }
    reverb(&mut st, "hall", 0.9, 0.3, 0.0, sr);
    st
}

/// Fire: a low roar breathing slowly, with crackles and the odd pop.
fn fire(len: usize, sr: f32, seed: u64) -> St {
    let mut st = stereo(len, seed ^ 0xF12E, |n, _, _| n.brown());
    lowpass_st(&mut st, 400.0, sr);
    let mut n = Noise::new(seed ^ 0xF1);
    let mut w = Wander::new(0.4, sr);
    for k in 0..len {
        let a = 0.6 + 0.4 * w.next(&mut n);
        st.l[k] *= a;
        st.r[k] *= a;
    }
    let mut k = 0usize;
    while k < len {
        k += frames(-(n.f().max(1e-4)).ln() / 25.0, sr).max(1);
        if k >= len {
            break;
        }
        let big = n.f() < 0.08;
        let cl = frames(if big { 0.012 } else { 0.003 }, sr).max(3);
        let mut c: Vec<f32> = (0..cl).map(|j| n.white() * env_ad(j, 1, cl as f32 * 0.25)).collect();
        bandpass(&mut c, n.range(1500.0, 6000.0), 1.0, sr);
        st.add_mono(k, &c, n.range(-0.6, 0.6), if big { 1.2 } else { n.f().powi(2) * 0.6 });
    }
    st
}

/// Vinyl: surface hiss, crackle (sparse clicks of random size) and a low rumble.
fn vinyl(len: usize, sr: f32, seed: u64) -> St {
    let mut st = stereo(len, seed ^ 0x717, |n, _, _| n.pink());
    highpass_st(&mut st, 2500.0, sr);
    lowpass_st(&mut st, 9000.0, sr);
    st.scale(0.25);
    let mut rum = stereo(len, seed ^ 0x718, |n, _, _| n.brown());
    lowpass_st(&mut rum, 60.0, sr);
    st.add(0, &rum, 0.12);
    let mut n = Noise::new(seed ^ 0x719);
    let mut k = 0usize;
    while k < len {
        k += frames(-(n.f().max(1e-4)).ln() / 18.0, sr).max(1);
        if k >= len {
            break;
        }
        let cl = frames(n.range(0.0002, 0.0015), sr).max(2);
        let mut c: Vec<f32> = (0..cl).map(|j| n.white() * (1.0 - j as f32 / cl as f32)).collect();
        highpass(&mut c, 1200.0, sr);
        st.add_mono(k, &c, n.range(-0.3, 0.3), n.f().powi(3) * 3.0);
    }
    st
}

/// Tape: hiss with a slow wow in its colour and level.
fn tape(len: usize, sr: f32, seed: u64) -> St {
    let mut st = stereo(len, seed ^ 0x7A9E, |n, _, _| n.white());
    highpass_st(&mut st, 1800.0, sr);
    lowpass_st(&mut st, 12000.0, sr);
    let mut n = Noise::new(seed ^ 0x7A);
    let mut w = Wander::new(0.7, sr);
    for k in 0..len {
        let a = 0.85 + 0.15 * w.next(&mut n);
        st.l[k] *= a;
        st.r[k] *= a;
    }
    st
}
