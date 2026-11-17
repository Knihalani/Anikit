//! The JSON music spec and its compilation into an [`Arrangement`]: timed
//! notes per track, mix settings, SFX cues and the exact beat grid.
//!
//! ```json
//! {"style":"lofi","bpm":84,"key":"Dm","length":"20s","hits":["@4.0","@bar9"]}
//! {"bpm":120,"key":"C","tracks":[{"inst":"piano","chords":"C Am F G","play":"broken"},
//!   {"inst":"drums:909","drums":{"kick":"x...x...x...x...","hat":"..x...x...x...x."}}]}
//! ```

use crate::arrange::{self, ChordAt, Play};
use crate::notes::{self, Ctx, Note};
use crate::seq::Grid;
use crate::theory::{Key, Length};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A time: seconds (`2.5`, `"2.5s"`, `"@2.5"`) or a musical position (`"@bar5"`, `"@beat9"`, `"@bar5+1.5"` = 1.5 beats after bar 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TimeRef {
    Secs(f64),
    Str(String),
}

impl TimeRef {
    /// Resolve to seconds on the music's own timeline (0 = start of the music).
    pub fn secs(&self, grid: &Grid) -> Result<f64, String> {
        match self {
            TimeRef::Secs(s) => Ok(*s),
            TimeRef::Str(s) => {
                let t = s.trim().trim_start_matches('@');
                let (head, plus) = match t.split_once('+') {
                    Some((a, b)) => (a, b.trim().parse::<f64>().map_err(|_| format!("time '{s}': offset after '+' is in beats, e.g. @bar5+0.5"))?),
                    None => (t, 0.0),
                };
                let musical = |n: &str, unit: f64| -> Result<f64, String> {
                    let k: f64 = n.parse().map_err(|_| format!("time '{s}': use @bar5 or @beat9 (counting from 1)"))?;
                    Ok(grid.secs((k - 1.0).max(0.0) * unit + plus) - grid.offset)
                };
                if let Some(n) = head.strip_prefix("bar") {
                    musical(n, grid.beats_per_bar)
                } else if let Some(n) = head.strip_prefix("beat") {
                    musical(n, 1.0)
                } else {
                    head.trim_end_matches('s').parse::<f64>().map(|v| v + plus * grid.beat()).map_err(|_| {
                        format!("time '{s}': use seconds (2.5 or \"@2.5\") or a musical position (\"@bar5\", \"@beat9\", \"@bar5+0.5\")")
                    })
                }
            }
        }
    }
}

/// `"intro:4"`, `"drop"`, or `{"name":"drop","bars":8,"energy":1}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SectionSpec {
    Short(String),
    Full(SectionObj),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SectionObj {
    pub name: String,
    pub bars: Option<f64>,
    /// 0..1; default from the name (intro 0.3, verse 0.5, build 0.7, drop/chorus 1, break 0.35, outro 0.3).
    pub energy: Option<f32>,
    /// Chords for this section only.
    pub chords: Option<String>,
}

/// A sound effect on the timeline, aligned by its hit point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SfxCue {
    pub sfx: String,
    /// When the effect's hit lands (a riser peaks here, an impact starts here).
    pub at: TimeRef,
    /// Gain in dB (default 0) or "-6dB".
    #[serde(default)]
    pub gain: Option<f64>,
    /// Extra parameters for the effect (duration, pitch, …).
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_json::Value>,
}

/// Per-track mix settings (all optional).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackMix {
    /// dB.
    pub gain: f64,
    /// −1 (left) … 1 (right).
    pub pan: f32,
    /// Stereo width 0 (mono) … 2 (extra wide), default 1.
    pub width: f32,
    /// Reverb send 0..1.
    pub reverb: f32,
    /// Tempo-synced delay send 0..1.
    pub delay: f32,
    /// Sidechain pump from the kick: depth in dB (true = 6 dB).
    #[serde(deserialize_with = "duck_de")]
    pub duck: f32,
    /// EQ (see the dsp EQ params): `{"low":-2,"high":1.5,"hp":40,"mid":[[400,-3,1]]}`.
    pub eq: Option<serde_json::Value>,
    /// Compressor params: `{"threshold":-18,"ratio":3}` or `true`.
    pub comp: Option<serde_json::Value>,
    /// Saturation drive 0..1.
    pub drive: f32,
    /// Low-pass cutoff in Hz (muffled / lofi), 0 = off.
    pub lp: f32,
    /// High-pass cutoff in Hz, 0 = off.
    pub hp: f32,
}

