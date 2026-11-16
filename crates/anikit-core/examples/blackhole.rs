//! cargo run --release -p anikit-core --example blackhole [out_dir]
//!
//! Renders reference shots of the black-hole renderer as PPM files (when an
//! output directory is given) and times the per-frame and camera-change paths.
use anikit_core::blackhole::{BlackHole, Params};
use std::time::Instant;

fn save(dir: &str, name: &str, bh: &BlackHole) {
    let mut v = format!("P6\n{} {}\n255\n", bh.w, bh.h).into_bytes();
    for p in bh.out.chunks(4) {
        v.extend_from_slice(&p[..3]);
    }
    std::fs::write(format!("{dir}/{name}.ppm"), v).unwrap();
}

fn main() {
    let dir = std::env::args().nth(1);
    let (w, h) = (1280, 800);
    let mut bh = BlackHole::new(w, h, 1);
    let shots: Vec<(&str, Params)> = vec![
        ("edge-30m", Params { distance: 30.0, inclination: 84.0, fov: 34.0, time: 3.0, ..Default::default() }),
        ("edge-30m-cinematic", Params { distance: 30.0, inclination: 84.0, fov: 34.0, time: 3.0, mode: 1, temperature: 6500.0, ..Default::default() }),
        ("eht-17", Params { distance: 40.0, inclination: 17.0, fov: 30.0, time: 3.0, ..Default::default() }),
        ("near-10m", Params { distance: 10.0, inclination: 80.0, fov: 70.0, time: 3.0, ..Default::default() }),
        ("close-4m", Params { distance: 4.0, inclination: 80.0, fov: 90.0, time: 3.0, fall: 1.0, ..Default::default() }),
        ("horizon-2.2m", Params { distance: 2.2, inclination: 80.0, fov: 100.0, time: 3.0, fall: 1.0, ..Default::default() }),
        ("inside-1.2m-up", Params { distance: 1.2, inclination: 80.0, fov: 100.0, time: 3.0, fall: 1.0, look: 180.0, ..Default::default() }),
        ("inside-1.5m-side", Params { distance: 1.5, inclination: 80.0, fov: 110.0, time: 3.0, fall: 1.0, look: 120.0, ..Default::default() }),
    ];
    if let Some(d) = &dir {
        std::fs::create_dir_all(d).unwrap();
        for (name, p) in &shots {
            bh.render(p);
            bh.render(&Params { time: p.time + 0.01, ..*p });
            save(d, name, &bh);
            println!("{name}: {:?}", bh.stats);
        }
    }
    // Timing: steady-state frames (time animating, camera fixed).
    for (name, p) in [&shots[0], &shots[1], &shots[3]] {
        let mut p = *p;
        bh.render(&p);
        for _ in 0..10 {
            p.time += 1.0 / 60.0;
            bh.render(&p);
        }
        let n = 60;
        let t = Instant::now();
        for _ in 0..n {
            p.time += 1.0 / 60.0;
            p.azimuth += 0.1;
            bh.render(&p);
        }
        let frame = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let t = Instant::now();
        for i in 0..10 {
            p.distance = 30.0 - i as f32 * 0.1;
            bh.render(&p);
        }
        let moving = t.elapsed().as_secs_f64() * 1000.0 / 10.0;
        #[cfg(feature = "std")]
        println!("  stages {:?}", bh.profile(&p));
        println!("{name} {w}x{h}: {frame:.2} ms/frame, {moving:.2} ms/frame while the camera moves");
    }
}
