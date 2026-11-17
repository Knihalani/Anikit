//! Time-axis review of a rendered spec, judged against its intent.
//!
//! The rendered video is sampled once at low resolution (6 frames a second,
//! 192 px wide), and every detector reads that one sample set:
//! - motion energy over time;
//! - shots (scene starts plus hard visual changes);
//! - pace against the intent's limits;
//! - dead air (nothing changing);
//! - rhythm monotony;
//! - the energy arc (does it build and land?);
//! - abrupt endings;
//! - cut alignment with the music's beats.
//!
//! Intent (`"intent": {"pace": "calm|steady|fast|frantic", "genre", "audience"}`
//! at the top of a spec) sets the limits. Without one, it is inferred from the
//! music's tempo, then the look, and defaults to steady. A calm film may hold
//! long shots; it is only flagged when it is lifeless or defective.

use serde_json::Value;

use crate::critique::Note;
use crate::eval::Evaluator;
use crate::pipeline;

/// How fast the video means to be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pace {
    Calm,
    Steady,
    Fast,
    Frantic,
}

/// Limits for one pace: longest shot, median shot, share of static seconds, longest dead air (s).
pub struct Limits {
    pub longest: f32,
    pub median: f32,
    pub same: f32,
    pub dead_air: f32,
}

impl Pace {
    pub fn name(self) -> &'static str {
        match self {
            Pace::Calm => "calm",
            Pace::Steady => "steady",
            Pace::Fast => "fast",
            Pace::Frantic => "frantic",
        }
    }
    pub fn parse(s: &str) -> Option<Pace> {
        Some(match s {
            "calm" | "slow" => Pace::Calm,
            "steady" | "medium" => Pace::Steady,
            "fast" => Pace::Fast,
            "frantic" => Pace::Frantic,
            _ => return None,
        })
    }
    /// The fast row is BAR.md's pace test.
    pub fn limits(self) -> Limits {
        match self {
            Pace::Calm => Limits { longest: f32::INFINITY, median: f32::INFINITY, same: 0.85, dead_air: 3.0 },
            Pace::Steady => Limits { longest: 6.0, median: 3.5, same: 0.45, dead_air: 2.0 },
            Pace::Fast => Limits { longest: 2.5, median: 1.5, same: 0.2, dead_air: 1.0 },
            Pace::Frantic => Limits { longest: 1.2, median: 0.7, same: 0.1, dead_air: 0.5 },
        }
    }
}

/// The intent a video is judged against, and where it came from.
pub struct Intent {
    pub pace: Pace,
    pub genre: Option<String>,
    pub audience: Option<String>,
    pub source: String,
}

/// Music beats (and tempo) from the spec's first music cue, if any.
pub fn music_beats(raw: &Value, base: &std::path::Path) -> Option<crate::audio::Beats> {
    let cue = raw.get("audio")?.as_array()?.iter().find(|c| c.get("src").is_some() && c.get("voice") != Some(&Value::Bool(true)))?;
    let src = cue.get("src")?.as_str()?;
    let pcm = crate::audio::decode_file(base.join(src)).ok()?;
    let mut b = crate::eval::beat_markers::refine(&crate::audio::analyze_pcm(&pcm));
    let at = cue.get("at").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let trim = cue.get("trimStart").and_then(|v| v.as_f64()).unwrap_or(0.0);
    for t in b.beats.iter_mut() {
        *t = *t - trim + at;
    }
    Some(b)
}

/// The declared intent, or one inferred from the music's tempo, then the look.
pub fn intent(raw: &Value, bpm: Option<f64>) -> Intent {
    let i = raw.get("intent");
    let s = |k: &str| i.and_then(|i| i.get(k)).and_then(|v| v.as_str()).map(String::from);
    let genre = s("genre");
    let audience = s("audience");
    if let Some(p) = s("pace").as_deref().and_then(Pace::parse) {
        return Intent { pace: p, genre, audience, source: "declared".into() };
    }
    if let Some(b) = bpm.filter(|b| *b > 40.0) {
        let p = if b >= 150.0 { Pace::Frantic } else if b >= 115.0 { Pace::Fast } else if b >= 88.0 { Pace::Steady } else { Pace::Calm };
        return Intent { pace: p, genre, audience, source: format!("music at {b:.0} BPM") };
    }
    let look = raw.get("look").and_then(|l| match l {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("base").and_then(|b| b.as_str()).map(String::from),
        Value::Array(a) => a.last().and_then(|b| b.as_str()).map(String::from),
        _ => None,
    });
    if let Some(l) = look {
        let p = match l.as_str() {
            "noir" | "documentary" | "editorial" => Pace::Calm,
            "neon-night" | "brutalist" | "y2k-chrome" | "risograph" => Pace::Fast,
            _ => Pace::Steady,
        };
        return Intent { pace: p, genre, audience, source: format!("look {l}") };
    }
    Intent { pace: Pace::Steady, genre, audience, source: "default".into() }
}

