//! A minimal Lottie (Bodymovin JSON) player.
//!
//! The JSON is loaded and parsed once in [`Prepare`] into compact structs
//! (no `serde_json::Value` survives into `eval`); each frame evaluates the
//! keyframed properties and emits display-list ops in box coordinates,
//! scaled into the component box per `fit`.
//!
//! Supported subset:
//! - composition `w`, `h`, `fr`, `ip`, `op`; precomp `assets` (`layers`);
//! - layers: shape (4), null (3), precomp (0, with `tm` time remap and its
//!   `w`×`h` clip) and solid (1); `ind`/`parent` parenting, `ip`/`op`,
//!   `st`, `sr`, `hd`, `bm` (separable blend modes), track-matte sources
//!   (`td`) are hidden;
//! - transforms `ks`/`tr`: anchor `a`, position `p` (joined or split, with
//!   spatial bezier `to`/`ti`), scale `s`, rotation `r`/`rz`, opacity `o`;
//! - shape items: group `gr`, rect `rc`, ellipse `el`, polystar `sr`
//!   (without roundness), path `sh`, fill `fl`, stroke `st` (width, caps,
//!   joins, miter limit, dashes), gradient fill/stroke `gf`/`gs` (linear and
//!   radial, with opacity stops), trim paths `tm` (mapped to
//!   [`dl::Stroke::trim`]; fills are not trimmed);
//! - keyframes: `t`, `s`, `e` or the next `s`, per-dimension cubic bezier
//!   easing `o`/`i`, hold `h`.
//!
//! Anything else (masks, mattes, effects, text, images, repeaters, …) is
//! skipped and counted; see [`Lottie::unsupported`].

use super::style::{self, Color};
use super::{with_opacity, Component, Ctx, Error, Prepare};
use crate::dl::{self, Affine, Blend, FillRule, Op, Paint, Path, Rgba, Stroke, IDENTITY};
use kurbo::{PathEl, Point, Shape};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/// How the composition is fitted into the component box.
#[derive(Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Scale to fit entirely inside the box, centred (letterboxed).
    #[default]
    Contain,
    /// Scale to cover the box, centred (cropped).
    Cover,
    /// Natural size, centred.
    #[serde(rename = "none")]
    Natural,
}

/// The `lottie` component.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Lottie {
    /// Path to a Lottie `.json` file.
    pub src: Option<String>,
    /// Inline Lottie JSON (an object, or a string holding JSON).
    pub data: Option<Value>,
    /// Playback rate (negative plays backwards).
    pub speed: f32,
    /// Loop forever (default) or hold the last frame.
    #[serde(rename = "loop")]
    pub looping: bool,
    pub fit: Fit,
    /// Seconds before playback starts (the first frame is shown meanwhile).
    pub delay: f32,
    /// Recolour every fill and stroke (alpha is kept), for monochrome icons.
    pub color: Option<Color>,
    #[serde(skip)]
    anim: Option<Anim>,
    #[serde(skip)]
    unsupported: Vec<String>,
}

impl Default for Lottie {
    fn default() -> Self {
        Lottie { src: None, data: None, speed: 1.0, looping: true, fit: Fit::Contain, delay: 0.0, color: None, anim: None, unsupported: Vec::new() }
    }
}

impl Lottie {
    /// Features found in the file that the player skipped, with counts
    /// (for example `"masks (×2)"`). A `src` that could not be read is
    /// reported here too, and the component then draws nothing.
    pub fn unsupported(&self) -> &[String] {
        &self.unsupported
    }

    /// Composition frame shown at local time `t`.
    fn frame(&self, a: &Anim, t: f32) -> f32 {
        let len = (a.op - a.ip) / a.fr;
        let mut x = (t - self.delay).max(0.0) * self.speed.abs();
        x = if self.looping { x.rem_euclid(len) } else { x.min(len) };
        if self.speed < 0.0 {
            x = len - x;
        }
        (a.ip + x * a.fr).min(a.op - 1e-3)
    }
}

impl Prepare for Lottie {
    fn prepare(&mut self) -> Result<(), Error> {
        if !self.speed.is_finite() || !self.delay.is_finite() {
            return Err(Error("`speed` and `delay` must be finite numbers".into()));
        }
        let value = match (&self.src, self.data.take()) {
            (Some(_), Some(_)) => return Err(Error("give either `src` or `data`, not both".into())),
            (None, Some(Value::String(s))) => serde_json::from_str(&s).map_err(|e| Error(format!("`data` is not valid JSON: {e}")))?,
            (None, Some(v)) => v,
            (Some(path), None) => match std::fs::read_to_string(path) {
                Ok(s) => serde_json::from_str(&s).map_err(|e| Error(format!("`{path}` is not valid JSON: {e}")))?,
                Err(e) => {
                    self.unsupported = vec![format!("could not read `{path}`: {e}")];
                    return Ok(());
                }
            },
            (None, None) => return Err(Error("needs `src` (path to a Lottie .json) or `data` (inline Lottie JSON)".into())),
        };
        let mut d = Diag::default();
        self.anim = Some(parse(&value, &mut d)?);
        self.unsupported = d.into_list();
        Ok(())
    }
}

impl Component for Lottie {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let Some(a) = &self.anim else { return Vec::new() };
        let frame = self.frame(a, t);
        let s = match self.fit {
            Fit::Contain => (ctx.width / a.w).min(ctx.height / a.h),
            Fit::Cover => (ctx.width / a.w).max(ctx.height / a.h),
            Fit::Natural => 1.0,
        };
        let (tx, ty) = ((ctx.width - a.w * s) * 0.5, (ctx.height - a.h * s) * 0.5);
        let r = Render { a, tint: self.color.map(|c| c.0) };
        let mut ops = Vec::new();
        r.comp(0, frame, &[s, 0.0, 0.0, s, tx, ty], &mut ops, 0);
        let (x0, y0) = (tx.max(0.0), ty.max(0.0));
        let (x1, y1) = ((tx + a.w * s).min(ctx.width), (ty + a.h * s).min(ctx.height));
        if ops.is_empty() || x1 <= x0 || y1 <= y0 {
            return Vec::new();
        }
        let clip = kurbo::Rect::new(x0 as f64, y0 as f64, x1 as f64, y1 as f64).to_path(0.1);
        ops.insert(0, Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: Some((clip, IDENTITY)), cache_key: None });
        ops.push(Op::PopLayer);
        ops
    }

    fn duration(&self) -> Option<f32> {
        let a = self.anim.as_ref()?;
        if self.looping || self.speed == 0.0 {
            return None;
        }
        Some(self.delay + (a.op - a.ip) / a.fr / self.speed.abs())
    }
}

// ---------------------------------------------------------------- diagnostics

#[derive(Default)]
struct Diag(BTreeMap<String, usize>);

impl Diag {
    fn skip(&mut self, what: impl Into<String>) {
        *self.0.entry(what.into()).or_default() += 1;
    }
    fn into_list(self) -> Vec<String> {
        self.0.into_iter().map(|(k, n)| if n == 1 { k } else { format!("{k} (×{n})") }).collect()
    }
}

// ---------------------------------------------------------------- properties

/// One keyframe. `e` is the value at the next keyframe (filled in at parse).
#[derive(Clone, Debug, Default)]
struct Key {
    t: f32,
    s: Vec<f32>,
    e: Vec<f32>,
    /// Per-dimension out/in bezier handles `(x, y)`.
    o: Vec<[f32; 2]>,
    i: Vec<[f32; 2]>,
    hold: bool,
    /// Spatial tangents (`to`, `ti`) for 2D position paths.
    spatial: Option<([f32; 2], [f32; 2])>,
}

