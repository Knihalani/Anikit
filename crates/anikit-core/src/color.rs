//! CSS Color parsing, conversion and perceptual interpolation.
//!
//! Colors are parsed into straight-alpha sRGB (`r, g, b` in 0–255, `a` in 0–1).
//! For interpolation they are converted once (at animation creation) into the
//! chosen interpolation space with premultiplied alpha, and converted back to
//! sRGB per frame. OKLab/OKLCH give perceptually even gradients without the
//! muddy midpoints of naive sRGB blending.

use crate::math::{atan2, cbrt, clamp, cos, fmod_pos, pow, sin, sqrt, DEG, RAD};
use crate::util::{split_call, Cursor};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Rgba { r, g, b, a }
    }
    pub fn to_array(self) -> [f64; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ColorSpace {
    Srgb = 0,
    LinearSrgb = 1,
    Oklab = 2,
    Oklch = 3,
    Hsl = 4,
}

impl ColorSpace {
    pub fn from_u8(v: u8) -> ColorSpace {
        match v {
            0 => ColorSpace::Srgb,
            1 => ColorSpace::LinearSrgb,
            3 => ColorSpace::Oklch,
            4 => ColorSpace::Hsl,
            _ => ColorSpace::Oklab,
        }
    }
    pub fn parse(s: &str) -> Option<ColorSpace> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "srgb" | "rgb" => ColorSpace::Srgb,
            "linear" | "srgb-linear" | "linear-srgb" => ColorSpace::LinearSrgb,
            "oklab" | "lab" => ColorSpace::Oklab,
            "oklch" | "lch" => ColorSpace::Oklch,
            "hsl" => ColorSpace::Hsl,
            _ => return None,
        })
    }
    /// Index of the hue channel, if the space is polar.
    pub fn hue_index(self) -> Option<usize> {
        match self {
            ColorSpace::Oklch => Some(2),
            ColorSpace::Hsl => Some(0),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Transfer functions & matrices
// ---------------------------------------------------------------------------

#[inline]
pub fn srgb_to_linear(c: f64) -> f64 {
    let a = if c < 0.0 { -c } else { c };
    let v = if a <= 0.04045 { a / 12.92 } else { pow((a + 0.055) / 1.055, 2.4) };
    if c < 0.0 {
        -v
    } else {
        v
    }
}
/// sRGB transfer function (encode). Uses a dedicated polynomial for
/// `a^(1/2.4)` ([`crate::math::pow_5_12`], relative error < 5e-9, i.e. about
/// 1e-6 of an 8-bit step) because this runs per colour channel per frame.
#[inline]
pub fn linear_to_srgb(c: f64) -> f64 {
    let a = if c < 0.0 { -c } else { c };
    let v = if a <= 0.0031308 { a * 12.92 } else { 1.055 * crate::math::pow_5_12(a) - 0.055 };
    if c < 0.0 {
        -v
    } else {
        v
    }
}

/// Linear sRGB (0–1) → OKLab.
#[inline]
pub fn linear_to_oklab(r: f64, g: f64, b: f64) -> [f64; 3] {
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
    let (l, m, s) = (cbrt(l), cbrt(m), cbrt(s));
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}
/// OKLab → linear sRGB (0–1).
#[inline]
pub fn oklab_to_linear(l: f64, a: f64, b: f64) -> [f64; 3] {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    [
        4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
        -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
        -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3,
    ]
}

pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [f64; 3] {
    // h in degrees, s/l in 0..1, result 0..1
    let h = fmod_pos(h, 360.0);
    let f = |n: f64| {
        let k = fmod_pos(n + h / 30.0, 12.0);
        let a = s * l.min(1.0 - l);
        l - a * clamp((k - 3.0).min(9.0 - k), -1.0, 1.0)
    };
    [f(0.0), f(8.0), f(4.0)]
}

pub fn rgb_to_hsl(r: f64, g: f64, b: f64) -> [f64; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return [f64::NAN, 0.0, l];
    }
    let s = if l == 0.0 || l == 1.0 { 0.0 } else { (max - l) / l.min(1.0 - l) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h * 60.0, s, l]
}

fn hwb_to_rgb(h: f64, w: f64, bl: f64) -> [f64; 3] {
    if w + bl >= 1.0 {
        let g = w / (w + bl);
        return [g, g, g];
    }
    let rgb = hsl_to_rgb(h, 1.0, 0.5);
    let k = 1.0 - w - bl;
    [rgb[0] * k + w, rgb[1] * k + w, rgb[2] * k + w]
}

/// CIE Lab (D50) → linear sRGB.
fn lab_to_linear(l: f64, a: f64, b: f64) -> [f64; 3] {
    const K: f64 = 24389.0 / 27.0;
    const E: f64 = 216.0 / 24389.0;
    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let x = if fx * fx * fx > E { fx * fx * fx } else { (116.0 * fx - 16.0) / K };
    let y = if l > K * E { fy * fy * fy } else { l / K };
    let z = if fz * fz * fz > E { fz * fz * fz } else { (116.0 * fz - 16.0) / K };
    // D50 white
    let (x, y, z) = (x * 0.3457 / 0.3585, y, z * (1.0 - 0.3457 - 0.3585) / 0.3585);
    // Bradford D50 → D65
    let x65 = 0.9554734527042182 * x - 0.023098536874261423 * y + 0.0632593086610217 * z;
    let y65 = -0.028369706963208136 * x + 1.0099954580058226 * y + 0.021041398966943008 * z;
    let z65 = 0.012314001688319899 * x - 0.020507696433477912 * y + 1.3303659366080753 * z;
    [
        3.2409699419045226 * x65 - 1.537383177570094 * y65 - 0.4986107602930034 * z65,
        -0.9692436362808796 * x65 + 1.8759675015077202 * y65 + 0.04155505740717559 * z65,
        0.05563007969699366 * x65 - 0.20397695888897652 * y65 + 1.0569715142428786 * z65,
    ]
}

// ---------------------------------------------------------------------------
// Space conversion used by the interpolation engine
// ---------------------------------------------------------------------------

/// Convert straight sRGB (0–255, a 0–1) to premultiplied coordinates in `space`.
pub fn to_space(c: Rgba, space: ColorSpace) -> [f64; 4] {
    let (r, g, b, a) = (c.r / 255.0, c.g / 255.0, c.b / 255.0, clamp(c.a, 0.0, 1.0));
    let v = match space {
        ColorSpace::Srgb => [r, g, b],
        ColorSpace::LinearSrgb => [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)],
        ColorSpace::Oklab => linear_to_oklab(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)),
        ColorSpace::Oklch => {
            let lab = linear_to_oklab(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
            let ch = sqrt(lab[1] * lab[1] + lab[2] * lab[2]);
            let h = if ch < 2e-4 { f64::NAN } else { fmod_pos(atan2(lab[2], lab[1]) * DEG, 360.0) };
            [lab[0], ch, h]
        }
        ColorSpace::Hsl => rgb_to_hsl(r, g, b),
    };
    // Premultiply every non-hue channel.
    match space {
        ColorSpace::Oklch => [v[0] * a, v[1] * a, v[2], a],
        ColorSpace::Hsl => [v[0], v[1] * a, v[2] * a, a],
        _ => [v[0] * a, v[1] * a, v[2] * a, a],
    }
}

