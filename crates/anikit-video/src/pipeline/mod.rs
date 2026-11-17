//! Frames → file: parallel rendering, identical-frame dedupe, YUV, encoders, audio mux.
//!
//! Frames are evaluated and rendered in ordered batches on the rayon pool,
//! one [`Renderer`] per worker thread (re-created only when shared assets
//! change). A frame whose display list hashes the same as the previous
//! frame's isn't rendered again. Finished YUV frames go to an [`Encoder`] in
//! order.

pub mod encode;
pub mod textures;

use crate::dl::{self, DisplayList, Op};
use crate::eval::Evaluator;
use crate::render::{Renderer, YuvFrame};
use rayon::prelude::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub use encode::{Codec, Encoder, FfmpegEncoder, Quality};

/// Output settings.
#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub codec: Codec,
    pub quality: Quality,
    /// Time range in seconds (default: the whole video).
    pub from: Option<f32>,
    pub to: Option<f32>,
    /// Output scale (0.5 = half resolution).
    pub scale: f32,
    /// Print progress to stderr.
    pub progress: bool,
    /// Mux audio.
    pub audio: bool,
    /// Print a per-stage timing breakdown to stderr.
    pub profile: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { codec: Codec::H264, quality: Quality::Good, from: None, to: None, scale: 1.0, progress: false, audio: true, profile: false }
    }
}

/// Machine-readable summary of a render.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Summary {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub frames: u32,
    /// Frames skipped because they were identical to the previous one.
    pub deduped: u32,
    pub duration: f64,
    /// Wall-clock seconds.
    pub seconds: f64,
    /// Frames per second achieved, end to end.
    pub render_fps: f64,
    pub bytes: u64,
    pub codec: String,
    /// Per-stage timing (fused ak264 path only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<StageProfile>,
}

/// Where the time went in a fused render, CPU ms per frame summed over threads.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct StageProfile {
    pub threads: usize,
    pub gop: u32,
    pub eval_ms: f64,
    pub render_ms: f64,
    pub copy_ms: f64,
    pub encode_ms: f64,
    /// Cores waiting (load imbalance, startup), ms per frame. Also counts work a
    /// stage hands to other threads (nested parallelism in shaders, 3D, post): it
    /// is wall × threads minus the per-frame stage times, so it overstates idling there.
    pub idle_ms: f64,
    /// Wall ms per frame for render + encode (excludes mux).
    pub wall_ms: f64,
    pub mux_ms: f64,
    pub unchanged: u64,
    pub held: u64,
    pub partial: u64,
    pub full: u64,
}

/// Called with every frame exactly as it goes to the encoder (index within the range, tight 4:2:0 planes).
pub type Tap<'a> = &'a (dyn Fn(usize, &ak264::YuvBuffer) + Sync);

/// Hash a display list (bit-exact on floats), for dedupe.
pub fn hash_list(dl: &DisplayList) -> u64 {
    struct H(std::collections::hash_map::DefaultHasher);
    let mut h = H(std::collections::hash_map::DefaultHasher::new());
    impl H {
        fn f(&mut self, v: f32) {
            v.to_bits().hash(&mut self.0)
        }
        fn fs(&mut self, v: &[f32]) {
            for x in v {
                self.f(*x)
            }
        }
        fn paint(&mut self, p: &dl::Paint) {
            match p {
                dl::Paint::Solid(c) => {
                    0u8.hash(&mut self.0);
                    self.fs(&c.0)
                }
                dl::Paint::Linear { p0, p1, stops } => {
                    1u8.hash(&mut self.0);
                    self.fs(p0);
                    self.fs(p1);
                    for (o, c) in stops {
                        self.f(*o);
                        self.fs(&c.0);
                    }
                }
                dl::Paint::Radial { c, r, stops } => {
                    2u8.hash(&mut self.0);
                    self.fs(c);
                    self.f(*r);
                    for (o, c) in stops {
                        self.f(*o);
                        self.fs(&c.0);
                    }
                }
                dl::Paint::Image { image, transform, opacity } => {
                    3u8.hash(&mut self.0);
                    image.hash(&mut self.0);
                    self.fs(transform);
                    self.f(*opacity);
                }
            }
        }
        fn path(&mut self, p: &dl::Path) {
            for el in p.elements() {
                // Debug formatting is stable and exact enough for f64 points.
                let pts: [f64; 6] = match *el {
                    kurbo::PathEl::MoveTo(a) => [0.0, a.x, a.y, 0.0, 0.0, 0.0],
                    kurbo::PathEl::LineTo(a) => [1.0, a.x, a.y, 0.0, 0.0, 0.0],
                    kurbo::PathEl::QuadTo(a, b) => [2.0, a.x, a.y, b.x, b.y, 0.0],
                    kurbo::PathEl::CurveTo(a, b, c) => [a.x, a.y, b.x, b.y, c.x, c.y + 3.0],
                    kurbo::PathEl::ClosePath => [4.0; 6],
                };
                for v in pts {
                    v.to_bits().hash(&mut self.0);
                }
            }
        }
    }
    dl.width.hash(&mut h.0);
    dl.height.hash(&mut h.0);
    h.fs(&dl.background.0);
    for op in &dl.ops {
        match op {
            Op::Fill { path, paint, rule, transform } => {
                10u8.hash(&mut h.0);
                h.path(path);
                h.paint(paint);
                (*rule as u8).hash(&mut h.0);
                h.fs(transform);
            }
            Op::Stroke { path, paint, stroke, transform } => {
                11u8.hash(&mut h.0);
                h.path(path);
                h.paint(paint);
                h.f(stroke.width);
                (stroke.cap, stroke.join).hash(&mut h.0);
                h.f(stroke.miter_limit);
                h.fs(&stroke.dash);
                h.f(stroke.dash_offset);
                h.fs(&stroke.trim);
                h.fs(transform);
            }
            Op::RoundRect { x, y, w, h: hh, radius, paint, transform } => {
                12u8.hash(&mut h.0);
                h.fs(&[*x, *y, *w, *hh, *radius]);
                h.paint(paint);
                h.fs(transform);
            }
            Op::Image { image, dst, opacity, transform } => {
                13u8.hash(&mut h.0);
                image.hash(&mut h.0);
                h.fs(dst);
                h.f(*opacity);
                h.fs(transform);
            }
            Op::Glyphs { font, size, glyphs, paint, transform } => {
                14u8.hash(&mut h.0);
                font.hash(&mut h.0);
                h.f(*size);
                for g in glyphs {
                    g.id.hash(&mut h.0);
                    h.f(g.x);
                    h.f(g.y);
                }
                h.paint(paint);
                h.fs(transform);
            }
            Op::PushLayer { opacity, blend, filters, clip, cache_key } => {
                15u8.hash(&mut h.0);
                h.f(*opacity);
                (*blend as u8).hash(&mut h.0);
                format!("{filters:?}").hash(&mut h.0);
                if let Some((p, t)) = clip {
                    h.path(p);
                    h.fs(t);
                }
                cache_key.hash(&mut h.0);
            }
            Op::PopLayer => 16u8.hash(&mut h.0),
        }
    }
    h.0.finish()
}

