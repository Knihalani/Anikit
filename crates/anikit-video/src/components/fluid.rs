//! `fluid`: ink in water, smoke and fire from a real fluid simulation.
//!
//! A 2D incompressible fluid (Stam's stable fluids): semi-Lagrangian
//! advection, a pressure projection (Jacobi), vorticity confinement to keep
//! the swirls alive (Fedkiw, Stam & Jensen 2001), buoyancy for smoke and fire,
//! and curl-noise stirring. Velocity lives on a coarse grid, the dye on a grid
//! `detail`× finer, so filaments stay sharp at full HD for a few ms per step.
//!
//! Sources inject dye and push (moving jets, bursts); `text` writes a word in
//! dye that the flow then takes apart. Looks:
//! - `ink`: Beer–Lambert absorption on paper, so thin wisps are pale and the
//!   core is deep, with a sheen on filament edges;
//! - `smoke`: a self-shadowed volume lit by a key light over a dark ground;
//! - `fire`: blackbody emission from temperature, with soot above it.
//!
//! The simulation steps at 60 Hz, deterministically (every cell is computed
//! from the previous arrays, rows in parallel), and frames are cached, so
//! renders can run frames in any order.

use super::style::{fnv, Color};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::Op;
use anikit_core::noise::Noise;
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const HZ: f32 = 60.0;

#[derive(Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Source {
    /// Position as fractions of the box (x right, y down); moves to `to` between `start` and `end`.
    pub pos: [f32; 2],
    pub to: Option<[f32; 2]>,
    /// Radius as a fraction of the box height.
    pub radius: f32,
    /// Push direction in degrees (0 = right, 90 = down, -90 = up) and speed in box heights per second.
    pub angle: f32,
    pub speed: f32,
    /// Dye colour (ink: the colour seen where it's dense).
    pub color: Option<Color>,
    /// Dye per second.
    pub amount: f32,
    /// Heat per second (smoke and fire rise with it).
    pub heat: f32,
    pub start: f32,
    pub end: f32,
}

