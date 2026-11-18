//! Make anything: custom instruments (synth patches and sample mappings) in
//! JSON, per-track effect chains, and automation of any parameter over time.
//!
//! ```json
//! {"define": {
//!    "wob": {"synth": {"base": "bass-synth", "osc": [{"wave": "saw", "voices": 3, "detune": 14}],
//!            "filter": {"type": "ladder", "cutoff": 400, "res": 0.5, "env": 2}, "amp": [0.005, 0.3, 0.7, 0.2]}},
//!    "myKit": {"sampler": {"zones": [{"file": "kick.wav", "key": 36}, {"file": "snare.wav", "key": 38}]}}},
//!  "tracks": [{"inst": "wob", "notes": "o2 l8 (C2 C2 Eb2 G2)x4",
//!              "fx": [{"type": "delay", "note": "1/8d", "wet": 0.2}, {"type": "reverb", "preset": "plate", "wet": 0.15}],
//!              "automate": {"lp": [["@bar1", 300], ["@bar3", 8000]], "pan": "=0.5*sin(beat*pi/4)",
//!                           "synth.cutoff": [[0, 300], [16, 3000]]}}]}
//! ```

use crate::dsp::{self, Processor};
use crate::seq::Grid;
use crate::spec::{closest, TimeRef};
use crate::synth::osc::Wave;
use crate::synth::patch::{env, FilterKind, Osc, Patch, SubSynth};
use crate::voice::{Instrument, NoteOn, Voice};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

// ───────────────────────────── instruments ─────────────────────────────

fn num(v: &Value, what: &str) -> Result<f32, String> {
    v.as_f64().map(|x| x as f32).ok_or_else(|| format!("{what}: expected a number, got {v}"))
}

fn check_keys(o: &serde_json::Map<String, Value>, allowed: &[&str], at: &str) -> Result<(), String> {
    for k in o.keys() {
        if !allowed.contains(&k.as_str()) {
            let hint = closest(k, allowed).map(|b| format!(" — did you mean `{b}`?")).unwrap_or_default();
            return Err(format!("{at}: unknown field `{k}`{hint} (fields: {})", allowed.join(" ")));
        }
    }
    Ok(())
}

/// `[a, d, s, r]` or `{"a":…, "d":…, "s":…, "r":…}`.
fn adsr(v: &Value, at: &str) -> Result<crate::synth::patch::Env, String> {
    let get = |k: usize, name: &str| -> Result<f32, String> {
        match v {
            Value::Array(a) => num(a.get(k).ok_or_else(|| format!("{at}: [attack, decay, sustain, release]"))?, at),
            Value::Object(o) => num(o.get(name).ok_or_else(|| format!("{at}: missing `{name}`"))?, at),
            _ => Err(format!("{at}: [attack, decay, sustain, release] in seconds (sustain 0..1)")),
        }
    };
    Ok(env(get(0, "a")?, get(1, "d")?, get(2, "s")?, get(3, "r")?))
}

/// Set one synth patch parameter by name (used by definitions and per-note automation).
pub fn set_patch_param(p: &mut Patch, name: &str, v: f32) -> Result<(), String> {
    match name {
        "cutoff" => p.cutoff = v.clamp(20.0, 20_000.0),
        "res" | "resonance" => p.res = v.clamp(0.0, 1.0),
        "drive" => p.drive = v.max(0.0),
        "track" => p.track = v,
        "env" | "envAmt" => p.env_amt = v,
        "velCut" => p.vel_cut = v,
        "sub" => p.sub = v.max(0.0),
        "noise" => p.noise = v.max(0.0),
        "vib" => p.vib = v,
        "lfoRate" => p.lfo_rate = v.max(0.0),
        "lfoCut" => p.lfo_cut = v,
        "trem" => p.trem = v,
        "pwm" => p.pwm = v,
        "glide" => p.glide = v.max(0.0),
        "drift" => p.drift = v.max(0.0),
        "ensemble" => p.ensemble = v.max(0.0),
        "hp" => p.hp = v.max(0.0),
        "sat" => p.sat = v.max(0.0),
        "gain" => p.gain = 10f32.powf(v / 20.0),
        "pan" => p.pan = v.clamp(-1.0, 1.0),
        "attack" => p.amp.a = v.max(0.0),
        "decay" => p.amp.d = v.max(0.0),
        "sustain" => p.amp.s = v.clamp(0.0, 1.0),
        "release" => p.amp.r = v.max(0.0),
        "pw" => p.osc.iter_mut().flatten().for_each(|o| o.pw = v.clamp(0.02, 0.98)),
        "detune" => p.osc.iter_mut().flatten().for_each(|o| o.cents = v),
        _ => return Err(format!("unknown synth parameter `{name}` (cutoff res drive track env sub noise vib lfoRate lfoCut trem pwm glide drift ensemble hp sat gain pan attack decay sustain release pw detune)")),
    }
    Ok(())
}

