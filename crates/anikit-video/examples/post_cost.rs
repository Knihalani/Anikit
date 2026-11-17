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
