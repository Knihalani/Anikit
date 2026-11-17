//! Optional, larger sample packs (not bundled): downloaded on request into a
//! cache directory, then usable by name. When a pack is not in the cache,
//! [`pack_instrument`] returns `None` so callers fall back to the GM bank.

use super::sfz::{SfzInstrument, SfzOptions};
use crate::voice::Instrument;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

/// A downloadable pack.
#[derive(Debug, Clone, Copy)]
pub struct Pack {
    /// Name (also the directory name in the cache).
    pub name: &'static str,
    /// What it contains.
    pub description: &'static str,
    /// Licence.
    pub license: &'static str,
    /// Approximate download size.
    pub size: &'static str,
    /// Git repository.
    pub url: &'static str,
    /// Branch to check out.
    pub branch: &'static str,
    /// Credit line that must accompany published work using the pack (empty if none required).
    pub credit: &'static str,
}

/// Known packs.
pub const PACKS: &[Pack] = &[
    Pack {
        name: "splendid",
        description: "Splendid Grand Piano — Akai Steinway samples, 4 velocity layers, SFZ + FLAC",
        license: "Public domain (released by Akai, 2000)",
        size: "≈150 MB",
        url: "https://github.com/sfzinstruments/SplendidGrandPiano",
        branch: "master",
        credit: "",
    },
    Pack {
        name: "epianos",
        description: "Greg Sullivan E-Pianos — Wurlitzer EP200, Yamaha CP80, Hohner Pianet T (SFZ + FLAC)",
        license: "CC BY 3.0",
        size: "≈40 MB",
        url: "https://github.com/sfzinstruments/GregSullivan.E-Pianos",
        branch: "master",
        credit: "E-Pianos by Greg Sullivan (CC BY 3.0), SFZ mapping by kinwie",
    },
    Pack {
        name: "vsco2-ce",
        description: "VS Chamber Orchestra 2: Community Edition — orchestral strings (sections and solo), \
                      woodwinds, brass, harp, timpani and orchestral percussion, keys and organ (SFZ, WAV)",
        license: "CC0 1.0 (public domain dedication)",
        size: "≈3,200 WAV files, a few GB",
        url: "https://github.com/sgossner/VSCO-2-CE",
        branch: "SFZ",
        credit: "",
    },
    Pack {
        name: "salamander",
        description: "Salamander Grand Piano V3 — Yamaha C5, 16 velocity layers, 48 kHz/24-bit, \
                      release and string-resonance layers (SFZ, FLAC)",
        license: "CC BY 3.0",
        size: "≈640 FLAC files, ≈1.2 GB",
        url: "https://github.com/sfzinstruments/SalamanderGrandPiano",
        branch: "master",
        credit: "Salamander Grand Piano V3 by Alexander Holm (CC BY 3.0), SFZ mapping by kinwie",
    },
];

