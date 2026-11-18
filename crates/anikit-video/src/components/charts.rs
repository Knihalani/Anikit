//! Charts: bar, line, area, donut, pie; counters and stat tiles.
//!
//! Every chart builds on over `duration` seconds after `delay`, with a
//! per-item `stagger`. Value labels count up with their bars.

use super::polish::{spring_z, Motion, Surface};
use super::style::*;
use super::text::{draw, Face, TextStyle};
use super::ui::arc_path;
use super::{transform_ops, with_opacity, Component, Ctx, Error, Prepare};
use crate::dl::{Op, Paint, Path, Rgba, IDENTITY};
use serde::Deserialize;

/// Number formatting shared by charts and counters.
#[derive(Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Format {
    pub prefix: String,
    pub suffix: String,
    pub decimals: Option<usize>,
    /// 12.5K / 3.4M style.
    pub compact: bool,
}
impl Format {
    pub fn fmt(&self, v: f64) -> String {
        let body = if self.compact { compact_number(v) } else { format_number(v, self.decimals.unwrap_or(if v.fract().abs() > 1e-9 && v.abs() < 100.0 { 1 } else { 0 }), true) };
        format!("{}{}{}", self.prefix, body, self.suffix)
    }
}

/// "Nice" axis maximum and tick step for `max`.
pub fn nice(max: f32, ticks: usize) -> (f32, f32) {
    if max <= 0.0 {
        return (1.0, 0.25);
    }
    let raw = max / ticks.max(1) as f32;
    let mag = 10f32.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    ((max / step).ceil() * step, step)
}

fn colors(cols: &[Color], th: &super::Theme, i: usize) -> Rgba {
    if cols.is_empty() {
        th.pal(i)
    } else {
        cols[i % cols.len()].0
    }
}

fn small(th: &super::Theme, size: f32) -> TextStyle {
    TextStyle::new(Face::Sans, size, 500, th.muted)
}

// ------------------------------------------------------------------ shared

/// A callout pinned to a data point: `{ "index": 5, "text": "Launch" }`
/// (`series` picks the line for line charts).
#[derive(Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Note {
    pub index: usize,
    pub text: String,
    pub series: usize,
}

