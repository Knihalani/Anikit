//! `scene3d` props → a prepared scene description.
//!
//! Everything flexible (`env`, `camera`, `floor`, `pos`, `rot`, `material`)
//! is read from JSON by hand so errors can name the exact field and suggest
//! fixes. Keys arrive snake_cased (the component layer converts camelCase).

use super::env::Env;
use super::geom;
use super::math::*;
use super::trace::{Face, Material, Shape};
use super::bvh::Bvh;
use crate::eval::expr::{Expr, Vars};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub type Res<T> = Result<T, String>;

/// A number or expression of `t` (seconds), `i` (instance index), `lane`, `n` (count).
#[derive(Clone, Debug)]
pub enum Num {
    C(f32),
    E(Arc<Expr>, bool),
}

impl Num {
    #[inline]
    pub fn at(&self, v: &Vars) -> f32 {
        match self {
            Num::C(c) => *c,
            Num::E(e, _) => e.eval(v) as f32,
        }
    }
    pub fn dynamic(&self) -> bool {
        matches!(self, Num::E(_, true))
    }
}

fn compile(src: &str) -> Res<Num> {
    // `lane` is spelled as the evaluator's `T` variable.
    let mut s = String::with_capacity(src.len());
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let word = |k: usize, w: &str| src[k..].starts_with(w) && (k == 0 || !(b[k - 1].is_ascii_alphanumeric() || b[k - 1] == b'_')) && b.get(k + w.len()).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        if word(i, "lane") {
            s.push('T');
            i += 4;
        } else {
            s.push(b[i] as char);
            i += 1;
        }
    }
    let e = Expr::compile(&s, 1.0, 1920.0, 1080.0).map_err(|e| format!("expression `{src}`: {e}"))?;
    if let Some(c) = e.constant() {
        return Ok(Num::C(c as f32));
    }
    let dynamic = {
        let has_t = |w: &str| {
            let mut k = 0;
            while let Some(p) = s[k..].find(w) {
                let a = k + p;
                let before = a == 0 || !(s.as_bytes()[a - 1].is_ascii_alphanumeric());
                let after = s.as_bytes().get(a + w.len()).is_none_or(|c| !c.is_ascii_alphanumeric());
                if before && after {
                    return true;
                }
                k = a + w.len();
            }
            false
        };
        has_t("t") || has_t("frame") || has_t("gt")
    };
    Ok(Num::E(Arc::new(e), dynamic))
}

fn num(v: &Value, path: &str) -> Res<Num> {
    match v {
        Value::Number(n) => Ok(Num::C(n.as_f64().unwrap_or(0.0) as f32)),
        Value::String(s) => compile(s.strip_prefix('=').unwrap_or(s)),
        _ => Err(format!("{path}: expected a number or an \"=expression\"")),
    }
}

fn f(v: &Value, path: &str) -> Res<f32> {
    v.as_f64().map(|x| x as f32).filter(|x| x.is_finite()).ok_or_else(|| format!("{path}: expected a number"))
}

/// Split `[a, b, c]` at top-level commas.
fn split_list(s: &str) -> Option<Vec<String>> {
    let s = s.trim();
    let inner = s.strip_prefix('[')?.strip_suffix(']')?;
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in inner.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                cur.push(c)
            }
            ')' | ']' => {
                depth -= 1;
                cur.push(c)
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    Some(out)
}

/// `[x, y, z]` of numbers or expressions, or `"=[a, b, c]"`.
pub fn vec3(v: &Value, path: &str) -> Res<[Num; 3]> {
    match v {
        Value::Array(a) if a.len() == 3 => Ok([num(&a[0], path)?, num(&a[1], path)?, num(&a[2], path)?]),
        Value::String(s) => {
            let body = s.strip_prefix('=').unwrap_or(s);
            let parts = split_list(body).ok_or_else(|| format!("{path}: expected [x, y, z] or \"=[x, y, z]\""))?;
            if parts.len() != 3 {
                return Err(format!("{path}: expected 3 values, got {}", parts.len()));
            }
            Ok([compile(&parts[0])?, compile(&parts[1])?, compile(&parts[2])?])
        }
        Value::Number(n) => {
            let c = n.as_f64().unwrap_or(0.0) as f32;
            Ok([Num::C(c), Num::C(c), Num::C(c)])
        }
        _ => Err(format!("{path}: expected [x, y, z]")),
    }
}

pub fn eval3(v: &[Num; 3], vars: &Vars) -> V3 {
    v3(v[0].at(vars), v[1].at(vars), v[2].at(vars))
}

