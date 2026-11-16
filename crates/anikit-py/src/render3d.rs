//! Solid 3D rendering (`anikit_core::raster`) and the film grade / frame
//! utilities (`anikit_core::video`).
//!
//! Like the rest of the native module this is a thin layer: meshes, textures
//! and the rasteriser are native objects; materials and lighting cross as flat
//! float sequences in a fixed layout (see [`MATERIAL_LEN`] / [`LIGHTING_LEN`]);
//! bulk data crosses as native-endian `bytes`/`bytearray`. Frames come back as
//! a `bytearray` (one copy of the colour buffer) so numpy can wrap them
//! writable, and the film grade mutates a `bytearray` in place.

use pyo3::exceptions::{PyIndexError, PyMemoryError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes};

use anikit_core::raster::{self, Blend, Lighting, Material, Mesh, PointStyle, Raster, Texture, INSTANCE_STRIDE, M4};
use anikit_core::video::{self, FrameMixer, Order, Post, PostFx};

use crate::{f32_bytes, finite, u32_bytes, F32Data};

/// Floats per material: ambient, diffuse, specular, shininess, rim, emissive,
/// metal, iridescence, hue, lines, line_color(3), grid(2), line_width, pulse,
/// pulse_count, pulse_phase, pulse_dir(3), tex_mode, tex_repeat(2), tex_gain,
/// pattern (0 grid, 1 hex, 2 dots, 3 circuit).
const MATERIAL_LEN: usize = 29;
/// Floats per lighting: key(3), key_color(3), fill(3), fill_color(3),
/// rim_color(3), fog(2), fog_color(3), sky(3), horizon(3), ground(3), strips(3),
/// ground_fog(3: bottom, top, range), point light count, then 8 lights ×
/// (position(3), colour(3), radius, intensity).
const LIGHTING_LEN: usize = 100;
/// Largest vertex grid a mesh builder accepts (keeps typos from allocating gigabytes).
const MAX_CELLS: usize = 1 << 24;

/// Read-only byte input: `bytes` or `bytearray`.
#[derive(FromPyObject)]
enum U8Data<'py> {
    Bytes(Bound<'py, PyBytes>),
    ByteArray(Bound<'py, PyByteArray>),
}

impl U8Data<'_> {
    /// Run `f` on the bytes. `f` must not call into Python.
    fn with<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        match self {
            U8Data::Bytes(b) => f(b.as_bytes()),
            // SAFETY: the GIL is held and `f` runs no Python code, so the
            // bytearray cannot be resized or freed while it is borrowed.
            U8Data::ByteArray(b) => f(unsafe { b.as_bytes() }),
        }
    }
}

/// Flat `u32` input: native-endian `bytes` or a sequence of ints.
#[derive(FromPyObject)]
enum U32Data<'py> {
    Bytes(Bound<'py, PyBytes>),
    Seq(Vec<u32>),
}

impl U32Data<'_> {
    fn into_vec(self) -> PyResult<Vec<u32>> {
        match self {
            U32Data::Seq(v) => Ok(v),
            U32Data::Bytes(b) => {
                let raw = b.as_bytes();
                if !raw.len().is_multiple_of(4) {
                    return Err(PyValueError::new_err("u32 byte buffers must be a multiple of 4 bytes long"));
                }
                Ok(raw.chunks_exact(4).map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]])).collect())
            }
        }
    }
}

fn all_finite(name: &str, v: &[f32]) -> PyResult<()> {
    if v.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(PyValueError::new_err(format!("{name} must be finite")))
    }
}

fn check_cells(name: &str, a: usize, b: usize) -> PyResult<()> {
    match a.checked_mul(b) {
        Some(n) if n <= MAX_CELLS => Ok(()),
        _ => Err(PyValueError::new_err(format!("{name}: {a} x {b} is too large (at most {MAX_CELLS} cells)"))),
    }
}

fn check_frame(width: usize, height: usize) -> PyResult<usize> {
    if width == 0 || height == 0 {
        return Err(PyValueError::new_err(format!("frame size must be at least 1x1, got {width}x{height}")));
    }
    match width.checked_mul(height).and_then(|n| n.checked_mul(24)) {
        Some(b) if b <= isize::MAX as usize => Ok(width * height * 4),
        _ => Err(PyMemoryError::new_err("frame too large")),
    }
}

fn mat16(name: &str, m: [f32; 16]) -> PyResult<M4> {
    all_finite(name, &m)?;
    Ok(m)
}

// ---------------------------------------------------------------------------
// Meshes and textures
// ---------------------------------------------------------------------------

/// An immutable triangle mesh: positions and normals (xyz per vertex), UVs
/// (two per vertex, may be empty) and triangle indices.
#[pyclass(frozen, module = "anikit._anikit", name = "NativeMesh")]
pub struct NativeMesh {
    m: Mesh,
}

