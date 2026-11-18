//! Generated music in videos (the spec's `music` key) and the `music` / `docs
//! music` commands. The engine is the `anikit-music` crate; this module
//! places its mastered output on the video timeline, ducks it under
//! voice-over and exposes its exact beat grid to `@beatN` / `@barN` markers.

use crate::audio::{self, Duck, Role, Source, Track};
use anikit_music::{MusicSpec, RenderOpts};
use std::path::{Path, PathBuf};

/// A compiled `music` cue.
#[derive(Debug, Clone)]
pub struct MusicCue {
    pub spec: MusicSpec,
    pub base: PathBuf,
    /// Start on the video timeline (seconds).
    pub at: f64,
    /// Level in dB.
    pub gain: f64,
    /// Duck under voice-over by this many dB (None = never).
    pub duck: Option<f64>,
    /// Video length and voice-over flag the score was bound with (`onNotes`): the render
    /// uses them too, so the sound matches the pictures exactly.
    pub pinned: Option<(f64, bool)>,
}

/// Parse the spec's `music` value.
pub fn parse(v: &serde_json::Value, base: &Path) -> Result<MusicCue, String> {
    parse_with_cuts(v, base, &[])
}

/// Parse `music`, scoring to the video's scene cuts (video seconds) unless `"followCuts": false`.
pub fn parse_with_cuts(v: &serde_json::Value, base: &Path, cuts: &[f64]) -> Result<MusicCue, String> {
    let mut v = match v {
        serde_json::Value::String(s) => serde_json::json!({ "style": s }),
        v => v.clone(),
    };
    if let Some(o) = v.as_object_mut() {
        let follow = o.get("followCuts").and_then(|f| f.as_bool()).unwrap_or(true);
        if follow && !o.contains_key("cuts") && !cuts.is_empty() {
            let at = o.get("at").and_then(|a| a.as_f64()).unwrap_or(0.0);
            o.insert("cuts".into(), serde_json::json!(cuts.iter().map(|c| c - at).filter(|c| *c > 0.0).collect::<Vec<_>>()));
        }
    }
    parse_inner(&v, base)
}

fn parse_inner(v: &serde_json::Value, base: &Path) -> Result<MusicCue, String> {
    let v = match v {
        // Shorthand: "music": "lofi"
        serde_json::Value::String(s) => serde_json::json!({ "style": s }),
        v => v.clone(),
    };
    let spec = MusicSpec::from_json(&v.to_string())?;
    let duck = match &spec.duck {
        None | Some(serde_json::Value::Bool(true)) | Some(serde_json::Value::Null) => Some(10.0),
        Some(serde_json::Value::Bool(false)) => None,
        Some(serde_json::Value::Number(n)) => Some(n.as_f64().unwrap_or(10.0).abs()),
        Some(o) => return Err(format!("music.duck: true, false or dB (got {o})")),
    };
    // Validate everything that can fail now (style, key, chords, notes, instruments)
    // so `check` reports it, not the render.
    let opts = RenderOpts { video_length: Some(10.0), base_dir: Some(base.to_path_buf()), ..Default::default() };
    let a = anikit_music::arrange(&spec, &opts)?;
    for t in &a.tracks {
        if let Some(def) = spec.define.get(&t.inst) {
            anikit_music::custom::build(&t.inst, def, Some(base), Vec::new()).map_err(|e| format!("track '{}': {e}", t.name))?;
        } else {
            anikit_music::instruments::resolve(&t.inst, Some(base)).map_err(|e| format!("track '{}': {e}", t.name))?;
        }
        if let Some(fx) = &t.fx {
            anikit_music::custom::fx_chain(fx, 48_000, a.grid.bpm, &format!("track '{}'.fx", t.name))?;
        }
        if let Some(au) = &t.automate {
            anikit_music::custom::TrackAuto::parse(au, &a.grid, &format!("track '{}'.automate", t.name))?;
        }
    }
    Ok(MusicCue { at: spec.at.unwrap_or(0.0), gain: spec.gain.unwrap_or(0.0), duck, spec, base: base.to_path_buf(), pinned: None })
}

