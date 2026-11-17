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
    from_value(v)
}

/// Parse YAML.
pub fn parse_yaml(src: &str) -> Result<Parsed, Vec<Diagnostic>> {
    let v: serde_json::Value =
        serde_norway::from_str(src).map_err(|e| vec![Diagnostic::error("", format!("invalid YAML: {e}"))])?;
    from_value(v)
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
pub fn from_value(v: serde_json::Value) -> Result<Parsed, Vec<Diagnostic>> {
    let v = super::reuse::expand(v)?;
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