/// A synth patch from JSON (see the module docs).
pub fn synth_patch(v: &Value, at: &str) -> Result<Patch, String> {
    let o = v.as_object().ok_or_else(|| format!("{at}: a synth definition is an object"))?;
    check_keys(o, &["base", "osc", "sub", "noise", "filter", "fenv", "amp", "lfo", "pitch", "glide", "mono", "drift", "ensemble", "stereo", "hp", "sat", "gain", "pan", "vel"], at)?;
    let mut p = match o.get("base").and_then(Value::as_str) {
        Some(b) => crate::synth::subtractive(b).ok_or_else(|| {
            let hint = closest(b, crate::synth::PRESETS).map(|x| format!(" — did you mean '{x}'?")).unwrap_or_default();
            format!("{at}.base: '{b}' is not a subtractive synth preset{hint}")
        })?,
        None => {
            let mut p = Patch::base();
            p.osc[0] = Some(Osc::new(Wave::Saw, 1, 0.0, 0.8));
            p
        }
    };
    if let Some(oscs) = o.get("osc") {
        let list = oscs.as_array().ok_or_else(|| format!("{at}.osc: a list of up to 3 oscillators"))?;
        if list.len() > 3 {
            return Err(format!("{at}.osc: at most 3 oscillators"));
        }
        p.osc = [None, None, None];
        for (i, x) in list.iter().enumerate() {
            let ap = format!("{at}.osc[{i}]");
            let xo = x.as_object().ok_or_else(|| format!("{ap}: an object like {{\"wave\":\"saw\",\"voices\":3,\"detune\":12}}"))?;
            check_keys(xo, &["wave", "voices", "detune", "semis", "level", "pw", "width", "mix"], &ap)?;
            let f = |k: &str, d: f32| xo.get(k).map(|v| num(v, &format!("{ap}.{k}"))).unwrap_or(Ok(d));
            let wave = xo.get("wave").and_then(Value::as_str).unwrap_or("saw");
            let mut osc = if wave == "supersaw" {
                Osc::supersaw(f("detune", 0.5)?, f("mix", 0.6)?, f("width", 0.8)?, f("level", 0.8)?)
            } else {
                let w = match wave {
                    "saw" => Wave::Saw,
                    "square" | "pulse" => Wave::Pulse,
                    "tri" | "triangle" => Wave::Tri,
                    "sine" => Wave::Sine,
                    other => return Err(format!("{ap}.wave: '{other}' (saw square pulse tri sine supersaw)")),
                };
                Osc::new(w, f("voices", 1.0)?.clamp(1.0, 8.0) as usize, f("detune", 0.0)?, f("level", 0.8)?).width(f("width", 0.0)?).pw(f("pw", 0.5)?)
            };
            osc.semis = f("semis", 0.0)?;
            p.osc[i] = Some(osc);
        }
    }
    for (k, field) in [("sub", "sub"), ("noise", "noise"), ("glide", "glide"), ("drift", "drift"), ("ensemble", "ensemble"), ("hp", "hp"), ("sat", "sat"), ("gain", "gain"), ("pan", "pan")] {
        if let Some(x) = o.get(k) {
            set_patch_param(&mut p, field, num(x, &format!("{at}.{k}"))?)?;
        }
    }
    if let Some(x) = o.get("vel") {
        p.vel_amt = num(x, &format!("{at}.vel"))?;
    }
    if let Some(b) = o.get("mono") {
        p.mono = b.as_bool().ok_or_else(|| format!("{at}.mono: true or false"))?;
    }
    if let Some(b) = o.get("stereo") {
        p.stereo = b.as_bool().ok_or_else(|| format!("{at}.stereo: true or false"))?;
    }
    if let Some(f) = o.get("filter") {
        let fo = f.as_object().ok_or_else(|| format!("{at}.filter: an object like {{\"type\":\"ladder\",\"cutoff\":1200,\"res\":0.3}}"))?;
        check_keys(fo, &["type", "cutoff", "res", "drive", "track", "env", "vel", "lfo"], &format!("{at}.filter"))?;
        if let Some(t) = fo.get("type").and_then(Value::as_str) {
            p.filter = match t {
                "ladder" => FilterKind::Ladder,
                "lp" | "lp2" => FilterKind::Lp,
                "lp4" => FilterKind::Lp4,
                "bp" => FilterKind::Bp,
                "formant" | "vowel" => FilterKind::Formant,
                o => return Err(format!("{at}.filter.type: '{o}' (ladder lp lp4 bp formant)")),
            };
        }
        for (k, name) in [("cutoff", "cutoff"), ("res", "res"), ("drive", "drive"), ("track", "track"), ("env", "env"), ("vel", "velCut"), ("lfo", "lfoCut")] {
            if let Some(x) = fo.get(k) {
                set_patch_param(&mut p, name, num(x, &format!("{at}.filter.{k}"))?)?;
            }
        }
    }
    if let Some(x) = o.get("amp") {
        p.amp = adsr(x, &format!("{at}.amp"))?;
    }
    if let Some(x) = o.get("fenv") {
        p.fenv = adsr(x, &format!("{at}.fenv"))?;
    }
    if let Some(l) = o.get("lfo") {
        let lo = l.as_object().ok_or_else(|| format!("{at}.lfo: an object like {{\"rate\":5,\"vib\":8}}"))?;
        check_keys(lo, &["rate", "shape", "delay", "vib", "cutoff", "trem", "pwm"], &format!("{at}.lfo"))?;
        for (k, name) in [("rate", "lfoRate"), ("vib", "vib"), ("cutoff", "lfoCut"), ("trem", "trem"), ("pwm", "pwm")] {
            if let Some(x) = lo.get(k) {
                set_patch_param(&mut p, name, num(x, &format!("{at}.lfo.{k}"))?)?;
            }
        }
        if let Some(x) = lo.get("delay") {
            p.lfo_delay = num(x, &format!("{at}.lfo.delay"))?;
        }
        if let Some(s) = lo.get("shape").and_then(Value::as_str) {
            p.lfo_shape = match s {
                "sine" => crate::synth::env::LfoShape::Sine,
                "tri" | "triangle" => crate::synth::env::LfoShape::Triangle,
                o => return Err(format!("{at}.lfo.shape: '{o}' (sine tri)")),
            };
        }
    }
    if let Some(x) = o.get("pitch") {
        let a = x.as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("{at}.pitch: [semitones, seconds] pitch envelope"))?;
        p.pitch_env = (num(&a[0], at)?, num(&a[1], at)?);
    }
    Ok(p)
}

/// A custom synth: the patch, optionally re-parameterised per note by automation.
pub struct CustomSynth {
    pub synth: SubSynth,
    /// (parameter, automation) applied at each note-on.
    pub per_note: Vec<(String, Automation)>,
}

impl Instrument for CustomSynth {
    fn voice(&self, note: &NoteOn) -> Box<dyn Voice> {
        self.synth.voice(note)
    }
    fn mono(&self) -> bool {
        self.synth.patch.mono
    }
    fn ordered_voices(&self) -> bool {
        self.synth.ordered_voices()
    }
    fn voice_legato(&self, note: &NoteOn, legato: bool) -> Box<dyn Voice> {
        self.synth.voice_ex(&self.synth.patch, note, legato)
    }
}

