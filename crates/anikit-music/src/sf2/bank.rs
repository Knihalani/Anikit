//! The bundled General MIDI bank: GeneralUser GS v2.0.3 by S. Christian
//! Collins (<http://www.schristiancollins.com>), re-encoded to compact SF3
//! (Vorbis q6.5, 5.9 MB). See `LICENSES.md`.

use super::sampler::SoundFont;
use crate::voice::Instrument;
use std::sync::{Arc, OnceLock};

static BYTES: &[u8] = include_bytes!("../../assets/GeneralUser-GS.sf3");

/// The bundled General MIDI bank (parsed on first call, ~10 ms; each sample
/// is decoded the first time a note uses it).
pub fn gm() -> Arc<SoundFont> {
    static GM: OnceLock<Arc<SoundFont>> = OnceLock::new();
    GM.get_or_init(|| SoundFont::from_bytes(BYTES).expect("bundled GeneralUser GS bank is valid")).clone()
}

/// A General MIDI melodic program (0–127, bank 0).
pub fn gm_instrument(program: u8) -> Arc<dyn Instrument> {
    let g = gm();
    g.instrument(0, program.min(127) as u16).or_else(|| g.instrument(0, 0)).expect("bank has a piano")
}

/// A drum kit: the program number in bank 128 (0 standard, 8 room, 16 power,
/// 24 electronic, 25 808/909, 26 dance, 32 jazz, 40 brush, 48 orchestral,
/// 56 SFX, 127 CM-64). Unknown kits fall back to the standard kit.
pub fn gm_drums(kit: u8) -> Arc<dyn Instrument> {
    let g = gm();
    g.instrument(128, kit as u16).or_else(|| g.instrument(128, 0)).expect("bank has a standard kit")
}