/// A colour: `"#rrggbb"` (sRGB) or `[r, g, b]` (linear, may exceed 1).
pub fn color(v: &Value, path: &str) -> Res<V3> {
    match v {
        Value::String(s) => {
            let h = s.trim_start_matches('#');
            let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("zz"), 16).map(|x| x as f32 / 255.0);
            let (r, g, b) = match h.len() {
                6 | 8 => (p(0), p(2), p(4)),
                3 => {
                    let q = |i: usize| u8::from_str_radix(&h[i..i + 1].repeat(2), 16).map(|x| x as f32 / 255.0);
                    (q(0), q(1), q(2))
                }
                _ => return Err(format!("{path}: expected a colour like \"#eeebe5\"")),
            };
            match (r, g, b) {
                (Ok(r), Ok(g), Ok(b)) => Ok(srgb_lin(r, g, b)),
                _ => Err(format!("{path}: bad colour `{s}`")),
            }
        }
        Value::Array(a) if a.len() == 3 => Ok(v3(f(&a[0], path)?, f(&a[1], path)?, f(&a[2], path)?)),
        _ => Err(format!("{path}: expected a colour like \"#eeebe5\"")),
    }
}

pub fn srgb_lin(r: f32, g: f32, b: f32) -> V3 {
    let f = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    v3(f(r), f(g), f(b))
}

fn check_keys(m: &Map<String, Value>, allowed: &[&str], path: &str) -> Res<()> {
    for k in m.keys() {
        if !allowed.contains(&k.as_str()) {
            let hint = crate::components::style::closest(k, allowed).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(format!("{path}: unknown key `{k}`{hint}"));
        }
    }
    Ok(())
}

// ------------------------------------------------------------ materials

pub const MATERIALS: &[&str] = &["chrome", "glass", "aluminium", "aluminum", "aluminium-bead", "matte", "white", "black", "gold", "emissive", "plastic", "ceramic", "rubber"];

pub fn builtin_material(name: &str) -> Option<Material> {
    let c = Material::chrome();
    Some(match name {
        "chrome" => c,
        "glass" => Material {
            base: v3(1.0, 1.0, 1.0),
            metallic: 0.0,
            roughness: 0.02,
            transmission: 1.0,
            ior: 1.5,
            // Soda-lime green: barely visible through 8 mm, deep green through edges.
            absorb: v3(0.9, 0.2, 0.6),
            ..c
        },
        "aluminium" | "aluminum" => Material { base: v3(0.91, 0.92, 0.92), metallic: 1.0, roughness: 0.32, ..c },
        "aluminium-bead" | "aluminum-bead" | "bead-blasted" => Material { base: v3(0.86, 0.87, 0.87), metallic: 1.0, roughness: 0.48, ..c },
        "matte" | "white" => Material { base: v3(0.8, 0.79, 0.77), metallic: 0.0, roughness: 0.75, specular: 0.6, ..c },
        "ceramic" => Material { base: v3(0.82, 0.81, 0.79), metallic: 0.0, roughness: 0.2, ..c },
        "plastic" => Material { base: v3(0.6, 0.6, 0.62), metallic: 0.0, roughness: 0.35, ..c },
        "rubber" => Material { base: v3(0.02, 0.02, 0.022), metallic: 0.0, roughness: 0.8, specular: 0.5, ..c },
        "black" => Material { base: v3(0.01, 0.01, 0.012), metallic: 0.0, roughness: 0.08, specular: 1.2, ..c },
        "gold" => Material { base: v3(1.0, 0.71, 0.38), metallic: 1.0, roughness: 0.12, ..c },
        "emissive" => Material { base: v3(0.02, 0.02, 0.02), metallic: 0.0, roughness: 0.4, emissive: v3(4.0, 4.0, 4.0), ..c },
        _ => return None,
    })
}

const MAT_KEYS: &[&str] = &["base", "color", "metallic", "roughness", "specular", "emissive", "emissive_strength", "clearcoat", "transmission", "ior", "tint", "tint_distance", "glow", "extends"];

pub fn material(v: &Value, named: &HashMap<String, Material>, path: &str) -> Res<Material> {
    match v {
        Value::String(s) => {
            let k = crate::components::kebab(s);
            named
                .get(s)
                .or_else(|| named.get(&k.replace('-', "_")))
                .cloned()
                .or_else(|| builtin_material(&k))
                .ok_or_else(|| {
                    let mut all: Vec<&str> = MATERIALS.to_vec();
                    all.extend(named.keys().map(|s| s.as_str()));
                    let hint = crate::components::style::closest(s, &all).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                    format!("{path}: unknown material `{s}`{hint}")
                })
        }
        Value::Object(m) => {
            check_keys(m, MAT_KEYS, path)?;
            let mut out = match m.get("extends") {
                Some(e) => material(e, named, &format!("{path}.extends"))?,
                None => Material { base: v3(0.8, 0.8, 0.8), metallic: 0.0, roughness: 0.4, ..Material::chrome() },
            };
            if let Some(c) = m.get("base").or(m.get("color")) {
                out.base = color(c, &format!("{path}.base"))?;
            }
            for (k, slot) in [("metallic", &mut out.metallic), ("roughness", &mut out.roughness), ("specular", &mut out.specular), ("clearcoat", &mut out.clearcoat), ("transmission", &mut out.transmission), ("ior", &mut out.ior), ("glow", &mut out.tex_glow)] {
                if let Some(x) = m.get(k) {
                    *slot = f(x, &format!("{path}.{k}"))?;
                }
            }
            if let Some(e) = m.get("emissive") {
                let s = m.get("emissive_strength").map(|x| f(x, &format!("{path}.emissiveStrength"))).transpose()?.unwrap_or(4.0);
                out.emissive = scale(color(e, &format!("{path}.emissive"))?, s);
            }
            if let Some(t) = m.get("tint") {
                // Colour seen through `tintDistance` metres of the material.
                let c = color(t, &format!("{path}.tint"))?;
                let d = m.get("tint_distance").map(|x| f(x, &format!("{path}.tintDistance"))).transpose()?.unwrap_or(0.3).max(1e-3);
                out.absorb = v3(-c.x.max(1e-4).ln() / d, -c.y.max(1e-4).ln() / d, -c.z.max(1e-4).ln() / d);
            }
            Ok(out)
        }
        _ => Err(format!("{path}: expected a material name or object")),
    }
}