impl Default for Source {
    fn default() -> Self {
        Source { pos: [0.5, 0.5], to: None, radius: 0.05, angle: -90.0, speed: 0.6, color: None, amount: 1.0, heat: 0.0, start: 0.0, end: 1.0 }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Word {
    pub text: String,
    pub font: String,
    pub weight: f32,
    /// Centre (fractions of the box) and cap height (fraction of the box height).
    pub pos: [f32; 2],
    pub size: f32,
    pub color: Option<Color>,
    /// Dye density inside the letters.
    pub amount: f32,
    /// Heat inside the letters (smoke type rises; 0 for ink).
    pub heat: f32,
    /// When the word is laid down (seconds).
    pub at: f32,
}

impl Default for Word {
    fn default() -> Self {
        Word { text: "FLUID".into(), font: "Inter".into(), weight: 800.0, pos: [0.5, 0.5], size: 0.22, color: None, amount: 1.0, heat: 0.0, at: 0.0 }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Fluid {
    /// `ink`, `smoke` or `fire`.
    pub look: String,
    pub background: Option<Color>,
    /// Default dye colour.
    pub color: Option<Color>,
    pub sources: Vec<Source>,
    pub text: Option<Word>,
    /// Velocity grid cells along the long side, and dye detail multiple.
    pub resolution: u32,
    pub detail: u32,
    /// Vorticity confinement (swirl kept alive): 0 = smeared, 0.3–0.6 = ink, 1+ = turbulent.
    pub vorticity: f32,
    /// Curl-noise stirring (box heights per second) and its scale (fraction of the box).
    pub stir: f32,
    pub stir_scale: f32,
    /// Buoyancy per unit heat (box heights per second²), and dye weight (ink sinking).
    pub buoyancy: f32,
    pub weight: f32,
    /// Fraction of dye and heat kept per second.
    pub fade: f32,
    pub cool: f32,
    /// Pressure iterations (more = more incompressible).
    pub iterations: u32,
    /// Ink strength / smoke density multiplier.
    pub density: f32,
    /// Key light direction for smoke (degrees, 0 = from the right, -90 = from above).
    pub light: f32,
    /// Simulated seconds per second.
    pub speed: f32,
    pub seed: u32,
    /// Advection: `maccormack` (sharp: little numerical diffusion) or `linear` (soft, semi-Lagrangian).
    pub advection: String,
    /// Sub-grid turbulence: curl noise finer than the velocity grid, scaled
    /// by the local swirl, that tears dye into filaments (0 = off).
    pub turbulence: f32,
    /// Octaves of it (each half the size of the last), down to about two dye cells.
    pub octaves: u32,
    /// Ink: extra absorption where the dye's edge is sharp (filament walls read darker).
    pub edge: f32,
    /// Ink: colour of thin, dilute ink (Beer–Lambert gives the dense colour);
    /// `chroma` is how far thin regions move towards it (0–1).
    pub thin_color: Option<Color>,
    pub chroma: f32,
    /// Ink: paper granulation (pigment settling into the paper's grain), 0–1.
    pub granulation: f32,
    #[serde(skip)]
    sim: OnceLock<Sim>,
    #[serde(skip)]
    key_hash: OnceLock<u64>,
}

impl Default for Fluid {
    fn default() -> Self {
        Fluid {
            look: "ink".into(),
            background: None,
            color: None,
            sources: Vec::new(),
            text: None,
            resolution: 192,
            detail: 4,
            vorticity: 0.45,
            stir: 0.06,
            stir_scale: 0.35,
            buoyancy: 0.0,
            weight: 0.0,
            fade: 0.98,
            cool: 0.6,
            iterations: 30,
            density: 1.0,
            light: -60.0,
            speed: 1.0,
            seed: 3,
            advection: "maccormack".into(),
            turbulence: 0.7,
            octaves: 3,
            edge: 0.35,
            thin_color: None,
            chroma: 0.0,
            granulation: 0.0,
            sim: OnceLock::new(),
            key_hash: OnceLock::new(),
        }
    }
}

/// Simulation fields: velocity (`nx × ny`), dye RGB and heat (`dx × dy`).
#[derive(Clone)]
struct Fields {
    step: u64,
    u: Vec<f32>,
    v: Vec<f32>,
    dye: [Vec<f32>; 3],
    heat: Vec<f32>,
}

/// What a frame needs to draw: the dye and heat at that instant.
struct Frame {
    dye: [Vec<f32>; 3],
    heat: Vec<f32>,
}

struct SimState {
    /// Live states (a render works on several stretches of the timeline at once).
    lives: Vec<Fields>,
    frames: HashMap<u64, Arc<Frame>>,
    checkpoints: Vec<Fields>,
}

struct Sim {
    nx: usize,
    ny: usize,
    dx: usize,
    dy: usize,
    word: Option<Vec<f32>>,
    state: Mutex<SimState>,
}

impl Prepare for Fluid {
    fn prepare(&mut self) -> Result<(), Error> {
        if !["ink", "smoke", "fire"].contains(&self.look.as_str()) {
            return Err(Error(format!("fluid: unknown look `{}` (ink, smoke, fire)", self.look)));
        }
        self.resolution = self.resolution.clamp(32, 512);
        self.detail = self.detail.clamp(1, 8);
        self.iterations = self.iterations.clamp(4, 200);
        if self.sources.is_empty() && self.text.is_none() {
            // A default: a jet rising from the bottom centre.
            self.sources.push(Source { pos: [0.5, 0.92], radius: 0.06, angle: -90.0, speed: 0.7, amount: 1.4, heat: if self.look == "ink" { 0.0 } else { 1.0 }, end: 4.0, ..Source::default() });
        }
        if !["maccormack", "linear"].contains(&self.advection.as_str()) {
            return Err(Error(format!("fluid: unknown advection `{}` (maccormack, linear)", self.advection)));
        }
        self.octaves = self.octaves.clamp(1, 6);
        self.chroma = self.chroma.clamp(0.0, 1.0);
        self.granulation = self.granulation.clamp(0.0, 1.0);
        if self.look != "ink" && self.buoyancy == 0.0 {
            self.buoyancy = 0.9;
        }
        self.key_hash = OnceLock::new();
        Ok(())
    }
}

#[inline]
fn lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn rgb_lin(c: Option<Color>, d: [f32; 3]) -> [f32; 3] {
    match c {
        Some(c) => [lin(c.0 .0[0]), lin(c.0 .0[1]), lin(c.0 .0[2])],
        None => d,
    }
}

/// Bilinear sample of a `w × h` grid at cell coordinates (cell centres at integers), clamped.
#[inline(always)]
fn bilerp(g: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x as usize, y as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let a = g[y0 * w + x0] + (g[y0 * w + x1] - g[y0 * w + x0]) * tx;
    let b = g[y1 * w + x0] + (g[y1 * w + x1] - g[y1 * w + x0]) * tx;
    a + (b - a) * ty
}

/// Catmull-Rom (bicubic) sample, clamped to non-negative: smooth when magnified.
#[inline(always)]
fn bicubic(g: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (tx, ty) = (x - fx, y - fy);
    let wts = |t: f32| {
        let t2 = t * t;
        let t3 = t2 * t;
        [-0.5 * t3 + t2 - 0.5 * t, 1.5 * t3 - 2.5 * t2 + 1.0, -1.5 * t3 + 2.0 * t2 + 0.5 * t, 0.5 * t3 - 0.5 * t2]
    };
    let (wx, wy) = (wts(tx), wts(ty));
    let (ix, iy) = (fx as i32, fy as i32);
    let mut acc = 0.0;
    for (j, wyj) in wy.iter().enumerate() {
        let yy = (iy - 1 + j as i32).clamp(0, h as i32 - 1) as usize;
        let row = &g[yy * w..(yy + 1) * w];
        let mut r = 0.0;
        for (i, wxi) in wx.iter().enumerate() {
            let xx = (ix - 1 + i as i32).clamp(0, w as i32 - 1) as usize;
            r += row[xx] * wxi;
        }
        acc += r * wyj;
    }
    // Clamp to the four nearest values: no ringing (dark halos, colour fringes) at sharp edges.
    let (x0, y0) = ((ix.clamp(0, w as i32 - 1)) as usize, (iy.clamp(0, h as i32 - 1)) as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (a, b, c, d) = (g[y0 * w + x0], g[y0 * w + x1], g[y1 * w + x0], g[y1 * w + x1]);
    acc.clamp(a.min(b).min(c).min(d), a.max(b).max(c).max(d))
}

/// Semi-Lagrangian advection of `src` (`w × h`) by displacements `ox, oy`
/// (cells per step) times `sign`. `open`: departures outside the grid read 0.
fn advect_sl(src: &[f32], w: usize, h: usize, ox: &[f32], oy: &[f32], sign: f32, open: bool) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            let (px, py) = (x as f32 - ox[i] * sign, y as f32 - oy[i] * sign);
            row[x] = if open && !(px > -0.5 && py > -0.5 && px < w as f32 - 0.5 && py < h as f32 - 0.5) { 0.0 } else { bilerp(src, w, h, px, py) };
        }
    });
    out
}

/// Bilinear taps (indices and weights) at a departure point, clamped to the grid.
#[inline(always)]
fn taps(w: usize, h: usize, px: f32, py: f32) -> ([usize; 4], [f32; 4]) {
    let x = px.clamp(0.0, (w - 1) as f32);
    let y = py.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x as usize, y as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    ([y0 * w + x0, y0 * w + x1, y1 * w + x0, y1 * w + x1], [(1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty])
}

/// `advect_sl` of several fields at once (shared departure points).
fn advect_sl_n<const N: usize>(src: [&[f32]; N], w: usize, h: usize, ox: &[f32], oy: &[f32], sign: f32) -> [Vec<f32>; N] {
    let mut out: [Vec<f32>; N] = std::array::from_fn(|_| vec![0.0f32; w * h]);
    let rows: Vec<(usize, Vec<[f32; N]>)> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut r = vec![[0.0f32; N]; w];
            for (x, o) in r.iter_mut().enumerate() {
                let i = y * w + x;
                let (px, py) = (x as f32 - ox[i] * sign, y as f32 - oy[i] * sign);
                if !(px > -0.5 && py > -0.5 && px < w as f32 - 0.5 && py < h as f32 - 0.5) {
                    continue;
                }
                let (ix, wt) = taps(w, h, px, py);
                for c in 0..N {
                    let g = src[c];
                    o[c] = g[ix[0]] * wt[0] + g[ix[1]] * wt[1] + g[ix[2]] * wt[2] + g[ix[3]] * wt[3];
                }
            }
            (y, r)
        })
        .collect();
    for (y, r) in rows {
        for (x, v) in r.into_iter().enumerate() {
            for c in 0..N {
                out[c][y * w + x] = v[c];
            }
        }
    }
    out
}

