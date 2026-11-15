//! `cargo run --release -p anikit-core --example physics_bench`
use anikit_core::world::{World, WorldConfig};
use std::time::Instant;

fn main() {
    for &(n, subs) in &[(1_000usize, 4u32), (5_000, 4), (10_000, 2), (20_000, 2), (50_000, 1)] {
        let side = 1600.0;
        let mut w = World::new(WorldConfig { bounds: [0.0, 0.0, side, side], substeps: subs, ..Default::default() });
        let cols = (n as f32).sqrt().ceil() as usize;
        let sp = side / cols as f32;
        for i in 0..n {
            let r = sp * 0.3 * (0.8 + 0.4 * ((i * 7919) % 100) as f32 / 100.0);
            w.add_body((i % cols) as f32 * sp + sp / 2.0, (i / cols) as f32 * sp + sp / 2.0, r, 1.0);
        }
        for _ in 0..120 {
            w.step(1.0 / 60.0); // settle into a pile (worst case: many contacts)
        }
        let frames = 60;
        let t = Instant::now();
        for _ in 0..frames {
            w.step(1.0 / 60.0);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        println!("{n:>6} bodies, {subs} substeps: {ms:>7.3} ms/frame  ({:.0} pair tests/frame)", w.pair_tests);
    }
}
