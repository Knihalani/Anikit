//! `anikit-video`: the command-line renderer.
//!
//!   anikit-video render spec.json -o out.mp4 [--quality draft|good|best] [--from S --to S] [--scale K] [--codec C]
//!   anikit-video still  spec.json --at 2.5 -o frame.png [--scale K] [--watch]
//!   anikit-video sheet  spec.json -o contact.png [--cols 4 --rows 3 --width 480]
//!   anikit-video check  spec.json
//!   anikit-video info   [--components]
//!   anikit-video schema [-o spec.schema.json]
//!
//! Add `--json` for machine-readable output on stdout. Progress goes to stderr.

use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::{self, Codec, FfmpegEncoder, Quality, RenderOptions};
use anikit_video::spec::{self, Diagnostic, Severity};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

const USAGE: &str = "usage: anikit-video <render|still|sheet|check|info|schema> [spec.json|spec.yaml] [options]
  render spec.json -o out.mp4   [--quality draft|good|best] [--codec ak264|h264|h265|vp9|av1|prores|gif|png]
                                [--from S] [--to S] [--scale K] [--no-audio]
  still  spec.json -o f.png     [--at S | --at @scene] [--scale K | --width W] [--watch]
  sheet  spec.json -o sheet.png [--frames N | --cols 4 --rows 3] [--width 480]
  check  spec.json              validate; print errors, warnings and timing
  info   [--components]         version, encoders, component catalogue
  schema [-o file]              print the JSON Schema of the spec
  beats  music.wav              tempo, beats and bars (use them as \"@beat17\", \"@bar5\")
common: --json (machine-readable output)
agents: new | templates | prompt | critique | preview | mcp   (anikit-video templates, anikit-video prompt)";

struct Args {
    cmd: String,
    input: Option<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse() -> Result<Args, String> {
        let mut it = std::env::args().skip(1).peekable();
        let cmd = it.next().ok_or(USAGE)?;
        let mut input = None;
        let mut flags = Vec::new();
        const BOOL: &[&str] = &["--json", "--watch", "--no-audio", "--components", "--quiet", "--profile"];
        while let Some(a) = it.next() {
            if a.starts_with('-') && a.len() > 1 {
                let (k, v) = match a.split_once('=') {
                    Some((k, v)) => (k.to_string(), Some(v.to_string())),
                    None if BOOL.contains(&a.as_str()) => (a.clone(), None),
                    None => (a.clone(), it.next()),
                };
                flags.push((k, v));
            } else if input.is_none() {
                input = Some(a);
            } else {
                return Err(format!("unexpected argument '{a}'"));
            }
        }
        Ok(Args { cmd, input, flags })
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.flags.iter().rev().find(|(n, _)| n == k).and_then(|(_, v)| v.as_deref())
    }
    fn has(&self, k: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == k)
    }
    fn num(&self, k: &str) -> Result<Option<f32>, String> {
        self.get(k).map(|v| v.parse::<f32>().map_err(|_| format!("{k}: expected a number, got '{v}'"))).transpose()
    }
}

fn load(path: &str) -> Result<(spec::Parsed, PathBuf), Vec<Diagnostic>> {
    let src = if path == "-" {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).map_err(|e| vec![Diagnostic::error("", e.to_string())])?;
        s
    } else {
        std::fs::read_to_string(path).map_err(|e| vec![Diagnostic::error("", format!("can't read {path}: {e}"))])?
    };
    let low = path.to_ascii_lowercase();
    let parsed = if low.ends_with(".yaml") || low.ends_with(".yml") {
        spec::parse_yaml(&src)
    } else if low.ends_with(".json") {
        spec::parse_json(&src)
    } else {
        spec::parse_str(&src)
    }?;
    let base = Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let base = if base.as_os_str().is_empty() { PathBuf::from(".") } else { base };
    Ok((parsed, base))
}

fn options(spec: &spec::Spec, base: &Path, load_assets: bool) -> (Options, Vec<Diagnostic>) {
    let fonts = anikit_video::text::FontStore::new();
    let mut d = Vec::new();
    if let Some(f) = &spec.fonts {
        for (name, file) in f {
            if let Err(e) = fonts.add_font_file(base.join(file)) {
                d.push(Diagnostic::error(format!("fonts.{name}"), format!("can't load {file}: {e}")));
            }
        }
    }
    (Options { base_dir: base.to_path_buf(), fonts: Some(Arc::new(fonts)), load_assets, assets: None }, d)
}

