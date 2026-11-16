//! Minimal vello_cpu renderer for the ops the text module emits (test-only;
//! the real renderer lives in `render`).
#![allow(dead_code)]

use anikit_video::dl::{Affine, DisplayList, Filter, Op, Paint, Rgba};
use anikit_video::text::FontStore;
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{self, Rect};
use vello_cpu::{Pixmap, RenderContext, Resources};

fn aff(a: &Affine) -> kurbo::Affine {
    kurbo::Affine::new(a.map(|v| v as f64))
}
fn col(c: Rgba) -> AlphaColor<Srgb> {
    AlphaColor::new(c.0)
}

fn set_paint(ctx: &mut RenderContext, p: &Paint) {
    match p {
        Paint::Solid(c) => ctx.set_paint(col(*c)),
        Paint::Linear { p0, p1, stops } => {
            let g = vello_cpu::peniko::Gradient::new_linear((p0[0] as f64, p0[1] as f64), (p1[0] as f64, p1[1] as f64))
                .with_stops(stops.iter().map(|(o, c)| (*o, col(*c))).collect::<Vec<_>>().as_slice());
            ctx.set_paint(g);
        }
        _ => {}
    }
}

pub fn render(fonts: &FontStore, dl: &DisplayList) -> Pixmap {
    let (w, h) = (dl.width as u16, dl.height as u16);
    let mut ctx = RenderContext::new(w, h);
    let mut res = Resources::new();
    ctx.set_paint(col(dl.background));
    ctx.fill_rect(&Rect::new(0.0, 0.0, w as f64, h as f64));
    for op in &dl.ops {
        match op {
            Op::RoundRect { x, y, w, h, radius, paint: Paint::Solid(c), transform } => {
                ctx.set_transform(aff(transform));
                ctx.set_paint(col(*c));
                let r = kurbo::RoundedRect::new(*x as f64, *y as f64, (*x + *w) as f64, (*y + *h) as f64, *radius as f64);
                ctx.fill_path(&kurbo::Shape::to_path(&r, 0.1));
            }
            Op::Fill { path, paint: Paint::Solid(c), transform, .. } => {
                ctx.set_transform(aff(transform));
                ctx.set_paint(col(*c));
                ctx.fill_path(path);
            }
            Op::Glyphs { font, size, glyphs, paint, transform } => {
                let f = fonts.face(*font);
                ctx.set_transform(aff(transform));
                set_paint(&mut ctx, paint);
                let mut b = ctx.glyph_run(&mut res, &f.data).font_size(*size).hint(false);
                b = b.normalized_coords(&f.coords);
                if let Some(s) = f.skew {
                    b = b.glyph_transform(kurbo::Affine::skew((-s as f64).to_radians().tan(), 0.0));
                }
                let _ = b.fill_glyphs(glyphs.iter().map(|g| vello_cpu::Glyph { id: g.id as u32, x: g.x, y: g.y }));
            }
            Op::PushLayer { opacity, clip, filters, .. } => {
                let clip_path = clip.as_ref().map(|(p, a)| aff(a) * p.clone());
                ctx.set_transform(kurbo::Affine::IDENTITY);
                let filter = filters.iter().find_map(|f| match f {
                    Filter::Blur(r) => Some(vello_cpu::filter_effects::Filter::from_primitive(
                        vello_cpu::filter_effects::FilterPrimitive::gaussian_blur(*r, vello_cpu::filter_effects::EdgeMode::None),
                    )),
                    _ => None,
                });
                ctx.push_layer(clip_path.as_ref(), None, Some(*opacity), None, filter);
            }
            Op::PopLayer => ctx.pop_layer(),
            _ => {}
        }
    }
    ctx.flush();
    let mut pm = Pixmap::new(w, h);
    ctx.render(&mut pm, &mut res);
    pm
}

pub fn save(pm: Pixmap, name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target-golden");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.png")), pm.into_png().unwrap()).unwrap();
}

/// Mean absolute difference vs a stored golden (0..255); writes the golden if missing
/// or when `UPDATE_GOLDEN=1`.
pub fn golden(pm: &Pixmap, name: &str) -> f64 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.png"));
    let cur = pm.data_as_u8_slice();
    if std::env::var("UPDATE_GOLDEN").is_ok() || !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, pm.clone().into_png().unwrap()).unwrap();
        return 0.0;
    }
    let old = Pixmap::from_png(std::io::Cursor::new(std::fs::read(&path).unwrap())).unwrap();
    let old = old.data_as_u8_slice();
    assert_eq!(old.len(), cur.len(), "golden {name}: size changed");
    old.iter().zip(cur).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as f64).sum::<f64>() / cur.len() as f64
}
