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
    let t0 = std::time::Instant::now();
    for i in 0..n {
        f.time = i as f32 / 30.0;
        f.frame = i as i32;
        s.render(&f, &mut px);
    }
    let ms = t0.elapsed().as_secs_f64() * 1e3 / n as f64;
    println!("{w}x{h}{}: {ms:.1} ms/frame ({} threads)", if ss { " 2x2 SS" } else { "" }, rayon::current_num_threads());
    if let Some(out) = a.get(6) {
        let f = std::fs::File::create(out).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }
}
