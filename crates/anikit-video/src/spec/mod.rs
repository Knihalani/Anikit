//! The agent-facing spec: serde types for `docs/video/spec.md` (cheat sheet) and `docs/video/spec-reference.md`.
//!
//! Every field is optional except `scenes` (and `layers` on a scene), so the
//! types are mostly `Option`s; defaults are applied by [`crate::eval`].
//! Values that may be numbers, unit strings or expressions are [`Num`];
//! colours and gradients are [`Paint`] strings or objects. Enumerated string
//! values (easings, layer types, presets) are kept as strings and checked in
//! [`validate`], which gives "did you mean" suggestions that serde can't.

#![allow(clippy::large_enum_variant)]

mod de;
pub mod parse;
pub mod reuse;
pub mod script;
pub mod validate;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use parse::{parse_json, parse_str, parse_yaml, Diagnostic, Parsed, Severity};

/// A number, a unit string (`"50%"`, `"10vh"`, `"1.5s"`, `"12f"`, `"@marker+0.2"`),
/// a relative value (`"+40"`, `"*1.5"`) or an expression (`"=sin(t)*10"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Num {
    Num(f64),
    Str(String),
}

impl From<f64> for Num {
    fn from(v: f64) -> Self {
        Num::Num(v)
    }
}

/// A value inside `from` / `to` / `keyframes`: a number, string (units,
/// expression, colour) or a pair (`scale: [sx, sy]`).
pub type AnimValue = serde_json::Value;

/// Implements `Deserialize` for "a string or an object" types without
/// buffering, so unknown keys inside the object are still reported.
macro_rules! string_or_struct {
    ($ty:ident, $obj:ident, |$s:ident| $from_str:expr) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;
                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = $ty;
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(f, "a string or an object")
                    }
                    fn visit_str<E: serde::de::Error>(self, $s: &str) -> Result<$ty, E> {
                        Ok($ty($from_str))
                    }
                    fn visit_bool<E: serde::de::Error>(self, b: bool) -> Result<$ty, E> {
                        if b {
                            let $s = "";
                            Ok($ty($from_str))
                        } else {
                            Err(E::custom("expected a string or an object"))
                        }
                    }
                    fn visit_map<M: serde::de::MapAccess<'de>>(self, m: M) -> Result<$ty, M::Error> {
                        $obj::deserialize(serde::de::value::MapAccessDeserializer::new(m)).map($ty)
                    }
                }
                d.deserialize_any(V)
            }
        }
        impl Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(s)
            }
        }
        impl JsonSchema for $ty {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($ty).into()
            }
            fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
                let obj = g.subschema_for::<$obj>();
                schemars::json_schema!({ "anyOf": [ { "type": "string" }, obj ] })
            }
        }
    };
}

/// Video size: a preset (`"16:9"`, `"9:16"`, `"1:1"`, `"4:5"`, `"720p"`,
/// `"1080p"`, `"4k"`, …) or `[width, height]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Size {
    Preset(String),
    Wh([u32; 2]),
}

/// The whole video.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    /// Frame size preset or `[w, h]`. Default `"16:9"` (1920×1080).
    pub size: Option<Size>,
    /// Frames per second. Default 30.
    pub fps: Option<f64>,
    /// Default background of every scene (colour or gradient). Default `"$bg"`.
    pub background: Option<Paint>,
    /// Colour tokens, fonts and text styles.
    pub theme: Option<Theme>,
    /// Reusable scenes and layers, referenced with `{ "use": "name", "with": {…}, …overrides }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defs: Option<BTreeMap<String, serde_json::Value>>,
    /// The scenes, in order.
    #[serde(deserialize_with = "de::scenes")]
    #[schemars(with = "Vec<de::SceneItem>")]
    pub scenes: Vec<Scene>,
    /// Default transition between scenes. Default crossfade 0.5 s.
    pub transition: Option<Transition>,
    /// Named times on the global timeline, referenced as `"@name"`.
    pub markers: Option<BTreeMap<String, Num>>,
    /// Music and sound effects.
    pub audio: Option<Vec<AudioCue>>,
    /// Generated, mastered music (`anikit-video docs music`): `{"style":"lofi","bpm":84,"key":"Dm","hits":["@bar5"]}`.
    /// Fills the video's length; `@beatN`/`@barN` markers follow its exact grid.
    #[schemars(with = "Option<serde_json::Map<String, serde_json::Value>>")]
    pub music: Option<serde_json::Value>,
    /// Burned-in captions.
    pub captions: Option<Captions>,
    /// Extra fonts: family name → file path.
    pub fonts: Option<BTreeMap<String, String>>,
    /// Override the total duration (truncate, or hold the last frame).
    pub duration: Option<Num>,
    /// Finishing look: a preset (`none clean cinematic dreamy punchy noir vintage`) or
    /// `{ "preset": "cinematic", "bloom": 0.4, … }`. Default none.
    #[schemars(with = "Option<PostValue>")]
    pub post: Option<serde_json::Value>,
    /// Encoder tuning (the built-in ak264 MP4 encoder): `{ "aq": 0.8 }`.
    pub encode: Option<EncodeSpec>,
    /// Motion blur on moving content: `true` (180° shutter), `false`, or `{ "shutter": 90 }` (degrees).
    /// Default: on at `--quality high` only (it renders several sub-frames per moving frame).
    pub motion_blur: Option<MotionBlur>,
    /// Named 2D sub-compositions: placed with a `composition` layer, or used as a texture in 3D.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compositions: Option<BTreeMap<String, Composition>>,
}

