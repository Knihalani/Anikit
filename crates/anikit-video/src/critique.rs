//! `critique`: fast heuristic review of a spec, the way a motion designer
//! would review a cut. It needs no rendering, so agents can run it on every
//! draft. Checks:
//!
//! - **reading time**: each text layer is on screen long enough to read
//!   (about 3 words per second plus 1 s to notice it);
//! - **hook**: something meaningful appears within the first 2 s;
//! - **pacing**: scenes are neither rushed (< 1.2 s) nor static (> 8 s);
//!   transitions are short compared with their scenes;
//! - **legibility**: font sizes are large enough for the frame, and lines
//!   are not too long;
//! - **contrast**: text colour against the scene background (WCAG ratio);
//! - **safe areas**: text and images stay inside the title-safe margins
//!   (wider on 9:16, where platform UI covers the top and bottom);
//! - **density**: not too many words on screen at once, and few font families.
//!
//! It works on the JSON value, so it also reviews specs that don't fully
//! validate yet.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::templates::{color, expr, Format};

#[path = "critique_taste.rs"]
mod taste;

/// One finding.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Note {
    /// `"error"` (will look broken), `"warning"` (hurts the video) or `"tip"`.
    pub severity: &'static str,
    /// Check that produced it, e.g. `"reading-time"`.
    pub check: &'static str,
    /// JSON path, e.g. `scenes[2].layers[1]`.
    pub path: String,
    pub message: String,
    /// What to change.
    pub fix: String,
}

/// The result of [`critique`].
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    /// 0..100: starts at 100, minus 15 per error, 6 per warning, 1 per tip. Not a quality verdict.
    pub score: u32,
    pub duration: f64,
    pub notes: Vec<Note>,
    /// Per-category scores (`time`, `text`, `composition`, `story`, `technical`), same arithmetic.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<(String, u32)>,
    /// Intent versus what the render measured, and the energy curve (render reviews only).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub summary: String,
}

/// The category a check belongs to.
pub fn category(check: &str) -> &'static str {
    match check {
        "pace" | "dead-air" | "rhythm" | "arc" | "ending" | "beat" | "pacing" | "length" => "time",
        "reading-time" | "legibility" | "contrast" | "render-contrast" | "typography" | "density" | "render-overlap" | "type-scale" | "text-crisp" => "text",
        "hook" | "structure" | "placeholder" | "copy" | "music" => "story",
        "motion" => "time",
        "frame-fill" | "layout" => "composition",
        "flicker" | "judder" | "banding" | "audio" | "render-frame" => "technical",
        _ => "composition",
    }
}

fn deduct(notes: &[&Note]) -> u32 {
    notes.iter().fold(100i32, |s, n| s - match n.severity { "error" => 15, "warning" => 6, _ => 1 }).max(0) as u32
}

impl Report {
    /// Recompute the score and the category breakdown from the notes.
    pub fn rescore(&mut self) {
        self.score = deduct(&self.notes.iter().collect::<Vec<_>>());
        self.categories = ["time", "text", "composition", "story", "technical"]
            .iter()
            .map(|c| (c.to_string(), deduct(&self.notes.iter().filter(|n| category(n.check) == *c).collect::<Vec<_>>())))
            .collect();
    }

    /// Plain-text rendering for the terminal: a short summary, then the issues
    /// most severe first (the first 6 unless `detail`).
    pub fn to_text_detail(&self, detail: bool) -> String {
        let count = |sev: &str| self.notes.iter().filter(|n| n.severity == sev).count();
        let (e, w, t) = (count("error"), count("warning"), count("tip"));
        let mut s = format!("review: {e} error{}, {w} warning{}, {t} tip{} · {:.1} s", if e == 1 { "" } else { "s" }, if w == 1 { "" } else { "s" }, if t == 1 { "" } else { "s" }, self.duration);
        if let Some((c, _)) = self.categories.iter().filter(|(_, v)| *v < 100).min_by_key(|(_, v)| *v) {
            s += &format!(" · most notes: {c}");
        }
        s += "\n";
        if !self.summary.is_empty() {
            s += &self.summary;
            s += "\n";
        }
        let rank = |n: &&Note| match n.severity { "error" => 0, "warning" => 1, _ => 2 };
        let mut v: Vec<&Note> = self.notes.iter().collect();
        v.sort_by_key(rank);
        // The short form: errors and warnings, one per check (the same advice
        // about ten layers is one note), at most five.
        let shown: Vec<&Note> = if detail {
            v.clone()
        } else {
            let mut seen = std::collections::BTreeSet::new();
            v.iter().filter(|n| n.severity != "tip" && seen.insert(n.check)).take(5).copied().collect()
        };
        for n in &shown {
            let same = self.notes.iter().filter(|m| m.check == n.check && m.severity == n.severity).count();
            let more = if !detail && same > 1 { format!(" (+{} more like it)", same - 1) } else { String::new() };
            s += &format!("{:<7} {:<13} {}: {}{more}\n        try: {}\n", n.severity, n.check, n.path, n.message, n.fix);
        }
        if shown.len() < v.len() {
            s += &format!("… {} more notes, tips included (--detail)\n", v.len() - shown.len());
        }
        s
    }

    /// [`Report::to_text_detail`] with every issue.
    pub fn to_text(&self) -> String {
        self.to_text_detail(true)
    }
}

/// The full review: the spec heuristics and, with `render`, the pixel checks
/// on sampled frames plus the time axis judged against the spec's intent.
pub fn review(spec: &Value, base: &std::path::Path, range: Option<(f64, f64)>, render: bool) -> Report {
    review_pack(spec, base, range, render, None).0
}

/// [`review`], optionally writing the review pack (see `review::write_pack`) to `pack`.
pub fn review_pack(spec: &Value, base: &std::path::Path, range: Option<(f64, f64)>, render: bool, pack: Option<&std::path::Path>) -> (Report, Result<Vec<String>, String>) {
    let mut r = critique_in_range(spec, range);
    let mut files = Ok(Vec::new());
    if render || pack.is_some() {
        let base = if base.as_os_str().is_empty() { std::path::Path::new(".") } else { base };
        if let Ok(p) = crate::preview::Preview::new(spec.clone(), base) {
            // Rendered pixels decide contrast: drop the spec-level estimate.
            r.notes.retain(|n| n.check != "contrast");
            r.notes.extend(crate::render_check::check_spec(spec, &p.ev));
            let beats = crate::review::music_beats(spec, base);
            let intent = crate::review::intent(spec, beats.as_ref().map(|b| b.bpm));
            let samples = crate::review::Samples::take(&p.ev, 6.0, 192);
            let (tl, notes) = crate::review::time_axis(&p.ev, &samples, &intent, beats.as_ref().map(|b| &b.beats[..]));
            r.summary = crate::review::summary(&intent, &tl);
            r.notes.extend(notes);
            let (cn, worst) = crate::review::composition(&p.ev, &samples, &tl);
            r.notes.extend(cn);
            r.notes.extend(crate::review_taste::checks(&samples, &tl));
            let (line, an) = crate::review::audio(&p.ev, base);
            if let Some(l) = line {
                r.summary += &format!("\n{l}");
            }
            r.notes.extend(an);
            r.rescore();
            if let Some(dir) = pack {
                files = crate::review::write_pack(dir, &p, &tl, &worst, &r.to_text_detail(true));
            }
            return (r, files);
        }
    }
    r.rescore();
    (r, files)
}

