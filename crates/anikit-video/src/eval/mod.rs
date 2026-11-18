//! Evaluation: spec at time `t` → [`DisplayList`].
//!
//! [`Evaluator::new`] does all the work that doesn't depend on time once:
//! themes, units, text layout, box layout, presets, stagger and expressions
//! are compiled into a tree of [`Node`]s, each with a small fixed array of
//! base property values and per-property keyframe tracks. [`Evaluator::eval_into`]
//! then walks the tree and only interpolates.

pub mod assets;
pub mod beat_markers;
mod compile;
/// Shader layers and transitions (anikit-shader).
pub(crate) mod shader;
pub(crate) mod fx;
pub(crate) mod graphic;
mod audio_react;
pub mod expr;
pub mod theme;

use crate::dl::{self, Affine, Blend, DisplayList, Filter, Glyph, ImageId, Op, Paint, Path, Rgba};
use crate::text::{kinetic::KineticPlan, TextLayout, TextSpec, UnitKind};
pub use assets::SharedAssets;
pub use shader::{set_preview_scale, ShaderStat};
use crate::spec::Diagnostic;
use anikit_core::easing::Easing;
use expr::{Expr, Vars};
use std::sync::Arc;

pub use compile::Options;

/// Animatable property slots.
pub(crate) mod prop {
    pub const X: usize = 0;
    pub const Y: usize = 1;
    pub const W: usize = 2;
    pub const H: usize = 3;
    pub const SX: usize = 4;
    pub const SY: usize = 5;
    pub const ROT: usize = 6;
    pub const SKX: usize = 7;
    pub const SKY: usize = 8;
    pub const OPACITY: usize = 9;
    pub const BLUR: usize = 10;
    pub const RADIUS: usize = 11;
    pub const SW: usize = 12;
    pub const TRIM0: usize = 13;
    pub const TRIM1: usize = 14;
    pub const REVEAL: usize = 15;
    pub const VALUE: usize = 16;
    pub const SIZE: usize = 17;
    pub const LS: usize = 18;
    pub const PROGRESS: usize = 19;
    pub const ZOOM: usize = 20;
    pub const WIPE: usize = 21;
    /// Fill colour (OKLab + alpha), 4 slots.
    pub const FILL: usize = 22;
    /// Stroke colour (OKLab + alpha), 4 slots.
    pub const STROKE: usize = 26;
    /// Layer depth (parallax and depth of field); camera focus plane and aperture.
    pub const DEPTH: usize = 30;
    pub const FOCUS: usize = 31;
    pub const APERTURE: usize = 32;
    /// Text-on-a-path shift (px) and stroke dash offset (px).
    pub const PATH_OFF: usize = 33;
    pub const DASH_OFF: usize = 34;
    /// Text animator range selector: start, end, offset (fractions of the units).
    pub const RANGE0: usize = 35;
    pub const RANGE1: usize = 36;
    pub const RANGE_OFF: usize = 37;
    /// Shader uniforms: `UNIFORM_SLOTS` floats, mapped per layer by name.
    pub const U0: usize = 38;
    pub const N: usize = U0 + super::shader::UNIFORM_SLOTS;

    /// Default values.
    pub const DEFAULTS: [f32; N] = {
        let mut d = [0.0; N];
        d[SX] = 1.0;
        d[SY] = 1.0;
        d[OPACITY] = 1.0;
        d[TRIM1] = 1.0;
        d[REVEAL] = 1.0;
        d[ZOOM] = 1.0;
        d[WIPE] = 1.0;
        d[FILL + 3] = 1.0;
        d[STROKE + 3] = 1.0;
        d[RANGE1] = 1.0;
        d
    };

    /// Name → (slot, channels).
    pub fn lookup(name: &str) -> Option<(usize, usize)> {
        Some(match name {
            "x" => (X, 1),
            "y" => (Y, 1),
            "width" => (W, 1),
            "height" => (H, 1),
            "scale" => (SX, 2),
            "scaleX" => (SX, 1),
            "scaleY" => (SY, 1),
            "rotation" => (ROT, 1),
            "skewX" => (SKX, 1),
            "skewY" => (SKY, 1),
            "opacity" => (OPACITY, 1),
            "blur" => (BLUR, 1),
            "radius" => (RADIUS, 1),
            "strokeWidth" => (SW, 1),
            "trimStart" => (TRIM0, 1),
            "trimEnd" => (TRIM1, 1),
            "reveal" => (REVEAL, 1),
            "value" => (VALUE, 1),
            "size" => (SIZE, 1),
            "letterSpacing" => (LS, 1),
            "progress" => (PROGRESS, 1),
            "zoom" => (ZOOM, 1),
            "wipe" => (WIPE, 1),
            "depth" => (DEPTH, 1),
            "focus" => (FOCUS, 1),
            "aperture" => (APERTURE, 1),
            "pathOffset" => (PATH_OFF, 1),
            "dashOffset" => (DASH_OFF, 1),
            "rangeStart" => (RANGE0, 1),
            "rangeEnd" => (RANGE1, 1),
            "rangeOffset" => (RANGE_OFF, 1),
            "fill" | "color" => (FILL, 4),
            "stroke" => (STROKE, 4),
            _ => return None,
        })
    }
}

/// Text whose layout depends on animated values (variable axes in uniform
/// slots, letter spacing in `prop::LS`).
pub(crate) struct DynText {
    pub spec: crate::text::TextSpec,
    /// Axis tag and its slot (`prop::U0 + i`).
    pub axes: Vec<(String, usize)>,
    /// Base letter spacing in px (`prop::LS`).
    pub ls: f32,
}

impl DynText {
    /// The layout for the current values, or `None` when they match the base.
    fn relayout(&self, fonts: &crate::text::FontStore, v: &[f32; prop::N]) -> Option<Arc<TextLayout>> {
        let q = |x: f32| (x * 10.0).round() / 10.0;
        let ls_changed = (v[prop::LS] - self.ls).abs() > 0.01;
        let ax_changed = self.axes.iter().any(|(t, slot)| self.spec.style.axes.iter().any(|(t2, b)| t2 == t && (q(v[*slot]) - q(*b)).abs() > 1e-4));
        if !ls_changed && !ax_changed {
            return None;
        }
        let mut sp = self.spec.clone();
        for (t, slot) in &self.axes {
            if let Some(a) = sp.style.axes.iter_mut().find(|a| &a.0 == t) {
                a.1 = q(v[*slot]);
            }
        }
        if ls_changed {
            sp.style.letter_spacing = (v[prop::LS] * 20.0).round() / 20.0 / sp.style.size.max(1.0);
        }
        Some(fonts.layout(&sp))
    }
}

/// One interpolation segment of a property track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Seg {
    pub t0: f32,
    pub t1: f32,
    /// `NaN` = "the base value at that moment" (dynamic base).
    pub v0: f32,
    pub v1: f32,
    /// Index into `Evaluator::easings`.
    pub ease: u32,
    /// Hold `v0` before `t0` (a `from` tween that comes first).
    pub hold_before: bool,
}

/// All segments of one property slot, sorted by `t0`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Track {
    pub slot: u8,
    pub segs: Vec<Seg>,
}

impl Track {
    #[inline]
    fn sample(&self, t: f32, base: f32, easings: &[Easing]) -> f32 {
        let s = &self.segs;
        if s.is_empty() {
            return base;
        }
        if t < s[0].t0 {
            return if s[0].hold_before { pick(s[0].v0, base) } else { base };
        }
        let mut k = 0;
        for (i, g) in s.iter().enumerate() {
            if g.t0 <= t {
                k = i;
            } else {
                break;
            }
        }
        let g = &s[k];
        let (v0, v1) = (pick(g.v0, base), pick(g.v1, base));
        if t >= g.t1 || g.t1 <= g.t0 {
            return v1;
        }
        let u = (t - g.t0) / (g.t1 - g.t0);
        let e = easings[g.ease as usize].ease(u as f64) as f32;
        v0 + (v1 - v0) * e
    }
    /// True if no segment is animating within `[a, b]`.
    fn still(&self, a: f32, b: f32) -> bool {
        self.segs.iter().all(|g| g.t1 <= a || g.t0 >= b || (g.v0 == g.v1 && !g.v0.is_nan()))
    }
}

#[inline]
fn pick(v: f32, base: f32) -> f32 {
    if v.is_nan() {
        base
    } else {
        v
    }
}

/// Compiled paint (resolved against the box size at emit time).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PaintC {
    /// Solid colour; animatable through the FILL / STROKE slots.
    Solid,
    /// Linear gradient by CSS angle (deg), stops.
    Linear { angle: f32, stops: Vec<(f32, Rgba)> },
    /// Radial gradient: centre (fraction of box), radius (fraction of half-diagonal), stops.
    Radial { c: [f32; 2], r: f32, stops: Vec<(f32, Rgba)> },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StrokeC {
    pub paint: PaintC,
    pub cap: u8,
    pub join: u8,
    pub dash: Vec<f32>,
    pub dash_offset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShapeKind {
    Rect,
    Ellipse,
    /// A fixed local path (line, arrow, polygon, star, path).
    Path,
}

/// A repeating motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LoopC {
    Float { amount: f32, period: f32 },
    Pulse { amount: f32, period: f32 },
    Spin { period: f32 },
    Wiggle { amount: f32, period: f32 },
    Breathe { amount: f32, period: f32 },
    Blink { period: f32 },
}

/// Per-unit animation of `split` text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SplitC {
    pub by: UnitKind,
    /// Tracks in unit space (x/y offsets, scale, rotation, opacity, blur…).
    pub tracks: Vec<Track>,
    /// Delay per unit index.
    pub delays: Vec<f32>,
    /// Per-unit tracks on the layer's own clock (e.g. caption word highlight colour).
    pub unit_tracks: Vec<Vec<Track>>,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum Kind {
    Text {
        layout: Arc<TextLayout>,
        /// Wrap width the layout was made with (glyphs are positioned within it).
        wrap: Option<f32>,
        paint: PaintC,
        /// Use the runs' own colours (rich spans).
        run_colors: bool,
        split: Option<SplitC>,
        counter: Option<Counter>,
        kinetic: Option<KineticPlan>,
        /// Animated variable axes / tracking: re-laid out per frame.
        dynamic: Option<Box<DynText>>,
    },
    Shape {
        shape: ShapeKind,
        fill: Option<PaintC>,
        stroke: Option<StrokeC>,
        path: Path,
    },
    Image {
        image: ImageId,
        natural: [f32; 2],
        /// 0 contain, 1 cover, 2 fill, 3 none.
        fit: u8,
    },
    /// Decoded clip frames at `rate` frames per clip-second.
    Video {
        frames: Vec<ImageId>,
        natural: [f32; 2],
        fit: u8,
        rate: f32,
        speed: f32,
        looped: bool,
    },
    Group {
        children: Vec<Node>,
    },
    /// Raw drawing: primitives in the layer's box.
    Draw(Vec<DrawC>),
    /// Data points: positions grouped by colour, a size, an optional per-dot scale.
    Dots { groups: Vec<(Rgba, Vec<[f32; 2]>)>, size: f32, appear: Option<Expr>, square: bool },
    /// A nested 2D composition, drawn as vectors in its own clipped layer.
    Comp {
        ev: Arc<Evaluator>,
        natural: [f32; 2],
        fit: u8,
        time: CompTime,
    },
    Box {
        children: Vec<Node>,
        bg: Option<PaintC>,
        border: Option<(f32, Rgba)>,
    },
    Component(Box<dyn crate::components::Component>, Option<crate::components::Theme>),
    /// A JIT-compiled fragment shader (see `shader.rs`).
    Shader(Box<shader::ShaderC>),
}

pub(crate) struct Counter {
    pub prefix: String,
    pub suffix: String,
    pub decimals: usize,
    pub thousands: bool,
    /// Layout template (its text is replaced per frame).
    pub spec: TextSpec,
}

