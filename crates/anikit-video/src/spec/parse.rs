//! Parsing JSON / YAML into [`Spec`] with path-tagged diagnostics.

use super::Spec;
use serde::Serialize;
use std::fmt;

/// Error or warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One message about the spec, with a path like `scenes[2].layers[4].ease`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Diagnostic { severity: Severity::Error, path: path.into(), message: message.into(), suggestion: None }
    }
    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Diagnostic { severity: Severity::Warning, path: path.into(), message: message.into(), suggestion: None }
    }
    pub fn suggest(mut self, s: Option<impl Into<String>>) -> Self {
        self.suggestion = s.map(Into::into);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        if self.path.is_empty() {
            write!(f, "{sev}: {}", self.message)?;
        } else {
            write!(f, "{sev}: {}: {}", self.path, self.message)?;
        }
        if let Some(s) = &self.suggestion {
            write!(f, ", did you mean '{s}'?")?;
        }
        Ok(())
    }
}

/// A parsed spec plus the warnings found while parsing (unknown keys).
#[derive(Debug)]
pub struct Parsed {
    pub spec: Spec,
    pub warnings: Vec<Diagnostic>,
}

/// Parse JSON.
pub fn parse_json(src: &str) -> Result<Parsed, Vec<Diagnostic>> {
    let v: serde_json::Value = serde_json::from_str(src).map_err(|e| {
        vec![Diagnostic::error("", format!("invalid JSON at line {}, column {}: {}", e.line(), e.column(), strip_pos(&e.to_string())))]
    })?;
    let mut p = from_value(v)?;
    size_warning(src.len(), &mut p);
    Ok(p)
}

/// Specs past this size are almost always hand-unrolled loops; `defs`, `repeat` and `script` say it in a few lines.
const BIG_SPEC: usize = 1_000_000;

fn size_warning(len: usize, p: &mut Parsed) {
    if len > BIG_SPEC {
        p.warnings.push(Diagnostic::warning(
            "",
            format!(
                "this spec is {:.1} MB; big specs are slow to load and can run out of memory. Generate repeated shapes instead of writing them out: `repeat` with `$i` (and `rand($i, …)` for scatter), `defs` + `use` with params, a `script` that returns layers, or a `draw` layer for thousands of points",
                len as f64 / 1e6
            ),
        ));
    }
}

/// Forms agents write naturally, rewritten to the canonical ones before typing:
/// a `shader` map (`{"src": …, "uniforms": …}`), CSS `fontStyle`, and `transformOrigin`
/// (alias of `anchor`, with CSS values like `"50% 0%"` or `"top left"`).
fn natural_forms(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::Array(a) => a.iter_mut().for_each(natural_forms),
        Value::Object(m) => {
            if let Some(Value::Object(sh)) = m.get("shader").cloned() {
                if sh.keys().any(|k| ["src", "code", "source", "glsl", "wgsl", "uniforms", "channels"].contains(&k.as_str())) {
                    m.remove("shader");
                    for (k, x) in sh {
                        let k = match k.as_str() {
                            "code" | "source" | "glsl" | "wgsl" => "shader".to_string(),
                            _ => k,
                        };
                        m.entry(k).or_insert(x);
                    }
                }
            }
            if let Some(Value::String(fs)) = m.get("fontStyle").cloned() {
                let it = matches!(fs.trim(), "italic" | "oblique");
                if it || fs.trim() == "normal" {
                    m.remove("fontStyle");
                    m.entry("italic").or_insert(Value::Bool(it));
                }
            }
            if let Some(o) = m.remove("transformOrigin") {
                m.entry("anchor").or_insert(o);
            }
            if let Some(Value::String(a)) = m.get("anchor").cloned() {
                if let Some(x) = css_anchor(&a) {
                    m.insert("anchor".into(), x);
                }
            }
            m.values_mut().for_each(natural_forms);
        }
        _ => {}
    }
}

/// `"50% 0%"`, `"top left"`, `"left top"`, `"center bottom"` → a canonical anchor (None if already canonical or unknown).
fn css_anchor(s: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let pct = |p: &str| p.strip_suffix('%').and_then(|n| n.trim().parse::<f64>().ok()).map(|n| n / 100.0);
    if let (Some(x), Some(y)) = (pct(parts[0]), pct(parts[1])) {
        return Some(serde_json::json!([x, y]));
    }
    let (mut x, mut y) = (None, None);
    for p in &parts {
        match *p {
            "left" => x = Some(0.0),
            "right" => x = Some(1.0),
            "top" => y = Some(0.0),
            "bottom" => y = Some(1.0),
            "center" | "centre" => {}
            _ => return None,
        }
    }
    let (x, y) = (x.unwrap_or(0.5), y.unwrap_or(0.5));
    Some(serde_json::json!([x, y]))
}

/// Parse YAML.
pub fn parse_yaml(src: &str) -> Result<Parsed, Vec<Diagnostic>> {
    let v: serde_json::Value =
        serde_norway::from_str(src).map_err(|e| vec![Diagnostic::error("", format!("invalid YAML: {e}"))])?;
    let mut p = from_value(v)?;
    size_warning(src.len(), &mut p);
    Ok(p)
}

/// Parse JSON or YAML (JSON if it starts with `{`).
pub fn parse_str(src: &str) -> Result<Parsed, Vec<Diagnostic>> {
    if src.trim_start().starts_with('{') {
        parse_json(src)
    } else {
        parse_yaml(src)
    }
}