struct Ctx<'a> {
    w: f64,
    h: f64,
    fps: f64,
    portrait: bool,
    safe: [f64; 4],
    colors: Map<String, Value>,
    styles: &'a Map<String, Value>,
    markers: Map<String, Value>,
    notes: Vec<Note>,
}

impl Ctx<'_> {
    fn note(&mut self, severity: &'static str, check: &'static str, path: &str, message: String, fix: impl Into<String>) {
        self.notes.push(Note { severity, check, path: path.to_string(), message, fix: fix.into() });
    }

    /// Seconds from a spec time: 1.5, "1.5s", "500ms", "12f", "@marker+0.2".
    fn time(&self, v: &Value) -> Option<f64> {
        match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => {
                let s = s.trim();
                if let Some(m) = s.strip_prefix('@') {
                    let (name, off) = match m.find(['+', '-']) {
                        Some(i) => (&m[..i], m[i..].parse::<f64>().unwrap_or(0.0)),
                        None => (m, 0.0),
                    };
                    return self.markers.get(name).and_then(|x| self.time(x)).map(|t| t + off);
                }
                if let Some(x) = s.strip_suffix("ms") {
                    return x.trim().parse::<f64>().ok().map(|x| x / 1000.0);
                }
                if let Some(x) = s.strip_suffix('s') {
                    return x.trim().parse().ok();
                }
                if let Some(x) = s.strip_suffix('f') {
                    return x.trim().parse::<f64>().ok().map(|x| x / self.fps);
                }
                s.parse().ok()
            }
            _ => None,
        }
    }

    /// Pixels from a spec length; `rel` is the parent size for percentages.
    fn len(&self, v: &Value, rel: f64) -> Option<f64> {
        match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => {
                let s = s.trim();
                if s.starts_with('=') {
                    return None;
                }
                let units: [(&str, f64); 6] = [
                    ("vmin", self.w.min(self.h) / 100.0),
                    ("vmax", self.w.max(self.h) / 100.0),
                    ("vw", self.w / 100.0),
                    ("vh", self.h / 100.0),
                    ("%", rel / 100.0),
                    ("px", 1.0),
                ];
                for (u, k) in units {
                    if let Some(x) = s.strip_suffix(u) {
                        return x.trim().parse::<f64>().ok().map(|x| x * k);
                    }
                }
                s.parse().ok()
            }
            _ => None,
        }
    }

    /// A colour, resolving `$token`s; gradients give their first stop.
    fn color(&self, v: &Value) -> Option<color::C> {
        let s = v.as_str()?.trim();
        if let Some(tok) = s.strip_prefix('$') {
            return self.colors.get(tok).and_then(|c| self.color(c));
        }
        if s.contains("gradient(") {
            let inner = &s[s.find('(')? + 1..s.rfind(')')?];
            let parts = split_top(inner);
            let stops: Vec<color::C> = parts
                .iter()
                .filter_map(|p| p.split_whitespace().next())
                .filter_map(|p| self.color(&Value::String(p.to_string())))
                .collect();
            if stops.is_empty() {
                return None;
            }
            // Average of the stops: a fair stand-in for "the background behind the text".
            let n = stops.len() as f64;
            let mut c = [0.0; 4];
            for st in &stops {
                for i in 0..4 {
                    c[i] += st[i] / n;
                }
            }
            return Some(c);
        }
        color::parse(&s.replace(' ', ",").replace(",,", ",")).ok().or_else(|| css_name(s))
    }

    fn style(&self, name: &str) -> Option<&Map<String, Value>> {
        self.styles.get(name).and_then(|v| v.as_object())
    }
}

fn split_top(s: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(String::new());
                continue;
            }
            _ => {}
        }
        out.last_mut().unwrap().push(c);
    }
    out.into_iter().map(|x| x.trim().to_string()).collect()
}

fn css_name(s: &str) -> Option<color::C> {
    let hex = match s.to_ascii_lowercase().as_str() {
        "white" => "#ffffff",
        "black" => "#000000",
        "red" => "#ff0000",
        "tomato" => "#ff6347",
        "gray" | "grey" => "#808080",
        "yellow" => "#ffff00",
        "orange" => "#ffa500",
        "blue" => "#0000ff",
        "green" => "#008000",
        _ => return None,
    };
    color::parse(hex).ok()
}

/// Built-in style defaults from spec.md, as fractions of the frame height.
fn builtin_style(name: &str) -> (f64, &'static str) {
    match name {
        "title" => (0.09, "$text"),
        "heading" => (0.06, "$text"),
        "subtitle" => (0.036, "$muted"),
        "caption" => (0.022, "$muted"),
        "label" => (0.018, "$muted"),
        "code" => (0.026, "$text"),
        _ => (0.03, "$text"),
    }
}

const DEFAULT_COLORS: &[(&str, &str)] = &[
    ("primary", "#7c5cff"),
    ("secondary", "#22d3ee"),
    ("accent", "#ff5ca8"),
    ("bg", "#0b0b12"),
    ("surface", "#171725"),
    ("text", "#f5f5fa"),
    ("muted", "#9a9ab0"),
];

/// Review a spec made from a template whose stated length is `range` seconds:
/// [`critique`] plus a `length` warning when the video falls outside it.
pub fn critique_in_range(spec: &Value, range: Option<(f64, f64)>) -> Report {
    let mut r = critique(spec);
    if let Some((lo, hi)) = range {
        let d = r.duration;
        if d > hi + 0.5 || d + 0.5 < lo {
            let mut per: Vec<String> = Vec::new();
            for (i, sc) in spec.get("scenes").and_then(|s| s.as_array()).into_iter().flatten().enumerate() {
                let id = sc.get("id").and_then(|x| x.as_str()).map(String::from).unwrap_or_else(|| format!("scene{}", i + 1));
                let dur = sc.get("duration").and_then(|x| x.as_f64()).unwrap_or(3.0);
                per.push(format!("{id} {dur:.1}"));
            }
            let fix = if d > hi { format!("cut about {:.0} s: drop a list item or shorten copy (scene seconds: {})", (d - hi).ceil(), per.join(", ")) } else { format!("add about {:.0} s of content", (lo - d).ceil()) };
            r.notes.push(Note { severity: "warning", check: "length", path: "scenes".into(), message: format!("{d:.1} s is outside this template's {lo}–{hi} s"), fix });
            r.score = r.score.saturating_sub(6);
        }
    }
    r
}