/// A compiled layer.
pub(crate) struct Node {
    pub kind: Kind,
    /// Visible window, seconds since the scene start.
    pub start: f32,
    pub end: f32,
    pub base: [f32; prop::N],
    pub exprs: Vec<(u8, Expr)>,
    /// Per-frame `update` script (compiled once).
    pub update: Option<Arc<rhai::AST>>,
    /// Per-frame colours (`"fill": "=hsl(t * 60, 70, 55)"`) for the 4-slot colour at `slot`.
    pub cexprs: Vec<(u8, expr::ColorExpr)>,
    pub tracks: Vec<Track>,
    pub loops: Vec<LoopC>,
    pub anchor: [f32; 2],
    pub blend: Blend,
    pub shadow: Option<Filter>,
    pub glow: Option<Filter>,
    /// Image treatments (tone map, halftone), applied first.
    pub treat: Vec<Filter>,
    /// Brush stroke: the outline drawn as a variable-width ribbon.
    pub brush: Option<Arc<graphic::Brush>>,
    /// A pattern whose `field` reads `t`: regenerated every frame (pattern, box size).
    pub live_pattern: Option<Arc<(graphic::Pat, [f64; 2])>>,
    /// First of 5 animatable slots for the treatments (toneAmount, halftone cell/angle/gain, ink amount); 0 = none.
    pub treat_slot: u8,
    /// Stepped time in fps (0 = smooth): the layer animates "on twos" etc.
    pub step: f32,
    /// Boil: `[px, degrees, fps, seed]` seeded jitter.
    pub boil: Option<[f32; 4]>,
    /// Clip children to the box (with radius).
    pub clip: bool,
    /// A clip path in the layer's local box (clip shapes, shape masks).
    pub clip_path: Option<Path>,
    /// Wipe direction (0 left→right reveal, 1 right→left, 2 top→bottom, 3 bottom→top).
    pub wipe_dir: u8,
    pub index: f32,
    pub count: f32,
    /// Index into `Evaluator::layer_info` (spec path and kind), for tagged evaluation.
    pub tag: u32,
    /// The spec `id` (transitions find layers by it).
    pub id: Option<Box<str>>,
    /// Motion-graphics extras (repeater, echo, motion path, text path, wiggle path).
    pub fx: Option<Box<fx::Fx>>,
    /// Track matte: the matte layer (a sibling, not drawn itself) and mode
    /// (0 alpha, 1 alphaInverted, 2 luma, 3 lumaInverted).
    pub matte: Option<Box<(Node, u8)>>,
    /// Inertia: a spring that follows the layer's own animated transform (follow-through, overshoot).
    pub inertia: Option<Box<fx::InertiaC>>,
    /// Posterize time: frames per second of this subtree's motion (0 = off).
    pub posterize_time: f32,
    /// Group drag: seconds of lag per child on the group's own transform.
    pub drag: f32,
}

pub(crate) const PART_BG: u8 = 1;
pub(crate) const PART_FG: u8 = 2;
pub(crate) const PART_ALL: u8 = 3;

/// Compiled transition into a scene.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransC {
    pub kind: TransKind,
    pub dur: f32,
    pub ease: u32,
    /// Unit vector the new scene moves along (slide/push/wipe).
    pub dir: [f32; 2],
    pub color: Rgba,
    pub origin: [f32; 2],
    pub amount: f32,
    /// `TransKind::Shader`: the compiled gl-transitions style shader.
    pub shader: Option<shader::ShaderRef>,
    /// `zoomThrough`: id of the outgoing scene's layer to fly into.
    pub layer: Option<String>,
    /// `reveal` shape (0 circle, 1 diamond, 2 square, 3 star, 4 blob); `blinds` bar count.
    pub shape: u8,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransKind {
    Cut,
    Crossfade,
    Dip,
    Slide,
    Push,
    Zoom,
    Wipe,
    Iris,
    Blur,
    Glitch,
    MorphCut,
    Shader,
    ZoomThrough,
    Reveal,
    Blinds,
    Split,
    Whip,
}

pub(crate) struct SceneC {
    pub id: String,
    /// Global start and duration.
    pub start: f32,
    pub dur: f32,
    pub bg: PaintC,
    pub bg_color: Rgba,
    pub trans: TransC,
    pub camera: Node,
    pub shake: Option<(f32, f32, f32, f32, f32)>,
    pub layers: Vec<Node>,
    /// Some layer or the camera uses depth (parallax / depth of field).
    pub depth: bool,
    /// How many leading layers are the backdrop (full-frame backgrounds).
    pub bg_layers: usize,
    /// Effective finishing look (the scene's or the video's).
    pub post: Option<crate::render::post::PostSettings>,
}

/// One layer's ops in a tagged evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpTag {
    pub ops: std::ops::Range<usize>,
    /// Index for [`Evaluator::layer_info`].
    pub layer: u32,
}

thread_local! {
    /// Composition display scales for this frame: `(collecting, scale per composition)`.
    static COMP_SCALE: std::cell::RefCell<Option<(bool, std::collections::HashMap<usize, f32>)>> = const { std::cell::RefCell::new(None) };
}

/// Whether this is the scale-collecting pass (shader layers emit nothing).
pub(crate) fn collecting_scales() -> bool {
    COMP_SCALE.with(|c| matches!(&*c.borrow(), Some((true, _))))
}

thread_local! {
    static TAGS: std::cell::RefCell<Option<Vec<OpTag>>> = const { std::cell::RefCell::new(None) };
}

/// Timing summary of one scene.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SceneInfo {
    pub id: String,
    pub start: f32,
    pub end: f32,
}

/// The compiled video.
pub struct Evaluator {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    /// Total duration in seconds.
    pub duration: f32,
    pub diagnostics: Vec<Diagnostic>,
    /// Audio tracks on the global timeline (paths resolved).
    pub audio: Vec<crate::audio::Track>,
    /// Generated music (the spec's `music`), rendered when the audio is mixed.
    pub music: Option<Arc<crate::music::MusicCue>>,
    /// Sound design (the spec's `sound` and layers' `sfx`), mixed with the audio.
    pub sound: Option<Arc<crate::audio::design::spec::SoundPlan>>,
    /// Fonts and images, shared with the renderers.
    pub assets: Arc<SharedAssets>,
    pub(crate) easings: Vec<Easing>,
    pub(crate) scenes: Vec<SceneC>,
    pub(crate) overlay: Vec<Node>,
    /// (spec path, kind) per compiled layer, indexed by `Node::tag`.
    pub(crate) layer_info: Vec<(String, String)>,
    /// Word-synced captions (TEXT's caption track) and the bottom-centre point they hang from.
    pub(crate) captions: Option<(crate::text::captions::CaptionTrack, [f32; 2])>,
    pub(crate) theme: theme::Theme,
    pub(crate) ctheme: crate::components::Theme,
    /// Beat grid and loudness of the music, when expressions read them.
    pub(crate) groove: Option<Arc<beat_markers::Music>>,
    /// Encoder adaptive-quantisation strength (`encode.aq`), when the spec sets it.
    pub encode_aq: Option<f32>,
    /// Encoder quantiser (`encode.crf` or `render --crf`), when set.
    pub encode_crf: Option<u8>,
    /// Motion-blur shutter in seconds (0 = off). Default half a frame (180°).
    pub shutter: f32,
    /// The spec asked for motion blur itself (otherwise only `--quality high` applies it).
    pub shutter_explicit: bool,
    /// Named sub-compositions (top-level `compositions`), in dependency order.
    pub(crate) comps: Vec<(String, Arc<Evaluator>)>,
}

/// A gradient stroke segment: path, start and end points, start and end colours.
pub(crate) type DrawSeg = (Path, [f32; 2], [f32; 2], Rgba, Rgba);

/// One compiled `draw` primitive.
pub(crate) struct DrawC {
    pub path: Path,
    pub fill: Option<Rgba>,
    /// Stroke colour (none when per-vertex colours draw the stroke).
    pub stroke: Option<Rgba>,
    /// Stroke width, cap, join.
    pub style: (f32, u8, u8),
    /// Per-vertex colours: one gradient segment per pair of points.
    pub segs: Vec<DrawSeg>,
    pub opacity: f32,
}

/// How a layer's clock maps onto a composition's clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompTime {
    /// Composition time at the layer's start (`trimStart`).
    pub trim: f32,
    /// Playback rate (`speed`).
    pub speed: f32,
    /// Wrap at the end instead of holding the last frame.
    pub looped: bool,
}

impl Default for CompTime {
    fn default() -> Self {
        CompTime { trim: 0.0, speed: 1.0, looped: false }
    }
}

impl CompTime {
    /// Composition time for layer-local time `lt`, for a composition lasting `dur` seconds.
    pub fn map(&self, lt: f32, dur: f32) -> f32 {
        let t = self.trim + lt * self.speed;
        if self.looped && dur > 0.0 {
            t.rem_euclid(dur)
        } else {
            // Hold the last frame (layers end exclusively at the duration).
            t.clamp(0.0, (dur - 5e-4).max(0.0))
        }
    }
}

/// Errors from building an evaluator: all diagnostics, at least one an error.
#[derive(Debug)]
pub struct SpecErrors(pub Vec<Diagnostic>);

impl std::fmt::Display for SpecErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for d in &self.0 {
            writeln!(f, "{d}")?;
        }
        Ok(())
    }
}
impl std::error::Error for SpecErrors {}

#[derive(Clone, Copy)]
struct Cx<'a> {
    ev: &'a Evaluator,
    scene_t: f32,
    gt: f32,
    /// Depth-of-field blur added to the node being emitted (top-level layers only).
    dof: f32,
    /// Opacity multiplier for the node being emitted (repeater / echo copies).
    amul: f32,
}

impl Evaluator {
    /// Number of frames.
    pub fn frames(&self) -> u32 {
        (self.duration * self.fps).round().max(1.0) as u32
    }

    /// The resolved theme.
    pub fn theme(&self) -> &theme::Theme {
        &self.theme
    }

    /// Scene timing.
    pub fn scenes(&self) -> Vec<SceneInfo> {
        self.scenes.iter().map(|s| SceneInfo { id: s.id.clone(), start: s.start, end: s.start + s.dur }).collect()
    }

    /// Start time of a scene by id.
    pub fn scene_start(&self, id: &str) -> Option<f32> {
        self.scenes.iter().find(|s| s.id == id).map(|s| s.start)
    }

    /// True if every frame in `[t0, t1]` is guaranteed identical (no scene
    /// change, transition, visibility change or animation in that span).
    /// Conservative: `false` means "may change".
    pub fn is_static(&self, t0: f32, t1: f32) -> bool {
        let (t0, t1) = (t0.max(0.0), t1.min(self.duration));
        if t1 < t0 {
            return true;
        }
        // Scene and transition boundaries.
        for (i, s) in self.scenes.iter().enumerate() {
            let edges = [s.start, s.start + s.trans.dur, s.start + s.dur];
            if i > 0 && edges.iter().any(|e| *e > t0 && *e <= t1) {
                return false;
            }
            if i > 0 && s.trans.kind != TransKind::Cut && t0 >= s.start && t0 < s.start + s.trans.dur {
                return false;
            }
        }
        let cur = self.scenes.iter().rposition(|s| s.start <= t0).unwrap_or(0);
        let s = &self.scenes[cur];
        let (a, b) = (t0 - s.start, t1 - s.start);
        if s.shake.is_some_and(|(amount, _, at, dur, _)| amount != 0.0 && (dur <= 0.0 || (at < b && at + dur > a))) {
            return false;
        }
        node_static(&s.camera, a, b) && s.layers.iter().all(|n| node_static(n, a, b)) && self.overlay.iter().all(|n| node_static(n, t0, t1))
            && self.captions.as_ref().is_none_or(|(tr, _)| t1 - t0 < 0.05 && tr.page_text(t0).is_none() && tr.page_text(t1).is_none())
    }

    /// The display list at `t` plus, for every layer that drew something, the op range
    /// it emitted (ranges nest: a group's range contains its children's), so tools like
    /// critique can tell user text from component text. `layer == u32::MAX` is captions.
    pub fn eval_tagged(&self, t: f32) -> (DisplayList, Vec<OpTag>) {
        TAGS.with(|tg| *tg.borrow_mut() = Some(Vec::new()));
        let dl = self.eval(t);
        let tags = TAGS.with(|tg| tg.borrow_mut().take()).unwrap_or_default();
        (dl, tags)
    }

    /// Spec path and kind of a tagged layer (`("scenes[1].layers[3]", "component:browser")`);
    /// `("captions", "caption")` for `u32::MAX`.
    /// Whether the video has layers whose frames are expensive (shaders, 3D, video):
    /// the ones worth estimating before a render.
    pub fn has_heavy_layers(&self) -> bool {
        const HEAVY: &[&str] = &["shader", "video"];
        const HEAVY_COMPONENTS: &[&str] = &["scene3d", "scene-3d", "model", "product-shot", "text-3d", "globe", "lottie"];
        self.layer_info.iter().any(|(_, k)| HEAVY.contains(&k.as_str()) || k.strip_prefix("component:").is_some_and(|c| HEAVY_COMPONENTS.contains(&c)))
            || self.comps.iter().any(|(_, c)| c.has_heavy_layers())
    }

    pub fn layer_info(&self, tag: u32) -> (&str, &str) {
        match self.layer_info.get(tag as usize) {
            Some((p, k)) => (p, k),
            None => ("captions", "caption"),
        }
    }

