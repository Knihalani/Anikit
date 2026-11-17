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
            Layer { pan: 0.0, reverb: 0.18, humanize: 0.45, ..layer("keys", "wurlitzer|rhodes|epiano", Gen::Chords("x.....x...x.....", 4, 62), 0.0, -5.0) },
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
        swing: 0.2,
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
            &[("kick", "x.........x....."), ("clap", "........x......."), ("hat", "x.x.x.x.x.x.x.x.")],
            &[("kick", "x.....x...x...x."), ("clap", "........x......."), ("hat", "x.xxx.x.x.xxx.x."), ("ohh", "......x.........")],
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
        kit: "drums:cinematic|drums:orchestra|drums",
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
            Layer { reverb: 0.2, humanize: 0.0, ..layer("sub", "bass-808|bass-sub", Gen::Downbeats(26, 2.0), 0.8, -3.0) },
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
        swing: 0.12,
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
        swing: 0.85,
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
            let hint = closest(n, &names).map(|b| format!(" — did you mean '{b}'?")).unwrap_or_default();
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
    // A ringing ending gets room: the last downbeat (the "button") lands early
    // enough for the final chord to ring out (15 % of the length, 1–3 s).
    let ringing = style.is_some() && spec.end.as_deref().unwrap_or("ring") == "ring" && spec.sections.is_empty();
    let ring_room = if ringing { (length * 0.15).clamp(1.0, 3.0) } else { 0.0 };
    let total_bars = ((length - ring_room) / grid.bar()).floor().max(if length > 0.0 { 1.0 } else { 0.0 });
    let hit_bars: Vec<f64> = hits.iter().map(|&h| (h / grid.bar()).round()).collect();
    let mut sections = if !spec.sections.is_empty() {
        explicit_sections(&spec.sections, total_bars)?
    } else if let Some(s) = style {
        let mut secs = auto_sections(s, total_bars, &hit_bars);
        if spec.follow_cuts != Some(false) && !spec.cuts.is_empty() {
            snap_to_cuts(&mut secs, &spec.cuts.iter().map(|&c| c / grid.bar()).collect::<Vec<_>>(), total_bars);
        }
        secs
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
    let has_section_chords = spec.sections.iter().any(|x| matches!(x, SectionSpec::Full(o) if o.chords.is_some()));
    if let (Some(st), None, false) = (style, &spec.chords, has_section_chords) {
        // The composition engine: progressions per section role, cadences, colour, inversions.
        let list = if minor { st.prog_minor } else { st.prog_major };
        let progs: Vec<Vec<ChordAt>> = list.iter().map(|p| parse_prog(p, &key, bpb, hr)).collect::<Result<_, _>>()?;
        let secs: Vec<(String, f64, f64)> = sections.iter().map(|s| (s.name.clone(), s.bar, s.bars)).collect();
        sec_chords = crate::compose::section_chords(&progs, &secs, &key, bpb, &crate::compose::harmony(st.name), seed.wrapping_add(rng.below(1 << 16) as u64));
        chords = sec_chords.iter().flatten().cloned().collect();
    }
    let engine = !sec_chords.is_empty();
    for s in sections.iter().filter(|_| !engine) {
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
        let underscore = spec.underscore == Some(true);
        let muted = |r: &str| {
            spec.mute.iter().any(|m| m.eq_ignore_ascii_case(r))
                || (underscore && matches!(r, "lead" | "melody" | "right" | "pluck" | "bells" | "glock" | "counter"))
        };
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
                // Variation inside the section: small fills every 4 bars, bigger every 8,
                // ghost snares, open-hat lifts, crashes every 8 bars at full energy.
                let next_e = sections.get(si + 1).map(|n| n.energy).unwrap_or(0.0);
                let into_bigger = next_e > s.energy + 0.15;
                let has = |v: &str| pat.iter().any(|(x, _)| *x == v);
                let mut vr = Rng::new(seed ^ 0xD5u64 ^ (si as u64) << 8);
                let nb = s.bars as usize;
                for b in 0..nb {
                    let t0 = bar0 + b as f64 * bpb;
                    let last = b + 1 == nb;
                    if tier > 0 && (has("snare") || has("clap")) && st.name != "trap" {
                        // Ghost notes on two 16ths.
                        for _ in 0..2 {
                            let pos = [0.75, 1.75, 2.25, 3.25, 3.75][vr.below(5)];
                            if pos < bpb {
                                drum_notes.push(Note { t: t0 + pos, len: 0.1, key: 38.0, vel: 0.22 + 0.08 * vr.f32() });
                            }
                        }
                    }
                    if last && into_bigger {
                        continue; // the roll below takes the last bar
                    }
                    if st.name == "trap" && tier > 0 && b % 2 == 1 {
                        // Hat rolls at phrase ends: triplet 16ths, then 32nds over the last beat.
                        drum_notes.retain(|n| !(n.key == 42.0 && n.t >= t0 + bpb - 1.0 - 1e-9 && n.t < t0 + bpb - 1e-9));
                        let per = if b % 4 == 3 { 8 } else { 6 };
                        for i in 0..per {
                            let f = i as f32 / per as f32;
                            drum_notes.push(Note { t: t0 + bpb - 1.0 + i as f64 / per as f64, len: 0.08, key: 42.0, vel: 0.45 + 0.4 * f });
                        }
                    }
                    if tier > 0 && b % 8 == 7 {
                        // Tom/snare fill over the last beat.
                        let toms: [f32; 4] = if st.name == "cinematic-hybrid" { [45.0, 45.0, 43.0, 41.0] } else { [38.0, 50.0, 47.0, 45.0] };
                        for (i, k) in toms.iter().enumerate() {
                            drum_notes.retain(|n| !(n.t >= t0 + bpb - 1.0 - 1e-9 && n.t < t0 + bpb - 1e-9 && (n.key == 42.0 || n.key == 46.0)));
                            drum_notes.push(Note { t: t0 + bpb - 1.0 + i as f64 * 0.25, len: 0.2, key: *k, vel: 0.6 + 0.1 * i as f32 });
                        }
                    } else if tier > 0 && b % 4 == 3 {
                        // Small lift: open hat on the last off-beat, snare pickup.
                        if has("hat") || has("ohh") {
                            drum_notes.push(Note { t: t0 + bpb - 0.5, len: 0.4, key: 46.0, vel: 0.7 });
                        }
                        if has("snare") {
                            drum_notes.push(Note { t: t0 + bpb - 0.25, len: 0.1, key: 38.0, vel: 0.5 });
                        }
                    }
                    if tier == 2 && b % 8 == 0 && b > 0 {
                        drum_notes.push(Note { t: t0, len: 2.0, key: 49.0, vel: 0.75 });
                    }
                }
                // Trailer pulse: builds into a climax tick on a low tom whose subdivision
                // doubles every third of the section (quarters → 8ths → 16ths), growing louder.
                if st.name == "cinematic-hybrid" && into_bigger && crate::compose::Role::of(&s.name) == crate::compose::Role::Build && s.bars >= 3.0 {
                    let span = (s.bars - 1.0) * bpb; // the last bar is the fill
                    let mut t = 0.0;
                    while t < span - 1e-9 {
                        let f = t / span;
                        drum_notes.push(Note { t: bar0 + t, len: 0.2, key: 41.0, vel: (0.3 + 0.45 * f) as f32 });
                        t += if f < 1.0 / 3.0 { 1.0 } else if f < 2.0 / 3.0 { 0.5 } else { 0.25 };
                    }
                }
                // Fill into a bigger section: snare roll accelerating over the last bar.
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
            // Synthwave's snare is the 80s gated one: its own track through a plate
            // whose tail is cut a quarter second after each hit.
            let gated = st.name == "synthwave" && inst_override.is_none();
            let snare: Vec<Note> = if gated { drum_notes.iter().filter(|n| n.key == 38.0 || n.key == 40.0).cloned().collect() } else { Vec::new() };
            if !snare.is_empty() {
                drum_notes.retain(|n| n.key != 38.0 && n.key != 40.0);
            }
            let snare_inst = inst.clone();
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
            if !snare.is_empty() {
                tracks.push(TrackPlan {
                    name: "snare".into(),
                    role: "drums".into(),
                    inst: snare_inst,
                    notes: snare,
                    // Tracks are loudness-matched, so the snare alone sits 9 dB under the kit
                    // (measured: the backbeat balance stays within 0.3 dB of the unsplit kit).
                    mix: TrackMix { gain: -9.0, ..Default::default() },
                    humanize: 0.0,
                    straight: false,
                    fx: Some(serde_json::json!([{ "type": "gated-reverb", "preset": "plate", "wet": 4.0, "hold": 0.22, "release": 0.025 }])),
                    automate: None,
                });
            }
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
                        // Same section role → same tune (the chorus hook returns); verses share theirs.
                        let role_seed = seed.wrapping_mul(31) ^ (li as u64 * 0x9E37) ^ (crate::compose::Role::of(&s.name) as u64 * 0x51ED);
                        let mut m = crate::compose::melody(&local, &key, center, d * density, vel, rhythm, role_seed, bpb, si + 1 == sections.len());
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
        // Performance and image: hats accented by position, a lazy backbeat in
        // hip-hop styles, sustain pedal on piano-family chords, stereo width per role.
        let electronic = matches!(st.name, "house" | "synthwave" | "trap" | "tech" | "dnb" | "ambient");
        for tr in tracks.iter_mut() {
            if tr.role == "drums" {
                for n in tr.notes.iter_mut() {
                    let pos = (n.t * 4.0).round() as i64 % 4; // 16th within the beat
                    if n.key == 42.0 || n.key == 44.0 || n.key == 82.0 || n.key == 51.0 {
                        n.vel *= [1.0, 0.62, 0.85, 0.6][pos as usize];
                    }
                    if matches!(st.name, "lofi" | "jazz") && (n.key == 38.0 || n.key == 39.0) && n.vel > 0.4 {
                        n.t += 0.02; // laid-back backbeat (≈ 15 ms at 82 bpm)
                    }
                }
                // Played kits sound in a room: a short stereo ambience gives them the
                // width overheads and room mics give a real kit (panning alone leaves each drum mono).
                if tr.fx.is_none() && !electronic {
                    tr.fx = Some(serde_json::json!([{ "type": "reverb", "preset": "ambience", "wet": 0.25 }]));
                }
                continue;
            }
            let piano_like = ["piano", "rhodes", "epiano", "keys", "salamander"].iter().any(|p| tr.inst.contains(p));
            if piano_like && matches!(tr.role.as_str(), "keys" | "piano" | "comp" | "left" | "stabs") {
                // Sustain pedal: chord notes ring to the next chord change.
                for n in tr.notes.iter_mut() {
                    if let Some(c) = arrange::chord_at(&chords, n.t + 1e-6) {
                        let end = c.t + c.len;
                        if end > n.t + n.len {
                            n.len = (end - n.t) * 0.99;
                        }
                    }
                }
            }
            if tr.mix.pan == 0.0 {
                // A stereo stage: parts get their own places instead of all sitting in the centre.
                tr.mix.pan = match tr.role.as_str() {
                    "keys" | "piano" | "comp" | "left" => -0.3,
                    "stabs" => 0.2,
                    "lead" | "melody" | "right" | "pluck" => 0.12,
                    "arp" | "ostinato" | "glock" | "harp" => 0.35,
                    "bells" | "brass" | "horns" => -0.25,
                    "choir" => 0.2,
                    "strum" | "guitar" | "clean" => -0.4,
                    "guitar2" | "picking" => 0.4,
                    _ => 0.0,
                };
            }
            // Mono keys and beds get real stereo from a gentle ensemble/chorus (the classic
            // Rhodes and pad treatment) in styles whose parts are otherwise centre-heavy.
            if tr.fx.is_none() && !electronic && matches!(tr.role.as_str(), "keys" | "pad" | "comp" | "piano" | "strings" | "choir" | "picking") {
                let mix = if tr.role == "keys" || tr.role == "comp" { 0.35 } else { 0.25 };
                let chorus = serde_json::json!({ "type": "chorus", "mode": "ensemble", "rate": 0.6, "depth": 2.5, "spread": 1.0, "mix": mix });
                // Electric pianos get their stereo tremolo (a mono sample panned stays mono).
                let ep = ["wurli", "rhodes", "epiano", "cp80", "pianet"].iter().any(|p| tr.inst.contains(p));
                tr.fx = Some(if ep {
                    serde_json::json!([{ "type": "autopan", "note": "1/4", "depth": 0.55 }, chorus])
                } else {
                    serde_json::json!([chorus])
                });
            }
            tr.mix.width = match tr.role.as_str() {
                "pad" | "glass" | "choir" | "strings" => 1.5,
                "arp" | "ostinato" | "bells" | "harp" => 1.35,
                "keys" | "stabs" | "piano" | "comp" | "strum" | "picking" => 1.25,
                _ => 1.0,
            };
        }
        // Countermelody: a guide-tone line under the lead in peak sections.
        if !muted("counter") && st.layers.iter().any(|l| matches!(l.gen, Gen::Melody(..))) {
            let mut ns = Vec::new();
            for (si, s) in sections.iter().enumerate() {
                if crate::compose::Role::of(&s.name) == crate::compose::Role::Peak && s.bars >= 4.0 {
                    ns.extend(crate::compose::counter(&sec_chords[si], 62, 0.6));
                }
            }
            if !ns.is_empty() {
                let inst = spec.instruments.get("counter").cloned().unwrap_or_else(|| {
                    match &instrumentation {
                        Some(i) => i.pad.to_string(),
                        None if electronic => "strings-synth".to_string(),
                        None => "slow-strings|strings".to_string(),
                    }
                });
                if !inst.is_empty() {
                    tracks.push(TrackPlan {
                        name: "counter".into(),
                        role: "counter".into(),
                        inst,
                        notes: ns,
                        mix: TrackMix { gain: -12.0, pan: -0.25, width: 1.3, reverb: 0.35, duck: if electronic { 4.0 } else { 0.0 }, ..Default::default() },
                        humanize: 0.3,
                        straight: false,
                        fx: None,
                        automate: None,
                    });
                }
            }
        }
        // Builds in electronic styles sweep a low-pass open into the drop.
        if electronic {
            let builds: Vec<(f64, f64)> = sections
                .iter()
                .enumerate()
                .filter(|(i, s)| crate::compose::Role::of(&s.name) == crate::compose::Role::Build && sections.get(i + 1).is_some_and(|n| n.energy > s.energy))
                .map(|(_, s)| (grid.secs(s.bar * bpb), grid.secs((s.bar + s.bars) * bpb)))
                .collect();
            if !builds.is_empty() {
                let mut keys = vec![serde_json::json!(["@0", 20000.0])];
                for (a, b) in &builds {
                    keys.push(serde_json::json!([format!("@{}", a - 0.001), 20000.0]));
                    keys.push(serde_json::json!([format!("@{a}"), 450.0]));
                    keys.push(serde_json::json!([format!("@{}", b - 0.01), 9000.0]));
                    keys.push(serde_json::json!([format!("@{b}"), 20000.0]));
                }
                for tr in tracks.iter_mut().filter(|t| matches!(t.role.as_str(), "pad" | "stabs" | "arp" | "keys" | "glass")) {
                    tr.automate = Some(serde_json::json!({ "lp": keys }));
                }
            }
        }
        // A stop before each drop: everything rests for the last half beat (the riser peaks into silence).
        if st.riser {
            let stops: Vec<f64> = sections
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(i, s)| s.energy >= 0.85 && sections[i - 1].energy < s.energy - 0.2)
                .map(|(_, s)| s.bar * bpb)
                .collect();
            for tr in tracks.iter_mut() {
                for &p in &stops {
                    let w0 = p - 0.5;
                    tr.notes.retain(|n| !(n.t >= w0 - 1e-9 && n.t < p - 1e-9));
                    for n in tr.notes.iter_mut().filter(|n| n.t < w0 && n.t + n.len > w0) {
                        n.len = (w0 - n.t).max(0.05);
                    }
                }
            }
        }
        // A false ending: a climax that falls into a break stops dead — a short hit on
        // the last beat, then two beats of silence (only the reverb rings) before the
        // break starts. With a reverse cymbal into the next drop this is the trailer
        // "hole before act III".
        if st.riser {
            let holes: Vec<f64> = sections
                .windows(2)
                .filter(|w| crate::compose::Role::of(&w[0].name) == crate::compose::Role::Peak && w[0].energy >= 0.8 && crate::compose::Role::of(&w[1].name) == crate::compose::Role::Break && w[0].bars >= 2.0)
                .map(|w| w[1].bar * bpb)
                .collect();
            for &p in &holes {
                let (w0, w1) = (p - 1.0, p + 1.0);
                for tr in tracks.iter_mut() {
                    tr.notes.retain(|n| !(n.t >= w0 - 1e-9 && n.t < w1 - 1e-9));
                    for n in tr.notes.iter_mut().filter(|n| n.t < w0 && n.t + n.len > w0) {
                        n.len = (w0 - n.t).max(0.05);
                    }
                }
                sfx.push(SfxPlan { name: "hit".into(), at: grid.secs(w0) - grid.offset, gain: -6.0, params: serde_json::json!({ "seed": seed }) });
            }
            if !holes.is_empty() {
                if let Some(last) = sections.iter().rev().find(|s| crate::compose::Role::of(&s.name) == crate::compose::Role::Peak && s.bar * bpb > holes[0]) {
                    sfx.push(SfxPlan { name: "reverse-cymbal".into(), at: grid.secs(last.bar * bpb) - grid.offset, gain: -9.0, params: serde_json::json!({ "seed": seed }) });
                }
            }
        }
        // Picture cuts: a cymbal accent on the beat nearest each cut (≥ 2 bars apart).
        if spec.follow_cuts != Some(false) {
            if let Some(dr) = tracks.iter_mut().find(|t| t.role == "drums") {
                let mut last = f64::NEG_INFINITY;
                for &c in &spec.cuts {
                    let b = (c / grid.beat()).round();
                    let t = grid.secs(b) - grid.offset;
                    if (t - c).abs() <= 0.3 * grid.beat() && b - last >= 2.0 * bpb && b < total_beats {
                        if !dr.notes.iter().any(|n| (n.t - b).abs() < 0.26 && (n.key == 49.0 || n.key == 57.0)) {
                            dr.notes.push(Note { t: b, len: 2.0, key: 57.0, vel: 0.62 });
                        }
                        last = b;
                    }
                }
            }
        }
        // A cut that opens a bigger section gets a reverse cymbal swelling into it
        // (risers already cover drops in riser styles).
        if spec.follow_cuts != Some(false) {
            for (si, s) in sections.iter().enumerate().skip(1) {
                let at = grid.secs(s.bar * bpb);
                let rise = s.energy > sections[si - 1].energy + 0.15;
                let riser_here = st.riser && s.energy >= 0.85 && sections[si - 1].energy < s.energy - 0.2;
                if rise && !riser_here && spec.cuts.iter().any(|&c| (c - at).abs() <= 0.3 * grid.beat()) {
                    sfx.push(SfxPlan { name: "reverse-cymbal".into(), at, gain: -12.0, params: serde_json::json!({ "seed": seed }) });
                }
            }
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
                "drums" if tr.name == "drums" => vec![Note { t, len: 2.0, key: 36.0, vel: 0.9 }, Note { t, len: 4.0, key: 49.0, vel: 0.8 }],
                "drums" => continue,
                "bass" | "808" | "low" | "basses" | "sub" => vec![Note { t, len: 6.0, key: (36 + key.tonic - if key.tonic > 7 { 12 } else { 0 }) as f32, vel: 0.8 }],
                "lead" | "melody" | "right" => vec![Note { t, len: 4.0, key: (72 + key.tonic - if key.tonic > 7 { 12 } else { 0 }) as f32, vel: 0.75 }],
                "bells" | "glock" | "pluck" | "arp" | "ostinato" | "timpani" | "counter" => continue,
                _ => arrange::play_chords(std::slice::from_ref(&tonic), &Play::Pad, 62, 4, 0.7, seed),
            };
            tr.notes.extend(tail);
        }
        if let Some(st) = style {
            let button = grid.secs(t) - grid.offset;
            // Short pieces end on a hit: an impact under the button and a breath before it.
            if length <= 30.0 && !sfx.iter().any(|x| (x.at - button).abs() < 0.05) {
                sfx.push(SfxPlan { name: st.impact.into(), at: button, gain: -9.0, params: serde_json::json!({ "seed": seed }) });
            }
            if length <= 20.0 && t >= 2.0 {
                let w0 = t - 0.5;
                for tr in tracks.iter_mut() {
                    tr.notes.retain(|n| !(n.t >= w0 - 1e-9 && n.t < t - 1e-9));
                    for n in tr.notes.iter_mut().filter(|n| n.t < w0 && n.t + n.len > w0) {
                        n.len = (w0 - n.t).max(0.05);
                    }
                }
            }
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
/// Three-act trailer form: act I (intro), act II (build → climax), a false
/// ending (break), act III (rise → drop).
const TRAILER_FORM: &[(&str, f64, f32)] = &[("intro", 0.16, 0.3), ("build", 0.2, 0.6), ("climax", 0.16, 0.85), ("break", 0.12, 0.2), ("rise", 0.12, 0.7), ("drop", 0.24, 1.0)];

pub fn auto_sections(st: &StyleDef, bars: f64, hit_bars: &[f64]) -> Vec<Section> {
    if bars <= 0.0 {
        return Vec::new();
    }
    if bars < 8.0 {
        // Short forms (idents, stings, 5–15 s spots): still an arc — set-up, lift, payoff.
        let (pi, peak) = st.form.iter().enumerate().max_by(|a, b| a.1 .2.total_cmp(&b.1 .2)).map(|(i, f)| (i, f.2)).unwrap_or((0, 1.0));
        let peak_name = st.form.get(pi).map(|f| f.0).unwrap_or("main");
        let intro_e = st.form.first().map(|f| f.2).unwrap_or(0.35);
        let plan: Vec<(&str, f64, f32)> = match bars as usize {
            0 | 1 => vec![(peak_name, bars, peak)],
            2 => vec![("build", 1.0, 0.7), (peak_name, 1.0, peak)],
            3 => vec![("intro", 1.0, intro_e), ("build", 1.0, 0.7), (peak_name, 1.0, peak)],
            4 => vec![("intro", 1.0, intro_e), ("build", 1.0, 0.7), (peak_name, 2.0, peak)],
            n => vec![("intro", 1.0, intro_e), ("build", 2.0, 0.7), (peak_name, (n - 3) as f64, peak)],
        };
        let mut bar = 0.0;
        return plan
            .into_iter()
            .map(|(n, b, e)| {
                let s = Section { name: n.into(), bar, bars: b, energy: e };
                bar += b;
                s
            })
            .collect();
    }
    // Long trailers (18+ bars, about 45 s at 100 bpm) get three acts: a first
    // climax, a false ending that drops to a lone piano, then a bigger act III.
    let form: &[(&str, f64, f32)] = if st.name == "cinematic-hybrid" && bars >= 18.0 { TRAILER_FORM } else { st.form };
    // Integer bars per section, at least 1, rounded to even counts when long enough.
    let mut sizes: Vec<f64> = form.iter().map(|f| (f.1 * bars).round().max(1.0)).collect();
    let peak = form.iter().enumerate().max_by(|a, b| a.1 .2.total_cmp(&b.1 .2)).map(|x| x.0).unwrap_or(0);
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
    for (f, &n) in form.iter().zip(&sizes) {
        if n <= 0.0 {
            continue;
        }
        out.push(Section { name: f.0.into(), bar, bars: n, energy: f.2 });
        bar += n;
    }
    out
}

/// Move section boundaries onto picture cuts (in bars) when a cut lies within
/// two bars of a boundary, keeping every section at least two bars long.
pub fn snap_to_cuts(secs: &mut [Section], cuts: &[f64], total: f64) {
    for i in 1..secs.len() {
        let b = secs[i].bar;
        let Some(&c) = cuts.iter().map(|c| c.round()).collect::<Vec<_>>().iter().filter(|&&c| (c - b).abs() <= 2.0 && c > 0.0).min_by(|x, y| (*x - b).abs().total_cmp(&(*y - b).abs())) else { continue };
        let lo = secs[i - 1].bar + 2.0;
        let hi = secs[i].bar + secs[i].bars - 2.0;
        if c >= lo && c <= hi && (c - b).abs() > 1e-9 {
            let d = c - b;
            secs[i - 1].bars += d;
            secs[i].bar = c;
            secs[i].bars -= d;
        }
    }
    let _ = total;
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
            // The button lands early enough for a 3 s ring-out (15 % of 20 s).
            assert_eq!(bars, ((20.0 - 3.0) / a.grid.bar()).floor(), "{}: {:?}", s.name, a.sections);
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
        for b in [1.0, 2.0, 3.0, 4.0, 5.0, 7.0, 9.0, 33.0] {
            let s = auto_sections(st, b, &[]);
            assert_eq!(s.iter().map(|x| x.bars).sum::<f64>(), b, "{b}: {s:?}");
        }
    }
}

