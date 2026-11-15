//! Easing functions.
//!
//! Every easing maps progress `t ∈ [0, 1]` to an eased progress (which may
//! overshoot for back/elastic/spring curves). Easings are plain data, cheap to
//! evaluate, and can be parsed from the string syntaxes people already know:
//!
//! * CSS: `ease`, `ease-in-out`, `cubic-bezier(.2,.8,.2,1)`, `steps(4, jump-end)`,
//!   `linear(0, 0.25 75%, 1)`, `step-start`
//! * Penner names: `easeInOutCubic`, `inOutCubic`, `outBack`, `circIn`
//! * GSAP style: `power2.inOut`, `back.out(1.7)`, `elastic.out(1, 0.3)`, `expo.in`
//! * Physics: `spring`, `spring(200, 20)`, `spring(bounce: 0.3, duration: 0.6)`
//! * Extras: `wiggle(5)`, `shake(8)`, `slowmo(0.7, 0.7)`, `anticipate`

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::bezier::CubicBezier;
use crate::math::{abs, asin, cos, exp2, floor, lerp, noise1, pow, sin, sqrt, PI, TAU};
use crate::spring::SpringParams;
use crate::util::{parse_numbers, split_call, Cursor};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Quad,
    Cubic,
    Quart,
    Quint,
    Sine,
    Expo,
    Circ,
    /// Overshoot amount (default 1.70158).
    Back(f64),
    /// Amplitude (>= 1) and period.
    Elastic(f64, f64),
    Bounce,
    /// Generic power curve `t^p`.
    Power(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    In,
    Out,
    InOut,
    OutIn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepPosition {
    JumpStart,
    JumpEnd,
    JumpNone,
    JumpBoth,
}

#[derive(Clone, Debug, PartialEq)]
#[derive(Default)]
pub enum Easing {
    #[default]
    Linear,
    Curve(Curve, Mode),
    Bezier(Box<CubicBezier>),
    Steps(u32, StepPosition),
    /// Duration-normalised spring: progress 1 maps to the settle time.
    Spring { params: SpringParams, velocity: f64, settle: f64 },
    /// CSS `linear()` control points, flattened `[x0, y0, x1, y1, ...]`.
    Points(Box<[f64]>),
    /// Uniformly sampled lookup table over `[0, 1]` (custom user functions).
    Table(Box<[f64]>),
    /// Oscillates around the start value and returns to it (use with relative values).
    Wiggle { count: f64, noise: bool, seed: u32 },
    /// Fast – slow (linear) – fast.
    SlowMo { ratio: f64, power: f64 },
    /// Pull back, then shoot to the target.
    Anticipate(f64),
    /// Time-reversed easing: `1 - f(1 - t)` (turns an "in" into an "out").
    Reverse(Box<Easing>),
    /// Goes there and back within one iteration: `f(2t)` then `f(2 - 2t)`.
    Mirror(Box<Easing>),
    /// Weighted mix of two easings: `a·(1-w) + b·w`.
    Blend(Box<Easing>, Box<Easing>, f64),
}


#[inline(always)]
fn curve_in(c: Curve, t: f64) -> f64 {
    match c {
        Curve::Quad => t * t,
        Curve::Cubic => t * t * t,
        Curve::Quart => {
            let t2 = t * t;
            t2 * t2
        }
        Curve::Quint => {
            let t2 = t * t;
            t2 * t2 * t
        }
        Curve::Sine => 1.0 - cos(t * (PI * 0.5)),
        Curve::Expo => {
            if t <= 0.0 {
                0.0
            } else {
                exp2(10.0 * t - 10.0)
            }
        }
        Curve::Circ => 1.0 - sqrt((1.0 - t * t).max(0.0)),
        Curve::Back(s) => t * t * ((s + 1.0) * t - s),
        Curve::Elastic(a, p) => {
            if t <= 0.0 || t >= 1.0 {
                return t;
            }
            let a = if a < 1.0 { 1.0 } else { a };
            let p = if p > 0.0 { p } else { 0.3 };
            let s = p / TAU * asin(1.0 / a);
            -(a * exp2(10.0 * (t - 1.0)) * sin((t - 1.0 - s) * TAU / p))
        }
        Curve::Bounce => 1.0 - bounce_out(1.0 - t),
        Curve::Power(p) => {
            if t <= 0.0 {
                0.0
            } else {
                pow(t, p)
            }
        }
    }
}

#[inline(always)]
fn bounce_out(t: f64) -> f64 {
    const N1: f64 = 7.5625;
    const D1: f64 = 2.75;
    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

#[inline(always)]
fn apply_mode(c: Curve, m: Mode, t: f64) -> f64 {
    match m {
        Mode::In => curve_in(c, t),
        Mode::Out => 1.0 - curve_in(c, 1.0 - t),
        Mode::InOut => {
            if t < 0.5 {
                0.5 * curve_in(c, 2.0 * t)
            } else {
                1.0 - 0.5 * curve_in(c, 2.0 - 2.0 * t)
            }
        }
        Mode::OutIn => {
            if t < 0.5 {
                0.5 * (1.0 - curve_in(c, 1.0 - 2.0 * t))
            } else {
                0.5 + 0.5 * curve_in(c, 2.0 * t - 1.0)
            }
        }
    }
}

fn steps(n: u32, pos: StepPosition, t: f64) -> f64 {
    let n = n.max(1) as f64;
    let mut step = floor(t * n);
    if matches!(pos, StepPosition::JumpStart | StepPosition::JumpBoth) {
        step += 1.0;
    }
    if t >= 0.0 && step < 0.0 {
        step = 0.0;
    }
    let jumps = match pos {
        StepPosition::JumpStart | StepPosition::JumpEnd => n,
        StepPosition::JumpNone => (n - 1.0).max(1.0),
        StepPosition::JumpBoth => n + 1.0,
    };
    if t <= 1.0 && step > jumps {
        step = jumps;
    }
    step / jumps
}

fn points(p: &[f64], t: f64) -> f64 {
    let n = p.len() / 2;
    if n == 0 {
        return t;
    }
    if n == 1 {
        return p[1];
    }
    // Find the segment containing t (points are sorted by x).
    let mut i = 0;
    while i + 2 < n && p[(i + 1) * 2] < t {
        i += 1;
    }
    // CSS: when several points share an x, use the last one for t past it.
    while i + 2 < n && p[(i + 1) * 2] <= t && p[(i + 2) * 2] <= t {
        i += 1;
    }
    let (x0, y0, x1, y1) = (p[i * 2], p[i * 2 + 1], p[i * 2 + 2], p[i * 2 + 3]);
    if x1 == x0 {
        return if t < x0 { y0 } else { y1 };
    }
    lerp(y0, y1, (t - x0) / (x1 - x0))
}

fn table(s: &[f64], t: f64) -> f64 {
    let n = s.len();
    if n == 0 {
        return t;
    }
    if n == 1 {
        return s[0];
    }
    let t = crate::math::clamp(t, 0.0, 1.0) * (n - 1) as f64;
    let i = (t as usize).min(n - 2);
    lerp(s[i], s[i + 1], t - i as f64)
}

impl Easing {
    /// The CSS `ease` keyword.
    pub fn css_ease() -> Self {
        Easing::bezier(0.25, 0.1, 0.25, 1.0)
    }
    pub fn bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        if x1 == y1 && x2 == y2 {
            Easing::Linear
        } else {
            Easing::Bezier(Box::new(CubicBezier::new(x1, y1, x2, y2)))
        }
    }
    pub fn spring(params: SpringParams, velocity: f64) -> Self {
        let settle = params.settle_time(velocity, 0.0005).max(1e-3);
        Easing::Spring { params, velocity, settle }
    }
    /// Build a CSS `linear()` easing from `(x, y)` stops.
    pub fn points(stops: &[(f64, f64)]) -> Self {
        let mut v = Vec::with_capacity(stops.len() * 2);
        for &(x, y) in stops {
            v.push(x);
            v.push(y);
        }
        Easing::Points(v.into_boxed_slice())
    }
    /// Build a lookup-table easing by sampling `f` at `n` uniform points.
    pub fn from_fn(n: usize, f: impl Fn(f64) -> f64) -> Self {
        let n = n.max(2);
        let v: Vec<f64> = (0..n).map(|i| f(i as f64 / (n - 1) as f64)).collect();
        Easing::Table(v.into_boxed_slice())
    }

    /// Evaluate the easing at progress `t`.
    #[inline]
    pub fn ease(&self, t: f64) -> f64 {
        match self {
            Easing::Linear => t,
            Easing::Curve(c, m) => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else {
                    apply_mode(*c, *m, t)
                }
            }
            Easing::Bezier(b) => b.ease(t),
            Easing::Steps(n, p) => steps(*n, *p, t),
            Easing::Spring { params, velocity, settle } => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else {
                    params.response(t * settle, *velocity * settle)
                }
            }
            Easing::Points(p) => points(p, t),
            Easing::Table(s) => table(s, t),
            Easing::Wiggle { count, noise, seed } => {
                if t <= 0.0 || t >= 1.0 {
                    return 0.0;
                }
                let env = 1.0 - t;
                if *noise {
                    noise1(t * count * 2.0, *seed) * env * 1.6
                } else {
                    sin(t * count * TAU) * env
                }
            }
            Easing::SlowMo { ratio, power } => {
                let t = crate::math::clamp(t, 0.0, 1.0);
                let p1 = (1.0 - ratio) / 2.0;
                let p3 = p1 + ratio;
                let r = t + (0.5 - t) * power;
                if t < p1 && p1 > 0.0 {
                    let q = 1.0 - t / p1;
                    r - q * q * q * q * r
                } else if t > p3 && p1 > 0.0 {
                    let q = (t - p3) / p1;
                    r + (t - r) * q * q * q * q
                } else {
                    r
                }
            }
            Easing::Anticipate(s) => {
                let t2 = t * 2.0;
                if t2 < 1.0 {
                    0.5 * curve_in(Curve::Back(*s), t2)
                } else if t >= 1.0 {
                    1.0
                } else {
                    0.5 * (2.0 - exp2(-10.0 * (t2 - 1.0)))
                }
            }
            Easing::Reverse(e) => 1.0 - e.ease(1.0 - t),
            Easing::Mirror(e) => {
                if t < 0.5 {
                    e.ease(2.0 * t)
                } else {
                    e.ease(2.0 - 2.0 * t)
                }
            }
            Easing::Blend(a, b, w) => a.ease(t) * (1.0 - w) + b.ease(t) * w,
        }
    }

    /// "CustomEase": an easing drawn as an SVG path from (0,0) to (1,1), with
    /// y up, e.g. `M0,0 C0.2,0 0.3,1.2 0.6,1 0.8,0.9 1,1`. Paths in other
    /// coordinate ranges are normalised by their start/end points.
    pub fn from_path(d: &str) -> Option<Easing> {
        let path = crate::path::Path::parse(d);
        let sp = path.subpaths.first()?;
        if sp.segs.is_empty() {
            return None;
        }
        // Dense samples along the curve.
        let mut pts: Vec<(f64, f64)> = Vec::with_capacity(sp.segs.len() * 64 + 1);
        let mut p0 = sp.start;
        pts.push((p0[0], p0[1]));
        for seg in &sp.segs {
            for k in 1..=64 {
                let q = crate::path::cubic_point(p0, seg, k as f64 / 64.0);
                pts.push((q[0], q[1]));
            }
            p0 = [seg[4], seg[5]];
        }
        let (x0, y0) = pts[0];
        let (x1, y1) = pts[pts.len() - 1];
        let (sx, sy) = (x1 - x0, y1 - y0);
        if sx.abs() < 1e-12 || sy.abs() < 1e-12 {
            return None;
        }
        // Normalise and force x to be non-decreasing (an easing is a function of x).
        let mut maxx = f64::NEG_INFINITY;
        for p in pts.iter_mut() {
            let x = ((p.0 - x0) / sx).max(maxx);
            maxx = x;
            *p = (x, (p.1 - y0) / sy);
        }
        // Resample uniformly in x → O(1) lookups.
        let n = 512;
        let mut table = Vec::with_capacity(n);
        let mut j = 0;
        for i in 0..n {
            let x = i as f64 / (n - 1) as f64;
            while j + 2 < pts.len() && pts[j + 1].0 < x {
                j += 1;
            }
            let (a, b) = (pts[j], pts[(j + 1).min(pts.len() - 1)]);
            let f = if b.0 > a.0 { ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 1.0 };
            table.push(a.1 + (b.1 - a.1) * f);
        }
        Some(Easing::Table(table.into_boxed_slice()))
    }

    /// Jittery "rough" easing around a template (seeded, deterministic).
    pub fn rough(points: usize, strength: f64, seed: u32, template: &Easing) -> Easing {
        let points = points.clamp(2, 400);
        let mut rng = crate::math::Rng(seed.max(1));
        let mut v = Vec::with_capacity((points + 2) * 2);
        v.push(0.0);
        v.push(0.0);
        for i in 1..=points {
            let x = i as f64 / (points + 1) as f64;
            let jitter = (rng.next_f64() * 2.0 - 1.0) * strength * 0.1;
            v.push(x);
            v.push(template.ease(x) + jitter);
        }
        v.push(1.0);
        v.push(1.0);
        Easing::Points(v.into_boxed_slice())
    }

    /// Ease every value of `ts` in place. The variant is matched once, outside
    /// the loop, so each arm compiles to a tight (auto-vectorisable) loop.
    pub fn ease_slice(&self, ts: &mut [f64]) {
        match self {
            Easing::Linear => {}
            Easing::Curve(Curve::Quad, Mode::Out) => {
                for t in ts.iter_mut() {
                    let c = crate::math::clamp(*t, 0.0, 1.0);
                    let u = 1.0 - c;
                    *t = 1.0 - u * u;
                }
            }
            Easing::Curve(Curve::Cubic, Mode::Out) => {
                for t in ts.iter_mut() {
                    let c = crate::math::clamp(*t, 0.0, 1.0);
                    let u = 1.0 - c;
                    *t = 1.0 - u * u * u;
                }
            }
            Easing::Curve(c, m) => {
                let (c, m) = (*c, *m);
                for t in ts.iter_mut() {
                    *t = if *t <= 0.0 {
                        0.0
                    } else if *t >= 1.0 {
                        1.0
                    } else {
                        apply_mode(c, m, *t)
                    };
                }
            }
            Easing::Bezier(b) => {
                for t in ts.iter_mut() {
                    *t = b.ease(*t);
                }
            }
            _ => {
                for t in ts.iter_mut() {
                    *t = self.ease(*t);
                }
            }
        }
    }

    /// Duration (seconds) a spring easing naturally takes, if this is one.
    pub fn natural_duration(&self) -> Option<f64> {
        match self {
            Easing::Spring { settle, .. } => Some(*settle),
            _ => None,
        }
    }

    /// Fill `out` with `out.len()` uniform samples over `[0, 1]`.
    pub fn sample_into(&self, out: &mut [f64]) {
        let n = out.len();
        if n == 1 {
            out[0] = self.ease(1.0);
            return;
        }
        let inv = 1.0 / (n - 1) as f64;
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.ease(i as f64 * inv);
        }
    }

    /// Parse any of the supported easing syntaxes.
    pub fn parse(src: &str) -> Option<Easing> {
        let (head, args) = split_call(src);
        let mut name: Vec<u8> = Vec::with_capacity(head.len());
        for &b in head.as_bytes() {
            match b {
                b'-' | b'_' | b'.' | b' ' => {}
                _ => name.push(b.to_ascii_lowercase()),
            }
        }
        let name = core::str::from_utf8(&name).ok()?;
        let nums = args.map(parse_numbers).unwrap_or_default();
        let arg = |i: usize, d: f64| nums.get(i).copied().unwrap_or(d);

        // Modifiers and forms whose arguments are themselves easings / paths.
        if let Some(a) = args {
            match name {
                "custom" | "path" | "customease" => return Easing::from_path(a),
                "reverse" | "invert" => return Some(Easing::Reverse(Box::new(Easing::parse(a)?))),
                "mirror" | "yoyo" => return Some(Easing::Mirror(Box::new(Easing::parse(a)?))),
                "blend" | "mix" => {
                    let parts = split_top_level(a);
                    if parts.len() < 2 {
                        return None;
                    }
                    let w = parts.get(2).and_then(|x| crate::util::parse_f64(x)).unwrap_or(0.5);
                    return Some(Easing::Blend(
                        Box::new(Easing::parse(parts[0])?),
                        Box::new(Easing::parse(parts[1])?),
                        w,
                    ));
                }
                "rough" => {
                    let parts = split_top_level(a);
                    let num = |i: usize, d: f64| parts.get(i).and_then(|x| crate::util::parse_f64(x)).unwrap_or(d);
                    let template = parts.get(3).and_then(|x| Easing::parse(x)).unwrap_or(Easing::Linear);
                    return Some(Easing::rough(num(0, 20.0) as usize, num(1, 1.0), num(2, 1.0) as u32, &template));
                }
                _ => {}
            }
        }

        // Spring presets (react-spring style).
        let preset = match name.strip_prefix("spring").unwrap_or(name) {
            "gentle" => Some((120.0, 14.0)),
            "wobbly" => Some((180.0, 12.0)),
            "stiff" => Some((210.0, 20.0)),
            "slow" => Some((280.0, 60.0)),
            "molasses" => Some((280.0, 120.0)),
            "snappy" => Some((400.0, 30.0)),
            "bouncy" => Some((300.0, 10.0)),
            _ => None,
        };
        if let Some((k, c)) = preset {
            return Some(Easing::spring(SpringParams::new(k, c, 1.0), 0.0));
        }

        // Exact keywords / function forms first.
        match name {
            "rough" => return Some(Easing::rough(20, 1.0, 1, &Easing::Linear)),
            "exposcale" => {
                let (a, b) = (arg(0, 1.0).max(1e-6), arg(1, 2.0).max(1e-6));
                if (a - b).abs() < 1e-12 {
                    return Some(Easing::Linear);
                }
                return Some(Easing::from_fn(512, |p| (pow(b / a, p) * a - a) / (b - a)));
            }
            "linear" | "none" | "power0" => {
                return match args {
                    Some(a) if !a.trim().is_empty() => parse_css_linear(a),
                    _ => Some(Easing::Linear),
                }
            }
            "ease" => return Some(Easing::css_ease()),
            "easein" => return Some(Easing::bezier(0.42, 0.0, 1.0, 1.0)),
            "easeout" => return Some(Easing::bezier(0.0, 0.0, 0.58, 1.0)),
            "easeinout" => return Some(Easing::bezier(0.42, 0.0, 0.58, 1.0)),
            "stepstart" => return Some(Easing::Steps(1, StepPosition::JumpStart)),
            "stepend" => return Some(Easing::Steps(1, StepPosition::JumpEnd)),
            "cubicbezier" | "bezier" => {
                if nums.len() != 4 {
                    return None;
                }
                return Some(Easing::bezier(nums[0], nums[1], nums[2], nums[3]));
            }
            "steps" => {
                let a = args.unwrap_or("");
                let mut c = Cursor::new(a);
                c.skip_ws();
                let n = c.number().unwrap_or(1.0).max(1.0) as u32;
                c.skip_sep();
                let pos = match c.ident() {
                    "jump-start" | "start" => StepPosition::JumpStart,
                    "jump-none" => StepPosition::JumpNone,
                    "jump-both" => StepPosition::JumpBoth,
                    _ => StepPosition::JumpEnd,
                };
                return Some(Easing::Steps(n, pos));
            }
            "spring" => return Some(parse_spring(args.unwrap_or(""))),
            "wiggle" => return Some(Easing::Wiggle { count: arg(0, 5.0), noise: false, seed: 0 }),
            "shake" => {
                return Some(Easing::Wiggle { count: arg(0, 8.0), noise: true, seed: arg(1, 1.0) as u32 })
            }
            "slowmo" => {
                return Some(Easing::SlowMo {
                    ratio: crate::math::clamp(arg(0, 0.7), 0.0, 1.0),
                    power: crate::math::clamp(arg(1, 0.7), 0.0, 1.0),
                })
            }
            "anticipate" => return Some(Easing::Anticipate(arg(0, 1.70158))),
            _ => {}
        }

        // Penner / GSAP / Motion names: strip "ease", split mode + curve.
        let n = name.strip_prefix("ease").unwrap_or(name);
        let (mode, curve_name) = split_mode(n)?;
        let curve = match curve_name {
            "quad" | "power1" => Curve::Quad,
            "cubic" | "power2" => Curve::Cubic,
            "quart" | "power3" => Curve::Quart,
            "quint" | "power4" | "strong" => Curve::Quint,
            "sine" => Curve::Sine,
            "expo" => Curve::Expo,
            "circ" => Curve::Circ,
            "back" => Curve::Back(arg(0, 1.70158)),
            "elastic" => Curve::Elastic(arg(0, 1.0), arg(1, 0.3)),
            "bounce" => Curve::Bounce,
            "pow" | "power" => Curve::Power(arg(0, 2.0)),
            _ => return None,
        };
        Some(Easing::Curve(curve, mode.unwrap_or(Mode::Out)))
    }
}