    /// Collisions reported by component layers (physics in `scene3d`), as
    /// `(layer tag, video seconds, impact)`; `aspect(tag)` is the layer box's
    /// width / height on screen.
    pub(crate) fn component_impacts(&self, aspect: &dyn Fn(u32) -> f32) -> Vec<(u32, f32, crate::components::Impact)> {
        fn walk(nodes: &[Node], t0: f32, until: f32, aspect: &dyn Fn(u32) -> f32, out: &mut Vec<(u32, f32, crate::components::Impact)>) {
            for n in nodes {
                let start = t0 + n.start;
                match &n.kind {
                    Kind::Component(c, _) => {
                        let local = (until.min(t0 + n.end) - start).max(0.0);
                        for i in c.impacts(local, aspect(n.tag)) {
                            out.push((n.tag, start + i.t, i));
                        }
                    }
                    Kind::Group { children } | Kind::Box { children, .. } => walk(children, start, until, aspect, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for s in &self.scenes {
            walk(&s.layers, s.start, s.start + s.dur, aspect, &mut out);
        }
        out
    }

    /// The display list at time `t` (seconds).
    pub fn eval(&self, t: f32) -> DisplayList {
        let mut dl = DisplayList::default();
        self.eval_into(t, &mut dl);
        dl
    }

    /// The display list at time `t`, reusing `out`'s allocations.
    pub fn eval_into(&self, t: f32, out: &mut DisplayList) {
        out.width = self.width;
        out.height = self.height;
        recycle(&mut out.ops);
        // With compositions: a first pass (no shader jobs) finds the largest
        // scale each is shown at, so its pixel content renders once, at that size.
        let top = !self.comps.is_empty() && COMP_SCALE.with(|c| c.borrow().is_none());
        if top {
            shader::reset_sizes();
            COMP_SCALE.with(|c| *c.borrow_mut() = Some((true, std::collections::HashMap::new())));
            // Tags belong to the real pass only.
            let tags = TAGS.with(|tg| tg.borrow_mut().take());
            let mut scratch = Vec::new();
            self.emit_frame(t, false, &mut scratch);
            TAGS.with(|tg| *tg.borrow_mut() = tags);
            COMP_SCALE.with(|c| {
                if let Some(st) = &mut *c.borrow_mut() {
                    st.0 = false;
                }
            });
        }
        out.background = self.emit_frame(t, false, &mut out.ops);
        if top {
            COMP_SCALE.with(|c| *c.borrow_mut() = None);
            shader::reset_sizes();
        }
    }

    /// Append the frame at `t` to `out` and return its background colour. With `bg_rect`
    /// a solid background is drawn as a rectangle too (nested compositions).
    fn emit_frame(&self, t: f32, bg_rect: bool, out: &mut Vec<Op>) -> Rgba {
        let t = t.clamp(0.0, self.duration.max(0.0));
        // Active scenes: the one at t, plus the previous one while transitioning in.
        let mut cur = 0;
        for (i, s) in self.scenes.iter().enumerate() {
            if s.start <= t {
                cur = i;
            }
        }
        let s = &self.scenes[cur];
        let local = t - s.start;
        let in_trans = cur > 0 && s.trans.kind != TransKind::Cut && local < s.trans.dur && s.trans.dur > 0.0;
        let black = Rgba([0.0, 0.0, 0.0, 1.0]);
        if !bg_rect {
            // Filters composite "below" onto the frame's background (shader.rs).
            shader::set_frame_bg(if !in_trans && matches!(s.bg, PaintC::Solid) { s.bg_color } else { black });
        }
        let background = if !in_trans {
            let solid = matches!(s.bg, PaintC::Solid);
            self.emit_scene(s, t, &dl::IDENTITY, solid && !(bg_rect && s.bg_color.0[3] > 0.0), out);
            if solid { s.bg_color } else { black }
        } else {
            let prev = &self.scenes[cur - 1];
            let p = (local / s.trans.dur).clamp(0.0, 1.0);
            let e = self.easings[s.trans.ease as usize].ease(p as f64) as f32;
            self.emit_transition(prev, s, t, p, e, out);
            black
        };
        let cx = Cx { ev: self, scene_t: t, gt: t, dof: 0.0, amul: 1.0 };
        let frame = [self.width as f32, self.height as f32];
        for n in &self.overlay {
            self.emit_node(n, &cx, &dl::IDENTITY, frame, 1.0, out);
        }
        if let Some((tr, [x, y])) = &self.captions {
            let start = out.len();
            tr.ops(t, translate(*x, *y), out);
            if out.len() > start {
                TAGS.with(|tg| {
                    if let Some(v) = tg.borrow_mut().as_mut() {
                        v.push(OpTag { ops: start..out.len(), layer: u32::MAX });
                    }
                });
            }
        }
        background
    }

    /// The finishing look at `t` (the incoming scene's during a transition), or `None`.
    pub fn post_at(&self, t: f32) -> Option<&crate::render::post::PostSettings> {
        let i = self.scenes.iter().rposition(|s| s.start <= t).unwrap_or(0);
        self.scenes.get(i).and_then(|s| s.post.as_ref())
    }

    /// Whether frame time `t` needs motion blur: a shutter is set and something moves while it is open.
    /// Pass `default_on` for renders where motion blur is on unless the spec says otherwise.
    pub fn needs_blur(&self, t: f32, default_on: bool) -> bool {
        (default_on || self.shutter_explicit) && self.shutter > 0.0 && !self.is_static(t - self.shutter * 0.5, t + self.shutter * 0.5)
    }

    /// Named sub-composition (top-level `compositions`). Evaluate it like any video:
    /// `eval_into(t, &mut dl)` gives its frame at composition time `t` (map a layer's clock
    /// with [`CompTime::map`]); `is_static` says when a cached texture stays valid.
    pub fn composition(&self, name: &str) -> Option<&Arc<Evaluator>> {
        self.comps.iter().find(|c| c.0 == name).map(|c| &c.1)
    }

    /// All sub-compositions by name, in dependency order.
    pub fn compositions(&self) -> impl Iterator<Item = (&str, &Arc<Evaluator>)> {
        self.comps.iter().map(|(n, e)| (n.as_str(), e))
    }

    fn emit_scene(&self, s: &SceneC, t: f32, xf: &Affine, bg_done: bool, out: &mut Vec<Op>) {
        self.emit_scene_part(s, t, xf, bg_done, PART_ALL, out)
    }

    /// Emit the backdrop (`PART_BG`: scene background + leading full-frame layers), the
    /// foreground (`PART_FG`) or both.
    fn emit_scene_part(&self, s: &SceneC, t: f32, xf: &Affine, bg_done: bool, part: u8, out: &mut Vec<Op>) {
        let (w, h) = (self.width as f32, self.height as f32);
        if part & PART_BG != 0 && (!bg_done || !matches!(s.bg, PaintC::Solid)) {
            let paint = self.paint(&s.bg, &solid_slots(s.bg_color), prop::FILL, [w, h], 1.0);
            out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: 0.0, paint, transform: *xf });
        }
        let lt = t - s.start;
        let cx = Cx { ev: self, scene_t: lt, gt: t, dof: 0.0, amul: 1.0 };
        let (cam, v, panning) = self.scene_cam(s, t, xf);
        let (cxp, cyp, rot, zoom) = (v[0], v[1], v[2], v[3]);
        let (focus, aperture) = (v[4], v[5]);
        for (li, n) in s.layers.iter().enumerate() {
            let is_bg = li < s.bg_layers;
            if (is_bg && part & PART_BG == 0) || (!is_bg && part & PART_FG == 0) {
                continue;
            }
            if !s.depth {
                self.emit_node(n, &cx, &cam, [w, h], 1.0, out);
                continue;
            }
            // Parallax and depth of field from the layer's depth.
            let mut lv = [0.0f32; prop::N];
            let lt = (lt - n.start).max(0.0);
            self.props(n, &cx, lt, &mut lv);
            let d = lv[prop::DEPTH];
            let lcam = if panning && d != 0.0 {
                let k = 1.0 / (1.0 + d.max(-0.9));
                let m = dl::mul(&translate(w * 0.5, h * 0.5), &dl::mul(&rotate(rot), &dl::mul(&scale(zoom, zoom), &translate(-w * 0.5 - cxp * k, -h * 0.5 - cyp * k))));
                dl::mul(xf, &m)
            } else {
                cam
            };
            let dcx = Cx { dof: (d - focus).abs() * aperture, ..Cx { ev: self, scene_t: cx.scene_t, gt: cx.gt, dof: 0.0, amul: 1.0 } };
            self.emit_node(n, &dcx, &lcam, [w, h], 1.0, out);
        }
    }

    /// A scene's camera transform at global time `t` (after `xf`), with
    /// `[x, y, rotation, zoom, focus, aperture]` and whether it pans.
    fn scene_cam(&self, s: &SceneC, t: f32, xf: &Affine) -> (Affine, [f32; 6], bool) {
        let (w, h) = (self.width as f32, self.height as f32);
        let lt = t - s.start;
        let cx = Cx { ev: self, scene_t: lt, gt: t, dof: 0.0, amul: 1.0 };
        // Camera.
        let mut v = [0.0f32; prop::N];
        self.props(&s.camera, &cx, lt, &mut v);
        let (mut cxp, mut cyp, mut rot) = (v[prop::X], v[prop::Y], v[prop::ROT]);
        if let Some((amount, freq, at, dur, rdeg)) = s.shake {
            let k = if dur > 0.0 {
                if lt < at || lt > at + dur {
                    0.0
                } else {
                    1.0 - (lt - at) / dur
                }
            } else {
                1.0
            };
            if k > 0.0 {
                let f = lt as f64 * freq as f64;
                cxp += amount * k * noise1(f) as f32;
                cyp += amount * k * noise1(f + 17.3) as f32;
                rot += rdeg * k * noise1(f + 41.1) as f32;
            }
        }
        let zoom = v[prop::ZOOM];
        let cam = if cxp == 0.0 && cyp == 0.0 && rot == 0.0 && zoom == 1.0 {
            *xf
        } else {
            let m = dl::mul(&translate(w * 0.5, h * 0.5), &dl::mul(&rotate(rot), &dl::mul(&scale(zoom, zoom), &translate(-w * 0.5 - cxp, -h * 0.5 - cyp))));
            dl::mul(xf, &m)
        };
        let (focus, aperture) = (v[prop::FOCUS], v[prop::APERTURE].max(0.0));
        let panning = cxp != 0.0 || cyp != 0.0;
        (cam, [cxp, cyp, rot, zoom, focus, aperture], panning)
    }

    fn emit_transition(&self, a: &SceneC, b: &SceneC, t: f32, p: f32, e: f32, out: &mut Vec<Op>) {
        let (w, h) = (self.width as f32, self.height as f32);
        let tr = &b.trans;
        let id = dl::IDENTITY;
        let layer = |out: &mut Vec<Op>, opacity: f32, filters: Vec<Filter>, clip: Option<(Path, Affine)>| {
            out.push(Op::PushLayer { opacity, blend: Blend::Normal, filters, clip, cache_key: None });
        };
        match tr.kind {
            TransKind::Cut => self.emit_scene(b, t, &id, false, out),
            TransKind::Shader => {
                let (mut fa, mut fb) = (Vec::new(), Vec::new());
                self.emit_scene(a, t, &id, false, &mut fa);
                self.emit_scene(b, t, &id, false, &mut fb);
                match &tr.shader {
                    Some(sc) => self.emit_shader_transition(&sc.0, (fa, a.bg_color), (fb, b.bg_color), t, e, out),
                    None => out.extend(fb),
                }
            }
            TransKind::Crossfade => {
                self.emit_scene(a, t, &id, false, out);
                layer(out, e, vec![], None);
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PopLayer);
            }
            TransKind::Dip => {
                let (sc, k) = if e < 0.5 { (a, e * 2.0) } else { (b, (1.0 - e) * 2.0) };
                self.emit_scene(sc, t, &id, false, out);
                let mut c = tr.color;
                c.0[3] *= k;
                out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: 0.0, paint: Paint::Solid(c), transform: id });
            }
            // Moving transitions: backdrops cross-fade in place (moving them would expose
            // their edges); only the foregrounds move.
            TransKind::Slide | TransKind::Push | TransKind::Zoom | TransKind::MorphCut => {
                self.emit_scene_part(a, t, &id, false, PART_BG, out);
                layer(out, e, vec![], None);
                self.emit_scene_part(b, t, &id, false, PART_BG, out);
                out.push(Op::PopLayer);
                let off = 1.0 - e;
                let into = translate(tr.dir[0] * -w * off, tr.dir[1] * -h * off);
                match tr.kind {
                    TransKind::Slide => {
                        self.emit_scene_part(a, t, &id, false, PART_FG, out);
                        self.emit_scene_part(b, t, &into, false, PART_FG, out);
                    }
                    TransKind::Push => {
                        self.emit_scene_part(a, t, &translate(tr.dir[0] * w * e, tr.dir[1] * h * e), false, PART_FG, out);
                        self.emit_scene_part(b, t, &into, false, PART_FG, out);
                    }
                    TransKind::Zoom => {
                        layer(out, e, vec![], None);
                        self.emit_scene_part(b, t, &about(w, h, 0.85 + 0.15 * e), false, PART_FG, out);
                        out.push(Op::PopLayer);
                        layer(out, 1.0 - e, vec![], None);
                        self.emit_scene_part(a, t, &about(w, h, 1.0 + 0.4 * e), false, PART_FG, out);
                        out.push(Op::PopLayer);
                    }
                    _ => {
                        layer(out, 1.0 - e, vec![], None);
                        self.emit_scene_part(a, t, &about(w, h, 1.0 - 0.02 * e), false, PART_FG, out);
                        out.push(Op::PopLayer);
                        layer(out, e, vec![], None);
                        self.emit_scene_part(b, t, &about(w, h, 1.02 - 0.02 * e), false, PART_FG, out);
                        out.push(Op::PopLayer);
                    }
                }
            }
            TransKind::Wipe => {
                self.emit_scene(a, t, &id, false, out);
                // The revealed region grows from the edge the new scene comes from.
                let r = match (tr.dir[0] as i32, tr.dir[1] as i32) {
                    (-1, _) => [w * (1.0 - e), 0.0, w, h],
                    (1, _) => [0.0, 0.0, w * e, h],
                    (_, -1) => [0.0, h * (1.0 - e), w, h],
                    _ => [0.0, 0.0, w, h * e],
                };
                let clip = kurbo::Rect::new(r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64);
                layer(out, 1.0, vec![], Some((shape_path(&clip, 0.1), id)));
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PopLayer);
            }
            TransKind::Iris => {
                self.emit_scene(a, t, &id, false, out);
                let (ox, oy) = (tr.origin[0], tr.origin[1]);
                let far = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
                    .iter()
                    .map(|(x, y)| ((x - ox) * (x - ox) + (y - oy) * (y - oy)).sqrt())
                    .fold(0.0, f32::max);
                let c = kurbo::Circle::new((ox as f64, oy as f64), (far * e).max(0.01) as f64);
                layer(out, 1.0, vec![], Some((shape_path(&c, 0.1), id)));
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PopLayer);
            }
            TransKind::Blur => {
                layer(out, 1.0, vec![Filter::Blur(tr.amount * e)], None);
                self.emit_scene(a, t, &id, false, out);
                out.push(Op::PopLayer);
                layer(out, e, vec![Filter::Blur(tr.amount * (1.0 - e))], None);
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PopLayer);
            }
            TransKind::ZoomThrough => {
                // Fly into a layer of the outgoing scene: the whole scene scales up about that
                // layer, which drifts to the frame centre, and the new scene shows through its shape.
                let found = tr.layer.as_deref().and_then(|id| a.layers.iter().find(|n| n.id.as_deref() == Some(id)));
                let Some(ln) = found else {
                    self.emit_scene(b, t, &id, false, out);
                    return;
                };
                let (cam, _, _) = self.scene_cam(a, t, &id);
                let acx = Cx { ev: self, scene_t: t - a.start, gt: t, dof: 0.0, amul: 1.0 };
                let mut v = [0.0f32; prop::N];
                self.props(ln, &acx, (t - a.start - ln.start).max(0.0), &mut v);
                let (lw, lh) = (v[prop::W].max(1.0), v[prop::H].max(1.0));
                let f = dl::mul(&cam, &local_xf(&v, ln.anchor, lw, lh));
                let c = [f[0] * lw * 0.5 + f[2] * lh * 0.5 + f[4], f[1] * lw * 0.5 + f[3] * lh * 0.5 + f[5]];
                let s0 = (f[0] * f[3] - f[1] * f[2]).abs().sqrt() * lw.min(lh);
                let zmax = (6.0 * (w * w + h * h).sqrt() / s0.max(1.0)).max(2.0);
                let z = zmax.powf(e);
                let to = [c[0] + (w * 0.5 - c[0]) * e, c[1] + (h * 0.5 - c[1]) * e];
                let m = dl::mul(&translate(to[0], to[1]), &dl::mul(&scale(z, z), &translate(-c[0], -c[1])));
                self.emit_scene(a, t, &m, false, out);
                // The new scene, fading up inside the layer's shape over the first third.
                layer(out, (e / 0.3).min(1.0), vec![], None);
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PushLayer { opacity: 1.0, blend: Blend::DestIn, filters: Vec::new(), clip: None, cache_key: None });
                let mb = out.len();
                self.emit_node(ln, &acx, &dl::mul(&m, &cam), [w, h], 1.0, out);
                if out.len() == mb {
                    out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: 0.0, paint: Paint::Solid(Rgba([0.0, 0.0, 0.0, 1.0])), transform: id });
                }
                out.push(Op::PopLayer);
                out.push(Op::PopLayer);
            }
            TransKind::Reveal => {
                self.emit_scene(a, t, &id, false, out);
                let (ox, oy) = (tr.origin[0], tr.origin[1]);
                let far = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)].iter().map(|(x, y)| ((x - ox).powi(2) + (y - oy).powi(2)).sqrt()).fold(0.0, f32::max);
                // Radius that covers the frame for each shape (its inscribed circle must reach `far`).
                let k = [1.0, 1.42, 1.42, 2.7, 1.25][tr.shape as usize];
                let r = (far * k * e).max(0.01) as f64;
                let spin = (tr.amount * e).to_radians() as f64;
                let pts: Vec<(f64, f64)> = match tr.shape {
                    1 => (0..4).map(|i| (i as f64 * std::f64::consts::FRAC_PI_2, r)).collect(),
                    2 => (0..4).map(|i| (std::f64::consts::FRAC_PI_4 + i as f64 * std::f64::consts::FRAC_PI_2, r)).collect(),
                    3 => (0..10).map(|i| (i as f64 * std::f64::consts::PI / 5.0 - std::f64::consts::FRAC_PI_2, if i % 2 == 0 { r } else { r * 0.5 })).collect(),
                    4 => (0..36)
                        .map(|i| {
                            let a = i as f64 * std::f64::consts::TAU / 36.0;
                            (a, r * (1.0 + 0.12 * noise1(i as f64 * 0.7 + e as f64 * 3.0) + 0.06 * (3.0 * a + e as f64 * 4.0).sin()))
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let path = if pts.is_empty() {
                    shape_path(&kurbo::Circle::new((ox as f64, oy as f64), r), 0.1)
                } else {
                    let mut p = Path::new();
                    for (i, (a, rr)) in pts.iter().enumerate() {
                        let q = (ox as f64 + rr * (a + spin).cos(), oy as f64 + rr * (a + spin).sin());
                        if i == 0 {
                            p.move_to(q)
                        } else {
                            p.line_to(q)
                        }
                    }
                    p.close_path();
                    p
                };
                layer(out, 1.0, vec![], Some((path, id)));
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PopLayer);
            }
            TransKind::Blinds => {
                self.emit_scene(a, t, &id, false, out);
                // Bars across the travel direction, each opening a little after the previous one.
                let n = tr.count.max(1) as usize;
                let horiz = tr.dir[0] != 0.0;
                let span = if horiz { w } else { h };
                let bar = span / n as f32;
                let spread = 0.5;
                let mut p = Path::new();
                for i in 0..n {
                    let k = if tr.dir[0] + tr.dir[1] < 0.0 { n - 1 - i } else { i };
                    let d = k as f32 / n as f32 * spread;
                    let f = ((e * (1.0 + spread) - d) / 1.0).clamp(0.0, 1.0);
                    if f <= 0.0 {
                        continue;
                    }
                    let a0 = i as f32 * bar;
                    let r = if horiz { kurbo::Rect::new(a0 as f64, 0.0, (a0 + bar * f + 0.5) as f64, h as f64) } else { kurbo::Rect::new(0.0, a0 as f64, w as f64, (a0 + bar * f + 0.5) as f64) };
                    p.extend(shape_path(&r, 0.1));
                }
                if p.elements().is_empty() {
                    return;
                }
                layer(out, 1.0, vec![], Some((p, id)));
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PopLayer);
            }
            TransKind::Split => {
                // The old scene splits down the middle and both halves slide apart over the new one.
                self.emit_scene(b, t, &id, false, out);
                // left/right: the halves part sideways; up/down: they part vertically.
                let horiz = tr.dir[1] != 0.0;
                let halves = if horiz { [[0.0, 0.0, w, h * 0.5], [0.0, h * 0.5, w, h]] } else { [[0.0, 0.0, w * 0.5, h], [w * 0.5, 0.0, w, h]] };
                for (i, r) in halves.iter().enumerate() {
                    let sgn = if i == 0 { -1.0 } else { 1.0 };
                    let off = if horiz { translate(0.0, sgn * h * 0.5 * e) } else { translate(sgn * w * 0.5 * e, 0.0) };
                    let clip = kurbo::Rect::new(r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64);
                    layer(out, 1.0, vec![], Some((shape_path(&clip, 0.1), off)));
                    self.emit_scene(a, t, &off, false, out);
                    out.push(Op::PopLayer);
                }
            }
            TransKind::Whip => {
                // A whip pan: both scenes travel a full frame, blurred by their speed (peak mid-move).
                let speed = 4.0 * e * (1.0 - e);
                let blur = (tr.amount.max(0.0) + 1.0) * 36.0 * speed;
                let filters = if blur > 0.5 { vec![Filter::Blur(blur)] } else { Vec::new() };
                let (dx, dy) = (tr.dir[0] * w, tr.dir[1] * h);
                let st = 1.0 + 0.25 * speed;
                let stretch = |tx: f32, ty: f32| dl::mul(&translate(tx + w * 0.5, ty + h * 0.5), &dl::mul(&scale(if dx != 0.0 { st } else { 1.0 }, if dy != 0.0 { st } else { 1.0 }), &translate(-w * 0.5, -h * 0.5)));
                layer(out, 1.0, filters.clone(), None);
                self.emit_scene(a, t, &stretch(dx * e, dy * e), false, out);
                out.push(Op::PopLayer);
                layer(out, 1.0, filters, None);
                self.emit_scene(b, t, &stretch(-dx * (1.0 - e), -dy * (1.0 - e)), false, out);
                out.push(Op::PopLayer);
            }
            TransKind::Glitch => {
                let (sc, k) = if p < 0.5 { (a, p * 2.0) } else { (b, (1.0 - p) * 2.0) };
                let bands = 9;
                let frame = (t * self.fps).floor() as f64;
                for i in 0..bands {
                    let y0 = h * i as f32 / bands as f32;
                    let y1 = h * (i + 1) as f32 / bands as f32;
                    let r = hash01(frame * 31.0 + i as f64);
                    let dx = if r > 0.45 { (hash01(frame * 7.0 + i as f64 * 3.0) as f32 - 0.5) * w * 0.12 * k * tr.amount } else { 0.0 };
                    let clip = kurbo::Rect::new(0.0, y0 as f64, w as f64, y1 as f64);
                    layer(out, 1.0, vec![], Some((shape_path(&clip, 0.1), id)));
                    self.emit_scene(sc, t, &translate(dx, 0.0), false, out);
                    out.push(Op::PopLayer);
                }
            }
        }
    }

    /// Compute a node's property values at local time `lt`.
    fn props(&self, n: &Node, cx: &Cx, lt: f32, v: &mut [f32; prop::N]) {
        self.props_raw(n, cx, lt, v);
        if let Some(ic) = n.inertia.as_deref() {
            let table = ic.table.get_or_init(|| {
                // Sample the layer's own motion and run the spring once (deterministic, 240 Hz).
                let life = (n.end - n.start).min(self.duration.max(1.0) + 1.0).clamp(0.0, 600.0);
                let mut tv = [0.0f32; prop::N];
                ic.simulate(life, |t| {
                    let c = Cx { scene_t: n.start + t, gt: cx.gt - (cx.scene_t - n.start - t), ..*cx };
                    self.props_raw(n, &c, t, &mut tv);
                    tv
                })
            });
            ic.apply(table, lt, v);
        }
    }

    fn props_raw(&self, n: &Node, cx: &Cx, lt: f32, v: &mut [f32; prop::N]) {
        *v = n.base;
        if !n.exprs.is_empty() || !n.cexprs.is_empty() {
            let dur = (n.end - n.start).max(1e-6);
            let vars = Vars {
                t: lt as f64,
                scene_t: cx.scene_t as f64,
                gt: cx.gt as f64,
                p: (lt / dur).clamp(0.0, 1.0) as f64,
                i: n.index as f64,
                n: n.count as f64,
                w: self.width as f64,
                h: self.height as f64,
                fps: self.fps as f64,
                ..Default::default()
            };
            let vars = match &self.groove {
                Some(m) if n.exprs.iter().any(|(_, e)| e.music) || n.cexprs.iter().any(|(_, e)| e.music) => {
                    let (beat, bar, level, bpm) = m.at(cx.gt as f64);
                    Vars { beat, bar, level, bpm, ..vars }
                }
                _ => vars,
            };
            for (slot, e) in &n.exprs {
                v[*slot as usize] = e.eval(&vars) as f32;
            }
            for (slot, e) in &n.cexprs {
                let s = *slot as usize;
                v[s..s + 4].copy_from_slice(&rgba_to_oklab(Rgba(e.eval(&vars))));
            }
        }
        for tr in &n.tracks {
            let s = tr.slot as usize;
            v[s] = tr.sample(lt, v[s], &self.easings);
        }
        for l in &n.loops {
            let tau = std::f32::consts::TAU;
            match *l {
                LoopC::Float { amount, period } => v[prop::Y] += amount * (tau * lt / period).sin(),
                LoopC::Pulse { amount, period } => {
                    let k = 1.0 + amount * 0.5 * (1.0 - (tau * lt / period).cos());
                    v[prop::SX] *= k;
                    v[prop::SY] *= k;
                }
                LoopC::Spin { period } => v[prop::ROT] += 360.0 * lt / period,
                LoopC::Wiggle { amount, period } => v[prop::ROT] += amount * noise1((lt / period) as f64 * 2.0) as f32,
                LoopC::Breathe { amount, period } => v[prop::OPACITY] *= 1.0 - amount * 0.5 * (1.0 - (tau * lt / period).cos()),
                LoopC::Blink { period } => {
                    if (lt / period).fract() >= 0.5 {
                        v[prop::OPACITY] = 0.0
                    }
                }
            }
        }
        if let Some(ast) = &n.update {
            self.run_update(ast, n, cx, lt, v);
        }
        apply_path(n, v);
    }

    /// Apply a layer's `update` script: its returned props win for this frame.
    fn run_update(&self, ast: &rhai::AST, n: &Node, cx: &Cx, lt: f32, v: &mut [f32; prop::N]) {
        let dur = (n.end - n.start).max(1e-6);
        let (beat, bar, level, bpm) = self.groove.as_ref().map(|m| m.at(cx.gt as f64)).unwrap_or_default();
        let vals = [
            lt as f64,
            cx.scene_t as f64,
            cx.gt as f64,
            (lt / dur).clamp(0.0, 1.0) as f64,
            n.index as f64,
            n.count as f64,
            self.width as f64,
            self.height as f64,
            self.fps as f64,
            (cx.gt * self.fps).floor() as f64,
            beat,
            bar,
            level,
            bpm,
        ];
        let Ok(map) = crate::spec::script::run_update(ast, &vals) else { return };
        for (k, val) in map {
            let Some((slot, ch)) = prop::lookup(k.as_str()) else { continue };
            if ch == 4 {
                if let Some(c) = val.clone().into_immutable_string().ok().and_then(|s| self.theme.color(&s)) {
                    v[slot..slot + 4].copy_from_slice(&rgba_to_oklab(c));
                }
            } else if let Some(x) = val.as_float().ok().or_else(|| val.as_int().ok().map(|i| i as f64)) {
                v[slot] = x as f32;
                if ch == 2 {
                    v[slot + 1] = x as f32;
                }
            }
        }
    }

    fn emit_node(&self, n: &Node, cx: &Cx, parent: &Affine, psize: [f32; 2], po: f32, out: &mut Vec<Op>) {
        let tagging = n.tag != u32::MAX && TAGS.with(|t| t.borrow().is_some());
        let pcx;
        let cx = if n.posterize_time > 0.0 {
            // Hold each pose for 1/posterizeTime s: quantize the clock this subtree sees.
            let st = (cx.scene_t * n.posterize_time + 1e-4).floor() / n.posterize_time;
            pcx = Cx { scene_t: st, gt: cx.gt - (cx.scene_t - st), ..*cx };
            &pcx
        } else {
            cx
        };
        let start = out.len();
        if n.matte.is_some() {
            out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: Vec::new(), clip: None, cache_key: None });
        }
        let body = out.len();
        match n.fx.as_deref() {
            Some(fx) if fx.repeat.is_some() || fx.echo.is_some() => self.emit_fx(n, fx, cx, parent, psize, po, out),
            _ => self.emit_node_inner(n, cx, parent, psize, po, out),
        }
        if let Some(m) = n.matte.as_deref() {
            let (mnode, mode) = (&m.0, m.1);
            if out.len() == body {
                out.truncate(start);
            } else {
                // Luma modes turn the matte's luminance into alpha; inverted modes cut instead of keep.
                const LUMA: [f32; 20] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.2126, 0.7152, 0.0722, 0.0, 0.0];
                let filters = if mode >= 2 { vec![Filter::ColorMatrix(LUMA)] } else { Vec::new() };
                let blend = if mode % 2 == 0 { Blend::DestIn } else { Blend::DestOut };
                let mstart = out.len();
                out.push(Op::PushLayer { opacity: 1.0, blend, filters, clip: None, cache_key: None });
                let mb = out.len();
                let mcx = Cx { amul: 1.0, ..*cx };
                self.emit_node(mnode, &mcx, parent, psize, po, out);
                if out.len() == mb {
                    // An empty matte hides everything (keep modes) or nothing (cut modes).
                    if blend == Blend::DestIn {
                        recycle_from(out, start);
                    } else {
                        out.truncate(mstart);
                        out.push(Op::PopLayer);
                    }
                } else {
                    out.push(Op::PopLayer);
                    out.push(Op::PopLayer);
                }
            }
        }
        if tagging && out.len() > start {
            TAGS.with(|t| {
                if let Some(v) = t.borrow_mut().as_mut() {
                    v.push(OpTag { ops: start..out.len(), layer: n.tag });
                }
            });
        }
    }

    /// Repeater and echo: the same node re-emitted at other times and transforms.
    #[allow(clippy::too_many_arguments)]
    fn emit_fx(&self, n: &Node, fx: &fx::Fx, cx: &Cx, parent: &Affine, psize: [f32; 2], po: f32, out: &mut Vec<Op>) {
        let (ec, delay, decay, eblend) = fx.echo.as_ref().map_or((0, 0.0, 1.0, Blend::Normal), |e| (e.count, e.delay, e.decay, e.blend));
        // Echoes (oldest first) share one blended layer; the live copy draws on top, unblended.
        let layered = ec > 0 && eblend != Blend::Normal;
        if layered {
            out.push(Op::PushLayer { opacity: 1.0, blend: eblend, filters: Vec::new(), clip: None, cache_key: None });
        }
        for k in (0..=ec).rev() {
            if k == 0 && layered {
                out.push(Op::PopLayer);
            }
            let tk = k as f32 * delay;
            let ak = decay.powi(k as i32);
            if ak <= 0.002 {
                continue;
            }
            match &fx.repeat {
                None => {
                    let c = Cx { scene_t: cx.scene_t - tk, gt: cx.gt - tk, amul: cx.amul * ak, ..*cx };
                    self.emit_node_inner(n, &c, parent, psize, po, out);
                }
                Some(r) => {
                    let ctr = r.center.unwrap_or([n.base[prop::X], n.base[prop::Y]]);
                    for i in 0..r.count {
                        let a = ak * r.alpha(i);
                        if a <= 0.002 {
                            continue;
                        }
                        let rank = r.ranks.get(i as usize).copied().unwrap_or(i);
                        let ts = tk + r.stagger * rank as f32;
                        let c = Cx { scene_t: cx.scene_t - ts, gt: cx.gt - ts, amul: cx.amul * a, ..*cx };
                        let px = dl::mul(parent, &r.xf(i, ctr));
                        self.emit_node_inner(n, &c, &px, psize, po, out);
                    }
                }
            }
        }
    }

    fn emit_node_inner(&self, n: &Node, cx: &Cx, parent: &Affine, psize: [f32; 2], _po: f32, out: &mut Vec<Op>) {
        let st = cx.scene_t;
        // Half-open [start, end), with both edges nudged by 0.1 ms so float noise in
        // `start + 1/fps` can't show a one-frame layer on two frames.
        if st < n.start - 1e-4 || st >= n.end - 1e-4 {
            return;
        }
        // Stepped time ("on twos"): this layer and its children see time held at `step` fps.
        let stepped;
        let cx = if n.step > 0.0 {
            let lt = st - n.start;
            let held = (lt * n.step + 1e-4).floor() / n.step;
            let d = lt - held;
            stepped = Cx { scene_t: st - d, gt: cx.gt - d, ..*cx };
            &stepped
        } else {
            cx
        };
        let st = cx.scene_t;
        let lt = st - n.start;
        let mut v = [0.0f32; prop::N];
        self.props(n, cx, lt, &mut v);
        if let Some([amt, rot, fps, seed]) = n.boil {
            // Boil: a seeded jitter re-drawn `fps` times a second (hand-drawn / misregistered plates).
            let k = ((cx.gt * fps) + 1e-4).floor() as i64 as u32;
            let sd = seed as u32;
            let r = |j: u32| (graphic::rnd(sd, k.wrapping_mul(3).wrapping_add(j)) as f32 - 0.5) * 2.0;
            v[prop::X] += r(0) * amt;
            v[prop::Y] += r(1) * amt;
            v[prop::ROT] += r(2) * rot;
        }
        let opacity = (v[prop::OPACITY] * cx.amul).clamp(0.0, 1.0);
        if opacity <= 0.0005 {
            return;
        }
        let _ = psize;
        let (w, h) = (v[prop::W].max(0.0), v[prop::H].max(0.0));
        let mut xf = dl::mul(parent, &local_xf(&v, n.anchor, w, h));
        if let Some(k) = n.fx.as_deref().and_then(|f| f.smear) {
            // Smear: stretch along the frame's travel (velocity from one frame earlier), volume kept.
            let dt = 1.0 / self.fps.max(1.0);
            let mut pv = [0.0f32; prop::N];
            let pc = Cx { scene_t: cx.scene_t - dt, gt: cx.gt - dt, ..*cx };
            self.props(n, &pc, lt - dt, &mut pv);
            let (dx, dy) = (v[prop::X] - pv[prop::X], v[prop::Y] - pv[prop::Y]);
            let speed = (dx * dx + dy * dy).sqrt();
            let size = (w * v[prop::SX].abs()).max(h * v[prop::SY].abs()).max(1.0);
            let s = (1.0 + k * 0.6 * speed / size).min(4.0);
            if s > 1.01 {
                let ang = dy.atan2(dx).to_degrees();
                let (cx0, cy0) = (v[prop::X], v[prop::Y]);
                let m = dl::mul(&translate(cx0, cy0), &dl::mul(&rotate(ang), &dl::mul(&scale(s, 1.0 / s.sqrt()), &dl::mul(&rotate(-ang), &translate(-cx0, -cy0)))));
                xf = dl::mul(parent, &dl::mul(&m, &local_xf(&v, n.anchor, w, h)));
            }
        }
        let blur = v[prop::BLUR].max(0.0) + cx.dof;
        let wipe = v[prop::WIPE].clamp(0.0, 1.0);
        let is_leaf = matches!(n.kind, Kind::Text { kinetic: None, .. } | Kind::Image { .. } | Kind::Video { .. } | Kind::Shader(_))
            || matches!(&n.kind, Kind::Shape { fill, stroke, .. } if fill.is_none() || stroke.is_none());
        let needs_layer = blur > 0.05
            || n.blend != Blend::Normal
            || n.shadow.is_some()
            || n.glow.is_some()
            || !n.treat.is_empty()
            || n.clip
            || n.clip_path.is_some()
            || wipe < 1.0
            || (opacity < 0.999 && !is_leaf);
        let mut alpha = opacity;
        if needs_layer {
            let mut filters = n.treat.clone();
            if n.treat_slot != 0 {
                let b = n.treat_slot as usize;
                for f in filters.iter_mut() {
                    match f {
                        Filter::Tone(t) => {
                            let a = v[b].clamp(0.0, 1.0);
                            if (a - t.amount).abs() > 1e-4 {
                                *t = Arc::new(dl::ToneLut { amount: a, ..(**t).clone() });
                            }
                        }
                        Filter::Halftone(h) => {
                            h.cell = v[b + 1].max(2.0);
                            h.angle = v[b + 2];
                            h.gain = v[b + 3].max(0.05);
                        }
                        Filter::Ink { amount, .. } => *amount = v[b + 4].clamp(0.0, 1.0),
                        _ => {}
                    }
                }
            }
            if blur > 0.05 {
                filters.push(Filter::Blur(blur));
            }
            if let Some(f) = &n.shadow {
                filters.push(f.clone());
            }
            if let Some(f) = &n.glow {
                filters.push(f.clone());
            }
            let clip = if wipe < 1.0 {
                let r = match n.wipe_dir {
                    0 => [0.0, 0.0, w * wipe, h],
                    1 => [w * (1.0 - wipe), 0.0, w, h],
                    2 => [0.0, 0.0, w, h * wipe],
                    _ => [0.0, h * (1.0 - wipe), w, h],
                };
                let r = kurbo::Rect::new(r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64);
                Some((shape_path(&r, 0.1), xf))
            } else if let Some(p) = &n.clip_path {
                Some((p.clone(), xf))
            } else if n.clip {
                let rr = kurbo::RoundedRect::new(0.0, 0.0, w as f64, h as f64, v[prop::RADIUS].max(0.0) as f64);
                Some((shape_path(&rr, 0.1), xf))
            } else {
                None
            };
            out.push(Op::PushLayer { opacity, blend: n.blend, filters, clip, cache_key: None });
            alpha = 1.0;
        }
        match &n.kind {
            Kind::Shape { shape, fill, stroke, path } => {
                let wiggled = match (n.fx.as_deref(), n.live_pattern.as_deref()) {
                    (_, Some((pat, [pw, ph]))) => {
                        let pt = graphic::Pat { t: lt as f64, ..pat.clone() };
                        Some(graphic::pattern(*pw, *ph, &pt))
                    }
                    (Some(fx::Fx { wiggle: Some(w), wiggle_src, .. }), None) => {
                        let mut p = take_path();
                        fx::wiggle_into(wiggle_src, *w, lt, &mut p);
                        Some(p)
                    }
                    _ => None,
                };
                let path = wiggled.as_ref().unwrap_or(path);
                let sw = v[prop::SW];
                let trimmed = v[prop::TRIM0] > 0.0 || v[prop::TRIM1] < 1.0;
                let geom = |out: &mut Vec<Op>, paint: Paint, st: Option<dl::Stroke>| match (shape, st) {
                    (ShapeKind::Rect, None) => out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: v[prop::RADIUS].min(w.min(h) * 0.5).max(0.0), paint, transform: xf }),
                    (ShapeKind::Rect, Some(s)) => {
                        let rr = kurbo::RoundedRect::new(0.0, 0.0, w as f64, h as f64, v[prop::RADIUS].min(w.min(h) * 0.5).max(0.0) as f64);
                        out.push(Op::Stroke { path: shape_path(&rr, 0.1), paint, stroke: s, transform: xf })
                    }
                    (ShapeKind::Ellipse, s) => {
                        let e = kurbo::Ellipse::new((w as f64 * 0.5, h as f64 * 0.5), (w as f64 * 0.5, h as f64 * 0.5), 0.0);
                        let path = shape_path(&e, 0.1);
                        match s {
                            None => out.push(Op::Fill { path, paint, rule: dl::FillRule::NonZero, transform: xf }),
                            Some(s) => out.push(Op::Stroke { path, paint, stroke: s, transform: xf }),
                        }
                    }
                    (ShapeKind::Path, None) => out.push(Op::Fill { path: copy_path(path), paint, rule: dl::FillRule::NonZero, transform: xf }),
                    (ShapeKind::Path, Some(s)) => out.push(Op::Stroke { path: copy_path(path), paint, stroke: s, transform: xf }),
                };
                if let Some(f) = fill {
                    if !trimmed || stroke.is_none() {
                        let paint = self.paint(f, &v, prop::FILL, [w, h], alpha);
                        geom(out, paint, None);
                    }
                }
                let along = n.fx.as_deref().and_then(|f| f.along.as_ref());
                if let (Some(s), Some(b)) = (stroke, n.brush.as_deref()) {
                    if sw > 0.0 && v[prop::TRIM1] > v[prop::TRIM0] {
                        let paint = self.paint(&s.paint, &v, prop::STROKE, [w, h], alpha);
                        let mut p = take_path();
                        b.build(sw, v[prop::TRIM0], v[prop::TRIM1], &mut p);
                        out.push(Op::Fill { path: p, paint, rule: dl::FillRule::NonZero, transform: xf });
                    }
                } else if let (Some(_), Some(al)) = (stroke, along) {
                    // Gradient along the path: each chunk carries its slice of the colours (and of the trim).
                    let (t0, t1) = (v[prop::TRIM0].clamp(0.0, 1.0), v[prop::TRIM1].clamp(0.0, 1.0));
                    if sw > 0.0 && t1 > t0 {
                        let fade = |c: Rgba| Rgba([c.0[0], c.0[1], c.0[2], c.0[3] * alpha]);
                        for (p, p0, p1, u0, u1, c0, c1) in &al.chunks {
                            if *u1 <= t0 || *u0 >= t1 {
                                continue;
                            }
                            let span = (u1 - u0).max(1e-6);
                            let trim = [((t0 - u0) / span).clamp(0.0, 1.0), ((t1 - u0) / span).clamp(0.0, 1.0)];
                            let paint = Paint::Linear { p0: *p0, p1: *p1, stops: vec![(0.0, fade(*c0)), (1.0, fade(*c1))] };
                            let st = dl::Stroke { width: sw, cap: 1, join: 1, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim };
                            out.push(Op::Stroke { path: copy_path(p), paint, stroke: st, transform: xf });
                        }
                    }
                } else if let Some(s) = stroke {
                    if sw > 0.0 && v[prop::TRIM1] > v[prop::TRIM0] {
                        let paint = self.paint(&s.paint, &v, prop::STROKE, [w, h], alpha);
                        let st = dl::Stroke {
                            width: sw,
                            cap: s.cap,
                            join: s.join,
                            miter_limit: 4.0,
                            dash: s.dash.clone(),
                            dash_offset: s.dash_offset + v[prop::DASH_OFF],
                            trim: [v[prop::TRIM0].clamp(0.0, 1.0), v[prop::TRIM1].clamp(0.0, 1.0)],
                        };
                        geom(out, paint, Some(st));
                    }
                }
                if let Some(p) = wiggled {
                    give_path(p);
                }
            }
            Kind::Text { layout, wrap, paint, run_colors, split, counter, kinetic, dynamic } => {
                let owned;
                let layout = if let Some(c) = counter {
                    owned = self.layout_counter(c, v[prop::VALUE]);
                    &owned
                } else if let Some(l) = dynamic.as_ref().and_then(|d| d.relayout(&self.assets.fonts, &v)) {
                    owned = l;
                    &owned
                } else {
                    layout
                };
                let lsize = [wrap.unwrap_or(layout.width), layout.height];
                // Centre the block in the layer box (it may differ when the box is wider).
                // Re-laid-out text (animated axes / tracking) keeps its aligned edge.
                let ox = match dynamic.as_ref().map(|d| d.spec.align) {
                    Some(crate::text::Align::Left) => 0.0,
                    Some(crate::text::Align::Right) => w - lsize[0],
                    _ => (w - lsize[0]) * 0.5,
                };
                let oy = (h - lsize[1]) * 0.5;
                let bxf = if ox.abs() > 0.01 || oy.abs() > 0.01 { dl::mul(&xf, &translate(ox, oy)) } else { xf };
                if let Some(k) = kinetic {
                    // Gradient fills: draw the fill, keep it only where the
                    // kinetic glyphs are (their animated alpha included).
                    let grad = !matches!(paint, PaintC::Solid);
                    if grad {
                        out.push(Op::PushLayer { opacity: alpha, blend: Blend::Normal, filters: Vec::new(), clip: None, cache_key: None });
                        let p = self.paint(paint, &v, prop::FILL, lsize, 1.0);
                        // Oversized so units flying in from outside the box are filled too.
                        let (mx, my) = (lsize[0].max(64.0), lsize[1].max(64.0));
                        out.push(Op::RoundRect { x: -mx, y: -my, w: lsize[0] + 2.0 * mx, h: lsize[1] + 2.0 * my, radius: 0.0, paint: p, transform: bxf });
                        out.push(Op::PushLayer { opacity: 1.0, blend: Blend::DestIn, filters: Vec::new(), clip: None, cache_key: None });
                    }
                    let at = out.len();
                    k.ops(lt, bxf, out);
                    if alpha < 1.0 && !grad {
                        for op in &mut out[at..] {
                            retransform(op, &dl::IDENTITY, alpha);
                        }
                    }
                    if grad {
                        out.push(Op::PopLayer);
                        out.push(Op::PopLayer);
                    }
                } else {
                    let fill = (!*run_colors).then(|| self.paint(paint, &v, prop::FILL, lsize, alpha));
                    let reveal = v[prop::REVEAL].clamp(0.0, 1.0);
                    let limit = if reveal >= 1.0 {
                        u32::MAX
                    } else {
                        let k = (reveal * layout.chars.len() as f32).floor() as usize;
                        layout.chars.get(k).map_or(u32::MAX, |u| u.glyphs.start)
                    };
                    let tpath = n.fx.as_deref().and_then(|f| f.text_path.as_ref());
                    match split {
                        None if n.fx.as_deref().is_some_and(|f| f.animator.is_some()) => {
                            let an = n.fx.as_deref().unwrap().animator.as_ref().unwrap();
                            let units = layout.units(an.by);
                            let off = match an.auto {
                                Some((delay, dur, e)) => self.easings[e as usize].ease(((lt - delay) / dur.max(1e-3)).clamp(0.0, 1.0) as f64) as f32 * (1.0 + an.smooth),
                                None => v[prop::RANGE_OFF],
                            };
                            let (lo, hi) = (v[prop::RANGE0] + off, v[prop::RANGE1] + off);
                            let nu = units.len().max(1) as f32;
                            let base_c = [v[prop::FILL], v[prop::FILL + 1], v[prop::FILL + 2], v[prop::FILL + 3]];
                            // Axis animators change advances: shift each unit by the growth of the units
                            // before it on its line, re-aligning the line (left/centre/right).
                            let shifts: Vec<f32> = if an.axes.is_empty() {
                                Vec::new()
                            } else {
                                let fonts = &self.assets.fonts;
                                let mut deltas = Vec::with_capacity(units.len());
                                for (u, unit) in units.iter().enumerate() {
                                    let rank = an.order.get(u).map_or(u as f32, |r| *r as f32);
                                    let a = an.amount((rank + 0.5) / nu, lo, hi);
                                    let mut d = 0.0;
                                    if a > 0.001 {
                                        for gi in unit.glyphs.clone() {
                                            let g = layout.glyphs[gi as usize];
                                            let r = &layout.runs[g.run as usize];
                                            let gid = r.glyphs[g.index as usize].id;
                                            let nf = fonts.toward_axes(r.font, &an.axes, a);
                                            d += fonts.advance(nf, gid, r.size) - fonts.advance(r.font, gid, r.size);
                                        }
                                    }
                                    deltas.push((unit.rect[1], d));
                                }
                                let mut out_s = vec![0.0; deltas.len()];
                                let mut i = 0;
                                while i < deltas.len() {
                                    let mut j = i;
                                    while j < deltas.len() && (deltas[j].0 - deltas[i].0).abs() < 1.0 {
                                        j += 1;
                                    }
                                    let total: f32 = deltas[i..j].iter().map(|x| x.1).sum();
                                    let mut acc = -total * an.align;
                                    for k in i..j {
                                        out_s[k] = acc;
                                        acc += deltas[k].1;
                                    }
                                    i = j;
                                }
                                out_s
                            };
                            for (u, unit) in units.iter().enumerate() {
                                let rank = an.order.get(u).map_or(u as f32, |r| *r as f32);
                                let a = an.amount((rank + 0.5) / nu, lo, hi);
                                let ua = (1.0 + (an.opacity - 1.0) * a).clamp(0.0, 1.0);
                                if ua <= 0.0005 {
                                    continue;
                                }
                                let [x0, y0, uw, uh] = unit.rect;
                                let (cxu, cyu) = (x0 + uw * 0.5, y0 + uh * 0.5);
                                let s = 1.0 + (an.scale - 1.0) * a;
                                let m = dl::mul(&translate(cxu + an.dx * a, cyu + an.dy * a), &dl::mul(&rotate(an.rot * a), &dl::mul(&scale(s, s), &translate(-cxu, -cyu))));
                                let m = match shifts.get(u) {
                                    // Glyphs grow rightwards from their pen position: move by the growth before them.
                                    Some(sh) => dl::mul(&translate(*sh, 0.0), &m),
                                    None => m,
                                };
                                let uxf = dl::mul(&bxf, &m);
                                let f2 = match (an.color, &fill) {
                                    (Some(c), Some(Paint::Solid(_))) => {
                                        let mut k = [0.0f32; 4];
                                        for i in 0..4 {
                                            k[i] = base_c[i] + (c[i] - base_c[i]) * a;
                                        }
                                        let mut c = oklab_to_rgba(k);
                                        c.0[3] *= ua * alpha;
                                        Some(Paint::Solid(c))
                                    }
                                    _ => fill.as_ref().map(|f| {
                                        let mut f = f.clone();
                                        scale_alpha(&mut f, ua);
                                        f
                                    }),
                                };
                                let ublur = an.blur * a;
                                if ublur > 0.05 {
                                    out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Blur(ublur)], clip: None, cache_key: None });
                                }
                                let at = out.len();
                                emit_glyphs(out, layout, unit.glyphs.clone(), limit, f2.as_ref(), alpha * ua, &uxf);
                                if !an.axes.is_empty() && a > 0.001 {
                                    for op in &mut out[at..] {
                                        if let Op::Glyphs { font, .. } = op {
                                            *font = self.assets.fonts.toward_axes(*font, &an.axes, a);
                                        }
                                    }
                                }
                                if ublur > 0.05 {
                                    out.push(Op::PopLayer);
                                }
                            }
                        }
                        None if tpath.is_some() => {
                            // Text on a path: each character rides the path at its x (+ pathOffset), baseline on the line.
                            let mp = tpath.unwrap();
                            for unit in layout.units(UnitKind::Char) {
                                let [x0, y0, uw, uh] = unit.rect;
                                let cxu = x0 + uw * 0.5;
                                let (q, ang) = mp.at(cxu + v[prop::PATH_OFF]);
                                let uxf = dl::mul(&xf, &dl::mul(&translate(q[0], q[1]), &dl::mul(&rotate(ang), &translate(-cxu, -(y0 + uh * 0.78)))));
                                emit_glyphs(out, layout, unit.glyphs.clone(), limit, fill.as_ref(), alpha, &uxf);
                            }
                        }
                        None if layout.outline.is_some() && limit == u32::MAX => {
                            // Outlined text (TEXT's glyph outline paths): fill (unless hollow), then the stroke.
                            let (paths, stroke, _, hollow) = layout.outline.as_ref().unwrap();
                            let mut sc = oklab_to_rgba([v[prop::STROKE], v[prop::STROKE + 1], v[prop::STROKE + 2], v[prop::STROKE + 3]]);
                            sc.0[3] *= alpha;
                            // Stroke under the fill (paint-order: stroke), so the outline frames the letters.
                            for (r, p) in layout.runs.iter().zip(paths) {
                                let w = if *hollow { v[prop::SW] } else { v[prop::SW] * 2.0 };
                                out.push(Op::Stroke { path: p.clone(), paint: Paint::Solid(sc), stroke: dl::Stroke { width: w, ..stroke.clone() }, transform: bxf });
                                if !hollow {
                                    let paint = fill.clone().unwrap_or(Paint::Solid(with_alpha(r.color, alpha)));
                                    out.push(Op::Glyphs { font: r.font, size: r.size, glyphs: crate::text::pool::copy(&r.glyphs), paint, transform: bxf });
                                }
                            }
                        }
                        None => emit_glyphs(out, layout, 0..layout.glyphs.len() as u32, limit, fill.as_ref(), alpha, &bxf),
                        Some(sp) => {
                            for (u, unit) in layout.units(sp.by).iter().enumerate() {
                                let delay = sp.delays.get(u).copied().unwrap_or(0.0);
                                let mut uv = prop::DEFAULTS;
                                for tr in &sp.tracks {
                                    let s = tr.slot as usize;
                                    uv[s] = tr.sample(lt - delay, uv[s], &self.easings);
                                }
                                let mut own_fill = false;
                                if let Some(ut) = sp.unit_tracks.get(u) {
                                    for tr in ut {
                                        let s = tr.slot as usize;
                                        uv[s] = tr.sample(lt, uv[s], &self.easings);
                                        own_fill |= (prop::FILL..prop::FILL + 4).contains(&s);
                                    }
                                }
                                let ua = uv[prop::OPACITY].clamp(0.0, 1.0);
                                if ua <= 0.0005 {
                                    continue;
                                }
                                let [x0, y0, uw, uh] = unit.rect;
                                let mut uvv = uv;
                                uvv[prop::X] = x0 + uw * 0.5 + uv[prop::X];
                                uvv[prop::Y] = y0 + uh * 0.5 + uv[prop::Y];
                                let uxf = dl::mul(&bxf, &dl::mul(&local_xf(&uvv, [0.5, 0.5], uw, uh), &translate(-x0, -y0)));
                                let f2 = if own_fill {
                                    let mut c = oklab_to_rgba([uv[prop::FILL], uv[prop::FILL + 1], uv[prop::FILL + 2], uv[prop::FILL + 3]]);
                                    c.0[3] *= ua * alpha;
                                    Some(Paint::Solid(c))
                                } else {
                                    fill.as_ref().map(|f| {
                                        let mut f = f.clone();
                                        scale_alpha(&mut f, ua);
                                        f
                                    })
                                };
                                let ublur = uv[prop::BLUR];
                                if ublur > 0.05 {
                                    out.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![Filter::Blur(ublur)], clip: None, cache_key: None });
                                }
                                emit_glyphs(out, layout, unit.glyphs.clone(), limit, f2.as_ref(), alpha * ua, &uxf);
                                if ublur > 0.05 {
                                    out.push(Op::PopLayer);
                                }
                            }
                        }
                    }
                }
            }
            Kind::Image { image, natural, fit } => {
                let dst = fit_rect(*natural, [w, h], *fit);
                out.push(Op::Image { image: *image, dst, opacity: alpha, transform: xf });
            }
            Kind::Video { frames, natural, fit, rate, speed, looped } => {
                if !frames.is_empty() {
                    let k = (lt * speed * rate).floor().max(0.0) as usize;
                    let k = if *looped { k % frames.len() } else { k.min(frames.len() - 1) };
                    let dst = fit_rect(*natural, [w, h], *fit);
                    out.push(Op::Image { image: frames[k], dst, opacity: alpha, transform: xf });
                }
            }
            Kind::Shader(sc) => self.emit_shader(sc, cx, &v, &xf, w, h, alpha, lt, out),
            Kind::Dots { groups, size, appear, square } => {
                let n_all: usize = groups.iter().map(|g| g.1.len()).sum();
                let mut vars = Vars { t: lt as f64, gt: cx.gt as f64, n: n_all as f64, w: self.width as f64, h: self.height as f64, fps: self.fps as f64, ..Default::default() };
                let mut i = 0usize;
                for (c, pts) in groups {
                    let mut path = Path::new();
                    for p in pts {
                        let k = match appear {
                            Some(e) => {
                                vars.i = i as f64;
                                e.eval(&vars).clamp(0.0, 1.0) as f32
                            }
                            None => 1.0,
                        };
                        i += 1;
                        let r = (size * 0.5 * k) as f64;
                        if r < 0.05 {
                            continue;
                        }
                        let (x, y) = (p[0] as f64, p[1] as f64);
                        if *square {
                            path.move_to((x - r, y - r));
                            path.line_to((x + r, y - r));
                            path.line_to((x + r, y + r));
                            path.line_to((x - r, y + r));
                            path.close_path();
                        } else {
                            // A circle from 4 cubic arcs.
                            let q = r * 0.552_284_75;
                            path.move_to((x + r, y));
                            path.curve_to((x + r, y + q), (x + q, y + r), (x, y + r));
                            path.curve_to((x - q, y + r), (x - r, y + q), (x - r, y));
                            path.curve_to((x - r, y - q), (x - q, y - r), (x, y - r));
                            path.curve_to((x + q, y - r), (x + r, y - q), (x + r, y));
                            path.close_path();
                        }
                    }
                    if !path.elements().is_empty() {
                        let mut c = *c;
                        c.0[3] *= alpha;
                        out.push(Op::Fill { path, paint: Paint::Solid(c), rule: dl::FillRule::NonZero, transform: xf });
                    }
                }
            }
            Kind::Draw(items) => {
                let trim = [v[prop::TRIM0].clamp(0.0, 1.0), v[prop::TRIM1].clamp(0.0, 1.0)];
                let fade = |c: Rgba, k: f32| Rgba([c.0[0], c.0[1], c.0[2], c.0[3] * k]);
                for it in items {
                    let a = alpha * it.opacity;
                    let (w, cap, join) = it.style;
                    let style = || dl::Stroke { width: w, cap, join, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim };
                    if let Some(c) = it.fill {
                        out.push(Op::Fill { path: copy_path(&it.path), paint: Paint::Solid(fade(c, a)), rule: dl::FillRule::NonZero, transform: xf });
                    }
                    if trim[1] <= trim[0] {
                        continue;
                    }
                    if let Some(c) = it.stroke {
                        out.push(Op::Stroke { path: copy_path(&it.path), paint: Paint::Solid(fade(c, a)), stroke: style(), transform: xf });
                    }
                    for (p, p0, p1, c0, c1) in &it.segs {
                        let paint = Paint::Linear { p0: *p0, p1: *p1, stops: vec![(0.0, fade(*c0, a)), (1.0, fade(*c1, a))] };
                        out.push(Op::Stroke { path: copy_path(p), paint, stroke: style(), transform: xf });
                    }
                }
            }
            Kind::Comp { ev, natural, fit, time } => {
                let ct = time.map(lt, ev.duration);
                let dst = fit_rect(*natural, [w, h], *fit);
                let m = dl::mul(&xf, &[dst[2] / natural[0].max(1.0), 0.0, 0.0, dst[3] / natural[1].max(1.0), dst[0], dst[1]]);
                let r = kurbo::Rect::new(0.0, 0.0, natural[0] as f64, natural[1] as f64);
                out.push(Op::PushLayer { opacity: alpha, blend: Blend::Normal, filters: Vec::new(), clip: Some((shape_path(&r, 0.1), m)), cache_key: None });
                let start = out.len();
                // The nested layers' tags would name the wrong evaluator's layers: the
                // whole composition is tagged as this layer by `emit_node`.
                let tags = TAGS.with(|tg| tg.borrow_mut().take());
                let k = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                // Pixel content inside (shader layers) renders at the largest size
                // the composition is shown at in this frame: display scale ×2
                // (headroom for filtering), in ⅛ steps, never above its declared size.
                let ps = shader::preview_scale();
                let want = (((k * 2.0 * 8.0).ceil() / 8.0).min(1.0) * ps).max(1.0 / 64.0);
                let key = Arc::as_ptr(ev) as usize;
                let inner = COMP_SCALE.with(|c| match &mut *c.borrow_mut() {
                    Some((true, m)) => {
                        let e = m.entry(key).or_insert(0.0);
                        *e = e.max(want);
                        want
                    }
                    Some((false, m)) => m.get(&key).copied().unwrap_or(0.0).max(want),
                    None => want,
                });
                shader::set_preview_scale(inner);
                ev.emit_frame(ct, true, out);
                shader::set_preview_scale(ps);
                TAGS.with(|tg| *tg.borrow_mut() = tags);
                crate::components::transform_ops(&mut out[start..], &m);
                if (k - 1.0).abs() > 1e-4 {
                    scale_filters(&mut out[start..], k);
                }
                out.push(Op::PopLayer);
            }
            Kind::Group { children } => {
                let ccx = Cx { ev: cx.ev, scene_t: cx.scene_t, gt: cx.gt, dof: 0.0, amul: 1.0 };
                for (i, c) in children.iter().enumerate() {
                    if n.drag > 0.0 && i > 0 {
                        // Drag: this child sees the group's transform from i × drag seconds ago.
                        let d = n.drag * i as f32;
                        let mut gv = [0.0f32; prop::N];
                        let gcx = Cx { scene_t: cx.scene_t - d, gt: cx.gt - d, ..*cx };
                        self.props(n, &gcx, (lt - d).max(0.0), &mut gv);
                        let gxf = dl::mul(parent, &local_xf(&gv, n.anchor, gv[prop::W].max(0.0), gv[prop::H].max(0.0)));
                        self.emit_node(c, &ccx, &gxf, [w, h], alpha, out);
                    } else {
                        self.emit_node(c, &ccx, &xf, [w, h], alpha, out);
                    }
                }
            }
            Kind::Box { children, bg, border } => {
                let r = v[prop::RADIUS].min(w.min(h) * 0.5).max(0.0);
                if let Some(bg) = bg {
                    let paint = self.paint(bg, &v, prop::FILL, [w, h], alpha);
                    out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: r, paint, transform: xf });
                }
                if let Some((bw, c)) = border {
                    let rr = kurbo::RoundedRect::new(*bw as f64 * 0.5, *bw as f64 * 0.5, (w - bw * 0.5) as f64, (h - bw * 0.5) as f64, (r - bw * 0.5).max(0.0) as f64);
                    let mut c = *c;
                    c.0[3] *= alpha;
                    out.push(Op::Stroke {
                        path: shape_path(&rr, 0.1),
                        paint: Paint::Solid(c),
                        stroke: dl::Stroke { width: *bw, cap: 0, join: 0, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] },
                        transform: xf,
                    });
                }
                let ccx = Cx { ev: cx.ev, scene_t: cx.scene_t, gt: cx.gt, dof: 0.0, amul: 1.0 };
                for c in children {
                    self.emit_node(c, &ccx, &xf, [w, h], alpha, out);
                }
            }
            Kind::Component(c, th) => {
                let ctx = crate::components::Ctx { width: w, height: h, fps: self.fps, theme: th.as_ref().unwrap_or(&self.ctheme), assets: &*self.assets };
                let ops = c.eval(lt.max(0.0), &ctx);
                let k = out.len();
                out.extend(ops);
                for op in &mut out[k..] {
                    retransform(op, &xf, alpha);
                }
            }
        }
        if needs_layer {
            out.push(Op::PopLayer);
        }
    }

    fn layout_counter(&self, c: &Counter, value: f32) -> Arc<TextLayout> {
        let mut spec = c.spec.clone();
        spec.text = format!("{}{}{}", c.prefix, format_value(value as f64, c.decimals, c.thousands), c.suffix);
        self.assets.fonts.layout(&spec)
    }

    /// Build a display-list paint; solid colours come from the colour slots.
    fn paint(&self, p: &PaintC, v: &[f32; prop::N], slot: usize, size: [f32; 2], alpha: f32) -> Paint {
        match p {
            PaintC::Solid => {
                let mut c = oklab_to_rgba([v[slot], v[slot + 1], v[slot + 2], v[slot + 3]]);
                c.0[3] *= alpha;
                Paint::Solid(c)
            }
            PaintC::Linear { angle, stops } => {
                // CSS angle: 0deg = to top, 90deg = to right.
                let a = angle.to_radians();
                let (dx, dy) = (a.sin(), -a.cos());
                let half = (size[0] * dx.abs() + size[1] * dy.abs()) * 0.5;
                let (cx, cy) = (size[0] * 0.5, size[1] * 0.5);
                Paint::Linear {
                    p0: [cx - dx * half, cy - dy * half],
                    p1: [cx + dx * half, cy + dy * half],
                    stops: stops.iter().map(|(o, c)| (*o, with_alpha(*c, alpha))).collect(),
                }
            }
            PaintC::Radial { c, r, stops } => Paint::Radial {
                c: [c[0] * size[0], c[1] * size[1]],
                r: r * (size[0] * size[0] + size[1] * size[1]).sqrt() * 0.5,
                stops: stops.iter().map(|(o, c)| (*o, with_alpha(*c, alpha))).collect(),
            },
        }
    }
}

