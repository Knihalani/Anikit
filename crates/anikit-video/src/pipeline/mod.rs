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
    /// Fused mode: this thread's incremental YUV frame, change map and last frame index.
    yuv: YuvFrame,
    map: crate::render::MbMap,
    last: Option<usize>,
}

thread_local! {
    static WORKER: RefCell<Worker> = RefCell::new(Worker {
        version: 0,
        renderer: None,
        dl: DisplayList::default(),
        yuv: YuvFrame::default(),
        map: crate::render::MbMap::new(0, 0),
        last: None,
    });
}

/// Run `f` with this thread's renderer, (re)creating it when assets changed.
fn with_renderer<R>(ev: &Evaluator, f: impl FnOnce(&mut Renderer, &mut DisplayList) -> R) -> R {
    WORKER.with(|w| {
        let mut w = w.borrow_mut();
        // Only new font faces need a new renderer; images come from a live source.
        let v = (ev.assets.font_count() as u64) ^ ((Arc::as_ptr(&ev.assets) as u64) << 16);
        if w.renderer.is_none() || w.version != v {
            let mut r = Renderer::new(ev.assets.snapshot_fonts());
            let a = ev.assets.clone();
            r.set_image_source(Arc::new(move |id| a.pixmap(id)));
            w.renderer = Some(r);
            w.version = v;
            w.last = None;
        }
        let Worker { renderer, dl, .. } = &mut *w;
        f(renderer.as_mut().unwrap(), dl)
    })
}

/// Per-stage CPU time (summed over threads), for `--profile`.
#[derive(Default)]
pub struct Profile {
    pub eval_ns: AtomicU64,
    pub render_ns: AtomicU64,
    pub copy_ns: AtomicU64,
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
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total) as usize;
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0 as u32, total) as usize;
    let gop = cfg.gop.max(1) as usize;
    let done = AtomicU64::new(0);
    let t0 = Instant::now();
    let last_report = std::sync::Mutex::new(Instant::now());
    let n = f1 - f0;
    let out = ak264::encode_parallel(cfg, n, |k, buf, hints| {
        let t = Instant::now();
        with_renderer(ev, |_, _| {});
        WORKER.with(|w| {
            let mut w = w.borrow_mut();
            let Worker { renderer, dl, yuv, map, last, .. } = &mut *w;
            let r = renderer.as_mut().unwrap();
            let continuing = k % gop != 0 && *last == Some(k.wrapping_sub(1));
            // Held frame: provably identical to the previous one; skip eval and render.
            let fi = (f0 + k) as f32;
            if continuing && ev.is_static((fi - 1.0) / ev.fps, fi / ev.fps) {
                prof.unchanged.fetch_add(1, Ordering::Relaxed);
                prof.held.fetch_add(1, Ordering::Relaxed);
                hints.changed = Some(vec![false; map.mb_width * map.mb_height]);
                *last = Some(k);
                return;
            }
            eval_frame(ev, (f0 + k) as u32, opts.scale, dl);
            prof.eval_ns.fetch_add(ns(t), Ordering::Relaxed);
            let t = Instant::now();
            let upd = r.render_yuv_incremental(dl, yuv, map);
            prof.render_ns.fetch_add(ns(t), Ordering::Relaxed);
            let t = Instant::now();
            use crate::render::incremental::Update;
            match upd {
                Update::Unchanged => prof.unchanged.fetch_add(1, Ordering::Relaxed),
                Update::Partial { .. } => prof.partial.fetch_add(1, Ordering::Relaxed),
                Update::Full => prof.full.fetch_add(1, Ordering::Relaxed),
            };
            if continuing {
                // `map` is exact versus the previous frame, which `buf` still holds.
                let n_mb = map.mb_width * map.mb_height;
                match upd {
                    Update::Unchanged => hints.changed = Some(vec![false; n_mb]),
                    _ => {
                        copy_yuv(yuv, buf, Some(map));
                        hints.changed = Some((0..n_mb).map(|i| map.get(i % map.mb_width, i / map.mb_width)).collect());
                    }
                }
            } else {
                copy_yuv(yuv, buf, None);
                hints.changed = None;
            }
            *last = Some(k);
            prof.copy_ns.fetch_add(ns(t), Ordering::Relaxed);
        });
        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
        if opts.progress {
            if let Ok(mut lr) = last_report.try_lock() {
                if lr.elapsed().as_secs_f32() > 0.5 {
                    *lr = Instant::now();
                    let el = t0.elapsed().as_secs_f32();
                    eprint!("\rrender {d}/{n} frames  {:.0} fps  {:.1}s", d as f32 / el.max(1e-6), el);
                }
            }
        }
    });
    if opts.progress {
        eprintln!();
    }
    out
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

