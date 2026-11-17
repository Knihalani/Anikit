//! Finishing helpers shared by the UI kit, content blocks, charts and code:
//! layered elevation (contact + key + ambient shadows), lit bevels, glass,
//! optical type (Inter Display above ~28 px with Inter's dynamic tracking),
//! masked line reveals, gradient / sheen repaints of laid-out text and a
//! cheap motion smear for fast moves.

use super::style::*;
use super::text::{Face, TextStyle};
use super::{transform_ops, Theme};
use crate::dl::{self, Blend, Filter, Op, Paint, Rgba, IDENTITY};

// ---------------------------------------------------------------- motion

/// Damped spring 0 → 1 with damping ratio `zeta` (1 = critically damped, no
/// overshoot; 0.5 = lively). Settles to exactly 1.
#[inline]
pub fn spring_z(t: f32, speed: f32, zeta: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    let w = 12.0 * speed;
    let z = zeta.clamp(0.05, 1.5);
    let e = (-z * w * t).exp();
    if e < 2e-4 {
        return 1.0;
    }
    if z >= 0.999 {
        return 1.0 - e * (1.0 + w * t);
    }
    let wd = w * (1.0 - z * z).sqrt();
    1.0 - e * ((wd * t).cos() + (z * w / wd) * (wd * t).sin())
}

/// Exponential ease-out (fast in, very long soft landing).
#[inline]
pub fn ease_out_expo(x: f32) -> f32 {
    let x = clamp01(x);
    if x >= 1.0 {
        1.0
    } else {
        1.0 - 2f32.powf(-10.0 * x)
    }
}

/// Quintic ease-in-out: a decisive, cinematic move.
#[inline]
pub fn ease_in_out_quint(x: f32) -> f32 {
    let x = clamp01(x);
    if x < 0.5 {
        16.0 * x.powi(5)
    } else {
        1.0 - (-2.0 * x + 2.0).powi(5) / 2.0
    }
}

#[inline]
pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = clamp01((x - a) / (b - a));
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------- type

/// Inter's dynamic tracking (em) for a size in px: loose at caption sizes,
/// about −2.2 % for display sizes.
#[inline]
pub fn tracking_for(size: f32) -> f32 {
    -0.0223 + 0.185 * (-0.1745 * size).exp()
}

/// UI text with optical sizing: Inter Display at 28 px and up, Inter below,
/// both with dynamic tracking.
pub fn ui(size: f32, weight: u16, c: Rgba) -> TextStyle {
    let face = if size >= 28.0 { Face::Display } else { Face::Sans };
    TextStyle::new(face, size, weight, c).tracking(tracking_for(size))
}

/// Headline type: Inter Display, tighter than UI text.
pub fn headline(size: f32, weight: u16, c: Rgba) -> TextStyle {
    TextStyle::new(Face::Display, size, weight, c).tracking(tracking_for(size) - 0.012)
}

/// Small caps-style label: uppercase-friendly, opened up.
pub fn eyebrow(size: f32, weight: u16, c: Rgba) -> TextStyle {
    TextStyle::new(Face::Sans, size, weight, c).tracking(0.12)
}

/// Repaint laid-out glyph runs with a multi-stop linear gradient given in the
/// same (absolute) coordinates the text was drawn at.
pub fn repaint(ops: &mut [Op], p0: [f32; 2], p1: [f32; 2], stops: &[(f32, Rgba)]) {
    for op in ops {
        if let Op::Glyphs { paint, transform, .. } | Op::RoundRect { paint, transform, .. } | Op::Fill { paint, transform, .. } = op {
            let (ox, oy) = (transform[4], transform[5]);
            let (sx, sy) = (transform[0].max(1e-6), transform[3].max(1e-6));
            *paint = Paint::Linear { p0: [(p0[0] - ox) / sx, (p0[1] - oy) / sy], p1: [(p1[0] - ox) / sx, (p1[1] - oy) / sy], stops: stops.to_vec() };
        }
    }
}

/// A narrow bright band at `pos` (0..1 along p0→p1), for sheen sweeps.
pub fn sheen_stops(pos: f32, width: f32, c: Rgba) -> Vec<(f32, Rgba)> {
    let z = alpha(c, 0.0);
    let a = (pos - width).clamp(0.0, 1.0);
    let m = pos.clamp(0.0, 1.0);
    let b = (pos + width).clamp(0.0, 1.0);
    let mut s = vec![(0.0, z)];
    if a > 0.0 {
        s.push((a, z));
    }
    s.push((m, c));
    if b < 1.0 {
        s.push((b, z));
    }
    s.push((1.0, z));
    s.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);
    s
}