/// A possibly animated property, flattened to `f32`s (a shape is
/// `[vertices…, in tangents…, out tangents…]`).
#[derive(Clone, Debug, Default)]
struct Prop {
    value: Vec<f32>,
    keys: Vec<Key>,
}

impl Prop {
    fn constant(v: &[f32]) -> Prop {
        Prop { value: v.to_vec(), keys: Vec::new() }
    }

    fn get(&self, f: f32) -> Vec<f32> {
        let k = &self.keys;
        if k.is_empty() {
            return self.value.clone();
        }
        if f <= k[0].t || k.len() == 1 {
            return k[0].s.clone();
        }
        let idx = k.partition_point(|x| x.t <= f) - 1;
        if idx + 1 >= k.len() {
            return k[idx].s.clone();
        }
        let (a, b) = (&k[idx], &k[idx + 1]);
        if a.hold || b.t <= a.t {
            return a.s.clone();
        }
        let u = (f - a.t) / (b.t - a.t);
        let mut out: Vec<f32> = a.s.iter().zip(&a.e).enumerate().map(|(d, (s, e))| s + (e - s) * bezier(ease(&a.o, d, [0.0, 0.0]), ease(&a.i, d, [1.0, 1.0]), u)).collect();
        if let (Some((to, ti)), true) = (a.spatial, a.s.len() >= 2) {
            let w = bezier(ease(&a.o, 0, [0.0, 0.0]), ease(&a.i, 0, [1.0, 1.0]), u).clamp(0.0, 1.0);
            let p0 = [a.s[0], a.s[1]];
            let p3 = [a.e[0], a.e[1]];
            let p = along_cubic([p0, [p0[0] + to[0], p0[1] + to[1]], [p3[0] + ti[0], p3[1] + ti[1]], p3], w);
            out[0] = p[0];
            out[1] = p[1];
        }
        out
    }

    fn scalar(&self, f: f32) -> f32 {
        if self.keys.is_empty() {
            return self.value.first().copied().unwrap_or(0.0);
        }
        self.get(f).first().copied().unwrap_or(0.0)
    }

    fn vec2(&self, f: f32) -> [f32; 2] {
        let v = if self.keys.is_empty() { std::borrow::Cow::Borrowed(&self.value) } else { std::borrow::Cow::Owned(self.get(f)) };
        let x = v.first().copied().unwrap_or(0.0);
        [x, v.get(1).copied().unwrap_or(x)]
    }

    fn is_zero(&self) -> bool {
        self.keys.is_empty() && self.value.iter().all(|v| *v == 0.0)
    }
}

fn ease(v: &[[f32; 2]], d: usize, default: [f32; 2]) -> [f32; 2] {
    v.get(d).or(v.last()).copied().unwrap_or(default)
}

/// CSS-style cubic bezier easing through `(0,0)`, `o`, `i`, `(1,1)`, at `x = u`.
fn bezier(o: [f32; 2], i: [f32; 2], u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    let (x1, y1, x2, y2) = (o[0].clamp(0.0, 1.0), o[1], i[0].clamp(0.0, 1.0), i[1]);
    if (x1 - y1).abs() < 1e-6 && (x2 - y2).abs() < 1e-6 {
        return u;
    }
    let cb = |p1: f32, p2: f32, t: f32| {
        let mt = 1.0 - t;
        3.0 * mt * mt * t * p1 + 3.0 * mt * t * t * p2 + t * t * t
    };
    let mut t = u;
    for _ in 0..8 {
        let x = cb(x1, x2, t) - u;
        let dx = 3.0 * (1.0 - t) * (1.0 - t) * x1 + 6.0 * (1.0 - t) * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
        if x.abs() < 1e-6 || dx.abs() < 1e-6 {
            break;
        }
        t -= x / dx;
    }
    if !(0.0..=1.0).contains(&t) || (cb(x1, x2, t) - u).abs() > 1e-4 {
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..40 {
            t = 0.5 * (lo + hi);
            if cb(x1, x2, t) < u {
                lo = t;
            } else {
                hi = t;
            }
        }
    }
    cb(y1, y2, t)
}

/// Point at fraction `w` of the arc length of a cubic.
fn along_cubic(p: [[f32; 2]; 4], w: f32) -> [f32; 2] {
    const N: usize = 24;
    let at = |t: f32| {
        let mt = 1.0 - t;
        let (a, b, c, d) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
        [a * p[0][0] + b * p[1][0] + c * p[2][0] + d * p[3][0], a * p[0][1] + b * p[1][1] + c * p[2][1] + d * p[3][1]]
    };
    let mut pts = [[0.0f32; 2]; N + 1];
    let mut len = [0.0f32; N + 1];
    for k in 0..=N {
        pts[k] = at(k as f32 / N as f32);
        if k > 0 {
            len[k] = len[k - 1] + ((pts[k][0] - pts[k - 1][0]).powi(2) + (pts[k][1] - pts[k - 1][1]).powi(2)).sqrt();
        }
    }
    if len[N] <= 1e-6 {
        return at(w);
    }
    let target = w * len[N];
    let k = len.partition_point(|l| *l < target).clamp(1, N);
    let seg = (len[k] - len[k - 1]).max(1e-9);
    let f = ((target - len[k - 1]) / seg).clamp(0.0, 1.0);
    [pts[k - 1][0] + (pts[k][0] - pts[k - 1][0]) * f, pts[k - 1][1] + (pts[k][1] - pts[k - 1][1]) * f]
}

// ---------------------------------------------------------------- parsed model

struct Anim {
    w: f32,
    h: f32,
    fr: f32,
    ip: f32,
    op: f32,
    /// `comps[0]` is the root composition, the rest are precomp assets.
    comps: Vec<Vec<Layer>>,
}

struct Layer {
    parent: Option<usize>,
    kind: LayerKind,
    ks: Xform,
    ip: f32,
    op: f32,
    st: f32,
    sr: f32,
    blend: Blend,
    hidden: bool,
}

enum LayerKind {
    Null,
    Shape(Vec<Item>),
    Solid { color: Rgba, w: f32, h: f32 },
    Precomp { comp: usize, w: f32, h: f32, tm: Option<Prop> },
}

enum Pos {
    Joined(Prop),
    Split(Prop, Prop),
}

struct Xform {
    a: Prop,
    p: Pos,
    s: Prop,
    r: Prop,
    o: Prop,
}

impl Xform {
    /// `T(p) · R(r) · S(s) · T(-a)`.
    fn matrix(&self, f: f32) -> Affine {
        let a = self.a.vec2(f);
        let p = match &self.p {
            Pos::Joined(p) => p.vec2(f),
            Pos::Split(x, y) => [x.scalar(f), y.scalar(f)],
        };
        let s = self.s.vec2(f);
        let (sx, sy) = (s[0] / 100.0, s[1] / 100.0);
        let (sn, cs) = self.r.scalar(f).to_radians().sin_cos();
        let (m0, m1, m2, m3) = (cs * sx, sn * sx, -sn * sy, cs * sy);
        [m0, m1, m2, m3, p[0] - (m0 * a[0] + m2 * a[1]), p[1] - (m1 * a[0] + m3 * a[1])]
    }

    fn opacity(&self, f: f32) -> f32 {
        (self.o.scalar(f) / 100.0).clamp(0.0, 1.0)
    }
}

struct Grad {
    radial: bool,
    s: Prop,
    e: Prop,
    /// Number of colour stops; `k` holds `n × (offset, r, g, b)` then `(offset, alpha)` pairs.
    n: usize,
    k: Prop,
}

