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
pub mod blur;
pub mod diff;
pub mod filter;
pub mod glyphs;
pub mod incremental;
pub mod resample;
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

pub use assets::{Assets, Face, FontStore, ImageError, ImageStore};
pub use diff::{diff_mb, MbMap};
pub use yuv::{rgba_to_yuv420, YuvFrame};

use crate::dl::{self, Blend, DisplayList, FillRule, Op, Paint, Rgba};

/// Shared assets plus this renderer's per-frame image overrides.
pub(crate) struct View<'a> {
    assets: &'a Assets,
    frame: &'a HashMap<dl::ImageId, Arc<Pixmap>>,
}

impl View<'_> {
    fn image(&self, id: dl::ImageId) -> Option<&Arc<Pixmap>> {
        self.frame.get(&id).or_else(|| self.assets.images.get(id))
    }
    fn face(&self, id: dl::FontId) -> Option<&Face> {
        self.assets.fonts.face(id)
    }
}

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
    sprites: HashMap<u64, CacheEntry>,
    text: HashMap<u64, CacheEntry>,
    scaled: HashMap<u64, CacheEntry>,
    seen: HashMap<u64, u64>,
    resample_buf: Vec<u8>,
    /// Frame-space position of the current context's pixel (0, 0).
    org: (i32, i32),
    frame_size: (u32, u32),
    frame_no: u64,
    path: BezPath,
    lens: Vec<f64>,
    filters: filter::FilterScratch,
    glyphs: glyphs::GlyphCache,
    run: BezPath,
    /// Layers rendered from cache in the last frame (for stats/tests).
    cache_hits: usize,
}