fn mesh(m: Mesh) -> NativeMesh {
    NativeMesh { m }
}

#[pymethods]
impl NativeMesh {
    /// Unit cube (−0.5…0.5), flat normals; each face has UVs 0…1.
    #[staticmethod]
    fn cube() -> NativeMesh {
        mesh(raster::cube())
    }

    /// Unit sphere.
    #[staticmethod]
    #[pyo3(signature = (rings=16, segments=24))]
    fn sphere(rings: usize, segments: usize) -> PyResult<NativeMesh> {
        check_cells("sphere", rings + 1, segments + 1)?;
        Ok(mesh(raster::sphere(rings.max(3), segments.max(3))))
    }

    /// A (p, q) torus knot tube: `radius` of the path, `spread` of its
    /// wobble, `tube` radius, and `segments × sides` resolution.
    #[staticmethod]
    #[pyo3(signature = (p=2.0, q=5.0, radius=300.0, spread=120.0, tube=40.0, segments=360, sides=16))]
    #[allow(clippy::too_many_arguments)]
    fn torus_knot(p: f32, q: f32, radius: f32, spread: f32, tube: f32, segments: usize, sides: usize) -> PyResult<NativeMesh> {
        all_finite("torus_knot parameters", &[p, q, radius, spread, tube])?;
        check_cells("torus_knot", segments + 1, sides + 1)?;
        Ok(mesh(raster::torus_knot(p, q, radius, spread, tube, segments.max(8), sides.max(3))))
    }

    /// A torus in the XY plane.
    #[staticmethod]
    #[pyo3(signature = (radius=300.0, tube=60.0, segments=96, sides=24))]
    fn torus(radius: f32, tube: f32, segments: usize, sides: usize) -> PyResult<NativeMesh> {
        all_finite("torus parameters", &[radius, tube])?;
        check_cells("torus", segments + 1, sides + 1)?;
        Ok(mesh(raster::torus(radius, tube, segments.max(8), sides.max(3))))
    }

    /// A flat grid in the XZ plane (−0.5…0.5) facing +Y with `nx × nz` cells.
    #[staticmethod]
    #[pyo3(signature = (nx=1, nz=1))]
    fn plane(nx: usize, nz: usize) -> PyResult<NativeMesh> {
        check_cells("plane", nx + 1, nz + 1)?;
        Ok(mesh(raster::plane(nx.max(1), nz.max(1))))
    }

    /// An open tube of `radius` along a polyline (`points`: flat xyz, at least
    /// two points) with twist-free frames; `radii` (one scale per point, or
    /// empty) thins it. `u` runs along it (0…1 by arc length), `v` around.
    #[staticmethod]
    #[pyo3(signature = (points, radii=None, radius=10.0, sides=8))]
    fn tube(points: F32Data<'_>, radii: Option<F32Data<'_>>, radius: f32, sides: usize) -> PyResult<NativeMesh> {
        let pts = points.into_vec()?;
        if !pts.len().is_multiple_of(3) {
            return Err(PyValueError::new_err("points must be flat [x, y, z] * n"));
        }
        let n = pts.len() / 3;
        if n < 2 {
            return Err(PyValueError::new_err(format!("tube needs at least 2 points, got {n}")));
        }
        let rs = match radii {
            Some(r) => r.into_vec()?,
            None => Vec::new(),
        };
        if !rs.is_empty() && rs.len() != n {
            return Err(PyValueError::new_err(format!("radii need one value per point ({n}), got {}", rs.len())));
        }
        check_cells("tube", n, sides.max(3) + 1)?;
        all_finite("points", &pts)?;
        all_finite("radii", &rs)?;
        finite("radius", radius)?;
        Ok(mesh(raster::tube_path(&pts, &rs, radius, sides.max(3))))
    }

    /// Capped cylinder of radius 1 and height 1 (Y −0.5…0.5).
    #[staticmethod]
    #[pyo3(signature = (segments=32, caps=true))]
    fn cylinder(segments: usize, caps: bool) -> PyResult<NativeMesh> {
        check_cells("cylinder", segments + 1, 4)?;
        Ok(mesh(raster::cylinder(segments.max(3), caps)))
    }

    /// Heightfield in the XZ plane from `width × depth` heights (row-major,
    /// row = z). Cells whose four corners are all `<= skip` are cut out.
    #[staticmethod]
    #[pyo3(signature = (width, depth, heights, skip=f32::NEG_INFINITY))]
    fn heightfield(width: usize, depth: usize, heights: F32Data<'_>, skip: f32) -> PyResult<NativeMesh> {
        if width < 2 || depth < 2 {
            return Err(PyValueError::new_err(format!("heightfield needs at least 2x2 samples, got {width}x{depth}")));
        }
        check_cells("heightfield", width, depth)?;
        let hs = heights.into_vec()?;
        if !hs.is_empty() && hs.len() != width * depth {
            return Err(PyValueError::new_err(format!("heightfield needs {} heights ({width} x {depth}), got {}", width * depth, hs.len())));
        }
        all_finite("heights", &hs)?;
        if skip.is_nan() {
            return Err(PyValueError::new_err("skip must not be NaN"));
        }
        Ok(mesh(raster::heightfield(width, depth, &hs, skip)))
    }

