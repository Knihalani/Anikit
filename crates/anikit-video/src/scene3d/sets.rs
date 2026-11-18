//! Set builders: the stage around the subject, the way a set designer builds
//! it. Each expands into ordinary `objects` (so materials, keys and
//! expressions still apply), and may lower the floor.
//!
//! `"set": "plinth"`, `{ "preset": "field", "color": "#2B2D31", "size": 1 }`,
//! or a list of these.
//! - `plinth` / `podium`: a two-tier round (or square) stand under the subject;
//!   the floor drops to its base, so subjects at y = 0 sit on top.
//! - `steps`: stepped blocks rising behind the subject.
//! - `sweep`: a seamless coloured cyc (floor, curve and wall in one piece).
//! - `field`: an abstract field of columns with noise heights, with a clearing
//!   for the subject.
//! - `arches`: a receding corridor of half-buried rings.
//! - `terrain`: rolling ground (fBm noise) that flattens near the subject.

use super::spec::{check_keys, f, Res};
use serde_json::{json, Value};

pub const SETS: &[&str] = &["plinth", "podium", "steps", "sweep", "field", "arches", "terrain"];

/// The objects for `set`, and the floor height it needs (if it changes it).
pub struct Built {
    pub objects: Vec<Value>,
    pub floor_y: Option<f32>,
    /// Turn the flat floor off (terrain replaces it).
    pub no_floor: bool,
}

