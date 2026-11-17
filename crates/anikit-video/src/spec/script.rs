//! `script` blocks: procedural generation inside the spec, in Rhai (a small,
//! sandboxed, deterministic language: no files, no network, no clock, bounded work).
//!
//! - In any `scenes`, `layers` or `children` list, `{ "script": "…" }` is replaced
//!   by what the script returns: a list of items, or one item (objects or strings,
//!   exactly as you would write them in JSON).
//! - At the top level, `"script": "…"` runs first with the whole spec in `spec`;
//!   it may change `spec` or return a new one.
//!
//! Scripts see `w`, `h` (frame size) and any bound `$params` (`i`, `n` inside
//! `repeat`, `with` values) as variables. Helpers: `rand(seed)`, `rand(a, b)`
//! (0..1, deterministic), `noise(x)`, `lerp(a, b, t)`, `clamp(x, lo, hi)`,
//! `remap(x, a, b, c, d)`, `hsl(h, s, l)` → `"#rrggbb"`, `fmt(x, digits)`.

use rhai::{Dynamic, Engine, Scope};
use serde_json::{Map, Value};

use super::Diagnostic;

fn hash(x: f64) -> f64 {
    let mut h = (x.to_bits() ^ 0x9E37_79B9_7F4A_7C15).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 31;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn noise(x: f64) -> f64 {
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let (a, b) = (hash(i) * 2.0 - 1.0, hash(i + 1.0) * 2.0 - 1.0);
    a + (b - a) * u
}

fn noise2(x: f64, y: f64) -> f64 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let h = |a: f64, b: f64| hash(a * 127.1 + b * 311.7) * 2.0 - 1.0;
    let (a, b, c, d) = (h(ix, iy), h(ix + 1.0, iy), h(ix, iy + 1.0), h(ix + 1.0, iy + 1.0));
    let top = a + (b - a) * ux;
    let bot = c + (d - c) * ux;
    top + (bot - top) * uy
}

fn noise3(x: f64, y: f64, z: f64) -> f64 {
    let iz = z.floor();
    let fz = z - iz;
    let uz = fz * fz * (3.0 - 2.0 * fz);
    let a = noise2(x + iz * 17.13, y - iz * 9.71);
    let b = noise2(x + (iz + 1.0) * 17.13, y - (iz + 1.0) * 9.71);
    a + (b - a) * uz
}