impl Default for TrackMix {
    fn default() -> Self {
        TrackMix { gain: 0.0, pan: 0.0, width: 1.0, reverb: 0.0, delay: 0.0, duck: 0.0, eq: None, comp: None, drive: 0.0, lp: 0.0, hp: 0.0 }
    }
}

fn duck_de<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Bool(true) => 6.0,
        serde_json::Value::Bool(false) | serde_json::Value::Null => 0.0,
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0).abs() as f32,
        v => return Err(serde::de::Error::custom(format!("duck: true or a depth in dB, got {v}"))),
    })
}

/// A clip: notes, drums or chords placed at a bar.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClipSpec {
    /// Bar where the clip starts (1-based, default 1) or a time ("@bar5", 2.5).
    pub at: Option<TimeRef>,
    /// Note language (see `notes`).
    pub notes: Option<String>,
    /// Drum grids: voice → pattern (`"x...x..."`).
    pub drums: Option<BTreeMap<String, String>>,
    /// Steps per bar for drum grids (default 16).
    pub steps: Option<usize>,
    /// Chord progression (`"Am F C G"`, one bar each).
    pub chords: Option<String>,
    /// How chords play: pad, block, stabs, arp[-updown][/16], strum, pulse/8, broken, power, or a rhythm `x..x..x.`.
    pub play: Option<String>,
    /// Bassline from `chords` (or the song's chords): root, half, pulse, octave, walk, house, synco, rock, arp, 808.
    pub bass: Option<String>,
    /// Generate a melody over the chords: density in notes per beat (e.g. 1).
    pub melody: Option<f64>,
    /// Bars the clip lasts (default: its content, looped to fill when `repeat`/`bars` given).
    pub bars: Option<f64>,
    /// Repeat the clip n times.
    pub repeat: Option<usize>,
    /// Octave for notes/degrees/chord voicing (default 4; bass 2).
    pub octave: Option<i32>,
    /// Velocity 0..1 (default 0.75).
    pub vel: Option<f32>,
    /// Transpose in semitones.
    pub transpose: Option<i32>,
}

/// A track: an instrument, its clips and mix.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackSpec {
    pub name: Option<String>,
    /// Instrument name (`piano`, `strings`, `nylon-guitar`, `pad-warm`, `bass-808`, `drums:808`, `gm:24`, `file.sf2#0`, `file.sfz`).
    pub inst: String,
    /// Clips (or put one clip's fields directly on the track).
    pub clips: Vec<ClipSpec>,
    #[serde(flatten)]
    pub clip: ClipSpec,
    #[serde(flatten)]
    pub mix: TrackMix,
    /// Timing/velocity humanisation 0..1 (default 0.3; drums 0.15).
    pub humanize: Option<f32>,
    /// Ignore swing.
    pub straight: bool,
    /// Effect chain: `[{"type":"delay","note":"1/8d","wet":0.2}, {"type":"reverb","preset":"plate","wet":0.15}]`.
    pub fx: Option<serde_json::Value>,
    /// Automation: `{"lp": [[0, 400], ["@bar5", 8000]], "pan": "=0.4*sin(beat*pi/4)", "synth.cutoff": [[0, 300], [16, 3000]]}`.
    pub automate: Option<serde_json::Value>,
}

/// Master bus settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct MasterSpec {
    /// Integrated loudness target, LUFS (default −14).
    pub lufs: f64,
    /// True-peak ceiling, dBTP (default −1).
    pub ceiling: f64,
    /// Reverb preset for the reverb bus: room, hall, plate, chamber, cathedral, ambience.
    pub reverb: Option<String>,
    /// Reverb bus return level, dB.
    pub reverb_gain: f64,
    /// Fade out the last n seconds.
    pub fade_out: Option<f64>,
    /// Fade in the first n seconds.
    pub fade_in: Option<f64>,
}

