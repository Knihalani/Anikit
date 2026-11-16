//! Shared, read-only assets: decoded images and font faces.
//!
//! Built once (by `eval`/`text` preprocessing), wrapped in an `Arc`, and
//! shared by every render worker.

use std::collections::HashMap;
use std::sync::Arc;

use vello_cpu::peniko::{Blob, FontData};
use vello_cpu::Pixmap;

use crate::dl::{FontId, ImageId};

/// Decoded images, premultiplied RGBA8 in vello's pixmap format.
#[derive(Default, Debug)]
pub struct ImageStore {
    images: HashMap<ImageId, Arc<Pixmap>>,
}

/// Error decoding an image.
#[derive(Debug)]
pub struct ImageError(pub String);

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ImageError {}

impl ImageStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode PNG / JPEG / WebP bytes and store them under `id`.
    pub fn insert_encoded(&mut self, id: ImageId, bytes: &[u8]) -> Result<(), ImageError> {
        let img = image::load_from_memory(bytes).map_err(|e| ImageError(e.to_string()))?;
        let rgba = img.into_rgba8();
        let (w, h) = rgba.dimensions();
        self.insert_rgba8(id, w, h, rgba.into_raw())
    }

    /// Store straight-alpha RGBA8 pixels (`w * h * 4` bytes) under `id`.
    pub fn insert_rgba8(&mut self, id: ImageId, w: u32, h: u32, mut rgba: Vec<u8>) -> Result<(), ImageError> {
        if w == 0 || h == 0 || w > u16::MAX as u32 || h > u16::MAX as u32 {
            return Err(ImageError(format!("image size {w}x{h} out of range")));
        }
        if rgba.len() != (w * h * 4) as usize {
            return Err(ImageError("pixel buffer size does not match dimensions".into()));
        }
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
        let mut pm = Pixmap::new(w as u16, h as u16);
        pm.data_as_u8_slice_mut().copy_from_slice(&rgba);
        pm.set_may_have_transparency(!opaque);
        self.images.insert(id, Arc::new(pm));
        Ok(())
    }

    /// Store an already premultiplied pixmap (e.g. a decoded video frame).
    pub fn insert_pixmap(&mut self, id: ImageId, pixmap: Arc<Pixmap>) {
        self.images.insert(id, pixmap);
    }

    pub fn get(&self, id: ImageId) -> Option<&Arc<Pixmap>> {
        self.images.get(&id)
    }
}

/// One drawable face instance: font data plus variation coordinates and synthesis.
#[derive(Clone, Debug)]
pub struct Face {
    /// Font file bytes plus the face index in a collection.
    pub data: FontData,
    /// Normalised variation coordinates (F2Dot14 bits), empty for static fonts / default instance.
    pub coords: Arc<[i16]>,
    /// Synthetic bold (outline expanded by 2% of the em per side).
    pub embolden: bool,
    /// Synthetic oblique angle in degrees (positive slants right).
    pub skew: Option<f32>,
}

/// Font faces keyed by [`FontId`]. Filled by the text system, read by the renderer.
#[derive(Default, Debug, Clone)]
pub struct FontStore {
    fonts: Vec<Option<Face>>,
}

impl FontStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a face (TTF/OTF bytes, face `index` in a collection) at its default instance; returns its id.
    pub fn add(&mut self, bytes: Arc<Vec<u8>>, index: u32) -> FontId {
        let id = FontId(self.fonts.len() as u32);
        self.insert(id, FontData::new(Blob::new(bytes), index));
        id
    }

    /// Store a face's default instance under a caller-chosen id.
    pub fn insert(&mut self, id: FontId, font: FontData) {
        self.insert_face(id, Face { data: font, coords: Arc::from([]), embolden: false, skew: None });
    }

    /// Store a face instance (variation coordinates, synthetic bold / oblique) under `id`.
    pub fn insert_face(&mut self, id: FontId, face: Face) {
        let i = id.0 as usize;
        if self.fonts.len() <= i {
            self.fonts.resize(i + 1, None);
        }
        self.fonts[i] = Some(face);
    }

    pub fn face(&self, id: FontId) -> Option<&Face> {
        self.fonts.get(id.0 as usize).and_then(|f| f.as_ref())
    }

    pub fn get(&self, id: FontId) -> Option<&FontData> {
        self.face(id).map(|f| &f.data)
    }
}

/// Everything a [`super::Renderer`] may reference: shared read-only across threads.
#[derive(Default, Debug)]
pub struct Assets {
    pub images: ImageStore,
    pub fonts: FontStore,
}
