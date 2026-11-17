//! Agent-facing CLI commands: `new`, `templates`, `prompt`, `critique`, `mcp`.
//!
//! The binary calls [`run`] first; it returns `None` for commands it doesn't
//! own (`render`, `still`, `sheet`, `check`, …).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::critique::critique;
use crate::templates::{brand::Brand, suggest, Format, Template, BUILTIN};
use crate::{mcp, prompts};

/// Write to stdout; a closed pipe (`anikit-video templates | head`) ends the process quietly.
fn emit(s: std::fmt::Arguments) {
    use std::io::Write;
    let mut o = std::io::stdout().lock();
    if let Err(e) = o.write_fmt(s).and_then(|_| o.flush()) {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
    }
}

macro_rules! out {
    ($($t:tt)*) => { emit(format_args!($($t)*)) };
}
macro_rules! outln {
    () => { emit(format_args!("\n")) };
    ($($t:tt)*) => { emit(format_args!("{}\n", format_args!($($t)*))) };
}

pub const HELP: &str = "\
Agent commands:
  anikit-video new <template> [--brand brand.json] [--from README.md] [--format 16:9] [--dir DIR]
                                    scaffold a project: params.json, brand.json, spec.json, NEXT.md
  anikit-video templates            list templates
  anikit-video templates schema <name> [--json] a template's parameters (compact; --json: JSON Schema)
  anikit-video templates guide <name>           how to fill it well (markdown)
  anikit-video templates expand <name> [params.json] [--brand brand.json] [--format 9:16] [--look noir] [-o spec.json]
  anikit-video prompt [name]        list prompts, or print one (e.g. director)
  anikit-video looks [name] [--json] art-direction looks (palette, type, motion, world); a spec takes \"look\": \"noir\"
  anikit-video critique <spec.json> [--render] [--pack dir] [--detail] [--json]  review; --render also checks rendered frames
                                    and the timeline against the spec's \"intent\" (pace, dead air, beats, arc)
                                    (contrast, overlaps, cut text, frame fill, centred layouts); --pack writes frames
                                    to look at (strip, cuts, worst frames, peak) and a review.md rubric
  anikit-video preview <spec.json|template> [--at T] [--width W] [-o out.png] [--format 9:16] [--brand b.json] [--params p.json]
                                    contact sheet (one frame per scene), or one still with --at
  anikit-video mcp                  Model Context Protocol server on stdio
";

/// Commands this module handles.
pub const COMMANDS: &[&str] = &["new", "templates", "template", "prompt", "prompts", "looks", "look", "critique", "preview", "mcp"];

/// Run an agent command. `args` excludes the program name.
pub fn run(args: &[String]) -> Option<i32> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("--help");
    if matches!(cmd, "--help" | "-h" | "help") {
        print!("Render: anikit-video render|still|sheet|check|info|schema SPEC [options]   (anikit-video render spec.json -o out.mp4)\n{HELP}");
        return Some(0);
    }
    if !COMMANDS.contains(&cmd) {
        return None;
    }
    let rest = &args[1..];
    let r = match cmd {
        "new" => cmd_new(rest),
        "templates" | "template" => cmd_templates(rest),
        "prompt" | "prompts" => cmd_prompt(rest),
        "looks" | "look" => cmd_looks(rest),
        "critique" => cmd_critique(rest),
        "preview" => cmd_preview(rest),
        "mcp" => mcp::serve_stdio().map_err(|e| e.to_string()),
        _ => unreachable!(),
    };
    Some(match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("anikit-video {cmd}: {e}");
            1
        }
    })
}

/// Simple flag parsing: positionals plus `--flag value` / `-o value` / `--switch`.
struct Args {
    pos: Vec<String>,
    flags: Map<String, Value>,
}

