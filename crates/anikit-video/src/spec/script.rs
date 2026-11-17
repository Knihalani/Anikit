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
