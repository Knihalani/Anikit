//! Vocabularies and "did you mean" suggestions.
//!
//! Semantic validation happens while compiling the spec in
//! [`crate::eval::Evaluator::new`], which records [`super::Diagnostic`]s with
//! paths; this module holds the word lists and the matcher.

use std::sync::OnceLock;

pub const LAYER_TYPES: &[&str] = &["text", "shape", "image", "video", "group", "box", "component", "shader"];
pub const SHAPES: &[&str] = &["rect", "circle", "ellipse", "line", "arrow", "polygon", "star", "path"];
pub const TRANSITIONS: &[&str] = &[
    "cut", "crossfade", "fade", "fadeBlack", "fadeWhite", "slide", "push", "zoom", "wipe", "iris", "blur", "glitch",
    "morphCut", "shader",
];
pub const DIRECTIONS: &[&str] = &["left", "right", "up", "down"];
pub const ENTER_PRESETS: &[&str] = &[
    "fadeIn", "fadeUp", "fadeDown", "fadeLeft", "fadeRight", "slideUp", "slideDown", "slideLeft", "slideRight", "popIn",
    "scaleIn", "zoomIn", "blurIn", "rotateIn", "flipIn", "bounceIn", "drawOn", "typewriter", "wipeIn", "countUp",
];
pub const EXIT_PRESETS: &[&str] = &[
    "fadeOut", "fadeUp", "fadeDown", "fadeLeft", "fadeRight", "slideUp", "slideDown", "slideLeft", "slideRight", "popOut",
    "scaleOut", "zoomOut", "blurOut", "drawOff", "wipeOut",
];
pub const LOOP_PRESETS: &[&str] = &["float", "pulse", "spin", "wiggle", "breathe", "blink"];
pub const BLENDS: &[&str] =
    &["normal", "add", "multiply", "screen", "overlay", "darken", "lighten", "softLight", "difference"];
pub const ANCHORS: &[&str] =
    &["center", "top", "bottom", "left", "right", "topLeft", "topRight", "bottomLeft", "bottomRight"];
pub const SPLITS: &[&str] = &["chars", "words", "lines"];
pub const FITS: &[&str] = &["contain", "cover", "fill", "none"];
pub const ALIGNS: &[&str] = &["left", "center", "right"];
pub const BOX_ALIGNS: &[&str] = &["start", "center", "end", "stretch"];
pub const JUSTIFIES: &[&str] = &["start", "center", "end", "spaceBetween", "spaceAround", "spaceEvenly"];
pub const STAGGER_FROM: &[&str] = &["start", "center", "end", "random"];
pub const SPRINGS: &[&str] = &["gentle", "wobbly", "stiff", "slow", "molasses", "snappy", "bouncy"];
pub const ANIM_PROPS: &[&str] = &[
    "x", "y", "width", "height", "scale", "scaleX", "scaleY", "rotation", "skewX", "skewY", "opacity", "blur", "radius",
    "fill", "color", "stroke", "strokeWidth", "trimStart", "trimEnd", "reveal", "value", "size", "letterSpacing",
    "progress", "zoom", "depth", "focus", "aperture",
];

/// Short preset aliases (`rise` = `fadeUp`, …). `exit` picks the exit form.
pub fn canon_preset(name: &str, exit: bool) -> &str {
    match (name, exit) {
        ("rise", _) => "fadeUp",
        ("drop", _) => "fadeDown",
        ("fade", false) | ("appear", _) => "fadeIn",
        ("fade", true) | ("disappear", _) => "fadeOut",
        ("pop", false) => "popIn",
        ("pop", true) => "popOut",
        ("grow", _) | ("scale", false) => "scaleIn",
        ("shrink", _) | ("scale", true) => "scaleOut",
        ("zoom", false) => "zoomIn",
        ("zoom", true) => "zoomOut",
        ("blur", false) | ("focus", _) => "blurIn",
        ("blur", true) => "blurOut",
        ("draw", false) => "drawOn",
        ("draw", true) => "drawOff",
        ("type", _) | ("typing", _) => "typewriter",
        ("count", _) => "countUp",
        ("wipe", false) => "wipeIn",
        ("wipe", true) => "wipeOut",
        ("slide", _) => "slideUp",
        ("bounce", _) => "bounceIn",
        ("spinIn", _) | ("rotate", _) => "rotateIn",
        ("flip", _) => "flipIn",
        _ => name,
    }
}

/// Canonical easing names (any Penner/CSS name `anikit-core` parses is accepted;
/// these are the ones suggested).
pub fn easings() -> &'static [String] {
    static E: OnceLock<Vec<String>> = OnceLock::new();
    E.get_or_init(|| {
        let mut v: Vec<String> =
            ["linear", "ease", "easeIn", "easeOut", "easeInOut"].iter().map(|s| s.to_string()).collect();
        for c in ["Sine", "Quad", "Cubic", "Quart", "Quint", "Expo", "Circ", "Back", "Elastic", "Bounce"] {
            for m in ["in", "out", "inOut"] {
                v.push(format!("{m}{c}"));
            }
        }
        v.extend(SPRINGS.iter().map(|s| s.to_string()));
        v
    })
}

