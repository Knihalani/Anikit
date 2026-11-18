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

pub fn compile(src: &str) -> Res<Num> {
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

pub fn num(v: &Value, path: &str) -> Res<Num> {
    match v {
        Value::Number(n) => Ok(Num::C(n.as_f64().unwrap_or(0.0) as f32)),
        Value::String(s) => compile(s.strip_prefix('=').unwrap_or(s)),
        _ => Err(format!("{path}: expected a number or an \"=expression\"")),
    }
}

pub fn f(v: &Value, path: &str) -> Res<f32> {
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

pub fn check_keys(m: &Map<String, Value>, allowed: &[&str], path: &str) -> Res<()> {
    for k in m.keys() {
        if !allowed.contains(&k.as_str()) {
            let hint = crate::components::style::closest(k, allowed).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(format!("{path}: unknown key `{k}`{hint}"));
        }
    }
    Ok(())
}

// ------------------------------------------------------------ materials

pub const MATERIALS: &[&str] = &["chrome", "mirror", "glass", "aluminium", "aluminum", "aluminium-bead", "titanium", "copper", "gold", "matte", "white", "black", "plastic", "ceramic", "rubber", "car-paint", "lacquer", "emissive", "decal", "toon", "clay", "paper", "cardboard", "gouache", "hatch"];

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
        "mirror" => Material { base: v3(0.97, 0.97, 0.97), roughness: 0.02, ..c },
        "titanium" => Material { base: v3(0.62, 0.6, 0.57), metallic: 1.0, roughness: 0.3, ..c },
        "copper" => Material { base: v3(0.96, 0.64, 0.54), metallic: 1.0, roughness: 0.18, ..c },
        // Deep red base under a glossy clear coat.
        "car-paint" => Material { base: v3(0.45, 0.02, 0.03), metallic: 0.3, roughness: 0.35, clearcoat: 1.0, ..c },
        // Glossy black piano lacquer.
        "lacquer" => Material { base: v3(0.012, 0.012, 0.014), metallic: 0.0, roughness: 0.3, clearcoat: 1.0, ..c },
        "emissive" => Material { base: v3(0.02, 0.02, 0.02), metallic: 0.0, roughness: 0.4, emissive: v3(4.0, 4.0, 4.0), ..c },
        // A print on another surface: a clear, index-1 layer (no reflection, no
        // refraction) that shows only its texture. Use it on a thin `plane`.
        "decal" => Material { base: v3(1.0, 1.0, 1.0), metallic: 0.0, roughness: 0.0, transmission: 1.0, ior: 1.0, absorb: v3(0.0, 0.0, 0.0), tex_glow: 0.0, ..c },
        // Non-photoreal styles (see `Material::style`); `base` sets the colour.
        "toon" => Material { base: v3(0.85, 0.32, 0.22), metallic: 0.0, roughness: 0.3, style: 1, ..c },
        "clay" => Material { base: v3(0.72, 0.45, 0.33), metallic: 0.0, roughness: 0.7, style: 2, ..c },
        "paper" => Material { base: v3(0.86, 0.84, 0.79), metallic: 0.0, roughness: 1.0, style: 3, ..c },
        "cardboard" => Material { base: v3(0.55, 0.4, 0.26), metallic: 0.0, roughness: 1.0, style: 3, ..c },
        "gouache" => Material { base: v3(0.25, 0.45, 0.7), metallic: 0.0, roughness: 1.0, style: 4, ..c },
        "hatch" => Material { base: v3(0.9, 0.88, 0.83), metallic: 0.0, roughness: 1.0, style: 5, ..c },
        _ => return None,
    })
}

