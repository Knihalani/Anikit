//! Particle throughput: simulation (per frame, 2 substeps) and splatting at 1080p.
//!
//! `cargo run --release -p anikit-video --example particles_bench [-- 1000000 10000000]`

use anikit_video::pipeline::textures::CompTextures;
use anikit_video::scene3d::math::v3;
use anikit_video::scene3d::particles::{self, Sim, View};
use anikit_video::scene3d::trace::Camera;
use std::collections::HashMap;
use std::time::Instant;

fn main() {
    let counts: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let counts = if counts.is_empty() { vec![1_000_000, 4_000_000, 10_000_000] } else { counts };
    let (w, h) = (1920usize, 1080usize);
    let cam = Camera { pos: v3(0.0, 1.4, 6.5), target: v3(0.0, 1.0, 0.0), fov: 30.0, focus: 6.5, aperture: 6.0, roll: 0.0, shutter: 0.5 };
    let depth = vec![1e9f32; w * h];
    let cache = CompTextures::new(1 << 20);
    for (blend, n) in counts.iter().flat_map(|n| [("add", *n), ("over", *n)]) {
        let spec = serde_json::json!({
            "type": "particles", "count": n,
            "emit": { "shape": "ball", "radius": 1.2, "pos": [0, 1.2, 0] },
            "flow": { "strength": 0.8, "scale": 0.8 }, "attractors": [{ "pos": [0, 1.2, 0], "strength": 0.6, "swirl": 0.8 }],
            "drag": 0.3, "size": 0.003, "color": ["#ffffff", "#7fd4ff"], "colorBy": "speed", "blend": blend
        });
        let mut shapes = anikit_video::scene3d::spec::Shapes::new();
        let def = particles::parse(&spec, &mut shapes, &mut Vec::new(), &HashMap::new(), &[], "bench").unwrap();
        let t0 = Instant::now();
        let sim = Sim::new(&def, 30.0, Some(0.0), None, &cache).unwrap();
        let _ = sim.at(0.0);
        let init = t0.elapsed().as_secs_f64();
        // Warm, then time 20 frames of simulation.
        let _ = sim.at(5.0 / 30.0);
        let t1 = Instant::now();
        let frames = 20;
        let mut snap = sim.at(5.0 / 30.0);
        for f in 6..6 + frames {
            snap = sim.at(f as f32 / 30.0);
        }
        let sim_ms = t1.elapsed().as_secs_f64() * 1e3 / frames as f64;
        let view = View { cam: &cam, prev: &cam, w, h, depth: &depth, focus: cam.focus, aperture: cam.aperture, shutter: cam.shutter, key: (v3(0.3, 0.9, 0.3), v3(3.0, 3.0, 3.0)), amb: (v3(0.3, 0.3, 0.3), v3(0.1, 0.1, 0.1)) };
        let t = (6 + frames) as f32 / 30.0;
        let _ = particles::render(&[(&sim, snap.clone())], &view, t);
        let t2 = Instant::now();
        let reps = 5;
        for _ in 0..reps {
            let _ = particles::render(&[(&sim, snap.clone())], &view, t);
        }
        let splat_ms = t2.elapsed().as_secs_f64() * 1e3 / reps as f64;
        println!(
            "{n:>9} particles ({blend:>4}): init {:.0} ms, sim {:.1} ms/frame ({:.1} M particle-frames/s), splat 1080p {:.1} ms ({:.1} M/s)",
            init * 1e3,
            sim_ms,
            n as f64 / sim_ms / 1e3,
            splat_ms,
            n as f64 / splat_ms / 1e3
        );
    }
}
