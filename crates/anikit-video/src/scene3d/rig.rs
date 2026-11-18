//! Lighting rigs: named, camera-relative setups of key, fill, rim, kickers,
//! strips and flags, the way a lighting designer would place them on set.
//!
//! A rig replaces the environment's softboxes (its sky gradient stays as
//! ambient) and is placed around the camera's view at t = 0: angle 0 is the
//! camera's side of the subject, +90 its left, 180 behind the subject.
//! Intensities are set as illuminance *ratios*, so `"ratio": 4` means the key
//! side is four times brighter than the fill side, as on a light meter.
//!
//! `"rig": "rembrandt"` or `{ "preset", "ratio", "rim", "key", "angle",
//! "height", "warmth", "gobo", "goboScale", "softness", "beam" }`.

use super::env::{Env, Softbox};
use super::math::*;
use super::spec::{check_keys, f, Res};
use serde_json::{Map, Value};

pub const RIGS: &[&str] = &["rembrandt", "clamshell", "product", "high-key", "low-key", "split", "rim", "golden", "window", "top", "butterfly", "loop", "pinspot", "stage"];

/// A gobo (cookie) in front of the key: the shadow pattern of a window,
/// blinds or foliage, projected along the key direction.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gobo {
    /// 0 none, 1 venetian blinds, 2 window panes, 3 leaves, 4 a text card ([`TextGobo`]).
    pub kind: u8,
    /// Text card: its index in the registry (see [`text_gobo`]).
    pub text: u32,
    /// The frame time (text cards can slide in).
    pub t: f32,
    /// Pattern period in metres.
    pub scale: f32,
    /// Edge softness (0..1).
    pub soft: f32,
    /// Light direction (towards the light) and its tangent axes.
    pub dir: V3,
    pub u: V3,
    pub v: V3,
}

impl Gobo {
    /// Light let through at `p` (0..1).
    #[inline]
    pub fn at(&self, p: V3) -> f32 {
        if self.kind == 0 {
            return 1.0;
        }
        if self.kind == 4 {
            return text_gobo_at(self.text, p, self.dir, self.t);
        }
        let (a, b) = (dot(p, self.u) / self.scale, dot(p, self.v) / self.scale);
        let s = self.soft.max(0.01);
        match self.kind {
            1 => {
                // Slats: 60% open.
                let y = fract(b);
                smoothstep(0.4 - s * 0.5, 0.4 + s * 0.5, y)
            }
            2 => {
                // Four panes per period with mullions 12% wide, and the window edge.
                let (x, y) = (fract(a * 2.0), fract(b * 2.0));
                let m = |t: f32| smoothstep(0.06 - s * 0.2, 0.06 + s * 0.2, t.min(1.0 - t));
                let edge = smoothstep(1.0 + s, 1.0 - s, a.abs().max(b.abs()));
                m(x) * m(y) * edge
            }
            _ => {
                // Dappled foliage: leaf clusters with small sun-flecks between them.
                let n = vn2(a * 5.0, b * 5.0) * 0.5 + vn2(a * 11.0 + 11.0, b * 11.0) * 0.3 + vn2(a * 23.0 + 5.0, b * 23.0) * 0.2;
                // Mostly light, with soft clusters of shade.
                smoothstep(0.36 - s * 0.35, 0.36 + s * 0.35, n)
            }
        }
    }
}

/// A word cut out of a card held above the table: the key light prints it
/// (as shadow, or as light through the cut-out with `invert`) on everything
/// below, projected along the light. Defined by where the word lands on the
/// plane `y = 0`, so it reads straight down the camera's z axis.
pub struct TextGobo {
    /// Coverage mask (1 = letter), row-major, `w × h`, rows going +z.
    mask: Vec<f32>,
    w: usize,
    h: usize,
    /// Footprint on y = 0: centre (x, z) and size in metres.
    cx: f32,
    cz: f32,
    sw: f32,
    sh: f32,
    /// Height of the card above y = 0 (sets how far objects' patterns shift).
    card: f32,
    /// Shadow darkness (1 = letters fully block the key), and inverted (light through letters).
    strength: f32,
    invert: bool,
    /// Slide-in: offset (x, z) in metres at `t0`, settling to 0 by `t0 + dur` (out-cubic).
    from: (f32, f32),
    t0: f32,
    dur: f32,
}

