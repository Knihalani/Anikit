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

pub const RIGS: &[&str] = &["rembrandt", "clamshell", "product", "high-key", "low-key", "split", "rim", "golden", "window", "top", "butterfly", "loop"];

/// A gobo (cookie) in front of the key: the shadow pattern of a window,
/// blinds or foliage, projected along the key direction.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gobo {
    /// 0 none, 1 venetian blinds, 2 window panes, 3 leaves.
    pub kind: u8,
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
                // Dappled foliage: two octaves of blobs.
                let n = vn2(a * 3.0, b * 3.0) * 0.65 + vn2(a * 7.3 + 11.0, b * 7.3) * 0.35;
                smoothstep(0.52 - s * 0.3, 0.52 + s * 0.3, n)
            }
        }
    }
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
        _ => return None,
    })
}

/// A parsed rig, ready to apply once the camera direction is known.
pub struct Rig {
    p: Preset,
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
    Ok(Rig {
        ratio,
        rim: g("rim", 1.0)?.max(0.0),
        key: g("key", 1.0)?.max(0.0),
        angle: g("angle", p.key.az)?,
        height: m.get("height").map(|x| f(x, &format!("{path}.height"))).transpose()?,
        warmth: g("warmth", 0.0)?.clamp(-1.0, 1.0),
        gobo,
        gobo_scale: g("gobo_scale", 0.6)?.max(0.01),
        gobo_soft: g("softness", 0.15)?.clamp(0.0, 1.0),
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
        let mut boxes = vec![make(&self.p.key, (self.key * k_on).max(1e-4), self.angle, key_el)];
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
        (out, Gobo { kind: self.gobo, scale: self.gobo_scale, soft: self.gobo_soft, dir: kd, u, v })
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
