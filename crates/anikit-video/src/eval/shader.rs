//! Shader layers and shader transitions (`anikit-shader` in the engine).
//!
//! Compile time: the GLSL/WGSL is JIT-compiled once per spec ([`ShaderC`]),
//! uniforms get animatable slots after `prop::APERTURE`, channel layers are
//! compiled like a group's children.
//!
//! Per frame: evaluation only records a [`Job`] (shader, uniform values, time,
//! the display lists its channels need) and emits an `Op::Image` with a job
//! id. Pixels are made when a renderer asks for that image
//! ([`SharedAssets::pixmap`]), so occluded or deduplicated frames cost
//! nothing and nothing large is kept per frame. Generators are keyed by
//! content, so a static shader renders once.

use super::{prop, Cx, Evaluator, Node, SharedAssets};
use crate::dl::{self, Affine, DisplayList, ImageId, Op, Rgba};
use crate::render::Renderer;
use anikit_shader::{Filter, Frame, Shader, Texture, Wrap};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use vello_cpu::Pixmap;

/// First image id used for shader output.
pub(crate) const SHADER_BASE: u32 = 0x7000_0000;
const KEYED_SPAN: u32 = 0x0800_0000;
/// Animatable float slots for uniforms (per shader layer).
pub(crate) const UNIFORM_SLOTS: usize = 12;
/// Largest side a shader renders at.
const MAX_SIDE: f32 = 8192.0;

/// Where a channel's texture comes from.
pub(crate) enum ChannelC {
    None,
    /// Everything drawn before this layer, composited (filters).
    Below,
    /// A tiling 256×256 RGBA noise texture.
    Noise,
    /// A sub-composition (images, videos, text…) rendered at the layer's size.
    Layers(Vec<Node>),
    /// A named composition (`"#name"`), through the shared `CompTextures` cache.
    Comp { ev: Arc<Evaluator>, time: super::CompTime },
    /// Buffer pass `0..4` (`"A"`…`"D"`) of this layer, at this frame.
    Buffer(usize),
}

/// What a buffer pass reads per channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum BufChan {
    None,
    Noise,
    /// Buffer `i`: this frame's when it runs earlier, else the previous frame's.
    Buffer(usize),
}

pub(crate) struct BufPass {
    /// Index 0..4 for A..D.
    pub slot: usize,
    pub shader: Arc<Shader>,
    pub chans: Vec<(BufChan, ChanOpts)>,
}

/// Buffer passes of one shader layer, with their feedback state.
pub(crate) struct Passes {
    /// In render order (A, B, C, D as present).
    pub bufs: Vec<BufPass>,
    /// Some pass reads itself or a later pass: frame n depends on frame n − 1.
    pub stateful: bool,
    /// Some pass reads `iTime`/`iFrame`/….
    pub dynamic: bool,
    /// Uniform values the passes see: the spec's, not animated, so a frame is
    /// the same however it was reached.
    pub uniforms: Vec<(String, Vec<f32>)>,
    pub audio: Option<Arc<super::audio_react::AudioFeatures>>,
    /// Recent frames.
    history: Mutex<VecDeque<Kept>>,
}

/// Frames of buffer state kept (each is up to 4 float textures at layer size).
const HISTORY: usize = 2;

type Bufs = Arc<[Option<Arc<Texture>>; 4]>;
/// A kept frame of buffer state: (frame, size, textures by slot).
type Kept = (i64, (u32, u32), Bufs);

impl Passes {
    pub fn new(bufs: Vec<BufPass>, uniforms: Vec<(String, Vec<f32>)>, audio: Option<Arc<super::audio_react::AudioFeatures>>) -> Passes {
        let order: Vec<usize> = bufs.iter().map(|b| b.slot).collect();
        let stateful = bufs.iter().enumerate().any(|(i, b)| {
            b.chans.iter().any(|(c, _)| match c {
                BufChan::Buffer(j) => order.iter().position(|s| s == j).is_none_or(|pj| pj >= i),
                _ => false,
            })
        });
        Passes { bufs, stateful, dynamic: false, uniforms, audio, history: Mutex::new(VecDeque::new()) }
    }