/// Renders display lists. Not `Sync`: create one per worker thread.
pub struct Renderer {
    assets: Arc<Assets>,
    ctx: RenderContext,
    frame: Pixmap,
    /// Context and pixmap for partial (dirty-region) renders.
    rctx: Option<RenderContext>,
    rpix: Pixmap,
    /// Per-frame image overrides (video frames, live textures).
    frame_images: Arc<HashMap<dl::ImageId, Arc<Pixmap>>>,
    inc: incremental::Tracker,
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
            rctx: None,
            rpix: Pixmap::new(1, 1),
            frame_images: Default::default(),
            inc: Default::default(),
            st: State {
                res: Resources::new(),
                pool: Vec::new(),
                pix_pool: Vec::new(),
                in_flight: Vec::new(),
                cache: HashMap::new(),
                sprites: HashMap::new(),
                text: HashMap::new(),
                scaled: HashMap::new(),
                seen: HashMap::new(),
                resample_buf: Vec::new(),
                org: (0, 0),
                frame_size: (1, 1),
                frame_no: 0,
                path: BezPath::new(),
                lens: Vec::new(),
                filters: Default::default(),
                glyphs: Default::default(),
                run: BezPath::new(),
                cache_hits: 0,
            },
        }
    }

    pub fn assets(&self) -> &Arc<Assets> {
        &self.assets
    }

    /// Dynamic-image hook: draw `pixmap` (premultiplied RGBA8) for `id` in the
    /// following renders, overriding [`Assets::images`] — for decoded video
    /// frames or other per-frame textures. Set it before each frame whose
    /// image changes; incremental renders notice the new pixels (identity of
    /// the `Arc`), so give each new frame its own `Arc`.
    pub fn set_frame_image(&mut self, id: dl::ImageId, pixmap: Arc<Pixmap>) {
        Arc::make_mut(&mut self.frame_images).insert(id, pixmap);
    }

    /// Drop one per-frame override (falls back to the shared store).
    pub fn clear_frame_image(&mut self, id: dl::ImageId) {
        Arc::make_mut(&mut self.frame_images).remove(&id);
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
        self.render_region(list, None, true, None);
        &self.frame
    }

    /// Render all of `list`, or only the pixels of `region` (`[x0, y0, x1, y1)`,
    /// clipped to the frame) — the rest of the frame keeps its previous pixels.
    pub(crate) fn render_region(&mut self, list: &DisplayList, region: Option<[u32; 4]>, new_frame: bool, rects: Option<&[Option<Rect>]>) {
        let (w, h) = (list.width.clamp(1, u16::MAX as u32) as u16, list.height.clamp(1, u16::MAX as u32) as u16);
        if self.ctx.width() != w || self.ctx.height() != h {
            self.ctx = RenderContext::new(w, h);
            self.frame = Pixmap::new(w, h);
            self.st.cache.clear();
        }
        let region = region.map(|[x0, y0, x1, y1]| [x0.min(w as u32), y0.min(h as u32), x1.min(w as u32), y1.min(h as u32)]);
        let full = match region {
            None => true,
            Some([x0, y0, x1, y1]) => x0 == 0 && y0 == 0 && x1 == w as u32 && y1 == h as u32,
        };
        let st = &mut self.st;
        if new_frame {
            st.frame_no += 1;
            st.cache_hits = 0;
        }
        st.frame_size = (w as u32, h as u32);
        let shared = self.assets.clone();
        let frame_images = self.frame_images.clone();
        let assets = View { assets: &shared, frame: &frame_images };
        let settings = RasterizerSettings { target_init: TargetInit::Clear(color(list.background)), ..Default::default() };
        if full {
            st.org = (0, 0);
            self.ctx.reset();
            draw_ops(st, &assets, &mut self.ctx, &list.ops, Affine::IDENTITY);
            self.ctx.flush();
            self.ctx.render_with(&mut self.frame, &mut st.res, settings);
            // Release the paints that referenced offscreen bitmaps.
            self.ctx.reset();
        } else {
            let [x0, y0, x1, y1] = region.unwrap_or_default();
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            let (rw, rh) = ((x1 - x0) as u16, (y1 - y0) as u16);
            let rctx = self.rctx.get_or_insert_with(|| RenderContext::new(rw, rh));
            rctx.reset_and_resize(rw, rh);
            st.org = (x0 as i32, y0 as i32);
            let cull = rects.map(|rects| Cull { region: Rect::new(0.0, 0.0, rw as f64, rh as f64), rects, org: (x0 as f64, y0 as f64) });
            draw_ops_culled(st, &assets, rctx, &list.ops, Affine::translate((-(x0 as f64), -(y0 as f64))), cull);
            rctx.flush();
            self.rpix.resize(rw, rh);
            rctx.render_with(&mut self.rpix, &mut st.res, settings);
            rctx.reset();
            st.org = (0, 0);
            let (fw, rwb) = (w as usize * 4, rw as usize * 4);
            let src = self.rpix.data_as_u8_slice();
            let dst = self.frame.data_as_u8_slice_mut();
            for (i, row) in src.chunks_exact(rwb).enumerate() {
                let o = (y0 as usize + i) * fw + x0 as usize * 4;
                dst[o..o + rwb].copy_from_slice(row);
            }
        }
        // Recycle offscreen bitmaps.
        for a in st.in_flight.drain(..) {
            if let Ok(p) = Arc::try_unwrap(a) {
                st.pix_pool.push(p);
            }
        }
        // Evict caches: on full frames by recency; on partial frames only when large
        // (entries outside the region weren't touched but are still valid).
        let now = st.frame_no;
        if full || st.cache.len() > 512 {
            st.cache.retain(|_, e| e.last_used + 1 >= now);
        }
        if full || st.text.len() > 2048 {
            st.text.retain(|_, e| e.last_used + 2 >= now);
        }
        if full || st.scaled.len() > 64 {
            st.scaled.retain(|_, e| e.last_used + 2 >= now);
        }
        st.seen.retain(|_, &mut f| f + 2 >= now);
        if st.sprites.len() > 256 {
            st.sprites.retain(|_, e| e.last_used + 30 >= now);
        }
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

fn draw_ops(st: &mut State, assets: &View, ctx: &mut RenderContext, ops: &[Op], base: Affine) {
    draw_ops_culled(st, assets, ctx, ops, base, None)
}

/// Culling for partial renders: the region (in the context's coordinates)
/// and each op's frame-space bounds (`None` = draws nothing), offset by `org`.
#[derive(Clone, Copy)]
pub(crate) struct Cull<'a> {
    pub region: Rect,
    pub rects: &'a [Option<Rect>],
    pub org: (f64, f64),
}

