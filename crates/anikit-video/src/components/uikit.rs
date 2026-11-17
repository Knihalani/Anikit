//! Shared drawing helpers for product UI (generated app screens and device
//! chrome): a UI palette derived from the theme, vector line icons (Lucide
//! geometry on a 24-unit grid), fitted text, avatars, chips and controls.
//!
//! Everything takes pixel coordinates; sizes that are "points" in a design
//! are multiplied by the caller's scale first.

use super::style::*;
use super::text::{draw, Face, TextStyle};
use super::Assets;
use crate::dl::{Op, Paint, Path, Rgba, IDENTITY};
use kurbo::Shape;

// ---------------------------------------------------------------- palette

/// Neutral family the UI greys lean towards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tint {
    /// Pure greys (zinc).
    #[default]
    Neutral,
    /// Warm greys (stone, paper).
    Warm,
    /// Cool greys (slate).
    Cool,
    /// Greys tinted with the accent.
    Accent,
}

impl Tint {
    pub fn parse(s: &str) -> Option<Tint> {
        Some(match s {
            "neutral" | "grey" | "gray" | "zinc" => Tint::Neutral,
            "warm" | "stone" | "paper" => Tint::Warm,
            "cool" | "slate" | "blue" => Tint::Cool,
            "accent" | "brand" | "tinted" => Tint::Accent,
            _ => return None,
        })
    }
}

/// Colours of one app UI (light or dark).
#[derive(Clone, Debug)]
pub struct Pal {
    pub dark: bool,
    /// Main canvas.
    pub bg: Rgba,
    /// Sidebar / grouped background.
    pub side: Rgba,
    /// Cards and panels.
    pub panel: Rgba,
    /// Inputs, segmented tracks, chips.
    pub sunken: Rgba,
    /// Hover / selected row.
    pub hover: Rgba,
    /// Hairlines.
    pub line: Rgba,
    /// Stronger borders (inputs, buttons).
    pub line2: Rgba,
    pub text: Rgba,
    pub text2: Rgba,
    pub muted: Rgba,
    pub faint: Rgba,
    pub acc: Rgba,
    /// Accent readable as text on `bg`.
    pub acc_text: Rgba,
    /// Accent wash for selected states.
    pub acc_soft: Rgba,
    /// Text on an accent fill.
    pub on_acc: Rgba,
    pub pos: Rgba,
    pub neg: Rgba,
    pub warn: Rgba,
    /// Card elevation (shadow alpha).
    pub elev: f32,
}

fn hx(s: &str) -> Rgba {
    parse_color(s).unwrap_or(Rgba([1.0, 0.0, 1.0, 1.0]))
}

impl Pal {
    pub fn new(dark: bool, accent: Rgba, tint: Tint) -> Pal {
        let tc = match tint {
            Tint::Neutral => None,
            Tint::Warm => Some((hx("#b48a52"), if dark { 0.07 } else { 0.10 })),
            Tint::Cool => Some((hx("#4f7ad0"), if dark { 0.09 } else { 0.07 })),
            Tint::Accent => Some((accent, if dark { 0.08 } else { 0.06 })),
        };
        let t = |c: &str| -> Rgba {
            let c = hx(c);
            match tc {
                Some((k, a)) => {
                    // Tint by luminance-preserving multiply towards the hue.
                    let l = luminance(c).max(1e-4);
                    let kl = luminance(k).max(1e-4);
                    let mut o = [0.0; 4];
                    for i in 0..3 {
                        let target = (k.0[i] * (l / kl).powf(1.0 / 2.2)).clamp(0.0, 1.0);
                        o[i] = c.0[i] + (target - c.0[i]) * a;
                    }
                    o[3] = c.0[3];
                    Rgba(o)
                }
                None => c,
            }
        };
        let acc = accent;
        let lum = luminance(acc);
        let (pos, neg, warn) = if dark { (hx("#3fb950"), hx("#f0605a"), hx("#e3a53a")) } else { (hx("#1a7f37"), hx("#cf222e"), hx("#b7791f")) };
        if dark {
            let bg = t("#0f1012");
            Pal {
                dark,
                bg,
                side: t("#0b0c0e"),
                panel: t("#16171a"),
                sunken: t("#1d1e22"),
                hover: Rgba([1.0, 1.0, 1.0, 0.055]),
                line: Rgba([1.0, 1.0, 1.0, 0.075]),
                line2: Rgba([1.0, 1.0, 1.0, 0.12]),
                text: t("#f4f5f7"),
                text2: t("#c9ccd3"),
                muted: t("#8b8f98"),
                faint: t("#5e626b"),
                acc,
                acc_text: if lum < 0.18 { shade(acc, 0.35) } else { acc },
                acc_soft: alpha(acc, 0.16),
                on_acc: on_color(&[acc]),
                pos,
                neg,
                warn,
                elev: 0.45,
            }
        } else {
            let bg = t("#ffffff");
            Pal {
                dark,
                bg,
                side: t("#f7f7f8"),
                panel: t("#ffffff"),
                sunken: t("#f2f2f4"),
                hover: Rgba([0.0, 0.0, 0.05, 0.045]),
                line: Rgba([0.0, 0.0, 0.08, 0.075]),
                line2: Rgba([0.0, 0.0, 0.08, 0.14]),
                text: t("#17181c"),
                text2: t("#3c3f46"),
                muted: t("#6e727c"),
                faint: t("#a3a6ae"),
                acc,
                acc_text: if lum > 0.35 { shade(acc, -0.45) } else { acc },
                acc_soft: alpha(acc, 0.10),
                on_acc: on_color(&[acc]),
                pos,
                neg,
                warn,
                elev: 0.08,
            }
        }
    }
}

