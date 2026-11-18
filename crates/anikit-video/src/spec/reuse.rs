//! Spec reuse: a pure JSON pre-pass that runs before serde typing.
//!
//! - `"defs": { "name": <scene or layer JSON> }` at the top level.
//! - Wherever a scene or layer is expected, `{ "use": "name", ...overrides }`
//!   deep-merges the overrides onto the def. Objects merge key by key; arrays
//!   and scalars replace.
//! - `"with": { "param": value }` substitutes `"$param"` inside the def. A
//!   string that is exactly `"$param"` takes the value's JSON type; inside
//!   longer strings it is interpolated. Names not bound by `with`, `$i` or
//!   `$n` are left alone, so theme tokens like `"$accent"` pass through.
//! - `"repeat": N` on a scene or layer emits N copies, with `"$i"` (0-based)
//!   and `"$n"` (= N) bound.
//! - In `scenes`, an item `{ "scenes": [...] }` is a sequence spliced in place
//!   (combine with `repeat` to loop several scenes).
//! - A string starting with `=` is evaluated as arithmetic after substitution:
//!   `"x": "=160 + $i * 420"`.
//!
//! Errors carry a path and a "did you mean" for unknown defs. Cycles between
//! defs are reported instead of recursing forever.

use serde_json::{Map, Value};

use super::parse::Diagnostic;

const MAX_REPEAT: u64 = 1000;

/// Expand `defs`/`use`/`with`/`repeat` in a raw spec value.
pub fn expand(mut v: Value) -> Result<Value, Vec<Diagnostic>> {
    // A top-level `script` runs first, on the whole spec (it may generate defs too).
    if let Some(code) = v.as_object_mut().and_then(|m| m.remove("script")) {
        v = super::script::top(&code, v).map_err(|d| vec![d])?;
    }
    let Some(root) = v.as_object_mut() else { return Ok(v) };
    let defs = match root.remove("defs") {
        None => Map::new(),
        Some(Value::Object(m)) => m,
        Some(_) => return Err(vec![Diagnostic::error("defs", "'defs' must be an object of named scenes or layers")]),
    };
    let quick = !defs.is_empty() || contains_reuse_keys(root);
    if !quick {
        return Ok(v);
    }
    let size = frame_size(root.get("size"));
    let mut cx = Cx { defs, errors: Vec::new(), stack: Vec::new(), size };
    if let Some(scenes) = root.get_mut("scenes") {
        let items = std::mem::take(scenes);
        *scenes = cx.items(items, "scenes", &Map::new());
    }
    if let Some(Value::Object(comps)) = root.get_mut("compositions") {
        for (name, c) in comps.iter_mut() {
            for key in ["layers", "scenes"] {
                if let Some(list) = c.get_mut(key) {
                    let items = std::mem::take(list);
                    *list = cx.items(items, &format!("compositions.{name}.{key}"), &Map::new());
                }
            }
        }
    }
    // `{ "script": … }` anywhere else in an array (scene3d objects, draw items, emitters,
    // keys, notes…): a returned list splices, an object replaces.
    let size = cx.size;
    splice_scripts(&mut v, "", size, &mut cx.errors);
    stack_scenes(&mut v);
    if cx.errors.is_empty() {
        Ok(v)
    } else {
        Err(cx.errors)
    }
}

