//! The note language: a compact, token-cheap way for agents to write music.
//!
//! ```text
//! notes:  "o4 l8 C4 E4 G4 C5/4 | Am/2 F/2 | (1 3 5 3)x2 | [C4 E4 G4]/2. r/4"
//! drums:  {"kick": "x...x...x...x...", "snare": "....X.......X..o", "hat": "x.x.x.x.x.x.x.3."}
//! ```
//!
//! Notes language (tokens separated by spaces; durations attach to a token):
//! - pitches: `C4` `Eb3` `F#5` (MIDI octave, C4 = middle C); scale degrees
//!   `1`…`7` in the track's key with `b`/`#` prefix and `'`/`,` octave marks
//!   (`5,` = fifth below, `3'` = third above); `r` or `-` rest; `_` holds the
//!   previous note(s) longer.
//! - chords: symbols `Am` `Cmaj` `Cmaj7` `Cdom7` `G/B` `F#m7b5`, Roman numerals `vi` `IV` `V7`,
//!   or explicit `[C4 E4 G4]` / `[1 3 5]`; voiced near the current octave.
//!   A letter with an octave is a note (`C7` = C in octave 7); a bare letter is a note in
//!   the current octave (`o7 C`). Chords need a quality: `Cmaj` `Am` `Cdom7` `[C E G]`.
//! - durations: `/4` quarter (1 beat), `/8`, `/16`, `/2`, `/1`, `/8t` triplet;
//!   `.` dotted; `:1.5` explicit beats. Default from `l8` / `l:0.5`.
//! - marks: `!` accent, `?` ghost, `^` staccato, `~` legato (tied into the next onset).
//! - directives: `o3` octave, `>` `<` octave up/down, `v100` velocity (1–127),
//!   `l16` default length, `t+2` transpose, `|` bar line (pads to the next bar),
//!   `@5` jump to bar 5, `(…)x4` repeat.

use crate::theory::{note_key, Chord, Key};

/// A note on the timeline, in beats (quarter notes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub t: f64,
    pub len: f64,
    pub key: f32,
    pub vel: f32,
}

/// Parsing context.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub key: Key,
    pub octave: i32,
    pub beats_per_bar: f64,
    /// Default note length in beats.
    pub len: f64,
    pub vel: f32,
}

impl Default for Ctx {
    fn default() -> Self {
        Ctx { key: Key::default(), octave: 4, beats_per_bar: 4.0, len: 1.0, vel: 0.75 }
    }
}

/// Parse a notes string. Returns the notes and the total length in beats.
pub fn parse(src: &str, ctx: &Ctx) -> Result<(Vec<Note>, f64), String> {
    let tokens = tokenize(src);
    let mut p = Parser { ctx: ctx.clone(), t: 0.0, out: Vec::new(), last: Vec::new(), transpose: 0, src };
    p.run(&tokens)?;
    let end = p.t;
    Ok((p.out, end))
}

fn tokenize(s: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut start = 0;
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' => {
                if depth == 0 && !cur.is_empty() {
                    out.push((start, std::mem::take(&mut cur)));
                }
                if cur.is_empty() {
                    start = i;
                }
                depth += 1;
                cur.push(c);
            }
            ']' => {
                depth -= 1;
                cur.push(c);
            }
            c if depth > 0 => cur.push(c),
            c if c.is_whitespace() || c == ',' && cur.is_empty() => {
                if !cur.is_empty() {
                    out.push((start, std::mem::take(&mut cur)));
                }
            }
            '|' | '(' | '>' | '<' => {
                if !cur.is_empty() {
                    out.push((start, std::mem::take(&mut cur)));
                }
                out.push((i, c.to_string()));
            }
            ')' => {
                if !cur.is_empty() {
                    out.push((start, std::mem::take(&mut cur)));
                }
                cur.push(')');
                start = i;
            }
            _ => {
                if cur.is_empty() {
                    start = i;
                }
                cur.push(c);
            }
        }
    }
    if !cur.is_empty() {
        out.push((start, cur));
    }
    out
}

