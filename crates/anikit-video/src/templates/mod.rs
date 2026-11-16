//! Templates: JSON video specs with typed parameters, brand kits and
//! automatic 16:9 / 9:16 / 1:1 adaptation.
//!
//! A template file (`crates/anikit-video/templates/<name>.json`) has:
//!
//! ```json
//! {
//!   "name": "launch", "title": "Product launch", "description": "…",
//!   "params": { "product": { "type": "string", "required": true, "maxWords": 3 } },
//!   "defs":   { "card": { … reusable fragment … } },
//!   "spec":   { … a video spec with {{ expressions }} and $directives … }
//! }
//! ```
//!
//! Expansion rules, applied recursively to `spec`:
//! - `"{{ expr }}"` (the whole string) becomes the expression's JSON value;
//!   `"Meet {{ product }}"` interpolates into a string. See [`expr`].
//! - `{"$each": expr, "as": "f", "index": "i", "do": T}` repeats `T` for each
//!   item; inside an array the results are spliced (a `do` array splices all).
//! - `{"$if": expr, "then": T, "else": T}` picks a branch; with no `else` the
//!   element (or object key) disappears.
//! - `{"$let": {"name": expr-or-T, …}, "in": T}` binds names.
//! - `{"$fmt": {"landscape": T, "portrait": T, "square": T}}` picks by aspect
//!   (keys `16:9`, `9:16`, `1:1`, `4:5` work too; missing ones fall back to
//!   `default`, then `landscape`).
//! - `{"$use": "def", "with": {…}}` expands `defs.def` with extra names.
//! - `{"$merge": [T, T, …]}` shallow-merges objects (later wins).
//!
//! Names in scope: every parameter, `brand` (the resolved [`brand::Brand`]),
//! `fmt` (see [`Format::scope`]) and loop variables.

pub mod brand;
pub mod color;
pub mod expr;

use serde_json::{json, Map, Value};

use expr::Scope;

/// Levenshtein distance, for "did you mean" hints.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let c = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + c);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The closest candidate to `name`, if it is plausibly a typo.
pub fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let lname = name.to_lowercase();
    let mut best: Option<(&str, usize)> = None;
    for c in candidates {
        let lc = c.to_lowercase();
        let mut d = edit_distance(&lname, &lc);
        if lc.len() >= 3 && (lname.starts_with(&lc) || lc.starts_with(&lname)) {
            d = d.min(1);
        }
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((c, d));
        }
    }
    let (c, d) = best?;
    let limit = (name.chars().count() / 3).max(2);
    (d <= limit && d > 0 || (d == 0 && c != name)).then_some(c)
}