fn gobos() -> &'static std::sync::RwLock<Vec<std::sync::Arc<TextGobo>>> {
    static G: std::sync::OnceLock<std::sync::RwLock<Vec<std::sync::Arc<TextGobo>>>> = std::sync::OnceLock::new();
    G.get_or_init(Default::default)
}

fn text_gobo_at(id: u32, p: V3, dir: V3, t: f32) -> f32 {
    let g = match gobos().read().unwrap().get(id as usize) {
        Some(g) => g.clone(),
        None => return 1.0,
    };
    if dir.y <= 1e-3 {
        return 1.0;
    }
    // Along the light to the card plane, then back down to y = 0: where this point's light crosses the card.
    let k = (g.card - p.y) / dir.y;
    let (qx, qz) = (p.x + dir.x * k - dir.x * g.card / dir.y, p.z + dir.z * k - dir.z * g.card / dir.y);
    let e = if g.dur > 0.0 { ((t - g.t0) / g.dur).clamp(0.0, 1.0) } else { 1.0 };
    let e = 1.0 - (1.0 - e).powi(3);
    let (ox, oz) = (g.from.0 * (1.0 - e), g.from.1 * (1.0 - e));
    let u = (qx - ox - g.cx) / g.sw + 0.5;
    let v = (qz - oz - g.cz) / g.sh + 0.5;
    let c = if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        0.0
    } else {
        let (x, y) = (u * g.w as f32 - 0.5, v * g.h as f32 - 0.5);
        let (x0, y0) = (x.floor().max(0.0) as usize, y.floor().max(0.0) as usize);
        let (x1, y1) = ((x0 + 1).min(g.w - 1), (y0 + 1).min(g.h - 1));
        let (tx, ty) = ((x - x0 as f32).clamp(0.0, 1.0), (y - y0 as f32).clamp(0.0, 1.0));
        let m = |xx: usize, yy: usize| g.mask[yy * g.w + xx];
        let a = m(x0, y0) + (m(x1, y0) - m(x0, y0)) * tx;
        let b = m(x0, y1) + (m(x1, y1) - m(x0, y1)) * tx;
        a + (b - a) * ty
    };
    if g.invert {
        // Light only through the letters (the rest of the card blocks most of it).
        1.0 - g.strength * (1.0 - c)
    } else {
        1.0 - g.strength * c
    }
}

