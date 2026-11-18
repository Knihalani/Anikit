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
            Pace::Fast => Limits { longest: 3.0, median: 2.0, same: 0.2, dead_air: 1.0 },
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
    /// The same frames averaged over 4×4 blocks: motion is measured on these,
    /// so film grain and dither don't read as movement.
    pub coarse: Vec<Vec<u8>>,
}

fn coarsen(f: &[u8], w: usize, h: usize) -> Vec<u8> {
    let (cw, ch) = (w / 4, h / 4);
    let mut out = vec![0u8; cw * ch * 3];
    for cy in 0..ch {
        for cx in 0..cw {
            for c in 0..3 {
                let mut t = 0u32;
                for y in cy * 4..cy * 4 + 4 {
                    for x in cx * 4..cx * 4 + 4 {
                        t += f[(y * w + x) * 3 + c] as u32;
                    }
                }
                out[(cy * cw + cx) * 3 + c] = (t / 16) as u8;
            }
        }
    }
    out
}

impl Samples {
    pub fn take(ev: &Evaluator, fps: f32, width: u32) -> Samples {
        let step = 1.0 / fps;
        let scale = (width as f32 / ev.width.max(1) as f32).min(1.0);
        // Shaders shade at the sample size too (restored afterwards).
        crate::eval::shader::set_preview_scale(scale);
        let n = (ev.duration / step).floor() as usize + 1;
        let mut w = 0;
        let mut h = 0;
        let frames: Vec<Vec<u8>> = (0..n)
            .map(|i| {
                let mut dl = ev.eval((i as f32 * step).min(ev.duration - 0.01).max(0.0));
                pipeline::scale_list(&mut dl, scale);
                let (fw, fh, px) = pipeline::render_list_rgba(ev, &dl);
                w = fw;
                h = fh;
                px.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
            })
            .collect();
        crate::eval::shader::set_preview_scale(1.0);
        let coarse = frames.iter().map(|f| coarsen(f, w as usize, h as usize)).collect();
        Samples { step, w, h, frames, coarse }
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
    let energy_raw: Vec<f32> = (1..n).map(|i| diff(&s.coarse[i - 1], &s.coarse[i])).collect();
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
    let secs: Vec<f32> = (per..n).step_by(per).map(|i| diff(&s.coarse[i - per], &s.coarse[i])).collect();
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
        // The last shot may hold on purpose (a lockup, an end card).
        let body_longest = if tl.shots.len() > 2 { tl.shots[..tl.shots.len() - 1].iter().copied().fold(0.0f32, f32::max) } else { tl.longest };
        if body_longest > lim.longest {
            slow.push(format!("longest shot {:.1} s > {:.1} s", body_longest, lim.longest));
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
            // Shot lengths are advice (holds can be deliberate); static seconds,
            // where nothing changes, are the stronger signal.
            let sev = if tl.same > lim.same { "warning" } else { "tip" };
            note(sev, "pace", format!("timeline ({} shots, median {:.1} s)", tl.shots.len(), tl.median), format!("slower than the {} intent ({}): {}", intent.pace.name(), intent.source, slow.join("; ")), fix);
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
            let last = diff(&s.coarse[n - 2], &s.coarse[n - 1]);
            let typical = {
                let mut v = energy_raw.clone();
                v.sort_by(|a, b| a.total_cmp(b));
                v[v.len() / 2]
            };
            if last > 3.0 && last > typical * 1.5 {
                note("warning", "ending", format!("t={dur:.1}s"), "the video ends mid-motion: it will feel cut off".into(), "end on a held frame (0.5–1.5 s) or a deliberate hard cut to black on a beat");
            }
        }
        // Flashes: one sample (≥ ~0.17 s) that differs hard from both neighbours, which agree.
        // Single-frame flashes need full-rate sampling (PERF metrics).
        let mut flashes = Vec::new();
        for i in 1..n.saturating_sub(1) {
            for w in 1..=2usize {
                if i + w >= n {
                    break;
                }
                let (a, b, c) = (diff(&s.coarse[i - 1], &s.coarse[i]), diff(&s.coarse[i + w - 1], &s.coarse[i + w]), diff(&s.coarse[i - 1], &s.coarse[i + w]));
                if a > 40.0 && b > 40.0 && c < 12.0 && flashes.last().is_none_or(|t| s.time(i) - t > 0.5) {
                    flashes.push(s.time(i));
                    break;
                }
            }
        }
        if !flashes.is_empty() {
            note("warning", "flicker", format!("t={:.1}s", flashes[0]), format!("{} single-frame flash/pop{} (first at t={:.1}s): reads as a glitch unless it's on purpose", flashes.len(), if flashes.len() == 1 { "" } else { "s" }, flashes[0]), "check the cut or transition there: a layer appearing for one frame, a background flashing, or a mismatched cut");
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

/// Detail map of a frame: 1 where the pixel has local contrast (content), 0 where it is smooth (empty or a soft gradient).
fn detail(s: &Samples, i: usize) -> Vec<u8> {
    let (w, h) = (s.w as usize, s.h as usize);
    let f = &s.frames[i];
    let lum = |x: usize, y: usize| {
        let k = (y * w + x) * 3;
        (f[k] as i32 * 2 + f[k + 1] as i32 * 5 + f[k + 2] as i32) / 8
    };
    let mut d = vec![0u8; w * h];
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let c = lum(x, y);
            let g = (c - lum(x + 1, y)).abs().max((c - lum(x, y + 1)).abs()).max((c - lum(x - 1, y)).abs()).max((c - lum(x, y - 1)).abs());
            d[y * w + x] = (g > 10) as u8;
        }
    }
    d
}

/// Fill (share of pixels with detail), detail centroid (0..1) and its bounding box.
struct Layout {
    fill: f32,
    /// Area of the subject's box (the largest detailed region), as a share of the frame.
    occupied: f32,
    cx: f32,
    cy: f32,
    bbox: [f32; 4],
    balance: f32,
    hash: u64,
}

fn layout(s: &Samples, i: usize) -> Layout {
    let (w, h) = (s.w as usize, s.h as usize);
    let d = detail(s, i);
    let (mut n, mut sx, mut sy, mut left) = (0f32, 0f32, 0f32, 0f32);
    for y in 0..h {
        for x in 0..w {
            if d[y * w + x] == 1 {
                let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
                n += 1.0;
                sx += fx;
                sy += fy;
                left += (fx < 0.5) as u8 as f32;
            }
        }
    }
    // The subject: the largest connected region of detailed blocks (dilated by one
    // block so words join up). Thin rules and corner labels form small regions.
    let bs = (w / 32).max(2);
    let (gw, gh) = (w.div_ceil(bs), h.div_ceil(bs));
    let mut grid = vec![false; gw * gh];
    for gy in 0..gh {
        for gx in 0..gw {
            let mut c = 0;
            for y in gy * bs..((gy + 1) * bs).min(h) {
                for x in gx * bs..((gx + 1) * bs).min(w) {
                    c += d[y * w + x] as usize;
                }
            }
            grid[gy * gw + gx] = c * 4 >= bs * bs;
        }
    }
    let mut dil = grid.clone();
    for gy in 0..gh {
        for gx in 0..gw {
            if grid[gy * gw + gx] {
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (x, y) = (gx as i32 + dx, gy as i32 + dy);
                    if x >= 0 && y >= 0 && (x as usize) < gw && (y as usize) < gh {
                        dil[y as usize * gw + x as usize] = true;
                    }
                }
            }
        }
    }
    let mut seen = vec![false; gw * gh];
    let mut best = (0usize, [0usize; 4]);
    for start in 0..gw * gh {
        if !dil[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let (mut cnt, mut b) = (0usize, [usize::MAX, usize::MAX, 0usize, 0usize]);
        while let Some(k) = stack.pop() {
            let (x, y) = (k % gw, k / gw);
            cnt += grid[k] as usize;
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < gw && (ny as usize) < gh {
                    let nk = ny as usize * gw + nx as usize;
                    if dil[nk] && !seen[nk] {
                        seen[nk] = true;
                        stack.push(nk);
                    }
                }
            }
        }
        if cnt > best.0 {
            best = (cnt, b);
        }
    }
    let b = best.1;
    let bb = if best.0 == 0 { [0.5, 0.5, 0.5, 0.5] } else { [b[0] as f32 / gw as f32, b[1] as f32 / gh as f32, (b[2] + 1) as f32 / gw as f32, (b[3] + 1) as f32 / gh as f32] };
    let fill = n / (w * h) as f32;
    // A dHash of the detail-weighted luminance (9×8), for spotting repeated layouts.
    let f = &s.frames[i];
    let cell = |gx: usize, gy: usize| {
        let (x0, x1, y0, y1) = (gx * w / 9, (gx + 1) * w / 9, gy * h / 8, (gy + 1) * h / 8);
        let mut t = 0u32;
        let mut c = 0u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let k = (y * w + x) * 3;
                t += (f[k] as u32 + f[k + 1] as u32 + f[k + 2] as u32) / 3;
                c += 1;
            }
        }
        t / c.max(1)
    };
    let mut hash = 0u64;
    for gy in 0..8 {
        for gx in 0..8 {
            hash = (hash << 1) | (cell(gx, gy) > cell(gx + 1, gy)) as u64;
        }
    }
    Layout { fill, occupied: (bb[2] - bb[0]).max(0.0) * (bb[3] - bb[1]).max(0.0), cx: if n > 0.0 { sx / n } else { 0.5 }, cy: if n > 0.0 { sy / n } else { 0.5 }, bbox: bb, balance: if n > 0.0 { left / n } else { 0.5 }, hash }
}

