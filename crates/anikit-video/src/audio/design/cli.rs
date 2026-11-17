//! `anikit-video sound …`:
//!
//! ```text
//! sound list                              every sound name
//! sound render glass [-o x.wav] [k=v …]   one sound (params: pitch=3 intensity=0.8 …)
//! sound plan spec.json [--sound JSON]     the cues the picture produces (time, sound, why)
//! sound mix spec.json -o out.wav [--sound JSON]   the soundtrack only (no frames)
//! sound packs | sound fetch <pack>        CC0 sample packs
//! ```

use super::{pack, render, DesignParams};
use std::path::{Path, PathBuf};

pub fn run(args: &[String]) -> Option<i32> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("sound"), sub) => Some(
            match cmd(sub.unwrap_or("help"), &args[2.min(args.len())..]) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("anikit-video sound: {e}");
                    1
                }
            },
        ),
        _ => None,
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn cmd(sub: &str, args: &[String]) -> Result<(), String> {
    match sub {
        "list" => {
            println!("{}", super::all_names().join(" "));
            Ok(())
        }
        "packs" => {
            for p in pack::PACKS {
                println!(
                    "{:<18} {}  [{}{}]\n{:<18} {}",
                    p.name,
                    p.description,
                    p.license,
                    if pack::is_cached(p.name) {
                        ", cached"
                    } else {
                        ""
                    },
                    "",
                    p.page
                );
                if pack::is_cached(p.name) {
                    println!("{:<18} sounds: {}", "", pack::groups(p.name).join(" "));
                }
            }
            Ok(())
        }
        "fetch" => {
            let name = args
                .first()
                .ok_or("sound fetch <pack> (see `sound packs`)")?;
            let d = pack::fetch(name)?;
            println!(
                "{} → {}\nsounds: {}",
                name,
                d.display(),
                pack::groups(name).join(" ")
            );
            Ok(())
        }
        "render" => {
            let name = args
                .first()
                .ok_or("sound render <name> [-o out.wav] [key=value …]")?;
            let mut p = DesignParams::default();
            let mut obj = serde_json::Map::new();
            for a in args.iter().skip(1).filter(|a| a.contains('=')) {
                let (k, v) = a.split_once('=').unwrap();
                obj.insert(
                    k.into(),
                    serde_json::from_str(v).unwrap_or(serde_json::Value::String(v.into())),
                );
            }
            if !obj.is_empty() {
                p = serde_json::from_value(serde_json::Value::Object(obj))
                    .map_err(|e| e.to_string())?;
            }
            let s = render(name, &p, crate::audio::DEFAULT_RATE)?;
            let out = flag(args, "-o")
                .unwrap_or_else(|| format!("{}.wav", name.replace(['/', '+'], "_")));
            let mut samples = Vec::with_capacity(s.l.len() * 2);
            for (l, r) in s.l.iter().zip(&s.r) {
                samples.push(*l);
                samples.push(*r);
            }
            crate::audio::write_wav(
                Path::new(&out),
                &crate::audio::Pcm {
                    rate: crate::audio::DEFAULT_RATE,
                    channels: 2,
                    samples,
                },
                crate::audio::WavFormat::Pcm16,
            )?;
            println!(
                "{out}: {:.2} s, hit at {:.3} s",
                s.l.len() as f32 / crate::audio::DEFAULT_RATE as f32,
                s.hit
            );
            Ok(())
        }
        "plan" | "mix" => {
            let file = args.first().ok_or(format!("sound {sub} spec.json"))?;
            let ev = load(file, flag(args, "--sound"))?;
            let plan = ev.sound.clone();
            if sub == "plan" {
                let Some(plan) = plan else {
                    return Err("the spec has no `sound` key and no layer `sfx` (try --sound auto)".into());
                };
                let t = std::time::Instant::now();
                let cues = super::auto::plan_cues(&ev, &plan);
                println!(
                    "palette {} · space {} · {} cues in {:.0} ms",
                    plan.palette,
                    plan.space.as_ref().map(|s| s.0.as_str()).unwrap_or("dry"),
                    cues.len(),
                    t.elapsed().as_secs_f64() * 1e3
                );
                for c in &cues {
                    println!(
                        "{:7.3}s  {:<14} pan {:+.2}  {:+.1} dB  {}",
                        c.at, c.name, c.params.pan, c.gain_db, c.why
                    );
                }
                return Ok(());
            }
            let out = PathBuf::from(flag(args, "-o").ok_or("sound mix spec.json -o out.wav")?);
            let t = std::time::Instant::now();
            let wav = if out.extension().is_some_and(|e| e == "wav") {
                out.clone()
            } else {
                out.with_extension("tmp.wav")
            };
            crate::pipeline::mix_audio(&ev, 0.0, ev.duration, &wav)?.ok_or("the video has no audio")?;
            if wav != out {
                let st = std::process::Command::new("ffmpeg")
                    .args(["-y", "-loglevel", "error", "-i"])
                    .arg(&wav)
                    .arg(&out)
                    .status()
                    .map_err(|e| format!("ffmpeg: {e}"))?;
                let _ = std::fs::remove_file(&wav);
                if !st.success() {
                    return Err("ffmpeg failed".into());
                }
            }
            let secs = t.elapsed().as_secs_f64();
            println!(
                "{}: {:.1} s of audio in {:.2} s ({:.0}× real time)",
                out.display(),
                ev.duration,
                secs,
                ev.duration as f64 / secs
            );
            Ok(())
        }
        _ => {
            println!("sound list | render <name> [-o x.wav] [k=v] | plan spec.json [--sound JSON] | mix spec.json -o out.wav [--sound JSON] | packs | fetch <pack>\n(anikit-video docs sound)");
            Ok(())
        }
    }
}

/// Compile a spec file, optionally replacing its `sound` key.
fn load(file: &str, sound: Option<String>) -> Result<crate::eval::Evaluator, String> {
    let path = Path::new(file);
    let text = std::fs::read_to_string(path).map_err(|e| format!("{file}: {e}"))?;
    let mut v: serde_json::Value = if file.ends_with(".yaml") || file.ends_with(".yml") {
        serde_norway::from_str(&text).map_err(|e| e.to_string())?
    } else {
        serde_json::from_str(&text).map_err(|e| format!("{file}: {e}"))?
    };
    match sound.as_deref() {
        // `--sound none`: the plain mix (no sound design at all), for before/after.
        Some("none") => {
            if let Some(o) = v.as_object_mut() {
                o.remove("sound");
            }
        }
        Some(s) => v["sound"] = serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.into())),
        None => {}
    }
    let base = path.parent().unwrap_or(Path::new("."));
    crate::preview::Preview::new(v, base).map(|p| p.ev)
}
