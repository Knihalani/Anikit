//! SFZ instruments (a practical subset of SFZ v1/v2 + common ARIA opcodes).
//!
//! Supported:
//! - headers `<control>`, `<global>`, `<master>`, `<group>`, `<region>` with
//!   inheritance (`<curve>`, `<effect>`, `<midi>` are skipped);
//! - preprocessor `#define $NAME value` (sequential, redefinable) and
//!   `#include "file"` (also mid-line);
//! - `default_path`, `note_offset`, `octave_offset`, `set_ccN`, `set_hdccN`;
//! - `sample` (WAV/FLAC/Ogg; `*sine`, `*silence`), `lokey/hikey/key`,
//!   `lovel/hivel`, `pitch_keycenter`, `pitch_keytrack`, `tune`, `transpose`,
//!   `volume`, `amplitude`, `amp_veltrack`, `pan`, `offset`, `end`,
//!   `loop_mode` (`no_loop`, `one_shot`, `loop_continuous`, `loop_sustain`),
//!   `loop_start/loop_end` (else the WAV `smpl` loop), `ampeg_delay/attack/
//!   hold/decay/sustain/release`, `fil_type` (`lpf_*`, `hpf_*`, `bpf_*`,
//!   `brf_*`), `cutoff`, `resonance`, `fil_veltrack`, `fil_keytrack`,
//!   `fil_keycenter`, `seq_length/seq_position`, `lorand/hirand`,
//!   `trigger=attack|release|first`, `rt_decay`, `group/off_by`,
//!   `sw_lokey/sw_hikey/sw_last/sw_default` (the default key switch is used;
//!   key-switch keys are silent), `loccN/hiccN` against the controller
//!   defaults, and `X_onccN`/`X_ccN` (with `X_curveccN` 0 = linear,
//!   2 = inverted) for `ampeg_*`, `amp_veltrack`, `offset`, `volume`, `pan`,
//!   `tune`, `cutoff`, evaluated at the controller defaults.
//!
//! Note names: `c4` = 60 (`c-1` = 0), sharps `#`, flats `b`.

use super::engine::{hz_to_cents, Env, FilterKind, Lfo, LoopMode, Osc, OscParams, SampleBuf, SamplerVoice};
use super::interp;
use super::sampler::{SampleSlot, Source};
use crate::voice::{Instrument, NoteOn, Rng, Voice};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// One parse event.
#[derive(Debug, Clone, PartialEq)]
enum Ev {
    Header(String),
    Op(String, String),
}

