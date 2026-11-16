//! Profiling target for the film grade (valgrind --tool=callgrind): the VELOCITY grade on a frame.
//!   cargo run --release -p anikit-core --features std --example post_prof [frame.rgba]
use anikit_core::video::{Post, PostFx};
fn main() {
    let (w, h) = (1920usize, 1080usize);
    let img: Vec<u8> = match std::env::args().nth(1).map(std::fs::read) {
        Some(Ok(b)) if b.len() == w * h * 4 => b,
        _ => (0..w * h * 4).map(|i| ((i as u32).wrapping_mul(2654435761) >> 24) as u8).collect(),
    };
    let p = Post { aa: 1.0, bloom: 0.85, bloom_threshold: 0.5, bloom_radius: 46.0, streaks: 0.45, streak_length: 300.0, rays: 0.9, zoom_blur: 0.1, aberration: 0.5, vignette: 0.42, grain: 3.5, contrast: 1.07, saturation: 1.12, flares: 0.6, shadows: [0.0, 0.012, 0.035], highlights: [0.05, 0.015, -0.02], ..Default::default() };
    let mut fx = PostFx::default();
    let mut buf = img.clone();
    for _ in 0..2 {
        buf.copy_from_slice(&img);
        fx.apply(&mut buf, w, h, &p);
    }
    println!("{}", buf[1000]);
}