/// `"stack": true` (or `{ "gap": 24, "align": "start" }`) on a scene: its free-floating
/// layers (no `x`/`y`, not full-frame components) go into one centred column box, in
/// order, instead of piling up at the centre.
pub fn stack_scenes(v: &mut Value) {
    let Some(scenes) = v.get_mut("scenes").and_then(|s| s.as_array_mut()) else { return };
    for sc in scenes {
        let Some(obj) = sc.as_object_mut() else { continue };
        let Some(stack) = obj.remove("stack") else { continue };
        let (gap, align) = match &stack {
            Value::Bool(false) => continue,
            Value::Object(m) => (m.get("gap").cloned().unwrap_or(Value::from(24)), m.get("align").cloned().unwrap_or(Value::from("center"))),
            Value::Number(n) => (Value::Number(n.clone()), Value::from("center")),
            _ => (Value::from(24), Value::from("center")),
        };
        let Some(Value::Array(layers)) = obj.get_mut("layers") else { continue };
        let free = |l: &Value| match l {
            Value::String(_) => true,
            Value::Object(m) => !m.contains_key("x") && !m.contains_key("y") && (m.contains_key("text") || m.contains_key("shape") || m.contains_key("src")),
            _ => false,
        };
        let first = layers.iter().position(free);
        let Some(first) = first else { continue };
        let mut kids = Vec::new();
        let mut rest = Vec::new();
        for (k, l) in std::mem::take(layers).into_iter().enumerate() {
            if free(&l) {
                kids.push(l);
            } else {
                rest.push((k, l));
            }
        }
        let boxed = serde_json::json!({ "id": "stack", "children": kids, "style": { "direction": "column", "gap": gap, "align": align } });
        let mut out: Vec<Value> = Vec::new();
        let mut placed = false;
        for (k, l) in rest {
            if !placed && k > first {
                out.push(boxed.clone());
                placed = true;
            }
            out.push(l);
        }
        if !placed {
            out.push(boxed);
        }
        *layers = out;
    }
}

fn splice_scripts(v: &mut Value, path: &str, size: (u64, u64), errors: &mut Vec<Diagnostic>) {
    match v {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (k, mut item) in std::mem::take(items).into_iter().enumerate() {
                let p = format!("{path}[{k}]");
                let code = match &mut item {
                    Value::Object(m) if m.contains_key("script") && m.len() == 1 => m.remove("script"),
                    _ => None,
                };
                match code {
                    Some(code) => {
                        let mut vars = Map::new();
                        vars.insert("w".into(), size.0.into());
                        vars.insert("h".into(), size.1.into());
                        match super::script::items(&code, &vars, &p) {
                            Ok(gen) => {
                                let mut gen = Value::Array(gen);
                                splice_scripts(&mut gen, &p, size, errors);
                                if let Value::Array(g) = gen {
                                    out.extend(g);
                                }
                            }
                            Err(d) => errors.push(d),
                        }
                    }
                    None => {
                        splice_scripts(&mut item, &p, size, errors);
                        out.push(item);
                    }
                }
            }
            *items = out;
        }
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                splice_scripts(x, &p, size, errors);
            }
        }
        _ => {}
    }
}

fn contains_reuse_keys(v: &Map<String, Value>) -> bool {
    fn walk(v: &Value) -> bool {
        match v {
            Value::Object(m) => m.contains_key("use") || m.contains_key("repeat") || m.contains_key("script") || m.values().any(walk),
            Value::Array(a) => a.iter().any(walk),
            _ => false,
        }
    }
    v.values().any(walk) || v.get("scenes").and_then(|s| s.as_array()).is_some_and(|s| s.iter().any(|x| x.get("stack").is_some()))
}

/// Frame size for scripts (`w`, `h`): `[w, h]` or a common preset; default 1920×1080.
fn frame_size(v: Option<&Value>) -> (u64, u64) {
    match v {
        Some(Value::Array(a)) if a.len() == 2 => (a[0].as_u64().unwrap_or(1920), a[1].as_u64().unwrap_or(1080)),
        Some(Value::String(s)) => match s.as_str() {
            "9:16" | "vertical" | "story" | "reels" | "tiktok" | "shorts" => (1080, 1920),
            "1:1" | "square" => (1080, 1080),
            "4:5" | "portrait" => (1080, 1350),
            "720p" | "hd" => (1280, 720),
            "1440p" | "2k" | "qhd" => (2560, 1440),
            "4k" | "2160p" | "uhd" => (3840, 2160),
            _ => (1920, 1080),
        },
        _ => (1920, 1080),
    }
}

struct Cx {
    size: (u64, u64),
    defs: Map<String, Value>,
    errors: Vec<Diagnostic>,
    stack: Vec<String>,
}