/// Preprocess and tokenize an SFZ file (and its includes).
struct Lexer {
    defines: Vec<(String, String)>,
    root: PathBuf,
    depth: usize,
    events: Vec<Ev>,
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

impl Lexer {
    fn expand(&self, s: &str) -> String {
        if !s.contains('$') {
            return s.to_string();
        }
        let mut out = String::with_capacity(s.len());
        let b: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < b.len() {
            if b[i] == '$' {
                let mut j = i + 1;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_') {
                    j += 1;
                }
                let name: String = b[i..j].iter().collect();
                // longest defined prefix wins ($OFF01 vs $OFF0)
                let mut hit = None;
                for k in (i + 2..=j).rev() {
                    let n: String = b[i..k].iter().collect();
                    if let Some((_, v)) = self.defines.iter().rev().find(|(d, _)| *d == n) {
                        hit = Some((k, v.clone()));
                        break;
                    }
                }
                match hit {
                    Some((k, v)) => {
                        out.push_str(&v);
                        i = k;
                    }
                    None => {
                        out.push_str(&name);
                        i = j;
                    }
                }
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        out
    }

    fn file(&mut self, path: &Path) -> Result<(), String> {
        if self.depth > 16 {
            return Err(format!("#include nested too deeply at {}", path.display()));
        }
        let text = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let text = String::from_utf8_lossy(&text).replace('\r', "");
        self.depth += 1;
        self.text(&text)?;
        self.depth -= 1;
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<(), String> {
        // strip block comments
        let mut t = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(a) = rest.find("/*") {
            t.push_str(&rest[..a]);
            match rest[a..].find("*/") {
                Some(b) => rest = &rest[a + b + 2..],
                None => rest = "",
            }
        }
        t.push_str(rest);
        for raw in t.lines() {
            let line = match raw.find("//") {
                Some(i) => &raw[..i],
                None => raw,
            };
            self.line(line)?;
        }
        Ok(())
    }

    fn line(&mut self, line: &str) -> Result<(), String> {
        let c: Vec<char> = line.chars().collect();
        let n = c.len();
        let mut i = 0;
        let starts_token = |c: &[char], j: usize| -> bool {
            // `<`, `#`, or an identifier followed by '='
            if c[j] == '<' || c[j] == '#' {
                return true;
            }
            let mut k = j;
            while k < c.len() && is_ident(c[k]) {
                k += 1;
            }
            k > j && k < c.len() && c[k] == '='
        };
        while i < n {
            if c[i].is_whitespace() {
                i += 1;
                continue;
            }
            if c[i] == '<' {
                let j = c[i..].iter().position(|&x| x == '>').map(|p| i + p).ok_or("unterminated <header>")?;
                let h: String = c[i + 1..j].iter().collect();
                self.events.push(Ev::Header(h.trim().to_lowercase()));
                i = j + 1;
                continue;
            }
            let rest: String = c[i..].iter().collect();
            if let Some(r) = rest.strip_prefix("#define") {
                let r = r.trim();
                let (name, val) = r.split_once(char::is_whitespace).unwrap_or((r, ""));
                let val = self.expand(val.trim());
                self.defines.push((name.to_string(), val));
                return Ok(());
            }
            if let Some(r) = rest.strip_prefix("#include") {
                let r = r.trim_start();
                let q = r.strip_prefix('"').ok_or("#include needs a quoted path")?;
                let e = q.find('"').ok_or("unterminated #include path")?;
                let rel = self.expand(&q[..e]).replace('\\', "/");
                let p = resolve(&self.root, &rel).unwrap_or_else(|| self.root.join(&rel));
                self.file(&p)?;
                let consumed = rest.len() - (r.len() - e - 2);
                i += rest[..consumed].chars().count();
                continue;
            }
            if c[i] == '#' {
                // unknown directive: skip the line
                return Ok(());
            }
            // opcode
            let eq = match c[i..].iter().position(|&x| x == '=') {
                Some(p) => i + p,
                None => return Ok(()), // stray text
            };
            let name: String = c[i..eq].iter().collect();
            let mut j = eq + 1;
            let mut end = n;
            while j < n {
                if c[j].is_whitespace() {
                    let mut k = j;
                    while k < n && c[k].is_whitespace() {
                        k += 1;
                    }
                    if k < n && starts_token(&c, k) {
                        end = j;
                        break;
                    }
                    j = k;
                } else {
                    j += 1;
                }
            }
            let val: String = c[eq + 1..end].iter().collect();
            let name = self.expand(name.trim());
            let val = self.expand(val.trim());
            self.events.push(Ev::Op(name.to_lowercase(), val));
            i = end;
        }
        Ok(())
    }
}

/// Resolve `rel` under `base`, matching each path component
/// case-insensitively when the exact name does not exist (SFZ files are
/// often written on case-insensitive file systems).
fn resolve(base: &Path, rel: &str) -> Option<PathBuf> {
    let direct = base.join(rel);
    if direct.exists() {
        return Some(direct);
    }
    let mut cur = base.to_path_buf();
    for comp in rel.split('/').filter(|s| !s.is_empty() && *s != ".") {
        if comp == ".." {
            cur.pop();
            continue;
        }
        let exact = cur.join(comp);
        if exact.exists() {
            cur = exact;
            continue;
        }
        let lc = comp.to_lowercase();
        let hit = std::fs::read_dir(&cur).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == lc)?;
        cur = hit.path();
    }
    Some(cur)
}

/// Parse a note number or name (`60`, `c4`, `C#4`, `db4`, `c-1`).
pub fn parse_key(v: &str) -> Option<i32> {
    let v = v.trim();
    if let Ok(n) = v.parse::<i32>() {
        return Some(n);
    }
    let lower = v.to_lowercase();
    let mut ch = lower.chars();
    let base = match ch.next()? {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest: String = ch.collect();
    let (acc, oct) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    let o: i32 = oct.parse().ok()?;
    Some((o + 1) * 12 + base + acc)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Trigger {
    Attack,
    Release,
    First,
    Legato,
}

/// A compiled region.
#[derive(Debug, Clone)]
struct Region {
    slot: usize,
    keys: (i32, i32),
    vels: (i32, i32),
    keycenter: f32,
    keytrack: f32,
    tune: f32,
    volume: f32,
    amplitude: f32,
    veltrack: f32,
    pan: f32,
    offset: u32,
    end: Option<u32>,
    loop_mode: Option<LoopMode>,
    loop_start: Option<u32>,
    loop_end: Option<u32>,
    env: Env,
    filter: FilterKind,
    cutoff: Option<f32>,
    resonance: f32,
    fil_veltrack: f32,
    fil_keytrack: f32,
    fil_keycenter: f32,
    seq_length: u32,
    seq_position: u32,
    rand: (f32, f32),
    trigger: Trigger,
    group: u32,
    off_by: u32,
    rt_decay: f32,
}

/// An SFZ instrument.
pub struct SfzInstrument {
    name: String,
    regions: Vec<Region>,
    slots: Vec<SampleSlot>,
    exclusive_of_group: HashMap<u32, u32>,
}

impl std::fmt::Debug for SfzInstrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SfzInstrument").field("name", &self.name).field("regions", &self.regions.len()).finish()
    }
}

fn num(m: &HashMap<String, String>, k: &str) -> Option<f32> {
    m.get(k).and_then(|v| v.trim().parse::<f32>().ok())
}

/// Built-in generated samples (`*sine`, `*silence`).
fn builtin(name: &str) -> Option<SampleBuf> {
    match name {
        "*sine" => {
            let n = 1024;
            let v: Vec<f32> = (0..n).map(|i| (2.0 * std::f64::consts::PI * i as f64 / n as f64).sin() as f32).collect();
            let rate = (n as f64 * 261.625_565) as u32; // one cycle = middle C
            let mut b = SampleBuf::new(vec![v], rate);
            b.file_loop = Some((0, n as u32));
            Some(b)
        }
        "*silence" => Some(SampleBuf::new(vec![vec![0.0; 64]], 48_000)),
        _ => None,
    }
}

/// Options for [`SfzInstrument::load_with`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SfzOptions {
    /// Drop regions whose sample file is missing instead of failing.
    pub skip_missing: bool,
}

impl SfzInstrument {
    /// Load an SFZ file; every referenced sample must exist (samples are
    /// decoded lazily, on first use).
    pub fn load(path: impl AsRef<Path>) -> Result<SfzInstrument, String> {
        Self::load_with(path, SfzOptions::default())
    }

