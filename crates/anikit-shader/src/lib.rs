//! # anikit-shader
//!
//! Shadertoy-style fragment shaders on the CPU: GLSL (or WGSL) is parsed and
//! validated with naga, then JIT-compiled with Cranelift into a SIMD kernel
//! that shades 2×2 pixel quads (4 lanes, one per pixel, like a GPU quad) and
//! is run over rows on rayon threads.
//!
//! ```
//! use anikit_shader::{Shader, Frame};
//! let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(p / iResolution.xy, 0.5, 1.0); }").unwrap();
//! let mut px = vec![0u8; 64 * 32 * 4];
//! s.render(&Frame::new(64, 32, 0.0), &mut px);
//! ```

// Polynomial coefficients are written as published (Cephes); slot loops index parallel arrays.
#![allow(clippy::excessive_precision, clippy::needless_range_loop)]

pub mod front;
pub mod jit;
pub mod library;
pub mod texture;
mod vmath;

pub use front::{DeclaredUniform, Kind, Lang, ShaderError, UType};
pub use texture::{Filter, Texture, Wrap};

/// A single-file multi-pass shader split into its passes.
pub struct PassSources {
    pub image: String,
    pub image_channels: Vec<String>,
    /// (name, source, channels).
    pub buffers: Vec<(String, String, Vec<String>)>,
}

/// Split `//--- buffer A: A, noise` / `//--- image: A` sections. Text before
/// the first section is common code, prepended to every pass (with blank
/// lines keeping each pass's line numbers). `None` when there are no sections.
pub fn split_passes(src: &str) -> Option<PassSources> {
    let lines: Vec<&str> = src.lines().collect();
    let heads: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.trim_start().starts_with("//---")).map(|(i, _)| i).collect();
    if heads.is_empty() {
        return None;
    }
    let common = lines[..heads[0]].join("\n");
    let chans = |spec: &str| -> Vec<String> { spec.split(',').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect() };
    let mut out = PassSources { image: String::new(), image_channels: Vec::new(), buffers: Vec::new() };
    let mut have_image = false;
    for (k, &h) in heads.iter().enumerate() {
        let end = heads.get(k + 1).copied().unwrap_or(lines.len());
        let head = lines[h].trim_start().trim_start_matches("//---").trim();
        let (kind, spec) = head.split_once(':').unwrap_or((head, ""));
        // Pad so errors report the line in the file.
        let pad = "\n".repeat(h + 1 - heads[0].min(h + 1));
        let body = format!("{common}\n{pad}{}", lines[h + 1..end].join("\n"));
        let mut words = kind.split_whitespace();
        match (words.next(), words.next()) {
            (Some("image"), _) => {
                out.image = body;
                out.image_channels = chans(spec);
                have_image = true;
            }
            (Some("buffer"), Some(name)) => out.buffers.push((name.to_string(), body, chans(spec))),
            _ => return None,
        }
    }
    have_image.then_some(out)
}

static LANE_GROUPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(2);

/// Quads shaded together per kernel iteration, for shaders compiled from now on.
/// 2 (the default) gives the CPU two independent dependency chains, which makes
/// raymarchers up to 1.4× faster, at about twice the compile time. 1 compiles
/// fastest: best for a single still. Output is identical either way.
/// `ANIKIT_SHADER_GROUPS=1|2` overrides it.
pub fn set_lane_groups(n: usize) {
    LANE_GROUPS.store(n.clamp(1, 2), std::sync::atomic::Ordering::Relaxed);
}

/// Directory for compiled kernels: `ANIKIT_SHADER_CACHE` (`0` turns the cache
/// off), else `$XDG_CACHE_HOME/anikit/shaders`, else `~/.cache/anikit/shaders`.
fn disk_dir() -> Option<std::path::PathBuf> {
    // Unit tests compile for real unless a test points the cache somewhere.
    if cfg!(test) && std::env::var_os("ANIKIT_SHADER_CACHE").is_none() {
        return None;
    }
    match std::env::var_os("ANIKIT_SHADER_CACHE") {
        Some(v) if v == "0" || v.is_empty() => None,
        Some(v) => Some(v.into()),
        None => std::env::var_os("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".cache")))
            .map(|d| d.join("anikit").join("shaders")),
    }
}

/// Cache key: the source and declared uniforms, the lane groups, this
/// build's code generator (its own source text) and the CPU's vector features.
fn disk_key(src: &str, extra: &[(&str, UType)]) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    static GEN: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    let gen = *GEN.get_or_init(|| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (env!("CARGO_PKG_VERSION"), include_str!("jit.rs"), include_str!("vmath.rs"), include_str!("texture.rs"), include_str!("front.rs"), include_str!("lib.rs")).hash(&mut h);
        #[cfg(target_arch = "x86_64")]
        {
            for f in [
                std::arch::is_x86_feature_detected!("sse4.1"),
                std::arch::is_x86_feature_detected!("sse4.2"),
                std::arch::is_x86_feature_detected!("avx"),
                std::arch::is_x86_feature_detected!("avx2"),
                std::arch::is_x86_feature_detected!("fma"),
                std::arch::is_x86_feature_detected!("bmi1"),
                std::arch::is_x86_feature_detected!("bmi2"),
                std::arch::is_x86_feature_detected!("lzcnt"),
                std::arch::is_x86_feature_detected!("popcnt"),
                std::arch::is_x86_feature_detected!("avx512f"),
                std::arch::is_x86_feature_detected!("avx512vl"),
            ] {
                f.hash(&mut h);
            }
        }
        h.finish()
    });
    disk_dir()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (gen, src, lane_groups()).hash(&mut h);
    for (n, t) in extra {
        (n, format!("{t:?}")).hash(&mut h);
    }
    Some(h.finish())
}

