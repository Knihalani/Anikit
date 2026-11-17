//! A modern synth palette and synthesised drum machines.
//!
//! - [`osc`]: PolyBLEP/PolyBLAMP saw, pulse (PWM), triangle, sine, supersaw
//!   lanes and analog drift; [`fm`]: DX-style FM pairs; [`organ`]: drawbar
//!   tonewheel organ.
//! - [`filter`]: ZDF ladder (4-pole, drive) and ZDF SVF; [`env`]: ADSR,
//!   LFO, mono glide.
//! - [`patch`]: the subtractive voice the analog-style presets are built on.
//! - [`drums`]: drum machines keyed by General MIDI drum keys ([`kit`]).
//!
//! Everything is available by name through [`preset`] (see [`PRESETS`]).

// Index loops over fixed-size lane arrays are deliberate: they vectorise.
#![allow(clippy::needless_range_loop)]

pub mod drums;
pub mod env;
pub mod filter;
pub mod fm;
pub mod organ;
pub mod osc;
pub mod patch;
pub(crate) mod util;

pub use drums::{kit, KITS};
#[doc(hidden)]
pub use util::force_baseline;

use crate::voice::Instrument;
use env::LfoShape;
use fm::{FmPair, FmPatch, FmSynth};
use organ::{Organ, OrganPatch};
use osc::Wave;
use patch::{env as adsr, FilterKind, Osc, Patch, SubSynth};
use std::sync::Arc;

/// Preset names accepted by [`preset`].
pub const PRESETS: &[&str] = &[
    "pad-warm",
    "pad-glass",
    "pad-dark",
    "pad-choir",
    "strings-synth",
    "lead-saw",
    "lead-square",
    "lead-soft",
    "pluck",
    "pluck-bright",
    "bell-fm",
    "epiano-fm",
    "organ",
    "bass-sub",
    "bass-reese",
    "bass-808",
    "bass-acid",
    "bass-synth",
    "bass-wobble",
    "brass-synth",
    "arp-pluck",
    "chip-square",
    "supersaw",
    "stab-house",
    "keys-lofi",
];

/// A synth preset by name (see [`PRESETS`]).
pub fn preset(name: &str) -> Option<Arc<dyn Instrument>> {
    if let Some(p) = subtractive(name) {
        return Some(Arc::new(SubSynth::new(p)));
    }
    if let Some(p) = fm_patch(name) {
        return Some(Arc::new(FmSynth(p)));
    }
    if let Some(p) = organ_patch(name) {
        return Some(Arc::new(Organ(p)));
    }
    None
}

