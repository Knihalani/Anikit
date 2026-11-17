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

/// Synth / model names → General MIDI stand-ins (used when no dedicated model exists).
const GM_STANDINS: &[(&str, u8)] = &[
    ("pad-warm", 89), ("pad-glass", 92), ("pad-dark", 95), ("pad-choir", 91), ("strings-synth", 50),
    ("lead-saw", 81), ("lead-square", 80), ("lead-soft", 82), ("pluck", 46), ("pluck-bright", 46), ("arp-pluck", 81),
    ("bell-fm", 88), ("epiano-fm", 5), ("bass-sub", 38), ("bass-reese", 39), ("bass-808", 38), ("bass-acid", 38),
    ("bass-synth", 38), ("bass-wobble", 39), ("brass-synth", 62), ("chip-square", 80), ("supersaw", 81),
    ("stab-house", 16), ("keys-lofi", 4), ("pluck-bass", 34), ("pizz", 45), ("crystal", 98), ("bell", 14),
    ("church-bell", 14), ("handpan", 114), ("electric-guitar", 27), ("vibraphone", 11), ("vibes", 11),
];

/// Names where the physical/modal model is preferred over the GM sample.
const PREFER_MODEL: &[&str] = &[
    "marimba", "vibraphone", "vibes", "xylophone", "glockenspiel", "celesta", "kalimba", "music-box", "tubular-bells", "church-bell",
    "bell", "handpan", "steel-drum", "wood-block", "crystal", "koto", "banjo", "electric-guitar",
];

/// Drum kit names → GM kit programs (bank 128).
const KITS: &[(&str, u8)] = &[
    ("standard", 0), ("room", 8), ("power", 16), ("rock", 16), ("electronic", 24), ("808", 25), ("tr808", 25),
    ("909", 25), ("house", 25), ("techno", 24), ("trap", 25), ("dance", 26), ("lofi", 0), ("pop", 0), ("jazz", 32),
    ("brush", 40), ("orchestra", 48), ("cinematic", 16),
];

fn resolve_one(name: &str, base: Option<&Path>) -> Result<Arc<dyn Instrument>, String> {
    use crate::sf2::{bank, sampler::SoundFont};
    let lower = name.to_ascii_lowercase();
    if lower == "drums" || lower == "kit" {
        return Ok(bank::gm_drums(0));
    }
    if let Some(p) = lower.strip_prefix("phys:") {
        return crate::physical::preset(p).ok_or_else(|| format!("unknown physical model '{name}' ({})", crate::physical::PRESETS.join(" ")));
    }
    if let Some(p) = lower.strip_prefix("synth:") {
        return crate::synth::preset(p).ok_or_else(|| format!("unknown synth '{name}' ({})", crate::synth::PRESETS.join(" ")));
    }
    if let Some(k) = lower.strip_prefix("drums:").or_else(|| lower.strip_prefix("kit:")) {
        if let Some(kit) = crate::synth::kit(k) {
            return Ok(kit);
        }
        return KITS.iter().find(|(n, _)| *n == k).map(|(_, p)| bank::gm_drums(*p)).ok_or_else(|| {
            let ks: Vec<&str> = KITS.iter().map(|x| x.0).collect();
            let hint = crate::spec::closest(k, &ks).map(|b| format!(" — did you mean 'drums:{b}'?")).unwrap_or_default();
            format!("unknown drum kit '{name}'{hint} (kits: {})", ks.join(" "))
        });
    }
    if let Some(p) = lower.strip_prefix("gm:") {
        if let Ok(n) = p.parse::<u8>() {
            return Ok(bank::gm_instrument(n.min(127)));
        }
        return bank::by_name(p).ok_or_else(|| format!("unknown GM instrument '{name}'"));
    }
    // Files: path.sf2#program, path.sf3#bank:program.
    let file = |f: &str| match base {
        Some(b) if Path::new(f).is_relative() => b.join(f),
        _ => Path::new(f).to_path_buf(),
    };
    let (path, frag) = match name.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (name, None),
    };
    let pl = path.to_ascii_lowercase();
    if pl.ends_with(".sf2") || pl.ends_with(".sf3") {
        let font = SoundFont::from_file(file(path))?;
        let (b, p) = match frag {
            None => (0, 0),
            Some(f) => match f.split_once(':') {
                Some((b, p)) => (b.parse().map_err(|_| format!("'{name}': use file.sf2#bank:program"))?, p.parse().map_err(|_| format!("'{name}': use file.sf2#bank:program"))?),
                None => match f.parse::<u16>() {
                    Ok(p) => (0, p),
                    Err(_) => return font.instrument_by_name(f).ok_or_else(|| format!("'{name}': no preset named '{f}'")),
                },
            },
        };
        return font.instrument(b, p).ok_or_else(|| format!("'{name}': no preset {b}:{p}"));
    }
    if pl.ends_with(".sfz") {
        return crate::sf2::load_sfz(file(path));
    }
    if let Some(i) = crate::sf2::pack_instrument(&lower) {
        return Ok(i);
    }
    if let Some(i) = crate::synth::preset(&lower) {
        return Ok(i);
    }
    if PREFER_MODEL.contains(&lower.as_str()) {
        if let Some(i) = crate::physical::preset(&lower) {
            return Ok(i);
        }
    }
    if let Some(i) = bank::by_name(&lower) {
        return Ok(i);
    }
    if let Some(i) = crate::physical::preset(&lower) {
        return Ok(i);
    }
    if let Some((_, p)) = GM_STANDINS.iter().find(|(n, _)| *n == lower) {
        return Ok(bank::gm_instrument(*p));
    }
    let all: Vec<&str> = bank::NAMES.iter().copied().chain(GM_STANDINS.iter().map(|x| x.0)).collect();
    let hint = crate::spec::closest(&lower, &all).map(|b| format!(" — did you mean '{b}'?")).unwrap_or_default();
    Err(format!("unknown instrument '{name}'{hint} (anikit-video music instruments lists them)"))
}

/// All built-in instrument names, by source (for docs and suggestions).
pub fn names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("sampled (GM bank)", crate::sf2::bank::NAMES.to_vec()),
        ("physical models (phys:NAME)", crate::physical::PRESETS.to_vec()),
        ("synths (synth:NAME)", crate::synth::PRESETS.to_vec()),
        ("drum kits (drums:NAME)", crate::synth::KITS.iter().copied().chain(KITS.iter().map(|x| x.0)).collect()),
        ("packs (anikit-video music fetch …)", crate::sf2::packs::PACKS.iter().map(|p| p.name).collect()),
    ]
}