/// Inverse of [`to_space`]: premultiplied coordinates → straight sRGB 0–255.
#[inline]
pub fn from_space(v: &[f64], space: ColorSpace) -> Rgba {
    let a = clamp(v[3], 0.0, 1.0);
    if a <= 0.0 {
        return Rgba::new(0.0, 0.0, 0.0, 0.0);
    }
    let inv = 1.0 / a;
    let rgb = match space {
        ColorSpace::Srgb => [v[0] * inv, v[1] * inv, v[2] * inv],
        ColorSpace::LinearSrgb => {
            [linear_to_srgb(v[0] * inv), linear_to_srgb(v[1] * inv), linear_to_srgb(v[2] * inv)]
        }
        ColorSpace::Oklab => {
            let l = oklab_to_linear(v[0] * inv, v[1] * inv, v[2] * inv);
            [linear_to_srgb(l[0]), linear_to_srgb(l[1]), linear_to_srgb(l[2])]
        }
        ColorSpace::Oklch => {
            let (l, ch, h) = (v[0] * inv, v[1] * inv, v[2] * RAD);
            let lin = oklab_to_linear(l, ch * cos(h), ch * sin(h));
            [linear_to_srgb(lin[0]), linear_to_srgb(lin[1]), linear_to_srgb(lin[2])]
        }
        ColorSpace::Hsl => hsl_to_rgb(v[0], v[1] * inv, v[2] * inv),
    };
    Rgba::new(
        clamp(rgb[0], 0.0, 1.0) * 255.0,
        clamp(rgb[1], 0.0, 1.0) * 255.0,
        clamp(rgb[2], 0.0, 1.0) * 255.0,
        a,
    )
}

