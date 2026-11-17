//! Instrument names → instruments.
//!
//! Resolution order for a plain name: optional packs in the cache
//! (`salamander-piano`, `vsco-*`), physical models, the synth palette, the
//! bundled General MIDI bank. Prefixes force a source: `phys:`, `synth:`,
//! `gm:` (`gm:piano`, `gm:24`), `drums:` (synth kits, then GM kits), and
//! files: `path.sfz`, `path.sf2#program` / `path.sf3#bank:program`.
//! `a|b|c` tries each in turn.

use crate::voice::Instrument;
use std::path::Path;
use std::sync::Arc;

/// Resolve an instrument name (see the module docs).
pub fn resolve(name: &str, base: Option<&Path>) -> Result<Arc<dyn Instrument>, String> {
    let mut errs = Vec::new();
    for n in name.split('|').map(str::trim).filter(|n| !n.is_empty()) {
        match resolve_one(n, base) {
            Ok(i) => return Ok(i),
            Err(e) => errs.push(e),
        }
    }
    Err(errs.pop().unwrap_or_else(|| format!("instrument '{name}': empty name")))
}

fn resolve_one(name: &str, base: Option<&Path>) -> Result<Arc<dyn Instrument>, String> {
    let _ = base;
    Err(format!("instrument '{name}': unknown"))
}

/// All built-in instrument names, by source (for docs and suggestions).
pub fn names() -> Vec<(&'static str, Vec<&'static str>)> {
    Vec::new()
}
