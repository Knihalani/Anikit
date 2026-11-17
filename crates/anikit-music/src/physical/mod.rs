//! Physical models: waveguide plucked strings ([`karplus`]) and modal
//! bars, bells, tines and plates ([`modal`]). Available by name through
//! [`preset`] (see [`PRESETS`]).

// Index loops over fixed-size mode arrays are deliberate: they vectorise.
#![allow(clippy::needless_range_loop)]

pub mod karplus;
pub mod modal;

use crate::voice::Instrument;
use std::sync::Arc;

/// Preset names accepted by [`preset`].
pub const PRESETS: &[&str] = &[
    // strings
    "nylon-guitar",
    "steel-guitar",
    "electric-guitar",
    "muted-guitar",
    "pluck-bass",
    "harp",
    "koto",
    "pizz",
    "banjo",
    // modal
    "marimba",
    "vibraphone",
    "xylophone",
    "glockenspiel",
    "celesta",
    "kalimba",
    "music-box",
    "tubular-bells",
    "church-bell",
    "bell",
    "handpan",
    "steel-drum",
    "wood-block",
    "crystal",
];

/// A physical-model preset by name (see [`PRESETS`]).
pub fn preset(name: &str) -> Option<Arc<dyn Instrument>> {
    if let Some(s) = karplus::spec(name) {
        return Some(Arc::new(karplus::PluckedString(s)));
    }
    modal::spec(name).map(|s| Arc::new(modal::Modal(s)) as Arc<dyn Instrument>)
}