/// Every object key that appears anywhere in the schema.
pub fn all_keys() -> &'static [String] {
    static K: OnceLock<Vec<String>> = OnceLock::new();
    K.get_or_init(|| {
        let schema: serde_json::Value = serde_json::from_str(&super::schema_json()).unwrap();
        let mut out = Vec::new();
        collect_props(&schema, &mut out);
        out.sort();
        out.dedup();
        out
    })
}

/// One step of a path into the spec.
pub enum Seg<'a> {
    Key(&'a str),
    Index,
}

/// The keys valid in the object at `path` (resolved through the schema), if known.
pub fn keys_at(path: &[Seg]) -> Option<Vec<String>> {
    static S: OnceLock<serde_json::Value> = OnceLock::new();
    let root = S.get_or_init(|| serde_json::from_str(&super::schema_json()).unwrap());
    let defs = root.get("$defs");
    // Resolve $ref / anyOf / oneOf to the object (or array) variant.
    fn resolve<'a>(v: &'a serde_json::Value, defs: Option<&'a serde_json::Value>, want_array: bool) -> Option<&'a serde_json::Value> {
        let mut v = v;
        for _ in 0..8 {
            if let Some(r) = v.get("$ref").and_then(|r| r.as_str()) {
                v = defs?.get(r.rsplit('/').next()?)?;
                continue;
            }
            if let Some(list) = v.get("anyOf").or_else(|| v.get("oneOf")).and_then(|x| x.as_array()) {
                let pick = list.iter().find_map(|x| {
                    let r = resolve(x, defs, want_array)?;
                    let ok = if want_array { r.get("items").is_some() } else { r.get("properties").is_some() };
                    ok.then_some(r)
                });
                return pick;
            }
            return Some(v);
        }
        None
    }
    let mut cur = resolve(root, defs, false)?;
    for seg in path {
        cur = match seg {
            Seg::Key(k) => resolve(cur.get("properties")?.get(*k)?, defs, false).or_else(|| resolve(cur.get("properties")?.get(*k)?, defs, true))?,
            Seg::Index => resolve(resolve(cur, defs, true)?.get("items")?, defs, false)?,
        };
    }
    let props = cur.get("properties")?.as_object()?;
    Some(props.keys().cloned().collect())
}

fn collect_props(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(serde_json::Value::Object(p)) = m.get("properties") {
                out.extend(p.keys().cloned());
            }
            for x in m.values() {
                collect_props(x, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_props(x, out)),
        _ => {}
    }
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

fn lev(a: &[u8], b: &[u8]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let c = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + c);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Closest candidate to `word`, if any is close enough to be a likely typo.
pub fn suggest<S: AsRef<str>>(word: &str, candidates: &[S]) -> Option<String> {
    let w = norm(word);
    // Common aliases from other tools / British spelling.
    let alias = match w.as_str() {
        "colour" => Some("color"),
        "fontsize" | "textsize" => Some("size"),
        "fontweight" => Some("weight"),
        "fontfamily" => Some("font"),
        "rotate" | "angle" => Some("rotation"),
        "alpha" => Some("opacity"),
        "delay" => Some("at"),
        "easing" => Some("ease"),
        "children" => Some("layers"),
        "left" => Some("x"),
        "top" => Some("y"),
        "content" | "label" => Some("text"),
        "textalign" => Some("align"),
        "backgroundcolor" | "bg" => Some("background"),
        "dissolve" => Some("crossfade"),
        _ => None,
    };
    if let Some(a) = alias {
        if candidates.iter().any(|c| c.as_ref() == a) {
            return Some(a.to_string());
        }
    }
    // Normalised exact match (case, dashes, an `ease` prefix: easeOutQuart → outQuart).
    let strip = |s: &str| {
        let n = norm(s);
        n.strip_prefix("ease").filter(|r| !r.is_empty()).map(|r| r.to_string()).unwrap_or(n)
    };
    let ws = strip(word);
    for c in candidates {
        if strip(c.as_ref()) == ws {
            return Some(c.as_ref().to_string());
        }
    }
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        let cn = strip(c.as_ref());
        let mut d = lev(ws.as_bytes(), cn.as_bytes());
        // Prefix / suffix matches (outQuartic → outQuart, txt → text).
        if cn.len() >= 3 && (ws.starts_with(&cn) || cn.starts_with(&ws)) {
            d = d.min(1 + ws.len().abs_diff(cn.len()) / 3);
        }
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c.as_ref()));
        }
    }
    let (d, c) = best?;
    let limit = (ws.len().max(3) / 3).clamp(1, 3);
    (d <= limit).then(|| c.to_string())
}

/// Check `value` is in `allowed`; returns the suggestion on failure.
pub fn check_word(value: &str, allowed: &[&str]) -> Result<(), Option<String>> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(suggest(value, allowed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions() {
        assert_eq!(suggest("easeOutQuartic", easings()).as_deref(), Some("outQuart"));
        assert_eq!(suggest("easeOutQuart", easings()).as_deref(), Some("outQuart"));
        assert_eq!(suggest("txt", LAYER_TYPES).as_deref(), Some("text"));
        assert_eq!(suggest("colour", all_keys()).as_deref(), Some("color"));
        assert_eq!(suggest("opactiy", all_keys()).as_deref(), Some("opacity"));
        assert_eq!(suggest("fadeup", ENTER_PRESETS).as_deref(), Some("fadeUp"));
        assert_eq!(suggest("zzzzzz", LAYER_TYPES), None);
    }
}
