//! `product-shot`: a phone on a stone plinth in a dark room, lit by low sun
//! through window blinds, filmed with a slow weighted camera move.
//!
//! The component is thin: [`ProductShot::scene`] turns props and time into a
//! [`Scene`] (camera, device pose, sun and blinds, set, dust, the screen
//! texture), and a [`ShotRenderer`] turns a scene into pixels. The built-in
//! renderer, [`Tracer`], is a specialised CPU ray caster for exactly this set
//! (analytic planes, a rounded slab edge, a signed-distance phone body,
//! analytic volumetric light through the blinds, depth-tested dust); another
//! renderer can draw the same [`Scene`].
//!
//! The screen is any named composition (`"screen": "ui"`), rendered at its
//! own size every frame and mapped onto the glass with trilinear,
//! anisotropic filtering. Everything static (dust seeds, the stone pattern's
//! parameters) is computed once; per-frame work is one pass over the frame,
//! supersampled only where geometry or material changes inside a pixel.

use super::style::{clamp01, fnv, parse_color};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Op, Rgba, IDENTITY};
use serde::Deserialize;
use std::sync::Arc;
use vello_cpu::Pixmap;

// ---------------------------------------------------------------- vectors

pub type V3 = [f32; 3];

#[inline(always)]
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
#[inline(always)]
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline(always)]
fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
#[inline(always)]
fn hadamard(a: V3, b: V3) -> V3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}
#[inline(always)]
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline(always)]
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
#[inline(always)]
fn norm(a: V3) -> V3 {
    mul(a, 1.0 / dot(a, a).sqrt().max(1e-12))
}
#[inline(always)]
fn mad(a: V3, b: V3, k: f32) -> V3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}
#[inline(always)]
fn reflect(d: V3, n: V3) -> V3 {
    mad(d, n, -2.0 * dot(d, n))
}
#[inline(always)]
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp01((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}
#[inline(always)]
fn mixf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
#[inline(always)]
fn mix3(a: V3, b: V3, t: f32) -> V3 {
    [mixf(a[0], b[0], t), mixf(a[1], b[1], t), mixf(a[2], b[2], t)]
}

// ---------------------------------------------------------------- scene

/// A pinhole camera with a focus distance (for dust bokeh).
#[derive(Clone, Debug)]
pub struct Camera {
    pub eye: V3,
    pub target: V3,
    /// Vertical field of view, radians.
    pub fov: f32,
    /// Focus distance (cm) and aperture diameter (cm) for out-of-focus dust.
    pub focus: f32,
    pub aperture: f32,
}

/// The device: a rounded slab with a screen on its +y face.
#[derive(Clone, Debug)]
pub struct Device {
    pub center: V3,
    /// Local x (width), y (screen normal), z (length; screen top at -z).
    pub axes: [V3; 3],
    /// Half extents (cm).
    pub half: V3,
    /// Plan corner radius and edge (profile) radius.
    pub corner: f32,
    pub edge: f32,
    /// Black border between the frame and the active screen (cm).
    pub bezel: f32,
    /// Metal colour (linear F0).
    pub metal: V3,
    pub metal_rough: f32,
}

/// Low sun through a window with horizontal blinds.
#[derive(Clone, Debug)]
pub struct Sun {
    /// Unit vector towards the sun.
    pub dir: V3,
    /// Radiance scale (linear RGB).
    pub color: V3,
    /// Distance from the origin to the window wall (cm).
    pub distance: f32,
    /// Slat period on the window (cm), open fraction, phase (periods).
    pub period: f32,
    pub open: f32,
    pub phase: f32,
    /// Window opening on the wall: horizontal `[u0, u1]`, height `[v0, v1]` (cm).
    pub window: [f32; 4],
}

/// The set: a stone plinth, the room behind it, the air.
#[derive(Clone, Debug)]
pub struct Set {
    /// Linear albedo of the stone.
    pub stone: V3,
    pub stone_rough: f32,
    /// Far edge of the plinth top (z, cm) and its nosing radius.
    pub edge_z: f32,
    pub edge_r: f32,
    /// Room floor height (below the plinth top).
    pub floor_y: f32,
    pub wall: V3,
    /// Room ambient (linear).
    pub ambient: V3,
    /// Haze density (in-scatter strength).
    pub haze: f32,
}

/// A dust mote: position, radius (cm), brightness.
#[derive(Clone, Copy, Debug)]
pub struct Mote {
    pub p: V3,
    pub size: f32,
    pub gain: f32,
}

/// Everything a renderer needs for one frame.
#[derive(Clone)]
pub struct Scene {
    pub camera: Camera,
    pub device: Device,
    pub sun: Sun,
    pub set: Set,
    pub motes: Vec<Mote>,
    /// Screen pixels (premultiplied sRGB RGBA8) and its brightness.
    pub screen: Option<Arc<Pixmap>>,
    pub screen_gain: f32,
    pub exposure: f32,
    /// Bloom strength from real highlights.
    pub bloom: f32,
    /// Seconds (for slow haze drift).
    pub time: f32,
}

/// Turns a [`Scene`] into straight-alpha RGBA8 pixels.
pub trait ShotRenderer {
    fn render(&self, scene: &Scene, w: usize, h: usize) -> Vec<u8>;
}

// ---------------------------------------------------------------- props

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ProductShot {
    /// Named composition drawn on the screen.
    pub screen: Option<String>,
    /// Camera orbit: azimuth (deg, 0 = in front), elevation (deg), distance (cm), start → end.
    pub azimuth: [f32; 2],
    pub elevation: [f32; 2],
    pub distance: [f32; 2],
    /// Vertical field of view, degrees.
    pub fov: f32,
    /// Length of the camera move (s); eased in and out.
    pub duration: f32,
    /// Device yaw (deg), tilt towards the camera (deg), hover height (cm).
    pub yaw: f32,
    pub tilt: f32,
    pub hover: f32,
    /// Slow yaw added over the move (deg).
    pub turn: f32,
    /// `titanium`, `graphite`, `silver`, `gold`.
    pub finish: String,
    /// Sun azimuth (deg, 180 = behind the device) and elevation (deg).
    pub sun_azimuth: f32,
    pub sun_elevation: f32,
    /// Sun strength.
    pub sun: f32,
    /// Blind slat period (cm) and drift (periods per second).
    pub slats: f32,
    pub drift: f32,
    /// Stone colour.
    pub stone: String,
    /// Dust motes in the beams.
    pub dust: u32,
    pub haze: f32,
    pub exposure: f32,
    pub bloom: f32,
    pub screen_brightness: f32,
    /// Aperture for dust bokeh (0 = pinhole).
    pub aperture: f32,
    #[serde(skip)]
    seeds: Vec<[f32; 8]>,
    #[serde(skip)]
    stone_c: V3,
}

impl Default for ProductShot {
    fn default() -> Self {
        ProductShot {
            screen: None,
            azimuth: [-16.0, 6.0],
            elevation: [36.0, 30.0],
            distance: [41.0, 35.0],
            fov: 27.0,
            duration: 7.0,
            yaw: -28.0,
            tilt: 0.0,
            hover: 0.0,
            turn: 6.0,
            finish: "titanium".into(),
            sun_azimuth: -128.0,
            sun_elevation: 35.0,
            sun: 1.0,
            slats: 7.0,
            drift: 0.035,
            stone: "#1c1e22".into(),
            dust: 90,
            haze: 1.0,
            exposure: 1.0,
            bloom: 0.5,
            screen_brightness: 1.0,
            aperture: 2.5,
            seeds: Vec::new(),
            stone_c: [0.0; 3],
        }
    }
}

fn srgb_to_lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn lin(c: Rgba) -> V3 {
    [srgb_to_lin(c.0[0]), srgb_to_lin(c.0[1]), srgb_to_lin(c.0[2])]
}

fn hash(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / 16_777_216.0
}

impl Prepare for ProductShot {
    fn prepare(&mut self) -> Result<(), Error> {
        let c = parse_color(&self.stone).ok_or_else(|| Error(format!("`stone`: `{}` is not a colour", self.stone)))?;
        self.stone_c = lin(c);
        if !matches!(self.finish.as_str(), "titanium" | "graphite" | "silver" | "gold") {
            return Err(Error(format!("unknown finish `{}`; use titanium, graphite, silver or gold", self.finish)));
        }
        if self.fov <= 1.0 || self.fov >= 120.0 {
            return Err(Error("`fov` must be between 1 and 120 degrees".into()));
        }
        if self.distance.iter().any(|d| *d < 12.0) {
            return Err(Error("`distance` must be at least 12 cm (the phone is ~15 cm long)".into()));
        }
        if self.slats <= 0.2 {
            return Err(Error("`slats` (blind period, cm) must be > 0.2".into()));
        }
        self.dust = self.dust.min(4000);
        self.seeds = (0..self.dust)
            .map(|i| std::array::from_fn(|k| hash(i.wrapping_mul(0x9E37_79B9) ^ (k as u32).wrapping_mul(0x85EB_CA6B) ^ 0x51ED)))
            .collect();
        Ok(())
    }
}

/// Smooth start and stop with no jerk at either end (camera with weight).
fn weighty(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

fn rot_y(a: f32, v: V3) -> V3 {
    let (s, c) = a.sin_cos();
    [c * v[0] + s * v[2], v[1], -s * v[0] + c * v[2]]
}

fn rot_x(a: f32, v: V3) -> V3 {
    let (s, c) = a.sin_cos();
    [v[0], c * v[1] - s * v[2], s * v[1] + c * v[2]]
}

impl ProductShot {
    /// Props and time → scene (no pixels).
    pub fn scene(&self, t: f32, screen: Option<Arc<Pixmap>>, aspect: f32) -> Scene {
        let k = weighty(t / self.duration.max(0.01));
        let lerp2 = |v: [f32; 2]| mixf(v[0], v[1], k);
        let (az, el, dist) = (lerp2(self.azimuth).to_radians(), lerp2(self.elevation).to_radians(), lerp2(self.distance));

        // Phone: 71 × 147 × 8.3 mm, tilted towards the camera, hovering.
        let half = [3.55, 0.415, 7.33];
        let tilt = self.tilt.to_radians();
        let yaw = (self.yaw + self.turn * (k - 0.5)).to_radians();
        // Hovering devices breathe a little; a resting one doesn't.
        let bob = if self.hover > 0.0 { 0.09 * (t * std::f32::consts::TAU / 5.5).sin() } else { 0.0 };
        let lift = self.hover + half[2] * tilt.sin() + half[1] * tilt.cos() + bob;
        let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(|a| rot_y(yaw, rot_x(tilt, a)));
        let center = [0.0, lift, 0.0];
        let (metal, rough) = match self.finish.as_str() {
            "graphite" => ([0.20, 0.20, 0.21], 0.16),
            "silver" => ([0.80, 0.80, 0.80], 0.10),
            "gold" => ([0.86, 0.68, 0.45], 0.12),
            _ => ([0.56, 0.53, 0.49], 0.14),
        };
        let device = Device { center, axes, half, corner: 1.15, edge: 0.3, bezel: 0.17, metal, metal_rough: rough };

        // Camera: orbit around a target just above the phone, drifting with the move.
        let target = [0.6 - 1.2 * k, lift * 0.62, 0.4];
        let eye = add(target, [dist * el.cos() * az.sin(), dist * el.sin(), dist * el.cos() * az.cos()]);
        let camera = Camera { eye, target, fov: self.fov.to_radians(), focus: dot(sub(center, eye), norm(sub(target, eye))), aperture: self.aperture * 0.1 };

        let sa = self.sun_azimuth.to_radians();
        let se = self.sun_elevation.to_radians();
        let dir = norm([se.cos() * sa.sin(), se.sin(), se.cos() * sa.cos()]);
        let sun = Sun {
            dir,
            color: mul([1.0, 0.73, 0.47], 8.5 * self.sun),
            distance: 120.0,
            period: self.slats,
            open: 0.5,
            phase: self.drift * t,
            window: [-22.0, 20.0, 66.0, 101.0],
        };
        let set = Set {
            stone: self.stone_c,
            stone_rough: 0.34,
            edge_z: -19.0,
            edge_r: 0.7,
            floor_y: -78.0,
            wall: [0.16, 0.16, 0.17],
            ambient: [0.005, 0.0075, 0.013],
            haze: 0.0015 * self.haze,
        };

        // Dust: seeded positions drifting on slow, smooth currents.
        let motes = self
            .seeds
            .iter()
            .map(|s| {
                let base = [mixf(-22.0, 20.0, s[0]), mixf(0.5, 9.0, s[1]), mixf(-2.0, 22.0, s[2])];
                let w = 0.25 + 0.5 * s[3];
                let ph = s[4] * 40.0;
                let p = [
                    base[0] + 0.9 * t + 2.2 * (w * t * 0.7 + ph).sin(),
                    base[1] + 1.5 * (w * t * 0.5 + ph * 1.3).sin() - 0.25 * t,
                    base[2] + 2.0 * (w * t * 0.6 + ph * 0.7).cos(),
                ];
                let tw = 0.45 + 0.55 * (s[6] * 30.0 + t * (1.2 + 2.0 * s[7])).sin().abs().powf(3.0);
                Mote { p, size: 0.012 + 0.03 * s[5] * s[5], gain: tw }
            })
            .collect();
        let _ = aspect;
        Scene {
            camera,
            device,
            sun,
            set,
            motes,
            screen,
            screen_gain: 0.85 * self.screen_brightness,
            exposure: 1.35 * self.exposure,
            bloom: self.bloom,
            time: t,
        }
    }
}

impl Component for ProductShot {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width.round().max(2.0) as usize, ctx.height.round().max(2.0) as usize);
        let screen = self.screen.as_deref().and_then(|name| {
            let info = ctx.assets.composition_info(name)?;
            let ct = if info.looped && info.duration > 0.0 { t % info.duration } else { t.min(info.duration) };
            ctx.assets.composition_frame(name, ct, 0)
        });
        let scene = self.scene(t, screen.clone(), w as f32 / h as f32);
        let key = fnv(&[
            b"product-shot-v1",
            &(self as *const Self as usize).to_le_bytes(),
            &t.to_bits().to_le_bytes(),
            &[w as u32, h as u32].map(u32::to_le_bytes).concat(),
            &screen.as_ref().map(|p| Arc::as_ptr(p) as usize).unwrap_or(0).to_le_bytes(),
        ]);
        let image = ctx.assets.frame_image(key, w as u32, h as u32, &mut || Tracer.render(&scene, w, h));
        vec![Op::Image { image, dst: [0.0, 0.0, ctx.width, ctx.height], opacity: 1.0, transform: IDENTITY }]
    }
}

