//! High-level building blocks that expand to display-list ops.
//!
//! A component is referenced from the spec as
//! `{ "type": "component", "kind": "bar-chart", ...params }`. The evaluator
//! calls [`build`] once (parameters are parsed, validated and preprocessed
//! there), then [`Component::eval`] every frame with the component's **local**
//! time `t` (seconds since it started) and a [`Ctx`].
//!
//! Every component draws into its own box `[0, 0, ctx.width, ctx.height]`
//! (the box comes from the generic `x`, `y`, `width`, `height` keys, which
//! [`build`] strips before the component parses its own parameters; it
//! defaults to the whole canvas). The returned ops are already translated to
//! the box position, so the evaluator can append them directly.
//!
//! Pixels that are cheaper to compute than to describe as vectors (noise
//! fields, 3D renders, grain) go through [`Assets::image`], keyed by content
//! so identical images are made once and shared across frames and threads.
//! Text goes through [`Assets::text`] so the TEXT module can lay it out.

#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

use crate::dl::{self, Affine, ImageId, Op, Rgba};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
#[cfg(test)]
use std::sync::Arc;

pub mod style;
pub mod docs;
pub mod fxkit;
pub mod polish;
pub mod host;
pub mod text;

pub mod backgrounds;
pub mod bgfx;
pub mod blocks;
pub mod charts;
pub mod code;
pub mod devices;
pub mod lottie;
pub mod metaballs;
pub mod morph;
pub mod particle_text;
pub mod particles;
pub mod screens;
pub mod rays;
pub mod text3d;
pub mod three;
pub mod globe;
pub mod transitions;
pub mod ui;
pub mod uikit;
pub mod visualizer;
pub mod ken_burns;
pub mod tiles;
pub mod reactions;
pub mod card_stack;

#[cfg(test)]
pub(crate) mod preview;

pub use style::{set_draft, Theme};
pub use text::{Face, TextStyle};

/// What a component can ask the engine for while evaluating.
pub trait Assets: Sync {
    /// An image made of straight-alpha RGBA8 pixels (`w * h * 4` bytes),
    /// identified by `key` (a hash of everything that determines its
    /// content). `make` is only called the first time a key is seen.
    fn image(&self, key: u64, w: u32, h: u32, make: &mut dyn FnMut() -> Vec<u8>) -> ImageId;
    /// A user image (or the current frame of a user video) by its spec `src`,
    /// with its pixel size. `t` is the component's local time (for videos).
    fn media(&self, src: &str, t: f32) -> Option<(ImageId, u32, u32)>;
    /// Lay out one line of text with its alphabetic baseline starting at
    /// `(x, y)`; returns the glyph ops and the advance width.
    fn text(&self, s: &str, style: &TextStyle, x: f32, y: f32) -> (Vec<Op>, f32);
    /// Advance width of one line of text, without drawing it.
    fn measure(&self, s: &str, style: &TextStyle) -> f32;
    /// A named 2D sub-composition (top-level `compositions`): size, clock and loop default.
    fn composition_info(&self, _name: &str) -> Option<CompInfo> {
        None
    }
    /// Its frame at composition time `t` (snapped to its frame grid) as premultiplied
    /// RGBA8, downscaled by `2^lod`. Deterministic, cached and shared across threads.
    fn composition_frame(&self, _name: &str, _t: f32, _lod: u8) -> Option<std::sync::Arc<vello_cpu::Pixmap>> {
        None
    }
}

/// Size and clock of a named sub-composition (see [`Assets::composition_info`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    /// Seconds.
    pub duration: f32,
    /// The composition's own `loop` default.
    pub looped: bool,
}

/// Per-frame context handed to [`Component::eval`].
pub struct Ctx<'a> {
    /// The component's box size in pixels.
    pub width: f32,
    pub height: f32,
    /// Output frame rate (some components snap motion to frames).
    pub fps: f32,
    pub theme: &'a Theme,
    pub assets: &'a dyn Assets,
}

/// A preprocessed component, evaluated once per frame.
pub trait Component: Send + Sync {
    /// Ops for local time `t` (seconds), in box coordinates.
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op>;
    /// How long the component's own animation takes, when it has a natural
    /// length (a typing animation, a chart build). `None` means "static" or
    /// "loops forever".
    fn duration(&self) -> Option<f32> {
        None
    }
}

