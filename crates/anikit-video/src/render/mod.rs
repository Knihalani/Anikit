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
pub mod post;
pub mod incremental;
pub mod motion;
pub mod resample;
pub mod smooth;
pub mod trim;
pub mod upscale;
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
    dynamic: Option<&'a dyn DynamicImages>,
}

/// A source of images made while rendering (component fields, 3D renders,
/// video frames), consulted for ids not in [`Assets::images`] or the
/// per-frame overrides. Must be thread-safe: every worker calls it.
pub trait DynamicImages: Send + Sync {
    /// The premultiplied RGBA8 image for `id`, if this source has it. Return
    /// the same `Arc` while the pixels are unchanged (incremental renders key
    /// on its identity) and a new one when they change.
    fn get(&self, id: dl::ImageId) -> Option<Arc<Pixmap>>;
}

impl<F: Fn(dl::ImageId) -> Option<Arc<Pixmap>> + Send + Sync> DynamicImages for F {
    fn get(&self, id: dl::ImageId) -> Option<Arc<Pixmap>> {
        self(id)
    }
}

impl View<'_> {
    fn image(&self, id: dl::ImageId) -> Option<Arc<Pixmap>> {
        self.frame
            .get(&id)
            .or_else(|| self.assets.images.get(id))
            .cloned()
            .or_else(|| self.dynamic.and_then(|d| d.get(id)))
    }
    fn face(&self, id: dl::FontId) -> Option<&Face> {
        self.assets.fonts.face(id)
    }
}

/// Speed/quality trade-off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Quality {
    /// Fastest: smooth backgrounds render at ¼ resolution with bilinear upscaling.
    Draft,
    /// Default: gradients at full resolution; low-res fields upscale with a
    /// smooth cubic B-spline and dither.
    #[default]
    Standard,
    /// As standard (reserved for heavier AA and filtering).
    High,
    /// Ground truth for the quality harness (`qbench`, `--quality reference`):
    /// every speed-only approximation off. Glyph runs and scaled images at their
    /// exact sub-pixel positions (no quarter-pixel snapping, no caches), card
    /// shadows through the general blur (no sprites), blurs at full resolution
    /// (no decimation), no layer cache, no fused YUV or incremental regions, and
    /// the most motion-blur samples. Slow; never the default.
    Reference,
}

/// A rendered frame: premultiplied RGBA8, `width × height`, tightly packed.
pub type FrameRgba = Pixmap;

pub(crate) fn affine(a: &dl::Affine) -> Affine {
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
    /// Smooth cubic upscaling for enlarged images (standard quality and up).
    cubic: bool,
    /// [`Quality::Reference`]: no speed-only approximations.
    reference: bool,
    up: upscale::Upscaler,
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
    dynamic: Option<Arc<dyn DynamicImages>>,
    upscaler: upscale::Upscaler,
    /// Post / film look applied to every frame (see [`post`]), and the frame index seeding its grain.
    post: Option<post::PostSettings>,
    post_frame: u64,
    pp: post::PostProcessor,
    /// Motion-blur accumulator (linear RGB).
    motion_acc: Vec<f32>,
    motion_tmp: Vec<f32>,
    motion_brute: bool,
    quality: Quality,
    /// Low-res backdrop as Y, Cb, Cr planes (fused backdrop → YUV path).
    lo: [Vec<u8>; 3],
    /// Overlay op bounds and macroblock map (multi-box fused path).
    box_rects: Vec<Option<Rect>>,
    box_map: MbMap,
    /// Low-resolution smooth backdrop, its context and content key.
    lctx: Option<RenderContext>,
    smooth_lo: Pixmap,
    smooth_key: Option<u64>,
    inc: incremental::Tracker,
    st: State,
}

/// Device-space rect of the pixels `r` touches, rounded out.
fn round_out(r: Rect) -> Rect {
    Rect::new(r.x0.floor(), r.y0.floor(), r.x1.ceil(), r.y1.ceil())
}

impl Renderer {
    pub fn new(assets: Arc<Assets>) -> Self {
        let mut r = Self {
            assets,
            ctx: RenderContext::new(1, 1),
            frame: Pixmap::new(1, 1),
            rctx: None,
            rpix: Pixmap::new(1, 1),
            frame_images: Default::default(),
            dynamic: None,
            upscaler: Default::default(),
            lo: Default::default(),
            post: None,
            post_frame: 0,
            pp: Default::default(),
            motion_acc: Vec::new(),
            motion_tmp: Vec::new(),
            motion_brute: false,
            quality: Quality::Standard,
            box_rects: Vec::new(),
            box_map: MbMap::default(),
            lctx: None,
            smooth_lo: Pixmap::new(1, 1),
            smooth_key: None,
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
                cubic: true,
                reference: false,
                up: upscale::Upscaler::cubic(),
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
        };
        r.upscaler.cubic = true;
        r
    }