// ---------------------------------------------------------------- screen texture

/// Linear-light mip pyramid of the screen (RGB, planar per level).
struct Tex {
    levels: Vec<(usize, usize, Vec<[f32; 3]>)>,
}

fn srgb_lut() -> &'static [f32; 256] {
    static L: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    L.get_or_init(|| std::array::from_fn(|i| srgb_to_lin(i as f32 / 255.0)))
}

impl Tex {
    fn new(pm: &Pixmap) -> Tex {
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let lut = srgb_lut();
        let d = pm.data_as_u8_slice();
        // Premultiplied over black = the colour itself for an opaque screen.
        let base: Vec<[f32; 3]> = d.chunks_exact(4).map(|p| [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize]]).collect();
        let mut levels = vec![(w, h, base)];
        while levels.last().is_some_and(|l| l.0 > 1 || l.1 > 1) {
            let (pw, ph, prev) = levels.last().unwrap();
            let (nw, nh) = ((pw / 2).max(1), (ph / 2).max(1));
            let mut v = vec![[0.0f32; 3]; nw * nh];
            for y in 0..nh {
                let (y0, y1) = ((2 * y).min(ph - 1), (2 * y + 1).min(ph - 1));
                for x in 0..nw {
                    let (x0, x1) = ((2 * x).min(pw - 1), (2 * x + 1).min(pw - 1));
                    let (a, b, c, e) = (prev[y0 * pw + x0], prev[y0 * pw + x1], prev[y1 * pw + x0], prev[y1 * pw + x1]);
                    v[y * nw + x] = std::array::from_fn(|k| 0.25 * (a[k] + b[k] + c[k] + e[k]));
                }
            }
            levels.push((nw, nh, v));
        }
        Tex { levels }
    }

    #[inline]
    fn bilinear(&self, l: usize, u: f32, v: f32) -> V3 {
        let (w, h, d) = &self.levels[l];
        let x = (u * *w as f32 - 0.5).clamp(0.0, (*w - 1) as f32);
        let y = (v * *h as f32 - 0.5).clamp(0.0, (*h - 1) as f32);
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let (a, b, c, e) = (d[y0 * w + x0], d[y0 * w + x1], d[y1 * w + x0], d[y1 * w + x1]);
        std::array::from_fn(|k| mixf(mixf(a[k], b[k], fx), mixf(c[k], e[k], fx), fy))
    }

    /// Trilinear sample with up to 4 taps along the footprint's major axis.
    fn sample(&self, u: f32, v: f32, du: [f32; 2], dv: [f32; 2]) -> V3 {
        let (w, h) = (self.levels[0].0 as f32, self.levels[0].1 as f32);
        let ax = [du[0] * w, dv[0] * h];
        let ay = [du[1] * w, dv[1] * h];
        let lx = (ax[0] * ax[0] + ax[1] * ax[1]).sqrt();
        let ly = (ay[0] * ay[0] + ay[1] * ay[1]).sqrt();
        let (major, minor, md) = if lx > ly { (lx, ly, [du[0], dv[0]]) } else { (ly, lx, [du[1], dv[1]]) };
        let n = (major / minor.max(1e-6)).ceil().clamp(1.0, 4.0);
        let lod = (major / n).max(1e-6).log2().clamp(0.0, (self.levels.len() - 1) as f32);
        let l0 = lod.floor() as usize;
        let l1 = (l0 + 1).min(self.levels.len() - 1);
        let f = lod - l0 as f32;
        let mut acc = [0.0f32; 3];
        let taps = n as usize;
        for i in 0..taps {
            let o = (i as f32 + 0.5) / n - 0.5;
            let (uu, vv) = (u + md[0] * o, v + md[1] * o);
            let a = self.bilinear(l0, uu, vv);
            let b = if f > 0.0 { self.bilinear(l1, uu, vv) } else { a };
            for k in 0..3 {
                acc[k] += mixf(a[k], b[k], f);
            }
        }
        mul(acc, 1.0 / n)
    }
}

