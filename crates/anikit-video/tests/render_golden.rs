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

#[test]
fn cached_image_resample_matches_direct() {
    let mut assets = Assets::default();
    let img: Vec<u8> = (0..40 * 30).flat_map(|i| [(i * 7 % 256) as u8, (i * 3 % 256) as u8, (i % 40 * 6) as u8, 255]).collect();
    assets.images.insert_rgba8(ImageId(3), 40, 30, img).unwrap();
    let mut r = Renderer::new(Arc::new(assets));
    for dst in [[10.25f32, 7.5, 60.0, 45.0], [3.0, 4.0, 20.0, 15.0], [0.0, 0.0, 40.0, 30.0]] {
        let l = list(vec![Op::Image { image: ImageId(3), dst, opacity: 0.8, transform: IDENTITY }]);
        let first = r.render(&l).data_as_u8_slice().to_vec();
        r.render(&l);
        let cached = r.render(&l).data_as_u8_slice().to_vec();
        let (mut sum, mut max) = (0u64, 0u8);
        for (a, b) in first.iter().zip(&cached) {
            sum += a.abs_diff(*b) as u64;
            max = max.max(a.abs_diff(*b));
        }
        let mean = sum as f64 / first.len() as f64;
        assert!(mean < 1.5 && max < 90, "{dst:?}: mean {mean} max {max}");
    }
}

#[test]
fn incremental_matches_full_render() {
    use anikit_video::render::incremental::Update;
    use anikit_video::render::{diff_mb, MbMap, YuvFrame};
    let mut assets = Assets::default();
    let f = font(&mut assets);
    let assets = Arc::new(assets);
    let (w, h) = (200u32, 120u32);
    let scene = |i: usize| {
        let mut ops = vec![
            Op::Fill { path: kurbo::Rect::new(0.0, 0.0, 200.0, 120.0).to_path(0.1), paint: Paint::Linear { p0: [0.0, 0.0], p1: [200.0, 0.0], stops: vec![(0.0, rgba(0.1, 0.1, 0.3, 1.0)), (1.0, rgba(0.3, 0.1, 0.2, 1.0))] }, rule: FillRule::NonZero, transform: IDENTITY },
            layer(1.0, Blend::Normal, vec![Filter::Shadow { dx: 2.0, dy: 3.0, blur: 3.0, color: rgba(0.0, 0.0, 0.0, 0.6) }], None, Some(9)),
            Op::Fill { path: star(), paint: solid(0.9, 0.9, 0.2), rule: FillRule::NonZero, transform: IDENTITY },
            Op::PopLayer,
            card(120.0 + (i as f32 * 3.7) % 50.0, 70.0, 30.0, 20.0, solid(0.2, 0.8, 0.9)),
        ];
        if i % 4 >= 2 {
            ops.push(Op::Fill { path: circle(170.0, 20.0, 8.0), paint: solid(1.0, 0.3, 0.3), rule: FillRule::NonZero, transform: IDENTITY });
        }
        if let Some(f) = f {
            let glyphs = vec![Glyph { id: 36, x: 0.0, y: 0.0 }, Glyph { id: 81, x: 14.0, y: 0.0 }];
            ops.push(Op::Glyphs { font: f, size: 18.0, glyphs, paint: solid(1.0, 1.0, 1.0), transform: [1.0, 0.0, 0.0, 1.0, 110.0, 30.0] });
        }
        if i == 5 {
            // Layer structure change → full frame.
            ops.push(layer(0.5, Blend::Normal, vec![], None, None));
            ops.push(card(10.0, 90.0, 20.0, 20.0, solid(1.0, 1.0, 1.0)));
            ops.push(Op::PopLayer);
        }
        DisplayList { width: w, height: h, background: rgba(0.0, 0.0, 0.0, 1.0), ops }
    };
    let mut inc = Renderer::new(assets.clone());
    let mut full = Renderer::new(assets.clone());
    let (mut yi, mut changed) = (YuvFrame::default(), MbMap::default());
    let mut prev_full: Option<YuvFrame> = None;
    let mut kinds = Vec::new();
    for i in 0..10 {
        let l = scene(if i == 7 { 6 } else { i }); // frame 7 repeats frame 6's list
        let l = if i == 7 { scene(6) } else { l };
        kinds.push(inc.render_yuv_incremental(&l, &mut yi, &mut changed));
        let mut yf = YuvFrame::default();
        anikit_video::render::rgba_to_yuv420(full.render(&l), &mut yf);
        // Same pixels up to float rounding (sub-region tiles see shifted coordinates).
        for (pa, pb) in [(&yi.y, &yf.y), (&yi.u, &yf.u), (&yi.v, &yf.v)] {
            let bad = pa.iter().zip(pb.iter()).filter(|(a, b)| a.abs_diff(**b) > 0).count();
            let max = pa.iter().zip(pb.iter()).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
            // Full frames over a smooth backdrop take the fused YUV path (≤ 2/4 LSB).
            assert!(max <= 4 && bad * 2 < pa.len(), "frame {i} ({:?}): {bad} diffs, max {max}", kinds[i]);
        }
        if let Some(p) = &prev_full {
            assert_eq!(changed, diff_mb(p, &yi), "frame {i}: changed map not exact");
        }
        prev_full = Some(yi.clone());
    }
    assert_eq!(kinds[0], Update::Full);
    assert!(matches!(kinds[1], Update::Partial { .. }), "{kinds:?}");
    // A layer appearing only dirties its own subtree.
    assert!(matches!(kinds[5], Update::Partial { .. }), "{kinds:?}");
    assert_eq!(kinds[7], Update::Unchanged);
}

