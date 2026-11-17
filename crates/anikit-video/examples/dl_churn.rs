//! Which ops change between consecutive frames (and which layer pushes)?
use anikit_video::dl::Op;
use anikit_video::eval::{Evaluator, Options};
use std::collections::BTreeMap;
fn kind(o: &Op) -> &'static str {
    match o {
        Op::Fill { .. } => "fill",
        Op::Stroke { .. } => "stroke",
        Op::RoundRect { .. } => "rrect",
        Op::Image { .. } => "image",
        Op::Glyphs { .. } => "glyphs",
        Op::PushLayer { filters, clip, .. } => {
            if !filters.is_empty() { "push+filter" } else if clip.is_some() { "push+clip" } else { "push(opacity)" }
        }
        Op::PopLayer => "pop",
    }
}
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = anikit_video::spec::parse_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let ev = Evaluator::new(&p.spec, Options { base_dir: std::path::Path::new(&path).parent().unwrap().into(), ..Default::default() }).unwrap();
    let mut changed: BTreeMap<&str, u32> = BTreeMap::new();
    let mut frames_with_layer_change = 0;
    let mut prev = ev.eval(0.0);
    for i in 1..ev.frames() {
        let cur = ev.eval(i as f32 / ev.fps);
        let mut lc = false;
        for k in 0..prev.ops.len().max(cur.ops.len()) {
            let (a, b) = (prev.ops.get(k), cur.ops.get(k));
            if a != b {
                for o in [a, b].into_iter().flatten() {
                    *changed.entry(kind(o)).or_default() += 1;
                    lc |= matches!(o, Op::PushLayer { .. } | Op::PopLayer);
                }
            }
        }
        frames_with_layer_change += lc as u32;
        prev = cur;
    }
    println!("frames with a changed layer push/pop: {frames_with_layer_change}/{}", ev.frames());
    for (k, v) in changed {
        println!("{k:14} {v}");
    }
}
