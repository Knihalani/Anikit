//! Code: an editor window with syntax highlighting, typing, highlights,
//! diffs, scrolling and a cursor; and a terminal.
//!
//! Highlighting is a small lexer per language family (keywords, types,
//! functions, strings, numbers, comments), done once in `prepare`. Each frame
//! only lays out visible tokens.

use super::polish::{self, Surface};
use super::style::*;
use super::text::{draw, Face, TextStyle};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Op, Paint, Rgba, IDENTITY};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Keyword,
    String,
    Number,
    Comment,
    Function,
    Type,
    Punct,
    Constant,
}

/// One coloured run.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub kind: Tok,
    /// Explicit colour (from the TEXT highlighter); `None` = theme colour for `kind`.
    pub color: Option<Rgba>,
}

struct Lang {
    keywords: &'static [&'static str],
    constants: &'static [&'static str],
    line_comment: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
}

fn lang(name: &str) -> Lang {
    const C_LIKE: &[&str] = &["//"];
    match name.to_ascii_lowercase().as_str() {
        "rust" | "rs" => Lang {
            keywords: &["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type", "unsafe", "use", "where", "while"],
            constants: &["true", "false", "None", "Some", "Ok", "Err"],
            line_comment: C_LIKE,
            block: Some(("/*", "*/")),
        },
        "python" | "py" => Lang {
            keywords: &["and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "self"],
            constants: &["True", "False", "None"],
            line_comment: &["#"],
            block: None,
        },
        "go" => Lang {
            keywords: &["break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type", "var"],
            constants: &["true", "false", "nil", "iota"],
            line_comment: C_LIKE,
            block: Some(("/*", "*/")),
        },
        "bash" | "sh" | "shell" | "zsh" => Lang {
            keywords: &["if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "function", "in", "export", "local", "return", "echo", "cd", "sudo", "npm", "npx", "cargo", "git", "pip", "curl"],
            constants: &["true", "false"],
            line_comment: &["#"],
            block: None,
        },
        "sql" => Lang {
            keywords: &["select", "from", "where", "and", "or", "not", "insert", "into", "values", "update", "set", "delete", "create", "table", "join", "left", "right", "inner", "on", "group", "by", "order", "limit", "as", "having", "SELECT", "FROM", "WHERE", "AND", "OR", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "TABLE", "JOIN", "LEFT", "ON", "GROUP", "BY", "ORDER", "LIMIT", "AS"],
            constants: &["null", "NULL", "true", "false"],
            line_comment: &["--"],
            block: Some(("/*", "*/")),
        },
        "json" => Lang { keywords: &[], constants: &["true", "false", "null"], line_comment: &[], block: None },
        "css" => Lang { keywords: &["@media", "@import", "!important"], constants: &[], line_comment: &[], block: Some(("/*", "*/")) },
        "html" | "xml" | "svg" => Lang { keywords: &[], constants: &[], line_comment: &[], block: Some(("<!--", "-->")) },
        "yaml" | "yml" | "toml" => Lang { keywords: &[], constants: &["true", "false", "null"], line_comment: &["#"], block: None },
        // JavaScript / TypeScript / C-family default.
        _ => Lang {
            keywords: &["abstract", "as", "async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else", "enum", "export", "extends", "finally", "for", "from", "function", "if", "implements", "import", "in", "instanceof", "interface", "let", "new", "of", "private", "protected", "public", "readonly", "return", "static", "switch", "this", "throw", "try", "type", "typeof", "var", "void", "while", "yield", "int", "float", "double", "char", "long", "unsigned", "struct", "auto", "include"],
            constants: &["true", "false", "null", "undefined", "NaN", "Infinity"],
            line_comment: C_LIKE,
            block: Some(("/*", "*/")),
        },
    }
}

/// Tokenise `code` into lines of coloured spans.
pub fn highlight(code: &str, language: &str) -> Vec<Vec<Span>> {
    let l = lang(language);
    let markup = matches!(language, "html" | "xml" | "svg");
    let json = language == "json";
    let mut out = Vec::new();
    let mut in_block = false;
    for src in code.split('\n') {
        let chars: Vec<char> = src.chars().collect();
        let mut spans: Vec<Span> = Vec::new();
        let push = |spans: &mut Vec<Span>, s: String, k: Tok| {
            if s.is_empty() {
                return;
            }
            if let Some(last) = spans.last_mut() {
                if last.kind == k {
                    last.text.push_str(&s);
                    return;
                }
            }
            spans.push(Span { text: s, kind: k, color: None });
        };
        let mut i = 0;
        let rest = |i: usize| chars[i..].iter().collect::<String>();
        while i < chars.len() {
            if in_block {
                let (_, end) = l.block.unwrap();
                let r = rest(i);
                if let Some(p) = r.find(end) {
                    let n = r[..p + end.len()].chars().count();
                    push(&mut spans, r.chars().take(n).collect(), Tok::Comment);
                    i += n;
                    in_block = false;
                } else {
                    push(&mut spans, r, Tok::Comment);
                    i = chars.len();
                }
                continue;
            }
            let r = rest(i);
            if let Some((start, _)) = l.block {
                if r.starts_with(start) {
                    in_block = true;
                    push(&mut spans, start.to_string(), Tok::Comment);
                    i += start.chars().count();
                    continue;
                }
            }
            if l.line_comment.iter().any(|c| r.starts_with(c)) && !(language == "bash" && r.starts_with("#!") && i > 0) {
                push(&mut spans, r, Tok::Comment);
                break;
            }
            let c = chars[i];
            if c == '"' || c == '\'' || c == '`' {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != c {
                    if chars[j] == '\\' {
                        j += 1;
                    }
                    j += 1;
                }
                let j = (j + 1).min(chars.len());
                let s: String = chars[i..j].iter().collect();
                // JSON keys read as properties, not strings.
                let is_key = json && chars[j..].iter().find(|c| !c.is_whitespace()) == Some(&':');
                push(&mut spans, s, if is_key { Tok::Function } else { Tok::String });
                i = j;
                continue;
            }
            if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
                let mut j = i;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '_') {
                    j += 1;
                }
                push(&mut spans, chars[i..j].iter().collect(), Tok::Number);
                i = j;
                continue;
            }
            if c.is_alphabetic() || c == '_' || c == '$' || c == '@' {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '$' || (markup && chars[j] == '-')) {
                    j += 1;
                }
                let w: String = chars[i..j].iter().collect();
                let next = chars[j..].iter().find(|c| !c.is_whitespace()).copied();
                let prev = if i > 0 { chars[i - 1] } else { ' ' };
                let kind = if l.keywords.contains(&w.as_str()) {
                    Tok::Keyword
                } else if l.constants.contains(&w.as_str()) {
                    Tok::Constant
                } else if markup && (prev == '<' || prev == '/') {
                    Tok::Keyword
                } else if (markup && next == Some('=')) || next == Some('(') || (next == Some('!') && language == "rust") {
                    Tok::Function
                } else if w.chars().next().is_some_and(|c| c.is_uppercase()) && w.chars().any(|c| c.is_lowercase()) {
                    Tok::Type
                } else if w.len() > 1 && w.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit()) {
                    Tok::Constant
                } else {
                    Tok::Plain
                };
                push(&mut spans, w, kind);
                i = j;
                continue;
            }
            let kind = if c.is_whitespace() { Tok::Plain } else { Tok::Punct };
            push(&mut spans, c.to_string(), kind);
            i += 1;
        }
        out.push(spans);
    }
    out
}

