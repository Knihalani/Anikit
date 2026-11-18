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
    /// Sparse-checkout patterns (gitignore syntax, e.g. `/*.sfz`); empty =
    /// the whole repository.
    pub sparse: &'static [&'static str],
    /// Files derived after download: (new file, source file, find, replace),
    /// e.g. a program pointed at a different keymap.
    pub derive: &'static [(&'static str, &'static str, &'static str, &'static str)],
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
        sparse: &[],
        derive: &[],
    },
    Pack {
        name: "epianos",
        description: "Greg Sullivan E-Pianos — Wurlitzer EP200, Yamaha CP80, Hohner Pianet T (SFZ + FLAC)",
        license: "CC BY 3.0",
        size: "≈40 MB",
        url: "https://github.com/sfzinstruments/GregSullivan.E-Pianos",
        branch: "master",
        credit: "E-Pianos by Greg Sullivan (CC BY 3.0), SFZ mapping by kinwie",
        sparse: &[],
        derive: &[],
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
        sparse: &[],
        derive: &[],
    },
    Pack {
        name: "vsco-strings",
        description: "VS Chamber Orchestra 2 CE, string sections only: violins, violas, cellos and contrabass, \
                      sustain (vibrato), spiccato, pizzicato and tremolo (SFZ, WAV)",
        license: "CC0 1.0 (public domain dedication)",
        size: "≈410 MB download, ≈1 GB on disk",
        url: "https://github.com/sgossner/VSCO-2-CE",
        branch: "SFZ",
        credit: "",
        sparse: &["/*.sfz", "/LICENSE", "/README.md", "/Strings/Violin Section/", "/Strings/Viola Section/", "/Strings/Cello Section/", "/Strings/Solo Contrabass/"],
        derive: &[],
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
        sparse: &[],
        derive: &[],
    },
    Pack {
        name: "freepats-bass",
        description: "FreePats electric bass YR — Yamaha RBX, finger and picked, one sample per semitone E1–D#2 (SFZ + FLAC)",
        license: "CC0 1.0 (public domain dedication)",
        size: "≈13 MB",
        url: "https://github.com/freepats/electric-bass-YR",
        branch: "2019-09-30",
        credit: "",
        sparse: &[],
        derive: &[],
    },
    Pack {
        name: "freepats-nylon",
        description: "FreePats Spanish classical guitar — nylon strings, 48 regions (SFZ + FLAC)",
        license: "CC0 1.0 (public domain dedication)",
        size: "≈5 MB",
        url: "https://github.com/freepats/spanish-classical-guitar",
        branch: "v1.0.0",
        credit: "",
        sparse: &[],
        derive: &[],
    },
    Pack {
        name: "freepats-eguitar",
        description: "FreePats clean electric guitar FSBS — bridge pickup, small set (SFZ + FLAC)",
        license: "CC0 1.0 (public domain dedication)",
        size: "≈3 MB (sparse checkout of the small set)",
        url: "https://github.com/freepats/electric-guitar-FSBS-clean",
        branch: "2026-08-07",
        credit: "",
        sparse: &["/*.sfz", "/*.txt", "/samples/bridge/small/"],
        derive: &[],
    },
    Pack {
        name: "virtuosity",
        description: "Virtuosity Drums — a real acoustic kit, stereo mid-room mic, many velocity layers and \
                      round robins (kick, snare, toms, hi-hat, ride, crash) (SFZ, WAV)",
        license: "CC0 1.0 (public domain dedication)",
        size: "≈250 MB download (only the mid-mic samples), ≈510 MB on disk",
        url: "https://github.com/sfzinstruments/virtuosity_drums",
        branch: "v0.925",
        credit: "",
        sparse: &["/Programs/", "/LICENSE", "/README.md", "/Samples/mid/"],
        // The shipped program uses the e-kit keymap (open hat = key 42 + pedal);
        // the default keymap puts it on GM key 46.
        derive: &[("Programs/anikit-mid-kit.sfz", "Programs/06-mid-mic.sfz", "#include \"keymaps/keymap.sfz\"", "#include \"keymaps/default/keymap.sfz\"")],
    },
];

