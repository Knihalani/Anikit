//! The composition engine: harmony per section (progressions by section role,
//! cadences, harmonic-rhythm acceleration, colour tones, inversions, borrowed
//! chords) and motif-based melody (a motif developed by repetition, sequence,
//! variation and an answer; the chorus hook returns note-for-note; a climax per
//! phrase; legato phrasing), plus guide-tone countermelodies.

use crate::arrange::ChordAt;
use crate::notes::Note;
use crate::theory::{Chord, Key};
use crate::voice::Rng;

/// The role a section plays in the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Intro,
    Verse,
    Build,
    Peak,
    Break,
    Outro,
}

impl Role {
    pub fn of(name: &str) -> Role {
        let n = name.to_ascii_lowercase();
        let has = |p: &[&str]| p.iter().any(|x| n.starts_with(x));
        if has(&["intro"]) {
            Role::Intro
        } else if has(&["build", "pre", "rise"]) {
            Role::Build
        } else if has(&["drop", "chorus", "hook", "tutti", "climax", "main", "solo", "bloom"]) {
            Role::Peak
        } else if has(&["break", "bridge"]) {
            Role::Break
        } else if has(&["outro", "end", "coda", "fade", "out"]) {
            Role::Outro
        } else {
            Role::Verse
        }
    }
}

/// How a style colours its harmony.
#[derive(Debug, Clone, Copy)]
pub struct Harmony {
    /// 0 = plain triads, 1 = add9/sus and dominant 7ths, 2 = 7ths everywhere, 3 = 9ths.
    pub colour: u8,
    /// Probability of a stepwise inversion in the bass.
    pub inversions: f32,
    /// Probability of a borrowed / secondary chord on repeats.
    pub borrowed: f32,
    /// Cadences into peaks and at the end.
    pub cadences: bool,
}

/// Harmony profile for a style.
pub fn harmony(style: &str) -> Harmony {
    match style {
        "lofi" | "jazz" => Harmony { colour: 3, inversions: 0.25, borrowed: 0.25, cadences: true },
        "ambient" => Harmony { colour: 2, inversions: 0.3, borrowed: 0.15, cadences: false },
        "house" | "dnb" => Harmony { colour: 2, inversions: 0.0, borrowed: 0.1, cadences: true },
        "synthwave" | "tech" => Harmony { colour: 1, inversions: 0.15, borrowed: 0.2, cadences: true },
        "trap" => Harmony { colour: 1, inversions: 0.0, borrowed: 0.15, cadences: false },
        "orchestral" | "piano" => Harmony { colour: 1, inversions: 0.55, borrowed: 0.3, cadences: true },
        "cinematic-hybrid" => Harmony { colour: 1, inversions: 0.3, borrowed: 0.35, cadences: true },
        "rock" => Harmony { colour: 0, inversions: 0.25, borrowed: 0.25, cadences: true },
        _ => Harmony { colour: 1, inversions: 0.45, borrowed: 0.25, cadences: true },
    }
}

fn chord(root: i32, ints: &[i32]) -> Chord {
    Chord { root: root.rem_euclid(12), intervals: ints.to_vec(), bass: None }
}

/// The dominant (major V, with a 7th when coloured) of `key`.
fn dominant(key: &Key, colour: u8) -> Chord {
    let r = key.tonic + 7;
    chord(r, if colour >= 1 { &[0, 4, 7, 10] } else { &[0, 4, 7] })
}

/// Scale degree (0-based) of a chord root in the key, if diatonic.
fn degree(key: &Key, root: i32) -> Option<usize> {
    key.scale.steps.iter().position(|&s| (key.tonic + s).rem_euclid(12) == root.rem_euclid(12))
}

