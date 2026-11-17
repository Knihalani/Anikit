//! Brand kits (`brand.json`): colours, fonts, logo and voice. Anything left
//! out is derived so the result is always legible (text colours are chosen
//! for contrast against the background).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::color;

/// Brand colours. All optional; missing ones are derived from the others.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    /// Main brand colour (buttons, highlights, gradients).
    pub primary: Option<String>,
    /// Second brand colour (gradient partner, charts). Default: primary shifted −20° in hue (an analogous, on-brand partner).
    pub secondary: Option<String>,
    /// Sparing highlight colour (underlines, a single number). Default: secondary.
    pub accent: Option<String>,
    /// Canvas colour. Default: near-black (dark theme) or near-white (light theme).
    pub background: Option<String>,
    /// Card and panel colour. Default: background mixed 6% towards text.
    pub surface: Option<String>,
    /// Main text colour. Default: black or white for the best contrast on background.
    pub text: Option<String>,
    /// Secondary text colour. Default: text mixed 38% towards background.
    pub muted: Option<String>,
}

/// Brand fonts (family names).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Fonts {
    /// Headlines. Default "Inter Display".
    pub heading: Option<String>,
    /// Body copy. Default "Inter".
    pub body: Option<String>,
    /// Code. Default "JetBrains Mono".
    pub mono: Option<String>,
}

/// Brand voice, used by the prompts (not by rendering).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Voice {
    /// For example "confident, warm, precise".
    pub tone: Option<String>,
    /// Words and phrases never to use.
    pub avoid: Vec<String>,
    /// Default call to action, e.g. "Start free".
    pub cta: Option<String>,
    /// Website shown on end cards, e.g. "acme.dev".
    pub url: Option<String>,
}

/// A brand kit, as read from `brand.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Brand {
    /// Product or company name.
    pub name: Option<String>,
    /// `"dark"` (default) or `"light"`.
    pub theme: Option<String>,
    pub colors: Colors,
    pub fonts: Fonts,
    /// Logo image path (SVG or PNG), relative to the brand file.
    pub logo: Option<String>,
    /// Corner radius for cards, in px at 1080p. Default 28.
    pub radius: Option<f64>,
    pub voice: Voice,
}

/// Keys accepted in `brand.json`, for "did you mean" errors.
pub const BRAND_KEYS: &[&str] = &["name", "theme", "colors", "fonts", "logo", "radius", "voice"];

impl Brand {
    /// Parse `brand.json` with actionable errors.
    pub fn from_json(src: &str) -> Result<Brand, String> {
        let v: Value = serde_json::from_str(src).map_err(|e| format!("brand.json: {e}"))?;
        Brand::from_value(&v)
    }

    pub fn from_value(v: &Value) -> Result<Brand, String> {
        if let Some(m) = v.as_object() {
            let groups: [(&str, &[&str]); 3] = [
                ("colors", &["primary", "secondary", "accent", "background", "surface", "text", "muted"]),
                ("fonts", &["heading", "body", "mono"]),
                ("voice", &["tone", "avoid", "cta", "url"]),
            ];
            for k in m.keys() {
                if !BRAND_KEYS.contains(&k.as_str()) {
                    return Err(unknown("brand", k, BRAND_KEYS));
                }
            }
            for (g, keys) in groups {
                if let Some(o) = m.get(g).and_then(|x| x.as_object()) {
                    for k in o.keys() {
                        if !keys.contains(&k.as_str()) {
                            return Err(unknown(&format!("brand.{g}"), k, keys));
                        }
                    }
                }
            }
        }
        let b: Brand = serde_json::from_value(v.clone()).map_err(|e| format!("brand.json: {e}"))?;
        // Validate colours now so the error names the field.
        let c = &b.colors;
        for (k, val) in [
            ("primary", &c.primary),
            ("secondary", &c.secondary),
            ("accent", &c.accent),
            ("background", &c.background),
            ("surface", &c.surface),
            ("text", &c.text),
            ("muted", &c.muted),
        ] {
            if let Some(s) = val {
                color::parse(s).map_err(|e| format!("brand.colors.{k}: {e}"))?;
            }
        }
        if let Some(t) = &b.theme {
            if t != "dark" && t != "light" {
                return Err(format!("brand.theme: '{t}' must be \"dark\" or \"light\""));
            }
        }
        Ok(b)
    }

