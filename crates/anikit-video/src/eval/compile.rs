//! Spec → compiled node tree (with diagnostics).
#![allow(clippy::type_complexity)]

use super::expr::Expr;
use super::theme::{merge_style, weight_value, Theme};
use crate::text::{self, FontStore};
use super::*;
use crate::spec::validate::{self as val, suggest};
use crate::spec::{self, Layer, Num, PresetObj, Spec, StyleRef};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// Inputs to [`Evaluator::new`] besides the spec.
#[derive(Clone)]
pub struct Options {
    /// Directory that relative asset paths resolve against.
    pub base_dir: PathBuf,
    /// Fonts (bundled fonts by default; share one store across renders).
    pub fonts: Option<Arc<FontStore>>,
    /// Decode images (off for `check`).
    pub load_assets: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { base_dir: PathBuf::from("."), fonts: None, load_assets: true }
    }
}

#[derive(Clone, Copy, Debug)]
enum ValS {
    Abs(f32),
    Add(f32),
    Mul(f32),
    Color([f32; 4]),
}

#[derive(Clone, Debug)]
struct TweenC {
    at: f32,
    dur: f32,
    ease: u32,
    from: Vec<(usize, usize, ValS)>,
    to: Vec<(usize, usize, ValS)>,
    /// (slot, channels, [(time 0..1, value)])
    kf: Vec<(usize, usize, Vec<(f32, ValS)>)>,
    repeat: i32,
    yoyo: bool,
}

#[derive(Clone, Copy)]
struct PCtx {
    size: [f32; 2],
    /// Scene time of the parent's start and end.
    t0: f32,
    end: f32,
    delay: f32,
    index: usize,
    count: usize,
    in_box: bool,
}

enum V {
    C(f32),
    E(Expr),
}

struct C<'a> {
    o: &'a Options,
    w: f32,
    h: f32,
    fps: f32,
    theme: Theme,
    d: Vec<Diagnostic>,
    easings: Vec<Easing>,
    ease_ids: HashMap<String, u32>,
    gmarkers: BTreeMap<String, f32>,
    smarkers: BTreeMap<String, f32>,
    scene_start: f32,
    scene_dur: f32,
    image_ids: HashMap<PathBuf, (u32, [f32; 2])>,
    assets: Arc<SharedAssets>,
}

fn size_preset(s: &str) -> Option<[u32; 2]> {
    Some(match s.to_ascii_lowercase().replace(' ', "").as_str() {
        "16:9" | "1080p" | "landscape" | "youtube" | "fhd" => [1920, 1080],
        "9:16" | "vertical" | "story" | "reels" | "tiktok" | "shorts" => [1080, 1920],
        "1:1" | "square" => [1080, 1080],
        "4:5" | "portrait" => [1080, 1350],
        "720p" | "hd" => [1280, 720],
        "1440p" | "2k" | "qhd" => [2560, 1440],
        "4k" | "2160p" | "uhd" => [3840, 2160],
        "480p" => [854, 480],
        "4:3" => [1440, 1080],
        "21:9" => [2520, 1080],
        _ => return None,
    })
}
const SIZE_NAMES: &[&str] = &["16:9", "9:16", "1:1", "4:5", "720p", "1080p", "1440p", "4k", "vertical", "square", "portrait"];

impl Evaluator {
    /// Compile a spec. Fails if there is any error; warnings are kept in `diagnostics`.
    pub fn new(spec: &Spec, opts: Options) -> Result<Evaluator, SpecErrors> {
        let (ev, d) = Self::build(spec, &opts);
        match ev {
            Some(mut e) if !d.iter().any(|x| x.severity == spec::Severity::Error) => {
                e.diagnostics = d;
                Ok(e)
            }
            _ => Err(SpecErrors(d)),
        }
    }

    /// Compile a spec, returning whatever could be built and every diagnostic.
    pub fn build(spec: &Spec, opts: &Options) -> (Option<Evaluator>, Vec<Diagnostic>) {
        let mut c = C {
            o: opts,
            w: 1920.0,
            h: 1080.0,
            fps: 30.0,
            theme: Theme::default(),
            d: Vec::new(),
            easings: vec![Easing::Linear],
            ease_ids: HashMap::new(),
            gmarkers: BTreeMap::new(),
            smarkers: BTreeMap::new(),
            scene_start: 0.0,
            scene_dur: 3.0,
            image_ids: HashMap::new(),
            assets: Arc::new(SharedAssets::new(opts.fonts.clone().unwrap_or_else(|| Arc::new(FontStore::new())))),
        };
        c.ease_ids.insert("linear".into(), 0);
        // Size & fps.
        match &spec.size {
            None => {}
            Some(spec::Size::Wh([w, h])) => {
                if *w < 2 || *h < 2 || *w > 16384 || *h > 16384 {
                    c.d.push(Diagnostic::error("size", format!("{w}×{h} is out of range (2..16384)")));
                } else {
                    (c.w, c.h) = (*w as f32, *h as f32);
                }
            }
            Some(spec::Size::Preset(s)) => match size_preset(s) {
                Some([w, h]) => (c.w, c.h) = (w as f32, h as f32),
                None => c.d.push(Diagnostic::error("size", format!("unknown size '{s}'")).suggest(suggest(s, SIZE_NAMES))),
            },
        }
        if let Some(f) = spec.fps {
            if !(1.0..=240.0).contains(&f) {
                c.d.push(Diagnostic::error("fps", format!("{f} is out of range (1..240)")));
            } else {
                c.fps = f as f32;
            }
        }
        let (theme, bad) = Theme::resolve(spec.theme.as_ref());
        c.theme = theme;
        for (k, v) in bad {
            c.d.push(Diagnostic::error(format!("theme.colors.{k}"), format!("invalid colour '{v}'")));
        }
        if let Some(m) = &spec.markers {
            for (k, v) in m {
                let t = c.time_raw(v, &format!("markers.{k}"));
                c.gmarkers.insert(k.clone(), t);
            }
        }
        let default_bg = spec.background.clone().unwrap_or(spec::Paint::Css("$bg".into()));
        let default_trans = spec.transition.clone();
        if spec.scenes.is_empty() {
            c.d.push(Diagnostic::error("scenes", "a video needs at least one scene"));
            return (None, c.d);
        }
        let mut scenes = Vec::new();
        let mut t = 0.0f32;
        let mut prev_dur = f32::INFINITY;
        let mut audio = Vec::new();
        for (i, s) in spec.scenes.iter().enumerate() {
            let path = format!("scenes[{i}]");
            let id = s.id.clone().unwrap_or_else(|| format!("scene{}", i + 1));
            let trans = if i == 0 {
                TransC { kind: TransKind::Cut, dur: 0.0, ease: 0, dir: [-1.0, 0.0], color: Rgba([0.0, 0.0, 0.0, 1.0]), origin: [0.0; 2], amount: 0.0 }
            } else {
                let (tr, tp) = match (&s.transition, &default_trans) {
                    (Some(t), _) => (Some(t), format!("{path}.transition")),
                    (None, Some(t)) => (Some(t), "transition".into()),
                    _ => (None, String::new()),
                };
                c.transition(tr, &tp)
            };
            // Explicit durations first (needed for global markers inside the scene).
            let auto = matches!(&s.duration, Some(Num::Str(x)) if x == "auto");
            let mut dur = match &s.duration {
                None => 3.0,
                Some(_) if auto => 1e6,
                Some(d) => c.time_raw(d, &format!("{path}.duration")),
            };
            if dur <= 0.0 {
                c.d.push(Diagnostic::error(format!("{path}.duration"), "must be positive"));
                dur = 3.0;
            }
            let mut trans = trans;
            if trans.dur > 0.0 && (trans.dur >= prev_dur || trans.dur >= dur) {
                c.d.push(Diagnostic::warning(
                    format!("{path}.transition.duration"),
                    format!("{:.2} s is not shorter than the scenes it joins; shortened", trans.dur),
                ));
                trans.dur = 0.5 * prev_dur.min(dur);
            }
            let start = t - trans.dur;
            c.scene_start = start;
            c.smarkers.clear();
            if let Some(m) = &s.markers {
                for (k, v) in m {
                    let tt = c.time_raw(v, &format!("{path}.markers.{k}"));
                    c.smarkers.insert(k.clone(), tt);
                }
            }
            let nd = c.d.len();
            let mut built = c.scene(s, &path, dur, &default_bg);
            if auto {
                let end = built.layers.iter().map(node_last).fold(0.0f32, f32::max);
                dur = (end + 0.5).max(0.5);
                c.d.truncate(nd);
                built = c.scene(s, &path, dur, &default_bg);
            }
            let (camera, shake, layers, bg, bg_color) = built.into_parts();
            if let Some(a) = &s.audio {
                for (k, cue) in a.iter().enumerate() {
                    if let Some(clip) = c.audio(cue, &format!("{path}.audio[{k}]"), start) {
                        audio.push(clip);
                    }
                }
            }
            scenes.push(SceneC { id, start, dur, bg, bg_color, trans, camera, shake, layers });
            t = start + dur;
            prev_dur = dur;
        }
        let mut duration = t;
        if let Some(d) = &spec.duration {
            duration = c.time_raw(d, "duration").max(1.0 / c.fps);
        }
        if let Some(a) = &spec.audio {
            for (k, cue) in a.iter().enumerate() {
                if let Some(clip) = c.audio(cue, &format!("audio[{k}]"), 0.0) {
                    audio.push(clip);
                }
            }
        }
        c.smarkers.clear();
        c.scene_start = 0.0;
        c.scene_dur = duration;
        let overlay = match &spec.captions {
            Some(cap) => c.captions(cap, duration),
            None => Vec::new(),
        };
        let ev = Evaluator {
            width: c.w as u32,
            height: c.h as u32,
            fps: c.fps,
            duration,
            diagnostics: Vec::new(),
            audio,
            assets: c.assets,
            easings: c.easings,
            scenes,
            overlay,
            ctheme: components_theme(&c.theme),
            theme: c.theme,
        };
        (Some(ev), c.d)
    }
}

/// Last moment anything in the node animates (scene time).
fn node_last(n: &Node) -> f32 {
    let mut e = n.start;
    for tr in &n.tracks {
        for s in &tr.segs {
            e = e.max(n.start + s.t1);
        }
    }
    let kids = match &n.kind {
        Kind::Group { children } | Kind::Box { children, .. } => children.as_slice(),
        _ => &[],
    };
    for k in kids {
        e = e.max(node_last(k));
    }
    if let Kind::Text { split: Some(sp), .. } = &n.kind {
        let md = sp.delays.iter().cloned().fold(0.0, f32::max);
        for tr in &sp.tracks {
            for s in &tr.segs {
                e = e.max(n.start + s.t1 + md);
            }
        }
    }
    if n.end.is_finite() && n.end < 1e5 {
        e = e.max(n.end);
    }
    e
}

struct Built {
    camera: Node,
    shake: Option<(f32, f32, f32, f32, f32)>,
    layers: Vec<Node>,
    bg: PaintC,
    bg_color: Rgba,
}
impl Built {
    fn into_parts(self) -> (Node, Option<(f32, f32, f32, f32, f32)>, Vec<Node>, PaintC, Rgba) {
        (self.camera, self.shake, self.layers, self.bg, self.bg_color)
    }
}

fn empty_node(kind: Kind) -> Node {
    Node {
        kind,
        start: 0.0,
        end: f32::INFINITY,
        base: prop::DEFAULTS,
        exprs: Vec::new(),
        tracks: Vec::new(),
        loops: Vec::new(),
        anchor: [0.5, 0.5],
        blend: Blend::Normal,
        shadow: None,
        glow: None,
        clip: false,
        wipe_dir: 0,
        index: 0.0,
        count: 1.0,
    }
}

