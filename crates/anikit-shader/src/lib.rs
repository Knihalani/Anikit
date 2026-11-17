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
    /// Treat `fragColor.a` as coverage (straight alpha). When false the
    /// output is opaque, as on Shadertoy.
    pub alpha: bool,
    /// Dither the 8-bit output (`render`): `floor(v·255 + r)` with a fixed
    /// per-pixel threshold `r` in [0, 1). Removes banding from smooth
    /// gradients; values that are exactly representable are unchanged.
    pub dither: bool,
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
            alpha: false,
            dither: false,
        }
    }
}

#[derive(Clone, Debug)]
struct Slot {
    off: u32,
    /// Per slot: true when the shader reads it as an int/bool.
    ints: Vec<bool>,
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
        })
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
        set("iResolution", &[w, h, 1.0]);
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
        if f.supersample {
            Env { lane_dx: [-0.25, 0.25, -0.25, 0.25], lane_dy: [0.25, 0.25, -0.25, -0.25], qstep: 1.0, _pad: [0.0; 3], sizes, channels, chan }
        } else {
            Env { lane_dx: [0.0, 1.0, 0.0, 1.0], lane_dy: [0.0, 0.0, -1.0, -1.0], qstep: 2.0, _pad: [0.0; 3], sizes, channels, chan }
        }
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
            self.refine(f, &mut out, w, h);
        } else {
            self.run(f, &mut out, w, |dst, px, _, _| *dst = px);
        }
        out
    }

    /// Adaptive supersampling: re-shade, with 2×2 sub-samples, every pixel that
    /// differs from any of its 8 neighbours by more than the threshold in any channel.
    fn refine(&self, f: &Frame, out: &mut [[f32; 4]], w: usize, h: usize) {
        let th = f.threshold;
        let src: &[[f32; 4]] = out;
        let differs = |a: [f32; 4], b: [f32; 4]| (0..4).any(|k| (a[k].clamp(0.0, 1.0) - b[k].clamp(0.0, 1.0)).abs() > th);
        let flags: Vec<bool> = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let c = src[i];
                let (x0, x1) = (x.saturating_sub(1), (x + 1).min(w - 1));
                let (y0, y1) = (y.saturating_sub(1), (y + 1).min(h - 1));
                (y0..=y1).any(|yy| (x0..=x1).any(|xx| differs(c, src[yy * w + xx])))
            })
            .collect();
        let mut fs = f.clone();
        fs.supersample = true;
        let uni = self.uniform_buffer(&fs);
        let env = self.env(&fs);
        let func = self.kernel.func;
        let env_ptr = &env as *const Env as usize;
        let uni_ptr = uni.as_ptr() as usize;
        let hf = h as f32;
        out.par_chunks_mut(w).enumerate().for_each_init(Vec::new, |buf: &mut Vec<f32>, (y, row)| {
            let fl = &flags[y * w..y * w + w];
            let mut x = 0;
            while x < w {
                if !fl[x] {
                    x += 1;
                    continue;
                }
                let s = x;
                while x < w && fl[x] {
                    x += 1;
                }
                let n = x - s;
                buf.resize(n * 16, 0.0);
                // SAFETY: as in `run`; one quad per pixel in supersample mode.
                unsafe { func(env_ptr as *const Env, uni_ptr as *const u32, s as f32 + 0.5, hf - y as f32 - 0.5, n as i32, buf.as_mut_ptr()) };
                for q in 0..n {
                    let b = &buf[q * 16..q * 16 + 16];
                    row[s + q] = std::array::from_fn(|k| (0..4).map(|l| sanitize(b[k * 4 + l])).sum::<f32>() * 0.25);
                }
            }
        });
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
            let fl = self.render_f32(f);
            px.par_chunks_mut(w).zip(fl.par_chunks(w)).enumerate().for_each(|(y, (row, src))| {
                for x in 0..w {
                    convert(&mut row[x], src[x], x, y);
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
        let h = out.len().checked_div(w).unwrap_or(0);
        if w == 0 || h == 0 {
            return;
        }
        let uni = self.uniform_buffer(f);
        let env = self.env(f);
        let func = self.kernel.func;
        let hf = h as f32;
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
                let x0 = 0.5f32;
                let y0 = hf - py as f32 - 0.5;
                // SAFETY: env/uniforms outlive the parallel loop; buf holds nq quads.
                unsafe { func(env_ptr as *const Env, uni_ptr as *const u32, x0, y0, nq as i32, buf.as_mut_ptr()) };
                let nrows = rows.len() / w;
                for q in 0..nq {
                    let b = &buf[q * 16..q * 16 + 16];
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
                        for (l, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                            let x = q * 2 + dx;
                            if x < w && dy < nrows {
                                put(&mut rows[dy * w + x], lane(l).map(sanitize), x, py + dy);
                            }
                        }
                    }
                }
            },
        );
    }
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
