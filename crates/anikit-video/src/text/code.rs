//! Code text: syntax highlighting into styled spans, with line numbers and a
//! line-range highlight, ready for the code-block component.
//!
//! Grammars are Sublime syntaxes (syntect, plus bat's extra set via two-face:
//! TypeScript/TSX, TOML, Dockerfile, …), parsed with a pure-Rust regex engine.
//! Highlighting runs once at preprocessing.

use std::str::FromStr;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SColor, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use super::color::Color;
use super::layout::{Span, TextSpec, TextStyle};

/// Colours of a code theme (everything a code block needs to draw itself).
#[derive(Clone, Debug, PartialEq)]
pub struct CodeTheme {
    pub name: &'static str,
    pub background: Color,
    pub foreground: Color,
    /// Line-number colour, and for highlighted lines.
    pub gutter: Color,
    pub gutter_active: Color,
    /// Background of highlighted lines.
    pub line_highlight: Color,
    /// Dim factor applied to non-highlighted lines when a range is highlighted (1 = none).
    pub dim: f32,
    pub(crate) rules: &'static [(&'static str, &'static str, u8)],
}

const B: u8 = 1;
const I: u8 = 2;

/// "Night": a deep, saturated dark theme.
static NIGHT_RULES: &[(&str, &str, u8)] = &[
    ("comment, punctuation.definition.comment", "#636d83", I),
    ("string, string.quoted, punctuation.definition.string", "#a5e075", 0),
    ("string.regexp, constant.character.escape", "#89ddff", 0),
    ("constant.numeric, constant.language, constant.other, support.constant", "#f78c6c", 0),
    ("keyword, storage, storage.type, storage.modifier, keyword.control", "#c792ea", 0),
    ("keyword.operator, punctuation.accessor, punctuation.separator.key-value", "#89ddff", 0),
    ("entity.name.function, support.function, meta.function-call entity.name.function, variable.function", "#82aaff", 0),
    ("entity.name.type, entity.name.class, support.type, support.class, entity.other.inherited-class, storage.type.primitive", "#ffcb6b", 0),
    ("variable.parameter", "#f07178", I),
    ("variable.language, variable.language.this", "#ff5370", I),
    ("entity.name.tag, meta.tag.sgml", "#f07178", 0),
    ("entity.other.attribute-name", "#ffcb6b", I),
    ("meta.object-literal.key, support.type.property-name, variable.other.property, variable.other.object.property", "#b2ccd6", 0),
    ("markup.heading, entity.name.section", "#82aaff", B),
    ("markup.bold", "#ffcb6b", B),
    ("markup.italic", "#c792ea", I),
    ("markup.inserted", "#a5e075", 0),
    ("markup.deleted", "#ff5370", 0),
    ("punctuation, meta.brace", "#a6accd", 0),
];

/// "Day": a crisp light theme.
static DAY_RULES: &[(&str, &str, u8)] = &[
    ("comment, punctuation.definition.comment", "#6e7781", I),
    ("string, string.quoted, punctuation.definition.string", "#0a3069", 0),
    ("string.regexp, constant.character.escape", "#116329", 0),
    ("constant.numeric, constant.language, constant.other, support.constant", "#0550ae", 0),
    ("keyword, storage, storage.type, storage.modifier, keyword.control", "#cf222e", 0),
    ("keyword.operator, punctuation.accessor", "#cf222e", 0),
    ("entity.name.function, support.function, variable.function", "#8250df", 0),
    ("entity.name.type, entity.name.class, support.type, support.class, entity.other.inherited-class", "#953800", 0),
    ("variable.parameter", "#953800", 0),
    ("variable.language", "#cf222e", 0),
    ("entity.name.tag", "#116329", 0),
    ("entity.other.attribute-name", "#0550ae", 0),
    ("meta.object-literal.key, support.type.property-name, variable.other.property", "#0550ae", 0),
    ("markup.heading, entity.name.section", "#0550ae", B),
    ("markup.bold", "#24292f", B),
    ("markup.italic", "#24292f", I),
    ("markup.inserted", "#116329", 0),
    ("markup.deleted", "#82071e", 0),
    ("punctuation, meta.brace", "#57606a", 0),
];