fn disk_path(key: u64) -> Option<std::path::PathBuf> {
    Some(disk_dir()?.join(format!("{key:016x}.bin")))
}

/// Best effort: a failed write just means compiling next time.
fn disk_store(key: u64, blob: &[u8]) {
    let Some(path) = disk_path(key) else { return };
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join(format!("{key:016x}.{}.tmp", std::process::id()));
    if std::fs::write(&tmp, blob).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
    // Keep the cache bounded: past MAX entries, drop the oldest quarter.
    const MAX: usize = 400;
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = rd
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "bin"))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .collect();
        if files.len() > MAX {
            files.sort();
            for (_, f) in files.iter().take(files.len() - MAX * 3 / 4) {
                let _ = std::fs::remove_file(f);
            }
        }
    }
}

/// See [`set_lane_groups`].
pub fn lane_groups() -> usize {
    match std::env::var("ANIKIT_SHADER_GROUPS").ok().as_deref() {
        Some("1") => 1,
        Some("2") => 2,
        _ => LANE_GROUPS.load(std::sync::atomic::Ordering::Relaxed),
    }
}

use rayon::prelude::*;
use std::collections::HashMap;
use texture::Env;

/// Everything that varies per frame.
#[derive(Clone)]
pub struct Frame<'a> {
    pub width: u32,
    pub height: u32,
    /// `iTime`, seconds.
    pub time: f32,
    /// `iFrame`.
    pub frame: i32,
    /// `iTimeDelta` is `1 / fps`; `iFrameRate` is `fps`.
    pub fps: f32,
    /// `progress` for transitions (0..1).
    pub progress: f32,
    /// `iMouse` (pixels; Shadertoy semantics).
    pub mouse: [f32; 4],
    /// `iAudio`: music loudness now, 0..1 — (level, bass, mid, treble).
    pub audio: [f32; 4],
    /// `iBeat`: (pulse 1 → 0 after each beat, phase 0..1 through the beat,
    /// beat number from 1, bar pulse 1 → 0 after each downbeat).
    pub beat: [f32; 4],
    /// User uniforms by name (1–4 floats each).
    pub uniforms: &'a [(&'a str, &'a [f32])],
    /// Bound textures (`iChannel0..3`, `getFromColor` = 0, `getToColor` = 1).
    pub channels: [Option<&'a Texture>; 4],
    /// 2×2 supersampling (four shader invocations per output pixel).
    pub supersample: bool,
    /// With `supersample`: shade once per pixel first and supersample only
    /// pixels that differ from one of their 8 neighbours by more than [`ADAPTIVE_THRESHOLD`]
    /// (edges, highlights, fine detail). Smooth regions keep the centre sample.
    pub adaptive: bool,
    /// With `adaptive`: refine a pixel when a channel differs from one of its
    /// 8 neighbours by more than this (0..1). Default [`ADAPTIVE_THRESHOLD`].
    pub threshold: f32,
    /// With `adaptive`: refine at most this fraction of the pixels, the
    /// highest-contrast first (a cost cap). 1 = no cap.
    pub max_refine: f32,
    /// Treat `fragColor.a` as coverage (straight alpha). When false the
    /// output is opaque, as on Shadertoy.
    pub alpha: bool,
    /// Dither the 8-bit output (`render`): `floor(v·255 + r)` with a fixed
    /// per-pixel threshold `r` in [0, 1). Removes banding from smooth
    /// gradients; values that are exactly representable are unchanged.
    pub dither: bool,
    /// fragCoord units per output pixel (default 1). A preview rendered at
    /// 1/k of the final size sets k: `fragCoord` and `iResolution` are then the
    /// final render's, so pixel-sized looks (cells, grain, line widths) match
    /// a downscaled final instead of shrinking with the preview.
    pub coord_scale: f32,
}

