//! Illustration toolkit (`illo`): a small scene graph of layered vector shapes
//! lit and printed like editorial illustration rather than clip-art.
//!
//! * Shapes: `path` (SVG `d`, or smooth `points`), `circle`, `ellipse`, `rect`,
//!   `blob`, `polygon`, `star`; optional `groups` with (animated) transforms.
//! * Colour: `palette` swatches (`"$1"`, `"$name"`), optional `inks` that every
//!   colour snaps to (and `overprint` to multiply overlaps), fills that are
//!   solid or OKLab gradients dithered with grain (never smooth airbrush).
//! * Light: one global `light` lights every shape. `shade` clips the silhouette,
//!   offset towards the light, into a hue-shifted form shadow and a crisp rim;
//!   `shadow` casts a hard silhouette (optionally flattened onto the ground).
//! * Print: `grain`, `texture` (`paper`, `halftone`, `riso`, `hatch`) and
//!   `misregister`, locked to each shape.
//! * Hand: one global `hand` (line weights, wobble, edge roughness, boil) and
//!   `corners` shape language. Outlines are heavier on the shadow side and thin
//!   (or broken) where the light hits; their colour is the fill darkened.
//! * Composition: `clipTo` (draw inside another shape) and `cut` (subtract).
//! * Motion: keyframes (`x y rotate scale sx sy squash opacity d`), arc-length
//!   path morphs, `sway`, `lag` (secondary motion), `onTwos`/`stepFps`, boil.
//!
//! Ops are cached per shape by everything that changes them, so static shapes
//! cost a clone per frame, and print textures are images keyed by content.

use super::bgfx::{from_lin3, hash2, lin3, oklab, oklab_inv, smoothstep};
use super::illus_fill::{hatch_lines, paper_field, rings};
use super::illustration::{resample, ribbon, tip, vnoise};
use super::style::{clamp01, closest, fnv, hash01, parse_color};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{self, Blend, FillRule, Filter, Op, Paint, Path, Rgba, IDENTITY};
use kurbo::{PathEl, Point, Shape as _};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::Mutex;

// ------------------------------------------------------------------ spec