/// Syntax colours for a theme.
pub fn syntax_color(k: Tok, dark: bool) -> Rgba {
    let s = if dark {
        match k {
            Tok::Plain => "#e4e5ee",
            Tok::Keyword => "#c4a5ff",
            Tok::String => "#a4e8a6",
            Tok::Number => "#ffb878",
            Tok::Comment => "#636878",
            Tok::Function => "#7fb4ff",
            Tok::Type => "#62dbe6",
            Tok::Punct => "#9aa0b2",
            Tok::Constant => "#ff8fa8",
        }
    } else {
        match k {
            Tok::Plain => "#1f2330",
            Tok::Keyword => "#7a3fd8",
            Tok::String => "#1a8a3a",
            Tok::Number => "#c25e00",
            Tok::Comment => "#9399a8",
            Tok::Function => "#1f5fd0",
            Tok::Type => "#0b8796",
            Tok::Punct => "#5c6273",
            Tok::Constant => "#c8274f",
        }
    };
    parse_color(s).unwrap()
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Highlight {
    /// 1-based line numbers (inclusive ranges as `[a, b]` also work via `range`).
    #[serde(default)]
    pub lines: Vec<usize>,
    #[serde(default)]
    pub range: Option<[usize; 2]>,
    /// When it turns on (s).
    #[serde(default)]
    pub at: f32,
    /// When it turns off (s); omitted = stays.
    #[serde(default)]
    pub until: Option<f32>,
    /// Dim the other lines while active.
    #[serde(default = "yes")]
    pub dim: bool,
}
fn yes() -> bool {
    true
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Scroll {
    /// 1-based line to bring to the top.
    pub line: usize,
    pub at: f32,
    #[serde(default = "scroll_dur")]
    pub duration: f32,
}
fn scroll_dur() -> f32 {
    0.6
}

/// A selection sweeping across part of a line (1-based line, 0-based columns).
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub line: usize,
    #[serde(default)]
    pub from: usize,
    /// End column (exclusive); default: end of line.
    #[serde(default)]
    pub to: Option<usize>,
    #[serde(default)]
    pub at: f32,
    #[serde(default)]
    pub until: Option<f32>,
}

#[derive(Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Chrome {
    Mac,
    Minimal,
    None,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CodeBlock {
    pub code: String,
    pub language: String,
    /// Window title (file name).
    pub title: String,
    pub font_size: f32,
    pub line_numbers: bool,
    /// "mac" (traffic lights), "minimal" or "none".
    pub chrome: Chrome,
    /// Type the code in: characters per second (0 = shown at once).
    pub typing: f32,
    /// When typing starts (s).
    pub delay: f32,
    /// Lines whose text starts with `+`/`-` are added/removed (diff view).
    pub diff: bool,
    pub highlights: Vec<Highlight>,
    pub scroll: Vec<Scroll>,
    /// Show a blinking caret at the typing position.
    pub cursor: bool,
    /// Background colour of the window.
    pub background: Option<Color>,
    /// Soft coloured glow behind the window.
    pub glow: Option<Color>,
    /// Text selections that sweep in: `[{ "line": 3, "from": 6, "to": 18, "at": 2.0 }]`.
    pub selections: Vec<Selection>,
    /// Code minimap down the right edge.
    pub minimap: bool,
    /// Status bar (language, line and column) along the bottom.
    pub status_bar: bool,
    /// Tint the line the caret is on.
    pub active_line: bool,
    /// Window finish: solid (default), glass, outline, flat, paper.
    pub surface: Surface,
    /// Corner radius (default: from the theme).
    pub radius: Option<f32>,
    /// Line height multiplier.
    pub line_height: f32,
    /// Accent for caret, highlights and selection (default: theme accent).
    pub accent: Option<Color>,
    #[serde(skip)]
    lines: Vec<Vec<Span>>,
    /// Same lines coloured by the TEXT highlighter for [night, day] themes.
    #[serde(skip)]
    themed: Option<[Vec<Vec<Span>>; 2]>,
    #[serde(skip)]
    marks: Vec<i8>,
    #[serde(skip)]
    /// Start time of each line while typing.
    line_t: Vec<f32>,
    #[serde(skip)]
    total_t: f32,
}
impl Default for CodeBlock {
    fn default() -> Self {
        CodeBlock {
            code: String::new(),
            language: "typescript".into(),
            title: String::new(),
            font_size: 26.0,
            line_numbers: true,
            chrome: Chrome::Mac,
            typing: 0.0,
            delay: 0.0,
            diff: false,
            highlights: Vec::new(),
            scroll: Vec::new(),
            cursor: true,
            background: None,
            glow: None,
            selections: Vec::new(),
            minimap: false,
            status_bar: false,
            active_line: true,
            surface: Surface::Solid,
            radius: None,
            line_height: 1.6,
            accent: None,
            lines: Vec::new(),
            themed: None,
            marks: Vec::new(),
            line_t: Vec::new(),
            total_t: 0.0,
        }
    }
}

impl Prepare for CodeBlock {
    fn prepare(&mut self) -> Result<(), Error> {
        let code = self.code.replace('\t', "    ").replace("\r\n", "\n");
        let code = code.trim_end_matches('\n');
        let mut src = String::new();
        for (i, l) in code.split('\n').enumerate() {
            let (mark, body) = match (self.diff, l.chars().next()) {
                (true, Some('+')) => (1, &l[1..]),
                (true, Some('-')) => (-1, &l[1..]),
                (true, _) if l.starts_with(' ') => (0, &l[1..]),
                _ => (0, l),
            };
            self.marks.push(mark);
            if i > 0 {
                src.push('\n');
            }
            src.push_str(body);
        }
        self.lines = highlight(&src, &self.language);
        let rich = |theme: &str| -> Option<Vec<Vec<Span>>> {
            let h = crate::text::code::highlight(&crate::text::code::CodeSpec { code: src.clone(), language: self.language.clone(), theme: theme.into(), tab_width: 4, ..Default::default() });
            if h.syntax == "Plain Text" || h.lines.len() != self.lines.len() {
                return None;
            }
            Some(h.lines.into_iter().map(|l| l.spans.into_iter().map(|sp| Span { text: sp.text, kind: Tok::Plain, color: Some(sp.color.0) }).collect()).collect())
        };
        if let (Some(n), Some(d)) = (rich("night"), rich("day")) {
            // Only use them when the text matches line for line (typing relies on it).
            let same = |a: &Vec<Vec<Span>>| a.iter().zip(&self.lines).all(|(x, y)| x.iter().map(|s| s.text.as_str()).collect::<String>() == y.iter().map(|s| s.text.as_str()).collect::<String>());
            if same(&n) && same(&d) {
                self.themed = Some([n, d]);
            }
        }
        // Typing schedule: deterministic human rhythm (pauses after
        // punctuation and at line ends; indentation appears at once).
        let mut t = 0.0;
        if self.typing > 0.0 {
            let base = 1.0 / self.typing;
            let mut n = 0u32;
            for line in &self.lines {
                self.line_t.push(t);
                let text: String = line.iter().map(|s| s.text.as_str()).collect();
                for c in text.trim_start().chars() {
                    n += 1;
                    let j = 0.6 + 0.8 * hash01(n);
                    t += base * j * if ",;{(".contains(c) { 1.8 } else { 1.0 };
                }
                t += base * 4.0;
            }
        } else {
            self.line_t = vec![0.0; self.lines.len()];
        }
        self.total_t = t;
        if self.font_size <= 4.0 {
            return Err(Error("font_size must be > 4".into()));
        }
        Ok(())
    }
}

impl CodeBlock {
    /// Characters visible on `line` at local time `t` (usize::MAX = all).
    fn visible(&self, line: usize, t: f32) -> usize {
        if self.typing <= 0.0 {
            return usize::MAX;
        }
        let t = t - self.delay;
        let lt = self.line_t[line];
        if t < lt {
            return 0;
        }
        let text: String = self.lines[line].iter().map(|s| s.text.as_str()).collect();
        let indent = text.len() - text.trim_start().len();
        let base = 1.0 / self.typing;
        let mut acc = lt;
        let mut n: u32 = self.line_t[..line].len() as u32; // keep jitter stable per line
        n = n.wrapping_mul(131);
        for (i, c) in text.trim_start().chars().enumerate() {
            n += 1;
            acc += base * (0.6 + 0.8 * hash01(n)) * if ",;{(".contains(c) { 1.8 } else { 1.0 };
            if acc > t {
                return indent + i;
            }
        }
        usize::MAX
    }
}

impl CodeBlock {
    /// Caret (line, column) while typing at local time `t`.
    fn caret_at(&self, t: f32) -> Option<(usize, usize)> {
        if self.typing <= 0.0 || self.lines.is_empty() {
            return None;
        }
        let tt = t - self.delay;
        let i = (0..self.lines.len()).rev().find(|&i| tt >= self.line_t[i]).unwrap_or(0);
        let len: usize = self.lines[i].iter().map(|s| s.text.chars().count()).sum();
        Some((i, self.visible(i, t).min(len)))
    }
}

impl Component for CodeBlock {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let lh = fs * self.line_height.max(1.0);
        let r = self.radius.unwrap_or(th.radius * 0.8);
        let acc = self.accent.map(|c| c.0).unwrap_or(th.accent);
        let bg = self.background.map(|c| c.0).unwrap_or(if th.dark { parse_color("#0d0e13").unwrap() } else { Rgba([1.0, 1.0, 1.0, 1.0]) });
        let ink = if self.surface == Surface::Glass || self.surface == Surface::Outline { th.text } else { on_color(&[bg]) };
        let dark = luminance(ink) > 0.5;
        let mut o = Vec::new();
        if let Some(g) = self.glow {
            glow(&mut o, 0.0, 0.0, w, h, r, 60.0, alpha(g.0, 0.45));
        }
        polish::finish(&mut o, th, self.surface, 0.0, 0.0, w, h, r, 2.0, Some(bg));
        let muted = alpha(ink, 0.45);
        // Chrome.
        let top = match self.chrome {
            Chrome::Mac | Chrome::Minimal => {
                let bar = (fs * 2.1).max(38.0);
                if !matches!(self.surface, Surface::Outline | Surface::Glass) {
                    push_clip(&mut o, 0.0, 0.0, w, h, r);
                    o.push(rect(0.0, 0.0, w, bar, 0.0, solid(shade(bg, if dark { 0.022 } else { -0.03 }))));
                    o.push(Op::PopLayer);
                }
                let mut tab_x = bar * 0.4;
                if self.chrome == Chrome::Mac {
                    for (i, c) in ["#ff5f57", "#febc2e", "#28c840"].iter().enumerate() {
                        let c = parse_color(c).unwrap();
                        let (cx, cy, cr) = (bar * 0.5 + i as f32 * bar * 0.4, bar * 0.5, bar * 0.135);
                        o.push(circle(cx, cy, cr, solid(c)));
                        o.push(Op::Stroke { path: circle_path(cx, cy, cr - 0.5), paint: solid(alpha(shade(c, -0.4), 0.6)), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
                    }
                    tab_x = bar * 1.75;
                }
                if !self.title.is_empty() {
                    let ts = polish::ui(fs * 0.62, 500, alpha(ink, 0.85));
                    if self.chrome == Chrome::Mac {
                        // An active editor tab with a language dot.
                        let tw = ctx.assets.measure(&self.title, &ts);
                        let (tx, ty, tww, th_) = (tab_x, bar * 0.18, tw + fs * 2.2, bar * 0.82);
                        o.push(Op::Fill { path: tab_path(tx, ty, tww, th_, fs * 0.4), paint: solid(bg), rule: crate::dl::FillRule::NonZero, transform: IDENTITY });
                        o.push(rect(tx + fs * 0.4, ty, tww - fs * 0.8, 1.5, 0.0, solid(acc)));
                        o.push(circle(tx + fs * 0.85, ty + th_ * 0.5, fs * 0.16, solid(lang_color(&self.language))));
                        draw(&mut o, ctx.assets, &self.title, &ts, tx + fs * 1.3, ty + th_ * 0.5 + ts.size * 0.36, 0.0);
                    } else {
                        draw(&mut o, ctx.assets, &self.title, &ts.clone().color(muted), w * 0.5, bar * 0.5 + ts.size * 0.36, 0.5);
                    }
                }
                o.push(rect(0.0, bar - 1.0, w, 1.0, 0.0, solid(alpha(ink, 0.07))));
                bar
            }
            Chrome::None => 0.0,
        };
        let status_h = if self.status_bar { (fs * 1.25).max(24.0) } else { 0.0 };
        let mini_w = if self.minimap { (w * 0.085).clamp(60.0, 130.0) } else { 0.0 };
        let pad = fs * 1.0;
        let st = TextStyle::new(Face::Mono, fs, 400, ink).tracking(-0.01);
        let cw = ctx.assets.measure("M", &st);
        let gutter = if self.line_numbers { cw * (self.lines.len().to_string().len() as f32 + 2.2) } else { 0.0 };
        let x0 = pad + gutter + if self.diff { cw * 2.0 } else { 0.0 };
        // Scroll offset in lines.
        let mut scroll = 0.0;
        for s in &self.scroll {
            let k = ease_in_out(seg(t, s.at, s.duration));
            scroll = lerp(scroll, s.line.saturating_sub(1) as f32, k);
        }
        let body_h = h - top - status_h;
        let rows = ((body_h - pad) / lh).floor().max(1.0);
        // Auto-scroll (smoothly) to keep the typing line in view.
        if self.typing > 0.0 {
            let follow = |tt: f32| -> f32 {
                let tl = self.caret_at(tt).map(|c| c.0).unwrap_or(0) as f32;
                (tl - rows + 2.0).max(0.0)
            };
            let auto = (0..6).map(|k| follow(t - k as f32 * 0.04)).sum::<f32>() / 6.0;
            scroll = scroll.max(auto);
        }
        let body_y = top + pad * 0.6;
        let caret = self.caret_at(t);
        let active = if self.typing > 0.0 { caret.map(|c| c.0) } else { None };
        o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((rrect_path(0.0, top, w - mini_w, body_h, 0.0), IDENTITY)), cache_key: None });
        // Highlights.
        let mut dim = 0.0f32;
        let mut lit = vec![0.0f32; self.lines.len()];
        for hl in &self.highlights {
            let on = ease_out(seg(t, hl.at, 0.35)) * hl.until.map(|u| 1.0 - ease_out(seg(t, u, 0.35))).unwrap_or(1.0);
            if on <= 0.0 {
                continue;
            }
            if hl.dim {
                dim = dim.max(on);
            }
            let mut ls = hl.lines.clone();
            if let Some([a, b]) = hl.range {
                ls.extend(a..=b);
            }
            for l in ls {
                if (1..=self.lines.len()).contains(&l) {
                    lit[l - 1] = lit[l - 1].max(on);
                }
            }
        }
        let lines = match &self.themed {
            Some([n, d]) => {
                if dark {
                    n
                } else {
                    d
                }
            }
            None => &self.lines,
        };
        let mut caret_y = None;
        for (i, line) in lines.iter().enumerate() {
            let y = body_y + (i as f32 - scroll) * lh;
            if y + lh < top || y > h {
                continue;
            }
            let vis = self.visible(i, t);
            if vis == 0 && self.typing > 0.0 && (t - self.delay) < self.line_t[i] {
                continue;
            }
            let mark = self.marks[i];
            if self.active_line && active == Some(i) && lit[i] <= 0.0 && mark == 0 {
                o.push(rect(0.0, y, w, lh, 0.0, solid(alpha(ink, 0.045))));
            }
            if mark != 0 {
                let c = if mark > 0 { th.positive } else { th.negative };
                o.push(rect(0.0, y, w, lh, 0.0, Paint::Linear { p0: [0.0, 0.0], p1: [w, 0.0], stops: vec![(0.0, alpha(c, 0.2)), (1.0, alpha(c, 0.07))] }));
                o.push(rect(0.0, y, 3.0, lh, 0.0, solid(c)));
                draw(&mut o, ctx.assets, if mark > 0 { "+" } else { "-" }, &st.clone().color(c), pad + gutter, y + lh * 0.68, 0.0);
            }
            if lit[i] > 0.0 {
                o.push(rect(0.0, y, w, lh, 0.0, Paint::Linear { p0: [0.0, 0.0], p1: [w, 0.0], stops: vec![(0.0, alpha(acc, 0.2 * lit[i])), (1.0, alpha(acc, 0.06 * lit[i]))] }));
                o.push(rect(0.0, y, 3.0, lh, 0.0, solid(alpha(acc, lit[i]))));
            }
            // Selections on this line.
            for sel in self.selections.iter().filter(|s| s.line == i + 1) {
                let k = polish::ease_out_expo(seg(t, sel.at, 0.3)) * sel.until.map(|u| 1.0 - ease_out(seg(t, u, 0.25))).unwrap_or(1.0);
                if k > 0.0 {
                    let len: usize = line.iter().map(|s| s.text.chars().count()).sum();
                    let to = sel.to.unwrap_or(len).min(len.max(sel.from));
                    let sw = (to.saturating_sub(sel.from)) as f32 * cw * k;
                    o.push(rect(x0 + sel.from as f32 * cw - 1.0, y + lh * 0.08, sw + 2.0, lh * 0.84, 3.0, solid(alpha(acc, 0.32 * k.min(1.0)))));
                }
            }
            let fade = 1.0 - dim * (1.0 - lit[i]) * 0.65;
            if self.line_numbers {
                let na = if active == Some(i) || lit[i] > 0.5 { 0.85 } else { 0.32 };
                draw(&mut o, ctx.assets, &(i + 1).to_string(), &st.clone().color(alpha(ink, na * fade)), pad + gutter - cw * 1.6, y + lh * 0.68, 1.0);
            }
            let mut x = x0;
            let mut shown = 0usize;
            for s in line {
                if shown >= vis {
                    break;
                }
                let n = s.text.chars().count();
                let take = n.min(vis - shown);
                let text: String = if take < n { s.text.chars().take(take).collect() } else { s.text.clone() };
                let c = s.color.unwrap_or_else(|| syntax_color(s.kind, dark));
                let c = if mark < 0 { alpha(c, 0.7) } else { c };
                if !text.trim().is_empty() {
                    let (ops, _) = ctx.assets.text(&text, &st.clone().color(alpha(c, fade)), x, y + lh * 0.68);
                    o.extend(ops);
                }
                x += cw * take as f32;
                shown += take;
            }
            if active == Some(i) {
                caret_y = Some(y);
            }
        }
        if self.cursor && self.typing > 0.0 {
            if let (Some((ci, _)), Some(cy)) = (caret, caret_y) {
                // Smooth caret: average recent positions on the same line so it glides.
                let xs: Vec<f32> = (0..4).filter_map(|k| self.caret_at(t - k as f32 * 0.016).filter(|c| c.0 == ci).map(|c| c.1 as f32)).collect();
                let col = xs.iter().sum::<f32>() / xs.len().max(1) as f32;
                let cx = x0 + col * cw;
                let typing_now = t - self.delay < self.total_t;
                let blink = if typing_now { 1.0 } else { polish::smoothstep(0.0, 1.0, 0.5 + 0.8 * (t * std::f32::consts::TAU / 1.1).cos()) };
                o.push(rect(cx, cy + lh * 0.16, (fs * 0.085).max(2.0), lh * 0.68, 1.0, solid(alpha(acc, blink))));
            }
        }
        o.push(Op::PopLayer);
        // Minimap.
        if self.minimap {
            let mx = w - mini_w + 8.0;
            let rowh = (fs * 0.13).max(2.0);
            let gap = rowh * 1.6;
            let cpx = (mini_w - 16.0) / 80.0;
            o.push(rect(w - mini_w, top, 1.0, body_h, 0.0, solid(alpha(ink, 0.06))));
            for (i, line) in lines.iter().enumerate() {
                let vis = self.visible(i, t);
                if vis == 0 && self.typing > 0.0 && (t - self.delay) < self.line_t[i] {
                    continue;
                }
                let y = top + 10.0 + i as f32 * gap;
                if y > h - status_h - 6.0 {
                    break;
                }
                let mut col = 0usize;
                for s in line {
                    let n = s.text.chars().count().min(vis.saturating_sub(col));
                    let lead = s.text.chars().take_while(|c| c.is_whitespace()).count().min(n);
                    if n > lead {
                        let c = s.color.unwrap_or_else(|| syntax_color(s.kind, dark));
                        o.push(rect(mx + (col + lead) as f32 * cpx, y, (n - lead) as f32 * cpx, rowh, 0.0, solid(alpha(c, 0.55))));
                    }
                    col += s.text.chars().count();
                }
            }
            let vy = top + 10.0 + scroll * gap - 4.0;
            o.push(rect(w - mini_w + 2.0, vy, mini_w - 4.0, rows * gap + 8.0, 3.0, solid(alpha(ink, 0.06))));
        }
        // Status bar.
        if self.status_bar {
            let sy = h - status_h;
            push_clip(&mut o, 0.0, 0.0, w, h, r);
            o.push(rect(0.0, sy, w, status_h, 0.0, solid(shade(bg, if dark { 0.022 } else { -0.03 }))));
            o.push(Op::PopLayer);
            o.push(rect(0.0, sy, w, 1.0, 0.0, solid(alpha(ink, 0.07))));
            let ss = polish::ui(status_h * 0.5, 500, muted);
            let base = sy + status_h * 0.5 + ss.size * 0.36;
            o.push(circle(pad * 0.6 + ss.size * 0.3, sy + status_h * 0.5, ss.size * 0.28, solid(th.positive)));
            draw(&mut o, ctx.assets, &self.language, &ss, pad * 0.6 + ss.size * 0.9, base, 0.0);
            let (ln, colnum) = caret.map(|(l, c)| (l + 1, c + 1)).unwrap_or((1, 1));
            draw(&mut o, ctx.assets, &format!("Ln {ln}, Col {colnum}    UTF-8    Spaces: 4"), &ss.clone().tabular(), w - pad * 0.6, base, 1.0);
        }
        if !matches!(self.surface, Surface::Outline) {
            border(&mut o, 0.0, 0.0, w, h, r, alpha(ink, 0.1));
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        let s = self.scroll.iter().fold(0.0f32, |m, s| m.max(s.at + s.duration));
        let hl = self.highlights.iter().fold(0.0f32, |m, h| m.max(h.until.unwrap_or(h.at) + 0.35));
        let sel = self.selections.iter().fold(0.0f32, |m, s| m.max(s.until.unwrap_or(s.at) + 0.35));
        Some((self.delay + self.total_t).max(s).max(hl).max(sel))
    }
}

/// Editor tab: rounded top corners, square bottom.
fn tab_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> crate::dl::Path {
    use kurbo::Shape;
    kurbo::RoundedRect::from_rect(kurbo::Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64), kurbo::RoundedRectRadii::new(r as f64, r as f64, 0.0, 0.0)).to_path(0.1)
}

