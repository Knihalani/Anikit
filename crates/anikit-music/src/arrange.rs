//! Arrangement helpers: chord timelines, ways to play chords (pads, stabs,
//! arpeggios, strums, broken piano patterns, rhythms), basslines from chords,
//! and a motif-based melody generator. All deterministic from a seed.

use crate::notes::Note;
use crate::theory::{Chord, Key};
use crate::voice::Rng;

/// A chord on the timeline (beats).
#[derive(Debug, Clone, PartialEq)]
pub struct ChordAt {
    pub chord: Chord,
    pub t: f64,
    pub len: f64,
}

/// Parse `"Am F C G"` (one bar each), `"Am/2 F/2 | C G"` (half notes), `"Am:3 G:1"` (beats);
/// `%` or `_` repeats the previous chord.
pub fn chord_line(s: &str, key: &Key, beats_per_bar: f64) -> Result<Vec<ChordAt>, String> {
    let mut out: Vec<ChordAt> = Vec::new();
    let mut t = 0.0;
    for tok in s.split(|c: char| c.is_whitespace() || c == ',').filter(|x| !x.is_empty()) {
        if tok == "|" {
            let bar = (t / beats_per_bar - 1e-9).ceil();
            t = (bar * beats_per_bar).max(t);
            continue;
        }
        let tok = tok.trim_matches('|');
        // Duration suffix: ":beats" or "/N" where N is a number (G/B is a slash chord).
        let (name, len) = if let Some((a, b)) = tok.split_once(':') {
            (a, b.parse::<f64>().map_err(|_| format!("chords: '{tok}': write durations as Am:2 (beats) or Am/2 (half note)"))?)
        } else {
            match tok.rfind('/') {
                Some(i) if tok[i + 1..].chars().next().is_some_and(|c| c.is_ascii_digit()) && !tok.ends_with("6/9") => {
                    let n: f64 = tok[i + 1..].trim_end_matches('.').parse().map_err(|_| format!("chords: '{tok}': bad duration"))?;
                    let dotted = tok.ends_with('.');
                    (&tok[..i], 4.0 / n * if dotted { 1.5 } else { 1.0 })
                }
                _ => (tok, beats_per_bar),
            }
        };
        let chord = if name == "%" || name == "_" {
            out.last().map(|c| c.chord.clone()).ok_or_else(|| format!("chords: '{tok}' repeats nothing"))?
        } else {
            Chord::parse(name, key).map_err(|e| format!("chords: {e}"))?
        };
        out.push(ChordAt { chord, t, len });
        t += len;
    }
    if out.is_empty() {
        return Err("chords: empty progression".into());
    }
    Ok(out)
}

/// Repeat a chord timeline to cover `beats` (looping), clipped at `beats`.
pub fn loop_chords(chords: &[ChordAt], beats: f64) -> Vec<ChordAt> {
    let period = chords.last().map(|c| c.t + c.len).unwrap_or(0.0);
    let mut out = Vec::new();
    if period <= 0.0 {
        return out;
    }
    let mut base = 0.0;
    while base < beats - 1e-9 {
        for c in chords {
            let t = base + c.t;
            if t >= beats - 1e-9 {
                break;
            }
            out.push(ChordAt { chord: c.chord.clone(), t, len: c.len.min(beats - t) });
        }
        base += period;
    }
    out
}

/// The chord sounding at beat `t`.
pub fn chord_at(chords: &[ChordAt], t: f64) -> Option<&ChordAt> {
    let i = chords.partition_point(|c| c.t <= t + 1e-9);
    i.checked_sub(1).map(|i| &chords[i])
}

/// How to play a chord timeline.
#[derive(Debug, Clone, PartialEq)]
pub enum Play {
    /// Sustained, voice-led chords (with a low root for weight).
    Pad,
    /// Held block chords, re-struck each bar.
    Block,
    /// Short off-beat stabs (house / reggae skank).
    Stabs,
    /// Arpeggio: pattern (`up`, `down`, `updown`, `random`, `pinky`) and step in beats.
    Arp(String, f64),
    /// Guitar strums with a down/up pattern (one char per 8th: D, U, .).
    Strum(String),
    /// Repeated chords every `step` beats.
    Pulse(f64),
    /// Broken piano pattern (left-hand bass + right-hand arpeggio in 8ths).
    Broken,
    /// A rhythm grid of 16ths (`x` hit, `X` accent, `.` rest) for block chords.
    Rhythm(String),
    /// Power chords (root, fifth, octave) chugging every `step` beats.
    Power(f64),
}