/// Friendly name → (bank, program). Bank 128 = drum kits.
const TABLE: &[(&str, u16, u16)] = &[
    // keys
    ("piano", 0, 0),
    ("grand-piano", 0, 0),
    ("bright-piano", 0, 1),
    ("electric-grand", 0, 2),
    ("honky-tonk", 0, 3),
    ("epiano", 0, 4),
    ("rhodes", 0, 4),
    ("wurli", 8, 4),
    ("fm-epiano", 0, 5),
    ("dx-epiano", 0, 5),
    ("harpsichord", 0, 6),
    ("clavinet", 0, 7),
    ("celesta", 0, 8),
    ("celeste", 0, 8),
    ("glockenspiel", 0, 9),
    ("music-box", 0, 10),
    ("vibes", 0, 11),
    ("vibraphone", 0, 11),
    ("marimba", 0, 12),
    ("xylophone", 0, 13),
    ("tubular-bells", 0, 14),
    ("church-bells", 8, 14),
    ("dulcimer", 0, 15),
    ("organ", 0, 16),
    ("tonewheel-organ", 0, 16),
    ("percussive-organ", 0, 17),
    ("rock-organ", 0, 18),
    ("pipe-organ", 0, 19),
    ("church-organ", 0, 19),
    ("reed-organ", 0, 20),
    ("accordion", 0, 21),
    ("harmonica", 0, 22),
    ("bandoneon", 0, 23),
    // guitars
    ("nylon-guitar", 0, 24),
    ("classical-guitar", 0, 24),
    ("acoustic-guitar", 0, 25),
    ("steel-guitar", 0, 25),
    ("twelve-string", 8, 25),
    ("ukulele", 8, 24),
    ("mandolin", 16, 25),
    ("jazz-guitar", 0, 26),
    ("clean-guitar", 0, 27),
    ("funk-guitar", 8, 28),
    ("muted-guitar", 0, 28),
    ("overdrive-guitar", 0, 29),
    ("distortion-guitar", 0, 30),
    ("guitar-harmonics", 0, 31),
    // basses
    ("acoustic-bass", 0, 32),
    ("upright-bass", 0, 32),
    ("bass", 0, 33),
    ("finger-bass", 0, 33),
    ("pick-bass", 0, 34),
    ("fretless", 0, 35),
    ("fretless-bass", 0, 35),
    ("slap-bass", 0, 36),
    ("synth-bass", 0, 38),
    ("synth-bass-2", 0, 39),
    ("acid-bass", 8, 38),
    // strings
    ("violin", 0, 40),
    ("viola", 0, 41),
    ("cello", 0, 42),
    ("contrabass", 0, 43),
    ("double-bass", 0, 43),
    ("tremolo-strings", 0, 44),
    ("pizzicato", 0, 45),
    ("harp", 0, 46),
    ("timpani", 0, 47),
    ("strings", 0, 48),
    ("string-ensemble", 0, 48),
    ("slow-strings", 0, 49),
    ("synth-strings", 0, 50),
    ("orchestra", 12, 48),
    ("orchestra-hit", 0, 55),
    // voices
    ("choir", 0, 52),
    ("aahs", 0, 52),
    ("oohs", 0, 53),
    ("synth-voice", 0, 54),
    // brass
    ("trumpet", 0, 56),
    ("trombone", 0, 57),
    ("tuba", 0, 58),
    ("muted-trumpet", 0, 59),
    ("horn", 1, 60),
    ("french-horn", 0, 60),
    ("horns", 0, 60),
    ("brass", 0, 61),
    ("synth-brass", 0, 62),
    // reeds and pipes
    ("soprano-sax", 0, 64),
    ("sax", 0, 65),
    ("alto-sax", 0, 65),
    ("tenor-sax", 0, 66),
    ("baritone-sax", 0, 67),
    ("oboe", 0, 68),
    ("english-horn", 0, 69),
    ("bassoon", 0, 70),
    ("clarinet", 0, 71),
    ("piccolo", 0, 72),
    ("flute", 0, 73),
    ("recorder", 0, 74),
    ("pan-flute", 0, 75),
    ("tin-whistle", 24, 75),
    ("bottle", 0, 76),
    ("shakuhachi", 0, 77),
    ("whistle", 0, 78),
    ("ocarina", 0, 79),
    ("woodwinds", 13, 48),
    // synths
    ("square-lead", 0, 80),
    ("saw-lead", 0, 81),
    ("calliope", 0, 82),
    ("warm-pad", 0, 89),
    ("pad", 0, 89),
    ("polysynth", 0, 90),
    ("space-voice", 0, 91),
    ("glass-pad", 0, 92),
    ("halo-pad", 0, 94),
    ("sweep-pad", 0, 95),
    ("atmosphere", 0, 99),
    ("sine", 8, 80),
    // ethnic and percussive
    ("sitar", 0, 104),
    ("banjo", 0, 105),
    ("shamisen", 0, 106),
    ("koto", 0, 107),
    ("kalimba", 0, 108),
    ("bagpipes", 0, 109),
    ("fiddle", 0, 110),
    ("shanai", 0, 111),
    ("agogo", 0, 113),
    ("steel-drums", 0, 114),
    ("woodblock", 0, 115),
    ("taiko", 0, 116),
    ("melodic-tom", 0, 117),
    ("castanets", 8, 115),
    // drum kits (bank 128)
    ("drums", 128, 0),
    ("standard", 128, 0),
    ("room", 128, 8),
    ("power", 128, 16),
    ("electronic", 128, 24),
    ("tr808", 128, 25),
    ("808", 128, 25),
    ("909", 128, 25),
    ("dance", 128, 26),
    ("jazz", 128, 32),
    ("brush", 128, 40),
    ("brushes", 128, 40),
    ("orchestral", 128, 48),
    ("sfx", 128, 56),
];