impl Default for MasterSpec {
    fn default() -> Self {
        MasterSpec { lufs: -14.0, ceiling: -1.0, reverb: None, reverb_gain: 0.0, fade_out: None, fade_in: None }
    }
}

/// The music spec. Every field is optional; `{"style":"lofi"}` is a complete track.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct MusicSpec {
    /// Genre: see [`crate::style::STYLES`].
    pub style: Option<String>,
    pub bpm: Option<f64>,
    /// `"Am"`, `"C"`, `"F# dorian"`.
    pub key: Option<String>,
    /// `"4/4"`, `"3/4"`, `"6/8"`.
    pub meter: Option<String>,
    /// 0 (straight) … 1 (hard swing); 0.33 ≈ triplet feel.
    pub swing: Option<f64>,
    /// Free words: dark, happy, epic, chill, tense, sad, dreamy, uplifting, aggressive, mysterious…
    pub mood: Option<String>,
    /// `"15s"`, `30`, `"16 bars"`, `"1:30"`. Default: 8 bars of the style, or the content.
    pub length: Option<TimeRef>,
    pub sections: Vec<SectionSpec>,
    /// Moments that must hit hard (impact + crash + section change), e.g. `["@2.0", "@bar9"]`.
    pub hits: Vec<TimeRef>,
    /// Override the progression (`"Am F C G"` or numerals `"vi IV I V"`).
    pub chords: Option<String>,
    /// Override instruments by role: `{"lead":"violin","chords":"piano"}`.
    pub instruments: BTreeMap<String, String>,
    /// Drop roles from a style: `["lead","drums"]`.
    pub mute: Vec<String>,
    /// Explicit tracks (added to the style's, or replacing a style role of the same name).
    pub tracks: Vec<TrackSpec>,
    pub sfx: Vec<SfxCue>,
    pub master: MasterSpec,
    /// Variation seed (same seed → identical audio).
    pub seed: u64,
    /// Nudge the tempo (±6 %) so `hits` land on beats (default true).
    pub fit: Option<bool>,
    /// How the track ends: `ring` (final chord rings out, default), `fade`, `cut`.
    pub end: Option<String>,
    /// Import a MIDI file as tracks (instruments from its programs).
    pub midi: Option<String>,
    /// Video only: where the music starts on the video timeline (seconds).
    pub at: Option<f64>,
    /// Video only: music bed level in dB relative to the mastered track (default 0).
    pub gain: Option<f64>,
    /// Video only: duck under voice-over (`true` = 10 dB, a number = dB, `false` = never).
    pub duck: Option<serde_json::Value>,
    /// Custom instruments by name: `{"wob": {"synth": {…}}, "kit": {"sampler": {"zones": […]}}}`.
    pub define: BTreeMap<String, serde_json::Value>,
}

impl MusicSpec {
    /// Parse JSON, with a helpful error.
    pub fn from_json(s: &str) -> Result<MusicSpec, String> {
        let v: serde_json::Value = serde_json::from_str(s).map_err(|e| format!("music: {e}"))?;
        check_track_keys(&v)?;
        let de = &mut serde_json::Deserializer::from_str(s);
        let v: MusicSpec = serde_path_to_error_free(de)?;
        Ok(v)
    }
}

