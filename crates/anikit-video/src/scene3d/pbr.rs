//! PBR texture sets on `scene3d` materials:
//! `"material": {"texture": "ambientcg:Wood049", "scale": 2}` (albedo, normal,
//! roughness, ambient occlusion, metalness), triplanar-mapped in object space
//! by default (no UVs needed, nothing swims when the object moves), or by the
//! mesh's UVs (`"mapping": "uv"`, and glTF models' own textures).
//!
//! Maps are kept as 8-bit mip chains (a 2k set is ~35 MB); a lookup picks the
//! mip whose texel matches the ray's footprint, so distant surfaces don't
//! sparkle. Sets are registered once per process and shared by index, so
//! [`super::trace::Material`] stays `Copy`.

use super::math::*;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

/// One map as a mip chain of `c`-channel 8-bit images.
pub struct Mips {
    pub c: usize,
    levels: Vec<(usize, usize, Vec<u8>)>,
}

impl Mips {
    /// From an image file, keeping `c` channels (1 = luminance/first channel, 3 = RGB).
    pub fn load(path: &Path, c: usize, max: usize) -> Result<Mips, String> {
        let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let img = if img.width().max(img.height()) as usize > max { img.resize(max as u32, max as u32, image::imageops::FilterType::Triangle) } else { img };
        let (w, h) = (img.width() as usize, img.height() as usize);
        let data: Vec<u8> = if c == 1 { img.to_luma8().into_raw() } else { img.to_rgb8().into_raw() };
        Ok(Self::from_raw(w, h, c, data))
    }

    /// From one channel of an RGB image (glTF packs AO/roughness/metalness into R/G/B).
    pub fn load_channel(path: &Path, ch: usize, max: usize) -> Result<Mips, String> {
        let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let img = if img.width().max(img.height()) as usize > max { img.resize(max as u32, max as u32, image::imageops::FilterType::Triangle) } else { img };
        let (w, h) = (img.width() as usize, img.height() as usize);
        let rgb = img.to_rgb8();
        Ok(Self::from_raw(w, h, 1, rgb.pixels().map(|p| p[ch]).collect()))
    }

    pub fn from_raw(w: usize, h: usize, c: usize, data: Vec<u8>) -> Mips {
        let mut levels = vec![(w, h, data)];
        loop {
            let (pw, ph, pd) = levels.last().unwrap();
            if *pw <= 1 && *ph <= 1 {
                break;
            }
            let (nw, nh) = ((pw / 2).max(1), (ph / 2).max(1));
            let mut nd = vec![0u8; nw * nh * c];
            for y in 0..nh {
                let (y0, y1) = ((2 * y).min(ph - 1), (2 * y + 1).min(ph - 1));
                for x in 0..nw {
                    let (x0, x1) = ((2 * x).min(pw - 1), (2 * x + 1).min(pw - 1));
                    for k in 0..c {
                        let s = pd[(y0 * pw + x0) * c + k] as u32 + pd[(y0 * pw + x1) * c + k] as u32 + pd[(y1 * pw + x0) * c + k] as u32 + pd[(y1 * pw + x1) * c + k] as u32;
                        nd[(y * nw + x) * c + k] = ((s + 2) / 4) as u8;
                    }
                }
            }
            levels.push((nw, nh, nd));
        }
        Mips { c, levels }
    }

    pub fn width(&self) -> usize {
        self.levels[0].0
    }

    /// Bilinear, wrapping, at mip `lod` (fractional: blends two levels). Channels 0..1.
    #[inline]
    pub fn sample(&self, u: f32, v: f32, lod: f32) -> [f32; 3] {
        let n = self.levels.len();
        let l = lod.clamp(0.0, (n - 1) as f32);
        let k = l as usize;
        let t = l - k as f32;
        let a = self.bilerp(k, u, v);
        if t < 0.01 || k + 1 >= n {
            return a;
        }
        let b = self.bilerp(k + 1, u, v);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    }

