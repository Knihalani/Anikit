//! A small glTF 2.0 importer for `scene3d` (`.glb`, or `.gltf` with `.bin`
//! files or base64 data URIs).
//!
//! Reads triangle meshes (positions, normals, first UV set, indices), the node
//! hierarchy (matrices or translation/rotation/scale) and each primitive's
//! PBR factors (base colour, metallic, roughness, emission, and
//! `KHR_materials_transmission`). Image textures, skins and animations are
//! not read. Meshes come back in model space, one part per primitive.

use super::math::*;
use super::trace::Material;
use anikit_core::raster::Mesh;
use serde_json::Value;
use std::path::Path;

pub struct Part {
    pub mesh: Mesh,
    pub mat: Material,
    /// The node this primitive hangs from (meshes are node-local when the
    /// file is animated, model space otherwise).
    pub node: usize,
}

/// One animated property of a node.
struct Channel {
    node: usize,
    /// 0 translation, 1 rotation, 2 scale.
    path: u8,
    times: Vec<f32>,
    /// 3 or 4 floats per key.
    values: Vec<f32>,
    step: bool,
}

/// Node hierarchy plus keyframe channels (the file's first animation).
pub struct Anim {
    /// Rest translation, rotation (quaternion), scale, or a fixed matrix.
    rest: Vec<([f32; 3], [f32; 4], [f32; 3], Option<M4>)>,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
    channels: Vec<Channel>,
    pub duration: f32,
}