/// One primitive of a `draw` layer: exactly one of `d`, `points`, `circle`, `rect`, `line`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DrawItem {
    /// SVG path data (`"M0 0 L100 0 Q150 50 200 0 Z"`).
    pub d: Option<String>,
    /// A polyline `[[x, y], …]` (`closed` for a polygon, `smooth` for a curve through the points).
    pub points: Option<Vec<[f64; 2]>>,
    pub closed: Option<bool>,
    pub smooth: Option<bool>,
    /// `[cx, cy, r]`.
    pub circle: Option<[f64; 3]>,
    /// `[x, y, w, h]` (with `radius` for rounded corners).
    pub rect: Option<[f64; 4]>,
    pub radius: Option<f64>,
    /// `[x1, y1, x2, y2]`.
    pub line: Option<[f64; 4]>,
    /// Fill colour (none by default; `points`, `circle`, `rect` and `d` can be filled).
    pub fill: Option<String>,
    /// Stroke colour (default `$text` when `width` is set; lines always stroke).
    pub stroke: Option<String>,
    /// Stroke width in px (default 2 when stroked).
    pub width: Option<f64>,
    /// Per-vertex stroke colours for `points` (one per point; each segment is a gradient).
    pub colors: Option<Vec<String>>,
    /// `butt` (default), `round`, `square`.
    pub cap: Option<String>,
    /// `miter` (default), `round`, `bevel`.
    pub join: Option<String>,
    /// 0..1, multiplied with the layer's opacity.
    pub opacity: Option<f64>,
}

/// `post`: a preset name or a preset with overrides.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PostValue {
    /// `none clean cinematic dreamy punchy noir vintage`.
    Preset(String),
    /// `{ "preset": "cinematic", …any post setting… }`.
    Custom(serde_json::Map<String, serde_json::Value>),
}

/// `encode`: encoder tuning.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EncodeSpec {
    /// Adaptive quantisation strength, 0 (off, default) to 2: spends bits on flat areas
    /// (gradients, skies) and saves them on busy texture.
    pub aq: Option<f64>,
    /// Constant quantiser 0–51 (lower = better and bigger). Default 23 (19 high, 28 draft). `render --crf` overrides.
    pub crf: Option<f64>,
}

/// `motionBlur`: on/off, or a shutter angle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MotionBlur {
    On(bool),
    Shutter {
        /// Shutter angle in degrees (360 = the whole frame interval). Default 180.
        shutter: f64,
    },
}

/// A 2D sub-composition with its own size and clock: layers (one scene) or scenes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Composition {
    /// Size preset or `[w, h]`. Default: the video size.
    pub size: Option<Size>,
    /// Seconds. Default: the length of its layers' animation (`"auto"`), or the sum of its scenes.
    pub duration: Option<Num>,
    /// Background (colour or gradient). Default transparent.
    pub background: Option<Paint>,
    /// The layers (one scene).
    #[serde(default, deserialize_with = "de::opt_layers")]
    #[schemars(with = "Option<Vec<de::LayerItem>>")]
    pub layers: Option<Vec<Layer>>,
    /// Or several scenes with transitions.
    #[serde(default, deserialize_with = "de::opt_scenes")]
    #[schemars(with = "Option<Vec<de::SceneItem>>")]
    pub scenes: Option<Vec<Scene>>,
    /// Default transition between its scenes.
    pub transition: Option<Transition>,
    /// Loop wherever it is used (default false: hold the last frame).
    #[serde(rename = "loop")]
    pub loop_: Option<bool>,
}

