//! Colour values in text specs: `"#rgb"`, `"#rgba"`, `"#rrggbb"`, `"#rrggbbaa"`,
//! `"rgba(255, 0, 0, 0.5)"`, a basic name, or `[r, g, b, a]` (0..1).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::dl::Rgba;

/// sRGB colour, straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color(pub Rgba);

impl Color {
    pub const WHITE: Color = Color(Rgba([1.0, 1.0, 1.0, 1.0]));
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color(Rgba([r, g, b, a]))
    }
    /// Parse `#rgb`, `#rrggbb` or `#rrggbbaa`.
    pub fn hex(s: &str) -> Option<Self> {
        let h = s.trim().strip_prefix('#')?;
        let n = |i: usize, l: usize| u8::from_str_radix(&h[i..i + l], 16).ok();
        let (r, g, b, a) = match h.len() {
            3 => (n(0, 1)? * 17, n(1, 1)? * 17, n(2, 1)? * 17, 255),
            4 => (n(0, 1)? * 17, n(1, 1)? * 17, n(2, 1)? * 17, n(3, 1)? * 17),
            6 => (n(0, 2)?, n(2, 2)?, n(4, 2)?, 255),
            8 => (n(0, 2)?, n(2, 2)?, n(4, 2)?, n(6, 2)?),
            _ => return None,
        };
        Some(Color(Rgba([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0])))
    }
    /// Parse any supported form: hex, `rgb(…)`/`rgba(…)` (0–255, alpha 0–1)
    /// or a basic name (`white`, `black`, `transparent`, `red`, …).
    pub fn parse(s: &str) -> Option<Self> {
        let t = s.trim();
        if t.starts_with('#') {
            return Self::hex(t);
        }
        if let Some(inner) = t.strip_prefix("rgba(").or_else(|| t.strip_prefix("rgb(")).and_then(|x| x.strip_suffix(')')) {
            let v: Vec<f32> = inner.split([',', ' ', '/']).filter(|x| !x.is_empty()).filter_map(|x| x.trim().parse().ok()).collect();
            return match v.as_slice() {
                [r, g, b] => Some(Color::rgba(r / 255.0, g / 255.0, b / 255.0, 1.0)),
                [r, g, b, a] => Some(Color::rgba(r / 255.0, g / 255.0, b / 255.0, *a)),
                _ => None,
            };
        }
        Some(match t.to_lowercase().as_str() {
            "white" => Color::rgba(1.0, 1.0, 1.0, 1.0),
            "black" => Color::rgba(0.0, 0.0, 0.0, 1.0),
            "transparent" | "none" => Color::rgba(0.0, 0.0, 0.0, 0.0),
            "red" => Color::rgba(1.0, 0.0, 0.0, 1.0),
            "green" => Color::rgba(0.0, 0.5, 0.0, 1.0),
            "blue" => Color::rgba(0.0, 0.0, 1.0, 1.0),
            "yellow" => Color::rgba(1.0, 1.0, 0.0, 1.0),
            "gray" | "grey" => Color::rgba(0.5, 0.5, 0.5, 1.0),
            _ => return None,
        })
    }

    pub fn with_alpha(self, a: f32) -> Self {
        let mut c = self.0 .0;
        c[3] = a;
        Color(Rgba(c))
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0 .0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            S(String),
            A3([f32; 3]),
            A4([f32; 4]),
        }
        match Raw::deserialize(d)? {
            Raw::S(s) => Color::parse(&s)
                .ok_or_else(|| serde::de::Error::custom(format!("bad colour {s:?}; use \"#rrggbb\", \"#rrggbbaa\" or \"rgba(r, g, b, a)\""))),
            Raw::A3([r, g, b]) => Ok(Color(Rgba([r, g, b, 1.0]))),
            Raw::A4(a) => Ok(Color(Rgba(a))),
        }
    }
}