fn parse(args: &[String], value_flags: &[&str], switches: &[&str]) -> Result<Args, String> {
    let mut pos = Vec::new();
    let mut flags = Map::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(name) = a.strip_prefix("--").or_else(|| a.strip_prefix('-').filter(|n| n.len() == 1)) {
            let (name, inline) = match name.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (name, None),
            };
            if switches.contains(&name) {
                flags.insert(name.into(), Value::Bool(true));
            } else if value_flags.contains(&name) {
                let v = match inline {
                    Some(v) => v,
                    None => {
                        i += 1;
                        args.get(i).cloned().ok_or_else(|| format!("--{name} needs a value"))?
                    }
                };
                flags.insert(name.into(), Value::String(v));
            } else {
                let all: Vec<&str> = value_flags.iter().chain(switches).copied().collect();
                let hint = suggest(name, all.iter().copied()).map(|s| format!(" (did you mean --{s}?)")).unwrap_or_default();
                return Err(format!("unknown option '{a}'{hint}; options: {}", all.iter().map(|f| format!("--{f}")).collect::<Vec<_>>().join(" ")));
            }
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    Ok(Args { pos, flags })
}

impl Args {
    fn get(&self, k: &str) -> Option<&str> {
        self.flags.get(k).and_then(|v| v.as_str())
    }
    fn has(&self, k: &str) -> bool {
        self.flags.contains_key(k)
    }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn read_json(path: &str) -> Result<Value, String> {
    let src = read(path)?;
    serde_json::from_str(&src).map_err(|e| format!("{path}: invalid JSON at line {}, column {}: {e}", e.line(), e.column()))
}

pub fn load_brand(path: Option<&str>) -> Result<Brand, String> {
    match path {
        None => Ok(Brand::default()),
        Some(p) => Brand::from_json(&read(p)?).map_err(|e| format!("{p}: {e}")),
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(d) = path.parent() {
        if !d.as_os_str().is_empty() {
            std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
        }
    }
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap() + "\n"
}

fn cmd_templates(args: &[String]) -> Result<(), String> {
    let a = parse(args, &["brand", "format", "o", "out", "look"], &["json"])?;
    match a.pos.first().map(|s| s.as_str()) {
        None | Some("list") => {
            if a.has("json") {
                let v: Vec<Value> = BUILTIN.iter().map(|(n, _)| Template::builtin(n).map(|t| t.summary()).unwrap_or(Value::Null)).collect();
                out!("{}", pretty(&Value::Array(v)));
            } else {
                for (n, _) in BUILTIN {
                    outln!("{}", Template::builtin(n)?.one_line());
                }
            }
            Ok(())
        }
        Some("schema") => {
            let t = Template::builtin(a.pos.get(1).ok_or("usage: templates schema <name> [--json]")?)?;
            if a.has("json") {
                out!("{}", pretty(&t.param_schema()));
            } else {
                outln!("{}", t.compact_params());
            }
            Ok(())
        }
        Some("guide") => {
            let t = Template::builtin(a.pos.get(1).ok_or("usage: templates guide <name>")?)?;
            out!("{}", prompts::template_guide(&t));
            Ok(())
        }
        Some("expand") => {
            let name = a.pos.get(1).ok_or("usage: templates expand <name> [params.json] [--brand brand.json] [--format 16:9] [-o spec.json]")?;
            let t = Template::builtin(name)?;
            let params = match a.pos.get(2) {
                Some(p) => read_json(p)?,
                None => t.example_params(),
            };
            let params = with_look(params, a.get("look"))?;
            let brand = load_brand(a.get("brand"))?;
            let format = Format::parse(a.get("format").unwrap_or("16:9"))?;
            let spec = t.expand(&params, &brand, format).map_err(|e| e.join("\n  "))?;
            match a.get("o").or(a.get("out")) {
                Some(o) => {
                    write(Path::new(o), &pretty(&spec))?;
                    let base = Path::new(o).parent().map(Path::to_path_buf).unwrap_or_default();
                    // Pixel and timeline checks too, so the printed score is the honest one.
                    let r = crate::critique::review(&spec, &base, t.duration_range(), true);
                    let v = mcp::validate(spec.clone(), &base);
                    for k in ["errors", "warnings"] {
                        for d in v[k].as_array().into_iter().flatten() {
                            eprintln!("{} {}: {}", &k[..k.len() - 1], d["path"].as_str().unwrap_or(""), d["message"].as_str().unwrap_or(""));
                        }
                    }
                    for n in r.notes.iter().filter(|n| n.severity != "tip") {
                        eprintln!("{} {}: {} → {}", n.severity, n.path, n.message, n.fix);
                    }
                    for n in t.expansion_notes(&params, &spec, r.duration) {
                        eprintln!("{n}");
                    }
                    eprintln!("wrote {o} ({:.1} s, critique {}/100)", r.duration, r.score);
                }
                None => out!("{}", pretty(&spec)),
            }
            Ok(())
        }
        Some(other) => {
            // `templates launch` = guide for a template name.
            if let Ok(t) = Template::builtin(other) {
                out!("{}", prompts::template_guide(&t));
                return Ok(());
            }
            let subs = ["list", "schema", "guide", "expand"];
            let hint = suggest(other, subs.iter().copied().chain(BUILTIN.iter().map(|(n, _)| *n))).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
            Err(format!("unknown subcommand '{other}'{hint}; use list, schema, guide or expand"))
        }
    }
}

fn cmd_prompt(args: &[String]) -> Result<(), String> {
    match args.first() {
        None => {
            for (n, d) in prompts::list() {
                outln!("{n:<26} {d}");
            }
            Ok(())
        }
        Some(n) => {
            out!("{}", prompts::get(n)?);
            Ok(())
        }
    }
}

fn cmd_critique(args: &[String]) -> Result<(), String> {
    let a = parse(args, &["pack"], &["json", "render", "detail"])?;
    let path = a.pos.first().ok_or("usage: critique <spec.json> [--render] [--pack dir] [--detail] [--json]")?;
    let spec = read_json(path)?;
    let base = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default();
    let (r, files) = crate::critique::review_pack(&spec, &base, None, a.has("render"), a.get("pack").map(Path::new));
    if let Some(dir) = a.get("pack") {
        let files = files?;
        eprintln!("review pack: {dir}/ ({} files): open review.md and look at every image", files.len());
    }
    if a.has("json") {
        out!("{}", pretty(&serde_json::to_value(&r).unwrap()));
    } else {
        out!("{}", r.to_text_detail(a.has("detail")));
        out!("A score is not evidence of quality: look at full-size frames against docs/video/quality/BAR.md.\n");
    }
    Ok(())
}

fn cmd_preview(args: &[String]) -> Result<(), String> {
    let a = parse(args, &["at", "width", "o", "out", "format", "brand", "params", "look"], &[])?;
    let src = a.pos.first().ok_or("usage: preview <spec.json|template> [--at T] [--width W] [-o out.png]")?;
    let (spec, base, stem) = if src.ends_with(".json") || src.ends_with(".yaml") || Path::new(src).is_file() {
        let text = read(src)?;
        let spec = crate::spec::parse::parse_str(&text).map(|_| ()).map_err(|d| d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"));
        spec?;
        let v: Value = if text.trim_start().starts_with('{') { serde_json::from_str(&text).map_err(|e| e.to_string())? } else { read_json(src)? };
        let p = Path::new(src);
        (v, p.parent().map(Path::to_path_buf).unwrap_or_default(), p.file_stem().unwrap_or_default().to_string_lossy().to_string())
    } else {
        let t = Template::builtin(src)?;
        let params = match a.get("params") {
            Some(p) => read_json(p)?,
            None => t.example_params(),
        };
        let params = with_look(params, a.get("look"))?;
        let spec = t.expand(&params, &load_brand(a.get("brand"))?, Format::parse(a.get("format").unwrap_or("16:9"))?).map_err(|e| e.join("\n  "))?;
        let base = a.get("params").and_then(|p| Path::new(p).parent()).map(Path::to_path_buf).unwrap_or_default();
        (spec, base, src.clone())
    };
    let p = crate::preview::Preview::new(spec, &base)?;
    let width = a.get("width").map(|w| w.parse::<u32>().map_err(|_| format!("--width: '{w}' is not a number"))).transpose()?;
    let (img, default_out) = match a.get("at") {
        Some(t) => {
            let t: f32 = t.parse().map_err(|_| format!("--at: '{t}' is not a number of seconds"))?;
            (p.frame(t, width), format!("{stem}-{t}s.png"))
        }
        None => (p.sheet(None, width.unwrap_or(640)), format!("{stem}-sheet.png")),
    };
    let out = a.get("o").or(a.get("out")).map(String::from).unwrap_or(default_out);
    write(Path::new(&out), "")?;
    std::fs::write(&out, crate::preview::png(&img)).map_err(|e| format!("cannot write {out}: {e}"))?;
    outln!("{out} ({}×{}, video {:.1} s)", img.width, img.height, p.ev.duration);
    Ok(())
}

/// Facts pulled from a README/description to seed template parameters.
#[derive(Debug, Default, PartialEq)]
pub struct Facts {
    pub name: Option<String>,
    pub tagline: Option<String>,
    pub bullets: Vec<(String, Option<String>)>,
    pub url: Option<String>,
    pub install: Option<String>,
}

/// Heuristic README reader: the first `#` heading is the name, the first
/// prose paragraph the tagline, bullet lists under a features-like heading
/// (or the first list) the features, the first code line the install command.
pub fn read_facts(md: &str) -> Facts {
    let mut f = Facts::default();
    let mut in_code = false;
    let mut heading = String::new();
    let mut feature_bullets = Vec::new();
    let mut first_bullets = Vec::new();
    for line in md.lines() {
        let l = line.trim();
        if l.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            if f.install.is_none() && (l.starts_with("npm ") || l.starts_with("pip ") || l.starts_with("cargo ") || l.starts_with("brew ") || l.starts_with("npx ")) {
                f.install = Some(l.trim_start_matches("$ ").to_string());
            }
            continue;
        }
        if let Some(h) = l.strip_prefix('#') {
            let h = h.trim_start_matches('#').trim();
            if f.name.is_none() && !h.is_empty() {
                f.name = Some(clean_md(h));
            }
            heading = h.to_lowercase();
            continue;
        }
        if f.url.is_none() {
            if let Some(i) = l.find("https://") {
                let u: String = l[i + 8..].chars().take_while(|c| !c.is_whitespace() && !")]>\"'".contains(*c)).collect();
                if !u.contains("shields.io") && !u.contains("github.com/") && !u.contains("badge") && !u.is_empty() {
                    f.url = Some(u.trim_end_matches('/').to_string());
                }
            }
        }
        if let Some(b) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
            let b = clean_md(b);
            let (title, body) = match b.split_once(" — ").or_else(|| b.split_once(" - ")).or_else(|| b.split_once(": ")).or_else(|| b.split_once(". ")) {
                Some((t, d)) => (t.trim().trim_end_matches(['.', ':']).to_string(), Some(d.trim().to_string())),
                None => (b.trim_end_matches('.').to_string(), None),
            };
            let item = (title, body);
            if ["feature", "why", "highlight", "benefit"].iter().any(|k| heading.contains(k)) {
                feature_bullets.push(item);
            } else if first_bullets.len() < 6 {
                first_bullets.push(item);
            }
            continue;
        }
        if f.tagline.is_none() && f.name.is_some() && l.len() > 20 && !l.starts_with(['<', '[', '!', '|', '>']) {
            let first = clean_md(l.split(". ").next().unwrap_or(l)).trim_end_matches('.').to_string();
            f.tagline = Some(first);
        }
    }
    f.bullets = if feature_bullets.is_empty() { first_bullets } else { feature_bullets };
    f
}

fn clean_md(s: &str) -> String {
    let mut out = String::new();
    let mut skip_link = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '_' | '`' => {}
            '[' => {}
            ']' if chars.peek() == Some(&'(') => {
                skip_link = true;
            }
            '(' if skip_link => {
                for c2 in chars.by_ref() {
                    if c2 == ')' {
                        break;
                    }
                }
                skip_link = false;
            }
            _ => out.push(c),
        }
    }
    out.trim().to_string()
}