impl<'a> Frame<'a> {
    pub fn new(width: u32, height: u32, time: f32) -> Self {
        Frame {
            width,
            height,
            time,
            frame: 0,
            fps: 30.0,
            progress: 0.0,
            mouse: [0.0; 4],
            audio: [0.0; 4],
            beat: [0.0; 4],
            uniforms: &[],
            channels: [None; 4],
            supersample: false,
            adaptive: true,
            threshold: ADAPTIVE_THRESHOLD,
            max_refine: 1.0,
            alpha: false,
            dither: false,
            coord_scale: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Slot {
    off: u32,
    /// Per slot: true when the shader reads it as an int/bool.
    ints: Vec<bool>,
}

#[derive(Default)]
struct AaScratch {
    img: Vec<[f32; 4]>,
    contrast: Vec<f32>,
}

/// Reusable adaptive-AA buffers, shared by all shaders; at most
/// `AA_POOL_MAX` are kept, so retained memory doesn't grow with shader count.
const AA_POOL_MAX: usize = 2;
static AA_POOL: std::sync::Mutex<Vec<AaScratch>> = std::sync::Mutex::new(Vec::new());

fn take_scratch() -> AaScratch {
    AA_POOL.lock().ok().and_then(|mut p| p.pop()).unwrap_or_default()
}

fn give_scratch(s: AaScratch) {
    if let Ok(mut p) = AA_POOL.lock() {
        if p.len() < AA_POOL_MAX {
            p.push(s);
        }
    }
}

/// A compiled shader. Cheap to share between threads; `render` is `&self`.
pub struct Shader {
    kernel: jit::Kernel,
    slots: HashMap<String, Slot>,
    uni_len: usize,
    pub kind: Kind,
    pub lang: Lang,
    /// Uniforms the source declares (`uniform float x; // = 1.0`) or that
    /// were declared for it from the caller's values.
    pub uniforms: Vec<DeclaredUniform>,
    /// Named `uniform sampler2D` inputs and the channel each was given.
    pub samplers: Vec<(String, u32)>,
    /// Which channels the code samples.
    pub channels_used: [bool; 4],
    /// Compile time, milliseconds (parse + validate + JIT).
    pub compile_ms: f64,
    /// Size of the generated code in Cranelift IR instructions.
    pub insts: usize,
    /// First pixel (fragCoord x, y) a render produced NaN or ±Inf at, packed
    /// `x << 32 | y`, or `u64::MAX`: see [`Shader::take_nonfinite`].
    nonfinite: std::sync::atomic::AtomicU64,
}

impl Shader {
    /// Compile with no extra uniforms.
    pub fn compile(src: &str) -> Result<Shader, ShaderError> {
        Self::compile_with(src, &[])
    }

    /// Like [`Shader::compile_with`], but shared: each distinct (source,
    /// extra uniforms) is compiled once per process and reused, so repeated
    /// layers, re-renders and watch mode pay no JIT time. Errors aren't cached.
    pub fn compile_shared(src: &str, extra: &[(&str, UType)]) -> Result<std::sync::Arc<Shader>, ShaderError> {
        use std::hash::{Hash, Hasher};
        use std::sync::{Arc, Mutex, OnceLock};
        type Cache = Mutex<HashMap<u64, Arc<OnceLock<Result<Arc<Shader>, ShaderError>>>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        src.hash(&mut h);
        for (n, t) in extra {
            n.hash(&mut h);
            format!("{t:?}").hash(&mut h);
        }
        lane_groups().hash(&mut h);
        let key = h.finish();
        let cell = CACHE.get_or_init(Default::default).lock().unwrap().entry(key).or_default().clone();
        // Concurrent requests for the same shader wait for one compile.
        let r = cell.get_or_init(|| Shader::compile_with(src, extra).map(Arc::new)).clone();
        if r.is_err() {
            CACHE.get().unwrap().lock().unwrap().remove(&key);
        }
        r
    }

    /// Compile, declaring `extra` uniforms (name, type) the source may use
    /// without declaring them.
    pub fn compile_with(src: &str, extra: &[(&str, UType)]) -> Result<Shader, ShaderError> {
        let t0 = std::time::Instant::now();
        let ex: Vec<front::Extra> = extra.iter().map(|(n, t)| front::Extra { name: n, ty: *t }).collect();
        let p = front::parse(src, &ex)?;
        let mut slots = HashMap::new();
        let mut off = 0u32;
        let wanted = |n: &str| front::BUILTIN_NAMES.contains(&n) || p.uniforms.iter().any(|u| u.name == n);
        for (_, g) in p.module.global_variables.iter() {
            if g.space != naga::AddressSpace::Private {
                continue;
            }
            let Some(name) = g.name.as_deref() else { continue };
            if !wanted(name) || slots.contains_key(name) {
                continue;
            }
            let ints = scalar_ints(&p.module, &p.module.types[g.ty].inner);
            let n = ints.len() as u32;
            slots.insert(name.to_string(), Slot { off, ints });
            off += n;
        }
        // Transitions read iResolution even when the source doesn't.
        if !slots.contains_key("iResolution") {
            slots.insert("iResolution".into(), Slot { off, ints: vec![false; 3] });
            off += 3;
        }
        let layout: jit::Layout = slots.iter().map(|(k, v)| (k.clone(), v.off)).collect();
        // Channels the code actually samples (the prelude declares all four).
        let mut channels_used = [false; 4];
        let mut mark = |f: &naga::Function| {
            for (_, e) in f.expressions.iter() {
                let img = match e {
                    naga::Expression::ImageSample { image, .. } | naga::Expression::ImageLoad { image, .. } | naga::Expression::ImageQuery { image, .. } => *image,
                    _ => continue,
                };
                if let naga::Expression::GlobalVariable(g) = &f.expressions[img] {
                    let gv = &p.module.global_variables[*g];
                    if let (Some(b), naga::TypeInner::Image { .. }) = (&gv.binding, &p.module.types[gv.ty].inner) {
                        if b.binding < 4 {
                            channels_used[b.binding as usize] = true;
                        }
                    }
                }
            }
        };
        for (_, f) in p.module.functions.iter() {
            mark(f);
        }
        // Compiled kernels are cached on disk, so a fresh process (`still`)
        // doesn't pay the JIT again for a shader it has seen.
        let key = disk_key(src, extra);
        let cached = key.and_then(|k| std::fs::read(disk_path(k)?).ok()).and_then(|b| jit::Kernel::from_blob(&b));
        let kernel = match cached {
            Some(k) => k,
            None => {
                let mut k = jit::compile(&p.module, &p.info, p.entry, p.kind, &layout).map_err(|mut e| {
                    if e.line.is_none() {
                        e.message = format!("{} (while compiling)", e.message);
                    }
                    e
                })?;
                if let (Some(key), Some(blob)) = (key, k.blob.take()) {
                    disk_store(key, &blob);
                }
                k
            }
        };
        Ok(Shader {
            insts: kernel.insts,
            kernel,
            slots,
            uni_len: off as usize,
            kind: p.kind,
            lang: p.lang,
            uniforms: p.uniforms,
            samplers: p.samplers,
            channels_used,
            compile_ms: t0.elapsed().as_secs_f64() * 1e3,
            nonfinite: std::sync::atomic::AtomicU64::new(u64::MAX),
        })
    }

    /// The first pixel (fragCoord x, y, rounded down) at which a render since
    /// the last call produced NaN or ±Inf (shown as 0), and clear it. A NaN
    /// usually means a division by zero, `sqrt`/`log`/`pow` of a negative, or
    /// `normalize` of a zero vector.
    pub fn take_nonfinite(&self) -> Option<(u32, u32)> {
        let v = self.nonfinite.swap(u64::MAX, std::sync::atomic::Ordering::Relaxed);
        (v != u64::MAX).then_some(((v >> 32) as u32, v as u32))
    }

    fn uniform_buffer(&self, f: &Frame) -> Vec<u32> {
        let mut buf = vec![0u32; self.uni_len.max(1)];
        let mut set = |name: &str, vals: &[f32]| {
            if let Some(s) = self.slots.get(name) {
                for (i, is_int) in s.ints.iter().enumerate() {
                    let v = vals.get(i).copied().unwrap_or(0.0);
                    buf[s.off as usize + i] = if *is_int { (v.round() as i32) as u32 } else { v.to_bits() };
                }
            }
        };
        for u in &self.uniforms {
            if let Some(d) = &u.default {
                set(&u.name, d);
            }
        }
        let (w, h) = (f.width as f32, f.height as f32);
        set("iTime", &[f.time]);
        set("iTimeDelta", &[1.0 / f.fps.max(1e-3)]);
        set("iFrameRate", &[f.fps]);
        set("iFrame", &[f.frame as f32]);
        let k = f.coord_scale;
        set("iResolution", &[w * k, h * k, 1.0]);
        set("iMouse", &f.mouse);
        set("iDate", &[2026.0, 0.0, 1.0, f.time]);
        set("iSampleRate", &[48000.0]);
        set("progress", &[f.progress]);
        set("ratio", &[w / h.max(1.0)]);
        set("iAudio", &f.audio);
        set("iBeat", &f.beat);
        let mut cres = [0.0f32; 12];
        for (i, c) in f.channels.iter().enumerate() {
            if let Some(t) = c {
                cres[i * 3] = t.width() as f32;
                cres[i * 3 + 1] = t.height() as f32;
                cres[i * 3 + 2] = 1.0;
            }
        }
        set("iChannelResolution", &cres);
        set("iChannelTime", &[f.time; 4]);
        for (n, v) in f.uniforms {
            set(n, v);
        }
        buf
    }

    fn env(&self, f: &Frame) -> Env {
        let mut sizes = [[0i32; 2]; 4];
        let mut channels = [std::ptr::null(); 4];
        let mut chan = [texture::ChanInfo::NONE; 4];
        for (i, c) in f.channels.iter().enumerate() {
            if let Some(t) = c {
                sizes[i] = [t.width() as i32, t.height() as i32];
                channels[i] = *t as *const Texture;
                chan[i] = texture::ChanInfo::of(t);
            }
        }
        let k = f.coord_scale;
        let (dx, dy, qstep) = if f.supersample { ([-0.25, 0.25, -0.25, 0.25], [0.25, 0.25, -0.25, -0.25], 1.0) } else { ([0.0, 1.0, 0.0, 1.0], [0.0, 0.0, -1.0, -1.0], 2.0) };
        Env { lane_dx: dx.map(|v: f32| v * k), lane_dy: dy.map(|v: f32| v * k), qstep: qstep * k, _pad: [0.0; 3], sizes, channels, chan }
    }

    /// Shade into straight-alpha `f32` RGBA (exact shader output; for tests
    /// and further processing). Rows top to bottom.
    pub fn render_f32(&self, f: &Frame) -> Vec<[f32; 4]> {
        let (w, h) = (f.width as usize, f.height as usize);
        let mut out = vec![[0.0f32; 4]; w * h];
        if f.supersample && f.adaptive {
            let mut f1 = f.clone();
            f1.supersample = false;
            self.run(&f1, &mut out, w, |dst, px, _, _| *dst = px);
            let mut scratch = take_scratch();
            let mut none = vec![(); w * h];
            self.refine(f, &mut out, &mut scratch.contrast, w, h, &mut none, |_, _, _| {});
            give_scratch(scratch);
        } else {
            self.run(f, &mut out, w, |dst, px, _, _| *dst = px);
        }
        out
    }

    /// Adaptive supersampling: re-shade, with 2×2 sub-samples, every pixel that
    /// differs from any of its 8 neighbours by more than the threshold in any channel.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn refine<P: Send>(
        &self,
        f: &Frame,
        img: &mut [[f32; 4]],
        contrast: &mut Vec<f32>,
        w: usize,
        h: usize,
        dst: &mut [P],
        put: impl Fn(&mut [P], &[[f32; 4]], usize) + Sync,
    ) {
        if w * h == 0 {
            return;
        }
        contrast.resize(w * h, 0.0);
        // Cost budget: refine at most `max_refine` of the pixels, the highest-contrast
        // ones first (a 256-bin histogram picks the cut). Deterministic: it depends
        // on the image, not on timing.
        let mut th = f.threshold;
        {
            let src: &[[f32; 4]] = img;
            let diff = |a: [f32; 4], b: [f32; 4]| (0..4).map(|k| (a[k].clamp(0.0, 1.0) - b[k].clamp(0.0, 1.0)).abs()).fold(0.0f32, f32::max);
            let contrast_row = |y: usize, row: &mut [f32]| {
                for x in 0..w {
                    let c = src[y * w + x];
                    let (x0, x1) = (x.saturating_sub(1), (x + 1).min(w - 1));
                    let (y0, y1) = (y.saturating_sub(1), (y + 1).min(h - 1));
                    let mut m = 0.0f32;
                    for yy in y0..=y1 {
                        for xx in x0..=x1 {
                            m = m.max(diff(c, src[yy * w + xx]));
                        }
                    }
                    row[x] = m;
                }
            };
            if f.max_refine < 1.0 {
                let hist = contrast
                    .par_chunks_mut(w)
                    .enumerate()
                    .fold(
                        || [0usize; 256],
                        |mut hist, (y, row)| {
                            contrast_row(y, row);
                            for &c in row.iter() {
                                hist[((c * 255.0) as usize).min(255)] += 1;
                            }
                            hist
                        },
                    )
                    .reduce(
                        || [0usize; 256],
                        |mut a, b| {
                            for i in 0..256 {
                                a[i] += b[i];
                            }
                            a
                        },
                    );
                let allowed = (f.max_refine.max(0.0) * (w * h) as f32) as usize;
                let mut above = 0usize;
                for b in (0..256).rev() {
                    if above + hist[b] > allowed {
                        th = th.max((b + 1) as f32 / 255.0);
                        break;
                    }
                    above += hist[b];
                }
            } else {
                contrast.par_chunks_mut(w).enumerate().for_each(|(y, row)| contrast_row(y, row));
            }
        }
        let mut fs = f.clone();
        fs.supersample = true;
        let uni = self.uniform_buffer(&fs);
        let env = self.env(&fs);
        let func = self.kernel.func;
        let env_ptr = &env as *const Env as usize;
        let uni_ptr = uni.as_ptr() as usize;
        let hf = h as f32;
        let k = f.coord_scale;
        img.par_chunks_mut(w).zip(dst.par_chunks_mut(w)).enumerate().for_each_init(Vec::new, |buf: &mut Vec<f32>, (y, (row, dst_row))| {
            let cr = &contrast[y * w..y * w + w];
            let mut x = 0;
            while x < w {
                // NaN threshold refines nothing, as before.
                if !(cr[x] > th) {
                    x += 1;
                    continue;
                }
                let s = x;
                while x < w && cr[x] > th {
                    x += 1;
                }
                let n = x - s;
                buf.resize(n * 16, 0.0);
                // SAFETY: as in `run`; one quad per pixel in supersample mode.
                unsafe { func(env_ptr as *const Env, uni_ptr as *const u32, (s as f32 + 0.5) * k, (hf - y as f32 - 0.5) * k, n as i32, buf.as_mut_ptr()) };
                for q in 0..n {
                    let b = &buf[q * 16..q * 16 + 16];
                    row[s + q] = std::array::from_fn(|k| (0..4).map(|l| sanitize(b[k * 4 + l])).sum::<f32>() * 0.25);
                }
            }
            put(dst_row, row, y);
        });
    }

    fn render_adaptive<P: Send>(
        &self,
        f: &Frame,
        scratch: &mut AaScratch,
        dst: &mut [P],
        put: impl Fn(&mut [P], &[[f32; 4]], usize) + Sync,
    ) {
        let (w, h) = (f.width as usize, f.height as usize);
        scratch.img.resize(w * h, [0.0; 4]);
        let mut f1 = f.clone();
        f1.supersample = false;
        self.run(&f1, &mut scratch.img, w, |dst, px, _, _| *dst = px);
        self.refine(f, &mut scratch.img, &mut scratch.contrast, w, h, dst, put);
    }

    /// Shade into premultiplied RGBA8 (`width × height × 4` bytes), the
    /// renderer's image format. Returns true when every pixel is opaque.
    pub fn render(&self, f: &Frame, out: &mut [u8]) -> bool {
        let (w, h) = (f.width as usize, f.height as usize);
        assert!(out.len() >= w * h * 4, "output buffer too small");
        let alpha = f.alpha;
        let dither = f.dither;
        let opaque = std::sync::atomic::AtomicBool::new(true);
        let px: &mut [[u8; 4]] = bytemuck_rgba(&mut out[..w * h * 4]);
        let convert = |dst: &mut [u8; 4], c: [f32; 4], x: usize, y: usize| {
            let q = |v: f32| -> f32 {
                if v > 0.0 {
                    v.min(1.0)
                } else {
                    0.0
                }
            };
            let a = if alpha { q(c[3]) } else { 1.0 };
            let vals = [q(c[0]) * a, q(c[1]) * a, q(c[2]) * a, a];
            *dst = if dither {
                let r = dither_thresholds(x as u32, y as u32);
                std::array::from_fn(|k| (vals[k] * 255.0 + if k == 3 { 0.5 } else { r[k] }).min(255.0) as u8)
            } else {
                vals.map(|v| (v * 255.0 + 0.5) as u8)
            };
        };
        if f.supersample && f.adaptive {
            let put = |row: &mut [[u8; 4]], src: &[[f32; 4]], y: usize| {
                for x in 0..w {
                    convert(&mut row[x], src[x], x, y);
                }
            };
            let mut scratch = take_scratch();
            self.render_adaptive(f, &mut scratch, px, put);
            give_scratch(scratch);
        } else if !f.supersample {
            self.run_quads(f, px, w, |rows, b, q, py| {
                let o = quad_rgba8(b, (q * 2) as u32, py as u32, alpha, dither);
                let nrows = rows.len() / w;
                let x = q * 2;
                for (l, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                    if x + dx < w && dy < nrows {
                        rows[dy * w + x + dx] = o[l];
                    }
                }
            });
        } else {
            self.run(f, px, w, convert);
        }
        if alpha {
            let any_t = out[..w * h * 4].par_chunks(4 * 1024).any(|c| c.chunks_exact(4).any(|p| p[3] != 255));
            opaque.store(!any_t, std::sync::atomic::Ordering::Relaxed);
        }
        opaque.into_inner()
    }

    /// Run the kernel over the frame; `put` writes one output pixel.
    fn run<P: Send>(&self, f: &Frame, out: &mut [P], w: usize, put: impl Fn(&mut P, [f32; 4], usize, usize) + Sync) {
        let ss = f.supersample;
        self.run_quads(f, out, w, |rows, b, q, py| {
            let lane = |l: usize| [b[l], b[4 + l], b[8 + l], b[12 + l]];
            if ss {
                let mut s = [0.0f32; 4];
                for l in 0..4 {
                    let c = lane(l);
                    for k in 0..4 {
                        s[k] += sanitize(c[k]);
                    }
                }
                put(&mut rows[q], s.map(|v| v * 0.25), q, py);
            } else {
                let nrows = rows.len() / w;
                for (l, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                    let x = q * 2 + dx;
                    if x < w && dy < nrows {
                        put(&mut rows[dy * w + x], lane(l).map(sanitize), x, py + dy);
                    }
                }
            }
        });
    }

    /// Run the kernel over the frame; `quad` gets the output rows of a task,
    /// one quad's 16 floats (r, g, b, a × 4 lanes), the quad index and the top row.
    fn run_quads<P: Send>(&self, f: &Frame, out: &mut [P], w: usize, quad: impl Fn(&mut [P], &[f32; 16], usize, usize) + Sync) {
        let h = out.len().checked_div(w).unwrap_or(0);
        if w == 0 || h == 0 {
            return;
        }
        let uni = self.uniform_buffer(f);
        let env = self.env(f);
        let func = self.kernel.func;
        let hf = h as f32;
        let k = f.coord_scale;
        let ss = f.supersample;
        // Supersampled: one quad per pixel, one row per task. Otherwise a
        // quad covers 2×2 pixels: two rows per task.
        let rows_per = if ss { 1 } else { 2 };
        let nq = if ss { w } else { w.div_ceil(2) };
        let env_ptr = &env as *const Env as usize;
        let uni_ptr = uni.as_ptr() as usize;
        out.par_chunks_mut(w * rows_per).enumerate().for_each_init(
            || vec![0.0f32; nq * 16],
            |buf, (ci, rows)| {
                let py = ci * rows_per;
                let x0 = 0.5 * k;
                let y0 = (hf - py as f32 - 0.5) * k;
                // SAFETY: env/uniforms outlive the parallel loop; buf holds nq quads.
                unsafe { func(env_ptr as *const Env, uni_ptr as *const u32, x0, y0, nq as i32, buf.as_mut_ptr()) };
                // NaN/Inf check: branch-free over the row (vectorises), then locate.
                if buf.iter().fold(false, |a, v| a | !v.is_finite()) {
                    let i = buf.iter().position(|v| !v.is_finite()).unwrap_or(0);
                    let (q, l) = (i / 16, i % 4);
                    let (x, row) = if ss { (q, py) } else { (q * 2 + l % 2, py + l / 2) };
                    let y = h.saturating_sub(row + 1);
                    let packed = ((x as u64) << 32) | y as u64;
                    let _ = self.nonfinite.compare_exchange(u64::MAX, packed, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed);
                }
                for (q, b) in buf.chunks_exact(16).enumerate() {
                    quad(rows, b.try_into().unwrap(), q, py);
                }
            },
        );
    }
}

/// One 1-sample quad to RGBA8, the same bits as the per-pixel conversion in
/// [`Shader::render`] but laid out lane-wise so it vectorises: clamp (NaN to
/// 0), premultiply, dither or round.
#[inline(always)]
fn quad_rgba8(b: &[f32; 16], x: u32, y: u32, alpha: bool, dither: bool) -> [[u8; 4]; 4] {
    // Not `clamp`: that keeps NaN, and NaN must read as 0 here.
    #[allow(clippy::manual_clamp)]
    let cl = |v: f32| v.max(0.0).min(1.0);
    let mut a = [1.0f32; 4];
    if alpha {
        for l in 0..4 {
            a[l] = cl(b[12 + l]);
        }
    }
    let xs = [x, x + 1, x, x + 1];
    let ys = [y, y, y + 1, y + 1];
    let mut h = [0u32; 4];
    for l in 0..4 {
        h[l] = xs[l].wrapping_mul(0x8da6_b343) ^ ys[l].wrapping_mul(0xd816_3841);
    }
    let mut o = [[0u8; 4]; 4];
    for c in 0..3 {
        let mut r = [0.5f32; 4];
        if dither {
            for l in 0..4 {
                let mut v = h[l];
                v ^= v >> 15;
                v = v.wrapping_mul(0x2c1b_3c6d);
                v ^= v >> 12;
                v = v.wrapping_mul(0x297a_2d39);
                v ^= v >> 15;
                h[l] = v;
                r[l] = (v >> 8) as f32 * (1.0 / 16_777_216.0);
            }
        }
        for l in 0..4 {
            o[l][c] = (cl(b[c * 4 + l]) * a[l] * 255.0 + r[l]).min(255.0) as u8;
        }
    }
    for l in 0..4 {
        o[l][3] = (a[l] * 255.0 + 0.5).min(255.0) as u8;
    }
    o
}

/// Per-pixel dither thresholds in [0, 1) for r, g, b (a fixed hash pattern).
fn dither_thresholds(x: u32, y: u32) -> [f32; 3] {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841);
    std::array::from_fn(|_| {
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        h = h.wrapping_mul(0x297a_2d39);
        h ^= h >> 15;
        (h >> 8) as f32 * (1.0 / 16_777_216.0)
    })
}

/// Adaptive supersampling re-shades a pixel when any channel differs from a
/// neighbour by more than this (in output units; 1.5 / 255).
pub const ADAPTIVE_THRESHOLD: f32 = 1.5 / 255.0;

/// The default anti-aliasing threshold for shader layers: the loosest that
/// keeps every library shader within SSIM 0.995 and ΔE76 p99.9 2.3 of full
/// 2×2 supersampling (`examples/aa_check.rs`).
pub const AA_THRESHOLD: f32 = 4.0 / 255.0;

/// The default cost cap for shader layers: anti-alias at most this fraction of
/// the pixels per frame (the highest-contrast ones), so AA costs at most about
/// 1 + 4 × this of a one-sample render.
pub const AA_MAX_REFINE: f32 = 0.2;

/// True when the source opts out of default anti-aliasing with a
/// `// anikit:aa off` comment (soft fields, intentional per-pixel grain).
pub fn aa_off(src: &str) -> bool {
    src.lines().any(|l| l.trim_start().starts_with("//") && l.contains("anikit:aa off"))
}

fn sanitize(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v
    }
}

