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

/// True when the source animates by itself.
pub(crate) fn uses_time(src: &str) -> bool {
    ["iTime", "iFrame", "iDate", "iTimeDelta"].iter().any(|w| src.contains(w))
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

pub(crate) fn set_frame_bg(c: Rgba) {
    FRAME_BG.with(|b| b.set(c));
}

// ---------------------------------------------------------------- jobs

pub(crate) enum ChanData {
    None,
    Noise,
    List(DisplayList),
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

/// Pending and recent shader renders, looked up by image id.
#[derive(Default)]
pub struct ShaderJobs {
    inner: Mutex<Inner>,
    assets: OnceLock<Weak<SharedAssets>>,
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
        let pm = Arc::new(run(&e.job, assets.as_ref()));
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

fn run(job: &Job, assets: Option<&Arc<SharedAssets>>) -> Pixmap {
    let mut owned: Vec<Option<Texture>> = Vec::new();
    for (c, o) in &job.channels {
        owned.push(match c {
            ChanData::List(dl) => Some(list_texture(dl, assets, *o)),
            _ => None,
        });
    }
    let mut chans: [Option<&Texture>; 4] = [None; 4];
    for (i, (c, _)) in job.channels.iter().enumerate().take(4) {
        chans[i] = match c {
            ChanData::None => None,
            ChanData::Noise => Some(noise_texture()),
            ChanData::List(_) => owned[i].as_ref(),
        };
    }
    let unis: Vec<(&str, &[f32])> = job.uniforms.iter().map(|(n, v)| (n.as_str(), v.as_slice())).collect();
    let mut f = Frame::new(job.w, job.h, job.time);
    f.frame = job.frame;
    f.fps = job.fps;
    f.progress = job.progress;
    f.uniforms = &unis;
    f.channels = chans;
    f.supersample = job.supersample;
    f.alpha = job.alpha;
    let mut pm = Pixmap::new(job.w.clamp(1, 65535) as u16, job.h.clamp(1, 65535) as u16);
    let opaque = job.shader.render(&f, pm.data_as_u8_slice_mut());
    pm.set_may_have_transparency(!opaque);
    pm
}

// ---------------------------------------------------------------- emission

fn hash_f32s<H: Hasher>(h: &mut H, v: &[f32]) {
    for x in v {
        x.to_bits().hash(h);
    }
}

impl Evaluator {
    fn job_for(&self, sc: &ShaderC, cx: &Cx, v: &[f32; prop::N], size: (u32, u32), time: f32, progress: f32, chans: Vec<(ChanData, ChanOpts)>) -> ImageId {
        let mut uniforms: Vec<(String, Vec<f32>)> = sc.fixed.clone();
        for (name, off, n) in &sc.uniforms {
            let base = prop::U0 + off;
            uniforms.push((name.clone(), v[base..base + n].to_vec()));
        }
        // Content key: generators without composited inputs render once per distinct input.
        let key = if chans.iter().all(|(c, _)| !matches!(c, ChanData::List(_))) {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            sc.id.hash(&mut h);
            size.hash(&mut h);
            for (_, u) in &uniforms {
                hash_f32s(&mut h, u);
            }
            if sc.dynamic {
                hash_f32s(&mut h, &[time, (cx.gt * self.fps).round()]);
            }
            hash_f32s(&mut h, &[progress]);
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
            supersample: sc.supersample,
            alpha: sc.alpha,
        };
        self.assets.shaders.submit(job, key)
    }

    /// Emit a shader layer. `ops_below` is the frame drawn so far (filters read it).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_shader(&self, sc: &ShaderC, cx: &Cx, v: &[f32; prop::N], xf: &Affine, w: f32, h: f32, alpha: f32, lt: f32, out: &mut Vec<Op>) {
        let (fw, fh) = (self.width as f32, self.height as f32);
        // Render at device resolution.
        let (sx, sy) = ((xf[0] * xf[0] + xf[1] * xf[1]).sqrt(), (xf[2] * xf[2] + xf[3] * xf[3]).sqrt());
        let (dw, dh) = if sc.filter { (fw, fh) } else { ((w * sx).round().clamp(1.0, MAX_SIDE), (h * sy).round().clamp(1.0, MAX_SIDE)) };
        if w < 0.5 || h < 0.5 {
            return;
        }
        let mut chans = Vec::with_capacity(sc.channels.len());
        for (c, o) in &sc.channels {
            chans.push((
                match c {
                    ChannelC::None => ChanData::None,
                    ChannelC::Noise => ChanData::Noise,
                    ChannelC::Below => ChanData::List(DisplayList {
                        width: self.width,
                        height: self.height,
                        background: FRAME_BG.with(|b| b.get()),
                        ops: out.clone(),
                    }),
                    ChannelC::Layers(nodes) => {
                        let mut ops = Vec::new();
                        let ccx = Cx { ev: cx.ev, scene_t: cx.scene_t, gt: cx.gt, dof: 0.0 };
                        let k = [dw / w.max(1e-3), 0.0, 0.0, dh / h.max(1e-3), 0.0, 0.0];
                        for n in nodes {
                            self.emit_node(n, &ccx, &k, [w, h], 1.0, &mut ops);
                        }
                        ChanData::List(DisplayList { width: dw as u32, height: dh as u32, background: Rgba([0.0; 4]), ops })
                    }
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
        let mk = |(ops, bg): (Vec<Op>, Rgba)| DisplayList { width: w, height: h, background: bg, ops };
        let o = ChanOpts { wrap: Wrap::Clamp, filter: Filter::Linear, mipmaps: false };
        let mut chans = vec![(ChanData::List(mk(from)), o), (ChanData::List(mk(to)), o)];
        for (c, o) in sc.channels.iter().skip(2) {
            chans.push((if matches!(c, ChannelC::Noise) { ChanData::Noise } else { ChanData::None }, *o));
        }
        let cx = Cx { ev: self, scene_t: t, gt: t, dof: 0.0 };
        let v = [0.0f32; prop::N];
        let image = self.job_for(sc, &cx, &v, (w, h), t, progress, chans);
        out.push(Op::Image { image, dst: [0.0, 0.0, w as f32, h as f32], opacity: 1.0, transform: dl::IDENTITY });
    }
}