/// Parse + compile; prints diagnostics. Returns the evaluator or exits with errors.
fn build(path: &str, json: bool, load_assets: bool) -> Result<(Evaluator, Vec<Diagnostic>), ExitCode> {
    let fail = |d: Vec<Diagnostic>| {
        report(&d, json, None);
        ExitCode::from(1)
    };
    let (parsed, base) = load(path).map_err(fail)?;
    let (opts, mut d) = options(&parsed.spec, &base, load_assets);
    d.extend(parsed.warnings);
    let (ev, more) = Evaluator::build(&parsed.spec, &opts);
    d.extend(more);
    if d.iter().any(|x| x.severity == Severity::Error) || ev.is_none() {
        return Err(fail(d));
    }
    Ok((ev.unwrap(), d))
}

fn report(d: &[Diagnostic], json: bool, ev: Option<&Evaluator>) {
    if json {
        // Compact (token-efficient): one string per issue, "path: message, did you mean 'x'?".
        let line = |x: &Diagnostic| {
            let mut s = if x.path.is_empty() { x.message.clone() } else { format!("{}: {}", x.path, x.message) };
            if let Some(g) = &x.suggestion {
                s += &format!(", did you mean '{g}'?");
            }
            s
        };
        let errors: Vec<String> = d.iter().filter(|x| x.severity == Severity::Error).map(line).collect();
        let warnings: Vec<String> = d.iter().filter(|x| x.severity == Severity::Warning).map(line).collect();
        let mut v = serde_json::json!({ "ok": errors.is_empty(), "errors": errors, "warnings": warnings });
        if let Some(ev) = ev {
            v["width"] = ev.width.into();
            v["height"] = ev.height.into();
            v["fps"] = ev.fps.into();
            let r = |x: f32| ((x as f64) * 1000.0).round() / 1000.0;
            v["duration"] = r(ev.duration).into();
            v["frames"] = ev.frames().into();
            v["scenes"] = ev.scenes().iter().map(|s| serde_json::json!({ "id": s.id, "start": r(s.start), "end": r(s.end) })).collect();
        }
        println!("{v}");
    } else {
        for x in d {
            eprintln!("{x}");
        }
    }
}

fn warn(d: &[Diagnostic], json: bool) {
    if !json {
        for x in d {
            eprintln!("{x}");
        }
    }
}

fn resolve_time(ev: &Evaluator, s: &str) -> Result<f32, String> {
    if let Some(id) = s.strip_prefix('@') {
        return ev.scene_start(id).map(|t| t + 0.75_f32.min(ev.duration)).ok_or_else(|| format!("no scene '{id}'"));
    }
    if let Some(r) = s.strip_suffix('f') {
        // Exactly the render path's frame time, so a still matches the video frame bit for bit.
        return r.trim().parse::<u32>().map(|i| pipeline::frame_time(ev, i)).map_err(|_| format!("bad frame '{s}'"));
    }
    s.strip_suffix('s').unwrap_or(s).parse::<f32>().map_err(|_| format!("bad time '{s}'"))
}

fn main() -> ExitCode {
    // Agent commands (new, templates, prompt, critique, preview, mcp) live in the library.
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = anikit_video::agent_cli::run(&raw) {
        return ExitCode::from(code as u8);
    }
    let args = match Args::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let json = args.has("--json");
    match run(&args, json) {
        Ok(c) => c,
        Err(e) => {
            if json {
                println!("{}", serde_json::json!({ "ok": false, "error": e }));
            } else {
                eprintln!("error: {e}");
            }
            ExitCode::from(1)
        }
    }
}

fn need_input(a: &Args) -> Result<&str, String> {
    a.input.as_deref().ok_or_else(|| format!("missing spec file\n{USAGE}"))
}