/// Fix up a list of keyframe colours already in `space` (4 values each) so
/// that polar hues take the shortest arc and achromatic hues borrow their
/// neighbour's hue (CSS "powerless" hue handling). Runs once at creation.
pub fn fix_hues(values: &mut [f64], space: ColorSpace) {
    let Some(hi) = space.hue_index() else { return };
    let n = values.len() / 4;
    if n == 0 {
        return;
    }
    // Fill NaN hues from the nearest defined neighbour.
    let defined: alloc::vec::Vec<usize> = (0..n).filter(|&i| !values[i * 4 + hi].is_nan()).collect();
    if defined.is_empty() {
        for i in 0..n {
            values[i * 4 + hi] = 0.0;
        }
        return;
    }
    for i in 0..n {
        if values[i * 4 + hi].is_nan() {
            let nearest = *defined.iter().min_by_key(|&&d| d.abs_diff(i)).unwrap_or(&0);
            values[i * 4 + hi] = values[nearest * 4 + hi];
        }
    }
    // Unwrap for shortest path.
    for i in 1..n {
        let prev = values[(i - 1) * 4 + hi];
        let mut h = values[i * 4 + hi];
        while h - prev > 180.0 {
            h -= 360.0;
        }
        while h - prev < -180.0 {
            h += 360.0;
        }
        values[i * 4 + hi] = h;
    }
}

/// Interpolate two straight sRGB colours in `space` (convenience API).
pub fn mix(a: Rgba, b: Rgba, t: f64, space: ColorSpace) -> Rgba {
    let mut v = [0.0; 8];
    v[..4].copy_from_slice(&to_space(a, space));
    v[4..].copy_from_slice(&to_space(b, space));
    fix_hues(&mut v, space);
    let mut o = [0.0; 4];
    for i in 0..4 {
        o[i] = v[i] + (v[i + 4] - v[i]) * t;
    }
    from_space(&o, space)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn hex_val(c: u8) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as u32),
        b'a'..=b'f' => Some((c - b'a' + 10) as u32),
        b'A'..=b'F' => Some((c - b'A' + 10) as u32),
        _ => None,
    }
}

fn parse_hex(h: &str) -> Option<Rgba> {
    let b = h.as_bytes();
    let mut v = [0u32; 8];
    for (i, &c) in b.iter().enumerate() {
        if i >= 8 {
            return None;
        }
        v[i] = hex_val(c)?;
    }
    let (r, g, bl, a) = match b.len() {
        3 => (v[0] * 17, v[1] * 17, v[2] * 17, 255),
        4 => (v[0] * 17, v[1] * 17, v[2] * 17, v[3] * 17),
        6 => (v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], 255),
        8 => (v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], v[6] * 16 + v[7]),
        _ => return None,
    };
    Some(Rgba::new(r as f64, g as f64, bl as f64, a as f64 / 255.0))
}