// ---------------------------------------------------------------- tracer

/// The built-in CPU renderer for [`Scene`].
pub struct Tracer;

/// What a ray hit (drives where supersampling is needed).
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Hit {
    Stone = 0,
    Nosing = 1,
    Room = 2,
    Screen = 3,
    Glass = 4,
    Metal = 5,
    /// Stone showing the phone's reflection.
    Mirror = 6,
}

/// Per-frame constants derived from the scene.
struct Frame<'a> {
    s: &'a Scene,
    tex: Option<&'a Tex>,
    right: V3,
    up: V3,
    fwd: V3,
    tan_y: f32,
    aspect: f32,
    /// Sun: horizontal direction towards the window, its tangent, tan(elevation).
    lh: V3,
    lt: V3,
    cos_e: f32,
    tan_e: f32,
    /// Screen half size in device-local units.
    scr: [f32; 2],
    /// Device-local bounding box (with margin).
    bound: V3,
    /// Pixel angle (radians per pixel).
    pix: f32,
}

#[inline(always)]
fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// Antiderivative of the open/closed slat pattern (open for the first `open` of each period).
#[inline(always)]
fn slat_integral(x: f32, open: f32) -> f32 {
    x.floor() * open + fract(x).min(open)
}

fn vnoise3(p: V3) -> f32 {
    let i = p.map(|v| v.floor());
    let f = [p[0] - i[0], p[1] - i[1], p[2] - i[2]].map(|v| v * v * (3.0 - 2.0 * v));
    let h = |x: f32, y: f32, z: f32| hash((x as i32 as u32).wrapping_mul(73_856_093) ^ (y as i32 as u32).wrapping_mul(19_349_663) ^ (z as i32 as u32).wrapping_mul(83_492_791));
    let (x, y, z) = (i[0], i[1], i[2]);
    let a = mixf(h(x, y, z), h(x + 1.0, y, z), f[0]);
    let b = mixf(h(x, y + 1.0, z), h(x + 1.0, y + 1.0, z), f[0]);
    let c = mixf(h(x, y, z + 1.0), h(x + 1.0, y, z + 1.0), f[0]);
    let d = mixf(h(x, y + 1.0, z + 1.0), h(x + 1.0, y + 1.0, z + 1.0), f[0]);
    mixf(mixf(a, b, f[1]), mixf(c, d, f[1]), f[2])
}