/// Low-resolution frames, sampled evenly over the video.
pub struct Samples {
    pub step: f32,
    pub w: u32,
    pub h: u32,
    /// RGB8 per sample.
    pub frames: Vec<Vec<u8>>,
}

impl Samples {
    pub fn take(ev: &Evaluator, fps: f32, width: u32) -> Samples {
        let step = 1.0 / fps;
        let scale = (width as f32 / ev.width.max(1) as f32).min(1.0);
        let n = (ev.duration / step).floor() as usize + 1;
        let mut w = 0;
        let mut h = 0;
        let frames = (0..n)
            .map(|i| {
                let mut dl = ev.eval((i as f32 * step).min(ev.duration - 0.01).max(0.0));
                pipeline::scale_list(&mut dl, scale);
                let (fw, fh, px) = pipeline::render_list_rgba(ev, &dl);
                w = fw;
                h = fh;
                px.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
            })
            .collect();
        Samples { step, w, h, frames }
    }
    pub fn time(&self, i: usize) -> f32 {
        i as f32 * self.step
    }
}

/// Mean absolute difference of two RGB8 frames, 0..255.
pub fn diff(a: &[u8], b: &[u8]) -> f32 {
    let s: u64 = a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64).sum();
    s as f32 / a.len().max(1) as f32
}

/// What the timeline measured.
pub struct Timeline {
    pub shots: Vec<f32>,
    pub cuts: Vec<f32>,
    pub median: f32,
    pub longest: f32,
    /// Share of seconds that look like the second before.
    pub same: f32,
    /// Motion energy per second (mean frame-to-frame change).
    pub energy: Vec<f32>,
    /// Share of cuts within 50 ms of a beat, when there is music.
    pub on_beat: Option<f32>,
}

/// `▁▂▃▅▇` per second.
pub fn sparkline(v: &[f32]) -> String {
    let max = v.iter().cloned().fold(0.0f32, f32::max).max(1e-3);
    v.iter().map(|x| ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][((x / max) * 7.0).round().clamp(0.0, 7.0) as usize]).collect()
}