/// MacCormack advection (Selle et al. 2008) of several fields at once, with
/// open boundaries: a forward and a backward semi-Lagrangian step estimate
/// the error and half of it is corrected (far less numerical diffusion).
fn advect_mac_n<const N: usize>(src: [&[f32]; N], w: usize, h: usize, ox: &[f32], oy: &[f32]) -> [Vec<f32>; N] {
    let mut fwd = advect_sl_n(src, w, h, ox, oy, 1.0);
    let back: [Vec<f32>; N] = advect_sl_n(std::array::from_fn(|c| fwd[c].as_slice()), w, h, ox, oy, -1.0);
    let fixes: Vec<Vec<[f32; N]>> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut r = vec![[0.0f32; N]; w];
            for (x, o) in r.iter_mut().enumerate() {
                let i = y * w + x;
                let (px, py) = (x as f32 - ox[i], y as f32 - oy[i]);
                if !(px > -0.5 && py > -0.5 && px < w as f32 - 0.5 && py < h as f32 - 0.5) {
                    continue;
                }
                let (ix, _) = taps(w, h, px, py);
                // Where the correction would leave the departure cell's range
                // in any channel, keep the first-order value in all of them
                // (Selle et al.): channels stay consistent, so no colour fringes.
                let mut ok = true;
                for c in 0..N {
                    let g = src[c];
                    let (a, b, cc, d) = (g[ix[0]], g[ix[1]], g[ix[2]], g[ix[3]]);
                    let (lo, hi) = (a.min(b).min(cc).min(d), a.max(b).max(cc).max(d));
                    let v = fwd[c][i] + 0.5 * (g[i] - back[c][i]);
                    ok &= v >= lo && v <= hi;
                    o[c] = v;
                }
                if !ok {
                    for c in 0..N {
                        o[c] = fwd[c][i];
                    }
                }
            }
            r
        })
        .collect();
    for (y, r) in fixes.into_iter().enumerate() {
        for (x, v) in r.into_iter().enumerate() {
            for c in 0..N {
                fwd[c][y * w + x] = v[c];
            }
        }
    }
    fwd
}

impl Fluid {
    /// The simulation (made for the first box aspect it's drawn at).
    fn sim(&self, aspect: f32) -> &Sim {
        self.sim.get_or_init(|| self.make_sim(aspect))
    }

    fn make_sim(&self, aspect: f32) -> Sim {
        let long = self.resolution as f32;
        let (nx, ny) = if aspect >= 1.0 { (long as usize, (long / aspect).round().max(8.0) as usize) } else { ((long * aspect).round().max(8.0) as usize, long as usize) };
        let (dx, dy) = (nx * self.detail as usize, ny * self.detail as usize);
        let word = self.text.as_ref().map(|w| word_mask(w, dx, dy, aspect));
        Sim { nx, ny, dx, dy, word, state: Mutex::new(SimState { lives: Vec::new(), frames: HashMap::new(), checkpoints: Vec::new() }) }
    }

    fn init(&self, s: &Sim) -> Fields {
        let (n, d) = (s.nx * s.ny, s.dx * s.dy);
        Fields { step: 0, u: vec![0.0; n], v: vec![0.0; n], dye: [vec![0.0; d], vec![0.0; d], vec![0.0; d]], heat: vec![0.0; d] }
    }

    /// The ink colour as absorbance (what thickness of it removes per channel).
    fn absorb(&self, c: [f32; 3]) -> [f32; 3] {
        c.map(|v| -(v.max(0.01)).ln())
    }

    fn dye_color(&self, c: Option<Color>) -> [f32; 3] {
        let base = rgb_lin(c.or(self.color), match self.look.as_str() {
            "ink" => [0.012, 0.05, 0.16],
            "fire" => [0.03, 0.025, 0.02],
            _ => [0.75, 0.74, 0.72],
        });
        if self.look == "ink" {
            self.absorb(base)
        } else {
            base
        }
    }