/// A component of a colour function: number, percentage, angle, or `none`.
#[derive(Clone, Copy)]
struct Comp {
    v: f64,
    pct: bool,
}

fn parse_components(args: &str) -> Option<([Comp; 4], usize)> {
    let mut c = Cursor::new(args);
    let mut out = [Comp { v: 0.0, pct: false }; 4];
    let mut n = 0;
    loop {
        c.skip_ws();
        if c.eat(b',') || c.eat(b'/') {
            continue;
        }
        if c.done() || n >= 4 {
            break;
        }
        if let Some(v) = c.number() {
            let unit = c.ident();
            let pct = c.eat(b'%');
            let v = match unit {
                "deg" | "" => v,
                "rad" => v * DEG,
                "grad" => v * 0.9,
                "turn" => v * 360.0,
                _ => v,
            };
            out[n] = Comp { v, pct };
            n += 1;
        } else {
            let id = c.ident();
            if id.eq_ignore_ascii_case("none") {
                out[n] = Comp { v: 0.0, pct: false };
                n += 1;
            } else {
                return None;
            }
        }
    }
    Some((out, n))
}

fn alpha_of(c: &[Comp; 4], n: usize, idx: usize) -> f64 {
    if n > idx {
        let a = c[idx];
        clamp(if a.pct { a.v / 100.0 } else { a.v }, 0.0, 1.0)
    } else {
        1.0
    }
}

/// Parse any CSS colour (hex, rgb[a], hsl[a], hwb, lab, lch, oklab, oklch,
/// `color(srgb|srgb-linear ...)`, named colours, `transparent`).
pub fn parse(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        return parse_hex(h);
    }
    let (name, args) = split_call(s);
    let lname = name.to_ascii_lowercase();
    let Some(args) = args else {
        return named(&lname);
    };
    if lname == "color" {
        return parse_color_fn(args);
    }
    let (c, n) = parse_components(args)?;
    if n < 3 {
        return None;
    }
    let pct255 = |x: Comp| if x.pct { x.v * 2.55 } else { x.v };
    let unit = |x: Comp| if x.pct { x.v / 100.0 } else { x.v };
    let rgb01 = |l: [f64; 3]| [linear_to_srgb(l[0]) * 255.0, linear_to_srgb(l[1]) * 255.0, linear_to_srgb(l[2]) * 255.0];
    let (rgb, a) = match lname.as_str() {
        "rgb" | "rgba" => ([pct255(c[0]), pct255(c[1]), pct255(c[2])], alpha_of(&c, n, 3)),
        "hsl" | "hsla" => {
            let v = hsl_to_rgb(c[0].v, c[1].v / 100.0, c[2].v / 100.0);
            ([v[0] * 255.0, v[1] * 255.0, v[2] * 255.0], alpha_of(&c, n, 3))
        }
        "hwb" => {
            let v = hwb_to_rgb(c[0].v, c[1].v / 100.0, c[2].v / 100.0);
            ([v[0] * 255.0, v[1] * 255.0, v[2] * 255.0], alpha_of(&c, n, 3))
        }
        "oklab" => {
            let l = unit(c[0]);
            let ab = |x: Comp| if x.pct { x.v / 100.0 * 0.4 } else { x.v };
            (rgb01(oklab_to_linear(l, ab(c[1]), ab(c[2]))), alpha_of(&c, n, 3))
        }
        "oklch" => {
            let l = unit(c[0]);
            let ch = if c[1].pct { c[1].v / 100.0 * 0.4 } else { c[1].v };
            let h = c[2].v * RAD;
            (rgb01(oklab_to_linear(l, ch * cos(h), ch * sin(h))), alpha_of(&c, n, 3))
        }
        "lab" => {
            let ab = |x: Comp| if x.pct { x.v / 100.0 * 125.0 } else { x.v };
            (rgb01(lab_to_linear(c[0].v, ab(c[1]), ab(c[2]))), alpha_of(&c, n, 3))
        }
        "lch" => {
            let ch = if c[1].pct { c[1].v / 100.0 * 150.0 } else { c[1].v };
            let h = c[2].v * RAD;
            (rgb01(lab_to_linear(c[0].v, ch * cos(h), ch * sin(h))), alpha_of(&c, n, 3))
        }
        _ => return None,
    };
    Some(Rgba::new(clamp(rgb[0], 0.0, 255.0), clamp(rgb[1], 0.0, 255.0), clamp(rgb[2], 0.0, 255.0), a))
}

