//! Display list → pixels, on vello_cpu.
//!
//! One [`Renderer`] per worker thread; it owns a vello `RenderContext`, its
//! `Resources`, the output pixmap and all scratch memory, and reuses them
//! across frames. [`Assets`] (images, fonts) are shared read-only.
//!
//! Layers without filters or a `cache_key` map onto vello's native layers.
//! Layers with filters (or a `cache_key`) render offscreen into a pixmap
//! covering only their (filter-expanded) bounds, are filtered there, and are
//! composited as an image; a `cache_key` keeps that bitmap for the next frame.

pub mod assets;
pub mod diff;
pub mod filter;
pub mod trim;
pub mod yuv;

use std::collections::HashMap;
use std::sync::Arc;

use kurbo::{Affine, BezPath, Rect, RoundedRect, Shape};
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::peniko::{
    BlendMode, ColorStop, ColorStops, Compose, Extend, Fill, Gradient, ImageQuality, ImageSampler,
    LinearGradientPosition, Mix, RadialGradientPosition,
};
use vello_cpu::{Image, ImageSource, Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};

pub use assets::{Assets, FontStore, ImageError, ImageStore};
pub use diff::{diff_mb, MbMap};
pub use yuv::{rgba_to_yuv420, YuvFrame};

use crate::dl::{self, Blend, DisplayList, FillRule, Op, Paint, Rgba};

/// A rendered frame: premultiplied RGBA8, `width × height`, tightly packed.
pub type FrameRgba = Pixmap;

fn affine(a: &dl::Affine) -> Affine {
    Affine::new(a.map(|v| v as f64))
}

fn color(c: Rgba) -> AlphaColor<Srgb> {
    AlphaColor::new(c.0)
}

fn blend_mode(b: Blend) -> BlendMode {
    let mix = match b {
        Blend::Normal | Blend::Add => Mix::Normal,
        Blend::Multiply => Mix::Multiply,
        Blend::Screen => Mix::Screen,
        Blend::Overlay => Mix::Overlay,
        Blend::Darken => Mix::Darken,
        Blend::Lighten => Mix::Lighten,
        Blend::SoftLight => Mix::SoftLight,
        Blend::Difference => Mix::Difference,
    };
    let compose = if b == Blend::Add { Compose::Plus } else { Compose::SrcOver };
    BlendMode::new(mix, compose)
}

struct CacheEntry {
    pix: Arc<Pixmap>,
    x: i32,
    y: i32,
    last_used: u64,
}

/// Everything except the main context (so draw code can borrow both).
struct State {
    res: Resources,
    pool: Vec<RenderContext>,
    pix_pool: Vec<Pixmap>,
    in_flight: Vec<Arc<Pixmap>>,
    cache: HashMap<u64, CacheEntry>,
    frame_no: u64,
    path: BezPath,
    lens: Vec<f64>,
    filters: filter::FilterScratch,
    /// Layers rendered from cache in the last frame (for stats/tests).
    cache_hits: usize,
}

/// Renders display lists. Not `Sync`: create one per worker thread.
pub struct Renderer {
    assets: Arc<Assets>,
    ctx: RenderContext,
    frame: Pixmap,
    st: State,
}

/// Device-space rect of the pixels `r` touches, rounded out.
fn round_out(r: Rect) -> Rect {
    Rect::new(r.x0.floor(), r.y0.floor(), r.x1.ceil(), r.y1.ceil())
}

impl Renderer {
    pub fn new(assets: Arc<Assets>) -> Self {
        Self {
            assets,
            ctx: RenderContext::new(1, 1),
            frame: Pixmap::new(1, 1),
            st: State {
                res: Resources::new(),
                pool: Vec::new(),
                pix_pool: Vec::new(),
                in_flight: Vec::new(),
                cache: HashMap::new(),
                frame_no: 0,
                path: BezPath::new(),
                lens: Vec::new(),
                filters: Default::default(),
                cache_hits: 0,
            },
        }
    }

    pub fn assets(&self) -> &Arc<Assets> {
        &self.assets
    }

    /// Number of cached layers composited without re-rendering in the last frame.
    pub fn last_cache_hits(&self) -> usize {
        self.st.cache_hits
    }

    /// The last rendered frame.
    pub fn frame(&self) -> &FrameRgba {
        &self.frame
    }

