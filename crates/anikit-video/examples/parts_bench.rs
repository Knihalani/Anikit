//! Per-op-kind cost at 1080p (single thread). `cargo run --release -p anikit-video --example parts_bench`
use std::sync::Arc;
use std::time::Instant;

use anikit_video::dl::*;
use anikit_video::render::{Assets, Renderer};
use kurbo::Shape;

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba([r, g, b, a])
}

fn time(name: &str, r: &mut Renderer, ops: Vec<Op>) {
    let l = DisplayList { width: 1920, height: 1080, background: rgba(0.0, 0.0, 0.0, 1.0), ops };
    for _ in 0..5 {
        r.render(&l);
    }
    let n = 60;
    let t0 = Instant::now();
    for _ in 0..n {
        r.render(&l);
    }
    println!("{name:40} {:7.3} ms", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
}

fn main() {
    let mut assets = Assets::default();
    let font = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").ok().map(|b| assets.fonts.add(Arc::new(b), 0));
    let img: Vec<u8> = (0..800 * 600 * 4).map(|i| if i % 4 == 3 { 255 } else { (i * 7 % 256) as u8 }).collect();
    assets.images.insert_rgba8(ImageId(1), 800, 600, img).unwrap();
    let mut r = Renderer::new(Arc::new(assets));
    let full = kurbo::Rect::new(0.0, 0.0, 1920.0, 1080.0).to_path(0.1);
    time("empty (clear only)", &mut r, vec![]);
    time("full-frame solid", &mut r, vec![Op::Fill { path: full.clone(), paint: Paint::Solid(rgba(0.2, 0.3, 0.4, 1.0)), rule: FillRule::NonZero, transform: IDENTITY }]);
    time("full-frame linear gradient", &mut r, vec![Op::Fill {
        path: full.clone(),
        paint: Paint::Linear { p0: [0.0, 0.0], p1: [1920.0, 1080.0], stops: vec![(0.0, rgba(0.1, 0.1, 0.2, 1.0)), (1.0, rgba(0.3, 0.1, 0.4, 1.0))] },
        rule: FillRule::NonZero,
        transform: IDENTITY,
    }]);
    time("full-frame translucent solid", &mut r, vec![Op::Fill { path: full.clone(), paint: Paint::Solid(rgba(0.2, 0.3, 0.4, 0.5)), rule: FillRule::NonZero, transform: IDENTITY }]);
    time("30 cards 280x170", &mut r, (0..30).map(|i| Op::RoundRect { x: (i % 6) as f32 * 310.0 + 40.0, y: (i / 6) as f32 * 205.0 + 40.0, w: 280.0, h: 170.0, radius: 16.0, paint: Paint::Solid(rgba(0.95, 0.95, 0.97, 0.9)), transform: IDENTITY }).collect());
    time("400 small circles", &mut r, (0..400).map(|i| Op::Fill { path: kurbo::Circle::new(((i % 40) as f64 * 47.0 + 20.0, (i / 40) as f64 * 100.0 + 60.0), 6.0).to_path(0.1), paint: Paint::Solid(rgba(0.5, 0.6, 1.0, 0.8)), rule: FillRule::NonZero, transform: IDENTITY }).collect());
    time("image 800x600 scaled to 1200x900", &mut r, vec![Op::Image { image: ImageId(1), dst: [100.0, 100.0, 1200.0, 900.0], opacity: 1.0, transform: IDENTITY }]);
    time("image 800x600 1:1", &mut r, vec![Op::Image { image: ImageId(1), dst: [100.0, 100.0, 800.0, 600.0], opacity: 1.0, transform: IDENTITY }]);
    if let Some(f) = font {
        let text = |size: f32, lines: usize| -> Vec<Op> {
            (0..lines).map(|line| Op::Glyphs {
                font: f,
                size,
                glyphs: (0..60).map(|k| Glyph { id: 36 + ((k * 7 + line) % 50) as u16, x: k as f32 * size * 0.6, y: 0.0 }).collect(),
                paint: Paint::Solid(rgba(1.0, 1.0, 1.0, 1.0)),
                transform: [1.0, 0.0, 0.0, 1.0, 40.0, 60.0 + line as f32 * size * 1.3],
            }).collect()
        };
        time("text 1500 glyphs @ 24px", &mut r, text(24.0, 25));
        time("text 360 glyphs @ 96px", &mut r, text(96.0, 6));
    }
    time("blur layer 400x300 sigma 8", &mut r, vec![
        Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Blur(8.0)], clip: None, cache_key: None },
        Op::RoundRect { x: 100.0, y: 100.0, w: 400.0, h: 300.0, radius: 20.0, paint: Paint::Solid(rgba(1.0, 1.0, 1.0, 1.0)), transform: IDENTITY },
        Op::PopLayer,
    ]);
    time("full-frame blur sigma 20", &mut r, vec![
        Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Blur(20.0)], clip: None, cache_key: None },
        Op::Fill { path: full.clone(), paint: Paint::Solid(rgba(0.2, 0.3, 0.4, 1.0)), rule: FillRule::NonZero, transform: IDENTITY },
        Op::PopLayer,
    ]);
    time("opacity layer full frame", &mut r, vec![
        Op::PushLayer { opacity: 0.5, blend: Blend::Normal, filters: vec![], clip: None, cache_key: None },
        Op::Fill { path: full, paint: Paint::Solid(rgba(0.2, 0.3, 0.4, 1.0)), rule: FillRule::NonZero, transform: IDENTITY },
        Op::PopLayer,
    ]);
}
