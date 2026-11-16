//! Text benchmarks: `cargo run --release -p anikit-video --example text_bench`.
use std::time::Instant;

use anikit_video::dl::IDENTITY;
use anikit_video::text::code::{highlight, CodeSpec};
use anikit_video::text::kinetic::*;
use anikit_video::text::*;

fn time<T>(label: &str, iters: u32, mut f: impl FnMut() -> T) -> f64 {
    for _ in 0..(iters / 10).max(1) {
        std::hint::black_box(f());
    }
    let t = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(f());
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / iters as f64;
    println!("{label:<48} {us:>10.1} µs");
    us
}

fn main() {
    let words = "Agents are best at writing structured data so the video is a spec of scenes text shapes and timing while a native renderer draws every frame in parallel across all cores and the encoder knows exactly which blocks changed";
    let w: Vec<&str> = words.split(' ').collect();
    let para: String = (0..200).map(|i| w[i % w.len()]).collect::<Vec<_>>().join(" ");
    let t = Instant::now();
    let fonts = FontStore::new();
    println!("{:<48} {:>10.1} µs", "FontStore::new (bundled)", t.elapsed().as_secs_f64() * 1e6);

    let spec = TextSpec { text: para.clone(), width: Some(1600.0), style: TextStyle { size: 36.0, ..Default::default() }, ..Default::default() };
    time("layout 200 words (uncached)", 50, || fonts.layout_uncached(&spec));
    time("layout 200 words (cache hit)", 10000, || fonts.layout(&spec));
    let rich = TextSpec {
        spans: para.split(". ").flat_map(|s| [Span { text: s.into(), ..Default::default() }, Span { text: ". ".into(), weight: Some(700.0), color: Some(Color::hex("#7c5cff").unwrap()), ..Default::default() }]).collect(),
        ..spec.clone()
    };
    time("layout 200 words, rich spans (uncached)", 50, || fonts.layout_uncached(&rich));
    let fit = TextSpec { fit: Some(Fit { width: 1600.0, height: 600.0, min_size: 8.0, max_size: Some(96.0) }), width: None, ..spec.clone() };
    time("auto-fit 200 words (uncached)", 10, || fonts.layout_uncached(&fit));
    let ell = TextSpec { max_lines: Some(3), ..spec.clone() };
    time("max 3 lines + ellipsis (uncached)", 10, || fonts.layout_uncached(&ell));

    let l = fonts.layout(&spec);
    println!("  ({} glyphs, {} words, {} lines)", l.glyphs.len(), l.words.len(), l.lines.len());
    let mut ops = Vec::with_capacity(4096);
    time("static ops, 200 words", 10000, || {
        ops.clear();
        l.ops(IDENTITY, &mut ops);
        ops.len()
    });
    for name in STYLE_NAMES {
        for unit in ["char", "word"] {
            let k: Kinetic = format!("{name} by {unit}").parse().unwrap();
            let plan = KineticPlan::new(&fonts, l.clone(), &k);
            let mid = plan.duration() * 0.5;
            let us = time(&format!("kinetic {name:<10} by {unit:<4} mid-frame, 200 words"), 2000, || {
                ops.clear();
                plan.ops(mid, IDENTITY, &mut ops);
                ops.len()
            });
            let _ = us;
        }
    }
    let title = fonts.layout(&TextSpec::plain("Ship faster with programmatic video", TextStyle { size: 96.0, weight: 800.0, ..Default::default() }));
    let plan = KineticPlan::new(&fonts, title, &"pop by char".parse().unwrap());
    time("kinetic pop by char mid-frame, title", 20000, || {
        ops.clear();
        plan.ops(0.4, IDENTITY, &mut ops);
        ops.len()
    });
    let c = CounterPlan::new(&fonts, &Counter { to: 1284503.0, prefix: "$".into(), ..Default::default() });
    time("counter frame", 20000, || {
        ops.clear();
        c.ops(0.7, IDENTITY, &mut ops);
        ops.len()
    });
    let code = include_str!("../src/text/kinetic.rs").lines().take(40).collect::<Vec<_>>().join("\n");
    time("highlight 40 lines of Rust (first call loads grammars)", 1, || highlight(&CodeSpec { code: code.clone(), language: "rust".into(), ..Default::default() }));
    time("highlight 40 lines of Rust", 20, || highlight(&CodeSpec { code: code.clone(), language: "rust".into(), ..Default::default() }));
    let ts = "const x: number = 1;\n".repeat(40);
    time("highlight 40 lines of TypeScript", 20, || highlight(&CodeSpec { code: ts.clone(), language: "ts".into(), ..Default::default() }));
}