/// Tasteful categorical hues for avatars, tags and file icons (not the
/// theme's launch-video palette: these sit inside a product UI).
pub const HUES: [&str; 8] = ["#e5603e", "#3e7be0", "#2f9e6e", "#d4a12a", "#b0569e", "#1e9fb4", "#7f6ad6", "#d2556b"];

pub fn hue(i: usize) -> Rgba {
    hx(HUES[i % HUES.len()])
}

/// Stable hue for a name.
pub fn hue_for(name: &str) -> Rgba {
    hue(fnv(&[name.as_bytes()]) as usize)
}

// ---------------------------------------------------------------- icons

/// Icon geometry: SVG path data on a 24×24 grid, `|`-separated parts;
/// `c cx cy r` is a circle and `r x y w h rx` a rounded rect.
fn icon_src(name: &str) -> &'static str {
    match name {
        "home" => "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8|M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
        "inbox" => "M22 12h-6l-2 3h-4l-2-3H2|M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z",
        "search" => "c 11 11 7.5|m21 21-4.3-4.3",
        "settings" => "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z|c 12 12 3",
        "chart" => "M3 3v16a2 2 0 0 0 2 2h16|M18 17V9|M13 17V5|M8 17v-3",
        "trending" => "M22 7 13.5 15.5 8.5 10.5 2 17|M16 7h6v6",
        "users" => "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2|c 9 7 4|M22 21v-2a4 4 0 0 0-3-3.87|M16 3.13a4 4 0 0 1 0 7.75",
        "user" => "c 12 8 4|M20 21a8 8 0 0 0-16 0",
        "bell" => "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9|M10.3 21a1.94 1.94 0 0 0 3.4 0",
        "folder" => "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
        "calendar" => "r 3 4 18 18 2|M16 2v4|M8 2v4|M3 10h18",
        "file" => "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z|M14 2v4a2 2 0 0 0 2 2h4|M16 13H8|M16 17H8|M10 9H8",
        "plus" => "M5 12h14|M12 5v14",
        "check" => "M20 6 9 17l-5-5",
        "x" => "M18 6 6 18|m6 6 12 12",
        "star" => "M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z",
        "message" => "M7.9 20A9 9 0 1 0 4 16.1L2 22Z",
        "heart" => "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z",
        "repeat" => "m2 9 3-3 3 3|M13 18H7a2 2 0 0 1-2-2V6|m22 15-3 3-3-3|M11 6h6a2 2 0 0 1 2 2v10",
        "share" => "M12 2v13|m16 6-4-4-4 4|M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8",
        "bookmark" => "m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z",
        "layers" => "M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z|m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65|m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65",
        "zap" => "M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z",
        "lock" => "r 4 11 16 10 2|M8 11V7a4 4 0 0 1 8 0v4",
        "globe" => "c 12 12 10|M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20|M2 12h20",
        "moon" => "M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z",
        "sun" => "c 12 12 4|M12 2v2|M12 20v2|m4.93 4.93 1.41 1.41|m17.66 17.66 1.41 1.41|M2 12h2|M20 12h2|m6.34 17.66-1.41 1.41|m19.07 4.93-1.41 1.41",
        "chevron-right" => "m9 18 6-6-6-6",
        "chevron-left" => "m15 18-6-6 6-6",
        "chevron-down" => "m6 9 6 6 6-6",
        "chevrons-updown" => "m7 15 5 5 5-5|m7 9 5-5 5 5",
        "arrow-up" => "m5 12 7-7 7 7|M12 19V5",
        "arrow-up-right" => "M7 7h10v10|M7 17 17 7",
        "send" => "M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z|m21.854 2.147-10.94 10.939",
        "paperclip" => "m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48",
        "filter" => "M3 6h18|M7 12h10|M10 18h4",
        "sliders" => "M21 4h-7|M10 4H3|M21 12h-9|M8 12H3|M21 20h-5|M12 20H3|M14 2v4|M8 10v4|M16 18v4",
        "sparkles" => "M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z",
        "code" => "m16 18 6-6-6-6|m8 6-6 6 6 6",
        "grid" => "r 3 3 7 7 1.5|r 14 3 7 7 1.5|r 14 14 7 7 1.5|r 3 14 7 7 1.5",
        "image" => "r 3 3 18 18 2|c 9 9 2|m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21",
        "credit-card" => "r 2 5 20 14 2|M2 10h20",
        "shield" => "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z",
        "edit" => "M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7|M18.375 2.625a1 1 0 0 1 3 3l-9.013 9.014a2 2 0 0 1-.853.505l-2.873.84a.5.5 0 0 1-.62-.62l.84-2.873a2 2 0 0 1 .506-.852z",
        "sidebar" => "r 3 3 18 18 2|M9 3v18",
        "reload" => "M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8|M21 3v5h-5",
        "copy" => "r 8 8 14 14 2|M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2",
        "smile" => "c 12 12 10|M8 14s1.5 2 4 2 4-2 4-2|M9 9h.01|M15 9h.01",
        "hash" => "M4 9h16|M4 15h16|M10 3 8 21|M16 3l-2 18",
        "branch" => "M6 3v12|c 18 6 3|c 6 18 3|M18 9a9 9 0 0 1-9 9",
        "terminal" => "m4 17 6-6-6-6|M12 19h8",
        "mic" => "M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z|M19 10v2a7 7 0 0 1-14 0v-2|M12 19v3",
        "video" => "m16 13 5.223 3.482a.5.5 0 0 0 .777-.416V7.87a.5.5 0 0 0-.752-.432L16 10.5|r 2 6 14 12 2",
        "phone" => "M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7A2 2 0 0 1 22 16.92z",
        "download" => "M12 15V3|M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4|m7 10 5 5 5-5",
        "palette" => "M12 22a1 1 0 0 1 0-20 10 9 0 0 1 10 9 5 5 0 0 1-5 5h-2.25a1.75 1.75 0 0 0-1.4 2.8l.3.4a1.75 1.75 0 0 1-1.4 2.8z|c 13.5 6.5 .5|c 17.5 10.5 .5|c 6.5 12.5 .5|c 8.5 7.5 .5",
        "key" => "m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4|m21 2-9.6 9.6|c 7.5 15.5 5.5",
        "help" => "c 12 12 10|M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3|M12 17h.01",
        "clock" => "c 12 12 10|M12 6v6l4 2",
        "tag" => "M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z|c 7.5 7.5 .5",
        "target" => "c 12 12 10|c 12 12 6|c 12 12 2",
        "box" => "M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z|m3.3 7 8.7 5 8.7-5|M12 22V12",
        "wand" => "m21.64 3.64-1.28-1.28a1.21 1.21 0 0 0-1.72 0L2.36 18.64a1.21 1.21 0 0 0 0 1.72l1.28 1.28a1.2 1.2 0 0 0 1.72 0L21.64 5.36a1.2 1.2 0 0 0 0-1.72|m14 7 3 3|M5 6v4|M19 14v4|M10 2v2|M7 8H3|M21 16h-4|M11 3H9",
        "more" => "c 5 12 1|c 12 12 1|c 19 12 1",
        _ => "c 12 12 9",
    }
}

