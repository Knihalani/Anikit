//! The display list: the one contract every part of the engine meets at.
//!
//! `eval` (and `components`) turn a spec at time `t` into a [`DisplayList`];
//! `render` rasterises a display list into a premultiplied RGBA frame (or
//! straight into planar YUV). Nothing else crosses module boundaries, so each
//! side can be optimised independently, and a display list can be hashed to
//! skip identical frames.
//!
//! Coordinates are in output pixels (origin top-left, y down) after the
//! op's `transform`. Colours are **sRGB, straight (non-premultiplied) alpha**,
//! `f32` 0..1, which maps 1:1 onto vello_cpu's paint colours.
//! Paths are `kurbo::BezPath` (re-exported), the rasteriser's native type.

/// 2D affine transform `[a, b, c, d, e, f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
pub type Affine = [f32; 6];

/// Identity transform.
pub const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Multiply two affines: apply `b` first, then `a`.
pub fn mul(a: &Affine, b: &Affine) -> Affine {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

/// sRGB colour with straight alpha, components 0..1.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rgba(pub [f32; 4]);

/// A path in local coordinates (the rasteriser's native path type).
pub use kurbo::BezPath as Path;

/// How a shape is coloured.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    /// One colour.
    Solid(Rgba),
    /// Linear gradient from `p0` to `p1` (local coordinates); stops are `(offset 0..1, colour)`.
    Linear { p0: [f32; 2], p1: [f32; 2], stops: Vec<(f32, Rgba)> },
    /// Radial gradient centred at `c` with radius `r`.
    Radial { c: [f32; 2], r: f32, stops: Vec<(f32, Rgba)> },
    /// An image (see [`ImageId`]) mapped by `transform` (local → image pixels).
    Image { image: ImageId, transform: Affine, opacity: f32 },
}

/// Stroke style.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    /// 0 butt, 1 round, 2 square.
    pub cap: u8,
    /// 0 miter, 1 round, 2 bevel.
    pub join: u8,
    pub miter_limit: f32,
    /// Dash pattern (on, off, …) and offset; empty = solid.
    pub dash: Vec<f32>,
    pub dash_offset: f32,
    /// Trim the path to `[start, end]` of its length (0..1), for draw-on effects.
    pub trim: [f32; 2],
}

/// Fill rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// Blend modes (premultiplied, Porter-Duff source-over plus the usual separable modes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    SoftLight,
    Difference,
    /// Porter-Duff destination-in: keep what is below only where this layer has alpha (track mattes).
    DestIn,
    /// Porter-Duff destination-out: keep what is below only where this layer has no alpha.
    DestOut,
}

impl Blend {
    /// Compositing (not colour-mixing) modes that change pixels outside their own coverage.
    pub fn is_compose(self) -> bool {
        matches!(self, Blend::DestIn | Blend::DestOut)
    }
}

/// Per-layer filters, applied to the layer's pixels before compositing.
#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    /// Gaussian blur, radius in pixels.
    Blur(f32),
    /// Drop shadow: offset, blur radius, colour.
    Shadow { dx: f32, dy: f32, blur: f32, color: Rgba },
    /// Glow: blur radius, intensity, colour tint.
    Glow { radius: f32, intensity: f32, color: Rgba },
    /// Colour matrix (4×5, row-major) on unpremultiplied colour.
    ColorMatrix([f32; 20]),
    /// Tone treatment: posterise and/or a gradient map (duotone, tritone) on luma.
    Tone(std::sync::Arc<ToneLut>),
    /// Print halftone screen, anchored to the frame (`ox`, `oy`: the layer
    /// bitmap's frame origin, set by the renderer).
    Halftone(Halftone),
}

/// A tone treatment, precomputed: `map` is a 256-entry gradient map indexed
/// by luma (after posterising to `levels` when `levels >= 2`); without a map,
/// `levels` posterises each channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneLut {
    pub map: Option<Box<[[u8; 3]; 256]>>,
    pub levels: u8,
    /// Mix with the original colour (1 = full treatment).
    pub amount: f32,
}

/// A halftone screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Halftone {
    /// Cell pitch in pixels.
    pub cell: f32,
    /// Screen angle in degrees.
    pub angle: f32,
    /// 0 = round dot, 1 = line, 2 = square (diamond at 45°).
    pub shape: u8,
    /// Ink colour; alpha 0 = take each cell's own colour.
    pub ink: Rgba,
    /// Paper colour; alpha 0 = transparent (only the dots remain).
    pub paper: Rgba,
    /// Dot gain: scales dot area (1 = linear in tone).
    pub gain: f32,
    pub ox: f32,
    pub oy: f32,
}

/// Opaque handle to a decoded image or video frame held by the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

/// Opaque handle to a font face held by the text system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId(pub u32);

/// One positioned glyph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub id: u16,
    pub x: f32,
    pub y: f32,
}

/// One drawing operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Fill a path.
    Fill { path: Path, paint: Paint, rule: FillRule, transform: Affine },
    /// Stroke a path.
    Stroke { path: Path, paint: Paint, stroke: Stroke, transform: Affine },
    /// Axis-aligned rounded rectangle (fast path; very common in video UI).
    RoundRect { x: f32, y: f32, w: f32, h: f32, radius: f32, paint: Paint, transform: Affine },
    /// Draw an image (`dst` rect in local coordinates).
    Image { image: ImageId, dst: [f32; 4], opacity: f32, transform: Affine },
    /// A run of glyphs from one font at one size.
    Glyphs { font: FontId, size: f32, glyphs: Vec<Glyph>, paint: Paint, transform: Affine },
    /// Begin an isolated layer: everything until the matching `PopLayer` is
    /// rendered into a group, filtered, clipped/masked and composited with
    /// `opacity` and `blend`.
    PushLayer { opacity: f32, blend: Blend, filters: Vec<Filter>, clip: Option<(Path, Affine)>, cache_key: Option<u64> },
    /// End the innermost layer.
    PopLayer,
}

/// A frame's worth of drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    pub width: u32,
    pub height: u32,
    /// Cleared to this colour first.
    pub background: Rgba,
    pub ops: Vec<Op>,
}