/// `color(srgb r g b / a)` and `color(srgb-linear r g b / a)`.
fn parse_color_fn(args: &str) -> Option<Rgba> {
    let mut cur = Cursor::new(args);
    cur.skip_ws();
    let space = cur.ident();
    let (cc, nn) = parse_components(&args[cur.i..])?;
    if nn < 3 {
        return None;
    }
    let unit = |x: Comp| if x.pct { x.v / 100.0 } else { x.v };
    let v = [unit(cc[0]), unit(cc[1]), unit(cc[2])];
    let rgb = match space {
        "srgb" => v,
        "srgb-linear" => [linear_to_srgb(v[0]), linear_to_srgb(v[1]), linear_to_srgb(v[2])],
        _ => return None,
    };
    Some(Rgba::new(
        clamp(rgb[0] * 255.0, 0.0, 255.0),
        clamp(rgb[1] * 255.0, 0.0, 255.0),
        clamp(rgb[2] * 255.0, 0.0, 255.0),
        alpha_of(&cc, nn, 3),
    ))
}

/// Format as a compact CSS `rgb()`/`rgba()` string.
pub fn to_css(c: Rgba) -> alloc::string::String {
    use crate::util::push_num;
    let mut s = alloc::string::String::with_capacity(24);
    let opaque = c.a >= 1.0;
    s.push_str(if opaque { "rgb(" } else { "rgba(" });
    push_num(&mut s, c.r, 0);
    s.push(',');
    push_num(&mut s, c.g, 0);
    s.push(',');
    push_num(&mut s, c.b, 0);
    if !opaque {
        s.push(',');
        push_num(&mut s, c.a, 3);
    }
    s.push(')');
    s
}

/// Hex form `#rrggbb` / `#rrggbbaa`.
pub fn to_hex(c: Rgba) -> alloc::string::String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = alloc::string::String::with_capacity(9);
    s.push('#');
    let mut push = |v: f64| {
        let v = clamp(crate::math::round(v), 0.0, 255.0) as usize;
        s.push(H[v >> 4] as char);
        s.push(H[v & 15] as char);
    };
    push(c.r);
    push(c.g);
    push(c.b);
    if c.a < 1.0 {
        push(c.a * 255.0);
    }
    s
}

static NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff), ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff), ("beige", 0xf5f5dc), ("bisque", 0xffe4c4), ("black", 0x000000),
    ("blanchedalmond", 0xffebcd), ("blue", 0x0000ff), ("blueviolet", 0x8a2be2), ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887), ("cadetblue", 0x5f9ea0), ("chartreuse", 0x7fff00), ("chocolate", 0xd2691e),
    ("coral", 0xff7f50), ("cornflowerblue", 0x6495ed), ("cornsilk", 0xfff8dc), ("crimson", 0xdc143c),
    ("cyan", 0x00ffff), ("darkblue", 0x00008b), ("darkcyan", 0x008b8b), ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9), ("darkgreen", 0x006400), ("darkgrey", 0xa9a9a9), ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b), ("darkolivegreen", 0x556b2f), ("darkorange", 0xff8c00), ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000), ("darksalmon", 0xe9967a), ("darkseagreen", 0x8fbc8f), ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f), ("darkslategrey", 0x2f4f4f), ("darkturquoise", 0x00ced1), ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493), ("deepskyblue", 0x00bfff), ("dimgray", 0x696969), ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff), ("firebrick", 0xb22222), ("floralwhite", 0xfffaf0), ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff), ("gainsboro", 0xdcdcdc), ("ghostwhite", 0xf8f8ff), ("gold", 0xffd700),
    ("goldenrod", 0xdaa520), ("gray", 0x808080), ("green", 0x008000), ("greenyellow", 0xadff2f),
    ("grey", 0x808080), ("honeydew", 0xf0fff0), ("hotpink", 0xff69b4), ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082), ("ivory", 0xfffff0), ("khaki", 0xf0e68c), ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5), ("lawngreen", 0x7cfc00), ("lemonchiffon", 0xfffacd), ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080), ("lightcyan", 0xe0ffff), ("lightgoldenrodyellow", 0xfafad2), ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90), ("lightgrey", 0xd3d3d3), ("lightpink", 0xffb6c1), ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa), ("lightskyblue", 0x87cefa), ("lightslategray", 0x778899), ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de), ("lightyellow", 0xffffe0), ("lime", 0x00ff00), ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6), ("magenta", 0xff00ff), ("maroon", 0x800000), ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd), ("mediumorchid", 0xba55d3), ("mediumpurple", 0x9370db), ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee), ("mediumspringgreen", 0x00fa9a), ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585), ("midnightblue", 0x191970), ("mintcream", 0xf5fffa), ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5), ("navajowhite", 0xffdead), ("navy", 0x000080), ("oldlace", 0xfdf5e6),
    ("olive", 0x808000), ("olivedrab", 0x6b8e23), ("orange", 0xffa500), ("orangered", 0xff4500),
    ("orchid", 0xda70d6), ("palegoldenrod", 0xeee8aa), ("palegreen", 0x98fb98), ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093), ("papayawhip", 0xffefd5), ("peachpuff", 0xffdab9), ("peru", 0xcd853f),
    ("pink", 0xffc0cb), ("plum", 0xdda0dd), ("powderblue", 0xb0e0e6), ("purple", 0x800080),
    ("rebeccapurple", 0x663399), ("red", 0xff0000), ("rosybrown", 0xbc8f8f), ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513), ("salmon", 0xfa8072), ("sandybrown", 0xf4a460), ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee), ("sienna", 0xa0522d), ("silver", 0xc0c0c0), ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd), ("slategray", 0x708090), ("slategrey", 0x708090), ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f), ("steelblue", 0x4682b4), ("tan", 0xd2b48c), ("teal", 0x008080),
    ("thistle", 0xd8bfd8), ("tomato", 0xff6347), ("turquoise", 0x40e0d0), ("violet", 0xee82ee),
    ("wheat", 0xf5deb3), ("white", 0xffffff), ("whitesmoke", 0xf5f5f5), ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

