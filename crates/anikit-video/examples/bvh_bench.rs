//! Microbenchmark: mesh BVH traversal (closest hit and occlusion), rays per second.
//! `cargo run --release -p anikit-video --example bvh_bench`
use anikit_video::scene3d::bvh::Bvh;
use anikit_video::scene3d::geom;
use anikit_video::scene3d::math::*;
use std::time::Instant;

fn rays(n: usize, lo: V3, hi: V3) -> Vec<(V3, V3)> {
    let mut s = 12345u32;
    let mut r = || {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        (s >> 8) as f32 / 16777216.0
    };
    let c = scale(add(lo, hi), 0.5);
    let e = len(sub(hi, lo));
    (0..n)
        .map(|_| {
            let o = add(c, v3((r() - 0.5) * e * 0.6, (r() - 0.2) * e * 0.3, e * 1.5));
            let k = if std::env::var_os("WIDE_RAYS").is_some() { 2.5 } else { 1.0 };
            let t = add(c, v3((r() - 0.5) * (hi.x - lo.x) * k, (r() - 0.5) * (hi.y - lo.y) * k, (r() - 0.5) * (hi.z - lo.z) * k));
            (o, norm(sub(t, o)))
        })
        .collect()
}

fn main() {
    let meshes = [("slab", geom::rounded_box([1.6, 0.9, 0.008], 0.027, 0.0024)), ("glass", geom::text("GLASS", "Inter", 800.0, 0.02, 0.22, 0.06, 0.0099).unwrap()), ("text", geom::text("ANIKIT", "", 700.0, 0.0, 1.0, 0.34, 0.045).unwrap()), ("torus", geom::torus(0.7, 0.112))];
    for (name, m) in meshes {
        let b = Bvh::with_uv(&m.pos, &m.nrm, &m.uv, &m.idx);
        let rs = rays(1_000_000, b.lo, b.hi);
        for rep in 0..2 {
            let c = Instant::now();
            let mut hits = 0;
            for &(o, d) in &rs {
                hits += b.intersect(o, d, 1e-4, 1e9).is_some() as u32;
            }
            let t1 = c.elapsed().as_secs_f64() * 1e9 / rs.len() as f64;
            let c = Instant::now();
            let mut occ = 0;
            for &(o, d) in &rs {
                occ += b.occluded(o, d, 1e-4, 1e9) as u32;
            }
            let t2 = c.elapsed().as_secs_f64() * 1e9 / rs.len() as f64;
            if rep == 1 {
                println!("{name} ({} tris): closest {t1:.0} ns/ray ({hits} hits), occluded {t2:.0} ns/ray ({occ})", b.tris.len());
            }
        }
    }
}
