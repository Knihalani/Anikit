//! Python bindings for `anikit-core`.
//!
//! This is deliberately a *thin, low-level* layer: handles are plain integers,
//! times are milliseconds (exactly like the core), track specifications are
//! tuples and outputs are flat lists of floats. The ergonomic, seconds-based
//! API lives in the pure-Python `anikit` package on top of this module.
//!
//! Performance notes:
//!
//! * One [`Engine::tick`] call advances everything; [`Engine::drain`] returns
//!   the handles that changed, their values and the frame's events in a
//!   single call so the Python side never polls idle animations.
//! * Easings, paths and morph plans are native objects, parsed once.

use pyo3::exceptions::{PyIndexError, PyKeyError, PyMemoryError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes};

mod render3d;

use anikit_core::cloud::Cloud;
use anikit_core::color::{self, ColorSpace, Rgba};
use anikit_core::css;
use anikit_core::easing::Easing;
use anikit_core::engine::{self as core_engine, Error, Handle, PlayState, TrackKind, TrackSpec, LINEAR, NONE};
use anikit_core::morph::{self, MorphPlan, MorphSeq};
use anikit_core::noise::Noise;
use anikit_core::path::{Path, PathLut};
use anikit_core::space::{self, Mat4, Projector};
use anikit_core::splat::{Splat, SplatParams};
use anikit_core::spring::SpringParams;
use anikit_core::stagger::{self as core_stagger, Axis, Stagger, StaggerFrom};
use anikit_core::timing::{Direction, Timing};
use anikit_core::world::{Attractor, World, WorldConfig};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

fn to_py(e: Error) -> PyErr {
    match e {
        Error::BadHandle => PyKeyError::new_err("invalid or released animation handle"),
        Error::WrongKind => PyTypeError::new_err("operation not supported for this kind of node"),
        Error::InvalidInput => PyValueError::new_err("invalid input"),
        Error::Capacity => PyMemoryError::new_err("animation engine node capacity exhausted"),
        Error::NotRoot => PyRuntimeError::new_err("operation requires a root node (this node belongs to a timeline)"),
    }
}

trait IntoPy<T> {
    fn py(self) -> PyResult<T>;
}

impl<T> IntoPy<T> for Result<T, Error> {
    #[inline]
    fn py(self) -> PyResult<T> {
        self.map_err(to_py)
    }
}

fn parse_easing(spec: &str) -> PyResult<Easing> {
    Easing::parse(spec).ok_or_else(|| PyValueError::new_err(format!("unknown easing: {spec:?}")))
}

fn parse_space(name: &str) -> PyResult<ColorSpace> {
    ColorSpace::parse(name).ok_or_else(|| PyValueError::new_err(format!("unknown colour space: {name:?}")))
}

fn direction(d: u8) -> PyResult<Direction> {
    if d > 3 {
        return Err(PyValueError::new_err("direction must be 0 (normal), 1 (reverse), 2 (alternate) or 3 (alternate-reverse)"));
    }
    Ok(Direction::from_u8(d))
}

type Rgba4 = (f64, f64, f64, f64);

#[inline]
fn rgba(c: Rgba4) -> Rgba {
    Rgba::new(c.0, c.1, c.2, c.3)
}

#[inline]
fn tuple4(c: Rgba) -> Rgba4 {
    (c.r, c.g, c.b, c.a)
}

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

/// A parsed easing curve (any syntax the core understands).
#[pyclass(frozen, module = "anikit._anikit", name = "NativeEasing")]
pub struct NativeEasing {
    e: Easing,
}

#[pymethods]
impl NativeEasing {
    #[new]
    fn new(spec: &str) -> PyResult<Self> {
        Ok(NativeEasing { e: parse_easing(spec)? })
    }

    /// Lookup-table easing from uniform samples over `[0, 1]`.
    #[staticmethod]
    fn table(values: Vec<f64>) -> PyResult<Self> {
        if values.len() < 2 || values.iter().any(|v| !v.is_finite()) {
            return Err(PyValueError::new_err("an easing table needs at least 2 finite samples"));
        }
        Ok(NativeEasing { e: Easing::Table(values.into_boxed_slice()) })
    }

    /// CSS `linear()`-style easing from `(x, y)` stops.
    #[staticmethod]
    fn points(stops: Vec<(f64, f64)>) -> PyResult<Self> {
        if stops.is_empty() {
            return Err(PyValueError::new_err("points() needs at least one stop"));
        }
        Ok(NativeEasing { e: Easing::points(&stops) })
    }

    #[staticmethod]
    fn bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        NativeEasing { e: Easing::bezier(x1, y1, x2, y2) }
    }

    /// Duration-normalised spring easing.
    #[staticmethod]
    #[pyo3(signature = (stiffness=100.0, damping=10.0, mass=1.0, velocity=0.0))]
    fn spring(stiffness: f64, damping: f64, mass: f64, velocity: f64) -> Self {
        NativeEasing { e: Easing::spring(SpringParams::new(stiffness, damping, mass), velocity) }
    }

    /// "CustomEase": an easing drawn as an SVG path from (0, 0) to (1, 1)
    /// (paths in other coordinate ranges are normalised by their end points).
    #[staticmethod]
    fn from_path(d: &str) -> PyResult<Self> {
        Easing::from_path(d)
            .map(|e| NativeEasing { e })
            .ok_or_else(|| PyValueError::new_err(format!("cannot build an easing from path {d:?}")))
    }

    /// Seeded jittery easing around `template` (linear by default).
    #[staticmethod]
    #[pyo3(signature = (points=20, strength=1.0, seed=1, template=None))]
    fn rough(points: usize, strength: f64, seed: u32, template: Option<PyRef<'_, NativeEasing>>) -> PyResult<Self> {
        if !strength.is_finite() {
            return Err(PyValueError::new_err("rough() strength must be finite"));
        }
        let t = template.map(|t| t.e.clone()).unwrap_or(Easing::Linear);
        Ok(NativeEasing { e: Easing::rough(points, strength, seed, &t) })
    }

    /// Weighted mix `a·(1-weight) + b·weight`.
    #[staticmethod]
    #[pyo3(signature = (a, b, weight=0.5))]
    fn blend(a: PyRef<'_, NativeEasing>, b: PyRef<'_, NativeEasing>, weight: f64) -> Self {
        NativeEasing { e: Easing::Blend(Box::new(a.e.clone()), Box::new(b.e.clone()), weight) }
    }

    /// Time-reversed easing `1 - f(1 - t)` (turns "in" into "out").
    fn reversed(&self) -> Self {
        NativeEasing { e: Easing::Reverse(Box::new(self.e.clone())) }
    }

    /// There-and-back easing: `f(2t)` then `f(2 - 2t)`.
    fn mirrored(&self) -> Self {
        NativeEasing { e: Easing::Mirror(Box::new(self.e.clone())) }
    }

    fn __call__(&self, t: f64) -> f64 {
        self.e.ease(t)
    }

    fn ease(&self, t: f64) -> f64 {
        self.e.ease(t)
    }

    /// Evaluate at many progress values at once.
    fn ease_many(&self, ts: Vec<f64>) -> Vec<f64> {
        ts.into_iter().map(|t| self.e.ease(t)).collect()
    }

    /// `n` uniform samples over `[0, 1]` (both ends included).
    fn sample(&self, n: usize) -> PyResult<Vec<f64>> {
        if n == 0 {
            return Err(PyValueError::new_err("sample count must be >= 1"));
        }
        let mut out = vec![0.0; n];
        self.e.sample_into(&mut out);
        Ok(out)
    }

    /// Most compact native CSS `<easing-function>` (springs → `linear()`).
    #[pyo3(signature = (tolerance=0.002))]
    fn css(&self, tolerance: f64) -> String {
        css::to_css(&self.e, tolerance)
    }

    /// Minimal `(x, y)` polyline approximating the curve within `tolerance`.
    #[pyo3(signature = (tolerance=0.002))]
    fn linear_points(&self, tolerance: f64) -> Vec<(f64, f64)> {
        css::linear_points(&self.e, tolerance)
    }

    /// Natural duration in seconds (springs only).
    #[getter]
    fn duration(&self) -> Option<f64> {
        self.e.natural_duration()
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match &self.e {
            Easing::Linear => "linear",
            Easing::Curve(..) => "curve",
            Easing::Bezier(_) => "bezier",
            Easing::Steps(..) => "steps",
            Easing::Spring { .. } => "spring",
            Easing::Points(_) => "points",
            Easing::Table(_) => "table",
            Easing::Wiggle { .. } => "wiggle",
            Easing::SlowMo { .. } => "slowmo",
            Easing::Anticipate(_) => "anticipate",
            Easing::Reverse(_) => "reverse",
            Easing::Mirror(_) => "mirror",
            Easing::Blend(..) => "blend",
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        match other.cast::<NativeEasing>() {
            Ok(o) => o.get().e == self.e,
            Err(_) => false,
        }
    }

    fn __hash__(&self) -> u64 {
        // Easings are rarely hashed; a coarse fingerprint keeps hash/eq consistent.
        let mut h: u64 = 1469598103934665603;
        for i in 0..=8 {
            let v = self.e.ease(i as f64 / 8.0).to_bits();
            h = (h ^ v).wrapping_mul(1099511628211);
        }
        h
    }

    fn __repr__(&self) -> String {
        format!("NativeEasing<{}>", self.kind())
    }
}

/// Evaluate an easing given by its string syntax.
#[pyfunction]
fn ease(spec: &str, t: f64) -> PyResult<f64> {
    Ok(parse_easing(spec)?.ease(t))
}

/// Native CSS form of an easing given by its string syntax.
#[pyfunction]
#[pyo3(signature = (spec, tolerance=0.002))]
fn easing_css(spec: &str, tolerance: f64) -> PyResult<String> {
    Ok(css::to_css(&parse_easing(spec)?, tolerance))
}

// ---------------------------------------------------------------------------
// Springs
// ---------------------------------------------------------------------------

/// Time (s) after which a spring released from 0 towards 1 stays within `epsilon`.
#[pyfunction]
#[pyo3(signature = (stiffness=100.0, damping=10.0, mass=1.0, velocity=0.0, epsilon=0.001))]
fn spring_settle_time(stiffness: f64, damping: f64, mass: f64, velocity: f64, epsilon: f64) -> f64 {
    SpringParams::new(stiffness, damping, mass).settle_time(velocity, epsilon.max(1e-9))
}

/// Normalised spring response (0 → 1) at time `t` seconds.
#[pyfunction]
#[pyo3(signature = (t, stiffness=100.0, damping=10.0, mass=1.0, velocity=0.0))]
fn spring_response(t: f64, stiffness: f64, damping: f64, mass: f64, velocity: f64) -> f64 {
    SpringParams::new(stiffness, damping, mass).response(t, velocity)
}

/// Convert perceptual `(duration s, bounce 0–1)` to `(stiffness, damping, mass)`.
#[pyfunction]
fn spring_from_duration_bounce(duration: f64, bounce: f64) -> (f64, f64, f64) {
    let p = SpringParams::from_duration_bounce(duration, bounce);
    (p.stiffness, p.damping, p.mass)
}

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

/// Parse any CSS colour into `(r, g, b, a)` with r/g/b in 0–255 and a in 0–1.
#[pyfunction]
fn color_parse(s: &str) -> PyResult<Rgba4> {
    color::parse(s).map(tuple4).ok_or_else(|| PyValueError::new_err(format!("not a CSS colour: {s:?}")))
}

/// Like [`color_parse`] but returns `None` instead of raising.
#[pyfunction]
fn color_try_parse(s: &str) -> Option<Rgba4> {
    color::parse(s).map(tuple4)
}