/// Mix the video's audio for `[t0, t0 + dur)` into a WAV at `wav`; `None` if the video is silent.
pub fn mix_audio(ev: &Evaluator, t0: f32, dur: f32, wav: &std::path::Path) -> Result<Option<std::path::PathBuf>, String> {
    use crate::audio::{self, Source};
    if ev.audio.is_empty() {
        return Ok(None);
    }
    let mut tracks = Vec::new();
    for t in &ev.audio {
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
    let pcm = audio::mix(&tracks, dur as f64, audio::DEFAULT_RATE, None)?;
    audio::write_wav(wav, &pcm, audio::WavFormat::Pcm16)?;
    Ok(Some(wav.to_path_buf()))
}

/// Render to an MP4 with ak264 in fused mode (see [`render_fused`]); muxes
/// without ffmpeg when there is no audio.
pub fn render_ak264(ev: &Evaluator, opts: &RenderOptions, out: &std::path::Path, audio: Option<&std::path::Path>) -> Result<Summary, String> {
    let t0 = Instant::now();
    let w = (((ev.width as f32 * opts.scale).round() as u32).max(2)) & !1;
    let h = (((ev.height as f32 * opts.scale).round() as u32).max(2)) & !1;
    let total = ev.frames();
    let f0 = opts.from.map(|t| (t * ev.fps).round() as u32).unwrap_or(0).min(total);
    let f1 = opts.to.map(|t| (t * ev.fps).round() as u32).unwrap_or(total).clamp(f0, total);
    let n = (f1 - f0) as usize;
    let threads = rayon::current_num_threads().max(1);
    // Enough chunks to keep every core busy, long enough to compress well.
    let gop = (n / (threads * 3)).clamp(30, 300) as u32;
    let qp = match opts.quality {
        Quality::Draft => 28,
        Quality::Good => 23,
        Quality::Best => 19,
    };
    let cfg = ak264::EncoderConfig {
        width: w,
        height: h,
        fps: ev.fps as f64,
        qp,
        gop,
        deblock: false,
        threads: 1,
        full_range: false,
        cabac: opts.quality != Quality::Draft,
        subpel: opts.quality == Quality::Best,
    };
    let prof = Profile::default();
    let annexb = render_fused(ev, opts, &cfg, &prof);
    let t_enc = t0.elapsed();
    let tm = Instant::now();
    match audio {
        None => std::fs::write(out, ak264::mp4::mux(&annexb, w, h, ev.fps as f64)).map_err(|e| format!("can't write {}: {e}", out.display()))?,
        Some(a) => {
            use std::io::Write;
            let mut c = std::process::Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "h264", "-r", &format!("{}", ev.fps), "-i", "-", "-i"])
                .arg(a)
                .args(["-map", "0:v", "-map", "1:a", "-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"])
                .arg(out)
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("can't start ffmpeg for the audio mux: {e}"))?;
            c.stdin.take().unwrap().write_all(&annexb).map_err(|e| e.to_string())?;
            let o = c.wait_with_output().map_err(|e| e.to_string())?;
            if !o.status.success() {
                return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
            }
        }
    }
    let t_mux = tm.elapsed();
    let secs = t0.elapsed().as_secs_f32();
    let unchanged = prof.unchanged.load(Ordering::Relaxed) as u32;
    if opts.profile {
        let per = |x: &AtomicU64| x.load(Ordering::Relaxed) as f64 / 1e6 / n.max(1) as f64;
        let (e, r, c) = (per(&prof.eval_ns), per(&prof.render_ns), per(&prof.copy_ns));
        let wall = t_enc.as_secs_f64() * 1e3 / n.max(1) as f64;
        let enc = (wall * threads as f64 - e - r - c).max(0.0);
        eprintln!(
            "profile ({n} frames, {threads} threads, gop {gop}; CPU ms/frame summed over threads)\n  eval    {e:7.3}\n  render  {r:7.3}  (incremental+YUV: {} unchanged ({} held, not even evaluated), {} partial, {} full)\n  copy    {c:7.3}\n  encode  {enc:7.3}  (remainder)\n  wall    {wall:7.3} ms/frame = {:.0} fps; mux {:.0} ms",
            unchanged,
            prof.held.load(Ordering::Relaxed),
            prof.partial.load(Ordering::Relaxed),
            prof.full.load(Ordering::Relaxed),
            1000.0 / wall,
            t_mux.as_secs_f64() * 1e3
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
    })
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
            // Render changed frames; diff display lists for renderer-aware encoders.
            let frames: Vec<(Option<YuvFrame>, Option<Vec<bool>>)> = (0..lists.len())
                .into_par_iter()
                .map(|k| {
                    if same[k] {
                        return (None, None);
                    }
                    let dl = &lists[k].0;
                    let hint = if wants_hints {
                        let prev = if k == 0 { last_dl.as_ref() } else { Some(&lists[k - 1].0) };
                        prev.map(|prev| {
                            let mut m = crate::render::MbMap::new(0, 0);
                            crate::render::diff::diff_mb_ops(prev, dl, &mut m);
                            (0..m.mb_width * m.mb_height).map(|i| m.get(i % m.mb_width, i / m.mb_width)).collect()
                        })
                    } else {
                        None
                    };
                    let f = with_renderer(ev, |r, _| {
                        let mut f = YuvFrame::new(dl.width, dl.height);
                        r.render_yuv(dl, &mut f);
                        f
                    });
                    (Some(f), hint)
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
                eprint!("\rrender {done}/{n} frames  {:.0} fps  {:.1}s", done as f32 / el.max(1e-6), el);
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
    let (w, h) = (((ev.width as f32 * opts.scale).round() as u32).max(2), ((ev.height as f32 * opts.scale).round() as u32).max(2));
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
    })
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}
