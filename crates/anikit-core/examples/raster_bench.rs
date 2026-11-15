//! cargo run --release -p anikit-core --example raster_bench
//! Software 3D renderer at 1920×1080: per-pixel shading with and without the depth pre-pass.
use anikit_core::raster::*;
use anikit_core::space::{look_at, mul, perspective};
use std::time::Instant;

fn affine(s: [f32; 3], t: [f32; 3], a: f32, rgb: [f32; 3]) -> [f32; 15] {
    let (c, n) = (a.cos(), a.sin());
    [c * s[0], 0.0, -n * s[0], 0.0, s[1], 0.0, n * s[2], 0.0, c * s[2], t[0], t[1], t[2], rgb[0], rgb[1], rgb[2]]
}

fn main() {
    let (w, h) = (1920usize, 1080usize);
    let m = mul(&perspective(50.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([300.0, 250.0, 1350.0], [0.0, -50.0, -200.0], [0.0, 1.0, 0.0]));
    let vp: M4 = core::array::from_fn(|i| m[i] as f32);
    let eye = [300.0, 250.0, 1350.0];
    let light = Lighting { sky: [0.05, 0.07, 0.16], horizon: [0.9, 0.5, 0.95], ground: [0.01; 3], strips: [2.2, 2.3, 2.6], fog: [1800.0, 5200.0], ..Lighting::default() };
    let knot = torus_knot(2.0, 3.0, 300.0, 120.0, 58.0, 600, 28);
    let cube = cube();
    let towers: Vec<f32> = (0..900).flat_map(|i| { let (c, r) = (i % 30, i / 30); let hh = 40.0 + ((i * 7919) % 97) as f32 * 4.0; affine([80.0, hh, 80.0], [(c as f32 - 15.0) * 120.0, -500.0 + hh / 2.0, -(r as f32) * 120.0 + 600.0], 0.0, [0.04, 0.045, 0.06]) }).collect();
    let swarm: Vec<f32> = (0..6000).flat_map(|i| affine([60.0; 3], [((i * 7919) % 2000) as f32 - 1000.0, ((i * 104729) % 1200) as f32 - 600.0, ((i * 1299709) % 1600) as f32 - 800.0], i as f32, [0.9, 0.5, 0.3])).collect();
    let one = affine([1.0; 3], [0.0; 3], 0.4, [0.06, 0.06, 0.08]);
    let chrome = Material { metal: 0.9, iridescence: 0.25, specular: 1.4, shininess: 120.0, lines: 1.2, grid: [0.0, 6.0], pulse: 2.5, pulse_count: 3.0, ..Material::default() };
    let edges = Material { lines: 1.6, grid: [1.0, 1.0], line_width: 0.035, pulse: 1.5, pulse_dir: [0.0, 0.0, 0.0015], ..Material::default() };
    let plain = Material::default();
    let metal = Material { metal: 0.9, ..Material::default() };
    let irid = Material { iridescence: 0.5, ..Material::default() };
    let lines = Material { lines: 1.2, grid: [0.0, 6.0], ..Material::default() };
    let pulse = Material { pulse: 1.2, ..Material::default() };
    let scenes: [(&str, &Mesh, &[f32], &Material); 8] = [("knot", &knot, &one, &chrome), ("knot plain", &knot, &one, &plain), ("knot metal", &knot, &one, &metal), ("knot irid", &knot, &one, &irid), ("knot lines", &knot, &one, &lines), ("knot pulse", &knot, &one, &pulse), ("900 towers", &cube, &towers, &edges), ("6000 cubes", &cube, &swarm, &plain)];
    let mut r = Raster::new(w, h);
    {
        let m2 = mul(&perspective(50.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([300.0, 250.0, 1450.0], [0.0, -50.0, -100.0], [0.0, 1.0, 0.0]));
        let prev: M4 = core::array::from_fn(|i| m2[i] as f32);
        r.clear(0, 255);
        r.draw(&cube, &towers, &vp, eye, &Material::default(), &light, true);
        let t = Instant::now();
        for _ in 0..5 { r.motion_blur(&vp, &prev, 0.5, 115.0); }
        println!("motion blur {:.1} ms", t.elapsed().as_secs_f64() * 1e3 / 5.0);
    }
    let t = Instant::now();
    for _ in 0..50 { r.clear(0x04050a, 255); }
    println!("clear {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / 50.0);
    // Vertex stage only: everything behind the camera.
    let behind = affine([1.0; 3], [0.0, 0.0, 5000.0], 0.0, [1.0; 3]);
    let t = Instant::now();
    for _ in 0..50 { r.draw(&knot, &behind, &vp, eye, &plain, &light, true); }
    {
        let big = affine([4000.0, 4000.0, 1.0], [0.0, -50.0, -200.0], 0.0, [0.5; 3]);
        let mut fq = Raster::new(w, h);
        let m2 = mul(&perspective(50.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([0.0, -50.0, 1000.0], [0.0, -50.0, -200.0], [0.0, 1.0, 0.0]));
        let vp2: M4 = core::array::from_fn(|i| m2[i] as f32);
        for (nm, mt) in [("plain", Material::default()), ("chrome", chrome)] {
            let t = Instant::now();
            for _ in 0..20 { fq.draw(&cube, &big, &vp2, [0.0, -50.0, 1000.0], &mt, &light, true); fq.clear(0, 255); }
            println!("fullscreen {nm}: {:.2} ms, {:.1} ns/px", t.elapsed().as_secs_f64() * 1e3 / 20.0, t.elapsed().as_secs_f64() * 1e9 / 20.0 / fq.shaded.max(1) as f64);
        }
    }
    println!("knot vertex stage {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / 50.0);
    for (name, mesh, inst, mat) in scenes {
        let mut res = [0.0f64; 2];
        let mut shaded = [0usize; 2];
        for round in 0..6 {
            for (k, nopre) in [false, true].into_iter().enumerate() {
                r.prepass = !nopre;
                let t = Instant::now();
                for _ in 0..3 {
                    r.clear(0x04050a, 255);
                    r.draw(mesh, inst, &vp, eye, mat, &light, true);
                }
                if round > 0 {
                    res[k] += t.elapsed().as_secs_f64() * 1e3 / 15.0;
                }
                shaded[k] = r.shaded;
            }
        }
        r.prepass = false; r.clear(0, 255); r.draw(mesh, inst, &vp, eye, mat, &light, true);
        println!("{name:12} pre-pass {:6.1} ms ({} px shaded)   no pre-pass {:6.1} ms ({} px shaded)", res[0], shaded[0], res[1], shaded[1]);
    }
}
