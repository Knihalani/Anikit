//! Size/PSNR for option variants: cargo run --release --example quality -- scene frames qp
use ak264::synth::Scene;
use ak264::*;
use std::process::Command;
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scene = a.get(1).cloned().unwrap_or("typical".into());
    let n: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(60);
    let qp: u8 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(24);
    let (w, h) = (1920, 1080);
    let s = Scene::preset(&scene, w, h);
    let bufs: Vec<YuvBuffer> = (0..n).map(|t| s.frame(t)).collect();
    let src: Vec<u8> = bufs.iter().flat_map(|b| b.to_bytes()).collect();
    let frames: Vec<YuvFrame> = bufs.iter().map(|b| b.frame()).collect();
    let dir = std::env::temp_dir();
    for (name, deblock, cabac, subpel) in [
        ("default (CABAC)", false, true, false),
        ("+ deblock", true, true, false),
        ("+ subpel", false, true, true),
        ("CAVLC", false, false, false),
    ] {
        let cfg = EncoderConfig { width: w, height: h, qp, gop: 1000, deblock, cabac, subpel, ..Default::default() };
        let t = Instant::now();
        let mut e = Encoder::new(cfg);
        let mut out = Vec::new();
        for f in &frames {
            out.extend_from_slice(e.encode(f, &FrameHints::default()));
        }
        let dt = t.elapsed().as_secs_f64();
        let p = dir.join("ak264-q.h264");
        std::fs::write(&p, &out).unwrap();
        let o = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&p)
            .args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
            .output()
            .unwrap();
        let mse: f64 = o.stdout.iter().zip(&src).map(|(&x, &y)| (x as f64 - y as f64).powi(2)).sum::<f64>() / src.len() as f64;
        println!("{name:28} {:6.0} fps {:8} B  PSNR {:.3}", n as f64 / dt, out.len(), 10.0 * (65025.0 / mse).log10());
    }
}