enum Brush {
    Solid(Prop),
    Grad(Grad),
}

struct Trim {
    s: Prop,
    e: Prop,
    o: Prop,
    /// 1 simultaneously (per path), 2 individually (across all paths).
    mode: u8,
}

enum Item {
    Group { items: Vec<Item>, tr: Option<Xform> },
    Rect { p: Prop, s: Prop, r: Prop, rev: bool },
    Ellipse { p: Prop, s: Prop, rev: bool },
    Star { polygon: bool, p: Prop, pt: Prop, r: Prop, or: Prop, ir: Prop, rev: bool },
    Path { ks: Prop, closed: bool, rev: bool },
    Fill { brush: Brush, o: Prop, rule: FillRule },
    Stroke { brush: Brush, o: Prop, w: Prop, cap: u8, join: u8, ml: f32, dash: Vec<(u8, Prop)> },
    Trim(Trim),
}

// ---------------------------------------------------------------- parsing

fn num(v: &Value, key: &str) -> Option<f32> {
    v.get(key).and_then(Value::as_f64).map(|x| x as f32)
}

fn int(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)))
}

fn flag(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() == Some(1.0),
        _ => false,
    }
}

/// Flatten a static value: a number, a number array, or a shape `{i, o, v, c}`.
fn flat(v: &Value) -> Option<Vec<f32>> {
    match v {
        Value::Number(n) => Some(vec![n.as_f64()? as f32]),
        Value::Array(a) => match a.first() {
            Some(Value::Object(_)) => flat(&a[0]),
            _ => a.iter().map(|x| x.as_f64().map(|f| f as f32)).collect(),
        },
        Value::Object(o) if o.contains_key("v") => {
            let pts = |k: &str| -> Vec<[f32; 2]> {
                o.get(k).and_then(Value::as_array).map(|a| a.iter().map(|p| [p.get(0).and_then(Value::as_f64).unwrap_or(0.0) as f32, p.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32]).collect()).unwrap_or_default()
            };
            let v = pts("v");
            let n = v.len();
            let mut out = Vec::with_capacity(n * 6);
            for list in [v, pts("i"), pts("o")] {
                for j in 0..n {
                    out.extend_from_slice(&list.get(j).copied().unwrap_or([0.0, 0.0]));
                }
            }
            Some(out)
        }
        _ => None,
    }
}

/// The `c` (closed) flag of a static or keyframed shape value.
fn shape_closed(k: &Value) -> bool {
    match k {
        Value::Object(o) => match o.get("c") {
            Some(c) => c.as_bool().unwrap_or(false),
            None => o.get("s").is_some_and(shape_closed),
        },
        Value::Array(a) => a.first().is_some_and(shape_closed),
        _ => false,
    }
}

fn easing(v: Option<&Value>) -> Vec<[f32; 2]> {
    let Some(v) = v else { return Vec::new() };
    let list = |k: &str| -> Vec<f32> {
        match v.get(k) {
            Some(Value::Number(n)) => vec![n.as_f64().unwrap_or(0.0) as f32],
            Some(Value::Array(a)) => a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect(),
            _ => Vec::new(),
        }
    };
    let (x, y) = (list("x"), list("y"));
    let n = x.len().max(y.len());
    (0..n).map(|j| [x.get(j).or(x.last()).copied().unwrap_or(0.0), y.get(j).or(y.last()).copied().unwrap_or(0.0)]).collect()
}

fn prop(v: Option<&Value>, default: &[f32], d: &mut Diag) -> Prop {
    let Some(v) = v else { return Prop::constant(default) };
    if v.get("x").is_some_and(Value::is_string) {
        d.skip("expressions");
    }
    let k = v.get("k").unwrap_or(v);
    let keyframed = matches!(k, Value::Array(a) if a.first().is_some_and(|f| f.get("t").is_some()));
    if !keyframed {
        return flat(k).filter(|v| !v.is_empty()).map(|value| Prop { value, keys: Vec::new() }).unwrap_or_else(|| Prop::constant(default));
    }
    let mut keys: Vec<Key> = Vec::new();
    let mut ends: Vec<Option<Vec<f32>>> = Vec::new();
    for kf in k.as_array().into_iter().flatten() {
        let Some(t) = num(kf, "t") else { continue };
        let Some(s) = kf.get("s").and_then(flat).or_else(|| ends.last().cloned().flatten()) else { continue };
        let spatial = match (kf.get("to").and_then(flat), kf.get("ti").and_then(flat)) {
            (Some(to), Some(ti)) if to.len() >= 2 && ti.len() >= 2 && to.iter().chain(&ti).any(|x| *x != 0.0) => Some(([to[0], to[1]], [ti[0], ti[1]])),
            _ => None,
        };
        ends.push(kf.get("e").and_then(flat));
        keys.push(Key { t, s, e: Vec::new(), o: easing(kf.get("o")), i: easing(kf.get("i")), hold: flag(kf, "h"), spatial });
    }
    for j in 0..keys.len() {
        let e = ends[j].take().or_else(|| keys.get(j + 1).map(|k| k.s.clone())).unwrap_or_else(|| keys[j].s.clone());
        if e.len() != keys[j].s.len() {
            keys[j].hold = true;
        }
        keys[j].e = e;
    }
    if keys.is_empty() {
        return Prop::constant(default);
    }
    Prop { value: Vec::new(), keys }
}

fn xform(v: &Value, d: &mut Diag) -> Xform {
    let p = match v.get("p") {
        Some(p) if flag(p, "s") => Pos::Split(prop(p.get("x"), &[0.0], d), prop(p.get("y"), &[0.0], d)),
        p => Pos::Joined(prop(p, &[0.0, 0.0], d)),
    };
    if let Some(sk) = v.get("sk") {
        if !prop(Some(sk), &[0.0], d).is_zero() {
            d.skip("skew");
        }
    }
    for k in ["rx", "ry"] {
        if let Some(r) = v.get(k) {
            if !prop(Some(r), &[0.0], d).is_zero() {
                d.skip("3D rotation");
            }
        }
    }
    Xform {
        a: prop(v.get("a"), &[0.0, 0.0], d),
        p,
        s: prop(v.get("s"), &[100.0, 100.0], d),
        r: prop(v.get("r").or_else(|| v.get("rz")), &[0.0], d),
        o: prop(v.get("o"), &[100.0], d),
    }
}

fn grad(it: &Value, d: &mut Diag) -> Grad {
    let g = it.get("g");
    Grad {
        radial: int(it, "t") == Some(2),
        s: prop(it.get("s"), &[0.0, 0.0], d),
        e: prop(it.get("e"), &[0.0, 0.0], d),
        n: g.and_then(|g| int(g, "p")).unwrap_or(0).max(0) as usize,
        k: prop(g.and_then(|g| g.get("k")), &[], d),
    }
}