/// Icon names that have a drawing (for docs and fallbacks).
pub const ICONS: &[&str] = &[
    "home", "inbox", "search", "settings", "chart", "trending", "users", "user", "bell", "folder", "calendar", "file", "plus", "check", "x", "star", "message", "heart", "repeat", "share", "bookmark", "layers", "zap", "lock", "globe", "moon", "sun", "code", "grid", "image", "credit-card", "shield", "edit", "sparkles", "palette", "key", "help", "clock", "tag", "target", "box", "wand",
];

/// Icon path in pixels: the 24-grid scaled to `size` with its top-left at `(x, y)`.
pub fn icon_path(name: &str, x: f32, y: f32, size: f32) -> Path {
    let k = size as f64 / 24.0;
    let mut out = Path::new();
    for part in icon_src(name).split('|') {
        let part = part.trim();
        let nums = |s: &str| -> Vec<f64> { s.split_whitespace().filter_map(|v| v.parse().ok()).collect() };
        let p: Path = if let Some(rest) = part.strip_prefix("c ") {
            let v = nums(rest);
            kurbo::Circle::new((v[0], v[1]), v[2]).to_path(0.02)
        } else if let Some(rest) = part.strip_prefix("r ") {
            let v = nums(rest);
            kurbo::RoundedRect::new(v[0], v[1], v[0] + v[2], v[1] + v[3], v[4]).to_path(0.02)
        } else {
            Path::from_svg(part).unwrap_or_default()
        };
        for el in p.elements() {
            out.push(*el);
        }
    }
    out.apply_affine(kurbo::Affine::new([k, 0.0, 0.0, k, x as f64, y as f64]));
    out
}