/// Scale a display list about the origin (for `--scale`).
pub fn scale_list(dl: &mut DisplayList, s: f32) {
    if s == 1.0 {
        return;
    }
    dl.width = ((dl.width as f32 * s).round() as u32).max(2);
    dl.height = ((dl.height as f32 * s).round() as u32).max(2);
    let m = [s, 0.0, 0.0, s, 0.0, 0.0];
    crate::components::transform_ops(&mut dl.ops, &m);
    for op in &mut dl.ops {
        if let Op::PushLayer { filters, .. } = op {
            for f in filters {
                match f {
                    dl::Filter::Blur(r) => *r *= s,
                    dl::Filter::Shadow { dx, dy, blur, .. } => {
                        *dx *= s;
                        *dy *= s;
                        *blur *= s;
                    }
                    dl::Filter::Glow { radius, .. } => *radius *= s,
                    dl::Filter::ColorMatrix(_) => {}
                }
            }
        }
    }
}

struct Worker {
    version: u64,
    /// The asset store the renderer draws from.
    store: Arc<crate::eval::SharedAssets>,
    renderer: Option<Renderer>,
    dl: DisplayList,
    /// Fused mode: this thread's incremental YUV frame, change map and last frame index.
    yuv: YuvFrame,
    map: crate::render::MbMap,
    last: Option<usize>,
    /// When this thread last returned a frame to the encoder (fused mode), to time encoding.
    returned: Option<Instant>,
}

thread_local! {
    static WORKER: RefCell<Worker> = RefCell::new(new_worker());
}

/// Run `f` with this thread's renderer, (re)creating it when assets changed.
fn new_worker() -> Worker {
    Worker { version: 0, store: Arc::default(), renderer: None, dl: DisplayList::default(), yuv: YuvFrame::default(), map: crate::render::MbMap::new(0, 0), last: None, returned: None }
}

/// This thread's worker state. A nested call (rayon work-stealing while the state is
/// in use further up the stack) gets a fresh temporary worker instead of panicking:
/// it renders a full frame and shares no incremental state, which is always correct.
fn with_worker<R>(f: impl FnOnce(&mut Worker) -> R) -> R {
    WORKER.with(|c| match c.try_borrow_mut() {
        Ok(mut w) => f(&mut w),
        Err(_) => f(&mut new_worker()),
    })
}

/// Create (or re-create, when fonts were added) this worker's renderer.
fn ensure_renderer(ev: &Evaluator, w: &mut Worker) {
    // Only new font faces need new assets; images come from a live source. New faces are
    // swapped into the existing renderer (ids are stable), never a new renderer: dropping
    // its incremental state at a moment set by other threads made output nondeterministic.
    let v = (ev.assets.font_count() as u64) ^ ((Arc::as_ptr(&ev.assets) as u64) << 16);
    match &mut w.renderer {
        Some(r) if w.version != v && Arc::ptr_eq(&ev.assets, &w.store) => r.update_assets(ev.assets.snapshot_fonts()),
        Some(_) if w.version == v => {}
        _ => {
            let mut r = Renderer::new(ev.assets.snapshot_fonts());
            let a = ev.assets.clone();
            r.set_image_source(Arc::new(move |id| a.pixmap(id)));
            w.renderer = Some(r);
            w.store = ev.assets.clone();
            w.last = None;
        }
    }
    w.version = v;
}

fn with_renderer<R>(ev: &Evaluator, f: impl FnOnce(&mut Renderer, &mut DisplayList) -> R) -> R {
    with_worker(|w| {
        ensure_renderer(ev, w);
        let Worker { renderer, dl, .. } = w;
        f(renderer.as_mut().unwrap(), dl)
    })
}

/// Per-stage CPU time (summed over threads), for `--profile`.
#[derive(Default)]
pub struct Profile {
    pub eval_ns: AtomicU64,
    pub render_ns: AtomicU64,
    pub copy_ns: AtomicU64,
    /// Time the encoder spent between our callbacks on the same chunk.
    pub encode_ns: AtomicU64,
    pub unchanged: AtomicU64,
    /// Frames skipped without evaluating (static spans).
    pub held: AtomicU64,
    pub partial: AtomicU64,
    pub full: AtomicU64,
}

fn ns(t: Instant) -> u64 {
    t.elapsed().as_nanos() as u64
}

/// Copy macroblock `(mx, my)` (or every one when `map` is `None`) from a padded render frame into a tight buffer.
fn copy_yuv(src: &YuvFrame, dst: &mut ak264::YuvBuffer, map: Option<&crate::render::MbMap>) {
    let (w, h) = (dst.width as usize, dst.height as usize);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    match map {
        None => {
            for y in 0..h {
                dst.y[y * w..y * w + w].copy_from_slice(&src.y[y * src.y_stride..y * src.y_stride + w]);
            }
            for y in 0..ch {
                dst.u[y * cw..y * cw + cw].copy_from_slice(&src.u[y * src.c_stride..y * src.c_stride + cw]);
                dst.v[y * cw..y * cw + cw].copy_from_slice(&src.v[y * src.c_stride..y * src.c_stride + cw]);
            }
        }
        Some(m) => {
            for my in 0..m.mb_height.min(h.div_ceil(16)) {
                let mut mx = 0;
                let mbw = m.mb_width.min(w.div_ceil(16));
                while mx < mbw {
                    if !m.get(mx, my) {
                        mx += 1;
                        continue;
                    }
                    // Copy a horizontal run of changed macroblocks at once.
                    let start = mx;
                    while mx < mbw && m.get(mx, my) {
                        mx += 1;
                    }
                    let (x0, x1) = (start * 16, (mx * 16).min(w));
                    for y in my * 16..((my + 1) * 16).min(h) {
                        dst.y[y * w + x0..y * w + x1].copy_from_slice(&src.y[y * src.y_stride + x0..y * src.y_stride + x1]);
                    }
                    let (c0, c1) = (x0 / 2, x1.div_ceil(2).min(cw));
                    for y in my * 8..((my + 1) * 8).min(ch) {
                        dst.u[y * cw + c0..y * cw + c1].copy_from_slice(&src.u[y * src.c_stride + c0..y * src.c_stride + c1]);
                        dst.v[y * cw + c0..y * cw + c1].copy_from_slice(&src.v[y * src.c_stride + c0..y * src.c_stride + c1]);
                    }
                }
            }
        }
    }
}