/// Review a spec.
pub fn critique(raw: &Value) -> Report {
    // Judge what renders: with a look applied (its theme, backdrop and post layers).
    let applied = raw.get("look").and_then(|_| crate::looks::apply(raw.clone()).ok());
    let spec = applied.as_ref().unwrap_or(raw);
    let empty = Map::new();
    let theme = spec.get("theme").and_then(|t| t.as_object()).unwrap_or(&empty);
    let mut colors: Map<String, Value> = DEFAULT_COLORS.iter().map(|(k, v)| (k.to_string(), Value::String(v.to_string()))).collect();
    if let Some(c) = theme.get("colors").and_then(|c| c.as_object()) {
        colors.extend(c.clone());
    }
    let styles = theme.get("styles").and_then(|s| s.as_object()).unwrap_or(&empty);
    let (w, h) = match spec.get("size") {
        Some(Value::Array(a)) if a.len() == 2 => (a[0].as_f64().unwrap_or(1920.0), a[1].as_f64().unwrap_or(1080.0)),
        Some(Value::String(s)) => Format::parse(s).map(|f| (f.width as f64, f.height as f64)).unwrap_or((1920.0, 1080.0)),
        _ => (1920.0, 1080.0),
    };
    let fmt = Format { width: w as u32, height: h as u32, fps: 30 };
    let mut cx = Ctx {
        w,
        h,
        fps: spec.get("fps").and_then(|f| f.as_f64()).unwrap_or(30.0),
        portrait: fmt.orientation() == "portrait",
        safe: fmt.safe(),
        colors,
        styles,
        markers: spec.get("markers").and_then(|m| m.as_object()).cloned().unwrap_or_default(),
        notes: Vec::new(),
    };

    let scenes = spec.get("scenes").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    if scenes.is_empty() {
        cx.note("error", "structure", "scenes", "the spec has no scenes".into(), "add at least one scene with layers");
    }
    let default_bg = spec.get("background").cloned().unwrap_or(Value::String("$bg".into()));
    let default_tr = spec.get("transition").and_then(|t| transition_secs(&cx, t)).unwrap_or(0.5);
    let mut fonts = std::collections::BTreeSet::new();
    let mut total = 0.0;
    let mut first_text_at: Option<f64> = None;

    for (si, scene) in scenes.iter().enumerate() {
        let path = format!("scenes[{si}]");
        let dur = scene.get("duration").and_then(|d| cx.time(d)).unwrap_or(3.0);
        let tr = if si == 0 { 0.0 } else { scene.get("transition").and_then(|t| transition_secs(&cx, t)).unwrap_or(default_tr) };
        let scene_start = total - tr;
        total += dur - tr;
        let id = scene.get("id").and_then(|v| v.as_str()).map(|s| format!(" ('{s}')")).unwrap_or_default();
        if tr > dur * 0.5 && si > 0 {
            cx.note("warning", "pacing", &path, format!("transition of {tr:.2} s is more than half of the {dur:.2} s scene"), "keep transitions at 0.3–0.7 s, at most a third of the scene");
        }
        let bg_v = scene.get("background").cloned().unwrap_or(default_bg.clone());
        let bg = cx.color(&bg_v).unwrap_or([0.04, 0.04, 0.07, 1.0]);
        let layers = scene.get("layers").and_then(|l| l.as_array()).cloned().unwrap_or_default();
        // Text over a full-frame shader, 3D scene or photo: the background is pixels, judged by `--render`.
        let pixel_bg = layers.iter().any(|l| {
            (l.get("shader").is_some() || l.get("type").and_then(|t| t.as_str()) == Some("shader")) && l.get("width").is_none()
                || l.get("component").and_then(|c| c.as_str()).is_some_and(|c| matches!(c, "scene3d" | "scene-3d" | "ken-burns"))
                || (l.get("src").is_some() && l.get("fit").and_then(|f| f.as_str()) == Some("cover"))
        });
        let mut words_on_screen = 0usize;
        let mut text_sizes: Vec<f64> = Vec::new();
        let mut has_motion = scene.get("camera").is_some();
        let mut kinetic_only = true;
        // A full-frame filled shape backs the layers drawn after it.
        let mut backing = bg;
        for (li, layer) in layers.iter().enumerate() {
            walk_layer(&mut cx, layer, &format!("{path}.layers[{li}]"), dur, backing, 0.0, &mut |cx, l, p, vis_from, vis_to, bg| {
                if l.get("enter").is_some() || l.get("animate").is_some() || l.get("loop").is_some() {
                    has_motion = true;
                }
                if let Some(f) = l.get("font").and_then(|f| f.as_str()) {
                    fonts.insert(f.to_string());
                }
                // Components that show words (a terminal, a prompt, a title card) count for the hook.
                if first_text_at.is_none() && l.get("component").is_some() {
                    if let Some(p) = l.get("props").and_then(|p| p.as_object()) {
                        // Words anywhere in the props: a terminal's lines, a 3D wordmark, particles emitted from text.
                        fn has_words(v: &Value) -> bool {
                            match v {
                                Value::Object(m) => ["text", "title", "lines", "code", "label", "words", "messages", "items"].iter().any(|k| m.get(*k).is_some_and(|x| x.is_string() || x.is_array())) || m.values().any(has_words),
                                Value::Array(a) => a.iter().any(has_words),
                                _ => false,
                            }
                        }
                        if has_words(&Value::Object(p.clone())) {
                            let delay = p.get("delay").and_then(|d| d.as_f64()).unwrap_or(0.0);
                            first_text_at = Some(scene_start.max(0.0) + vis_from + delay + 0.3);
                        }
                    }
                }
                let Some(raw) = l.get("text").and_then(|t| t.as_str()) else { return };
                let text_owned = with_value(raw, l.get("value"));
                let text = text_owned.as_str();
                let words = expr::word_count(text);
                kinetic_only &= words <= 2;
                words_on_screen += words;
                let visible = vis_to - vis_from;
                // Single kinetic words and numbers (a teaser beat, a countdown) read in a glance.
                // Labels, dates and numbers (a few short words) read in a glance too.
                let need = if (words <= 2 && text.chars().count() <= 14 && visible < 1.5) || (words <= 4 && text.chars().count() <= 24) { 0.35 + 0.15 * words as f64 } else { expr::read_time(text) };
                if words > 0 && visible + 0.05 < need {
                    cx.note("warning", "reading-time", p, format!("\"{}\" ({words} words) is readable for {visible:.1} s but needs about {need:.1} s", trunc(text, 40)),
                        format!("extend the scene/layer by {:.1} s or cut it to {} words", need - visible, ((visible - 1.0).max(0.3) * 3.0).floor().max(1.0)));
                }
                if first_text_at.is_none() && words > 0 {
                    first_text_at = Some(scene_start.max(0.0) + vis_from);
                }
                // Legibility: font size.
                let style_name = l.get("style").and_then(|s| s.as_str()).unwrap_or("body");
                let (def_frac, def_color) = builtin_style(style_name);
                let st = cx.style(style_name).cloned();
                let size = l.get("size").or_else(|| st.as_ref().and_then(|s| s.get("size"))).and_then(|v| cx.len(v, cx.h)).unwrap_or(def_frac * cx.h);
                let short = cx.w.min(cx.h);
                if size < short * 0.022 {
                    cx.note("warning", "legibility", p, format!("text size {size:.0} px is small for a {}×{} frame (phones shrink it further)", cx.w, cx.h),
                        format!("use at least {:.0} px (2.2vmin); 3vmin+ for body copy", short * 0.022));
                }
                text_sizes.push(size);
                type_checks(cx, l, p, text, size, words);
                // Line length.
                let width = l.get("width").and_then(|v| cx.len(v, cx.w));
                let chars = text.lines().map(|x| x.chars().count()).max().unwrap_or(0) as f64;
                let line_px = chars * size * 0.52;
                let max_w = cx.w - cx.safe[1] - cx.safe[3];
                if width.is_none() && line_px > max_w {
                    cx.note("warning", "safe-area", p, format!("\"{}\" is about {line_px:.0} px wide on one line; the safe width is {max_w:.0} px", trunc(text, 40)),
                        format!("set \"width\": {max_w:.0} (or a %), so it wraps, or shorten it"));
                }
                // Contrast.
                let col_v = l.get("color").cloned().or_else(|| st.as_ref().and_then(|s| s.get("color").cloned())).unwrap_or(Value::String(def_color.into()));
                if let Some(fg) = cx.color(&col_v).filter(|_| !pixel_bg) {
                    let fg = color::over(fg, bg);
                    let ratio = color::contrast(fg, bg);
                    let large = size >= short * 0.04;
                    let min = if large { 3.0 } else { 4.5 };
                    if ratio < min {
                        cx.note(if ratio < 2.0 { "error" } else { "warning" }, "contrast", p,
                            format!("text contrast {ratio:.1}:1 against the background (needs {min}:1 at this size)"),
                            format!("use {} text, or darken/lighten the background", color::to_hex(color::contrast_text(bg))));
                    }
                }
            });
            if layer.get("shape").is_some() && layer.get("text").is_none() {
                let full = |k: &str, n: f64| layer.get(k).and_then(|v| cx.len(v, n)).is_some_and(|v| v >= 0.9 * n);
                if full("width", cx.w) && full("height", cx.h) {
                    if let Some(mut c) = layer.get("fill").and_then(|c| cx.color(c)) {
                        c[3] *= layer.get("opacity").and_then(|o| o.as_f64()).unwrap_or(1.0).clamp(0.0, 1.0);
                        backing = color::over(c, backing);
                    }
                }
            }
            // Safe area for the layer's anchor point (top level only: children are laid out by their parent).
            check_safe(&mut cx, layer, &format!("{path}.layers[{li}]"));
        }
        if dur < 1.2 && !kinetic_only {
            cx.note("warning", "pacing", &path, format!("scene{id} lasts {dur:.2} s, too short to register"), "make it at least 1.5 s, or merge it with a neighbour (fast cuts work only for single words on a beat)");
        }
        if text_sizes.len() >= 2 {
            let (lo, hi) = text_sizes.iter().fold((f64::MAX, 0.0f64), |(a, b), x| (a.min(*x), b.max(*x)));
            if hi / lo.max(1.0) < 1.8 {
                cx.note("tip", "type-scale", &path, format!("{} text layers between {lo:.0} and {hi:.0} px: the hierarchy is flat", text_sizes.len()),
                    "when type leads, give the headline 4–25× the caption's size (30–60 vh over 2.2–3 vh); when it supports, use one small size consistently (docs type-craft)");
            }
        }
        if words_on_screen > 32 {
            cx.note("warning", "density", &path, format!("{words_on_screen} words in one scene; viewers skim, they don't read paragraphs"), "keep scenes under ~20 words; split the copy across scenes or move detail to a voice-over");
        }
        if dur > 8.0 && !has_motion {
            cx.note("warning", "pacing", &path, format!("scene{id} holds {dur:.1} s with no motion"), "add a slow camera push (zoom 1 → 1.05), a loop, or split it");
        } else if dur > 12.0 {
            cx.note("tip", "pacing", &path, format!("scene{id} is {dur:.1} s long"), "most motion-design beats are 2–6 s; consider splitting it");
        }
    }

    // Generated device screens: they must carry the product's own copy, not sample data.
    scan_placeholders(spec, "", &mut cx);

    match first_text_at {
        // Most world-class films lead with an image, not copy: late or absent
        // words are a choice, so this is a tip about the opening, not a rule.
        Some(t) if t > 2.0 => cx.note("tip", "hook", "scenes[0]", format!("the first words appear at {t:.1} s"), "fine when the opening image is the hook (viewers decide in about 2 s); otherwise put a line on screen by 0.3–1 s"),
        _ => {}
    }
    if fonts.len() > 3 {
        cx.note("tip", "typography", "theme.fonts", format!("{} font families in use ({})", fonts.len(), fonts.iter().cloned().collect::<Vec<_>>().join(", ")), "stick to one display face and one text face, plus mono for code");
    }
    if cx.portrait && total > 60.0 {
        cx.note("tip", "pacing", "scenes", format!("{total:.0} s vertical video; social platforms favour 6–30 s"), "cut to the hook, two beats and the payoff");
    }
    sameness(raw, spec, &mut cx);
    recipe_copy(raw, &mut cx);
    medium(raw, &mut cx);
    bar(spec, &mut cx);
    taste::run(raw, spec, &mut cx);
    if applied.is_some() {
        let off = crate::looks::layer_offsets(raw);
        for n in &mut cx.notes {
            n.path = crate::looks::remap_path(&off, &n.path);
        }
    }
    let score = cx.notes.iter().fold(100i32, |s, n| s - match n.severity { "error" => 15, "warning" => 6, _ => 1 }).max(0) as u32;
    Report { score, duration: (total * 100.0).round() / 100.0, notes: cx.notes, categories: Vec::new(), summary: String::new() }
}

