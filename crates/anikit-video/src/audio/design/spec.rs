//! The spec's `sound` key and per-layer `sfx`.
//!
//! ```json
//! "sound": "auto"
//! "sound": { "palette": "organic", "density": 0.5, "ambience": "city", "space": "room",
//!            "loudness": -14, "cues": [ { "sfx": "glass", "at": 2.4, "x": 0.7 } ] }
//! layer:   "sfx": "auto" | false | "glass" | { "in": "swish", "land": "wood", "out": "air", "gain": -3 }
//! ```

use super::{canonical, is_known, unknown, DesignParams, AMBIENCES};
use serde::Deserialize;

/// Sound families picked for auto cues.
pub const PALETTES: &[&str] = &[
    "auto",
    "clean",
    "cinematic",
    "organic",
    "digital",
    "luxury",
    "playful",
];
/// Shared spaces.
pub const SPACES: &[&str] = &[
    "auto",
    "dry",
    "room",
    "studio",
    "hall",
    "plate",
    "chamber",
    "cathedral",
    "outdoor",
];

/// `sound` as written.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SoundSpec {
    /// `auto` (derive cues from the picture) or `off` (only explicit cues, beds and the bus).
    pub design: String,
    /// Sound family for auto cues (see [`PALETTES`]).
    pub palette: String,
    /// 0..1: how much gets a sound (0.5 ≈ the hero moves and landings; 1 = most motion).
    pub density: f32,
    /// Ambience bed: a name or `{ "type": "city", "level": -6 }` (level in dB relative to its default).
    pub ambience: Option<serde_json::Value>,
    /// Shared reverb the effects sit in (see [`SPACES`]).
    pub space: String,
    /// Integrated loudness target (LUFS) or `"off"`.
    pub loudness: serde_json::Value,
    /// True-peak ceiling (dBTP).
    pub true_peak: f32,
    /// Dip the music a few dB under hits.
    pub duck: bool,
    /// Pan effects to where they happen on screen.
    pub pan: bool,
    /// SFX level trim (dB).
    pub level: f32,
    /// Sound the scene transitions (slides, pushes, glitches).
    pub transitions: bool,
    /// Explicit cues.
    pub cues: Vec<CueSpec>,
    pub seed: u64,
}

impl Default for SoundSpec {
    fn default() -> Self {
        SoundSpec {
            design: "auto".into(),
            palette: "auto".into(),
            density: 0.5,
            ambience: None,
            space: "auto".into(),
            loudness: serde_json::json!(-14),
            true_peak: -1.0,
            duck: true,
            pan: true,
            level: 0.0,
            transitions: true,
            cues: Vec::new(),
            seed: 1,
        }
    }
}

/// One explicit cue in `sound.cues`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CueSpec {
    pub sfx: String,
    /// Where the hit lands: seconds or a marker (`"@beat9"`, `"@scene2"`).
    pub at: serde_json::Value,
    /// Horizontal screen position 0..1 (pans the effect).
    pub x: Option<f32>,
    /// Level trim in dB.
    pub gain: Option<f32>,
    /// Music dip under this hit in dB.
    pub duck: Option<f32>,
    /// Reverb send 0..1.
    pub send: Option<f32>,
    /// Seconds of near-silence in the music just before the hit (0.08–0.3).
    pub suck: Option<f32>,
    pub duration: Option<f32>,
    pub pitch: Option<f32>,
    pub intensity: Option<f32>,
    pub size: Option<f32>,
    pub motion: Option<f32>,
    pub speed: Option<f32>,
    pub tone: Option<f32>,
    pub count: Option<u32>,
    pub seed: Option<u64>,
}

impl CueSpec {
    pub fn apply(&self, p: &mut DesignParams) {
        if let Some(v) = self.duration {
            p.duration = Some(v);
        }
        macro_rules! set {
            ($($f:ident),*) => { $( if let Some(v) = self.$f { p.$f = v; } )* };
        }
        set!(pitch, intensity, size, motion, speed, tone, seed);
        if self.count.is_some() {
            p.count = self.count;
        }
    }
}

/// A layer's `sfx`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerSfx {
    /// `false`: never sound this layer.
    pub off: bool,
    /// Sound it even when `sound.design` is off.
    pub auto: bool,
    /// Fixed effects for entering motion, landing (hard stop) and leaving.
    pub enter: Option<String>,
    pub land: Option<String>,
    pub exit: Option<String>,
    /// Level trim (dB).
    pub gain: f32,
}