/// Fused render + encode with ak264: frames are split into GOP chunks; each
/// worker evaluates, renders incrementally (only changed regions are
/// rasterised) and encodes its chunk, and unchanged macroblocks skip both
/// rasterising and encoding. Returns the Annex-B stream.
pub fn render_fused(ev: &Evaluator, opts: &RenderOptions, cfg: &ak264::EncoderConfig, prof: &Profile) -> Vec<u8> {
    render_fused_tap(ev, opts, cfg, prof, None)
}

/// [`render_fused`], showing each finished frame to `tap` before it is encoded.
pub fn render_fused_tap(ev: &Evaluator, opts: &RenderOptions, cfg: &ak264::EncoderConfig, prof: &Profile, tap: Option<Tap>) -> Vec<u8> {
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total) as usize;
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0 as u32, total) as usize;
    let done = AtomicU64::new(0);
    let t0 = Instant::now();
    let last_report = std::sync::Mutex::new(Instant::now());
    let n = f1 - f0;
    // Chunk starts exactly as the encoder splits them (each starts with a fresh buffer and an IDR).
    // Hints use the encoder's macroblock grid (its size is cropped to even dimensions).
    let (emw, emh) = ((cfg.width as usize).div_ceil(16), (cfg.height as usize).div_ceil(16));
    let hint_map = |map: &crate::render::MbMap| -> Vec<bool> {
        let mut v = vec![false; emw * emh];
        for y in 0..emh.min(map.mb_height) {
            for x in 0..emw.min(map.mb_width) {
                v[y * emw + x] = map.get(x, y);
            }
        }
        v
    };
    // Deterministic output: chunk ranges don't depend on the thread count, and each
    // chunk renders with its own state (fresh at the chunk start, then incremental
    // frame by frame), whichever thread runs it.
    let ranges = chunk_plan(n, cfg.gop);
    let starts: std::collections::HashSet<usize> = ranges.iter().map(|r| r.0).collect();
    let chunk_start = |k: usize| ranges[ranges.partition_point(|r| r.0 <= k) - 1].0;
    let workers: std::sync::Mutex<std::collections::HashMap<usize, Worker>> = Default::default();
    let pool: std::sync::Mutex<Vec<Worker>> = Default::default();
    let out = ak264::encode_ranges(cfg, ranges.clone(), |k, buf, hints| {
        let t = Instant::now();
        let c = chunk_start(k);
        // A chunk starts from a pooled renderer (its caches are history-independent:
        // RENDER's `output_is_independent_of_render_history`) with its incremental state reset.
        let mut wk = if k == c {
            let mut w = pool.lock().unwrap().pop().unwrap_or_else(new_worker);
            if let Some(r) = &mut w.renderer {
                r.invalidate();
            }
            w.last = None;
            w
        } else {
            workers.lock().unwrap().remove(&c).unwrap_or_else(new_worker)
        };
        (|w: &mut Worker| {
            ensure_renderer(ev, w);
            if let (Some(r), Some(l)) = (w.returned.take(), w.last) {
                if l + 1 == k && !starts.contains(&k) {
                    prof.encode_ns.fetch_add((t - r).as_nanos() as u64, Ordering::Relaxed);
                }
            }
            let Worker { dl, last, returned, .. } = &mut *w;
            let continuing = !starts.contains(&k) && *last == Some(k.wrapping_sub(1));
            // Held frame: provably identical to the previous one; skip eval and render.
            let fi = (f0 + k) as f32;
            let (post, blur) = frame_look(ev, opts.quality, (f0 + k) as u32);
            let sh = if blur || ev.shutter_explicit || opts.quality == Quality::Best { ev.shutter * 0.5 } else { 0.0 };
            if continuing && !post.as_ref().is_some_and(|p| p.grain > 0.0) && ev.is_static((fi - 1.0) / ev.fps - sh, fi / ev.fps + sh) {
                prof.unchanged.fetch_add(1, Ordering::Relaxed);
                prof.held.fetch_add(1, Ordering::Relaxed);
                hints.changed = Some(vec![false; emw * emh]);
                *last = Some(k);
                *returned = Some(Instant::now());
                return;
            }
            eval_frame(ev, (f0 + k) as u32, opts.scale, dl);
            prof.eval_ns.fetch_add(ns(t), Ordering::Relaxed);
            // Evaluating can intern new font faces (layouts, counters): sync the renderer after it.
            ensure_renderer(ev, w);
            let Worker { renderer, dl, yuv, map, last, .. } = &mut *w;
            let r = renderer.as_mut().unwrap();
            let continuing = !starts.contains(&k) && *last == Some(k.wrapping_sub(1));
            let t = Instant::now();
            r.set_quality(render_quality(opts.quality));
            // Tell the encoder how much grain the post stack draws (σ in 8-bit luma at mid-grey),
            // so re-drawn random grain isn't spent bits on.
            let grain = post.as_ref().map(|p| p.grain * 89.4).unwrap_or(0.0);
            r.set_post(post, (f0 + k) as u64);
            use crate::render::incremental::Update;
            let upd = if blur {
                render_blurred(ev, r, (f0 + k) as u32, opts.scale, yuv);
                Update::Full
            } else {
                r.render_yuv_incremental(dl, yuv, map)
            };
            prof.render_ns.fetch_add(ns(t), Ordering::Relaxed);
            let t = Instant::now();
            match upd {
                Update::Unchanged => prof.unchanged.fetch_add(1, Ordering::Relaxed),
                Update::Partial { .. } => prof.partial.fetch_add(1, Ordering::Relaxed),
                Update::Full => prof.full.fetch_add(1, Ordering::Relaxed),
            };
            if continuing {
                // `map` is exact versus the previous frame, which `buf` still holds.
                match upd {
                    Update::Unchanged => hints.changed = Some(vec![false; emw * emh]),
                    Update::Full => {
                        copy_yuv(yuv, buf, None);
                        hints.changed = None;
                    }
                    _ => {
                        copy_yuv(yuv, buf, Some(map));
                        hints.changed = Some(hint_map(map));
                    }
                }
            } else {
                copy_yuv(yuv, buf, None);
                hints.changed = None;
            }
            hints.grain = grain;
            *last = Some(k);
            prof.copy_ns.fetch_add(ns(t), Ordering::Relaxed);
            w.returned = Some(Instant::now());
        })(&mut wk);
        if ranges.iter().any(|r| r.0 == c && k + 1 < r.1) {
            workers.lock().unwrap().insert(c, wk);
        } else {
            pool.lock().unwrap().push(wk);
        }
        if let Some(tap) = tap {
            tap(k, buf);
        }
        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
        if opts.progress {
            if let Ok(mut lr) = last_report.try_lock() {
                if lr.elapsed().as_secs_f32() > 0.5 {
                    *lr = Instant::now();
                    let el = t0.elapsed().as_secs_f32();
                    let fps = d as f32 / el.max(1e-6);
                    eprint!("\rrender {d}/{n} frames  {fps:.0} fps  {el:.1}s  ETA {:.0}s   ", (n as f32 - d as f32) / fps.max(1e-6));
                }
            }
        }
    });
    if opts.progress {
        eprintln!();
    }
    out
}

