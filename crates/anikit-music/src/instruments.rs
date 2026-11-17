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
    // Recorded instruments from cached packs replace the GM samples by default
    // (`anikit-video music fetch essentials`).
    for p in pack_defaults(&lower) {
        if let Some(i) = crate::sf2::pack_instrument(p) {
            return Ok(i);
        }
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

/// Where a name resolves (same order as [`resolve`], files excluded):
/// Pack instruments that stand in for a common name when their pack is
/// cached, best first.
pub fn pack_defaults(name: &str) -> &'static [&'static str] {
    match name {
        "piano" | "grand-piano" | "bright-piano" => &["salamander-piano", "splendid-piano"],
        "wurli" | "wurlitzer" => &["wurlitzer"],
        "bass" | "finger-bass" | "fingered-bass" | "electric-bass" | "bass-guitar" => &["finger-bass-yr"],
        "pick-bass" | "picked-bass" => &["picked-bass-yr"],
        "nylon-guitar" | "classical-guitar" | "spanish-guitar" => &["nylon-guitar-fp"],
        "clean-guitar" | "electric-guitar" | "jazz-guitar" => &["clean-guitar-fsbs"],
        _ => &[],
    }
}

fn pack_cached(friendly: &str) -> bool {
    crate::sf2::packs::INSTRUMENTS.iter().find(|x| x.0 == friendly).is_some_and(|x| crate::sf2::packs::is_cached(x.1))
}

/// `"synth"`, `"synth-kit"`, `"physical"`, `"sampled"`, `"sampled-kit"`, `"pack"` or `"unknown"`.
pub fn source(name: &str) -> &'static str {
    for n in name.split('|').map(|n| n.trim().to_ascii_lowercase()) {
        let kind = if n == "drums" || n == "kit" {
            "sampled-kit"
        } else if n.starts_with("phys:") {
            "physical"
        } else if n.starts_with("synth:") {
            "synth"
        } else if let Some(k) = n.strip_prefix("drums:").or_else(|| n.strip_prefix("kit:")) {
            if crate::synth::KITS.contains(&k) {
                "synth-kit"
            } else if KITS.iter().any(|x| x.0 == k) {
                "sampled-kit"
            } else {
                continue;
            }
        } else if n.starts_with("gm:") {
            "sampled"
        } else if pack_cached(&n) || pack_defaults(&n).iter().any(|p| pack_cached(p)) {
            "pack"
        } else if crate::synth::PRESETS.contains(&n.as_str()) {
            "synth"
        } else if PREFER_MODEL.contains(&n.as_str()) && crate::physical::PRESETS.contains(&n.as_str()) {
            "physical"
        } else if crate::sf2::bank::lookup(&n).is_some() {
            "sampled"
        } else if crate::physical::PRESETS.contains(&n.as_str()) {
            "physical"
        } else if GM_STANDINS.iter().any(|x| x.0 == n) {
            "sampled"
        } else {
            continue;
        };
        return kind;
    }
    "unknown"
}

/// All built-in instrument names, by source (for docs and suggestions).
pub fn names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("sampled (GM bank)", crate::sf2::bank::NAMES.to_vec()),
        ("physical models (phys:NAME)", crate::physical::PRESETS.to_vec()),
        ("synths (synth:NAME)", crate::synth::PRESETS.to_vec()),
        ("drum kits (drums:NAME; synth kits first, then GM kits)", {
            let mut k: Vec<&str> = crate::synth::KITS.to_vec();
            k.extend(KITS.iter().map(|x| x.0).filter(|n| !crate::synth::KITS.contains(n)));
            k
        }),
        ("packs (anikit-video music fetch …)", crate::sf2::packs::PACKS.iter().map(|p| p.name).collect()),
    ]
}

/// A drum kit placed across the stereo field by GM key (audience view):
/// kick and snare centre, hats right, ride and floor tom left, toms spread,
/// crashes either side, hand percussion wide. Balance-style (the voice's own
/// stereo is kept), allocation-free.
pub struct KitStage(pub Arc<dyn Instrument>);

/// Pan (−1…1) for a GM drum key.
pub fn kit_pan(key: i32) -> f32 {
    match key {
        35 | 36 => 0.0,
        37 | 38 | 40 => 0.05,
        39 => -0.12,
        42 | 44 | 46 => 0.35,
        41 | 43 => -0.4,
        45 => -0.2,
        47 | 48 => 0.05,
        50 => 0.28,
        49 => -0.35,
        57 => 0.4,
        51 | 53 | 59 => -0.3,
        52 | 55 => 0.45,
        54 => -0.5,
        56 => 0.3,
        69 | 70 | 82 => 0.55,
        60..=68 => -0.45,
        _ => 0.0,
    }
}

impl Instrument for KitStage {
    fn voice(&self, note: &crate::voice::NoteOn) -> Box<dyn crate::voice::Voice> {
        let p = kit_pan(note.key.round() as i32);
        // Round-robin emulation: every hit is a slightly different strike (±8 cents,
        // ±5 % level), so repeated hats and snares don't machine-gun. Deterministic per note.
        let mut on = *note;
        let mut rng = crate::voice::Rng::new(note.seed ^ 0x2B0B);
        on.key += rng.bi() * 0.08;
        on.vel = (on.vel * (1.0 + 0.05 * rng.bi())).clamp(0.01, 1.0);
        let inner = self.0.voice(&on);
        if p == 0.0 {
            return inner;
        }
        let (gl, gr) = crate::voice::pan_gains(p);
        let s = std::f32::consts::SQRT_2;
        Box::new(StagedVoice { inner, gl: gl * s, gr: gr * s })
    }
    fn mono(&self) -> bool {
        self.0.mono()
    }
    fn ordered_voices(&self) -> bool {
        self.0.ordered_voices()
    }
    fn voice_legato(&self, note: &crate::voice::NoteOn, legato: bool) -> Box<dyn crate::voice::Voice> {
        let _ = legato;
        self.voice(note)
    }
}

struct StagedVoice {
    inner: Box<dyn crate::voice::Voice>,
    gl: f32,
    gr: f32,
}

impl crate::voice::Voice for StagedVoice {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len();
        let mut bl = [0.0f32; crate::voice::BLOCK];
        let mut br = [0.0f32; crate::voice::BLOCK];
        let alive = self.inner.render(&mut bl[..n], &mut br[..n]);
        for i in 0..n {
            l[i] += bl[i] * self.gl;
            r[i] += br[i] * self.gr;
        }
        alive
    }
    fn release(&mut self) {
        self.inner.release();
    }
    fn exclusive(&self) -> u32 {
        self.inner.exclusive()
    }
    fn kill(&mut self) {
        self.inner.kill();
    }
}