impl CustomSynth {
    /// A voice with the per-note automation evaluated at `secs`.
    pub fn voice_at(&self, note: &NoteOn, secs: f64, legato: bool) -> Box<dyn Voice> {
        if self.per_note.is_empty() {
            return self.synth.voice_ex(&self.synth.patch, note, legato);
        }
        let mut p = self.synth.patch.clone();
        for (k, a) in &self.per_note {
            let _ = set_patch_param(&mut p, k, a.at(secs) as f32);
        }
        self.synth.voice_ex(&p, note, legato)
    }
}

/// A sampler from zones: written as SFZ and loaded by the SFZ engine.
pub fn sampler_sfz(v: &Value, at: &str) -> Result<String, String> {
    let o = v.as_object().ok_or_else(|| format!("{at}: a sampler definition is an object with `zones`"))?;
    check_keys(o, &["zones", "attack", "release", "oneShot", "volume"], at)?;
    let zones = o.get("zones").and_then(Value::as_array).ok_or_else(|| format!("{at}.zones: a list like [{{\"file\":\"c4.wav\",\"root\":60}}]"))?;
    let mut s = String::from("<group>");
    if let Some(a) = o.get("attack") {
        s += &format!(" ampeg_attack={}", num(a, &format!("{at}.attack"))?);
    }
    s += &format!(" ampeg_release={}", o.get("release").map(|r| num(r, &format!("{at}.release"))).unwrap_or(Ok(0.25))?);
    if o.get("oneShot").and_then(Value::as_bool) == Some(true) {
        s += " loop_mode=one_shot";
    }
    if let Some(vol) = o.get("volume") {
        s += &format!(" volume={}", num(vol, &format!("{at}.volume"))?);
    }
    s.push('\n');
    for (i, z) in zones.iter().enumerate() {
        let ap = format!("{at}.zones[{i}]");
        let zo = z.as_object().ok_or_else(|| format!("{ap}: an object with `file`"))?;
        check_keys(zo, &["file", "key", "lo", "hi", "root", "vel", "loop", "tune", "gain", "pan", "seq"], &ap)?;
        let file = zo.get("file").and_then(Value::as_str).ok_or_else(|| format!("{ap}.file: the sample's path (wav, flac, ogg)"))?;
        let key = |k: &str| -> Result<Option<i32>, String> {
            match zo.get(k) {
                None => Ok(None),
                Some(Value::Number(n)) => Ok(n.as_i64().map(|x| x as i32)),
                Some(Value::String(n)) => crate::theory::note_key(n).map(Some).ok_or_else(|| format!("{ap}.{k}: a MIDI key or note name like \"C4\"")),
                Some(o) => Err(format!("{ap}.{k}: a MIDI key or note name, got {o}")),
            }
        };
        let mut r = format!("<region> sample={}", file.replace(' ', "\\ "));
        let (single, lo, hi, root) = (key("key")?, key("lo")?, key("hi")?, key("root")?);
        if let Some(k) = single {
            r += &format!(" key={k}");
        }
        if let Some(k) = lo {
            r += &format!(" lokey={k}");
        }
        if let Some(k) = hi {
            r += &format!(" hikey={k}");
        }
        if let Some(k) = root.or(single) {
            r += &format!(" pitch_keycenter={k}");
        }
        if single.is_some() && root.is_none() && lo.is_none() && hi.is_none() {
            // A one-key hit (drum): don't repitch.
            r += " pitch_keytrack=0";
        }
        if let Some(v) = zo.get("vel") {
            let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("{ap}.vel: [low, high] MIDI velocities 1–127"))?;
            r += &format!(" lovel={} hivel={}", num(&a[0], &ap)? as i32, num(&a[1], &ap)? as i32);
        }
        if let Some(v) = zo.get("loop") {
            let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("{ap}.loop: [start, end] in samples"))?;
            r += &format!(" loop_mode=loop_continuous loop_start={} loop_end={}", num(&a[0], &ap)? as i64, num(&a[1], &ap)? as i64);
        }
        if let Some(v) = zo.get("tune") {
            r += &format!(" tune={}", num(v, &ap)?);
        }
        if let Some(v) = zo.get("gain") {
            r += &format!(" volume={}", num(v, &ap)?);
        }
        if let Some(v) = zo.get("pan") {
            r += &format!(" pan={}", num(v, &ap)? * 100.0);
        }
        if let Some(v) = zo.get("seq") {
            // Round robin: [position, length].
            let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("{ap}.seq: [position, length] round robin (1-based)"))?;
            r += &format!(" seq_position={} seq_length={}", num(&a[0], &ap)? as i32, num(&a[1], &ap)? as i32);
        }
        s += &r;
        s.push('\n');
    }
    Ok(s)
}

/// Build a defined instrument. `per_note` is the track's patch automation (`synth.*`).
pub fn build(name: &str, def: &Value, base: Option<&Path>, per_note: Vec<(String, Automation)>) -> Result<Arc<dyn Instrument>, String> {
    let at = format!("define.{name}");
    let o = def.as_object().ok_or_else(|| format!("{at}: {{\"synth\": {{…}}}} or {{\"sampler\": {{…}}}}"))?;
    check_keys(o, &["synth", "sampler"], &at)?;
    if let Some(s) = o.get("synth") {
        let p = synth_patch(s, &format!("{at}.synth"))?;
        return Ok(Arc::new(CustomSynth { synth: SubSynth::new(p), per_note }));
    }
    if let Some(s) = o.get("sampler") {
        let text = sampler_sfz(s, &format!("{at}.sampler"))?;
        let inst = crate::sf2::sfz::SfzInstrument::from_str(&text, base.unwrap_or(Path::new(".")), Default::default()).map_err(|e| format!("{at}.sampler: {e}"))?;
        return Ok(Arc::new(inst));
    }
    Err(format!("{at}: give \"synth\" or \"sampler\""))
}

// ───────────────────────────── automation ─────────────────────────────

/// A value over time: keyframes (linear between them, held outside) or an expression.
#[derive(Debug, Clone)]
pub enum Automation {
    /// (seconds, value), sorted.
    Keys(Vec<(f64, f64)>),
    Expr(Arc<Expr>, Grid),
}