/// Output format: size and the derived layout values templates use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Format {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Format {
    /// Parse `16:9`, `9:16`, `1:1`, `4:5`, `1080p`, `720p`, `4k`, `vertical`,
    /// `square`, `portrait`, `landscape` or `WxH`.
    pub fn parse(s: &str) -> Result<Format, String> {
        let f = |w, h| Ok(Format { width: w, height: h, fps: 30 });
        match s.trim().to_ascii_lowercase().as_str() {
            "16:9" | "landscape" | "1080p" | "youtube" => f(1920, 1080),
            "720p" => f(1280, 720),
            "4k" | "2160p" => f(3840, 2160),
            "9:16" | "portrait" | "vertical" | "story" | "reel" | "tiktok" | "shorts" => f(1080, 1920),
            "1:1" | "square" => f(1080, 1080),
            "4:5" | "feed" => f(1080, 1350),
            other => {
                if let Some((w, h)) = other.split_once('x') {
                    if let (Ok(w), Ok(h)) = (w.trim().parse::<u32>(), h.trim().parse::<u32>()) {
                        if (16..=8192).contains(&w) && (16..=8192).contains(&h) {
                            return f(w, h);
                        }
                    }
                }
                Err(format!(
                    "unknown format '{s}': use 16:9, 9:16, 1:1, 4:5, 1080p, 720p, 4k or WIDTHxHEIGHT (e.g. 1920x1080)"
                ))
            }
        }
    }

    /// `"landscape"`, `"portrait"` or `"square"`.
    pub fn orientation(&self) -> &'static str {
        let r = self.width as f64 / self.height as f64;
        if r > 1.15 {
            "landscape"
        } else if r < 0.87 {
            "portrait"
        } else {
            "square"
        }
    }

    pub fn aspect(&self) -> String {
        let g = gcd(self.width, self.height);
        format!("{}:{}", self.width / g, self.height / g)
    }

    /// Title-safe margins `[top, right, bottom, left]` in px. Vertical video
    /// keeps clear of the platform UI (captions, buttons) at top and bottom.
    pub fn safe(&self) -> [f64; 4] {
        let (w, h) = (self.width as f64, self.height as f64);
        match self.orientation() {
            "portrait" => [h * 0.12, w * 0.08, h * 0.2, w * 0.08],
            "square" => [h * 0.08, w * 0.08, h * 0.08, w * 0.08],
            _ => [h * 0.09, w * 0.07, h * 0.09, w * 0.07],
        }
    }

    /// The `fmt` object templates see.
    pub fn scope(&self) -> Value {
        let (w, h) = (self.width as f64, self.height as f64);
        let o = self.orientation();
        let [top, right, bottom, left] = self.safe();
        let short = w.min(h);
        json!({
            "w": self.width, "h": self.height, "fps": self.fps,
            "aspect": self.aspect(), "name": o,
            "landscape": o == "landscape", "portrait": o == "portrait", "square": o == "square",
            "cx": expr::num(w / 2.0), "cy": expr::num(h / 2.0),
            // 1% of the short side, and the scale relative to a 1080 short side.
            "u": expr::num(short / 100.0), "s": expr::num(short / 1080.0),
            "safe": { "top": expr::num(top), "right": expr::num(right), "bottom": expr::num(bottom), "left": expr::num(left),
                      "w": expr::num(w - left - right), "h": expr::num(h - top - bottom) },
        })
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

/// A parsed template.
#[derive(Clone, Debug)]
pub struct Template {
    pub name: String,
    pub doc: Value,
}

const DIRECTIVES: [&str; 6] = ["$each", "$if", "$let", "$fmt", "$use", "$merge"];

/// Shared defs available to every template (`templates/_common.json`).
pub const COMMON: &str = include_str!("../../templates/_common.json");

/// Templates shipped with the crate.
pub const BUILTIN: &[(&str, &str)] = &[
    ("launch", include_str!("../../templates/launch.json")),
    ("feature", include_str!("../../templates/feature.json")),
    ("changelog", include_str!("../../templates/changelog.json")),
    ("demo", include_str!("../../templates/demo.json")),
    ("teaser", include_str!("../../templates/teaser.json")),
    ("data-story", include_str!("../../templates/data-story.json")),
    ("testimonial", include_str!("../../templates/testimonial.json")),
    ("tutorial", include_str!("../../templates/tutorial.json")),
    ("countdown", include_str!("../../templates/countdown.json")),
    ("logo-reveal", include_str!("../../templates/logo-reveal.json")),
];

impl Template {
    pub fn from_json(src: &str) -> Result<Template, String> {
        let doc: Value = serde_json::from_str(src).map_err(|e| format!("template JSON: {e}"))?;
        let name = doc.get("name").and_then(|v| v.as_str()).ok_or("template: missing \"name\"")?.to_string();
        for k in ["spec", "params"] {
            if doc.get(k).is_none() {
                return Err(format!("template '{name}': missing \"{k}\""));
            }
        }
        Ok(Template { name, doc })
    }

    /// A built-in template by name.
    pub fn builtin(name: &str) -> Result<Template, String> {
        match BUILTIN.iter().find(|(n, _)| *n == name) {
            Some((_, src)) => Template::from_json(src),
            None => {
                let hint = suggest(name, BUILTIN.iter().map(|(n, _)| *n)).map(|s| format!("did you mean '{s}'? ")).unwrap_or_default();
                Err(format!(
                    "unknown template '{name}'. {hint}Available: {}",
                    BUILTIN.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                ))
            }
        }
    }

    pub fn params(&self) -> Map<String, Value> {
        self.doc.get("params").and_then(|v| v.as_object()).cloned().unwrap_or_default()
    }

    /// Summary for listings: name, title, description, formats, required params.
    pub fn summary(&self) -> Value {
        let params = self.params();
        let required: Vec<&String> = params.iter().filter(|(_, p)| p.get("required") == Some(&Value::Bool(true))).map(|(k, _)| k).collect();
        json!({
            "name": self.name,
            "title": self.doc.get("title"),
            "description": self.doc.get("description"),
            "duration": self.doc.get("duration"),
            "required": required,
            "params": params.keys().collect::<Vec<_>>(),
        })
    }

    /// Parameters built from each declaration's `example` (falling back to
    /// defaults): what `new` scaffolds and what tests expand.
    pub fn example_params(&self) -> Value {
        let mut out = Map::new();
        for (k, p) in self.params() {
            if let Some(e) = p.get("example") {
                out.insert(k, e.clone());
            }
        }
        Value::Object(out)
    }

    /// JSON Schema for the parameters (draft 2020-12 subset).
    pub fn param_schema(&self) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for (k, p) in self.params() {
            if p.get("required") == Some(&Value::Bool(true)) {
                required.push(Value::String(k.clone()));
            }
            props.insert(k, param_to_schema(&p));
        }
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": self.doc.get("title"),
            "description": self.doc.get("description"),
            "type": "object",
            "properties": props,
            "required": required,
            "additionalProperties": false,
        })
    }

    /// Check and complete parameters: unknown keys, types, required, limits, defaults.
    pub fn resolve_params(&self, given: &Value, brand: &Value, fmt: &Value) -> Result<Map<String, Value>, Vec<String>> {
        let decl = self.params();
        let empty = Map::new();
        let given = match given {
            Value::Object(m) => m,
            Value::Null => &empty,
            _ => return Err(vec!["params must be a JSON object".into()]),
        };
        let mut errs = Vec::new();
        for k in given.keys() {
            if !decl.contains_key(k) {
                let hint = suggest(k, decl.keys().map(|s| s.as_str())).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
                errs.push(format!("unknown parameter '{k}'{hint}; this template takes: {}", decl.keys().cloned().collect::<Vec<_>>().join(", ")));
            }
        }
        let mut out = Map::new();
        for (k, p) in &decl {
            match given.get(k) {
                Some(v) if !v.is_null() => {
                    if let Err(e) = check_value(k, p, v) {
                        errs.extend(e);
                    }
                    out.insert(k.clone(), v.clone());
                }
                _ => {
                    if p.get("required") == Some(&Value::Bool(true)) {
                        let ex = p.get("example").map(|e| format!(", e.g. {e}")).unwrap_or_default();
                        let d = p.get("description").and_then(|d| d.as_str()).unwrap_or("");
                        errs.push(format!("missing required parameter '{k}': {d}{ex}"));
                        continue;
                    }
                    // Defaults may reference earlier params, brand and fmt.
                    let def = p.get("default").cloned().unwrap_or(Value::Null);
                    let extra = json!({"brand": brand, "fmt": fmt}).as_object().cloned().unwrap();
                    let mut sc = Scope::new();
                    sc.push(&extra);
                    sc.push_owned(out.clone());
                    match expand_value(&def, &mut sc, &Map::new(), &format!("params.{k}.default")) {
                        Ok(Some(v)) => {
                            out.insert(k.clone(), v);
                        }
                        Ok(None) => {
                            out.insert(k.clone(), Value::Null);
                        }
                        Err(e) => errs.push(e),
                    }
                }
            }
        }
        if errs.is_empty() {
            Ok(out)
        } else {
            Err(errs)
        }
    }

    /// Expand with parameters, a brand kit and a format into a spec.
    pub fn expand(&self, params: &Value, brand: &brand::Brand, format: Format) -> Result<Value, Vec<String>> {
        let b = brand.resolve();
        let fmt = format.scope();
        let p = self.resolve_params(params, &b, &fmt)?;
        let globals = json!({"brand": b, "fmt": fmt, "template": self.name}).as_object().cloned().unwrap();
        let common: Value = serde_json::from_str(COMMON).expect("templates/_common.json is valid JSON");
        let mut defs = common.get("defs").and_then(|d| d.as_object()).cloned().unwrap_or_default();
        if let Some(own) = self.doc.get("defs").and_then(|d| d.as_object()) {
            defs.extend(own.clone());
        }
        let mut sc = Scope::new();
        sc.push(&globals);
        sc.push(&p);
        let spec = self.doc.get("spec").unwrap();
        expand_value(spec, &mut sc, &defs, "spec").map(|v| v.unwrap_or(Value::Null)).map_err(|e| vec![format!("template '{}': {e}", self.name)])
    }
}