    /// Load with options.
    pub fn load_with(path: impl AsRef<Path>, opt: SfzOptions) -> Result<SfzInstrument, String> {
        let path = path.as_ref();
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut lx = Lexer { defines: Vec::new(), root: root.clone(), depth: 0, events: Vec::new() };
        lx.file(path)?;
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        Self::build(name, &root, lx.events, opt).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Parse SFZ text whose samples live under `base`.
    pub fn from_str(text: &str, base: impl AsRef<Path>, opt: SfzOptions) -> Result<SfzInstrument, String> {
        let base = base.as_ref().to_path_buf();
        let mut lx = Lexer { defines: Vec::new(), root: base.clone(), depth: 0, events: Vec::new() };
        lx.text(text)?;
        Self::build("sfz".into(), &base, lx.events, opt)
    }

    /// Number of playable regions.
    pub fn regions(&self) -> usize {
        self.regions.len()
    }

    /// Decode every sample now (in parallel).
    pub fn preload(&self) -> Result<(), String> {
        use rayon::prelude::*;
        self.slots.par_iter().map(|s| s.get().map(|_| ())).collect()
    }

    fn build(name: String, root: &Path, events: Vec<Ev>, opt: SfzOptions) -> Result<SfzInstrument, String> {
        #[derive(PartialEq)]
        enum Level {
            None,
            Control,
            Global,
            Master,
            Group,
            Region,
            Other,
        }
        let mut level = Level::None;
        let mut control: HashMap<String, String> = HashMap::new();
        let mut global: HashMap<String, String> = HashMap::new();
        let mut master: HashMap<String, String> = HashMap::new();
        let mut group: HashMap<String, String> = HashMap::new();
        let mut region: Option<HashMap<String, String>> = None;
        // (opcodes, default_path, note offset) per region
        let mut raw: Vec<(HashMap<String, String>, String, i32)> = Vec::new();
        let mut cc = [0f32; 128];
        let mut default_path = String::new();
        let mut note_shift = 0i32;
        let mut octave_shift = 0i32;

        let mut flush = |region: &mut Option<HashMap<String, String>>, dp: &str, no: i32| {
            if let Some(r) = region.take() {
                raw.push((r, dp.to_string(), no));
            }
        };
        for ev in events {
            match ev {
                Ev::Header(h) => {
                    flush(&mut region, &default_path, note_shift + 12 * octave_shift);
                    level = match h.as_str() {
                        "control" => {
                            control.clear();
                            Level::Control
                        }
                        "global" => {
                            global.clear();
                            master.clear();
                            group.clear();
                            Level::Global
                        }
                        "master" => {
                            master.clear();
                            group.clear();
                            Level::Master
                        }
                        "group" => {
                            group.clear();
                            Level::Group
                        }
                        "region" => {
                            let mut r = global.clone();
                            r.extend(master.iter().map(|(k, v)| (k.clone(), v.clone())));
                            r.extend(group.iter().map(|(k, v)| (k.clone(), v.clone())));
                            region = Some(r);
                            Level::Region
                        }
                        _ => Level::Other,
                    };
                }
                Ev::Op(k, v) => match level {
                    Level::Control => {
                        match k.as_str() {
                            "default_path" => default_path = v.replace('\\', "/"),
                            "note_offset" => note_shift = v.trim().parse::<i32>().unwrap_or(0),
                            "octave_offset" => octave_shift = v.trim().parse::<i32>().unwrap_or(0),
                            _ => {
                                if let Some(n) = k.strip_prefix("set_hdcc").and_then(|n| n.parse::<usize>().ok()) {
                                    if n < 128 {
                                        cc[n] = v.trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 1.0);
                                    }
                                } else if let Some(n) = k.strip_prefix("set_cc").and_then(|n| n.parse::<usize>().ok()) {
                                    if n < 128 {
                                        cc[n] = v.trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 127.0) / 127.0;
                                    }
                                }
                            }
                        }
                        control.insert(k, v);
                    }
                    Level::Global => {
                        global.insert(k, v);
                    }
                    Level::Master => {
                        master.insert(k, v);
                    }
                    Level::Group => {
                        group.insert(k, v);
                    }
                    Level::Region => {
                        if let Some(r) = region.as_mut() {
                            r.insert(k, v);
                        }
                    }
                    Level::None => {
                        // opcodes before any header behave like <global>
                        global.insert(k, v);
                    }
                    Level::Other => {}
                },
            }
        }
        flush(&mut region, &default_path, note_shift + 12 * octave_shift);

