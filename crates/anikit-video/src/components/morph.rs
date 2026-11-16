//! `morph`: smooth shape morphing between named shapes and SVG paths.
//!
//! Every shape is normalised to a unit box (centred, similar visual area),
//! then `anikit_core::morph::prepare_many` gives all of them one structure
//! (same subpaths and cubic counts, rotated to minimise travel), once, in
//! `prepare`. A frame is a plain interpolation of control points.

use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Blend, Op, Paint, Path, Rgba};
use anikit_core::morph::{prepare_many, MorphSeq};
use anikit_core::path::Path as CPath;
use serde::Deserialize;
use std::f32::consts::PI;

const NAMES: &[&str] = &["circle", "square", "star", "heart", "blob", "triangle", "hexagon", "diamond"];

const HEART: &str = "M50,92 C22,72 0,54 0,31 C0,13 14,0 30,0 C40,0 47,6 50,15 C53,6 60,0 70,0 C86,0 100,13 100,31 C100,54 78,72 50,92 Z";

fn named(name: &str, seed: u32) -> Option<CPath> {
    Some(match name {
        "circle" => CPath::ellipse(0.0, 0.0, 1.0, 1.0),
        "square" => CPath::rect(-1.0, -1.0, 2.0, 2.0, 0.28, 0.28),
        "star" => CPath::star(0.0, 0.0, 5, 1.0, 0.45, 0.0),
        "heart" => CPath::parse(HEART),
        "triangle" => {
            let p: Vec<f64> = (0..3).flat_map(|i| {
                let a = (-90.0 + 120.0 * i as f64).to_radians();
                [a.cos(), a.sin()]
            }).collect();
            CPath::polygon(&p, true)
        }
        "hexagon" => {
            let p: Vec<f64> = (0..6).flat_map(|i| {
                let a = (60.0 * i as f64).to_radians();
                [a.cos(), a.sin()]
            }).collect();
            CPath::polygon(&p, true)
        }
        "diamond" => CPath::polygon(&[0.0, -1.0, 0.75, 0.0, 0.0, 1.0, -0.75, 0.0], true),
        "blob" => {
            let mut rng = Rng::new(seed as u64 * 977 + 13);
            let n = 7;
            let p: Vec<f64> = (0..n)
                .flat_map(|i| {
                    let a = (i as f64 / n as f64) * std::f64::consts::TAU;
                    let r = 0.78 + 0.32 * rng.f() as f64;
                    [a.cos() * r, a.sin() * r]
                })
                .collect();
            CPath::through_points(&p, 1.0, true)
        }
        _ => return None,
    })
}

/// Centre in a unit box and scale to a similar visual weight.
fn normalise(mut p: CPath) -> CPath {
    let [x0, y0, x1, y1] = p.bounds();
    let (w, h) = ((x1 - x0).max(1e-9), (y1 - y0).max(1e-9));
    let fit = 2.0 / w.max(h);
    p.transform([fit, 0.0, 0.0, fit, -(x0 + w * 0.5) * fit, -(y0 + h * 0.5) * fit]);
    let area = p.area().abs().max(1e-6);
    // Squares look much bigger than stars at the same bounds: even it out.
    let s = (std::f64::consts::PI * 0.82 / area).sqrt().min(1.12);
    p.transform([s, 0.0, 0.0, s, 0.0, 0.0]);
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Morph {
    /// Shape names (`circle`, `square`, `star`, `heart`, `blob`, `triangle`,
    /// `hexagon`, `diamond`) or SVG path data, in order.
    pub shapes: Vec<String>,
    /// Size as a fraction of the box's shorter side.
    pub size: f32,
    /// Fill gradient (2–3 colours). Default: theme accent → palette pink.
    pub colors: Vec<Color>,
    /// Seconds per morph.
    pub duration: f32,
    /// Seconds each shape holds.
    pub hold: f32,
    /// Morph back to the first shape and repeat.
    #[serde(rename = "loop")]
    pub repeat: bool,
    /// Degrees each morph turns the shape (adds life; 0 = none).
    pub turn: f32,
    /// Soft coloured glow behind the shape.
    pub glow: bool,
    /// Optional outline colour.
    pub stroke: Option<Color>,
    pub delay: f32,
    pub seed: u32,
    #[serde(skip)]
    seq: Option<MorphSeq>,
}

impl Default for Morph {
    fn default() -> Self {
        Morph { shapes: Vec::new(), size: 0.62, colors: Vec::new(), duration: 0.9, hold: 0.7, repeat: true, turn: 0.0, glow: true, stroke: None, delay: 0.0, seed: 1, seq: None }
    }
}

impl Prepare for Morph {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.shapes.is_empty() {
            self.shapes = vec!["circle".into(), "star".into(), "heart".into(), "blob".into()];
        }
        let mut paths = Vec::new();
        for (i, s) in self.shapes.iter().enumerate() {
            let key = s.trim().to_ascii_lowercase();
            let p = match named(&key, self.seed + i as u32) {
                Some(p) => p,
                None if key.starts_with(['m', 'M']) => CPath::parse(s),
                None => {
                    return Err(Error(match closest(&key, NAMES) {
                        Some(n) => format!("unknown shape `{s}`; did you mean `{n}`? (or give SVG path data starting with M)"),
                        None => format!("unknown shape `{s}`; use one of {} or SVG path data", NAMES.join(", ")),
                    }))
                }
            };
            if p.subpaths.is_empty() {
                return Err(Error(format!("shape {i} has no path data")));
            }
            paths.push(normalise(p));
        }
        if self.repeat && paths.len() > 1 {
            paths.push(paths[0].clone());
        }
        self.duration = self.duration.max(0.05);
        self.hold = self.hold.max(0.0);
        self.seq = Some(prepare_many(&paths));
        Ok(())
    }
}