/// Device mockups showing a generated screen without the product's copy (or a
/// dashboard with invented numbers) read as a different product.
/// Per-text-layer typography tells (advice, from docs/video/craft/typography.md).
fn type_checks(cx: &mut Ctx, l: &serde_json::Map<String, Value>, p: &str, text: &str, size: f64, words: usize) {
    let align = l.get("align").and_then(|a| a.as_str()).unwrap_or("");
    let left_anchor = l.get("anchor").and_then(|a| a.as_str()).is_some_and(|a| a.to_ascii_lowercase().contains("left"));
    let centred = matches!(align, "" | "center") && !(align.is_empty() && left_anchor);
    let lines = text.lines().filter(|x| !x.trim().is_empty()).count();
    let wraps = l.get("width").is_some() || l.get("maxWidth").is_some();
    if centred && (lines >= 3 || (wraps && words >= 14)) {
        cx.note("tip", "typography", p, format!("\"{}\" is a centred paragraph", trunc(text, 32)),
            "set multi-line text flush left on an edge or grid line (\"align\": \"left\" or a left anchor); centre only a word, a date or a logo");
    }
    let em = match l.get("letterSpacing") {
        Some(Value::String(s)) if s.trim().ends_with("em") => s.trim().trim_end_matches("em").trim().parse::<f64>().ok(),
        Some(Value::Number(n)) => n.as_f64().map(|px| px / size.max(1.0)),
        _ => None,
    };
    let upper = l.get("uppercase").and_then(|u| u.as_bool()).unwrap_or(false) || (text.chars().any(|c| c.is_alphabetic()) && !text.chars().any(|c| c.is_lowercase()));
    let font = l.get("font").and_then(|f| f.as_str()).unwrap_or("Inter").to_ascii_lowercase();
    if let Some(em) = em {
        if !upper && em > 0.06 && words >= 2 {
            cx.note("tip", "typography", p, format!("lowercase text tracked +{em:.2} em"),
                "lowercase tends to fall apart when tracked loose; keep it at −0.03…+0.02 em (track caps and small labels instead)");
        }
        if upper && em >= 0.15 && centred && (font.starts_with("inter") || font.starts_with('$')) && size > 20.0 {
            cx.note("tip", "typography", p, format!("\"{}\": widely tracked caps, centred, in the default face", trunc(text, 32)),
                "this reads as the generic 'cinematic' template; give the line a face with a voice, set it flush to a grid line, or drop the tracking");
        }
    }
}

fn scan_placeholders(v: &Value, path: &str, cx: &mut Ctx) {
    match v {
        Value::Object(m) => {
            let kind = m.get("component").and_then(|k| k.as_str()).unwrap_or("");
            let kind = kind.trim_start_matches("device.");
            if ["phone", "laptop", "browser"].contains(&kind) {
                if let Some(p) = m.get("props").and_then(|p| p.as_object()) {
                    let has_src = p.get("src").is_some_and(|s| s.as_str().is_some_and(|s| !s.is_empty()));
                    let items = p.get("items").and_then(|i| i.as_array()).map_or(0, |a| a.iter().filter(|x| x.as_str().is_some_and(|s| !s.is_empty())).count());
                    if !has_src {
                        if items == 0 {
                            cx.note("warning", "placeholder", path, format!("the {kind} shows a generated screen with sample content that isn't this product's"), "pass a screenshot (`src`), or the product's features as `items` so the screen shows its own copy");
                        } else if p.get("screen").and_then(|s| s.as_str()) == Some("dashboard") && p.get("kpis").is_none() {
                            cx.note("warning", "placeholder", path, format!("the {kind} dashboard shows invented numbers"), "pass real `kpis` ([{label, value}]) or use `screen: \"app\"`");
                        }
                    }
                }
            }
            for (k, x) in m {
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                scan_placeholders(x, &p, cx);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                scan_placeholders(x, &format!("{path}[{i}]"), cx);
            }
        }
        _ => {}
    }
}

