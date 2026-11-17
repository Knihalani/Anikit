//! `anikit-video mcp`: a Model Context Protocol server on stdio
//! (newline-delimited JSON-RPC 2.0).
//!
//! Tools: `list_templates`, `get_template_schema`, `create_from_template`,
//! `validate_spec`, `critique_spec`, `render_still`, `render_contact_sheet`,
//! `render_video`, `get_prompt`. Prompts: the prompt library. Resources: the
//! spec reference and the agent quick start.
//!
//! Rendering tools run this same executable's `check` / `still` / `sheet` /
//! `render` commands in a child process, so a crash in a render can't take
//! the server down, and the server never holds frames in memory.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::critique::critique;
use crate::templates::{brand::Brand, Format, Template, BUILTIN};
use crate::{agent_cli, prompts};

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "JSON spec → MP4, rendered natively. Flow: list_templates → get_template_schema → create_from_template (returns a spec path) → \
fix listed issues → render_contact_sheet (look) → render_video. Custom specs: resource anikit://docs/spec. Rules: get_prompt('motion-rules').";

/// Serve until stdin closes.
pub fn serve_stdio() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    let mut server = Server::new();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(resp) = server.handle_line(&line) {
            writeln!(out, "{resp}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// The protocol state machine, separate from I/O so it can be tested.
pub struct Server {
    /// Executable used for `check`/`still`/`sheet`/`render`.
    pub exe: PathBuf,
    /// Where generated specs and renders go when no path is given.
    pub workdir: PathBuf,
    counter: u32,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    pub fn new() -> Server {
        let exe = std::env::var_os("ANIKIT_VIDEO_EXE").map(PathBuf::from).or_else(|| std::env::current_exe().ok()).unwrap_or_else(|| "anikit-video".into());
        let workdir = std::env::current_dir().unwrap_or_default().join(".anikit-video");
        Server { exe, workdir, counter: 0 }
    }

    /// Handle one JSON-RPC message; `None` for notifications.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Value::Array(batch) = &msg {
            let out: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
            return (!out.is_empty()).then(|| Value::Array(out).to_string());
        }
        self.handle(&msg).map(|v| v.to_string())
    }

    fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let id = id?; // notifications (initialized, cancelled, …) get no reply
        let result = match method {
            "initialize" => {
                let v = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or(PROTOCOL);
                Ok(json!({
                    "protocolVersion": v,
                    "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
                    "serverInfo": { "name": "anikit-video", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(match self.call(name, &args) {
                    Ok(content) => json!({ "content": content, "isError": false }),
                    Err(e) => json!({ "content": [ { "type": "text", "text": e } ], "isError": true }),
                })
            }
            "prompts/list" => Ok(json!({
                "prompts": prompts::list().into_iter().map(|(n, d)| json!({ "name": n, "description": d })).collect::<Vec<_>>()
            })),
            "prompts/get" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                prompts::get(name).map(|md| {
                    json!({
                        "description": prompts::description(&md),
                        "messages": [ { "role": "user", "content": { "type": "text", "text": prompts::body(&md) } } ]
                    })
                }).map_err(|e| (-32602, e))
            }
            "resources/list" => Ok(json!({ "resources": RESOURCES.iter().map(|(uri, name, desc, _)| json!({ "uri": uri, "name": name, "description": desc, "mimeType": "text/markdown" })).collect::<Vec<_>>() })),
            "resources/read" => {
                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                match RESOURCES.iter().find(|r| r.0 == uri) {
                    Some((uri, _, _, text)) => Ok(json!({ "contents": [ { "uri": uri, "mimeType": "text/markdown", "text": text } ] })),
                    None => Err((-32002, format!("unknown resource '{uri}'; see resources/list"))),
                }
            }
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, m)) => serde_json::from_str(&error(id, code, &m)).unwrap(),
        })
    }

    fn call(&mut self, name: &str, a: &Value) -> Result<Vec<Value>, String> {
        match name {
            "list_templates" => Ok(vec![text(&BUILTIN.iter().filter_map(|(n, _)| Template::builtin(n).ok()).map(|t| t.one_line()).collect::<Vec<_>>().join("\n"))]),
            "get_template_schema" => {
                let t = Template::builtin(str_arg(a, "name")?)?;
                if a.get("full") == Some(&Value::Bool(true)) {
                    let mut s = t.param_schema();
                    s["example"] = t.example_params();
                    return Ok(vec![text(&s.to_string())]);
                }
                Ok(vec![text(&t.compact_params())])
            }
            "create_from_template" => {
                let t = Template::builtin(str_arg(a, "template")?)?;
                let brand = match a.get("brand") {
                    Some(Value::String(p)) => agent_cli::load_brand(Some(p))?,
                    Some(v @ Value::Object(_)) => Brand::from_value(v)?,
                    _ => Brand::default(),
                };
                let fmt_s = a.get("format").and_then(|f| f.as_str()).unwrap_or("16:9");
                let format = Format::parse(fmt_s)?;
                let mut params = a.get("params").cloned().unwrap_or(json!({}));
                if let (Some(l), Some(m)) = (a.get("look"), params.as_object_mut()) {
                    m.insert("look".into(), match l.as_str() {
                        Some(s) => agent_cli::parse_look(s)?,
                        None => l.clone(),
                    });
                }
                let spec = t.expand(&params, &brand, format).map_err(|e| e.join("\n"))?;
                let out = match a.get("out").and_then(|o| o.as_str()) {
                    Some(p) => PathBuf::from(p),
                    None => {
                        self.counter += 1;
                        self.workdir.join(format!("{}-{}-{}.json", t.name, fmt_s.replace(':', "x"), self.counter))
                    }
                };
                write_file(&out, &pretty(&spec))?;
                let base = std::env::current_dir().unwrap_or_default();
                let mut lines = vec![summary_line(&spec, &base, &out, t.duration_range())];
                lines.extend(issue_lines(&spec, &base, t.duration_range()));
                lines.extend(t.expansion_notes(&params, &spec, critique(&spec).duration));
                if a.get("return_spec") == Some(&Value::Bool(true)) {
                    lines.push(spec.to_string());
                }
                Ok(vec![text(&lines.join("\n"))])
            }
            "critique_spec" => {
                let (spec, path) = self.spec_arg(a)?;
                let mut r = critique(&spec);
                if a.get("render") == Some(&Value::Bool(true)) {
                    let p = crate::preview::Preview::new(spec.clone(), path.parent().unwrap_or(Path::new(".")))?;
                    let extra = crate::render_check::check_spec(&spec, &p.ev);
                    r.score = extra.iter().fold(r.score as i32, |s, n| s - if n.severity == "error" { 15 } else { 6 }).max(0) as u32;
                    r.notes.extend(extra);
                }
                let mut lines = vec![format!("score {}/100 · {:.1} s", r.score, r.duration)];
                lines.extend(r.notes.iter().map(|n| format!("{} {}: {} → {}", n.severity, n.path, n.message, n.fix)));
                Ok(vec![text(&lines.join("\n"))])
            }
            "validate_spec" => {
                let (spec, path) = self.spec_arg(a)?;
                let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
                let v = validate(spec, &base);
                let mut lines = vec![if v["ok"] == true {
                    format!("ok · {:.1} s · {} frames · {} scenes", v["duration"].as_f64().unwrap_or(0.0), v["frames"], v["scenes"].as_array().map_or(0, |a| a.len()))
                } else {
                    "invalid".to_string()
                }];
                lines.extend(diag_lines(&v));
                Ok(vec![text(&lines.join("\n"))])
            }
            "render_still" => {
                let (spec, path) = self.spec_arg(a)?;
                let at = a.get("at").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                let out = self.out_path(a, "png");
                let p = crate::preview::Preview::new(spec, path.parent().unwrap_or(Path::new(".")))?;
                let w = a.get("width").and_then(|v| v.as_u64()).map(|w| w as u32).or((a.get("inline") == Some(&Value::Bool(true))).then_some(640));
                let img = p.frame(at, w);
                write_bytes(&out, &crate::preview::png(&img))?;
                self.image_result(&out, a)
            }
            "render_contact_sheet" => {
                let (spec, path) = self.spec_arg(a)?;
                let out = self.out_path(a, "png");
                let p = crate::preview::Preview::new(spec, path.parent().unwrap_or(Path::new(".")))?;
                let times = a.get("times").and_then(|t| t.as_array()).map(|v| v.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect());
                let times = times.or_else(|| {
                    let n = a.get("frames").and_then(|v| v.as_u64())? as usize;
                    let d = p.ev.duration;
                    Some((0..n).map(|i| (i as f32 + 0.5) * d / n as f32).collect())
                });
                let cell = a.get("width").and_then(|v| v.as_u64()).unwrap_or(400) as u32;
                let img = p.sheet(times, cell);
                write_bytes(&out, &crate::preview::png(&img))?;
                let mut res = self.image_result(&out, a)?;
                let scenes: Vec<String> = p.ev.scenes().iter().map(|s| format!("{} {:.1}-{:.1}", s.id, s.start, s.end)).collect();
                res[0] = text(&format!("{} (scenes: {})", out.display(), scenes.join(", ")));
                Ok(res)
            }
            "render_video" => {
                let (_, path) = self.spec_arg(a)?;
                let out = self.out_path(a, "mp4");
                let mut cmd = vec!["render".into(), path.display().to_string(), "-o".into(), out.display().to_string()];
                if let Some(q) = a.get("quality").and_then(|v| v.as_str()) {
                    cmd.extend(["--quality".into(), q.into()]);
                }
                let log = self.exe_run(&cmd)?;
                Ok(vec![text(&format!("rendered {}\n{}", out.display(), log.trim()))])
            }
            "get_prompt" => Ok(vec![text(&prompts::get(str_arg(a, "name")?)?)]),
            "list_looks" => Ok(vec![text(&agent_cli::looks_text(a.get("name").and_then(|x| x.as_str()), a.get("full") == Some(&json!(true)))?)]),
            _ => {
                let names: Vec<String> = tools().iter().filter_map(|t| t["name"].as_str().map(String::from)).collect();
                let hint = crate::templates::suggest(name, names.iter().map(|s| s.as_str())).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
                Err(format!("unknown tool '{name}'{hint}"))
            }
        }
    }

    /// The spec from `spec` (an object) or `path`; inline specs are written to
    /// the work directory with relative asset paths made absolute.
    fn spec_arg(&mut self, a: &Value) -> Result<(Value, PathBuf), String> {
        if let Some(p) = a.get("path").and_then(|p| p.as_str()) {
            let src = std::fs::read_to_string(p).map_err(|e| format!("cannot read {p}: {e}"))?;
            let v = serde_json::from_str(&src).map_err(|e| format!("{p}: invalid JSON at line {}, column {}: {e}", e.line(), e.column()))?;
            return Ok((v, PathBuf::from(p)));
        }
        let mut spec = match a.get("spec") {
            Some(Value::String(s)) => serde_json::from_str(s).map_err(|e| format!("spec: invalid JSON at line {}, column {}: {e}", e.line(), e.column()))?,
            Some(v @ Value::Object(_)) => v.clone(),
            _ => return Err("pass the spec as \"spec\" (a JSON object) or \"path\" (a spec file)".into()),
        };
        let base = a.get("base_dir").and_then(|b| b.as_str()).map(PathBuf::from).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        absolutize(&mut spec, &base);
        self.counter += 1;
        let path = self.workdir.join(format!("spec-{}-{}.json", std::process::id(), self.counter));
        write_file(&path, &pretty(&spec))?;
        Ok((spec, path))
    }

    fn out_path(&mut self, a: &Value, ext: &str) -> PathBuf {
        if let Some(o) = a.get("out").and_then(|o| o.as_str()) {
            return PathBuf::from(o);
        }
        self.counter += 1;
        self.workdir.join(format!("out-{}-{}.{ext}", std::process::id(), self.counter))
    }

    fn exe_run(&self, args: &[String]) -> Result<String, String> {
        let o = Command::new(&self.exe).args(args).output().map_err(|e| format!("cannot run {}: {e}", self.exe.display()))?;
        let stdout = String::from_utf8_lossy(&o.stdout).to_string();
        let stderr = String::from_utf8_lossy(&o.stderr).to_string();
        if o.status.success() {
            Ok(if stdout.trim().is_empty() { stderr } else { stdout })
        } else {
            Err(format!("anikit-video {} failed:\n{}{}", args[0], stdout, stderr))
        }
    }

    /// A path, plus the PNG inline when `inline` is not false and it is small (< 1.5 MB).
    fn image_result(&self, out: &Path, a: &Value) -> Result<Vec<Value>, String> {
        let mut v = vec![text(&out.display().to_string())];
        if a.get("inline") == Some(&Value::Bool(true)) {
            let bytes = std::fs::read(out).map_err(|e| e.to_string())?;
            if bytes.len() < 1_500_000 {
                v.push(json!({ "type": "image", "data": base64(&bytes), "mimeType": "image/png" }));
            } else {
                v.push(text("too large to inline; pass a smaller width"));
            }
        }
        Ok(v)
    }
}