impl LayerSfx {
    pub fn parse(v: &serde_json::Value) -> Result<LayerSfx, String> {
        let check = |n: &str| -> Result<String, String> {
            if is_known(n) {
                Ok(canonical(n))
            } else {
                Err(unknown(n))
            }
        };
        match v {
            serde_json::Value::Bool(false) => Ok(LayerSfx {
                off: true,
                ..Default::default()
            }),
            serde_json::Value::Bool(true) => Ok(LayerSfx {
                auto: true,
                ..Default::default()
            }),
            serde_json::Value::String(s) if s == "auto" => Ok(LayerSfx {
                auto: true,
                ..Default::default()
            }),
            serde_json::Value::String(s) if s == "off" || s == "none" => Ok(LayerSfx {
                off: true,
                ..Default::default()
            }),
            // One name: plays on whatever this layer's main event is.
            serde_json::Value::String(s) => {
                let n = check(s)?;
                Ok(LayerSfx {
                    auto: true,
                    enter: Some(n.clone()),
                    land: Some(n),
                    ..Default::default()
                })
            }
            serde_json::Value::Object(o) => {
                let mut l = LayerSfx {
                    auto: true,
                    ..Default::default()
                };
                for (k, v) in o {
                    let name = || {
                        v.as_str()
                            .map(check)
                            .unwrap_or_else(|| Err(format!("'{k}' must be a sound name")))
                    };
                    match k.as_str() {
                        "in" | "enter" => l.enter = Some(name()?),
                        "land" | "stop" | "hit" => l.land = Some(name()?),
                        "out" | "exit" => l.exit = Some(name()?),
                        "gain" => l.gain = v.as_f64().ok_or("'gain' is dB (a number)")? as f32,
                        _ => return Err(format!("unknown key '{k}' (in, land, out, gain)")),
                    }
                }
                Ok(l)
            }
            v => Err(format!(
                "expected \"auto\", false, a sound name or {{in, land, out, gain}}, got {v}"
            )),
        }
    }
}

/// Everything the audio job needs, compiled.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SoundPlan {
    pub spec: SoundSpec,
    /// `sound.design == "auto"`.
    pub auto: bool,
    /// Palette after resolving `auto` (from the music style).
    pub palette: String,
    /// Shared space preset and size (`None` = dry).
    pub space: Option<(String, f32)>,
    /// Ambience bed: name, level dB.
    pub ambience: Option<(String, f32)>,
    pub target_lufs: Option<f32>,
    /// Per layer tag (index into the evaluator's layer table).
    pub layers: Vec<(u32, LayerSfx)>,
    /// Explicit cues with resolved times (seconds).
    pub cues: Vec<(CueSpec, f64)>,
    /// Tuning root (Hz) from the music key.
    pub tune: Option<f32>,
    /// Ready-made cues (from `audio[].sfx` naming sound-design effects).
    pub extra: Vec<super::Cue>,
}

/// Palette for a music style (when `palette` is `auto`).
pub fn palette_for_style(style: Option<&str>) -> &'static str {
    match style.unwrap_or("").to_ascii_lowercase().as_str() {
        "orchestral" | "acoustic" | "piano" | "jazz" | "folk" | "classical" => "organic",
        "cinematic-hybrid" | "epic" | "trailer" => "cinematic",
        "synthwave" | "trap" | "tech" | "dnb" | "house" | "edm" | "80s" => "digital",
        "lofi" | "chill" | "ambient" => "luxury",
        "pop" | "upbeat" => "playful",
        _ => "clean",
    }
}