#[pyfunction]
#[pyo3(signature = (a, b, t, space="oklab"))]
fn color_mix(a: Rgba4, b: Rgba4, t: f64, space: &str) -> PyResult<Rgba4> {
    Ok(tuple4(color::mix(rgba(a), rgba(b), t, parse_space(space)?)))
}

#[pyfunction]
fn color_to_css(c: Rgba4) -> String {
    color::to_css(rgba(c))
}

#[pyfunction]
fn color_to_hex(c: Rgba4) -> String {
    color::to_hex(rgba(c))
}

/// Numeric id of a colour space name (as used in track specs).
#[pyfunction]
fn color_space_id(name: &str) -> PyResult<u8> {
    Ok(parse_space(name)? as u8)
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Parsed SVG path (normalised to absolute cubic segments).
#[pyclass(module = "anikit._anikit", name = "NativePath")]
pub struct NativePath {
    p: Path,
    lut: Option<PathLut>,
}

impl NativePath {
    fn wrap(p: Path) -> Self {
        NativePath { p, lut: None }
    }

    fn lut(&mut self) -> &PathLut {
        if self.lut.is_none() {
            let n = ((self.p.length() / 1.5) as usize).clamp(512, 4096);
            self.lut = Some(PathLut::new(&self.p, n));
        }
        self.lut.as_ref().expect("lut built above")
    }
}

#[pymethods]
impl NativePath {
    #[new]
    fn new(d: &str) -> PyResult<Self> {
        let p = Path::parse(d);
        if p.subpaths.is_empty() && !d.trim().is_empty() {
            return Err(PyValueError::new_err(format!("invalid SVG path data: {d:?}")));
        }
        Ok(NativePath::wrap(p))
    }

    #[staticmethod]
    #[pyo3(signature = (x, y, width, height, rx=0.0, ry=None))]
    fn rect(x: f64, y: f64, width: f64, height: f64, rx: f64, ry: Option<f64>) -> Self {
        NativePath::wrap(Path::rect(x, y, width, height, rx, ry.unwrap_or(rx)))
    }

    #[staticmethod]
    fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Self {
        NativePath::wrap(Path::ellipse(cx, cy, rx, ry))
    }

    /// Polyline / polygon from flat `[x0, y0, x1, y1, ...]`.
    #[staticmethod]
    #[pyo3(signature = (points, closed=true))]
    fn polygon(points: Vec<f64>, closed: bool) -> PyResult<Self> {
        if !points.len().is_multiple_of(2) {
            return Err(PyValueError::new_err("polygon points must be flat x, y pairs"));
        }
        Ok(NativePath::wrap(Path::polygon(&points, closed)))
    }

    #[staticmethod]
    #[pyo3(signature = (cx, cy, points, outer, inner, rotation=0.0))]
    fn star(cx: f64, cy: f64, points: usize, outer: f64, inner: f64, rotation: f64) -> Self {
        NativePath::wrap(Path::star(cx, cy, points, outer, inner, rotation))
    }

    #[getter]
    fn length(&mut self) -> f64 {
        self.lut().length
    }

    /// `(min_x, min_y, max_x, max_y)`.
    #[getter]
    fn bounds(&self) -> (f64, f64, f64, f64) {
        let b = self.p.bounds();
        (b[0], b[1], b[2], b[3])
    }

    #[getter]
    fn segment_count(&self) -> usize {
        self.p.segment_count()
    }

    #[getter]
    fn subpath_count(&self) -> usize {
        self.p.subpaths.len()
    }

    /// `(x, y, angle_deg)` at fraction `t` of the total length.
    fn point_at(&mut self, t: f64) -> (f64, f64, f64) {
        let p = self.lut().sample(t);
        (p[0], p[1], p[2])
    }

    fn points_at(&mut self, ts: Vec<f64>) -> Vec<(f64, f64, f64)> {
        let lut = self.lut();
        ts.into_iter()
            .map(|t| {
                let p = lut.sample(t);
                (p[0], p[1], p[2])
            })
            .collect()
    }

    #[pyo3(signature = (decimals=3))]
    fn to_svg(&self, decimals: u32) -> String {
        self.p.to_svg(decimals.min(12))
    }

    /// New path transformed by the SVG matrix `(a, b, c, d, e, f)`.
    fn transformed(&self, m: [f64; 6]) -> Self {
        let mut p = self.p.clone();
        p.transform(m);
        NativePath::wrap(p)
    }

    fn reversed(&self) -> Self {
        let mut p = self.p.clone();
        p.reverse();
        NativePath::wrap(p)
    }

    /// Polylines (one per subpath) deviating at most `tolerance` from the curve.
    #[pyo3(signature = (tolerance=0.25))]
    fn flatten(&self, tolerance: f64) -> PyResult<Vec<Vec<(f64, f64)>>> {
        if !(tolerance.is_finite() && tolerance > 0.0) {
            return Err(PyValueError::new_err("tolerance must be a positive number"));
        }
        Ok(self.p.flatten(tolerance).into_iter().map(|poly| poly.into_iter().map(|q| (q[0], q[1])).collect()).collect())
    }

    /// Signed area (positive = clockwise in SVG's y-down space).
    #[getter]
    fn area(&self) -> f64 {
        self.p.area()
    }

    /// The part between fractions `start` and `end` of the total length.
    fn trim(&self, start: f64, end: f64) -> Self {
        NativePath::wrap(self.p.trim(start, end))
    }

    /// Ramer–Douglas–Peucker simplification (lines, or a spline with `smooth`).
    #[pyo3(signature = (tolerance=1.0, smooth=false))]
    fn simplify(&self, tolerance: f64, smooth: bool) -> PyResult<Self> {
        if !(tolerance.is_finite() && tolerance > 0.0) {
            return Err(PyValueError::new_err("tolerance must be a positive number"));
        }
        Ok(NativePath::wrap(self.p.simplify(tolerance, smooth)))
    }

    /// Catmull–Rom spline through flat `[x0, y0, x1, y1, ...]`.
    #[staticmethod]
    #[pyo3(signature = (points, curviness=1.0, closed=false))]
    fn through(points: Vec<f64>, curviness: f64, closed: bool) -> PyResult<Self> {
        if !points.len().is_multiple_of(2) {
            return Err(PyValueError::new_err("points must be flat x, y pairs"));
        }
        if points.len() < 4 {
            return Err(PyValueError::new_err("a path through points needs at least 2 points"));
        }
        Ok(NativePath::wrap(Path::through_points(&points, curviness, closed)))
    }

    /// `(x, y, angle_deg)` at absolute distance `l` along the path.
    fn point_at_length(&mut self, l: f64) -> (f64, f64, f64) {
        let p = self.lut().sample_length(l);
        (p[0], p[1], p[2])
    }

    /// Closest point to `(x, y)`: `(progress, x, y, distance)`.
    fn closest(&mut self, x: f64, y: f64) -> (f64, f64, f64, f64) {
        let r = self.lut().closest(x, y);
        (r[0], r[1], r[2], r[3])
    }

    /// `n` `(x, y)` points evenly spaced by distance (both ends included).
    fn sample_points(&mut self, n: usize) -> Vec<(f64, f64)> {
        self.lut().points(n).chunks_exact(2).map(|c| (c[0], c[1])).collect()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        match other.cast::<NativePath>() {
            Ok(o) => o.borrow().p == self.p,
            Err(_) => false,
        }
    }

    fn __repr__(&self) -> String {
        let mut s = self.p.to_svg(3);
        if s.len() > 60 {
            s.truncate(57);
            s.push_str("...");
        }
        format!("NativePath({s:?})")
    }
}

// ---------------------------------------------------------------------------
// Morphing
// ---------------------------------------------------------------------------

/// Two paths normalised to the same structure; interpolate `from_values` →
/// `to_values` numerically and format with [`NativeMorph::write`].
#[pyclass(frozen, module = "anikit._anikit", name = "NativeMorph")]
pub struct NativeMorph {
    plan: MorphPlan,
}

#[pymethods]
impl NativeMorph {
    #[new]
    fn new(a: PyRef<'_, NativePath>, b: PyRef<'_, NativePath>) -> Self {
        NativeMorph { plan: morph::prepare(&a.p, &b.p) }
    }

    #[getter(from_values)]
    fn start_values(&self) -> Vec<f64> {
        self.plan.from.clone()
    }

    #[getter(to_values)]
    fn end_values(&self) -> Vec<f64> {
        self.plan.to.clone()
    }

    /// Per subpath `(segment_count, closed)`.
    #[getter]
    fn template(&self) -> Vec<(usize, bool)> {
        self.plan.template.clone()
    }

    #[getter]
    fn channels(&self) -> usize {
        self.plan.channels()
    }

    #[pyo3(signature = (t, decimals=3))]
    fn at(&self, t: f64, decimals: u32) -> String {
        let v: Vec<f64> = self.plan.from.iter().zip(&self.plan.to).map(|(a, b)| a + (b - a) * t).collect();
        self.plan.write(&v, decimals.min(12))
    }

    /// Format already-interpolated channel values as path data.
    #[pyo3(signature = (values, decimals=3))]
    fn write(&self, values: Vec<f64>, decimals: u32) -> PyResult<String> {
        if values.len() != self.plan.channels() {
            return Err(PyValueError::new_err(format!(
                "expected {} values, got {}",
                self.plan.channels(),
                values.len()
            )));
        }
        Ok(self.plan.write(&values, decimals.min(12)))
    }

    fn __repr__(&self) -> String {
        format!("NativeMorph(subpaths={}, channels={})", self.plan.template.len(), self.plan.channels())
    }
}

/// Format channel values with an explicit morph template.
#[pyfunction]
#[pyo3(signature = (template, values, decimals=3))]
fn morph_write(template: Vec<(usize, bool)>, values: Vec<f64>, decimals: u32) -> String {
    morph::write_template(&template, &values, decimals.min(12))
}

/// Any number of shapes normalised to one structure: `frames[k]` are the
/// channel values of shape `k`; interpolate between them and format with
/// [`NativeMorphSeq::write`].
#[pyclass(frozen, module = "anikit._anikit", name = "NativeMorphSeq")]
pub struct NativeMorphSeq {
    seq: MorphSeq,
}

#[pymethods]
impl NativeMorphSeq {
    #[new]
    fn new(paths: Vec<PyRef<'_, NativePath>>) -> PyResult<Self> {
        if paths.len() < 2 {
            return Err(PyValueError::new_err("a morph needs at least 2 shapes"));
        }
        let ps: Vec<Path> = paths.iter().map(|p| p.p.clone()).collect();
        Ok(NativeMorphSeq { seq: morph::prepare_many(&ps) })
    }

    /// One channel vector per shape.
    #[getter]
    fn frames(&self) -> Vec<Vec<f64>> {
        self.seq.frames.clone()
    }

    /// Per subpath `(segment_count, closed)`.
    #[getter]
    fn template(&self) -> Vec<(usize, bool)> {
        self.seq.template.clone()
    }

    /// Template flattened to `[segments, closed, segments, closed, ...]`.
    fn template_flat(&self) -> Vec<f64> {
        self.seq.template_flat()
    }

    #[getter]
    fn channels(&self) -> usize {
        self.seq.channels()
    }

    fn __len__(&self) -> usize {
        self.seq.frames.len()
    }

    /// Path data at sequence progress `t` in `[0, 1]` (linear between shapes).
    #[pyo3(signature = (t, decimals=3))]
    fn at(&self, t: f64, decimals: u32) -> String {
        let decimals = decimals.min(12);
        let f = &self.seq.frames;
        let n = f.len();
        let x = t.clamp(0.0, 1.0) * (n - 1) as f64;
        let i = (x as usize).min(n - 2);
        let k = x - i as f64;
        let v: Vec<f64> = f[i].iter().zip(&f[i + 1]).map(|(a, b)| a + (b - a) * k).collect();
        morph::write_template(&self.seq.template, &v, decimals)
    }

    /// Format already-interpolated channel values as path data.
    #[pyo3(signature = (values, decimals=3))]
    fn write(&self, values: Vec<f64>, decimals: u32) -> PyResult<String> {
        if values.len() != self.seq.channels() {
            return Err(PyValueError::new_err(format!(
                "expected {} values, got {}",
                self.seq.channels(),
                values.len()
            )));
        }
        Ok(morph::write_template(&self.seq.template, &values, decimals.min(12)))
    }

    fn __repr__(&self) -> String {
        format!(
            "NativeMorphSeq(shapes={}, subpaths={}, channels={})",
            self.seq.frames.len(),
            self.seq.template.len(),
            self.seq.channels()
        )
    }
}

// ---------------------------------------------------------------------------
// Noise
// ---------------------------------------------------------------------------

/// A coordinate argument of the vectorised noise functions: one value
/// (broadcast) or a list of values.
#[derive(FromPyObject)]
enum Coord {
    One(f64),
    Many(Vec<f64>),
}

impl Coord {
    #[inline]
    fn get(&self, i: usize) -> f64 {
        match self {
            Coord::One(v) => *v,
            Coord::Many(v) => v[i],
        }
    }
}

/// Common length of broadcast coordinates (a scalar-only call yields 1).
fn broadcast_len(cs: &[&Coord]) -> PyResult<usize> {
    let mut n: Option<usize> = None;
    for c in cs {
        if let Coord::Many(v) = c {
            match n {
                None => n = Some(v.len()),
                Some(m) if m != v.len() => {
                    return Err(PyValueError::new_err(format!(
                        "coordinate lists must have the same length ({m} != {})",
                        v.len()
                    )))
                }
                _ => {}
            }
        }
    }
    Ok(n.unwrap_or(1))
}

fn check_fbm(octaves: u32, lacunarity: f64, gain: f64) -> PyResult<()> {
    if octaves == 0 {
        return Err(PyValueError::new_err("octaves must be >= 1"));
    }
    if !lacunarity.is_finite() || !gain.is_finite() || gain <= 0.0 {
        return Err(PyValueError::new_err("lacunarity must be finite and gain > 0"));
    }
    Ok(())
}

/// Seeded simplex noise (2D/3D) and fractal Brownian motion; output ≈ [-1, 1].
#[pyclass(frozen, module = "anikit._anikit", name = "NativeNoise")]
pub struct NativeNoise {
    n: Noise,
}

#[pymethods]
impl NativeNoise {
    #[new]
    #[pyo3(signature = (seed=0))]
    fn new(seed: u32) -> Self {
        NativeNoise { n: Noise::new(seed) }
    }

    #[getter]
    fn seed(&self) -> u32 {
        self.n.seed()
    }

    fn noise2(&self, x: f64, y: f64) -> f64 {
        self.n.noise2(x, y)
    }

    fn noise3(&self, x: f64, y: f64, z: f64) -> f64 {
        self.n.noise3(x, y, z)
    }

    #[pyo3(signature = (x, y, octaves=4, lacunarity=2.0, gain=0.5))]
    fn fbm2(&self, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> PyResult<f64> {
        check_fbm(octaves, lacunarity, gain)?;
        Ok(self.n.fbm2(x, y, octaves, lacunarity, gain))
    }

    #[pyo3(signature = (x, y, z, octaves=4, lacunarity=2.0, gain=0.5))]
    fn fbm3(&self, x: f64, y: f64, z: f64, octaves: u32, lacunarity: f64, gain: f64) -> PyResult<f64> {
        check_fbm(octaves, lacunarity, gain)?;
        Ok(self.n.fbm3(x, y, z, octaves, lacunarity, gain))
    }

    /// Vectorised `noise2`; each coordinate is a float (broadcast) or a list.
    fn noise2_many(&self, py: Python<'_>, x: Coord, y: Coord) -> PyResult<Vec<f64>> {
        let len = broadcast_len(&[&x, &y])?;
        Ok(py.detach(|| (0..len).map(|i| self.n.noise2(x.get(i), y.get(i))).collect()))
    }

    /// Vectorised `noise3`; each coordinate is a float (broadcast) or a list.
    fn noise3_many(&self, py: Python<'_>, x: Coord, y: Coord, z: Coord) -> PyResult<Vec<f64>> {
        let len = broadcast_len(&[&x, &y, &z])?;
        Ok(py.detach(|| (0..len).map(|i| self.n.noise3(x.get(i), y.get(i), z.get(i))).collect()))
    }

    /// Vectorised `fbm2`.
    #[pyo3(signature = (x, y, octaves=4, lacunarity=2.0, gain=0.5))]
    fn fbm2_many(&self, py: Python<'_>, x: Coord, y: Coord, octaves: u32, lacunarity: f64, gain: f64) -> PyResult<Vec<f64>> {
        check_fbm(octaves, lacunarity, gain)?;
        let len = broadcast_len(&[&x, &y])?;
        Ok(py.detach(|| (0..len).map(|i| self.n.fbm2(x.get(i), y.get(i), octaves, lacunarity, gain)).collect()))
    }

    /// Vectorised `fbm3`.
    #[pyo3(signature = (x, y, z, octaves=4, lacunarity=2.0, gain=0.5))]
    #[allow(clippy::too_many_arguments)]
    fn fbm3_many(
        &self,
        py: Python<'_>,
        x: Coord,
        y: Coord,
        z: Coord,
        octaves: u32,
        lacunarity: f64,
        gain: f64,
    ) -> PyResult<Vec<f64>> {
        check_fbm(octaves, lacunarity, gain)?;
        let len = broadcast_len(&[&x, &y, &z])?;
        Ok(py.detach(|| (0..len).map(|i| self.n.fbm3(x.get(i), y.get(i), z.get(i), octaves, lacunarity, gain)).collect()))
    }

    /// `width * height` row-major samples of `noise2` (or `fbm2` with
    /// `octaves > 1`) at `(x0 + i*step, y0 + j*step)`, plus `z` as a third
    /// axis when given.
    #[pyo3(signature = (width, height, x0=0.0, y0=0.0, step=0.1, z=None, octaves=1, lacunarity=2.0, gain=0.5))]
    #[allow(clippy::too_many_arguments)]
    fn grid(
        &self,
        py: Python<'_>,
        width: usize,
        height: usize,
        x0: f64,
        y0: f64,
        step: f64,
        z: Option<f64>,
        octaves: u32,
        lacunarity: f64,
        gain: f64,
    ) -> PyResult<Vec<f64>> {
        check_fbm(octaves, lacunarity, gain)?;
        let total = width.checked_mul(height).filter(|&t| t <= 1 << 26);
        let Some(total) = total else {
            return Err(PyValueError::new_err("grid too large"));
        };
        let n = &self.n;
        Ok(py.detach(|| {
            let mut out = Vec::with_capacity(total);
            for j in 0..height {
                let y = y0 + j as f64 * step;
                for i in 0..width {
                    let x = x0 + i as f64 * step;
                    out.push(match (z, octaves) {
                        (None, 1) => n.noise2(x, y),
                        (None, o) => n.fbm2(x, y, o, lacunarity, gain),
                        (Some(z), 1) => n.noise3(x, y, z),
                        (Some(z), o) => n.fbm3(x, y, z, o, lacunarity, gain),
                    });
                }
            }
            out
        }))
    }

    fn __repr__(&self) -> String {
        format!("NativeNoise(seed={})", self.n.seed())
    }
}

// ---------------------------------------------------------------------------
// Stagger
// ---------------------------------------------------------------------------

/// Distribute values over `count` items. `origin` is one of `first`, `last`,
/// `center`, `edges`, `random`, or `index` (with `index`).
#[pyfunction]
#[pyo3(signature = (count, each=100.0, total=None, start=0.0, origin="first", index=0.0, seed=1, grid=None, axis=None, ease=None))]
#[allow(clippy::too_many_arguments)]
fn stagger(
    count: usize,
    each: f64,
    total: Option<f64>,
    start: f64,
    origin: &str,
    index: f64,
    seed: u32,
    grid: Option<(usize, usize)>,
    axis: Option<&str>,
    ease: Option<PyRef<'_, NativeEasing>>,
) -> PyResult<Vec<f64>> {
    let from = match origin {
        "first" | "start" => StaggerFrom::First,
        "last" | "end" => StaggerFrom::Last,
        "center" | "centre" => StaggerFrom::Center,
        "edges" => StaggerFrom::Edges,
        "random" => StaggerFrom::Random(seed),
        "index" => StaggerFrom::Index(index),
        o => return Err(PyValueError::new_err(format!("unknown stagger origin: {o:?}"))),
    };
    let axis = match axis {
        None => None,
        Some("x") | Some("X") => Some(Axis::X),
        Some("y") | Some("Y") => Some(Axis::Y),
        Some(a) => return Err(PyValueError::new_err(format!("axis must be 'x' or 'y', got {a:?}"))),
    };
    if let Some((c, _)) = grid {
        if c == 0 {
            return Err(PyValueError::new_err("grid needs at least one column"));
        }
    }
    let s = Stagger {
        each,
        total,
        start,
        from,
        grid,
        axis,
        ease: ease.map(|e| e.e.clone()).unwrap_or(Easing::Linear),
    };
    Ok(core_stagger::stagger(count, &s))
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// One track: `(kind, channels, values, offsets, easings, color_space, path)`.
///
/// * `kind`: 0 number, 1 colour, 2 discrete, 3 path, 4 matrix (16 channels, `matrix3d` order;
///   interpolated by decomposition like browsers do)
/// * `values`: `keyframes * channels` floats (colour: r, g, b 0–255, a 0–1)
/// * `offsets`: empty (evenly spaced) or one per keyframe in `[0, 1]`
/// * `easings`: empty (linear), one, or `keyframes - 1` easing ids
/// * `color_space`: colour space id for colour tracks (see `color_space_id`)
/// * `path`: a `NativePath` for path tracks (values are progress 0–1)
#[derive(FromPyObject)]
struct TrackArg(u8, usize, Vec<f64>, Vec<f64>, Vec<u32>, u8, Option<Py<NativePath>>);

fn timing(duration: f64, delay: f64, iterations: f64, dir: u8, end_delay: f64, easing: u32) -> PyResult<Timing> {
    Ok(Timing { delay, duration, iterations, direction: direction(dir)?, end_delay, easing })
}

/// Low-level animation engine. All times are milliseconds; nodes are integer handles.
#[pyclass(unsendable, module = "anikit._anikit", name = "Engine")]
pub struct Engine {
    inner: core_engine::Engine,
}

#[pymethods]
impl Engine {
    #[new]
    fn new() -> Self {
        Engine { inner: core_engine::Engine::new() }
    }

    // ---- easing registry ------------------------------------------------

    /// Parse (cached by string) and register an easing; returns its id.
    fn easing(&mut self, spec: &str) -> PyResult<u32> {
        self.inner.easing(spec).ok_or_else(|| PyValueError::new_err(format!("unknown easing: {spec:?}")))
    }

    /// Register a native easing object; identical easings share an id.
    fn add_easing(&mut self, e: PyRef<'_, NativeEasing>) -> u32 {
        self.inner.add_easing(e.e.clone())
    }

    /// Evaluate a registered easing.
    fn ease(&self, id: u32, t: f64) -> f64 {
        self.inner.ease(id, t)
    }

    /// Number of registered (non-linear) easings.
    #[getter]
    fn easing_count(&self) -> usize {
        let mut n = 0;
        while self.inner.easing_ref(n as u32).is_some() {
            n += 1;
        }
        n
    }

    // ---- creation ---------------------------------------------------------

    #[pyo3(signature = (tracks, duration=1000.0, delay=0.0, iterations=1.0, direction=0, end_delay=0.0, easing=LINEAR, flags=core_engine::flags::DEFAULT))]
    #[allow(clippy::too_many_arguments)]
    fn create_anim(
        &mut self,
        py: Python<'_>,
        tracks: Vec<TrackArg>,
        duration: f64,
        delay: f64,
        iterations: f64,
        direction: u8,
        end_delay: f64,
        easing: u32,
        flags: u8,
    ) -> PyResult<Handle> {
        let mut specs = Vec::with_capacity(tracks.len());
        for TrackArg(kind, channels, values, offsets, easings, space, path) in tracks {
            let kind = match kind {
                0 => TrackKind::Number,
                1 => TrackKind::Color(ColorSpace::from_u8(space)),
                2 => TrackKind::Discrete,
                3 => TrackKind::Path,
                4 => TrackKind::Matrix,
                k => return Err(PyValueError::new_err(format!("unknown track kind {k}"))),
            };
            let lut = match (kind, path) {
                (TrackKind::Path, Some(p)) => {
                    let p = p.borrow(py);
                    let n = ((p.p.length() / 1.5) as usize).clamp(256, 4096);
                    Some(PathLut::new(&p.p, n))
                }
                (TrackKind::Path, None) => return Err(PyValueError::new_err("path tracks need a NativePath")),
                _ => None,
            };
            specs.push(TrackSpec { kind, channels, offsets, values, easings, path: lut });
        }
        let t = timing(duration, delay, iterations, direction, end_delay, easing)?;
        self.inner.create_anim(t, &specs, flags).py()
    }

    #[pyo3(signature = (duration=0.0, delay=0.0, iterations=1.0, direction=0, end_delay=0.0, easing=LINEAR, auto_duration=true, flags=core_engine::flags::DEFAULT))]
    #[allow(clippy::too_many_arguments)]
    fn create_group(
        &mut self,
        duration: f64,
        delay: f64,
        iterations: f64,
        direction: u8,
        end_delay: f64,
        easing: u32,
        auto_duration: bool,
        flags: u8,
    ) -> PyResult<Handle> {
        let t = timing(duration, delay, iterations, direction, end_delay, easing)?;
        self.inner.create_group(t, auto_duration, flags).py()
    }

    fn create_callback(&mut self) -> PyResult<Handle> {
        self.inner.create_callback().py()
    }

    fn group_add(&mut self, group: Handle, child: Handle, start: f64) -> PyResult<()> {
        self.inner.group_add(group, child, start).py()
    }

    fn group_remove(&mut self, child: Handle) -> PyResult<()> {
        self.inner.group_remove(child).py()
    }

    #[pyo3(signature = (pos, target, vel=None, stiffness=100.0, damping=10.0, mass=1.0))]
    fn create_spring(
        &mut self,
        pos: Vec<f64>,
        target: Vec<f64>,
        vel: Option<Vec<f64>>,
        stiffness: f64,
        damping: f64,
        mass: f64,
    ) -> PyResult<Handle> {
        let vel = vel.unwrap_or_default();
        self.inner.create_spring(SpringParams::new(stiffness, damping, mass), &pos, &vel, &target).py()
    }

    #[pyo3(signature = (pos, vel, power=0.8, time_constant=350.0, target=None, min=None, max=None, bounce_stiffness=500.0, bounce_damping=40.0, bounce_mass=1.0))]
    #[allow(clippy::too_many_arguments)]
    fn create_decay(
        &mut self,
        pos: Vec<f64>,
        vel: Vec<f64>,
        power: f64,
        time_constant: f64,
        target: Option<Vec<f64>>,
        min: Option<Vec<f64>>,
        max: Option<Vec<f64>>,
        bounce_stiffness: f64,
        bounce_damping: f64,
        bounce_mass: f64,
    ) -> PyResult<Handle> {
        self.inner
            .create_decay(
                &pos,
                &vel,
                power,
                time_constant,
                target.as_deref(),
                min.as_deref(),
                max.as_deref(),
                SpringParams::new(bounce_stiffness, bounce_damping, bounce_mass),
            )
            .py()
    }

    /// Ballistic simulation: velocity (units/s), constant acceleration
    /// (units/s²), exponential `friction` (1/s), bouncing `min`/`max` bounds
    /// with `restitution` (0–1). Finishes at rest or after `life` ms.
    #[pyo3(signature = (pos, vel, acc=None, friction=0.0, min=None, max=None, restitution=0.0, life=None))]
    #[allow(clippy::too_many_arguments)]
    fn create_physics(
        &mut self,
        pos: Vec<f64>,
        vel: Vec<f64>,
        acc: Option<Vec<f64>>,
        friction: f64,
        min: Option<Vec<f64>>,
        max: Option<Vec<f64>>,
        restitution: f64,
        life: Option<f64>,
    ) -> PyResult<Handle> {
        if !friction.is_finite() || friction < 0.0 {
            return Err(PyValueError::new_err("friction must be finite and >= 0"));
        }
        if !restitution.is_finite() {
            return Err(PyValueError::new_err("restitution must be finite"));
        }
        let ch = pos.len();
        for (name, b) in [("min", &min), ("max", &max)] {
            if let Some(b) = b {
                if b.len() != ch {
                    return Err(PyValueError::new_err(format!("{name} needs {ch} values, got {}", b.len())));
                }
            }
        }
        let acc = acc.unwrap_or_default();
        let life = match life {
            None => f64::INFINITY,
            Some(l) if l > 0.0 => l,
            Some(_) => return Err(PyValueError::new_err("life must be > 0 (or None for unlimited)")),
        };
        self.inner
            .create_physics(&pos, &vel, &acc, friction, min.as_deref(), max.as_deref(), restitution, life)
            .py()
    }

    /// Change a physics simulation's acceleration (e.g. gravity, wind).
    fn sim_set_acceleration(&mut self, h: Handle, acc: Vec<f64>) -> PyResult<()> {
        self.inner.sim_set_acceleration(h, &acc).py()
    }

    // ---- simulations -----------------------------------------------------

    fn sim_set_target(&mut self, h: Handle, target: Vec<f64>) -> PyResult<()> {
        self.inner.sim_set_target(h, &target).py()
    }

    #[pyo3(signature = (h, pos, vel=None))]
    fn sim_set_position(&mut self, h: Handle, pos: Vec<f64>, vel: Option<Vec<f64>>) -> PyResult<()> {
        self.inner.sim_set_position(h, &pos, vel.as_deref()).py()
    }

    #[pyo3(signature = (h, stiffness=100.0, damping=10.0, mass=1.0))]
    fn sim_set_params(&mut self, h: Handle, stiffness: f64, damping: f64, mass: f64) -> PyResult<()> {
        self.inner.sim_set_params(h, SpringParams::new(stiffness, damping, mass)).py()
    }

    fn sim_set_rest(&mut self, h: Handle, rest_delta: f64, rest_speed: f64) -> PyResult<()> {
        self.inner.sim_set_rest(h, rest_delta, rest_speed).py()
    }

    fn sim_velocity(&self, h: Handle) -> PyResult<Vec<f64>> {
        self.inner.sim_velocity(h).py().map(<[f64]>::to_vec)
    }

    fn sim_target(&self, h: Handle) -> PyResult<Vec<f64>> {
        self.inner.sim_target(h).py().map(<[f64]>::to_vec)
    }

    // ---- playback ----------------------------------------------------------

    fn play(&mut self, h: Handle) -> PyResult<()> {
        self.inner.play(h).py()
    }

    fn pause(&mut self, h: Handle) -> PyResult<()> {
        self.inner.pause(h).py()
    }

    /// Jump to local time `t` (ms) and render immediately.
    fn seek(&mut self, h: Handle, t: f64) -> PyResult<()> {
        self.inner.seek(h, t).py()
    }

    fn set_progress(&mut self, h: Handle, p: f64) -> PyResult<()> {
        self.inner.set_progress(h, p).py()
    }

    fn set_rate(&mut self, h: Handle, rate: f64) -> PyResult<()> {
        self.inner.set_rate(h, rate).py()
    }

    fn reverse(&mut self, h: Handle) -> PyResult<()> {
        self.inner.reverse(h).py()
    }

    fn restart(&mut self, h: Handle) -> PyResult<()> {
        self.inner.restart(h).py()
    }

    /// Change timing; `None` keeps the current value.
    #[pyo3(signature = (h, duration=None, delay=None, iterations=None, direction=None, end_delay=None, easing=None))]
    #[allow(clippy::too_many_arguments)]
    fn set_timing(
        &mut self,
        h: Handle,
        duration: Option<f64>,
        delay: Option<f64>,
        iterations: Option<f64>,
        direction: Option<u8>,
        end_delay: Option<f64>,
        easing: Option<u32>,
    ) -> PyResult<()> {
        let mut t = self.inner.timing(h).py()?;
        if let Some(v) = duration {
            t.duration = v;
        }
        if let Some(v) = delay {
            t.delay = v;
        }
        if let Some(v) = iterations {
            t.iterations = v;
        }
        if let Some(v) = direction {
            t.direction = self::direction(v)?;
        }
        if let Some(v) = end_delay {
            t.end_delay = v;
        }
        if let Some(v) = easing {
            t.easing = v;
        }
        self.inner.set_timing(h, t).py()
    }

    fn set_start(&mut self, h: Handle, start: f64) -> PyResult<()> {
        self.inner.set_start(h, start).py()
    }

    fn set_track_enabled(&mut self, h: Handle, track: usize, enabled: bool) -> PyResult<()> {
        self.inner.set_track_enabled(h, track, enabled).py()
    }

    /// Free a node (groups: with all descendants).
    fn release(&mut self, h: Handle) -> PyResult<()> {
        self.inner.release(h).py()
    }

    /// Release every node (easings stay registered).
    fn clear(&mut self) {
        self.inner.clear()
    }

    // ---- queries -------------------------------------------------------------

    fn values(&self, h: Handle) -> PyResult<Vec<f64>> {
        self.inner.values(h).py().map(<[f64]>::to_vec)
    }

    fn track_offsets(&self, h: Handle) -> PyResult<Vec<u32>> {
        self.inner.track_offsets(h).py()
    }

    fn time(&self, h: Handle) -> PyResult<f64> {
        self.inner.time(h).py()
    }

    /// Total duration (delay + active + end delay), ms.
    fn duration(&self, h: Handle) -> PyResult<f64> {
        self.inner.duration(h).py()
    }

    /// `(duration, delay, iterations, direction, end_delay, easing)`.
    fn timing(&self, h: Handle) -> PyResult<(f64, f64, f64, u8, f64, u32)> {
        let t = self.inner.timing(h).py()?;
        Ok((t.duration, t.delay, t.iterations, t.direction as u8, t.end_delay, t.easing))
    }

    fn progress(&self, h: Handle) -> PyResult<f64> {
        self.inner.progress(h).py()
    }

    /// 0 idle, 1 playing, 2 paused, 3 finished.
    fn state(&self, h: Handle) -> PyResult<u8> {
        Ok(match self.inner.state(h).py()? {
            PlayState::Idle => 0,
            PlayState::Playing => 1,
            PlayState::Paused => 2,
            PlayState::Finished => 3,
        })
    }

    fn iteration(&self, h: Handle) -> PyResult<f64> {
        self.inner.iteration(h).py()
    }

    fn rate(&self, h: Handle) -> PyResult<f64> {
        self.inner.rate_of(h).py()
    }

    fn start(&self, h: Handle) -> PyResult<f64> {
        self.inner.start_of(h).py()
    }

    fn parent(&self, h: Handle) -> PyResult<Option<Handle>> {
        let p = self.inner.parent_of(h).py()?;
        Ok(if p == NONE { None } else { Some(p) })
    }

    fn is_alive(&self, h: Handle) -> bool {
        self.inner.is_alive(h)
    }

    /// Evaluate an animation at raw iteration progress `p` without touching playback.
    fn sample(&mut self, h: Handle, p: f64) -> PyResult<Vec<f64>> {
        let n = self.inner.values(h).py()?.len();
        let mut out = vec![0.0; n];
        self.inner.sample(h, p, &mut out).py()?;
        Ok(out)
    }

    /// Sample at many progress values; returns one list per progress value.
    fn sample_many(&mut self, h: Handle, ps: Vec<f64>) -> PyResult<Vec<Vec<f64>>> {
        let n = self.inner.values(h).py()?.len();
        let mut res = Vec::with_capacity(ps.len());
        for p in ps {
            let mut out = vec![0.0; n];
            self.inner.sample(h, p, &mut out).py()?;
            res.push(out);
        }
        Ok(res)
    }

    // ---- the frame -------------------------------------------------------------

    /// Advance the world by `dt` milliseconds.
    fn tick(&mut self, dt: f64) {
        self.inner.tick(dt)
    }

    /// Handles whose outputs changed since the last tick began.
    fn updated(&self) -> Vec<Handle> {
        self.inner.updated().to_vec()
    }

    /// Flat `[handle, kind, handle, kind, ...]` event list.
    fn events(&self) -> Vec<u32> {
        self.inner.events().to_vec()
    }

    fn clear_frame(&mut self) {
        self.inner.clear_frame()
    }

    /// Take the frame's results in one call and clear them:
    /// `(updated_handles, values_per_handle, flat_events)`. Handles released
    /// since they were updated are skipped.
    fn drain(&mut self) -> (Vec<Handle>, Vec<Vec<f64>>, Vec<u32>) {
        let mut hs = Vec::with_capacity(self.inner.updated().len());
        let mut vs = Vec::with_capacity(hs.capacity());
        for &h in self.inner.updated() {
            if let Ok(v) = self.inner.values(h) {
                hs.push(h);
                vs.push(v.to_vec());
            }
        }
        let ev = self.inner.events().to_vec();
        self.inner.clear_frame();
        (hs, vs, ev)
    }

    /// `tick(dt)` followed by `drain()`.
    fn step(&mut self, dt: f64) -> (Vec<Handle>, Vec<Vec<f64>>, Vec<u32>) {
        self.inner.tick(dt);
        self.drain()
    }

    #[getter]
    fn active_count(&self) -> usize {
        self.inner.active_count()
    }

    #[getter]
    fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Engine clock (ms of world time advanced so far).
    #[getter]
    fn now(&self) -> f64 {
        self.inner.now
    }

    /// Global time scale.
    #[getter(global_rate)]
    fn get_global_rate(&self) -> f64 {
        self.inner.rate
    }

    #[setter(global_rate)]
    fn set_global_rate(&mut self, r: f64) -> PyResult<()> {
        if !r.is_finite() || r < 0.0 {
            return Err(PyValueError::new_err("global rate must be finite and >= 0"));
        }
        self.inner.rate = r;
        Ok(())
    }

    /// Frame gaps longer than `threshold` ms are treated as `adjusted` ms; `threshold <= 0` disables.
    fn set_lag_smoothing(&mut self, threshold: f64, adjusted: f64) {
        self.inner.set_lag_smoothing(threshold, adjusted)
    }

    fn __repr__(&self) -> String {
        format!("Engine(nodes={}, active={})", self.inner.node_count(), self.inner.active_count())
    }
}

// ---------------------------------------------------------------------------
// Flat f32 buffers
// ---------------------------------------------------------------------------

/// Flat `f32` input: native-endian `bytes` or `bytearray` (e.g.
/// `array('f').tobytes()`, a float32 numpy array's buffer) or any sequence of
/// numbers.
#[derive(FromPyObject)]
enum F32Data<'py> {
    Bytes(Bound<'py, PyBytes>),
    ByteArray(Bound<'py, PyByteArray>),
    Seq(Vec<f32>),
}

fn decode_f32(raw: &[u8], out: &mut Vec<f32>) -> PyResult<()> {
    if !raw.len().is_multiple_of(4) {
        return Err(PyValueError::new_err("f32 byte buffers must be a multiple of 4 bytes long"));
    }
    out.clear();
    out.extend(raw.chunks_exact(4).map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]])));
    Ok(())
}