/// Axis value labels and hairline gridlines.
fn grid_lines(o: &mut Vec<Op>, ctx: &Ctx, fmt: &Format, fs: f32, a: f32, horizontal: bool, plot: [f32; 4], lo: f32, hi: f32, step: f32, dashed: bool) {
    let th = ctx.theme;
    let [px, py, pw, ph] = plot;
    let mut g = Path::new();
    let ticks = ((hi - lo) / step).round().max(1.0) as i32;
    let ls = small(th, fs * 0.8).tabular().color(alpha(th.muted, a * 0.85));
    for i in 0..=ticks {
        let v = lo + i as f32 * step;
        let f = (v - lo) / (hi - lo);
        if horizontal {
            let x = (px + pw * f).round() + 0.5;
            g.move_to((x as f64, py as f64));
            g.line_to((x as f64, (py + ph) as f64));
            draw(o, ctx.assets, &fmt.fmt(v as f64), &ls, x, py + ph + fs * 1.4, 0.5);
        } else {
            let y = (py + ph - ph * f).round() + 0.5;
            g.move_to((px as f64, y as f64));
            g.line_to(((px + pw) as f64, y as f64));
            draw(o, ctx.assets, &fmt.fmt(v as f64), &ls, px - fs * 0.7, y + fs * 0.28, 1.0);
        }
    }
    let dash = if dashed { vec![2.0, 5.0] } else { vec![] };
    o.push(Op::Stroke { path: g, paint: solid(alpha(th.text, 0.075 * a)), stroke: crate::dl::Stroke { width: 1.0, cap: 0, join: 0, miter_limit: 4.0, dash, dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY });
}

/// A value pill (rounded label with a small pointer) above `(x, y)`.
fn value_pill(o: &mut Vec<Op>, ctx: &Ctx, txt: &str, x: f32, y: f32, fs: f32, fillc: Rgba, a: f32) {
    if a <= 0.0 {
        return;
    }
    let st = TextStyle::new(Face::Sans, fs * 0.82, 650, on_color(&[fillc])).tabular();
    let tw = ctx.assets.measure(txt, &st);
    let (w, h) = (tw + fs * 0.9, fs * 1.55);
    let (x0, y0) = (x - w * 0.5, y - h - fs * 0.45);
    let mut e = Vec::new();
    shadow(&mut e, x0, y0, w, h, h * 0.5, 3.0, 8.0, 0.35);
    e.push(rect(x0, y0, w, h, h * 0.5, solid(fillc)));
    let mut tri = Path::new();
    tri.move_to(((x - fs * 0.28) as f64, (y0 + h - 0.5) as f64));
    tri.line_to((x as f64, (y0 + h + fs * 0.3) as f64));
    tri.line_to(((x + fs * 0.28) as f64, (y0 + h - 0.5) as f64));
    tri.close_path();
    e.push(fill(tri, solid(fillc)));
    draw(&mut e, ctx.assets, txt, &st, x, y0 + h * 0.5 + st.size * 0.36, 0.5);
    transform_ops(&mut e, &scale_about(lerp(0.85, 1.0, a.min(1.0)), x, y));
    o.extend(with_opacity(e, clamp01(a * 1.5)));
}

// ------------------------------------------------------------------ bar chart

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BarChart {
    pub values: Vec<f32>,
    pub labels: Vec<String>,
    /// One colour, or one per bar; default: the accent (or the palette when `multicolor`).
    pub colors: Vec<Color>,
    pub multicolor: bool,
    pub horizontal: bool,
    /// Axis maximum (default: a nice number above the largest value).
    pub max: Option<f32>,
    pub delay: f32,
    pub duration: f32,
    pub stagger: f32,
    /// Show values on the bars.
    pub values_shown: bool,
    pub grid: bool,
    /// Index of a bar to emphasise (others dim).
    pub highlight: Option<usize>,
    pub format: Format,
    /// Bar width as a fraction of its slot.
    pub bar_width: f32,
    pub font_size: f32,
    /// Bar shape: "rounded" (default: rounded top), "square", "pill".
    pub shape: String,
    /// Fill: "gradient" (default: lit top, deeper base), "solid", "outline".
    pub fill: String,
    /// Soft bloom under bars (off by default).
    pub glow: bool,
    /// Dashed gridlines instead of hairlines.
    pub dashed: bool,
    /// Pinned callouts: `[{ "index": 5, "text": "+38%" }]`.
    pub notes: Vec<Note>,
    pub motion: Motion,
    /// `"editorial"`: ranked horizontal rows, flat bars (the highlight in the accent, the rest grey), category
    /// labels left, values at the bar ends in tabular figures, a baseline instead of a grid.
    pub look: String,
    /// Editorial: earlier values drawn as ticks on each row (before → after).
    pub compare: Vec<f32>,
    /// Editorial: labels for the two states, e.g. `["2016", "2026"]` (shown once, on the first row).
    pub compare_labels: Vec<String>,
}
impl Default for BarChart {
    fn default() -> Self {
        BarChart { values: Vec::new(), labels: Vec::new(), colors: Vec::new(), multicolor: false, horizontal: false, max: None, delay: 0.0, duration: 0.9, stagger: 0.08, values_shown: true, grid: true, highlight: None, format: Format::default(), bar_width: 0.62, font_size: 22.0, shape: "rounded".into(), fill: "gradient".into(), glow: false, dashed: false, notes: Vec::new(), motion: Motion::Smooth, look: String::new(), compare: Vec::new(), compare_labels: Vec::new() }
    }
}
impl Prepare for BarChart {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.values.is_empty() {
            return Err(Error("bar-chart needs `values`".into()));
        }
        if self.values.iter().any(|v| !v.is_finite()) {
            return Err(Error("bar-chart values must be finite numbers".into()));
        }
        if !["rounded", "square", "pill"].contains(&self.shape.as_str()) {
            return Err(Error(format!("bar-chart shape must be rounded, square or pill, not {:?}", self.shape)));
        }
        if !["gradient", "solid", "outline"].contains(&self.fill.as_str()) {
            return Err(Error(format!("bar-chart fill must be gradient, solid or outline, not {:?}", self.fill)));
        }
        if !matches!(self.look.as_str(), "" | "default" | "editorial") {
            return Err(Error(format!("bar-chart: unknown look `{}` (default, editorial)", self.look)));
        }
        if !self.compare.is_empty() && self.compare.len() != self.values.len() {
            return Err(Error("bar-chart: `compare` needs one value per bar".into()));
        }
        Ok(())
    }
}
impl BarChart {
    /// Editorial bars: rows, flat fills, direct values, one highlight.
    fn eval_editorial(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let n = self.values.len();
        let accent = colors(&self.colors, th, 0);
        let grey = alpha(th.muted, 0.5);
        let vmax = self.values.iter().chain(&self.compare).fold(0.0f32, |m, v| m.max(*v));
        let hi = self.max.unwrap_or(vmax).max(1e-6);
        let ls = TextStyle::new(Face::Sans, fs, 600, th.text);
        let label_w = self.labels.iter().map(|l| ctx.assets.measure(l, &ls)).fold(0.0f32, f32::max) + fs * 1.2;
        let vs = TextStyle::new(Face::Sans, fs * 0.9, 600, th.text).tabular();
        let value_w = ctx.assets.measure(&self.format.fmt(vmax as f64), &vs) + fs * 1.2;
        let top = if self.compare_labels.is_empty() { 0.0 } else { fs * 1.6 };
        let (px, pw) = (label_w, (w - label_w - value_w).max(10.0));
        let row = (h - top) / n as f32;
        let bh = (row * self.bar_width).min(fs * 2.2);
        let mut o = Vec::new();
        let gi = ease_out(seg(t, self.delay - 0.2, 0.4));
        // Baseline.
        o.push(rect(px - 0.75, top, 1.5, h - top, 0.0, solid(alpha(th.text, 0.5 * gi))));
        for (i, v) in self.values.iter().enumerate() {
            let k = self.motion.ease(seg(t, self.delay + i as f32 * self.stagger, self.duration)).min(1.0);
            let yc = top + row * (i as f32 + 0.5);
            let hl = self.highlight.map_or(i == 0, |hh| hh == i);
            let c = if hl { accent } else { grey };
            if let Some(l) = self.labels.get(i) {
                draw(&mut o, ctx.assets, l, &ls.clone().color(alpha(if hl { accent } else { th.text }, gi)), 0.0, yc + fs * 0.36, 0.0);
            }
            let bw = pw * (v / hi) * k;
            if bw > 0.0 {
                o.push(rect(px, yc - bh * 0.5, bw, bh, 0.0, solid(c)));
            }
            if let Some(cv) = self.compare.get(i) {
                let a = ease_out(seg(t, self.delay + i as f32 * self.stagger + self.duration * 0.6, 0.3));
                let x = px + pw * (cv / hi);
                o.push(rect(x - 1.5, yc - bh * 0.5 - fs * 0.25, 3.0, bh + fs * 0.5, 0.0, solid(alpha(th.text, 0.9 * a))));
                if i == 0 && self.compare_labels.len() >= 2 {
                    let cs = small(th, fs * 0.72).tabular().color(alpha(th.muted, a));
                    draw(&mut o, ctx.assets, &self.compare_labels[0], &cs, x, yc - bh * 0.5 - fs * 0.55, 0.5);
                    draw(&mut o, ctx.assets, &self.compare_labels[1], &cs, px + pw * (v / hi), yc - bh * 0.5 - fs * 0.55, 0.5);
                }
            }
            if self.values_shown && k > 0.05 {
                let shown = v * k;
                draw(&mut o, ctx.assets, &self.format.fmt(shown as f64), &vs.clone().color(alpha(if hl { accent } else { th.text }, gi)), px + bw + fs * 0.5, yc + fs * 0.32, 0.0);
            }
        }
        o
    }
}