struct Parser<'a> {
    ctx: Ctx,
    t: f64,
    out: Vec<Note>,
    /// Indices in `out` of the last sounding event (for `_` and `~`).
    last: Vec<usize>,
    transpose: i32,
    src: &'a str,
}

impl Parser<'_> {
    fn err(&self, pos: usize, tok: &str, msg: &str) -> String {
        let line_start = self.src[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = self.src[line_start..pos].chars().count();
        format!("notes: '{tok}' at column {}: {msg}", col + 1)
    }

    fn run(&mut self, toks: &[(usize, String)]) -> Result<(), String> {
        let mut i = 0;
        while i < toks.len() {
            let (pos, tok) = (&toks[i].0, toks[i].1.as_str());
            if tok == "(" {
                // Find the matching ")xN".
                let mut depth = 1;
                let mut j = i + 1;
                while j < toks.len() {
                    if toks[j].1 == "(" {
                        depth += 1;
                    } else if toks[j].1.starts_with(')') {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    j += 1;
                }
                if j >= toks.len() {
                    return Err(self.err(*pos, tok, "unclosed '(' — write (…)x2"));
                }
                let rep = &toks[j].1[1..];
                let n: usize = if rep.is_empty() {
                    1
                } else {
                    rep.trim_start_matches(['x', '*']).parse().map_err(|_| self.err(toks[j].0, &toks[j].1, "repeat as ')x4'"))?
                };
                for _ in 0..n.min(10_000) {
                    self.run(&toks[i + 1..j])?;
                }
                i = j + 1;
                continue;
            }
            self.token(*pos, tok)?;
            i += 1;
        }
        Ok(())
    }

    fn token(&mut self, pos: usize, tok: &str) -> Result<(), String> {
        let bpb = self.ctx.beats_per_bar;
        match tok {
            "|" => {
                let bar = (self.t / bpb - 1e-9).ceil();
                self.t = (bar * bpb).max(self.t);
                return Ok(());
            }
            ">" => {
                self.ctx.octave += 1;
                return Ok(());
            }
            "<" => {
                self.ctx.octave -= 1;
                return Ok(());
            }
            _ => {}
        }
        if tok.starts_with(')') {
            return Err(self.err(pos, tok, "')' without '('"));
        }
        // Directives.
        if let Some(r) = tok.strip_prefix('o').filter(|r| r.parse::<i32>().is_ok()) {
            self.ctx.octave = r.parse().unwrap();
            return Ok(());
        }
        if let Some(r) = tok.strip_prefix('v').filter(|r| r.parse::<u32>().is_ok()) {
            self.ctx.vel = (r.parse::<f32>().unwrap() / 127.0).clamp(0.0, 1.0);
            return Ok(());
        }
        if let Some(r) = tok.strip_prefix('l').filter(|r| !r.is_empty() && (r.starts_with(':') || r.starts_with(|c: char| c.is_ascii_digit()))) {
            let (d, _) = self.duration(pos, tok, if r.starts_with(':') { r } else { &tok[0..0] }, if r.starts_with(':') { "" } else { r })?;
            self.ctx.len = d;
            return Ok(());
        }
        if let Some(r) = tok.strip_prefix('t').filter(|r| r.starts_with(['+', '-'])) {
            self.transpose = r.parse().map_err(|_| self.err(pos, tok, "transpose as t+2 or t-12"))?;
            return Ok(());
        }
        if let Some(r) = tok.strip_prefix('@') {
            let bar: f64 = r.parse().map_err(|_| self.err(pos, tok, "jump to a bar as @5"))?;
            self.t = (bar - 1.0).max(0.0) * bpb;
            return Ok(());
        }
        // Split marks and duration off the pitch part.
        let (head, dur, marks) = split_token(tok);
        let (len, _) = self.duration(pos, tok, dur, "")?;
        let mut vel = self.ctx.vel;
        let mut gate = 0.92;
        let mut legato = false;
        for m in marks.chars() {
            match m {
                '!' => vel = (vel * 1.25 + 0.1).min(1.0),
                '?' => vel *= 0.45,
                '^' => gate = 0.45,
                '~' => legato = true,
                _ => {}
            }
        }
        if legato {
            gate = 1.0;
        }
        match head {
            "r" | "-" => {
                self.t += len;
                self.last.clear();
                return Ok(());
            }
            "_" => {
                for &i in &self.last {
                    self.out[i].len += len;
                }
                self.t += len;
                return Ok(());
            }
            _ => {}
        }
        let keys = self.pitches(pos, tok, head)?;
        self.last.clear();
        for k in keys {
            let k = k + self.transpose;
            if !(0..=127).contains(&k) {
                return Err(self.err(pos, tok, &format!("key {k} is outside MIDI 0–127")));
            }
            self.last.push(self.out.len());
            self.out.push(Note { t: self.t, len: len * gate, key: k as f32, vel });
        }
        self.t += len;
        Ok(())
    }

    /// `dur` is the suffix (`/8.`, `:1.5`, `/8t`); `bare` a bare number for `l8`.
    fn duration(&self, pos: usize, tok: &str, dur: &str, bare: &str) -> Result<(f64, bool), String> {
        let bad = || self.err(pos, tok, "durations: /4 (quarter), /8, /16, /2, /1, /8t (triplet), /4. (dotted) or :1.5 (beats)");
        let s = if !bare.is_empty() { bare } else { dur };
        if s.is_empty() {
            return Ok((self.ctx.len, false));
        }
        let (body, dots) = {
            let b = s.trim_end_matches('.');
            (b, s.len() - b.len())
        };
        let mut d = if let Some(b) = body.strip_prefix(':') {
            b.parse::<f64>().map_err(|_| bad())?
        } else {
            let b = body.strip_prefix('/').unwrap_or(body);
            let (b, trip) = match b.strip_suffix('t') {
                Some(x) => (x, true),
                None => (b, false),
            };
            let n: f64 = b.parse().map_err(|_| bad())?;
            if n <= 0.0 {
                return Err(bad());
            }
            4.0 / n * if trip { 2.0 / 3.0 } else { 1.0 }
        };
        let mut add = d / 2.0;
        for _ in 0..dots {
            d += add;
            add /= 2.0;
        }
        Ok((d, true))
    }

    fn pitches(&self, pos: usize, tok: &str, head: &str) -> Result<Vec<i32>, String> {
        let oct = self.ctx.octave;
        if let Some(inner) = head.strip_prefix('[') {
            let inner = inner.strip_suffix(']').ok_or_else(|| self.err(pos, tok, "unclosed '['"))?;
            let mut out = Vec::new();
            for t in inner.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()) {
                out.extend(self.pitches(pos, tok, t)?);
            }
            return Ok(out);
        }
        // A letter with an octave is a note (`C7` = C in octave 7); a bare letter
        // (`C`, `F#`, `Bb`) is a note in the current octave (`o7 C`). Chords need a
        // quality: `Cmaj` `Am7` `Cdom7` `G/B`, or numerals `V7`, or `[C E G]`.
        if let Some(k) = note_key(head) {
            return Ok(vec![k]);
        }
        if crate::theory::pitch_class(head).is_some() {
            let letter = crate::theory::pitch_class(&head[..1]).unwrap_or(0);
            let acc: i32 = head[1..].chars().map(|c| if c == '#' || c == '♯' { 1 } else { -1 }).sum();
            return Ok(vec![(oct + 1) * 12 + letter + acc]);
        }
        let dom = head.replacen("dom", "", 1);
        if head.contains("dom") {
            if let Some(c) = Chord::parse_symbol(&dom) {
                let mut ks = c.voice(4, (oct + 1) * 12 + 4);
                ks.sort();
                return Ok(ks);
            }
        }
        if let Some(k) = self.degree(head) {
            return Ok(vec![k]);
        }
        match Chord::parse(head, &self.ctx.key) {
            Ok(c) => {
                let center = (oct + 1) * 12 + 4;
                let mut ks = c.voice(4, center);
                if let Some(b) = c.bass {
                    let lo = ks.first().copied().unwrap_or(center);
                    let mut bk = lo - (lo - b).rem_euclid(12);
                    if bk == lo {
                        bk -= 12;
                    }
                    ks.insert(0, bk);
                }
                Ok(ks)
            }
            Err(_) => {
                let fix = suggest(head);
                Err(self.err(
                    pos,
                    tok,
                    &format!("not a note, chord or directive{fix}. Notes: C5 (octave 5), C (current octave, set with o5), Eb3, F#4; chords: Cmaj Am7 Cdom7 G/B [C E G] or numerals vi IV V7; rest r; hold _; durations C5/8 C5/4. C5:1.5"),
                ))
            }
        }
    }

    fn degree(&self, s: &str) -> Option<i32> {
        let mut acc = 0;
        let mut t = s;
        loop {
            if let Some(r) = t.strip_prefix('b') {
                acc -= 1;
                t = r;
            } else if let Some(r) = t.strip_prefix('#') {
                acc += 1;
                t = r;
            } else {
                break;
            }
        }
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        let d: i32 = t[..digits].parse().ok()?;
        if d == 0 {
            return None;
        }
        let mut o = 0;
        for c in t[digits..].chars() {
            match c {
                '\'' => o += 1,
                ',' => o -= 1,
                _ => return None,
            }
        }
        Some(self.ctx.key.degree(d - 1, self.ctx.octave) + acc + 12 * o)
    }
}

