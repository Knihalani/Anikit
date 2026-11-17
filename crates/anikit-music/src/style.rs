//! One-line generation: a style (genre) + bpm/key/mood/length/sections/hits
//! becomes a full arrangement — drums, bass, harmony, pads, arps, melodies,
//! risers into drops, impacts on hits — that the mixer then masters.
//!
//! Styles are data ([`StyleDef`]); the composer is generic. Sections carry an
//! energy (0..1) that switches layers on/off and picks drum patterns.

use crate::arrange::{self, ChordAt, Play};
use crate::notes::{self, Note};
use crate::seq::Grid;
use crate::spec::{self, closest, Arrangement, MusicSpec, Section, SectionSpec, SfxPlan, TrackMix, TrackPlan};
use crate::theory::Key;
use crate::voice::Rng;

/// How a layer generates notes.
#[derive(Debug, Clone, Copy)]
pub enum Gen {
    Drums,
    /// Chords with a play mode, voices, centre key.
    Chords(&'static str, usize, i32),
    /// Bassline pattern, octave.
    Bass(&'static str, i32),
    /// Melody: density (notes/beat), centre key, optional 16th rhythm for the motif.
    Melody(f64, i32, Option<&'static str>),
    /// Low hits on each bar's downbeat (taiko / timpani / sub booms): GM key or pitch, every n bars.
    Downbeats(i32, f64),
    /// A repeating 16th ostinato on chord tones: pattern of chord-tone indices ('0'..'3', '.' rest, 'o' octave root).
    Ostinato(&'static str, i32),
}

/// A layer of a style.
#[derive(Debug, Clone, Copy)]
pub struct Layer {
    pub role: &'static str,
    /// Instrument name; `|` separates fallbacks.
    pub inst: &'static str,
    pub gen: Gen,
    /// Plays in sections with energy in [min, max].
    pub min: f32,
    pub max: f32,
    /// Target level relative to the drums (LU): mix balance by loudness, not raw gain.
    pub level: f64,
    pub pan: f32,
    pub reverb: f32,
    pub delay: f32,
    /// Sidechain pump depth from the kick (dB).
    pub duck: f32,
    pub humanize: f32,
}

const fn layer(role: &'static str, inst: &'static str, gen: Gen, min: f32, level: f64) -> Layer {
    Layer { role, inst, gen, min, max: 1.01, level, pan: 0.0, reverb: 0.15, delay: 0.0, duck: 0.0, humanize: 0.25 }
}

/// Drum patterns for one energy tier: (voice, 16th grid per bar).
pub type Pattern = &'static [(&'static str, &'static str)];

/// A genre.
#[derive(Debug, Clone)]
pub struct StyleDef {
    pub name: &'static str,
    pub about: &'static str,
    pub bpm: f64,
    pub swing: f64,
    pub minor: bool,
    pub key_major: &'static str,
    pub key_minor: &'static str,
    /// Bars per chord.
    pub harmonic_rhythm: f64,
    pub prog_major: &'static [&'static str],
    pub prog_minor: &'static [&'static str],
    pub kit: &'static str,
    /// Drum tiers: low (energy < 0.45), mid (< 0.8), high.
    pub drums: [Pattern; 3],
    pub layers: &'static [Layer],
    pub reverb: &'static str,
    /// Riser + crash into high-energy sections, impact on hits.
    pub impact: &'static str,
    pub riser: bool,
    /// Section template for auto arrangement: (name, share of bars, energy).
    pub form: &'static [(&'static str, f64, f32)],
    /// Default master tone (EQ params on the master).
    pub drums_level: f64,
}

const P0: Pattern = &[];

