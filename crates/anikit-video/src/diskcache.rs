//! A small on-disk cache for pure, expensive results derived from input files
//! (beat analysis, visualizer tables), so repeat renders and stills of the same
//! project skip the work: agents re-render a spec many times while editing it.
//!
//! Entries are keyed by a hash of a kind tag, the file's path, size and
//! modification time, and any parameters, so a changed file is never served
//! stale. Directory: `$ANIKIT_CACHE_DIR`, else `$XDG_CACHE_HOME/anikit`, else
//! `~/.cache/anikit`. `ANIKIT_CACHE=0` turns it off. Failures are silent: the
//! cache is only ever an optimisation.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

fn dir() -> Option<PathBuf> {
    if std::env::var_os("ANIKIT_CACHE").is_some_and(|v| v == "0") {
        return None;
    }
    if let Some(d) = std::env::var_os("ANIKIT_CACHE_DIR") {
        return Some(PathBuf::from(d));
    }
    if let Some(d) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(d).join("anikit"));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache").join("anikit"))
}

/// A key for results derived from `file` with `params`; `None` if the file can't be stat'ed.
pub fn key(kind: &str, file: &Path, params: &[u64]) -> Option<String> {
    let m = std::fs::metadata(file).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let path = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (kind, path, m.len(), mtime, params).hash(&mut h);
    Some(format!("{kind}-{:016x}", h.finish()))
}

/// The cached bytes for `key`, if any.
pub fn get(key: &str) -> Option<Vec<u8>> {
    std::fs::read(dir()?.join(key)).ok()
}

/// Store `bytes` under `key` (atomically: write then rename).
pub fn put(key: &str, bytes: &[u8]) {
    let Some(d) = dir() else { return };
    if std::fs::create_dir_all(&d).is_err() {
        return;
    }
    let tmp = d.join(format!("{key}.{}.tmp", std::process::id()));
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(&tmp, d.join(key));
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}
