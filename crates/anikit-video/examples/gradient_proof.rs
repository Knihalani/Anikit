//! Proof stills for dithered gradients: a dark card gradient drawn over content,
//! by vello (8-bit) and by the float + dither path, plus ×8 contrast crops.
//! `cargo run --release -p anikit-video --example gradient_proof -- out_dir`
use anikit_video::dl::*;
use anikit_video::render::{Assets, Quality, Renderer};
use std::sync::Arc;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "gradient-proof".into());
    std::fs::create_dir_all(&out).unwrap();
    let (w, h) = (1920u32, 1080u32);
    let l = DisplayList {
        width: w,
        height: h,
        background: Rgba([0.03, 0.035, 0.05, 1.0]),
        ops: vec![
            Op::RoundRect { x: 80.0, y: 80.0, w: 200.0, h: 120.0, radius: 16.0, paint: Paint::Solid(Rgba([0.9, 0.9, 0.95, 1.0])), transform: IDENTITY },
            Op::RoundRect {
                x: 160.0,
                y: 240.0,
                w: 1600.0,
                h: 760.0,
                radius: 40.0,
                paint: Paint::Radial { c: [800.0, 200.0], r: 1100.0, stops: vec![(0.0, Rgba([0.16, 0.12, 0.26, 1.0])), (1.0, Rgba([0.04, 0.04, 0.07, 1.0]))] },
                transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            },
        ],
    };
    let assets = Arc::new(Assets::default());
    for (name, q) in [("vello", Quality::Reference), ("dithered", Quality::Standard)] {
        let mut r = Renderer::new(assets.clone());
        r.set_quality(q);
        let px = r.render(&l).data_as_u8_slice().to_vec();
        image::save_buffer(format!("{out}/gradient-{name}.png"), &px, w, h, image::ExtendedColorType::Rgba8).unwrap();
        // A 480×270 crop, contrast ×8 around its mean, so 1-level bands show.
        let (cx, cy, cw, ch) = (900usize, 500usize, 480usize, 270usize);
        let mut crop = vec![0u8; cw * ch * 4];
        let mean: f64 = (0..ch).flat_map(|y| (0..cw).map(move |x| (y, x))).map(|(y, x)| px[((cy + y) * w as usize + cx + x) * 4 + 2] as f64).sum::<f64>() / (cw * ch) as f64;
        for y in 0..ch {
            for x in 0..cw {
                let s = ((cy + y) * w as usize + cx + x) * 4;
                for k in 0..3 {
                    crop[(y * cw + x) * 4 + k] = (128.0 + (px[s + k] as f64 - mean) * 8.0).clamp(0.0, 255.0) as u8;
                }
                crop[(y * cw + x) * 4 + 3] = 255;
            }
        }
        image::save_buffer(format!("{out}/gradient-{name}-x8.png"), &crop, cw as u32, ch as u32, image::ExtendedColorType::Rgba8).unwrap();
    }
    println!("wrote {out}/gradient-{{vello,dithered}}{{,-x8}}.png");
}
