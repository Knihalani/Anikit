//! Test-only: validate ops, and rasterise a display list with vello_cpu so
//! components can be eyeballed as PNGs (`target/components/*.png`) before
//! the RENDER module lands. Filters other than blur and shadow are ignored.

use super::host::Host;
use crate::dl::{self, Op, Paint};
use std::sync::Arc;
use vello_cpu::kurbo;

/// Check ops are well formed: finite numbers, balanced layers, sane sizes.
pub fn validate(ops: &[Op]) -> Result<(), String> {
    let mut depth = 0i32;
    let fin = |v: &[f32], what: &str| -> Result<(), String> {
        if v.iter().all(|x| x.is_finite()) {
            Ok(())
        } else {
            Err(format!("non-finite {what}: {v:?}"))
        }
    };
    let paint = |p: &Paint| -> Result<(), String> {
        match p {
            Paint::Solid(c) => fin(&c.0, "colour"),
            Paint::Linear { p0, p1, stops } => {
                fin(p0, "p0")?;
                fin(p1, "p1")?;
                stops.iter().try_for_each(|(o, c)| fin(&[*o], "stop").and(fin(&c.0, "stop colour")))
            }
            Paint::Radial { c, r, stops } => {
                fin(c, "centre")?;
                fin(&[*r], "radius")?;
                stops.iter().try_for_each(|(o, c)| fin(&[*o], "stop").and(fin(&c.0, "stop colour")))
            }
            Paint::Image { transform, .. } => fin(transform, "image transform"),
        }
    };
    let path = |p: &dl::Path| -> Result<(), String> {
        for el in p.elements() {
            let ok = match el {
                kurbo::PathEl::MoveTo(a) | kurbo::PathEl::LineTo(a) => a.is_finite(),
                kurbo::PathEl::QuadTo(a, b) => a.is_finite() && b.is_finite(),
                kurbo::PathEl::CurveTo(a, b, c) => a.is_finite() && b.is_finite() && c.is_finite(),
                kurbo::PathEl::ClosePath => true,
            };
            if !ok {
                return Err("non-finite path point".into());
            }
        }
        Ok(())
    };
    for op in ops {
        match op {
            Op::Fill { path: p, paint: pa, transform, .. } => {
                path(p)?;
                paint(pa)?;
                fin(transform, "transform")?;
            }
            Op::Stroke { path: p, paint: pa, stroke, transform } => {
                path(p)?;
                paint(pa)?;
                fin(transform, "transform")?;
                fin(&[stroke.width, stroke.trim[0], stroke.trim[1]], "stroke")?;
                if stroke.width < 0.0 {
                    return Err("negative stroke width".into());
                }
            }
            Op::RoundRect { x, y, w, h, radius, paint: pa, transform } => {
                fin(&[*x, *y, *w, *h, *radius], "rect")?;
                if *w < 0.0 || *h < 0.0 {
                    return Err(format!("negative rect size {w}×{h}"));
                }
                paint(pa)?;
                fin(transform, "transform")?;
            }
            Op::Image { dst, opacity, transform, .. } => {
                fin(dst, "dst")?;
                fin(&[*opacity], "opacity")?;
                fin(transform, "transform")?;
            }
            Op::Glyphs { size, glyphs, paint: pa, transform, .. } => {
                fin(&[*size], "size")?;
                paint(pa)?;
                fin(transform, "transform")?;
                for g in glyphs {
                    fin(&[g.x, g.y], "glyph")?;
                }
            }
            Op::PushLayer { opacity, clip, .. } => {
                fin(&[*opacity], "opacity")?;
                if let Some((p, t)) = clip {
                    path(p)?;
                    fin(t, "clip transform")?;
                }
                depth += 1;
            }
            Op::PopLayer => {
                depth -= 1;
                if depth < 0 {
                    return Err("PopLayer without PushLayer".into());
                }
            }
        }
    }
    if depth != 0 {
        return Err(format!("{depth} unclosed layer(s)"));
    }
    Ok(())
}

/// Rasterise with the RENDER module to straight RGBA8.
pub fn render(list: &dl::DisplayList, host: &Host) -> Vec<u8> {
    let assets = Arc::new(host.render_assets(None));
    let mut r = crate::render::Renderer::new(assets);
    let frame = r.render(list);
    let mut out = frame.data_as_u8_slice().to_vec();
    for px in out.chunks_mut(4) {
        let a = px[3] as f32 / 255.0;
        if a > 0.0 && a < 1.0 {
            for c in &mut px[..3] {
                *c = ((*c as f32 / a).round()).min(255.0) as u8;
            }
        }
    }
    out
}

/// A shared host with the bundled fonts (fonts load once per test binary).
pub fn host() -> &'static Host {
    static H: std::sync::OnceLock<Host> = std::sync::OnceLock::new();
    H.get_or_init(Host::bundled)
}

/// Write a PNG of straight RGBA8.
pub fn save_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let f = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

/// Render ops on a background and save under `target/components/<name>.png`.
pub fn snapshot(name: &str, w: u32, h: u32, bg: dl::Rgba, ops: Vec<Op>, assets: &Host) -> Vec<u8> {
    let list = dl::DisplayList { width: w, height: h, background: bg, ops };
    let px = render(&list, assets);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/components");
    save_png(&dir.join(format!("{name}.png")), w, h, &px);
    px
}

