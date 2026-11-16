//! Evaluation: spec at time `t` → [`DisplayList`].
//!
//! [`Evaluator::new`] does all the work that doesn't depend on time once:
//! themes, units, text layout, box layout, presets, stagger and expressions
//! are compiled into a tree of [`Node`]s, each with a small fixed array of
//! base property values and per-property keyframe tracks. [`Evaluator::eval_into`]
//! then walks the tree and only interpolates.

pub mod assets;
mod compile;
pub mod expr;
pub mod theme;

use crate::dl::{self, Affine, Blend, DisplayList, Filter, Glyph, ImageId, Op, Paint, Path, Rgba};
use crate::text::{kinetic::KineticPlan, TextLayout, TextSpec, UnitKind};
pub use assets::SharedAssets;
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
    pub const N: usize = 30;

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
            "fill" | "color" => (FILL, 4),
            "stroke" => (STROKE, 4),
            _ => return None,
        })
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
    Box {
        children: Vec<Node>,
        bg: Option<PaintC>,
        border: Option<(f32, Rgba)>,
    },
    Component(Box<dyn crate::components::Component>, Option<crate::components::Theme>),
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
    pub tracks: Vec<Track>,
    pub loops: Vec<LoopC>,
    pub anchor: [f32; 2],
    pub blend: Blend,
    pub shadow: Option<Filter>,
    pub glow: Option<Filter>,
    /// Clip children to the box (with radius).
    pub clip: bool,
    /// A clip path in the layer's local box (clip shapes, shape masks).
    pub clip_path: Option<Path>,
    /// Wipe direction (0 left→right reveal, 1 right→left, 2 top→bottom, 3 bottom→top).
    pub wipe_dir: u8,
    pub index: f32,
    pub count: f32,
}

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
    /// Fonts and images, shared with the renderers.
    pub assets: Arc<SharedAssets>,
    pub(crate) easings: Vec<Easing>,
    pub(crate) scenes: Vec<SceneC>,
    pub(crate) overlay: Vec<Node>,
    pub(crate) theme: theme::Theme,
    pub(crate) ctheme: crate::components::Theme,
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