/// Line icon `name`, `size` px square at `(x, y)` (top-left), stroke `weight`
/// in grid units (Lucide uses 2; 1.6–1.8 reads finer at UI sizes).
pub fn icon(o: &mut Vec<Op>, name: &str, x: f32, y: f32, size: f32, c: Rgba, weight: f32) {
    if size <= 0.5 || c.0[3] <= 0.0 {
        return;
    }
    let path = icon_path(name, x, y, size);
    if name == "more" {
        o.push(fill(path, solid(c)));
        return;
    }
    o.push(Op::Stroke { path, paint: solid(c), stroke: line(weight * size / 24.0, [0.0, 1.0]), transform: IDENTITY });
}

/// Filled (selected-state) icon.
pub fn icon_filled(o: &mut Vec<Op>, name: &str, x: f32, y: f32, size: f32, c: Rgba) {
    let path = icon_path(name, x, y, size);
    o.push(fill(path.clone(), solid(c)));
    o.push(Op::Stroke { path, paint: solid(c), stroke: line(1.6 * size / 24.0, [0.0, 1.0]), transform: IDENTITY });
}

// ---------------------------------------------------------------- text

/// Text style shorthand.
pub fn ts(face: Face, size: f32, weight: u16, c: Rgba) -> TextStyle {
    let mut st = TextStyle::new(face, size, weight, c);
    // Inter wants slightly negative tracking at display sizes.
    if size >= 20.0 && face != Face::Mono {
        st.tracking = -0.012 * ((size - 20.0) / 20.0).clamp(0.0, 1.0) - 0.004;
    }
    st
}

/// Draw `text` no wider than `maxw`: shrinks up to 12 %, then ellipsizes.
/// Returns the drawn width.
pub fn fit(o: &mut Vec<Op>, a: &dyn Assets, text: &str, st: &TextStyle, x: f32, y: f32, maxw: f32, align: f32) -> f32 {
    let w = a.measure(text, st);
    if w <= maxw || maxw <= 0.0 {
        draw(o, a, text, st, x, y, align);
        return w;
    }
    let mut st = st.clone();
    st.size = (st.size * maxw / w).max(st.size * 0.88);
    let w2 = a.measure(text, &st);
    if w2 <= maxw {
        draw(o, a, text, &st, x, y, align);
        return w2;
    }
    let cut = ellipsize(a, text, &st, maxw);
    draw(o, a, &cut, &st, x, y, align);
    a.measure(&cut, &st)
}

