//! The font store: every face the video can draw, keyed by [`FontId`].
//!
//! Loaded once (bundled fonts, then optional system and user fonts) and shared
//! read-only across render threads. Layout (parley) runs at preprocessing time
//! behind a mutex; each distinct *face instance* it picks (font data + variable
//! font coordinates + synthesis) is interned as one [`FontId`], so a display
//! list's `Op::Glyphs { font, .. }` fully determines how to draw the glyphs.
//!
//! **Render contract:** `fonts.face(id)` returns a [`Face`]; draw with
//! `ctx.glyph_run(&mut res, &face.data).font_size(size).normalized_coords(&face.coords)`,
//! plus `font_embolden` when `face.embolden` and a horizontal skew
//! `glyph_transform` when `face.skew` is set.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use parley::fontique::{Blob, Collection, CollectionOptions, FamilyId, GenericFamily, SourceCache};
use parley::{FontContext, LayoutContext};
pub use vello_cpu::peniko::FontData;

use crate::dl::{FontId, Rgba};

/// One drawable face instance.
#[derive(Clone, Debug)]
pub struct Face {
    /// Font file bytes plus the index of the face in a collection.
    pub data: FontData,
    /// Normalised variation coordinates (F2Dot14 as `i16`), empty for static fonts.
    pub coords: Box<[i16]>,
    /// Synthetic bold requested by font matching.
    pub embolden: bool,
    /// Synthetic oblique angle in degrees, if requested.
    pub skew: Option<f32>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FaceKey {
    blob: u64,
    index: u32,
    coords: Box<[i16]>,
    embolden: bool,
    skew: i32,
}

/// Bundled faces (see `fonts/LICENSE-*`): Inter (variable, with the `opsz`
/// axis that gives Inter Display), JetBrains Mono (variable), Instrument Serif
/// Twemoji (COLR emoji), and Noto Sans Arabic / Hebrew as script fallbacks.
pub mod bundled {
    pub const INTER: &[u8] = include_bytes!("../../fonts/InterVariable.ttf");
    pub const INTER_ITALIC: &[u8] = include_bytes!("../../fonts/InterVariable-Italic.ttf");
    pub const JETBRAINS_MONO: &[u8] = include_bytes!("../../fonts/JetBrainsMono-Variable.ttf");
    pub const JETBRAINS_MONO_ITALIC: &[u8] = include_bytes!("../../fonts/JetBrainsMono-Italic-Variable.ttf");
    pub const INSTRUMENT_SERIF: &[u8] = include_bytes!("../../fonts/InstrumentSerif-Regular.ttf");
    pub const INSTRUMENT_SERIF_ITALIC: &[u8] = include_bytes!("../../fonts/InstrumentSerif-Italic.ttf");
    pub const TWEMOJI: &[u8] = include_bytes!("../../fonts/Twemoji.ttf");
    pub const NOTO_ARABIC: &[u8] = include_bytes!("../../fonts/NotoSansArabic.ttf");
    pub const NOTO_HEBREW: &[u8] = include_bytes!("../../fonts/NotoSansHebrew.ttf");
}

pub(crate) struct Shaper {
    pub fcx: FontContext,
    pub lcx: LayoutContext<Rgba>,
    /// Lower-cased family name (and aliases) → canonical family name.
    pub families: HashMap<String, String>,
    pub emoji: Option<String>,
    /// Script fallbacks appended after every family list (Arabic, Hebrew).
    pub fallbacks: Vec<String>,
}

/// Fonts for one render: loaded once, shared read-only (`Arc<FontStore>`).
pub struct FontStore {
    paths: Mutex<HashMap<(u32, u16, u32), Arc<kurbo::BezPath>>>,
    files: Mutex<HashMap<std::path::PathBuf, Vec<String>>>,
    pub(crate) shaper: Mutex<Shaper>,
    faces: RwLock<Vec<Arc<Face>>>,
    index: Mutex<HashMap<FaceKey, FontId>>,
    pub(crate) cache: RwLock<HashMap<u64, Arc<super::layout::TextLayout>>>,
}

impl Default for FontStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FontStore {
    /// Bundled fonts only: deterministic output on every machine.
    pub fn new() -> Self {
        Self::build(false)
    }

    /// Bundled fonts first, then the system's fonts as fallbacks (fontique).
    pub fn with_system_fonts() -> Self {
        Self::build(true)
    }

