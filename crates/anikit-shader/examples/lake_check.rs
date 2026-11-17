//! Aurora lake: traced in one pass (reference) vs sky-only + `lake-mirror`
//! filter, at 1080p over consecutive frames. Reports time, SSIM (luma, 8×8),
//! CIE76 ΔE p99.9 and flicker (mean |Δframe(tuned) − Δframe(ref)| in luma).
use anikit_shader::{library, Frame, Shader, Texture};

fn lab(p: [u8; 4]) -> [f32; 3] {
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
    let (w, h) = (1920u32, 1080u32);
    let src = library::get("aurora").unwrap().src;
    let reference = Shader::compile(src).unwrap();
    let sky = Shader::compile(src).unwrap();
    let mirror = Shader::compile(library::get("lake-mirror").unwrap().src).unwrap();
    let zero = [0.0f32];
    let unis = [("lake", &zero[..])];
    let (mut tr, mut tt) = (0.0, 0.0);
    let mut prev: Option<(Vec<u8>, Vec<u8>)> = None;
    let (mut ssim_min, mut des, mut flick, mut nflick) = (1.0f64, Vec::new(), 0.0f64, 0usize);
    let frames = 6;
    for i in 0..frames {
        let mut f = Frame::new(w, h, 2.0 + i as f32 / 30.0);
        f.dither = true;
        let mut r = vec![0u8; (w * h * 4) as usize];
        let t0 = std::time::Instant::now();
        reference.render(&f, &mut r);
        tr += t0.elapsed().as_secs_f64();
        let t0 = std::time::Instant::now();
        let mut s = vec![0u8; (w * h * 4) as usize];
        let mut fs = f.clone();
        fs.uniforms = &unis;
        sky.render(&fs, &mut s);
        let tex = Texture::from_rgba8(w, h, &s, true);
        let mut fm = f.clone();
        fm.channels[0] = Some(&tex);
        let mut m = vec![0u8; (w * h * 4) as usize];
        mirror.render(&fm, &mut m);
        tt += t0.elapsed().as_secs_f64();
        ssim_min = ssim_min.min(ssim(&r, &m, w as usize, h as usize));
        for (p, q) in r.chunks_exact(4).zip(m.chunks_exact(4)) {
            let (a, b) = (lab([p[0], p[1], p[2], 255]), lab([q[0], q[1], q[2], 255]));
            des.push(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt());
        }
        if let Some((pr, pm)) = &prev {
            for k in (0..r.len()).step_by(4) {
                let dr = luma(&r[k..]) - luma(&pr[k..]);
                let dm = luma(&m[k..]) - luma(&pm[k..]);
                flick += (dr - dm).abs() as f64;
                nflick += 1;
            }
        }
        if i == frames - 1 {
            if let Some(dir) = std::env::args().nth(1) {
                let save = |name: &str, px: &[u8]| {
                    let f = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
                    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
                    e.set_color(png::ColorType::Rgba);
                    e.set_depth(png::BitDepth::Eight);
                    e.write_header().unwrap().write_image_data(px).unwrap();
                };
                save("lake_ref", &r);
                save("lake_tuned", &m);
            }
        }
        prev = Some((r, m));
    }
    des.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p999 = des[(des.len() as f64 * 0.999) as usize];
    println!(
        "reference {:.0} ms/frame, sky + lake-mirror {:.0} ms/frame ({:.2}x); SSIM min {:.4}; ΔE p99.9 {:.2}, max {:.2}; flicker {:.3} luma levels",
        tr * 1e3 / frames as f64,
        tt * 1e3 / frames as f64,
        tr / tt,
        ssim_min,
        p999,
        des[des.len() - 1],
        flick / nflick.max(1) as f64
    );
}