impl Play {
    /// `"pad" "block" "stabs" "arp" "arp-updown/16" "strum" "strum:D.DU.UDU" "pulse/8" "broken" "x..x..x."`.
    pub fn parse(s: &str) -> Result<Play, String> {
        let s = s.trim();
        let step = |suffix: Option<&str>, default: f64| -> Result<f64, String> {
            match suffix {
                None => Ok(default),
                Some(n) => n.parse::<f64>().map(|n| 4.0 / n).map_err(|_| format!("play '{s}': step as /8 or /16")),
            }
        };
        let (head, suf) = match s.split_once('/') {
            Some((a, b)) => (a, Some(b)),
            None => (s, None),
        };
        Ok(match head {
            "pad" | "sustain" | "hold" => Play::Pad,
            "block" | "chords" => Play::Block,
            "stabs" | "stab" | "skank" => Play::Stabs,
            "broken" | "ballad" => Play::Broken,
            "pulse" => Play::Pulse(step(suf, 0.5)?),
            "power" => Play::Power(step(suf, 0.5)?),
            "strum" => Play::Strum("D.DU.UDU".into()),
            h if h.starts_with("strum:") => Play::Strum(h[6..].to_string()),
            "arp" => Play::Arp("up".into(), step(suf, 0.25)?),
            h if h.starts_with("arp-") => Play::Arp(h[4..].to_string(), step(suf, 0.25)?),
            h if !h.is_empty() && h.chars().all(|c| matches!(c, 'x' | 'X' | '.' | '-')) => Play::Rhythm(h.to_string()),
            _ => return Err(format!("play '{s}': use pad, block, stabs, arp[-up|-down|-updown|-random|-pinky][/16], strum[:D.DU.UDU], pulse[/8], power[/8], broken or a rhythm like x..x..x.")),
        })
    }
}