/// A targeted hint for a token that failed to parse.
fn suggest(head: &str) -> String {
    let first = head.chars().next().unwrap_or(' ');
    if matches!(first, 'H' | 'h') {
        return " — use B (German H)".into();
    }
    if first.is_ascii_lowercase() && matches!(first, 'a'..='g') {
        let up: String = first.to_ascii_uppercase().to_string() + &head[1..];
        if note_key(&up).is_some() || crate::theory::pitch_class(&up).is_some() {
            return format!(" — note names are uppercase: {up}");
        }
    }
    if let Some(rest) = head.get(1..) {
        if rest.parse::<i32>().is_ok_and(|o| !(-1..=9).contains(&o)) {
            return format!(" — octave {rest} is out of range (−1…9)");
        }
    }
    if head.ends_with("maj") || head.ends_with('m') || head.contains("7") {
        return " — check the chord root (A–G, then # or b)".into();
    }
    String::new()
}

/// `C4/8.!` → (`C4`, `/8.`, `!`).
fn split_token(tok: &str) -> (&str, &str, &str) {
    // Marks are trailing !?^~ characters.
    let body = tok.trim_end_matches(['!', '?', '^', '~']);
    let marks = &tok[body.len()..];
    if body.starts_with('[') {
        let close = body.rfind(']').map(|i| i + 1).unwrap_or(body.len());
        return (&body[..close], &body[close..], marks);
    }
    // Duration starts at '/' or ':'.
    match body.find(['/', ':']) {
        // A slash chord (G/B) is not a duration: the char after '/' is a letter.
        Some(i) if body[i + 1..].starts_with(|c: char| c.is_ascii_uppercase()) => {
            match body[i + 1..].find(['/', ':']) {
                Some(j) => (&body[..i + 1 + j], &body[i + 1 + j..], marks),
                None => (body, "", marks),
            }
        }
        Some(i) => (&body[..i], &body[i..], marks),
        None => (body, "", marks),
    }
}

