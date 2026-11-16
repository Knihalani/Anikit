//! GIF encoder profile target: a moving full-frame gradient with a glow
//! (thousands of colours per frame, every pixel changing), the worst case of
//! `record()`. `cargo run --release -p anikit-core --example gif_prof`
//! prints ms per frame; run it under `valgrind --tool=callgrind` for a
//! per-function breakdown.
use anikit_core::gif::GifEncoder;
use std::time::Instant;

fn main() {
    let (w, h, n) = (480usize, 270usize, 30usize);
    let frames: Vec<Vec<u8>> = (0..n)
        .map(|f| {
            let t = f as f32 / n as f32;
            let mut px = vec![0u8; w * h * 4];
            for y in 0..h {
                for x in 0..w {
                    let p = (y * w + x) * 4;
                    let (dx, dy) = (x as f32 - w as f32 * (0.3 + 0.4 * t), y as f32 - h as f32 / 2.0);
                    let glow = 120.0 * (-(dx * dx + dy * dy) / 4000.0).exp();
                    px[p] = (x as f32 / w as f32 * 200.0 + glow).min(255.0) as u8;
                    px[p + 1] = (y as f32 / h as f32 * 160.0 + 40.0 * t + glow * 0.6).min(255.0) as u8;
                    px[p + 2] = (255.0 - x as f32 / w as f32 * 120.0 + glow * 0.2).min(255.0) as u8;
                    px[p + 3] = 255;
                }
            }
            px
        })
        .collect();
    let start = Instant::now();
    let mut e = GifEncoder::new(w as u16, h as u16, 0);
    for f in &frames {
        e.add_frame(f, 3);
    }
    let len = e.finish().len();
    println!("{:.3} ms/frame, {} KB", start.elapsed().as_secs_f64() * 1000.0 / n as f64, len / 1024);
}
