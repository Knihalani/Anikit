//! cargo run --release -p anikit-core --example cloud_bench
//!
//! Particle pipeline per frame: cloud step, projection (with and without the
//! radix depth sort), additive splat, tone-map + trail fade; plus the 3D
//! emitter. Reports the median of several timed batches.
use anikit_core::cloud::Cloud;
use anikit_core::emitter::Emitter;
use anikit_core::space::{look_at, mul, perspective, Projector};
use anikit_core::splat::{Splat, SplatParams};
use std::time::Instant;

fn time(k: usize, mut f: impl FnMut()) -> f64 {
    for _ in 0..k / 4 {
        f();
    }
    let mut runs: Vec<f64> = (0..5)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..k {
                f();
            }
            t.elapsed().as_secs_f64() * 1e3 / k as f64
        })
        .collect();
    runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    runs[2]
}

fn main() {
    // (particles, buffer width, height, buffer pixels per projected unit)
    for &(n, w, h, scale) in &[(50_000usize, 1080usize, 675usize, 0.75f32), (150_000, 1080, 675, 0.75), (120_000, 1920, 1080, 1.0)] {
        let mut c = Cloud::new(n, 2, 3);
        for (i, v) in c.shape_mut(0).iter_mut().enumerate() {
            *v = ((i * 7919) % 600) as f32 - 300.0;
        }
        for (i, v) in c.shape_mut(1).iter_mut().enumerate() {
            *v = ((i * 104_729) % 500) as f32 - 250.0;
        }
        c.snap(0);
        let k = 40;
        let step = time(k, || c.step(1.0 / 60.0, 0, 1, 0.5));
        let model = anikit_core::space::IDENTITY;
        let vp = mul(&perspective(40.0, 1.6, 10.0, 6000.0), &look_at([0.0, 0.0, 1000.0], [0.0; 3], [0.0, 1.0, 0.0]));
        let (vw, vh) = (w as f64 / scale as f64, h as f64 / scale as f64);
        let mut p = Projector::default();
        let proj = time(k, || p.project(&c.pos, 3, [0, 1, 2], n, &model, &vp, vw, vh, false));
        let sort = time(k, || p.project(&c.pos, 3, [0, 1, 2], n, &model, &vp, vw, vh, true));
        let mut s = Splat::new(w, h);
        let sp = SplatParams { size: 1.15, intensity: 0.3, fog_near: 700.0, fog_far: 2600.0, scale, lens: [700.0, 400.0, 150.0, 46.0], max_radius: 5.0, tint: [1.0; 3] };
        let discs = p.out.chunks_exact(4).filter(|o| o[3] > 0.0 && sp.size * sp.scale * o[3] > 1.6).count();
        let draw = time(k, || s.draw(&p.out, n, &sp));
        // A real frame: splat, then tone-map + fade.
        let frame = time(k, || {
            s.draw(&p.out, n, &sp);
            s.resolve_fade(1.0, 0x07080b, 0.5);
        });
        let res = frame - draw;
        println!(
            "{n:>7} particles {w}×{h}: step {step:.2} ms · project {proj:.2} ms (+sort {:.2}) · splat {draw:.2} ms ({:.0}% discs) · resolve {res:.2} ms",
            sort - proj,
            discs as f64 * 100.0 / n as f64
        );
    }
    let mut e = Emitter::new(100_000, 7);
    e.params.rate = 0.0;
    e.params.life = 1e9;
    e.params.turbulence = 80.0;
    e.params.attract = 2000.0;
    e.burst(100_000);
    let em = time(100, || e.step(1.0 / 60.0));
    println!("emitter: 100000 particles (turbulence + attractor) step {em:.2} ms");
}