impl Component for BarChart {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        if self.look == "editorial" {
            return self.eval_editorial(t, ctx);
        }
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let n = self.values.len();
        let vmax = self.values.iter().fold(0.0f32, |m, v| m.max(*v));
        let (top, step) = match self.max {
            Some(m) => (m, m / 4.0),
            None => nice(vmax, 4),
        };
        let fs = self.font_size;
        let m = self.motion;
        let mut o = Vec::new();
        let has_labels = !self.labels.is_empty();
        let label_st = small(th, fs * 0.9);
        // Plot rectangle.
        let (px, py, pw, ph) = if self.horizontal {
            let lw = self.labels.iter().map(|l| ctx.assets.measure(l, &label_st)).fold(0.0, f32::max);
            let lw = if has_labels { lw + fs } else { 0.0 };
            (lw, 0.0, w - lw - fs * 4.0, h - if self.grid { fs * 1.8 } else { 0.0 })
        } else {
            let left = if self.grid { ctx.assets.measure(&self.format.fmt(top as f64), &small(th, fs * 0.8)) + fs * 0.9 } else { 0.0 };
            (left, fs * 2.4, w - left, h - fs * 2.4 - if has_labels { fs * 2.2 } else { 0.0 })
        };
        let grid_in = ease_out(seg(t, self.delay - 0.2, 0.6));
        if self.grid && grid_in > 0.0 {
            grid_lines(&mut o, ctx, &self.format, fs, grid_in, self.horizontal, [px, py, pw, ph], 0.0, top, step, self.dashed);
        }
        // Baseline axis.
        if grid_in > 0.0 {
            if self.horizontal {
                o.push(rect(px, py, 1.0, ph * grid_in, 0.0, solid(alpha(th.text, 0.22))));
            } else {
                o.push(rect(px, py + ph - 0.5, pw * grid_in, 1.0, 0.0, solid(alpha(th.text, 0.22))));
            }
        }
        let slot = if self.horizontal { ph / n as f32 } else { pw / n as f32 };
        let bw = slot * self.bar_width.clamp(0.1, 1.0);
        let speed = 0.9 / self.duration.max(0.1);
        for (i, &v) in self.values.iter().enumerate() {
            let tt = t - self.delay - i as f32 * self.stagger * m.pace();
            let k = match m {
                Motion::Snappy => spring_z(tt, speed * 1.6, 1.0),
                Motion::Soft => ease_in_out(seg(tt, 0.0, self.duration * 1.3)),
                Motion::Bouncy => spring_z(tt, speed, 0.45),
                Motion::Smooth => spring_z(tt, speed * 0.9, 0.82),
            }
            .max(0.0);
            let shown = v * k;
            let f = (shown / top).max(0.0);
            let base_c = if self.multicolor || self.colors.len() > 1 { colors(&self.colors, th, i) } else { self.colors.first().map(|c| c.0).unwrap_or(th.accent) };
            let dimmed = self.highlight.is_some_and(|hi| hi != i);
            let c = if dimmed { mix(base_c, th.surface2, 0.7) } else { base_c };
            let r = match self.shape.as_str() {
                "square" => (bw * 0.04).min(2.0),
                "pill" => bw * 0.5,
                _ => (bw * 0.2).min(12.0),
            };
            let label_a = clamp01(k * 3.0 - 1.2);
            let lc = if dimmed { alpha(th.muted, 0.55) } else { th.muted };
            let paint_for = |p0: [f32; 2], p1: [f32; 2]| -> Paint {
                match self.fill.as_str() {
                    "solid" | "outline" => solid(c),
                    _ => Paint::Linear { p0, p1, stops: vec![(0.0, shade(c, -0.22)), (1.0, shade(c, 0.12))] },
                }
            };
            let bar = |o: &mut Vec<Op>, path: Path, paint: Paint, glow_box: [f32; 4]| {
                if self.glow && !dimmed {
                    let [gx, gy, gw, gh] = glow_box;
                    super::polish::bloom(o, vec![rect(gx, gy, gw, gh, r, solid(alpha(c, 0.8)))], bw * 0.35, 0.45);
                }
                if self.fill == "outline" {
                    o.push(Op::Fill { path: path.clone(), paint: solid(alpha(c, 0.14)), rule: crate::dl::FillRule::NonZero, transform: IDENTITY });
                    o.push(Op::Stroke { path, paint, stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
                } else {
                    o.push(Op::Fill { path, paint, rule: crate::dl::FillRule::NonZero, transform: IDENTITY });
                }
            };
            if self.horizontal {
                let y = py + slot * i as f32 + (slot - bw) * 0.5;
                let len = pw * f;
                if len > 0.5 {
                    let rr = r.min(len * 0.5);
                    bar(&mut o, rrect4(px, y, len, bw, [if self.shape == "pill" { rr } else { 0.0 }, rr, rr, if self.shape == "pill" { rr } else { 0.0 }]), paint_for([px, 0.0], [px + len, 0.0]), [px, y, len, bw]);
                    if self.fill == "gradient" && !dimmed {
                        o.push(rect(px, y, len, 1.0f32.max(bw * 0.04), 0.0, solid(Rgba([1.0, 1.0, 1.0, 0.18]))));
                    }
                }
                if let Some(l) = self.labels.get(i) {
                    draw(&mut o, ctx.assets, l, &label_st.clone().color(lc), px - fs * 0.6, y + bw * 0.5 + fs * 0.32, 1.0);
                }
                if self.values_shown && label_a > 0.0 {
                    let vs = TextStyle::new(Face::Display, fs * 1.05, 650, alpha(th.text, label_a * if dimmed { 0.6 } else { 1.0 })).tabular();
                    draw(&mut o, ctx.assets, &self.format.fmt(shown as f64), &vs, px + len + fs * 0.5, y + bw * 0.5 + fs * 0.36, 0.0);
                }
            } else {
                let x = px + slot * i as f32 + (slot - bw) * 0.5;
                let len = ph * f;
                let y = py + ph - len;
                if len > 0.5 {
                    let rr = r.min(len * 0.5).min(bw * 0.5);
                    let br = if self.shape == "pill" { rr } else { 0.0 };
                    bar(&mut o, rrect4(x, y, bw, len, [rr, rr, br, br]), paint_for([0.0, py + ph], [0.0, y]), [x, y, bw, len]);
                    if self.fill == "gradient" && !dimmed {
                        // Lit top edge.
                        o.push(Op::Stroke { path: rrect4(x + 0.5, y + 0.5, bw - 1.0, len - 1.0, [rr, rr, br, br]), paint: vgrad(y, y + rr.max(8.0) * 2.0, Rgba([1.0, 1.0, 1.0, 0.35]), Rgba([1.0, 1.0, 1.0, 0.0])), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
                    }
                }
                if let Some(l) = self.labels.get(i) {
                    draw(&mut o, ctx.assets, l, &label_st.clone().color(lc), x + bw * 0.5, py + ph + fs * 1.55, 0.5);
                }
                if self.values_shown && label_a > 0.0 {
                    let vs = TextStyle::new(Face::Display, fs * 1.05, 650, alpha(if dimmed { th.muted } else { th.text }, label_a)).tabular();
                    let mut e = Vec::new();
                    draw(&mut e, ctx.assets, &self.format.fmt(shown as f64), &vs, x + bw * 0.5, y - fs * 0.55, 0.5);
                    transform_ops(&mut e, &translate(0.0, (1.0 - label_a) * fs * 0.4));
                    o.extend(e);
                }
                for note in self.notes.iter().filter(|nt| nt.index == i) {
                    let a = ease_out(seg(t, self.delay + self.duration + n as f32 * self.stagger * m.pace() + 0.1, 0.5));
                    let lift = if self.values_shown { fs * 1.6 } else { 0.0 };
                    value_pill(&mut o, ctx, &note.text, x + bw * 0.5, y - lift, fs, if dimmed { th.surface2 } else { shade(base_c, 0.1) }, a);
                }
            }
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration + self.stagger * self.motion.pace() * self.values.len() as f32 + if self.notes.is_empty() { 0.0 } else { 0.6 })
    }
}

/// Rounded rect with per-corner radii (tl, tr, br, bl).
fn rrect4(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Path {
    use kurbo::Shape;
    let m = (w.min(h) * 0.5).max(0.0);
    let c = |v: f32| v.clamp(0.0, m) as f64;
    kurbo::RoundedRect::from_rect(kurbo::Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64), kurbo::RoundedRectRadii::new(c(r[0]), c(r[1]), c(r[2]), c(r[3]))).to_path(0.1)
}

// ------------------------------------------------------------------ line / area

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineChart {
    /// One array per series.
    pub series: Vec<Vec<f32>>,
    /// Series names (legend).
    pub names: Vec<String>,
    /// X-axis labels.
    pub labels: Vec<String>,
    pub colors: Vec<Color>,
    /// Fill under the lines with a fading gradient.
    pub area: Option<bool>,
    /// Dots on data points.
    pub dots: bool,
    /// Value bubble at the end of each line.
    pub end_label: bool,
    pub smooth: bool,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub grid: bool,
    pub delay: f32,
    pub duration: f32,
    pub stagger: f32,
    pub width: f32,
    pub format: Format,
    pub font_size: f32,
    /// Soft bloom along the lines (off by default).
    pub glow: bool,
    /// Dashed gridlines instead of hairlines.
    pub dashed: bool,
    /// Pinned callouts on data points: `[{ "index": 4, "text": "Launch", "series": 0 }]`.
    pub notes: Vec<Note>,
    /// Vertical guide line following the drawing head.
    pub crosshair: bool,
    pub motion: Motion,
    /// `"editorial"`: one highlighted series in the accent, the rest as grey context, direct labels at the line
    /// ends (no legend, no pills), hairline grid with the unit on the top line, notes as leader lines.
    pub look: String,
    /// Editorial: the series to highlight (index). Default 0.
    pub highlight: usize,
}
impl Default for LineChart {
    fn default() -> Self {
        LineChart { series: Vec::new(), names: Vec::new(), labels: Vec::new(), colors: Vec::new(), area: None, dots: false, end_label: true, smooth: true, min: None, max: None, grid: true, delay: 0.0, duration: 1.6, stagger: 0.2, width: 4.0, format: Format::default(), font_size: 22.0, glow: false, dashed: false, notes: Vec::new(), crosshair: false, motion: Motion::Soft, look: String::new(), highlight: 0 }
    }
}
impl Prepare for LineChart {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.series.is_empty() || self.series.iter().any(|s| s.len() < 2) {
            return Err(Error("line-chart needs `series`: arrays of at least 2 numbers".into()));
        }
        if !matches!(self.look.as_str(), "" | "default" | "editorial") {
            return Err(Error(format!("line-chart: unknown look `{}` (default, editorial)", self.look)));
        }
        Ok(())
    }
}
impl LineChart {
    /// Editorial line chart (FT/Bloomberg-style): context in grey, one story line, direct labels.
    fn eval_editorial(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let all = self.series.iter().flatten();
        let vmax = all.clone().fold(f32::MIN, |m, v| m.max(*v));
        let vmin = all.fold(f32::MAX, |m, v| m.min(*v));
        let lo = self.min.unwrap_or(if vmin >= 0.0 { 0.0 } else { vmin });
        let (hi, step) = match self.max {
            Some(m) => (m, (m - lo) / 4.0),
            None => {
                let (n, s) = nice(vmax - lo, 4);
                (lo + n, s)
            }
        };
        let hl = self.highlight.min(self.series.len() - 1);
        let accent = colors(&self.colors, th, 0);
        let grey = alpha(th.muted, 0.55);
        // Room for direct labels on the right.
        let lab = |si: usize| -> String {
            let last = *self.series[si].last().unwrap_or(&0.0);
            match self.names.get(si) {
                Some(n) if si == hl => format!("{n}  {}", self.format.fmt(last as f64)),
                Some(n) => n.clone(),
                None => self.format.fmt(last as f64),
            }
        };
        let hs = TextStyle::new(Face::Sans, fs, 700, accent).tabular();
        let cs = small(th, fs * 0.82).color(alpha(th.muted, 0.95));
        let right = (0..self.series.len()).map(|si| ctx.assets.measure(&lab(si), if si == hl { &hs } else { &cs })).fold(0.0f32, f32::max) + fs;
        let (px, py, pw, ph) = (0.0, fs * 1.4, w - right, h - fs * 1.4 - if self.labels.is_empty() { fs * 0.5 } else { fs * 2.2 });
        let mut o = Vec::new();
        let gi = ease_out(seg(t, self.delay - 0.2, 0.5));
        let yv = |v: f32| py + ph - ph * (v - lo) / (hi - lo);
        // Hairline grid; the unit only on the top line; a stronger baseline.
        if self.grid && gi > 0.0 {
            let ticks = ((hi - lo) / step).round().max(1.0) as i32;
            let mut g = Path::new();
            let ls = small(th, fs * 0.78).tabular().color(alpha(th.muted, gi * 0.9));
            for i in 0..=ticks {
                let v = lo + i as f32 * step;
                let y = yv(v).round() + 0.5;
                if i == 0 {
                    o.push(rect(px, y - 0.5, pw, 1.5, 0.0, solid(alpha(th.text, 0.55 * gi))));
                } else {
                    g.move_to((px as f64, y as f64));
                    g.line_to(((px + pw) as f64, y as f64));
                }
                let txt = if i == ticks { self.format.fmt(v as f64) } else { Format { prefix: String::new(), suffix: String::new(), ..self.format.clone() }.fmt(v as f64) };
                draw(&mut o, ctx.assets, &txt, &ls, px, y - fs * 0.3, 0.0);
            }
            o.push(Op::Stroke { path: g, paint: solid(alpha(th.text, 0.12 * gi)), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
        }
        let n = self.series[0].len();
        let every = if n <= 8 { 1 } else { n.div_ceil(6) };
        for (i, l) in self.labels.iter().enumerate().take(n) {
            if i % every != 0 && i != n - 1 {
                continue;
            }
            let x = px + pw * i as f32 / (n - 1).max(1) as f32;
            let al = if i == 0 { 0.0 } else if i == n - 1 { 1.0 } else { 0.5 };
            draw(&mut o, ctx.assets, l, &small(th, fs * 0.8).tabular().color(alpha(th.muted, gi)), x, py + ph + fs * 1.5, al);
        }
        // Context first, the story line last (on top).
        let order: Vec<usize> = (0..self.series.len()).filter(|i| *i != hl).chain([hl]).collect();
        let mut ends: Vec<(usize, f32, f32)> = Vec::new();
        for (oi, &si) in order.iter().enumerate() {
            let s = &self.series[si];
            let story = si == hl;
            let start = if story { self.delay + self.stagger * (order.len() - 1) as f32 } else { self.delay + oi as f32 * self.stagger * 0.3 };
            let k = self.motion.ease(seg(t, start, if story { self.duration } else { self.duration * 0.7 })).min(1.0);
            if k <= 0.0 {
                continue;
            }
            let pts: Vec<(f32, f32)> = s.iter().enumerate().map(|(i, v)| (px + pw * i as f32 / (s.len() - 1) as f32, yv(*v))).collect();
            let path = if self.smooth { smooth_path(&pts, 1.0) } else { poly(&pts) };
            let (c, wd) = if story { (accent, self.width * 1.25) } else { (grey, self.width * 0.55) };
            let clip = Some((rrect_path(px - 12.0, py - 40.0, pw * k + 12.0, ph + 52.0, 0.0), IDENTITY));
            o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip, cache_key: None });
            o.push(stroke_op(path, solid(c), wd, [0.0, 1.0]));
            o.push(Op::PopLayer);
            if k >= 1.0 {
                let e = *pts.last().unwrap();
                ends.push((si, e.0, e.1));
                if story {
                    o.push(circle(e.0, e.1, wd * 1.6, solid(c)));
                }
            }
            if story {
                for note in self.notes.iter().filter(|nt| nt.index < s.len()) {
                    let p = pts[note.index];
                    let reached = note.index as f32 / (s.len() - 1) as f32;
                    let a = ease_out(seg(t, start + self.duration * reached, 0.35));
                    if a > 0.0 && k >= reached {
                        // A leader up to the label, text right-aligned to the leader (reads into the line).
                        let top = py + fs * 1.2;
                        o.push(rect(p.0 - 0.75, top, 1.5, (p.1 - top - wd * 2.0).max(0.0) * a, 0.0, solid(alpha(th.text, 0.6 * a))));
                        let ns = small(th, fs * 0.8).color(alpha(th.text, a));
                        draw(&mut o, ctx.assets, &note.text, &ns, p.0 - fs * 0.4, top + fs * 0.6, 1.0);
                    }
                }
            }
        }
        // Direct labels, nudged apart vertically.
        ends.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut last_y = f32::MIN;
        for (si, x, y) in ends {
            let story = si == hl;
            let gap = if story { fs * 1.2 } else { fs * 0.95 };
            let yy = y.max(last_y + gap);
            last_y = yy;
            let a = ease_out(seg(t, self.delay + self.duration * 0.9 + if story { self.stagger * (self.series.len() - 1) as f32 } else { 0.0 }, 0.3));
            let st = if story { hs.clone().color(alpha(accent, a)) } else { cs.clone().color(alpha(th.muted, a)) };
            draw(&mut o, ctx.assets, &lab(si), &st, x + fs * 0.6, yy + st.size * 0.35, 0.0);
        }
        o
    }
}

impl Component for LineChart {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        if self.look == "editorial" {
            return self.eval_editorial(t, ctx);
        }
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let all = self.series.iter().flatten();
        let vmax = all.clone().fold(f32::MIN, |m, v| m.max(*v));
        let vmin = all.fold(f32::MAX, |m, v| m.min(*v));
        let lo = self.min.unwrap_or(if vmin >= 0.0 { 0.0 } else { vmin });
        let (hi, step) = match self.max {
            Some(m) => (m, (m - lo) / 4.0),
            None => {
                let (n, s) = nice(vmax - lo, 4);
                (lo + n, s)
            }
        };
        let left = if self.grid { ctx.assets.measure(&self.format.fmt(hi as f64), &small(th, fs * 0.8)) + fs * 0.9 } else { 0.0 };
        let right = if self.end_label { fs * 4.5 } else { fs };
        let legend = if self.names.is_empty() { 0.0 } else { fs * 2.2 };
        let (px, py, pw, ph) = (left, fs + legend, w - left - right, h - fs - legend - if self.labels.is_empty() { fs } else { fs * 2.4 });
        let mut o = Vec::new();
        let gi = ease_out(seg(t, self.delay - 0.2, 0.6));
        if self.grid && gi > 0.0 {
            grid_lines(&mut o, ctx, &self.format, fs, gi, false, [px, py, pw, ph], lo, hi, step, self.dashed);
        }
        let n = self.series[0].len();
        for (i, l) in self.labels.iter().enumerate().take(n) {
            let x = px + pw * i as f32 / (n - 1).max(1) as f32;
            draw(&mut o, ctx.assets, l, &small(th, fs * 0.9).color(alpha(th.muted, gi)), x, py + ph + fs * 1.7, 0.5);
        }
        let mut lx = px;
        for (si, name) in self.names.iter().enumerate() {
            let c = colors(&self.colors, th, si);
            o.push(rect(lx, fs * 0.42, fs * 1.1, fs * 0.36, fs * 0.18, solid(alpha(c, gi))));
            lx += fs * 1.6;
            lx += draw(&mut o, ctx.assets, name, &small(th, fs * 0.9).color(alpha(th.text, gi)), lx, fs * 0.9, 0.0) + fs * 1.6;
        }
        let area = self.area.unwrap_or(false);
        let yv = |v: f32| py + ph - ph * (v - lo) / (hi - lo);
        for (si, s) in self.series.iter().enumerate() {
            let k = self.motion.ease(seg(t, self.delay + si as f32 * self.stagger, self.duration)).min(1.0);
            if k <= 0.0 {
                continue;
            }
            let c = colors(&self.colors, th, si);
            let pts: Vec<(f32, f32)> = s.iter().enumerate().map(|(i, v)| (px + pw * i as f32 / (s.len() - 1) as f32, yv(*v))).collect();
            let path = if self.smooth { smooth_path(&pts, 1.0) } else { poly(&pts) };
            // Current head position along x.
            let xf = (s.len() - 1) as f32 * k;
            let i0 = (xf.floor() as usize).min(s.len() - 2);
            let fr = xf - i0 as f32;
            let head_v = s[i0] + (s[i0 + 1] - s[i0]) * fr;
            let head = (px + pw * k, yv(head_v));
            let clip = Some((rrect_path(px - 12.0, py - 40.0, pw * k + 12.0, ph + 52.0, 0.0), IDENTITY));
            if area {
                let mut a = path.clone();
                a.line_to(((px + pw) as f64, (py + ph) as f64));
                a.line_to((px as f64, (py + ph) as f64));
                a.close_path();
                o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: clip.clone(), cache_key: None });
                let top_y = pts.iter().fold(f32::MAX, |m, p| m.min(p.1));
                o.push(fill(a, Paint::Linear { p0: [0.0, top_y], p1: [0.0, py + ph], stops: vec![(0.0, alpha(c, 0.42)), (0.55, alpha(c, 0.12)), (1.0, alpha(c, 0.0))] }));
                o.push(Op::PopLayer);
            }
            if self.crosshair && k < 1.0 {
                o.push(rect(head.0 - 0.5, py, 1.0, ph, 0.0, Paint::Linear { p0: [0.0, py], p1: [0.0, py + ph], stops: vec![(0.0, alpha(c, 0.0)), (0.5, alpha(c, 0.5)), (1.0, alpha(c, 0.0))] }));
            }
            o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip, cache_key: None });
            if self.glow {
                super::polish::bloom(&mut o, vec![stroke_op(path.clone(), solid(c), self.width * 2.0, [0.0, 1.0])], self.width * 2.5, 0.7);
            }
            o.push(stroke_op(path, Paint::Linear { p0: [px, 0.0], p1: [px + pw, 0.0], stops: vec![(0.0, shade(c, -0.15)), (1.0, shade(c, 0.12))] }, self.width, [0.0, 1.0]));
            o.push(Op::PopLayer);
            if self.dots {
                for (i, p) in pts.iter().enumerate() {
                    let d = spring_z(t - self.delay - si as f32 * self.stagger - self.duration * i as f32 / (s.len() - 1) as f32 * 0.8, 1.4, 0.5);
                    if d > 0.0 && p.0 <= head.0 + 1.0 {
                        o.push(circle(p.0, p.1, self.width * 1.9 * d, solid(th.background)));
                        o.push(Op::Stroke { path: circle_path(p.0, p.1, self.width * 1.3 * d), paint: solid(c), stroke: line(self.width * 0.6, [0.0, 1.0]), transform: IDENTITY });
                    }
                }
            }
            // Head dot with halo.
            o.push(circle(head.0, head.1, self.width * 4.2, solid(alpha(c, 0.18))));
            o.push(circle(head.0, head.1, self.width * 2.0, solid(th.background)));
            o.push(circle(head.0, head.1, self.width * 1.3, solid(c)));
            if self.end_label {
                let txt = self.format.fmt(head_v as f64);
                let st = TextStyle::new(Face::Sans, fs * 0.85, 650, on_color(&[c])).tabular();
                let tw = ctx.assets.measure(&self.format.fmt(s.iter().fold(f32::MIN, |m, v| m.max(*v)) as f64), &st).max(ctx.assets.measure(&txt, &st));
                let (bx, by) = (head.0 + fs * 0.9, head.1 - fs * 0.8);
                let mut e = Vec::new();
                shadow(&mut e, bx, by, tw + fs, fs * 1.6, fs * 0.8, 3.0, 8.0, 0.3);
                e.push(rect(bx, by, tw + fs, fs * 1.6, fs * 0.8, solid(c)));
                draw(&mut e, ctx.assets, &txt, &st, bx + fs * 0.5, by + fs * 0.8 + st.size * 0.36, 0.0);
                o.extend(e);
            }
            for note in self.notes.iter().filter(|nt| nt.series == si && nt.index < s.len()) {
                let p = pts[note.index];
                let reached = note.index as f32 / (s.len() - 1) as f32;
                let a = spring_z(t - self.delay - si as f32 * self.stagger - self.duration * reached, 1.1, 0.7);
                if a > 0.0 && k >= reached {
                    o.push(Op::Stroke { path: circle_path(p.0, p.1, self.width * 2.4), paint: solid(c), stroke: line(2.0, [0.0, 1.0]), transform: IDENTITY });
                    value_pill(&mut o, ctx, &note.text, p.0, p.1 - self.width * 2.4, fs, th.text, a);
                }
            }
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration + self.stagger * self.series.len() as f32)
    }
}