/// Parse `"gobo": {"text", "font", "weight", "tracking", "size", "at": [x, z], "height", "strength", "invert", "softness", "from": [dx, dz], "start", "settle"}`
/// and register it; returns a [`Gobo`] for the key direction `dir`.
pub fn text_gobo(v: &Value, dir: V3, path: &str) -> Res<Gobo> {
    let m = v.as_object().ok_or_else(|| format!("{path}: expected an object with `text`"))?;
    check_keys(m, &["text", "font", "weight", "tracking", "size", "at", "height", "strength", "invert", "softness", "from", "start", "settle"], path)?;
    let text = m.get("text").and_then(|x| x.as_str()).ok_or_else(|| format!("{path}.text: the word the light prints"))?;
    let font = m.get("font").and_then(|x| x.as_str()).unwrap_or("Instrument Serif");
    let g = |k: &str, d: f32| -> Res<f32> { m.get(k).map(|x| f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
    let pair = |k: &str, d: (f32, f32)| -> Res<(f32, f32)> {
        match m.get(k) {
            None => Ok(d),
            Some(Value::Array(a)) if a.len() == 2 => Ok((f(&a[0], path)?, f(&a[1], path)?)),
            _ => Err(format!("{path}.{k}: expected [x, z] in metres")),
        }
    };
    let size = g("size", 0.06)?.max(1e-3);
    let at = pair("at", (0.0, 0.0))?;
    let letters = crate::components::text3d::glyph_outlines(text, font, g("weight", 500.0)?, g("tracking", 0.0)?);
    // Bounds in em units (y up from the baseline).
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for c in letters.iter().flatten().flatten() {
        x0 = x0.min(c[0]);
        x1 = x1.max(c[0]);
        y0 = y0.min(c[1]);
        y1 = y1.max(c[1]);
    }
    if x1 <= x0 || y1 <= y0 {
        return Err(format!("{path}.text: `{text}` has no outlines in font `{font}`"));
    }
    // `size` is the cap-ish height of the printed word (the outline's ink height).
    let em = y1 - y0;
    let (sw, sh) = (size * (x1 - x0) / em, size);
    let pad = 0.06;
    let (sw, sh) = (sw * (1.0 + 2.0 * pad), sh * (1.0 + 2.0 * pad * (x1 - x0) / em));
    let w = 2400usize;
    let h = ((w as f32 * sh / sw).round() as usize).clamp(8, 3000);
    // Even-odd scanline fill, 4× vertical supersampling.
    let mut mask = vec![0f32; w * h];
    let px = |x: f32| ((x - x0) / (x1 - x0) * (1.0 - 2.0 * pad) + pad) * w as f32;
    let (ey0, ey1) = (y0, y1);
    let ys = (sh - size) / 2.0 / sh;
    let py = |y: f32| (ys + (ey1 - y) / (ey1 - ey0) * (size / sh)) * h as f32;
    let edges: Vec<(f32, f32, f32, f32)> = letters
        .iter()
        .flatten()
        .flat_map(|c| (0..c.len()).map(move |i| (c[i], c[(i + 1) % c.len()])))
        .map(|(a, b)| (px(a[0]), py(a[1]), px(b[0]), py(b[1])))
        .collect();
    const SS: usize = 4;
    let mut xs = Vec::new();
    for row in 0..h {
        for s in 0..SS {
            let y = row as f32 + (s as f32 + 0.5) / SS as f32;
            xs.clear();
            for &(ax, ay, bx, by) in &edges {
                if (ay <= y && by > y) || (by <= y && ay > y) {
                    xs.push(ax + (y - ay) / (by - ay) * (bx - ax));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let (a, b) = (pair[0].max(0.0), pair[1].min(w as f32));
                let mut x = a;
                while x < b {
                    let xi = x.floor();
                    let next = (xi + 1.0).min(b);
                    let i = xi as usize;
                    if i < w {
                        mask[row * w + i] += (next - x) / SS as f32;
                    }
                    x = next;
                }
            }
        }
    }
    // Penumbra: a box blur `softness` metres wide (a real card's shadow edge).
    let soft = g("softness", 0.0015)?.max(0.0);
    let r = ((soft / sw * w as f32).round() as usize).min(64);
    if r > 0 {
        let mut tmp = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
                tmp[y * w + x] = mask[y * w + a..=y * w + b].iter().sum::<f32>() / (b - a + 1) as f32;
            }
        }
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
                mask[y * w + x] = (a..=b).map(|yy| tmp[yy * w + x]).sum::<f32>() / (b - a + 1) as f32;
            }
        }
    }
    for c in &mut mask {
        *c = c.clamp(0.0, 1.0);
    }
    let tg = TextGobo {
        mask,
        w,
        h,
        cx: at.0,
        cz: at.1,
        sw,
        sh,
        card: g("height", 0.3)?.max(0.01),
        strength: g("strength", 0.92)?.clamp(0.0, 1.0),
        invert: m.get("invert").and_then(|x| x.as_bool()).unwrap_or(false),
        from: pair("from", (0.0, 0.0))?,
        t0: g("start", 0.0)?,
        dur: g("settle", 0.0)?.max(0.0),
    };
    let mut reg = gobos().write().unwrap();
    reg.push(std::sync::Arc::new(tg));
    Ok(Gobo { kind: 4, text: (reg.len() - 1) as u32, t: 0.0, scale: 1.0, soft: 0.0, dir: norm(dir), u: v3(1.0, 0.0, 0.0), v: v3(0.0, 0.0, 1.0) })
}

fn vn2(x: f32, y: f32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (tx, ty) = (x - fx, y - fy);
    let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (ix, iy) = (fx as i32 as u32, fy as i32 as u32);
    let h = |a: u32, b: u32| u01(hash2(ix.wrapping_add(a), iy.wrapping_add(b)));
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    l(l(h(0, 0), h(1, 0), tx), l(h(0, 1), h(1, 1), tx), ty)
}

/// One light of a rig, relative to the camera.
struct L {
    /// Azimuth from the camera direction (degrees, + = camera left) and elevation.
    az: f32,
    el: f32,
    /// Size in degrees (w × h) and roll.
    w: f32,
    h: f32,
    roll: f32,
    /// Illuminance relative to the key (negative: a black flag).
    e: f32,
    /// Colour temperature shift: -1 cool .. +1 warm.
    warm: f32,
    soft: f32,
}

struct Preset {
    key: L,
    fill: Option<L>,
    rim: Vec<L>,
    /// Other boxes (strips, flags, top lights) with absolute relative illuminance.
    extra: Vec<L>,
    ratio: f32,
    gobo: u8,
}