/// Friendly instrument names [`by_name`] accepts (drum kits also accept a
/// `-kit` or `-drums` suffix, e.g. `jazz-kit`, `brush-drums`).
pub const NAMES: &[&str] = &[
    "piano",
    "grand-piano",
    "bright-piano",
    "electric-grand",
    "honky-tonk",
    "epiano",
    "rhodes",
    "wurli",
    "fm-epiano",
    "dx-epiano",
    "harpsichord",
    "clavinet",
    "celesta",
    "celeste",
    "glockenspiel",
    "music-box",
    "vibes",
    "vibraphone",
    "marimba",
    "xylophone",
    "tubular-bells",
    "church-bells",
    "dulcimer",
    "organ",
    "tonewheel-organ",
    "percussive-organ",
    "rock-organ",
    "pipe-organ",
    "church-organ",
    "reed-organ",
    "accordion",
    "harmonica",
    "bandoneon",
    "nylon-guitar",
    "classical-guitar",
    "acoustic-guitar",
    "steel-guitar",
    "twelve-string",
    "ukulele",
    "mandolin",
    "jazz-guitar",
    "clean-guitar",
    "funk-guitar",
    "muted-guitar",
    "overdrive-guitar",
    "distortion-guitar",
    "guitar-harmonics",
    "acoustic-bass",
    "upright-bass",
    "bass",
    "finger-bass",
    "pick-bass",
    "fretless",
    "fretless-bass",
    "slap-bass",
    "synth-bass",
    "synth-bass-2",
    "acid-bass",
    "violin",
    "viola",
    "cello",
    "contrabass",
    "double-bass",
    "tremolo-strings",
    "pizzicato",
    "harp",
    "timpani",
    "strings",
    "string-ensemble",
    "slow-strings",
    "synth-strings",
    "orchestra",
    "orchestra-hit",
    "choir",
    "aahs",
    "oohs",
    "synth-voice",
    "trumpet",
    "trombone",
    "tuba",
    "muted-trumpet",
    "horn",
    "french-horn",
    "horns",
    "brass",
    "synth-brass",
    "soprano-sax",
    "sax",
    "alto-sax",
    "tenor-sax",
    "baritone-sax",
    "oboe",
    "english-horn",
    "bassoon",
    "clarinet",
    "piccolo",
    "flute",
    "recorder",
    "pan-flute",
    "tin-whistle",
    "bottle",
    "shakuhachi",
    "whistle",
    "ocarina",
    "woodwinds",
    "square-lead",
    "saw-lead",
    "calliope",
    "warm-pad",
    "pad",
    "polysynth",
    "space-voice",
    "glass-pad",
    "halo-pad",
    "sweep-pad",
    "atmosphere",
    "sine",
    "sitar",
    "banjo",
    "shamisen",
    "koto",
    "kalimba",
    "bagpipes",
    "fiddle",
    "shanai",
    "agogo",
    "steel-drums",
    "woodblock",
    "taiko",
    "melodic-tom",
    "castanets",
    "drums",
    "standard",
    "room",
    "power",
    "electronic",
    "tr808",
    "808",
    "909",
    "dance",
    "jazz",
    "brush",
    "brushes",
    "orchestral",
    "sfx",
];

/// Normalise a user-typed name: lower case, `_`/space → `-`.
fn norm(name: &str) -> String {
    name.trim().to_lowercase().chars().map(|c| if c == '_' || c == ' ' { '-' } else { c }).collect()
}

/// (bank, program) for a friendly name, if it is in the curated table.
pub fn lookup(name: &str) -> Option<(u16, u16)> {
    let n = norm(name);
    let find = |k: &str| TABLE.iter().find(|(t, _, _)| *t == k).map(|&(_, b, p)| (b, p));
    if let Some(bp) = find(&n) {
        return Some(bp);
    }
    for suffix in ["-kit", "-drums", "-drum-kit"] {
        if let Some(base) = n.strip_suffix(suffix) {
            if let Some((128, p)) = find(base) {
                return Some((128, p));
            }
        }
    }
    // "gm:NN" / "gm-NN" program numbers
    if let Some(num) = n.strip_prefix("gm:").or_else(|| n.strip_prefix("gm-")) {
        if let Ok(p) = num.parse::<u16>() {
            return (p < 128).then_some((0, p));
        }
    }
    None
}

/// An instrument by friendly name (see [`NAMES`]); otherwise the first GM
/// preset whose name contains it (case-insensitive), melodic banks first.
pub fn by_name(name: &str) -> Option<Arc<dyn Instrument>> {
    let g = gm();
    if let Some((b, p)) = lookup(name) {
        return g.instrument(b, p);
    }
    let q = norm(name).replace('-', " ");
    if q.is_empty() {
        return None;
    }
    let ps = g.presets();
    let hit = ps
        .iter()
        .find(|(b, _, n)| *b < 120 && n.to_lowercase() == q)
        .or_else(|| ps.iter().find(|(b, _, n)| *b < 120 && n.to_lowercase().contains(&q)))
        .or_else(|| ps.iter().find(|(_, _, n)| n.to_lowercase().contains(&q)))?;
    g.instrument(hit.0, hit.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_bank_names_resolve() {
        let g = gm();
        assert_eq!(g.presets().len(), 287);
        for n in NAMES {
            let (b, p) = lookup(n).unwrap_or_else(|| panic!("{n} not in table"));
            assert!(g.instrument(b, p).is_some(), "{n} → {b}/{p} missing");
        }
        assert_eq!(TABLE.len(), NAMES.len());
        assert_eq!(lookup("Jazz Kit"), Some((128, 32)));
        assert_eq!(lookup("brush-drums"), Some((128, 40)));
        assert_eq!(lookup("gm:73"), Some((0, 73)));
        assert!(by_name("Bagpipe").is_some());
        assert!(by_name("no such instrument xyz").is_none());
    }
}
