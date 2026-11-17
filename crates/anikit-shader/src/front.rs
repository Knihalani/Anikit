//! Source → validated naga module.
//!
//! The agent writes a Shadertoy-style `mainImage` (or a gl-transitions
//! `transition`) in GLSL, or the WGSL equivalent. We wrap it in a small
//! prelude that declares the Shadertoy inputs as private globals (the kernel
//! fills them per pixel quad), turn `iChannelN` / `uniform sampler2D` into
//! naga's separate texture + sampler pair, strip `uniform` from scalar and
//! vector uniforms (they become private globals too) and keep every user line
//! on its original line number so errors point at the agent's code.

use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};
use std::fmt;

/// What the shader computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `void mainImage(out vec4 fragColor, in vec2 fragCoord)`.
    Image,
    /// gl-transitions: `vec4 transition(vec2 uv)` with `getFromColor`,
    /// `getToColor`, `progress` and `ratio`; from = channel 0, to = channel 1.
    Transition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Glsl,
    Wgsl,
}

/// Type of a uniform as declared (or inferred).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UType {
    Float,
    Int,
    Bool,
    Vec(u8),
}

impl UType {
    pub fn components(self) -> usize {
        match self {
            UType::Vec(n) => n as usize,
            _ => 1,
        }
    }
}

/// A user uniform found in the source (`uniform float amp; // = 0.5`).
#[derive(Clone, Debug)]
pub struct DeclaredUniform {
    pub name: String,
    pub ty: UType,
    /// Default from a gl-transitions style `// = value` comment.
    pub default: Option<Vec<f32>>,
    /// Prose after the default in that comment (`// = 0.5 how far it bends`).
    pub doc: String,
}

/// The tunable uniforms a source declares, with defaults and docs.
pub fn declared_uniforms(src: &str) -> Vec<DeclaredUniform> {
    strip_uniforms(src).1
}

/// A compile error with a position in the agent's source.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderError {
    pub message: String,
    /// 1-based line and column in the user's source, when known.
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// The offending source line.
    pub snippet: Option<String>,
}

