//! Line-level profiling target: valgrind --tool=callgrind (knot scene, 3 frames).
use anikit_core::raster::*;
use anikit_core::space::{look_at, mul, perspective};
fn main() {
    let (w, h) = (1920usize, 1080usize);
    let m = mul(&perspective(50.0, w as f64 / h as f64, 8.0, 40000.0), &look_at([300.0, 250.0, 1350.0], [0.0, -50.0, -200.0], [0.0, 1.0, 0.0]));
    let vp: M4 = core::array::from_fn(|i| m[i] as f32);
    let knot = torus_knot(2.0, 3.0, 300.0, 120.0, 58.0, 600, 28);
    let one = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.06, 0.06, 0.08];
    let mat = Material { metal: 0.9, iridescence: 0.25, lines: 1.2, grid: [0.0, 6.0], pulse: 2.5, ..Material::default() };
    let mut r = Raster::new(w, h);
    for _ in 0..3 {
        r.clear(0, 255);
        r.draw(&knot, &one, &vp, [300.0, 250.0, 1350.0], &mat, &Lighting::default(), true);
    }
    println!("{}", r.shaded);
}