struct Cx<'a> {
    ev: &'a Evaluator,
    scene_t: f32,
    gt: f32,
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
        out.ops.clear();
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
        if !in_trans {
            match s.bg {
                PaintC::Solid => out.background = s.bg_color,
                _ => out.background = Rgba([0.0, 0.0, 0.0, 1.0]),
            }
            self.emit_scene(s, t, &dl::IDENTITY, true, &mut out.ops);
        } else {
            out.background = Rgba([0.0, 0.0, 0.0, 1.0]);
            let prev = &self.scenes[cur - 1];
            let p = (local / s.trans.dur).clamp(0.0, 1.0);
            let e = self.easings[s.trans.ease as usize].ease(p as f64) as f32;
            self.emit_transition(prev, s, t, p, e, &mut out.ops);
        }
        let cx = Cx { ev: self, scene_t: t, gt: t };
        let frame = [self.width as f32, self.height as f32];
        for n in &self.overlay {
            self.emit_node(n, &cx, &dl::IDENTITY, frame, 1.0, &mut out.ops);
        }
    }

    fn emit_scene(&self, s: &SceneC, t: f32, xf: &Affine, bg_done: bool, out: &mut Vec<Op>) {
        let (w, h) = (self.width as f32, self.height as f32);
        if !bg_done || !matches!(s.bg, PaintC::Solid) {
            let paint = self.paint(&s.bg, &solid_slots(s.bg_color), prop::FILL, [w, h], 1.0);
            out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: 0.0, paint, transform: *xf });
        }
        let lt = t - s.start;
        let cx = Cx { ev: self, scene_t: lt, gt: t };
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
        for n in &s.layers {
            self.emit_node(n, &cx, &cam, [w, h], 1.0, out);
        }
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
            TransKind::Crossfade => {
                self.emit_scene(a, t, &id, false, out);
                layer(out, e, vec![], None);
                self.emit_scene(b, t, &id, false, out);
                out.push(Op::PopLayer);
            }
            TransKind::MorphCut => {
                let sa = 1.0 - 0.02 * e;
                self.emit_scene(a, t, &about(w, h, sa), false, out);
                layer(out, e, vec![], None);
                self.emit_scene(b, t, &about(w, h, 1.02 - 0.02 * e), false, out);
                out.push(Op::PopLayer);
            }
            TransKind::Dip => {
                let (sc, k) = if e < 0.5 { (a, e * 2.0) } else { (b, (1.0 - e) * 2.0) };
                self.emit_scene(sc, t, &id, false, out);
                let mut c = tr.color;
                c.0[3] *= k;
                out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: 0.0, paint: Paint::Solid(c), transform: id });
            }
            TransKind::Slide => {
                self.emit_scene(a, t, &id, false, out);
                let off = 1.0 - e;
                self.emit_scene(b, t, &translate(tr.dir[0] * -w * off, tr.dir[1] * -h * off), false, out);
            }
            TransKind::Push => {
                self.emit_scene(a, t, &translate(tr.dir[0] * w * e, tr.dir[1] * h * e), false, out);
                let off = 1.0 - e;
                self.emit_scene(b, t, &translate(tr.dir[0] * -w * off, tr.dir[1] * -h * off), false, out);
            }
            TransKind::Zoom => {
                self.emit_scene(b, t, &about(w, h, 0.85 + 0.15 * e), false, out);
                layer(out, 1.0 - e, vec![], None);
                self.emit_scene(a, t, &about(w, h, 1.0 + 0.4 * e), false, out);
                out.push(Op::PopLayer);
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
                layer(out, 1.0, vec![], Some((kurbo::Shape::to_path(&clip, 0.1), id)));
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
                layer(out, 1.0, vec![], Some((kurbo::Shape::to_path(&c, 0.1), id)));
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
                    layer(out, 1.0, vec![], Some((kurbo::Shape::to_path(&clip, 0.1), id)));
                    self.emit_scene(sc, t, &translate(dx, 0.0), false, out);
                    out.push(Op::PopLayer);
                }
            }
        }
    }

    /// Compute a node's property values at local time `lt`.
    fn props(&self, n: &Node, cx: &Cx, lt: f32, v: &mut [f32; prop::N]) {
        *v = n.base;
        if !n.exprs.is_empty() {
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
            };
            for (slot, e) in &n.exprs {
                v[*slot as usize] = e.eval(&vars) as f32;
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
    }

    fn emit_node(&self, n: &Node, cx: &Cx, parent: &Affine, psize: [f32; 2], _po: f32, out: &mut Vec<Op>) {
        let st = cx.scene_t;
        if st < n.start || st >= n.end {
            return;
        }
        let lt = st - n.start;
        let mut v = [0.0f32; prop::N];
        self.props(n, cx, lt, &mut v);
        let opacity = v[prop::OPACITY].clamp(0.0, 1.0);
        if opacity <= 0.0005 {
            return;
        }
        let _ = psize;
        let (w, h) = (v[prop::W].max(0.0), v[prop::H].max(0.0));
        let xf = dl::mul(parent, &local_xf(&v, n.anchor, w, h));
        let blur = v[prop::BLUR].max(0.0);
        let wipe = v[prop::WIPE].clamp(0.0, 1.0);
        let is_leaf = matches!(n.kind, Kind::Text { kinetic: None, .. } | Kind::Image { .. } | Kind::Video { .. })
            || matches!(&n.kind, Kind::Shape { fill, stroke, .. } if fill.is_none() || stroke.is_none());
        let needs_layer = blur > 0.05
            || n.blend != Blend::Normal
            || n.shadow.is_some()
            || n.glow.is_some()
            || n.clip
            || n.clip_path.is_some()
            || wipe < 1.0
            || (opacity < 0.999 && !is_leaf);
        let mut alpha = opacity;
        if needs_layer {
            let mut filters = Vec::new();
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
                Some((kurbo::Shape::to_path(&r, 0.1), xf))
            } else if let Some(p) = &n.clip_path {
                Some((p.clone(), xf))
            } else if n.clip {
                let rr = kurbo::RoundedRect::new(0.0, 0.0, w as f64, h as f64, v[prop::RADIUS].max(0.0) as f64);
                Some((kurbo::Shape::to_path(&rr, 0.1), xf))
            } else {
                None
            };
            out.push(Op::PushLayer { opacity, blend: n.blend, filters, clip, cache_key: None });
            alpha = 1.0;
        }
        match &n.kind {
            Kind::Shape { shape, fill, stroke, path } => {
                let sw = v[prop::SW];
                let trimmed = v[prop::TRIM0] > 0.0 || v[prop::TRIM1] < 1.0;
                let geom = |out: &mut Vec<Op>, paint: Paint, st: Option<dl::Stroke>| match (shape, st) {
                    (ShapeKind::Rect, None) => out.push(Op::RoundRect { x: 0.0, y: 0.0, w, h, radius: v[prop::RADIUS].min(w.min(h) * 0.5).max(0.0), paint, transform: xf }),
                    (ShapeKind::Rect, Some(s)) => {
                        let rr = kurbo::RoundedRect::new(0.0, 0.0, w as f64, h as f64, v[prop::RADIUS].min(w.min(h) * 0.5).max(0.0) as f64);
                        out.push(Op::Stroke { path: kurbo::Shape::to_path(&rr, 0.1), paint, stroke: s, transform: xf })
                    }
                    (ShapeKind::Ellipse, s) => {
                        let e = kurbo::Ellipse::new((w as f64 * 0.5, h as f64 * 0.5), (w as f64 * 0.5, h as f64 * 0.5), 0.0);
                        let path = kurbo::Shape::to_path(&e, 0.1);
                        match s {
                            None => out.push(Op::Fill { path, paint, rule: dl::FillRule::NonZero, transform: xf }),
                            Some(s) => out.push(Op::Stroke { path, paint, stroke: s, transform: xf }),
                        }
                    }
                    (ShapeKind::Path, None) => out.push(Op::Fill { path: path.clone(), paint, rule: dl::FillRule::NonZero, transform: xf }),
                    (ShapeKind::Path, Some(s)) => out.push(Op::Stroke { path: path.clone(), paint, stroke: s, transform: xf }),
                };
                if let Some(f) = fill {
                    if !trimmed || stroke.is_none() {
                        let paint = self.paint(f, &v, prop::FILL, [w, h], alpha);
                        geom(out, paint, None);
                    }
                }
                if let Some(s) = stroke {
                    if sw > 0.0 && v[prop::TRIM1] > v[prop::TRIM0] {
                        let paint = self.paint(&s.paint, &v, prop::STROKE, [w, h], alpha);
                        let st = dl::Stroke {
                            width: sw,
                            cap: s.cap,
                            join: s.join,
                            miter_limit: 4.0,
                            dash: s.dash.clone(),
                            dash_offset: s.dash_offset,
                            trim: [v[prop::TRIM0].clamp(0.0, 1.0), v[prop::TRIM1].clamp(0.0, 1.0)],
                        };
                        geom(out, paint, Some(st));
                    }
                }
            }
            Kind::Text { layout, wrap, paint, run_colors, split, counter, kinetic } => {
                let owned;
                let layout = if let Some(c) = counter {
                    owned = self.layout_counter(c, v[prop::VALUE]);
                    &owned
                } else {
                    layout
                };
                let lsize = [wrap.unwrap_or(layout.width), layout.height];
                // Centre the block in the layer box (it may differ when the box is wider).
                let ox = (w - lsize[0]) * 0.5;
                let oy = (h - lsize[1]) * 0.5;
                let bxf = if ox.abs() > 0.01 || oy.abs() > 0.01 { dl::mul(&xf, &translate(ox, oy)) } else { xf };
                if let Some(k) = kinetic {
                    let at = out.len();
                    k.ops(lt, bxf, out);
                    if alpha < 1.0 {
                        for op in &mut out[at..] {
                            retransform(op, &dl::IDENTITY, alpha);
                        }
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
                    match split {
                        None => emit_glyphs(out, layout, 0..layout.glyphs.len() as u32, limit, fill.as_ref(), alpha, &bxf),
                        Some(sp) => {
                            for (u, unit) in layout.units(sp.by).iter().enumerate() {
                                let delay = sp.delays.get(u).copied().unwrap_or(0.0);
                                let mut uv = prop::DEFAULTS;
                                for tr in &sp.tracks {
                                    let s = tr.slot as usize;
                                    uv[s] = tr.sample(lt - delay, uv[s], &self.easings);
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
                                let f2 = fill.as_ref().map(|f| {
                                    let mut f = f.clone();
                                    scale_alpha(&mut f, ua);
                                    f
                                });
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
            Kind::Group { children } => {
                let ccx = Cx { ev: cx.ev, scene_t: cx.scene_t, gt: cx.gt };
                for c in children {
                    self.emit_node(c, &ccx, &xf, [w, h], alpha, out);
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
                        path: kurbo::Shape::to_path(&rr, 0.1),
                        paint: Paint::Solid(c),
                        stroke: dl::Stroke { width: *bw, cap: 0, join: 0, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] },
                        transform: xf,
                    });
                }
                for c in children {
                    self.emit_node(c, cx, &xf, [w, h], alpha, out);
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
fn node_static(n: &Node, a: f32, b: f32) -> bool {
    // Appears or disappears inside the span.
    if (n.start > a && n.start <= b) || (n.end > a && n.end <= b) {
        return false;
    }
    if b < n.start || a >= n.end {
        return true; // invisible throughout
    }
    let (la, lb) = (a - n.start, b - n.start);
    if n.exprs.iter().any(|(_, e)| e.dynamic) || !n.loops.is_empty() || !n.tracks.iter().all(|t| t.still(la, lb)) {
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
        Kind::Component(c, _) => c.duration().is_some_and(|d| la >= d),
        _ => true,
    }
}

fn emit_glyphs(out: &mut Vec<Op>, layout: &TextLayout, range: std::ops::Range<u32>, limit: u32, fill: Option<&Paint>, alpha: f32, xf: &Affine) {
    let mut i = range.start;
    let end = range.end.min(limit);
    while i < end {
        let run = layout.glyphs[i as usize].run;
        let r = &layout.runs[run as usize];
        let mut glyphs = Vec::new();
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
