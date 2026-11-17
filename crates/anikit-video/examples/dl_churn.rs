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
        if lc && std::env::var_os("CHURN_DETAIL").is_some() {
            // First changed layer push: what is it?
            for k in 0..prev.ops.len().max(cur.ops.len()) {
                let (a, b) = (prev.ops.get(k), cur.ops.get(k));
                if a != b {
                    if let Some(Op::PushLayer { opacity, filters, clip, .. }) = b.or(a) {
                        let bb = clip.as_ref().map(|(p, t)| {
                            let r = kurbo::Shape::bounding_box(p);
                            let s = (t[0] * t[0] + t[1] * t[1]).sqrt();
                            format!("{:.0}x{:.0}@({:.0},{:.0})", r.width() * s as f64, r.height() * s as f64, t[4], t[5])
                        });
                        let same_len = prev.ops.len() == cur.ops.len();
                        println!("t={:.2} op#{k} opacity={opacity:.2} filters={} clip={:?} same_op_count={same_len}", i as f32 / ev.fps, filters.len(), bb);
                        break;
                    }
                }
            }
        }
        prev = cur;
    }
    println!("frames with a changed layer push/pop: {frames_with_layer_change}/{}", ev.frames());
    for (k, v) in changed {
        println!("{k:14} {v}");
    }
}