    /// Render `list` and return the frame (premultiplied RGBA8).
    pub fn render(&mut self, list: &DisplayList) -> &FrameRgba {
        let (w, h) = (list.width.clamp(1, u16::MAX as u32) as u16, list.height.clamp(1, u16::MAX as u32) as u16);
        if self.ctx.width() != w || self.ctx.height() != h {
            self.ctx = RenderContext::new(w, h);
            self.frame = Pixmap::new(w, h);
            self.st.cache.clear();
        }
        let st = &mut self.st;
        st.frame_no += 1;
        st.cache_hits = 0;
        self.ctx.reset();
        let assets = self.assets.clone();
        draw_ops(st, &assets, &mut self.ctx, &list.ops, Affine::IDENTITY);
        self.ctx.flush();
        let settings = RasterizerSettings { target_init: TargetInit::Clear(color(list.background)), ..Default::default() };
        self.ctx.render_with(&mut self.frame, &mut st.res, settings);
        // Release the paints that referenced offscreen bitmaps, then recycle them.
        self.ctx.reset();
        for a in st.in_flight.drain(..) {
            if let Ok(p) = Arc::try_unwrap(a) {
                st.pix_pool.push(p);
            }
        }
        let now = st.frame_no;
        st.cache.retain(|_, e| e.last_used + 1 >= now);
        &self.frame
    }

    /// Render and convert to YUV 4:2:0 (BT.709 limited range) in one call.
    pub fn render_yuv(&mut self, list: &DisplayList, out: &mut YuvFrame) {
        self.render(list);
        rgba_to_yuv420(&self.frame, out);
    }
}