fn l(az: f32, el: f32, w: f32, h: f32, e: f32, warm: f32) -> L {
    L { az, el, w, h, roll: 0.0, e, warm, soft: 0.04 }
}

fn preset(name: &str) -> Option<Preset> {
    Some(match name {
        // Classic portrait key 45° to the side and 40° up; the shadow side keeps
        // a triangle of light. 4:1.
        "rembrandt" => Preset { key: l(45.0, 40.0, 28.0, 32.0, 1.0, 0.15), fill: Some(l(-35.0, 10.0, 70.0, 50.0, 1.0, -0.2)), rim: vec![l(-150.0, 35.0, 10.0, 50.0, 0.8, -0.3)], extra: vec![], ratio: 4.0, gobo: 0 },
        // Loop: a little closer to the camera than Rembrandt, 3:1.
        "loop" => Preset { key: l(30.0, 35.0, 30.0, 30.0, 1.0, 0.1), fill: Some(l(-30.0, 5.0, 70.0, 50.0, 1.0, -0.1)), rim: vec![l(160.0, 40.0, 10.0, 50.0, 0.6, 0.0)], extra: vec![], ratio: 3.0, gobo: 0 },
        // Beauty clamshell: a big key above the lens, a fill below it, 2:1.
        "clamshell" | "beauty" => Preset { key: l(0.0, 35.0, 70.0, 30.0, 1.0, 0.05), fill: Some(l(0.0, -15.0, 80.0, 20.0, 1.0, 0.0)), rim: vec![l(140.0, 20.0, 8.0, 70.0, 0.7, 0.0), l(-140.0, 20.0, 8.0, 70.0, 0.7, 0.0)], extra: vec![], ratio: 2.0, gobo: 0 },
        "butterfly" => Preset { key: l(0.0, 50.0, 30.0, 30.0, 1.0, 0.05), fill: Some(l(0.0, -10.0, 60.0, 20.0, 1.0, 0.0)), rim: vec![], extra: vec![], ratio: 3.0, gobo: 0 },
        // Product: a big top key, two tall strips behind the sides that draw
        // crisp edge highlights on metal and glass, black flags at the sides
        // for contrast, a soft front fill. 3:1.
        "product" | "edge" => Preset {
            key: l(20.0, 65.0, 60.0, 40.0, 1.0, 0.0),
            fill: Some(l(0.0, 15.0, 90.0, 25.0, 1.0, 0.0)),
            rim: vec![
                L { az: 115.0, el: 15.0, w: 7.0, h: 80.0, roll: 0.0, e: 1.6, warm: 0.0, soft: 0.01 },
                L { az: -115.0, el: 15.0, w: 7.0, h: 80.0, roll: 0.0, e: 1.4, warm: -0.1, soft: 0.01 },
            ],
            extra: vec![L { az: 75.0, el: 10.0, w: 30.0, h: 70.0, roll: 0.0, e: -1.0, warm: 0.0, soft: 0.03 }, L { az: -75.0, el: 10.0, w: 30.0, h: 70.0, roll: 0.0, e: -1.0, warm: 0.0, soft: 0.03 }],
            ratio: 3.0,
            gobo: 0,
        },
        // High key: bright, almost shadowless, 1.5:1.
        "high-key" => Preset { key: l(25.0, 40.0, 90.0, 70.0, 1.0, 0.0), fill: Some(l(-30.0, 20.0, 100.0, 70.0, 1.0, 0.0)), rim: vec![l(180.0, 40.0, 60.0, 60.0, 0.6, 0.0)], extra: vec![], ratio: 1.5, gobo: 0 },
        // Low key / noir: a hard side key, deep shadows (16:1), a cool rim.
        "low-key" | "noir" => Preset { key: L { az: 70.0, el: 30.0, w: 8.0, h: 10.0, roll: 0.0, e: 1.0, warm: 0.2, soft: 0.01 }, fill: Some(l(-40.0, 0.0, 60.0, 40.0, 1.0, -0.4)), rim: vec![L { az: -150.0, el: 25.0, w: 5.0, h: 60.0, roll: 0.0, e: 1.2, warm: -0.6, soft: 0.01 }], extra: vec![], ratio: 16.0, gobo: 0 },
        // Split: the key at 90°, half the subject in shadow.
        "split" => Preset { key: L { az: 90.0, el: 10.0, w: 20.0, h: 30.0, roll: 0.0, e: 1.0, warm: 0.0, soft: 0.02 }, fill: None, rim: vec![], extra: vec![], ratio: 1e3, gobo: 0 },
        // Rim / silhouette: light from behind only, edges glow.
        "rim" | "silhouette" => Preset {
            key: L { az: 160.0, el: 25.0, w: 10.0, h: 60.0, roll: 0.0, e: 1.0, warm: 0.0, soft: 0.01 },
            fill: Some(l(0.0, 10.0, 90.0, 40.0, 1.0, -0.3)),
            rim: vec![L { az: -160.0, el: 25.0, w: 10.0, h: 60.0, roll: 0.0, e: 1.0, warm: 0.0, soft: 0.01 }],
            extra: vec![],
            ratio: 12.0,
            gobo: 0,
        },
        // Golden hour: a low warm sun with leaf shadows, a cool sky fill.
        "golden" => Preset { key: L { az: 60.0, el: 12.0, w: 4.0, h: 4.0, roll: 0.0, e: 1.0, warm: 1.0, soft: 0.01 }, fill: Some(l(-60.0, 50.0, 120.0, 60.0, 1.0, -0.8)), rim: vec![], extra: vec![], ratio: 5.0, gobo: 3 },
        // Window: daylight through panes, a crisp pattern on the set.
        "window" => Preset { key: L { az: 70.0, el: 30.0, w: 6.0, h: 6.0, roll: 0.0, e: 1.0, warm: 0.15, soft: 0.01 }, fill: Some(l(-50.0, 20.0, 100.0, 60.0, 1.0, -0.3)), rim: vec![], extra: vec![], ratio: 6.0, gobo: 2 },
        // Top: a single overhead source, like a pendant or a skylight.
        "top" => Preset { key: L { az: 0.0, el: 85.0, w: 25.0, h: 25.0, roll: 0.0, e: 1.0, warm: 0.1, soft: 0.02 }, fill: Some(l(0.0, 10.0, 90.0, 40.0, 1.0, 0.0)), rim: vec![], extra: vec![], ratio: 10.0, gobo: 0 },
        // Pinspot: a museum vitrine. One narrow source straight down, a faint
        // warm rim, everything else falls to black. ~30:1.
        "pinspot" | "vitrine" => Preset {
            key: L { az: 10.0, el: 82.0, w: 5.0, h: 5.0, roll: 0.0, e: 1.0, warm: 0.25, soft: 0.005 },
            fill: Some(l(0.0, 10.0, 80.0, 40.0, 1.0, -0.2)),
            rim: vec![L { az: 165.0, el: 30.0, w: 6.0, h: 30.0, roll: 0.0, e: 0.35, warm: 0.3, soft: 0.01 }],
            extra: vec![],
            ratio: 30.0,
            gobo: 0,
        },
        // Stage: a hard frontal key from high (a follow spot), a strong cool
        // backlight that draws a halo, little fill. Pair with `beam`.
        "stage" | "theatre" => Preset {
            key: L { az: 15.0, el: 38.0, w: 6.0, h: 6.0, roll: 0.0, e: 1.0, warm: 0.2, soft: 0.005 },
            fill: Some(l(-40.0, 5.0, 70.0, 40.0, 1.0, -0.3)),
            rim: vec![L { az: 180.0, el: 45.0, w: 10.0, h: 10.0, roll: 0.0, e: 1.6, warm: -0.5, soft: 0.01 }],
            extra: vec![],
            ratio: 10.0,
            gobo: 0,
        },
        _ => return None,
    })
}