impl F32Data<'_> {
    fn into_vec(self) -> PyResult<Vec<f32>> {
        match self {
            F32Data::Seq(v) => Ok(v),
            other => {
                let mut v = Vec::new();
                other.read_into(&mut v)?;
                Ok(v)
            }
        }
    }

    /// Decode into `out`, reusing its allocation.
    fn read_into(&self, out: &mut Vec<f32>) -> PyResult<()> {
        match self {
            F32Data::Seq(v) => {
                out.clear();
                out.extend_from_slice(v);
                Ok(())
            }
            F32Data::Bytes(b) => decode_f32(b.as_bytes(), out),
            // SAFETY: the GIL is held and no Python code runs while the slice is borrowed.
            F32Data::ByteArray(b) => decode_f32(unsafe { b.as_bytes() }, out),
        }
    }
}

/// Native-endian `bytes` holding `vals` (`array('f').frombytes(...)` reads it back).
fn f32_bytes<'py>(py: Python<'py>, vals: impl ExactSizeIterator<Item = f32>) -> PyResult<Bound<'py, PyBytes>> {
    let n = vals.len();
    PyBytes::new_with(py, n * 4, |buf| {
        for (chunk, v) in buf.chunks_exact_mut(4).zip(vals) {
            chunk.copy_from_slice(&v.to_ne_bytes());
        }
        Ok(())
    })
}