/// Packs recorded over a narrow range: their edge samples are stretched to
/// cover every key (see [`SfzOptions::extend`]).
/// Write a pack's derived files (idempotent; also run for caches fetched
/// before a derivation existed).
fn derive_files(pack: &Pack, dir: &std::path::Path) {
    for (new, src, find, replace) in pack.derive {
        let out = dir.join(new);
        if out.is_file() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(dir.join(src)) {
            let _ = std::fs::write(&out, text.replacen(find, replace, 1));
        }
    }
}

/// Packs recorded with distant mics: the first milliseconds of every sample
/// are travel time (the Virtuosity mid mic is about 3 m from the kit, ≈8 ms),
/// skipped so hits land on the grid. A millisecond of lead-in is kept.
const TRIM_MS: &[(&str, f32)] = &[("virtuosity", 7.0)];

/// Packs recorded with widely spaced mics: their side signal is scaled so the
/// image stays mono-compatible (Splendid's samples measure L/R correlation
/// −0.22…0.43 and lose up to 4 dB summed to mono; at 0.5 they sit near 0.45).
const WIDTH: &[(&str, f32)] = &[("splendid", 0.5)];

/// Per-pack fixes applied to every voice: skip `trim` frames (at 48 kHz),
/// scale the side signal by `width`.
struct PackVoice {
    inner: SfzInstrument,
    trim: usize,
    width: f32,
}

impl Instrument for PackVoice {
    fn voice(&self, note: &crate::voice::NoteOn) -> Box<dyn crate::voice::Voice> {
        let mut v = self.inner.voice(note);
        let n = (self.trim as f32 * note.rate / 48_000.0) as usize;
        let (mut l, mut r) = ([0f32; crate::voice::BLOCK], [0f32; crate::voice::BLOCK]);
        let mut left = n;
        while left > 0 {
            let m = left.min(crate::voice::BLOCK);
            v.render(&mut l[..m], &mut r[..m]);
            left -= m;
        }
        if (self.width - 1.0).abs() < 1e-6 {
            v
        } else {
            Box::new(Narrow { v, width: self.width })
        }
    }
}

struct Narrow {
    v: Box<dyn crate::voice::Voice>,
    width: f32,
}

impl crate::voice::Voice for Narrow {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let (mut a, mut b) = ([0f32; crate::voice::BLOCK], [0f32; crate::voice::BLOCK]);
        let n = l.len();
        let alive = self.v.render(&mut a[..n], &mut b[..n]);
        let (wm, ws) = (0.5, 0.5 * self.width);
        for i in 0..n {
            let (m, s) = (wm * (a[i] + b[i]), ws * (a[i] - b[i]));
            l[i] += m + s;
            r[i] += m - s;
        }
        alive
    }
    fn release(&mut self) {
        self.v.release();
    }
    fn exclusive(&self) -> u32 {
        self.v.exclusive()
    }
    fn kill(&mut self) {
        self.v.kill();
    }
}

const EXTEND: &[&str] = &["freepats-bass", "freepats-nylon", "freepats-eguitar"];