/// GM drum key for a drum voice name.
pub fn drum_key(name: &str) -> Option<u8> {
    Some(match name.to_ascii_lowercase().as_str() {
        "kick" | "bd" | "k" => 36,
        "kick2" | "bd2" => 35,
        "snare" | "sd" | "s" => 38,
        "snare2" | "sd2" => 40,
        "rim" | "rimshot" | "stick" => 37,
        "clap" | "cp" => 39,
        "hat" | "hh" | "chh" | "h" | "closedhat" => 42,
        "phh" | "pedal" | "pedalhat" => 44,
        "ohh" | "open" | "openhat" | "oh" => 46,
        "tom" | "tom1" | "htom" | "hightom" => 50,
        "tom2" | "mtom" | "midtom" => 47,
        "tom3" | "ltom" | "lowtom" => 45,
        "ftom" | "floortom" => 41,
        "crash" | "cy" => 49,
        "crash2" => 57,
        "ride" | "rd" => 51,
        "bell" | "ridebell" => 53,
        "china" => 52,
        "splash" => 55,
        "tamb" | "tambourine" => 54,
        "cowbell" | "cb" => 56,
        "shaker" | "sh" => 82,
        "maracas" => 70,
        "conga" => 63,
        "congalo" => 64,
        "bongo" => 60,
        "claves" => 75,
        "block" | "woodblock" => 76,
        "triangle" => 81,
        "timbale" => 65,
        "agogo" => 67,
        "cabasa" => 69,
        "guiro" => 73,
        "snap" => 39,
        _ => return name.parse::<u8>().ok().filter(|k| *k < 128),
    })
}

