//! Extruded 3D text and logos: glyph outlines (or a polygon / SVG path)
//! extruded with a rounded bevel, lit like a studio product shot and
//! animated in letter by letter.
//!
//! Geometry is built once in [`Prepare`]: every letter becomes its own mesh
//! (front and back caps triangulated with their holes, a quarter-round bevel
//! and smooth side walls), centred on itself so it can rise or fly in on its
//! own. The caps carry a gentle vertical normal tilt across the whole word,
//! so a flat face reflects a sweep of the studio environment (the chrome
//! "horizon line") instead of one flat colour.
//!
//! Each frame the letters are drawn by `anikit-core`'s rasteriser into an
//! image covering only their on-screen bounds, 2× supersampled, in parallel
//! bands, with coverage as alpha (transparent background). Images are keyed
//! by content and frame-quantised time.

use super::style::*;
use super::{rgba, Component, Ctx, Error, Prepare};
use crate::dl::{FillRule, Op, Rgba};
use crate::text::{FontStore, TextSpec, TextStyle};
use anikit_core::raster::{Lighting, Material, Mesh, Raster, M4};
use anikit_core::space;
use serde::Deserialize;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Vertical field of view (degrees): a long lens, like a title shot.
const FOV: f32 = 24.0;
/// Largest side of the rendered image (before supersampling).
const MAX_SIDE: f32 = 2400.0;
/// Segments in each quarter-round bevel.
const BEVEL_SEGS: usize = 4;
const INTROS: &[&str] = &["rise", "fly-in", "drop", "none"];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Text3d {
    /// The text (`"ANIKIT"` when neither `points` nor `path` is given). `\n` breaks lines.
    pub text: Option<String>,
    /// A logo outline `[[x, y], ...]` (y down) instead of text.
    pub points: Vec<[f32; 2]>,
    /// An SVG path (`M L H V C S Q T Z`, absolute or relative) instead of text;
    /// holes come from nested sub-paths.
    pub path: Option<String>,
    /// Font family (`Inter`, `serif`, `mono`, ...).
    pub font: String,
    pub weight: f32,
    /// Letter spacing in em.
    pub tracking: f32,
    /// Extrusion depth and bevel size, in em (fractions of the font size).
    pub depth: f32,
    pub bevel: f32,
    /// Base colour (defaults to chrome tinted with the theme accent).
    pub color: Option<Color>,
    /// Colours spread across the letters, left to right (overrides `color`).
    pub gradient: Vec<Color>,
    pub metallic: f32,
    pub roughness: f32,
    /// Turntable speed (degrees per second) and starting angle.
    pub spin: f32,
    pub angle: f32,
    /// Tilt towards the camera (degrees): shows the tops of the letters.
    pub tilt: f32,
    /// Idle sway amplitude (degrees), so highlights glide across the faces.
    pub sway: f32,
    /// Entrance: `rise`, `fly-in`, `drop` or `none`.
    pub intro: String,
    /// Seconds between letters during the entrance.
    pub stagger: f32,
    /// Seconds before the entrance starts.
    pub delay: f32,
    /// Size multiplier (1 fits the text to the box with a margin).
    pub scale: f32,
    /// Strength of the sheen that sweeps across the letters (0 = off).
    pub shine: f32,
    /// Soft shadow under the text (0 = none).
    pub shadow: f32,
    /// Render scale (1 = native box pixels).
    pub resolution: f32,
    #[serde(skip)]
    letters: Vec<Letter>,
    /// Word bounds `[x0, y0, x1, y1]` in em, y up, centred on the origin.
    #[serde(skip)]
    bounds: [f32; 4],
    #[serde(skip)]
    hz: f32,
    #[serde(skip)]
    key: u64,
}

/// One letter (or the whole logo): a mesh centred on `center`.
struct Letter {
    mesh: Mesh,
    center: [f32; 3],
    /// Local bounds (min, max) of the mesh.
    lo: [f32; 3],
    hi: [f32; 3],
    /// 0..1 across the word, for gradients.
    u: f32,
}

impl Default for Text3d {
    fn default() -> Self {
        Text3d {
            text: None,
            points: Vec::new(),
            path: None,
            font: "Inter".into(),
            weight: 800.0,
            tracking: 0.03,
            depth: 0.2,
            bevel: 0.028,
            color: None,
            gradient: Vec::new(),
            metallic: 0.85,
            roughness: 0.22,
            spin: 0.0,
            angle: -10.0,
            tilt: 8.0,
            sway: 7.0,
            intro: "rise".into(),
            stagger: 0.07,
            delay: 0.0,
            scale: 1.0,
            shine: 1.0,
            shadow: 0.35,
            resolution: 1.0,
            letters: Vec::new(),
            bounds: [0.0; 4],
            hz: 0.1,
            key: 0,
        }
    }
}

fn fonts() -> &'static FontStore {
    static F: std::sync::OnceLock<FontStore> = std::sync::OnceLock::new();
    F.get_or_init(FontStore::new)
}