/// A parsed rig, ready to apply once the camera direction is known.
pub struct Rig {
    p: Preset,
    /// A local spot (pinspot, follow spot) for this preset: azimuth, elevation,
    /// distance in subject radii, cone angle, penumbra.
    spot: Option<(f32, f32, f32, f32, f32)>,
    ratio: f32,
    rim: f32,
    key: f32,
    angle: f32,
    height: Option<f32>,
    warmth: f32,
    gobo: u8,
    gobo_scale: f32,
    gobo_soft: f32,
    pub beam: bool,
    /// 0 still; 1 `sweep` (strips travel across, a highlight runs over chrome);
    /// 2 `reveal` (rim first, then the key, then the fill).
    pub anim: u8,
    /// Animation start and length (seconds).
    pub anim_t: (f32, f32),
}

pub fn parse(v: &Value, path: &str) -> Res<Rig> {
    let empty = Map::new();
    let (name, m) = match v {
        Value::String(s) => (s.clone(), &empty),
        Value::Object(m) => {
            check_keys(m, &["preset", "ratio", "rim", "key", "angle", "height", "warmth", "gobo", "gobo_scale", "softness", "beam", "animate", "duration", "start"], path)?;
            (m.get("preset").and_then(|p| p.as_str()).unwrap_or("rembrandt").to_string(), m)
        }
        _ => return Err(format!("{path}: expected a rig name or {{ preset, ratio, rim, ... }}")),
    };
    let name = crate::components::kebab(&name);
    let p = preset(&name).ok_or_else(|| {
        let hint = crate::components::style::closest(&name, RIGS).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
        format!("{path}: unknown rig `{name}`{hint} (one of {})", RIGS.join(", "))
    })?;
    let g = |k: &str, d: f32| -> Res<f32> { m.get(k).map(|x| f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
    let gobo = match m.get("gobo") {
        None => p.gobo,
        Some(Value::Bool(false)) => 0,
        Some(Value::String(s)) => match s.as_str() {
            "none" => 0,
            "blinds" | "venetian" => 1,
            "window" => 2,
            "leaves" | "foliage" => 3,
            o => return Err(format!("{path}.gobo: unknown gobo `{o}` (blinds, window, leaves)")),
        },
        Some(_) => return Err(format!("{path}.gobo: expected blinds, window, leaves or false")),
    };
    let ratio = g("ratio", p.ratio)?.max(1.0);
    let spot = match name.as_str() {
        "pinspot" | "vitrine" => Some((10.0, 84.0, 4.5, 24.0, 0.45)),
        "stage" | "theatre" => Some((15.0, 38.0, 6.0, 20.0, 0.25)),
        _ => None,
    };
    Ok(Rig {
        spot,
        ratio,
        rim: g("rim", 1.0)?.max(0.0),
        key: g("key", 1.0)?.max(0.0),
        angle: g("angle", p.key.az)?,
        height: m.get("height").map(|x| f(x, &format!("{path}.height"))).transpose()?,
        warmth: g("warmth", 0.0)?.clamp(-1.0, 1.0),
        gobo,
        gobo_scale: g("gobo_scale", 0.6)?.max(0.01),
        gobo_soft: g("softness", if gobo == 3 { 0.35 } else { 0.15 })?.clamp(0.0, 1.0),
        beam: m.get("beam").and_then(|b| b.as_bool()).unwrap_or(false),
        anim: match m.get("animate").and_then(|a| a.as_str()) {
            None => 0,
            Some("sweep") => 1,
            Some("reveal") => 2,
            Some(o) => return Err(format!("{path}.animate: unknown `{o}` (sweep, reveal)")),
        },
        anim_t: (g("start", 0.0)?, g("duration", 2.5)?.max(0.05)),
        p,
    })
}

/// Colour of a light at temperature shift `w` (-1 cool, +1 warm), unit luminance.
fn temp(w: f32) -> V3 {
    let c = if w >= 0.0 { lerp3(v3(1.0, 1.0, 1.0), v3(1.0, 0.72, 0.42), w) } else { lerp3(v3(1.0, 1.0, 1.0), v3(0.68, 0.82, 1.0), -w) };
    scale(c, 1.0 / luma(c).max(1e-4))
}

impl Rig {
    /// The environment with the rig's lights placed around `view` (camera →
    /// subject direction), plus the key's gobo. `level` is the key's diffuse
    /// irradiance factor (the scene's exposure is set from it).
    pub fn apply(&self, env: &Env, view: V3, level: f32) -> (Env, Gobo) {
        self.apply_at(env, view, level, f32::INFINITY)
    }

    /// The rig at time `t` (its animation, if any; `t` = ∞ is the end state).
    pub fn apply_at(&self, env: &Env, view: V3, level: f32, t: f32) -> (Env, Gobo) {
        let u = if self.anim == 0 { 1.0 } else { ((t - self.anim_t.0) / self.anim_t.1).clamp(0.0, 1.0) };
        let ease = |a: f32, b: f32| smoothstep(a, b, u);
        // Sweep: strips travel 130° across the back; reveal: rim, then key, then fill.
        let sweep = if self.anim == 1 { -65.0 + 130.0 * ease(0.0, 1.0) } else { 0.0 };
        let (k_on, f_on, r_on) = if self.anim == 2 { (ease(0.25, 0.7), ease(0.55, 1.0), ease(0.0, 0.3)) } else { (1.0, 1.0, 1.0) };
        let cam_az = (-view.x).atan2(-view.z);
        let dir = |az: f32, el: f32| {
            let a = cam_az + az.to_radians();
            let e = el.to_radians();
            v3(a.sin() * e.cos(), e.sin(), a.cos() * e.cos())
        };
        let make = |li: &L, e: f32, az: f32, el: f32| -> Softbox {
            let mut b = Softbox::new(dir(az, el), li.w, li.h, li.roll, v3(1.0, 1.0, 1.0));
            b.soft = li.soft;
            b.falloff = 0.15;
            if e < 0.0 {
                // A black flag (negative radiance marks it).
                b.radiance = v3(e, e, e);
                b.falloff = 0.0;
                return b;
            }
            let sa = b.solid_angle().max(1e-5);
            let k = e * level * std::f32::consts::PI / (sa * (1.0 - b.falloff / 3.0));
            b.radiance = scale(temp((li.warm + self.warmth * 0.6).clamp(-1.0, 1.0)), k);
            b
        };
        let key_el = self.height.unwrap_or(self.p.key.el);
        // (A switched-off key keeps a tiny level: it still carries the traced shadows.)
        // Rigs with a local spot keep the softbox key only as a faint sky.
        let key_e = if self.spot.is_some() { 0.04 } else { self.key };
        let mut boxes = vec![make(&self.p.key, (key_e * k_on).max(1e-4), self.angle, key_el)];
        if let Some(fl) = &self.p.fill {
            // Fill so that (key + fill) : fill = ratio on the key side vs the shadow side.
            let e = self.key / (self.ratio - 1.0).max(1e-3) * f_on;
            if e > 1e-3 {
                boxes.push(make(fl, e, fl.az, fl.el));
            }
        }
        for r in &self.p.rim {
            let az = if r.az >= 0.0 { r.az + sweep } else { r.az - sweep };
            boxes.push(make(r, r.e * self.rim * self.key * r_on, az, r.el));
        }
        if self.anim == 1 && self.p.rim.is_empty() {
            // Rigs without strips get one for the sweep.
            let strip = L { az: 150.0 + sweep, el: 20.0, w: 6.0, h: 80.0, roll: 0.0, e: 1.5, warm: 0.0, soft: 0.01 };
            boxes.push(make(&strip, strip.e * self.rim * self.key, strip.az, strip.el));
        }
        for x in &self.p.extra {
            boxes.push(make(x, x.e, x.az, x.el));
        }
        // Ambient: the env's sky gradient, kept dim so the rig's ratios hold.
        let s = |c: V3| scale(c, 0.35);
        let out = Env::new(boxes, Some(0), s(env.zenith), s(env.horizon), s(env.ground), scale(env.band, 0.5));
        let kd = dir(self.angle, key_el);
        let up = if kd.y.abs() > 0.95 { v3(0.0, 0.0, 1.0) } else { v3(0.0, 1.0, 0.0) };
        let u = norm(cross(up, kd));
        let v = cross(kd, u);
        (out, Gobo { kind: self.gobo, text: 0, t: 0.0, scale: self.gobo_scale, soft: self.gobo_soft, dir: kd, u, v })
    }
}

impl Rig {
    /// Local spot lights the rig places around the subject (`centre`, `radius`),
    /// sized so the pool on the subject matches the key `level`.
    pub fn spots(&self, centre: V3, radius: f32, view: V3, level: f32) -> Vec<super::trace::Light> {
        let Some((az, el, dist, angle, pen)) = self.spot else { return Vec::new() };
        let cam_az = (-view.x).atan2(-view.z);
        let a = cam_az + (az + self.angle - self.p.key.az).to_radians();
        let e = self.height.unwrap_or(el).to_radians();
        let d = dist * radius.max(0.3);
        let pos = add(centre, scale(v3(a.sin() * e.cos(), e.sin(), a.cos() * e.cos()), d));
        let outer = (angle * 0.5).to_radians();
        let col = temp((self.p.key.warm + self.warmth * 0.6).clamp(-1.0, 1.0));
        // Irradiance factor at the subject ≈ power / d² (π folded in like the softboxes).
        let power = scale(col, level * self.key * d * d * std::f32::consts::PI * 1.4);
        vec![super::trace::Light { pos, power, dir: norm(sub(centre, pos)), cos_outer: outer.cos(), cos_inner: (outer * (1.0 - pen)).cos(), shadow: true }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rigs_hold_their_ratio_and_face_the_camera() {
        let env = Env::preset_tinted("studio", v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(1.0, 1.0, 1.0), v3(0.0, 1.0, 0.0)).unwrap();
        let r = parse(&json!({ "preset": "rembrandt", "ratio": 4 }), "rig").unwrap();
        // Camera looking down -Z: the key (az +45) sits camera-left of the subject, in front.
        let (e, _) = r.apply(&env, v3(0.0, 0.0, -1.0), 1.0);
        let k = e.key_box().unwrap();
        assert!(k.c.z > 0.3 && k.c.y > 0.3, "{:?}", k.c);
        let irr = |b: &Softbox| luma(b.radiance) * b.solid_angle() * (1.0 - b.falloff / 3.0) / std::f32::consts::PI;
        let (ek, ef) = (irr(&e.boxes[0]), irr(&e.boxes[1]));
        assert!(((ek + ef) / ef - 4.0).abs() < 0.05, "{ek} {ef}");
        assert!(parse(&json!("rembrant"), "rig").err().unwrap().contains("did you mean `rembrandt`"));
        let g = parse(&json!({ "preset": "window" }), "rig").unwrap().apply(&env, v3(0.0, 0.0, -1.0), 1.0).1;
        let lit: Vec<f32> = (0..50).map(|i| g.at(add(scale(g.u, i as f32 * 0.02), scale(g.v, 0.07)))).collect();
        assert!(lit.iter().any(|&x| x < 0.2) && lit.iter().any(|&x| x > 0.8), "window gobo casts mullions");
        // A reveal starts dark (rim only) and ends at the full rig.
        let rv = parse(&json!({ "preset": "rembrandt", "animate": "reveal", "duration": 2 }), "rig").unwrap();
        let (e0, e1) = (rv.apply_at(&env, v3(0.0, 0.0, -1.0), 1.0, 0.0).0, rv.apply_at(&env, v3(0.0, 0.0, -1.0), 1.0, 2.0).0);
        assert!(luma(e0.boxes[0].radiance) < luma(e1.boxes[0].radiance) * 0.01);
        let sw = parse(&json!({ "preset": "product", "animate": "sweep" }), "rig").unwrap();
        let (a, b) = (sw.apply_at(&env, v3(0.0, 0.0, -1.0), 1.0, 0.0).0, sw.apply_at(&env, v3(0.0, 0.0, -1.0), 1.0, 2.5).0);
        assert!(dot(a.boxes[2].c, b.boxes[2].c) < 0.5, "the strip travels");
    }
}

#[cfg(test)]
mod text_gobo_tests {
    use super::*;

    #[test]
    fn a_word_is_printed_where_asked_and_slides_in() {
        let d = norm(v3(0.6, 0.6, 0.2));
        let v: Value = serde_json::json!({ "text": "HI", "font": "Inter", "size": 0.1, "at": [0.0, 0.0], "height": 0.3, "from": [0.5, 0.0], "start": 0.0, "settle": 1.0 });
        let mut g = text_gobo(&v, d, "gobo").unwrap();
        g.t = 2.0;
        // Shadow letters: somewhere across the word's footprint the light is blocked; far away it isn't.
        let dark = (0..200).map(|i| g.at(v3(-0.08 + i as f32 * 0.0008, 0.0, 0.0))).fold(1.0f32, f32::min);
        assert!(dark < 0.3, "{dark}");
        assert_eq!(g.at(v3(1.0, 0.0, 1.0)), 1.0);
        // Before it settles the word is still to the right: the centre is lit.
        g.t = 0.0;
        let early = (0..200).map(|i| g.at(v3(-0.08 + i as f32 * 0.0008, 0.0, 0.0))).fold(1.0f32, f32::min);
        assert!(early > 0.9, "{early}");
        // Inverted: light only through the letters.
        let inv: Value = serde_json::json!({ "text": "HI", "font": "Inter", "size": 0.1, "invert": true, "strength": 0.8 });
        let gi = text_gobo(&inv, d, "gobo").unwrap();
        assert!((gi.at(v3(1.0, 0.0, 1.0)) - 0.2).abs() < 1e-4);
        assert!(text_gobo(&serde_json::json!({ "txt": "x" }), d, "gobo").unwrap_err().contains("txt"));
    }
}
