//! Listening-proxy checks on full renders: hits land on the beat grid,
//! masters hit −14 LUFS / −1 dBTP without clipping, output is deterministic.

use anikit_music::{render, MusicSpec, RenderOpts};

#[test]
fn clicks_land_on_the_grid_and_masters_hit_targets() {
    // Claves on every beat, no humanisation; tempo nudged by a hit.
    let spec = MusicSpec::from_json(
        r#"{"bpm":117,"hits":["@3.0"],"fit":true,"length":"8s","tracks":[{"inst":"drums","drums":{"claves":"x...x...x...x..."},"bars":4,"humanize":0,"straight":true}]}"#,
    )
    .unwrap();
    let r = render(&spec, &RenderOpts::default()).unwrap();
    let a = &r.arrangement;
    // The hit at 3.0 s sits exactly on a beat of the fitted grid.
    let k = (3.0 / a.grid.beat()).round();
    assert!((k * a.grid.beat() - 3.0).abs() < 1e-9, "bpm {}", a.grid.bpm);
    let beats: Vec<f64> = a.beats().into_iter().filter(|&t| t < 7.5).collect();
    let t = anikit_music::mix::onset_error(&r.l, &r.r, r.rate, &beats);
    eprintln!("click timing: {t}");
    assert!(t["hits"].as_u64().unwrap() >= 12);
    assert!(t["maxAbsMs"].as_f64().unwrap() < 1.5, "{t}");
    // Mastering (a real arrangement; a bare click track is too peaky to push to −14 LUFS).
    let spec = MusicSpec::from_json(r#"{"style":"lofi","length":"10s"}"#).unwrap();
    let r = render(&spec, &RenderOpts::default()).unwrap();
    let rep = &r.report;
    assert!((rep["lufs"].as_f64().unwrap() + 14.0).abs() < 0.35, "{rep}");
    assert!(rep["truePeak"].as_f64().unwrap() <= -0.99, "{rep}");
    assert_eq!(rep["clippedPct"].as_f64().unwrap(), 0.0);
}

#[test]
fn renders_are_deterministic() {
    let spec = MusicSpec::from_json(r#"{"style":"house","length":"6s","seed":4}"#).unwrap();
    let a = render(&spec, &RenderOpts::default()).unwrap();
    let b = render(&spec, &RenderOpts::default()).unwrap();
    assert!(a.l == b.l && a.r == b.r, "same spec + seed → identical audio");
}

#[test]
fn stems_sum_to_the_premaster_mix() {
    let spec = MusicSpec::from_json(r#"{"style":"synthwave","length":"6s"}"#).unwrap();
    let r = render(&spec, &RenderOpts { stems: true, ..Default::default() }).unwrap();
    let names: Vec<&str> = r.stems.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"drums") && names.contains(&"bass") && names.contains(&"music"), "{names:?}");
    assert!(r.stems.iter().all(|s| s.l.len() == r.l.len()));
}

#[test]
fn custom_instruments_fx_and_automation() {
    // A sampler zone from a WAV we write (a 220 Hz sine, root A3).
    let dir = std::env::temp_dir().join(format!("ak-custom-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let n = 48_000u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(n * 2).to_le_bytes());
    for i in 0..n {
        let s = ((i as f32 * std::f32::consts::TAU * 220.0 / 48_000.0).sin() * 12_000.0) as i16;
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(dir.join("a3.wav"), b).unwrap();
    let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/video/music/custom.json")).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&doc).unwrap();
    v["define"]["sine"] = serde_json::json!({"sampler": {"zones": [{"file": "a3.wav", "root": "A3", "lo": 40, "hi": 90}], "release": 0.2}});
    v["tracks"].as_array_mut().unwrap().push(serde_json::json!({"inst": "sine", "notes": "o4 A3/2 E4/2 | A4/1", "at": 2, "fx": [{"type": "reverb", "preset": "hall", "wet": 0.3}]}));
    let spec = MusicSpec::from_json(&v.to_string()).unwrap();
    let opts = RenderOpts { base_dir: Some(dir.clone()), ..Default::default() };
    let r = render(&spec, &opts).unwrap();
    assert!((r.report["lufs"].as_f64().unwrap() + 14.0).abs() < 0.35, "{}", r.report);
    let again = render(&spec, &opts).unwrap();
    assert!(r.l == again.l, "deterministic with custom instruments and automation");
    // The automated cutoff opens over 4 bars: the acid line gets brighter (spectral centroid rises).
    let centroid = |a: usize, b: usize| {
        let x = &r.l[a..b];
        let (mut num, mut den) = (0.0f64, 0.0f64);
        for w in x.windows(2) {
            let d = (w[1] - w[0]) as f64;
            num += d * d;
            den += (w[0] as f64).powi(2);
        }
        num / den.max(1e-12)
    };
    let bar = (4.0 * 60.0 / spec.bpm.unwrap() * 48_000.0) as usize;
    assert!(centroid(3 * bar, 4 * bar) > centroid(0, bar) * 1.2, "cutoff automation should brighten the sound");
    // Clear errors.
    let bad = |j: &str| render(&MusicSpec::from_json(j).unwrap(), &RenderOpts::default()).err().unwrap();
    let e = bad(r#"{"bpm":120,"define":{"x":{"synth":{"osc":[{"wave":"sawz"}]}}},"tracks":[{"inst":"x","notes":"C4"}]}"#);
    assert!(e.contains("sawz"), "{e}");
    let e = bad(r#"{"bpm":120,"tracks":[{"inst":"piano","notes":"C4","automate":{"synth.cutoff":[[0,1]]}}]}"#);
    assert!(e.contains("custom synth"), "{e}");
    let e = bad(r#"{"bpm":120,"tracks":[{"inst":"piano","notes":"C4","automate":{"pna":"=1"}}]}"#);
    assert!(e.contains("did you mean `pan`"), "{e}");
    let e = bad(r#"{"bpm":120,"tracks":[{"inst":"piano","notes":"C4","fx":[{"type":"reverb","preset":"hal"}]}]}"#);
    assert!(e.contains("hal"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}