/// Theme: colour tokens (`"$name"`), font roles and text styles.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    /// Colour tokens: `primary`, `secondary`, `accent`, `bg`, `surface`, `text`, `muted`, or your own.
    pub colors: Option<BTreeMap<String, String>>,
    /// Font roles: `heading`, `body`, `mono`.
    pub fonts: Option<BTreeMap<String, String>>,
    /// Default corner radius for boxes.
    pub radius: Option<f64>,
    /// Text styles: `title`, `heading`, `subtitle`, `body`, `caption`, `label`, `code`, or your own.
    pub styles: Option<BTreeMap<String, TextStyle>>,
    /// Material defaults every component takes unless its props say otherwise.
    pub material: Option<Material>,
}

/// Art-direction defaults for components (a look's finish). Explicit component props win.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// `flat`, `glass`, `outline`, `paper` or `solid`.
    pub surface: Option<String>,
    /// Shadow depth 0..1.
    pub shadow: Option<f64>,
    /// Glow intensity 0..1 (0 = none).
    pub glow: Option<f64>,
    /// Texture (film grain) 0..1.
    pub texture: Option<f64>,
    /// `snappy`, `soft`, `bouncy`, `mechanical` or `smooth`.
    pub motion: Option<String>,
}

/// A named text style.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    pub font: Option<String>,
    pub size: Option<Num>,
    pub weight: Option<Weight>,
    pub italic: Option<bool>,
    pub color: Option<Paint>,
    pub line_height: Option<f64>,
    pub letter_spacing: Option<Num>,
    pub uppercase: Option<bool>,
    pub align: Option<String>,
}

/// Font weight: 100..900 or a name (`"bold"`, `"semibold"`, …).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Weight {
    Num(f64),
    Name(String),
}

/// A colour or gradient: a CSS string (`"#fff"`, `"$primary"`,
/// `"linear-gradient(90deg, #f00, #00f)"`) or a gradient object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Paint {
    Css(String),
    Gradient(Gradient),
}

/// Gradient object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Gradient {
    /// `linear` (default) or `radial`.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Linear angle in degrees (CSS convention: 180 = top to bottom). Default 180.
    pub angle: Option<f64>,
    /// Colours, evenly spaced, or `[offset, colour]` pairs.
    pub stops: Vec<serde_json::Value>,
    /// Radial centre `[x, y]` as fractions of the box. Default `[0.5, 0.5]`.
    pub center: Option<[f64; 2]>,
    /// Radial radius as a fraction of the box diagonal / 2. Default 1.
    pub radius: Option<f64>,
}

/// One scene.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub id: Option<String>,
    /// Free text for the author (ignored by the renderer): what this shot
    /// shows and why, e.g. the reference shot it recreates (`analyze --spec`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Reuse: the name of an entry in top-level `defs`; other keys here override it
    /// (expanded before typing, see `spec/reuse.rs`).
    #[serde(rename = "use", default, skip_serializing_if = "Option::is_none")]
    pub use_def: Option<String>,
    /// Values for `"$param"` placeholders in the `use`d def.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with: Option<BTreeMap<String, serde_json::Value>>,
    /// Emit this many copies; `"$i"` (0-based) and `"$n"` are available, and
    /// `"=…"` strings are arithmetic (`"x": "=160 + $i * 420"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,
    /// Seconds (or a time string), or `"auto"`. Default 3.
    pub duration: Option<Num>,
    pub background: Option<Paint>,
    /// How this scene comes in from the previous one.
    pub transition: Option<Transition>,
    pub camera: Option<Camera>,
    /// Finishing look for this scene (overrides the video's `post`; `"none"` turns it off).
    #[schemars(with = "Option<PostValue>")]
    pub post: Option<serde_json::Value>,
    /// Times relative to the scene start.
    pub markers: Option<BTreeMap<String, Num>>,
    /// Drawn bottom to top.
    #[serde(deserialize_with = "de::layers")]
    #[schemars(with = "Vec<de::LayerItem>")]
    pub layers: Vec<Layer>,
    /// Audio cues relative to the scene start.
    pub audio: Option<Vec<AudioCue>>,
}

