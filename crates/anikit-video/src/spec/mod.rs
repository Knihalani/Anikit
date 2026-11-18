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
    /// Sound design (`anikit-video docs sound`): `"auto"` derives whooshes, landings, ticks and
    /// transition sounds from the picture, pans them to their place on screen, puts them in one
    /// shared space, ducks the music under hits and masters to −14 LUFS / −1 dBTP.
    /// `{ "palette": "organic", "density": 0.5, "ambience": "city", "space": "room", "cues": [...] }`.
    #[schemars(with = "Option<serde_json::Value>")]
    pub sound: Option<serde_json::Value>,
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
    /// Default scene length: `"bar"`, `"2 beats"`, `"4 bars"` (at the music's tempo) or seconds.
    /// Scenes without a `duration` cut on that grid.
    pub cut: Option<Num>,
    /// Encoder tuning (the built-in ak264 MP4 encoder): `{ "aq": 0.8 }`.
    pub encode: Option<EncodeSpec>,
    /// Motion blur on moving content: `true` (180° shutter), `false`, or `{ "shutter": 90 }` (degrees).
    /// Default: on at `--quality high` only (it renders several sub-frames per moving frame).
    pub motion_blur: Option<MotionBlur>,
    /// Named 2D sub-compositions: placed with a `composition` layer, or used as a texture in 3D.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compositions: Option<BTreeMap<String, Composition>>,
    /// Modular layout grid for every scene (a scene's own `grid` replaces it). Layers snap to it with `cell`
    /// (`anikit-video docs grid`): `{ "columns": 12, "rows": 8, "margin": 96, "gutter": 24, "baseline": 12, "areas": { "head": "1-8/1-3" } }`.
    pub grid: Option<GridSpec>,
}

/// A modular grid: columns × rows of fields inside margins, separated by gutters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GridSpec {
    /// Number of columns. Default 12.
    pub columns: Option<u32>,
    /// Number of rows. Default 8 (6 on 9:16 is not inferred: set it).
    pub rows: Option<u32>,
    /// Outer margin: a length, or `[vertical, horizontal]`, or `[top, right, bottom, left]`. Default 5% of the frame height (graphics-safe).
    pub margin: Option<serde_json::Value>,
    /// Gap between fields: a length or `[column gap, row gap]`. Default 2% of the frame height.
    pub gutter: Option<serde_json::Value>,
    /// Baseline grid in px: text inside a `cell` gets its line height rounded to a multiple of it
    /// and its top on a baseline line.
    pub baseline: Option<f64>,
    /// Named areas: name → cell range (`"1-8/1-3"` = columns 1–8, rows 1–3).
    pub areas: Option<BTreeMap<String, String>>,
    /// Draw the grid: `true` (over the scene, to check a layout) or `"under"` (beneath the layers, as a
    /// visible design element, e.g. brand guidelines). Fields and baseline lines when set.
    #[schemars(with = "Option<serde_json::Value>")]
    pub show: Option<serde_json::Value>,
    /// Colour of the drawn grid. Default a translucent magenta over, a 25% ink hairline under.
    pub color: Option<String>,
}

/// `cutout`: an edge name or the full settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CutoutValue {
    Edge(String),
    Full(CutoutSpec),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutoutSpec {
    /// `torn` (default), `scissor` or `clean`.
    pub edge: Option<String>,
    /// How far the edge bites in, px. Default 14 (torn), 18 (scissor).
    pub amount: Option<f64>,
    /// Paper rim around the cut, px. Default 8 for torn, 0 otherwise.
    pub rim: Option<f64>,
    /// Rim colour. Default `"#f4f1ea"`.
    pub rim_color: Option<String>,
    /// A tight printed shadow under the piece. Default true.
    pub shadow: Option<bool>,
    pub seed: Option<u32>,
}

/// `inkTexture`: amount, or the full settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum InkValue {
    Amount(f64),
    Full(InkSpec),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InkSpec {
    /// How much ink thins out in the worst spots (0..1). Default 0.35. Animatable as `inkAmount`, or `"=expr"`.
    pub amount: Option<Num>,
    /// Size of the blotches in px. Default 28.
    pub scale: Option<f64>,
    pub seed: Option<u32>,
}