/// Render chords as notes. `center` is the MIDI key the voicing gravitates to
/// (≈ 60 for keys, 55 for pads), `n` voices per chord.
pub fn play_chords(chords: &[ChordAt], play: &Play, center: i32, n: usize, vel: f32, seed: u64) -> Vec<Note> {
    let mut out = Vec::new();
    let mut rng = Rng::new(seed);
    let mut prev: Vec<i32> = Vec::new();
    for c in chords {
        let mut v = c.chord.voice_lead(if prev.is_empty() { &[] } else { &prev }, n);
        if prev.is_empty() {
            v = c.chord.voice(n, center);
        }
        // Keep the voicing near the centre over a long progression.
        let avg = v.iter().sum::<i32>() / v.len().max(1) as i32;
        if avg - center > 5 {
            v.iter_mut().for_each(|k| *k -= 12);
        } else if center - avg > 6 {
            v.iter_mut().for_each(|k| *k += 12);
        }
        prev = v.clone();
        let bass = {
            let pc = c.chord.bass_pc();
            let lo = v.first().copied().unwrap_or(center) - 12;
            lo - (lo - pc).rem_euclid(12)
        };
        let (t0, len) = (c.t, c.len);
        let push = |out: &mut Vec<Note>, t: f64, l: f64, k: i32, vel: f32| {
            if t < t0 + len - 1e-9 && l > 0.0 {
                out.push(Note { t, len: l.min(t0 + len - t), key: k as f32, vel: vel.clamp(0.02, 1.0) });
            }
        };
        match play {
            Play::Pad => {
                for &k in &v {
                    push(&mut out, t0, len, k, vel * 0.85);
                }
                push(&mut out, t0, len, bass, vel * 0.7);
            }
            Play::Block => {
                let mut t = t0;
                while t < t0 + len - 1e-9 {
                    let l = 4.0f64.min(t0 + len - t);
                    for &k in &v {
                        push(&mut out, t, l * 0.95, k, vel);
                    }
                    push(&mut out, t, l * 0.95, bass, vel * 0.8);
                    t += 4.0;
                }
            }
            Play::Stabs => {
                let mut t = t0 + 0.5;
                while t < t0 + len - 1e-9 {
                    for &k in &v {
                        push(&mut out, t, 0.22, k, vel * if (t - t0) % 2.0 > 1.0 { 1.0 } else { 0.9 });
                    }
                    t += 1.0;
                }
            }
            Play::Pulse(step) => {
                let mut t = t0;
                let mut i = 0;
                while t < t0 + len - 1e-9 {
                    let accent = if i % 2 == 0 { 1.0 } else { 0.8 };
                    for &k in &v {
                        push(&mut out, t, step * 0.8, k, vel * accent);
                    }
                    t += step;
                    i += 1;
                }
            }
            Play::Power(step) => {
                let r = center - (center - c.chord.bass_pc()).rem_euclid(12);
                let mut t = t0;
                let mut i = 0;
                while t < t0 + len - 1e-9 {
                    let accent = if i % 4 == 0 { 1.0 } else if i % 2 == 0 { 0.85 } else { 0.72 };
                    for k in [r, r + 7, r + 12] {
                        push(&mut out, t, step * 0.85, k, vel * accent);
                    }
                    t += step;
                    i += 1;
                }
            }
            Play::Rhythm(pat) => {
                let cells: Vec<char> = pat.chars().collect();
                let mut i = 0usize;
                let mut t = t0;
                while t < t0 + len - 1e-9 {
                    let c = cells[i % cells.len()];
                    if c == 'x' || c == 'X' {
                        // Hold until the next hit (max 1 beat).
                        let mut l = 0.25;
                        let mut j = i + 1;
                        while j < i + 4 && !matches!(cells[j % cells.len()], 'x' | 'X') {
                            l += 0.25;
                            j += 1;
                        }
                        let a = if c == 'X' { 1.1 } else { 0.9 };
                        for &k in &v {
                            push(&mut out, t, l * 0.85, k, vel * a);
                        }
                    }
                    t += 0.25;
                    i += 1;
                }
            }
            Play::Arp(pat, step) => {
                // Chord tones over two octaves.
                let mut tones: Vec<i32> = v.clone();
                tones.extend(v.iter().map(|k| k + 12));
                tones.sort();
                tones.dedup();
                let m = tones.len();
                let seq: Vec<usize> = match pat.as_str() {
                    "down" => (0..m).rev().collect(),
                    "updown" => (0..m).chain((1..m - 1).rev()).collect(),
                    "pinky" => (0..m - 1).flat_map(|i| [i, m - 1]).collect(),
                    "random" => (0..m * 4).map(|_| rng.below(m)).collect(),
                    _ => (0..m).collect(),
                };
                let mut t = t0;
                let mut i = 0;
                while t < t0 + len - 1e-9 {
                    let accent = if ((t - t0) % 1.0).abs() < 1e-9 { 1.0 } else { 0.82 };
                    push(&mut out, t, step * 0.9, tones[seq[i % seq.len()]], vel * accent);
                    t += step;
                    i += 1;
                }
            }
            Play::Strum(pat) => {
                // Guitar-like voicing: bass + spread chord tones (6 strings).
                let mut strings: Vec<i32> = vec![bass + 12];
                let mut tones = c.chord.voice(4, center - 2);
                tones.extend(c.chord.voice(2, center + 7));
                tones.sort();
                tones.dedup();
                strings.extend(tones.iter().filter(|&&k| k > bass + 12));
                let cells: Vec<char> = pat.chars().collect();
                let mut t = t0;
                let mut i = 0;
                while t < t0 + len - 1e-9 {
                    let c = cells[i % cells.len()];
                    if c == 'D' || c == 'U' || c == 'd' || c == 'u' {
                        let down = c.eq_ignore_ascii_case(&'D');
                        let order: Vec<i32> = if down { strings.clone() } else { strings.iter().rev().take(4).copied().collect() };
                        let a = if down { 1.0 } else { 0.7 } * if c.is_ascii_lowercase() { 0.6 } else { 1.0 };
                        // Ring until the next stroke.
                        let mut l = 0.5;
                        let mut j = i + 1;
                        while j < i + 8 && cells[j % cells.len()] == '.' {
                            l += 0.5;
                            j += 1;
                        }
                        // No two strokes alike: the sweep speed varies ±20 %, the hand hits a
                        // little harder on the beat than between, and each stroke's level wanders ±8 %.
                        let spread = 0.018 * if down { 1.0 } else { 0.8 } * (0.8 + 0.4 * rng.f32() as f64);
                        let on_beat = (t - t.floor()).abs() < 1e-6;
                        let accent = if on_beat { 1.0 } else { 0.88 } * (0.92 + 0.16 * rng.f32());
                        for (s, &k) in order.iter().enumerate() {
                            let dt = s as f64 * spread;
                            push(&mut out, t + dt, l - dt, k, vel * a * accent * (1.0 - 0.04 * s as f32));
                        }
                    }
                    t += 0.5;
                    i += 1;
                }
            }
            Play::Broken => {
                // LH: bass on 1 (and 3 in 4/4); RH: 8th-note arpeggio 5-8-3-8 pattern.
                let fifth = bass + 7;
                let mut rh: Vec<i32> = v.clone();
                rh.sort();
                let pat = [bass, fifth, bass + 12, *rh.get(1).unwrap_or(&(bass + 16)), rh.last().copied().unwrap_or(bass + 19), *rh.get(1).unwrap_or(&(bass + 16)), bass + 12, fifth];
                let mut t = t0;
                let mut i = 0;
                while t < t0 + len - 1e-9 {
                    let k = pat[i % pat.len()];
                    let a = if i % 4 == 0 { 0.95 } else { 0.7 };
                    // Pedal: notes ring to the end of the chord.
                    push(&mut out, t, (t0 + len - t).min(4.0), k, vel * a);
                    t += 0.5;
                    i += 1;
                }
            }
        }
    }
    out
}