/// Names accepted by [`drum_key`] (for error messages).
pub const DRUM_NAMES: &[&str] = &[
    "kick", "kick2", "snare", "snare2", "rim", "clap", "hat", "phh", "ohh", "tom", "tom2", "tom3", "ftom", "crash", "crash2",
    "ride", "bell", "china", "splash", "tamb", "cowbell", "shaker", "maracas", "conga", "bongo", "claves", "block", "triangle",
];

/// Parse a drum grid: one character per step (`steps` steps per bar, default 16).
/// `x` hit, `X` accent, `o`/`g` ghost, `.`/`-` rest, `2`/`3`/`4` a roll of that many
/// hits inside the step; spaces and `|` are ignored. The pattern repeats to fill `bars`.
pub fn parse_grid(pattern: &str, key: u8, steps: usize, beats_per_bar: f64, bars: f64, vel: f32) -> Result<Vec<Note>, String> {
    let cells: Vec<char> = pattern.chars().filter(|c| !c.is_whitespace() && *c != '|').collect();
    if cells.is_empty() {
        return Ok(Vec::new());
    }
    let step = beats_per_bar / steps as f64;
    let total_steps = (bars * steps as f64).round() as usize;
    let mut out = Vec::new();
    for s in 0..total_steps {
        let c = cells[s % cells.len()];
        let t = s as f64 * step;
        let (n, v) = match c {
            'x' => (1, vel),
            'X' => (1, (vel * 1.25 + 0.1).min(1.0)),
            'o' | 'g' => (1, vel * 0.45),
            '2' => (2, vel * 0.85),
            '3' => (3, vel * 0.8),
            '4' => (4, vel * 0.75),
            '6' => (6, vel * 0.7),
            '.' | '-' | '_' => continue,
            other => {
                return Err(format!(
                    "drum pattern '{pattern}': unknown step '{other}' (use x hit, X accent, o ghost, . rest, 2/3/4 roll)"
                ))
            }
        };
        for k in 0..n {
            // Rolls ramp up slightly in velocity, like a real hat roll.
            let ramp = if n > 1 { 0.85 + 0.15 * k as f32 / (n - 1) as f32 } else { 1.0 };
            out.push(Note { t: t + step * k as f64 / n as f64, len: step / n as f64 * 0.9, key: key as f32, vel: v * ramp });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(s: &str) -> Vec<(f64, f64, i32)> {
        let ctx = Ctx { len: 1.0, ..Default::default() };
        parse(s, &ctx).unwrap().0.iter().map(|n| (n.t, n.len, n.key as i32)).collect()
    }

    #[test]
    fn basic_notes() {
        let n = keys("C4 D4/8 E4/8. r/16 G4:2");
        assert_eq!(n[0], (0.0, 0.92, 60));
        assert_eq!(n[1].0, 1.0);
        assert!((n[1].1 - 0.46).abs() < 1e-9);
        assert_eq!(n[2].0, 1.5);
        assert_eq!(n[3].0, 1.5 + 0.75 + 0.25);
        assert_eq!(n[3].2, 67);
    }

    #[test]
    fn degrees_chords_and_bars() {
        let ctx = Ctx { key: Key::parse("Am").unwrap(), octave: 4, ..Default::default() };
        let (n, end) = parse("1 3 5' | Am/2 Fmaj/2 | [1 3 5]/1", &ctx).unwrap();
        assert_eq!(n[0].key as i32, 69);
        assert_eq!(n[1].key as i32, 72);
        assert_eq!(n[2].key as i32, 76 + 12);
        assert_eq!(n[3].t, 4.0); // bar line padded
        let am: Vec<i32> = n.iter().filter(|x| x.t == 4.0).map(|x| x.key as i32 % 12).collect();
        assert!(am.contains(&9) && am.contains(&0) && am.contains(&4));
        assert_eq!(end, 12.0);
        let (n, _) = parse("(1 2)x3 vi IV", &ctx).unwrap();
        assert_eq!(n.iter().filter(|x| x.t < 6.0).count(), 6);
        let (n, _) = parse("G/B/2 o3 C4 > C4 t+12 C4", &Ctx::default()).unwrap();
        assert_eq!(n[0].key as i32 % 12, 11); // slash bass first
        assert_eq!(n[n.len() - 2].key, 60.0);
        assert_eq!(n[n.len() - 1].key, 72.0);
    }

    #[test]
    fn ties_marks_and_errors() {
        let n = keys("C4~ _ D4! E4?");
        assert_eq!(n[0].1, 2.0);
        let ctx = Ctx::default();
        let (v, _) = parse("C4 D4! E4?", &ctx).unwrap();
        assert!(v[1].vel > v[0].vel && v[2].vel < v[0].vel);
        let e = parse("C4 Q9", &ctx).unwrap_err();
        assert!(e.contains("'Q9'") && e.contains("column 4"), "{e}");
        assert!(parse("(C4 D4", &ctx).is_err());
        assert!(parse("C4/0", &ctx).is_err());
    }

    #[test]
    fn forms_agents_try() {
        let ctx = Ctx::default();
        let k = |s: &str| parse(s, &ctx).unwrap().0.iter().map(|n| n.key as i32).collect::<Vec<_>>();
        assert_eq!(k("o7 C"), vec![96]);
        assert_eq!(k("[C7]"), vec![96]);
        assert_eq!(k("C7"), vec![96]);
        assert_eq!(k("[C E G]"), vec![60, 64, 67]);
        assert_eq!(k("Cmaj").len(), 3);
        assert_eq!(k("Cdom7").len(), 4);
        assert!(k("Cmaj7").len() == 4);
        let e = parse("c5", &ctx).err().unwrap_or_default();
        assert!(e.is_empty() || e.contains("uppercase"), "{e}");
        let e = parse("H4", &ctx).unwrap_err();
        assert!(e.contains("use B") && e.contains("Cmaj"), "{e}");
    }

    #[test]
    fn grids() {
        let n = parse_grid("x...x...x...x...", 36, 16, 4.0, 2.0, 0.8).unwrap();
        assert_eq!(n.len(), 8);
        assert_eq!(n[1].t, 1.0);
        let n = parse_grid("x.3.", 42, 16, 4.0, 0.25, 0.8).unwrap();
        assert_eq!(n.len(), 4);
        assert!((n[2].t - (0.5 + 0.25 / 3.0)).abs() < 1e-9);
        assert!(parse_grid("x.y.", 36, 16, 4.0, 1.0, 0.8).is_err());
        assert_eq!(drum_key("ohh"), Some(46));
    }
}
