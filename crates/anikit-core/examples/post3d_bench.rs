//! Timing for the 3D post passes at 1080p: camera motion blur and depth of field.
//!   cargo run --release -p anikit-core --example post3d_bench
use anikit_core::raster::*;
use anikit_core::space::{look_at, mul, perspective};
use std::time::Instant;
fn main() {
    let (w, h) = (1920usize, 1080usize);
    let m = mul(&perspective(60.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([0.0, 150.0, 1350.0], [0.0, 100.0, -2000.0], [0.0, 1.0, 0.0]));
    let vp: M4 = core::array::from_fn(|i| m[i] as f32);
    let m2 = mul(&perspective(60.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([10.0, 150.0, 1450.0], [0.0, 100.0, -1900.0], [0.0, 1.0, 0.0]));
    let prev: M4 = core::array::from_fn(|i| m2[i] as f32);
    let mut inst = Vec::new();
    for i in 0..900usize {
        let (c, r) = (i % 30, i / 30);
        let hh = 40.0 + ((i * 7919) % 97) as f32 * 4.0;
        inst.extend_from_slice(&[80.0, 0.0, 0.0, 0.0, hh, 0.0, 0.0, 0.0, 80.0, (c as f32 - 15.0) * 120.0, -500.0 + hh / 2.0, -(r as f32) * 120.0 + 600.0, 0.5, 0.5, 0.6]);
    }
    let mut r = Raster::new(w, h);
    r.clear(0, 255);
    r.draw(&cube(), &inst, &vp, [0.0, 150.0, 1350.0], &Material::default(), &Lighting::default(), true);
    let c0 = r.color.clone();
    let time = |label: &str, r: &mut Raster, f: &dyn Fn(&mut Raster)| {
        for _ in 0..3 {
            r.color.copy_from_slice(&c0);
            f(r);
        }
        let n = 20;
        let t = Instant::now();
        for _ in 0..n {
            r.color.copy_from_slice(&c0);
            f(r);
        }
        let mut sum = 0u64;
        for &c in r.color.iter().step_by(97) {
            sum = sum.wrapping_add(c as u64);
        }
        println!("{label:<16} {:6.2} ms  (checksum {sum})", t.elapsed().as_secs_f64() * 1000.0 / n as f64);
    };
    time("motion blur", &mut r, &|r| r.motion_blur(&vp, &prev, 0.55, 110.0));
    time("depth of field", &mut r, &|r| r.depth_of_field(&vp, 1500.0, 12.0, 60.0));
}
