//! Print a render estimate: `cargo run --release -p anikit-video --example estimate -- spec.json`
use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::{self, RenderOptions};

fn main() {
    let path = std::env::args().nth(1).expect("spec path");
    let p = anikit_video::spec::parse_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let base = std::path::Path::new(&path).parent().unwrap().to_path_buf();
    let ev = Evaluator::new(&p.spec, Options { base_dir: base, ..Default::default() }).unwrap();
    let t = std::time::Instant::now();
    let e = pipeline::estimate(&ev, &RenderOptions::default(), 0.0);
    println!("{e}  [estimating took {:.2} s]", t.elapsed().as_secs_f64());
}