/// The exact time (video seconds) of `@beatN` / `@barN` (1-based).
pub fn marker(cue: &MusicCue, kind: &str, n: usize) -> Result<f64, String> {
    let g = anikit_music::beat_grid(&cue.spec)?;
    let unit = if kind == "bar" { g.beats_per_bar } else { 1.0 };
    Ok(cue.at + g.secs((n.max(1) - 1) as f64 * unit))
}

/// One note of the generated music, on the video timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreNote {
    /// Track name and role (`drums`, `bass`, `lead`, …).
    pub track: String,
    pub role: String,
    /// Video seconds of the note's start (on the grid, swing included; humanisation is ignored).
    pub t: f64,
    /// Length in seconds.
    pub dur: f64,
    /// MIDI key (GM drum key on drum tracks) and velocity 0..1.
    pub key: f32,
    pub vel: f32,
    /// Beat position from the music's start (fractional).
    pub beat: f64,
}

/// Every note the music will play for a video of `video_len` seconds, sorted by time.
/// It uses the same arrangement as [`render_track`], so pictures bound to it land on the sound.
pub fn score(cue: &MusicCue, video_len: f64, has_voice: bool) -> Result<Vec<ScoreNote>, String> {
    let opts = RenderOpts { video_length: Some((video_len - cue.at).max(0.1)), base_dir: Some(cue.base.clone()), rate: audio::DEFAULT_RATE, stems: false };
    let mut spec = cue.spec.clone();
    if has_voice && spec.underscore.is_none() {
        spec.underscore = Some(true);
    }
    let a = anikit_music::arrange(&spec, &opts)?;
    let mut out = Vec::new();
    for tr in &a.tracks {
        for n in &tr.notes {
            let (t0, t1) = if tr.straight { (a.grid.secs(n.t), a.grid.secs(n.t + n.len)) } else { (a.grid.swung(n.t), a.grid.swung(n.t + n.len)) };
            out.push(ScoreNote { track: tr.name.clone(), role: tr.role.clone(), t: cue.at + t0, dur: (t1 - t0).max(0.0), key: n.key, vel: n.vel, beat: n.t });
        }
    }
    out.sort_by(|x, y| x.t.total_cmp(&y.t).then(x.key.total_cmp(&y.key)));
    Ok(out)
}

/// Render the music for a video of `video_len` seconds into a float WAV at
/// `wav`, returning the track that places it (ducked under voice when the
/// video has voice-over).
pub fn render_track(cue: &MusicCue, video_len: f64, wav: &Path, has_voice: bool) -> Result<Track, String> {
    let (video_len, has_voice) = cue.pinned.unwrap_or((video_len, has_voice));
    let opts = RenderOpts { video_length: Some((video_len - cue.at).max(0.1)), base_dir: Some(cue.base.clone()), rate: audio::DEFAULT_RATE, stems: false };
    // Under voice-over, play the underscore mix (no melody) unless the spec says otherwise.
    let mut spec = cue.spec.clone();
    if has_voice && spec.underscore.is_none() {
        spec.underscore = Some(true);
    }
    let r = anikit_music::render(&spec, &opts)?;
    let pcm = audio::Pcm { rate: r.rate, channels: 2, samples: r.interleaved() };
    audio::write_wav(wav, &pcm, audio::WavFormat::Float32)?;
    Ok(Track {
        src: Source::File(wav.to_string_lossy().into_owned()),
        start: cue.at,
        db: cue.gain,
        role: Some(Role::Music),
        duck: match (has_voice, cue.duck) {
            (true, Some(db)) => Some(Duck { amount_db: db, ..Duck::default() }),
            _ => None,
        },
        ..Default::default()
    })
}