/// Variable-font coordinates, synthetic bold and oblique change the ink.
#[test]
fn font_variations_and_synthesis() {
    use anikit_video::render::Face;
    let path = std::env::var("AK_VARFONT").unwrap_or_else(|_| format!("{}/fonts/InterVariable.ttf", env!("CARGO_MANIFEST_DIR")));
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: no variable font at {path}");
        return;
    };
    let data = vello_cpu::peniko::FontData::new(vello_cpu::peniko::Blob::from(bytes), 0);
    // Inter's axes are [opsz, wght]; wght 400 → 0.0, 900 → 1.0 (F2Dot14 16384).
    let fref = skrifa::FontRef::new(data.data.data()).unwrap();
    use skrifa::MetadataProvider;
    let axes: Vec<String> = fref.axes().iter().map(|a| a.tag().to_string()).collect();
    let wi = axes.iter().position(|t| t == "wght").expect("wght axis");
    let mut heavy = vec![0i16; axes.len()];
    heavy[wi] = 16384;
    let mut assets = Assets::default();
    let face = |coords: Vec<i16>, embolden: bool, skew: Option<f32>| Face { data: data.clone(), coords: coords.into(), embolden, skew };
    assets.fonts.insert_face(FontId(0), face(vec![], false, None));
    assets.fonts.insert_face(FontId(1), face(heavy, false, None));
    assets.fonts.insert_face(FontId(2), face(vec![], true, None));
    assets.fonts.insert_face(FontId(3), face(vec![], false, Some(12.0)));
    let mut r = Renderer::new(Arc::new(assets));
    // Inter glyph ids for a few letters, drawn both via the run cache (solid) and outlines (gradient).
    let glyphs: Vec<Glyph> = (0..6).map(|k| Glyph { id: 40 + k as u16 * 3, x: k as f32 * 14.0, y: 0.0 }).collect();
    let ink = |r: &mut Renderer, font: u32, paint: Paint| -> (u64, Vec<u8>) {
        let l = list(vec![Op::Glyphs { font: FontId(font), size: 22.0, glyphs: glyphs.clone(), paint, transform: [1.0, 0.0, 0.0, 1.0, 4.0, 50.0] }]);
        let px = r.render(&l).data_as_u8_slice().to_vec();
        let bg = (0.1f32 * 255.0 + 0.5) as u64;
        (px.chunks(4).map(|p| (p[1] as u64).saturating_sub(bg)).sum(), px)
    };
    let grad = Paint::Linear { p0: [0.0, 0.0], p1: [90.0, 0.0], stops: vec![(0.0, rgba(0.0, 1.0, 0.0, 1.0)), (1.0, rgba(0.0, 1.0, 0.0, 1.0))] };
    for paint in [solid(0.0, 1.0, 0.0), grad] {
        let (base, bp) = ink(&mut r, 0, paint.clone());
        let (bold, _) = ink(&mut r, 1, paint.clone());
        let (emb, _) = ink(&mut r, 2, paint.clone());
        let (obl, op) = ink(&mut r, 3, paint.clone());
        assert!(bold as f64 > base as f64 * 1.3, "wght 900 should add ink: {base} → {bold}");
        assert!(emb as f64 > base as f64 * 1.1, "embolden should add ink: {base} → {emb}");
        assert!((obl as f64 - base as f64).abs() < base as f64 * 0.1 && op != bp, "oblique moves ink, keeps area");
    }
}

#[test]
fn dynamic_frame_images() {
    use anikit_video::render::incremental::Update;
    use anikit_video::render::{MbMap, YuvFrame};
    let mut r = Renderer::new(Arc::new(Assets::default()));
    let frame = |v: u8| {
        let mut p = vello_cpu::Pixmap::new(32, 32);
        p.data_as_u8_slice_mut().chunks_mut(4).for_each(|c| c.copy_from_slice(&[v, v / 2, 255 - v, 255]));
        Arc::new(p)
    };
    let l = DisplayList {
        width: 128,
        height: 64,
        background: rgba(0.0, 0.0, 0.0, 1.0),
        ops: vec![
            card(70.0, 10.0, 40.0, 40.0, solid(1.0, 1.0, 1.0)),
            Op::Image { image: ImageId(77), dst: [4.0, 4.0, 48.0, 48.0], opacity: 1.0, transform: IDENTITY },
        ],
    };
    let (mut yuv, mut changed) = (YuvFrame::default(), MbMap::default());
    r.set_frame_image(ImageId(77), frame(10));
    assert_eq!(r.render_yuv_incremental(&l, &mut yuv, &mut changed), Update::Full);
    let a = r.frame().data_as_u8_slice()[(20 * 128 + 20) * 4];
    assert_eq!(r.render_yuv_incremental(&l, &mut yuv, &mut changed), Update::Unchanged);
    r.set_frame_image(ImageId(77), frame(200));
    assert!(matches!(r.render_yuv_incremental(&l, &mut yuv, &mut changed), Update::Partial { .. }));
    let b = r.frame().data_as_u8_slice()[(20 * 128 + 20) * 4];
    assert!(a < 30 && b > 180, "{a} {b}");
    assert!(changed.get(0, 0) && !changed.get(5, 1), "only the image's macroblocks change");
    // The incremental YUV equals a fresh render's (the RGBA frame may be stale
    // outside what was redrawn: plain-colour backgrounds go straight to YUV).
    let mut fresh = Renderer::new(Arc::new(Assets::default()));
    fresh.set_frame_image(ImageId(77), frame(200));
    let mut want = YuvFrame::default();
    fresh.render_yuv(&l, &mut want);
    let worst = yuv.y.iter().zip(&want.y).chain(yuv.u.iter().zip(&want.u)).chain(yuv.v.iter().zip(&want.v)).map(|(p, q)| p.abs_diff(*q)).max().unwrap();
    assert!(worst <= 2, "max YUV diff {worst}");
}