/// `spec.json · 25.1 s · 6 scenes · valid · critique 100/100`.
fn summary_line(spec: &Value, base: &Path, out: &Path, range: Option<(f64, f64)>) -> String {
    let v = validate(spec.clone(), base);
    let r = crate::critique::critique_in_range(spec, range);
    format!(
        "{} · {:.1} s · {} scenes · {} · critique {}/100",
        out.display(),
        r.duration,
        spec["scenes"].as_array().map_or(0, |a| a.len()),
        if v["ok"] == true { "valid" } else { "INVALID" },
        r.score
    )
}

/// One line per validation diagnostic, then per critique note (tips omitted).
fn issue_lines(spec: &Value, base: &Path, range: Option<(f64, f64)>) -> Vec<String> {
    let mut l = diag_lines(&validate(spec.clone(), base));
    l.extend(crate::critique::critique_in_range(spec, range).notes.iter().filter(|n| n.severity != "tip").map(|n| format!("{} {}: {} → {}", n.severity, n.path, n.message, n.fix)));
    l
}

fn diag_lines(v: &Value) -> Vec<String> {
    ["errors", "warnings"]
        .iter()
        .flat_map(|k| v[*k].as_array().cloned().unwrap_or_default().into_iter().map(move |d| (k, d)))
        .map(|(k, d)| {
            let sug = d["suggestion"].as_str().map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
            format!("{} {}: {}{sug}", &k[..k.len() - 1], d["path"].as_str().unwrap_or(""), d["message"].as_str().unwrap_or(""))
        })
        .collect()
}