/// Friendly name → (pack, SFZ file inside it).
pub const INSTRUMENTS: &[(&str, &str, &str)] = &[
    ("splendid-piano", "splendid", "Splendid Grand Piano.sfz"),
    ("virtuosity-kit", "virtuosity", "Programs/anikit-mid-kit.sfz"),
    ("finger-bass-yr", "freepats-bass", "FingerBassYR 20190930.sfz"),
    ("picked-bass-yr", "freepats-bass", "PickedBassYR 20190930.sfz"),
    ("nylon-guitar-fp", "freepats-nylon", "SpanishClassicalGuitar-20190618.sfz"),
    ("clean-guitar-fsbs", "freepats-eguitar", "EGuitarFSBS-clean bridge small 20260807.sfz"),
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
    ("vsco-violas-spiccato", "vsco2-ce", "ViolaEnsSpic.sfz"),
    ("vsco-violas-tremolo", "vsco2-ce", "ViolaEnsTrem.sfz"),
    ("vsco-cellos-spiccato", "vsco2-ce", "CelloEnsSpic.sfz"),
    ("vsco-cellos-tremolo", "vsco2-ce", "CelloEnsTrem.sfz"),
    ("vsco-contrabass-spiccato", "vsco2-ce", "ContrabassSpic.sfz"),
    ("vsco-contrabass-tremolo", "vsco2-ce", "ContrabassTrem.sfz"),
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

/// A pack whose files are also in a smaller pack (a sparse subset of the same
/// repository): looked up there when the full pack is not cached.
const ALSO_IN: &[(&str, &str)] = &[("vsco2-ce", "vsco-strings")];

/// Section instruments: one name, each register played by its own section
/// (lowest key of each part, ascending).
pub const SECTIONS: &[(&str, &[(&str, i32)])] = &[
    ("vsco-strings-section", &[("vsco-contrabass", 0), ("vsco-cellos", 36), ("vsco-violas", 52), ("vsco-violins", 62)]),
    ("vsco-strings-spiccato", &[("vsco-contrabass-spiccato", 0), ("vsco-cellos-spiccato", 36), ("vsco-violas-spiccato", 52), ("vsco-violins-spiccato", 62)]),
    ("vsco-strings-pizz", &[("vsco-contrabass-pizz", 0), ("vsco-cello-pizz", 36), ("vsco-violas-pizz", 52), ("vsco-violins-pizz", 62)]),
    ("vsco-strings-tremolo", &[("vsco-contrabass-tremolo", 0), ("vsco-cellos-tremolo", 36), ("vsco-violas-tremolo", 52), ("vsco-violins-tremolo", 62)]),
];

/// Plays each note on the part whose register contains it.
struct Split(Vec<(i32, Arc<dyn Instrument>)>);

impl Instrument for Split {
    fn voice(&self, note: &crate::voice::NoteOn) -> Box<dyn crate::voice::Voice> {
        let k = note.key.round() as i32;
        let part = self.0.iter().rev().find(|(lo, _)| k >= *lo).unwrap_or(&self.0[0]);
        part.1.voice(note)
    }
}

/// Names [`pack_instrument`] understands.
pub fn instrument_names() -> Vec<&'static str> {
    INSTRUMENTS.iter().map(|(n, _, _)| *n).chain(SECTIONS.iter().map(|s| s.0)).collect()
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

/// The small packs worth having for everyday scores (≈210 MB): a better grand
/// piano, vintage electric pianos, a real electric bass, nylon and clean
/// electric guitars. `fetch("essentials")` gets them all.
pub const ESSENTIALS: &[&str] = &["splendid", "epianos", "freepats-bass", "freepats-nylon", "freepats-eguitar"];

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
    let mut clone = Command::new("git");
    clone.args(["clone", "--depth", "1", "--single-branch", "--branch", pack.branch]);
    if !pack.sparse.is_empty() {
        clone.args(["--filter=blob:none", "--sparse"]);
    }
    let st = clone.arg(pack.url).arg(&tmp).status().map_err(|e| format!("cannot run git (is it installed?): {e}"))?;
    let st = if st.success() && !pack.sparse.is_empty() {
        Command::new("git")
            .arg("-C")
            .arg(&tmp)
            .args(["sparse-checkout", "set", "--no-cone"])
            .args(pack.sparse)
            .status()
            .map_err(|e| format!("cannot run git: {e}"))?
    } else {
        st
    };
    if !st.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("git clone of {} failed ({st})", pack.url));
    }
    std::fs::rename(&tmp, &dir).map_err(|e| format!("cannot move pack into place: {e}"))?;
    let _ = std::fs::write(dir.join(".anikit-complete"), pack.license);
    derive_files(pack, &dir);
    Ok(dir)
}