fn items(arr: &[Value], d: &mut Diag, trimmed: bool) -> (Vec<Item>, Option<Xform>) {
    let mut out = Vec::new();
    let mut tr = None;
    let has_trim = trimmed || arr.iter().any(|it| it.get("ty").and_then(Value::as_str) == Some("tm"));
    for it in arr {
        if flag(it, "hd") {
            continue;
        }
        let rev = int(it, "d") == Some(3);
        let ty = it.get("ty").and_then(Value::as_str).unwrap_or("");
        match ty {
            "gr" => {
                let sub = it.get("it").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                if has_trim && sub.iter().any(|s| s.get("ty").and_then(Value::as_str) == Some("tm")) {
                    d.skip("nested trim paths (inner one wins)");
                }
                let (items, tr) = items(sub, d, has_trim);
                out.push(Item::Group { items, tr });
            }
            "tr" => tr = Some(xform(it, d)),
            "rc" => out.push(Item::Rect { p: prop(it.get("p"), &[0.0, 0.0], d), s: prop(it.get("s"), &[0.0, 0.0], d), r: prop(it.get("r"), &[0.0], d), rev }),
            "el" => out.push(Item::Ellipse { p: prop(it.get("p"), &[0.0, 0.0], d), s: prop(it.get("s"), &[0.0, 0.0], d), rev }),
            "sr" => {
                for k in ["is", "os"] {
                    if it.get(k).is_some_and(|v| !prop(Some(v), &[0.0], d).is_zero()) {
                        d.skip("polystar roundness");
                    }
                }
                out.push(Item::Star {
                    polygon: int(it, "sy") == Some(2),
                    p: prop(it.get("p"), &[0.0, 0.0], d),
                    pt: prop(it.get("pt"), &[5.0], d),
                    r: prop(it.get("r"), &[0.0], d),
                    or: prop(it.get("or"), &[0.0], d),
                    ir: prop(it.get("ir"), &[0.0], d),
                    rev,
                });
            }
            "sh" => {
                let ks = it.get("ks");
                let closed = ks.and_then(|k| k.get("k")).is_some_and(shape_closed);
                out.push(Item::Path { ks: prop(ks, &[], d), closed, rev });
            }
            "fl" | "gf" => {
                let brush = if ty == "fl" { Brush::Solid(prop(it.get("c"), &[0.0, 0.0, 0.0], d)) } else { Brush::Grad(grad(it, d)) };
                let rule = if int(it, "r") == Some(2) { FillRule::EvenOdd } else { FillRule::NonZero };
                out.push(Item::Fill { brush, o: prop(it.get("o"), &[100.0], d), rule });
            }
            "st" | "gs" => {
                let brush = if ty == "st" { Brush::Solid(prop(it.get("c"), &[0.0, 0.0, 0.0], d)) } else { Brush::Grad(grad(it, d)) };
                let dash = it
                    .get("d")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| {
                        let kind = match e.get("n").and_then(Value::as_str)? {
                            "d" => 0,
                            "g" => 1,
                            "o" => 2,
                            _ => return None,
                        };
                        Some((kind, prop(e.get("v"), &[0.0], d)))
                    })
                    .collect();
                out.push(Item::Stroke {
                    brush,
                    o: prop(it.get("o"), &[100.0], d),
                    w: prop(it.get("w"), &[1.0], d),
                    cap: (int(it, "lc").unwrap_or(2).clamp(1, 3) - 1) as u8,
                    join: (int(it, "lj").unwrap_or(2).clamp(1, 3) - 1) as u8,
                    ml: num(it, "ml").unwrap_or(4.0),
                    dash,
                });
            }
            "tm" => out.push(Item::Trim(Trim { s: prop(it.get("s"), &[0.0], d), e: prop(it.get("e"), &[100.0], d), o: prop(it.get("o"), &[0.0], d), mode: if int(it, "m") == Some(2) { 2 } else { 1 } })),
            other => d.skip(match other {
                "rd" => "round corners".to_string(),
                "rp" => "repeater".to_string(),
                "mm" => "merge paths".to_string(),
                "tw" => "twist".to_string(),
                "op" => "offset path".to_string(),
                "zz" => "zig zag".to_string(),
                "pb" => "pucker/bloat".to_string(),
                "" => "shape item without type".to_string(),
                o => format!("shape item `{o}`"),
            }),
        }
    }
    (out, tr)
}