        let mut slots: Vec<SampleSlot> = Vec::new();
        let mut slot_of: HashMap<PathBuf, usize> = HashMap::new();
        let mut regions = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        for (m, dp, no) in raw {
            if let Some(r) = compile(&m, &dp, no, root, &cc, &mut slots, &mut slot_of, &mut missing)? {
                regions.push(r);
            }
        }
        if !missing.is_empty() && !opt.skip_missing {
            return Err(format!(
                "{} sample file(s) not found, e.g. {} (pass skip_missing to load what exists)",
                missing.len(),
                missing[0]
            ));
        }
        if regions.is_empty() {
            return Err("no playable regions".into());
        }
        // Choke mapping (see `Voice::exclusive`): a region with off_by=N is
        // choked by notes of group N; give both the class N.
        let offs: std::collections::HashSet<u32> = regions.iter().map(|r| r.off_by).filter(|&o| o != 0).collect();
        let mut exclusive_of_group = HashMap::new();
        for r in &regions {
            if r.group != 0 && offs.contains(&r.group) {
                exclusive_of_group.insert(r.group, r.group);
            }
        }
        Ok(SfzInstrument { name, regions, slots, exclusive_of_group })
    }

    fn exclusive(&self, r: &Region) -> u32 {
        if r.off_by != 0 {
            r.off_by
        } else {
            self.exclusive_of_group.get(&r.group).copied().unwrap_or(0)
        }
    }