impl Prepare for Text3d {
    fn prepare(&mut self) -> Result<(), Error> {
        for (name, v) in [("depth", self.depth), ("bevel", self.bevel), ("metallic", self.metallic), ("roughness", self.roughness), ("scale", self.scale), ("resolution", self.resolution), ("shadow", self.shadow), ("shine", self.shine), ("stagger", self.stagger), ("weight", self.weight)] {
            if !v.is_finite() || v < 0.0 {
                return Err(Error(format!("`{name}` must be a non-negative number, got {v}")));
            }
        }
        for (name, v) in [("spin", self.spin), ("angle", self.angle), ("tilt", self.tilt), ("sway", self.sway), ("delay", self.delay), ("tracking", self.tracking)] {
            if !v.is_finite() {
                return Err(Error(format!("`{name}` must be a finite number")));
            }
        }
        self.intro = super::kebab(&self.intro);
        if !INTROS.contains(&self.intro.as_str()) {
            return Err(Error(match closest(&self.intro, INTROS) {
                Some(c) => format!("unknown intro `{}`; did you mean `{c}`?", self.intro),
                None => format!("unknown intro `{}`; expected one of {}", self.intro, INTROS.join(", ")),
            }));
        }
        // Outlines, as letters of contours in em units (y up).
        let shapes: Vec<Vec<Vec<[f32; 2]>>> = if let Some(p) = &self.path {
            vec![normalise_logo(parse_svg_path(p).map_err(Error)?)?]
        } else if !self.points.is_empty() {
            vec![normalise_logo(vec![self.points.iter().map(|p| [p[0], -p[1]]).collect()])?]
        } else {
            let text = self.text.clone().unwrap_or_else(|| "ANIKIT".into());
            if text.trim().is_empty() {
                return Err(Error("`text` is empty".into()));
            }
            glyph_outlines(&text, &self.font, self.weight, self.tracking)
        };
        let mut shapes: Vec<Vec<Vec<[f32; 2]>>> = shapes
            .into_iter()
            .map(|cs| cs.into_iter().filter_map(clean).collect::<Vec<_>>())
            .filter(|cs: &Vec<Vec<[f32; 2]>>| !cs.is_empty())
            .collect();
        if shapes.is_empty() {
            return Err(Error("nothing to extrude (no visible outlines)".into()));
        }
        // Centre the word.
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for p in shapes.iter().flatten().flatten() {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let c = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
        for p in shapes.iter_mut().flatten().flatten() {
            p[0] -= c[0];
            p[1] -= c[1];
        }
        let (hw, hh) = ((hi[0] - lo[0]) * 0.5, (hi[1] - lo[1]) * 0.5);
        self.bounds = [-hw, -hh, hw, hh];
        let hz = (self.depth * 0.5).max(0.005);
        self.hz = hz;
        let bevel = self.bevel.min(hz * 0.9).min(0.12);
        // Front-face normal tilt across the word: a convex "pillow" so flat faces
        // sweep through the environment's horizon instead of reading as one colour.
        let pillow = 0.6 / hh.max(0.05);
        self.letters = shapes
            .iter()
            .map(|cs| {
                let mut m = extrude(cs, hz, bevel, pillow);
                let (mut a, mut b) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                for p in m.pos.chunks_exact(3) {
                    for k in 0..3 {
                        a[k] = a[k].min(p[k]);
                        b[k] = b[k].max(p[k]);
                    }
                }
                let center = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, 0.0];
                for p in m.pos.chunks_exact_mut(3) {
                    p[0] -= center[0];
                    p[1] -= center[1];
                }
                let lo = [a[0] - center[0], a[1] - center[1], a[2]];
                let hi = [b[0] - center[0], b[1] - center[1], b[2]];
                Letter { mesh: m, center, lo, hi, u: clamp01((center[0] + hw) / (2.0 * hw).max(1e-6)) }
            })
            .filter(|l| !l.mesh.idx.is_empty())
            .collect();
        if self.letters.is_empty() {
            return Err(Error("nothing to extrude (outlines have no area)".into()));
        }
        let mut h = Vec::new();
        h.extend_from_slice(self.text.as_deref().unwrap_or("").as_bytes());
        h.extend_from_slice(self.path.as_deref().unwrap_or("").as_bytes());
        h.extend_from_slice(self.font.as_bytes());
        h.extend_from_slice(self.intro.as_bytes());
        for v in self.points.iter().flatten().chain(&[
            self.weight,
            self.tracking,
            self.depth,
            self.bevel,
            self.metallic,
            self.roughness,
            self.spin,
            self.angle,
            self.tilt,
            self.sway,
            self.stagger,
            self.delay,
            self.scale,
            self.shine,
            self.resolution,
        ]) {
            h.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        for c in &self.gradient {
            h.extend(c.0 .0.iter().flat_map(|v| v.to_bits().to_le_bytes()));
        }
        self.key = fnv(&[b"text-3d", &h]);
        Ok(())
    }
}

type M3 = [[f32; 3]; 3];

fn mm(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|j| std::array::from_fn(|i| a[0][i] * b[j][0] + a[1][i] * b[j][1] + a[2][i] * b[j][2]))
}
fn mv(a: &M3, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[0][i] * v[0] + a[1][i] * v[1] + a[2][i] * v[2])
}
fn rot_x(a: f32) -> M3 {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]]
}
fn rot_y(a: f32) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]]
}
fn rot_z(a: f32) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
}

impl Text3d {
    /// When the last letter has landed.
    fn intro_end(&self) -> f32 {
        if self.intro == "none" {
            0.45
        } else {
            (self.letters.len().saturating_sub(1)) as f32 * self.stagger + 1.1
        }
    }

    /// Letter `i`'s entrance at `local` seconds: (offset in em, rotation, scale).
    fn letter_motion(&self, i: usize, local: f32) -> ([f32; 3], M3, f32) {
        let p = local - i as f32 * self.stagger;
        let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let hh = self.bounds[3];
        match self.intro.as_str() {
            "rise" => {
                let e = spring(p, 0.75);
                let s = ease_out(seg(p, 0.0, 0.35));
                let lift = ease_out(seg(p, 0.0, 0.9));
                ([0.0, -(1.0 - lift) * hh * 1.6, 0.0], rot_x((1.0 - e) * 80f32.to_radians()), s)
            }
            "fly-in" => {
                let e = ease_out(seg(p, 0.0, 1.0));
                let r = spring(p, 0.6);
                let side = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
                let s = ease_out(seg(p, 0.0, 0.3));
                ([(1.0 - e) * side * hh * 2.5, (1.0 - e) * hh * 0.8, -(1.0 - e) * hh * 14.0], mm(&rot_y((1.0 - r) * side * 140f32.to_radians()), &rot_z((1.0 - r) * side * 0.4)), s)
            }
            "drop" => {
                let e = spring(p, 0.8);
                let s = ease_out(seg(p, 0.0, 0.2));
                ([0.0, (1.0 - e) * hh * 3.0, 0.0], rot_z((1.0 - e) * 0.35), s)
            }
            _ => ([0.0; 3], id, 1.0),
        }
    }

    fn colors(&self, ctx: &Ctx) -> Vec<Rgba> {
        let base = self.color.map(|c| c.0).unwrap_or_else(|| {
            // Chrome with a whisper of the accent.
            mix(rgba(0.86, 0.88, 0.93, 1.0), ctx.theme.accent, 0.18)
        });
        self.letters
            .iter()
            .map(|l| match self.gradient.len() {
                0 => base,
                1 => self.gradient[0].0,
                n => {
                    let x = l.u * (n - 1) as f32;
                    let k = (x.floor() as usize).min(n - 2);
                    mix(self.gradient[k].0, self.gradient[k + 1].0, x - k as f32)
                }
            })
            .collect()
    }

    /// Material and studio lighting: warm key upper-left, cool fill, cool
    /// fresnel rim, a chrome environment (dark ground, bright horizon,
    /// softbox strips).
    fn look(&self, dark: bool) -> (Material, Lighting) {
        let rough = self.roughness.clamp(0.0, 1.0);
        let metal = self.metallic.clamp(0.0, 1.0);
        let gloss = 1.0 - rough;
        let mat = Material {
            ambient: 0.14 + 0.06 * rough,
            diffuse: 0.95 - 0.5 * metal,
            specular: (0.3 + 0.9 * gloss * gloss) * (1.0 + 0.5 * metal),
            shininess: 8.0 + 220.0 * gloss * gloss * gloss,
            rim: 0.6,
            metal: (metal * (0.4 + 0.6 * gloss)).min(0.92),
            fog: 0.0,
            ..Material::default()
        };
        let floor = if dark { 0.025 } else { 0.12 };
        let light = Lighting {
            key: [-0.5, 0.75, 0.65],
            key_color: [1.1, 1.04, 0.96],
            fill: [0.85, 0.1, 0.45],
            fill_color: [0.28, 0.31, 0.4],
            rim_color: [0.75, 0.85, 1.05],
            sky: [0.34, 0.4, 0.56],
            horizon: [1.45, 1.4, 1.34],
            ground: [floor, floor, floor * 1.15],
            strips: [1.3, 1.3, 1.36],
            ..Lighting::default()
        };
        (mat, light)
    }
}

