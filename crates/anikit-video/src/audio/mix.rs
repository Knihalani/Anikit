//! The timeline mixer: tracks (files or built-in SFX) placed sample-accurately,
//! trimmed, looped, faded with equal-power curves, music ducked under voice
//! through a sidechain envelope, and a transparent look-ahead limiter on the
//! master so nothing hard-clips.
//!
//! JSON shape of a [`Track`] (every field optional except `src`):
//!
//! ```json
//! { "src": "music.mp3", "start": 0, "trim": [12.5, 80], "volume": 0.8, "db": -3,
//!   "fadeIn": 1, "fadeOut": 2, "loop": true, "duration": 30, "role": "music",
//!   "duck": { "under": "voice", "amountDb": 12, "attack": 0.15, "release": 0.5 } }
//! ```
//!
//! `src` is a file path, `"sfx:whoosh"`, or `{ "sfx": "typing", "params": { "duration": 2 } }`.

use super::sfx::{sfx_at, SfxParams};
use super::{db_to_gain, decode, Pcm};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// What a track plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "SourceRepr", into = "SourceRepr")]
pub enum Source {
    /// An audio file (relative paths resolve against the spec's directory).
    File(String),
    /// A built-in procedural sound effect (see [`super::sfx::SFX_NAMES`]).
    Sfx { name: String, params: SfxParams },
}

impl Default for Source {
    fn default() -> Self {
        Source::File(String::new())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SourceRepr {
    Str(String),
    Sfx(SfxSource),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SfxSource {
    sfx: String,
    #[serde(default)]
    params: SfxParams,
}

impl From<SourceRepr> for Source {
    fn from(r: SourceRepr) -> Self {
        match r {
            SourceRepr::Str(s) => match s.strip_prefix("sfx:") {
                Some(name) => Source::Sfx {
                    name: name.to_string(),
                    params: SfxParams::default(),
                },
                None => Source::File(s),
            },
            SourceRepr::Sfx(s) => Source::Sfx {
                name: s.sfx,
                params: s.params,
            },
        }
    }
}

impl From<Source> for SourceRepr {
    fn from(s: Source) -> Self {
        match s {
            Source::File(p) => SourceRepr::Str(p),
            Source::Sfx { name, params } if params == SfxParams::default() => {
                SourceRepr::Str(format!("sfx:{name}"))
            }
            Source::Sfx { name, params } => SourceRepr::Sfx(SfxSource { sfx: name, params }),
        }
    }
}

/// A track's role, used for ducking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Music,
    Voice,
    Sfx,
}

/// Lower this track while tracks of role `under` are sounding. In JSON it is
/// an object, `true` (defaults), or a number: the level to duck *to*
/// (0..1, e.g. `0.4` ≈ −8 dB), as in the spec's shorthand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase", remote = "Self")]
pub struct Duck {
    /// The sidechain role (default `"voice"`).
    pub under: Role,
    /// Gain reduction in dB while ducked (positive, default 12).
    pub amount_db: f64,
    /// Seconds to duck down (≈95 %); ducking starts this early (look-ahead).
    pub attack: f64,
    /// Seconds to recover after the sidechain falls silent.
    pub release: f64,
}

impl Default for Duck {
    fn default() -> Self {
        Duck {
            under: Role::Voice,
            amount_db: 12.0,
            attack: 0.15,
            release: 0.5,
        }
    }
}

impl<'de> Deserialize<'de> for Duck {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::Bool(true) => Ok(Duck::default()),
            serde_json::Value::Number(n) => {
                let level = n.as_f64().unwrap_or(0.4).clamp(1e-4, 1.0);
                Ok(Duck { amount_db: -20.0 * level.log10(), ..Duck::default() })
            }
            _ => Duck::deserialize(v).map_err(serde::de::Error::custom),
        }
    }
}

impl Serialize for Duck {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Duck::serialize(self, s)
    }
}