const MAT_KEYS: &[&str] = &["base", "color", "metallic", "roughness", "specular", "emissive", "emissive_strength", "clearcoat", "transmission", "ior", "tint", "tint_distance", "glow", "extends", "texture", "scale", "mapping", "normal_strength", "texture_rotate", "imperfections", "roughness_variation", "smudges", "scratches", "dust", "pattern", "pattern_scale", "pattern_strength"];

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
            if let Some(t) = m.get("texture") {
                let src = t.as_str().ok_or_else(|| format!("{path}.texture: expected an asset ref (`ambientcg:Wood049`, `polyhaven:texture/…`) or a folder of maps"))?;
                let o = super::pbr::SetOpts {
                    scale: m.get("scale").map(|x| f(x, &format!("{path}.scale"))).transpose()?.unwrap_or(1.0).max(1e-3),
                    tint: match m.get("base").or(m.get("color")) {
                        Some(c) => color(c, &format!("{path}.color"))?,
                        None => v3(1.0, 1.0, 1.0),
                    },
                    mapping: match m.get("mapping").and_then(|x| x.as_str()) {
                        None | Some("triplanar") | Some("box") => super::pbr::Mapping::Triplanar,
                        Some("uv") => super::pbr::Mapping::Uv,
                        Some(x) => return Err(format!("{path}.mapping: `{x}` (triplanar or uv)")),
                    },
                    rough_mul: 1.0,
                    normal_strength: m.get("normal_strength").map(|x| f(x, &format!("{path}.normalStrength"))).transpose()?.unwrap_or(1.0),
                    keep_rough: m.contains_key("roughness"),
                    keep_metal: m.contains_key("metallic"),
                    rotate: m.get("texture_rotate").map(|x| f(x, &format!("{path}.textureRotate"))).transpose()?.unwrap_or(0.0),
                    max: 2048,
                };
                out.pbr = super::pbr::load(src, &o).map_err(|e| format!("{path}.texture: {e}"))?;
                // The tint went into the set; the base multiplies it.
                out.base = v3(1.0, 1.0, 1.0);
            }
            // Machined patterns (bump): brushed, sunburst, guilloche, clous-de-paris, knurl.
            if let Some(pt) = m.get("pattern") {
                let name = pt.as_str().ok_or_else(|| format!("{path}.pattern: expected a pattern name"))?;
                let names = ["brushed", "sunburst", "guilloche", "clous-de-paris", "knurl"];
                let k = crate::components::kebab(name);
                let kind = match k.as_str() {
                    "brushed" => 1,
                    "sunburst" => 2,
                    "guilloche" | "guilloché" => 3,
                    "clous-de-paris" | "hobnail" => 4,
                    "knurl" | "knurled" => 5,
                    o => {
                        let hint = crate::components::style::closest(o, &names).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                        return Err(format!("{path}.pattern: unknown pattern `{o}`{hint}"));
                    }
                };
                let sc = m.get("pattern_scale").map(|x| f(x, &format!("{path}.patternScale"))).transpose()?.unwrap_or(0.01).max(1e-5);
                let st = m.get("pattern_strength").map(|x| f(x, &format!("{path}.patternStrength"))).transpose()?.unwrap_or(1.0).max(0.0);
                out.pattern = (kind, sc, st);
            }
            // Imperfections: one amount (a believable mix), or each by name.
            if let Some(x) = m.get("imperfections") {
                let a = f(x, &format!("{path}.imperfections"))?.clamp(0.0, 1.0);
                out.imperf = [a, a * 0.6, a * 0.5, a * 0.25];
            }
            for (k, i) in [("roughness_variation", 0), ("smudges", 1), ("scratches", 2), ("dust", 3)] {
                if let Some(x) = m.get(k) {
                    out.imperf[i] = f(x, &format!("{path}.{k}"))?.clamp(0.0, 1.0);
                }
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

/// An animated glTF model: per-part nodes, the fit to the requested size,
/// and playback speed (the animation loops).
pub struct ModelAnim {
    pub anim: super::gltf::Anim,
    pub nodes: Vec<usize>,
    pub k: f32,
    pub centre: V3,
    pub speed: f32,
}

impl ModelAnim {
    /// Each part's transform at time `t`, composed after `obj`.
    pub fn parts_at(&self, obj: &Xf, t: f32) -> Vec<Xf> {
        let d = self.anim.duration;
        let tt = if d > 0.0 { (t * self.speed).rem_euclid(d) } else { 0.0 };
        let world = self.anim.world(tt);
        // Object (with fit) as columns: R·S, then translation.
        let fit = |p: V3| v3((p.x - self.centre.x) * self.k, (p.y - self.centre.y) * self.k, (p.z - self.centre.z) * self.k);
        self.nodes
            .iter()
            .map(|&n| {
                let w = &world[n];
                // Columns of fit ∘ W: W's columns scaled by k; origin fitted.
                let cols = [v3(w[0][0], w[0][1], w[0][2]), v3(w[1][0], w[1][1], w[1][2]), v3(w[2][0], w[2][1], w[2][2])].map(|c| scale(c, self.k));
                let org = fit(v3(w[3][0], w[3][1], w[3][2]));
                // Then the object: o = R_o·(S_o∘·) + t_o.
                let cols = cols.map(|c| obj.rot(mul(c, obj.s)));
                let org = obj.point(org);
                // Decompose (orthonormalise; no shear).
                let sx = len(cols[0]).max(1e-9);
                let x = scale(cols[0], 1.0 / sx);
                let y0 = sub(cols[1], scale(x, dot(cols[1], x)));
                let sy = len(cols[1]).max(1e-9);
                let y = norm(y0);
                let z = cross(x, y);
                let sz = len(cols[2]).max(1e-9) * if dot(z, cols[2]) < 0.0 { -1.0 } else { 1.0 };
                Xf { r: [x, y, z], s: v3(sx, sy, sz), t: org }
            })
            .collect()
    }
}

/// A transform key.
#[derive(Clone, Debug)]
pub struct TKey {
    pub t: f32,
    pub pos: Option<V3>,
    pub rot: Option<V3>,
    pub scale: Option<V3>,
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
    /// Open geometry that reads `t`: (type, description, path, shape id), rebuilt every frame.
    pub regen: Option<Arc<(String, Map<String, Value>, String, u32)>>,
    /// (shape, material) per part: one for primitives, one per glTF primitive for models.
    pub parts: Vec<(u32, u32)>,
    pub pos: [Num; 3],
    pub rot: [Num; 3],
    pub scale: [Num; 3],
    pub keys: Vec<TKey>,
    /// Degrees per second about X, Y, Z.
    pub spin: V3,
    pub count: u32,
    pub lanes: u32,
    pub tex: Vec<TexSpec>,
    /// Visible to the camera (false: a reflector card).
    pub cam: bool,
    /// For camera targeting.
    pub name: Option<String>,
    /// glTF node animation for a model's parts.
    pub anim: Option<Arc<ModelAnim>>,
    pub thin: bool,
    pub shadow: bool,
    /// Half width of the shape in metres (texture LOD).
    pub extent: f32,
    /// Visible from / until (seconds).
    pub start: f32,
    pub end: f32,
    /// Shape size folded into the scale (a sphere's radius).
    pub scale_mul: f32,
    /// Collision shape (object space, before scale).
    pub col: Col,
    /// Takes part in `physics` as a fixed or kinematic collider.
    pub collide: bool,
    /// Transforms come from a physics simulation: the sim and this object's first body.
    pub phys: Option<(Arc<super::physics::Sim>, u32)>,
}

/// A collision shape in object space (scaled by the instance scale).
#[derive(Clone, Debug)]
pub enum Col {
    Cuboid { c: V3, h: V3 },
    /// The unit sphere.
    Ball,
    Cylinder { c: V3, hh: f32, r: f32 },
    Hull(Arc<Vec<V3>>),
}

impl ObjDef {
    pub fn dynamic(&self) -> bool {
        self.anim.is_some() || self.regen.is_some() || self.phys.is_some() || !self.keys.is_empty() || self.spin != v3(0.0, 0.0, 0.0) || self.pos.iter().chain(self.rot.iter()).chain(self.scale.iter()).any(|n| n.dynamic()) || self.start > 0.0 || self.end.is_finite()
    }

    pub fn xf_at(&self, t: f32, i: u32, lane: u32) -> Xf {
        let vars = Vars { t: t as f64, scene_t: lane as f64, gt: t as f64, i: i as f64, n: self.count as f64, ..Vars::default() };
        let mut pos = eval3(&self.pos, &vars);
        let mut rot = eval3(&self.rot, &vars);
        let mut sc = eval3(&self.scale, &vars);
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
            if let Some(s) = pick(&|k| k.scale) {
                sc = mul(sc, s);
            }
        }
        rot = madd(rot, self.spin, t);
        Xf::new(pos, rot, vmax(scale(sc, self.scale_mul), v3(1e-6, 1e-6, 1e-6)))
    }
}

pub const OBJ_KEYS: &[&str] = &[
    "type", "src", "text", "font", "weight", "tracking", "height", "depth", "bevel", "size", "radius", "tube", "material", "pos", "position", "rot", "rotation", "scale", "keys", "spin", "count", "lanes", "texture", "thin", "shadow", "start", "end", "name", "camera", "align", "color", "intensity", "speed", "collide",
    "d", "path", "points", "segments", "sides", "x", "y", "z", "u", "v", "expr", "res", "vertices", "faces", "flat", "smooth",
];
const TYPES: &[&str] = &["text", "box", "slab", "pane", "plane", "sphere", "cylinder", "torus", "phone", "model", "card", "extrude", "lathe", "tube", "surface", "heightfield", "mesh", "cloth"];

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

