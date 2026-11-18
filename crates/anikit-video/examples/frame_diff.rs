//! Which display-list ops differ between frame k and k+1 (to find what keeps a
//! "held" span from being skipped).
//!   cargo run --release -p anikit-video --example frame_diff -- spec.json frame
use anikit_video::eval::{Evaluator, Options};
use anikit_video::spec;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = PathBuf::from(&a[1]);
    let k: u32 = a[2].parse().unwrap();
    let parsed = spec::parse_json(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|_| panic!("parse"));
    let base = path.parent().unwrap().to_path_buf();
    let opts = Options { base_dir: base, fonts: Some(Arc::new(anikit_video::text::FontStore::new())), load_assets: true, assets: None };
    let ev: Evaluator = Evaluator::build(&parsed.spec, &opts).0.unwrap();
    let t = |i: u32| anikit_video::pipeline::frame_time(&ev, i);
    let (x, y) = (ev.eval(t(k)), ev.eval(t(k + 1)));
    println!("ops {} vs {}", x.ops.len(), y.ops.len());
    for (i, (p, q)) in x.ops.iter().zip(&y.ops).enumerate() {
        if p != q {
            let (sp, sq) = (format!("{p:?}"), format!("{q:?}"));
            let n = sp.chars().zip(sq.chars()).take_while(|(a, b)| a == b).count();
            println!("op {i}: …{}… vs …{}…", &sp[n.saturating_sub(120)..(n + 60).min(sp.len())], &sq[n.saturating_sub(0)..(n + 60).min(sq.len())]);
        }
    }
}