fn serde_path_to_error_free(de: &mut serde_json::Deserializer<serde_json::de::StrRead>) -> Result<MusicSpec, String> {
    MusicSpec::deserialize(de).map_err(|e| {
        let msg = e.to_string();
        // Suggest the closest field for unknown ones.
        if let Some(rest) = msg.strip_prefix("unknown field `") {
            let name = rest.split('`').next().unwrap_or("");
            let fields = [
                "style", "bpm", "key", "meter", "swing", "mood", "length", "sections", "hits", "chords", "instruments", "mute", "tracks", "sfx", "master", "seed", "fit", "end", "midi", "at", "duck", "define", "fx", "automate",
                "inst", "clips", "notes", "drums", "steps", "play", "bass", "melody", "bars", "repeat", "octave", "vel", "transpose", "gain", "pan", "width", "reverb", "delay",
                "duck", "eq", "comp", "drive", "lp", "hp", "humanize", "straight", "name", "at", "lufs", "ceiling", "fadeOut", "fadeIn", "reverbGain",
            ];
            if let Some(best) = closest(name, &fields) {
                return format!("music: {msg} — did you mean `{best}`?");
            }
        }
        format!("music: {msg}")
    })
}

const CLIP_KEYS: &[&str] = &["at", "notes", "drums", "steps", "chords", "play", "bass", "melody", "bars", "repeat", "octave", "vel", "transpose"];
const TRACK_KEYS: &[&str] = &["name", "inst", "clips", "humanize", "straight", "fx", "automate", "gain", "pan", "width", "reverb", "delay", "duck", "eq", "comp", "drive", "lp", "hp"];

/// Unknown keys on tracks and clips (serde's `flatten` can't reject them).
pub fn check_track_keys(v: &serde_json::Value) -> Result<(), String> {
    let all: Vec<&str> = TRACK_KEYS.iter().chain(CLIP_KEYS).copied().collect();
    for (ti, t) in v.get("tracks").and_then(|t| t.as_array()).into_iter().flatten().enumerate() {
        let check = |obj: &serde_json::Value, allowed: &[&str], at: String| -> Result<(), String> {
            for k in obj.as_object().into_iter().flat_map(|o| o.keys()) {
                if !allowed.contains(&k.as_str()) {
                    let hint = closest(k, allowed).map(|b| format!(" — did you mean `{b}`?")).unwrap_or_default();
                    return Err(format!("music: {at}: unknown field `{k}`{hint}"));
                }
            }
            Ok(())
        };
        check(t, &all, format!("tracks[{ti}]"))?;
        for (ci, c) in t.get("clips").and_then(|c| c.as_array()).into_iter().flatten().enumerate() {
            check(c, CLIP_KEYS, format!("tracks[{ti}].clips[{ci}]"))?;
        }
    }
    Ok(())
}

/// The closest word by edit distance (≤ 3).
pub fn closest<'a>(word: &str, words: &[&'a str]) -> Option<&'a str> {
    let d = |a: &str, b: &str| {
        let (a, b): (Vec<char>, Vec<char>) = (a.to_lowercase().chars().collect(), b.to_lowercase().chars().collect());
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        for i in 1..=a.len() {
            let mut cur = vec![i; b.len() + 1];
            for j in 1..=b.len() {
                cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
            }
            prev = cur;
        }
        prev[b.len()]
    };
    words.iter().map(|w| (d(word, w), *w)).filter(|(n, _)| *n <= 3).min_by_key(|(n, _)| *n).map(|(_, w)| w)
}

// ───────────────────────────── compiled form ─────────────────────────────

/// A section of the arrangement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub name: String,
    /// First bar (0-based).
    pub bar: f64,
    pub bars: f64,
    pub energy: f32,
}

/// A track ready to render.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackPlan {
    pub name: String,
    /// Role (drums, bass, chords, pad, lead, arp, …): used for stems and defaults.
    pub role: String,
    /// Instrument name (may contain `|` fallbacks).
    pub inst: String,
    pub notes: Vec<Note>,
    pub mix: TrackMix,
    pub humanize: f32,
    pub straight: bool,
    /// Effect chain (see [`crate::custom::fx_chain`]).
    pub fx: Option<serde_json::Value>,
    /// Automation (see [`crate::custom::TrackAuto`]).
    pub automate: Option<serde_json::Value>,
}

/// A placed sound effect.
#[derive(Debug, Clone, PartialEq)]
pub struct SfxPlan {
    pub name: String,
    /// Seconds (music timeline) where the hit lands.
    pub at: f64,
    pub gain: f64,
    pub params: serde_json::Value,
}