/// Transition: a type name or an object.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition(pub TransitionObj);
string_or_struct!(Transition, TransitionObj, |s| TransitionObj { kind: Some(s.to_string()), ..Default::default() });

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitionObj {
    /// `cut`, `crossfade`, `fadeBlack`, `fadeWhite`, `slide`, `push`, `zoom`, `wipe`, `iris`, `blur`, `glitch`, `morphCut`.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub duration: Option<Num>,
    pub ease: Option<String>,
    /// `left`, `right`, `up`, `down`.
    pub direction: Option<String>,
    pub color: Option<String>,
    pub softness: Option<f64>,
    pub origin: Option<[Num; 2]>,
    pub amount: Option<f64>,
    pub intensity: Option<f64>,
    /// `type: "shader"`: a gl-transitions style shader (`vec4 transition(vec2 uv)` with
    /// `progress`, `getFromColor`, `getToColor`) or a built-in transition name.
    pub shader: Option<String>,
    /// Uniforms for a shader transition.
    pub uniforms: Option<BTreeMap<String, serde_json::Value>>,
}

/// Scene camera.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    pub x: Option<Num>,
    pub y: Option<Num>,
    pub zoom: Option<Num>,
    pub rotation: Option<Num>,
    /// Depth in focus (layers at this `depth` are sharp). Default 0. Animatable (focus pulls).
    pub focus: Option<Num>,
    /// Depth-of-field strength: blur σ in px per unit of |depth − focus|. Default 0 (off).
    pub aperture: Option<Num>,
    #[serde(default, deserialize_with = "de::opt_tweens")]
    #[schemars(with = "Option<de::AnimateItem>")]
    pub animate: Option<Vec<Tween>>,
    pub shake: Option<Shake>,
}

/// Camera shake.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Shake {
    /// Pixels. Default 8.
    pub amount: Option<f64>,
    /// Hz. Default 6.
    pub frequency: Option<f64>,
    pub at: Option<Num>,
    pub duration: Option<Num>,
    /// Shake rotation in degrees. Default 0.
    pub rotation: Option<f64>,
}

/// Anchor: `[ax, ay]` in 0..1 or a name (`center`, `topLeft`, …).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Anchor {
    Name(String),
    Xy([f64; 2]),
}

/// `scale`: one number (or expression) or `[sx, sy]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Scale {
    One(Num),
    Xy([Num; 2]),
}