fn layers(arr: &[Value], ids: &HashMap<String, usize>, d: &mut Diag) -> Vec<Layer> {
    let mut out = Vec::with_capacity(arr.len());
    let mut by_ind = HashMap::new();
    let mut parents = Vec::with_capacity(arr.len());
    for (j, l) in arr.iter().enumerate() {
        if let Some(ind) = int(l, "ind") {
            by_ind.insert(ind, j);
        }
        parents.push(int(l, "parent"));
        if flag(l, "hasMask") || l.get("masksProperties").and_then(Value::as_array).is_some_and(|m| !m.is_empty()) {
            d.skip("masks");
        }
        if int(l, "tt").is_some_and(|t| t != 0) {
            d.skip("track mattes");
        }
        if l.get("ef").and_then(Value::as_array).is_some_and(|e| !e.is_empty()) {
            d.skip("effects");
        }
        if flag(l, "ao") {
            d.skip("auto-orient");
        }
        if flag(l, "ddd") {
            d.skip("3D layers (z ignored)");
        }
        let blend = match int(l, "bm").unwrap_or(0) {
            0 => Blend::Normal,
            1 => Blend::Multiply,
            2 => Blend::Screen,
            3 => Blend::Overlay,
            4 => Blend::Darken,
            5 => Blend::Lighten,
            9 => Blend::SoftLight,
            10 => Blend::Difference,
            n => {
                d.skip(format!("blend mode {n}"));
                Blend::Normal
            }
        };
        let kind = match int(l, "ty").unwrap_or(-1) {
            0 => match l.get("refId").and_then(Value::as_str).and_then(|r| ids.get(r)) {
                Some(&comp) => {
                    let tm = l.get("tm").map(|v| prop(Some(v), &[0.0], d));
                    LayerKind::Precomp { comp, w: num(l, "w").unwrap_or(0.0), h: num(l, "h").unwrap_or(0.0), tm }
                }
                None => {
                    d.skip("precomp with a missing asset");
                    LayerKind::Null
                }
            },
            1 => LayerKind::Solid {
                color: l.get("sc").and_then(Value::as_str).and_then(style::parse_color).unwrap_or(Rgba([0.0, 0.0, 0.0, 1.0])),
                w: num(l, "sw").unwrap_or(0.0),
                h: num(l, "sh").unwrap_or(0.0),
            },
            3 => LayerKind::Null,
            4 => LayerKind::Shape(items(l.get("shapes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]), d, false).0),
            t => {
                d.skip(match t {
                    2 => "image layers".to_string(),
                    5 => "text layers".to_string(),
                    6 => "audio layers".to_string(),
                    13 => "camera layers".to_string(),
                    t => format!("layer type {t}"),
                });
                LayerKind::Null
            }
        };
        let empty = Value::Null;
        out.push(Layer {
            parent: None,
            kind,
            ks: xform(l.get("ks").unwrap_or(&empty), d),
            ip: num(l, "ip").unwrap_or(f32::NEG_INFINITY),
            op: num(l, "op").unwrap_or(f32::INFINITY),
            st: num(l, "st").unwrap_or(0.0),
            sr: num(l, "sr").filter(|s| *s != 0.0 && s.is_finite()).unwrap_or(1.0),
            blend,
            hidden: flag(l, "hd") || flag(l, "td"),
        });
    }
    for (l, p) in out.iter_mut().zip(parents) {
        l.parent = p.and_then(|p| by_ind.get(&p).copied());
    }
    out
}

fn parse(v: &Value, d: &mut Diag) -> Result<Anim, Error> {
    if !v.is_object() {
        return Err(Error("Lottie data must be a JSON object".into()));
    }
    let need = |k: &str| num(v, k).filter(|x| x.is_finite()).ok_or_else(|| Error(format!("Lottie data is missing a numeric `{k}`")));
    let (w, h, fr, ip, op) = (need("w")?, need("h")?, need("fr")?, need("ip")?, need("op")?);
    if w <= 0.0 || h <= 0.0 || fr <= 0.0 || op <= ip {
        return Err(Error(format!("Lottie composition needs w, h, fr > 0 and op > ip (got w={w} h={h} fr={fr} ip={ip} op={op})")));
    }
    let root = v.get("layers").and_then(Value::as_array).ok_or_else(|| Error("Lottie data has no `layers` array".into()))?;
    let mut sources = vec![root.as_slice()];
    let mut ids = HashMap::new();
    for a in v.get("assets").and_then(Value::as_array).into_iter().flatten() {
        if let (Some(id), Some(ls)) = (a.get("id").and_then(Value::as_str), a.get("layers").and_then(Value::as_array)) {
            ids.insert(id.to_string(), sources.len());
            sources.push(ls.as_slice());
        }
    }
    let comps = sources.iter().map(|ls| layers(ls, &ids, d)).collect();
    Ok(Anim { w, h, fr, ip, op, comps })
}

// ---------------------------------------------------------------- rendering

/// A path collected in a group, with the trim that applies to it.
struct Pend {
    path: Path,
    trim: Option<[f32; 4]>,
}

struct Render<'a> {
    a: &'a Anim,
    tint: Option<Rgba>,
}

fn kaff(m: &Affine) -> kurbo::Affine {
    kurbo::Affine::new(m.map(|v| v as f64))
}

fn rgb(v: &[f32]) -> [f32; 3] {
    let mut c = [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)];
    if c.iter().any(|x| *x > 1.0) {
        c = c.map(|x| x / 255.0);
    }
    c.map(|x| x.clamp(0.0, 1.0))
}

/// Trim `[start, end, offset]` (percent, percent, degrees) → up to two `[a, b]` ranges of 0..1.
fn trim_ranges(t: [f32; 4]) -> Vec<[f32; 2]> {
    let (mut a, mut b) = ((t[0] / 100.0).clamp(0.0, 1.0), (t[1] / 100.0).clamp(0.0, 1.0));
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    if b - a >= 0.9999 {
        return vec![[0.0, 1.0]];
    }
    if b - a <= 1e-4 {
        return Vec::new();
    }
    let off = t[2] / 360.0;
    a += off;
    b += off;
    let k = a.floor();
    a -= k;
    b -= k;
    if b <= 1.0 {
        vec![[a, b]]
    } else {
        vec![[a, 1.0], [0.0, b - 1.0]]
    }
}

fn combined(pend: &[Pend]) -> Path {
    let mut p = Path::new();
    for x in pend {
        p.extend(x.path.elements().iter().copied());
    }
    p
}

impl Render<'_> {
    fn comp(&self, ci: usize, f: f32, m: &Affine, out: &mut Vec<Op>, depth: u32) {
        let Some(layers) = self.a.comps.get(ci) else { return };
        if depth > 16 {
            return;
        }
        for (li, l) in layers.iter().enumerate().rev() {
            if l.hidden || f < l.ip || f >= l.op || matches!(l.kind, LayerKind::Null) {
                continue;
            }
            let lf = (f - l.st) / l.sr;
            let lm = dl::mul(m, &self.layer_matrix(layers, li, f, 0));
            let mut ops = Vec::new();
            match &l.kind {
                LayerKind::Null => {}
                LayerKind::Shape(items) => {
                    self.group(items, lf, &lm, &mut ops, None);
                }
                LayerKind::Solid { color, w, h } => {
                    let p = kurbo::Rect::new(0.0, 0.0, *w as f64, *h as f64).to_path(0.1);
                    ops.push(Op::Fill { path: p, paint: Paint::Solid(self.tinted(*color)), rule: FillRule::NonZero, transform: lm });
                }
                LayerKind::Precomp { comp, w, h, tm } => {
                    let cf = tm.as_ref().map(|p| p.scalar(lf) * self.a.fr).unwrap_or(lf);
                    let mut sub = Vec::new();
                    self.comp(*comp, cf, &lm, &mut sub, depth + 1);
                    if !sub.is_empty() && *w > 0.0 && *h > 0.0 {
                        let clip = kurbo::Rect::new(0.0, 0.0, *w as f64, *h as f64).to_path(0.1);
                        ops.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: Some((clip, lm)), cache_key: None });
                        ops.extend(sub);
                        ops.push(Op::PopLayer);
                    } else {
                        ops.extend(sub);
                    }
                }
            }
            let mut ops = with_opacity(ops, l.ks.opacity(lf));
            if l.blend != Blend::Normal && !ops.is_empty() {
                ops.insert(0, Op::PushLayer { opacity: 1.0, blend: l.blend, filters: Vec::new(), clip: None, cache_key: None });
                ops.push(Op::PopLayer);
            }
            out.extend(ops);
        }
    }

    /// Layer → composition transform, through its parents (each at its own local time).
    fn layer_matrix(&self, layers: &[Layer], li: usize, f: f32, depth: u32) -> Affine {
        let l = &layers[li];
        let own = l.ks.matrix((f - l.st) / l.sr);
        match l.parent {
            Some(p) if p != li && depth < 32 => dl::mul(&self.layer_matrix(layers, p, f, depth + 1), &own),
            _ => own,
        }
    }

    fn tinted(&self, c: Rgba) -> Rgba {
        match self.tint {
            Some(t) => Rgba([t.0[0], t.0[1], t.0[2], c.0[3] * t.0[3]]),
            None => c,
        }
    }

    fn paint(&self, b: &Brush, f: f32, alpha: f32) -> Paint {
        match b {
            Brush::Solid(c) => {
                let [r, g, bl] = rgb(&c.get(f));
                Paint::Solid(self.tinted(Rgba([r, g, bl, alpha])))
            }
            Brush::Grad(g) => {
                let v = g.k.get(f);
                let n = g.n.min(v.len() / 4);
                let alphas = &v[n * 4..];
                let alpha_at = |x: f32| -> f32 {
                    let pairs: Vec<[f32; 2]> = alphas.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
                    match pairs.as_slice() {
                        [] => 1.0,
                        [only] => only[1],
                        ps => {
                            if x <= ps[0][0] {
                                return ps[0][1];
                            }
                            for w in ps.windows(2) {
                                if x <= w[1][0] {
                                    let span = (w[1][0] - w[0][0]).max(1e-6);
                                    return w[0][1] + (w[1][1] - w[0][1]) * ((x - w[0][0]) / span);
                                }
                            }
                            ps[ps.len() - 1][1]
                        }
                    }
                };
                let stops: Vec<(f32, Rgba)> = (0..n)
                    .map(|j| {
                        let off = v[j * 4];
                        let [r, gg, bl] = rgb(&v[j * 4 + 1..j * 4 + 4]);
                        (off.clamp(0.0, 1.0), self.tinted(Rgba([r, gg, bl, alpha_at(off).clamp(0.0, 1.0) * alpha])))
                    })
                    .collect();
                if stops.is_empty() {
                    return Paint::Solid(Rgba([0.0, 0.0, 0.0, 0.0]));
                }
                let (p0, p1) = (g.s.vec2(f), g.e.vec2(f));
                if g.radial {
                    Paint::Radial { c: p0, r: (p1[0] - p0[0]).hypot(p1[1] - p0[1]), stops }
                } else {
                    Paint::Linear { p0, p1, stops }
                }
            }
        }
    }

    /// Render a group's items into `out` (later items draw underneath) and
    /// return its paths in the group's own coordinates, so enclosing styles
    /// can draw them too (the caller applies the group transform).
    fn group(&self, items: &[Item], f: f32, m: &Affine, out: &mut Vec<Op>, inherited: Option<[f32; 4]>) -> Vec<Pend> {
        // Trim applying to each item: the nearest trim after it, else the parent's.
        let mut trim_after = vec![None; items.len()];
        let mut cur = inherited;
        for (j, it) in items.iter().enumerate().rev() {
            trim_after[j] = cur;
            if let Item::Trim(t) = it {
                cur = Some([t.s.scalar(f), t.e.scalar(f), t.o.scalar(f), t.mode as f32]);
            }
        }
        let mut pend: Vec<Pend> = Vec::new();
        let mut chunks: Vec<Vec<Op>> = Vec::new();
        for (j, it) in items.iter().enumerate() {
            match it {
                Item::Group { items, tr } => {
                    let gm = tr.as_ref().map(|t| t.matrix(f)).unwrap_or(IDENTITY);
                    let go = tr.as_ref().map(|t| t.opacity(f)).unwrap_or(1.0);
                    let mut sub = Vec::new();
                    let paths = self.group(items, f, &dl::mul(m, &gm), &mut sub, trim_after[j]);
                    chunks.push(with_opacity(sub, go));
                    let ka = kaff(&gm);
                    for mut p in paths {
                        p.path.apply_affine(ka);
                        pend.push(p);
                    }
                }
                Item::Trim(_) => {}
                Item::Fill { brush, o, rule } => {
                    let alpha = (o.scalar(f) / 100.0).clamp(0.0, 1.0);
                    if pend.is_empty() || alpha <= 0.0 {
                        continue;
                    }
                    chunks.push(vec![Op::Fill { path: combined(&pend), paint: self.paint(brush, f, alpha), rule: *rule, transform: *m }]);
                }
                Item::Stroke { brush, o, w, cap, join, ml, dash } => {
                    let alpha = (o.scalar(f) / 100.0).clamp(0.0, 1.0);
                    let width = w.scalar(f);
                    if pend.is_empty() || alpha <= 0.0 || width <= 0.0 {
                        continue;
                    }
                    let paint = self.paint(brush, f, alpha);
                    let mut pattern = Vec::new();
                    let mut offset = 0.0;
                    for (k, p) in dash {
                        match k {
                            2 => offset = p.scalar(f),
                            _ => pattern.push(p.scalar(f).max(0.0)),
                        }
                    }
                    if pattern.iter().sum::<f32>() <= 0.0 {
                        pattern.clear();
                    } else if pattern.len() % 2 == 1 {
                        let again = pattern.clone();
                        pattern.extend(again);
                    }
                    let style = Stroke { width, cap: *cap, join: *join, miter_limit: *ml, dash: pattern, dash_offset: offset, trim: [0.0, 1.0] };
                    let mut ops = Vec::new();
                    let mut emit = |path: Path, trim: Option<[f32; 4]>| {
                        let ranges = trim.map(trim_ranges).unwrap_or_else(|| vec![[0.0, 1.0]]);
                        for r in ranges {
                            ops.push(Op::Stroke { path: path.clone(), paint: paint.clone(), stroke: Stroke { trim: r, ..style.clone() }, transform: *m });
                        }
                    };
                    let first = pend[0].trim;
                    if pend.iter().all(|p| p.trim.is_none()) || (first.is_some_and(|t| t[3] == 2.0) && pend.iter().all(|p| p.trim == first)) {
                        emit(combined(&pend), first);
                    } else {
                        for p in &pend {
                            emit(p.path.clone(), p.trim);
                        }
                    }
                    chunks.push(ops);
                }
                shape => {
                    if let Some(path) = shape_path(shape, f) {
                        pend.push(Pend { path, trim: trim_after[j] });
                    }
                }
            }
        }
        for c in chunks.into_iter().rev() {
            out.extend(c);
        }
        pend
    }
}