/// Split on commas that are not nested inside parentheses.
fn split_top_level(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

fn split_mode(n: &str) -> Option<(Option<Mode>, &str)> {
    const MODES: [(&str, Mode); 4] =
        [("inout", Mode::InOut), ("outin", Mode::OutIn), ("in", Mode::In), ("out", Mode::Out)];
    for (p, m) in MODES {
        if let Some(rest) = n.strip_prefix(p) {
            if !rest.is_empty() {
                return Some((Some(m), rest));
            }
        }
    }
    for (p, m) in MODES {
        if let Some(rest) = n.strip_suffix(p) {
            if !rest.is_empty() {
                return Some((Some(m), rest));
            }
        }
    }
    if n.is_empty() {
        None
    } else {
        Some((None, n))
    }
}

fn parse_spring(args: &str) -> Easing {
    let mut p = SpringParams::default();
    let mut velocity = 0.0;
    let mut duration = None;
    let mut bounce = None;
    let mut positional = 0;
    for part in args.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, val) = match part.find([':', '=']) {
            Some(i) => (part[..i].trim(), part[i + 1..].trim()),
            None => ("", part),
        };
        let v = match crate::util::parse_f64(val) {
            Some(v) => v,
            None => continue,
        };
        let key = if key.is_empty() {
            positional += 1;
            match positional {
                1 => "stiffness",
                2 => "damping",
                3 => "mass",
                _ => "velocity",
            }
        } else {
            key
        };
        match key {
            "stiffness" | "k" | "tension" => p.stiffness = v,
            "damping" | "c" | "friction" => p.damping = v,
            "mass" | "m" => p.mass = v,
            "velocity" | "v" => velocity = v,
            "duration" | "visualDuration" => duration = Some(v),
            "bounce" => bounce = Some(v),
            _ => {}
        }
    }
    if duration.is_some() || bounce.is_some() {
        p = SpringParams::from_duration_bounce(duration.unwrap_or(0.5), bounce.unwrap_or(0.25));
    }
    Easing::spring(SpringParams::new(p.stiffness, p.damping, p.mass), velocity)
}