/// As [`draw_ops`], skipping top-level ops and layers whose bounds miss the cull region.
fn draw_ops_culled(st: &mut State, assets: &View, ctx: &mut RenderContext, ops: &[Op], base: Affine, cull: Option<Cull>) {
    let misses = |a: usize, b: usize| -> bool {
        let Some(c) = cull else { return false };
        let reg = c.region + kurbo::Vec2::new(c.org.0, c.org.1);
        !c.rects[a..b.min(c.rects.len())]
            .iter()
            .flatten()
            .any(|r| r.x1 > reg.x0 && r.y1 > reg.y0 && r.x0 < reg.x1 && r.y0 < reg.y1)
    };
    let mut i = 0;
    while i < ops.len() {
        match &ops[i] {
            Op::PushLayer { opacity, blend, filters, clip, cache_key } => {
                let end = i + matching_pop(&ops[i..]);
                let inner = &ops[i + 1..end.min(ops.len())];
                let start = i;
                i = end + 1;
                let opacity = if op_ok(&ops[start], base) { opacity.clamp(0.0, 1.0) } else { 0.0 };
                if opacity <= 0.0 || inner.is_empty() {
                    continue;
                }
                if misses(start + 1, end) {
                    continue;
                }
                if let [dl::Filter::Shadow { dx, dy, blur, color: sc }] = filters.as_slice() {
                    if let Some(card) = card_shadow(inner, base) {
                        let trivial = opacity >= 1.0 && *blend == Blend::Normal && clip.is_none();
                        if !trivial {
                            push_layer(ctx, base, clip.as_ref(), *blend, opacity);
                        }
                        let mut c = *sc;
                        c.0[3] *= card.alpha;
                        if c.0[3] > 0.0 {
                            draw_shadow_sprite(st, ctx, &card, *dx, *dy, blur.max(0.0), c);
                        }
                        draw_ops(st, assets, ctx, inner, base);
                        if !trivial {
                            ctx.pop_layer();
                        }
                        continue;
                    }
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
                if !misses(i, i + 1) {
                    draw_op(st, assets, ctx, op, base);
                }
                i += 1;
            }
        }
    }
}

/// Draw a card's drop shadow from a cached sprite (rendered and blurred once
/// per size/radius/blur/colour), at an integer device offset.
#[allow(clippy::too_many_arguments)]
fn draw_shadow_sprite(st: &mut State, ctx: &mut RenderContext, card: &Card, dx: f32, dy: f32, sigma: f32, c: Rgba) {
    let dev = card.transform.transform_rect_bbox(card.rect);
    let (w, h) = (dev.width(), dev.height());
    let r = card.radius as f64 * card.scale as f64;
    let m = (sigma * 3.0).ceil() as f64 + 1.0;
    let q = |v: f64| (v * 16.0).round() as u64;
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (q(w), q(h), q(r), q(sigma as f64), c.0.map(|v| (v * 1023.0) as u32)).hash(&mut hs);
        hs.finish()
    };
    let now = st.frame_no;
    let pix = match st.sprites.get_mut(&key) {
        Some(e) => {
            e.last_used = now;
            e.pix.clone()
        }
        None => {
            let (sw, sh) = ((w + 2.0 * m).ceil(), (h + 2.0 * m).ceil());
            if sw > 8192.0 || sh > 8192.0 {
                return;
            }
            let (sw, sh) = (sw as u16, sh as u16);
            let mut sub = st.pool.pop().unwrap_or_else(|| RenderContext::new(sw, sh));
            sub.reset_and_resize(sw, sh);
            sub.set_paint(color(c));
            sub.set_fill_rule(Fill::NonZero);
            st.path.truncate(0);
            st.path.extend(RoundedRect::new(m, m, m + w, m + h, r).path_elements(0.1));
            sub.fill_path(&st.path);
            sub.flush();
            let mut pm = Pixmap::new(sw, sh);
            sub.render_with(&mut pm, &mut st.res, RasterizerSettings::default());
            sub.reset();
            st.pool.push(sub);
            filter::apply(&dl::Filter::Blur(sigma), pm.data_as_u8_slice_mut(), sw as usize, sh as usize, &mut st.filters);
            pm.set_may_have_transparency(true);
            let pix = Arc::new(pm);
            st.sprites.insert(key, CacheEntry { pix: pix.clone(), x: 0, y: 0, last_used: now });
            pix
        }
    };
    let (x, y) = ((dev.x0 - m + dx as f64).round(), (dev.y0 - m + dy as f64).round());
    blit(ctx, pix, x, y, 1.0);
}

