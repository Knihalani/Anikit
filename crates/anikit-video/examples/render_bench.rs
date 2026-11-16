//! 1080p render benchmark: a typical motion-graphics display list.
//!
//! `cargo run --release -p anikit-video --example render_bench`

use std::sync::Arc;
use std::time::Instant;

use anikit_video::dl::*;
use anikit_video::render::{rgba_to_yuv420, Assets, Renderer, YuvFrame};
use kurbo::{BezPath, Shape};
use rayon::prelude::*;

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba([r, g, b, a])
}

/// Background gradient, 30 cards (optionally with cached drop shadows), 400
/// glyph-sized shapes, 10 strokes, text if a font is present. Animated by `t`.
fn scene(t: f32, font: Option<FontId>, shadows: bool) -> DisplayList {
    let mut ops = Vec::new();
    ops.push(Op::Fill {
        path: kurbo::Rect::new(0.0, 0.0, 1920.0, 1080.0).to_path(0.1),
        paint: Paint::Linear {
            p0: [0.0, 0.0],
            p1: [1920.0, 1080.0],
            stops: vec![(0.0, rgba(0.05, 0.05, 0.12, 1.0)), (1.0, rgba(0.15, 0.05, 0.25, 1.0))],
        },
        rule: FillRule::NonZero,
        transform: IDENTITY,
    });
    for i in 0..30 {
        let (cx, cy) = ((i % 6) as f32 * 310.0 + 40.0, (i / 6) as f32 * 205.0 + 40.0);
        let moving = i < 4;
        let dx = if moving { (t * 3.0 + i as f32).sin() * 40.0 } else { 0.0 };
        let card = Op::RoundRect {
            x: cx + dx,
            y: cy,
            w: 280.0,
            h: 170.0,
            radius: 16.0,
            paint: Paint::Solid(rgba(0.95, 0.95, 0.97, 0.9)),
            transform: IDENTITY,
        };
        if shadows {
            ops.push(Op::PushLayer {
                opacity: 1.0,
                blend: Blend::Normal,
                filters: vec![Filter::Shadow { dx: 0.0, dy: 8.0, blur: 12.0, color: rgba(0.0, 0.0, 0.0, 0.5) }],
                clip: None,
                cache_key: if moving { None } else { Some(1000 + i as u64) },
            });
            ops.push(card);
            ops.push(Op::PopLayer);
        } else {
            ops.push(card);
        }
    }
    for i in 0..400 {
        let (x, y) = ((i % 40) as f64 * 47.0 + 20.0, (i / 40) as f64 * 100.0 + 60.0 + (t as f64 * 2.0 + i as f64).sin() * 10.0);
        ops.push(Op::Fill {
            path: kurbo::Circle::new((x, y), 6.0).to_path(0.1),
            paint: Paint::Solid(rgba((i % 7) as f32 / 7.0, 0.6, 1.0, 0.8)),
            rule: FillRule::NonZero,
            transform: IDENTITY,
        });
    }
    for i in 0..10 {
        let mut p = BezPath::new();
        p.move_to((100.0, 100.0 + i as f64 * 90.0));
        p.curve_to((600.0, 0.0), (1200.0, 1080.0), (1800.0, 100.0 + i as f64 * 90.0));
        ops.push(Op::Stroke {
            path: p,
            paint: Paint::Solid(rgba(1.0, 0.5, 0.2, 1.0)),
            stroke: Stroke { width: 4.0, cap: 1, join: 1, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, (t * 0.5).fract().max(0.05)] },
            transform: IDENTITY,
        });
    }
    if let Some(f) = font {
        for line in 0..6 {
            let glyphs = (0..40).map(|k| Glyph { id: 36 + (k * 7 + line) as u16 % 50, x: k as f32 * 22.0, y: 0.0 }).collect();
            ops.push(Op::Glyphs {
                font: f,
                size: 36.0,
                glyphs,
                paint: Paint::Solid(rgba(1.0, 1.0, 1.0, 1.0)),
                transform: [1.0, 0.0, 0.0, 1.0, 60.0 + t * 10.0, 120.0 + line as f32 * 160.0],
            });
        }
    }
    DisplayList { width: 1920, height: 1080, background: rgba(0.0, 0.0, 0.0, 1.0), ops }
}

fn main() {
    let mut assets = Assets::default();
    let font = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").ok().map(|b| assets.fonts.add(Arc::new(b), 0));
    let assets = Arc::new(assets);
    let frames = 240;
    let threads = rayon::current_num_threads();
    println!("1080p, {frames} frames, {threads} threads, yuv kernel: {}", anikit_video::render::yuv::simd_level());
    for (name, shadows) in [("typical", false), ("typical + 30 shadowed cards (26 cached)", true)] {
        let lists: Vec<_> = (0..frames).map(|i| scene(i as f32 / 60.0, font, shadows)).collect();
        // Single thread: render, then render + YUV.
        let mut r = Renderer::new(assets.clone());
        let mut yuv = YuvFrame::default();
        for l in lists.iter().take(10) {
            r.render(l);
        }
        let t0 = Instant::now();
        for l in &lists {
            r.render(l);
        }
        let st = frames as f64 / t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        for l in &lists {
            r.render_yuv(l, &mut yuv);
        }
        let st_yuv = frames as f64 / t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        for _ in 0..frames {
            rgba_to_yuv420(r.frame(), &mut yuv);
        }
        let yuv_ms = t0.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        // Frame-parallel: one renderer per worker, contiguous chunks (keeps layer caches warm).
        let chunk = frames / threads;
        let run = || {
            lists.par_chunks(chunk).for_each(|c| {
                let mut r = Renderer::new(assets.clone());
                let mut yuv = YuvFrame::default();
                for l in c {
                    r.render_yuv(l, &mut yuv);
                }
            })
        };
        run();
        let t0 = Instant::now();
        run();
        let mt = frames as f64 / t0.elapsed().as_secs_f64();
        println!("{name}: {} ops | 1 thread: {st:.0} fps render, {st_yuv:.0} fps render+yuv | yuv alone {yuv_ms:.2} ms | {threads} threads: {mt:.0} fps render+yuv", lists[0].ops.len());
    }
}
