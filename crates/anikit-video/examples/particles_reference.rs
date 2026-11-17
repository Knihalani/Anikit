//! Particle renderer fidelity: the fast path (pyramid splats, adaptive
//! streak samples) against the reference (exact box-filtered discs of the
//! full size + defocus footprint, 32 instants over the shutter), on whole
//! 1080p scene3d frames (the particle layer goes through the same post).
//!
//! `cargo run --release -p anikit-video --example particles_reference [-- spec.json t …]`
//!
//! Reports PSNR (luma), SSIM (8×8 luma windows) and CIE76 ΔE (p99.9, max)
//! with the thresholds PERF's qbench uses: SSIM ≥ 0.995, PSNR-Y ≥ 42 dB,
//! ΔE p99.9 ≤ 2.3.

use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline;

fn luma(p: &[u8]) -> f64 {
    0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64
}

fn lab(p: &[u8]) -> [f64; 3] {
    let l = |v: u8| {
        let c = v as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (l(p[0]), l(p[1]), l(p[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    [116.0 * f(y) - 16.0, 500.0 * (f(x) - f(y)), 200.0 * (f(y) - f(z))]
}

fn compare(a: &[u8], b: &[u8], w: usize, h: usize) -> (f64, f64, f64, f64) {
    let mut se = 0.0;
    let mut de: Vec<f64> = Vec::with_capacity(w * h);
    for i in 0..w * h {
        let (p, q) = (&a[i * 4..i * 4 + 3], &b[i * 4..i * 4 + 3]);
        let d = luma(p) - luma(q);
        se += d * d;
        let (x, y) = (lab(p), lab(q));
        de.push(((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt());
    }
    let psnr = 10.0 * (255.0f64 * 255.0 / (se / (w * h) as f64).max(1e-12)).log10();
    let (mut ssim, mut n) = (0.0, 0);
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    for by in (0..h - 8).step_by(4) {
        for bx in (0..w - 8).step_by(4) {
            let (mut ma, mut mb, mut va, mut vb, mut cv) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for y in by..by + 8 {
                for x in bx..bx + 8 {
                    let (p, q) = (luma(&a[(y * w + x) * 4..]), luma(&b[(y * w + x) * 4..]));
                    ma += p;
                    mb += q;
                    va += p * p;
                    vb += q * q;
                    cv += p * q;
                }
            }
            let k = 64.0;
            let (ma, mb) = (ma / k, mb / k);
            let (va, vb, cv) = (va / k - ma * ma, vb / k - mb * mb, cv / k - ma * mb);
            ssim += ((2.0 * ma * mb + c1) * (2.0 * cv + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
        }
    }
    de.sort_by(|x, y| x.total_cmp(y));
    (psnr, ssim / n as f64, de[(de.len() as f64 * 0.999) as usize], *de.last().unwrap())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pairs: Vec<(String, f32)> = if args.len() >= 2 {
        args.chunks(2).map(|c| (c[0].clone(), c[1].parse().unwrap())).collect()
    } else {
        vec![("examples/video/sim/particles.json".into(), 1.0), ("examples/video/sim/particles.json".into(), 4.5), ("examples/video/sim/glow.json".into(), 2.0)]
    };
    std::env::set_var("ANIKIT_CACHE", "off");
    let out = std::path::Path::new("target/particles_reference");
    std::fs::create_dir_all(out).unwrap();
    println!("{:<34} {:>5} {:>9} {:>9} {:>8} {:>7} {:>9} {:>9}", "spec", "t", "PSNR-Y", "SSIM", "ΔE p99.9", "ΔE max", "fast", "reference");
    for (spec, t) in pairs {
        let src = std::fs::read_to_string(&spec).unwrap();
        let mut frames = Vec::new();
        let mut times = Vec::new();
        for refm in [false, true] {
            anikit_video::scene3d::particles::set_reference(refm);
            let parsed = anikit_video::spec::parse_str(&src).unwrap();
            let ev = Evaluator::new(&parsed.spec, Options { base_dir: std::path::Path::new(&spec).parent().unwrap().into(), ..Default::default() }).unwrap();
            let clock = std::time::Instant::now();
            let (w, h, px) = pipeline::render_rgba(&ev, t, 1.0);
            times.push(clock.elapsed().as_secs_f64());
            let name = format!("{}-{t}-{}.png", std::path::Path::new(&spec).file_stem().unwrap().to_string_lossy(), if refm { "ref" } else { "fast" });
            image::save_buffer(out.join(name), &px, w, h, image::ColorType::Rgba8).unwrap();
            frames.push((w as usize, h as usize, px));
        }
        let (w, h) = (frames[0].0, frames[0].1);
        let (psnr, ssim, de999, demax) = compare(&frames[0].2, &frames[1].2, w, h);
        let ok = ssim >= 0.995 && psnr >= 42.0 && de999 <= 2.3;
        println!("{:<34} {:>5.2} {:>8.2}dB {:>9.5} {:>8.2} {:>7.2} {:>8.1}s {:>8.1}s {}", spec, t, psnr, ssim, de999, demax, times[0], times[1], if ok { "ok" } else { "FAIL" });
    }
}