fn poly(pts: &[(f32, f32)]) -> Path {
    let mut p = Path::new();
    for (i, q) in pts.iter().enumerate() {
        if i == 0 {
            p.move_to((q.0 as f64, q.1 as f64));
        } else {
            p.line_to((q.0 as f64, q.1 as f64));
        }
    }
    p
}

/// Line chart with an area fill (same parameters as `line-chart`).
#[derive(Deserialize)]
#[serde(transparent)]
pub struct AreaChart(LineChart);
impl Prepare for AreaChart {
    fn prepare(&mut self) -> Result<(), Error> {
        self.0.area.get_or_insert(true);
        self.0.prepare()
    }
}
impl Component for AreaChart {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        self.0.eval(t, ctx)
    }
    fn duration(&self) -> Option<f32> {
        self.0.duration()
    }
}

// ------------------------------------------------------------------ donut / pie

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DonutChart {
    pub values: Vec<f32>,
    pub labels: Vec<String>,
    pub colors: Vec<Color>,
    /// Hole radius as a fraction of the radius (0 = pie).
    pub hole: Option<f32>,
    /// Gap between segments in degrees.
    pub gap: f32,
    /// Centre text (default: the total); "" for none.
    pub center: Option<String>,
    pub center_label: String,
    pub legend: bool,
    /// Show percentages on/next to segments.
    pub percents: bool,
    pub delay: f32,
    pub duration: f32,
    pub format: Format,
    pub font_size: f32,
}
impl Default for DonutChart {
    fn default() -> Self {
        DonutChart { values: Vec::new(), labels: Vec::new(), colors: Vec::new(), hole: None, gap: 1.5, center: None, center_label: String::new(), legend: true, percents: true, delay: 0.0, duration: 1.3, format: Format::default(), font_size: 22.0 }
    }
}
impl Prepare for DonutChart {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.values.is_empty() || self.values.iter().any(|v| *v < 0.0 || !v.is_finite()) {
            return Err(Error("donut/pie needs non-negative `values`".into()));
        }
        if self.values.iter().sum::<f32>() <= 0.0 {
            return Err(Error("donut/pie values sum to zero".into()));
        }
        Ok(())
    }
}
impl Component for DonutChart {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let fs = self.font_size;
        let legend_w = if self.legend && !self.labels.is_empty() { self.labels.iter().map(|l| ctx.assets.measure(l, &small(th, fs))).fold(0.0, f32::max) + fs * 6.0 } else { 0.0 };
        let size = (ctx.width - legend_w).min(ctx.height);
        let (cx, cy) = (size * 0.5, ctx.height * 0.5);
        let r_out = size * 0.5 - fs;
        let hole = self.hole.unwrap_or(0.62).clamp(0.0, 0.95);
        let r_in = r_out * hole;
        let total: f32 = self.values.iter().sum();
        let k = ease_in_out(seg(t, self.delay, self.duration));
        let mut o = Vec::new();
        let mut acc = 0.0;
        let gap = self.gap / 360.0;
        for (i, v) in self.values.iter().enumerate() {
            let f = v / total;
            let (a0, a1) = (acc, acc + f);
            acc = a1;
            // Sweep reveals clockwise over the whole ring.
            let b1 = a1.min(k);
            if b1 <= a0 {
                continue;
            }
            let c = colors(&self.colors, th, i);
            let pop = spring(t - self.delay - self.duration * a1, 1.2);
            let (s0, s1) = (a0 + gap * 0.5, (b1 - gap * 0.5).max(a0 + gap * 0.5));
            let p = sector(cx, cy, r_in, r_out, s0, s1);
            let mid = (a0 + a1) * 0.5 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            o.push(fill(p, Paint::Linear { p0: [cx + mid.cos() * r_in, cy + mid.sin() * r_in], p1: [cx + mid.cos() * r_out, cy + mid.sin() * r_out], stops: vec![(0.0, shade(c, -0.12)), (1.0, shade(c, 0.1))] }));
            if self.percents && pop > 0.0 && f > 0.04 {
                let rr = if hole > 0.2 { (r_in + r_out) * 0.5 } else { r_out * 0.62 };
                let txt = format!("{}%", (f * 100.0).round());
                draw(&mut o, ctx.assets, &txt, &TextStyle::new(Face::Sans, fs * 0.9, 700, alpha(Rgba([1.0, 1.0, 1.0, 1.0]), clamp01(pop))), cx + mid.cos() * rr, cy + mid.sin() * rr + fs * 0.32, 0.5);
            }
        }
        if hole > 0.2 {
            let center = self.center.clone().unwrap_or_else(|| self.format.fmt((total * k) as f64));
            if !center.is_empty() {
                let st = TextStyle::new(Face::Display, r_in * 0.42, 700, th.text).tracking(-0.02).tabular();
                draw(&mut o, ctx.assets, &center, &st, cx, cy + st.size * if self.center_label.is_empty() { 0.35 } else { 0.18 }, 0.5);
                draw(&mut o, ctx.assets, &self.center_label, &small(th, (r_in * 0.14).max(12.0)), cx, cy + r_in * 0.38, 0.5);
            }
        }
        if legend_w > 0.0 {
            let x0 = size + fs * 1.5;
            let rows = self.labels.len() as f32;
            let y0 = cy - rows * fs * 1.1;
            for (i, l) in self.labels.iter().enumerate() {
                let a = ease_out(seg(t, self.delay + 0.1 * i as f32, 0.5));
                let y = y0 + i as f32 * fs * 2.2;
                let mut e = vec![rect(x0, y, fs * 0.9, fs * 0.9, fs * 0.25, solid(colors(&self.colors, th, i)))];
                draw(&mut e, ctx.assets, l, &small(th, fs).color(th.text), x0 + fs * 1.5, y + fs * 0.82, 0.0);
                let v = self.values.get(i).copied().unwrap_or(0.0);
                draw(&mut e, ctx.assets, &self.format.fmt(v as f64), &small(th, fs), x0 + legend_w - fs * 1.5, y + fs * 0.82, 1.0);
                transform_ops(&mut e, &translate((1.0 - a) * 16.0, 0.0));
                o.extend(with_opacity(e, a));
            }
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration + 0.4)
    }
}