/// Degenerate input must never panic (a panicking worker kills the render).
#[test]
fn degenerate_inputs_do_not_panic() {
    use anikit_video::render::{MbMap, YuvFrame};
    let nan = f32::NAN;
    let inf = f32::INFINITY;
    let empty = BezPath::new();
    let mut wild = BezPath::new();
    wild.move_to((1e12, -1e12));
    wild.line_to((f64::NAN, 3.0));
    wild.curve_to((1e30, 1e30), (-1e30, 0.0), (5.0, 5.0));
    let mut ops = vec![];
    for t in [[nan, 0.0, 0.0, 1.0, 0.0, 0.0], [0.0; 6], [1e9, 0.0, 0.0, 1e9, -1e12, 1e12], [inf, 0.0, 0.0, inf, 0.0, 0.0], IDENTITY] {
        for p in [circle(10.0, 10.0, 5.0), empty.clone(), wild.clone()] {
            ops.push(Op::Fill { path: p.clone(), paint: Paint::Solid(rgba(nan, 2.0, -1.0, 0.5)), rule: FillRule::EvenOdd, transform: t });
            ops.push(Op::Stroke { path: p.clone(), paint: solid(1.0, 1.0, 1.0), stroke: Stroke { width: nan, dash: vec![0.0, 0.0], trim: [nan, 2.0], ..stroke(2.0) }, transform: t });
            ops.push(Op::Stroke { path: p, paint: solid(1.0, 1.0, 1.0), stroke: Stroke { dash: vec![1e-9, 1e-9], trim: [0.9, 0.1], ..stroke(1e6) }, transform: t });
        }
        ops.push(Op::RoundRect { x: nan, y: 0.0, w: 1e30, h: -5.0, radius: inf, paint: Paint::Linear { p0: [0.0, 0.0], p1: [0.0, 0.0], stops: vec![] }, transform: t });
        ops.push(Op::RoundRect { x: 0.0, y: 0.0, w: 50.0, h: 50.0, radius: 5.0, paint: Paint::Radial { c: [nan, 0.0], r: -1.0, stops: vec![(nan, rgba(1.0, 0.0, 0.0, 1.0)), (2.0, rgba(0.0, 1.0, 0.0, 1.0))] }, transform: t });
        ops.push(Op::Image { image: ImageId(999), dst: [0.0, 0.0, 10.0, 10.0], opacity: 1.0, transform: t });
        ops.push(Op::Glyphs { font: FontId(999), size: 12.0, glyphs: vec![Glyph { id: 5, x: 0.0, y: 0.0 }], paint: solid(1.0, 1.0, 1.0), transform: t });
        ops.push(layer(nan, Blend::Overlay, vec![Filter::Blur(nan), Filter::Blur(1e9), Filter::Shadow { dx: inf, dy: nan, blur: -3.0, color: rgba(0.0, 0.0, 0.0, 1.0) }, Filter::Glow { radius: 1e6, intensity: nan, color: rgba(1.0, 1.0, 1.0, 1.0) }], Some((wild.clone(), t)), Some(1)));
        ops.push(card(0.0, 0.0, 40.0, 40.0, solid(1.0, 1.0, 1.0)));
        ops.push(Op::PopLayer);
        ops.push(layer(1.0, Blend::Normal, vec![Filter::Shadow { dx: 1e9, dy: 0.0, blur: 2.0, color: rgba(0.0, 0.0, 0.0, 1.0) }], None, None));
        ops.push(Op::RoundRect { x: 0.0, y: 0.0, w: 1e7, h: 1e7, radius: 3.0, paint: solid(1.0, 1.0, 1.0), transform: t });
        ops.push(Op::PopLayer);
    }
    ops.push(Op::PopLayer); // stray
    ops.push(layer(1.0, Blend::Normal, vec![Filter::Blur(2.0)], None, None)); // unclosed
    ops.push(card(5.0, 5.0, 10.0, 10.0, solid(1.0, 0.0, 0.0)));
    let mut r = Renderer::new(Arc::new(Assets::default()));
    let (mut y, mut m) = (YuvFrame::default(), MbMap::default());
    for (w, h) in [(W, H), (1, 1), (17, 3), (0, 0)] {
        let l = DisplayList { width: w, height: h, background: rgba(nan, 0.0, 0.0, 1.0), ops: ops.clone() };
        r.render(&l);
        r.render_yuv_incremental(&l, &mut y, &mut m);
        r.render_yuv_incremental(&l, &mut y, &mut m);
    }
}

