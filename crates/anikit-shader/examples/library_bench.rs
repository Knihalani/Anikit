//! Time every built-in at 1920×1080 (and with 2×2 supersampling) and write a
//! PNG still of each: `cargo run --release -p anikit-shader --example library_bench -- [out_dir] [frames]`.
use anikit_shader::library::{Use, ALL};
use anikit_shader::{Frame, Shader, Texture};

fn test_card(w: u32, h: u32, seed: u32) -> Texture {
    // A colourful card with shapes and a gradient, to filter and transition between.
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut c = if seed == 0 { [0.1 + 0.5 * u, 0.15 + 0.3 * v, 0.45, 1.0] } else { [0.9 - 0.3 * v, 0.55 + 0.2 * u, 0.2, 1.0] };
            let (cx, cy) = (u - 0.5, (v - 0.5) * h as f32 / w as f32);
            if (cx * cx + cy * cy).sqrt() < 0.18 {
                c = if seed == 0 { [0.98, 0.8, 0.3, 1.0] } else { [0.15, 0.2, 0.6, 1.0] };
            }
            if ((x / 120) + (y / 120)) % 2 == 0 && v > 0.8 {
                c = [c[0] * 0.6, c[1] * 0.6, c[2] * 0.6, 1.0];
            }
            px.push(c);
        }
    }
    Texture::from_f32(w, h, px)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned();
    let frames: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
    let (w, h) = (1920u32, 1080u32);
    let ta = test_card(w, h, 0);
    let tb = test_card(w, h, 1);
    println!("| shader | use | compile ms | ms/frame | ms/frame 2×2 SS |\n|---|---|---|---|---|");
    let only = a.get(3).cloned();
    for e in ALL {
        if only.as_ref().is_some_and(|o| !o.split(',').any(|n| n == e.name)) {
            continue;
        }
        if anikit_shader::split_passes(e.src).is_some() {
            // Multi-pass recipes need buffers: time them with `anikit-video still`.
            println!("| `{}` | multi-pass | – | see `anikit-video still` | – |", e.name);
            continue;
        }
        let s = Shader::compile(e.src).unwrap_or_else(|err| panic!("{}: {err}", e.name));
        let mut px = vec![0u8; (w * h * 4) as usize];
        let time = |ss: bool, n: usize, px: &mut Vec<u8>| {
            let mut f = Frame::new(w, h, 2.0);
            f.supersample = ss;
            f.dither = std::env::var_os("DITHER").is_some(); // as anikit-video renders
            f.channels = [Some(&ta), Some(&tb), None, None];
            f.progress = 0.5;
            let amount = [0.5f32];
            let unis = [("amount", &amount[..])];
            if e.use_ == Use::Filter {
                f.uniforms = &unis;
            }
            s.render(&f, px); // warm-up
            let t0 = std::time::Instant::now();
            for i in 0..n {
                f.time = 2.0 + i as f32 / 30.0;
                f.frame = 60 + i as i32;
                s.render(&f, px);
            }
            t0.elapsed().as_secs_f64() * 1e3 / n as f64
        };
        let ms = time(false, frames, &mut px);
        if let Some(dir) = &out {
            let file = std::fs::File::create(format!("{dir}/{}.png", e.name)).unwrap();
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&px).unwrap();
        }
        let ms_ss = time(true, (frames / 3).max(1), &mut px);
        let use_ = match e.use_ {
            Use::Image => "image",
            Use::Filter => "filter",
            Use::Transition => "transition",
        };
        println!("| `{}` | {use_} | {:.0} | {ms:.0} | {ms_ss:.0} |", e.name, s.compile_ms);
    }
    println!("\n{} threads", rayon::current_num_threads());
}