/// `brush`: on, or the full settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BrushValue {
    On(bool),
    Full(BrushSpec),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrushSpec {
    /// Taper lengths at the start and end, as fractions of the stroke. Default `[0.12, 0.25]`.
    pub taper: Option<[f64; 2]>,
    /// Width wobble (0..1). Default 0.35.
    pub pressure: Option<f64>,
    /// Dry brush (0..1): bristles that skip, more towards the end. Default 0.
    pub dry: Option<f64>,
    pub seed: Option<u32>,
}

/// `boil`: px, or the full settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BoilValue {
    Px(f64),
    Full(BoilSpec),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoilSpec {
    /// Max offset in px. Default 2.
    pub amount: Option<f64>,
    /// Max rotation in degrees. Default 0.
    pub rotate: Option<f64>,
    /// New positions per second. Default 12.
    pub fps: Option<f64>,
    pub seed: Option<u32>,
}

/// `halftone`: a cell size in px, or the full settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum HalftoneValue {
    Cell(f64),
    Full(HalftoneSpec),
}

/// A print halftone screen over the layer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HalftoneSpec {
    /// Screen pitch in px (distance between dot centres). Default 10. Animatable as `halftoneCell` (also `halftoneAngle`,
    /// `halftoneGain`), or `"=expr"`.
    pub cell: Option<Num>,
    /// Screen angle in degrees. Default 45 (black ink); 15/75/0 for other plates.
    pub angle: Option<Num>,
    /// `dot` (default), `line` or `square`.
    pub shape: Option<String>,
    /// Ink colour. Default `"source"`: each dot keeps the layer's colour there.
    pub ink: Option<String>,
    /// Paper colour under the dots, or `"none"` (default): only the dots remain.
    pub paper: Option<String>,
    /// Dot gain: > 1 darkens (fatter dots), < 1 lightens. Default 1.
    pub gain: Option<Num>,
}

/// `pattern`: a kind with an optional spacing (`"dots 32"`), or the full settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PatternValue {
    Short(String),
    Full(PatternSpec),
}

/// A generated pattern filling the layer's box.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatternSpec {
    /// `dots`, `grid`, `lines`, `stripes`, `checker`, `triangles`, `diamonds`, `crosses`, `rings`, `waves`, `zigzag`, `truchet`, `arcs`,
    /// `scales`, `bricks`, or `tile` (your own `tile` motif; implied when `tile` is given).
    #[serde(default)]
    pub kind: String,
    /// Repeat distance in px. Default 40.
    pub spacing: Option<f64>,
    /// Mark size as a fraction of `spacing` (dot diameter, line weight…). Default depends on the kind.
    pub mark: Option<f64>,
    /// Rotation of the whole pattern in degrees.
    pub angle: Option<f64>,
    /// Seed for random kinds (`truchet`, `arcs`) and `jitter`.
    pub seed: Option<u32>,
    /// Random offset of each mark, as a fraction of `spacing`. Default 0.
    pub jitter: Option<f64>,
    /// Mark size from `[a, b]` across the box (left→right; with `angle` it follows the angle): a size ramp like a halftone. Fractions of `mark`.
    pub ramp: Option<[f64; 2]>,
    /// Your own motif for `kind: "tile"` (the default when `tile` is given): SVG path data in px, centred on 0,0,
    /// drawn at every lattice point and scaled by `mark` (default 1) × `ramp`.
    pub tile: Option<String>,
    /// Random rotation of each mark in steps of 360/turns degrees, seeded (`4` = quarter turns, Truchet-style).
    pub turns: Option<u32>,
    /// Shift every other row by half the spacing (brick / hexagonal lattice).
    pub stagger: Option<bool>,
    /// Stroke the `tile` motif at this width in px instead of filling it (open lines: squiggles, ticks, zigzags).
    pub tile_stroke: Option<f64>,
    /// Size of each mark as an expression (multiplies `mark` and `ramp`): `u`, `v` (0..1 across the box), `x`, `y`
    /// (px from the centre), `d` (0 centre … 1 corner), `a` (angle), `r` (random per mark), `t`, plus the usual functions.
    /// On `lines`/`stripes` it sets the weight along each line (op art); on `waves`/`zigzag` the amplitude.
    /// Using `t` regenerates the pattern every frame. E.g. `"0.5 + 0.5 * sin(d * 18 - t * 3)"`.
    pub field: Option<String>,
}