impl Component for Text3d {
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.intro_end())
    }

    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        // Pose updates at most 30×/s during the intro and 15×/s once only the
        // slow sway and glint move: frames in between reuse the same render
        // (CPU raster is the cost here).
        let settled = t - self.delay > self.intro_end() + 0.2 && self.spin == 0.0;
        let fps = ctx.fps.clamp(1.0, if settled { 15.0 } else { 30.0 });
        let frame = (t * fps).round();
        let t = frame / fps;
        let local = t - self.delay;
        if local < 0.0 || ctx.width < 2.0 || ctx.height < 2.0 || self.scale <= 0.0 {
            return Vec::new();
        }
        let (w, h) = (ctx.width, ctx.height);
        let aspect = w / h;
        let half = (FOV * 0.5).to_radians().tan();
        let [bx0, by0, bx1, by1] = self.bounds;
        let (hw, hh) = ((bx1 - bx0) * 0.5, (by1 - by0) * 0.5);
        // Frame the word: 84 % of the width or 56 % of the height.
        let view_h = (hw / (0.84 * aspect)).max(hh / 0.56);
        let dist = view_h / half + self.hz;
        let eye = [0.0, 0.0, dist];
        let view = space::look_at([0.0, 0.0, dist as f64], [0.0; 3], [0.0, 1.0, 0.0]);
        let proj = space::perspective(FOV as f64, aspect as f64, (dist * 0.05) as f64, (dist * 4.0) as f64);
        let vp_full = space::mul(&proj, &view);
        let to_px = |p: [f32; 3]| -> Option<(f32, f32, f32)> {
            let m = &vp_full;
            let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
            let cw = m[3] * x + m[7] * y + m[11] * z + m[15];
            if cw <= 1e-6 {
                return None;
            }
            let cx = (m[0] * x + m[4] * y + m[8] * z + m[12]) / cw;
            let cy = (m[1] * x + m[5] * y + m[9] * z + m[13]) / cw;
            Some(((cx as f32 + 1.0) * 0.5 * w, (1.0 - cy as f32) * 0.5 * h, cw as f32))
        };

        let end = self.intro_end();
        let fade = if self.intro == "none" { ease_out(seg(local, 0.0, 0.45)) } else { 1.0 };
        // Word rotation: settle from a little extra yaw, then sway gently.
        let settle = (1.0 - ease_out(seg(local, 0.0, end + 0.6))) * 12.0;
        let yaw = (self.angle + settle + self.spin * t + self.sway * (local * TAU / 7.0).sin()).to_radians();
        let pitch = (self.tilt + 1.5 * (local * TAU / 9.0).cos()).to_radians();
        let rw = mm(&rot_x(pitch), &rot_y(yaw));
        let sw = self.scale;

        let mut out = Vec::new();
        // Soft shadow under the text (vector).
        if self.shadow > 0.0 {
            let landed = ease_out(seg(local, 0.0, end));
            if let (Some((sx, sy, d)), true) = (to_px([0.0, by0 * sw - 0.22 * hh, 0.0]), landed > 0.01) {
                let focal = h * 0.5 / half;
                let rx = hw * sw * focal / d * 1.05;
                let ry = rx * 0.09;
                let a = self.shadow * landed * fade * if ctx.theme.dark { 1.0 } else { 0.5 };
                out.push(Op::Fill {
                    path: circle_path(0.0, 0.0, 1.0),
                    paint: radial(0.0, 0.0, 1.0, vec![(0.0, rgba(0.0, 0.0, 0.0, a)), (0.5, rgba(0.0, 0.0, 0.0, a * 0.45)), (1.0, rgba(0.0, 0.0, 0.0, 0.0))]),
                    rule: FillRule::NonZero,
                    transform: [rx, 0.0, 0.0, ry, sx, sy],
                });
            }
        }

        // Per-letter instances.
        let colors = self.colors(ctx);
        let mut insts: Vec<(usize, [f32; 15])> = Vec::with_capacity(self.letters.len());
        let mut lo = [f32::INFINITY; 2];
        let mut hi = [f32::NEG_INFINITY; 2];
        let mut whole = false;
        for (i, l) in self.letters.iter().enumerate() {
            let (off, rl, s) = self.letter_motion(i, local);
            if s <= 0.002 {
                continue;
            }
            let m = mm(&rw, &rl);
            let m: M3 = m.map(|c| c.map(|v| v * s * sw));
            let tr = mv(&rw, [(l.center[0] + off[0]) * sw, (l.center[1] + off[1]) * sw, (l.center[2] + off[2]) * sw]);
            let c = colors[i].0;
            insts.push((i, [m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2], tr[0], tr[1], tr[2], c[0], c[1], c[2]]));
            for k in 0..8 {
                let p = [if k & 1 == 0 { l.lo[0] } else { l.hi[0] }, if k & 2 == 0 { l.lo[1] } else { l.hi[1] }, if k & 4 == 0 { l.lo[2] } else { l.hi[2] }];
                let q = mv(&m, p);
                match to_px([q[0] + tr[0], q[1] + tr[1], q[2] + tr[2]]) {
                    Some((x, y, _)) => {
                        lo = [lo[0].min(x), lo[1].min(y)];
                        hi = [hi[0].max(x), hi[1].max(y)];
                    }
                    None => whole = true,
                }
            }
        }
        if insts.is_empty() {
            return out;
        }
        if whole {
            lo = [0.0, 0.0];
            hi = [w, h];
        }
        let x0 = (lo[0] - 2.0).floor().clamp(0.0, w);
        let y0 = (lo[1] - 2.0).floor().clamp(0.0, h);
        let x1 = (hi[0] + 2.0).ceil().clamp(0.0, w.floor());
        let y1 = (hi[1] + 2.0).ceil().clamp(0.0, h.floor());
        if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
            return out;
        }
        let (rw_px, rh_px) = (x1 - x0, y1 - y0);
        let res = self.resolution.max(0.05).min(MAX_SIDE / rw_px.max(rh_px));
        let iw = (rw_px * res).round().max(1.0) as u32;
        let ih = (rh_px * res).round().max(1.0) as u32;
        let (cxn, hxn) = ((x0 + x1) / w - 1.0, rw_px / w);
        let (cyn, hyn) = (1.0 - (y0 + y1) / h, rh_px / h);
        let mut sub = [0.0f64; 16];
        sub[0] = 1.0 / hxn as f64;
        sub[5] = 1.0 / hyn as f64;
        sub[10] = 1.0;
        sub[15] = 1.0;
        sub[12] = -(cxn / hxn) as f64;
        sub[13] = -(cyn / hyn) as f64;
        let m = space::mul(&sub, &vp_full);
        let vp: M4 = std::array::from_fn(|i| m[i] as f32);

        // A sheen sweeps across the letters after the entrance, then every 6 s:
        // a slanted band (in image pixels) brightening what's under it.
        let sheen = if self.shine > 0.0 {
            let ph = (local - end * 0.7) / 6.0;
            let f = ph - ph.floor();
            (ph >= 0.0 && f < 0.2).then(|| {
                let (fw, fh) = (iw as f32, ih as f32);
                let band = fw.max(fh) * 0.05 + fh * 0.12;
                let c = lerp(-band * 2.0, fw + fh * 0.4 + band * 2.0, ease_in_out(f / 0.2));
                [c, band, 0.55 * self.shine.min(2.0)]
            })
        } else {
            None
        };
        let (mat, light) = self.look(ctx.theme.dark);
        let key = fnv(&[
            &self.key.to_le_bytes(),
            &frame.to_bits().to_le_bytes(),
            &fps.to_bits().to_le_bytes(),
            &[w.to_bits(), h.to_bits(), x0.to_bits(), y0.to_bits(), iw, ih].map(u32::to_le_bytes).concat(),
            &colors.iter().flat_map(|c| c.0.map(f32::to_bits)).flat_map(u32::to_le_bytes).collect::<Vec<u8>>(),
            &[ctx.theme.dark as u8],
        ]);
        let letters = &self.letters;
        let draws: Vec<(&Mesh, [f32; 15])> = insts.iter().map(|(i, inst)| (&letters[*i].mesh, *inst)).collect();
        let image = ctx.assets.image(key, iw, ih, &mut || render(&draws, &vp, eye, &mat, &light, sheen, iw as usize, ih as usize));
        out.push(Op::Image { image, dst: [x0, y0, rw_px, rh_px], opacity: fade, transform: crate::dl::IDENTITY });
        out
    }
}