/// Fractal noise: `octaves` layers of `noise2`, each twice the frequency and half the amplitude.
fn fbm(x: f64, y: f64, octaves: i64) -> f64 {
    let (mut sum, mut amp, mut f, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..octaves.clamp(1, 10) {
        sum += amp * noise2(x * f, y * f);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

fn ease(name: &str, x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    match name {
        "linear" => x,
        "in" | "inCubic" => x * x * x,
        "out" | "outCubic" => 1.0 - (1.0 - x).powi(3),
        "inOutSine" => -((std::f64::consts::PI * x).cos() - 1.0) / 2.0,
        "outExpo" => if x >= 1.0 { 1.0 } else { 1.0 - 2f64.powf(-10.0 * x) },
        "inExpo" => if x <= 0.0 { 0.0 } else { 2f64.powf(10.0 * x - 10.0) },
        "outBack" => {
            let c = 1.70158;
            1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2)
        }
        "outElastic" => {
            if x <= 0.0 || x >= 1.0 { x } else { 2f64.powf(-10.0 * x) * ((x * 10.0 - 0.75) * std::f64::consts::TAU / 3.0).sin() + 1.0 }
        }
        "outBounce" => {
            let (n, d) = (7.5625, 2.75);
            if x < 1.0 / d {
                n * x * x
            } else if x < 2.0 / d {
                let x = x - 1.5 / d;
                n * x * x + 0.75
            } else if x < 2.5 / d {
                let x = x - 2.25 / d;
                n * x * x + 0.9375
            } else {
                let x = x - 2.625 / d;
                n * x * x + 0.984375
            }
        }
        // inOutCubic
        _ => if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 },
    }
}

/// Named gradient palettes sampled at `x` in 0..1 → `"#rrggbb"`.
fn palette(name: &str, x: f64) -> String {
    let stops: &[(u8, u8, u8)] = match name {
        "viridis" => &[(68, 1, 84), (59, 82, 139), (33, 145, 140), (94, 201, 98), (253, 231, 37)],
        "magma" => &[(0, 0, 4), (81, 18, 124), (183, 55, 121), (252, 137, 97), (252, 253, 191)],
        "sunset" => &[(25, 20, 60), (120, 40, 110), (230, 80, 90), (250, 160, 80), (255, 230, 150)],
        "ocean" => &[(5, 20, 50), (10, 70, 120), (20, 140, 170), (90, 200, 200), (210, 250, 240)],
        "neon" => &[(255, 0, 128), (140, 0, 255), (0, 140, 255), (0, 255, 200), (200, 255, 0)],
        "mono" => &[(10, 10, 12), (250, 250, 250)],
        // rainbow
        _ => return hsl(x.clamp(0.0, 1.0) * 300.0, 85.0, 58.0),
    };
    let x = x.clamp(0.0, 1.0) * (stops.len() - 1) as f64;
    let i = (x.floor() as usize).min(stops.len() - 2);
    let f = x - i as f64;
    let (a, b) = (stops[i], stops[i + 1]);
    let m = |p: u8, q: u8| (p as f64 + (q as f64 - p as f64) * f).round() as u8;
    format!("#{:02x}{:02x}{:02x}", m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

fn path_poly(cx: f64, cy: f64, r: f64, n: i64, inner: Option<f64>) -> String {
    let n = n.clamp(3, 1000) as usize;
    let count = if inner.is_some() { n * 2 } else { n };
    let mut d = String::new();
    for k in 0..count {
        let rr = match inner {
            Some(ri) if k % 2 == 1 => ri,
            _ => r,
        };
        let a = -std::f64::consts::FRAC_PI_2 + k as f64 * std::f64::consts::TAU / count as f64;
        d.push_str(&format!("{}{:.2} {:.2} ", if k == 0 { "M" } else { "L" }, cx + rr * a.cos(), cy + rr * a.sin()));
    }
    d.push('Z');
    d
}

fn hsl(h: f64, s: f64, l: f64) -> String {
    let (h, s, l) = (h.rem_euclid(360.0) / 60.0, (s / 100.0).clamp(0.0, 1.0), (l / 100.0).clamp(0.0, 1.0));
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let q = |v: f64| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", q(r), q(g), q(b))
}

fn engine() -> Engine {
    let mut e = Engine::new();
    e.set_max_operations(50_000_000);
    e.set_max_call_levels(64);
    e.set_max_expr_depths(128, 64);
    e.set_max_string_size(1 << 20);
    e.set_max_array_size(200_000);
    e.set_max_map_size(50_000);
    e.on_print(|_| {});
    e.on_debug(|_, _, _| {});
    // Helpers accept ints or floats in any position.
    fn n(d: Dynamic) -> f64 {
        d.as_float().or_else(|_| d.as_int().map(|i| i as f64)).unwrap_or(0.0)
    }
    e.register_fn("rand", |x: Dynamic| hash(n(x)));
    e.register_fn("rand", |a: Dynamic, b: Dynamic| hash(n(a) * 12.9898 + hash(n(b) * 78.233) * 4375.0));
    e.register_fn("noise", |x: Dynamic| noise(n(x)));
    e.register_fn("lerp", |a: Dynamic, b: Dynamic, t: Dynamic| n(a.clone()) + (n(b) - n(a)) * n(t));
    e.register_fn("clamp", |x: Dynamic, lo: Dynamic, hi: Dynamic| {
        let (lo, hi) = (n(lo), n(hi));
        n(x).clamp(lo.min(hi), hi.max(lo))
    });
    e.register_fn("remap", |x: Dynamic, a: Dynamic, b: Dynamic, c: Dynamic, d: Dynamic| {
        let (a, c) = (n(a), n(c));
        c + (n(x) - a) / (n(b) - a) * (n(d) - c)
    });
    e.register_fn("hsl", |h: Dynamic, s: Dynamic, l: Dynamic| hsl(n(h), n(s), n(l)));
    e.register_fn("fmt", |x: Dynamic, d: i64| format!("{:.*}", d.clamp(0, 12) as usize, n(x)));
    e.register_fn("noise2", |x: Dynamic, y: Dynamic| noise2(n(x), n(y)));
    e.register_fn("noise3", |x: Dynamic, y: Dynamic, z: Dynamic| noise3(n(x), n(y), n(z)));
    e.register_fn("noise", |x: Dynamic, y: Dynamic| noise2(n(x), n(y)));
    e.register_fn("fbm", |x: Dynamic, y: Dynamic, o: i64| fbm(n(x), n(y), o));
    e.register_fn("fbm", |x: Dynamic, y: Dynamic| fbm(n(x), n(y), 4));
    e.register_fn("smoothstep", |a: Dynamic, b: Dynamic, x: Dynamic| {
        let (a, b) = (n(a), n(b));
        let t = ((n(x) - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    });
    e.register_fn("ease", |name: &str, x: Dynamic| ease(name, n(x)));
    e.register_fn("fract", |x: Dynamic| {
        let x = n(x);
        x - x.floor()
    });
    e.register_fn("mod", |a: Dynamic, b: Dynamic| n(a).rem_euclid(n(b)));
    e.register_fn("min", |a: Dynamic, b: Dynamic| n(a).min(n(b)));
    e.register_fn("max", |a: Dynamic, b: Dynamic| n(a).max(n(b)));
    e.register_fn("palette", |name: &str, x: Dynamic| palette(name, n(x)));
    e.register_fn("path_circle", |cx: Dynamic, cy: Dynamic, r: Dynamic| {
        let (cx, cy, r) = (n(cx), n(cy), n(r));
        format!("M{:.2} {:.2} a{r:.2} {r:.2} 0 1 0 {:.2} 0 a{r:.2} {r:.2} 0 1 0 {:.2} 0 Z", cx - r, cy, 2.0 * r, -2.0 * r)
    });
    e.register_fn("path_poly", |cx: Dynamic, cy: Dynamic, r: Dynamic, k: i64| path_poly(n(cx), n(cy), n(r), k, None));
    e.register_fn("path_star", |cx: Dynamic, cy: Dynamic, r: Dynamic, ri: Dynamic, k: i64| path_poly(n(cx), n(cy), n(r), k, Some(n(ri))));
    // Vectors are arrays `[x, y]` or `[x, y, z]`.
    fn v(a: &rhai::Array) -> Vec<f64> {
        a.iter().map(|d| n(d.clone())).collect()
    }
    fn arr(x: Vec<f64>) -> rhai::Array {
        x.into_iter().map(Dynamic::from_float).collect()
    }
    e.register_fn("vadd", |a: rhai::Array, b: rhai::Array| arr(v(&a).iter().zip(v(&b)).map(|(p, q)| p + q).collect()));
    e.register_fn("vsub", |a: rhai::Array, b: rhai::Array| arr(v(&a).iter().zip(v(&b)).map(|(p, q)| p - q).collect()));
    e.register_fn("vscale", |a: rhai::Array, k: Dynamic| {
        let k = n(k);
        arr(v(&a).iter().map(|p| p * k).collect())
    });
    e.register_fn("vlen", |a: rhai::Array| v(&a).iter().map(|p| p * p).sum::<f64>().sqrt());
    e.register_fn("vdist", |a: rhai::Array, b: rhai::Array| v(&a).iter().zip(v(&b)).map(|(p, q)| (p - q) * (p - q)).sum::<f64>().sqrt());
    e.register_fn("vnorm", |a: rhai::Array| {
        let x = v(&a);
        let l = x.iter().map(|p| p * p).sum::<f64>().sqrt().max(1e-12);
        arr(x.iter().map(|p| p / l).collect())
    });
    e.register_fn("vrot", |a: rhai::Array, deg: Dynamic| {
        let x = v(&a);
        let (s, c) = n(deg).to_radians().sin_cos();
        arr(vec![x.first().copied().unwrap_or(0.0) * c - x.get(1).copied().unwrap_or(0.0) * s, x.first().copied().unwrap_or(0.0) * s + x.get(1).copied().unwrap_or(0.0) * c])
    });
    e.register_fn("vlerp", |a: rhai::Array, b: rhai::Array, t: Dynamic| {
        let t = n(t);
        arr(v(&a).iter().zip(v(&b)).map(|(p, q)| p + (q - p) * t).collect())
    });
    for (name, f) in [("sin", f64::sin as fn(f64) -> f64), ("cos", f64::cos), ("tan", f64::tan), ("sqrt", f64::sqrt), ("exp", f64::exp), ("ln", f64::ln), ("to_radians", f64::to_radians)] {
        e.register_fn(name, move |x: i64| f(x as f64));
    }
    // Mixed int/float arithmetic without casts (`i * 0.5`).
    e.register_fn("*", |a: i64, b: f64| a as f64 * b);
    e.register_fn("*", |a: f64, b: i64| a * b as f64);
    e.register_fn("+", |a: i64, b: f64| a as f64 + b);
    e.register_fn("+", |a: f64, b: i64| a + b as f64);
    e.register_fn("-", |a: i64, b: f64| a as f64 - b);
    e.register_fn("-", |a: f64, b: i64| a - b as f64);
    e.register_fn("/", |a: i64, b: f64| a as f64 / b);
    e.register_fn("/", |a: f64, b: i64| a / b as f64);
    e
}

/// One engine for every script (it is `Sync`): per-frame `update` scripts share it.
pub fn shared() -> &'static Engine {
    static E: std::sync::OnceLock<Engine> = std::sync::OnceLock::new();
    E.get_or_init(engine)
}

/// Compile a per-frame `update` script once.
pub fn compile_update(code: &Value, path: &str) -> Result<rhai::AST, Diagnostic> {
    let src = source(code).ok_or_else(|| Diagnostic::error(format!("{path}.update"), "'update' must be a string or a list of lines"))?;
    shared().compile(&src).map_err(|e| Diagnostic::error(format!("{path}.update"), format!("update: {e}")))
}

/// Variables of an `update` script, in scope order.
pub const UPDATE_VARS: &[&str] = &["t", "T", "gt", "p", "i", "n", "w", "h", "fps", "frame", "beat", "bar", "level", "bpm"];

/// Run an `update` script with `vals` (in [`UPDATE_VARS`] order); returns its map of props.
pub fn run_update(ast: &rhai::AST, vals: &[f64; 14]) -> Result<rhai::Map, String> {
    let mut sc = Scope::new();
    for (k, v) in UPDATE_VARS.iter().zip(vals) {
        sc.push_constant(*k, *v);
    }
    shared().eval_ast_with_scope::<rhai::Map>(&mut sc, ast).map_err(|e| e.to_string())
}

fn source(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        // A list of lines reads better in JSON.
        Value::Array(a) => a.iter().map(|l| l.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(|l| l.join("\n")),
        _ => None,
    }
}

fn to_dyn(v: &Value) -> Dynamic {
    rhai::serde::to_dynamic(v).unwrap_or(Dynamic::UNIT)
}

fn scope_with(vars: &Map<String, Value>) -> Scope<'static> {
    let mut sc = Scope::new();
    for (k, v) in vars {
        if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !k.starts_with(|c: char| c.is_ascii_digit()) {
            sc.push_dynamic(k.clone(), to_dyn(v));
        }
    }
    sc
}

fn err(path: &str, e: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(format!("{path}.script"), format!("script: {e}"))
}

/// Run an item script; returns the items it generates.
pub fn items(code: &Value, vars: &Map<String, Value>, path: &str) -> Result<Vec<Value>, Diagnostic> {
    let src = source(code).ok_or_else(|| err(path, "'script' must be a string or a list of lines"))?;
    let mut sc = scope_with(vars);
    let out: Dynamic = engine().eval_with_scope(&mut sc, &src).map_err(|e| err(path, e))?;
    let v: Value = rhai::serde::from_dynamic(&out).map_err(|e| err(path, e))?;
    match v {
        Value::Array(a) => Ok(a),
        Value::Object(_) | Value::String(_) => Ok(vec![v]),
        Value::Null => Err(err(path, "the script returned nothing: end it with the list of items (e.g. `out`)")),
        _ => Err(err(path, "the script must return a list of layers or scenes (or one)")),
    }
}

/// Run the top-level script with the spec in `spec`; returns the new spec.
pub fn top(code: &Value, spec: Value) -> Result<Value, Diagnostic> {
    let src = source(code).ok_or_else(|| Diagnostic::error("script", "'script' must be a string or a list of lines"))?;
    let mut sc = Scope::new();
    sc.push_dynamic("spec", to_dyn(&spec));
    let out: Dynamic = engine().eval_with_scope(&mut sc, &src).map_err(|e| Diagnostic::error("script", format!("script: {e}")))?;
    let pick = if out.is_map() { out } else { sc.get_value::<Dynamic>("spec").unwrap_or(Dynamic::UNIT) };
    rhai::serde::from_dynamic(&pick).map_err(|e| Diagnostic::error("script", format!("script: {e}")))
}
