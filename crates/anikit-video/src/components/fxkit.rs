//! Light and particle building blocks shared by the effect components
//! (`god-rays`, `lens-flare`, the particle kinds, `particle-text`,
//! `metaballs`, `globe`, `morph`, `reactions`).
//!
//! Everything here is plain vector ops: photographic falloffs as gradient
//! stops (inverse-square halos, gaussian cores), tapered needles for
//! diffraction spikes and motion streaks, chromatic rings, bokeh discs, and
//! a little colour science (colour temperature, light-on-theme tints).
//! Depth of field is a blurred layer per depth band, so out-of-focus
//! particles are genuinely defocused rather than just dimmer.

use super::style::*;
use super::Theme;
use crate::dl::{Blend, Filter, FillRule, Op, Paint, Path, Rgba};
use anikit_core::noise::Noise;

// ---------------------------------------------------------------- colour

/// Colour of a black body at `kelvin` (1000–12000 K), as display sRGB,
/// normalised so the brightest channel is 1. 6500 K is neutral daylight,
/// 3200 K tungsten, 1900 K candle light.
pub fn kelvin(kelvin: f32) -> Rgba {
    // Tanner Helland's fit, in sRGB 0..255.
    let t = kelvin.clamp(1000.0, 40000.0) / 100.0;
    let r = if t <= 66.0 { 255.0 } else { 329.698_73 * (t - 60.0).powf(-0.133_204_76) };
    let g = if t <= 66.0 { 99.470_8 * t.ln() - 161.119_57 } else { 288.122_16 * (t - 60.0).powf(-0.075_514_85) };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_73 * (t - 10.0).ln() - 305.044_8
    };
    let c = [r, g, b].map(|v| (v / 255.0).clamp(0.0, 1.0));
    let m = c[0].max(c[1]).max(c[2]).max(1e-3);
    Rgba([c[0] / m, c[1] / m, c[2] / m, 1.0])
}

/// A light colour that sits with the theme: neutral `kelvin` light, nudged
/// a little (`amount` 0..1) towards the theme accent's hue.
pub fn light_tint(theme: &Theme, kelvin_k: f32, amount: f32) -> Rgba {
    let base = kelvin(kelvin_k);
    let acc = theme.accent;
    let m = acc.0[0].max(acc.0[1]).max(acc.0[2]).max(1e-3);
    let hue = Rgba([acc.0[0] / m, acc.0[1] / m, acc.0[2] / m, 1.0]);
    mix(base, hue, amount.clamp(0.0, 1.0))
}

/// Brighten towards white keeping the hue (for hot cores of coloured light).
pub fn hot(c: Rgba, k: f32) -> Rgba {
    mix(c, Rgba([1.0, 1.0, 1.0, c.0[3]]), k)
}

/// Gradient colour at `f` (0..1) through `cols`.
pub fn ramp(cols: &[Rgba], f: f32) -> Rgba {
    match cols.len() {
        0 => Rgba([1.0; 4]),
        1 => cols[0],
        n => {
            let f = clamp01(f) * (n - 1) as f32;
            let i = (f as usize).min(n - 2);
            mix(cols[i], cols[i + 1], f - i as f32)
        }
    }
}

/// `colors` parameter, or `fallback` when empty.
pub fn colors_or(colors: &[Color], fallback: impl FnOnce() -> Vec<Rgba>) -> Vec<Rgba> {
    if colors.is_empty() {
        fallback()
    } else {
        colors.iter().map(|c| c.0).collect()
    }
}

// ---------------------------------------------------------------- falloffs

/// Gaussian falloff stops `exp(-x² · 4.6)` (1 % at the edge), `n` stops.
pub fn gauss(c: Rgba, a: f32) -> Vec<(f32, Rgba)> {
    const X: [f32; 7] = [0.0, 0.15, 0.3, 0.45, 0.6, 0.8, 1.0];
    X.iter().map(|&x| (x, alpha(c, if x >= 1.0 { 0.0 } else { a * (-4.6 * x * x).exp() }))).collect()
}

/// Photographic glow: a bright core with a long inverse-square-like tail,
/// `1 / (1 + (x / core)²)`, windowed to zero at the edge.
pub fn bloom(c: Rgba, a: f32, core: f32) -> Vec<(f32, Rgba)> {
    const X: [f32; 9] = [0.0, 0.03, 0.07, 0.13, 0.22, 0.35, 0.5, 0.72, 1.0];
    X.iter()
        .map(|&x| {
            let v = 1.0 / (1.0 + (x / core.max(1e-3)).powi(2));
            let win = (1.0 - x * x).max(0.0).powi(2);
            (x, alpha(c, a * v * win))
        })
        .collect()
}

