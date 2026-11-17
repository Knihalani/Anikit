//! Music theory: pitch names, keys, scales, chord symbols, Roman numerals,
//! voicings and voice leading.

/// Pitch class of a note letter with accidentals (`"C#"`, `"Eb"`, `"bb"`… case-insensitive).
pub fn pitch_class(name: &str) -> Option<i32> {
    let mut ch = name.chars();
    let base = match ch.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut pc: i32 = base;
    for c in ch {
        match c {
            '#' | '♯' => pc += 1,
            'b' | '♭' => pc -= 1,
            _ => return None,
        }
    }
    Some(pc.rem_euclid(12))
}

/// Parse a note name with octave (`"C4"` = 60, `"A#3"`, `"Eb5"`, `"C-1"` = 0) into a MIDI key.
pub fn note_key(s: &str) -> Option<i32> {
    let s = s.trim();
    let split = s.char_indices().find(|&(i, c)| i > 0 && (c.is_ascii_digit() || c == '-')).map(|(i, _)| i)?;
    let (name, oct) = s.split_at(split);
    let pc = pitch_class(name)?;
    // Accidentals can push B#/Cb across octave boundaries: compute from the letter.
    let letter = pitch_class(&name[..1])?;
    let acc = {
        let mut a = 0;
        for c in name[1..].chars() {
            a += if c == '#' || c == '♯' { 1 } else { -1 };
        }
        a
    };
    let _ = pc;
    let oct: i32 = oct.parse().ok()?;
    let k = (oct + 1) * 12 + letter + acc;
    (0..=127).contains(&k).then_some(k)
}

const SHARP_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLAT_NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

/// `60` → `"C4"`.
pub fn key_name(key: i32, flats: bool) -> String {
    let names = if flats { &FLAT_NAMES } else { &SHARP_NAMES };
    format!("{}{}", names[key.rem_euclid(12) as usize], key.div_euclid(12) - 1)
}

/// A scale / mode as semitone steps from the tonic.
#[derive(Debug, Clone, PartialEq)]
pub struct Scale {
    pub name: &'static str,
    pub steps: &'static [i32],
}

pub const SCALES: &[Scale] = &[
    Scale { name: "major", steps: &[0, 2, 4, 5, 7, 9, 11] },
    Scale { name: "minor", steps: &[0, 2, 3, 5, 7, 8, 10] },
    Scale { name: "dorian", steps: &[0, 2, 3, 5, 7, 9, 10] },
    Scale { name: "phrygian", steps: &[0, 1, 3, 5, 7, 8, 10] },
    Scale { name: "lydian", steps: &[0, 2, 4, 6, 7, 9, 11] },
    Scale { name: "mixolydian", steps: &[0, 2, 4, 5, 7, 9, 10] },
    Scale { name: "locrian", steps: &[0, 1, 3, 5, 6, 8, 10] },
    Scale { name: "harmonic-minor", steps: &[0, 2, 3, 5, 7, 8, 11] },
    Scale { name: "melodic-minor", steps: &[0, 2, 3, 5, 7, 9, 11] },
    Scale { name: "pentatonic", steps: &[0, 2, 4, 7, 9] },
    Scale { name: "minor-pentatonic", steps: &[0, 3, 5, 7, 10] },
    Scale { name: "blues", steps: &[0, 3, 5, 6, 7, 10] },
    Scale { name: "hirajoshi", steps: &[0, 2, 3, 7, 8] },
    Scale { name: "whole-tone", steps: &[0, 2, 4, 6, 8, 10] },
    Scale { name: "chromatic", steps: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] },
];

/// A key: tonic pitch class + scale.
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub tonic: i32,
    pub scale: &'static Scale,
}

impl Default for Key {
    fn default() -> Self {
        Key { tonic: 0, scale: &SCALES[0] }
    }
}