/// Bassline patterns.
pub const BASS_PATTERNS: &[&str] = &["root", "half", "pulse", "octave", "walk", "house", "synco", "rock", "arp", "808", "sub"];

/// A bassline from chords. `oct` is the octave of the root (2 = around C2).
/// `kicks` (beats) drive the `808` pattern.
pub fn bassline(chords: &[ChordAt], pattern: &str, key: &Key, oct: i32, vel: f32, kicks: &[f64], seed: u64) -> Result<Vec<Note>, String> {
    let mut out = Vec::new();
    let mut rng = Rng::new(seed);
    let root_of = |c: &Chord| {
        let pc = c.bass_pc();
        let base = (oct + 1) * 12;
        let mut k = base + pc;
        // Keep it within E1..D#3-ish for oct=2 (avoid jumping a 7th up).
        if pc > 7 {
            k -= 12;
        }
        k
    };
    for (ci, c) in chords.iter().enumerate() {
        let r = root_of(&c.chord);
        let fifth = r + 7;
        let next_root = chords.get(ci + 1).map(|n| root_of(&n.chord));
        let (t0, len) = (c.t, c.len);
        let mut push = |t: f64, l: f64, k: i32, v: f32| {
            if t < t0 + len - 1e-9 {
                out.push(Note { t, len: l.min(t0 + len - t), key: k as f32, vel: v.clamp(0.05, 1.0) });
            }
        };
        let mut steps = |step: f64, f: &mut dyn FnMut(usize, f64) -> Option<(i32, f64, f32)>| {
            let mut t = t0;
            let mut i = 0;
            while t < t0 + len - 1e-9 {
                if let Some((k, l, v)) = f(i, t) {
                    push(t, l, k, v);
                }
                t += step;
                i += 1;
            }
        };
        match pattern {
            "root" | "sub" => steps(len.max(0.25), &mut |_, _| Some((r, len * 0.98, vel))),
            "half" => steps(2.0, &mut |i, _| Some((if i % 2 == 1 && len >= 4.0 { fifth } else { r }, 1.9, vel))),
            "pulse" => steps(0.5, &mut |i, _| Some((r, 0.42, vel * if i % 2 == 0 { 1.0 } else { 0.8 }))),
            "octave" => steps(0.5, &mut |i, _| Some((if i % 2 == 1 { r + 12 } else { r }, 0.4, vel * if i % 2 == 0 { 1.0 } else { 0.85 }))),
            "rock" => steps(0.5, &mut |i, t| {
                let last = t + 0.5 >= t0 + len - 1e-9;
                Some((if last && next_root.is_some() { fifth } else { r }, 0.45, vel * if i % 2 == 0 { 1.0 } else { 0.85 }))
            }),
            "house" => steps(0.5, &mut |i, _| (i % 2 == 1).then_some((r, 0.35, vel))),
            "arp" => steps(0.5, &mut |i, _| Some(([r, fifth, r + 12, fifth][i % 4], 0.45, vel * if i % 2 == 0 { 1.0 } else { 0.8 }))),
            "synco" => {
                // 1, the "and" of 2, beat 4 (lofi / hip-hop).
                for (dt, l, k, a) in [(0.0, 1.4, r, 1.0), (1.5, 0.5, r, 0.85), (3.0, 0.9, fifth - 12 + 12, 0.8)] {
                    let mut tt = t0 + dt;
                    while tt < t0 + len - 1e-9 {
                        push(tt, l, k, vel * a);
                        tt += 4.0;
                    }
                }
            }
            "walk" => {
                // Quarter notes: root, chord tone, scale tone, chromatic approach to the next root.
                let tones = c.chord.pcs();
                let third = r + (tones.get(1).copied().unwrap_or(c.chord.root + 4) - c.chord.root).rem_euclid(12);
                steps(1.0, &mut |i, t| {
                    let beats_left = t0 + len - t;
                    let k = if i == 0 {
                        r
                    } else if beats_left <= 1.0 + 1e-9 {
                        match next_root {
                            Some(n) => n + if rng.f32() < 0.5 { -1 } else { 1 },
                            None => fifth,
                        }
                    } else if i % 2 == 1 {
                        third
                    } else {
                        key.snap(fifth + if rng.f32() < 0.4 { 2 } else { 0 })
                    };
                    Some((k, 0.9, vel * if i == 0 { 1.0 } else { 0.85 }))
                })
            }
            "808" => {
                // Notes on the kicks inside this chord, held until the next kick.
                let ks: Vec<f64> = kicks.iter().copied().filter(|&k| k >= t0 - 1e-9 && k < t0 + len - 1e-9).collect();
                if ks.is_empty() {
                    push(t0, len, r, vel);
                }
                // Occasional octave jump on later hits, the trap signature; the note
                // before a jump runs into it so the 808 slides (mono glide on legato).
                let keys: Vec<i32> = (0..ks.len()).map(|i| if i > 0 && rng.f32() < 0.2 { r + 12 } else { r }).collect();
                for (i, &k) in ks.iter().enumerate() {
                    let end = ks.get(i + 1).copied().unwrap_or(t0 + len);
                    let slide = keys.get(i + 1).is_some_and(|&n| n != keys[i]);
                    let l = if slide { end - k + 0.06 } else { (end - k) * 0.97 };
                    push(k, l, keys[i], vel * if i == 0 { 1.0 } else { 0.9 });
                }
            }
            other => {
                return Err(format!("bass pattern '{other}': use one of {}", BASS_PATTERNS.join(", ")));
            }
        }
    }
    Ok(out)
}