/// Render meshes at 2× and box-filter down to straight RGBA; alpha is
/// coverage. Horizontal bands render in parallel, each with its own
/// off-centre projection. `sheen` (`[centre, half width, strength]`, image
/// pixels) adds a slanted highlight band, strongest on bright pixels.
pub(crate) fn render(draws: &[(&Mesh, [f32; 15])], vp: &M4, eye: [f32; 3], mat: &Material, light: &Lighting, sheen: Option<[f32; 3]>, w: usize, h: usize) -> Vec<u8> {
    use rayon::prelude::*;
    const SS: usize = 2;
    let band = h.div_ceil(rayon::current_num_threads().max(1) * 3).max(24);
    let mut out = vec![0u8; w * h * 4];
    let body = |(k, dst): (usize, &mut [u8])| {
        let rows = dst.len() / (w * 4);
        let (y0, y1) = ((k * band) as f32, (k * band + rows) as f32);
        let (cy, hy) = (1.0 - (y0 + y1) / h as f32, (y1 - y0) / h as f32);
        let mut bvp = *vp;
        for c in 0..4 {
            bvp[4 * c + 1] = (vp[4 * c + 1] - cy * vp[4 * c + 3]) / hy;
        }
        let (sw, sh) = (w * SS, rows * SS);
        let mut r = Raster::new(sw, sh);
        r.clear(0, 0);
        for (mesh, inst) in draws {
            r.draw(mesh, inst, &bvp, eye, mat, light, true);
        }
        for y in 0..rows {
            for x in 0..w {
                let mut acc = [0u32; 3];
                let mut n = 0u32;
                for dy in 0..SS {
                    let row = (y * SS + dy) * sw + x * SS;
                    for i in row..row + SS {
                        if r.depth[i].is_finite() {
                            let c = r.color[i];
                            acc[0] += c & 255;
                            acc[1] += (c >> 8) & 255;
                            acc[2] += (c >> 16) & 255;
                            n += 1;
                        }
                    }
                }
                if n == 0 {
                    continue;
                }
                let o = (y * w + x) * 4;
                let mut c = [(acc[0] + n / 2) / n, (acc[1] + n / 2) / n, (acc[2] + n / 2) / n];
                if let Some([cx, hw, k]) = sheen {
                    // Slanted 20°: the band leans right going up.
                    let d = (x as f32 + (y0 as usize + y) as f32 * 0.36 - cx) / hw;
                    if d.abs() < 3.0 {
                        let g = (-d * d).exp() + 0.5 * (-(d * 3.0 + 4.5).powi(2)).exp();
                        let lum = (c[0] + c[1] + c[2]) as f32 / 765.0;
                        let add = 255.0 * k * g * (0.25 + 0.75 * lum);
                        c = c.map(|v| (v as f32 + add).min(255.0) as u32);
                    }
                }
                dst[o] = c[0] as u8;
                dst[o + 1] = c[1] as u8;
                dst[o + 2] = c[2] as u8;
                dst[o + 3] = ((n * 255 + (SS * SS) as u32 / 2) / (SS * SS) as u32) as u8;
            }
        }
    };
    // Inside a rayon worker (frame-parallel render) run serially: nested
    // par_iter lets rayon steal another frame job onto this thread while the
    // caller holds per-thread state (RefCell/locks), which panicked or hung.
    if rayon::current_thread_index().is_some() {
        out.chunks_mut(band * w * 4).enumerate().for_each(body);
    } else {
        out.par_chunks_mut(band * w * 4).enumerate().for_each(body);
    }
    out
}

// ---------------------------------------------------------------- outlines

/// Collects flattened contours from curve commands.
struct Flatten {
    contours: Vec<Vec<[f32; 2]>>,
    cur: Vec<[f32; 2]>,
    /// Maps outline coordinates to output: `p * k + o` (y flipped when `k[1] < 0`).
    k: [f32; 2],
    o: [f32; 2],
    tol: f32,
}

impl Flatten {
    fn new(k: [f32; 2], o: [f32; 2], tol: f32) -> Self {
        Flatten { contours: Vec::new(), cur: Vec::new(), k, o, tol }
    }
    fn map(&self, x: f32, y: f32) -> [f32; 2] {
        [x * self.k[0] + self.o[0], y * self.k[1] + self.o[1]]
    }
    fn last(&self) -> [f32; 2] {
        *self.cur.last().unwrap_or(&[0.0, 0.0])
    }
    fn flush(&mut self) {
        if self.cur.len() >= 3 {
            self.contours.push(std::mem::take(&mut self.cur));
        }
        self.cur.clear();
    }
    fn move_to(&mut self, p: [f32; 2]) {
        self.flush();
        self.cur.push(p);
    }
    fn line_to(&mut self, p: [f32; 2]) {
        self.cur.push(p);
    }
    fn quad_to(&mut self, c: [f32; 2], p: [f32; 2]) {
        let a = self.last();
        let dev = ((a[0] - 2.0 * c[0] + p[0]).hypot(a[1] - 2.0 * c[1] + p[1])) * 0.25;
        let n = (dev / self.tol).sqrt().ceil().clamp(1.0, 32.0) as usize;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            self.cur.push([u * u * a[0] + 2.0 * u * t * c[0] + t * t * p[0], u * u * a[1] + 2.0 * u * t * c[1] + t * t * p[1]]);
        }
    }
    fn curve_to(&mut self, c1: [f32; 2], c2: [f32; 2], p: [f32; 2]) {
        let a = self.last();
        let d1 = (a[0] - 2.0 * c1[0] + c2[0]).hypot(a[1] - 2.0 * c1[1] + c2[1]);
        let d2 = (c1[0] - 2.0 * c2[0] + p[0]).hypot(c1[1] - 2.0 * c2[1] + p[1]);
        let dev = d1.max(d2) * 0.75;
        let n = (dev / self.tol).sqrt().ceil().clamp(1.0, 40.0) as usize;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.cur.push([b0 * a[0] + b1 * c1[0] + b2 * c2[0] + b3 * p[0], b0 * a[1] + b1 * c1[1] + b2 * c2[1] + b3 * p[1]]);
        }
    }
}

impl skrifa::outline::OutlinePen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        Flatten::move_to(self, p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        Flatten::line_to(self, p);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.map(cx0, cy0), self.map(x, y));
        Flatten::quad_to(self, c, p);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (c1, c2, p) = (self.map(cx0, cy0), self.map(cx1, cy1), self.map(x, y));
        Flatten::curve_to(self, c1, c2, p);
    }
    fn close(&mut self) {
        self.flush();
    }
}