fn vnoise2(x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (fx, fy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let h = |x: f32, y: f32| hash((x as i32 as u32).wrapping_mul(73_856_093) ^ (y as i32 as u32).wrapping_mul(19_349_663) ^ 0x2545_F491);
    mixf(mixf(h(ix, iy), h(ix + 1.0, iy), fx), mixf(h(ix, iy + 1.0), h(ix + 1.0, iy + 1.0), fx), fy)
}

/// Rounded slab: plan corner radius `rc`, edge radius `re`.
#[inline(always)]
fn sd_phone(p: V3, half: V3, rc: f32, re: f32) -> f32 {
    let qx = p[0].abs() - (half[0] - rc);
    let qz = p[2].abs() - (half[2] - rc);
    let d2 = (qx.max(0.0).powi(2) + qz.max(0.0).powi(2)).sqrt() + qx.max(qz).min(0.0) - rc;
    let a = d2 + re;
    let b = p[1].abs() - (half[1] - re);
    (a.max(0.0).powi(2) + b.max(0.0).powi(2)).sqrt() + a.max(b).min(0.0) - re
}

/// Signed distance to a 2D rounded rect (half size `hx, hy`, radius `r`).
#[inline(always)]
fn sd_rrect(x: f32, y: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = x.abs() - (hx - r);
    let qy = y.abs() - (hy - r);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

fn ggx(n_h: f32, rough: f32) -> f32 {
    let a2 = (rough * rough).max(1e-4);
    let d = n_h * n_h * (a2 - 1.0) + 1.0;
    a2 / (std::f32::consts::PI * d * d)
}

fn schlick(cos: f32, f0: f32) -> f32 {
    f0 + (1.0 - f0) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
}

impl<'a> Frame<'a> {
    fn new(s: &'a Scene, tex: Option<&'a Tex>, w: usize, h: usize) -> Frame<'a> {
        let c = &s.camera;
        let fwd = norm(sub(c.target, c.eye));
        let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
        let up = cross(right, fwd);
        let tan_y = (c.fov * 0.5).tan();
        let l = s.sun.dir;
        let lh = norm([l[0], 0.0, l[2]]);
        let lt = [-lh[2], 0.0, lh[0]];
        let cos_e = (l[0] * l[0] + l[2] * l[2]).sqrt().max(1e-3);
        let d = &s.device;
        let inset = d.edge + d.bezel;
        let sx = d.half[0] - inset;
        let (sz, _) = match tex {
            Some(t) => (sx * t.levels[0].1 as f32 / t.levels[0].0 as f32, 0),
            None => (d.half[2] - inset, 0),
        };
        Frame {
            s,
            tex,
            right,
            up,
            fwd,
            tan_y,
            aspect: w as f32 / h as f32,
            lh,
            lt,
            cos_e,
            tan_e: l[1] / cos_e,
            scr: [sx, sz.min(d.half[2] - inset)],
            bound: add(d.half, [0.05, 0.05, 0.05]),
            pix: 2.0 * tan_y / h as f32,
        }
    }

    #[inline]
    fn ray(&self, px: f32, py: f32, h: usize) -> V3 {
        let sx = (2.0 * px / h as f32 - self.aspect) * self.tan_y;
        let sy = (1.0 - 2.0 * py / h as f32) * self.tan_y;
        norm(add(self.fwd, add(mul(self.right, sx), mul(self.up, sy))))
    }

    /// Where the sun ray from `p` crosses the window wall: (u, v, distance).
    #[inline]
    fn window_coords(&self, p: V3) -> (f32, f32, f32) {
        let s = (self.s.sun.distance - dot(p, self.lh)) / self.cos_e;
        let hit = mad(p, self.s.sun.dir, s);
        (dot(hit, self.lt), hit[1], s.max(0.0))
    }

    /// Fraction of the sun reaching `p` through the window and blinds
    /// (penumbra widens with distance from the window). `blur` adds extra width (cm).
    fn sun_mask(&self, p: V3, blur: f32) -> f32 {
        let sun = &self.s.sun;
        let (u, v, s) = self.window_coords(p);
        // The sun's disc (0.53°) plus a little slat-edge softness.
        let pen = s * 0.0095 + 0.12 + blur;
        let e = pen * 1.4;
        let win = smooth(sun.window[0] - e, sun.window[0] + e, u)
            * (1.0 - smooth(sun.window[1] - e, sun.window[1] + e, u))
            * smooth(sun.window[2] - e, sun.window[2] + e, v)
            * (1.0 - smooth(sun.window[3] - e, sun.window[3] + e, v));
        if win <= 0.0 {
            return 0.0;
        }
        win * blinds(v / sun.period + sun.phase, sun.open, pen / sun.period)
    }

    // ------------------------------------------------ device

    fn to_local(&self, p: V3) -> V3 {
        let d = &self.s.device;
        let q = sub(p, d.center);
        [dot(q, d.axes[0]), dot(q, d.axes[1]), dot(q, d.axes[2])]
    }

    fn dir_local(&self, v: V3) -> V3 {
        let d = &self.s.device;
        [dot(v, d.axes[0]), dot(v, d.axes[1]), dot(v, d.axes[2])]
    }

    fn dir_world(&self, v: V3) -> V3 {
        let d = &self.s.device;
        add(add(mul(d.axes[0], v[0]), mul(d.axes[1], v[1])), mul(d.axes[2], v[2]))
    }

    fn sd_local(&self, q: V3) -> f32 {
        let d = &self.s.device;
        sd_phone(q, d.half, d.corner, d.edge)
    }

    /// Local-space slab entry/exit of a ray, if it hits the box.
    fn box_range(&self, o: V3, rd: V3) -> Option<(f32, f32)> {
        let b = self.bound;
        let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
        for k in 0..3 {
            let inv = 1.0 / if rd[k].abs() < 1e-9 { 1e-9f32.copysign(rd[k]) } else { rd[k] };
            let (a, c) = ((-b[k] - o[k]) * inv, (b[k] - o[k]) * inv);
            t0 = t0.max(a.min(c));
            t1 = t1.min(a.max(c));
        }
        (t0 <= t1).then_some((t0, t1))
    }

    /// Ray march against the phone; returns hit distance (world = local, rigid).
    fn trace_device(&self, o: V3, rd: V3) -> Option<f32> {
        let (lo, ld) = (self.to_local(o), self.dir_local(rd));
        let (t0, t1) = self.box_range(lo, ld)?;
        let mut t = t0;
        for _ in 0..64 {
            let d = self.sd_local(mad(lo, ld, t));
            if d < 2e-4 * (1.0 + t * 0.02) {
                return Some(t);
            }
            t += d;
            if t > t1 {
                return None;
            }
        }
        None
    }

    /// Soft shadow of the phone towards the sun (1 = lit).
    fn device_shadow(&self, p: V3) -> f32 {
        let l = self.s.sun.dir;
        let (lo, ld) = (self.to_local(p), self.dir_local(l));
        let Some((t0, t1)) = self.box_range(lo, ld) else { return 1.0 };
        let mut res = 1.0f32;
        let mut t = t0.max(0.02);
        for _ in 0..40 {
            let d = self.sd_local(mad(lo, ld, t));
            res = res.min(18.0 * d / t);
            if res < 0.002 {
                return 0.0;
            }
            t += d.clamp(0.03, 1.0);
            if t > t1 {
                break;
            }
        }
        let r = clamp01(res);
        r * r * (3.0 - 2.0 * r)
    }

    /// Ambient occlusion of the plinth by the hovering phone.
    fn device_ao(&self, p: V3, n: V3) -> f32 {
        // Only near the body: beyond the farthest tap it can't occlude.
        if self.sd_local(self.to_local(p)) > 8.0 {
            return 1.0;
        }
        let mut occ = 0.0;
        let mut w = 1.0;
        for hgt in [0.8f32, 2.0, 3.6, 5.6, 8.0] {
            let d = self.sd_local(self.to_local(mad(p, n, hgt)));
            occ += w * (hgt - d).max(0.0) / hgt;
            w *= 0.8;
        }
        clamp01(1.0 - 0.33 * occ)
    }

    // ------------------------------------------------ environment

    /// Radiance seen in direction `d` from `o` (reflections): the dark room,
    /// the window with its slats, the sun, a dim cool bounce on the right.
    fn env(&self, o: V3, d: V3, rough: f32) -> V3 {
        let sun = &self.s.sun;
        let up = d[1];
        let warm = [1.0, 0.86, 0.72];
        let mut c = mul(warm, mixf(0.004, 0.018, smooth(-0.4, 0.7, up)));
        // Broad, cool bounce off a pale wall to camera-right (shape for the metal).
        let fill = norm([0.85, 0.35, 0.55]);
        let f = dot(d, fill).max(0.0);
        c = mad(c, [0.75, 0.85, 1.0], 0.05 * f * f * f);
        // A large pale card overhead and behind: the soft gradient on glass and frame.
        let card = norm([0.3, 0.8, -0.5]);
        let soft = 0.06 + rough * 0.5;
        let k = smooth(0.80 - soft, 0.94, dot(d, card));
        c = mad(c, [0.92, 0.95, 1.0], 0.55 * k * smooth(0.3, 0.15, rough));
        // The lit plinth below reflects a little warm light back up.
        c = mad(c, warm, 0.03 * smooth(0.1, -0.6, up));
        let dl = dot(d, self.lh);
        if dl > 0.02 {
            let s = (sun.distance - dot(o, self.lh)) / dl;
            let hit = mad(o, d, s);
            let (u, v) = (dot(hit, self.lt), hit[1]);
            let soft = 0.6 + rough * s * 0.9;
            let win = smooth(sun.window[0] - soft, sun.window[0] + soft, u)
                * (1.0 - smooth(sun.window[1] - soft, sun.window[1] + soft, u))
                * smooth(sun.window[2] - soft, sun.window[2] + soft, v)
                * (1.0 - smooth(sun.window[3] - soft, sun.window[3] + soft, v));
            if win > 0.0 {
                let gap = blinds(v / sun.period + sun.phase, sun.open, (0.15 + rough * s * 0.5) / sun.period);
                // Through the gaps: bright sky; the slats glow where sun hits them.
                let sky = [1.0, 0.82, 0.62];
                let slat = mul(warm, 0.35);
                c = add(c, mul(add(mul(sky, 3.2 * gap), mul(slat, 1.0 - gap)), win));
                // The sun disc itself (glints), widened by roughness.
                let cs = dot(d, sun.dir);
                let width = 0.00012 + rough * rough * 0.08;
                let g = ((cs - 1.0) / width).exp();
                c = mad(c, sun.color, 22.0 * g * gap * win / (1.0 + rough * 30.0));
            }
        }
        c
    }

    // ------------------------------------------------ shading

    /// Stone albedo variation (band-limited by the pixel footprint `fp`, cm).
    fn stone(&self, p: V3, fp: f32) -> (V3, f32) {
        let set = &self.s.set;
        let (x, z) = (p[0], p[2]);
        // Broad mottling and a faint directional grain.
        let m = vnoise2(x * 0.045 + 3.1, z * 0.06) * 0.6 + vnoise2(x * 0.13, z * 0.11 + 7.0) * 0.4;
        let g = vnoise2(x * 0.9 + z * 0.25, z * 0.12) - 0.5;
        // Fine speckle, faded out before it would alias.
        let fine_w = clamp01(0.35 / fp.max(1e-4) - 1.0);
        let fine = (vnoise2(x * 5.0, z * 5.0) - 0.5) * fine_w;
        let vfine_w = clamp01(0.09 / fp.max(1e-4) - 1.0);
        let vfine = (vnoise2(x * 19.0 + 11.0, z * 19.0) - 0.5) * vfine_w;
        let k = 0.78 + 0.42 * m + 0.12 * g + 0.22 * fine + 0.28 * vfine;
        let rough = set.stone_rough * (0.85 + 0.35 * m);
        (mul(set.stone, k), rough)
    }

    /// Radiance leaving a dielectric surface (stone, plaster).
    fn shade_diffuse(&self, p: V3, n: V3, v: V3, albedo: V3, rough: f32, fp: f32, occluded: bool) -> (V3, bool) {
        let s = self.s;
        let l = s.sun.dir;
        let nl = dot(n, l).max(0.0);
        let mut lit = if nl > 0.0 { self.sun_mask(p, fp * 0.5) } else { 0.0 };
        if lit > 0.0 && occluded {
            lit *= self.device_shadow(p);
        }
        let ao = if occluded { self.device_ao(p, n) } else { 1.0 };
        let mut c = hadamard(albedo, add(mul(s.set.ambient, ao), mul(s.sun.color, nl * lit)));
        // Glossy: sun highlight (through the blinds) and the room.
        let h = norm(add(l, v));
        let nv = dot(n, v).max(1e-3);
        let f = schlick(dot(h, v).max(0.0), 0.04);
        let spec = ggx(dot(n, h).max(0.0), rough) * f * nl * lit / (4.0 * nv * nl.max(1e-3)).max(0.2) * nl;
        c = mad(c, s.sun.color, spec);
        let r = reflect(mul(v, -1.0), n);
        let fe = schlick(nv, 0.04) * (1.0 - rough * 0.6);
        c = mad(c, self.env(p, r, rough), fe * ao);
        (c, false)
    }

    /// Shade the device at world point `p` (local `q`), viewed along `rd`.
    fn shade_device(&self, p: V3, q: V3, rd: V3, fp: f32) -> (V3, Hit) {
        let s = self.s;
        let dev = &s.device;
        // Normal from the distance field (tetrahedron taps), in local space.
        let e = 0.002;
        let k = [[1.0, -1.0, -1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, 1.0f32]];
        let mut nl = [0.0f32; 3];
        for kk in k {
            let d = self.sd_local(mad(q, kk, e));
            nl = mad(nl, kk, d);
        }
        let nl = norm(nl);
        let n = self.dir_world(nl);
        let v = mul(rd, -1.0);
        let nv = dot(n, v).max(1e-3);
        let l = s.sun.dir;
        let ndl = dot(n, l).max(0.0);
        let lit = if ndl > 0.0 { self.sun_mask(p, 0.0) } else { 0.0 };
        let r = reflect(rd, n);

        // Glass where the face is flat; metal on the rounded band.
        let glass = smooth(0.94, 0.995, nl[1]);
        let mut c = [0.0f32; 3];
        let mut hit = Hit::Metal;
        if glass > 0.0 {
            let sd = sd_rrect(q[0], q[2], self.scr[0], self.scr[1], (dev.corner - dev.edge - dev.bezel).max(0.2));
            let on = 1.0 - smooth(-fp, fp, sd);
            let mut emit = [0.0f32; 3];
            if on > 0.0 {
                if let Some(tex) = self.tex {
                    let u = (q[0] / self.scr[0]) * 0.5 + 0.5;
                    let vv = (q[2] / self.scr[1]) * 0.5 + 0.5;
                    // Footprint in uv: the pixel's size on the glass, stretched by obliqueness.
                    let du = fp / (2.0 * self.scr[0]);
                    let dv = fp / (2.0 * self.scr[1]);
                    // Major axis along the view direction projected on the glass.
                    let vd = self.dir_local(rd);
                    let pl = (vd[0] * vd[0] + vd[2] * vd[2]).sqrt().max(1e-4);
                    let stretch = 1.0 / nv.max(0.08);
                    let (ax, az) = (vd[0] / pl, vd[2] / pl);
                    let du_x = [du * ax * stretch, -du * az];
                    let dv_x = [dv * az * stretch, dv * ax];
                    emit = mul(tex.sample(u, vv, du_x, dv_x), s.screen_gain * on);
                } else {
                    emit = mul([0.02, 0.022, 0.03], on);
                }
            }
            hit = if on > 0.5 { Hit::Screen } else { Hit::Glass };
            let fr = schlick(nv, 0.04);
            // Black glass and the panel scatter a little sunlight: the blinds show on the screen.
            let diffuse = mul(s.sun.color, ndl * lit * 0.007);
            let g = add(add(mul(emit, 1.0 - fr), diffuse), mul(self.env(p, r, 0.015), fr));
            c = mad(c, g, glass);
        }
        if glass < 1.0 {
            let f0 = dev.metal;
            let fw = schlick(nv, 0.0);
            let spec_col = mix3(f0, [1.0, 1.0, 1.0], fw);
            let rough = dev.metal_rough;
            let env = self.env(p, r, rough);
            let h = norm(add(l, v));
            let sun_spec = ggx(dot(n, h).max(0.0), rough) * ndl * lit * 0.25;
            let mut m = hadamard(add(env, mul(s.sun.color, sun_spec)), spec_col);
            // A little diffuse from the anodised/blasted finish.
            m = add(m, hadamard(mul(f0, 0.06), add(s.set.ambient, mul(s.sun.color, ndl * lit))));
            c = mad(c, m, 1.0 - glass);
        }
        (c, hit)
    }

    /// One camera ray: radiance, what it hit, hit distance.
    fn trace(&self, rd: V3, fp_k: f32) -> (V3, Hit, f32) {
        let s = self.s;
        let o = s.camera.eye;
        let mut col;
        let hit;
        let mut tmax;
        let v = mul(rd, -1.0);
        if let Some(t) = self.trace_device(o, rd) {
            let p = mad(o, rd, t);
            let q = self.to_local(p);
            let (c, h) = self.shade_device(p, q, rd, t * self.pix * fp_k);
            col = c;
            hit = h;
            tmax = t;
        } else {
            // Plinth top (y = 0, z > edge), its rounded nosing, then the room.
            let set = &s.set;
            let ez = set.edge_z + set.edge_r;
            let mut done = None;
            if rd[1] < 0.0 {
                let t = -o[1] / rd[1];
                let p = mad(o, rd, t);
                if p[2] >= ez {
                    done = Some((t, p, [0.0, 1.0, 0.0], Hit::Stone));
                } else {
                    // Cylinder along x at (y = -r, z = ez).
                    let (oy, oz) = (o[1] + set.edge_r, o[2] - ez);
                    let (dy, dz) = (rd[1], rd[2]);
                    let a = dy * dy + dz * dz;
                    let b = oy * dy + oz * dz;
                    let c = oy * oy + oz * oz - set.edge_r * set.edge_r;
                    let disc = b * b - a * c;
                    if disc >= 0.0 {
                        let tc = (-b - disc.sqrt()) / a;
                        let pc = mad(o, rd, tc);
                        if tc > 0.0 && pc[2] <= ez && pc[1] >= -set.edge_r {
                            let n = norm([0.0, pc[1] + set.edge_r, pc[2] - ez]);
                            done = Some((tc, pc, n, Hit::Nosing));
                        }
                    }
                }
            }
            if let Some((t, p, n, h)) = done {
                let fp = t * self.pix * fp_k / dot(n, v).max(0.15);
                let (alb, rough) = self.stone(p, fp);
                let (c, mirror) = self.shade_diffuse(p, n, v, alb, rough, fp, true);
                col = c;
                hit = if mirror { Hit::Mirror } else { h };
                tmax = t;
            } else {
                // Room: the window wall (facing the plinth) and the floor.
                let tw = {
                    let dl = dot(rd, self.lh);
                    if dl > 1e-4 { (s.sun.distance - dot(o, self.lh)) / dl } else { f32::INFINITY }
                };
                let tf = if rd[1] < 0.0 { (set.floor_y - o[1]) / rd[1] } else { f32::INFINITY };
                let (t, n) = if tw < tf { (tw, mul(self.lh, -1.0)) } else { (tf, [0.0, 1.0, 0.0]) };
                tmax = t.min(2000.0);
                let p = mad(o, rd, tmax);
                // Plaster: low-frequency unevenness only (it sits far out of focus).
                let k = 0.9 + 0.2 * vnoise3(mul(p, 0.02));
                let alb = if n[1] > 0.5 { mul([0.09, 0.075, 0.06], k) } else { mul(set.wall, k) };
                // Light bounced off the sunlit plinth: warm, falling off with height and distance.
                let lift = (-(p[1] - 6.0).abs() / 42.0).exp() * (-dot(p, p).sqrt() / 260.0).exp();
                let bounce = mul([1.0, 0.8, 0.6], 0.012 * lift);
                col = hadamard(alb, add(set.ambient, bounce));
                if n[1] > 0.5 {
                    col = mad(col, hadamard(alb, s.sun.color), self.sun_mask(p, 6.0) * dot(n, s.sun.dir).max(0.0));
                }
                hit = Hit::Room;
            }
        }
        // Light shafts: in-scatter integrated analytically along the ray.
        col = add(col, self.shafts(o, rd, tmax));
        tmax = tmax.max(0.0);
        (col, hit, tmax)
    }

    /// Sunlight scattered towards the camera by haze along `o + rd·[0, tmax]`.
    /// The slat coordinate is linear along the ray, so the integral is exact.
    fn shafts(&self, o: V3, rd: V3, tmax: f32) -> V3 {
        let s = self.s;
        let sun = &s.sun;
        if s.set.haze <= 0.0 {
            return [0.0; 3];
        }
        // Window coordinates along the ray: u(t) = u0 + du t, v(t) = v0 + dv t.
        let (u0, v0, _) = self.window_coords(o);
        let du = dot(rd, self.lt);
        let dv = rd[1] - dot(rd, self.lh) * self.tan_e;
        let mut total = 0.0;
        for soft in [-1.6f32, 0.0, 1.6] {
            let wnd = [sun.window[0] - soft, sun.window[1] + soft, sun.window[2] - soft, sun.window[3] + soft];
            let (mut a, mut b) = (0.0f32, tmax);
            for (p0, dp, lo, hi) in [(u0, du, wnd[0], wnd[1]), (v0, dv, wnd[2], wnd[3])] {
                if dp.abs() < 1e-6 {
                    if p0 < lo || p0 > hi {
                        b = -1.0;
                    }
                } else {
                    let (ta, tb) = ((lo - p0) / dp, (hi - p0) / dp);
                    a = a.max(ta.min(tb));
                    b = b.min(ta.max(tb));
                }
            }
            // Only above the plinth top or beyond its edge.
            if b <= a {
                continue;
            }
            let ph = sun.phase;
            let (x0, x1) = ((v0 + dv * a) / sun.period + ph, (v0 + dv * b) / sun.period + ph);
            let open = if (x1 - x0).abs() < 1e-4 {
                blinds(x0, sun.open, 0.05) * (b - a)
            } else {
                let w = 0.05;
                let f = |x: f32| (slat_integral(x + w, sun.open) + slat_integral(x, sun.open) + slat_integral(x - w, sun.open)) / 3.0;
                (f(x1) - f(x0)) / (x1 - x0) * (b - a)
            };
            total += open / 3.0;
        }
        if total <= 0.0 {
            return [0.0; 3];
        }
        // Henyey-Greenstein, forward scattering (looking towards the window glows).
        let g = 0.62f32;
        let cos = dot(rd, sun.dir);
        let hg = (1.0 - g * g) / (4.0 * std::f32::consts::PI * (1.0 + g * g - 2.0 * g * cos).max(1e-4).powf(1.5));
        // Slow drift in the air's density.
        let mid = mad(o, rd, tmax.min(120.0) * 0.5);
        let dens = 0.75 + 0.5 * vnoise3(add(mul(mid, 0.05), [s.time * 0.05, 0.0, s.time * 0.03]));
        mul(sun.color, s.set.haze * total * hg * dens)
    }
}

/// Slat pattern at `x` (periods): 1 in the open fraction, with edges `w` periods wide.
#[inline]
fn blinds(x: f32, open: f32, w: f32) -> f32 {
    let c = fract(x - open * 0.5 + 0.5) - 0.5;
    let dd = c.abs();
    let w = w.max(1e-4);
    let m = clamp01((open * 0.5 - dd) / w + 0.5);
    mixf(m, open, clamp01((w - 0.25) / 0.5))
}

/// Narkowicz's ACES fit, per channel (no hue-skewing matrices).
#[inline(always)]
fn tonemap(x: f32) -> f32 {
    let x = x.max(0.0);
    clamp01(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14))
}