fn u32_bytes<'py>(py: Python<'py>, vals: &[u32]) -> PyResult<Bound<'py, PyBytes>> {
    PyBytes::new_with(py, vals.len() * 4, |buf| {
        for (chunk, v) in buf.chunks_exact_mut(4).zip(vals) {
            chunk.copy_from_slice(&v.to_ne_bytes());
        }
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// 2D physics world
// ---------------------------------------------------------------------------

type Bounds4 = (f32, f32, f32, f32);

fn bounds_of(b: Option<Bounds4>) -> PyResult<[f32; 4]> {
    match b {
        None => Ok([f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY]),
        Some((x0, y0, x1, y1)) => {
            if [x0, y0, x1, y1].iter().any(|v| v.is_nan()) {
                return Err(PyValueError::new_err("bounds must not be NaN"));
            }
            Ok([x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)])
        }
    }
}

fn finite(name: &str, v: f32) -> PyResult<f32> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(PyValueError::new_err(format!("{name} must be finite, got {v}")))
    }
}

/// 2D position-based physics world: circles, static boxes, distance
/// constraints, attractors and bounds. Bodies are integer indices; bulk
/// outputs are native-endian `f32` bytes.
#[pyclass(module = "anikit._anikit", name = "NativeWorld")]
pub struct NativeWorld {
    w: World,
}