/// Composite a premultiplied bitmap at an integer device position.
fn blit(ctx: &mut RenderContext, pix: Arc<Pixmap>, x: f64, y: f64, alpha: f32) {
    let (w, h) = (pix.width() as f64, pix.height() as f64);
    ctx.set_transform(Affine::translate((x, y)));
    ctx.reset_paint_transform();
    ctx.set_paint(Image {
        image: ImageSource::Pixmap(pix),
        sampler: ImageSampler { x_extend: Extend::Pad, y_extend: Extend::Pad, quality: ImageQuality::Low, alpha },
    });
    ctx.fill_rect(&Rect::new(0.0, 0.0, w, h));
}

/// Draw a solid-colour, translation-only glyph run from a cached bitmap
/// (rendered once per content and quarter-pixel phase). Returns false if the
/// run can't be cached (colour font) and must be drawn as outlines.
#[allow(clippy::too_many_arguments)]
fn draw_cached_run(
    st: &mut State,
    ctx: &mut RenderContext,
    font_id: dl::FontId,
    font: &Face,
    size: f32,
    glyphs: &[dl::Glyph],
    c: Rgba,
    tx: f64,
    ty: f64,
) -> bool {
    let (fx, fy) = (tx.floor(), ty.floor());
    let (qx, qy) = (((tx - fx) * 4.0).round() as u8, ((ty - fy) * 4.0).round() as u8);
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (font_id.0, size.to_bits(), qx, qy, c.0.map(f32::to_bits)).hash(&mut hs);
        for g in glyphs {
            (g.id, g.x.to_bits(), g.y.to_bits()).hash(&mut hs);
        }
        hs.finish()
    };
    let now = st.frame_no;
    let (pix, x0, y0) = match st.text.get_mut(&key) {
        Some(e) => {
            e.last_used = now;
            (e.pix.clone(), e.x, e.y)
        }
        None => {
            let mut run = std::mem::take(&mut st.run);
            if !st.glyphs.run_path(font_id, font, size, glyphs, &mut run) {
                st.run = run;
                return false;
            }
            let (ox, oy) = (qx as f64 / 4.0, qy as f64 / 4.0);
            if run.is_empty() {
                st.run = run;
                return true;
            }
            let bb = run.bounding_box();
            let (x0, y0) = ((bb.x0 + ox).floor() - 1.0, (bb.y0 + oy).floor() - 1.0);
            let (w, h) = ((bb.x1 + ox).ceil() + 1.0 - x0, (bb.y1 + oy).ceil() + 1.0 - y0);
            if w > 4096.0 || h > 4096.0 {
                st.run = run;
                return false;
            }
            let (w, h) = (w as u16, h as u16);
            let mut sub = st.pool.pop().unwrap_or_else(|| RenderContext::new(w, h));
            sub.reset_and_resize(w, h);
            sub.set_paint(color(c));
            sub.set_fill_rule(Fill::NonZero);
            sub.set_transform(Affine::translate((ox - x0, oy - y0)));
            sub.fill_path(&run);
            sub.flush();
            let mut pm = st.pix_pool.pop().unwrap_or_else(|| Pixmap::new(w, h));
            pm.resize(w, h);
            sub.render_with(&mut pm, &mut st.res, RasterizerSettings::default());
            sub.reset();
            st.pool.push(sub);
            st.run = run;
            pm.set_may_have_transparency(true);
            let pix = Arc::new(pm);
            let (x0, y0) = (x0 as i32, y0 as i32);
            st.text.insert(key, CacheEntry { pix: pix.clone(), x: x0, y: y0, last_used: now });
            (pix, x0, y0)
        }
    };
    blit(ctx, pix, fx + x0 as f64, fy + y0 as f64, 1.0);
    true
}