const ENCODE_N: usize = 4096;

/// sRGB encoding of tone-mapped values (linear interpolation in a fine table).
fn encode_lut() -> &'static [f32; ENCODE_N] {
    static L: std::sync::OnceLock<[f32; ENCODE_N]> = std::sync::OnceLock::new();
    L.get_or_init(|| std::array::from_fn(|i| to_srgb(i as f32 / (ENCODE_N - 1) as f32)))
}

fn to_srgb(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

thread_local! {
    static TEX: std::cell::RefCell<Option<TexEntry>> = const { std::cell::RefCell::new(None) };
}

fn screen_tex(pm: &Arc<Pixmap>) -> Arc<Tex> {
    let key = Arc::as_ptr(pm) as usize;
    TEX.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((k, keep, t)) = c.as_ref() {
            if *k == key && Arc::ptr_eq(keep, pm) {
                return t.clone();
            }
        }
        let t = Arc::new(Tex::new(pm));
        *c = Some((key, pm.clone(), t.clone()));
        t
    })
}

/// Rotated-grid 4× pattern (pixel offsets).
const RGSS: [[f32; 2]; 4] = [[0.125, -0.375], [0.375, 0.125], [-0.125, 0.375], [-0.375, -0.125]];

impl ShotRenderer for Tracer {
    fn render(&self, scene: &Scene, w: usize, h: usize) -> Vec<u8> {
        let tex = scene.screen.as_ref().map(screen_tex);
        let f = Frame::new(scene, tex.as_deref(), w, h);
        let n = w * h;
        let mut rgb = vec![[0.0f32; 3]; n];
        let mut ids = vec![0u8; n];
        let mut depth = vec![0.0f32; n];
        // Pass 1: one ray per pixel centre.
        for y in 0..h {
            for x in 0..w {
                let rd = f.ray(x as f32 + 0.5, y as f32 + 0.5, h);
                let (c, id, t) = f.trace(rd, 1.0);
                let i = y * w + x;
                rgb[i] = c;
                ids[i] = id as u8;
                depth[i] = t * dot(rd, f.fwd);
            }
        }
        // Pass 2: supersample where geometry or material changes, and on metal and nosing.
        let mut redo = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let id = ids[i];
                let mut need = id == Hit::Metal as u8 || id == Hit::Nosing as u8;
                if !need {
                    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && ids[ny as usize * w + nx as usize] != id {
                            need = true;
                            break;
                        }
                    }
                }
                if need {
                    redo.push(i);
                }
            }
        }
        for &i in &redo {
            let (x, y) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            let mut acc = [0.0f32; 3];
            for o in RGSS {
                let (c, _, _) = f.trace(f.ray(x + o[0], y + o[1], h), 0.5);
                acc = add(acc, c);
            }
            rgb[i] = mul(acc, 0.25);
        }
        // Dust: depth-tested soft discs, lit only inside the beams; defocused off the focal plane.
        let cam = &scene.camera;
        let focal_px = h as f32 * 0.5 / f.tan_y;
        for m in &scene.motes {
            let rel = sub(m.p, cam.eye);
            let z = dot(rel, f.fwd);
            if z < 2.0 {
                continue;
            }
            let lit = f.sun_mask(m.p, 0.0);
            if lit <= 0.002 {
                continue;
            }
            // Brightness: forward scattering off a flake.
            let rd = norm(rel);
            let cos = dot(rd, scene.sun.dir);
            let g = 0.55f32;
            let hg = (1.0 - g * g) / (1.0 + g * g - 2.0 * g * cos).powf(1.5);
            let sx = dot(rel, f.right) / z * focal_px + w as f32 * 0.5;
            let sy = -dot(rel, f.up) / z * focal_px + h as f32 * 0.5;
            let coc = (cam.aperture * focal_px * (z - cam.focus).abs() / (z * cam.focus.max(1.0))).max(0.0);
            let core = (m.size * focal_px / z).max(0.0);
            let r = (coc * 0.5).max(core).max(0.6);
            // Same light whatever the blur: a defocused mote spreads it over its disc.
            // (Spread a little less than energy-conserving: near motes stay legible as discs.)
            let energy = lit * m.gain * hg * (m.size / 0.03).powi(2) * 40.0;
            let peak = 0.6 * energy / (std::f32::consts::PI * r * r).max(1.0).powf(0.55);
            let col = mul(hadamard(scene.sun.color, [1.0, 0.86, 0.66]), peak * 0.05);
            let (x0, x1) = ((sx - r - 1.5).floor().max(0.0) as i32, (sx + r + 1.5).ceil().min(w as f32 - 1.0) as i32);
            let (y0, y1) = ((sy - r - 1.5).floor().max(0.0) as i32, (sy + r + 1.5).ceil().min(h as f32 - 1.0) as i32);
            for py in y0..=y1 {
                for px in x0..=x1 {
                    let i = py as usize * w + px as usize;
                    if depth[i] < z {
                        continue;
                    }
                    let d = ((px as f32 + 0.5 - sx).powi(2) + (py as f32 + 0.5 - sy).powi(2)).sqrt();
                    // Soft disc (bokeh) that becomes a soft point when small.
                    let a = 1.0 - smooth(r * 0.55 - 0.4, r + 0.6, d);
                    if a > 0.0 {
                        rgb[i] = mad(rgb[i], col, a);
                    }
                }
            }
        }
        // Bloom from real highlights: a soft, wide halo from what exceeds 1.0.
        if scene.bloom > 0.0 {
            bloom(&mut rgb, w, h, scene.bloom * 0.06, scene.exposure);
        }
        // Tone map, encode, dither.
        let mut out = vec![0u8; n * 4];
        let ex = scene.exposure;
        let lut = encode_lut();
        let (cx, cy, rr) = (w as f32 * 0.5, h as f32 * 0.5, 1.0 / (w as f32 * w as f32 * 0.25 + h as f32 * h as f32 * 0.25));
        for (i, (px, c)) in out.chunks_exact_mut(4).zip(rgb.iter()).enumerate() {
            // Natural lens falloff towards the corners.
            let (dx, dy) = ((i % w) as f32 + 0.5 - cx, (i / w) as f32 + 0.5 - cy);
            let fall = 1.0 / (1.0 + 0.32 * (dx * dx + dy * dy) * rr).powi(2);
            let c = &mul(*c, fall);
            let dth = hash(i as u32 ^ 0xA511_E9B3) - hash((i as u32).wrapping_mul(0x9E37_79B9) ^ 0x1B87_3593);
            for k in 0..3 {
                let tm = tonemap(c[k] * ex) * (ENCODE_N - 1) as f32;
                let j = (tm as usize).min(ENCODE_N - 2);
                let enc = mixf(lut[j], lut[j + 1], tm - j as f32);
                let v = enc * 255.0 + dth * 0.75 + 0.5;
                px[k] = v.clamp(0.0, 255.0) as u8;
            }
            px[3] = 255;
        }
        out
    }
}