/// The subtractive patch behind a preset name, for tweaking.
pub fn subtractive(name: &str) -> Option<Patch> {
    let mut p = Patch::base();
    match name {
        "pad-warm" => {
            p.filter = FilterKind::Lp4;
            p.osc = [
                Some(Osc::new(Wave::Saw, 4, 14.0, 0.8).width(0.9)),
                Some(Osc::new(Wave::Saw, 4, 9.0, 0.55).width(0.7).semis(-12.0)),
                None,
            ];
            p.cutoff = 1100.0;
            p.res = 0.15;
            p.track = 0.4;
            p.vel_cut = 0.8;
            p.env_amt = 1.0;
            p.fenv = adsr(0.9, 2.5, 0.35, 1.5);
            p.amp = adsr(0.7, 2.0, 0.85, 1.8);
            p.lfo_rate = 0.25;
            p.lfo_cut = 0.25;
            p.drift = 2.0;
            p.stereo = true;
            p.hp = 90.0;
            p.gain = 0.295;
            p.vel_amt = 0.6;
        }
        "pad-glass" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 3, 10.0, 0.5).width(0.8)),
                Some(
                    Osc::new(Wave::Pulse, 2, 6.0, 0.4)
                        .width(0.6)
                        .pw(0.3)
                        .semis(12.0),
                ),
                Some(Osc::new(Wave::Sine, 2, 4.0, 0.35).width(0.9).semis(19.0)),
            ];
            p.filter = FilterKind::Lp;
            p.cutoff = 3200.0;
            p.res = 0.12;
            p.track = 0.6;
            p.env_amt = 1.2;
            p.fenv = adsr(0.05, 1.8, 0.4, 1.5);
            p.amp = adsr(0.35, 2.0, 0.8, 2.0);
            p.lfo_rate = 0.4;
            p.pwm = 0.15;
            p.lfo_cut = 0.15;
            p.stereo = true;
            p.hp = 150.0;
            p.gain = 0.298;
            p.vel_amt = 0.6;
        }
        "pad-dark" => {
            p.filter = FilterKind::Lp4;
            p.osc = [
                Some(Osc::new(Wave::Saw, 4, 11.0, 0.7).width(0.8)),
                Some(Osc::new(Wave::Tri, 2, 5.0, 0.5).width(0.5).semis(-12.0)),
                None,
            ];
            p.sub = 0.25;
            p.cutoff = 480.0;
            p.res = 0.25;
            p.track = 0.3;
            p.env_amt = 0.8;
            p.fenv = adsr(1.6, 3.0, 0.3, 2.0);
            p.amp = adsr(1.2, 3.0, 0.9, 2.5);
            p.lfo_rate = 0.12;
            p.lfo_cut = 0.35;
            p.drift = 2.5;
            p.stereo = true;
            p.gain = 0.403;
            p.vel_amt = 0.5;
        }
        "pad-choir" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 4, 12.0, 0.8).width(0.8)),
                Some(Osc::new(Wave::Saw, 2, 7.0, 0.3).width(0.6)),
                None,
            ];
            p.noise = 0.04;
            p.filter = FilterKind::Formant;
            p.amp = adsr(0.6, 2.0, 0.9, 1.6);
            p.lfo_rate = 0.18;
            p.vib = 6.0;
            p.lfo_delay = 0.4;
            p.ensemble = 7.0;
            p.drift = 2.0;
            p.stereo = true;
            p.hp = 120.0;
            p.gain = 0.433;
            p.vel_amt = 0.5;
        }
        "strings-synth" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 6, 8.0, 0.8).width(0.9)),
                Some(Osc::new(Wave::Saw, 2, 5.0, 0.3).width(0.5).semis(12.0)),
                None,
            ];
            p.filter = FilterKind::Lp;
            p.cutoff = 3600.0;
            p.res = 0.05;
            p.track = 0.5;
            p.env_amt = 0.6;
            p.fenv = adsr(0.3, 1.5, 0.6, 0.9);
            p.amp = adsr(0.28, 1.5, 0.9, 0.9);
            p.ensemble = 9.0;
            p.lfo_rate = 5.2;
            p.vib = 4.0;
            p.lfo_delay = 0.6;
            p.stereo = true;
            p.hp = 160.0;
            p.gain = 0.397;
        }
        "lead-saw" => {
            p.oversample = true;
            p.osc = [
                Some(Osc::new(Wave::Saw, 2, 9.0, 0.8).width(0.4)),
                Some(Osc::new(Wave::Saw, 1, 0.0, 0.35).semis(-12.0)),
                None,
            ];
            p.cutoff = 2600.0;
            p.res = 0.3;
            p.drive = 1.6;
            p.track = 0.7;
            p.env_amt = 1.4;
            p.fenv = adsr(0.002, 0.45, 0.35, 0.3);
            p.amp = adsr(0.004, 1.0, 0.9, 0.18);
            p.lfo_rate = 5.6;
            p.vib = 18.0;
            p.lfo_delay = 0.35;
            p.glide = 0.07;
            p.mono = true;
            p.stereo = true;
            p.gain = 1.25;
        }
        "lead-square" => {
            p.oversample = true;
            p.osc = [
                Some(Osc::new(Wave::Pulse, 2, 6.0, 0.7).width(0.3).pw(0.42)),
                Some(Osc::new(Wave::Pulse, 1, 0.0, 0.3).semis(-12.0).pw(0.5)),
                None,
            ];
            p.cutoff = 2200.0;
            p.res = 0.2;
            p.drive = 1.4;
            p.env_amt = 1.0;
            p.fenv = adsr(0.002, 0.35, 0.4, 0.3);
            p.amp = adsr(0.004, 1.0, 0.9, 0.15);
            p.lfo_rate = 5.2;
            p.vib = 15.0;
            p.lfo_delay = 0.4;
            p.pwm = 0.12;
            p.glide = 0.06;
            p.mono = true;
            p.stereo = true;
            p.gain = 1.114;
        }
        "lead-soft" => {
            p.osc = [
                Some(Osc::new(Wave::Tri, 2, 5.0, 0.8).width(0.3)),
                Some(Osc::new(Wave::Sine, 1, 0.0, 0.4).semis(12.0)),
                None,
            ];
            p.filter = FilterKind::Lp;
            p.cutoff = 2400.0;
            p.res = 0.05;
            p.amp = adsr(0.03, 1.0, 0.9, 0.3);
            p.lfo_rate = 5.0;
            p.vib = 14.0;
            p.lfo_delay = 0.3;
            p.noise = 0.015;
            p.glide = 0.09;
            p.mono = true;
            p.stereo = true;
            p.gain = 0.591;
        }
        "pluck" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 2, 7.0, 0.7).width(0.5)),
                Some(Osc::new(Wave::Pulse, 1, 0.0, 0.35).pw(0.4).semis(12.0)),
                None,
            ];
            p.cutoff = 500.0;
            p.res = 0.25;
            p.track = 0.8;
            p.env_amt = 4.0;
            p.fenv = adsr(0.001, 0.32, 0.0, 0.3);
            p.amp = adsr(0.001, 1.1, 0.0, 0.35);
            p.key_decay = 0.8;
            p.stereo = true;
            p.gain = 0.952;
        }
        "pluck-bright" => {
            p.oversample = true;
            p.osc = [Some(Osc::supersaw(0.35, 0.5, 0.8, 0.8)), None, None];
            p.filter = FilterKind::Lp;
            p.cutoff = 900.0;
            p.res = 0.1;
            p.track = 0.8;
            p.env_amt = 3.8;
            p.fenv = adsr(0.001, 0.2, 0.1, 0.3);
            p.amp = adsr(0.001, 0.8, 0.0, 0.3);
            p.key_decay = 0.8;
            p.stereo = true;
            p.hp = 120.0;
            p.gain = 0.604;
        }
        "bass-sub" => {
            p.osc = [
                Some(Osc::new(Wave::Sine, 1, 0.0, 1.0)),
                Some(Osc::new(Wave::Tri, 1, 0.0, 0.15)),
                None,
            ];
            p.filter = FilterKind::Lp;
            p.cutoff = 300.0;
            p.track = 1.0;
            p.amp = adsr(0.004, 1.0, 1.0, 0.08);
            p.glide = 0.05;
            p.mono = true;
            p.drift = 0.0;
            p.sat = 0.15;
            p.gain = 0.858;
        }
        "bass-reese" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 2, 22.0, 0.8).width(0.5)),
                Some(Osc::new(Wave::Saw, 2, 9.0, 0.5).width(0.2).semis(-12.0)),
                None,
            ];
            p.sub = 0.35;
            p.cutoff = 700.0;
            p.res = 0.2;
            p.drive = 2.0;
            p.track = 0.3;
            p.env_amt = 0.6;
            p.fenv = adsr(0.01, 0.6, 0.6, 0.3);
            p.amp = adsr(0.006, 1.0, 1.0, 0.12);
            p.lfo_rate = 0.3;
            p.lfo_cut = 0.5;
            p.glide = 0.05;
            p.mono = true;
            p.stereo = true;
            p.sat = 0.3;
            p.gain = 1.323;
        }
        "bass-808" => {
            p.osc = [Some(Osc::new(Wave::Sine, 1, 0.0, 1.0)), None, None];
            p.filter = FilterKind::Lp;
            p.cutoff = 1800.0;
            p.track = 0.6;
            p.amp = adsr(0.002, 3.2, 0.0, 0.25);
            p.key_decay = 0.85;
            p.pitch_env = (7.0, 0.025);
            p.glide = 0.08;
            p.mono = true;
            p.drift = 0.0;
            p.sat = 0.45;
            p.gain = 1.138;
        }
        "bass-acid" => {
            p.osc = [Some(Osc::new(Wave::Saw, 1, 0.0, 1.0)), None, None];
            p.cutoff = 380.0;
            p.res = 0.82;
            p.drive = 2.2;
            p.track = 0.5;
            p.vel_cut = 0.5;
            p.env_amt = 3.2;
            p.fenv = adsr(0.001, 0.32, 0.0, 0.2);
            p.amp = adsr(0.002, 2.0, 0.8, 0.06);
            p.accent = true;
            p.glide = 0.06;
            p.mono = true;
            p.drift = 0.5;
            p.sat = 0.2;
            p.gain = 1.05;
        }
        "bass-synth" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 1, 0.0, 0.7)),
                Some(Osc::new(Wave::Pulse, 1, 0.0, 0.5).semis(-12.0).pw(0.5)),
                None,
            ];
            p.cutoff = 520.0;
            p.res = 0.25;
            p.drive = 1.6;
            p.track = 0.5;
            p.env_amt = 1.6;
            p.fenv = adsr(0.002, 0.35, 0.25, 0.2);
            p.amp = adsr(0.003, 1.2, 0.85, 0.09);
            p.glide = 0.04;
            p.mono = true;
            p.gain = 1.317;
        }
        "bass-wobble" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 3, 15.0, 0.8).width(0.4)),
                Some(Osc::new(Wave::Pulse, 1, 0.0, 0.4).semis(-12.0)),
                None,
            ];
            p.sub = 0.4;
            p.cutoff = 600.0;
            p.res = 0.5;
            p.drive = 2.5;
            p.track = 0.3;
            p.amp = adsr(0.004, 1.0, 1.0, 0.1);
            p.lfo_rate = 3.0;
            p.lfo_shape = LfoShape::Triangle;
            p.lfo_cut = 2.2;
            p.glide = 0.05;
            p.mono = true;
            p.stereo = true;
            p.sat = 0.35;
            p.gain = 1.525;
        }
        "brass-synth" => {
            p.osc = [
                Some(Osc::new(Wave::Saw, 3, 7.0, 0.8).width(0.6)),
                Some(Osc::new(Wave::Saw, 1, 0.0, 0.3).semis(-12.0)),
                None,
            ];
            p.cutoff = 700.0;
            p.res = 0.12;
            p.drive = 1.3;
            p.track = 0.7;
            p.vel_cut = 1.4;
            p.env_amt = 2.4;
            p.fenv = adsr(0.07, 0.6, 0.45, 0.25);
            p.amp = adsr(0.03, 1.0, 0.85, 0.25);
            p.lfo_rate = 5.3;
            p.vib = 10.0;
            p.lfo_delay = 0.5;
            p.stereo = true;
            p.gain = 1.085;
        }
        "arp-pluck" => {
            p.oversample = true;
            p.osc = [
                Some(Osc::new(Wave::Pulse, 2, 5.0, 0.7).width(0.6).pw(0.3)),
                Some(Osc::new(Wave::Saw, 1, 0.0, 0.4)),
                None,
            ];
            p.cutoff = 700.0;
            p.res = 0.35;
            p.track = 0.8;
            p.env_amt = 3.2;
            p.fenv = adsr(0.001, 0.14, 0.0, 0.12);
            p.amp = adsr(0.001, 0.45, 0.0, 0.15);
            p.stereo = true;
            p.gain = 0.97;
        }
        "chip-square" => {
            p.oversample = true;
            p.osc = [
                Some(Osc::new(Wave::Pulse, 1, 0.0, 1.0).pw(0.25)),
                None,
                None,
            ];
            p.filter = FilterKind::Lp;
            p.cutoff = 14000.0;
            p.track = 0.0;
            p.vel_cut = 0.0;
            p.amp = adsr(0.001, 1.0, 0.75, 0.05);
            p.lfo_rate = 6.0;
            p.lfo_shape = LfoShape::Triangle;
            p.vib = 20.0;
            p.lfo_delay = 0.25;
            p.drift = 0.0;
            p.vel_amt = 0.4;
            p.gain = 0.338;
        }
        "supersaw" => {
            p.oversample = true;
            p.osc = [
                Some(Osc::supersaw(0.42, 0.65, 1.0, 0.8)),
                Some(Osc::supersaw(0.3, 0.5, 0.8, 0.35).semis(-12.0)),
                None,
            ];
            p.filter = FilterKind::Lp;
            p.cutoff = 6000.0;
            p.res = 0.05;
            p.track = 0.3;
            p.env_amt = 0.6;
            p.fenv = adsr(0.002, 0.8, 0.6, 0.4);
            p.amp = adsr(0.004, 1.0, 0.95, 0.35);
            p.stereo = true;
            p.hp = 180.0;
            p.gain = 0.341;
        }
        _ => return None,
    }
    Some(p)
}

