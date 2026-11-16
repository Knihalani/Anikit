//! [`Host`]: the production [`Assets`] implementation.
//!
//! * Text goes through the TEXT module's [`FontStore`] (Inter, Inter Display,
//!   JetBrains Mono), laid out once per (string, style) and recoloured per
//!   frame, so animating colour or opacity never re-shapes.
//! * Images that components compute (noise fields, 3D renders, grain tiles)
//!   are stored here under ids at or above [`DYNAMIC_BASE`], so they never
//!   collide with the spec's own image ids. The renderer resolves them with
//!   [`Host::pixmap`] (or [`Host::render_assets`] builds a complete
//!   `render::Assets` snapshot for stills and tests).

use super::text::{Face, TextStyle};
use super::Assets;
use crate::dl::{self, ImageId, Op};
use crate::text::{FontStore, TextSpec};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use vello_cpu::Pixmap;

/// First id used for component-made images.
pub const DYNAMIC_BASE: u32 = 0x4000_0000;

pub struct Host {
    pub fonts: Arc<FontStore>,
    by_key: RwLock<HashMap<u64, ImageId>>,
    pixmaps: RwLock<Vec<Arc<Pixmap>>>,
    media: RwLock<HashMap<String, (ImageId, u32, u32)>>,
}

impl Host {
    pub fn new(fonts: Arc<FontStore>) -> Host {
        Host { fonts, by_key: RwLock::default(), pixmaps: RwLock::default(), media: RwLock::default() }
    }

    /// A host with the bundled fonts.
    pub fn bundled() -> Host {
        Host::new(Arc::new(FontStore::new()))
    }

    /// Pixels of a component-made image (premultiplied, vello format).
    pub fn pixmap(&self, id: ImageId) -> Option<Arc<Pixmap>> {
        let i = id.0.checked_sub(DYNAMIC_BASE)? as usize;
        self.pixmaps.read().unwrap().get(i).cloned()
    }

    /// Register a user image/video frame under its spec `src` (straight RGBA8).
    pub fn add_media(&self, src: &str, w: u32, h: u32, rgba: Vec<u8>) -> ImageId {
        let id = self.push(w, h, rgba);
        self.media.write().unwrap().insert(src.to_string(), (id, w, h));
        id
    }

    /// Point a `src` at an image the caller already owns (e.g. in the spec's image store).
    pub fn alias_media(&self, src: &str, id: ImageId, w: u32, h: u32) {
        self.media.write().unwrap().insert(src.to_string(), (id, w, h));
    }

    fn push(&self, w: u32, h: u32, mut rgba: Vec<u8>) -> ImageId {
        let mut opaque = true;
        for px in rgba.chunks_exact_mut(4) {
            let a = px[3] as u32;
            if a != 255 {
                opaque = false;
                for c in &mut px[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
        }
        let mut pm = Pixmap::new(w.clamp(1, 65535) as u16, h.clamp(1, 65535) as u16);
        if pm.data_as_u8_slice().len() == rgba.len() {
            pm.data_as_u8_slice_mut().copy_from_slice(&rgba);
        }
        pm.set_may_have_transparency(!opaque);
        let mut v = self.pixmaps.write().unwrap();
        v.push(Arc::new(pm));
        ImageId(DYNAMIC_BASE + v.len() as u32 - 1)
    }

    /// Number of component-made images held (for cache-pressure stats).
    pub fn image_count(&self) -> usize {
        self.pixmaps.read().unwrap().len()
    }

    /// Everything the renderer needs: every font face interned so far plus
    /// every image held here. `base` contributes the spec's own images.
    pub fn render_assets(&self, base: Option<&crate::render::ImageStore>) -> crate::render::Assets {
        let mut a = crate::render::Assets::default();
        for i in 0..self.fonts.face_count() {
            let id = dl::FontId(i as u32);
            a.fonts.insert(id, self.fonts.face(id).data.clone());
        }
        let _ = base; // spec images are inserted by the caller into `a.images` as usual
        for (i, pm) in self.pixmaps.read().unwrap().iter().enumerate() {
            a.images.insert_pixmap(ImageId(DYNAMIC_BASE + i as u32), pm.clone());
        }
        a
    }
}

fn spec_for(s: &str, st: &TextStyle) -> TextSpec {
    let font = match st.face {
        Face::Sans => "Inter",
        Face::Display => "Inter Display",
        Face::Mono => "JetBrains Mono",
    };
    let style = crate::text::TextStyle {
        font: font.into(),
        weight: st.weight as f32,
        size: st.size,
        letter_spacing: st.tracking,
        line_height: 1.0,
        // Laid out in white and recoloured per draw, so colour changes hit the cache.
        color: crate::text::Color(dl::Rgba([1.0, 1.0, 1.0, 1.0])),
        features: st.tabular.then(|| "\"tnum\" on".to_string()),
        ..Default::default()
    };
    TextSpec::plain(s, style)
}

impl Assets for Host {
    fn image(&self, key: u64, w: u32, h: u32, make: &mut dyn FnMut() -> Vec<u8>) -> ImageId {
        if let Some(id) = self.by_key.read().unwrap().get(&key) {
            return *id;
        }
        let px = make();
        let id = self.push(w, h, px);
        *self.by_key.write().unwrap().entry(key).or_insert(id)
    }

    fn media(&self, src: &str, _t: f32) -> Option<(ImageId, u32, u32)> {
        self.media.read().unwrap().get(src).copied()
    }

    fn text(&self, s: &str, st: &TextStyle, x: f32, y: f32) -> (Vec<Op>, f32) {
        if s.is_empty() {
            return (Vec::new(), 0.0);
        }
        let l = self.fonts.layout(&spec_for(s, st));
        let base = l.lines.first().map(|u| u.baseline).unwrap_or(st.size * 0.8);
        let mut ops = l.to_ops([1.0, 0.0, 0.0, 1.0, x, y - base]);
        let paint = st.paint_at(x, y - base);
        for op in &mut ops {
            match op {
                Op::Glyphs { paint: p, .. } | Op::RoundRect { paint: p, .. } => *p = paint.clone(),
                _ => {}
            }
        }
        (ops, l.width)
    }

    fn measure(&self, s: &str, st: &TextStyle) -> f32 {
        if s.is_empty() {
            return 0.0;
        }
        // Layout drops trailing whitespace; measure it via a bracketing glyph.
        let trail = s.len() - s.trim_end().len();
        if trail == 0 {
            return self.fonts.layout(&spec_for(s, st)).width;
        }
        let with = self.fonts.layout(&spec_for(&format!("{s}|"), st)).width;
        let bar = self.fonts.layout(&spec_for("|", st)).width;
        with - bar
    }
}