/// Index of the `PopLayer` matching the `PushLayer` at `ops[0]`, or `ops.len()`.
fn matching_pop(ops: &[Op]) -> usize {
    let mut depth = 0usize;
    for (i, op) in ops.iter().enumerate() {
        match op {
            Op::PushLayer { .. } => depth += 1,
            Op::PopLayer => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    ops.len()
}

fn draw_ops(st: &mut State, assets: &Assets, ctx: &mut RenderContext, ops: &[Op], base: Affine) {
    let mut i = 0;
    while i < ops.len() {
        match &ops[i] {
            Op::PushLayer { opacity, blend, filters, clip, cache_key } => {
                let end = i + matching_pop(&ops[i..]);
                let inner = &ops[i + 1..end.min(ops.len())];
                i = end + 1;
                let opacity = opacity.clamp(0.0, 1.0);
                if opacity <= 0.0 || inner.is_empty() {
                    continue;
                }
                if !filters.is_empty() || cache_key.is_some() {
                    draw_offscreen(st, assets, ctx, inner, base, opacity, *blend, filters, clip.as_ref(), *cache_key);
                    continue;
                }
                let trivial = opacity >= 1.0 && *blend == Blend::Normal && clip.is_none();
                if !trivial {
                    push_layer(ctx, base, clip.as_ref(), *blend, opacity);
                }
                draw_ops(st, assets, ctx, inner, base);
                if !trivial {
                    ctx.pop_layer();
                }
            }
            Op::PopLayer => i += 1, // stray pop: ignore
            op => {
                draw_op(st, assets, ctx, op, base);
                i += 1;
            }
        }
    }
}

fn push_layer(ctx: &mut RenderContext, base: Affine, clip: Option<&(dl::Path, dl::Affine)>, blend: Blend, opacity: f32) {
    ctx.set_fill_rule(Fill::NonZero);
    match clip {
        Some((p, t)) => {
            ctx.set_transform(base * affine(t));
            ctx.push_layer(Some(p), Some(blend_mode(blend)), Some(opacity), None, None);
        }
        None => {
            ctx.set_transform(base);
            ctx.push_layer(None, Some(blend_mode(blend)), Some(opacity), None, None);
        }
    }
}

/// Conservative device-space bounds of `ops` (with nested filter margins added).
pub(crate) fn ops_bounds(ops: &[Op], base: Affine) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    let mut extra = [0f32; 4];
    for op in ops {
        let r = match op {
            Op::Fill { path, transform, .. } => Some((base * affine(transform)).transform_rect_bbox(path.bounding_box())),
            Op::Stroke { path, transform, stroke, .. } => {
                let t = base * affine(transform);
                let s = t.as_coeffs();
                let scale = (s[0].abs() + s[1].abs()).max(s[2].abs() + s[3].abs());
                let k = if stroke.join == 0 { stroke.miter_limit.max(1.5) } else { 1.5 } as f64;
                Some(t.transform_rect_bbox(path.bounding_box()).inflate(
                    stroke.width as f64 * 0.5 * k * scale,
                    stroke.width as f64 * 0.5 * k * scale,
                ))
            }
            Op::RoundRect { x, y, w, h, transform, .. } => Some(
                (base * affine(transform)).transform_rect_bbox(Rect::new(*x as f64, *y as f64, (*x + *w) as f64, (*y + *h) as f64)),
            ),
            Op::Image { dst, transform, .. } => Some((base * affine(transform)).transform_rect_bbox(Rect::new(
                dst[0] as f64,
                dst[1] as f64,
                (dst[0] + dst[2]) as f64,
                (dst[1] + dst[3]) as f64,
            ))),
            Op::Glyphs { size, glyphs, transform, .. } => {
                let mut r: Option<Rect> = None;
                for g in glyphs {
                    let gr = Rect::new(g.x as f64, g.y as f64, g.x as f64, g.y as f64);
                    r = Some(r.map_or(gr, |r| r.union(gr)));
                }
                let s = *size as f64;
                r.map(|r| {
                    let r = Rect::new(r.x0 - s, r.y0 - 1.5 * s, r.x1 + 2.0 * s, r.y1 + 0.75 * s);
                    (base * affine(transform)).transform_rect_bbox(r)
                })
            }
            Op::PushLayer { filters, .. } => {
                for f in filters {
                    let m = filter::margin(f);
                    for k in 0..4 {
                        extra[k] += m[k];
                    }
                }
                None
            }
            Op::PopLayer => None,
        };
        if let Some(r) = r {
            acc = Some(acc.map_or(r, |a| a.union(r)));
        }
    }
    acc.map(|r| {
        Rect::new(
            r.x0 - extra[0] as f64 - 1.0,
            r.y0 - extra[1] as f64 - 1.0,
            r.x1 + extra[2] as f64 + 1.0,
            r.y1 + extra[3] as f64 + 1.0,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn draw_offscreen(
    st: &mut State,
    assets: &Assets,
    ctx: &mut RenderContext,
    inner: &[Op],
    base: Affine,
    opacity: f32,
    blend: Blend,
    filters: &[dl::Filter],
    clip: Option<&(dl::Path, dl::Affine)>,
    cache_key: Option<u64>,
) {
    let now = st.frame_no;
    let cached = cache_key.and_then(|k| st.cache.get_mut(&k)).map(|e| {
        e.last_used = now;
        (e.pix.clone(), e.x, e.y)
    });
    let (pix, x, y) = match cached {
        Some(c) => {
            st.cache_hits += 1;
            c
        }
        None => {
            let Some(content) = ops_bounds(inner, base) else { return };
            let mut m = [0f32; 4];
            for f in filters {
                let fm = filter::margin(f);
                for k in 0..4 {
                    m[k] += fm[k];
                }
            }
            let grow = |r: Rect| Rect::new(r.x0 - m[0] as f64, r.y0 - m[1] as f64, r.x1 + m[2] as f64, r.y1 + m[3] as f64);
            let view = Rect::new(0.0, 0.0, ctx.width() as f64, ctx.height() as f64);
            // Pixels outside the view can still blur or cast shadows into it.
            let mut r = grow(content).intersect(grow(view));
            if let Some((p, t)) = clip {
                r = r.intersect((base * affine(t)).transform_rect_bbox(p.bounding_box()).inflate(1.0, 1.0));
            }
            let r = round_out(r);
            if r.width() < 1.0 || r.height() < 1.0 || r.width() > u16::MAX as f64 || r.height() > u16::MAX as f64 {
                return;
            }
            let (w, h) = (r.width() as u16, r.height() as u16);
            let mut sub = st.pool.pop().unwrap_or_else(|| RenderContext::new(w, h));
            sub.reset_and_resize(w, h);
            draw_ops(st, assets, &mut sub, inner, Affine::translate((-r.x0, -r.y0)) * base);
            sub.flush();
            let mut pm = st.pix_pool.pop().unwrap_or_else(|| Pixmap::new(w, h));
            pm.resize(w, h);
            sub.render_with(&mut pm, &mut st.res, RasterizerSettings::default());
            sub.reset();
            st.pool.push(sub);
            for f in filters {
                filter::apply(f, pm.data_as_u8_slice_mut(), w as usize, h as usize, &mut st.filters);
            }
            pm.set_may_have_transparency(true);
            let pix = Arc::new(pm);
            let (x, y) = (r.x0 as i32, r.y0 as i32);
            match cache_key {
                Some(k) => {
                    st.cache.insert(k, CacheEntry { pix: pix.clone(), x, y, last_used: now });
                }
                None => st.in_flight.push(pix.clone()),
            }
            (pix, x, y)
        }
    };
    let layered = clip.is_some() || blend != Blend::Normal;
    if layered {
        push_layer(ctx, base, clip, blend, opacity);
    }
    let (w, h) = (pix.width() as f64, pix.height() as f64);
    ctx.set_transform(Affine::translate((x as f64, y as f64)));
    ctx.reset_paint_transform();
    ctx.set_paint(Image {
        image: ImageSource::Pixmap(pix),
        sampler: ImageSampler {
            x_extend: Extend::Pad,
            y_extend: Extend::Pad,
            quality: ImageQuality::Low,
            alpha: if layered { 1.0 } else { opacity },
        },
    });
    ctx.fill_rect(&Rect::new(0.0, 0.0, w, h));
    if layered {
        ctx.pop_layer();
    }
}

fn stops(stops: &[(f32, Rgba)]) -> ColorStops {
    ColorStops::from(stops.iter().map(|(o, c)| ColorStop::from((*o, color(*c)))).collect::<Vec<_>>().as_slice())
}

/// Set `paint` on `ctx`. Returns false when there is nothing to draw.
fn set_paint(ctx: &mut RenderContext, assets: &Assets, paint: &Paint) -> bool {
    ctx.reset_paint_transform();
    match paint {
        Paint::Solid(c) => {
            if c.0[3] <= 0.0 {
                return false;
            }
            ctx.set_paint(color(*c));
        }
        Paint::Linear { stops: s, .. } | Paint::Radial { stops: s, .. } if s.len() < 2 => match s.first() {
            Some((_, c)) if c.0[3] > 0.0 => ctx.set_paint(color(*c)),
            _ => return false,
        },
        Paint::Linear { p0, p1, stops: s } => {
            ctx.set_paint(Gradient {
                kind: LinearGradientPosition {
                    start: (p0[0] as f64, p0[1] as f64).into(),
                    end: (p1[0] as f64, p1[1] as f64).into(),
                }
                .into(),
                stops: stops(s),
                extend: Extend::Pad,
                ..Default::default()
            });
        }
        Paint::Radial { c, r, stops: s } => {
            ctx.set_paint(Gradient {
                kind: RadialGradientPosition {
                    start_center: (c[0] as f64, c[1] as f64).into(),
                    start_radius: 0.0,
                    end_center: (c[0] as f64, c[1] as f64).into(),
                    end_radius: r.max(1e-3),
                }
                .into(),
                stops: stops(s),
                extend: Extend::Pad,
                ..Default::default()
            });
        }
        Paint::Image { image, transform, opacity } => {
            let Some(pix) = assets.images.get(*image) else { return false };
            let Some(inv) = invert(affine(transform)) else { return false };
            if *opacity <= 0.0 {
                return false;
            }
            ctx.set_paint(Image {
                image: ImageSource::Pixmap(pix.clone()),
                sampler: ImageSampler { alpha: opacity.min(1.0), quality: ImageQuality::Medium, ..Default::default() },
            });
            ctx.set_paint_transform(inv);
        }
    }
    true
}

fn invert(a: Affine) -> Option<Affine> {
    if a.determinant().abs() < 1e-12 {
        None
    } else {
        Some(a.inverse())
    }
}

fn draw_op(st: &mut State, assets: &Assets, ctx: &mut RenderContext, op: &Op, base: Affine) {
    match op {
        Op::Fill { path, paint, rule, transform } => {
            if !set_paint(ctx, assets, paint) {
                return;
            }
            ctx.set_transform(base * affine(transform));
            ctx.set_fill_rule(match rule {
                FillRule::NonZero => Fill::NonZero,
                FillRule::EvenOdd => Fill::EvenOdd,
            });
            ctx.fill_path(path);
        }
        Op::Stroke { path, paint, stroke, transform } => {
            if stroke.width <= 0.0 || !set_paint(ctx, assets, paint) {
                return;
            }
            let mut ks = kurbo::Stroke::new(stroke.width as f64)
                .with_caps(match stroke.cap {
                    1 => kurbo::Cap::Round,
                    2 => kurbo::Cap::Square,
                    _ => kurbo::Cap::Butt,
                })
                .with_join(match stroke.join {
                    1 => kurbo::Join::Round,
                    2 => kurbo::Join::Bevel,
                    _ => kurbo::Join::Miter,
                })
                .with_miter_limit(stroke.miter_limit.max(1.0) as f64);
            let dash_sum: f32 = stroke.dash.iter().map(|d| d.max(0.0)).sum();
            if !stroke.dash.is_empty() && dash_sum > 1e-3 {
                let reps = if stroke.dash.len() % 2 == 1 { 2 } else { 1 };
                let pat = (0..reps).flat_map(|_| stroke.dash.iter().map(|d| d.max(0.0) as f64));
                ks = ks.with_dashes(stroke.dash_offset as f64, pat);
            }
            ctx.set_stroke(ks);
            ctx.set_transform(base * affine(transform));
            let [t0, t1] = stroke.trim;
            if t0 <= 0.0 && t1 >= 1.0 {
                ctx.stroke_path(path);
            } else {
                trim::trim_path(path, t0, t1, &mut st.path, &mut st.lens);
                if !st.path.is_empty() {
                    ctx.stroke_path(&st.path);
                }
            }
        }
        Op::RoundRect { x, y, w, h, radius, paint, transform } => {
            if *w <= 0.0 || *h <= 0.0 || !set_paint(ctx, assets, paint) {
                return;
            }
            ctx.set_transform(base * affine(transform));
            let rect = Rect::new(*x as f64, *y as f64, (*x + *w) as f64, (*y + *h) as f64);
            let r = radius.clamp(0.0, w.min(*h) * 0.5) as f64;
            if r <= 0.0 {
                ctx.fill_rect(&rect);
            } else {
                ctx.set_fill_rule(Fill::NonZero);
                st.path.truncate(0);
                st.path.extend(RoundedRect::from_rect(rect, r).path_elements(0.1));
                ctx.fill_path(&st.path);
            }
        }
        Op::Image { image, dst, opacity, transform } => {
            let Some(pix) = assets.images.get(*image) else { return };
            if *opacity <= 0.0 || dst[2] <= 0.0 || dst[3] <= 0.0 {
                return;
            }
            let (iw, ih) = (pix.width() as f64, pix.height() as f64);
            ctx.set_transform(base * affine(transform));
            ctx.set_paint(Image {
                image: ImageSource::Pixmap(pix.clone()),
                sampler: ImageSampler { alpha: opacity.min(1.0), quality: ImageQuality::Medium, ..Default::default() },
            });
            ctx.set_paint_transform(
                Affine::translate((dst[0] as f64, dst[1] as f64)) * Affine::scale_non_uniform(dst[2] as f64 / iw, dst[3] as f64 / ih),
            );
            ctx.fill_rect(&Rect::new(dst[0] as f64, dst[1] as f64, (dst[0] + dst[2]) as f64, (dst[1] + dst[3]) as f64));
        }
        Op::Glyphs { font, size, glyphs, paint, transform } => {
            let Some(font) = assets.fonts.get(*font) else { return };
            if glyphs.is_empty() || *size <= 0.0 || !set_paint(ctx, assets, paint) {
                return;
            }
            ctx.set_transform(base * affine(transform));
            ctx.set_fill_rule(Fill::NonZero);
            let _ = ctx
                .glyph_run(&mut st.res, font)
                .font_size(*size)
                .hint(false)
                .fill_glyphs(glyphs.iter().map(|g| vello_cpu::Glyph { id: g.id as u32, x: g.x, y: g.y }));
        }
        Op::PushLayer { .. } | Op::PopLayer => {}
    }
}
