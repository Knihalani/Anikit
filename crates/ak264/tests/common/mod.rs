#![allow(dead_code)]
use std::path::PathBuf;
use std::process::Command;

pub fn have_ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

pub fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join("ak264-tests");
    std::fs::create_dir_all(&d).unwrap();
    d.join(format!("{}-{}", std::process::id(), name))
}

/// Decode an Annex-B stream with ffmpeg; returns (stderr, raw yuv420p frames).
pub fn decode(stream: &[u8], name: &str) -> (String, Vec<u8>) {
    let p = tmp(&format!("{name}.h264"));
    std::fs::write(&p, stream).unwrap();
    let o = tmp(&format!("{name}.yuv"));
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "h264", "-i"])
        .arg(&p)
        .args(["-f", "rawvideo", "-pix_fmt", "yuv420p"])
        .arg(&o)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    let data = std::fs::read(&o).unwrap_or_default();
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(&o);
    (err, data)
}

pub fn psnr(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mse: f64 = a.iter().zip(b).map(|(&x, &y)| ((x as f64) - (y as f64)).powi(2)).sum::<f64>() / a.len() as f64;
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}