const DOCS: &str = r#"MUSIC — generated, mastered soundtracks (−14 LUFS, −1 dBTP), deterministic, 25–80× real time, rendered alongside the frames
In a video spec:   "music": {"style":"lofi","bpm":84,"key":"Dm","mood":"warm","hits":["@bar5"]}
  or just          "music": "synthwave"         (fills the video; @beatN/@barN follow its exact grid)
Standalone:        anikit-video music '{"style":"house","length":"15s"}' -o bed.wav [--stems DIR] [--midi out.mid] [--alts DIR: underscore, no-drums, 30s, 15s]
                   anikit-video music --style trap --bpm 140 --key Cm --length 20s -o out.ogg   (ogg/mp3/flac via ffmpeg)
                   anikit-video music styles | instruments | packs | sfx | fetch essentials|orchestra|all|<pack>

STYLES  played instruments: orchestral acoustic piano jazz rock corporate pop lofi · hybrid: cinematic-hybrid
        synth-led: house synthwave trap tech dnb ambient   (aliases: epic trailer edm chill 80s classical upbeat folk …)
CHOOSING (your call; see docs/video/taste/TASTE.md §7): pick from the film's concept, not habit.
  works: piano/acoustic/orchestral for warm, human, editorial or documentary films; band for energetic product launches;
         synth-led for tech, night, futurist or club-energy films; cinematic-hybrid for trailers and reveals (45 s+: three acts, climax → silence + false ending → bigger act III).
  avoid: a generic synth bed under a calm brand or people film; a busy beat under voice-over; four-on-the-floor under slow footage.
  "instrumentation": "acoustic|orchestral|band|piano|electronic|hybrid" keeps a style's groove and swaps its instruments,
  e.g. {"style":"house","instrumentation":"band"}; per-part {"instruments":{"lead":"violin"}} wins over both.
KEYS    instrumentation  bpm  key "Am" "F# dorian"  meter "6/8"  swing 0..1 (0.16 loose, 0.45 lofi, 0.667 triplet)  mood "dark epic"  length "15s"|"8 bars"  seed N
        sections ["intro:4","build:4","drop:8","outro:2"] or [{"name":"drop","bars":8,"energy":1,"stop":2}] (stop = beats of silence before it)
        hits ["@2.0","@bar9"] → impact + section change there; tempo nudged (±8%) so hits land on beats (fit:false to keep bpm)
        chords "vi IV I V" | "Am F C G"   instruments {"lead":"violin","drums":"drums:909"}   mute ["lead"]
        sfx [{"sfx":"riser","at":"@bar5"}]  (aligned by hit point: a riser PEAKS at "at")   end ring|fade|cut
        master {"lufs":-14,"ceiling":-1,"reverb":"hall"}   video-only: at (s), gain (dB), duck (dB under voice, default 10)
        underscore: true drops lead/countermelody (automatic under voice-over; false keeps the melody)
        scores to picture: scene cuts move section starts, get cymbal accents and swell into bigger sections (followCuts:false to stop)
TRACKS  (add to a style, or write everything yourself)
  {"inst":"piano","chords":"C Am F G","play":"broken"}        play: pad block stabs arp[-updown][/16] strum pulse/8 power broken x..x..x.
  {"inst":"bass","chords":"C Am F G","bass":"walk"}           bass: root half pulse octave walk house synco rock arp 808
  {"inst":"flute","chords":"C Am F G","melody":1.0}           generated melody, ~1 note per beat
  {"inst":"drums:808","drums":{"kick":"x...x...x...x...","hat":"x.x.x.x.x.x.x3x."},"bars":4}
                                                              x hit X accent o ghost . rest 2/3/4 roll; voices kick snare clap hat ohh rim tom crash ride shaker…
  {"inst":"strings","notes":"o4 l4 A4 C5 E5/2 | [A3 C4 E4]/1","at":3,"gain":-4,"reverb":0.4,"pan":-0.3,"duck":6}
