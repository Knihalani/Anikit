//! Colour helpers for brand kits and templates: parsing, mixing, WCAG contrast.

/// Straight-alpha sRGB, components 0..1.
pub type C = [f64; 4];

/// Parse `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)`, `rgba(r,g,b,a)` or a few names.
pub fn parse(s: &str) -> Result<C, String> {
    let t = s.trim();
    let named = match t.to_ascii_lowercase().as_str() {
        "white" => Some("#ffffff"),
        "black" => Some("#000000"),
        "transparent" => Some("#00000000"),
        _ => None,
    };
    let t = named.unwrap_or(t);
    if let Some(h) = t.strip_prefix('#') {
        let d: Vec<u8> = h
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect::<Option<_>>()
            .ok_or_else(|| format!("'{s}' is not a hex colour"))?;
        let v = match d.len() {
            3 | 4 => d.iter().map(|x| x * 17).collect::<Vec<_>>(),
            6 | 8 => d.chunks(2).map(|p| p[0] * 16 + p[1]).collect(),
            _ => return Err(format!("'{s}' is not a hex colour (use #RRGGBB)")),
        };
        let a = if v.len() == 4 { v[3] as f64 / 255.0 } else { 1.0 };
        return Ok([v[0] as f64 / 255.0, v[1] as f64 / 255.0, v[2] as f64 / 255.0, a]);
    }
    if let Some(inner) = t.strip_prefix("rgba(").or_else(|| t.strip_prefix("rgb(")).and_then(|x| x.strip_suffix(')')) {
        let p: Vec<f64> = inner.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("bad colour '{s}'"))?;
        if p.len() == 3 || p.len() == 4 {
            return Ok([p[0] / 255.0, p[1] / 255.0, p[2] / 255.0, *p.get(3).unwrap_or(&1.0)]);
        }
    }
    Err(format!("'{s}' is not a colour (use #RRGGBB, #RRGGBBAA or rgb(r, g, b))"))
}

/// `#RRGGBB`, or `#RRGGBBAA` when not opaque.
pub fn to_hex(c: C) -> String {
    let b = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    if c[3] >= 0.999 {
        format!("#{:02X}{:02X}{:02X}", b(c[0]), b(c[1]), b(c[2]))
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", b(c[0]), b(c[1]), b(c[2]), b(c[3]))
    }
}

pub fn mix(a: C, b: C, t: f64) -> C {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn lin(x: f64) -> f64 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance.
pub fn luminance(c: C) -> f64 {
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio, 1..21.
pub fn contrast(a: C, b: C) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Black or white, whichever has more contrast on `bg`.
pub fn contrast_text(bg: C) -> C {
    let (w, k) = ([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
    if contrast(bg, w) >= contrast(bg, k) {
        w
    } else {
        k
    }
}

/// Composite straight-alpha `fg` over opaque `bg`.
pub fn over(fg: C, bg: C) -> C {
    let a = fg[3];
    [fg[0] * a + bg[0] * (1.0 - a), fg[1] * a + bg[1] * (1.0 - a), fg[2] * a + bg[2] * (1.0 - a), 1.0]
}

/// Rotate the hue of `c` by `deg` degrees (for deriving a secondary colour).
pub fn hue_shift(c: C, deg: f64) -> C {
    let (r, g, b) = (c[0], c[1], c[2]);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-9 {
        return c;
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let mut h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    h = (h + deg).rem_euclid(360.0);
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let h = h / 360.0;
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0), c[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format() {
        assert_eq!(to_hex(parse("#abc").unwrap()), "#AABBCC");
        assert_eq!(to_hex(parse("#6E56CF").unwrap()), "#6E56CF");
        assert_eq!(to_hex(parse("rgb(255, 0, 0)").unwrap()), "#FF0000");
        assert_eq!(to_hex(parse("#FF000080").unwrap()), "#FF000080");
        assert!(parse("#12345").is_err());
        assert!(parse("blue-ish").is_err());
    }

    #[test]
    fn contrast_ratios() {
        let w = parse("#fff").unwrap();
        let k = parse("#000").unwrap();
        assert!((contrast(w, k) - 21.0).abs() < 0.01);
        assert_eq!(to_hex(contrast_text(parse("#FFD400").unwrap())), "#000000");
        assert_eq!(to_hex(contrast_text(parse("#1E1B4B").unwrap())), "#FFFFFF");
        assert_eq!(to_hex(hue_shift(parse("#FF0000").unwrap(), 120.0)), "#00FF00");
    }
}