/// A layer. One flat type for every kind; `type` picks the kind (and is
/// inferred when obvious). Keys that don't apply to the kind are warned about.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    /// `text`, `shape`, `image`, `video`, `group`, `box`, `component`. Inferred when omitted.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub id: Option<String>,
    /// Reuse: the name of an entry in top-level `defs`; other keys here override it
    /// (expanded before typing, see `spec/reuse.rs`).
    #[serde(rename = "use", default, skip_serializing_if = "Option::is_none")]
    pub use_def: Option<String>,
    /// Values for `"$param"` placeholders in the `use`d def.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with: Option<BTreeMap<String, serde_json::Value>>,
    /// Emit this many copies; `"$i"` (0-based) and `"$n"` are available, and
    /// `"=…"` strings are arithmetic (`"x": "=160 + $i * 420"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,

    // ---- transform & appearance (all kinds)
    pub x: Option<Num>,
    pub y: Option<Num>,
    pub anchor: Option<Anchor>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub scale: Option<Scale>,
    pub scale_x: Option<Num>,
    pub scale_y: Option<Num>,
    /// Degrees, clockwise.
    pub rotation: Option<Num>,
    pub skew_x: Option<Num>,
    pub skew_y: Option<Num>,
    pub opacity: Option<Num>,
    pub blur: Option<Num>,
    pub blend: Option<String>,
    /// Distance from the focal plane (0 = the plane, positive = farther, negative = nearer).
    /// Farther layers move less when the camera pans (parallax) and blur with the camera's `aperture`.
    pub depth: Option<Num>,
    pub shadow: Option<Shadow>,
    pub glow: Option<Glow>,
    /// `true` clips children to the layer box, or a clip shape.
    pub clip: Option<Clip>,
    /// A layer whose alpha masks this one.
    pub mask: Option<Box<Layer>>,
    pub visible: Option<bool>,

    // ---- timing & animation
    pub start: Option<Num>,
    pub end: Option<Num>,
    pub duration: Option<Num>,
    pub enter: Option<Preset>,
    pub exit: Option<Preset>,
    /// Repeating motion: `float`, `pulse`, `spin`, `wiggle`, `breathe`, `blink`
    /// (for `video` layers, a bool: loop the clip).
    #[serde(rename = "loop")]
    pub loop_: Option<LoopValue>,
    #[serde(default, deserialize_with = "de::opt_tweens")]
    #[schemars(with = "Option<de::AnimateItem>")]
    pub animate: Option<Vec<Tween>>,
    /// Delay children's animations by index × seconds (group, box, split text).
    pub stagger: Option<Stagger>,

    // ---- text
    pub text: Option<String>,
    pub style: Option<StyleRef>,
    pub font: Option<String>,
    pub size: Option<Num>,
    pub weight: Option<Weight>,
    pub italic: Option<bool>,
    pub color: Option<Paint>,
    pub align: Option<String>,
    pub line_height: Option<f64>,
    pub letter_spacing: Option<Num>,
    pub uppercase: Option<bool>,
    pub max_lines: Option<u32>,
    /// `chars`, `words`, `lines`.
    pub split: Option<String>,
    pub reveal: Option<Num>,
    pub value: Option<Num>,
    /// Kinetic text animation (TEXT module): shorthand like `"pop by char from center 0.4s 30ms outBack"`, or an object.
    pub kinetic: Option<serde_json::Value>,

    // ---- shape
    /// `rect`, `circle`, `ellipse`, `line`, `arrow`, `polygon`, `star`, `path`.
    pub shape: Option<String>,
    pub radius: Option<Num>,
    pub fill: Option<Paint>,
    pub stroke: Option<StrokeSpec>,
    pub stroke_width: Option<Num>,
    pub points: Option<Vec<[Num; 2]>>,
    pub sides: Option<u32>,
    pub inner_radius: Option<f64>,
    pub d: Option<String>,
    pub head_size: Option<f64>,
    pub trim_start: Option<Num>,
    pub trim_end: Option<Num>,

    // ---- image / video
    pub src: Option<String>,
    /// Images: `contain`, `cover`, `fill`, `none`. Text: `true` shrinks the
    /// size (never grows it) so the widest line fits `width` (default 85% of
    /// the parent), keeping line breaks.
    pub fit: Option<FitValue>,
    pub speed: Option<f64>,
    pub volume: Option<f64>,

    // ---- containers
    /// Children of a `group`.
    #[serde(default, deserialize_with = "de::opt_layers")]
    #[schemars(with = "Option<Vec<de::LayerItem>>")]
    pub layers: Option<Vec<Layer>>,
    /// Children of a `box`.
    #[serde(default, deserialize_with = "de::opt_layers")]
    #[schemars(with = "Option<Vec<de::LayerItem>>")]
    pub children: Option<Vec<Layer>>,

    /// Per-frame script: returns props to set this frame, e.g. `"#{ x: 960 + 200 * sin(t), fill: hsl(t * 90, 80, 60) }"`.
    /// Sees `t T gt p i n w h fps frame beat bar level bpm`. Runs every frame (a few µs).
    pub update: Option<serde_json::Value>,

    // ---- draw
    /// Raw drawing: primitives in frame pixels (the layer defaults to the parent's box), each
    /// with its own fill and stroke. The layer's transforms, opacity, animation and `trimStart`/`trimEnd` apply.
    pub draw: Option<Vec<DrawItem>>,

    // ---- composition (`trimStart`, `speed`, `loop: true` and `fit` apply too)
    /// Name of an entry in top-level `compositions`.
    pub composition: Option<String>,

    // ---- component
    pub component: Option<String>,
    pub props: Option<serde_json::Value>,
    pub progress: Option<Num>,

    // ---- shader (SHADER module: `eval/shader.rs`, crate `anikit-shader`)
    /// Fragment shader: Shadertoy-style GLSL (`void mainImage(out vec4 fragColor, in vec2 fragCoord)`),
    /// WGSL (`fn mainImage(fragCoord: vec2f) -> vec4f`), or a built-in name (`aurora`, `liquid-chrome`, …;
    /// `anikit-video info --shaders`). `src: "fx.glsl"` loads one from a file.
    pub shader: Option<String>,
    /// Shader uniforms by name: a number, `[x, y(, z, w)]`, a colour or an `"=expr"`. Animatable by name in `animate`.
    pub uniforms: Option<BTreeMap<String, serde_json::Value>>,
    /// Shader inputs `iChannel0..3`: `"below"`, `"noise"`, an image/video path, `{ "layers": [...] }`,
    /// optionally with `"wrap": "repeat"|"mirror"`, `"filter": "nearest"`, `"mipmaps": true`.
    pub channels: Option<Vec<serde_json::Value>>,
    /// Shader as a filter: `iChannel0` is everything drawn below, and the output replaces the frame.
    pub filter: Option<bool>,
    /// Shader: anti-aliasing (adaptive 2×2 where neighbours differ). Default on for shader layers
    /// unless the source says `// anikit:aa off`; off for filters. `false` is up to 2× faster.
    pub supersample: Option<bool>,
    /// Shader: use `fragColor.a` as alpha (default: opaque, like Shadertoy).
    pub alpha: Option<bool>,
    /// Shader: Shadertoy-style buffer passes `"A"`…`"D"`, rendered in order before the image
    /// pass at full float precision. A channel `"A"` reads buffer A: this frame's if A ran
    /// earlier in the order, else the previous frame's (feedback, for simulations).
    pub buffers: Option<BTreeMap<String, ShaderBuffer>>,
}