fn param_to_schema(p: &Value) -> Value {
    let mut s = Map::new();
    let ty = p.get("type").and_then(|t| t.as_str()).unwrap_or("string");
    let (jt, fmt) = match ty {
        "string" | "text" | "url" => ("string", None),
        "color" => ("string", Some("color")),
        "image" => ("string", Some("image-path")),
        "number" | "seconds" => ("number", None),
        "integer" => ("integer", None),
        "boolean" => ("boolean", None),
        "enum" => ("string", None),
        "array" => ("array", None),
        "object" => ("object", None),
        _ => ("string", None),
    };
    s.insert("type".into(), json!(jt));
    if let Some(f) = fmt {
        s.insert("format".into(), json!(f));
    }
    for k in ["description", "default", "example", "minimum", "maximum", "minItems", "maxItems", "maxLength"] {
        if let Some(v) = p.get(k) {
            s.insert(k.into(), v.clone());
        }
    }
    if let Some(v) = p.get("maxChars") {
        s.insert("maxLength".into(), v.clone());
    }
    if let Some(v) = p.get("maxWords") {
        s.insert("x-maxWords".into(), v.clone());
    }
    if let Some(v) = p.get("values") {
        s.insert("enum".into(), v.clone());
    }
    if let Some(items) = p.get("items") {
        s.insert("items".into(), param_to_schema(items));
    }
    if let Some(props) = p.get("properties").and_then(|x| x.as_object()) {
        let mut ps = Map::new();
        let mut req = Vec::new();
        for (k, v) in props {
            if v.get("required") == Some(&Value::Bool(true)) {
                req.push(json!(k));
            }
            ps.insert(k.clone(), param_to_schema(v));
        }
        s.insert("properties".into(), Value::Object(ps));
        s.insert("required".into(), Value::Array(req));
        s.insert("additionalProperties".into(), json!(false));
    }
    Value::Object(s)
}

