//! Sound design: every procedural effect under one name registry, picture-
//! driven cues (`"sfx": "auto"`), ambience beds, and a mix bus that pans
//! effects to their on-screen position, puts them in one shared space,
//! ducks the music under hits and masters to −14 LUFS / −1 dBTP.
//!
//! - [`render`]: any effect by name → stereo buffer + hit point. Names come from
//!   this module (foley, motion air, sparkle, ambience), the music engine's
//!   cinematic library (`impact`, `riser`, `braam`, …) and the UI library.
//! - [`auto`]: derive cues from what moves on screen.
//! - [`bus`]: the final mix.
//! - [`spec`]: the spec's `sound` key.

pub mod ambience;
pub mod auto;
pub mod bus;
pub mod cli;
mod foley;
pub mod pack;
pub mod spec;
pub mod synth;

use anikit_music::sfx::Sfx;
use serde::Deserialize;

pub use ambience::AMBIENCES;
pub use bus::{mix_design, BusOpts, Cue, MixReport};
pub use spec::SoundSpec;

/// Effects synthesised by this module (see the module docs of [`foley`]).
pub const DESIGN_SFX: &[&str] = &[
    "glass",
    "clink",
    "metal",
    "ping",
    "wood",
    "knock",
    "plastic",
    "ceramic",
    "stone",
    "thump",
    "land",
    "paper",
    "page",
    "cloth",
    "rustle",
    "air",
    "swish",
    "whoosh-deep",
    "whoosh-air",
    "passby",
    "sparkle",
    "magic",
    "shepard",
    "step",
    "crunch",
    "footsteps",
    "breath",
    "shatter",
    "droplet",
    "blip",
    "ticker",
    "downer",
];

/// Every name [`render`] accepts: design effects, ambience beds, then the
/// music engine's and the UI library's effects.
pub fn all_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = DESIGN_SFX.to_vec();
    v.extend(AMBIENCES);
    for n in anikit_music::sfx::SFX
        .iter()
        .chain(crate::audio::sfx::SFX_NAMES)
    {
        if !v.contains(n) {
            v.push(n);
        }
    }
    v
}

/// Whether `name` is a known sound: a built-in name, a pack sound
/// (`kenney-interface/click`) or layers joined with `+` (`thump+cc0-foley/wooden`).
pub fn is_known(name: &str) -> bool {
    let names = all_names();
    name.split('+').all(|n| {
        let c = canonical(n);
        names.contains(&c.as_str()) || pack::is_pack_sound(&c)
    })
}

/// Error text for an unknown sound name, with a suggestion.
pub fn unknown(name: &str) -> String {
    let names = all_names();
    let bad = name
        .split('+')
        .map(canonical)
        .find(|c| !names.contains(&c.as_str()) && !pack::is_pack_sound(c))
        .unwrap_or_else(|| canonical(name));
    let hint = anikit_music::dsp::reverb::closest(&bad, &names)
        .map(|s| format!(" (did you mean '{s}'?)"))
        .unwrap_or_default();
    format!("unknown sound '{bad}'{hint}; `anikit-video docs sound` lists them")
}

/// Parameters for any effect. Every field is optional.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct DesignParams {
    /// Length in seconds (beds, whooshes, textures); one-shots ignore it.
    pub duration: Option<f32>,
    /// Transposition in semitones.
    pub pitch: f32,
    /// 0..1: force, ring length, layers.
    pub intensity: f32,
    /// 0..1: how much of the effect's own room tail.
    pub size: f32,
    /// −1..1 stereo position.
    pub pan: f32,
    /// −1..1 stereo travel for moving sounds (−1 = right→left).
    pub motion: f32,
    /// 0..1 on-screen speed (whooshes get shorter, brighter, heavier).
    pub speed: f32,
    /// 0..1 brightness.
    pub tone: f32,
    /// Tuning root in Hz: tonal effects snap to its pitch classes (set from the music key).
    pub tune: Option<f32>,
    /// Event count (grains, ticks, keystrokes).
    pub count: Option<u32>,
    pub seed: u64,
}

impl Default for DesignParams {
    fn default() -> Self {
        DesignParams {
            duration: None,
            pitch: 0.0,
            intensity: 0.6,
            size: 0.4,
            pan: 0.0,
            motion: 0.0,
            speed: 0.5,
            tone: 0.5,
            tune: None,
            count: None,
            seed: 1,
        }
    }
}

