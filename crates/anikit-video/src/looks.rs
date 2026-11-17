//! Looks: art-direction presets that make a whole video a distinct world.
//!
//! A look bundles a palette, a type pairing and text styles, materials (radius,
//! text shadow or glow), a motion grammar (default text entrance), the default
//! transition, camera language, world layers (a backdrop under every scene and
//! overlay layers over it, e.g. grain) and the finishing grade (`post`, `motionBlur`).
//!
//! In a spec: `"look": "noir"`, composed `"look": ["editorial", "noir"]`
//! (later wins), or overridden `"look": {"base": "swiss", "theme": {"colors": {"primary": "#0050FF"}}}`.
//! Anything the spec sets itself (theme keys, `transition`, a scene `camera`,
//! a layer's `enter`) wins over the look. A scene with `"look": false` is left alone.

use serde_json::{json, Map, Value};

use crate::spec::parse::Diagnostic;
use crate::templates::suggest;

/// `(name, json)` for every built-in look.
pub const LOOKS: &[(&str, &str)] = &[
    ("daylight-lab", include_str!("../looks/daylight-lab.json")),
    ("editorial", include_str!("../looks/editorial.json")),
    ("swiss", include_str!("../looks/swiss.json")),
    ("brutalist", include_str!("../looks/brutalist.json")),
    ("y2k-chrome", include_str!("../looks/y2k-chrome.json")),
    ("noir", include_str!("../looks/noir.json")),
    ("risograph", include_str!("../looks/risograph.json")),
    ("neon-night", include_str!("../looks/neon-night.json")),
    ("soft-clay", include_str!("../looks/soft-clay.json")),
    ("documentary", include_str!("../looks/documentary.json")),
];

/// Every built-in look name.
pub fn names() -> Vec<&'static str> {
    LOOKS.iter().map(|(n, _)| *n).collect()
}

/// A built-in look by name.
pub fn builtin(name: &str) -> Result<Value, String> {
    match LOOKS.iter().find(|(n, _)| *n == name) {
        Some((n, src)) => {
            let mut v: Value = serde_json::from_str(src).map_err(|e| format!("look '{n}': {e}"))?;
            v["name"] = json!(n);
            Ok(v)
        }
        None => {
            let hint = suggest(name, names().into_iter()).map(|s| format!("did you mean '{s}'? ")).unwrap_or_default();
            Err(format!("unknown look '{name}'. {hint}Looks: {}", names().join(", ")))
        }
    }
}