/// One buffer pass of a shader layer (`buffers.A`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShaderBuffer {
    /// GLSL/WGSL source or a built-in name, as for a shader layer.
    pub shader: Option<String>,
    /// Load the source from a file.
    pub src: Option<String>,
    /// `iChannel0..3`: `"A"`…`"D"` (buffers), `"noise"`, or `"none"`; options as for layer channels.
    pub channels: Option<Vec<serde_json::Value>>,
}

/// `style`: a theme text-style name (text layers) or a box style (box layers).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum StyleRef {
    Name(String),
    Box(BoxStyle),
}

impl<'de> Deserialize<'de> for StyleRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = StyleRef;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a text style name or a box style object")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<StyleRef, E> {
                Ok(StyleRef::Name(s.to_string()))
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, m: M) -> Result<StyleRef, M::Error> {
                BoxStyle::deserialize(serde::de::value::MapAccessDeserializer::new(m)).map(StyleRef::Box)
            }
        }
        d.deserialize_any(V)
    }
}

/// Flexbox style of a `box` (and of a box's children: `grow`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoxStyle {
    /// `row` (default) or `column`.
    pub direction: Option<String>,
    pub gap: Option<Num>,
    /// Number, `[v, h]` or `[t, r, b, l]`.
    pub padding: Option<Padding>,
    /// Cross axis: `start`, `center` (default), `end`, `stretch`.
    pub align: Option<String>,
    /// Main axis: `start`, `center`, `end`, `spaceBetween`, `spaceAround`, `spaceEvenly`.
    pub justify: Option<String>,
    pub wrap: Option<bool>,
    pub grow: Option<f64>,
    pub background: Option<Paint>,
    pub radius: Option<Num>,
    /// `"2px #fff"`.
    pub border: Option<String>,
    /// Grid with this many equal columns.
    pub columns: Option<u32>,
}

/// Padding: one value, `[v, h]` or `[t, r, b, l]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Padding {
    One(Num),
    Many(Vec<Num>),
}

/// Drop shadow.
#[derive(Clone, Debug, PartialEq)]
pub struct Shadow(pub ShadowObj);
string_or_struct!(Shadow, ShadowObj, |s| ShadowObj { color: (!s.is_empty()).then(|| s.to_string()), ..Default::default() });

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShadowObj {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub blur: Option<f64>,
    pub color: Option<String>,
}

/// Glow.
#[derive(Clone, Debug, PartialEq)]
pub struct Glow(pub GlowObj);
string_or_struct!(Glow, GlowObj, |s| GlowObj { color: (!s.is_empty()).then(|| s.to_string()), ..Default::default() });

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GlowObj {
    pub radius: Option<f64>,
    pub intensity: Option<f64>,
    pub color: Option<String>,
}

/// `clip`: `true` (the layer box) or a shape layer.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Clip {
    On(bool),
    Shape(Box<Layer>),
}

/// Stroke: a colour string or an object.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeSpec(pub StrokeObj);
string_or_struct!(StrokeSpec, StrokeObj, |s| StrokeObj { color: Some(s.to_string()), ..Default::default() });

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StrokeObj {
    pub color: Option<String>,
    pub width: Option<Num>,
    /// `butt`, `round`, `square`.
    pub cap: Option<String>,
    /// `miter`, `round`, `bevel`.
    pub join: Option<String>,
    pub dash: Option<Vec<f64>>,
    pub dash_offset: Option<f64>,
}

/// `loop`: a preset (name or object), or a bool for video clips.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum LoopValue {
    Clip(bool),
    Preset(Preset),
}

