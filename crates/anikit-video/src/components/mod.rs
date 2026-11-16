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

pub mod style;
pub mod text;

pub mod backgrounds;
pub mod charts;
pub mod code;
pub mod devices;
pub mod lottie;
pub mod particles;
pub mod three;
pub mod transitions;
pub mod ui;

#[cfg(test)]
pub(crate) mod preview;

pub use style::Theme;
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
    // 3D
    "model" => three::Model, "Product spin/orbit of a 3D primitive (rounded box, sphere, torus, knot, extruded logo).", r##"{"shape":"torus"}"##;
    // Transitions
    "wipe" => transitions::Wipe, "Soft-edged wipe mask as a transition overlay.", r##"{}"##;
    "iris" => transitions::Iris, "Circular iris open/close.", r##"{}"##;
    "glitch" => transitions::Glitch, "Glitch slices with RGB split.", r##"{}"##;
    // Lottie
    "lottie" => lottie::Lottie, "Minimal Lottie player (shape layers, transforms, trim paths).", r##"{"src":"anim.json"}"##;
}

/// Keys every component accepts, handled here rather than by the component.
pub const COMMON_KEYS: &[&str] = &["type", "kind", "id", "x", "y", "width", "height", "theme"];

/// Build a component from its kind and the spec object (which may still
/// contain `type`, `kind` and the other [`COMMON_KEYS`]).
pub fn build(kind: &str, params: &Value) -> Result<Placed, Error> {
    let Some(k) = KINDS.iter().find(|k| k.name == kind) else {
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
    let component = (k.build)(Value::Object(obj)).map_err(|e| Error(format!("component `{kind}`: {}", suggest_field(&e.0))))?;
    Ok(Placed { component, x, y, w, h, theme })
}

/// Turn serde's "unknown field `colour`, expected one of `a`, `b`" into a
/// "did you mean" message.
fn suggest_field(msg: &str) -> String {
    if let Some(rest) = msg.strip_prefix("unknown field `") {
        if let Some((bad, tail)) = rest.split_once('`') {
            let options: Vec<&str> = tail.split('`').skip(1).step_by(2).collect();
            if let Some(s) = style::closest(bad, &options) {
                return format!("unknown parameter `{bad}`; did you mean `{s}`?");
            }
            return format!("unknown parameter `{bad}`; expected one of {}", options.join(", "));
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
        let assets = text::StubAssets::default();
        for k in KINDS {
            let v: Value = serde_json::from_str(k.example).unwrap();
            let p = build(k.name, &v).unwrap_or_else(|e| panic!("{}: {e}", k.name));
            let end = p.component.duration().unwrap_or(3.0);
            for t in [0.0, end * 0.5, end, end + 1.0] {
                let ops = p.eval(t, 1280.0, 720.0, 30.0, &theme, &assets);
                preview::validate(&ops).unwrap_or_else(|e| panic!("{} at t={t}: {e}", k.name));
            }
        }
    }

    #[test]
    fn errors_suggest() {
        let e = build("bar-chrt", &Value::Null).err().unwrap();
        assert!(e.0.contains("did you mean `bar-chart`"), "{e}");
        let e = build("bar-chart", &serde_json::json!({"valuess": [1]})).err().unwrap();
        assert!(e.0.contains("did you mean `values`"), "{e}");
    }
}