#[test]
fn dynamic_image_source_and_tiling() {
    use anikit_video::render::DynamicImages;
    let tile = {
        let mut p = vello_cpu::Pixmap::new(4, 4);
        for (i, c) in p.data_as_u8_slice_mut().chunks_mut(4).enumerate() {
            let on = (i % 4 < 2) ^ (i / 8 == 1);
            c.copy_from_slice(if on { &[255, 255, 255, 255] } else { &[0, 0, 0, 255] });
        }
        Arc::new(p)
    };
    let t2 = tile.clone();
    let src: Arc<dyn DynamicImages> = Arc::new(move |id: ImageId| (id.0 >= 0x4000_0000).then(|| t2.clone()));
    let mut r = Renderer::new(Arc::new(Assets::default()));
    r.set_image_source(src);
    let id = ImageId(0x4000_0001);
    let l = list(vec![
        Op::Image { image: id, dst: [0.0, 0.0, 4.0, 4.0], opacity: 1.0, transform: IDENTITY },
        // Tiled: a 4×4 pattern repeating across a 40×40 rect.
        Op::Fill { path: kurbo::Rect::new(40.0, 40.0, 80.0, 80.0).to_path(0.1), paint: Paint::Image { image: id, transform: IDENTITY, opacity: 1.0 }, rule: FillRule::NonZero, transform: IDENTITY },
    ]);
    let px = r.render(&l).data_as_u8_slice().to_vec();
    let at = |x: usize, y: usize| px[(y * W as usize + x) * 4];
    assert_eq!(at(0, 0), 255);
    assert_eq!(at(2, 0), 0);
    // Repeats far from the origin (Pad would smear the edge pixel).
    assert_eq!(at(72, 72), at(0, 0));
    assert_eq!(at(74, 72), at(2, 0));
    assert_eq!(at(72, 74), at(0, 2));
}

#[test]
fn backdrop_fast_path_matches_vello() {
    use anikit_video::render::{MbMap, YuvFrame};
    let mut assets = Assets::default();
    let field: Vec<u8> = (0..16 * 9).flat_map(|i| [(i * 37 % 256) as u8, (i * 11 % 256) as u8, (255 - i % 256) as u8, 255]).collect();
    assets.images.insert_rgba8(ImageId(5), 16, 9, field).unwrap();
    let assets = Arc::new(assets);
    let (w, h) = (160u32, 90u32);
    let scene = |x: f32, wrap: bool| {
        let bg = Op::Image { image: ImageId(5), dst: [0.0, 0.0, w as f32, h as f32], opacity: 1.0, transform: IDENTITY };
        let mut ops = vec![card(0.0, 0.0, 30.0, 30.0, solid(1.0, 0.0, 0.0))]; // hidden under the backdrop
        if wrap {
            ops.extend([layer(1.0, Blend::Normal, vec![], None, None), bg, Op::PopLayer]);
        } else {
            ops.push(bg);
        }
        ops.push(card(x, 30.0, 40.0, 30.0, Paint::Solid(rgba(1.0, 1.0, 1.0, 0.7))));
        DisplayList { width: w, height: h, background: rgba(0.0, 0.0, 0.0, 1.0), ops }
    };
    let mut fast = Renderer::new(assets.clone());
    // Compare like with like: vello samples bilinearly, as Draft does.
    fast.set_quality(anikit_video::render::Quality::Draft);
    let mut slow = Renderer::new(assets.clone());
    slow.set_quality(anikit_video::render::Quality::Draft);
    let (mut yuv, mut changed) = (YuvFrame::default(), MbMap::default());
    for i in 0..4 {
        let x = 20.0 + i as f32 * 9.0;
        fast.render_yuv_incremental(&scene(x, false), &mut yuv, &mut changed);
        let mut reference = YuvFrame::default();
        anikit_video::render::rgba_to_yuv420(slow.render(&scene(x, true)), &mut reference);
        for (pi, (a, b)) in [(&yuv.y, &reference.y), (&yuv.u, &reference.u), (&yuv.v, &reference.v)].into_iter().enumerate() {
            let max = a.iter().zip(b.iter()).map(|(p, q)| p.abs_diff(*q)).max().unwrap();
            let mean = a.iter().zip(b.iter()).map(|(p, q)| p.abs_diff(*q) as u64).sum::<u64>() as f64 / a.len() as f64;
            // The synthetic field is jagged (adjacent texels differ wildly), so
            // chroma point-sampled at block centres vs averaged over 2×2 differs
            // at texel kinks far more than on real (smooth) fields.
            let tol = if pi == 0 { 4 } else { 12 };
            assert!(max <= tol && mean < 1.0, "frame {i} plane {pi}: max {max} mean {mean}");
        }
    }
}

