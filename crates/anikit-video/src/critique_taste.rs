//! Taste checks from the canon (`docs/video/taste/TASTE.md`): heuristics with
//! numbers from practice, reported as tips (and a warning only for blurred
//! words, which is a defect). They never prescribe a style: each note names
//! the tell and the decision to make, not the answer.

use serde_json::Value;

use super::{color, walk_layer, Ctx};

/// Music styles led by synthesizers (pads, saw leads, 808/reese bass), with their aliases.
pub(super) const SYNTH_LED: &[&str] = &[
    "house", "synthwave", "trap", "tech", "dnb", "ambient", "edm", "dance", "deep-house", "techno", "minimal", "future", "retro", "80s", "outrun",
    "drum-and-bass", "drill", "rap", "pad", "drone",
];
/// Looks whose world is printed, photographed or hand-made: a synth bed fights them.
const ACOUSTIC_LOOKS: &[&str] = &["editorial", "documentary", "noir", "soft-clay", "risograph", "daylight-lab", "swiss"];

struct TextInfo {
    scene: usize,
    size: f64,
    centred: bool,
}

/// Run every taste check on the look-applied `spec` (`raw` is what the author wrote).
pub(super) fn run(raw: &Value, spec: &Value, cx: &mut Ctx) {
    let scenes = spec.get("scenes").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let mut texts: Vec<TextInfo> = Vec::new();
    let mut blurred: Option<String> = None;
    for (si, scene) in scenes.iter().enumerate() {
        let dur = scene.get("duration").and_then(|d| cx.time(d)).unwrap_or(3.0);
        for (li, layer) in scene.get("layers").and_then(|l| l.as_array()).into_iter().flatten().enumerate() {
            walk_layer(cx, layer, &format!("scenes[{si}].layers[{li}]"), dur, [0.0; 4], 0.0, &mut |cx, l, p, _, _, _| {
                let Some(t) = l.get("text").and_then(|t| t.as_str()) else { return };
                if super::expr::word_count(t) == 0 {
                    return;
                }
                let style_name = l.get("style").and_then(|s| s.as_str()).unwrap_or("body");
                let def = super::builtin_style(style_name).0 * cx.h;
                let st = cx.style(style_name).cloned();
                let size = l.get("size").or_else(|| st.as_ref().and_then(|s| s.get("size"))).and_then(|v| cx.len(v, cx.h)).unwrap_or(def);
                let x = l.get("x").and_then(|v| cx.len(v, cx.w));
                let side = l.get("anchor").and_then(|a| a.as_str()).is_some_and(|a| a.contains("left") || a.contains("right"))
                    || l.get("align").and_then(|a| a.as_str()).is_some_and(|a| a == "left" || a == "right");
                let centred = !side && x.is_none_or(|x| (x - cx.w / 2.0).abs() < cx.w * 0.04);
                texts.push(TextInfo { scene: si, size, centred });
                if blurred.is_none() && words_blurred(l) {
                    blurred = Some(p.to_string());
                }
            });
        }
    }
    if let Some(p) = blurred {
        cx.note("warning", "text-crisp", &p, "words stay blurred while they are meant to be read, which tends to look low-resolution and slows reading".into(),
            "consider animating blur to 0 by the end of the entrance so the words are sharp while read (TASTE.md §1)");
    }
    type_scale(&texts, cx);
    centred(&texts, scenes.len(), cx);
    palette(spec, cx);
    rhythm(&scenes, cx);
    music(raw, spec, cx);
}

/// A static `blur`, or keyframes that end blurred, on a text layer.
fn words_blurred(l: &serde_json::Map<String, Value>) -> bool {
    let num = |v: Option<&Value>| v.and_then(|b| b.as_f64()).unwrap_or(0.0);
    let anims: Vec<&Value> = match l.get("animate") {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(o @ Value::Object(_)) => vec![o],
        _ => Vec::new(),
    };
    let last_to = anims.iter().rev().find_map(|a| a.get("to").and_then(|t| t.get("blur")));
    match last_to {
        Some(b) => num(Some(b)) > 0.5,
        None => num(l.get("blur")) > 0.5 && !anims.iter().any(|a| a.get("from").and_then(|f| f.get("blur")).is_some()),
    }
}

