//! Render the reference demos and print listening-proxy metrics.
//!
//!   cargo run --release -p anikit-music --example demos -- [out_dir] [style…]
//!
//! Writes `<style>.wav` (and `.ogg` when ffmpeg is present) and a JSON line of
//! metrics per demo: loudness, true peak, spectral balance, clipping, and the
//! timing of kick onsets against the beat grid.

use anikit_music::{render, MusicSpec, RenderOpts};
use std::path::PathBuf;

const DEMOS: &[(&str, &str)] = &[
    ("lofi", r#"{"style":"lofi","bpm":84,"key":"Dm","length":"24s","seed":3}"#),
    ("house", r#"{"style":"house","bpm":124,"key":"Am","length":"24s","hits":["@7.74"],"seed":1}"#),
    ("cinematic", r#"{"style":"cinematic-hybrid","key":"Dm","length":"24s","hits":["@9.6"],"seed":2}"#),
    ("orchestral", r#"{"style":"orchestral","key":"D","length":"24s","seed":4}"#),
    ("synthwave", r#"{"style":"synthwave","key":"Am","length":"24s","seed":5}"#),
    ("trap", r#"{"style":"trap","key":"Cm","length":"20s","seed":6}"#),
    ("ambient", r#"{"style":"ambient","length":"24s","seed":7}"#),
    ("rock", r#"{"style":"rock","length":"20s","seed":8}"#),
    ("corporate", r#"{"style":"corporate","length":"20s","seed":9}"#),
    ("jazz", r#"{"style":"jazz","length":"20s","seed":10}"#),
    ("piano", r#"{"style":"piano","length":"24s","seed":11}"#),
    ("acoustic", r#"{"style":"acoustic","length":"20s","seed":12}"#),
    ("pop", r#"{"style":"pop","length":"20s","seed":13}"#),
    ("tech", r#"{"style":"tech","length":"20s","seed":14}"#),
    ("dnb", r#"{"style":"dnb","length":"20s","seed":15}"#),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = PathBuf::from(args.first().cloned().unwrap_or_else(|| "music-demos".into()));
    std::fs::create_dir_all(&out).unwrap();
    let only: Vec<&String> = args.iter().skip(1).collect();
    for (name, json) in DEMOS {
        if !only.is_empty() && !only.iter().any(|o| o.as_str() == *name) {
            continue;
        }
        let spec = MusicSpec::from_json(json).unwrap();
        let r = render(&spec, &RenderOpts { stems: true, ..Default::default() }).unwrap_or_else(|e| panic!("{name}: {e}"));
        let wav = out.join(format!("{name}.wav"));
        write_wav(&wav, r.rate, &r.l, &r.r);
        let ogg = out.join(format!("{name}.ogg"));
        let _ = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&wav)
            .args(["-c:a", "libvorbis", "-q:a", "4"])
            .arg(&ogg)
            .status();
        // Timing: kick onsets in the drum stem against the kick grid.
        let timing = r.stems.iter().find(|s| s.name == "drums").map(|d| anikit_music::mix::onset_error(&d.l, &d.r, r.rate, &r.arrangement.kicks));
        println!(
            "{}",
            serde_json::json!({
                "demo": name, "seconds": r.duration(), "bpm": r.arrangement.grid.bpm, "tracks": r.arrangement.tracks.iter().map(|t| format!("{}={}", t.name, t.inst)).collect::<Vec<_>>(),
                "report": r.report, "timings": r.timings, "kickTimingMs": timing,
            })
        );
    }
}

fn write_wav(path: &std::path::Path, rate: u32, l: &[f32], r: &[f32]) {
    let n = l.len();
    let mut b = Vec::with_capacity(44 + n * 4);
    let data = (n * 4) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    let mut rng = anikit_music::Rng::new(1);
    for (x, y) in l.iter().zip(r) {
        for v in [x, y] {
            // TPDF dither to 16 bits.
            let d = (rng.f32() - rng.f32()) / 32768.0;
            let s = ((v + d).clamp(-1.0, 1.0) * 32767.0).round() as i16;
            b.extend_from_slice(&s.to_le_bytes());
        }
    }
    std::fs::write(path, b).unwrap();
}