    /// Buffer textures at layer frame `n` (frame 0 starts from zero buffers).
    /// `gt0` is the global time of layer frame 0 (for `iAudio`/`iBeat`).
    fn at(&self, n: i64, size: (u32, u32), fps: f32, gt0: f32) -> Bufs {
        let n = n.max(0);
        if !self.stateful {
            return self.step(n, size, fps, gt0, None);
        }
        // Resume from the latest kept frame at or before n; no lock is held while
        // rendering (the renders use rayon, and a nested request must not block).
        let (mut k, mut prev) = {
            let h = self.history.lock().unwrap();
            if let Some((_, _, b)) = h.iter().find(|(f, s, _)| *f == n && *s == size) {
                return b.clone();
            }
            match h.iter().filter(|(f, s, _)| *f < n && *s == size).max_by_key(|(f, _, _)| *f) {
                Some((f, _, b)) => (*f + 1, Some(b.clone())),
                None => (0, None),
            }
        };
        loop {
            let cur = self.step(k, size, fps, gt0, prev.as_ref());
            if k >= n.saturating_sub(HISTORY as i64 - 1) {
                let mut h = self.history.lock().unwrap();
                if !h.iter().any(|(f, s, _)| *f == k && *s == size) {
                    h.push_back((k, size, cur.clone()));
                    while h.len() > HISTORY {
                        h.pop_front();
                    }
                }
            }
            if k == n {
                return cur;
            }
            prev = Some(cur);
            k += 1;
        }
    }

    fn step(&self, k: i64, (w, h): (u32, u32), fps: f32, gt0: f32, prev: Option<&Bufs>) -> Bufs {
        let zero = OnceLock::new();
        let zero = || zero.get_or_init(|| Texture::from_f32_exact(w, h, Vec::new()));
        let mut cur: [Option<Arc<Texture>>; 4] = Default::default();
        let time = k as f32 / fps.max(1e-3);
        let (audio, beat) = self.audio.as_ref().map_or(([0.0; 4], [0.0; 4]), |a| a.at(gt0 + time));
        let unis: Vec<(&str, &[f32])> = self.uniforms.iter().map(|(n, v)| (n.as_str(), v.as_slice())).collect();
        for b in &self.bufs {
            // Bind: earlier passes this frame, the others from the previous frame
            // (zero on frame 0). Channel options apply per binding, as views.
            let views: Vec<Option<Texture>> = b
                .chans
                .iter()
                .take(4)
                .map(|(c, o)| match c {
                    BufChan::Buffer(j) => {
                        let t = cur[*j].clone().or_else(|| prev.and_then(|p| p[*j].clone()));
                        Some(match &t {
                            Some(t) => t.view(o.wrap, o.filter),
                            None => zero().view(o.wrap, o.filter),
                        })
                    }
                    _ => None,
                })
                .collect();
            let mut chans: [Option<&Texture>; 4] = [None; 4];
            for (i, (c, _)) in b.chans.iter().enumerate().take(4) {
                chans[i] = match c {
                    BufChan::None => None,
                    BufChan::Noise => Some(noise_texture()),
                    BufChan::Buffer(_) => views[i].as_ref(),
                };
            }
            let mut f = Frame::new(w, h, time);
            f.frame = k as i32;
            f.fps = fps;
            f.uniforms = &unis;
            f.channels = chans;
            f.audio = audio;
            f.beat = beat;
            f.alpha = true;
            let px = b.shader.render_f32(&f);
            cur[b.slot] = Some(Arc::new(Texture::from_f32_exact(w, h, px)));
        }
        // Passes that didn't run (none) keep nothing; the rest are this frame's.
        Arc::new(cur)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ChanOpts {
    pub wrap: Wrap,
    pub filter: Filter,
    pub mipmaps: bool,
}

/// A compiled shader layer (or shader transition).
pub(crate) struct ShaderC {
    pub shader: Arc<Shader>,
    /// (name, first slot after `prop::U0`, components).
    pub uniforms: Vec<(String, usize, usize)>,
    /// Static values (transitions have no animation slots).
    pub fixed: Vec<(String, Vec<f32>)>,
    pub channels: Vec<(ChannelC, ChanOpts)>,
    pub filter: bool,
    pub supersample: bool,
    pub alpha: bool,
    /// Reads `iTime`/`iFrame`/`iDate`: changes every frame.
    pub dynamic: bool,
    /// Distinguishes layers in content keys.
    pub id: u64,
    /// Spec path and shader name, for cost reports (`still` prints them).
    pub label: Arc<str>,
    /// Music features when the source reads `iAudio`/`iBeat`.
    pub audio: Option<Arc<super::audio_react::AudioFeatures>>,
    /// Buffer passes (`"buffers"`).
    pub passes: Option<Arc<Passes>>,
}

/// Shared handle for transitions (`TransC` is `Clone + PartialEq + Debug`).
#[derive(Clone)]
pub(crate) struct ShaderRef(pub Arc<ShaderC>);

impl PartialEq for ShaderRef {
    fn eq(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }
}

impl std::fmt::Debug for ShaderRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShaderRef({})", self.0.id)
    }
}

