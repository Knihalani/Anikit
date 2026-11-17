//! Physical models, synth presets and drum kits: tuning, termination,
//! finiteness, levels, determinism, glide, chokes, velocity timbre and
//! oscillator aliasing.
//!
//! These render a lot of audio; `cargo test --release -p anikit-music` is
//! much faster than a debug run.

mod support;

use anikit_music::synth::osc::{Lanes, Wave};
use anikit_music::{physical, synth, Instrument, Rng, BLOCK, RATE};
use std::sync::Arc;
use support::*;

fn physical_all() -> Vec<(&'static str, Arc<dyn Instrument>)> {
    physical::PRESETS
        .iter()
        .map(|p| (*p, physical::preset(p).unwrap()))
        .collect()
}

fn synth_all() -> Vec<(&'static str, Arc<dyn Instrument>)> {
    synth::PRESETS
        .iter()
        .map(|p| (*p, synth::preset(p).unwrap()))
        .collect()
}

/// The partial that carries a preset's nominal pitch, as a multiple of the key frequency.
fn nominal(name: &str) -> f64 {
    match name {
        // Strike tone = half of the 4th mode (Rayleigh): mode 4 sits at 2·f.
        "tubular-bells" => 2.0,
        // Minor-third bell tuned to the prime; the nominal is an octave above.
        "church-bell" => 2.0,
        _ => 1.0,
    }
}

fn keys(name: &str) -> [f32; 2] {
    if name.contains("bass") {
        [45.0, 57.0] // A2, A3
    } else {
        [57.0, 69.0] // A3, A4
    }
}