#[test]
fn smooth_backdrop_matches_full_res() {
    use anikit_video::render::{MbMap, YuvFrame};
    let (w, h) = (320u32, 180u32);
    let glow = |cx: f64, cy: f64, s: f32, a: f32| {
        let r = 200.0f32;
        Op::Fill {
            path: kurbo::Circle::new((cx, cy), (r * 0.71) as f64).to_path(0.1),
            paint: Paint::Radial { c: [cx as f32, cy as f32], r, stops: vec![(0.0, rgba(0.4, 0.3, 1.0, a)), (0.35, rgba(0.4, 0.3, 1.0, a * 0.3)), (0.7, rgba(0.4, 0.3, 1.0, 0.0))] },
            rule: FillRule::NonZero,
            transform: [s, 0.0, 0.0, s, 10.0, -5.0],
        }
    };
    let scene = |k: f32, wrap: bool| {
        let mut ops = vec![
            Op::RoundRect { x: 0.0, y: 0.0, w: w as f32, h: h as f32, radius: 0.0, paint: Paint::Radial { c: [160.0, 90.0], r: 190.0, stops: vec![(0.0, rgba(0.6, 0.5, 0.7, 1.0)), (1.0, rgba(0.4, 0.45, 0.5, 1.0))] }, transform: IDENTITY },
            glow(100.0 + k as f64 * 10.0, 80.0, 1.03, 0.4),
            glow(250.0, 120.0 - k as f64 * 5.0, 0.9, 0.3),
        ];
        if wrap {
            // A no-op layer stops the smooth prefix: reference full-res rendering.
            ops.insert(0, layer(1.0, Blend::Normal, vec![], None, None));
            ops.insert(1, Op::PopLayer);
        }
        ops.push(card(40.0 + k * 7.0, 60.0, 60.0, 40.0, solid(1.0, 1.0, 1.0)));
        DisplayList { width: w, height: h, background: rgba(0.0, 0.0, 0.0, 1.0), ops }
    };
    let assets = Arc::new(Assets::default());
    let (mut fast, mut slow) = (Renderer::new(assets.clone()), Renderer::new(assets));
    fast.set_quality(anikit_video::render::Quality::Draft);
    let (mut yuv, mut changed) = (YuvFrame::default(), MbMap::default());
    for k in 0..4 {
        let k = k as f32;
        fast.render_yuv_incremental(&scene(k, false), &mut yuv, &mut changed);
        // RGBA: low-res smooth backdrop vs full-resolution vello.
        let a = fast.render(&scene(k, false)).data_as_u8_slice().to_vec();
        let b = slow.render(&scene(k, true)).data_as_u8_slice().to_vec();
        let max = a.iter().zip(&b).map(|(p, q)| p.abs_diff(*q)).max().unwrap();
        let mean = a.iter().zip(&b).map(|(p, q)| p.abs_diff(*q) as u64).sum::<u64>() as f64 / a.len() as f64;
        assert!(max <= 6 && mean < 0.6, "frame {k}: max {max} mean {mean}");
        // YUV from the incremental path agrees with converting that RGBA.
        let mut reference = YuvFrame::default();
        anikit_video::render::rgba_to_yuv420(fast.frame(), &mut reference);
        for (p, q) in [(&yuv.y, &reference.y), (&yuv.u, &reference.u), (&yuv.v, &reference.v)] {
            let m = p.iter().zip(q.iter()).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
            assert!(m <= 4, "frame {k}: incremental YUV differs by {m}");
        }
    }
}

#[test]
fn fused_backdrop_yuv_matches_rgba_path() {
    use anikit_video::render::{rgba_to_yuv420, YuvFrame};
    let mut assets = Assets::default();
    let field: Vec<u8> = (0..20 * 12).flat_map(|i| [(i * 37 % 256) as u8, (i * 11 % 256) as u8, (255 - i % 256) as u8, 255]).collect();
    assets.images.insert_rgba8(ImageId(5), 20, 12, field).unwrap();
    let assets = Arc::new(assets);
    for (w, h) in [(160u32, 96u32), (157, 91)] {
        let image_bg = Op::Image { image: ImageId(5), dst: [0.0, 0.0, w as f32, h as f32], opacity: 1.0, transform: IDENTITY };
        let grad_bg = Op::RoundRect { x: 0.0, y: 0.0, w: w as f32, h: h as f32, radius: 0.0, paint: Paint::Radial { c: [80.0, 48.0], r: 120.0, stops: vec![(0.0, rgba(0.2, 0.1, 0.4, 1.0)), (1.0, rgba(0.0, 0.05, 0.1, 1.0))] }, transform: IDENTITY };
        for bg in [image_bg, grad_bg] {
            for top in [vec![], vec![card(37.0, 21.0, 40.0, 30.0, Paint::Solid(rgba(1.0, 1.0, 1.0, 0.8)))], vec![card(-10.0, 0.0, 400.0, 20.0, solid(1.0, 0.0, 0.0))], vec![card(4.0, 4.0, 20.0, 12.0, solid(0.0, 1.0, 0.0)), card(120.0, 70.0, 30.0, 15.0, Paint::Solid(rgba(1.0, 0.5, 0.0, 0.6)))]] {
                let mut ops = vec![bg.clone()];
                ops.extend(top);
                let l = DisplayList { width: w, height: h, background: rgba(0.0, 0.0, 0.0, 1.0), ops };
                let mut r = Renderer::new(assets.clone());
                let mut fused = YuvFrame::default();
                r.render_yuv(&l, &mut fused);
                let mut reference = YuvFrame::default();
                rgba_to_yuv420(Renderer::new(assets.clone()).render(&l), &mut reference);
                assert_eq!((fused.width, fused.padded_height), (reference.width, reference.padded_height));
                for (pi, (a, b)) in [(&fused.y, &reference.y), (&fused.u, &reference.u), (&fused.v, &reference.v)].into_iter().enumerate() {
                    let max = a.iter().zip(b.iter()).map(|(p, q)| p.abs_diff(*q)).max().unwrap();
                    // Chroma interpolated from low-res 4:4:4 vs averaged 2×2 blocks.
                    // The jagged synthetic field makes the cubic filter overshoot, which
                    // clamps differently per RGB channel than per YUV plane.
                    let tol = 8;
                    assert!(max <= tol, "{w}x{h}: fused plane {pi} differs by {max}");
                }
            }
        }
    }
}

/// Random edits (insertions, deletions, moving clip layers, filters) must keep
/// incremental frames equal to full renders.
#[test]
fn incremental_random_edits_match_full() {
    let partial: usize = (1..=6u32).map(|s| random_edits_run(s.wrapping_mul(0x9e37_79b9) | 1)).sum();
    assert!(partial >= 120, "only {partial} partial frames over 6 runs");
}

/// Soak test: `cargo test --release -p anikit-video --test render_golden -- --ignored soak`.
#[test]
#[ignore]
fn incremental_random_edits_soak() {
    for s in 1..=200u32 {
        random_edits_run(s.wrapping_mul(0x85eb_ca6b) | 1);
    }
}