/// `path (name)` for cost reports: a built-in's name or a file name; inline source is just the path.
pub(crate) fn label(path: &str, src: Option<&str>) -> Arc<str> {
    match src.map(str::trim) {
        Some(s) if !s.contains(['(', '{', ';', '\n']) => format!("{path} ({s})").into(),
        _ => path.into(),
    }
}

/// True when the source animates by itself.
pub(crate) fn uses_time(src: &str) -> bool {
    ["iTime", "iFrame", "iDate", "iTimeDelta", "iAudio", "iBeat"].iter().any(|w| src.contains(w))
}

/// Built-in name → source, or the source itself.
pub(crate) fn resolve_source(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.contains('(') || t.contains('{') || t.contains(';') {
        return Some(s);
    }
    anikit_shader::library::get(t.trim_start_matches('@')).map(|e| e.src)
}

thread_local! {
    /// Background colour of the frame being evaluated (filters composite onto it).
    static FRAME_BG: Cell<Rgba> = const { Cell::new(Rgba([0.0, 0.0, 0.0, 1.0])) };
}

thread_local! {
    /// Output scale of the frame being evaluated (`still --scale`): shaders render
    /// at the size they will be shown, not at full size and then shrunk.
    static PREVIEW_SCALE: Cell<f32> = const { Cell::new(1.0) };
}

/// Set the output scale for evaluations on this thread (1 = full size).
pub fn set_preview_scale(s: f32) {
    PREVIEW_SCALE.with(|c| c.set(if s.is_finite() && s > 0.0 { s.min(1.0) } else { 1.0 }));
}

pub(crate) fn preview_scale() -> f32 {
    PREVIEW_SCALE.with(|c| c.get())
}

/// A frame-sized display list, shrunk to the preview scale.
fn frame_list(w: u32, h: u32, background: Rgba, ops: Vec<Op>) -> DisplayList {
    let mut dl = DisplayList { width: w, height: h, background, ops };
    let s = preview_scale();
    if s != 1.0 {
        crate::pipeline::scale_list(&mut dl, s);
    }
    dl
}

pub(crate) fn set_frame_bg(c: Rgba) {
    FRAME_BG.with(|b| b.set(c));
}

// ---------------------------------------------------------------- jobs

pub(crate) enum ChanData {
    None,
    Noise,
    List(DisplayList),
    /// Named composition at its own time (weak: the root evaluator owns it).
    Comp(Weak<Evaluator>, f32),
    /// Buffer pass by slot (A = 0).
    Buffer(usize),
}

pub(crate) struct Job {
    shader: Arc<Shader>,
    w: u32,
    h: u32,
    time: f32,
    frame: i32,
    fps: f32,
    progress: f32,
    uniforms: Vec<(String, Vec<f32>)>,
    channels: Vec<(ChanData, ChanOpts)>,
    supersample: bool,
    alpha: bool,
    label: Arc<str>,
    /// `iAudio`, `iBeat`.
    audio: [f32; 4],
    beat: [f32; 4],
    /// Buffer passes and the global time of layer frame 0.
    passes: Option<(Arc<Passes>, f32)>,
}