/// Generate a melody over `chords`: a 2-bar motif repeated in an A A' B A″
/// phrase shape, chord tones on strong beats, stepwise motion elsewhere,
/// ending on a stable tone. `density` ≈ notes per beat (0.5…2), `center`
/// the register (MIDI key), `rhythm` an optional 16th grid for the motif.
#[allow(clippy::too_many_arguments)]
pub fn melody(chords: &[ChordAt], key: &Key, center: i32, density: f64, vel: f32, rhythm: Option<&str>, seed: u64, beats_per_bar: f64) -> Vec<Note> {
    let mut rng = Rng::new(seed ^ 0x6d656c);
    let total = chords.last().map(|c| c.t + c.len).unwrap_or(0.0);
    if total <= 0.0 {
        return Vec::new();
    }
    let motif_len = 2.0 * beats_per_bar;
    // Motif rhythm: 16th grid onsets within two bars.
    let steps = (motif_len * 4.0).round() as usize;
    let onsets: Vec<(usize, usize)> = match rhythm {
        Some(r) => {
            let cells: Vec<char> = r.chars().filter(|c| !c.is_whitespace()).collect();
            let hits: Vec<usize> = (0..steps).filter(|&i| matches!(cells[i % cells.len()], 'x' | 'X')).collect();
            hits.iter().enumerate().map(|(j, &i)| (i, hits.get(j + 1).copied().unwrap_or(steps) - i)).collect()
        }
        None => {
            let mut hits = vec![0usize];
            let p = (density / 4.0).clamp(0.05, 0.9);
            for i in 1..steps - 4 {
                let strong = i % 4 == 0;
                let eighth = i % 2 == 0;
                let pr = if strong { p * 2.2 } else if eighth { p * 1.3 } else { p * 0.5 };
                if rng.f32() < pr as f32 {
                    hits.push(i);
                }
            }
            // Leave room to breathe at the end of the motif.
            hits.iter().enumerate().map(|(j, &i)| (i, hits.get(j + 1).copied().unwrap_or(steps) - i)).collect()
        }
    };
    // Contour of the motif in scale steps (relative), reused with variation.
    let contour: Vec<i32> = onsets.iter().map(|_| [-2, -1, -1, 0, 1, 1, 2, 3][rng.below(8)]).collect();
    let mut out = Vec::new();
    let mut prev = center;
    let n_motifs = (total / motif_len).ceil() as usize;
    for m in 0..n_motifs {
        let base = m as f64 * motif_len;
        let phrase = m % 4; // A A' B A''
        let last_phrase = m + 1 == n_motifs;
        for (j, &(i, dur)) in onsets.iter().enumerate() {
            let t = base + i as f64 * 0.25;
            if t >= total - 1e-9 {
                break;
            }
            // B section: different contour (inverted), sparser.
            if phrase == 2 && j % 3 == 2 {
                continue;
            }
            let step = match phrase {
                2 => -contour[j],
                1 if j + 1 == onsets.len() => contour[j] + 1,
                _ => contour[j],
            };
            let Some(c) = chord_at(chords, t) else { continue };
            let strong = i % 4 == 0;
            let mut k = scale_step(key, prev, step);
            if strong {
                // Nearest chord tone.
                let pcs = c.chord.pcs();
                k = nearest_pc(k, &pcs);
            }
            // Stay in range (≈ an octave and a half around the centre).
            while k > center + 9 {
                k -= 12;
            }
            while k < center - 8 {
                k += 12;
            }
            let is_end = last_phrase && j + 1 == onsets.len() || t + dur as f64 * 0.25 >= total - 1e-9;
            if is_end {
                k = nearest_pc(k, &[key.tonic, (key.tonic + key.scale.steps[2]) % 12]);
            }
            let l = (dur as f64 * 0.25).min(total - t);
            let a = if strong { 1.0 } else { 0.85 } * (0.95 + 0.1 * rng.f32());
            out.push(Note { t, len: l * 0.9, key: k as f32, vel: (vel * a).min(1.0) });
            prev = k;
        }
    }
    out
}