    #[inline]
    fn bilerp(&self, k: usize, u: f32, v: f32) -> [f32; 3] {
        let (w, h, d) = &self.levels[k];
        let (w, h) = (*w, *h);
        let x = u * w as f32 - 0.5;
        let y = v * h as f32 - 0.5;
        let (xf, yf) = (ffloor(x), ffloor(y));
        let (fx, fy) = (x - xf, y - yf);
        let x0 = (xf as i64).rem_euclid(w as i64) as usize;
        let y0 = (yf as i64).rem_euclid(h as i64) as usize;
        let x1 = if x0 + 1 == w { 0 } else { x0 + 1 };
        let y1 = if y0 + 1 == h { 0 } else { y0 + 1 };
        let c = self.c;
        let mut o = [0.0f32; 3];
        for (k, ok) in o.iter_mut().enumerate().take(c) {
            let p = |x: usize, y: usize| d[(y * w + x) * c + k] as f32;
            let top = p(x0, y0) + (p(x1, y0) - p(x0, y0)) * fx;
            let bot = p(x0, y1) + (p(x1, y1) - p(x0, y1)) * fx;
            *ok = (top + (bot - top) * fy) * (1.0 / 255.0);
        }
        if c == 1 {
            o[1] = o[0];
            o[2] = o[0];
        }
        o
    }
}

/// How a set is laid on a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mapping {
    /// Object-space triplanar; `scale` = texture repeats per metre.
    Triplanar,
    /// The mesh's UVs; `scale` = repeats per UV unit.
    Uv,
}

/// A PBR set.
pub struct PbrSet {
    pub albedo: Option<Mips>,
    pub normal: Option<Mips>,
    pub rough: Option<Mips>,
    pub ao: Option<Mips>,
    pub metal: Option<Mips>,
    pub mapping: Mapping,
    pub scale: f32,
    /// Multiplies the albedo (linear).
    pub tint: V3,
    /// Multiplies the map's roughness / normal strength.
    pub rough_mul: f32,
    pub normal_strength: f32,
    /// Use the material's own roughness/metallic instead of the maps.
    pub keep_rough: bool,
    pub keep_metal: bool,
    /// Rotate the texture (degrees) in each plane.
    pub rotate: f32,
}

/// What a surface point looks like.
pub struct Sampled {
    pub base: V3,
    pub rough: Option<f32>,
    pub metal: Option<f32>,
    pub ao: f32,
    /// Perturbed normal (world space), when there's a normal map.
    pub n: Option<V3>,
}

fn srgb_lin(v: f32) -> f32 {
    crate::render::post::srgb_to_linear(v)
}

impl PbrSet {
    /// Mip level for a footprint of `world` metres (per pixel) at `rep` repeats per metre.
    #[inline]
    fn lod(&self, m: &Mips, footprint: f32) -> f32 {
        let texels = footprint * self.scale * m.width() as f32;
        if texels <= 1.0 { 0.0 } else { texels.log2() }
    }