/// A frame worth looking at: (detector, time, box in 0..1 frame coordinates).
pub type Worst = (String, f32, [f32; 4]);

/// Mid-shot sample indices.
fn shot_frames(s: &Samples, tl: &Timeline, dur: f32) -> Vec<(usize, f32)> {
    let mut bounds = vec![0.0];
    bounds.extend(tl.cuts.iter().cloned());
    bounds.push(dur);
    bounds
        .windows(2)
        .filter(|w| w[1] - w[0] > 0.05)
        .map(|w| {
            let t = w[0] + (w[1] - w[0]) * 0.6;
            (((t / s.step).round() as usize).min(s.frames.len() - 1), t)
        })
        .collect()
}

/// Composition at real pixels, one frame per shot: frame fill, centre bias,
/// layout repetition, and BAR #1 (a centred headline over a smooth backdrop).
pub fn composition(ev: &Evaluator, s: &Samples, tl: &Timeline) -> (Vec<Note>, Vec<Worst>) {
    let mut notes = Vec::new();
    let mut worst: Vec<Worst> = Vec::new();
    let shots = shot_frames(s, tl, ev.duration);
    if shots.is_empty() {
        return (notes, worst);
    }
    let lays: Vec<(f32, Layout)> = shots.iter().map(|(i, t)| (*t, layout(s, *i))).collect();
    if std::env::var_os("ANIKIT_REVIEW_DEBUG").is_some() {
        for (t, l) in &lays {
            eprintln!("t={t:.1} fill={:.3} occupied={:.3} c=({:.2},{:.2}) balance={:.2} bbox={:?}", l.fill, l.occupied, l.cx, l.cy, l.balance, l.bbox);
        }
    }
    // Small content in an empty frame (BAR #4), ignoring the end card.
    let body = if lays.len() > 2 { &lays[..lays.len() - 1] } else { &lays[..] };
    let empty: Vec<&(f32, Layout)> = body.iter().filter(|(_, l)| l.fill < 0.035 || l.occupied < 0.08).collect();
    if !empty.is_empty() {
        let (t, l) = empty.iter().min_by(|a, b| a.1.occupied.total_cmp(&b.1.occupied)).unwrap();
        worst.push(("empty-frame".into(), *t, l.bbox));
        notes.push(Note {
            severity: if empty.len() * 3 >= body.len() { "warning" } else { "tip" },
            check: "frame-fill",
            path: format!("t={t:.1}s"),
            message: format!("{} of {} shots are mostly empty (the content sits in {:.0}% of the frame at t={t:.1}s; BAR #4)", empty.len(), body.len(), l.occupied * 100.0),
            fix: "fill the frame: bigger subject, a world behind it (depth, texture, light), crop in tighter".into(),
        });
    }
    // Centred, symmetric layouts repeated.
    let mid = |l: &Layout| (((l.bbox[0] + l.bbox[2]) / 2.0 - 0.5).abs(), ((l.bbox[1] + l.bbox[3]) / 2.0 - 0.5).abs());
    // A full-bleed image (a shader world, a photo) fills the frame: its box is
    // centred by definition, so it says nothing about the composition.
    let centred: Vec<&(f32, Layout)> = lays.iter().filter(|(_, l)| mid(l).0 < 0.06 && mid(l).1 < 0.12 && l.occupied < 0.8).collect();
    if lays.len() >= 4 && centred.len() * 10 >= lays.len() * 7 {
        let (t, l) = centred[0];
        worst.push(("centred".into(), *t, l.bbox));
        notes.push(Note {
            severity: "warning",
            check: "layout",
            path: format!("{} shots", lays.len()),
            message: format!("{} of {} shots are centred and symmetric: it reads as slides", centred.len(), lays.len()),
            fix: "compose off-centre (thirds, a strong diagonal, flush-left type against a subject), and vary it shot to shot".into(),
        });
    }
    // Centred headline over a smooth backdrop (BAR #1–2), at pixels.
    // Only shots with words on screen: a small centred image (an eye, an
    // object) in a wordless film is a composition choice, not a slide.
    let has_text = |t: f32| ev.eval(t).ops.iter().any(|o| matches!(o, crate::dl::Op::Glyphs { .. }));
    let slide: Vec<&(f32, Layout)> = lays.iter().filter(|(t, l)| l.occupied < 0.2 && ((l.bbox[0] + l.bbox[2]) / 2.0 - 0.5).abs() < 0.08 && has_text(*t)).collect();
    if !slide.is_empty() {
        let (t, l) = slide[0];
        worst.push(("slide".into(), *t, l.bbox));
        notes.push(Note {
            severity: "warning",
            check: "bar",
            path: format!("t={t:.1}s"),
            message: format!("{} shot(s) look like a centred headline on a soft backdrop (BAR #1–2), first at t={t:.1}s", slide.len()),
            fix: "give the shot a subject with light and material, and place the type with intent".into(),
        });
    }
    // Repeated layouts: non-adjacent shots that hash alike.
    let mut pairs = Vec::new();
    for i in 0..lays.len() {
        for j in i + 2..lays.len() {
            if (lays[i].1.hash ^ lays[j].1.hash).count_ones() <= 6 {
                pairs.push((lays[i].0, lays[j].0));
            }
        }
    }
    if pairs.len() >= 2 {
        worst.push(("repeat".into(), pairs[0].1, [0.0, 0.0, 1.0, 1.0]));
        notes.push(Note {
            severity: "tip",
            check: "layout",
            path: format!("t={:.1}s ~ t={:.1}s", pairs[0].0, pairs[0].1),
            message: format!("{} pairs of shots share a layout", pairs.len()),
            fix: "change the composition each shot: scale, angle, crop, side".into(),
        });
    }
    (notes, worst)
}