/// An instrument from a cached pack by friendly name (see
/// [`instrument_names`]), or a path inside a pack such as
/// `vsco2-ce/FluteStac.sfz`. `None` if the pack is not cached or the name
/// is unknown, so callers can fall back to the GM bank.
pub fn pack_instrument(name: &str) -> Option<Arc<dyn Instrument>> {
    let n = name.trim().to_lowercase().replace([' ', '_'], "-");
    if let Some((_, parts)) = SECTIONS.iter().find(|s| s.0 == n) {
        let parts: Option<Vec<(i32, Arc<dyn Instrument>)>> = parts.iter().map(|(p, lo)| Some((*lo, pack_instrument(p)?))).collect();
        return Some(Arc::new(Split(parts?)));
    }
    let (pack, file) = match INSTRUMENTS.iter().find(|(k, _, _)| *k == n) {
        Some(&(_, p, f)) => (p.to_string(), f.to_string()),
        None => {
            let (p, f) = name.trim().split_once('/')?;
            PACKS.iter().find(|k| k.name == p)?;
            (p.to_string(), f.to_string())
        }
    };
    let mut pack = pack;
    if !is_cached(&pack) {
        if let Some((_, sub)) = ALSO_IN.iter().find(|a| a.0 == pack) {
            if is_cached(sub) {
                pack = sub.to_string();
            }
        }
    }
    let path = pack_dir(&pack).join(&file);
    if !path.is_file() && is_cached(&pack) {
        if let Some(p) = PACKS.iter().find(|k| k.name == pack) {
            derive_files(p, &pack_dir(&pack));
        }
    }
    if !path.is_file() {
        return None;
    }
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<dyn Instrument>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(i) = cache.lock().ok()?.get(&path) {
        return Some(i.clone());
    }
    // Lenient: a sparse or partial checkout still plays what it has.
    let opt = SfzOptions { skip_missing: true, extend: EXTEND.contains(&pack.as_str()) };
    let sfz = SfzInstrument::load_with(&path, opt).ok()?;
    let trim = TRIM_MS.iter().find(|t| t.0 == pack).map_or(0, |t| (t.1 * 48.0) as usize);
    let width = WIDTH.iter().find(|t| t.0 == pack).map_or(1.0, |t| t.1);
    let inst: Arc<dyn Instrument> = if trim > 0 || width != 1.0 { Arc::new(PackVoice { inner: sfz, trim, width }) } else { Arc::new(sfz) };
    cache.lock().ok()?.insert(path, inst.clone());
    Some(inst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_plays_each_register_on_its_part() {
        use crate::voice::{NoteOn, Voice};
        struct Const(f32);
        struct ConstVoice(f32);
        impl Voice for ConstVoice {
            fn render(&mut self, l: &mut [f32], _r: &mut [f32]) -> bool {
                l.iter_mut().for_each(|x| *x += self.0);
                false
            }
            fn release(&mut self) {}
        }
        impl Instrument for Const {
            fn voice(&self, _: &NoteOn) -> Box<dyn Voice> {
                Box::new(ConstVoice(self.0))
            }
        }
        let s = Split(vec![(0, Arc::new(Const(1.0))), (36, Arc::new(Const(2.0))), (62, Arc::new(Const(3.0)))]);
        let which = |key: f32| {
            let mut v = s.voice(&NoteOn { key, vel: 0.5, rate: 48_000.0, seed: 0, rr: 0, len: 1.0 });
            let (mut l, mut r) = ([0f32; 1], [0f32; 1]);
            v.render(&mut l, &mut r);
            l[0]
        };
        assert_eq!((which(28.0), which(36.0), which(61.0), which(62.0), which(90.0)), (1.0, 2.0, 2.0, 3.0, 3.0));
        for (name, parts) in SECTIONS {
            assert!(parts.windows(2).all(|w| w[0].1 < w[1].1), "{name}: parts ascend");
            assert!(parts.iter().all(|(p, _)| INSTRUMENTS.iter().any(|i| i.0 == *p)), "{name}: every part is a known instrument");
        }
    }

    #[test]
    fn narrow_scales_the_side_only() {
        struct Wide;
        impl crate::voice::Voice for Wide {
            fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
                l.iter_mut().for_each(|x| *x += 0.75);
                r.iter_mut().for_each(|x| *x += -0.25);
                true
            }
            fn release(&mut self) {}
        }
        use crate::voice::Voice;
        let mut v = Narrow { v: Box::new(Wide), width: 0.5 };
        let (mut l, mut r) = ([0.1f32; 8], [0.1f32; 8]);
        assert!(v.render(&mut l, &mut r));
        // mid 0.25 stays, side 0.5 → 0.25; added onto what was there.
        assert!((l[0] - (0.1 + 0.5)).abs() < 1e-6 && (r[0] - (0.1 + 0.0)).abs() < 1e-6, "{} {}", l[0], r[0]);
    }

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
        for name in ["vsco-violins", "vsco-timpani", "salamander-piano", "splendid-piano", "wurlitzer", "finger-bass-yr", "picked-bass-yr", "nylon-guitar-fp", "clean-guitar-fsbs", "virtuosity-kit", "vsco-strings-section", "vsco-strings-spiccato"] {
            let Some(inst) = pack_instrument(name) else {
                eprintln!("{name}: pack not cached, skipped");
                continue;
            };
            let key = if name.contains("timpani") || name.contains("bass") { 43.0 } else if name.contains("kit") { 38.0 } else if name.contains("strings") { 67.0 } else { 69.0 };
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