/// Add colour tones to a plain triad according to its function.
pub fn colour(c: &Chord, key: &Key, level: u8, rng: &mut Rng) -> Chord {
    if level == 0 || c.intervals.len() != 3 {
        // Already extended (or power chord): lofi/jazz get 9ths on 7ths.
        if level >= 3 && c.intervals.len() == 4 && rng.f32() < 0.5 {
            let mut x = c.clone();
            x.intervals.push(14);
            return x;
        }
        return c.clone();
    }
    let minor = c.intervals[1] == 3;
    let dim = c.intervals[2] == 6;
    if dim {
        return c.clone();
    }
    let deg = degree(key, c.root);
    let tonic_like = deg == Some(0);
    let dominant_like = (c.root - key.tonic).rem_euclid(12) == 7 && !minor;
    let mut x = c.clone();
    let roll = rng.f32();
    x.intervals = match (minor, level) {
        (_, _) if dominant_like => vec![0, 4, 7, 10],
        (false, 1) if roll < 0.55 => {
            if tonic_like || roll < 0.3 {
                vec![0, 4, 7, 14] // add9
            } else {
                vec![0, 2, 7] // sus2 colour
            }
        }
        (false, 1) => vec![0, 4, 7],
        (false, 2) => vec![0, 4, 7, 11],
        (false, _) => vec![0, 4, 7, 11, 14],
        (true, 1) if roll < 0.5 => vec![0, 3, 7, 10],
        (true, 1) => vec![0, 3, 7],
        (true, 2) => vec![0, 3, 7, 10],
        (true, _) => vec![0, 3, 7, 10, 14],
    };
    x
}

/// Chords for every section: verse and chorus progressions differ, builds end on
/// the dominant, phrase ends accelerate into the next downbeat, colour tones and
/// stepwise bass inversions are added, borrowed chords appear on repeats, and the
/// piece cadences home. `progs` are the style's progressions (already parsed, one
/// chord per `hr` bars); `sections` are (name, bar, bars).
pub fn section_chords(progs: &[Vec<ChordAt>], sections: &[(String, f64, f64)], key: &Key, bpb: f64, h: &Harmony, seed: u64) -> Vec<Vec<ChordAt>> {
    let mut rng = Rng::new(seed ^ 0xC40D5);
    let n = progs.len().max(1);
    let verse_i = (seed as usize) % n;
    let peak_i = if n > 1 { (verse_i + 1) % n } else { 0 };
    let mut out = Vec::with_capacity(sections.len());
    for (si, (name, bar, bars)) in sections.iter().enumerate() {
        let role = Role::of(name);
        let next_role = sections.get(si + 1).map(|s| Role::of(&s.0));
        let p = match role {
            Role::Peak | Role::Outro => &progs[peak_i],
            Role::Build => &progs[(verse_i + 2) % n],
            _ => &progs[verse_i],
        };
        let span = bars * bpb;
        let mut cs = crate::arrange::loop_chords(p, span);
        // Borrowed / secondary chords on repeats (second half of long sections).
        let period = p.last().map(|c| c.t + c.len).unwrap_or(span);
        for c in cs.iter_mut() {
            if c.t >= period - 1e-9 && rng.f32() < h.borrowed {
                if let Some(d) = degree(key, c.chord.root) {
                    if !key.is_minor() && d == 3 && c.chord.intervals.get(1) == Some(&4) {
                        c.chord.intervals[1] = 3; // IV → iv (borrowed from minor)
                    } else if !key.is_minor() && d == 1 && c.chord.intervals.get(1) == Some(&3) {
                        c.chord.intervals[1] = 4; // ii → II (V/V)
                    } else if key.is_minor() && d == 4 && c.chord.intervals.get(1) == Some(&3) {
                        c.chord.intervals[1] = 4; // v → V (harmonic minor pull)
                    }
                }
            }
        }
        // Colour.
        for c in cs.iter_mut() {
            c.chord = colour(&c.chord, key, h.colour, &mut rng);
        }
        if h.cadences && span >= 2.0 * bpb {
            let last_bar = span - bpb;
            let into_peak = next_role == Some(Role::Peak) && role != Role::Peak;
            let ending = si + 1 == sections.len();
            if into_peak || role == Role::Build {
                // Half cadence: the last bar sits on the dominant.
                cs.retain(|c| c.t < last_bar - 1e-9);
                if let Some(l) = cs.last_mut() {
                    l.len = last_bar - l.t;
                }
                cs.push(ChordAt { chord: dominant(key, h.colour), t: last_bar, len: bpb });
            } else if ending {
                // Authentic cadence: … | IV (iv) V | (the final tonic rings after the last bar).
                cs.retain(|c| c.t < last_bar - 1e-9);
                if let Some(l) = cs.last_mut() {
                    l.len = last_bar - l.t;
                }
                let four = chord(key.tonic + 5, if key.is_minor() { &[0, 3, 7] } else { &[0, 4, 7] });
                cs.push(ChordAt { chord: colour(&four, key, h.colour, &mut rng), t: last_bar, len: bpb / 2.0 });
                cs.push(ChordAt { chord: dominant(key, h.colour), t: last_bar + bpb / 2.0, len: bpb / 2.0 });
            } else if span >= 8.0 * bpb && bpb >= 3.0 {
                // Phrase-end acceleration every 8 bars: a dominant on the last two beats before the loop restarts on the tonic.
                let mut k = 8.0 * bpb;
                while k < span - 1e-9 {
                    let t = k - bpb / 2.0;
                    let starts_tonic = cs.iter().find(|c| (c.t - k).abs() < 1e-9).is_some_and(|c| degree(key, c.chord.root) == Some(0));
                    if starts_tonic {
                        if let Some(i) = cs.iter().position(|c| c.t < t && c.t + c.len > t + 1e-9) {
                            let end = cs[i].t + cs[i].len;
                            cs[i].len = t - cs[i].t;
                            cs.insert(i + 1, ChordAt { chord: dominant(key, h.colour), t, len: end - t });
                        }
                    }
                    k += 8.0 * bpb;
                }
            }
        }
        // Stepwise bass: put the 3rd (or 5th) in the bass when it makes a step to the next root.
        if h.inversions > 0.0 {
            for i in 0..cs.len().saturating_sub(1) {
                let (cur, next) = (cs[i].chord.clone(), cs[i + 1].chord.clone());
                if cur.bass.is_some() || cur.intervals.len() < 3 {
                    continue;
                }
                let prev_bass = if i > 0 { Some(cs[i - 1].chord.bass_pc()) } else { None };
                let third = (cur.root + cur.intervals[1]).rem_euclid(12);
                let step = |a: i32, b: i32| {
                    let d = (b - a).rem_euclid(12);
                    d == 1 || d == 2 || d == 10 || d == 11
                };
                let root_step = step(cur.root, next.root) && prev_bass.is_none_or(|p| step(p, cur.root));
                if !root_step && step(third, next.bass_pc()) && prev_bass.is_none_or(|p| step(p, third)) && rng.f32() < h.inversions {
                    cs[i].chord.bass = Some(third);
                }
            }
        }
        cs.iter_mut().for_each(|c| c.t += bar * bpb);
        out.push(cs);
    }
    out
}