    /// The resolved kit as the `brand` object templates see: every field filled.
    pub fn resolve(&self) -> Value {
        let light = self.theme.as_deref() == Some("light")
            || (self.theme.is_none()
                && self.colors.background.as_deref().and_then(|s| color::parse(s).ok()).is_some_and(|c| color::luminance(c) > 0.4));
        let p = |o: &Option<String>| o.as_deref().and_then(|s| color::parse(s).ok());
        let primary = p(&self.colors.primary).unwrap_or([0.43, 0.34, 0.98, 1.0]); // #6E56FA
        let bg = p(&self.colors.background).unwrap_or(if light { [0.98, 0.98, 0.97, 1.0] } else { [0.035, 0.035, 0.05, 1.0] });
        let text = p(&self.colors.text).unwrap_or_else(|| {
            let t = color::contrast_text(bg);
            // Pure white/black is harsh on screen; soften a touch.
            color::mix(t, bg, 0.04)
        });
        let secondary = p(&self.colors.secondary).unwrap_or_else(|| color::hue_shift(primary, -20.0));
        let accent = p(&self.colors.accent).unwrap_or(secondary);
        let surface = p(&self.colors.surface).unwrap_or_else(|| color::mix(bg, text, if light { 0.04 } else { 0.07 }));
        let muted = p(&self.colors.muted).unwrap_or_else(|| color::mix(text, bg, 0.38));
        let h = color::to_hex;
        let name = self.name.clone().unwrap_or_else(|| "Your Product".into());
        json!({
            "name": name,
            "theme": if light { "light" } else { "dark" },
            "colors": {
                "primary": h(primary),
                "secondary": h(secondary),
                "accent": h(accent),
                "background": h(bg),
                "surface": h(surface),
                "text": h(text),
                "muted": h(muted),
                "onPrimary": h(color::contrast_text(primary)),
                "onAccent": h(color::contrast_text(accent)),
                "border": h(color::mix(bg, text, 0.14)),
                "glow": h([primary[0], primary[1], primary[2], 0.35]),
                "backgroundDeep": h(color::mix(bg, [0.0, 0.0, 0.0, 1.0], if light { 0.04 } else { 0.35 })),
            },
            "fonts": {
                "heading": self.fonts.heading.clone().unwrap_or_else(|| "Inter Display".into()),
                "body": self.fonts.body.clone().unwrap_or_else(|| "Inter".into()),
                "mono": self.fonts.mono.clone().unwrap_or_else(|| "JetBrains Mono".into()),
            },
            "logo": self.logo.clone(),
            "radius": self.radius.unwrap_or(28.0),
            "voice": {
                "tone": self.voice.tone.clone().unwrap_or_else(|| "confident, clear, warm".into()),
                "avoid": self.voice.avoid.clone(),
                "cta": self.voice.cta.clone().unwrap_or_else(|| "Try it today".into()),
                "url": self.voice.url.clone(),
            },
        })
    }
}

fn unknown(at: &str, k: &str, keys: &[&str]) -> String {
    let hint = super::suggest(k, keys.iter().copied()).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
    format!("{at}: unknown key '{k}'{hint}; allowed: {}", keys.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_missing_colours_with_contrast() {
        let b = Brand::from_json(r##"{"name":"Acme","colors":{"primary":"#FFD400","background":"#FFFFFF"}}"##).unwrap();
        let r = b.resolve();
        assert_eq!(r["theme"], "light");
        let text = color::parse(r["colors"]["text"].as_str().unwrap()).unwrap();
        assert!(color::contrast(text, color::parse("#FFFFFF").unwrap()) > 12.0);
        assert_eq!(r["colors"]["onPrimary"], "#000000");
        assert_eq!(r["fonts"]["heading"], "Inter Display");
    }

    #[test]
    fn unknown_keys_suggest() {
        let e = Brand::from_json(r##"{"colours":{}}"##).unwrap_err();
        assert!(e.contains("did you mean 'colors'"), "{e}");
        let e = Brand::from_json(r##"{"colors":{"primay":"#fff"}}"##).unwrap_err();
        assert!(e.contains("did you mean 'primary'"), "{e}");
        let e = Brand::from_json(r##"{"colors":{"primary":"purple-ish"}}"##).unwrap_err();
        assert!(e.contains("brand.colors.primary"), "{e}");
    }
}