impl Automation {
    /// `[[time, value], …]` (time: beats as a number, or "@bar5", "@beat3", "@2.5s"-style strings) or `"=expr"`.
    pub fn parse(v: &Value, grid: &Grid, at: &str) -> Result<Automation, String> {
        match v {
            Value::String(s) if s.trim_start().starts_with('=') => Ok(Automation::Expr(Arc::new(Expr::parse(&s.trim_start()[1..]).map_err(|e| format!("{at}: {e}"))?), *grid)),
            Value::Number(n) => Ok(Automation::Keys(vec![(0.0, n.as_f64().unwrap_or(0.0))])),
            Value::Array(a) => {
                let mut keys = Vec::with_capacity(a.len());
                for (i, k) in a.iter().enumerate() {
                    let p = k.as_array().filter(|p| p.len() == 2).ok_or_else(|| format!("{at}[{i}]: [time, value] — time in beats or \"@bar5\""))?;
                    let t = match &p[0] {
                        Value::Number(b) => grid.secs(b.as_f64().unwrap_or(0.0)) - grid.offset,
                        Value::String(s) => TimeRef::Str(s.clone()).secs(grid).map_err(|e| format!("{at}[{i}]: {e}"))?,
                        o => return Err(format!("{at}[{i}]: time {o}: beats or \"@bar5\"")),
                    };
                    let v = p[1].as_f64().ok_or_else(|| format!("{at}[{i}]: value must be a number"))?;
                    keys.push((t, v));
                }
                if keys.is_empty() {
                    return Err(format!("{at}: no keyframes"));
                }
                keys.sort_by(|x, y| x.0.total_cmp(&y.0));
                Ok(Automation::Keys(keys))
            }
            o => Err(format!("{at}: keyframes [[beat, value], …] or \"=expr\" (e.g. \"=0.5*sin(beat*pi/4)\"), got {o}")),
        }
    }

    /// Value at `t` seconds (music timeline).
    pub fn at(&self, t: f64) -> f64 {
        match self {
            Automation::Keys(k) => {
                let i = k.partition_point(|x| x.0 <= t);
                if i == 0 {
                    k[0].1
                } else if i == k.len() {
                    k[i - 1].1
                } else {
                    let (a, b) = (k[i - 1], k[i]);
                    let f = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 1.0 };
                    a.1 + (b.1 - a.1) * f
                }
            }
            Automation::Expr(e, g) => {
                let beat = (t / g.beat()).max(0.0);
                e.eval(&|v| match v {
                    "t" | "time" => Some(t),
                    "beat" => Some(beat),
                    "bar" => Some(beat / g.beats_per_bar),
                    "bpm" => Some(g.bpm),
                    _ => None,
                })
            }
        }
    }

    /// Sample the curve every 32 frames and interpolate linearly: `out[i]` at frame i.
    pub fn render(&self, frames: usize, rate: u32) -> Vec<f32> {
        const STEP: usize = 32;
        let pts: Vec<f64> = (0..=frames / STEP + 1).map(|k| self.at((k * STEP) as f64 / rate as f64)).collect();
        (0..frames)
            .map(|i| {
                let k = i / STEP;
                let f = (i % STEP) as f64 / STEP as f64;
                (pts[k] + (pts[k + 1] - pts[k]) * f) as f32
            })
            .collect()
    }
}

/// A tiny arithmetic expression: numbers, `beat bar t bpm pi e`, + − × ÷ ^ %,
/// parentheses, comparisons, and `sin cos tan abs floor ceil sqrt exp log min max
/// clamp(x,lo,hi) mix(a,b,f) saw(x) tri(x) sq(x) step(edge,x) smooth(x)` (periodic
/// shapes take cycles: saw(beat/4) ramps 0→1 every 4 beats).
#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

impl Expr {
    pub fn parse(s: &str) -> Result<Expr, String> {
        let toks = lex(s)?;
        let mut p = P { t: &toks, i: 0 };
        let e = p.cmp()?;
        if p.i != toks.len() {
            return Err(format!("expression '{s}': unexpected '{}'", toks[p.i]));
        }
        // Validate names now (clear errors before rendering).
        e.check()?;
        Ok(e)
    }

    fn check(&self) -> Result<(), String> {
        match self {
            Expr::Num(_) => Ok(()),
            Expr::Var(v) => match v.as_str() {
                "beat" | "bar" | "t" | "time" | "bpm" | "pi" | "e" => Ok(()),
                o => Err(format!("unknown variable '{o}' (beat bar t bpm pi e)")),
            },
            Expr::Neg(a) => a.check(),
            Expr::Bin(_, a, b) => a.check().and(b.check()),
            Expr::Call(f, args) => {
                let n = match f.as_str() {
                    "sin" | "cos" | "tan" | "abs" | "floor" | "ceil" | "sqrt" | "exp" | "log" | "saw" | "tri" | "sq" | "smooth" | "fract" => 1,
                    "min" | "max" | "step" | "pow" => 2,
                    "clamp" | "mix" => 3,
                    o => return Err(format!("unknown function '{o}' (sin cos tan abs floor ceil sqrt exp log fract min max pow clamp mix saw tri sq step smooth)")),
                };
                if args.len() != n {
                    return Err(format!("{f}() takes {n} argument(s)"));
                }
                args.iter().try_for_each(Expr::check)
            }
        }
    }