/// Validate a spec with the real parser and compiler (assets are checked for
/// existence, not decoded): `{ ok, errors, warnings, duration, frames, scenes }`.
pub fn validate(spec: Value, base_dir: &Path) -> Value {
    use crate::eval::{Evaluator, Options};
    use crate::spec::parse::{from_value, Severity};
    let (ev, diags) = match from_value(spec) {
        Err(d) => (None, d),
        Ok(p) => {
            let (ev, mut d) = Evaluator::build(&p.spec, &Options { base_dir: base_dir.to_path_buf(), load_assets: false, ..Options::default() });
            d.extend(p.warnings);
            (ev, d)
        }
    };
    let pick = |sev: Severity| diags.iter().filter(|d| d.severity == sev).map(|d| serde_json::to_value(d).unwrap()).collect::<Vec<_>>();
    let errors = pick(Severity::Error);
    json!({
        "ok": errors.is_empty() && ev.is_some(),
        "errors": errors,
        "warnings": pick(Severity::Warning),
        "duration": ev.as_ref().map(|e| e.duration),
        "frames": ev.as_ref().map(|e| e.frames()),
        "size": ev.as_ref().map(|e| [e.width, e.height]),
        "scenes": ev.as_ref().map(|e| e.scenes()),
    })
}

/// Resources served to clients.
const RESOURCES: &[(&str, &str, &str, &str)] = &[
    ("anikit://docs/spec", "Spec cheat sheet", "One page: the Anikit Video JSON spec (layers, animation, presets, transitions, CLI).", include_str!("../../../../docs/video/spec.md")),
    ("anikit://docs/authoring", "Authoring anything", "Expressions (numbers and colours, music-reactive), script blocks that generate layers, and raw draw layers.", include_str!("../../../../docs/video/authoring.md")),
    ("anikit://docs/spec-reference", "Spec reference", "The complete Anikit Video JSON spec reference, with every key and recipes.", include_str!("../../../../docs/video/spec-reference.md")),
    ("anikit://docs/agents", "Agent quick start", "How to make a great video with Anikit Video as an agent.", include_str!("../../../../docs/video/AGENTS.md")),
    ("anikit://prompts/motion-rules", "Motion rules", "Timing, hierarchy, easing, reading time, safe areas.", include_str!("../../../../prompts/motion-rules.md")),
];

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        b.extend(e.clone());
    }
    base
}

