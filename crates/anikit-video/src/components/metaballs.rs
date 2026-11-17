//! `metaballs`: liquid blobs that drift, merge and split.
//!
//! The scalar field `Σ r²/d²` is sampled on a coarse grid (about 14 px cells
//! at 1080p) and its iso-contour is extracted with marching squares into one
//! vector path, so edges stay crisp and anti-aliased at any resolution while
//! a frame costs a few thousand field samples. Cells fully inside are merged
//! into row runs; all cell polygons share one winding, so interior edges
//! cancel exactly in the rasteriser.

use super::fxkit::{self, hot, kelvin};
use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Blend, FillRule, Filter, Op, Paint, Path, Rgba};
use serde::Deserialize;
use std::f32::consts::TAU;

/// Inside parts of the grid cells where `v >= iso`, as one path.
/// `v` holds `(nx + 1) × (ny + 1)` corner samples, row-major.
fn contour(v: &[f32], nx: usize, ny: usize, cell: f32, iso: f32, off: (f32, f32)) -> Path {
    let mut p = Path::new();
    let at = |i: usize, j: usize| v[j * (nx + 1) + i];
    let pt = |x: f32, y: f32| ((x * cell + off.0) as f64, (y * cell + off.1) as f64);
    for j in 0..ny {
        let mut run: Option<usize> = None;
        let flush = |p: &mut Path, run: &mut Option<usize>, end: usize| {
            if let Some(s) = run.take() {
                p.move_to(pt(s as f32, j as f32));
                p.line_to(pt(end as f32, j as f32));
                p.line_to(pt(end as f32, (j + 1) as f32));
                p.line_to(pt(s as f32, (j + 1) as f32));
                p.close_path();
            }
        };
        for i in 0..nx {
            // Corners clockwise on screen: TL, TR, BR, BL.
            let c = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            let ins = c.map(|x| x >= iso);
            if ins.iter().all(|&b| b) {
                run.get_or_insert(i);
                continue;
            }
            flush(&mut p, &mut run, i);
            if !ins.iter().any(|&b| b) {
                continue;
            }
            let pos = [(i as f32, j as f32), ((i + 1) as f32, j as f32), ((i + 1) as f32, (j + 1) as f32), (i as f32, (j + 1) as f32)];
            let mut first = true;
            for k in 0..4 {
                let n = (k + 1) % 4;
                if ins[k] {
                    let q = pt(pos[k].0, pos[k].1);
                    if first {
                        p.move_to(q);
                        first = false;
                    } else {
                        p.line_to(q);
                    }
                }
                if ins[k] != ins[n] {
                    let f = ((iso - c[k]) / (c[n] - c[k])).clamp(0.0, 1.0);
                    let q = pt(pos[k].0 + (pos[n].0 - pos[k].0) * f, pos[k].1 + (pos[n].1 - pos[k].1) * f);
                    if first {
                        p.move_to(q);
                        first = false;
                    } else {
                        p.line_to(q);
                    }
                }
            }
            p.close_path();
        }
        flush(&mut p, &mut run, nx);
    }
    p
}

/// Iso level of the liquid's edge.
const ISO: f32 = 0.25;

struct Blob {
    amp: [f32; 2],
    freq: [f32; 2],
    phase: [f32; 2],
    r: f32,
}

const MATERIALS: &[&str] = &["glossy", "glass", "chrome", "matte"];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Metaballs {
    /// Number of blobs.
    pub count: u32,
    /// Body colours (1–4, top-left → bottom-right). Default: shades of the
    /// theme accent (`glossy`, `matte`), neutral (`glass`, `chrome`).
    pub colors: Vec<Color>,
    /// `glossy` (liquid with rim light and highlights), `glass` (clear with a
    /// bright rim and caustics), `chrome` (mirror bands) or `matte` (soft clay).
    pub material: String,
    /// Blob radius as a fraction of the box height.
    pub size: f32,
    /// How far blobs wander (fraction of the box).
    pub spread: f32,
    pub speed: f32,
    /// Soft coloured glow around the liquid.
    pub glow: bool,
    /// Specular highlights.
    pub gloss: bool,
    /// Rim light strength (0..2).
    pub rim: f32,
    /// Grid cell in px at 1080p (smaller = smoother, slower).
    pub detail: f32,
    pub seed: u32,
    #[serde(skip)]
    blobs: Vec<Blob>,
}