/// Longest prefix of `text` (at a char boundary) that fits `maxw` with "…".
pub fn ellipsize(a: &dyn Assets, text: &str, st: &TextStyle, maxw: f32) -> String {
    if a.measure(text, st) <= maxw {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand: String = chars[..mid].iter().collect::<String>().trim_end().to_string() + "…";
        if a.measure(&cand, st) <= maxw {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>().trim_end().trim_end_matches([',', '.', ':', ';', '-']).to_string() + "…"
}

/// Wrap into at most `max_lines`, ellipsizing the last one. Returns lines.
pub fn wrap_lines(a: &dyn Assets, text: &str, st: &TextStyle, maxw: f32, max_lines: usize) -> Vec<String> {
    let mut lines = super::text::wrap(a, text, st, maxw);
    if lines.len() > max_lines {
        let rest = lines[max_lines - 1..].join(" ");
        lines.truncate(max_lines - 1);
        lines.push(ellipsize(a, &rest, st, maxw));
    } else if let Some(l) = lines.last_mut() {
        if a.measure(l, st) > maxw {
            *l = ellipsize(a, l, st, maxw);
        }
    }
    lines
}

/// Baseline that vertically centres a single line of `size` on `cy`.
#[inline]
pub fn mid(cy: f32, size: f32) -> f32 {
    cy + size * 0.355
}

/// Initials of a name ("Ada Lovelace" → "AL", "northwind" → "N").
pub fn initials(name: &str) -> String {
    let mut s: String = name.split_whitespace().filter_map(|w| w.chars().find(|c| c.is_alphanumeric())).take(2).collect();
    if s.is_empty() {
        s.push('?');
    }
    s.to_uppercase()
}

// ---------------------------------------------------------------- pieces

/// Circular avatar with initials on a soft two-tone gradient.
pub fn avatar(o: &mut Vec<Op>, a: &dyn Assets, name: &str, cx: f32, cy: f32, r: f32, ring: Option<Rgba>) {
    let c = hue_for(name);
    if let Some(rc) = ring {
        o.push(circle(cx, cy, r + (r * 0.12).max(1.0), solid(rc)));
    }
    o.push(circle(cx, cy, r, Paint::Linear { p0: [cx - r, cy - r], p1: [cx + r, cy + r], stops: vec![(0.0, shade(c, 0.22)), (1.0, shade(c, -0.18))] }));
    let s = initials(name);
    let st = ts(Face::Sans, r * if s.chars().count() > 1 { 0.78 } else { 0.92 }, 600, Rgba([1.0, 1.0, 1.0, 0.96]));
    draw(o, a, &s, &st, cx, mid(cy, st.size), 0.5);
}

/// Product logo tile: rounded square in the accent with the name's initial.
pub fn logo(o: &mut Vec<Op>, a: &dyn Assets, name: &str, acc: Rgba, x: f32, y: f32, s: f32) {
    o.push(rect(x, y, s, s, s * 0.26, Paint::Linear { p0: [x, y], p1: [x + s, y + s], stops: vec![(0.0, shade(acc, 0.12)), (1.0, shade(acc, -0.16))] }));
    o.push(Op::Stroke { path: rrect_path(x + 0.5, y + 0.5, s - 1.0, s - 1.0, s * 0.26), paint: Paint::Linear { p0: [x, y], p1: [x, y + s], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.35])), (0.5, Rgba([1.0, 1.0, 1.0, 0.0]))] }, stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
    let ch: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "A".into());
    let st = ts(Face::Display, s * 0.56, 700, on_color(&[acc]));
    draw(o, a, &ch, &st, x + s * 0.5, mid(y + s * 0.5, st.size), 0.5);
}

/// A card: optional shadow, fill and hairline.
pub fn card(o: &mut Vec<Op>, p: &Pal, x: f32, y: f32, w: f32, h: f32, r: f32, elevated: bool) {
    if elevated && p.elev > 0.0 {
        shadow(o, x, y, w, h, r, r * 0.12 + 1.0, r * 0.6 + 4.0, p.elev);
    }
    o.push(rect(x, y, w, h, r, solid(p.panel)));
    border(o, x, y, w, h, r, p.line);
}