/// Measured cost of one shader layer (or transition) over the renders so far.
#[derive(Clone, Debug)]
pub struct ShaderStat {
    /// Spec path and shader name, e.g. `scenes[0].layers[1] (sdf-scene)`.
    pub label: String,
    /// Mean milliseconds per render (the shader alone, not its channels).
    pub ms: f32,
    /// Renders measured.
    pub renders: u32,
    /// Size rendered at (scaled for stills).
    pub width: u32,
    pub height: u32,
    pub supersample: bool,
}

struct Entry {
    key: u64,
    job: Job,
    result: Mutex<Option<Arc<Pixmap>>>,
}

#[derive(Default)]
struct Inner {
    next: u32,
    map: HashMap<u32, Arc<Entry>>,
    order: VecDeque<u32>,
    with_result: VecDeque<u32>,
}

/// Per label: (total ms, renders, w, h, supersample).
type Tally = (f64, u32, u32, u32, bool);

/// Pending and recent shader renders, looked up by image id.
#[derive(Default)]
pub struct ShaderJobs {
    inner: Mutex<Inner>,
    assets: OnceLock<Weak<SharedAssets>>,
    stats: Mutex<HashMap<Arc<str>, Tally>>,
}

/// Jobs kept (an evaluation batch is a few dozen frames ahead of rendering).
const MAX_JOBS: usize = 256;
/// Rendered results kept (each is a full layer of pixels).
const MAX_RESULTS: usize = 8;

impl ShaderJobs {
    pub(crate) fn bind(&self, assets: &Arc<SharedAssets>) {
        let _ = self.assets.set(Arc::downgrade(assets));
    }

    /// Register a render; `key` (content hash) dedupes identical renders.
    fn submit(&self, job: Job, key: Option<u64>) -> ImageId {
        let mut g = self.inner.lock().unwrap();
        let id = match key {
            Some(k) => {
                let id = SHADER_BASE + (k as u32 % KEYED_SPAN);
                if g.map.get(&id).is_some_and(|e| e.key == k) {
                    return ImageId(id);
                }
                id
            }
            None => {
                g.next = (g.next + 1) % KEYED_SPAN;
                SHADER_BASE + KEYED_SPAN + g.next
            }
        };
        g.map.insert(id, Arc::new(Entry { key: key.unwrap_or(0), job, result: Mutex::new(None) }));
        g.order.push_back(id);
        while g.order.len() > MAX_JOBS {
            if let Some(old) = g.order.pop_front() {
                if !g.order.contains(&old) {
                    g.map.remove(&old);
                }
            }
        }
        ImageId(id)
    }

    /// Pixels for a shader image id (rendering it on first use).
    pub(crate) fn get(&self, id: ImageId) -> Option<Arc<Pixmap>> {
        let e = self.inner.lock().unwrap().map.get(&id.0).cloned()?;
        if let Some(p) = e.result.lock().unwrap().clone() {
            return Some(p);
        }
        // No lock is held while rendering: the shader and nested renders use rayon.
        let assets = self.assets.get().and_then(|w| w.upgrade());
        let (pm, ms) = run(&e.job, assets.as_ref());
        let pm = Arc::new(pm);
        {
            let j = &e.job;
            let mut st = self.stats.lock().unwrap();
            let s = st.entry(j.label.clone()).or_insert((0.0, 0, j.w, j.h, j.supersample));
            s.0 += ms;
            s.1 += 1;
            (s.2, s.3) = (j.w, j.h);
        }
        *e.result.lock().unwrap() = Some(pm.clone());
        let mut g = self.inner.lock().unwrap();
        g.with_result.push_back(id.0);
        while g.with_result.len() > MAX_RESULTS {
            if let Some(old) = g.with_result.pop_front() {
                if old != id.0 {
                    if let Some(oe) = g.map.get(&old) {
                        *oe.result.lock().unwrap() = None;
                    }
                }
            }
        }
        Some(pm)
    }