/// Default chunk length for `n` frames: up to 16 chunks of 30–300 frames, or, when the
/// spec has expensive layers (3D, shaders, video), chunks of 8+ frames (up to 16), so a
/// short heavy clip keeps every core busy. Depends only on the spec, never the machine.
pub fn default_gop(n: usize, heavy: bool) -> u32 {
    let min_len = if heavy { 8 } else { 30 };
    let count = n.div_ceil(300).max(n.div_ceil(min_len).min(16)).clamp(1, n.max(1));
    n.div_ceil(count).max(1) as u32
}

/// Independently encoded chunks for `n` frames. `gop > 0` caps their length;
/// otherwise up to 16 equal chunks of 30–300 frames (enough to keep 16 cores busy).
/// Depends only on its arguments, never on the machine.
pub fn chunk_plan(n: usize, gop: u32) -> Vec<(usize, usize)> {
    let count = if gop > 0 { n.div_ceil(gop as usize) } else { n.div_ceil(300).max(n.div_ceil(30).min(16)) }.clamp(1, n.max(1));
    (0..count).map(|i| (i * n / count, (i + 1) * n / count)).filter(|r| r.1 > r.0).collect()
}

/// Renderer settings for an output quality: only drafts take the fast approximations
/// (¼-res smooth backdrops, bilinear upscales) that change visible output.
pub fn render_quality(q: Quality) -> crate::render::Quality {
    match q {
        Quality::Draft => crate::render::Quality::Draft,
        Quality::Good => crate::render::Quality::Standard,
        Quality::Best => crate::render::Quality::High,
    }
}

/// Frame `i`'s finishing look, and whether it gets motion blur (never in drafts; by default only at high quality).
pub fn frame_look(ev: &Evaluator, q: Quality, i: u32) -> (Option<crate::render::post::PostSettings>, bool) {
    let t = frame_time(ev, i);
    (ev.post_at(t).cloned(), q != Quality::Draft && ev.needs_blur(t, q == Quality::Best))
}

/// Render frame `i` with motion blur into `yuv` (a full frame), leaving the
/// renderer ready for a full incremental render next.
fn render_blurred(ev: &Evaluator, r: &mut Renderer, i: u32, scale: f32, yuv: &mut YuvFrame) {
    let mut eval = |t: f32| {
        let mut d = ev.eval(t);
        scale_list(&mut d, scale);
        d
    };
    r.render_yuv_motion_blur(&mut eval, frame_time(ev, i), ev.shutter, yuv);
    r.invalidate();
}

/// Evaluate frame `i` into `out` at `scale`.
pub fn eval_frame(ev: &Evaluator, i: u32, scale: f32, out: &mut DisplayList) {
    ev.eval_into(frame_time(ev, i), out);
    scale_list(out, scale);
}

/// Render one frame to straight-alpha RGBA8 (for stills).
pub fn render_rgba(ev: &Evaluator, t: f32, scale: f32) -> (u32, u32, Vec<u8>) {
    // Shaders render at the output size (reset when this returns).
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            crate::eval::set_preview_scale(1.0);
        }
    }
    let _reset = Reset;
    crate::eval::set_preview_scale(scale);
    let mut dl = ev.eval(t);
    scale_list(&mut dl, scale);
    let post = ev.post_at(t).cloned();
    let frame = (t * ev.fps).round().max(0.0) as u64;
    if ev.needs_blur(t, false) {
        return with_renderer(ev, |r, _| {
            r.set_post(post, frame);
            let mut eval = |t: f32| {
                let mut d = ev.eval(t);
                scale_list(&mut d, scale);
                d
            };
            let px = straight(r.render_motion_blur(&mut eval, t, ev.shutter));
            r.invalidate();
            r.set_post(None, 0);
            px
        });
    }
    with_renderer(ev, |r, _| {
        r.set_post(post, frame);
        let px = straight(r.render(&dl));
        r.set_post(None, 0);
        px
    })
}

/// Render an already evaluated (and possibly edited) display list to straight RGBA8.
pub fn render_list_rgba(ev: &Evaluator, dl: &DisplayList) -> (u32, u32, Vec<u8>) {
    // Evaluating may have created images (components); render after.
    with_renderer(ev, |r, _| straight(r.render(dl)))
}

/// Premultiplied pixmap → straight RGBA8.
fn straight(pm: &crate::render::FrameRgba) -> (u32, u32, Vec<u8>) {
    let (w, h) = (pm.width() as u32, pm.height() as u32);
    let mut px = pm.data_as_u8_slice().to_vec();
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    (w, h, px)
}

/// Save straight RGBA8 as PNG.
pub fn save_png(path: &std::path::Path, w: u32, h: u32, rgba: Vec<u8>) -> Result<(), String> {
    let img = image::RgbaImage::from_raw(w, h, rgba).ok_or("bad image buffer")?;
    img.save(path).map_err(|e| format!("can't write {}: {e}", path.display()))
}

