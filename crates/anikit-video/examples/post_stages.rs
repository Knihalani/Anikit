//! Cost of each post stage on a 1080p frame: the `cinematic` look, then with
//! one stage switched off at a time (the difference is that stage's cost).
//!   cargo run --release -p anikit-video --example post_stages -- [threads] [preset]
use anikit_video::render::post::{PostProcessor, PostSettings, Tonemap};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let threads: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let preset = a.get(2).cloned().unwrap_or_else(|| "cinematic".into());
    rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().unwrap();
    let (w, h) = (1920usize, 1080usize);
    // A test card: gradients, bright blobs (bloom), edges.
    let mut src = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let blob = ((x as i32 - 900).pow(2) + (y as i32 - 500).pow(2) < 160 * 160) as u8;
            src[i] = (40.0 + 180.0 * u) as u8 | (blob * 255);
            src[i + 1] = (30.0 + 120.0 * v) as u8 | (blob * 240);
            src[i + 2] = if (x / 64 + y / 64) % 2 == 0 { 200 } else { 60 };
            src[i + 3] = 255;
        }
    }
    let base = PostSettings::preset(&preset).expect("preset");
    let mut variants: Vec<(&str, PostSettings)> = vec![("all", base.clone())];
    variants.push(("- lens", PostSettings { distortion: 0.0, aberration: 0.0, ..base.clone() }));
    variants.push(("- bloom+halation", PostSettings { bloom: 0.0, halation: 0.0, ..base.clone() }));
    variants.push(("- grain", PostSettings { grain: 0.0, ..base.clone() }));
    variants.push(("- vignette", PostSettings { vignette: 0.0, ..base.clone() }));
    variants.push(("- tonemap", PostSettings { tonemap: Tonemap::None, ..base.clone() }));
    variants.push(("- saturation", PostSettings { saturation: 1.0, ..base.clone() }));
    variants.push(("decode+final only", PostSettings { distortion: 0.0, aberration: 0.0, bloom: 0.0, halation: 0.0, ..base.clone() }));
    let mut pp = PostProcessor::default();
    let mut buf = src.clone();
    for (name, p) in &variants {
        let n = 12;
        for f in 0..3 {
            buf.copy_from_slice(&src);
            pp.apply(&mut buf, w, h, p, f);
        }
        let mut tot = 0.0;
        for f in 0..n {
            buf.copy_from_slice(&src);
            let t = std::time::Instant::now();
            pp.apply(&mut buf, w, h, p, f as u64);
            tot += t.elapsed().as_secs_f64();
        }
        println!("{name:20} {:7.2} ms", tot * 1e3 / n as f64);
    }
}