/// A reference to another shape: its index in `shapes` or its `name`.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Ref {
    Index(usize),
    Name(String),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Refs {
    One(Ref),
    Many(Vec<Ref>),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Num2 {
    One(f32),
    Two([f32; 2]),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum OnOff<T> {
    On(bool),
    Amount(f32),
    Full(T),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct LightSpec {
    /// Direction the key light comes from, degrees (0 = from the right, 90 = from below; default 225 = top-left).
    pub angle: f32,
    /// Light colour (rims and highlights lean towards it).
    pub color: String,
    /// Ambient / shadow colour (form and cast shadows rotate their hue towards it).
    pub shadow: String,
    /// Direction of the rim light, degrees (default: the key light).
    pub rim: Option<f32>,
    /// Scales every shade and rim (default 1).
    pub strength: f32,
}
impl Default for LightSpec {
    fn default() -> Self {
        LightSpec { angle: 225.0, color: "#fff1d6".into(), shadow: "#33306b".into(), rim: None, strength: 1.0 }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct HandSpec {
    /// Outline weight on the lit side and on the shadow side, view px (default [0.8, 4]).
    pub weight: [f32; 2],
    /// Low-frequency hand wobble of every edge, view px (default 1.5).
    pub wobble: f32,
    /// High-frequency edge breakup ("ink on paper"), view px (default 0.6).
    pub rough: f32,
    /// Redraws per second (wobble, roughness and line jitter re-seed; 0 = off, 6–12 typical).
    pub boil: f32,
    /// Extra line jitter per boil redraw, view px (default 0.8).
    pub boil_amount: f32,
    /// One ink colour for every outline (default: each fill darkened).
    pub color: Option<String>,
    /// How much darker than its fill a line is, 0..1 (default 0.5).
    pub darken: f32,
    /// Outline every closed shape by default.
    pub outline: bool,
    /// Break the line where the light hits hardest, 0..1 (default 0.15).
    pub gaps: f32,
}
impl Default for HandSpec {
    fn default() -> Self {
        HandSpec { weight: [0.8, 4.0], wobble: 1.5, rough: 0.6, boil: 0.0, boil_amount: 0.8, color: None, darken: 0.5, outline: false, gaps: 0.15 }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum CornerSpec {
    Name(String),
    Px(f32),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Stop {
    At((f32, String)),
    Color(String),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GradSpec {
    /// `"linear"` (default) or `"radial"`.
    #[serde(default)]
    pub gradient: Option<String>,
    pub stops: Vec<Stop>,
    /// Linear direction in degrees (default: from the lit side to the shadow side).
    #[serde(default)]
    pub angle: Option<f32>,
    /// Radial centre as a fraction of the shape's box (default: towards the light).
    #[serde(default)]
    pub center: Option<[f32; 2]>,
    /// Radial radius as a fraction of the box's larger side (default 0.75).
    #[serde(default)]
    pub radius: Option<f32>,
    /// Grain dither across the transition, 0..1 (default 0.12).
    #[serde(default)]
    pub dither: Option<f32>,
    /// Hard value steps (posterised, with dithered edges); 0 = continuous.
    #[serde(default)]
    pub steps: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum FillSpec {
    Color(String),
    Grad(GradSpec),
    Image(ImageFill),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct ShadeSpec {
    /// Form-shadow strength 0..1 (default 0.6).
    pub amount: f32,
    /// How far the lit copy is pushed towards the light, fraction of the shape's size (default 0.2).
    pub offset: f32,
    /// Terminator softness, fraction of size (default 0 = hard-edged, the print look).
    pub soft: f32,
    /// Rim width, fraction of size (default 0.035; 0 = none).
    pub rim: f32,
    /// Rim opacity (default 0.9).
    pub rim_amount: f32,
    /// Explicit shadow / rim colours.
    pub color: Option<String>,
    pub rim_color: Option<String>,
}
impl Default for ShadeSpec {
    fn default() -> Self {
        ShadeSpec { amount: 0.6, offset: 0.2, soft: 0.0, rim: 0.035, rim_amount: 0.9, color: None, rim_color: None }
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ShadowSpec {
    /// Distance away from the light, view px (default 0.12 of the shape's size).
    pub dist: Option<f32>,
    /// Explicit offset (adds to `dist`).
    pub dx: f32,
    pub dy: f32,
    /// Lay the shadow on the ground: heights scale by this about the shape's base, leaning away from the light
    /// (0.2–0.5 falls back, up the screen; negative falls forward).
    pub flatten: Option<f32>,
    pub color: Option<String>,
    /// Multiply strength (default 0.7).
    pub opacity: Option<f32>,
    /// Softness px (default 0: hard-edged shapes, not airbrush).
    pub blur: f32,
    /// Only fall on this shape.
    pub clip_to: Option<Ref>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct GrainSpec {
    /// Luminance amplitude (default 0.06).
    pub amount: Option<f32>,
    /// Grain size, output px (default 1.4).
    pub scale: Option<f32>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum GrainOpt {
    Amount(f32),
    Full(GrainSpec),
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct TextureFull {
    /// `"paper"`, `"halftone"`, `"riso"` or `"hatch"` (`crosshatch`, `stipple` also work).
    pub style: String,
    pub amount: Option<f32>,
    /// Halftone cell / hatch spacing / riso speck size, px.
    pub scale: Option<f32>,
    /// Screen or hatch angle, degrees.
    pub angle: Option<f32>,
    pub color: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum TextureSpec {
    Name(String),
    Full(TextureFull),
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Key {
    pub t: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub rotate: Option<f32>,
    pub scale: Option<f32>,
    pub sx: Option<f32>,
    pub sy: Option<f32>,
    /// Area-preserving squash: `sy = squash`, `sx = 1 / squash`.
    pub squash: Option<f32>,
    pub opacity: Option<f32>,
    /// Morph target (any closed or open SVG path; resampled by arc length).
    pub d: Option<String>,
    /// Ease into this key (default: the animation's `ease`).
    pub ease: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum LoopSpec {
    On(bool),
    Period(f32),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct AnimFull {
    pub keys: Vec<Key>,
    #[serde(default, rename = "loop")]
    pub looped: Option<LoopSpec>,
    #[serde(default)]
    pub ease: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum AnimSpec {
    Keys(Vec<Key>),
    Full(AnimFull),
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Sway {
    pub rotate: f32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// Seconds per cycle (default 3).
    pub period: Option<f32>,
    /// Cycle offset 0..1.
    pub phase: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct ShapeSpec {
    pub shape: String,
    pub name: Option<String>,
    pub group: Option<String>,
    pub d: Option<String>,
    pub closed: Option<bool>,
    pub points: Option<Vec<[f32; 2]>>,
    pub smooth: bool,
    pub cx: Option<f32>,
    pub cy: Option<f32>,
    pub r: Option<f32>,
    pub rx: Option<f32>,
    pub ry: Option<f32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub radius: Option<f32>,
    pub n: Option<u32>,
    pub inner: Option<f32>,
    pub lumpy: Option<f32>,
    pub corners: Option<CornerSpec>,
    pub rotate: f32,
    pub scale: Option<Num2>,
    pub origin: Option<[f32; 2]>,
    pub fill: Option<FillSpec>,
    pub opacity: f32,
    pub blend: Option<String>,
    pub stroke: Option<String>,
    pub line_color: Option<String>,
    pub outline: Option<OnOff<f32>>,
    pub weight: Option<[f32; 2]>,
    pub taper: Option<f32>,
    pub wobble: Option<f32>,
    pub rough: Option<f32>,
    pub shade: Option<OnOff<ShadeSpec>>,
    pub shadow: Option<OnOff<ShadowSpec>>,
    pub grain: Option<GrainOpt>,
    pub texture: Option<TextureSpec>,
    pub misregister: Option<Num2>,
    pub clip_to: Option<Ref>,
    pub cut: Option<Refs>,
    pub animate: Option<AnimSpec>,
    pub sway: Option<Sway>,
    pub lag: f32,
    pub visible: bool,
    pub seed: Option<u32>,
    /// Per-shape light (any of `angle`, `rim`, `color`, `shadow`, `strength`; the rest come from the global `light`).
    pub light: Option<LightOver>,
    /// Per-shape inks to snap to (`[]` turns snapping off for this shape).
    pub inks: Option<Vec<String>>,
    /// Per-shape line gaps on the lit side (default: the hand's).
    pub gaps: Option<f32>,
    /// Per-shape boil rate in redraws per second (default: the hand's; 0 = still).
    pub boil: Option<f32>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct LightOver {
    pub angle: Option<f32>,
    pub rim: Option<f32>,
    pub color: Option<String>,
    pub shadow: Option<String>,
    pub strength: Option<f32>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ImageFill {
    /// An image (or video) file, like any layer `src`.
    pub image: String,
    /// `"cover"` (default), `"contain"`, `"stretch"` or `"tile"`.
    #[serde(default)]
    pub fit: Option<String>,
    /// Extra zoom (default 1).
    #[serde(default)]
    pub scale: Option<f32>,
    /// Pan inside the shape, fraction of the box (default [0.5, 0.5]).
    #[serde(default)]
    pub focus: Option<[f32; 2]>,
    /// The colour that shading, rims and line work derive from (default mid grey).
    #[serde(default)]
    pub base: Option<String>,
}
impl Default for ShapeSpec {
    fn default() -> Self {
        ShapeSpec {
            shape: "path".into(),
            name: None,
            group: None,
            d: None,
            closed: None,
            points: None,
            smooth: false,
            cx: None,
            cy: None,
            r: None,
            rx: None,
            ry: None,
            x: None,
            y: None,
            w: None,
            h: None,
            radius: None,
            n: None,
            inner: None,
            lumpy: None,
            corners: None,
            rotate: 0.0,
            scale: None,
            origin: None,
            fill: None,
            opacity: 1.0,
            blend: None,
            stroke: None,
            line_color: None,
            outline: None,
            weight: None,
            taper: None,
            wobble: None,
            rough: None,
            shade: None,
            shadow: None,
            grain: None,
            texture: None,
            misregister: None,
            clip_to: None,
            cut: None,
            animate: None,
            sway: None,
            lag: 0.0,
            visible: true,
            seed: None,
            light: None,
            inks: None,
            gaps: None,
            boil: None,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct GroupSpec {
    pub name: String,
    pub parent: Option<String>,
    pub x: f32,
    pub y: f32,
    pub rotate: f32,
    pub scale: Option<Num2>,
    pub origin: [f32; 2],
    /// Bone pivot (alias of `origin`): rotation and scale happen about it.
    pub pivot: Option<[f32; 2]>,
    pub animate: Option<AnimSpec>,
    pub sway: Option<Sway>,
    /// Procedural motion cycle: `"idle"`, `"breathe"`, `"walk"`, `"bob"`, `"wave"` or `"nod"` (or an object).
    pub cycle: Option<CycleSpec>,
    /// Seconds this group trails its parent's motion (secondary action).
    pub lag: f32,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct CycleFull {
    pub style: String,
    /// Cycles per second (defaults per style).
    pub speed: Option<f32>,
    /// Amplitude multiplier (default 1).
    pub amount: Option<f32>,
    /// Offset 0..1 (0.5 puts a leg or arm in opposition).
    pub phase: f32,
    /// Per-channel amplitudes replacing the style's (degrees, view px, scale fractions).
    pub rotate: Option<f32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub sx: Option<f32>,
    pub sy: Option<f32>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum CycleSpec {
    Style(String),
    Full(CycleFull),
}

const CYCLES: &[&str] = &["idle", "breathe", "walk", "bob", "wave", "nod"];

#[derive(Clone, Debug)]
struct PCycle {
    style: String,
    speed: f32,
    amount: f32,
    phase: f32,
    amp: [Option<f32>; 5],
}

fn cycle_of(c: &Option<CycleSpec>) -> Result<Option<PCycle>, Error> {
    let Some(c) = c else { return Ok(None) };
    let f = match c {
        CycleSpec::Style(s) => CycleFull { style: s.clone(), ..Default::default() },
        CycleSpec::Full(f) => f.clone(),
    };
    one_of("cycle", &f.style, CYCLES)?;
    let speed = f.speed.unwrap_or(match f.style.as_str() {
        "idle" => 0.3,
        "breathe" => 0.25,
        "walk" | "bob" => 0.9,
        "wave" => 1.6,
        _ => 0.5,
    });
    Ok(Some(PCycle { style: f.style, speed, amount: f.amount.unwrap_or(1.0), phase: f.phase, amp: [f.rotate, f.x, f.y, f.sx, f.sy] }))
}

fn cycle_at(c: &PCycle, t: f32) -> Pose {
    let ph = (t * c.speed + c.phase) * TAU;
    let (s, s1) = (ph.sin(), (ph + 1.1).sin());
    // Style defaults per channel: (rotate°, x px, y px, sx, sy) amplitudes and waveforms.
    let (d, wave): ([f32; 5], [f32; 5]) = match c.style.as_str() {
        "idle" => ([1.5, 0.0, 2.0, 0.0, 0.006], [s, s, s1, s1, s1]),
        "breathe" => ([0.0, 0.0, 0.0, -0.006, 0.02], [s, s, s, s, s]),
        "walk" => ([24.0, 0.0, 0.0, 0.0, 0.0], [s, s, s.abs(), s, s]),
        // One bob per step: twice per walk cycle.
        "bob" => ([0.0, 0.0, -7.0, 0.0, 0.0], [s, s, s.abs().powf(0.8), s, s]),
        "wave" => ([18.0, 0.0, 0.0, 0.0, 0.0], [s + 0.22 * (2.0 * ph).sin(), s, s, s, s]),
        _ => ([4.0, 0.0, 0.0, 0.0, 0.0], [s * s * s, s, s, s, s]),
    };
    let ch = |i: usize| c.amp[i].unwrap_or(d[i]) * c.amount * wave[i];
    Pose { rot: ch(0), dx: ch(1), dy: ch(2), sx: 1.0 + ch(3), sy: 1.0 + ch(4), op: 1.0 }
}
impl Default for GroupSpec {
    fn default() -> Self {
        GroupSpec { name: String::new(), parent: None, x: 0.0, y: 0.0, rotate: 0.0, scale: None, origin: [0.0, 0.0], pivot: None, animate: None, sway: None, cycle: None, lag: 0.0 }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Illo {
    pub shapes: Vec<ShapeSpec>,
    pub groups: Vec<GroupSpec>,
    /// Swatches: an array (`"$1"` is the first) or an object (`"$skin"`).
    pub palette: Option<Value>,
    /// Spot inks: every colour snaps to the nearest one.
    pub inks: Vec<String>,
    /// Every shape multiplies over what is below it (riso / screen-print overprint).
    pub overprint: bool,
    pub light: LightSpec,
    pub hand: HandSpec,
    /// Shape language: `"sharp"`, `"soft"`, `"round"` or a radius in px (rects, polygons, stars).
    pub corners: Option<CornerSpec>,
    /// Design-space size `[w, h]` fitted (contain, centred) into the box; default: box pixels.
    pub view: Option<[f32; 2]>,
    /// Defaults for every closed filled shape (each shape can override).
    pub shade: Option<OnOff<ShadeSpec>>,
    pub grain: Option<GrainOpt>,
    pub texture: Option<TextureSpec>,
    /// Fill plates offset from the line work, view px.
    pub misregister: Option<Num2>,
    /// Step time to 12 fps (or `stepFps`).
    pub on_twos: bool,
    pub step_fps: f32,
    /// Vector art to import: an SVG string whose `<path>`, `<circle>`, `<ellipse>`, `<rect>`,
    /// `<polygon>`, `<polyline>` and `<line>` elements (with fill / stroke attributes) become
    /// shapes drawn before `shapes`, named by their `id` (or `svg0`, `svg1`…). Transforms are ignored.
    pub svg: Option<String>,
    /// Draw every closed shape as brush outline plus hatching in its shadow colour, without flat fills.
    pub paint_over: bool,
    pub seed: u32,
    #[serde(skip)]
    p: Prepared,
}
impl Default for Illo {
    fn default() -> Self {
        Illo {
            shapes: Vec::new(),
            groups: Vec::new(),
            palette: None,
            inks: Vec::new(),
            overprint: false,
            light: LightSpec::default(),
            hand: HandSpec::default(),
            corners: None,
            view: None,
            shade: None,
            grain: None,
            texture: None,
            misregister: None,
            on_twos: false,
            step_fps: 0.0,
            svg: None,
            paint_over: false,
            seed: 1,
            p: Prepared::default(),
        }
    }
}

// ------------------------------------------------------------------ prepared

#[derive(Clone, Debug)]
struct Grad {
    radial: bool,
    stops: Vec<(f32, Rgba)>,
    angle: Option<f32>,
    center: Option<[f32; 2]>,
    radius: f32,
    dither: f32,
    steps: u32,
}

#[derive(Clone, Debug)]
enum PFill {
    Solid(Rgba),
    Grad(Grad),
    Image { src: String, fit: String, scale: f32, focus: [f32; 2], base: Rgba },
}

impl PFill {
    /// The representative colour (solid, or the middle of the gradient).
    fn base(&self) -> Rgba {
        match self {
            PFill::Solid(c) => *c,
            PFill::Grad(g) => grad_at(&g.stops, 0.5),
            PFill::Image { base, .. } => *base,
        }
    }
}

#[derive(Clone, Debug)]
struct PShade {
    amount: f32,
    offset: f32,
    soft: f32,
    rim: f32,
    rim_amount: f32,
    color: Option<Rgba>,
    rim_color: Option<Rgba>,
}

#[derive(Clone, Debug)]
struct PShadow {
    dist: Option<f32>,
    dx: f32,
    dy: f32,
    flatten: Option<f32>,
    color: Rgba,
    opacity: f32,
    blur: f32,
    clip: Option<usize>,
}

#[derive(Clone, Debug)]
struct PTex {
    style: String,
    amount: f32,
    scale: f32,
    angle: f32,
    color: Option<Rgba>,
}

#[derive(Clone, Debug)]
struct PKey {
    t: f32,
    x: Option<f32>,
    y: Option<f32>,
    rot: Option<f32>,
    sx: Option<f32>,
    sy: Option<f32>,
    op: Option<f32>,
    morph: Option<usize>,
    ease: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct PAnim {
    keys: Vec<PKey>,
    period: Option<f32>,
    ease: String,
    /// Morph targets, resampled by arc length: per target, rings of `MORPH_N` points (and open flags).
    morphs: Vec<Vec<(Vec<[f32; 2]>, bool)>>,
}

#[derive(Clone, Debug)]
struct PShape {
    base: Path,
    closed: bool,
    origin: [f32; 2],
    rotate: f32,
    scale: [f32; 2],
    fill: Option<PFill>,
    opacity: f32,
    blend: Blend,
    line: Option<(Rgba, [f32; 2])>,
    taper: f32,
    wobble: f32,
    rough: f32,
    shade: Option<PShade>,
    shadow: Option<PShadow>,
    grain: Option<(f32, f32)>,
    texture: Option<PTex>,
    misregister: [f32; 2],
    clip: Option<usize>,
    cut: Vec<usize>,
    anim: Option<PAnim>,
    sway: Option<Sway>,
    group: Option<usize>,
    lag: f32,
    visible: bool,
    seed: u32,
    /// Outline + hatching only (no flat fill or shade).
    paint_over: bool,
    lt: Lit,
    gaps: f32,
    boil: f32,
}

/// The light one shape sees.
#[derive(Clone, Debug)]
struct Lit {
    dir: [f32; 2],
    rim: [f32; 2],
    color: Rgba,
    tint: Rgba,
    strength: f32,
}

#[derive(Clone, Debug)]
struct PGroup {
    parent: Option<usize>,
    x: f32,
    y: f32,
    rotate: f32,
    scale: [f32; 2],
    origin: [f32; 2],
    anim: Option<PAnim>,
    sway: Option<Sway>,
    cycle: Option<PCycle>,
    lag: f32,
}

#[derive(Default)]
struct Prepared {
    shapes: Vec<PShape>,
    groups: Vec<PGroup>,
    light_color: Rgba,
    shadow_tint: Rgba,
    cache: Mutex<HashMap<u64, (Vec<Op>, Vec<Op>)>>,
    hash: u64,
}

const SHAPES: &[&str] = &["path", "circle", "ellipse", "rect", "blob", "polygon", "star"];
const TEXTURES: &[&str] = &["paper", "halftone", "riso", "hatch", "crosshatch", "stipple", "scribble"];
const BLENDS: &[&str] = &["normal", "multiply", "screen", "overlay", "darken", "lighten", "softLight", "add", "difference"];
const EASES: &[&str] = &["linear", "in", "out", "inOut", "outBack", "inOutBack", "step"];
const MORPH_N: usize = 160;

fn err(s: impl Into<String>) -> Error {
    Error(s.into())
}

fn one_of(what: &str, v: &str, ok: &[&str]) -> Result<(), Error> {
    if ok.contains(&v) {
        return Ok(());
    }
    let hint = closest(v, ok).map(|c| format!(" (did you mean {c:?}?)")).unwrap_or_default();
    Err(err(format!("{what} {v:?}: use one of {ok:?}{hint}")))
}

// ------------------------------------------------------------------ colour

fn lab(c: Rgba) -> [f32; 3] {
    oklab(lin3(c))
}

fn unlab(l: [f32; 3], a: f32) -> Rgba {
    let mut c = from_lin3(oklab_inv(l));
    c.0[3] = a;
    c
}

fn lab_mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let (x, y) = (lab(a), lab(b));
    unlab([0, 1, 2].map(|i| x[i] + (y[i] - x[i]) * t), a.0[3] + (b.0[3] - a.0[3]) * t)
}

/// Rotate `c`'s OKLCh hue towards `toward`'s by at most `max_deg`, scale lightness and chroma.
fn lch_shift(c: Rgba, toward: Rgba, max_deg: f32, l_mul: f32, l_add: f32, c_mul: f32, tint: f32) -> Rgba {
    let [l, a, b] = lab(c);
    let [_, ta, tb] = lab(toward);
    let ch = (a * a + b * b).sqrt();
    let h = b.atan2(a);
    let th = tb.atan2(ta);
    let mut d = th - h;
    while d > PI {
        d -= TAU;
    }
    while d < -PI {
        d += TAU;
    }
    let lim = max_deg.to_radians();
    let h2 = h + d.clamp(-lim, lim);
    let c2 = ch * c_mul;
    // Near-greys take the tint's hue directly (they have none of their own).
    let (mut a2, mut b2) = (c2 * h2.cos(), c2 * h2.sin());
    a2 += (ta - a2) * tint;
    b2 += (tb - b2) * tint;
    unlab([(l * l_mul + l_add).clamp(0.0, 1.0), a2, b2], c.0[3])
}

/// Form-shadow colour: hue rotated 10–30° towards the ambient, 15–30% darker, a little richer.
fn shadow_tone(c: Rgba, tint: Rgba, k: f32) -> Rgba {
    lch_shift(c, tint, 28.0 * k, 1.0 - 0.3 * k, 0.0, 1.0 + 0.12 * k, 0.12 * k)
}

/// Rim colour: lighter, leaning towards the light colour.
fn rim_tone(c: Rgba, light: Rgba, k: f32) -> Rgba {
    let [l, ..] = lab(c);
    let lifted = lch_shift(c, light, 20.0 * k, 1.0, (0.97 - l).max(0.0) * 0.55 * k, 0.95, 0.0);
    lab_mix(lifted, light, 0.25 * k)
}

/// Line colour: the fill darkened and enriched, never black.
fn line_tone(c: Rgba, tint: Rgba, darken: f32) -> Rgba {
    let mut o = lch_shift(c, tint, 14.0 * darken, 1.0 - 0.55 * darken, -0.02 * darken, 1.0 + 0.2 * darken, 0.05);
    o.0[3] = 1.0;
    o
}

fn grad_at(stops: &[(f32, Rgba)], u: f32) -> Rgba {
    if stops.is_empty() {
        return Rgba([0.0, 0.0, 0.0, 1.0]);
    }
    if u <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if u <= w[1].0 {
            let f = (u - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
            return lab_mix(w[0].1, w[1].1, f);
        }
    }
    stops[stops.len() - 1].1
}

// ------------------------------------------------------------------ geometry

fn kaff(m: &[f32; 6]) -> kurbo::Affine {
    kurbo::Affine::new(m.map(|v| v as f64))
}

fn mul(a: &[f32; 6], b: &[f32; 6]) -> [f32; 6] {
    dl::mul(a, b)
}

fn inv(m: &[f32; 6]) -> [f32; 6] {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-12 {
        return IDENTITY;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    [a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])]
}

/// translate(t) · rotate(rot) · scale(s) about `o`.
fn trs(tx: f32, ty: f32, rot_deg: f32, s: [f32; 2], o: [f32; 2]) -> [f32; 6] {
    let (sn, cs) = rot_deg.to_radians().sin_cos();
    let (a, b, c, d) = (cs * s[0], sn * s[0], -sn * s[1], cs * s[1]);
    [a, b, c, d, o[0] + tx - (a * o[0] + c * o[1]), o[1] + ty - (b * o[0] + d * o[1])]
}

fn bbox(p: &Path) -> kurbo::Rect {
    p.bounding_box()
}

fn moved(p: &Path, dx: f32, dy: f32) -> Path {
    if dx == 0.0 && dy == 0.0 {
        return p.clone();
    }
    let mut q = p.clone();
    q.apply_affine(kurbo::Affine::translate((dx as f64, dy as f64)));
    q
}

fn pt(p: [f32; 2]) -> Point {
    Point::new(p[0] as f64, p[1] as f64)
}

/// Closed Catmull-Rom through `pts`.
fn smooth_closed(pts: &[[f32; 2]]) -> Path {
    let n = pts.len();
    let mut p = Path::new();
    if n < 3 {
        return p;
    }
    let q: Vec<Point> = pts.iter().map(|&x| pt(x)).collect();
    p.move_to(q[0]);
    for i in 0..n {
        let (p0, p1, p2, p3) = (q[(i + n - 1) % n], q[i], q[(i + 1) % n], q[(i + 2) % n]);
        p.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
    }
    p.close_path();
    p
}

/// Open Catmull-Rom through `pts`.
fn smooth_open(pts: &[[f32; 2]]) -> Path {
    let n = pts.len();
    let mut p = Path::new();
    if n < 2 {
        return p;
    }
    let q: Vec<Point> = pts.iter().map(|&x| pt(x)).collect();
    p.move_to(q[0]);
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (q[i.saturating_sub(1)], q[i], q[i + 1], q[(i + 2).min(n - 1)]);
        p.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
    }
    p
}

/// Corner radius rule: a fraction of the shorter adjacent edge, or pixels.
#[derive(Clone, Copy, Debug)]
enum Corner {
    Frac(f32),
    /// A fraction of the shorter edge, capped at a radius in px.
    Soft(f32, f32),
    Px(f32),
}

fn corner_of(c: &Option<CornerSpec>) -> Result<Option<Corner>, Error> {
    Ok(match c {
        None => None,
        Some(CornerSpec::Px(p)) => Some(Corner::Px(p.max(0.0))),
        Some(CornerSpec::Name(n)) => Some(match n.as_str() {
            "sharp" => Corner::Px(0.0),
            "soft" => Corner::Soft(0.14, 18.0),
            "round" => Corner::Frac(0.38),
            _ => return Err(err(format!("corners {n:?}: use \"sharp\", \"soft\", \"round\" or a radius in px"))),
        }),
    })
}

/// A closed polygon with filleted corners.
fn filleted(pts: &[[f32; 2]], c: Corner) -> Path {
    let n = pts.len();
    let mut p = Path::new();
    if n < 3 {
        return p;
    }
    let len = |a: [f32; 2], b: [f32; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt().max(1e-6);
    for i in 0..n {
        let (a, v, b) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let (la, lb) = (len(v, a), len(v, b));
        let m = la.min(lb);
        let r = match c {
            Corner::Frac(f) => f * m,
            Corner::Soft(f, cap) => (f * m).min(cap),
            Corner::Px(px) => px,
        }
        .min(0.5 * m);
        let p0 = [v[0] + (a[0] - v[0]) / la * r, v[1] + (a[1] - v[1]) / la * r];
        let p1 = [v[0] + (b[0] - v[0]) / lb * r, v[1] + (b[1] - v[1]) / lb * r];
        if i == 0 {
            p.move_to(pt(p0));
        } else {
            p.line_to(pt(p0));
        }
        if r > 0.01 {
            p.quad_to(pt(v), pt(p1));
        }
    }
    p.close_path();
    p
}

/// Flatten a path into polylines (each with a closed flag).
fn polylines(path: &Path, tol: f64) -> Vec<(Vec<[f32; 2]>, bool)> {
    let mut out: Vec<(Vec<[f32; 2]>, bool)> = Vec::new();
    kurbo::flatten(path.iter(), tol, |el| match el {
        PathEl::MoveTo(p) => out.push((vec![[p.x as f32, p.y as f32]], false)),
        PathEl::LineTo(p) => {
            if let Some(r) = out.last_mut() {
                r.0.push([p.x as f32, p.y as f32]);
            }
        }
        PathEl::ClosePath => {
            if let Some(r) = out.last_mut() {
                r.1 = true;
            }
        }
        _ => {}
    });
    out.retain(|r| r.0.len() >= 2);
    out
}

/// Resample a polyline to `n` points evenly by arc length.
fn resample_n(poly: &[[f32; 2]], closed: bool, n: usize) -> Vec<[f32; 2]> {
    let mut pts = poly.to_vec();
    if closed {
        pts.push(poly[0]);
    }
    let mut cum = vec![0.0f32];
    for w in pts.windows(2) {
        let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        cum.push(cum.last().unwrap() + l);
    }
    let total = cum.last().copied().unwrap_or(0.0).max(1e-6);
    let m = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for i in 0..n {
        let s = total * i as f32 / m.max(1) as f32;
        while j + 2 < cum.len() && cum[j + 1] < s {
            j += 1;
        }
        let seg = (cum[j + 1] - cum[j]).max(1e-6);
        let u = ((s - cum[j]) / seg).clamp(0.0, 1.0);
        let (a, b) = (pts[j], pts[(j + 1).min(pts.len() - 1)]);
        out.push([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]);
    }
    out
}

fn signed_area(r: &[[f32; 2]]) -> f32 {
    let n = r.len();
    (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).sum::<f32>() * 0.5
}

/// Rotate (and if needed reverse) closed ring `b` so it lines up with `a` point for point.
fn align_ring(a: &[[f32; 2]], b: &mut Vec<[f32; 2]>) {
    if (signed_area(a) > 0.0) != (signed_area(b) > 0.0) {
        b.reverse();
    }
    let n = a.len();
    let mut best = (f32::MAX, 0);
    for off in (0..n).step_by(2) {
        let mut s = 0.0;
        for i in (0..n).step_by(4) {
            let q = b[(i + off) % n];
            s += (q[0] - a[i][0]).powi(2) + (q[1] - a[i][1]).powi(2);
        }
        if s < best.0 {
            best = (s, off);
        }
    }
    b.rotate_left(best.1);
}

fn rings_path(rs: &[(Vec<[f32; 2]>, bool)]) -> Path {
    let mut p = Path::new();
    for (r, closed) in rs {
        for (i, q) in r.iter().enumerate() {
            if i == 0 {
                p.move_to(pt(*q));
            } else {
                p.line_to(pt(*q));
            }
        }
        if *closed {
            p.close_path();
        }
    }
    p
}

/// Hand wobble and edge breakup: displace closed outlines along their normals.
fn roughen(path: &Path, wobble: f32, rough: f32, seed: u32) -> Path {
    if wobble <= 0.0 && rough <= 0.0 {
        return path.clone();
    }
    let step = if rough > 0.0 { 2.0 } else { 4.0 };
    let mut out = Path::new();
    for (k, ring) in rings(path, step).into_iter().enumerate() {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        let pts: Vec<[f32; 2]> = ring.iter().map(|&(x, y)| [x, y]).collect();
        let sign = if signed_area(&pts) > 0.0 { 1.0 } else { -1.0 };
        let s = seed ^ (k as u32).wrapping_mul(0x9e37_79b9);
        let mut arc = 0.0;
        for i in 0..n {
            let (a, b) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let l = (tx * tx + ty * ty).sqrt().max(1e-6);
            let (nx, ny) = (ty / l * sign, -tx / l * sign);
            let o = wobble * (0.8 * vnoise(arc / 70.0, s) + 0.35 * vnoise(arc / 24.0, s ^ 0x51)) + rough * (0.7 * vnoise(arc / 6.0, s ^ 0xa7) + 0.3 * vnoise(arc / 2.6, s ^ 0x3c));
            let q = Point::new((pts[i][0] + nx * o) as f64, (pts[i][1] + ny * o) as f64);
            if i == 0 {
                out.move_to(q);
            } else {
                out.line_to(q);
            }
            arc += l * 0.5;
        }
        out.close_path();
    }
    out
}

// ------------------------------------------------------------------ prepare

fn ease_fn(name: &str, x: f32) -> f32 {
    let x = clamp01(x);
    match name {
        "linear" => x,
        "in" => x * x * x,
        "out" => 1.0 - (1.0 - x).powi(3),
        "outBack" => {
            let c = 1.70158;
            1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2)
        }
        "inOutBack" => {
            let c = 1.70158 * 1.525;
            if x < 0.5 {
                ((2.0 * x).powi(2) * ((c + 1.0) * 2.0 * x - c)) / 2.0
            } else {
                ((2.0 * x - 2.0).powi(2) * ((c + 1.0) * (x * 2.0 - 2.0) + c) + 2.0) / 2.0
            }
        }
        "step" => {
            if x >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
        _ => {
            if x < 0.5 {
                4.0 * x * x * x
            } else {
                1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
            }
        }
    }
}

struct Resolver {
    palette: HashMap<String, Rgba>,
    inks: Vec<(Rgba, [f32; 3])>,
}

impl Resolver {
    fn color(&self, s: &str) -> Result<Rgba, Error> {
        Ok(self.snap(self.color_raw(s)?))
    }

    fn color_raw(&self, s: &str) -> Result<Rgba, Error> {
        let c = if let Some(k) = s.strip_prefix('$') {
            // Palette names arrive snake_cased (the spec's keys are), so `$wallHi` finds `wall_hi`.
            let snake = super::kebab(k).replace('-', "_");
            *self.palette.get(k).or_else(|| self.palette.get(&snake)).ok_or_else(|| {
                let names: Vec<&str> = self.palette.keys().map(|s| s.as_str()).collect();
                let hint = closest(k, &names).map(|c| format!(" (did you mean \"${c}\"?)")).unwrap_or_default();
                err(format!("colour {s:?} is not in the palette{hint}"))
            })?
        } else {
            parse_color(s).ok_or_else(|| err(format!("not a colour: {s:?} (use \"#rrggbb\", \"$1\" for a palette swatch, or a CSS colour)")))?
        };
        Ok(c)
    }

    fn snap(&self, c: Rgba) -> Rgba {
        if self.inks.is_empty() {
            return c;
        }
        let l = lab(c);
        let mut best = (f32::MAX, c);
        for (ink, il) in &self.inks {
            let d = (0..3).map(|i| (l[i] - il[i]).powi(2) * if i == 0 { 1.0 } else { 2.0 }).sum::<f32>();
            if d < best.0 {
                best = (d, *ink);
            }
        }
        Rgba([best.1 .0[0], best.1 .0[1], best.1 .0[2], c.0[3]])
    }

    fn fill(&self, f: &FillSpec) -> Result<Option<PFill>, Error> {
        Ok(match f {
            FillSpec::Color(s) if s == "none" || s == "transparent" => None,
            FillSpec::Color(s) => Some(PFill::Solid(self.color(s)?)),
            FillSpec::Grad(g) => {
                let kind = g.gradient.as_deref().unwrap_or("linear");
                one_of("gradient", kind, &["linear", "radial"])?;
                if g.stops.len() < 2 {
                    return Err(err("a gradient needs at least 2 stops"));
                }
                let n = g.stops.len();
                let mut stops = Vec::with_capacity(n);
                for (i, s) in g.stops.iter().enumerate() {
                    let (o, c) = match s {
                        Stop::At((o, c)) => (*o, c),
                        Stop::Color(c) => (i as f32 / (n - 1) as f32, c),
                    };
                    stops.push((clamp01(o), self.color(c)?));
                }
                stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                Some(PFill::Grad(Grad { radial: kind == "radial", stops, angle: g.angle, center: g.center, radius: g.radius.unwrap_or(0.75).max(0.01), dither: g.dither.unwrap_or(0.12).max(0.0), steps: g.steps }))
            }
            FillSpec::Image(f) => {
                let fit = f.fit.clone().unwrap_or_else(|| "cover".into());
                one_of("image fit", &fit, &["cover", "contain", "stretch", "tile"])?;
                let base = match &f.base {
                    Some(c) => self.color(c)?,
                    None => Rgba([0.5, 0.5, 0.5, 1.0]),
                };
                Some(PFill::Image { src: f.image.clone(), fit, scale: f.scale.unwrap_or(1.0).max(0.01), focus: f.focus.unwrap_or([0.5, 0.5]), base })
            }
        })
    }
}

fn num2(v: &Option<Num2>, d: [f32; 2]) -> [f32; 2] {
    match v {
        None => d,
        Some(Num2::One(x)) => [*x, *x],
        Some(Num2::Two(a)) => *a,
    }
}

fn parse_svg(d: &str) -> Result<Path, Error> {
    Path::from_svg(d).map_err(|e| err(format!("bad SVG path `d`: {e}")))
}

fn anim_of(a: &Option<AnimSpec>) -> Result<Option<PAnim>, Error> {
    let Some(a) = a else { return Ok(None) };
    let (keys, looped, ease) = match a {
        AnimSpec::Keys(k) => (k.clone(), None, None),
        AnimSpec::Full(f) => (f.keys.clone(), f.looped.clone(), f.ease.clone()),
    };
    if keys.is_empty() {
        return Ok(None);
    }
    let ease = ease.unwrap_or_else(|| "inOut".into());
    one_of("ease", &ease, EASES)?;
    let mut keys = keys;
    keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    let last = keys.last().map(|k| k.t).unwrap_or(0.0);
    let period = match looped {
        None | Some(LoopSpec::On(false)) => None,
        Some(LoopSpec::On(true)) => Some(last.max(0.01)),
        Some(LoopSpec::Period(p)) => Some(p.max(0.01)),
    };
    let mut out = PAnim { keys: Vec::new(), period, ease, morphs: Vec::new() };
    for k in keys {
        if let Some(e) = &k.ease {
            one_of("ease", e, EASES)?;
        }
        let morph = match &k.d {
            None => None,
            Some(d) => {
                let p = parse_svg(d)?;
                let mut rs: Vec<(Vec<[f32; 2]>, bool)> = polylines(&p, 0.2).into_iter().map(|(r, c)| (resample_n(&r, c, MORPH_N), c)).collect();
                // Line each ring up with the previous target's, so the morph doesn't twist.
                if let Some(prev) = out.morphs.last() {
                    if prev.len() == rs.len() {
                        for (r, pr) in rs.iter_mut().zip(prev) {
                            if r.1 && pr.1 {
                                align_ring(&pr.0, &mut r.0);
                            }
                        }
                    }
                }
                out.morphs.push(rs);
                Some(out.morphs.len() - 1)
            }
        };
        let (sx, sy) = match k.squash {
            Some(q) => (Some(k.sx.unwrap_or(1.0) / q.max(0.05)), Some(k.sy.unwrap_or(1.0) * q)),
            None => (k.sx.or(k.scale), k.sy.or(k.scale)),
        };
        out.keys.push(PKey { t: k.t, x: k.x, y: k.y, rot: k.rotate, sx, sy, op: k.opacity, morph, ease: k.ease });
    }
    Ok(Some(out))
}

impl Prepare for Illo {
    fn prepare(&mut self) -> Result<(), Error> {
        if let Some(src) = self.svg.take() {
            let (mut shapes, vb) = svg_shapes(&src)?;
            if self.view.is_none() {
                self.view = vb;
            }
            shapes.append(&mut self.shapes);
            self.shapes = shapes;
        }
        let mut palette = HashMap::new();
        match &self.palette {
            None => {}
            Some(Value::Array(a)) => {
                for (i, v) in a.iter().enumerate() {
                    let s = v.as_str().ok_or_else(|| err("palette entries are colour strings"))?;
                    palette.insert((i + 1).to_string(), parse_color(s).ok_or_else(|| err(format!("palette: not a colour: {s:?}")))?);
                }
            }
            Some(Value::Object(m)) => {
                for (k, v) in m {
                    let s = v.as_str().ok_or_else(|| err("palette entries are colour strings"))?;
                    palette.insert(k.clone(), parse_color(s).ok_or_else(|| err(format!("palette: not a colour: {s:?}")))?);
                }
            }
            Some(_) => return Err(err("palette is an array of colours (\"$1\"…) or an object of named colours (\"$skin\")")),
        }
        let mut inks = Vec::new();
        for s in &self.inks {
            let c = if let Some(k) = s.strip_prefix('$') { palette.get(k).copied() } else { parse_color(s) };
            let c = c.ok_or_else(|| err(format!("inks: not a colour: {s:?}")))?;
            inks.push((c, lab(c)));
        }
        let rv = Resolver { palette, inks };

        let mut p = Prepared {
            light_color: rv.color(&self.light.color)?,
            shadow_tint: rv.color(&self.light.shadow)?,
            ..Default::default()
        };
        let hand_color = self.hand.color.as_deref().map(|c| rv.color(c)).transpose()?;
        let global_corner = corner_of(&self.corners)?;
        let names: Vec<Option<&str>> = self.shapes.iter().map(|s| s.name.as_deref()).collect();
        let resolve = |r: &Ref, me: usize| -> Result<usize, Error> {
            let i = match r {
                Ref::Index(i) => *i,
                Ref::Name(n) => names.iter().position(|x| *x == Some(n.as_str())).ok_or_else(|| {
                    let all: Vec<&str> = names.iter().flatten().copied().collect();
                    let hint = closest(n, &all).map(|c| format!(" (did you mean {c:?}?)")).unwrap_or_default();
                    err(format!("shape {me}: no shape named {n:?}{hint}"))
                })?,
            };
            if i >= names.len() || i == me {
                return Err(err(format!("shape {me}: reference {r:?} must name another shape (0..{})", names.len())));
            }
            Ok(i)
        };
        // Groups.
        let gnames: Vec<&str> = self.groups.iter().map(|g| g.name.as_str()).collect();
        let find_group = |n: &str| -> Result<usize, Error> {
            gnames.iter().position(|g| *g == n).ok_or_else(|| {
                let hint = closest(n, &gnames).map(|c| format!(" (did you mean {c:?}?)")).unwrap_or_default();
                err(format!("no group named {n:?}{hint}"))
            })
        };
        for g in &self.groups {
            p.groups.push(PGroup {
                parent: g.parent.as_deref().map(find_group).transpose()?,
                x: g.x,
                y: g.y,
                rotate: g.rotate,
                scale: num2(&g.scale, [1.0, 1.0]),
                origin: g.pivot.unwrap_or(g.origin),
                cycle: cycle_of(&g.cycle).map_err(|e| err(format!("group {:?}: {}", g.name, e.0)))?,
                anim: anim_of(&g.animate)?,
                sway: g.sway.clone(),
                lag: g.lag,
            });
        }
        for (i, g) in p.groups.iter().enumerate() {
            let mut cur = g.parent;
            let mut depth = 0;
            while let Some(c) = cur {
                depth += 1;
                if c == i || depth > 16 {
                    return Err(err(format!("group {:?}: parent chain loops", gnames[i])));
                }
                cur = p.groups[c].parent;
            }
        }
        let g_shade = self.shade.clone();
        let g_grain = self.grain.clone();
        let g_tex = self.texture.clone();
        let g_mr = num2(&self.misregister, [0.0, 0.0]);
        for (i, s) in self.shapes.iter().enumerate() {
            let ctx = |e: Error| err(format!("shape {i}{}: {}", s.name.as_deref().map(|n| format!(" ({n})")).unwrap_or_default(), e.0));
            one_of("shape", &s.shape, SHAPES).map_err(ctx)?;
            let corner = corner_of(&s.corners).map_err(ctx)?.or(global_corner);
            let cx = s.cx.unwrap_or(0.0);
            let cy = s.cy.unwrap_or(0.0);
            let need = |v: Option<f32>, what: &str| v.ok_or_else(|| ctx(err(format!("`{}` needs `{what}`", s.shape))));
            let (base, closed) = match s.shape.as_str() {
                "circle" | "ellipse" => {
                    let rx = s.rx.or(s.w.map(|w| w * 0.5)).or(s.r).ok_or_else(|| ctx(err("needs `r` (or `rx`/`ry`)")))?;
                    let ry = s.ry.or(s.h.map(|h| h * 0.5)).or(s.r).unwrap_or(rx);
                    (kurbo::Ellipse::new((cx as f64, cy as f64), (rx as f64, ry as f64), 0.0).to_path(0.05), true)
                }
                "rect" => {
                    let w = need(s.w, "w")?;
                    let h = need(s.h, "h")?;
                    let (x0, y0) = match (s.x, s.y) {
                        (Some(x), Some(y)) => (x, y),
                        _ => (cx - w * 0.5, cy - h * 0.5),
                    };
                    let r = s.radius.unwrap_or(match corner {
                        None => 0.0,
                        Some(Corner::Px(p)) => p,
                        Some(Corner::Frac(f)) => f * w.min(h),
                        Some(Corner::Soft(f, cap)) => (f * w.min(h)).min(cap),
                    });
                    let r = r.min(w.min(h) * 0.5).max(0.0);
                    (kurbo::RoundedRect::new(x0 as f64, y0 as f64, (x0 + w) as f64, (y0 + h) as f64, r as f64).to_path(0.05), true)
                }
                "polygon" | "star" => {
                    let pts: Vec<[f32; 2]> = match (&s.points, s.shape.as_str()) {
                        (Some(p), "polygon") => p.clone(),
                        _ => {
                            let r = need(s.r, "r")?;
                            let star = s.shape == "star";
                            let n = s.n.unwrap_or(if star { 5 } else { 6 }).clamp(3, 64) as usize;
                            let inner = s.inner.unwrap_or(0.45);
                            let m = if star { n * 2 } else { n };
                            (0..m)
                                .map(|k| {
                                    let a = k as f32 / m as f32 * TAU - PI * 0.5;
                                    let rr = if star && k % 2 == 1 { r * inner } else { r };
                                    [cx + rr * a.cos(), cy + rr * a.sin()]
                                })
                                .collect()
                        }
                    };
                    if pts.len() < 3 {
                        return Err(ctx(err("a polygon needs at least 3 points")));
                    }
                    if s.smooth {
                        (smooth_closed(&pts), true)
                    } else {
                        (filleted(&pts, s.radius.map(Corner::Px).or(corner).unwrap_or(Corner::Px(0.0))), true)
                    }
                }
                "blob" => {
                    let rx = s.rx.or(s.r).ok_or_else(|| ctx(err("needs `r` (or `rx`/`ry`)")))?;
                    let ry = s.ry.or(s.r).unwrap_or(rx);
                    let lumpy = s.lumpy.unwrap_or(0.22);
                    let seed = s.seed.unwrap_or(i as u32 * 31 + self.seed);
                    let n = s.n.unwrap_or(9).clamp(4, 40) as usize;
                    let pts: Vec<[f32; 2]> = (0..n)
                        .map(|k| {
                            let a = k as f32 / n as f32 * TAU;
                            let rr = 1.0 + lumpy * (hash01(seed.wrapping_mul(977) ^ k as u32 * 0x9e37) * 2.0 - 1.0);
                            [cx + rx * rr * a.cos(), cy + ry * rr * a.sin()]
                        })
                        .collect();
                    (smooth_closed(&pts), true)
                }
                _ => {
                    if let Some(d) = &s.d {
                        let p = parse_svg(d).map_err(ctx)?;
                        let has_close = p.elements().iter().any(|e| matches!(e, PathEl::ClosePath));
                        let closed = s.closed.unwrap_or(has_close);
                        (p, closed)
                    } else if let Some(pts) = &s.points {
                        if pts.len() < 2 {
                            return Err(ctx(err("`points` needs at least 2 points")));
                        }
                        let closed = s.closed.unwrap_or(false);
                        match (closed, s.smooth) {
                            (true, true) => (smooth_closed(pts), true),
                            (true, false) => (filleted(pts, corner.unwrap_or(Corner::Px(0.0))), true),
                            (false, true) => (smooth_open(pts), false),
                            (false, false) => {
                                let mut p = Path::new();
                                p.move_to(pt(pts[0]));
                                for q in &pts[1..] {
                                    p.line_to(pt(*q));
                                }
                                (p, false)
                            }
                        }
                    } else {
                        return Err(ctx(err("a `path` needs `d` or `points`")));
                    }
                }
            };
            let bb = bbox(&base);
            let origin = s.origin.unwrap_or([((bb.x0 + bb.x1) * 0.5) as f32, ((bb.y0 + bb.y1) * 0.5) as f32]);
            let rv_own;
            let rv = match &s.inks {
                None => &rv,
                Some(list) => {
                    let mut inks = Vec::new();
                    for c in list {
                        let c = rv.color_raw(c).map_err(ctx)?;
                        inks.push((c, lab(c)));
                    }
                    rv_own = Resolver { palette: rv.palette.clone(), inks };
                    &rv_own
                }
            };
            let lt = {
                let o = s.light.clone().unwrap_or_default();
                let a = o.angle.unwrap_or(self.light.angle).to_radians();
                let r = o.rim.or(o.angle.filter(|_| self.light.rim.is_none())).or(self.light.rim).unwrap_or(self.light.angle).to_radians();
                Lit {
                    dir: [a.cos(), a.sin()],
                    rim: [r.cos(), r.sin()],
                    color: match &o.color {
                        Some(c) => rv.color(c).map_err(ctx)?,
                        None => p.light_color,
                    },
                    tint: match &o.shadow {
                        Some(c) => rv.color(c).map_err(ctx)?,
                        None => p.shadow_tint,
                    },
                    strength: o.strength.unwrap_or(self.light.strength).max(0.0),
                }
            };
            let fill = match &s.fill {
                Some(f) => rv.fill(f).map_err(ctx)?,
                None if closed && s.stroke.is_none() => Some(PFill::Solid(Rgba([0.5, 0.5, 0.5, 1.0]))),
                None => None,
            };
            let fill = if closed { fill } else { None };
            // Line: on for open paths, `outline`, `stroke`, or the hand's default.
            let line_on = match &s.outline {
                Some(OnOff::On(b)) => *b,
                Some(OnOff::Amount(a)) => *a > 0.0,
                Some(OnOff::Full(_)) => true,
                None => !closed || s.stroke.is_some() || s.line_color.is_some() || self.hand.outline || self.paint_over,
            };
            let line_k = match &s.outline {
                Some(OnOff::Amount(a)) => *a,
                Some(OnOff::Full(a)) => *a,
                _ => 1.0,
            };
            let line = if line_on {
                let c = match s.line_color.as_deref().or(s.stroke.as_deref()) {
                    Some(c) => rv.color(c).map_err(ctx)?,
                    None => match (hand_color, &fill) {
                        (Some(h), _) => h,
                        (None, Some(f)) => line_tone(f.base(), lt.tint, self.hand.darken),
                        (None, None) => line_tone(Rgba([0.2, 0.18, 0.22, 1.0]), lt.tint, 0.3),
                    },
                };
                let w = s.weight.unwrap_or(self.hand.weight);
                Some((c, [w[0] * line_k, w[1] * line_k]))
            } else {
                None
            };
            let shade_src = s.shade.clone().or(if closed && fill.is_some() { g_shade.clone() } else { None });
            let shade = match shade_src {
                None | Some(OnOff::On(false)) => None,
                Some(OnOff::On(true)) => Some(ShadeSpec::default()),
                Some(OnOff::Amount(a)) if a <= 0.0 => None,
                Some(OnOff::Amount(a)) => Some(ShadeSpec { amount: a, ..Default::default() }),
                Some(OnOff::Full(f)) => Some(f),
            };
            let shade = match shade {
                None => None,
                Some(f) => Some(PShade {
                    amount: f.amount,
                    offset: f.offset,
                    soft: f.soft.max(0.0),
                    rim: f.rim.max(0.0),
                    rim_amount: clamp01(f.rim_amount),
                    color: f.color.as_deref().map(|c| rv.color(c)).transpose().map_err(ctx)?,
                    rim_color: f.rim_color.as_deref().map(|c| rv.color(c)).transpose().map_err(ctx)?,
                }),
            };
            let shadow = match &s.shadow {
                None | Some(OnOff::On(false)) => None,
                Some(o) => {
                    let f = match o {
                        OnOff::Full(f) => f.clone(),
                        OnOff::Amount(a) => ShadowSpec { dist: Some(*a), ..Default::default() },
                        _ => ShadowSpec::default(),
                    };
                    Some(PShadow {
                        dist: f.dist,
                        dx: f.dx,
                        dy: f.dy,
                        flatten: f.flatten,
                        color: match &f.color {
                            Some(c) => rv.color(c).map_err(ctx)?,
                            None => lt.tint,
                        },
                        opacity: clamp01(f.opacity.unwrap_or(0.7)),
                        blur: f.blur.max(0.0),
                        clip: f.clip_to.as_ref().map(|r| resolve(r, i)).transpose()?,
                    })
                }
            };
            let grain = match s.grain.clone().or(if closed { g_grain.clone() } else { None }) {
                None => None,
                Some(GrainOpt::Amount(a)) => (a > 0.0).then_some((a, 1.4)),
                Some(GrainOpt::Full(g)) => Some((g.amount.unwrap_or(0.06), g.scale.unwrap_or(1.4).max(0.5))),
            };
            let paint_tex = (self.paint_over && closed && fill.is_some()).then(|| TextureSpec::Full(TextureFull { style: "hatch".into(), amount: Some(0.75), scale: Some(7.0), ..Default::default() }));
            let texture = match s.texture.clone().or(paint_tex).or(if closed { g_tex.clone() } else { None }) {
                None => None,
                Some(t) => {
                    let f = match t {
                        TextureSpec::Name(n) => TextureFull { style: n, ..Default::default() },
                        TextureSpec::Full(f) => f,
                    };
                    if f.style == "none" {
                        None
                    } else {
                        one_of("texture", &f.style, TEXTURES).map_err(ctx)?;
                        let (amt, sc, an) = match f.style.as_str() {
                            "paper" => (0.5, 1.0, 0.0),
                            "halftone" => (0.85, 9.0, 45.0),
                            "riso" => (0.35, 1.5, 0.0),
                            _ => (0.6, 7.0, 35.0),
                        };
                        Some(PTex {
                            style: f.style,
                            amount: f.amount.unwrap_or(amt),
                            scale: f.scale.unwrap_or(sc).max(0.6),
                            angle: f.angle.unwrap_or(an),
                            color: f.color.as_deref().map(|c| rv.color(c)).transpose().map_err(ctx)?,
                        })
                    }
                }
            };
            let blend = match s.blend.as_deref() {
                None => {
                    if self.overprint {
                        Blend::Multiply
                    } else {
                        Blend::Normal
                    }
                }
                Some(b) => {
                    let b = super::camel(&super::kebab(b).replace('_', "-"));
                    one_of("blend", &b, BLENDS).map_err(ctx)?;
                    match b.as_str() {
                        "multiply" => Blend::Multiply,
                        "screen" => Blend::Screen,
                        "overlay" => Blend::Overlay,
                        "darken" => Blend::Darken,
                        "lighten" => Blend::Lighten,
                        "softLight" => Blend::SoftLight,
                        "add" => Blend::Add,
                        "difference" => Blend::Difference,
                        _ => Blend::Normal,
                    }
                }
            };
            let cut = match &s.cut {
                None => Vec::new(),
                Some(Refs::One(r)) => vec![resolve(r, i)?],
                Some(Refs::Many(v)) => v.iter().map(|r| resolve(r, i)).collect::<Result<_, _>>()?,
            };
            p.shapes.push(PShape {
                base,
                closed,
                origin,
                rotate: s.rotate,
                scale: num2(&s.scale, [1.0, 1.0]),
                fill,
                opacity: clamp01(s.opacity),
                blend,
                line,
                taper: s.taper.unwrap_or(0.25),
                wobble: s.wobble.unwrap_or(self.hand.wobble).max(0.0),
                rough: s.rough.unwrap_or(self.hand.rough).max(0.0),
                shade,
                shadow,
                grain,
                texture,
                misregister: num2(&s.misregister, g_mr),
                clip: s.clip_to.as_ref().map(|r| resolve(r, i)).transpose()?,
                cut,
                anim: anim_of(&s.animate).map_err(ctx)?,
                sway: s.sway.clone(),
                group: s.group.as_deref().map(find_group).transpose().map_err(ctx)?,
                lag: s.lag,
                visible: s.visible,
                seed: s.seed.unwrap_or_else(|| (i as u32).wrapping_mul(0x9e37_79b9) ^ self.seed.wrapping_mul(0x85eb_ca6b)),
                paint_over: self.paint_over && closed,
                lt,
                gaps: s.gaps.unwrap_or(self.hand.gaps),
                boil: s.boil.unwrap_or(self.hand.boil).max(0.0),
            });
        }
        p.hash = fnv(&[b"illo", format!("{:?}", p.shapes.len()).as_bytes(), self.seed.to_le_bytes().as_slice()]);
        self.p = p;
        Ok(())
    }
}

// ------------------------------------------------------------------ svg import

/// `name="value"` attributes of one tag (plus `style="a:b; c:d"` entries).
fn svg_attrs(tag: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let b = tag.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && !b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let n0 = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b':' || b[i] == b'_') {
            i += 1;
        }
        let name = &tag[n0..i];
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            continue;
        }
        i += 1;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let q = b[i];
        if q != b'"' && q != b'\'' {
            continue;
        }
        i += 1;
        let v0 = i;
        while i < b.len() && b[i] != q {
            i += 1;
        }
        let val = &tag[v0..i.min(b.len())];
        i += 1;
        if name == "style" {
            for kv in val.split(';') {
                if let Some((k, v)) = kv.split_once(':') {
                    out.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        } else {
            out.entry(name.to_string()).or_insert_with(|| val.to_string());
        }
    }
    out
}

/// Shapes (and the `viewBox` size) from an SVG string.
fn svg_shapes(src: &str) -> Result<(Vec<ShapeSpec>, Option<[f32; 2]>), Error> {
    let mut shapes = Vec::new();
    let mut view = None;
    let mut rest = src;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt + 1..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        rest = &rest[end..];
        let name: String = tag.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        let a = svg_attrs(&tag[name.len()..]);
        let num = |k: &str| a.get(k).and_then(|v| v.trim_end_matches("px").trim().parse::<f32>().ok());
        if name == "svg" {
            if let Some(vb) = a.get("viewBox") {
                let v: Vec<f32> = vb.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|x| x.parse().ok()).collect();
                if v.len() == 4 && v[2] > 0.0 && v[3] > 0.0 {
                    view = Some([v[2], v[3]]);
                }
            } else if let (Some(w), Some(h)) = (num("width"), num("height")) {
                view = Some([w, h]);
            }
            continue;
        }
        let mut sh = ShapeSpec { name: a.get("id").cloned().or_else(|| Some(format!("svg{}", shapes.len()))), ..Default::default() };
        let pts = |k: &str| -> Option<Vec<[f32; 2]>> {
            let v: Vec<f32> = a.get(k)?.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|x| x.parse().ok()).collect();
            Some(v.chunks_exact(2).map(|c| [c[0], c[1]]).collect())
        };
        match name.as_str() {
            "path" => {
                sh.d = a.get("d").cloned();
                if sh.d.is_none() {
                    continue;
                }
            }
            "circle" => {
                sh.shape = "circle".into();
                (sh.cx, sh.cy, sh.r) = (num("cx").or(Some(0.0)), num("cy").or(Some(0.0)), num("r"));
            }
            "ellipse" => {
                sh.shape = "ellipse".into();
                (sh.cx, sh.cy, sh.rx, sh.ry) = (num("cx").or(Some(0.0)), num("cy").or(Some(0.0)), num("rx"), num("ry"));
            }
            "rect" => {
                sh.shape = "rect".into();
                (sh.x, sh.y, sh.w, sh.h, sh.radius) = (num("x").or(Some(0.0)), num("y").or(Some(0.0)), num("width"), num("height"), num("rx").or(Some(0.0)));
            }
            "polygon" | "polyline" => {
                sh.points = pts("points");
                sh.closed = Some(name == "polygon");
                if name == "polygon" {
                    sh.shape = "polygon".into();
                }
            }
            "line" => {
                sh.points = Some(vec![[num("x1").unwrap_or(0.0), num("y1").unwrap_or(0.0)], [num("x2").unwrap_or(0.0), num("y2").unwrap_or(0.0)]]);
            }
            _ => continue,
        }
        let open = name == "line" || name == "polyline";
        let fill = a.get("fill").map(|s| s.as_str()).unwrap_or(if open { "none" } else { "#000000" });
        sh.fill = Some(FillSpec::Color(if fill == "currentColor" { "#000000".into() } else { fill.to_string() }));
        if let Some(st) = a.get("stroke").filter(|s| s.as_str() != "none") {
            sh.stroke = Some(if st == "currentColor" { "#000000".into() } else { st.clone() });
            let w = num("stroke-width").unwrap_or(1.0);
            sh.weight = Some([w * 0.6, w * 1.4]);
        }
        if let Some(o) = num("opacity").or(num("fill-opacity")) {
            sh.opacity = o;
        }
        shapes.push(sh);
    }
    if shapes.is_empty() {
        return Err(err("svg: no <path>, <circle>, <ellipse>, <rect>, <polygon>, <polyline> or <line> elements found"));
    }
    Ok((shapes, view))
}

// ------------------------------------------------------------------ animation

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Pose {
    dx: f32,
    dy: f32,
    rot: f32,
    sx: f32,
    sy: f32,
    op: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Morph {
    None,
    /// Morph targets `a` → `b` at `u` (same ring count: point lerp).
    Lerp(usize, usize, f32),
    /// Different structure: cross-fade.
    Cross(usize, usize, f32),
}

fn sample_anim(a: &PAnim, t: f32) -> (Pose, Morph) {
    let t = match a.period {
        Some(p) => t.rem_euclid(p),
        None => t,
    };
    let prop = |get: &dyn Fn(&PKey) -> Option<f32>, def: f32| -> f32 {
        let mut prev: Option<(f32, f32)> = None;
        for k in &a.keys {
            if let Some(v) = get(k) {
                if k.t >= t {
                    return match prev {
                        None => v,
                        Some((pt, pv)) => {
                            let e = k.ease.as_deref().unwrap_or(&a.ease);
                            pv + (v - pv) * ease_fn(e, (t - pt) / (k.t - pt).max(1e-6))
                        }
                    };
                }
                prev = Some((k.t, v));
            }
        }
        prev.map(|p| p.1).unwrap_or(def)
    };
    let pose = Pose {
        dx: prop(&|k| k.x, 0.0),
        dy: prop(&|k| k.y, 0.0),
        rot: prop(&|k| k.rot, 0.0),
        sx: prop(&|k| k.sx, 1.0),
        sy: prop(&|k| k.sy, 1.0),
        op: prop(&|k| k.op, 1.0),
    };
    let mut morph = Morph::None;
    let mut prev: Option<(f32, usize)> = None;
    for k in &a.keys {
        if let Some(m) = k.morph {
            if k.t >= t {
                morph = match prev {
                    None => Morph::Lerp(m, m, 0.0),
                    Some((pt, pm)) => {
                        let e = k.ease.as_deref().unwrap_or(&a.ease);
                        let u = ease_fn(e, (t - pt) / (k.t - pt).max(1e-6));
                        let same = a.morphs[pm].len() == a.morphs[m].len() && a.morphs[pm].iter().zip(&a.morphs[m]).all(|(x, y)| x.1 == y.1);
                        if same {
                            Morph::Lerp(pm, m, u)
                        } else {
                            Morph::Cross(pm, m, u)
                        }
                    }
                };
                prev = None;
                break;
            }
            prev = Some((k.t, m));
        }
    }
    if let Some((_, m)) = prev {
        morph = Morph::Lerp(m, m, 0.0);
    }
    (pose, morph)
}

fn sway_at(s: &Sway, t: f32) -> Pose {
    let ph = (t / s.period.unwrap_or(3.0).max(0.05) + s.phase) * TAU;
    Pose { dx: s.x * (ph + 0.9).sin(), dy: s.y * (ph * 0.5 + 0.3).sin(), rot: s.rotate * ph.sin(), sx: 1.0 + s.scale * (ph + 1.7).sin(), sy: 1.0 + s.scale * (ph + 1.7).sin(), op: 1.0 }
}

fn pose_at(anim: &Option<PAnim>, sway: &Option<Sway>, t: f32) -> (Pose, Morph) {
    let (mut p, m) = match anim {
        Some(a) => sample_anim(a, t),
        None => (Pose { sx: 1.0, sy: 1.0, op: 1.0, ..Default::default() }, Morph::None),
    };
    if let Some(s) = sway {
        let q = sway_at(s, t);
        p.dx += q.dx;
        p.dy += q.dy;
        p.rot += q.rot;
        p.sx *= q.sx;
        p.sy *= q.sy;
    }
    (p, m)
}

// ------------------------------------------------------------------ eval

#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    m: [f32; 6],
    op: f32,
    morph: Morph,
    frame: u32,
}

impl State {
    fn hash(&self) -> u64 {
        let mut b = Vec::with_capacity(48);
        for v in self.m {
            b.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        b.extend_from_slice(&self.op.to_bits().to_le_bytes());
        let (k, a, c, u) = match self.morph {
            Morph::None => (0u32, 0usize, 0usize, 0.0f32),
            Morph::Lerp(a, c, u) => (1, a, c, u),
            Morph::Cross(a, c, u) => (2, a, c, u),
        };
        b.extend_from_slice(&k.to_le_bytes());
        b.extend_from_slice(&(a as u32).to_le_bytes());
        b.extend_from_slice(&(c as u32).to_le_bytes());
        b.extend_from_slice(&u.to_bits().to_le_bytes());
        b.extend_from_slice(&self.frame.to_le_bytes());
        fnv(&[&b])
    }
}

struct Frame<'a> {
    il: &'a Illo,
    /// Local time (for video fills).
    t: f32,
    states: Vec<State>,
    /// View → box scale.
    k: f32,
    ctx: &'a Ctx<'a>,
}

impl Illo {
    fn group_m(&self, g: usize, t: f32, depth: u32) -> [f32; 6] {
        let gr = &self.p.groups[g];
        let tl = t - gr.lag;
        let (mut pose, _) = pose_at(&gr.anim, &gr.sway, tl);
        if let Some(c) = &gr.cycle {
            let q = cycle_at(c, tl);
            pose.dx += q.dx;
            pose.dy += q.dy;
            pose.rot += q.rot;
            pose.sx *= q.sx;
            pose.sy *= q.sy;
        }
        let own = trs(gr.x + pose.dx, gr.y + pose.dy, gr.rotate + pose.rot, [gr.scale[0] * pose.sx, gr.scale[1] * pose.sy], gr.origin);
        match gr.parent {
            Some(p) if depth < 16 => mul(&self.group_m(p, tl, depth + 1), &own),
            _ => own,
        }
    }

    fn states(&self, t: f32, fit: &[f32; 6]) -> Vec<State> {
        self.p
            .shapes
            .iter()
            .map(|s| {
                let ts = t - s.lag;
                let (pose, morph) = pose_at(&s.anim, &s.sway, ts);
                let own = trs(pose.dx, pose.dy, s.rotate + pose.rot, [s.scale[0] * pose.sx, s.scale[1] * pose.sy], s.origin);
                let g = match s.group {
                    Some(g) => mul(&self.group_m(g, ts, 0), &own),
                    None => own,
                };
                let frame = if s.boil > 0.0 { (t * s.boil).floor().max(0.0) as u32 + 1 } else { 0 };
                State { m: mul(fit, &g), op: s.opacity * pose.op.clamp(0.0, 1.0), morph, frame }
            })
            .collect()
    }
}

impl Frame<'_> {
    /// The shape's outline in box pixels (`which` picks the cross-fade side).
    fn body(&self, i: usize, which: bool) -> Path {
        let s = &self.il.p.shapes[i];
        let st = &self.states[i];
        let local = match (st.morph, &s.anim) {
            (Morph::None, _) | (_, None) => s.base.clone(),
            (Morph::Lerp(a, b, u), Some(an)) => {
                let rs: Vec<(Vec<[f32; 2]>, bool)> = an.morphs[a]
                    .iter()
                    .zip(&an.morphs[b])
                    .map(|((ra, c), (rb, _))| (ra.iter().zip(rb).map(|(p, q)| [p[0] + (q[0] - p[0]) * u, p[1] + (q[1] - p[1]) * u]).collect(), *c))
                    .collect();
                rings_path(&rs)
            }
            (Morph::Cross(a, b, _), Some(an)) => rings_path(&an.morphs[if which { b } else { a }]),
        };
        let seed = s.seed ^ st.frame.wrapping_mul(0x2545_f491);
        let mut p = if s.closed { roughen(&local, s.wobble, s.rough, seed) } else { local };
        p.apply_affine(kaff(&st.m));
        p
    }

    /// `(cast shadow, everything else)` for shape `i`.
    fn ops(&self, i: usize) -> (Vec<Op>, Vec<Op>) {
        let st = self.states[i];
        let b = self.body(i, false);
        let shadow = self.shadow_ops(i, &b, st.op);
        let body = match st.morph {
            Morph::Cross(_, _, u) => {
                let mut out = super::with_opacity(self.shape_ops(i, b, 1.0), 1.0 - u);
                out.extend(super::with_opacity(self.shape_ops(i, self.body(i, true), 1.0), u));
                super::with_opacity(out, st.op)
            }
            _ => self.shape_ops(i, b, st.op),
        };
        (shadow, body)
    }

    /// Cast shadow: a hard silhouette away from the light (or laid on the ground).
    fn shadow_ops(&self, i: usize, b: &Path, op: f32) -> Vec<Op> {
        let pp = &self.il.p;
        let s = &pp.shapes[i];
        let k = self.k;
        let mut out = Vec::new();
        let Some(sh) = &s.shadow else { return out };
        let bb = bbox(b);
        let size = (bb.width().min(bb.height()) as f32).max(1.0);
        let [lx, ly] = s.lt.dir;
        let mut sp = b.clone();
        if let Some(f) = sh.flatten {
            // Heights above the base scale by `f` (positive: the shadow falls back,
            // up the screen; negative: forward) and lean away from the light.
            let base = bb.y1 as f32;
            let c = lx * 1.1 * f.abs();
            let m = [1.0, 0.0, c, f, -c * base, base - f * base];
            sp.apply_affine(kaff(&m));
        }
        let d = sh.dist.map(|d| d * k).unwrap_or(if sh.flatten.is_some() { 0.0 } else { 0.12 * size });
        sp = moved(&sp, -lx * d + sh.dx * k, -ly * d + sh.dy * k);
        let clip = sh.clip.map(|c| (self.body(c, false), IDENTITY));
        let filters = if sh.blur > 0.0 { vec![Filter::Blur(sh.blur * k)] } else { Vec::new() };
        out.push(Op::PushLayer { opacity: sh.opacity * op * s.lt.strength.min(1.0), blend: Blend::Multiply, filters, clip, cache_key: None });
        out.push(Op::Fill { path: sp, paint: Paint::Solid(sh.color), rule: FillRule::NonZero, transform: IDENTITY });
        out.push(Op::PopLayer);
        out
    }

    fn shape_ops(&self, i: usize, b: Path, op: f32) -> Vec<Op> {
        let il = self.il;
        let pp = &il.p;
        let s = &pp.shapes[i];
        let k = self.k;
        let mut out = Vec::new();
        let bb = bbox(&b);
        if bb.width() <= 0.0 && bb.height() <= 0.0 {
            return out;
        }
        let size = (bb.width().min(bb.height()) as f32).max(1.0);
        let big = |pad: f64| kurbo::Rect::new(bb.x0 - pad, bb.y0 - pad, bb.x1 + pad, bb.y1 + pad).to_path(0.1);
        let [lx, ly] = s.lt.dir;
        let strength = s.lt.strength;

        let clip = s.clip.map(|c| (self.body(c, false), IDENTITY));
        let outer = op < 0.999 || s.blend != Blend::Normal || clip.is_some() || !s.cut.is_empty();
        if outer {
            out.push(Op::PushLayer { opacity: op, blend: s.blend, filters: Vec::new(), clip, cache_key: None });
        }
        if !s.cut.is_empty() {
            out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: None, cache_key: None });
        }
        let mr = [s.misregister[0] * k, s.misregister[1] * k];
        if let Some(fill) = &s.fill {
            let bf = moved(&b, mr[0], mr[1]);
            let paint = match fill {
                PFill::Solid(c) => Paint::Solid(*c),
                PFill::Grad(g) => self.grad_paint(g, &bf, s.seed, s.lt.dir),
                PFill::Image { src, fit, scale, focus, .. } => self.image_paint(src, fit, *scale, *focus, &bf),
            };
            if !s.paint_over {
                out.push(Op::Fill { path: bf.clone(), paint, rule: FillRule::NonZero, transform: IDENTITY });
            }
            let base = fill.base();
            if let Some(sh) = s.shade.as_ref().filter(|_| !s.paint_over) {
                // Overlays each get their own clipped layer at this level, so blend
                // modes see the fill below (a nested group would isolate them).
                out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: Some((bf.clone(), IDENTITY)), cache_key: None });
                let amt = (sh.amount * strength).min(1.5);
                // Form shadow: everything not covered by a copy pushed towards the light.
                let off = sh.offset * size;
                let mut p = big((off + sh.soft * size * 3.0 + 4.0) as f64);
                p.extend(moved(&bf, lx * off, ly * off).iter());
                let col = sh.color.unwrap_or_else(|| shadow_tone(base, s.lt.tint, amt));
                let soft = sh.soft * size;
                if soft > 0.3 {
                    out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Blur(soft)], clip: None, cache_key: None });
                }
                out.push(Op::Fill { path: p, paint: Paint::Solid(col), rule: FillRule::EvenOdd, transform: IDENTITY });
                if soft > 0.3 {
                    out.push(Op::PopLayer);
                }
                // Rim: the sliver a copy pushed away from the (rim) light leaves uncovered.
                if sh.rim > 0.0 && sh.rim_amount > 0.0 {
                    let [rx, ry] = s.lt.rim;
                    let rw = (sh.rim * size).clamp(1.2 * k, 14.0 * k);
                    let mut p = big((rw + 4.0) as f64);
                    p.extend(moved(&bf, -rx * rw, -ry * rw).iter());
                    let mut rc = sh.rim_color.unwrap_or_else(|| rim_tone(base, s.lt.color, (0.9 * strength).min(1.2)));
                    rc.0[3] *= sh.rim_amount;
                    out.push(Op::Fill { path: p, paint: Paint::Solid(rc), rule: FillRule::EvenOdd, transform: IDENTITY });
                }
                out.push(Op::PopLayer);
            }
            if let Some(tx) = &s.texture {
                self.texture_ops(&mut out, tx, &bf, base, s.seed, &st_m(&self.states[i]), &s.lt);
            }
            if let Some((amt, sc)) = s.grain.filter(|_| !s.paint_over) {
                let tile = grain_tile(self.ctx);
                let m = st_m(&self.states[i]);
                let o = (hash01(s.seed) * 256.0).floor();
                let tm = mul(&[1.0 / sc, 0.0, 0.0, 1.0 / sc, o, o], &mul(&[k, 0.0, 0.0, k, 0.0, 0.0], &inv(&m)));
                out.push(Op::PushLayer { opacity: clamp01(amt * 2.0), blend: Blend::Overlay, filters: Vec::new(), clip: Some((bf.clone(), IDENTITY)), cache_key: None });
                out.push(Op::Fill { path: big(2.0), paint: Paint::Image { image: tile, transform: tm, opacity: 1.0 }, rule: FillRule::NonZero, transform: IDENTITY });
                out.push(Op::PopLayer);
            }
        }
        if let Some((lc, w)) = s.line {
            let frame = self.states[i].frame;
            let jit = if frame > 0 { il.hand.boil_amount * k } else { 0.0 };
            let seed = s.seed ^ 0x6c1e ^ frame.wrapping_mul(0x9e37_79b9);
            let path = if s.closed { outline_path(&b, [w[0] * k, w[1] * k], s.lt.dir, s.gaps, jit, seed) } else { open_lines(&b, [w[0] * k, w[1] * k], s.taper, jit, seed) };
            out.push(Op::Fill { path, paint: Paint::Solid(lc), rule: FillRule::NonZero, transform: IDENTITY });
        }
        if !s.cut.is_empty() {
            out.push(Op::PushLayer { opacity: 1.0, blend: Blend::DestOut, filters: Vec::new(), clip: None, cache_key: None });
            for &c in &s.cut {
                out.push(Op::Fill { path: self.body(c, false), paint: Paint::Solid(Rgba([0.0, 0.0, 0.0, 1.0])), rule: FillRule::NonZero, transform: IDENTITY });
            }
            out.push(Op::PopLayer);
            out.push(Op::PopLayer);
        }
        if outer {
            out.push(Op::PopLayer);
        }
        out
    }

    /// A dithered OKLab gradient as an image locked to the shape's box.
    /// An image (or video frame) mapped onto the shape's box.
    fn image_paint(&self, src: &str, fit: &str, scale: f32, focus: [f32; 2], b: &Path) -> Paint {
        let Some((id, iw, ih)) = self.ctx.assets.media(src, self.t) else { return Paint::Solid(Rgba([0.5, 0.5, 0.5, 1.0])) };
        let bb = bbox(b);
        let (bw, bh) = (bb.width().max(1.0) as f32, bb.height().max(1.0) as f32);
        let (iw, ih) = (iw.max(1) as f32, ih.max(1) as f32);
        let (sx, sy) = match fit {
            "stretch" => (bw / iw, bh / ih),
            "contain" => {
                let k = (bw / iw).min(bh / ih);
                (k, k)
            }
            "tile" => (self.k, self.k),
            _ => {
                let k = (bw / iw).max(bh / ih);
                (k, k)
            }
        };
        let (sx, sy) = (sx * scale, sy * scale);
        // Image placed so its `focus` point sits at the same fraction of the box.
        let ox = bb.x0 as f32 + (bw - iw * sx) * focus[0];
        let oy = bb.y0 as f32 + (bh - ih * sy) * focus[1];
        // Paint transforms map local → image pixels.
        Paint::Image { image: id, transform: [1.0 / sx, 0.0, 0.0, 1.0 / sy, -ox / sx, -oy / sy], opacity: 1.0 }
    }

    fn grad_paint(&self, g: &Grad, b: &Path, seed: u32, light: [f32; 2]) -> Paint {
        let bb = bbox(b);
        let [lx, ly] = light;
        let (w, h) = ((bb.width().ceil() as usize + 4).clamp(2, 2048), (bb.height().ceil() as usize + 4).clamp(2, 2048));
        let (ox, oy) = (bb.x0.floor() as f32 - 2.0, bb.y0.floor() as f32 - 2.0);
        let mut kb = Vec::new();
        for (o, c) in &g.stops {
            kb.extend_from_slice(&o.to_le_bytes());
            for v in c.0 {
                kb.extend_from_slice(&v.to_le_bytes());
            }
        }
        let ang = g.angle.map(|a| a.to_radians());
        let (dx, dy) = match ang {
            Some(a) => (a.cos(), a.sin()),
            None => (-lx, -ly),
        };
        let center = g.center.unwrap_or([0.5 + 0.22 * lx, 0.5 + 0.22 * ly]);
        let params = [w as f32, h as f32, dx, dy, center[0], center[1], g.radius, g.dither, g.steps as f32, if g.radial { 1.0 } else { 0.0 }, (seed & 0xffff) as f32];
        for v in params {
            kb.extend_from_slice(&v.to_le_bytes());
        }
        let key = fnv(&[b"illo-grad", &kb]);
        let stops = g.stops.clone();
        let (radial, radius, dither, steps) = (g.radial, g.radius, g.dither, g.steps);
        let id = self.ctx.assets.image(key, w as u32, h as u32, &mut || {
            // 256-entry LUT along the gradient, in OKLab.
            let lut: Vec<[u8; 4]> = (0..256)
                .map(|i| {
                    let c = grad_at(&stops, i as f32 / 255.0);
                    [0, 1, 2, 3].map(|j| (c.0[j].clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
                })
                .collect();
            let mut px = vec![0u8; w * h * 4];
            let (fw, fh) = ((w - 4) as f32, (h - 4) as f32);
            let ext = 0.5 * (fw * dx.abs() + fh * dy.abs()).max(1.0);
            let rr = radius * fw.max(fh);
            let (cx, cy) = (2.0 + center[0] * fw, 2.0 + center[1] * fh);
            for y in 0..h {
                for x in 0..w {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                    let mut u = if radial {
                        ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() / rr.max(1.0)
                    } else {
                        0.5 + ((fx - 2.0 - fw * 0.5) * dx + (fy - 2.0 - fh * 0.5) * dy) / (2.0 * ext)
                    };
                    // Grain dither: a stochastic threshold, like riso "grain touch".
                    let n = hash2(x as i32, y as i32, seed) * 0.6 + hash2(x as i32 >> 1, y as i32 >> 1, seed ^ 0x55) * 0.4;
                    u += dither * n * 0.5;
                    if steps > 1 {
                        u = (clamp01(u) * steps as f32).floor().min(steps as f32 - 1.0) / (steps - 1) as f32;
                    }
                    let c = lut[(clamp01(u) * 255.0 + 0.5) as usize];
                    px[(y * w + x) * 4..][..4].copy_from_slice(&c);
                }
            }
            px
        });
        Paint::Image { image: id, transform: [1.0, 0.0, 0.0, 1.0, -ox, -oy], opacity: 1.0 }
    }

    fn texture_ops(&self, out: &mut Vec<Op>, tx: &PTex, b: &Path, base: Rgba, seed: u32, m: &[f32; 6], lt: &Lit) {
        let bb = bbox(b);
        let k = self.k;
        let ink = tx.color.unwrap_or_else(|| shadow_tone(base, lt.tint, 1.0));
        match tx.style.as_str() {
            "paper" => {
                let (w, h) = (self.ctx.width.ceil().clamp(4.0, 4096.0) as usize, self.ctx.height.ceil().clamp(4.0, 4096.0) as usize);
                let key = fnv(&[b"illo-paper", &(w as u32).to_le_bytes(), &(h as u32).to_le_bytes()]);
                let id = self.ctx.assets.image(key, w as u32, h as u32, &mut || paper_field("cotton", Rgba([1.0, 1.0, 1.0, 1.0]), 0.9, 0.7, 0.0, 7, w, h).to_rgba8());
                out.push(Op::PushLayer { opacity: clamp01(tx.amount), blend: Blend::Multiply, filters: Vec::new(), clip: Some((b.clone(), IDENTITY)), cache_key: None });
                out.push(Op::Image { image: id, dst: [0.0, 0.0, w as f32, h as f32], opacity: 1.0, transform: IDENTITY });
                out.push(Op::PopLayer);
            }
            "riso" => {
                let tile = speck_tile(self.ctx);
                let sc = tx.scale;
                let o = (hash01(seed ^ 0x77) * 256.0).floor();
                let tm = mul(&[1.0 / sc, 0.0, 0.0, 1.0 / sc, o, o], &mul(&[k, 0.0, 0.0, k, 0.0, 0.0], &inv(m)));
                // Specks of the shadow ink, denser towards the shadow side.
                let [lx, ly] = lt.dir;
                let c = (bb.center().x as f32, bb.center().y as f32);
                let ext = 0.5 * (bb.width() as f32 * lx.abs() + bb.height() as f32 * ly.abs()).max(1.0);
                out.push(Op::PushLayer { opacity: clamp01(tx.amount), blend: Blend::Multiply, filters: Vec::new(), clip: Some((b.clone(), IDENTITY)), cache_key: None });
                out.push(Op::Fill { path: bb.inflate(2.0, 2.0).to_path(0.1), paint: Paint::Image { image: tile, transform: tm, opacity: 1.0 }, rule: FillRule::NonZero, transform: IDENTITY });
                // Fade the specks out on the lit side.
                let lit = Rgba([1.0, 1.0, 1.0, 0.85]);
                out.push(Op::Fill {
                    path: bb.inflate(2.0, 2.0).to_path(0.1),
                    paint: Paint::Linear { p0: [c.0 + lx * ext, c.1 + ly * ext], p1: [c.0 - lx * ext * 0.2, c.1 - ly * ext * 0.2], stops: vec![(0.0, lit), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))] },
                    rule: FillRule::NonZero,
                    transform: IDENTITY,
                });
                out.push(Op::PopLayer);
                let _ = ink;
            }
            "halftone" => {
                let (w, h) = ((bb.width().ceil() as usize + 4).clamp(2, 2048), (bb.height().ceil() as usize + 4).clamp(2, 2048));
                let (ox, oy) = (bb.x0.floor() as f32 - 2.0, bb.y0.floor() as f32 - 2.0);
                let [lx, ly] = lt.dir;
                let cell = tx.scale * k.max(0.25);
                let ang = tx.angle.to_radians();
                let amt = tx.amount;
                let key = fnv(&[b"illo-halftone", &[w as f32, h as f32, lx, ly, cell, ang, amt, ink.0[0], ink.0[1], ink.0[2]].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()]);
                let id = self.ctx.assets.image(key, w as u32, h as u32, &mut || {
                    let mut px = vec![0u8; w * h * 4];
                    let (sa, ca) = ang.sin_cos();
                    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
                    let ext = 0.5 * ((w as f32 - 4.0) * lx.abs() + (h as f32 - 4.0) * ly.abs()).max(1.0);
                    let rgb = [0, 1, 2].map(|j| (ink.0[j].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
                    for y in 0..h {
                        for x in 0..w {
                            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                            // Tone: 0 on the lit side → amt on the shadow side.
                            let proj = -((fx - cx) * lx + (fy - cy) * ly) / ext;
                            let tone = amt * smoothstep(-0.7, 1.0, proj);
                            let (u, v) = ((fx * ca + fy * sa) / cell, (-fx * sa + fy * ca) / cell);
                            let (du, dv) = (u - u.floor() - 0.5, v - v.floor() - 0.5);
                            let d = (du * du + dv * dv).sqrt() * cell;
                            let r = cell * 0.62 * tone.sqrt();
                            let a = clamp01(r - d + 0.5);
                            let o = (y * w + x) * 4;
                            px[o..o + 3].copy_from_slice(&rgb);
                            px[o + 3] = (a * 255.0 + 0.5) as u8;
                        }
                    }
                    px
                });
                out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: Some((b.clone(), IDENTITY)), cache_key: None });
                out.push(Op::Image { image: id, dst: [ox, oy, w as f32, h as f32], opacity: 1.0, transform: IDENTITY });
                out.push(Op::PopLayer);
            }
            style => {
                let style = if style == "hatch" { "hatch" } else { style };
                let light_deg = lt.dir[1].atan2(lt.dir[0]).to_degrees();
                let (lines, dots) = hatch_lines(b.clone(), style, tx.angle, tx.scale * k, light_deg, (0.0, tx.amount), 1.3 * k, 0.5, seed);
                out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: Some((b.clone(), IDENTITY)), cache_key: None });
                let mut buckets: [Path; 3] = Default::default();
                for (p, _w, a) in lines {
                    let bi = ((a - 0.65) / 0.35 * 3.0).clamp(0.0, 2.0) as usize;
                    buckets[bi].extend(p.iter());
                }
                for (bi, p) in buckets.into_iter().enumerate() {
                    if !p.elements().is_empty() {
                        let mut c = ink;
                        c.0[3] = 0.55 + 0.15 * bi as f32;
                        out.push(Op::Stroke { path: p, paint: Paint::Solid(c), stroke: super::style::line(1.3 * k, [0.0, 1.0]), transform: IDENTITY });
                    }
                }
                if !dots.elements().is_empty() {
                    out.push(Op::Fill { path: dots, paint: Paint::Solid(ink), rule: FillRule::NonZero, transform: IDENTITY });
                }
                out.push(Op::PopLayer);
            }
        }
    }
}

fn st_m(s: &State) -> [f32; 6] {
    s.m
}

/// 256² tile of mid-grey film grain (two octaves of white noise), shared by every shape.
fn grain_tile(ctx: &Ctx) -> dl::ImageId {
    const N: usize = 256;
    ctx.assets.image(fnv(&[b"illo-grain-tile-v1"]), N as u32, N as u32, &mut || {
        let mut px = vec![255u8; N * N * 4];
        for y in 0..N {
            for x in 0..N {
                let (xi, yi) = (x as i32, y as i32);
                // Wrap the coarse octave so the tile repeats seamlessly.
                let c = |dx: i32, dy: i32| hash2(((xi >> 1) + dx).rem_euclid(N as i32 / 2), ((yi >> 1) + dy).rem_euclid(N as i32 / 2), 77);
                let n = 0.6 * hash2(xi, yi, 13) + 0.4 * c(0, 0);
                let v = (127.5 + n * 120.0).clamp(0.0, 255.0) as u8;
                let o = (y * N + x) * 4;
                px[o] = v;
                px[o + 1] = v;
                px[o + 2] = v;
            }
        }
        px
    })
}

/// 256² tile of riso specks: dark clumps on white (toroidal blur → seamless).
fn speck_tile(ctx: &Ctx) -> dl::ImageId {
    const N: usize = 256;
    ctx.assets.image(fnv(&[b"illo-speck-tile-v1"]), N as u32, N as u32, &mut || {
        let mut f = vec![0f32; N * N];
        for y in 0..N {
            for x in 0..N {
                f[y * N + x] = hash2(x as i32, y as i32, 991);
            }
        }
        // Two toroidal box blurs give clumps.
        for _ in 0..2 {
            let mut g = vec![0f32; N * N];
            for y in 0..N {
                for x in 0..N {
                    let mut s = 0.0;
                    for (dx, dy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                        s += f[((y as i32 + dy).rem_euclid(N as i32) as usize) * N + (x as i32 + dx).rem_euclid(N as i32) as usize];
                    }
                    g[y * N + x] = s / 5.0;
                }
            }
            f = g;
        }
        let mut px = vec![255u8; N * N * 4];
        for i in 0..N * N {
            let fine = hash2((i % N) as i32, (i / N) as i32, 5);
            let a = smoothstep(0.2, 0.34, f[i] + fine * 0.1);
            let v = (255.0 * (1.0 - 0.85 * a)) as u8;
            px[i * 4] = v;
            px[i * 4 + 1] = v;
            px[i * 4 + 2] = v;
        }
        px
    })
}

/// Variable-weight outline of a closed shape: heavy on the side away from the
/// light, thin (and broken by `gaps`) where it hits; centred on the edge.
fn outline_path(b: &Path, w: [f32; 2], light: [f32; 2], gaps: f32, jit: f32, seed: u32) -> Path {
    let mut out = Path::new();
    for (ri, ring) in rings(b, 2.0).into_iter().enumerate() {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        let pts: Vec<[f32; 2]> = ring.iter().map(|&(x, y)| [x, y]).collect();
        let sign = if signed_area(&pts) > 0.0 { 1.0 } else { -1.0 };
        let s = seed ^ (ri as u32).wrapping_mul(0x51ed_27a3);
        let mut outer = Vec::with_capacity(n);
        let mut inner = Vec::with_capacity(n);
        let mut arc = 0.0;
        for i in 0..n {
            let (a, c) = (pts[(i + n - 3) % n], pts[(i + 3) % n]);
            let (tx, ty) = (c[0] - a[0], c[1] - a[1]);
            let l = (tx * tx + ty * ty).sqrt().max(1e-6);
            let (nx, ny) = (ty / l * sign, -tx / l * sign);
            let facing = nx * light[0] + ny * light[1];
            let lit = smoothstep(-0.45, 0.85, facing);
            let mut hw = 0.5 * (w[1] + (w[0] - w[1]) * lit) * (1.0 + 0.22 * vnoise(arc / 38.0, s));
            if gaps > 0.0 {
                hw *= 1.0 - smoothstep(1.0 - gaps - 0.08, 1.0 - gaps + 0.04, lit + 0.08 * vnoise(arc / 25.0, s ^ 9));
            }
            let c0 = jit * vnoise(arc / 30.0, s ^ 0x33);
            let p = pts[i];
            outer.push(Point::new((p[0] + nx * (c0 + hw)) as f64, (p[1] + ny * (c0 + hw)) as f64));
            inner.push(Point::new((p[0] + nx * (c0 - hw)) as f64, (p[1] + ny * (c0 - hw)) as f64));
            let q = pts[(i + 1) % n];
            arc += ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
        }
        out.move_to(outer[0]);
        for q in &outer[1..] {
            out.line_to(*q);
        }
        out.close_path();
        out.move_to(inner[n - 1]);
        for q in inner.iter().rev().skip(1) {
            out.line_to(*q);
        }
        out.close_path();
    }
    out
}

/// Tapered, swelling brush lines along an open path (the brush component's ribbon).
fn open_lines(b: &Path, w: [f32; 2], taper: f32, jit: f32, seed: u32) -> Path {
    let mut out = Path::new();
    for (pi, (poly, _)) in polylines(b, 0.2).into_iter().enumerate() {
        let line = resample(&poly, jit, seed ^ (pi as u32 * 7));
        let m = line.p.len();
        if m < 2 {
            continue;
        }
        let len = line.len().max(1e-3);
        let t = (taper * len).max(0.5);
        let hw: Vec<f32> = (0..m)
            .map(|i| {
                let u = line.s[i] / len;
                let swell = (PI * u).sin().max(0.0).powf(0.7);
                (0.5 * (w[0] + (w[1] - w[0]) * swell) * tip(line.s[i] / t) * tip((len - line.s[i]) / t)).max(0.25)
            })
            .collect();
        ribbon(&mut out, &line, 0, m - 1, &|i| (0.0, hw[i], hw[i]));
    }
    out
}

impl Component for Illo {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let rate = if self.step_fps > 0.0 { self.step_fps } else if self.on_twos { 12.0 } else { 0.0 };
        let t = if rate > 0.0 { (t * rate + 1e-4).floor() / rate } else { t };
        let (fit, k) = match self.view {
            Some([vw, vh]) if vw > 0.0 && vh > 0.0 => {
                let k = (ctx.width / vw).min(ctx.height / vh);
                ([k, 0.0, 0.0, k, (ctx.width - vw * k) * 0.5, (ctx.height - vh * k) * 0.5], k)
            }
            _ => (IDENTITY, 1.0),
        };
        let states = self.states(t, &fit);
        let fr = Frame { il: self, t, states, k, ctx };
        let dims = [ctx.width.to_bits(), ctx.height.to_bits()];
        let n = self.p.shapes.len();
        let mut lists: Vec<(Vec<Op>, Vec<Op>)> = Vec::with_capacity(n);
        for (i, s) in self.p.shapes.iter().enumerate() {
            if !s.visible {
                lists.push(Default::default());
                continue;
            }
            // Everything the shape's ops depend on.
            let mut deps = vec![fr.states[i].hash(), i as u64, self.p.hash, dims[0] as u64, dims[1] as u64];
            if matches!(s.fill, Some(PFill::Image { .. })) {
                deps.push(t.to_bits() as u64);
            }
            for d in s.clip.iter().chain(s.cut.iter()).chain(s.shadow.as_ref().and_then(|sh| sh.clip).iter()) {
                deps.push(fr.states[*d].hash());
            }
            let key = fnv(&[&deps.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()]);
            let hit = self.p.cache.lock().ok().and_then(|c| c.get(&key).cloned());
            lists.push(match hit {
                Some(o) => o,
                None => {
                    let o = fr.ops(i);
                    if let Ok(mut c) = self.p.cache.lock() {
                        if c.len() > 4096 {
                            c.clear();
                        }
                        c.insert(key, o.clone());
                    }
                    o
                }
            });
        }
        // A shadow that falls on a shape (`clipTo`) is painted on it: right after
        // that shape, under everything drawn later (so it never darkens its caster).
        let mut on: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, s) in self.p.shapes.iter().enumerate() {
            if let Some(c) = s.shadow.as_ref().and_then(|sh| sh.clip) {
                on[c].push(i);
            }
        }
        let mut out = Vec::new();
        for j in 0..n {
            if !on.iter().any(|v| v.contains(&j)) {
                out.extend(lists[j].0.iter().cloned());
            }
            out.extend(lists[j].1.iter().cloned());
            for &i in &on[j] {
                out.extend(lists[i].0.iter().cloned());
            }
        }
        out
    }

    fn duration(&self) -> Option<f32> {
        let last = |a: &Option<PAnim>| a.as_ref().filter(|a| a.period.is_none()).and_then(|a| a.keys.last()).map(|k| k.t);
        let d = self.p.shapes.iter().filter_map(|s| last(&s.anim).map(|t| t + s.lag.max(0.0))).chain(self.p.groups.iter().filter_map(|g| last(&g.anim))).fold(0.0f32, f32::max);
        (d > 0.0).then_some(d)
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{build, preview, Theme};

    fn eval(v: Value, t: f32) -> Vec<Op> {
        let p = build("illo", &v).unwrap_or_else(|e| panic!("{e}"));
        let ops = p.eval(t, 800.0, 600.0, 30.0, &Theme::default(), preview::host());
        preview::validate(&ops).unwrap();
        ops
    }

    #[test]
    fn gradient_interpolates_in_oklab() {
        let stops = vec![(0.0, Rgba([1.0, 0.0, 0.0, 1.0])), (1.0, Rgba([0.0, 0.0, 1.0, 1.0]))];
        let mid = grad_at(&stops, 0.5);
        // sRGB mixing gives a dark muddy purple (0.5, 0, 0.5); OKLab keeps it bright and blue-leaning.
        assert!(mid.0[0] > 0.5 && mid.0[2] > 0.6, "{mid:?}");
        assert_eq!(grad_at(&stops, 0.0), stops[0].1);
        let ops = eval(serde_json::json!({"shapes": [{"shape": "circle", "cx": 400, "cy": 300, "r": 120, "fill": {"stops": ["#ff0000", "#0000ff"]}}]}), 0.0);
        assert!(ops.iter().any(|o| matches!(o, Op::Fill { paint: Paint::Image { .. }, .. })));
    }

    #[test]
    fn shadow_tone_shifts_hue_and_darkens() {
        let c = Rgba([0.9, 0.6, 0.3, 1.0]);
        let s = shadow_tone(c, Rgba([0.2, 0.2, 0.5, 1.0]), 1.0);
        assert!(lab(s)[0] < lab(c)[0] * 0.8);
        let h = |c: Rgba| {
            let l = lab(c);
            l[2].atan2(l[1])
        };
        assert!((h(s) - h(c)).abs() > 0.1, "hue must rotate, not just darken");
        let r = rim_tone(c, Rgba([1.0, 0.95, 0.85, 1.0]), 1.0);
        assert!(lab(r)[0] > lab(c)[0]);
    }

    #[test]
    fn rim_sits_on_the_lit_side() {
        // Light from the left (180°): the rim copy is pushed right, so the sliver is on the left.
        let ops = eval(
            serde_json::json!({"light": {"angle": 180}, "hand": {"wobble": 0, "rough": 0}, "shapes": [{"shape": "rect", "x": 200, "y": 100, "w": 400, "h": 400, "fill": "#c06040", "shade": {"amount": 0.6, "rim": 0.05, "offset": 0.2}}]}),
            0.0,
        );
        let evenodd: Vec<kurbo::Rect> = ops
            .iter()
            .filter_map(|o| match o {
                Op::Fill { path, rule: FillRule::EvenOdd, .. } => {
                    // The second subpath is the shifted copy of the body.
                    let mut sub = Path::new();
                    let mut n = 0;
                    for el in path.elements() {
                        if matches!(el, PathEl::MoveTo(_)) {
                            n += 1;
                        }
                        if n == 2 {
                            sub.push(*el);
                        }
                    }
                    Some(sub.bounding_box())
                }
                _ => None,
            })
            .collect();
        assert_eq!(evenodd.len(), 2, "form shadow + rim");
        // Form shadow: copy moved towards the light (left) → shadow crescent on the right.
        assert!(evenodd[0].x0 < 200.0 - 50.0);
        // Rim: copy moved away from the light (right) → lit sliver on the left.
        assert!(evenodd[1].x0 > 200.0 && evenodd[1].x0 < 230.0);
    }

    #[test]
    fn clip_and_cut_make_layers() {
        let ops = eval(
            serde_json::json!({"shapes": [
                {"shape": "circle", "name": "head", "cx": 400, "cy": 300, "r": 150, "fill": "#e0a080"},
                {"shape": "ellipse", "cx": 470, "cy": 230, "rx": 80, "ry": 40, "fill": "#fff3e0", "clipTo": "head"},
                {"shape": "circle", "cx": 400, "cy": 300, "r": 60, "fill": "#333", "cut": 3},
                {"shape": "circle", "cx": 400, "cy": 300, "r": 30, "visible": false}
            ]}),
            0.0,
        );
        let clips: Vec<kurbo::Rect> = ops.iter().filter_map(|o| if let Op::PushLayer { clip: Some((p, _)), .. } = o { Some(p.bounding_box()) } else { None }).collect();
        assert!(clips.iter().any(|r| (r.width() - 300.0).abs() < 12.0), "clipped to the head: {clips:?}");
        assert!(ops.iter().any(|o| matches!(o, Op::PushLayer { blend: Blend::DestOut, .. })));
        assert!(build("illo", &serde_json::json!({"shapes": [{"shape": "circle", "r": 5, "clipTo": "nope"}]})).is_err());
    }

    #[test]
    fn outline_is_heavier_away_from_the_light() {
        let b = kurbo::Circle::new((0.0, 0.0), 100.0).to_path(0.05);
        // Light from the right.
        let p = outline_path(&b, [1.0, 8.0], [1.0, 0.0], 0.0, 0.0, 1);
        let bb = p.bounding_box();
        // Outer ring extends further on the left (shadow side) than on the right.
        assert!(-bb.x0 > bb.x1 + 2.0, "{bb:?}");
    }

    #[test]
    fn morph_steps_and_inks() {
        let v = serde_json::json!({
            "onTwos": true,
            "inks": ["#ff48b0", "#0078bf"],
            "shapes": [{"shape": "path", "d": "M0 0 L100 0 L100 100 L0 100 Z", "fill": "#ff0000",
                "animate": {"keys": [{"t": 0, "d": "M0 0 L100 0 L100 100 L0 100 Z"}, {"t": 1, "d": "M50 0 A50 50 0 1 1 49.9 0 Z", "squash": 0.8}]}}]
        });
        let a = eval(v.clone(), 0.50);
        let b = eval(v.clone(), 0.52);
        assert_eq!(a, b, "on twos: both times land on the same drawing");
        let c = eval(v, 0.6);
        assert_ne!(a, c);
        // The red fill snapped to the pink ink.
        assert!(a.iter().any(|o| matches!(o, Op::Fill { paint: Paint::Solid(c), .. } if (c.0[2] - 0xb0 as f32 / 255.0).abs() < 0.01)));
    }

    #[test]
    fn svg_import_paint_over_and_rig() {
        let svg = r##"<svg viewBox="0 0 200 100"><path id="hill" d="M0 100 L100 20 L200 100 Z" fill="#3a7d5c"/><circle cx="150" cy="30" r="12" style="fill:#f6b94c"/><line x1="0" y1="90" x2="200" y2="90" stroke="#222" stroke-width="3"/></svg>"##;
        let v = serde_json::json!({"svg": svg, "paintOver": true, "shapes": [{"shape": "circle", "cx": 50, "cy": 50, "r": 10, "fill": "#fff", "clipTo": "hill"}]});
        let ops = eval(v, 0.0);
        assert!(ops.iter().any(|o| matches!(o, Op::Stroke { .. })), "hatching");
        assert!(build("illo", &serde_json::json!({"svg": "<svg></svg>"})).is_err());
        let rig = serde_json::json!({
            "groups": [{"name": "body", "pivot": [100, 200], "cycle": "bob"}, {"name": "arm", "parent": "body", "pivot": [100, 120], "cycle": {"style": "walk", "phase": 0.5}, "lag": 0.06}],
            "shapes": [{"shape": "rect", "x": 80, "y": 100, "w": 40, "h": 100, "group": "body"}, {"shape": "rect", "x": 95, "y": 120, "w": 10, "h": 60, "group": "arm"}]
        });
        assert_ne!(eval(rig.clone(), 0.0), eval(rig.clone(), 0.3));
        assert!(build("illo", &serde_json::json!({"groups": [{"name": "a", "cycle": "jog"}]})).is_err());
    }

    #[test]
    fn deterministic_with_boil() {
        let v = serde_json::json!({"hand": {"boil": 8, "outline": true}, "grain": 0.06, "texture": "halftone", "shapes": [{"shape": "blob", "cx": 300, "cy": 300, "r": 120, "fill": "$1", "shade": true, "shadow": {"flatten": 0.3}}], "palette": ["#3a7d5c"]});
        assert_eq!(eval(v.clone(), 1.0), eval(v.clone(), 1.0));
        assert_ne!(eval(v.clone(), 1.0), eval(v, 1.2));
    }
}