impl<'de> Deserialize<'de> for LoopValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = LoopValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a preset name, a preset object or a bool")
            }
            fn visit_bool<E: serde::de::Error>(self, b: bool) -> Result<LoopValue, E> {
                Ok(LoopValue::Clip(b))
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<LoopValue, E> {
                Ok(LoopValue::Preset(Preset(PresetObj { preset: Some(s.to_string()), ..Default::default() })))
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, m: M) -> Result<LoopValue, M::Error> {
                PresetObj::deserialize(serde::de::value::MapAccessDeserializer::new(m))
                    .map(|p| LoopValue::Preset(Preset(p)))
            }
        }
        d.deserialize_any(V)
    }
}

/// An animation preset: a name or an object.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset(pub PresetObj);
string_or_struct!(Preset, PresetObj, |s| PresetObj { preset: Some(s.to_string()), ..Default::default() });

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresetObj {
    /// Preset name (`fadeUp`, `popIn`, `drawOn`, `typewriter`, …).
    pub preset: Option<String>,
    pub delay: Option<Num>,
    pub duration: Option<Num>,
    pub ease: Option<String>,
    pub spring: Option<SpringSpec>,
    pub stagger: Option<Stagger>,
    /// Move distance (px or unit string) for move presets.
    pub distance: Option<Num>,
    pub direction: Option<String>,
    /// Loop amplitude.
    pub amount: Option<f64>,
    /// Loop period in seconds.
    pub period: Option<f64>,
    /// Typewriter characters per second.
    pub cps: Option<f64>,
}

/// Stagger: seconds per index, or an object.
#[derive(Clone, Debug, PartialEq)]
pub struct Stagger(pub StaggerObj);

impl<'de> Deserialize<'de> for Stagger {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Stagger;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "seconds per item or a stagger object")
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Stagger, E> {
                Ok(Stagger(StaggerObj { each: Some(v), ..Default::default() }))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Stagger, E> {
                self.visit_f64(v as f64)
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Stagger, E> {
                self.visit_f64(v as f64)
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, m: M) -> Result<Stagger, M::Error> {
                StaggerObj::deserialize(serde::de::value::MapAccessDeserializer::new(m)).map(Stagger)
            }
        }
        d.deserialize_any(V)
    }
}
impl Serialize for Stagger {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl JsonSchema for Stagger {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Stagger".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let obj = g.subschema_for::<StaggerObj>();
        schemars::json_schema!({ "anyOf": [ { "type": "number" }, obj ] })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaggerObj {
    /// Seconds between consecutive items.
    pub each: Option<f64>,
    /// Spread the delays over this many seconds instead.
    pub total: Option<f64>,
    /// `start` (default), `center`, `end`, `random`.
    pub from: Option<String>,
    /// Ease applied to the delay distribution.
    pub ease: Option<String>,
}

/// Spring: a preset name or parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SpringSpec {
    Name(String),
    Params(SpringParams),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpringParams {
    pub stiffness: Option<f64>,
    pub damping: Option<f64>,
    pub mass: Option<f64>,
    pub velocity: Option<f64>,
}

/// One tween in `animate`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tween {
    /// Start, relative to the layer's start. `"+0.2"` = after the previous tween.
    pub at: Option<Num>,
    pub duration: Option<Num>,
    pub ease: Option<String>,
    pub spring: Option<SpringSpec>,
    /// Animate from these values to the layer's own.
    pub from: Option<BTreeMap<String, AnimValue>>,
    /// Animate to these values (and hold).
    pub to: Option<BTreeMap<String, AnimValue>>,
    /// `{ prop: [v0, v1, …] }` or `{ prop: [[time 0..1, value], …] }`.
    pub keyframes: Option<BTreeMap<String, Vec<AnimValue>>>,
    /// A preset run as a tween at `at`.
    pub preset: Option<String>,
    /// Repeat count (`-1` = forever).
    pub repeat: Option<i32>,
    pub yoyo: Option<bool>,
}

/// An audio cue: a file (`src`) or a built-in sound effect (`sfx`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioCue {
    /// Audio file (mp3, wav, ogg, flac, aac/m4a).
    pub src: Option<String>,
    /// Built-in effect: `click`, `tick`, `pop`, `whoosh`, `swoosh`, `rise`, `riser`, `drop`, `impact`, `ding`, `chime`, `typing`, `success`.
    pub sfx: Option<String>,
    pub at: Option<Num>,
    /// 0..1 or `"-6dB"`.
    pub volume: Option<Num>,
    pub trim_start: Option<Num>,
    pub duration: Option<Num>,
    pub fade_in: Option<f64>,
    pub fade_out: Option<f64>,
    #[serde(rename = "loop")]
    pub loop_: Option<bool>,
    /// Duck this track while voice plays: `true`, or the reduction in dB (default 12).
    pub duck: Option<serde_json::Value>,
    /// This track is a voice-over (others duck under it).
    pub voice: Option<bool>,
    /// SFX pitch multiplier.
    pub pitch: Option<f64>,
}