impl Cx {
    /// Expand a list of scenes or layers (`repeat` multiplies items).
    fn items(&mut self, list: Value, path: &str, params: &Map<String, Value>) -> Value {
        let Value::Array(items) = list else { return list };
        let mut out = Vec::with_capacity(items.len());
        for (k, item) in items.into_iter().enumerate() {
            let p = format!("{path}[{k}]");
            let Value::Object(mut obj) = item else {
                out.push(item);
                continue;
            };
            // `{ "script": … }`: replaced by the items it returns (expanded in turn).
            if let Some(code) = obj.remove("script") {
                let mut vars = params.clone();
                vars.insert("w".into(), self.size.0.into());
                vars.insert("h".into(), self.size.1.into());
                match super::script::items(&code, &vars, &p) {
                    Ok(gen) => {
                        if let Value::Array(v) = self.items(Value::Array(gen), path, params) {
                            out.extend(v);
                        }
                    }
                    Err(d) => self.errors.push(d),
                }
                continue;
            }
            // `"repeat": { "count": … }` on a layer is the render-time repeater: leave it.
            let repeater = !path.ends_with("scenes") && obj.get("repeat").is_some_and(Value::is_object);
            let repeat = match if repeater { None } else { obj.remove("repeat") } {
                None => None,
                Some(r) => match subst(r, params).as_u64() {
                    Some(n) if n <= MAX_REPEAT => Some(n),
                    _ => {
                        self.errors.push(Diagnostic::error(format!("{p}.repeat"), format!("'repeat' must be a whole number from 0 to {MAX_REPEAT}")));
                        continue;
                    }
                },
            };
            // A sequence: `{ "scenes": [...], "repeat": N }` inside a scene list is spliced in place.
            let seq = path.ends_with("scenes") && obj.contains_key("scenes") && !obj.contains_key("layers");
            let n = repeat.unwrap_or(1);
            for i in 0..n {
                let mut ps = params.clone();
                if repeat.is_some() {
                    ps.insert("i".into(), Value::from(i));
                    ps.insert("n".into(), Value::from(n));
                }
                if seq {
                    let inner = obj.get("scenes").cloned().unwrap_or(Value::Null);
                    if let Value::Array(v) = self.items(inner, &format!("{p}.scenes"), &ps) {
                        out.extend(v);
                    }
                } else {
                    out.push(self.node(Value::Object(obj.clone()), &p, &ps));
                }
            }
        }
        Value::Array(out)
    }

    /// Resolve `use` on one scene/layer, then expand its child lists.
    fn node(&mut self, v: Value, path: &str, params: &Map<String, Value>) -> Value {
        let v = subst(v, params);
        let mut v = self.resolve(v, path);
        if let Value::Object(m) = &mut v {
            for key in ["layers", "children"] {
                if let Some(list) = m.get_mut(key) {
                    let items = std::mem::take(list);
                    *list = self.items(items, &format!("{path}.{key}"), params);
                }
            }
        }
        v
    }

    fn resolve(&mut self, v: Value, path: &str) -> Value {
        let Value::Object(mut obj) = v else { return v };
        let Some(name) = obj.remove("use") else { return Value::Object(obj) };
        let Some(name) = name.as_str().map(str::to_string) else {
            self.errors.push(Diagnostic::error(format!("{path}.use"), "'use' must be the name of an entry in 'defs'"));
            return Value::Object(obj);
        };
        let Some(def) = self.defs.get(&name).cloned() else {
            let names: Vec<&str> = self.defs.keys().map(String::as_str).collect();
            let msg = if names.is_empty() {
                format!("unknown def '{name}' (there is no top-level 'defs')")
            } else {
                format!("unknown def '{name}' (defs: {})", names.join(", "))
            };
            self.errors.push(Diagnostic::error(format!("{path}.use"), msg).suggest(closest(&name, &names)));
            return Value::Object(obj);
        };
        if self.stack.contains(&name) {
            let mut chain = self.stack.clone();
            chain.push(name.clone());
            self.errors.push(Diagnostic::error(format!("{path}.use"), format!("defs refer to each other in a cycle: {}", chain.join(" -> "))));
            return Value::Object(obj);
        }
        let mut with = match obj.remove("with") {
            None => Map::new(),
            Some(Value::Object(m)) => m,
            Some(_) => {
                self.errors.push(Diagnostic::error(format!("{path}.with"), "'with' must be an object of parameter values"));
                Map::new()
            }
        };
        // A def's `"params": { "size": 34, … }` declares its parameters with defaults; at
        // the use site they can be given directly (`{ "use": "label", "size": 40 }`).
        let mut def = def;
        if let Some(Value::Object(params)) = def.as_object_mut().and_then(|d| d.remove("params")) {
            for (k, default) in params {
                let v = obj.remove(&k).or_else(|| with.remove(&k)).unwrap_or(default);
                with.insert(k, v);
            }
        }
        self.stack.push(name.clone());
        // The def may itself `use` another def.
        let base = self.resolve(subst(def, &with), &format!("defs.{name}"));
        self.stack.pop();
        merge(base, Value::Object(obj))
    }
}