    fn osc(&self, r: &Region, note: &NoteOn, key: i32, vel: f32) -> Option<Osc> {
        let buf = self.slots[r.slot].get().ok()?.clone();
        let len = buf.len as u32;
        let file_loop = buf.file_loop;
        let loop_mode = r.loop_mode.unwrap_or(if file_loop.is_some() { LoopMode::Continuous } else { LoopMode::None });
        let (fls, fle) = file_loop.unwrap_or((0, len));
        let ls = r.loop_start.unwrap_or(fls).min(len);
        let le = r.loop_end.unwrap_or(fle).min(len);
        let cents = (note.key - r.keycenter) * r.keytrack + r.tune;
        let ratio = (cents as f64 / 1200.0).exp2() * buf.rate as f64 / note.rate as f64;
        let vel_cb = if vel > 0.0 { -400.0 * vel.log10() * r.veltrack / 100.0 } else { 1440.0 };
        let atten = -10.0 * r.volume - 200.0 * (r.amplitude / 100.0).max(1e-5).log10() + vel_cb.max(0.0);
        let (filter, fc, q) = match r.cutoff {
            Some(hz) if r.filter != FilterKind::Off => {
                let c = hz_to_cents(hz) + r.fil_veltrack * vel + r.fil_keytrack * (key as f32 - r.fil_keycenter);
                (r.filter, c, 10f32.powf((r.resonance - 3.01) / 20.0))
            }
            _ => (FilterKind::Off, 13500.0, 0.707),
        };
        Some(Osc::new(OscParams {
            sample: buf,
            start: r.offset.min(len),
            end: r.end.unwrap_or(len).min(len),
            loop_start: ls,
            loop_end: le,
            loop_mode,
            ratio,
            rate: note.rate,
            atten,
            gain: if q > 1.0 { 1.0 / q.sqrt() } else { 1.0 },
            pan: (r.pan / 100.0).clamp(-1.0, 1.0),
            vol_env: r.env,
            mod_env: Env::GATE,
            mod_env_pitch: 0.0,
            mod_env_fc: 0.0,
            mod_lfo: Lfo::default(),
            mod_lfo_pitch: 0.0,
            mod_lfo_fc: 0.0,
            mod_lfo_vol: 0.0,
            vib_lfo: Lfo::default(),
            vib_lfo_pitch: 0.0,
            filter,
            fc,
            q,
            exclusive: self.exclusive(r),
            interp: interp::interp(),
        }))
    }
}

