//! Where a still's startup goes: parse, font store, evaluator build (assets,
//! music, beat analysis), first evaluation and render.
//!   cargo run --release -p anikit-video --example still_cost -- spec.json [t]
use anikit_video::eval::{Evaluator, Options};
use anikit_video::spec;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = PathBuf::from(&a[1]);
    let t: f32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let t0 = Instant::now();
    let src = std::fs::read_to_string(&path).expect("read");
    let parsed = spec::parse_json(&src).unwrap_or_else(|_| panic!("parse"));
    let t_parse = t0.elapsed();
    let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let t1 = Instant::now();
    let fonts = anikit_video::text::FontStore::new();
    if let Some(f) = &parsed.spec.fonts {
        for file in f.values() {
            fonts.add_font_file(base.join(file)).expect("font");
        }
    }
    let t_fonts = t1.elapsed();
    let t2 = Instant::now();
    let opts = Options { base_dir: base, fonts: Some(Arc::new(fonts)), load_assets: true, assets: None };
    let (ev, _d) = Evaluator::build(&parsed.spec, &opts);
    let ev: Evaluator = ev.expect("build");
    let t_build = t2.elapsed();
    let t3 = Instant::now();
    let (w, h, px) = anikit_video::pipeline::render_rgba(&ev, t, 1.0);
    let t_frame = t3.elapsed();
    println!(
        "parse {:.1} ms · fonts {:.1} ms · build {:.1} ms · first frame {:.1} ms ({w}×{h}, {} bytes)",
        t_parse.as_secs_f64() * 1e3,
        t_fonts.as_secs_f64() * 1e3,
        t_build.as_secs_f64() * 1e3,
        t_frame.as_secs_f64() * 1e3,
        px.len()
    );
}