/// Parse the argument list of CSS `linear()`, e.g. `0, 0.25 75%, 1`.
fn parse_css_linear(args: &str) -> Option<Easing> {
    // Each entry: output [input%] [input%]
    let mut ys: Vec<f64> = Vec::new();
    let mut xs: Vec<Option<f64>> = Vec::new();
    for entry in args.split(',') {
        let mut c = Cursor::new(entry);
        c.skip_ws();
        let y = c.number()?;
        let mut pcts = Vec::new();
        loop {
            c.skip_ws();
            match c.number() {
                Some(x) => {
                    c.eat(b'%');
                    pcts.push(x / 100.0);
                }
                None => break,
            }
        }
        if pcts.is_empty() {
            ys.push(y);
            xs.push(None);
        } else {
            for x in pcts.into_iter().take(2) {
                ys.push(y);
                xs.push(Some(x));
            }
        }
    }
    let n = ys.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(Easing::Points(Box::new([0.0, ys[0], 1.0, ys[0]])));
    }
    if xs[0].is_none() {
        xs[0] = Some(0.0);
    }
    if xs[n - 1].is_none() {
        xs[n - 1] = Some(xs.iter().flatten().fold(1.0f64, |a, &b| a.max(b)));
    }
    // Clamp to non-decreasing.
    let mut maxx = f64::NEG_INFINITY;
    for x in xs.iter_mut().flatten() {
        if *x < maxx {
            *x = maxx;
        }
        maxx = *x;
    }
    // Fill runs of missing x evenly between known neighbours.
    let mut i = 0;
    while i < n {
        if xs[i].is_none() {
            let start = i - 1;
            let mut end = i;
            while xs[end].is_none() {
                end += 1;
            }
            let (a, b) = (xs[start].unwrap_or(0.0), xs[end].unwrap_or(1.0));
            for (k, slot) in xs.iter_mut().enumerate().take(end).skip(i) {
                *slot = Some(a + (b - a) * (k - start) as f64 / (end - start) as f64);
            }
            i = end;
        }
        i += 1;
    }
    let mut v = Vec::with_capacity(n * 2);
    for k in 0..n {
        v.push(xs[k].unwrap_or(0.0));
        v.push(ys[k]);
    }
    Some(Easing::Points(v.into_boxed_slice()))
}