fn strip_pos(s: &str) -> String {
    match s.find(" at line ") {
        Some(i) => s[..i].to_string(),
        None => s.to_string(),
    }
}

/// Deserialize a JSON value, reporting unknown keys as warnings and type
/// errors with their path.
pub fn from_value(mut v: serde_json::Value) -> Result<Parsed, Vec<Diagnostic>> {
    bezier_eases(&mut v);
    let mut v = super::reuse::expand(v)?;
    natural_forms(&mut v);
    // `{"scene3d": {...}}` on a layer or a scene is shorthand for the scene3d component.
    let v = crate::scene3d::shorthand(v);
    let v = crate::looks::apply(v)?;
    let mut warnings = Vec::new();
    let mut track = serde_path_to_error::Track::new();
    let res = {
        let de = serde_path_to_error::Deserializer::new(&v, &mut track);
        serde_ignored::deserialize(de, |p| {
            let path = fmt_ignored(&p);
            let mut segs = Vec::new();
            segments(&p, &mut segs);
            let key = match segs.pop() {
                Some(Seg2::Key(k)) => k,
                _ => String::new(),
            };
            let parent: Vec<super::validate::Seg> = segs
                .iter()
                .map(|s| match s {
                    Seg2::Key(k) => super::validate::Seg::Key(k),
                    Seg2::Index => super::validate::Seg::Index,
                })
                .collect();
            let local = super::validate::keys_at(&parent);
            let sug = match &local {
                Some(keys) => super::validate::suggest(&key, keys),
                None => super::validate::suggest(&key, super::validate::all_keys()),
            };
            let elsewhere = local.as_ref().is_some_and(|k| !k.contains(&key)) && super::validate::all_keys().contains(&key);
            if sug.as_deref() == Some(key.as_str()) || (sug.is_none() && elsewhere) {
                warnings.push(Diagnostic::warning(path.clone(), format!("'{key}' is not valid here (ignored)")));
            } else {
                warnings.push(Diagnostic::warning(path.clone(), "unknown key (ignored)").suggest(sug));
            }
        })
    };
    match res {
        Ok(spec) => Ok(Parsed { spec, warnings }),
        Err(e) => {
            let path = fmt_track(&track.path());
            let msg = e.to_string();
            let mut d = Diagnostic::error(path, humanize(&msg));
            if msg.contains("missing field `layers`") {
                d.message = "missing 'layers' (every scene needs a list of layers)".into();
            } else if msg.contains("missing field `scenes`") {
                d.message = "missing 'scenes' (a video is a list of scenes, each with 'layers')".into();
            }
            let mut out = warnings;
            out.insert(0, d);
            Err(out)
        }
    }
}

fn humanize(msg: &str) -> String {
    let m = msg.replace("data did not match any variant of untagged enum ", "invalid value for ");
    match m.as_str() {
        s if s.starts_with("invalid value for Num") => "expected a number or a string like \"50%\", \"1.5s\" or \"=sin(t)\"".into(),
        s if s.starts_with("invalid value for Paint") => "expected a colour string (\"#fff\", \"$primary\") or a gradient".into(),
        s if s.starts_with("invalid value for Scale") => "expected a number or [sx, sy]".into(),
        s if s.starts_with("invalid value for Size") => "expected a preset like \"16:9\" or [width, height]".into(),
        s if s.starts_with("invalid value for Anchor") => "expected an anchor name (\"center\", \"topLeft\", …) or [ax, ay]".into(),
        _ => m,
    }
}

fn fmt_track(p: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut s = String::new();
    for seg in p.iter() {
        match seg {
            Segment::Seq { index } => s.push_str(&format!("[{index}]")),
            Segment::Map { key } => {
                if !s.is_empty() {
                    s.push('.');
                }
                s.push_str(key);
            }
            Segment::Enum { .. } | Segment::Unknown => {}
        }
    }
    s
}

fn fmt_ignored(p: &serde_ignored::Path) -> String {
    use serde_ignored::Path;
    match p {
        Path::Root => String::new(),
        Path::Seq { parent, index } => format!("{}[{index}]", fmt_ignored(parent)),
        Path::Map { parent, key } => {
            let pa = fmt_ignored(parent);
            if pa.is_empty() {
                key.clone()
            } else {
                format!("{pa}.{key}")
            }
        }
        Path::Some { parent } | Path::NewtypeStruct { parent } | Path::NewtypeVariant { parent } => fmt_ignored(parent),
    }
}

enum Seg2 {
    Key(String),
    Index,
}

fn segments(p: &serde_ignored::Path, out: &mut Vec<Seg2>) {
    use serde_ignored::Path;
    match p {
        Path::Root => {}
        Path::Seq { parent, .. } => {
            segments(parent, out);
            out.push(Seg2::Index);
        }
        Path::Map { parent, key } => {
            segments(parent, out);
            out.push(Seg2::Key(key.clone()));
        }
        Path::Some { parent } | Path::NewtypeStruct { parent } | Path::NewtypeVariant { parent } => segments(parent, out),
    }
}

/// `"ease": [x1, y1, x2, y2]` anywhere means `"cubic-bezier(x1,y1,x2,y2)"`.
fn bezier_eases(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k == "ease" {
                    if let Value::Array(a) = x {
                        if a.len() == 4 && a.iter().all(Value::is_number) {
                            *x = Value::String(format!("cubic-bezier({},{},{},{})", a[0], a[1], a[2], a[3]));
                            continue;
                        }
                    }
                }
                bezier_eases(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(bezier_eases),
        _ => {}
    }
}