    /// Your own geometry: flat xyz `positions` and `normals`, three `indices`
    /// per triangle, optional uv pairs.
    #[staticmethod]
    #[pyo3(signature = (positions, normals, indices, uvs=None))]
    fn custom(positions: F32Data<'_>, normals: F32Data<'_>, indices: U32Data<'_>, uvs: Option<F32Data<'_>>) -> PyResult<NativeMesh> {
        let pos = positions.into_vec()?;
        let nrm = normals.into_vec()?;
        let idx = indices.into_vec()?;
        let uv = match uvs {
            Some(u) => u.into_vec()?,
            None => Vec::new(),
        };
        if !pos.len().is_multiple_of(3) {
            return Err(PyValueError::new_err("positions must be flat [x, y, z] * n"));
        }
        let nv = pos.len() / 3;
        if nrm.len() != pos.len() {
            return Err(PyValueError::new_err(format!("normals need {} values (one xyz per vertex), got {}", pos.len(), nrm.len())));
        }
        if !uv.is_empty() && uv.len() != nv * 2 {
            return Err(PyValueError::new_err(format!("uvs need {} values (one pair per vertex), got {}", nv * 2, uv.len())));
        }
        if !idx.len().is_multiple_of(3) {
            return Err(PyValueError::new_err("indices must be three per triangle"));
        }
        if let Some(&bad) = idx.iter().find(|&&i| i as usize >= nv) {
            return Err(PyIndexError::new_err(format!("index {bad} out of range ({nv} vertices)")));
        }
        all_finite("positions", &pos)?;
        all_finite("normals", &nrm)?;
        all_finite("uvs", &uv)?;
        Ok(mesh(Mesh { pos, nrm, uv, idx }))
    }

    #[getter]
    fn vertex_count(&self) -> usize {
        self.m.pos.len() / 3
    }

    #[getter]
    fn triangle_count(&self) -> usize {
        self.m.idx.len() / 3
    }

    /// Positions as native-endian `f32` bytes (`[x, y, z] * vertex_count`).
    fn positions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        f32_bytes(py, self.m.pos.iter().copied())
    }

    /// Normals as native-endian `f32` bytes.
    fn normals<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        f32_bytes(py, self.m.nrm.iter().copied())
    }

    /// UVs as native-endian `f32` bytes (empty when the mesh has none).
    fn uvs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        f32_bytes(py, self.m.uv.iter().copied())
    }

    /// Triangle indices as native-endian `u32` bytes.
    fn indices<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        u32_bytes(py, &self.m.idx)
    }

    fn __repr__(&self) -> String {
        format!("NativeMesh(vertices={}, triangles={})", self.vertex_count(), self.triangle_count())
    }
}

/// An RGBA image with a mip chain for material textures.
#[pyclass(frozen, module = "anikit._anikit", name = "NativeTexture")]
pub struct NativeTexture {
    t: Texture,
}

#[pymethods]
impl NativeTexture {
    /// From `width * height * 4` RGBA bytes, row 0 at the top (v = 0).
    #[new]
    fn new(width: usize, height: usize, data: U8Data<'_>) -> PyResult<Self> {
        let n = check_frame(width, height)?;
        let t = data.with(|raw| {
            if raw.len() != n {
                return Err(PyValueError::new_err(format!("texture needs {n} bytes ({width} x {height} x 4), got {}", raw.len())));
            }
            Ok(Texture::from_rgba(width, height, raw))
        })?;
        Ok(NativeTexture { t })
    }

    #[getter]
    fn width(&self) -> usize {
        self.t.width()
    }

    #[getter]
    fn height(&self) -> usize {
        self.t.height()
    }

    fn __repr__(&self) -> String {
        format!("NativeTexture({}x{})", self.t.width(), self.t.height())
    }
}

// ---------------------------------------------------------------------------
// The rasteriser
// ---------------------------------------------------------------------------