/// Everything needed to render.
#[derive(Debug, Clone)]
pub struct Arrangement {
    pub grid: Grid,
    pub meter: (u32, u32),
    pub key: Key,
    pub style: String,
    pub sections: Vec<Section>,
    pub tracks: Vec<TrackPlan>,
    pub sfx: Vec<SfxPlan>,
    pub master: MasterSpec,
    /// Kick times in seconds (sidechain source).
    pub kicks: Vec<f64>,
    /// Output length in seconds.
    pub length: f64,
    pub seed: u64,
    pub end: String,
    /// Reverb preset for the bus.
    pub reverb: String,
    /// Custom instrument definitions (the spec's `define`).
    pub defines: BTreeMap<String, serde_json::Value>,
}

impl Arrangement {
    /// Beat times (seconds) from 0 up to the end.
    pub fn beats(&self) -> Vec<f64> {
        let n = (self.length / self.grid.beat()).floor() as usize + 1;
        (0..n).map(|i| self.grid.secs(i as f64)).filter(|&t| t <= self.length + 1e-9).collect()
    }
    /// Downbeat (bar) times in seconds.
    pub fn bars(&self) -> Vec<f64> {
        let n = (self.length / self.grid.bar()).floor() as usize + 1;
        (0..n).map(|i| self.grid.secs(i as f64 * self.grid.beats_per_bar)).filter(|&t| t <= self.length + 1e-9).collect()
    }
}

/// Parse `"6/8"` → (6, 8) and beats (quarter notes) per bar.
pub fn meter(s: Option<&str>) -> Result<((u32, u32), f64), String> {
    let s = s.unwrap_or("4/4");
    let (a, b) = s.split_once('/').ok_or_else(|| format!("meter '{s}': use \"4/4\", \"3/4\", \"6/8\""))?;
    let (n, d): (u32, u32) = (a.trim().parse().map_err(|_| format!("meter '{s}'"))?, b.trim().parse().map_err(|_| format!("meter '{s}'"))?);
    if n == 0 || !matches!(d, 2 | 4 | 8 | 16) {
        return Err(format!("meter '{s}': use \"4/4\", \"3/4\", \"6/8\""));
    }
    Ok(((n, d), n as f64 * 4.0 / d as f64))
}

/// Compile explicit `tracks` into plans (style-independent part).
pub fn compile_tracks(spec: &MusicSpec, grid: &Grid, key: &Key, song_chords: Option<&[ChordAt]>, total_beats: Option<f64>) -> Result<Vec<TrackPlan>, String> {
    let mut out = Vec::new();
    for (ti, t) in spec.tracks.iter().enumerate() {
        let name = t.name.clone().unwrap_or_else(|| t.inst.split([':', '|', '#', '/']).next().unwrap_or("track").to_string());
        let ctxname = format!("tracks[{ti}] ({name})");
        if t.inst.trim().is_empty() {
            return Err(format!("{ctxname}: missing \"inst\" (e.g. \"piano\", \"drums:808\", \"strings\")"));
        }
        let is_drums = t.inst.starts_with("drums") || t.inst.contains("kit");
        let mut clips: Vec<&ClipSpec> = t.clips.iter().collect();
        if clips.is_empty() || t.clip != ClipSpec::default() {
            clips.insert(0, &t.clip);
        }
        let mut notes = Vec::new();
        for (ci, c) in clips.iter().enumerate() {
            let at = match &c.at {
                None => 0.0,
                Some(TimeRef::Secs(bar)) => (bar - 1.0).max(0.0) * grid.beats_per_bar,
                Some(r) => (r.secs(grid)? / grid.beat()).max(0.0),
            };
            let mut n = clip_notes(c, grid, key, song_chords, total_beats.map(|b| b - at), is_drums, ti as u64 * 7919 + ci as u64 + spec.seed)
                .map_err(|e| format!("{ctxname}: {e}"))?;
            n.iter_mut().for_each(|x| x.t += at);
            notes.extend(n);
        }
        let role = if is_drums { "drums" } else if clips.iter().any(|c| c.bass.is_some()) { "bass" } else { "music" };
        // `gain` is relative to a sensible default level for the role (tracks are
        // loudness-matched in the mixer, so 0 always sits right).
        let mut mix = t.mix.clone();
        mix.gain += match role {
            "drums" => 0.0,
            "bass" => -2.0,
            _ => -5.0,
        };
        out.push(TrackPlan {
            name,
            role: role.into(),
            inst: t.inst.clone(),
            notes,
            mix,
            humanize: t.humanize.unwrap_or(if is_drums { 0.15 } else { 0.3 }),
            straight: t.straight,
            fx: t.fx.clone(),
            automate: t.automate.clone(),
        });
    }
    Ok(out)
}