impl C<'_> {
    // ---------- units ----------

    /// Seconds from a time value (no marker origin adjustment).
    fn time_raw(&mut self, n: &Num, path: &str) -> f32 {
        match self.time_m(n, path) {
            (t, Some(_)) => t,
            (t, None) => t,
        }
    }

    /// Seconds; for markers also returns the marker's scene time.
    fn time_m(&mut self, n: &Num, path: &str) -> (f32, Option<f32>) {
        match n {
            Num::Num(v) => (*v as f32, None),
            Num::Str(s) => {
                let s = s.trim();
                if let Some(m) = s.strip_prefix('@') {
                    let (name, off) = match m.find(['+', '-']) {
                        Some(i) => (&m[..i], m[i..].parse::<f32>().ok()),
                        None => (m, Some(0.0)),
                    };
                    let Some(off) = off else {
                        self.d.push(Diagnostic::error(path, format!("bad marker offset in '{s}'")));
                        return (0.0, None);
                    };
                    let st = if let Some(v) = self.smarkers.get(name) {
                        *v
                    } else if let Some(v) = self.gmarkers.get(name) {
                        *v - self.scene_start
                    } else {
                        let names: Vec<&String> = self.smarkers.keys().chain(self.gmarkers.keys()).collect();
                        self.d.push(Diagnostic::error(path, format!("unknown marker '{name}'")).suggest(suggest(name, &names)));
                        return (0.0, None);
                    };
                    return (st + off, Some(st + off));
                }
                let (num, mul) = if let Some(r) = s.strip_suffix("ms") {
                    (r, 0.001)
                } else if let Some(r) = s.strip_suffix('s') {
                    (r, 1.0)
                } else if let Some(r) = s.strip_suffix('f') {
                    (r, 1.0 / self.fps)
                } else {
                    (s, 1.0)
                };
                match num.trim().parse::<f32>() {
                    Ok(v) => (v * mul, None),
                    Err(_) => {
                        self.d.push(Diagnostic::error(path, format!("invalid time '{s}' (use 1.5, \"1.5s\", \"500ms\", \"12f\" or \"@marker\")")));
                        (0.0, None)
                    }
                }
            }
        }
    }

    /// Time relative to `origin` (scene time): markers resolve to scene time minus origin.
    fn time_rel(&mut self, n: &Num, path: &str, origin: f32) -> f32 {
        match self.time_m(n, path) {
            (_, Some(st)) => st - origin,
            (t, None) => t,
        }
    }

    fn parse_len(&self, s: &str, refl: f32, em: f32) -> Option<f32> {
        let s = s.trim();
        let (num, mul) = if let Some(r) = s.strip_suffix('%') {
            (r, refl / 100.0)
        } else if let Some(r) = s.strip_suffix("vmin") {
            (r, self.w.min(self.h) / 100.0)
        } else if let Some(r) = s.strip_suffix("vmax") {
            (r, self.w.max(self.h) / 100.0)
        } else if let Some(r) = s.strip_suffix("vw") {
            (r, self.w / 100.0)
        } else if let Some(r) = s.strip_suffix("vh") {
            (r, self.h / 100.0)
        } else if let Some(r) = s.strip_suffix("px") {
            (r, 1.0)
        } else if let Some(r) = s.strip_suffix("em") {
            (r, em)
        } else if let Some(r) = s.strip_suffix("deg") {
            (r, 1.0)
        } else {
            (s, 1.0)
        };
        num.trim().parse::<f32>().ok().map(|v| v * mul)
    }

    /// A length / number that may be an expression.
    fn len(&mut self, n: &Num, path: &str, refl: f32, em: f32) -> V {
        match n {
            Num::Num(v) => V::C(*v as f32),
            Num::Str(s) => {
                if let Some(e) = s.trim().strip_prefix('=') {
                    match Expr::compile(e, refl as f64, self.w as f64, self.h as f64) {
                        Ok(x) => match x.constant() {
                            Some(c) => V::C(c as f32),
                            None => V::E(x),
                        },
                        Err(m) => {
                            self.d.push(Diagnostic::error(path, format!("expression: {m}")));
                            V::C(0.0)
                        }
                    }
                } else {
                    match self.parse_len(s, refl, em) {
                        Some(v) => V::C(v),
                        None => {
                            self.d.push(Diagnostic::error(
                                path,
                                format!("invalid value '{s}' (use a number, \"50%\", \"10vw\", \"8vh\" or an expression \"=…\")"),
                            ));
                            V::C(0.0)
                        }
                    }
                }
            }
        }
    }

    fn len_c(&mut self, n: &Num, path: &str, refl: f32, em: f32) -> f32 {
        match self.len(n, path, refl, em) {
            V::C(v) => v,
            V::E(e) => {
                self.d.push(Diagnostic::warning(path, "expressions aren't animated here; using its value at t = 0"));
                e.eval(&expr::Vars { w: self.w as f64, h: self.h as f64, fps: self.fps as f64, ..Default::default() }) as f32
            }
        }
    }

    /// Set slot from a value (constant into base, expression into exprs).
    fn set(&mut self, n: &mut Node, slot: usize, val: Option<&Num>, path: &str, refl: f32) {
        if let Some(v) = val {
            match self.len(v, path, refl, 16.0) {
                V::C(c) => n.base[slot] = c,
                V::E(e) => n.exprs.push((slot as u8, e)),
            }
        }
    }

    // ---------- easing ----------

    fn ease(&mut self, name: Option<&str>, default: &str, path: &str) -> u32 {
        let name = name.unwrap_or(default);
        if let Some(&i) = self.ease_ids.get(name) {
            return i;
        }
        match Easing::parse(name) {
            Some(e) => {
                let i = self.easings.len() as u32;
                self.easings.push(e);
                self.ease_ids.insert(name.to_string(), i);
                i
            }
            None => {
                self.d.push(Diagnostic::error(path, format!("unknown easing '{name}'")).suggest(suggest(name, val::easings())));
                self.ease(Some(default), "linear", path)
            }
        }
    }

    /// Spring easing and its settle duration.
    fn spring(&mut self, s: &spec::SpringSpec, path: &str) -> (u32, f32) {
        let e = match s {
            spec::SpringSpec::Name(n) => {
                if val::check_word(n, val::SPRINGS).is_err() {
                    self.d.push(Diagnostic::error(path, format!("unknown spring '{n}'")).suggest(suggest(n, val::SPRINGS)));
                    Easing::parse("gentle")
                } else {
                    Easing::parse(n)
                }
            }
            spec::SpringSpec::Params(p) => Some(Easing::spring(
                anikit_core::spring::SpringParams::new(p.stiffness.unwrap_or(170.0), p.damping.unwrap_or(26.0), p.mass.unwrap_or(1.0)),
                p.velocity.unwrap_or(0.0),
            )),
        }
        .unwrap_or(Easing::Linear);
        let d = e.natural_duration().unwrap_or(1.0) as f32;
        let i = self.easings.len() as u32;
        self.easings.push(e);
        (i, d.clamp(0.05, 10.0))
    }

    // ---------- paint ----------

    fn color(&mut self, s: &str, path: &str) -> Option<Rgba> {
        match self.theme.color(s) {
            Some(c) => Some(c),
            None => {
                if let Some(t) = s.trim().strip_prefix('$') {
                    let names: Vec<&String> = self.theme.colors.keys().collect();
                    self.d.push(Diagnostic::error(path, format!("unknown colour token '${t}'")).suggest(suggest(t, &names).map(|x| format!("${x}"))));
                } else {
                    self.d.push(Diagnostic::error(path, format!("invalid colour '{s}'")));
                }
                None
            }
        }
    }

    /// Compile a paint. Returns the paint and its solid colour (for the colour slots).
    fn paint(&mut self, p: &spec::Paint, path: &str) -> Option<(PaintC, Rgba)> {
        match p {
            spec::Paint::Css(s) => {
                let st = s.trim();
                if st.eq_ignore_ascii_case("none") || st.eq_ignore_ascii_case("transparent") {
                    return None;
                }
                let low = st.to_ascii_lowercase();
                if low.starts_with("linear-gradient(") || low.starts_with("radial-gradient(") {
                    let radial = low.starts_with('r');
                    let inner = &st[st.find('(').unwrap() + 1..st.rfind(')').unwrap_or(st.len())];
                    let parts = split_top(inner);
                    let mut angle = 180.0;
                    let mut stops_src: &[&str] = &parts;
                    if let Some(first) = parts.first() {
                        let f = first.trim();
                        if let Some(a) = f.strip_suffix("deg") {
                            if let Ok(a) = a.trim().parse::<f32>() {
                                angle = a;
                                stops_src = &parts[1..];
                            }
                        } else if let Some(dir) = f.strip_prefix("to ") {
                            angle = match dir.trim() {
                                "top" => 0.0,
                                "right" => 90.0,
                                "bottom" => 180.0,
                                "left" => 270.0,
                                "top right" | "right top" => 45.0,
                                "bottom right" | "right bottom" => 135.0,
                                "bottom left" | "left bottom" => 225.0,
                                _ => 315.0,
                            };
                            stops_src = &parts[1..];
                        } else if radial && (f.starts_with("circle") || f.starts_with("ellipse")) {
                            stops_src = &parts[1..];
                        }
                    }
                    let vals: Vec<serde_json::Value> = stops_src.iter().map(|s| serde_json::Value::String(s.trim().to_string())).collect();
                    let stops = self.stops(&vals, path)?;
                    let first = stops[0].1;
                    let pc = if radial { PaintC::Radial { c: [0.5, 0.5], r: 1.0, stops } } else { PaintC::Linear { angle, stops } };
                    return Some((pc, first));
                }
                self.color(st, path).map(|c| (PaintC::Solid, c))
            }
            spec::Paint::Gradient(g) => {
                let stops = self.stops(&g.stops, &format!("{path}.stops"))?;
                let first = stops[0].1;
                let radial = match g.kind.as_deref() {
                    None | Some("linear") => false,
                    Some("radial") => true,
                    Some(k) => {
                        self.d.push(Diagnostic::error(format!("{path}.type"), format!("unknown gradient type '{k}'")).suggest(suggest(k, &["linear", "radial"])));
                        false
                    }
                };
                let pc = if radial {
                    let c = g.center.unwrap_or([0.5, 0.5]);
                    PaintC::Radial { c: [c[0] as f32, c[1] as f32], r: g.radius.unwrap_or(1.0) as f32, stops }
                } else {
                    PaintC::Linear { angle: g.angle.unwrap_or(180.0) as f32, stops }
                };
                Some((pc, first))
            }
        }
    }

    fn stops(&mut self, src: &[serde_json::Value], path: &str) -> Option<Vec<(f32, Rgba)>> {
        let n = src.len();
        if n < 2 {
            self.d.push(Diagnostic::error(path, "a gradient needs at least two colours"));
            return None;
        }
        let mut out = Vec::new();
        for (i, v) in src.iter().enumerate() {
            let def = i as f32 / (n - 1) as f32;
            let (col, off) = match v {
                serde_json::Value::String(s) => {
                    // "colour 40%" (colour may contain spaces inside parens).
                    let s = s.trim();
                    match s.rfind(' ') {
                        Some(k) if s[k + 1..].ends_with('%') && !s[..k].ends_with(',') => {
                            (s[..k].trim().to_string(), s[k + 1..].trim_end_matches('%').parse::<f32>().ok().map(|x| x / 100.0))
                        }
                        _ => (s.to_string(), None),
                    }
                }
                serde_json::Value::Array(a) if a.len() == 2 => (a[1].as_str().unwrap_or("").to_string(), a[0].as_f64().map(|x| x as f32)),
                _ => {
                    self.d.push(Diagnostic::error(format!("{path}[{i}]"), "expected a colour or [offset, colour]"));
                    return None;
                }
            };
            let c = self.color(&col, &format!("{path}[{i}]"))?;
            out.push((off.unwrap_or(def).clamp(0.0, 1.0), c));
        }
        Some(out)
    }

    fn stroke(&mut self, s: &spec::StrokeObj, width_override: Option<&Num>, path: &str) -> Option<(StrokeC, Rgba, f32)> {
        let col = s.color.clone().unwrap_or_else(|| "$text".into());
        let (paint, c) = self.paint(&spec::Paint::Css(col), &format!("{path}.color"))?;
        let w = match width_override.or(s.width.as_ref()) {
            Some(n) => self.len_c(n, &format!("{path}.width"), self.h, 16.0),
            None => 4.0,
        };
        let cap = match s.cap.as_deref() {
            None | Some("butt") => 0,
            Some("round") => 1,
            Some("square") => 2,
            Some(x) => {
                self.d.push(Diagnostic::error(format!("{path}.cap"), format!("unknown cap '{x}'")).suggest(suggest(x, &["butt", "round", "square"])));
                0
            }
        };
        let join = match s.join.as_deref() {
            None | Some("miter") => 0,
            Some("round") => 1,
            Some("bevel") => 2,
            Some(x) => {
                self.d.push(Diagnostic::error(format!("{path}.join"), format!("unknown join '{x}'")).suggest(suggest(x, &["miter", "round", "bevel"])));
                0
            }
        };
        let dash = s.dash.clone().unwrap_or_default().iter().map(|x| *x as f32).collect();
        Some((StrokeC { paint, cap, join, dash, dash_offset: s.dash_offset.unwrap_or(0.0) as f32 }, c, w))
    }

    fn word(&mut self, v: &str, allowed: &[&str], path: &str, what: &str) -> bool {
        match val::check_word(v, allowed) {
            Ok(()) => true,
            Err(s) => {
                self.d.push(Diagnostic::error(path, format!("unknown {what} '{v}'")).suggest(s));
                false
            }
        }
    }

    // ---------- scenes ----------

    fn transition(&mut self, t: Option<&spec::Transition>, path: &str) -> TransC {
        let o = t.map(|t| t.0.clone()).unwrap_or_default();
        let kind_s = o.kind.as_deref().unwrap_or("crossfade");
        let kind = match kind_s {
            "cut" | "none" => TransKind::Cut,
            "crossfade" | "fade" => TransKind::Crossfade,
            "fadeBlack" | "fadeWhite" => TransKind::Dip,
            "slide" => TransKind::Slide,
            "push" => TransKind::Push,
            "zoom" => TransKind::Zoom,
            "wipe" => TransKind::Wipe,
            "iris" => TransKind::Iris,
            "blur" => TransKind::Blur,
            "glitch" => TransKind::Glitch,
            "morphCut" => TransKind::MorphCut,
            k => {
                self.d.push(Diagnostic::error(format!("{path}.type"), format!("unknown transition '{k}'")).suggest(suggest(k, val::TRANSITIONS)));
                TransKind::Crossfade
            }
        };
        let def = match kind {
            TransKind::Cut => 0.0,
            TransKind::Glitch => 0.3,
            TransKind::MorphCut => 0.25,
            _ => 0.5,
        };
        let dur = match &o.duration {
            Some(d) if kind != TransKind::Cut => self.time_raw(d, &format!("{path}.duration")).max(0.0),
            _ => def,
        };
        let ease = self.ease(o.ease.as_deref(), "inOutCubic", &format!("{path}.ease"));
        let dir = match o.direction.as_deref() {
            None | Some("left") => [-1.0, 0.0],
            Some("right") => [1.0, 0.0],
            Some("up") => [0.0, -1.0],
            Some("down") => [0.0, 1.0],
            Some(d) => {
                self.d.push(Diagnostic::error(format!("{path}.direction"), format!("unknown direction '{d}'")).suggest(suggest(d, val::DIRECTIONS)));
                [-1.0, 0.0]
            }
        };
        let color = match (&o.color, kind_s) {
            (Some(c), _) => self.color(c, &format!("{path}.color")).unwrap_or(Rgba([0.0, 0.0, 0.0, 1.0])),
            (None, "fadeWhite") => Rgba([1.0; 4]),
            _ => Rgba([0.0, 0.0, 0.0, 1.0]),
        };
        let origin = match &o.origin {
            Some([x, y]) => [self.len_c(x, &format!("{path}.origin[0]"), self.w, 16.0), self.len_c(y, &format!("{path}.origin[1]"), self.h, 16.0)],
            None => [self.w * 0.5, self.h * 0.5],
        };
        let amount = match kind {
            TransKind::Blur => o.amount.unwrap_or(40.0) as f32,
            TransKind::Glitch => o.intensity.unwrap_or(1.0) as f32,
            _ => o.amount.unwrap_or(0.0) as f32,
        };
        TransC { kind, dur, ease, dir, color, origin, amount }
    }

    fn scene(&mut self, s: &spec::Scene, path: &str, dur: f32, default_bg: &spec::Paint) -> Built {
        self.scene_dur = dur;
        let (bp, bpath) = match &s.background {
            Some(b) => (b, format!("{path}.background")),
            None => (default_bg, "background".to_string()),
        };
        let (bg, bg_color) = self.paint(bp, &bpath).unwrap_or((PaintC::Solid, Rgba([0.0, 0.0, 0.0, 1.0])));
        let bg_color = match &bg {
            PaintC::Solid => bg_color,
            PaintC::Linear { stops, .. } | PaintC::Radial { stops, .. } => stops[0].1,
        };
        // Camera.
        let mut camera = empty_node(Kind::Group { children: Vec::new() });
        camera.start = 0.0;
        camera.end = f32::INFINITY;
        let mut shake = None;
        if let Some(cam) = &s.camera {
            let cp = format!("{path}.camera");
            self.set(&mut camera, prop::X, cam.x.as_ref(), &format!("{cp}.x"), self.w);
            self.set(&mut camera, prop::Y, cam.y.as_ref(), &format!("{cp}.y"), self.h);
            self.set(&mut camera, prop::ZOOM, cam.zoom.as_ref(), &format!("{cp}.zoom"), 1.0);
            self.set(&mut camera, prop::ROT, cam.rotation.as_ref(), &format!("{cp}.rotation"), 1.0);
            let mut tw = Vec::new();
            if let Some(a) = &cam.animate {
                for (k, t) in a.iter().enumerate() {
                    if let Some(x) = self.tween(t, &format!("{cp}.animate[{k}]"), 0.0, [self.w, self.h], &mut tw) {
                        tw.push(x);
                    }
                }
            }
            camera.tracks = self.tracks(&camera.base, &camera.exprs, &mut tw, dur);
            if let Some(sh) = &cam.shake {
                let at = sh.at.as_ref().map(|a| self.time_rel(a, &format!("{cp}.shake.at"), 0.0)).unwrap_or(0.0);
                let d = sh.duration.as_ref().map(|a| self.time_raw(a, &format!("{cp}.shake.duration"))).unwrap_or(if sh.at.is_some() { 0.5 } else { 0.0 });
                shake = Some((sh.amount.unwrap_or(8.0) as f32, sh.frequency.unwrap_or(6.0) as f32, at, d, sh.rotation.unwrap_or(0.0) as f32));
            }
        }
        let pc = PCtx { size: [self.w, self.h], t0: 0.0, end: dur, delay: 0.0, index: 0, count: s.layers.len(), in_box: false };
        let layers = self.layers(&s.layers, &format!("{path}.layers"), pc, None);
        Built { camera, shake, layers, bg, bg_color }
    }

    fn stagger_delays(&mut self, st: Option<&spec::Stagger>, n: usize, path: &str) -> Vec<f32> {
        let Some(st) = st else { return vec![0.0; n] };
        let o = &st.0;
        let from = o.from.as_deref().unwrap_or("start");
        if !self.word(from, val::STAGGER_FROM, &format!("{path}.from"), "stagger origin") {
            return vec![0.0; n];
        }
        let m = (n.max(2) - 1) as f32;
        let total = match (o.total, o.each) {
            (Some(t), _) => t as f32,
            (None, Some(e)) => e as f32 * m,
            _ => 0.1 * m,
        };
        let ease = o.ease.as_deref().map(|e| self.ease(Some(e), "linear", &format!("{path}.ease")));
        (0..n)
            .map(|i| {
                let i = i as f32;
                let k = match from {
                    "center" => (i - m * 0.5).abs() / (m * 0.5).max(1e-6),
                    "end" => (m - i) / m.max(1e-6),
                    "random" => super::hash01(i as f64 * 7.13 + 1.0) as f32,
                    _ => i / m.max(1e-6),
                };
                let k = if n <= 1 { 0.0 } else { k };
                let k = match ease {
                    Some(e) => self.easings[e as usize].ease(k as f64) as f32,
                    None => k,
                };
                total * k
            })
            .collect()
    }

    fn layers(&mut self, ls: &[Layer], path: &str, pc: PCtx, stagger: Option<&spec::Stagger>) -> Vec<Node> {
        let delays = self.stagger_delays(stagger, ls.len(), &format!("{path}.stagger"));
        let mut out = Vec::new();
        for (i, l) in ls.iter().enumerate() {
            let p = format!("{path}[{i}]");
            let cpc = PCtx { index: i, count: ls.len(), delay: delays[i], ..pc };
            if let Some(n) = self.layer(l, &p, cpc) {
                out.push(n);
            }
        }
        out
    }

    // ---------- layers ----------

    fn infer_kind(&mut self, l: &Layer, path: &str) -> Option<&'static str> {
        if let Some(k) = &l.kind {
            if let Some(&k) = val::LAYER_TYPES.iter().find(|x| **x == k) {
                return Some(k);
            }
            self.d.push(Diagnostic::error(format!("{path}.type"), format!("unknown layer type '{k}'")).suggest(suggest(k, val::LAYER_TYPES)));
            return None;
        }
        Some(if l.text.is_some() {
            "text"
        } else if l.shape.is_some() {
            "shape"
        } else if l.component.is_some() {
            "component"
        } else if l.children.is_some() {
            "box"
        } else if l.layers.is_some() {
            "group"
        } else if let Some(src) = &l.src {
            let low = src.to_ascii_lowercase();
            if [".mp4", ".mov", ".webm", ".mkv", ".m4v"].iter().any(|e| low.ends_with(e)) {
                "video"
            } else {
                "image"
            }
        } else {
            self.d.push(Diagnostic::error(
                path,
                "layer has no 'type' and none can be inferred (add \"text\", \"shape\", \"src\", \"children\", \"layers\" or \"component\")",
            ));
            return None;
        })
    }

    fn misplaced(&mut self, l: &Layer, kind: &str, path: &str) {
        let text_only = [
            ("text", l.text.is_some()),
            ("font", l.font.is_some()),
            ("weight", l.weight.is_some()),
            ("split", l.split.is_some()),
            ("align", l.align.is_some()),
            ("lineHeight", l.line_height.is_some()),
            ("reveal", l.reveal.is_some()),
        ];
        let shape_only = [("shape", l.shape.is_some()), ("points", l.points.is_some()), ("d", l.d.is_some()), ("sides", l.sides.is_some())];
        let img_only = [("src", l.src.is_some()), ("fit", l.fit.is_some())];
        let check = |d: &mut Vec<Diagnostic>, list: &[(&str, bool)], ok: bool, owner: &str| {
            if !ok {
                for (k, set) in list {
                    if *set {
                        d.push(Diagnostic::warning(format!("{path}.{k}"), format!("'{k}' only applies to {owner} layers; this is a {kind} layer")));
                    }
                }
            }
        };
        check(&mut self.d, &text_only, kind == "text", "text");
        check(&mut self.d, &shape_only, kind == "shape", "shape");
        check(&mut self.d, &img_only, kind == "image" || kind == "video", "image/video");
        if l.layers.is_some() && kind != "group" {
            self.d.push(Diagnostic::warning(format!("{path}.layers"), format!("'layers' only applies to group layers (a box uses 'children'); this is a {kind} layer")));
        }
        if l.children.is_some() && kind != "box" {
            self.d.push(Diagnostic::warning(format!("{path}.children"), format!("'children' only applies to box layers (a group uses 'layers'); this is a {kind} layer")));
        }
    }

    fn layer(&mut self, l: &Layer, path: &str, pc: PCtx) -> Option<Node> {
        let kind = self.infer_kind(l, path)?;
        if l.visible == Some(false) {
            return None;
        }
        self.misplaced(l, kind, path);
        let start = pc.t0 + l.start.as_ref().map(|s| self.time_rel(s, &format!("{path}.start"), pc.t0)).unwrap_or(0.0);
        let mut end = pc.end;
        if let Some(e) = &l.end {
            end = pc.t0 + self.time_rel(e, &format!("{path}.end"), pc.t0);
        } else if let Some(d) = &l.duration {
            end = start + self.time_raw(d, &format!("{path}.duration"));
        }
        if start >= self.scene_dur && self.scene_dur < 1e5 && pc.t0 == 0.0 {
            self.d.push(Diagnostic::warning(
                format!("{path}.start"),
                format!("{start:.2} s is after the scene ends ({:.2} s); the layer is never visible", self.scene_dur),
            ));
        }
        if end <= start {
            self.d.push(Diagnostic::warning(path, "the layer ends before it starts; it is never visible"));
        }
        let [pw, ph] = pc.size;
        let mut n = empty_node(Kind::Group { children: Vec::new() });
        n.start = start;
        n.end = end;
        n.index = pc.index as f32;
        n.count = pc.count as f32;

        // Size (explicit).
        let given_w = l.width.as_ref().map(|v| self.len_c(v, &format!("{path}.width"), pw, 16.0));
        let given_h = l.height.as_ref().map(|v| self.len_c(v, &format!("{path}.height"), ph, 16.0));
        let mut split_presets: Option<(Option<spec::Preset>, Option<spec::Preset>)> = None;
        let mut text_chars = 0usize;
        let mut kinetic_enter = false;

        // Kind-specific.
        let (kind_v, nat) = match kind {
            "text" => {
                let r = self.text_layer(l, path, given_w, &mut n, pc);
                text_chars = r.2;
                kinetic_enter = r.4;
                if r.3 {
                    split_presets = Some((l.enter.clone(), l.exit.clone()));
                }
                (r.0, r.1)
            }
            "shape" => self.shape_layer(l, path, given_w, given_h, &mut n),
            "image" => self.image_layer(l, path, given_w, given_h),
            "video" => {
                self.d.push(Diagnostic::warning(path, "video layers are not supported yet; drawn as a placeholder"));
                let s = [given_w.unwrap_or(640.0), given_h.unwrap_or(360.0)];
                n.base[prop::RADIUS] = 12.0;
                n.base[prop::FILL..prop::FILL + 4].copy_from_slice(&rgba_to_oklab(self.theme.colors["surface"]));
                (Kind::Shape { shape: ShapeKind::Rect, fill: Some(PaintC::Solid), stroke: None, path: Path::new() }, s)
            }
            "group" => {
                let size = [given_w.unwrap_or(pw), given_h.unwrap_or(ph)];
                let cpc = PCtx { size, t0: start, end, delay: 0.0, index: 0, count: 0, in_box: false };
                let kids = self.layers(l.layers.as_deref().unwrap_or(&[]), &format!("{path}.layers"), cpc, l.stagger.as_ref());
                (Kind::Group { children: kids }, size)
            }
            "box" => self.box_layer(l, path, given_w, given_h, start, end, pc, &mut n),
            _ => self.component_layer(l, path, given_w, given_h)?,
        };
        n.kind = kind_v;
        n.base[prop::W] = given_w.unwrap_or(nat[0]);
        n.base[prop::H] = given_h.unwrap_or(nat[1]);
        if let (Some(_), None, Kind::Image { natural, .. }) = (given_w, given_h, &n.kind) {
            n.base[prop::H] = given_w.unwrap() * natural[1] / natural[0].max(1.0);
        }
        if let (None, Some(_), Kind::Image { natural, .. }) = (given_w, given_h, &n.kind) {
            n.base[prop::W] = given_h.unwrap() * natural[0] / natural[1].max(1.0);
        }
        if let Some(w) = &l.width {
            if let V::E(e) = self.len(w, &format!("{path}.width"), pw, 16.0) {
                n.exprs.push((prop::W as u8, e));
            }
        }
        if let Some(h) = &l.height {
            if let V::E(e) = self.len(h, &format!("{path}.height"), ph, 16.0) {
                n.exprs.push((prop::H as u8, e));
            }
        }

        // Position, anchor.
        n.base[prop::X] = pw * 0.5;
        n.base[prop::Y] = ph * 0.5;
        if pc.in_box && (l.x.is_some() || l.y.is_some()) {
            self.d.push(Diagnostic::warning(path, "x / y of a box child are set by the layout and ignored"));
        } else {
            self.set(&mut n, prop::X, l.x.as_ref(), &format!("{path}.x"), pw);
            self.set(&mut n, prop::Y, l.y.as_ref(), &format!("{path}.y"), ph);
        }
        n.anchor = match &l.anchor {
            None => [0.5, 0.5],
            Some(spec::Anchor::Xy([a, b])) => [*a as f32, *b as f32],
            Some(spec::Anchor::Name(s)) => match s.as_str() {
                "center" | "centre" => [0.5, 0.5],
                "top" => [0.5, 0.0],
                "bottom" => [0.5, 1.0],
                "left" => [0.0, 0.5],
                "right" => [1.0, 0.5],
                "topLeft" => [0.0, 0.0],
                "topRight" => [1.0, 0.0],
                "bottomLeft" => [0.0, 1.0],
                "bottomRight" => [1.0, 1.0],
                x => {
                    self.d.push(Diagnostic::error(format!("{path}.anchor"), format!("unknown anchor '{x}'")).suggest(suggest(x, val::ANCHORS)));
                    [0.5, 0.5]
                }
            },
        };

        // Transform & appearance.
        match &l.scale {
            Some(spec::Scale::One(s)) => {
                self.set(&mut n, prop::SX, Some(s), &format!("{path}.scale"), 1.0);
                self.set(&mut n, prop::SY, Some(s), &format!("{path}.scale"), 1.0);
            }
            Some(spec::Scale::Xy([a, b])) => {
                self.set(&mut n, prop::SX, Some(a), &format!("{path}.scale[0]"), 1.0);
                self.set(&mut n, prop::SY, Some(b), &format!("{path}.scale[1]"), 1.0);
            }
            None => {}
        }
        self.set(&mut n, prop::SX, l.scale_x.as_ref(), &format!("{path}.scaleX"), 1.0);
        self.set(&mut n, prop::SY, l.scale_y.as_ref(), &format!("{path}.scaleY"), 1.0);
        self.set(&mut n, prop::ROT, l.rotation.as_ref(), &format!("{path}.rotation"), 1.0);
        self.set(&mut n, prop::SKX, l.skew_x.as_ref(), &format!("{path}.skewX"), 1.0);
        self.set(&mut n, prop::SKY, l.skew_y.as_ref(), &format!("{path}.skewY"), 1.0);
        self.set(&mut n, prop::OPACITY, l.opacity.as_ref(), &format!("{path}.opacity"), 1.0);
        self.set(&mut n, prop::BLUR, l.blur.as_ref(), &format!("{path}.blur"), ph);
        self.set(&mut n, prop::PROGRESS, l.progress.as_ref(), &format!("{path}.progress"), 1.0);
        if let Some(b) = &l.blend {
            n.blend = match b.as_str() {
                "normal" => Blend::Normal,
                "add" => Blend::Add,
                "multiply" => Blend::Multiply,
                "screen" => Blend::Screen,
                "overlay" => Blend::Overlay,
                "darken" => Blend::Darken,
                "lighten" => Blend::Lighten,
                "softLight" => Blend::SoftLight,
                "difference" => Blend::Difference,
                x => {
                    self.d.push(Diagnostic::error(format!("{path}.blend"), format!("unknown blend mode '{x}'")).suggest(suggest(x, val::BLENDS)));
                    Blend::Normal
                }
            };
        }
        if let Some(s) = &l.shadow {
            let s = &s.0;
            let color = match &s.color {
                Some(c) => self.color(c, &format!("{path}.shadow.color")).unwrap_or(Rgba([0.0, 0.0, 0.0, 0.5])),
                None => Rgba([0.0, 0.0, 0.0, 0.45]),
            };
            n.shadow = Some(Filter::Shadow { dx: s.x.unwrap_or(0.0) as f32, dy: s.y.unwrap_or(12.0) as f32, blur: s.blur.unwrap_or(32.0) as f32, color });
        }
        if let Some(g) = &l.glow {
            let g = &g.0;
            let color = match &g.color {
                Some(c) => self.color(c, &format!("{path}.glow.color")).unwrap_or(self.theme.colors["primary"]),
                None => self.theme.colors["primary"],
            };
            n.glow = Some(Filter::Glow { radius: g.radius.unwrap_or(24.0) as f32, intensity: g.intensity.unwrap_or(1.0) as f32, color });
        }
        match &l.clip {
            Some(spec::Clip::On(b)) => n.clip = *b,
            Some(spec::Clip::Shape(_)) => {
                self.d.push(Diagnostic::warning(format!("{path}.clip"), "clip shapes are not supported yet; clipping to the layer box"));
                n.clip = true;
            }
            None => {}
        }
        if l.mask.is_some() {
            self.d.push(Diagnostic::warning(format!("{path}.mask"), "masks are not supported yet; ignored"));
        }

        // Animation.
        let refs = [pw, ph];
        let mut tw: Vec<TweenC> = Vec::new();
        let life_end = if end.is_finite() { end - start } else { self.scene_dur - start };
        let pctx = PresetCx { life: life_end, chars: text_chars, refs, unit: false };
        if split_presets.is_none() {
            if let (Some(p), false) = (&l.enter, kinetic_enter) {
                self.preset(&p.0, &format!("{path}.enter"), false, pc.delay, pctx, &mut tw, &mut n);
            }
            if let Some(p) = &l.exit {
                self.preset(&p.0, &format!("{path}.exit"), true, 0.0, pctx, &mut tw, &mut n);
            }
        }
        if let Some(a) = &l.animate {
            for (k, t) in a.iter().enumerate() {
                let tp = format!("{path}.animate[{k}]");
                if let Some(p) = &t.preset {
                    let at = t.at.as_ref().map(|x| self.time_rel(x, &format!("{tp}.at"), start)).unwrap_or(0.0);
                    let po = PresetObj {
                        preset: Some(p.clone()),
                        delay: Some(Num::Num((at + pc.delay) as f64)),
                        duration: t.duration.clone(),
                        ease: t.ease.clone(),
                        spring: t.spring.clone(),
                        ..Default::default()
                    };
                    self.preset(&po, &tp, false, 0.0, pctx, &mut tw, &mut n);
                    continue;
                }
                if let Some(mut x) = self.tween(t, &tp, start, refs, &mut tw) {
                    x.at += pc.delay;
                    tw.push(x);
                }
            }
        }
        // Components: progress 0 → 1 over the lifetime unless set.
        if matches!(n.kind, Kind::Component(..)) && l.progress.is_none() && life_end.is_finite() {
            tw.insert(0, TweenC { at: 0.0, dur: life_end.max(1e-3), ease: 0, from: vec![(prop::PROGRESS, 1, ValS::Abs(0.0))], to: vec![(prop::PROGRESS, 1, ValS::Abs(1.0))], kf: vec![], repeat: 0, yoyo: false });
        }
        n.tracks = self.tracks(&n.base, &n.exprs, &mut tw, life_end);

        // Split text: presets per unit.
        if let Some((enter, exit)) = split_presets {
            let units = match &n.kind {
                Kind::Text { split: Some(sp), layout, .. } => layout.units(sp.by).len(),
                _ => 0,
            };
            let mut utw = Vec::new();
            let mut unit_node = empty_node(Kind::Group { children: Vec::new() });
            let upc = PresetCx { life: life_end, chars: text_chars, refs, unit: true };
            let mut st_src = None;
            if let Some(p) = &enter {
                self.preset(&p.0, &format!("{path}.enter"), false, pc.delay, upc, &mut utw, &mut unit_node);
                st_src = p.0.stagger.clone();
            }
            if let Some(p) = &exit {
                self.preset(&p.0, &format!("{path}.exit"), true, 0.0, upc, &mut utw, &mut unit_node);
            }
            let st_src = st_src.or_else(|| l.stagger.clone()).or(Some(spec::Stagger(spec::StaggerObj { each: Some(0.04), ..Default::default() })));
            let delays = self.stagger_delays(st_src.as_ref(), units, &format!("{path}.enter.stagger"));
            let tracks = self.tracks(&prop::DEFAULTS, &[], &mut utw, life_end);
            if let Kind::Text { split: Some(sp), .. } = &mut n.kind {
                sp.tracks = tracks;
                sp.delays = delays;
            }
        }

        // Loops.
        if let Some(spec::LoopValue::Preset(p)) = &l.loop_ {
            let p = &p.0;
            let name = p.preset.as_deref().unwrap_or("");
            let period = p.period.map(|x| x as f32);
            let amount = p.amount.map(|x| x as f32);
            let lp = match name {
                "float" => Some(LoopC::Float { amount: amount.unwrap_or(12.0), period: period.unwrap_or(3.0) }),
                "pulse" => Some(LoopC::Pulse { amount: amount.unwrap_or(0.06), period: period.unwrap_or(1.2) }),
                "spin" => Some(LoopC::Spin { period: period.unwrap_or(4.0) }),
                "wiggle" => Some(LoopC::Wiggle { amount: amount.unwrap_or(6.0), period: period.unwrap_or(0.5) }),
                "breathe" => Some(LoopC::Breathe { amount: amount.unwrap_or(0.3), period: period.unwrap_or(2.5) }),
                "blink" => Some(LoopC::Blink { period: period.unwrap_or(1.0) }),
                x => {
                    self.d.push(Diagnostic::error(format!("{path}.loop"), format!("unknown loop preset '{x}'")).suggest(suggest(x, val::LOOP_PRESETS)));
                    None
                }
            };
            if let Some(lp) = lp {
                if matches!(lp, LoopC::Float { period, .. } | LoopC::Pulse { period, .. } | LoopC::Spin { period } | LoopC::Wiggle { period, .. } | LoopC::Breathe { period, .. } | LoopC::Blink { period } if period <= 0.0)
                {
                    self.d.push(Diagnostic::error(format!("{path}.loop.period"), "must be positive"));
                } else {
                    n.loops.push(lp);
                }
            }
        } else if let Some(spec::LoopValue::Clip(_)) = &l.loop_ {
            if kind != "video" {
                self.d.push(Diagnostic::warning(format!("{path}.loop"), "a bool 'loop' only applies to video layers; use a preset name like \"float\""));
            }
        }
        Some(n)
    }

    /// Returns (kind, natural size, char count, split?).
    fn text_layer(&mut self, l: &Layer, path: &str, given_w: Option<f32>, n: &mut Node, pc: PCtx) -> (Kind, [f32; 2], usize, bool, bool) {
        let style_name = match &l.style {
            Some(StyleRef::Name(s)) => s.clone(),
            Some(StyleRef::Box(_)) => {
                self.d.push(Diagnostic::warning(format!("{path}.style"), "a text layer's style is a theme style name like \"title\"; box styles are ignored"));
                "body".into()
            }
            None => "body".into(),
        };
        let base = match self.theme.styles.get(&style_name) {
            Some(s) => s.clone(),
            None => {
                let names: Vec<&String> = self.theme.styles.keys().collect();
                self.d.push(Diagnostic::error(format!("{path}.style"), format!("unknown text style '{style_name}'")).suggest(suggest(&style_name, &names)));
                self.theme.styles["body"].clone()
            }
        };
        let over = spec::TextStyle {
            font: l.font.clone(),
            size: l.size.clone(),
            weight: l.weight.clone(),
            italic: l.italic,
            color: l.color.clone(),
            line_height: l.line_height,
            letter_spacing: l.letter_spacing.clone(),
            uppercase: l.uppercase,
            align: l.align.clone(),
        };
        let st = merge_style(&base, &over);
        let family = self.theme.font(st.font.as_deref().unwrap_or("$body")).to_string();
        let size = st.size.as_ref().map(|s| self.len_c(s, &format!("{path}.size"), pc.size[1], 16.0)).unwrap_or(32.0).max(1.0);
        let weight = st.weight.as_ref().and_then(weight_value).unwrap_or(400.0);
        let ls = st.letter_spacing.as_ref().map(|s| self.len_c(s, &format!("{path}.letterSpacing"), size, size)).unwrap_or(0.0);
        let align = match st.align.as_deref() {
            None | Some("center") => text::Align::Center,
            Some("left") | Some("start") => text::Align::Left,
            Some("right") | Some("end") => text::Align::Right,
            Some("justify") => text::Align::Justify,
            Some(a) => {
                self.d.push(Diagnostic::error(format!("{path}.align"), format!("unknown align '{a}'")).suggest(suggest(a, val::ALIGNS)));
                text::Align::Center
            }
        };
        let mut raw = l.text.clone().unwrap_or_default();
        if st.uppercase == Some(true) {
            raw = raw.to_uppercase();
        }
        let col = st.color.clone().unwrap_or(spec::Paint::Css("$text".into()));
        let (paint, c) = self.paint(&col, &format!("{path}.color")).unwrap_or((PaintC::Solid, Rgba([1.0; 4])));
        n.base[prop::FILL..prop::FILL + 4].copy_from_slice(&rgba_to_oklab(c));
        n.base[prop::SIZE] = size;
        if l.stroke.is_some() {
            self.d.push(Diagnostic::warning(format!("{path}.stroke"), "outlined text is not supported yet; ignored"));
        }
        let style = text::TextStyle {
            font: family,
            weight,
            italic: st.italic.unwrap_or(false),
            size,
            color: text::Color(c),
            letter_spacing: ls / size,
            line_height: st.line_height.unwrap_or(1.2) as f32,
            ..Default::default()
        };
        let (plain, spans) = self.markup(&raw, path);
        let run_colors = !spans.is_empty();
        let mut tspec = text::TextSpec { text: plain.clone(), spans, style, width: given_w, align, max_lines: l.max_lines, ..Default::default() };
        // Counter template.
        let mut counter = None;
        if let Some(i) = plain.find("{value") {
            if let Some(j) = plain[i..].find('}') {
                let fmt = plain[i + 6..i + j].strip_prefix(':').unwrap_or("").to_string();
                let thousands = fmt.contains(',');
                let decimals = fmt.find('.').map(|k| fmt[k + 1..].chars().filter(|c| *c == '0').count()).unwrap_or(0);
                tspec.spans.clear();
                let mut cs = tspec.clone();
                cs.style.features = Some("\"tnum\" on".into());
                counter = Some(Counter { prefix: plain[..i].to_string(), suffix: plain[i + j + 1..].to_string(), decimals, thousands, spec: cs });
                if l.value.is_none() {
                    self.d.push(Diagnostic::warning(format!("{path}.value"), "the text uses {value} but no 'value' is set (shows 0)"));
                }
            }
        }
        if let Some(v) = &l.value {
            match self.len(v, &format!("{path}.value"), 1.0, 1.0) {
                V::C(c) => n.base[prop::VALUE] = c,
                V::E(e) => n.exprs.push((prop::VALUE as u8, e)),
            }
            if counter.is_none() {
                self.d.push(Diagnostic::warning(format!("{path}.value"), "'value' is set but the text has no {value} placeholder"));
            }
        }
        let layout = match &counter {
            Some(c) => {
                let mut sp = c.spec.clone();
                sp.text = format!("{}{}{}", c.prefix, format_value(n.base[prop::VALUE] as f64, c.decimals, c.thousands), c.suffix);
                self.assets.fonts.layout(&sp)
            }
            None => self.assets.fonts.layout(&tspec),
        };
        self.set(n, prop::REVEAL, l.reveal.as_ref(), &format!("{path}.reveal"), 1.0);
        // Kinetic text: an explicit `kinetic`, or an `enter` that is a TEXT animation (`"scramble"`, `"wave by word"`).
        let mut from_enter = false;
        let kin: Option<text::kinetic::Kinetic> = if let Some(k) = &l.kinetic {
            match serde_json::from_value(k.clone()) {
                Ok(k) => Some(k),
                Err(e) => {
                    self.d.push(Diagnostic::error(format!("{path}.kinetic"), e.to_string()));
                    None
                }
            }
        } else {
            match &l.enter {
                Some(p) if p.0.preset.as_deref().is_some_and(|x| !val::ENTER_PRESETS.contains(&x)) => {
                    let name = p.0.preset.as_deref().unwrap();
                    match text::kinetic::Kinetic::parse(name) {
                        Ok(mut k) => {
                            from_enter = true;
                            if let Some(d) = &p.0.delay {
                                k.delay += self.time_raw(d, &format!("{path}.enter.delay"));
                            }
                            if let Some(d) = &p.0.duration {
                                k.duration = Some(self.time_raw(d, &format!("{path}.enter.duration")));
                            }
                            if let Some(e) = &p.0.ease {
                                k.ease = Some(e.clone());
                            }
                            if let Some(st) = &p.0.stagger {
                                k.stagger = st.0.each.map(|x| x as f32);
                            }
                            Some(k)
                        }
                        Err(_) => None,
                    }
                }
                _ => None,
            }
        };
        let kinetic = kin.map(|mut k| {
            k.delay += pc.delay;
            text::kinetic::KineticPlan::new(&self.assets.fonts, layout.clone(), &k)
        });
        let split = match l.split.as_deref() {
            None => None,
            Some("chars") => Some(text::UnitKind::Char),
            Some("words") => Some(text::UnitKind::Word),
            Some("lines") => Some(text::UnitKind::Line),
            Some(x) => {
                self.d.push(Diagnostic::error(format!("{path}.split"), format!("unknown split '{x}'")).suggest(suggest(x, val::SPLITS)));
                None
            }
        };
        let chars = layout.chars.len();
        let nat = [layout.width, layout.height];
        if split.is_some() && (counter.is_some() || kinetic.is_some()) {
            self.d.push(Diagnostic::warning(format!("{path}.split"), "split is ignored on counters and kinetic text"));
        }
        let split = if counter.is_some() || kinetic.is_some() { None } else { split.map(|by| SplitC { by, tracks: Vec::new(), delays: Vec::new() }) };
        let has_split = split.is_some();
        (Kind::Text { layout, paint, run_colors, split, counter, kinetic }, nat, chars, has_split, from_enter)
    }

    /// Inline markup: `**bold**`, `*italic*`, `[text]{colour}`. Returns the plain text and spans (empty if none).
    fn markup(&mut self, src: &str, path: &str) -> (String, Vec<text::Span>) {
        if !src.contains('*') && !src.contains("]{") {
            return (src.to_string(), Vec::new());
        }
        let mut spans: Vec<text::Span> = Vec::new();
        let mut cur = text::Span::default();
        let (mut bold, mut ital) = (false, false);
        let b = src.as_bytes();
        let mut i = 0;
        let flush = |spans: &mut Vec<text::Span>, cur: &mut text::Span, bold: bool, ital: bool| {
            if !cur.text.is_empty() {
                let mut s = std::mem::take(cur);
                if bold {
                    s.weight = Some(700.0);
                }
                if ital {
                    s.italic = Some(true);
                }
                spans.push(s);
            }
        };
        while i < b.len() {
            if src[i..].starts_with("**") {
                flush(&mut spans, &mut cur, bold, ital);
                bold = !bold;
                i += 2;
            } else if b[i] == b'*' {
                flush(&mut spans, &mut cur, bold, ital);
                ital = !ital;
                i += 1;
            } else if b[i] == b'[' {
                if let Some(close) = src[i..].find("]{").map(|k| k + i) {
                    if let Some(end) = src[close..].find('}').map(|k| k + close) {
                        flush(&mut spans, &mut cur, bold, ital);
                        let col = &src[close + 2..end];
                        let mut sp = text::Span::new(&src[i + 1..close]);
                        sp.color = self.color(col, path).map(text::Color);
                        cur = sp;
                        flush(&mut spans, &mut cur, bold, ital);
                        i = end + 1;
                        continue;
                    }
                }
                cur.text.push('[');
                i += 1;
            } else {
                let ch = src[i..].chars().next().unwrap();
                cur.text.push(ch);
                i += ch.len_utf8();
            }
        }
        flush(&mut spans, &mut cur, bold, ital);
        let plain: String = spans.iter().map(|s| s.text.as_str()).collect();
        (plain, spans)
    }

    fn shape_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>, n: &mut Node) -> (Kind, [f32; 2]) {
        let sname = l.shape.as_deref().unwrap_or("rect");
        let shape_s = if self.word(sname, val::SHAPES, &format!("{path}.shape"), "shape") { sname } else { "rect" };
        let (w, h) = match (gw, gh) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, if shape_s == "rect" { w * 0.6 } else { w }),
            (None, Some(h)) => (h, h),
            (None, None) => (200.0, if shape_s == "rect" { 120.0 } else { 200.0 }),
        };
        let mut open = false;
        let (kind, mut p, size) = match shape_s {
            "rect" => (ShapeKind::Rect, Path::new(), [w, h]),
            "circle" | "ellipse" => (ShapeKind::Ellipse, Path::new(), [w, h]),
            "line" | "arrow" => {
                open = true;
                let pts: Vec<[f32; 2]> = match &l.points {
                    Some(ps) if ps.len() >= 2 => ps
                        .iter()
                        .enumerate()
                        .map(|(i, [x, y])| [self.len_c(x, &format!("{path}.points[{i}][0]"), self.w, 16.0), self.len_c(y, &format!("{path}.points[{i}][1]"), self.h, 16.0)])
                        .collect(),
                    Some(_) => {
                        self.d.push(Diagnostic::error(format!("{path}.points"), "a line needs at least two points"));
                        vec![[0.0, 0.0], [w, 0.0]]
                    }
                    None => vec![[0.0, 0.0], [gw.unwrap_or(400.0), 0.0]],
                };
                let mut bp = Path::new();
                bp.move_to((pts[0][0] as f64, pts[0][1] as f64));
                for q in &pts[1..] {
                    bp.line_to((q[0] as f64, q[1] as f64));
                }
                if shape_s == "arrow" {
                    let sw = l.stroke_width.as_ref().map(|v| self.len_c(v, &format!("{path}.strokeWidth"), self.h, 16.0)).unwrap_or(4.0);
                    let hs = l.head_size.map(|x| x as f32).unwrap_or(sw * 4.0);
                    let a = pts[pts.len() - 2];
                    let b = pts[pts.len() - 1];
                    let ang = (b[1] - a[1]).atan2(b[0] - a[0]);
                    for s in [-1.0f32, 1.0] {
                        let t = ang + std::f32::consts::PI + s * 0.5;
                        bp.move_to((b[0] as f64, b[1] as f64));
                        bp.line_to(((b[0] + hs * t.cos()) as f64, (b[1] + hs * t.sin()) as f64));
                    }
                }
                (ShapeKind::Path, bp, [0.0, 0.0])
            }
            "polygon" | "star" => {
                let mut bp = Path::new();
                if let (Some(ps), "polygon") = (&l.points, shape_s) {
                    for (i, [x, y]) in ps.iter().enumerate() {
                        let pt = (self.len_c(x, &format!("{path}.points[{i}][0]"), self.w, 16.0) as f64, self.len_c(y, &format!("{path}.points[{i}][1]"), self.h, 16.0) as f64);
                        if i == 0 {
                            bp.move_to(pt)
                        } else {
                            bp.line_to(pt)
                        }
                    }
                    bp.close_path();
                    (ShapeKind::Path, bp, [0.0, 0.0])
                } else {
                    let star = shape_s == "star";
                    let k = l.sides.unwrap_or(if star { 5 } else { 6 }).max(3) as usize;
                    let inner = l.inner_radius.unwrap_or(0.5) as f32;
                    let m = if star { k * 2 } else { k };
                    for i in 0..m {
                        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / m as f32;
                        let r = if star && i % 2 == 1 { inner } else { 1.0 };
                        let pt = (((0.5 + 0.5 * r * a.cos()) * w) as f64, ((0.5 + 0.5 * r * a.sin()) * h) as f64);
                        if i == 0 {
                            bp.move_to(pt)
                        } else {
                            bp.line_to(pt)
                        }
                    }
                    bp.close_path();
                    (ShapeKind::Path, bp, [w, h])
                }
            }
            _ => {
                let d = l.d.as_deref().unwrap_or("");
                if d.is_empty() {
                    self.d.push(Diagnostic::error(format!("{path}.d"), "a path shape needs 'd' (SVG path data)"));
                }
                let bp = match Path::from_svg(d) {
                    Ok(p) => p,
                    Err(e) => {
                        self.d.push(Diagnostic::error(format!("{path}.d"), format!("invalid SVG path: {e}")));
                        Path::new()
                    }
                };
                open = !d.trim_end().ends_with(['z', 'Z']);
                (ShapeKind::Path, bp, [0.0, 0.0])
            }
        };
        let mut size = size;
        if kind == ShapeKind::Path && size == [0.0, 0.0] {
            // Normalise so the bounding box starts at the origin.
            let bb = kurbo::Shape::bounding_box(&p);
            if bb.width().is_finite() {
                p.apply_affine(kurbo::Affine::translate((-bb.x0, -bb.y0)));
                size = [bb.width() as f32, bb.height() as f32];
            }
        }
        if (gw.is_some() || gh.is_some()) && shape_s == "path" && size[0] > 0.0 && size[1] > 0.0 {
            // An explicit size scales a path to fit.
            let sx = gw.unwrap_or(size[0]) / size[0].max(1e-6);
            let sy = gh.unwrap_or(size[1] * sx) / size[1].max(1e-6);
            p.apply_affine(kurbo::Affine::scale_non_uniform(sx as f64, sy as f64));
            size = [size[0] * sx, size[1] * sy];
        }
        if let Some(r) = &l.radius {
            self.set(n, prop::RADIUS, Some(r), &format!("{path}.radius"), w.min(h));
        }
        // Fill.
        let fill = match &l.fill {
            Some(f) => self.paint(f, &format!("{path}.fill")),
            None if open => None,
            None => self.paint(&spec::Paint::Css("$primary".into()), path),
        };
        let fill = fill.map(|(pc, c)| {
            n.base[prop::FILL..prop::FILL + 4].copy_from_slice(&rgba_to_oklab(c));
            pc
        });
        if let Some(c) = &l.color {
            self.d.push(Diagnostic::warning(format!("{path}.color"), "shapes use 'fill' (and 'stroke'); 'color' is for text").suggest(Some("fill")));
            let _ = c;
        }
        // Stroke.
        let stroke_obj = match (&l.stroke, open) {
            (Some(s), _) => Some(s.0.clone()),
            (None, true) => Some(spec::StrokeObj::default()),
            (None, false) if l.stroke_width.is_some() => Some(spec::StrokeObj::default()),
            _ => None,
        };
        let stroke = match stroke_obj {
            Some(so) => self.stroke(&so, l.stroke_width.as_ref(), &format!("{path}.stroke")).map(|(sc, c, w)| {
                n.base[prop::STROKE..prop::STROKE + 4].copy_from_slice(&rgba_to_oklab(c));
                n.base[prop::SW] = w;
                sc
            }),
            None => None,
        };
        self.set(n, prop::TRIM0, l.trim_start.as_ref(), &format!("{path}.trimStart"), 1.0);
        self.set(n, prop::TRIM1, l.trim_end.as_ref(), &format!("{path}.trimEnd"), 1.0);
        (Kind::Shape { shape: kind, fill, stroke, path: p }, size)
    }

    fn image_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>) -> (Kind, [f32; 2]) {
        let src = l.src.clone().unwrap_or_default();
        let fit = match l.fit.as_deref() {
            None | Some("contain") => 0,
            Some("cover") => 1,
            Some("fill") => 2,
            Some("none") => 3,
            Some(x) => {
                self.d.push(Diagnostic::error(format!("{path}.fit"), format!("unknown fit '{x}'")).suggest(suggest(x, val::FITS)));
                0
            }
        };
        let full = self.o.base_dir.join(&src);
        let (id, nat) = if let Some(&(i, nat)) = self.image_ids.get(&full) {
            (i, nat)
        } else if !full.exists() {
            self.d.push(Diagnostic::error(format!("{path}.src"), format!("file not found: {}", full.display())));
            (u32::MAX, [gw.unwrap_or(100.0), gh.unwrap_or(100.0)])
        } else if !self.o.load_assets {
            let dims = image::image_dimensions(&full).map(|(w, h)| [w as f32, h as f32]).unwrap_or([100.0, 100.0]);
            (u32::MAX, dims)
        } else {
            match image::open(&full).map_err(|e| e.to_string()).and_then(|img| {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                self.assets.add_rgba(w, h, rgba.into_raw()).map(|id| (id, [w as f32, h as f32]))
            }) {
                Ok((id, nat)) => {
                    self.assets.add_media(&src, id);
                    self.image_ids.insert(full, (id.0, nat));
                    (id.0, nat)
                }
                Err(e) => {
                    self.d.push(Diagnostic::error(format!("{path}.src"), format!("can't decode image: {e}")));
                    (u32::MAX, [100.0, 100.0])
                }
            }
        };
        (Kind::Image { image: ImageId(id), natural: nat, fit }, nat)
    }

    #[allow(clippy::too_many_arguments)]
    fn box_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>, start: f32, end: f32, pc: PCtx, n: &mut Node) -> (Kind, [f32; 2]) {
        use taffy::prelude::*;
        let st = match &l.style {
            Some(StyleRef::Box(b)) => b.clone(),
            Some(StyleRef::Name(s)) => {
                self.d.push(Diagnostic::warning(format!("{path}.style"), format!("a box's style is an object like {{\"direction\": \"column\", \"gap\": 24}}, not '{s}'")));
                spec::BoxStyle::default()
            }
            None => spec::BoxStyle::default(),
        };
        let sp = format!("{path}.style");
        let ref_size = [gw.unwrap_or(pc.size[0]), gh.unwrap_or(pc.size[1])];
        let cpc = PCtx { size: ref_size, t0: start, end, delay: 0.0, index: 0, count: 0, in_box: true };
        let kids_spec = l.children.as_deref().unwrap_or(&[]);
        let mut kids = self.layers(kids_spec, &format!("{path}.children"), cpc, l.stagger.as_ref());
        let gap = st.gap.as_ref().map(|g| self.len_c(g, &format!("{sp}.gap"), ref_size[0], 16.0)).unwrap_or(16.0);
        let pad: [f32; 4] = match &st.padding {
            None => [0.0; 4],
            Some(spec::Padding::One(v)) => [self.len_c(v, &format!("{sp}.padding"), ref_size[0], 16.0); 4],
            Some(spec::Padding::Many(v)) => {
                let vals: Vec<f32> = v.iter().enumerate().map(|(i, x)| self.len_c(x, &format!("{sp}.padding[{i}]"), ref_size[0], 16.0)).collect();
                match vals.len() {
                    2 => [vals[0], vals[1], vals[0], vals[1]],
                    4 => [vals[0], vals[1], vals[2], vals[3]],
                    1 => [vals[0]; 4],
                    _ => {
                        self.d.push(Diagnostic::error(format!("{sp}.padding"), "use one value, [vertical, horizontal] or [top, right, bottom, left]"));
                        [0.0; 4]
                    }
                }
            }
        };
        let column = match st.direction.as_deref() {
            None | Some("row") => false,
            Some("column") => true,
            Some(x) => {
                self.d.push(Diagnostic::error(format!("{sp}.direction"), format!("unknown direction '{x}'")).suggest(suggest(x, &["row", "column"])));
                false
            }
        };
        let align = match st.align.as_deref() {
            None | Some("center") => AlignItems::CENTER,
            Some("start") => AlignItems::FLEX_START,
            Some("end") => AlignItems::FLEX_END,
            Some("stretch") => AlignItems::STRETCH,
            Some(x) => {
                self.d.push(Diagnostic::error(format!("{sp}.align"), format!("unknown align '{x}'")).suggest(suggest(x, val::BOX_ALIGNS)));
                AlignItems::CENTER
            }
        };
        let justify = match st.justify.as_deref() {
            None | Some("start") => JustifyContent::FLEX_START,
            Some("center") => JustifyContent::CENTER,
            Some("end") => JustifyContent::FLEX_END,
            Some("spaceBetween") => JustifyContent::SPACE_BETWEEN,
            Some("spaceAround") => JustifyContent::SPACE_AROUND,
            Some("spaceEvenly") => JustifyContent::SPACE_EVENLY,
            Some(x) => {
                self.d.push(Diagnostic::error(format!("{sp}.justify"), format!("unknown justify '{x}'")).suggest(suggest(x, val::JUSTIFIES)));
                JustifyContent::FLEX_START
            }
        };
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let mut ids = Vec::new();
        for (k, c) in kids.iter().enumerate() {
            let grow = match kids_spec[k].style.as_ref() {
                Some(StyleRef::Box(b)) => b.grow.unwrap_or(0.0) as f32,
                _ => 0.0,
            };
            let s = Style {
                size: Size { width: length(c.base[prop::W]), height: length(c.base[prop::H]) },
                flex_grow: grow,
                flex_shrink: 0.0,
                ..Default::default()
            };
            ids.push(tree.new_leaf(s).unwrap());
        }
        let mut rs = Style {
            display: Display::Flex,
            flex_direction: if column { FlexDirection::Column } else { FlexDirection::Row },
            flex_wrap: if st.wrap == Some(true) { FlexWrap::Wrap } else { FlexWrap::NoWrap },
            gap: Size { width: length(gap), height: length(gap) },
            padding: Rect { left: length(pad[3]), right: length(pad[1]), top: length(pad[0]), bottom: length(pad[2]) },
            align_items: align,
            justify_content: justify,
            size: Size { width: gw.map(length).unwrap_or(Dimension::auto()), height: gh.map(length).unwrap_or(Dimension::auto()) },
            ..Default::default()
        };
        if let Some(cols) = st.columns {
            rs.display = Display::Grid;
            rs.grid_template_columns = (0..cols.max(1)).map(|_| fr(1.0)).collect();
            rs.justify_items = align;
        }
        let root = tree.new_with_children(rs, &ids).unwrap();
        let _ = tree.compute_layout(root, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent });
        for (c, id) in kids.iter_mut().zip(&ids) {
            let lay = tree.layout(*id).unwrap();
            let nv = [
                (prop::W, lay.size.width),
                (prop::H, lay.size.height),
                (prop::X, lay.location.x + c.anchor[0] * lay.size.width),
                (prop::Y, lay.location.y + c.anchor[1] * lay.size.height),
            ];
            for (slot, v) in nv {
                // Tracks were built against the pre-layout base: move them with it.
                let d = v - c.base[slot];
                c.base[slot] = v;
                for tr in c.tracks.iter_mut().filter(|t| t.slot as usize == slot) {
                    for g in &mut tr.segs {
                        g.v0 += d;
                        g.v1 += d;
                    }
                }
            }
        }
        let rl = tree.layout(root).unwrap();
        let size = [rl.size.width, rl.size.height];
        let bg = match &st.background {
            Some(b) => self.paint(b, &format!("{sp}.background")).map(|(p, c)| {
                n.base[prop::FILL..prop::FILL + 4].copy_from_slice(&rgba_to_oklab(c));
                p
            }),
            None => None,
        };
        if let Some(r) = &st.radius {
            self.set(n, prop::RADIUS, Some(r), &format!("{sp}.radius"), size[0].min(size[1]));
        } else if bg.is_some() {
            n.base[prop::RADIUS] = self.theme.radius;
        }
        if let Some(r) = &l.radius {
            self.set(n, prop::RADIUS, Some(r), &format!("{path}.radius"), size[0].min(size[1]));
        }
        let border = match &st.border {
            Some(b) => {
                let mut it = b.splitn(2, ' ');
                let w = it.next().and_then(|x| self.parse_len(x, 1.0, 16.0));
                let c = it.next().map(|x| x.trim().to_string());
                match (w, c) {
                    (Some(w), Some(c)) => self.color(&c, &format!("{sp}.border")).map(|c| (w, c)),
                    _ => {
                        self.d.push(Diagnostic::error(format!("{sp}.border"), format!("expected \"<width> <colour>\" like \"2px #ffffff30\", got '{b}'")));
                        None
                    }
                }
            }
            None => None,
        };
        if l.fill.is_some() {
            self.d.push(Diagnostic::warning(format!("{path}.fill"), "a box's colour is style.background").suggest(Some("style.background")));
        }
        (Kind::Box { children: kids, bg, border }, size)
    }

    fn component_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>) -> Option<(Kind, [f32; 2])> {
        let raw = l.component.clone().unwrap_or_default();
        // Accept camelCase (`barChart`) as well as the catalogue's kebab-case (`bar-chart`).
        let mut name = String::new();
        for ch in raw.chars() {
            if ch.is_ascii_uppercase() {
                name.push('-');
                name.push(ch.to_ascii_lowercase());
            } else {
                name.push(ch);
            }
        }
        let names: Vec<&str> = crate::components::KINDS.iter().map(|k| k.name).collect();
        if !names.contains(&name.as_str()) {
            self.d.push(Diagnostic::error(format!("{path}.component"), format!("unknown component '{raw}'")).suggest(suggest(&name, &names)));
            return None;
        }
        let props = l.props.clone().unwrap_or(serde_json::Value::Object(Default::default()));
        match crate::components::build(&name, &props) {
            Ok(p) => {
                let size = [gw.or(p.w).unwrap_or(self.w), gh.or(p.h).unwrap_or(self.h)];
                Some((Kind::Component(p.component, p.theme), size))
            }
            Err(e) => {
                self.d.push(Diagnostic::error(format!("{path}.props"), e.0));
                None
            }
        }
    }

    // ---------- animation ----------

    fn anim_val(&mut self, name: &str, v: &serde_json::Value, path: &str, refs: [f32; 2]) -> Option<ValS> {
        let (slot, ch) = prop::lookup(name)?;
        let refl = match slot {
            prop::X | prop::W => refs[0],
            prop::Y | prop::H => refs[1],
            prop::BLUR | prop::SIZE => self.h,
            _ => 1.0,
        };
        if ch == 4 {
            let s = v.as_str().unwrap_or("");
            return self.color(s, path).map(|c| ValS::Color(rgba_to_oklab(c)));
        }
        match v {
            serde_json::Value::Number(x) => Some(ValS::Abs(x.as_f64().unwrap_or(0.0) as f32)),
            serde_json::Value::String(s) => {
                let s = s.trim();
                let rel = |me: &Self, r: &str| me.parse_len(r, refl, 16.0);
                if let Some(r) = s.strip_prefix('+') {
                    rel(self, r).map(ValS::Add).or_else(|| self.bad_val(path, s))
                } else if let Some(r) = s.strip_prefix('*') {
                    r.trim().parse::<f32>().ok().map(ValS::Mul).or_else(|| self.bad_val(path, s))
                } else if s.starts_with('-') && !s.starts_with("-=") {
                    rel(self, &s[1..]).map(|x| ValS::Add(-x)).or_else(|| self.bad_val(path, s))
                } else {
                    let n = Num::Str(s.to_string());
                    Some(ValS::Abs(self.len_c(&n, path, refl, 16.0)))
                }
            }
            _ => self.bad_val(path, &v.to_string()),
        }
    }

    fn bad_val(&mut self, path: &str, s: &str) -> Option<ValS> {
        self.d.push(Diagnostic::error(path, format!("invalid value {s} (use a number, a unit string, \"+40\", \"-40\" or \"*1.5\")")));
        None
    }

    fn props_map(&mut self, m: &BTreeMap<String, serde_json::Value>, path: &str, refs: [f32; 2]) -> Vec<(usize, usize, ValS)> {
        let mut out = Vec::new();
        for (k, v) in m {
            let p = format!("{path}.{k}");
            match prop::lookup(k) {
                Some((slot, ch)) => {
                    if let Some(x) = self.anim_val(k, v, &p, refs) {
                        out.push((slot, ch, x));
                    }
                }
                None => {
                    self.d.push(Diagnostic::error(p, format!("'{k}' is not animatable")).suggest(suggest(k, val::ANIM_PROPS)));
                }
            }
        }
        out
    }

    fn tween(&mut self, t: &spec::Tween, path: &str, origin: f32, refs: [f32; 2], prev: &mut [TweenC]) -> Option<TweenC> {
        let at = match &t.at {
            Some(Num::Str(s)) if s.starts_with('+') && !s.starts_with("+@") => {
                let after = prev.iter().map(|x| x.at + x.dur).fold(0.0, f32::max);
                after + self.time_raw(&Num::Str(s[1..].to_string()), &format!("{path}.at"))
            }
            Some(a) => self.time_rel(a, &format!("{path}.at"), origin),
            None => 0.0,
        };
        let (ease, mut dur) = match &t.spring {
            Some(s) => {
                if t.ease.is_some() {
                    self.d.push(Diagnostic::warning(format!("{path}.ease"), "ignored because 'spring' is set"));
                }
                self.spring(s, &format!("{path}.spring"))
            }
            None => (self.ease(t.ease.as_deref(), "outCubic", &format!("{path}.ease")), 0.6),
        };
        if let Some(d) = &t.duration {
            dur = self.time_raw(d, &format!("{path}.duration"));
            if dur < 0.0 {
                self.d.push(Diagnostic::error(format!("{path}.duration"), "must not be negative"));
                dur = 0.0;
            }
        }
        let from = t.from.as_ref().map(|m| self.props_map(m, &format!("{path}.from"), refs)).unwrap_or_default();
        let to = t.to.as_ref().map(|m| self.props_map(m, &format!("{path}.to"), refs)).unwrap_or_default();
        let mut kf = Vec::new();
        if let Some(k) = &t.keyframes {
            for (name, vals) in k {
                let p = format!("{path}.keyframes.{name}");
                let Some((slot, ch)) = prop::lookup(name) else {
                    self.d.push(Diagnostic::error(p, format!("'{name}' is not animatable")).suggest(suggest(name, val::ANIM_PROPS)));
                    continue;
                };
                if vals.len() < 2 {
                    self.d.push(Diagnostic::error(p, "keyframes need at least two values"));
                    continue;
                }
                let n = vals.len();
                let mut list = Vec::new();
                for (i, v) in vals.iter().enumerate() {
                    let (tt, vv) = match v {
                        serde_json::Value::Array(a) if a.len() == 2 && a[0].is_number() => (a[0].as_f64().unwrap() as f32, &a[1]),
                        _ => (i as f32 / (n - 1) as f32, v),
                    };
                    if let Some(x) = self.anim_val(name, vv, &format!("{p}[{i}]"), refs) {
                        list.push((tt.clamp(0.0, 1.0), x));
                    }
                }
                kf.push((slot, ch, list));
            }
        }
        if from.is_empty() && to.is_empty() && kf.is_empty() {
            self.d.push(Diagnostic::warning(path, "tween has no 'from', 'to' or 'keyframes'; nothing animates"));
            return None;
        }
        Some(TweenC { at, dur, ease, from, to, kf, repeat: t.repeat.unwrap_or(0), yoyo: t.yoyo.unwrap_or(false) })
    }

    /// Build per-slot tracks from tweens.
    fn tracks(&mut self, base: &[f32; prop::N], exprs: &[(u8, Expr)], tw: &mut [TweenC], life: f32) -> Vec<Track> {
        tw.sort_by(|a, b| a.at.partial_cmp(&b.at).unwrap_or(std::cmp::Ordering::Equal));
        let dynamic = |s: usize| exprs.iter().any(|(x, _)| *x as usize == s);
        let mut tracks: Vec<Track> = Vec::new();
        let easings = &self.easings;
        let cur = |tracks: &Vec<Track>, s: usize, t: f32| -> f32 {
            let b = if dynamic(s) { f32::NAN } else { base[s] };
            match tracks.iter().find(|x| x.slot as usize == s) {
                Some(tr) if !b.is_nan() => tr.sample(t, b, easings),
                Some(tr) => {
                    // Dynamic base: only resolve if a segment fully determines it.
                    let v = tr.sample(t, 0.0, easings);
                    let w = tr.sample(t, 1.0, easings);
                    if v == w {
                        v
                    } else {
                        f32::NAN
                    }
                }
                None => b,
            }
        };
        let apply = |v: ValS, rel: f32, ch: usize| -> f32 {
            match v {
                ValS::Abs(x) => x,
                ValS::Add(x) => rel + x,
                ValS::Mul(x) => rel * x,
                ValS::Color(c) => c[ch],
            }
        };
        let push = |tracks: &mut Vec<Track>, s: usize, seg: Seg| {
            let i = match tracks.iter().position(|x| x.slot as usize == s) {
                Some(i) => i,
                None => {
                    tracks.push(Track { slot: s as u8, segs: Vec::new() });
                    tracks.len() - 1
                }
            };
            let first = tracks[i].segs.is_empty();
            let mut seg = seg;
            seg.hold_before &= first;
            tracks[i].segs.push(seg);
        };
        for t in tw.iter() {
            let (t0, t1) = (t.at, t.at + t.dur);
            let mut slots: Vec<(usize, Option<ValS>, Option<ValS>)> = Vec::new();
            for &(slot, ch, v) in &t.from {
                for c in 0..ch {
                    let e = slots.iter_mut().find(|x| x.0 == slot + c);
                    let vv = if ch == 4 { ValS::Abs(apply(v, 0.0, c)) } else { v };
                    match e {
                        Some(e) => e.1 = Some(vv),
                        None => slots.push((slot + c, Some(vv), None)),
                    }
                }
            }
            for &(slot, ch, v) in &t.to {
                for c in 0..ch {
                    let vv = if ch == 4 { ValS::Abs(apply(v, 0.0, c)) } else { v };
                    match slots.iter_mut().find(|x| x.0 == slot + c) {
                        Some(e) => e.2 = Some(vv),
                        None => slots.push((slot + c, None, Some(vv))),
                    }
                }
            }
            let mut segs: Vec<(usize, Seg)> = Vec::new();
            for (s, f, to) in slots {
                let c0 = cur(&tracks, s, t0);
                let base_v = if dynamic(s) { f32::NAN } else { base[s] };
                let (v0, v1, hold) = match (f, to) {
                    (Some(f), None) => (apply(f, base_v, 0), base_v, true),
                    (None, Some(to)) => (c0, apply(to, c0, 0), false),
                    (Some(f), Some(to)) => {
                        let a = apply(f, c0, 0);
                        (a, apply(to, a, 0), true)
                    }
                    (None, None) => continue,
                };
                segs.push((s, Seg { t0, t1, v0, v1, ease: t.ease, hold_before: hold }));
            }
            for &(slot, ch, ref list) in &t.kf {
                for c in 0..ch {
                    let s = slot + c;
                    let c0 = cur(&tracks, s, t0);
                    let mut prev: Option<(f32, f32)> = None;
                    for &(tt, v) in list {
                        let val = match v {
                            ValS::Color(col) => col[c],
                            v => apply(v, c0, 0),
                        };
                        if let Some((pt, pv)) = prev {
                            segs.push((s, Seg { t0: t0 + pt * t.dur, t1: t0 + tt * t.dur, v0: pv, v1: val, ease: t.ease, hold_before: false }));
                        } else if tt > 0.0 {
                            segs.push((s, Seg { t0, t1: t0 + tt * t.dur, v0: c0, v1: val, ease: t.ease, hold_before: false }));
                        }
                        prev = Some((tt, val));
                    }
                }
            }
            // Repeats.
            let reps = if t.repeat < 0 {
                if t.dur > 0.0 && life.is_finite() {
                    ((life - t1) / t.dur).ceil().clamp(0.0, 10000.0) as i32
                } else {
                    0
                }
            } else {
                t.repeat.min(10000)
            };
            let base_segs = segs.clone();
            for k in 1..=reps {
                for (s, g) in &base_segs {
                    let off = t.dur * k as f32;
                    let flip = t.yoyo && k % 2 == 1;
                    let (v0, v1) = if flip { (g.v1, g.v0) } else { (g.v0, g.v1) };
                    let (a, b) = if flip { (t0 + (t1 - g.t1) + off, t0 + (t1 - g.t0) + off) } else { (g.t0 + off, g.t1 + off) };
                    segs.push((*s, Seg { t0: a, t1: b, v0, v1, ease: g.ease, hold_before: false }));
                }
            }
            for (s, g) in segs {
                push(&mut tracks, s, g);
            }
        }
        for tr in &mut tracks {
            tr.segs.sort_by(|a, b| a.t0.partial_cmp(&b.t0).unwrap_or(std::cmp::Ordering::Equal));
        }
        tracks
    }

    #[allow(clippy::too_many_arguments)]
    fn preset(&mut self, p: &PresetObj, path: &str, exit: bool, delay: f32, cx: PresetCx, tw: &mut Vec<TweenC>, n: &mut Node) {
        let name = p.preset.clone().unwrap_or_default();
        let list = if exit { val::EXIT_PRESETS } else { val::ENTER_PRESETS };
        if !list.contains(&name.as_str()) {
            let other = if exit { val::ENTER_PRESETS } else { val::EXIT_PRESETS };
            let hint = if other.contains(&name.as_str()) {
                format!(" ('{name}' is an {} preset)", if exit { "enter" } else { "exit" })
            } else {
                String::new()
            };
            self.d.push(Diagnostic::error(path, format!("unknown {} preset '{name}'{hint}", if exit { "exit" } else { "enter" })).suggest(suggest(&name, list)));
            return;
        }
        let (h, w) = (self.h, self.w);
        let dist = p.distance.as_ref().map(|d| self.len_c(d, &format!("{path}.distance"), h, 16.0));
        let pd = |d: f32| ValS::Add(d);
        use prop::*;
        // (props, default duration, default ease)
        let (props, dd, de): (Vec<(usize, usize, ValS)>, f32, &str) = match name.trim_end_matches("In").trim_end_matches("Out") {
            "fade" => (vec![(OPACITY, 1, ValS::Abs(0.0))], 0.5, "outCubic"),
            "fadeUp" | "fadeDown" | "fadeLeft" | "fadeRight" => {
                let d = dist.unwrap_or(40.0);
                // Enter: comes from the opposite side; exit: leaves towards the named side.
                let s = if exit { -1.0 } else { 1.0 };
                let (slot, v) = match name.as_str() {
                    "fadeUp" => (Y, d * s),
                    "fadeDown" => (Y, -d * s),
                    "fadeLeft" => (X, d * s),
                    _ => (X, -d * s),
                };
                (vec![(OPACITY, 1, ValS::Abs(0.0)), (slot, 1, pd(v))], 0.6, "outCubic")
            }
            "slideUp" | "slideDown" | "slideLeft" | "slideRight" => {
                let s = if exit { -1.0 } else { 1.0 };
                let (slot, v) = match name.as_str() {
                    "slideUp" => (Y, dist.unwrap_or(h * 0.15) * s),
                    "slideDown" => (Y, -dist.unwrap_or(h * 0.15) * s),
                    "slideLeft" => (X, dist.unwrap_or(w * 0.15) * s),
                    _ => (X, -dist.unwrap_or(w * 0.15) * s),
                };
                (vec![(OPACITY, 1, ValS::Abs(0.0)), (slot, 1, pd(v))], 0.7, "outExpo")
            }
            "pop" => (vec![(SX, 2, ValS::Abs(0.0)), (OPACITY, 1, ValS::Abs(0.0))], 0.5, "outBack"),
            "scale" => (vec![(SX, 2, ValS::Mul(0.8)), (OPACITY, 1, ValS::Abs(0.0))], 0.5, "outCubic"),
            "zoom" => (vec![(SX, 2, ValS::Mul(1.4)), (OPACITY, 1, ValS::Abs(0.0)), (BLUR, 1, ValS::Abs(20.0))], 0.7, "outExpo"),
            "blur" => (vec![(BLUR, 1, ValS::Abs(30.0)), (OPACITY, 1, ValS::Abs(0.0))], 0.6, "outCubic"),
            "rotate" => (vec![(ROT, 1, ValS::Add(-90.0)), (SX, 2, ValS::Mul(0.6)), (OPACITY, 1, ValS::Abs(0.0))], 0.6, "outBack"),
            "flip" => (vec![(SX, 1, ValS::Abs(0.0))], 0.5, "outCubic"),
            "bounce" => (vec![(Y, 1, ValS::Add(-dist.unwrap_or(h * 0.3))), (OPACITY, 1, ValS::Abs(0.0))], 0.9, "bouncy"),
            "draw" => {
                if !matches!(n.kind, Kind::Shape { stroke: Some(_), .. }) {
                    self.d.push(Diagnostic::warning(path, "drawOn/drawOff animate a stroke; this layer has no stroke"));
                }
                if exit {
                    (vec![(TRIM0, 1, ValS::Abs(1.0))], 1.0, "inOutCubic")
                } else {
                    (vec![(TRIM1, 1, ValS::Abs(0.0))], 1.0, "inOutCubic")
                }
            }
            "typewriter" => {
                if !matches!(n.kind, Kind::Text { .. }) {
                    self.d.push(Diagnostic::warning(path, "typewriter applies to text layers"));
                }
                let cps = p.cps.unwrap_or(20.0).max(0.1) as f32;
                (vec![(REVEAL, 1, ValS::Abs(0.0))], cx.chars.max(1) as f32 / cps, "linear")
            }
            "wipe" => {
                n.wipe_dir = match p.direction.as_deref() {
                    None | Some("right") => 0,
                    Some("left") => 1,
                    Some("down") => 2,
                    Some("up") => 3,
                    Some(d) => {
                        self.d.push(Diagnostic::error(format!("{path}.direction"), format!("unknown direction '{d}'")).suggest(suggest(d, val::DIRECTIONS)));
                        0
                    }
                };
                (vec![(WIPE, 1, ValS::Abs(0.0))], 0.6, "inOutCubic")
            }
            "countUp" => {
                if !matches!(n.kind, Kind::Text { counter: Some(_), .. }) {
                    self.d.push(Diagnostic::warning(path, "countUp needs a text layer with {value} in its text"));
                }
                (vec![(VALUE, 1, ValS::Abs(0.0))], 1.5, "outCubic")
            }
            _ => return,
        };
        if cx.unit && props.iter().any(|(s, _, _)| matches!(*s, TRIM0 | TRIM1 | REVEAL | VALUE | WIPE)) {
            self.d.push(Diagnostic::warning(path, format!("'{name}' doesn't work per unit with 'split'; use fadeUp, popIn, blurIn, …")));
            return;
        }
        let exit_ease = |e: &str| -> String {
            if !exit {
                return e.to_string();
            }
            match e.strip_prefix("out") {
                Some(r) => format!("in{r}"),
                None => e.to_string(),
            }
        };
        let (ease, mut dur) = match &p.spring {
            Some(s) => self.spring(s, &format!("{path}.spring")),
            None => {
                let e = exit_ease(de);
                let i = self.ease(p.ease.as_deref(), &e, &format!("{path}.ease"));
                let d = self.easings[i as usize].natural_duration().map(|x| x as f32).filter(|_| p.ease.is_none() && de == "bouncy").unwrap_or(dd);
                (i, d)
            }
        };
        if let Some(d) = &p.duration {
            dur = self.time_raw(d, &format!("{path}.duration"));
        }
        let pdelay = p.delay.as_ref().map(|d| self.time_raw(d, &format!("{path}.delay"))).unwrap_or(0.0);
        if exit {
            if !cx.life.is_finite() || cx.life > 1e5 {
                self.d.push(Diagnostic::warning(path, "exit needs a known end (set 'end' or 'duration' on the layer, or a scene duration)"));
                return;
            }
            let at = (cx.life - dur - pdelay).max(0.0);
            tw.push(TweenC { at, dur, ease, from: vec![], to: props, kf: vec![], repeat: 0, yoyo: false });
        } else {
            // Opacity in the move presets reaches 1 a little sooner, so things don't look washed out.
            tw.push(TweenC { at: delay + pdelay, dur, ease, from: props, to: vec![], kf: vec![], repeat: 0, yoyo: false });
        }
        let _ = cx.refs;
    }

    // ---------- audio & captions ----------

    fn audio(&mut self, a: &spec::AudioCue, path: &str, scene_start: f32) -> Option<AudioClip> {
        let full = self.o.base_dir.join(&a.src);
        if !full.exists() {
            self.d.push(Diagnostic::error(format!("{path}.src"), format!("file not found: {}", full.display())));
        }
        let at = scene_start + a.at.as_ref().map(|x| self.time_rel(x, &format!("{path}.at"), 0.0)).unwrap_or(0.0);
        let volume = match &a.volume {
            None => 1.0,
            Some(Num::Num(v)) => *v as f32,
            Some(Num::Str(s)) => match s.trim().strip_suffix("dB").or_else(|| s.trim().strip_suffix("db")).and_then(|x| x.trim().parse::<f32>().ok()) {
                Some(db) => 10f32.powf(db / 20.0),
                None => {
                    self.d.push(Diagnostic::error(format!("{path}.volume"), format!("invalid volume '{s}' (0..1 or \"-6dB\")")));
                    1.0
                }
            },
        };
        let trim_start = a.trim_start.as_ref().map(|x| self.time_raw(x, &format!("{path}.trimStart"))).unwrap_or(0.0);
        let duration = a.duration.as_ref().map(|x| self.time_raw(x, &format!("{path}.duration")));
        if a.duck.is_some() || a.voice.is_some() {
            self.d.push(Diagnostic::warning(path, "ducking is not supported yet; 'duck' and 'voice' are ignored"));
        }
        Some(AudioClip {
            src: full,
            at,
            volume,
            trim_start,
            duration,
            fade_in: a.fade_in.unwrap_or(0.0) as f32,
            fade_out: a.fade_out.unwrap_or(0.0) as f32,
            looped: a.loop_.unwrap_or(false),
        })
    }

    fn captions(&mut self, cap: &spec::Captions, duration: f32) -> Vec<Node> {
        let (cues, style, y, color): (Vec<spec::CaptionCue>, String, Option<Num>, Option<String>) = match cap {
            spec::Captions::Cues(c) => (c.clone(), "caption".into(), None, None),
            spec::Captions::File(f) => {
                let mut cues = f.cues.clone().unwrap_or_default();
                if let Some(src) = &f.src {
                    let full = self.o.base_dir.join(src);
                    match std::fs::read_to_string(&full) {
                        Ok(s) => cues.extend(parse_srt(&s)),
                        Err(e) => self.d.push(Diagnostic::error("captions.src", format!("can't read {}: {e}", full.display()))),
                    }
                }
                (cues, f.style.clone().unwrap_or_else(|| "caption".into()), f.y.clone(), f.color.clone())
            }
        };
        let mut out = Vec::new();
        for (i, c) in cues.iter().enumerate() {
            let l = Layer {
                text: Some(c.text.clone()),
                style: Some(StyleRef::Name(style.clone())),
                color: Some(spec::Paint::Css(color.clone().unwrap_or_else(|| "$text".into()))),
                y: Some(y.clone().unwrap_or(Num::Str("88%".into()))),
                width: Some(Num::Str("80%".into())),
                start: Some(c.at.clone()),
                end: Some(c.end.clone()),
                shadow: Some(spec::Shadow(spec::ShadowObj { x: Some(0.0), y: Some(2.0), blur: Some(8.0), color: Some("#000000c0".into()) })),
                enter: Some(spec::Preset(PresetObj { preset: Some("fadeIn".into()), duration: Some(Num::Num(0.15)), ..Default::default() })),
                ..Default::default()
            };
            let pc = PCtx { size: [self.w, self.h], t0: 0.0, end: duration, delay: 0.0, index: i, count: cues.len(), in_box: false };
            if let Some(n) = self.layer(&l, &format!("captions[{i}]"), pc) {
                out.push(n);
            }
        }
        out
    }
}