    pub fn assets(&self) -> &Arc<Assets> {
        &self.assets
    }

    /// Swap in a newer snapshot of the same asset store (e.g. after fonts were
    /// added; ids are stable, so caches and incremental state stay valid). The
    /// pipeline uses this instead of a new renderer, keeping output independent
    /// of when other threads intern fonts.
    pub fn update_assets(&mut self, assets: Arc<Assets>) {
        self.assets = assets;
    }

    /// Dynamic-image hook: draw `pixmap` (premultiplied RGBA8) for `id` in the
    /// following renders, overriding [`Assets::images`] — for decoded video
    /// frames or other per-frame textures. Set it before each frame whose
    /// image changes; incremental renders notice the new pixels (identity of
    /// the `Arc`), so give each new frame its own `Arc`.
    pub fn set_frame_image(&mut self, id: dl::ImageId, pixmap: Arc<Pixmap>) {
        Arc::make_mut(&mut self.frame_images).insert(id, pixmap);
    }

    /// Dynamic-image hook: images not in the shared store (e.g. component
    /// images at ids ≥ `0x4000_0000`) are fetched from `source` while rendering.
    pub fn set_image_source(&mut self, source: Arc<dyn DynamicImages>) {
        self.dynamic = Some(source);
        self.invalidate();
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
        self.render_region(list, None, true, None, None);
        if let Some(p) = &self.post {
            let (w, h) = (self.frame.width() as usize, self.frame.height() as usize);
            self.pp.apply(self.frame.data_as_u8_slice_mut(), w, h, p, self.post_frame);
        }
        &self.frame
    }

    /// The current speed/quality trade-off.
    pub fn quality(&self) -> Quality {
        self.quality
    }

    /// Speed/quality trade-off for the following frames (default [`Quality::Standard`]).
    pub fn set_quality(&mut self, q: Quality) {
        if q != self.quality {
            self.invalidate();
            self.smooth_key = None;
        }
        self.quality = q;
        let cubic = q != Quality::Draft;
        self.upscaler.cubic = cubic;
        self.st.cubic = cubic;
        self.st.up.cubic = cubic;
        self.st.reference = q == Quality::Reference;
        self.st.filters.set_exact(q == Quality::Reference);
    }

    /// Motion blur by brute force (64 full frames per blurred frame): the
    /// ground truth the fast path is tested against. Also on at [`Quality::Reference`].
    #[doc(hidden)]
    pub fn set_brute_force_motion(&mut self, on: bool) {
        self.motion_brute = on;
    }

    /// Speed-only backdrop shortcuts (¼-res smooth backdrops, bilinear
    /// upscaling) on or off; off by default. `true` selects [`Quality::Draft`];
    /// `false` returns a draft renderer to [`Quality::Standard`] and leaves
    /// High/Reference alone.
    pub fn set_fast_backdrops(&mut self, on: bool) {
        if on {
            self.set_quality(Quality::Draft);
        } else if self.quality == Quality::Draft {
            self.set_quality(Quality::Standard);
        }
    }

    /// Film look for the following frames (`None` = off). `frame_index` seeds
    /// the animated grain: pass the video frame number so output is
    /// deterministic whichever worker renders it.
    pub fn set_post(&mut self, post: Option<post::PostSettings>, frame_index: u64) {
        let post = post.filter(|p| !p.is_identity());
        if post != self.post {
            self.invalidate();
        }
        self.post = post;
        self.post_frame = frame_index;
    }