fn material(v: &[f32]) -> PyResult<Material> {
    if v.len() != MATERIAL_LEN {
        return Err(PyValueError::new_err(format!("material needs {MATERIAL_LEN} floats, got {}", v.len())));
    }
    all_finite("material", v)?;
    let tex_mode = v[22];
    if !(tex_mode == 0.0 || tex_mode == 1.0 || tex_mode == 2.0) {
        return Err(PyValueError::new_err(format!("texture mode must be 0, 1 or 2, got {tex_mode}")));
    }
    let pattern = v[26];
    if !(pattern == 0.0 || pattern == 1.0 || pattern == 2.0 || pattern == 3.0) {
        return Err(PyValueError::new_err(format!("pattern must be 0 (grid), 1 (hex), 2 (dots) or 3 (circuit), got {pattern}")));
    }
    let v3 = |i: usize| [v[i], v[i + 1], v[i + 2]];
    // Every field is set; `..Default` keeps this building if the core adds fields.
    #[allow(clippy::needless_update)]
    Ok(Material {
        ambient: v[0],
        diffuse: v[1],
        specular: v[2],
        shininess: v[3],
        rim: v[4],
        emissive: v[5],
        metal: v[6],
        iridescence: v[7],
        hue: v[8],
        lines: v[9],
        line_color: v3(10),
        grid: [v[13], v[14]],
        line_width: v[15],
        pulse: v[16],
        pulse_count: v[17],
        pulse_phase: v[18],
        pulse_dir: v3(19),
        tex_mode: tex_mode as u8,
        tex_repeat: [v[23], v[24]],
        tex_gain: v[25],
        pattern: pattern as u8,
        fog: v[27],
        tex_filter: if v[28] != 0.0 { 1 } else { 0 },
        ..Material::default()
    })
}

fn lighting(v: &[f32]) -> PyResult<Lighting> {
    if v.len() != LIGHTING_LEN {
        return Err(PyValueError::new_err(format!("lighting needs {LIGHTING_LEN} floats, got {}", v.len())));
    }
    // Fog distances may be infinite (no fog); everything else must be finite.
    if v.iter().any(|x| x.is_nan()) || v.iter().enumerate().any(|(i, x)| !(i == 15 || i == 16) && !x.is_finite()) {
        return Err(PyValueError::new_err("lighting values must be finite (fog distances may be inf)"));
    }
    let v3 = |i: usize| [v[i], v[i + 1], v[i + 2]];
    let far = if v[16] > 0.0 { v[16] } else { f32::INFINITY };
    #[allow(clippy::needless_update)]
    Ok(Lighting {
        key: v3(0),
        key_color: v3(3),
        fill: v3(6),
        fill_color: v3(9),
        rim_color: v3(12),
        fog: [v[15], far],
        fog_color: v3(17),
        sky: v3(20),
        horizon: v3(23),
        ground: v3(26),
        strips: v3(29),
        ground_fog: v3(32),
        point_lights: core::array::from_fn(|i| core::array::from_fn(|k| v[36 + i * 8 + k])),
        point_count: (v[35].max(0.0) as usize).min(8) as u8,
        ..Lighting::default()
    })
}

/// A z-buffered, lit, instanced triangle rasteriser with an RGBA colour buffer.
#[pyclass(module = "anikit._anikit", name = "NativeRaster")]
pub struct NativeRaster {
    r: Raster,
    inst: Vec<f32>,
    pts: Vec<f32>,
    rgb: Vec<f32>,
}

impl NativeRaster {
    /// Read `instances` into the reusable instance buffer (`None` = one
    /// identity white instance).
    fn read_instances(&mut self, instances: Option<F32Data<'_>>) -> PyResult<()> {
        match instances {
            Some(d) => {
                d.read_into(&mut self.inst)?;
                if !self.inst.len().is_multiple_of(INSTANCE_STRIDE) {
                    return Err(PyValueError::new_err(format!("instances must be {INSTANCE_STRIDE} floats each (3x4 affine + rgb)")));
                }
                all_finite("instances", &self.inst)?;
            }
            None => {
                self.inst.clear();
                self.inst.extend_from_slice(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
            }
        }
        Ok(())
    }
}

#[pymethods]
impl NativeRaster {
    #[new]
    fn new(width: usize, height: usize) -> PyResult<Self> {
        check_frame(width, height)?;
        Ok(NativeRaster { r: Raster::new(width, height), inst: Vec::new(), pts: Vec::new(), rgb: Vec::new() })
    }

    #[getter]
    fn width(&self) -> usize {
        self.r.width
    }

    #[getter]
    fn height(&self) -> usize {
        self.r.height
    }

    /// Triangles rasterised by the last draw.
    #[getter]
    fn drawn(&self) -> usize {
        self.r.drawn
    }

    /// Depth-only pre-pass before shading opaque draws (helps heavily
    /// self-overlapping single meshes).
    #[getter]
    fn get_prepass(&self) -> bool {
        self.r.prepass
    }

    #[setter]
    fn set_prepass(&mut self, v: bool) {
        self.r.prepass = v;
    }

    /// Rasterise only the pixel rectangle `(x0, y0, x1, y1)` (`x1`, `y1`
    /// exclusive), or everything (`None`). Pixels inside come out exactly as in
    /// a full draw; passes after the scene still run on the whole frame.
    #[getter]
    fn get_scissor(&self) -> Option<(usize, usize, usize, usize)> {
        self.r.scissor.map(|s| (s[0], s[1], s[2], s[3]))
    }