#[allow(clippy::too_many_arguments)]
fn compile(
    m: &HashMap<String, String>,
    default_path: &str,
    note_off: i32,
    root: &Path,
    cc: &[f32; 128],
    slots: &mut Vec<SampleSlot>,
    slot_of: &mut HashMap<PathBuf, usize>,
    missing: &mut Vec<String>,
) -> Result<Option<Region>, String> {
    // controller conditions against the defaults
    for (k, v) in m {
        let lim = v.trim().parse::<f32>().unwrap_or(0.0);
        if let Some(n) = k.strip_prefix("locc").and_then(|n| n.parse::<usize>().ok()) {
            if n < 128 && cc[n] * 127.0 < lim - 0.5 {
                return Ok(None);
            }
        }
        if let Some(n) = k.strip_prefix("hicc").and_then(|n| n.parse::<usize>().ok()) {
            if n < 128 && cc[n] * 127.0 > lim + 0.5 {
                return Ok(None);
            }
        }
    }
    // key switches: only the default switch's regions sound
    if let Some(sw) = m.get("sw_last").and_then(|v| parse_key(v)) {
        let def = m.get("sw_default").and_then(|v| parse_key(v)).or_else(|| m.get("sw_lokey").and_then(|v| parse_key(v)));
        if def.is_some_and(|d| d != sw) {
            return Ok(None);
        }
    }
    let Some(sample) = m.get("sample") else { return Ok(None) };
    // X_onccN / X_ccN modulation at controller defaults
    let modulated = |base: &str| -> f32 {
        let mut add = 0.0;
        for (k, v) in m {
            let rest = k.strip_prefix(base).and_then(|r| r.strip_prefix("_oncc").or_else(|| r.strip_prefix("_cc")));
            if let Some(n) = rest.and_then(|n| n.parse::<usize>().ok()) {
                if n < 128 {
                    let amt = v.trim().parse::<f32>().unwrap_or(0.0);
                    let curve = m.get(&format!("{base}_curvecc{n}")).and_then(|c| c.trim().parse::<i32>().ok()).unwrap_or(0);
                    let x = if curve == 2 { 1.0 - cc[n] } else { cc[n] };
                    add += amt * x;
                }
            }
        }
        add
    };
    let f = |k: &str, d: f32| num(m, k).unwrap_or(d) + modulated(k);
    let key_of = |k: &str| m.get(k).and_then(|v| parse_key(v));

    let slot = if let Some(b) = builtin(sample.trim()) {
        let s = SampleSlot::new(None, Source::File(PathBuf::new()), b.rate);
        let _ = s.data.set(Ok(Arc::new(b)));
        slots.push(s);
        slots.len() - 1
    } else {
        let rel = format!("{}{}", default_path, sample.trim().replace('\\', "/"));
        let path = match resolve(root, &rel) {
            Some(p) if p.is_file() => p,
            _ => {
                missing.push(root.join(&rel).display().to_string());
                return Ok(None);
            }
        };
        match slot_of.get(&path) {
            Some(&i) => i,
            None => {
                slots.push(SampleSlot::new(None, Source::File(path.clone()), 0));
                slot_of.insert(path, slots.len() - 1);
                slots.len() - 1
            }
        }
    };

    let mut keys = (key_of("lokey").unwrap_or(0), key_of("hikey").unwrap_or(127));
    let mut keycenter = key_of("pitch_keycenter").unwrap_or(60) as f32;
    if let Some(k) = key_of("key") {
        keys = (k, k);
        if !m.contains_key("pitch_keycenter") {
            keycenter = k as f32;
        }
    }
    keys = (keys.0 + note_off, keys.1 + note_off);
    keycenter += note_off as f32;
    let vels = (num(m, "lovel").unwrap_or(0.0) as i32, num(m, "hivel").unwrap_or(127.0) as i32);
    let loop_mode = m.get("loop_mode").or_else(|| m.get("loopmode")).map(|v| match v.trim() {
        "one_shot" => LoopMode::OneShot,
        "loop_continuous" => LoopMode::Continuous,
        "loop_sustain" => LoopMode::UntilRelease,
        _ => LoopMode::None,
    });
    let loop_start = num(m, "loop_start").or_else(|| num(m, "loopstart")).map(|v| v.max(0.0) as u32);
    let loop_end = num(m, "loop_end").or_else(|| num(m, "loopend")).map(|v| v.max(0.0) as u32 + 1);
    let sustain_pct = f("ampeg_sustain", 100.0).clamp(0.0, 100.0);
    let env = Env {
        delay: f("ampeg_delay", 0.0).max(0.0),
        attack: f("ampeg_attack", 0.0).max(0.0),
        hold: f("ampeg_hold", 0.0).max(0.0),
        decay: f("ampeg_decay", 0.0).max(0.0),
        sustain: if sustain_pct <= 0.0 { 1000.0 } else { -200.0 * (sustain_pct / 100.0).log10() },
        release: f("ampeg_release", 0.001).max(0.0005),
    };
    let filter = match m.get("fil_type").map(|s| s.trim().to_lowercase()) {
        None => FilterKind::Lp,
        Some(t) if t.starts_with("lpf") => FilterKind::Lp,
        Some(t) if t.starts_with("hpf") => FilterKind::Hp,
        Some(t) if t.starts_with("bpf") => FilterKind::Bp,
        Some(t) if t.starts_with("brf") => FilterKind::Br,
        Some(_) => FilterKind::Lp,
    };
    let cutoff = num(m, "cutoff").map(|hz| crate::sf2::engine::cents_to_hz(hz_to_cents(hz) + modulated("cutoff")));
    let trigger = match m.get("trigger").map(|s| s.trim()) {
        Some("release") => Trigger::Release,
        Some("first") => Trigger::First,
        Some("legato") => Trigger::Legato,
        _ => Trigger::Attack,
    };
    Ok(Some(Region {
        slot,
        keys,
        vels,
        keycenter,
        keytrack: f("pitch_keytrack", 100.0),
        tune: f("tune", 0.0) + 100.0 * num(m, "transpose").unwrap_or(0.0),
        volume: f("volume", 0.0),
        amplitude: num(m, "amplitude").unwrap_or(100.0),
        veltrack: f("amp_veltrack", 100.0).clamp(-100.0, 100.0),
        pan: f("pan", 0.0),
        offset: f("offset", 0.0).max(0.0) as u32,
        end: num(m, "end").map(|e| e.max(0.0) as u32 + 1),
        loop_mode,
        loop_start,
        loop_end,
        env,
        filter,
        cutoff,
        resonance: num(m, "resonance").unwrap_or(0.0).max(0.0),
        fil_veltrack: num(m, "fil_veltrack").unwrap_or(0.0),
        fil_keytrack: num(m, "fil_keytrack").unwrap_or(0.0),
        fil_keycenter: key_of("fil_keycenter").unwrap_or(60) as f32,
        seq_length: num(m, "seq_length").unwrap_or(1.0).max(1.0) as u32,
        seq_position: num(m, "seq_position").unwrap_or(1.0).max(1.0) as u32,
        rand: (num(m, "lorand").unwrap_or(0.0), num(m, "hirand").unwrap_or(1.0)),
        trigger,
        group: num(m, "group").unwrap_or(0.0).max(0.0) as u32,
        off_by: num(m, "off_by").unwrap_or(0.0).max(0.0) as u32,
        rt_decay: num(m, "rt_decay").unwrap_or(0.0),
    }))
}