pub static STYLES: &[StyleDef] = &[
    StyleDef {
        name: "lofi",
        about: "dusty swung hip-hop beat, jazzy 7th chords on worn keys, warm bass, sparse melody",
        bpm: 82.0,
        swing: 0.45,
        minor: true,
        key_major: "F",
        key_minor: "Dm",
        harmonic_rhythm: 1.0,
        prog_major: &["ii7 V7 Imaj7 vi7", "Imaj7 vi7 ii7 V7", "IVmaj7 iii7 ii7 Imaj7"],
        prog_minor: &["i7 iv7 VIImaj7 IIImaj7", "i9 iv7 i9 V7", "VImaj7 v7 i7 i7", "i7 VImaj7 iv7 v7"],
        kit: "drums:standard|drums",
        drums: [
            &[("kick", "x.........x....."), ("hat", "..x...x...x...x."), ("rim", "............x...")],
            &[("kick", "x.......x.x....."), ("snare", "....x.......x..."), ("hat", "x.o.x.o.x.o.x.oo")],
            &[("kick", "x.....x...x..x.."), ("snare", "....x.......x..o"), ("hat", "x.oxx.o.x.oxx.o."), ("shaker", "..x...x...x...x.")],
        ],
        layers: &[
            Layer { pan: 0.0, reverb: 0.18, humanize: 0.45, ..layer("keys", "rhodes|epiano", Gen::Chords("x.....x...x.....", 4, 62), 0.0, -5.0) },
            Layer { humanize: 0.3, ..layer("bass", "bass|bass-synth", Gen::Bass("synco", 2), 0.35, -3.0) },
            Layer { pan: 0.25, reverb: 0.3, delay: 0.2, humanize: 0.5, ..layer("lead", "vibraphone|kalimba|vibes", Gen::Melody(0.6, 74, None), 0.55, -7.0) },
            Layer { reverb: 0.4, ..layer("pad", "slow-strings|strings", Gen::Chords("pad", 3, 60), 0.0, -14.0) },
        ],
        reverb: "room",
        impact: "boom",
        riser: false,
        form: &[("intro", 0.2, 0.3), ("verse", 0.4, 0.6), ("hook", 0.3, 0.9), ("outro", 0.1, 0.3)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "house",
        about: "four-on-the-floor, offbeat hats, pumping sidechain, piano/organ stabs, rolling bass",
        bpm: 124.0,
        swing: 0.08,
        minor: true,
        key_major: "A",
        key_minor: "Am",
        harmonic_rhythm: 1.0,
        prog_major: &["vi IV I V", "I V vi IV", "IV V iii vi"],
        prog_minor: &["i7 VI III VII", "i7 iv7 VI V", "i7 VII VI VII", "i7 i7 iv7 iv7"],
        kit: "drums:house|drums:909|drums",
        drums: [
            &[("kick", "x...x...x...x..."), ("hat", "..x...x...x...x.")],
            &[("kick", "x...x...x...x..."), ("clap", "....x.......x..."), ("ohh", "..x...x...x...x."), ("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x...x...x...x..."), ("clap", "....x.......x..."), ("ohh", "..x...x...x...x."), ("hat", "xxxxxxxxxxxxxxxx"), ("shaker", "x.xxx.xxx.xxx.xx"), ("rim", "...x..x....x..x.")],
        ],
        layers: &[
            Layer { duck: 7.0, reverb: 0.2, ..layer("stabs", "stab-house|organ|piano", Gen::Chords("..x..x....x..x..", 4, 64), 0.5, -4.0) },
            Layer { duck: 6.0, humanize: 0.05, ..layer("bass", "bass-synth|synth-bass", Gen::Bass("house", 2), 0.4, -2.0) },
            Layer { duck: 8.0, reverb: 0.45, ..layer("pad", "pad-warm|strings-synth", Gen::Chords("pad", 4, 62), 0.0, -12.0) },
            Layer { duck: 4.0, reverb: 0.25, delay: 0.25, pan: 0.15, ..layer("lead", "pluck-bright|pluck", Gen::Melody(1.2, 76, Some("x..x..x...x..x..")), 0.85, -7.0) },
        ],
        reverb: "plate",
        impact: "impact",
        riser: true,
        form: &[("intro", 0.2, 0.4), ("build", 0.15, 0.7), ("drop", 0.45, 1.0), ("outro", 0.2, 0.45)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "synthwave",
        about: "80s retro: gated snare, octave-pumping bass, lush pads, arps and a saw lead",
        bpm: 104.0,
        swing: 0.0,
        minor: true,
        key_major: "C",
        key_minor: "Am",
        harmonic_rhythm: 1.0,
        prog_major: &["vi IV I V", "I V vi IV", "IV V vi vi"],
        prog_minor: &["i VI III VII", "i VI VII VII", "i iv VI V", "VI VII i i"],
        kit: "drums:808|drums",
        drums: [
            &[("kick", "x.......x......."), ("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x...x...x...x..."), ("snare", "....x.......x..."), ("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x...x...x...x..."), ("snare", "....X.......X..."), ("hat", "xxxxxxxxxxxxxxxx"), ("ohh", "..............x.")],
        ],
        layers: &[
            Layer { duck: 5.0, humanize: 0.0, ..layer("bass", "bass-synth|synth-bass", Gen::Bass("octave", 2), 0.35, -2.0) },
            Layer { duck: 6.0, reverb: 0.4, ..layer("pad", "pad-glass|pad-warm|strings-synth", Gen::Chords("pad", 4, 62), 0.0, -9.0) },
            Layer { duck: 4.0, reverb: 0.3, delay: 0.3, pan: -0.2, humanize: 0.0, ..layer("arp", "arp-pluck|pluck", Gen::Chords("arp-updown/16", 3, 67), 0.45, -9.0) },
            Layer { reverb: 0.35, delay: 0.3, pan: 0.1, ..layer("lead", "lead-saw|lead-square", Gen::Melody(0.9, 74, None), 0.8, -5.0) },
        ],
        reverb: "hall",
        impact: "impact",
        riser: true,
        form: &[("intro", 0.2, 0.35), ("verse", 0.3, 0.6), ("chorus", 0.4, 1.0), ("outro", 0.1, 0.35)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "trap",
        about: "half-time drums, rolling hi-hats, gliding 808 bass, dark bells and pads",
        bpm: 140.0,
        swing: 0.0,
        minor: true,
        key_major: "C",
        key_minor: "Cm",
        harmonic_rhythm: 2.0,
        prog_major: &["vi IV", "I vi"],
        prog_minor: &["i VI", "i iv", "i VI iv V", "i bII"],
        kit: "drums:trap|drums:808|drums",
        drums: [
            &[("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x.........x....."), ("clap", "........x......."), ("hat", "x.x.x.x.x.x.x3x.")],
            &[("kick", "x.....x...x...x."), ("clap", "........x......."), ("hat", "x.xxx.x.x3x.x.4."), ("ohh", "......x.........")],
        ],
        layers: &[
            Layer { humanize: 0.0, ..layer("808", "bass-808|synth-bass", Gen::Bass("808", 1), 0.45, 0.0) },
            Layer { reverb: 0.35, delay: 0.2, ..layer("bells", "bell-fm|music-box|glockenspiel", Gen::Melody(0.75, 79, Some("x..x..x.x..x..x.")), 0.0, -8.0) },
            Layer { reverb: 0.4, ..layer("pad", "pad-dark|strings", Gen::Chords("pad", 3, 60), 0.0, -12.0) },
        ],
        reverb: "hall",
        impact: "boom",
        riser: true,
        form: &[("intro", 0.25, 0.35), ("drop", 0.6, 1.0), ("outro", 0.15, 0.4)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "cinematic-hybrid",
        about: "trailer: taiko ostinatos, driving string ostinato, brass and choir, braams, risers, impacts",
        bpm: 100.0,
        swing: 0.0,
        minor: true,
        key_major: "D",
        key_minor: "Dm",
        harmonic_rhythm: 1.0,
        prog_major: &["I V vi IV", "vi IV I V"],
        prog_minor: &["i VI III VII", "i VI VII i", "i iv VI V", "i VI iv VII"],
        kit: "drums:orchestra|drums",
        drums: [
            &[("tom3", "x.........x.....")],
            &[("kick", "x.......x......."), ("tom3", "x..x..x...x..x.."), ("tom2", "......x.......x.")],
            &[("kick", "x..x..x.x..x..x."), ("tom3", "x.xx.xx.x.xx.xx."), ("tom2", "...x...x...x...x"), ("snare", "....X.......X..."), ("crash", "x...............")],
        ],
        layers: &[
            Layer { reverb: 0.25, humanize: 0.1, ..layer("ostinato", "strings|vsco-violins|strings-synth", Gen::Ostinato("0o0101o0101o0101", 50), 0.3, -3.0) },
            Layer { reverb: 0.35, ..layer("low", "cello|strings", Gen::Bass("root", 2), 0.0, -4.0) },
            Layer { reverb: 0.4, ..layer("brass", "french-horn|brass|brass-synth", Gen::Chords("pad", 3, 55), 0.8, -4.0) },
            Layer { reverb: 0.5, ..layer("choir", "choir|aahs|pad-choir", Gen::Chords("pad", 4, 64), 0.6, -9.0) },
            Layer { reverb: 0.35, humanize: 0.15, ..layer("piano", "piano", Gen::Ostinato("0..1..2.0..1..2.", 72), 0.0, -9.0) },
            Layer { reverb: 0.2, humanize: 0.0, ..layer("sub", "taiko|timpani", Gen::Downbeats(38, 2.0), 0.8, -3.0) },
        ],
        reverb: "hall",
        impact: "impact",
        riser: true,
        form: &[("intro", 0.25, 0.3), ("build", 0.25, 0.65), ("drop", 0.4, 1.0), ("outro", 0.1, 0.35)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "orchestral",
        about: "strings, woodwinds, horns, harp and timpani; film-score warmth",
        bpm: 92.0,
        swing: 0.0,
        minor: false,
        key_major: "D",
        key_minor: "Dm",
        harmonic_rhythm: 1.0,
        prog_major: &["I V vi IV", "I IV vi V", "IV I V vi", "I vi IV V"],
        prog_minor: &["i VI III VII", "i iv VI V", "i VI iv V"],
        kit: "drums:orchestra|drums",
        drums: [P0, &[("crash", "x...............")], &[("crash", "x...............")]],
        layers: &[
            Layer { reverb: 0.4, ..layer("strings", "strings|slow-strings|vsco-violins", Gen::Chords("pad", 4, 62), 0.0, 0.0) },
            Layer { reverb: 0.4, ..layer("basses", "contrabass|cello|strings", Gen::Bass("half", 2), 0.0, -4.0) },
            Layer { reverb: 0.4, pan: 0.3, ..layer("harp", "harp", Gen::Chords("arp-up/8", 4, 64), 0.3, -8.0) },
            Layer { reverb: 0.45, pan: -0.15, ..layer("melody", "flute|oboe|clarinet", Gen::Melody(0.7, 76, None), 0.3, -4.0) },
            Layer { reverb: 0.45, pan: 0.2, ..layer("horns", "french-horn|horn|brass", Gen::Chords("pad", 3, 55), 0.7, -5.0) },
            Layer { reverb: 0.4, humanize: 0.05, ..layer("timpani", "timpani", Gen::Downbeats(-1, 1.0), 0.75, -6.0) },
        ],
        reverb: "hall",
        impact: "boom",
        riser: false,
        form: &[("intro", 0.25, 0.3), ("theme", 0.35, 0.65), ("tutti", 0.3, 1.0), ("coda", 0.1, 0.3)],
        drums_level: -6.0,
    },
    StyleDef {
        name: "ambient",
        about: "slow evolving pads, glassy textures, sparse piano and bells, big reverb",
        bpm: 70.0,
        swing: 0.0,
        minor: false,
        key_major: "E",
        key_minor: "C#m",
        harmonic_rhythm: 2.0,
        prog_major: &["Imaj7 IVmaj7", "I V/7 vi IV", "IVmaj7 Imaj7 vi7 V"],
        prog_minor: &["i VI", "i7 VImaj7 IIImaj7 VII"],
        kit: "",
        drums: [P0, P0, P0],
        layers: &[
            Layer { reverb: 0.6, ..layer("pad", "pad-warm|warm-pad", Gen::Chords("pad", 4, 60), 0.0, 0.0) },
            Layer { reverb: 0.7, pan: 0.2, ..layer("glass", "pad-glass|strings-synth|pad-warm", Gen::Chords("pad", 3, 72), 0.4, -6.0) },
            Layer { reverb: 0.55, delay: 0.35, humanize: 0.5, ..layer("piano", "piano", Gen::Melody(0.35, 76, None), 0.25, -3.0) },
            Layer { reverb: 0.7, delay: 0.3, pan: -0.3, ..layer("bells", "crystal|celesta|glockenspiel", Gen::Ostinato("2.......1.......", 84), 0.6, -9.0) },
            Layer { reverb: 0.3, ..layer("sub", "bass-sub|synth-bass", Gen::Bass("root", 2), 0.5, -6.0) },
        ],
        reverb: "cathedral",
        impact: "swell",
        riser: false,
        form: &[("intro", 0.3, 0.3), ("bloom", 0.5, 0.75), ("fade", 0.2, 0.35)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "rock",
        about: "live kit, driving pick bass, power-chord guitars, overdriven lead",
        bpm: 128.0,
        swing: 0.0,
        minor: false,
        key_major: "E",
        key_minor: "Em",
        harmonic_rhythm: 1.0,
        prog_major: &["I V vi IV", "I IV V IV", "vi IV I V", "I bVII IV I"],
        prog_minor: &["i VII VI VII", "i VI VII i", "i iv VII III"],
        kit: "drums:rock|drums:power|drums",
        drums: [
            &[("kick", "x.......x......."), ("hat", "x.x.x.x.x.x.x.x."), ("rim", "....x.......x...")],
            &[("kick", "x.......x.x....."), ("snare", "....x.......x..."), ("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x.x.....x.x....."), ("snare", "....X.......X..."), ("ride", "x.x.x.x.x.x.x.x."), ("crash", "x...............")],
        ],
        layers: &[
            Layer { humanize: 0.15, ..layer("bass", "pick-bass|bass", Gen::Bass("rock", 2), 0.3, -2.0) },
            Layer { pan: -0.6, humanize: 0.15, ..layer("guitar", "overdrive-guitar|distortion-guitar", Gen::Chords("power", 3, 52), 0.6, -1.0) },
            Layer { pan: 0.6, humanize: 0.15, ..layer("guitar2", "distortion-guitar|overdrive-guitar", Gen::Chords("power", 3, 52), 0.85, -2.0) },
            Layer { pan: 0.4, reverb: 0.2, ..layer("clean", "clean-guitar|steel-guitar", Gen::Chords("strum", 5, 60), 0.0, -5.0) },
            Layer { reverb: 0.25, delay: 0.2, ..layer("lead", "overdrive-guitar|lead-saw", Gen::Melody(0.9, 71, None), 0.9, -4.0) },
        ],
        reverb: "room",
        impact: "hit",
        riser: false,
        form: &[("intro", 0.2, 0.4), ("verse", 0.35, 0.65), ("chorus", 0.35, 1.0), ("outro", 0.1, 0.4)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "pop",
        about: "modern pop: punchy drums, piano + synth chords, plucks, catchy top line",
        bpm: 112.0,
        swing: 0.0,
        minor: false,
        key_major: "G",
        key_minor: "Em",
        harmonic_rhythm: 1.0,
        prog_major: &["I V vi IV", "vi IV I V", "I vi IV V", "IV I V vi"],
        prog_minor: &["i VI III VII", "vi IV I V"],
        kit: "drums:room|drums",
        drums: [
            &[("kick", "x.......x......."), ("snap", "....x.......x...")],
            &[("kick", "x.....x.x......."), ("clap", "....x.......x..."), ("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x...x...x...x..."), ("clap", "....x.......x..."), ("hat", "x.xxx.xxx.xxx.xx"), ("ohh", "..............x.")],
        ],
        layers: &[
            Layer { reverb: 0.25, humanize: 0.2, ..layer("piano", "piano|bright-piano", Gen::Chords("x..x..x.x..x..x.", 4, 62), 0.0, -3.0) },
            Layer { duck: 3.0, ..layer("bass", "finger-bass|bass", Gen::Bass("pulse", 2), 0.4, -2.0) },
            Layer { duck: 5.0, reverb: 0.4, ..layer("pad", "strings", Gen::Chords("pad", 4, 64), 0.5, -10.0) },
            Layer { reverb: 0.25, delay: 0.25, pan: 0.2, ..layer("pluck", "clean-guitar|nylon-guitar", Gen::Melody(1.3, 76, Some("x.x..x.x..x.x...")), 0.8, -6.0) },
        ],
        reverb: "plate",
        impact: "impact",
        riser: true,
        form: &[("intro", 0.2, 0.35), ("verse", 0.3, 0.6), ("chorus", 0.4, 1.0), ("outro", 0.1, 0.35)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "corporate",
        about: "bright uplifting product-video bed: strummed guitar, piano 8ths, claps, glockenspiel",
        bpm: 116.0,
        swing: 0.0,
        minor: false,
        key_major: "C",
        key_minor: "Am",
        harmonic_rhythm: 1.0,
        prog_major: &["I V vi IV", "I IV vi V", "IV I V vi", "I iii IV V"],
        prog_minor: &["vi IV I V"],
        kit: "drums:pop|drums:909|drums",
        drums: [
            &[("clap", "....x.......x...")],
            &[("kick", "x.......x......."), ("clap", "....x.......x..."), ("shaker", "x.xxx.xxx.xxx.xx")],
            &[("kick", "x...x...x...x..."), ("clap", "....x.......x..."), ("hat", "..x...x...x...x."), ("shaker", "x.xxx.xxx.xxx.xx")],
        ],
        layers: &[
            Layer { pan: -0.35, humanize: 0.25, ..layer("guitar", "steel-guitar|nylon-guitar", Gen::Chords("strum", 5, 60), 0.0, -3.0) },
            Layer { pan: 0.25, reverb: 0.2, ..layer("piano", "piano|bright-piano", Gen::Chords("pulse/8", 3, 67), 0.3, -6.0) },
            Layer { ..layer("bass", "bass|pick-bass", Gen::Bass("pulse", 2), 0.4, -3.0) },
            Layer { reverb: 0.3, delay: 0.15, pan: 0.15, ..layer("glock", "glockenspiel|celesta|music-box", Gen::Melody(1.0, 84, Some("x.x.x...x.x.x...")), 0.7, -9.0) },
            Layer { reverb: 0.35, ..layer("strings", "strings|pad-warm", Gen::Chords("pad", 4, 64), 0.6, -11.0) },
        ],
        reverb: "plate",
        impact: "hit",
        riser: false,
        form: &[("intro", 0.2, 0.4), ("verse", 0.35, 0.65), ("chorus", 0.35, 1.0), ("outro", 0.1, 0.4)],
        drums_level: -1.0,
    },
    StyleDef {
        name: "tech",
        about: "clean modern tech/product: tight minimal beat, sub, glassy plucks and arps",
        bpm: 120.0,
        swing: 0.0,
        minor: true,
        key_major: "D",
        key_minor: "F#m",
        harmonic_rhythm: 2.0,
        prog_major: &["I V vi IV", "IV vi"],
        prog_minor: &["i VI", "i VII VI VII", "i iv"],
        kit: "drums:techno|drums:909|drums",
        drums: [
            &[("hat", "..x...x...x...x."), ("rim", "...x.....x......")],
            &[("kick", "x...x...x...x..."), ("hat", "..x...x...x...x."), ("rim", "...x..x....x..x.")],
            &[("kick", "x...x...x...x..."), ("clap", "....x.......x..."), ("hat", "xxxxxxxxxxxxxxxx"), ("ohh", "..x...x...x...x.")],
        ],
        layers: &[
            Layer { duck: 6.0, humanize: 0.0, ..layer("bass", "bass-sub|bass-synth", Gen::Bass("pulse", 2), 0.4, -2.0) },
            Layer { duck: 4.0, reverb: 0.3, delay: 0.35, humanize: 0.0, ..layer("arp", "arp-pluck|pluck", Gen::Chords("arp-up/16", 3, 69), 0.0, -6.0) },
            Layer { duck: 6.0, reverb: 0.45, ..layer("pad", "pad-glass|pad-warm", Gen::Chords("pad", 4, 62), 0.0, -12.0) },
            Layer { reverb: 0.3, delay: 0.3, ..layer("lead", "pluck-bright|bell-fm", Gen::Melody(0.8, 79, None), 0.85, -8.0) },
        ],
        reverb: "plate",
        impact: "impact",
        riser: true,
        form: &[("intro", 0.25, 0.4), ("build", 0.15, 0.65), ("main", 0.45, 1.0), ("outro", 0.15, 0.4)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "dnb",
        about: "drum & bass: fast breakbeat, reese bass, atmospheric pads",
        bpm: 172.0,
        swing: 0.0,
        minor: true,
        key_major: "F",
        key_minor: "Fm",
        harmonic_rhythm: 2.0,
        prog_major: &["vi IV"],
        prog_minor: &["i VI", "i VII VI VII", "i iv"],
        kit: "drums:909|drums",
        drums: [
            &[("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x.........x....."), ("snare", "....x.......x..."), ("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x.........xx...."), ("snare", "....x..o....x..o"), ("hat", "x.xxx.xxx.xxx.xx"), ("ride", "x...x...x...x...")],
        ],
        layers: &[
            Layer { duck: 4.0, humanize: 0.0, ..layer("bass", "bass-reese|bass-synth", Gen::Bass("root", 1), 0.5, -1.0) },
            Layer { reverb: 0.5, ..layer("pad", "pad-dark|pad-warm", Gen::Chords("pad", 4, 62), 0.0, -9.0) },
            Layer { reverb: 0.35, delay: 0.3, ..layer("lead", "pluck|bell-fm", Gen::Melody(0.6, 76, None), 0.8, -8.0) },
        ],
        reverb: "hall",
        impact: "impact",
        riser: true,
        form: &[("intro", 0.25, 0.4), ("drop", 0.6, 1.0), ("outro", 0.15, 0.4)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "piano",
        about: "solo piano: broken-chord left hand and a singing melody",
        bpm: 76.0,
        swing: 0.0,
        minor: false,
        key_major: "Eb",
        key_minor: "Cm",
        harmonic_rhythm: 1.0,
        prog_major: &["I V vi IV", "I vi IV V", "vi IV I V", "IV V iii vi"],
        prog_minor: &["i VI III VII", "i iv VI V"],
        kit: "",
        drums: [P0, P0, P0],
        layers: &[
            Layer { humanize: 0.5, reverb: 0.3, ..layer("left", "salamander-piano|piano", Gen::Chords("broken", 3, 55), 0.0, 0.0) },
            Layer { humanize: 0.5, reverb: 0.3, ..layer("right", "salamander-piano|piano", Gen::Melody(0.6, 74, None), 0.0, -1.0) },
        ],
        reverb: "hall",
        impact: "swell",
        riser: false,
        form: &[("intro", 0.25, 0.4), ("theme", 0.6, 0.8), ("coda", 0.15, 0.35)],
        drums_level: 0.0,
    },
    StyleDef {
        name: "acoustic",
        about: "warm folk: strummed and fingerpicked guitars, light percussion, upright feel",
        bpm: 96.0,
        swing: 0.15,
        minor: false,
        key_major: "G",
        key_minor: "Em",
        harmonic_rhythm: 1.0,
        prog_major: &["I V vi IV", "I IV I V", "vi IV I V", "I iii IV IV"],
        prog_minor: &["i VII VI VII", "i iv VII III"],
        kit: "drums:brush|drums:jazz|drums",
        drums: [
            &[("shaker", "x.xxx.xxx.xxx.xx")],
            &[("kick", "x.......x......."), ("rim", "....x.......x..."), ("shaker", "x.xxx.xxx.xxx.xx")],
            &[("kick", "x.....x.x......."), ("snare", "....x.......x..."), ("tamb", "..x...x...x...x."), ("shaker", "x.xxx.xxx.xxx.xx")],
        ],
        layers: &[
            Layer { pan: -0.3, humanize: 0.35, ..layer("strum", "steel-guitar|nylon-guitar", Gen::Chords("strum", 5, 60), 0.0, 0.0) },
            Layer { pan: 0.35, humanize: 0.35, reverb: 0.2, ..layer("picking", "nylon-guitar|steel-guitar|harp", Gen::Chords("arp-pinky/8", 4, 64), 0.3, -4.0) },
            Layer { ..layer("bass", "bass|fretless", Gen::Bass("half", 2), 0.4, -3.0) },
            Layer { reverb: 0.3, ..layer("melody", "harmonica|flute|violin", Gen::Melody(0.6, 74, None), 0.75, -5.0) },
        ],
        reverb: "room",
        impact: "hit",
        riser: false,
        form: &[("intro", 0.2, 0.4), ("verse", 0.35, 0.6), ("chorus", 0.35, 1.0), ("outro", 0.1, 0.35)],
        drums_level: -2.0,
    },
    StyleDef {
        name: "jazz",
        about: "swung piano trio: walking bass, ride and brushes, comping 7th chords",
        bpm: 132.0,
        swing: 0.6,
        minor: false,
        key_major: "Bb",
        key_minor: "Cm",
        harmonic_rhythm: 1.0,
        prog_major: &["ii7 V7 Imaj7 vi7", "Imaj7 vi7 ii7 V7", "iii7 vi7 ii7 V7"],
        prog_minor: &["iim7b5 V7 i7 i7"],
        kit: "drums:jazz|drums:brush|drums",
        drums: [
            &[("ride", "x..x.xx..x.xx..."), ("phh", "....x.......x...")],
            &[("ride", "x..x.xx..x.xx..."), ("phh", "....x.......x..."), ("kick", "o.......o.......")],
            &[("ride", "x..x.xx..x.xx..."), ("phh", "....x.......x..."), ("snare", "......o.......o."), ("kick", "o.......o.......")],
        ],
        layers: &[
            Layer { humanize: 0.4, ..layer("bass", "contrabass|bass|fretless", Gen::Bass("walk", 2), 0.0, -1.0) },
            Layer { humanize: 0.45, reverb: 0.2, ..layer("comp", "piano|epiano", Gen::Chords("...x.....x..x...", 4, 62), 0.0, -3.0) },
            Layer { humanize: 0.45, reverb: 0.25, ..layer("lead", "tenor-sax|sax|vibraphone", Gen::Melody(1.0, 70, None), 0.55, -3.0) },
        ],
        reverb: "room",
        impact: "hit",
        riser: false,
        form: &[("head", 0.4, 0.6), ("solo", 0.45, 0.9), ("out", 0.15, 0.5)],
        drums_level: -3.0,
    },
];

/// An instrumentation: which instrument plays each kind of part, whatever the style.
#[derive(Debug, Clone, Copy)]
pub struct Instrumentation {
    pub name: &'static str,
    /// Drum kit ("" = no drums).
    pub kit: &'static str,
    pub bass: &'static str,
    pub chords: &'static str,
    /// Pads and sustained beds ("" = dropped).
    pub pad: &'static str,
    pub lead: &'static str,
    /// Arpeggios, ostinatos, bells.
    pub arp: &'static str,
}

pub const INSTRUMENTATIONS: &[Instrumentation] = &[
    Instrumentation { name: "acoustic", kit: "drums:brush|drums", bass: "upright-bass|bass", chords: "acoustic-guitar|steel-guitar", pad: "strings", lead: "nylon-guitar", arp: "nylon-guitar" },
    Instrumentation { name: "orchestral", kit: "drums:orchestra|drums", bass: "contrabass|cello", chords: "pizzicato|strings", pad: "strings", lead: "violin|flute", arp: "harp" },
    Instrumentation { name: "band", kit: "drums:room|drums", bass: "pick-bass|bass", chords: "piano", pad: "tonewheel-organ|strings", lead: "clean-guitar", arp: "clean-guitar" },
    Instrumentation { name: "piano", kit: "", bass: "salamander-piano|piano", chords: "salamander-piano|piano", pad: "", lead: "salamander-piano|piano", arp: "salamander-piano|piano" },
    Instrumentation { name: "electronic", kit: "drums:909", bass: "bass-synth", chords: "stab-house", pad: "pad-warm", lead: "lead-saw", arp: "arp-pluck" },
];

impl Instrumentation {
    pub fn find(name: &str) -> Result<&'static Instrumentation, String> {
        let n = name.trim().to_ascii_lowercase();
        let n = match n.as_str() {
            "real" | "live" | "unplugged" | "folk" => "acoustic",
            "classical" | "strings" | "symphonic" => "orchestral",
            "rock" | "live-band" => "band",
            "solo-piano" => "piano",
            "synth" | "synths" | "edm" => "electronic",
            o => o,
        };
        INSTRUMENTATIONS.iter().find(|i| i.name == n).ok_or_else(|| {
            let names: Vec<&str> = INSTRUMENTATIONS.iter().map(|i| i.name).chain(["hybrid"]).collect();
            let hint = closest(&n, &names).map(|b| format!(" — did you mean '{b}'?")).unwrap_or_default();
            format!("music: instrumentation '{name}'{hint} (acoustic orchestral band piano electronic hybrid)")
        })
    }

    /// The instrument for a style layer's role; `None` drops the part.
    pub fn part(&self, role: &str) -> Option<&'static str> {
        let x = match role {
            "bass" | "808" | "sub" | "low" | "basses" => self.bass,
            "keys" | "stabs" | "piano" | "comp" | "guitar" | "guitar2" | "clean" | "strum" | "picking" | "left" => self.chords,
            "pad" | "glass" | "strings" | "choir" | "horns" | "brass" => self.pad,
            "arp" | "ostinato" | "bells" | "glock" | "timpani" => self.arp,
            _ => self.lead,
        };
        (!x.is_empty()).then_some(x)
    }
}

/// Aliases → style names.
const ALIASES: &[(&str, &str)] = &[
    ("cinematic", "cinematic-hybrid"),
    ("epic", "cinematic-hybrid"),
    ("trailer", "cinematic-hybrid"),
    ("hybrid", "cinematic-hybrid"),
    ("edm", "house"),
    ("dance", "house"),
    ("deep-house", "house"),
    ("techno", "tech"),
    ("minimal", "tech"),
    ("future", "tech"),
    ("chillhop", "lofi"),
    ("lo-fi", "lofi"),
    ("hiphop", "lofi"),
    ("hip-hop", "lofi"),
    ("chill", "lofi"),
    ("retro", "synthwave"),
    ("80s", "synthwave"),
    ("outrun", "synthwave"),
    ("classical", "orchestral"),
    ("film", "orchestral"),
    ("score", "orchestral"),
    ("drum-and-bass", "dnb"),
    ("drumandbass", "dnb"),
    ("upbeat", "corporate"),
    ("uplifting", "corporate"),
    ("folk", "acoustic"),
    ("solo-piano", "piano"),
    ("ballad", "piano"),
    ("swing", "jazz"),
    ("drill", "trap"),
    ("rap", "trap"),
    ("indie", "rock"),
    ("pad", "ambient"),
    ("drone", "ambient"),
];

/// Look up a style by name or alias.
pub fn find(name: &str) -> Result<&'static StyleDef, String> {
    let n = name.trim().to_ascii_lowercase().replace([' ', '_'], "-");
    let n = ALIASES.iter().find(|(a, _)| *a == n).map(|(_, b)| b.to_string()).unwrap_or(n);
    STYLES.iter().find(|s| s.name == n).ok_or_else(|| {
        let mut names: Vec<&str> = STYLES.iter().map(|s| s.name).collect();
        names.extend(ALIASES.iter().map(|a| a.0));
        let hint = closest(&n, &names).map(|b| format!(" — did you mean '{b}'?")).unwrap_or_default();
        format!("music: unknown style '{name}'{hint} (styles: {})", STYLES.iter().map(|s| s.name).collect::<Vec<_>>().join(", "))
    })
}

/// Mood words → (minor?, energy offset, density factor, tempo factor).
fn mood(m: Option<&str>) -> (Option<bool>, f32, f64, f64) {
    let m = m.unwrap_or("").to_ascii_lowercase();
    let has = |ws: &[&str]| ws.iter().any(|w| m.contains(w));
    let minor = if has(&["dark", "sad", "tense", "melanch", "mysterious", "ominous", "moody", "sinister", "suspense", "serious", "dramatic"]) {
        Some(true)
    } else if has(&["happy", "bright", "uplift", "joy", "hopeful", "optimis", "playful", "fun", "cheer", "sunny", "inspir", "positive"]) {
        Some(false)
    } else {
        None
    };
    let energy = if has(&["energetic", "aggressive", "intense", "epic", "hype", "driving", "powerful", "big"]) {
        0.12
    } else if has(&["calm", "chill", "soft", "gentle", "relax", "dreamy", "peaceful", "quiet", "minimal", "subtle"]) {
        -0.15
    } else {
        0.0
    };
    let density = if has(&["busy", "energetic", "playful"]) { 1.25 } else if has(&["sparse", "calm", "minimal", "gentle"]) { 0.7 } else { 1.0 };
    let tempo = if has(&["energetic", "hype", "fast", "driving"]) { 1.05 } else if has(&["calm", "slow", "relax", "dreamy"]) { 0.94 } else { 1.0 };
    (minor, energy, density, tempo)
}

/// Default energy for a section name.
pub fn section_energy(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    match n.as_str() {
        x if x.starts_with("intro") => 0.3,
        x if x.starts_with("verse") || x.starts_with("theme") || x.starts_with("head") => 0.55,
        x if x.starts_with("pre") || x.starts_with("build") || x.starts_with("rise") => 0.7,
        x if x.starts_with("drop") || x.starts_with("chorus") || x.starts_with("hook") || x.starts_with("tutti") || x.starts_with("climax") => 1.0,
        x if x.starts_with("break") || x.starts_with("bridge") => 0.4,
        x if x.starts_with("main") || x.starts_with("solo") || x.starts_with("bloom") => 0.85,
        x if x.starts_with("outro") || x.starts_with("end") || x.starts_with("coda") || x.starts_with("fade") || x.starts_with("out") => 0.3,
        _ => 0.6,
    }
}

/// Build the arrangement for a spec with a `style` (and/or explicit tracks).
/// `video_length` (seconds) is used when the spec has no `length`.
pub fn compose(spec: &MusicSpec, video_length: Option<f64>) -> Result<Arrangement, String> {
    let style = match &spec.style {
        Some(s) => Some(find(s)?),
        None => None,
    };
    let (mood_minor, e_off, density, tempo_f) = mood(spec.mood.as_deref());
    let instrumentation = match spec.instrumentation.as_deref() {
        None | Some("hybrid") | Some("default") => None,
        Some(name) => Some(Instrumentation::find(name)?),
    };
    let minor = mood_minor.or(style.map(|s| s.minor)).unwrap_or(false);
    let key = match &spec.key {
        Some(k) => Key::parse(k).map_err(|e| format!("music: {e}"))?,
        None => Key::parse(style.map(|s| if minor { s.key_minor } else { s.key_major }).unwrap_or(if minor { "Am" } else { "C" }))?,
    };
    let (meter, bpb) = spec::meter(spec.meter.as_deref())?;
    let mut bpm = spec.bpm.unwrap_or_else(|| style.map(|s| (s.bpm * tempo_f).round()).unwrap_or(120.0));
    if !(20.0..=400.0).contains(&bpm) {
        return Err(format!("music: bpm {bpm} is out of range (20–400)"));
    }
    let swing = spec.swing.unwrap_or(style.map(|s| s.swing).unwrap_or(0.0));
    let mut grid = Grid { bpm, beats_per_bar: bpb, swing, swing_unit: if style.is_some_and(|s| s.name == "jazz") { 0.5 } else { 0.25 }, offset: 0.0 };

    // Hits (seconds), then fit the tempo so they land on beats (downbeats preferred).
    let hits: Vec<f64> = spec.hits.iter().map(|h| h.secs(&grid)).collect::<Result<_, _>>()?;
    if !hits.is_empty() && spec.fit != Some(false) {
        bpm = fit_tempo(&hits, bpm, bpb);
        grid.bpm = bpm;
    }
    // Length.
    let length = match spec::length_secs(spec, &grid)? {
        Some(l) => l,
        None => match (video_length, spec.sections.is_empty()) {
            (Some(v), _) => v,
            (None, false) => spec.sections.iter().map(|s| sec_bars(s).unwrap_or(4.0)).sum::<f64>() * grid.bar(),
            (None, true) if style.is_some() => 8.0 * grid.bar(),
            (None, true) => 0.0,
        },
    };
    if !(0.0..=1800.0).contains(&length) {
        return Err(format!("music: length {length:.1}s is out of range (0–30 min)"));
    }
    let seed = spec.seed;
    let mut rng = Rng::new(seed ^ 0xA11CE);

    // Sections: explicit or from the style's form, with the first hit starting the peak.
    let total_bars = (length / grid.bar()).floor().max(if length > 0.0 { 1.0 } else { 0.0 });
    let hit_bars: Vec<f64> = hits.iter().map(|&h| (h / grid.bar()).round()).collect();
    let mut sections = if !spec.sections.is_empty() {
        explicit_sections(&spec.sections, total_bars)?
    } else if let Some(s) = style {
        auto_sections(s, total_bars, &hit_bars)
    } else {
        vec![Section { name: "main".into(), bar: 0.0, bars: total_bars, energy: 0.8 }]
    };
    for s in sections.iter_mut() {
        s.energy = (s.energy + e_off).clamp(0.0, 1.0);
    }

    // Chords: per section (each restarts the progression), looped.
    let prog_src = match &spec.chords {
        Some(c) => c.clone(),
        None => {
            let list = style.map(|s| if minor { s.prog_minor } else { s.prog_major }).unwrap_or(if minor { &["i VI III VII"][..] } else { &["I V vi IV"][..] });
            list[(seed as usize).wrapping_add(rng.below(list.len())) % list.len()].to_string()
        }
    };
    let hr = style.map(|s| s.harmonic_rhythm).unwrap_or(1.0);
    let base_prog = parse_prog(&prog_src, &key, bpb, if spec.chords.is_some() { 1.0 } else { hr })?;
    let mut chords: Vec<ChordAt> = Vec::new();
    let mut sec_chords: Vec<Vec<ChordAt>> = Vec::new();
    for s in &sections {
        let own = match spec.sections.iter().find_map(|x| match x {
            SectionSpec::Full(o) if o.name == s.name => o.chords.clone(),
            _ => None,
        }) {
            Some(c) => parse_prog(&c, &key, bpb, 1.0)?,
            None => base_prog.clone(),
        };
        let mut c = arrange::loop_chords(&own, s.bars * bpb);
        c.iter_mut().for_each(|x| x.t += s.bar * bpb);
        chords.extend(c.iter().cloned());
        sec_chords.push(c);
    }
    let total_beats = total_bars * bpb;

    // Layers.
    let mut tracks: Vec<TrackPlan> = Vec::new();
    let mut kicks: Vec<f64> = Vec::new();
    let mut sfx: Vec<SfxPlan> = Vec::new();
    if let Some(st) = style {
        let muted = |r: &str| spec.mute.iter().any(|m| m.eq_ignore_ascii_case(r));
        // Drums first (the 808 bass follows the kicks).
        let mut drum_notes: Vec<Note> = Vec::new();
        let has_kit = match &instrumentation {
            Some(i) => !i.kit.is_empty(),
            None => !st.kit.is_empty(),
        };
        if has_kit && !muted("drums") {
            for (si, s) in sections.iter().enumerate() {
                let tier = if s.energy < 0.45 { 0 } else if s.energy < 0.8 { 1 } else { 2 };
                let pat = st.drums[tier];
                let bar0 = s.bar * bpb;
                for (voice, p) in pat.iter() {
                    let k = notes::drum_key(voice).unwrap_or(36);
                    let mut ns = notes::parse_grid(p, k, 16, bpb, s.bars, 0.8)?;
                    // Build sections: drop the kick in the last bar for tension.
                    let next_e = sections.get(si + 1).map(|n| n.energy).unwrap_or(0.0);
                    if next_e > s.energy + 0.15 && s.bars >= 2.0 && (*voice == "kick" || *voice == "hat") {
                        ns.retain(|n| n.t < (s.bars - 1.0) * bpb);
                    }
                    ns.iter_mut().for_each(|n| n.t += bar0);
                    drum_notes.extend(ns);
                }
                // Fill into a bigger section: snare roll accelerating over the last bar.
                let next_e = sections.get(si + 1).map(|n| n.energy).unwrap_or(0.0);
                if next_e > s.energy + 0.15 && tier > 0 {
                    let t0 = bar0 + (s.bars - 1.0) * bpb;
                    let fill_key = if st.name == "cinematic-hybrid" { 45 } else { 38 };
                    let mut t = t0;
                    let mut step = 0.5;
                    while t < t0 + bpb - 1e-9 {
                        let f = ((t - t0) / bpb) as f32;
                        drum_notes.push(Note { t, len: step * 0.9, key: fill_key as f32, vel: 0.45 + 0.5 * f });
                        if t - t0 >= bpb * 0.5 {
                            step = 0.25;
                        }
                        if t - t0 >= bpb * 0.75 {
                            step = 0.125;
                        }
                        t += step;
                    }
                }
                // Crash on big section starts.
                if s.energy >= 0.6 && si > 0 && sections[si - 1].energy < s.energy {
                    drum_notes.push(Note { t: bar0, len: 2.0, key: 49.0, vel: 0.85 });
                }
            }
            kicks = drum_notes.iter().filter(|n| n.key == 36.0 || n.key == 35.0).map(|n| n.t).collect();
            kicks.sort_by(f64::total_cmp);
            let inst_override = spec.instruments.get("drums");
            let inst = match (inst_override, &instrumentation) {
                (Some(o), _) => o.clone(),
                (None, Some(i)) => i.kit.to_string(),
                (None, None) => st.kit.to_string(),
            };
            tracks.push(TrackPlan {
                name: "drums".into(),
                role: "drums".into(),
                inst,
                notes: drum_notes,
                mix: TrackMix { reverb: 0.1, lp: if st.name == "lofi" && inst_override.is_none() { 6500.0 } else { 0.0 }, ..Default::default() },
                humanize: 0.12,
                straight: false,
                fx: None,
                automate: None,
            });
        }
        let level_of = |l: &Layer| l.level;
        for (li, l) in st.layers.iter().enumerate() {
            if muted(l.role) {
                continue;
            }
            let mut ns: Vec<Note> = Vec::new();
            for (si, s) in sections.iter().enumerate() {
                if s.energy + 1e-6 < l.min || s.energy > l.max {
                    continue;
                }
                let ch = &sec_chords[si];
                if ch.is_empty() {
                    continue;
                }
                let lseed = seed ^ (li as u64 * 0x9E37) ^ (si as u64 * 0x85EB);
                let vel = (0.55 + 0.4 * s.energy).min(1.0);
                let mut part = match l.gen {
                    Gen::Drums => Vec::new(),
                    Gen::Chords(play, n, center) => arrange::play_chords(ch, &parse_play(play)?, center, n, vel, lseed),
                    Gen::Bass(p, oct) => {
                        let sk: Vec<f64> = kicks.iter().copied().filter(|&k| k >= s.bar * bpb && k < (s.bar + s.bars) * bpb).collect();
                        arrange::bassline(ch, p, &key, oct, vel, &sk, lseed)?
                    }
                    Gen::Melody(d, center, rhythm) => {
                        // A melody restarts with each section; local times from the section start.
                        let local: Vec<ChordAt> = ch.iter().map(|c| ChordAt { t: c.t - s.bar * bpb, ..c.clone() }).collect();
                        let mut m = arrange::melody(&local, &key, center, d * density, vel, rhythm, lseed ^ (seed.wrapping_mul(31)), bpb);
                        m.iter_mut().for_each(|n| n.t += s.bar * bpb);
                        m
                    }
                    Gen::Downbeats(k, every) => {
                        let mut v = Vec::new();
                        let mut b = 0.0;
                        while b < s.bars - 1e-9 {
                            let t = (s.bar + b) * bpb;
                            let key_n = if k < 0 {
                                // Timpani: the chord root in the low register.
                                let r = arrange::chord_at(ch, t).map(|c| c.chord.root).unwrap_or(key.tonic);
                                (36 + 12 + (r - 36 - 12).rem_euclid(12)) as f32 + if r > 7 { -12.0 } else { 0.0 }
                            } else {
                                let r = arrange::chord_at(ch, t).map(|c| c.chord.root).unwrap_or(key.tonic);
                                (k - (k - r).rem_euclid(12)) as f32 + if (k - r).rem_euclid(12) > 6 { 12.0 } else { 0.0 }
                            };
                            v.push(Note { t, len: every * bpb * 0.9, key: key_n, vel });
                            b += every;
                        }
                        v
                    }
                    Gen::Ostinato(pat, center) => ostinato(ch, pat, center, vel),
                };
                ns.append(&mut part);
            }
            if ns.is_empty() {
                continue;
            }
            let inst = match (spec.instruments.get(l.role), &instrumentation) {
                (Some(o), _) => o.clone(),
                (None, Some(i)) => match i.part(l.role) {
                    Some(x) => x.to_string(),
                    None => continue, // this instrumentation drops the part (e.g. pads under solo piano)
                },
                (None, None) => l.inst.to_string(),
            };
            tracks.push(TrackPlan {
                name: l.role.into(),
                role: l.role.into(),
                inst,
                notes: ns,
                mix: TrackMix { gain: level_of(l), pan: l.pan, reverb: l.reverb, delay: l.delay, duck: l.duck, ..Default::default() },
                humanize: l.humanize,
                straight: false,
                fx: None,
                automate: None,
            });
        }
        // Risers into big sections, impacts on hits and drops.
        for (si, s) in sections.iter().enumerate().skip(1) {
            let prev = &sections[si - 1];
            if st.riser && s.energy >= 0.85 && prev.energy < s.energy - 0.2 {
                let at = grid.secs(s.bar * bpb);
                let dur = (prev.bars.min(2.0) * grid.bar()).clamp(1.0, 8.0);
                sfx.push(SfxPlan { name: "riser".into(), at, gain: -6.0, params: serde_json::json!({ "duration": dur, "seed": seed }) });
                sfx.push(SfxPlan { name: st.impact.into(), at, gain: -5.0, params: serde_json::json!({ "seed": seed }) });
                if st.name == "cinematic-hybrid" {
                    sfx.push(SfxPlan { name: "braam".into(), at, gain: -6.0, params: serde_json::json!({ "seed": seed, "pitch": (key.tonic as f64 + 2.0).rem_euclid(12.0) - 2.0 }) });
                }
            }
        }
        for &h in &hits {
            if sfx.iter().any(|x| (x.at - h).abs() < 0.05) {
                continue;
            }
            sfx.push(SfxPlan { name: st.impact.into(), at: h, gain: -4.0, params: serde_json::json!({ "seed": seed }) });
        }
    }
    // Explicit tracks.
    let song_chords = if chords.is_empty() { None } else { Some(&chords[..]) };
    for t in spec::compile_tracks(spec, &grid, &key, song_chords, if total_beats > 0.0 { Some(total_beats) } else { None })? {
        if let Some(i) = tracks.iter().position(|x| x.name == t.name) {
            tracks[i] = t;
        } else {
            tracks.push(t);
        }
    }
    for c in &spec.sfx {
        let mut params = serde_json::Map::new();
        for (k, v) in &c.params {
            params.insert(k.clone(), v.clone());
        }
        sfx.push(SfxPlan { name: c.sfx.clone(), at: c.at.secs(&grid)?, gain: c.gain.unwrap_or(0.0), params: serde_json::Value::Object(params) });
    }
    // Content-defined length when nothing set it.
    let mut length = length;
    if length <= 0.0 {
        let end_beat = tracks.iter().flat_map(|t| t.notes.iter()).map(|n| n.t + n.len).fold(0.0f64, f64::max);
        let end = sfx.iter().map(|s| s.at + 1.0).fold(grid.secs(end_beat) + 1.5, f64::max);
        if end_beat <= 0.0 && sfx.is_empty() {
            return Err("music: nothing to play — give a \"style\", \"tracks\", \"sfx\" or \"midi\"".into());
        }
        length = end;
    }
    // Final chord: when time remains after the last whole bar, ring the tonic out.
    let end = spec.end.clone().unwrap_or_else(|| "ring".into());
    if !matches!(end.as_str(), "ring" | "fade" | "cut") {
        return Err(format!("music: end '{end}': use ring, fade or cut"));
    }
    if style.is_some() && end == "ring" && total_beats > 0.0 && length - grid.secs(total_beats) > 0.4 {
        let t = total_beats;
        let tonic = arrange::ChordAt { chord: crate::theory::Chord { root: key.tonic, intervals: if key.is_minor() { vec![0, 3, 7] } else { vec![0, 4, 7] }, bass: None }, t, len: 8.0 };
        for tr in tracks.iter_mut() {
            let tail: Vec<Note> = match tr.role.as_str() {
                "drums" => vec![Note { t, len: 2.0, key: 36.0, vel: 0.9 }, Note { t, len: 4.0, key: 49.0, vel: 0.8 }],
                "bass" | "808" | "low" | "basses" | "sub" => vec![Note { t, len: 6.0, key: (36 + key.tonic - if key.tonic > 7 { 12 } else { 0 }) as f32, vel: 0.8 }],
                "lead" | "melody" | "bells" | "glock" | "pluck" | "arp" | "right" | "ostinato" | "timpani" => continue,
                _ => arrange::play_chords(std::slice::from_ref(&tonic), &Play::Pad, 62, 4, 0.7, seed),
            };
            tr.notes.extend(tail);
        }
    }
    Ok(Arrangement {
        grid,
        meter,
        key,
        style: style.map(|s| s.name.to_string()).unwrap_or_else(|| "custom".into()),
        sections,
        tracks,
        sfx,
        master: spec.master.clone(),
        kicks: kicks.iter().map(|&b| grid.swung(b)).collect(),
        length,
        seed,
        end,
        reverb: spec.master.reverb.clone().unwrap_or_else(|| style.map(|s| s.reverb).unwrap_or("hall").to_string()),
        defines: spec.define.clone(),
    })
}

fn parse_play(s: &str) -> Result<Play, String> {
    Play::parse(s)
}

/// A progression where each chord lasts `bars` bars unless given explicit durations.
fn parse_prog(s: &str, key: &Key, bpb: f64, bars: f64) -> Result<Vec<ChordAt>, String> {
    let mut c = arrange::chord_line(s, key, bpb).map_err(|e| format!("music: {e}"))?;
    if (bars - 1.0).abs() > 1e-9 && !s.contains([':', '/']) {
        let mut t = 0.0;
        for x in c.iter_mut() {
            x.t = t;
            x.len *= bars;
            t += x.len;
        }
    }
    Ok(c)
}

/// Ostinato over chord tones: '0'..'3' chord-tone index, 'o' root an octave down, '.' rest.
fn ostinato(chords: &[ChordAt], pat: &str, center: i32, vel: f32) -> Vec<Note> {
    let cells: Vec<char> = pat.chars().collect();
    let mut out = Vec::new();
    for c in chords {
        let v = c.chord.voice(3, center);
        let root = v.iter().copied().find(|k| k.rem_euclid(12) == c.chord.root).unwrap_or(v[0]);
        let mut t = c.t;
        let mut i = 0;
        while t < c.t + c.len - 1e-9 {
            let ch = cells[i % cells.len()];
            let k = match ch {
                'o' => Some(root - 12),
                d @ '0'..='3' => Some(v[(d as usize - '0' as usize).min(v.len() - 1)]),
                _ => None,
            };
            if let Some(k) = k {
                let accent = if i % 4 == 0 { 1.0 } else { 0.8 };
                out.push(Note { t, len: 0.22, key: k as f32, vel: vel * accent });
            }
            t += 0.25;
            i += 1;
        }
    }
    out
}

fn sec_bars(s: &SectionSpec) -> Option<f64> {
    match s {
        SectionSpec::Short(x) => x.split_once(':').and_then(|(_, b)| b.trim().parse().ok()),
        SectionSpec::Full(o) => o.bars,
    }
}

fn explicit_sections(list: &[SectionSpec], total_bars: f64) -> Result<Vec<Section>, String> {
    let mut out = Vec::new();
    let mut fixed = 0.0;
    let mut free = 0;
    for s in list {
        match sec_bars(s) {
            Some(b) => fixed += b,
            None => free += 1,
        }
    }
    let share = if free > 0 { ((total_bars - fixed) / free as f64).max(1.0).floor() } else { 0.0 };
    let mut bar = 0.0;
    for s in list {
        let (name, energy) = match s {
            SectionSpec::Short(x) => (x.split(':').next().unwrap_or("main").trim().to_string(), None),
            SectionSpec::Full(o) => (o.name.clone(), o.energy),
        };
        let bars = sec_bars(s).unwrap_or(share);
        if bars <= 0.0 {
            return Err(format!("music: section '{name}' has no bars"));
        }
        out.push(Section { energy: energy.unwrap_or_else(|| section_energy(&name)), name, bar, bars });
        bar += bars;
    }
    if total_bars > 0.0 && bar < total_bars {
        // Extend the last section to the end.
        if let Some(l) = out.last_mut() {
            l.bars += total_bars - bar;
        }
    }
    Ok(out)
}

/// Sections from the style's form, scaled to `bars`; the first hit (if any) starts the peak section.
pub fn auto_sections(st: &StyleDef, bars: f64, hit_bars: &[f64]) -> Vec<Section> {
    if bars <= 0.0 {
        return Vec::new();
    }
    if bars <= 4.0 {
        let peak = st.form.iter().map(|f| f.2).fold(0.0f32, f32::max);
        return vec![Section { name: "main".into(), bar: 0.0, bars, energy: (peak * 0.85).max(0.6) }];
    }
    // Integer bars per section, at least 1, rounded to even counts when long enough.
    let mut sizes: Vec<f64> = st.form.iter().map(|f| (f.1 * bars).round().max(1.0)).collect();
    let peak = st.form.iter().enumerate().max_by(|a, b| a.1 .2.total_cmp(&b.1 .2)).map(|x| x.0).unwrap_or(0);
    // Align the peak start to the first hit.
    if let Some(&h) = hit_bars.iter().find(|&&h| h >= 1.0 && h < bars) {
        let before: f64 = sizes[..peak].iter().sum();
        if before > 0.0 {
            let scale = h / before;
            for s in sizes[..peak].iter_mut() {
                *s = (*s * scale).round().max(1.0);
            }
            let now: f64 = sizes[..peak].iter().sum();
            sizes[peak - 1] += h - now;
            if sizes[peak - 1] < 1.0 {
                sizes[peak - 1] = 1.0;
            }
        }
    }
    let sum: f64 = sizes.iter().sum();
    let diff = bars - sum;
    sizes[peak] = (sizes[peak] + diff).max(1.0);
    // If still over (tiny lengths), trim from the end.
    let mut total: f64 = sizes.iter().sum();
    let mut i = sizes.len();
    while total > bars && i > 0 {
        i -= 1;
        let cut = (total - bars).min(sizes[i] - 1.0).max(0.0);
        sizes[i] -= cut;
        total -= cut;
        if cut == 0.0 && sizes[i] <= 1.0 && total > bars {
            sizes.remove(i);
            total -= 1.0;
        }
    }
    let mut out = Vec::new();
    let mut bar = 0.0;
    for (f, &n) in st.form.iter().zip(&sizes) {
        if n <= 0.0 {
            continue;
        }
        out.push(Section { name: f.0.into(), bar, bars: n, energy: f.2 });
        bar += n;
    }
    out
}

/// Nudge the tempo within ±8 % so the hits land on beats (downbeats preferred).
pub fn fit_tempo(hits: &[f64], bpm: f64, bpb: f64) -> f64 {
    let cost = |b: f64| {
        let beat = 60.0 / b;
        hits.iter()
            .map(|&h| {
                let k = (h / beat).round();
                let on_bar = (k / bpb).fract().abs() < 1e-9;
                (h - k * beat).abs() + if on_bar { 0.0 } else { 0.004 }
            })
            .sum::<f64>()
            + (b - bpm).abs() / bpm * 0.05
    };
    let mut best = bpm;
    let mut bc = cost(bpm);
    // Exact candidates: tempos that put a hit exactly on beat k.
    for &h in hits {
        if h <= 0.0 {
            continue;
        }
        for k in 1..=((h * bpm * 1.1 / 60.0).ceil() as usize) {
            let b = 60.0 * k as f64 / h;
            if (b / bpm - 1.0).abs() <= 0.08 {
                let c = cost(b);
                if c < bc - 1e-12 {
                    bc = c;
                    best = b;
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_composes() {
        for s in STYLES {
            let spec = MusicSpec { style: Some(s.name.into()), length: Some(crate::spec::TimeRef::Str("20s".into())), ..Default::default() };
            let a = compose(&spec, None).unwrap_or_else(|e| panic!("{}: {e}", s.name));
            assert!(!a.tracks.is_empty(), "{}", s.name);
            assert!(a.tracks.iter().all(|t| !t.notes.is_empty()), "{}", s.name);
            assert_eq!(a.length, 20.0);
            let bars: f64 = a.sections.iter().map(|x| x.bars).sum();
            assert_eq!(bars, (20.0 / a.grid.bar()).floor(), "{}: {:?}", s.name, a.sections);
            // Everything stays in MIDI range.
            for t in &a.tracks {
                assert!(t.notes.iter().all(|n| (0.0..=127.0).contains(&n.key)), "{} {}", s.name, t.name);
            }
        }
        assert!(find("lof").unwrap_err().contains("did you mean 'lofi'"));
        assert_eq!(find("Epic").unwrap().name, "cinematic-hybrid");
    }

    #[test]
    fn hits_fit_the_tempo_and_start_the_drop() {
        let spec = MusicSpec::from_json(r#"{"style":"house","bpm":128,"length":"15s","hits":["@2.0","@7.5"]}"#).unwrap();
        let a = compose(&spec, None).unwrap();
        for h in [2.0, 7.5] {
            let k = (h / a.grid.beat()).round();
            assert!((k * a.grid.beat() - h).abs() < 0.03, "hit {h} vs beat grid {:.2} bpm", a.grid.bpm);
            assert!(a.sfx.iter().any(|s| (s.at - h).abs() < 1e-9 || (s.at - h).abs() < 0.03));
        }
        assert!((a.grid.bpm / 128.0 - 1.0).abs() <= 0.08);
        // A single hit at 2 s at ~128 bpm → exact.
        let b = fit_tempo(&[2.0], 128.0, 4.0);
        let k = (2.0 / (60.0 / b)).round();
        assert!((k * 60.0 / b - 2.0).abs() < 1e-9, "{b}");
    }

    #[test]
    fn sections_follow_hits() {
        let st = find("house").unwrap();
        let s = auto_sections(st, 16.0, &[8.0]);
        let drop = s.iter().find(|x| x.name == "drop").unwrap();
        assert_eq!(drop.bar, 8.0);
        assert_eq!(s.iter().map(|x| x.bars).sum::<f64>(), 16.0);
        for b in [1.0, 2.0, 3.0, 5.0, 7.0, 9.0, 33.0] {
            let s = auto_sections(st, b, &[]);
            assert_eq!(s.iter().map(|x| x.bars).sum::<f64>(), b, "{b}: {s:?}");
        }
    }
}
