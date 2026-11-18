use super::*;

const SR: u32 = 48_000;

#[test]
fn every_name_renders_finite_and_bounded() {
    for name in all_names() {
        let p = DesignParams {
            duration: Some(1.0),
            ..Default::default()
        };
        let s = render(name, &p, SR).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!s.l.is_empty(), "{name} is empty");
        let peak = s.l.iter().chain(&s.r).fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(
            s.l.iter().chain(&s.r).all(|x| x.is_finite()),
            "{name} not finite"
        );
        assert!(peak > 1e-4 && peak <= 1.0, "{name} peak {peak}");
        assert!(
            s.hit >= 0.0 && (s.hit as f64) <= s.l.len() as f64 / SR as f64 + 1e-3,
            "{name} hit {}",
            s.hit
        );
    }
}

#[test]
fn deterministic_and_seeded() {
    let p = DesignParams::default();
    let a = render("glass", &p, SR).unwrap();
    let b = render("glass", &p, SR).unwrap();
    assert_eq!(a, b);
    let c = render("glass", &DesignParams { seed: 9, ..p }, SR).unwrap();
    assert_ne!(a.l, c.l);
}

#[test]
fn layering_aligns_hits() {
    let p = DesignParams {
        duration: Some(0.8),
        ..Default::default()
    };
    let w = render("whoosh", &p, SR).unwrap();
    let l = render("whoosh+thump", &p, SR).unwrap();
    assert!((l.hit - w.hit).abs() < 1e-6);
    assert!(render("whoosh+nonsense", &p, SR).is_err());
    assert!(is_known("thump+kenney-interface/click"));
    assert!(!is_known("thmp"));
    assert!(unknown("thmp").contains("thump"));
}

#[test]
fn whoosh_follows_speed() {
    // Faster → shorter and brighter (more energy above 3 kHz).
    let slow = render(
        "swish",
        &DesignParams {
            speed: 0.1,
            ..Default::default()
        },
        SR,
    )
    .unwrap();
    let fast = render(
        "swish",
        &DesignParams {
            speed: 1.0,
            ..Default::default()
        },
        SR,
    )
    .unwrap();
    assert!(fast.l.len() < slow.l.len());
    let hf = |x: &[f32]| {
        let mut y = x.to_vec();
        synth::highpass(&mut y, 3000.0, SR as f32);
        let e: f32 = y.iter().map(|v| v * v).sum();
        let t: f32 = x.iter().map(|v| v * v).sum();
        e / t
    };
    assert!(
        hf(&fast.l) > hf(&slow.l),
        "{} vs {}",
        hf(&fast.l),
        hf(&slow.l)
    );
}

#[test]
fn motion_pans_across() {
    let s = render(
        "passby",
        &DesignParams {
            motion: 1.0,
            duration: Some(1.0),
            ..Default::default()
        },
        SR,
    )
    .unwrap();
    let n = s.l.len();
    let e = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>();
    // Left-heavy at the start, right-heavy at the end.
    assert!(e(&s.l[..n / 3]) > e(&s.r[..n / 3]));
    assert!(e(&s.r[2 * n / 3..]) > e(&s.l[2 * n / 3..]));
}

#[test]
fn tuned_effects_snap_to_key() {
    // A glass tuned to D: its strongest partial is a D (pitch class 2).
    let root = key_root_hz("Dm").unwrap();
    assert!((root - 587.33).abs() < 0.5, "{root}");
    assert!((key_root_hz("A").unwrap() - 440.0).abs() < 1e-3);
    assert!((key_root_hz("Bb").unwrap() - 466.16).abs() < 0.1);
}