    pub fn eval(&self, var: &dyn Fn(&str) -> Option<f64>) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::Var(v) => match v.as_str() {
                "pi" => std::f64::consts::PI,
                "e" => std::f64::consts::E,
                o => var(o).unwrap_or(0.0),
            },
            Expr::Neg(a) => -a.eval(var),
            Expr::Bin(op, a, b) => {
                let (x, y) = (a.eval(var), b.eval(var));
                match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    '/' => x / y,
                    '%' => x.rem_euclid(y),
                    '^' => x.powf(y),
                    '<' => f64::from(u8::from(x < y)),
                    '>' => f64::from(u8::from(x > y)),
                    _ => 0.0,
                }
            }
            Expr::Call(f, a) => {
                let v: Vec<f64> = a.iter().map(|x| x.eval(var)).collect();
                match f.as_str() {
                    "sin" => v[0].sin(),
                    "cos" => v[0].cos(),
                    "tan" => v[0].tan(),
                    "abs" => v[0].abs(),
                    "floor" => v[0].floor(),
                    "ceil" => v[0].ceil(),
                    "sqrt" => v[0].max(0.0).sqrt(),
                    "exp" => v[0].exp(),
                    "log" => v[0].max(1e-12).ln(),
                    "fract" => v[0].rem_euclid(1.0),
                    "saw" => v[0].rem_euclid(1.0),
                    "tri" => 1.0 - (2.0 * v[0].rem_euclid(1.0) - 1.0).abs(),
                    "sq" => f64::from(u8::from(v[0].rem_euclid(1.0) < 0.5)),
                    "smooth" => {
                        let x = v[0].clamp(0.0, 1.0);
                        x * x * (3.0 - 2.0 * x)
                    }
                    "min" => v[0].min(v[1]),
                    "max" => v[0].max(v[1]),
                    "pow" => v[0].powf(v[1]),
                    "step" => f64::from(u8::from(v[1] >= v[0])),
                    "clamp" => v[0].clamp(v[1].min(v[2]), v[2].max(v[1])),
                    "mix" => v[0] + (v[1] - v[0]) * v[2],
                    _ => 0.0,
                }
            }
        }
    }
}

fn lex(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || ch == '.' {
            let j = (i..c.len()).find(|&j| !(c[j].is_ascii_digit() || c[j] == '.')).unwrap_or(c.len());
            out.push(c[i..j].iter().collect());
            i = j;
        } else if ch.is_alphabetic() || ch == '_' {
            let j = (i..c.len()).find(|&j| !(c[j].is_alphanumeric() || c[j] == '_')).unwrap_or(c.len());
            out.push(c[i..j].iter().collect());
            i = j;
        } else if "+-*/^%(),<>".contains(ch) {
            out.push(ch.to_string());
            i += 1;
        } else {
            return Err(format!("expression '{s}': unexpected character '{ch}'"));
        }
    }
    Ok(out)
}

struct P<'a> {
    t: &'a [String],
    i: usize,
}

impl P<'_> {
    fn peek(&self) -> Option<&str> {
        self.t.get(self.i).map(String::as_str)
    }
    fn bin(&mut self, ops: &str, next: fn(&mut Self) -> Result<Expr, String>) -> Result<Expr, String> {
        let mut a = next(self)?;
        while let Some(op) = self.peek().filter(|o| o.len() == 1 && ops.contains(*o)) {
            let op = op.chars().next().unwrap();
            self.i += 1;
            a = Expr::Bin(op, Box::new(a), Box::new(next(self)?));
        }
        Ok(a)
    }
    fn cmp(&mut self) -> Result<Expr, String> {
        self.bin("<>", Self::add)
    }
    fn add(&mut self) -> Result<Expr, String> {
        self.bin("+-", Self::mul)
    }
    fn mul(&mut self) -> Result<Expr, String> {
        self.bin("*/%", Self::pow)
    }
    fn pow(&mut self) -> Result<Expr, String> {
        let a = self.unary()?;
        if self.peek() == Some("^") {
            self.i += 1;
            return Ok(Expr::Bin('^', Box::new(a), Box::new(self.pow()?)));
        }
        Ok(a)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if self.peek() == Some("-") {
            self.i += 1;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.atom()
    }
    fn atom(&mut self) -> Result<Expr, String> {
        let t = self.peek().ok_or("expression ends too early")?.to_string();
        self.i += 1;
        if t == "(" {
            let e = self.cmp()?;
            if self.peek() != Some(")") {
                return Err("missing ')'".into());
            }
            self.i += 1;
            return Ok(e);
        }
        if let Ok(v) = t.parse::<f64>() {
            return Ok(Expr::Num(v));
        }
        if t.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') {
            if self.peek() == Some("(") {
                self.i += 1;
                let mut args = Vec::new();
                if self.peek() != Some(")") {
                    loop {
                        args.push(self.cmp()?);
                        match self.peek() {
                            Some(",") => self.i += 1,
                            Some(")") => break,
                            _ => return Err(format!("{t}(): expected ',' or ')'")),
                        }
                    }
                }
                self.i += 1;
                return Ok(Expr::Call(t, args));
            }
            return Ok(Expr::Var(t));
        }
        Err(format!("unexpected '{t}'"))
    }
}

// ───────────────────────────── FX chains ─────────────────────────────

/// One effect of a track's chain.
pub enum Fx {
    Proc(Box<dyn Processor>),
    /// Reverb as a wet/dry insert (cached IR convolution).
    Reverb(dsp::ReverbParams, f32, f32),
    /// Gated reverb (the 80s snare): the tail is cut `hold` seconds after each hit.
    Gated { p: dsp::ReverbParams, wet: f32, hold: f32, release: f32 },
}

/// Effect types for `fx`.
pub const FX_TYPES: &[&str] = &["eq", "comp", "delay", "reverb", "chorus", "saturate", "widener", "transient", "filter", "gain", "autopan", "gated-reverb"];