    /// Render all of `list`, or only the pixels of `region` (`[x0, y0, x1, y1)`,
    /// clipped to the frame) — the rest of the frame keeps its previous pixels.
    /// Returns true when it wrote `yuv` directly (fused backdrop path; the RGBA
    /// frame is then only partly updated).
    pub(crate) fn render_region(
        &mut self,
        list: &DisplayList,
        region: Option<[u32; 4]>,
        new_frame: bool,
        rects: Option<&[Option<Rect>]>,
        yuv: Option<&mut YuvFrame>,
    ) -> bool {
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
        let dynamic = self.dynamic.clone();
        let assets = View { assets: &shared, frame: &frame_images, dynamic: dynamic.as_deref() };
        let settings = RasterizerSettings { target_init: TargetInit::Clear(color(list.background)), ..Default::default() };
        // A full-frame opaque image hides everything before it: upscale it
        // straight into the target and draw only the ops above it.
        let mut backdrop = backdrop(&list.ops, &assets, w as f64, h as f64);
        let mut from_smooth = false;
        // Or: a smooth leading run (gradients, glows, fields) rendered at ¼ resolution.
        let mut n_smooth = smooth::prefix(&list.ops, w as f64, h as f64, &|id| assets.image(id).map(|p| (p.width() as u32, p.height() as u32)));
        // Standard quality: evaluate gradient backdrops exactly, in float, at full
        // resolution (dithered: no banding); Draft: ¼ resolution through vello.
        let field = self.quality != Quality::Draft;
        if field && !smooth::field_ok(&list.ops[..n_smooth]) {
            n_smooth = 0;
        }
        // vello treats the clear colour and pixels already in the target as an
        // isolated backdrop, so a non-Normal blend (Overlay grain, Screen glow…)
        // would blend against transparency. With such a layer, draw the background
        // inside the scene and skip the pre-filled backdrop paths.
        let blends = list.ops.iter().any(|o| matches!(o, Op::PushLayer { blend, .. } if *blend != Blend::Normal));
        if blends {
            backdrop = None;
            n_smooth = 0;
        }
        let with_bg: Vec<Op>;
        let list_ops: &[Op] = if blends {
            with_bg = std::iter::once(Op::RoundRect { x: 0.0, y: 0.0, w: w as f32, h: h as f32, radius: 0.0, paint: Paint::Solid(list.background), transform: dl::IDENTITY })
                .chain(list.ops.iter().cloned())
                .collect();
            &with_bg
        } else {
            &list.ops
        };
        let rects = if blends { None } else { rects };
        if n_smooth > 0 && backdrop.as_ref().is_none_or(|(k, _, _)| *k < n_smooth) {
            let f = smooth::FACTOR;
            let (lw, lh) = ((w as u32).div_ceil(f).max(2), (h as u32).div_ceil(f).max(2));
            let key = {
                let mut hs = incremental::Fx::default();
                use std::hash::Hasher;
                hs.write_u64(((w as u64) << 32) | h as u64 | (field as u64) << 63);
                hs.write_u64(list.background.0.iter().fold(0u64, |a, v| a.rotate_left(16) ^ v.to_bits() as u64));
                for op in &list.ops[..n_smooth] {
                    hs.write_u64(incremental::hash_op(op));
                }
                // Images can change pixels under the same id.
                for op in &list.ops[..n_smooth] {
                    if let Op::Image { image, .. } = op {
                        hs.write_u64(assets.image(*image).map_or(0, |p| Arc::as_ptr(&p) as u64));
                    }
                }
                hs.finish()
            };
            if field && self.smooth_key != Some(key) {
                self.smooth_lo.resize(w, h);
                smooth::render_field(&list.ops[..n_smooth], w as usize, h as usize, list.background, self.smooth_lo.data_as_u8_slice_mut());
                self.smooth_lo.recompute_may_have_transparency();
                self.smooth_key = Some(key);
            } else if self.smooth_key != Some(key) {
                let lctx = self.lctx.get_or_insert_with(|| RenderContext::new(lw as u16, lh as u16));
                lctx.reset_and_resize(lw as u16, lh as u16);
                st.org = (0, 0);
                draw_ops(st, &assets, lctx, &list.ops[..n_smooth], Affine::scale_non_uniform(lw as f64 / w as f64, lh as f64 / h as f64));
                lctx.flush();
                self.smooth_lo.resize(lw as u16, lh as u16);
                lctx.render_with(&mut self.smooth_lo, &mut st.res, settings);
                lctx.reset();
                self.smooth_lo.recompute_may_have_transparency();
                self.smooth_key = Some(key);
            }
            let lo = Arc::new(std::mem::replace(&mut self.smooth_lo, Pixmap::new(1, 1)));
            backdrop = Some((n_smooth - 1, lo, Rect::new(0.0, 0.0, w as f64, h as f64)));
            from_smooth = true;
        }
        let (ops, skip) = match &backdrop {
            Some((k, _, _)) => (&list.ops[k + 1..], k + 1),
            None => (list_ops, 0),
        };
        let settings = if backdrop.is_some() { RasterizerSettings { target_init: TargetInit::SrcOver, ..Default::default() } } else { settings };
        if backdrop.is_some() {
            // The backdrop's pixels may have changed in place (cached low-res buffer).
            self.upscaler.invalidate();
        }
        let mut fused = false;
        let (wu, hu) = (w as usize, h as usize);
        // Macroblock-aligned box of the ops drawn over a backdrop.
        let rgn = backdrop.as_ref().and_then(|_| ops_bounds(ops, Affine::IDENTITY)).and_then(|r| {
            let a = |v: f64, hi: usize| ((v.max(0.0) / 16.0).floor() as usize * 16).min(hi);
            let b = |v: f64, hi: usize| ((v.max(0.0) / 16.0).ceil() as usize * 16).min(hi);
            let r = [a(r.x0, wu), a(r.y0, hu), b(r.x1, wu), b(r.y1, hu)];
            (r[2] > r[0] && r[3] > r[1]).then_some(r)
        });
        let small = rgn.is_none_or(|r| ((r[2] - r[0]) * (r[3] - r[1])) as f64 <= 0.6 * (wu * hu) as f64);
        // Ops whose top-level layers all blend normally can be drawn on a
        // transparent box and composited over the backdrop in YUV space.
        let mut depth = 0usize;
        let normal_only = ops.iter().all(|o| match o {
            Op::PushLayer { blend, .. } => {
                depth += 1;
                depth > 1 || *blend == Blend::Normal
            }
            Op::PopLayer => {
                depth = depth.saturating_sub(1);
                true
            }
            _ => true,
        });
        if let (true, true, Some(yuv), Some((_, pix, dev))) = (full, small || normal_only, yuv, &backdrop) {
            // Fused: YUV straight from the upscaled backdrop planes, with RGBA +
            // vello only inside the macroblock-aligned box of the ops on top.
            fused = true;
            yuv.resize(w as u32, h as u32);
            // Convert the (small) backdrop to YUV 4:4:4 once, then upscale the
            // planes straight into the frame: 1.5 bytes per pixel instead of
            // upscaling 4 and converting. (Conversion is affine, so this matches
            // converting the upscaled RGBA to within rounding.)
            let (pw, ph) = (pix.width() as usize, pix.height() as usize);
            let [ly, lu, lv] = &mut self.lo;
            yuv::rgba_to_yuv444(pix.data_as_u8_slice(), ly, lu, lv);
            // Padding for the gather path's 4-byte reads.
            for p in [ly, lu, lv] {
                p.extend_from_slice(&[0; 4]);
            }
            let (cw, ch) = (wu.div_ceil(2), hu.div_ceil(2));
            let cdev = Rect::new(dev.x0 / 2.0, dev.y0 / 2.0, dev.x1 / 2.0, dev.y1 / 2.0);
            let (ys, cs) = (yuv.y_stride, yuv.c_stride);
            // One kernel for all three planes, chosen from the luma scale.
            self.upscaler.bspline = Some(dev.width() >= 12.0 * pw as f64 && dev.height() >= 12.0 * ph as f64);
            self.upscaler.run_plane(&self.lo[0], pw, ph, *dev, &mut yuv.y, ys, wu, hu, 0, 0);
            self.upscaler.run_plane(&self.lo[1], pw, ph, cdev, &mut yuv.u, cs, cw, ch, 0, 0);
            self.upscaler.run_plane(&self.lo[2], pw, ph, cdev, &mut yuv.v, cs, cw, ch, 0, 0);
            // Odd sizes: the last chroma column/row covers one pixel, centred half a
            // pixel earlier than a full 2×2 block.
            let edges = [(wu % 2 == 1, false), (false, hu % 2 == 1), (wu % 2 == 1, hu % 2 == 1)];
            for (ex, ey) in edges {
                if !(ex || ey) || (ex && ey && !(wu % 2 == 1 && hu % 2 == 1)) {
                    continue;
                }
                let (sx, sy) = (if ex { 0.25 } else { 0.0 }, if ey { 0.25 } else { 0.0 });
                let d = cdev + kurbo::Vec2::new(sx, sy);
                let (x0, y0) = (if ex { cw - 1 } else { 0 }, if ey { ch - 1 } else { 0 });
                let (ow, oh) = (if ex { 1 } else { cw }, if ey { 1 } else { ch });
                let o = y0 * cs + x0;
                self.upscaler.run_plane(&self.lo[1], pw, ph, d, &mut yuv.u[o..], cs, ow, oh, x0 as i64, y0 as i64);
                self.upscaler.run_plane(&self.lo[2], pw, ph, d, &mut yuv.v[o..], cs, ow, oh, x0 as i64, y0 as i64);
            }
            // Normal blending: draw the ops in a few tight boxes (clustered
            // macroblocks), each on a transparent canvas composited in YUV.
            let boxes: Vec<[usize; 4]> = match rgn {
                Some(_) if normal_only => {
                    incremental::op_rects(ops, &mut self.box_rects);
                    let (mw, mh) = (wu.div_ceil(16), hu.div_ceil(16));
                    self.box_map.reset(mw, mh, false);
                    for r in self.box_rects.iter().flatten() {
                        self.box_map.mark_rect(r.inflate(-1.0, -1.0));
                    }
                    incremental::cluster(&self.box_map, 6)
                        .into_iter()
                        .map(|b| [b[0] as usize * 16, b[1] as usize * 16, (b[2] as usize * 16).min(wu), (b[3] as usize * 16).min(hu)])
                        .filter(|b| b[2] > b[0] && b[3] > b[1])
                        .collect()
                }
                Some(r) => vec![r],
                None => Vec::new(),
            };
            for [x0, y0, x1, y1] in boxes {
                let (rw, rh) = ((x1 - x0) as u16, (y1 - y0) as u16);
                self.rpix.resize(rw, rh);
                let box_settings = if normal_only {
                    RasterizerSettings { target_init: TargetInit::Clear(AlphaColor::TRANSPARENT), ..Default::default() }
                } else {
                    self.upscaler.run(pix.data_as_u8_slice(), pw, ph, *dev, self.rpix.data_as_u8_slice_mut(), rw as usize, rh as usize, x0 as i64, y0 as i64);
                    self.rpix.set_may_have_transparency(false);
                    settings
                };
                let rctx = self.rctx.get_or_insert_with(|| RenderContext::new(rw, rh));
                rctx.reset_and_resize(rw, rh);
                st.org = (x0 as i32, y0 as i32);
                let cull = normal_only.then(|| Cull { region: Rect::new(0.0, 0.0, rw as f64, rh as f64), rects: &self.box_rects, org: (x0 as f64, y0 as f64) });
                draw_ops_culled(st, &assets, rctx, ops, Affine::translate((-(x0 as f64), -(y0 as f64))), cull);
                rctx.flush();
                rctx.render_with(&mut self.rpix, &mut st.res, box_settings);
                rctx.reset();
                st.org = (0, 0);
                if normal_only {
                    yuv::composite_over(self.rpix.data_as_u8_slice(), rw as usize, rh as usize, (x0, y0), yuv);
                } else {
                    yuv::convert_into(self.rpix.data_as_u8_slice(), rw as usize, rh as usize, [0, 0, rw as usize, rh as usize], yuv, (x0, y0));
                }
            }
            self.upscaler.bspline = None;
            yuv::pad(yuv);
        } else if full {
            st.org = (0, 0);
            if let Some((_, pix, dev)) = &backdrop {
                self.upscaler.run(pix.data_as_u8_slice(), pix.width() as usize, pix.height() as usize, *dev, self.frame.data_as_u8_slice_mut(), w as usize, h as usize, 0, 0);
                self.frame.set_may_have_transparency(false);
            }
            // Over a backdrop, only the ops' bounding box needs vello.
            let sub = backdrop.as_ref().map(|_| {
                ops_bounds(ops, Affine::IDENTITY).map(|r| round_out(r.intersect(Rect::new(0.0, 0.0, w as f64, h as f64))))
            });
            match sub {
                // Nothing on top of the backdrop.
                Some(None) => {}
                Some(Some(r)) if r.width() < 1.0 || r.height() < 1.0 => {}
                Some(Some(r)) if r.area() < 0.7 * w as f64 * h as f64 => {
                    let (x0, y0, rw, rh) = (r.x0 as usize, r.y0 as usize, r.width() as u16, r.height() as u16);
                    let rctx = self.rctx.get_or_insert_with(|| RenderContext::new(rw, rh));
                    rctx.reset_and_resize(rw, rh);
                    st.org = (x0 as i32, y0 as i32);
                    draw_ops(st, &assets, rctx, ops, Affine::translate((-(x0 as f64), -(y0 as f64))));
                    rctx.flush();
                    self.rpix.resize(rw, rh);
                    let (fw, rwb) = (w as usize * 4, rw as usize * 4);
                    let frame = self.frame.data_as_u8_slice_mut();
                    for (i, row) in self.rpix.data_as_u8_slice_mut().chunks_exact_mut(rwb).enumerate() {
                        let o = (y0 + i) * fw + x0 * 4;
                        row.copy_from_slice(&frame[o..o + rwb]);
                    }
                    self.rpix.set_may_have_transparency(false);
                    rctx.render_with(&mut self.rpix, &mut st.res, settings);
                    rctx.reset();
                    st.org = (0, 0);
                    for (i, row) in self.rpix.data_as_u8_slice().chunks_exact(rwb).enumerate() {
                        let o = (y0 + i) * fw + x0 * 4;
                        frame[o..o + rwb].copy_from_slice(row);
                    }
                }
                _ => {
                    self.ctx.reset();
                    draw_ops(st, &assets, &mut self.ctx, ops, Affine::IDENTITY);
                    self.ctx.flush();
                    self.ctx.render_with(&mut self.frame, &mut st.res, settings);
                    // Release the paints that referenced offscreen bitmaps.
                    self.ctx.reset();
                }
            }
        } else {
            let [x0, y0, x1, y1] = region.unwrap_or_default();
            if x1 <= x0 || y1 <= y0 {
                return false;
            }
            let (rw, rh) = ((x1 - x0) as u16, (y1 - y0) as u16);
            let rctx = self.rctx.get_or_insert_with(|| RenderContext::new(rw, rh));
            rctx.reset_and_resize(rw, rh);
            st.org = (x0 as i32, y0 as i32);
            let cull = rects.map(|rects| Cull { region: Rect::new(0.0, 0.0, rw as f64, rh as f64), rects: &rects[skip.min(rects.len())..], org: (x0 as f64, y0 as f64) });
            draw_ops_culled(st, &assets, rctx, ops, Affine::translate((-(x0 as f64), -(y0 as f64))), cull);
            rctx.flush();
            self.rpix.resize(rw, rh);
            if let Some((_, pix, dev)) = &backdrop {
                self.upscaler.run(pix.data_as_u8_slice(), pix.width() as usize, pix.height() as usize, *dev, self.rpix.data_as_u8_slice_mut(), rw as usize, rh as usize, x0 as i64, y0 as i64);
                self.rpix.set_may_have_transparency(false);
            }
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
        // Keep the low-res backdrop for the next frame.
        if from_smooth {
            if let Some((_, p, _)) = backdrop {
                self.smooth_lo = Arc::try_unwrap(p).unwrap_or_else(|p| (*p).clone());
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
        fused
    }

    /// Render and convert to YUV 4:2:0 (BT.709 limited range) in one call.
    ///
    /// Over a full-frame backdrop (image field or smooth gradients) the YUV is
    /// produced directly from upscaled rows, and the RGBA [`Self::frame`] is not
    /// fully updated: call [`Self::render`] when you need the RGBA pixels.
    pub fn render_yuv(&mut self, list: &DisplayList, out: &mut YuvFrame) {
        if self.post.is_some() || self.st.reference {
            self.render(list);
            rgba_to_yuv420(&self.frame, out);
            return;
        }
        if !self.render_region(list, None, true, None, Some(out)) {
            rgba_to_yuv420(&self.frame, out);
        }
    }
}

/// The last top-level op that is an opaque, axis-aligned image covering the
/// whole `w × h` frame: (index, image, device rect). Everything before it is hidden.
fn backdrop(ops: &[Op], assets: &View, w: f64, h: f64) -> Option<(usize, Arc<Pixmap>, Rect)> {
    let mut depth = 0usize;
    let mut found: Option<(usize, dl::ImageId, Rect)> = None;
    for (i, op) in ops.iter().enumerate() {
        match op {
            Op::PushLayer { .. } => depth += 1,
            Op::PopLayer => depth = depth.saturating_sub(1),
            Op::Image { image, dst, opacity, transform } if depth == 0 && *opacity >= 1.0 => {
                let [a, b, c, d, e, f] = *transform;
                if b != 0.0 || c != 0.0 || a <= 0.0 || d <= 0.0 || !op_ok(op, Affine::IDENTITY) {
                    continue;
                }
                let r = Rect::new(
                    (a * dst[0] + e) as f64,
                    (d * dst[1] + f) as f64,
                    (a * (dst[0] + dst[2]) + e) as f64,
                    (d * (dst[1] + dst[3]) + f) as f64,
                );
                if r.x0 <= 0.0 && r.y0 <= 0.0 && r.x1 >= w && r.y1 >= h && r.width() > 0.0 && r.height() > 0.0 {
                    found = Some((i, *image, r));
                }
            }
            _ => {}
        }
    }
    let (i, id, r) = found?;
    let pix = assets.image(id)?;
    (!pix.may_have_transparency()).then_some((i, pix, r))
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
                if let (false, [dl::Filter::Shadow { dx, dy, blur, color: sc }]) = (st.reference, filters.as_slice()) {
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
    // The sprite is drawn from the quantised key values, so its pixels depend
    // only on the key, never on which frame filled the cache first.
    let q = |v: f64| (v * 16.0).round() as u64;
    let dq = |v: f64| q(v) as f64 / 16.0;
    let (w, h) = (dq(dev.width()), dq(dev.height()));
    let r = dq(card.radius as f64 * card.scale as f64);
    let sigma = dq(sigma as f64) as f32;
    let cq = c.0.map(|v| (v.clamp(0.0, 1.0) * 1023.0).round() as u32);
    let c = Rgba(cq.map(|v| v as f32 / 1023.0));
    let m = (sigma * 3.0).ceil() as f64 + 1.0;
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (q(w), q(h), q(r), q(sigma as f64), cq).hash(&mut hs);
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

/// Draw a solid-colour glyph run from a cached bitmap (rendered once per
/// content, linear transform `lin` and quarter-pixel phase). Returns false if
/// the run can't be cached (colour font, huge) and must be drawn as outlines.
#[allow(clippy::too_many_arguments)]
fn draw_cached_run(
    st: &mut State,
    ctx: &mut RenderContext,
    font_id: dl::FontId,
    font: &Face,
    size: f32,
    glyphs: &[dl::Glyph],
    c: Rgba,
    lin: [f64; 4],
    tx: f64,
    ty: f64,
) -> bool {
    let (fx, fy) = (tx.floor(), ty.floor());
    let identity = lin == [1.0, 0.0, 0.0, 1.0];
    let shape = |run: &mut BezPath| {
        if !identity {
            run.apply_affine(Affine::new([lin[0], lin[1], lin[2], lin[3], 0.0, 0.0]));
        }
    };
    if st.reference {
        // Exact sub-pixel phase, same coverage shaping, nothing cached.
        let mut run = std::mem::take(&mut st.run);
        if !st.glyphs.run_path(font_id, font, size, glyphs, &mut run) {
            st.run = run;
            return false;
        }
        shape(&mut run);
        let ok = run.is_empty() || {
            let (ox, oy) = (tx - fx, ty - fy);
            let bb = run.bounding_box();
            let (x0, y0) = ((bb.x0 + ox).floor() - 1.0, (bb.y0 + oy).floor() - 1.0);
            let (w, h) = ((bb.x1 + ox).ceil() + 1.0 - x0, (bb.y1 + oy).ceil() + 1.0 - y0);
            w <= 4096.0 && h <= 4096.0 && {
                let pix = text_bitmap(st, &run, c, ox - x0, oy - y0, w as u16, h as u16);
                blit(ctx, Arc::new(pix), fx + x0, fy + y0, 1.0);
                true
            }
        };
        st.run = run;
        return ok;
    }
    let (qx, qy) = (((tx - fx) * 4.0).round() as u8, ((ty - fy) * 4.0).round() as u8);
    let (ox, oy) = (qx as f64 / 4.0, qy as f64 / 4.0);
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (font_id.0, size.to_bits(), qx, qy, c.0.map(f32::to_bits), lin.map(f64::to_bits)).hash(&mut hs);
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
            shape(&mut run);
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
            let pm = text_bitmap(st, &run, c, ox - x0, oy - y0, w as u16, h as u16);
            st.run = run;
            let pix = Arc::new(pm);
            let (x0, y0) = (x0 as i32, y0 as i32);
            st.text.insert(key, CacheEntry { pix: pix.clone(), x: x0, y: y0, last_used: now });
            (pix, x0, y0)
        }
    };
    blit(ctx, pix, fx + x0 as f64, fy + y0 as f64, 1.0);
    true
}

/// Rasterise a glyph run (already a path) into a `w × h` bitmap at offset
/// `(dx, dy)`, with gamma-aware coverage.
fn text_bitmap(st: &mut State, run: &BezPath, c: Rgba, dx: f64, dy: f64, w: u16, h: u16) -> Pixmap {
    let mut sub = st.pool.pop().unwrap_or_else(|| RenderContext::new(w, h));
    sub.reset_and_resize(w, h);
    sub.set_paint(color(c));
    sub.set_fill_rule(Fill::NonZero);
    sub.set_transform(Affine::translate((dx, dy)));
    sub.fill_path(run);
    sub.flush();
    let mut pm = st.pix_pool.pop().unwrap_or_else(|| Pixmap::new(w, h));
    pm.resize(w, h);
    sub.render_with(&mut pm, &mut st.res, RasterizerSettings::default());
    text_gamma(&mut pm, c);
    sub.reset();
    st.pool.push(sub);
    pm.set_may_have_transparency(true);
    pm
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
    let reference = st.reference;
    let q = |v: f64| if reference { v } else { (v * 4.0).round() / 4.0 };
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
    if let (false, Some(e)) = (reference, st.scaled.get_mut(&key)) {
        e.last_used = now;
        let p = e.pix.clone();
        blit(ctx, p, fx, fy, alpha);
        return true;
    }
    // Only pay for resampling once the same placement shows up in consecutive frames.
    // Enlarged images (low-res fields, small textures) get the smooth cubic
    // filter at standard quality, every time; others are resampled once they repeat.
    let enlarge = st.cubic && (qw >= 2.0 * iw || qh >= 2.0 * ih);
    let seen_last = reference || st.seen.insert(key, now).is_some_and(|f| f + 1 >= now);
    if !seen_last && !enlarge {
        return false;
    }
    let (ow, oh) = ((px + qw).ceil() as usize, (py + qh).ceil() as usize);
    if enlarge {
        st.resample_buf.resize(ow * oh * 4, 0);
        st.up.invalidate();
        st.up.run(pix.data_as_u8_slice(), iw as usize, ih as usize, Rect::new(px, py, px + qw, py + qh), &mut st.resample_buf, ow, oh, 0, 0);
        // Edge pixels the image only partly covers: scale by coverage.
        let cov = |o: usize, a: f64, b: f64| ((o as f64 + 1.0).min(b) - (o as f64).max(a)).clamp(0.0, 1.0) as f32;
        for y in [0, oh - 1] {
            let cy = cov(y, py, py + qh);
            if cy < 1.0 {
                for p in st.resample_buf[y * ow * 4..(y + 1) * ow * 4].iter_mut() {
                    *p = (*p as f32 * cy + 0.5) as u8;
                }
            }
        }
        for x in [0, ow - 1] {
            let cx = cov(x, px, px + qw);
            if cx < 1.0 {
                for y in 0..oh {
                    for p in st.resample_buf[(y * ow + x) * 4..(y * ow + x) * 4 + 4].iter_mut() {
                        *p = (*p as f32 * cx + 0.5) as u8;
                    }
                }
            }
        }
    } else {
        resample::resample(pix.data_as_u8_slice(), iw as usize, ih as usize, px, py, qw, qh, ow, oh, &mut st.resample_buf);
    }
    let mut pm = Pixmap::new(ow as u16, oh as u16);
    pm.data_as_u8_slice_mut().copy_from_slice(&st.resample_buf[..ow * oh * 4]);
    pm.recompute_may_have_transparency();
    let p = Arc::new(pm);
    if !reference {
        st.scaled.insert(key, CacheEntry { pix: p.clone(), x: 0, y: 0, last_used: now });
    }
    blit(ctx, p, fx, fy, alpha);
    true
}

/// Gamma-aware text coverage. Blending in sRGB makes light-on-dark text look
/// thin and dark-on-light text look heavy; re-shape edge coverage by a power
/// chosen from the text's luminance (as Skia/FreeType "contrast" hinting does).
fn text_gamma(pm: &mut Pixmap, c: Rgba) {
    let lum = 0.2126 * c.0[0] + 0.7152 * c.0[1] + 0.0722 * c.0[2];
    let e = 1.2 - 0.46 * lum.clamp(0.0, 1.0);
    if (e - 1.0).abs() < 0.02 {
        return;
    }
    let ca = (c.0[3].clamp(0.0, 1.0) * 255.0).round().max(1.0);
    let lut: Vec<u8> = (0..=255u32)
        .map(|a| {
            let cov = (a as f32 / ca).min(1.0);
            (cov.powf(e) * ca).round().min(255.0) as u8
        })
        .collect();
    for p in pm.data_as_u8_slice_mut().chunks_exact_mut(4) {
        let a = p[3];
        if a == 0 || a as f32 >= ca {
            continue;
        }
        let na = lut[a as usize];
        let k = na as u32 * 256 / a as u32;
        for v in &mut p[..3] {
            *v = ((*v as u32 * k + 128) >> 8).min(na as u32) as u8;
        }
        p[3] = na;
    }
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
    // Filtered layers without a key are cached by content: a static glow or
    // blur is filtered once, while its opacity can still animate.
    let cache_key = cache_key.or_else(|| {
        let frame_base = Affine::translate((org.0 as f64, org.1 as f64)) * base;
        incremental::auto_layer_key(inner, filters, &frame_base)
    });
    let cache_key = cache_key.filter(|_| !st.reference);
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
            // Integer translations sample texel centres exactly: nearest is identical and ~4× cheaper.
            let t = affine(transform).as_coeffs();
            let exact = t[0] == 1.0 && t[1] == 0.0 && t[2] == 0.0 && t[3] == 1.0 && t[4].fract() == 0.0 && t[5].fract() == 0.0;
            ctx.set_paint(Image {
                image: ImageSource::Pixmap(pix),
                // `Paint::Image` tiles; `Op::Image` draws once.
                sampler: ImageSampler {
                    alpha: opacity.min(1.0),
                    quality: if exact { ImageQuality::Low } else { ImageQuality::Medium },
                    x_extend: Extend::Repeat,
                    y_extend: Extend::Repeat,
                },
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
            if draw_image_fast(st, ctx, *image, &pix, dst, *opacity, t) {
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
                if k[0] * k[3] - k[1] * k[2] != 0.0 {
                    if draw_cached_run(st, ctx, *font_id, font, *size, glyphs, *c, [k[0], k[1], k[2], k[3]], k[4], k[5]) {
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
                // Colour glyphs (COLR/bitmap emoji) ignore paint alpha: apply it as a layer.
                let alpha = match paint {
                    Paint::Solid(c) => c.0[3].clamp(0.0, 1.0),
                    _ => 1.0,
                };
                if alpha < 1.0 {
                    if let Paint::Solid(c) = paint {
                        ctx.set_paint(color(Rgba([c.0[0], c.0[1], c.0[2], 1.0])));
                    }
                    ctx.push_layer(None, None, Some(alpha), None, None);
                }
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
                if alpha < 1.0 {
                    ctx.pop_layer();
                }
            }
        }
        Op::PushLayer { .. } | Op::PopLayer => {}
    }
}
