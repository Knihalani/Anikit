//! Anikit Music: a fast, deterministic music engine for programmatic video.
//!
//! - [`sf2`]: SoundFont 2 / SF3 (Vorbis-compressed) and SFZ sampler with
//!   velocity layers, round robin, loops, release tails and per-voice
//!   filters, plus the bundled General MIDI bank.
//! - [`physical`]: Karplus-Strong strings and guitars, modal bells, mallets, kalimba.
//! - [`synth`]: a synth palette (pads, leads, basses, plucks, supersaws, 808s) and drum machines.
//! - [`theory`], [`notes`], [`arrange`], [`seq`], [`midi`]: keys, scales,
//!   chords, the note language, arrangement helpers, the sequencer, MIDI I/O.
//! - [`style`]: one-line generation of complete arrangements by genre.
//! - [`dsp`], [`mix`]: EQ, compression, sidechain, reverb, delay, the master
//!   bus (−14 LUFS, −1 dBTP) and stems.
//! - [`sfx`]: beat-alignable sound effects.
//!
//! ```no_run
//! let spec = anikit_music::MusicSpec::from_json(r#"{"style":"lofi","length":"20s"}"#).unwrap();
//! let out = anikit_music::render(&spec, &Default::default()).unwrap();
//! assert_eq!(out.rate, 48_000);
//! ```
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

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
pub mod instruments;
pub mod mix;
pub mod render;
pub mod custom;
pub mod compose;

pub use render::{arrange, render, render_arrangement, RenderOpts, Rendered, Stem, Timings};
pub use seq::Grid;
pub use spec::{Arrangement, MusicSpec};
pub use voice::{key_to_freq, Instrument, NoteOn, Rng, Voice, BLOCK, RATE};

/// The exact beat grid a spec will play on (tempo after fitting hits), without composing.
pub fn beat_grid(spec: &MusicSpec) -> Result<Grid, String> {
    let mut s = spec.clone();
    s.tracks.clear();
    s.length = Some(spec::TimeRef::Secs(spec.length.as_ref().map(|_| 1.0).unwrap_or(1.0)));
    s.sections.clear();
    s.midi = None;
    Ok(style::compose(&s, Some(1.0))?.grid)
}
