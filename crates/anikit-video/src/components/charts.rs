//! Charts: bar, line, area, donut, pie; counters and stat tiles.
//!
//! Every chart builds on over `duration` seconds after `delay`, with a
//! per-item `stagger`. Value labels count up with their bars.

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
}
impl Default for BarChart {
    fn default() -> Self {
        BarChart { values: Vec::new(), labels: Vec::new(), colors: Vec::new(), multicolor: false, horizontal: false, max: None, delay: 0.0, duration: 0.9, stagger: 0.08, values_shown: true, grid: true, highlight: None, format: Format::default(), bar_width: 0.62, font_size: 16.0 }
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
        Ok(())
    }
}
impl Component for BarChart {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let n = self.values.len();
        let vmax = self.values.iter().fold(0.0f32, |m, v| m.max(*v));
        let (top, step) = match self.max {
            Some(m) => (m, m / 4.0),
            None => nice(vmax, 4),
        };
        let fs = self.font_size;
        let mut o = Vec::new();
        let has_labels = !self.labels.is_empty();
        // Plot rectangle.
        let (px, py, pw, ph) = if self.horizontal {
            let lw = self.labels.iter().map(|l| ctx.assets.measure(l, &small(th, fs))).fold(0.0, f32::max);
            let lw = if has_labels { lw + fs } else { 0.0 };
            (lw, 0.0, w - lw - fs * 4.0, h - if self.grid { fs * 1.8 } else { 0.0 })
        } else {
            let left = if self.grid { ctx.assets.measure(&self.format.fmt(top as f64), &small(th, fs * 0.85)) + fs * 0.8 } else { 0.0 };
            (left, fs * 1.8, w - left, h - fs * 1.8 - if has_labels { fs * 2.2 } else { 0.0 })
        };
        let grid_in = ease_out(seg(t, self.delay - 0.2, 0.6));
        if self.grid && grid_in > 0.0 {
            let mut g = Path::new();
            let ticks = (top / step).round() as i32;
            for i in 0..=ticks {
                let v = i as f32 * step;
                let f = v / top;
                if self.horizontal {
                    let x = (px + pw * f).round() + 0.5;
                    g.move_to((x as f64, py as f64));
                    g.line_to((x as f64, (py + ph) as f64));
                    draw(&mut o, ctx.assets, &self.format.fmt(v as f64), &small(th, fs * 0.85).color(alpha(th.muted, grid_in)), x, py + ph + fs * 1.4, 0.5);
                } else {
                    let y = (py + ph - ph * f).round() + 0.5;
                    g.move_to((px as f64, y as f64));
                    g.line_to(((px + pw) as f64, y as f64));
                    draw(&mut o, ctx.assets, &self.format.fmt(v as f64), &small(th, fs * 0.85).color(alpha(th.muted, grid_in)), px - fs * 0.6, y + fs * 0.3, 1.0);
                }
            }
            o.push(Op::Stroke { path: g, paint: solid(alpha(th.text, 0.07 * grid_in)), stroke: crate::dl::Stroke { width: 1.0, cap: 0, join: 0, miter_limit: 4.0, dash: vec![3.0, 4.0], dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY });
        }
        let slot = if self.horizontal { ph / n as f32 } else { pw / n as f32 };
        let bw = slot * self.bar_width.clamp(0.1, 1.0);
        for (i, &v) in self.values.iter().enumerate() {
            let k = spring(t - self.delay - i as f32 * self.stagger, 0.9 / self.duration.max(0.1)).max(0.0);
            let shown = v * k;
            let f = (shown / top).max(0.0);
            let base_c = if self.multicolor || self.colors.len() > 1 { colors(&self.colors, th, i) } else { self.colors.first().map(|c| c.0).unwrap_or(th.accent) };
            let dimmed = self.highlight.is_some_and(|hi| hi != i);
            let c = if dimmed { mix(base_c, th.surface2, 0.65) } else { base_c };
            let r = (bw * 0.18).min(10.0);
            let label_a = clamp01(k * 3.0 - 1.0);
            if self.horizontal {
                let y = py + slot * i as f32 + (slot - bw) * 0.5;
                let len = pw * f;
                if len > 0.5 {
                    o.push(rect(px, y, len, bw, r.min(len * 0.5), Paint::Linear { p0: [px, 0.0], p1: [px + len, 0.0], stops: vec![(0.0, shade(c, -0.15)), (1.0, shade(c, 0.12))] }));
                }
                if let Some(l) = self.labels.get(i) {
                    draw(&mut o, ctx.assets, l, &small(th, fs).color(if dimmed { alpha(th.muted, 0.6) } else { th.muted }), px - fs * 0.6, y + bw * 0.5 + fs * 0.35, 1.0);
                }
                if self.values_shown && label_a > 0.0 {
                    draw(&mut o, ctx.assets, &self.format.fmt(shown as f64), &TextStyle::new(Face::Sans, fs, 600, alpha(th.text, label_a)).tabular(), px + len + fs * 0.5, y + bw * 0.5 + fs * 0.35, 0.0);
                }
            } else {
                let x = px + slot * i as f32 + (slot - bw) * 0.5;
                let len = ph * f;
                let y = py + ph - len;
                if len > 0.5 {
                    o.push(rect(x, y, bw, len, r.min(len * 0.5), vgrad(y, py + ph, shade(c, 0.12), shade(c, -0.18))));
                    // Squared-off bottom corners.
                    o.push(rect(x, (py + ph - r).max(y + len * 0.5), bw, r.min(len * 0.5), 0.0, solid(shade(c, -0.18))));
                    if !dimmed {
                        o.push(rect(x, y, bw, 2.0f32.min(len), r.min(len * 0.5), solid(Rgba([1.0, 1.0, 1.0, 0.25]))));
                    }
                }
                if let Some(l) = self.labels.get(i) {
                    draw(&mut o, ctx.assets, l, &small(th, fs).color(if dimmed { alpha(th.muted, 0.6) } else { th.muted }), x + bw * 0.5, py + ph + fs * 1.6, 0.5);
                }
                if self.values_shown && label_a > 0.0 {
                    draw(&mut o, ctx.assets, &self.format.fmt(shown as f64), &TextStyle::new(Face::Sans, fs, 600, alpha(th.text, label_a * if dimmed { 0.6 } else { 1.0 })).tabular(), x + bw * 0.5, y - fs * 0.6, 0.5);
                }
            }
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration + self.stagger * self.values.len() as f32)
    }
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
}
impl Default for LineChart {
    fn default() -> Self {
        LineChart { series: Vec::new(), names: Vec::new(), labels: Vec::new(), colors: Vec::new(), area: None, dots: false, end_label: true, smooth: true, min: None, max: None, grid: true, delay: 0.0, duration: 1.6, stagger: 0.2, width: 3.0, format: Format::default(), font_size: 16.0 }
    }
}
impl Prepare for LineChart {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.series.is_empty() || self.series.iter().any(|s| s.len() < 2) {
            return Err(Error("line-chart needs `series`: arrays of at least 2 numbers".into()));
        }
        Ok(())
    }
}
impl Component for LineChart {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
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
        let left = if self.grid { ctx.assets.measure(&self.format.fmt(hi as f64), &small(th, fs * 0.85)) + fs * 0.8 } else { 0.0 };
        let right = if self.end_label { fs * 4.5 } else { fs };
        let legend = if self.names.is_empty() { 0.0 } else { fs * 2.2 };
        let (px, py, pw, ph) = (left, fs + legend, w - left - right, h - fs - legend - if self.labels.is_empty() { fs } else { fs * 2.4 });
        let mut o = Vec::new();
        let gi = ease_out(seg(t, self.delay - 0.2, 0.6));
        if self.grid && gi > 0.0 {
            let mut g = Path::new();
            let ticks = ((hi - lo) / step).round() as i32;
            for i in 0..=ticks {
                let v = lo + i as f32 * step;
                let y = (py + ph - ph * (v - lo) / (hi - lo)).round() + 0.5;
                g.move_to((px as f64, y as f64));
                g.line_to(((px + pw) as f64, y as f64));
                draw(&mut o, ctx.assets, &self.format.fmt(v as f64), &small(th, fs * 0.85).color(alpha(th.muted, gi)), px - fs * 0.6, y + fs * 0.3, 1.0);
            }
            o.push(Op::Stroke { path: g, paint: solid(alpha(th.text, 0.07 * gi)), stroke: crate::dl::Stroke { width: 1.0, cap: 0, join: 0, miter_limit: 4.0, dash: vec![3.0, 4.0], dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY });
        }
        let n = self.series[0].len();
        for (i, l) in self.labels.iter().enumerate().take(n) {
            let x = px + pw * i as f32 / (n - 1).max(1) as f32;
            draw(&mut o, ctx.assets, l, &small(th, fs).color(alpha(th.muted, gi)), x, py + ph + fs * 1.7, 0.5);
        }
        let mut lx = px;
        for (si, name) in self.names.iter().enumerate() {
            let c = colors(&self.colors, th, si);
            o.push(rect(lx, fs * 0.2, fs * 0.9, fs * 0.9, fs * 0.25, solid(c)));
            lx += fs * 1.4;
            lx += draw(&mut o, ctx.assets, name, &small(th, fs).color(alpha(th.text, gi)), lx, fs * 0.98, 0.0) + fs * 1.4;
        }
        let area = self.area.unwrap_or(false);
        for (si, s) in self.series.iter().enumerate() {
            let k = ease_in_out(seg(t, self.delay + si as f32 * self.stagger, self.duration));
            if k <= 0.0 {
                continue;
            }
            let c = colors(&self.colors, th, si);
            let pts: Vec<(f32, f32)> = s.iter().enumerate().map(|(i, v)| (px + pw * i as f32 / (s.len() - 1) as f32, py + ph - ph * (v - lo) / (hi - lo))).collect();
            let path = if self.smooth { smooth_path(&pts, 1.0) } else { poly(&pts) };
            // Current head position along x.
            let xf = (s.len() - 1) as f32 * k;
            let i0 = (xf.floor() as usize).min(s.len() - 2);
            let fr = xf - i0 as f32;
            let head_v = s[i0] + (s[i0 + 1] - s[i0]) * fr;
            let head = (px + pw * k, py + ph - ph * (head_v - lo) / (hi - lo));
            if area {
                let mut a = path.clone();
                a.line_to(((px + pw) as f64, (py + ph) as f64));
                a.line_to((px as f64, (py + ph) as f64));
                a.close_path();
                o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((rrect_path(px - 2.0, py - 10.0, pw * k + 2.0, ph + 12.0, 0.0), IDENTITY)), cache_key: None });
                let top_y = pts.iter().fold(f32::MAX, |m, p| m.min(p.1));
                o.push(fill(a, vgrad(top_y, py + ph, alpha(c, 0.38), alpha(c, 0.0))));
                o.push(Op::PopLayer);
            }
            // Glow pass + crisp line, drawn on by x (clip), so curves draw at even speed in time.
            o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((rrect_path(px - 10.0, py - 20.0, pw * k + 10.0, ph + 40.0, 0.0), IDENTITY)), cache_key: None });
            // Soft glow from layered translucent strokes (no blur pass).
            o.push(stroke_op(path.clone(), solid(alpha(c, 0.07)), self.width * 6.0, [0.0, 1.0]));
            o.push(stroke_op(path.clone(), solid(alpha(c, 0.12)), self.width * 3.2, [0.0, 1.0]));
            o.push(stroke_op(path, solid(c), self.width, [0.0, 1.0]));
            o.push(Op::PopLayer);
            if self.dots {
                for (i, p) in pts.iter().enumerate() {
                    let d = spring(t - self.delay - si as f32 * self.stagger - self.duration * i as f32 / (s.len() - 1) as f32, 1.4);
                    if d > 0.0 {
                        o.push(circle(p.0, p.1, self.width * 1.9 * d, solid(th.background)));
                        o.push(circle(p.0, p.1, self.width * 1.2 * d, solid(c)));
                    }
                }
            }
            // Head dot with halo.
            o.push(circle(head.0, head.1, self.width * 4.0, solid(alpha(c, 0.25))));
            o.push(circle(head.0, head.1, self.width * 1.8, solid(Rgba([1.0, 1.0, 1.0, 1.0]))));
            o.push(circle(head.0, head.1, self.width * 1.1, solid(c)));
            if self.end_label {
                let txt = self.format.fmt(head_v as f64);
                let st = TextStyle::new(Face::Sans, fs * 0.9, 600, Rgba([1.0, 1.0, 1.0, 1.0])).tabular();
                let tw = ctx.assets.measure(&txt, &st);
                let (bx, by) = (head.0 + fs * 0.8, head.1 - fs * 0.85);
                o.push(rect(bx, by, tw + fs, fs * 1.7, fs * 0.5, solid(c)));
                draw(&mut o, ctx.assets, &txt, &st, bx + fs * 0.5, by + fs * 1.17, 0.0);
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
        DonutChart { values: Vec::new(), labels: Vec::new(), colors: Vec::new(), hole: None, gap: 1.5, center: None, center_label: String::new(), legend: true, percents: true, delay: 0.0, duration: 1.3, format: Format::default(), font_size: 16.0 }
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
}
impl Default for Counter {
    fn default() -> Self {
        Counter { from: 0.0, to: 100.0, prefix: String::new(), suffix: String::new(), decimals: 0, compact: false, separator: true, size: 120.0, color: None, gradient: None, weight: 700, delay: 0.0, duration: 1.8, align: 0.5, label: String::new() }
    }
}
impl Prepare for Counter {}
impl Counter {
    pub fn text_at(&self, t: f32) -> String {
        let k = ease_out(seg(t, self.delay, self.duration)) as f64;
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
        let st = TextStyle::new(Face::Display, self.size, self.weight, self.color.map(|c| c.0).unwrap_or(th.text)).tracking(-0.03).tabular();
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
}
impl Default for Stat {
    fn default() -> Self {
        Stat { label: String::new(), value: 0.0, prefix: String::new(), suffix: String::new(), decimals: 0, compact: false, delta: None, spark: Vec::new(), color: None, delay: 0.0, duration: 1.4 }
    }
}
impl Prepare for Stat {}
impl Component for Stat {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width.min(4000.0), ctx.height.min(4000.0));
        let p = spring(t - self.delay, 1.0);
        if p <= 0.0 {
            return Vec::new();
        }
        let mut o = Vec::new();
        surface(&mut o, th, 0.0, 0.0, w, h, th.radius, 1.0);
        let pad = (h * 0.14).clamp(16.0, 32.0);
        let ls = TextStyle::new(Face::Sans, (h * 0.1).clamp(13.0, 22.0), 500, th.muted);
        draw(&mut o, ctx.assets, &self.label, &ls, pad, pad + ls.size * 0.8, 0.0);
        let k = ease_out(seg(t, self.delay + 0.1, self.duration)) as f64;
        let v = self.value * k;
        let body = if self.compact { compact_number(v) } else { format_number(v, self.decimals, true) };
        let vs = TextStyle::new(Face::Display, (h * 0.3).clamp(24.0, 96.0), 700, th.text).tracking(-0.03).tabular();
        let vy = pad + ls.size * 1.4 + vs.size * 0.85;
        let vw = draw(&mut o, ctx.assets, &format!("{}{}{}", self.prefix, body, self.suffix), &vs, pad, vy, 0.0);
        if let Some(d) = self.delta {
            let a = ease_out(seg(t, self.delay + self.duration * 0.6, 0.4));
            let c = if d >= 0.0 { th.positive } else { th.negative };
            let txt = format!("{}{:.1}%", if d >= 0.0 { "▲ " } else { "▼ " }, d.abs());
            let ds = TextStyle::new(Face::Sans, ls.size * 0.9, 600, alpha(c, a));
            let tw = ctx.assets.measure(&txt, &ds);
            let (bx, by) = (pad + vw + pad * 0.5, vy - vs.size * 0.5 - ds.size * 0.9);
            if bx + tw + ds.size < w {
                o.push(rect(bx, by, tw + ds.size, ds.size * 1.8, ds.size * 0.9, solid(alpha(c, 0.15 * a))));
                draw(&mut o, ctx.assets, &txt, &ds, bx + ds.size * 0.5, by + ds.size * 1.25, 0.0);
            }
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