fn scale_step(key: &Key, from: i32, steps: i32) -> i32 {
    let mut k = key.snap(from);
    let dir = steps.signum();
    for _ in 0..steps.abs() {
        k += dir;
        while !key.contains(k) {
            k += dir;
        }
    }
    k
}

fn nearest_pc(k: i32, pcs: &[i32]) -> i32 {
    let mut best = k;
    let mut bd = i32::MAX;
    for d in -6..=6 {
        if pcs.contains(&(k + d).rem_euclid(12)) && d.abs() < bd {
            bd = d.abs();
            best = k + d;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_lines_and_loops() {
        let k = Key::parse("Am").unwrap();
        let c = chord_line("Am F/2 G/2 | C:3 E7:1", &k, 4.0).unwrap();
        assert_eq!(c.iter().map(|x| (x.t, x.len)).collect::<Vec<_>>(), vec![(0.0, 4.0), (4.0, 2.0), (6.0, 2.0), (8.0, 3.0), (11.0, 1.0)]);
        let l = loop_chords(&c, 30.0);
        assert_eq!(l.len(), 12);
        assert_eq!(l.last().unwrap().t, 28.0);
        assert_eq!(chord_at(&l, 4.5).unwrap().chord.root, 5);
        assert!(chord_line("Zm", &k, 4.0).is_err());
    }

    #[test]
    fn plays_and_basses_stay_in_their_chords() {
        let k = Key::parse("C").unwrap();
        let c = loop_chords(&chord_line("C Am F G", &k, 4.0).unwrap(), 16.0);
        for play in ["pad", "block", "stabs", "arp-updown/16", "strum", "pulse/8", "power", "broken", "x..x..x.x..x..x."] {
            let notes = play_chords(&c, &Play::parse(play).unwrap(), 60, 4, 0.8, 1);
            assert!(!notes.is_empty(), "{play}");
            for n in &notes {
                let ch = chord_at(&c, n.t + 0.001).unwrap();
                assert!(ch.chord.pcs().contains(&(n.key as i32 % 12)), "{play}: {n:?}");
                assert!(n.t + n.len <= ch.t + ch.len + 0.06, "{play}: {n:?} overruns its chord");
            }
        }
        for p in BASS_PATTERNS {
            let notes = bassline(&c, p, &k, 2, 0.9, &[0.0, 2.5, 4.0, 6.5], 3).unwrap();
            assert!(!notes.is_empty(), "{p}");
            assert!(notes.iter().all(|n| (28..=55).contains(&(n.key as i32))), "{p}: {:?}", notes.iter().map(|n| n.key).collect::<Vec<_>>());
        }
        assert!(bassline(&c, "nope", &k, 2, 0.9, &[], 3).is_err());
    }

    #[test]
    fn melody_is_in_key_and_deterministic() {
        let k = Key::parse("Am").unwrap();
        let c = loop_chords(&chord_line("Am F C G", &k, 4.0).unwrap(), 32.0);
        let m = melody(&c, &k, 72, 1.0, 0.8, None, 9, 4.0);
        assert!(m.len() > 10);
        assert!(m.iter().all(|n| k.contains(n.key as i32)));
        assert_eq!(m, melody(&c, &k, 72, 1.0, 0.8, None, 9, 4.0));
        let last = m.last().unwrap().key as i32 % 12;
        assert!(last == 9 || last == 0, "ends on a stable tone: {last}");
    }
}

#[cfg(test)]
mod strum_tests {
    use super::*;

    #[test]
    fn strums_vary_stroke_to_stroke() {
        let chords = chord_line("C G Am F", &crate::theory::Key::parse("C").unwrap(), 4.0).unwrap();
        let notes = play_chords(&chords, &Play::Strum("D.DU.UDU".into()), 60, 4, 0.8, 7);
        // Group notes into strokes by start time (a stroke spans < 0.15 beat).
        let mut strokes: Vec<Vec<&Note>> = Vec::new();
        let mut sorted: Vec<&Note> = notes.iter().collect();
        sorted.sort_by(|a, b| a.t.total_cmp(&b.t));
        for n in sorted {
            match strokes.last_mut() {
                Some(s) if n.t - s[0].t < 0.15 => s.push(n),
                _ => strokes.push(vec![n]),
            }
        }
        let spans: Vec<f64> = strokes.iter().filter(|s| s.len() >= 4).map(|s| s[s.len() - 1].t - s[0].t).collect();
        let (lo, hi) = spans.iter().fold((f64::MAX, 0f64), |(a, b), &x| (a.min(x), b.max(x)));
        assert!(hi > lo * 1.2, "sweep speeds differ: {lo:.4}–{hi:.4}");
        let first: Vec<f32> = strokes.iter().map(|s| s[0].vel).collect();
        let mean = first.iter().sum::<f32>() / first.len() as f32;
        let sd = (first.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / first.len() as f32).sqrt();
        assert!(sd > 0.05, "stroke levels vary: sd {sd}");
        // Deterministic.
        assert_eq!(notes, play_chords(&chords, &Play::Strum("D.DU.UDU".into()), 60, 4, 0.8, 7));
    }
}