impl Instrument for SfzInstrument {
    fn voice(&self, note: &NoteOn) -> Box<dyn Voice> {
        let key = note.key.round() as i32;
        let vel7 = (note.vel * 127.0).round().clamp(1.0, 127.0) as i32;
        let vel = vel7 as f32 / 127.0;
        let rnd = Rng::new(note.seed ^ 0x5F2_5A3B).f32();
        let mut oscs = Vec::new();
        let mut rel = Vec::new();
        let mut decay = Vec::new();
        for r in &self.regions {
            if key < r.keys.0 || key > r.keys.1 || vel7 < r.vels.0 || vel7 > r.vels.1 {
                continue;
            }
            if r.seq_length > 1 && (note.rr % r.seq_length) + 1 != r.seq_position {
                continue;
            }
            if rnd < r.rand.0 || rnd >= r.rand.1 {
                continue;
            }
            match r.trigger {
                Trigger::Attack | Trigger::First => {
                    if let Some(o) = self.osc(r, note, key, vel) {
                        oscs.push(o);
                    }
                }
                Trigger::Release => {
                    if let Some(o) = self.osc(r, note, key, vel) {
                        rel.push(o);
                        decay.push(r.rt_decay);
                    }
                }
                Trigger::Legato => {}
            }
        }
        let exclusive = oscs.iter().chain(rel.iter()).map(|o| o.exclusive()).find(|&e| e != 0).unwrap_or(0);
        Box::new(SamplerVoice::with_exclusive(oscs, rel, decay, note.rate, exclusive))
    }
}

/// Load an SFZ instrument (samples decoded lazily on first use).
pub fn load_sfz(path: impl AsRef<Path>) -> Result<Arc<dyn Instrument>, String> {
    Ok(Arc::new(SfzInstrument::load(path)?))
}