/// A built component plus its box.
pub struct Placed {
    pub component: Box<dyn Component>,
    /// `[x, y, w, h]`; `w`/`h` are `None` to fill the canvas.
    pub x: f32,
    pub y: f32,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub theme: Option<Theme>,
}

impl Placed {
    /// Evaluate at local time `t` for a canvas of `cw × ch`, returning ops in
    /// canvas coordinates.
    pub fn eval(&self, t: f32, cw: f32, ch: f32, fps: f32, theme: &Theme, assets: &dyn Assets) -> Vec<Op> {
        let ctx = Ctx {
            width: self.w.unwrap_or(cw - self.x),
            height: self.h.unwrap_or(ch - self.y),
            fps,
            theme: self.theme.as_ref().unwrap_or(theme),
            assets,
        };
        let mut ops = self.component.eval(t.max(0.0), &ctx);
        if self.x != 0.0 || self.y != 0.0 {
            transform_ops(&mut ops, &[1.0, 0.0, 0.0, 1.0, self.x, self.y]);
        }
        ops
    }
}

/// Error from [`build`]: what was wrong and, when possible, a suggestion.
#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// One entry of the component catalogue.
pub struct Kind {
    pub name: &'static str,
    pub summary: &'static str,
    /// A minimal, valid example of the params (JSON), for docs and tests.
    pub example: &'static str,
    build: fn(Value) -> Result<Box<dyn Component>, Error>,
}

fn make<T: Component + DeserializeOwned + Prepare + 'static>(v: Value) -> Result<Box<dyn Component>, Error> {
    let mut c: T = serde_json::from_value(v).map_err(|e| Error(e.to_string()))?;
    c.prepare()?;
    Ok(Box::new(c))
}

