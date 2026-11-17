//! `cargo run --release -p anikit-shader --example bench -- file.glsl [w h frames ss]`
use anikit_shader::{Frame, Shader};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = std::fs::read_to_string(&a[1]).unwrap();
    let w: u32 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(1920);
    let h: u32 = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(1080);
    let n: usize = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(10);
    let ss = a.get(5).map(|s| s == "ss").unwrap_or(false);
    let s = Shader::compile(&src).unwrap_or_else(|e| panic!("{e}"));
    println!("compile {:.1} ms, {} IR insts", s.compile_ms, s.insts);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut f = Frame::new(w, h, 0.0);
    f.supersample = ss;
    s.render(&f, &mut px);
    // Best single frame of n (this machine is shared and noisy).
    let mut ms = f64::MAX;
    for i in 0..n {
        f.time = i as f32 / 30.0;
        f.frame = i as i32;
        let t0 = std::time::Instant::now();
        s.render(&f, &mut px);
        ms = ms.min(t0.elapsed().as_secs_f64() * 1e3);
    }
    println!("{w}x{h}{}: {ms:.1} ms/frame (best of {n}) {} threads", if ss { " 2x2 SS" } else { "" }, rayon::current_num_threads());
    if ss {
        // Adaptive vs full 2×2 supersampling on the same frame.
        f.adaptive = false;
        let t0 = std::time::Instant::now();
        let full = s.render_f32(&f);
        let tf = t0.elapsed().as_secs_f64() * 1e3;
        f.adaptive = true;
        let t0 = std::time::Instant::now();
        let ad = s.render_f32(&f);
        let ta = t0.elapsed().as_secs_f64() * 1e3;
        let (mut maxe, mut se, mut over) = (0.0f32, 0.0f64, 0usize);
        for (p, q) in full.iter().zip(&ad) {
            for k in 0..3 {
                let e = (p[k].clamp(0.0, 1.0) - q[k].clamp(0.0, 1.0)).abs();
                maxe = maxe.max(e);
                se += (e * e) as f64;
                if e > 1.0 / 255.0 {
                    over += 1;
                }
            }
        }
        let mse = se / (full.len() * 3) as f64;
        println!("full SS {tf:.0} ms, adaptive {ta:.0} ms; max err {:.2}/255, PSNR {:.1} dB, {:.3}% of samples off by >1/255", maxe * 255.0, -10.0 * mse.max(1e-20).log10(), over as f64 * 100.0 / (full.len() * 3) as f64);
    }
    if let Some(out) = a.get(6) {
        let f = std::fs::File::create(out).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }
}
