//! Golden-image tests for every display-list op.
//!
//! References live in `tests/golden/*.png`. Regenerate with
//! `AK_BLESS=1 cargo test -p anikit-video --test render_golden`.

use std::sync::Arc;

use anikit_video::dl::*;
use anikit_video::render::{Assets, Renderer};
use kurbo::{BezPath, Shape};

const W: u32 = 96;
const H: u32 = 96;

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba([r, g, b, a])
}

fn solid(r: f32, g: f32, b: f32) -> Paint {
    Paint::Solid(rgba(r, g, b, 1.0))
}

fn list(ops: Vec<Op>) -> DisplayList {
    DisplayList { width: W, height: H, background: rgba(0.1, 0.1, 0.15, 1.0), ops }
}

fn circle(cx: f64, cy: f64, r: f64) -> BezPath {
    kurbo::Circle::new((cx, cy), r).to_path(0.1)
}

fn star() -> BezPath {
    let mut p = BezPath::new();
    for i in 0..5 {
        let a = std::f64::consts::PI * (i as f64 * 0.8 - 0.5);
        let pt = (48.0 + 38.0 * a.cos(), 50.0 + 38.0 * a.sin());
        if i == 0 {
            p.move_to(pt)
        } else {
            p.line_to(pt)
        }
    }
    p.close_path();
    p
}

fn wave() -> BezPath {
    let mut p = BezPath::new();
    p.move_to((10.0, 70.0));
    p.curve_to((30.0, 10.0), (60.0, 10.0), (86.0, 60.0));
    p.line_to((60.0, 86.0));
    p
}

fn stroke(width: f32) -> Stroke {
    Stroke { width, cap: 0, join: 0, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] }
}

fn layer(opacity: f32, blend: Blend, filters: Vec<Filter>, clip: Option<(Path, Affine)>, cache_key: Option<u64>) -> Op {
    Op::PushLayer { opacity, blend, filters, clip, cache_key }
}

fn card(x: f32, y: f32, w: f32, h: f32, p: Paint) -> Op {
    Op::RoundRect { x, y, w, h, radius: 10.0, paint: p, transform: IDENTITY }
}

fn test_image(assets: &mut Assets) {
    // 8×8 checker with an alpha gradient, encoded to PNG and decoded by the store.
    let mut img = image::RgbaImage::new(8, 8);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let on = (x + y) % 2 == 0;
        *p = image::Rgba([if on { 255 } else { 30 }, (x * 32) as u8, (y * 32) as u8, 128 + (x * 16) as u8]);
    }
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    assets.images.insert_encoded(ImageId(1), &png).unwrap();
}

fn font(assets: &mut Assets) -> Option<FontId> {
    let bytes = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").ok()?;
    Some(assets.fonts.add(Arc::new(bytes), 0))
}