/// At most `n` words, cut at a natural point: the first clause if it fits,
/// never ending on a dangling preposition or article.
fn first_words(s: &str, n: usize) -> String {
    let w: Vec<&str> = s.split_whitespace().collect();
    if w.len() <= n {
        return s.to_string();
    }
    for sep in [" — ", " - ", ": ", ", ", "; "] {
        if let Some((head, _)) = s.split_once(sep) {
            let hw = head.split_whitespace().count();
            if hw >= 2 && hw <= n {
                return head.trim().to_string();
            }
        }
    }
    const DANGLING: &[&str] = &["a", "an", "the", "in", "of", "to", "for", "with", "and", "or", "on", "at", "by", "from", "into", "your", "our", "its"];
    let mut cut: Vec<&str> = w[..n].to_vec();
    while cut.len() > 2 && DANGLING.contains(&cut.last().unwrap().to_lowercase().trim_matches(|c: char| !c.is_alphanumeric())) {
        cut.pop();
    }
    cut.join(" ").trim_end_matches([',', ';', ':']).to_string()
}

/// Seed template params from facts, keeping within each slot's word limits.
pub fn params_from_facts(t: &Template, facts: &Facts) -> Value {
    let mut p = t.example_params();
    let decl = t.params();
    let limit = |k: &str| decl.get(k).and_then(|d| d.get("maxWords")).and_then(|x| x.as_u64()).unwrap_or(12) as usize;
    let obj = p.as_object_mut().unwrap();
    if let Some(n) = &facts.name {
        for k in ["product", "name", "event"] {
            if decl.contains_key(k) {
                obj.insert(k.into(), json!(first_words(n, limit(k))));
            }
        }
    }
    if let Some(tl) = &facts.tagline {
        for k in ["tagline", "subtitle", "takeaway"] {
            if decl.contains_key(k) {
                obj.insert(k.into(), json!(first_words(tl, limit(k))));
            }
        }
    }
    if let Some(u) = &facts.url {
        if decl.contains_key("url") {
            let max = decl["url"].get("maxChars").and_then(|x| x.as_u64()).unwrap_or(40) as usize;
            if u.len() <= max {
                obj.insert("url".into(), json!(u));
            }
        }
    }
    if !facts.bullets.is_empty() {
        for k in ["features", "points", "items"] {
            let Some(d) = decl.get(k) else { continue };
            let max = d.get("maxItems").and_then(|x| x.as_u64()).unwrap_or(4) as usize;
            let props = d.get("items").and_then(|i| i.get("properties")).and_then(|x| x.as_object());
            let lim = |f: &str| props.and_then(|p| p.get(f)).and_then(|x| x.get("maxWords")).and_then(|x| x.as_u64()).unwrap_or(12) as usize;
            let items: Vec<Value> = facts
                .bullets
                .iter()
                .take(max)
                .map(|(title, body)| match props {
                    Some(pr) => {
                        let tk = ["title", "text", "label", "name"].into_iter().find(|k| pr.contains_key(*k));
                        let bk = ["body", "description", "detail"].into_iter().find(|k| pr.contains_key(*k));
                        let mut o = Map::new();
                        if let Some(tk) = tk {
                            o.insert(tk.into(), json!(first_words(title, lim(tk))));
                        }
                        if let (Some(b), Some(bk)) = (body, bk) {
                            o.insert(bk.into(), json!(first_words(b, lim(bk))));
                        }
                        if let Some(vals) = pr.get("type").and_then(|t| t.get("values")).and_then(|v| v.as_array()) {
                            o.insert("type".into(), vals.first().cloned().unwrap_or(json!("new")));
                        }
                        Value::Object(o)
                    }
                    None => json!(first_words(title, 8)),
                })
                .collect();
            let min = d.get("minItems").and_then(|x| x.as_u64()).unwrap_or(1) as usize;
            if items.len() >= min {
                obj.insert(k.into(), Value::Array(items));
            }
        }
    }
    p
}