/// One randomized run; returns the number of partial frames.
fn random_edits_run(seed0: u32) -> usize {
    use anikit_video::render::{rgba_to_yuv420, MbMap, YuvFrame};
    let mut seed = seed0;
    let mut rnd = move |n: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed % n
    };
    let (w, h) = (192u32, 128u32);
    // A pool of items; each frame picks a subset (in order) and perturbs some.
    let item = |k: u32, dx: f32, clip_grow: f32| -> Vec<Op> {
        let (x, y) = ((k * 37 % 150) as f32 + dx, (k * 53 % 100) as f32);
        match k % 4 {
            0 => vec![card(x, y, 30.0, 20.0, solid((k % 3) as f32 / 2.0, 0.5, 1.0))],
            1 => vec![
                layer(1.0, Blend::Normal, vec![], Some((kurbo::Rect::new(x as f64, y as f64, (x + 10.0 + clip_grow) as f64, (y + 25.0) as f64).to_path(0.1), IDENTITY)), None),
                card(x, y, 40.0, 25.0, solid(1.0, 0.8, 0.2)),
                Op::PopLayer,
            ],
            2 => vec![
                layer(0.7, Blend::Normal, vec![Filter::Shadow { dx: 2.0, dy: 3.0, blur: 3.0, color: rgba(0.0, 0.0, 0.0, 0.8) }], None, None),
                Op::Fill { path: circle(x as f64, y as f64, 9.0), paint: solid(0.2, 1.0, 0.4), rule: FillRule::NonZero, transform: IDENTITY },
                Op::PopLayer,
            ],
            _ => vec![Op::Stroke { path: wave(), paint: solid(1.0, 1.0, 1.0), stroke: stroke(2.0), transform: [1.0, 0.0, 0.0, 1.0, dx, y - 40.0] }],
        }
    };
    let mut present: Vec<bool> = (0..14).map(|k| k % 3 != 0).collect();
    let mut order: Vec<usize> = (0..14).collect();
    let mut offs = [0.0f32; 14];
    let mut grow = [0.0f32; 14];
    let assets = Arc::new(Assets::default());
    let (mut inc, mut full) = (Renderer::new(assets.clone()), Renderer::new(assets));
    let (mut yi, mut changed) = (YuvFrame::default(), MbMap::default());
    let mut prev: Option<YuvFrame> = None;
    let mut partial = 0;
    for f in 0..40 {
        for _ in 0..2 {
            let k = rnd(14) as usize;
            match rnd(5) {
                0 => present[k] = !present[k],
                1 => offs[k] += 1.0 + rnd(3) as f32,
                2 => grow[k] += 5.0 + rnd(20) as f32,
                3 => {
                    let k2 = rnd(14) as usize;
                    order.swap(k, k2);
                }
                _ => {}
            }
        }
        let mut ops = vec![Op::Fill { path: kurbo::Rect::new(0.0, 0.0, w as f64, h as f64).to_path(0.1), paint: solid(0.1, 0.1, 0.2), rule: FillRule::NonZero, transform: IDENTITY }];
        for &k in &order {
            if present[k] {
                ops.extend(item(k as u32, offs[k], grow[k]));
            }
        }
        let l = DisplayList { width: w, height: h, background: rgba(0.0, 0.0, 0.0, 1.0), ops };
        if matches!(inc.render_yuv_incremental(&l, &mut yi, &mut changed), anikit_video::render::incremental::Update::Partial { .. }) {
            partial += 1;
        }
        let mut yf = YuvFrame::default();
        rgba_to_yuv420(full.render(&l), &mut yf);
        for (pa, pb) in [(&yi.y, &yf.y), (&yi.u, &yf.u), (&yi.v, &yf.v)] {
            let max = pa.iter().zip(pb.iter()).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
            let bad = pa.iter().zip(pb.iter()).filter(|(a, b)| a != b).count();
            assert!(max <= 1 && bad * 100 < pa.len(), "seed {seed0:#x} frame {f}: max {max}, {bad} differing samples");
        }
        if let Some(p) = &prev {
            assert_eq!(changed, anikit_video::render::diff_mb(p, &yi), "seed {seed0:#x} frame {f}: changed map not exact");
        }
        prev = Some(yi.clone());
    }
    partial
}

#[test]
fn motion_blur_smears_moving_layers_only() {
    let assets = Arc::new(Assets::default());
    let mut r = Renderer::new(assets.clone());
    // A card moving 600 px/s to the right; at 30 fps with a 180° shutter it moves 10 px.
    let mut eval = |t: f32| list(vec![card(20.0 + 600.0 * t, 30.0, 20.0, 30.0, solid(1.0, 1.0, 1.0))]);
    let t = 0.05;
    let blurred = r.render_motion_blur(&mut eval, t, 1.0 / 60.0).data_as_u8_slice().to_vec();
    let sharp = Renderer::new(assets.clone()).render(&eval(t)).data_as_u8_slice().to_vec();
    // Card spans x ≈ 50..70 at t; blur spreads ±5 px: x = 47 partly lit only when blurred.
    let at = |px: &[u8], x: usize| px[(45 * W as usize + x) * 4];
    assert!(at(&blurred, 47) > at(&sharp, 47) + 20, "{} vs {}", at(&blurred, 47), at(&sharp, 47));
    assert!(at(&blurred, 72) > at(&sharp, 72) + 20);
    assert!(at(&blurred, 60) > 200, "the core stays solid");
    // Static content: exactly the plain render.
    let mut still = |_t: f32| list(vec![card(20.0, 30.0, 20.0, 30.0, solid(1.0, 1.0, 1.0))]);
    let a = r.render_motion_blur(&mut still, t, 1.0 / 60.0).data_as_u8_slice().to_vec();
    let b = Renderer::new(assets).render(&still(t)).data_as_u8_slice().to_vec();
    assert_eq!(a, b);
}

