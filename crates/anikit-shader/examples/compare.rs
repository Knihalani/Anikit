//! Compare a tuned shader against its reference over several frames:
//! `cargo run --release -p anikit-shader --example compare -- reference.glsl tuned.glsl [w h]`.
use anikit_shader::{Frame, Shader};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = Shader::compile(&std::fs::read_to_string(&a[1]).unwrap()).unwrap_or_else(|e| panic!("{e}"));
    let t = Shader::compile(&std::fs::read_to_string(&a[2]).unwrap()).unwrap_or_else(|e| panic!("{e}"));
    let w: u32 = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(1920);
    let h: u32 = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(1080);
    let (mut tr, mut tt) = (f64::MAX, f64::MAX);
    let (mut maxe, mut se, mut n) = (0.0f32, 0.0f64, 0usize);
    for i in 0..4 {
        let f = Frame::new(w, h, 0.7 + i as f32 * 1.9);
        let t0 = std::time::Instant::now();
        let x = r.render_f32(&f);
        tr = tr.min(t0.elapsed().as_secs_f64() * 1e3);
        let t0 = std::time::Instant::now();
        let y = t.render_f32(&f);
        tt = tt.min(t0.elapsed().as_secs_f64() * 1e3);
        for (p, q) in x.iter().zip(&y) {
            for k in 0..3 {
                let e = (p[k].clamp(0.0, 1.0) - q[k].clamp(0.0, 1.0)).abs();
                maxe = maxe.max(e);
                se += (e * e) as f64;
                n += 1;
            }
        }
    }
    let psnr = -10.0 * (se / n as f64).max(1e-20).log10();
    println!("reference {tr:.0} ms, tuned {tt:.0} ms ({:.2}x); max err {:.3}/255, PSNR {psnr:.1} dB over 4 frames", tr / tt, maxe * 255.0);
}
