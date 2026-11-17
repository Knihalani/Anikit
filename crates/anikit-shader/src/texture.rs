//! Channel textures and the sampler the JIT calls into.
//!
//! Texels are straight-alpha `f32` RGBA, stored top row first. Texture
//! coordinates follow GLSL/Shadertoy: `(0, 0)` is the bottom-left corner,
//! so `texture(iChannel0, fragCoord / iResolution.xy)` is upright.

/// Address mode outside `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Wrap {
    #[default]
    Clamp,
    Repeat,
    Mirror,
}

/// Magnification/minification filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    Linear,
    Nearest,
}

/// Texels are stored as straight RGBA8 (the sources are 8-bit): a 1080p
/// channel is 8 MB instead of 33, which matters for scattered reads.
struct Level {
    w: usize,
    h: usize,
    px: Vec<[u8; 4]>,
}

const INV255: f32 = 1.0 / 255.0;

#[inline]
fn texel(p: [u8; 4]) -> [f32; 4] {
    [p[0] as f32 * INV255, p[1] as f32 * INV255, p[2] as f32 * INV255, p[3] as f32 * INV255]
}

fn quantize(c: [f32; 4]) -> [u8; 4] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
}

/// One bound channel.
pub struct Texture {
    levels: Vec<Level>,
    pub wrap: Wrap,
    pub filter: Filter,
}

impl Texture {
    /// From straight or premultiplied RGBA8 (top row first).
    pub fn from_rgba8(w: u32, h: u32, rgba: &[u8], premultiplied: bool) -> Texture {
        let (w, h) = (w.max(1) as usize, h.max(1) as usize);
        let mut px = Vec::with_capacity(w * h);
        for p in rgba.chunks_exact(4).take(w * h) {
            let a = p[3];
            if premultiplied && a > 0 && a < 255 {
                let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                px.push([un(p[0]), un(p[1]), un(p[2]), a]);
            } else {
                px.push([p[0], p[1], p[2], a]);
            }
        }
        px.resize(w * h, [0; 4]);
        Texture { levels: vec![Level { w, h, px }], wrap: Wrap::Clamp, filter: Filter::Linear }
    }

    /// From straight-alpha float texels (top row first; stored as 8-bit).
    pub fn from_f32(w: u32, h: u32, px: Vec<[f32; 4]>) -> Texture {
        let (w, h) = (w.max(1) as usize, h.max(1) as usize);
        let mut px: Vec<[u8; 4]> = px.into_iter().map(quantize).collect();
        px.resize(w * h, [0; 4]);
        Texture { levels: vec![Level { w, h, px }], wrap: Wrap::Clamp, filter: Filter::Linear }
    }

    pub fn width(&self) -> u32 {
        self.levels[0].w as u32
    }

    pub fn height(&self) -> u32 {
        self.levels[0].h as u32
    }

    pub fn with_wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    pub fn has_mipmaps(&self) -> bool {
        self.levels.len() > 1
    }

    /// Build the mip chain (2×2 box filter), for `textureLod` and minified
    /// `texture` lookups.
    pub fn with_mipmaps(mut self) -> Self {
        self.levels.truncate(1);
        while {
            let l = self.levels.last().unwrap();
            l.w > 1 || l.h > 1
        } {
            let l = self.levels.last().unwrap();
            let (w, h) = ((l.w / 2).max(1), (l.h / 2).max(1));
            let mut px = vec![[0u8; 4]; w * h];
            for y in 0..h {
                for x in 0..w {
                    let mut s = [0.0f32; 4];
                    let (x0, y0) = ((x * 2).min(l.w - 1), (y * 2).min(l.h - 1));
                    let (x1, y1) = ((x * 2 + 1).min(l.w - 1), (y * 2 + 1).min(l.h - 1));
                    for (xx, yy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                        let p = l.px[yy * l.w + xx];
                        for k in 0..4 {
                            s[k] += p[k] as f32;
                        }
                    }
                    px[y * w + x] = s.map(|v| (v * 0.25 + 0.5) as u8);
                }
            }
            self.levels.push(Level { w, h, px });
        }
        self
    }

