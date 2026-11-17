use ak264::synth::Scene;
use ak264::*;
use std::time::Instant;
fn main() {
    let scene = std::env::args().nth(1).unwrap_or("typical".into());
    let s = Scene::preset(&scene, 1920, 1080);
    let bufs: Vec<YuvBuffer> = (0..120).map(|t| s.frame(t)).collect();
    for threads in [4usize, 8, 16] {
        let mut best = f64::MAX;
        let mut bytes = 0;
        for _ in 0..3 {
            let mut e = Encoder::new(EncoderConfig { width: 1920, height: 1080, qp: 26, gop: 60, threads, deblock: std::env::var("DEBLOCK").is_ok(), ..Default::default() });
            let t = Instant::now();
            bytes = 0;
            for b in &bufs {
                bytes += e.encode(&b.frame(), &FrameHints::default()).len();
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!("{scene} slices={threads}: {:.0} fps, {} KB", 120.0 / best, bytes / 1024);
    }
}