/// Deep merge: objects key by key, everything else replaced by `over`.
fn merge(base: Value, over: Value) -> Value {
    match (base, over) {
        (Value::Object(mut b), Value::Object(o)) => {
            for (k, v) in o {
                let merged = match b.remove(&k) {
                    Some(bv) => merge(bv, v),
                    None => v,
                };
                b.insert(k, merged);
            }
            Value::Object(b)
        }
        (_, o) => o,
    }
}

/// Substitute bound `$params` (and evaluate `=` arithmetic) throughout `v`.
pub(crate) fn subst(v: Value, params: &Map<String, Value>) -> Value {
    if params.is_empty() {
        return v;
    }
    match v {
        Value::String(s) => subst_str(s, params),
        Value::Array(a) => Value::Array(a.into_iter().map(|x| subst(x, params)).collect()),
        Value::Object(m) => {
            // A nested `with` binds its own names; don't substitute into a def it will receive.
            Value::Object(
                m.into_iter()
                    .map(|(k, x)| {
                        let v = subst(x, params);
                        // A number placed into `text` (e.g. `"=10 * $i"`) is meant as text.
                        let v = match v {
                            Value::Number(n) if k == "text" => Value::String(n.to_string()),
                            v => v,
                        };
                        (k, v)
                    })
                    .collect(),
            )
        }
        other => other,
    }
}

fn subst_str(s: String, params: &Map<String, Value>) -> Value {
    if let Some(name) = s.strip_prefix('$') {
        if let Some(v) = params.get(name) {
            return v.clone();
        }
    }
    if !s.contains('$') {
        return eval_if_expr(s);
    }
    // Interpolate, longest names first so `$item` wins over `$i`.
    let mut names: Vec<&String> = params.keys().collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    'outer: while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        for n in &names {
            if after.starts_with(n.as_str()) && !after[n.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                match &params[n.as_str()] {
                    Value::String(x) => out.push_str(x),
                    other => out.push_str(&other.to_string()),
                }
                rest = &after[n.len()..];
                continue 'outer;
            }
        }
        out.push('$');
        rest = after;
    }
    out.push_str(rest);
    eval_if_expr(out)
}

fn eval_if_expr(s: String) -> Value {
    if let Some(e) = s.strip_prefix('=') {
        if let Some(x) = Expr::new(e).parse() {
            if x.is_finite() {
                return serde_json::Number::from_f64(x).map_or(Value::String(s.clone()), |n| {
                    if x.fract() == 0.0 && x.abs() < 1e15 {
                        Value::from(x as i64)
                    } else {
                        Value::Number(n)
                    }
                });
            }
        }
    }
    Value::String(s)
}

/// Tiny arithmetic: numbers, `+ - * / %`, parentheses, unary minus.
struct Expr<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Expr<'a> {
    fn new(s: &'a str) -> Self {
        Expr { s: s.as_bytes(), i: 0 }
    }
    fn parse(mut self) -> Option<f64> {
        let v = self.sum()?;
        self.ws();
        (self.i == self.s.len()).then_some(v)
    }
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn sum(&mut self) -> Option<f64> {
        let mut v = self.prod()?;
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            let r = self.prod()?;
            v = if c == b'+' { v + r } else { v - r };
        }
        Some(v)
    }
    fn prod(&mut self) -> Option<f64> {
        let mut v = self.atom()?;
        while let Some(c @ (b'*' | b'/' | b'%')) = self.peek() {
            self.i += 1;
            let r = self.atom()?;
            v = match c {
                b'*' => v * r,
                b'/' => v / r,
                _ => v % r,
            };
        }
        Some(v)
    }
    fn atom(&mut self) -> Option<f64> {
        match self.peek()? {
            b'-' => {
                self.i += 1;
                Some(-self.atom()?)
            }
            b'(' => {
                self.i += 1;
                let v = self.sum()?;
                (self.peek()? == b')').then(|| self.i += 1)?;
                Some(v)
            }
            _ => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[st..self.i]).ok()?.parse().ok()
            }
        }
    }
}