/// Write the review pack: a 1 fps strip, the frame after every cut, the worst
/// frames per detector with the region boxed, and `review.md` (a rubric
/// answered by looking at the images).
pub fn write_pack(dir: &std::path::Path, p: &crate::preview::Preview, tl: &Timeline, worst: &[Worst], report_text: &str) -> Result<Vec<String>, String> {
    use crate::preview::png;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let w = |name: &str, img: &crate::preview::Image| std::fs::write(dir.join(name), png(img)).map_err(|e| format!("{name}: {e}"));
    let mut files = Vec::new();
    let dur = p.ev.duration;
    let secs: Vec<f32> = (0..dur.ceil() as usize).map(|i| (i as f32 + 0.5).min(dur - 0.05)).collect();
    w("strip.png", &p.sheet(Some(secs), 240))?;
    files.push("strip.png".to_string());
    for (k, c) in tl.cuts.iter().enumerate() {
        let name = format!("cut-{:02}-t{:.1}.png", k + 1, c);
        // Just before and just after the cut, side by side.
        w(&name, &p.sheet(Some(vec![(c - 0.07).max(0.0), c + 0.07]), 480))?;
        files.push(name);
    }
    for (k, (what, t, bb)) in worst.iter().enumerate() {
        let mut img = p.frame(*t, Some(960));
        let (iw, ih) = (img.width as f32, img.height as f32);
        let r = [bb[0] * iw, bb[1] * ih, bb[2] * iw, bb[3] * ih];
        for (x0, y0, x1, y1) in [(r[0], r[1], r[2], r[1] + 3.0), (r[0], r[3] - 3.0, r[2], r[3]), (r[0], r[1], r[0] + 3.0, r[3]), (r[2] - 3.0, r[1], r[2], r[3])] {
            for y in (y0.max(0.0) as u32)..(y1.min(ih) as u32) {
                for x in (x0.max(0.0) as u32)..(x1.min(iw) as u32) {
                    let k = ((y * img.width + x) * 4) as usize;
                    img.rgba[k..k + 4].copy_from_slice(&[255, 40, 60, 255]);
                }
            }
        }
        let name = format!("worst-{}-{what}-t{t:.1}.png", k + 1);
        w(&name, &img)?;
        files.push(name);
    }
    let peak = tl.energy.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i as f32 + 0.5).unwrap_or(0.0);
    let wow = p.frame(peak.min(dur - 0.05), Some(960));
    w(&format!("peak-t{peak:.1}.png"), &wow)?;
    files.push(format!("peak-t{peak:.1}.png"));
    let md = format!(
        "# Review pack\n\nLook at the images. Answer each question in one line, then fix the three weakest answers. A score is not a quality verdict: this is.\n\n```\n{report_text}```\n\nImages: {}\n\n1. `strip.png` (1 frame per second): what is the subject of each second? Name any second without one.\n2. Where is the one 'wow' moment? (`peak-*.png` is the energy peak.) If there is none, which shot should become it?\n3. `cut-*.png` (before and after each cut): which cuts are motivated (a match, the beat, a reveal) and which just happen? Any pop or flash?\n4. `worst-*.png`: is the boxed problem real? What exactly would you change?\n5. Would a top studio post any single frame as is? Which one, and what would they change about the others?\n6. With the sound off, does the story read? Where does it lose you?\n7. Against BAR.md: which frames show a centred headline on a gradient, placeholder UI, an empty frame, flat 2D, or a stock look?\n",
        files.join(", ")
    );
    std::fs::write(dir.join("review.md"), md).map_err(|e| e.to_string())?;
    files.push("review.md".into());
    Ok(files)
}