impl NativeWorld {
    fn check(&self, i: usize) -> PyResult<usize> {
        if i < self.w.len() {
            Ok(i)
        } else {
            Err(PyIndexError::new_err(format!("body index {i} out of range ({} bodies)", self.w.len())))
        }
    }
}

#[pymethods]
impl NativeWorld {
    #[new]
    #[pyo3(signature = (gravity=(0.0, 980.0), damping=0.05, substeps=4, iterations=2, restitution=0.2, wall_restitution=0.3, friction=0.05, bounds=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        gravity: (f32, f32),
        damping: f32,
        substeps: u32,
        iterations: u32,
        restitution: f32,
        wall_restitution: f32,
        friction: f32,
        bounds: Option<Bounds4>,
    ) -> PyResult<Self> {
        let config = WorldConfig {
            gravity: [finite("gravity", gravity.0)?, finite("gravity", gravity.1)?],
            damping: finite("damping", damping)?.max(0.0),
            substeps: substeps.max(1),
            iterations: iterations.max(1),
            restitution: finite("restitution", restitution)?,
            wall_restitution: finite("wall_restitution", wall_restitution)?,
            friction: finite("friction", friction)?,
            bounds: bounds_of(bounds)?,
        };
        Ok(NativeWorld { w: World::new(config) })
    }

    // ---- configuration -----------------------------------------------------

    #[getter]
    fn get_gravity(&self) -> (f32, f32) {
        (self.w.config.gravity[0], self.w.config.gravity[1])
    }
    #[setter]
    fn set_gravity(&mut self, g: (f32, f32)) -> PyResult<()> {
        self.w.config.gravity = [finite("gravity", g.0)?, finite("gravity", g.1)?];
        Ok(())
    }
    #[getter]
    fn get_damping(&self) -> f32 {
        self.w.config.damping
    }
    #[setter]
    fn set_damping(&mut self, v: f32) -> PyResult<()> {
        self.w.config.damping = finite("damping", v)?.max(0.0);
        Ok(())
    }
    #[getter]
    fn get_substeps(&self) -> u32 {
        self.w.config.substeps
    }
    #[setter]
    fn set_substeps(&mut self, v: u32) {
        self.w.config.substeps = v.max(1);
    }
    #[getter]
    fn get_iterations(&self) -> u32 {
        self.w.config.iterations
    }
    #[setter]
    fn set_iterations(&mut self, v: u32) {
        self.w.config.iterations = v.max(1);
    }
    #[getter]
    fn get_restitution(&self) -> f32 {
        self.w.config.restitution
    }
    #[setter]
    fn set_restitution(&mut self, v: f32) -> PyResult<()> {
        self.w.config.restitution = finite("restitution", v)?;
        Ok(())
    }
    #[getter]
    fn get_wall_restitution(&self) -> f32 {
        self.w.config.wall_restitution
    }
    #[setter]
    fn set_wall_restitution(&mut self, v: f32) -> PyResult<()> {
        self.w.config.wall_restitution = finite("wall_restitution", v)?;
        Ok(())
    }
    #[getter]
    fn get_friction(&self) -> f32 {
        self.w.config.friction
    }
    #[setter]
    fn set_friction(&mut self, v: f32) -> PyResult<()> {
        self.w.config.friction = finite("friction", v)?;
        Ok(())
    }
    /// `(min_x, min_y, max_x, max_y)` or `None` (unbounded).
    #[getter]
    fn get_bounds(&self) -> Option<Bounds4> {
        let [x0, y0, x1, y1] = self.w.config.bounds;
        if [x0, y0, x1, y1].iter().all(|v| v.is_infinite()) {
            None
        } else {
            Some((x0, y0, x1, y1))
        }
    }
    #[setter]
    fn set_bounds(&mut self, b: Option<Bounds4>) -> PyResult<()> {
        self.w.config.bounds = bounds_of(b)?;
        Ok(())
    }

    // ---- bodies ------------------------------------------------------------

    fn __len__(&self) -> usize {
        self.w.len()
    }

    /// Add a circle; `mass <= 0` makes it static. Returns its index.
    fn add_body(&mut self, x: f32, y: f32, radius: f32, mass: f32) -> PyResult<u32> {
        Ok(self.w.add_body(finite("x", x)?, finite("y", y)?, finite("radius", radius)?, finite("mass", mass)?))
    }

    /// Add bodies from flat `[x, y, radius, mass] * n`; returns the first index.
    fn add_bodies(&mut self, data: F32Data<'_>) -> PyResult<u32> {
        let v = data.into_vec()?;
        if !v.len().is_multiple_of(4) {
            return Err(PyValueError::new_err("add_bodies expects flat [x, y, radius, mass] * n"));
        }
        if v.iter().any(|x| !x.is_finite()) {
            return Err(PyValueError::new_err("body values must be finite"));
        }
        Ok(self.w.add_bodies(&v))
    }

    #[pyo3(signature = (i, x, y, keep_velocity=false))]
    fn set_position(&mut self, i: usize, x: f32, y: f32, keep_velocity: bool) -> PyResult<()> {
        let i = self.check(i)?;
        self.w.set_position(i, finite("x", x)?, finite("y", y)?, keep_velocity);
        Ok(())
    }

    /// Move many bodies: flat `[x, y] * n` starting at body `first`.
    #[pyo3(signature = (data, first=0, keep_velocity=false))]
    fn set_positions(&mut self, data: F32Data<'_>, first: usize, keep_velocity: bool) -> PyResult<()> {
        let v = data.into_vec()?;
        if !v.len().is_multiple_of(2) || first + v.len() / 2 > self.w.len() {
            return Err(PyValueError::new_err("set_positions expects flat [x, y] pairs for existing bodies"));
        }
        for (k, c) in v.chunks_exact(2).enumerate() {
            self.w.set_position(first + k, c[0], c[1], keep_velocity);
        }
        Ok(())
    }

    fn set_velocity(&mut self, i: usize, vx: f32, vy: f32) -> PyResult<()> {
        let i = self.check(i)?;
        self.w.set_velocity(i, finite("vx", vx)?, finite("vy", vy)?);
        Ok(())
    }

    fn position(&self, i: usize) -> PyResult<(f32, f32)> {
        let [x, y] = self.w.position(self.check(i)?);
        Ok((x, y))
    }

    fn velocity(&self, i: usize) -> PyResult<(f32, f32)> {
        let [x, y] = self.w.velocity(self.check(i)?);
        Ok((x, y))
    }

    /// Rolling angle in radians.
    fn angle(&self, i: usize) -> PyResult<f32> {
        let i = self.check(i)?;
        Ok(self.w.output()[i * 3 + 2])
    }

    fn set_pinned(&mut self, i: usize, pinned: bool) -> PyResult<()> {
        let i = self.check(i)?;
        self.w.set_pinned(i, pinned);
        Ok(())
    }

    fn set_radius(&mut self, i: usize, r: f32) -> PyResult<()> {
        let i = self.check(i)?;
        self.w.set_radius(i, finite("radius", r)?);
        Ok(())
    }

    fn set_active(&mut self, i: usize, on: bool) -> PyResult<()> {
        let i = self.check(i)?;
        self.w.set_active(i, on);
        Ok(())
    }

    /// Instantaneous radial velocity change (units/s at the centre, fading to 0 at `radius`).
    fn impulse(&mut self, x: f32, y: f32, radius: f32, strength: f32) {
        self.w.impulse(x, y, radius, strength)
    }

    /// Index of the topmost body under a point, or `None`.
    fn pick(&self, x: f32, y: f32) -> Option<u32> {
        match self.w.pick(x, y) {
            u32::MAX => None,
            i => Some(i),
        }
    }

    // ---- boxes, constraints, attractors -------------------------------------

    fn add_box(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) -> u32 {
        self.w.add_box(x0, y0, x1, y1)
    }

    fn clear_boxes(&mut self) {
        self.w.clear_boxes()
    }

    /// Distance constraint; `length < 0` uses the current distance.
    #[pyo3(signature = (a, b, length=-1.0, stiffness=1.0, tear=0.0))]
    fn add_constraint(&mut self, a: u32, b: u32, length: f32, stiffness: f32, tear: f32) -> PyResult<u32> {
        self.check(a as usize)?;
        self.check(b as usize)?;
        Ok(self.w.add_constraint(a, b, finite("length", length)?, finite("stiffness", stiffness)?, finite("tear", tear)?))
    }

    /// Add many constraints from flat `[a, b, length, stiffness, tear] * n`; returns the first index.
    fn add_constraints(&mut self, data: Vec<f64>) -> PyResult<u32> {
        if !data.len().is_multiple_of(5) {
            return Err(PyValueError::new_err("add_constraints expects flat [a, b, length, stiffness, tear] * n"));
        }
        let first = self.w.constraints().len() as u32;
        let n = self.w.len() as f64;
        for c in data.chunks_exact(5) {
            if !(c[0] >= 0.0 && c[0] < n && c[1] >= 0.0 && c[1] < n) {
                return Err(PyIndexError::new_err("constraint body index out of range"));
            }
            self.w.add_constraint(c[0] as u32, c[1] as u32, c[2] as f32, c[3] as f32, c[4] as f32);
        }
        Ok(first)
    }

    /// `(a, b, length, stiffness, tear, alive)` per constraint.
    fn constraints(&self) -> Vec<(u32, u32, f32, f32, f32, bool)> {
        self.w.constraints().iter().map(|c| (c.a, c.b, c.length, c.stiffness, c.tear, c.alive)).collect()
    }

    fn constraint_count(&self) -> usize {
        self.w.constraints().len()
    }

    /// Replace all attractors: `(x, y, strength, radius)`; negative strength repels.
    fn set_attractors(&mut self, items: Vec<(f32, f32, f32, f32)>) {
        let a: Vec<Attractor> = items.into_iter().map(|(x, y, strength, radius)| Attractor { x, y, strength, radius }).collect();
        self.w.set_attractors(&a);
    }

    // ---- simulation ------------------------------------------------------------

    /// Advance `steps` times by `dt` seconds each (each step is clamped to 1/20 s
    /// by the core). Releases the GIL.
    #[pyo3(signature = (dt, steps=1))]
    fn step(&mut self, py: Python<'_>, dt: f32, steps: u32) -> PyResult<()> {
        let dt = finite("dt", dt)?;
        let w = &mut self.w;
        py.detach(|| {
            for _ in 0..steps {
                w.step(dt);
            }
        });
        Ok(())
    }

    /// `[x, y, angle] * n` as native-endian `f32` bytes (positions are current
    /// even after `set_position`; the core's render buffer only refreshes on `step`).
    fn output<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let (w, out) = (&self.w, self.w.output());
        f32_bytes(py, (0..w.len() * 3).map(|k| if k % 3 == 2 { out[k] } else { w.position(k / 3)[k % 3] }))
    }

    /// `[x, y] * n` as native-endian `f32` bytes.
    fn positions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let w = &self.w;
        f32_bytes(py, (0..w.len() * 2).map(|k| w.position(k / 2)[k % 2]))
    }

    /// `[vx, vy] * n` as native-endian `f32` bytes.
    fn velocities<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let n = self.w.len();
        let w = &self.w;
        f32_bytes(py, (0..n * 2).map(|k| w.velocity(k / 2)[k % 2]))
    }

    #[getter]
    fn kinetic_energy(&self) -> f32 {
        self.w.kinetic_energy()
    }

    /// Broadphase pair tests performed by the last step (diagnostics).
    #[getter]
    fn pair_tests(&self) -> u64 {
        self.w.pair_tests
    }

    fn __repr__(&self) -> String {
        format!("NativeWorld(bodies={}, constraints={})", self.w.len(), self.w.constraints().len())
    }
}

