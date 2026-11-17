//! Text: fonts, rich layout, kinetic typography and code highlighting.
//!
//! * [`FontStore`]: bundled + system + user fonts, shared read-only; every
//!   face instance a layout uses is interned as a [`crate::dl::FontId`].
//! * [`FontStore::layout`]: [`TextSpec`] (rich spans, wrap, align, max lines,
//!   auto-fit) → [`TextLayout`] (glyph runs, decorations, char/word/line units),
//!   cached by content.
//! * [`kinetic`]: per-unit animations that emit display-list ops for a time `t`.
//! * [`code`]: syntax highlighting into styled spans.

pub mod captions;
pub mod code;
pub mod color;
pub mod fonts;
pub mod kinetic;
pub mod layout;

pub use color::Color;
pub use fonts::{Face, FontStore};
pub use layout::{parse_font, Align, Fit, FontShorthand, GlyphRun, Gradient, Span, TextLayout, TextSpec, TextStyle, Unit, UnitKind};

/// Recycling of glyph buffers so steady-state text eval does not allocate.
///
/// Every `Op::Glyphs` owns a `Vec<Glyph>`. Text ops take their buffers from a
/// per-thread pool; give finished display lists back with [`recycle`] (on the
/// thread that evaluates frames) and the next frame reuses them.
pub mod pool {
    use std::cell::RefCell;

    use crate::dl::{Glyph, Op};

    const MAX: usize = 4096;

    thread_local! {
        static POOL: RefCell<Vec<Vec<Glyph>>> = const { RefCell::new(Vec::new()) };
    }

    /// An empty buffer with at least `cap` capacity (pooled when possible).
    #[inline]
    pub fn take(cap: usize) -> Vec<Glyph> {
        let v = POOL.with(|p| p.borrow_mut().pop());
        match v {
            Some(mut v) => {
                v.reserve(cap);
                v
            }
            None => Vec::with_capacity(cap.max(16)),
        }
    }

    /// A pooled copy of `glyphs`.
    #[inline]
    pub fn copy(glyphs: &[Glyph]) -> Vec<Glyph> {
        let mut v = take(glyphs.len());
        v.extend_from_slice(glyphs);
        v
    }

    /// Return one buffer to the pool.
    #[inline]
    pub fn give(mut v: Vec<Glyph>) {
        if v.capacity() == 0 {
            return;
        }
        v.clear();
        POOL.with(|p| {
            let mut p = p.borrow_mut();
            if p.len() < MAX {
                p.push(v);
            }
        });
    }

    /// Drain `ops`, returning every glyph buffer to this thread's pool.
    pub fn recycle(ops: &mut Vec<Op>) {
        for op in ops.drain(..) {
            if let Op::Glyphs { glyphs, .. } = op {
                give(glyphs);
            }
        }
    }
}

pub use pool::recycle;