/// Audio levels of the final mix: approximate loudness (gated RMS, ≈ LUFS), peak and clipping.
pub fn audio(ev: &Evaluator, base: &std::path::Path) -> (Option<String>, Vec<Note>) {
    if ev.audio.is_empty() && ev.music.is_none() {
        return (None, Vec::new());
    }
    // Generated music is rendered to a temporary file and measured with the rest.
    let mut tracks = ev.audio.clone();
    let tmp = std::env::temp_dir().join(format!("anikit-review-{}.music.wav", std::process::id()));
    if let Some(m) = &ev.music {
        if let Ok(t) = crate::music::render_track(m, ev.duration as f64, &tmp, !ev.audio.is_empty()) {
            tracks.push(t);
        }
    }
    let mixed = crate::audio::mix(&tracks, ev.duration as f64, 24_000, Some(base));
    let _ = std::fs::remove_file(&tmp);
    let Ok(pcm) = mixed else { return (None, Vec::new()) };
    let mono: Vec<f32> = pcm.samples.chunks(2).map(|c| (c[0] + c.get(1).copied().unwrap_or(c[0])) * 0.5).collect();
    let peak = pcm.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let clipped = pcm.samples.iter().filter(|x| x.abs() >= 0.999).count();
    // 400 ms blocks, gated at −70 dB absolute and −10 dB relative (BS.1770-style, unweighted).
    let blk = (pcm.rate as usize * 2) / 5;
    let ms: Vec<f64> = mono.chunks(blk).filter(|c| c.len() == blk).map(|c| c.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / c.len() as f64).collect();
    let db = |m: f64| -0.691 + 10.0 * m.max(1e-12).log10();
    let abs: Vec<f64> = ms.iter().copied().filter(|m| db(*m) > -70.0).collect();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let rel = db(mean(&abs)) - 10.0;
    let gated: Vec<f64> = abs.iter().copied().filter(|m| db(*m) > rel).collect();
    let lufs = if gated.is_empty() { -70.0 } else { db(mean(&gated)) };
    let peak_db = 20.0 * (peak.max(1e-9) as f64).log10();
    let mut notes = Vec::new();
    if clipped > 0 {
        notes.push(Note { severity: "error", check: "audio", path: "audio".into(), message: format!("{clipped} clipped samples (peak {peak_db:.1} dBFS)"), fix: "lower the loudest cue's volume, or let the music engine master it (\"music\")".into() });
    } else if peak_db > -1.0 {
        notes.push(Note { severity: "warning", check: "audio", path: "audio".into(), message: format!("peak {peak_db:.1} dBFS leaves no headroom (true peak may clip after encoding)"), fix: "keep peaks under −1 dBFS".into() });
    }
    if lufs < -20.0 && lufs > -69.0 {
        notes.push(Note { severity: "warning", check: "audio", path: "audio".into(), message: format!("quiet: ≈{lufs:.0} LUFS (web and social normalise to about −14)"), fix: "raise the music/voice volume towards −14 LUFS".into() });
    } else if lufs > -9.0 {
        notes.push(Note { severity: "warning", check: "audio", path: "audio".into(), message: format!("very loud: ≈{lufs:.0} LUFS (platforms will turn it down and it may sound crushed)"), fix: "lower the volume towards −14 LUFS".into() });
    }
    (Some(format!("audio: ≈{lufs:.1} LUFS, peak {peak_db:.1} dBFS")), notes)
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

    /// The user-rejected slow shot (docs/video/quality/rejected/lighthouse-slow.jpg).
    /// It is slow against a fast intent, but not punished under a calm one.
    #[test]
    fn rejected_slow_shot_is_judged_by_intent() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review");
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("lighthouse-slow.json")).unwrap()).unwrap();
        let mut fast = spec.clone();
        fast["intent"] = json!({ "pace": "fast" });
        let r = crate::critique::review(&fast, &dir, None, true);
        assert!(r.notes.iter().any(|n| n.check == "pace" && n.message.contains("fast intent")), "{}", r.to_text());
        let mut calm = spec;
        calm["intent"] = json!({ "pace": "calm" });
        let r = crate::critique::review(&calm, &dir, None, true);
        assert!(!r.notes.iter().any(|n| matches!(n.check, "pace" | "dead-air" | "ending")), "{}", r.to_text());
        assert!(r.summary.contains("intent: calm (declared)"), "{}", r.summary);
    }

    #[test]
    fn composition_finds_empty_and_centred_slides() {
        let scenes: Vec<_> = (0..5).map(|i| json!({ "duration": 1.2, "transition": "cut", "background": "linear-gradient(135deg, #1a1040, #402080)", "layers": [ { "text": format!("Slide {i}"), "size": 22, "color": "#ffffff" } ] })).collect();
        let spec = json!({ "size": [640, 360], "scenes": scenes });
        let e = ev(spec.clone());
        let s = Samples::take(&e, 6.0, 192);
        let (tl, _) = time_axis(&e, &s, &intent(&spec, None), None);
        let (n, worst) = composition(&e, &s, &tl);
        let checks: Vec<&str> = n.iter().map(|x| x.check).collect();
        assert!(checks.contains(&"frame-fill") && checks.contains(&"layout") && checks.contains(&"bar"), "{n:?}");
        assert!(!worst.is_empty());
        // A full, off-centre frame is not flagged.
        let full = json!({ "size": [640, 360], "scenes": (0..5).map(|i| json!({ "duration": 1.2, "transition": "cut", "background": "#101010", "layers": [
            { "shape": "rect", "fill": "#d0d0d0", "width": 300, "height": 300, "x": 180 + i * 20, "y": 180 },
            { "text": "A very large headline", "size": 48, "color": "#ffffff", "x": 470, "y": 120 + i * 10 } ] })).collect::<Vec<_>>() });
        let e = ev(full.clone());
        let s = Samples::take(&e, 6.0, 192);
        let (tl, _) = time_axis(&e, &s, &intent(&full, None), None);
        let (n, _) = composition(&e, &s, &tl);
        assert!(!n.iter().any(|x| x.check == "frame-fill" || x.check == "bar"), "{n:?}");
    }

    #[test]
    fn flash_frames_and_audio_levels() {
        let spec = json!({ "size": [320, 180], "scenes": [
            { "duration": 1.5, "background": "#101010", "layers": [ { "text": "A", "color": "#888888" } ] },
            { "duration": 0.25, "transition": "cut", "background": "#ffffff", "layers": [] },
            { "duration": 1.5, "transition": "cut", "background": "#101010", "layers": [ { "text": "A", "color": "#888888" } ] } ] });
        let (_, n) = run(spec);
        assert!(n.iter().any(|x| x.check == "flicker"), "{n:?}");
        let loud = ev(json!({ "size": [320, 180], "audio": [ { "sfx": "impact", "at": 0.5, "volume": 12 } ], "scenes": [ { "duration": 2, "layers": [] } ] }));
        let (line, n) = audio(&loud, std::path::Path::new("."));
        assert!(line.is_some() && n.iter().any(|x| x.check == "audio"), "{line:?} {n:?}");
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