    fn wrap_i(&self, i: i64, n: usize) -> usize {
        let n = n as i64;
        (match self.wrap {
            Wrap::Clamp => i.clamp(0, n - 1),
            Wrap::Repeat => i.rem_euclid(n),
            Wrap::Mirror => {
                let p = i.rem_euclid(2 * n);
                if p < n {
                    p
                } else {
                    2 * n - 1 - p
                }
            }
        }) as usize
    }

    fn sample_level(&self, li: usize, u: f32, v: f32) -> [f32; 4] {
        let l = &self.levels[li];
        // Row 0 is the top of the image; v = 0 is the bottom.
        let x = u * l.w as f32;
        let y = (1.0 - v) * l.h as f32;
        if self.filter == Filter::Nearest {
            let xi = self.wrap_i(x.floor() as i64, l.w);
            let yi = self.wrap_i(y.floor() as i64, l.h);
            return texel(l.px[yi * l.w + xi]);
        }
        let (x, y) = (x - 0.5, y - 0.5);
        let (fx, fy) = (x.floor(), y.floor());
        let (tx, ty) = (x - fx, y - fy);
        let (x0, y0) = (fx as i64, fy as i64);
        let xa = self.wrap_i(x0, l.w);
        let xb = self.wrap_i(x0 + 1, l.w);
        let ya = self.wrap_i(y0, l.h) * l.w;
        let yb = self.wrap_i(y0 + 1, l.h) * l.w;
        let (p00, p10, p01, p11) = (texel(l.px[ya + xa]), texel(l.px[ya + xb]), texel(l.px[yb + xa]), texel(l.px[yb + xb]));
        let mut o = [0.0; 4];
        for k in 0..4 {
            let top = p00[k] + (p10[k] - p00[k]) * tx;
            let bot = p01[k] + (p11[k] - p01[k]) * tx;
            o[k] = top + (bot - top) * ty;
        }
        o
    }

    /// Sample at `(u, v)` and level of detail `lod`.
    pub fn sample(&self, u: f32, v: f32, lod: f32) -> [f32; 4] {
        let (u, v) = (if u.is_finite() { u } else { 0.0 }, if v.is_finite() { v } else { 0.0 });
        // Keep huge coordinates from overflowing the integer conversion.
        let (u, v) = (u.clamp(-1.0e6, 1.0e6), v.clamp(-1.0e6, 1.0e6));
        if self.levels.len() == 1 || lod.is_nan() || lod <= 0.0 {
            return self.sample_level(0, u, v);
        }
        let max = (self.levels.len() - 1) as f32;
        let lod = lod.min(max);
        let l0 = lod.floor();
        let t = lod - l0;
        let a = self.sample_level(l0 as usize, u, v);
        if t <= 0.0 {
            return a;
        }
        let b = self.sample_level((l0 as usize + 1).min(self.levels.len() - 1), u, v);
        std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
    }

    /// `texelFetch`: integer texel, `y = 0` is the bottom row. Outside → 0.
    pub fn fetch(&self, x: i32, y: i32, lod: i32) -> [f32; 4] {
        let l = &self.levels[(lod.max(0) as usize).min(self.levels.len() - 1)];
        if x < 0 || y < 0 || x as usize >= l.w || y as usize >= l.h {
            return [0.0; 4];
        }
        texel(l.px[(l.h - 1 - y as usize) * l.w + x as usize])
    }
}

/// What the kernel reads per call (`#[repr(C)]`, offsets are baked into the JIT).
#[repr(C)]
pub struct Env {
    /// Lane offsets within a quad (fragCoord).
    pub lane_dx: [f32; 4],
    pub lane_dy: [f32; 4],
    /// x advance per quad.
    pub qstep: f32,
    pub _pad: [f32; 3],
    /// Channel sizes (w, h) for `textureSize`.
    pub sizes: [[i32; 2]; 4],
    pub channels: [*const Texture; 4],
    /// What the JIT's inline bilinear path needs per channel.
    pub chan: [ChanInfo; 4],
}

/// Per-channel data for inline sampling (`#[repr(C)]`, offsets baked into the JIT).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ChanInfo {
    /// Level-0 RGBA8 texels, row 0 at the top.
    pub data: *const u8,
    pub w: i32,
    pub h: i32,
    pub wf: f32,
    pub hf: f32,
    /// 1 = repeat wrap (else clamp).
    pub repeat: i32,
    /// 1 = nearest filtering.
    pub nearest: i32,
    /// 1 = the inline path applies (bound, no mipmaps, no mirror wrap).
    pub simple: i32,
    pub _pad: i32,
}

