//! Profiling target: 12000 instanced cones (a forest) seen from a low camera.
//!   cargo run --release -p anikit-core --example trees_prof [frames]
//!   valgrind --tool=callgrind target/release/examples/trees_prof 1
use anikit_core::raster::*;
use anikit_core::space::{look_at, mul, perspective};
fn main() {
    let (w, h) = (1920usize, 1080usize);
    let eye = [-6000.0f64, 500.0, 8000.0];
    let m = mul(&perspective(55.0, w as f64 / h as f64, 5.0, 400000.0), &look_at(eye, [-4500.0, 120.0, 4000.0], [0.0, 1.0, 0.0]));
    let vp: M4 = core::array::from_fn(|i| m[i] as f32);
    let sides = 24;
    let (mut pos, mut nrm, mut idx) = (vec![0.0f32, 1.0, 0.0], vec![0.0f32, 1.0, 0.0], vec![]);
    for s in 0..=sides {
        let a = s as f32 / sides as f32 * core::f32::consts::TAU;
        pos.extend_from_slice(&[a.cos() * 0.35, 0.0, a.sin() * 0.35, a.cos() * 0.35, 0.0, a.sin() * 0.35]);
        nrm.extend_from_slice(&[a.cos(), 0.35, a.sin(), 0.0, -1.0, 0.0]);
    }
    let base = (pos.len() / 3) as u32;
    pos.extend_from_slice(&[0.0, 0.0, 0.0]);
    nrm.extend_from_slice(&[0.0, -1.0, 0.0]);
    for s in 0..sides as u32 {
        let a = 1 + s * 2;
        idx.extend_from_slice(&[0, a + 2, a, base, a + 1, a + 3]);
    }
    let cone = Mesh { uv: vec![0.0; pos.len() / 3 * 2], pos, nrm, idx };
    let mut inst = vec![];
    let mut seed = 12345u32;
    let mut rnd = || {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        seed as f32 / u32::MAX as f32
    };
    for i in 0..12000 {
        let (x, z) = ((rnd() - 0.5) * 22000.0, (rnd() - 0.5) * 22000.0);
        let s = 30.0 + (i % 7) as f32 * 6.0;
        inst.extend_from_slice(&[s, 0.0, 0.0, 0.0, s * 3.2, 0.0, 0.0, 0.0, s, x, 100.0, z, 0.12, 0.3, 0.1]);
    }
    let mat = Material { specular: 0.15, rim: 0.2, ..Material::default() };
    let mut r = Raster::new(w, h);
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(3);
    let t = std::time::Instant::now();
    for _ in 0..n {
        r.clear(0, 255);
        r.draw(&cone, &inst, &vp, [eye[0] as f32, eye[1] as f32, eye[2] as f32], &mat, &Lighting::default(), true);
    }
    println!("{:.2} ms/frame, {} drawn, {} shaded", t.elapsed().as_secs_f64() * 1000.0 / n as f64, r.drawn, r.shaded);
}