impl Key {
    /// `"Am"`, `"C"`, `"F#m"`, `"Bb major"`, `"D dorian"`, `"E minor-pentatonic"`, `"c minor"`.
    pub fn parse(s: &str) -> Result<Key, String> {
        let s = s.trim();
        let err = || format!("key '{s}': use e.g. \"C\", \"Am\", \"F#m\", \"Bb major\", \"D dorian\"");
        let (name, rest) = match s.find(|c: char| c.is_whitespace()) {
            Some(i) => (&s[..i], s[i..].trim()),
            None => (s, ""),
        };
        let (root, minor_suffix) = if let Some(r) = name.strip_suffix("min") {
            (r, true)
        } else if let Some(r) = name.strip_suffix('m') {
            (r, true)
        } else {
            (name, false)
        };
        let tonic = pitch_class(root).ok_or_else(err)?;
        let scale_name = if !rest.is_empty() {
            rest.to_ascii_lowercase()
        } else if minor_suffix {
            "minor".into()
        } else {
            "major".into()
        };
        let scale_name = match scale_name.as_str() {
            "maj" | "ionian" => "major".to_string(),
            "min" | "aeolian" | "natural-minor" => "minor".to_string(),
            o => o.replace(' ', "-"),
        };
        let scale = SCALES.iter().find(|x| x.name == scale_name).ok_or_else(|| {
            format!("key '{s}': unknown scale '{scale_name}' (known: {})", SCALES.iter().map(|x| x.name).collect::<Vec<_>>().join(", "))
        })?;
        Ok(Key { tonic, scale })
    }

    pub fn is_minor(&self) -> bool {
        self.scale.steps.len() >= 3 && self.scale.steps[2] == 3
    }

    /// Prefer flat spellings (F, Bb, Eb… majors; D, G, C… minors).
    pub fn flats(&self) -> bool {
        let major_tonic = if self.is_minor() { (self.tonic + 3) % 12 } else { self.tonic };
        matches!(major_tonic, 5 | 10 | 3 | 8 | 1 | 6)
    }

    /// MIDI key of scale degree `deg` (0-based, may be negative or ≥ len, wraps octaves)
    /// counted from the tonic in octave `oct` (tonic of octave 4 is near middle C).
    pub fn degree(&self, deg: i32, oct: i32) -> i32 {
        let n = self.scale.steps.len() as i32;
        let o = deg.div_euclid(n);
        let i = deg.rem_euclid(n) as usize;
        (oct + 1) * 12 + self.tonic + self.scale.steps[i] + 12 * o
    }

    /// Snap a MIDI key to the nearest scale tone (ties go down).
    pub fn snap(&self, key: i32) -> i32 {
        for d in 0..7 {
            for k in [key - d, key + d] {
                let pc = (k - self.tonic).rem_euclid(12);
                if self.scale.steps.contains(&pc) {
                    return k;
                }
            }
        }
        key
    }

    pub fn contains(&self, key: i32) -> bool {
        self.scale.steps.contains(&(key - self.tonic).rem_euclid(12))
    }
}

/// A chord: root pitch class, intervals above the root, optional slash bass.
#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    pub root: i32,
    pub intervals: Vec<i32>,
    pub bass: Option<i32>,
}