/// Notes of one clip (beats from the clip start).
fn clip_notes(c: &ClipSpec, grid: &Grid, key: &Key, song_chords: Option<&[ChordAt]>, room: Option<f64>, is_drums: bool, seed: u64) -> Result<Vec<Note>, String> {
    let bpb = grid.beats_per_bar;
    let vel = c.vel.unwrap_or(0.75);
    let mut notes = Vec::new();
    let mut length: f64 = 0.0;
    let want_bars = c.bars;
    if let Some(src) = &c.notes {
        let ctx = Ctx { key: key.clone(), octave: c.octave.unwrap_or(if is_drums { 2 } else { 4 }), beats_per_bar: bpb, len: 1.0, vel };
        let (n, end) = notes::parse(src, &ctx)?;
        notes.extend(n);
        length = length.max(end);
    }
    if let Some(d) = &c.drums {
        let steps = c.steps.unwrap_or(16);
        let bars = want_bars.unwrap_or_else(|| {
            // One bar per `steps` cells of the longest pattern.
            d.values().map(|p| p.chars().filter(|c| !c.is_whitespace() && *c != '|').count()).max().unwrap_or(steps) as f64 / steps as f64
        });
        for (voice, pat) in d {
            let k = notes::drum_key(voice).ok_or_else(|| {
                let hint = closest(voice, notes::DRUM_NAMES).map(|b| format!(" — did you mean '{b}'?")).unwrap_or_default();
                format!("drums: unknown voice '{voice}'{hint} (kick snare clap hat ohh rim tom crash ride shaker … or a GM key number)")
            })?;
            notes.extend(notes::parse_grid(pat, k, steps, bpb, bars.max(1.0 / steps as f64), vel)?);
        }
        length = length.max(bars * bpb);
    }
    let chords: Option<Vec<ChordAt>> = match &c.chords {
        Some(s) => Some(arrange::chord_line(s, key, bpb)?),
        None => None,
    };
    if c.play.is_some() || c.bass.is_some() || c.melody.is_some() || chords.is_some() {
        let mut ch = match (&chords, song_chords) {
            (Some(ch), _) => ch.clone(),
            (None, Some(s)) => s.to_vec(),
            (None, None) => return Err("\"play\"/\"bass\"/\"melody\" need \"chords\" (on the clip or the song)".into()),
        };
        let span = want_bars.map(|b| b * bpb).or(if chords.is_none() { room } else { None });
        if let Some(span) = span {
            ch = arrange::loop_chords(&ch, span);
        }
        let end = ch.last().map(|c| c.t + c.len).unwrap_or(0.0);
        length = length.max(end);
        if let Some(b) = &c.bass {
            let oct = c.octave.unwrap_or(2);
            notes.extend(arrange::bassline(&ch, b, key, oct, vel, &[], seed)?);
        } else if let Some(d) = c.melody {
            let oct = c.octave.unwrap_or(5);
            notes.extend(arrange::melody(&ch, key, (oct + 1) * 12, d, vel, None, seed, bpb));
        } else {
            let play = Play::parse(c.play.as_deref().unwrap_or("pad"))?;
            let oct = c.octave.unwrap_or(4);
            notes.extend(arrange::play_chords(&ch, &play, (oct + 1) * 12 + 2, 4, vel, seed));
        }
    }
    if notes.is_empty() && c.notes.is_none() && c.drums.is_none() {
        return Err("a clip needs \"notes\", \"drums\", \"chords\"+\"play\", \"bass\" or \"melody\"".into());
    }
    if let Some(tr) = c.transpose {
        notes.iter_mut().for_each(|n| n.key += tr as f32);
    }
    // Explicit bar count / repeats loop the content.
    let period = match want_bars {
        Some(b) => b * bpb,
        None => (length / bpb).ceil().max(1.0) * bpb,
    };
    let reps = c.repeat.unwrap_or(1).max(1);
    if reps > 1 || want_bars.is_some_and(|b| b * bpb > length + 1e-9) && c.drums.is_none() && c.notes.is_some() {
        let base = notes.clone();
        let content = (length / bpb).ceil().max(1.0) * bpb;
        let total = if reps > 1 { period * reps as f64 } else { period };
        notes.clear();
        let mut off = 0.0;
        while off < total - 1e-9 {
            notes.extend(base.iter().filter(|n| n.t + off < total - 1e-9).map(|n| Note { t: n.t + off, ..*n }));
            off += if reps > 1 { period } else { content };
        }
    }
    Ok(notes)
}