impl Default for Metaballs {
    fn default() -> Self {
        Metaballs {
            count: 9,
            colors: Vec::new(),
            material: "glossy".into(),
            size: 0.085,
            spread: 1.0,
            speed: 1.0,
            glow: true,
            gloss: true,
            rim: 1.0,
            detail: 7.0,
            seed: 3,
            blobs: Vec::new(),
        }
    }
}

impl Prepare for Metaballs {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.count == 0 || self.count > 64 {
            return Err(Error("count must be 1..64".into()));
        }
        fxkit::choice("material", &self.material, MATERIALS)?;
        self.detail = self.detail.clamp(3.0, 64.0);
        self.rim = self.rim.clamp(0.0, 2.0);
        let mut rng = Rng::new(self.seed as u64 * 31 + 7);
        self.blobs = (0..self.count)
            .map(|i| Blob {
                amp: [rng.range(0.10, 0.36), rng.range(0.08, 0.28)],
                freq: [rng.range(0.18, 0.42) * if i % 2 == 0 { 1.0 } else { -1.0 }, rng.range(0.15, 0.38)],
                phase: [rng.f() * TAU, rng.f() * TAU],
                r: rng.range(0.55, 1.35),
            })
            .collect();
        Ok(())
    }
}

impl Metaballs {
    /// Blob centres and radii (px) at `t`.
    fn blobs_at(&self, t: f32, w: f32, h: f32) -> Vec<(f32, f32, f32)> {
        let t = t * self.speed;
        self.blobs
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let x = w * (0.5 + self.spread * b.amp[0] * (b.freq[0] * t * TAU * 0.25 + b.phase[0]).sin());
                let y = h * (0.5 + self.spread * b.amp[1] * (b.freq[1] * t * TAU * 0.25 + b.phase[1]).sin() * 1.3);
                let r = self.size * h * b.r * (1.0 + 0.08 * (t * 0.9 + i as f32).sin());
                (x, y, r)
            })
            .collect()
    }
}

impl Component for Metaballs {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = step(t, AMBIENT_RATE);
        let (w, h) = (ctx.width, ctx.height);
        let k = h / 1080.0;
        let cell = (self.detail * k).max(2.0);
        let (nx, ny) = ((w / cell).ceil() as usize, (h / cell).ceil() as usize);
        let blobs = self.blobs_at(t, w, h);
        // Wyvill kernel (1 - d²/R²)³: compact, so blobs only fuse when close,
        // with the classic gooey bridge. ISO is reached at d ≈ 0.61 R.
        let mut v = vec![0f32; (nx + 1) * (ny + 1)];
        for b in &blobs {
            let big = b.2 / 0.608;
            let inv = 1.0 / (big * big);
            let (i0, i1) = (((b.0 - big) / cell).floor().max(0.0) as usize, (((b.0 + big) / cell).ceil().max(0.0) as usize).min(nx));
            let (j0, j1) = (((b.1 - big) / cell).floor().max(0.0) as usize, (((b.1 + big) / cell).ceil().max(0.0) as usize).min(ny));
            for j in j0..=j1.max(j0) {
                if j > ny {
                    break;
                }
                let dy = j as f32 * cell - b.1;
                let dy2 = dy * dy;
                let row = &mut v[j * (nx + 1)..(j + 1) * (nx + 1)];
                for i in i0..=i1.max(i0).min(nx) {
                    let dx = i as f32 * cell - b.0;
                    let q = (1.0 - (dx * dx + dy2) * inv).max(0.0);
                    row[i] += q * q * q;
                }
            }
        }
        let body = contour(&v, nx, ny, cell, ISO, (0.0, 0.0));
        if body.is_empty() {
            return Vec::new();
        }
        let mat = self.material.as_str();
        let th = ctx.theme;
        let cols: Vec<Rgba> = fxkit::colors_or(&self.colors, || match mat {
            "glass" => vec![mix(th.text, kelvin(7500.0), 0.3)],
            "chrome" => vec![Rgba([0.86, 0.88, 0.92, 1.0])],
            _ => vec![shade(th.accent, 0.05), shade(th.accent, -0.3), shade(th.accent, -0.6)],
        });
        let n = (cols.len() - 1).max(1) as f32;
        // Gradient axis drifts slowly so colour flows through the liquid.
        let ga = (55.0 + 15.0 * (t * 0.3).sin()).to_radians();
        let (gs, gc) = ga.sin_cos();
        let half = 0.5 * (w * gc.abs() + h * gs.abs()) * 0.8;
        let lin = |stops: Vec<(f32, Rgba)>| Paint::Linear { p0: [w * 0.5 - gc * half, h * 0.5 - gs * half], p1: [w * 0.5 + gc * half, h * 0.5 + gs * half], stops };
        let ramp_at = |x: f32, y: f32| fxkit::ramp(&cols, clamp01(((x - w * 0.5) * gc + (y - h * 0.5) * gs) / (2.0 * half) + 0.5));
        let mean_r = blobs.iter().map(|b| b.2).sum::<f32>() / blobs.len() as f32;
        let white = Rgba([1.0, 1.0, 1.0, 1.0]);
        let mut out = Vec::new();

