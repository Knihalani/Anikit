//! A key over the files a spec reads, for the render cache.
//!
//! The chunk cache keys each frame by its evaluated display list, which
//! identifies images by their pixels and shaders by their source, but not
//! everything read from disk shows up there: a font is an id in a glyph run,
//! and a LUT, HDRI, glTF or texture is named by its path. So the cache also
//! keys on the contents of every file the spec names: any string in the spec
//! that resolves to a file next to it (or an absolute path). Spec files it
//! names (external compositions) are read the same way, relative to their own
//! folder. Changing any of those files changes every chunk's key.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Files hashed whole up to this size; bigger ones by size, mtime and their
/// first and last MiB (video and audio files, where a full hash would cost
/// more than the cache saves).
const FULL: u64 = 32 << 20;
const EDGE: usize = 1 << 20;

/// Key over the contents of every file `spec` names, resolved against `base`.
pub(crate) fn inputs_key(spec: &serde_json::Value, base: &Path) -> u64 {
    let mut files = BTreeSet::new();
    collect(spec, base, &mut files, 0);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "anikit-inputs-1".hash(&mut h);
    for f in &files {
        f.hash(&mut h);
        hash_file(f, &mut h);
    }
    h.finish()
}

fn collect(v: &serde_json::Value, base: &Path, out: &mut BTreeSet<PathBuf>, depth: usize) {
    match v {
        serde_json::Value::String(s) => {
            // Paths only: not expressions, colours, prose or URLs.
            if s.is_empty() || s.len() > 1024 || s.starts_with('=') || s.starts_with('#') || s.contains('\n') || s.contains("://") {
                return;
            }
            let p = base.join(s);
            if !p.is_file() {
                return;
            }
            let p = p.canonicalize().unwrap_or(p);
            if !out.insert(p.clone()) {
                return;
            }
            // A spec it names (a composition file): its own references too.
            let low = s.to_ascii_lowercase();
            if depth < 4 && (low.ends_with(".json") || low.ends_with(".yaml") || low.ends_with(".yml")) {
                if let Ok(text) = std::fs::read_to_string(&p) {
                    let parsed = if low.ends_with(".json") { serde_json::from_str(&text).ok() } else { serde_norway::from_str(&text).ok() };
                    if let Some(sub) = parsed {
                        collect(&sub, p.parent().unwrap_or(base), out, depth + 1);
                    }
                }
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect(x, base, out, depth)),
        serde_json::Value::Object(m) => m.values().for_each(|x| collect(x, base, out, depth)),
        _ => {}
    }
}

fn hash_file(p: &Path, h: &mut impl Hasher) {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(meta) = std::fs::metadata(p) else { return };
    meta.len().hash(h);
    let Ok(mut f) = std::fs::File::open(p) else { return };
    if meta.len() <= FULL {
        let mut buf = Vec::with_capacity(meta.len() as usize);
        if f.read_to_end(&mut buf).is_ok() {
            buf.hash(h);
        }
        return;
    }
    meta.modified().ok().hash(h);
    let mut buf = vec![0u8; EDGE];
    if f.read_exact(&mut buf).is_ok() {
        buf.hash(h);
    }
    if f.seek(SeekFrom::End(-(EDGE as i64))).is_ok() && f.read_exact(&mut buf).is_ok() {
        buf.hash(h);
    }
}

#[cfg(test)]
mod tests {
    use super::inputs_key;
    use serde_json::json;

    #[test]
    fn key_follows_file_contents_and_nested_specs() {
        let dir = std::env::temp_dir().join(format!("anikit-inputs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.cube"), "LUT 1").unwrap();
        std::fs::write(dir.join("sub/tex.png"), "png 1").unwrap();
        std::fs::write(dir.join("comp.json"), r#"{"layers": [{"image": "sub/tex.png"}]}"#).unwrap();
        let spec = json!({"post": {"lut": "a.cube"}, "compositions": {"c": {"src": "comp.json"}}, "text": "a.cube is a file", "color": "#fff"});
        let k0 = inputs_key(&spec, &dir);
        assert_eq!(k0, inputs_key(&spec, &dir), "stable");
        std::fs::write(dir.join("a.cube"), "LUT 2").unwrap();
        let k1 = inputs_key(&spec, &dir);
        assert_ne!(k0, k1, "a referenced file changed");
        // A file named only inside a referenced spec, relative to that spec.
        std::fs::write(dir.join("sub/tex.png"), "png 2").unwrap();
        assert_ne!(k1, inputs_key(&spec, &dir), "a nested spec's file changed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