NOTES   C4 Eb3 F#5 (C7 = octave 7) · C = note in the current octave (o5 C) · degrees 1-7 b3 5' 3,
        chords need a quality: Cmaj Am Cmaj7 Cdom7 G/B [C E G] or numerals vi IV V7 · r rest · _ hold
        /4 quarter /8 /16 /2 /1 /8t triplet . dotted :1.5 beats · ! accent ? ghost ^ staccato ~ legato
        o3 octave > < · l8 default length · v100 velocity · t+2 transpose · | bar line · @5 bar · (…)x4 repeat
MAKE ANYTHING (example: docs/video/music/custom.json)
  "define": {"wob": {"synth": {"base":"bass-synth"?, "osc":[{"wave":"saw|square|tri|sine|supersaw","voices":3,"detune":12,"semis":0,"level":0.8,"pw":0.5}],
             "sub":0.3, "noise":0, "filter":{"type":"ladder|lp|lp4|bp|formant","cutoff":800,"res":0.4,"drive":1.5,"env":2,"track":0.5},
             "amp":[a,d,s,r], "fenv":[a,d,s,r], "lfo":{"rate":5,"vib":8,"cutoff":0.5,"trem":0.2}, "pitch":[12,0.05], "glide":0.05, "mono":true}},
             "kit": {"sampler": {"zones":[{"file":"kick.wav","key":36},{"file":"c4.wav","root":"C4","lo":48,"hi":72,"vel":[1,90],"loop":[s,e]}],"release":0.3}}}
  then {"inst":"wob", …}.  Per track: "fx":[{"type":"eq|comp|delay|reverb|chorus|saturate|widener|transient|filter|gain|autopan|gated-reverb", …params}]
  "automate": {"volume"|"pan"|"lp"|"hp"|"reverb"|"delay"|"synth.cutoff|res|drive|env|vib|…": [[beat, v], ["@bar5", v]] or "=400+3000*saw(bar/4)"}
        expr vars beat bar t bpm; fns sin cos abs min max clamp mix saw tri sq step smooth exp log pow
INSTRUMENTS (sampled GM bank bundled; `music instruments` lists all) piano epiano rhodes organ strings violin cello
        choir trumpet french-horn brass flute oboe clarinet sax harp timpani nylon-guitar steel-guitar overdrive-guitar
        bass pick-bass fretless · models: marimba vibraphone kalimba glockenspiel celesta tubular-bells church-bell handpan
        · synths: pad-warm pad-glass lead-saw pluck bell-fm epiano-fm supersaw bass-808 bass-reese bass-acid keys-lofi
        · kits: drums:808 909 trap lofi house techno cinematic, drums (GM) drums:jazz brush orchestra
        · files: "my.sfz", "font.sf2#0"   · fallbacks: "salamander-piano|piano"
        · deeper samples: `music fetch essentials` (≈210 MB: grand, e-pianos, bass, guitars), `fetch orchestra` (≈930 MB: string, brass, wind
          sections), `fetch virtuosity` (real kit); `music packs` lists all. Common names then use them (credit Greg Sullivan, CC BY 3.0, for e-pianos)
EXAMPLES
  {"music":{"style":"cinematic-hybrid","key":"Dm","hits":["@bar5"],"sfx":[{"sfx":"braam","at":"@bar5"}]}}
  {"music":{"style":"lofi","mood":"chill","chords":"ii7 V7 Imaj7 vi7","instruments":{"lead":"kalimba"}}}
  {"music":{"bpm":96,"key":"G","tracks":[{"inst":"steel-guitar","chords":"G D Em C","play":"strum"},
            {"inst":"drums","drums":{"kick":"x.......x.......","snare":"....x.......x..."}}]}}
Errors name the field and suggest the closest word. Same spec + seed → bit-identical audio.
"#;

