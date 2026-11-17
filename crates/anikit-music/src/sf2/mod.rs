//! Sample playback: SoundFont 2 / SF3 and SFZ instruments, the bundled
//! General MIDI bank, and optional downloadable packs.
//!
//! ```no_run
//! use anikit_music::sf2;
//! let piano = sf2::by_name("piano").unwrap();            // bundled GM bank
//! let kit = sf2::gm_drums(0);                              // standard kit
//! let font = sf2::SoundFont::from_file("my.sf2").unwrap(); // any SF2/SF3
//! let pad = font.instrument_by_name("pad");
//! let violins = sf2::pack_instrument("vsco-violins")      // cached pack…
//!     .or_else(|| sf2::by_name("strings"));               // …or GM fallback
//! ```
//!
//! - [`parse`]: RIFF/SF2/SF3 parser, generator and modulator resolution.
//! - [`engine`]: the voice engine (interpolation, loops, DAHDSR in dB,
//!   state-variable filter, LFOs, release layers).
//! - [`interp`]: cubic Hermite and 16-tap windowed-sinc kernels.
//! - [`sampler`]: [`SoundFont`] and its [`Instrument`](crate::Instrument)s.
//! - [`sfz`]: SFZ loader.
//! - [`bank`]: the bundled GeneralUser GS bank and friendly names.
//! - [`packs`]: optional large packs (VSCO-2 CE, Salamander Grand Piano).

pub mod bank;
pub mod engine;
pub mod interp;
pub mod packs;
pub mod parse;
pub mod sampler;
pub mod sfz;
#[cfg(test)]
mod tests;

pub use bank::{by_name, gm, gm_drums, gm_instrument, NAMES};
pub use interp::{set_interp, Interp};
pub use packs::{cache_dir, fetch, pack_instrument, PACKS};
pub use sampler::{Bytes, Sf2Instrument, SoundFont};
pub use sfz::{load_sfz, SfzInstrument, SfzOptions};