/// Tool declarations.
pub fn tools() -> Vec<Value> {
    let obj = |props: Value, req: &[&str]| json!({ "type": "object", "properties": props, "required": req });
    let spec = || json!({ "path": { "type": "string", "description": "spec file (preferred)" }, "spec": { "type": "object", "description": "or inline spec" } });
    let img = |extra: Value| with(with(spec(), json!({ "inline": { "type": "boolean", "description": "also return base64 PNG (default false)" }, "width": { "type": "integer" }, "out": { "type": "string" } })), extra);
    vec![
        json!({ "name": "list_templates", "description": "Template names, one line each.", "inputSchema": obj(json!({}), &[]) }),
        json!({ "name": "get_template_schema", "description": "A template's parameters (compact) with an example.", "inputSchema": obj(json!({ "name": { "type": "string" }, "full": { "type": "boolean", "description": "JSON Schema instead" } }), &["name"]) }),
        json!({ "name": "create_from_template", "description": "Template + params + brand → spec file. Returns path, duration, validity, critique issues.", "inputSchema": obj(json!({
            "template": { "type": "string" }, "params": { "type": "object" },
            "brand": { "type": ["object", "string"], "description": "{name, colors:{primary}, logo, voice:{cta,url}} or brand.json path" },
            "format": { "type": "string", "description": "16:9 | 9:16 | 1:1" }, "out": { "type": "string" },
            "look": { "description": "art direction: a look name (list_looks), a+b, or {base, ...overrides}; default: the template's own" },
            "return_spec": { "type": "boolean" }
        }), &["template", "params"]) }),
        json!({ "name": "validate_spec", "description": "Errors/warnings, one line each, with did-you-mean.", "inputSchema": obj(spec(), &[]) }),
        json!({ "name": "critique_spec", "description": "Design review: reading time, hook, contrast, safe areas, pacing. render:true also checks rendered frames (contrast vs real pixels, overlaps, cut text). One line per issue with a fix.", "inputSchema": obj(with(spec(), json!({ "render": { "type": "boolean" } })), &[]) }),
        json!({ "name": "render_still", "description": "One frame → PNG path.", "inputSchema": obj(img(json!({ "at": { "type": "number", "description": "seconds" } })), &[]) }),
        json!({ "name": "render_contact_sheet", "description": "One frame per scene in one PNG → path. Cheapest full review.", "inputSchema": obj(img(json!({ "frames": { "type": "integer" } })), &[]) }),
        json!({ "name": "render_video", "description": "Final render → path.", "inputSchema": obj(with(spec(), json!({ "out": { "type": "string", "description": "e.g. out.mp4" }, "quality": { "type": "string", "description": "draft|standard|high" } })), &[]) }),
        json!({ "name": "list_looks", "description": "Art-direction looks (palette, type, motion, world layers). name → its direction; full:true → JSON to override.", "inputSchema": obj(json!({ "name": { "type": "string" }, "full": { "type": "boolean" } }), &[]) }),
        json!({ "name": "get_prompt", "description": "Prompt library: director, launch-video-generator, script-writer, storyboarder, critic, copy-guard, brand-kit, motion-rules, template:<name>.", "inputSchema": obj(json!({ "name": { "type": "string" } }), &["name"]) }),
    ]
}