/// Measure the timeline and judge it against `intent`.
pub fn time_axis(ev: &Evaluator, s: &Samples, intent: &Intent, beats: Option<&[f64]>) -> (Timeline, Vec<Note>) {
    let n = s.frames.len();
    let dur = ev.duration;
    let energy_raw: Vec<f32> = (1..n).map(|i| diff(&s.frames[i - 1], &s.frames[i])).collect();
    // Shots: scene starts plus hard visual changes.
    let mut cuts: Vec<f32> = ev.scenes().iter().map(|sc| sc.start).filter(|&t| t > 0.05 && t < dur - 0.05).collect();
    for (i, e) in energy_raw.iter().enumerate() {
        if *e > 32.0 {
            cuts.push(s.time(i + 1));
        }
    }
    cuts.sort_by(|a, b| a.total_cmp(b));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 0.2);
    let mut bounds = vec![0.0];
    bounds.extend(cuts.iter().cloned());
    bounds.push(dur);
    let shots: Vec<f32> = bounds.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 0.05).collect();
    let mut sorted = shots.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(dur);
    let longest = sorted.last().copied().unwrap_or(dur);
    let per = (1.0 / s.step).round().max(1.0) as usize;
    let secs: Vec<f32> = (per..n).step_by(per).map(|i| diff(&s.frames[i - per], &s.frames[i])).collect();
    let same = if secs.is_empty() { 0.0 } else { secs.iter().filter(|d| **d < 6.0).count() as f32 / secs.len() as f32 };
    let energy: Vec<f32> = energy_raw.chunks(per).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
    let on_beat = beats.filter(|b| !b.is_empty() && !cuts.is_empty()).map(|b| {
        let hits = cuts.iter().filter(|c| b.iter().any(|t| (*t as f32 - **c).abs() <= 0.05 + s.step * 0.5)).count();
        hits as f32 / cuts.len() as f32
    });
    let tl = Timeline { shots, cuts, median, longest, same, energy, on_beat };

    let mut notes = Vec::new();
    let lim = intent.pace.limits();
    let mut note = |sev: &'static str, check: &'static str, path: String, message: String, fix: &str| {
        notes.push(Note { severity: sev, check, path, message, fix: fix.into() })
    };
    if dur >= 3.0 {
        // Pace against intent.
        let mut slow = Vec::new();
        if tl.longest > lim.longest {
            slow.push(format!("longest shot {:.1} s > {:.1} s", tl.longest, lim.longest));
        }
        if tl.median > lim.median {
            slow.push(format!("median shot {:.1} s > {:.1} s", tl.median, lim.median));
        }
        if tl.same > lim.same {
            slow.push(format!("{:.0}% of seconds look like the second before (> {:.0}%)", tl.same * 100.0, lim.same * 100.0));
        }
        if !slow.is_empty() {
            let fix = match intent.pace {
                Pace::Fast | Pace::Frantic => "cut or make a major change on the beat every 0.5–1.5 s (\"@beat17\"): whips, snaps, speed ramps, impacts, match cuts; 1–4 words per shot",
                _ => "shorten the longest holds, or give them a change (a camera move, a reveal, light), or declare a calmer \"intent\"",
            };
            note("warning", "pace", format!("timeline ({} shots, median {:.1} s)", tl.shots.len(), tl.median), format!("slower than the {} intent ({}): {}", intent.pace.name(), intent.source, slow.join("; ")), fix);
        }
        // Too fast for a calm or steady film.
        if matches!(intent.pace, Pace::Calm | Pace::Steady) && tl.median < 0.6 && tl.shots.len() >= 6 {
            note("tip", "pace", "timeline".into(), format!("median shot {:.1} s is frantic for a {} intent", tl.median, intent.pace.name()), "let shots breathe, or declare a faster \"intent\"");
        }
        // Dead air: a stretch where nothing changes at all.
        let quiet = |e: f32| e < 0.25;
        let mut run = 0usize;
        let mut worst = (0usize, 0usize);
        for (i, e) in energy_raw.iter().enumerate() {
            if quiet(*e) {
                run += 1;
                if run > worst.1 {
                    worst = (i + 1 - run, run);
                }
            } else {
                run = 0;
            }
        }
        let dead = worst.1 as f32 * s.step;
        if dead > lim.dead_air {
            let at = s.time(worst.0);
            note(
                "warning",
                "dead-air",
                format!("t={at:.1}–{:.1}s", at + dead),
                format!("{dead:.1} s where nothing moves at all{}", if intent.pace == Pace::Calm { ": calm is fine, lifeless is not" } else { "" }),
                "keep the frame alive: a slow push, parallax, drifting light or particles, or cut sooner",
            );
        }
        // Rhythm monotony: every shot the same length.
        if tl.shots.len() >= 5 {
            let inner = &tl.shots[..tl.shots.len() - 1];
            let mean = inner.iter().sum::<f32>() / inner.len() as f32;
            let sd = (inner.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / inner.len() as f32).sqrt();
            if sd / mean.max(1e-3) < 0.1 {
                note("tip", "rhythm", "timeline".into(), format!("{} shots all about {mean:.1} s long: the rhythm is mechanical", inner.len()), "vary shot lengths with the music: hold on a downbeat, then a run of quick cuts");
            }
        }
        // Energy arc: does it build and land?
        if tl.energy.len() >= 8 {
            let e = &tl.energy;
            let mean = e.iter().sum::<f32>() / e.len() as f32;
            let peak = e.iter().cloned().fold(0.0f32, f32::max);
            let peak_at = e.iter().position(|x| *x == peak).unwrap_or(0) as f32 / e.len() as f32;
            if mean > 0.05 && peak < mean * 1.6 && intent.pace != Pace::Calm {
                note("tip", "arc", "timeline".into(), format!("flat energy {}: no build to a peak", sparkline(e)), "plan a build: start sparser, peak at about two-thirds, then land on a held end frame");
            } else if peak_at < 0.15 && peak > mean * 2.0 {
                note("tip", "arc", "timeline".into(), format!("energy peaks in the first seconds and fades {}", sparkline(e)), "save the biggest moment for about two-thirds of the way through");
            }
        }
        // Abrupt ending: still moving hard on the last frame.
        if n >= 4 {
            let last = diff(&s.frames[n - 2], &s.frames[n - 1]);
            let typical = {
                let mut v = energy_raw.clone();
                v.sort_by(|a, b| a.total_cmp(b));
                v[v.len() / 2]
            };
            if last > 3.0 && last > typical * 1.5 {
                note("warning", "ending", format!("t={dur:.1}s"), "the video ends mid-motion: it will feel cut off".into(), "end on a held frame (0.5–1.5 s) or a deliberate hard cut to black on a beat");
            }
        }
        // Cuts against the music.
        if let Some(r) = tl.on_beat {
            if matches!(intent.pace, Pace::Fast | Pace::Frantic) && r < 0.6 {
                note("warning", "beat", "timeline".into(), format!("only {:.0}% of {} cuts land on a beat", r * 100.0, tl.cuts.len()), "time scene starts and changes with \"@beat17\" / \"@bar5\" markers");
            }
        }
    }
    (tl, notes)
}