#[derive(Clone, Copy)]
struct PresetCx {
    life: f32,
    chars: usize,
    refs: [f32; 2],
    unit: bool,
}

/// Split on commas not inside parentheses.
fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut st = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[st..i]);
                st = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[st..]);
    out
}

/// Minimal SRT parser.
pub(crate) fn parse_srt(s: &str) -> Vec<spec::CaptionCue> {
    let ts = |x: &str| -> Option<f64> {
        let x = x.trim().replace(',', ".");
        let p: Vec<&str> = x.split(':').collect();
        if p.len() != 3 {
            return None;
        }
        Some(p[0].parse::<f64>().ok()? * 3600.0 + p[1].parse::<f64>().ok()? * 60.0 + p[2].parse::<f64>().ok()?)
    };
    let mut out = Vec::new();
    for block in s.replace("\r\n", "\n").split("\n\n") {
        let lines: Vec<&str> = block.lines().filter(|l| !l.trim().is_empty()).collect();
        let Some(k) = lines.iter().position(|l| l.contains("-->")) else { continue };
        let mut it = lines[k].split("-->");
        let (Some(a), Some(b)) = (it.next().and_then(ts), it.next().and_then(ts)) else { continue };
        out.push(spec::CaptionCue { text: lines[k + 1..].join("\n"), at: Num::Num(a), end: Num::Num(b), words: None });
    }
    out
}

/// Map the spec theme onto the components' theme.
fn components_theme(t: &Theme) -> crate::components::Theme {
    let mut c = crate::components::Theme::dark();
    let get = |k: &str| t.colors.get(k).copied();
    if let Some(v) = get("bg") {
        c.dark = (v.0[0] + v.0[1] + v.0[2]) < 1.5;
        if !c.dark {
            c = crate::components::Theme::light();
        }
        c.background = v;
    }
    if let Some(v) = get("surface") {
        c.surface = v;
    }
    if let Some(v) = get("text") {
        c.text = v;
    }
    if let Some(v) = get("muted") {
        c.muted = v;
    }
    if let Some(v) = get("primary") {
        c.accent = v;
    }
    let mut pal: Vec<Rgba> = ["primary", "secondary", "accent"].iter().filter_map(|k| get(k)).collect();
    for p in &c.palette {
        if pal.len() >= 8 {
            break;
        }
        pal.push(*p);
    }
    c.palette = pal;
    c.radius = t.radius;
    c
}