fn transition_secs(cx: &Ctx, t: &Value) -> Option<f64> {
    match t {
        Value::String(s) if s == "cut" => Some(0.0),
        Value::String(s) if s == "glitch" => Some(0.3),
        Value::String(_) => Some(0.5),
        Value::Object(o) => {
            if o.get("type").and_then(|x| x.as_str()) == Some("cut") {
                return Some(0.0);
            }
            Some(o.get("duration").and_then(|d| cx.time(d)).unwrap_or(0.5))
        }
        _ => None,
    }
}

/// Substitute a counter's `{value…}` placeholder with its final value.
fn with_value(text: &str, value: Option<&Value>) -> String {
    let (Some(a), Some(v)) = (text.find("{value"), value.and_then(|v| v.as_f64())) else { return text.to_string() };
    let Some(b) = text[a..].find('}') else { return text.to_string() };
    let digits = format!("{}", v.round() as i64);
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 && c.is_ascii_digit() {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{}{}{}", &text[..a], grouped, &text[a + b + 1..])
}

fn trunc(s: &str, n: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n - 1).collect::<String>())
    } else {
        one
    }
}

type Visit<'f> = dyn FnMut(&mut Ctx, &Map<String, Value>, &str, f64, f64, color::C) + 'f;

/// Visit a layer and its children with their visible window in scene time.
fn walk_layer(cx: &mut Ctx, layer: &Value, path: &str, scene_dur: f64, bg: color::C, parent_start: f64, f: &mut Visit) {
    let Some(l) = layer.as_object() else { return };
    if l.get("visible") == Some(&Value::Bool(false)) {
        return;
    }
    let start = parent_start + l.get("start").and_then(|v| cx.time(v)).unwrap_or(0.0);
    let mut end = scene_dur;
    if let Some(e) = l.get("end").and_then(|v| cx.time(v)) {
        end = (parent_start + e).min(scene_dur);
    } else if let Some(d) = l.get("duration").and_then(|v| cx.time(v)) {
        end = (start + d).min(scene_dur);
    }
    // Entrances delay legibility until they have mostly played.
    let enter_delay = match l.get("enter") {
        Some(Value::Object(o)) => {
            let d = o.get("delay").and_then(|v| cx.time(v)).unwrap_or(0.0);
            let dur = o.get("duration").and_then(|v| cx.time(v)).unwrap_or(0.6);
            let n = l.get("text").and_then(|t| t.as_str()).map(|t| match l.get("split").and_then(|s| s.as_str()) {
                Some("words") => t.split_whitespace().count(),
                Some("chars") => t.chars().count(),
                Some("lines") => t.lines().count(),
                _ => 1,
            }).unwrap_or(1);
            let stagger = o.get("stagger").and_then(|v| v.as_f64()).unwrap_or(0.0) * (n.saturating_sub(1)) as f64;
            if o.get("preset").and_then(|p| p.as_str()) == Some("typewriter") {
                let cps = o.get("cps").and_then(|v| v.as_f64()).unwrap_or(20.0);
                d + l.get("text").and_then(|t| t.as_str()).map(|t| t.chars().count() as f64 / cps).unwrap_or(0.0)
            } else {
                // Strong ease-outs are legible after about a third of their duration.
                let ease = o.get("ease").and_then(|e| e.as_str()).unwrap_or("");
                let k = if ["outExpo", "outQuart", "outQuint", "outCirc"].iter().any(|e| ease.eq_ignore_ascii_case(e)) { 0.35 } else { 0.5 };
                d + dur * k + stagger
            }
        }
        Some(Value::String(_)) => 0.3,
        _ => 0.0,
    };
    let bg_here = l
        .get("style")
        .and_then(|s| s.as_object())
        .and_then(|s| s.get("background"))
        .and_then(|b| cx.color(b))
        .map(|c| color::over(c, bg))
        .unwrap_or(bg);
    f(cx, l, path, start + enter_delay, end, bg_here);
    for key in ["children", "layers"] {
        if let Some(kids) = l.get(key).and_then(|k| k.as_array()) {
            // A filled shape inside a group backs the siblings drawn after it
            // (a pill or card behind its label).
            let mut backing = bg_here;
            for (i, k) in kids.iter().enumerate() {
                walk_layer(cx, k, &format!("{path}.{key}[{i}]"), scene_dur, backing, start, f);
                if k.get("shape").is_some() && k.get("text").is_none() {
                    if let Some(mut c) = k.get("fill").or_else(|| k.get("color")).and_then(|c| cx.color(c)) {
                        c[3] *= k.get("opacity").and_then(|o| o.as_f64()).unwrap_or(1.0).clamp(0.0, 1.0);
                        backing = color::over(c, backing);
                    }
                }
            }
        }
    }
}

/// Text and images whose anchor point sits outside the title-safe area.
fn check_safe(cx: &mut Ctx, layer: &Value, path: &str) {
    let Some(l) = layer.as_object() else { return };
    if l.get("text").is_none() && l.get("src").is_none() {
        return;
    }
    let [top, right, bottom, left] = cx.safe;
    let x = l.get("x").and_then(|v| cx.len(v, cx.w)).unwrap_or(cx.w / 2.0);
    let y = l.get("y").and_then(|v| cx.len(v, cx.h)).unwrap_or(cx.h / 2.0);
    let width = l.get("width").and_then(|v| cx.len(v, cx.w)).unwrap_or(0.0);
    let anchor_x = match l.get("anchor").and_then(|a| a.as_str()) {
        Some(a) if a.to_ascii_lowercase().contains("left") => 0.0,
        Some(a) if a.to_ascii_lowercase().contains("right") => 1.0,
        _ => 0.5,
    };
    let x0 = x - width * anchor_x;
    let x1 = x0 + width;
    let what = if l.get("text").is_some() { "text" } else { "image" };
    if x0 < left - 1.0 || x1 > cx.w - right + 1.0 || y < top - 1.0 || y > cx.h - bottom + 1.0 {
        let extra = if cx.portrait { " (on 9:16, platform UI covers the top 12% and bottom 20%)" } else { "" };
        cx.note("warning", "safe-area", path,
            format!("{what} at ({x:.0}, {y:.0}){} is outside the title-safe area{extra}", if width > 0.0 { format!(", {width:.0} px wide") } else { String::new() }),
            format!("keep it within x {left:.0}–{:.0}, y {top:.0}–{:.0}", cx.w - right, cx.h - bottom));
    }
}