/// A contact sheet: `cols × rows` frames evenly spread over the video, each `cell_w` wide.
pub fn contact_sheet(ev: &Evaluator, cols: u32, rows: u32, cell_w: u32) -> (u32, u32, Vec<u8>) {
    let n = cols * rows;
    let scale = cell_w as f32 / ev.width as f32;
    let cw = ((ev.width as f32 * scale).round() as u32).max(2);
    let ch = ((ev.height as f32 * scale).round() as u32).max(2);
    let gap = 4;
    let (sw, sh) = (cols * cw + (cols + 1) * gap, rows * ch + (rows + 1) * gap);
    let cells: Vec<(u32, u32, Vec<u8>)> = (0..n)
        .into_par_iter()
        .map(|k| {
            let t = if n > 1 { ev.duration * (k as f32 + 0.5) / n as f32 } else { ev.duration * 0.5 };
            render_rgba(ev, t, scale)
        })
        .collect();
    let mut out = vec![24u8; (sw * sh * 4) as usize];
    for p in out.chunks_exact_mut(4) {
        p[3] = 255;
    }
    for (k, (w, h, px)) in cells.iter().enumerate() {
        let (cx, cy) = (gap + (k as u32 % cols) * (cw + gap), gap + (k as u32 / cols) * (ch + gap));
        for y in 0..(*h).min(ch) {
            let src = &px[(y * w * 4) as usize..((y * w + (*w).min(cw)) * 4) as usize];
            let o = (((cy + y) * sw + cx) * 4) as usize;
            out[o..o + src.len()].copy_from_slice(src);
        }
    }
    (sw, sh, out)
}

/// A deferred audio mix (music generation included): run it on any thread to write the WAV.
pub struct AudioJob {
    pub wav: std::path::PathBuf,
    run: Box<dyn FnOnce() -> Result<(), String> + Send>,
}

impl AudioJob {
    /// Mix now; returns the WAV path.
    pub fn run(self) -> Result<std::path::PathBuf, String> {
        (self.run)()?;
        Ok(self.wav)
    }
    /// A job for a WAV that already exists.
    pub fn ready(wav: &std::path::Path) -> AudioJob {
        AudioJob { wav: wav.to_path_buf(), run: Box::new(|| Ok(())) }
    }
}

/// The audio mix for `[t0, t0 + dur)` as a job writing a WAV at `wav`; `None` if the video is silent.
/// Generated music is rendered inside the job, so running it on a thread renders
/// the soundtrack in parallel with the video frames.
pub fn audio_job(ev: &Evaluator, t0: f32, dur: f32, wav: &std::path::Path) -> Option<AudioJob> {
    use crate::audio::{self, Role, Source};
    if ev.audio.is_empty() && ev.music.is_none() {
        return None;
    }
    let cues = ev.audio.clone();
    let music = ev.music.clone();
    let video_len = ev.duration as f64;
    let out = wav.to_path_buf();
    let run = move || -> Result<(), String> {
        let mut all = cues;
        let music_wav = out.with_extension("music.wav");
        if let Some(m) = &music {
            let has_voice = all.iter().any(|t| t.role() == Role::Voice);
            all.push(crate::music::render_track(m, video_len, &music_wav, has_voice)?);
        }
        let mut tracks = Vec::new();
        for t in &all {
            let mut t = t.clone();
            t.start -= t0 as f64;
            if t.start < 0.0 {
                match &t.src {
                    Source::File(_) => {
                        let tr = t.trim.unwrap_or([0.0, 0.0]);
                        t.trim = Some([tr[0] - t.start, tr[1]]);
                        if let Some(d) = &mut t.duration {
                            *d += t.start;
                        }
                        t.start = 0.0;
                    }
                    Source::Sfx { .. } => continue,
                }
            }
            tracks.push(t);
        }
        let pcm = audio::mix(&tracks, dur as f64, audio::DEFAULT_RATE, None);
        if music.is_some() {
            let _ = std::fs::remove_file(&music_wav);
        }
        audio::write_wav(&out, &pcm?, audio::WavFormat::Pcm16)
    };
    Some(AudioJob { wav: wav.to_path_buf(), run: Box::new(run) })
}

/// Mix the video's audio for `[t0, t0 + dur)` into a WAV at `wav`; `None` if the video is silent.
pub fn mix_audio(ev: &Evaluator, t0: f32, dur: f32, wav: &std::path::Path) -> Result<Option<std::path::PathBuf>, String> {
    audio_job(ev, t0, dur, wav).map(AudioJob::run).transpose()
}

/// Render to an MP4 with ak264 in fused mode (see [`render_fused`]); muxes
/// without ffmpeg when there is no audio.
pub fn render_ak264(ev: &Evaluator, opts: &RenderOptions, out: &std::path::Path, audio: Option<&std::path::Path>) -> Result<Summary, String> {
    render_ak264_job_tap(ev, opts, out, audio.map(AudioJob::ready), None)
}

/// [`render_ak264`], showing each frame to `tap` as it goes to the encoder (for the quality harness).
pub fn render_ak264_tap(ev: &Evaluator, opts: &RenderOptions, out: &std::path::Path, audio: Option<&std::path::Path>, tap: Option<Tap>) -> Result<Summary, String> {
    render_ak264_job_tap(ev, opts, out, audio.map(AudioJob::ready), tap)
}

/// [`render_ak264`] with the audio mixed (and music generated) on a thread
/// while the frames render, then encoded to AAC there too.
pub fn render_ak264_job(ev: &Evaluator, opts: &RenderOptions, out: &std::path::Path, audio: Option<AudioJob>) -> Result<Summary, String> {
    render_ak264_job_tap(ev, opts, out, audio, None)
}