// ─────────────────────────────── melody ───────────────────────────────

/// A motif: onsets (beats within its span), durations and scale steps from its first note.
#[derive(Debug, Clone)]
struct Motif {
    on: Vec<f64>,
    dur: Vec<f64>,
    steps: Vec<i32>,
}

fn motif(rng: &mut Rng, bpb: f64, density: f64, rhythm: Option<&str>) -> Motif {
    let mut on: Vec<f64> = match rhythm {
        Some(r) => {
            let cells: Vec<char> = r.chars().filter(|c| !c.is_whitespace()).collect();
            let steps = (bpb * 4.0) as usize;
            (0..steps).filter(|&i| matches!(cells[i % cells.len()], 'x' | 'X')).map(|i| i as f64 * 0.25).collect()
        }
        None => {
            // Favour 8ths, one syncopation, a rest at the end of the bar.
            let want = ((density * bpb).round() as usize).clamp(2, (bpb * 2.0) as usize);
            let mut grid: Vec<f64> = (0..(bpb * 2.0) as usize).map(|i| i as f64 * 0.5).collect();
            grid.retain(|&t| t < bpb - 0.75);
            let mut chosen = vec![0.0];
            while chosen.len() < want && chosen.len() < grid.len() {
                let t = grid[rng.below(grid.len())];
                if !chosen.contains(&t) {
                    chosen.push(t);
                }
            }
            if rng.f32() < 0.5 {
                // Syncopate one onset by a 16th.
                if let Some(x) = chosen.iter_mut().skip(1).find(|x| (**x * 2.0).fract() == 0.0 && (**x).fract() != 0.0) {
                    *x -= 0.25;
                }
            }
            chosen.sort_by(f64::total_cmp);
            chosen.dedup();
            chosen
        }
    };
    if on.is_empty() {
        on.push(0.0);
    }
    let dur: Vec<f64> = on.iter().enumerate().map(|(i, &t)| on.get(i + 1).copied().unwrap_or(bpb) - t).collect();
    // Contour: mostly steps, at most one leap, which then steps back.
    let mut steps = vec![0];
    let mut cur = 0;
    let mut leapt = false;
    let up = rng.f32() < 0.6;
    for i in 1..on.len() {
        let s = if !leapt && i == 1 && rng.f32() < 0.45 {
            leapt = true;
            if up { 3 + rng.below(2) as i32 } else { -3 }
        } else if leapt && steps.len() == 2 {
            if steps[1] > 0 { -1 } else { 1 }
        } else {
            [1, 1, -1, -1, 2, -2, 0][rng.below(7)]
        };
        cur += s;
        steps.push(cur);
    }
    Motif { on, dur, steps }
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

fn nearest_tone(k: i32, pcs: &[i32]) -> i32 {
    (0..=6).flat_map(|d| [k - d, k + d]).find(|x| pcs.contains(&x.rem_euclid(12))).unwrap_or(k)
}

fn chord_at(ch: &[ChordAt], t: f64) -> Option<&ChordAt> {
    let i = ch.partition_point(|c| c.t <= t + 1e-9);
    i.checked_sub(1).map(|i| &ch[i])
}

/// A melody over `chords` (section-local beats from 0): an 8-bar sentence built
/// from one motif — statement, sequence on the next chord, variation, answer
/// (half cadence), statement, sequence, climax, full cadence — repeated to fill.
/// The same `motif_seed` gives the same tune (choruses reuse it: the hook returns).
pub fn melody(chords: &[ChordAt], key: &Key, center: i32, density: f64, vel: f32, rhythm: Option<&str>, motif_seed: u64, bpb: f64, ending: bool) -> Vec<Note> {
    let total = chords.last().map(|c| c.t + c.len).unwrap_or(0.0);
    if total <= 0.0 {
        return Vec::new();
    }
    let mut rng = Rng::new(motif_seed ^ 0x6D6F7469);
    let m = motif(&mut rng, bpb, density, rhythm);
    // A short variation: drop the last onset and hold the one before.
    let var = if m.on.len() > 2 {
        let mut v = m.clone();
        v.on.pop();
        v.steps.pop();
        let last = v.on.len() - 1;
        v.dur[last] = bpb - v.on[last];
        v.dur.truncate(v.on.len());
        v
    } else {
        m.clone()
    };
    let tonic_pcs = [key.tonic.rem_euclid(12), (key.tonic + key.scale.steps[2]).rem_euclid(12), (key.tonic + key.scale.steps[4 % key.scale.steps.len()]).rem_euclid(12)];
    let mut out = Vec::new();
    let mut prev = key.snap(center);
    let bars = (total / bpb).ceil() as usize;
    for b in 0..bars {
        let t0 = b as f64 * bpb;
        if t0 >= total - 1e-9 {
            break;
        }
        let pos = b % 8;
        let Some(c) = chord_at(chords, t0) else { continue };
        let pcs = c.chord.pcs();
        let last_bar = b + 1 == bars;
        // Cadence bars: one or two long notes.
        if pos == 3 || pos == 7 || last_bar {
            let full = pos == 7 || (last_bar && ending);
            let target = if full { nearest_tone(prev, &tonic_pcs) } else { nearest_tone(prev, &pcs) };
            let target = clamp_range(target, center);
            if rng.f32() < 0.6 {
                // Pickup into the long note.
                let pk = scale_step(key, target, if target > prev { -1 } else { 1 });
                out.push(Note { t: t0, len: 0.45, key: pk as f32, vel: vel * 0.85 });
                out.push(Note { t: t0 + 0.5, len: (bpb - 0.5) * 0.95, key: target as f32, vel: vel * 0.95 });
            } else {
                out.push(Note { t: t0, len: bpb * 0.95, key: target as f32, vel: vel * 0.95 });
            }
            prev = target;
            continue;
        }
        let mot = if pos == 2 { &var } else { &m };
        // Anchor: the motif's first note on the nearest chord tone (sequence);
        // bar 7 (pos 6) is the climax, lifted by a third.
        let lift = if pos == 6 { 2 } else { 0 };
        let start = clamp_range(nearest_tone(scale_step(key, prev, lift), &pcs), center);
        for (i, (&on, &du)) in mot.on.iter().zip(&mot.dur).enumerate() {
            let t = t0 + on;
            if t >= total - 1e-9 {
                break;
            }
            let mut k = scale_step(key, start, mot.steps[i]);
            // Strong beats snap to chord tones.
            if (on % 1.0).abs() < 1e-9 {
                if let Some(cc) = chord_at(chords, t) {
                    k = nearest_tone(k, &cc.chord.pcs());
                }
            }
            let k = clamp_range(k, center);
            // Phrase arch: louder towards the climax bar, accents on the beat.
            let arch = 0.85 + 0.15 * ((pos as f32 + on as f32 / bpb as f32) / 7.0 * std::f32::consts::PI).sin();
            let acc = if (on % 1.0).abs() < 1e-9 { 1.0 } else { 0.88 };
            out.push(Note { t, len: du, key: k as f32, vel: (vel * arch * acc).min(1.0) });
            prev = k;
        }
    }
    // Legato: notes run into the next one when the gap is small; detached otherwise.
    for i in 0..out.len() {
        let next = out.get(i + 1).map(|n| n.t);
        let n = &mut out[i];
        n.len = match next {
            Some(nt) if nt - n.t <= 1.0 + 1e-9 => (nt - n.t) * 0.98,
            Some(nt) => (n.len.min(nt - n.t)) * 0.9,
            None => n.len.min(total - n.t),
        };
    }
    out
}

fn clamp_range(mut k: i32, center: i32) -> i32 {
    while k > center + 9 {
        k -= 12;
    }
    while k < center - 8 {
        k += 12;
    }
    k
}

/// A guide-tone countermelody: the 3rd or 7th of each chord, voice-led by the
/// smallest step, held for the chord.
pub fn counter(chords: &[ChordAt], center: i32, vel: f32) -> Vec<Note> {
    let mut prev = center;
    let mut out = Vec::new();
    for c in chords {
        let ints = &c.chord.intervals;
        let mut cands: Vec<i32> = Vec::new();
        if ints.len() > 1 {
            cands.push((c.chord.root + ints[1]).rem_euclid(12));
        }
        if let Some(&s) = ints.iter().find(|&&i| i == 10 || i == 11) {
            cands.push((c.chord.root + s).rem_euclid(12));
        }
        if cands.is_empty() {
            cands.push(c.chord.root);
        }
        let k = clamp_range(nearest_tone(prev, &cands), center);
        out.push(Note { t: c.t, len: c.len * 0.98, key: k as f32, vel });
        prev = k;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(s: &str, key: &Key) -> Vec<ChordAt> {
        crate::arrange::chord_line(s, key, 4.0).unwrap()
    }

    #[test]
    fn sections_get_their_own_harmony_with_cadences() {
        let key = Key::parse("C").unwrap();
        let progs = vec![prog("I V vi IV", &key), prog("vi IV I V", &key), prog("IV I V vi", &key)];
        let secs = vec![("intro".to_string(), 0.0, 4.0), ("verse".into(), 4.0, 8.0), ("chorus".into(), 12.0, 8.0), ("outro".into(), 20.0, 4.0)];
        let h = harmony("pop");
        let c = section_chords(&progs, &secs, &key, 4.0, &h, 1);
        // The verse ends on the dominant before the chorus.
        let v_last = c[1].last().unwrap();
        assert_eq!(v_last.chord.root, 7);
        // Verse and chorus differ.
        let roots = |x: &Vec<ChordAt>| x.iter().take(4).map(|c| c.chord.root).collect::<Vec<_>>();
        assert_ne!(roots(&c[1]), roots(&c[2]));
        // The outro ends IV V.
        let o = &c[3];
        assert_eq!(o[o.len() - 1].chord.root, 7);
        assert_eq!(o[o.len() - 2].chord.root, 5);
        // Colour: some chords are extended.
        assert!(c.iter().flatten().any(|x| x.chord.intervals.len() > 3 || x.chord.intervals == vec![0, 2, 7]));
    }

    #[test]
    fn melody_has_a_motif_and_a_returning_hook() {
        let key = Key::parse("Am").unwrap();
        let ch = crate::arrange::loop_chords(&prog("Am F C G", &key), 32.0);
        let a = melody(&ch, &key, 72, 1.0, 0.8, None, 42, 4.0, false);
        let b = melody(&ch, &key, 72, 1.0, 0.8, None, 42, 4.0, false);
        assert_eq!(a, b, "same seed → same hook");
        assert!(a.iter().all(|n| key.contains(n.key as i32)));
        // Bars 1 and 5 state the same motif (same rhythm).
        let rhythm = |bar: f64| a.iter().filter(|n| n.t >= bar * 4.0 && n.t < bar * 4.0 + 4.0).map(|n| n.t - bar * 4.0).collect::<Vec<_>>();
        assert_eq!(rhythm(0.0), rhythm(4.0));
        // Mostly steps.
        let iv: Vec<i32> = a.windows(2).map(|w| (w[1].key - w[0].key).abs() as i32).collect();
        let steps = iv.iter().filter(|&&d| d <= 2).count() as f32 / iv.len() as f32;
        assert!(steps >= 0.5, "steps {steps}");
    }
}