impl SoundPlan {
    /// Build from the raw `sound` value (`None` when only layers have `sfx`).
    /// `music` is the spec's `music` value, used for the palette and tuning.
    pub fn new(
        v: Option<&serde_json::Value>,
        music: Option<&serde_json::Value>,
    ) -> Result<SoundPlan, String> {
        let spec: SoundSpec = match v {
            None => SoundSpec {
                design: "off".into(),
                ..Default::default()
            },
            Some(serde_json::Value::String(s)) if s == "auto" => SoundSpec::default(),
            Some(serde_json::Value::String(s)) if s == "off" => SoundSpec {
                design: "off".into(),
                ..Default::default()
            },
            Some(serde_json::Value::String(s)) => {
                // "sound": "organic" — a palette.
                if PALETTES.contains(&s.as_str()) {
                    SoundSpec {
                        palette: s.clone(),
                        ..Default::default()
                    }
                } else {
                    return Err(format!(
                        "sound: \"auto\", \"off\", a palette ({}) or an object",
                        PALETTES.join(", ")
                    ));
                }
            }
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("sound: {e}"))?,
        };
        if !["auto", "off"].contains(&spec.design.as_str()) {
            return Err(format!(
                "sound.design: \"auto\" or \"off\" (got \"{}\")",
                spec.design
            ));
        }
        if !PALETTES.contains(&spec.palette.as_str()) {
            return Err(format!(
                "sound.palette: unknown '{}' (one of {})",
                spec.palette,
                PALETTES.join(", ")
            ));
        }
        if !SPACES.contains(&spec.space.as_str()) {
            return Err(format!(
                "sound.space: unknown '{}' (one of {})",
                spec.space,
                SPACES.join(", ")
            ));
        }
        for (i, c) in spec.cues.iter().enumerate() {
            if !is_known(&c.sfx) {
                return Err(format!("sound.cues[{i}].sfx: {}", unknown(&c.sfx)));
            }
        }
        let style = music.and_then(|m| {
            m.as_str()
                .or_else(|| m.get("style").and_then(|s| s.as_str()))
        });
        let palette = if spec.palette == "auto" {
            palette_for_style(style).to_string()
        } else {
            spec.palette.clone()
        };
        let space = match spec.space.as_str() {
            "dry" => None,
            "auto" => Some(match palette.as_str() {
                "cinematic" => ("hall".to_string(), 0.9),
                "luxury" => ("plate".to_string(), 0.7),
                "organic" => ("chamber".to_string(), 0.6),
                _ => ("room".to_string(), 0.5),
            }),
            "studio" => Some(("room".into(), 0.3)),
            "outdoor" => Some(("ambience".into(), 0.5)),
            "cathedral" => Some(("cathedral".into(), 1.2)),
            "hall" => Some(("hall".into(), 0.9)),
            s => Some((s.to_string(), 0.6)),
        };
        let ambience = match &spec.ambience {
            None | Some(serde_json::Value::Null) | Some(serde_json::Value::Bool(false)) => None,
            Some(serde_json::Value::String(s)) if s == "none" || s == "off" => None,
            Some(serde_json::Value::String(s)) => Some((s.clone(), 0.0)),
            Some(serde_json::Value::Object(o)) => {
                let t = o
                    .get("type")
                    .and_then(|t| t.as_str())
                    .ok_or("sound.ambience: {\"type\": \"city\", \"level\": -6}")?;
                let level = o.get("level").and_then(|l| l.as_f64()).unwrap_or(0.0) as f32;
                Some((t.to_string(), level))
            }
            Some(v) => {
                return Err(format!(
                    "sound.ambience: a name ({}) or {{type, level}}, got {v}",
                    AMBIENCES.join(", ")
                ))
            }
        };
        if let Some((a, _)) = &ambience {
            if !AMBIENCES.contains(&a.as_str()) {
                return Err(format!(
                    "sound.ambience: unknown '{a}'{} (one of {})",
                    anikit_music::dsp::reverb::closest(a, AMBIENCES)
                        .map(|s| format!(", did you mean '{s}'?"))
                        .unwrap_or_default(),
                    AMBIENCES.join(", ")
                ));
            }
        }
        let target_lufs = match &spec.loudness {
            serde_json::Value::Number(n) => {
                Some((n.as_f64().unwrap_or(-14.0) as f32).clamp(-40.0, -6.0))
            }
            serde_json::Value::String(s) if s == "off" => None,
            v => {
                return Err(format!(
                    "sound.loudness: LUFS (e.g. -14) or \"off\", got {v}"
                ))
            }
        };
        let tune = music
            .and_then(|m| m.get("key"))
            .and_then(|k| k.as_str())
            .and_then(super::key_root_hz);
        Ok(SoundPlan {
            auto: spec.design == "auto",
            palette,
            space,
            ambience,
            target_lufs,
            tune,
            spec,
            layers: Vec::new(),
            cues: Vec::new(),
            extra: Vec::new(),
        })
    }
}