/// Emit glyphs `range` (indices into `layout.glyphs`) below `limit`, one op per run.
/// Is a node (and its subtree) unchanged over scene times `[a, b]`?
/// Scale blur, shadow and glow sizes (ops moved under a scaling transform).
pub(crate) fn scale_filters(ops: &mut [Op], s: f32) {
    for op in ops {
        if let Op::PushLayer { filters, .. } = op {
            for f in filters {
                match f {
                    Filter::Blur(r) => *r *= s,
                    Filter::Shadow { dx, dy, blur, .. } => {
                        *dx *= s;
                        *dy *= s;
                        *blur *= s;
                    }
                    Filter::Glow { radius, .. } => *radius *= s,
                    Filter::Halftone(h) => h.cell *= s,
                    Filter::Ink { scale, .. } => *scale *= s,
                    Filter::ColorMatrix(_) | Filter::Tone(_) => {}
                }
            }
        }
    }
}

fn node_static(n: &Node, a: f32, b: f32) -> bool {
    // Appears or disappears inside the span.
    if (n.start > a && n.start <= b) || (n.end > a && n.end <= b) {
        return false;
    }
    if b < n.start || a >= n.end {
        return true; // invisible throughout
    }
    let (la, lb) = (a - n.start, b - n.start);
    if n.boil.is_some() || n.live_pattern.is_some() || n.update.is_some() || n.exprs.iter().any(|(_, e)| e.dynamic) || n.cexprs.iter().any(|(_, e)| e.dynamic) || !n.loops.is_empty() || !n.tracks.iter().all(|t| t.still(la, lb)) {
        return false;
    }
    match &n.kind {
        Kind::Group { children } | Kind::Box { children, .. } => children.iter().all(|c| node_static(c, a, b)),
        Kind::Text { kinetic: Some(_), .. } => false,
        Kind::Text { split: Some(sp), .. } => {
            let md = sp.delays.iter().cloned().fold(0.0, f32::max);
            sp.tracks.iter().all(|t| t.still(la - md, lb))
        }
        Kind::Video { frames, .. } => frames.len() <= 1,
        Kind::Dots { appear, .. } => appear.as_ref().is_none_or(|e| !e.dynamic),
        Kind::Comp { ev, time, .. } => {
            let (ca, cb) = (time.map(la, ev.duration), time.map(lb, ev.duration));
            if time.looped && (cb < ca || lb - la >= ev.duration / time.speed.abs().max(1e-6)) {
                ev.is_static(0.0, ev.duration)
            } else {
                ev.is_static(ca.min(cb), ca.max(cb))
            }
        }
        Kind::Component(c, _) => c.duration().is_some_and(|d| la >= d),
        // A shader is static only if it ignores time and every input is static
        // (animated uniforms are tracks and expressions, checked above).
        Kind::Shader(sc) => !sc.dynamic && sc.channels.iter().all(|(c, _)| match c {
            shader::ChannelC::Layers(ns) => ns.iter().all(|c| node_static(c, a, b)),
            shader::ChannelC::Comp { ev, time } => {
                let (ca, cb) = (time.map(la, ev.duration), time.map(lb, ev.duration));
                let wraps = time.looped && (cb < ca || lb - la >= ev.duration / time.speed.abs().max(1e-6));
                if wraps { ev.is_static(0.0, ev.duration) } else { ev.is_static(ca.min(cb), ca.max(cb)) }
            }
            _ => true,
        }),
        _ => true,
    }
}

