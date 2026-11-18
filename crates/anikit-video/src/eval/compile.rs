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
    /// Share an existing asset store (fonts and images) instead of making a new one.
    pub assets: Option<Arc<SharedAssets>>,
}

impl Default for Options {
    fn default() -> Self {
        Options { base_dir: PathBuf::from("."), fonts: None, load_assets: true, assets: None }
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
    /// Per property: `(time 0..1, value, ease of the move into this key)`.
    kf: Vec<(usize, usize, Vec<(f32, ValS, Option<u32>)>)>,
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
    /// Audio from video clips.
    audio_extra: Vec<crate::audio::Track>,
    layer_info: Vec<(String, String)>,
    /// Layers with an `sfx` key: (tag, value, path).
    layer_sfx: Vec<(u32, serde_json::Value, String)>,
    /// `audio[].sfx` cues naming sound-design effects (placed by their hit point).
    design_cues: Vec<crate::audio::design::Cue>,
    beat_marks: super::beat_markers::BeatMarkers,
    /// Music analysed for `iAudio`/`iBeat` (once, on first use).
    music: Option<Result<Arc<super::audio_react::AudioFeatures>, String>>,
    /// Uniform name → (slot after `prop::U0`, components) of the shader layer being compiled.
    uniform_slots: Vec<(String, usize, usize)>,
    /// Compiled sub-compositions by name, with their own `loop` default.
    comps: Vec<(String, Arc<Evaluator>, bool)>,
    /// The colour expression the last `paint` call compiled (claimed by the caller for its slot).
    last_cexpr: Option<super::expr::ColorExpr>,
    /// Music analysis for `beat`/`bar`/`level`/`bpm` in expressions (loaded on first use).
    groove: Option<Result<Arc<super::beat_markers::Music>, String>>,
    /// `groove` includes the loudness envelope.
    groove_level: bool,
    /// The spec's theme as written (nested compositions inherit it).
    spec_theme: Option<spec::Theme>,
    /// The video's grid, and the grid in force for the layers being compiled.
    video_grid: Option<spec::GridSpec>,
    grid: Option<spec::GridSpec>,
    /// The layer being compiled: its anchor and whether it is text (for `cell` tweens).
    cur_anchor: [f32; 2],
    cur_text: bool,
    /// Resting boxes `[left, top, right, bottom]` of the earlier siblings with an `id`, for `"=title.bottom + 24"`.
    sib: Vec<(String, [f32; 4])>,
    /// The generated music's notes (video seconds) and beats per bar, for `onNotes`.
    score: Option<Result<(Arc<Vec<crate::music::ScoreNote>>, f64), String>>,
    /// The music cue the score was bound to (rendered as is).
    pinned_music: Option<Arc<crate::music::MusicCue>>,
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

impl C<'_> {
    /// Resolve every font the spec names: library families are fetched on
    /// first use (an error if that fails), unknown names warn with suggestions.
    fn load_fonts(&mut self, spec: &Spec) {
        let Ok(v) = serde_json::to_value(spec) else { return };
        let fonts = self.assets.fonts.clone();
        // The `fonts` map: files relative to the spec; each key is an alias for the family it adds.
        for (key, file) in spec.fonts.iter().flatten() {
            let low = file.to_ascii_lowercase();
            if !(low.ends_with(".ttf") || low.ends_with(".otf") || low.ends_with(".ttc") || file.contains('/')) {
                // `"fonts": {"heading": "Fraunces"}`: a library family under a nickname.
                match fonts.ensure_family(file) {
                    Ok(_) => match fonts.resolve_family(file) {
                        Some(f) => fonts.add_alias(key, &f),
                        None => self.d.push(Diagnostic::error(format!("fonts.{key}"), format!("'{file}' is neither a font file nor a library family; see `anikit-video fonts`"))),
                    },
                    Err(e) => self.d.push(Diagnostic::error(format!("fonts.{key}"), e)),
                }
                continue;
            }
            match fonts.add_font_file(self.o.base_dir.join(file)) {
                Ok(names) if names.is_empty() => self.d.push(Diagnostic::error(format!("fonts.{key}"), format!("{file} contains no usable font"))),
                Ok(names) => {
                    if fonts.resolve_family(key).is_none() {
                        fonts.add_alias(key, &names[0]);
                    }
                }
                Err(e) => self.d.push(Diagnostic::error(format!("fonts.{key}"), format!("can't load {file}: {e}"))),
            }
        }
        let mut seen = std::collections::HashSet::new();
        for (path, name) in text::library::spec_font_names(&v) {
            // Theme tokens (`$heading`) resolve to their family; a token the
            // theme doesn't define is someone else's diagnostic.
            let name = if name.starts_with('$') { self.theme.font(&name).to_string() } else { name };
            if name.starts_with('$') || !seen.insert(name.to_lowercase()) {
                continue;
            }
            const GENERIC: &[&str] = &["serif", "sans-serif", "monospace", "system-ui", "cursive", "fantasy", "inherit", "default"];
            if GENERIC.contains(&name.to_lowercase().as_str()) || fonts.resolve_family(&name).is_some() {
                continue;
            }
            let exact = text::library::find(&name).is_some();
            match fonts.ensure_family(&name) {
                Err(e) => self.d.push(Diagnostic::error(path, e)),
                Ok(_) if fonts.resolve_family(&name).is_some() => {
                    if let (false, Some(f)) = (exact, text::library::find_loose(&name)) {
                        self.d.push(Diagnostic::warning(path, format!("font '{name}' resolved to the family '{}'; set weight/italic separately", f.name)));
                    }
                }
                Ok(_) => {
                    let s = text::library::suggest(&name);
                    let hint = if s.is_empty() { None } else { Some(s.join("', '")) };
                    self.d.push(
                        Diagnostic::warning(path, format!("unknown font '{name}' (falls back to Inter); see `anikit-video fonts`")).suggest(hint),
                    );
                }
            }
        }
    }
}

/// Visible ink bounds `[x0, y0, x1, y1]` of a layout (glyph outlines).
fn ink_bounds(fonts: &FontStore, lay: &text::TextLayout) -> [f32; 4] {
    use kurbo::Shape;
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for r in &lay.runs {
        let bb = fonts.run_path(r, &r.glyphs).bounding_box();
        if bb.width() > 0.0 {
            b = [b[0].min(bb.x0 as f32), b[1].min(bb.y0 as f32), b[2].max(bb.x1 as f32), b[3].max(bb.y1 as f32)];
        }
    }
    if b[0] > b[2] { [0.0, 0.0, lay.width, lay.height] } else { b }
}

fn anim_src_of(l: &Layer) -> String {
    l.animate.as_ref().map(|a| format!("{a:?}")).unwrap_or_default()
}

impl C<'_> {
    /// Static `axes` into the style; every axis (static or named in `animate`
    /// as `axes.TAG`) gets a uniform slot so it can be tweened.
    fn text_axes(&mut self, l: &Layer, anim: &str, style: &mut text::TextStyle, n: &mut Node, path: &str) {
        let mut tags: Vec<(String, Option<f32>)> = Vec::new();
        for (k, v) in l.axes.iter().flatten() {
            if k.len() != 4 || !k.bytes().all(|b| b.is_ascii_alphanumeric()) {
                self.d.push(Diagnostic::error(format!("{path}.axes.{k}"), "an axis tag is 4 letters, e.g. wght, wdth, opsz, slnt, ital or a custom one like SOFT"));
                continue;
            }
            tags.push((k.clone(), Some(*v as f32)));
        }
        let mut rest = anim;
        while let Some(i) = rest.find("axes.") {
            let t: String = rest[i + 5..].chars().take(4).collect();
            if t.len() == 4 && t.bytes().all(|b| b.is_ascii_alphanumeric()) && !tags.iter().any(|x| x.0 == t) {
                tags.push((t, None));
            }
            rest = &rest[i + 5..];
        }
        if tags.is_empty() {
            return;
        }
        if tags.len() > super::shader::UNIFORM_SLOTS {
            self.d.push(Diagnostic::error(format!("{path}.axes"), format!("at most {} axes", super::shader::UNIFORM_SLOTS)));
            tags.truncate(super::shader::UNIFORM_SLOTS);
        }
        // Axes with no static value start at the face's default.
        let probe = text::TextSpec { text: "a".into(), style: style.clone(), ..Default::default() };
        let face = self.assets.fonts.layout(&probe).runs.first().map(|r| r.font);
        for (i, (t, v)) in tags.into_iter().enumerate() {
            let v = v.or_else(|| face.and_then(|f| self.assets.fonts.axis_default(f, &t))).unwrap_or(0.0);
            style.axes.push((t.clone(), v));
            n.base[prop::U0 + i] = v;
            self.uniform_slots.push((format!("axes.{t}"), i, 1));
        }
    }
}

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
        Self::build_with(spec, opts, Vec::new())
    }

    fn build_with(spec: &Spec, opts: &Options, comps: Vec<(String, Arc<Evaluator>, bool)>) -> (Option<Evaluator>, Vec<Diagnostic>) {
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
            audio_extra: Vec::new(),
            layer_info: Vec::new(),
            layer_sfx: Vec::new(),
            design_cues: Vec::new(),
            beat_marks: super::beat_markers::BeatMarkers::new(spec, &opts.base_dir),
            music: None,
            assets: opts.assets.clone().unwrap_or_else(|| Arc::new(SharedAssets::new(opts.fonts.clone().unwrap_or_else(|| Arc::new(FontStore::new()))))),
            comps,
            spec_theme: spec.theme.clone(),
            groove: None,
            groove_level: false,
            last_cexpr: None,
            uniform_slots: Vec::new(),
            video_grid: spec.grid.clone(),
            grid: None,
            cur_anchor: [0.5, 0.5],
            cur_text: false,
            sib: Vec::new(),
            score: None,
            pinned_music: None,
        };
        c.assets.shaders.bind(&c.assets);
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
            if k == "palette" {
                c.d.push(Diagnostic::error("theme.palette", v));
            } else {
                c.d.push(Diagnostic::error(format!("theme.colors.{k}"), format!("invalid colour '{v}' ({COLOUR_FORMS})")));
            }
        }
        c.load_fonts(spec);
        if let Some(m) = &spec.markers {
            for (k, v) in m {
                let t = c.time_raw(v, &format!("markers.{k}"));
                c.gmarkers.insert(k.clone(), t);
            }
        }
        c.prewarm_shaders(spec);
        if let Some(defs) = &spec.compositions {
            c.compositions(defs);
        }
        let gpost = spec.post.as_ref().and_then(|p| c.post(p, "post"));
        let shutter = match &spec.motion_blur {
            None | Some(spec::MotionBlur::On(true)) => 0.5 / c.fps,
            Some(spec::MotionBlur::On(false)) => 0.0,
            Some(spec::MotionBlur::Shutter { shutter }) => {
                if !(0.0..=720.0).contains(shutter) {
                    c.d.push(Diagnostic::error("motionBlur.shutter", format!("{shutter}° is out of range (0..720)")));
                }
                (shutter.clamp(0.0, 720.0) as f32 / 360.0) / c.fps
            }
        };
        let encode_crf = spec.encode.as_ref().and_then(|e| e.crf).and_then(|q| {
            if (0.0..=51.0).contains(&q) {
                Some(q.round() as u8)
            } else {
                c.d.push(Diagnostic::error("encode.crf", format!("{q} is out of range (0..51)")));
                None
            }
        });
        let encode_aq = spec.encode.as_ref().and_then(|e| e.aq).filter(|a| {
            let ok = (0.0..=2.0).contains(a);
            if !ok {
                c.d.push(Diagnostic::error("encode.aq", format!("{a} is out of range (0..2)")));
            }
            ok
        });
        let default_bg = spec.background.clone().unwrap_or(spec::Paint::Css("$bg".into()));
        let default_trans = spec.transition.clone();
        if spec.scenes.is_empty() {
            c.d.push(Diagnostic::error("scenes", "a video needs at least one scene"));
            return (None, c.d);
        }
        // `"cut": "bar"` / `"2 beats"` / seconds: the length of scenes that don't set one.
        let default_dur = match &spec.cut {
            Some(cut) => {
                let d = c.time_raw(cut, "cut");
                if d > 0.0 { d } else { 3.0 }
            }
            None => 3.0,
        };
        if spec.scenes.iter().any(|s| any_on_notes(&s.layers)) {
            c.bind_score(spec, default_dur);
        }
        let mut scenes = Vec::new();
        let mut t = 0.0f32;
        let mut prev_dur = f32::INFINITY;
        let mut audio = Vec::new();
        for (i, s) in spec.scenes.iter().enumerate() {
            let path = format!("scenes[{i}]");
            let id = s.id.clone().unwrap_or_else(|| format!("scene{}", i + 1));
            let trans = if i == 0 {
                TransC { kind: TransKind::Cut, dur: 0.0, ease: 0, dir: [-1.0, 0.0], color: Rgba([0.0, 0.0, 0.0, 1.0]), origin: [0.0; 2], amount: 0.0, shader: None, layer: None, shape: 0, count: 0, carry: Vec::new() }
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
                None => default_dur,
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
            // Scene timing never shrinks: a transition into this scene happens during the
            // last `trans.dur` before its cut point, so cuts land at the sum of the scene
            // durations (on the beat with musical durations) and the video is exactly as long
            // as the durations add up to. The scene's own clock starts when the transition does.
            let own = dur;
            let in_dur = trans.dur;
            dur += trans.dur;
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
            let na = c.audio_extra.len();
            let mut built = c.scene(s, &path, dur, &default_bg);
            if auto {
                c.audio_extra.truncate(na);
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
            let depth = camera.base[prop::APERTURE] != 0.0
                || camera.tracks.iter().any(|t| t.slot as usize == prop::APERTURE)
                || layers.iter().any(|n| n.base[prop::DEPTH] != 0.0 || n.tracks.iter().any(|t| t.slot as usize == prop::DEPTH) || n.exprs.iter().any(|e| e.0 as usize == prop::DEPTH));
            // Leading layers that fill the frame and don't move are the backdrop.
            let (fw, fh) = (c.w, c.h);
            let bg_layers = layers
                .iter()
                .take_while(|n| {
                    let b = &n.base;
                    let covers = b[prop::W] >= fw - 1.0 && b[prop::H] >= fh - 1.0 && (b[prop::X] - n.anchor[0] * b[prop::W]) <= 1.0 && (b[prop::Y] - n.anchor[1] * b[prop::H]) <= 1.0;
                    let still = b[prop::ROT] == 0.0 && b[prop::SX] >= 1.0 && b[prop::SY] >= 1.0 && n.tracks.iter().all(|t| !matches!(t.slot as usize, prop::X | prop::Y | prop::W | prop::H | prop::SX | prop::SY | prop::ROT));
                    covers && still && n.start <= 0.0 && n.exprs.is_empty() && !matches!(n.kind, Kind::Text { .. })
                })
                .count();
            let post = match &s.post {
                Some(p) => c.post(p, &format!("{path}.post")),
                None => gpost.clone(),
            };
            scenes.push(SceneC { id, start, dur, bg, bg_color, trans, camera, shake, layers, depth, bg_layers, post });
            t = start + dur;
            prev_dur = if auto { dur - in_dur } else { own };
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
        audio.append(&mut c.audio_extra);
        let music = match (&spec.music, c.pinned_music.take()) {
            (_, Some(m)) => Some(m),
            (Some(v), None) => match crate::music::parse_with_cuts(v, &opts.base_dir, &scenes.iter().skip(1).map(|s| (s.start + 0.5 * s.trans.dur) as f64).collect::<Vec<_>>()) {
                Ok(m) => Some(Arc::new(m)),
                Err(e) => {
                    c.d.push(Diagnostic::error("music", e));
                    None
                }
            },
            (None, None) => None,
        };
        let sound = c.sound_plan(spec, duration);
        c.smarkers.clear();
        c.scene_start = 0.0;
        c.scene_dur = duration;
        let overlay = Vec::new();
        let captions = spec.captions.as_ref().and_then(|cap| c.captions(cap));
        let ev = Evaluator {
            width: c.w as u32,
            height: c.h as u32,
            fps: c.fps,
            duration,
            diagnostics: Vec::new(),
            audio,
            music,
            sound,
            assets: c.assets,
            easings: c.easings,
            scenes,
            overlay,
            layer_info: c.layer_info,
            captions,
            ctheme: components_theme(&c.theme),
            theme: c.theme,
            comps: c.comps.into_iter().map(|(n, e, _)| (n, e)).collect(),
            shutter,
            encode_aq: encode_aq.map(|a| a as f32),
            encode_crf,
            groove: c.groove.and_then(|m| m.ok()),
            shutter_explicit: matches!(spec.motion_blur, Some(spec::MotionBlur::On(true)) | Some(spec::MotionBlur::Shutter { .. })),
        };
        (Some(ev), c.d)
    }
}

const COLOUR_FORMS: &str = "use \"#ff5a1f\", \"#f51\", \"#ff5a1f80\" (with alpha), a name like \"white\", a theme token like \"$accent\", \"rgb(255, 90, 31)\", \"hsl(20, 100, 56)\", \"oklch(0.7, 0.19, 40)\", or a per-frame expression \"=mix($primary, $accent, p)\"";

fn any_on_notes(ls: &[Layer]) -> bool {
    ls.iter().any(|l| l.on_notes.is_some() || l.layers.as_deref().is_some_and(any_on_notes) || l.children.as_deref().is_some_and(any_on_notes))
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

/// Spring names whose anikit-core defaults overshoot more than motion design wants by default:
/// `bouncy` lands at ~8% here (core's is ~39%); `springy` keeps the big bounce on request.
fn spring_alias(name: &str) -> Option<&'static str> {
    match name {
        "bouncy" | "spring.bouncy" => Some("spring(300, 22)"),
        "springy" => Some("spring(300, 10)"),
        _ => None,
    }
}

fn empty_node(kind: Kind) -> Node {
    Node {
        kind,
        start: 0.0,
        end: f32::INFINITY,
        base: prop::DEFAULTS,
        exprs: Vec::new(),
        cexprs: Vec::new(),
        update: None,
        tracks: Vec::new(),
        loops: Vec::new(),
        anchor: [0.5, 0.5],
        blend: Blend::Normal,
        shadow: None,
        glow: None,
        treat: Vec::new(),
        treat_slot: 0,
        live_pattern: None,
        ink_follow: false,
        brush: None,
        step: 0.0,
        boil: None,
        clip: false,
        clip_path: None,
        wipe_dir: 0,
        index: 0.0,
        count: 1.0,
        tag: u32::MAX,
        id: None,
        fx: None,
        matte: None,
        inertia: None,
        posterize_time: 0.0,
        drag: 0.0,
    }
}

impl C<'_> {
    // ---------- units ----------

    /// Seconds from a time value (no marker origin adjustment).
    /// `"2 beats"`, `"1 bar"`, `"4b"` → seconds at the music's tempo (`None` if not musical).
    fn musical(&mut self, s: &str, path: &str) -> Option<f32> {
        let low = s.to_ascii_lowercase();
        let (num, unit) = ["beats", "beat", "bars", "bar", "b"].iter().find_map(|u| low.strip_suffix(u).map(|n| (n.trim().to_string(), *u)))?;
        let k: f32 = if num.is_empty() { 1.0 } else { num.parse().ok()? };
        if self.groove.is_none() {
            self.load_groove(false, path);
        }
        let m = self.groove.as_ref()?.as_ref().ok()?;
        let (beat, bar) = m.lengths();
        Some(k * if unit.starts_with("bar") { bar } else { beat } as f32)
    }

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
                // Musical lengths: "2 beats", "1 bar", "4b", "0.5bar" (from the music's tempo).
                if let Some(t) = self.musical(s, path) {
                    return (t, None);
                }
                // `"=1.2 + 0.1 * 3"`: an expression, worked out once (a time can't change per frame).
                if let Some(e) = s.strip_prefix('=') {
                    return match Expr::compile(e, 1.0, self.w as f64, self.h as f64) {
                        Ok(x) if !x.dynamic => (x.eval(&super::expr::Vars { w: self.w as f64, h: self.h as f64, fps: self.fps as f64, ..Default::default() }) as f32, None),
                        Ok(x) => match x.constant() {
                            Some(v) => (v as f32, None),
                            None if x.music => {
                                self.d.push(Diagnostic::error(path, format!("'{s}': a time is fixed when the spec loads, so it can't read beat/bar/level; use a marker (\"@beat17\", \"@bar5+0.25\") or a musical length (\"2 beats\")")));
                                (0.0, None)
                            }
                            None => {
                                self.d.push(Diagnostic::error(path, format!("'{s}': a time is fixed when the spec loads, so it can't use t/gt/p/frame (w, h, fps and maths are fine, e.g. \"=12 / fps\")")));
                                (0.0, None)
                            }
                        },
                        Err(m) => {
                            self.d.push(Diagnostic::error(path, format!("time expression '{s}': {m}")));
                            (0.0, None)
                        }
                    };
                }
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
                    } else if let Some(r) = self.beat_marks.resolve(name) {
                        match r {
                            Ok(v) => v - self.scene_start,
                            Err(e) => {
                                self.d.push(Diagnostic::error(path, e));
                                return (0.0, None);
                            }
                        }
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
                        self.d.push(Diagnostic::error(path, format!("invalid time '{s}' (use seconds 1.5, \"1.5s\", \"500ms\", frames \"12f\", a marker \"@intro\" / \"@beat17\" / \"@bar5+0.25\", music \"2 beats\", or arithmetic \"=12 / fps\")")));
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
                    let Some(e) = &self.sibling_refs(e, path) else { return V::C(0.0) };
                    match Expr::compile(e, refl as f64, self.w as f64, self.h as f64) {
                        Ok(x) if x.music && (self.groove.is_none() || (x.level && !self.groove_level)) => {
                            self.load_groove(x.level, path);
                            V::E(x)
                        }
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
                                format!("invalid value '{s}' (use a number like 960, \"50%\" of the parent, \"10vw\" / \"8vh\" / \"5vmin\", \"24px\", \"1.5em\", or an expression like \"=w / 2 + 100 * sin(t)\")"),
                            ));
                            V::C(0.0)
                        }
                    }
                }
            }
        }
    }

    /// Bind the generated music's notes before the scenes compile (`onNotes`). Cuts and the
    /// video length come from the durations as written; the cue is pinned so the render
    /// plays exactly this arrangement.
    fn bind_score(&mut self, spec: &Spec, default_dur: f32) {
        let Some(v) = &spec.music else {
            self.score = Some(Err("'onNotes' places layers on the notes of generated `music`; add a top-level \"music\" (e.g. \"music\": \"house\")".into()));
            return;
        };
        let nd = self.d.len();
        let mut t = 0.0f64;
        let mut cuts = Vec::new();
        for (i, s) in spec.scenes.iter().enumerate() {
            if i > 0 {
                cuts.push(t);
            }
            t += match &s.duration {
                Some(Num::Str(x)) if x == "auto" => default_dur,
                Some(d) => self.time_raw(d, "duration"),
                None => default_dur,
            } as f64;
        }
        if let Some(d) = &spec.duration {
            t = self.time_raw(d, "duration") as f64;
        }
        self.d.truncate(nd);
        let voice = spec.audio.iter().flatten().chain(spec.scenes.iter().flat_map(|s| s.audio.iter().flatten())).any(|a| a.voice == Some(true));
        self.score = Some((|| {
            let mut cue = crate::music::parse_with_cuts(v, &self.o.base_dir, &cuts)?;
            cue.pinned = Some((t, voice));
            self.beat_marks.pin(t);
            let notes = crate::music::score(&cue, t, voice)?;
            let bpb = anikit_music::beat_grid(&cue.spec)?.beats_per_bar;
            self.pinned_music = Some(Arc::new(cue));
            Ok((Arc::new(notes), bpb))
        })());
    }

    /// `{ "onNotes": … }`: one layer per matching note inside this scene.
    fn note_layers(&mut self, on: &spec::OnNotes, path: &str, pc: PCtx) -> Vec<Node> {
        let path = format!("{path}.onNotes");
        let (notes, bpb) = match &self.score {
            Some(Ok((n, b))) => (n.clone(), *b),
            Some(Err(e)) => {
                self.d.push(Diagnostic::error(&path, e.clone()));
                return Vec::new();
            }
            None => {
                self.d.push(Diagnostic::error(&path, "'onNotes' works in a scene's layers (and its groups)"));
                return Vec::new();
            }
        };
        if let Some(tr) = &on.track {
            if !notes.iter().any(|n| &n.track == tr || &n.role == tr) {
                let mut names: Vec<&str> = notes.iter().flat_map(|n| [n.track.as_str(), n.role.as_str()]).collect();
                names.sort_unstable();
                names.dedup();
                self.d.push(Diagnostic::error(format!("{path}.track"), format!("the music has no track '{tr}' (tracks and roles: {})", names.join(", "))).suggest(suggest(tr, &names)));
                return Vec::new();
            }
        }
        let mut keys = Vec::new();
        for (k, v) in on.keys.iter().flatten().enumerate() {
            match v {
                serde_json::Value::Number(n) => keys.push(n.as_f64().unwrap_or(0.0) as f32),
                serde_json::Value::String(s) => match anikit_music::notes::drum_key(s) {
                    Some(k) => keys.push(k as f32),
                    None => self.d.push(Diagnostic::error(format!("{path}.keys[{k}]"), format!("unknown drum '{s}' (kick snare hat ohh clap rim tom crash ride shaker …, or a MIDI number)"))),
                },
                _ => self.d.push(Diagnostic::error(format!("{path}.keys[{k}]"), "a MIDI number or a drum name")),
            }
        }
        let (lo, hi) = (self.scene_start + pc.t0, self.scene_start + pc.end.min(self.scene_dur));
        let every = on.every.unwrap_or(1).max(1) as usize;
        let sel: Vec<&crate::music::ScoreNote> = notes
            .iter()
            .filter(|n| on.track.as_ref().is_none_or(|tr| &n.track == tr || &n.role == tr))
            .filter(|n| keys.is_empty() || keys.iter().any(|k| (n.key - k).abs() < 0.5))
            .filter(|n| n.t >= lo as f64 - 1e-6 && n.t < hi as f64)
            .step_by(every)
            .collect();
        let (kmin, kmax) = sel.iter().fold((f32::MAX, f32::MIN), |(a, b), n| (a.min(n.key), b.max(n.key)));
        let mut out = Vec::with_capacity(sel.len());
        let nd = self.d.len();
        for (i, n) in sel.iter().enumerate() {
            let mut params = serde_json::Map::new();
            let num = |x: f64| serde_json::Number::from_f64((x * 1e6).round() / 1e6).map_or(serde_json::Value::Null, serde_json::Value::Number);
            params.insert("t".into(), num(n.t));
            params.insert("dur".into(), num(n.dur));
            params.insert("key".into(), num(n.key as f64));
            params.insert("pitch".into(), num(if kmax > kmin { ((n.key - kmin) / (kmax - kmin)) as f64 } else { 0.5 }));
            params.insert("vel".into(), num(n.vel as f64));
            params.insert("beat".into(), num(n.beat));
            params.insert("bar".into(), num(n.beat / bpb.max(1.0)));
            params.insert("i".into(), i.into());
            params.insert("n".into(), sel.len().into());
            let mut v = crate::spec::reuse::subst(on.layer.clone(), &params);
            let Some(o) = v.as_object_mut() else {
                self.d.push(Diagnostic::error(format!("{path}.layer"), "must be a layer object"));
                return out;
            };
            let rel = n.t as f32 - lo;
            let off = match o.remove("start").or_else(|| o.remove("delay")) {
                None => 0.0,
                Some(serde_json::Value::Number(x)) => x.as_f64().unwrap_or(0.0) as f32,
                Some(_) => {
                    if i == 0 {
                        self.d.push(Diagnostic::warning(format!("{path}.layer.start"), "in onNotes, 'start' is seconds from the note (a number, e.g. -0.05 to anticipate)"));
                    }
                    0.0
                }
            };
            o.insert("start".into(), num((rel + off) as f64));
            if !["duration", "end", "frames"].iter().any(|k| o.contains_key(*k)) {
                o.insert("duration".into(), num((n.dur as f32).max(1.0 / self.fps) as f64));
            }
            let mut unknown = Vec::new();
            let layer: Layer = match serde_ignored::deserialize(v, |p| unknown.push(p.to_string())) {
                Ok(l) => l,
                Err(e) => {
                    self.d.push(Diagnostic::error(format!("{path}.layer"), e.to_string()));
                    return out;
                }
            };
            if i == 0 {
                for u in unknown {
                    self.d.push(Diagnostic::warning(format!("{path}.layer.{u}"), format!("unknown key '{u}' (ignored)")));
                }
            }
            let cpc = PCtx { index: i, count: sel.len(), delay: 0.0, ..pc };
            let nd_i = self.d.len();
            if let Some(node) = self.layer(&layer, &format!("{path}.layer"), cpc) {
                out.push(node);
            }
            // The template's problems are reported once, not once per note.
            if i > 0 {
                self.d.truncate(nd_i.max(nd));
            }
        }
        out
    }

    /// Replace `id.left|right|top|bottom|cx|cy|width|height` (an earlier sibling's resting box) with its value.
    fn sibling_refs(&mut self, e: &str, path: &str) -> Option<String> {
        const PROPS: [&str; 8] = ["left", "right", "top", "bottom", "cx", "cy", "width", "height"];
        if !e.contains('.') {
            return Some(e.to_string());
        }
        let b = e.as_bytes();
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-';
        let mut out = String::with_capacity(e.len());
        let mut i = 0;
        while i < b.len() {
            let starts = (b[i].is_ascii_alphabetic() || b[i] == b'_') && (i == 0 || !(word(b[i - 1]) || b[i - 1] == b'.'));
            if starts {
                // The longest earlier sibling id followed by `.prop`.
                let hit = self
                    .sib
                    .iter()
                    .filter_map(|(id, bx)| {
                        let rest = e[i..].strip_prefix(id.as_str())?.strip_prefix('.')?;
                        let p = PROPS.iter().find(|p| rest.starts_with(**p) && !rest.as_bytes().get(p.len()).is_some_and(|c| word(*c)))?;
                        Some((id.len() + 1 + p.len(), *p, *bx))
                    })
                    .max_by_key(|h| h.0);
                if let Some((len, p, [l, t, r, bt])) = hit {
                    let v = match p {
                        "left" => l,
                        "right" => r,
                        "top" => t,
                        "bottom" => bt,
                        "cx" => (l + r) * 0.5,
                        "cy" => (t + bt) * 0.5,
                        "width" => r - l,
                        _ => bt - t,
                    };
                    out.push_str(&format!("({v})"));
                    i += len;
                    continue;
                }
                // `name.top` with no such sibling: say so, with the ids in scope.
                let mut j = i;
                while j < b.len() && (word(b[j])) {
                    j += 1;
                }
                if b.get(j) == Some(&b'.') && PROPS.iter().any(|p| e[j + 1..].starts_with(p)) {
                    let name = &e[i..j];
                    let ids: Vec<&str> = self.sib.iter().map(|s| s.0.as_str()).collect();
                    let known = if ids.is_empty() { "none with an id yet".to_string() } else { ids.join(", ") };
                    let prop = e[j + 1..].split(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or("");
                    self.d.push(
                        Diagnostic::error(path, format!("'{name}.{prop}': no earlier sibling layer with id '{name}' (a layer can refer to siblings above it in the list: {known})"))
                            .suggest(suggest(name, &ids)),
                    );
                    return None;
                }
            }
            let ch = e[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
        Some(out)
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
        // Named curves from studio and design-system practice (docs/video/craft/motion-design.md).
        let named = match name {
            "glide" => Some("cubic-bezier(0.16, 1, 0.3, 1)"),
            "snap" => Some("cubic-bezier(0.87, 0, 0.13, 1)"),
            "land" => Some("cubic-bezier(0.25, 0.1, 0.1, 1)"),
            "launch" => Some("cubic-bezier(0.7, 0, 0.84, 0)"),
            "material" | "standard" => Some("cubic-bezier(0.2, 0, 0, 1)"),
            "emphasizedDecelerate" => Some("cubic-bezier(0.05, 0.7, 0.1, 1)"),
            "emphasizedAccelerate" => Some("cubic-bezier(0.3, 0, 0.8, 0.15)"),
            "productive" => Some("cubic-bezier(0.2, 0, 0.38, 0.9)"),
            "expressive" => Some("cubic-bezier(0.4, 0.14, 0.3, 1)"),
            "expressiveEntrance" => Some("cubic-bezier(0, 0, 0.3, 1)"),
            "expressiveExit" => Some("cubic-bezier(0.4, 0.14, 1, 1)"),
            _ => None,
        };
        match Easing::parse(named.or(spring_alias(name)).unwrap_or(name)) {
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
            spec::SpringSpec::Name(n) if spring_alias(n).is_some() => Easing::parse(spring_alias(n).unwrap()),
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

    /// A colour function written without `=`: `oklch(0.7, 0.15, 30)`, CSS's `oklch(70% 0.15 30 / 0.5)`, `hsl(…)`, `mix(…)`.
    fn color_fn(&self, s: &str) -> Option<Rgba> {
        let s = s.trim();
        let open = s.find('(')?;
        if !s.ends_with(')') {
            return None;
        }
        let inner = &s[open + 1..s.len() - 1];
        let args = if inner.contains(',') {
            inner.to_string()
        } else {
            // CSS level 4: spaces between channels, `/ alpha`, percentages.
            inner.replace('/', " ").split_whitespace().map(|a| a.trim_end_matches('%').to_string()).collect::<Vec<_>>().join(", ")
        };
        let src = format!("{}({args})", &s[..open]);
        let theme = &self.theme;
        let e = super::expr::ColorExpr::compile(&src, &mut |x: &str| theme.color(x).map(|c| c.0)).ok()?;
        Some(Rgba(e.eval(&super::expr::Vars::default())))
    }

    fn color(&mut self, s: &str, path: &str) -> Option<Rgba> {
        match self.theme.color(s) {
            Some(c) => Some(c),
            None => {
                if let Some(t) = s.trim().strip_prefix('$') {
                    let names: Vec<&String> = self.theme.colors.keys().collect();
                    self.d.push(Diagnostic::error(path, format!("unknown colour token '${t}'")).suggest(suggest(t, &names).map(|x| format!("${x}"))));
                } else if let Some(c) = self.color_fn(s) {
                    return Some(c);
                } else {
                    self.d.push(Diagnostic::error(path, format!("invalid colour '{s}' ({COLOUR_FORMS})")));
                }
                None
            }
        }
    }

    /// Compile a paint. Returns the paint and its solid colour (for the colour slots).
    fn paint(&mut self, p: &spec::Paint, path: &str) -> Option<(PaintC, Rgba)> {
        self.last_cexpr = None;
        match p {
            spec::Paint::Css(s) if s.trim().starts_with('=') => {
                let src = s.trim()[1..].to_string();
                let mut resolve = |c: &str| self.theme.color(c).map(|x| x.0);
                match super::expr::ColorExpr::compile(&src, &mut resolve) {
                    Ok(ce) => {
                        if ce.music && (self.groove.is_none() || (ce.level && !self.groove_level)) {
                            self.load_groove(ce.level, path);
                        }
                        let c = Rgba(ce.eval(&super::expr::Vars { w: self.w as f64, h: self.h as f64, fps: self.fps as f64, ..Default::default() }));
                        self.last_cexpr = Some(ce);
                        Some((PaintC::Solid, c))
                    }
                    Err(m) => {
                        self.d.push(Diagnostic::error(path, format!("colour expression: {m}")));
                        None
                    }
                }
            }
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
        // No transition given: a hard cut (transitions are opt-in). An object without a
        // `type` (`{ "duration": 0.6 }`) is a crossfade.
        let o = t.map(|t| t.0.clone()).unwrap_or_default();
        let kind_s = o.kind.as_deref().unwrap_or(if t.is_none() { "cut" } else { "crossfade" });
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
            "shader" => TransKind::Shader,
            "zoomThrough" => TransKind::ZoomThrough,
            "reveal" => TransKind::Reveal,
            "blinds" => TransKind::Blinds,
            "split" => TransKind::Split,
            "whip" => TransKind::Whip,
            k if anikit_shader::library::get(k).is_some_and(|e| e.use_ == anikit_shader::library::Use::Transition) => TransKind::Shader,
            k => {
                self.d.push(Diagnostic::error(format!("{path}.type"), format!("unknown transition '{k}'")).suggest(suggest(k, val::TRANSITIONS)));
                TransKind::Crossfade
            }
        };
        let def = match kind {
            TransKind::Cut => 0.0,
            TransKind::Glitch => 0.3,
            TransKind::MorphCut => 0.25,
            TransKind::Whip => 0.35,
            TransKind::ZoomThrough => 0.8,
            _ => 0.5,
        };
        let dur = match &o.duration {
            Some(d) if kind != TransKind::Cut => self.time_raw(d, &format!("{path}.duration")).max(0.0),
            _ => def,
        };
        let def_ease = match kind {
            TransKind::ZoomThrough => "inQuad",
            TransKind::Reveal | TransKind::Blinds | TransKind::Split | TransKind::Whip => "inOutExpo",
            _ => "inOutCubic",
        };
        let ease = self.ease(o.ease.as_deref(), def_ease, &format!("{path}.ease"));
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
        let shader = if kind == TransKind::Shader {
            let src = o.shader.clone().unwrap_or_else(|| kind_s.to_string());
            self.shader_transition(&src, o.uniforms.as_ref(), path)
        } else {
            None
        };
        let kind = if kind == TransKind::Shader && shader.is_none() { TransKind::Crossfade } else { kind };
        let shape = match o.shape.as_deref() {
            None | Some("circle") => 0,
            Some("diamond") => 1,
            Some("square") => 2,
            Some("star") => 3,
            Some("blob") => 4,
            Some(x) => {
                let names = ["circle", "diamond", "square", "star", "blob"];
                self.d.push(Diagnostic::error(format!("{path}.shape"), format!("unknown reveal shape '{x}'")).suggest(suggest(x, &names)));
                0
            }
        };
        if kind == TransKind::ZoomThrough && o.layer.is_none() {
            self.d.push(Diagnostic::error(format!("{path}.layer"), "zoomThrough needs 'layer': the id of a layer in the previous scene to fly into"));
        }
        let carry = o.carry.clone().unwrap_or_default();
        if !carry.is_empty() && kind == TransKind::Cut {
            self.d.push(Diagnostic::warning(format!("{path}.carry"), "a cut has no duration to carry layers across; give the transition a type and duration"));
        }
        TransC { kind, dur, ease, dir, color, origin, amount, shader, layer: o.layer.clone(), shape, count: o.count.unwrap_or(8).clamp(1, 64), carry }
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
            self.set(&mut camera, prop::FOCUS, cam.focus.as_ref(), &format!("{cp}.focus"), 1.0);
            self.set(&mut camera, prop::APERTURE, cam.aperture.as_ref(), &format!("{cp}.aperture"), 1.0);
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
        self.grid = s.grid.clone().or_else(|| self.video_grid.clone());
        let (overlay, under) = match self.grid.clone() {
            Some(g) => match grid_show(&g) {
                0 => (None, false),
                m => (Some(self.grid_overlay(&g, [self.w, self.h], m == 2, &format!("{path}.grid"))), m == 2),
            },
            None => (None, false),
        };
        let mut layers = Vec::new();
        if let (Some(o), true) = (&overlay, under) {
            layers.extend(self.layers(o, &format!("{path}.grid"), pc, None));
        }
        layers.extend(self.layers(&s.layers, &format!("{path}.layers"), pc, None));
        if let (Some(o), false) = (&overlay, under) {
            layers.extend(self.layers(o, &format!("{path}.grid"), pc, None));
        }
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
        let mut at: Vec<Option<usize>> = Vec::with_capacity(ls.len());
        let outer_sib = std::mem::take(&mut self.sib);
        for (i, l) in ls.iter().enumerate() {
            let p = format!("{path}[{i}]");
            let cpc = PCtx { index: i, count: ls.len(), delay: delays[i], ..pc };
            at.push(None);
            if let Some(on) = &l.on_notes {
                out.extend(self.note_layers(on, &p, cpc));
                continue;
            }
            if let Some(n) = self.layer(l, &p, cpc) {
                if let Some(id) = &l.id {
                    let rest = |k: usize| {
                        n.exprs.iter().find(|e| e.0 as usize == k).map_or(n.base[k], |e| {
                            e.1.eval(&super::expr::Vars { w: self.w as f64, h: self.h as f64, fps: self.fps as f64, ..Default::default() }) as f32
                        })
                    };
                    let (x, y, w, h) = (rest(prop::X), rest(prop::Y), rest(prop::W), rest(prop::H));
                    let (l0, t0) = (x - n.anchor[0] * w, y - n.anchor[1] * h);
                    self.sib.push((id.clone(), [l0, t0, l0 + w, t0 + h]));
                }
                at[i] = Some(out.len());
                out.push(n);
            }
        }
        self.sib = outer_sib;
        if ls.iter().any(|l| l.matte.is_some()) {
            self.resolve_mattes(ls, path, &at, &mut out);
        }
        out
    }

    /// Track mattes: move each matte layer (by `id`, a sibling) into the layer it mattes.
    fn resolve_mattes(&mut self, ls: &[Layer], path: &str, at: &[Option<usize>], out: &mut Vec<Node>) {
        let mut pairs: Vec<(usize, usize, u8)> = Vec::new();
        for (i, l) in ls.iter().enumerate() {
            let Some(m) = &l.matte else { continue };
            let m = &m.0;
            let p = format!("{path}[{i}].matte");
            let mode = match m.mode.as_deref().unwrap_or("alpha") {
                "alpha" => 0,
                "alphaInverted" => 1,
                "luma" => 2,
                "lumaInverted" => 3,
                x => {
                    let names = ["alpha", "alphaInverted", "luma", "lumaInverted"];
                    self.d.push(Diagnostic::error(format!("{p}.mode"), format!("unknown matte mode '{x}'")).suggest(suggest(x, &names)));
                    0
                }
            };
            let Some(j) = ls.iter().position(|o| o.id.as_deref() == Some(m.layer.as_str())) else {
                let ids: Vec<&str> = ls.iter().filter_map(|o| o.id.as_deref()).collect();
                self.d.push(Diagnostic::error(format!("{p}.layer"), format!("no sibling layer with id '{}'", m.layer)).suggest(suggest(&m.layer, &ids)));
                continue;
            };
            if j == i {
                self.d.push(Diagnostic::error(format!("{p}.layer"), "a layer can't be its own matte"));
                continue;
            }
            if let (Some(a), Some(b)) = (at[i], at[j]) {
                pairs.push((a, b, mode));
            }
        }
        // Take the matte nodes out (highest index first so indices stay valid).
        let mut mattes: Vec<usize> = pairs.iter().map(|p| p.1).collect();
        mattes.sort_unstable();
        mattes.dedup();
        let mut taken: Vec<(usize, Node)> = Vec::new();
        for &k in mattes.iter().rev() {
            if pairs.iter().any(|p| p.0 == k) {
                continue; // a matted layer used as a matte: keep it simple, leave it drawn
            }
            taken.push((k, out.remove(k)));
        }
        let removed: Vec<usize> = taken.iter().map(|t| t.0).collect();
        let shift = |k: usize| k - removed.iter().filter(|&&m| m < k).count();
        for (a, b, mode) in pairs {
            let Some(pos) = taken.iter().position(|t| t.0 == b) else {
                self.d.push(Diagnostic::warning(format!("{path}[{a}].matte"), "each matte layer can matte one layer (and not be matted itself); ignored"));
                continue;
            };
            let node = taken.remove(pos).1;
            out[shift(a)].matte = Some(Box::new((node, mode)));
        }
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
        } else if l.shape.is_some() || l.pattern.is_some() {
            "shape"
        } else if l.component.is_some() {
            "component"
        } else if l.composition.is_some() {
            "composition"
        } else if l.draw.is_some() {
            "draw"
        } else if l.dots.is_some() {
            "dots"
        } else if l.shader.is_some() || l.src.as_deref().is_some_and(|s| s.ends_with(".glsl") || s.ends_with(".wgsl") || s.ends_with(".frag")) {
            "shader"
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
            ("maxWidth", l.max_width.is_some()),
            ("axes", l.axes.is_some()),
            ("features", l.features.is_some()),
        ];
        let shape_only = [("shape", l.shape.is_some()), ("points", l.points.is_some()), ("d", l.d.is_some()), ("sides", l.sides.is_some())];
        let img_only = [("src", l.src.is_some() && kind != "shader"), ("fit", l.fit.is_some() && kind != "text")];
        let shader_only = [
            ("uniforms", l.uniforms.is_some()),
            ("channels", l.channels.is_some()),
            ("filter", l.filter.is_some()),
            ("supersample", l.supersample.is_some()),
            ("alpha", l.alpha.is_some()),
            ("buffers", l.buffers.is_some()),
        ];
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
        check(&mut self.d, &shader_only, kind == "shader", "shader");
        check(&mut self.d, &img_only[..1], kind == "image" || kind == "video", "image/video");
        check(&mut self.d, &img_only[1..], kind == "image" || kind == "video" || kind == "composition", "image/video/composition");
        if l.layers.is_some() && kind != "group" {
            self.d.push(Diagnostic::warning(format!("{path}.layers"), format!("'layers' only applies to group layers (a box uses 'children'); this is a {kind} layer")));
        }
        if l.children.is_some() && kind != "box" {
            self.d.push(Diagnostic::warning(format!("{path}.children"), format!("'children' only applies to box layers (a group uses 'layers'); this is a {kind} layer")));
        }
    }

    fn layer(&mut self, l: &Layer, path: &str, pc: PCtx) -> Option<Node> {
        // `"shader": "scene.glsl"` means the shader's source file.
        let shader_file;
        let l = match &l.shader {
            Some(f) if l.src.is_none() && [".glsl", ".wgsl", ".frag"].iter().any(|e| f.trim().to_ascii_lowercase().ends_with(e)) => {
                shader_file = Layer { src: Some(f.trim().to_string()), shader: None, ..l.clone() };
                &shader_file
            }
            _ => l,
        };
        // `"text": "=expr"`: a number computed per frame (an animatable counter).
        let expr_text;
        let l = match &l.text {
            Some(t) if t.trim_start().starts_with('=') && l.value.is_none() => {
                expr_text = Layer { text: Some("{value}".into()), value: Some(Num::Str(t.trim().to_string())), ..l.clone() };
                &expr_text
            }
            _ => l,
        };
        // `"animate": "rise"` (a lone preset) means `"enter": "rise"`.
        let norm;
        let l = match &l.animate {
            Some(a) if l.enter.is_none() && a.len() == 1 && a[0].preset.is_some() && a[0].at.is_none() && a[0].from.is_none() && a[0].to.is_none() && a[0].keyframes.is_none() => {
                let t = &a[0];
                norm = Layer {
                    enter: Some(spec::Preset(PresetObj { preset: t.preset.clone(), duration: t.duration.clone(), ease: t.ease.clone(), spring: t.spring.clone(), ..Default::default() })),
                    animate: None,
                    ..l.clone()
                };
                &norm
            }
            _ => l,
        };
        let celled;
        let l = match &l.cell {
            Some(c) => {
                celled = self.place_in_cell(l, c, path, pc.size);
                &celled
            }
            None => l,
        };
        let cut;
        let l = match &l.cutout {
            Some(c) => match self.cutout_group(l, c, path) {
                Some(g) => {
                    cut = g;
                    &cut
                }
                None => l,
            },
            None => l,
        };
        let kind = self.infer_kind(l, path)?;
        self.uniform_slots.clear();
        let label = match (kind, &l.component) {
            ("component", Some(c)) => format!("component:{c}"),
            _ => kind.to_string(),
        };
        let tag = self.layer_info.len() as u32;
        self.layer_info.push((path.to_string(), label));
        if let Some(v) = &l.sfx {
            self.layer_sfx.push((tag, v.clone(), path.to_string()));
        }
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
        let start = match &l.frames {
            Some(f) => {
                if l.start.is_some() || l.end.is_some() || l.duration.is_some() {
                    self.d.push(Diagnostic::warning(format!("{path}.frames"), "'frames' replaces start/end/duration; drop one of them"));
                }
                let [a, b] = match f {
                    spec::Frames::One(a) => [*a, a + 1.0],
                    spec::Frames::Range(r) => *r,
                };
                if b <= a {
                    self.d.push(Diagnostic::error(format!("{path}.frames"), format!("empty range [{a}, {b}): the end frame is excluded, so use [{a}, {}] for one frame", a + 1.0)));
                }
                end = pc.t0 + (b / self.fps as f64) as f32;
                pc.t0 + (a / self.fps as f64) as f32
            }
            None => start,
        };
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
        n.tag = tag;
        n.count = pc.count as f32;
        n.id = l.id.as_deref().map(Into::into);

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
                let life = if end.is_finite() && end < 1e5 { end - start } else { (self.scene_dur - start).clamp(0.0, 600.0) };
                self.video_layer(l, path, given_w, given_h, life, start)
            }
            "group" => {
                let size = [given_w.unwrap_or(pw), given_h.unwrap_or(ph)];
                let cpc = PCtx { size, t0: start, end, delay: 0.0, index: 0, count: 0, in_box: false };
                let saved = l.grid.as_ref().map(|g| self.grid.replace(g.clone()));
                let mut kids = self.layers(l.layers.as_deref().unwrap_or(&[]), &format!("{path}.layers"), cpc, l.stagger.as_ref());
                if let Some(g) = l.grid.as_ref() {
                    let m = grid_show(g);
                    if m != 0 {
                        let o = self.grid_overlay(g, size, m == 2, &format!("{path}.grid"));
                        let ok = self.layers(&o, &format!("{path}.grid"), cpc, None);
                        if m == 2 {
                            kids.splice(0..0, ok);
                        } else {
                            kids.extend(ok);
                        }
                    }
                }
                if let Some(prev) = saved {
                    self.grid = prev;
                }
                (Kind::Group { children: kids }, size)
            }
            "box" => self.box_layer(l, path, given_w, given_h, start, end, pc, &mut n),
            "shader" => self.shader_layer(l, path, given_w, given_h, start, end, &mut n)?,
            "composition" => self.comp_layer(l, path)?,
            "draw" => (self.draw_layer(l, path), [given_w.unwrap_or(pw), given_h.unwrap_or(ph)]),
            "dots" => (self.dots_layer(l, path), [given_w.unwrap_or(pw), given_h.unwrap_or(ph)]),
            _ => self.component_layer(l, path, given_w, given_h)?,
        };
        n.kind = kind_v;
        if let Some(code) = &l.update {
            match crate::spec::script::compile_update(code, path) {
                Ok(ast) => {
                    let src = code.to_string();
                    let reads = |w: &str| src.split(|c: char| !c.is_ascii_alphanumeric()).any(|x| x == w);
                    if ["beat", "bar", "level", "bpm"].iter().any(|w| reads(w)) && (self.groove.is_none() || (reads("level") && !self.groove_level)) {
                        self.load_groove(reads("level"), &format!("{path}.update"));
                    }
                    // A trial run at t = 0 reports runtime errors and unknown props now, not silently per frame.
                    let mut vals = [0.0f64; 14];
                    (vals[6], vals[7], vals[8]) = (self.w as f64, self.h as f64, self.fps as f64);
                    match crate::spec::script::run_update(&ast, &vals) {
                        Ok(m) => {
                            for k in m.keys() {
                                if prop::lookup(k.as_str()).is_none() {
                                    let names = ["x", "y", "width", "height", "scale", "scaleX", "scaleY", "rotation", "skewX", "skewY", "opacity", "blur", "radius", "strokeWidth", "trimStart", "trimEnd", "reveal", "value", "size", "letterSpacing", "progress", "zoom", "wipe", "depth", "fill", "color", "stroke"];
                                    self.d.push(Diagnostic::error(format!("{path}.update"), format!("update: '{k}' isn't an animatable property")).suggest(suggest(k.as_str(), &names)));
                                }
                            }
                        }
                        Err(e) => self.d.push(Diagnostic::error(format!("{path}.update"), format!("update: {e} (it must return a map like #{{ x: 100 }})"))),
                    }
                    n.update = Some(Arc::new(ast));
                }
                Err(d) => self.d.push(d),
            }
        }
        n.base[prop::W] = given_w.unwrap_or(nat[0]);
        n.base[prop::H] = given_h.unwrap_or(nat[1]);
        let natural = match &n.kind {
            Kind::Image { natural, .. } | Kind::Video { natural, .. } | Kind::Comp { natural, .. } => Some(*natural),
            _ => None,
        };
        if let Some(nat) = natural {
            // One side given: keep the aspect ratio.
            match (given_w, given_h) {
                (Some(w), None) => n.base[prop::H] = w * nat[1] / nat[0].max(1.0),
                (None, Some(h)) => n.base[prop::W] = h * nat[0] / nat[1].max(1.0),
                _ => {}
            }
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

        if l.text_box_trim == Some(true) {
            if let Kind::Text { layout, .. } = &n.kind {
                // Re-anchor so the anchor's y refers to the cap-top…baseline box.
                let h = n.base[prop::H].max(1e-3);
                let (top, bot) = (layout.cap_top, layout.last_baseline);
                if bot > top {
                    n.anchor[1] = (top + n.anchor[1] * (bot - top)) / h;
                }
            }
        }
        if l.cell.is_none() && !pc.in_box {
            self.near_grid_check(l, path, &n, pc.size);
        }

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
        self.set(&mut n, prop::DEPTH, l.depth.as_ref(), &format!("{path}.depth"), 1.0);
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
        self.treatments(l, path, &mut n);
        if let Some(st) = &l.step {
            let sp = format!("{path}.step");
            n.step = match st {
                serde_json::Value::Number(x) => x.as_f64().unwrap_or(0.0) as f32,
                serde_json::Value::String(x) => match x.as_str() {
                    "ones" => self.fps,
                    "twos" => self.fps / 2.0,
                    "threes" => self.fps / 3.0,
                    "fours" => self.fps / 4.0,
                    _ => {
                        self.d.push(Diagnostic::error(&sp, format!("unknown step '{x}'")).suggest(suggest(x, &["twos", "threes", "fours"])));
                        0.0
                    }
                },
                _ => {
                    self.d.push(Diagnostic::error(&sp, "expected fps (e.g. 12) or \"twos\" / \"threes\""));
                    0.0
                }
            };
            if n.step < 0.0 || n.step > self.fps * 1.001 {
                self.d.push(Diagnostic::warning(&sp, format!("step {} fps is outside 0..{} (the video's fps); ignored", n.step, self.fps)));
                n.step = 0.0;
            }
        }
        if let Some(b) = &l.boil {
            let (amt, rot, fps, seed) = match b {
                spec::BoilValue::Px(px) => (*px, 0.0, 12.0, 1),
                spec::BoilValue::Full(o) => (o.amount.unwrap_or(2.0), o.rotate.unwrap_or(0.0), o.fps.unwrap_or(12.0), o.seed.unwrap_or(1)),
            };
            if amt > 0.0 || rot > 0.0 {
                n.boil = Some([amt as f32, rot as f32, (fps as f32).clamp(0.5, self.fps), seed as f32]);
            }
        }
        if matches!(n.kind, Kind::Image { .. } | Kind::Video { .. }) {
            if let Some(r) = &l.radius {
                let refl = n.base[prop::W].min(n.base[prop::H]);
                self.set(&mut n, prop::RADIUS, Some(r), &format!("{path}.radius"), refl);
                n.clip = true;
            }
        }
        match &l.clip {
            Some(spec::Clip::On(b)) => n.clip = *b,
            Some(spec::Clip::Shape(c)) => {
                n.clip_path = self.clip_shape(c, &format!("{path}.clip"), [n.base[prop::W], n.base[prop::H]]);
            }
            None => {}
        }
        if let Some(m) = &l.mask {
            if m.shape.is_some() && m.kind.as_deref().is_none_or(|k| k == "shape") {
                n.clip_path = self.clip_shape(m, &format!("{path}.mask"), [n.base[prop::W], n.base[prop::H]]);
            } else {
                self.d.push(Diagnostic::warning(format!("{path}.mask"), "only shape masks are supported (as hard-edged clips); ignored"));
            }
        }

        // Animation.
        self.cur_anchor = n.anchor;
        self.cur_text = matches!(n.kind, Kind::Text { .. });
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
        if (matches!(n.kind, Kind::Component(..)) || l.path.is_some()) && l.progress.is_none() && life_end.is_finite() {
            tw.insert(0, TweenC { at: 0.0, dur: life_end.max(1e-3), ease: 0, from: vec![(prop::PROGRESS, 1, ValS::Abs(0.0))], to: vec![(prop::PROGRESS, 1, ValS::Abs(1.0))], kf: vec![], repeat: 0, yoyo: false });
        }
        self.warn_from_overrides(&tw, path);
        n.tracks = self.tracks(&n.base, &n.exprs, &mut tw, life_end);
        self.mograph(l, path, &mut n);

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

    /// Motion-graphics extras: repeater, echo, motion path, text path, shape modifiers.
    fn mograph(&mut self, l: &Layer, path: &str, n: &mut Node) {
        use super::fx::{self, EchoC, Fx, MPath, Mods, RepC};
        self.set(n, prop::PATH_OFF, l.path_offset.as_ref(), &format!("{path}.pathOffset"), 1.0);
        self.set(n, prop::DASH_OFF, l.dash_offset.as_ref(), &format!("{path}.dashOffset"), 1.0);
        let mut f = Fx::default();
        if let Some(spec::RepeatValue::Repeater(r)) = &l.repeat {
            let o = r.offset.clone().unwrap_or_default();
            let sc = match &o.scale {
                None => [1.0, 1.0],
                Some(serde_json::Value::Number(x)) => [x.as_f64().unwrap_or(1.0) as f32; 2],
                Some(serde_json::Value::Array(a)) if a.len() == 2 => [a[0].as_f64().unwrap_or(1.0) as f32, a[1].as_f64().unwrap_or(1.0) as f32],
                Some(_) => {
                    self.d.push(Diagnostic::error(format!("{path}.repeat.offset.scale"), "scale is a multiplier per copy: a number or [sx, sy]"));
                    [1.0, 1.0]
                }
            };
            let count = r.count.unwrap_or(3);
            if count > 2000 {
                self.d.push(Diagnostic::warning(format!("{path}.repeat.count"), "more than 2000 copies; capped"));
            }
            f.repeat = Some(RepC {
                count: count.min(2000),
                d: [o.x.unwrap_or(0.0) as f32, o.y.unwrap_or(0.0) as f32],
                rot: o.rotate.unwrap_or(0.0) as f32,
                scale: sc,
                opacity: o.opacity.unwrap_or(0.0) as f32,
                stagger: r.stagger.unwrap_or(0.0) as f32,
                ranks: {
                    let order = r.order.as_deref().unwrap_or("forward");
                    if !["forward", "reverse", "center", "random"].contains(&order) {
                        self.d.push(Diagnostic::error(format!("{path}.repeat.order"), format!("unknown order '{order}'")).suggest(suggest(order, &["forward", "reverse", "center", "random"])));
                    }
                    fx::order_ranks(count.min(2000) as usize, order, r.seed.unwrap_or(1.0))
                },
                center: r.center.map(|c| [c[0] as f32, c[1] as f32]),
            });
        }
        if let Some(e) = &l.echo {
            let blend = match e.blend.as_deref().unwrap_or("normal") {
                "normal" => Blend::Normal,
                "add" => Blend::Add,
                "screen" => Blend::Screen,
                "lighten" => Blend::Lighten,
                "multiply" => Blend::Multiply,
                "overlay" => Blend::Overlay,
                "darken" => Blend::Darken,
                "softLight" => Blend::SoftLight,
                "difference" => Blend::Difference,
                x => {
                    self.d.push(Diagnostic::error(format!("{path}.echo.blend"), format!("unknown blend mode '{x}'")).suggest(suggest(x, val::BLENDS)));
                    Blend::Normal
                }
            };
            f.echo = Some(EchoC {
                count: e.count.unwrap_or(5).min(64),
                delay: e.delay.unwrap_or(0.05) as f32,
                decay: e.decay.unwrap_or(0.6).clamp(0.0, 1.0) as f32,
                blend,
            });
        }
        let svg = |d: &str, key: &str, me: &mut Self| -> Option<MPath> {
            match Path::from_svg(d) {
                Ok(p) => {
                    let m = MPath::from_path(&p);
                    if m.is_none() {
                        me.d.push(Diagnostic::error(format!("{path}.{key}"), "the path has no length"));
                    }
                    m
                }
                Err(e) => {
                    me.d.push(Diagnostic::error(format!("{path}.{key}"), format!("invalid SVG path: {e} (SVG path data like \"M0 0 L100 0 L50 80 Z\" or \"M10 80 C40 10 65 10 95 80\")")));
                    None
                }
            }
        };
        if let Some(d) = &l.path {
            f.path = svg(d, "path", self);
            f.orient = match &l.orient {
                None | Some(serde_json::Value::Bool(false)) => None,
                Some(serde_json::Value::Bool(true)) => Some(0.0),
                Some(serde_json::Value::Number(x)) => Some(x.as_f64().unwrap_or(0.0) as f32),
                Some(_) => {
                    self.d.push(Diagnostic::error(format!("{path}.orient"), "orient is true/false or an extra angle in degrees"));
                    None
                }
            };
        } else if l.orient.is_some() {
            self.d.push(Diagnostic::warning(format!("{path}.orient"), "'orient' needs 'path' (a motion path)"));
        }
        if let Some(d) = &l.text_path {
            if matches!(n.kind, Kind::Text { kinetic: None, split: None, .. }) {
                f.text_path = svg(d, "textPath", self);
            } else {
                self.d.push(Diagnostic::warning(format!("{path}.textPath"), "'textPath' applies to plain text layers (no split/kinetic)"));
            }
        }
        // Shape modifiers.
        let mods = Mods {
            round: l.round_corners.unwrap_or(0.0).max(0.0),
            zigzag: l.zigzag.as_ref().map(|z| (z.size.unwrap_or(10.0), z.ridges.unwrap_or(10).clamp(1, 500), z.smooth.unwrap_or(false))),
            pucker: l.pucker_bloat.unwrap_or(0.0).clamp(-2.0, 2.0),
            offset: l.offset_path.unwrap_or(0.0),
            twist: l.twist.unwrap_or(0.0),
        };
        let along: Option<Vec<Rgba>> = l.stroke.as_ref().and_then(|s| s.0.along.as_ref()).map(|cs| {
            cs.iter().enumerate().filter_map(|(i, c)| self.color(c, &format!("{path}.stroke.along[{i}]"))).collect()
        });
        let along = along.filter(|a| !a.is_empty());
        let any_mod = !mods.is_empty() || l.wiggle_path.is_some() || along.is_some();
        if any_mod {
            let (w, h) = (n.base[prop::W] as f64, n.base[prop::H] as f64);
            if let Kind::Shape { shape, path: p, .. } = &mut n.kind {
                let src = match shape {
                    ShapeKind::Rect => {
                        let r = (n.base[prop::RADIUS] as f64).min(w.min(h) * 0.5).max(0.0);
                        kurbo::Shape::to_path(&kurbo::RoundedRect::new(0.0, 0.0, w, h, r), 0.1)
                    }
                    ShapeKind::Ellipse => kurbo::Shape::to_path(&kurbo::Ellipse::new((w * 0.5, h * 0.5), (w * 0.5, h * 0.5), 0.0), 0.1),
                    ShapeKind::Path => p.clone(),
                };
                *shape = ShapeKind::Path;
                *p = mods.apply(&src);
                if let Some(cs) = &along {
                    if l.wiggle_path.is_some() {
                        self.d.push(Diagnostic::warning(format!("{path}.stroke.along"), "a gradient along the stroke ignores wigglePath"));
                    }
                    f.along = Some(fx::AlongC::build(p, cs, 96));
                }
                if let Some(wp) = &l.wiggle_path {
                    let detail = wp.detail.unwrap_or(32).clamp(3, 2000);
                    f.wiggle = Some([wp.amount.unwrap_or(10.0) as f32, detail as f32, wp.speed.unwrap_or(1.0) as f32, wp.seed.unwrap_or(1.0) as f32]);
                    f.wiggle_src = fx::resample(p, detail);
                }
            } else {
                self.d.push(Diagnostic::warning(path, "shape modifiers (roundCorners, zigzag, puckerBloat, offsetPath, twist, wigglePath) apply to shape layers"));
            }
        }
        if let Some(bv) = &l.brush {
            let o = match bv {
                spec::BrushValue::On(false) => None,
                spec::BrushValue::On(true) => Some(spec::BrushSpec::default()),
                spec::BrushValue::Full(f) => Some(f.clone()),
            };
            if let Some(o) = o {
                let (w, h) = (n.base[prop::W] as f64, n.base[prop::H] as f64);
                match &n.kind {
                    Kind::Shape { shape, path: p, stroke: Some(_), .. } => {
                        let src = match shape {
                            ShapeKind::Rect => kurbo::Shape::to_path(&kurbo::RoundedRect::new(0.0, 0.0, w, h, (n.base[prop::RADIUS] as f64).min(w.min(h) * 0.5).max(0.0)), 0.1),
                            ShapeKind::Ellipse => kurbo::Shape::to_path(&kurbo::Ellipse::new((w * 0.5, h * 0.5), (w * 0.5, h * 0.5), 0.0), 0.1),
                            ShapeKind::Path => p.clone(),
                        };
                        let t = o.taper.unwrap_or([0.12, 0.25]);
                        n.brush = super::graphic::Brush::new(&src, [t[0] as f32, t[1] as f32], o.pressure.unwrap_or(0.35) as f32, o.dry.unwrap_or(0.0) as f32, o.seed.unwrap_or(1)).map(Arc::new);
                    }
                    Kind::Shape { .. } => self.d.push(Diagnostic::warning(format!("{path}.brush"), "a brush paints the stroke: add \"stroke\" (and \"strokeWidth\")")),
                    _ => self.d.push(Diagnostic::warning(format!("{path}.brush"), "brush applies to shape layers (lines, paths, any outline)")),
                }
            }
        }
        if let Some(a) = &l.animator {
            let units = match &n.kind {
                Kind::Text { kinetic: None, split: None, layout, .. } => Some(layout.clone()),
                _ => None,
            };
            match units {
                None => self.d.push(Diagnostic::warning(format!("{path}.animator"), "'animator' applies to plain text layers (no split/kinetic)")),
                Some(layout) => {
                    let by = match a.by.as_deref().unwrap_or("char") {
                        "char" | "chars" => crate::text::UnitKind::Char,
                        "word" | "words" => crate::text::UnitKind::Word,
                        "line" | "lines" => crate::text::UnitKind::Line,
                        x => {
                            self.d.push(Diagnostic::error(format!("{path}.animator.by"), format!("unknown unit '{x}'")).suggest(suggest(x, &["char", "word", "line"])));
                            crate::text::UnitKind::Char
                        }
                    };
                    let order = a.order.as_deref().unwrap_or("forward");
                    if !["forward", "reverse", "center", "random"].contains(&order) {
                        self.d.push(Diagnostic::error(format!("{path}.animator.order"), format!("unknown order '{order}'")).suggest(suggest(order, &["forward", "reverse", "center", "random"])));
                    }
                    let color = a.color.as_ref().and_then(|c| self.color(c, &format!("{path}.animator.color"))).map(rgba_to_oklab);
                    self.set(n, prop::RANGE0, a.start.as_ref(), &format!("{path}.animator.start"), 1.0);
                    self.set(n, prop::RANGE1, a.end.as_ref(), &format!("{path}.animator.end"), 1.0);
                    self.set(n, prop::RANGE_OFF, a.offset.as_ref(), &format!("{path}.animator.offset"), 1.0);
                    let animated = n.tracks.iter().any(|t| t.slot as usize == prop::RANGE_OFF);
                    let auto = (a.offset.is_none() && !animated).then(|| {
                        let e = self.ease(a.ease.as_deref(), "outCubic", &format!("{path}.animator.ease"));
                        (a.delay.unwrap_or(0.0) as f32, a.duration.unwrap_or(1.0) as f32, e)
                    });
                    f.animator = Some(fx::AnimatorC {
                        by,
                        dx: a.x.unwrap_or(0.0) as f32,
                        dy: a.y.unwrap_or(0.0) as f32,
                        rot: a.rotation.unwrap_or(0.0) as f32,
                        scale: a.scale.unwrap_or(1.0) as f32,
                        opacity: a.opacity.unwrap_or(1.0) as f32,
                        blur: a.blur.unwrap_or(0.0) as f32,
                        color,
                        axes: a.axes.iter().flatten().filter(|(k, _)| k.len() == 4).map(|(k, v)| (k.clone(), *v as f32)).collect(),
                        align: match l.align.as_deref() {
                            Some("left" | "start") => 0.0,
                            Some("right" | "end") => 1.0,
                            None if matches!(&l.anchor, Some(spec::Anchor::Name(a)) if a.to_ascii_lowercase().contains("left")) => 0.0,
                            _ => 0.5,
                        },
                        smooth: a.smooth.unwrap_or(0.25).max(0.0) as f32,
                        order: fx::order_ranks(layout.units(by).len(), order, a.seed.unwrap_or(1.0)),
                        auto,
                    });
                }
            }
        }
        if let Some(iv) = &l.inertia {
            let p = format!("{path}.inertia");
            // (stiffness, damping, mass): damping ratio ≈ damping / (2·sqrt(stiffness·mass)).
            let preset = |name: &str| match name {
                // Overshoot on a step: soft 5%, bouncy 7%, tight 3%, heavy 2% (tasteful range); springy 37%, wobbly 18% go further.
                "soft" => Some((90.0, 13.0, 1.0)),
                "bouncy" => Some((220.0, 19.0, 1.0)),
                "tight" => Some((400.0, 30.0, 1.0)),
                "heavy" => Some((120.0, 25.0, 2.2)),
                "springy" => Some((220.0, 9.0, 1.0)),
                "wobbly" => Some((90.0, 9.0, 1.0)),
                "" | "default" => Some((170.0, 14.0, 1.0)),
                _ => None,
            };
            let mut props: Vec<String> = vec!["x".into(), "y".into(), "scaleX".into(), "scaleY".into(), "rotation".into()];
            let params = match iv {
                serde_json::Value::Bool(true) => preset(""),
                serde_json::Value::Bool(false) => None,
                serde_json::Value::String(s) => {
                    let r = preset(s);
                    if r.is_none() {
                        self.d.push(Diagnostic::error(&p, format!("unknown inertia preset '{s}'")).suggest(suggest(s, &["soft", "bouncy", "tight", "heavy", "springy", "wobbly"])));
                    }
                    r
                }
                serde_json::Value::Object(m) => {
                    for k in m.keys() {
                        if !["stiffness", "damping", "mass", "props", "preset"].contains(&k.as_str()) {
                            self.d.push(Diagnostic::warning(format!("{p}.{k}"), "unknown key (ignored)").suggest(suggest(k, &["stiffness", "damping", "mass", "props", "preset"])));
                        }
                    }
                    let (k0, c0, m0) = m.get("preset").and_then(|x| x.as_str()).and_then(preset).unwrap_or((170.0, 14.0, 1.0));
                    if let Some(serde_json::Value::Array(a)) = m.get("props") {
                        props = a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
                    }
                    let g = |k: &str, d: f64| m.get(k).and_then(|x| x.as_f64()).unwrap_or(d);
                    Some((g("stiffness", k0), g("damping", c0), g("mass", m0).max(0.01)))
                }
                _ => {
                    self.d.push(Diagnostic::error(&p, "inertia is true, a preset name or {stiffness, damping, mass, props}"));
                    None
                }
            };
            if let Some((k, c, m)) = params {
                let mut slots = Vec::new();
                for name in &props {
                    match prop::lookup(if name == "scale" { "scaleX" } else { name }) {
                        Some((s, ch)) if ch <= 2 && !(prop::FILL..prop::FILL + 8).contains(&s) => {
                            slots.push(s as u8);
                            if name == "scale" {
                                slots.push(prop::SY as u8);
                            }
                        }
                        _ => self.d.push(Diagnostic::error(format!("{p}.props"), format!("'{name}' can't follow a spring (use x, y, rotation, scale, opacity, …)"))),
                    }
                }
                slots.dedup();
                n.inertia = Some(Box::new(fx::InertiaC { slots, stiffness: k as f32, damping: c as f32, mass: m as f32, table: Default::default() }));
            }
        }
        if let Some(d) = l.drag {
            if !matches!(n.kind, Kind::Group { .. }) {
                self.d.push(Diagnostic::warning(format!("{path}.drag"), "'drag' applies to group layers (children trail the group's motion)"));
            } else if (0.0..=2.0).contains(&d) {
                n.drag = d as f32;
            } else {
                self.d.push(Diagnostic::error(format!("{path}.drag"), "drag is seconds per child (e.g. 0.04)"));
            }
        }
        if let Some(p) = l.posterize_time {
            if p > 0.0 && p <= 240.0 {
                n.posterize_time = p as f32;
            } else {
                self.d.push(Diagnostic::error(format!("{path}.posterizeTime"), "posterizeTime is poses per second (e.g. 12)"));
            }
        }
        f.smear = match &l.smear {
            None | Some(serde_json::Value::Bool(false)) => None,
            Some(serde_json::Value::Bool(true)) => Some(1.0),
            Some(serde_json::Value::Number(x)) => Some(x.as_f64().unwrap_or(1.0).clamp(0.0, 10.0) as f32),
            Some(_) => {
                self.d.push(Diagnostic::error(format!("{path}.smear"), "smear is true or a strength (1 = default)"));
                None
            }
        };
        if !f.is_empty() {
            n.fx = Some(Box::new(f));
        }
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
        // Multi-line text anchored on its left defaults to flush left (centred
        // paragraphs are rarely what a left anchor means).
        let left_anchor = match &l.anchor {
            Some(spec::Anchor::Name(a)) => a.to_ascii_lowercase().contains("left"),
            Some(spec::Anchor::Xy([x, _])) => *x < 0.34,
            None => false,
        };
        let multi = l.text.as_deref().is_some_and(|t| t.contains('\n')) || given_w.is_some() || l.max_width.is_some();
        let align = match st.align.as_deref() {
            None if left_anchor && multi => text::Align::Left,
            None | Some("center") => text::Align::Center,
            Some("left") | Some("start") => text::Align::Left,
            Some("right") | Some("end") => text::Align::Right,
            Some("justify") => text::Align::Justify,
            Some(a) => {
                self.d.push(Diagnostic::error(format!("{path}.align"), format!("unknown align '{a}'")).suggest(suggest(a, val::ALIGNS)));
                text::Align::Center
            }
        };
        let st_size = size;
        let mut raw = l.text.clone().unwrap_or_default();
        if l.typeset != Some(false) && !raw.contains("{value") {
            raw = text::layout::typeset(&raw);
        }
        if st.uppercase == Some(true) {
            // Not inside `[word]{$token}` markup: colour tokens stay as written.
            let mut depth = 0;
            raw = raw
                .chars()
                .flat_map(|c| {
                    match c {
                        '{' => depth += 1,
                        '}' => depth = (depth - 1).max(0),
                        _ => {}
                    }
                    if depth > 0 { vec![c] } else { c.to_uppercase().collect() }
                })
                .collect();
        }
        let col = st.color.clone().unwrap_or(spec::Paint::Css("$text".into()));
        let (paint, c) = self.paint(&col, &format!("{path}.color")).unwrap_or((PaintC::Solid, Rgba([1.0; 4])));
        n.base[prop::FILL..prop::FILL + 4].copy_from_slice(&rgba_to_oklab(c));
        if let Some(ce) = self.last_cexpr.take() {
            n.cexprs.push((prop::FILL as u8, ce));
        }
        n.base[prop::SIZE] = size;
        // Outlined text: `stroke` (colour or object); `color: "none"` draws only the outline.
        let hollow = matches!(&st.color, Some(spec::Paint::Css(c)) if c.trim() == "none" || c.trim() == "transparent");
        let outline = match &l.stroke {
            Some(sk) => self.stroke(&sk.0, l.stroke_width.as_ref(), &format!("{path}.stroke")).map(|(_, c, w)| {
                n.base[prop::STROKE..prop::STROKE + 4].copy_from_slice(&rgba_to_oklab(c));
                n.base[prop::SW] = w;
                text::layout::Outline { width: w / size, color: Some(text::Color(c)), hollow }
            }),
            None => None,
        };
        let accent = self.theme.colors.get("accent").copied().unwrap_or(Rgba([1.0, 0.36, 0.66, 1.0]));
        let style = text::TextStyle {
            outline,
            accent: text::Color(accent),
            font: family,
            weight,
            italic: st.italic.unwrap_or(false),
            size,
            color: text::Color(c),
            letter_spacing: ls / size,
            line_height: st.line_height.unwrap_or(1.2) as f32,
            ..Default::default()
        };
        let mut style = style;
        if let Some(f) = &l.features {
            style.features = Some(text::layout::features_css(f));
        }
        self.text_axes(l, &anim_src_of(l), &mut style, n, path);
        let (plain, spans) = self.markup(&raw, path);
        let run_colors = !spans.is_empty();
        // Text on a path never wraps: one line rides the path.
        let wrap_w = if l.text_path.is_some() { None } else { given_w };
        let mut tspec = text::TextSpec { text: plain.clone(), spans, style, width: wrap_w, align, max_lines: l.max_lines, ..Default::default() };
        // `fit: true`: shrink (never grow) so the widest line fits `width`
        // (default 85% of the parent), keeping the author's line breaks.
        match l.fit.as_deref() {
            None | Some("false") => {}
            Some("true") => {
                let target = given_w.unwrap_or(pc.size[0] * 0.85);
                let mut probe = tspec.clone();
                probe.width = None;
                for _ in 0..3 {
                    let w0 = self.assets.fonts.layout(&probe).width;
                    if w0 <= target + 0.5 {
                        break;
                    }
                    probe.style.size *= target / w0;
                }
                tspec.style.size = probe.style.size;
                n.base[prop::SIZE] = probe.style.size;
            }
            Some(mode @ ("width" | "box" | "height")) => {
                // Optical fit, grow or shrink: the *ink* (not the em box) fills
                // `width` (default 90% of the parent) and, for "box"/"height",
                // `height`. Line breaks are kept.
                let tw = given_w.unwrap_or(pc.size[0] * 0.9);
                let th = l.height.as_ref().map(|v| self.len_c(v, &format!("{path}.height"), pc.size[1], 16.0)).unwrap_or(pc.size[1] * 0.9);
                let mut probe = tspec.clone();
                probe.width = None;
                for _ in 0..4 {
                    let lay = self.assets.fonts.layout(&probe);
                    let ink = ink_bounds(&self.assets.fonts, &lay);
                    let (iw, ih) = ((ink[2] - ink[0]).max(1.0), (ink[3] - ink[1]).max(1.0));
                    let k = match mode {
                        "width" => tw / iw,
                        "height" => th / ih,
                        _ => (tw / iw).min(th / ih),
                    };
                    if (k - 1.0).abs() < 0.002 {
                        break;
                    }
                    probe.style.size = (probe.style.size * k).clamp(1.0, 4000.0);
                }
                tspec.style.size = probe.style.size;
                tspec.width = None;
                n.base[prop::SIZE] = probe.style.size;
            }
            Some(x) => self.d.push(
                Diagnostic::error(format!("{path}.fit"), format!("on text, 'fit' is true (shrink to width), \"width\", \"height\" or \"box\" (got '{x}')"))
                    .suggest(suggest(x, &["width", "height", "box"])),
            ),
        }
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
            None => {
                let l0 = self.assets.fonts.layout(&tspec);
                // No width given and wider than the parent: wrap at 90% of it.
                let mw = l.max_width.as_ref().map(|v| self.len_c(v, &format!("{path}.maxWidth"), pc.size[0], st_size));
                let maxw = mw.unwrap_or(pc.size[0] * 0.9);
                if given_w.is_none() && l0.width > maxw && (!pc.in_box || mw.is_some()) {
                    tspec.width = Some(maxw);
                    let lw = self.assets.fonts.layout(&tspec);
                    // Balanced breaks for headlines (CSS text-wrap: balance): the narrowest
                    // width with the same line count, so no line is a stub.
                    let n_lines = lw.lines.len();
                    let headline = tspec.style.size >= self.h * 0.04;
                    if n_lines >= 2 && l.balance.unwrap_or(headline) {
                        let (mut lo, mut hi) = (maxw / n_lines as f32 * 0.8, maxw);
                        for _ in 0..10 {
                            let mid = (lo + hi) * 0.5;
                            let mut probe = tspec.clone();
                            probe.width = Some(mid);
                            if self.assets.fonts.layout(&probe).lines.len() > n_lines {
                                lo = mid;
                            } else {
                                hi = mid;
                            }
                        }
                        tspec.width = Some((hi + 1.0).min(maxw));
                        self.assets.fonts.layout(&tspec)
                    } else {
                        lw
                    }
                } else {
                    l0
                }
            }
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
                Some(p) if p.0.preset.as_deref().is_some_and(|x| !val::ENTER_PRESETS.contains(&val::canon_preset(x, false))) => {
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
        let wrap = if counter.is_some() { None } else { tspec.width };
        let nat = [wrap.or(given_w).unwrap_or(layout.width), layout.height];
        if split.is_some() && (counter.is_some() || kinetic.is_some()) {
            self.d.push(Diagnostic::warning(format!("{path}.split"), "split is ignored on counters and kinetic text"));
        }
        let split = if counter.is_some() || kinetic.is_some() { None } else { split.map(|by| SplitC { by, tracks: Vec::new(), delays: Vec::new(), unit_tracks: Vec::new() }) };
        let has_split = split.is_some();
        // Animated variable axes or tracking: lay out again per frame (the
        // static layout above is the base; values are quantised and cached).
        let anim_src = l.animate.as_ref().map(|a| format!("{a:?}")).unwrap_or_default();
        let dynamic = (counter.is_none() && kinetic.is_none() && (!self.uniform_slots.is_empty() || anim_src.contains("letterSpacing"))).then(|| {
            n.base[prop::LS] = tspec.style.letter_spacing * tspec.style.size;
            let axes = self.uniform_slots.iter().filter_map(|(name, o, _)| name.strip_prefix("axes.").map(|t| (t.to_string(), prop::U0 + o))).collect();
            Box::new(DynText { spec: tspec.clone(), axes, ls: n.base[prop::LS] })
        });
        (Kind::Text { layout, wrap, paint, run_colors, split, counter, kinetic, dynamic }, nat, chars, has_split, from_enter)
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

    /// Resolve a grid spec against a box.
    fn grid_c(&mut self, g: &spec::GridSpec, size: [f32; 2], path: &str) -> super::graphic::GridC {
        let (fw, fh) = (self.w, self.h);
        let m = self.grid_lens(&g.margin, "margin", (fh.min(fw) * 0.05).round(), [size[1], size[0]], path);
        let margin = match m.len() {
            1 => [m[0]; 4],
            2 => [m[0], m[1], m[0], m[1]],
            4 => [m[0], m[1], m[2], m[3]],
            _ => {
                self.d.push(Diagnostic::error(format!("{path}.margin"), "margin takes 1, 2 or 4 lengths"));
                [m[0]; 4]
            }
        };
        let gt = self.grid_lens(&g.gutter, "gutter", (fh.min(fw) * 0.02).round(), [size[0], size[1]], path);
        let gutter = if gt.len() >= 2 { [gt[0], gt[1]] } else { [gt[0]; 2] };
        let cols = g.columns.unwrap_or(12).clamp(1, 200);
        let rows = g.rows.unwrap_or(8).clamp(1, 200);
        let gc = super::graphic::GridC { cols, rows, margin, gutter, baseline: g.baseline.map(|b| b as f32).filter(|b| *b >= 2.0), size };
        if gc.col_w() <= 0.0 || gc.row_h() <= 0.0 {
            self.d.push(Diagnostic::error(path, "the grid's margins and gutters leave no room for its fields"));
        }
        gc
    }

    fn grid_lens(&mut self, v: &Option<serde_json::Value>, key: &str, def: f32, refl: [f32; 2], path: &str) -> Vec<f32> {
        let items: Vec<serde_json::Value> = match v {
            None => return vec![def],
            Some(serde_json::Value::Array(a)) => a.clone(),
            Some(x) => vec![x.clone()],
        };
        if items.is_empty() {
            return vec![def];
        }
        let mut out = Vec::new();
        for (i, x) in items.iter().enumerate() {
            match serde_json::from_value::<Num>(x.clone()) {
                Ok(n) => out.push(self.len_c(&n, &format!("{path}.{key}"), refl[i % 2], 16.0)),
                Err(_) => {
                    self.d.push(Diagnostic::error(format!("{path}.{key}"), "expected a length or an array of lengths"));
                    out.push(def);
                }
            }
        }
        out
    }

    /// A hand-placed layer whose edge sits a few px off a grid line reads as a mistake: say so, with the snap.
    fn near_grid_check(&mut self, l: &Layer, path: &str, n: &Node, size: [f32; 2]) {
        let Some(g) = self.grid.clone() else { return };
        // Overprinted plates are offset on purpose; the grid overlay is the grid.
        if path.contains(".grid[") || l.blend.as_deref().is_some_and(|b| b != "normal") || l.boil.is_some() {
            return;
        }
        if !matches!((&l.x, &l.y), (Some(Num::Num(_)), _) | (_, Some(Num::Num(_)))) {
            return;
        }
        let saved = self.d.len();
        let gc = self.grid_c(&g, size, path);
        self.d.truncate(saved);
        let (w, h) = (n.base[prop::W], n.base[prop::H]);
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for c in 1..=gc.cols {
            let [x, _, cw, _] = gc.rect(c, c, 1, 1);
            xs.extend([x, x + cw]);
        }
        for r in 1..=gc.rows {
            let [_, y, _, rh] = gc.rect(1, 1, r, r);
            ys.extend([y, y + rh]);
        }
        let near = |v: f32, lines: &[f32]| {
            // On a line already (within rounding) means aligned: no advice.
            if lines.iter().any(|l| (l - v).abs() <= 1.5) {
                return None;
            }
            lines.iter().map(|l| (l - v, *l)).filter(|(d, _)| d.abs() < 8.0).min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
        };
        let mut msgs = Vec::new();
        // Only edges the author placed (left/right/top/bottom anchors), not centred layers.
        if let (Some(Num::Num(x)), true) = (&l.x, n.anchor[0] < 0.01 || n.anchor[0] > 0.99) {
            let left = *x as f32 - n.anchor[0] * w;
            if let Some((d, line)) = near(left, &xs) {
                msgs.push(format!("left edge {left:.0} px is {:.0} px off the grid line at {line:.0} (x = {:.0} snaps it)", d.abs(), *x as f32 + d));
            }
        }
        if let (Some(Num::Num(y)), true) = (&l.y, n.anchor[1] < 0.01 || n.anchor[1] > 0.99) {
            let top = *y as f32 - n.anchor[1] * h;
            if let Some((d, line)) = near(top, &ys) {
                msgs.push(format!("top edge {top:.0} px is {:.0} px off the grid line at {line:.0} (y = {:.0} snaps it)", d.abs(), *y as f32 + d));
            }
        }
        if !msgs.is_empty() {
            self.d.push(Diagnostic::warning(path, format!("almost on the grid: {}; or use \"cell\"", msgs.join("; "))));
        }
    }

    /// The rect of a cell value in the current grid (for tweens).
    fn cell_rect(&mut self, c: &serde_json::Value, path: &str, size: [f32; 2]) -> Option<[f32; 4]> {
        let Some(g) = self.grid.clone() else {
            self.d.push(Diagnostic::error(path, "'cell' needs a grid on the video, the scene or a parent group"));
            return None;
        };
        let gc = self.grid_c(&g, size, path);
        let r = match c {
            serde_json::Value::String(s) => match g.areas.as_ref().and_then(|a| a.get(s.as_str())) {
                Some(r) => super::graphic::parse_cell(r, gc.cols, gc.rows),
                None => super::graphic::parse_cell(s, gc.cols, gc.rows),
            },
            serde_json::Value::Array(a) if (2..=4).contains(&a.len()) => {
                let v: Vec<u32> = a.iter().map(|x| x.as_f64().unwrap_or(1.0).max(1.0) as u32).collect();
                let (nc, nr) = (v.get(2).copied().unwrap_or(1), v.get(3).copied().unwrap_or(1));
                if v[0] + nc - 1 > gc.cols || v[1] + nr - 1 > gc.rows {
                    Err("runs past the grid".into())
                } else {
                    Ok((v[0], v[0] + nc - 1, v[1], v[1] + nr - 1))
                }
            }
            _ => Err("expected an area name, a range like \"1-6/2-4\" or [col, row, cols, rows]".into()),
        };
        match r {
            Ok((c0, c1, r0, r1)) => Some(gc.rect(c0, c1, r0, r1)),
            Err(e) => {
                self.d.push(Diagnostic::error(path, e));
                None
            }
        }
    }

    /// A copy of `l` placed in its grid cell.
    fn place_in_cell(&mut self, l: &Layer, c: &serde_json::Value, path: &str, size: [f32; 2]) -> Layer {
        let cp = format!("{path}.cell");
        let Some(g) = self.grid.clone() else {
            self.d.push(Diagnostic::error(&cp, "'cell' needs a grid: add \"grid\": { \"columns\": 12, \"rows\": 8 } to the video, the scene or a parent group"));
            return l.clone();
        };
        let gc = self.grid_c(&g, size, &cp);
        let range = match c {
            serde_json::Value::String(s) => match g.areas.as_ref().and_then(|a| a.get(s.as_str())) {
                Some(r) => super::graphic::parse_cell(r, gc.cols, gc.rows).map_err(|e| format!("area '{s}': {e}")),
                None if !s.trim_start().starts_with(|c: char| c.is_ascii_digit()) => {
                    let names: Vec<&str> = g.areas.as_ref().map(|a| a.keys().map(|k| k.as_str()).collect()).unwrap_or_default();
                    Err(if names.is_empty() { format!("unknown grid area '{s}' (the grid has no `areas`)") } else { format!("unknown grid area '{s}' (areas: {})", names.join(", ")) })
                }
                None => super::graphic::parse_cell(s, gc.cols, gc.rows),
            },
            serde_json::Value::Array(a) if (2..=4).contains(&a.len()) => {
                let v: Vec<u32> = a.iter().map(|x| x.as_f64().unwrap_or(1.0).max(1.0) as u32).collect();
                let (c0, r0) = (v[0], v[1]);
                let (nc, nr) = (v.get(2).copied().unwrap_or(1), v.get(3).copied().unwrap_or(1));
                if c0 + nc - 1 > gc.cols || r0 + nr - 1 > gc.rows {
                    Err(format!("[{c0}, {r0}, {nc}, {nr}] runs past the {}×{} grid", gc.cols, gc.rows))
                } else {
                    Ok((c0, c0 + nc - 1, r0, r0 + nr - 1))
                }
            }
            serde_json::Value::Number(n) => super::graphic::parse_cell(&n.to_string(), gc.cols, gc.rows),
            _ => Err("expected an area name, a range like \"1-6/2-4\" or [col, row, cols, rows]".into()),
        };
        let (c0, c1, r0, r1) = match range {
            Ok(r) => r,
            Err(e) => {
                let mut d = Diagnostic::error(&cp, e);
                if let (serde_json::Value::String(s), Some(a)) = (c, &g.areas) {
                    let names: Vec<&String> = a.keys().collect();
                    d = d.suggest(suggest(s, &names));
                }
                self.d.push(d);
                return l.clone();
            }
        };
        let [x, y, w, h] = gc.rect(c0, c1, r0, r1);
        let anchor = match &l.anchor {
            None => [0.0, 0.0],
            Some(spec::Anchor::Xy([a, b])) => [*a as f32, *b as f32],
            Some(spec::Anchor::Name(s)) => match s.as_str() {
                "top" => [0.5, 0.0],
                "bottom" => [0.5, 1.0],
                "left" => [0.0, 0.5],
                "right" => [1.0, 0.5],
                "topRight" => [1.0, 0.0],
                "bottomLeft" => [0.0, 1.0],
                "bottomRight" => [1.0, 1.0],
                "center" | "centre" => [0.5, 0.5],
                _ => [0.0, 0.0],
            },
        };
        let is_text = l.text.is_some() || l.kind.as_deref() == Some("text");
        let mut o = l.clone();
        o.cell = None;
        if o.anchor.is_none() {
            o.anchor = Some(spec::Anchor::Name("topLeft".into()));
        }
        if o.x.is_none() {
            o.x = Some(Num::Num((x + anchor[0] * w) as f64));
        }
        if o.y.is_none() {
            o.y = Some(Num::Num((y + anchor[1] * h) as f64));
        }
        if o.width.is_none() {
            o.width = Some(Num::Num(w as f64));
        }
        if is_text {
            if o.text_box_trim.is_none() {
                o.text_box_trim = Some(true);
            }
            if o.align.is_none() {
                o.align = Some(if anchor[0] < 0.25 { "left" } else if anchor[0] > 0.75 { "right" } else { "center" }.into());
            }
            if let (Some(b), Some(Num::Num(sz))) = (gc.baseline, &o.size) {
                // Leading on the baseline grid: the nearest whole number of baseline units.
                let lh = o.line_height.unwrap_or(1.2) * sz;
                let snapped = ((lh / b as f64).round().max(1.0)) * b as f64;
                o.line_height = Some(snapped / sz);
            }
        } else if o.height.is_none() {
            o.height = Some(Num::Num(h as f64));
        }
        o
    }

    /// Layers that draw a grid (fields tinted, baseline lines) for checking a layout.
    fn grid_overlay(&mut self, g: &spec::GridSpec, size: [f32; 2], under: bool, path: &str) -> Vec<Layer> {
        let gc = self.grid_c(g, size, path);
        let mut out = Vec::new();
        if under {
            // A hairline drawing of the field edges: a visible design element, not a debug tint.
            let col = g.color.clone().unwrap_or_else(|| "#12121240".into());
            let line = |x: f32, y: f32, w: f32, h: f32| Layer {
                shape: Some("rect".into()),
                x: Some(Num::Num(x as f64)),
                y: Some(Num::Num(y as f64)),
                width: Some(Num::Num(w as f64)),
                height: Some(Num::Num(h as f64)),
                anchor: Some(spec::Anchor::Name("topLeft".into())),
                fill: Some(spec::Paint::Css(col.clone())),
                ..Default::default()
            };
            let full = gc.rect(1, gc.cols, 1, gc.rows);
            for c in 1..=gc.cols {
                let [x, _, w, _] = gc.rect(c, c, 1, 1);
                out.push(line(x, full[1], 1.0, full[3]));
                out.push(line(x + w - 1.0, full[1], 1.0, full[3]));
            }
            for r in 1..=gc.rows {
                let [_, y, _, h] = gc.rect(1, 1, r, r);
                out.push(line(full[0], y, full[2], 1.0));
                out.push(line(full[0], y + h - 1.0, full[2], 1.0));
            }
            return out;
        }
        let tint = g.color.clone().unwrap_or_else(|| "#ff2d7a22".into());
        let rect = |x: f32, y: f32, w: f32, h: f32, fill: &str| Layer {
            shape: Some("rect".into()),
            x: Some(Num::Num(x as f64)),
            y: Some(Num::Num(y as f64)),
            width: Some(Num::Num(w.max(0.5) as f64)),
            height: Some(Num::Num(h.max(0.5) as f64)),
            anchor: Some(spec::Anchor::Name("topLeft".into())),
            fill: Some(spec::Paint::Css(fill.into())),
            ..Default::default()
        };
        for c in 1..=gc.cols {
            for r in 1..=gc.rows {
                let [x, y, w, h] = gc.rect(c, c, r, r);
                out.push(rect(x, y, w, h, &tint));
            }
        }
        if let Some(b) = gc.baseline {
            let mut y = gc.margin[0];
            while y <= size[1] - gc.margin[2] + 0.5 {
                out.push(rect(gc.margin[3], y, size[0] - gc.margin[1] - gc.margin[3], 1.0, "#00b3ff55"));
                y += b;
            }
        }
        out
    }

    /// `cutout`: the layer becomes a group of a paper rim (with a printed shadow) and the
    /// layer itself clipped to a torn / scissor-cut outline of its box.
    fn cutout_group(&mut self, l: &Layer, c: &spec::CutoutValue, path: &str) -> Option<Layer> {
        let cp = format!("{path}.cutout");
        let o = match c {
            spec::CutoutValue::Edge(e) => spec::CutoutSpec { edge: Some(e.clone()), ..Default::default() },
            spec::CutoutValue::Full(f) => f.clone(),
        };
        let edge = o.edge.clone().unwrap_or_else(|| "torn".into());
        if !["torn", "scissor", "clean"].contains(&edge.as_str()) {
            self.d.push(Diagnostic::error(&cp, format!("unknown cutout edge '{edge}'")).suggest(suggest(&edge, &["torn", "scissor", "clean"])));
            return None;
        }
        let (Some(Num::Num(w)), Some(Num::Num(h))) = (&l.width, &l.height) else {
            self.d.push(Diagnostic::error(&cp, "a cutout needs a numeric width and height (or a grid cell)"));
            return None;
        };
        let (w, h) = (*w, *h);
        let amount = o.amount.unwrap_or(if edge == "scissor" { 18.0 } else { 14.0 });
        let rim = o.rim.unwrap_or(if edge == "torn" { 8.0 } else { 0.0 }).max(0.0);
        let seed = o.seed.unwrap_or(1);
        let inner_d = super::graphic::cutout_d(w, h, &edge, amount, 0.0, seed);
        let rim_d = super::graphic::cutout_d(w, h, &edge, amount * 1.25, rim, seed.wrapping_add(17));
        let mut inner = l.clone();
        inner.cutout = None;
        let mut g = Layer {
            kind: Some("group".into()),
            id: l.id.clone(),
            width: Some(Num::Num(w)),
            height: Some(Num::Num(h)),
            ..Default::default()
        };
        // Placement, time and motion move to the group; looks stay on the content.
        macro_rules! mv {
            ($($f:ident),*) => { $( g.$f = inner.$f.take(); )* };
        }
        mv!(x, y, anchor, scale, scale_x, scale_y, rotation, skew_x, skew_y, opacity, start, end, duration, enter, exit, animate, loop_, depth, blend, step, boil, path, orient, repeat, echo);
        inner.id = None;
        inner.x = Some(Num::Num(w / 2.0));
        inner.y = Some(Num::Num(h / 2.0));
        let clip = Layer { shape: Some("path".into()), d: Some(inner_d), ..Default::default() };
        inner.clip = Some(spec::Clip::Shape(Box::new(clip)));
        let mut layers = Vec::new();
        if rim > 0.0 || o.shadow != Some(false) {
            let mut r = Layer {
                shape: Some("path".into()),
                d: Some(if rim > 0.0 { rim_d } else { super::graphic::cutout_d(w, h, &edge, amount, 0.0, seed) }),
                x: Some(Num::Num(w / 2.0)),
                y: Some(Num::Num(h / 2.0)),
                fill: Some(spec::Paint::Css(o.rim_color.clone().unwrap_or_else(|| "#f4f1ea".into()))),
                ..Default::default()
            };
            if o.shadow != Some(false) {
                r.shadow = Some(spec::Shadow(spec::ShadowObj { x: Some(5.0), y: Some(7.0), blur: Some(3.0), color: Some("#00000059".into()) }));
            }
            layers.push(r);
        }
        layers.push(inner);
        g.layers = Some(layers);
        Some(g)
    }

    /// Image treatments (`duotone`, `gradientMap`, `posterize`, `halftone`) as layer filters.
    fn treatments(&mut self, l: &Layer, path: &str, n: &mut Node) {
        let expanded;
        let l = match l.print.as_deref() {
            Some(p) => match print_preset(l, p) {
                Some(x) => {
                    if l.blend.is_none() && x.blend.as_deref() == Some("multiply") {
                        n.blend = Blend::Multiply;
                    }
                    expanded = x;
                    &expanded
                }
                None => {
                    self.d.push(Diagnostic::error(format!("{path}.print"), format!("unknown print '{p}'")).suggest(suggest(p, PRINTS)));
                    l
                }
            },
            None => l,
        };
        let mut ht_nums: Option<(Option<Num>, Option<Num>, Option<Num>)> = None;
        let mut ink_num: Option<Num> = None;
        let mut stops: Option<Vec<(f32, Rgba)>> = None;
        if let Some(d) = &l.duotone {
            if d.len() != 2 {
                self.d.push(Diagnostic::error(format!("{path}.duotone"), "duotone takes two colours: [shadows, highlights]"));
            } else {
                let v: Vec<serde_json::Value> = d.iter().map(|c| serde_json::Value::String(c.clone())).collect();
                stops = self.stops(&v, &format!("{path}.duotone"));
            }
        }
        if let Some(g) = &l.gradient_map {
            if stops.is_some() {
                self.d.push(Diagnostic::warning(format!("{path}.gradientMap"), "both duotone and gradientMap given; gradientMap wins"));
            }
            stops = self.stops(g, &format!("{path}.gradientMap"));
        }
        let levels = l.posterize.map(|p| p.clamp(0, 64) as u8).unwrap_or(0);
        if l.posterize == Some(1) {
            self.d.push(Diagnostic::warning(format!("{path}.posterize"), "posterize needs at least 2 levels"));
        }
        if stops.is_some() || levels >= 2 {
            let map = stops.map(|mut st| {
                st.sort_by(|a, b| a.0.total_cmp(&b.0));
                let labs: Vec<(f32, [f32; 4])> = st.iter().map(|(o, c)| (*o, rgba_to_oklab(*c))).collect();
                let mut m = Box::new([[0u8; 3]; 256]);
                let [lo, hi] = l.tone_range.unwrap_or([0.0, 1.0]).map(|v| v as f32);
                for (i, out) in m.iter_mut().enumerate() {
                    let mut t = ((i as f32 / 255.0 - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0);
                    if levels >= 2 {
                        let k = (levels - 1) as f32;
                        t = (t * k).round() / k;
                    }
                    let k = labs.iter().position(|(o, _)| *o >= t).unwrap_or(labs.len() - 1);
                    let lab = if k == 0 {
                        labs[0].1
                    } else {
                        let (a, b) = (labs[k - 1], labs[k]);
                        let f = if b.0 > a.0 { ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 1.0 };
                        std::array::from_fn(|j| a.1[j] + (b.1[j] - a.1[j]) * f)
                    };
                    let c = super::oklab_to_rgba(lab);
                    *out = [0, 1, 2].map(|j| (c.0[j].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
                }
                m
            });
            let amount = 1.0;
            // With a map, posterising is baked into it.
            let levels = if map.is_some() { 0 } else { levels };
            n.treat.push(Filter::Tone(Arc::new(dl::ToneLut { map, levels, amount })));
        }
        if let Some(h) = &l.halftone {
            let hp = format!("{path}.halftone");
            let hs = match h {
                spec::HalftoneValue::Cell(c) => spec::HalftoneSpec { cell: Some(Num::Num(*c)), ..Default::default() },
                spec::HalftoneValue::Full(f) => f.clone(),
            };
            let shape = match hs.shape.as_deref().unwrap_or("dot") {
                "dot" | "dots" | "round" => 0,
                "line" | "lines" => 1,
                "square" | "diamond" => 2,
                x => {
                    self.d.push(Diagnostic::error(format!("{hp}.shape"), format!("unknown halftone shape '{x}'")).suggest(suggest(x, &["dot", "line", "square"])));
                    0
                }
            };
            let col = |me: &mut Self, v: &Option<String>, key: &str, none: &str| -> Rgba {
                match v.as_deref() {
                    None => Rgba([0.0; 4]),
                    Some(x) if x == none || x == "none" || x == "source" => Rgba([0.0; 4]),
                    Some(x) => me.color(x, &format!("{hp}.{key}")).unwrap_or(Rgba([0.0, 0.0, 0.0, 1.0])),
                }
            };
            let cmyk = hs.ink.as_deref() == Some("cmyk");
            let ink = if cmyk { Rgba([0.0; 4]) } else { col(self, &hs.ink, "ink", "source") };
            let paper = col(self, &hs.paper, "paper", "none");
            let cell = match &hs.cell {
                Some(Num::Num(c)) => *c as f32,
                _ => 10.0,
            };
            if cell < 3.0 {
                self.d.push(Diagnostic::warning(format!("{hp}.cell"), "cells under 3 px alias into moiré after encoding; use 4 px or more"));
            }
            n.treat.push(Filter::Halftone(dl::Halftone { cell: cell.max(2.0), angle: 45.0, shape, ink, paper, gain: 1.0, ox: 0.0, oy: 0.0, cmyk }));
            ht_nums = Some((hs.cell.clone(), hs.angle.clone(), hs.gain.clone()));
        }
        if let Some(iv) = &l.ink_texture {
            let (amount, scale, seed) = match iv {
                spec::InkValue::Amount(a) => (Num::Num(*a), 28.0, 1),
                spec::InkValue::Full(o) => (o.amount.clone().unwrap_or(Num::Num(0.35)), o.scale.unwrap_or(28.0), o.seed.unwrap_or(1)),
            };
            n.ink_follow = !matches!(iv, spec::InkValue::Full(spec::InkSpec { fixed: Some(true), .. }));
            n.treat.push(Filter::Ink { amount: 0.35, scale: scale.max(1.0) as f32, seed, ox: 0.0, oy: 0.0 });
            ink_num = Some(amount);
        }
        // Halftone on reading-size text breaks it into dots that H.264 smears.
        if let (Some(_), Some(_), Some(Num::Num(sz))) = (&l.halftone, &l.text, &l.size) {
            let cell = match &l.halftone {
                Some(spec::HalftoneValue::Cell(c)) => *c,
                Some(spec::HalftoneValue::Full(f)) => match &f.cell { Some(Num::Num(c)) => *c, _ => 10.0 },
                None => 10.0,
            };
            if *sz < cell * 8.0 {
                self.d.push(Diagnostic::warning(format!("{path}.halftone"), format!("halftone cells of {cell} px on {sz} px text: under ~8 cells per letter height it stops reading; use it on display type (≥ {} px) or a smaller cell", (cell * 8.0).round())));
            }
        }
        // Animatable treatment parameters live in uniform slots after any shader uniforms,
        // so `animate`, keyframes and "=expr" all reach them by name.
        if !n.treat.is_empty() {
            let used: usize = self.uniform_slots.iter().map(|(_, o, c)| o + c).max().unwrap_or(0);
            if used + 5 > super::shader::UNIFORM_SLOTS {
                self.d.push(Diagnostic::warning(path, "too many shader uniforms to also animate this layer's treatments; they stay fixed"));
            } else {
                let b = prop::U0 + used;
                n.treat_slot = b as u8;
                for (k, name) in ["toneAmount", "halftoneCell", "halftoneAngle", "halftoneGain", "inkAmount"].iter().enumerate() {
                    self.uniform_slots.push((name.to_string(), used + k, 1));
                }
                let (c, a, g) = ht_nums.unwrap_or((None, None, None));
                let one = Num::Num(1.0);
                self.set(n, b, Some(l.tone_amount.as_ref().unwrap_or(&one)), &format!("{path}.toneAmount"), 1.0);
                self.set(n, b + 1, Some(c.as_ref().unwrap_or(&Num::Num(10.0))), &format!("{path}.halftone.cell"), 1.0);
                self.set(n, b + 2, Some(a.as_ref().unwrap_or(&Num::Num(45.0))), &format!("{path}.halftone.angle"), 1.0);
                self.set(n, b + 3, Some(g.as_ref().unwrap_or(&one)), &format!("{path}.halftone.gain"), 1.0);
                self.set(n, b + 4, Some(ink_num.as_ref().unwrap_or(&Num::Num(0.35))), &format!("{path}.inkTexture.amount"), 1.0);
            }
        }
    }

    fn shape_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>, n: &mut Node) -> (Kind, [f32; 2]) {
        let sname = l.shape.as_deref().unwrap_or(if l.pattern.is_some() { "pattern" } else { "rect" });
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
            "superellipse" | "squircle" => (ShapeKind::Path, super::graphic::superellipse(w as f64, h as f64, l.exponent.unwrap_or(4.0)), [w, h]),
            "blob" => (ShapeKind::Path, super::graphic::blob(w as f64, h as f64, l.sides.unwrap_or(6), l.wobble.unwrap_or(0.3), l.seed.unwrap_or(1)), [w, h]),
            "arc" => {
                let a = l.start_angle.unwrap_or(0.0);
                let b = l.end_angle.unwrap_or(270.0);
                (ShapeKind::Path, super::graphic::arc(w as f64, h as f64, a, b, l.inner_radius.unwrap_or(0.0)), [w, h])
            }
            "pattern" => {
                let (w, h) = (gw.unwrap_or(self.w), gh.unwrap_or(self.h));
                let pp = format!("{path}.pattern");
                let pat = match &l.pattern {
                    None => {
                        self.d.push(Diagnostic::error(&pp, "a pattern shape needs 'pattern' (e.g. \"dots 32\")"));
                        None
                    }
                    Some(spec::PatternValue::Short(s)) => {
                        let mut it = s.split_whitespace();
                        let kind = it.next().unwrap_or("dots").to_string();
                        let spacing = it.next().and_then(|x| x.trim_end_matches("px").parse::<f64>().ok()).unwrap_or(40.0);
                        Some(super::graphic::Pat { kind, spacing, mark: None, angle: 0.0, seed: l.seed.unwrap_or(1), jitter: 0.0, ramp: None, tile: None, turns: 0, stagger: false, tile_stroke: 0.0, field: None, t: 0.0, cover: None })
                    }
                    Some(spec::PatternValue::Full(f)) => Some(super::graphic::Pat {
                        kind: if f.kind.is_empty() && f.tile.is_some() { "tile".into() } else { f.kind.clone() },
                        tile: match &f.tile {
                            Some(d) => match Path::from_svg(d) {
                                Ok(p) => Some(p),
                                Err(e) => {
                                    self.d.push(Diagnostic::error(format!("{pp}.tile"), format!("invalid SVG path: {e} (SVG path data like \"M0 0 L100 0 L50 80 Z\" or \"M10 80 C40 10 65 10 95 80\")")));
                                    None
                                }
                            },
                            None => None,
                        },
                        turns: f.turns.unwrap_or(0).min(360),
                        stagger: f.stagger.unwrap_or(false),
                        tile_stroke: f.tile_stroke.unwrap_or(0.0).max(0.0),
                        cover: None,
                        field: match &f.field {
                            Some(src) => match super::expr::Expr::compile_field(src.trim().trim_start_matches('='), self.w as f64, self.h as f64) {
                                Ok(e) => Some(e),
                                Err(e) => {
                                    self.d.push(Diagnostic::error(format!("{pp}.field"), format!("field: {e}")));
                                    None
                                }
                            },
                            None => None,
                        },
                        t: 0.0,
                        spacing: f.spacing.unwrap_or(40.0),
                        mark: f.mark,
                        angle: f.angle.unwrap_or(0.0),
                        seed: f.seed.or(l.seed).unwrap_or(1),
                        jitter: f.jitter.unwrap_or(0.0),
                        ramp: f.ramp,
                    }),
                };
                let bp = match pat {
                    Some(mut pt) => {
                        if !super::graphic::PATTERNS.contains(&pt.kind.as_str()) {
                            self.d.push(Diagnostic::error(&pp, format!("unknown pattern '{}'", pt.kind)).suggest(suggest(&pt.kind, super::graphic::PATTERNS)));
                            pt.kind = "dots".into();
                        }
                        if pt.kind == "tile" && pt.tile.is_none() {
                            self.d.push(Diagnostic::error(&pp, "a \"tile\" pattern needs \"tile\": SVG path data centred on 0,0 (e.g. \"M-8 -8 L8 -8 L0 10 Z\")"));
                        }
                        if pt.spacing < 4.0 {
                            self.d.push(Diagnostic::warning(&pp, "spacing under 4 px turns to moiré after encoding"));
                        }
                        // If the size animates (cell tweens, width/height, scale), cover the whole frame: marks are clipped anyway.
                        let anim = format!("{:?} {:?}", l.animate, l.update);
                        if ["\"cell\"", "\"width\"", "\"height\"", "\"scale", "\"zoom", "width:", "height:", "scale"].iter().any(|k| anim.contains(k)) || matches!(&l.width, Some(Num::Str(_))) || matches!(&l.height, Some(Num::Str(_))) {
                            pt.cover = Some([self.w as f64 * 1.2, self.h as f64 * 1.2]);
                        }
                        if pt.field.as_ref().is_some_and(|e| e.dynamic) {
                            n.live_pattern = Some(Arc::new((pt.clone(), [w as f64, h as f64])));
                        }
                        super::graphic::pattern(w as f64, h as f64, &pt)
                    }
                    None => Path::new(),
                };
                n.clip = true;
                (ShapeKind::Path, bp, [w, h])
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
                        self.d.push(Diagnostic::error(format!("{path}.d"), format!("invalid SVG path: {e} (SVG path data like \"M0 0 L100 0 L50 80 Z\" or \"M10 80 C40 10 65 10 95 80\")")));
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
        if let Some(ce) = self.last_cexpr.take() {
            n.cexprs.push((prop::FILL as u8, ce));
        }
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
                if let Some(ce) = self.last_cexpr.take() {
                    n.cexprs.push((prop::STROKE as u8, ce));
                }
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
        let full = self.media_path(&src, path);
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
        for (k, ch) in kids_spec.iter().enumerate() {
            if ch.x.is_some() || ch.y.is_some() {
                self.d.push(Diagnostic::warning(
                    format!("{path}.children[{k}]"),
                    "x/y on a box child are ignored: the box lays its children out (use the box's style.align/justify/gap, or put the layer in a group to place it freely)",
                ));
            }
        }
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
                if let Some(ce) = self.last_cexpr.take() {
                    n.cexprs.push((prop::FILL as u8, ce));
                }
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

    /// A static clip path from a shape layer, in the local box of a layer of size `box_`.
    fn clip_shape(&mut self, l: &Layer, path: &str, box_: [f32; 2]) -> Option<Path> {
        let mut l = l.clone();
        if l.shape.is_none() {
            l.shape = Some("rect".into());
        }
        l.kind = Some("shape".into());
        let pc = PCtx { size: box_, t0: 0.0, end: f32::INFINITY, delay: 0.0, index: 0, count: 1, in_box: false };
        let n = self.layer(&l, path, pc)?;
        if !n.tracks.is_empty() || !n.exprs.is_empty() {
            self.d.push(Diagnostic::warning(path, "clip shapes are static; their animation is ignored"));
        }
        let (w, h) = (n.base[prop::W], n.base[prop::H]);
        let mut p = match &n.kind {
            Kind::Shape { shape: ShapeKind::Rect, .. } => {
                let r = n.base[prop::RADIUS].min(w.min(h) * 0.5).max(0.0);
                kurbo::Shape::to_path(&kurbo::RoundedRect::new(0.0, 0.0, w as f64, h as f64, r as f64), 0.1)
            }
            Kind::Shape { shape: ShapeKind::Ellipse, .. } => {
                kurbo::Shape::to_path(&kurbo::Ellipse::new((w as f64 * 0.5, h as f64 * 0.5), (w as f64 * 0.5, h as f64 * 0.5), 0.0), 0.1)
            }
            Kind::Shape { path, .. } => path.clone(),
            _ => return None,
        };
        let m = super::local_xf(&n.base, n.anchor, w, h);
        p.apply_affine(kurbo::Affine::new(m.map(|x| x as f64)));
        Some(p)
    }

    /// Load the music for expressions (with its loudness envelope when `level` is read).
    fn load_groove(&mut self, level: bool, path: &str) {
        let need = level || self.groove_level;
        let g = self.beat_marks.groove(need);
        if let Err(m) = &g {
            self.d.push(Diagnostic::error(path, format!("expression uses the music: {m}")));
        }
        self.groove = Some(g);
        self.groove_level = need;
    }

    /// Resolve a `post` value (RENDER's presets and settings); `"none"` gives `None`.
    fn post(&mut self, v: &serde_json::Value, path: &str) -> Option<crate::render::post::PostSettings> {
        use crate::render::post::{PostSettings, PostSpec, PRESETS};
        // The spec is camelCase (`bloomThreshold`); the settings are snake_case.
        let snake = |k: &str| k.chars().fold(String::new(), |mut o, ch| {
            if ch.is_ascii_uppercase() {
                o.push('_');
                o.push(ch.to_ascii_lowercase());
            } else {
                o.push(ch);
            }
            o
        });
        let camel = |k: &str| k.split('_').enumerate().map(|(i, w)| if i == 0 { w.to_string() } else { w[..1].to_uppercase() + &w[1..] }).collect::<String>();
        let v = match v {
            serde_json::Value::Object(m) => serde_json::Value::Object(m.iter().map(|(k, x)| (snake(k), x.clone())).collect()),
            x => x.clone(),
        };
        let r = serde_json::from_value::<PostSpec>(v).map_err(|e| e.to_string()).and_then(|p| p.resolve());
        match r {
            Ok(s) => (!s.is_identity()).then_some(s),
            Err(e) => {
                let word = e.split('"').nth(1).unwrap_or_default().to_string();
                let (msg, sug) = if e.contains("field") {
                    let fields: Vec<String> = serde_json::to_value(PostSettings::preset("clean"))
                        .ok()
                        .and_then(|v| v.as_object().map(|m| m.keys().map(|k| camel(k)).collect()))
                        .unwrap_or_default();
                    let w = camel(&word);
                    (format!("unknown post setting '{w}'"), suggest(&w, &fields))
                } else {
                    (e, suggest(&word, PRESETS))
                };
                self.d.push(Diagnostic::error(path, msg).suggest(sug));
                None
            }
        }
    }

    /// Compile `compositions` in dependency order, each as a nested evaluator sharing this one's assets.
    fn compositions(&mut self, defs: &BTreeMap<String, spec::Composition>) {
        // Names each composition references through `"composition": "…"` layers.
        fn refs(v: &serde_json::Value, out: &mut Vec<String>) {
            match v {
                serde_json::Value::Object(m) => {
                    if let Some(serde_json::Value::String(n)) = m.get("composition") {
                        out.push(n.clone());
                    }
                    m.values().for_each(|x| refs(x, out));
                }
                serde_json::Value::Array(a) => a.iter().for_each(|x| refs(x, out)),
                _ => {}
            }
        }
        let deps: BTreeMap<&str, Vec<String>> = defs
            .iter()
            .map(|(k, c)| {
                let mut r = Vec::new();
                refs(&serde_json::json!({ "l": serde_json::to_value(&c.layers).unwrap_or_default(), "s": serde_json::to_value(&c.scenes).unwrap_or_default() }), &mut r);
                (k.as_str(), r)
            })
            .collect();
        let mut done: Vec<&str> = Vec::new();
        loop {
            let ready = defs.keys().find(|k| !done.contains(&k.as_str()) && deps[k.as_str()].iter().all(|d| done.contains(&d.as_str()) || !defs.contains_key(d)));
            let Some(name) = ready else { break };
            done.push(name);
            self.composition(name, &defs[name]);
        }
        for k in defs.keys().filter(|k| !done.contains(&k.as_str())) {
            self.d.push(Diagnostic::error(format!("compositions.{k}"), "compositions refer to each other in a cycle"));
        }
    }

    /// Compile one composition (also usable for anonymous inline ones: pass a unique
    /// `name`, e.g. the layer path). Returns its evaluator; it is also registered by name.
    pub(crate) fn composition(&mut self, name: &str, cdef: &spec::Composition) -> Option<Arc<Evaluator>> {
        let path = format!("compositions.{name}");
        let (scenes, from) = match (&cdef.layers, &cdef.scenes) {
            (Some(_), Some(_)) => {
                self.d.push(Diagnostic::error(&path, "a composition has 'layers' or 'scenes', not both"));
                return None;
            }
            (_, Some(s)) => (s.clone(), "scenes"),
            (l, None) => (
                vec![spec::Scene {
                    duration: Some(cdef.duration.clone().unwrap_or(Num::Str("auto".into()))),
                    layers: l.clone().unwrap_or_default(),
                    ..Default::default()
                }],
                "scenes[0].layers",
            ),
        };
        let sub = Spec {
            size: cdef.size.clone().or(Some(spec::Size::Wh([self.w as u32, self.h as u32]))),
            fps: Some(self.fps as f64),
            background: Some(cdef.background.clone().unwrap_or(spec::Paint::Css("#00000000".into()))),
            theme: self.spec_theme.clone(),
            scenes,
            transition: cdef.transition.clone(),
            duration: if from == "scenes" { cdef.duration.clone() } else { None },
            ..Default::default()
        };
        let opts = Options { assets: Some(self.assets.clone()), ..self.o.clone() };
        let (ev, d) = Evaluator::build_with(&sub, &opts, self.comps.clone());
        let to = if from == "scenes" { format!("{path}.scenes") } else { format!("{path}.layers") };
        for mut x in d {
            x.path = match x.path.strip_prefix(from) {
                Some(rest) => format!("{to}{rest}"),
                None => format!("{path}.{}", x.path),
            };
            self.d.push(x);
        }
        if let Some(ev) = ev {
            if (ev.width, ev.height) != (self.w as u32, self.h as u32) && ev.layer_info.iter().any(|(_, k)| k == "shader") {
                self.d.push(Diagnostic::warning(&path, format!("shader layers inside a composition render at the composition's size: iResolution is {}×{}, not the video's", ev.width, ev.height)));
            }
            let ev = Arc::new(ev);
            self.assets.register_composition(name, &ev, cdef.loop_.unwrap_or(false));
            self.comps.push((name.to_string(), ev.clone(), cdef.loop_.unwrap_or(false)));
            return Some(ev);
        }
        None
    }

    fn dots_layer(&mut self, l: &Layer, path: &str) -> Kind {
        let d = l.dots.clone().unwrap_or_default();
        let base = d.color.as_deref().and_then(|c| self.color(c, &format!("{path}.dots.color"))).or_else(|| self.theme.color("$text")).unwrap_or(Rgba([1.0; 4]));
        // Rows: [x, y] or [x, y, colour].
        let rows: Vec<(f64, f64, Option<String>)> = match &d.data {
            serde_json::Value::Array(a) => a
                .iter()
                .filter_map(|r| {
                    let r = r.as_array()?;
                    Some((r.first()?.as_f64()?, r.get(1)?.as_f64()?, r.get(2).and_then(|c| c.as_str()).map(str::to_string)))
                })
                .collect(),
            serde_json::Value::String(file) => match std::fs::read_to_string(self.o.base_dir.join(file)) {
                Ok(src) => src
                    .lines()
                    .filter_map(|line| {
                        let mut f = line.split(',').map(str::trim);
                        let x = f.next()?.parse().ok()?;
                        let y = f.next()?.parse().ok()?;
                        Some((x, y, f.next().filter(|c| !c.is_empty()).map(str::to_string)))
                    })
                    .collect(),
                Err(e) => {
                    self.d.push(Diagnostic::error(format!("{path}.dots.data"), format!("cannot read {file}: {e}")));
                    Vec::new()
                }
            },
            _ => {
                self.d.push(Diagnostic::error(format!("{path}.dots.data"), "'data' is a list of [x, y] (or [x, y, colour]) or a CSV file path"));
                Vec::new()
            }
        };
        let mut groups: Vec<(Rgba, Vec<[f32; 2]>)> = Vec::new();
        let mut cache: HashMap<String, Rgba> = HashMap::new();
        for (x, y, c) in rows {
            let col = match c {
                Some(c) => match cache.get(&c) {
                    Some(v) => *v,
                    None => {
                        let v = self.color(&c, &format!("{path}.dots.data")).unwrap_or(base);
                        cache.insert(c, v);
                        v
                    }
                },
                None => base,
            };
            match groups.iter_mut().find(|g| g.0 == col) {
                Some(g) => g.1.push([x as f32, y as f32]),
                None => groups.push((col, vec![[x as f32, y as f32]])),
            }
        }
        let appear = d.appear.as_deref().and_then(|a| {
            let src = a.trim().strip_prefix('=').unwrap_or(a);
            match Expr::compile(src, 1.0, self.w as f64, self.h as f64) {
                Ok(e) => Some(e),
                Err(m) => {
                    self.d.push(Diagnostic::error(format!("{path}.dots.appear"), format!("expression: {m}")));
                    None
                }
            }
        });
        Kind::Dots { groups, size: d.size.unwrap_or(4.0).max(0.1) as f32, appear, square: d.shape.as_deref() == Some("square") }
    }

    fn draw_layer(&mut self, l: &Layer, path: &str) -> Kind {
        use kurbo::Shape;
        let mut out = Vec::new();
        for (k, it) in l.draw.iter().flatten().enumerate() {
            let p = format!("{path}.draw[{k}]");
            let given = [it.d.is_some(), it.points.is_some(), it.circle.is_some(), it.rect.is_some(), it.line.is_some()];
            if given.iter().filter(|g| **g).count() != 1 {
                self.d.push(Diagnostic::error(&p, "give exactly one of 'd', 'points', 'circle', 'rect', 'line'"));
                continue;
            }
            let closed = it.closed == Some(true);
            let pt = |q: [f64; 2]| kurbo::Point::new(q[0], q[1]);
            let mut bp = Path::new();
            if let Some(d) = &it.d {
                match Path::from_svg(d) {
                    Ok(b) => bp = b,
                    Err(e) => {
                        self.d.push(Diagnostic::error(format!("{p}.d"), format!("invalid SVG path: {e} (SVG path data like \"M0 0 L100 0 L50 80 Z\" or \"M10 80 C40 10 65 10 95 80\")")));
                        continue;
                    }
                }
            } else if let Some(ps) = &it.points {
                if ps.len() < 2 {
                    self.d.push(Diagnostic::error(format!("{p}.points"), "needs at least 2 points"));
                    continue;
                }
                bp.move_to(pt(ps[0]));
                let n = ps.len();
                if it.smooth == Some(true) && n > 2 {
                    // Catmull-Rom through the points, as cubic Béziers.
                    let get = |i: isize| pt(ps[if closed { i.rem_euclid(n as isize) as usize } else { i.clamp(0, n as isize - 1) as usize }]);
                    for i in 0..(if closed { n } else { n - 1 }) as isize {
                        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
                        bp.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
                    }
                } else {
                    for q in &ps[1..] {
                        bp.line_to(pt(*q));
                    }
                }
                if closed {
                    bp.close_path();
                }
            } else if let Some([cx, cy, r]) = it.circle {
                bp = kurbo::Circle::new((cx, cy), r.max(0.0)).to_path(0.1);
            } else if let Some([x, y, w, h]) = it.rect {
                bp = kurbo::RoundedRect::new(x, y, x + w, y + h, it.radius.unwrap_or(0.0).max(0.0)).to_path(0.1);
            } else if let Some([x1, y1, x2, y2]) = it.line {
                bp.move_to((x1, y1));
                bp.line_to((x2, y2));
            }
            let fill = it.fill.as_deref().filter(|f| *f != "none").and_then(|f| self.color(f, &format!("{p}.fill")));
            let cap = match it.cap.as_deref() {
                None | Some("butt") => 0,
                Some("round") => 1,
                Some("square") => 2,
                Some(x) => {
                    self.d.push(Diagnostic::error(format!("{p}.cap"), format!("unknown cap '{x}'")).suggest(suggest(x, &["butt", "round", "square"])));
                    0
                }
            };
            let join = match it.join.as_deref() {
                None | Some("miter") => 0,
                Some("round") => 1,
                Some("bevel") => 2,
                Some(x) => {
                    self.d.push(Diagnostic::error(format!("{p}.join"), format!("unknown join '{x}'")).suggest(suggest(x, &["miter", "round", "bevel"])));
                    0
                }
            };
            let mut segs = Vec::new();
            if let (Some(cs), Some(ps)) = (&it.colors, &it.points) {
                let cols: Vec<Rgba> = cs.iter().enumerate().map(|(i, c)| self.color(c, &format!("{p}.colors[{i}]")).unwrap_or(Rgba([1.0; 4]))).collect();
                if cols.len() != ps.len() {
                    self.d.push(Diagnostic::error(format!("{p}.colors"), format!("{} colours for {} points (one per point)", cols.len(), ps.len())));
                } else {
                    let n = ps.len();
                    for i in 0..(if closed { n } else { n - 1 }) {
                        let (a, b) = (ps[i], ps[(i + 1) % n]);
                        let mut sp = Path::new();
                        sp.move_to(pt(a));
                        sp.line_to(pt(b));
                        segs.push((sp, [a[0] as f32, a[1] as f32], [b[0] as f32, b[1] as f32], cols[i], cols[(i + 1) % n]));
                    }
                }
            }
            // Stroked when asked, or when nothing else would show (lines, open polylines, no fill).
            let stroked = segs.is_empty() && (it.stroke.is_some() || it.width.is_some() || fill.is_none());
            let stroke = if stroked {
                match it.stroke.as_deref() {
                    Some(s) => self.color(s, &format!("{p}.stroke")),
                    None => self.theme.color("$text"),
                }
            } else {
                None
            };
            out.push(super::DrawC {
                path: bp,
                fill,
                stroke,
                style: (it.width.unwrap_or(2.0).max(0.0) as f32, cap, join),
                segs,
                opacity: it.opacity.unwrap_or(1.0).clamp(0.0, 1.0) as f32,
            });
        }
        Kind::Draw(out)
    }

    fn comp_layer(&mut self, l: &Layer, path: &str) -> Option<(Kind, [f32; 2])> {
        let name = l.composition.as_deref().unwrap_or_default();
        let Some((_, ev, looped)) = self.comps.iter().find(|c| c.0 == name).cloned() else {
            let names: Vec<&str> = self.comps.iter().map(|c| c.0.as_str()).collect();
            let msg = if names.is_empty() { format!("unknown composition '{name}' (define it in top-level 'compositions')") } else { format!("unknown composition '{name}'") };
            self.d.push(Diagnostic::error(format!("{path}.composition"), msg).suggest(suggest(name, &names)));
            return None;
        };
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
        let looped = match &l.loop_ {
            Some(spec::LoopValue::Clip(b)) => *b,
            _ => looped,
        };
        let time = super::CompTime {
            trim: l.trim_start.as_ref().map(|t| self.time_raw(t, &format!("{path}.trimStart"))).unwrap_or(0.0),
            speed: l.speed.unwrap_or(1.0) as f32,
            looped,
        };
        let nat = [ev.width as f32, ev.height as f32];
        Some((Kind::Comp { ev, natural: nat, fit, time }, nat))
    }

    /// Decode a clip's frames for the layer's lifetime (once, via ffmpeg) into the shared image store.
    fn video_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>, life: f32, start: f32) -> (Kind, [f32; 2]) {
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
        let empty = |nat: [f32; 2]| (Kind::Video { frames: Vec::new(), natural: nat, fit, rate: 1.0, speed: 1.0, looped: false }, nat);
        if !full.exists() {
            self.d.push(Diagnostic::error(format!("{path}.src"), format!("file not found: {}", full.display())));
            return empty([gw.unwrap_or(640.0), gh.unwrap_or(360.0)]);
        }
        // Probe size, duration and whether it has audio.
        let probe = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-show_entries", "stream=codec_type,width,height:format=duration", "-of", "json"])
            .arg(&full)
            .output();
        let info: serde_json::Value = match probe {
            Ok(o) if o.status.success() => serde_json::from_slice(&o.stdout).unwrap_or_default(),
            Ok(o) => {
                self.d.push(Diagnostic::error(format!("{path}.src"), format!("can't read video: {}", String::from_utf8_lossy(&o.stderr).trim())));
                return empty([gw.unwrap_or(640.0), gh.unwrap_or(360.0)]);
            }
            Err(e) => {
                self.d.push(Diagnostic::error(format!("{path}.src"), format!("video layers need ffprobe/ffmpeg: {e}")));
                return empty([gw.unwrap_or(640.0), gh.unwrap_or(360.0)]);
            }
        };
        let streams = info["streams"].as_array().cloned().unwrap_or_default();
        let v = streams.iter().find(|s| s["codec_type"] == "video");
        let has_audio = streams.iter().any(|s| s["codec_type"] == "audio");
        let (nw, nh) = match v {
            Some(v) => (v["width"].as_f64().unwrap_or(640.0) as f32, v["height"].as_f64().unwrap_or(360.0) as f32),
            None => {
                self.d.push(Diagnostic::error(format!("{path}.src"), "no video stream in this file"));
                return empty([gw.unwrap_or(640.0), gh.unwrap_or(360.0)]);
            }
        };
        let clip_dur = info["format"]["duration"].as_str().and_then(|d| d.parse::<f32>().ok()).unwrap_or(life);
        let nat = [nw, nh];
        let speed = l.speed.unwrap_or(1.0).max(0.01) as f32;
        let looped = matches!(l.loop_, Some(spec::LoopValue::Clip(true)));
        let trim = l.trim_start.as_ref().map(|t| self.time_raw(t, &format!("{path}.trimStart"))).unwrap_or(0.0).max(0.0);
        // Audio from the clip.
        if let Some(vol) = l.volume.filter(|v| *v > 0.0) {
            if has_audio {
                self.audio_extra.push(crate::audio::Track {
                    src: crate::audio::Source::File(full.to_string_lossy().into_owned()),
                    start: (self.scene_start + start) as f64,
                    trim: Some([trim as f64, 0.0]),
                    volume: vol,
                    looped,
                    duration: Some(life as f64),
                    ..Default::default()
                });
            } else {
                self.d.push(Diagnostic::warning(format!("{path}.volume"), "this clip has no audio track"));
            }
        }
        if !self.o.load_assets {
            return empty(nat);
        }
        // Decode size: the layer's box (or natural size), at most the frame, at most ~1 GB for the clip.
        let rate = self.fps.min(60.0);
        let span = if looped { (clip_dur - trim).max(0.04) } else { (life * speed).min((clip_dur - trim).max(0.04)) };
        let count = (span * rate).ceil().max(1.0);
        let bw = gw.unwrap_or(nw).min(self.w).max(2.0);
        let bh = gh.unwrap_or(nh).min(self.h).max(2.0);
        let mut k = (bw / nw).max(bh / nh).min(1.0);
        let budget = 1.0e9f32;
        let bytes = |k: f32| nw * k * nh * k * 4.0 * count;
        if bytes(k) > budget {
            k *= (budget / bytes(k)).sqrt();
            self.d.push(Diagnostic::warning(path, format!("long clip: decoded at {:.0}% resolution to fit in memory", k * 100.0)));
        }
        let (dw, dh) = (((nw * k).round() as u32).max(2) & !1, ((nh * k).round() as u32).max(2) & !1);
        let out = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-ss", &format!("{trim:.4}"), "-t", &format!("{span:.4}"), "-i"])
            .arg(&full)
            .args(["-vf", &format!("fps={rate},scale={dw}:{dh}:flags=bicubic"), "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .output();
        let px = match out {
            Ok(o) if o.status.success() => o.stdout,
            Ok(o) => {
                self.d.push(Diagnostic::error(format!("{path}.src"), format!("can't decode video: {}", String::from_utf8_lossy(&o.stderr).trim())));
                return empty(nat);
            }
            Err(e) => {
                self.d.push(Diagnostic::error(format!("{path}.src"), format!("video layers need ffmpeg: {e}")));
                return empty(nat);
            }
        };
        let fsz = (dw * dh * 4) as usize;
        let mut frames = Vec::new();
        for chunk in px.chunks_exact(fsz) {
            match self.assets.add_rgba(dw, dh, chunk.to_vec()) {
                Ok(id) => frames.push(id),
                Err(e) => {
                    self.d.push(Diagnostic::error(format!("{path}.src"), e));
                    break;
                }
            }
        }
        if frames.is_empty() {
            self.d.push(Diagnostic::error(format!("{path}.src"), "no frames decoded (is trimStart past the end?)"));
        }
        (Kind::Video { frames, natural: nat, fit, rate, speed, looped }, nat)
    }

    /// Where an image `src` lives: library refs (`polyhaven:texture/…`,
    /// `ambientcg:…`) are fetched once (the albedo map of a texture set);
    /// anything else is relative to the spec. A failed fetch is reported here
    /// and yields a path that doesn't exist (so callers skip the image).
    fn media_path(&mut self, src: &str, path: &str) -> std::path::PathBuf {
        if !crate::assetlib::is_ref(src) {
            return self.o.base_dir.join(src);
        }
        // An HDRI in a 2D layer is a photo: a rectilinear view of its panorama.
        let hdri = crate::assetlib::find(src).is_some_and(|e| e.kind == "hdri") || src.starts_with("polyhaven:hdri/");
        if hdri {
            return match crate::assetlib::photo_view(crate::assetlib::global(), src) {
                Ok(p) => p,
                Err(e) => {
                    self.d.push(Diagnostic::error(format!("{path}.src"), e));
                    std::path::PathBuf::from("\0missing-asset")
                }
            };
        }
        match crate::assetlib::get(src) {
            Ok(f) => f.main(),
            Err(e) => {
                self.d.push(Diagnostic::error(format!("{path}.src"), e));
                std::path::PathBuf::from("\0missing-asset")
            }
        }
    }

    /// Load an image file referenced by a component and register it under its `src`.
    fn load_media(&mut self, src: &str, path: &str) {
        let full = self.media_path(src, path);
        if self.image_ids.contains_key(&full) || !self.o.load_assets {
            if let Some(&(id, _)) = self.image_ids.get(&full) {
                self.assets.add_media(src, ImageId(id));
            }
            return;
        }
        if !full.exists() {
            self.d.push(Diagnostic::warning(path.to_string(), format!("image not found: {}", full.display())));
            return;
        }
        match image::open(&full).map_err(|e| e.to_string()).and_then(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            self.assets.add_rgba(w, h, rgba.into_raw()).map(|id| (id, [w as f32, h as f32]))
        }) {
            Ok((id, nat)) => {
                self.assets.add_media(src, id);
                self.image_ids.insert(full, (id.0, nat));
            }
            Err(e) => self.d.push(Diagnostic::warning(path.to_string(), format!("can't decode image {src}: {e}"))),
        }
    }

    // ---------- shader layers (SHADER module; runtime in `eval/shader.rs`) ----------

    /// JIT every shader the spec uses in parallel, before compiling layers in
    /// order: the layer pass then hits `Shader::compile_shared`'s cache, so
    /// time to first frame is the slowest shader, not the sum.
    fn prewarm_shaders(&mut self, spec: &Spec) {
        fn walk<'a>(ls: &'a [Layer], out: &mut Vec<&'a Layer>) {
            for l in ls {
                if l.shader.is_some() || l.src.as_deref().is_some_and(|s| s.ends_with(".glsl") || s.ends_with(".wgsl") || s.ends_with(".frag")) {
                    out.push(l);
                }
                walk(l.layers.as_deref().unwrap_or(&[]), out);
                walk(l.children.as_deref().unwrap_or(&[]), out);
            }
        }
        let mut layers = Vec::new();
        let mut trans: Vec<&spec::TransitionObj> = Vec::new();
        for sc in &spec.scenes {
            walk(&sc.layers, &mut layers);
            if let Some(t) = &sc.transition {
                trans.push(&t.0);
            }
        }
        for c in spec.compositions.iter().flat_map(|m| m.values()) {
            walk(c.layers.as_deref().unwrap_or(&[]), &mut layers);
        }
        if layers.is_empty() && trans.is_empty() {
            return;
        }
        // Resolve sources and uniform types exactly as the layer pass will
        // (so the cache keys match), without reporting anything twice.
        let saved = std::mem::take(&mut self.d);
        let mut jobs: Vec<(String, Vec<(String, anikit_shader::UType)>)> = Vec::new();
        for l in layers {
            if let Some(j) = self.shader_source(l.shader.as_deref(), l.src.as_deref(), l.uniforms.as_ref(), "") {
                jobs.push(j);
            }
        }
        for t in trans {
            let kind = t.kind.as_deref().unwrap_or("");
            let named = anikit_shader::library::get(kind).is_some_and(|e| e.use_ == anikit_shader::library::Use::Transition);
            if kind == "shader" || named {
                let src = t.shader.clone().unwrap_or_else(|| kind.to_string());
                if let Some(j) = self.shader_source(Some(&src), None, t.uniforms.as_ref(), "") {
                    jobs.push(j);
                }
            }
        }
        self.d = saved;
        use rayon::prelude::*;
        jobs.par_iter().for_each(|(text, extra)| {
            let ex: Vec<(&str, anikit_shader::UType)> = extra.iter().map(|(n, t)| (n.as_str(), *t)).collect();
            let _ = anikit_shader::Shader::compile_shared(text, &ex);
        });
    }

    /// Source text and spec-declared uniform types of a shader (no compile).
    fn shader_source(&mut self, src_field: Option<&str>, file: Option<&str>, unis: Option<&BTreeMap<String, serde_json::Value>>, path: &str) -> Option<(String, Vec<(String, anikit_shader::UType)>)> {
        let text = match (src_field, file) {
            (Some(s), _) => super::shader::resolve_source(s)?.to_string(),
            (None, Some(f)) => std::fs::read_to_string(self.o.base_dir.join(f)).ok()?,
            _ => return None,
        };
        let mut extra = Vec::new();
        for (k, v) in unis.into_iter().flatten() {
            if let Some(x) = self.uniform_value(v, &format!("{path}.uniforms.{k}")) {
                extra.push((k.clone(), match x {
                    Ok(x) if x.len() > 1 => anikit_shader::UType::Vec(x.len() as u8),
                    _ => anikit_shader::UType::Float,
                }));
            }
        }
        Some((text, extra))
    }

    /// Spec uniform value → floats (or an expression string).
    fn uniform_value(&mut self, v: &serde_json::Value, path: &str) -> Option<Result<Vec<f32>, String>> {
        Some(Ok(match v {
            serde_json::Value::Number(x) => vec![x.as_f64().unwrap_or(0.0) as f32],
            serde_json::Value::Bool(b) => vec![*b as u8 as f32],
            serde_json::Value::Array(a) => a.iter().take(4).map(|x| x.as_f64().unwrap_or(0.0) as f32).collect(),
            serde_json::Value::String(s) if s.trim_start().starts_with('=') => return Some(Err(s.clone())),
            serde_json::Value::String(s) => {
                let c = self.color(s, path)?;
                if c.0[3] < 1.0 {
                    c.0.to_vec()
                } else {
                    c.0[..3].to_vec()
                }
            }
            _ => {
                self.d.push(Diagnostic::error(path, "a uniform is a number, [x, y(, z, w)], a colour or an \"=expression\""));
                return None;
            }
        }))
    }

    fn compile_shader(&mut self, src_field: Option<&str>, file: Option<&str>, unis: Option<&BTreeMap<String, serde_json::Value>>, path: &str) -> Option<(Arc<anikit_shader::Shader>, String, Vec<(String, Result<Vec<f32>, String>)>)> {
        use anikit_shader::UType;
        let text = match (src_field, file) {
            (Some(s), _) => match super::shader::resolve_source(s) {
                Some(t) => t.to_string(),
                None => {
                    let names = anikit_shader::library::names();
                    self.d.push(Diagnostic::error(format!("{path}.shader"), format!("unknown built-in shader '{s}' (or write GLSL with a mainImage function)")).suggest(suggest(s.trim(), &names)));
                    return None;
                }
            },
            (None, Some(f)) => match std::fs::read_to_string(self.o.base_dir.join(f)) {
                Ok(t) => t,
                Err(e) => {
                    self.d.push(Diagnostic::error(format!("{path}.src"), format!("can't read shader {f}: {e}")));
                    return None;
                }
            },
            _ => return None,
        };
        let mut vals = Vec::new();
        for (k, v) in unis.into_iter().flatten() {
            if let Some(x) = self.uniform_value(v, &format!("{path}.uniforms.{k}")) {
                vals.push((k.clone(), x));
            }
        }
        let extra: Vec<(&str, UType)> = vals
            .iter()
            .map(|(k, v)| (k.as_str(), match v {
                Ok(x) if x.len() > 1 => UType::Vec(x.len() as u8),
                _ => UType::Float,
            }))
            .collect();
        match anikit_shader::Shader::compile_shared(&text, &extra) {
            Ok(s) => Some((s, text, vals)),
            Err(e) => {
                let field = if src_field.is_some() { "shader" } else { "src" };
                self.d.push(Diagnostic::error(format!("{path}.{field}"), format!("shader: {e}")));
                None
            }
        }
    }

    fn shader_id(path: &str, text: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut h);
        text.hash(&mut h);
        h.finish()
    }

    fn channel_opts(&mut self, v: &serde_json::Value, path: &str) -> super::shader::ChanOpts {
        use anikit_shader::{Filter, Wrap};
        let wrap = match v.get("wrap").and_then(|w| w.as_str()) {
            None | Some("clamp") => Wrap::Clamp,
            Some("repeat") => Wrap::Repeat,
            Some("mirror") => Wrap::Mirror,
            Some(w) => {
                self.d.push(Diagnostic::error(format!("{path}.wrap"), format!("unknown wrap '{w}'")).suggest(suggest(w, &["clamp", "repeat", "mirror"])));
                Wrap::Clamp
            }
        };
        let filter = match v.get("filter").and_then(|w| w.as_str()) {
            Some("nearest") => Filter::Nearest,
            _ => Filter::Linear,
        };
        let mipmaps = v.get("mipmaps").and_then(|m| m.as_bool()).unwrap_or(false);
        super::shader::ChanOpts { wrap, filter, mipmaps }
    }

    #[allow(clippy::too_many_arguments)]
    fn shader_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>, start: f32, end: f32, n: &mut Node) -> Option<(Kind, [f32; 2])> {
        // A single-file multi-pass source (`//--- buffer A: …` / `//--- image: …`
        // sections) becomes buffers + the image pass, unless the spec sets them.
        let raw = match (l.shader.as_deref(), l.src.as_deref()) {
            (Some(s), _) => super::shader::resolve_source(s).map(str::to_string),
            (None, Some(f)) if !f.ends_with(".wgsl") => std::fs::read_to_string(self.o.base_dir.join(f)).ok(),
            _ => None,
        };
        if let Some(sections) = raw.as_deref().and_then(anikit_shader::split_passes) {
            let mut l2 = l.clone();
            l2.src = None;
            l2.shader = Some(sections.image);
            if l2.channels.is_none() {
                l2.channels = Some(sections.image_channels.into_iter().map(serde_json::Value::String).collect());
            }
            if l2.buffers.is_none() {
                l2.buffers = Some(
                    sections
                        .buffers
                        .into_iter()
                        .map(|(name, src, ch)| (name, crate::spec::ShaderBuffer { shader: Some(src), src: None, channels: Some(ch.into_iter().map(serde_json::Value::String).collect()) }))
                        .collect(),
                );
            }
            return self.shader_layer_inner(&l2, path, gw, gh, start, end, n);
        }
        self.shader_layer_inner(l, path, gw, gh, start, end, n)
    }

    #[allow(clippy::too_many_arguments)]
    fn shader_layer_inner(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>, start: f32, end: f32, n: &mut Node) -> Option<(Kind, [f32; 2])> {
        use super::shader::{ChannelC, ShaderC, UNIFORM_SLOTS};
        let (shader, text, vals) = self.compile_shader(l.shader.as_deref(), l.src.as_deref(), l.uniforms.as_ref(), path)?;
        if shader.kind != anikit_shader::Kind::Image {
            self.d.push(Diagnostic::error(format!("{path}.shader"), "this is a transition shader; use it as a scene transition: {\"transition\": {\"type\": \"shader\", \"shader\": …}}"));
            return None;
        }
        let size = [gw.unwrap_or(self.w), gh.unwrap_or(self.h)];
        // Uniform slots: declared defaults, then spec values (numbers or expressions).
        let mut slots = Vec::new();
        let mut fixed = Vec::new();
        let mut off = 0;
        // Animation slots go first to uniforms that need them: given as an
        // expression, or named by the layer's keyframes/animations; the rest
        // follow in declaration order. Static values past the slots are fixed.
        let anim_keys = {
            let mut v = serde_json::to_value(l).unwrap_or_default();
            if let Some(o) = v.as_object_mut() {
                for k in ["uniforms", "shader", "src", "buffers"] {
                    o.remove(k);
                }
            }
            v.to_string()
        };
        let animated = |name: &str| vals.iter().any(|(k, v)| k == name && v.is_err()) || anim_keys.contains(&format!("\"{name}\":"));
        let mut order: Vec<&anikit_shader::DeclaredUniform> = shader.uniforms.iter().collect();
        order.sort_by_key(|u| !animated(&u.name));
        for u in order {
            let len = u.ty.components();
            let given = vals.iter().find(|(k, _)| *k == u.name).map(|(_, v)| v.clone());
            if off + len > UNIFORM_SLOTS {
                if let Some(Ok(v)) = given.clone().or(u.default.clone().map(Ok)) {
                    fixed.push((u.name.clone(), v));
                }
                if animated(&u.name) {
                    self.d.push(Diagnostic::warning(format!("{path}.uniforms.{}", u.name), format!("more than {UNIFORM_SLOTS} animated uniform floats: '{}' can't be animated (it keeps its first value)", u.name)));
                }
                continue;
            }
            if let Some(d) = &u.default {
                for (i, x) in d.iter().take(len).enumerate() {
                    n.base[prop::U0 + off + i] = *x;
                }
            }
            match given {
                Some(Ok(v)) => {
                    for (i, x) in v.iter().take(len).enumerate() {
                        n.base[prop::U0 + off + i] = *x;
                    }
                }
                Some(Err(e)) => self.set(n, prop::U0 + off, Some(&Num::Str(e)), &format!("{path}.uniforms.{}", u.name), 1.0),
                None => {}
            }
            slots.push((u.name.clone(), off, len));
            off += len;
        }
        let (passes, buf_names) = self.shader_buffers(l, path, &vals);
        let filter = l.filter.unwrap_or(false);
        let mut channels = Vec::new();
        let list: Vec<serde_json::Value> = match &l.channels {
            Some(c) => c.clone(),
            None if filter => vec![serde_json::Value::String("below".into())],
            None => Vec::new(),
        };
        if list.len() > 4 {
            self.d.push(Diagnostic::error(format!("{path}.channels"), "at most 4 channels (iChannel0..3)"));
        }
        for (i, v) in list.iter().take(4).enumerate() {
            let cp = format!("{path}.channels[{i}]");
            let o = self.channel_opts(v, &cp);
            let src = v.as_str().or_else(|| v.get("src").and_then(|s| s.as_str()));
            let comp_name = src.and_then(|x| x.strip_prefix('#')).or_else(|| v.get("composition").and_then(|x| x.as_str()));
            let c = match (src, v.get("layers")) {
                (Some(b), _) if buf_names.iter().any(|n| n == b) => ChannelC::Buffer(buf_slot(b)),
                (Some(b), _) if buf_slot_opt(b).is_some() => {
                    self.d.push(Diagnostic::error(cp.clone(), format!("channel '{b}' reads buffer {b}, but the layer has no \"buffers\": {{\"{b}\": {{…}}}}")));
                    ChannelC::None
                }
                _ if comp_name.is_some() => {
                    let name = comp_name.unwrap_or_default();
                    match self.comps.iter().find(|c| c.0 == name).cloned() {
                        Some((_, ev, looped)) => {
                            let time = super::CompTime {
                                trim: v.get("trimStart").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                                speed: v.get("speed").and_then(|x| x.as_f64()).unwrap_or(1.0) as f32,
                                looped: v.get("loop").and_then(|x| x.as_bool()).unwrap_or(looped),
                            };
                            ChannelC::Comp { ev, time }
                        }
                        None => {
                            let names: Vec<&str> = self.comps.iter().map(|c| c.0.as_str()).collect();
                            self.d.push(Diagnostic::error(cp.clone(), format!("unknown composition '{name}' (define it in top-level 'compositions')")).suggest(suggest(name, &names)));
                            ChannelC::None
                        }
                    }
                }
                (Some("below"), _) => ChannelC::Below,
                (Some("noise"), _) => ChannelC::Noise,
                (Some("") | Some("none"), _) => ChannelC::None,
                (Some(file), _) => {
                    // An image or video, stretched over the layer (uv 0..1 covers it).
                    let synth: Layer = serde_json::from_value(serde_json::json!({ "src": file, "width": "100%", "height": "100%", "fit": "fill", "x": "50%", "y": "50%" })).unwrap_or_default();
                    let pc = PCtx { size, t0: start, end, delay: 0.0, index: 0, count: 1, in_box: false };
                    let saved = std::mem::take(&mut self.uniform_slots);
                    let node = self.layer(&synth, &cp, pc);
                    self.uniform_slots = saved;
                    ChannelC::Layers(node.into_iter().collect())
                }
                (None, Some(ls)) => {
                    let g: Layer = match serde_json::from_value(serde_json::json!({ "layers": ls })) {
                        Ok(g) => g,
                        Err(e) => {
                            self.d.push(Diagnostic::error(format!("{cp}.layers"), e.to_string()));
                            Layer::default()
                        }
                    };
                    let pc = PCtx { size, t0: start, end, delay: 0.0, index: 0, count: 0, in_box: false };
                    let saved = std::mem::take(&mut self.uniform_slots);
                    let kids = self.layers(g.layers.as_deref().unwrap_or(&[]), &format!("{cp}.layers"), pc, None);
                    self.uniform_slots = saved;
                    ChannelC::Layers(kids)
                }
                _ => {
                    self.d.push(Diagnostic::error(cp, "a channel is \"below\", \"noise\", \"#composition\", an image/video path or {\"layers\": [...]}"));
                    ChannelC::None
                }
            };
            channels.push((c, o));
        }
        for (i, used) in shader.channels_used.iter().enumerate() {
            if *used && channels.get(i).is_none_or(|(c, _)| matches!(c, ChannelC::None)) {
                self.d.push(Diagnostic::warning(format!("{path}.channels"), format!("the shader samples iChannel{i} but no channel {i} is given (it reads as black)")));
            }
        }
        self.uniform_slots = slots.clone();
        let sc = ShaderC {
            dynamic: super::shader::uses_time(&text) || passes.as_ref().is_some_and(|p| p.stateful || p.dynamic),
            id: Self::shader_id(path, &text),
            shader,
            uniforms: slots,
            fixed,
            channels,
            filter,
            // Anti-aliased by default (adaptive 2×2 where neighbours differ), except
            // filters, which would soften the frame they read, and sources that
            // opt out with `// anikit:aa off`.
            supersample: l.supersample.unwrap_or(!filter && !anikit_shader::aa_off(&text)),
            alpha: l.alpha.unwrap_or(false),
            label: super::shader::label(path, l.shader.as_deref().or(l.src.as_deref())),
            audio: self.shader_audio(&text, path),
            faithful: passes.is_none() && !text.contains("texelFetch"),
            passes,
        };
        Some((Kind::Shader(Box::new(sc)), size))
    }

    /// Compile a shader layer's `"buffers"` (A–D). Returns the passes and the names given.
    fn shader_buffers(&mut self, l: &Layer, path: &str, vals: &[(String, Result<Vec<f32>, String>)]) -> (Option<Arc<super::shader::Passes>>, Vec<String>) {
        use super::shader::{BufChan, BufPass, Passes};
        let Some(map) = &l.buffers else { return (None, Vec::new()) };
        let mut bufs = Vec::new();
        let mut names = Vec::new();
        let mut dynamic = false;
        let mut audio = None;
        let mut uniforms: Vec<(String, Vec<f32>)> = Vec::new();
        for (name, b) in map {
            let bp = format!("{path}.buffers.{name}");
            let Some(slot) = buf_slot_opt(name) else {
                self.d.push(Diagnostic::error(bp, "buffers are named \"A\", \"B\", \"C\" or \"D\""));
                continue;
            };
            let Some((shader, text, bvals)) = self.compile_shader(b.shader.as_deref(), b.src.as_deref(), l.uniforms.as_ref(), &bp) else { continue };
            if shader.kind != anikit_shader::Kind::Image {
                self.d.push(Diagnostic::error(format!("{bp}.shader"), "a buffer pass is an image shader (mainImage)"));
                continue;
            }
            dynamic |= super::shader::uses_time(&text);
            if audio.is_none() {
                audio = self.shader_audio(&text, &bp);
            }
            // Static uniform values: declared defaults, then the spec's numbers.
            for u in &shader.uniforms {
                let v = bvals.iter().chain(vals.iter()).find(|(k, _)| *k == u.name).and_then(|(_, v)| v.clone().ok()).or_else(|| u.default.clone());
                if let Some(v) = v {
                    if !uniforms.iter().any(|(n, _)| *n == u.name) {
                        uniforms.push((u.name.clone(), v));
                    }
                }
            }
            let mut chans = Vec::new();
            for (i, v) in b.channels.iter().flatten().take(4).enumerate() {
                let cp = format!("{bp}.channels[{i}]");
                let o = self.channel_opts(v, &cp);
                let src = v.as_str().or_else(|| v.get("src").and_then(|s| s.as_str())).unwrap_or("");
                let c = match src {
                    "noise" => BufChan::Noise,
                    "" | "none" => BufChan::None,
                    s if buf_slot_opt(s).is_some() && map.contains_key(s) => BufChan::Buffer(buf_slot(s)),
                    s => {
                        self.d.push(Diagnostic::error(cp, format!("a buffer channel is \"A\"…\"D\" (a buffer of this layer), \"noise\" or \"none\", not '{s}'")));
                        BufChan::None
                    }
                };
                chans.push((c, o));
            }
            names.push(name.clone());
            bufs.push(BufPass { slot, shader, chans });
        }
        if bufs.is_empty() {
            return (None, names);
        }
        let mut p = Passes::new(bufs, uniforms, audio);
        p.dynamic = dynamic;
        (Some(Arc::new(p)), names)
    }

    /// Music features for a shader that reads `iAudio`/`iBeat` (analysed once per spec).
    fn shader_audio(&mut self, text: &str, path: &str) -> Option<Arc<super::audio_react::AudioFeatures>> {
        if !text.contains("iAudio") && !text.contains("iBeat") {
            return None;
        }
        if self.music.is_none() {
            // Generated `music` first (its exact grid, the same one expressions use),
            // then the first music `audio` track.
            let r = match (self.beat_marks.generated(), self.beat_marks.music()) {
                (Some(g), _) => g.map(|(mono, rate, beats, bars, at)| Arc::new(super::audio_react::AudioFeatures::from_mono(&mono, rate, beats, bars, at))).map_err(|e| format!("iAudio/iBeat: {e}")),
                (None, None) => Err("iAudio/iBeat need music: add generated `music` or a top-level `audio` track with a `src` (they stay 0)".to_string()),
                (None, Some((p, at, trim))) => super::audio_react::AudioFeatures::load(p, *at, *trim).map(Arc::new).map_err(|e| format!("iAudio/iBeat: {e}")),
            };
            self.music = Some(r);
        }
        match self.music.as_ref()? {
            Ok(a) => Some(a.clone()),
            Err(e) => {
                self.d.push(Diagnostic::warning(format!("{path}.shader"), e.clone()));
                None
            }
        }
    }

    fn shader_transition(&mut self, src: &str, unis: Option<&BTreeMap<String, serde_json::Value>>, path: &str) -> Option<super::shader::ShaderRef> {
        use super::shader::{ChanOpts, ChannelC, ShaderC, ShaderRef};
        let (shader, text, vals) = self.compile_shader(Some(src), None, unis, path)?;
        if shader.kind != anikit_shader::Kind::Transition {
            self.d.push(Diagnostic::error(format!("{path}.shader"), "a transition shader defines vec4 transition(vec2 uv) using getFromColor(uv), getToColor(uv) and progress"));
            return None;
        }
        let mut fixed: Vec<(String, Vec<f32>)> = Vec::new();
        for (k, v) in vals {
            match v {
                Ok(v) => fixed.push((k, v)),
                Err(_) => self.d.push(Diagnostic::error(format!("{path}.uniforms.{k}"), "transition uniforms can't be expressions")),
            }
        }
        let o = ChanOpts { wrap: anikit_shader::Wrap::Clamp, filter: anikit_shader::Filter::Linear, mipmaps: false };
        let mut channels = vec![(ChannelC::Below, o), (ChannelC::Below, o)];
        if shader.channels_used[2] {
            channels.push((ChannelC::Noise, ChanOpts { wrap: anikit_shader::Wrap::Repeat, ..o }));
        }
        Some(ShaderRef(Arc::new(ShaderC {
            dynamic: true,
            id: Self::shader_id(path, &text),
            shader,
            uniforms: Vec::new(),
            fixed,
            channels,
            filter: false,
            supersample: false,
            alpha: false,
            label: super::shader::label(path, Some(src)),
            audio: self.shader_audio(&text, path),
            passes: None,
            faithful: !text.contains("texelFetch"),
        })))
    }

    fn component_layer(&mut self, l: &Layer, path: &str, gw: Option<f32>, gh: Option<f32>) -> Option<(Kind, [f32; 2])> {
        let raw = l.component.clone().unwrap_or_default();
        // camelCase (`barChart`), kebab-case and short aliases (`chart.bar`) all resolve.
        let name = crate::components::canonical(&raw);
        let names: Vec<&str> = crate::components::KINDS.iter().map(|k| k.name).collect();
        if !names.contains(&name.as_str()) {
            self.d.push(Diagnostic::error(format!("{path}.component"), format!("unknown component '{raw}'")).suggest(suggest(&name, &names)));
            return None;
        }
        let mut props = l.props.clone().unwrap_or(serde_json::Value::Object(Default::default()));
        // `"$accent"`-style colour tokens resolve against the scene theme.
        let colors = &self.theme.colors;
        crate::components::resolve_tokens(&mut props, &|k| {
            colors.get(k).map(|c| {
                let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                format!("#{:02x}{:02x}{:02x}{:02x}", b(c.0[0]), b(c.0[1]), b(c.0[2]), b(c.0[3]))
            })
        });
        // The theme's material (surface, motion, glow…) fills props the layer leaves out.
        if let Ok(m) = serde_json::to_value(&self.theme.material) {
            props = crate::components::with_material(&name, &props, &m);
        }
        // Audio files a component reads itself (the visualizer's `src`) resolve
        // against the spec's folder, like every other asset; otherwise a relative
        // path only worked from that folder and the visualizer silently fell back
        // to its synthetic groove.
        if let Some(obj) = props.as_object_mut() {
            if let Some(serde_json::Value::String(src)) = obj.get("src") {
                let l = src.to_ascii_lowercase();
                let audio = [".mp3", ".wav", ".flac", ".ogg", ".m4a", ".aac", ".mp4"].iter().any(|e| l.ends_with(e));
                let p = std::path::Path::new(src);
                if audio && p.is_relative() {
                    let full = self.o.base_dir.join(p);
                    if full.exists() {
                        obj.insert("src".into(), serde_json::Value::String(full.to_string_lossy().into_owned()));
                    }
                }
            }
        }
        // Load the images the component refers to, so `Assets::media` finds them.
        for src in crate::components::media_srcs(&props) {
            self.load_media(&src, &format!("{path}.props"));
        }
        if name == "scene-3d" {
            for (sub, msg) in crate::scene3d::lint(&props) {
                self.d.push(Diagnostic::warning(format!("{path}.props{sub}"), msg));
            }
        }
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

    /// A shader uniform's animation slot (only while compiling that shader layer).
    fn uniform_lookup(&self, name: &str) -> Option<(usize, usize)> {
        self.uniform_slots.iter().find(|(n, _, _)| n == name).map(|(_, o, c)| (prop::U0 + o, *c))
    }

    fn anim_val(&mut self, name: &str, v: &serde_json::Value, path: &str, refs: [f32; 2]) -> Option<ValS> {
        let (slot, ch) = prop::lookup(name).or_else(|| self.uniform_lookup(name))?;
        if slot >= prop::U0 {
            return match v.as_f64() {
                Some(x) => Some(ValS::Abs(x as f32)),
                None => self.bad_val(path, &v.to_string()),
            };
        }
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
        self.d.push(Diagnostic::error(path, format!("invalid value {s} (use a number like 120, a unit string like \"50%\" or \"10vw\", a relative change \"+40\" / \"-40\" / \"*1.5\", or a colour for fill/stroke)")));
        None
    }

    fn props_map(&mut self, m: &BTreeMap<String, serde_json::Value>, path: &str, refs: [f32; 2]) -> Vec<(usize, usize, ValS)> {
        let mut out = Vec::new();
        for (k, v) in m {
            let p = format!("{path}.{k}");
            if k == "cell" {
                // Tween to (or from) a grid field: x, y, width (and height, except text).
                if let Some([x, y, w, h]) = self.cell_rect(v, &p, refs) {
                    let a = self.cur_anchor;
                    out.push((prop::X, 1, ValS::Abs(x + a[0] * w)));
                    out.push((prop::Y, 1, ValS::Abs(y + a[1] * h)));
                    out.push((prop::W, 1, ValS::Abs(w)));
                    if !self.cur_text {
                        out.push((prop::H, 1, ValS::Abs(h)));
                    }
                }
                continue;
            }
            if let (Some((slot, n)), Some(arr)) = (self.uniform_lookup(k), v.as_array()) {
                // A vector uniform: one track per component.
                for (i, x) in arr.iter().take(n).enumerate() {
                    if let Some(x) = self.anim_val(k, x, &format!("{p}[{i}]"), refs) {
                        out.push((slot + i, 1, x));
                    }
                }
                continue;
            }
            match prop::lookup(k).or_else(|| self.uniform_lookup(k)) {
                Some((slot, ch)) => {
                    if let Some(x) = self.anim_val(k, v, &p, refs) {
                        out.push((slot, ch, x));
                    }
                }
                None if !self.uniform_slots.is_empty() => {
                    // A shader layer: the candidates are its uniforms too.
                    let mut names: Vec<&str> = self.uniform_slots.iter().map(|u| u.0.as_str()).collect();
                    let list = names.join(", ");
                    names.extend_from_slice(val::ANIM_PROPS);
                    let msg = format!("'{k}' is not animatable (this shader's uniforms: {list})");
                    self.d.push(Diagnostic::error(p, msg).suggest(suggest(k, &names)));
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
                let Some((slot, ch)) = prop::lookup(name).or_else(|| self.uniform_lookup(name).map(|(s, _)| (s, 1))) else {
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
                    let (tt, vv, ke) = match v {
                        serde_json::Value::Array(a) if a.len() == 2 && a[0].is_number() => (a[0].as_f64().unwrap() as f32, &a[1], None),
                        // `[time, value, ease]`: the ease of the move into this key.
                        serde_json::Value::Array(a) if a.len() == 3 && a[0].is_number() => {
                            let e = match &a[2] {
                                serde_json::Value::String(e) => Some(self.ease(Some(e), "linear", &format!("{p}[{i}][2]"))),
                                serde_json::Value::Array(b) if b.len() == 4 && b.iter().all(|x| x.is_number()) => {
                                    let e = format!("cubic-bezier({},{},{},{})", b[0], b[1], b[2], b[3]);
                                    Some(self.ease(Some(&e), "linear", &format!("{p}[{i}][2]")))
                                }
                                _ => {
                                    self.d.push(Diagnostic::error(format!("{p}[{i}][2]"), "a keyframe's third item is its ease: a name or [x1, y1, x2, y2]"));
                                    None
                                }
                            };
                            (a[0].as_f64().unwrap() as f32, &a[1], e)
                        }
                        _ => (i as f32 / (n - 1) as f32, v, None),
                    };
                    if let Some(x) = self.anim_val(name, vv, &format!("{p}[{i}]"), refs) {
                        list.push((tt.clamp(0.0, 1.0), x, ke));
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

    /// A `from` holds its value until the tween starts, so a later `from` on a property an
    /// earlier move already animated snaps it back for the whole gap. Say so, with the fix.
    fn warn_from_overrides(&mut self, tw: &[TweenC], path: &str) {
        const NAMES: [&str; 18] = ["x", "y", "width", "height", "scale", "scaleY", "rotation", "skewX", "skewY", "opacity", "blur", "radius", "strokeWidth", "trimStart", "trimEnd", "fill", "stroke", "value"];
        let touches = |t: &TweenC, s: usize| t.from.iter().chain(&t.to).any(|f| f.0 == s) || t.kf.iter().any(|k| k.0 == s);
        let mut seen = Vec::new();
        for b in tw.iter().filter(|b| b.at > 1e-3) {
            for &(slot, _, _) in &b.from {
                if seen.contains(&slot) {
                    continue;
                }
                if let Some(a) = tw.iter().find(|a| a.at + 1e-3 < b.at && a.at + a.dur <= b.at + 1e-3 && touches(a, slot)) {
                    seen.push(slot);
                    let name = NAMES.iter().find(|n| prop::lookup(n).is_some_and(|(s, _)| s == slot)).copied().unwrap_or("a property");
                    self.d.push(Diagnostic::warning(
                        format!("{path}.animate"),
                        format!(
                            "'{name}': the 'from' at {:.2} s holds its start value from the beginning, undoing the move at {:.2}–{:.2} s; use 'to' (it continues from where the earlier move ended) or keyframes",
                            b.at,
                            a.at,
                            a.at + a.dur
                        ),
                    ));
                }
            }
        }
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
                    for &(tt, v, ke) in list {
                        let val = match v {
                            ValS::Color(col) => col[c],
                            v => apply(v, c0, 0),
                        };
                        if let Some((pt, pv)) = prev {
                            segs.push((s, Seg { t0: t0 + pt * t.dur, t1: t0 + tt * t.dur, v0: pv, v1: val, ease: ke.unwrap_or(t.ease), hold_before: false }));
                        } else if tt > 0.0 {
                            segs.push((s, Seg { t0, t1: t0 + tt * t.dur, v0: c0, v1: val, ease: ke.unwrap_or(t.ease), hold_before: false }));
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
        let name = val::canon_preset(p.preset.as_deref().unwrap_or_default(), exit).to_string();
        let list = if exit { val::EXIT_PRESETS } else { val::ENTER_PRESETS };
        if !list.contains(&name.as_str()) {
            let other = if exit { val::ENTER_PRESETS } else { val::EXIT_PRESETS };
            let hint = if other.contains(&name.as_str()) {
                format!(" ('{name}' is an {} preset)", if exit { "enter" } else { "exit" })
            } else {
                String::new()
            };
            let mut cands: Vec<&str> = list.to_vec();
            cands.extend(["rise", "drop", "fade", "pop", "grow", "zoom", "blur", "draw", "type", "count", "wipe", "slide", "bounce"]);
            if matches!(n.kind, Kind::Text { .. }) && !exit {
                cands.extend(crate::text::kinetic::STYLE_NAMES);
            }
            self.d.push(Diagnostic::error(path, format!("unknown {} preset '{name}'{hint}", if exit { "exit" } else { "enter" })).suggest(suggest(&name, &cands)));
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
            // Overshoot kept in the tasteful 3–8% range (docs/video/craft/motion-design.md); `back.out(1.7)` / `springy` go further.
            "pop" => (vec![(SX, 2, ValS::Abs(0.0)), (OPACITY, 1, ValS::Abs(0.0))], 0.5, "back.out(1.2)"),
            "scale" => (vec![(SX, 2, ValS::Mul(0.8)), (OPACITY, 1, ValS::Abs(0.0))], 0.5, "outCubic"),
            "zoom" => (vec![(SX, 2, ValS::Mul(1.4)), (OPACITY, 1, ValS::Abs(0.0)), (BLUR, 1, ValS::Abs(20.0))], 0.7, "outExpo"),
            "blur" => (vec![(BLUR, 1, ValS::Abs(30.0)), (OPACITY, 1, ValS::Abs(0.0))], 0.6, "outCubic"),
            "rotate" => (vec![(ROT, 1, ValS::Add(-90.0)), (SX, 2, ValS::Mul(0.6)), (OPACITY, 1, ValS::Abs(0.0))], 0.6, "back.out(1.2)"),
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

    /// The spec's `sound` and layers' `sfx`, compiled (`None` when neither is used).
    fn sound_plan(&mut self, spec: &Spec, duration: f32) -> Option<Arc<crate::audio::design::spec::SoundPlan>> {
        use crate::audio::design::spec::{LayerSfx, SoundPlan};
        if spec.sound.is_none() && self.layer_sfx.is_empty() && self.design_cues.is_empty() {
            return None;
        }
        let mut plan = match SoundPlan::new(spec.sound.as_ref(), spec.music.as_ref()) {
            Ok(p) => p,
            Err(e) => {
                self.d.push(Diagnostic::error("sound", e));
                return None;
            }
        };
        if spec.sound.is_none() {
            // Only `audio[].sfx` / layer `sfx` asked for sound design: keep the
            // mix as it was (no loudness change, no shared space).
            plan.target_lufs = None;
            plan.space = None;
        }
        plan.extra = std::mem::take(&mut self.design_cues);
        for (tag, v, path) in std::mem::take(&mut self.layer_sfx) {
            match LayerSfx::parse(&v) {
                Ok(l) => plan.layers.push((tag, l)),
                Err(e) => self.d.push(Diagnostic::error(format!("{path}.sfx"), e)),
            }
        }
        for (i, c) in plan.spec.cues.clone().into_iter().enumerate() {
            let path = format!("sound.cues[{i}].at");
            let at = match serde_json::from_value::<Num>(c.at.clone()) {
                Ok(n) => self.time_raw(&n, &path),
                Err(_) => {
                    self.d.push(Diagnostic::error(path, "seconds or a marker like \"@beat9\""));
                    continue;
                }
            };
            if at > duration + 0.5 {
                self.d.push(Diagnostic::warning(format!("sound.cues[{i}].at"), format!("{at:.2}s is after the end of the video ({duration:.2}s)")));
            }
            plan.cues.push((c, at as f64));
        }
        Some(Arc::new(plan))
    }

    fn audio(&mut self, a: &spec::AudioCue, path: &str, scene_start: f32) -> Option<crate::audio::Track> {
        use crate::audio::{Duck, Role, SfxParams, Source, SFX_NAMES};
        let src = match (&a.src, &a.sfx) {
            (Some(f), None) => {
                let full = self.o.base_dir.join(f);
                if !full.exists() {
                    self.d.push(Diagnostic::error(format!("{path}.src"), format!("file not found: {}", full.display())));
                }
                Source::File(full.to_string_lossy().into_owned())
            }
            (None, Some(n)) => {
                if !SFX_NAMES.contains(&n.as_str()) {
                    // Sound-design names (foley, cinematic, packs, layers): the hit lands on `at`.
                    if crate::audio::design::is_known(n) {
                        let at = scene_start + a.at.as_ref().map(|x| self.time_rel(x, &format!("{path}.at"), 0.0)).unwrap_or(0.0);
                        let mut c = crate::audio::design::Cue::new(n, at as f64);
                        c.params.duration = a.duration.as_ref().map(|x| self.time_raw(x, &format!("{path}.duration")));
                        c.params.pitch = 12.0 * (a.pitch.unwrap_or(1.0).max(0.01) as f32).log2();
                        c.params.seed = 1 + self.design_cues.len() as u64;
                        c.gain_db = match &a.volume {
                            Some(Num::Num(v)) => 20.0 * (v.max(1e-4) as f32).log10(),
                            Some(Num::Str(s)) => s.trim().trim_end_matches("dB").trim_end_matches("db").trim().parse().unwrap_or(0.0),
                            None => 0.0,
                        };
                        c.why = path.to_string();
                        self.design_cues.push(c);
                        return None;
                    }
                    let names = crate::audio::design::all_names();
                    self.d.push(Diagnostic::error(format!("{path}.sfx"), format!("unknown sound effect '{n}'")).suggest(suggest(n, &names)));
                    return None;
                }
                let duration = a.duration.as_ref().map(|x| self.time_raw(x, &format!("{path}.duration")) as f64);
                Source::Sfx { name: n.clone(), params: SfxParams { duration, pitch: a.pitch.unwrap_or(1.0), seed: 0 } }
            }
            (Some(_), Some(_)) => {
                self.d.push(Diagnostic::error(path, "set either 'src' or 'sfx', not both"));
                return None;
            }
            (None, None) => {
                self.d.push(Diagnostic::error(path, "an audio cue needs 'src' (a file) or 'sfx' (a built-in effect)"));
                return None;
            }
        };
        let at = scene_start + a.at.as_ref().map(|x| self.time_rel(x, &format!("{path}.at"), 0.0)).unwrap_or(0.0);
        let volume = match &a.volume {
            None => 1.0,
            Some(Num::Num(v)) => *v,
            Some(Num::Str(s)) => match s.trim().strip_suffix("dB").or_else(|| s.trim().strip_suffix("db")).and_then(|x| x.trim().parse::<f64>().ok()) {
                Some(db) => crate::audio::db_to_gain(db),
                None => {
                    self.d.push(Diagnostic::error(format!("{path}.volume"), format!("invalid volume '{s}' (0..1 or \"-6dB\")")));
                    1.0
                }
            },
        };
        let trim_start = a.trim_start.as_ref().map(|x| self.time_raw(x, &format!("{path}.trimStart")) as f64);
        let duration = match &a.sfx {
            None => a.duration.as_ref().map(|x| self.time_raw(x, &format!("{path}.duration")) as f64),
            Some(_) => None,
        };
        let duck = match &a.duck {
            None | Some(serde_json::Value::Bool(false)) => None,
            Some(serde_json::Value::Bool(true)) => Some(Duck::default()),
            Some(serde_json::Value::Number(n)) => Some(Duck { amount_db: n.as_f64().unwrap_or(12.0).abs(), ..Default::default() }),
            Some(v) => {
                self.d.push(Diagnostic::error(format!("{path}.duck"), format!("expected true or a reduction in dB, got {v}")));
                None
            }
        };
        Some(crate::audio::Track {
            src,
            start: at as f64,
            trim: trim_start.map(|t| [t, 0.0]),
            volume,
            db: 0.0,
            fade_in: a.fade_in.unwrap_or(0.0),
            fade_out: a.fade_out.unwrap_or(0.0),
            looped: a.loop_.unwrap_or(false),
            duration,
            duck,
            role: if a.voice == Some(true) { Some(Role::Voice) } else { None },
            ..Default::default()
        })
    }

    /// Captions → one TEXT `CaptionTrack` (paging, per-word pop and highlight).
    fn captions(&mut self, cap: &spec::Captions) -> Option<(text::captions::CaptionTrack, [f32; 2])> {
        use text::captions::{CaptionStyle, CaptionTrack, Cue, Upcoming, Word};
        let empty = spec::CaptionFile::default();
        let (list, f): (Vec<spec::CaptionCue>, &spec::CaptionFile) = match cap {
            spec::Captions::Cues(c) => (c.clone(), &empty),
            spec::Captions::File(f) => (f.cues.clone().unwrap_or_default(), f),
        };
        let mut cues: Vec<Cue> = Vec::new();
        for (i, c) in list.iter().enumerate() {
            let p = format!("captions[{i}]");
            let (a, e) = (self.time_raw(&c.at, &format!("{p}.at")), self.time_raw(&c.end, &format!("{p}.end")));
            let words: Vec<Word> = c.words.iter().flatten().map(|(t, s, e)| Word { text: t.clone(), start: *s as f32, end: *e as f32 }).collect();
            let text = if c.text.trim().is_empty() { words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ") } else { c.text.clone() };
            cues.push(Cue { text, start: a, end: e, words });
        }
        if let Some(src) = &f.src {
            let full = self.o.base_dir.join(src);
            match std::fs::read_to_string(&full) {
                Ok(s) => cues.extend(text::captions::parse(&s)),
                Err(e) => self.d.push(Diagnostic::error("captions.src", format!("can't read {}: {e}", full.display()))),
            }
        }
        if cues.is_empty() {
            return None;
        }
        let mut st = CaptionStyle::default();
        let size = f.size.as_ref().map(|s| self.len_c(s, "captions.size", self.h, 16.0)).unwrap_or(self.h.min(self.w) * 0.06);
        st.style.size = size;
        st.width = self.w * 0.86;
        let col = |me: &mut Self, c: &Option<String>, path: &str| c.as_ref().and_then(|c| me.color(c, path)).map(text::Color);
        if let Some(c) = col(self, &f.color, "captions.color") {
            st.style.color = c;
        } else if let Some(c) = self.theme.colors.get("text") {
            st.style.color = text::Color(*c);
        }
        // `background`: a pill behind the caption line; `highlightBox`: a box behind the spoken word.
        st.background = col(self, &f.background, "captions.background");
        if st.background.is_some() {
            st.radius = 0.5;
        }
        st.highlight_box = col(self, &f.highlight_box, "captions.highlightBox");
        // With a box behind the spoken word, keep its text colour by default (an accent on an accent box vanishes).
        st.highlight = match col(self, &f.highlight, "captions.highlight") {
            Some(c) => Some(c),
            None if st.highlight_box.is_some() || st.background.is_some() => Some(st.style.color),
            None => self.theme.colors.get("accent").map(|c| text::Color(*c)),
        };
        if let Some(n) = f.max_words {
            st.max_words = n.max(1);
        }
        st.uppercase = f.uppercase.unwrap_or(false);
        st.upcoming = match f.upcoming.as_deref() {
            None | Some("dim") => Upcoming::Dim,
            Some("hidden") => Upcoming::Hidden,
            Some("show") => Upcoming::Show,
            Some(x) => {
                self.d.push(Diagnostic::error("captions.upcoming", format!("unknown value '{x}'")).suggest(suggest(x, &["dim", "hidden", "show"])));
                Upcoming::Dim
            }
        };
        if let Some(sname) = &f.style {
            if let Some(ts) = self.theme.styles.get(sname).cloned() {
                if let Some(fnt) = &ts.font {
                    st.style.font = self.theme.font(fnt).to_string();
                }
                if let Some(w) = ts.weight.as_ref().and_then(weight_value) {
                    st.style.weight = w;
                }
            } else {
                let names: Vec<&String> = self.theme.styles.keys().collect();
                self.d.push(Diagnostic::error("captions.style", format!("unknown text style '{sname}'")).suggest(suggest(sname, &names)));
            }
        }
        let y = f.y.as_ref().map(|y| self.len_c(y, "captions.y", self.h, 16.0)).unwrap_or(self.h * 0.9);
        Some((CaptionTrack::new(&self.assets.fonts, &cues, &st), [self.w * 0.5, y]))
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
#[allow(dead_code)]
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

/// `"A"`…`"D"` → 0..4.
fn buf_slot_opt(name: &str) -> Option<usize> {
    match name {
        "A" => Some(0),
        "B" => Some(1),
        "C" => Some(2),
        "D" => Some(3),
        _ => None,
    }
}

fn buf_slot(name: &str) -> usize {
    buf_slot_opt(name).unwrap_or(0)
}

/// A grid's `show`: 0 off, 1 over the layers, 2 under them.
fn grid_show(g: &spec::GridSpec) -> u8 {
    match &g.show {
        Some(serde_json::Value::Bool(true)) => 1,
        Some(serde_json::Value::String(s)) if s == "under" => 2,
        Some(serde_json::Value::String(s)) if s == "over" => 1,
        _ => 0,
    }
}

const PRINTS: &[&str] = &["photocopy", "newsprint", "engraving", "riso", "screenprint"];

/// `print` presets: starting values for the treatment keys the layer doesn't set itself.
fn print_preset(l: &Layer, p: &str) -> Option<Layer> {
    let mut o = l.clone();
    let hs = |cell: f64, angle: f64, shape: &str, ink: &str, paper: &str| {
        spec::HalftoneValue::Full(spec::HalftoneSpec {
            cell: Some(Num::Num(cell)),
            angle: Some(Num::Num(angle)),
            shape: Some(shape.into()),
            ink: Some(ink.into()),
            paper: Some(paper.into()),
            gain: None,
        })
    };
    let ink = |a: f64, scale: f64| spec::InkValue::Full(spec::InkSpec { amount: Some(Num::Num(a)), scale: Some(scale), seed: None, fixed: None });
    match p {
        "photocopy" => {
            o.duotone.get_or_insert_with(|| vec!["#161616".into(), "#efeee9".into()]);
            o.posterize.get_or_insert(2);
            o.tone_range.get_or_insert([0.28, 0.68]);
            o.ink_texture.get_or_insert_with(|| ink(0.3, 10.0));
        }
        "newsprint" => {
            o.tone_range.get_or_insert([0.05, 0.95]);
            o.halftone.get_or_insert_with(|| hs(6.0, 45.0, "dot", "#1c1c1c", "#e8e4da"));
        }
        "engraving" => {
            o.tone_range.get_or_insert([0.1, 1.0]);
            o.halftone.get_or_insert_with(|| hs(5.0, 30.0, "line", "#151515", "#f2ede2"));
        }
        "riso" => {
            o.duotone.get_or_insert_with(|| vec!["riso:medium-blue".into(), "#ffffff".into()]);
            o.tone_range.get_or_insert([0.15, 0.75]);
            o.blend.get_or_insert_with(|| "multiply".into());
            o.ink_texture.get_or_insert_with(|| ink(0.25, 28.0));
        }
        "screenprint" => {
            // Three flat inks on paper: dark, a red and a yellow (override with gradientMap).
            if o.duotone.is_none() {
                o.gradient_map.get_or_insert_with(|| ["#1b1a19", "#d7301f", "#f2c230", "#f4efe6"].iter().map(|c| serde_json::Value::String(c.to_string())).collect());
            }
            o.posterize.get_or_insert(4);
            o.tone_range.get_or_insert([0.12, 0.85]);
            o.ink_texture.get_or_insert_with(|| ink(0.15, 40.0));
        }
        _ => return None,
    }
    Some(o)
}