pub fn mesh_shape(m: anikit_core::raster::Mesh) -> Shape {
    if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
        eprintln!("scene3d: mesh with {} triangles", m.idx.len() / 3);
    }
    Shape::Mesh(Arc::new(Bvh::with_uv(&m.pos, &m.nrm, &m.uv, &m.idx)))
}

pub fn object(v: &Value, shapes: &mut Shapes, mats: &mut Vec<Material>, named: &HashMap<String, Material>, path: &str) -> Res<ObjDef> {
    let m = v.as_object().ok_or_else(|| format!("{path}: expected an object"))?;
    if m.get("type").and_then(|t| t.as_str()) == Some("cloth") {
        let all: Vec<&str> = OBJ_KEYS.iter().chain(super::cloth::KEYS).copied().collect();
        check_keys(m, &all, path)?;
    } else {
        check_keys(m, OBJ_KEYS, path)?;
    }
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
        None if m.contains_key("src") => "model".into(),
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
    let mut model_mats: Vec<Material> = Vec::new();
    let mut model_anim: Option<Arc<ModelAnim>> = None;
    let mut extra: Vec<u32> = Vec::new();
    let mut dyn_geom = false;
    let (shape, extent) = match ty.as_str() {
        "text" => {
            let text = m.get("text").and_then(|x| x.as_str()).unwrap_or("ANIKIT").to_string();
            let font = m.get("font").and_then(|x| x.as_str()).unwrap_or("Inter").to_string();
            let (wt, tr, hh) = (gf("weight", 800.0)?, gf("tracking", 0.02)?, gf("height", 1.0)?);
            let (dp, bv) = (gf("depth", hh * 0.34)?, gf("bevel", hh * 0.045)?);
            let align = match m.get("align").and_then(|a| a.as_str()).unwrap_or("center") {
                "left" => -1.0,
                "right" => 1.0,
                "center" | "centre" => 0.0,
                o => return Err(format!("{path}.align: unknown `{o}` (left, center, right)")),
            };
            let s = shapes.add(format!("{key}|{align}"), || geom::text(&text, &font, wt, tr, hh, dp, bv, align).map(mesh_shape).map_err(|e| format!("{path}: {e}")))?;
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
        "card" => {
            // A reflector card: a flat emitter (bright card) or absorber
            // (black flag) that shows only in reflections and lighting.
            let sz = size3([2.0, 1.2, 0.0])?;
            (shapes.add(format!("plane|{}|{}", sz[0], sz[1]), || Ok(mesh_shape(geom::plane(sz[0], sz[1]))))?, sz[0] * 0.5)
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
        "model" => {
            // glTF / GLB: every primitive becomes a part with its own material,
            // fitted to `height` (or the largest side to `size`) and standing
            // on y = 0, centred on x and z.
            let src = m.get("src").and_then(|x| x.as_str()).ok_or_else(|| format!("{path}: a model needs `src` (a .glb or .gltf file)"))?;
            let file = crate::assetlib::resolve_path(std::path::Path::new(""), src).map_err(|e| format!("{path}.src: {e}"))?;
            let (parts, anim) = super::gltf::load(&file).map_err(|e| format!("{path}.src: {e}"))?;
            // Rest-pose bounds (animated files: nodes placed at t = 0).
            let rest = anim.as_ref().map(|a| a.world(0.0));
            let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
            for pt in &parts {
                for q in pt.mesh.pos.chunks_exact(3) {
                    let mut pp = v3(q[0], q[1], q[2]);
                    if let Some(w) = &rest {
                        pp = super::gltf::apply(&w[pt.node], pp);
                    }
                    lo = vmin(lo, pp);
                    hi = vmax(hi, pp);
                }
            }
            let ext = sub(hi, lo);
            let k = match (m.get("height"), m.get("size")) {
                (Some(h), _) => f(h, &format!("{path}.height"))? / ext.y.max(1e-6),
                (None, Some(sz)) => f(sz, &format!("{path}.size"))? / ext.x.max(ext.y).max(ext.z).max(1e-6),
                _ => 1.0,
            };
            let centre = v3((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5);
            let mut first = None;
            let mut nodes = Vec::new();
            for (j, mut pt) in parts.into_iter().enumerate() {
                if anim.is_none() {
                    for q in pt.mesh.pos.chunks_exact_mut(3) {
                        q[0] = (q[0] - centre.x) * k;
                        q[1] = (q[1] - centre.y) * k;
                        q[2] = (q[2] - centre.z) * k;
                    }
                }
                nodes.push(pt.node);
                let id = shapes.add(format!("{key}|{src}|{j}|{k}|{}", anim.is_some()), || Ok(mesh_shape(pt.mesh)))?;
                if first.is_none() {
                    first = Some(id);
                } else {
                    extra.push(id);
                }
                model_mats.push(pt.mat);
            }
            if let Some(a) = anim {
                let speed = gf("speed", 1.0)?;
                model_anim = Some(Arc::new(ModelAnim { anim: a, nodes, k, centre, speed }));
            }
            (first.unwrap(), ext.x.max(ext.z) * k * 0.5)
        }
        "cloth" => {
            // Simulated: rebuilt every frame (two-sided, matte unless told otherwise).
            let mesh = super::cloth::mesh_at(m, path, 0.0)?;
            dyn_geom = true;
            thin = true;
            let gkey = format!("cloth|{}", serde_json::to_string(m).unwrap_or_default());
            let mut r: f32 = 0.0;
            for q in mesh.pos.chunks_exact(3) {
                r = r.max((q[0] * q[0] + q[2] * q[2]).sqrt());
            }
            (shapes.add(gkey, || Ok(mesh_shape(mesh)))?, r)
        }
        "extrude" | "lathe" | "tube" | "surface" | "heightfield" | "mesh" => {
            // Open geometry: deduplicated by its whole description.
            let gkey = format!("{ty}|{}", serde_json::to_string(m).unwrap_or_default());
            let mesh = super::authoring::build(&ty, m, path, 0.0)?.unwrap();
            dyn_geom = super::authoring::is_dynamic(m);
            let mut r: f32 = 0.0;
            for q in mesh.pos.chunks_exact(3) {
                r = r.max((q[0] * q[0] + q[2] * q[2]).sqrt());
            }
            (shapes.add(gkey, || Ok(mesh_shape(mesh)))?, r)
        }
        _ => unreachable!(),
    };
    let mat = match m.get("material") {
        Some(x) => material(x, named, &format!("{path}.material"))?,
        None if ty == "card" => {
            let c = m.get("color").map(|c| color(c, &format!("{path}.color"))).transpose()?.unwrap_or(v3(1.0, 1.0, 1.0));
            let k = gf("intensity", 1.5)?;
            Material { base: v3(0.0, 0.0, 0.0), metallic: 0.0, roughness: 1.0, specular: 0.0, emissive: scale(c, k), ..Material::chrome() }
        }
        None => builtin_material(if ty == "slab" || ty == "pane" { "glass" } else if ty == "cloth" { "matte" } else { "chrome" }).unwrap(),
    };
    // Open shapes have no inside: as glass they are thin shells (a crystal, a
    // dome, a sheet), or rays would enter and never leave.
    let open = match ty.as_str() {
        "plane" | "card" | "surface" | "heightfield" => true,
        "lathe" => m.get("points").and_then(|p| p.as_array()).is_some_and(|a| {
            let r = |v: Option<&Value>| v.and_then(|q| q.get(0)).and_then(|x| x.as_f64()).unwrap_or(0.0).abs();
            r(a.first()) > 1e-4 || r(a.last()) > 1e-4
        }),
        _ => false,
    };
    if open {
        thin = true;
    }
    if let Some(t) = m.get("thin") {
        thin = t.as_bool().ok_or_else(|| format!("{path}.thin: expected true or false"))?;
    }
    // Parts: an explicit material overrides a model's own.
    let mut parts = Vec::new();
    let all: Vec<u32> = std::iter::once(shape).chain(extra.iter().copied()).collect();
    for (j, sh) in all.iter().enumerate() {
        let pm = if m.contains_key("material") || model_mats.is_empty() { mat.clone() } else { model_mats[j].clone() };
        mats.push(pm);
        parts.push((*sh, mats.len() as u32 - 1));
    }
    let zero = || [Num::C(0.0), Num::C(0.0), Num::C(0.0)];
    let pos = m.get("pos").or(m.get("position")).map(|x| vec3(x, &format!("{path}.pos"))).transpose()?.unwrap_or_else(zero);
    let rot = m.get("rot").or(m.get("rotation")).map(|x| vec3(x, &format!("{path}.rot"))).transpose()?.unwrap_or_else(zero);
    // A number (uniform) or [sx, sy, sz]; items may be expressions. Scale 0 hides.
    let scale_n = m.get("scale").map(|x| vec3(x, &format!("{path}.scale"))).transpose()?.unwrap_or([Num::C(1.0), Num::C(1.0), Num::C(1.0)]);
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
                scale: km
                    .get("scale")
                    .map(|x| match x {
                        Value::Number(_) => f(x, &format!("{kp}.scale")).map(|s| v3(s, s, s)),
                        _ => v3c(x, &format!("{kp}.scale")),
                    })
                    .transpose()?,
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
    // One texture, or a list (e.g. a different composition on each face).
    let tex = match m.get("texture") {
        None => Vec::new(),
        Some(Value::Array(a)) => a.iter().enumerate().map(|(j, t)| texture(t, &format!("{path}.texture[{j}]"))).collect::<Res<_>>()?,
        Some(t) => vec![texture(t, &format!("{path}.texture"))?],
    };
    let cam = m.get("camera").map(|x| x.as_bool().ok_or_else(|| format!("{path}.camera: expected true or false"))).transpose()?.unwrap_or(ty != "card");
    let count = m.get("count").map(|x| f(x, &format!("{path}.count"))).transpose()?.unwrap_or(1.0).max(0.0) as u32;
    let lanes = m.get("lanes").map(|x| f(x, &format!("{path}.lanes"))).transpose()?.unwrap_or(1.0).max(1.0) as u32;
    if count as u64 * lanes as u64 > 200_000 {
        return Err(format!("{path}: {} instances is too many (max 200000)", count as u64 * lanes as u64));
    }
    let shadow = m.get("shadow").and_then(|x| x.as_bool()).unwrap_or(ty != "card");
    let start = gf("start", 0.0)?;
    let end = m.get("end").map(|x| f(x, &format!("{path}.end"))).transpose()?.unwrap_or(f32::INFINITY);
    let scale_mul = if ty == "sphere" { extent } else { 1.0 };
    let extent = if ty == "sphere" { 1.0 } else { extent };
    let col = collider_of(&ty, &parts.iter().map(|p| &shapes.list[p.0 as usize]).collect::<Vec<_>>(), path)?;
    let collide = m.get("collide").map(|x| x.as_bool().ok_or_else(|| format!("{path}.collide: expected true or false"))).transpose()?.unwrap_or(false);
    let name = m.get("name").and_then(|x| x.as_str()).map(|s| s.to_string());
    let regen = dyn_geom.then(|| Arc::new((ty.clone(), m.clone(), path.to_string(), shape)));
    Ok(ObjDef { regen, anim: model_anim, parts, cam, name, pos, rot, scale: scale_n, keys, spin, count, lanes, tex, thin, shadow, extent, start, end, scale_mul, col, collide, phys: None })
}

/// The collision shape for an object type: boxes from their bounds, spheres
/// exactly, everything else (text, tori, models) as the convex hull of its meshes.
fn collider_of(ty: &str, parts: &[&Shape], path: &str) -> Res<Col> {
    let shape = parts[0];
    let (lo, hi) = shape.bounds();
    let c = scale(add(lo, hi), 0.5);
    let h = scale(sub(hi, lo), 0.5);
    Ok(match ty {
        "sphere" => Col::Ball,
        "box" | "slab" | "pane" | "phone" | "plane" => Col::Cuboid { c, h: vmax(h, v3(1e-3, 1e-3, 1e-3)) },
        "cylinder" => Col::Cylinder { c, hh: h.y, r: h.x.max(h.z) },
        _ => {
            if parts.len() == 1 && matches!(shape, Shape::Sphere) {
                return Ok(Col::Ball);
            }
            let mut pts = Vec::new();
            for p in parts {
                match p {
                    Shape::Mesh(b) => {
                        for t in &b.tris {
                            pts.extend([t.v0, add(t.v0, t.e1), add(t.v0, t.e2)]);
                        }
                    }
                    Shape::Sphere => {
                        for k in 0..26 {
                            let d = v3((k % 3) as f32 - 1.0, ((k / 3) % 3) as f32 - 1.0, (k / 9) as f32 - 1.0);
                            if len(d) > 0.0 {
                                pts.push(norm(d));
                            }
                        }
                    }
                }
            }
            if pts.is_empty() {
                return Err(format!("{path}: empty mesh can't collide"));
            }
            Col::Hull(Arc::new(pts))
        }
    })
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
    /// Focus on a named object instead (a rack focus between two keys).
    pub focus_obj: Option<String>,
    /// Easing of the move *into* this key (`None`: the camera's default).
    pub ease: Option<u8>,
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
    /// Aim at (and with `follow`, move with) a named object: key targets
    /// (and positions) become offsets from its centre.
    pub target_obj: Option<String>,
    pub follow: bool,
    /// Handheld shake: amplitude (m) and frequency (Hz) of the legacy
    /// positional wobble.
    pub shake: (f32, f32),
    /// Rotational camera noise: (amplitude as a fraction of the frame height,
    /// base frequency Hz, roll amplitude in degrees, vertical bob in metres).
    pub noise: (f32, f32, f32, f32),
}

/// Camera eases: how a move arrives at a key.
pub const CAM_EASES: &[&str] = &["linear", "in", "out", "inOut", "inExpo", "outExpo", "inOutExpo", "settle"];

fn cam_ease(k: u8, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    match k {
        0 => x,
        1 => x * x * x,
        2 => 1.0 - (1.0 - x).powi(3),
        3 => if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 },
        4 => if x <= 0.0 { 0.0 } else { 2f32.powf(10.0 * x - 10.0) },
        5 => if x >= 1.0 { 1.0 } else { 1.0 - 2f32.powf(-10.0 * x) },
        6 => {
            if x <= 0.0 || x >= 1.0 {
                x
            } else if x < 0.5 {
                2f32.powf(20.0 * x - 10.0) / 2.0
            } else {
                (2.0 - 2f32.powf(-20.0 * x + 10.0)) / 2.0
            }
        }
        // Settle: out-cubic that lands with a ~1% overshoot, like a dolly braking.
        _ => {
            let c = 0.35;
            1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2)
        }
    }
}

const CAM_KEYS: &[&str] = &["t", "at", "pos", "target", "fov", "focal", "focus", "fstop", "aperture", "shutter", "roll", "ease"];

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
        // Optional when the camera targets a named object (then it's an offset).
        target: if m.contains_key("target") { v3c("target")? } else { v3(0.0, 0.0, 0.0) },
        fov: of("fov")?,
        focal: of("focal")?,
        focus: if m.get("focus").is_some_and(|x| x.is_string()) { None } else { of("focus")? },
        focus_obj: m.get("focus").and_then(|x| x.as_str()).map(|x| x.to_string()),
        ease: match m.get("ease").and_then(|e| e.as_str()) {
            None => None,
            Some(n) => Some(CAM_EASES.iter().position(|e| *e == n).ok_or_else(|| {
                let hint = crate::components::style::closest(n, CAM_EASES).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                format!("{path}.ease: unknown ease `{n}`{hint} ({})", CAM_EASES.join(", "))
            })? as u8),
        },
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
        let key = |t: f32, pos: V3| CamKey { t, pos, target: tgt, fov: Some(30.0), focal: None, focus: None, focus_obj: None, ease: None, fstop: Some(4.0), aperture: None, shutter: None, roll: 0.0 };
        let keys = match name {
            "orbit" => (0..=8).map(|k| k as f32 / 8.0).map(|u| key(u * dur, at(-amount * 0.5 + amount * u, dist, r * height))).collect(),
            "dolly" => vec![key(0.0, at(-8.0, dist * 1.25, r * height)), key(dur, at(4.0, dist * 0.85, r * height * 0.8))],
            "crane" => vec![key(0.0, at(-10.0, dist, r * 0.15)), key(dur, at(10.0, dist * 0.95, r * height * 2.2))],
            "static" => vec![key(0.0, at(-12.0, dist, r * height))],
            // Energetic moves: a fast push in, a low-to-high reveal, a lateral fly-by.
            "push" => vec![key(0.0, at(-6.0, dist * 1.7, r * height * 1.2)), key(dur * 0.55, at(-2.0, dist * 0.85, r * height)), key(dur, at(0.0, dist * 0.72, r * height * 0.9))],
            "reveal" => vec![key(0.0, at(-25.0, dist * 0.55, r * 0.05)), key(dur, at(8.0, dist * 1.05, r * height * 1.8))],
            "flyby" => vec![key(0.0, at(-amount, dist * 0.8, r * height * 0.7)), key(dur, at(amount, dist * 0.8, r * height * 0.7))],
            // Product-film moves: a lateral slide with parallax (camera and aim
            // travel together), a slow 30° arc, a pedestal rise over the subject,
            // and an overhead flat-lay that turns slowly.
            "slide" => {
                let side = v3(r * 0.9, 0.0, 0.0);
                let p0 = at(-4.0, dist, r * height);
                vec![CamKey { target: sub(tgt, side), ..key(0.0, sub(p0, side)) }, CamKey { target: add(tgt, side), ..key(dur, add(p0, side)) }]
            }
            "arc" => (0..=4).map(|k| k as f32 / 4.0).map(|u| key(u * dur, at(-15.0 + 30.0 * u, dist, r * height))).collect(),
            "rise" => vec![key(0.0, at(-6.0, dist * 0.95, -r * 0.2)), key(dur, at(-2.0, dist, r * height * 2.4))],
            "topdown" => (0..=4)
                .map(|k| k as f32 / 4.0)
                .map(|u| {
                    let a = (u * 20.0 - 10.0).to_radians();
                    CamKey { roll: u * 20.0 - 10.0, ..key(u * dur, v3(c.x + a.sin() * 0.01, c.y + dist * 1.1, c.z + a.cos() * 0.01)) }
                })
                .collect(),
            other => {
                let hint = crate::components::style::closest(other, &["orbit", "dolly", "crane", "static", "push", "reveal", "flyby", "slide", "arc", "rise", "topdown"]).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(format!("{path}: unknown camera preset `{other}`{hint}"));
            }
        };
        Ok(CamSpec { target_obj: None, follow: false, keys, linear: matches!(name, "orbit" | "flyby"), ease: !matches!(name, "orbit" | "flyby"), shake: (0.0, 0.0), noise: (0.0, 0.0, 0.0, 0.0) })
    };
    match v {
        // Defaults are brisk: 5 s, 70° for an orbit.
        Value::Null => preset("orbit", 5.0, 70.0, 0.45),
        Value::String(s) => preset(&crate::components::kebab(s), 5.0, 70.0, 0.45),
        Value::Array(a) => {
            let keys: Vec<CamKey> = a.iter().enumerate().map(|(i, k)| cam_key(k, &format!("{path}[{i}]"))).collect::<Res<_>>()?;
            if keys.is_empty() {
                return Err(format!("{path}: at least one key"));
            }
            Ok(CamSpec { target_obj: None, follow: false, keys, linear: false, ease: true, shake: (0.0, 0.0), noise: (0.0, 0.0, 0.0, 0.0) })
        }
        Value::Object(m) => {
            check_keys(m, &["keys", "interp", "ease", "shake", "preset", "duration", "degrees", "height", "target", "follow", "lens"], path)?;
            let mut spec = if let Some(p) = m.get("preset").and_then(|p| p.as_str()) {
                let dur = m.get("duration").and_then(|x| x.as_f64()).unwrap_or(5.0) as f32;
                let deg = m.get("degrees").and_then(|x| x.as_f64()).unwrap_or(70.0) as f32;
                let hgt = m.get("height").and_then(|x| x.as_f64()).unwrap_or(0.45) as f32;
                preset(p, dur, deg, hgt)?
            } else {
                let a = m.get("keys").and_then(|k| k.as_array()).ok_or_else(|| format!("{path}: expected `keys` or `preset`"))?;
                let keys: Vec<CamKey> = a.iter().enumerate().map(|(i, k)| cam_key(k, &format!("{path}.keys[{i}]"))).collect::<Res<_>>()?;
                if keys.is_empty() {
                    return Err(format!("{path}.keys: at least one key"));
                }
                CamSpec { target_obj: None, follow: false, keys, linear: false, ease: false, shake: (0.0, 0.0), noise: (0.0, 0.0, 0.0, 0.0) }
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
            if let Some(t) = m.get("target") {
                spec.target_obj = Some(t.as_str().ok_or_else(|| format!("{path}.target: expected an object name"))?.to_string());
                spec.follow = m.get("follow").and_then(|f| f.as_bool()).unwrap_or(false);
            }
            match m.get("shake") {
                None => {}
                Some(Value::Number(n)) => spec.shake = (n.as_f64().unwrap_or(0.0) as f32, 0.7),
                // Rotational noise presets, sized as a fraction of the frame (so
                // they read the same at any focal length): a locked-off camera
                // that breathes, handheld, shoulder rig, running.
                Some(Value::String(name)) => {
                    spec.noise = match name.as_str() {
                        "breath" | "locked" => (0.0015, 0.3, 0.04, 0.0),
                        "handheld" => (0.006, 0.8, 0.35, 0.002),
                        "shoulder" => (0.004, 0.55, 0.2, 0.001),
                        "run" | "running" => (0.018, 1.9, 1.2, 0.025),
                        o => {
                            let hint = crate::components::style::closest(o, &["breath", "handheld", "shoulder", "run"]).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                            return Err(format!("{path}.shake: unknown shake `{o}`{hint} (breath, handheld, shoulder, run)"));
                        }
                    }
                }
                Some(Value::Object(s)) if s.contains_key("frame") || s.contains_key("roll") => {
                    let g = |k: &str, d: f64| s.get(k).and_then(|x| x.as_f64()).unwrap_or(d) as f32;
                    spec.noise = (g("frame", 0.004), g("frequency", 0.7), g("roll", 0.2), g("bob", 0.0));
                }
                Some(Value::Object(s)) => spec.shake = (s.get("amount").and_then(|x| x.as_f64()).unwrap_or(0.01) as f32, s.get("frequency").and_then(|x| x.as_f64()).unwrap_or(0.7) as f32),
                Some(_) => return Err(format!("{path}.shake: expected breath, handheld, shoulder, run, a number (metres) or {{frame, frequency, roll, bob}}")),
            }
            // `lens`: a focal length and its usual working aperture, for keys
            // that set neither.
            if let Some(l) = m.get("lens") {
                let name = l.as_str().ok_or_else(|| format!("{path}.lens: expected a lens name"))?;
                let (fm, n) = match name {
                    "wide24" | "wide" => (24.0, 8.0),
                    "normal35" => (35.0, 5.6),
                    "normal50" | "normal" => (50.0, 4.0),
                    "portrait85" | "portrait" => (85.0, 2.8),
                    "macro100" | "macro" => (100.0, 5.6),
                    "tele135" | "tele" => (135.0, 4.0),
                    "tele200" => (200.0, 4.0),
                    o => {
                        let names = ["wide24", "normal35", "normal50", "portrait85", "macro100", "tele135", "tele200"];
                        let hint = crate::components::style::closest(o, &names).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                        return Err(format!("{path}.lens: unknown lens `{o}`{hint}"));
                    }
                };
                for k in &mut spec.keys {
                    if k.fov.is_none() && k.focal.is_none() {
                        k.focal = Some(fm);
                    }
                    if k.fstop.is_none() && k.aperture.is_none() {
                        k.fstop = Some(n);
                    }
                }
                if m.get("preset").is_some() {
                    // Presets frame with a 30° fov: keep the framing by moving
                    // the camera along its view to match the new focal length.
                    for k in &mut spec.keys {
                        // Vertical half-angle of this lens at 16:9 vs the presets' 15°.
                        let half_v = ((18.0f32 / fm) / (16.0 / 9.0)).atan();
                        let ratio = (15.0f32.to_radians().tan() / half_v.tan()).max(0.2);
                        k.pos = add(k.target, scale(sub(k.pos, k.target), ratio));
                        k.fov = None;
                        k.focal = Some(fm);
                        k.fstop = Some(n);
                    }
                }
            }
            Ok(spec)
        }
        _ => Err(format!("{path}: expected keys, an object or a preset name")),
    }
}