/// Deep merge: objects merge key by key, anything else is replaced.
pub fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(bv) if bv.is_object() && v.is_object() => merge(bv, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

/// Resolve a `look` value (name, list, or object with `base`) to one look object.
pub fn resolve(v: &Value) -> Result<Value, String> {
    match v {
        Value::String(s) => builtin(s),
        Value::Array(items) => {
            let mut out = json!({});
            let mut names = Vec::new();
            for it in items {
                let l = resolve(it)?;
                if let Some(n) = l.get("name").and_then(|x| x.as_str()) {
                    names.push(n.to_string());
                }
                merge(&mut out, &l);
            }
            out["name"] = json!(names.join("+"));
            Ok(out)
        }
        Value::Object(m) => {
            let mut out = match m.get("base") {
                Some(b) => resolve(b)?,
                None => json!({}),
            };
            let mut over = m.clone();
            over.remove("base");
            let name = out.get("name").and_then(|x| x.as_str()).unwrap_or("custom").to_string();
            merge(&mut out, &Value::Object(over));
            if out.get("name").and_then(|x| x.as_str()) == Some(name.as_str()) && m.contains_key("base") {
                out["name"] = json!(format!("{name}*"));
            }
            Ok(out)
        }
        _ => Err("'look' must be a look name, a list of names, or an object with 'base'".into()),
    }
}

/// One line per look, for listings.
pub fn list_lines() -> Vec<String> {
    LOOKS
        .iter()
        .map(|(n, _)| {
            let l = builtin(n).unwrap();
            format!("{n:13} {}", l.get("summary").and_then(|x| x.as_str()).unwrap_or(""))
        })
        .collect()
}

/// Components that already paint a full-frame background: a scene that starts
/// with one keeps it, and the look's backdrop is not added.
const BACKGROUNDS: &[&str] = &["mesh-gradient", "aurora", "gradient", "ken-burns", "starfield", "god-rays", "light-leak", "tile-reveal", "kinetic-grid", "metaballs"];

fn is_background(layer: &Value) -> bool {
    let Some(m) = layer.as_object() else { return false };
    if m.get("id").and_then(|x| x.as_str()).is_some_and(|s| s == "bg" || s.starts_with("bg-") || s.starts_with("look-")) {
        return true;
    }
    if m.get("component").and_then(|x| x.as_str()).is_some_and(|c| BACKGROUNDS.contains(&c)) {
        return true;
    }
    // A full-frame shader, or a full-bleed image or video.
    if (m.contains_key("shader") || m.get("type").and_then(|t| t.as_str()) == Some("shader")) && m.get("filter") != Some(&Value::Bool(true)) && !m.contains_key("width") {
        return true;
    }
    m.contains_key("src") && m.get("fit").and_then(|x| x.as_str()) == Some("cover")
}

fn is_text(layer: &Value) -> bool {
    layer.is_string() || layer.as_object().is_some_and(|m| m.contains_key("text") && !m.contains_key("component"))
}

/// Apply the spec's top-level `look` (if any). Runs after `defs`/`use` expansion.
pub fn apply(mut v: Value) -> Result<Value, Vec<Diagnostic>> {
    let Some(root) = v.as_object_mut() else { return Ok(v) };
    // `intent` is for the reviewer (`review.rs`), not the renderer.
    root.remove("intent");
    let Some(lv) = root.remove("look") else { return Ok(v) };
    if lv == Value::Bool(false) || lv.as_str() == Some("none") {
        return Ok(v);
    }
    let look = resolve(&lv).map_err(|e| vec![Diagnostic::error("look", e)])?;
    // Theme: the spec's own keys win.
    if let Some(t) = look.get("theme") {
        let mut theme = t.clone();
        if let Some(own) = root.get("theme") {
            merge(&mut theme, own);
        }
        root.insert("theme".into(), theme);
    }
    // Finishing: the colour grade and motion blur, unless the spec sets its own.
    for k in ["transition", "post", "motionBlur"] {
        if let Some(t) = look.get(k).filter(|x| !x.is_null()) {
            root.entry(k).or_insert_with(|| t.clone());
        }
    }
    let empty = Vec::new();
    let backdrop = look.get("backdrop").and_then(|x| x.as_array()).unwrap_or(&empty);
    let overlay = look.get("overlay").and_then(|x| x.as_array()).unwrap_or(&empty);
    let some = |k: &str| look.get(k).filter(|x| !x.is_null());
    let enter = some("enter");
    let shadow = some("textShadow");
    let glow = some("textGlow");
    let camera = some("camera");
    if let Some(Value::Array(scenes)) = root.get_mut("scenes") {
        for scene in scenes.iter_mut() {
            if let Value::String(s) = scene {
                // A title card: "Title | subtitle".
                let mut parts = s.splitn(2, '|').map(|x| x.trim().to_string());
                let title = parts.next().unwrap_or_default();
                let mut layers = vec![json!({"text": title, "style": "title", "width": "86%"})];
                if let Some(sub) = parts.next().filter(|x| !x.is_empty()) {
                    layers[0]["y"] = json!("44%");
                    layers.push(json!({"text": sub, "style": "subtitle", "y": "60%", "width": "86%"}));
                }
                *scene = json!({ "layers": layers });
            }
            let Some(sm) = scene.as_object_mut() else { continue };
            if sm.remove("look") == Some(Value::Bool(false)) {
                continue;
            }
            if let Some(c) = camera {
                sm.entry("camera").or_insert_with(|| c.clone());
            }
            let layers = sm.entry("layers").or_insert_with(|| json!([]));
            let Some(ls) = layers.as_array_mut() else { continue };
            let mut k = 0;
            for l in ls.iter_mut() {
                if !is_text(l) {
                    continue;
                }
                if let Value::String(s) = l {
                    *l = json!({ "text": s });
                }
                let m = l.as_object_mut().unwrap();
                if let Some(e) = enter {
                    if !m.contains_key("enter") && !m.contains_key("animate") && !m.contains_key("kinetic") {
                        let mut e = match e {
                            Value::String(s) => json!({ "preset": s }),
                            other => other.clone(),
                        };
                        if k > 0 && e.get("delay").is_none() {
                            e["delay"] = json!(0.18 * k as f64);
                        }
                        m.insert("enter".into(), e);
                    }
                }
                if let Some(s) = shadow {
                    m.entry("shadow").or_insert_with(|| s.clone());
                }
                if let Some(g) = glow {
                    m.entry("glow").or_insert_with(|| g.clone());
                }
                k += 1;
            }
            if !ls.first().is_some_and(is_background) {
                for (i, b) in backdrop.iter().enumerate() {
                    ls.insert(i, b.clone());
                }
            }
            ls.extend(overlay.iter().cloned());
        }
    }
    Ok(v)
}

/// Per scene, how many look backdrop layers `apply` puts in front of the spec's own layers.
pub fn layer_offsets(raw: &Value) -> Vec<usize> {
    let Some(look) = raw.get("look").and_then(|l| resolve(l).ok()) else { return Vec::new() };
    let n = look.get("backdrop").and_then(|b| b.as_array()).map_or(0, |b| b.len());
    let v = crate::spec::reuse::expand(raw.clone()).unwrap_or_else(|_| raw.clone());
    v.get("scenes")
        .and_then(|s| s.as_array())
        .map(|scenes| {
            scenes
                .iter()
                .map(|sc| match sc {
                    Value::String(_) => n,
                    Value::Object(m) if m.get("look") == Some(&Value::Bool(false)) => 0,
                    Value::Object(m) if m.get("layers").and_then(|l| l.as_array()).and_then(|l| l.first()).is_some_and(is_background) => 0,
                    _ => n,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Map a path in the looked spec (`scenes[2].layers[4]…`) back to the author's own spec.
pub fn remap_path(offsets: &[usize], path: &str) -> String {
    let parse = || -> Option<(usize, usize, usize, usize)> {
        let rest = path.strip_prefix("scenes[")?;
        let i_end = rest.find(']')?;
        let si: usize = rest[..i_end].parse().ok()?;
        let rest2 = rest[i_end..].strip_prefix("].layers[")?;
        let j_end = rest2.find(']')?;
        let li: usize = rest2[..j_end].parse().ok()?;
        Some((si, li, "scenes[".len() + i_end + "].layers[".len(), j_end))
    };
    match parse() {
        Some((si, li, start, len)) => {
            let off = offsets.get(si).copied().unwrap_or(0);
            if off == 0 {
                return path.to_string();
            }
            if li < off {
                return format!("scenes[{si}] (the look's backdrop){}", &path[start + len + 1..]);
            }
            format!("{}{}{}", &path[..start], li - off, &path[start + len..])
        }
        None => path.to_string(),
    }
}

/// Brand defaults a look supplies to templates: `(theme, colors, fonts, radius)`.
pub fn brand_defaults(look: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let th = look.get("theme").cloned().unwrap_or(Value::Null);
    if let Some(mode) = look.get("mode") {
        out.insert("theme".into(), mode.clone());
    }
    if let Some(c) = th.get("colors").and_then(|x| x.as_object()) {
        let mut colors = Map::new();
        for (k, v) in c {
            let key = if k == "bg" { "background" } else { k.as_str() };
            if ["primary", "secondary", "accent", "background", "surface", "text", "muted"].contains(&key) && v.as_str().is_some_and(|s| s.starts_with('#')) {
                colors.insert(key.into(), v.clone());
            }
        }
        out.insert("colors".into(), Value::Object(colors));
    }
    if let Some(f) = th.get("fonts").and_then(|x| x.as_object()) {
        out.insert("fonts".into(), Value::Object(f.clone()));
    }
    if let Some(r) = th.get("radius") {
        out.insert("radius".into(), r.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_is_valid_and_distinct() {
        let mut seen = std::collections::HashSet::new();
        for n in names() {
            let l = builtin(n).unwrap();
            for k in ["title", "summary", "direction", "mode", "theme"] {
                assert!(l.get(k).is_some(), "{n}: missing {k}");
            }
            // Text colours must read on the canvas (templates set labels in accent).
            let c = &l["theme"]["colors"];
            let bg = crate::templates::color::parse(c["bg"].as_str().unwrap()).unwrap();
            for k in ["text", "muted", "accent"] {
                let x = crate::templates::color::parse(c[k].as_str().unwrap()).unwrap();
                assert!(crate::templates::color::contrast(x, bg) >= 4.5, "{n}: {k} is below 4.5:1 on bg");
            }
            let key = (l["theme"]["colors"]["bg"].clone(), l["theme"]["colors"]["primary"].clone(), l["theme"]["fonts"]["heading"].clone());
            assert!(seen.insert(format!("{key:?}")), "{n} duplicates another look");
            // A one-scene spec in the look parses without warnings.
            let spec = json!({ "look": n, "scenes": ["Hello | world", { "layers": [ { "text": "One", "style": "title" }, "two" ] }] });
            let p = crate::spec::parse::from_value(spec).unwrap_or_else(|e| panic!("{n}: {e:?}"));
            assert!(p.warnings.is_empty(), "{n}: {:?}", p.warnings);
        }
    }

    #[test]
    fn compose_override_and_opt_out() {
        let l = resolve(&json!(["editorial", "noir"])).unwrap();
        assert_eq!(l["name"], "editorial+noir");
        assert_eq!(l["theme"]["colors"]["bg"], "#090908");
        let l = resolve(&json!({"base": "swiss", "theme": {"colors": {"primary": "#0050FF"}}})).unwrap();
        assert_eq!(l["theme"]["colors"]["primary"], "#0050FF");
        assert_eq!(l["theme"]["radius"], 0);
        assert!(resolve(&json!("noire")).unwrap_err().contains("did you mean 'noir'"));
        let v = apply(json!({ "look": "noir", "theme": {"colors": {"text": "#FFFFFF"}}, "scenes": [
            { "layers": [ { "text": "a", "enter": "fade" } ] },
            { "look": false, "layers": [ { "text": "b" } ] } ] }))
        .unwrap();
        assert_eq!(v["theme"]["colors"]["text"], "#FFFFFF");
        assert_eq!(v["theme"]["colors"]["bg"], "#090908");
        let s0 = v["scenes"][0]["layers"].as_array().unwrap();
        assert_eq!(s0[0]["id"], "look-rays");
        assert_eq!(s0[1]["enter"], "fade");
        assert_eq!(s0.last().unwrap()["id"], "look-grain");
        assert_eq!(v["post"]["preset"], "noir");
        assert_eq!(v["scenes"][1]["layers"].as_array().unwrap().len(), 1);
        let raw = json!({ "look": "neon-night", "scenes": [ { "layers": [ { "text": "a" } ] }, { "look": false, "layers": [] } ] });
        let off = layer_offsets(&raw);
        assert_eq!(off, vec![3, 0]);
        assert_eq!(remap_path(&off, "scenes[0].layers[4].text"), "scenes[0].layers[1].text");
        assert_eq!(remap_path(&off, "scenes[0].layers[1] (t=1s)"), "scenes[0] (the look's backdrop) (t=1s)");
        assert_eq!(remap_path(&off, "scenes[1].layers[0]"), "scenes[1].layers[0]");
    }
}