/// A soft radial light at `(x, y)` with radius `r` and the given stops.
pub fn light(x: f32, y: f32, r: f32, stops: Vec<(f32, Rgba)>) -> Op {
    Op::Fill { path: circle_path(x, y, r.max(0.01)), paint: radial(x, y, r.max(0.01), stops), rule: FillRule::NonZero, transform: crate::dl::IDENTITY }
}

/// An ellipse (unit circle under a transform), rotated by `rot`, painted
/// with radial `stops` that stretch with it.
pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, stops: Vec<(f32, Rgba)>) -> Op {
    let (s, c) = rot.sin_cos();
    Op::Fill {
        path: circle_path(0.0, 0.0, 1.0),
        paint: radial(0.0, 0.0, 1.0, stops),
        rule: FillRule::NonZero,
        transform: [c * rx, s * rx, -s * ry, c * ry, cx, cy],
    }
}

/// A thin ring of light at radius `r` (soft both sides, `width` px wide).
pub fn ring(cx: f32, cy: f32, r: f32, width: f32, c: Rgba, a: f32) -> Op {
    let outer = r + width * 1.5;
    let f = |d: f32| clamp01((r + d) / outer);
    light(cx, cy, outer, vec![
        (0.0, alpha(c, 0.0)),
        (f(-width * 1.5), alpha(c, 0.0)),
        (f(-width * 0.6), alpha(c, a * 0.35)),
        (f(0.0), alpha(c, a)),
        (f(width * 0.6), alpha(c, a * 0.3)),
        (1.0, alpha(c, 0.0)),
    ])
}

/// A ring with chromatic dispersion: red outside, blue inside.
pub fn chroma_ring(out: &mut Vec<Op>, cx: f32, cy: f32, r: f32, width: f32, spread: f32, a: f32) {
    for (k, c) in [(-1.0, Rgba([0.25, 0.45, 1.0, 1.0])), (0.0, Rgba([0.35, 1.0, 0.55, 1.0])), (1.0, Rgba([1.0, 0.35, 0.25, 1.0]))] {
        out.push(ring(cx, cy, r + k * spread, width, c, a));
    }
}

/// A needle of light through `(cx, cy)` along `ang`: `len` px each way,
/// `width` px wide at the centre, tapering to a point, fading out along
/// its length with a `1/(1+x)`-like falloff.
pub fn needle(cx: f32, cy: f32, ang: f32, len: f32, width: f32, c: Rgba, a: f32) -> Op {
    let (s, co) = ang.sin_cos();
    let (dx, dy) = (co * len, s * len);
    let (nx, ny) = (-s * width * 0.5, co * width * 0.5);
    let mut p = Path::new();
    p.move_to(((cx - dx) as f64, (cy - dy) as f64));
    p.line_to(((cx + nx) as f64, (cy + ny) as f64));
    p.line_to(((cx + dx) as f64, (cy + dy) as f64));
    p.line_to(((cx - nx) as f64, (cy - ny) as f64));
    p.close_path();
    let f = |x: f32| a / (1.0 + 9.0 * x) * (1.0 - x);
    let mut stops = Vec::with_capacity(11);
    for x in [1.0, 0.6, 0.3, 0.12, 0.04] {
        stops.push((0.5 - 0.5 * x, alpha(c, f(x))));
    }
    stops.push((0.5, alpha(c, a)));
    for x in [0.04, 0.12, 0.3, 0.6, 1.0] {
        stops.push((0.5 + 0.5 * x, alpha(c, f(x))));
    }
    Op::Fill { path: p, paint: Paint::Linear { p0: [cx - dx, cy - dy], p1: [cx + dx, cy + dy], stops }, rule: FillRule::NonZero, transform: crate::dl::IDENTITY }
}

