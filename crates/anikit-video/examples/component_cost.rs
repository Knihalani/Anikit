//! Per-component cost at 1080p, single thread: eval (ops for one frame) and
//! render (rasterising those ops), averaged over a second of 30 fps frames
//! after a warm-up. Also reports how many frames were identical to the
//! previous one (static spans dedupe / P_Skip for free downstream).
//!
//! `cargo run --release -p anikit-video --example component_cost [kind,kind,…]`

use anikit_video::components::{self, host::Host, Theme};
use anikit_video::dl::DisplayList;
use anikit_video::render::{MbMap, Renderer, YuvFrame};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let only: Vec<String> = std::env::args().nth(1).map(|s| s.split(',').map(|s| s.to_string()).collect()).unwrap_or_default();
    let theme = Theme::default();
    let host = Host::bundled();
    let (w, h, fps) = (1920.0f32, 1080.0f32, 30.0f32);
    println!("| kind | eval ms | full render ms | incremental YUV ms | static frames |");
    println!("|---|---:|---:|---:|---:|");
    let mut total = (0.0, 0.0, 0.0);
    for k in components::KINDS.iter().filter(|k| only.is_empty() || only.iter().any(|o| o == k.name)) {
        let params: serde_json::Value = serde_json::from_str(k.example).unwrap();
        let placed = components::build(k.name, &params).unwrap();
        // Warm up: text layouts, images, caches (first half second).
        for f in 0..15 {
            let _ = placed.eval(f as f32 / fps, w, h, fps, &theme, &host);
        }
        let n = 30;
        let t0 = Instant::now();
        let lists: Vec<_> = (15..15 + n).map(|f| placed.eval(f as f32 / fps, w, h, fps, &theme, &host)).collect();
        let eval_ms = t0.elapsed().as_secs_f64() * 1e3 / n as f64;
        let statics = lists.windows(2).filter(|p| p[0] == p[1]).count();
        // The assets snapshot holds every image the frames above made.
        let mut r = Renderer::new(Arc::new(host.render_assets(None)));
        let mut yuv = YuvFrame::new(w as u32, h as u32);
        let mut mb = MbMap::new((w as usize).div_ceil(16), (h as usize).div_ceil(16));
        let dl = |ops: &Vec<_>| DisplayList { width: w as u32, height: h as u32, background: theme.background, ops: ops.clone() };
        // Full render (worst case: every frame drawn from scratch).
        for ops in lists.iter().take(3) {
            r.render(&dl(ops));
        }
        let t1 = Instant::now();
        for ops in &lists {
            r.render(&dl(ops));
        }
        let render_ms = t1.elapsed().as_secs_f64() * 1e3 / n as f64;
        // Pipeline path: incremental render straight to YUV (unchanged ops are skipped).
        let mut r = Renderer::new(Arc::new(host.render_assets(None)));
        r.render_yuv_incremental(&dl(&lists[0]), &mut yuv, &mut mb);
        let t2 = Instant::now();
        for ops in &lists[1..] {
            r.render_yuv_incremental(&dl(ops), &mut yuv, &mut mb);
        }
        let inc_ms = t2.elapsed().as_secs_f64() * 1e3 / (n - 1) as f64;
        total.0 += eval_ms;
        total.1 += render_ms;
        total.2 += inc_ms;
        println!("| `{}` | {eval_ms:.3} | {render_ms:.2} | {inc_ms:.2} | {statics}/{} |", k.name, n - 1);
    }
    eprintln!("total: eval {:.2} ms, full render {:.2} ms, incremental {:.2} ms", total.0, total.1, total.2);
}
