//! Shared assets: fonts (TEXT's `FontStore`) and images (user images plus
//! images components make on demand). Implements `components::Assets` and
//! produces `render::Assets` snapshots for the renderers.
//!
//! Components and counters may add images or font faces while frames are
//! evaluated, so the store is append-only with a version number; a render
//! worker re-snapshots when the version it holds is stale.

use crate::components;
use crate::dl::{self, ImageId, Op};
use crate::render;
use crate::text::FontStore;
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
    /// Named sub-compositions (weak: the root evaluator owns them) and their texture cache.
    comps: RwLock<Vec<(String, std::sync::Weak<super::Evaluator>, bool)>>,
    textures: std::sync::OnceLock<crate::pipeline::textures::CompTextures>,
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
            comps: RwLock::new(Vec::new()),
            textures: std::sync::OnceLock::new(),
        }
    }

    /// Make a compiled sub-composition reachable from components by name.
    pub(crate) fn register_composition(&self, name: &str, ev: &Arc<super::Evaluator>, looped: bool) {
        let mut c = self.comps.write().unwrap();
        c.retain(|x| x.0 != name);
        c.push((name.to_string(), Arc::downgrade(ev), looped));
    }

    fn comp(&self, name: &str) -> Option<(Arc<super::Evaluator>, bool)> {
        let c = self.comps.read().unwrap();
        c.iter().find(|x| x.0 == name).and_then(|x| Some((x.1.upgrade()?, x.2)))
    }

    /// The shared composition texture cache (`ANIKIT_TEXTURE_CACHE_MB`, default 2048 MB).
    pub fn textures(&self) -> &crate::pipeline::textures::CompTextures {
        self.textures.get_or_init(|| {
            let mb = std::env::var("ANIKIT_TEXTURE_CACHE_MB").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(2048);
            crate::pipeline::textures::CompTextures::new(mb << 20)
        })
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

    /// Number of font faces (grows when layouts intern new face instances).
    pub fn font_count(&self) -> usize {
        self.fonts.face_count()
    }

    /// An image by id (for `render::Renderer::set_image_source`).
    pub fn pixmap(&self, id: ImageId) -> Option<Arc<Pixmap>> {
        self.images.read().unwrap().get(id.0 as usize).cloned()
    }

    /// Fonts only; images come through [`Self::pixmap`] as a dynamic source,
    /// so a renderer survives images being added while frames are evaluated.
    pub fn snapshot_fonts(&self) -> Arc<render::Assets> {
        let mut fonts = render::FontStore::new();
        for i in 0..self.fonts.face_count() {
            let f = self.fonts.face(dl::FontId(i as u32));
            fonts.insert_face(dl::FontId(i as u32), render::Face { data: f.data.clone(), coords: Arc::from(&*f.coords), embolden: f.embolden, skew: f.skew });
        }
        Arc::new(render::Assets { images: render::ImageStore::new(), fonts })
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



impl components::Assets for SharedAssets {
    fn composition_info(&self, name: &str) -> Option<components::CompInfo> {
        let (ev, looped) = self.comp(name)?;
        Some(components::CompInfo { width: ev.width, height: ev.height, fps: ev.fps, duration: ev.duration, looped })
    }

    fn composition_frame(&self, name: &str, t: f32, lod: u8) -> Option<Arc<Pixmap>> {
        let (ev, _) = self.comp(name)?;
        Some(self.textures().get(&ev, t, lod))
    }

    fn image(&self, key: u64, w: u32, h: u32, make: &mut dyn FnMut() -> Vec<u8>) -> ImageId {
        if let Some(id) = self.by_key.read().unwrap().get(&key) {
            return *id;
        }
        // Build without holding the key lock: `make` may use rayon, and a job
        // stolen while it waits can call `image` again on this thread, which
        // would deadlock on the lock. A rare duplicate build is harmless.
        let px = make();
        let mut keys = self.by_key.write().unwrap();
        if let Some(id) = keys.get(&key) {
            return *id;
        }
        let id = self.add_rgba(w, h, px).unwrap_or(ImageId(u32::MAX));
        keys.insert(key, id);
        id
    }

    fn media(&self, src: &str, _t: f32) -> Option<(ImageId, u32, u32)> {
        let id = *self.media.read().unwrap().get(src)?;
        let (w, h) = self.image_size(id)?;
        Some((id, w, h))
    }

    fn text(&self, s: &str, st: &components::TextStyle, x: f32, y: f32) -> (Vec<Op>, f32) {
        components::host::draw_text(&self.fonts, s, st, x, y)
    }

    fn measure(&self, s: &str, st: &components::TextStyle) -> f32 {
        components::host::measure_text(&self.fonts, s, st)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Assets;

    /// `make` may re-enter `image` (rayon work-stealing does exactly this when
    /// a component renders with par_iter); that used to deadlock.
    #[test]
    fn image_make_can_reenter() {
        let a = SharedAssets::new(Arc::new(FontStore::new()));
        let outer = a.image(1, 1, 1, &mut || {
            let inner = a.image(2, 1, 1, &mut || vec![255; 4]);
            assert_ne!(inner, ImageId(u32::MAX));
            vec![0; 4]
        });
        assert_ne!(outer, a.image(2, 1, 1, &mut || unreachable!()));
        assert_eq!(outer, a.image(1, 1, 1, &mut || unreachable!()));
    }
}