// ------------------------------------------------------------ objects

/// A transform key.
#[derive(Clone, Debug)]
pub struct TKey {
    pub t: f32,
    pub pos: Option<V3>,
    pub rot: Option<V3>,
    pub scale: Option<f32>,
    /// `linear`, `smooth` (default), `in`, `out`.
    pub ease: u8,
}

#[derive(Clone, Debug)]
pub struct TexSpec {
    pub comp: String,
    /// Composition frame index (expression of `i`, `lane`, `t`) or time mapping.
    pub frame: Option<Num>,
    pub offset: f32,
    pub speed: f32,
    pub looped: bool,
    pub face: Face,
}

/// One entry of `objects`: a shape, material, placement and animation; with
/// `count`/`lanes`, a whole array of instances.
#[derive(Clone)]
pub struct ObjDef {
    pub shape: u32,
    pub mat: u32,
    pub pos: [Num; 3],
    pub rot: [Num; 3],
    pub scale: Num,
    pub keys: Vec<TKey>,
    /// Degrees per second about X, Y, Z.
    pub spin: V3,
    pub count: u32,
    pub lanes: u32,
    pub tex: Option<TexSpec>,
    pub thin: bool,
    pub shadow: bool,
    /// Half width of the shape in metres (texture LOD).
    pub extent: f32,
    /// Visible from / until (seconds).
    pub start: f32,
    pub end: f32,
    /// Shape size folded into the scale (a sphere's radius).
    pub scale_mul: f32,
}

impl ObjDef {
    pub fn dynamic(&self) -> bool {
        !self.keys.is_empty() || self.spin != v3(0.0, 0.0, 0.0) || self.pos.iter().chain(self.rot.iter()).chain(std::iter::once(&self.scale)).any(|n| n.dynamic()) || self.start > 0.0 || self.end.is_finite()
    }

    pub fn xf_at(&self, t: f32, i: u32, lane: u32) -> Xf {
        let vars = Vars { t: t as f64, scene_t: lane as f64, gt: t as f64, i: i as f64, n: self.count as f64, ..Vars::default() };
        let mut pos = eval3(&self.pos, &vars);
        let mut rot = eval3(&self.rot, &vars);
        let mut sc = self.scale.at(&vars);
        if !self.keys.is_empty() {
            let pick = |get: &dyn Fn(&TKey) -> Option<V3>| -> Option<V3> {
                let ks: Vec<&TKey> = self.keys.iter().filter(|k| get(k).is_some()).collect();
                let first = ks.first()?;
                if t <= first.t {
                    return get(first);
                }
                for w in ks.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    if t <= b.t {
                        let u = ((t - a.t) / (b.t - a.t).max(1e-6)).clamp(0.0, 1.0);
                        let e = match b.ease {
                            0 => u,
                            2 => u * u * u,
                            3 => 1.0 - (1.0 - u).powi(3),
                            _ => u * u * (3.0 - 2.0 * u),
                        };
                        return Some(lerp3(get(a)?, get(b)?, e));
                    }
                }
                get(ks.last()?)
            };
            if let Some(p) = pick(&|k| k.pos) {
                pos = add(pos, p);
            }
            if let Some(r) = pick(&|k| k.rot) {
                rot = add(rot, r);
            }
            if let Some(s) = pick(&|k| k.scale.map(|s| v3(s, s, s))) {
                sc *= s.x;
            }
        }
        rot = madd(rot, self.spin, t);
        Xf::new(pos, rot, (sc * self.scale_mul).max(1e-6))
    }
}

const OBJ_KEYS: &[&str] = &[
    "type", "text", "font", "weight", "tracking", "height", "depth", "bevel", "size", "radius", "tube", "material", "pos", "position", "rot", "rotation", "scale", "keys", "spin", "count", "lanes", "texture", "thin", "shadow", "start", "end", "name",
];
const TYPES: &[&str] = &["text", "box", "slab", "pane", "plane", "sphere", "cylinder", "torus", "phone"];

/// Shapes are deduplicated by their JSON description.
pub struct Shapes {
    pub list: Vec<Shape>,
    seen: HashMap<String, u32>,
}

impl Default for Shapes {
    fn default() -> Self {
        Shapes::new()
    }
}