// ---------------------------------------------------------------- depth

/// Layered elevation for an **opaque** surface drawn right after: a tight
/// contact shadow, a key shadow and a wide ambient one. `level` ≈ 0.5 (chip)
/// to 2 (floating window).
pub fn elevation(out: &mut Vec<Op>, th: &Theme, x: f32, y: f32, w: f32, h: f32, r: f32, level: f32) {
    if level <= 0.0 {
        return;
    }
    let k = if th.dark { 1.0 } else { 0.32 };
    // Big surfaces skip the contact shadow and cap the ambient blur: their
    // sprites are re-blurred every frame while they animate in.
    let big = w * h > 500_000.0;
    if !big {
        shadow(out, x, y, w, h, r, 1.0 + 0.5 * level, 1.5 + level, 0.32 * k);
    }
    shadow(out, x, y, w, h, r, 6.0 * level, 12.0 * level, 0.30 * k);
    shadow(out, x, y, w, h, r, 22.0 * level, (44.0 * level).min(if big { 32.0 } else { 64.0 }), 0.38 * k);
}

/// Lit top edge: a 1 px inner highlight that fades out down the sides, plus
/// a faint sheen over the top of the surface.
pub fn bevel(out: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32, strength: f32) {
    if strength <= 0.0 || w <= 2.0 || h <= 2.0 {
        return;
    }
    let reach = (r * 2.5).max(18.0).min(h);
    out.push(Op::Stroke {
        path: rrect_path(x + 0.5, y + 0.5, w - 1.0, h - 1.0, (r - 0.5).max(0.0)),
        paint: Paint::Linear { p0: [0.0, y], p1: [0.0, y + reach], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.22 * strength])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))] },
        stroke: line(1.0, [0.0, 1.0]),
        transform: IDENTITY,
    });
}

/// Premium opaque panel: layered elevation, a top-lit vertical gradient, a
/// soft radial sheen, a lit bevel and a hairline border.
pub fn panel(out: &mut Vec<Op>, th: &Theme, x: f32, y: f32, w: f32, h: f32, r: f32, level: f32, fill: Option<Rgba>) {
    let base = fill.unwrap_or(th.surface);
    elevation(out, th, x, y, w, h, r, level);
    let top = shade(base, if th.dark { 0.018 } else { 0.0 });
    let bot = shade(base, if th.dark { -0.05 } else { -0.015 });
    out.push(rect(x, y, w, h, r, Paint::Linear { p0: [0.0, y], p1: [0.0, y + h], stops: vec![(0.0, top), (1.0, bot)] }));
    if th.dark {
        out.push(rect(x, y, w, h, r, radial(x + w * 0.3, y - h * 0.1, w.max(h) * 0.9, vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.03])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))])));
    }
    border(out, x, y, w, h, r, th.border);
    bevel(out, x, y, w, h, r, if th.dark { 1.0 } else { 0.0 });
}

/// Frosted glass panel: a translucent tint (its own shadow scaled by the
/// tint's opacity), a diagonal sheen, a lit bevel and a hairline border.
pub fn glass(out: &mut Vec<Op>, th: &Theme, x: f32, y: f32, w: f32, h: f32, r: f32, opacity: f32) {
    let tint = if th.dark { shade(th.surface, 0.02) } else { Rgba([1.0, 1.0, 1.0, 1.0]) };
    let a = opacity.clamp(0.2, 1.0);
    let k = if th.dark { 1.0 } else { 0.3 };
    out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Shadow { dx: 0.0, dy: 18.0, blur: 40.0, color: Rgba([0.0, 0.0, 0.0, 0.5 * k]) }], clip: None, cache_key: None });
    out.push(rect(x, y, w, h, r, solid(alpha(tint, a))));
    out.push(Op::PopLayer);
    out.push(rect(x, y, w, h, r, Paint::Linear { p0: [x, y], p1: [x + w * 0.6, y + h], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, if th.dark { 0.07 } else { 0.25 }])), (0.5, Rgba([1.0, 1.0, 1.0, 0.0]))] }));
    border(out, x, y, w, h, r, if th.dark { Rgba([1.0, 1.0, 1.0, 0.12]) } else { Rgba([0.0, 0.0, 0.0, 0.08]) });
    bevel(out, x, y, w, h, r, 1.2);
}