/// Validate one parameter value against its declaration.
fn check_value(path: &str, p: &Value, v: &Value) -> Result<(), Vec<String>> {
    let ty = p.get("type").and_then(|t| t.as_str()).unwrap_or("string");
    let mut errs = Vec::new();
    let want = |what: &str| format!("{path}: expected {what}, got {}", short(v));
    match ty {
        "string" | "text" | "url" | "image" => {
            if let Some(s) = v.as_str() {
                let words = expr::word_count(s);
                if let Some(mw) = p.get("maxWords").and_then(|x| x.as_u64()) {
                    if words as u64 > mw {
                        errs.push(format!(
                            "{path}: {words} words is too long for this slot (max {mw}); viewers read about 3 words per second, so cut it to the essential phrase"
                        ));
                    }
                }
                if let Some(mc) = p.get("maxChars").and_then(|x| x.as_u64()) {
                    if s.chars().count() as u64 > mc {
                        errs.push(format!("{path}: {} characters is too long (max {mc}); shorten it so it fits on screen", s.chars().count()));
                    }
                }
            } else {
                errs.push(want("a string"));
            }
        }
        "color" => match v.as_str().map(color::parse) {
            Some(Ok(_)) => {}
            Some(Err(e)) => errs.push(format!("{path}: {e}")),
            None => errs.push(want("a colour string like \"#6E56CF\"")),
        },
        "number" | "seconds" | "integer" => match v.as_f64() {
            Some(n) => {
                if ty == "integer" && n.fract() != 0.0 {
                    errs.push(want("a whole number"));
                }
                if let Some(lo) = p.get("minimum").and_then(|x| x.as_f64()) {
                    if n < lo {
                        errs.push(format!("{path}: {n} is below the minimum {lo}"));
                    }
                }
                if let Some(hi) = p.get("maximum").and_then(|x| x.as_f64()) {
                    if n > hi {
                        errs.push(format!("{path}: {n} is above the maximum {hi}"));
                    }
                }
            }
            None => errs.push(want("a number")),
        },
        "boolean" => {
            if !v.is_boolean() {
                errs.push(want("true or false"));
            }
        }
        "enum" => {
            let vals: Vec<&str> = p.get("values").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
            match v.as_str() {
                Some(s) if vals.contains(&s) => {}
                Some(s) => {
                    let hint = suggest(s, vals.iter().copied()).map(|x| format!(" (did you mean '{x}'?)")).unwrap_or_default();
                    errs.push(format!("{path}: '{s}' is not one of {}{hint}", vals.join(", ")));
                }
                None => errs.push(want(&format!("one of {}", vals.join(", ")))),
            }
        }
        "array" => match v.as_array() {
            Some(a) => {
                if let Some(n) = p.get("minItems").and_then(|x| x.as_u64()) {
                    if (a.len() as u64) < n {
                        errs.push(format!("{path}: needs at least {n} items, got {}", a.len()));
                    }
                }
                if let Some(n) = p.get("maxItems").and_then(|x| x.as_u64()) {
                    if a.len() as u64 > n {
                        errs.push(format!("{path}: at most {n} items fit this template, got {}; keep the strongest {n}", a.len()));
                    }
                }
                if let Some(items) = p.get("items") {
                    for (i, x) in a.iter().enumerate() {
                        if let Err(e) = check_value(&format!("{path}[{i}]"), items, x) {
                            errs.extend(e);
                        }
                    }
                }
            }
            None => errs.push(want("a list")),
        },
        "object" => match v.as_object() {
            Some(o) => {
                let props = p.get("properties").and_then(|x| x.as_object()).cloned().unwrap_or_default();
                for k in o.keys() {
                    if !props.is_empty() && !props.contains_key(k) {
                        let hint = suggest(k, props.keys().map(|s| s.as_str())).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
                        errs.push(format!("{path}: unknown field '{k}'{hint}; fields: {}", props.keys().cloned().collect::<Vec<_>>().join(", ")));
                    }
                }
                for (k, pp) in &props {
                    match o.get(k) {
                        Some(x) if !x.is_null() => {
                            if let Err(e) = check_value(&format!("{path}.{k}"), pp, x) {
                                errs.extend(e);
                            }
                        }
                        _ if pp.get("required") == Some(&Value::Bool(true)) => errs.push(format!("{path}: missing field '{k}'")),
                        _ => {}
                    }
                }
            }
            None => errs.push(want("an object")),
        },
        _ => {}
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 40 {
        format!("{}…", s.chars().take(39).collect::<String>())
    } else {
        s
    }
}

/// Expand a template value. `None` means "omit this element / key".
pub fn expand_value(v: &Value, sc: &mut Scope, defs: &Map<String, Value>, path: &str) -> Result<Option<Value>, String> {
    match v {
        Value::String(s) => expand_str(s, sc).map(Some).map_err(|e| format!("{path}: {e}")),
        Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for (i, x) in a.iter().enumerate() {
                let p = format!("{path}[{i}]");
                // Directives that yield a list splice into the parent list.
                let splice = x.as_object().is_some_and(|o| o.keys().any(|k| DIRECTIVES.contains(&k.as_str())));
                match expand_value(x, sc, defs, &p)? {
                    Some(Value::Array(items)) if splice => out.extend(items),
                    Some(y) => out.push(y),
                    None => {}
                }
            }
            Ok(Some(Value::Array(out)))
        }
        Value::Object(m) => {
            if let Some(src) = m.get("$each") {
                return expand_each(m, src, sc, defs, path);
            }
            if let Some(c) = m.get("$if") {
                let cond = match c {
                    // An undefined name is false, so defs can test optional arguments.
                    Value::String(s) => match eval_cond(s, sc) {
                        Ok(v) => expr::truthy(&v),
                        Err(e) if e.starts_with("unknown name") => false,
                        Err(e) => return Err(format!("{path}.$if: {e}")),
                    },
                    other => expr::truthy(other),
                };
                let branch = if cond { m.get("then") } else { m.get("else") };
                return match branch {
                    Some(b) => expand_value(b, sc, defs, path),
                    None => Ok(None),
                };
            }
            if let Some(Value::Object(binds)) = m.get("$let") {
                let mut frame = Map::new();
                for (k, b) in binds {
                    // Each binding sees the previous ones.
                    let val = {
                        sc.push_owned(frame.clone());
                        let r = match b {
                            Value::String(s) if !s.contains("{{") => eval_cond(s, sc).map_err(|e| format!("{path}.$let.{k}: {e}")),
                            other => expand_value(other, sc, defs, &format!("{path}.$let.{k}")).map(|v| v.unwrap_or(Value::Null)),
                        };
                        sc.pop_owned();
                        r?
                    };
                    frame.insert(k.clone(), val);
                }
                let body = m.get("in").ok_or_else(|| format!("{path}: $let needs \"in\""))?;
                sc.push_owned(frame.clone());
                let r = expand_value(body, sc, defs, path);
                sc.pop_owned();
                return r;
            }
            if let Some(Value::Object(choices)) = m.get("$fmt") {
                let o = sc.get("fmt").cloned().unwrap_or(Value::Null);
                let name = o.get("name").and_then(|x| x.as_str()).unwrap_or("landscape");
                let aspect = o.get("aspect").and_then(|x| x.as_str()).unwrap_or("16:9");
                let alias = match name {
                    "portrait" => "9:16",
                    "square" => "1:1",
                    _ => "16:9",
                };
                let pick = choices
                    .get(aspect)
                    .or_else(|| choices.get(name))
                    .or_else(|| choices.get(alias))
                    .or_else(|| choices.get("default"))
                    .or_else(|| choices.get("landscape"))
                    .or_else(|| choices.get("16:9"));
                return match pick {
                    Some(b) => expand_value(b, sc, defs, path),
                    None => Err(format!("{path}: $fmt has no branch for {name} ({aspect}); add \"{name}\" or \"default\"")),
                };
            }
            if let Some(Value::String(name)) = m.get("$use") {
                let def = defs.get(name).ok_or_else(|| {
                    let hint = suggest(name, defs.keys().map(|s| s.as_str())).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
                    format!("{path}: unknown def '{name}'{hint}")
                })?;
                let mut frame = Map::new();
                if let Some(Value::Object(with)) = m.get("with") {
                    for (k, b) in with {
                        frame.insert(k.clone(), expand_value(b, sc, defs, &format!("{path}.with.{k}"))?.unwrap_or(Value::Null));
                    }
                }
                sc.push_owned(frame.clone());
                let r = expand_value(def, sc, defs, &format!("{path}<{name}>"));
                sc.pop_owned();
                // Extra keys next to $use/with override the def's keys.
                let r = r?;
                let extra: Vec<(&String, &Value)> = m.iter().filter(|(k, _)| *k != "$use" && *k != "with").collect();
                if let (Some(Value::Object(mut o)), false) = (r.clone(), extra.is_empty()) {
                    for (k, x) in extra {
                        if let Some(y) = expand_value(x, sc, defs, &format!("{path}.{k}"))? {
                            o.insert(k.clone(), y);
                        }
                    }
                    return Ok(Some(Value::Object(o)));
                }
                return Ok(r);
            }
            if let Some(Value::Array(parts)) = m.get("$merge") {
                let mut o = Map::new();
                for (i, part) in parts.iter().enumerate() {
                    match expand_value(part, sc, defs, &format!("{path}.$merge[{i}]"))? {
                        Some(Value::Object(x)) => o.extend(x),
                        None | Some(Value::Null) => {}
                        Some(other) => return Err(format!("{path}.$merge[{i}]: expected an object, got {}", short(&other))),
                    }
                }
                return Ok(Some(Value::Object(o)));
            }
            if let Some(k) = m.keys().find(|k| k.starts_with('$')) {
                let hint = suggest(k, DIRECTIVES.iter().copied()).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
                return Err(format!("{path}: unknown directive '{k}'{hint}"));
            }
            let mut o = Map::new();
            for (k, x) in m {
                if let Some(y) = expand_value(x, sc, defs, &format!("{path}.{k}"))? {
                    o.insert(k.clone(), y);
                }
            }
            Ok(Some(Value::Object(o)))
        }
        other => Ok(Some(other.clone())),
    }
}