/// Parse a track's `fx` list (each `{"type": …, …params}`).
pub fn fx_chain(list: &Value, rate: u32, bpm: f64, at: &str) -> Result<Vec<Fx>, String> {
    let arr = list.as_array().ok_or_else(|| format!("{at}: a list like [{{\"type\":\"delay\",\"note\":\"1/8d\",\"wet\":0.2}}]"))?;
    let fs = rate as f32;
    let mut out = Vec::new();
    for (i, f) in arr.iter().enumerate() {
        let ap = format!("{at}[{i}]");
        let mut o = f.as_object().cloned().ok_or_else(|| format!("{ap}: an object with \"type\""))?;
        let ty = o.remove("type").and_then(|t| t.as_str().map(str::to_string)).ok_or_else(|| format!("{ap}: missing \"type\" ({})", FX_TYPES.join(" ")))?;
        let v = Value::Object(o);
        let de = |e: serde_json::Error| format!("{ap} ({ty}): {e}");
        out.push(match ty.as_str() {
            "eq" => Fx::Proc(Box::new(dsp::Eq::new(fs, &serde_json::from_value(v).map_err(de)?))),
            "comp" | "compressor" => Fx::Proc(Box::new(dsp::Compressor::new(fs, &serde_json::from_value(v).map_err(de)?))),
            "delay" => {
                let mut p: dsp::DelayParams = serde_json::from_value(v).map_err(de)?;
                p.bpm = bpm as f32;
                Fx::Proc(Box::new(dsp::Delay::new(fs, &p)))
            }
            "chorus" => Fx::Proc(Box::new(dsp::Chorus::new(fs, &serde_json::from_value(v).map_err(de)?))),
            "saturate" | "drive" => Fx::Proc(Box::new(dsp::Saturator::new(fs, &serde_json::from_value(v).map_err(de)?))),
            "widener" | "width" => Fx::Proc(Box::new(dsp::Widener::new(fs, &serde_json::from_value(v).map_err(de)?))),
            "transient" => Fx::Proc(Box::new(dsp::TransientShaper::new(fs, &serde_json::from_value(v).map_err(de)?))),
            "reverb" => {
                let mut p: dsp::ReverbParams = serde_json::from_value(v).map_err(de)?;
                if let Some(pr) = &p.preset {
                    if !dsp::REVERB_PRESETS.contains(&pr.as_str()) {
                        return Err(format!("{ap}: reverb preset '{pr}' ({})", dsp::REVERB_PRESETS.join(" ")));
                    }
                }
                let (wet, dry) = (p.wet.unwrap_or(0.25), p.dry.unwrap_or(1.0));
                p.wet = Some(1.0);
                p.dry = Some(0.0);
                Fx::Reverb(p, wet, dry)
            }
            "gated-reverb" | "gated" => {
                let fo = v.as_object().unwrap();
                check_keys(fo, &["preset", "wet", "hold", "release"], &ap)?;
                let preset = fo.get("preset").and_then(Value::as_str).unwrap_or("plate");
                if !dsp::REVERB_PRESETS.contains(&preset) {
                    return Err(format!("{ap}: reverb preset '{preset}' ({})", dsp::REVERB_PRESETS.join(" ")));
                }
                let p: dsp::ReverbParams = serde_json::from_value(serde_json::json!({ "preset": preset, "wet": 1.0, "dry": 0.0 })).map_err(de)?;
                let wet = fo.get("wet").map(|c| num(c, &ap)).unwrap_or(Ok(0.5))?;
                let hold = fo.get("hold").map(|c| num(c, &ap)).unwrap_or(Ok(0.25))?.clamp(0.02, 2.0);
                let release = fo.get("release").map(|c| num(c, &ap)).unwrap_or(Ok(0.03))?.clamp(0.002, 1.0);
                Fx::Gated { p, wet, hold, release }
            }
            "filter" => {
                let fo = v.as_object().unwrap();
                check_keys(fo, &["mode", "cutoff", "q"], &ap)?;
                let mode = match fo.get("mode").and_then(Value::as_str).unwrap_or("lp") {
                    "lp" | "lowpass" => dsp::SvfMode::LowPass,
                    "hp" | "highpass" => dsp::SvfMode::HighPass,
                    "bp" | "bandpass" => dsp::SvfMode::BandPass,
                    "notch" => dsp::SvfMode::Notch,
                    o => return Err(format!("{ap}.mode: '{o}' (lp hp bp notch)")),
                };
                let cut = fo.get("cutoff").map(|c| num(c, &ap)).unwrap_or(Ok(1000.0))?;
                let q = fo.get("q").map(|c| num(c, &ap)).unwrap_or(Ok(0.707))?;
                Fx::Proc(Box::new(SvfFx(dsp::Svf::new(fs, mode, cut, q))))
            }
            "autopan" | "tremolo" => {
                // Stereo tremolo (the suitcase electric-piano vibrato): an equal-power
                // pan swinging at `rate` Hz or a note value (`"note":"1/8"`), `depth` 0–1.
                let fo = v.as_object().unwrap();
                check_keys(fo, &["rate", "note", "depth"], &ap)?;
                let hz = match (fo.get("note").and_then(Value::as_str), fo.get("rate")) {
                    (Some(n), _) => 1.0 / dsp::note_seconds(n, bpm as f32).map_err(|e| format!("{ap}.note: {e}"))?,
                    (None, Some(r)) => num(r, &ap)?,
                    (None, None) => 1.0 / dsp::note_seconds("1/8", bpm as f32)?,
                };
                let depth = fo.get("depth").map(|c| num(c, &ap)).unwrap_or(Ok(0.5))?.clamp(0.0, 1.0);
                if !(0.01..=40.0).contains(&hz) {
                    return Err(format!("{ap}: rate {hz:.2} Hz is out of range (0.01–40)"));
                }
                Fx::Proc(Box::new(AutoPan { step: std::f64::consts::TAU * hz as f64 / rate as f64, phase: 0.0, depth }))
            }
            "gain" => {
                let db = v.get("db").map(|c| num(c, &ap)).unwrap_or(Ok(0.0))?;
                Fx::Proc(Box::new(GainFx(10f32.powf(db / 20.0))))
            }
            o => {
                let hint = closest(o, FX_TYPES).map(|b| format!(" — did you mean '{b}'?")).unwrap_or_default();
                return Err(format!("{ap}: unknown effect '{o}'{hint} ({})", FX_TYPES.join(" ")));
            }
        });
    }
    Ok(out)
}

struct SvfFx(dsp::Svf);
impl Processor for SvfFx {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            *a = self.0.tick(0, *a);
            *b = self.0.tick(1, *b);
        }
    }
    fn reset(&mut self) {}
}

