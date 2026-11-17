//! Adaptive anti-aliasing: for each image shader in the library, 1 sample per
//! pixel and adaptive 2×2 at several thresholds against full 2×2 (the
//! reference), at 1080p over two consecutive frames. Reports SSIM (luma 8×8,
//! min), CIE76 ΔE p99.9, flicker and cost.
//! `cargo run --release -p anikit-shader --example aa_check -- [thresholds/255,…] [names,…]`
use anikit_shader::{library, Frame, Shader};

fn lab(p: &[u8]) -> [f32; 3] {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(p[0]), lin(p[1]), lin(p[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    [116.0 * f(y) - 16.0, 500.0 * (f(x) - f(y)), 200.0 * (f(y) - f(z))]
}

fn luma(p: &[u8]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn ssim(a: &[u8], b: &[u8], w: usize, h: usize) -> f64 {
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let (mut sum, mut n) = (0.0, 0);
    for by in (0..h - 8).step_by(4) {
        for bx in (0..w - 8).step_by(4) {
            let (mut ma, mut mb, mut va, mut vb, mut cov) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for y in by..by + 8 {
                for x in bx..bx + 8 {
                    let i = (y * w + x) * 4;
                    let (p, q) = (luma(&a[i..]) as f64, luma(&b[i..]) as f64);
                    ma += p;
                    mb += q;
                    va += p * p;
                    vb += q * q;
                    cov += p * q;
                }
            }
            let k = 64.0;
            let (ma, mb) = (ma / k, mb / k);
            let (va, vb, cov) = (va / k - ma * ma, vb / k - mb * mb, cov / k - ma * mb);
            sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
        }
    }
    sum / n as f64
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ths: Vec<f32> = a.get(1).map(|s| s.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or(vec![1.5, 4.0, 8.0]);
    let only: Option<Vec<String>> = a.get(2).map(|s| s.split(',').map(String::from).collect());
    let (w, h) = (1920u32, 1080u32);
    println!("| shader | mode | ms/frame | × 1 spp | SSIM min | ΔE p99.9 | flicker |");
    println!("|---|---|---|---|---|---|---|");
    for e in library::ALL.iter().filter(|e| e.use_ == library::Use::Image) {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == e.name)) {
            continue;
        }
        let s = Shader::compile(e.src).unwrap();
        let render = |t: f32, ss: bool, adaptive: bool, th: f32| -> (Vec<u8>, f64) {
            let mut f = Frame::new(w, h, t);
            f.frame = (t * 30.0).round() as i32;
            f.supersample = ss;
            f.adaptive = adaptive;
            f.threshold = th / 255.0;
            f.max_refine = std::env::var("CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
            let mut px = vec![0u8; (w * h * 4) as usize];
            let t0 = std::time::Instant::now();
            s.render(&f, &mut px);
            (px, t0.elapsed().as_secs_f64() * 1e3)
        };
        let times = [1.0f32, 1.0 + 1.0 / 30.0];
        let refs: Vec<Vec<u8>> = times.iter().map(|&t| render(t, true, false, 0.0).0).collect();
        let mut modes: Vec<(String, bool, f32)> = vec![("1 spp".into(), false, 0.0)];
        for &t in &ths {
            modes.push((format!("adaptive {t}/255"), true, t));
        }
        let mut base_ms = 0.0;
        for (name, ss, th) in modes {
            let mut ms = 0.0;
            let mut ssim_min = 1.0f64;
            let mut des: Vec<f32> = Vec::new();
            let mut outs = Vec::new();
            for (k, &t) in times.iter().enumerate() {
                let _ = render(t, ss, true, th); // warm
                let (px, m) = render(t, ss, true, th);
                ms += m / times.len() as f64;
                ssim_min = ssim_min.min(ssim(&px, &refs[k], w as usize, h as usize));
                for (p, q) in px.chunks_exact(4).zip(refs[k].chunks_exact(4)).step_by(3) {
                    let (l1, l2) = (lab(p), lab(q));
                    des.push(((l1[0] - l2[0]).powi(2) + (l1[1] - l2[1]).powi(2) + (l1[2] - l2[2]).powi(2)).sqrt());
                }
                outs.push(px);
            }
            des.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p999 = des[(des.len() as f64 * 0.999) as usize];
            // Flicker: mean |Δframe(test) − Δframe(ref)| in luma levels.
            let mut fl = 0.0f64;
            let n = (w * h) as usize;
            for i in 0..n {
                let d1 = luma(&outs[1][i * 4..]) - luma(&outs[0][i * 4..]);
                let d2 = luma(&refs[1][i * 4..]) - luma(&refs[0][i * 4..]);
                fl += (d1 - d2).abs() as f64;
            }
            fl /= n as f64;
            if !ss {
                base_ms = ms;
            }
            println!("| `{}` | {name} | {ms:.0} | {:.2} | {ssim_min:.4} | {p999:.2} | {fl:.2} |", e.name, ms / base_ms.max(1e-9));
        }
    }
}