/// A `$if` / `$each` source: a bare expression or a `{{ }}` string.
fn eval_cond(s: &str, sc: &Scope) -> Result<Value, String> {
    let t = s.trim();
    if let Some(inner) = t.strip_prefix("{{").and_then(|x| x.strip_suffix("}}")) {
        if !inner.contains("{{") {
            return expr::eval(inner, sc);
        }
    }
    expr::eval(t, sc)
}

fn expand_each(m: &Map<String, Value>, src: &Value, sc: &mut Scope, defs: &Map<String, Value>, path: &str) -> Result<Option<Value>, String> {
    let list = match src {
        Value::String(s) => eval_cond(s, sc).map_err(|e| format!("{path}.$each: {e}"))?,
        other => other.clone(),
    };
    let items = match list {
        Value::Array(a) => a,
        Value::Null => vec![],
        Value::Number(n) => (0..n.as_u64().unwrap_or(0)).map(Value::from).collect(),
        other => return Err(format!("{path}.$each: expected a list, got {}", short(&other))),
    };
    let as_name = m.get("as").and_then(|x| x.as_str()).unwrap_or("item").to_string();
    let idx_name = m.get("index").and_then(|x| x.as_str()).unwrap_or("i").to_string();
    let body = m.get("do").ok_or_else(|| format!("{path}: $each needs \"do\""))?;
    let n = items.len();
    let mut out = Vec::new();
    for (i, it) in items.into_iter().enumerate() {
        let mut frame = Map::new();
        frame.insert(as_name.clone(), it);
        frame.insert(idx_name.clone(), Value::from(i));
        frame.insert("count".into(), Value::from(n));
        frame.insert("first".into(), Value::Bool(i == 0));
        frame.insert("last".into(), Value::Bool(i + 1 == n));
        sc.push_owned(frame.clone());
        let r = expand_value(body, sc, defs, &format!("{path}[{as_name}={i}]"));
        sc.pop_owned();
        let splice = body.is_array() || body.as_object().is_some_and(|o| o.keys().any(|k| DIRECTIVES.contains(&k.as_str())));
        match (r?, splice) {
            (Some(Value::Array(v)), true) => out.extend(v),
            (Some(v), _) => out.push(v),
            (None, _) => {}
        }
    }
    Ok(Some(Value::Array(out)))
}