/// Volume: a linear number or a dB string like `"-6dB"`.
fn volume_de<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_f64().ok_or_else(|| serde::de::Error::custom("volume must be a number")),
        serde_json::Value::String(s) => {
            let t = s.trim().trim_end_matches("dB").trim_end_matches("db").trim();
            t.parse::<f64>().map(|db| 10f64.powf(db / 20.0)).map_err(|_| serde::de::Error::custom(format!("volume {s:?}: use a number (0..1) or \"-6dB\"")))
        }
        v => Err(serde::de::Error::custom(format!("volume must be a number or \"-6dB\", got {v}"))),
    }
}

/// One audio track on the timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Track {
    pub src: Source,
    /// Timeline seconds where the (trimmed) source starts (`at` also works).
    #[serde(alias = "at")]
    pub start: f64,
    /// `[in, out]` in source seconds; `out <= in` (or absent) means "to the end".
    pub trim: Option<[f64; 2]>,
    /// Shorthand for `trim: [trimStart, end]`.
    pub trim_start: Option<f64>,
    /// Linear gain (default 1), or a dB string like `"-6dB"`.
    #[serde(deserialize_with = "volume_de")]
    pub volume: f64,
    /// Extra gain in dB, multiplied with `volume` (default 0).
    pub db: f64,
    /// Equal-power fade-in seconds.
    pub fade_in: f64,
    /// Equal-power fade-out seconds (at the end of the track's play length).
    pub fade_out: f64,
    /// Repeat the trimmed source; without `duration` it loops to the end of the mix.
    #[serde(rename = "loop")]
    pub looped: bool,
    /// Timeline seconds the track plays for (default: the trimmed length, or
    /// to the end of the mix when looping).
    pub duration: Option<f64>,
    /// Duck this track under another role.
    pub duck: Option<Duck>,
    /// Defaults to `"sfx"` for built-in SFX sources and `"music"` otherwise.
    pub role: Option<Role>,
    /// Shorthand for `role: "voice"`.
    pub voice: bool,
}

impl Default for Track {
    fn default() -> Self {
        Track {
            src: Source::default(),
            start: 0.0,
            trim: None,
            trim_start: None,
            volume: 1.0,
            db: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            looped: false,
            duration: None,
            duck: None,
            role: None,
            voice: false,
        }
    }
}

impl Track {
    /// The effective role.
    pub fn role(&self) -> Role {
        if self.voice && self.role.is_none() {
            return Role::Voice;
        }
        self.role.unwrap_or(match self.src {
            Source::Sfx { .. } => Role::Sfx,
            Source::File(_) => Role::Music,
        })
    }
}

/// Decode / synthesise every track's source at `rate`. Files are decoded once
/// even if several tracks use them; relative paths resolve against `base`.
pub fn load_sources(
    tracks: &[Track],
    rate: u32,
    base: Option<&Path>,
) -> Result<Vec<Arc<Pcm>>, String> {
    let mut cache: HashMap<String, Arc<Pcm>> = HashMap::new();
    let mut out = Vec::with_capacity(tracks.len());
    for t in tracks {
        let pcm = match &t.src {
            Source::File(p) => {
                if let Some(c) = cache.get(p) {
                    c.clone()
                } else {
                    let path = match base {
                        Some(b) if Path::new(p).is_relative() => b.join(p),
                        _ => Path::new(p).to_path_buf(),
                    };
                    let pcm = Arc::new(decode::decode_file_at(&path, rate)?);
                    cache.insert(p.clone(), pcm.clone());
                    pcm
                }
            }
            Source::Sfx { name, params } => Arc::new(sfx_at(name, params, rate)?),
        };
        out.push(pcm);
    }
    Ok(out)
}

/// Load every source and mix `duration` seconds at `rate`.
pub fn mix(tracks: &[Track], duration: f64, rate: u32, base: Option<&Path>) -> Result<Pcm, String> {
    let sources = load_sources(tracks, rate, base)?;
    Ok(mix_sources(tracks, &sources, duration, rate))
}

/// Where a track lands: output frames `[s0, s0 + len)` (clipped to the mix),
/// reading the source segment `[seg0, seg0 + seg_len)`, looped or not.
struct Placement {
    s0: i64,
    len: i64,
    seg0: usize,
    seg_len: usize,
    gain: f32,
    fade_in: i64,
    fade_out: i64,
}