/// Validation and one-off preprocessing after parameters are parsed.
pub trait Prepare {
    fn prepare(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

macro_rules! kinds {
    ($( $name:literal => $ty:ty, $summary:literal, $example:literal; )*) => {
        /// Every component kind, with a summary and a minimal example.
        pub static KINDS: &[Kind] = &[ $( Kind { name: $name, summary: $summary, example: $example, build: make::<$ty> }, )* ];
    };
}

kinds! {
    // Code
    "code" => code::CodeBlock, "Editor window with syntax highlighting, typing animation, line highlights, diffs, scrolling and a cursor.", r##"{"code":"fn main() {\n    println!(\"hi\");\n}","language":"rust"}"##;
    "terminal" => code::Terminal, "Terminal window that types commands and prints output line by line.", r##"{"lines":["$ npm i anikit","added 1 package in 0.4s"]}"##;
    // Charts
    "bar-chart" => charts::BarChart, "Vertical or horizontal bars that grow in with staggered springs, value labels and gridlines.", r##"{"values":[3,7,5,9],"labels":["Q1","Q2","Q3","Q4"]}"##;
    "line-chart" => charts::LineChart, "One or more lines that draw on, with an optional area fill, dots and an end label.", r##"{"series":[[1,3,2,5,4,7]]}"##;
    "area-chart" => charts::AreaChart, "Line chart with a gradient area fill.", r##"{"series":[[1,3,2,5,4,7]]}"##;
    "donut-chart" => charts::DonutChart, "Donut or pie that sweeps in segment by segment, with a centre total.", r##"{"values":[45,30,25],"labels":["A","B","C"]}"##;
    "pie-chart" => charts::PieChart, "Pie chart (a donut with no hole).", r##"{"values":[45,30,25]}"##;
    "counter" => charts::Counter, "Big number that counts up with an ease, prefix/suffix and thousands separators.", r##"{"to":12500,"prefix":"$"}"##;
    "stat" => charts::Stat, "Stat tile: label, counting value, delta badge and a sparkline.", r##"{"label":"Revenue","value":48200,"prefix":"$","delta":12.4,"spark":[3,4,3,5,6,8]}"##;
    // Devices
    "phone" => devices::Phone, "Phone mockup with a screen image or video, reflection and shadow.", r##"{}"##;
    "laptop" => devices::Laptop, "Laptop mockup with a screen image or video.", r##"{}"##;
    "browser" => devices::Browser, "Browser window with tabs, address bar and page content.", r##"{"url":"anikit.dev"}"##;
    // UI kit
    "card" => ui::Card, "Surface card with title, body, icon dot, shadow and border.", r##"{"title":"Card title","body":"Supporting text"}"##;
    "button" => ui::Button, "Pill or rounded button with a press animation.", r##"{"label":"Get started"}"##;
    "toggle" => ui::Toggle, "iOS-style switch that flips at a given time.", r##"{"at":0.5}"##;
    "badge" => ui::Badge, "Small coloured label.", r##"{"label":"New"}"##;
    "avatar" => ui::Avatar, "Circular avatar with initials or an image, and a status dot.", r##"{"name":"Ada Lovelace"}"##;
    "avatar-stack" => ui::AvatarStack, "Overlapping avatars that pop in.", r##"{"names":["Ada","Grace","Alan"]}"##;
    "progress-bar" => ui::ProgressBar, "Progress bar that fills over time, with an optional label.", r##"{"to":0.8}"##;
    "progress-ring" => ui::ProgressRing, "Circular progress ring with a percentage.", r##"{"to":0.72}"##;
    "toast" => ui::Toast, "Notification toast that slides in, holds and leaves.", r##"{"title":"Deployed","body":"Production is live"}"##;
    "cursor" => ui::Cursor, "Animated mouse pointer moving along waypoints with click ripples.", r##"{"path":[[100,100],[400,300]],"clicks":[1.0]}"##;
    "tooltip" => ui::Tooltip, "Tooltip bubble with an arrow, popping in.", r##"{"text":"Click to copy"}"##;
    "callout" => ui::Callout, "Annotation: a dot, a drawn leader line and a label.", r##"{"from":[100,100],"to":[300,60],"text":"New API"}"##;
    "arrow" => ui::Arrow, "Curved hand-drawn-style arrow that draws on.", r##"{"from":[100,300],"to":[400,120]}"##;
    "lower-third" => ui::LowerThird, "Name and title strap that wipes in and out.", r##"{"title":"Ada Lovelace","subtitle":"Analyst"}"##;
    "title-card" => ui::TitleCard, "Large centred title with eyebrow and subtitle, revealed word by word.", r##"{"title":"Introducing Anikit Video"}"##;
    // Backgrounds
    "mesh-gradient" => backgrounds::MeshGradient, "Slowly drifting mesh gradient of soft colour blobs.", r##"{}"##;
    "aurora" => backgrounds::Aurora, "Aurora ribbons swaying over a dark sky.", r##"{}"##;
    "grain" => backgrounds::Grain, "Animated film grain overlay.", r##"{}"##;
    "grid" => backgrounds::Grid, "Line grid with a radial fade and optional perspective.", r##"{}"##;
    "dots" => backgrounds::Dots, "Dot grid with a radial fade.", r##"{}"##;
    "light-leak" => backgrounds::LightLeak, "Warm drifting light leaks.", r##"{}"##;
    "bokeh" => backgrounds::Bokeh, "Out-of-focus light circles drifting.", r##"{}"##;
    "starfield" => backgrounds::Starfield, "Twinkling stars, optionally flying towards the camera.", r##"{}"##;
    "gradient" => backgrounds::Gradient, "Linear or radial gradient fill.", r##"{"colors":["#0b1020","#1d1340"]}"##;
    // Particles
    "confetti" => particles::Confetti, "Confetti burst with gravity, drag and tumbling (seeded).", r##"{}"##;
    "sparks" => particles::Sparks, "Bright sparks bursting out with trails.", r##"{}"##;
    "snow" => particles::Snow, "Gently falling snow or dust.", r##"{}"##;
    "dust" => particles::Dust, "Floating glowing dust motes.", r##"{}"##;
    "particle-text" => particle_text::ParticleText, "Thousands of glowing particles fly in, form a word, hold, re-form into the next word and blow away.", r##"{"text":"Hello"}"##;
    // Shapes and light
    "morph" => morph::Morph, "Smooth gradient shape morphing between named shapes (circle, square, star, heart, blob, triangle) or SVG paths.", r##"{"shapes":["circle","star","heart"]}"##;
    "metaballs" => metaballs::Metaballs, "Liquid gooey blobs that drift, merge and split, with a gradient fill and glow.", r##"{}"##;
    "god-rays" => rays::GodRays, "Volumetric light rays from a point, slowly turning, with dust motes in the beams.", r##"{}"##;
    "lens-flare" => rays::LensFlare, "Anamorphic lens flare: streak, halo and ghosts along the axis through the centre; `at` can travel a path.", r##"{"at":[0.3,0.3]}"##;
    // 3D
    "model" => three::Model, "Product spin/orbit of a 3D primitive (rounded box, sphere, torus, knot, extruded logo).", r##"{"shape":"torus"}"##;
    "text-3d" => text3d::Text3d, "Extruded, bevelled chrome 3D text or logo whose letters rise in, with studio lighting, sway and a glint sweep.", r##"{"text":"ANIKIT"}"##;
    "globe" => globe::Globe, "Rotating dot-matrix globe with glowing great-circle arcs, travelling pulses, pulsing markers and an atmosphere.", r##"{"arcs":[[37.8,-122.4,51.5,-0.1]]}"##;
    // Transitions
    "wipe" => transitions::Wipe, "Soft-edged wipe mask as a transition overlay.", r##"{}"##;
    "iris" => transitions::Iris, "Circular iris open/close.", r##"{}"##;
    "glitch" => transitions::Glitch, "Glitch slices with RGB split.", r##"{}"##;
    // Content blocks
    "chat" => blocks::Chat, "Chat thread: user and assistant bubbles, typing dots, streamed replies, auto-scroll.", r##"{"messages":[{"from":"user","text":"Make me a launch video"},{"from":"assistant","text":"Rendering now."}]}"##;
    "prompt" => blocks::Prompt, "AI prompt box that types a request and presses send, with a glowing gradient rim.", r##"{"text":"Summarise this quarter in a chart"}"##;
    "checklist" => blocks::Checklist, "Items that appear and tick off one by one.", r##"{"items":["Fast","Native","Agent-first"]}"##;
    "quote" => blocks::Quote, "Testimonial: big quote mark, word-by-word reveal, stars, avatar, name and title.", r##"{"text":"It just works.","name":"Ada Lovelace","title":"CTO"}"##;
    "table" => blocks::Table, "Comparison table with check/cross cells and a highlighted column.", r##"{"columns":["","Us","Them"],"rows":[["Fast",true,false]],"highlight":1}"##;
    "timeline" => blocks::Timeline, "Milestones along a line that draws on, with a pulsing current step.", r##"{"items":[{"label":"Q1","title":"Beta"},{"label":"Q2","title":"Launch"}],"current":1}"##;
    "marquee" => blocks::Marquee, "Endless scrolling ticker of words or logos with faded edges.", r##"{"items":["Stripe","Linear","Vercel"]}"##;
    "spotlight" => blocks::Spotlight, "Dims everything except a focus rectangle that glides between targets (product demos).", r##"{"targets":[[100,100,300,120],[500,300,200,80]]}"##;
    "keys" => blocks::Keys, "Keyboard shortcut keycaps that pop in and press.", r##"{"keys":["⌘","K"],"press":[0.6]}"##;
    // Media
    "visualizer" => visualizer::Visualizer, "Audio visualiser synced to a music src: spectrum bars, a mirrored wave or radial bars around an image (synthetic groove when no src).", r##"{"style":"bars"}"##;
    "ken-burns" => ken_burns::KenBurns, "Photo with a slow zoom/pan (presets or from/to rects), parallax layers, vignette and a film grade.", r##"{"motion":"in"}"##;
    "tile-reveal" => tiles::TileReveal, "Grid of tiles that flip, scale or fade in with a diagonal/radial wave to reveal an image or gradient; can reverse.", r##"{"cols":10}"##;
    "kinetic-grid" => tiles::KineticGrid, "Mesmerising grid of rounded squares or dots whose size, rotation and colour ripple with a wave.", r##"{}"##;
    "reactions" => reactions::Reactions, "Live-stream reactions (hearts, likes, fire, stars, sparkles, 100, laugh) floating up with wobble, pop and fade.", r##"{"kinds":["heart","like","fire"]}"##;
    "card-stack" => card_stack::CardStack, "2.5D stack of image or titled cards; the front card swipes away every interval and the stack advances.", r##"{"cards":["Design","Build","Ship"]}"##;
    // Lottie
    "lottie" => lottie::Lottie, "Minimal Lottie player (shape layers, transforms, trim paths).", r##"{"src":"anim.json"}"##;
}

/// Keys every component accepts, handled here rather than by the component.
pub const COMMON_KEYS: &[&str] = &["type", "kind", "id", "x", "y", "width", "height", "theme"];

/// Build a component from its kind and the spec object (which may still
/// contain `type`, `kind` and the other [`COMMON_KEYS`]).
/// Replace `"$token"` strings in component props with colours from the scene theme
/// (`lookup` maps a token name to a CSS colour). Unknown tokens are left as they are,
/// so the component reports them.
pub fn resolve_tokens(props: &mut Value, lookup: &dyn Fn(&str) -> Option<String>) {
    match props {
        Value::String(s) => {
            if let Some(c) = s.strip_prefix('$').and_then(lookup) {
                *s = c;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|v| resolve_tokens(v, lookup)),
        Value::Object(m) => {
            for (k, v) in m.iter_mut() {
                // `data`/`text`-like content stays literal.
                if !matches!(k.as_str(), "text" | "code" | "title" | "label" | "body" | "lines" | "messages") {
                    resolve_tokens(v, lookup);
                }
            }
        }
        _ => {}
    }
}

/// Fill a component's props with the theme's material defaults (`theme.material`:
/// `surface`, `motion`, `glow`, `shadow`, `texture`) where the props leave them out and
/// the kind accepts them. Explicit props always win; a value a kind can't take is skipped.
pub fn with_material(kind: &str, props: &Value, material: &Value) -> Value {
    let Some(m) = material.as_object().filter(|m| !m.is_empty()) else { return props.clone() };
    let mut out = match props {
        Value::Object(o) => o.clone(),
        _ => Map::new(),
    };
    let has = |o: &Map<String, Value>, k: &str| o.contains_key(k) || o.contains_key(&camel(k));
    let candidates = |key: &str, v: &Value| -> Vec<Value> {
        match (key, v) {
            ("motion", Value::String(s)) => {
                let s = if s == "mechanical" { "snappy" } else { s.as_str() };
                vec![Value::String(s.into())]
            }
            ("surface", Value::String(s)) => vec![Value::String(s.clone())],
            ("glow", Value::Number(n)) => {
                let g = n.as_f64().unwrap_or(0.0);
                vec![v.clone(), Value::Bool(g >= 0.5)]
            }
            ("shadow", Value::Number(_)) => vec![v.clone()],
            _ => Vec::new(),
        }
    };
    for (key, v) in m {
        if v.is_null() || has(&out, key) {
            continue;
        }
        for cand in candidates(key, v) {
            let mut trial = out.clone();
            trial.insert(key.clone(), cand.clone());
            if build(kind, &Value::Object(trial)).is_ok() {
                out.insert(key.clone(), cand);
                break;
            }
        }
    }
    Value::Object(out)
}

pub fn build(kind: &str, params: &Value) -> Result<Placed, Error> {
    let kebab = canonical(kind);
    let Some(k) = KINDS.iter().find(|k| k.name == kebab) else {
        let names: Vec<&str> = KINDS.iter().map(|k| k.name).collect();
        return Err(Error(match style::closest(kind, &names) {
            Some(s) => format!("unknown component kind `{kind}`; did you mean `{s}`?"),
            None => format!("unknown component kind `{kind}`; known kinds: {}", names.join(", ")),
        }));
    };
    let mut obj = match params {
        Value::Object(m) => m.clone(),
        Value::Null => Map::new(),
        _ => return Err(Error(format!("component `{kind}`: params must be an object"))),
    };
    let num = |obj: &mut Map<String, Value>, key: &str| -> Result<Option<f32>, Error> {
        match obj.remove(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => Ok(n.as_f64().map(|v| v as f32)),
            Some(v) => Err(Error(format!("component `{kind}`: `{key}` must be a number, got {v}"))),
        }
    };
    let x = num(&mut obj, "x")?.unwrap_or(0.0);
    let y = num(&mut obj, "y")?.unwrap_or(0.0);
    let w = num(&mut obj, "width")?;
    let h = num(&mut obj, "height")?;
    let theme = match obj.remove("theme") {
        None | Some(Value::Null) => None,
        Some(v) => Some(Theme::from_value(&v).map_err(|e| Error(format!("component `{kind}`: theme: {e}")))?),
    };
    for key in ["type", "kind", "id"] {
        obj.remove(key);
    }
    let mut obj = snake_keys(obj);
    shorthand(&kebab, &mut obj);
    let component = (k.build)(Value::Object(obj)).map_err(|e| Error(format!("component `{kind}`: {}", suggest_field(&e.0))))?;
    Ok(Placed { component, x, y, w, h, theme })
}

/// Short dotted aliases (`chart.bar`, `bg.mesh`, `device.phone`, …) and other
/// spellings → the canonical kebab-case kind.
pub fn canonical(kind: &str) -> String {
    let k = kebab(kind).replace('_', "-");
    let alias = match k.as_str() {
        "chart.bar" | "bar" | "bars" => "bar-chart",
        "chart.line" | "line" => "line-chart",
        "chart.area" | "area" => "area-chart",
        "chart.donut" | "donut" => "donut-chart",
        "chart.pie" | "pie" => "pie-chart",
        "chart.counter" | "number" | "count" => "counter",
        "chart.stat" => "stat",
        "bg.mesh" | "mesh" => "mesh-gradient",
        "bg.aurora" => "aurora",
        "bg.grain" => "grain",
        "bg.grid" => "grid",
        "bg.dots" => "dots",
        "bg.leak" | "bg.light-leak" => "light-leak",
        "bg.bokeh" => "bokeh",
        "bg.stars" | "stars" | "bg.starfield" => "starfield",
        "bg.gradient" => "gradient",
        "device.phone" | "iphone" => "phone",
        "device.laptop" | "macbook" => "laptop",
        "device.browser" | "window" => "browser",
        "ui.card" => "card",
        "ui.button" => "button",
        "ui.toast" | "notification" => "toast",
        "ui.cursor" | "mouse" => "cursor",
        "ui.badge" | "pill" | "tag" => "badge",
        "ui.progress" | "progress" => "progress-bar",
        "ui.ring" | "ring" => "progress-ring",
        "title" | "ui.title" => "title-card",
        "lower3rd" | "ui.lower-third" => "lower-third",
        "fx.confetti" => "confetti",
        "fx.sparks" | "fireworks" => "sparks",
        "fx.snow" => "snow",
        "fx.dust" => "dust",
        "3d" | "3d.model" => "model",
        "code-block" | "editor" => "code",
        "shell" | "console" => "terminal",
        "testimonial" => "quote",
        "todo" | "checks" => "checklist",
        "ticker" | "logos" => "marquee",
        "shortcut" | "kbd" => "keys",
        "focus" => "spotlight",
        "messages" => "chat",
        "input" | "ai-prompt" => "prompt",
        "roadmap" => "timeline",
        "comparison" => "table",
        _ => return k,
    };
    alias.to_string()
}

/// Every image file a component's props refer to (`src`, `srcs`, `avatar`,
/// `image`, parallax `layers[].src`, `cards[].src`, image marquee `items`), so
/// the evaluator can load them before the component asks `Assets::media`.
pub fn media_srcs(props: &Value) -> Vec<String> {
    fn is_image(s: &str) -> bool {
        let l = s.to_ascii_lowercase();
        [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"].iter().any(|e| l.ends_with(e))
    }
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) if is_image(s) => {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(m) => m.iter().filter(|(k, _)| k.as_str() != "data").for_each(|(_, x)| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(props, &mut out);
    out
}

/// Shared shorthands: `data` means the main data of any data component, and a
/// flat number array is a single line series.
fn shorthand(kind: &str, obj: &mut Map<String, Value>) {
    if let Some(d) = obj.remove("data") {
        let key = match kind {
            "line-chart" | "area-chart" => "series",
            "counter" => "to",
            "stat" => "value",
            "chat" => "messages",
            "checklist" | "timeline" | "marquee" => "items",
            "table" => "rows",
            "terminal" => "lines",
            "lottie" => {
                // Lottie's `data` is the animation itself.
                obj.insert("data".into(), d);
                return;
            }
            _ => "values",
        };
        obj.entry(key.to_string()).or_insert(d);
    }
    // `text` is the main text of any text-bearing component.
    let text_key = match kind {
        "title-card" | "card" | "lower-third" | "toast" => Some("title"),
        "button" | "badge" | "progress-bar" | "progress-ring" => Some("label"),
        "code" => Some("code"),
        _ => None,
    };
    if let Some(k) = text_key {
        if k != "text" {
            if let Some(t) = obj.remove("text") {
                obj.entry(k.to_string()).or_insert(t);
            }
        }
    }
    if kind == "counter" {
        if let Some(v) = obj.remove("value") {
            obj.entry("to".to_string()).or_insert(v);
        }
    }
    if matches!(kind, "line-chart" | "area-chart") {
        if let Some(Value::Array(a)) = obj.get("series") {
            if a.iter().all(|v| v.is_number()) {
                let flat = Value::Array(a.clone());
                obj.insert("series".into(), Value::Array(vec![flat]));
            }
        }
    }
}

/// camelCase keys → snake_case, recursively (except inside `data`, which is
/// foreign JSON such as a Lottie file).
fn snake_keys(m: Map<String, Value>) -> Map<String, Value> {
    fn conv(v: Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(snake_keys(m)),
            Value::Array(a) => Value::Array(a.into_iter().map(conv).collect()),
            v => v,
        }
    }
    m.into_iter()
        .map(|(k, v)| {
            let sk = kebab(&k).replace('-', "_");
            if k == "data" {
                (k, v)
            } else {
                (sk, conv(v))
            }
        })
        .collect()
}

/// `barChart` / `BarChart` / `bar_chart` → `bar-chart`.
pub fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_digit() && out.ends_with(|p: char| p.is_ascii_lowercase()) {
            // `text3d` → `text-3d`.
            out.push('-');
            out.push(c);
        } else if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('-') && !out.ends_with(|p: char| p.is_ascii_digit()) {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '_' || c == ' ' {
            out.push('-');
        } else {
            out.push(c);
        }
    }
    out
}

/// `bar-chart` → `barChart` (the spelling the JSON spec uses).
pub fn camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut up = false;
    for c in name.chars() {
        if c == '-' {
            up = true;
        } else if up {
            out.push(c.to_ascii_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Turn serde's "unknown field `colour`, expected one of `a`, `b`" into a
/// "did you mean" message.
fn suggest_field(msg: &str) -> String {
    if let Some(rest) = msg.strip_prefix("unknown field `") {
        if let Some((bad, tail)) = rest.split_once('`') {
            let options: Vec<String> = tail.split('`').skip(1).step_by(2).map(|o| camel(&o.replace('_', "-"))).collect();
            let opts: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
            let bad = camel(&bad.replace('_', "-"));
            if let Some(s) = style::closest(&bad, &opts) {
                return format!("unknown parameter `{bad}`; did you mean `{s}`?");
            }
            return format!("unknown parameter `{bad}`; expected one of {}", opts.join(", "));
        }
    }
    msg.to_string()
}

/// Apply `m` after every op's own transform (and to layer clips).
pub fn transform_ops(ops: &mut [Op], m: &Affine) {
    for op in ops {
        match op {
            Op::Fill { transform, .. }
            | Op::Stroke { transform, .. }
            | Op::RoundRect { transform, .. }
            | Op::Image { transform, .. }
            | Op::Glyphs { transform, .. } => *transform = dl::mul(m, transform),
            Op::PushLayer { clip: Some((_, t)), .. } => *t = dl::mul(m, t),
            Op::PushLayer { .. } | Op::PopLayer => {}
        }
    }
}

/// Wrap `ops` in a layer with `opacity` (skipped when fully opaque, dropped when invisible).
pub fn with_opacity(ops: Vec<Op>, opacity: f32) -> Vec<Op> {
    if opacity >= 0.999 {
        return ops;
    }
    if opacity <= 0.001 || ops.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(ops.len() + 2);
    out.push(Op::PushLayer { opacity, blend: dl::Blend::Normal, filters: Vec::new(), clip: None, cache_key: None });
    out.extend(ops);
    out.push(Op::PopLayer);
    out
}

/// Straight-alpha colour helper.
pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba([r, g, b, a])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_builds_and_evaluates() {
        let theme = Theme::default();
        let assets = preview::host();
        for k in KINDS {
            let v: Value = serde_json::from_str(k.example).unwrap();
            let p = build(k.name, &v).unwrap_or_else(|e| panic!("{}: {e}", k.name));
            let end = p.component.duration().unwrap_or(3.0);
            for t in [0.0, end * 0.5, end, end + 1.0] {
                let ops = p.eval(t, 1280.0, 720.0, 30.0, &theme, assets);
                preview::validate(&ops).unwrap_or_else(|e| panic!("{} at t={t}: {e}", k.name));
            }
        }
    }

    #[test]
    fn gallery_example_builds() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/video/examples/components.json");
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut n = 0;
        for scene in spec["scenes"].as_array().unwrap() {
            for layer in scene["layers"].as_array().unwrap() {
                if let Some(kind) = layer["component"].as_str() {
                    build(kind, &layer["props"]).unwrap_or_else(|e| panic!("{kind}: {e}"));
                    n += 1;
                }
            }
            if let Some(tr) = scene.get("transition").filter(|t| t.is_object()) {
                let tr: transitions::Transition = serde_json::from_value(tr.clone()).unwrap();
                tr.validate().unwrap();
            }
        }
        assert!(n > 20);
    }

    #[test]
    fn material_and_tokens() {
        let m = serde_json::json!({"surface": "paper", "motion": "mechanical", "glow": 1.0, "shadow": null});
        let p = with_material("card", &serde_json::json!({"title": "x"}), &m);
        assert_eq!(p["surface"], "paper");
        assert_eq!(p["motion"], "snappy");
        // explicit props win
        let p = with_material("card", &serde_json::json!({"surface": "glass"}), &m);
        assert_eq!(p["surface"], "glass");
        // kinds without the key are untouched
        assert!(with_material("aurora", &serde_json::json!({}), &m).get("motion").is_none_or(|v| build("aurora", &serde_json::json!({"motion": v})).is_ok()));
        let mut p = serde_json::json!({"color": "$accent", "title": "$accent", "colors": ["$accent", "#fff"]});
        resolve_tokens(&mut p, &|k| (k == "accent").then(|| "#ff0000ff".to_string()));
        assert_eq!(p["color"], "#ff0000ff");
        assert_eq!(p["title"], "$accent");
        assert_eq!(p["colors"][0], "#ff0000ff");
    }

    #[test]
    fn errors_suggest() {
        let e = build("bar-chrt", &Value::Null).err().unwrap();
        assert!(e.0.contains("did you mean `bar-chart`"), "{e}");
        assert!(build("barChart", &serde_json::json!({"values": [1], "fontSize": 20, "format": {"prefix": "$"}})).is_ok());
        assert!(build("chart.bar", &serde_json::json!({"data": [3, 7, 5]})).is_ok());
        assert!(build("chart.line", &serde_json::json!({"data": [3, 7, 5]})).is_ok());
        assert!(build("chart.donut", &serde_json::json!({"data": [3, 7, 5]})).is_ok());
        assert!(build("bg.mesh", &Value::Null).is_ok());
        assert_eq!(media_srcs(&serde_json::json!({"src": "a.png", "layers": [{"src": "b.JPG"}], "text": "x.png?no", "audio": "m.mp3"})), vec!["a.png", "b.JPG"]);
        assert!(build("title", &serde_json::json!({"text": "Hi"})).is_ok());
        assert!(build("counter", &serde_json::json!({"value": 10})).is_ok());
        let e = build("code", &serde_json::json!({"lineNumbrs": false})).err().unwrap();
        assert!(e.0.contains("did you mean `lineNumbers`"), "{e}");
        assert_eq!(camel("avatar-stack"), "avatarStack");
        assert_eq!(kebab("avatarStack"), "avatar-stack");
        let e = build("bar-chart", &serde_json::json!({"valuess": [1]})).err().unwrap();
        assert!(e.0.contains("did you mean `values`"), "{e}");
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// `cargo test --release -p anikit-video components::bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn per_frame_cost_1080p() {
        let theme = Theme::default();
        let host = preview::host();
        let mut rows = Vec::new();
        let only = std::env::var("BENCH_KINDS").unwrap_or_default();
        for k in KINDS.iter().filter(|k| only.is_empty() || only.split(',').any(|o| o == k.name)) {
            let v: Value = serde_json::from_str(k.example).unwrap();
            let p = build(k.name, &v).unwrap();
            // Warm caches (text layouts, images) over the first frames.
            for f in 0..10 {
                let _ = p.eval(f as f32 / 60.0, 1920.0, 1080.0, 60.0, &theme, host);
            }
            let n = 60;
            let t0 = std::time::Instant::now();
            let mut lists = Vec::new();
            for f in 10..10 + n {
                lists.push(p.eval(f as f32 / 60.0, 1920.0, 1080.0, 60.0, &theme, host));
            }
            let eval_ms = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
            let mut r = crate::render::Renderer::new(Arc::new(host.render_assets(None)));
            let t1 = std::time::Instant::now();
            for ops in &lists {
                let dl = dl::DisplayList { width: 1920, height: 1080, background: theme.background, ops: ops.clone() };
                r.render(&dl);
            }
            let render_ms = t1.elapsed().as_secs_f64() * 1000.0 / n as f64;
            rows.push((k.name, eval_ms, render_ms, lists[0].len()));
        }
        rows.sort_by(|a, b| (b.1 + b.2).total_cmp(&(a.1 + a.2)));
        for (name, e, r, n) in rows {
            println!("{name:16} eval {e:7.3} ms  render {r:7.2} ms  ops {n}");
        }
    }
}