fn cmd_new(args: &[String]) -> Result<(), String> {
    let a = parse(args, &["brand", "from", "format", "dir"], &["force"])?;
    let name = a.pos.first().ok_or_else(|| {
        format!("usage: new <template> [--brand brand.json] [--from README.md] [--format 16:9]\ntemplates: {}", BUILTIN.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))
    })?;
    let t = Template::builtin(name)?;
    let dir = PathBuf::from(a.get("dir").unwrap_or(name));
    if dir.join("params.json").exists() && !a.has("force") {
        return Err(format!("{} already has a params.json; use --dir to pick another folder or --force to overwrite", dir.display()));
    }
    let mut brand = load_brand(a.get("brand"))?;
    let format = Format::parse(a.get("format").unwrap_or("16:9"))?;
    let facts = match a.get("from") {
        Some(src) if src.starts_with("http://") || src.starts_with("https://") => {
            return Err(format!("--from takes a local file; save the page first (e.g. curl -L {src} -o page.md) or pass the README"));
        }
        Some(src) => read_facts(&read(src)?),
        None => Facts::default(),
    };
    if brand.name.is_none() {
        brand.name = facts.name.clone();
    }
    if brand.voice.url.is_none() {
        brand.voice.url = facts.url.clone();
    }
    let params = params_from_facts(&t, &facts);
    let spec = t.expand(&params, &brand, format).map_err(|e| format!("the seeded params don't fit the template yet:\n  {}", e.join("\n  ")))?;
    let report = critique(&spec);
    write(&dir.join("params.json"), &pretty(&params))?;
    write(&dir.join("brand.json"), &pretty(&serde_json::to_value(&brand).unwrap()))?;
    write(&dir.join("spec.json"), &pretty(&spec))?;
    let fmt_flag = a.get("format").unwrap_or("16:9");
    let example = t.example_params();
    let leftover: Vec<String> = params.as_object().unwrap().iter().filter(|(k, v)| example.get(k.as_str()) == Some(v)).map(|(k, _)| format!("`{k}`")).collect();
    let d = dir.display();
    let next = format!(
        "# {name} video: next steps\n\nScaffolded by `anikit-video new {name}`{from}. Files:\n\n- `params.json`: the copy and options. **Edit this.** The schema is `anikit-video templates schema {name}`; the guide is `anikit-video templates guide {name}`.\n- `brand.json`: colours, fonts, logo and voice (`anikit-video prompt brand-kit`).\n- `spec.json`: generated; re-create it after every edit.\n\n## Loop\n\n```bash\nanikit-video templates expand {name} {d}/params.json --brand {d}/brand.json --format {fmt_flag} -o {d}/spec.json\nanikit-video critique {d}/spec.json        # fix every warning\nanikit-video preview {d}/spec.json -o {d}/sheet.png   # look at it\nanikit-video render {d}/spec.json -o {d}/{name}.mp4\n```\n\nOther formats: `--format 9:16` and `--format 1:1` reuse the same params.\n\n## Writing the copy\n\n{todo}Follow `anikit-video prompt launch-video-generator` (steps 1–4) for the script and storyboard, and `anikit-video prompt critic` to review.\n\nCurrent critique: {score}/100, {dur:.1} s.\n",
        from = a.get("from").map(|f| format!(" from `{f}`")).unwrap_or_default(),
        todo = if facts.name.is_none() {
            "The params are the template's **example copy**: replace every value with your product's facts.\n\n".to_string()
        } else {
            format!("Params were seeded from the source (shortened to fit the word limits; rewrite them as benefits). **Still example copy, replace or remove:** {}.\n\n", if leftover.is_empty() { "none".into() } else { leftover.join(", ") })
        },
        score = report.score,
        dur = report.duration,
    );
    write(&dir.join("NEXT.md"), &next)?;
    outln!("created {d}/params.json, brand.json, spec.json, NEXT.md ({:.1} s, critique {}/100)", report.duration, report.score);
    for n in report.notes.iter().filter(|n| n.severity != "tip").take(5) {
        outln!("  {} {}: {}", n.severity, n.path, n.message);
    }
    if facts.name.is_some() && !leftover.is_empty() {
        outln!("  still example copy (replace): {}", leftover.join(", ").replace('`', ""));
    }
    outln!("next: edit {d}/params.json, then follow {d}/NEXT.md");
    Ok(())
}