    /// Cost of every shader rendered so far, most expensive first.
    pub fn stats(&self) -> Vec<ShaderStat> {
        let mut v: Vec<ShaderStat> = self
            .stats
            .lock()
            .unwrap()
            .iter()
            .map(|(l, &(ms, n, w, h, ss))| ShaderStat { label: l.to_string(), ms: (ms / n.max(1) as f64) as f32, renders: n, width: w, height: h, supersample: ss })
            .collect();
        v.sort_by(|a, b| b.ms.total_cmp(&a.ms));
        v
    }

    pub(crate) fn owns(id: ImageId) -> bool {
        id.0 >= SHADER_BASE && id.0 < SHADER_BASE + 2 * KEYED_SPAN
    }
}

thread_local! {
    static NESTED: RefCell<Option<(u64, Renderer)>> = const { RefCell::new(None) };
}

/// Render a display list (a channel) to a texture.
fn list_texture(dl: &DisplayList, assets: Option<&Arc<SharedAssets>>, o: ChanOpts) -> Texture {
    let draw = |r: &mut Renderer| {
        let pm = r.render(dl);
        Texture::from_rgba8(pm.width() as u32, pm.height() as u32, pm.data_as_u8_slice(), true)
    };
    let fresh = || {
        let mut r = match assets {
            Some(a) => {
                let mut r = Renderer::new(a.snapshot_fonts());
                let a2 = a.clone();
                r.set_image_source(Arc::new(move |id| a2.pixmap(id)));
                r
            }
            None => Renderer::new(Arc::new(crate::render::Assets::default())),
        };
        let t = draw(&mut r);
        (t, r)
    };
    let version = assets.map(|a| (a.font_count() as u64) ^ ((Arc::as_ptr(a) as u64) << 16)).unwrap_or(0);
    let tex = NESTED.with(|c| match c.try_borrow_mut() {
        Ok(mut slot) => match slot.as_mut() {
            Some((v, r)) if *v == version => draw(r),
            _ => {
                let (t, r) = fresh();
                *slot = Some((version, r));
                t
            }
        },
        // Nested inside another channel render on this thread: a temporary renderer.
        Err(_) => fresh().0,
    });
    finish_texture(tex, o)
}

fn finish_texture(t: Texture, o: ChanOpts) -> Texture {
    let t = t.with_wrap(o.wrap).with_filter(o.filter);
    if o.mipmaps {
        t.with_mipmaps()
    } else {
        t
    }
}

fn noise_texture() -> &'static Texture {
    static N: OnceLock<Texture> = OnceLock::new();
    N.get_or_init(|| {
        let mut s: u32 = 0x9e37_79b9;
        let px = (0..256 * 256)
            .map(|_| {
                std::array::from_fn(|_| {
                    s ^= s << 13;
                    s ^= s >> 17;
                    s ^= s << 5;
                    (s >> 8) as f32 / 16_777_216.0
                })
            })
            .collect();
        Texture::from_f32(256, 256, px).with_wrap(Wrap::Repeat).with_mipmaps()
    })
}

