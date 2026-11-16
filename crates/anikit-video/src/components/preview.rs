//! Test-only: validate ops, and rasterise a display list with vello_cpu so
//! components can be eyeballed as PNGs (`target/components/*.png`) before
//! the RENDER module lands. Filters other than blur and shadow are ignored.

use super::text::StubAssets;
use crate::dl::{self, Op, Paint};
use std::sync::Arc;
use vello_cpu::kurbo::{self, Affine};
use vello_cpu::peniko::{self, ColorStop, Extend, Gradient};
use vello_cpu::{color::AlphaColor, Pixmap, RenderContext, Resources};

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

fn aff(t: &dl::Affine) -> Affine {
    Affine::new(t.map(|v| v as f64))
}

fn col(c: dl::Rgba) -> AlphaColor<peniko::color::Srgb> {
    AlphaColor::new(c.0)
}

fn stops(s: &[(f32, dl::Rgba)]) -> peniko::ColorStops {
    let v: Vec<ColorStop> = s.iter().map(|(o, c)| ColorStop::from((*o, col(*c)))).collect();
    peniko::ColorStops(v.into_iter().collect())
}

fn set_paint(ctx: &mut RenderContext, p: &Paint, assets: &StubAssets) {
    match p {
        Paint::Solid(c) => ctx.set_paint(col(*c)),
        Paint::Linear { p0, p1, stops: s } => ctx.set_paint(Gradient {
            kind: peniko::LinearGradientPosition { start: (p0[0] as f64, p0[1] as f64).into(), end: (p1[0] as f64, p1[1] as f64).into() }.into(),
            stops: stops(s),
            extend: Extend::Pad,
            ..Default::default()
        }),
        Paint::Radial { c, r, stops: s } => ctx.set_paint(Gradient {
            kind: peniko::RadialGradientPosition { start_center: (c[0] as f64, c[1] as f64).into(), start_radius: 0.0, end_center: (c[0] as f64, c[1] as f64).into(), end_radius: *r }.into(),
            stops: stops(s),
            extend: Extend::Pad,
            ..Default::default()
        }),
        Paint::Image { image, transform, opacity } => {
            if let Some(im) = assets.get(*image) {
                let pm = pixmap(&im.rgba, im.w, im.h, *opacity);
                ctx.set_paint(vello_cpu::Image {
                    image: vello_cpu::ImageSource::Pixmap(Arc::new(pm)),
                    sampler: peniko::ImageSampler { x_extend: Extend::Repeat, y_extend: Extend::Repeat, quality: peniko::ImageQuality::Medium, ..Default::default() },
                });
                // transform maps local → image pixels; paint transform maps image → local.
                ctx.set_paint_transform(aff(transform).inverse());
            } else {
                ctx.set_paint(col(dl::Rgba([1.0, 0.0, 1.0, 1.0])));
            }
        }
    }
}

fn pixmap(rgba: &[u8], w: u32, h: u32, opacity: f32) -> Pixmap {
    let mut data = rgba.to_vec();
    if opacity < 1.0 {
        for px in data.chunks_mut(4) {
            px[3] = (px[3] as f32 * opacity) as u8;
        }
    }
    Pixmap::from_parts(data, w as u16, h as u16, vello_cpu::PixelMetadata { may_have_transparency: true, alpha_type: peniko::ImageAlphaType::Alpha })
}