/// `dots`: many points from an array or a CSV file (`x,y[,colour]` per line).
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DotsSpec {
    /// `[[x, y], …]` or `[[x, y, "#colour"], …]` in frame pixels, or a CSV path.
    pub data: serde_json::Value,
    /// Dot diameter in px (default 4).
    pub size: Option<f64>,
    /// Colour of dots without their own (default `$text`).
    pub color: Option<String>,
    /// Per-dot scale 0..1, an expression of `i` (index), `n` (count), `t`… e.g.
    /// `"=clamp(t * 2000 - i, 0, 1)"` to reveal dots in order. Default 1.
    pub appear: Option<String>,
    /// `circle` (default) or `square` (faster for very many tiny dots).
    pub shape: Option<String>,
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
    /// Lay out the scene's free-floating layers (no `x`/`y`) as one centred column:
    /// `true`, a gap in px, or `{ "gap": 24, "align": "start|center|end" }`. Expanded at load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<serde_json::Value>,
    /// Modular grid for this scene (replaces the video's `grid`); layers snap to it with `cell`.
    pub grid: Option<GridSpec>,
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
    /// `cut`, `crossfade`, `fadeBlack`, `fadeWhite`, `slide`, `push`, `zoom`, `wipe`, `iris`, `blur`, `glitch`, `morphCut`, `zoomThrough`, `reveal`, `blinds`, `split`, `whip`.
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
    /// `zoomThrough`: the `id` of a layer in the outgoing scene to fly into (the new scene shows through its shape).
    pub layer: Option<String>,
    /// `reveal`: `circle`, `diamond`, `square`, `star`, `blob`.
    pub shape: Option<String>,
    /// `blinds`: number of bars (default 8).
    pub count: Option<u32>,
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

/// `onNotes`: one copy of `layer` per note of the generated `music`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OnNotes {
    /// Track name or role (`drums`, `bass`, `lead`, `chords`, …). Default: every track.
    pub track: Option<String>,
    /// Only these keys: MIDI numbers or drum names (`kick`, `snare`, `hat`, `clap`, `crash`, …).
    pub keys: Option<Vec<serde_json::Value>>,
    /// Keep every n-th matching note (1 = all).
    pub every: Option<u32>,
    /// The layer to place on each note. `$t $dur $key $pitch $vel $beat $bar $i $n` are
    /// replaced per note; it starts on the note (its own `start` is an offset from it) and
    /// lasts the note's length unless it sets `duration`, `end` or `frames`.
    pub layer: serde_json::Value,
}