/// `fit`: the subject's footprint half-width (m), so stands and clearings fit it.
pub fn build(v: &Value, path: &str, fit: Option<f32>) -> Res<Built> {
    let mut out = Built { objects: Vec::new(), floor_y: None, no_floor: false };
    let list: Vec<(usize, &Value)> = match v {
        Value::Array(a) => a.iter().enumerate().collect(),
        _ => vec![(usize::MAX, v)],
    };
    for (k, item) in list {
        let p = if k == usize::MAX { path.to_string() } else { format!("{path}[{k}]") };
        let (name, m) = match item {
            Value::String(s) => (s.clone(), serde_json::Map::new()),
            Value::Object(m) => {
                check_keys(m, &["preset", "color", "material", "size", "shape", "density"], &p)?;
                (m.get("preset").and_then(|x| x.as_str()).ok_or_else(|| format!("{p}: missing `preset` (one of {})", SETS.join(", ")))?.to_string(), m.clone())
            }
            _ => return Err(format!("{p}: expected a set name or {{ preset, color, size }}")),
        };
        let name = crate::components::kebab(&name);
        let size = m.get("size").map(|x| f(x, &format!("{p}.size"))).transpose()?.unwrap_or(1.0).max(0.05);
        let mat = |base: &str, rough: f32| -> Value {
            match (m.get("material"), m.get("color")) {
                (Some(mt), _) => mt.clone(),
                (None, Some(c)) => json!({ "extends": "matte", "base": c, "roughness": rough }),
                (None, None) => json!({ "extends": "matte", "base": base, "roughness": rough }),
            }
        };
        match name.as_str() {
            "plinth" | "podium" => {
                let square = name == "podium" || m.get("shape").and_then(|s| s.as_str()) == Some("square");
                // Fit the subject with a 15% margin, unless `size` is given.
                let r = match (m.contains_key("size"), fit) {
                    (false, Some(w)) => (w * 1.15).max(0.3),
                    _ => 0.62 * size,
                };
                let h = (0.32 * size).min(r * 0.24);
                let (r2, h2) = (r * 1.32, h * 0.38);
                let top = mat("#D8D4CD", 0.5);
                if square {
                    out.objects.push(json!({ "type": "box", "size": [r * 2.0, h, r * 2.0], "radius": 0.012 * size, "pos": [0, -h * 0.5, 0], "material": top, "name": "plinth" }));
                    out.objects.push(json!({ "type": "box", "size": [r2 * 2.0, h2, r2 * 2.0], "radius": 0.01 * size, "pos": [0, -h - h2 * 0.5, 0], "material": top }));
                } else {
                    out.objects.push(json!({ "type": "cylinder", "radius": r, "height": h, "pos": [0, -h * 0.5, 0], "material": top, "name": "plinth" }));
                    out.objects.push(json!({ "type": "cylinder", "radius": r2, "height": h2, "pos": [0, -h - h2 * 0.5, 0], "material": top }));
                }
                out.floor_y = Some(-(h + h2));
            }
            "steps" => {
                let m0 = mat("#CFCAC2", 0.6);
                for s in 0..4 {
                    let hh = (0.18 + 0.2 * s as f32) * size;
                    let z = -(1.1 + 0.55 * s as f32) * size;
                    out.objects.push(json!({ "type": "box", "size": [5.0 * size, hh, 0.55 * size], "radius": 0.01 * size, "pos": [0, hh * 0.5, z], "material": m0 }));
                }
            }
            "sweep" => {
                // Floor → quarter curve (radius 1.5) → wall, 16 m wide.
                let (d, r) = (2.6 * size, 1.5 * size);
                out.objects.push(json!({
                    "type": "surface",
                    "x": "=u",
                    // v: 0–0.35 the floor (from in front of the camera), 0.35–0.65
                    // the curve, 0.65–1 the wall.
                    "y": format!("=0.002+{r}-{r}*cos(clamp((v-0.35)/0.3,0,1)*1.5708)+max(0,(v-0.65)/0.35)*{}", 6.0 * size),
                    "z": format!("={f}-({f}+{d})*clamp(v/0.35,0,1)-{r}*sin(clamp((v-0.35)/0.3,0,1)*1.5708)", f = 9.0 * size),
                    "u": [-9.0 * size, 9.0 * size, 2],
                    "v": [0, 1, 120],
                    "material": mat("#E9E5DE", 0.85),
                    "shadow": false
                }));
            }
            "field" => {
                // A 28×28 grid of columns, noise heights, a clearing of radius 1.6.
                let density = m.get("density").map(|x| f(x, &format!("{p}.density"))).transpose()?.unwrap_or(1.0).clamp(0.25, 3.0);
                let cols = (28.0 * density.sqrt()).round() as i64;
                let sp = 0.42 * size / density.sqrt();
                let half = (cols as f32 - 1.0) * 0.5;
                let x = format!("(mod(i,{cols})-{half})*{sp}");
                // The field sits mostly behind the subject and rises with distance,
                // so it frames the subject instead of hiding it.
                let z = format!("(floor(i/{cols})-{half})*{sp}-{}", (half * sp - 1.0 * size).max(0.0));
                let hgt = format!("({}+{}*max(0,noise2({x}*0.55,{z}*0.55)+0.35)*(0.25+0.75*clamp(-({z})/{},0,1)))", 0.05 * size, 1.8 * size, 6.0 * size);
                let c0 = (1.3 * size).max(fit.unwrap_or(0.0) * 1.3);
                let clear = format!("smoothstep({},{},hypot({x},{z}))", c0, c0 + 0.6 * size);
                out.objects.push(json!({
                    "type": "box",
                    "count": cols * cols,
                    "size": [sp * 0.86, 1.0, sp * 0.86],
                    "radius": 0.006 * size,
                    "pos": format!("=[{x}, {hgt}*{clear}*0.5, {z}]"),
                    "scale": format!("=[{clear}, {hgt}*{clear}+0.0001, {clear}]"),
                    "material": mat("#2E3036", 0.45)
                }));
            }
            "arches" => {
                out.objects.push(json!({
                    "type": "torus",
                    "count": 7,
                    "radius": 2.2 * size,
                    "tube": 0.09 * size,
                    "pos": format!("=[0, 0, -{}-i*{}]", 1.4 * size, 1.6 * size),
                    "material": mat("#D9D3C8", 0.55)
                }));
            }
            "terrain" => {
                let e = format!(
                    "=({a}*noise2(x*0.13,z*0.13)+{b}*noise2(x*0.41+3.1,z*0.41)+{c}*noise2(x*1.3+7.7,z*1.3)+{a})*smoothstep({r0},{r1},hypot(x,z))",
                    a = 0.9 * size,
                    b = 0.3 * size,
                    c = 0.08 * size,
                    r0 = 2.2 * size,
                    r1 = 7.0 * size
                );
                out.objects.push(json!({ "type": "heightfield", "expr": e, "size": [60.0 * size, 60.0 * size], "res": 220, "material": mat("#59613F", 0.9) }));
                out.no_floor = true;
            }
            o => {
                let hint = crate::components::style::closest(o, SETS).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(format!("{p}: unknown set `{o}`{hint} (one of {})", SETS.join(", ")));
            }
        }
    }
    Ok(out)
}