/// The alternate mixes `--alts` writes: (name, spec). Cutdowns only when the
/// piece is longer than them.
fn alt_mixes(v: &serde_json::Value, seconds: f64) -> Result<Vec<(&'static str, serde_json::Value)>, String> {
    let mut out = Vec::new();
    let mut u = v.clone();
    u["underscore"] = serde_json::json!(true);
    out.push(("underscore", u));
    let mut d = v.clone();
    let mut mute: Vec<serde_json::Value> = d.get("mute").and_then(|m| m.as_array().cloned()).unwrap_or_default();
    mute.push(serde_json::json!("drums"));
    d["mute"] = serde_json::Value::Array(mute);
    out.push(("no-drums", d));
    for (name, len) in [("30s", 30.0), ("15s", 15.0)] {
        if seconds > len + 1.0 {
            let mut c = v.clone();
            c["length"] = serde_json::json!(format!("{len}s"));
            if let Some(o) = c.as_object_mut() {
                o.remove("sections");
            }
            out.push((name, c));
        }
    }
    Ok(out)
}

/// `anikit-video docs music` text (≤ 60 lines).
pub fn docs() -> &'static str {
    DOCS
}

/// Handle `music …` and `docs music`. `None` when `args` is not ours.
pub fn run(args: &[String]) -> Option<i32> {
    match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        (Some("docs"), Some("music")) => {
            print!("{DOCS}");
            Some(0)
        }
        (Some("music"), _) => Some(match cmd_music(&args[1..]) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("anikit-video music: {e}");
                1
            }
        }),
        _ => None,
    }
}