/// Annular sector between turn fractions `a..b` (clockwise from 12 o'clock).
fn sector(cx: f32, cy: f32, r0: f32, r1: f32, a: f32, b: f32) -> Path {
    let outer = arc_path(cx, cy, r1, a, b);
    let mut p = Path::new();
    let mut first = true;
    for el in outer.elements() {
        match *el {
            kurbo::PathEl::MoveTo(q) if first => {
                p.move_to(q);
                first = false;
            }
            kurbo::PathEl::MoveTo(_) => {}
            e => p.push(e),
        }
    }
    if r0 > 0.5 {
        let inner = arc_path(cx, cy, r0, a, b);
        // Walk the inner arc backwards.
        let segs: Vec<kurbo::PathSeg> = inner.segments().collect();
        let end = segs.last().map(|s| { use kurbo::ParamCurve; s.end() }).unwrap_or_default();
        p.line_to(end);
        for s in segs.iter().rev() {
            match s.reverse() {
                kurbo::PathSeg::Line(l) => p.line_to(l.p1),
                kurbo::PathSeg::Quad(q) => p.quad_to(q.p1, q.p2),
                kurbo::PathSeg::Cubic(c) => p.curve_to(c.p1, c.p2, c.p3),
            }
        }
    } else {
        p.line_to((cx as f64, cy as f64));
    }
    p.close_path();
    p
}