fn emit_glyphs(out: &mut Vec<Op>, layout: &TextLayout, range: std::ops::Range<u32>, limit: u32, fill: Option<&Paint>, alpha: f32, xf: &Affine) {
    let mut i = range.start;
    let end = range.end.min(limit);
    while i < end {
        let run = layout.glyphs[i as usize].run;
        let r = &layout.runs[run as usize];
        let mut glyphs = crate::text::pool::take(16);
        while i < end && layout.glyphs[i as usize].run == run {
            let g = layout.glyphs[i as usize];
            let gl = r.glyphs[g.index as usize];
            glyphs.push(Glyph { id: gl.id, x: gl.x, y: gl.y });
            i += 1;
        }
        let paint = match fill {
            Some(p) => p.clone(),
            None => Paint::Solid(with_alpha(r.color, alpha)),
        };
        out.push(Op::Glyphs { font: r.font, size: r.size, glyphs, paint, transform: *xf });
    }
    if limit == u32::MAX && range.start == 0 && range.end as usize == layout.glyphs.len() {
        for d in &layout.decorations {
            let paint = match fill {
                Some(p) => p.clone(),
                None => Paint::Solid(with_alpha(d.color, alpha)),
            };
            out.push(Op::RoundRect { x: d.rect[0], y: d.rect[1], w: d.rect[2], h: d.rect[3], radius: 0.0, paint, transform: *xf });
        }
    }
}