    fn build(system: bool) -> Self {
        let collection = Collection::new(CollectionOptions { shared: false, system_fonts: system });
        let fcx = FontContext { collection, source_cache: SourceCache::default() };
        let store = FontStore {
            paths: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
            shaper: Mutex::new(Shaper { fcx, lcx: LayoutContext::new(), families: HashMap::new(), emoji: None, fallbacks: Vec::new() }),
            faces: RwLock::new(Vec::new()),
            index: Mutex::new(HashMap::new()),
            cache: RwLock::new(HashMap::new()),
        };
        let sans = store.add_blob(Blob::new(Arc::new(bundled::INTER)));
        store.add_blob(Blob::new(Arc::new(bundled::INTER_ITALIC)));
        let mono = store.add_blob(Blob::new(Arc::new(bundled::JETBRAINS_MONO)));
        store.add_blob(Blob::new(Arc::new(bundled::JETBRAINS_MONO_ITALIC)));
        let serif = store.add_blob(Blob::new(Arc::new(bundled::INSTRUMENT_SERIF)));
        store.add_blob(Blob::new(Arc::new(bundled::INSTRUMENT_SERIF_ITALIC)));
        let emoji = store.add_blob(Blob::new(Arc::new(bundled::TWEMOJI)));
        let mut fallbacks = store.add_blob(Blob::new(Arc::new(bundled::NOTO_ARABIC)));
        fallbacks.extend(store.add_blob(Blob::new(Arc::new(bundled::NOTO_HEBREW))));
        {
            let mut s = store.shaper.lock().unwrap();
            let s = &mut *s;
            let fid = |c: &mut Collection, names: &[String]| -> Vec<FamilyId> {
                names.iter().filter_map(|n| c.family_id(n)).collect()
            };
            let ids = fid(&mut s.fcx.collection, &sans);
            s.fcx.collection.set_generic_families(GenericFamily::SansSerif, ids.iter().copied());
            s.fcx.collection.set_generic_families(GenericFamily::SystemUi, ids.iter().copied());
            s.fcx.collection.set_generic_families(GenericFamily::UiSansSerif, ids.into_iter());
            let ids = fid(&mut s.fcx.collection, &mono);
            s.fcx.collection.set_generic_families(GenericFamily::Monospace, ids.iter().copied());
            s.fcx.collection.set_generic_families(GenericFamily::UiMonospace, ids.into_iter());
            let ids = fid(&mut s.fcx.collection, &serif);
            s.fcx.collection.set_generic_families(GenericFamily::Serif, ids.iter().copied());
            s.fcx.collection.set_generic_families(GenericFamily::UiSerif, ids.into_iter());
            let ids = fid(&mut s.fcx.collection, &emoji);
            s.fcx.collection.set_generic_families(GenericFamily::Emoji, ids.into_iter());
            s.emoji = emoji.first().cloned();
            s.fallbacks = fallbacks.clone();
            // Friendly aliases.
            for (alias, target) in [
                ("inter", sans.first()),
                ("inter display", sans.first()),
                ("sans", sans.first()),
                ("jetbrains mono", mono.first()),
                ("mono", mono.first()),
                ("code", mono.first()),
                ("instrument serif", serif.first()),
                ("emoji", emoji.first()),
            ] {
                if let Some(t) = target {
                    s.families.entry(alias.into()).or_insert_with(|| t.clone());
                }
            }
        }
        store
    }

    /// Register font bytes (TTF/OTF/TTC); returns the family names it added.
    pub fn add_font_bytes(&self, bytes: Vec<u8>) -> Vec<String> {
        self.add_blob(Blob::new(Arc::new(bytes)))
    }