/// Soft coloured bloom behind a shape (blurred, screen-blended).
pub fn bloom(out: &mut Vec<Op>, ops: Vec<Op>, sigma: f32, opacity: f32) {
    if ops.is_empty() || opacity <= 0.001 {
        return;
    }
    out.push(Op::PushLayer { opacity: opacity.min(1.0), blend: Blend::Screen, filters: vec![Filter::Blur(sigma)], clip: None, cache_key: None });
    out.extend(ops);
    out.push(Op::PopLayer);
}

// ---------------------------------------------------------------- reveals

/// Clip `ops` to a rectangle (a line's box) and slide them up into it from
/// `dy` below as `p` goes 0 → 1: the classic masked line reveal.
pub fn masked_rise(ops: Vec<Op>, clip: [f32; 4], dy: f32, p: f32) -> Vec<Op> {
    if p <= 0.0 || ops.is_empty() {
        return Vec::new();
    }
    let mut ops = ops;
    if p < 1.0 {
        transform_ops(&mut ops, &translate(0.0, (1.0 - p) * dy));
    }
    let mut o = Vec::with_capacity(ops.len() + 2);
    o.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((rrect_path(clip[0], clip[1], clip[2], clip[3], 0.0), IDENTITY)), cache_key: None });
    o.extend(ops);
    o.push(Op::PopLayer);
    o
}

/// Wrap ops in a blurred layer while `sigma` is noticeable (focus pulls).
pub fn blurred(ops: Vec<Op>, sigma: f32, opacity: f32) -> Vec<Op> {
    if ops.is_empty() || opacity <= 0.001 {
        return Vec::new();
    }
    if sigma < 0.35 {
        return super::with_opacity(ops, opacity);
    }
    let mut o = Vec::with_capacity(ops.len() + 2);
    o.push(Op::PushLayer { opacity: opacity.min(1.0), blend: Blend::Normal, filters: vec![Filter::Blur(sigma)], clip: None, cache_key: None });
    o.extend(ops);
    o.push(Op::PopLayer);
    o
}

/// Motion smear: faint copies of `ops` trailing behind a move of `(vx, vy)`
/// px per frame, then the sharp copy. Cheap (vector re-draws, no blur pass).
pub fn smear(ops: Vec<Op>, vx: f32, vy: f32) -> Vec<Op> {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed < 2.0 || ops.is_empty() {
        return ops;
    }
    let n = ((speed / 6.0).ceil() as usize).clamp(2, 6);
    let mut o = Vec::new();
    for i in (1..=n).rev() {
        let f = i as f32 / n as f32;
        let mut g = ops.clone();
        transform_ops(&mut g, &translate(-vx * f * 0.9, -vy * f * 0.9));
        o.push(Op::PushLayer { opacity: 0.22 * (1.0 - f * 0.7), blend: Blend::Normal, filters: vec![], clip: None, cache_key: None });
        o.extend(g);
        o.push(Op::PopLayer);
    }
    o.extend(ops);
    o
}

/// A small check mark path in a `s`-sized box at `(x, y)`.
pub fn check_path(x: f32, y: f32, s: f32) -> dl::Path {
    let mut p = dl::Path::new();
    p.move_to(((x + s * 0.27) as f64, (y + s * 0.53) as f64));
    p.line_to(((x + s * 0.44) as f64, (y + s * 0.69) as f64));
    p.line_to(((x + s * 0.74) as f64, (y + s * 0.35) as f64));
    p
}

/// Coloured drop shadow under an opaque shape drawn right after (e.g. an
/// accent-tinted lift under a primary button).
pub fn tinted_shadow(out: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32, dy: f32, blur: f32, c: Rgba) {
    if c.0[3] <= 0.001 || w <= 0.0 || h <= 0.0 {
        return;
    }
    out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Shadow { dx: 0.0, dy, blur, color: c }], clip: None, cache_key: None });
    out.push(rect(x, y, w, h, r, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
    out.push(Op::PopLayer);
}

// ---------------------------------------------------------------- looks

/// How a surface is finished. Every surface-bearing component takes
/// `surface`, so the same component can read as a lit app panel, frosted
/// glass, a wireframe, a flat poster block or matte paper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    /// Lit panel: layered shadows, top-lit gradient, bevel, hairline border.
    #[default]
    Solid,
    /// Translucent frosted pane with a sheen and a bright rim.
    Glass,
    /// No fill: a crisp 1.5 px outline (wireframe / editorial).
    Outline,
    /// One flat colour, no shadow, no border (poster / Swiss).
    Flat,
    /// Matte fill with one soft low shadow, no highlights (print / paper).
    Paper,
}