/// Friendly name → (pack, SFZ file inside it).
pub const INSTRUMENTS: &[(&str, &str, &str)] = &[
    ("splendid-piano", "splendid", "Splendid Grand Piano.sfz"),
    ("wurlitzer", "epianos", "Wurlitzer EP200/Wurlitzer EP200.sfz"),
    ("cp80", "epianos", "CP80/CP80.sfz"),
    ("pianet", "epianos", "Pianet T/Pianet T.sfz"),
    ("salamander-piano", "salamander", "Salamander Grand Piano V3.sfz"),
    ("salamander", "salamander", "Salamander Grand Piano V3.sfz"),
    ("vsco-violins", "vsco2-ce", "ViolinEnsSusVib.sfz"),
    ("vsco-violins-pizz", "vsco2-ce", "ViolinEnsPizz.sfz"),
    ("vsco-violins-spiccato", "vsco2-ce", "ViolinEnsSpic.sfz"),
    ("vsco-violins-tremolo", "vsco2-ce", "ViolinEnsTrem.sfz"),
    ("vsco-violin", "vsco2-ce", "SViolinVib.sfz"),
    ("vsco-violin-pizz", "vsco2-ce", "SViolinPizz.sfz"),
    ("vsco-violas", "vsco2-ce", "ViolaEnsSusVib.sfz"),
    ("vsco-violas-pizz", "vsco2-ce", "ViolaEnsPizz.sfz"),
    ("vsco-cello", "vsco2-ce", "CelloEnsSusVib.sfz"),
    ("vsco-cellos", "vsco2-ce", "CelloEnsSusVib.sfz"),
    ("vsco-cello-pizz", "vsco2-ce", "CelloEnsPizz.sfz"),
    ("vsco-contrabass", "vsco2-ce", "ContrabassSusVB.sfz"),
    ("vsco-contrabass-pizz", "vsco2-ce", "ContrabassPizz.sfz"),
    ("vsco-harp", "vsco2-ce", "Harp.sfz"),
    ("vsco-horn", "vsco2-ce", "FHornSus.sfz"),
    ("vsco-horn-stac", "vsco2-ce", "FHornStac.sfz"),
    ("vsco-trumpet", "vsco2-ce", "TrumpetSus.sfz"),
    ("vsco-trombone", "vsco2-ce", "TromboneSus.sfz"),
    ("vsco-tuba", "vsco2-ce", "TubaSus.sfz"),
    ("vsco-flute", "vsco2-ce", "FluteSusVib.sfz"),
    ("vsco-piccolo", "vsco2-ce", "PiccoloSus.sfz"),
    ("vsco-oboe", "vsco2-ce", "OboeSusVib.sfz"),
    ("vsco-clarinet", "vsco2-ce", "ClarinetSus.sfz"),
    ("vsco-bassoon", "vsco2-ce", "BassoonSus.sfz"),
    ("vsco-timpani", "vsco2-ce", "Timpani.sfz"),
    ("vsco-timpani-roll", "vsco2-ce", "TimpaniRolls.sfz"),
    ("vsco-glockenspiel", "vsco2-ce", "Glockenspiel.sfz"),
    ("vsco-xylophone", "vsco2-ce", "Xylophone.sfz"),
    ("vsco-marimba", "vsco2-ce", "Marimba.sfz"),
    ("vsco-tubular-bells", "vsco2-ce", "TubularBells.sfz"),
    ("vsco-organ", "vsco2-ce", "OrganLoud.sfz"),
    ("vsco-upright-piano", "vsco2-ce", "UprightPiano.sfz"),
    ("vsco-percussion", "vsco2-ce", "GM-StylePerc.sfz"),
];

/// Names [`pack_instrument`] understands.
pub fn instrument_names() -> Vec<&'static str> {
    INSTRUMENTS.iter().map(|(n, _, _)| *n).collect()
}

/// Cache directory: `$ANIKIT_MUSIC_CACHE`, else `$XDG_CACHE_HOME/anikit/music`,
/// else `~/.cache/anikit/music`.
pub fn cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("ANIKIT_MUSIC_CACHE").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_CACHE_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("anikit").join("music");
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".cache").join("anikit").join("music")
}

/// Where a pack lives (whether or not it has been fetched).
pub fn pack_dir(name: &str) -> PathBuf {
    cache_dir().join(name)
}

/// Whether a pack is in the cache.
pub fn is_cached(name: &str) -> bool {
    pack_dir(name).join(".git").exists() || pack_dir(name).join(".anikit-complete").exists()
}

/// The small packs worth having for everyday scores (≈190 MB): a better grand
/// piano and vintage electric pianos. `fetch("essentials")` gets them all.
pub const ESSENTIALS: &[&str] = &["splendid", "epianos"];