/// The fast motion-blur path (box sampling, slide smear, coarse slots) against brute force.
#[test]
fn motion_blur_matches_brute_force() {
    let assets = Arc::new(Assets::default());
    let moved = |x: f32, y: f32, a: f32| {
        let (s, c) = a.sin_cos();
        [c, s, -s, c, x, y]
    };
    let bg = || Op::RoundRect { x: -20.0, y: -20.0, w: 136.0, h: 136.0, radius: 0.0, paint: solid(0.1, 0.1, 0.2), transform: IDENTITY };
    // A slide (two cards translating together), a spin, and a card that pops in mid-shutter.
    let scenes: Vec<Box<dyn Fn(f32) -> DisplayList>> = vec![
        Box::new(move |t| {
            let x = 1500.0 * t;
            list(vec![
                bg(),
                Op::RoundRect { x: 0.0, y: 0.0, w: 30.0, h: 20.0, radius: 6.0, paint: solid(1.0, 1.0, 1.0), transform: moved(x, 20.0, 0.0) },
                Op::RoundRect { x: 0.0, y: 0.0, w: 20.0, h: 30.0, radius: 4.0, paint: solid(1.0, 0.4, 0.2), transform: moved(x + 10.0, 50.0, 0.0) },
            ])
        }),
        Box::new(move |t| list(vec![bg(), Op::RoundRect { x: -20.0, y: -6.0, w: 40.0, h: 12.0, radius: 2.0, paint: solid(1.0, 1.0, 1.0), transform: moved(48.0, 48.0, 30.0 * t) }])),
        Box::new(move |t| {
            let mut ops = vec![bg()];
            if t > 0.05 {
                ops.push(card(30.0, 30.0, 30.0, 30.0, solid(1.0, 1.0, 1.0)));
            }
            list(ops)
        }),
    ];
    for (k, scene) in scenes.iter().enumerate() {
        let mut eval = |t: f32| scene(t);
        let fast = Renderer::new(assets.clone()).render_motion_blur(&mut eval, 0.05, 1.0 / 60.0).data_as_u8_slice().to_vec();
        let mut brute = Renderer::new(assets.clone());
        brute.set_brute_force_motion(true);
        let want = brute.render_motion_blur(&mut eval, 0.05, 1.0 / 60.0).data_as_u8_slice().to_vec();
        let (mut worst, mut sum) = (0u8, 0u64);
        for (a, b) in fast.iter().zip(&want) {
            worst = worst.max(a.abs_diff(*b));
            sum += a.abs_diff(*b) as u64;
        }
        let mean = sum as f64 / fast.len() as f64;
        println!("scene {k}: max {worst}, mean {mean:.3}");
        assert!(worst <= 24 && mean < 0.5, "scene {k}: max {worst}, mean {mean:.3}");
    }
}

/// Image content (shader layers, 3D scenes) is fetched once per blurred
/// frame, not per sub-frame sample, even when it changes over the shutter.
#[test]
fn motion_blur_fetches_image_content_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let assets = Arc::new(Assets::default());
    let mut r = Renderer::new(assets);
    let fetched = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    let calls = Arc::new(AtomicUsize::new(0));
    let (f2, c2) = (fetched.clone(), calls.clone());
    r.set_image_source(Arc::new(move |id: ImageId| {
        c2.fetch_add(1, Ordering::Relaxed);
        f2.lock().unwrap().insert(id.0);
        let mut p = vello_cpu::Pixmap::new(16, 16);
        p.data_as_u8_slice_mut().fill(200);
        Some(Arc::new(p))
    }));
    // A "shader" whose frame id changes with time, moving fast, plus a moving card.
    let mut eval = |t: f32| {
        let x = 1200.0 * t;
        list(vec![
            Op::Image { image: ImageId(0x7000_0000 + (t * 1e4) as u32), dst: [0.0, 0.0, 16.0, 16.0], opacity: 1.0, transform: [1.0, 0.0, 0.0, 1.0, x, 40.0] },
            card(x, 10.0, 20.0, 20.0, solid(1.0, 1.0, 1.0)),
        ])
    };
    r.render_motion_blur(&mut eval, 0.05, 1.0 / 60.0);
    assert_eq!(fetched.lock().unwrap().len(), 1, "image content fetched for {:?}", fetched.lock().unwrap());
}

/// Output depends only on the display list, never on what the renderer drew
/// before (cache fills are keyed on everything that affects their pixels).
#[test]
fn output_is_independent_of_render_history() {
    let assets = Arc::new(Assets::default());
    let shadowed = |w: f32, x: f32| {
        list(vec![
            Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Shadow { dx: 0.0, dy: 4.0, blur: 6.0, color: rgba(0.0, 0.0, 0.0, 0.5) }], clip: None, cache_key: None },
            card(x, 30.0, w, 30.0, solid(0.9, 0.9, 0.95)),
            Op::PopLayer,
        ])
    };
    // A, B and C differ by less than the caches' quantisation steps.
    let (a, b, c) = (shadowed(40.01, 20.0), shadowed(40.03, 20.3), shadowed(40.05, 20.6));
    let render_after = |prev: &DisplayList| {
        let mut r = Renderer::new(assets.clone());
        r.render(prev);
        r.invalidate();
        r.render(&b).data_as_u8_slice().to_vec()
    };
    let fresh = Renderer::new(assets.clone()).render(&b).data_as_u8_slice().to_vec();
    assert!(render_after(&a) == fresh, "B after A differs from a fresh render");
    assert!(render_after(&c) == fresh, "B after C differs from a fresh render");
}

