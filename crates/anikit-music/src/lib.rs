//! Anikit Music: a fast, deterministic music engine for programmatic video.
//!
//! - [`sf2`]: SoundFont 2 / SF3 (Vorbis-compressed) sampler with velocity
//!   layers, round robin, loops, release tails and per-voice filters, plus
//!   the bundled General MIDI bank.
//! - [`physical`]: Karplus-Strong strings and guitars, modal bells, mallets, kalimba.
//! - [`synth`]: a synth palette (pads, leads, basses, plucks, supersaws, 808s) and drum machines.
//! - [`theory`], [`notes`], [`seq`], [`midi`]: keys, scales, chords, a compact
//!   note language, the sequencer, MIDI import/export.
//! - [`style`]: one-line generation of complete arrangements by genre.
//! - [`dsp`], [`mix`]: EQ, compression, sidechain, reverb, delay, the master
//!   bus (−14 LUFS, −1 dBTP) and stems.
//! - [`sfx`]: beat-alignable sound effects.

pub mod voice;
pub mod dsp;
pub mod sf2;
pub mod physical;
pub mod synth;
pub mod sfx;
pub mod theory;
pub mod notes;
pub mod seq;
pub mod midi;
pub mod arrange;
pub mod spec;
pub mod style;

pub use voice::{key_to_freq, Instrument, NoteOn, Rng, Voice, BLOCK, RATE};
