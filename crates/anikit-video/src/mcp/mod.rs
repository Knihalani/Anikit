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

const INSTRUCTIONS: &str = "Anikit Video renders MP4s from a JSON spec, natively and fast. \
Workflow: 1) list_templates, then get_template_schema; 2) create_from_template (with a brand kit and a format: 16:9, 9:16 or 1:1) or write a spec yourself \
(read resource anikit://docs/spec); 3) validate_spec and critique_spec, fixing every warning; 4) render_contact_sheet or render_still to look at it \
(cheap); 5) render_video. For a launch video, get_prompt('launch-video-generator') first. Motion rules: get_prompt('motion-rules').";

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
            "list_templates" => {
                let v: Vec<Value> = BUILTIN.iter().filter_map(|(n, _)| Template::builtin(n).ok()).map(|t| t.summary()).collect();
                Ok(vec![text(&pretty(&Value::Array(v)))])
            }
            "get_template_schema" => {
                let t = Template::builtin(str_arg(a, "name")?)?;
                let mut s = t.param_schema();
                s["example"] = t.example_params();
                Ok(vec![text(&pretty(&s)), text(&prompts::template_guide(&t))])
            }
            "create_from_template" => {
                let t = Template::builtin(str_arg(a, "template")?)?;
                let brand = match a.get("brand") {
                    Some(Value::String(p)) => agent_cli::load_brand(Some(p))?,
                    Some(v @ Value::Object(_)) => Brand::from_value(v)?,
                    _ => Brand::default(),
                };
                let format = Format::parse(a.get("format").and_then(|f| f.as_str()).unwrap_or("16:9"))?;
                let params = a.get("params").cloned().unwrap_or(json!({}));
                let spec = t.expand(&params, &brand, format).map_err(|e| format!("template parameters need fixing:\n- {}", e.join("\n- ")))?;
                let r = critique(&spec);
                let v = validate(spec.clone(), &std::env::current_dir().unwrap_or_default());
                let mut out = vec![];
                if let Some(p) = a.get("out").and_then(|o| o.as_str()) {
                    write_file(Path::new(p), &pretty(&spec))?;
                    out.push(text(&format!("wrote {p}")));
                } else {
                    out.push(text(&pretty(&spec)));
                }
                let issues: Vec<String> = ["errors", "warnings"].iter().flat_map(|k| v[*k].as_array().cloned().unwrap_or_default()).map(|d| format!("{}: {}", d["path"].as_str().unwrap_or(""), d["message"].as_str().unwrap_or(""))).collect();
                if !issues.is_empty() {
                    out.push(text(&format!("spec check (asset paths resolve against the server's working directory):\n{}", issues.join("\n"))));
                }
                out.push(text(&format!("duration {:.1} s · critique {}/100\n{}", r.duration, r.score, r.notes.iter().map(|n| format!("{} {}: {} → {}", n.severity, n.path, n.message, n.fix)).collect::<Vec<_>>().join("\n"))));
                Ok(out)
            }
            "critique_spec" => {
                let (spec, _) = self.spec_arg(a)?;
                let r = critique(&spec);
                Ok(vec![text(&r.to_text()), text(&pretty(&serde_json::to_value(&r).unwrap()))])
            }
            "validate_spec" => {
                let (spec, path) = self.spec_arg(a)?;
                let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
                Ok(vec![text(&pretty(&validate(spec, &base)))])
            }
            "render_still" => {
                let (spec, path) = self.spec_arg(a)?;
                let at = a.get("at").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                let out = self.out_path(a, "png");
                let p = crate::preview::Preview::new(spec, path.parent().unwrap_or(Path::new(".")))?;
                let img = p.frame(at, a.get("width").and_then(|v| v.as_u64()).map(|w| w as u32));
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
                let cell = a.get("width").and_then(|v| v.as_u64()).unwrap_or(480) as u32;
                let img = p.sheet(times, cell);
                write_bytes(&out, &crate::preview::png(&img))?;
                let mut res = self.image_result(&out, a)?;
                let scenes: Vec<String> = p.ev.scenes().iter().map(|s| format!("{} {:.1}–{:.1} s", s.id, s.start, s.end)).collect();
                res.insert(1, text(&format!("frames at each scene's settled moment (70%): {}", scenes.join(" · "))));
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
        let mut v = vec![text(&format!("wrote {}", out.display()))];
        if a.get("inline") != Some(&Value::Bool(false)) {
            if let Ok(bytes) = std::fs::read(out) {
                if bytes.len() < 1_500_000 {
                    v.push(json!({ "type": "image", "data": base64(&bytes), "mimeType": "image/png" }));
                } else {
                    v.push(text("image too large to inline; pass a smaller \"width\" or read the file"));
                }
            }
        }
        Ok(v)
    }
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
    ("anikit://docs/spec", "Spec reference", "The complete Anikit Video JSON spec (layers, animation, presets, transitions).", include_str!("../../../../docs/video/spec.md")),
    ("anikit://docs/agents", "Agent quick start", "How to make a great video with Anikit Video as an agent.", include_str!("../../../../docs/video/AGENTS.md")),
    ("anikit://prompts/motion-rules", "Motion rules", "Timing, hierarchy, easing, reading time, safe areas.", include_str!("../../../../prompts/motion-rules.md")),
];