/// Download a pack into the cache (`git clone --depth 1`); returns its
/// directory. Does nothing if it is already there.
pub fn fetch(name: &str) -> Result<PathBuf, String> {
    if name == "essentials" {
        let mut last = cache_dir();
        for p in ESSENTIALS {
            last = fetch(p)?;
        }
        return Ok(last.parent().map(PathBuf::from).unwrap_or(last));
    }
    let pack = PACKS
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("unknown pack {name:?}; known: {}", PACKS.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")))?;
    let dir = pack_dir(name);
    if is_cached(name) {
        return Ok(dir);
    }
    std::fs::create_dir_all(cache_dir()).map_err(|e| format!("cannot create {}: {e}", cache_dir().display()))?;
    let tmp = cache_dir().join(format!(".{name}.partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let st = Command::new("git")
        .args(["clone", "--depth", "1", "--single-branch", "--branch", pack.branch, pack.url])
        .arg(&tmp)
        .status()
        .map_err(|e| format!("cannot run git (is it installed?): {e}"))?;
    if !st.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("git clone of {} failed ({st})", pack.url));
    }
    std::fs::rename(&tmp, &dir).map_err(|e| format!("cannot move pack into place: {e}"))?;
    let _ = std::fs::write(dir.join(".anikit-complete"), pack.license);
    Ok(dir)
}

/// An instrument from a cached pack by friendly name (see
/// [`instrument_names`]), or a path inside a pack such as
/// `vsco2-ce/FluteStac.sfz`. `None` if the pack is not cached or the name
/// is unknown, so callers can fall back to the GM bank.
pub fn pack_instrument(name: &str) -> Option<Arc<dyn Instrument>> {
    let n = name.trim().to_lowercase().replace([' ', '_'], "-");
    let (pack, file) = match INSTRUMENTS.iter().find(|(k, _, _)| *k == n) {
        Some(&(_, p, f)) => (p.to_string(), f.to_string()),
        None => {
            let (p, f) = name.trim().split_once('/')?;
            PACKS.iter().find(|k| k.name == p)?;
            (p.to_string(), f.to_string())
        }
    };
    let path = pack_dir(&pack).join(&file);
    if !path.is_file() {
        return None;
    }
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<dyn Instrument>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(i) = cache.lock().ok()?.get(&path) {
        return Some(i.clone());
    }
    // Lenient: a sparse or partial checkout still plays what it has.
    let inst: Arc<dyn Instrument> = Arc::new(SfzInstrument::load_with(&path, SfzOptions { skip_missing: true }).ok()?);
    cache.lock().ok()?.insert(path, inst.clone());
    Some(inst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_packs_table_is_consistent() {
        for (n, p, f) in INSTRUMENTS {
            assert!(PACKS.iter().any(|k| k.name == *p), "{n}: unknown pack {p}");
            assert!(f.ends_with(".sfz"));
        }
        assert!(fetch("no-such-pack").is_err());
        assert!(pack_instrument("not-an-instrument").is_none());
    }

    /// Run with `ANIKIT_MUSIC_CACHE=<dir with vsco2-ce and salamander>`.
    #[test]
    #[ignore]
    fn sf2_packs_load_cached_instruments() {
        use crate::sf2::tests::{measure_freq, note, render};
        for name in ["vsco-violins", "vsco-timpani", "salamander-piano"] {
            let Some(inst) = pack_instrument(name) else {
                eprintln!("{name}: pack not cached, skipped");
                continue;
            };
            let key = if name.contains("timpani") { 43.0 } else { 69.0 };
            let mut v = inst.voice(&note(key, 0.7));
            let (l, r, _) = render(v.as_mut(), 2.0, 1.5, 128);
            let m: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a + b).collect();
            let peak = m.iter().fold(0f32, |a, v| a.max(v.abs()));
            assert!(peak > 0.01 && peak < 4.0, "{name}: peak {peak}");
            if key == 69.0 {
                let f = measure_freq(&m[24_000..48_000], 440.0, 48_000.0);
                eprintln!("{name}: A4 → {f:.2} Hz, peak {peak:.3}");
                assert!((1200.0 * (f / 440.0).log2()).abs() < 15.0, "{name}: {f}");
            } else {
                eprintln!("{name}: peak {peak:.3}");
            }
        }
    }
}