/// Small pill label (`fg` text on a wash of it).
pub fn chip(o: &mut Vec<Op>, a: &dyn Assets, label: &str, fg: Rgba, x: f32, cy: f32, size: f32, dot: bool) -> f32 {
    let st = ts(Face::Sans, size, 500, fg);
    let tw = a.measure(label, &st);
    let h = size * 1.75;
    let pad = size * 0.6;
    let dw = if dot { size * 0.9 } else { 0.0 };
    let w = tw + pad * 2.0 + dw;
    o.push(rect(x, cy - h * 0.5, w, h, h * 0.5, solid(alpha(fg, 0.12))));
    if dot {
        o.push(circle(x + pad + size * 0.22, cy, size * 0.22, solid(fg)));
    }
    draw(o, a, label, &st, x + pad + dw, mid(cy, size), 0.0);
    w
}

/// Outlined label chip with a coloured dot (Linear-style labels).
pub fn label_chip(o: &mut Vec<Op>, a: &dyn Assets, p: &Pal, label: &str, dotc: Rgba, x: f32, cy: f32, size: f32, align_right: bool) -> f32 {
    let st = ts(Face::Sans, size, 500, p.text2);
    let tw = a.measure(label, &st);
    let h = size * 1.8;
    let w = tw + size * 2.0;
    let x = if align_right { x - w } else { x };
    o.push(rect(x, cy - h * 0.5, w, h, h * 0.5, solid(p.panel)));
    border(o, x, cy - h * 0.5, w, h, h * 0.5, p.line2);
    o.push(circle(x + size * 0.75, cy, size * 0.25, solid(dotc)));
    draw(o, a, label, &st, x + size * 1.25, mid(cy, size), 0.0);
    w
}

/// Keyboard hint ("⌘K").
pub fn kbd(o: &mut Vec<Op>, a: &dyn Assets, p: &Pal, label: &str, right: f32, cy: f32, size: f32) -> f32 {
    let st = ts(Face::Sans, size, 500, p.muted);
    let tw = a.measure(label, &st);
    let (w, h) = (tw + size * 0.9, size * 1.55);
    o.push(rect(right - w, cy - h * 0.5, w, h, size * 0.3, solid(p.sunken)));
    border(o, right - w, cy - h * 0.5, w, h, size * 0.3, p.line);
    draw(o, a, label, &st, right - w * 0.5, mid(cy, size), 0.5);
    w
}

/// Toggle switch, `k` 0 (off) .. 1 (on); `h` is its height.
pub fn toggle(o: &mut Vec<Op>, p: &Pal, x: f32, cy: f32, h: f32, k: f32, ios: bool) {
    let w = h * if ios { 1.62 } else { 1.75 };
    let off = if p.dark { Rgba([1.0, 1.0, 1.0, 0.16]) } else { Rgba([0.0, 0.0, 0.08, 0.14]) };
    o.push(rect(x, cy - h * 0.5, w, h, h * 0.5, solid(mix(off, if ios { p.pos } else { p.acc }, k))));
    let inset = h * 0.09;
    let d = h - inset * 2.0;
    let kx = x + inset + d * 0.5 + (w - d - inset * 2.0) * k;
    shadow(o, kx - d * 0.5, cy - d * 0.5, d, d, d * 0.5, h * 0.06, h * 0.12, 0.25);
    o.push(circle(kx, cy, d * 0.5, solid(Rgba([1.0, 1.0, 1.0, 1.0]))));
}

/// Button: `primary` fills with the accent, else a bordered neutral button.
/// Returns its width. `icon_name` is drawn before the label.
pub fn button(o: &mut Vec<Op>, a: &dyn Assets, p: &Pal, label: &str, icon_name: Option<&str>, x: f32, y: f32, h: f32, primary: bool, radius: f32, align_right: bool) -> f32 {
    let size = h * 0.42;
    let st = ts(Face::Sans, size, 500, if primary { p.on_acc } else { p.text });
    let tw = if label.is_empty() { 0.0 } else { a.measure(label, &st) };
    let isz = size * 1.15;
    let gap = if label.is_empty() || icon_name.is_none() { 0.0 } else { size * 0.45 };
    let pad = if label.is_empty() { (h - isz) * 0.5 } else { size * 0.95 };
    let w = pad * 2.0 + tw + if icon_name.is_some() { isz + gap } else { 0.0 };
    let x = if align_right { x - w } else { x };
    if primary {
        o.push(rect(x, y, w, h, radius, Paint::Linear { p0: [x, y], p1: [x, y + h], stops: vec![(0.0, shade(p.acc, 0.06)), (1.0, shade(p.acc, -0.05))] }));
        o.push(Op::Stroke { path: rrect_path(x + 0.5, y + 0.5, w - 1.0, h - 1.0, radius), paint: Paint::Linear { p0: [x, y], p1: [x, y + h], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.22])), (0.45, Rgba([1.0, 1.0, 1.0, 0.0]))] }, stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
    } else {
        o.push(rect(x, y, w, h, radius, solid(if p.dark { p.sunken } else { p.panel })));
        border(o, x, y, w, h, radius, p.line2);
    }
    let mut cx = x + pad;
    if let Some(n) = icon_name {
        icon(o, n, cx, y + (h - isz) * 0.5, isz, if primary { p.on_acc } else { p.text2 }, 1.9);
        cx += isz + gap;
    }
    if !label.is_empty() {
        draw(o, a, label, &st, cx, mid(y + h * 0.5, size), 0.0);
    }
    w
}

