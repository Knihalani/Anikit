//! Text: glyph outlines cached per (font, glyph) and merged into one path
//! per run, so a run costs one fill instead of one fill per glyph.
//! Colour fonts (COLR/CBDT/sbix) fall back to vello's glyph runs.

use std::collections::HashMap;

use kurbo::{Affine, BezPath, PathEl};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use vello_cpu::peniko::FontData;

use crate::dl::{FontId, Glyph};

/// Per-font data: whether it needs the colour path, and outlines in em units (y down).
#[derive(Default)]
struct FontEntry {
    color: bool,
    outlines: HashMap<u16, Option<BezPath>>,
}

#[derive(Default)]
pub struct GlyphCache {
    fonts: HashMap<FontId, FontEntry>,
}

struct Pen<'a>(&'a mut BezPath, f64);

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x as f64 * self.1, -y as f64 * self.1));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x as f64 * self.1, -y as f64 * self.1));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to((cx0 as f64 * self.1, -cy0 as f64 * self.1), (x as f64 * self.1, -y as f64 * self.1));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to(
            (cx0 as f64 * self.1, -cy0 as f64 * self.1),
            (cx1 as f64 * self.1, -cy1 as f64 * self.1),
            (x as f64 * self.1, -y as f64 * self.1),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

impl GlyphCache {
    /// Build the run's outline (in run-local pixels) into `out`.
    /// Returns false if the font must use the colour-glyph fallback (or is unreadable).
    pub fn run_path(&mut self, id: FontId, font: &FontData, size: f32, glyphs: &[Glyph], out: &mut BezPath) -> bool {
        out.truncate(0);
        let Ok(fref) = FontRef::from_index(font.data.data(), font.index) else { return false };
        let entry = self.fonts.entry(id).or_insert_with(|| {
            let has = |t: &[u8; 4]| fref.table_data(skrifa::Tag::new(t)).is_some();
            FontEntry { color: has(b"COLR") || has(b"CBDT") || has(b"sbix"), outlines: HashMap::new() }
        });
        if entry.color {
            return false;
        }
        let mut outlines = None;
        let upem = fref.head().map(|h| h.units_per_em()).unwrap_or(1000).max(1) as f64;
        let s = size as f64;
        for g in glyphs {
            let path = entry.outlines.entry(g.id).or_insert_with(|| {
                let outlines = outlines.get_or_insert_with(|| fref.outline_glyphs());
                let glyph = outlines.get(GlyphId::new(g.id as u32))?;
                let mut p = BezPath::new();
                let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
                glyph.draw(settings, &mut Pen(&mut p, 1.0 / upem)).ok()?;
                (!p.is_empty()).then_some(p)
            });
            if let Some(p) = path {
                let t = Affine::new([s, 0.0, 0.0, s, g.x as f64, g.y as f64]);
                out.extend(p.elements().iter().map(|el| match *el {
                    PathEl::MoveTo(a) => PathEl::MoveTo(t * a),
                    PathEl::LineTo(a) => PathEl::LineTo(t * a),
                    PathEl::QuadTo(a, b) => PathEl::QuadTo(t * a, t * b),
                    PathEl::CurveTo(a, b, c) => PathEl::CurveTo(t * a, t * b, t * c),
                    PathEl::ClosePath => PathEl::ClosePath,
                }));
            }
        }
        true
    }
}