/// Apply a layer transform and opacity to a component op (in place).
fn retransform(op: &mut Op, xf: &Affine, alpha: f32) {
    let fix = |p: &mut Paint| {
        if alpha < 1.0 {
            scale_alpha(p, alpha)
        }
    };
    match op {
        Op::Fill { paint, transform, .. } | Op::Stroke { paint, transform, .. } | Op::RoundRect { paint, transform, .. } | Op::Glyphs { paint, transform, .. } => {
            *transform = dl::mul(xf, transform);
            fix(paint);
        }
        Op::Image { opacity, transform, .. } => {
            *transform = dl::mul(xf, transform);
            *opacity *= alpha;
        }
        Op::PushLayer { clip, .. } => {
            if let Some((_, t)) = clip {
                *t = dl::mul(xf, t);
            }
        }
        Op::PopLayer => {}
    }
}

fn scale_alpha(p: &mut Paint, a: f32) {
    match p {
        Paint::Solid(c) => c.0[3] *= a,
        Paint::Linear { stops, .. } | Paint::Radial { stops, .. } => stops.iter_mut().for_each(|s| s.1 .0[3] *= a),
        Paint::Image { opacity, .. } => *opacity *= a,
    }
}

fn with_alpha(mut c: Rgba, a: f32) -> Rgba {
    c.0[3] *= a;
    c
}