    fn add_blob(&self, blob: Blob<u8>) -> Vec<String> {
        let mut s = self.shaper.lock().unwrap();
        let s = &mut *s;
        let added = s.fcx.collection.register_fonts(blob, None);
        let mut names = Vec::new();
        for (fam, _) in added {
            if let Some(n) = s.fcx.collection.family_name(fam) {
                let n = n.to_string();
                s.families.entry(n.to_lowercase()).or_insert_with(|| n.clone());
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        // New fonts can change any layout.
        self.cache.write().unwrap().clear();
        names
    }

    /// Make a catalogue family (see [`super::library`]) available, fetching
    /// it on first use. `Ok(false)`: already known or not a library family.
    pub fn ensure_family(&self, name: &str) -> Result<bool, String> {
        let mut s = self.shaper.lock().unwrap();
        let r = load_library_locked(&mut s, name);
        drop(s);
        if r == Ok(true) {
            self.cache.write().unwrap().clear();
        }
        r
    }

    /// Register a font file from disk (once per path; again returns its families).
    pub fn add_font_file(&self, path: impl AsRef<Path>) -> std::io::Result<Vec<String>> {
        let path = path.as_ref().to_path_buf();
        if let Some(n) = self.files.lock().unwrap().get(&path) {
            return Ok(n.clone());
        }
        let names = self.add_font_bytes(std::fs::read(&path)?);
        self.files.lock().unwrap().insert(path, names.clone());
        Ok(names)
    }

    /// Make `alias` (case-insensitive) name the family `target`.
    pub fn add_alias(&self, alias: &str, target: &str) {
        let mut s = self.shaper.lock().unwrap();
        s.families.insert(alias.trim().to_lowercase(), target.to_string());
        drop(s);
        self.cache.write().unwrap().clear();
    }

    /// Canonical family name for a user-supplied name or alias (case-insensitive).
    pub fn resolve_family(&self, name: &str) -> Option<String> {
        let s = self.shaper.lock().unwrap();
        s.families.get(&name.trim().to_lowercase()).cloned()
    }

    /// All known family names (bundled, user and, if loaded, system).
    pub fn family_names(&self) -> Vec<String> {
        let mut s = self.shaper.lock().unwrap();
        let mut v: Vec<String> = s.fcx.collection.family_names().map(str::to_string).collect();
        v.sort();
        v.dedup();
        v
    }

    /// Intern a face instance, returning its stable id.
    pub fn intern(&self, data: &FontData, coords: &[i16], embolden: bool, skew: Option<f32>) -> FontId {
        let key = FaceKey {
            blob: data.data.id(),
            index: data.index,
            coords: coords.into(),
            embolden,
            skew: skew.map_or(i32::MIN, |s| (s * 100.0) as i32),
        };
        let mut idx = self.index.lock().unwrap();
        if let Some(id) = idx.get(&key) {
            return *id;
        }
        let mut faces = self.faces.write().unwrap();
        let id = FontId(faces.len() as u32);
        faces.push(Arc::new(Face { data: data.clone(), coords: coords.into(), embolden, skew }));
        idx.insert(key, id);
        id
    }

    /// The face for an id from a display list. Panics on an id from another store.
    pub fn face(&self, id: FontId) -> Arc<Face> {
        self.faces.read().unwrap()[id.0 as usize].clone()
    }

    /// Number of interned faces.
    pub fn face_count(&self) -> usize {
        self.faces.read().unwrap().len()
    }

    /// Glyph id for `ch` in a face (0 if missing).
    pub fn glyph_for_char(&self, id: FontId, ch: char) -> u16 {
        use skrifa::MetadataProvider;
        let face = self.face(id);
        skrifa::FontRef::from_index(face.data.data.as_ref(), face.data.index)
            .ok()
            .and_then(|f| f.charmap().map(ch))
            .map_or(0, |g| g.to_u32() as u16)
    }

    /// The default value of variable axis `tag` in a face (`None` if it has no such axis).
    pub fn axis_default(&self, id: FontId, tag: &str) -> Option<f32> {
        use skrifa::MetadataProvider;
        let face = self.face(id);
        let f = skrifa::FontRef::from_index(face.data.data.as_ref(), face.data.index).ok()?;
        let t = tag.as_bytes();
        let ax = f.axes().iter().find(|a| a.tag().to_be_bytes() == [t[0], t[1], t[2], t[3]])?;
        Some(ax.default_value())
    }

    /// The face `id` moved a fraction `t` (0..1) toward the axis values
    /// `targets` (user units), interpolated in normalised space and quantised
    /// to 1/128 so a sweep reuses a bounded set of instances.
    pub fn toward_axes(&self, id: FontId, targets: &[(String, f32)], t: f32) -> FontId {
        use skrifa::MetadataProvider;
        let face = self.face(id);
        let Ok(f) = skrifa::FontRef::from_index(face.data.data.as_ref(), face.data.index) else { return id };
        let axes = f.axes();
        if axes.is_empty() {
            return id;
        }
        let mut coords: Vec<i16> = (0..axes.len()).map(|i| face.coords.get(i).copied().unwrap_or(0)).collect();
        for (tag, target) in targets {
            let tb = tag.as_bytes();
            if let Some(ax) = axes.iter().find(|a| a.tag().to_be_bytes() == [tb[0], tb[1], tb[2], tb[3]]) {
                let i = ax.index();
                let to = ax.normalize(*target).to_bits() as f32;
                let v = coords[i] as f32 + (to - coords[i] as f32) * t.clamp(0.0, 1.0);
                coords[i] = ((v / 128.0).round() * 128.0).clamp(-16384.0, 16384.0) as i16;
            }
        }
        self.intern(&face.data, &coords, face.embolden, face.skew)
    }

    /// Horizontal advance of glyph `gid` at `size` px (variation-aware).
    pub fn advance(&self, id: FontId, gid: u16, size: f32) -> f32 {
        use skrifa::MetadataProvider;
        let face = self.face(id);
        let Ok(f) = skrifa::FontRef::from_index(face.data.data.as_ref(), face.data.index) else { return 0.0 };
        let coords: Vec<skrifa::instance::NormalizedCoord> =
            face.coords.iter().map(|c| skrifa::instance::NormalizedCoord::from_bits(*c)).collect();
        let loc = skrifa::instance::LocationRef::new(&coords);
        f.glyph_metrics(skrifa::instance::Size::new(size), loc)
            .advance_width(skrifa::GlyphId::new(gid as u32))
            .unwrap_or(0.0)
    }
}

/// Register a library family into a locked shaper (no-op if already known).
pub(crate) fn load_library_locked(s: &mut Shaper, name: &str) -> Result<bool, String> {
    let lower = name.trim().to_lowercase();
    if s.families.contains_key(&lower) {
        return Ok(false);
    }
    let Some(fam) = super::library::find(name).or_else(|| super::library::find_loose(name)) else { return Ok(false) };
    if fam.bundled || s.families.contains_key(&fam.name.to_lowercase()) {
        if let Some(t) = s.families.get(&fam.name.to_lowercase()).cloned() {
            s.families.insert(lower, t);
        }
        return Ok(false);
    }
    let mut first = None;
    for bytes in fam.load()? {
        for (id, _) in s.fcx.collection.register_fonts(Blob::new(Arc::new(bytes)), None) {
            if let Some(n) = s.fcx.collection.family_name(id) {
                let n = n.to_string();
                first.get_or_insert_with(|| n.clone());
                s.families.entry(n.to_lowercase()).or_insert(n);
            }
        }
    }
    let target = first.ok_or_else(|| format!("font '{}': the files contain no usable face", fam.name))?;
    s.families.entry(fam.name.to_lowercase()).or_insert_with(|| target.clone());
    s.families.entry(lower).or_insert(target);
    Ok(true)
}

struct PathPen<'a>(&'a mut kurbo::BezPath);

impl skrifa::outline::OutlinePen for PathPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x as f64, -y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x as f64, -y as f64));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.quad_to((cx as f64, -cy as f64), (x as f64, -y as f64));
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.0.curve_to((a as f64, -b as f64), (c as f64, -d as f64), (x as f64, -y as f64));
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