    #[setter]
    fn set_scissor(&mut self, v: Option<(usize, usize, usize, usize)>) {
        self.r.scissor = v.and_then(|(x0, y0, x1, y1)| {
            let (x1, y1) = (x1.min(self.r.width), y1.min(self.r.height));
            (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
        });
    }

    /// Resize (contents are cleared to transparent black).
    fn resize(&mut self, width: usize, height: usize) -> PyResult<()> {
        check_frame(width, height)?;
        self.r.resize(width, height);
        Ok(())
    }

    /// Clear colour to `rgb` (0xRRGGBB) with `alpha`, and depth.
    #[pyo3(signature = (rgb=0, alpha=255))]
    fn clear(&mut self, rgb: u32, alpha: u8) {
        self.r.clear(rgb & 0xff_ffff, alpha);
    }

    /// Reset depth only (between a mirrored pass and the real scene).
    fn clear_depth(&mut self) {
        self.r.clear_depth();
    }

    /// Start a half-resolution reflection pass: draws go to a half-size
    /// target cleared to `rgb`/`alpha` until :meth:`reflection_end`.
    #[pyo3(signature = (rgb=0, alpha=255))]
    fn reflection_begin(&mut self, rgb: u32, alpha: u8) {
        self.r.reflection_begin(rgb & 0xff_ffff, alpha);
    }

    /// End the reflection pass: upsample it into the frame and clear depth.
    fn reflection_end(&mut self) {
        self.r.reflection_end();
    }

    /// Draw `mesh` once per instance (`instances`: flat `f32`, 15 per instance:
    /// a column-major 3×4 affine then rgb; `None` = one identity white
    /// instance). `blend` is "opaque", "additive" or "gloss" (opaque blended
    /// over the colour buffer by `gloss`). Returns triangles rasterised.
    /// Releases the GIL while drawing.
    #[pyo3(signature = (mesh, instances, view_projection, eye, material, lighting, cull=true, blend="opaque", flip_winding=false, gloss=0.5, texture=None))]
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        py: Python<'_>,
        mesh: &Bound<'_, NativeMesh>,
        instances: Option<F32Data<'_>>,
        view_projection: [f32; 16],
        eye: (f32, f32, f32),
        material: Vec<f32>,
        lighting: Vec<f32>,
        cull: bool,
        blend: &str,
        flip_winding: bool,
        gloss: f32,
        texture: Option<&Bound<'_, NativeTexture>>,
    ) -> PyResult<usize> {
        let blend = match blend {
            "opaque" => Blend::Opaque,
            "additive" => Blend::Additive,
            "gloss" => Blend::Gloss,
            other => return Err(PyValueError::new_err(format!("blend must be 'opaque', 'additive' or 'gloss', got {other:?}"))),
        };
        let vp = mat16("view_projection", view_projection)?;
        let eye = [finite("eye", eye.0)?, finite("eye", eye.1)?, finite("eye", eye.2)?];
        let mat = self::material(&material)?;
        let light = self::lighting(&lighting)?;
        let gloss = finite("gloss", gloss)?.clamp(0.0, 1.0);
        self.read_instances(instances)?;
        let m = &mesh.get().m;
        let tex = texture.map(|t| &t.get().t);
        let (r, inst) = (self.r.target(), &self.inst);
        r.gloss = gloss;
        r.flip_winding = flip_winding;
        py.detach(|| r.draw_with(m, inst, &vp, eye, &mat, &light, cull, blend, tex));
        Ok(r.drawn)
    }

    /// Additive light points (`positions`: flat xyz; `colors`: flat rgb or
    /// `None` for white), depth-tested against what is drawn. With
    /// `prev_view_projection` and `streak > 0` points streak along their
    /// screen motion. `fog = (near, far)` fades them out (far inf = off).
    #[pyo3(signature = (positions, view_projection, eye, colors=None, prev_view_projection=None, size=2.0, intensity=1.0, streak=0.0, fog=(0.0, f32::INFINITY)))]
    #[allow(clippy::too_many_arguments)]
    fn points(
        &mut self,
        py: Python<'_>,
        positions: F32Data<'_>,
        view_projection: [f32; 16],
        eye: (f32, f32, f32),
        colors: Option<F32Data<'_>>,
        prev_view_projection: Option<[f32; 16]>,
        size: f32,
        intensity: f32,
        streak: f32,
        fog: (f32, f32),
    ) -> PyResult<()> {
        let vp = mat16("view_projection", view_projection)?;
        let prev = match prev_view_projection {
            Some(p) => Some(mat16("prev_view_projection", p)?),
            None => None,
        };
        let eye = [finite("eye", eye.0)?, finite("eye", eye.1)?, finite("eye", eye.2)?];
        if fog.0.is_nan() || fog.1.is_nan() {
            return Err(PyValueError::new_err("fog must not be NaN"));
        }
        let style = PointStyle {
            size: finite("size", size)?.max(0.0),
            intensity: finite("intensity", intensity)?,
            streak: finite("streak", streak)?.max(0.0),
            fog: [fog.0, if fog.1 > 0.0 { fog.1 } else { f32::INFINITY }],
        };
        positions.read_into(&mut self.pts)?;
        if !self.pts.len().is_multiple_of(3) {
            return Err(PyValueError::new_err("positions must be flat [x, y, z] * n"));
        }
        all_finite("positions", &self.pts)?;
        match colors {
            Some(c) => {
                c.read_into(&mut self.rgb)?;
                if self.rgb.len() != self.pts.len() {
                    return Err(PyValueError::new_err(format!("colors need {} values (rgb per point), got {}", self.pts.len(), self.rgb.len())));
                }
                all_finite("colors", &self.rgb)?;
            }
            None => self.rgb.clear(),
        }
        let (r, pts, rgb) = (&mut self.r, &self.pts, &self.rgb);
        py.detach(|| r.points(pts, rgb, &vp, prev.as_ref(), eye, &style));
        Ok(())
    }

