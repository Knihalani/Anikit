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
    /// Component material defaults (`theme.material`).
    pub material: spec::Material,
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
            if let Some(p) = &t.palette {
                match palette(p) {
                    Ok(toks) => {
                        for (k, v) in toks {
                            colors.insert(k, v);
                        }
                    }
                    Err(e) => bad.push(("palette".into(), e)),
                }
            }
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
        (Theme { colors, fonts, radius, styles, material: t.and_then(|t| t.material.clone()).unwrap_or_default() }, bad)
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
    if let Some(n) = s.trim().strip_prefix("riso:") {
        return riso(n);
    }
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

/// Risograph inks (Matt DesLauriers' `riso-colors`, from stencil.wiki): use as `riso:fluorescent-pink`.
pub const RISO_INKS: &[(&str, &str)] = &[
    ("fluorescent-pink", "#FF48B0"),
    ("medium-blue", "#3255A4"),
    ("blue", "#0078BF"),
    ("federal-blue", "#3D5588"),
    ("yellow", "#FFE800"),
    ("teal", "#00838A"),
    ("aqua", "#5EC8E5"),
    ("green", "#00A95C"),
    ("hunter-green", "#407060"),
    ("bright-red", "#F15060"),
    ("scarlet", "#F65058"),
    ("orange", "#FF6C2F"),
    ("fluorescent-orange", "#FF7477"),
    ("sunflower", "#FFB511"),
    ("purple", "#765BA7"),
    ("burgundy", "#914E72"),
    ("flat-gold", "#BB8B41"),
    ("light-gray", "#88898A"),
    ("black", "#2A2A2A"),
];

fn riso(name: &str) -> Option<Rgba> {
    let n = name.trim().to_ascii_lowercase().replace([' ', '_'], "-");
    RISO_INKS.iter().find(|(k, _)| *k == n).and_then(|(_, v)| parse_css_color(v))
}

/// OKLCH → sRGB, reducing chroma until the colour is in gamut (keeps lightness and hue).
pub fn oklch(l: f32, c: f32, h_deg: f32) -> Rgba {
    use anikit_core::color::{linear_to_srgb, oklab_to_linear};
    let h = h_deg.to_radians();
    let inside = |c: f32| {
        let v = oklab_to_linear(l as f64, (c * h.cos()) as f64, (c * h.sin()) as f64);
        v.iter().all(|x| (-1e-4..=1.0001).contains(x))
    };
    let (mut lo, mut hi) = (0.0f32, c.max(0.0));
    if !inside(hi) {
        for _ in 0..20 {
            let m = 0.5 * (lo + hi);
            if inside(m) {
                lo = m
            } else {
                hi = m
            }
        }
        hi = lo;
    }
    let v = oklab_to_linear(l as f64, (hi * h.cos()) as f64, (hi * h.sin()) as f64);
    let f = |x: f64| (linear_to_srgb(x.clamp(0.0, 1.0)) as f32).clamp(0.0, 1.0);
    Rgba([f(v[0]), f(v[1]), f(v[2]), 1.0])
}

/// Palette schemes for [`palette`].
pub const SCHEMES: &[&str] = &["mono", "analogous", "complement", "split", "triad"];

/// A value-structured palette from a seed colour: `ink` (L ≈ 0.18), `paper` (≈ 0.96), `mid` (0.62),
/// `accent` (the seed), `accent2` (by scheme), `tint` (a pale accent), plus the theme roles
/// (`bg`, `text`, `primary`, `secondary`, `muted`, `surface`). `dark` swaps ink and paper.
pub fn palette(spec: &str) -> Result<Vec<(String, Rgba)>, String> {
    let mut words = spec.split_whitespace();
    let seed_s = words.next().ok_or("palette needs a seed colour, e.g. \"#e4572e split\"")?;
    let seed = parse_css_color(seed_s).ok_or_else(|| format!("palette: invalid seed colour '{seed_s}'"))?;
    let mut scheme = "complement";
    let mut dark = false;
    for w in words {
        match w {
            "dark" => dark = true,
            "light" => dark = false,
            s if SCHEMES.contains(&s) => scheme = SCHEMES.iter().find(|x| **x == s).unwrap(),
            s => return Err(format!("palette: unknown word '{s}' (schemes: {}; or dark/light)", SCHEMES.join(", "))),
        }
    }
    let lab = super::rgba_to_oklab(seed);
    let (l, c) = (lab[0], (lab[1] * lab[1] + lab[2] * lab[2]).sqrt());
    let h = lab[2].atan2(lab[1]).to_degrees();
    let h2 = match scheme {
        "analogous" => h + 38.0,
        "complement" => h + 180.0,
        "split" => h + 150.0,
        "triad" => h + 120.0,
        _ => h,
    };
    // Neutrals carry a trace of the seed's hue, so the palette reads as one family.
    let ink = oklch(0.19, (c * 0.18).min(0.035), h);
    // Near-neutral paper: a tinted cream from a warm seed is a cliché (BAR.md), so only a trace of hue.
    let paper = oklch(0.97, (c * 0.04).min(0.006), h + 20.0);
    let mid = oklch(0.62, c * 0.35, h);
    let accent2 = if scheme == "mono" { oklch((l + 0.32).min(0.9), c * 0.6, h) } else { oklch(l.clamp(0.45, 0.8), c * 0.9, h2) };
    let tint = oklch(0.9, c * 0.25, h);
    let (bg, text) = if dark { (ink, paper) } else { (paper, ink) };
    let muted = oklch(if dark { 0.72 } else { 0.45 }, (c * 0.12).min(0.03), h);
    let surface = if dark { oklch(0.25, (c * 0.18).min(0.035), h) } else { oklch(0.93, (c * 0.04).min(0.008), h + 20.0) };
    Ok(vec![
        ("ink".into(), ink),
        ("paper".into(), paper),
        ("mid".into(), mid),
        ("accent".into(), seed),
        ("accent2".into(), accent2),
        ("tint".into(), tint),
        ("bg".into(), bg),
        ("text".into(), text),
        ("primary".into(), seed),
        ("secondary".into(), accent2),
        ("muted".into(), muted),
        ("surface".into(), surface),
    ])
}

/// WCAG contrast ratio of two colours.
pub fn contrast(a: Rgba, b: Rgba) -> f32 {
    let lum = |c: Rgba| {
        let f = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * f(c.0[0]) + 0.7152 * f(c.0[1]) + 0.0722 * f(c.0[2])
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn riso_inks_and_palettes() {
        assert_eq!(parse_css_color("riso:Fluorescent Pink"), parse_css_color("#FF48B0"));
        assert!(parse_css_color("riso:nope").is_none());
        for s in SCHEMES {
            let p = palette(&format!("#e4572e {s}")).unwrap();
            let get = |k: &str| p.iter().find(|(n, _)| n == k).unwrap().1;
            // Value structure: text on bg is AAA, the accent is not lost on the paper.
            assert!(contrast(get("text"), get("bg")) >= 12.0, "{s}");
            assert!(contrast(get("accent"), get("bg")) >= 2.5, "{s}");
        }
        let d = palette("#3255a4 split dark").unwrap();
        assert!(contrast(d[6].1, d[7].1) >= 12.0);
        assert!(palette("#zz").is_err() && palette("#e4572e wobbly").is_err());
    }
}