fn scenes(font: Option<FontId>) -> Vec<(&'static str, DisplayList)> {
    let grad = Paint::Linear {
        p0: [0.0, 0.0],
        p1: [96.0, 96.0],
        stops: vec![(0.0, rgba(1.0, 0.2, 0.4, 1.0)), (0.5, rgba(1.0, 0.7, 0.0, 1.0)), (1.0, rgba(0.2, 0.4, 1.0, 1.0))],
    };
    let radial = Paint::Radial {
        c: [48.0, 48.0],
        r: 40.0,
        stops: vec![(0.0, rgba(1.0, 1.0, 1.0, 1.0)), (1.0, rgba(0.0, 0.5, 1.0, 0.0))],
    };
    let rot = {
        let (s, c) = (0.4f32.sin(), 0.4f32.cos());
        [c, s, -s, c, 48.0 - 48.0 * c + 48.0 * s, 48.0 - 48.0 * s - 48.0 * c]
    };
    let mut v = vec![
        ("fill_nonzero", list(vec![Op::Fill { path: star(), paint: solid(0.9, 0.8, 0.2), rule: FillRule::NonZero, transform: IDENTITY }])),
        ("fill_evenodd", list(vec![Op::Fill { path: star(), paint: solid(0.9, 0.8, 0.2), rule: FillRule::EvenOdd, transform: rot }])),
        ("fill_linear", list(vec![Op::Fill { path: circle(48.0, 48.0, 40.0), paint: grad.clone(), rule: FillRule::NonZero, transform: IDENTITY }])),
        ("fill_radial", list(vec![Op::Fill { path: circle(48.0, 48.0, 44.0), paint: radial, rule: FillRule::NonZero, transform: IDENTITY }])),
        ("stroke_joins_caps", list(vec![
            Op::Stroke { path: wave(), paint: solid(0.3, 0.9, 0.6), stroke: Stroke { cap: 1, join: 1, ..stroke(8.0) }, transform: IDENTITY },
            Op::Stroke { path: wave(), paint: solid(1.0, 0.3, 0.3), stroke: Stroke { cap: 2, join: 0, ..stroke(2.0) }, transform: IDENTITY },
        ])),
        ("stroke_dash", list(vec![Op::Stroke {
            path: circle(48.0, 48.0, 36.0),
            paint: grad.clone(),
            stroke: Stroke { dash: vec![12.0, 6.0, 2.0], dash_offset: 3.0, cap: 1, ..stroke(5.0) },
            transform: IDENTITY,
        }])),
        ("stroke_trim", list(vec![Op::Stroke {
            path: wave(),
            paint: solid(0.4, 0.7, 1.0),
            stroke: Stroke { trim: [0.15, 0.7], cap: 1, ..stroke(6.0) },
            transform: IDENTITY,
        }])),
        ("roundrect", list(vec![
            card(8.0, 8.0, 50.0, 30.0, solid(0.95, 0.95, 0.95)),
            Op::RoundRect { x: 20.0, y: 30.0, w: 60.0, h: 50.0, radius: 18.0, paint: Paint::Solid(rgba(0.2, 0.6, 1.0, 0.7)), transform: rot },
            Op::RoundRect { x: 70.0, y: 6.0, w: 20.0, h: 20.0, radius: 0.0, paint: solid(1.0, 0.4, 0.2), transform: IDENTITY },
        ])),
        ("image", list(vec![
            Op::Image { image: ImageId(1), dst: [8.0, 8.0, 48.0, 48.0], opacity: 1.0, transform: IDENTITY },
            Op::Image { image: ImageId(1), dst: [40.0, 40.0, 48.0, 48.0], opacity: 0.5, transform: rot },
        ])),
        ("image_paint", list(vec![Op::Fill {
            path: circle(48.0, 48.0, 40.0),
            paint: Paint::Image { image: ImageId(1), transform: [0.1, 0.0, 0.0, 0.1, 0.0, 0.0], opacity: 1.0 },
            rule: FillRule::NonZero,
            transform: IDENTITY,
        }])),
        ("layer_opacity_clip", list(vec![
            layer(0.6, Blend::Normal, vec![], Some((circle(48.0, 48.0, 30.0), IDENTITY)), None),
            card(0.0, 0.0, 96.0, 50.0, solid(1.0, 0.3, 0.3)),
            card(0.0, 46.0, 96.0, 50.0, solid(0.3, 1.0, 0.3)),
            Op::PopLayer,
        ])),
        ("layer_blend", {
            let mut ops = vec![card(10.0, 10.0, 50.0, 76.0, solid(0.9, 0.5, 0.1))];
            for (i, b) in [Blend::Multiply, Blend::Screen, Blend::Add, Blend::Difference].into_iter().enumerate() {
                ops.push(layer(1.0, b, vec![], None, None));
                ops.push(card(30.0, 6.0 + i as f32 * 22.0, 60.0, 18.0, solid(0.2, 0.5, 0.9)));
                ops.push(Op::PopLayer);
            }
            list(ops)
        }),
        ("filter_blur", list(vec![
            layer(1.0, Blend::Normal, vec![Filter::Blur(4.0)], None, None),
            Op::Fill { path: star(), paint: solid(1.0, 1.0, 1.0), rule: FillRule::NonZero, transform: IDENTITY },
            Op::PopLayer,
        ])),
        ("filter_shadow", list(vec![
            Op::RoundRect { x: 0.0, y: 0.0, w: 96.0, h: 96.0, radius: 0.0, paint: solid(0.9, 0.9, 0.92), transform: IDENTITY },
            layer(1.0, Blend::Normal, vec![Filter::Shadow { dx: 4.0, dy: 6.0, blur: 5.0, color: rgba(0.0, 0.0, 0.0, 0.6) }], None, None),
            card(18.0, 18.0, 56.0, 50.0, solid(1.0, 1.0, 1.0)),
            Op::PopLayer,
        ])),
        ("filter_glow", list(vec![
            layer(1.0, Blend::Normal, vec![Filter::Glow { radius: 6.0, intensity: 1.5, color: rgba(0.2, 0.8, 1.0, 1.0) }], None, None),
            Op::Stroke { path: wave(), paint: solid(1.0, 1.0, 1.0), stroke: Stroke { cap: 1, join: 1, ..stroke(4.0) }, transform: IDENTITY },
            Op::PopLayer,
        ])),
        ("filter_colormatrix", list(vec![
            // grayscale
            layer(1.0, Blend::Normal, vec![Filter::ColorMatrix([
                0.2126, 0.7152, 0.0722, 0.0, 0.0,
                0.2126, 0.7152, 0.0722, 0.0, 0.0,
                0.2126, 0.7152, 0.0722, 0.0, 0.0,
                0.0, 0.0, 0.0, 1.0, 0.0,
            ])], Some((kurbo::Rect::new(0.0, 0.0, 48.0, 96.0).to_path(0.1), IDENTITY)), None),
            Op::Fill { path: circle(48.0, 48.0, 40.0), paint: grad.clone(), rule: FillRule::NonZero, transform: IDENTITY },
            Op::PopLayer,
            layer(1.0, Blend::Normal, vec![], Some((kurbo::Rect::new(48.0, 0.0, 96.0, 96.0).to_path(0.1), IDENTITY)), None),
            Op::Fill { path: circle(48.0, 48.0, 40.0), paint: grad, rule: FillRule::NonZero, transform: IDENTITY },
            Op::PopLayer,
        ])),
        ("nested_layers", list(vec![
            layer(0.8, Blend::Normal, vec![Filter::Shadow { dx: 0.0, dy: 4.0, blur: 3.0, color: rgba(0.0, 0.0, 0.0, 0.8) }], None, Some(7)),
            card(10.0, 10.0, 76.0, 76.0, solid(0.95, 0.95, 0.95)),
            layer(1.0, Blend::Normal, vec![Filter::Blur(2.0)], Some((circle(48.0, 48.0, 28.0), IDENTITY)), None),
            Op::Fill { path: star(), paint: solid(0.9, 0.2, 0.4), rule: FillRule::NonZero, transform: IDENTITY },
            Op::PopLayer,
            Op::PopLayer,
        ])),
        ("shadow_card_with_content", list(vec![
            Op::RoundRect { x: 0.0, y: 0.0, w: 96.0, h: 96.0, radius: 0.0, paint: solid(0.85, 0.87, 0.9), transform: IDENTITY },
            layer(0.9, Blend::Normal, vec![Filter::Shadow { dx: 3.0, dy: 5.0, blur: 4.0, color: rgba(0.1, 0.0, 0.3, 0.7) }], None, None),
            Op::RoundRect { x: 12.0, y: 14.0, w: 60.0, h: 56.0, radius: 12.0, paint: solid(1.0, 1.0, 1.0), transform: [1.2, 0.0, 0.0, 1.2, -4.0, -6.0] },
            card(24.0, 26.0, 40.0, 12.0, solid(0.3, 0.5, 1.0)),
            Op::PopLayer,
        ])),
    ];
    if let Some(f) = font {
        // DejaVu Sans glyph ids for "Anikit" (A=36, n=81, i=76, k=78, t=87).
        let ids = [36u16, 81, 76, 78, 76, 87];
        let mut x = 0.0;
        let glyphs = ids.iter().map(|&id| {
            let g = Glyph { id, x, y: 0.0 };
            x += if id == 76 { 6.0 } else { 13.0 };
            g
        });
        let glyphs: Vec<_> = glyphs.collect();
        v.push(("glyphs", list(vec![
            Op::Glyphs { font: f, size: 22.0, glyphs: glyphs.clone(), paint: solid(1.0, 1.0, 1.0), transform: [1.0, 0.0, 0.0, 1.0, 6.0, 40.0] },
            Op::Glyphs { font: f, size: 22.0, glyphs, paint: Paint::Linear { p0: [0.0, -20.0], p1: [80.0, 0.0], stops: vec![(0.0, rgba(1.0, 0.3, 0.5, 1.0)), (1.0, rgba(0.3, 0.6, 1.0, 1.0))] }, transform: [1.0, 0.0, -0.3, 1.0, 10.0, 80.0] },
        ])));
    }
    v
}

