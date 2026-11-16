//! cargo run --release -p anikit-core --features std --example post_bench [frame.rgba]
//! Film grade cost per effect at 1920×1080: on a raw 1920×1080 RGBA frame if
//! given (ffmpeg -i film.mp4 -frames:v 1 -f rawvideo -pix_fmt rgba frame.rgba),
//! else on noise (a worst case for the edge-dependent passes).
use anikit_core::video::{Post, PostFx};
use std::time::Instant;
fn main() {
    let (w, h) = (1920usize, 1080usize);
    let img: Vec<u8> = match std::env::args().nth(1).map(std::fs::read) {
        Some(Ok(b)) if b.len() == w * h * 4 => b,
        _ => (0..w * h * 4).map(|i| ((i as u32).wrapping_mul(2654435761) >> 24) as u8).collect(),
    };
    let mut fx = PostFx::default();
    let d = Post::default();
    let cases: [(&str, Post); 11] = [
        ("bloom", Post { bloom: 0.8, ..d }),
        ("streaks", Post { streaks: 0.6, ..d }),
        ("bloom+streak", Post { bloom: 0.8, streaks: 0.6, ..d }),
        ("aberration", Post { aberration: 0.6, ..d }),
        ("zoom blur", Post { zoom_blur: 0.1, ..d }),
        ("grade", Post { contrast: 1.1, saturation: 1.1, shadows: [0.0, 0.02, 0.04], ..d }),
        ("vignette+grain", Post { vignette: 0.4, grain: 4.0, ..d }),
        ("rays", Post { rays: 0.9, ..d }),
        ("aa", Post { aa: 1.0, ..d }),
        ("film (VELOCITY)", Post { aa: 1.0, bloom: 0.85, bloom_threshold: 0.5, bloom_radius: 46.0, streaks: 0.45, streak_length: 300.0, rays: 0.9, zoom_blur: 0.1, aberration: 0.5, vignette: 0.42, grain: 3.5, contrast: 1.07, saturation: 1.12, shadows: [0.0, 0.012, 0.035], highlights: [0.05, 0.015, -0.02], ..d }),
        ("all", Post { bloom: 0.8, streaks: 0.6, aberration: 0.6, zoom_blur: 0.1, contrast: 1.1, saturation: 1.1, vignette: 0.4, grain: 4.0, ..d }),
    ];
    let mut buf = img.clone();
    for (name, p) in cases {
        for _ in 0..2 {
            buf.copy_from_slice(&img);
            fx.apply(&mut buf, w, h, &p);
        }
        let n = 8;
        let mut tot = 0.0;
        for _ in 0..n {
            buf.copy_from_slice(&img);
            let t = Instant::now();
            fx.apply(&mut buf, w, h, &p);
            tot += t.elapsed().as_secs_f64();
        }
        println!("{name:16} {:6.1} ms", tot * 1e3 / n as f64);
    }
}