        // Glow: the body itself, blurred and added.
        if self.glow && mat != "chrome" {
            let a = if mat == "glass" { 0.18 } else { 0.32 };
            push_layer(&mut out, 1.0, if th.dark { Blend::Add } else { Blend::Normal }, vec![Filter::Blur(mean_r * 0.45)]);
            let stops: Vec<(f32, Rgba)> = cols.iter().enumerate().map(|(i, c)| (i as f32 / n, alpha(*c, a))).collect();
            out.push(fill(body.clone(), lin(stops)));
            out.push(Op::PopLayer);
        }

        // Base body.
        let base_stops: Vec<(f32, Rgba)> = cols
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let c = match mat {
                    "glass" => alpha(*c, 0.1),
                    "chrome" => shade(*c, -0.55),
                    _ => *c,
                };
                (i as f32 / n, c)
            })
            .collect();
        out.push(fill(body.clone(), lin(base_stops)));

        // Everything else is lighting inside the liquid: clip to the body.
        // Higher iso-levels of the same field are nested "terraces" that
        // follow the merged shape exactly; blurred, they shade its volume.
        let iso = |lv: f32, dx: f32, dy: f32| contour(&v, nx, ny, cell, lv, (dx * mean_r, dy * mean_r));
        let even_odd = |a: Path, b: Path| {
            let mut p = a;
            p.extend(b);
            Op::Fill { path: p, paint: solid(white), rule: FillRule::EvenOdd, transform: crate::dl::IDENTITY }
        };
        let tint = |op: Op, c: Rgba| match op {
            Op::Fill { path, rule, transform, .. } => Op::Fill { path, paint: solid(c), rule, transform },
            o => o,
        };
        out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: Some((body.clone(), crate::dl::IDENTITY)), cache_key: None });
        let c0 = cols[0];
        match mat {
            "chrome" => {
                // Mirror bands: sky above a dark horizon, ground light below,
                // offset up-left like a reflected studio.
                push_layer(&mut out, 1.0, Blend::Normal, vec![Filter::Blur(mean_r * 0.035)]);
                for (lv, dx, dy, c) in [
                    (0.3, -0.04, -0.06, mix(Rgba([0.55, 0.56, 0.6, 1.0]), c0, 0.3)),
                    (0.42, -0.1, -0.16, mix(Rgba([0.08, 0.08, 0.1, 1.0]), c0, 0.15)),
                    (0.58, -0.14, -0.24, mix(Rgba([0.82, 0.86, 0.94, 1.0]), c0, 0.3)),
                    (0.8, -0.17, -0.3, mix(white, c0, 0.15)),
                ] {
                    out.push(fill(iso(lv, dx, dy), solid(c)));
                }
                out.push(Op::PopLayer);
            }
            "glass" => {
                // Caustic: light focused through the body, opposite the key light.
                push_layer(&mut out, 1.0, Blend::Screen, vec![Filter::Blur(mean_r * 0.2)]);
                out.push(fill(iso(0.6, 0.18, 0.24), solid(alpha(hot(c0, 0.5), 0.35))));
                out.push(Op::PopLayer);
            }
            _ => {
                // Inner glow (glossy) or soft clay light (matte).
                let (blend, a) = if mat == "matte" { (Blend::Normal, 0.22) } else { (Blend::Screen, 0.32) };
                push_layer(&mut out, 1.0, blend, vec![Filter::Blur(mean_r * 0.22)]);
                let (dx, dy) = if mat == "matte" { (-0.18, -0.24) } else { (0.06, 0.1) };
                for lv in [0.45, 0.7, 1.0] {
                    out.push(fill(iso(lv, dx, dy), solid(alpha(hot(ramp_at(w * 0.5, h * 0.5), 0.4), a))));
                }
                out.push(Op::PopLayer);
            }
        }
        // Shadow side: a broad gradient darkening the lower right of the whole body.
        if mat != "glass" {
            out.push(fill(body.clone(), Paint::Linear {
                p0: [w * 0.5 - gc * half, h * 0.5 - gs * half],
                p1: [w * 0.5 + gc * half, h * 0.5 + gs * half],
                stops: vec![(0.0, Rgba([0.0, 0.0, 0.0, 0.0])), (0.6, Rgba([0.0, 0.0, 0.0, 0.1])), (1.0, Rgba([0.0, 0.0, 0.0, 0.3]))],
            }));
        }
        // Rim light (fresnel): the band between the edge and a slightly
        // higher iso-level, softened.
        if self.rim > 0.0 && mat != "matte" {
            let (rc, ra) = match mat {
                "glass" => (white, 0.9),
                "chrome" => (white, 0.4),
                _ => (hot(c0, 0.6), 0.55),
            };
            push_layer(&mut out, (ra * self.rim).min(1.0), Blend::Screen, vec![Filter::Blur(mean_r * 0.06)]);
            out.push(tint(even_odd(body.clone(), iso(0.4, 0.0, 0.0)), rc));
            out.push(Op::PopLayer);
        }
        // Speculars: a crisp window highlight and a broad soft sheen per blob.
        if self.gloss && mat != "matte" {
            for &(x, y, r) in &blobs {
                let (hx, hy) = (x - 0.36 * r, y - 0.4 * r);
                out.push(fxkit::ellipse(hx, hy, 0.5 * r, 0.32 * r, -0.6, vec![(0.0, alpha(white, 0.28)), (0.6, alpha(white, 0.08)), (1.0, alpha(white, 0.0))]));
                out.push(fxkit::ellipse(hx + 0.05 * r, hy + 0.04 * r, 0.16 * r, 0.085 * r, -0.6, vec![(0.0, alpha(white, 0.95)), (0.6, alpha(white, 0.8)), (1.0, alpha(white, 0.0))]));
                // A small bounce light on the opposite side.
                out.push(fxkit::ellipse(x + 0.4 * r, y + 0.44 * r, 0.2 * r, 0.08 * r, -0.6, vec![(0.0, alpha(white, 0.22)), (1.0, alpha(white, 0.0))]));
            }
        }
        out.push(Op::PopLayer);
        let _ = k;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use super::*;
    use serde_json::json;

    #[test]
    fn contour_covers_inside() {
        // A single blob of radius 10 at the centre of a 40×40 grid of 1 px cells.
        let (nx, ny) = (40, 40);
        let mut v = vec![0f32; 41 * 41];
        for j in 0..=40 {
            for i in 0..=40 {
                let d2 = ((i as f32 - 20.0).powi(2) + (j as f32 - 20.0).powi(2)).max(1e-3);
                v[j * 41 + i] = 100.0 / d2;
            }
        }
        use kurbo::Shape;
        let p = contour(&v, nx, ny, 1.0, 1.0, (0.0, 0.0));
        let a = p.area().abs();
        assert!((a - std::f64::consts::PI * 100.0).abs() < 8.0, "{a}");
    }

    #[test]
    fn gallery() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("metaballs", &json!({})).unwrap();
        for t in [0.0f32, 2.0, 5.0] {
            let ops = c.eval(t, 960.0, 540.0, 30.0, &th, a);
            preview::validate(&ops).unwrap();
            preview::snapshot(&format!("metaballs-{t}"), 960, 540, th.background, ops, a);
        }
        let ops = c.eval(3.0, 1920.0, 1080.0, 30.0, &th, a);
        preview::snapshot("metaballs-1080", 1920, 1080, th.background, ops, a);
    }
}