fn place(t: &Track, src: &Pcm, rate: u32, n_out: usize) -> Option<Placement> {
    let r = rate as f64;
    let frames = src.frames();
    let [a, b] = t.trim.unwrap_or([t.trim_start.unwrap_or(0.0), 0.0]);
    let seg0 = ((a.max(0.0) * r).round() as usize).min(frames);
    let seg1 = if b > a {
        ((b * r).round() as usize).min(frames)
    } else {
        frames
    };
    let seg_len = seg1.saturating_sub(seg0);
    let s0 = (t.start * r).round() as i64;
    let mut len = match t.duration {
        Some(d) => (d.max(0.0) * r).round() as i64,
        None if t.looped => n_out as i64 - s0,
        None => seg_len as i64,
    };
    if !t.looped {
        len = len.min(seg_len as i64);
    }
    let gain = (t.volume * db_to_gain(t.db)) as f32;
    if len <= 0 || seg_len == 0 || gain == 0.0 || s0 >= n_out as i64 {
        return None;
    }
    Some(Placement {
        s0,
        len,
        seg0,
        seg_len,
        gain,
        fade_in: (t.fade_in.max(0.0) * r).round() as i64,
        fade_out: (t.fade_out.max(0.0) * r).round() as i64,
    })
}

/// Render one placed track, calling `sink(out_frame, l, r)` per frame.
#[inline(always)]
fn render(p: &Placement, src: &Pcm, n_out: usize, mut sink: impl FnMut(usize, f32, f32)) {
    use std::f32::consts::FRAC_PI_2;
    let s = &src.samples;
    // Play offsets j in [j0, j1) map to output frame s0 + j.
    let j0 = (-p.s0).max(0);
    let j1 = p.len.min(n_out as i64 - p.s0);
    let mut j = j0;
    while j < j1 {
        // A contiguous run inside one pass over the source segment.
        let pos = (j as usize) % p.seg_len;
        let run = ((p.seg_len - pos) as i64).min(j1 - j);
        let base = (p.seg0 + pos) * 2;
        let out0 = (p.s0 + j) as usize;
        let src_run = &s[base..base + run as usize * 2];
        let fades = j < p.fade_in || j + run > p.len - p.fade_out;
        if !fades {
            for (k, c) in src_run.chunks_exact(2).enumerate() {
                sink(out0 + k, c[0] * p.gain, c[1] * p.gain);
            }
        } else {
            for (k, c) in src_run.chunks_exact(2).enumerate() {
                let jj = j + k as i64;
                let mut g = p.gain;
                if jj < p.fade_in {
                    g *= (FRAC_PI_2 * (jj as f32 + 0.5) / p.fade_in as f32).sin();
                }
                let left = p.len - jj;
                if left <= p.fade_out {
                    g *= (FRAC_PI_2 * (left as f32 - 0.5).max(0.0) / p.fade_out as f32).sin();
                }
                sink(out0 + k, c[0] * g, c[1] * g);
            }
        }
        j += run;
    }
}

/// Sidechain activity threshold (−40 dBFS) and hold time.
const GATE: f32 = 0.01;
const HOLD: f64 = 0.15;
/// Sidechain activity is tracked per block of `1 << BLOCK_SHIFT` frames
/// (1.3 ms at 48 kHz), which keeps it tiny even for long mixes.
const BLOCK_SHIFT: usize = 6;