/// Axis-aligned images: 1:1 at integer positions blit directly; scaled ones
/// that repeat across frames are resampled once (tent filter) and blitted.
/// Returns false to fall back to vello's bilinear sampling.
#[allow(clippy::too_many_arguments)]
fn draw_image_fast(st: &mut State, ctx: &mut RenderContext, id: dl::ImageId, pix: &Arc<Pixmap>, dst: &[f32; 4], opacity: f32, t: Affine) -> bool {
    let k = t.as_coeffs();
    if k[1] != 0.0 || k[2] != 0.0 || k[0] <= 0.0 || k[3] <= 0.0 {
        return false;
    }
    let (x0, y0) = (k[0] * dst[0] as f64 + k[4], k[3] * dst[1] as f64 + k[5]);
    let (ew, eh) = (k[0] * dst[2] as f64, k[3] * dst[3] as f64);
    let (iw, ih) = (pix.width() as f64, pix.height() as f64);
    let alpha = opacity.min(1.0);
    if ew == iw && eh == ih && x0.fract() == 0.0 && y0.fract() == 0.0 {
        blit(ctx, pix.clone(), x0, y0, alpha);
        return true;
    }
    let q = |v: f64| (v * 4.0).round() / 4.0;
    let (fx, fy) = (x0.floor(), y0.floor());
    let (px, py, qw, qh) = (q(x0 - fx), q(y0 - fy), q(ew), q(eh));
    if qw < 1.0 || qh < 1.0 || qw > 8192.0 || qh > 8192.0 {
        return false;
    }
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (id.0, px.to_bits(), py.to_bits(), qw.to_bits(), qh.to_bits(), Arc::as_ptr(pix) as usize).hash(&mut hs);
        hs.finish()
    };
    let now = st.frame_no;
    if let Some(e) = st.scaled.get_mut(&key) {
        e.last_used = now;
        let p = e.pix.clone();
        blit(ctx, p, fx, fy, alpha);
        return true;
    }
    // Only pay for resampling once the same placement shows up in consecutive frames.
    let seen_last = st.seen.insert(key, now).is_some_and(|f| f + 1 >= now);
    if !seen_last {
        return false;
    }
    let (ow, oh) = ((px + qw).ceil() as usize, (py + qh).ceil() as usize);
    resample::resample(pix.data_as_u8_slice(), iw as usize, ih as usize, px, py, qw, qh, ow, oh, &mut st.resample_buf);
    let mut pm = Pixmap::new(ow as u16, oh as u16);
    pm.data_as_u8_slice_mut().copy_from_slice(&st.resample_buf);
    pm.recompute_may_have_transparency();
    let p = Arc::new(pm);
    st.scaled.insert(key, CacheEntry { pix: p.clone(), x: 0, y: 0, last_used: now });
    blit(ctx, p, fx, fy, alpha);
    true
}

struct Card {
    rect: Rect,
    radius: f32,
    transform: Affine,
    scale: f32,
    alpha: f32,
}

