//! Memory probe: render a spec N times in one process and print resident memory
//! after each render, to find retained memory (caches, leaks) and the peak.
//!
//!   cargo run --release -p anikit-video --example mem_probe -- spec.json [runs] [from] [to]
use anikit_video::eval::{Evaluator, Options};
use anikit_video::memstat::{mib, peak, rss};
use anikit_video::pipeline::{self, Codec, Quality, RenderOptions};
use anikit_video::spec::{self, Severity};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn build(path: &Path) -> Evaluator {
    let src = std::fs::read_to_string(path).expect("read spec");
    let parsed = spec::parse_json(&src).unwrap_or_else(|d| panic!("{}", d.iter().map(|x| format!("{}: {}", x.path, x.message)).collect::<Vec<_>>().join("; ")));
    let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let fonts = anikit_video::text::FontStore::new();
    if let Some(f) = &parsed.spec.fonts {
        for file in f.values() {
            fonts.add_font_file(base.join(file)).expect("font");
        }
    }
    let opts = Options { base_dir: base, fonts: Some(Arc::new(fonts)), load_assets: true, assets: None };
    let (ev, d) = Evaluator::build(&parsed.spec, &opts);
    let errs: Vec<String> = d.iter().filter(|x| x.severity == Severity::Error).map(|x| format!("{}: {}", x.path, x.message)).collect();
    assert!(errs.is_empty(), "{}", errs.join("; "));
    ev.unwrap()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = PathBuf::from(&a[1]);
    let runs: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
    let from = a.get(3).and_then(|s| s.parse().ok());
    let to = a.get(4).and_then(|s| s.parse().ok());
    std::env::set_var("ANIKIT_RENDER_CACHE", "0");
    let out = std::env::temp_dir().join(format!("mem_probe-{}.mp4", std::process::id()));
    println!("start: rss {:.0} MiB", mib(rss().unwrap_or(0)));
    for run in 0..runs {
        let ev = build(&path);
        let built = rss().unwrap_or(0);
        let o = RenderOptions { codec: Codec::Ak264, quality: Quality::Good, from, to, scale: 1.0, progress: false, audio: false, profile: run == 0 };
        let s = pipeline::render_ak264(&ev, &o, &out, None).expect("render");
        let after = rss().unwrap_or(0);
        drop(ev);
        println!(
            "run {run}: built {:.0} MiB, after render {:.0} MiB, after drop {:.0} MiB, process peak {:.0} MiB, {:.2} s",
            mib(built),
            mib(after),
            mib(rss().unwrap_or(0)),
            mib(peak().unwrap_or(0)),
            s.seconds
        );
    }
    let _ = std::fs::remove_file(&out);
}