    /// Sample at object-space point `p` with object-space normal `n_o`
    /// (`uv` for UV mapping; `tan` an object-space tangent along +u), then
    /// bring the normal to world space with `to_world`.
    pub fn sample(&self, p: V3, n_o: V3, uv: [f32; 2], tan: V3, footprint: f32, to_world: impl Fn(V3) -> V3) -> Sampled {
        let (s, c) = self.rotate.to_radians().sin_cos();
        let rot = |a: f32, b: f32| -> (f32, f32) { (a * c - b * s, a * s + b * c) };
        // Planes: (u, v, weight); for UV mapping, one plane.
        let planes: [(f32, f32, f32); 3] = match self.mapping {
            Mapping::Uv => {
                let (u, v) = rot(uv[0] * self.scale, uv[1] * self.scale);
                [(u, v, 1.0), (0.0, 0.0, 0.0), (0.0, 0.0, 0.0)]
            }
            Mapping::Triplanar => {
                let k = self.scale;
                let w = v3(n_o.x.abs().powi(4), n_o.y.abs().powi(4), n_o.z.abs().powi(4));
                let ws = (w.x + w.y + w.z).max(1e-6);
                // Flip u on back faces so text-like textures don't mirror.
                let (ux, vx) = rot(-p.z * k * n_o.x.signum(), -p.y * k);
                let (uy, vy) = rot(p.x * k, p.z * k * n_o.y.signum());
                let (uz, vz) = rot(p.x * k * n_o.z.signum(), -p.y * k);
                [(ux, vx, w.x / ws), (uy, vy, w.y / ws), (uz, vz, w.z / ws)]
            }
        };
        let tri = |m: &Mips| -> [f32; 3] {
            let lod = self.lod(m, footprint);
            let mut o = [0.0f32; 3];
            for &(u, v, w) in &planes {
                if w < 0.003 {
                    continue;
                }
                let x = m.sample(u, v, lod);
                for k in 0..3 {
                    o[k] += x[k] * w;
                }
            }
            o
        };
        let base = match &self.albedo {
            Some(m) => {
                // Average in sRGB then linearise: close enough at these blend weights.
                let a = tri(m);
                mul(v3(srgb_lin(a[0]), srgb_lin(a[1]), srgb_lin(a[2])), self.tint)
            }
            None => self.tint,
        };
        let rough = if self.keep_rough { None } else { self.rough.as_ref().map(|m| (tri(m)[0] * self.rough_mul).clamp(0.02, 1.0)) };
        let metal = if self.keep_metal { None } else { self.metal.as_ref().map(|m| tri(m)[0]) };
        let ao = self.ao.as_ref().map(|m| tri(m)[0]).unwrap_or(1.0);
        let n = self.normal.as_ref().map(|m| {
            let lod = self.lod(m, footprint);
            let st = self.normal_strength;
            let unpack = |u: f32, v: f32| -> (f32, f32) {
                let x = m.sample(u, v, lod);
                let (a, b) = ((x[0] * 2.0 - 1.0) * st, (x[1] * 2.0 - 1.0) * st);
                // Undo the plane rotation on the tangent-space vector.
                (a * c + b * s, -a * s + b * c)
            };
            let no = match self.mapping {
                Mapping::Uv => {
                    let (a, b) = unpack(planes[0].0, planes[0].1);
                    let t = norm(sub(tan, scale(n_o, dot(tan, n_o))));
                    let bt = cross(n_o, t);
                    // glTF/OpenGL normal maps: +y is +v up; our v grows downward.
                    norm(add(add(scale(t, a), scale(bt, b)), n_o))
                }
                Mapping::Triplanar => {
                    // Whiteout-style blend: each plane's tangent offsets added
                    // to the matching components of the geometric normal.
                    let mut acc = v3(0.0, 0.0, 0.0);
                    let sx = n_o.x.signum();
                    let sy = n_o.y.signum();
                    let sz = n_o.z.signum();
                    if planes[0].2 >= 0.003 {
                        let (a, b) = unpack(planes[0].0, planes[0].1);
                        acc = madd(acc, v3(n_o.x, n_o.y + b, n_o.z - a * sx), planes[0].2);
                    }
                    if planes[1].2 >= 0.003 {
                        let (a, b) = unpack(planes[1].0, planes[1].1);
                        acc = madd(acc, v3(n_o.x + a, n_o.y, n_o.z - b * sy), planes[1].2);
                    }
                    if planes[2].2 >= 0.003 {
                        let (a, b) = unpack(planes[2].0, planes[2].1);
                        acc = madd(acc, v3(n_o.x + a * sz, n_o.y + b, n_o.z), planes[2].2);
                    }
                    norm(acc)
                }
            };
            to_world(no)
        });
        Sampled { base, rough, metal, ao, n }
    }
}

fn registry() -> &'static RwLock<Vec<Arc<PbrSet>>> {
    static R: OnceLock<RwLock<Vec<Arc<PbrSet>>>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register a set; returns its id (≥ 1; 0 means "no set").
pub fn register(s: PbrSet) -> u32 {
    let mut r = registry().write().unwrap();
    r.push(Arc::new(s));
    r.len() as u32
}

/// Every registered set, for a scene to hold (index `id - 1`).
pub fn snapshot() -> Vec<Arc<PbrSet>> {
    registry().read().unwrap().clone()
}

/// Options from a material object.
#[derive(Clone, Debug, PartialEq)]
pub struct SetOpts {
    pub scale: f32,
    pub tint: V3,
    pub mapping: Mapping,
    pub rough_mul: f32,
    pub normal_strength: f32,
    pub keep_rough: bool,
    pub keep_metal: bool,
    pub rotate: f32,
    pub max: usize,
}

