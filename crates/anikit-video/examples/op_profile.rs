//! Where a component's render time goes: lists its ops at one frame and the cost of each
//! top-level op group (an op, or a whole layer), measured by rendering the list with
//! that group removed.
//!
//! `cargo run --release -p anikit-video --example op_profile <kind> [t] [props-json]`

use anikit_video::components::{self, host::Host, Theme};
use anikit_video::dl::{DisplayList, Op};
use anikit_video::render::Renderer;
use std::sync::Arc;
use std::time::Instant;

fn groups(ops: &[Op]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        let start = i;
        if let Op::PushLayer { .. } = ops[i] {
            let mut depth = 0;
            loop {
                match ops[i] {
                    Op::PushLayer { .. } => depth += 1,
                    Op::PopLayer => depth -= 1,
                    _ => {}
                }
                i += 1;
                if depth == 0 {
                    break;
                }
            }
        } else {
            i += 1;
        }
        out.push((start, i));
    }
    out
}

fn label(op: &Op) -> String {
    match op {
        Op::Fill { .. } => "Fill".into(),
        Op::Stroke { .. } => "Stroke".into(),
        Op::RoundRect { w, h, .. } => format!("RoundRect {w:.0}x{h:.0}"),
        Op::Image { dst, .. } => format!("Image {:.0}x{:.0}", dst[2], dst[3]),
        Op::Glyphs { glyphs, .. } => format!("Glyphs ×{}", glyphs.len()),
        Op::PushLayer { opacity, blend, filters, clip, .. } => format!("Layer op={opacity:.2} {blend:?} {filters:?} clip={}", clip.is_some()),
        Op::PopLayer => "Pop".into(),
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let kind = a.next().expect("kind");
    let t: f32 = a.next().map(|s| s.parse().unwrap()).unwrap_or(1.0);
    let props: serde_json::Value = a.next().map(|s| serde_json::from_str(&s).unwrap()).unwrap_or_else(|| {
        let k = components::KINDS.iter().find(|k| k.name == kind).expect("kind");
        serde_json::from_str(k.example).unwrap()
    });
    let host = Host::bundled();
    let theme = Theme::default();
    let placed = components::build(&kind, &props).unwrap();
    let (w, h, fps) = (1920.0, 1080.0, 60.0);
    let ops = placed.eval(t, w, h, fps, &theme, &host);
    let mut r = Renderer::new(Arc::new(host.render_assets(None)));
    let dl = |ops: Vec<Op>| DisplayList { width: 1920, height: 1080, background: theme.background, ops };
    let time = |r: &mut Renderer, ops: &[Op]| {
        let d = dl(ops.to_vec());
        for _ in 0..3 {
            r.render(&d);
        }
        let n = 10;
        let t0 = Instant::now();
        for _ in 0..n {
            r.render(&d);
        }
        t0.elapsed().as_secs_f64() * 1e3 / n as f64
    };
    let full = time(&mut r, &ops);
    println!("{kind} t={t}: {} ops, full render {full:.2} ms", ops.len());
    for (s, e) in groups(&ops) {
        let mut rest = ops[..s].to_vec();
        rest.extend_from_slice(&ops[e..]);
        let without = time(&mut r, &rest);
        println!("  {:>6.2} ms  [{s}..{e}) {}", full - without, label(&ops[s]));
    }
}