#[test]
fn ambience_beds_are_long_quiet_and_decorrelated() {
    for name in AMBIENCES {
        let s = render(
            name,
            &DesignParams {
                duration: Some(4.0),
                ..Default::default()
            },
            SR,
        )
        .unwrap();
        assert_eq!(s.l.len(), 4 * SR as usize, "{name}");
        let rms = (s.l.iter().map(|x| (x * x) as f64).sum::<f64>() / s.l.len() as f64).sqrt();
        let db = 20.0 * rms.log10();
        assert!(db < -25.0 && db > -70.0, "{name}: {db:.1} dBFS");
        let corr: f64 =
            s.l.iter()
                .zip(&s.r)
                .map(|(a, b)| (*a as f64) * (*b as f64))
                .sum::<f64>()
                / s.l.iter().map(|a| (*a as f64).powi(2)).sum::<f64>();
        assert!(corr < 0.98, "{name} is mono ({corr:.3})");
    }
}

#[test]
fn bus_hits_loudness_and_true_peak() {
    let cues: Vec<Cue> = (0..8)
        .map(|i| Cue {
            duck_db: 3.0,
            ..Cue::new(
                ["impact", "whoosh", "glass", "thump"][i % 4],
                0.5 + i as f64 * 0.6,
            )
        })
        .collect();
    let opts = BusOpts {
        ambience: Some(("room".into(), 0.0, 1)),
        ..Default::default()
    };
    let (pcm, rep) = mix_design(&[], &cues, &opts, 6.0, SR, None).unwrap();
    assert_eq!(pcm.frames(), 6 * SR as usize);
    assert!((rep.integrated_lufs + 14.0).abs() < 0.6, "{rep:?}");
    assert!(rep.true_peak_dbtp <= -0.9, "{rep:?}");
    // Deterministic.
    let (pcm2, _) = mix_design(&[], &cues, &opts, 6.0, SR, None).unwrap();
    assert_eq!(pcm.samples, pcm2.samples);
}