impl ChanInfo {
    pub const NONE: ChanInfo = ChanInfo { data: std::ptr::null(), w: 1, h: 1, wf: 1.0, hf: 1.0, repeat: 0, nearest: 0, simple: 0, _pad: 0 };

    pub fn of(t: &Texture) -> ChanInfo {
        let l = &t.levels[0];
        ChanInfo {
            data: l.px.as_ptr() as *const u8,
            w: l.w as i32,
            h: l.h as i32,
            wf: l.w as f32,
            hf: l.h as f32,
            repeat: (t.wrap == Wrap::Repeat) as i32,
            nearest: (t.filter == Filter::Nearest) as i32,
            simple: (t.levels.len() == 1 && t.wrap != Wrap::Mirror) as i32,
            _pad: 0,
        }
    }
}

pub const ENV_CHAN: i32 = 112;
pub const CHAN_SIZE: i32 = 40;

pub const ENV_LANE_DX: i32 = 0;
pub const ENV_LANE_DY: i32 = 16;
pub const ENV_QSTEP: i32 = 32;
pub const ENV_SIZES: i32 = 48;

/// Sample modes passed by the JIT.
pub const MODE_LOD0: i32 = 0;
pub const MODE_LOD: i32 = 1;
pub const MODE_AUTO: i32 = 2;

/// `buf`: u[4] v[4] lod_or_bias[4] dudx[4] dvdx[4] dudy[4] dvdy[4]; writes r[4] g[4] b[4] a[4] at the start.
pub(crate) extern "C" fn ak_sample(env: *const Env, ch: i32, buf: *mut f32, mode: i32) {
    // SAFETY: the kernel passes its own env and a 28-float stack buffer.
    let (env, buf) = unsafe { (&*env, std::slice::from_raw_parts_mut(buf, 28)) };
    let tex = env.channels.get(ch as usize).copied().unwrap_or(std::ptr::null());
    let mut out = [[0.0f32; 4]; 4];
    if !tex.is_null() {
        // SAFETY: channel pointers stay valid for the whole render call.
        let t = unsafe { &*tex };
        let (w, h) = (t.width() as f32, t.height() as f32);
        for l in 0..4 {
            let lod = match mode {
                MODE_LOD => buf[8 + l],
                MODE_AUTO if t.has_mipmaps() => {
                    let (ax, ay) = (buf[12 + l] * w, buf[16 + l] * h);
                    let (bx, by) = (buf[20 + l] * w, buf[24 + l] * h);
                    let rho = (ax * ax + ay * ay).max(bx * bx + by * by).sqrt();
                    if rho > 0.0 && rho.is_finite() {
                        rho.log2() + buf[8 + l]
                    } else {
                        buf[8 + l]
                    }
                }
                _ => 0.0,
            };
            out[l] = t.sample(buf[l], buf[4 + l], if lod.is_finite() { lod } else { 0.0 });
        }
    }
    for c in 0..4 {
        for l in 0..4 {
            buf[c * 4 + l] = out[l][c];
        }
    }
}

/// `buf`: x[4] y[4] lod[4] as i32; writes rgba planes as f32 at the start.
pub(crate) extern "C" fn ak_fetch(env: *const Env, ch: i32, buf: *mut i32) {
    // SAFETY: see `ak_sample`.
    let (env, ibuf) = unsafe { (&*env, std::slice::from_raw_parts_mut(buf, 16)) };
    let tex = env.channels.get(ch as usize).copied().unwrap_or(std::ptr::null());
    let mut out = [[0.0f32; 4]; 4];
    if !tex.is_null() {
        // SAFETY: see `ak_sample`.
        let t = unsafe { &*tex };
        for l in 0..4 {
            out[l] = t.fetch(ibuf[l], ibuf[4 + l], ibuf[8 + l]);
        }
    }
    for c in 0..4 {
        for l in 0..4 {
            ibuf[c * 4 + l] = out[l][c].to_bits() as i32;
        }
    }
}