/// The quality bar (`docs/video/quality/BAR.md`): patterns a viewer reads as slides or slop.
fn bar(spec: &Value, cx: &mut Ctx) {
    const WASH: &[&str] = &["mesh-gradient", "aurora", "gradient", "bokeh", "light-leak"];
    let scenes = spec.get("scenes").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let mut slides = Vec::new();
    for (si, sc) in scenes.iter().enumerate() {
        let layers = sc.get("layers").and_then(|l| l.as_array()).cloned().unwrap_or_default();
        let wash = layers.iter().any(|l| l.get("component").and_then(|c| c.as_str()).is_some_and(|c| WASH.contains(&c)));
        let rest: Vec<&Value> = layers.iter().filter(|l| !l.get("component").and_then(|c| c.as_str()).is_some_and(|c| WASH.contains(&c) || c == "grain")).collect();
        let centred_text_only = !rest.is_empty() && rest.iter().all(|l| l.get("text").is_some() && l.get("x").is_none() && l.get("component").is_none());
        if wash && centred_text_only {
            slides.push(format!("scenes[{si}]"));
        }
    }
    if !slides.is_empty() {
        cx.note("warning", "bar", &slides[0], format!("centred text on a gradient or blob backdrop ({} scene{}): reads as a slide (BAR #1–2)", slides.len(), if slides.len() == 1 { "" } else { "s" }), "compose the frame: a subject with light and material (3D, a shader, photography), type placed with intent, the frame filled");
    }
    let dims = |v: &Value| -> bool {
        fn walk(v: &Value) -> bool {
            match v {
                Value::Object(m) => {
                    m.contains_key("shader")
                        || m.get("type").and_then(|t| t.as_str()) == Some("shader")
                        || m.get("component").and_then(|c| c.as_str()).is_some_and(|c| ["text-3d", "model", "globe", "scene3d", "ken-burns"].contains(&c))
                        || m.get("aperture").is_some()
                        || m.get("src").and_then(|s| s.as_str()).is_some_and(|s| !s.ends_with(".svg"))
                        || m.values().any(walk)
                }
                Value::Array(a) => a.iter().any(walk),
                _ => false,
            }
        }
        walk(v)
    };
    if scenes.len() >= 2 && !dims(spec) {
        cx.note("tip", "bar", "scenes", "flat 2D only: no light, lens, texture or real imagery anywhere (BAR #5)".into(), "flat 2D is a valid choice when it is designed (strict grid, scale contrast, value structure, real UI or photos); if the concept is spatial, add depth (camera depth + aperture, a shader or scene3d world). Don't force 3D (TASTE.md §3, §8)");
    }
}

/// Generic defaults make every video look the same: flag them and suggest a direction.
/// Leaf `key=value` pairs that make a spec's design (words and timing aside).
fn design_leaves(v: &Value, out: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if matches!(k.as_str(), "text" | "about" | "cost" | "duration" | "start" | "end" | "at") {
                    continue;
                }
                match x {
                    Value::String(s) => {
                        out.insert(format!("{k}={s}"));
                    }
                    Value::Number(n) => {
                        out.insert(format!("{k}={n}"));
                    }
                    _ => design_leaves(x, out),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| design_leaves(x, out)),
        _ => {}
    }
}

/// A spec that is mostly a recipe with new words: say so, since the recipe
/// is a technique to borrow, not the piece.
fn recipe_copy(raw: &Value, cx: &mut Ctx) {
    let mut mine = std::collections::BTreeSet::new();
    design_leaves(raw, &mut mine);
    for (name, src) in crate::agent_docs::RECIPES {
        let Ok(r) = serde_json::from_str::<Value>(src) else { continue };
        let mut theirs = std::collections::BTreeSet::new();
        design_leaves(&r["spec"], &mut theirs);
        if theirs.len() < 12 {
            continue;
        }
        let common = theirs.intersection(&mine).count() as f64;
        let (shared, of_mine) = (common / theirs.len() as f64, common / mine.len().max(1) as f64);
        // Most of the recipe, making up most of this spec: one borrowed shot in
        // a bigger piece is fine.
        if shared >= 0.6 && of_mine >= 0.5 {
            cx.note("warning", "recipe", "scenes", format!("{:.0}% of this spec's design is the `{name}` recipe with new words", shared * 100.0), "build the shots from this brief's own nouns: its object as the 3D hero, its data as particles or physics, its world as the shader; borrow the recipe's technique, not its shots");
            return;
        }
    }
}

/// 3D and music chosen by habit rather than by the idea: the same set and
/// materials in every 3D scene, words traced under defocus or at reduced
/// resolution, and electronic music on a calm piece.
fn medium(raw: &Value, cx: &mut Ctx) {
    let scenes = raw.get("scenes").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let mut sets = Vec::new();
    for (si, sc) in scenes.iter().enumerate() {
        let Some(layers) = sc.get("layers").and_then(|l| l.as_array()) else { continue };
        for (li, l) in layers.iter().enumerate() {
            if !l.get("component").and_then(|c| c.as_str()).is_some_and(|c| c == "scene3d" || c == "scene-3d") {
                continue;
            }
            let p = l.get("props").cloned().unwrap_or(Value::Null);
            let env = p.get("env").map(|e| e.to_string()).unwrap_or_default();
            let mut mats = std::collections::BTreeSet::new();
            let mut text = false;
            if let Some(objs) = p.get("objects").and_then(|o| o.as_array()) {
                for o in objs {
                    if let Some(m) = o.get("material") {
                        mats.insert(m.as_str().map(str::to_string).unwrap_or_else(|| m.to_string()));
                    }
                    text |= o.get("text").is_some();
                }
            }
            let path = format!("scenes[{si}].layers[{li}]");
            let res = p.get("resolution").and_then(|r| r.as_f64()).unwrap_or(1.0);
            let defocus = ["aperture", "dof", "focus"].iter().any(|k| p.get(*k).is_some() || p.get("camera").and_then(|c| c.get(*k)).is_some());
            if text && (res < 1.0 || defocus) {
                cx.note("warning", "legibility", &path, format!("3D words are traced {}; words read best crisp, at full resolution and in focus", if res < 1.0 { format!("at resolution {res}") } else { "under depth of field".into() }),
                    "drop `resolution` (drafts reduce it automatically) and keep the words in the focal plane, or set the words as a 2D text layer over the 3D");
            }
            sets.push((path, env, mats));
        }
    }
    let studio = |e: &str| ["studio", "cyc"].iter().any(|k| e.contains(k));
    let shiny = |m: &std::collections::BTreeSet<String>| m.iter().any(|x| ["chrome", "glass", "gold", "metal"].iter().any(|k| x.contains(k)));
    if sets.len() >= 2 && sets.iter().all(|(_, e, m)| *e == sets[0].1 && *m == sets[0].2) {
        cx.note("warning", "sameness", &sets[0].0, format!("every 3D scene uses the same set and materials ({} {})", sets[0].1, sets[0].2.iter().cloned().collect::<Vec<_>>().join(", ")),
            "use 3D only where the idea is physical, and give each 3D shot its own world; tell the rest in 2D type, illustration or shaders");
    } else if let Some((p, _, _)) = sets.iter().find(|(_, e, m)| studio(e) && shiny(m)) {
        cx.note("tip", "sameness", p, "a studio/cyc set with chrome or glass is the default 3D look".into(), "build the set from the brief's world (its materials, place and light), or tell this shot in 2D");
    }
    let style = match raw.get("music") {
        Some(Value::String(s)) => s.clone(),
        Some(m) => m.get("style").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        None => String::new(),
    };
    let pace = raw.get("intent").and_then(|i| i.get("pace")).and_then(|p| p.as_str()).unwrap_or("");
    if ["synthwave", "house", "trap", "dnb", "tech"].contains(&style.as_str()) && matches!(pace, "calm" | "steady") {
        cx.note("tip", "audio", "music", format!("electronic `{style}` score on a {pace} piece"), "acoustic, orchestral, piano or ambient usually suit a calm or brand piece; keep electronic for electronic concepts");
    }
}