    /// One step of `dt` simulated seconds.
    fn step(&self, s: &Sim, f: &mut Fields, dt: f32, noise: &Noise) {
        let c_step = std::time::Instant::now();
        let (nx, ny, ddx, ddy) = (s.nx, s.ny, s.dx, s.dy);
        let t = f.step as f32 / HZ;
        // Velocity units: cells per second (the box height is `ny` cells).
        let hcell = ny as f32;
        let det = self.detail as f32;
        // 1. Sources: push velocity, add dye and heat.
        for src in &self.sources {
            if t < src.start || t > src.end {
                continue;
            }
            let k = ((t - src.start) / (src.end - src.start).max(1e-3)).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let p = match src.to {
                Some(to) => [src.pos[0] + (to[0] - src.pos[0]) * k, src.pos[1] + (to[1] - src.pos[1]) * k],
                None => src.pos,
            };
            let (cx, cy) = (p[0] * nx as f32, p[1] * ny as f32);
            let r = (src.radius * hcell).max(0.75);
            let a = src.angle.to_radians();
            let (vx, vy) = (a.cos() * src.speed * hcell, a.sin() * src.speed * hcell);
            let (x0, x1) = (((cx - 2.0 * r).floor().max(0.0)) as usize, ((cx + 2.0 * r).ceil() as usize).min(nx - 1));
            let (y0, y1) = (((cy - 2.0 * r).floor().max(0.0)) as usize, ((cy + 2.0 * r).ceil() as usize).min(ny - 1));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let d2 = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)) / (r * r);
                    let wgt = (-d2 * 2.0).exp();
                    let i = y * nx + x;
                    f.u[i] += (vx - f.u[i]) * wgt * (dt * 12.0).min(1.0);
                    f.v[i] += (vy - f.v[i]) * wgt * (dt * 12.0).min(1.0);
                }
            }
            let col = self.dye_color(src.color);
            let (dcx, dcy, dr) = (cx * det, cy * det, r * det);
            let (x0, x1) = (((dcx - 2.0 * dr).floor().max(0.0)) as usize, ((dcx + 2.0 * dr).ceil() as usize).min(ddx - 1));
            let (y0, y1) = (((dcy - 2.0 * dr).floor().max(0.0)) as usize, ((dcy + 2.0 * dr).ceil() as usize).min(ddy - 1));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let d2 = ((x as f32 + 0.5 - dcx).powi(2) + (y as f32 + 0.5 - dcy).powi(2)) / (dr * dr);
                    let wgt = (-d2 * 2.0).exp() * dt;
                    let i = y * ddx + x;
                    for c in 0..3 {
                        f.dye[c][i] += col[c] * src.amount * wgt * 2.0;
                    }
                    f.heat[i] += src.heat * wgt * 2.0;
                }
            }
        }
        // The word, laid down once.
        if let (Some(w), Some(mask)) = (&self.text, &s.word) {
            let step_at = (w.at * HZ).round() as u64;
            if f.step == step_at {
                let col = self.dye_color(w.color);
                for (i, m) in mask.iter().enumerate() {
                    if *m > 0.0 {
                        for c in 0..3 {
                            f.dye[c][i] += col[c] * w.amount * m;
                        }
                        f.heat[i] += w.heat * m;
                    }
                }
            }
        }
        // 2. Forces: stirring (curl of a drifting noise potential), buoyancy, vorticity confinement.
        let mut fu = vec![0.0f32; nx * ny];
        let mut fv = vec![0.0f32; nx * ny];
        if self.stir > 0.0 {
            let sc = 1.0 / (self.stir_scale.max(0.02) * hcell);
            let psi: Vec<f32> = (0..(nx + 1) * (ny + 1))
                .into_par_iter()
                .map(|k| {
                    let (x, y) = ((k % (nx + 1)) as f64, (k / (nx + 1)) as f64);
                    let p = noise.noise3(x * sc as f64, y * sc as f64, t as f64 * 0.25) + 0.5 * noise.noise3(x * sc as f64 * 2.3 + 17.0, y * sc as f64 * 2.3, t as f64 * 0.5);
                    p as f32
                })
                .collect();
            // Stirring eases in over the first second, so a word reads before it goes.
            let ramp = (t.min(1.0)) * (t.min(1.0)) * (3.0 - 2.0 * t.min(1.0));
            let amp = self.stir * hcell * (1.0 / sc) * 2.0 * ramp;
            fu.par_chunks_mut(nx).zip(fv.par_chunks_mut(nx)).enumerate().for_each(|(y, (ru, rv))| {
                for x in 0..nx {
                    let (a, b, c, d) = (psi[y * (nx + 1) + x], psi[y * (nx + 1) + x + 1], psi[(y + 1) * (nx + 1) + x], psi[(y + 1) * (nx + 1) + x + 1]);
                    // u = ∂ψ/∂y, v = -∂ψ/∂x (divergence-free), as a target velocity.
                    ru[x] = ((c + d) - (a + b)) * 0.5 * amp;
                    rv[x] = -((b + d) - (a + c)) * 0.5 * amp;
                }
            });
        }
        let du = &f.u;
        let dv = &f.v;
        // Vorticity at cell centres.
        let w: Vec<f32> = (0..nx * ny)
            .into_par_iter()
            .map(|i| {
                let (x, y) = (i % nx, i / nx);
                let (xl, xr, yu, yd) = (x.saturating_sub(1), (x + 1).min(nx - 1), y.saturating_sub(1), (y + 1).min(ny - 1));
                (dv[y * nx + xr] - dv[y * nx + xl]) * 0.5 - (du[yd * nx + x] - du[yu * nx + x]) * 0.5
            })
            .collect();
        let heat_c: Vec<f32> = if self.buoyancy != 0.0 || self.weight != 0.0 {
            (0..nx * ny)
                .into_par_iter()
                .map(|i| {
                    let (x, y) = (i % nx, i / nx);
                    let (cx, cy) = ((x as f32 + 0.5) * det - 0.5, (y as f32 + 0.5) * det - 0.5);
                    let h = bilerp(&f.heat, ddx, ddy, cx, cy);
                    let dsum = bilerp(&f.dye[0], ddx, ddy, cx, cy) + bilerp(&f.dye[1], ddx, ddy, cx, cy) + bilerp(&f.dye[2], ddx, ddy, cx, cy);
                    // Up is -y: heat rises; dye weight sinks (ink).
                    -self.buoyancy * h * hcell + self.weight * dsum * hcell * 0.3
                })
                .collect()
        } else {
            Vec::new()
        };
        let eps = self.vorticity;
        let (stir_on, stir_k) = (self.stir > 0.0, (dt * 2.0).min(1.0));
        let mut nu = f.u.clone();
        let mut nv = f.v.clone();
        nu.par_chunks_mut(nx).zip(nv.par_chunks_mut(nx)).enumerate().for_each(|(y, (ru, rv))| {
            for x in 0..nx {
                let i = y * nx + x;
                let (xl, xr, yu, yd) = (x.saturating_sub(1), (x + 1).min(nx - 1), y.saturating_sub(1), (y + 1).min(ny - 1));
                // Confinement: push along N × ω, N = ∇|ω| / |∇|ω||.
                let gx = (w[y * nx + xr].abs() - w[y * nx + xl].abs()) * 0.5;
                let gy = (w[yd * nx + x].abs() - w[yu * nx + x].abs()) * 0.5;
                let gl = (gx * gx + gy * gy).sqrt() + 1e-5;
                let (nxv, nyv) = (gx / gl, gy / gl);
                let wc = w[i];
                ru[x] += eps * nyv * wc * dt * 60.0 * 0.0167;
                rv[x] += -eps * nxv * wc * dt * 60.0 * 0.0167;
                if stir_on {
                    ru[x] += (fu[i] - ru[x]) * stir_k * 0.5;
                    rv[x] += (fv[i] - rv[x]) * stir_k * 0.5;
                }
                if !heat_c.is_empty() {
                    rv[x] += heat_c[i] * dt;
                }
            }
        });
        // 3. Advect velocity, in cells.
        let mac = self.advection == "maccormack";
        let (ox, oy): (Vec<f32>, Vec<f32>) = (nu.iter().map(|x| x * dt).collect(), nv.iter().map(|x| x * dt).collect());
        // Velocity stays first-order: MacCormack on the coarse grid leaves
        // kinks that the fine dye shears into axis-aligned steps.
        let (au, av) = (advect_sl(&nu, nx, ny, &ox, &oy, 1.0, false), advect_sl(&nv, nx, ny, &ox, &oy, 1.0, false));
        // Open boundaries: the flow (and the dye) leaves the frame freely.
        // 4. Project: solve ∇²p = ∇·u (Jacobi), subtract ∇p.
        let div: Vec<f32> = (0..nx * ny)
            .into_par_iter()
            .map(|i| {
                let (x, y) = (i % nx, i / nx);
                let ur = if x + 1 < nx { au[i + 1] } else { au[i] };
                let ul = if x > 0 { au[i - 1] } else { au[i] };
                let vd = if y + 1 < ny { av[i + nx] } else { av[i] };
                let vu = if y > 0 { av[i - nx] } else { av[i] };
                0.5 * (ur - ul + vd - vu)
            })
            .collect();
        let mut p = vec![0.0f32; nx * ny];
        let mut q = vec![0.0f32; nx * ny];
        for _ in 0..self.iterations {
            q.par_chunks_mut(nx).enumerate().for_each(|(y, row)| {
                for x in 0..nx {
                    let i = y * nx + x;
                    let pl = if x > 0 { p[i - 1] } else { 0.0 };
                    let pr = if x + 1 < nx { p[i + 1] } else { 0.0 };
                    let pu = if y > 0 { p[i - nx] } else { 0.0 };
                    let pd = if y + 1 < ny { p[i + nx] } else { 0.0 };
                    row[x] = (pl + pr + pu + pd - div[i]) * 0.25;
                }
            });
            std::mem::swap(&mut p, &mut q);
        }
        f.u.par_chunks_mut(nx).zip(f.v.par_chunks_mut(nx)).enumerate().for_each(|(y, (ru, rv))| {
            for x in 0..nx {
                let i = y * nx + x;
                let pl = if x > 0 { p[i - 1] } else { 0.0 };
                let pr = if x + 1 < nx { p[i + 1] } else { 0.0 };
                let pu = if y > 0 { p[i - nx] } else { 0.0 };
                let pd = if y + 1 < ny { p[i + nx] } else { 0.0 };
                ru[x] = au[i] - 0.5 * (pr - pl);
                rv[x] = av[i] - 0.5 * (pd - pu);
            }
        });
        // 5. Advect dye and heat on the fine grid (BFECC-free semi-Lagrangian, velocity bilinear).
        let keep = self.fade.clamp(0.0, 1.0).powf(dt);
        let hkeep = self.cool.clamp(0.0, 1.0).powf(dt);
        let (u, v) = (&f.u, &f.v);
        // Displacement of each fine cell this step (fine cells): the coarse
        // flow, plus sub-grid turbulence.
        let mut fox = vec![0.0f32; ddx * ddy];
        let mut foy = vec![0.0f32; ddx * ddy];
        fox.par_chunks_mut(ddx).zip(foy.par_chunks_mut(ddx)).enumerate().for_each(|(y, (rx, ry))| {
            for x in 0..ddx {
                let (cx, cy) = ((x as f32 + 0.5) / det - 0.5, (y as f32 + 0.5) / det - 0.5);
                rx[x] = bilerp(u, nx, ny, cx, cy) * dt * det;
                ry[x] = bilerp(v, nx, ny, cx, cy) * dt * det;
            }
        });
        let c_turb = std::time::Instant::now();
        if self.turbulence > 0.0 {
            self.turbulence_into(s, &f.u, &f.v, t, dt, noise, &mut fox, &mut foy);
        }
        // Outside the frame is clear water.
        let adv = |src: &Vec<f32>, k: f32| -> Vec<f32> {
            let [mut o] = if mac { advect_mac_n([src.as_slice()], ddx, ddy, &fox, &foy) } else { advect_sl_n([src.as_slice()], ddx, ddy, &fox, &foy, 1.0) };
            if k != 1.0 {
                o.iter_mut().for_each(|x| *x *= k);
            }
            o
        };
        let c_adv = std::time::Instant::now();
        let src3 = [f.dye[0].as_slice(), f.dye[1].as_slice(), f.dye[2].as_slice()];
        let mut d3 = if mac { advect_mac_n(src3, ddx, ddy, &fox, &foy) } else { advect_sl_n(src3, ddx, ddy, &fox, &foy, 1.0) };
        if keep != 1.0 {
            d3.iter_mut().for_each(|g| g.iter_mut().for_each(|x| *x *= keep));
        }
        f.dye = d3;
        if std::env::var_os("ANIKIT_FLUID_STEP_PROFILE").is_some() {
            eprintln!("fluid step: before turbulence {:.1} ms, turbulence {:.1} ms, dye {:.1} ms", (c_turb - c_step).as_secs_f64() * 1e3, (c_adv - c_turb).as_secs_f64() * 1e3, c_adv.elapsed().as_secs_f64() * 1e3);
        }
        if self.look != "ink" || self.text.as_ref().is_some_and(|w| w.heat > 0.0) || self.sources.iter().any(|s| s.heat > 0.0) {
            f.heat = adv(&f.heat, hkeep);
        }
        f.step += 1;
    }

    /// Adds sub-grid turbulence to the fine displacements: the curl of
    /// `m · ψ`, where ψ is multi-octave noise finer than the velocity grid
    /// (Kolmogorov-like amplitudes, velocity ∝ size^⅓) and `m` the local
    /// coarse swirl |ω|, so still water stays still and the field stays
    /// divergence-free (after Kim et al. 2008, "Wavelet turbulence").
    #[allow(clippy::too_many_arguments)]
    fn turbulence_into(&self, s: &Sim, u: &[f32], v: &[f32], t: f32, dt: f32, noise: &Noise, fox: &mut [f32], foy: &mut [f32]) {
        let (nx, ny, ddx, ddy) = (s.nx, s.ny, s.dx, s.dy);
        let det = self.detail as f32;
        // Coarse |ω| (1/s), smoothed by the bilinear lookup below.
        let w: Vec<f32> = (0..nx * ny)
            .into_par_iter()
            .map(|i| {
                let (x, y) = (i % nx, i / nx);
                let (xl, xr, yu, yd) = (x.saturating_sub(1), (x + 1).min(nx - 1), y.saturating_sub(1), (y + 1).min(ny - 1));
                ((v[y * nx + xr] - v[y * nx + xl]) * 0.5 - (u[yd * nx + x] - u[yu * nx + x]) * 0.5).abs()
            })
            .collect();
        // ψ on fine-cell corners. Largest eddy: three coarse cells; the
        // smallest is kept at four dye cells or more (finer aliases into
        // sawtooth edges).
        let (cw, ch) = (ddx + 1, ddy + 1);
        let big = 3.0 * det;
        let base = 1.0 / big as f64;
        let oct = (self.octaves as usize).min(((big / 4.0).log2().floor() as usize + 1).max(1));
        let tf = t as f64;
        let psi: Vec<f32> = (0..cw * ch)
            .into_par_iter()
            .with_min_len(1024)
            .map(|k| {
                let (x, y) = ((k % cw) as f32, (k / cw) as f32);
                let m = bilerp(&w, nx, ny, x / det - 0.5, y / det - 0.5);
                if m < 1e-3 {
                    return 0.0;
                }
                let mut acc = 0.0f64;
                let (mut f, mut a) = (base, 1.0f64);
                for o in 0..oct {
                    // Smaller eddies turn over faster (τ ∝ size^⅔).
                    let ev = 0.6 * (1u32 << o) as f64 * 0.63f64.powi(o as i32);
                    acc += a * noise.noise3(x as f64 * f + o as f64 * 31.7, y as f64 * f - o as f64 * 17.3, tf * ev + o as f64 * 5.1);
                    f *= 2.0;
                    // ψ ∝ size^(4/3): halving the size scales it by 2^-4/3.
                    a *= 0.397;
                }
                // ψ in fine cells² per second: |ω| (1/s) × the largest eddy's size² (fine cells).
                (acc * m as f64) as f32
            })
            .collect();
        let amp = self.turbulence * big * 0.5 * dt;
        fox.par_chunks_mut(ddx).zip(foy.par_chunks_mut(ddx)).enumerate().for_each(|(y, (rx, ry))| {
            for x in 0..ddx {
                let (a, b, c, d) = (psi[y * cw + x], psi[y * cw + x + 1], psi[(y + 1) * cw + x], psi[(y + 1) * cw + x + 1]);
                rx[x] += ((c + d) - (a + b)) * 0.5 * amp;
                ry[x] -= ((b + d) - (a + c)) * 0.5 * amp;
            }
        });
    }

    /// The fields at video frame `frame` (`fps`), stepping as needed.
    fn frame(&self, s: &Sim, frame: u64, fps: f32) -> Arc<Frame> {
        let target = ((frame as f32 / fps) * HZ * self.speed).round() as u64;
        let mut st = s.state.lock().unwrap();
        if let Some(fr) = st.frames.get(&target) {
            return fr.clone();
        }
        let noise = Noise::new(self.seed);
        // Resume from the latest live state or checkpoint at or before the target.
        let best_live = st.lives.iter().enumerate().filter(|(_, l)| l.step <= target).max_by_key(|(_, l)| l.step).map(|(k, l)| (k, l.step));
        let best_cp = st.checkpoints.iter().filter(|c| c.step <= target).map(|c| c.step).max();
        let start = match (best_live, best_cp) {
            (Some((k, ls)), cp) if cp.is_none_or(|c| ls >= c) => st.lives.swap_remove(k),
            (_, Some(c)) => st.checkpoints.iter().find(|x| x.step == c).cloned().unwrap(),
            _ => self.init(s),
        };
        let st_ref = &mut *st;
        let (from, clock) = (start.step, std::time::Instant::now());
        let out = crate::scene3d::sim_blocking(|| {
            let mut f = start;
            while f.step < target {
                self.step(s, &mut f, 1.0 / HZ, &noise);
                if f.step % 60 == 0 && !st_ref.checkpoints.iter().any(|c| c.step == f.step) && st_ref.checkpoints.len() < 64 {
                    st_ref.checkpoints.push(f.clone());
                }
            }
            f
        });
        if std::env::var_os("ANIKIT_FLUID_PROFILE").is_some() {
            eprintln!("fluid: steps {from}..{target} in {:.0} ms", clock.elapsed().as_secs_f64() * 1e3);
        }
        let fr = Arc::new(Frame { dye: out.dye.clone(), heat: out.heat.clone() });
        st.frames.insert(target, fr.clone());
        // Keep a window of recent frames.
        if st.frames.len() > 48 {
            let mut keys: Vec<u64> = st.frames.keys().copied().collect();
            keys.sort_unstable();
            for k in keys.iter().take(keys.len() - 48) {
                st.frames.remove(k);
            }
        }
        st.lives.push(out);
        if st.lives.len() > 8 {
            // Drop the one furthest behind.
            let k = st.lives.iter().enumerate().min_by_key(|(_, l)| l.step).map(|(k, _)| k).unwrap();
            st.lives.swap_remove(k);
        }
        fr
    }

    fn render(&self, s: &Sim, fr: &Frame, w: usize, h: usize) -> Vec<u8> {
        let (ddx, ddy) = (s.dx, s.dy);
        let bg = rgb_lin(self.background, match self.look.as_str() {
            "ink" => [0.86, 0.84, 0.80],
            _ => [0.006, 0.006, 0.008],
        });
        let dens = self.density;
        // Smoke: light transmittance per fine cell, marching towards the light.
        // Shadows are soft: march a 2× box-downsampled density in steps of
        // one of its cells (sharper fields alias into streaks along the light).
        let (hx, hy) = (ddx.div_ceil(2), ddy.div_ceil(2));
        let shadow: Vec<f32> = if self.look != "ink" {
            let rho: Vec<f32> = (0..hx * hy)
                .into_par_iter()
                .map(|k| {
                    let (x, y) = ((k % hx) * 2, (k / hx) * 2);
                    let mut acc = 0.0;
                    for (xx, yy) in [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)] {
                        let i = yy.min(ddy - 1) * ddx + xx.min(ddx - 1);
                        acc += fr.dye[0][i] + fr.dye[1][i] + fr.dye[2][i];
                    }
                    acc * 0.25
                })
                .collect();
            let a = self.light.to_radians();
            let (lx, ly) = (-a.cos(), -a.sin());
            // As far as before: 29% of the height.
            let steps = ((hy as f32 * 0.29).ceil() as usize).max(4);
            (0..hx * hy)
                .into_par_iter()
                .map(|i| {
                    let (x, y) = ((i % hx) as f32, (i / hx) as f32);
                    let mut od = 0.0;
                    for k in 1..=steps {
                        let (px, py) = (x + lx * k as f32, y + ly * k as f32);
                        if px < 0.0 || py < 0.0 || px >= hx as f32 || py >= hy as f32 {
                            break;
                        }
                        od += bilerp(&rho, hx, hy, px, py) * 2.0;
                    }
                    (-od * dens * 0.02).exp()
                })
                .collect()
        } else {
            Vec::new()
        };
        let shadow_at = |gx: f32, gy: f32| bilerp(&shadow, hx, hy, (gx - 0.5) * 0.5, (gy - 0.5) * 0.5);
        // Thin ink's absorbance shape (sums to 1).
        let thin_abs = {
            let a = self.absorb(rgb_lin(self.thin_color, [0.1, 0.3, 0.55]));
            let t = (a[0] + a[1] + a[2]).max(1e-6);
            a.map(|x| x / t)
        };
        let paper = (self.granulation > 0.0).then(|| Noise::new(self.seed ^ 0x5a5a));
        let mut out = vec![0u8; w * h * 4];
        let (kx, ky) = (ddx as f32 / w as f32, ddy as f32 / h as f32);
        out.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let (gx, gy) = ((x as f32 + 0.5) * kx - 0.5, (y as f32 + 0.5) * ky - 0.5);
                let d = [bicubic(&fr.dye[0], ddx, ddy, gx, gy), bicubic(&fr.dye[1], ddx, ddy, gx, gy), bicubic(&fr.dye[2], ddx, ddy, gx, gy)];
                let c = match self.look.as_str() {
                    "ink" => {
                        // Beer–Lambert: transmittance exp(-absorbance × thickness).
                        let sum = d[0] + d[1] + d[2];
                        let mut a = d.map(|x| x * dens * 1.3);
                        if self.edge > 0.0 && sum > 1e-4 {
                            // Filament walls: relative gradient |∇ρ| / ρ (per dye cell).
                            let sx = |ox: f32, oy: f32| bilerp(&fr.dye[0], ddx, ddy, gx + ox, gy + oy) + bilerp(&fr.dye[1], ddx, ddy, gx + ox, gy + oy) + bilerp(&fr.dye[2], ddx, ddy, gx + ox, gy + oy);
                            let (ex, ey) = ((sx(1.0, 0.0) - sx(-1.0, 0.0)) * 0.5, (sx(0.0, 1.0) - sx(0.0, -1.0)) * 0.5);
                            let r = ((ex * ex + ey * ey).sqrt() / (sum + 0.08)).min(1.5);
                            a = a.map(|x| x * (1.0 + self.edge * r));
                        }
                        if self.chroma > 0.0 {
                            // Thin ink drifts towards `thin_color` (same total absorbance).
                            let thin = thin_abs;
                            let tot = a[0] + a[1] + a[2];
                            let wt = self.chroma * (-sum * dens * 1.5).exp();
                            for k in 0..3 {
                                a[k] += (thin[k] * tot - a[k]) * wt;
                            }
                        }
                        if self.granulation > 0.0 {
                            // Pigment settles in the paper's valleys: fine grain, in output pixels.
                            let g = paper.as_ref().map(|p| p.noise3(x as f64 * 0.35, y as f64 * 0.35, 0.0) as f32 * 0.6 + p.noise3(x as f64 * 0.9, y as f64 * 0.9, 3.0) as f32 * 0.4).unwrap_or(0.0);
                            let k = 1.0 + self.granulation * g * 1.6 * (1.0 - (-sum * 3.0).exp());
                            a = a.map(|x| x * k.max(0.0));
                        }
                        [bg[0] * (-a[0]).exp(), bg[1] * (-a[1]).exp(), bg[2] * (-a[2]).exp()]
                    }
                    "smoke" => {
                        let rho = (d[0] + d[1] + d[2]) / 3.0 * dens * 2.5;
                        let a = 1.0 - (-rho).exp();
                        let t = shadow_at(gx, gy);
                        let lit = [d[0], d[1], d[2]].map(|v| v / (d[0] + d[1] + d[2]).max(1e-6) * 3.0);
                        let l = 0.08 + 1.1 * t;
                        [bg[0] * (1.0 - a) + lit[0] * l * a, bg[1] * (1.0 - a) + lit[1] * l * a, bg[2] * (1.0 - a) + lit[2] * l * a]
                    }
                    _ => {
                        // Fire: blackbody from heat, soot from dye.
                        let ht = bilerp(&fr.heat, ddx, ddy, gx, gy);
                        let soot = (d[0] + d[1] + d[2]) * dens;
                        let a = 1.0 - (-soot * 1.5).exp();
                        let tk = (ht * 1.3).min(3.0);
                        let em = [tk.powf(1.5) * 2.2, tk.powf(2.4) * 1.1, tk.powf(4.0) * 0.45];
                        let t = shadow_at(gx, gy);
                        [bg[0] * (1.0 - a) + 0.05 * t * a + em[0], bg[1] * (1.0 - a) + 0.045 * t * a + em[1], bg[2] * (1.0 - a) + 0.04 * t * a + em[2]]
                    }
                };
                // Filmic shoulder for emission, then sRGB with a little dither.
                let hs = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
                let dith = ((hs >> 8) & 0xff) as f32 / 255.0 - 0.5;
                for k in 0..3 {
                    let v = c[k] / (1.0 + c[k] * 0.15);
                    row[x * 4 + k] = (srgb(v) * 255.0 + 0.5 + dith).clamp(0.0, 255.0) as u8;
                }
                row[x * 4 + 3] = 255;
            }
        });
        out
    }
}

