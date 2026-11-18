//! Time a GLSL file at 1920×1080 the way `anikit-video` renders a shader layer:
//! one sample per pixel, adaptive 2×2 AA (the default), and full 2×2.
//! Uniforms take the value in a `// = v` comment after their declaration, else 0.
//!
//!   cargo run --release -p anikit-shader --example file_cost -- shader.glsl [time] [frames] [name=value ...]
use anikit_shader::{Frame, Shader};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = std::fs::read_to_string(&a[1]).expect("read shader");
    let t: f32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let n: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(6);
    let mut unis: Vec<(String, Vec<f32>)> = Vec::new();
    for l in src.lines() {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("uniform float ") {
            let name = rest.split(|c: char| c == ';' || c.is_whitespace()).next().unwrap().to_string();
            let v = l.split("// =").nth(1).and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            unis.push((name, vec![v]));
        }
    }
    for kv in a.iter().skip(4) {
        let (k, v) = kv.split_once('=').expect("name=value");
        let v: f32 = v.parse().expect("value");
        match unis.iter_mut().find(|u| u.0 == k) {
            Some(u) => u.1 = vec![v],
            None => unis.push((k.to_string(), vec![v])),
        }
    }
    let s = Shader::compile(&src).unwrap_or_else(|e| panic!("{e}"));
    let (w, h) = (1920u32, 1080u32);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let refs: Vec<(&str, &[f32])> = unis.iter().map(|(n, v)| (n.as_str(), v.as_slice())).collect();
    let time = |ss: bool, adaptive: bool, px: &mut Vec<u8>| {
        let mut f = Frame::new(w, h, t);
        f.uniforms = &refs;
        f.supersample = ss;
        f.adaptive = adaptive;
        f.threshold = anikit_shader::AA_THRESHOLD;
        f.max_refine = anikit_shader::AA_MAX_REFINE;
        f.dither = true;
        s.render(&f, px);
        let t0 = std::time::Instant::now();
        for i in 0..n {
            f.time = t + i as f32 / 30.0;
            f.frame = (f.time * 30.0) as i32;
            s.render(&f, px);
        }
        t0.elapsed().as_secs_f64() * 1e3 / n as f64
    };
    println!(
        "compile {:.0} ms · 1 sample {:.1} ms · adaptive AA {:.1} ms · full 2×2 {:.1} ms (wall, {} threads)",
        s.compile_ms,
        time(false, false, &mut px),
        time(true, true, &mut px),
        time(true, false, &mut px),
        rayon::current_num_threads()
    );
}
