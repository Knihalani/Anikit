//! Engine throughput benchmark: `cargo run --release -p anikit-core --example bench`
use anikit_core::color::ColorSpace;
use anikit_core::engine::{flags, Engine, TrackKind, TrackSpec};
use anikit_core::morph;
use anikit_core::path::Path;
use anikit_core::{SpringParams, Timing};
use std::time::Instant;

fn bench(name: &str, n: usize, mut setup: impl FnMut(&mut Engine), frames: usize) {
    let mut e = Engine::new();
    setup(&mut e);
    e.set_lag_smoothing(0.0, 0.0);
    // warm up
    for _ in 0..5 {
        e.tick(1.0);
    }
    let t = Instant::now();
    for _ in 0..frames {
        e.tick(1.0);
    }
    let el = t.elapsed().as_secs_f64();
    let per_frame = el / frames as f64;
    println!(
        "{name:<44} {n:>7} items  {:>8.3} ms/frame  {:>6.1} ns/item",
        per_frame * 1e3,
        per_frame * 1e9 / n as f64
    );
}

fn main() {
    let long = Timing { duration: 1e9, ..Default::default() };
    for n in [1_000usize, 10_000] {
        bench("tween (cache-resident sizes)", n, |e| {
            let ease = e.easing("outCubic").unwrap();
            for i in 0..n {
                let s = TrackSpec::number(vec![0.0, i as f64], 1).with_easings(vec![ease]);
                let h = e.create_anim(long, &[s], flags::DEFAULT).unwrap();
                e.play(h).unwrap();
            }
        }, 2000);
    }
    bench("tween: 1 number track, outCubic", 100_000, |e| {
        let ease = e.easing("outCubic").unwrap();
        for i in 0..100_000 {
            let s = TrackSpec::number(vec![0.0, i as f64], 1).with_easings(vec![ease]);
            let h = e.create_anim(long, &[s], flags::DEFAULT).unwrap();
            e.play(h).unwrap();
        }
    }, 200);
    bench("tween: x,y,opacity + OKLab colour", 50_000, |e| {
        let ease = e.easing("inOutQuad").unwrap();
        for i in 0..50_000 {
            let mut c = TrackSpec::number(vec![255.0, 0.0, 0.0, 1.0, 0.0, 0.0, 255.0, 1.0], 4);
            c.kind = TrackKind::Color(ColorSpace::Oklab);
            let specs = [
                TrackSpec::number(vec![0.0, 0.0, i as f64, 100.0], 2).with_easings(vec![ease]),
                TrackSpec::number(vec![0.0, 1.0], 1).with_easings(vec![ease]),
                c,
            ];
            let h = e.create_anim(long, &specs, flags::DEFAULT).unwrap();
            e.play(h).unwrap();
        }
    }, 100);
    bench("keyframes: 8 kf, spring easing", 50_000, |e| {
        let ease = e.easing("spring(200, 12)").unwrap();
        for _ in 0..50_000 {
            let s = TrackSpec::number((0..8).map(|k| k as f64 * 10.0).collect(), 1).with_easings(vec![ease]);
            let h = e.create_anim(long, &[s], flags::DEFAULT).unwrap();
            e.play(h).unwrap();
        }
    }, 100);
    bench("timeline: 1 group x 50k children", 50_000, |e| {
        let g = e.create_group(Timing::default(), true, flags::DEFAULT).unwrap();
        for i in 0..50_000 {
            let s = TrackSpec::number(vec![0.0, 1.0], 1);
            let h = e.create_anim(long, &[s], flags::DEFAULT).unwrap();
            e.group_add(g, h, (i % 100) as f64).unwrap();
        }
        e.play(g).unwrap();
    }, 100);
    bench("springs: 2 channels (shared config)", 100_000, |e| {
        for i in 0..100_000 {
            e.create_spring(SpringParams::new(0.01, 0.0, 1.0), &[0.0, 0.0], &[], &[i as f64, 1e6]).unwrap();
        }
    }, 200);
    let a = Path::parse("M0 0 H100 V100 H0 Z");
    let b = Path::star(50.0, 50.0, 24, 50.0, 25.0, 0.0);
    let plan = morph::prepare(&a, &b);
    let ch = plan.channels();
    bench(&format!("morph: {ch}-channel path"), 2_000, |e| {
        for _ in 0..2_000 {
            let mut v = plan.from.clone();
            v.extend_from_slice(&plan.to);
            let h = e.create_anim(long, &[TrackSpec::number(v, ch)], flags::DEFAULT).unwrap();
            e.play(h).unwrap();
        }
    }, 100);
}