fn trs(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> M4 {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let n = (x * x + y * y + z * z + w * w).sqrt().max(1e-12);
    let (x, y, z, w) = (x / n, y / n, z / n, w / n);
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    [
        [r[0][0] * s[0], r[0][1] * s[0], r[0][2] * s[0], 0.0],
        [r[1][0] * s[1], r[1][1] * s[1], r[1][2] * s[1], 0.0],
        [r[2][0] * s[2], r[2][1] * s[2], r[2][2] * s[2], 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

impl Anim {
    /// Every node's model-space matrix at time `t` (seconds, clamped).
    pub fn world(&self, t: f32) -> Vec<M4> {
        let mut local: Vec<([f32; 3], [f32; 4], [f32; 3])> = self.rest.iter().map(|r| (r.0, r.1, r.2)).collect();
        for c in &self.channels {
            let n = c.times.len();
            if n == 0 {
                continue;
            }
            let w = if c.path == 1 { 4 } else { 3 };
            let k = c.times.partition_point(|&x| x <= t);
            let (i0, i1, u) = if k == 0 {
                (0, 0, 0.0)
            } else if k >= n {
                (n - 1, n - 1, 0.0)
            } else {
                let (a, b) = (c.times[k - 1], c.times[k]);
                (k - 1, k, if c.step { 0.0 } else { ((t - a) / (b - a).max(1e-6)).clamp(0.0, 1.0) })
            };
            let get = |i: usize, j: usize| c.values.get(i * w + j).copied().unwrap_or(0.0);
            let l = &mut local[c.node];
            match c.path {
                0 => l.0 = std::array::from_fn(|j| get(i0, j) + (get(i1, j) - get(i0, j)) * u),
                2 => l.2 = std::array::from_fn(|j| get(i0, j) + (get(i1, j) - get(i0, j)) * u),
                _ => {
                    // Normalised lerp along the short arc (smooth for key spacing used in practice).
                    let a: [f32; 4] = std::array::from_fn(|j| get(i0, j));
                    let mut b: [f32; 4] = std::array::from_fn(|j| get(i1, j));
                    if a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>() < 0.0 {
                        b = b.map(|v| -v);
                    }
                    l.1 = std::array::from_fn(|j| a[j] + (b[j] - a[j]) * u);
                }
            }
        }
        let mut out = vec![IDENT; self.rest.len()];
        let mut stack: Vec<(usize, M4)> = self.roots.iter().map(|&r| (r, IDENT)).collect();
        let mut guard = 0;
        while let Some((ni, parent)) = stack.pop() {
            guard += 1;
            if guard > 100_000 {
                break;
            }
            let m = match self.rest[ni].3 {
                Some(fixed) => fixed,
                None => trs(local[ni].0, local[ni].1, local[ni].2),
            };
            let w = mm(&parent, &m);
            out[ni] = w;
            for &c in &self.children[ni] {
                stack.push((c, w));
            }
        }
        out
    }
}

/// A model-space matrix applied to a point.
pub fn apply(m: &M4, p: V3) -> V3 {
    v3(m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0], m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1], m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2])
}

pub type M4 = [[f32; 4]; 4];

const IDENT: M4 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// Column-major 4×4 product `a · b`.
fn mm(a: &M4, b: &M4) -> M4 {
    std::array::from_fn(|c| std::array::from_fn(|r| (0..4).map(|k| a[k][r] * b[c][k]).sum()))
}

fn node_matrix(n: &Value) -> M4 {
    if let Some(m) = n.get("matrix").and_then(|m| m.as_array()).filter(|m| m.len() == 16) {
        let f: Vec<f32> = m.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
        return std::array::from_fn(|c| std::array::from_fn(|r| f[c * 4 + r]));
    }
    let arr = |k: &str, d: &[f32]| -> Vec<f32> { n.get(k).and_then(|v| v.as_array()).map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_else(|| d.to_vec()) };
    let t = arr("translation", &[0.0, 0.0, 0.0]);
    let q = arr("rotation", &[0.0, 0.0, 0.0, 1.0]);
    let s = arr("scale", &[1.0, 1.0, 1.0]);
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    [
        [r[0][0] * s[0], r[0][1] * s[0], r[0][2] * s[0], 0.0],
        [r[1][0] * s[1], r[1][1] * s[1], r[1][2] * s[1], 0.0],
        [r[2][0] * s[2], r[2][1] * s[2], r[2][2] * s[2], 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

fn b64(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return Err("bad base64 in data URI".into()),
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

struct Doc {
    json: Value,
    buffers: Vec<Vec<u8>>,
    /// Folder for external image URIs, and the file (texture cache key).
    dir: std::path::PathBuf,
    file: String,
}

impl Doc {
    fn accessor(&self, i: usize, comps: usize) -> Result<Vec<f32>, String> {
        let a = &self.json["accessors"][i];
        let count = a["count"].as_u64().ok_or("accessor without count")? as usize;
        let ct = a["componentType"].as_u64().unwrap_or(5126);
        let n = match a["type"].as_str().unwrap_or("SCALAR") {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            t => return Err(format!("unsupported accessor type {t}")),
        };
        if n < comps {
            return Err("accessor has too few components".into());
        }
        let Some(bvi) = a["bufferView"].as_u64() else {
            return Ok(vec![0.0; count * comps]);
        };
        let bv = &self.json["bufferViews"][bvi as usize];
        let buf = self.buffers.get(bv["buffer"].as_u64().unwrap_or(0) as usize).ok_or("missing buffer")?;
        let csize = match ct {
            5126 | 5125 => 4,
            5123 | 5122 => 2,
            5121 | 5120 => 1,
            _ => return Err(format!("unsupported component type {ct}")),
        };
        let stride = bv["byteStride"].as_u64().map(|s| s as usize).unwrap_or(n * csize);
        let base = bv["byteOffset"].as_u64().unwrap_or(0) as usize + a["byteOffset"].as_u64().unwrap_or(0) as usize;
        let norm = a["normalized"].as_bool().unwrap_or(false);
        let mut out = Vec::with_capacity(count * comps);
        for e in 0..count {
            for c in 0..comps {
                let o = base + e * stride + c * csize;
                let b = buf.get(o..o + csize).ok_or("accessor out of bounds")?;
                let v = match ct {
                    5126 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
                    5123 => {
                        let x = u16::from_le_bytes([b[0], b[1]]) as f32;
                        if norm { x / 65535.0 } else { x }
                    }
                    5122 => {
                        let x = i16::from_le_bytes([b[0], b[1]]) as f32;
                        if norm { (x / 32767.0).max(-1.0) } else { x }
                    }
                    5121 => {
                        let x = b[0] as f32;
                        if norm { x / 255.0 } else { x }
                    }
                    _ => {
                        let x = b[0] as i8 as f32;
                        if norm { (x / 127.0).max(-1.0) } else { x }
                    }
                };
                out.push(v);
            }
        }
        Ok(out)
    }

    fn animation(&self, roots: Vec<usize>) -> Result<Anim, String> {
        let nodes = self.json["nodes"].as_array().cloned().unwrap_or_default();
        let arr = |n: &Value, k: &str, d: &[f32]| -> Vec<f32> { n.get(k).and_then(|v| v.as_array()).map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_else(|| d.to_vec()) };
        let rest = nodes
            .iter()
            .map(|n| {
                let t = arr(n, "translation", &[0.0, 0.0, 0.0]);
                let r = arr(n, "rotation", &[0.0, 0.0, 0.0, 1.0]);
                let s = arr(n, "scale", &[1.0, 1.0, 1.0]);
                let fixed = n.get("matrix").map(|_| node_matrix(n));
                ([t[0], t[1], t[2]], [r[0], r[1], r[2], r[3]], [s[0], s[1], s[2]], fixed)
            })
            .collect();
        let children = nodes.iter().map(|n| n["children"].as_array().map(|a| a.iter().filter_map(|c| c.as_u64()).map(|c| c as usize).filter(|&c| c < nodes.len()).collect()).unwrap_or_default()).collect();
        let a = &self.json["animations"][0];
        let mut channels = Vec::new();
        let mut duration = 0.0f32;
        for ch in a["channels"].as_array().cloned().unwrap_or_default() {
            let Some(node) = ch["target"]["node"].as_u64() else { continue };
            let path = match ch["target"]["path"].as_str() {
                Some("translation") => 0,
                Some("rotation") => 1,
                Some("scale") => 2,
                _ => continue,
            };
            let smp = &a["samplers"][ch["sampler"].as_u64().unwrap_or(0) as usize];
            let times = self.accessor(smp["input"].as_u64().ok_or("sampler without input")? as usize, 1)?;
            let w = if path == 1 { 4 } else { 3 };
            let mut values = self.accessor(smp["output"].as_u64().ok_or("sampler without output")? as usize, w)?;
            let interp = smp["interpolation"].as_str().unwrap_or("LINEAR");
            if interp == "CUBICSPLINE" {
                // Keep the values, drop the tangents (linear between keys).
                values = values.chunks_exact(w * 3).flat_map(|c| c[w..2 * w].to_vec()).collect();
            }
            duration = duration.max(times.last().copied().unwrap_or(0.0));
            if (node as usize) < nodes.len() {
                channels.push(Channel { node: node as usize, path, times, values, step: interp == "STEP" });
            }
        }
        Ok(Anim { rest, children, roots, channels, duration })
    }

    fn material(&self, i: Option<u64>) -> Material {
        let mut m = Material { base: v3(0.8, 0.8, 0.8), metallic: 1.0, roughness: 1.0, ..Material::chrome() };
        let Some(i) = i else {
            m.metallic = 0.0;
            m.roughness = 0.5;
            return m;
        };
        let j = &self.json["materials"][i as usize];
        let pbr = &j["pbrMetallicRoughness"];
        if let Some(c) = pbr["baseColorFactor"].as_array() {
            let f = |k: usize| c.get(k).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32;
            m.base = v3(f(0), f(1), f(2));
        }
        m.metallic = pbr["metallicFactor"].as_f64().unwrap_or(1.0) as f32;
        m.roughness = pbr["roughnessFactor"].as_f64().unwrap_or(1.0) as f32;
        if let Some(e) = j["emissiveFactor"].as_array() {
            let f = |k: usize| e.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            let s = j["extensions"]["KHR_materials_emissive_strength"]["emissiveStrength"].as_f64().unwrap_or(1.0) as f32;
            m.emissive = scale(v3(f(0), f(1), f(2)), s);
        }
        if let Some(t) = j["extensions"]["KHR_materials_transmission"]["transmissionFactor"].as_f64() {
            m.transmission = t as f32;
            m.absorb = v3(0.9, 0.2, 0.6);
        }
        if let Some(ior) = j["extensions"]["KHR_materials_ior"]["ior"].as_f64() {
            m.ior = ior as f32;
        }
        match self.textures(i as usize, &m) {
            Ok(Some(id)) => {
                m.pbr = id;
                m.base = v3(1.0, 1.0, 1.0);
            }
            Ok(None) => {}
            Err(e) => eprintln!("anikit: glTF {}: material {i}: textures skipped ({e})", self.file),
        }
        m
    }

    /// Encoded bytes of texture `t`'s image (file, data URI or buffer view).
    fn image(&self, t: &Value) -> Result<Option<Vec<u8>>, String> {
        let Some(ti) = t["index"].as_u64() else { return Ok(None) };
        let tex = &self.json["textures"][ti as usize];
        let src = tex["source"].as_u64().or_else(|| tex["extensions"]["EXT_texture_webp"]["source"].as_u64()).ok_or("texture without source")?;
        let img = &self.json["images"][src as usize];
        if let Some(u) = img["uri"].as_str() {
            if u.starts_with("data:") {
                return b64(u.split_once(',').map(|x| x.1).unwrap_or("")).map(Some);
            }
            let u = percent_decode(u);
            if std::path::Path::new(&u).components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir)) {
                return Err(format!("image uri `{u}` leaves the model's folder"));
            }
            return std::fs::read(self.dir.join(&u)).map(Some).map_err(|e| format!("image {u}: {e}"));
        }
        let bv = &self.json["bufferViews"][img["bufferView"].as_u64().ok_or("image without uri or bufferView")? as usize];
        let buf = self.buffers.get(bv["buffer"].as_u64().unwrap_or(0) as usize).ok_or("bad buffer")?;
        let o = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
        let n = bv["byteLength"].as_u64().unwrap_or(0) as usize;
        buf.get(o..o + n).map(|b| Some(b.to_vec())).ok_or_else(|| "image bufferView out of range".into())
    }

    /// `textures/x_leaves_diff_1k.jpg` → an existing `textures/x_leaves_alpha_1k.png` (or .jpg).
    fn sibling_alpha(&self, t: &Value) -> Option<std::path::PathBuf> {
        let tex = &self.json["textures"][t["index"].as_u64()? as usize];
        let uri = percent_decode(self.json["images"][tex["source"].as_u64()? as usize]["uri"].as_str()?);
        let stem = uri.rsplit_once('.')?.0;
        for (a, b) in [("_diff_", "_alpha_"), ("_diffuse_", "_alpha_"), ("_color_", "_alpha_"), ("_basecolor_", "_alpha_")] {
            if stem.to_lowercase().contains(a) {
                let i = stem.to_lowercase().find(a)?;
                let cand = format!("{}{b}{}", &stem[..i], &stem[i + a.len()..]);
                for ext in ["png", "jpg"] {
                    let p = self.dir.join(format!("{cand}.{ext}"));
                    if p.exists() && !cand.contains("..") {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    /// The material's base colour / normal / metallic-roughness / occlusion
    /// textures as a UV-mapped PBR set (cached per file and material).
    fn textures(&self, i: usize, m: &Material) -> Result<Option<u32>, String> {
        use super::pbr::{Mapping, Mips, PbrSet};
        static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<(String, usize), u32>>> = std::sync::OnceLock::new();
        let key = (self.file.clone(), i);
        if let Some(&id) = CACHE.get_or_init(Default::default).lock().unwrap().get(&key) {
            return Ok(Some(id));
        }
        let j = &self.json["materials"][i];
        let pbr = &j["pbrMetallicRoughness"];
        const MAX: usize = 2048;
        let dec = |b: Vec<u8>| image::load_from_memory(&b).map_err(|e| e.to_string());
        let albedo = self.image(&pbr["baseColorTexture"])?;
        let normal = self.image(&j["normalTexture"])?;
        let mr = self.image(&pbr["metallicRoughnessTexture"])?;
        let occ = self.image(&j["occlusionTexture"])?;
        if albedo.is_none() && normal.is_none() && mr.is_none() {
            return Ok(None);
        }
        let fit = |img: image::DynamicImage| if img.width().max(img.height()) as usize > MAX { img.resize(MAX as u32, MAX as u32, image::imageops::FilterType::Triangle) } else { img };
        let rgb = |b: Vec<u8>| -> Result<Mips, String> {
            let im = fit(dec(b)?).to_rgb8();
            Ok(Mips::from_raw(im.width() as usize, im.height() as usize, 3, im.into_raw()))
        };
        let chan = |b: &[u8], ch: usize| -> Result<Mips, String> {
            let im = fit(dec(b.to_vec())?).to_rgb8();
            Ok(Mips::from_raw(im.width() as usize, im.height() as usize, 1, im.pixels().map(|p| p[ch]).collect()))
        };
        let (rough, metal) = match &mr {
            Some(b) => (Some(chan(b, 1)?), Some(chan(b, 2)?)),
            None => (None, None),
        };
        // Same image for occlusion and metallic-roughness (ARM packing) is common.
        let ao = match &occ {
            Some(b) => Some(chan(b, 0)?),
            None => None,
        };
        // Cutouts (leaves, fences): coverage from the base colour's alpha.
        let cutoff = match j["alphaMode"].as_str() {
            Some("MASK") => Some(j["alphaCutoff"].as_f64().unwrap_or(0.5) as f32),
            Some("BLEND") => Some(0.5),
            _ => None,
        };
        let alpha = match (cutoff, &albedo) {
            (Some(c), Some(b)) => {
                let im = fit(dec(b.clone())?);
                if im.color().has_alpha() {
                    let im = im.to_rgba8();
                    Some((Mips::from_raw(im.width() as usize, im.height() as usize, 1, im.pixels().map(|p| p[3]).collect()), c))
                } else {
                    // A JPEG colour map (Poly Haven's exports): the cutout is a
                    // sibling `…_alpha_…` file, when there is one.
                    self.sibling_alpha(&pbr["baseColorTexture"]).map(|p| Mips::load(&p, 1, MAX)).transpose()?.map(|m| (m, c))
                }
            }
            _ => None,
        };
        let set = PbrSet {
            alpha,
            albedo: albedo.map(rgb).transpose()?,
            normal: normal.map(rgb).transpose()?,
            rough,
            ao,
            metal,
            mapping: Mapping::Uv,
            scale: 1.0,
            tint: m.base,
            rough_mul: m.roughness.max(0.0),
            normal_strength: j["normalTexture"]["scale"].as_f64().unwrap_or(1.0) as f32,
            keep_rough: false,
            keep_metal: m.metallic <= 0.0,
            rotate: 0.0,
        };
        let id = super::pbr::register(set);
        CACHE.get_or_init(Default::default).lock().unwrap().insert(key, id);
        Ok(Some(id))
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Load every triangle primitive of the default scene, in model space.
/// With animations, meshes stay node-local and the [`Anim`] places them.
pub fn load(path: &Path) -> Result<(Vec<Part>, Option<Anim>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let (json, bin) = if bytes.starts_with(b"glTF") {
        let rd = |o: usize| -> Result<u32, String> { bytes.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "truncated GLB".to_string()) };
        let len0 = rd(12)? as usize;
        let json: Value = serde_json::from_slice(bytes.get(20..20 + len0).ok_or("truncated GLB")?).map_err(|e| format!("GLB JSON: {e}"))?;
        let o = 20 + len0;
        let bin = if o + 8 <= bytes.len() {
            let len1 = rd(o)? as usize;
            Some(bytes.get(o + 8..o + 8 + len1).ok_or("truncated GLB")?.to_vec())
        } else {
            None
        };
        (json, bin)
    } else {
        (serde_json::from_slice::<Value>(&bytes).map_err(|e| format!("glTF JSON: {e}"))?, None)
    };
    let mut buffers = Vec::new();
    for (i, b) in json["buffers"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        match b["uri"].as_str() {
            None if i == 0 => buffers.push(bin.clone().ok_or("buffer 0 has no data")?),
            None => return Err(format!("buffer {i} has no uri")),
            Some(u) if u.starts_with("data:") => buffers.push(b64(u.split_once(',').map(|x| x.1).unwrap_or(""))?),
            Some(u) => buffers.push(std::fs::read(dir.join(u)).map_err(|e| format!("can't read buffer {u}: {e}"))?),
        }
    }
    let doc = Doc { json, buffers, dir: dir.to_path_buf(), file: path.display().to_string() };
    let scene = doc.json["scene"].as_u64().unwrap_or(0) as usize;
    let roots: Vec<u64> = match doc.json["scenes"][scene]["nodes"].as_array() {
        Some(a) => a.iter().filter_map(|x| x.as_u64()).collect(),
        None => (0..doc.json["nodes"].as_array().map(|a| a.len()).unwrap_or(0) as u64).collect(),
    };
    let mut parts = Vec::new();
    let animated = doc.json["animations"].as_array().is_some_and(|a| !a.is_empty());
    let roots_all: Vec<usize> = roots.iter().map(|&r| r as usize).collect();
    let mut stack: Vec<(u64, M4)> = roots.into_iter().map(|r| (r, IDENT)).collect();
    let mut guard = 0;
    while let Some((ni, parent)) = stack.pop() {
        guard += 1;
        if guard > 100_000 {
            return Err("node hierarchy too deep or cyclic".into());
        }
        let node = &doc.json["nodes"][ni as usize];
        let m = mm(&parent, &node_matrix(node));
        // Animated files keep meshes node-local; the animation places them.
        let mv = if animated { IDENT } else { m };
        if let Some(mi) = node["mesh"].as_u64() {
            for prim in doc.json["meshes"][mi as usize]["primitives"].as_array().cloned().unwrap_or_default() {
                if prim["mode"].as_u64().unwrap_or(4) != 4 {
                    continue;
                }
                let attrs = &prim["attributes"];
                let Some(pi) = attrs["POSITION"].as_u64() else { continue };
                let pos = doc.accessor(pi as usize, 3)?;
                let nv = pos.len() / 3;
                let nrm = match attrs["NORMAL"].as_u64() {
                    Some(i) => doc.accessor(i as usize, 3)?,
                    None => Vec::new(),
                };
                let uv = match attrs["TEXCOORD_0"].as_u64() {
                    Some(i) => doc.accessor(i as usize, 2)?,
                    None => vec![0.0; nv * 2],
                };
                let idx: Vec<u32> = match prim["indices"].as_u64() {
                    Some(i) => doc.accessor(i as usize, 1)?.into_iter().map(|x| x as u32).collect(),
                    None => (0..nv as u32).collect(),
                };
                if idx.iter().any(|&i| i as usize >= nv) {
                    return Err("index out of range".into());
                }
                // Transform to model space; normals by the inverse transpose.
                let mut mesh = Mesh { pos: Vec::with_capacity(nv * 3), nrm: Vec::new(), uv, idx };
                for p in pos.chunks_exact(3) {
                    for r in 0..3 {
                        mesh.pos.push(mv[0][r] * p[0] + mv[1][r] * p[1] + mv[2][r] * p[2] + mv[3][r]);
                    }
                }
                let a = v3(mv[0][0], mv[0][1], mv[0][2]);
                let b = v3(mv[1][0], mv[1][1], mv[1][2]);
                let c = v3(mv[2][0], mv[2][1], mv[2][2]);
                let (cb, cc, ca) = (cross(b, c), cross(c, a), cross(a, b));
                let flip = dot(a, cb) < 0.0;
                if nrm.len() == pos.len() {
                    for n in nrm.chunks_exact(3) {
                        let w = norm(add(add(scale(cb, n[0]), scale(cc, n[1])), scale(ca, n[2])));
                        mesh.nrm.extend_from_slice(&[w.x, w.y, w.z]);
                    }
                } else {
                    // No normals: smooth them from the faces.
                    let mut acc = vec![v3(0.0, 0.0, 0.0); nv];
                    for t in mesh.idx.chunks_exact(3) {
                        let p = |i: u32| v3(mesh.pos[i as usize * 3], mesh.pos[i as usize * 3 + 1], mesh.pos[i as usize * 3 + 2]);
                        let f = cross(sub(p(t[1]), p(t[0])), sub(p(t[2]), p(t[0])));
                        for &i in t {
                            acc[i as usize] = add(acc[i as usize], f);
                        }
                    }
                    for n in acc {
                        let n = norm(n);
                        mesh.nrm.extend_from_slice(&[n.x, n.y, n.z]);
                    }
                }
                if flip {
                    for t in mesh.idx.chunks_exact_mut(3) {
                        t.swap(1, 2);
                    }
                }
                parts.push(Part { mesh, mat: doc.material(prim["material"].as_u64()), node: ni as usize });
            }
        }
        for c in node["children"].as_array().cloned().unwrap_or_default() {
            if let Some(c) = c.as_u64() {
                stack.push((c, m));
            }
        }
    }
    if parts.is_empty() {
        return Err(format!("{}: no triangle meshes", path.display()));
    }
    let anim = if animated { Some(doc.animation(roots_all)?) } else { None };
    Ok((parts, anim))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Write a one-triangle glTF (embedded buffer, a translated node) to `p`.
    pub(crate) fn write_triangle(p: &std::path::Path) {
        write_tri(p, false)
    }

    /// The triangle with a 1 s translation animation from x = 0 to x = 2.
    pub(crate) fn write_animated_triangle(p: &std::path::Path) {
        write_tri(p, true)
    }

    fn write_tri(p: &std::path::Path, animated: bool) {
        let mut bin = Vec::new();
        for f in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&f.to_le_bytes());
        }
        for i in [0u16, 1, 2, 0] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        // Animation data at byte 44: times [0, 1], translations [0,2,0]→[2,2,0].
        for f in [0.0f32, 1.0, 0.0, 2.0, 0.0, 2.0, 2.0, 0.0] {
            bin.extend_from_slice(&f.to_le_bytes());
        }
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bin.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            for k in 0..4 {
                if k <= c.len() {
                    s.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        let mut gltf = serde_json::json!({
            "asset": { "version": "2.0" },
            "buffers": [ { "byteLength": bin.len(), "uri": format!("data:application/octet-stream;base64,{s}") } ],
            "bufferViews": [ { "buffer": 0, "byteOffset": 0, "byteLength": 36 }, { "buffer": 0, "byteOffset": 36, "byteLength": 6 } ],
            "accessors": [ { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" }, { "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" } ],
            "materials": [ { "pbrMetallicRoughness": { "baseColorFactor": [1, 0, 0, 1], "metallicFactor": 0, "roughnessFactor": 0.3 } } ],
            "meshes": [ { "primitives": [ { "attributes": { "POSITION": 0 }, "indices": 1, "material": 0 } ] } ],
            "nodes": [ { "mesh": 0, "translation": [0, 2, 0] } ],
            "scenes": [ { "nodes": [0] } ]
        });
        if animated {
            gltf["bufferViews"].as_array_mut().unwrap().push(serde_json::json!({ "buffer": 0, "byteOffset": 44, "byteLength": 8 }));
            gltf["bufferViews"].as_array_mut().unwrap().push(serde_json::json!({ "buffer": 0, "byteOffset": 52, "byteLength": 24 }));
            gltf["accessors"].as_array_mut().unwrap().push(serde_json::json!({ "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR" }));
            gltf["accessors"].as_array_mut().unwrap().push(serde_json::json!({ "bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3" }));
            gltf["animations"] = serde_json::json!([ { "samplers": [ { "input": 2, "output": 3 } ], "channels": [ { "sampler": 0, "target": { "node": 0, "path": "translation" } } ] } ]);
        }
        std::fs::write(p, gltf.to_string()).unwrap();
    }

    #[test]
    fn loads_embedded_gltf() {
        let dir = std::env::temp_dir().join(format!("scene3d-gltf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("tri.gltf");
        write_triangle(&p);
        let (parts, anim) = load(&p).unwrap();
        assert!(anim.is_none());
        assert_eq!(parts.len(), 1);
        let m = &parts[0].mesh;
        assert_eq!(m.idx, vec![0, 1, 2]);
        assert!((m.pos[1] - 2.0).abs() < 1e-6 && (m.pos[3] - 1.0).abs() < 1e-6);
        assert!((m.nrm[2] - 1.0).abs() < 1e-5, "computed normal faces +z");
        assert!((parts[0].mat.base.x - 1.0).abs() < 1e-6 && parts[0].mat.metallic == 0.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