fn sameness(raw: &Value, spec: &Value, cx: &mut Ctx) {
    let scenes = spec.get("scenes").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    if scenes.len() < 2 {
        return;
    }
    let has_look = raw.get("look").is_some_and(|l| l.as_str() != Some("none") && *l != Value::Bool(false));
    let theme = spec.get("theme");
    let mut signals = Vec::new();
    // Colours and type set on the layers themselves count as a point of view too.
    let mut hexes = std::collections::BTreeSet::new();
    let mut styled_type = false;
    fn walk(v: &Value, hexes: &mut std::collections::BTreeSet<String>, styled: &mut bool) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    if let Value::String(s) = x {
                        if matches!(k.as_str(), "color" | "fill" | "background" | "stroke") && s.starts_with('#') {
                            hexes.insert(s.to_lowercase());
                        }
                        if k == "font" && !s.starts_with("Inter") {
                            *styled = true;
                        }
                    }
                    if matches!(k.as_str(), "letterSpacing" | "weight") && m.contains_key("text") {
                        *styled = true;
                    }
                    walk(x, hexes, styled);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, hexes, styled)),
            _ => {}
        }
    }
    walk(spec.get("scenes").unwrap_or(&Value::Null), &mut hexes, &mut styled_type);
    let own_palette = hexes.len() >= 3;
    if !has_look {
        let primary = theme.and_then(|t| t.pointer("/colors/primary")).and_then(|p| p.as_str()).and_then(|p| color::parse(p).ok());
        let house = match primary {
            None => true,
            // Violet on near-black: the default every tool ships.
            Some(c) => {
                let (h, sat) = hue_sat(c);
                let bg = cx.color(&Value::String("$bg".into())).unwrap_or([0.0; 4]);
                (240.0..290.0).contains(&h) && sat > 0.4 && color::luminance(bg) < 0.03
            }
        };
        // Layer colours excuse a missing theme, not a violet one.
        // A film whose colour comes from shaders, 3D or images isn't using the
        // default palette, whatever the (unused) theme says.
        let pixel_film = raw.to_string().contains("\"shader\"") || raw.to_string().contains("scene3d");
        if house && (primary.is_some() || (!own_palette && !pixel_film)) {
            signals.push("the house palette (violet on near-black)");
        }
        let fonts = theme.and_then(|t| t.get("fonts")).and_then(|f| f.as_object());
        let custom_type = fonts.is_some_and(|f| f.values().any(|v| v.as_str().is_some_and(|s| !s.starts_with("Inter") && !s.contains("JetBrains"))))
            || theme.and_then(|t| t.get("styles")).is_some_and(|s| s.as_object().is_some_and(|m| !m.is_empty()));
        if !custom_type && !styled_type {
            signals.push("default typography (Inter, stock sizes and weights; `anikit-video fonts` lists ~130 faces)");
        }
    }
    let first_kind = |sc: &Value| sc.get("layers").and_then(|l| l.as_array()).and_then(|l| l.first()).and_then(|l| l.get("component")).and_then(|c| c.as_str()).map(String::from);
    if !has_look && scenes.iter().all(|sc| first_kind(sc).as_deref() == Some("mesh-gradient")) {
        signals.push("the same mesh-gradient backdrop in every scene");
    }
    // One entrance everywhere.
    let mut enters = std::collections::BTreeMap::<String, usize>::new();
    let mut texts = 0;
    for sc in &scenes {
        for l in sc.get("layers").and_then(|l| l.as_array()).into_iter().flatten() {
            if l.get("text").is_some() {
                texts += 1;
                let e = match l.get("enter") {
                    Some(Value::String(s)) => s.clone(),
                    Some(o) => o.get("preset").and_then(|p| p.as_str()).unwrap_or("custom").to_string(),
                    None => "none".into(),
                };
                *enters.entry(e).or_default() += 1;
            }
        }
    }
    if texts >= 6 && enters.values().any(|&n| n * 10 >= texts * 9) {
        signals.push("one text entrance everywhere");
    }
    // Layout repeated: consecutive scenes with the same layer signature.
    let sig = |sc: &Value| -> String {
        sc.get("layers").and_then(|l| l.as_array()).map(|ls| ls.iter().map(|l| l.get("component").or(l.get("style")).or(l.get("shape")).and_then(|x| x.as_str()).unwrap_or("?").to_string()).collect::<Vec<_>>().join(",")).unwrap_or_default()
    };
    let sigs: Vec<String> = scenes.iter().map(sig).collect();
    if sigs.windows(3).any(|w| w[0] == w[1] && w[1] == w[2] && !w[0].is_empty()) {
        signals.push("three scenes in a row with the same layout");
    }
    if signals.is_empty() {
        return;
    }
    // A direction to try: two looks, picked from the copy so suggestions vary.
    let words = crate::critique::all_text(spec);
    let h = words.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    let names = crate::looks::names();
    let (a, b) = (names[(h % names.len() as u64) as usize], names[((h / 7 + 3) % names.len() as u64) as usize]);
    let sev = if signals.len() >= 2 { "warning" } else { "tip" };
    cx.note(
        sev,
        "sameness",
        "spec",
        format!("generic: {}; this will look like every other generated video", signals.join("; ")),
        if has_look {
            "vary layouts and entrances per beat, and build one signature visual from the brief's own subject (3D, paths, real footage)".to_string()
        } else {
            format!("give it a point of view: set \"look\" (e.g. \"{a}\" or \"{b}\"; anikit-video looks), vary layouts and entrances per beat, and build one signature visual (3D, paths, real footage)")
        },
    );
}

fn hue_sat(c: color::C) -> (f64, f64) {
    let (r, g, b) = (c[0], c[1], c[2]);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    if d < 1e-6 {
        return (0.0, 0.0);
    }
    let h = if mx == r { 60.0 * (((g - b) / d) % 6.0) } else if mx == g { 60.0 * ((b - r) / d + 2.0) } else { 60.0 * ((r - g) / d + 4.0) };
    ((h + 360.0) % 360.0, d / mx)
}