fn run(a: &Args, json: bool) -> Result<ExitCode, String> {
    match a.cmd.as_str() {
        "schema" => {
            let s = spec::schema_json();
            match a.get("-o") {
                Some(o) => std::fs::write(o, s + "\n").map_err(|e| e.to_string())?,
                None => println!("{s}"),
            }
            Ok(ExitCode::SUCCESS)
        }
        "info" => {
            let kinds: Vec<_> = anikit_video::components::KINDS.iter().map(|k| serde_json::json!({ "name": k.name, "summary": k.summary, "example": serde_json::from_str::<serde_json::Value>(k.example).unwrap_or_default() })).collect();
            let ff = std::process::Command::new("ffmpeg").arg("-version").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.lines().next().map(str::to_string));
            if json {
                println!("{}", serde_json::json!({ "version": env!("CARGO_PKG_VERSION"), "threads": rayon::current_num_threads(), "ffmpeg": ff, "codecs": ["h264","h265","vp9","av1","prores","gif","png"], "components": kinds }));
            } else {
                println!("anikit-video {}  ({} threads)", env!("CARGO_PKG_VERSION"), rayon::current_num_threads());
                println!("ffmpeg: {}", ff.unwrap_or_else(|| "not found".into()));
                println!("codecs: h264 h265 vp9 av1 prores gif png");
                if a.has("--components") {
                    for k in anikit_video::components::KINDS {
                        println!("  {:<14} {}\n  {:<14} e.g. {}", k.name, k.summary, "", k.example);
                    }
                } else {
                    println!("components: {} (see --components)", kinds.len());
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        "check" => {
            let path = need_input(a)?;
            let t = Instant::now();
            let (ev, d) = match build(path, json, false) {
                Ok(x) => x,
                Err(c) => return Ok(c),
            };
            report(&d, json, Some(&ev));
            if !json {
                eprintln!(
                    "ok: {}×{} @ {} fps, {:.2} s ({} frames), {} scene(s): {}  [{} ms]",
                    ev.width,
                    ev.height,
                    ev.fps,
                    ev.duration,
                    ev.frames(),
                    ev.scenes().len(),
                    ev.scenes().iter().map(|s| format!("{} {:.2}–{:.2}", s.id, s.start, s.end)).collect::<Vec<_>>().join(", "),
                    t.elapsed().as_millis()
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        "still" => {
            let path = need_input(a)?;
            let out = PathBuf::from(a.get("-o").or(a.get("--out")).unwrap_or("still.png"));
            let scale = a.num("--scale")?.unwrap_or(1.0);
            let mut last = None;
            loop {
                let t0 = Instant::now();
                match build(path, json, true) {
                    Ok((ev, d)) => {
                        warn(&d, json);
                        let t = resolve_time(&ev, a.get("--at").unwrap_or("0"))?;
                        // `--width W` (as the MCP tools pass) sets the scale from the output width.
                        let scale = match a.num("--width")? {
                            Some(wd) if a.get("--scale").is_none() => (wd / ev.width as f32).clamp(0.05, 4.0),
                            _ => scale,
                        };
                        let (w, h, px) = pipeline::render_rgba(&ev, t, scale);
                        pipeline::save_png(&out, w, h, px)?;
                        let ms = t0.elapsed().as_secs_f32() * 1000.0;
                        if json {
                            println!("{}", serde_json::json!({ "ok": true, "output": out, "at": t, "width": w, "height": h, "ms": ms, "warnings": d }));
                        } else {
                            eprintln!("wrote {} ({w}×{h}, t = {t:.2} s) in {ms:.0} ms", out.display());
                        }
                    }
                    Err(c) => {
                        if !a.has("--watch") {
                            return Ok(c);
                        }
                    }
                }
                if !a.has("--watch") {
                    return Ok(ExitCode::SUCCESS);
                }
                // Poll for changes.
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let m = std::fs::metadata(path).and_then(|m| m.modified()).ok();
                    if last.is_none() {
                        last = m;
                    }
                    if m != last {
                        last = m;
                        break;
                    }
                }
            }
        }
        "beats" => {
            let path = a.input.as_deref().ok_or("missing audio file")?;
            let b = anikit_video::eval::beat_markers::refine(&anikit_video::audio::analyze_pcm(&anikit_video::audio::decode_file(path)?));
            if json {
                println!("{}", serde_json::json!({ "bpm": b.bpm, "beats": b.beats, "downbeats": b.downbeats }));
            } else {
                let f = |v: &[f64]| v.iter().map(|t| format!("{t:.3}")).collect::<Vec<_>>().join(" ");
                println!("bpm {:.1}\nbeats ({}): {}\nbars ({}): {}", b.bpm, b.beats.len(), f(&b.beats), b.downbeats.len(), f(&b.downbeats));
            }
            Ok(ExitCode::SUCCESS)
        }
        "sheet" => {
            let path = need_input(a)?;
            let out = PathBuf::from(a.get("-o").unwrap_or("sheet.png"));
            let (ev, d) = match build(path, json, true) {
                Ok(x) => x,
                Err(c) => return Ok(c),
            };
            warn(&d, json);
            // `--frames N` picks a near-square grid of N frames.
            let (dc, dr) = match a.num("--frames")? {
                Some(n) => {
                    let n = n.max(1.0);
                    let c = n.sqrt().ceil();
                    (c, (n / c).ceil())
                }
                None => (4.0, 3.0),
            };
            let cols = a.num("--cols")?.unwrap_or(dc) as u32;
            let rows = a.num("--rows")?.unwrap_or(dr) as u32;
            let width = a.num("--width")?.unwrap_or(480.0) as u32;
            let (w, h, px) = pipeline::contact_sheet(&ev, cols.max(1), rows.max(1), width.max(16));
            pipeline::save_png(&out, w, h, px)?;
            if json {
                println!("{}", serde_json::json!({ "ok": true, "output": out, "width": w, "height": h }));
            } else {
                eprintln!("wrote {} ({w}×{h}, {cols}×{rows} frames)", out.display());
            }
            Ok(ExitCode::SUCCESS)
        }
        "render" => {
            let path = need_input(a)?;
            let out = PathBuf::from(a.get("-o").or(a.get("--out")).unwrap_or("out.mp4"));
            let (ev, d) = match build(path, json, true) {
                Ok(x) => x,
                Err(c) => return Ok(c),
            };
            warn(&d, json);
            let quality = match a.get("--quality") {
                None => Quality::Good,
                Some(q) => Quality::parse(q).ok_or_else(|| format!("--quality: use draft, standard or high (got '{q}')"))?,
            };
            let codec = match a.get("--codec") {
                // Our encoder by default for MP4; x264 for `--quality best` (smallest files).
                None => match Codec::from_path(&out) {
                    Codec::H264 if quality != Quality::Best => Codec::Ak264,
                    c => c,
                },
                Some(c) => Codec::parse(c).ok_or_else(|| format!("--codec: unknown '{c}'"))?,
            };
            let from = a.get("--from").map(|s| resolve_time(&ev, s)).transpose()?;
            let to = a.get("--to").map(|s| resolve_time(&ev, s)).transpose()?;
            let scale = a.num("--scale")?.unwrap_or(if quality == Quality::Draft { 0.5 } else { 1.0 });
            let opts = RenderOptions { codec, quality, from, to, scale, progress: !json && !a.has("--quiet"), audio: !a.has("--no-audio"), profile: a.has("--profile") };
            let w = ((ev.width as f32 * scale).round() as u32).max(2);
            let h = ((ev.height as f32 * scale).round() as u32).max(2);
            let t0 = from.unwrap_or(0.0);
            let dur = to.unwrap_or(ev.duration) - t0;
            let wav = std::env::temp_dir().join(format!("anikit-video-{}.wav", std::process::id()));
            let audio = if opts.audio { pipeline::mix_audio(&ev, t0, dur, &wav)? } else { None };
            let s = if codec == Codec::Ak264 {
                pipeline::render_ak264(&ev, &opts, &out, audio.as_deref())
            } else {
                FfmpegEncoder::new(&out, w, h, ev.fps, codec, quality, audio.as_deref()).and_then(|mut enc| pipeline::render_video(&ev, &opts, &mut enc))
            };
            if audio.is_some() {
                let _ = std::fs::remove_file(&wav);
            }
            let s = s?;
            if json {
                println!("{}", serde_json::json!({ "ok": true, "summary": s, "warnings": d }));
            } else {
                eprintln!(
                    "wrote {} — {}×{} {} fps, {} frames ({} deduped), {:.2} s video in {:.2} s ({:.0} fps), {:.1} KB",
                    s.output,
                    s.width,
                    s.height,
                    s.fps,
                    s.frames,
                    s.deduped,
                    s.duration,
                    s.seconds,
                    s.render_fps,
                    s.bytes as f32 / 1024.0
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        c => Err(format!("unknown command '{c}'\n{USAGE}")),
    }
}