impl Morph {
    /// (segment index, eased progress within the morph, raw progress).
    fn phase(&self, t: f32) -> (usize, f32, f32) {
        let seq = self.seq.as_ref().unwrap();
        let segs = seq.frames.len().saturating_sub(1);
        if segs == 0 {
            return (0, 0.0, 0.0);
        }
        let cyc = self.hold + self.duration;
        let t = (t - self.delay).max(0.0);
        let total = segs as f32 * cyc;
        let t = if self.repeat { t % total } else { t.min(total - 1e-4) };
        let i = ((t / cyc) as usize).min(segs - 1);
        let q = clamp01((t - i as f32 * cyc - self.hold) / self.duration);
        (i, ease_in_out(q), q)
    }
}

impl Component for Morph {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let seq = self.seq.as_ref().unwrap();
        let (i, e, q) = self.phase(t);
        let a = &seq.frames[i];
        let b = seq.frames.get(i + 1).unwrap_or(a);
        let (w, h) = (ctx.width, ctx.height);
        let (cx, cy) = (w * 0.5, h * 0.5);
        // Squash and stretch through the morph, a little turn.
        let pulse = (q * PI).sin();
        let r = w.min(h) * 0.5 * self.size * (1.0 + 0.05 * pulse);
        let rot = (self.turn * (i as f32 + e) + 6.0 * pulse).to_radians();
        let (sn, cs) = rot.sin_cos();
        let m = [cs * r, sn * r, -sn * r, cs * r, cx, cy];
        let tr = |x: f64, y: f64| -> (f64, f64) {
            let (x, y) = (x as f32, y as f32);
            ((m[0] * x + m[2] * y + m[4]) as f64, (m[1] * x + m[3] * y + m[5]) as f64)
        };
        let mut path = Path::new();
        let mut at = 0;
        let ef = e as f64;
        let v = |k: usize| a[k] + (b[k] - a[k]) * ef;
        for &(n, closed) in &seq.template {
            path.move_to(tr(v(at), v(at + 1)));
            at += 2;
            for _ in 0..n {
                path.curve_to(tr(v(at), v(at + 1)), tr(v(at + 2), v(at + 3)), tr(v(at + 4), v(at + 5)));
                at += 6;
            }
            if closed {
                path.close_path();
            }
        }
        let cols: Vec<Rgba> = if self.colors.is_empty() { vec![shade(ctx.theme.accent, 0.1), ctx.theme.pal(2), shade(ctx.theme.pal(4), 0.1)] } else { self.colors.iter().map(|c| c.0).collect() };
        let n = (cols.len() - 1).max(1) as f32;
        let stops: Vec<(f32, Rgba)> = cols.iter().enumerate().map(|(i, c)| (i as f32 / n, *c)).collect();
        // The gradient turns slowly so the fill feels alive.
        let ga = (-50.0 + 20.0 * (t * 0.4).sin()).to_radians();
        let (gs, gc) = ga.sin_cos();
        let paint = Paint::Linear { p0: [cx - gc * r * 1.1, cy - gs * r * 1.1], p1: [cx + gc * r * 1.1, cy + gs * r * 1.1], stops };
        let mut out = Vec::new();
        if self.glow {
            let gcol = cols[cols.len() / 2];
            push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
            out.push(circle(cx, cy, r * 1.9, radial(cx, cy, r * 1.9, vec![(0.0, alpha(gcol, 0.5)), (0.45, alpha(gcol, 0.16)), (1.0, alpha(gcol, 0.0))])));
            out.push(Op::PopLayer);
        }
        out.push(fill(path.clone(), paint));
        // Glossy top light: a soft white radial clipped by the shape itself.
        out.push(fill(path.clone(), radial(cx - r * 0.35, cy - r * 0.55, r * 1.2, vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.22])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))])));
        if let Some(s) = self.stroke {
            out.push(stroke_op(path, solid(s.0), (r * 0.02).max(1.0), [0.0, 1.0]));
        }
        out
    }

    fn duration(&self) -> Option<f32> {
        if self.repeat {
            return None;
        }
        let segs = self.seq.as_ref().map_or(0, |s| s.frames.len().saturating_sub(1));
        Some(self.delay + segs as f32 * (self.hold + self.duration))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use serde_json::json;

    #[test]
    fn morphs_and_snapshots() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("morph", &json!({"shapes": ["circle", "star", "heart", "M0,0 L10,0 L5,8 Z"], "loop": false})).unwrap();
        let end = c.component.duration().unwrap();
        for (k, t) in [0.0f32, 1.1, 1.6, 2.2, end * 0.5, end, end + 1.0].iter().enumerate() {
            let ops = c.eval(*t, 960.0, 540.0, 30.0, &th, a);
            preview::validate(&ops).unwrap();
            if k < 4 {
                preview::snapshot(&format!("morph-{k}"), 960, 540, th.background, ops, a);
            }
        }
        let c = build("morph", &json!({})).unwrap();
        assert!(c.component.duration().is_none());
        let ops = c.eval(3.0, 960.0, 540.0, 30.0, &th, a);
        preview::validate(&ops).unwrap();
        preview::snapshot("morph-blob", 960, 540, th.background, ops, a);
        let e = build("morph", &json!({"shapes": ["hart"]})).err().unwrap();
        assert!(e.0.contains("heart"), "{e}");
    }
}