#[test]
fn bus_ducks_music_under_hits() {
    // A steady "music" file track; one hit at 1 s ducks it.
    let dir = std::env::temp_dir().join(format!("ak-sound-duck-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("tone.wav");
    let tone: Vec<f32> = (0..SR as usize * 3)
        .map(|i| 0.3 * (std::f32::consts::TAU * 220.0 * i as f32 / SR as f32).sin())
        .collect();
    crate::audio::write_wav(
        &wav,
        &crate::audio::Pcm::from_mono(SR, &tone),
        crate::audio::WavFormat::Float32,
    )
    .unwrap();
    let track: crate::audio::Track =
        serde_json::from_value(serde_json::json!({ "src": wav.to_string_lossy() })).unwrap();
    let cue = Cue {
        duck_db: 6.0,
        ..Cue::new("tick", 1.0)
    };
    let opts = BusOpts {
        target_lufs: None,
        space: None,
        ..Default::default()
    };
    let (pcm, rep) = mix_design(&[track], &[cue], &opts, 3.0, SR, None).unwrap();
    assert!((rep.max_music_duck_db - 6.0).abs() < 1e-3);
    let rms = |a: f32, b: f32| {
        let s = &pcm.samples[(a * SR as f32) as usize * 2..(b * SR as f32) as usize * 2];
        (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
    };
    // Before the hit: full level; 60 ms after (past the tick): several dB down.
    let before = rms(0.5, 0.9);
    let after = rms(1.06, 1.1);
    assert!(20.0 * (after / before).log10() < -3.0, "{before} {after}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn render_speed_is_well_above_realtime() {
    // About what `auto` produces for a busy 15 s film: 15 cues and a bed.
    let cues: Vec<Cue> = (0..15)
        .map(|i| {
            Cue::new(
                ["swish", "thump", "glass", "ticker", "air", "impact"][i % 6],
                0.3 + i as f64,
            )
        })
        .collect();
    let opts = BusOpts {
        ambience: Some(("city".into(), 0.0, 1)),
        ..Default::default()
    };
    let t = std::time::Instant::now();
    mix_design(&[], &cues, &opts, 15.0, SR, None).unwrap();
    let secs = t.elapsed().as_secs_f64();
    // Measured alone (release, 4 cores): ≈55× real time. The suite runs tests
    // in parallel on the same cores, so this only guards against regressions.
    let floor = if cfg!(debug_assertions) { 1.0 } else { 15.0 };
    assert!(15.0 / secs > floor, "{:.1}× real time", 15.0 / secs);
}

fn evaluator(spec: serde_json::Value) -> crate::eval::Evaluator {
    crate::preview::Preview::new(spec, std::path::Path::new("."))
        .unwrap()
        .ev
}

/// A card slides in from the left and stops hard at 1.0 s; a label pops in at 2 s.
fn slide_spec(sound: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "size": [640, 360], "fps": 30, "sound": sound,
        "scenes": [{ "duration": 3, "layers": [
            { "id": "card", "shape": "rect", "width": 200, "height": 120, "fill": "#f80", "x": 420, "y": 180,
              "animate": [{ "from": { "x": -200 }, "at": 0.5, "duration": 0.5, "ease": "linear" }] },
            { "shape": "rect", "width": 80, "height": 80, "fill": "#08f", "x": 150, "y": 180,
              "animate": [{ "from": { "scale": 0.01 }, "at": 2.0, "duration": 0.2, "ease": "outBack" }] }
        ]}]
    })
}

#[test]
fn auto_cues_follow_the_picture() {
    let ev = evaluator(slide_spec(serde_json::json!({ "density": 1.0 })));
    let plan = ev.sound.clone().expect("plan");
    let cues = auto::plan_cues(&ev, &plan);
    let names: Vec<_> = cues
        .iter()
        .map(|c| (c.name.as_str(), c.at, c.why.as_str()))
        .collect();
    // The move peaks during the slide, travelling left → right.
    let mv = cues
        .iter()
        .find(|c| c.why.contains("moves"))
        .unwrap_or_else(|| panic!("{names:?}"));
    assert!(mv.at > 0.45 && mv.at < 1.05, "{names:?}");
    assert!(mv.params.motion > 0.3, "{:?}", mv.params);
    // The hard stop lands on the contact frame (±1 frame).
    let land = cues
        .iter()
        .find(|c| c.why.contains("lands"))
        .unwrap_or_else(|| panic!("{names:?}"));
    assert!((land.at - 1.0).abs() <= 1.0 / 30.0 + 1e-6, "{names:?}");
    // The pop-in.
    assert!(
        cues.iter()
            .any(|c| c.why.contains("pops") && c.at > 1.95 && c.at < 2.3),
        "{names:?}"
    );
    // Panned to where it happens: the card stops right of centre.
    assert!(land.params.pan > 0.0);
}

#[test]
fn layer_sfx_overrides_and_silences() {
    let mut spec = slide_spec(serde_json::json!({ "density": 1.0 }));
    spec["scenes"][0]["layers"][0]["sfx"] = serde_json::json!({ "land": "glass", "in": "passby" });
    spec["scenes"][0]["layers"][1]["sfx"] = serde_json::json!(false);
    let ev = evaluator(spec);
    let cues = auto::plan_cues(&ev, ev.sound.as_ref().unwrap());
    assert!(cues
        .iter()
        .any(|c| c.name == "glass" && c.why.contains("lands")));
    assert!(cues.iter().any(|c| c.name == "passby"));
    assert!(!cues.iter().any(|c| c.why.contains("layers[1]")));
}

#[test]
fn layer_sfx_alone_enables_sound() {
    let mut spec = slide_spec(serde_json::Value::Null);
    spec.as_object_mut().unwrap().remove("sound");
    spec["scenes"][0]["layers"][0]["sfx"] = serde_json::json!("auto");
    let ev = evaluator(spec);
    let plan = ev.sound.clone().expect("plan");
    assert!(!plan.auto);
    let cues = auto::plan_cues(&ev, &plan);
    assert!(
        !cues.is_empty() && cues.iter().all(|c| c.why.contains("layers[0]")),
        "{cues:?}"
    );
}

#[test]
fn bad_sound_keys_are_diagnosed() {
    for (v, want) in [
        (serde_json::json!({ "palette": "organc" }), "palette"),
        (
            serde_json::json!({ "ambience": "raim" }),
            "did you mean 'rain'",
        ),
        (
            serde_json::json!({ "cues": [{ "sfx": "glas", "at": 1 }] }),
            "did you mean 'glass'",
        ),
        (serde_json::json!({ "space": "hal" }), "space"),
    ] {
        let e = spec::SoundPlan::new(Some(&v), None).unwrap_err();
        assert!(e.contains(want), "{v}: {e}");
    }
    assert!(
        spec::LayerSfx::parse(&serde_json::json!({ "land": "thmp" }))
            .unwrap_err()
            .contains("thump")
    );
    let p = spec::SoundPlan::new(
        Some(&serde_json::json!("auto")),
        Some(&serde_json::json!({ "style": "synthwave", "key": "F#m" })),
    )
    .unwrap();
    assert_eq!(p.palette, "digital");
    assert!((p.tune.unwrap() - 739.99).abs() < 0.1);
    assert_eq!(p.target_lufs, Some(-14.0));
}

#[test]
fn explicit_cues_resolve_markers_and_render() {
    let ev = evaluator(slide_spec(
        serde_json::json!({ "design": "off", "cues": [{ "sfx": "impact+glass", "at": 1.5, "x": 0.9, "duck": 4 }] }),
    ));
    let plan = ev.sound.clone().unwrap();
    let cues = auto::plan_cues(&ev, &plan);
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].at, 1.5);
    assert!(cues[0].params.pan > 0.4);
    let dir = std::env::temp_dir().join(format!("ak-sound-mix-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("a.wav");
    crate::pipeline::mix_audio(&ev, 0.0, ev.duration, &wav)
        .unwrap()
        .expect("audio");
    let pcm = crate::audio::decode_file_at(&wav, SR).unwrap();
    // Silent until the hit, loud right after it, right channel louder (x = 0.9).
    let e = |a: f64, b: f64, ch: usize| {
        pcm.samples[(a * 48e3) as usize * 2..(b * 48e3) as usize * 2]
            .iter()
            .skip(ch)
            .step_by(2)
            .map(|x| x * x)
            .sum::<f32>()
    };
    assert!(e(0.0, 1.4, 0) < 1e-6, "{}", e(0.0, 1.4, 0));
    assert!(e(1.5, 1.7, 1) > e(1.5, 1.7, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore]
fn time_each_sound() {
    for name in all_names() {
        let t = std::time::Instant::now();
        let s = render(
            name,
            &DesignParams {
                duration: Some(1.0),
                size: 0.0,
                ..Default::default()
            },
            SR,
        )
        .unwrap();
        eprintln!(
            "{name:<16} {:6.1} ms  {:.2} s",
            t.elapsed().as_secs_f64() * 1e3,
            s.l.len() as f32 / SR as f32
        );
    }
}

#[test]
fn audio_cue_design_names_land_their_hit() {
    let spec = serde_json::json!({
        "size": [320, 180], "audio": [{ "sfx": "whoosh-air", "at": 1.5, "duration": 0.8 }],
        "scenes": [{ "duration": 3, "layers": [{ "shape": "rect", "width": 10, "height": 10, "fill": "#fff" }] }]
    });
    let ev = evaluator(spec);
    let plan = ev.sound.clone().expect("plan from audio[].sfx");
    assert_eq!(plan.target_lufs, None);
    let cues = auto::plan_cues(&ev, &plan);
    assert_eq!(cues.len(), 1);
    let dir = std::env::temp_dir().join(format!("ak-sound-cue-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("a.wav");
    crate::pipeline::mix_audio(&ev, 0.0, ev.duration, &wav).unwrap().expect("audio");
    let pcm = crate::audio::decode_file_at(&wav, SR).unwrap();
    // The loudest 20 ms window is within ~60 ms of 1.5 s (the whoosh peaks on its cue).
    let w = (0.02 * SR as f64) as usize;
    let frames = pcm.frames();
    let (best, _) = (0..frames / w).map(|i| (i, pcm.samples[i * w * 2..(i + 1) * w * 2].iter().map(|x| x * x).sum::<f32>())).fold((0, 0.0), |m, x| if x.1 > m.1 { x } else { m });
    let t = (best * w) as f64 / SR as f64;
    assert!((t - 1.5).abs() < 0.08, "peak at {t:.3}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn physics_collisions_are_heard() {
    // A metal ball dropped from 2 m lands at ≈ √(2·2/9.81) ≈ 0.64 s after the sim starts.
    let spec = serde_json::json!({
        "size": [320, 180], "fps": 30, "sound": { "density": 0.6 },
        "scenes": [{ "duration": 2, "layers": [{ "component": "scene3d", "props": {
            "env": "cyc", "quality": "draft",
            "camera": { "keys": [{ "t": 0, "pos": [0, 1.5, 6], "target": [0, 0.5, 0], "focal": 35 }] },
            "physics": { "bodies": [{ "type": "sphere", "radius": 0.2, "pos": [1.2, 2.2, 0], "material": "chrome" }] }
        } }] }]
    });
    let ev = evaluator(spec);
    let cues = auto::plan_cues(&ev, ev.sound.as_ref().unwrap());
    let hits: Vec<_> = cues.iter().filter(|c| c.why.contains("collides")).collect();
    assert!(!hits.is_empty(), "{cues:?}");
    let first = hits[0];
    assert_eq!(first.name, "metal", "{hits:?}");
    assert!(first.at > 0.45 && first.at < 0.85, "{hits:?}");
    // Right of centre on screen → panned right.
    assert!(first.params.pan > 0.05, "{hits:?}");
}

#[test]
fn sound_design_steps_back_under_voice() {
    let dir = std::env::temp_dir().join(format!("ak-sound-vo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("vo.wav");
    // "Voice" from 1.0 to 2.0 s.
    let vo: Vec<f32> = (0..SR as usize * 3).map(|i| if (SR as usize..2 * SR as usize).contains(&i) { 0.2 * (std::f32::consts::TAU * 180.0 * i as f32 / SR as f32).sin() } else { 0.0 }).collect();
    crate::audio::write_wav(&wav, &crate::audio::Pcm::from_mono(SR, &vo), crate::audio::WavFormat::Float32).unwrap();
    let track: crate::audio::Track = serde_json::from_value(serde_json::json!({ "src": wav.to_string_lossy(), "voice": true })).unwrap();
    let opts = BusOpts { target_lufs: None, space: None, ..Default::default() };
    // The same tick, once in the clear and once under the voice; compare its level.
    let level = |at: f64, with_voice: bool| {
        let tracks = if with_voice { vec![track.clone()] } else { vec![] };
        let (pcm, _) = mix_design(&tracks, &[Cue::new("blip", at)], &opts, 3.0, SR, None).unwrap();
        let a = (at * SR as f64) as usize * 2;
        let s = &pcm.samples[a..a + 2 * 2400];
        let mut e: f32 = s.iter().map(|x| x * x).sum();
        if with_voice {
            // Remove the voice's own energy.
            let v: f32 = vo[a / 2..a / 2 + 2400].iter().map(|x| 2.0 * x * x).sum();
            e -= v;
        }
        e
    };
    let clear = level(1.5, false);
    let under = level(1.5, true);
    assert!(10.0 * (under / clear).log10() < -4.0, "{clear} {under}");
    let _ = std::fs::remove_dir_all(&dir);
}