// ---------------------------------------------------------------------------
// 3D: matrices, quaternions, CSS transforms, projection
// ---------------------------------------------------------------------------

type Vec3 = (f64, f64, f64);
type Quat4 = (f64, f64, f64, f64);

fn mat(v: Vec<f64>) -> PyResult<Mat4> {
    v.try_into().map_err(|v: Vec<f64>| PyValueError::new_err(format!("a 4x4 matrix needs 16 values, got {}", v.len())))
}

fn decomposed(v: Vec<f64>) -> PyResult<space::Decomposed> {
    v.try_into().map_err(|v: Vec<f64>| PyValueError::new_err(format!("a decomposed transform needs 17 values, got {}", v.len())))
}

fn quat(q: Quat4) -> space::Quat {
    [q.0, q.1, q.2, q.3]
}

fn quat4(q: space::Quat) -> Quat4 {
    (q[0], q[1], q[2], q[3])
}

/// Parse a CSS `transform` list into a column-major 4×4 matrix (`matrix3d` order);
/// `None` when it is not a valid transform.
#[pyfunction]
fn transform_parse(css: &str) -> Option<Vec<f64>> {
    space::parse_transform(css).map(|m| m.to_vec())
}

/// Browser-style interpolation of two CSS transforms; `None` if either is invalid.
#[pyfunction]
fn transform_mix(a: &str, b: &str, t: f64) -> Option<Vec<f64>> {
    let (a, b) = (space::parse_transform(a)?, space::parse_transform(b)?);
    Some(space::mix_matrix(&a, &b, t).to_vec())
}

/// `[tx, ty, tz, sx, sy, sz, skew_xy, skew_xz, skew_yz, p0, p1, p2, p3, qx, qy, qz, qw]`,
/// or `None` for singular matrices.
#[pyfunction]
fn mat4_decompose(m: Vec<f64>) -> PyResult<Option<Vec<f64>>> {
    Ok(space::decompose(&mat(m)?).map(|d| d.to_vec()))
}

#[pyfunction]
fn mat4_recompose(d: Vec<f64>) -> PyResult<Vec<f64>> {
    Ok(space::recompose(&decomposed(d)?).to_vec())
}

#[pyfunction]
fn mat4_mix(a: Vec<f64>, b: Vec<f64>, t: f64) -> PyResult<Vec<f64>> {
    Ok(space::mix_matrix(&mat(a)?, &mat(b)?, t).to_vec())
}

#[pyfunction]
fn mat4_mix_decomposed(a: Vec<f64>, b: Vec<f64>, t: f64) -> PyResult<Vec<f64>> {
    Ok(space::mix_decomposed(&decomposed(a)?, &decomposed(b)?, t).to_vec())
}

/// `a × b` (apply `b` first, then `a`).
#[pyfunction]
fn mat4_mul(a: Vec<f64>, b: Vec<f64>) -> PyResult<Vec<f64>> {
    Ok(space::mul(&mat(a)?, &mat(b)?).to_vec())
}

/// Inverse, or `None` when singular.
#[pyfunction]
fn mat4_invert(m: Vec<f64>) -> PyResult<Option<Vec<f64>>> {
    Ok(space::invert(&mat(m)?).map(|m| m.to_vec()))
}

#[pyfunction]
fn mat4_determinant(m: Vec<f64>) -> PyResult<f64> {
    Ok(space::determinant(&mat(m)?))
}

/// Homogeneous `(x, y, z, w)` of `m × (x, y, z, 1)`.
#[pyfunction]
fn mat4_transform_point(m: Vec<f64>, p: Vec3) -> PyResult<(f64, f64, f64, f64)> {
    let r = space::transform_point(&mat(m)?, [p.0, p.1, p.2]);
    Ok((r[0], r[1], r[2], r[3]))
}

#[pyfunction]
fn mat4_translate(x: f64, y: f64, z: f64) -> Vec<f64> {
    space::translate(x, y, z).to_vec()
}

#[pyfunction]
fn mat4_scale(x: f64, y: f64, z: f64) -> Vec<f64> {
    space::scale(x, y, z).to_vec()
}

/// Rotation of `deg` degrees about the `axis` vector (CSS `rotate3d`).
#[pyfunction]
fn mat4_rotate(axis: Vec3, deg: f64) -> Vec<f64> {
    let m = if axis == (1.0, 0.0, 0.0) {
        space::rotate_x(deg)
    } else if axis == (0.0, 1.0, 0.0) {
        space::rotate_y(deg)
    } else if axis == (0.0, 0.0, 1.0) {
        space::rotate_z(deg)
    } else {
        space::rotate_axis(axis.0, axis.1, axis.2, deg)
    };
    m.to_vec()
}

#[pyfunction]
fn mat4_skew(ax_deg: f64, ay_deg: f64) -> Vec<f64> {
    space::skew(ax_deg, ay_deg).to_vec()
}

/// CSS `perspective(d)`.
#[pyfunction]
fn mat4_perspective_css(d: f64) -> Vec<f64> {
    space::perspective_css(d).to_vec()
}

