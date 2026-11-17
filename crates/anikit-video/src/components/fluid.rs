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
    #[serde(skip)]
    sim: OnceLock<Sim>,
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
            sim: OnceLock::new(),
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
    live: Option<Fields>,
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
        if self.look != "ink" && self.buoyancy == 0.0 {
            self.buoyancy = 0.9;
        }
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
    acc.max(0.0)
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
        Sim { nx, ny, dx, dy, word, state: Mutex::new(SimState { live: None, frames: HashMap::new(), checkpoints: Vec::new() }) }
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
                            f.dye[c][i] += col[c] * w.amount * m * if self.look == "ink" { 1.0 } else { 1.0 };
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
        // 3. Advect velocity (semi-Lagrangian), in cells.
        let (su, sv) = (nu, nv);
        let mut au = vec![0.0f32; nx * ny];
        let mut av = vec![0.0f32; nx * ny];
        au.par_chunks_mut(nx).zip(av.par_chunks_mut(nx)).enumerate().for_each(|(y, (ru, rv))| {
            for x in 0..nx {
                let i = y * nx + x;
                let (px, py) = (x as f32 - su[i] * dt, y as f32 - sv[i] * dt);
                ru[x] = bilerp(&su, nx, ny, px, py);
                rv[x] = bilerp(&sv, nx, ny, px, py);
            }
        });
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
        let adv = |src: &Vec<f32>, k: f32| -> Vec<f32> {
            let mut out = vec![0.0f32; ddx * ddy];
            out.par_chunks_mut(ddx).enumerate().for_each(|(y, row)| {
                for x in 0..ddx {
                    // Velocity at this fine cell (coarse cell coordinates).
                    let (cx, cy) = ((x as f32 + 0.5) / det - 0.5, (y as f32 + 0.5) / det - 0.5);
                    let (vx, vy) = (bilerp(u, nx, ny, cx, cy), bilerp(v, nx, ny, cx, cy));
                    let (px, py) = (x as f32 - vx * dt * det, y as f32 - vy * dt * det);
                    // Outside the frame is clear water.
                    let inside = px > -0.5 && py > -0.5 && px < ddx as f32 - 0.5 && py < ddy as f32 - 0.5;
                    row[x] = if inside { bilerp(src, ddx, ddy, px, py) * k } else { 0.0 };
                }
            });
            out
        };
        f.dye = [adv(&f.dye[0], keep), adv(&f.dye[1], keep), adv(&f.dye[2], keep)];
        if self.look != "ink" || self.text.as_ref().is_some_and(|w| w.heat > 0.0) || self.sources.iter().any(|s| s.heat > 0.0) {
            f.heat = adv(&f.heat, hkeep);
        }
        f.step += 1;
    }

    /// The fields at video frame `frame` (`fps`), stepping as needed.
    fn frame(&self, s: &Sim, frame: u64, fps: f32) -> Arc<Frame> {
        let target = ((frame as f32 / fps) * HZ * self.speed).round() as u64;
        let mut st = s.state.lock().unwrap();
        if let Some(fr) = st.frames.get(&target) {
            return fr.clone();
        }
        let noise = Noise::new(self.seed);
        let start = match st.live.take() {
            Some(l) if l.step <= target => l,
            ahead => {
                st.live = ahead;
                st.checkpoints.iter().filter(|c| c.step <= target).max_by_key(|c| c.step).cloned().unwrap_or_else(|| self.init(s))
            }
        };
        let st_ref = &mut *st;
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
        if st.live.as_ref().is_none_or(|l| l.step <= out.step) {
            st.live = Some(out);
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
        let shadow: Vec<f32> = if self.look != "ink" {
            let a = self.light.to_radians();
            let (lx, ly) = (-a.cos(), -a.sin());
            let steps = 24;
            let stepl = (ddy as f32 * 0.012).max(1.0);
            (0..ddx * ddy)
                .into_par_iter()
                .map(|i| {
                    let (x, y) = ((i % ddx) as f32, (i / ddx) as f32);
                    let mut od = 0.0;
                    for k in 1..=steps {
                        let (px, py) = (x + lx * stepl * k as f32, y + ly * stepl * k as f32);
                        if px < 0.0 || py < 0.0 || px >= ddx as f32 || py >= ddy as f32 {
                            break;
                        }
                        od += (bilerp(&fr.dye[0], ddx, ddy, px, py) + bilerp(&fr.dye[1], ddx, ddy, px, py) + bilerp(&fr.dye[2], ddx, ddy, px, py)) * stepl;
                    }
                    (-od * dens * 0.02).exp()
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut out = vec![0u8; w * h * 4];
        let (kx, ky) = (ddx as f32 / w as f32, ddy as f32 / h as f32);
        out.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let (gx, gy) = ((x as f32 + 0.5) * kx - 0.5, (y as f32 + 0.5) * ky - 0.5);
                let d = [bicubic(&fr.dye[0], ddx, ddy, gx, gy), bicubic(&fr.dye[1], ddx, ddy, gx, gy), bicubic(&fr.dye[2], ddx, ddy, gx, gy)];
                let c = match self.look.as_str() {
                    "ink" => {
                        // Beer–Lambert on paper, plus a faint sheen where filaments are thin and sharp.
                        let sum = d[0] + d[1] + d[2];
                        let e = 1.0;
                        let gxr = bilerp(&fr.dye[2], ddx, ddy, gx + e, gy) - bilerp(&fr.dye[2], ddx, ddy, gx - e, gy);
                        let gyr = bilerp(&fr.dye[2], ddx, ddy, gx, gy + e) - bilerp(&fr.dye[2], ddx, ddy, gx, gy - e);
                        let edge = ((gxr * gxr + gyr * gyr).sqrt() * 0.6).min(0.12) * (1.0 - (-sum * 2.0).exp());
                        let mut o = [0.0f32; 3];
                        for k in 0..3 {
                            o[k] = bg[k] * (-d[k] * dens * 1.3).exp() * (1.0 - edge) + edge * 0.04;
                        }
                        o
                    }
                    "smoke" => {
                        let rho = (d[0] + d[1] + d[2]) / 3.0 * dens * 2.5;
                        let a = 1.0 - (-rho).exp();
                        let t = bilerp(&shadow, ddx, ddy, gx, gy);
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
                        let t = bilerp(&shadow, ddx, ddy, gx, gy);
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
fn word_mask(w: &Word, dx: usize, dy: usize, aspect: f32) -> Vec<f32> {
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

impl Fluid {
    /// Identity of this fluid's look and motion (image cache key).
    fn key(&self) -> u64 {
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
        fnv(&[all.as_bytes()])
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
        let key = fnv(&[b"fluid", &self.key().to_le_bytes(), &frame.to_le_bytes(), &fps.to_bits().to_le_bytes(), &(w as u32).to_le_bytes(), &(h as u32).to_le_bytes()]);
        let image = ctx.assets.image(key, w as u32, h as u32, &mut || {
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
}