fn absolutize(v: &mut Value, base: &Path) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                match (k.as_str(), &*x) {
                    ("src" | "logo", Value::String(s)) if is_relative_path(s) => *x = Value::String(base.join(s).display().to_string()),
                    ("fonts", Value::Object(_)) => {
                        if let Value::Object(f) = x {
                            for (_, p) in f.iter_mut() {
                                if let Value::String(s) = p {
                                    if is_relative_path(s) && (s.contains('.') || s.contains('/')) {
                                        *p = Value::String(base.join(&*s).display().to_string());
                                    }
                                }
                            }
                        }
                    }
                    _ => absolutize(x, base),
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| absolutize(x, base)),
        _ => {}
    }
}

fn is_relative_path(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('/') && !s.contains("://") && !s.starts_with("data:") && !s.starts_with('$')
}

fn write_bytes(p: &Path, b: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    }
    std::fs::write(p, b).map_err(|e| format!("cannot write {}: {e}", p.display()))
}

fn write_file(p: &Path, s: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    }
    std::fs::write(p, s).map_err(|e| format!("cannot write {}: {e}", p.display()))
}

fn str_arg<'a>(a: &'a Value, k: &str) -> Result<&'a str, String> {
    a.get(k).and_then(|v| v.as_str()).ok_or_else(|| format!("missing string argument '{k}'"))
}