fn cmd_music(args: &[String]) -> Result<(), String> {
    let mut input: Option<String> = None;
    let mut flags: std::collections::BTreeMap<String, String> = Default::default();
    let mut json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--json" {
            json = true;
        } else if let Some(k) = a.strip_prefix("--").or_else(|| a.strip_prefix('-').filter(|k| *k == "o")) {
            let (k, v) = match k.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (k.to_string(), it.next().cloned().ok_or_else(|| format!("--{k} needs a value"))?),
            };
            flags.insert(if k == "o" { "out".into() } else { k }, v);
        } else if input.is_none() {
            input = Some(a.clone());
        } else if input.as_deref() == Some("fetch") && !flags.contains_key("pack") {
            flags.insert("pack".into(), a.clone());
        } else {
            return Err(format!("unexpected argument '{a}'"));
        }
    }
    match input.as_deref() {
        Some("styles") => {
            for s in anikit_music::style::STYLES {
                println!("{:<17} {:>3} bpm  {}", s.name, s.bpm, s.about);
            }
            return Ok(());
        }
        Some("instruments") => {
            for (group, names) in anikit_music::instruments::names() {
                println!("{group}: {}", names.join(" "));
            }
            return Ok(());
        }
        Some("packs") => {
            for p in anikit_music::sf2::packs::PACKS {
                let have = if anikit_music::sf2::packs::is_cached(p.name) { "cached" } else { "-" };
                println!("{:<17} {:<7} {:<36} {}  [{}]", p.name, have, p.size, p.description, p.license);
            }
            println!("groups: essentials ({}), orchestra ({}), all", anikit_music::sf2::packs::ESSENTIALS.join(" "), anikit_music::sf2::packs::ORCHESTRA.join(" "));
            return Ok(());
        }
        Some("sfx") => {
            println!("{}", anikit_music::sfx::SFX.join(" "));
            return Ok(());
        }
        Some("fetch") => {
            let pack = flags.get("pack").cloned().ok_or("fetch which pack? (essentials, orchestra, all, or a name from `anikit-video music packs`)")?;
            let p = anikit_music::sf2::packs::fetch(&pack)?;
            println!("{}", p.display());
            return Ok(());
        }
        _ => {}
    }
    // The spec: a file, inline JSON, or flags.
    let (mut v, base): (serde_json::Value, PathBuf) = match input.as_deref() {
        Some(s) if s.trim_start().starts_with('{') => (serde_json::from_str(s).map_err(|e| format!("inline JSON: {e}"))?, PathBuf::from(".")),
        Some(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| format!("can't read {p}: {e}"))?;
            let mut v: serde_json::Value = if p.ends_with(".yaml") || p.ends_with(".yml") {
                serde_norway::from_str(&text).map_err(|e| format!("{p}: {e}"))?
            } else {
                serde_json::from_str(&text).map_err(|e| format!("{p}: {e}"))?
            };
            // A video spec: take its `music` and the video length.
            if let Some(m) = v.get("music").cloned() {
                v = match m {
                    serde_json::Value::String(s) => serde_json::json!({ "style": s }),
                    m => m,
                };
            }
            (v, Path::new(p).parent().map(Path::to_path_buf).unwrap_or_default())
        }
        None => (serde_json::json!({}), PathBuf::from(".")),
    };
    for k in ["style", "key", "mood", "length", "meter", "chords"] {
        if let Some(x) = flags.get(k) {
            v[k] = serde_json::Value::String(x.clone());
        }
    }
    for k in ["bpm", "swing", "seed"] {
        if let Some(x) = flags.get(k) {
            v[k] = serde_json::json!(x.parse::<f64>().map_err(|_| format!("--{k} needs a number"))?);
        }
    }
    if let Some(s) = v.get("seed").and_then(|s| s.as_f64()) {
        v["seed"] = serde_json::json!(s as u64);
    }
    if v.as_object().is_none_or(|o| o.is_empty()) {
        return Err(format!("give a spec file, inline JSON or --style (try: anikit-video music --style lofi --length 20s -o lofi.wav)\n\n{DOCS}"));
    }
    let spec = MusicSpec::from_json(&v.to_string())?;
    let stems_dir = flags.get("stems").map(PathBuf::from);
    let opts = RenderOpts { video_length: None, base_dir: Some(base), rate: audio::DEFAULT_RATE, stems: stems_dir.is_some() };
    let r = anikit_music::render(&spec, &opts)?;
    let out = flags.get("out").cloned().unwrap_or_else(|| "music.wav".into());
    write_audio(Path::new(&out), r.rate, &r.l, &r.r)?;
    if let Some(dir) = &stems_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for s in &r.stems {
            write_audio(&dir.join(format!("{}.wav", s.name)), r.rate, &s.l, &s.r)?;
        }
    }
    if let Some(dir) = flags.get("alts").map(PathBuf::from) {
        // Alternate mixes for editors: the same piece under voice-over, without
        // drums, and cut down to 30 and 15 s (each with its own ending).
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let stem = Path::new(&out).file_stem().and_then(|s| s.to_str()).unwrap_or("music").to_string();
        for (name, path) in alt_mixes(&v, r.duration())? {
            let spec = MusicSpec::from_json(&path.to_string())?;
            let a = anikit_music::render(&spec, &RenderOpts { stems: false, ..opts.clone() })?;
            write_audio(&dir.join(format!("{stem}-{name}.wav")), a.rate, &a.l, &a.r)?;
        }
    }
    if let Some(m) = flags.get("midi") {
        let song = anikit_music::midi::MidiSong {
            bpm: r.arrangement.grid.bpm,
            beats_per_bar: r.arrangement.grid.beats_per_bar,
            tracks: r
                .arrangement
                .tracks
                .iter()
                .enumerate()
                .map(|(i, t)| anikit_music::midi::MidiTrack {
                    name: t.name.clone(),
                    channel: if t.role == "drums" { 9 } else { (i % 15) as u8 + u8::from(i % 15 >= 9) },
                    program: None,
                    notes: t.notes.clone(),
                })
                .collect(),
        };
        std::fs::write(m, anikit_music::midi::export(&song)).map_err(|e| format!("{m}: {e}"))?;
    }
    let a = &r.arrangement;
    let summary = serde_json::json!({
        "ok": true, "output": out, "seconds": r.duration(), "style": a.style, "bpm": a.grid.bpm, "key": format!("{} {}", anikit_music::theory::key_name(a.key.tonic, a.key.flats()).trim_end_matches(char::is_numeric).trim_end_matches('-'), a.key.scale.name),
        "sections": a.sections, "beats": a.beats().len(), "tracks": a.tracks.iter().map(|t| format!("{} ({})", t.name, t.inst)).collect::<Vec<_>>(),
        "report": r.report, "timings": r.timings,
    });
    if json {
        println!("{summary}");
    } else {
        let rep = &r.report;
        eprintln!(
            "wrote {out} — {:.1} s {} at {:.1} bpm, {} tracks, {} notes; {:.1} LUFS, {:.1} dBTP; rendered in {:.2} s ({:.0}× real time)",
            r.duration(),
            a.style,
            a.grid.bpm,
            a.tracks.len(),
            r.timings.notes,
            rep["lufs"].as_f64().unwrap_or(f64::NAN),
            rep["truePeak"].as_f64().unwrap_or(f64::NAN),
            r.timings.total,
            r.timings.realtime
        );
        let secs: Vec<String> = a.sections.iter().map(|s| format!("{} {}b@{:.1}s", s.name, s.bars, a.grid.secs(s.bar * a.grid.beats_per_bar))).collect();
        eprintln!("sections: {}", secs.join(" · "));
    }
    Ok(())
}

