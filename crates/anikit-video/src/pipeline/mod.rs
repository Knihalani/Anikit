//! Frames → file: parallel rendering, identical-frame dedupe, YUV, encoders, audio mux.
//!
//! Frames are evaluated and rendered in ordered batches on the rayon pool,
//! one [`Renderer`] per worker thread (re-created only when shared assets
//! change). A frame whose display list hashes the same as the previous
//! frame's isn't rendered again. Finished YUV frames go to an [`Encoder`] in
//! order.

pub mod encode;

use crate::dl::{self, DisplayList, Op};
use crate::eval::Evaluator;
use crate::render::{Renderer, YuvFrame};
use rayon::prelude::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
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
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { codec: Codec::H264, quality: Quality::Good, from: None, to: None, scale: 1.0, progress: false, audio: true }
    }
}

/// Machine-readable summary of a render.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Summary {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub frames: u32,
    /// Frames skipped because they were identical to the previous one.
    pub deduped: u32,
    pub duration: f32,
    /// Wall-clock seconds.
    pub seconds: f32,
    /// Frames per second achieved, end to end.
    pub render_fps: f32,
    pub bytes: u64,
    pub codec: String,
}

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
    renderer: Option<Renderer>,
    dl: DisplayList,
}

thread_local! {
    static WORKER: RefCell<Worker> = RefCell::new(Worker { version: 0, renderer: None, dl: DisplayList::default() });
}

/// Run `f` with this thread's renderer, (re)creating it when assets changed.
fn with_renderer<R>(ev: &Evaluator, f: impl FnOnce(&mut Renderer, &mut DisplayList) -> R) -> R {
    WORKER.with(|w| {
        let mut w = w.borrow_mut();
        let v = ev.assets.version() ^ (Arc::as_ptr(&ev.assets) as u64);
        if w.renderer.is_none() || w.version != v {
            w.renderer = Some(Renderer::new(ev.assets.snapshot()));
            w.version = v;
        }
        let Worker { renderer, dl, .. } = &mut *w;
        f(renderer.as_mut().unwrap(), dl)
    })
}

/// Evaluate frame `i` into `out` at `scale`.
pub fn eval_frame(ev: &Evaluator, i: u32, scale: f32, out: &mut DisplayList) {
    ev.eval_into(i as f32 / ev.fps, out);
    scale_list(out, scale);
}

/// Render one frame to straight-alpha RGBA8 (for stills).
pub fn render_rgba(ev: &Evaluator, t: f32, scale: f32) -> (u32, u32, Vec<u8>) {
    let mut dl = ev.eval(t);
    scale_list(&mut dl, scale);
    // Evaluating may have created images (components); render after.
    with_renderer(ev, |r, _| {
        let pm = r.render(&dl);
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
    })
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

/// Render the video (or a range of it) into `enc`.
pub fn render_video(ev: &Evaluator, opts: &RenderOptions, enc: &mut dyn Encoder) -> Result<Summary, String> {
    let t0 = Instant::now();
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total);
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0, total);
    let n = f1 - f0;
    let threads = rayon::current_num_threads().max(1) as u32;
    let batch = (threads * 4).max(8);
    let mut prev_hash = None;
    let mut prev_frame: Option<Arc<YuvFrame>> = None;
    let mut deduped = 0;
    let mut done = 0u32;
    let mut last_report = Instant::now();
    let mut start = f0;
    while start < f1 {
        let end = (start + batch).min(f1);
        // Evaluate the batch (cheap) and hash for dedupe.
        let lists: Vec<(DisplayList, u64)> = (start..end)
            .into_par_iter()
            .map(|i| {
                let mut dl = DisplayList::default();
                eval_frame(ev, i, opts.scale, &mut dl);
                let h = hash_list(&dl);
                (dl, h)
            })
            .collect();
        let mut same = vec![false; lists.len()];
        let mut ph = prev_hash;
        for (k, (_, h)) in lists.iter().enumerate() {
            same[k] = ph == Some(*h);
            ph = Some(*h);
        }
        prev_hash = ph;
        let frames: Vec<Option<YuvFrame>> = lists
            .par_iter()
            .zip(same.par_iter())
            .map(|((dl, _), &s)| {
                if s {
                    return None;
                }
                Some(with_renderer(ev, |r, _| {
                    let mut f = YuvFrame::new(dl.width, dl.height);
                    r.render_yuv(dl, &mut f);
                    f
                }))
            })
            .collect();
        for f in frames {
            let f = match f {
                Some(f) => {
                    let a = Arc::new(f);
                    prev_frame = Some(a.clone());
                    enc.write(&a, false)?;
                    a
                }
                None => {
                    deduped += 1;
                    let a = prev_frame.clone().ok_or("dedupe without a previous frame")?;
                    enc.write(&a, true)?;
                    a
                }
            };
            drop(f);
            done += 1;
        }
        if opts.progress && (last_report.elapsed().as_secs_f32() > 0.5 || end == f1) {
            last_report = Instant::now();
            let el = t0.elapsed().as_secs_f32();
            eprint!("\rrender {done}/{n} frames  {:.0} fps  {:.1}s", done as f32 / el.max(1e-6), el);
        }
        start = end;
    }
    let bytes = enc.finish()?;
    if opts.progress {
        eprintln!();
    }
    let secs = t0.elapsed().as_secs_f32();
    let (w, h) = (((ev.width as f32 * opts.scale).round() as u32).max(2), ((ev.height as f32 * opts.scale).round() as u32).max(2));
    Ok(Summary {
        output: enc.output().to_string(),
        width: w,
        height: h,
        fps: ev.fps,
        frames: n,
        deduped,
        duration: n as f32 / ev.fps,
        seconds: secs,
        render_fps: n as f32 / secs.max(1e-6),
        bytes,
        codec: enc.name().to_string(),
    })
}
