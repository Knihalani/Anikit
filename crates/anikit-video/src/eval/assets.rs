//! Shared assets: fonts (TEXT's `FontStore`) and images (user images plus
//! images components make on demand). Implements `components::Assets` and
//! produces `render::Assets` snapshots for the renderers.
//!
//! Components and counters may add images or font faces while frames are
//! evaluated, so the store is append-only with a version number; a render
//! worker re-snapshots when the version it holds is stale.

use crate::components::{self, text::Face as CFace};
use crate::dl::{self, ImageId, Op};
use crate::render;
use crate::text::{self, FontStore, TextSpec};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use vello_cpu::Pixmap;

/// Fonts and images shared by evaluation and rendering.
pub struct SharedAssets {
    pub fonts: Arc<FontStore>,
    images: RwLock<Vec<Arc<Pixmap>>>,
    by_key: RwLock<HashMap<u64, ImageId>>,
    media: RwLock<HashMap<String, ImageId>>,
    version: AtomicU64,
}

impl Default for SharedAssets {
    fn default() -> Self {
        Self::new(Arc::new(FontStore::new()))
    }
}

impl SharedAssets {
    pub fn new(fonts: Arc<FontStore>) -> Self {
        SharedAssets {
            fonts,
            images: RwLock::new(Vec::new()),
            by_key: RwLock::new(HashMap::new()),
            media: RwLock::new(HashMap::new()),
            version: AtomicU64::new(1),
        }
    }

    /// Changes whenever an image or a font face is added.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire) ^ ((self.fonts.face_count() as u64) << 40)
    }

    /// Add straight-alpha RGBA8 pixels; returns the id.
    pub fn add_rgba(&self, w: u32, h: u32, rgba: Vec<u8>) -> Result<ImageId, String> {
        let mut scratch = render::ImageStore::new();
        scratch.insert_rgba8(ImageId(0), w, h, rgba).map_err(|e| e.0)?;
        let pm = scratch.get(ImageId(0)).cloned().ok_or("image store")?;
        let mut v = self.images.write().unwrap();
        v.push(pm);
        self.version.fetch_add(1, Ordering::AcqRel);
        Ok(ImageId(v.len() as u32 - 1))
    }

    /// Register a user image under its spec `src`.
    pub fn add_media(&self, src: &str, id: ImageId) {
        self.media.write().unwrap().insert(src.to_string(), id);
    }

    pub fn image_size(&self, id: ImageId) -> Option<(u32, u32)> {
        self.images.read().unwrap().get(id.0 as usize).map(|p| (p.width() as u32, p.height() as u32))
    }

    /// A snapshot for `render::Renderer` (cheap: every image and face is an `Arc`).
    pub fn snapshot(&self) -> Arc<render::Assets> {
        let mut images = render::ImageStore::new();
        for (i, p) in self.images.read().unwrap().iter().enumerate() {
            images.insert_pixmap(ImageId(i as u32), p.clone());
        }
        let mut fonts = render::FontStore::new();
        for i in 0..self.fonts.face_count() {
            let f = self.fonts.face(dl::FontId(i as u32));
            fonts.insert_face(dl::FontId(i as u32), render::Face { data: f.data.clone(), coords: Arc::from(&*f.coords), embolden: f.embolden, skew: f.skew });
        }
        Arc::new(render::Assets { images, fonts })
    }
}

fn family(f: CFace) -> &'static str {
    match f {
        CFace::Sans => "Inter",
        CFace::Display => "Inter Display",
        CFace::Mono => "JetBrains Mono",
        CFace::Serif => "Instrument Serif",
    }
}

fn spec_for(s: &str, st: &components::TextStyle) -> TextSpec {
    TextSpec::plain(
        s,
        text::TextStyle {
            font: family(st.face).into(),
            weight: st.weight as f32,
            size: st.size,
            color: text::Color(st.color),
            letter_spacing: st.tracking,
            features: st.tabular.then(|| "\"tnum\" on".to_string()),
            ..Default::default()
        },
    )
}

impl components::Assets for SharedAssets {
    fn image(&self, key: u64, w: u32, h: u32, make: &mut dyn FnMut() -> Vec<u8>) -> ImageId {
        if let Some(id) = self.by_key.read().unwrap().get(&key) {
            return *id;
        }
        // Hold the key lock while making it so two threads don't both build it.
        let mut keys = self.by_key.write().unwrap();
        if let Some(id) = keys.get(&key) {
            return *id;
        }
        let id = self.add_rgba(w, h, make()).unwrap_or(ImageId(u32::MAX));
        keys.insert(key, id);
        id
    }

    fn media(&self, src: &str, _t: f32) -> Option<(ImageId, u32, u32)> {
        let id = *self.media.read().unwrap().get(src)?;
        let (w, h) = self.image_size(id)?;
        Some((id, w, h))
    }

    fn text(&self, s: &str, st: &components::TextStyle, x: f32, y: f32) -> (Vec<Op>, f32) {
        let l = self.fonts.layout(&spec_for(s, st));
        let base = l.lines.first().map_or(st.size * 0.8, |u| u.baseline);
        let mut ops = Vec::new();
        l.ops([1.0, 0.0, 0.0, 1.0, x, y - base], &mut ops);
        (ops, l.width)
    }

    fn measure(&self, s: &str, st: &components::TextStyle) -> f32 {
        self.fonts.layout(&spec_for(s, st)).width
    }
}
