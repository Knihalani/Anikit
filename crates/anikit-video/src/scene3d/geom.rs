//! Meshes for `scene3d` objects, in metres, centred on their origin unless
//! noted. Rounded boxes and slabs are extruded rounded rectangles with a
//! quarter-round bevel (the same extruder as 3D text), so their edges catch
//! highlights. Front faces (+Z) carry UVs `(0,0)` top-left to `(1,1)`
//! bottom-right, for textures.

use crate::components::text3d::{clean, extrude_segs, glyph_outlines};
use anikit_core::raster::Mesh;
use std::f32::consts::{FRAC_PI_2, TAU};

/// A rounded rectangle outline (counter-clockwise), `w × h`, corner radius `r`.
fn rounded_rect(w: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    let r = r.clamp(0.0, w.min(h) * 0.5 * 0.999);
    let (hw, hh) = (w * 0.5, h * 0.5);
    if r <= 1e-5 {
        return vec![[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]];
    }
    let segs = 10;
    let mut out = Vec::new();
    for (cx, cy, a0) in [(hw - r, -hh + r, -FRAC_PI_2), (hw - r, hh - r, 0.0), (-hw + r, hh - r, FRAC_PI_2), (-hw + r, -hh + r, std::f32::consts::PI)] {
        for k in 0..=segs {
            let a = a0 + k as f32 / segs as f32 * FRAC_PI_2;
            out.push([cx + r * a.cos(), cy + r * a.sin()]);
        }
    }
    out
}

/// Planar UVs over the XY bounds (front view).
fn planar_uv(m: &mut Mesh, w: f32, h: f32) {
    m.uv.clear();
    for p in m.pos.chunks_exact(3) {
        m.uv.extend_from_slice(&[p[0] / w + 0.5, 0.5 - p[1] / h]);
    }
}

/// A box `size` with rounded vertical edges (`radius`) and a bevel on the
/// front and back edges (`bevel`, defaults to the radius, capped by depth).
pub fn rounded_box(size: [f32; 3], radius: f32, bevel: f32) -> Mesh {
    let outline = rounded_rect(size[0], size[1], radius);
    let hz = size[2] * 0.5;
    let b = bevel.min(hz * 0.95).min(size[0].min(size[1]) * 0.45).max(0.0);
    let mut m = extrude_segs(&[outline], hz, b, 0.0, if b > 0.0 { 6 } else { 0 });
    planar_uv(&mut m, size[0], size[1]);
    m
}

/// A thin plane `w × h` facing +Z, double-sided by the renderer.
pub fn plane(w: f32, h: f32) -> Mesh {
    let (hw, hh) = (w * 0.5, h * 0.5);
    Mesh {
        pos: vec![-hw, -hh, 0.0, hw, -hh, 0.0, hw, hh, 0.0, -hw, hh, 0.0],
        nrm: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
        uv: vec![0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
        idx: vec![0, 1, 2, 0, 2, 3],
    }
}

/// A cylinder of `radius` and `height` along Y, centred, with caps.
pub fn cylinder(radius: f32, height: f32) -> Mesh {
    let mut m = anikit_core::raster::cylinder(128, true);
    for p in m.pos.chunks_exact_mut(3) {
        p[0] *= radius;
        p[1] *= height;
        p[2] *= radius;
    }
    m
}

/// A torus in the XY plane (ring facing +Z).
pub fn torus(radius: f32, tube: f32) -> Mesh {
    anikit_core::raster::torus(radius, tube, 192, 48)
}

/// Extruded text, `height` metres tall (cap height), `depth` deep, with a
/// smooth bevel; centred on X, sitting on y = 0, centred on Z. The cap
/// normals get a gentle vertical "pillow" tilt so flat chrome faces sweep
/// through the environment instead of reading as one flat tone.
/// `align`: -1 puts the left edge at x = 0, 1 the right edge, 0 centres.
pub fn text(s: &str, font: &str, weight: f32, tracking: f32, height: f32, depth: f32, bevel: f32, align: f32) -> Result<Mesh, String> {
    let letters = glyph_outlines(s, font, weight, tracking);
    let shapes: Vec<Vec<Vec<[f32; 2]>>> = letters.into_iter().map(|cs| cs.into_iter().filter_map(clean).collect::<Vec<_>>()).filter(|c: &Vec<_>| !c.is_empty()).collect();
    if shapes.is_empty() {
        return Err("text has no visible glyphs".into());
    }
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in shapes.iter().flatten().flatten() {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let k = height / (hi[1] - lo[1]).max(1e-3);
    let cx = (lo[0] + hi[0]) * 0.5 + align * (hi[0] - lo[0]) * 0.5;
    let mut out = Mesh::default();
    for cs in &shapes {
        let cs: Vec<Vec<[f32; 2]>> = cs.iter().map(|c| c.iter().map(|p| [(p[0] - cx) * k, (p[1] - lo[1]) * k - height * 0.5]).collect()).collect();
        let hz = depth * 0.5;
        let b = bevel.min(hz * 0.9);
        let m = extrude_segs(&cs, hz, b, 0.55 / height.max(1e-3), 10);
        let base = (out.pos.len() / 3) as u32;
        for p in m.pos.chunks_exact(3) {
            out.pos.extend_from_slice(&[p[0], p[1] + height * 0.5, p[2]]);
        }
        out.nrm.extend_from_slice(&m.nrm);
        out.idx.extend(m.idx.iter().map(|i| i + base));
    }
    let w = (hi[0] - lo[0]) * k;
    for p in out.pos.chunks_exact(3) {
        out.uv.extend_from_slice(&[(p[0] / w.max(1e-3) + 0.5 + align * 0.5).clamp(0.0, 1.0), 1.0 - p[1] / height]);
    }
    Ok(out)
}

/// A phone: a rounded slab of `size` (the screen is the front face's UVs).
pub fn phone(size: [f32; 3]) -> Mesh {
    rounded_box(size, size[0].min(size[1]) * 0.16, size[2] * 0.45)
}

#[allow(dead_code)]
fn _unused() -> f32 {
    TAU
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slab_has_front_uvs_and_closed_shape() {
        let m = rounded_box([1.6, 0.9, 0.008], 0.03, 0.002);
        assert!(m.idx.len() > 100);
        assert_eq!(m.uv.len() / 2, m.pos.len() / 3);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for p in m.pos.chunks_exact(3) {
            lo = lo.min(p[0]);
            hi = hi.max(p[0]);
        }
        assert!((hi - lo - 1.6).abs() < 1e-3);
    }

    #[test]
    fn text_is_requested_height() {
        let m = text("AK", "Inter", 800.0, 0.02, 0.5, 0.1, 0.01, 0.0).unwrap();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for p in m.pos.chunks_exact(3) {
            lo = lo.min(p[1]);
            hi = hi.max(p[1]);
        }
        assert!(lo.abs() < 1e-3 && (hi - 0.5).abs() < 1e-3, "{lo} {hi}");
    }
}
