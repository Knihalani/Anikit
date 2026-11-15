//! cargo run --release -p anikit-core --example cloud_bench
use anikit_core::cloud::Cloud;
use anikit_core::space::{look_at, mul, perspective, Projector};
use anikit_core::splat::{Splat, SplatParams};
use std::time::Instant;

fn main() {
    for &n in &[50_000usize, 150_000] {
        let mut c = Cloud::new(n, 2, 3);
        for (i, v) in c.shape_mut(0).iter_mut().enumerate() {
            *v = ((i * 7919) % 600) as f32 - 300.0;
        }
        for (i, v) in c.shape_mut(1).iter_mut().enumerate() {
            *v = ((i * 104_729) % 500) as f32 - 250.0;
        }
        c.snap(0);
        let k = 200;
        let t = Instant::now();
        for _ in 0..k {
            c.step(1.0 / 60.0, 0, 1, 0.5);
        }
        let step = t.elapsed().as_secs_f64() * 1e3 / k as f64;
        let model = anikit_core::space::IDENTITY;
        let vp = mul(&perspective(40.0, 1.6, 10.0, 6000.0), &look_at([0.0, 0.0, 1000.0], [0.0; 3], [0.0, 1.0, 0.0]));
        let mut p = Projector::default();
        let t = Instant::now();
        for _ in 0..k {
            p.project(&c.pos, 3, [0, 1, 2], n, &model, &vp, 1440.0, 900.0, false);
        }
        let proj = t.elapsed().as_secs_f64() * 1e3 / k as f64;
        let mut s = Splat::new(1080, 675);
        let sp = SplatParams { size: 1.15, intensity: 0.3, fog_near: 700.0, fog_far: 2600.0, scale: 0.75, lens: [700.0, 400.0, 150.0, 46.0], max_radius: 5.0, tint: [1.0; 3] };
        let t = Instant::now();
        for _ in 0..k {
            s.draw(&p.out, n, &sp);
        }
        let draw = t.elapsed().as_secs_f64() * 1e3 / k as f64;
        // A real frame: splat, then tone-map + fade.
        let t = Instant::now();
        for _ in 0..k {
            s.draw(&p.out, n, &sp);
            s.resolve_fade(1.0, 0x07080b, 0.5);
        }
        let frame = t.elapsed().as_secs_f64() * 1e3 / k as f64;
        let res = frame - draw;
        println!("{n:>7} particles: step {step:.2} ms · project {proj:.2} ms · splat {draw:.2} ms · resolve 1080×675 {res:.2} ms");
    }
}