/// Segmented control; returns its width. `sel` may be fractional (glides).
pub fn segmented(o: &mut Vec<Op>, a: &dyn Assets, p: &Pal, labels: &[&str], sel: f32, x: f32, y: f32, h: f32, align_right: bool) -> f32 {
    let size = h * 0.44;
    let st = ts(Face::Sans, size, 500, p.text);
    let pad = size * 0.9;
    let ws: Vec<f32> = labels.iter().map(|l| a.measure(l, &st) + pad * 2.0).collect();
    let w: f32 = ws.iter().sum::<f32>() + h * 0.12 * 2.0;
    let x = if align_right { x - w } else { x };
    let r = h * 0.26;
    o.push(rect(x, y, w, h, r, solid(p.sunken)));
    let inset = h * 0.12;
    // Selected thumb, interpolated between segments.
    let i0 = (sel.floor() as usize).min(labels.len() - 1);
    let i1 = (i0 + 1).min(labels.len() - 1);
    let f = sel - i0 as f32;
    let start = |i: usize| x + inset + ws[..i].iter().sum::<f32>();
    let tx = lerp(start(i0), start(i1), f);
    let tw = lerp(ws[i0], ws[i1], f);
    shadow(o, tx, y + inset, tw, h - inset * 2.0, r * 0.8, 1.0, 3.0, if p.dark { 0.4 } else { 0.12 });
    o.push(rect(tx, y + inset, tw, h - inset * 2.0, r * 0.8, solid(if p.dark { shade(p.sunken, 0.09) } else { p.panel })));
    for (i, l) in labels.iter().enumerate() {
        let on = (i as f32 - sel).abs() < 0.5;
        draw(o, a, l, &st.clone().color(if on { p.text } else { p.muted }), start(i) + ws[i] * 0.5, mid(y + h * 0.5, size), 0.5);
    }
    w
}

/// Segmented control stretched to `w` (segments share the width equally).
pub fn segmented_w(o: &mut Vec<Op>, a: &dyn Assets, p: &Pal, labels: &[&str], sel: f32, x: f32, y: f32, w: f32, h: f32) {
    let size = h * 0.44;
    let st = ts(Face::Sans, size, 500, p.text);
    let r = h * 0.26;
    o.push(rect(x, y, w, h, r, solid(p.sunken)));
    let inset = h * 0.1;
    let n = labels.len().max(1) as f32;
    let sw = (w - inset * 2.0) / n;
    let tx = x + inset + sw * sel.clamp(0.0, n - 1.0);
    shadow(o, tx, y + inset, sw, h - inset * 2.0, r * 0.8, 1.0, 3.0, if p.dark { 0.4 } else { 0.12 });
    o.push(rect(tx, y + inset, sw, h - inset * 2.0, r * 0.8, solid(if p.dark { shade(p.sunken, 0.09) } else { p.panel })));
    for (i, l) in labels.iter().enumerate() {
        let on = (i as f32 - sel).abs() < 0.5;
        if i > 0 && !on && ((i as f32 - 1.0) - sel).abs() >= 0.5 {
            o.push(rect(x + inset + sw * i as f32, y + h * 0.3, 1.0, h * 0.4, 0.0, solid(p.line2)));
        }
        draw(o, a, l, &st.clone().color(if on { p.text } else { p.muted }), x + inset + sw * (i as f32 + 0.5), mid(y + h * 0.5, size), 0.5);
    }
}

