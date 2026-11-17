use super::*;

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success())
}

/// A textured, moving frame in one colour family (so shots differ by colour, frames by motion).
fn frame(w: usize, h: usize, base: [f32; 3], t: usize, shift: i32) -> image::RgbImage {
    image::RgbImage::from_fn(w as u32, h as u32, |x, y| {
        // Random 8 px cells (not periodic, so block matching is unambiguous).
        let xx = x as i32 + shift * t as i32;
        let (cx, cy) = (xx.div_euclid(8) as u32, y / 8);
        let hsh = (cx.wrapping_mul(73_856_093) ^ cy.wrapping_mul(19_349_663)).wrapping_mul(2_654_435_761) >> 24;
        let v = 0.45 + 0.5 * (hsh as f32 / 255.0);
        image::Rgb([(base[0] * v * 255.0) as u8, (base[1] * v * 255.0) as u8, (base[2] * v * 255.0) as u8])
    })
}

#[test]
fn finds_known_cuts_and_a_crossfade() {
    if !have_ffmpeg() {
        eprintln!("skipped: no ffmpeg");
        return;
    }
    let dir = std::env::temp_dir().join(format!("anikit-analyze-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (w, h, fps) = (320usize, 180usize, 30usize);
    // Shots: red 0–1 s, green 1–2.2 s, blue 2.2–3.5 s (crossfade 3.2–3.8 into) yellow to 5 s.
    let cols = [[1.0, 0.15, 0.1], [0.1, 0.9, 0.2], [0.15, 0.25, 1.0], [1.0, 0.9, 0.1]];
    let total = 5 * fps;
    for i in 0..total {
        let t = i as f64 / fps as f64;
        let img = if t < 1.0 {
            frame(w, h, cols[0], i, 2)
        } else if t < 2.2 {
            frame(w, h, cols[1], i, -2)
        } else if t < 3.2 {
            frame(w, h, cols[2], i, 1)
        } else if t < 3.8 {
            let k = ((t - 3.2) / 0.6) as f32;
            let (a, b) = (frame(w, h, cols[2], i, 1), frame(w, h, cols[3], i, 1));
            image::RgbImage::from_fn(w as u32, h as u32, |x, y| {
                let (p, q) = (a.get_pixel(x, y).0, b.get_pixel(x, y).0);
                image::Rgb([0, 1, 2].map(|c| (p[c] as f32 * (1.0 - k) + q[c] as f32 * k) as u8))
            })
        } else {
            frame(w, h, cols[3], i, 1)
        };
        img.save(dir.join(format!("f{i:04}.png"))).unwrap();
    }
    let a = analyze(&dir, &Options { sequence_fps: fps as f64, ..Default::default() }).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let at: Vec<f64> = a.transitions.iter().map(|t| t.at).collect();
    assert_eq!(a.shots.len(), 4, "transitions {:?}", a.transitions);
    assert!((at[0] - 1.0).abs() < 0.05 && (at[1] - 2.2).abs() < 0.05, "{at:?}");
    assert_eq!(a.transitions[0].kind, "cut");
    assert_eq!(a.transitions[2].kind, "crossfade", "{:?}", a.transitions[2]);
    assert!((at[2] - 3.5).abs() < 0.15, "{at:?}");
    // The red shot's palette is red; the camera reads the texture's motion as a pan
    // (texture drifting left at 2 px/frame of 320 = −0.19 frame widths over 1 s).
    let red = hex_to_rgb(&a.shots[0].palette[0].hex);
    assert!(red[0] > 120 && red[0] > 3 * red[1].max(red[2]), "{:?}", a.shots[0].palette);
    assert!((a.shots[0].camera.pan_x + 0.18).abs() < 0.05, "{:?}", a.shots[0].camera);
    assert!((a.shots[1].camera.pan_x - 0.22).abs() < 0.06, "{:?}", a.shots[1].camera);
    assert_eq!(a.shots[0].camera.label, "pan right", "{:?}", a.shots[0].camera);
    // The draft spec has one scene per shot, and it compiles.
    let spec = draft_spec(&a);
    assert_eq!(spec["scenes"].as_array().unwrap().len(), 4);
    let p = crate::spec::parse::from_value(spec.clone()).unwrap_or_else(|e| panic!("{e:?}\n{spec:#}"));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    let (sw, sh, px) = contact_sheet(&a);
    assert_eq!(px.len(), (sw * sh * 3) as usize);
}

#[test]
fn tempo_of_generated_music_and_cuts_on_beat() {
    let spec = anikit_music::MusicSpec::from_json(r#"{"style":"house","bpm":124,"length":"12s","seed":3}"#).unwrap();
    let r = anikit_music::render(&spec, &Default::default()).unwrap();
    let mut samples = Vec::with_capacity(r.l.len() * 2);
    for (l, rr) in r.l.iter().zip(&r.r) {
        samples.push(*l);
        samples.push(*rr);
    }
    let pcm = crate::audio::Pcm { rate: r.rate, channels: 2, samples };
    let beat = 60.0 / 124.0;
    let first = crate::eval::beat_markers::refine(&crate::audio::analyze_pcm(&pcm)).beats[2];
    let cuts: Vec<Transition> = (0..4).map(|k| Transition { at: first + beat * 4.0 * k as f64, duration: 0.0, kind: "cut".into(), direction: None, strength: 1.0 }).collect();
    let a = audio_info(&pcm, &cuts, 30.0);
    assert!((a.bpm - 124.0).abs() < 1.5, "bpm {}", a.bpm);
    assert!(a.cuts_on_beat > 0.99, "{}", a.cuts_on_beat);
    assert!(!a.loudness.is_empty() && !a.onsets.is_empty());
}

#[test]
fn oklab_round_trip_and_palette() {
    for c in [[255u8, 0, 0], [12, 200, 90], [240, 240, 240], [0, 0, 0]] {
        assert_eq!(image_ops::oklab_to_srgb(srgb_to_oklab(c)), c);
    }
    let s: Vec<[f32; 3]> = (0..300).map(|i| srgb_to_oklab(if i < 200 { [10, 10, 10] } else { [250, 40, 40] })).collect();
    let p = kmeans_palette(&s, 3);
    assert_eq!(p[0].hex, "#0A0A0A");
    assert!((p[0].share - 0.67).abs() < 0.01);
}