/// Lay `text` out with the bundled fonts and return each visible character's
/// contours in em units (y up).
pub(crate) fn glyph_outlines(text: &str, font: &str, weight: f32, tracking: f32) -> Vec<Vec<Vec<[f32; 2]>>> {
    use skrifa::instance::{LocationRef, NormalizedCoord, Size};
    use skrifa::outline::DrawSettings;
    use skrifa::MetadataProvider;
    const SIZE: f32 = 100.0;
    let store = fonts();
    let style = TextStyle { font: font.to_string(), weight, size: SIZE, letter_spacing: tracking, line_height: 1.05, ..TextStyle::default() };
    let layout = store.layout(&TextSpec::plain(text, style));
    let mut out = Vec::new();
    for ch in &layout.chars {
        let mut fl = Flatten::new([1.0 / SIZE, 1.0 / SIZE], [0.0, 0.0], 0.0006);
        for gi in ch.glyphs.clone() {
            let (run, g) = layout.glyph(gi);
            let face = store.face(run.font);
            let Ok(f) = skrifa::FontRef::from_index(face.data.data.as_ref(), face.data.index) else { continue };
            let coords: Vec<NormalizedCoord> = face.coords.iter().map(|c| NormalizedCoord::from_bits(*c)).collect();
            let Some(outline) = f.outline_glyphs().get(skrifa::GlyphId::new(g.id as u32)) else { continue };
            // Layout is y down from the block's top; outlines are y up from the baseline.
            fl.o = [g.x / SIZE, -g.y / SIZE];
            let _ = outline.draw(DrawSettings::unhinted(Size::new(run.size), LocationRef::new(&coords)), &mut fl);
            fl.flush();
        }
        if !fl.contours.is_empty() {
            out.push(fl.contours);
        }
    }
    out
}