    /// Start a key-light shadow map from the light's (orthographic)
    /// `light_view_projection` at `size × size` texels: draw casters with
    /// [`shadow_draw`](Self::shadow_draw), then the scene as usual; occluded
    /// pixels lose `strength` (0…1) of the key light. `clear()` ends it.
    #[pyo3(signature = (light_view_projection, size=2048, bias=0.0015, strength=0.8))]
    fn shadow_begin(&mut self, light_view_projection: [f32; 16], size: usize, bias: f32, strength: f32) -> PyResult<()> {
        let vp = mat16("light_view_projection", light_view_projection)?;
        let bias = finite("bias", bias)?;
        let strength = finite("strength", strength)?;
        self.r.shadow_begin(&vp, size, bias, strength);
        Ok(())
    }

    /// Render `mesh` instances (as for `draw`; `None` = one identity
    /// instance) into the shadow map started by `shadow_begin` (no culling).
    fn shadow_draw(&mut self, py: Python<'_>, mesh: &Bound<'_, NativeMesh>, instances: Option<F32Data<'_>>) -> PyResult<()> {
        self.read_instances(instances)?;
        let m = &mesh.get().m;
        let (r, inst) = (&mut self.r, &self.inst);
        py.detach(|| r.shadow_draw(m, inst, false));
        Ok(())
    }

    /// Fill every pixel no geometry covered with the environment (the
    /// lighting's sky / horizon / ground gradient, meeting the fog at the
    /// horizon) and, with `sun > 0`, a sun disc + halo towards `key`.
    #[pyo3(signature = (view_projection, lighting, sun=0.0))]
    fn sky(&mut self, py: Python<'_>, view_projection: [f32; 16], lighting: Vec<f32>, sun: f32) -> PyResult<()> {
        let vp = mat16("view_projection", view_projection)?;
        let light = self::lighting(&lighting)?;
        let sun = finite("sun", sun)?.max(0.0);
        let r = &mut self.r;
        py.detach(|| r.sky(&vp, &light, sun));
        Ok(())
    }

    /// Depth of field from the depth buffer: sharp at `focus` (distance from
    /// the camera), `aperture` px of blur at infinity, capped at `max_px`.
    fn depth_of_field(&mut self, py: Python<'_>, view_projection: [f32; 16], focus: f32, aperture: f32, max_px: f32) -> PyResult<()> {
        let vp = mat16("view_projection", view_projection)?;
        let (focus, aperture, max_px) = (finite("focus", focus)?, finite("aperture", aperture)?, finite("max_px", max_px)?);
        let r = &mut self.r;
        py.detach(|| r.depth_of_field(&vp, focus, aperture, max_px));
        Ok(())
    }

    /// Camera motion blur from the depth buffer against `prev_view_projection`
    /// (the previous frame's camera): `shutter` 0…1 of a frame, capped at `max_px`.
    fn motion_blur(&mut self, py: Python<'_>, view_projection: [f32; 16], prev_view_projection: [f32; 16], shutter: f32, max_px: f32) -> PyResult<()> {
        let vp = mat16("view_projection", view_projection)?;
        let prev = mat16("prev_view_projection", prev_view_projection)?;
        let (shutter, max_px) = (finite("shutter", shutter)?, finite("max_px", max_px)?);
        let r = &mut self.r;
        py.detach(|| r.motion_blur(&vp, &prev, shutter, max_px));
        Ok(())
    }

    /// The frame as a new `bytearray` of RGBA (`width * height * 4`, row-major).
    fn rgba<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyByteArray>> {
        PyByteArray::new_with(py, self.r.color.len() * 4, |buf| {
            for (chunk, px) in buf.chunks_exact_mut(4).zip(&self.r.color) {
                chunk.copy_from_slice(&px.to_le_bytes());
            }
            Ok(())
        })
    }

