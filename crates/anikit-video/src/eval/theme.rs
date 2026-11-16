//! Resolved theme: colour tokens, font roles and text styles with defaults.

use crate::dl::Rgba;
use crate::spec::{self, Num, Paint, TextStyle, Weight};
use std::collections::BTreeMap;

/// The resolved theme.
#[derive(Clone, Debug)]
pub struct Theme {
    pub colors: BTreeMap<String, Rgba>,
    pub fonts: BTreeMap<String, String>,
    pub radius: f32,
    pub styles: BTreeMap<String, TextStyle>,
}

const DEFAULT_COLORS: &[(&str, &str)] = &[
    ("primary", "#7c5cff"),
    ("secondary", "#22d3ee"),
    ("accent", "#ff5ca8"),
    ("bg", "#0b0b12"),
    ("surface", "#171725"),
    ("text", "#f5f5fa"),
    ("muted", "#9a9ab0"),
    ("white", "#ffffff"),
    ("black", "#000000"),
];

fn style(font: &str, size: &str, weight: f64, color: &str) -> TextStyle {
    TextStyle {
        font: Some(font.into()),
        size: Some(Num::Str(size.into())),
        weight: Some(Weight::Num(weight)),
        color: Some(Paint::Css(color.into())),
        ..Default::default()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::resolve(None).0
    }
}

impl Theme {
    /// Resolve a spec theme over the defaults. Returns the theme and bad colour tokens.
    pub fn resolve(t: Option<&spec::Theme>) -> (Theme, Vec<(String, String)>) {
        let mut bad = Vec::new();
        let mut colors = BTreeMap::new();
        for (k, v) in DEFAULT_COLORS {
            colors.insert(k.to_string(), parse_css_color(v).unwrap());
        }
        let mut fonts: BTreeMap<String, String> = [("heading", "Inter Display"), ("body", "Inter"), ("mono", "JetBrains Mono")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let mut styles = BTreeMap::new();
        styles.insert("title".into(), style("$heading", "9vh", 800.0, "$text"));
        styles.insert("heading".into(), style("$heading", "6vh", 700.0, "$text"));
        styles.insert("subtitle".into(), style("$body", "3.6vh", 500.0, "$muted"));
        styles.insert("body".into(), style("$body", "3vh", 400.0, "$text"));
        styles.insert("caption".into(), style("$body", "2.2vh", 500.0, "$muted"));
        let mut label = style("$body", "1.8vh", 600.0, "$muted");
        label.uppercase = Some(true);
        label.letter_spacing = Some(Num::Str("0.08em".into()));
        styles.insert("label".into(), label);
        styles.insert("code".into(), style("$mono", "2.6vh", 400.0, "$text"));
        let mut radius = 20.0;
        if let Some(t) = t {
            if let Some(c) = &t.colors {
                // Two passes so tokens may reference each other.
                for _ in 0..2 {
                    for (k, v) in c {
                        let r = if let Some(name) = v.strip_prefix('$') { colors.get(name).copied() } else { parse_css_color(v) };
                        if let Some(r) = r {
                            colors.insert(k.clone(), r);
                        }
                    }
                }
                for (k, v) in c {
                    let ok = if let Some(name) = v.strip_prefix('$') { colors.contains_key(name) } else { parse_css_color(v).is_some() };
                    if !ok {
                        bad.push((k.clone(), v.clone()));
                    }
                }
            }
            if let Some(f) = &t.fonts {
                for (k, v) in f {
                    fonts.insert(k.clone(), v.clone());
                }
            }
            if let Some(r) = t.radius {
                radius = r as f32;
            }
            if let Some(s) = &t.styles {
                for (k, v) in s {
                    let base = styles.get(k).cloned().unwrap_or_else(|| styles["body"].clone());
                    styles.insert(k.clone(), merge_style(&base, v));
                }
            }
        }
        (Theme { colors, fonts, radius, styles }, bad)
    }

    /// Resolve a colour string (CSS or `$token`).
    pub fn color(&self, s: &str) -> Option<Rgba> {
        let s = s.trim();
        if let Some(name) = s.strip_prefix('$') {
            return self.colors.get(name).copied();
        }
        parse_css_color(s)
    }

    /// Resolve a font family (`$heading` → the family name).
    pub fn font<'a>(&'a self, s: &'a str) -> &'a str {
        match s.strip_prefix('$') {
            Some(r) => self.fonts.get(r).map(|s| s.as_str()).unwrap_or("Inter"),
            None => s,
        }
    }
}

/// Overlay `over` onto `base`.
pub fn merge_style(base: &TextStyle, over: &TextStyle) -> TextStyle {
    TextStyle {
        font: over.font.clone().or_else(|| base.font.clone()),
        size: over.size.clone().or_else(|| base.size.clone()),
        weight: over.weight.clone().or_else(|| base.weight.clone()),
        italic: over.italic.or(base.italic),
        color: over.color.clone().or_else(|| base.color.clone()),
        line_height: over.line_height.or(base.line_height),
        letter_spacing: over.letter_spacing.clone().or_else(|| base.letter_spacing.clone()),
        uppercase: over.uppercase.or(base.uppercase),
        align: over.align.clone().or_else(|| base.align.clone()),
    }
}

/// Parse a CSS colour into straight-alpha sRGB 0..1.
pub fn parse_css_color(s: &str) -> Option<Rgba> {
    let c = anikit_core::color::parse(s.trim())?;
    Some(Rgba([(c.r / 255.0) as f32, (c.g / 255.0) as f32, (c.b / 255.0) as f32, c.a as f32]))
}

/// Font weight to a number.
pub fn weight_value(w: &Weight) -> Option<f32> {
    match w {
        Weight::Num(n) => Some(*n as f32),
        Weight::Name(s) => Some(match s.to_ascii_lowercase().replace(['-', ' ', '_'], "").as_str() {
            "thin" | "hairline" => 100.0,
            "extralight" | "ultralight" => 200.0,
            "light" => 300.0,
            "normal" | "regular" | "book" => 400.0,
            "medium" => 500.0,
            "semibold" | "demibold" => 600.0,
            "bold" => 700.0,
            "extrabold" | "ultrabold" => 800.0,
            "black" | "heavy" => 900.0,
            _ => return s.parse().ok(),
        }),
    }
}