/// Trailing grain (tiled image under Overlay / Soft Light) blended straight
/// into the frame matches vello drawing the same layers.
#[test]
fn grain_fast_path_matches_vello() {
    let mut assets = Assets::default();
    let mut img = image::RgbaImage::new(8, 8);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let g = ((x * 37 + y * 91) % 256) as u8;
        *p = image::Rgba([g, g, g, 255]);
    }
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    assets.images.insert_encoded(ImageId(7), &png).unwrap();
    let assets = Arc::new(assets);
    for blend in [Blend::Overlay, Blend::SoftLight] {
        let mut ops = vec![
            Op::RoundRect { x: 0.0, y: 0.0, w: 96.0, h: 96.0, radius: 0.0, paint: Paint::Linear { p0: [0.0, 0.0], p1: [96.0, 96.0], stops: vec![(0.0, rgba(0.05, 0.1, 0.3, 1.0)), (1.0, rgba(0.95, 0.8, 0.6, 1.0))] }, transform: IDENTITY },
            card(20.0, 20.0, 40.0, 30.0, solid(1.0, 1.0, 1.0)),
            layer(0.7, blend, vec![], None, None),
            Op::RoundRect { x: 0.0, y: 0.0, w: 96.0, h: 96.0, radius: 0.0, paint: Paint::Image { image: ImageId(7), transform: [1.0, 0.0, 0.0, 1.0, 3.0, 5.0], opacity: 1.0 }, transform: IDENTITY },
            Op::PopLayer,
        ];
        let fast = Renderer::new(assets.clone()).render(&list(ops.clone())).data_as_u8_slice().to_vec();
        // A trailing transparent op: not a grain tail any more, so vello draws it.
        ops.push(card(0.0, 0.0, 1.0, 1.0, Paint::Solid(rgba(0.0, 0.0, 0.0, 0.0))));
        let slow = Renderer::new(assets.clone()).render(&list(ops)).data_as_u8_slice().to_vec();
        let worst = fast.iter().zip(&slow).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        let plain = Renderer::new(assets.clone()).render(&list(vec![])).data_as_u8_slice().to_vec();
        assert!(fast != plain);
        let over = fast.iter().zip(&slow).filter(|(a, b)| a.abs_diff(**b) > 1).count();
        let mean = fast.iter().zip(&slow).map(|(a, b)| a.abs_diff(*b) as f64).sum::<f64>() / fast.len() as f64;
        println!("{blend:?}: max {worst}, mean {mean:.3}, >1: {over}");
        // vello rounds its intermediate layer to 8 bits; one float pass differs by a few levels at most.
        assert!(worst <= 4 && mean < 0.5, "{blend:?}: max diff {worst}, mean {mean:.3}");
    }
}

/// Strokes reaching far off-screen are clipped to the view first: the
/// visible pixels match the same lines ending just off-frame.
#[test]
fn long_strokes_clip_to_the_view() {
    let assets = Arc::new(Assets::default());
    let stroke = |far: f64| {
        let mut p = BezPath::new();
        for k in 0..6 {
            let y = 10.0 + k as f64 * 15.0;
            p.move_to((48.0, y));
            p.line_to((48.0 - 100.0 * far, y + 30.0 * far));
        }
        list(vec![Op::Stroke { path: p, paint: solid(1.0, 0.8, 0.3), stroke: Stroke { width: 2.5, cap: 0, join: 0, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY }])
    };
    // Ending just off-frame: drawn as is.
    let near = Renderer::new(assets.clone()).render(&stroke(1.0)).data_as_u8_slice().to_vec();
    // The same lines, 4000× longer: clipped before rasterising.
    let far_list = stroke(4000.0);
    let far = Renderer::new(assets.clone()).render(&far_list).data_as_u8_slice().to_vec();
    let worst = near.iter().zip(&far).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(near.iter().zip(&Renderer::new(assets).render(&list(vec![])).data_as_u8_slice().to_vec()).any(|(a, b)| a != b));
    assert!(worst <= 1, "max diff {worst}");
}

/// A plain background colour acts as a backdrop (no full-frame clear and
/// convert): the same pixels and YUV as vello clearing to it.
#[test]
fn solid_background_backdrop_matches_clear() {
    use anikit_video::render::{Quality, YuvFrame};
    let assets = Arc::new(Assets::default());
    let mut l = list(vec![card(20.0, 30.0, 40.0, 25.0, solid(0.9, 0.4, 0.2)), card(10.0, 70.0, 70.0, 10.0, Paint::Solid(rgba(1.0, 1.0, 1.0, 0.5)))]);
    l.background = rgba(0.137, 0.52, 0.81, 1.0);
    let mut exact = Renderer::new(assets.clone());
    exact.set_quality(Quality::Reference);
    let want = exact.render(&l).data_as_u8_slice().to_vec();
    let got = Renderer::new(assets.clone()).render(&l).data_as_u8_slice().to_vec();
    // Identical, up to vello's rounding of exact halves (SIMD builds round
    // source-over onto existing pixels and onto its own clear differently).
    let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 1, "max RGBA diff {worst}");
    let (mut a, mut b) = (YuvFrame::new(W, H), YuvFrame::new(W, H));
    Renderer::new(assets.clone()).render_yuv(&l, &mut a);
    exact.render_yuv(&l, &mut b);
    let worst = a.y.iter().zip(&b.y).chain(a.u.iter().zip(&b.u)).chain(a.v.iter().zip(&b.v)).map(|(p, q)| p.abs_diff(*q)).max().unwrap();
    assert!(worst <= 1, "max YUV diff {worst}");
}