/// If the layer's alpha is exactly one axis-aligned rounded rect (a card,
/// optionally with content drawn inside an opaque card), return it: its drop
/// shadow is then an analytic blurred rounded rect, with no offscreen pass.
fn card_shadow(inner: &[Op], base: Affine) -> Option<Card> {
    let Op::RoundRect { x, y, w, h, radius, paint, transform } = inner.first()? else { return None };
    let t = base * affine(transform);
    let [a, b, c, d, _, _] = t.as_coeffs();
    if b != 0.0 || c != 0.0 || a <= 0.0 || (a - d).abs() > 1e-6 * a.abs() {
        return None;
    }
    let alpha = match paint {
        Paint::Solid(c) => c.0[3].clamp(0.0, 1.0),
        Paint::Linear { stops, .. } | Paint::Radial { stops, .. } if stops.iter().all(|s| s.1 .0[3] >= 1.0) => 1.0,
        _ => return None,
    };
    let rect = Rect::new(*x as f64, *y as f64, (*x + *w) as f64, (*y + *h) as f64);
    if inner.len() > 1 {
        // Everything else must sit inside an opaque card, so the alpha is the card's.
        if alpha < 1.0 {
            return None;
        }
        let dev = t.transform_rect_bbox(rect).inflate(1.5, 1.5);
        let rest = ops_bounds(&inner[1..], base)?;
        if rest.x0 < dev.x0 || rest.y0 < dev.y0 || rest.x1 > dev.x1 || rest.y1 > dev.y1 {
            return None;
        }
    }
    Some(Card { rect, radius: radius.clamp(0.0, w.min(*h) * 0.5), transform: t, scale: a as f32, alpha })
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
/// Largest device coordinate we draw (anything beyond is degenerate input).
const MAX_COORD: f64 = 1e6;

fn rect_ok(r: Rect) -> bool {
    [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD)
}

/// Whether an op's numbers are sane enough to hand to the rasteriser:
/// finite transform, finite geometry within ±1e6 px. Ops failing this are skipped.
pub(crate) fn op_ok(op: &Op, base: Affine) -> bool {
    let fin = |v: &[f32]| v.iter().all(|x| x.is_finite());
    let dev = |t: &dl::Affine, r: Rect| fin(t) && rect_ok((base * affine(t)).transform_rect_bbox(r));
    match op {
        Op::Fill { path, transform, .. } => dev(transform, path.bounding_box()),
        Op::Stroke { path, transform, stroke, .. } => {
            let w = stroke.width as f64;
            stroke.width.is_finite() && stroke.width >= 0.0 && dev(transform, path.bounding_box().inflate(w, w))
        }
        Op::RoundRect { x, y, w, h, radius, transform, .. } => {
            fin(&[*x, *y, *w, *h, *radius]) && dev(transform, Rect::new(*x as f64, *y as f64, (*x + *w) as f64, (*y + *h) as f64))
        }
        Op::Image { dst, opacity, transform, .. } => {
            fin(dst) && opacity.is_finite() && dev(transform, Rect::new(dst[0] as f64, dst[1] as f64, (dst[0] + dst[2]) as f64, (dst[1] + dst[3]) as f64))
        }
        Op::Glyphs { size, glyphs, transform, .. } => {
            size.is_finite()
                && glyphs.iter().all(|g| g.x.is_finite() && g.y.is_finite())
                && glyphs.first().is_none_or(|g| {
                    let s = *size as f64 * 2.0;
                    dev(transform, Rect::new(g.x as f64 - s, g.y as f64 - s, g.x as f64 + s, g.y as f64 + s))
                })
        }
        Op::PushLayer { opacity, clip, .. } => {
            opacity.is_finite() && clip.as_ref().is_none_or(|(p, t)| fin(t) && { let b = p.bounding_box(); [b.x0, b.y0, b.x1, b.y1].iter().all(|v| v.is_finite()) })
        }
        Op::PopLayer => true,
    }
}

pub(crate) fn ops_bounds(ops: &[Op], base: Affine) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    let mut extra = [0f32; 4];
    for op in ops {
        if !matches!(op, Op::PushLayer { .. } | Op::PopLayer) && !op_ok(op, base) {
            continue;
        }
        let r = match op {
            Op::Fill { path, transform, .. } => Some((base * affine(transform)).transform_rect_bbox(path.bounding_box())),
            Op::Stroke { path, transform, stroke, .. } => {
                let t = base * affine(transform);
                let trimmed;
                let path = if stroke.trim[0] > 0.0 || stroke.trim[1] < 1.0 {
                    let mut p = BezPath::new();
                    trim::trim_path(path, stroke.trim[0], stroke.trim[1], &mut p, &mut Vec::new());
                    if p.is_empty() {
                        continue;
                    }
                    trimmed = p;
                    &trimmed
                } else {
                    path
                };
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
    assets: &View,
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
    let org = st.org;
    let cached = cache_key.and_then(|k| st.cache.get_mut(&k)).map(|e| {
        e.last_used = now;
        (e.pix.clone(), e.x - org.0, e.y - org.1)
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
            // Cached bitmaps must hold everything visible in the *frame* (they
            // outlive this render's region); others only what this context shows.
            let view = if cache_key.is_some() {
                let (fw, fh) = st.frame_size;
                Rect::new(-org.0 as f64, -org.1 as f64, fw as f64 - org.0 as f64, fh as f64 - org.1 as f64)
            } else {
                Rect::new(0.0, 0.0, ctx.width() as f64, ctx.height() as f64)
            };
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
            st.org = (org.0 + r.x0 as i32, org.1 + r.y0 as i32);
            draw_ops(st, assets, &mut sub, inner, Affine::translate((-r.x0, -r.y0)) * base);
            st.org = org;
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
                    st.cache.insert(k, CacheEntry { pix: pix.clone(), x: x + org.0, y: y + org.1, last_used: now });
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
    blit(ctx, pix, x as f64, y as f64, if layered { 1.0 } else { opacity });
    if layered {
        ctx.pop_layer();
    }
}

fn stops(stops: &[(f32, Rgba)]) -> ColorStops {
    ColorStops::from(stops.iter().map(|(o, c)| ColorStop::from((*o, color(*c)))).collect::<Vec<_>>().as_slice())
}

/// Set `paint` on `ctx`. Returns false when there is nothing to draw.
fn set_paint(ctx: &mut RenderContext, assets: &View, paint: &Paint) -> bool {
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
            let Some(pix) = assets.image(*image) else { return false };
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

fn el_points(el: &kurbo::PathEl) -> impl Iterator<Item = kurbo::Point> {
    use kurbo::PathEl::*;
    let v: [Option<kurbo::Point>; 3] = match *el {
        MoveTo(a) | LineTo(a) => [Some(a), None, None],
        QuadTo(a, b) => [Some(a), Some(b), None],
        CurveTo(a, b, c) => [Some(a), Some(b), Some(c)],
        ClosePath => [None, None, None],
    };
    v.into_iter().flatten()
}

fn invert(a: Affine) -> Option<Affine> {
    if a.determinant().abs() < 1e-12 {
        None
    } else {
        Some(a.inverse())
    }
}

fn draw_op(st: &mut State, assets: &View, ctx: &mut RenderContext, op: &Op, base: Affine) {
    if !op_ok(op, base) {
        return;
    }
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
            // Guard against dash patterns that would produce millions of dashes.
            let t = base * affine(transform);
            let c = t.as_coeffs();
            let scale = (c[0].abs() + c[1].abs()).max(c[2].abs() + c[3].abs()).max(1e-9);
            let mut len = 0.0;
            let mut last = kurbo::Point::ZERO;
            for el in path.elements() {
                for p in el_points(el) {
                    len += p.distance(last);
                    last = p;
                }
            }
            let dashes = len * scale / (dash_sum as f64 * scale).max(1e-9);
            if !stroke.dash.is_empty() && dash_sum > 1e-3 && stroke.dash.iter().all(|d| d.is_finite()) && dashes < 20_000.0 {
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
            let Some(pix) = assets.image(*image) else { return };
            if *opacity <= 0.0 || dst[2] <= 0.0 || dst[3] <= 0.0 {
                return;
            }
            let t = base * affine(transform);
            if draw_image_fast(st, ctx, *image, pix, dst, *opacity, t) {
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
        Op::Glyphs { font: font_id, size, glyphs, paint, transform } => {
            let Some(font) = assets.face(*font_id) else { return };
            if glyphs.is_empty() || *size <= 0.0 || !set_paint(ctx, assets, paint) {
                return;
            }
            ctx.set_transform(base * affine(transform));
            ctx.set_fill_rule(Fill::NonZero);
            if let Paint::Solid(c) = paint {
                let t = base * affine(transform);
                let k = t.as_coeffs();
                if k[0] == 1.0 && k[1] == 0.0 && k[2] == 0.0 && k[3] == 1.0 {
                    if draw_cached_run(st, ctx, *font_id, font, *size, glyphs, *c, k[4], k[5]) {
                        return;
                    }
                    ctx.set_transform(t);
                }
            }
            if st.glyphs.run_path(*font_id, font, *size, glyphs, &mut st.run) {
                if !st.run.is_empty() {
                    ctx.fill_path(&st.run);
                }
            } else {
                let mut b = ctx.glyph_run(&mut st.res, &font.data).font_size(*size).hint(false).atlas_cache(true);
                b = b.normalized_coords(&font.coords);
                if let Some(a) = font.skew {
                    b = b.glyph_transform(Affine::skew(-(a as f64).to_radians().tan(), 0.0));
                }
                if font.embolden {
                    let e = 0.02 * *size as f64;
                    b = b.font_embolden(glifo::FontEmbolden::new(kurbo::Diagonal2::new(e, e)));
                }
                let _ = b.fill_glyphs(glyphs.iter().map(|g| vello_cpu::Glyph { id: g.id as u32, x: g.x, y: g.y }));
            }
        }
        Op::PushLayer { .. } | Op::PopLayer => {}
    }
}
