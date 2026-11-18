//! CC0 sample packs, fetched on demand (curl + unzip into a cache) and used
//! by name: `"kenney-interface/click"` picks one of the `click_00N` variants
//! by seed (round robin, so repeats differ). Only CC0 packs are listed: they
//! may be used, layered and redistributed in rendered work with no credit.
//! Licences checked 2026-11 on the pack pages (see docs/video/craft/sound-design.md §6).
//!
//! `anikit-video sound fetch <pack>` downloads one ahead of time; a cue naming
//! an uncached pack fetches it at render time.

use crate::audio::Pcm;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A downloadable pack.
#[derive(Debug, Clone, Copy)]
pub struct Pack {
    pub name: &'static str,
    pub description: &'static str,
    pub license: &'static str,
    pub url: &'static str,
    pub page: &'static str,
    /// Expected download size in bytes (integrity check).
    pub bytes: u64,
}

pub const PACKS: &[Pack] = &[
    Pack {
        name: "kenney-interface",
        description: "Kenney Interface Sounds: 100 UI sounds (click, tick, select, toggle, switch, confirmation, error, glass, glitch, drop, open, close, maximize, minimize, scroll, scratch, pluck, bong)",
        license: "CC0 1.0",
        url: "https://opengameart.org/sites/default/files/kenney_interfaceSounds.zip",
        page: "https://opengameart.org/content/interface-sounds",
        bytes: 834_536,
    },
    Pack {
        name: "cc0-foley",
        description: "100 CC0 SFX: foley (glass, metal, wooden, paper, dishes, door, slam, hit, spring, splash, plop, pot, tools, switch, bell, gong, machine)",
        license: "CC0 1.0",
        url: "https://opengameart.org/sites/default/files/100-CC0-SFX_0.zip",
        page: "https://opengameart.org/content/100-cc0-sfx",
        bytes: 2_921_904,
    },
    Pack {
        name: "kenney-switches",
        description: "Kenney UI SFX set: 51 switches, clicks, mouse clicks and rollovers",
        license: "CC0 1.0",
        url: "https://opengameart.org/sites/default/files/UI_SFX_Set.zip",
        page: "https://opengameart.org/content/51-ui-sound-effects-buttons-switches-and-clicks",
        bytes: 1_425_902,
    },
];

/// `$ANIKIT_SFX_CACHE`, else `<music cache>/../sfx`.
pub fn cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("ANIKIT_SFX_CACHE").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let m = anikit_music::sf2::packs::cache_dir();
    m.parent()
        .map(|p| p.join("sfx"))
        .unwrap_or_else(|| m.join("sfx"))
}

pub fn pack_dir(name: &str) -> PathBuf {
    cache_dir().join(name)
}

pub fn is_cached(name: &str) -> bool {
    pack_dir(name).join(".anikit-complete").exists()
}

/// Download and unpack a pack (no-op when cached).
pub fn fetch(name: &str) -> Result<PathBuf, String> {
    let pack = PACKS.iter().find(|p| p.name == name).ok_or_else(|| {
        format!(
            "unknown sample pack '{name}'; known: {}",
            PACKS.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")
        )
    })?;
    let dir = pack_dir(name);
    if is_cached(name) {
        return Ok(dir);
    }
    std::fs::create_dir_all(cache_dir())
        .map_err(|e| format!("cannot create {}: {e}", cache_dir().display()))?;
    let tmp = cache_dir().join(format!(".{name}.partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let zip = tmp.join("pack.zip");
    let st = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&zip)
        .arg(pack.url)
        .status()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    let got = std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
    if !st.success() || got != pack.bytes {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!(
            "download of {} failed ({st}, {got} bytes, expected {})",
            pack.url, pack.bytes
        ));
    }
    let st = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(&zip)
        .arg("-d")
        .arg(&tmp)
        .status()
        .map_err(|e| format!("cannot run unzip: {e}"))?;
    if !st.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("unzip of {name} failed"));
    }
    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::write(
        tmp.join(".anikit-complete"),
        format!("{}\n{}\n", pack.license, pack.page),
    );
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&tmp, &dir).map_err(|e| format!("cannot move pack into place: {e}"))?;
    Ok(dir)
}

/// Audio files in a pack, sorted.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if matches!(
                    p.extension().and_then(|x| x.to_str()),
                    Some("ogg" | "wav" | "flac")
                ) {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

/// The group of a file: its stem lower-cased with trailing digits / `_NNN` removed.
fn group(p: &Path) -> String {
    let s = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    s.trim_end_matches(|c: char| c.is_ascii_digit())
        .trim_end_matches(['_', '-', ' '])
        .to_string()
}

/// Groups (sound names) in a cached pack.
pub fn groups(name: &str) -> Vec<String> {
    let mut g: Vec<String> = files(&pack_dir(name)).iter().map(|p| group(p)).collect();
    g.dedup();
    g.sort();
    g.dedup();
    g
}

/// Whether `name` looks like a pack sound (`pack/group`).
pub fn is_pack_sound(name: &str) -> bool {
    name.split_once('/')
        .is_some_and(|(p, _)| PACKS.iter().any(|k| k.name == p))
}

/// Load `pack/group` (variant chosen by `seed`), fetching the pack if needed.
/// Returns the audio and its onset (seconds), the hit point.
pub fn load(name: &str, seed: u64, rate: u32) -> Result<(Pcm, f32), String> {
    let (pack, grp) = name
        .split_once('/')
        .ok_or("pack sounds are named pack/sound")?;
    let dir = fetch(pack)?;
    let all = files(&dir);
    let matches: Vec<&PathBuf> = all.iter().filter(|p| group(p) == grp).collect();
    if matches.is_empty() {
        let gs = groups(pack);
        let names: Vec<&str> = gs.iter().map(|s| s.as_str()).collect();
        return Err(format!(
            "no sound '{grp}' in pack {pack}{}; it has: {}",
            anikit_music::dsp::reverb::closest(grp, &names)
                .map(|s| format!(" (did you mean '{s}'?)"))
                .unwrap_or_default(),
            gs.join(", ")
        ));
    }
    let pick = matches[(seed as usize) % matches.len()];
    let pcm = crate::audio::decode_file_at(pick, rate)?;
    // Onset: first frame above 10% of the peak.
    let peak = pcm.peak().max(1e-9);
    let on = pcm
        .samples
        .chunks_exact(2)
        .position(|c| c[0].abs().max(c[1].abs()) > 0.1 * peak)
        .unwrap_or(0);
    Ok((pcm, on as f32 / rate as f32))
}