/// Append a tapered streak from `tail` to `head` (a capsule-ish quad that is
/// `w0` wide at the tail and `w1` at the head, with a round head) to `p`.
pub fn streak_path(p: &mut Path, tail: (f32, f32), head: (f32, f32), w0: f32, w1: f32) {
    let (dx, dy) = (head.0 - tail.0, head.1 - tail.1);
    let l = (dx * dx + dy * dy).sqrt();
    if l < 0.3 {
        dot(p, head.0, head.1, w1 * 0.5);
        return;
    }
    let (ux, uy) = (dx / l, dy / l);
    let (nx, ny) = (-uy, ux);
    let (h0, h1) = (w0 * 0.5, w1 * 0.5);
    let k = 0.552_284_7 * h1;
    let pt = |x: f32, y: f32| (x as f64, y as f64);
    p.move_to(pt(tail.0 + nx * h0, tail.1 + ny * h0));
    p.line_to(pt(head.0 + nx * h1, head.1 + ny * h1));
    // Round cap at the head (two quarter arcs).
    p.curve_to(pt(head.0 + nx * h1 + ux * k, head.1 + ny * h1 + uy * k), pt(head.0 + ux * h1 + nx * k, head.1 + uy * h1 + ny * k), pt(head.0 + ux * h1, head.1 + uy * h1));
    p.curve_to(pt(head.0 + ux * h1 - nx * k, head.1 + uy * h1 - ny * k), pt(head.0 - nx * h1 + ux * k, head.1 - ny * h1 + uy * k), pt(head.0 - nx * h1, head.1 - ny * h1));
    p.line_to(pt(tail.0 - nx * h0, tail.1 - ny * h0));
    p.close_path();
}

/// Append a tapered trail through `pts` (tail first, head last) to `p`:
/// `w0` wide at the tail growing to `w1` at the head, with a round head.
/// One polygon, so curved trails have no overlaps.
pub fn trail_path(p: &mut Path, pts: &[(f32, f32)], w0: f32, w1: f32) {
    let n = pts.len();
    if n < 2 {
        if let Some(q) = pts.first() {
            dot(p, q.0, q.1, w1 * 0.5);
        }
        return;
    }
    let total: f32 = pts.windows(2).map(|s| ((s[1].0 - s[0].0).powi(2) + (s[1].1 - s[0].1).powi(2)).sqrt()).sum();
    if total < 0.5 {
        dot(p, pts[n - 1].0, pts[n - 1].1, w1 * 0.5);
        return;
    }
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    let mut acc = 0.0;
    for i in 0..n {
        let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-4);
        let (nx, ny) = (-dy / l, dx / l);
        if i > 0 {
            acc += ((pts[i].0 - pts[i - 1].0).powi(2) + (pts[i].1 - pts[i - 1].1).powi(2)).sqrt();
        }
        let hw = 0.5 * (w0 + (w1 - w0) * (acc / total));
        left.push((pts[i].0 + nx * hw, pts[i].1 + ny * hw));
        right.push((pts[i].0 - nx * hw, pts[i].1 - ny * hw));
    }
    let pt = |q: (f32, f32)| (q.0 as f64, q.1 as f64);
    p.move_to(pt(left[0]));
    for q in &left[1..] {
        p.line_to(pt(*q));
    }
    // Round head: a half circle from left to right around the tip.
    let head = pts[n - 1];
    let prev = pts[n - 2];
    let (dx, dy) = (head.0 - prev.0, head.1 - prev.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-4);
    let (ux, uy) = (dx / l, dy / l);
    let h = 0.5 * w1;
    let k = 0.552_284_7 * h;
    let (nx, ny) = (-uy, ux);
    p.curve_to(pt((head.0 + nx * h + ux * k, head.1 + ny * h + uy * k)), pt((head.0 + ux * h + nx * k, head.1 + uy * h + ny * k)), pt((head.0 + ux * h, head.1 + uy * h)));
    p.curve_to(pt((head.0 + ux * h - nx * k, head.1 + uy * h - ny * k)), pt((head.0 - nx * h + ux * k, head.1 - ny * h + uy * k)), pt((head.0 - nx * h, head.1 - ny * h)));
    for q in right[..n - 1].iter().rev() {
        p.line_to(pt(*q));
    }
    p.close_path();
}

/// Append a circle to `p`: an octagon below 2.5 px radius (indistinguishable
/// and far cheaper to rasterise), four cubics above.
#[inline]
pub fn dot(p: &mut Path, x: f32, y: f32, r: f32) {
    let (x, y, r) = (x as f64, y as f64, r.max(0.05) as f64);
    if r < 2.5 {
        const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
        const K: [(f64, f64); 8] = [(1.0, 0.0), (D, D), (0.0, 1.0), (-D, D), (-1.0, 0.0), (-D, -D), (0.0, -1.0), (D, -D)];
        p.move_to((x + r, y));
        for (cx, cy) in &K[1..] {
            p.line_to((x + cx * r, y + cy * r));
        }
        p.close_path();
        return;
    }
    let c = r * 0.552_284_75;
    p.move_to((x + r, y));
    p.curve_to((x + r, y + c), (x + c, y + r), (x, y + r));
    p.curve_to((x - c, y + r), (x - r, y + c), (x - r, y));
    p.curve_to((x - r, y - c), (x - c, y - r), (x, y - r));
    p.curve_to((x + c, y - r), (x + r, y - c), (x + r, y));
    p.close_path();
}