fn run(job: &Job, assets: Option<&Arc<SharedAssets>>) -> (Pixmap, f64) {
    let bufs = job.passes.as_ref().map(|(p, gt0)| p.at(job.frame as i64, (job.w, job.h), job.fps, *gt0));
    let mut owned: Vec<Option<Texture>> = Vec::new();
    for (c, o) in &job.channels {
        owned.push(match c {
            ChanData::Buffer(j) => bufs.as_ref().and_then(|b| b[*j].as_deref()).map(|t| t.view(o.wrap, o.filter)),
            ChanData::List(dl) => Some(list_texture(dl, assets, *o)),
            ChanData::Comp(ev, t) => match (ev.upgrade(), assets) {
                (Some(ev), Some(a)) => {
                    let pm = a.textures().get(&ev, *t, 0);
                    Some(finish_texture(Texture::from_rgba8(pm.width() as u32, pm.height() as u32, pm.data_as_u8_slice(), true), *o))
                }
                _ => None,
            },
            _ => None,
        });
    }
    let mut chans: [Option<&Texture>; 4] = [None; 4];
    for (i, (c, _)) in job.channels.iter().enumerate().take(4) {
        chans[i] = match c {
            ChanData::None => None,
            ChanData::Noise => Some(noise_texture()),
            ChanData::List(_) | ChanData::Comp(..) | ChanData::Buffer(_) => owned[i].as_ref(),
        };
    }
    let unis: Vec<(&str, &[f32])> = job.uniforms.iter().map(|(n, v)| (n.as_str(), v.as_slice())).collect();
    let mut f = Frame::new(job.w, job.h, job.time);
    f.frame = job.frame;
    f.fps = job.fps;
    f.progress = job.progress;
    f.uniforms = &unis;
    f.audio = job.audio;
    f.beat = job.beat;
    f.channels = chans;
    f.supersample = job.supersample;
    f.alpha = job.alpha;
    // Smooth shader gradients would band in 8 bits; exact values are untouched.
    f.dither = true;
    let mut pm = Pixmap::new(job.w.clamp(1, 65535) as u16, job.h.clamp(1, 65535) as u16);
    let t0 = std::time::Instant::now();
    let opaque = job.shader.render(&f, pm.data_as_u8_slice_mut());
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    pm.set_may_have_transparency(!opaque);
    (pm, ms)
}

// ---------------------------------------------------------------- emission

fn hash_f32s<H: Hasher>(h: &mut H, v: &[f32]) {
    for x in v {
        x.to_bits().hash(h);
    }
}

impl Evaluator {
    /// Measured cost of every shader layer and transition rendered so far.
    pub fn shader_stats(&self) -> Vec<ShaderStat> {
        self.assets.shaders.stats()
    }

    #[allow(clippy::too_many_arguments)]
    fn job_for(&self, sc: &ShaderC, cx: &Cx, v: &[f32; prop::N], size: (u32, u32), time: f32, progress: f32, chans: Vec<(ChanData, ChanOpts)>) -> ImageId {
        let mut uniforms: Vec<(String, Vec<f32>)> = sc.fixed.clone();
        for (name, off, n) in &sc.uniforms {
            let base = prop::U0 + off;
            uniforms.push((name.clone(), v[base..base + n].to_vec()));
        }
        // Music at the global time (the audio track is on the global timeline).
        let (audio, beat) = sc.audio.as_ref().map_or(([0.0; 4], [0.0; 4]), |a| a.at(cx.gt));
        // Content key: generators without composited inputs render once per distinct input.
        let key = if chans.iter().all(|(c, _)| !matches!(c, ChanData::List(_))) {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for (c, _) in &chans {
                if let ChanData::Comp(ev, t) = c {
                    if let Some(ev) = ev.upgrade() {
                        (Arc::as_ptr(&ev) as usize, crate::pipeline::textures::CompTextures::frame_index(&ev, *t)).hash(&mut h);
                    }
                }
            }
            sc.id.hash(&mut h);
            size.hash(&mut h);
            for (_, u) in &uniforms {
                hash_f32s(&mut h, u);
            }
            if sc.dynamic {
                hash_f32s(&mut h, &[time, (cx.gt * self.fps).round()]);
            }
            hash_f32s(&mut h, &[progress]);
            hash_f32s(&mut h, &audio);
            hash_f32s(&mut h, &beat);
            Some(h.finish())
        } else {
            None
        };
        let job = Job {
            shader: sc.shader.clone(),
            w: size.0,
            h: size.1,
            time,
            frame: (time * self.fps).round() as i32,
            fps: self.fps,
            progress,
            uniforms,
            channels: chans,
            supersample: sc.supersample || crate::components::reference(),
            alpha: sc.alpha,
            label: sc.label.clone(),
            audio,
            beat,
            passes: sc.passes.clone().map(|p| (p, cx.gt - time)),
        };
        self.assets.shaders.submit(job, key)
    }