fn named(n: &str) -> Option<Rgba> {
    if n == "transparent" {
        return Some(Rgba::new(0.0, 0.0, 0.0, 0.0));
    }
    let i = NAMED.binary_search_by(|(k, _)| (*k).cmp(n)).ok()?;
    let v = NAMED[i].1;
    Some(Rgba::new(((v >> 16) & 255) as f64, ((v >> 8) & 255) as f64, (v & 255) as f64, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: Rgba, b: Rgba, eps: f64) -> bool {
        (a.r - b.r).abs() < eps && (a.g - b.g).abs() < eps && (a.b - b.b).abs() < eps && (a.a - b.a).abs() < 1e-6
    }
    #[test]
    fn srgb_encode_matches_exact_pow() {
        // Against libm's pow: ~1e-6 of an 8-bit step (was ~3e-5 with the
        // generic fast_pow), across the encode range and out-of-gamut values.
        let exact = |c: f64| if c <= 0.0031308 { c * 12.92 } else { 1.055 * libm::pow(c, 1.0 / 2.4) - 0.055 };
        let mut worst: f64 = 0.0;
        for i in 0..=200_000 {
            let c = i as f64 / 100_000.0;
            worst = worst.max((linear_to_srgb(c) - exact(c)).abs() * 255.0);
            assert_eq!(linear_to_srgb(-c), -linear_to_srgb(c));
        }
        assert!(worst < 2e-6, "{worst}");
        // OKLab interpolation end to end: within 1e-5 of the exact-pow result.
        let a = to_space(Rgba::new(255.0, 30.0, 0.0, 1.0), ColorSpace::Oklab);
        let b = to_space(Rgba::new(0.0, 90.0, 255.0, 0.5), ColorSpace::Oklab);
        for k in 0..=100 {
            let t = k as f64 / 100.0;
            let v: [f64; 4] = core::array::from_fn(|j| a[j] + (b[j] - a[j]) * t);
            let got = from_space(&v, ColorSpace::Oklab);
            let inv = 1.0 / v[3];
            let l = oklab_to_linear(v[0] * inv, v[1] * inv, v[2] * inv);
            for (g, l) in [got.r, got.g, got.b].into_iter().zip(l) {
                assert!((g - exact(l).clamp(0.0, 1.0) * 255.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn named_sorted() {
        for w in NAMED.windows(2) {
            assert!(w[0].0 < w[1].0, "{} !< {}", w[0].0, w[1].0);
        }
    }
    #[test]
    fn parse_forms() {
        let red = Rgba::new(255.0, 0.0, 0.0, 1.0);
        for s in [
            "red", "#f00", "#ff0000", "#FF0000FF", "rgb(255,0,0)", "rgb(255 0 0)", "rgba(255, 0, 0, 1)",
            "rgb(100% 0% 0%)", "hsl(0, 100%, 50%)", "hsl(0deg 100% 50% / 1)", "hwb(0 0% 0%)",
            "oklch(0.62796 0.25768 29.234)", "oklab(0.62796 0.22486 0.12585)", "color(srgb 1 0 0)",
            "lab(54.29 80.8 69.89)", "lch(54.29 106.84 40.85)",
        ] {
            let c = parse(s).unwrap_or_else(|| panic!("{s}"));
            assert!(close(c, red, 1.0), "{s} -> {c:?}");
        }
        assert_eq!(parse("rgb(0 0 0 / 50%)").unwrap().a, 0.5);
        assert_eq!(parse("transparent").unwrap().a, 0.0);
        assert!(parse("notacolor").is_none());
    }
    #[test]
    fn roundtrip_spaces() {
        let c = Rgba::new(12.0, 200.0, 99.0, 0.7);
        for sp in [ColorSpace::Srgb, ColorSpace::LinearSrgb, ColorSpace::Oklab, ColorSpace::Oklch, ColorSpace::Hsl] {
            let v = to_space(c, sp);
            let back = from_space(&v, sp);
            assert!(close(back, c, 1e-3), "{sp:?} {back:?}");
        }
    }
    #[test]
    fn mix_premultiplied() {
        // Mixing towards transparent should not darken the colour.
        let m = mix(Rgba::new(255.0, 0.0, 0.0, 1.0), Rgba::new(0.0, 0.0, 0.0, 0.0), 0.5, ColorSpace::Oklab);
        assert!((m.r - 255.0).abs() < 1e-6 && (m.a - 0.5).abs() < 1e-9);
        // OKLCH: white → blue keeps blue's hue (powerless hue).
        let m = mix(Rgba::new(255.0, 255.0, 255.0, 1.0), Rgba::new(0.0, 0.0, 255.0, 1.0), 0.5, ColorSpace::Oklch);
        assert!(m.b > m.r && m.b > m.g);
        assert_eq!(to_hex(Rgba::new(255.0, 128.0, 0.0, 1.0)), "#ff8000");
        assert_eq!(to_css(Rgba::new(255.0, 128.0, 0.0, 0.5)), "rgba(255,128,0,0.5)");
    }
}