fn check_pitch(list: &[(&'static str, Arc<dyn Instrument>)], seeds: &[u64]) {
    let mut bad = Vec::new();
    for (name, inst) in list {
        for k in keys(name) {
            let c = measure_pitch_seeds(inst.as_ref(), k, nominal(name), seeds);
            println!("{name:<16} key {k}: {c:+.2} cents");
            if c.abs() > 3.0 {
                bad.push(format!("{name} key {k}: {c:+.2} cents"));
            }
        }
    }
    assert!(bad.is_empty(), "out of tune: {bad:?}");
}

#[test]
fn physical_pitch_within_3_cents() {
    // Single-component tones: one seed is representative.
    check_pitch(&physical_all(), &[7, 42]);
}

#[test]
fn synth_pitch_within_3_cents() {
    // Unison lanes, drift and vibrato average out over seeds (as the ear averages them).
    check_pitch(&synth_all(), &[7, 11, 23, 42, 77, 99]);
}

#[test]
fn string_partials_are_harmonic_or_stretched() {
    // Without dispersion the 2nd–5th partials are harmonic; with stiffness they are sharp.
    for (name, stretched) in [
        ("nylon-guitar", false),
        ("harp", false),
        ("pluck-bass", true),
    ] {
        let inst = physical::preset(name).unwrap();
        let key = if stretched { 40.0 } else { 57.0 };
        let r = render(inst.as_ref(), &note(key, 0.8, 1), 1.6, 1.5);
        let m = mono(&r);
        let seg = &m[2400..2400 + 60000];
        let f0 = 440.0 * ((key as f64 - 69.0) / 12.0).exp2();
        for h in 2..=5 {
            let (pk, _) = pitch_cents(seg, f0 * h as f64, 40.0);
            if stretched {
                assert!(pk > 0.0 && pk < 30.0, "{name} partial {h}: {pk:+.2} cents");
            } else {
                assert!(pk.abs() < 2.0, "{name} partial {h}: {pk:+.2} cents");
            }
        }
    }
}

fn all_instruments() -> Vec<(String, Arc<dyn Instrument>)> {
    let mut v: Vec<(String, Arc<dyn Instrument>)> = Vec::new();
    for (n, i) in physical_all() {
        v.push((n.to_string(), i));
    }
    for (n, i) in synth_all() {
        v.push((n.to_string(), i));
    }
    v
}

/// Seconds after note-off by which each kind of voice must have ended.
fn release_budget(name: &str) -> f32 {
    match name {
        // No dampers: they ring out naturally (−90 dB).
        "church-bell" | "glockenspiel" | "bell" | "handpan" | "kalimba" | "music-box"
        | "marimba" | "xylophone" | "steel-drum" | "wood-block" => 25.0,
        "harp" | "koto" | "tubular-bells" | "crystal" | "bell-fm" => 8.0,
        n if n.starts_with("pad-") => 6.0,
        _ => 4.0,
    }
}

#[test]
fn voices_terminate_after_release() {
    for (name, inst) in all_instruments() {
        let budget = release_budget(&name);
        let r = render(inst.as_ref(), &note(60.0, 0.9, 3), 1.0 + budget, 1.0);
        let end = r
            .ended
            .unwrap_or_else(|| panic!("{name} still sounding {budget} s after release"));
        println!(
            "{name:<16} ended {:.2} s after release",
            end as f32 / RATE as f32 - 1.0
        );
    }
}

#[test]
fn kill_fades_quickly() {
    for (name, inst) in all_instruments() {
        let mut v = inst.voice(&note(60.0, 0.9, 3));
        let (mut l, mut r) = ([0.0f32; BLOCK], [0.0f32; BLOCK]);
        for _ in 0..20 {
            v.render(&mut l, &mut r);
        }
        v.kill();
        let mut blocks = 0;
        while v.render(&mut l, &mut r) {
            blocks += 1;
            assert!(blocks < 4, "{name}: kill took more than ~10 ms");
        }
    }
}

#[test]
fn outputs_are_finite_at_extremes() {
    for (name, inst) in all_instruments() {
        for key in [21.0, 40.5, 96.0, 108.0] {
            for vel in [0.02, 1.0] {
                let r = render(inst.as_ref(), &note(key, vel, 9), 0.6, 0.3);
                assert!(
                    r.l.iter().chain(&r.r).all(|x| x.is_finite()),
                    "{name} key {key} vel {vel}: non-finite output"
                );
                assert!(
                    peak(&r.l).max(peak(&r.r)) < 1.5,
                    "{name} key {key} vel {vel}: runaway output"
                );
            }
        }
    }
    for k in synth::KITS {
        let inst = synth::kit(k).unwrap();
        for key in 0..128 {
            let r = render(inst.as_ref(), &note(key as f32, 1.0, 9), 0.3, 0.1);
            assert!(
                r.l.iter().chain(&r.r).all(|x| x.is_finite()),
                "kit {k} key {key}: non-finite"
            );
        }
    }
}

#[test]
fn levels_are_sensible() {
    // Forte (vel 0.8) peaks around −6…−3 dBFS; nothing clips at full velocity.
    let mut report = Vec::new();
    for (name, inst) in all_instruments() {
        let ks: [f32; 3] = if name.contains("bass") {
            [33.0, 45.0, 57.0]
        } else {
            [48.0, 60.0, 72.0]
        };
        let mut mx = -200.0f32;
        let mut full = -200.0f32;
        for k in ks {
            let r = render(inst.as_ref(), &note(k, 0.8, 5), 1.2, 1.0);
            mx = mx.max(dbfs(peak(&r.l).max(peak(&r.r))));
            let r = render(inst.as_ref(), &note(k, 1.0, 5), 1.2, 1.0);
            full = full.max(dbfs(peak(&r.l).max(peak(&r.r))));
        }
        println!("{name:<16} forte {mx:+.1} dBFS, vel 1 {full:+.1} dBFS");
        if !(-9.0..=-3.0).contains(&mx) || full > -0.5 {
            report.push(format!("{name}: forte {mx:.1}, full {full:.1}"));
        }
    }
    assert!(report.is_empty(), "{report:?}");
}

#[test]
fn drum_levels_are_sensible() {
    for k in synth::KITS {
        let inst = synth::kit(k).unwrap();
        for key in [
            35, 36, 37, 38, 39, 41, 42, 44, 46, 49, 51, 54, 56, 70, 75, 76,
        ] {
            let r = render(inst.as_ref(), &note(key as f32, 1.0, 5), 1.0, 0.1);
            let p = dbfs(peak(&r.l).max(peak(&r.r)));
            assert!(
                (-30.0..=-0.5).contains(&p),
                "kit {k} key {key}: {p:.1} dBFS"
            );
        }
    }
}

#[test]
fn rendering_is_deterministic_and_simd_paths_agree() {
    let mut all = all_instruments();
    for k in synth::KITS {
        all.push((format!("kit {k}"), synth::kit(k).unwrap()));
    }
    for (name, inst) in &all {
        let key = if name.starts_with("kit") { 38.0 } else { 62.3 };
        let a = render(inst.as_ref(), &note(key, 0.77, 1234), 0.5, 0.3);
        let b = render(inst.as_ref(), &note(key, 0.77, 1234), 0.5, 0.3);
        synth::force_baseline(true);
        let c = render(inst.as_ref(), &note(key, 0.77, 1234), 0.5, 0.3);
        synth::force_baseline(false);
        let bits = |r: &Render| {
            r.l.iter()
                .chain(&r.r)
                .map(|x| x.to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&a), bits(&b), "{name}: not deterministic");
        assert_eq!(bits(&a), bits(&c), "{name}: AVX and baseline paths differ");
        let d = render(inst.as_ref(), &note(key, 0.77, 1235), 0.5, 0.3);
        if !name.starts_with("kit")
            && !matches!(
                name.as_str(),
                "organ"
                    | "stab-house"
                    | "epiano-fm"
                    | "bell-fm"
                    | "bass-sub"
                    | "bass-808"
                    | "chip-square"
            )
        {
            assert_ne!(bits(&a), bits(&d), "{name}: seed has no effect");
        }
    }
}

/// Instantaneous frequency from zero crossings over `x`.
fn zc_freq(x: &[f32]) -> f64 {
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0;
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = (i - 1) as f64 + (-x[i - 1] / (x[i] - x[i - 1])) as f64;
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    count as f64 * RATE as f64 / (last - first.unwrap())
}

#[test]
fn mono_glide_slides_on_legato_only() {
    let inst = synth::preset("bass-sub").unwrap();
    assert!(inst.mono());
    let (mut l, mut r) = (vec![0.0f32; 24000], vec![0.0f32; 24000]);
    // Legato: the engine releases the held note and starts the next one.
    let mut a = inst.voice(&note(48.0, 0.8, 1));
    a.render(&mut l[..BLOCK], &mut r[..BLOCK]);
    a.release();
    let mut b = inst.voice(&note(60.0, 0.8, 2));
    l.fill(0.0);
    for c in 0..24000 / BLOCK {
        let s = c * BLOCK;
        b.render(&mut l[s..s + BLOCK], &mut r[s..s + BLOCK]);
    }
    let early = zc_freq(&l[0..480]);
    let late = zc_freq(&l[12000..24000]);
    let (fa, fb) = (130.81, 261.63);
    println!("legato glide: early {early:.1} Hz, late {late:.1} Hz");
    assert!(
        early < 0.8 * fb && early > 0.9 * fa,
        "glide should start near the previous note: {early}"
    );
    assert!(
        (late / fb - 1.0).abs() < 0.003,
        "glide should land on the new note: {late}"
    );

    // Detached: the previous note was released and has rendered since → no glide.
    let mut a = inst.voice(&note(48.0, 0.8, 1));
    a.render(&mut l[..BLOCK], &mut r[..BLOCK]);
    a.release();
    a.render(&mut l[..BLOCK], &mut r[..BLOCK]);
    let mut b = inst.voice(&note(60.0, 0.8, 2));
    l.fill(0.0);
    for c in 0..2400 / BLOCK {
        let s = c * BLOCK;
        b.render(&mut l[s..s + BLOCK], &mut r[s..s + BLOCK]);
    }
    let early = zc_freq(&l[0..960]);
    assert!(
        (early / fb - 1.0).abs() < 0.01,
        "detached note should not glide: {early}"
    );
}

#[test]
fn hats_choke_and_kits_cover_gm_keys() {
    for k in synth::KITS {
        let kit = synth::kit(k).unwrap();
        for key in [42.0, 44.0, 46.0] {
            assert_eq!(
                kit.voice(&note(key, 0.8, 1)).exclusive(),
                1,
                "kit {k} key {key}"
            );
        }
        assert_eq!(kit.voice(&note(36.0, 0.8, 1)).exclusive(), 0);
        // Every listed GM key makes sound.
        for key in [
            35, 36, 37, 38, 39, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 54, 56, 57, 70, 75, 76,
            77, 82,
        ] {
            let r = render(kit.as_ref(), &note(key as f32, 0.9, 1), 0.2, 0.1);
            assert!(peak(&r.l) > 0.01, "kit {k} key {key} is silent");
        }
        // Open hat is longer than closed hat.
        let c = render(kit.as_ref(), &note(42.0, 0.9, 1), 3.0, 0.1)
            .ended
            .unwrap();
        let o = render(kit.as_ref(), &note(46.0, 0.9, 1), 3.0, 0.1)
            .ended
            .unwrap_or(usize::MAX);
        assert!(o > 2 * c, "kit {k}: open hat {o} vs closed {c}");
    }
    assert!(synth::kit("nope").is_none());
    // 808 bass below the GM map follows the key.
    let kit = synth::kit("808").unwrap();
    for key in [28.0f32, 33.0] {
        let r = render(kit.as_ref(), &note(key, 0.9, 1), 1.0, 0.1);
        let m = mono(&r);
        let mut f = 440.0 * ((key as f64 - 69.0) / 12.0).exp2();
        while f < 27.0 {
            f *= 2.0;
        }
        let (pk, _) = pitch_cents(&m[14400..48000], f, 60.0);
        assert!(pk.abs() < 5.0, "808 bass key {key}: {pk:+.2} cents");
    }
}

#[test]
fn velocity_brightens_the_attack() {
    let skip = [
        "organ",
        "chip-square",
        "bass-sub",
        "bass-808",
        "lead-soft",
        "pad-choir",
        "bass-wobble",
        "bass-reese",
        "supersaw",
        "pad-dark",
    ];
    for (name, inst) in all_instruments() {
        if skip.contains(&name.as_str()) {
            continue;
        }
        // Share of the attack energy above 2 kHz.
        let soft = render(inst.as_ref(), &note(60.0, 0.3, 4), 0.25, 0.2);
        let hard = render(inst.as_ref(), &note(60.0, 1.0, 4), 0.25, 0.2);
        let (ms, mh) = (mono(&soft), mono(&hard));
        let ratio = |hz: f64| energy_above(&mh, hz) / energy_above(&ms, hz).max(1e-30);
        let (r2, r5) = (ratio(2000.0), ratio(5000.0));
        println!("{name:<16} brightness hard/soft: above 2 kHz ×{r2:.2}, above 5 kHz ×{r5:.2}");
        assert!(
            r2 > 1.25 || r5 > 1.25,
            "{name}: velocity should brighten the attack (×{r2:.2}, ×{r5:.2})"
        );
    }
}

fn saw_bank(f: f32, naive: bool) -> Vec<f32> {
    let n = RATE as usize * 2;
    let mut rng = Rng::new(1);
    let mut x = vec![0.0f32; n];
    if naive {
        let dt = f / RATE as f32;
        let mut t = 0.0f32;
        for v in x.iter_mut() {
            t += dt;
            if t >= 1.0 {
                t -= 1.0;
            }
            *v = 2.0 * t - 1.0;
        }
    } else {
        let mut b = Lanes::unison(Wave::Saw, 1, 0.0, 0.0, 1.0, 1.0, &mut rng);
        b.ph[0] = 0.0;
        b.set_freq(f, RATE as f32, &[1.0; 8]);
        for c in x.chunks_mut(BLOCK) {
            b.render_mono(c);
        }
    }
    x
}

#[test]
fn saw_aliasing_c7() {
    let f = 2093.0f32;
    let ideal: Vec<f32> = (0..RATE as usize * 2)
        .map(|i| {
            let mut s = 0.0f64;
            let mut h = 1;
            while (h as f64) * (f as f64) < RATE as f64 / 2.0 {
                s += (2.0 * std::f64::consts::PI * h as f64 * f as f64 * i as f64 / RATE as f64)
                    .sin()
                    / h as f64;
                h += 1;
            }
            (-s * 2.0 / std::f64::consts::PI) as f32
        })
        .collect();
    let full = |x: &[f32]| alias_db(x, f as f64, 0.002, 0.0, 24000.0);
    let hi = |x: &[f32]| alias_db(x, f as f64, 0.002, 16000.0, 24000.0);
    let (bl, nv, id) = (saw_bank(f, false), saw_bank(f, true), ideal);
    println!(
        "C7 saw alias energy (whole band): polyBLEP {:.1} dB, naive {:.1} dB, ideal {:.1} dB",
        full(&bl),
        full(&nv),
        full(&id)
    );
    println!(
        "C7 saw alias energy (16–24 kHz):  polyBLEP {:.1} dB, naive {:.1} dB, ideal {:.1} dB",
        hi(&bl),
        hi(&nv),
        hi(&id)
    );
    assert!(full(&bl) < full(&nv) - 50.0);
    assert!(full(&bl) < -65.0);
    assert!(hi(&bl) < -75.0);
    // Base rate (no oversampling): A4 and C7.
    for (f, max) in [(440.0f32, -58.0), (2093.0, -45.0)] {
        let n = RATE as usize * 2;
        let mut x = vec![0.0f32; n];
        let mut b = Lanes::unison(Wave::Saw, 1, 0.0, 0.0, 1.0, 1.0, &mut Rng::new(1));
        b.oversample(false);
        b.set_freq(f, RATE as f32, &[1.0; 8]);
        for c in x.chunks_mut(BLOCK) {
            b.render_mono(c);
        }
        let a = alias_db(&x, f as f64, 0.002, 0.0, 24000.0);
        println!("saw {f} Hz at the base rate: alias {a:.1} dB");
        assert!(a < max);
    }
    // The lead preset at C7 (filter, unison, drift included).
    let lead = synth::preset("lead-saw").unwrap();
    let r = render(lead.as_ref(), &note(96.0, 0.9, 2), 1.5, 1.4);
    let m = mono(&r);
    // Before the delayed vibrato starts (it would smear the harmonics).
    let a = alias_db(&m[2400..15840], f as f64 / 2.0, 0.012, 16000.0, 24000.0);
    println!("lead-saw C7 inharmonic energy 16–24 kHz: {a:.1} dB");
    assert!(a < -55.0);
}

#[test]
fn hard_plucks_start_sharp_and_settle() {
    // Tension modulation: a hard pluck starts a few cents sharp, a soft one does not.
    for name in ["koto", "banjo"] {
        let inst = physical::preset(name).unwrap();
        let f = 880.0;
        let measure = |vel: f32| {
            let r = render(inst.as_ref(), &note(81.0, vel, 3), 0.6, 0.5);
            let m = mono(&r);
            (
                pitch_cents(&m[240..2640], f, 60.0).0,
                pitch_cents(&m[9600..28800], f, 60.0).0,
            )
        };
        let (hard0, hard1) = measure(1.0);
        let (soft0, _) = measure(0.2);
        println!("{name}: hard {hard0:+.1} → {hard1:+.1} cents, soft {soft0:+.1}");
        assert!(hard0 > soft0 + 2.0 && hard1.abs() < 1.0, "{name}");
    }
}