/// Built-in themes: `"night"`/`"dark"` and `"day"`/`"light"`.
pub fn theme(name: &str) -> CodeTheme {
    let h = |s| Color::hex(s).unwrap();
    match name.to_lowercase().as_str() {
        "day" | "light" => CodeTheme {
            name: "day",
            background: h("#ffffff"),
            foreground: h("#24292f"),
            gutter: h("#8c959f"),
            gutter_active: h("#24292f"),
            line_highlight: Color::rgba(0.33, 0.68, 1.0, 0.14),
            dim: 0.55,
            rules: DAY_RULES,
        },
        _ => CodeTheme {
            name: "night",
            background: h("#0f111a"),
            foreground: h("#d0d6f0"),
            gutter: h("#3b4261"),
            gutter_active: h("#a6accd"),
            line_highlight: Color::rgba(0.51, 0.67, 1.0, 0.12),
            dim: 0.45,
            rules: NIGHT_RULES,
        },
    }
}

impl CodeTheme {
    fn syntect(&self) -> Theme {
        let c = |c: Color| {
            let [r, g, b, a] = c.0 .0;
            SColor { r: (r * 255.0) as u8, g: (g * 255.0) as u8, b: (b * 255.0) as u8, a: (a * 255.0) as u8 }
        };
        Theme {
            name: Some(self.name.into()),
            author: None,
            settings: ThemeSettings { foreground: Some(c(self.foreground)), background: Some(c(self.background)), ..Default::default() },
            scopes: self
                .rules
                .iter()
                .filter_map(|(sel, col, fs)| {
                    let mut f = FontStyle::empty();
                    if fs & B != 0 {
                        f |= FontStyle::BOLD;
                    }
                    if fs & I != 0 {
                        f |= FontStyle::ITALIC;
                    }
                    Some(ThemeItem {
                        scope: ScopeSelectors::from_str(sel).ok()?,
                        style: StyleModifier { foreground: Some(c(Color::hex(col)?)), background: None, font_style: Some(f) },
                    })
                })
                .collect(),
        }
    }
}

/// What to highlight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodeSpec {
    pub code: String,
    /// Language name, extension or token: `ts`, `tsx`, `rust`, `py`, `json`, `bash`, …
    pub language: String,
    /// `night` (default) or `day`.
    pub theme: String,
    /// First line's number.
    pub start_line: u32,
    /// Inclusive 1-based line ranges to highlight, e.g. `[[3, 5], [9, 9]]`.
    pub highlight: Vec<[u32; 2]>,
    pub tab_width: usize,
}

impl Default for CodeSpec {
    fn default() -> Self {
        CodeSpec { code: String::new(), language: "ts".into(), theme: "night".into(), start_line: 1, highlight: Vec::new(), tab_width: 2 }
    }
}

/// One styled piece of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeSpan {
    pub text: String,
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
}

/// One line of highlighted code.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeLine {
    pub number: u32,
    pub spans: Vec<CodeSpan>,
    pub highlighted: bool,
}

/// Highlighted code plus its theme.
#[derive(Clone, Debug, PartialEq)]
pub struct Highlighted {
    pub lines: Vec<CodeLine>,
    pub theme: CodeTheme,
    /// The syntax actually used (falls back to plain text).
    pub syntax: String,
}

fn syntaxes() -> &'static SyntaxSet {
    static S: OnceLock<SyntaxSet> = OnceLock::new();
    S.get_or_init(two_face::syntax::extra_newlines)
}

fn find_syntax<'a>(ss: &'a SyntaxSet, lang: &str) -> &'a SyntaxReference {
    let l = lang.trim().to_lowercase();
    let alias = match l.as_str() {
        "typescript" | "ts" => "ts",
        "javascript" | "js" | "jsx" | "mjs" | "cjs" => "js",
        "tsx" => "tsx",
        "rust" | "rs" => "rs",
        "python" | "py" => "py",
        "shell" | "sh" | "bash" | "zsh" | "console" => "sh",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "markdown" | "md" => "md",
        "html" => "html",
        "css" => "css",
        "go" | "golang" => "go",
        "c++" | "cpp" => "cpp",
        "csharp" | "c#" | "cs" => "cs",
        "kotlin" | "kt" => "kt",
        "swift" => "swift",
        "ruby" | "rb" => "rb",
        "toml" => "toml",
        "sql" => "sql",
        "diff" | "patch" => "diff",
        other => other,
    };
    ss.find_syntax_by_token(alias)
        .or_else(|| ss.find_syntax_by_name(lang))
        .unwrap_or_else(|| ss.find_syntax_plain_text())
}