/// Motion character for entrances and state changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    /// Critically damped, quick start and a long soft landing.
    #[default]
    Smooth,
    /// Fast and decisive, no overshoot.
    Snappy,
    /// Slow, gentle ease-in-out (luxury, calm).
    Soft,
    /// Springy overshoot (playful).
    Bouncy,
}

impl Motion {
    /// Entrance progress 0 → 1 (may overshoot slightly for `Bouncy`) at `t` s
    /// after the start.
    pub fn enter(self, t: f32) -> f32 {
        match self {
            Motion::Smooth => spring_z(t, 0.85, 1.0),
            Motion::Snappy => spring_z(t, 1.7, 1.0),
            Motion::Soft => ease_in_out(seg(t, 0.0, 1.1)),
            Motion::Bouncy => spring_z(t, 1.0, 0.42),
        }
    }
    /// How far things travel on entrance, relative to `Smooth`.
    pub fn travel(self) -> f32 {
        match self {
            Motion::Smooth => 1.0,
            Motion::Snappy => 0.6,
            Motion::Soft => 1.4,
            Motion::Bouncy => 1.2,
        }
    }
    /// Stagger multiplier between items.
    pub fn pace(self) -> f32 {
        match self {
            Motion::Smooth => 1.0,
            Motion::Snappy => 0.6,
            Motion::Soft => 1.6,
            Motion::Bouncy => 1.0,
        }
    }
    /// Ease for a 0..1 segment (bars filling, lines drawing).
    pub fn ease(self, x: f32) -> f32 {
        match self {
            Motion::Smooth => ease_out_expo(x),
            Motion::Snappy => ease_out(x),
            Motion::Soft => ease_in_out(x),
            Motion::Bouncy => {
                let x = clamp01(x);
                let c = 1.70158 * 1.3;
                1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2)
            }
        }
    }
}

/// Draw a surface in the requested finish. `fill` overrides the theme
/// surface colour; `level` is the elevation for `Solid`.
pub fn finish(out: &mut Vec<Op>, th: &Theme, s: Surface, x: f32, y: f32, w: f32, h: f32, r: f32, level: f32, fill: Option<Rgba>) {
    match s {
        Surface::Solid => panel(out, th, x, y, w, h, r, level, fill),
        Surface::Glass => glass(out, th, x, y, w, h, r, 0.55),
        Surface::Outline => {
            out.push(Op::Stroke { path: rrect_path(x + 0.75, y + 0.75, w - 1.5, h - 1.5, (r - 0.75).max(0.0)), paint: solid(alpha(fill.unwrap_or(th.text), 0.55)), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
        }
        Surface::Flat => out.push(rect(x, y, w, h, r, solid(fill.unwrap_or(th.surface2)))),
        Surface::Paper => {
            let k = if th.dark { 0.5 } else { 0.14 };
            shadow(out, x, y, w, h, r, 3.0, 10.0, k);
            out.push(rect(x, y, w, h, r, solid(fill.unwrap_or(th.surface))));
        }
    }
}

/// Text style in a chosen face (`None` = optical-size UI type).
pub fn face_style(face: Option<Face>, size: f32, weight: u16, c: Rgba) -> TextStyle {
    match face {
        None | Some(Face::Sans) => ui(size, weight, c),
        Some(Face::Display) => headline(size, weight, c),
        Some(Face::Serif) => TextStyle::new(Face::Serif, size * 1.12, 400, c).tracking(-0.01),
        Some(Face::Mono) => TextStyle::new(Face::Mono, size * 0.9, weight.min(600), c).tracking(-0.01),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn springs_settle() {
        assert_eq!(spring_z(5.0, 1.0, 0.7), 1.0);
        assert_eq!(spring_z(5.0, 1.0, 1.0), 1.0);
        // Critically damped never overshoots.
        assert!((0..200).all(|i| spring_z(i as f32 / 100.0, 1.0, 1.0) <= 1.0));
        assert!(tracking_for(12.0) > tracking_for(64.0));
        let s = sheen_stops(0.5, 0.1, Rgba([1.0; 4]));
        assert!(s.windows(2).all(|w| w[0].0 <= w[1].0));
    }
}