impl ShaderError {
    pub fn new(message: impl Into<String>) -> Self {
        ShaderError { message: message.into(), line: None, column: None, snippet: None }
    }
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "line {l}:{c}: {}", self.message)?,
            (Some(l), None) => write!(f, "line {l}: {}", self.message)?,
            _ => f.write_str(&self.message)?,
        }
        if let (Some(s), Some(l)) = (&self.snippet, self.line) {
            write!(f, "\n  {l:>4} | {s}")?;
            if let Some(c) = self.column {
                write!(f, "\n       | {}^", " ".repeat(c.saturating_sub(1) as usize))?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ShaderError {}

/// The prelude-wrapped source and how to map positions back.
pub struct Parsed {
    pub module: naga::Module,
    pub info: ModuleInfo,
    pub kind: Kind,
    pub lang: Lang,
    pub uniforms: Vec<DeclaredUniform>,
    /// Extra `uniform sampler2D` names, in channel order after the builtins.
    pub samplers: Vec<(String, u32)>,
    pub entry: naga::Handle<naga::Function>,
}

/// Shadertoy inputs every shader gets (private globals filled per quad).
pub const BUILTIN_GLSL: &str = "float iTime; float iTimeDelta; float iFrameRate; int iFrame; vec3 iResolution; vec4 iMouse; vec4 iDate; float iSampleRate; vec3 iChannelResolution[4]; float iChannelTime[4]; float progress; float ratio; vec4 iAudio; vec4 iBeat;";
const BUILTIN_WGSL: &str = "var<private> iTime: f32; var<private> iTimeDelta: f32; var<private> iFrameRate: f32; var<private> iFrame: i32; var<private> iResolution: vec3f; var<private> iMouse: vec4f; var<private> iDate: vec4f; var<private> iSampleRate: f32; var<private> iChannelResolution: array<vec3f, 4>; var<private> iChannelTime: array<f32, 4>; var<private> progress: f32; var<private> ratio: f32; var<private> iAudio: vec4f; var<private> iBeat: vec4f;";

pub const BUILTIN_NAMES: &[&str] =
    &["iTime", "iTimeDelta", "iFrameRate", "iFrame", "iResolution", "iMouse", "iDate", "iSampleRate", "iChannelResolution", "iChannelTime", "progress", "ratio", "iAudio", "iBeat"];

/// GLSL built-in functions, for "did you mean" suggestions.
const GLSL_FUNCS: &[&str] = &[
    "radians", "degrees", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "pow", "exp", "log", "exp2",
    "log2", "sqrt", "inversesqrt", "abs", "sign", "floor", "trunc", "round", "roundEven", "ceil", "fract", "mod", "modf", "min", "max", "clamp", "mix",
    "step", "smoothstep", "isnan", "isinf", "length", "distance", "dot", "cross", "normalize", "faceforward", "reflect", "refract", "matrixCompMult",
    "outerProduct", "transpose", "determinant", "inverse", "lessThan", "lessThanEqual", "greaterThan", "greaterThanEqual", "equal", "notEqual", "any",
    "all", "not", "texture", "textureLod", "texelFetch", "textureSize", "textureGrad", "dFdx", "dFdy", "fwidth", "floatBitsToInt", "floatBitsToUint",
    "intBitsToFloat", "uintBitsToFloat", "fma", "ldexp", "vec2", "vec3", "vec4", "ivec2", "ivec3", "ivec4", "uvec2", "uvec3", "uvec4", "bvec2",
    "bvec3", "bvec4", "mat2", "mat3", "mat4", "float", "int", "uint", "bool", "getFromColor", "getToColor", "mainImage",
];

/// Common mistakes with a direct fix.
const HINTS: &[(&str, &str)] = &[
    ("lerp", "use mix(a, b, t)"),
    ("saturate", "use clamp(x, 0.0, 1.0)"),
    ("frac", "use fract(x)"),
    ("atan2", "use atan(y, x)"),
    ("fmod", "use mod(x, y)"),
    ("float2", "use vec2"),
    ("float3", "use vec3"),
    ("float4", "use vec4"),
    ("tex2D", "use texture(iChannel0, uv)"),
    ("iGlobalTime", "use iTime"),
    ("gl_FragColor", "write to fragColor inside mainImage"),
    ("gl_FragCoord", "use the fragCoord argument of mainImage"),
];

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Replace whole-word identifiers (outside of nothing fancy: comments too,
/// which is harmless). Keeps newlines, so line numbers are stable.
fn replace_idents(src: &str, f: &dyn Fn(&str) -> Option<String>) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 64);
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if is_ident(c) && !c.is_ascii_digit() && (i == 0 || !is_ident(bytes[i - 1])) {
            let s = i;
            while i < bytes.len() && is_ident(bytes[i]) {
                i += 1;
            }
            let w = &src[s..i];
            match f(w) {
                Some(r) => out.push_str(&r),
                None => out.push_str(w),
            }
        } else {
            let ch_len = src[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            out.push_str(&src[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

fn glsl_utype(t: &str) -> Option<UType> {
    Some(match t {
        "float" => UType::Float,
        "int" | "uint" => UType::Int,
        "bool" => UType::Bool,
        "vec2" => UType::Vec(2),
        "vec3" => UType::Vec(3),
        "vec4" => UType::Vec(4),
        _ => return None,
    })
}

fn parse_default(comment: &str) -> Option<Vec<f32>> {
    // `// = 0.5`, `// = vec2(1.0, 2.0)`, `// = true`, with any prose after the value.
    let s = comment.trim_start_matches('/').trim();
    let s = s.strip_prefix('=')?.trim();
    let word: String = s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    let inner = if !word.is_empty() && !word.starts_with(|c: char| c.is_ascii_digit()) && s[word.len()..].trim_start().starts_with('(') {
        // A constructor: up to its closing parenthesis.
        let open = s.find('(')?;
        let close = open + s[open..].find(')')?;
        &s[open + 1..close]
    } else {
        // A bare number or bool: the first token.
        s.split(|c: char| c.is_whitespace() || c == ';' || c == ',').next().unwrap_or("")
    };
    match inner {
        "true" => return Some(vec![1.0]),
        "false" => return Some(vec![0.0]),
        _ => {}
    }
    let v: Option<Vec<f32>> = inner.split(',').map(|p| p.trim().trim_end_matches('f').parse::<f32>().ok()).collect();
    v.filter(|v| !v.is_empty())
}

/// The prose in a uniform's comment, after any `= value`.
fn parse_doc(comment: &str) -> String {
    let s = comment.trim_start_matches('/').trim();
    let rest = match s.strip_prefix('=') {
        None => s,
        Some(v) => {
            let v = v.trim_start();
            let word: String = v.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !word.is_empty() && !word.starts_with(|c: char| c.is_ascii_digit()) && v[word.len()..].trim_start().starts_with('(') {
                v.find(')').map_or("", |c| &v[c + 1..])
            } else {
                let end = v.find(|c: char| c.is_whitespace() || c == ';').unwrap_or(v.len());
                &v[end..]
            }
        }
    };
    rest.trim().trim_start_matches(['—', '–', '-', ':', ',', ';']).trim().to_string()
}

/// Rewrite user uniform declarations. Returns the new source plus what was found.
fn strip_uniforms(src: &str) -> (String, Vec<DeclaredUniform>, Vec<String>) {
    let mut out = String::with_capacity(src.len());
    let mut unis = Vec::new();
    let mut samplers = Vec::new();
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        let mut handled = false;
        if let Some(rest) = t.strip_prefix("uniform ") {
            let (decl, comment) = match rest.find("//") {
                Some(p) => (&rest[..p], &rest[p..]),
                None => (rest, ""),
            };
            if let Some(semi) = decl.find(';') {
                let body = decl[..semi].trim();
                let mut parts = body.split_whitespace().filter(|w| !matches!(*w, "lowp" | "mediump" | "highp"));
                if let (Some(ty), Some(name)) = (parts.next(), parts.next()) {
                    let name = name.split(['=', '[']).next().unwrap_or(name).trim().to_string();
                    if ty == "sampler2D" {
                        samplers.push(name);
                        out.push_str(&" ".repeat(line.trim_end_matches('\n').len()));
                        handled = true;
                    } else if let Some(ut) = glsl_utype(ty) {
                        if !BUILTIN_NAMES.contains(&name.as_str()) {
                            let default = parse_default(comment).or_else(|| {
                                // `uniform float x = 1.0;`
                                body.split_once('=').and_then(|(_, v)| parse_default(&format!("= {v}")))
                            });
                            unis.push(DeclaredUniform { name: name.clone(), ty: ut, default, doc: parse_doc(comment) });
                            // Keep the declaration (minus `uniform` and any initialiser) on its line.
                            let indent = &line[..line.len() - t.len()];
                            let decl_txt = format!("{indent}{ty} {name};");
                            out.push_str(&decl_txt);
                            let pad = line.trim_end_matches('\n').len().saturating_sub(decl_txt.len());
                            out.push_str(&" ".repeat(pad));
                        } else {
                            // Re-declaring a builtin (`uniform float iTime;`): drop it.
                            out.push_str(&" ".repeat(line.trim_end_matches('\n').len()));
                        }
                        handled = true;
                    }
                }
            }
            if handled && line.ends_with('\n') {
                out.push('\n');
            }
        } else if t.starts_with("#version") || t.starts_with("precision ") {
            out.push_str(&" ".repeat(line.trim_end_matches('\n').len()));
            if line.ends_with('\n') {
                out.push('\n');
            }
            handled = true;
        }
        if !handled {
            out.push_str(line);
        }
    }
    (out, unis, samplers)
}

fn detect_lang(src: &str) -> Lang {
    let has = |w: &str| src.contains(w);
    if (has("fn mainImage") || has("fn transition")) && !has("void mainImage") && !has("vec4 transition") {
        Lang::Wgsl
    } else {
        Lang::Glsl
    }
}

fn detect_kind(src: &str, lang: Lang) -> Kind {
    let image = match lang {
        Lang::Glsl => src.contains("mainImage"),
        Lang::Wgsl => src.contains("fn mainImage"),
    };
    if !image && src.contains("transition") {
        Kind::Transition
    } else {
        Kind::Image
    }
}

/// Extra uniforms the caller supplies that the source does not declare.
pub struct Extra<'a> {
    pub name: &'a str,
    pub ty: UType,
}

/// Parse and validate. `extra` declares spec uniforms the source didn't.
pub fn parse(src: &str, extra: &[Extra]) -> Result<Parsed, ShaderError> {
    let lang = detect_lang(src);
    let kind = detect_kind(src, lang);
    if kind == Kind::Image && !src.contains("mainImage") {
        let hint = if src.contains("void main") { " (Shadertoy style: rename `main` and write the colour to the `fragColor` argument)" } else { "" };
        return Err(ShaderError::new(format!("no mainImage function: write `void mainImage(out vec4 fragColor, in vec2 fragCoord) {{ ... }}`{hint}")));
    }
    match lang {
        Lang::Glsl => parse_glsl(src, kind, extra),
        Lang::Wgsl => parse_wgsl(src, kind, extra),
    }
}

fn channel_expr(i: u32) -> String {
    format!("sampler2D(_ak_t{i},_ak_s{i})")
}

fn parse_glsl(src: &str, kind: Kind, extra: &[Extra]) -> Result<Parsed, ShaderError> {
    let (body, mut unis, sampler_names) = strip_uniforms(src);
    // Named samplers take the channels after the ones a transition uses.
    let first = if kind == Kind::Transition { 2 } else { 0 };
    let samplers: Vec<(String, u32)> = sampler_names.iter().enumerate().map(|(i, n)| (n.clone(), (first + i as u32).min(3))).collect();
    let body = replace_idents(&body, &|w| {
        if let Some(n) = w.strip_prefix("iChannel") {
            if let Ok(i) = n.parse::<u32>() {
                if i < 4 {
                    return Some(channel_expr(i));
                }
            }
        }
        match w {
            "texture2D" => Some("texture".into()),
            "texture2DLod" | "texture2DLodEXT" => Some("textureLod".into()),
            _ => samplers.iter().find(|(n, _)| n == w).map(|(_, c)| channel_expr(*c)),
        }
    });
    let mut pre = String::from("#version 450\n");
    pre.push_str(BUILTIN_GLSL);
    for i in 0..4 {
        pre.push_str(&format!(" layout(set=0,binding={i}) uniform texture2D _ak_t{i}; layout(set=0,binding={}) uniform sampler _ak_s{i};", i + 4));
    }
    for e in extra {
        if unis.iter().any(|u| u.name == e.name) {
            continue;
        }
        let ty = match e.ty {
            UType::Float => "float".to_string(),
            UType::Int => "int".to_string(),
            UType::Bool => "bool".to_string(),
            UType::Vec(n) => format!("vec{n}"),
        };
        pre.push_str(&format!(" {ty} {};", e.name));
        unis.push(DeclaredUniform { name: e.name.to_string(), ty: e.ty, default: None, doc: String::new() });
    }
    if kind == Kind::Transition {
        pre.push_str(&format!(
            " vec4 getFromColor(vec2 uv) {{ return texture({}, uv); }} vec4 getToColor(vec2 uv) {{ return texture({}, uv); }}",
            channel_expr(0),
            channel_expr(1)
        ));
    }
    pre.push('\n');
    let pre_lines = pre.matches('\n').count() as u32;
    let full = format!("{pre}{body}\nvoid main() {{}}\n");

    let mut fe = naga::front::glsl::Frontend::default();
    let opts = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);
    let module = fe.parse(&opts, &full).map_err(|e| {
        let err = e.errors.first();
        let msg = err.map(|e| e.kind.to_string()).unwrap_or_else(|| "parse error".into());
        let loc = err.map(|e| e.meta.location(&full));
        located(src, &full, pre_lines, msg, loc)
    })?;
    finish(module, src, &full, pre_lines, kind, Lang::Glsl, unis, samplers)
}

fn parse_wgsl(src: &str, kind: Kind, extra: &[Extra]) -> Result<Parsed, ShaderError> {
    let mut pre = String::from(BUILTIN_WGSL);
    for i in 0..4 {
        pre.push_str(&format!(" @group(0) @binding({i}) var iChannel{i}: texture_2d<f32>; @group(0) @binding({}) var iSampler{i}: sampler;", i + 4));
    }
    let mut unis = Vec::new();
    for e in extra {
        let ty = match e.ty {
            UType::Float => "f32".to_string(),
            UType::Int => "i32".to_string(),
            UType::Bool => "bool".to_string(),
            UType::Vec(n) => format!("vec{n}f"),
        };
        if src.contains(&format!("var<private> {}", e.name)) {
            unis.push(DeclaredUniform { name: e.name.to_string(), ty: e.ty, default: None, doc: String::new() });
            continue;
        }
        pre.push_str(&format!(" var<private> {}: {ty};", e.name));
        unis.push(DeclaredUniform { name: e.name.to_string(), ty: e.ty, default: None, doc: String::new() });
    }
    if kind == Kind::Transition {
        pre.push_str(" fn getFromColor(uv: vec2f) -> vec4f { return textureSampleLevel(iChannel0, iSampler0, uv, 0.0); } fn getToColor(uv: vec2f) -> vec4f { return textureSampleLevel(iChannel1, iSampler1, uv, 0.0); }");
    }
    pre.push('\n');
    let pre_lines = pre.matches('\n').count() as u32;
    let full = format!("{pre}{src}\n@fragment fn _ak_main() -> @location(0) vec4f {{ return vec4f(0.0); }}\n");
    let module = naga::front::wgsl::parse_str(&full).map_err(|e| {
        let loc = e.location(&full);
        located(src, &full, pre_lines, e.message().to_string(), loc)
    })?;
    finish(module, src, &full, pre_lines, kind, Lang::Wgsl, unis, Vec::new())
}

#[allow(clippy::too_many_arguments)]
fn finish(
    module: naga::Module,
    src: &str,
    full: &str,
    pre_lines: u32,
    kind: Kind,
    lang: Lang,
    uniforms: Vec<DeclaredUniform>,
    samplers: Vec<(String, u32)>,
) -> Result<Parsed, ShaderError> {
    let flags = ValidationFlags::all() - ValidationFlags::CONTROL_FLOW_UNIFORMITY;
    let info = Validator::new(flags, Capabilities::all()).validate(&module).map_err(|e| {
        let loc = e.spans().next().map(|(s, _)| s.location(full));
        let mut msg = e.as_inner().to_string();
        let mut source = std::error::Error::source(e.as_inner());
        while let Some(s) = source {
            msg.push_str(": ");
            msg.push_str(&s.to_string());
            source = s.source();
        }
        located(src, full, pre_lines, msg, loc)
    })?;
    let want = match kind {
        Kind::Image => "mainImage",
        Kind::Transition => "transition",
    };
    let entry = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(want))
        .map(|(h, _)| h)
        .ok_or_else(|| {
            ShaderError::new(match kind {
                Kind::Image => "no mainImage function: write `void mainImage(out vec4 fragColor, in vec2 fragCoord) { ... }`",
                Kind::Transition => "no transition function: write `vec4 transition(vec2 uv) { ... }`",
            })
        })?;
    Ok(Parsed { module, info, kind, lang, uniforms, samplers, entry })
}

/// Turn a position in the wrapped source into one in the user's source and
/// add a suggestion for unknown names.
pub fn located(src: &str, full: &str, pre_lines: u32, msg: String, loc: Option<naga::SourceLocation>) -> ShaderError {
    let mut msg = msg;
    let ident = quoted_ident(&msg);
    if let Some(w) = ident.as_deref() {
        if let Some((_, h)) = HINTS.iter().find(|(k, _)| *k == w) {
            msg.push_str(&format!(" ({h})"));
        } else if let Some(s) = did_you_mean(w, src) {
            msg.push_str(&format!(" (did you mean `{s}`?)"));
        }
    }
    let Some(loc) = loc else {
        return ShaderError::new(msg);
    };
    let _ = full;
    if loc.line_number <= pre_lines {
        return ShaderError::new(msg);
    }
    let line = loc.line_number - pre_lines;
    let snippet = src.lines().nth(line as usize - 1).map(|s| s.to_string());
    if snippet.is_none() {
        return ShaderError { message: msg, line: None, column: None, snippet: None };
    }
    ShaderError { message: msg, line: Some(line), column: Some(loc.line_position), snippet }
}

fn quoted_ident(msg: &str) -> Option<String> {
    for q in ['`', '"', '\''] {
        if let Some(a) = msg.find(q) {
            if let Some(b) = msg[a + 1..].find(q) {
                let w = &msg[a + 1..a + 1 + b];
                if !w.is_empty() && w.bytes().all(is_ident) {
                    return Some(w.to_string());
                }
            }
        }
    }
    None
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let c = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + c);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Closest builtin, Shadertoy input or identifier in the source.
pub fn did_you_mean(w: &str, src: &str) -> Option<String> {
    let mut cands: Vec<String> = GLSL_FUNCS.iter().chain(BUILTIN_NAMES).map(|s| s.to_string()).collect();
    cands.extend((0..4).map(|i| format!("iChannel{i}")));
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if is_ident(bytes[i]) && !bytes[i].is_ascii_digit() {
            let s = i;
            while i < bytes.len() && is_ident(bytes[i]) {
                i += 1;
            }
            cands.push(src[s..i].to_string());
        } else {
            i += 1;
        }
    }
    let max = (w.len() / 3).clamp(1, 3);
    cands
        .into_iter()
        .filter(|c| c != w)
        .map(|c| (edit_distance(w, &c), c))
        .filter(|(d, _)| *d <= max)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.len().cmp(&b.1.len())))
        .map(|(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_defaults() {
        let (s, u, smp) = strip_uniforms("uniform float amp; // = 0.5\nuniform vec2 dir; // = vec2(1.0, -1.0)\nuniform sampler2D disp;\nvoid f(){}\n");
        assert_eq!(u.len(), 2);
        assert_eq!(u[0].default, Some(vec![0.5]));
        assert_eq!(u[1].default, Some(vec![1.0, -1.0]));
        assert_eq!(parse_default("// = 1.0   overall fold amount (0 = a plain slab)"), Some(vec![1.0]));
        assert_eq!(parse_default("// = vec3(1.0, 0.5, 0.2) warm tint"), Some(vec![1.0, 0.5, 0.2]));
        assert_eq!(parse_default("// = -2.5; comment"), Some(vec![-2.5]));
        assert_eq!(parse_default("// = true"), Some(vec![1.0]));
        assert_eq!(smp, vec!["disp".to_string()]);
        assert_eq!(s.lines().count(), 4);
    }

    #[test]
    fn suggestions() {
        assert_eq!(did_you_mean("smoothStep", "").as_deref(), Some("smoothstep"));
        assert_eq!(did_you_mean("iTim", "").as_deref(), Some("iTime"));
    }
}