const QUALITIES: &[(&str, &[i32])] = &[
    ("maj13", &[0, 4, 7, 11, 14, 21]),
    ("maj9", &[0, 4, 7, 11, 14]),
    ("maj7", &[0, 4, 7, 11]),
    ("M7", &[0, 4, 7, 11]),
    ("Δ7", &[0, 4, 7, 11]),
    ("Δ", &[0, 4, 7, 11]),
    ("mmaj7", &[0, 3, 7, 11]),
    ("m7b5", &[0, 3, 6, 10]),
    ("ø7", &[0, 3, 6, 10]),
    ("ø", &[0, 3, 6, 10]),
    ("m11", &[0, 3, 7, 10, 14, 17]),
    ("m9", &[0, 3, 7, 10, 14]),
    ("m7", &[0, 3, 7, 10]),
    ("m6", &[0, 3, 7, 9]),
    ("madd9", &[0, 3, 7, 14]),
    ("min", &[0, 3, 7]),
    ("m", &[0, 3, 7]),
    ("-7", &[0, 3, 7, 10]),
    ("-", &[0, 3, 7]),
    ("dim7", &[0, 3, 6, 9]),
    ("°7", &[0, 3, 6, 9]),
    ("dim", &[0, 3, 6]),
    ("°", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("+", &[0, 4, 8]),
    ("7sus4", &[0, 5, 7, 10]),
    ("sus4", &[0, 5, 7]),
    ("sus2", &[0, 2, 7]),
    ("sus", &[0, 5, 7]),
    ("add9", &[0, 4, 7, 14]),
    ("13", &[0, 4, 7, 10, 14, 21]),
    ("11", &[0, 7, 10, 14, 17]),
    ("9", &[0, 4, 7, 10, 14]),
    ("7", &[0, 4, 7, 10]),
    ("6/9", &[0, 4, 7, 9, 14]),
    ("69", &[0, 4, 7, 9, 14]),
    ("6", &[0, 4, 7, 9]),
    ("5", &[0, 7]),
    ("maj", &[0, 4, 7]),
    ("M", &[0, 4, 7]),
    ("", &[0, 4, 7]),
];

impl Chord {
    /// Parse a chord symbol (`"Am7"`, `"F#m7b5"`, `"Cmaj9"`, `"G/B"`, `"Bbsus2"`, `"E7"`)
    /// or, given a key, a Roman numeral (`"vi"`, `"IV"`, `"V7"`, `"ii7"`, `"bVII"`, `"V/V"` not supported).
    pub fn parse(s: &str, key: &Key) -> Result<Chord, String> {
        let s = s.trim();
        if let Some(c) = Self::parse_roman(s, key) {
            return Ok(c);
        }
        Self::parse_symbol(s).ok_or_else(|| format!("chord '{s}': use a symbol like Am, C, F#m7, Gsus4, Cmaj7, G/B or a numeral like vi, IV, V7"))
    }

    pub fn parse_symbol(s: &str) -> Option<Chord> {
        let (main, bass) = match s.rsplit_once('/') {
            Some((a, b)) if pitch_class(b).is_some() && !a.is_empty() && b != "9" => (a, Some(pitch_class(b)?)),
            _ => (s, None),
        };
        let mut chars = main.char_indices();
        let (_, first) = chars.next()?;
        if !matches!(first.to_ascii_uppercase(), 'A'..='G') || first.is_ascii_lowercase() {
            return None;
        }
        let _ = chars;
        let mut end = 1;
        if let Some(c) = main[1..].chars().next() {
            if matches!(c, '#' | 'b' | '♯' | '♭') {
                end += c.len_utf8();
            }
        }
        let root = pitch_class(&main[..end])?;
        let qual = &main[end..];
        let ints = QUALITIES.iter().find(|(q, _)| *q == qual).map(|(_, i)| i.to_vec())?;
        Some(Chord { root, intervals: ints, bass })
    }

    fn parse_roman(s: &str, key: &Key) -> Option<Chord> {
        let mut acc = 0;
        let mut t = s;
        while let Some(r) = t.strip_prefix('b').or_else(|| t.strip_prefix('♭')) {
            acc -= 1;
            t = r;
        }
        while let Some(r) = t.strip_prefix('#') {
            acc += 1;
            t = r;
        }
        const NUM: [&str; 7] = ["vii", "vi", "v", "iv", "iii", "ii", "i"];
        const DEG: [i32; 7] = [6, 5, 4, 3, 2, 1, 0];
        let lower = t.to_ascii_lowercase();
        let (deg, len) = NUM.iter().zip(DEG).find(|(n, _)| lower.starts_with(**n)).map(|(n, d)| (d, n.len()))?;
        let numeral = &t[..len];
        let upper = numeral.chars().all(|c| c.is_ascii_uppercase());
        let lowercase = numeral.chars().all(|c| c.is_ascii_lowercase());
        if !upper && !lowercase {
            return None;
        }
        let rest = &t[len..];
        // Root from the major scale degrees for chromatic numerals (bVII), else the key's scale.
        let major = [0, 2, 4, 5, 7, 9, 11];
        let root = if acc != 0 {
            (key.tonic + major[deg as usize] + acc).rem_euclid(12)
        } else {
            (key.degree(deg, 4)).rem_euclid(12)
        };
        let qual = match (upper, rest) {
            (_, "°" | "dim" | "o") => "dim",
            (_, "°7" | "dim7" | "o7") => "dim7",
            (_, "ø" | "ø7" | "m7b5") => "m7b5",
            (true, "") => "",
            (true, "7") => "7",
            (true, "maj7" | "M7") => "maj7",
            (true, "9") => "9",
            (true, "sus4" | "sus") => "sus4",
            (true, "sus2") => "sus2",
            (true, "add9") => "add9",
            (true, "6") => "6",
            (false, "") => "m",
            (false, "7") => "m7",
            (false, "9") => "m9",
            (false, "6") => "m6",
            _ => return None,
        };
        let ints = QUALITIES.iter().find(|(q, _)| *q == qual)?.1.to_vec();
        Some(Chord { root, intervals: ints, bass: None })
    }

    pub fn is_minor(&self) -> bool {
        self.intervals.get(1) == Some(&3)
    }

    /// Pitch classes of the chord tones.
    pub fn pcs(&self) -> Vec<i32> {
        self.intervals.iter().map(|i| (self.root + i).rem_euclid(12)).collect()
    }

    /// Bass pitch class (slash bass or root).
    pub fn bass_pc(&self) -> i32 {
        self.bass.unwrap_or(self.root)
    }

    /// Root-position keys from `root_key` upward (root placed in octave `oct`).
    pub fn keys(&self, oct: i32) -> Vec<i32> {
        let r = (oct + 1) * 12 + self.root;
        self.intervals.iter().map(|i| r + i).collect()
    }

    /// A close voicing of up to `n` notes centred near `center`, keeping the
    /// 3rd and 7th (guide tones) and dropping the 5th first when thinning.
    pub fn voice(&self, n: usize, center: i32) -> Vec<i32> {
        let mut pcs: Vec<i32> = self.intervals.iter().map(|&i| i % 24).collect();
        // Thin: drop the fifth, then extensions beyond the 9th.
        while pcs.len() > n {
            if let Some(p) = pcs.iter().position(|&i| i == 7) {
                pcs.remove(p);
            } else {
                pcs.pop();
            }
        }
        let mut keys: Vec<i32> = pcs
            .iter()
            .map(|&i| {
                let pc = (self.root + i).rem_euclid(12);
                // Nearest key with this pitch class to center.
                let mut k = center - (center - pc).rem_euclid(12);
                if center - k > 6 {
                    k += 12;
                }
                k
            })
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    /// Voice-lead from `prev`: choose an inversion of `n` notes minimising total motion.
    pub fn voice_lead(&self, prev: &[i32], n: usize) -> Vec<i32> {
        if prev.is_empty() {
            return self.voice(n, 64);
        }
        let center = prev.iter().sum::<i32>() / prev.len() as i32;
        let mut best = self.voice(n, center);
        let mut best_cost = i32::MAX;
        for shift in -6..=6 {
            let cand = self.voice(n, center + shift);
            let cost: i32 = cand
                .iter()
                .map(|k| prev.iter().map(|p| (k - p).abs()).min().unwrap_or(0))
                .sum::<i32>()
                + (cand.iter().sum::<i32>() / cand.len().max(1) as i32 - center).abs();
            if cost < best_cost {
                best_cost = cost;
                best = cand;
            }
        }
        best
    }
}

/// Parse a progression: chord symbols / numerals separated by spaces, `|` or `-`.
pub fn progression(s: &str, key: &Key) -> Result<Vec<Chord>, String> {
    s.split(|c: char| c.is_whitespace() || c == '|' || c == ',')
        .filter(|t| !t.is_empty())
        .map(|t| Chord::parse(t, key))
        .collect()
}

/// Seconds/beats parsing helper: `"15s"`, `"15"`, `"8 bars"`, `"8bars"`, `"32 beats"`, `"1:30"`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Seconds(f64),
    Bars(f64),
    Beats(f64),
}

impl Length {
    pub fn parse(s: &str) -> Result<Length, String> {
        let t = s.trim().to_ascii_lowercase();
        let num = |x: &str| x.trim().parse::<f64>().map_err(|_| format!("length '{s}': use \"15s\", \"8 bars\", \"32 beats\" or \"1:30\""));
        if let Some((m, sec)) = t.split_once(':') {
            return Ok(Length::Seconds(num(m)? * 60.0 + num(sec)?));
        }
        for (suf, kind) in [("bars", 1), ("bar", 1), ("beats", 2), ("beat", 2), ("sec", 0), ("s", 0)] {
            if let Some(n) = t.strip_suffix(suf) {
                let v = num(n)?;
                return Ok(match kind {
                    1 => Length::Bars(v),
                    2 => Length::Beats(v),
                    _ => Length::Seconds(v),
                });
            }
        }
        Ok(Length::Seconds(num(&t)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_keys() {
        assert_eq!(note_key("C4"), Some(60));
        assert_eq!(note_key("A4"), Some(69));
        assert_eq!(note_key("Bb3"), Some(58));
        assert_eq!(note_key("C#-1"), Some(1));
        assert_eq!(note_key("B#3"), Some(60));
        assert_eq!(key_name(61, true), "Db4");
        let am = Key::parse("Am").unwrap();
        assert!(am.is_minor());
        assert_eq!(am.degree(0, 4), 69 - 12 + 12); // A4? tonic in octave 4 = A4 = 69
        assert_eq!(am.degree(2, 3), 60); // C4
        assert_eq!(Key::parse("F# dorian").unwrap().tonic, 6);
        assert!(Key::parse("H").is_err());
        assert_eq!(am.snap(61), 60);
    }

    #[test]
    fn chords() {
        let k = Key::parse("C").unwrap();
        let c = Chord::parse("Am7", &k).unwrap();
        assert_eq!((c.root, c.intervals.clone()), (9, vec![0, 3, 7, 10]));
        let c = Chord::parse("F#m7b5", &k).unwrap();
        assert_eq!((c.root, c.intervals.clone()), (6, vec![0, 3, 6, 10]));
        let c = Chord::parse("G/B", &k).unwrap();
        assert_eq!((c.root, c.bass), (7, Some(11)));
        let c = Chord::parse("Bbmaj7", &k).unwrap();
        assert_eq!(c.root, 10);
        let p = progression("vi IV I V7", &k).unwrap();
        assert_eq!(p.iter().map(|c| c.root).collect::<Vec<_>>(), vec![9, 5, 0, 7]);
        assert!(p[0].is_minor() && !p[1].is_minor());
        assert_eq!(p[3].intervals, vec![0, 4, 7, 10]);
        let am = Key::parse("Am").unwrap();
        let p = progression("i VI III VII", &am).unwrap();
        assert_eq!(p.iter().map(|c| c.root).collect::<Vec<_>>(), vec![9, 5, 0, 7]);
        assert_eq!(Chord::parse("bVII", &k).unwrap().root, 10);
        assert!(Chord::parse("Xm", &k).is_err());
        let v = Chord::parse("C", &k).unwrap().voice_lead(&[64, 67, 72], 3);
        assert_eq!(v, vec![64, 67, 72]);
        let v = Chord::parse("F", &k).unwrap().voice_lead(&[64, 67, 72], 3);
        assert_eq!(v, vec![65, 69, 72]);
    }

    #[test]
    fn lengths() {
        assert_eq!(Length::parse("15s").unwrap(), Length::Seconds(15.0));
        assert_eq!(Length::parse("8 bars").unwrap(), Length::Bars(8.0));
        assert_eq!(Length::parse("1:30").unwrap(), Length::Seconds(90.0));
        assert_eq!(Length::parse("12").unwrap(), Length::Seconds(12.0));
    }
}
