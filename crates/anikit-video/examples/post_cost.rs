//! Where a post + motion blur frame's time goes (single thread, 1080p).
//! `cargo run --release -p anikit-video --example post_cost [spec] [frame]`
use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline;
use anikit_video::render::Renderer;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = a.get(1).cloned().unwrap_or("crates/anikit-video/qbench/post.json".into());
    let frame: u32 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(20);
    let src = std::fs::read_to_string(&path).unwrap();
    let parsed = anikit_video::spec::parse_json(&src).unwrap();
    let (ev, _) = Evaluator::build(&parsed.spec, &Options { base_dir: std::path::Path::new(&path).parent().unwrap().into(), ..Default::default() });
    let ev: Evaluator = ev.unwrap();
    let t = pipeline::frame_time(&ev, frame);
    let mut r = Renderer::new(ev.assets.snapshot_fonts());
    let ass = ev.assets.clone();
    r.set_image_source(Arc::new(move |id| ass.pixmap(id)));
    let dl = ev.eval(t);
    let time = |name: &str, f: &mut dyn FnMut()| {
        f();
        let n = 10;
        let t0 = Instant::now();
        for _ in 0..n {
            f();
        }
        eprintln!("{name:32} {:8.2} ms", t0.elapsed().as_secs_f64() * 1e3 / n as f64);
    };
    if std::env::var("OPS").is_ok() {
        op_cost(&ev, &mut r, t);
        return;
    }
    if std::env::var("FIELD").is_ok() {
        field_cost(&ev, t);
    }
    let (post, blur) = pipeline::frame_look(&ev, pipeline::Quality::Good, frame);
    eprintln!("frame {frame}: post {} blur {blur}, shutter {}", post.is_some(), ev.shutter);
    time("eval", &mut || {
        let _ = ev.eval(t);
    });
    r.set_post(None, 0);
    time("render (no post)", &mut || {
        r.render(&dl);
    });
    r.set_post(post.clone(), frame as u64);
    time("render + post", &mut || {
        r.render(&dl);
    });
    r.set_post(None, 0);
    time("motion blur (no post)", &mut || {
        let mut e = |t: f32| ev.eval(t);
        r.render_motion_blur(&mut e, t, ev.shutter);
    });
    r.set_post(post.clone(), frame as u64);
    time("motion blur + post", &mut || {
        let mut e = |t: f32| ev.eval(t);
        r.render_motion_blur(&mut e, t, ev.shutter);
    });
}

/// Gradient-field share of a frame (`FIELD=1`).
#[allow(dead_code)]
pub fn field_cost(ev: &Evaluator, t: f32) {
    use anikit_video::render::smooth;
    let dl = ev.eval(t);
    let (w, h) = (dl.width as usize, dl.height as usize);
    let n = smooth::prefix(&dl.ops, w as f64, h as f64, &|id| ev.assets.pixmap(id).map(|p| (p.width() as u32, p.height() as u32)));
    let ok = n > 0 && smooth::field_ok(&dl.ops[..n]);
    eprintln!("smooth prefix {n} ops of {}, field {ok}", dl.ops.len());
    if ok {
        let mut out = vec![0u8; w * h * 4];
        let t0 = Instant::now();
        for _ in 0..5 {
            smooth::render_field(&dl.ops[..n], w, h, dl.background, &mut out);
        }
        eprintln!("render_field (pool)               {:8.2} ms", t0.elapsed().as_secs_f64() * 1e3 / 5.0);
    }
}

/// Cost of each top-level item (an op, or a whole layer subtree) by cumulative prefixes (`OPS=1`).
#[allow(dead_code)]
pub fn op_cost(ev: &Evaluator, r: &mut Renderer, t: f32) {
    use anikit_video::dl::Op;
    let dl = ev.eval(t);
    let mut items = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    for (i, op) in dl.ops.iter().enumerate() {
        match op {
            Op::PushLayer { .. } => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            Op::PopLayer => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    items.push((start, i + 1));
                }
            }
            _ if depth == 0 => items.push((i, i + 1)),
            _ => {}
        }
    }
    let time = |r: &mut Renderer, d: &anikit_video::dl::DisplayList| {
        r.render(d);
        let t0 = Instant::now();
        for _ in 0..5 {
            r.invalidate();
            r.render(d);
        }
        t0.elapsed().as_secs_f64() * 1e3 / 5.0
    };
    let mut prev = {
        let mut d = dl.clone();
        d.ops.clear();
        time(r, &d)
    };
    eprintln!("empty frame {prev:.2} ms");
    for (a, b) in items {
        let mut d = dl.clone();
        d.ops.truncate(b);
        let c = time(r, &d);
        let desc = format!("{:?}", dl.ops[a]);
        eprintln!("{:6.2} ms  ops {a}..{b}  {}", c - prev, &desc[..desc.len().min(150)]);
        prev = c;
    }
}