/// One line on intent versus what was measured.
pub fn summary(intent: &Intent, tl: &Timeline) -> String {
    let mut s = format!(
        "intent: {} ({}){} · measured: {} shots, median {:.1} s, longest {:.1} s, {:.0}% static seconds",
        intent.pace.name(),
        intent.source,
        intent.genre.as_deref().map(|g| format!(", {g}")).unwrap_or_default(),
        tl.shots.len(),
        tl.median,
        tl.longest,
        tl.same * 100.0
    );
    if let Some(r) = tl.on_beat {
        s += &format!(", {:.0}% of cuts on the beat", r * 100.0);
    }
    s += &format!("\nenergy per second: {}", sparkline(&tl.energy));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Options;
    use serde_json::json;

    fn ev(spec: Value) -> Evaluator {
        let p = crate::spec::parse::from_value(spec).unwrap();
        let opts = Options { fonts: Some(std::sync::Arc::new(crate::text::FontStore::new())), ..Options::default() };
        Evaluator::new(&p.spec, opts).unwrap()
    }

    fn run(spec: Value) -> (Timeline, Vec<Note>) {
        let e = ev(spec.clone());
        let s = Samples::take(&e, 6.0, 160);
        time_axis(&e, &s, &intent(&spec, None), None)
    }

    #[test]
    fn intent_is_declared_or_inferred() {
        assert_eq!(intent(&json!({"intent": {"pace": "frantic"}}), Some(80.0)).pace, Pace::Frantic);
        assert_eq!(intent(&json!({}), Some(128.0)).pace, Pace::Fast);
        assert_eq!(intent(&json!({"look": "noir"}), None).pace, Pace::Calm);
        assert_eq!(intent(&json!({}), None).pace, Pace::Steady);
    }

    #[test]
    fn slow_fails_fast_intent_but_a_living_calm_hold_passes() {
        // One 8 s shot with a slow, continuous move.
        let scene = json!({ "duration": 8, "background": "#202020", "layers": [
            { "shape": "circle", "fill": "#e0e0e0", "width": 120, "height": 120, "animate": [ { "from": { "x": 60 }, "to": { "x": 260 }, "duration": 8, "ease": "linear" } ] } ] });
        let (_, n) = run(json!({ "size": [320, 180], "intent": {"pace": "fast"}, "scenes": [scene.clone()] }));
        assert!(n.iter().any(|x| x.check == "pace" && x.message.contains("fast intent")), "{n:?}");
        let (_, n) = run(json!({ "size": [320, 180], "intent": {"pace": "calm"}, "scenes": [scene] }));
        assert!(!n.iter().any(|x| x.check == "pace" || x.check == "dead-air"), "{n:?}");
    }

    #[test]
    fn calm_but_lifeless_is_flagged() {
        let (_, n) = run(json!({ "size": [320, 180], "intent": {"pace": "calm"}, "scenes": [ { "duration": 8, "background": "#202020", "layers": [ { "text": "Still", "color": "#ffffff" } ] } ] }));
        assert!(n.iter().any(|x| x.check == "dead-air"), "{n:?}");
    }

    #[test]
    fn beat_cuts_pass_fast_intent_and_monotony_is_noticed() {
        let colors = ["#101010", "#f0f0f0", "#d03020", "#2040d0"];
        let scenes: Vec<_> = (0..12).map(|i| json!({ "duration": 0.7, "transition": "cut", "background": colors[i % 4], "layers": [ { "text": format!("{i}"), "x": 40 + i * 20 } ] })).collect();
        let (tl, n) = run(json!({ "size": [320, 180], "intent": {"pace": "fast"}, "scenes": scenes }));
        assert!(!n.iter().any(|x| x.check == "pace"), "{n:?}");
        assert!(n.iter().any(|x| x.check == "rhythm"), "{n:?}");
        assert!(tl.shots.len() >= 11);
    }
}