/// Right-handed look-at view matrix.
#[pyfunction]
fn mat4_look_at(eye: Vec3, target: Vec3, up: Vec3) -> Vec<f64> {
    space::look_at([eye.0, eye.1, eye.2], [target.0, target.1, target.2], [up.0, up.1, up.2]).to_vec()
}

/// OpenGL-style perspective projection.
#[pyfunction]
fn mat4_perspective(fov_y_deg: f64, aspect: f64, near: f64, far: f64) -> Vec<f64> {
    space::perspective(fov_y_deg, aspect, near, far).to_vec()
}

/// Quaternion `(x, y, z, w)` from Euler angles in degrees (CSS `rotateX() rotateY() rotateZ()`).
#[pyfunction]
fn quat_euler(x: f64, y: f64, z: f64) -> Quat4 {
    quat4(space::quat_euler(x, y, z))
}

#[pyfunction]
fn quat_axis_angle(x: f64, y: f64, z: f64, deg: f64) -> Quat4 {
    quat4(space::quat_axis_angle(x, y, z, deg))
}

#[pyfunction]
fn quat_mul(a: Quat4, b: Quat4) -> Quat4 {
    quat4(space::quat_mul(&quat(a), &quat(b)))
}

/// Spherical interpolation along the shortest arc.
#[pyfunction]
fn quat_slerp(a: Quat4, b: Quat4, t: f64) -> Quat4 {
    quat4(space::slerp(&quat(a), &quat(b), t))
}

#[pyfunction]
fn quat_to_matrix(q: Quat4) -> Vec<f64> {
    space::to_matrix(&quat(q)).to_vec()
}

/// Bulk perspective projector with radix depth sorting (buffers are reused).
#[pyclass(module = "anikit._anikit", name = "NativeProjector")]
#[derive(Default)]
pub struct NativeProjector {
    p: Projector,
}

#[pymethods]
impl NativeProjector {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Project `n` points (read from flat `src` at `stride` with x/y/z channel
    /// `offsets`) by `model` then `view_proj` into a `width × height` viewport.
    /// Returns `(out, order)` as native-endian bytes: `f32 [sx, sy, depth, scale] * n`
    /// and `u32` indices far → near (culled points: scale 0, sorted first).
    #[pyo3(signature = (src, stride, offsets, model, view_proj, width, height, sort=true))]
    #[allow(clippy::too_many_arguments)]
    fn project<'py>(
        &mut self,
        py: Python<'py>,
        src: F32Data<'_>,
        stride: usize,
        offsets: (usize, usize, usize),
        model: Vec<f64>,
        view_proj: Vec<f64>,
        width: f64,
        height: f64,
        sort: bool,
    ) -> PyResult<(Bound<'py, PyBytes>, Bound<'py, PyBytes>)> {
        let src = src.into_vec()?;
        let offs = [offsets.0, offsets.1, offsets.2];
        if stride == 0 || offs.iter().any(|&o| o >= stride) {
            return Err(PyValueError::new_err("offsets must be smaller than stride"));
        }
        let (model, vp) = (mat(model)?, mat(view_proj)?);
        let n = src.len() / stride;
        let p = &mut self.p;
        py.detach(|| p.project(&src, stride, offs, n, &model, &vp, width, height, sort));
        Ok((f32_bytes(py, self.p.out[..n * 4].iter().copied())?, u32_bytes(py, &self.p.order[..n])?))
    }

    fn __repr__(&self) -> String {
        "NativeProjector()".to_string()
    }
}

// ---------------------------------------------------------------------------
// Black hole
// ---------------------------------------------------------------------------

/// Physically based Schwarzschild black-hole renderer (see docs/black-hole.md).
#[pyclass(module = "anikit._anikit", name = "NativeBlackHole")]
pub struct NativeBlackHole {
    b: anikit_core::blackhole::BlackHole,
    mode: u32,
}

#[pymethods]
impl NativeBlackHole {
    #[new]
    #[pyo3(signature = (width, height, mode=0, seed=1))]
    fn new(width: usize, height: usize, mode: u32, seed: u32) -> PyResult<Self> {
        if width == 0 || height == 0 || width * height > 64 << 20 {
            return Err(PyValueError::new_err("black hole size out of range"));
        }
        Ok(NativeBlackHole { b: anikit_core::blackhole::BlackHole::new(width, height, seed), mode: mode.min(1) })
    }
    #[getter]
    fn width(&self) -> usize {
        self.b.w
    }
    #[getter]
    fn height(&self) -> usize {
        self.b.h
    }
    fn resize(&mut self, width: usize, height: usize) -> PyResult<()> {
        if width == 0 || height == 0 || width * height > 64 << 20 {
            return Err(PyValueError::new_err("black hole size out of range"));
        }
        self.b.resize(width, height);
        Ok(())
    }
    /// Render one frame; returns RGBA bytes.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (distance=30.0, inclination=80.0, fov=40.0, time=0.0, exposure=1.0, bloom=1.0, grain=1.0, hot_spots=1.0, jet=0.0, azimuth=0.0, look=0.0, roll=0.0, fall=0.0, temperature=None, disk_outer=22.0, stars=1.0, speed=1.0, nebula=1.0))]
    fn render<'py>(
        &mut self,
        py: Python<'py>,
        distance: f32,
        inclination: f32,
        fov: f32,
        time: f32,
        exposure: f32,
        bloom: f32,
        grain: f32,
        hot_spots: f32,
        jet: f32,
        azimuth: f32,
        look: f32,
        roll: f32,
        fall: f32,
        temperature: Option<f32>,
        disk_outer: f32,
        stars: f32,
        speed: f32,
        nebula: f32,
    ) -> Bound<'py, PyBytes> {
        let p = anikit_core::blackhole::Params {
            distance,
            inclination,
            fov,
            time,
            exposure,
            bloom,
            grain,
            hot_spots,
            jet,
            azimuth,
            look,
            roll,
            fall,
            mode: self.mode,
            temperature: temperature.unwrap_or(if self.mode == 1 { 6500.0 } else { 7000.0 }),
            disk_outer,
            stars,
            speed,
            nebula,
        };
        self.b.render(&p);
        PyBytes::new(py, &self.b.out)
    }
    /// `(samples, stars, table_built, geometry_built, shadow_radius)`.
    fn stats(&self) -> ((u32, u32, u32), u32, bool, bool, f32) {
        let s = self.b.stats;
        ((s.samples[0], s.samples[1], s.samples[2]), s.stars, s.table_built == 1, s.geometry_built == 1, s.shadow)
    }
}

// ---------------------------------------------------------------------------
// Point clouds and additive splatting
// ---------------------------------------------------------------------------

/// Particle cloud spring-following a blend between target shapes. Shapes and
/// positions are flat `[x, y, z] * n`; bulk outputs are native-endian `f32` bytes.
#[pyclass(module = "anikit._anikit", name = "NativeCloud")]
pub struct NativeCloud {
    c: Cloud,
}

impl NativeCloud {
    fn check_shape(&self, k: usize) -> PyResult<usize> {
        if k < self.c.shape_count() {
            Ok(k)
        } else {
            Err(PyIndexError::new_err(format!("shape index {k} out of range ({} shapes)", self.c.shape_count())))
        }
    }
}

#[pymethods]
impl NativeCloud {
    #[new]
    #[pyo3(signature = (n, shapes=0, seed=1))]
    fn new(n: usize, shapes: usize, seed: u32) -> PyResult<Self> {
        if n.checked_mul(3 * 4).is_none_or(|b| b > isize::MAX as usize) || shapes > 1 << 16 {
            return Err(PyMemoryError::new_err("point cloud too large"));
        }
        Ok(NativeCloud { c: Cloud::new(n, shapes, seed) })
    }

    // ---- parameters --------------------------------------------------------

    #[getter]
    fn get_stiffness(&self) -> f32 {
        self.c.params.stiffness
    }
    #[setter]
    fn set_stiffness(&mut self, v: f32) -> PyResult<()> {
        self.c.params.stiffness = finite("stiffness", v)?.max(0.0);
        Ok(())
    }
    #[getter]
    fn get_damping(&self) -> f32 {
        self.c.params.damping
    }
    #[setter]
    fn set_damping(&mut self, v: f32) -> PyResult<()> {
        self.c.params.damping = finite("damping", v)?.max(0.0);
        Ok(())
    }
    #[getter]
    fn get_spread(&self) -> f32 {
        self.c.params.spread
    }
    #[setter]
    fn set_spread(&mut self, v: f32) -> PyResult<()> {
        self.c.params.spread = finite("spread", v)?.max(0.0);
        Ok(())
    }
    #[getter]
    fn get_noise(&self) -> f32 {
        self.c.params.noise
    }
    #[setter]
    fn set_noise(&mut self, v: f32) -> PyResult<()> {
        self.c.params.noise = finite("noise", v)?;
        Ok(())
    }
    #[getter]
    fn get_swirl(&self) -> f32 {
        self.c.params.swirl
    }
    #[setter]
    fn set_swirl(&mut self, v: f32) -> PyResult<()> {
        self.c.params.swirl = finite("swirl", v)?;
        Ok(())
    }
    #[getter]
    fn get_frequency(&self) -> f32 {
        self.c.params.frequency
    }
    #[setter]
    fn set_frequency(&mut self, v: f32) -> PyResult<()> {
        self.c.params.frequency = finite("frequency", v)?;
        Ok(())
    }

    // ---- shapes ------------------------------------------------------------

    fn __len__(&self) -> usize {
        self.c.len()
    }

    fn shape_count(&self) -> usize {
        self.c.shape_count()
    }

    /// Define shape `k` from flat `[x, y, z] * n`. `k` may be at most
    /// `shape_count()` (replace a shape or append the next one).
    fn set_shape(&mut self, k: usize, data: F32Data<'_>) -> PyResult<()> {
        if k > self.c.shape_count() {
            return Err(PyIndexError::new_err(format!(
                "shape index {k} skips ahead: define shapes in order (next is {})",
                self.c.shape_count()
            )));
        }
        let v = data.into_vec()?;
        if v.len() != self.c.len() * 3 {
            return Err(PyValueError::new_err(format!(
                "shape needs {} values ([x, y, z] * {}), got {}",
                self.c.len() * 3,
                self.c.len(),
                v.len()
            )));
        }
        if v.iter().any(|x| !x.is_finite()) {
            return Err(PyValueError::new_err("shape values must be finite"));
        }
        self.c.shape_mut(k).copy_from_slice(&v);
        Ok(())
    }

    /// Shape `k` as native-endian `f32` bytes.
    fn shape<'py>(&mut self, py: Python<'py>, k: usize) -> PyResult<Bound<'py, PyBytes>> {
        let k = self.check_shape(k)?;
        f32_bytes(py, self.c.shape_mut(k).iter().copied())
    }

    /// Put every particle on shape `k` with zero velocity.
    fn snap(&mut self, k: usize) -> PyResult<()> {
        let k = self.check_shape(k)?;
        self.c.snap(k);
        Ok(())
    }

    // ---- simulation ----------------------------------------------------------

    /// Advance `steps` times by `dt` seconds each (each step is clamped to 1/20 s
    /// by the core) following the blend `from_ → to` at `mix ∈ [0, 1]`.
    /// Releases the GIL. A cloud without shapes does not move.
    #[pyo3(signature = (dt, from_, to, mix, steps=1))]
    fn step(&mut self, py: Python<'_>, dt: f32, from_: usize, to: usize, mix: f32, steps: u32) -> PyResult<()> {
        let dt = finite("dt", dt)?;
        let mix = finite("mix", mix)?;
        if self.c.shape_count() == 0 {
            return Ok(());
        }
        let (from_, to) = (self.check_shape(from_)?, self.check_shape(to)?);
        let c = &mut self.c;
        py.detach(|| {
            for _ in 0..steps {
                c.step(dt, from_, to, mix);
            }
        });
        Ok(())
    }

    /// Radial velocity kick away from `(x, y, z)` (scaled by 1/distance);
    /// negative strength pulls in.
    fn explode(&mut self, x: f32, y: f32, z: f32, strength: f32) -> PyResult<()> {
        self.c.explode(finite("x", x)?, finite("y", y)?, finite("z", z)?, finite("strength", strength)?);
        Ok(())
    }

    /// Mean particle speed (units/s).
    #[getter]
    fn mean_speed(&self) -> f32 {
        self.c.mean_speed()
    }

    /// `[x, y, z] * n` as native-endian `f32` bytes.
    fn positions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        f32_bytes(py, self.c.pos.iter().copied())
    }

    fn __repr__(&self) -> String {
        format!("NativeCloud(points={}, shapes={})", self.c.len(), self.c.shape_count())
    }
}