/// Mix already-loaded `sources` (one per track, any rate; resampled if needed).
pub fn mix_sources<P: AsRef<Pcm>>(
    tracks: &[Track],
    sources: &[P],
    duration: f64,
    rate: u32,
) -> Pcm {
    assert_eq!(tracks.len(), sources.len(), "one source per track");
    let n_out = (duration.max(0.0) * rate as f64).round() as usize;
    let srcs: Vec<Cow<Pcm>> = sources
        .iter()
        .map(|s| {
            let s = s.as_ref();
            if s.rate == rate {
                Cow::Borrowed(s)
            } else {
                Cow::Owned(s.resample(rate))
            }
        })
        .collect();
    let places: Vec<Option<Placement>> = tracks
        .iter()
        .zip(&srcs)
        .map(|(t, s)| place(t, s, rate, n_out))
        .collect();
    let n_blocks = (n_out >> BLOCK_SHIFT) + 1;

    // Sidechain activity (per block) for each role that something ducks under.
    let mut activity: HashMap<Role, Vec<bool>> = HashMap::new();
    for t in tracks {
        if let Some(d) = &t.duck {
            activity.entry(d.under).or_insert_with(|| {
                let mut act = vec![false; n_blocks];
                for ((t2, p), s) in tracks.iter().zip(&places).zip(&srcs) {
                    if let (true, Some(p)) = (t2.role() == d.under, p) {
                        render(p, s, n_out, |n, l, r| {
                            if l.abs().max(r.abs()) > GATE {
                                act[n >> BLOCK_SHIFT] = true;
                            }
                        });
                    }
                }
                act
            });
        }
    }

    // One ducking target per distinct duck setting.
    let mut targets: Vec<(Duck, Vec<bool>)> = Vec::new();
    let mut target_of: Vec<Option<usize>> = Vec::with_capacity(tracks.len());
    for t in tracks {
        target_of.push(t.duck.as_ref().map(|d| {
            if let Some(i) = targets.iter().position(|(c, _)| c == d) {
                i
            } else {
                targets.push((d.clone(), duck_target(&activity[&d.under], d, rate)));
                targets.len() - 1
            }
        }));
    }

    let mut out = vec![0.0f32; n_out * 2];
    for ((p, s), ti) in places.iter().zip(&srcs).zip(&target_of) {
        let Some(p) = p else { continue };
        match ti {
            None => render(p, s, n_out, |n, l, r| {
                out[2 * n] += l;
                out[2 * n + 1] += r;
            }),
            Some(ti) => {
                // Smooth the gain toward 1 (idle) or the ducked level with
                // one-pole attack / release (≈95 % in `attack` / `release`
                // seconds). It never decays toward 0, so no denormals.
                let (d, tg) = &targets[*ti];
                let k = |secs: f64| {
                    if secs <= 0.0 {
                        1.0
                    } else {
                        (1.0 - (-3.0 / (secs * rate as f64)).exp()) as f32
                    }
                };
                let (ka, kr) = (k(d.attack), k(d.release));
                let low = db_to_gain(-d.amount_db.abs()) as f32;
                let first = p.s0.max(0) as usize;
                let mut g = if tg[first >> BLOCK_SHIFT] { low } else { 1.0 };
                render(p, s, n_out, |n, l, r| {
                    let target = if tg[n >> BLOCK_SHIFT] { low } else { 1.0 };
                    g += (target - g) * if target < g { ka } else { kr };
                    out[2 * n] += l * g;
                    out[2 * n + 1] += r * g;
                })
            }
        }
    }
    limit(&mut out, rate);
    Pcm {
        rate,
        channels: 2,
        samples: out,
    }
}

/// Where a duck setting wants to be ducked: sidechain activity held for
/// [`HOLD`] and anticipated by `attack` (offline look-ahead, so the music is
/// already down when the voice starts).
fn duck_target(act: &[bool], d: &Duck, rate: u32) -> Vec<bool> {
    let block = (1usize << BLOCK_SHIFT) as f64 / rate as f64;
    let hold = (HOLD / block).ceil() as usize;
    let ahead = (d.attack.max(0.0) / block).ceil() as usize;
    let mut out = vec![false; act.len()];
    let mut since = usize::MAX;
    for (o, &a) in out.iter_mut().zip(act) {
        since = if a { 0 } else { since.saturating_add(1) };
        *o = since <= hold;
    }
    let mut until = usize::MAX;
    for (o, &a) in out.iter_mut().zip(act).rev() {
        until = if a { 0 } else { until.saturating_add(1) };
        *o |= until <= ahead;
    }
    out
}