/// A `--look` flag value: a name, `a+b` (composed), or JSON (list or object).
pub fn parse_look(s: &str) -> Result<Value, String> {
    let s = s.trim();
    if s.starts_with('[') || s.starts_with('{') {
        return serde_json::from_str(s).map_err(|e| format!("--look: {e}"));
    }
    if s.contains('+') {
        return Ok(Value::Array(s.split('+').map(|x| Value::String(x.trim().into())).collect()));
    }
    Ok(Value::String(s.into()))
}

fn with_look(mut params: Value, look: Option<&str>) -> Result<Value, String> {
    if let Some(l) = look {
        let v = parse_look(l)?;
        if v.as_str() != Some("none") {
            crate::looks::resolve(&v)?;
        }
        if !params.is_object() {
            params = serde_json::json!({});
        }
        params["look"] = v;
    }
    Ok(params)
}

/// `looks [name] [--json]`.
pub fn looks_text(name: Option<&str>, full: bool) -> Result<String, String> {
    let Some(n) = name else {
        return Ok(format!(
            "{}\nUse: \"look\": \"noir\" in a spec, [\"editorial\", \"noir\"] to compose, {{\"base\": \"swiss\", \"theme\": {{...}}}} to override. Details: looks <name>.\n",
            crate::looks::list_lines().join("\n")
        ));
    };
    let l = crate::looks::resolve(&parse_look(name.unwrap_or(n))?)?;
    if full {
        return Ok(format!("{}\n", serde_json::to_string_pretty(&l).unwrap()));
    }
    let g = |k: &str| l.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let c = &l["theme"]["colors"];
    let f = &l["theme"]["fonts"];
    let comp = |k: &str| l.get(k).and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x["component"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    Ok(format!(
        "{} ({n}, {})\n{}\npalette: bg {} · text {} · primary {} · accent {}\ntype: {} / {}\nmotion: enter {} · transition {}\nworld: {} · over: {}\nOverride anything: {{\"look\": {{\"base\": \"{n}\", \"theme\": {{\"colors\": {{\"primary\": \"#...\"}}}}}}}}; --json prints it all.\n",
        g("title"),
        g("mode"),
        g("direction"),
        c["bg"].as_str().unwrap_or("-"),
        c["text"].as_str().unwrap_or("-"),
        c["primary"].as_str().unwrap_or("-"),
        c["accent"].as_str().unwrap_or("-"),
        f["heading"].as_str().unwrap_or("Inter Display"),
        f["body"].as_str().unwrap_or("Inter"),
        compact(&l["enter"]),
        compact(&l["transition"]),
        comp("backdrop"),
        {
            let p = comp("overlay");
            let g = compact(&l["post"]);
            if p.is_empty() { format!("grade {g}") } else { format!("{p} · grade {g}") }
        },
        n = l["name"].as_str().unwrap_or(n),
    ))
}

fn compact(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "-".into(),
        o => o.get("type").or(o.get("preset")).and_then(|x| x.as_str()).map(String::from).unwrap_or_else(|| o.to_string()),
    }
}