struct AutoPan {
    step: f64,
    phase: f64,
    depth: f32,
}
impl Processor for AutoPan {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            // θ = π/4 · (1 + depth·sin φ): centre at φ = 0, √2 so the centre is unity.
            let th = std::f32::consts::FRAC_PI_4 * (1.0 + self.depth * (self.phase as f32).sin());
            *a *= std::f32::consts::SQRT_2 * th.cos();
            *b *= std::f32::consts::SQRT_2 * th.sin();
            self.phase += self.step;
            if self.phase > std::f64::consts::TAU {
                self.phase -= std::f64::consts::TAU;
            }
        }
    }
    fn reset(&mut self) {
        self.phase = 0.0;
    }
}

struct GainFx(f32);
impl Processor for GainFx {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        l.iter_mut().chain(r.iter_mut()).for_each(|x| *x *= self.0);
    }
    fn reset(&mut self) {}
}

/// Run a chain over a whole track (latency compensated).
pub fn apply_chain(chain: &mut [Fx], l: &mut [f32], r: &mut [f32], rate: u32) {
    for f in chain.iter_mut() {
        match f {
            Fx::Proc(p) => dsp::process_aligned(p.as_mut(), l, r),
            Fx::Reverb(p, wet, dry) => {
                let ir = crate::mix::reverb_ir(rate, p);
                let n = l.len();
                let ((a, b), (c, d)) = rayon::join(|| dsp::convolve(l, l, &ir.0), || dsp::convolve(r, r, &ir.1));
                for i in 0..n {
                    l[i] = l[i] * *dry + *wet * (a[i] + c[i]);
                    r[i] = r[i] * *dry + *wet * (b[i] + d[i]);
                }
            }
            Fx::Gated { p, wet, hold, release } => {
                let ir = crate::mix::reverb_ir(rate, p);
                let n = l.len();
                let ((a, b), (c, d)) = rayon::join(|| dsp::convolve(l, l, &ir.0), || dsp::convolve(r, r, &ir.1));
                // The gate opens on every hit (dry above a third of the track's peak),
                // stays open for `hold`, then closes in `release`.
                let peak = l.iter().chain(r.iter()).fold(0f32, |m, x| m.max(x.abs()));
                let thr = peak / 3.0;
                let hold_n = (*hold * rate as f32) as usize;
                let k = (-1.0 / (*release * rate as f32)).exp();
                let (mut left, mut g) = (0usize, 0f32);
                for i in 0..n {
                    if l[i].abs().max(r[i].abs()) > thr && thr > 0.0 {
                        left = hold_n;
                    }
                    if left > 0 {
                        left -= 1;
                        g = 1.0;
                    } else {
                        g *= k;
                    }
                    l[i] += *wet * g * (a[i] + c[i]);
                    r[i] += *wet * g * (b[i] + d[i]);
                }
            }
        }
    }
}

/// Continuous track automation: `volume` (dB), `pan` (−1…1), `lp`/`hp` (Hz,
/// swept filters), and the sends `reverb`/`delay` (returned as envelopes).
pub struct TrackAuto {
    pub volume: Option<Automation>,
    pub pan: Option<Automation>,
    pub lp: Option<Automation>,
    pub hp: Option<Automation>,
    pub reverb: Option<Automation>,
    pub delay: Option<Automation>,
    /// `synth.<param>` → applied per note at note-on.
    pub per_note: Vec<(String, Automation)>,
}

pub const AUTO_KEYS: &[&str] = &["volume", "pan", "lp", "hp", "reverb", "delay", "synth.<param>"];

impl TrackAuto {
    pub fn parse(v: &Value, grid: &Grid, at: &str) -> Result<TrackAuto, String> {
        let o = v.as_object().ok_or_else(|| format!("{at}: an object like {{\"lp\": [[0, 400], [16, 8000]], \"pan\": \"=0.5*sin(beat)\"}}"))?;
        let mut a = TrackAuto { volume: None, pan: None, lp: None, hp: None, reverb: None, delay: None, per_note: Vec::new() };
        for (k, v) in o {
            let ap = format!("{at}.{k}");
            let auto = Automation::parse(v, grid, &ap)?;
            match k.as_str() {
                "volume" | "gain" => a.volume = Some(auto),
                "pan" => a.pan = Some(auto),
                "lp" | "cutoff" => a.lp = Some(auto),
                "hp" => a.hp = Some(auto),
                "reverb" => a.reverb = Some(auto),
                "delay" => a.delay = Some(auto),
                k if k.starts_with("synth.") => {
                    let name = &k[6..];
                    set_patch_param(&mut Patch::base(), name, 1.0).map_err(|e| format!("{ap}: {e}"))?;
                    a.per_note.push((name.to_string(), auto));
                }
                other => {
                    let hint = closest(other, &["volume", "pan", "lp", "hp", "reverb", "delay"]).map(|b| format!(" — did you mean `{b}`?")).unwrap_or_default();
                    return Err(format!("{ap}: unknown automation target `{other}`{hint} ({})", AUTO_KEYS.join(" ")));
                }
            }
        }
        Ok(a)
    }

