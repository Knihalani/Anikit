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

use pyo3::exceptions::{PyKeyError, PyMemoryError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;

use anikit_core::color::{self, ColorSpace, Rgba};
use anikit_core::css;
use anikit_core::easing::Easing;
use anikit_core::engine::{self as core_engine, Error, Handle, PlayState, TrackKind, TrackSpec, LINEAR, NONE};
use anikit_core::morph::{self, MorphPlan};
use anikit_core::path::{Path, PathLut};
use anikit_core::spring::SpringParams;
use anikit_core::stagger::{self as core_stagger, Axis, Stagger, StaggerFrom};
use anikit_core::timing::{Direction, Timing};

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
/// * `kind`: 0 number, 1 colour, 2 discrete, 3 path
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
// Module
// ---------------------------------------------------------------------------

#[pymodule]
fn _anikit(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    m.add_class::<NativeEasing>()?;
    m.add_class::<NativePath>()?;
    m.add_class::<NativeMorph>()?;
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
    // Play states.
    m.add("STATE_IDLE", 0u8)?;
    m.add("STATE_PLAYING", 1u8)?;
    m.add("STATE_PAUSED", 2u8)?;
    m.add("STATE_FINISHED", 3u8)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