/// Add a wide soft glow of everything brighter than the white point.
fn bloom(rgb: &mut [[f32; 3]], w: usize, h: usize, k: f32, ex: f32) {
    // Quarter resolution bright pass.
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let mut b = vec![[0.0f32; 3]; bw * bh];
    for y in 0..h {
        for x in 0..w {
            let c = rgb[y * w + x];
            let l = (c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722) * ex;
            let over = (l - 0.9).max(0.0) / l.max(1e-4);
            let t = &mut b[(y / 4) * bw + x / 4];
            *t = mad(*t, c, over / 16.0);
        }
    }
    // Three box passes each way ≈ gaussian.
    let mut tmp = vec![[0.0f32; 3]; bw * bh];
    for r in [6usize, 6, 6] {
        box_h(&b, &mut tmp, bw, bh, r);
        box_v(&tmp, &mut b, bw, bh, r);
    }
    for y in 0..h {
        let fy = (y as f32 + 0.5) / 4.0 - 0.5;
        let y0 = (fy.floor().max(0.0) as usize).min(bh - 1);
        let y1 = (y0 + 1).min(bh - 1);
        let ty = clamp01(fy - y0 as f32);
        for x in 0..w {
            let fx = (x as f32 + 0.5) / 4.0 - 0.5;
            let x0 = (fx.floor().max(0.0) as usize).min(bw - 1);
            let x1 = (x0 + 1).min(bw - 1);
            let tx = clamp01(fx - x0 as f32);
            let a = mix3(b[y0 * bw + x0], b[y0 * bw + x1], tx);
            let c = mix3(b[y1 * bw + x0], b[y1 * bw + x1], tx);
            let i = y * w + x;
            rgb[i] = mad(rgb[i], mix3(a, c, ty), k * 16.0);
        }
    }
}