/// Cache of loaded SFZ instruments by path (so packs load once per process).
pub fn load_sfz_cached(path: impl AsRef<Path>) -> Result<Arc<dyn Instrument>, String> {
    use std::sync::Mutex;
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<dyn Instrument>>>> = OnceLock::new();
    let p = path.as_ref().to_path_buf();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(i) = cache.lock().unwrap().get(&p) {
        return Ok(i.clone());
    }
    let i = load_sfz(&p)?;
    cache.lock().unwrap().insert(p, i.clone());
    Ok(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sfz_note_names() {
        assert_eq!(parse_key("c4"), Some(60));
        assert_eq!(parse_key("C#4"), Some(61));
        assert_eq!(parse_key("db4"), Some(61));
        assert_eq!(parse_key("a4"), Some(69));
        assert_eq!(parse_key("c-1"), Some(0));
        assert_eq!(parse_key("72"), Some(72));
        assert_eq!(parse_key("h4"), None);
    }

    #[test]
    fn sfz_lexer_defines_includes_and_spaces() {
        let mut lx = Lexer { defines: Vec::new(), root: PathBuf::from("."), depth: 0, events: Vec::new() };
        lx.text("#define $V v8\n#define $EXT flac\n<region> sample=My File $V.$EXT lokey=c4 // comment\n/* block\n */ <group> volume=-3 #define $V v9\n<region>sample=x$V.wav")
            .unwrap();
        assert_eq!(
            lx.events,
            vec![
                Ev::Header("region".into()),
                Ev::Op("sample".into(), "My File v8.flac".into()),
                Ev::Op("lokey".into(), "c4".into()),
                Ev::Header("group".into()),
                Ev::Op("volume".into(), "-3".into()),
                Ev::Header("region".into()),
                Ev::Op("sample".into(), "xv9.wav".into()),
            ]
        );
    }

    fn write_wav(path: &Path, data: &[f32], rate: u32, lp: Option<(u32, u32)>) {
        let mut w = Vec::new();
        let n = data.len() as u32;
        let smpl_len = if lp.is_some() { 8 + 60 } else { 0 };
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + n * 2 + smpl_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(n * 2).to_le_bytes());
        for v in data {
            w.extend_from_slice(&((v * 32767.0).round() as i16).to_le_bytes());
        }
        if let Some((s, e)) = lp {
            w.extend_from_slice(b"smpl");
            w.extend_from_slice(&60u32.to_le_bytes());
            let mut h = [0u32; 15];
            h[7] = 1; // loops
            h[9 + 2] = s;
            h[9 + 3] = e - 1;
            for v in h {
                w.extend_from_slice(&v.to_le_bytes());
            }
        }
        std::fs::write(path, w).unwrap();
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anikit-sfz-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(d.join("Samples")).unwrap();
        d
    }

    #[test]
    fn sfz_round_robin_release_trigger_and_loops() {
        use crate::sf2::tests::{measure_freq, render};
        let d = dir("rr");
        let sine = |f: f64, n: usize| -> Vec<f32> { (0..n).map(|i| (2.0 * std::f64::consts::PI * f * i as f64 / 44_000.0).sin() as f32 * 0.5).collect() };
        // 100-frame period at 44 kHz = 440 Hz; loop over whole periods
        write_wav(&d.join("Samples/A.wav"), &sine(440.0, 4400), 44_000, Some((1000, 3000)));
        write_wav(&d.join("Samples/b.wav"), &sine(660.0, 4400), 44_000, None);
        write_wav(&d.join("Samples/rel.wav"), &sine(880.0, 2200), 44_000, None);
        let text = "<control> default_path=samples\\\n<global> ampeg_release=0.05\n\
            <group> seq_length=2 seq_position=1 <region> sample=a.wav key=a4\n\
            <group> seq_length=2 seq_position=2 <region> sample=B.wav lokey=a4 hikey=a4 pitch_keycenter=e5 tune=-2 transpose=0\n\
            <group> trigger=release rt_decay=6 <region> sample=rel.wav key=a4 volume=-6";
        let inst = SfzInstrument::from_str(text, &d, SfzOptions::default()).unwrap();
        assert_eq!(inst.regions(), 3);
        let mk = |rr: u32| NoteOn { key: 69.0, vel: 1.0, rate: 48_000.0, seed: 3, rr, len: 1.0 };
        // rr 0 → looped 440 Hz sample (keeps sounding well past its 0.1 s length)
        let mut v = inst.voice(&mk(0));
        let (l, _, _) = render(v.as_mut(), 1.0, 10.0, 128);
        let f = measure_freq(&l[24_000..], 440.0, 48_000.0);
        assert!((1200.0 * (f / 440.0).log2()).abs() < 1.0, "{f}");
        // rr 1 → 660 Hz sample mapped from e5 (659.26) to a4: plays at 440·660/659.26, −2 cents
        let mut v = inst.voice(&mk(1));
        let (l, _, _) = render(v.as_mut(), 0.08, 10.0, 128);
        let want = 440.0 * 660.0 / 659.255 * 2f64.powf(-2.0 / 1200.0);
        let f = measure_freq(&l[480..], want, 48_000.0);
        assert!((1200.0 * (f / want).log2()).abs() < 2.0, "{f} vs {want}");
        // release trigger: an 880 Hz tail starts at note-off
        let mut v = inst.voice(&mk(1));
        let (l, _, _) = render(v.as_mut(), 0.4, 0.2, 128);
        let tail = &l[(0.21 * 48_000.0) as usize..(0.24 * 48_000.0) as usize];
        let f = measure_freq(tail, 880.0, 48_000.0);
        assert!((1200.0 * (f / 880.0).log2()).abs() < 10.0, "{f}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sfz_missing_samples_are_reported() {
        let d = dir("missing");
        let e = SfzInstrument::from_str("<region> sample=nope.wav", &d, SfzOptions::default()).unwrap_err();
        assert!(e.contains("not found"), "{e}");
        let e = SfzInstrument::from_str("<region> sample=nope.wav", &d, SfzOptions { skip_missing: true }).unwrap_err();
        assert!(e.contains("no playable regions"), "{e}");
        assert!(load_sfz(d.join("none.sfz")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sfz_builtin_sine_and_filter() {
        let inst = SfzInstrument::from_str(
            "<region> sample=*sine pitch_keycenter=60 cutoff=300 fil_type=lpf_2p ampeg_release=0.01",
            ".",
            SfzOptions::default(),
        )
        .unwrap();
        let mut v = inst.voice(&NoteOn { key: 69.0, vel: 1.0, rate: 48_000.0, seed: 0, rr: 0, len: 1.0 });
        let (l, _, alive) = crate::sf2::tests::render(v.as_mut(), 0.5, 0.3, 128);
        let f = crate::sf2::tests::measure_freq(&l[4_800..14_400], 440.0, 48_000.0);
        assert!((1200.0 * (f / 440.0).log2()).abs() < 1.0, "{f}");
        // 440 Hz through a 300 Hz 2-pole low-pass: ≈ −7 dB (×0.707 pan) vs unity
        let peak = l[9_600..14_400].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.2 && peak < 0.45, "{peak}");
        assert!(!alive);
    }
}