/// `frames`: one frame number, or a half-open range `[first, end)` (frames into the scene).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Frames {
    One(f64),
    Range([f64; 2]),
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
    /// Sound for this layer (`docs sound`): `"auto"`, `false` (silent), a sound name, or
    /// `{ "in": "swish", "land": "wood", "out": "air", "gain": -3 }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<serde_json::Value>")]
    pub sfx: Option<serde_json::Value>,
    /// Reuse: the name of an entry in top-level `defs`; other keys here override it
    /// (expanded before typing, see `spec/reuse.rs`).
    #[serde(rename = "use", default, skip_serializing_if = "Option::is_none")]
    pub use_def: Option<String>,
    /// Values for `"$param"` placeholders in the `use`d def.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with: Option<BTreeMap<String, serde_json::Value>>,
    /// A number: emit this many copies at spec level (`"$i"`, `"$n"`, `"=…"` arithmetic).
    /// An object: a render-time repeater (`{"count": 12, "offset": {"rotate": 30}, "stagger": 0.05}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<RepeatValue>,
    /// Motion trails: draw earlier-time copies under the layer, `{"count": 6, "delay": 0.04, "decay": 0.6}`.
    pub echo: Option<EchoSpec>,
    /// Follow a motion path (SVG path data in parent pixels): `x`/`y` come from the path at `progress`
    /// (0..1, animatable; default: linear over the layer's life).
    pub path: Option<String>,
    /// With `path`: rotate the layer along the path's tangent (`true`, or an extra angle in degrees).
    pub orient: Option<serde_json::Value>,
    /// Text on a path: SVG path data (layer-local pixels); glyphs ride it, shifted by `pathOffset` px (animatable).
    pub text_path: Option<String>,
    /// Text on a path: shift along the path in px (animatable as `pathOffset`).
    pub path_offset: Option<Num>,
    /// Stroke dash offset in px, added to `stroke.dashOffset` (animatable as `dashOffset`; `"=t*-120"` marches ants).
    pub dash_offset: Option<Num>,
    /// Shape modifier: round every corner of the outline by this radius (px).
    pub round_corners: Option<f64>,
    /// Shape modifier: zigzag the outline, `{"size": 12, "ridges": 20, "smooth": false}`.
    pub zigzag: Option<ZigzagSpec>,
    /// Shape modifier: pucker (< 0) or bloat (> 0) the outline, −1..1.
    pub pucker_bloat: Option<f64>,
    /// Shape modifier: grow (> 0) or shrink (< 0) the outline by this many px.
    pub offset_path: Option<f64>,
    /// Shape modifier: twist the outline around its centre by this many degrees (more at the rim).
    pub twist: Option<f64>,
    /// Shape modifier: noise-displace the outline's vertices, `{"amount": 10, "detail": 24, "speed": 1, "seed": 1}` (animated, deterministic).
    pub wiggle_path: Option<WigglePathSpec>,
    /// Text animator with a range selector (After Effects style): units inside the range take these props.
    /// `{"by": "char", "y": 40, "opacity": 0, "duration": 0.8}` sweeps a reveal; see `docs mograph`.
    pub animator: Option<AnimatorSpec>,
    /// Follow-through: a spring chases the layer's own motion, so every move overshoots and settles.
    /// `true`, a preset (`soft`, `bouncy`, `tight`, `heavy`) or `{"stiffness": 170, "damping": 12, "mass": 1, "props": ["x", "y", "rotation", "scale"]}`.
    #[schemars(with = "Option<serde_json::Value>")]
    pub inertia: Option<serde_json::Value>,
    /// Posterize time (After Effects style): this layer and its children move at this many frames per second
    /// (`12` animates "on twos" at 24 fps: a handmade, stop-motion feel). The video's frame rate is unchanged.
    pub posterize_time: Option<f64>,
    /// Group drag (overlapping action): child i follows the group's own motion i × drag seconds later,
    /// so a moving group trails and settles part by part. Pairs well with `inertia` on the group.
    pub drag: Option<f64>,
    /// Smear: stretch the layer along its direction of travel on fast moves (`true`, or strength; 1 = default).
    #[schemars(with = "Option<serde_json::Value>")]
    pub smear: Option<serde_json::Value>,
    /// Track matte: another layer of the scene (by `id`) whose alpha or luma shows this one.
    pub matte: Option<MatteSpec>,

    // ---- transform & appearance (all kinds)
    pub x: Option<Num>,
    pub y: Option<Num>,
    pub anchor: Option<Anchor>,
    /// Snap the layer to the scene's `grid`: an area name (`"head"`), a range `"1-6/2-4"`
    /// (columns 1–6, rows 2–4; `"7/1"` one field; `"1-6"` all rows), or `[col, row, cols, rows]`.
    /// Sets `x`, `y`, `width` (and `height`, except for text); `anchor` picks the point of the cell (default top left).
    pub cell: Option<serde_json::Value>,
    /// A group's own grid for its children's `cell`, inside the group's box.
    pub grid: Option<GridSpec>,
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
    /// Two-colour print look: luma mapped from the first colour (shadows) to the second (highlights).
    pub duotone: Option<Vec<String>>,
    /// Gradient map on luma: colours dark→light (tritone = 3), or `[offset, colour]` pairs.
    pub gradient_map: Option<Vec<serde_json::Value>>,
    /// Posterise to N tone levels (on luma when combined with `duotone`/`gradientMap`, else per channel).
    pub posterize: Option<u32>,
    /// Strength of `duotone` / `gradientMap` / `posterize` (0..1). Default 1. Animatable (`toneAmount`), or `"=expr"`.
    pub tone_amount: Option<Num>,
    /// Input luma range `[black, white]` (0..1) stretched to the full map before `duotone`/`gradientMap`:
    /// the pre-curve a print designer sets so a flat source still spans ink to paper. Default `[0, 1]`.
    pub tone_range: Option<[f64; 2]>,
    /// Print halftone screen: a cell size in px, or `{ "cell", "angle", "shape", "ink", "paper", "gain" }`.
    pub halftone: Option<HalftoneValue>,
    /// Uneven ink coverage (riso, screenprint, letterpress): amount 0..1, or `{ "amount", "scale": px, "seed" }`.
    /// Fixed to the frame like the paper, so moving shapes travel through it.
    pub ink_texture: Option<InkValue>,
    /// Collage cut-out: `"torn"`, `"scissor"` or `"clean"`, or `{ "edge", "amount": px, "rim": px, "rimColor", "shadow": bool, "seed" }`.
    /// Cuts the layer's box with that edge, backs it with a paper rim and a printed shadow. Needs a numeric `width`/`height` (or a `cell`).
    pub cutout: Option<CutoutValue>,
    /// Stepped time: the layer (and its children) animate at this many fps, or `"twos"`/`"threes"`/`"fours"`
    /// of the video's fps (stop-motion, hand-pulled print, cel looks) while the rest stays smooth.
    #[schemars(with = "Option<serde_json::Value>")]
    pub step: Option<serde_json::Value>,
    /// Boil: a seeded jitter re-drawn a few times a second: px, or `{ "amount": px, "rotate": deg, "fps": 12, "seed": 1 }`.
    /// On a `multiply` ink layer it reads as plates drifting out of register.
    pub boil: Option<BoilValue>,
    /// `true` clips children to the layer box, or a clip shape.
    pub clip: Option<Clip>,
    /// A layer whose alpha masks this one.
    pub mask: Option<Box<Layer>>,
    pub visible: Option<bool>,

    // ---- timing & animation
    /// When the layer appears (seconds into the scene). `delay` means the same.
    #[serde(alias = "delay")]
    pub start: Option<Num>,
    pub end: Option<Num>,
    pub duration: Option<Num>,
    /// Generated layers, one per note of the music: `{ "onNotes": { "track": "drums", "keys": ["kick"], "layer": {…} } }`.
    #[serde(rename = "onNotes")]
    pub on_notes: Option<OnNotes>,
    /// Timing in frames instead of seconds: `"frames": 12` (only frame 12) or `[12, 18]` (frames 12–17).
    pub frames: Option<Frames>,
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
    /// Trim the text box to the first line's cap height and the last line's baseline (CSS `text-box: cap alphabetic`),
    /// so `anchor` and grid cells align the letters, not the line box. Default on for text in a grid `cell`.
    pub text_box_trim: Option<bool>,
    pub max_lines: Option<u32>,
    /// Text: wrap at this width (px, `"40vw"`, `"60%"`) without fixing the box
    /// width (`width` wraps *and* sets the box).
    pub max_width: Option<Num>,
    /// `chars`, `words`, `lines`.
    pub split: Option<String>,
    pub reveal: Option<Num>,
    pub value: Option<Num>,
    /// Kinetic text animation (TEXT module): shorthand like `"pop by char from center 0.4s 30ms outBack"`, or an object.
    pub kinetic: Option<serde_json::Value>,
    /// Variable-font axes: `{"wght": 720, "wdth": 125, "opsz": 144, "SOFT": 100}`
    /// (`anikit-video fonts NAME` lists a face's axes). Animate one with
    /// `"axes.wght"` in `animate`; the text re-lays out per frame.
    pub axes: Option<BTreeMap<String, f64>>,
    /// OpenType features: `"ss01 tnum smcp -liga"` (or CSS `"\"ss01\" on"`).
    pub features: Option<String>,
    /// Typesetting pass (default on): curly quotes, `...` → …, ` -- ` → –,
    /// `---` → —, and no one-word last line. `false` keeps the text verbatim.
    pub typeset: Option<bool>,

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
    /// `superellipse` exponent: 2 = ellipse, 4–5 = squircle, higher = squarer, < 2 = pinched star. Default 4.
    pub exponent: Option<f64>,
    /// `blob`: how irregular the outline is (0..1). Default 0.3.
    pub wobble: Option<f64>,
    /// Seed for `blob` and random `pattern` kinds.
    pub seed: Option<u32>,
    /// `arc`: start and end angle in degrees (0 = 12 o'clock, clockwise). Default 0 and 270.
    pub start_angle: Option<f64>,
    pub end_angle: Option<f64>,
    /// A generated pattern filling `width` × `height`: `"dots"`, `"truchet 48"`, or `{ "kind", "spacing", "mark", "angle", "seed", "jitter", "ramp", "tile", "tileStroke", "turns", "stagger" }` (`tile`: your own SVG motif).
    pub pattern: Option<PatternValue>,
    pub trim_start: Option<Num>,
    pub trim_end: Option<Num>,
    /// Paint the stroke as a brush: `true`, or `{ "taper": [start, end], "pressure": 0..1, "dry": 0..1, "seed" }`.
    /// Width from `strokeWidth`; tapered ends, pressure wobble, dry-brush bristles; `trimEnd` paints it on.
    pub brush: Option<BrushValue>,

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

    /// Data points drawn as dots in frame pixels (thousands are fine: one path per colour):
    /// `{ "data": [[x, y], …] | "points.csv", "size": 4, "color": "$accent", "appear": "=t * 800 - i" }`.
    pub dots: Option<DotsSpec>,

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