/// Rasterise to straight RGBA8.
pub fn render(list: &dl::DisplayList, assets: &StubAssets) -> Vec<u8> {
    let (w, h) = (list.width as u16, list.height as u16);
    let mut ctx = RenderContext::new(w, h);
    ctx.set_paint(col(list.background));
    ctx.fill_rect(&kurbo::Rect::new(0.0, 0.0, w as f64, h as f64));
    for op in &list.ops {
        ctx.reset_paint_transform();
        match op {
            Op::Fill { path, paint, rule, transform } => {
                ctx.set_transform(aff(transform));
                ctx.set_fill_rule(if *rule == dl::FillRule::EvenOdd { peniko::Fill::EvenOdd } else { peniko::Fill::NonZero });
                set_paint(&mut ctx, paint, assets);
                ctx.fill_path(path);
            }
            Op::Stroke { path, paint, stroke, transform } => {
                ctx.set_transform(aff(transform));
                let mut s = kurbo::Stroke::new(stroke.width as f64)
                    .with_caps([kurbo::Cap::Butt, kurbo::Cap::Round, kurbo::Cap::Square][stroke.cap.min(2) as usize])
                    .with_join([kurbo::Join::Miter, kurbo::Join::Round, kurbo::Join::Bevel][stroke.join.min(2) as usize]);
                if !stroke.dash.is_empty() {
                    s = s.with_dashes(stroke.dash_offset as f64, stroke.dash.iter().map(|d| *d as f64));
                }
                ctx.set_stroke(s);
                set_paint(&mut ctx, paint, assets);
                if stroke.trim != [0.0, 1.0] {
                    ctx.stroke_path(&trim(path, stroke.trim));
                } else {
                    ctx.stroke_path(path);
                }
            }
            Op::RoundRect { x, y, w, h, radius, paint, transform } => {
                ctx.set_transform(aff(transform));
                set_paint(&mut ctx, paint, assets);
                ctx.fill_path(&super::style::rrect_path(*x, *y, *w, *h, *radius));
            }
            Op::Image { image, dst, opacity, transform } => {
                ctx.set_transform(aff(transform));
                if let Some(im) = assets.get(*image) {
                    let sx = im.w as f32 / dst[2];
                    let sy = im.h as f32 / dst[3];
                    set_paint(&mut ctx, &Paint::Image { image: *image, transform: [sx, 0.0, 0.0, sy, -dst[0] * sx, -dst[1] * sy], opacity: *opacity }, assets);
                    ctx.fill_rect(&kurbo::Rect::new(dst[0] as f64, dst[1] as f64, (dst[0] + dst[2]) as f64, (dst[1] + dst[3]) as f64));
                }
            }
            Op::Glyphs { .. } => {}
            Op::PushLayer { opacity, blend, filters, clip, .. } => {
                let clip = clip.as_ref().map(|(p, t)| aff(t) * p.clone());
                ctx.set_transform(Affine::IDENTITY);
                let mut filter = None;
                for f in filters {
                    match f {
                        dl::Filter::Blur(r) if *r > 0.0 => filter = Some(vello_cpu::filter_effects::Filter::from_primitive(peniko_blur(*r * 0.5))),
                        dl::Filter::Glow { radius, .. } => filter = Some(vello_cpu::filter_effects::Filter::from_primitive(peniko_blur(*radius * 0.5))),
                        _ => {}
                    }
                }
                let bm = match blend {
                    dl::Blend::Normal => None,
                    dl::Blend::Add => Some(peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::Plus)),
                    dl::Blend::Multiply => Some(peniko::Mix::Multiply.into()),
                    dl::Blend::Screen => Some(peniko::Mix::Screen.into()),
                    dl::Blend::Overlay => Some(peniko::Mix::Overlay.into()),
                    dl::Blend::Darken => Some(peniko::Mix::Darken.into()),
                    dl::Blend::Lighten => Some(peniko::Mix::Lighten.into()),
                    dl::Blend::SoftLight => Some(peniko::Mix::SoftLight.into()),
                    dl::Blend::Difference => Some(peniko::Mix::Difference.into()),
                };
                ctx.push_layer(clip.as_ref(), bm, Some(*opacity), None, filter);
            }
            Op::PopLayer => ctx.pop_layer(),
        }
    }
    ctx.flush();
    let mut pm = Pixmap::new(w, h);
    ctx.render(&mut pm, &mut Resources::new());
    let mut out = pm.data_as_u8_slice().to_vec();
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

fn peniko_blur(std_dev: f32) -> vello_cpu::filter_effects::FilterPrimitive {
    vello_cpu::filter_effects::FilterPrimitive::gaussian_blur(std_dev, Default::default())
}

/// Trim a path to `[a, b]` of its arc length.
fn trim(path: &dl::Path, t: [f32; 2]) -> kurbo::BezPath {
    use kurbo::{ParamCurve, ParamCurveArclen};
    let segs: Vec<kurbo::PathSeg> = path.segments().collect();
    let lens: Vec<f64> = segs.iter().map(|s| s.arclen(0.1)).collect();
    let total: f64 = lens.iter().sum();
    let (a, b) = (t[0].clamp(0.0, 1.0) as f64 * total, t[1].clamp(0.0, 1.0) as f64 * total);
    let mut out = kurbo::BezPath::new();
    let mut acc = 0.0;
    let mut open = false;
    for (s, l) in segs.iter().zip(&lens) {
        let (s0, s1) = (acc, acc + l);
        acc = s1;
        if s1 <= a || s0 >= b || *l <= 0.0 {
            open = false;
            continue;
        }
        let u0 = if a > s0 { s.inv_arclen(a - s0, 0.1) } else { 0.0 };
        let u1 = if b < s1 { s.inv_arclen(b - s0, 0.1) } else { 1.0 };
        let sub = s.subsegment(u0..u1);
        if !open {
            out.move_to(sub.start());
            open = true;
        }
        match sub {
            kurbo::PathSeg::Line(l) => out.line_to(l.p1),
            kurbo::PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
            kurbo::PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
        }
    }
    out
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
pub fn snapshot(name: &str, w: u32, h: u32, bg: dl::Rgba, ops: Vec<Op>, assets: &StubAssets) -> Vec<u8> {
    let list = dl::DisplayList { width: w, height: h, background: bg, ops };
    let px = render(&list, assets);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/components");
    save_png(&dir.join(format!("{name}.png")), w, h, &px);
    px
}