/// Level trim (dB) per design effect after peak-normalising to −1 dBFS: UI
/// and small foley sit well under cinematic hits.
fn trim_db(name: &str) -> f32 {
    match name {
        "blip" | "ticker" | "droplet" | "plastic" => -12.0,
        "air" | "swish" | "cloth" | "rustle" | "paper" | "page" | "step" | "crunch" | "footsteps" | "breath" => -9.0,
        "glass" | "clink" | "wood" | "knock" | "ceramic" | "stone" | "thump" | "land" | "metal"
        | "ping" | "sparkle" | "magic" => -7.0,
        "passby" | "whoosh-deep" | "whoosh-air" | "shepard" => -5.0,
        _ => -3.0,
    }
}

/// Render effect `name` at `rate`. Ambience names render a bed of
/// `duration` seconds (default 10).
pub fn render(name: &str, p: &DesignParams, rate: u32) -> Result<Sfx, String> {
    if name.contains('+') {
        // Layers, aligned on their hit points; each layer gets its own seed.
        let parts: Vec<Sfx> = name
            .split('+')
            .enumerate()
            .map(|(i, n)| {
                render(
                    n,
                    &DesignParams {
                        seed: p.seed.wrapping_add(i as u64 * 101),
                        ..p.clone()
                    },
                    rate,
                )
            })
            .collect::<Result<_, _>>()?;
        let hit = parts.iter().map(|s| s.hit).fold(0.0f32, f32::max);
        let mut st = synth::St::default();
        for s in &parts {
            let off = synth::frames(hit - s.hit, rate as f32);
            st.add(
                off,
                &synth::St {
                    l: s.l.clone(),
                    r: s.r.clone(),
                },
                0.8,
            );
        }
        return Ok(Sfx {
            l: st.l,
            r: st.r,
            hit,
        });
    }
    let key = canonical(name);
    if pack::is_pack_sound(&key) {
        let (pcm, onset) = pack::load(&key, p.seed, rate)?;
        let (l, r): (Vec<f32>, Vec<f32>) =
            pcm.samples.chunks_exact(2).map(|c| (c[0], c[1])).unzip();
        let mut st = synth::St { l, r };
        if p.pitch != 0.0 {
            // Varispeed (tape-style) pitch.
            let f = (p.pitch / 12.0).exp2();
            let n = (st.len() as f32 / f) as usize;
            let mut o = synth::St::new(n);
            for k in 0..n {
                let x = k as f32 * f;
                let i = x as usize;
                let fr = x - i as f32;
                let get = |v: &Vec<f32>, j: usize| v.get(j).copied().unwrap_or(0.0);
                o.l[k] = get(&st.l, i) * (1.0 - fr) + get(&st.l, i + 1) * fr;
                o.r[k] = get(&st.r, i) * (1.0 - fr) + get(&st.r, i + 1) * fr;
            }
            st = o;
            st.normalize_peak(-7.0);
            return Ok(Sfx {
                l: st.l,
                r: st.r,
                hit: onset / f,
            });
        }
        st.normalize_peak(-7.0);
        return Ok(Sfx {
            l: st.l,
            r: st.r,
            hit: onset,
        });
    }
    let sr = rate as f32;
    let seed = p.seed ^ fnv(&key);
    let mut c = foley::Ctx {
        sr,
        p,
        n: synth::Noise::new(seed),
    };
    use foley::Air;
    let out = match key.as_str() {
        "glass" | "clink" => Some(foley::glass(&mut c)),
        "metal" => Some(foley::metal(&mut c)),
        "ping" => {
            let mut p2 = p.clone();
            p2.pitch += 12.0;
            let mut c2 = foley::Ctx {
                sr,
                p: &p2,
                n: synth::Noise::new(seed),
            };
            Some(foley::metal(&mut c2))
        }
        "wood" | "knock" => Some(foley::wood(&mut c)),
        "plastic" => Some(foley::plastic(&mut c)),
        "ceramic" => Some(foley::ceramic(&mut c)),
        "stone" => Some(foley::stone(&mut c)),
        "thump" | "land" => Some(foley::thump(&mut c)),
        "paper" => Some(foley::paper(&mut c, false)),
        "page" => Some(foley::paper(&mut c, true)),
        "cloth" | "rustle" => Some(foley::cloth(&mut c)),
        "air" => Some(foley::air(&mut c, Air::Soft)),
        "swish" => Some(foley::air(&mut c, Air::Swish)),
        "whoosh-deep" => Some(foley::air(&mut c, Air::Deep)),
        "whoosh-air" => Some(foley::air(&mut c, Air::Whoosh)),
        "passby" => Some(foley::air(&mut c, Air::Passby)),
        "sparkle" => Some(foley::sparkle(&mut c, false)),
        "magic" => Some(foley::sparkle(&mut c, true)),
        "shepard" => Some(foley::shepard(&mut c)),
        "step" => Some(foley::step(&mut c, false)),
        "crunch" => Some(foley::step(&mut c, true)),
        "footsteps" => Some(foley::footsteps(&mut c)),
        "breath" => Some(foley::breath(&mut c)),
        "shatter" => Some(foley::shatter(&mut c)),
        "droplet" => Some(foley::droplet(&mut c)),
        "blip" => Some(foley::blip(&mut c)),
        "ticker" => Some(foley::ticker(&mut c)),
        _ => None,
    };
    if let Some((mut st, hit)) = out {
        st.trim_tail(synth::frames(hit + 0.02, sr));
        st.fade_end(synth::frames(0.004, sr));
        st.normalize_peak(-1.0 + trim_db(&key));
        return Ok(Sfx {
            l: st.l,
            r: st.r,
            hit,
        });
    }
    if AMBIENCES.contains(&key.as_str()) {
        let st = ambience::bed(&key, p, sr).expect("bed");
        return Ok(Sfx {
            l: st.l,
            r: st.r,
            hit: 0.0,
        });
    }
    let music_name = match key.as_str() {
        "downer" => "downlifter",
        k => k,
    };
    if anikit_music::sfx::SFX.contains(&music_name) {
        let mp = anikit_music::sfx::SfxParams {
            duration: p.duration,
            pitch: p.pitch,
            intensity: p.intensity,
            size: p.size,
            width: 0.8,
            pan: p.pan,
            motion: (p.motion != 0.0).then_some(p.motion),
            gain: 0.0,
            count: p.count,
            bpm: None,
            seed: p.seed,
        };
        return anikit_music::sfx::sfx(music_name, &mp, rate);
    }
    if crate::audio::sfx::SFX_NAMES.contains(&key.as_str()) {
        let vp = crate::audio::SfxParams {
            duration: p.duration.map(|d| d as f64),
            pitch: (p.pitch as f64 / 12.0).exp2(),
            seed: p.seed,
        };
        let pcm = crate::audio::sfx_at(&key, &vp, rate)?;
        let (l, r): (Vec<f32>, Vec<f32>) =
            pcm.samples.chunks_exact(2).map(|c| (c[0], c[1])).unzip();
        let hit = if matches!(key.as_str(), "rise" | "success") {
            l.len() as f32 / sr * 0.9
        } else {
            0.0
        };
        return Ok(Sfx { l, r, hit });
    }
    let names = all_names();
    let hint = anikit_music::dsp::reverb::closest(&key, &names)
        .map(|s| format!(" (did you mean \"{s}\"?)"))
        .unwrap_or_default();
    Err(format!(
        "unknown sound \"{name}\"{hint}; `anikit-video docs sound` lists them"
    ))
}

/// Lower case, `_`/space → `-`.
pub fn canonical(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace(['_', ' '], "-")
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    })
}

/// Frequency of a key name's root (`"Dm"`, `"F# dorian"`, `"Bb"`), in the
/// octave above A4 = 440 (so tonal effects land in a bright register).
pub fn key_root_hz(key: &str) -> Option<f32> {
    let k = key.trim();
    let mut ch = k.chars();
    let letter = ch.next()?.to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let acc = match ch.next() {
        Some('#') | Some('♯') => 1,
        Some('b') | Some('♭') => -1,
        _ => 0,
    };
    let pc = (base + acc + 12) % 12;
    // Semitones from A.
    let semis = ((pc - 9 + 12) % 12) as f32;
    Some(440.0 * (semis / 12.0).exp2())
}

#[cfg(test)]
mod tests;
