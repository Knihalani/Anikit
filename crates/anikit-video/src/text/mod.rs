//! Text: fonts, rich layout, kinetic typography and code highlighting.
//!
//! * [`FontStore`]: bundled + system + user fonts, shared read-only; every
//!   face instance a layout uses is interned as a [`crate::dl::FontId`].
//! * [`FontStore::layout`]: [`TextSpec`] (rich spans, wrap, align, max lines,
//!   auto-fit) → [`TextLayout`] (glyph runs, decorations, char/word/line units),
//!   cached by content.
//! * [`kinetic`]: per-unit animations that emit display-list ops for a time `t`.
//! * [`code`]: syntax highlighting into styled spans.

pub mod code;
pub mod color;
pub mod fonts;
pub mod kinetic;
pub mod layout;

pub use color::Color;
pub use fonts::{Face, FontStore};
pub use layout::{parse_font, Align, Fit, FontShorthand, GlyphRun, Gradient, Span, TextLayout, TextSpec, TextStyle, Unit, UnitKind};