fn pt(x: f32, y: f32) -> Point {
    Point::new(x as f64, y as f64)
}

fn shape_path(it: &Item, f: f32) -> Option<Path> {
    let (mut p, rev) = match it {
        Item::Rect { p, s, r, rev } => {
            let ([cx, cy], [w, h]) = (p.vec2(f), s.vec2(f));
            let r = r.scalar(f).min(w * 0.5).min(h * 0.5).max(0.0);
            let (x0, y0, x1, y1) = (cx - w * 0.5, cy - h * 0.5, cx + w * 0.5, cy + h * 0.5);
            let path = if r > 0.0 {
                kurbo::RoundedRect::new(x0 as f64, y0 as f64, x1 as f64, y1 as f64, r as f64).to_path(0.1)
            } else {
                // Lottie order: from the top-right corner, clockwise.
                let mut b = Path::new();
                b.move_to(pt(x1, y0));
                b.line_to(pt(x1, y1));
                b.line_to(pt(x0, y1));
                b.line_to(pt(x0, y0));
                b.close_path();
                b
            };
            (path, *rev)
        }
        Item::Ellipse { p, s, rev } => {
            let ([cx, cy], [w, h]) = (p.vec2(f), s.vec2(f));
            let (rx, ry) = (w * 0.5, h * 0.5);
            const K: f32 = 0.552_284_8;
            let mut b = Path::new();
            b.move_to(pt(cx, cy - ry));
            b.curve_to(pt(cx + K * rx, cy - ry), pt(cx + rx, cy - K * ry), pt(cx + rx, cy));
            b.curve_to(pt(cx + rx, cy + K * ry), pt(cx + K * rx, cy + ry), pt(cx, cy + ry));
            b.curve_to(pt(cx - K * rx, cy + ry), pt(cx - rx, cy + K * ry), pt(cx - rx, cy));
            b.curve_to(pt(cx - rx, cy - K * ry), pt(cx - K * rx, cy - ry), pt(cx, cy - ry));
            b.close_path();
            (b, *rev)
        }
        Item::Star { polygon, p, pt: n, r, or, ir, rev } => {
            let [cx, cy] = p.vec2(f);
            let n = n.scalar(f).round().clamp(3.0, 100.0) as usize;
            let (or, ir) = (or.scalar(f), ir.scalar(f));
            let a0 = (r.scalar(f) - 90.0).to_radians();
            let count = if *polygon { n } else { 2 * n };
            let mut b = Path::new();
            for k in 0..count {
                let ang = a0 + k as f32 * std::f32::consts::TAU / count as f32;
                let rad = if *polygon || k % 2 == 0 { or } else { ir };
                let q = pt(cx + rad * ang.cos(), cy + rad * ang.sin());
                if k == 0 {
                    b.move_to(q);
                } else {
                    b.line_to(q);
                }
            }
            b.close_path();
            (b, *rev)
        }
        Item::Path { ks, closed, rev } => {
            let v = ks.get(f);
            let n = v.len() / 6;
            if n == 0 {
                return None;
            }
            let at = |base: usize, j: usize| [v[base + 2 * j], v[base + 2 * j + 1]];
            let (vi, ii, oi) = (0, 2 * n, 4 * n);
            let mut b = Path::new();
            let v0 = at(vi, 0);
            b.move_to(pt(v0[0], v0[1]));
            let seg = |b: &mut Path, j0: usize, j1: usize| {
                let (a, o) = (at(vi, j0), at(oi, j0));
                let (c, i) = (at(vi, j1), at(ii, j1));
                b.curve_to(pt(a[0] + o[0], a[1] + o[1]), pt(c[0] + i[0], c[1] + i[1]), pt(c[0], c[1]));
            };
            for j in 1..n {
                seg(&mut b, j - 1, j);
            }
            if *closed {
                seg(&mut b, n - 1, 0);
                b.close_path();
            }
            (b, *rev)
        }
        _ => return None,
    };
    if rev {
        p = p.reverse_subpaths();
    }
    if p.elements().iter().any(|e| matches!(e, PathEl::MoveTo(q) | PathEl::LineTo(q) if !q.is_finite())) {
        return None;
    }
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use super::*;
    use serde_json::json;

    const DEMO: &str = include_str!("../../tests/fixtures/lottie-demo.json");

    fn demo() -> Value {
        serde_json::from_str(DEMO).unwrap()
    }

    fn lottie(params: Value) -> Lottie {
        let mut l: Lottie = serde_json::from_value(params).unwrap();
        l.prepare().unwrap();
        l
    }

    fn eval(l: &Lottie, t: f32, w: f32, h: f32) -> Vec<Op> {
        let th = Theme::default();
        let a = preview::host();
        let ctx = Ctx { width: w, height: h, fps: 30.0, theme: &th, assets: a };
        l.eval(t, &ctx)
    }

    fn apply(m: &Affine, p: [f32; 2]) -> [f32; 2] {
        [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
    }

    /// Fill ops with the given solid colour.
    fn fills_of(ops: &[Op], rgb: [f32; 3]) -> Vec<(Path, Affine)> {
        ops.iter()
            .filter_map(|o| match o {
                Op::Fill { path, paint: Paint::Solid(c), transform, .. } if (0..3).all(|i| (c.0[i] - rgb[i]).abs() < 1e-3) => Some((path.clone(), *transform)),
                _ => None,
            })
            .collect()
    }

    fn centre(p: &Path, m: &Affine) -> [f32; 2] {
        let b = p.bounding_box();
        apply(m, [b.center().x as f32, b.center().y as f32])
    }

    /// Reference easing by dense sampling (independent of the solver).
    fn ease_ref(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
        let cb = |p1: f32, p2: f32, t: f32| 3.0 * (1.0 - t).powi(2) * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t * t * t;
        let mut best = (f32::MAX, 0.0);
        for k in 0..=100_000 {
            let t = k as f32 / 100_000.0;
            let d = (cb(x1, x2, t) - x).abs();
            if d < best.0 {
                best = (d, cb(y1, y2, t));
            }
        }
        best.1
    }

    #[test]
    fn keyframes_interpolate() {
        let mut d = Diag::default();
        let p = prop(
            Some(&json!({"a": 1, "k": [
                {"t": 0, "s": [0, 10], "o": {"x": [0.42], "y": [0]}, "i": {"x": [0.58], "y": [1]}},
                {"t": 10, "s": [100, 20], "h": 1},
                {"t": 20, "s": [50, 50], "o": {"x": 0, "y": 0}, "i": {"x": 1, "y": 1}},
                {"t": 30, "s": [0, 0]}
            ]})),
            &[0.0],
            &mut d,
        );
        assert_eq!(p.get(-5.0), vec![0.0, 10.0]);
        assert_eq!(p.get(0.0), vec![0.0, 10.0]);
        assert_eq!(p.get(10.0), vec![100.0, 20.0]);
        // symmetric ease-in-out: exactly half way at the midpoint
        let mid = p.get(5.0);
        assert!((mid[0] - 50.0).abs() < 1e-3 && (mid[1] - 15.0).abs() < 1e-3, "{mid:?}");
        // a quarter in: matches the reference curve, and lags linear
        let q = p.get(2.5)[0];
        let r = 100.0 * ease_ref(0.42, 0.0, 0.58, 1.0, 0.25);
        assert!((q - r).abs() < 0.05, "{q} vs {r}");
        assert!(q < 25.0);
        // hold
        assert_eq!(p.get(15.0), vec![100.0, 20.0]);
        assert_eq!(p.get(19.99), vec![100.0, 20.0]);
        // linear
        assert_eq!(p.get(25.0), vec![25.0, 25.0]);
        assert_eq!(p.get(99.0), vec![0.0, 0.0]);
        // old format: `e` given, last key only has `t`
        let old = prop(Some(&json!({"k": [{"t": 0, "s": [0], "e": [10], "o": {"x": [0], "y": [0]}, "i": {"x": [1], "y": [1]}}, {"t": 10}]})), &[0.0], &mut d);
        assert_eq!(old.scalar(5.0), 5.0);
        assert_eq!(old.scalar(12.0), 10.0);
        // spatial bezier: a semicircle-ish arc bulges off the straight line
        let sp = prop(Some(&json!({"k": [{"t": 0, "s": [0, 0], "to": [0, -50], "ti": [0, -50], "o": {"x": 0, "y": 0}, "i": {"x": 1, "y": 1}}, {"t": 10, "s": [100, 0]}]})), &[0.0], &mut d);
        let m = sp.vec2(5.0);
        assert!((m[0] - 50.0).abs() < 0.5 && m[1] < -30.0, "{m:?}");
        assert!(d.0.is_empty());
    }

    #[test]
    fn ball_bounces_with_easing() {
        let l = lottie(json!({"data": demo()}));
        assert!(l.unsupported().is_empty(), "{:?}", l.unsupported());
        let red = [0.95, 0.3, 0.35];
        let ball_y = |t: f32| {
            let ops = eval(&l, t, 200.0, 200.0);
            let f = fills_of(&ops, red);
            assert_eq!(f.len(), 1);
            centre(&f[0].0, &f[0].1)
        };
        let at0 = ball_y(0.0);
        assert!((at0[0] - 100.0).abs() < 1e-3 && (at0[1] - 40.0).abs() < 1e-3, "{at0:?}");
        assert!((ball_y(1.0)[1] - 150.0).abs() < 1e-3); // frame 30
        assert!((ball_y(0.5)[1] - 95.0).abs() < 1e-2); // symmetric ease, half way
        let q = ball_y(0.25)[1]; // frame 7.5
        let r = 40.0 + 110.0 * ease_ref(0.33, 0.0, 0.67, 1.0, 0.25);
        assert!((q - r).abs() < 0.05, "{q} vs {r}");
        // loops: t = 2 s is frame 0 again
        assert!((ball_y(2.0)[1] - 40.0).abs() < 1e-3);
    }

    #[test]
    fn trim_maps_to_stroke_trim() {
        let l = lottie(json!({"data": demo()}));
        let trims = |t: f32| -> Vec<[f32; 2]> {
            eval(&l, t, 200.0, 200.0)
                .iter()
                .filter_map(|o| match o {
                    Op::Stroke { stroke, .. } => Some(stroke.trim),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(trims(0.0), Vec::<[f32; 2]>::new()); // empty trim draws nothing
        let half = trims(0.5);
        assert_eq!(half.len(), 1);
        assert!((half[0][0]).abs() < 1e-4 && (half[0][1] - 0.5).abs() < 1e-4, "{half:?}");
        assert_eq!(trims(1.5), vec![[0.0, 1.0]]);
        // offsets wrap into two ranges
        assert_eq!(trim_ranges([0.0, 50.0, 270.0, 1.0]), vec![[0.75, 1.0], [0.0, 0.25]]);
        assert_eq!(trim_ranges([80.0, 20.0, 0.0, 1.0]), vec![[0.2, 0.8]]);
    }

    #[test]
    fn parenting_and_group_rotation() {
        let l = lottie(json!({"data": demo()}));
        let yellow = [1.0, 0.8, 0.2];
        let sq = |t: f32| {
            let ops = eval(&l, t, 200.0, 200.0);
            let f = fills_of(&ops, yellow);
            assert_eq!(f.len(), 1);
            f[0].clone()
        };
        let (p, m) = sq(0.0);
        let c = centre(&p, &m);
        assert!((c[0] - 190.0).abs() < 1e-3 && (c[1] - 60.0).abs() < 1e-3, "{c:?}");
        // the group's 45° rotation: the square's corner points straight right
        let b = kurbo::Affine::new(m.map(|v| v as f64)) * p.clone();
        let bb = b.bounding_box();
        assert!((bb.width() - 20.0 * 2f64.sqrt()).abs() < 1e-3, "{bb:?}");
        // the null parent turns 90° by frame 15 (linear 0→360 over 60 frames)
        let (p, m) = sq(0.5);
        let c = centre(&p, &m);
        assert!((c[0] - 160.0).abs() < 1e-2 && (c[1] - 90.0).abs() < 1e-2, "{c:?}");
    }

    #[test]
    fn hold_opacity_and_gradient() {
        let l = lottie(json!({"data": demo()}));
        let floor = |t: f32| -> (Option<f32>, Paint) {
            let ops = eval(&l, t, 200.0, 200.0);
            let i = ops.iter().position(|o| matches!(o, Op::Fill { paint: Paint::Linear { .. }, .. })).expect("gradient floor");
            let op = match &ops[i - 1] {
                Op::PushLayer { opacity, clip: None, .. } => Some(*opacity),
                _ => None,
            };
            let Op::Fill { paint, .. } = &ops[i] else { unreachable!() };
            (op, paint.clone())
        };
        assert_eq!(floor(1.3).0, None); // frame 39: held at 100
        assert_eq!(floor(1.34).0, Some(0.5)); // frame 40.2
        let Paint::Linear { p0, p1, stops } = floor(0.0).1 else { panic!() };
        assert_eq!((p0, p1), ([20.0, 185.0], [180.0, 185.0]));
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0].1, Rgba([0.2, 0.6, 1.0, 1.0]));
        assert!((stops[1].1 .0[3] - 0.4).abs() < 1e-5);
    }

    #[test]
    fn fit_loop_speed_and_delay() {
        let mut v = demo();
        v["op"] = json!(60);
        let red = [0.95, 0.3, 0.35];
        // contain into 400×200: scale 1, centred horizontally
        let l = lottie(json!({"data": v.clone()}));
        let ops = eval(&l, 0.0, 400.0, 200.0);
        let f = fills_of(&ops, red);
        assert_eq!(centre(&f[0].0, &f[0].1), [200.0, 40.0]);
        // clip covers the composition only
        let Op::PushLayer { clip: Some((c, _)), .. } = &ops[0] else { panic!() };
        assert_eq!(c.bounding_box(), kurbo::Rect::new(100.0, 0.0, 300.0, 200.0));
        // cover into 400×200: scale 2, centred vertically
        let l = lottie(json!({"data": v.clone(), "fit": "cover"}));
        let ops = eval(&l, 0.0, 400.0, 200.0);
        let f = fills_of(&ops, red);
        assert_eq!(centre(&f[0].0, &f[0].1), [200.0, -20.0]);
        // none: natural size centred
        let l = lottie(json!({"data": v.clone(), "fit": "none"}));
        let f = fills_of(&eval(&l, 0.0, 400.0, 400.0), red);
        assert_eq!(centre(&f[0].0, &f[0].1), [200.0, 140.0]);
        // no loop: holds the end, has a duration
        let l = lottie(json!({"data": v.clone(), "loop": false, "speed": 2.0, "delay": 0.5}));
        assert_eq!(l.duration(), Some(1.5));
        let y = |l: &Lottie, t: f32| {
            let f = fills_of(&eval(l, t, 200.0, 200.0), red);
            centre(&f[0].0, &f[0].1)[1]
        };
        assert!((y(&l, 0.3) - 40.0).abs() < 1e-3); // during the delay
        assert!((y(&l, 1.0) - 150.0).abs() < 1e-3); // frame 30 at 2×
        assert!((y(&l, 9.0) - 40.0).abs() < 0.1); // held at the end
        // recolour
        let l = lottie(json!({"data": v, "color": "#00ff00"}));
        assert!(fills_of(&eval(&l, 0.0, 200.0, 200.0), red).is_empty());
        assert!(!fills_of(&eval(&l, 0.0, 200.0, 200.0), [0.0, 1.0, 0.0]).is_empty());
    }

    #[test]
    fn unsupported_is_counted_and_errors_are_clear() {
        let l = lottie(json!({"data": {
            "w": 100, "h": 100, "fr": 30, "ip": 0, "op": 30,
            "layers": [
                {"ty": 5, "ind": 1, "ks": {}},
                {"ty": 4, "ind": 2, "hasMask": true, "masksProperties": [{}], "ks": {}, "shapes": [
                    {"ty": "rc", "p": {"k": [50, 50]}, "s": {"k": [20, 20]}, "r": {"k": 0}},
                    {"ty": "rp"}, {"ty": "rd"}, {"ty": "rd"},
                    {"ty": "fl", "c": {"k": [1, 0, 0, 1]}, "o": {"k": 100}}
                ]}
            ]
        }}));
        let u = l.unsupported();
        assert!(u.contains(&"text layers".to_string()), "{u:?}");
        assert!(u.contains(&"masks".to_string()), "{u:?}");
        assert!(u.contains(&"repeater".to_string()), "{u:?}");
        assert!(u.contains(&"round corners (×2)".to_string()), "{u:?}");
        // still draws what it understands
        assert_eq!(fills_of(&eval(&l, 0.1, 100.0, 100.0), [1.0, 0.0, 0.0]).len(), 1);

        let err = |p: Value| {
            let mut l: Lottie = serde_json::from_value(p).unwrap();
            l.prepare().err().unwrap().0
        };
        assert!(err(json!({})).contains("needs `src`"));
        assert!(err(json!({"src": "a.json", "data": {}})).contains("not both"));
        assert!(err(json!({"data": {"w": 10}})).contains("missing"));
        assert!(err(json!({"data": "{not json"})).contains("not valid JSON"));
        assert!(build("lottie", &json!({"data": {}, "spped": 2})).err().unwrap().0.contains("did you mean `speed`"));
        // a missing file is reported, not fatal
        let l = lottie(json!({"src": "/nonexistent/anim.json"}));
        assert!(l.unsupported()[0].contains("could not read"));
        assert!(eval(&l, 0.0, 100.0, 100.0).is_empty());
    }

    #[test]
    fn loads_src_and_validates_over_time() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/lottie-demo.json");
        let th = Theme::default();
        let a = preview::host();
        let p = build("lottie", &json!({"src": path, "x": 20, "y": 10, "width": 300, "height": 200})).unwrap();
        for k in 0..=40 {
            let t = k as f32 * 0.07;
            let ops = p.eval(t, 640.0, 360.0, 30.0, &th, a);
            preview::validate(&ops).unwrap_or_else(|e| panic!("t={t}: {e}"));
            assert!(!ops.is_empty());
        }
    }

    #[test]
    fn snapshots() {
        let l = lottie(json!({"data": demo()}));
        let th = Theme::default();
        let a = preview::host();
        let ctx = Ctx { width: 400.0, height: 300.0, fps: 30.0, theme: &th, assets: a };
        for (i, t) in [0.0f32, 0.25, 0.5, 1.0, 1.5].iter().enumerate() {
            let ops = l.eval(*t, &ctx);
            preview::validate(&ops).unwrap();
            let px = preview::snapshot(&format!("lottie-{i}"), 400, 300, th.background, ops, a);
            assert!(px.chunks(4).any(|p| p[0] > 200 && p[1] < 120), "ball visible at t={t}");
        }
    }
}