fn text(s: &str) -> Value {
    json!({ "type": "text", "text": s })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap()
}

fn error(id: Value, code: i64, msg: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } }).to_string()
}

/// Standard base64 (no dependency needed for one encoder).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> Server {
        let mut s = Server::new();
        s.workdir = std::env::temp_dir().join(format!("anikit-mcp-test-{}", std::process::id()));
        s
    }

    fn call(s: &mut Server, method: &str, params: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }).to_string();
        serde_json::from_str(&s.handle_line(&line).unwrap()).unwrap()
    }

    #[test]
    fn handshake_and_listing() {
        let mut s = server();
        let r = call(&mut s, "initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }));
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["serverInfo"]["name"], "anikit-video");
        assert!(s.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        let r = call(&mut s, "tools/list", json!({}));
        let names: Vec<&str> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        for n in ["list_templates", "get_template_schema", "create_from_template", "validate_spec", "render_still", "render_contact_sheet", "render_video", "get_prompt", "critique_spec"] {
            assert!(names.contains(&n), "missing {n}");
        }
        let r = call(&mut s, "prompts/get", json!({ "name": "launch-video-generator" }));
        assert!(r["result"]["messages"][0]["content"]["text"].as_str().unwrap().starts_with("# Launch video generator"));
        let r = call(&mut s, "resources/read", json!({ "uri": "anikit://docs/spec" }));
        assert!(r["result"]["contents"][0]["text"].as_str().unwrap().contains("Anikit Video spec"));
        let r = call(&mut s, "nope", json!({}));
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn template_tools_round_trip() {
        let mut s = server();
        let t = |r: &Value| r["result"]["content"][0]["text"].as_str().unwrap().to_string();
        let r = call(&mut s, "tools/call", json!({ "name": "list_templates", "arguments": {} }));
        assert_eq!(t(&r).lines().count(), BUILTIN.len());
        assert!(t(&r).starts_with("launch — "), "{}", t(&r));

        let r = call(&mut s, "tools/call", json!({ "name": "get_template_schema", "arguments": { "name": "launch" } }));
        assert!(t(&r).contains("* tagline: string ≤10w"), "{}", t(&r));
        let r = call(&mut s, "tools/call", json!({ "name": "get_template_schema", "arguments": { "name": "launch", "full": true } }));
        let schema: Value = serde_json::from_str(&t(&r)).unwrap();
        assert_eq!(schema["required"], json!(["tagline", "features"]));

        let out = s.workdir.join("t.json");
        let r = call(&mut s, "tools/call", json!({ "name": "create_from_template", "arguments": { "template": "launch", "params": schema["example"], "format": "9:16", "out": out, "brand": { "name": "Acme", "colors": { "primary": "#FF5500" } } } }));
        assert_eq!(r["result"]["isError"], false, "{r}");
        // 94: the quality bar flags templates as flat 2D (BAR #5), and nothing else.
        assert!(t(&r).contains("valid · critique 94/100") && t(&r).contains("BAR #5"), "{}", t(&r));
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(spec["size"], json!([1080, 1920]));
        assert_eq!(spec["theme"]["colors"]["primary"], "#FF5500");

        let r = call(&mut s, "tools/call", json!({ "name": "validate_spec", "arguments": { "spec": { "scenes": [ { "layers": [ { "type": "txt", "text": "x" } ] } ] } } }));
        assert!(t(&r).starts_with("invalid\nerror scenes[0].layers[0]"), "{}", t(&r));
        let r = call(&mut s, "tools/call", json!({ "name": "critique_spec", "arguments": { "path": out } }));
        assert!(t(&r).starts_with("score 94/100"), "{}", t(&r));

        // Errors are tool errors with actionable text, not protocol errors.
        let r = call(&mut s, "tools/call", json!({ "name": "create_from_template", "arguments": { "template": "lanch", "params": {} } }));
        assert_eq!(r["result"]["isError"], true);
        assert!(t(&r).contains("did you mean 'launch'"));
        let r = call(&mut s, "tools/call", json!({ "name": "create_from_template", "arguments": { "template": "launch", "params": { "hok": "x" } } }));
        assert!(t(&r).contains("did you mean 'hook'"));
    }

    #[test]
    fn previews_render_in_process() {
        let mut s = server();
        let spec = json!({ "size": [320, 180], "scenes": [ { "duration": 1, "layers": [ { "text": "Hi" } ] }, { "duration": 1, "layers": [ { "text": "There" } ] } ] });
        let r = call(&mut s, "tools/call", json!({ "name": "render_contact_sheet", "arguments": { "spec": spec, "width": 160, "inline": true } }));
        assert_eq!(r["result"]["isError"], false, "{r}");
        let c = r["result"]["content"].as_array().unwrap();
        assert!(c.iter().any(|x| x["type"] == "image" && x["mimeType"] == "image/png"), "{r}");
        let r = call(&mut s, "tools/call", json!({ "name": "render_still", "arguments": { "spec": spec, "at": 0.5 } }));
        assert_eq!(r["result"]["content"].as_array().unwrap().len(), 1, "{r}");
    }

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }

    #[test]
    fn inline_specs_get_absolute_asset_paths() {
        let mut v = json!({ "scenes": [ { "layers": [ { "src": "shot.png" }, { "src": "https://x/y.png" } ] } ], "fonts": { "Brand": "fonts/brand.ttf" } });
        absolutize(&mut v, Path::new("/proj"));
        assert_eq!(v["scenes"][0]["layers"][0]["src"], "/proj/shot.png");
        assert_eq!(v["scenes"][0]["layers"][1]["src"], "https://x/y.png");
        assert_eq!(v["fonts"]["Brand"], "/proj/fonts/brand.ttf");
    }
}