fn spec_input() -> Value {
    json!({
        "spec": { "type": "object", "description": "The spec as a JSON object (see resource anikit://docs/spec)." },
        "path": { "type": "string", "description": "Or: path to a spec file." },
        "base_dir": { "type": "string", "description": "Directory that relative asset paths in an inline spec are resolved against (default: the server's working directory)." }
    })
}

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        b.extend(e.clone());
    }
    base
}

/// Tool declarations.
pub fn tools() -> Vec<Value> {
    let obj = |props: Value, req: &[&str]| json!({ "type": "object", "properties": props, "required": req });
    vec![
        json!({ "name": "list_templates", "description": "List the video templates (launch, feature, changelog, demo, teaser, data-story, testimonial, tutorial, countdown, logo-reveal) with their required parameters.", "inputSchema": obj(json!({}), &[]) }),
        json!({ "name": "get_template_schema", "description": "JSON Schema of a template's parameters, with example values, word limits and a guide to writing them well.", "inputSchema": obj(json!({ "name": { "type": "string", "description": "Template name, e.g. 'launch'." } }), &["name"]) }),
        json!({ "name": "create_from_template", "description": "Expand a template with parameters, a brand kit and a format into a complete spec. Returns the spec (or writes it to 'out') plus a critique. Errors name the parameter and suggest fixes.", "inputSchema": obj(json!({
            "template": { "type": "string" },
            "params": { "type": "object", "description": "Parameter values (see get_template_schema)." },
            "brand": { "description": "Brand kit object ({name, theme, colors{primary,…}, fonts, logo, voice}) or a path to brand.json.", "type": ["object", "string"] },
            "format": { "type": "string", "description": "16:9 (default), 9:16, 1:1, 4:5, 720p, 4k or WxH." },
            "out": { "type": "string", "description": "Optional path to write spec.json." }
        }), &["template", "params"]) }),
        json!({ "name": "validate_spec", "description": "Validate a spec: errors and warnings with JSON paths and 'did you mean' suggestions, plus duration and scene timings.", "inputSchema": obj(spec_input(), &[]) }),
        json!({ "name": "critique_spec", "description": "Heuristic design review with no rendering: reading time per text (about 3 words/s + 1 s), the hook within 2 s, contrast, safe areas, font sizes, pacing and density. Returns a score and fixes.", "inputSchema": obj(spec_input(), &[]) }),
        json!({ "name": "render_still", "description": "Render one frame to PNG and return it (inline when small). Cheap: use it to look at your work.", "inputSchema": obj(with(spec_input(), json!({
            "at": { "type": "number", "description": "Time in seconds (default 1)." },
            "width": { "type": "integer", "description": "Downscale to this width (e.g. 480) for a cheap preview." },
            "inline": { "type": "boolean", "description": "Return the image inline (default true)." },
            "out": { "type": "string" }
        })), &[]) }),
        json!({ "name": "render_contact_sheet", "description": "Render a grid of frames, one per scene at its settled moment, as one PNG: the fastest way to review a whole video.", "inputSchema": obj(with(spec_input(), json!({
            "frames": { "type": "integer", "description": "Number of evenly spaced frames (default: one per scene, at its settled moment)." },
            "times": { "type": "array", "items": { "type": "number" }, "description": "Exact times in seconds instead." },
            "width": { "type": "integer", "description": "Width of each cell (default 480)." },
            "inline": { "type": "boolean" }, "out": { "type": "string" }
        })), &[]) }),
        json!({ "name": "render_video", "description": "Render the final video (MP4 by default; the extension of 'out' picks the format). Returns the path.", "inputSchema": obj(with(spec_input(), json!({
            "out": { "type": "string", "description": "Output path, e.g. out.mp4." },
            "quality": { "type": "string", "description": "draft, standard (default) or high." }
        })), &[]) }),
        json!({ "name": "get_prompt", "description": "Get a prompt from the library: launch-video-generator, script-writer, storyboarder, critic, copy-guard, brand-kit, motion-rules, or template:<name> guides.", "inputSchema": obj(json!({ "name": { "type": "string" } }), &["name"]) }),
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
        let r = call(&mut s, "tools/call", json!({ "name": "list_templates", "arguments": {} }));
        let list: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(list.as_array().unwrap().len(), BUILTIN.len());

        let r = call(&mut s, "tools/call", json!({ "name": "get_template_schema", "arguments": { "name": "launch" } }));
        let schema: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(schema["required"], json!(["hook", "tagline", "features"]));

        let params = schema["example"].clone();
        let r = call(&mut s, "tools/call", json!({ "name": "create_from_template", "arguments": { "template": "launch", "params": params, "format": "9:16", "brand": { "name": "Acme", "colors": { "primary": "#FF5500" } } } }));
        assert_eq!(r["result"]["isError"], false, "{r}");
        let spec: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(spec["size"], json!([1080, 1920]));
        assert_eq!(spec["theme"]["colors"]["primary"], "#FF5500");

        let r = call(&mut s, "tools/call", json!({ "name": "validate_spec", "arguments": { "spec": { "scenes": [ { "layers": [ { "type": "txt", "text": "x" } ] } ] } } }));
        let v: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["ok"], false, "{v}");
        assert!(v["errors"][0]["path"].as_str().unwrap().contains("layers[0]"), "{v}");
        let r = call(&mut s, "tools/call", json!({ "name": "critique_spec", "arguments": { "spec": spec } }));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().starts_with("score"));

        // Errors are tool errors with actionable text, not protocol errors.
        let r = call(&mut s, "tools/call", json!({ "name": "create_from_template", "arguments": { "template": "lanch", "params": {} } }));
        assert_eq!(r["result"]["isError"], true);
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("did you mean 'launch'"));
        let r = call(&mut s, "tools/call", json!({ "name": "create_from_template", "arguments": { "template": "launch", "params": { "hok": "x" } } }));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("did you mean 'hook'"));
    }

    #[test]
    fn previews_render_in_process() {
        let mut s = server();
        let spec = json!({ "size": [320, 180], "scenes": [ { "duration": 1, "layers": [ { "text": "Hi" } ] }, { "duration": 1, "layers": [ { "text": "There" } ] } ] });
        let r = call(&mut s, "tools/call", json!({ "name": "render_contact_sheet", "arguments": { "spec": spec, "width": 160 } }));
        assert_eq!(r["result"]["isError"], false, "{r}");
        let c = r["result"]["content"].as_array().unwrap();
        assert!(c.iter().any(|x| x["type"] == "image" && x["mimeType"] == "image/png"), "{r}");
        let r = call(&mut s, "tools/call", json!({ "name": "render_still", "arguments": { "spec": spec, "at": 0.5, "inline": false } }));
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