/// All visible copy, for seeding suggestions.
pub fn all_text(v: &Value) -> String {
    let mut out = String::new();
    fn walk(v: &Value, out: &mut String) {
        match v {
            Value::Object(m) => {
                if let Some(Value::String(t)) = m.get("text") {
                    out.push_str(t);
                }
                m.values().for_each(|x| walk(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(v, &mut out);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn text_on_a_full_frame_rect_uses_the_rect() {
        let spec = serde_json::json!({ "scenes": [{ "duration": 3, "background": "#f4f1ea", "layers": [ { "shape": "rect", "width": "100%", "height": "100%", "fill": "#1d3fbb" }, { "text": "Cobalt", "color": "#ffffff" } ] }] });
        let r = critique(&spec);
        assert!(!r.notes.iter().any(|n| n.check == "contrast"), "{}", r.to_text());
    }

    #[test]
    fn text_on_a_backing_shape_in_a_group_uses_the_shape() {
        let spec = serde_json::json!({ "scenes": [{ "duration": 3, "background": "#f4f1ea", "layers": [
            { "layers": [ { "shape": "rect", "width": 400, "height": 90, "fill": "#111111" }, { "text": "Ship it", "color": "#f4f1ea" } ] }
        ] }] });
        let r = critique(&spec);
        assert!(!r.notes.iter().any(|n| n.check == "contrast"), "{}", r.to_text());
    }

    #[test]
    fn motion_tells_are_flagged() {
        let t = |extra: serde_json::Value| {
            let mut l = serde_json::json!({ "text": "A", "enter": "fadeUp" });
            l.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            l
        };
        let spec = serde_json::json!({ "scenes": [{ "duration": 3, "layers": [
            t(serde_json::json!({ "animate": { "to": { "x": 900 }, "ease": "linear" } })), t(serde_json::json!({})), t(serde_json::json!({})), t(serde_json::json!({})) ] }] });
        let r = critique(&spec);
        let m: Vec<_> = r.notes.iter().filter(|n| n.check == "motion").map(|n| n.message.clone()).collect();
        assert!(m.iter().any(|x| x.contains("linear ease")), "{m:?}");
        assert!(m.iter().any(|x| x.contains("same frame")), "{m:?}");
        let calm = serde_json::json!({ "scenes": [{ "duration": 3, "layers": [ { "type": "group", "stagger": 0.08, "layers": [
            { "text": "A", "enter": "fadeUp", "inertia": "tight" }, { "text": "B", "enter": "fadeUp" } ] } ] }] });
        assert!(!critique(&calm).notes.iter().any(|n| n.check == "motion"), "{}", critique(&calm).to_text());
    }

    #[test]
    fn habitual_3d_and_music_are_flagged() {
        let sc = |m: &str| serde_json::json!({ "duration": 2, "layers": [{ "component": "scene3d", "props": { "env": "studio", "aperture": 2.8, "objects": [{ "text": "SIFT", "material": m }] } }] });
        let spec = serde_json::json!({ "intent": {"pace": "calm"}, "music": "synthwave", "scenes": [sc("chrome"), sc("chrome")] });
        let r = critique(&spec);
        let checks: Vec<_> = r.notes.iter().map(|n| (n.check, n.message.clone())).collect();
        assert!(r.notes.iter().any(|n| n.check == "legibility" && n.message.contains("depth of field")), "{checks:?}");
        assert!(r.notes.iter().any(|n| n.check == "sameness" && n.message.contains("same set")), "{checks:?}");
        assert!(r.notes.iter().any(|n| n.check == "audio" && n.message.contains("synthwave")), "{checks:?}");
    }

    #[test]
    fn recipe_with_new_words_is_flagged() {
        let (_, spec) = crate::agent_docs::recipe_spec("launch-sting").unwrap();
        let r = critique(&spec);
        assert!(r.notes.iter().any(|n| n.check == "recipe"), "{:?}", r.notes);
        let own = serde_json::json!({"scenes": [{"duration": 2, "layers": [{"text": "Hi", "font": "Fraunces", "color": "#ff5a1f"}]}, {"duration": 2, "layers": [{"shape": "rect", "fill": "#111111"}]}]});
        assert!(!critique(&own).notes.iter().any(|n| n.check == "recipe"));
        // One borrowed shot inside a bigger piece is a technique, not a copy.
        let (_, whip) = crate::agent_docs::recipe_spec("whip-transition").unwrap();
        let mut big = spec.clone();
        big["scenes"].as_array_mut().unwrap().extend(whip["scenes"].as_array().unwrap().iter().cloned());
        let r = critique(&big);
        assert!(!r.notes.iter().any(|n| n.check == "recipe" && n.message.contains("whip")), "{:?}", r.notes);
    }

    #[test]
    fn sameness_flags_generic_defaults_not_a_look() {
        let generic = serde_json::json!({ "theme": { "colors": { "primary": "#7C5CFF" } }, "scenes": [
            { "layers": [ { "component": "mesh-gradient" }, { "text": "One", "style": "title" } ] },
            { "layers": [ { "component": "mesh-gradient" }, { "text": "Two", "style": "title" } ] },
            { "layers": [ { "component": "mesh-gradient" }, { "text": "Three", "style": "title" } ] } ] });
        let r = super::critique(&generic);
        let n = r.notes.iter().find(|n| n.check == "sameness").expect("flagged");
        assert_eq!(n.severity, "warning");
        assert!(n.message.contains("house palette") && n.message.contains("mesh-gradient") && n.fix.contains("\"look\""), "{n:?}");
        let mut directed = generic.clone();
        directed["look"] = serde_json::json!("editorial");
        directed.as_object_mut().unwrap().remove("theme");
        for s in directed["scenes"].as_array_mut().unwrap() {
            s["layers"].as_array_mut().unwrap().remove(0);
        }
        directed["scenes"][1]["layers"][0]["style"] = serde_json::json!("heading");
        assert!(super::critique(&directed).notes.iter().all(|n| n.check != "sameness"));
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn flags_reading_time_contrast_hook_and_safe_area() {
        let spec = json!({
            "size": "9:16",
            "scenes": [
                { "duration": 4, "layers": [ { "shape": "rect" } ] },
                { "duration": 1.5, "layers": [
                    { "text": "This sentence has far too many words to read in a second and a half", "width": "80%" },
                    { "text": "Low contrast", "color": "#151520" },
                    { "text": "Edge", "y": "97%" }
                ] }
            ]
        });
        let r = critique(&spec);
        let checks: Vec<&str> = r.notes.iter().map(|n| n.check).collect();
        assert!(checks.contains(&"reading-time"), "{}", r.to_text());
        assert!(checks.contains(&"contrast"), "{}", r.to_text());
        assert!(checks.contains(&"hook"), "{}", r.to_text());
        assert!(checks.contains(&"safe-area"), "{}", r.to_text());
        assert!(r.score < 80);
        assert!((r.duration - 5.0).abs() < 1e-9, "{}", r.duration);
    }

    #[test]
    fn typography_tells() {
        let spec = json!({ "scenes": [ { "duration": 6, "layers": [
            { "text": "THE FUTURE OF WORK", "size": "6vh", "letterSpacing": "0.3em" },
            { "text": "One line\nanother line\nand a third", "size": "4vh", "y": "65%" },
            { "text": "a lowercase tracked line", "size": "4vh", "letterSpacing": "0.12em", "y": "85%" } ] } ] });
        let t = critique(&spec).to_text();
        for want in ["widely tracked caps", "centred paragraph", "lowercase text tracked", "hierarchy is flat"] {
            assert!(t.contains(want), "{want} missing in\n{t}");
        }
    }

    #[test]
    fn clean_spec_scores_high() {
        let spec = json!({ "scenes": [ { "duration": 3, "layers": [ { "text": "Hello, world", "enter": "fadeUp" } ] } ] });
        let r = critique(&spec);
        assert!(r.notes.is_empty(), "{}", r.to_text());
        assert_eq!(r.score, 100);
    }
}

#[cfg(test)]
mod template_quality {
    use crate::templates::{brand::Brand, Format, Template, BUILTIN};

    /// Every template, with its example copy, must pass the critique without
    /// errors or warnings in every format: templates are the quality bar.
    #[test]
    fn templates_pass_critique() {
        let mut failures = String::new();
        for (name, _) in BUILTIN {
            let t = Template::builtin(name).unwrap();
            for f in ["16:9", "9:16", "1:1"] {
                for b in [Brand::default(), Brand::from_json(r##"{"theme":"light","colors":{"primary":"#0F766E"},"logo":"logo.svg"}"##).unwrap()] {
                    let spec = t.expand(&t.example_params(), &b, Format::parse(f).unwrap()).unwrap();
                    let r = super::critique(&spec);
                    // Templates are flat 2D quick starts: the quality bar (BAR #5) flags that honestly.
                    let bad: Vec<_> = r.notes.iter().filter(|n| n.severity != "tip" && n.check != "bar").collect();
                    if !bad.is_empty() {
                        failures += &format!("--- {name} {f} {}\n", b.theme.as_deref().unwrap_or("dark"));
                        for n in bad {
                            failures += &format!("{} {} {}: {}\n", n.severity, n.check, n.path, n.message);
                        }
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures}");
    }
}