/// Master ceiling (≈ −0.3 dBFS).
const CEILING: f32 = 0.966;

/// Soft look-ahead limiter: below the ceiling the signal is untouched; above
/// it the gain ramps down linearly over ≈2 ms ahead of each peak and recovers
/// with a smooth ≈80 ms release, so the output never exceeds the ceiling and
/// is never hard-clipped. Works in blocks with a small reused scratch buffer.
fn limit(out: &mut [f32], rate: u32) {
    const B: usize = 4096;
    let n = out.len() / 2;
    let look = ((0.002 * rate as f64) as usize).max(1);
    let step = 1.0 / look as f32;
    let kr = 1.0 - (-3.0 / (0.08 * rate as f32)).exp();
    let mut g = vec![1.0f32; B + look];
    let mut prev = 1.0f32;
    let mut a = 0;
    while a < n {
        let b = (a + B).min(n);
        let ext = (b + look).min(n);
        if prev >= 1.0 && out[2 * a..2 * ext].iter().all(|s| s.abs() <= CEILING) {
            a = b; // nothing to do in this block
            continue;
        }
        // Backward: required gain, falling at most `step` per frame toward a
        // peak (the attack ramp happens before the peak). Frames past `b`
        // only feed the look-ahead.
        let mut next = 1.0f32;
        for (gi, c) in g[..ext - a]
            .iter_mut()
            .zip(out[2 * a..2 * ext].chunks_exact(2))
            .rev()
        {
            let p = c[0].abs().max(c[1].abs());
            let req = if p > CEILING { CEILING / p } else { 1.0 };
            next = req.min(next + step);
            *gi = next;
        }
        // Forward: exponential release, then apply.
        for (c, &gi) in out[2 * a..2 * b].chunks_exact_mut(2).zip(&g) {
            prev = gi.min(prev + (1.0 - prev) * kr);
            c[0] *= prev;
            c[1] *= prev;
        }
        if prev > 0.9999 {
            prev = 1.0;
        }
        a = b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: u32 = 48_000;

    fn constant(v: f32, secs: f64) -> Pcm {
        Pcm {
            rate: R,
            channels: 2,
            samples: vec![v; (secs * R as f64) as usize * 2],
        }
    }

    fn file(name: &str) -> Source {
        Source::File(name.into())
    }

    #[test]
    fn places_sample_accurately() {
        let t = Track {
            src: file("a"),
            start: 1.0,
            volume: 0.5,
            ..Default::default()
        };
        let out = mix_sources(&[t], &[constant(1.0, 2.0)], 4.0, R);
        assert_eq!(out.frames(), 4 * R as usize);
        let at = R as usize;
        assert_eq!(out.samples[2 * (at - 1)], 0.0);
        assert_eq!(out.samples[2 * at], 0.5);
        assert_eq!(out.samples[2 * at + 1], 0.5);
        assert_eq!(out.samples[2 * (3 * at - 1)], 0.5);
        assert_eq!(out.samples[2 * 3 * at], 0.0);
    }

    #[test]
    fn trim_and_db() {
        // Source ramps 0,1,2,…; trim [0.5, 1.0] starts at frame 24000.
        let src = Pcm {
            rate: R,
            channels: 2,
            samples: (0..R as usize * 2)
                .flat_map(|i| [i as f32 * 1e-6; 2])
                .collect(),
        };
        let t = Track {
            src: file("a"),
            trim: Some([0.5, 1.0]),
            db: -6.0206,
            ..Default::default()
        };
        let out = mix_sources(&[t], &[src], 1.0, R);
        assert!((out.samples[0] - 24_000.0 * 1e-6 * 0.5).abs() < 1e-5);
        assert_eq!(out.samples[2 * 24_000], 0.0);
    }

    #[test]
    fn fades_are_monotone_and_equal_power() {
        let t = Track {
            src: file("a"),
            fade_in: 0.5,
            fade_out: 0.5,
            ..Default::default()
        };
        let out = mix_sources(&[t], &[constant(0.8, 2.0)], 2.0, R);
        let l: Vec<f32> = out.samples.iter().step_by(2).copied().collect();
        let h = R as usize / 2;
        assert!(l[..h].windows(2).all(|w| w[1] >= w[0]));
        assert!(l[l.len() - h..].windows(2).all(|w| w[1] <= w[0]));
        assert!(l[0] < 0.01 && *l.last().unwrap() < 0.01);
        assert!((l[R as usize] - 0.8).abs() < 1e-6);
        // Equal power: half-way through the fade-in the gain is 1/√2.
        assert!((l[h / 2] / 0.8 - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    }

    #[test]
    fn loops_repeat() {
        let src = Pcm {
            rate: R,
            channels: 2,
            samples: (0..1000)
                .flat_map(|i| [(i as f32 / 1000.0) * 0.5; 2])
                .collect(),
        };
        let t = Track {
            src: file("a"),
            looped: true,
            start: 0.25,
            ..Default::default()
        };
        let out = mix_sources(&[t], std::slice::from_ref(&src), 1.0, R);
        let s0 = R as usize / 4;
        for k in [0usize, 1, 999, 1000, 1001, 5555, 20_000] {
            assert_eq!(
                out.samples[2 * (s0 + k)],
                src.samples[2 * (k % 1000)],
                "frame {k}"
            );
        }
        assert_eq!(
            out.samples[2 * (R as usize - 1)],
            src.samples[2 * ((R as usize - 1 - s0) % 1000)]
        );
        // Bounded looping with `duration`.
        let t = Track {
            src: file("a"),
            looped: true,
            duration: Some(0.1),
            ..Default::default()
        };
        let out = mix_sources(&[t], &[src], 1.0, R);
        assert_eq!(out.samples[2 * 4799], src_val(4799));
        assert_eq!(out.samples[2 * 4800], 0.0);
        fn src_val(k: usize) -> f32 {
            ((k % 1000) as f32 / 1000.0) * 0.5
        }
    }

    #[test]
    fn music_ducks_under_voice() {
        let music = Track {
            src: file("m"),
            role: Some(Role::Music),
            duck: Some(Duck::default()),
            ..Default::default()
        };
        let voice = Track {
            src: file("v"),
            role: Some(Role::Voice),
            start: 2.0,
            ..Default::default()
        };
        let out = mix_sources(
            &[music, voice],
            &[constant(0.5, 6.0), constant(0.1, 1.0)],
            6.0,
            R,
        );
        let at = |s: f64| out.samples[2 * (s * R as f64) as usize];
        assert!((at(1.0) - 0.5).abs() < 1e-4, "{}", at(1.0));
        let ducked = at(2.5) - 0.1; // music part while the voice plays
        let want = 0.5 * 10f32.powf(-12.0 / 20.0);
        assert!((ducked - want).abs() < 0.01, "ducked {ducked} want {want}");
        // Ducking ramps in before the voice starts and recovers afterwards.
        assert!(at(1.95) < 0.45);
        assert!((at(5.5) - 0.5).abs() < 0.01, "{}", at(5.5));
    }

    #[test]
    fn limiter_never_clips() {
        let tracks = vec![
            Track {
                src: file("a"),
                ..Default::default()
            },
            Track {
                src: file("b"),
                ..Default::default()
            },
        ];
        let loud: Vec<f32> = (0..R as usize)
            .flat_map(|i| [(i as f32 * 0.05).sin() * 0.9; 2])
            .collect();
        let a = Pcm {
            rate: R,
            channels: 2,
            samples: loud,
        };
        let out = mix_sources(&tracks, &[a.clone(), a], 1.0, R);
        assert!(out.peak() <= CEILING + 1e-6, "{}", out.peak());
        assert!(out.peak() > 0.9);
    }

    #[test]
    fn track_json() {
        let t: Track = serde_json::from_str(
            r#"{"src":"music.mp3","start":1.5,"trim":[2,10],"db":-3,"fadeIn":1,"fadeOut":2,"loop":true,
                "duck":{"under":"voice","amountDb":9},"role":"music"}"#,
        )
        .unwrap();
        assert_eq!(t.src, Source::File("music.mp3".into()));
        assert!(
            t.looped
                && t.fade_out == 2.0
                && t.duck.as_ref().unwrap().amount_db == 9.0
                && t.volume == 1.0
        );
        let s: Track = serde_json::from_str(r#"{"src":"sfx:whoosh"}"#).unwrap();
        assert!(matches!(s.src, Source::Sfx { ref name, .. } if name == "whoosh"));
        assert_eq!(s.role(), Role::Sfx);
        let s: Track =
            serde_json::from_str(r#"{"src":{"sfx":"typing","params":{"duration":2}}}"#).unwrap();
        assert!(matches!(s.src, Source::Sfx { ref params, .. } if params.duration == Some(2.0)));
        assert!(serde_json::from_str::<Track>(r#"{"src":"a.mp3","fadein":1}"#).is_err());
        // Round trip.
        let back: Track = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn mixes_builtin_sfx_and_is_fast() {
        let tracks: Vec<Track> = serde_json::from_str(
            r#"[{"src":"sfx:whoosh","start":0.5},{"src":"sfx:ding","start":1.0,"volume":0.7},
                {"src":{"sfx":"typing","params":{"duration":1}},"start":2}]"#,
        )
        .unwrap();
        let out = mix(&tracks, 4.0, R, None).unwrap();
        assert!(out.peak() > 0.05 && out.peak() < 1.0);
        assert!(mix(
            &[Track {
                src: Source::File("/nope.wav".into()),
                ..Default::default()
            }],
            1.0,
            R,
            None
        )
        .is_err());

        // 3 minutes, 5 tracks (music looped + ducked, voice, 3 one-shots).
        let music: Vec<f32> = (0..R as usize * 30)
            .flat_map(|i| [((i as f32) * 0.03).sin() * 0.6; 2])
            .collect();
        let music = Pcm {
            rate: R,
            channels: 2,
            samples: music,
        };
        let tracks = vec![
            Track {
                src: file("m"),
                looped: true,
                fade_in: 2.0,
                fade_out: 3.0,
                duck: Some(Duck::default()),
                ..Default::default()
            },
            Track {
                src: file("v"),
                role: Some(Role::Voice),
                start: 10.0,
                ..Default::default()
            },
            Track {
                src: file("m"),
                start: 60.0,
                volume: 0.5,
                ..Default::default()
            },
            Track {
                src: file("m"),
                start: 100.0,
                trim: Some([1.0, 20.0]),
                ..Default::default()
            },
            Track {
                src: file("m"),
                start: 150.0,
                fade_out: 1.0,
                ..Default::default()
            },
        ];
        let srcs = vec![
            music.clone(),
            music.clone(),
            music.clone(),
            music.clone(),
            music,
        ];
        let t0 = std::time::Instant::now();
        let out = mix_sources(&tracks, &srcs, 180.0, R);
        let dt = t0.elapsed();
        eprintln!("3-minute 5-track mix: {dt:?}");
        assert_eq!(out.frames(), 180 * R as usize);
        assert!(out.peak() < 1.0);
        // Generous in debug builds; release is a few tens of ms.
        assert!(
            dt.as_secs_f64() < if cfg!(debug_assertions) { 20.0 } else { 1.0 },
            "{dt:?}"
        );
    }

    #[test]
    fn spec_spellings() {
        let t: Track = serde_json::from_str(r#"{"src":"sfx:whoosh","at":2.5,"volume":"-6dB","trimStart":1,"duck":0.5,"voice":true}"#).unwrap();
        assert_eq!(t.start, 2.5);
        assert!((t.volume - 0.501).abs() < 1e-3);
        assert_eq!(t.trim_start, Some(1.0));
        assert!((t.duck.as_ref().unwrap().amount_db - 6.02).abs() < 0.01);
        assert_eq!(t.role(), Role::Voice);
        let t: Track = serde_json::from_str(r#"{"src":"m.mp3","duck":true}"#).unwrap();
        assert_eq!(t.duck.unwrap().amount_db, 12.0);
    }

}