/// `dst` rect for an image of size `nat` fitted into `box_`.
fn fit_rect(nat: [f32; 2], b: [f32; 2], fit: u8) -> [f32; 4] {
    let (nw, nh) = (nat[0].max(1.0), nat[1].max(1.0));
    let (w, h) = match fit {
        2 => (b[0], b[1]),
        3 => (nw, nh),
        f => {
            let s = if f == 1 { (b[0] / nw).max(b[1] / nh) } else { (b[0] / nw).min(b[1] / nh) };
            (nw * s, nh * s)
        }
    };
    [(b[0] - w) * 0.5, (b[1] - h) * 0.5, w, h]
}

pub(crate) fn local_xf(v: &[f32; prop::N], anchor: [f32; 2], w: f32, h: f32) -> Affine {
    let (sx, sy) = (v[prop::SX], v[prop::SY]);
    let r = v[prop::ROT].to_radians();
    let (kx, ky) = (v[prop::SKX].to_radians().tan(), v[prop::SKY].to_radians().tan());
    let (c, s) = (r.cos(), r.sin());
    // R · K · S, K = [[1, kx], [ky, 1]].
    let (k00, k01, k10, k11) = (sx, kx * sy, ky * sx, sy);
    let a = c * k00 - s * k10;
    let b = s * k00 + c * k10;
    let cc = c * k01 - s * k11;
    let d = s * k01 + c * k11;
    let (px, py) = (-anchor[0] * w, -anchor[1] * h);
    [a, b, cc, d, v[prop::X] + a * px + cc * py, v[prop::Y] + b * px + d * py]
}