/// Hierarchy: too many distinct sizes blur it; two sizes within 20% are a near miss.
fn type_scale(texts: &[TextInfo], cx: &mut Ctx) {
    let mut sizes: Vec<f64> = texts.iter().map(|t| t.size).collect();
    sizes.sort_by(|a, b| a.total_cmp(b));
    let mut levels: Vec<f64> = Vec::new();
    for s in sizes {
        if levels.last().is_none_or(|l| s > l * 1.12) {
            levels.push(s);
        }
    }
    if levels.len() > 4 {
        let list: Vec<String> = levels.iter().map(|s| format!("{s:.0}")).collect();
        cx.note("tip", "type-scale", "scenes", format!("{} type sizes across the video ({} px); this many levels tends to blur the hierarchy", levels.len(), list.join(", ")),
            "consider 2–4 levels from one ratio (2:1 or 1.618 suit statements, 1.25–1.333 dense data) and reusing them (TASTE.md §1)");
    }
    let scenes: std::collections::BTreeSet<usize> = texts.iter().map(|t| t.scene).collect();
    for si in scenes {
        let mut s: Vec<f64> = texts.iter().filter(|t| t.scene == si).map(|t| t.size).collect();
        s.sort_by(|a, b| a.total_cmp(b));
        s.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        if let Some(w) = s.windows(2).find(|w| w[1] / w[0] < 1.2) {
            cx.note("tip", "type-scale", &format!("scenes[{si}]"), format!("text sizes {:.0} and {:.0} px are within 20%, so they tend to read as an accident rather than a hierarchy", w[0], w[1]),
                "consider making them equal, or separating them by a clear step (≥ 1.25×; motion often reads best at 1.5–3×)");
            return;
        }
    }
}

/// Centred-everything: most text scenes set every word on the centre axis.
fn centred(texts: &[TextInfo], n_scenes: usize, cx: &mut Ctx) {
    let text_scenes: std::collections::BTreeSet<usize> = texts.iter().map(|t| t.scene).collect();
    if text_scenes.len() < 4 || n_scenes < 4 {
        return;
    }
    let all_centred = text_scenes.iter().filter(|&&s| texts.iter().filter(|t| t.scene == s).all(|t| t.centred)).count();
    let r = all_centred as f64 / text_scenes.len() as f64;
    if r >= 0.8 {
        cx.note("tip", "composition", "scenes", format!("{:.0}% of text scenes centre every word; centred-everything tends to read as slides", r * 100.0),
            "centring works as a committed system (a statement, a symmetric world); otherwise consider anchoring type to a grid edge and letting scale and negative space work (TASTE.md §3)");
    }
}

/// Every colour string on a colour key, with `#hex` and `$token`s inside gradients.
fn colours(v: &Value, cx: &Ctx, out: &mut Vec<color::C>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if let (true, Value::String(s)) = (matches!(k.as_str(), "color" | "fill" | "stroke" | "background" | "glow" | "tint"), x) {
                    for tok in s.split(|c: char| c == ',' || c == '(' || c == ')' || c.is_whitespace()) {
                        if tok.starts_with('#') || tok.starts_with('$') {
                            if let Some(c) = cx.color(&Value::String(tok.to_string())) {
                                out.push(c);
                            }
                        }
                    }
                }
                colours(x, cx, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| colours(x, cx, out)),
        _ => {}
    }
}

/// Palette size: saturated hue families (30° bins) used in the scenes.
fn palette(spec: &Value, cx: &mut Ctx) {
    let mut cs = Vec::new();
    colours(spec.get("scenes").unwrap_or(&Value::Null), cx, &mut cs);
    let mut bins = std::collections::BTreeSet::new();
    for c in cs {
        let (h, s) = super::hue_sat(c);
        let l = color::luminance(c);
        if s > 0.35 && l > 0.02 && l < 0.9 {
            bins.insert(((h / 30.0).round() as i32).rem_euclid(12));
        }
    }
    if bins.len() >= 5 {
        cx.note("tip", "palette", "scenes", format!("{} saturated hue families in use; with this many, no colour tends to carry meaning", bins.len()),
            "consider settling the values first, then a base family plus an accent taken from the subject (TASTE.md §2); a deliberate rainbow is fine when it is the idea");
    }
}