    /// `(r, g, b, a)` at pixel `(x, y)`.
    fn pixel(&self, x: usize, y: usize) -> PyResult<(u8, u8, u8, u8)> {
        if x >= self.r.width || y >= self.r.height {
            return Err(PyIndexError::new_err(format!("pixel ({x}, {y}) outside {}x{}", self.r.width, self.r.height)));
        }
        let [r, g, b, a] = self.r.color[y * self.r.width + x].to_le_bytes();
        Ok((r, g, b, a))
    }

    fn __repr__(&self) -> String {
        format!("NativeRaster({}x{})", self.r.width, self.r.height)
    }
}

// ---------------------------------------------------------------------------
// Film grade and frame utilities
// ---------------------------------------------------------------------------

fn rgb3(name: &str, v: Option<(f32, f32, f32)>, d: [f32; 3]) -> PyResult<[f32; 3]> {
    match v {
        None => Ok(d),
        Some((a, b, c)) => Ok([finite(name, a)?, finite(name, b)?, finite(name, c)?]),
    }
}

fn xy(name: &str, v: Option<(f32, f32)>, d: [f32; 2]) -> PyResult<[f32; 2]> {
    match v {
        None => Ok(d),
        Some((a, b)) => Ok([finite(name, a)?, finite(name, b)?]),
    }
}

fn opt(name: &str, v: Option<f32>, d: f32) -> PyResult<f32> {
    v.map_or(Ok(d), |x| finite(name, x))
}

/// Film-look post pass (bloom, streaks, god rays, zoom blur, edge
/// antialiasing, chromatic aberration, colour grade, vignette, grain) with reusable scratch buffers.
#[pyclass(module = "anikit._anikit", name = "NativePostFx")]
#[derive(Default)]
pub struct NativePostFx {
    fx: PostFx,
}

#[pymethods]
impl NativePostFx {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Grade `frame` (an RGBA `bytearray` of `width * height * 4`) in place.
    /// Omitted options keep the core defaults (all effects off).
    #[pyo3(signature = (
        frame, width, height, *, seed=0, bloom=None, bloom_threshold=None, bloom_radius=None, aberration=None,
        vignette=None, grain=None, streaks=None, streak_length=None, streak_tint=None, zoom_blur=None,
        zoom_center=None, rays=None, rays_center=None, rays_length=None, aa=None, flares=None, shock=None, exposure=None, contrast=None,
        saturation=None, shadows=None, highlights=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        frame: &Bound<'_, PyByteArray>,
        width: usize,
        height: usize,
        seed: u32,
        bloom: Option<f32>,
        bloom_threshold: Option<f32>,
        bloom_radius: Option<f32>,
        aberration: Option<f32>,
        vignette: Option<f32>,
        grain: Option<f32>,
        streaks: Option<f32>,
        streak_length: Option<f32>,
        streak_tint: Option<(f32, f32, f32)>,
        zoom_blur: Option<f32>,
        zoom_center: Option<(f32, f32)>,
        rays: Option<f32>,
        rays_center: Option<(f32, f32)>,
        rays_length: Option<f32>,
        aa: Option<f32>,
        flares: Option<f32>,
        shock: Option<(f32, f32, f32, f32, f32)>,
        exposure: Option<f32>,
        contrast: Option<f32>,
        saturation: Option<f32>,
        shadows: Option<(f32, f32, f32)>,
        highlights: Option<(f32, f32, f32)>,
    ) -> PyResult<()> {
        let n = check_frame(width, height)?;
        if frame.len() != n {
            return Err(PyValueError::new_err(format!("frame needs {n} bytes ({width} x {height} x 4), got {}", frame.len())));
        }
        let d = Post::default();
        #[allow(clippy::needless_update)]
        let p = Post {
            bloom_threshold: opt("bloom_threshold", bloom_threshold, d.bloom_threshold)?,
            bloom: opt("bloom", bloom, d.bloom)?,
            bloom_radius: opt("bloom_radius", bloom_radius, d.bloom_radius)?.max(0.0),
            aberration: opt("aberration", aberration, d.aberration)?,
            vignette: opt("vignette", vignette, d.vignette)?,
            grain: opt("grain", grain, d.grain)?,
            seed,
            streaks: opt("streaks", streaks, d.streaks)?,
            streak_length: opt("streak_length", streak_length, d.streak_length)?.max(0.0),
            streak_tint: rgb3("streak_tint", streak_tint, d.streak_tint)?,
            zoom_blur: opt("zoom_blur", zoom_blur, d.zoom_blur)?,
            zoom_center: xy("zoom_center", zoom_center, d.zoom_center)?,
            rays: opt("rays", rays, d.rays)?,
            rays_center: xy("rays_center", rays_center, d.rays_center)?,
            rays_length: opt("rays_length", rays_length, d.rays_length)?,
            aa: opt("aa", aa, d.aa)?,
            flares: opt("flares", flares, d.flares)?,
            shock: match shock {
                Some((x, y, r, hw, k)) => [finite("shock", x)?, finite("shock", y)?, finite("shock", r)?.max(0.0), finite("shock", hw)?.max(0.0), finite("shock", k)?],
                None => d.shock,
            },
            exposure: opt("exposure", exposure, d.exposure)?,
            contrast: opt("contrast", contrast, d.contrast)?,
            saturation: opt("saturation", saturation, d.saturation)?,
            shadows: rgb3("shadows", shadows, d.shadows)?,
            highlights: rgb3("highlights", highlights, d.highlights)?,
            ..d
        };
        // SAFETY: the GIL is held for the whole call and the post pass runs no
        // Python code, so nothing else can resize, free or touch the bytearray.
        let buf = unsafe { frame.as_bytes_mut() };
        self.fx.apply(buf, width, height, &p);
        Ok(())
    }