/// Load a library texture set (or a folder of maps) once per (source, options).
pub fn load(src: &str, o: &SetOpts) -> Result<u32, String> {
    static CACHE: OnceLock<std::sync::Mutex<Vec<(String, SetOpts, u32)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(x) = cache.lock().unwrap().iter().find(|x| x.0 == src && x.1 == *o) {
        return Ok(x.2);
    }
    let (maps, size_m): (Vec<(String, std::path::PathBuf)>, Option<f64>) = if crate::assetlib::is_ref(src) {
        let f = crate::assetlib::get(src)?;
        (f.files.iter().filter_map(|(r, p)| Some((r.clone()?, p.clone()))).collect(), f.entry.size_m)
    } else {
        let dir = Path::new(src);
        let rd = std::fs::read_dir(dir).map_err(|e| format!("texture `{src}`: {e} (expected an asset ref like `ambientcg:Wood049` or a folder of maps)"))?;
        (rd.filter_map(|e| e.ok()).filter_map(|e| Some((crate::assetlib::role_of(&e.file_name().to_string_lossy())?.to_string(), e.path()))).collect(), None)
    };
    let get = |role: &str| maps.iter().find(|m| m.0 == role).map(|m| m.1.clone());
    let ld = |role: &str, c: usize| -> Result<Option<Mips>, String> { get(role).map(|p| Mips::load(&p, c, o.max)).transpose() };
    let albedo = ld("albedo", 3)?;
    if albedo.is_none() {
        return Err(format!("texture `{src}` has no albedo/colour map"));
    }
    // A tile's real size: `scale` 1 shows the texture at its true size.
    let per_m = 1.0 / size_m.unwrap_or(1.0).max(0.05) as f32;
    let set = PbrSet {
        albedo,
        normal: ld("normal", 3)?,
        rough: ld("roughness", 1)?,
        ao: ld("ao", 1)?,
        metal: ld("metalness", 1)?,
        mapping: o.mapping,
        scale: o.scale * if o.mapping == Mapping::Triplanar { per_m } else { 1.0 },
        tint: o.tint,
        rough_mul: o.rough_mul,
        normal_strength: o.normal_strength,
        keep_rough: o.keep_rough,
        keep_metal: o.keep_metal,
        rotate: o.rotate,
    };
    let id = register(set);
    cache.lock().unwrap().push((src.to_string(), o.clone(), id));
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(n: usize) -> Mips {
        let mut d = vec![0u8; n * n * 3];
        for y in 0..n {
            for x in 0..n {
                let v = if (x + y) % 2 == 0 { 255 } else { 0 };
                d[(y * n + x) * 3..(y * n + x) * 3 + 3].copy_from_slice(&[v, v, v]);
            }
        }
        Mips::from_raw(n, n, 3, d)
    }

    #[test]
    fn mips_average_and_far_lookups_are_grey() {
        let m = checker(64);
        assert_eq!(m.levels.len(), 7);
        let far = m.sample(0.37, 0.61, 10.0);
        assert!((far[0] - 0.5).abs() < 0.02, "{far:?}");
        let near = m.sample(0.5 / 64.0, 0.5 / 64.0, 0.0);
        assert!(near[0] > 0.98);
    }

    #[test]
    fn triplanar_blends_and_flat_normals_stay_put() {
        let flat = Mips::from_raw(2, 2, 3, [128, 128, 255].repeat(4));
        let s = PbrSet { albedo: Some(checker(8)), normal: Some(flat), rough: None, ao: None, metal: None, mapping: Mapping::Triplanar, scale: 1.0, tint: v3(1.0, 1.0, 1.0), rough_mul: 1.0, normal_strength: 1.0, keep_rough: false, keep_metal: false, rotate: 0.0 };
        for n in [v3(0.0, 1.0, 0.0), v3(1.0, 0.0, 0.0), norm(v3(0.3, -0.5, 0.8))] {
            let x = s.sample(v3(0.1, 0.2, 0.3), n, [0.0, 0.0], v3(1.0, 0.0, 0.0), 0.0, |v| v);
            let got = x.n.unwrap();
            assert!(dot(got, n) > 0.999, "{n:?} → {got:?}");
            assert!(x.base.x >= 0.0 && x.base.x <= 1.0);
        }
    }
}