impl CamSpec {
    /// Camera pose and lens at time `t` for an image of `aspect`.
    pub fn at(&self, t: f32, aspect: f32, default_aperture: f32) -> super::trace::Camera {
        self.at_with(t, aspect, default_aperture, &|_| None)
    }

    /// Whether any key focuses on a named object.
    pub fn focuses_on_objects(&self) -> bool {
        self.keys.iter().any(|k| k.focus_obj.is_some())
    }

    /// [`CamSpec::at`], resolving `focus: "name"` keys to the named object's
    /// centre with `resolve`.
    pub fn at_with(&self, t: f32, aspect: f32, default_aperture: f32, resolve: &dyn Fn(&str) -> Option<V3>) -> super::trace::Camera {
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
        // A key's own ease shapes the move into it.
        let s = match b.ease {
            Some(e) => cam_ease(e, s),
            None => s,
        };
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
        let ss = if self.linear || b.ease.is_some() { s } else { s * s * (3.0 - 2.0 * s) };
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
        let kf = |k: &CamKey| -> f32 { k.focus_obj.as_deref().and_then(resolve).map(|c| len(sub(c, pos))).or(k.focus).unwrap_or(dist_t) };
        let focus = kf(a) + (kf(b) - kf(a)) * ss;
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
        let mut roll = roll;
        if self.noise.0 > 0.0 {
            // Layered smooth noise (three bands), as rotation of the view so it
            // reads as a fraction of the frame at any focal length.
            let (amp, fq, roll_deg, bob) = self.noise;
            let n = |seed: u32| -> f32 {
                let band = |f: f32, s: u32| {
                    let x = t * f;
                    let (i, u) = (x.floor(), x - x.floor());
                    let u = u * u * (3.0 - 2.0 * u);
                    let h = |k: f32| u01(hash2(k as i32 as u32, s)) * 2.0 - 1.0;
                    h(i) + (h(i + 1.0) - h(i)) * u
                };
                band(fq, seed) * 0.62 + band(fq * 2.37, seed + 101) * 0.28 + band(fq * 5.9, seed + 211) * 0.1
            };
            let fwd = sub(target, pos);
            let d = len(fwd);
            let fw = scale(fwd, 1.0 / d.max(1e-6));
            let right = norm(cross(fw, v3(0.0, 1.0, 0.0)));
            let up = cross(right, fw);
            // Half the frame height spans fov/2.
            let ang = (fov.to_radians() * 0.5).tan() * 2.0 * amp;
            target = add(target, add(scale(right, n(1) * ang * d * aspect.min(1.5)), scale(up, n(2) * ang * d)));
            roll += n(3) * roll_deg;
            pos = add(pos, scale(up, n(4) * bob));
            target = add(target, scale(up, n(4) * bob));
        }
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
    let names = ["cyc", "studio", "night", "neon", "sunset", "overcast", "interior", "city", "underwater"];
    if let Some(e) = hdri_env(v, path)? {
        return Ok(e);
    }
    let (name, tint, key, haze) = match v {
        Value::Null => ("cyc".to_string(), srgb_lin(0.933, 0.922, 0.898), None, None),
        Value::String(s) => (s.clone(), srgb_lin(0.933, 0.922, 0.898), None, None),
        Value::Object(m) => {
            check_keys(m, &["preset", "color", "key", "haze", "hdri"], path)?;
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
        "window" => "interior".to_string(),
        "night-city" => "city".to_string(),
        n => n.to_string(),
    };
    let e = Env::preset_tinted(&canon, accent, accent2, tint, key.unwrap_or(norm(v3(-0.55, 1.0, 0.45)))).ok_or_else(|| {
        let hint = crate::components::style::closest(&name, &names).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
        format!("{path}: unknown env `{name}`{hint}")
    })?;
    let haze = haze.unwrap_or(if canon == "cyc" { 0.0015 } else { 0.0 });
    Ok(EnvSpec { env: e, name: canon, tint, haze })
}

/// `"env": "polyhaven:hdri/…"` or `{"hdri": …, "rotate", "exposure", "background", "backgroundExposure", "haze"}`.
fn hdri_env(v: &Value, path: &str) -> Res<Option<EnvSpec>> {
    use super::hdri::{Background, EnvMap, Opts};
    let (src, m) = match v {
        Value::String(s) if crate::assetlib::is_ref(s) || s.ends_with(".hdr") => (s.clone(), None),
        Value::Object(m) if m.contains_key("hdri") => (m["hdri"].as_str().ok_or_else(|| format!("{path}.hdri: expected an asset ref or .hdr path"))?.to_string(), Some(m)),
        _ => return Ok(None),
    };
    // A curated mood (`"hdri": "sunset"`) names a library HDRI.
    let src = match crate::assetlib::hdri_mood(&src) {
        Some(r) => r.to_string(),
        None if !crate::assetlib::is_ref(&src) && !src.contains('.') => {
            let moods: Vec<&str> = crate::assetlib::HDRI_MOODS.iter().map(|m| m.0).collect();
            let hint = crate::components::style::closest(&src, &moods).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(format!("{path}.hdri: `{src}` is not a mood ({}), an asset ref (`polyhaven:hdri/…`) or an .hdr path{hint}", moods.join(", ")));
        }
        None => src,
    };
    let mut o = Opts::default();
    let mut haze = 0.0;
    if let Some(m) = m {
        check_keys(m, &["hdri", "rotate", "exposure", "background", "background_exposure", "haze", "key"], path)?;
        if let Some(x) = m.get("rotate") {
            o.rotate = f(x, &format!("{path}.rotate"))?;
        }
        if let Some(x) = m.get("exposure") {
            o.exposure = f(x, &format!("{path}.exposure"))?;
        }
        if let Some(x) = m.get("background_exposure") {
            o.bg_exposure = f(x, &format!("{path}.backgroundExposure"))?;
        }
        if let Some(x) = m.get("haze") {
            haze = f(x, &format!("{path}.haze"))?;
        }
        if let Some(x) = m.get("key") {
            o.extract_key = x.as_bool().ok_or_else(|| format!("{path}.key: true (lift the sun/key lamp out as a traced shadow-casting light, default) or false"))?;
        }
        o.background = match m.get("background") {
            None | Some(Value::Bool(true)) => Background::Map(0.0),
            Some(Value::Bool(false)) => Background::Off,
            Some(Value::String(s)) if s == "blur" => Background::Map(0.06),
            Some(Value::String(s)) if s == "sharp" => Background::Map(0.0),
            Some(Value::String(s)) if s == "none" || s == "off" => Background::Off,
            Some(Value::Number(n)) => Background::Map(n.as_f64().unwrap_or(0.0).max(0.0).to_radians() as f32),
            Some(x) => return Err(format!("{path}.background: expected true, false, \"blur\" or a blur angle in degrees, got {x}")),
        };
    }
    // A sharp background needs pixels: a 50 mm lens sees ~40° of 360°, so a
    // 1k map gives ~110 px across a 1080p frame. Sharp → 2k unless asked;
    // `@4k` keeps 4096 wide.
    let src = if crate::assetlib::is_ref(&src) && !src.contains('@') && o.background == Background::Map(0.0) { format!("{src}@2k") } else { src };
    if src.ends_with("@4k") {
        o.max_width = 4096;
    }
    let file = if crate::assetlib::is_ref(&src) {
        crate::assetlib::get(&src).map_err(|e| format!("{path}.hdri: {e}"))?.main()
    } else {
        std::path::PathBuf::from(&src)
    };
    // Loaded maps, most recent last; a few are kept (a 2k map is ~33 MB) so a
    // long-lived process (the MCP server) doesn't grow without bound.
    type Loaded = (String, std::sync::Arc<super::hdri::EnvMap>, Option<super::env::Softbox>);
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<Loaded>>> = std::sync::OnceLock::new();
    let ck = format!("{}|{:?}", file.display(), (o.rotate, o.exposure, o.background, o.bg_exposure, o.extract_key));
    let hit = CACHE.get_or_init(Default::default).lock().unwrap().iter().find(|x| x.0 == ck).map(|x| (x.1.clone(), x.2.clone()));
    let (map, key) = match hit {
        Some(x) => x,
        None => {
            let (m, k) = EnvMap::load(&file, o).map_err(|e| format!("{path}.hdri: {e}"))?;
            if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
                eprintln!("hdri {}: key {:?} ({:.0}% of the light), mean {:?}", file.display(), k.as_ref().map(|k| (k.c, k.radiance, k.hu.atan().to_degrees() * 2.0)), m.key_fraction * 100.0, m.mean());
            }
            let x = (std::sync::Arc::new(m), k);
            let mut c = CACHE.get_or_init(Default::default).lock().unwrap();
            if c.len() >= 6 {
                c.remove(0);
            }
            c.push((ck, x.0.clone(), x.1.clone()));
            x
        }
    };
    let (boxes, ki) = match key {
        Some(k) => (vec![k], Some(0)),
        None => (Vec::new(), None),
    };
    let z = v3(0.0, 0.0, 0.0);
    let env = Env::new(boxes, ki, z, z, z, z).with_map(Some(map));
    Ok(Some(EnvSpec { env, name: "hdri".into(), tint: v3(0.8, 0.8, 0.8), haze }))
}

// ------------------------------------------------------------ lights

/// `lights`: extra area lights (softboxes or strips, as far as the
/// environment) added to `env`:
/// `[{ "dir": [x, y, z], "size": [w, h] (degrees), "roll", "color", "intensity", "soft", "key": true }]`.
/// They light diffuse surfaces and show as crisp shapes in reflections; the
/// one marked `key` also casts the traced soft shadows.
/// A local light whose position, aim or intensity is an expression of `t`.
#[derive(Clone)]
pub struct LightAnim {
    pub idx: usize,
    pub pos: [Num; 3],
    pub target: Option<[Num; 3]>,
    pub intensity: Num,
    pub color: V3,
}

impl LightAnim {
    /// Update `l` for time `t`.
    pub fn apply(&self, l: &mut super::trace::Light, t: f32) {
        let v = Vars { t: t as f64, gt: t as f64, ..Vars::default() };
        l.pos = eval3(&self.pos, &v);
        l.power = scale(self.color, self.intensity.at(&v).max(0.0));
        if let Some(tg) = &self.target {
            l.dir = norm(sub(eval3(tg, &v), l.pos));
        }
    }
}

pub fn lights(v: &Value, env: &Env, path: &str) -> Res<(Env, Vec<super::trace::Light>, Vec<LightAnim>)> {
    let mut anims = Vec::new();
    let a = v.as_array().ok_or_else(|| format!("{path}: expected an array of lights"))?;
    let mut boxes = env.boxes.clone();
    let mut key = env.key;
    let mut local = Vec::new();
    for (j, l) in a.iter().enumerate() {
        let p = format!("{path}[{j}]");
        let m = l.as_object().ok_or_else(|| format!("{p}: expected an object"))?;
        check_keys(m, &["dir", "size", "roll", "color", "intensity", "soft", "key", "pos", "target", "angle", "penumbra", "shadow", "type"], &p)?;
        if let Some(pos) = m.get("pos") {
            // A local point light, or a spot light when it has a `target`.
            // `pos`, `target` and `intensity` may be "=expressions" of `t`
            // (a practical that moves, a spot that tracks, a light that pulses).
            let pos_n = vec3(pos, &format!("{p}.pos"))?;
            let tgt_n = m.get("target").map(|t| vec3(t, &format!("{p}.target"))).transpose()?;
            let k_n = m.get("intensity").map(|x| num(x, &format!("{p}.intensity"))).transpose()?.unwrap_or(Num::C(10.0));
            let v0 = Vars::default();
            let pos = eval3(&pos_n, &v0);
            let c = m.get("color").map(|c| color(c, &format!("{p}.color"))).transpose()?.unwrap_or(v3(1.0, 1.0, 1.0));
            let k = k_n.at(&v0);
            let animated = pos_n.iter().chain(tgt_n.iter().flatten()).any(|n| matches!(n, Num::E(..))) || matches!(k_n, Num::E(..));
            if animated {
                anims.push(LightAnim { idx: local.len(), pos: pos_n, target: tgt_n.clone(), intensity: k_n, color: c });
            }
            let (dir, co, ci) = match m.get("target") {
                Some(_) => {
                    let tg = eval3(tgt_n.as_ref().unwrap(), &v0);
                    let ang = m.get("angle").map(|x| f(x, &format!("{p}.angle"))).transpose()?.unwrap_or(30.0).clamp(1.0, 179.0);
                    let pen = m.get("penumbra").map(|x| f(x, &format!("{p}.penumbra"))).transpose()?.unwrap_or(0.25).clamp(0.0, 1.0);
                    let outer = (ang * 0.5).to_radians();
                    let inner = outer * (1.0 - pen);
                    (norm(sub(tg, pos)), outer.cos(), inner.cos())
                }
                None => (v3(0.0, -1.0, 0.0), -1.0, -1.0),
            };
            let shadow = m.get("shadow").and_then(|x| x.as_bool()).unwrap_or(true);
            local.push(super::trace::Light { pos, power: scale(c, k), dir, cos_outer: co, cos_inner: ci, shadow });
            continue;
        }
        let d = m.get("dir").and_then(|d| d.as_array()).filter(|d| d.len() == 3).ok_or_else(|| format!("{p}.dir: expected a direction [x, y, z] towards the light"))?;
        let dir = v3(f(&d[0], &p)?, f(&d[1], &p)?, f(&d[2], &p)?);
        if len(dir) < 1e-6 {
            return Err(format!("{p}.dir: must not be zero"));
        }
        let (sw, sh) = match m.get("size") {
            None => (20.0, 20.0),
            Some(Value::Array(s)) if s.len() == 2 => (f(&s[0], &p)?, f(&s[1], &p)?),
            Some(x) => {
                let s = f(x, &format!("{p}.size"))?;
                (s, s)
            }
        };
        let c = m.get("color").map(|c| color(c, &format!("{p}.color"))).transpose()?.unwrap_or(v3(1.0, 1.0, 1.0));
        let k = m.get("intensity").map(|x| f(x, &format!("{p}.intensity"))).transpose()?.unwrap_or(4.0);
        let roll = m.get("roll").map(|x| f(x, &format!("{p}.roll"))).transpose()?.unwrap_or(0.0);
        let mut b = super::env::Softbox::new(dir, sw.clamp(0.5, 170.0), sh.clamp(0.5, 170.0), roll, scale(c, k));
        if let Some(s) = m.get("soft") {
            b.soft = f(s, &format!("{p}.soft"))?.max(0.0);
        }
        boxes.push(b);
        if m.get("key").and_then(|k| k.as_bool()).unwrap_or(false) {
            key = Some(boxes.len() - 1);
        }
    }
    Ok((Env::new(boxes, key, env.zenith, env.horizon, env.ground, env.band).with_map(env.map.clone()), local, anims))
}