/// Resolve the song length in seconds from the spec (if any).
pub fn length_secs(spec: &MusicSpec, grid: &Grid) -> Result<Option<f64>, String> {
    Ok(match &spec.length {
        None => None,
        Some(TimeRef::Secs(s)) => Some(*s),
        Some(TimeRef::Str(s)) => Some(match Length::parse(s.trim_start_matches('@'))? {
            Length::Seconds(v) => v,
            Length::Bars(b) => b * grid.bar(),
            Length::Beats(b) => b * grid.beat(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_refs() {
        let g = Grid { bpm: 120.0, ..Default::default() };
        assert_eq!(TimeRef::Str("@bar3".into()).secs(&g).unwrap(), 4.0);
        assert_eq!(TimeRef::Str("@beat2".into()).secs(&g).unwrap(), 0.5);
        assert_eq!(TimeRef::Str("@2.25".into()).secs(&g).unwrap(), 2.25);
        assert_eq!(TimeRef::Str("@bar2+1".into()).secs(&g).unwrap(), 2.5);
        assert_eq!(TimeRef::Secs(7.5).secs(&g).unwrap(), 7.5);
        assert!(TimeRef::Str("@soon".into()).secs(&g).is_err());
    }

    #[test]
    fn explicit_tracks_compile() {
        let spec = MusicSpec::from_json(
            r#"{"bpm":120,"key":"C","tracks":[
              {"inst":"piano","chords":"C Am F G","play":"broken","gain":-3},
              {"inst":"bass","chords":"C Am F G","bass":"octave"},
              {"inst":"drums:909","drums":{"kick":"x...x...x...x...","hat":"..x...x...x...x."},"repeat":4},
              {"inst":"lead-saw","clips":[{"at":3,"notes":"o5 E5/8 D5/8 C5/4 r/2"}]}]}"#,
        )
        .unwrap();
        let g = Grid { bpm: 120.0, ..Default::default() };
        let k = Key::parse("C").unwrap();
        let t = compile_tracks(&spec, &g, &k, None, None).unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].mix.gain, -8.0);
        assert_eq!(t[1].role, "bass");
        assert_eq!(t[2].notes.iter().filter(|n| n.key == 36.0).count(), 16);
        assert_eq!(t[3].notes[0].t, 8.0); // bar 3
        let e = MusicSpec::from_json(r#"{"styel":"lofi"}"#).unwrap_err();
        assert!(e.contains("did you mean `style`"), "{e}");
        let e = MusicSpec::from_json(r#"{"tracks":[{"inst":"piano","note":"C4"}]}"#).unwrap_err();
        assert!(e.contains("did you mean `notes`"), "{e}");
        let bad = MusicSpec::from_json(r#"{"tracks":[{"inst":"drums:808","drums":{"kik":"x..."}}]}"#).unwrap();
        let e = compile_tracks(&bad, &g, &k, None, None).unwrap_err();
        assert!(e.contains("did you mean 'kick'"), "{e}");
    }
}
