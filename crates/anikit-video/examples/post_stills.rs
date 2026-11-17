//! Before/after stills for the post presets:
//! `cargo run --release -p anikit-video --example post_stills -- spec.json 3.0 out_dir [preset…]`
use anikit_video::eval::{Evaluator, Options};
use anikit_video::render::post::PostSettings;
use anikit_video::render::Renderer;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = anikit_video::spec::parse_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let t: f32 = a[2].parse().unwrap();
    let out = &a[3];
    let presets: Vec<String> = if a.len() > 4 { a[4..].to_vec() } else { vec!["none".into(), "cinematic".into(), "dreamy".into(), "punchy".into(), "noir".into(), "vintage".into()] };
    let l = ev.eval(t);
    let mut r = Renderer::new(ev.assets.snapshot());
    if std::env::var("AK_QUALITY").as_deref() == Ok("draft") {
        r.set_quality(anikit_video::render::Quality::Draft);
    }
    for name in presets {
        r.set_post(PostSettings::preset(&name), (t * ev.fps) as u64);
        let t0 = Instant::now();
        let px = r.render(&l).data_as_u8_slice().to_vec();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let path = format!("{out}/{name}.png");
        image::save_buffer(&path, &px, l.width, l.height, image::ExtendedColorType::Rgba8).unwrap();
        println!("{name:10} {ms:7.1} ms  {path}");
    }
}