/// [`render_ak264_job`] with a frame `tap` (see [`render_ak264_tap`]).
pub fn render_ak264_job_tap(ev: &Evaluator, opts: &RenderOptions, out: &std::path::Path, audio: Option<AudioJob>, tap: Option<Tap>) -> Result<Summary, String> {
    let t0 = Instant::now();
    let w = (((ev.width as f32 * opts.scale).round() as u32).max(2)) & !1;
    let h = (((ev.height as f32 * opts.scale).round() as u32).max(2)) & !1;
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total);
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0, total);
    let n = (f1 - f0) as usize;
    let threads = rayon::current_num_threads().max(1);
    // Chunk length: fixed by the frame count alone (never the machine), so output is identical on any core count.
    let gop = std::env::var("ANIKIT_GOP").ok().and_then(|g| g.parse().ok()).unwrap_or_else(|| default_gop(n, ev.has_heavy_layers()));
    // `--crf` / `encode.crf` (constant quantiser, lower = bigger and better) over the quality default.
    let qp = ev.encode_crf.unwrap_or(match opts.quality {
        Quality::Draft => 28,
        Quality::Good => 23,
        Quality::Best => 19,
    });
    let cfg = ak264::EncoderConfig {
        width: w,
        height: h,
        fps: ev.fps as f64,
        qp,
        gop,
        // In-loop deblocking: off only for drafts (it costs encode time, and blocks show without it).
        deblock: opts.quality != Quality::Draft,
        threads: 1,
        full_range: false,
        cabac: opts.quality != Quality::Draft,
        subpel: opts.quality != Quality::Draft,
        aq_strength: ev.encode_aq.unwrap_or(0.0),
        ..Default::default()
    };
    let prof = Profile::default();
    // Encode the audio to AAC while the frames render, so the final mux is a stream copy.
    let m4a = audio.as_ref().map(|_| out.with_extension(format!("audio-{}.m4a", std::process::id())));
    let has_audio = audio.is_some();
    let aac_job = match (audio, &m4a) {
        (Some(job), Some(m)) => {
            let m = m.clone();
            Some(std::thread::spawn(move || -> Result<std::process::Output, String> {
                let a = job.run()?;
                std::process::Command::new("ffmpeg")
                    .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                    .arg(&a)
                    .args(["-c:a", "aac", "-b:a", "192k"])
                    .arg(&m)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map_err(|e| format!("can't start ffmpeg for audio: {e}"))
            }))
        }
        _ => None,
    };
    let annexb = render_fused_tap(ev, opts, &cfg, &prof, tap);
    let t_enc = t0.elapsed();
    let tm = Instant::now();
    match has_audio {
        false => std::fs::write(out, ak264::mp4::mux(&annexb, w, h, ev.fps as f64)).map_err(|e| format!("can't write {}: {e}", out.display()))?,
        true => {
            // Mux from a temp MP4 (real timestamps), not a raw H.264 pipe: ffmpeg can stall
            // draining a timestamp-less pipe while it waits on the audio input.
            let tmp = out.with_extension(format!("video-{}.mp4", std::process::id()));
            std::fs::write(&tmp, ak264::mp4::mux(&annexb, w, h, ev.fps as f64)).map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
            let m4a = m4a.clone().unwrap();
            let aac = match aac_job.unwrap().join().map_err(|_| "audio thread panicked".to_string())? {
                Ok(o) => o,
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e);
                }
            };
            if !aac.status.success() {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!("ffmpeg (audio) failed: {}", String::from_utf8_lossy(&aac.stderr).trim()));
            }
            let o = std::process::Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&tmp)
                .arg("-i")
                .arg(&m4a)
                .args(["-map", "0:v", "-map", "1:a", "-c", "copy", "-shortest", "-movflags", "+faststart"])
                .arg(out)
                .stdin(std::process::Stdio::null())
                .output()
                .map_err(|e| format!("can't start ffmpeg for the audio mux: {e}"));
            let _ = std::fs::remove_file(&tmp);
            let _ = std::fs::remove_file(&m4a);
            let o = o?;
            if !o.status.success() {
                return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
            }
        }
    }
    let t_mux = tm.elapsed();
    let secs = t0.elapsed().as_secs_f32();
    let unchanged = prof.unchanged.load(Ordering::Relaxed) as u32;
    let per = |x: &AtomicU64| x.load(Ordering::Relaxed) as f64 / 1e6 / n.max(1) as f64;
    let (e, r, c) = (per(&prof.eval_ns), per(&prof.render_ns), per(&prof.copy_ns));
    let wall = t_enc.as_secs_f64() * 1e3 / n.max(1) as f64;
    let enc = per(&prof.encode_ns);
    let idle = (wall * threads as f64 - e - r - c - enc).max(0.0);
    let sp = StageProfile {
        threads,
        gop,
        eval_ms: e,
        render_ms: r,
        copy_ms: c,
        encode_ms: enc,
        idle_ms: idle,
        wall_ms: wall,
        mux_ms: t_mux.as_secs_f64() * 1e3,
        unchanged: unchanged as u64,
        held: prof.held.load(Ordering::Relaxed),
        partial: prof.partial.load(Ordering::Relaxed),
        full: prof.full.load(Ordering::Relaxed),
    };
    if opts.profile {
        eprintln!(
            "profile ({n} frames, {threads} threads, gop {gop}; CPU ms/frame summed over threads)\n  eval    {e:7.3}\n  render  {r:7.3}  (incremental+YUV: {} unchanged ({} held, not even evaluated), {} partial, {} full)\n  copy    {c:7.3}\n  encode  {enc:7.3}\n  idle    {idle:7.3}  (cores waiting, or nested parallel work in shaders/3D/post: not counted per stage)\n  wall    {wall:7.3} ms/frame = {:.0} fps; mux {:.0} ms",
            unchanged,
            sp.held,
            sp.partial,
            sp.full,
            1000.0 / wall,
            sp.mux_ms
        );
    }
    Ok(Summary {
        output: out.display().to_string(),
        width: w,
        height: h,
        fps: round3(ev.fps as f64),
        frames: n as u32,
        deduped: unchanged,
        duration: round3((n as f32 / ev.fps) as f64),
        seconds: round3(secs as f64),
        render_fps: (n as f64 / secs.max(1e-6) as f64).round(),
        bytes: std::fs::metadata(out).map(|m| m.len()).unwrap_or(0),
        codec: "ak264".into(),
        profile: Some(sp),
    })
}

/// RGBA path (PNG sequences, GIF): straight-alpha frames, cropped to even dimensions.
fn render_video_rgba(ev: &Evaluator, opts: &RenderOptions, enc: &mut dyn Encoder) -> Result<Summary, String> {
    let t0 = Instant::now();
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total);
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0, total);
    let n = f1 - f0;
    let (w, h) = (((ev.width as f32 * opts.scale).round() as u32).max(2) & !1, ((ev.height as f32 * opts.scale).round() as u32).max(2) & !1);
    let batch = (rayon::current_num_threads() as u32 * 4).max(8);
    let mut start = f0;
    while start < f1 {
        let end = (start + batch).min(f1);
        let frames: Vec<Vec<u8>> = (start..end)
            .into_par_iter()
            .map(|i| {
                let (fw, _, px) = render_rgba(ev, frame_time(ev, i), opts.scale);
                let mut out = Vec::with_capacity((w * h * 4) as usize);
                for y in 0..h as usize {
                    out.extend_from_slice(&px[y * fw as usize * 4..(y * fw as usize + w as usize) * 4]);
                }
                out
            })
            .collect();
        for f in &frames {
            enc.write_rgba(f)?;
        }
        start = end;
    }
    let bytes = enc.finish()?;
    let secs = t0.elapsed().as_secs_f32();
    Ok(Summary {
        output: enc.output().to_string(),
        width: w,
        height: h,
        fps: round3(ev.fps as f64),
        frames: n,
        deduped: 0,
        duration: round3((n as f32 / ev.fps) as f64),
        seconds: round3(secs as f64),
        render_fps: (n as f64 / secs.max(1e-6) as f64).round(),
        bytes,
        codec: enc.name().to_string(),
        profile: None,
    })
}