pub(crate) fn translate(x: f32, y: f32) -> Affine {
    [1.0, 0.0, 0.0, 1.0, x, y]
}
fn scale(x: f32, y: f32) -> Affine {
    [x, 0.0, 0.0, y, 0.0, 0.0]
}
fn rotate(deg: f32) -> Affine {
    let (s, c) = deg.to_radians().sin_cos();
    [c, s, -s, c, 0.0, 0.0]
}
/// Scale about the frame centre.
fn about(w: f32, h: f32, k: f32) -> Affine {
    [k, 0.0, 0.0, k, w * 0.5 * (1.0 - k), h * 0.5 * (1.0 - k)]
}

fn hash01(x: f64) -> f64 {
    let mut h = (x.to_bits() ^ 0x9E37_79B9_7F4A_7C15).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 31;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Smooth 1D value noise in -1..1.
pub(crate) fn noise1(x: f64) -> f64 {
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash01(i) * 2.0 - 1.0;
    let b = hash01(i + 1.0) * 2.0 - 1.0;
    a + (b - a) * u
}

/// sRGB (straight) → OKLab + alpha.
pub(crate) fn rgba_to_oklab(c: Rgba) -> [f32; 4] {
    use anikit_core::color::{linear_to_oklab, srgb_to_linear};
    let l = linear_to_oklab(srgb_to_linear(c.0[0] as f64), srgb_to_linear(c.0[1] as f64), srgb_to_linear(c.0[2] as f64));
    [l[0] as f32, l[1] as f32, l[2] as f32, c.0[3]]
}

/// OKLab + alpha → sRGB (straight).
pub(crate) fn oklab_to_rgba(v: [f32; 4]) -> Rgba {
    use anikit_core::color::{linear_to_srgb, oklab_to_linear};
    let l = oklab_to_linear(v[0] as f64, v[1] as f64, v[2] as f64);
    let f = |x: f64| (linear_to_srgb(x) as f32).clamp(0.0, 1.0);
    Rgba([f(l[0]), f(l[1]), f(l[2]), v[3].clamp(0.0, 1.0)])
}

fn solid_slots(c: Rgba) -> [f32; prop::N] {
    let mut v = prop::DEFAULTS;
    v[prop::FILL..prop::FILL + 4].copy_from_slice(&rgba_to_oklab(c));
    v
}

/// Format a counter value.
pub(crate) fn format_value(v: f64, decimals: usize, thousands: bool) -> String {
    let s = format!("{:.*}", decimals, v);
    if !thousands {
        return s;
    }
    let (sign, s) = if let Some(r) = s.strip_prefix('-') { ("-", r) } else { ("", s.as_str()) };
    let (int, frac) = match s.find('.') {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    };
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{sign}{out}{frac}")
}

// ---- buffer pools: steady-state eval reuses last frame's allocations

thread_local! {
    static PATHS: std::cell::RefCell<Vec<Path>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn take_path() -> Path {
    PATHS.with(|p| p.borrow_mut().pop()).unwrap_or_default()
}

fn give_path(mut p: Path) {
    p.truncate(0);
    PATHS.with(|v| {
        let mut v = v.borrow_mut();
        if v.len() < 4096 {
            v.push(p);
        }
    });
}

/// A pooled copy of `src`.
fn copy_path(src: &Path) -> Path {
    let mut p = take_path();
    p.extend(src.iter());
    p
}

/// A pooled path of a kurbo shape.
fn shape_path(s: &impl kurbo::Shape, tol: f64) -> Path {
    let mut p = take_path();
    p.extend(s.path_elements(tol));
    p
}

/// Drain `ops`, returning glyph and path buffers to this thread's pools.
/// Motion path: position (and orientation) from the path at `progress`.
#[inline]
fn apply_path(n: &Node, v: &mut [f32; prop::N]) {
    if let Some(fx) = n.fx.as_deref() {
        if let Some(mp) = &fx.path {
            let (q, ang) = mp.at(v[prop::PROGRESS] * mp.len());
            v[prop::X] = q[0];
            v[prop::Y] = q[1];
            if let Some(o) = fx.orient {
                v[prop::ROT] += ang + o;
            }
        }
    }
}

/// Recycle the ops from `start` on.
fn recycle_from(ops: &mut Vec<Op>, start: usize) {
    for op in ops.drain(start..) {
        match op {
            Op::Glyphs { glyphs, .. } => crate::text::pool::give(glyphs),
            Op::Fill { path, .. } | Op::Stroke { path, .. } => give_path(path),
            Op::PushLayer { clip: Some((path, _)), .. } => give_path(path),
            _ => {}
        }
    }
}

pub(crate) fn recycle(ops: &mut Vec<Op>) {
    for op in ops.drain(..) {
        match op {
            Op::Glyphs { glyphs, .. } => crate::text::pool::give(glyphs),
            Op::Fill { path, .. } | Op::Stroke { path, .. } => give_path(path),
            Op::PushLayer { clip: Some((path, _)), .. } => give_path(path),
            _ => {}
        }
    }
}