/// Highlight code into styled lines.
pub fn highlight(spec: &CodeSpec) -> Highlighted {
    let th = theme(&spec.theme);
    let st = th.syntect();
    let ss = syntaxes();
    let syn = find_syntax(ss, &spec.language);
    let mut h = HighlightLines::new(syn, &st);
    let tabs = " ".repeat(spec.tab_width.max(1));
    let code = spec.code.replace('\t', &tabs);
    let code = code.trim_end_matches('\n');
    let mut lines = Vec::new();
    for (i, line) in syntect::util::LinesWithEndings::from(code).enumerate() {
        let number = spec.start_line + i as u32;
        let highlighted = spec.highlight.iter().any(|[a, b]| number >= *a && number <= *b);
        let mut spans: Vec<CodeSpan> = Vec::new();
        let ranges = h.highlight_line(line, ss).unwrap_or_default();
        for (style, text) in ranges {
            let text = text.trim_end_matches(['\n', '\r']);
            if text.is_empty() {
                continue;
            }
            let f = style.foreground;
            let color = Color::rgba(f.r as f32 / 255.0, f.g as f32 / 255.0, f.b as f32 / 255.0, f.a as f32 / 255.0);
            let bold = style.font_style.contains(FontStyle::BOLD);
            let italic = style.font_style.contains(FontStyle::ITALIC);
            match spans.last_mut() {
                // Merge neighbours with the same look (whitespace takes any colour).
                Some(p) if p.bold == bold && p.italic == italic && (p.color == color || text.trim().is_empty()) => p.text.push_str(text),
                _ => spans.push(CodeSpan { text: text.to_string(), color, bold, italic }),
            }
        }
        lines.push(CodeLine { number, spans, highlighted });
    }
    Highlighted { lines, syntax: syn.name.clone(), theme: th }
}

impl Highlighted {
    /// The code as one rich [`TextSpec`] (JetBrains Mono, no wrapping), with
    /// non-highlighted lines dimmed when any range is highlighted.
    pub fn text_spec(&self, size: f32, line_height: f32) -> TextSpec {
        let any = self.lines.iter().any(|l| l.highlighted);
        let mut spans = Vec::new();
        for (i, l) in self.lines.iter().enumerate() {
            let a = if any && !l.highlighted { self.theme.dim } else { 1.0 };
            for s in &l.spans {
                spans.push(Span {
                    text: s.text.clone(),
                    color: Some(s.color.with_alpha(s.color.0 .0[3] * a)),
                    weight: s.bold.then_some(700.0),
                    italic: s.italic.then_some(true),
                    ..Default::default()
                });
            }
            if i + 1 < self.lines.len() {
                spans.push(Span::new("\n"));
            }
        }
        TextSpec {
            spans,
            style: TextStyle { font: "JetBrains Mono".into(), size, line_height, color: self.theme.foreground, ..Default::default() },
            ..Default::default()
        }
    }

    /// Line numbers as a right-aligned [`TextSpec`] (one per line).
    pub fn gutter_spec(&self, size: f32, line_height: f32) -> TextSpec {
        let any = self.lines.iter().any(|l| l.highlighted);
        let spans = self
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| Span {
                text: format!("{}{}", l.number, if i + 1 < self.lines.len() { "\n" } else { "" }),
                color: Some(if any && l.highlighted { self.theme.gutter_active } else { self.theme.gutter }),
                ..Default::default()
            })
            .collect();
        TextSpec {
            spans,
            style: TextStyle { font: "JetBrains Mono".into(), size, line_height, color: self.theme.gutter, ..Default::default() },
            align: super::layout::Align::Right,
            ..Default::default()
        }
    }
}