fn expand_str(s: &str, sc: &Scope) -> Result<Value, String> {
    if !s.contains("{{") {
        return Ok(Value::String(s.to_string()));
    }
    let t = s.trim();
    if t.starts_with("{{") && t.ends_with("}}") && t[2..].find("{{").is_none() && t[..t.len() - 2].rfind("}}").is_none() {
        return expr::eval(&t[2..t.len() - 2], sc);
    }
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find("{{") {
        out.push_str(&rest[..a]);
        let after = &rest[a + 2..];
        let b = after.find("}}").ok_or_else(|| format!("unclosed '{{{{' in \"{s}\""))?;
        out.push_str(&expr::to_text(&expr::eval(&after[..b], sc)?));
        rest = &after[b + 2..];
    }
    out.push_str(rest);
    Ok(Value::String(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tpl(spec: Value, params: Value) -> Template {
        Template::from_json(&json!({"name": "t", "params": params, "spec": spec}).to_string()).unwrap()
    }

    #[test]
    fn expands_each_if_fmt_let_use() {
        let t = Template::from_json(
            &json!({
                "name": "t",
                "params": {
                    "title": {"type": "string", "required": true, "maxWords": 4},
                    "items": {"type": "array", "default": ["a", "b c"], "items": {"type": "string"}},
                    "logo": {"type": "image"},
                    "cta": {"type": "string", "default": "{{brand.voice.cta}}"}
                },
                "defs": {"chip": {"text": "{{label}}", "x": "{{i * 100}}"}},
                "spec": {
                    "size": ["{{fmt.w}}", "{{fmt.h}}"],
                    "scenes": [
                        {"text": "Meet {{title}}!", "size": {"$fmt": {"portrait": 120, "default": 96}}},
                        {"$each": "items", "as": "it", "do": {"$use": "chip", "with": {"label": "{{upper(it)}}"}, "d": "{{readtime(it)}}"}},
                        {"$if": "logo", "then": {"image": "{{logo}}"}},
                        {"$let": {"n": "len(items)"}, "in": {"count": "{{n}}", "cta": "{{cta}}"}}
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();
        let out = t.expand(&json!({"title": "Anikit"}), &brand::Brand::default(), Format::parse("9:16").unwrap()).unwrap();
        assert_eq!(out["size"], json!([1080, 1920]));
        let sc = out["scenes"].as_array().unwrap();
        assert_eq!(sc.len(), 4, "{out}");
        assert_eq!(sc[0], json!({"text": "Meet Anikit!", "size": 120}));
        assert_eq!(sc[1], json!({"text": "A", "x": 0, "d": 1.5}));
        assert_eq!(sc[2]["text"], "B C");
        assert_eq!(sc[2]["x"], 100);
        assert_eq!(sc[3], json!({"count": 2, "cta": "Try it today"}));
    }

    #[test]
    fn param_errors_are_actionable() {
        let t = tpl(json!({}), json!({"product": {"type": "string", "required": true, "maxWords": 2, "example": "Anikit"}, "mode": {"type": "enum", "values": ["dark", "light"]}}));
        let f = Format::parse("16:9").unwrap();
        let e = t.expand(&json!({"prodcut": "x"}), &brand::Brand::default(), f).unwrap_err();
        assert!(e.iter().any(|m| m.contains("did you mean 'product'")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("missing required parameter 'product'") && m.contains("Anikit")), "{e:?}");
        let e = t.expand(&json!({"product": "far too many words", "mode": "drak"}), &brand::Brand::default(), f).unwrap_err();
        assert!(e.iter().any(|m| m.contains("4 words is too long")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("did you mean 'dark'")), "{e:?}");
    }

    #[test]
    fn builtin_templates_expand_in_every_format() {
        let brands = [
            brand::Brand::default(),
            brand::Brand::from_json(r##"{"name":"Lumen","theme":"light","colors":{"primary":"#0F766E"},"logo":"logo.svg","voice":{"url":"lumen.app"}}"##).unwrap(),
        ];
        for (name, _) in BUILTIN {
            let t = Template::builtin(name).unwrap();
            let params = t.example_params();
            for b in &brands {
                for f in ["16:9", "9:16", "1:1"] {
                    let spec = t.expand(&params, b, Format::parse(f).unwrap()).unwrap_or_else(|e| panic!("{name} {f}: {e:?}"));
                    let text = spec.to_string();
                    assert!(!text.contains("{{"), "{name} {f}: unexpanded expression in {text}");
                    for d in DIRECTIVES {
                        assert!(!text.contains(&format!("\"{d}\"")), "{name} {f}: unexpanded {d} in {text}");
                    }
                    assert!(spec["scenes"].as_array().is_some_and(|s| !s.is_empty()), "{name} {f}: no scenes");
                }
            }
        }
    }

    /// Expanded templates must pass the spec parser and compiler with no
    /// diagnostics at all (missing example assets aside).
    #[test]
    fn builtin_templates_are_valid_specs() {
        use crate::eval::{Evaluator, Options};
        let mut bad = Vec::new();
        for (name, _) in BUILTIN {
            let t = Template::builtin(name).unwrap();
            for f in ["16:9", "9:16", "1:1"] {
                let b = brand::Brand::from_json(r##"{"theme":"light","colors":{"primary":"#0F766E"},"logo":"logo.svg"}"##).unwrap();
                for brand in [brand::Brand::default(), b] {
                    let spec = t.expand(&t.example_params(), &brand, Format::parse(f).unwrap()).unwrap();
                    let diags = match crate::spec::parse::from_value(spec) {
                        Err(d) => d,
                        Ok(p) => {
                            let (_, mut d) = Evaluator::build(&p.spec, &Options { load_assets: false, ..Options::default() });
                            d.extend(p.warnings);
                            d
                        }
                    };
                    bad.extend(diags.iter().filter(|d| !d.message.contains("file not found")).map(|d| format!("{name} {f}: {d}")));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn formats() {
        assert_eq!(Format::parse("9:16").unwrap().orientation(), "portrait");
        assert_eq!(Format::parse("1:1").unwrap().orientation(), "square");
        assert_eq!(Format::parse("1280x720").unwrap().aspect(), "16:9");
        assert!(Format::parse("16x9").unwrap_err().contains("WIDTHxHEIGHT"));
    }

    #[test]
    fn suggestions() {
        assert_eq!(suggest("prodcut", ["product", "tagline"].into_iter()), Some("product"));
        assert_eq!(suggest("zzz", ["product"].into_iter()), None);
    }
}