/// `repeat` on a layer: a spec-level copy count, or a render-time repeater.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RepeatValue {
    Count(u32),
    Repeater(RepeaterSpec),
}

/// Render-time repeater (After Effects style): `count` copies, each offset from the last.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepeaterSpec {
    /// Number of copies (including the original). Default 3.
    pub count: Option<u32>,
    /// Per-copy change: `x`, `y` (px), `rotate` (deg), `scale` (multiplier, number or `[sx, sy]`),
    /// `opacity` (added per copy, e.g. −0.1), `hue` (degrees per copy).
    pub offset: Option<RepeatOffset>,
    /// Seconds of delay per copy (copy i plays i × stagger later).
    pub stagger: Option<f64>,
    /// Stagger order: `forward` (default), `reverse`, `center` (middle first), `random` (with `seed`).
    pub order: Option<String>,
    pub seed: Option<f64>,
    /// Rotation/scale centre `[x, y]` in parent pixels (default: the layer's own position).
    pub center: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepeatOffset {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotate: Option<f64>,
    pub scale: Option<serde_json::Value>,
    pub opacity: Option<f64>,
}

/// `echo`: earlier-time copies drawn under the layer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EchoSpec {
    /// Number of echoes. Default 5.
    pub count: Option<u32>,
    /// Seconds between echoes. Default 0.05.
    pub delay: Option<f64>,
    /// Opacity multiplier per echo. Default 0.6.
    pub decay: Option<f64>,
    /// Blend mode of the echoes (`normal`, `add`, `screen`, …). Default normal.
    pub blend: Option<String>,
}