/// Regular polygon (`n` sides, circumradius `r`, rotated by `rot`) appended to `p`.
pub fn polygon(p: &mut Path, cx: f32, cy: f32, r: f32, n: usize, rot: f32) {
    for j in 0..n {
        let a = rot + j as f32 * std::f32::consts::TAU / n as f32;
        let q = ((cx + r * a.cos()) as f64, (cy + r * a.sin()) as f64);
        if j == 0 {
            p.move_to(q);
        } else {
            p.line_to(q);
        }
    }
    p.close_path();
}

/// Four-pointed sparkle (concave star) appended to `p`.
pub fn sparkle(p: &mut Path, cx: f32, cy: f32, r: f32, rot: f32, pinch: f32) {
    let k = r * pinch;
    let (s, c) = rot.sin_cos();
    let pt = |x: f32, y: f32| ((cx + x * c - y * s) as f64, (cy + x * s + y * c) as f64);
    p.move_to(pt(0.0, -r));
    p.quad_to(pt(k, -k), pt(r, 0.0));
    p.quad_to(pt(k, k), pt(0.0, r));
    p.quad_to(pt(-k, k), pt(-r, 0.0));
    p.quad_to(pt(-k, -k), pt(0.0, -r));
    p.close_path();
}

// ---------------------------------------------------------------- layers

/// Begin an additive (light) layer, or a normal one on light themes where
/// adding light would wash out.
pub fn push_light(out: &mut Vec<Op>, opacity: f32, additive: bool) {
    push_layer(out, opacity, if additive { Blend::Add } else { Blend::Normal }, Vec::new());
}

/// Begin a layer blurred by `sigma` px (skipped when `sigma` is tiny);
/// returns whether a layer was pushed (pop it with [`pop_if`]).
pub fn push_blur(out: &mut Vec<Op>, sigma: f32, blend: Blend) -> bool {
    if sigma < 0.35 && blend == Blend::Normal {
        return false;
    }
    let f = if sigma >= 0.35 { vec![Filter::Blur(sigma)] } else { Vec::new() };
    push_layer(out, 1.0, blend, f);
    true
}

pub fn pop_if(out: &mut Vec<Op>, pushed: bool) {
    if pushed {
        out.push(Op::PopLayer);
    }
}

/// Fill `p` with a solid colour (skipping empty paths).
pub fn fill_solid(out: &mut Vec<Op>, p: Path, c: Rgba) {
    if !p.is_empty() && c.0[3] > 0.002 {
        out.push(fill(p, solid(c)));
    }
}

// ---------------------------------------------------------------- motion

/// Curl of a 2D noise potential at `(x, y)` (slice `z`): a divergence-free
/// flow, so particles swirl without bunching up. Roughly unit magnitude.
pub fn curl(n: &Noise, x: f32, y: f32, z: f32) -> (f32, f32) {
    let e = 0.05f64;
    let (x, y, z) = (x as f64, y as f64, z as f64);
    let dy = (n.noise3(x, y + e, z) - n.noise3(x, y - e, z)) / (2.0 * e);
    let dx = (n.noise3(x + e, y, z) - n.noise3(x - e, y, z)) / (2.0 * e);
    ((dy * 0.5) as f32, (-dx * 0.5) as f32)
}

/// Smoothstep.
#[inline]
pub fn smooth(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * (3.0 - 2.0 * x)
}

/// Parse a choice parameter against `options`, with a "did you mean" error.
pub fn choice(what: &str, value: &str, options: &[&str]) -> Result<(), super::Error> {
    if options.contains(&value) {
        return Ok(());
    }
    Err(super::Error(match closest(value, options) {
        Some(s) => format!("unknown {what} `{value}`; did you mean `{s}`? (one of {})", options.join(", ")),
        None => format!("unknown {what} `{value}`; use one of {}", options.join(", ")),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kelvin_is_sane() {
        let warm = kelvin(2000.0);
        let cool = kelvin(10000.0);
        assert!(warm.0[0] > warm.0[2] + 0.3);
        assert!(cool.0[2] >= cool.0[0]);
        let d = kelvin(6500.0);
        assert!(d.0.iter().take(3).all(|v| *v > 0.9), "{d:?}");
    }

    #[test]
    fn curl_is_bounded() {
        let n = Noise::new(3);
        for i in 0..50 {
            let (x, y) = curl(&n, i as f32 * 0.37, i as f32 * 0.11, 0.5);
            assert!(x.is_finite() && y.is_finite() && x.abs() < 4.0 && y.abs() < 4.0);
        }
    }
}