/// Captions: a list of cues, or a subtitle file with styling.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Captions {
    Cues(Vec<CaptionCue>),
    File(CaptionFile),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CaptionFile {
    pub src: Option<String>,
    pub cues: Option<Vec<CaptionCue>>,
    pub style: Option<String>,
    pub y: Option<Num>,
    /// Colour of the word being spoken (default `$accent`).
    pub highlight: Option<String>,
    pub color: Option<String>,
    /// A pill behind the caption line.
    pub background: Option<String>,
    /// A box behind the word being spoken (TikTok style).
    pub highlight_box: Option<String>,
    /// Words per caption page (default 4; 1 = one word at a time).
    pub max_words: Option<usize>,
    pub uppercase: Option<bool>,
    /// Words not yet spoken: `dim` (default), `hidden` (build up) or `show`.
    pub upcoming: Option<String>,
    /// Font size (default 6vh).
    pub size: Option<Num>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CaptionCue {
    pub text: String,
    pub at: Num,
    pub end: Num,
    /// Word timings `[text, at, end]`.
    pub words: Option<Vec<(String, f64, f64)>>,
}

/// The JSON Schema of the spec, as pretty JSON.
pub fn schema_json() -> String {
    use validate as v;
    let mut s = schemars::schema_for!(Spec);
    s.insert("$id".into(), "https://anikit.dev/video/spec.schema.json".into());
    s.insert("title".into(), "Anikit Video spec".into());
    let mut root = serde_json::to_value(&s).unwrap();
    // Allowed words (strings are validated by the compiler; the schema lists them for tools and agents).
    let enums: &[(&str, &str, &[&str])] = &[
        ("Layer", "type", v::LAYER_TYPES),
        ("Layer", "shape", v::SHAPES),
        ("Layer", "blend", v::BLENDS),
        ("Layer", "split", v::SPLITS),
        ("Layer", "fit", v::FITS),
        ("Layer", "align", &["left", "center", "right", "justify"]),
        ("TransitionObj", "type", v::TRANSITIONS),
        ("TransitionObj", "direction", v::DIRECTIONS),
        ("BoxStyle", "direction", &["row", "column"]),
        ("BoxStyle", "align", v::BOX_ALIGNS),
        ("BoxStyle", "justify", v::JUSTIFIES),
        ("StrokeObj", "cap", &["butt", "round", "square"]),
        ("StrokeObj", "join", &["miter", "round", "bevel"]),
        ("StaggerObj", "from", v::STAGGER_FROM),
        ("AudioCue", "sfx", crate::audio::SFX_NAMES),
    ];
    for (def, prop, words) in enums {
        if let Some(p) = root.pointer_mut(&format!("/$defs/{def}/properties/{prop}")) {
            p["examples"] = serde_json::json!(words);
            let desc = p.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
            p["description"] = format!("{desc} One of: {}.", words.join(", ")).trim().into();
        }
    }
    if let Some(p) = root.pointer_mut("/$defs/Layer/properties/component") {
        let names: Vec<&str> = crate::components::KINDS.iter().map(|k| k.name).collect();
        p["examples"] = serde_json::json!(names);
        p["description"] = format!("Component kind. One of: {}.", names.join(", ")).into();
    }
    if let Some(p) = root.pointer_mut("/$defs/PresetObj/properties/preset") {
        let mut all: Vec<&str> = v::ENTER_PRESETS.to_vec();
        all.extend(v::EXIT_PRESETS.iter().filter(|x| !v::ENTER_PRESETS.contains(x)));
        all.extend(v::LOOP_PRESETS);
        p["examples"] = serde_json::json!(all);
    }
    serde_json::to_string_pretty(&root).unwrap()
}

/// `fit`: an image mode (`"cover"`) or, on text, `true`. Derefs to the word
/// (`"true"` / `"false"` for booleans) so callers match on strings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum FitValue {
    Flag(bool),
    Mode(String),
}

impl std::ops::Deref for FitValue {
    type Target = str;
    fn deref(&self) -> &str {
        match self {
            FitValue::Flag(true) => "true",
            FitValue::Flag(false) => "false",
            FitValue::Mode(m) => m,
        }
    }
}
