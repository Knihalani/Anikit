//! scene3d post-stack benchmark on dumped primary passes.
//!
//! Dump: `ANIKIT_SCENE3D_DUMP=target/cyc.px anikit-video still … ` (or the
//! scene3d_bench), then
//! `ANIKIT_SCENE3D_PROFILE=1 cargo run --release -p anikit-video --example post_bench -- target/cyc.px [runs]`.
use anikit_video::scene3d::post;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let runs: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    let (px, w, h, p) = post::load(std::path::Path::new(&a[1])).expect("can't read the dump");
    let mut best = f64::MAX;
    let mut out = Vec::new();
    for _ in 0..runs {
        let c = Instant::now();
        out = post::process_planes(&px, w, h, &p, None, None);
        best = best.min(c.elapsed().as_secs_f64() * 1e3);
    }
    println!("post {w}x{h}: best {best:.1} ms of {runs}");
    if let Some(o) = a.get(3) {
        image::RgbaImage::from_raw(w as u32, h as u32, out).unwrap().save(o).unwrap();
    }
}