impl FontStore {
    /// A glyph's outline at `size` px, y-down, origin on the pen position at
    /// the baseline (cached). Empty for bitmap/COLR-only glyphs.
    pub fn glyph_path(&self, id: FontId, gid: u16, size: f32) -> Arc<kurbo::BezPath> {
        use skrifa::MetadataProvider;
        let key = (id.0, gid, size.to_bits());
        if let Some(p) = self.paths.lock().unwrap().get(&key) {
            return p.clone();
        }
        let face = self.face(id);
        let mut path = kurbo::BezPath::new();
        if let Ok(f) = skrifa::FontRef::from_index(face.data.data.as_ref(), face.data.index) {
            let coords: Vec<skrifa::instance::NormalizedCoord> =
                face.coords.iter().map(|c| skrifa::instance::NormalizedCoord::from_bits(*c)).collect();
            if let Some(g) = f.outline_glyphs().get(skrifa::GlyphId::new(gid as u32)) {
                let settings = skrifa::outline::DrawSettings::unhinted(
                    skrifa::instance::Size::new(size),
                    skrifa::instance::LocationRef::new(&coords),
                );
                let _ = g.draw(settings, &mut PathPen(&mut path));
            }
        }
        if let Some(s) = face.skew {
            let t = (s as f64).to_radians().tan();
            path.apply_affine(kurbo::Affine::new([1.0, 0.0, -t, 1.0, 0.0, 0.0]));
        }
        let p = Arc::new(path);
        self.paths.lock().unwrap().insert(key, p.clone());
        p
    }
}