impl Shapes {
    pub fn new() -> Shapes {
        Shapes { list: Vec::new(), seen: HashMap::new() }
    }
    fn add(&mut self, key: String, make: impl FnOnce() -> Res<Shape>) -> Res<u32> {
        if let Some(i) = self.seen.get(&key) {
            return Ok(*i);
        }
        let s = make()?;
        self.list.push(s);
        let i = self.list.len() as u32 - 1;
        self.seen.insert(key, i);
        Ok(i)
    }
}

fn mesh_shape(m: anikit_core::raster::Mesh) -> Shape {
    if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
        eprintln!("scene3d: mesh with {} triangles", m.idx.len() / 3);
    }
    Shape::Mesh(Arc::new(Bvh::with_uv(&m.pos, &m.nrm, &m.uv, &m.idx)))
}

pub fn object(v: &Value, shapes: &mut Shapes, mats: &mut Vec<Material>, named: &HashMap<String, Material>, path: &str) -> Res<ObjDef> {
    let m = v.as_object().ok_or_else(|| format!("{path}: expected an object"))?;
    check_keys(m, OBJ_KEYS, path)?;
    let ty = match m.get("type").and_then(|t| t.as_str()) {
        Some(t) => {
            let t = crate::components::kebab(t);
            if !TYPES.contains(&t.as_str()) {
                let hint = crate::components::style::closest(&t, TYPES).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(format!("{path}.type: unknown type `{t}`{hint}"));
            }
            t
        }
        None if m.contains_key("text") => "text".into(),
        None if m.contains_key("count") => "slab".into(),
        None => return Err(format!("{path}: missing `type` (one of {})", TYPES.join(", "))),
    };
    let gf = |k: &str, d: f32| -> Res<f32> { m.get(k).map(|x| f(x, &format!("{path}.{k}"))).transpose().map(|o| o.unwrap_or(d)) };
    let size3 = |d: [f32; 3]| -> Res<[f32; 3]> {
        match m.get("size") {
            None => Ok(d),
            Some(Value::Array(a)) if a.len() == 3 => Ok([f(&a[0], path)?, f(&a[1], path)?, f(&a[2], path)?]),
            Some(Value::Array(a)) if a.len() == 2 => Ok([f(&a[0], path)?, f(&a[1], path)?, d[2]]),
            Some(x) => {
                let s = f(x, &format!("{path}.size"))?;
                Ok([s, s, s])
            }
        }
    };
    let key = format!("{ty}|{}|{}|{}|{}|{}|{}|{}|{}|{}", m.get("text").map(|x| x.to_string()).unwrap_or_default(), m.get("font").map(|x| x.to_string()).unwrap_or_default(), m.get("weight").map(|x| x.to_string()).unwrap_or_default(), m.get("tracking").map(|x| x.to_string()).unwrap_or_default(), m.get("height").map(|x| x.to_string()).unwrap_or_default(), m.get("depth").map(|x| x.to_string()).unwrap_or_default(), m.get("bevel").map(|x| x.to_string()).unwrap_or_default(), m.get("size").map(|x| x.to_string()).unwrap_or_default(), m.get("radius").map(|x| x.to_string()).unwrap_or_default());
    let mut thin = false;
    let (shape, extent) = match ty.as_str() {
        "text" => {
            let text = m.get("text").and_then(|x| x.as_str()).unwrap_or("ANIKIT").to_string();
            let font = m.get("font").and_then(|x| x.as_str()).unwrap_or("Inter").to_string();
            let (wt, tr, hh) = (gf("weight", 800.0)?, gf("tracking", 0.02)?, gf("height", 1.0)?);
            let (dp, bv) = (gf("depth", hh * 0.34)?, gf("bevel", hh * 0.045)?);
            let s = shapes.add(key, || geom::text(&text, &font, wt, tr, hh, dp, bv).map(mesh_shape).map_err(|e| format!("{path}: {e}")))?;
            (s, hh * text.chars().count() as f32 * 0.4)
        }
        "box" => {
            let sz = size3([1.0, 1.0, 1.0])?;
            let r = gf("radius", sz[0].min(sz[1]).min(sz[2]) * 0.06)?;
            let b = gf("bevel", r)?;
            (shapes.add(key, || Ok(mesh_shape(geom::rounded_box(sz, r, b))))?, sz[0] * 0.5)
        }
        "slab" | "pane" => {
            let sz = size3([1.6, 0.9, 0.008])?;
            let r = gf("radius", sz[0].min(sz[1]) * 0.03)?;
            let b = gf("bevel", sz[2] * 0.3)?;
            thin = sz[2] <= 0.03;
            (shapes.add(key, || Ok(mesh_shape(geom::rounded_box(sz, r, b))))?, sz[0] * 0.5)
        }
        "phone" => {
            let sz = size3([0.075, 0.16, 0.008])?;
            (shapes.add(key, || Ok(mesh_shape(geom::phone(sz))))?, sz[0] * 0.5)
        }
        "plane" => {
            let sz = size3([1.0, 1.0, 0.0])?;
            (shapes.add(key, || Ok(mesh_shape(geom::plane(sz[0], sz[1]))))?, sz[0] * 0.5)
        }
        "sphere" => {
            let r = gf("radius", 0.5)?;
            // The unit sphere scaled by its radius (folded into the transform below).
            (shapes.add("sphere".into(), || Ok(Shape::Sphere))?, r)
        }
        "cylinder" => {
            let (r, hh) = (gf("radius", 0.5)?, gf("height", 1.0)?);
            (shapes.add(key, || Ok(mesh_shape(geom::cylinder(r, hh))))?, r)
        }
        "torus" => {
            let (r, tb) = (gf("radius", 0.5)?, gf("tube", 0.08)?);
            (shapes.add(key, || Ok(mesh_shape(geom::torus(r, tb))))?, r + tb)
        }
        _ => unreachable!(),
    };
    let mat = match m.get("material") {
        Some(x) => material(x, named, &format!("{path}.material"))?,
        None => builtin_material(if ty == "slab" || ty == "pane" { "glass" } else { "chrome" }).unwrap(),
    };
    if let Some(t) = m.get("thin") {
        thin = t.as_bool().ok_or_else(|| format!("{path}.thin: expected true or false"))?;
    }
    mats.push(mat);
    let mat = mats.len() as u32 - 1;
    let zero = || [Num::C(0.0), Num::C(0.0), Num::C(0.0)];
    let pos = m.get("pos").or(m.get("position")).map(|x| vec3(x, &format!("{path}.pos"))).transpose()?.unwrap_or_else(zero);
    let rot = m.get("rot").or(m.get("rotation")).map(|x| vec3(x, &format!("{path}.rot"))).transpose()?.unwrap_or_else(zero);
    let scale_n = m.get("scale").map(|x| num(x, &format!("{path}.scale"))).transpose()?.unwrap_or(Num::C(1.0));
    let mut keys = Vec::new();
    if let Some(ks) = m.get("keys") {
        let a = ks.as_array().ok_or_else(|| format!("{path}.keys: expected an array"))?;
        for (j, k) in a.iter().enumerate() {
            let kp = format!("{path}.keys[{j}]");
            let km = k.as_object().ok_or_else(|| format!("{kp}: expected an object"))?;
            check_keys(km, &["t", "at", "pos", "rot", "scale", "ease"], &kp)?;
            let t = km.get("t").or(km.get("at")).map(|x| f(x, &format!("{kp}.t"))).transpose()?.ok_or_else(|| format!("{kp}: missing `t` (seconds)"))?;
            let v3c = |x: &Value, p: &str| -> Res<V3> {
                let a = x.as_array().filter(|a| a.len() == 3).ok_or_else(|| format!("{p}: expected [x, y, z]"))?;
                Ok(v3(f(&a[0], p)?, f(&a[1], p)?, f(&a[2], p)?))
            };
            keys.push(TKey {
                t,
                pos: km.get("pos").map(|x| v3c(x, &format!("{kp}.pos"))).transpose()?,
                rot: km.get("rot").map(|x| v3c(x, &format!("{kp}.rot"))).transpose()?,
                scale: km.get("scale").map(|x| f(x, &format!("{kp}.scale"))).transpose()?,
                ease: match km.get("ease").and_then(|e| e.as_str()).unwrap_or("smooth") {
                    "linear" => 0,
                    "in" => 2,
                    "out" => 3,
                    _ => 1,
                },
            });
        }
        keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    let spin = match m.get("spin") {
        None => v3(0.0, 0.0, 0.0),
        Some(Value::Number(n)) => v3(0.0, n.as_f64().unwrap_or(0.0) as f32, 0.0),
        Some(x) => {
            let a = x.as_array().filter(|a| a.len() == 3).ok_or_else(|| format!("{path}.spin: expected degrees per second (number or [x, y, z])"))?;
            v3(f(&a[0], path)?, f(&a[1], path)?, f(&a[2], path)?)
        }
    };
    let tex = match m.get("texture") {
        None => None,
        Some(t) => Some(texture(t, &format!("{path}.texture"))?),
    };
    let count = m.get("count").map(|x| f(x, &format!("{path}.count"))).transpose()?.unwrap_or(1.0).max(0.0) as u32;
    let lanes = m.get("lanes").map(|x| f(x, &format!("{path}.lanes"))).transpose()?.unwrap_or(1.0).max(1.0) as u32;
    if count as u64 * lanes as u64 > 200_000 {
        return Err(format!("{path}: {} instances is too many (max 200000)", count as u64 * lanes as u64));
    }
    let shadow = m.get("shadow").and_then(|x| x.as_bool()).unwrap_or(true);
    let start = gf("start", 0.0)?;
    let end = m.get("end").map(|x| f(x, &format!("{path}.end"))).transpose()?.unwrap_or(f32::INFINITY);
    let scale_mul = if ty == "sphere" { extent } else { 1.0 };
    let extent = if ty == "sphere" { 1.0 } else { extent };
    Ok(ObjDef { shape, mat, pos, rot, scale: scale_n, keys, spin, count, lanes, tex, thin, shadow, extent, start, end, scale_mul })
}

fn texture(v: &Value, path: &str) -> Res<TexSpec> {
    let mut t = TexSpec { comp: String::new(), frame: None, offset: 0.0, speed: 1.0, looped: true, face: Face::Front };
    match v {
        Value::String(s) => t.comp = s.trim_start_matches('#').to_string(),
        Value::Object(m) => {
            check_keys(m, &["composition", "frame", "offset", "speed", "loop", "face"], path)?;
            t.comp = m.get("composition").and_then(|c| c.as_str()).ok_or_else(|| format!("{path}: missing `composition` (a name from the spec's `compositions`)"))?.trim_start_matches('#').to_string();
            if let Some(fr) = m.get("frame") {
                t.frame = Some(num(fr, &format!("{path}.frame"))?);
            }
            if let Some(x) = m.get("offset") {
                t.offset = f(x, &format!("{path}.offset"))?;
            }
            if let Some(x) = m.get("speed") {
                t.speed = f(x, &format!("{path}.speed"))?;
            }
            if let Some(x) = m.get("loop") {
                t.looped = x.as_bool().ok_or_else(|| format!("{path}.loop: expected true or false"))?;
            }
            if let Some(x) = m.get("face") {
                t.face = match x.as_str() {
                    Some("front") => Face::Front,
                    Some("back") => Face::Back,
                    Some("both") => Face::Both,
                    _ => return Err(format!("{path}.face: expected `front`, `back` or `both`")),
                };
            }
        }
        _ => return Err(format!("{path}: expected \"#composition\" or {{ \"composition\": … }}")),
    }
    Ok(t)
}

// ------------------------------------------------------------ camera

#[derive(Clone, Debug)]
pub struct CamKey {
    pub t: f32,
    pub pos: V3,
    pub target: V3,
    /// Vertical field of view (degrees) or focal length (mm, 36 mm-wide sensor).
    pub fov: Option<f32>,
    pub focal: Option<f32>,
    /// Focus distance (`None` = the target).
    pub focus: Option<f32>,
    pub fstop: Option<f32>,
    /// Blur radius in px at 1080p for points at infinity (overrides `fstop`).
    pub aperture: Option<f32>,
    pub shutter: Option<f32>,
    pub roll: f32,
}

#[derive(Clone, Debug)]
pub struct CamSpec {
    pub keys: Vec<CamKey>,
    pub linear: bool,
    /// Ease the whole move in and out (smooth mode only).
    pub ease: bool,
    /// Handheld shake: amplitude (m) and frequency (Hz).
    pub shake: (f32, f32),
}

const CAM_KEYS: &[&str] = &["t", "at", "pos", "target", "fov", "focal", "focus", "fstop", "aperture", "shutter", "roll"];

fn cam_key(v: &Value, path: &str) -> Res<CamKey> {
    let m = v.as_object().ok_or_else(|| format!("{path}: expected a camera key object"))?;
    check_keys(m, CAM_KEYS, path)?;
    let v3c = |k: &str| -> Res<V3> {
        let p = format!("{path}.{k}");
        let a = m.get(k).and_then(|x| x.as_array()).filter(|a| a.len() == 3).ok_or_else(|| format!("{p}: expected [x, y, z]"))?;
        Ok(v3(f(&a[0], &p)?, f(&a[1], &p)?, f(&a[2], &p)?))
    };
    let of = |k: &str| -> Res<Option<f32>> { m.get(k).filter(|x| !x.is_null()).map(|x| f(x, &format!("{path}.{k}"))).transpose() };
    Ok(CamKey {
        t: of("t")?.or(of("at")?).unwrap_or(0.0),
        pos: v3c("pos")?,
        target: v3c("target")?,
        fov: of("fov")?,
        focal: of("focal")?,
        focus: of("focus")?,
        fstop: of("fstop")?,
        aperture: of("aperture")?,
        shutter: of("shutter")?,
        roll: of("roll")?.unwrap_or(0.0),
    })
}

/// `camera`: an array of keys, `{ keys, interp, ease, shake }`, or a preset
/// (`orbit`, `dolly`, `crane`, `static`, or `{ "preset": "orbit", … }`)
/// framed on the scene bounds `(centre, radius)`.
pub fn camera(v: &Value, bounds: (V3, f32), path: &str) -> Res<CamSpec> {
    let (c, r) = bounds;
    let r = r.max(0.3);
    let preset = |name: &str, dur: f32, amount: f32, height: f32| -> Res<CamSpec> {
        let dist = r * 2.9;
        let tgt = c;
        let at = |ang: f32, d: f32, y: f32| v3(c.x + ang.to_radians().sin() * d, c.y + y, c.z + ang.to_radians().cos() * d);
        let key = |t: f32, pos: V3| CamKey { t, pos, target: tgt, fov: Some(30.0), focal: None, focus: None, fstop: Some(4.0), aperture: None, shutter: None, roll: 0.0 };
        let keys = match name {
            "orbit" => (0..=8).map(|k| k as f32 / 8.0).map(|u| key(u * dur, at(-amount * 0.5 + amount * u, dist, r * height))).collect(),
            "dolly" => vec![key(0.0, at(-8.0, dist * 1.25, r * height)), key(dur, at(4.0, dist * 0.85, r * height * 0.8))],
            "crane" => vec![key(0.0, at(-10.0, dist, r * 0.15)), key(dur, at(10.0, dist * 0.95, r * height * 2.2))],
            "static" => vec![key(0.0, at(-12.0, dist, r * height))],
            other => {
                let hint = crate::components::style::closest(other, &["orbit", "dolly", "crane", "static"]).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(format!("{path}: unknown camera preset `{other}`{hint}"));
            }
        };
        Ok(CamSpec { keys, linear: name == "orbit", ease: name != "orbit", shake: (0.0, 0.0) })
    };
    match v {
        Value::Null => preset("orbit", 8.0, 50.0, 0.45),
        Value::String(s) => preset(s, 8.0, 50.0, 0.45),
        Value::Array(a) => {
            let keys: Vec<CamKey> = a.iter().enumerate().map(|(i, k)| cam_key(k, &format!("{path}[{i}]"))).collect::<Res<_>>()?;
            if keys.is_empty() {
                return Err(format!("{path}: at least one key"));
            }
            Ok(CamSpec { keys, linear: false, ease: true, shake: (0.0, 0.0) })
        }
        Value::Object(m) => {
            check_keys(m, &["keys", "interp", "ease", "shake", "preset", "duration", "degrees", "height"], path)?;
            let mut spec = if let Some(p) = m.get("preset").and_then(|p| p.as_str()) {
                let dur = m.get("duration").and_then(|x| x.as_f64()).unwrap_or(8.0) as f32;
                let deg = m.get("degrees").and_then(|x| x.as_f64()).unwrap_or(50.0) as f32;
                let hgt = m.get("height").and_then(|x| x.as_f64()).unwrap_or(0.45) as f32;
                preset(p, dur, deg, hgt)?
            } else {
                let a = m.get("keys").and_then(|k| k.as_array()).ok_or_else(|| format!("{path}: expected `keys` or `preset`"))?;
                let keys: Vec<CamKey> = a.iter().enumerate().map(|(i, k)| cam_key(k, &format!("{path}.keys[{i}]"))).collect::<Res<_>>()?;
                if keys.is_empty() {
                    return Err(format!("{path}.keys: at least one key"));
                }
                CamSpec { keys, linear: false, ease: false, shake: (0.0, 0.0) }
            };
            if let Some(i) = m.get("interp") {
                spec.linear = match i.as_str() {
                    Some("linear") => true,
                    Some("smooth") => false,
                    _ => return Err(format!("{path}.interp: expected `linear` or `smooth`")),
                };
            }
            if let Some(e) = m.get("ease") {
                spec.ease = e.as_bool().ok_or_else(|| format!("{path}.ease: expected true or false"))?;
            }
            match m.get("shake") {
                None => {}
                Some(Value::Number(n)) => spec.shake = (n.as_f64().unwrap_or(0.0) as f32, 0.7),
                Some(Value::Object(s)) => spec.shake = (s.get("amount").and_then(|x| x.as_f64()).unwrap_or(0.01) as f32, s.get("frequency").and_then(|x| x.as_f64()).unwrap_or(0.7) as f32),
                Some(_) => return Err(format!("{path}.shake: expected a number (metres) or {{amount, frequency}}")),
            }
            Ok(spec)
        }
        _ => Err(format!("{path}: expected keys, an object or a preset name")),
    }
}

impl CamSpec {
    /// Camera pose and lens at time `t` for an image of `aspect`.
    pub fn at(&self, t: f32, aspect: f32, default_aperture: f32) -> super::trace::Camera {
        let ks = &self.keys;
        let (t0, t1) = (ks[0].t, ks[ks.len() - 1].t);
        let t = if self.ease && t1 > t0 {
            let u = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
            t0 + (t1 - t0) * (u * u * (3.0 - 2.0 * u) * 0.6 + u * 0.4)
        } else {
            t
        };
        // Binary search for the segment (dense per-frame tracks are fine).
        let mut i = ks.partition_point(|k| k.t <= t).saturating_sub(1);
        i = i.min(ks.len().saturating_sub(2));
        let a = &ks[i];
        let b = &ks[(i + 1).min(ks.len() - 1)];
        let s = if b.t > a.t { ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0) } else { 0.0 };
        let (pos, target) = if self.linear || ks.len() < 3 {
            (lerp3(a.pos, b.pos, s), lerp3(a.target, b.target, s))
        } else {
            let pm = &ks[i.saturating_sub(1)];
            let pn = &ks[(i + 2).min(ks.len() - 1)];
            let cr = |p0: V3, p1: V3, p2: V3, p3: V3| -> V3 {
                let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (-a + c) * s + (2.0 * a - 5.0 * b + 4.0 * c - d) * s * s + (-a + 3.0 * b - 3.0 * c + d) * s * s * s);
                v3(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y), f(p0.z, p1.z, p2.z, p3.z))
            };
            (cr(pm.pos, a.pos, b.pos, pn.pos), cr(pm.target, a.target, b.target, pn.target))
        };
        let ss = if self.linear { s } else { s * s * (3.0 - 2.0 * s) };
        let mix = |x: Option<f32>, y: Option<f32>| -> Option<f32> {
            match (x, y) {
                (Some(x), Some(y)) => Some(x + (y - x) * ss),
                (Some(x), None) | (None, Some(x)) => Some(x),
                _ => None,
            }
        };
        let vfov_of = |k: &CamKey| -> Option<f32> {
            k.fov.or_else(|| k.focal.map(|fm| {
                let hf = 2.0 * (18.0 / fm.max(1.0)).atan();
                (2.0 * ((hf * 0.5).tan() / aspect).atan()).to_degrees()
            }))
        };
        let fov = mix(vfov_of(a), vfov_of(b)).unwrap_or(30.0);
        let dist_t = len(sub(target, pos));
        let focus = a.focus.unwrap_or(dist_t) + (b.focus.unwrap_or(dist_t) - a.focus.unwrap_or(dist_t)) * ss;
        // Aperture: explicit px, or from the f-stop and focal length (thin lens,
        // 36 mm-wide sensor), as a blur radius at 1080p for points at infinity.
        let aperture = match mix(a.aperture, b.aperture) {
            Some(px) => px,
            None => match mix(a.fstop, b.fstop) {
                Some(n) => {
                    let hf = 2.0 * ((fov.to_radians() * 0.5).tan() * aspect).atan();
                    let fm = 18.0 / (hf * 0.5).tan();
                    let s_mm = (focus * 1000.0).max(fm * 1.01);
                    let coc_mm = fm * fm / (n.max(0.5) * (s_mm - fm));
                    0.5 * coc_mm / 36.0 * 1080.0 * aspect
                }
                None => default_aperture,
            },
        };
        let shutter = mix(a.shutter, b.shutter).unwrap_or(0.5);
        let roll = a.roll + (b.roll - a.roll) * ss;
        let (mut pos, mut target) = (pos, target);
        if self.shake.0 > 0.0 {
            let n = |k: f32| {
                let x = t * self.shake.1 + k;
                (x * 1.0).sin() * 0.5 + (x * 2.31 + 1.3).sin() * 0.3 + (x * 4.17 + 2.1).sin() * 0.2
            };
            let a = self.shake.0;
            pos = add(pos, v3(n(0.0) * a, n(5.1) * a, n(9.7) * a * 0.5));
            target = add(target, v3(n(3.3) * a * 0.6, n(7.7) * a * 0.6, 0.0));
        }
        super::trace::Camera { pos, target, fov, focus, aperture, roll, shutter }
    }
}