/// The word as a dye mask on the fine grid (1 inside the letters).
pub(crate) fn word_mask(w: &Word, dx: usize, dy: usize, aspect: f32) -> Vec<f32> {
    let mut mask = vec![0.0f32; dx * dy];
    let Ok(mesh) = crate::scene3d::geom::text(&w.text, &w.font, w.weight, 0.02, 1.0, 0.1, 0.0, 0.5) else { return mask };
    // Centre the word's bounds on `pos`, cap height `size`.
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in mesh.pos.chunks_exact(3) {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    let s = w.size * dy as f32;
    let (cx, cy) = (w.pos[0] * dx as f32, w.pos[1] * dy as f32);
    let (mx, my) = ((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5);
    let _ = aspect;
    let px = |i: u32| -> (f32, f32) {
        let k = i as usize * 3;
        (cx + (mesh.pos[k] - mx) * s, cy - (mesh.pos[k + 1] - my) * s)
    };
    // Rasterise every triangle's footprint (the front faces cover the letters).
    for t in mesh.idx.chunks_exact(3) {
        let (a, b, c) = (px(t[0]), px(t[1]), px(t[2]));
        let (x0, x1) = (a.0.min(b.0).min(c.0).floor().max(0.0) as usize, (a.0.max(b.0).max(c.0).ceil() as usize).min(dx - 1));
        let (y0, y1) = (a.1.min(b.1).min(c.1).floor().max(0.0) as usize, (a.1.max(b.1).max(c.1).ceil() as usize).min(dy - 1));
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area.abs() < 1e-6 {
            continue;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (qx, qy) = (x as f32 + 0.5, y as f32 + 0.5);
                let e0 = (b.0 - a.0) * (qy - a.1) - (b.1 - a.1) * (qx - a.0);
                let e1 = (c.0 - b.0) * (qy - b.1) - (c.1 - b.1) * (qx - b.0);
                let e2 = (a.0 - c.0) * (qy - c.1) - (a.1 - c.1) * (qx - c.0);
                if (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0) {
                    mask[y * dx + x] = 1.0;
                }
            }
        }
    }
    // Soften the edge by one cell so the first frames don't alias.
    let m2 = mask.clone();
    for y in 1..dy - 1 {
        for x in 1..dx - 1 {
            let i = y * dx + x;
            mask[i] = (m2[i] * 4.0 + m2[i - 1] + m2[i + 1] + m2[i - dx] + m2[i + dx]) / 8.0;
        }
    }
    mask
}

fn color_bytes(c: Option<Color>) -> [u8; 17] {
    let mut bytes = [0; 17];
    if let Some(c) = c {
        bytes[0] = 1;
        for (i, value) in c.0 .0.into_iter().enumerate() {
            bytes[1 + i * 4..1 + (i + 1) * 4].copy_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    bytes
}

impl Fluid {
    /// Identity of this fluid's spec (cached; render-only fields are also hashed per frame by `render_key`).
    fn key(&self) -> u64 {
        *self.key_hash.get_or_init(|| self.compute_key())
    }

    fn compute_key(&self) -> u64 {
        let src: Vec<String> = self.sources.iter().map(|s| format!("{:?}{:?}{}{}{}{:?}{}{}{}{}", s.pos, s.to, s.radius, s.angle, s.speed, s.color.map(|c| c.0 .0), s.amount, s.heat, s.start, s.end)).collect();
        let word = self.text.as_ref().map(|w| format!("{}{}{}{:?}{}{:?}{}{}{}", w.text, w.font, w.weight, w.pos, w.size, w.color.map(|c| c.0 .0), w.amount, w.heat, w.at)).unwrap_or_default();
        let all = format!(
            "{}{:?}{:?}{:?}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            self.look,
            self.background.map(|c| c.0 .0),
            self.color.map(|c| c.0 .0),
            src,
            word,
            self.resolution,
            self.detail,
            self.vorticity,
            self.stir,
            self.stir_scale,
            self.buoyancy,
            self.weight,
            self.fade,
            self.cool,
            self.iterations,
            self.density,
            self.light,
            self.speed,
            self.seed
        );
        let more = format!("{}{}{}{}{:?}{}{}", self.advection, self.turbulence, self.octaves, self.edge, self.thin_color.map(|c| c.0 .0), self.chroma, self.granulation);
        fnv(&[all.as_bytes(), more.as_bytes()])
    }

    fn render_key(&self) -> u64 {
        let background = color_bytes(self.background);
        let thin_color = color_bytes(self.thin_color);
        let density = self.density.to_bits().to_le_bytes();
        let light = self.light.to_bits().to_le_bytes();
        let edge = self.edge.to_bits().to_le_bytes();
        let chroma = self.chroma.to_bits().to_le_bytes();
        let granulation = self.granulation.to_bits().to_le_bytes();
        fnv(&[
            self.look.as_bytes(),
            &background,
            &thin_color,
            &density,
            &light,
            &edge,
            &chroma,
            &granulation,
            &self.seed.to_le_bytes(),
        ])
    }

    fn frame_key(&self, frame: u64, fps: f32, w: usize, h: usize) -> u64 {
        fnv(&[
            b"fluid",
            &self.key().to_le_bytes(),
            &self.render_key().to_le_bytes(),
            &frame.to_le_bytes(),
            &fps.to_bits().to_le_bytes(),
            &(w as u32).to_le_bytes(),
            &(h as u32).to_le_bytes(),
        ])
    }
}

impl Component for Fluid {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let ps = crate::eval::shader::preview_scale();
        let (w, h) = ((ctx.width * ps).round().max(2.0) as usize, (ctx.height * ps).round().max(2.0) as usize);
        let aspect = ctx.width / ctx.height.max(1.0);
        let fps = ctx.fps.max(1.0);
        let frame = (t * fps).round().max(0.0) as u64;
        let s = self.sim(aspect);
        let key = self.frame_key(frame, fps, w, h);
        let image = ctx.assets.frame_image(key, w as u32, h as u32, &mut || {
            let fr = self.frame(s, frame, fps);
            self.render(s, &fr, w, h)
        });
        vec![Op::Image { image, dst: [0.0, 0.0, ctx.width, ctx.height], opacity: 1.0, transform: crate::dl::IDENTITY }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fluid(v: serde_json::Value) -> Fluid {
        let mut f: Fluid = serde_json::from_value(v).unwrap();
        f.prepare().unwrap();
        f
    }

    #[test]
    fn identical_at_any_thread_count() {
        let spec = serde_json::json!({ "look": "smoke", "resolution": 64, "detail": 2, "text": { "text": "AB", "heat": 0.8 } });
        let run = |threads: usize| {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                let f = fluid(spec.clone());
                let s = f.sim(16.0 / 9.0);
                let fr = f.frame(s, 40, 30.0);
                f.render(s, &fr, 160, 90)
            })
        };
        assert_eq!(run(1), run(4));
    }

    #[test]
    fn frames_in_any_order_match() {
        let spec = serde_json::json!({ "look": "ink", "resolution": 48, "detail": 2, "sources": [ { "pos": [0.2, 0.5], "angle": 0, "speed": 0.8, "end": 2 } ] });
        let (a, b) = (fluid(spec.clone()), fluid(spec));
        let (sa, sb) = (a.sim(1.5), b.sim(1.5));
        let x = a.frame(sa, 30, 30.0);
        let _ = b.frame(sb, 45, 30.0);
        let y = b.frame(sb, 30, 30.0);
        assert_eq!(x.dye[0], y.dye[0]);
    }

    #[test]
    fn a_jet_carries_dye_downstream() {
        let f = fluid(serde_json::json!({ "look": "ink", "resolution": 64, "detail": 1, "stir": 0, "sources": [ { "pos": [0.15, 0.5], "angle": 0, "speed": 1.0, "radius": 0.05, "end": 3 } ] }));
        let s = f.sim(1.0);
        let fr = f.frame(s, 60, 30.0);
        let (w, h) = (s.dx, s.dy);
        let right: f32 = (0..h).map(|y| fr.dye[0][y * w + w * 6 / 10]).sum();
        assert!(right > 0.01, "no dye reached x = 0.6: {right}");
    }

    #[test]
    fn cached_key_matches_spec() {
        let spec = serde_json::json!({ "look": "ink", "text": { "text": "AB" }, "sources": [ { "pos": [0.2, 0.5] } ] });
        let a = fluid(spec.clone());
        assert_eq!(a.key(), a.compute_key());
        assert_eq!(a.key(), a.key());
        assert_eq!(a.key(), fluid(spec).key());
        let b = fluid(serde_json::json!({ "look": "ink", "text": { "text": "AC" }, "sources": [ { "pos": [0.2, 0.5] } ] }));
        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn render_fields_change_frame_key_after_first_use() {
        let mut f = fluid(serde_json::json!({"look":"ink","resolution":48,"detail":1}));
        let k0 = f.frame_key(10, 30.0, 160, 90);
        f.background = Some(Color::hex("#000000"));
        let k1 = f.frame_key(10, 30.0, 160, 90);
        assert_ne!(k0, k1);
        f.density = 2.0;
        assert_ne!(k1, f.frame_key(10, 30.0, 160, 90));
        assert_eq!(f.frame_key(10, 30.0, 160, 90), f.frame_key(10, 30.0, 160, 90));
    }
}
