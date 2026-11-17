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
        "reading-time" | "legibility" | "contrast" | "render-contrast" | "typography" | "density" | "render-overlap" => "text",
        "hook" | "structure" | "placeholder" | "copy" => "story",
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
        let mut s = format!("score {}/100 (not a quality verdict) · {:.1} s", self.score, self.duration);
        for (c, v) in &self.categories {
            s += &format!(" · {c} {v}");
        }
        s += "\n";
        if !self.summary.is_empty() {
            s += &self.summary;
            s += "\n";
        }
        let rank = |n: &&Note| match n.severity { "error" => 0, "warning" => 1, _ => 2 };
        let mut v: Vec<&Note> = self.notes.iter().collect();
        v.sort_by_key(rank);
        let shown = if detail { v.len() } else { v.len().min(6) };
        for n in &v[..shown] {
            s += &format!("{:<7} {:<13} {}: {}\n        fix: {}\n", n.severity, n.check, n.path, n.message, n.fix);
        }
        if shown < v.len() {
            s += &format!("… {} more (--detail)\n", v.len() - shown);
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
            r.notes.extend(crate::render_check::check_spec(spec, &p.ev));
            let beats = crate::review::music_beats(spec, base);
            let intent = crate::review::intent(spec, beats.as_ref().map(|b| b.bpm));
            let samples = crate::review::Samples::take(&p.ev, 6.0, 192);
            let (tl, notes) = crate::review::time_axis(&p.ev, &samples, &intent, beats.as_ref().map(|b| &b.beats[..]));
            r.summary = crate::review::summary(&intent, &tl);
            r.notes.extend(notes);
            let (cn, worst) = crate::review::composition(&p.ev, &samples, &tl);
            r.notes.extend(cn);
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
        let mut words_on_screen = 0usize;
        let mut has_motion = scene.get("camera").is_some();
        let mut kinetic_only = true;
        for (li, layer) in layers.iter().enumerate() {
            walk_layer(&mut cx, layer, &format!("{path}.layers[{li}]"), dur, bg, 0.0, &mut |cx, l, p, vis_from, vis_to, bg| {
                if l.get("enter").is_some() || l.get("animate").is_some() || l.get("loop").is_some() {
                    has_motion = true;
                }
                if let Some(f) = l.get("font").and_then(|f| f.as_str()) {
                    fonts.insert(f.to_string());
                }
                // Components that show words (a terminal, a prompt, a title card) count for the hook.
                if first_text_at.is_none() && l.get("component").is_some() {
                    if let Some(p) = l.get("props").and_then(|p| p.as_object()) {
                        if ["text", "title", "lines", "code", "label", "words", "messages", "items"].iter().any(|k| p.get(*k).is_some_and(|v| !v.is_null())) {
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
                let need = if words <= 2 && text.chars().count() <= 14 && visible < 1.5 { 0.35 + 0.15 * words as f64 } else { expr::read_time(text) };
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
                if let Some(fg) = cx.color(&col_v) {
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
            // Safe area for the layer's anchor point (top level only: children are laid out by their parent).
            check_safe(&mut cx, layer, &format!("{path}.layers[{li}]"));
        }
        if dur < 1.2 && !kinetic_only {
            cx.note("warning", "pacing", &path, format!("scene{id} lasts {dur:.2} s, too short to register"), "make it at least 1.5 s, or merge it with a neighbour (fast cuts work only for single words on a beat)");
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
        Some(t) if t > 2.0 => cx.note("warning", "hook", "scenes[0]", format!("the first words appear at {t:.1} s; viewers decide in the first 2 s"), "open with the hook line on screen by 0.3–1 s"),
        None if !scenes.is_empty() => cx.note("tip", "hook", "scenes[0]", "the video has no text; make sure the opening image is a strong hook".into(), "consider a 3–6 word hook line in the first scene"),
        _ => {}
    }
    if fonts.len() > 3 {
        cx.note("tip", "typography", "theme.fonts", format!("{} font families in use ({})", fonts.len(), fonts.iter().cloned().collect::<Vec<_>>().join(", ")), "stick to one display face and one text face, plus mono for code");
    }
    if cx.portrait && total > 60.0 {
        cx.note("tip", "pacing", "scenes", format!("{total:.0} s vertical video; social platforms favour 6–30 s"), "cut to the hook, two beats and the payoff");
    }
    sameness(raw, spec, &mut cx);
    bar(spec, &mut cx);
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
            for (i, k) in kids.iter().enumerate() {
                walk_layer(cx, k, &format!("{path}.{key}[{i}]"), scene_dur, bg_here, start, f);
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
        cx.note("warning", "bar", "scenes", "flat 2D only: no 3D, light, lens or real imagery anywhere (BAR #5)".into(), "give the film a world with depth: a shader or text-3d/model/scene3d subject, camera depth + aperture, or real photos");
    }
}

/// Generic defaults make every video look the same: flag them and suggest a direction.
fn sameness(raw: &Value, spec: &Value, cx: &mut Ctx) {
    let scenes = spec.get("scenes").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    if scenes.len() < 2 {
        return;
    }
    let has_look = raw.get("look").is_some_and(|l| l.as_str() != Some("none") && *l != Value::Bool(false));
    let theme = spec.get("theme");
    let mut signals = Vec::new();
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
        if house {
            signals.push("the house palette (violet on near-black)");
        }
        let fonts = theme.and_then(|t| t.get("fonts")).and_then(|f| f.as_object());
        let custom_type = fonts.is_some_and(|f| f.values().any(|v| v.as_str().is_some_and(|s| !s.starts_with("Inter") && !s.contains("JetBrains"))))
            || theme.and_then(|t| t.get("styles")).is_some_and(|s| s.as_object().is_some_and(|m| !m.is_empty()));
        if !custom_type {
            signals.push("default typography (Inter, stock sizes and weights)");
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