fn compare(name: &str, got: &[u8]) -> Result<(), String> {
    let path = format!("{}/tests/golden/{name}.png", env!("CARGO_MANIFEST_DIR"));
    // Store straight RGBA in the PNG; the frame is opaque so premul = straight.
    if std::env::var_os("AK_BLESS").is_some() {
        image::save_buffer(&path, got, W, H, image::ExtendedColorType::Rgba8).unwrap();
        return Ok(());
    }
    let want = image::open(&path).map_err(|e| format!("{name}: missing reference ({e}); run with AK_BLESS=1"))?.into_rgba8();
    let want = want.as_raw();
    let (mut sum, mut max) = (0u64, 0u8);
    for (a, b) in got.iter().zip(want) {
        let d = a.abs_diff(*b);
        sum += d as u64;
        max = max.max(d);
    }
    let mean = sum as f64 / got.len() as f64;
    if mean > 0.5 || max > 48 {
        let out = format!("{}/{name}.actual.png", std::env::temp_dir().display());
        image::save_buffer(&out, got, W, H, image::ExtendedColorType::Rgba8).unwrap();
        return Err(format!("{name}: mean diff {mean:.3}, max {max} (actual written to {out})"));
    }
    Ok(())
}

#[test]
fn golden_images() {
    let mut assets = Assets::default();
    test_image(&mut assets);
    let font = font(&mut assets);
    let mut r = Renderer::new(Arc::new(assets));
    let errs: Vec<String> =
        scenes(font).iter().filter_map(|(name, dl)| compare(name, r.render(dl).data_as_u8_slice()).err()).collect();
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

#[test]
fn layer_cache_hits_and_matches() {
    let mut r = Renderer::new(Arc::new(Assets::default()));
    let scene = |x: f32| {
        list(vec![
            layer(1.0, Blend::Normal, vec![Filter::Shadow { dx: 2.0, dy: 3.0, blur: 4.0, color: rgba(0.0, 0.0, 0.0, 0.7) }], None, Some(42)),
            Op::Fill { path: star(), paint: solid(1.0, 1.0, 1.0), rule: FillRule::NonZero, transform: IDENTITY },
            Op::PopLayer,
            Op::Fill { path: circle(x as f64, 70.0, 10.0), paint: solid(1.0, 0.2, 0.2), rule: FillRule::NonZero, transform: IDENTITY },
        ])
    };
    let first = r.render(&scene(20.0)).data_as_u8_slice().to_vec();
    assert_eq!(r.last_cache_hits(), 0);
    let second = r.render(&scene(20.0)).data_as_u8_slice().to_vec();
    assert_eq!(r.last_cache_hits(), 1);
    assert_eq!(first, second);
    // Uncached render of the same frame must match the cached one.
    let mut fresh = Renderer::new(Arc::new(Assets::default()));
    assert_eq!(fresh.render(&scene(20.0)).data_as_u8_slice(), &second[..]);
    r.render(&scene(60.0));
    assert_eq!(r.last_cache_hits(), 1);
}

#[test]
fn diff_mb_marks_only_changed_blocks() {
    use anikit_video::render::{diff::diff_mb_ops, diff_mb, MbMap, YuvFrame};
    let mut r = Renderer::new(Arc::new(Assets::default()));
    let scene = |x: f32| list(vec![card(x, 8.0, 12.0, 12.0, solid(1.0, 1.0, 1.0)), card(60.0, 60.0, 20.0, 20.0, solid(0.2, 0.9, 0.3))]);
    let (a, b) = (scene(4.0), scene(40.0));
    let (mut ya, mut yb) = (YuvFrame::default(), YuvFrame::default());
    r.render_yuv(&a, &mut ya);
    r.render_yuv(&b, &mut yb);
    let m = diff_mb(&ya, &yb);
    assert_eq!((m.mb_width, m.mb_height), (6, 6));
    assert!(m.get(0, 0) && m.get(2, 0) && m.get(3, 1));
    assert!(!m.get(4, 4) && !m.get(5, 5) && !m.get(0, 5));
    let mut c = MbMap::default();
    diff_mb_ops(&a, &b, &mut c);
    // Conservative: a superset of the exact map.
    for y in 0..6 {
        for x in 0..6 {
            assert!(!m.get(x, y) || c.get(x, y), "ops map misses ({x},{y})");
        }
    }
    assert!(c.count() < 36);
    assert_eq!(diff_mb(&ya, &ya).count(), 0);
}