/// A donut with no hole (same parameters as `donut-chart`).
#[derive(Deserialize)]
#[serde(transparent)]
pub struct PieChart(DonutChart);
impl Prepare for PieChart {
    fn prepare(&mut self) -> Result<(), Error> {
        self.0.hole.get_or_insert(0.0);
        self.0.prepare()
    }
}
impl Component for PieChart {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        self.0.eval(t, ctx)
    }
    fn duration(&self) -> Option<f32> {
        self.0.duration()
    }
}

// ------------------------------------------------------------------ counter

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Counter {
    pub from: f64,
    pub to: f64,
    pub prefix: String,
    pub suffix: String,
    pub decimals: usize,
    pub compact: bool,
    pub separator: bool,
    pub size: f32,
    pub color: Option<Color>,
    pub gradient: Option<[Color; 2]>,
    pub weight: u16,
    pub delay: f32,
    pub duration: f32,
    /// 0 left, 0.5 centre (default), 1 right of the box.
    pub align: f32,
    /// Caption under the number.
    pub label: String,
    /// Number face: display (default), sans, serif, mono.
    pub face: Option<Face>,
    /// Count curve: smooth (expo, default), snappy, soft (ease in-out), bouncy.
    pub motion: Motion,
}
impl Default for Counter {
    fn default() -> Self {
        Counter { from: 0.0, to: 100.0, prefix: String::new(), suffix: String::new(), decimals: 0, compact: false, separator: true, size: 160.0, color: None, gradient: None, weight: 700, delay: 0.0, duration: 1.8, align: 0.5, label: String::new(), face: None, motion: Motion::Smooth }
    }
}
impl Prepare for Counter {}
impl Counter {
    pub fn text_at(&self, t: f32) -> String {
        let k = self.motion.ease(seg(t, self.delay, self.duration)) as f64;
        let v = self.from + (self.to - self.from) * k;
        let body = if self.compact { compact_number(v) } else { format_number(v, self.decimals, self.separator) };
        format!("{}{}{}", self.prefix, body, self.suffix)
    }
}
impl Component for Counter {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let p = ease_out(seg(t, self.delay - 0.1, 0.5));
        if p <= 0.0 {
            return Vec::new();
        }
        let txt = self.text_at(t);
        let ink = self.color.map(|c| c.0).unwrap_or(th.text);
        let st = match self.face {
            None => TextStyle::new(Face::Display, self.size, self.weight, ink).tracking(-0.03),
            f => super::polish::face_style(f, self.size, self.weight, ink),
        }
        .tabular();
        // Measure the final value so the number does not jitter while counting.
        let fw = ctx.assets.measure(&self.text_at(f32::MAX), &st).max(ctx.assets.measure(&txt, &st));
        let x = ctx.width * self.align - fw * self.align;
        let base = (ctx.height * 0.5 + self.size * 0.36).min(self.size * 0.95 + (ctx.height - self.size) * 0.5);
        let mut o = Vec::new();
        let tw = ctx.assets.measure(&txt, &st);
        let st = match self.gradient {
            Some([a, b]) => st.gradient(a.0, b.0, [0.0, base - self.size * 0.75], [0.0, base]),
            None => st,
        };
        draw(&mut o, ctx.assets, &txt, &st, x + fw - tw, base, 0.0);
        if !self.label.is_empty() {
            draw(&mut o, ctx.assets, &self.label, &TextStyle::new(Face::Sans, self.size * 0.2, 500, th.muted), ctx.width * self.align, base + self.size * 0.42, self.align);
        }
        transform_ops(&mut o, &translate(0.0, (1.0 - p) * self.size * 0.15));
        with_opacity(o, p)
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration)
    }
}

