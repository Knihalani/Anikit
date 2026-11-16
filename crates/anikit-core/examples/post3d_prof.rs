//! Profiling target for depth of field + camera motion blur (valgrind --tool=callgrind).
use anikit_core::raster::*;
use anikit_core::space::{look_at, mul, perspective};
fn main() {
    let (w, h) = (1920usize, 1080usize);
    let m = mul(&perspective(50.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([300.0, 250.0, 1350.0], [0.0, -50.0, -200.0], [0.0, 1.0, 0.0]));
    let vp: M4 = core::array::from_fn(|i| m[i] as f32);
    let m2 = mul(&perspective(50.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([300.0, 250.0, 1450.0], [0.0, -50.0, -100.0], [0.0, 1.0, 0.0]));
    let prev: M4 = core::array::from_fn(|i| m2[i] as f32);
    let mut inst = Vec::new();
    for i in 0..900usize {
        let (c, r) = (i % 30, i / 30);
        let hh = 40.0 + ((i * 7919) % 97) as f32 * 4.0;
        inst.extend_from_slice(&[80.0, 0.0, 0.0, 0.0, hh, 0.0, 0.0, 0.0, 80.0, (c as f32 - 15.0) * 120.0, -500.0 + hh / 2.0, -(r as f32) * 120.0 + 600.0, 0.5, 0.5, 0.6]);
    }
    let mut r = Raster::new(w, h);
    r.clear(0, 255);
    r.draw(&cube(), &inst, &vp, [300.0, 250.0, 1350.0], &Material::default(), &Lighting::default(), true);
    let c0 = r.color.clone();
    for _ in 0..2 {
        r.color.copy_from_slice(&c0);
        r.depth_of_field(&vp, 1500.0, 14.0, 60.0);
        r.color.copy_from_slice(&c0);
        r.motion_blur(&vp, &prev, 0.6, 115.0);
    }
    println!("{}", r.color[1000]);
}