    /// Apply volume, pan and the swept filters to a track in place.
    pub fn apply(&self, l: &mut [f32], r: &mut [f32], rate: u32) {
        let n = l.len();
        if let Some(v) = &self.volume {
            let e = v.render(n, rate);
            for i in 0..n {
                let g = 10f32.powf(e[i] / 20.0);
                l[i] *= g;
                r[i] *= g;
            }
        }
        for (auto, mode) in [(&self.lp, dsp::SvfMode::LowPass), (&self.hp, dsp::SvfMode::HighPass)] {
            if let Some(a) = auto {
                let e = a.render(n, rate);
                let mut f = dsp::Svf::new(rate as f32, mode, e.first().copied().unwrap_or(1000.0).clamp(20.0, 20_000.0), 0.707);
                for (k, (cl, cr)) in l.chunks_mut(32).zip(r.chunks_mut(32)).enumerate() {
                    f.set(e[k * 32].clamp(20.0, 20_000.0), 0.707);
                    for (x, y) in cl.iter_mut().zip(cr.iter_mut()) {
                        *x = f.tick(0, *x);
                        *y = f.tick(1, *y);
                    }
                }
            }
        }
        if let Some(p) = &self.pan {
            let e = p.render(n, rate);
            let s = std::f32::consts::SQRT_2;
            for i in 0..n {
                let (gl, gr) = crate::voice::pan_gains(e[i]);
                l[i] *= gl * s;
                r[i] *= gr * s;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gated_reverb_cuts_the_tail_after_hold() {
        let n = 48_000;
        let hit = |x: &mut Vec<f32>| {
            for (i, v) in x.iter_mut().enumerate().take(480) {
                *v = (i as f32 * 0.37).sin() * (1.0 - i as f32 / 480.0);
            }
        };
        let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
        hit(&mut l);
        hit(&mut r);
        let (l0, r0) = (l.clone(), r.clone());
        let mut plain = fx_chain(&serde_json::json!([{ "type": "reverb", "preset": "plate", "wet": 0.7, "dry": 1.0 }]), 48_000, 120.0, "fx").unwrap();
        let mut gated = fx_chain(&serde_json::json!([{ "type": "gated-reverb", "preset": "plate", "wet": 0.7, "hold": 0.2, "release": 0.02 }]), 48_000, 120.0, "fx").unwrap();
        let (mut pl, mut pr) = (l0.clone(), r0.clone());
        apply_chain(&mut plain, &mut pl, &mut pr, 48_000);
        apply_chain(&mut gated, &mut l, &mut r, 48_000);
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        // Inside the hold the tail is the plate's; 150 ms after the hold it is gone.
        let (a, b) = (rms(&l[4_800..9_000]), rms(&pl[4_800..9_000]));
        assert!(a > 0.01 && (a / b - 1.0).abs() < 0.05, "{a} vs {b}");
        let (c, d) = (rms(&l[16_800..24_000]), rms(&pl[16_800..24_000]));
        assert!(c < d * 0.01 && d > 0.002, "gated {c} vs plain {d}");
    }

    #[test]
    fn autopan_swings_a_mono_source_at_the_note_rate() {
        let mut chain = fx_chain(&serde_json::json!([{ "type": "autopan", "note": "1/4", "depth": 0.5 }]), 48_000, 120.0, "fx").unwrap();
        let (mut l, mut r) = (vec![0.5f32; 48_000], vec![0.5f32; 48_000]);
        apply_chain(&mut chain, &mut l, &mut r, 48_000);
        // 1/4 at 120 bpm = 2 Hz: right-heavy at 1/8 cycle, left-heavy at 3/8, centred at 1/4.
        assert!(r[6_000] > l[6_000] * 1.5 && l[18_000] > r[18_000] * 1.5, "{} {} / {} {}", l[6_000], r[6_000], l[18_000], r[18_000]);
        assert!((l[12_000] - r[12_000]).abs() < 1e-3);
        // Equal power: L² + R² stays at the input's.
        for i in (0..48_000).step_by(997) {
            assert!(((l[i] * l[i] + r[i] * r[i]) - 0.5).abs() < 1e-4);
        }
        assert!(fx_chain(&serde_json::json!([{ "type": "autopan", "rate": 100 }]), 48_000, 120.0, "fx").is_err());
    }

    #[test]
    fn expressions() {
        let e = Expr::parse("0.5 + 0.5*sin(beat*pi/2) + max(1, 2)^2 - -1").unwrap();
        let v = e.eval(&|n| (n == "beat").then_some(1.0));
        assert!((v - (0.5 + 0.5 + 4.0 + 1.0)).abs() < 1e-12, "{v}");
        assert!(Expr::parse("sin(beat").is_err());
        assert!(Expr::parse("foo(1)").unwrap_err().contains("unknown function"));
        assert!(Expr::parse("beats").unwrap_err().contains("unknown variable"));
        assert_eq!(Expr::parse("tri(0.25)").unwrap().eval(&|_| None), 0.5);
    }

    #[test]
    fn keyframes_interpolate_in_musical_time() {
        let g = Grid { bpm: 120.0, ..Default::default() };
        let a = Automation::parse(&serde_json::json!([[0, 100], ["@bar2", 300]]), &g, "x").unwrap();
        assert_eq!(a.at(0.0), 100.0);
        assert_eq!(a.at(1.0), 200.0); // half-way to bar 2 (2 s)
        assert_eq!(a.at(5.0), 300.0);
        let e = Automation::parse(&serde_json::json!("=bar*10"), &g, "x").unwrap();
        assert!((e.at(4.0) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn custom_synth_and_errors() {
        let def = serde_json::json!({"synth": {"osc": [{"wave": "square", "voices": 2, "detune": 8}, {"wave": "sine", "semis": -12, "level": 0.5}],
            "filter": {"type": "lp4", "cutoff": 900, "res": 0.4, "env": 2}, "amp": [0.01, 0.2, 0.6, 0.3], "lfo": {"rate": 5, "vib": 6}}});
        let inst = build("lead", &def, None, Vec::new()).unwrap();
        let mut v = inst.voice(&NoteOn { key: 69.0, vel: 0.8, rate: 48_000.0, seed: 1, rr: 0, len: 1.0 });
        let (mut l, mut r) = (vec![0.0f32; 4800], vec![0.0f32; 4800]);
        for (a, b) in l.chunks_mut(128).zip(r.chunks_mut(128)) {
            v.render(a, b);
        }
        let peak = l.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.01 && peak < 1.5, "{peak}");
        let e = build("x", &serde_json::json!({"synth": {"osc": [{"wave": "sawtooth"}]}}), None, Vec::new()).err().unwrap();
        assert!(e.contains("sawtooth"), "{e}");
        let e = build("x", &serde_json::json!({"synth": {"filtr": {}}}), None, Vec::new()).err().unwrap();
        assert!(e.contains("did you mean `filter`"), "{e}");
        let s = sampler_sfz(&serde_json::json!({"zones": [{"file": "k.wav", "key": 36}, {"file": "c4.wav", "root": "C4", "lo": 48, "hi": 72, "vel": [1, 90]}]}), "s").unwrap();
        assert!(s.contains("key=36") && s.contains("pitch_keycenter=60") && s.contains("lovel=1 hivel=90"), "{s}");
        assert!(fx_chain(&serde_json::json!([{"type": "delya"}]), 48_000, 120.0, "fx").err().unwrap().contains("did you mean 'delay'"));
    }
}