/// Text animator: per-unit property offsets, weighted by a range selector.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatorSpec {
    /// `char` (default), `word`, `line`.
    pub by: Option<String>,
    /// Offsets at full selection: `x`, `y` (px), `rotation` (deg), `scale` (multiplier), `opacity` (0..1), `blur` (px), `color`.
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotation: Option<f64>,
    pub scale: Option<f64>,
    pub opacity: Option<f64>,
    pub blur: Option<f64>,
    pub color: Option<String>,
    /// Variable-font axis values at full selection, e.g. `{"wght": 900, "wdth": 125}`
    /// (a weight wave across the units). Spacing stays that of the base text.
    pub axes: Option<BTreeMap<String, f64>>,
    /// Range start / end as fractions of the units (0..1). Defaults 0 and 1. Animatable as `rangeStart` / `rangeEnd`.
    pub start: Option<Num>,
    pub end: Option<Num>,
    /// Shifts the range (−1..1). Animatable as `rangeOffset`. When not given, it sweeps 0 → 1 over `duration` after `delay`
    /// (a left-to-right reveal of units that start at the animator's props).
    pub offset: Option<Num>,
    /// Edge softness as a fraction of all units (0 = hard). Default 0.25.
    pub smooth: Option<f64>,
    /// `forward` (default), `reverse`, `center` (from the middle out), `random`.
    pub order: Option<String>,
    pub seed: Option<f64>,
    /// Auto sweep: seconds, delay and ease. Defaults 1, 0, `outCubic`.
    pub duration: Option<f64>,
    pub delay: Option<f64>,
    pub ease: Option<String>,
}