// ------------------------------------------------------------------ stat tile

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Stat {
    pub label: String,
    pub value: f64,
    pub prefix: String,
    pub suffix: String,
    pub decimals: usize,
    pub compact: bool,
    /// Percent change; positive is green with an up arrow.
    pub delta: Option<f32>,
    /// Sparkline values.
    pub spark: Vec<f32>,
    pub color: Option<Color>,
    pub delay: f32,
    pub duration: f32,
    /// Tile finish: solid, glass, outline, flat, paper.
    pub surface: Surface,
    /// Value face: display (default), sans, serif, mono.
    pub face: Option<Face>,
    pub radius: Option<f32>,
    pub motion: Motion,
}
impl Default for Stat {
    fn default() -> Self {
        Stat { label: String::new(), value: 0.0, prefix: String::new(), suffix: String::new(), decimals: 0, compact: false, delta: None, spark: Vec::new(), color: None, delay: 0.0, duration: 1.4, surface: Surface::Solid, face: None, radius: None, motion: Motion::Smooth }
    }
}
impl Prepare for Stat {}
impl Component for Stat {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width.min(4000.0), ctx.height.min(4000.0));
        let p = self.motion.enter(t - self.delay);
        if p <= 0.0 {
            return Vec::new();
        }
        let mut o = Vec::new();
        super::polish::finish(&mut o, th, self.surface, 0.0, 0.0, w, h, self.radius.unwrap_or(th.radius), 1.0, None);
        let pad = (h * 0.14).clamp(16.0, 32.0);
        let ls = TextStyle::new(Face::Sans, (h * 0.1).clamp(13.0, 22.0), 500, th.muted);
        draw(&mut o, ctx.assets, &self.label, &ls, pad, pad + ls.size * 0.8, 0.0);
        let k = super::polish::ease_out_expo(seg(t, self.delay + 0.1, self.duration)) as f64;
        let v = self.value * k;
        let body = if self.compact { compact_number(v) } else { format_number(v, self.decimals, true) };
        let vsz = (h * 0.3).clamp(24.0, 96.0);
        let vs = match self.face {
            None => super::polish::headline(vsz, 700, th.text),
            f => super::polish::face_style(f, vsz, 700, th.text),
        }
        .tabular();
        let vy = pad + ls.size * 1.4 + vs.size * 0.85;
        let vw = draw(&mut o, ctx.assets, &format!("{}{}{}", self.prefix, body, self.suffix), &vs, pad, vy, 0.0);
        if let Some(d) = self.delta {
            let a = ease_out(seg(t, self.delay + self.duration * 0.6, 0.4));
            let c = if d >= 0.0 { th.positive } else { th.negative };
            let txt = format!("{}{:.1}%", if d >= 0.0 { "▲ " } else { "▼ " }, d.abs());
            let ds = TextStyle::new(Face::Sans, ls.size * 0.9, 600, alpha(c, a));
            let tw = ctx.assets.measure(&txt, &ds);
            let fits = pad + vw + pad * 0.5 + tw + ds.size < w - pad * 0.5;
            // Beside the value when it fits, else pinned top-right next to the label.
            let (bx, by) = if fits { (pad + vw + pad * 0.5, vy - vs.size * 0.5 - ds.size * 0.9) } else { (w - pad - tw - ds.size, pad + ls.size * 0.8 - ds.size * 1.25) };
            o.push(rect(bx, by, tw + ds.size, ds.size * 1.8, ds.size * 0.9, solid(alpha(c, 0.15 * a))));
            draw(&mut o, ctx.assets, &txt, &ds, bx + ds.size * 0.5, by + ds.size * 1.25, 0.0);
        }
        if self.spark.len() >= 2 {
            let c = self.color.map(|c| c.0).unwrap_or(th.accent);
            let (sx, sy, sw, sh) = (pad, vy + pad * 0.6, w - 2.0 * pad, (h - vy - pad * 1.6).max(10.0));
            let (lo, hi) = self.spark.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
            let pts: Vec<(f32, f32)> = self.spark.iter().enumerate().map(|(i, v)| (sx + sw * i as f32 / (self.spark.len() - 1) as f32, sy + sh - sh * (v - lo) / (hi - lo).max(1e-6))).collect();
            let path = smooth_path(&pts, 1.0);
            let kk = ease_in_out(seg(t, self.delay + 0.2, self.duration));
            let mut a = path.clone();
            a.line_to(((sx + sw) as f64, (sy + sh) as f64));
            a.line_to((sx as f64, (sy + sh) as f64));
            a.close_path();
            o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((rrect_path(sx - 4.0, sy - 8.0, sw * kk + 8.0, sh + 12.0, 0.0), IDENTITY)), cache_key: None });
            o.push(fill(a, vgrad(sy, sy + sh, alpha(c, 0.3), alpha(c, 0.0))));
            o.push(stroke_op(path, solid(c), 2.5, [0.0, 1.0]));
            o.push(Op::PopLayer);
        }
        transform_ops(&mut o, &crate::dl::mul(&translate(0.0, (1.0 - p) * 20.0), &scale_about(lerp(0.96, 1.0, p), w * 0.5, h * 0.5)));
        with_opacity(o, clamp01(p * 1.6))
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration + 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use super::*;
    use serde_json::json;

    #[test]
    fn nice_ticks() {
        assert_eq!(nice(9.0, 4), (10.0, 2.5));
        assert_eq!(nice(87.0, 4), (100.0, 25.0));
        assert_eq!(nice(1234.0, 4), (1500.0, 500.0));
    }

    #[test]
    fn counter_counts() {
        let c: Counter = serde_json::from_value(json!({"to": 12500, "prefix": "$", "duration": 1.0})).unwrap();
        assert_eq!(c.text_at(0.0), "$0");
        assert_eq!(c.text_at(2.0), "$12,500");
    }

    #[test]
    fn gallery() {
        let th = Theme::default();
        let a = preview::host();
        let items = [
            ("bar-chart", json!({"x": 30, "y": 20, "width": 440, "height": 240, "values": [32, 54, 41, 78, 66, 92], "labels": ["Jan", "Feb", "Mar", "Apr", "May", "Jun"], "highlight": 5})),
            ("line-chart", json!({"x": 500, "y": 20, "width": 430, "height": 240, "series": [[12, 18, 15, 26, 31, 29, 42], [8, 10, 14, 13, 17, 22, 24]], "names": ["Anikit", "Browser"], "labels": ["M", "T", "W", "T", "F", "S", "S"]})),
            ("donut-chart", json!({"x": 30, "y": 290, "width": 440, "height": 220, "values": [45, 30, 25], "labels": ["Render", "Encode", "Audio"], "center_label": "frames"})),
            ("stat", json!({"x": 500, "y": 290, "width": 210, "height": 150, "label": "Revenue", "value": 48200, "prefix": "$", "delta": 12.4, "spark": [3, 4, 3, 5, 6, 8]})),
            ("counter", json!({"x": 720, "y": 290, "width": 220, "height": 150, "to": 935, "suffix": " fps", "size": 56, "gradient": ["#ffffff", "#9c8cff"]})),
            ("bar-chart", json!({"x": 500, "y": 450, "width": 430, "height": 80, "values": [935, 77], "labels": ["Anikit", "Chrome"], "horizontal": true, "multicolor": true, "grid": false, "format": {"suffix": " fps"}})),
        ];
        for (name, t) in [("charts-mid", 0.7f32), ("charts-end", 4.0)] {
            let mut ops = Vec::new();
            for (k, p) in &items {
                ops.extend(build(k, p).unwrap().eval(t, 960.0, 540.0, 30.0, &th, a));
            }
            preview::validate(&ops).unwrap();
            preview::snapshot(name, 960, 540, th.background, ops, a);
        }
        let pie = build("pie-chart", &json!({"values": [1, 2, 3]})).unwrap();
        preview::validate(&pie.eval(5.0, 400.0, 300.0, 30.0, &th, a)).unwrap();
    }
}