fn cmd_looks(args: &[String]) -> Result<(), String> {
    let a = parse(args, &[], &["json"])?;
    let t = looks_text(a.pos.first().map(|s| s.as_str()), a.has("json"))?;
    out!("{t}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const README: &str = "# Anikit Video\n\n[![badge](https://img.shields.io/x)](x)\n\nProgrammatic video for AI agents, rendered natively in Rust. More text.\n\n## Features\n\n- **Fast** — renders 935 frames per second\n- Agent-first: one JSON spec, clear errors\n- Runs anywhere\n\n```bash\nnpm i anikit\n```\n\nDocs: https://anikit.dev/video\n";

    #[test]
    fn reads_facts_from_a_readme() {
        let f = read_facts(README);
        assert_eq!(f.name.as_deref(), Some("Anikit Video"));
        assert_eq!(f.tagline.as_deref(), Some("Programmatic video for AI agents, rendered natively in Rust"));
        assert_eq!(f.bullets.len(), 3);
        assert_eq!(f.bullets[0], ("Fast".into(), Some("renders 935 frames per second".into())));
        assert_eq!(f.install.as_deref(), Some("npm i anikit"));
        assert_eq!(f.url.as_deref(), Some("anikit.dev/video"));
    }

    #[test]
    fn seeded_params_expand_for_every_template() {
        let f = read_facts(README);
        for (n, _) in BUILTIN {
            let t = Template::builtin(n).unwrap();
            let p = params_from_facts(&t, &f);
            t.expand(&p, &Brand::default(), Format::parse("9:16").unwrap()).unwrap_or_else(|e| panic!("{n}: {e:?}\n{p}"));
        }
    }

    #[test]
    fn trims_at_natural_points() {
        assert_eq!(first_words("Lumen turns your notes into a searchable second brain in seconds", 10), "Lumen turns your notes into a searchable second brain");
        assert_eq!(first_words("Fast renders, every core, no browser at all", 4), "Fast renders");
    }

    #[test]
    fn flag_typos_suggest() {
        let e = parse(&["--brnad".into(), "x".into()], &["brand", "format"], &[]).err().unwrap();
        assert!(e.contains("did you mean --brand"), "{e}");
    }
}