/// File-type dot colour for the tab.
fn lang_color(lang: &str) -> Rgba {
    let c = match lang.to_ascii_lowercase().as_str() {
        "ts" | "typescript" | "tsx" => "#3178c6",
        "js" | "javascript" | "jsx" => "#f1e05a",
        "rust" | "rs" => "#dea584",
        "python" | "py" => "#3572a5",
        "go" => "#00add8",
        "swift" => "#f05138",
        "json" => "#cbcb41",
        "bash" | "sh" | "shell" => "#89e051",
        "css" => "#663399",
        "html" => "#e34c26",
        _ => "#8b8f9c",
    };
    parse_color(c).unwrap()
}

// ------------------------------------------------------------------ terminal

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Terminal {
    /// Lines starting with `$ ` (or `> `) are typed as commands; others print.
    pub lines: Vec<String>,
    pub title: String,
    pub font_size: f32,
    /// Typing speed for commands (chars/s).
    pub typing: f32,
    /// Pause after each command before its output (s).
    pub pause: f32,
    /// Delay between output lines (s).
    pub line_delay: f32,
    pub delay: f32,
    pub prompt_color: Option<Color>,
    #[serde(skip)]
    sched: Vec<(f32, f32)>,
}
impl Default for Terminal {
    fn default() -> Self {
        Terminal { lines: Vec::new(), title: String::new(), font_size: 26.0, typing: 28.0, pause: 0.35, line_delay: 0.06, delay: 0.0, prompt_color: None, sched: Vec::new() }
    }
}
impl Terminal {
    fn is_cmd(l: &str) -> bool {
        l.starts_with("$ ") || l.starts_with("> ")
    }
}
impl Prepare for Terminal {
    fn prepare(&mut self) -> Result<(), Error> {
        // (start, typing end) per line.
        let mut t = self.delay;
        for l in &self.lines {
            if Terminal::is_cmd(l) {
                t += 0.25;
                let d = (l.chars().count() - 2) as f32 / self.typing.max(1.0);
                self.sched.push((t, t + d));
                t += d + self.pause;
            } else {
                self.sched.push((t, t));
                t += self.line_delay;
            }
        }
        Ok(())
    }
}
impl Component for Terminal {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let lh = fs * 1.55;
        let r = th.radius * 0.8;
        let mut o = Vec::new();
        polish::elevation(&mut o, th, 0.0, 0.0, w, h, r, 2.0);
        let bg = parse_color("#0b0c10").unwrap();
        o.push(rect(0.0, 0.0, w, h, r, vgrad(0.0, h, shade(bg, 0.02), bg)));
        polish::bevel(&mut o, 0.0, 0.0, w, h, r, 0.8);
        let bar = (fs * 2.0).max(36.0);
        for (i, c) in ["#ff5f57", "#febc2e", "#28c840"].iter().enumerate() {
            o.push(circle(bar * 0.5 + i as f32 * bar * 0.42, bar * 0.5, bar * 0.15, solid(parse_color(c).unwrap())));
        }
        if !self.title.is_empty() {
            draw(&mut o, ctx.assets, &self.title, &TextStyle::new(Face::Sans, fs * 0.7, 500, Rgba([0.55, 0.57, 0.63, 1.0])), w * 0.5, bar * 0.5 + fs * 0.24, 0.5);
        }
        let st = TextStyle::new(Face::Mono, fs, 400, parse_color("#e6e7ee").unwrap());
        let cw = ctx.assets.measure("M", &st);
        let pc = self.prompt_color.map(|c| c.0).unwrap_or(th.positive);
        let pad = fs;
        let shown: Vec<usize> = (0..self.lines.len()).filter(|&i| t >= self.sched[i].0).collect();
        let rows = ((h - bar - pad) / lh).floor().max(1.0) as usize;
        let first = shown.len().saturating_sub(rows);
        let mut caret = (pad, bar + pad * 0.6);
        for (row, &i) in shown[first..].iter().enumerate() {
            let y = bar + pad * 0.6 + row as f32 * lh;
            let l = &self.lines[i];
            let base = y + lh * 0.7;
            if Terminal::is_cmd(l) {
                let (s, e) = self.sched[i];
                let body = &l[2..];
                let n = body.chars().count();
                let k = if e > s { (((t - s) / (e - s)) * n as f32).floor() as usize } else { n };
                let typed: String = body.chars().take(k.min(n)).collect();
                draw(&mut o, ctx.assets, &l[..1], &st.clone().color(pc), pad, base, 0.0);
                let spans = highlight(&typed, "bash");
                let mut x = pad + cw * 2.0;
                for sp in spans.first().into_iter().flatten() {
                    let c = if sp.kind == Tok::Plain || sp.kind == Tok::Punct { st.color } else { syntax_color(sp.kind, true) };
                    draw(&mut o, ctx.assets, &sp.text, &st.clone().color(c), x, base, 0.0);
                    x += cw * sp.text.chars().count() as f32;
                }
                caret = (x, y);
            } else {
                let c = if l.starts_with('✓') || l.contains("success") || l.contains("done") {
                    th.positive
                } else if l.starts_with("error") || l.starts_with('✗') {
                    th.negative
                } else {
                    parse_color("#a8acb8").unwrap()
                };
                draw(&mut o, ctx.assets, l, &st.clone().color(c), pad, base, 0.0);
                caret = (pad, y + lh);
            }
        }
        let blink = (0.5 + 0.5 * (t * std::f32::consts::TAU / 1.05).cos()).round();
        o.push(rect(caret.0 + 1.0, caret.1 + lh * 0.16, cw * 0.55, lh * 0.68, 1.5, solid(alpha(Rgba([0.9, 0.9, 0.95, 1.0]), 0.85 * blink))));
        border(&mut o, 0.0, 0.0, w, h, r, Rgba([1.0, 1.0, 1.0, 0.1]));
        o
    }
    fn duration(&self) -> Option<f32> {
        self.sched.last().map(|s| s.1 + 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use super::*;
    use serde_json::json;

    #[test]
    fn lexer() {
        let l = highlight("fn main() { let x = \"hi\"; // c\n}", "rust");
        let kinds: Vec<Tok> = l[0].iter().map(|s| s.kind).collect();
        assert_eq!(l[0][0], Span { text: "fn".into(), kind: Tok::Keyword, color: None });
        assert!(kinds.contains(&Tok::Function) && kinds.contains(&Tok::String) && kinds.contains(&Tok::Comment));
        let l = highlight("/* a\nb */ x", "ts");
        assert_eq!(l[1][0].kind, Tok::Comment);
        assert_eq!(l[1].last().unwrap().kind, Tok::Plain);
        let l = highlight("{\"a\": 1}", "json");
        assert_eq!(l[0][1].kind, Tok::Function);
    }

    #[test]
    fn gallery() {
        let th = Theme::default();
        let a = preview::host();
        let code = "import { video } from \"anikit\";\n\n// One spec in, a film out.\nconst film = video({ width: 1920, height: 1080 });\nfilm.scene(\"intro\", { duration: 3 });\n-film.render(\"old.mp4\");\n+await film.render(\"launch.mp4\");";
        let c = build("code", &json!({"code": code, "title": "launch.ts", "typing": 40, "diff": true, "highlights": [{"lines": [4], "at": 3.5}], "glow": "#7c6cff"})).unwrap();
        let end = c.component.duration().unwrap();
        for (i, t) in [0.0, end * 0.4, end + 0.5].iter().enumerate() {
            let ops = c.eval(*t, 960.0, 540.0, 30.0, &th, a);
            preview::validate(&ops).unwrap();
            if i > 0 {
                let mut all = vec![super::super::style::rect(0.0, 0.0, 960.0, 540.0, 0.0, super::super::style::solid(th.background))];
                let mut ops = ops;
                super::super::transform_ops(&mut ops, &[0.85, 0.0, 0.0, 0.85, 72.0, 40.0]);
                all.extend(ops);
                preview::snapshot(&format!("code-{i}"), 960, 540, th.background, all, a);
            }
        }
        let c = build("terminal", &json!({"lines": ["$ npm install anikit", "added 1 package in 0.4s", "$ npx anikit-video render launch.json", "✓ 1800 frames in 6.1s (295 fps)"], "title": "zsh"})).unwrap();
        let end = c.component.duration().unwrap();
        let ops = c.eval(end, 720.0, 300.0, 30.0, &th, a);
        preview::validate(&ops).unwrap();
        preview::snapshot("code-terminal", 720, 300, th.background, ops, a);
    }
}