/// Search / input field with an icon and placeholder; `kbd_hint` on the right.
pub fn field(o: &mut Vec<Op>, a: &dyn Assets, p: &Pal, placeholder: &str, icon_name: Option<&str>, x: f32, y: f32, w: f32, h: f32, r: f32, kbd_hint: Option<&str>) {
    o.push(rect(x, y, w, h, r, solid(p.sunken)));
    border(o, x, y, w, h, r, p.line);
    let size = h * 0.42;
    let mut tx = x + h * 0.32;
    if let Some(n) = icon_name {
        let isz = size * 1.15;
        icon(o, n, tx, y + (h - isz) * 0.5, isz, p.muted, 2.0);
        tx += isz + size * 0.5;
    }
    let right = match kbd_hint {
        Some(k) => x + w - h * 0.22 - kbd(o, a, p, k, x + w - h * 0.22, y + h * 0.5, size * 0.85) - size * 0.4,
        None => x + w - size,
    };
    fit(o, a, placeholder, &ts(Face::Sans, size, 400, p.muted), tx, mid(y + h * 0.5, size), right - tx, 0.0);
}

/// Linear-style issue status glyph: 0 backlog (dashed), 1 todo, 2 in progress, 3 in review, 4 done.
pub fn status(o: &mut Vec<Op>, p: &Pal, kind: u8, cx: f32, cy: f32, r: f32) {
    let w = (r * 0.2).max(1.0);
    match kind {
        0 => o.push(Op::Stroke { path: circle_path(cx, cy, r - w * 0.5), paint: solid(p.faint), stroke: crate::dl::Stroke { width: w, cap: 0, join: 1, miter_limit: 4.0, dash: vec![r * 0.42, r * 0.36], dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY }),
        1 => o.push(stroke_op(circle_path(cx, cy, r - w * 0.5), solid(p.muted), w, [0.0, 1.0])),
        2 | 3 => {
            let c = if kind == 2 { p.warn } else { p.pos };
            o.push(stroke_op(circle_path(cx, cy, r - w * 0.5), solid(c), w, [0.0, 1.0]));
            let frac = if kind == 2 { 0.5 } else { 0.75 };
            let mut pie = Path::new();
            pie.move_to((cx as f64, cy as f64));
            let rr = (r - w * 1.9) as f64;
            let n = 24;
            for i in 0..=n {
                let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * frac * i as f64 / n as f64;
                pie.line_to((cx as f64 + rr * a.cos(), cy as f64 + rr * a.sin()));
            }
            pie.close_path();
            o.push(fill(pie, solid(c)));
        }
        _ => {
            o.push(circle(cx, cy, r, solid(p.acc)));
            let mut ck = Path::new();
            ck.move_to(((cx - r * 0.42) as f64, (cy + r * 0.02) as f64));
            ck.line_to(((cx - r * 0.1) as f64, (cy + r * 0.32) as f64));
            ck.line_to(((cx + r * 0.45) as f64, (cy - r * 0.3) as f64));
            o.push(stroke_op(ck, solid(p.on_acc), r * 0.24, [0.0, 1.0]));
        }
    }
}

/// Small sparkline in `(x, y, w, h)`.
pub fn sparkline(o: &mut Vec<Op>, data: &[f32], x: f32, y: f32, w: f32, h: f32, c: Rgba, width: f32, k: f32) {
    if data.len() < 2 {
        return;
    }
    let (lo, hi) = data.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let span = (hi - lo).max(1e-3);
    let pts: Vec<(f32, f32)> = data.iter().enumerate().map(|(i, v)| (x + w * i as f32 / (data.len() - 1) as f32, y + h - h * (v - lo) / span)).collect();
    let path = smooth_path(&pts, 1.0);
    let mut area = path.clone();
    area.line_to(((x + w) as f64, (y + h) as f64));
    area.line_to((x as f64, (y + h) as f64));
    area.close_path();
    if k > 0.0 {
        o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((rrect_path(x - width, y - width * 2.0, (w + width * 2.0) * k, h + width * 4.0, 0.0), IDENTITY)), cache_key: None });
        o.push(fill(area, vgrad(y, y + h, alpha(c, 0.18), alpha(c, 0.0))));
        o.push(stroke_op(path, solid(c), width, [0.0, 1.0]));
        o.push(Op::PopLayer);
    }
}
