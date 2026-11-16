//! Stage timings for a spec: eval, hash, render (one thread), parallel render, encode.
//! `cargo run --release -p anikit-video --example pipeline_bench -- docs/video/examples/showcase.json`
use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline;
use anikit_video::render::{Renderer, YuvFrame};
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "docs/video/examples/showcase.json".into());
    let p = anikit_video::spec::parse_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let t = Instant::now();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    println!("compile        {:7.2} ms", t.elapsed().as_secs_f64() * 1e3);
    let n = ev.frames();
    let mut dl = Default::default();
    let t = Instant::now();
    for i in 0..n {
        ev.eval_into(i as f32 / ev.fps, &mut dl);
    }
    let te = t.elapsed().as_secs_f64() / n as f64;
    println!("eval           {:7.3} ms/frame  ({} ops last)", te * 1e3, dl.ops.len());
    let lists: Vec<_> = (0..n).map(|i| ev.eval(i as f32 / ev.fps)).collect();
    let t = Instant::now();
    for l in &lists {
        std::hint::black_box(pipeline::hash_list(l));
    }
    println!("hash           {:7.3} ms/frame", t.elapsed().as_secs_f64() / n as f64 * 1e3);
    let mut r = Renderer::new(ev.assets.snapshot());
    let mut f = YuvFrame::new(ev.width, ev.height);
    for l in lists.iter().take(10) {
        r.render_yuv(l, &mut f);
    }
    let t = Instant::now();
    for l in &lists {
        r.render_yuv(l, &mut f);
    }
    let tr = t.elapsed().as_secs_f64() / n as f64;
    println!("render_yuv 1T  {:7.3} ms/frame  ({:.0} fps)", tr * 1e3, 1.0 / tr);
    let t = Instant::now();
    let mut alloc = 0usize;
    for _ in 0..n {
        let f = YuvFrame::new(ev.width, ev.height);
        alloc += f.y.len();
    }
    println!("YuvFrame::new  {:7.3} ms/frame  ({alloc})", t.elapsed().as_secs_f64() / n as f64 * 1e3);
    let mut m = anikit_video::render::MbMap::new(0, 0);
    let t = Instant::now();
    for k in 1..lists.len() {
        anikit_video::render::diff::diff_mb_ops(&lists[k - 1], &lists[k], &mut m);
    }
    println!("diff_mb_ops    {:7.3} ms/frame", t.elapsed().as_secs_f64() / n as f64 * 1e3);
    // ak264 encode alone (single stream, sequential).
    let frames: Vec<YuvFrame> = lists.iter().map(|l| { let mut f = YuvFrame::new(ev.width, ev.height); r.render_yuv(l, &mut f); f }).collect();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let mut e = ak264::Encoder::new(ak264::EncoderConfig { width: ev.width, height: ev.height, fps: ev.fps as f64, qp: 23, gop: 120, deblock: true, threads, full_range: false });
    let t = Instant::now();
    let mut bytes = 0;
    for f in &frames {
        let fr = ak264::YuvFrame { width: ev.width, height: ev.height, y: &f.y, u: &f.u, v: &f.v, y_stride: f.y_stride, c_stride: f.c_stride };
        bytes += e.encode(&fr, &Default::default()).len();
    }
    let tk = t.elapsed().as_secs_f64() / n as f64;
    println!("ak264 encode   {:7.3} ms/frame  ({:.0} fps, {} KB, {threads} slices, no hints)", tk * 1e3, 1.0 / tk, bytes / 1024);
}
