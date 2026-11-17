//! Render every SFX to WAV and print QA metrics.
//!
//! `cargo run --release -p anikit-music --example sfx_render -- <out-dir> [seed]`

use anikit_music::dsp::analyze;
use anikit_music::sfx::{sfx, SfxParams, SFX};

/// Write 32-bit float stereo WAV.
fn write_wav(path: &std::path::Path, l: &[f32], r: &[f32], rate: u32) -> std::io::Result<()> {
    let mut b = Vec::with_capacity(44 + l.len() * 8);
    let data = (l.len() * 8) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 8).to_le_bytes());
    b.extend_from_slice(&8u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for (x, y) in l.iter().zip(r) {
        b.extend_from_slice(&x.to_le_bytes());
        b.extend_from_slice(&y.to_le_bytes());
    }
    std::fs::write(path, b)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| "sfx-out".into()));
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    std::fs::create_dir_all(&dir).expect("create output dir");
    println!(
        "{:<15} {:>5} {:>5} {:>6} {:>6} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>5} {:>6}",
        "name", "len", "hit", "LUFS", "dBTP", "crest", "sub", "low", "lmid", "hmid", "air", "cent", "corr", "ms"
    );
    for name in SFX {
        let t = std::time::Instant::now();
        let s = sfx(name, &SfxParams { seed, ..Default::default() }, 48_000).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let a = analyze(&s.l, &s.r, 48_000.0);
        println!(
            "{:<15} {:>5.2} {:>5.2} {:>6.1} {:>6.2} {:>5.1} {:>6.1} {:>6.1} {:>6.1} {:>6.1} {:>6.1} {:>6.0} {:>5.2} {:>6.1}",
            name,
            s.duration(48_000),
            s.hit,
            a.integrated_lufs.max(-99.0),
            a.true_peak_dbtp,
            a.crest_db,
            a.bands.sub,
            a.bands.low,
            a.bands.low_mid,
            a.bands.high_mid,
            a.bands.air,
            a.centroid_hz,
            a.correlation,
            ms
        );
        write_wav(&dir.join(format!("{name}.wav")), &s.l, &s.r, 48_000).expect("write wav");
    }
}