fn box_h(src: &[[f32; 3]], dst: &mut [[f32; 3]], w: usize, h: usize, r: usize) {
    let norm_k = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let mut acc = [0.0f32; 3];
        for i in 0..=r {
            acc = add(acc, row[i.min(w - 1)]);
        }
        acc = add(acc, mul(row[0], r as f32));
        for x in 0..w {
            dst[y * w + x] = mul(acc, norm_k);
            let add_i = (x + r + 1).min(w - 1);
            let rem_i = x.saturating_sub(r);
            acc = sub(add(acc, row[add_i]), row[rem_i]);
        }
    }
}

fn box_v(src: &[[f32; 3]], dst: &mut [[f32; 3]], w: usize, h: usize, r: usize) {
    let norm_k = 1.0 / (2 * r + 1) as f32;
    for x in 0..w {
        let mut acc = [0.0f32; 3];
        for i in 0..=r {
            acc = add(acc, src[i.min(h - 1) * w + x]);
        }
        acc = add(acc, mul(src[x], r as f32));
        for y in 0..h {
            dst[y * w + x] = mul(acc, norm_k);
            let a = (y + r + 1).min(h - 1);
            let rm = y.saturating_sub(r);
            acc = sub(add(acc, src[a * w + x]), src[rm * w + x]);
        }
    }
}