/// Monotony: five or more shots all about the same length.
fn rhythm(scenes: &[Value], cx: &mut Ctx) {
    let d: Vec<f64> = scenes.iter().filter_map(|s| s.get("duration").and_then(|d| cx.time(d))).collect();
    if d.len() < 5 || d.len() != scenes.len() {
        return;
    }
    let mean = d.iter().sum::<f64>() / d.len() as f64;
    let sd = (d.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / d.len() as f64).sqrt();
    if mean > 0.0 && sd / mean < 0.25 {
        cx.note("tip", "rhythm", "scenes", format!("{} shots all about {mean:.1} s (variation {:.0}%); equal lengths tend to feel like a metronome", d.len(), sd / mean * 100.0),
            "consider varying lengths with the phrase: a run of short cuts, then a hold on a downbeat (edited film typically varies 60–120%; TASTE.md §6)");
    }
}

/// Instrumentation against the concept: the stock corporate bed, or synths under a printed/photographed look.
fn music(raw: &Value, spec: &Value, cx: &mut Ctx) {
    let m = spec.get("music").or_else(|| raw.get("music"));
    let style = match m {
        Some(Value::String(s)) => s.as_str(),
        Some(o @ Value::Object(_)) => o.get("style").and_then(|s| s.as_str()).unwrap_or(""),
        _ => return,
    };
    let own_instruments = m.and_then(|m| m.get("instruments")).is_some_and(|i| i.as_object().is_some_and(|o| !o.is_empty()))
        || m.and_then(|m| m.get("instrumentation")).and_then(|i| i.as_str()).is_some_and(|i| i != "electronic");
    if matches!(style, "corporate" | "upbeat" | "uplifting") {
        cx.note("tip", "music", "music", format!("the \"{style}\" style tends to sound like the stock uplifting bed viewers have heard many times"),
            "consider instrumentation from the concept's world (piano, acoustic, orchestral, jazz, rock, found percussion…) scored to its hit points (TASTE.md §7)");
        return;
    }
    // `medium()` already flags electronic styles on calm/steady pieces ("audio"): don't say it twice.
    let said = cx.notes.iter().any(|n| n.check == "audio" && n.path == "music");
    if !SYNTH_LED.contains(&style) || own_instruments || said {
        return;
    }
    let look = match raw.get("look") {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Array(a)) => a.first().and_then(|x| x.as_str()).map(String::from),
        Some(Value::Object(o)) => o.get("base").and_then(|b| b.as_str()).map(String::from),
        _ => None,
    };
    let msg = match look.as_deref() {
        Some(l) if ACOUSTIC_LOOKS.contains(&l) => format!("a synth-led \"{style}\" score under the \"{l}\" look, so sound and picture tend to describe different worlds"),
        _ => format!("a synth-led \"{style}\" score: synth-led scores tend to read as generic unless the concept is electronic"),
    };
    cx.note("tip", "music", "music", msg, "consider instruments from the concept's world: keep the groove with \"instrumentation\": \"band\" | \"acoustic\" | \"orchestral\" | \"piano\", set \"instruments\": {\"lead\": \"cello\"}, or use the piano / acoustic / orchestral / jazz / rock styles (TASTE.md §7)");
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    fn checks(spec: serde_json::Value) -> Vec<(&'static str, &'static str)> {
        super::super::critique(&spec).notes.iter().map(|n| (n.check, n.severity)).collect()
    }

    #[test]
    fn blurred_words_warn_unless_they_resolve() {
        let s = |anim: serde_json::Value| json!({ "scenes": [ { "duration": 3, "layers": [ { "text": "Ship it", "animate": anim } ] } ] });
        assert!(checks(s(json!([{ "to": { "blur": 8 }, "duration": 0.5 }]))).contains(&("text-crisp", "warning")));
        assert!(!checks(s(json!([{ "from": { "blur": 20 }, "duration": 0.3 }]))).iter().any(|c| c.0 == "text-crisp"));
        let stat = json!({ "scenes": [ { "duration": 3, "layers": [ { "text": "Soft", "blur": 4 } ] } ] });
        assert!(checks(stat).contains(&("text-crisp", "warning")));
    }

    #[test]
    fn type_scale_levels_and_near_misses() {
        let sizes = [20, 30, 45, 70, 110, 170];
        let scenes: Vec<_> = sizes.iter().map(|s| json!({ "duration": 2, "layers": [ { "text": "Word here", "size": s } ] })).collect();
        assert!(checks(json!({ "scenes": scenes })).contains(&("type-scale", "tip")));
        let near = json!({ "scenes": [ { "duration": 3, "layers": [ { "text": "Big", "size": 100, "y": 300 }, { "text": "Also big", "size": 110, "y": 600 } ] } ] });
        assert!(checks(near).contains(&("type-scale", "tip")));
        let clear = json!({ "scenes": [ { "duration": 3, "layers": [ { "text": "Big", "size": 160, "y": 300 }, { "text": "small", "size": 48, "y": 600 } ] } ] });
        assert!(!checks(clear).iter().any(|c| c.0 == "type-scale"));
    }

    #[test]
    fn centred_everything_and_monotone_cuts() {
        let scenes: Vec<_> = (0..5).map(|i| json!({ "duration": 1.0, "layers": [ { "text": format!("Line {i}") } ] })).collect();
        let c = checks(json!({ "scenes": scenes }));
        assert!(c.contains(&("composition", "tip")) && c.contains(&("rhythm", "tip")), "{c:?}");
        let varied: Vec<_> = [0.6, 1.4, 0.5, 2.2, 0.8].iter().enumerate()
            .map(|(i, d)| json!({ "duration": d, "layers": [ { "text": format!("Line {i}"), "x": "8%", "anchor": "left" } ] })).collect();
        let c = checks(json!({ "scenes": varied }));
        assert!(!c.iter().any(|c| c.0 == "composition" || c.0 == "rhythm"), "{c:?}");
    }

    #[test]
    fn palette_and_music() {
        let hues = ["#e63946", "#f4a261", "#2a9d8f", "#457b9d", "#9b5de5", "#f15bb5"];
        let layers: Vec<_> = hues.iter().map(|h| json!({ "shape": "rect", "fill": h, "width": 100, "height": 100 })).collect();
        assert!(checks(json!({ "scenes": [ { "duration": 3, "layers": layers } ] })).contains(&("palette", "tip")));
        let two = json!({ "scenes": [ { "duration": 3, "layers": [ { "shape": "rect", "fill": "#1b1b1b" }, { "shape": "rect", "fill": "#ff5a1f" }, { "shape": "rect", "fill": "#ff7a45" } ] } ] });
        assert!(!checks(two).iter().any(|c| c.0 == "palette"));
        let sc = json!([ { "duration": 3, "layers": [ { "text": "Hi" } ] } ]);
        assert!(checks(json!({ "music": "corporate", "scenes": sc })).contains(&("music", "tip")));
        assert!(checks(json!({ "music": "synthwave", "look": "editorial", "scenes": sc })).contains(&("music", "tip")));
        assert!(!checks(json!({ "music": "piano", "scenes": sc })).iter().any(|c| c.0 == "music"));
        assert!(!checks(json!({ "music": { "style": "house", "instruments": { "lead": "violin" } }, "scenes": sc })).iter().any(|c| c.0 == "music"));
        assert!(!checks(json!({ "music": { "style": "house", "instrumentation": "band" }, "scenes": sc })).iter().any(|c| c.0 == "music"));
        assert!(checks(json!({ "music": "techno", "scenes": sc })).contains(&("music", "tip")));
    }
}
