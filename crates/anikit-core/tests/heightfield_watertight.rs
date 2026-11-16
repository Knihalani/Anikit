//! Merged heightfield runs (flat plateaus collapsed into long quads) create
//! T-junctions against neighbouring rows. Rasterise a text-like field merged
//! and unmerged from several angles and check the merged one has no holes.
use anikit_core::raster::*;
use anikit_core::space::{look_at, mul, perspective};

fn unmerged(w: usize, h: usize, merged: &Mesh) -> Mesh {
    let mut m = Mesh { pos: merged.pos.clone(), nrm: merged.nrm.clone(), uv: merged.uv.clone(), idx: Vec::new() };
    for z in 0..h - 1 {
        for x in 0..w - 1 {
            let (a, b, c, d) = ((z * w + x) as u32, (z * w + x + 1) as u32, ((z + 1) * w + x + 1) as u32, ((z + 1) * w + x) as u32);
            m.idx.extend_from_slice(&[a, c, b, a, d, c]);
        }
    }
    m
}

#[test]
fn merged_heightfield_has_no_holes() {
    // Blocky "letters": raised plateaus with sloped edges, like mesh.text().
    let (w, h) = (160usize, 48usize);
    let mut hs = vec![0.0f32; w * h];
    for z in 0..h {
        for x in 0..w {
            let inside = (8..40).contains(&z) && ((10..40).contains(&x) || (60..70).contains(&x) || ((90..150).contains(&x) && (z < 16 || z > 32 || (110..130).contains(&x))));
            hs[z * w + x] = if inside { 0.06 } else { 0.0 };
        }
    }
    // Soften the edges a little (a bevel), keeping exact plateaus inside.
    let src = hs.clone();
    for z in 1..h - 1 {
        for x in 1..w - 1 {
            let n = [src[z * w + x - 1], src[z * w + x + 1], src[(z - 1) * w + x], src[(z + 1) * w + x]];
            if n.iter().any(|v| *v != src[z * w + x]) {
                hs[z * w + x] = (src[z * w + x] * 2.0 + n.iter().sum::<f32>()) / 6.0;
            }
        }
    }
    let merged = heightfield(w, h, &hs, -1.0);
    let full = unmerged(w, h, &merged);
    assert!(merged.idx.len() < full.idx.len(), "merging reduced triangles");
    let inst = [3.0f32, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 3.0 * h as f32 / w as f32, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let mat = Material { ambient: 1.0, diffuse: 0.0, specular: 0.0, rim: 0.0, ..Material::default() };
    for (eye, size) in [([0.0, 3.0, 0.4], (640usize, 360usize)), ([1.2, 1.6, 2.0], (800, 450)), ([-0.8, 0.6, 2.6], (1280, 720))] {
        let m = mul(&perspective(45.0, size.0 as f64 / size.1 as f64, 0.05, 50.0), &look_at(eye, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
        let vp: M4 = core::array::from_fn(|i| m[i] as f32);
        let ef = [eye[0] as f32, eye[1] as f32, eye[2] as f32];
        let draw = |mesh: &Mesh| {
            let mut r = Raster::new(size.0, size.1);
            r.clear(0, 255);
            r.draw(mesh, &inst, &vp, ef, &mat, &Lighting::default(), false);
            r.color.iter().map(|c| c & 0x00ff_ffff != 0).collect::<Vec<_>>()
        };
        let (a, b) = (draw(&merged), draw(&full));
        let holes = a.iter().zip(&b).filter(|(m, f)| !**m && **f).count();
        let covered = b.iter().filter(|f| **f).count();
        assert!(covered > 1000, "the field is in view");
        assert_eq!(holes, 0, "pixels covered unmerged but not merged (eye {eye:?}): {holes} of {covered}");
    }
}