/// Scale a logo's contours (any units, y up) to about one em tall.
pub(crate) fn normalise_logo(cs: Vec<Vec<[f32; 2]>>) -> Result<Vec<Vec<[f32; 2]>>, Error> {
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for p in cs.iter().flatten() {
        if !p[0].is_finite() || !p[1].is_finite() {
            return Err(Error("logo coordinates must be finite".into()));
        }
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    if cs.iter().all(|c| c.len() < 3) || span.is_nan() || span <= 1e-9 {
        return Err(Error("`points`/`path` needs at least three points enclosing an area".into()));
    }
    let k = 1.0 / span;
    Ok(cs.into_iter().map(|c| c.into_iter().map(|p| [p[0] * k, p[1] * k]).collect()).collect())
}

/// Minimal SVG path parser (y down in, y up out): M L H V C S Q T Z, upper
/// and lower case. Arcs are not supported.
pub(crate) fn parse_svg_path(d: &str) -> Result<Vec<Vec<[f32; 2]>>, String> {
    let mut toks: Vec<Result<char, f32>> = Vec::new();
    let b = d.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() {
            if "MLHVCSQTZmlhvcsqtz".contains(c) {
                toks.push(Ok(c));
                i += 1;
            } else {
                return Err(format!("path: unsupported command `{c}`"));
            }
        } else if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' {
            let s = i;
            i += 1;
            let mut dot = c == '.';
            while i < b.len() {
                let d = b[i] as char;
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == '.' && !dot {
                    dot = true;
                    i += 1;
                } else if (d == 'e' || d == 'E') && i + 1 < b.len() {
                    i += 1;
                    if b[i] == b'-' || b[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            toks.push(Err(d[s..i].parse::<f32>().map_err(|_| format!("path: bad number `{}`", &d[s..i]))?));
        } else {
            i += 1;
        }
    }
    let mut fl = Flatten::new([1.0, -1.0], [0.0, 0.0], 0.0);
    let mut start = [0.0f32; 2];
    let mut cur = [0.0f32; 2];
    let mut last_c: Option<[f32; 2]> = None;
    let mut cmd = 'M';
    let mut k = 0;
    // First pass for the tolerance: 1/2000 of the coordinate span.
    let nums: Vec<f32> = toks.iter().filter_map(|t| t.err()).collect();
    let span = nums.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    fl.tol = (span / 2000.0).max(1e-6);
    let take = |k: &mut usize, n: usize| -> Result<Vec<f32>, String> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            match toks.get(*k) {
                Some(Err(x)) => {
                    v.push(*x);
                    *k += 1;
                }
                _ => return Err(format!("path: a command is missing numbers (needs {n})")),
            }
        }
        Ok(v)
    };
    let y = |p: [f32; 2]| [p[0], -p[1]];
    while k < toks.len() {
        if let Ok(c) = toks[k] {
            cmd = c;
            k += 1;
            if c == 'Z' || c == 'z' {
                fl.flush();
                cur = start;
                last_c = None;
                continue;
            }
        } else if k == 0 {
            return Err("path must start with a command".into());
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { [0.0, 0.0] };
        let pt = |v: &[f32], i: usize| [base[0] + v[i], base[1] + v[i + 1]];
        match cmd.to_ascii_uppercase() {
            'M' => {
                let v = take(&mut k, 2)?;
                cur = pt(&v, 0);
                start = cur;
                fl.move_to(y(cur));
                // Further pairs are implicit line-tos.
                cmd = if rel { 'l' } else { 'L' };
                last_c = None;
            }
            'L' => {
                let v = take(&mut k, 2)?;
                cur = pt(&v, 0);
                fl.line_to(y(cur));
                last_c = None;
            }
            'H' => {
                let v = take(&mut k, 1)?;
                cur[0] = if rel { cur[0] + v[0] } else { v[0] };
                fl.line_to(y(cur));
                last_c = None;
            }
            'V' => {
                let v = take(&mut k, 1)?;
                cur[1] = if rel { cur[1] + v[0] } else { v[0] };
                fl.line_to(y(cur));
                last_c = None;
            }
            'C' | 'S' => {
                let s = cmd.eq_ignore_ascii_case(&'S');
                let v = take(&mut k, if s { 4 } else { 6 })?;
                let (c1, c2, p) = if s {
                    let c1 = last_c.map(|c| [2.0 * cur[0] - c[0], 2.0 * cur[1] - c[1]]).unwrap_or(cur);
                    (c1, pt(&v, 0), pt(&v, 2))
                } else {
                    (pt(&v, 0), pt(&v, 2), pt(&v, 4))
                };
                if fl.cur.is_empty() {
                    fl.move_to(y(cur));
                }
                fl.curve_to(y(c1), y(c2), y(p));
                last_c = Some(c2);
                cur = p;
            }
            'Q' | 'T' => {
                let t = cmd.eq_ignore_ascii_case(&'T');
                let v = take(&mut k, if t { 2 } else { 4 })?;
                let (c, p) = if t {
                    (last_c.map(|c| [2.0 * cur[0] - c[0], 2.0 * cur[1] - c[1]]).unwrap_or(cur), pt(&v, 0))
                } else {
                    (pt(&v, 0), pt(&v, 2))
                };
                if fl.cur.is_empty() {
                    fl.move_to(y(cur));
                }
                fl.quad_to(y(c), y(p));
                last_c = Some(c);
                cur = p;
            }
            _ => unreachable!(),
        }
    }
    fl.flush();
    if fl.contours.is_empty() {
        return Err("path has no closed shape with three or more points".into());
    }
    Ok(fl.contours)
}

fn area2(p: &[[f32; 2]]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a
}

/// Drop duplicate and collinear points; `None` when nothing with area is left.
pub(crate) fn clean(mut c: Vec<[f32; 2]>) -> Option<Vec<[f32; 2]>> {
    let eps = 1e-4;
    c.dedup_by(|a, b| (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps);
    while c.len() > 1 && {
        let (a, b) = (c[0], c[c.len() - 1]);
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps
    } {
        c.pop();
    }
    let mut changed = true;
    while changed && c.len() > 3 {
        changed = false;
        let n = c.len();
        let mut keep = vec![true; n];
        for i in 0..n {
            let (a, b, d) = (c[(i + n - 1) % n], c[i], c[(i + 1) % n]);
            let (e1, e2) = ([b[0] - a[0], b[1] - a[1]], [d[0] - b[0], d[1] - b[1]]);
            let cr = e1[0] * e2[1] - e1[1] * e2[0];
            let l = e1[0].hypot(e1[1]) * e2[0].hypot(e2[1]);
            if cr.abs() <= 1e-4 * l && e1[0] * e2[0] + e1[1] * e2[1] > 0.0 && keep[(i + n - 1) % n] {
                keep[i] = false;
                changed = true;
            }
        }
        let mut k = 0;
        c.retain(|_| {
            k += 1;
            keep[k - 1]
        });
    }
    (c.len() >= 3 && area2(&c).abs() > 1e-7).then_some(c)
}

fn inside(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

/// An outline (counter-clockwise) and its holes (clockwise).
type Group = (Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>);
/// An edge: endpoints, ring, index in the ring, ring length.
type Seg = ([f32; 2], [f32; 2], usize, usize, usize);

/// Group contours into outers (counter-clockwise) with their holes (clockwise),
/// by how many other contours *fully* contain each one. Fonts often overlap
/// contours (the crossbar of an `A`); those only partly overlap, so they stay
/// solid and are extruded on their own.
fn group(cs: &[Vec<[f32; 2]>]) -> Vec<Group> {
    let n = cs.len();
    let areas: Vec<f32> = cs.iter().map(|c| area2(c).abs()).collect();
    let within = |i: usize, j: usize| -> bool {
        let step = (cs[i].len() / 24).max(1);
        let c = &cs[i];
        // Vertices and edge midpoints (a crossbar's corners can all sit inside the legs).
        j != i && areas[j] > areas[i] && (0..c.len()).step_by(step).all(|k| {
            let (a, b) = (c[k], c[(k + 1) % c.len()]);
            inside(a, &cs[j]) && inside([(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5], &cs[j])
        })
    };
    let depth: Vec<usize> = (0..n).map(|i| (0..n).filter(|&j| within(i, j)).count()).collect();
    let mut groups: Vec<Group> = Vec::new();
    let mut gi = vec![usize::MAX; n];
    for i in (0..n).filter(|&i| depth[i].is_multiple_of(2)) {
        let mut c = cs[i].clone();
        if area2(&c) < 0.0 {
            c.reverse();
        }
        gi[i] = groups.len();
        groups.push((c, Vec::new()));
    }
    for i in (0..n).filter(|&i| depth[i] % 2 == 1) {
        // Parent: the smallest containing outer one level up.
        let parent = (0..n)
            .filter(|&j| depth[j] + 1 == depth[i] && within(i, j))
            .min_by(|&a, &b| areas[a].total_cmp(&areas[b]));
        if let Some(p) = parent {
            let mut c = cs[i].clone();
            if area2(&c) > 0.0 {
                c.reverse();
            }
            groups[gi[p]].1.push(c);
        }
    }
    groups
}

fn cross3(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Triangulate a counter-clockwise polygon with clockwise holes (ear
/// clipping after bridging each hole to the outline). Returns indices into
/// `outer ++ holes`.
fn triangulate(outer: &[[f32; 2]], holes: &[Vec<[f32; 2]>]) -> Vec<u32> {
    let mut pts: Vec<[f32; 2]> = outer.to_vec();
    let mut poly: Vec<u32> = (0..outer.len() as u32).collect();
    let mut hs: Vec<(u32, usize)> = Vec::new(); // (start index, len)
    for h in holes {
        hs.push((pts.len() as u32, h.len()));
        pts.extend_from_slice(h);
    }
    // Bridge holes, rightmost first.
    let maxx = |&(s, n): &(u32, usize)| (s..s + n as u32).map(|i| pts[i as usize][0]).fold(f32::NEG_INFINITY, f32::max);
    hs.sort_by(|a, b| maxx(b).total_cmp(&maxx(a)));
    for (s, n) in hs {
        let hm = (s..s + n as u32).max_by(|&a, &b| pts[a as usize][0].total_cmp(&pts[b as usize][0])).unwrap();
        let m = pts[hm as usize];
        // Nearest edge hit by a ray to +x.
        let mut best: Option<(f32, usize)> = None;
        for k in 0..poly.len() {
            let (a, b) = (pts[poly[k] as usize], pts[poly[(k + 1) % poly.len()] as usize]);
            if (a[1] > m[1]) == (b[1] > m[1]) || (a[1] - b[1]).abs() < 1e-12 {
                continue;
            }
            let x = a[0] + (m[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if x >= m[0] && best.is_none_or(|(bx, _)| x < bx) {
                best = Some((x, k));
            }
        }
        let Some((ix, k)) = best else { continue };
        let (ka, kb) = (k, (k + 1) % poly.len());
        let mut pk = if pts[poly[ka] as usize][0] > pts[poly[kb] as usize][0] { ka } else { kb };
        // Any polygon vertex inside the triangle (M, I, P) blocks the view: take
        // the one closest in angle to the ray.
        let p = pts[poly[pk] as usize];
        let i = [ix, m[1]];
        let (t0, t1, t2) = if cross3(m, i, p) >= 0.0 { (m, i, p) } else { (m, p, i) };
        let mut best_a = f32::INFINITY;
        for (q, &vi) in poly.iter().enumerate() {
            let v = pts[vi as usize];
            if v == p || v[0] < m[0] {
                continue;
            }
            if cross3(t0, t1, v) >= 0.0 && cross3(t1, t2, v) >= 0.0 && cross3(t2, t0, v) >= 0.0 {
                let a = (v[1] - m[1]).abs() / (v[0] - m[0]).max(1e-12);
                if a < best_a {
                    best_a = a;
                    pk = q;
                }
            }
        }
        let mut merged = Vec::with_capacity(poly.len() + n + 2);
        merged.extend_from_slice(&poly[..=pk]);
        let off = hm - s;
        for j in 0..=n as u32 {
            merged.push(s + (off + j) % n as u32);
        }
        merged.push(poly[pk]);
        merged.extend_from_slice(&poly[pk + 1..]);
        poly = merged;
    }
    ear_clip(&pts, poly)
}

fn ear_clip(pts: &[[f32; 2]], mut idx: Vec<u32>) -> Vec<u32> {
    let mut out = Vec::with_capacity(idx.len() * 3);
    let p = |i: u32| pts[i as usize];
    let mut i = 0usize;
    let mut misses = 0usize;
    while idx.len() > 3 {
        let n = idx.len();
        i %= n;
        let (a, b, c) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
        let (pa, pb, pc) = (p(a), p(b), p(c));
        let mut ear = cross3(pa, pb, pc) > 1e-12;
        if ear {
            for &j in &idx {
                let q = p(j);
                if q == pa || q == pb || q == pc {
                    continue;
                }
                if cross3(pa, pb, q) >= 0.0 && cross3(pb, pc, q) >= 0.0 && cross3(pc, pa, q) >= 0.0 {
                    ear = false;
                    break;
                }
            }
        }
        if ear {
            out.extend_from_slice(&[a, b, c]);
            idx.remove(i);
            i = (i + n - 2) % (n - 1);
            misses = 0;
        } else {
            i += 1;
            misses += 1;
            if misses > n {
                // Degenerate (collinear or self-touching): drop the flattest vertex.
                let k = (0..n)
                    .min_by(|&x, &y| {
                        let f = |k: usize| cross3(p(idx[(k + n - 1) % n]), p(idx[k]), p(idx[(k + 1) % n])).abs();
                        f(x).total_cmp(&f(y))
                    })
                    .unwrap();
                idx.remove(k);
                misses = 0;
            }
        }
    }
    if idx.len() == 3 && cross3(p(idx[0]), p(idx[1]), p(idx[2])) > 0.0 {
        out.extend_from_slice(&idx);
    }
    out
}

/// Inward offset direction (scaled miter) at each vertex of a contour whose
/// solid is on its left, plus the outward edge normals.
fn miters(c: &[[f32; 2]]) -> (Vec<[f32; 2]>, Vec<[f32; 2]>) {
    let n = c.len();
    let out_n: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let (a, b) = (c[i], c[(i + 1) % n]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy).max(1e-12);
            [dy / l, -dx / l]
        })
        .collect();
    let mit = (0..n)
        .map(|i| {
            let (n0, n1) = (out_n[(i + n - 1) % n], out_n[i]);
            let (x, y) = (n0[0] + n1[0], n0[1] + n1[1]);
            let l = x.hypot(y);
            if l < 1e-6 {
                return [-n1[0], -n1[1]];
            }
            let m = [x / l, y / l];
            let k = 1.0 / (m[0] * n1[0] + m[1] * n1[1]).max(0.4);
            [-m[0] * k, -m[1] * k]
        })
        .collect();
    (mit, out_n)
}

/// Vertex `i` of contour `c` moved inwards by `d` along its miter (clamped
/// at sharp corners so they don't spike).
fn inset_point(c: &[[f32; 2]], mit: &[[f32; 2]], i: usize, d: f32) -> [f32; 2] {
    let mx = mit[i];
    let l = mx[0].hypot(mx[1]);
    let s = if l > 2.5 { 2.5 / l } else { 1.0 };
    [c[i][0] + mx[0] * d * s, c[i][1] + mx[1] * d * s]
}

/// Whether insetting the outline and its holes by `d` keeps every edge free
/// of crossings (and each ring's orientation).
fn simple_inset(outer: &[[f32; 2]], holes: &[Vec<[f32; 2]>], d: f32) -> bool {
    let rings: Vec<Vec<[f32; 2]>> = std::iter::once(outer)
        .chain(holes.iter().map(|h| h.as_slice()))
        .map(|c| {
            let (mit, _) = miters(c);
            (0..c.len()).map(|i| inset_point(c, &mit, i, d)).collect()
        })
        .collect();
    for (r, orig) in rings.iter().zip(std::iter::once(outer).chain(holes.iter().map(|h| h.as_slice()))) {
        if area2(r).signum() != area2(orig).signum() || area2(r).abs() < area2(orig).abs() * 0.05 {
            return false;
        }
    }
    let segs: Vec<Seg> = rings
        .iter()
        .enumerate()
        .flat_map(|(ri, r)| (0..r.len()).map(move |i| (r[i], r[(i + 1) % r.len()], ri, i, r.len())))
        .collect();
    let hit = |a: [f32; 2], b: [f32; 2], c: [f32; 2], e: [f32; 2]| -> bool {
        let d1 = cross3(a, b, c);
        let d2 = cross3(a, b, e);
        let d3 = cross3(c, e, a);
        let d4 = cross3(c, e, b);
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    };
    for x in 0..segs.len() {
        let (a, b, ra, ia, na) = segs[x];
        let (lo_x, hi_x) = (a[0].min(b[0]), a[0].max(b[0]));
        for y in x + 1..segs.len() {
            let (c, e, rb, ib, _) = segs[y];
            if ra == rb && (ib == (ia + 1) % na || ia == (ib + 1) % na) {
                continue;
            }
            if c[0].max(e[0]) < lo_x || c[0].min(e[0]) > hi_x {
                continue;
            }
            if hit(a, b, c, e) {
                return false;
            }
        }
    }
    true
}

/// Extrude one letter's contours `2·hz` deep with a quarter-round bevel of
/// `b`. Caps get a vertical normal tilt of `pillow` per em (word space).
fn extrude(cs: &[Vec<[f32; 2]>], hz: f32, bevel: f32, pillow: f32) -> Mesh {
    extrude_segs(cs, hz, bevel, pillow, BEVEL_SEGS)
}

/// [`extrude`] with `segs` segments per quarter-round bevel.
pub(crate) fn extrude_segs(cs: &[Vec<[f32; 2]>], hz: f32, bevel: f32, pillow: f32, bevel_segs: usize) -> Mesh {
    let mut m = Mesh::default();
    let push = |m: &mut Mesh, p: [f32; 3], n: [f32; 3]| -> u32 {
        let i = (m.pos.len() / 3) as u32;
        m.pos.extend_from_slice(&p);
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
        m.nrm.extend_from_slice(&[n[0] / l, n[1] / l, n[2] / l]);
        m.uv.extend_from_slice(&[0.0, 0.0]);
        i
    };
    for (outer, holes) in group(cs) {
        // Thin strokes (serif hairlines) can't take the full bevel: shrink it
        // until the inset cap outlines stay simple.
        let mut b = bevel;
        while b > 0.0 && !simple_inset(&outer, &holes, b) {
            b = if b < bevel * 0.1 { 0.0 } else { b * 0.6 };
        }
        let segs = if b > 0.0 { bevel_segs } else { 0 };
        // Profile from back to front: (inset, z, normal xy weight, normal z).
        let mut prof: Vec<(f32, f32, f32, f32)> = Vec::new();
        for k in (0..=segs).rev() {
            let a = if segs == 0 { 0.0 } else { k as f32 / segs as f32 * FRAC_PI_2 };
            prof.push((b * (1.0 - a.cos()), -(hz - b + b * a.sin()), a.cos(), -a.sin()));
        }
        for k in 0..=segs {
            let a = if segs == 0 { 0.0 } else { k as f32 / segs as f32 * FRAC_PI_2 };
            prof.push((b * (1.0 - a.cos()), hz - b + b * a.sin(), a.cos(), a.sin()));
        }
        let rings: Vec<&Vec<[f32; 2]>> = std::iter::once(&outer).chain(holes.iter()).collect();
        let mut caps: Vec<Vec<[f32; 2]>> = Vec::new();
        for c in &rings {
            let n = c.len();
            let (mit, on) = miters(c);
            let off = |i: usize, d: f32| inset_point(c, &mit, i, d);
            caps.push((0..n).map(|i| off(i, b)).collect());
            let smooth = |a: [f32; 2], o: [f32; 2]| -> [f32; 2] {
                if a[0] * o[0] + a[1] * o[1] > 0.8 {
                    let (x, y) = (a[0] + o[0], a[1] + o[1]);
                    let l = x.hypot(y).max(1e-9);
                    [x / l, y / l]
                } else {
                    a
                }
            };
            for i in 0..n {
                let j = (i + 1) % n;
                let na = smooth(on[i], on[(i + n - 1) % n]);
                let nb = smooth(on[i], on[j]);
                let base = (m.pos.len() / 3) as u32;
                for &(inset, z, wxy, wz) in &prof {
                    let pa = off(i, inset);
                    let pb = off(j, inset);
                    push(&mut m, [pa[0], pa[1], z], [na[0] * wxy, na[1] * wxy, wz]);
                    push(&mut m, [pb[0], pb[1], z], [nb[0] * wxy, nb[1] * wxy, wz]);
                }
                for r in 0..prof.len() as u32 - 1 {
                    let (a0, b0, a1, b1) = (base + 2 * r, base + 2 * r + 1, base + 2 * r + 2, base + 2 * r + 3);
                    m.idx.extend_from_slice(&[a0, b0, b1, a0, b1, a1]);
                }
            }
        }
        let tris = triangulate(&caps[0], &caps[1..]);
        let flat: Vec<[f32; 2]> = caps.iter().flatten().copied().collect();
        for (z, nz) in [(hz, 1.0f32), (-hz, -1.0)] {
            let base = (m.pos.len() / 3) as u32;
            for q in &flat {
                push(&mut m, [q[0], q[1], z], [0.0, q[1] * pillow * nz.max(0.0), nz]);
            }
            for t in tris.chunks_exact(3) {
                if nz > 0.0 {
                    m.idx.extend_from_slice(&[base + t[0], base + t[1], base + t[2]]);
                } else {
                    m.idx.extend_from_slice(&[base + t[0], base + t[2], base + t[1]]);
                }
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::super::preview::{snapshot, validate};
    use super::super::{build, Theme};
    use super::*;
    use kurbo::Shape;

    fn backdrop(w: f32, h: f32) -> Op {
        Op::Fill {
            path: kurbo::Rect::new(0.0, 0.0, w as f64, h as f64).into_path(0.1),
            paint: radial(w * 0.5, h * 0.42, w * 0.7, vec![(0.0, rgba(0.12, 0.12, 0.16, 1.0)), (1.0, rgba(0.025, 0.025, 0.04, 1.0))]),
            rule: FillRule::NonZero,
            transform: crate::dl::IDENTITY,
        }
    }

    fn cases() -> Vec<(&'static str, serde_json::Value)> {
        use serde_json::json;
        vec![
            ("default", json!({})),
            ("gold", json!({"text": "Gold 24", "color": "#e8b85a", "metallic": 0.9, "font": "serif", "weight": 400})),
            ("gradient", json!({"text": "Launch", "gradient": ["#7c6cff", "#ff6fb1", "#ffb547"], "metallic": 0.4, "roughness": 0.3, "intro": "drop"})),
            ("matte", json!({"text": "2026", "color": "#2ad4e8", "metallic": 0.0, "roughness": 0.6, "intro": "fly-in", "depth": 0.35})),
            ("logo", json!({"points": [[58, 0], [12, 62], [44, 62], [32, 112], [86, 40], [52, 40], [70, 0]], "color": "#ffb547"})),
            ("path", json!({"path": "M0 0 H100 V100 H0 Z M30 30 V70 H70 V30 Z", "angle": -30})),
        ]
    }

    #[test]
    fn renders() {
        let theme = Theme::default();
        let assets = super::super::preview::host();
        let (w, h) = (960.0, 400.0);
        for (name, params) in cases() {
            let p = build("text-3d", &params).unwrap_or_else(|e| panic!("{name}: {e}"));
            let end = p.component.duration().unwrap();
            let mut seen = 0;
            let shine = end * 0.7 + 0.6;
            for t in [0.0, end * 0.3, end * 0.5, end, shine, end + 2.0] {
                let ops = p.eval(t, w, h, 30.0, &theme, assets);
                validate(&ops).unwrap_or_else(|e| panic!("{name} at {t}: {e}"));
                if t == end * 0.3 || t == end || t == shine {
                    let mut all = vec![backdrop(w, h)];
                    all.extend(ops);
                    let tag = if t == end { "" } else if t == shine { "-shine" } else { "-intro" };
                    let px = snapshot(&format!("text3d-{name}{tag}"), w as u32, h as u32, theme.background, all, assets);
                    if t == end {
                        seen = px.chunks_exact(4).filter(|c| c[0].max(c[1]).max(c[2]) > 90).count();
                    }
                }
            }
            assert!(seen > 3000, "{name}: only {seen} bright pixels");
        }
    }

    #[test]
    fn camel_kind_and_caching() {
        let theme = Theme::default();
        let assets = super::super::preview::host();
        assert!(build("text3d", &serde_json::json!({"text": "Hi"})).is_ok());
        let p = build("text-3d", &serde_json::json!({"text": "Hi", "delay": 0.5})).unwrap();
        assert!(p.eval(0.2, 320.0, 240.0, 30.0, &theme, assets).is_empty());
        p.eval(2.0, 320.0, 240.0, 30.0, &theme, assets);
        let n = assets.image_count();
        p.eval(2.0, 320.0, 240.0, 30.0, &theme, assets);
        assert_eq!(assets.image_count(), n, "same frame reuses the image");
    }

    #[test]
    fn triangulates_holes() {
        let outer = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let hole = vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
        let t = triangulate(&outer, &[hole]);
        let pts = [outer.clone(), vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]]].concat();
        let area: f32 = t.chunks_exact(3).map(|c| cross3(pts[c[0] as usize], pts[c[1] as usize], pts[c[2] as usize]) * 0.5).sum();
        assert!((area - 12.0).abs() < 1e-4, "area {area}");
        // "ANIKIT" outlines produce closed letters with caps.
        let shapes = glyph_outlines("AB", "Inter", 800.0, 0.0);
        assert_eq!(shapes.len(), 2);
        assert_eq!(group(&shapes[1]).iter().map(|g| g.1.len()).sum::<usize>(), 2, "B has two holes");
    }

    #[test]
    fn errors() {
        let e = build("text-3d", &serde_json::json!({"intro": "rsie"})).err().unwrap();
        assert!(e.0.contains("did you mean `rise`"), "{e}");
        assert!(build("text-3d", &serde_json::json!({"points": [[0, 0], [1, 1]]})).is_err());
        assert!(build("text-3d", &serde_json::json!({"path": "M0 0 A 1 1 0 0 0 1 1 Z"})).is_err());
        assert!(build("text-3d", &serde_json::json!({"text": "  "})).is_err());
        assert!(build("text-3d", &serde_json::json!({"intro": "flyIn"})).is_ok());
    }

    /// `cargo test --release -p anikit-video text3d::tests::timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn timing() {
        let theme = Theme::default();
        for (name, params) in cases() {
            let assets = super::super::preview::host();
            let p = build("text-3d", &params).unwrap();
            let n = 30;
            let t0 = std::time::Instant::now();
            for i in 0..n {
                p.eval(1.0 + i as f32 / 30.0, 1920.0, 1080.0, 30.0, &theme, assets);
            }
            println!("{name:12} {:.2} ms/frame", t0.elapsed().as_secs_f64() * 1e3 / n as f64);
        }
    }
}