/// Write audio: `.wav` natively (24-bit), anything else (ogg, mp3, flac, m4a) through ffmpeg.
pub fn write_audio(path: &Path, rate: u32, l: &[f32], r: &[f32]) -> Result<(), String> {
    let mut inter = Vec::with_capacity(l.len() * 2);
    for (a, b) in l.iter().zip(r) {
        inter.push(*a);
        inter.push(*b);
    }
    let pcm = audio::Pcm { rate, channels: 2, samples: inter };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("wav").to_ascii_lowercase();
    if ext == "wav" {
        return audio::write_wav(path, &pcm, audio::WavFormat::Float32);
    }
    let tmp = path.with_extension(format!("tmp-{}.wav", std::process::id()));
    audio::write_wav(&tmp, &pcm, audio::WavFormat::Float32)?;
    let codec: &[&str] = match ext.as_str() {
        "ogg" | "oga" => &["-c:a", "libvorbis", "-q:a", "5"],
        "opus" => &["-c:a", "libopus", "-b:a", "128k"],
        "mp3" => &["-c:a", "libmp3lame", "-q:a", "2"],
        "flac" => &["-c:a", "flac"],
        "m4a" | "aac" => &["-c:a", "aac", "-b:a", "192k"],
        _ => &[],
    };
    let o = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&tmp)
        .args(codec)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .output();
    let _ = std::fs::remove_file(&tmp);
    let o = o.map_err(|e| format!("can't start ffmpeg to write .{ext} (write .wav instead): {e}"))?;
    if !o.status.success() {
        return Err(format!("ffmpeg: {}", String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alt_mixes_for_editors() {
        let v = serde_json::json!({ "style": "pop", "length": "40s", "mute": ["pad"], "sections": ["intro", "chorus"] });
        let alts = alt_mixes(&v, 40.0).unwrap();
        let names: Vec<&str> = alts.iter().map(|a| a.0).collect();
        assert_eq!(names, ["underscore", "no-drums", "30s", "15s"]);
        assert_eq!(alts[0].1["underscore"], true);
        assert_eq!(alts[1].1["mute"], serde_json::json!(["pad", "drums"]));
        assert_eq!(alts[2].1["length"], "30s");
        assert!(alts[2].1.get("sections").is_none());
        // A 20 s piece only gets the 15 s cutdown.
        assert_eq!(alt_mixes(&v, 20.0).unwrap().len(), 3);
        for (_, s) in &alts {
            MusicSpec::from_json(&s.to_string()).unwrap();
        }
    }
}