fn closest(name: &str, names: &[&str]) -> Option<String> {
    let lev = |a: &str, b: &str| -> usize {
        let a: Vec<char> = a.to_lowercase().chars().collect();
        let b: Vec<char> = b.to_lowercase().chars().collect();
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.iter().enumerate() {
            let mut cur = vec![i + 1];
            for (j, cb) in b.iter().enumerate() {
                cur.push((prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1));
            }
            prev = cur;
        }
        prev[b.len()]
    };
    names
        .iter()
        .map(|n| (lev(name, n), *n))
        .filter(|(d, n)| *d <= 2.max(n.len() / 3))
        .min_by_key(|x| x.0)
        .map(|x| x.1.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn use_merge_with_and_repeat() {
        let v = json!({
            "defs": {
                "card": { "type": "box", "w": 400, "fill": "$accent", "children": [ { "text": "$title", "y": "$y" } ] },
                "hero": { "use": "card", "with": { "title": "Hero", "y": 10 }, "w": 800 }
            },
            "scenes": [
                { "layers": [
                    { "use": "card", "with": { "title": "One", "y": 40 }, "x": 100 },
                    { "use": "hero", "fill": "#fff" },
                    { "use": "card", "repeat": 3, "with": { "title": "Item $i of $n", "y": "=$i * 100" }, "x": "=160 + $i * 420" }
                ] },
                { "repeat": 2, "duration": 2, "layers": [ "Scene $i" ] }
            ]
        });
        let out = expand(v).unwrap();
        let l = &out["scenes"][0]["layers"];
        assert_eq!(l.as_array().unwrap().len(), 5);
        assert_eq!(l[0], json!({ "type": "box", "w": 400, "fill": "$accent", "x": 100, "children": [ { "text": "One", "y": 40 } ] }));
        assert_eq!(l[1]["w"], 800);
        assert_eq!(l[1]["fill"], "#fff");
        assert_eq!(l[1]["children"][0]["text"], "Hero");
        assert_eq!(l[4]["x"], 1000);
        assert_eq!(l[4]["children"][0], json!({ "text": "Item 2 of 3", "y": 200 }));
        assert_eq!(out["scenes"].as_array().unwrap().len(), 3);
        assert!(out.get("defs").is_none());
        // Sequences of scenes repeat as a block.
        let out = expand(json!({ "scenes": [ { "repeat": 2, "scenes": [ { "id": "a$i", "layers": [] }, { "id": "b", "layers": [] } ] } ] })).unwrap();
        let ids: Vec<&str> = out["scenes"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["a0", "b", "a1", "b"]);
    }

    #[test]
    fn errors_are_helpful() {
        let e = expand(json!({ "defs": { "card": {} }, "scenes": [ { "layers": [ { "use": "crad" } ] } ] })).unwrap_err();
        assert_eq!(e[0].path, "scenes[0].layers[0].use");
        assert_eq!(e[0].suggestion.as_deref(), Some("card"));
        let e = expand(json!({ "defs": { "a": { "use": "b" }, "b": { "use": "a" } }, "scenes": [ { "layers": [ { "use": "a" } ] } ] })).unwrap_err();
        assert!(e[0].message.contains("a -> b -> a"), "{}", e[0].message);
        let e = expand(json!({ "scenes": [ { "layers": [ { "text": "x", "repeat": "lots" } ] } ] })).unwrap_err();
        assert_eq!(e[0].path, "scenes[0].layers[0].repeat");
        // No reuse keys: untouched (tween `repeat` inside `animate` is not ours).
        let plain = json!({ "scenes": [ { "layers": [ { "text": "hi", "animate": { "preset": "pulse", "repeat": -1 } } ] } ] });
        assert_eq!(expand(plain.clone()).unwrap(), plain);
    }

    #[test]
    fn arithmetic() {
        assert_eq!(Expr::new("160 + 2 * 420").parse(), Some(1000.0));
        assert_eq!(Expr::new("-(1 + 2) / 2").parse(), Some(-1.5));
        assert_eq!(Expr::new("1 +").parse(), None);
    }
}