/// One cached screen texture: (key, source pixmap, mip chain).
type TexEntry = (usize, Arc<Pixmap>, Arc<Tex>);

#[cfg(test)]
mod tests {
    use super::*;

    fn shot() -> ProductShot {
        let mut p: ProductShot = serde_json::from_str("{}").unwrap();
        p.prepare().unwrap();
        p
    }

    #[test]
    fn renders_a_lit_frame() {
        let p = shot();
        let s = p.scene(1.0, None, 16.0 / 9.0);
        let px = Tracer.render(&s, 160, 90);
        assert_eq!(px.len(), 160 * 90 * 4);
        let mean = px.chunks_exact(4).map(|c| c[0] as f32).sum::<f32>() / (160.0 * 90.0);
        assert!(mean > 4.0 && mean < 200.0, "mean {mean}");
    }

    #[test]
    fn camera_move_starts_and_ends_at_rest() {
        let p = shot();
        let e = |t: f32| p.scene(t, None, 1.0).camera.eye;
        let v0 = sub(e(0.01), e(0.0));
        let v1 = sub(e(p.duration), e(p.duration - 0.01));
        assert!(dot(v0, v0).sqrt() < 1e-3 && dot(v1, v1).sqrt() < 1e-3);
        let mid = sub(e(p.duration * 0.5 + 0.01), e(p.duration * 0.5));
        assert!(dot(mid, mid).sqrt() > 1e-3);
    }

    #[test]
    fn slat_integral_matches_mean() {
        let open = 0.56;
        let x = 37.25;
        assert!(((slat_integral(x + 10.0, open) - slat_integral(x, open)) / 10.0 - open).abs() < 1e-4);
    }

    #[test]
    fn rejects_bad_props() {
        let mut p: ProductShot = serde_json::from_str(r#"{"finish":"chrome"}"#).unwrap();
        assert!(p.prepare().is_err());
        assert!(serde_json::from_str::<ProductShot>(r#"{"fob":30}"#).is_err());
    }
}
