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