/// The FM patch behind a preset name.
pub fn fm_patch(name: &str) -> Option<FmPatch> {
    Some(match name {
        "epiano-fm" => FmPatch {
            pairs: vec![
                // Body: 1:1 with a velocity-sensitive, decaying index ("bark").
                FmPair::new(1.0, 1.0, 0.55, 1.9, 1.6, 0.18, 1.0)
                    .cents(2.5)
                    .pan(-0.35)
                    .index_key(1.5),
                FmPair::new(1.0, 1.0, 0.45, 1.7, 1.4, 0.15, 1.0)
                    .cents(-2.5)
                    .pan(0.35)
                    .index_key(1.5),
                // Tine: 1:14 with a very short index envelope (the metallic "ding").
                FmPair::new(1.0, 14.0, 0.25, 1.6, 0.09, 0.0, 0.55)
                    .index_key(4.0)
                    .level_key(-1.5),
                // Upper tine partial for sparkle on hard hits.
                FmPair::new(4.0, 1.0, 0.1, 0.8, 0.25, 0.0, 0.08).level_key(-3.0),
            ],
            attack: 0.0015,
            t60_fast: 2.2,
            slow_mix: 0.45,
            t60_slow: 9.0,
            key_decay: 0.72,
            release: 0.35,
            trem: (4.2, 0.18),
            wow: (0.0, 0.5),
            flutter: (0.0, 6.0),
            lp: 0.0,
            lp_vel: 0.0,
            drive: 0.35,
            hiss: 0.0,
            gain: 0.333,
        },
        "keys-lofi" => FmPatch {
            pairs: vec![
                FmPair::new(1.0, 1.0, 0.5, 1.2, 1.4, 0.2, 1.0)
                    .cents(7.0)
                    .pan(-0.4)
                    .index_key(1.5),
                FmPair::new(1.0, 1.0, 0.4, 1.1, 1.2, 0.2, 1.0)
                    .cents(-7.0)
                    .pan(0.4)
                    .index_key(1.5),
                FmPair::new(1.0, 14.0, 0.15, 0.8, 0.07, 0.0, 0.35).index_key(4.0),
                FmPair::new(2.0, 1.0, 0.2, 0.4, 0.8, 0.3, 0.12),
            ],
            attack: 0.003,
            t60_fast: 1.8,
            slow_mix: 0.4,
            t60_slow: 6.0,
            key_decay: 0.75,
            release: 0.3,
            trem: (0.0, 0.0),
            wow: (9.0, 0.55),
            flutter: (2.5, 7.5),
            lp: 2600.0,
            lp_vel: 0.6,
            drive: 0.8,
            hiss: 0.0015,
            gain: 0.55,
        },
        "bell-fm" => FmPatch {
            pairs: vec![
                FmPair::new(1.0, 3.5, 1.6, 2.6, 2.5, 0.12, 1.0).pan(-0.2),
                FmPair::new(1.0, 1.4, 0.8, 1.2, 1.2, 0.0, 0.6)
                    .cents(4.0)
                    .pan(0.25),
                FmPair::new(3.0, 4.07, 0.5, 1.0, 0.6, 0.0, 0.18).level_key(-2.0),
            ],
            attack: 0.001,
            t60_fast: 2.0,
            slow_mix: 0.55,
            t60_slow: 7.0,
            key_decay: 0.7,
            release: 2.5,
            trem: (0.0, 0.0),
            wow: (0.0, 0.5),
            flutter: (0.0, 6.0),
            lp: 0.0,
            lp_vel: 0.0,
            drive: 0.0,
            hiss: 0.0,
            gain: 0.435,
        },
        _ => return None,
    })
}

/// The organ patch behind a preset name.
pub fn organ_patch(name: &str) -> Option<OrganPatch> {
    Some(match name {
        "organ" => OrganPatch {
            drawbars: [8.0, 8.0, 8.0, 5.0, 0.0, 3.0, 0.0, 0.0, 2.0],
            perc: (4, 0.55, 0.45),
            click: 0.35,
            rotary: (0.75, 0.18),
            amp: adsr(0.004, 1.0, 1.0, 0.05),
            vel_amt: 0.0,
            drive: 0.5,
            lp: 9000.0,
            vel_bright: 0.0,
            chorus: 4.0,
            gain: 0.311,
        },
        "stab-house" => OrganPatch {
            drawbars: [0.0, 0.0, 8.0, 7.0, 0.0, 6.0, 0.0, 0.0, 4.0],
            perc: (3, 0.6, 0.18),
            click: 0.25,
            rotary: (0.0, 0.0),
            amp: adsr(0.001, 0.75, 0.12, 0.12),
            vel_amt: 0.7,
            drive: 0.9,
            lp: 4500.0,
            vel_bright: 1.6,
            chorus: 6.0,
            gain: 0.557,
        },
        _ => return None,
    })
}