// ------------------------------------------------------------ environment

pub struct EnvSpec {
    pub env: Env,
    pub name: String,
    pub tint: V3,
    pub haze: f32,
}

pub fn env(v: &Value, path: &str) -> Res<EnvSpec> {
    let accent = srgb_lin(0.486, 0.361, 1.0);
    let accent2 = srgb_lin(0.133, 0.827, 0.933);
    let names = ["cyc", "studio", "night", "neon", "sunset"];
    let (name, tint, key, haze) = match v {
        Value::Null => ("cyc".to_string(), srgb_lin(0.933, 0.922, 0.898), None, None),
        Value::String(s) => (s.clone(), srgb_lin(0.933, 0.922, 0.898), None, None),
        Value::Object(m) => {
            check_keys(m, &["preset", "color", "key", "haze"], path)?;
            let name = m.get("preset").and_then(|p| p.as_str()).unwrap_or("cyc").to_string();
            let tint = m.get("color").map(|c| color(c, &format!("{path}.color"))).transpose()?.unwrap_or(srgb_lin(0.933, 0.922, 0.898));
            let key = m
                .get("key")
                .map(|k| {
                    let a = k.as_array().filter(|a| a.len() == 3).ok_or_else(|| format!("{path}.key: expected a direction [x, y, z]"))?;
                    Ok::<V3, String>(norm(v3(f(&a[0], path)?, f(&a[1], path)?, f(&a[2], path)?)))
                })
                .transpose()?;
            let haze = m.get("haze").map(|h| f(h, &format!("{path}.haze"))).transpose()?;
            (name, tint, key, haze)
        }
        _ => return Err(format!("{path}: expected a preset name or object")),
    };
    let name = crate::components::kebab(&name);
    let canon = match name.as_str() {
        "white" | "white-cyc" | "cyclorama" => "cyc".to_string(),
        n => n.to_string(),
    };
    let e = Env::preset_tinted(&canon, accent, accent2, tint, key.unwrap_or(norm(v3(-0.55, 1.0, 0.45)))).ok_or_else(|| {
        let hint = crate::components::style::closest(&name, &names).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
        format!("{path}: unknown env `{name}`{hint}")
    })?;
    let haze = haze.unwrap_or(if canon == "cyc" { 0.0015 } else { 0.0 });
    Ok(EnvSpec { env: e, name: canon, tint, haze })
}