/// Additive point renderer: projected points (`[sx, sy, depth, scale] * n`,
/// e.g. from `NativeProjector.project`) accumulate as light, then tone-map to
/// RGBA bytes.
#[pyclass(module = "anikit._anikit", name = "NativeSplat")]
pub struct NativeSplat {
    s: Splat,
}

fn check_size(width: usize, height: usize) -> PyResult<()> {
    if width.checked_mul(height).and_then(|n| n.checked_mul(16)).is_none_or(|b| b > isize::MAX as usize) {
        return Err(PyMemoryError::new_err("splat buffer too large"));
    }
    Ok(())
}

#[pymethods]
impl NativeSplat {
    /// A `width × height` buffer (each at least 1).
    #[new]
    fn new(width: usize, height: usize) -> PyResult<Self> {
        check_size(width, height)?;
        Ok(NativeSplat { s: Splat::new(width, height) })
    }

    #[getter]
    fn width(&self) -> usize {
        self.s.width
    }
    #[getter]
    fn height(&self) -> usize {
        self.s.height
    }

    /// Resize (clears the accumulated light).
    fn resize(&mut self, width: usize, height: usize) -> PyResult<()> {
        check_size(width, height)?;
        self.s.resize(width, height);
        Ok(())
    }

    /// Multiply the accumulated light by `k` (0 clears; 0.8 leaves trails).
    fn fade(&mut self, k: f32) -> PyResult<()> {
        self.s.fade(finite("k", k)?);
        Ok(())
    }

    /// Per-point linear RGB in [0, 1], flat `[r, g, b] * n` (empty = all white).
    /// Colours apply only when they cover every drawn point.
    fn set_colors(&mut self, data: F32Data<'_>) -> PyResult<()> {
        let v = data.into_vec()?;
        if !v.len().is_multiple_of(3) {
            return Err(PyValueError::new_err("colors expects flat [r, g, b] * n"));
        }
        if v.iter().any(|x| !x.is_finite()) {
            return Err(PyValueError::new_err("colour values must be finite"));
        }
        self.s.colors = v;
        Ok(())
    }

    /// Number of points with a colour.
    fn color_count(&self) -> usize {
        self.s.colors.len() / 3
    }

    /// Splat projected points `[sx, sy, depth, scale] * n` (`scale <= 0` is
    /// culled). `fog_far = None` disables fog; `lens = (x, y, radius,
    /// strength)` pushes points away from a projected point (radius 0 = off).
    /// Releases the GIL.
    #[pyo3(signature = (proj, size=1.0, intensity=0.3, fog_near=0.0, fog_far=None, scale=1.0, lens=(0.0, 0.0, 0.0, 0.0), max_radius=6.0))]
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        py: Python<'_>,
        proj: F32Data<'_>,
        size: f32,
        intensity: f32,
        fog_near: f32,
        fog_far: Option<f32>,
        scale: f32,
        lens: (f32, f32, f32, f32),
        max_radius: f32,
    ) -> PyResult<()> {
        let mut proj = proj.into_vec()?;
        if !proj.len().is_multiple_of(4) {
            return Err(PyValueError::new_err("draw expects flat [sx, sy, depth, scale] * n"));
        }
        // Cull points with a non-finite position or scale (the core would draw
        // a NaN scale as a max-radius disc). Depth may be infinite (culled).
        for o in proj.chunks_exact_mut(4) {
            if !(o[0].is_finite() && o[1].is_finite() && o[3].is_finite()) {
                o[3] = 0.0;
            }
        }
        let fog_far = match fog_far {
            None => f32::INFINITY,
            Some(f) if f.is_nan() => return Err(PyValueError::new_err("fog_far must not be NaN")),
            Some(f) => f,
        };
        let scale = finite("scale", scale)?;
        if scale <= 0.0 {
            return Err(PyValueError::new_err("scale must be > 0"));
        }
        let p = SplatParams {
            size: finite("size", size)?.max(0.0),
            intensity: finite("intensity", intensity)?,
            fog_near: finite("fog_near", fog_near)?,
            fog_far,
            scale,
            lens: [finite("lens", lens.0)?, finite("lens", lens.1)?, finite("lens", lens.2)?, finite("lens", lens.3)?],
            max_radius: finite("max_radius", max_radius)?,
            ..SplatParams::default()
        };
        let n = proj.len() / 4;
        let s = &mut self.s;
        py.detach(|| s.draw(&proj, n, &p));
        Ok(())
    }

    /// Tone-map the light over `background` (0xRRGGBB) and return the frame as
    /// RGBA bytes (`width * height * 4`, row-major, alpha 255). Afterwards the
    /// light is multiplied by `keep` (1 = keep, 0 = clear, 0.85 = trails).
    #[pyo3(signature = (exposure=1.0, background=0, keep=1.0))]
    fn resolve<'py>(&mut self, py: Python<'py>, exposure: f32, background: u32, keep: f32) -> PyResult<Bound<'py, PyBytes>> {
        let (exposure, keep) = (finite("exposure", exposure)?, finite("keep", keep)?);
        let s = &mut self.s;
        py.detach(|| s.resolve_fade(exposure, background & 0xff_ffff, keep));
        self.rgba(py)
    }

    /// RGBA bytes of the last `resolve` (all zero before the first).
    fn rgba<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        PyBytes::new_with(py, self.s.rgba.len() * 4, |buf| {
            for (chunk, b) in buf.iter_mut().zip(self.s.bytes()) {
                *chunk = b;
            }
            Ok(())
        })
    }

    /// `(r, g, b, a)` of the last `resolve` at pixel `(x, y)`.
    fn pixel(&self, x: usize, y: usize) -> PyResult<(u8, u8, u8, u8)> {
        if x >= self.s.width || y >= self.s.height {
            return Err(PyIndexError::new_err(format!("pixel ({x}, {y}) outside {}x{}", self.s.width, self.s.height)));
        }
        let [r, g, b, a] = self.s.rgba[y * self.s.width + x].to_le_bytes();
        Ok((r, g, b, a))
    }

    /// Linear light accumulated at pixel `(x, y)`.
    fn light(&self, x: usize, y: usize) -> PyResult<(f32, f32, f32)> {
        if x >= self.s.width || y >= self.s.height {
            return Err(PyIndexError::new_err(format!("pixel ({x}, {y}) outside {}x{}", self.s.width, self.s.height)));
        }
        let [r, g, b] = self.s.light(x, y);
        Ok((r, g, b))
    }

    /// Sum of the accumulated light over all pixels `(r, g, b)`.
    fn total_light(&self) -> (f64, f64, f64) {
        let mut t = (0.0, 0.0, 0.0);
        for y in 0..self.s.height {
            for x in 0..self.s.width {
                let [r, g, b] = self.s.light(x, y);
                t.0 += r as f64;
                t.1 += g as f64;
                t.2 += b as f64;
            }
        }
        t
    }

    fn __repr__(&self) -> String {
        format!("NativeSplat({}x{})", self.s.width, self.s.height)
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

#[pymodule]
fn _anikit(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    m.add_class::<NativeEasing>()?;
    m.add_class::<NativePath>()?;
    m.add_class::<NativeMorph>()?;
    m.add_class::<NativeMorphSeq>()?;
    m.add_class::<NativeNoise>()?;
    m.add_class::<NativeWorld>()?;
    m.add_class::<NativeProjector>()?;
    m.add_class::<NativeCloud>()?;
    m.add_class::<NativeBlackHole>()?;
    m.add_class::<NativeSplat>()?;
    render3d::register(m)?;
    m.add_function(wrap_pyfunction!(ease, m)?)?;
    m.add_function(wrap_pyfunction!(easing_css, m)?)?;
    m.add_function(wrap_pyfunction!(spring_settle_time, m)?)?;
    m.add_function(wrap_pyfunction!(spring_response, m)?)?;
    m.add_function(wrap_pyfunction!(spring_from_duration_bounce, m)?)?;
    m.add_function(wrap_pyfunction!(color_parse, m)?)?;
    m.add_function(wrap_pyfunction!(color_try_parse, m)?)?;
    m.add_function(wrap_pyfunction!(color_mix, m)?)?;
    m.add_function(wrap_pyfunction!(color_to_css, m)?)?;
    m.add_function(wrap_pyfunction!(color_to_hex, m)?)?;
    m.add_function(wrap_pyfunction!(color_space_id, m)?)?;
    m.add_function(wrap_pyfunction!(morph_write, m)?)?;
    m.add_function(wrap_pyfunction!(stagger, m)?)?;
    m.add_function(wrap_pyfunction!(transform_parse, m)?)?;
    m.add_function(wrap_pyfunction!(transform_mix, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_decompose, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_recompose, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_mix, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_mix_decomposed, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_mul, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_invert, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_determinant, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_transform_point, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_translate, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_scale, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_rotate, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_skew, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_perspective_css, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_look_at, m)?)?;
    m.add_function(wrap_pyfunction!(mat4_perspective, m)?)?;
    m.add_function(wrap_pyfunction!(quat_euler, m)?)?;
    m.add_function(wrap_pyfunction!(quat_axis_angle, m)?)?;
    m.add_function(wrap_pyfunction!(quat_mul, m)?)?;
    m.add_function(wrap_pyfunction!(quat_slerp, m)?)?;
    m.add_function(wrap_pyfunction!(quat_to_matrix, m)?)?;

    m.add("LINEAR", LINEAR)?;
    m.add("NONE", NONE)?;
    // Event kinds.
    m.add("EVENT_START", core_engine::event::START)?;
    m.add("EVENT_REPEAT", core_engine::event::REPEAT)?;
    m.add("EVENT_COMPLETE", core_engine::event::COMPLETE)?;
    m.add("EVENT_REVERSE_COMPLETE", core_engine::event::REVERSE_COMPLETE)?;
    m.add("EVENT_CALLBACK", core_engine::event::CALLBACK)?;
    m.add("EVENT_FINISH", core_engine::event::FINISH)?;
    // Node flags.
    m.add("FLAG_FILL_BACKWARDS", core_engine::flags::FILL_BACKWARDS)?;
    m.add("FLAG_FILL_FORWARDS", core_engine::flags::FILL_FORWARDS)?;
    m.add("FLAG_IMMEDIATE", core_engine::flags::IMMEDIATE)?;
    m.add("FLAG_AUTO_RELEASE", core_engine::flags::AUTO_RELEASE)?;
    m.add("FLAG_DEFAULT", core_engine::flags::DEFAULT)?;
    // Track kinds.
    m.add("TRACK_NUMBER", 0u8)?;
    m.add("TRACK_COLOR", 1u8)?;
    m.add("TRACK_DISCRETE", 2u8)?;
    m.add("TRACK_PATH", 3u8)?;
    m.add("TRACK_MATRIX", 4u8)?;
    // Play states.
    m.add("STATE_IDLE", 0u8)?;
    m.add("STATE_PLAYING", 1u8)?;
    m.add("STATE_PAUSED", 2u8)?;
    m.add("STATE_FINISHED", 3u8)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