    /// Emit a shader layer. `ops_below` is the frame drawn so far (filters read it).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_shader(&self, sc: &ShaderC, cx: &Cx, v: &[f32; prop::N], xf: &Affine, w: f32, h: f32, alpha: f32, lt: f32, out: &mut Vec<Op>) {
        let (fw, fh) = (self.width as f32, self.height as f32);
        // Render at device resolution (times the preview scale).
        let ps = preview_scale();
        let (sx, sy) = ((xf[0] * xf[0] + xf[1] * xf[1]).sqrt() * ps, (xf[2] * xf[2] + xf[3] * xf[3]).sqrt() * ps);
        let (dw, dh) = if sc.filter { ((fw * ps).round().max(1.0), (fh * ps).round().max(1.0)) } else { ((w * sx).round().clamp(1.0, MAX_SIDE), (h * sy).round().clamp(1.0, MAX_SIDE)) };
        if w < 0.5 || h < 0.5 {
            return;
        }
        let mut chans = Vec::with_capacity(sc.channels.len());
        for (c, o) in &sc.channels {
            chans.push((
                match c {
                    ChannelC::None => ChanData::None,
                    ChannelC::Noise => ChanData::Noise,
                    ChannelC::Below => ChanData::List(frame_list(self.width, self.height, FRAME_BG.with(|b| b.get()), out.clone())),
                    ChannelC::Layers(nodes) => {
                        let mut ops = Vec::new();
                        let ccx = Cx { ev: cx.ev, scene_t: cx.scene_t, gt: cx.gt, dof: 0.0 };
                        let k = [dw / w.max(1e-3), 0.0, 0.0, dh / h.max(1e-3), 0.0, 0.0];
                        for n in nodes {
                            self.emit_node(n, &ccx, &k, [w, h], 1.0, &mut ops);
                        }
                        ChanData::List(DisplayList { width: dw as u32, height: dh as u32, background: Rgba([0.0; 4]), ops })
                    }
                    ChannelC::Comp { ev, time } => ChanData::Comp(Arc::downgrade(ev), time.map(lt, ev.duration)),
                    ChannelC::Buffer(j) => ChanData::Buffer(*j),
                },
                *o,
            ));
        }
        let image = self.job_for(sc, cx, v, (dw as u32, dh as u32), lt, v[prop::PROGRESS], chans);
        if sc.filter {
            out.push(Op::Image { image, dst: [0.0, 0.0, fw, fh], opacity: alpha, transform: dl::IDENTITY });
        } else {
            out.push(Op::Image { image, dst: [0.0, 0.0, w, h], opacity: alpha, transform: *xf });
        }
    }

    /// A shader transition between two scenes' ops (`from` = channel 0, `to` = channel 1).
    pub(super) fn emit_shader_transition(&self, sc: &ShaderC, from: (Vec<Op>, Rgba), to: (Vec<Op>, Rgba), t: f32, progress: f32, out: &mut Vec<Op>) {
        let (w, h) = (self.width, self.height);
        let mk = |(ops, bg): (Vec<Op>, Rgba)| frame_list(w, h, bg, ops);
        let o = ChanOpts { wrap: Wrap::Clamp, filter: Filter::Linear, mipmaps: false };
        let mut chans = vec![(ChanData::List(mk(from)), o), (ChanData::List(mk(to)), o)];
        for (c, o) in sc.channels.iter().skip(2) {
            chans.push((if matches!(c, ChannelC::Noise) { ChanData::Noise } else { ChanData::None }, *o));
        }
        let cx = Cx { ev: self, scene_t: t, gt: t, dof: 0.0 };
        let v = [0.0f32; prop::N];
        let ps = preview_scale();
        let size = (((w as f32 * ps).round() as u32).max(1), ((h as f32 * ps).round() as u32).max(1));
        let image = self.job_for(sc, &cx, &v, size, t, progress, chans);
        out.push(Op::Image { image, dst: [0.0, 0.0, w as f32, h as f32], opacity: 1.0, transform: dl::IDENTITY });
    }
}