    fn __repr__(&self) -> String {
        "NativePostFx()".to_string()
    }
}

/// Convert an RGBA (or BGRA, `order="bgra"`) frame to planar I420 (Y, U, V;
/// BT.709 limited range). Width and height must be even.
#[pyfunction]
#[pyo3(signature = (frame, width, height, order="rgba"))]
fn rgba_to_i420<'py>(py: Python<'py>, frame: U8Data<'_>, width: usize, height: usize, order: &str) -> PyResult<Bound<'py, PyBytes>> {
    let order = match order {
        "rgba" => Order::Rgba,
        "bgra" => Order::Bgra,
        other => return Err(PyValueError::new_err(format!("order must be 'rgba' or 'bgra', got {other:?}"))),
    };
    let n = check_frame(width, height)?;
    frame.with(|src| {
        if src.len() != n {
            return Err(PyValueError::new_err(format!("frame needs {n} bytes ({width} x {height} x 4), got {}", src.len())));
        }
        PyBytes::new_with(py, video::i420_len(width, height), |out| {
            video::rgba_to_i420(src, width, height, order, out);
            Ok(())
        })
    })
}

/// Bytes in an I420 frame of `width × height`.
#[pyfunction]
fn i420_len(width: usize, height: usize) -> usize {
    video::i420_len(width, height)
}

/// Averages RGBA sub-frames (motion blur): add up to 257 frames, resolve.
#[pyclass(module = "anikit._anikit", name = "NativeFrameMixer")]
pub struct NativeFrameMixer {
    m: FrameMixer,
}

#[pymethods]
impl NativeFrameMixer {
    #[new]
    fn new(width: usize, height: usize) -> PyResult<Self> {
        check_frame(width, height)?;
        Ok(NativeFrameMixer { m: FrameMixer::new(width, height) })
    }

    #[getter]
    fn width(&self) -> usize {
        self.m.width
    }

    #[getter]
    fn height(&self) -> usize {
        self.m.height
    }

    /// Frames accumulated since the last reset.
    #[getter]
    fn count(&self) -> u32 {
        self.m.count
    }

    /// Accumulate one RGBA frame; returns the count.
    fn add(&mut self, frame: U8Data<'_>) -> PyResult<u32> {
        if self.m.count >= 257 {
            return Err(PyValueError::new_err("at most 257 frames can be mixed; resolve() first"));
        }
        let n = self.m.width * self.m.height * 4;
        let m = &mut self.m;
        frame.with(|src| {
            if src.len() != n {
                return Err(PyValueError::new_err(format!("frame needs {n} bytes, got {}", src.len())));
            }
            m.add(src);
            Ok(())
        })?;
        Ok(self.m.count)
    }

    /// The average of the accumulated frames as a new RGBA `bytearray`; resets.
    fn resolve<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyByteArray>> {
        let out = PyByteArray::new(py, self.m.resolve());
        self.m.reset();
        Ok(out)
    }

    /// Drop the accumulated frames.
    fn reset(&mut self) {
        self.m.reset();
    }

    fn __repr__(&self) -> String {
        format!("NativeFrameMixer({}x{}, count={})", self.m.width, self.m.height, self.m.count)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NativeMesh>()?;
    m.add_class::<NativeTexture>()?;
    m.add_class::<NativeRaster>()?;
    m.add_class::<NativePostFx>()?;
    m.add_class::<NativeFrameMixer>()?;
    m.add_function(wrap_pyfunction!(rgba_to_i420, m)?)?;
    m.add_function(wrap_pyfunction!(i420_len, m)?)?;
    m.add("RASTER_INSTANCE_STRIDE", INSTANCE_STRIDE)?;
    m.add("RASTER_MATERIAL_LEN", MATERIAL_LEN)?;
    m.add("RASTER_LIGHTING_LEN", LIGHTING_LEN)?;
    Ok(())
}