#[cfg(test)]
mod instrumentation_tests {
    use super::*;
    use crate::instruments::source;

    fn insts(json: &str) -> Vec<(String, String)> {
        let a = compose(&MusicSpec::from_json(json).unwrap(), None).unwrap();
        a.tracks.iter().map(|t| (t.role.clone(), t.inst.clone())).collect()
    }

    #[test]
    fn non_electronic_styles_use_real_instruments() {
        for s in ["lofi", "pop", "orchestral", "rock", "corporate", "acoustic", "jazz", "piano"] {
            for (role, inst) in insts(&format!(r#"{{"style":"{s}","length":"30s"}}"#)) {
                let k = source(&inst);
                assert!(!k.starts_with("synth"), "{s}: {role} uses {inst} ({k})");
            }
        }
    }

    #[test]
    fn instrumentation_swaps_every_part() {
        for (inst, synth_ok) in [("acoustic", false), ("orchestral", false), ("band", false), ("piano", false), ("electronic", true)] {
            let t = insts(&format!(r#"{{"style":"house","instrumentation":"{inst}","length":"30s"}}"#));
            assert!(!t.is_empty());
            for (role, i) in &t {
                let k = source(i);
                assert!(synth_ok || !k.starts_with("synth"), "{inst}: {role} uses {i} ({k})");
            }
            if inst == "piano" {
                assert!(t.iter().all(|(r, _)| r != "drums" && r != "pad"), "{t:?}");
            }
        }
        // Per-role overrides still win.
        let t = insts(r#"{"style":"house","instrumentation":"band","instruments":{"lead":"violin"},"length":"30s"}"#);
        assert!(t.iter().any(|(r, i)| r == "lead" && i == "violin"));
        let e = compose(&MusicSpec::from_json(r#"{"style":"house","instrumentation":"acustic"}"#).unwrap(), None).unwrap_err();
        assert!(e.contains("did you mean 'acoustic'"), "{e}");
    }
}

#[cfg(test)]
mod cut_tests {
    use super::*;

    #[test]
    fn sections_and_accents_follow_picture_cuts() {
        // 120 bpm, 4/4: a bar is 2 s. The chorus would start at bar 10 (20 s); a cut at
        // 18.0 s (bar 9) is within two bars, so the chorus moves onto it.
        let spec = MusicSpec::from_json(r#"{"style":"pop","bpm":120,"length":"40s","cuts":[13.9, 18.0]}"#).unwrap();
        let a = compose(&spec, None).unwrap();
        assert!(a.sections.iter().any(|s| s.bar == 9.0 && s.name == "chorus"), "{:?}", a.sections);
        let d = a.tracks.iter().find(|t| t.role == "drums").unwrap();
        assert!(d.notes.iter().any(|n| n.t == 28.0 && n.key == 57.0), "cymbal on the beat nearest 13.9 s");
        // Pop already has a riser into that chorus: no extra swell.
        assert!(!a.sfx.iter().any(|x| x.name == "reverse-cymbal"));
        let off = MusicSpec::from_json(r#"{"style":"pop","bpm":120,"length":"40s","cuts":[13.9],"followCuts":false}"#).unwrap();
        let b = compose(&off, None).unwrap();
        assert!(!b.tracks.iter().find(|t| t.role == "drums").unwrap().notes.iter().any(|n| n.key == 57.0));
    }

    #[test]
    fn cuts_into_bigger_sections_get_a_reverse_swell() {
        let plain = compose(&MusicSpec::from_json(r#"{"style":"acoustic","bpm":120,"length":"40s"}"#).unwrap(), None).unwrap();
        let bpb = plain.grid.beats_per_bar;
        let w = plain.sections.windows(2).find(|w| w[1].energy > w[0].energy + 0.15).expect("a rising boundary");
        let t = plain.grid.secs(w[1].bar * bpb);
        let a = compose(&MusicSpec::from_json(&format!(r#"{{"style":"acoustic","bpm":120,"length":"40s","cuts":[{t}]}}"#)).unwrap(), None).unwrap();
        assert!(a.sfx.iter().any(|x| x.name == "reverse-cymbal" && (x.at - t).abs() < 1e-6));
        let off = compose(&MusicSpec::from_json(&format!(r#"{{"style":"acoustic","bpm":120,"length":"40s","cuts":[{t}],"followCuts":false}}"#)).unwrap(), None).unwrap();
        assert!(!off.sfx.iter().any(|x| x.name == "reverse-cymbal"));
        assert!(!plain.sfx.iter().any(|x| x.name == "reverse-cymbal"), "no cuts, no swell");
    }
}

#[cfg(test)]
mod underscore_tests {
    use super::*;

    #[test]
    fn underscore_drops_the_melody_but_keeps_the_bed() {
        let full = compose(&MusicSpec::from_json(r#"{"style":"pop","length":"30s"}"#).unwrap(), None).unwrap();
        let under = compose(&MusicSpec::from_json(r#"{"style":"pop","length":"30s","underscore":true}"#).unwrap(), None).unwrap();
        assert!(full.tracks.iter().any(|t| t.role == "pluck" || t.role == "counter"));
        assert!(!under.tracks.iter().any(|t| matches!(t.role.as_str(), "pluck" | "lead" | "counter")));
        assert!(under.tracks.iter().any(|t| t.role == "drums") && under.tracks.iter().any(|t| t.role == "piano"));
    }
}

#[cfg(test)]
mod trailer_tests {
    use super::*;

    #[test]
    fn long_trailers_have_three_acts_a_hole_and_a_pulse_ramp() {
        let a = compose(&MusicSpec::from_json(r#"{"style":"cinematic-hybrid","length":"60s","seed":2}"#).unwrap(), None).unwrap();
        let names: Vec<&str> = a.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["intro", "build", "climax", "break", "rise", "drop"]);
        let bpb = a.grid.beats_per_bar;
        let brk = a.sections.iter().find(|s| s.name == "break").unwrap().bar * bpb;
        // Two beats of silence around the climax → break boundary, a hit at its start.
        for t in &a.tracks {
            assert!(!t.notes.iter().any(|n| n.t >= brk - 1.0 && n.t < brk + 1.0), "{} plays in the hole", t.name);
            assert!(!t.notes.iter().any(|n| n.t < brk - 1.0 && n.t + n.len > brk - 1.0 + 1e-6), "{} rings into the hole", t.name);
        }
        assert!(a.sfx.iter().any(|x| x.name == "hit" && (x.at - a.grid.secs(brk - 1.0)).abs() < 1e-6));
        let drop = a.sections.iter().find(|s| s.name == "drop").unwrap().bar * bpb;
        assert!(a.sfx.iter().any(|x| x.name == "reverse-cymbal" && (x.at - a.grid.secs(drop)).abs() < 1e-6));
        // The build's pulse gets faster: first gap a beat, last a 16th.
        let b = a.sections.iter().find(|s| s.name == "build").unwrap();
        let mut pulse: Vec<f64> = a.tracks.iter().find(|t| t.role == "drums").unwrap().notes.iter().filter(|n| n.key == 41.0 && n.t >= b.bar * bpb && n.t < (b.bar + b.bars - 1.0) * bpb).map(|n| n.t).collect();
        pulse.sort_by(f64::total_cmp);
        assert!(pulse.len() > 8, "{pulse:?}");
        assert!((pulse[1] - pulse[0] - 1.0).abs() < 1e-9 && (pulse[pulse.len() - 1] - pulse[pulse.len() - 2] - 0.25).abs() < 1e-9);
        // Short cues keep the simple form.
        let s = compose(&MusicSpec::from_json(r#"{"style":"cinematic-hybrid","length":"30s"}"#).unwrap(), None).unwrap();
        assert!(!s.sections.iter().any(|x| x.name == "break"));
    }

    #[test]
    fn synthwave_snare_is_gated_on_its_own_track() {
        let a = compose(&MusicSpec::from_json(r#"{"style":"synthwave","length":"30s"}"#).unwrap(), None).unwrap();
        let sn = a.tracks.iter().find(|t| t.name == "snare").expect("snare track");
        assert!(sn.notes.iter().all(|n| n.key == 38.0 || n.key == 40.0) && !sn.notes.is_empty());
        assert!(sn.fx.as_ref().unwrap().to_string().contains("gated-reverb"));
        let kit = a.tracks.iter().find(|t| t.name == "drums").unwrap();
        assert!(!kit.notes.iter().any(|n| n.key == 38.0 || n.key == 40.0));
        // Other styles keep one drum track.
        let p = compose(&MusicSpec::from_json(r#"{"style":"pop","length":"30s"}"#).unwrap(), None).unwrap();
        assert_eq!(p.tracks.iter().filter(|t| t.role == "drums").count(), 1);
    }
}
