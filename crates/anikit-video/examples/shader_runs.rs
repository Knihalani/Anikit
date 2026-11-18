//! How many times each shader layer runs, and its mean cost, over a frame range
//! rendered through the video pipeline (frames ≠ runs means re-renders or
//! extra passes).
//!   cargo run --release -p anikit-video --example shader_runs -- spec.json from to
use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::{self, Codec, Quality, RenderOptions};
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = PathBuf::from(&a[1]);
    let (from, to): (f32, f32) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let parsed = anikit_video::spec::parse_json(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|_| panic!("parse"));
    let opts = Options { base_dir: path.parent().unwrap().to_path_buf(), fonts: Some(Arc::new(anikit_video::text::FontStore::new())), load_assets: true, assets: None };
    let ev: Evaluator = Evaluator::build(&parsed.spec, &opts).0.unwrap();
    std::env::set_var("ANIKIT_RENDER_CACHE", "0");
    let out = std::env::temp_dir().join("shader_runs.mp4");
    let o = RenderOptions { codec: Codec::Ak264, quality: Quality::Good, from: Some(from), to: Some(to), scale: 1.0, progress: false, audio: false, profile: false };
    let s = pipeline::render_ak264(&ev, &o, &out, None).unwrap();
    println!("{} frames in {:.2} s", s.frames, s.seconds);
    for st in ev.shader_stats() {
        println!("{}: {} runs, {:.1} ms mean, {}×{}{}", st.label, st.renders, st.ms, st.width, st.height, if st.supersample { ", supersampled" } else { "" });
    }
}