/// The time of frame `i`, computed the same way on every path (render, still, sheet).
pub fn frame_time(ev: &Evaluator, i: u32) -> f32 {
    (i as f64 / ev.fps as f64) as f32
}

/// Render the video (or a range of it) into `enc`.
pub fn render_video(ev: &Evaluator, opts: &RenderOptions, enc: &mut dyn Encoder) -> Result<Summary, String> {
    if enc.wants_rgba() {
        return render_video_rgba(ev, opts, enc);
    }
    let t0 = Instant::now();
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total);
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0, total);
    let n = f1 - f0;
    // Fixed batch and run sizes (never the core count), so output is identical on any machine.
    let batch = 64;
    let wants_hints = enc.wants_hints();
    type Item = (Arc<YuvFrame>, bool, Option<Vec<bool>>);
    // The encoder runs on its own thread, one batch behind the renderers.
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<Item>>(1);
    let encr: &mut dyn Encoder = &mut *enc;
    let (deduped, enc_result) = std::thread::scope(|sc| {
        let writer = sc.spawn(move || -> Result<(), String> {
            for items in rx {
                for (f, repeat, hint) in items {
                    encr.write(&f, repeat, hint)?;
                }
            }
            Ok(())
        });
        let mut prev_hash = None;
        let mut last_dl: Option<DisplayList> = None;
        let mut prev_frame: Option<Arc<YuvFrame>> = None;
        let mut deduped = 0u32;
        let mut done = 0u32;
        let mut last_report = Instant::now();
        let mut start = f0;
        while start < f1 {
            let end = (start + batch).min(f1);
            // Evaluate the batch (cheap) and hash for dedupe.
            let lists: Vec<(DisplayList, u64, (Option<crate::render::post::PostSettings>, bool))> = (start..end)
                .into_par_iter()
                .map(|i| {
                    let mut dl = DisplayList::default();
                    eval_frame(ev, i, opts.scale, &mut dl);
                    let look = frame_look(ev, opts.quality, i);
                    // Blurred or grainy frames are never repeats of the previous one.
                    let unique = look.1 || look.0.as_ref().is_some_and(|p| p.grain > 0.0);
                    let h = if unique { hash_list(&dl) ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) } else { hash_list(&dl) };
                    (dl, h, look)
                })
                .collect();
            let mut same = vec![false; lists.len()];
            let mut ph = prev_hash;
            for (k, (_, h, _)) in lists.iter().enumerate() {
                same[k] = ph == Some(*h);
                ph = Some(*h);
            }
            prev_hash = ph;
            // Render changed frames; diff display lists for renderer-aware encoders.
            // Contiguous runs of 16 frames, each with its own state (a full render first,
            // then incremental), so the pixels don't depend on which thread ran what.
            let frames: Vec<(Option<YuvFrame>, Option<Vec<bool>>)> = (0..lists.len())
                .collect::<Vec<_>>()
                .par_chunks(16)
                .flat_map_iter(|ks| {
                    let mut wk = new_worker();
                    ks.iter()
                        .map(|&k| {
                            if same[k] {
                                return (None, None);
                            }
                            let (dl, _, look) = &lists[k];
                            let hint = if wants_hints && !look.1 {
                                let prev = if k == 0 { last_dl.as_ref() } else { Some(&lists[k - 1].0) };
                                prev.map(|prev| {
                                    let mut m = crate::render::MbMap::new(0, 0);
                                    crate::render::diff::diff_mb_ops(prev, dl, &mut m);
                                    (0..m.mb_width * m.mb_height).map(|i| m.get(i % m.mb_width, i / m.mb_width)).collect()
                                })
                            } else {
                                None
                            };
                            let f = {
                                let w = &mut wk;
                                ensure_renderer(ev, w);
                                let Worker { renderer, yuv, map, last, .. } = &mut *w;
                                let r = renderer.as_mut().unwrap();
                                r.set_quality(render_quality(opts.quality));
                                r.set_post(look.0.clone(), start as u64 + k as u64);
                                if look.1 {
                                    render_blurred(ev, r, start + k as u32, opts.scale, yuv);
                                } else {
                                    r.render_yuv_incremental(dl, yuv, map);
                                }
                                *last = None;
                                yuv.clone()
                            };
                            (Some(f), hint)
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let mut items = Vec::with_capacity(frames.len());
            for (f, hint) in frames {
                match f {
                    Some(f) => {
                        let a = Arc::new(f);
                        prev_frame = Some(a.clone());
                        items.push((a, false, hint));
                    }
                    None => {
                        deduped += 1;
                        if let Some(a) = prev_frame.clone() {
                            items.push((a, true, None));
                        }
                    }
                }
                done += 1;
            }
            last_dl = lists.into_iter().last().map(|x| x.0);
            if tx.send(items).is_err() {
                break; // the encoder failed; its error is reported below
            }
            if opts.progress && (last_report.elapsed().as_secs_f32() > 0.5 || end == f1) {
                last_report = Instant::now();
                let el = t0.elapsed().as_secs_f32();
                let fps = done as f32 / el.max(1e-6);
                eprint!("\rrender {done}/{n} frames  {fps:.0} fps  {el:.1}s  ETA {:.0}s   ", (n as f32 - done as f32) / fps.max(1e-6));
            }
            start = end;
        }
        drop(tx);
        (deduped, writer.join().unwrap_or_else(|_| Err("encoder thread panicked".into())))
    });
    enc_result?;
    let bytes = enc.finish()?;
    if opts.progress {
        eprintln!();
    }
    let secs = t0.elapsed().as_secs_f32();
    // The ffmpeg encoder crops to even dimensions (4:2:0).
    let (w, h) = (((ev.width as f32 * opts.scale).round() as u32).max(2) & !1, ((ev.height as f32 * opts.scale).round() as u32).max(2) & !1);
    Ok(Summary {
        output: enc.output().to_string(),
        width: w,
        height: h,
        fps: round3(ev.fps as f64),
        frames: n,
        deduped,
        duration: round3((n as f32 / ev.fps) as f64),
        seconds: round3(secs as f64),
        render_fps: (n as f64 / secs.max(1e-6) as f64).round(),
        bytes,
        codec: enc.name().to_string(),
        profile: None,
    })
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// What a render will cost, measured on a few sample frames before it starts.
#[derive(Clone, Debug)]
pub struct Estimate {
    /// Expected wall-clock seconds.
    pub secs: f64,
    /// Mean render time of a changing frame on one thread (ms).
    pub frame_ms: f64,
    /// Frames that change (held frames cost nothing).
    pub changing: u32,
    pub threads: usize,
    /// The layer that costs the most in the slowest sampled frame: (spec path, kind, share of that frame 0..1).
    pub heaviest: Option<(String, String, f64)>,
}

impl std::fmt::Display for Estimate {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "estimate: ~{:.0} s ({:.1} ms/frame × {} changing frames on {} threads)", self.secs, self.frame_ms, self.changing, self.threads)?;
        if let Some((p, k, share)) = &self.heaviest {
            write!(f, "; heaviest: {p} ({k}) ~{:.0}% of a frame", share * 100.0)?;
        }
        Ok(())
    }
}

/// Estimate a render by timing 4 sample frames (about 4 frame renders). When the
/// render looks long (`heavy_after` seconds or more), also find the heaviest layer.
pub fn estimate(ev: &Evaluator, opts: &RenderOptions, heavy_after: f64) -> Estimate {
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total);
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0, total);
    let changing = (f0..f1).filter(|&i| i == f0 || !ev.is_static((i as f32 - 1.0) / ev.fps, i as f32 / ev.fps)).count() as u32;
    let threads = rayon::current_num_threads().max(1);
    let n = f1 - f0;
    // Quick look first: one full frame in the middle. Most renders stop here (a few ms).
    let mid = f0 + n / 2;
    let t = Instant::now();
    let mut dl = ev.eval(frame_time(ev, mid));
    scale_list(&mut dl, opts.scale);
    with_renderer(ev, |r, _| {
        r.set_quality(render_quality(opts.quality));
        r.render(&dl);
    });
    let quick_ms = t.elapsed().as_secs_f64() * 1e3;
    // A full frame is an upper bound for an incremental one, and other frames render alongside.
    let quick = quick_ms * changing as f64 / threads as f64 / 1e3 + 0.3;
    if quick < heavy_after * 0.5 {
        return Estimate { secs: quick, frame_ms: quick_ms, changing, threads, heaviest: None };
    }
    // Render one sample frame per thread, concurrently, as the pipeline does: layers that
    // parallelise internally (shaders, 3D) then show their real throughput.
    let k = threads.clamp(2, 8) as u32;
    let samples: Vec<u32> = (0..k).map(|j| f0 + (n.saturating_sub(1) * (2 * j + 1)) / (2 * k)).collect();
    let t = Instant::now();
    let times: Vec<(f64, u32)> = samples
        .par_iter()
        .map(|&i| {
            // A full frame, then the next one incrementally, as the pipeline renders them;
            // the second is the typical cost of a changing frame.
            let next = (i + 1).min(f1.saturating_sub(1));
            with_worker(|w| {
                let mut dl = ev.eval(frame_time(ev, i));
                scale_list(&mut dl, opts.scale);
                ensure_renderer(ev, w);
                let Worker { renderer, yuv, map, .. } = w;
                let r = renderer.as_mut().unwrap();
                r.set_quality(render_quality(opts.quality));
                r.invalidate();
                r.render_yuv_incremental(&dl, yuv, map);
                let t = Instant::now();
                let mut dl = ev.eval(frame_time(ev, next));
                scale_list(&mut dl, opts.scale);
                ensure_renderer(ev, w);
                let Worker { renderer, yuv, map, last, .. } = w;
                renderer.as_mut().unwrap().render_yuv_incremental(&dl, yuv, map);
                *last = None;
                (t.elapsed().as_secs_f64() * 1e3, i)
            })
        })
        .collect();
    let _ = t;
    let frame_ms = times.iter().map(|x| x.0).sum::<f64>() / times.len().max(1) as f64;
    // Concurrent samples: the batch's throughput is the mean frame time over the threads
    // that ran them (layers that use all cores themselves show up as slow frames here).
    let wall = frame_ms / 1e3 * samples.len() as f64 / threads.min(samples.len()) as f64;
    let worst = times.iter().copied().max_by(|a, b| a.0.total_cmp(&b.0));
    // Throughput of the batch, scaled to the changing frames; incremental rendering makes
    // most changing frames cheaper than a full one, encode adds a little.
    let secs = wall / samples.len().max(1) as f64 * changing as f64 * 1.1 + 0.3;
    let heaviest = match worst {
        Some((ms, i)) if secs >= heavy_after && ms > 0.0 => {
            // Evaluate the slowest frame again: lazily made images (shader layers) are then
            // computed inside the layer that uses them, so their cost lands on that layer.
            // Half a frame later, so nothing cached from the timing pass is reused.
            let (mut dl, tags) = ev.eval_tagged(frame_time(ev, i) + 0.5 / ev.fps);
            scale_list(&mut dl, opts.scale);
            let mut best: Option<(f64, u32)> = None;
            // A layer's cost: its share of the render plus its eval work (shader / 3D images are
            // made while evaluating, so re-time those by forcing a fresh evaluation of the frame).
            for tag in &tags {
                let sub = DisplayList { width: dl.width, height: dl.height, background: dl.background, ops: dl.ops[tag.ops.clone()].to_vec() };
                let t = Instant::now();
                with_renderer(ev, |r, _| {
                    r.invalidate();
                    r.render(&sub);
                });
                let lm = t.elapsed().as_secs_f64() * 1e3;
                if best.is_none_or(|b| lm > b.0) {
                    best = Some((lm, tag.layer));
                }
            }
            best.map(|(lm, layer)| {
                let (p, k) = ev.layer_info(layer);
                (p.to_string(), k.to_string(), (lm / ms).min(1.0))
            })
        }
        _ => None,
    };
    Estimate { secs, frame_ms, changing, threads, heaviest }
}