fn bytemuck_rgba(b: &mut [u8]) -> &mut [[u8; 4]] {
    let n = b.len() / 4;
    // SAFETY: [u8; 4] has alignment 1 and the length is a multiple of 4.
    unsafe { std::slice::from_raw_parts_mut(b.as_mut_ptr() as *mut [u8; 4], n) }
}

fn scalar_ints(m: &naga::Module, t: &naga::TypeInner) -> Vec<bool> {
    use naga::TypeInner as T;
    let is_int = |s: &naga::Scalar| !matches!(s.kind, naga::ScalarKind::Float | naga::ScalarKind::AbstractFloat);
    match t {
        T::Scalar(s) => vec![is_int(s)],
        T::Vector { size, scalar } => vec![is_int(scalar); *size as usize],
        T::Matrix { columns, rows, .. } => vec![false; *columns as usize * *rows as usize],
        T::Array { base, size: naga::ArraySize::Constant(n), .. } => {
            let e = scalar_ints(m, &m.types[*base].inner);
            (0..n.get()).flat_map(|_| e.clone()).collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod layout {
    #[test]
    fn env_offsets_match_the_jit() {
        let e = std::mem::MaybeUninit::<crate::texture::Env>::uninit();
        let p = e.as_ptr();
        // SAFETY: only field addresses are taken.
        let off = |f: *const u8| f as usize - p as usize;
        unsafe {
            assert_eq!(off(std::ptr::addr_of!((*p).sizes) as _), crate::texture::ENV_SIZES as usize);
            assert_eq!(off(std::ptr::addr_of!((*p).chan) as _), crate::texture::ENV_CHAN as usize);
        }
        assert_eq!(std::mem::size_of::<crate::texture::ChanInfo>(), crate::texture::CHAN_SIZE as usize);
    }
}

#[cfg(test)]
mod shared {
    #[test]
    fn compile_shared_reuses_kernels() {
        let src = "void mainImage(out vec4 c, in vec2 p) { c = vec4(0.25); }";
        let a = crate::Shader::compile_shared(src, &[]).unwrap();
        let b = crate::Shader::compile_shared(src, &[]).unwrap();
        assert!(std::sync::Arc::ptr_eq(&a, &b));
        let c = crate::Shader::compile_shared(src, &[("k", crate::UType::Float)]).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&a, &c));
        assert!(crate::Shader::compile_shared("void mainImage(out vec4 c, in vec2 p) { c = vec4(nope); }", &[]).is_err());
    }
}