/// `zigzag` shape modifier.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZigzagSpec {
    /// Amplitude in px. Default 10.
    pub size: Option<f64>,
    /// Ridges per segment (per closed outline for ellipses). Default 10.
    pub ridges: Option<u32>,
    /// Smooth waves instead of sharp corners.
    pub smooth: Option<bool>,
}

/// `wigglePath` shape modifier.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WigglePathSpec {
    /// Displacement in px. Default 10.
    pub amount: Option<f64>,
    /// Points the outline is resampled to. Default 32.
    pub detail: Option<u32>,
    /// Noise cycles per second (0 = frozen). Default 1.
    pub speed: Option<f64>,
    pub seed: Option<f64>,
}

/// `matte`: show this layer through another layer's alpha or luminance.
#[derive(Clone, Debug, PartialEq)]
pub struct MatteSpec(pub MatteObj);
string_or_struct!(MatteSpec, MatteObj, |s| MatteObj { layer: s.to_string(), mode: None });

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatteObj {
    /// `id` of the matte layer (in the same scene; it is not drawn itself).
    pub layer: String,
    /// `alpha` (default), `alphaInverted`, `luma`, `lumaInverted`.
    pub mode: Option<String>,
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
string_or_struct!(Shadow, ShadowObj, |s| css_shadow(s));

/// `"#0008"` (a colour) or CSS shorthand `"0 4px 24px #0008"` (x, y, blur, colour).
fn css_shadow(s: &str) -> ShadowObj {
    let mut nums = Vec::new();
    let mut rest = s.trim();
    while let Some(tok) = rest.split_whitespace().next() {
        match tok.trim_end_matches("px").parse::<f64>() {
            Ok(v) if nums.len() < 3 => {
                nums.push(v);
                rest = rest[tok.len()..].trim_start();
            }
            _ => break,
        }
    }
    ShadowObj {
        x: nums.first().copied(),
        y: nums.get(1).copied(),
        blur: nums.get(2).copied(),
        color: (!rest.is_empty()).then(|| rest.to_string()),
    }
}

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
    /// Colours along the stroke, start to end: a gradient that follows the path (and its `trimEnd` write-on).
    pub along: Option<Vec<String>>,
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
    #[serde(alias = "at")]
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
    /// Built-in effect: `click`, `tick`, `pop`, `whoosh`, `swoosh`, `rise`, `riser`, `drop`, `impact`, `ding`, `chime`, `typing`, `success`;
    /// any other `docs sound` name (`glass`, `passby`, `impact+glass`, `kenney-interface/click`) lands its hit on `at`.
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