/// Max absolute difference between two easings over `[0, 1]`, sampled.
pub fn max_error(a: &Easing, b: &Easing, samples: usize) -> f64 {
    let mut m: f64 = 0.0;
    for i in 0..=samples {
        let t = i as f64 / samples as f64;
        m = m.max(abs(a.ease(t) - b.ease(t)));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Easing {
        Easing::parse(s).unwrap_or_else(|| panic!("parse {s}"))
    }

    #[test]
    fn ease_slice_matches_ease_bit_for_bit() {
        let curves = [Curve::Quad, Curve::Cubic, Curve::Quart, Curve::Quint, Curve::Sine, Curve::Expo, Curve::Circ, Curve::Back(1.7), Curve::Elastic(1.0, 0.3), Curve::Bounce, Curve::Power(2.5)];
        let modes = [Mode::In, Mode::Out, Mode::InOut, Mode::OutIn];
        let xs: Vec<f64> = (-20..=1020).map(|i| i as f64 / 1000.0).chain([f64::NAN, -0.0, 0.5, 1.0]).collect();
        for c in curves {
            for m in modes {
                let e = Easing::Curve(c, m);
                let mut v = xs.clone();
                e.ease_slice(&mut v);
                for (&x, &y) in xs.iter().zip(&v) {
                    // Quad/Cubic Out use a clamp formulation: equal values, but
                    // NaN maps to 1 there; compare non-NaN inputs only for those.
                    if x.is_nan() && matches!((c, m), (Curve::Quad | Curve::Cubic, Mode::Out)) {
                        continue;
                    }
                    assert_eq!(y.to_bits(), e.ease(x).to_bits(), "{c:?} {m:?} {x}");
                }
            }
        }
    }

    #[test]
    fn endpoints() {
        for name in [
            "linear", "ease", "ease-in", "ease-out", "ease-in-out", "easeInQuad", "easeOutCubic", "inOutQuart",
            "quint.in", "sine.out", "expo.inOut", "circ.outIn", "back.out(1.7)", "elastic.out(1, 0.3)", "bounce.out",
            "power3.inOut", "spring", "spring(300, 10)", "spring(bounce: 0.4, duration: 0.5)", "slowmo", "anticipate",
            "cubic-bezier(0.68,-0.55,0.27,1.55)", "pow(3).in",
        ] {
            let e = p(name);
            assert!(e.ease(0.0).abs() < 1e-9, "{name} at 0 = {}", e.ease(0.0));
            assert!((e.ease(1.0) - 1.0).abs() < 1e-9, "{name} at 1 = {}", e.ease(1.0));
        }
    }

    #[test]
    fn modes() {
        let i = p("inQuad");
        let o = p("outQuad");
        let io = p("inOutQuad");
        assert!((i.ease(0.5) - 0.25).abs() < 1e-12);
        assert!((o.ease(0.5) - 0.75).abs() < 1e-12);
        assert!((io.ease(0.25) - 0.125).abs() < 1e-12);
        assert!((io.ease(0.5) - 0.5).abs() < 1e-12);
        assert_eq!(p("power2.inOut"), p("easeInOutCubic"));
        assert_eq!(p("circIn"), p("easeInCirc"));
        assert_eq!(p("back"), Easing::Curve(Curve::Back(1.70158), Mode::Out));
    }

    #[test]
    fn steps_css() {
        let e = p("steps(4)");
        assert_eq!(e.ease(0.0), 0.0);
        assert_eq!(e.ease(0.3), 0.25);
        assert_eq!(e.ease(1.0), 1.0);
        let s = p("steps(4, jump-start)");
        assert_eq!(s.ease(0.0), 0.25);
        let b = p("steps(3, jump-both)");
        assert_eq!(b.ease(0.0), 0.25);
        let nn = p("steps(5, jump-none)");
        assert_eq!(nn.ease(0.0), 0.0);
        assert_eq!(nn.ease(0.99), 1.0);
        assert_eq!(p("step-end").ease(0.99), 0.0);
        assert_eq!(p("step-start").ease(0.01), 1.0);
    }

    #[test]
    fn css_linear() {
        let e = p("linear(0, 0.25 75%, 1)");
        assert!((e.ease(0.75) - 0.25).abs() < 1e-12);
        assert!((e.ease(0.375) - 0.125).abs() < 1e-12);
        assert!((e.ease(0.875) - 0.625).abs() < 1e-12);
        let e = p("linear(0, 0.5, 1)");
        assert!((e.ease(0.25) - 0.25).abs() < 1e-12);
        let e = p("linear(0, 0.5 25% 75%, 1)");
        assert!((e.ease(0.5) - 0.5).abs() < 1e-12);
        assert!((e.ease(0.125) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn overshoot_and_wiggle() {
        let b = p("back.out(2)");
        let max = (0..100).map(|i| b.ease(i as f64 / 100.0)).fold(0.0, f64::max);
        assert!(max > 1.05);
        let w = p("wiggle(4)");
        assert_eq!(w.ease(1.0), 0.0);
        assert!(w.ease(1.0 / 16.0) > 0.5);
    }

    #[test]
    fn spring_easing_is_close_to_physical_response() {
        let e = p("spring(170, 26)");
        if let Easing::Spring { params, settle, .. } = &e {
            assert!(*settle > 0.2 && *settle < 2.0);
            assert!((e.ease(0.5) - params.response(settle * 0.5, 0.0)).abs() < 1e-12);
        } else {
            panic!()
        }
    }

    #[test]
    fn extended_easings() {
        // CustomEase from a path: a straight diagonal is linear.
        let e = p("custom(M0,0 L1,1)");
        assert!((e.ease(0.3) - 0.3).abs() < 1e-3);
        let c = p("custom(M0,0 C0.5,0 0.5,1 1,1)");
        assert!(c.ease(0.25) < 0.25 && c.ease(0.75) > 0.75);
        assert!((c.ease(1.0) - 1.0).abs() < 1e-9);
        // SVG-space path (y down, arbitrary range) is normalised.
        let s = p("custom(M0,100 L200,0)");
        assert!((s.ease(0.5) - 0.5).abs() < 1e-3);
        // Modifiers.
        let r = p("reverse(inQuad)");
        assert!((r.ease(0.5) - 0.75).abs() < 1e-12);
        let m = p("mirror(linear)");
        assert!((m.ease(0.5) - 1.0).abs() < 1e-12 && m.ease(1.0).abs() < 1e-12);
        let b = p("blend(inQuad, outQuad, 0.5)");
        assert!((b.ease(0.5) - 0.5).abs() < 1e-12);
        let nested = p("mirror(back.out(2))");
        assert!(nested.ease(0.25) > 0.5);
        // Presets and others.
        for n in ["spring.gentle", "wobbly", "spring.stiff", "molasses", "rough", "rough(30, 2, 7, outQuad)", "expoScale(1, 4)"] {
            let e = p(n);
            assert!(e.ease(0.0).abs() < 1e-6, "{n}");
            assert!((e.ease(1.0) - 1.0).abs() < 1e-6, "{n}");
        }
        let x = p("expoScale(1, 10)");
        assert!(x.ease(0.5) < 0.5);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Easing::parse("nonsense").is_none());
        assert!(Easing::parse("cubic-bezier(1,2)").is_none());
    }
}
