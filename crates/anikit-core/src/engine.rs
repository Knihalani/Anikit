//! The animation engine.
//!
//! Design goals:
//!
//! * **One call per frame.** [`Engine::tick`] advances every playing animation,
//!   timeline and physics simulation, writes results into per-node output
//!   buffers and records *which* nodes changed (`updated`) plus discrete
//!   events (start/repeat/complete/callback). Bindings only touch what changed.
//! * **No allocation in the hot path.** All buffers are sized at creation.
//!   Group children are iterated in place; easings are interned.
//! * **Stable output memory.** Output buffers never move after creation, so the
//!   WebAssembly host can read values directly from linear memory (zero copy).
//! * **Generational handles.** Stale handles are detected and ignored, making
//!   use-after-free impossible from the bindings.
//! * **Exact physics.** Springs use the closed-form solution (see
//!   [`crate::spring`]); one state-transition matrix per distinct spring
//!   configuration per frame advances all channels of all springs.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::color::{self, ColorSpace, Rgba};
use crate::easing::Easing;
use crate::math::{abs, exp};
use crate::path::{sample_lut, PathLut};
use crate::spring::SpringParams;
use crate::timing::{Phase, Timing};

pub type Handle = u32;
pub const NONE: Handle = u32::MAX;
pub const LINEAR: u32 = u32::MAX;

const INDEX_BITS: u32 = 20;
const INDEX_MASK: u32 = (1 << INDEX_BITS) - 1;
const GEN_MASK: u32 = (1 << (32 - INDEX_BITS)) - 1;
const MAX_NODES: usize = INDEX_MASK as usize; // index INDEX_MASK is never used

/// Event kinds reported through [`Engine::events`].
pub mod event {
    /// Entered the active phase moving forward.
    pub const START: u32 = 1;
    /// Iteration changed while active.
    pub const REPEAT: u32 = 2;
    /// Reached the end of the active phase moving forward.
    pub const COMPLETE: u32 = 3;
    /// Went back before the start while moving backward.
    pub const REVERSE_COMPLETE: u32 = 4;
    /// A timeline callback was crossed.
    pub const CALLBACK: u32 = 5;
    /// A root animation / simulation stopped playing on its own.
    pub const FINISH: u32 = 6;
}

/// Node flags.
pub mod flags {
    /// Render start values while before the active phase (and when moving back past the start).
    pub const FILL_BACKWARDS: u8 = 1;
    /// Keep end values after the active phase.
    pub const FILL_FORWARDS: u8 = 2;
    /// Render start values immediately on creation (e.g. `from` tweens with a delay).
    pub const IMMEDIATE: u8 = 4;
    /// Free the node automatically once it finishes.
    pub const AUTO_RELEASE: u8 = 8;
    pub const DEFAULT: u8 = FILL_BACKWARDS | FILL_FORWARDS;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayState {
    Idle = 0,
    Playing = 1,
    Paused = 2,
    Finished = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    /// Plain numeric channels, linearly interpolated.
    Number,
    /// RGBA colour (input/output: r,g,b 0–255, a 0–1) interpolated in a colour space.
    Color(ColorSpace),
    /// Non-interpolable: outputs the value of the keyframe in effect (switches at 50%).
    Discrete,
    /// Motion along a path: keyframe values are progress along the path (0–1);
    /// outputs `x, y, angle°`.
    Path,
}

/// Description of one animated track (one property).
#[derive(Clone, Debug)]
pub struct TrackSpec {
    pub kind: TrackKind,
    /// Values per keyframe (Number: any; Color: 4; Discrete/Path: 1).
    pub channels: usize,
    /// Keyframe offsets in `[0, 1]`. Empty → evenly distributed.
    pub offsets: Vec<f64>,
    /// `keyframes * channels` values.
    pub values: Vec<f64>,
    /// Easing ids: empty (linear), one (all segments) or `keyframes - 1`.
    pub easings: Vec<u32>,
    /// Lookup table for [`TrackKind::Path`].
    pub path: Option<PathLut>,
}

impl TrackSpec {
    pub fn number(values: Vec<f64>, channels: usize) -> Self {
        TrackSpec { kind: TrackKind::Number, channels, offsets: Vec::new(), values, easings: Vec::new(), path: None }
    }
    pub fn with_offsets(mut self, o: Vec<f64>) -> Self {
        self.offsets = o;
        self
    }
    pub fn with_easings(mut self, e: Vec<u32>) -> Self {
        self.easings = e;
        self
    }
}

#[derive(Clone, Debug)]
struct Track {
    kind: TrackKind,
    ch: u32,
    out_ch: u32,
    nkf: u32,
    off_at: u32,
    val_at: u32,
    ease_at: u32,
    ease_n: u32,
    lut_at: u32,
    lut_len: u32,
    out_at: u32,
    cursor: u32,
    enabled: bool,
    /// Fast path: a single segment spanning [0, 1] on a numeric track.
    single: bool,
    /// Exactly two keyframes at offsets 0 and 1 (any kind): no segment search.
    seg1: bool,
    /// Easing of the first segment (used by the fast path).
    ease0: u32,
}

/// A keyframe animation. Everything it needs per frame lives in two
/// allocations: the track table and one `f64` buffer laid out as
/// `[offsets/values/LUTs/easing ids…, outputs…]`.
#[derive(Debug)]
struct Anim {
    tracks: Box<[Track]>,
    buf: Box<[f64]>,
    out_at: usize,
}

impl Anim {
    #[inline]
    fn out(&self) -> &[f64] {
        &self.buf[self.out_at..]
    }
    #[inline]
    fn eval(&mut self, easings: &[Easing], p: f64) {
        let (data, out) = self.buf.split_at_mut(self.out_at);
        Anim::eval_into(&mut self.tracks, data, easings, p, out);
    }
}

#[derive(Debug, Default)]
struct Group {
    children: Vec<u32>,
    auto_duration: bool,
    last_inner: f64,
}

/// Instanced animation: `n` instances share keyframe offsets, easing and
/// timing but have their own values and start delay. Output is one
/// contiguous `f32` buffer (`n * channels`) ready for GPU upload / canvas.
#[derive(Debug)]
struct Batch {
    n: usize,
    ch: usize,
    nkf: usize,
    timing: Timing,
    offsets: Box<[f64]>,
    eases: Box<[u32]>,
    values: Box<[f64]>,
    delays: Box<[f64]>,
    scratch: Box<[f64]>,
    seg: Box<[u32]>,
    out: Box<[f32]>,
}

/// Description of an instanced batch, see [`Engine::create_batch`].
#[derive(Clone, Debug)]
pub struct BatchSpec {
    pub count: usize,
    pub channels: usize,
    /// Keyframe offsets (empty → evenly spaced).
    pub offsets: Vec<f64>,
    /// Easing ids per segment (empty, one, or `keyframes - 1`).
    pub easings: Vec<u32>,
    /// `count * keyframes * channels`, instance-major.
    pub values: Vec<f64>,
    /// Per-instance start delay in ms (empty → all zero).
    pub delays: Vec<f64>,
}

impl Batch {
    fn eval(&mut self, easings: &[Easing], t: f64) {
        let (n, ch, nkf) = (self.n, self.ch, self.nkf);
        let tm = self.timing;
        // 1. Per-instance progress (shared timing, own delay).
        for (p, &d) in self.scratch.iter_mut().zip(self.delays.iter()) {
            *p = tm.eval(t - d).progress;
        }
        // 2. Timing easing over the whole slice (one dispatch).
        if tm.easing != LINEAR {
            if let Some(e) = easings.get(tm.easing as usize) {
                e.ease_slice(&mut self.scratch);
            }
        }
        if nkf == 2 && self.offsets[0] == 0.0 && self.offsets[1] == 1.0 {
            // 3a. Single segment: ease the slice, then lerp all channels.
            if let Some(&id) = self.eases.first() {
                if let Some(e) = easings.get(id as usize) {
                    e.ease_slice(&mut self.scratch);
                }
            }
            let stride = 2 * ch;
            for i in 0..n {
                let e = self.scratch[i];
                let v = &self.values[i * stride..i * stride + stride];
                let o = &mut self.out[i * ch..i * ch + ch];
                for k in 0..ch {
                    o[k] = (v[k] + (v[ch + k] - v[k]) * e) as f32;
                }
            }
            return;
        }
        // 3b. General keyframes.
        let stride = nkf * ch;
        let off = &self.offsets;
        for i in 0..n {
            let p = self.scratch[i];
            let mut s = (self.seg[i] as usize).min(nkf.saturating_sub(2));
            while s > 0 && p < off[s] {
                s -= 1;
            }
            while s + 2 < nkf && p >= off[s + 1] {
                s += 1;
            }
            self.seg[i] = s as u32;
            let (e, a_at) = if nkf < 2 {
                (0.0, 0)
            } else {
                let span = off[s + 1] - off[s];
                let local = if span > 0.0 { (p - off[s]) / span } else { 1.0 };
                let id = match self.eases.len() {
                    0 => LINEAR,
                    1 => self.eases[0],
                    _ => self.eases[s],
                };
                (ease_by(easings, id, local), s * ch)
            };
            let v = &self.values[i * stride..i * stride + stride];
            let o = &mut self.out[i * ch..i * ch + ch];
            for k in 0..ch {
                let a = v[a_at + k];
                let b = if nkf < 2 { a } else { v[a_at + ch + k] };
                o[k] = (a + (b - a) * e) as f32;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimKind {
    Spring,
    Decay,
}

#[derive(Debug)]
struct Sim {
    kind: SimKind,
    ch: usize,
    cfg: u32,
    /// `[pos * ch, vel * ch, target * ch, min * ch, max * ch]`
    buf: Vec<f64>,
    /// Per channel for decay: 0 = decaying, 1 = springing to a bound, 2 = at rest.
    mode: Vec<u8>,
    time_constant: f64,
    rest_delta: f64,
    rest_speed: f64,
}

#[derive(Debug)]
enum Kind {
    Free,
    Anim(Anim),
    Group(Group),
    Callback,
    Sim(Box<Sim>),
    Batch(Box<Batch>),
}

const PHASE_UNKNOWN: u8 = 3;

#[derive(Debug)]
struct Node {
    gen: u32,
    parent: u32,
    start: f64,
    rate: f64,
    timing: Timing,
    time: f64,
    state: PlayState,
    last_phase: u8,
    last_iter: f64,
    flags: u8,
    active_at: u32,
    kind: Kind,
}

impl Node {
    fn empty() -> Node {
        Node {
            gen: 1,
            parent: NONE,
            start: 0.0,
            rate: 1.0,
            timing: Timing::default(),
            time: 0.0,
            state: PlayState::Idle,
            last_phase: Phase::Before as u8,
            last_iter: 0.0,
            flags: flags::DEFAULT,
            active_at: NONE,
            kind: Kind::Free,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SpringCfg {
    params: SpringParams,
    matrix: [f64; 4],
    dt: f64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Error {
    BadHandle,
    WrongKind,
    InvalidInput,
    Capacity,
    NotRoot,
}

/// The animation engine. See the module docs.
pub struct Engine {
    nodes: Vec<Node>,
    free: Vec<u32>,
    active: Vec<u32>,
    active_sims: Vec<u32>,
    easings: Vec<Easing>,
    easing_names: BTreeMap<String, u32>,
    springs: Vec<SpringCfg>,
    updated: Vec<Handle>,
    events: Vec<u32>,
    pub now: f64,
    pub rate: f64,
    lag_threshold: f64,
    lag_adjusted: f64,
    released_scratch: Vec<u32>,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

#[inline(always)]
fn ease_by(easings: &[Easing], id: u32, t: f64) -> f64 {
    if id == LINEAR {
        t
    } else {
        match easings.get(id as usize) {
            Some(e) => e.ease(t),
            None => t,
        }
    }
}

impl Anim {
    /// Evaluate every enabled track at (eased) iteration progress `p` into `out`.
    #[inline]
    fn eval_into(tracks: &mut [Track], data: &[f64], easings: &[Easing], p: f64, out: &mut [f64]) {
        for tr in tracks.iter_mut() {
            if !tr.enabled {
                continue;
            }
            if tr.single {
                // Hot path: from → to tween. No segment search, no kind dispatch.
                let e = ease_by(easings, tr.ease0, p);
                let va = tr.val_at as usize;
                let oa = tr.out_at as usize;
                let ch = tr.ch as usize;
                if ch == 1 {
                    if let (Some(&a), Some(&b), Some(o)) = (data.get(va), data.get(va + 1), out.get_mut(oa)) {
                        *o = a + (b - a) * e;
                    }
                } else {
                    let (a, b) = data[va..va + 2 * ch].split_at(ch);
                    let o = &mut out[oa..oa + ch];
                    for k in 0..ch {
                        o[k] = a[k] + (b[k] - a[k]) * e;
                    }
                }
                continue;
            }
            let n = tr.nkf as usize;
            let ch = tr.ch as usize;
            let off = &data[tr.off_at as usize..tr.off_at as usize + n];
            let vals = &data[tr.val_at as usize..tr.val_at as usize + n * ch];
            let o = &mut out[tr.out_at as usize..tr.out_at as usize + tr.out_ch as usize];
            // Segment search with a temporal-coherence cursor: amortised O(1).
            let (i, e) = if tr.seg1 {
                (0usize, ease_by(easings, tr.ease0, p))
            } else if n <= 1 {
                (0usize, 0.0)
            } else {
                let mut i = (tr.cursor as usize).min(n - 2);
                while i > 0 && p < off[i] {
                    i -= 1;
                }
                while i < n - 2 && p >= off[i + 1] {
                    i += 1;
                }
                tr.cursor = i as u32;
                let span = off[i + 1] - off[i];
                let local = if span > 0.0 {
                    (p - off[i]) / span
                } else if p >= off[i + 1] {
                    1.0
                } else {
                    0.0
                };
                let id = match tr.ease_n {
                    0 => LINEAR,
                    1 => tr.ease0,
                    _ => data[tr.ease_at as usize + i] as u32,
                };
                (i, ease_by(easings, id, local))
            };
            if n <= 1 {
                match tr.kind {
                    TrackKind::Color(space) => {
                        let c = color::from_space(&vals[..4], space);
                        o.copy_from_slice(&c.to_array());
                    }
                    TrackKind::Path => {
                        let lut = &data[tr.lut_at as usize..(tr.lut_at + tr.lut_len) as usize];
                        o.copy_from_slice(&sample_lut(lut, vals.first().copied().unwrap_or(0.0)));
                    }
                    _ => o.copy_from_slice(&vals[..ch.min(o.len())]),
                }
                continue;
            }
            let a = &vals[i * ch..i * ch + ch];
            let b = &vals[(i + 1) * ch..(i + 1) * ch + ch];
            match tr.kind {
                TrackKind::Number => {
                    // Tight loop: auto-vectorised (wasm simd128 / SSE / NEON).
                    for ((o, &a), &b) in o.iter_mut().zip(a).zip(b) {
                        *o = a + (b - a) * e;
                    }
                }
                TrackKind::Color(space) => {
                    let mut c = [0.0; 4];
                    for k in 0..4 {
                        c[k] = a[k] + (b[k] - a[k]) * e;
                    }
                    o.copy_from_slice(&color::from_space(&c, space).to_array());
                }
                TrackKind::Discrete => {
                    o[0] = if e < 0.5 { a[0] } else { b[0] };
                }
                TrackKind::Path => {
                    let lut = &data[tr.lut_at as usize..(tr.lut_at + tr.lut_len) as usize];
                    o.copy_from_slice(&sample_lut(lut, a[0] + (b[0] - a[0]) * e));
                }
            }
        }
    }
}

impl Engine {
    pub const fn new() -> Engine {
        Engine {
            nodes: Vec::new(),
            free: Vec::new(),
            active: Vec::new(),
            active_sims: Vec::new(),
            easings: Vec::new(),
            easing_names: BTreeMap::new(),
            springs: Vec::new(),
            updated: Vec::new(),
            events: Vec::new(),
            now: 0.0,
            rate: 1.0,
            lag_threshold: 500.0,
            lag_adjusted: 33.0,
            released_scratch: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Handles
    // ------------------------------------------------------------------

    #[inline]
    fn resolve(&self, h: Handle) -> Result<usize, Error> {
        let idx = (h & INDEX_MASK) as usize;
        let gen = h >> INDEX_BITS;
        match self.nodes.get(idx) {
            Some(n) if n.gen == gen && !matches!(n.kind, Kind::Free) => Ok(idx),
            _ => Err(Error::BadHandle),
        }
    }

    #[inline]
    fn handle_of(&self, idx: usize) -> Handle {
        (self.nodes[idx].gen << INDEX_BITS) | idx as u32
    }

    /// Index part of a handle (dense, reused after release) — handy as an
    /// array index on the binding side.
    #[inline]
    pub fn handle_index(h: Handle) -> u32 {
        h & INDEX_MASK
    }

    pub fn is_alive(&self, h: Handle) -> bool {
        self.resolve(h).is_ok()
    }

    fn alloc(&mut self, kind: Kind, timing: Timing, fl: u8) -> Result<usize, Error> {
        let idx = if let Some(i) = self.free.pop() {
            i as usize
        } else {
            if self.nodes.len() >= MAX_NODES {
                return Err(Error::Capacity);
            }
            self.nodes.push(Node::empty());
            self.nodes.len() - 1
        };
        let gen = self.nodes[idx].gen;
        let n = &mut self.nodes[idx];
        *n = Node::empty();
        n.gen = gen;
        n.kind = kind;
        n.timing = timing;
        n.flags = fl;
        n.last_phase = if fl & flags::IMMEDIATE != 0 { PHASE_UNKNOWN } else { Phase::Before as u8 };
        Ok(idx)
    }

    // ------------------------------------------------------------------
    // Easing registry
    // ------------------------------------------------------------------

    /// Register an easing, returning its id. Identical easings are shared.
    pub fn add_easing(&mut self, e: Easing) -> u32 {
        if e == Easing::Linear {
            return LINEAR;
        }
        if let Some(i) = self.easings.iter().position(|x| *x == e) {
            return i as u32;
        }
        self.easings.push(e);
        (self.easings.len() - 1) as u32
    }

    /// Parse and register an easing (cached by string).
    pub fn easing(&mut self, s: &str) -> Option<u32> {
        if let Some(&id) = self.easing_names.get(s) {
            return Some(id);
        }
        let e = Easing::parse(s)?;
        let id = self.add_easing(e);
        self.easing_names.insert(String::from(s), id);
        Some(id)
    }

    pub fn easing_ref(&self, id: u32) -> Option<&Easing> {
        self.easings.get(id as usize)
    }

    pub fn ease(&self, id: u32, t: f64) -> f64 {
        ease_by(&self.easings, id, t)
    }

    // ------------------------------------------------------------------
    // Creation
    // ------------------------------------------------------------------

    /// Create a keyframe animation. It starts paused (idle); call [`play`](Self::play)
    /// or add it to a timeline.
    pub fn create_anim(&mut self, timing: Timing, specs: &[TrackSpec], fl: u8) -> Result<Handle, Error> {
        // Build into temporary vectors, then pack into the compact layout.
        let mut data: Vec<f64> = Vec::new();
        let mut eases: Vec<u32> = Vec::new();
        let mut tracks: Vec<Track> = Vec::with_capacity(specs.len());
        let mut out_len = 0u32;
        for spec in specs {
            let ch = spec.channels.max(1);
            let nkf = spec.values.len() / ch;
            if nkf == 0 || spec.values.len() != nkf * ch {
                return Err(Error::InvalidInput);
            }
            if !spec.offsets.is_empty() && spec.offsets.len() != nkf {
                return Err(Error::InvalidInput);
            }
            let (store_ch, out_ch) = match spec.kind {
                TrackKind::Number => (ch, ch),
                TrackKind::Color(_) => {
                    if ch != 4 {
                        return Err(Error::InvalidInput);
                    }
                    (4, 4)
                }
                TrackKind::Discrete => (ch, ch),
                TrackKind::Path => {
                    if ch != 1 || spec.path.is_none() {
                        return Err(Error::InvalidInput);
                    }
                    (1, 3)
                }
            };
            let off_at = data.len() as u32;
            if spec.offsets.is_empty() {
                for i in 0..nkf {
                    data.push(if nkf == 1 { 1.0 } else { i as f64 / (nkf - 1) as f64 });
                }
            } else {
                // Clamp to non-decreasing order.
                let mut m = f64::NEG_INFINITY;
                for &o in &spec.offsets {
                    let o = if o.is_finite() { o.max(m) } else { m.max(0.0) };
                    data.push(o);
                    m = o;
                }
            }
            let val_at = data.len() as u32;
            match spec.kind {
                TrackKind::Color(space) => {
                    let start = data.len();
                    for k in 0..nkf {
                        let v = &spec.values[k * 4..k * 4 + 4];
                        data.extend_from_slice(&color::to_space(Rgba::new(v[0], v[1], v[2], v[3]), space));
                    }
                    color::fix_hues(&mut data[start..], space);
                }
                _ => data.extend_from_slice(&spec.values),
            }
            let (lut_at, lut_len) = if let (TrackKind::Path, Some(p)) = (spec.kind, &spec.path) {
                let at = data.len() as u32;
                data.extend_from_slice(&p.data);
                (at, p.data.len() as u32)
            } else {
                (0, 0)
            };
            let ease_at = eases.len() as u32;
            let ease_n = match spec.easings.len() {
                0 => 0,
                1 => 1,
                n if n == nkf.saturating_sub(1) => n as u32,
                _ => return Err(Error::InvalidInput),
            };
            eases.extend_from_slice(&spec.easings[..ease_n as usize]);
            tracks.push(Track {
                kind: spec.kind,
                ch: store_ch as u32,
                out_ch: out_ch as u32,
                nkf: nkf as u32,
                off_at,
                val_at,
                ease_at,
                ease_n,
                lut_at,
                lut_len,
                out_at: out_len,
                cursor: 0,
                enabled: true,
                single: spec.kind == TrackKind::Number
                    && nkf == 2
                    && data[off_at as usize] == 0.0
                    && data[off_at as usize + 1] == 1.0,
                seg1: nkf == 2 && data[off_at as usize] == 0.0 && data[off_at as usize + 1] == 1.0,
                ease0: if ease_n > 0 { eases[ease_at as usize] } else { LINEAR },
            });
            out_len += out_ch as u32;
        }
        // Pack: data, then easing ids (as f64), then outputs.
        let ease_base = data.len() as u32;
        for t in tracks.iter_mut() {
            t.ease_at += ease_base;
        }
        data.extend(eases.iter().map(|&e| e as f64));
        let out_at = data.len();
        data.resize(out_at + out_len as usize, 0.0);
        let mut anim = Anim { tracks: tracks.into_boxed_slice(), buf: data.into_boxed_slice(), out_at };
        // Pre-fill outputs with the start values so readers never see zeros.
        anim.eval(&self.easings, 0.0);
        let idx = self.alloc(Kind::Anim(anim), sanitize(timing), fl)?;
        Ok(self.handle_of(idx))
    }

    /// Create an (empty) timeline group. Its duration is derived from its
    /// children unless `timing.duration` is finite and `auto_duration` is false.
    pub fn create_group(&mut self, timing: Timing, auto_duration: bool, fl: u8) -> Result<Handle, Error> {
        let mut t = sanitize(timing);
        if auto_duration {
            t.duration = 0.0;
        }
        let idx = self.alloc(Kind::Group(Group { children: Vec::new(), auto_duration, last_inner: 0.0 }), t, fl)?;
        Ok(self.handle_of(idx))
    }

    /// Create an instanced batch animation (see [`BatchSpec`]). `timing`
    /// applies to each instance (its `delay` is ignored; use per-instance
    /// delays). The batch node's own duration covers the latest instance.
    pub fn create_batch(&mut self, timing: Timing, spec: BatchSpec, fl: u8) -> Result<Handle, Error> {
        let (n, ch) = (spec.count, spec.channels.max(1));
        if n == 0 || !spec.values.len().is_multiple_of(n * ch) {
            return Err(Error::InvalidInput);
        }
        let nkf = spec.values.len() / (n * ch);
        if nkf == 0 || (!spec.offsets.is_empty() && spec.offsets.len() != nkf) {
            return Err(Error::InvalidInput);
        }
        if !spec.delays.is_empty() && spec.delays.len() != n {
            return Err(Error::InvalidInput);
        }
        if !(spec.easings.len() <= 1 || spec.easings.len() == nkf - 1) {
            return Err(Error::InvalidInput);
        }
        let offsets: Vec<f64> = if spec.offsets.is_empty() {
            (0..nkf).map(|i| if nkf == 1 { 1.0 } else { i as f64 / (nkf - 1) as f64 }).collect()
        } else {
            spec.offsets
        };
        let mut tm = sanitize(timing);
        tm.delay = 0.0;
        let delays: Vec<f64> = if spec.delays.is_empty() { alloc::vec![0.0; n] } else { spec.delays };
        let max_delay = delays.iter().cloned().fold(0.0, f64::max);
        let node_timing = Timing {
            delay: 0.0,
            duration: max_delay + tm.active_duration() + tm.end_delay,
            iterations: 1.0,
            direction: crate::timing::Direction::Normal,
            end_delay: 0.0,
            easing: LINEAR,
        };
        let mut b = Batch {
            n,
            ch,
            nkf,
            timing: tm,
            offsets: offsets.into_boxed_slice(),
            eases: spec.easings.into_boxed_slice(),
            values: spec.values.into_boxed_slice(),
            delays: delays.into_boxed_slice(),
            scratch: alloc::vec![0.0; n].into_boxed_slice(),
            seg: alloc::vec![0; n].into_boxed_slice(),
            out: alloc::vec![0.0f32; n * ch].into_boxed_slice(),
        };
        b.eval(&self.easings, 0.0);
        let idx = self.alloc(Kind::Batch(Box::new(b)), node_timing, fl)?;
        Ok(self.handle_of(idx))
    }

    /// Output of a batch: `count * channels` `f32` values (instance-major).
    pub fn batch_values(&self, h: Handle) -> Result<&[f32], Error> {
        let i = self.resolve(h)?;
        match &self.nodes[i].kind {
            Kind::Batch(b) => Ok(&b.out),
            _ => Err(Error::WrongKind),
        }
    }

    /// Create a callback marker; add it to a group to fire [`event::CALLBACK`]
    /// whenever playback crosses its start time (in either direction).
    pub fn create_callback(&mut self) -> Result<Handle, Error> {
        let t = Timing { duration: 0.0, ..Timing::default() };
        let idx = self.alloc(Kind::Callback, t, flags::DEFAULT)?;
        Ok(self.handle_of(idx))
    }

    /// Add `child` (a root node) to `group`, starting at `start` ms of the group's time.
    pub fn group_add(&mut self, group: Handle, child: Handle, start: f64) -> Result<(), Error> {
        let g = self.resolve(group)?;
        let c = self.resolve(child)?;
        if g == c || !matches!(self.nodes[g].kind, Kind::Group(_)) {
            return Err(Error::WrongKind);
        }
        if matches!(self.nodes[c].kind, Kind::Sim(_)) {
            return Err(Error::WrongKind);
        }
        // Reject cycles: `group` must not be a descendant of `child`.
        let mut p = self.nodes[g].parent;
        while p != NONE {
            if p as usize == c {
                return Err(Error::InvalidInput);
            }
            p = self.nodes[p as usize].parent;
        }
        self.detach(c);
        self.deactivate(c);
        let n = &mut self.nodes[c];
        n.parent = g as u32;
        n.start = if start.is_finite() { start } else { 0.0 };
        n.state = PlayState::Playing;
        if n.rate <= 0.0 {
            n.rate = 1.0;
        }
        if let Kind::Group(gr) = &mut self.nodes[g].kind {
            gr.children.push(c as u32);
        }
        // Adding can only extend durations: O(depth) instead of O(children).
        let cn = &self.nodes[c];
        let end = cn.start + cn.timing.end_time() / cn.rate.max(1e-9);
        self.grow_duration(g, end);
        Ok(())
    }

    /// Extend auto durations from group `idx` upwards so they cover `end`
    /// (expressed in `idx`'s local time).
    fn grow_duration(&mut self, mut idx: usize, mut end: f64) {
        loop {
            let n = &mut self.nodes[idx];
            let auto = matches!(&n.kind, Kind::Group(g) if g.auto_duration);
            if !auto || end <= n.timing.duration {
                return;
            }
            n.timing.duration = end;
            if n.parent == NONE {
                return;
            }
            end = n.start + n.timing.end_time() / n.rate.max(1e-9);
            idx = n.parent as usize;
        }
    }

    /// Remove `child` from its group, making it a paused root again.
    pub fn group_remove(&mut self, child: Handle) -> Result<(), Error> {
        let c = self.resolve(child)?;
        let parent = self.nodes[c].parent;
        self.detach(c);
        self.nodes[c].state = PlayState::Paused;
        if parent != NONE {
            self.refresh_duration(parent as usize);
        }
        Ok(())
    }

    fn detach(&mut self, c: usize) {
        let p = self.nodes[c].parent;
        if p == NONE {
            return;
        }
        if let Kind::Group(g) = &mut self.nodes[p as usize].kind {
            g.children.retain(|&x| x as usize != c);
        }
        self.nodes[c].parent = NONE;
    }

    /// Recompute auto durations from `idx` up to the root.
    fn refresh_duration(&mut self, mut idx: usize) {
        loop {
            let n = &self.nodes[idx];
            if let Kind::Group(g) = &n.kind {
                if g.auto_duration {
                    let mut d: f64 = 0.0;
                    for &c in &g.children {
                        let cn = &self.nodes[c as usize];
                        let end = cn.start + cn.timing.end_time() / cn.rate.max(1e-9);
                        if end > d {
                            d = end;
                        }
                    }
                    self.nodes[idx].timing.duration = d;
                }
            }
            let p = self.nodes[idx].parent;
            if p == NONE {
                break;
            }
            idx = p as usize;
        }
    }

    /// Start (or resume) a spring simulation with `channels` channels.
    pub fn create_spring(
        &mut self,
        params: SpringParams,
        pos: &[f64],
        vel: &[f64],
        target: &[f64],
    ) -> Result<Handle, Error> {
        let ch = pos.len();
        if ch == 0 || target.len() != ch || (vel.len() != ch && !vel.is_empty()) {
            return Err(Error::InvalidInput);
        }
        let cfg = self.spring_cfg(params);
        let mut buf = alloc::vec![0.0; ch * 5];
        buf[..ch].copy_from_slice(pos);
        if !vel.is_empty() {
            buf[ch..ch * 2].copy_from_slice(vel);
        }
        buf[ch * 2..ch * 3].copy_from_slice(target);
        for k in 0..ch {
            buf[ch * 3 + k] = f64::NEG_INFINITY;
            buf[ch * 4 + k] = f64::INFINITY;
        }
        let sim = Sim {
            kind: SimKind::Spring,
            ch,
            cfg,
            buf,
            mode: alloc::vec![1; ch],
            time_constant: 0.35,
            rest_delta: 0.005,
            rest_speed: 0.05,
        };
        let idx = self.alloc(Kind::Sim(Box::new(sim)), Timing::default(), flags::DEFAULT)?;
        self.activate_sim(idx);
        Ok(self.handle_of(idx))
    }

    /// Inertia ("decay") simulation: position glides from `pos` with velocity
    /// `vel` (units/s) and exponentially slows down. `target` (if given)
    /// overrides the natural resting point (e.g. snapping). When `min`/`max`
    /// bounds are crossed, the channel switches to `bounce` spring towards the bound.
    #[allow(clippy::too_many_arguments)]
    pub fn create_decay(
        &mut self,
        pos: &[f64],
        vel: &[f64],
        power: f64,
        time_constant_ms: f64,
        target: Option<&[f64]>,
        min: Option<&[f64]>,
        max: Option<&[f64]>,
        bounce: SpringParams,
    ) -> Result<Handle, Error> {
        let ch = pos.len();
        if ch == 0 || vel.len() != ch {
            return Err(Error::InvalidInput);
        }
        let tc = (time_constant_ms / 1000.0).max(1e-3);
        let cfg = self.spring_cfg(bounce);
        let mut buf = alloc::vec![0.0; ch * 5];
        let mut mode = alloc::vec![0u8; ch];
        for k in 0..ch {
            let lo = min.and_then(|m| m.get(k).copied()).unwrap_or(f64::NEG_INFINITY);
            let hi = max.and_then(|m| m.get(k).copied()).unwrap_or(f64::INFINITY);
            let natural = pos[k] + power * vel[k] * tc;
            let tgt = target.and_then(|t| t.get(k).copied()).unwrap_or(natural);
            buf[k] = pos[k];
            buf[ch + k] = vel[k];
            buf[ch * 2 + k] = tgt;
            buf[ch * 3 + k] = lo;
            buf[ch * 4 + k] = hi;
            if pos[k] < lo || pos[k] > hi {
                mode[k] = 1;
                buf[ch * 2 + k] = if pos[k] < lo { lo } else { hi };
            }
        }
        let sim = Sim {
            kind: SimKind::Decay,
            ch,
            cfg,
            buf,
            mode,
            time_constant: tc,
            rest_delta: 0.005,
            rest_speed: 0.05,
        };
        let idx = self.alloc(Kind::Sim(Box::new(sim)), Timing::default(), flags::DEFAULT)?;
        self.activate_sim(idx);
        Ok(self.handle_of(idx))
    }

    fn spring_cfg(&mut self, p: SpringParams) -> u32 {
        if let Some(i) = self.springs.iter().position(|c| c.params == p) {
            return i as u32;
        }
        self.springs.push(SpringCfg { params: p, matrix: [1.0, 0.0, 0.0, 1.0], dt: 0.0 });
        (self.springs.len() - 1) as u32
    }

    fn activate_sim(&mut self, idx: usize) {
        let n = &mut self.nodes[idx];
        n.state = PlayState::Playing;
        if n.active_at == NONE {
            n.active_at = self.active_sims.len() as u32;
            self.active_sims.push(idx as u32);
        }
    }

    // ------------------------------------------------------------------
    // Simulation control
    // ------------------------------------------------------------------

    fn sim_mut(&mut self, h: Handle) -> Result<(usize, &mut Sim), Error> {
        let i = self.resolve(h)?;
        match &mut self.nodes[i].kind {
            Kind::Sim(s) => Ok((i, s)),
            _ => Err(Error::WrongKind),
        }
    }

    /// Retarget a spring, preserving its current position and velocity.
    pub fn sim_set_target(&mut self, h: Handle, target: &[f64]) -> Result<(), Error> {
        let (i, s) = self.sim_mut(h)?;
        if target.len() != s.ch {
            return Err(Error::InvalidInput);
        }
        let ch = s.ch;
        s.buf[ch * 2..ch * 3].copy_from_slice(target);
        s.kind = SimKind::Spring;
        for m in s.mode.iter_mut() {
            *m = 1;
        }
        self.activate_sim(i);
        Ok(())
    }

    /// Jump to a position (keeps velocity unless `vel` is given).
    pub fn sim_set_position(&mut self, h: Handle, pos: &[f64], vel: Option<&[f64]>) -> Result<(), Error> {
        let (i, s) = self.sim_mut(h)?;
        let ch = s.ch;
        if pos.len() != ch || vel.map(|v| v.len() != ch).unwrap_or(false) {
            return Err(Error::InvalidInput);
        }
        s.buf[..ch].copy_from_slice(pos);
        if let Some(v) = vel {
            s.buf[ch..ch * 2].copy_from_slice(v);
        }
        for m in s.mode.iter_mut() {
            if *m == 2 {
                *m = 1;
            }
        }
        self.activate_sim(i);
        self.updated.push(h);
        Ok(())
    }

    pub fn sim_set_params(&mut self, h: Handle, p: SpringParams) -> Result<(), Error> {
        let cfg = self.spring_cfg(p);
        let (_, s) = self.sim_mut(h)?;
        s.cfg = cfg;
        Ok(())
    }

    pub fn sim_set_rest(&mut self, h: Handle, rest_delta: f64, rest_speed: f64) -> Result<(), Error> {
        let (_, s) = self.sim_mut(h)?;
        s.rest_delta = rest_delta.max(0.0);
        s.rest_speed = rest_speed.max(0.0);
        Ok(())
    }

    pub fn sim_velocity(&self, h: Handle) -> Result<&[f64], Error> {
        let i = self.resolve(h)?;
        match &self.nodes[i].kind {
            Kind::Sim(s) => Ok(&s.buf[s.ch..s.ch * 2]),
            _ => Err(Error::WrongKind),
        }
    }

    pub fn sim_target(&self, h: Handle) -> Result<&[f64], Error> {
        let i = self.resolve(h)?;
        match &self.nodes[i].kind {
            Kind::Sim(s) => Ok(&s.buf[s.ch * 2..s.ch * 3]),
            _ => Err(Error::WrongKind),
        }
    }

    // ------------------------------------------------------------------
    // Playback control (root nodes)
    // ------------------------------------------------------------------

    fn root(&self, h: Handle) -> Result<usize, Error> {
        let i = self.resolve(h)?;
        if self.nodes[i].parent != NONE {
            return Err(Error::NotRoot);
        }
        Ok(i)
    }

    fn activate(&mut self, idx: usize) {
        if self.nodes[idx].active_at == NONE {
            self.nodes[idx].active_at = self.active.len() as u32;
            self.active.push(idx as u32);
        }
    }

    fn deactivate(&mut self, idx: usize) {
        let at = self.nodes[idx].active_at;
        if at == NONE {
            return;
        }
        let list = if matches!(self.nodes[idx].kind, Kind::Sim(_)) { &mut self.active_sims } else { &mut self.active };
        let at = at as usize;
        list.swap_remove(at);
        if at < list.len() {
            let moved = list[at] as usize;
            self.nodes[moved].active_at = at as u32;
        }
        self.nodes[idx].active_at = NONE;
    }

    pub fn play(&mut self, h: Handle) -> Result<(), Error> {
        let i = self.root(h)?;
        if let Kind::Sim(_) = self.nodes[i].kind {
            self.activate_sim(i);
            return Ok(());
        }
        let n = &self.nodes[i];
        let end = n.timing.end_time();
        if n.rate >= 0.0 && n.time >= end && end.is_finite() {
            if n.state == PlayState::Finished {
                self.rewind(i);
            } else {
                self.nodes[i].time = 0.0;
            }
        } else if n.rate < 0.0 && n.time <= 0.0 {
            self.nodes[i].time = end;
        }
        self.nodes[i].state = PlayState::Playing;
        self.activate(i);
        Ok(())
    }

    /// Silently return a subtree to its start: values are rendered (so the
    /// targets reset) but no events are emitted, and every node ends up in the
    /// "before" phase so START fires again on the next forward tick.
    fn rewind(&mut self, i: usize) {
        let ev = self.events.len();
        self.render(i, 0.0, true);
        self.events.truncate(ev);
    }

    pub fn pause(&mut self, h: Handle) -> Result<(), Error> {
        let i = self.root(h)?;
        self.deactivate(i);
        self.nodes[i].state = PlayState::Paused;
        Ok(())
    }

    /// Jump to local time `t` (ms) and render immediately.
    pub fn seek(&mut self, h: Handle, t: f64) -> Result<(), Error> {
        let i = self.root(h)?;
        if matches!(self.nodes[i].kind, Kind::Sim(_)) {
            return Err(Error::WrongKind);
        }
        let end = self.nodes[i].timing.end_time();
        let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
        let t = if end.is_finite() { t.min(end) } else { t };
        let backward = t < self.nodes[i].time;
        self.render(i, t, backward);
        if self.nodes[i].state == PlayState::Finished && t < end {
            self.nodes[i].state = PlayState::Paused;
        }
        Ok(())
    }

    pub fn set_progress(&mut self, h: Handle, p: f64) -> Result<(), Error> {
        let i = self.root(h)?;
        let end = self.nodes[i].timing.end_time();
        if !end.is_finite() {
            return Err(Error::InvalidInput);
        }
        self.seek(h, p * end)
    }

    /// Playback rate (negative plays backwards). Also valid on group children.
    pub fn set_rate(&mut self, h: Handle, rate: f64) -> Result<(), Error> {
        let i = self.resolve(h)?;
        let n = &mut self.nodes[i];
        if !rate.is_finite() {
            return Err(Error::InvalidInput);
        }
        if n.parent != NONE {
            n.rate = rate.abs().max(1e-9);
            let p = n.parent as usize;
            self.refresh_duration(p);
        } else {
            n.rate = rate;
        }
        Ok(())
    }

    pub fn reverse(&mut self, h: Handle) -> Result<(), Error> {
        let i = self.root(h)?;
        self.nodes[i].rate = -self.nodes[i].rate;
        if self.nodes[i].state != PlayState::Paused {
            self.play(h)?;
        }
        Ok(())
    }

    pub fn restart(&mut self, h: Handle) -> Result<(), Error> {
        let i = self.root(h)?;
        let back = self.nodes[i].rate < 0.0;
        let end = self.nodes[i].timing.end_time();
        if back {
            self.render(i, end, false);
        } else {
            self.rewind(i);
        }
        self.nodes[i].state = PlayState::Playing;
        self.activate(i);
        Ok(())
    }

    /// Change timing (duration/delay/iterations/...) of any node.
    pub fn set_timing(&mut self, h: Handle, timing: Timing) -> Result<(), Error> {
        let i = self.resolve(h)?;
        let auto = matches!(&self.nodes[i].kind, Kind::Group(g) if g.auto_duration);
        let mut t = sanitize(timing);
        if auto {
            t.duration = self.nodes[i].timing.duration;
        }
        self.nodes[i].timing = t;
        let p = self.nodes[i].parent;
        if p != NONE {
            self.refresh_duration(p as usize);
        }
        Ok(())
    }

    pub fn set_start(&mut self, h: Handle, start: f64) -> Result<(), Error> {
        let i = self.resolve(h)?;
        self.nodes[i].start = start;
        let p = self.nodes[i].parent;
        if p != NONE {
            self.refresh_duration(p as usize);
        }
        Ok(())
    }

    /// Enable/disable a single track (e.g. when another animation takes over
    /// the property). Disabled tracks cost nothing.
    pub fn set_track_enabled(&mut self, h: Handle, track: usize, enabled: bool) -> Result<(), Error> {
        let i = self.resolve(h)?;
        match &mut self.nodes[i].kind {
            Kind::Anim(a) => match a.tracks.get_mut(track) {
                Some(t) => {
                    t.enabled = enabled;
                    Ok(())
                }
                None => Err(Error::InvalidInput),
            },
            _ => Err(Error::WrongKind),
        }
    }

    /// Free a node (and, for groups, all descendants).
    pub fn release(&mut self, h: Handle) -> Result<(), Error> {
        let i = self.resolve(h)?;
        let parent = self.nodes[i].parent;
        self.detach(i);
        self.release_rec(i);
        if parent != NONE {
            self.refresh_duration(parent as usize);
        }
        Ok(())
    }

    fn release_rec(&mut self, i: usize) {
        if let Kind::Group(g) = &mut self.nodes[i].kind {
            let children = core::mem::take(&mut g.children);
            for c in children {
                self.nodes[c as usize].parent = NONE;
                self.release_rec(c as usize);
            }
        }
        self.deactivate(i);
        let n = &mut self.nodes[i];
        n.kind = Kind::Free;
        n.gen = (n.gen + 1) & GEN_MASK;
        if n.gen == 0 {
            n.gen = 1;
        }
        self.free.push(i as u32);
    }

    /// Release every node and reset the clock (easings stay registered).
    pub fn clear(&mut self) {
        for i in 0..self.nodes.len() {
            if !matches!(self.nodes[i].kind, Kind::Free) {
                self.nodes[i].parent = NONE;
                if let Kind::Group(g) = &mut self.nodes[i].kind {
                    g.children.clear();
                }
                self.release_rec(i);
            }
        }
        self.active.clear();
        self.active_sims.clear();
        self.updated.clear();
        self.events.clear();
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Output values of an animation (track outputs concatenated) or the
    /// current position of a simulation.
    pub fn values(&self, h: Handle) -> Result<&[f64], Error> {
        let i = self.resolve(h)?;
        match &self.nodes[i].kind {
            Kind::Anim(a) => Ok(a.out()),
            Kind::Sim(s) => Ok(&s.buf[..s.ch]),
            _ => Err(Error::WrongKind),
        }
    }

    /// Raw pointer to the output buffer (stable for the node's lifetime).
    pub fn values_ptr(&self, h: Handle) -> *const f64 {
        self.values(h).map(|v| v.as_ptr()).unwrap_or(core::ptr::null())
    }

    /// Output offset of each track within [`values`](Self::values).
    pub fn track_offsets(&self, h: Handle) -> Result<Vec<u32>, Error> {
        let i = self.resolve(h)?;
        match &self.nodes[i].kind {
            Kind::Anim(a) => Ok(a.tracks.iter().map(|t| t.out_at).collect()),
            _ => Err(Error::WrongKind),
        }
    }

    pub fn time(&self, h: Handle) -> Result<f64, Error> {
        Ok(self.nodes[self.resolve(h)?].time)
    }
    /// Total duration (delay + iterations + end delay), ms.
    pub fn duration(&self, h: Handle) -> Result<f64, Error> {
        Ok(self.nodes[self.resolve(h)?].timing.end_time())
    }
    pub fn timing(&self, h: Handle) -> Result<Timing, Error> {
        Ok(self.nodes[self.resolve(h)?].timing)
    }
    pub fn progress(&self, h: Handle) -> Result<f64, Error> {
        let n = &self.nodes[self.resolve(h)?];
        let end = n.timing.end_time();
        Ok(if end > 0.0 && end.is_finite() { (n.time / end).clamp(0.0, 1.0) } else { 0.0 })
    }
    pub fn state(&self, h: Handle) -> Result<PlayState, Error> {
        Ok(self.nodes[self.resolve(h)?].state)
    }
    pub fn iteration(&self, h: Handle) -> Result<f64, Error> {
        Ok(self.nodes[self.resolve(h)?].last_iter)
    }
    pub fn rate_of(&self, h: Handle) -> Result<f64, Error> {
        Ok(self.nodes[self.resolve(h)?].rate)
    }
    pub fn start_of(&self, h: Handle) -> Result<f64, Error> {
        Ok(self.nodes[self.resolve(h)?].start)
    }
    pub fn parent_of(&self, h: Handle) -> Result<Handle, Error> {
        let p = self.nodes[self.resolve(h)?].parent;
        Ok(if p == NONE { NONE } else { self.handle_of(p as usize) })
    }

    /// Evaluate an animation at an arbitrary *raw* iteration progress `p`
    /// (timing easing applied) into `out`, without touching playback state.
    pub fn sample(&mut self, h: Handle, p: f64, out: &mut [f64]) -> Result<(), Error> {
        let i = self.resolve(h)?;
        let te = self.nodes[i].timing.easing;
        let pe = ease_by(&self.easings, te, p);
        match &mut self.nodes[i].kind {
            Kind::Anim(a) => {
                let n = a.buf.len() - a.out_at;
                if out.len() < n {
                    return Err(Error::InvalidInput);
                }
                let data = &a.buf[..a.out_at];
                Anim::eval_into(&mut a.tracks, data, &self.easings, pe, &mut out[..n]);
                Ok(())
            }
            _ => Err(Error::WrongKind),
        }
    }

    /// Handles whose outputs changed since the last [`tick`](Self::tick) began.
    pub fn updated(&self) -> &[Handle] {
        &self.updated
    }
    /// Flat `[handle, kind, handle, kind, ...]` event list (see [`event`]).
    pub fn events(&self) -> &[u32] {
        &self.events
    }
    pub fn clear_frame(&mut self) {
        self.updated.clear();
        self.events.clear();
    }
    pub fn active_count(&self) -> usize {
        self.active.len() + self.active_sims.len()
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len() - self.free.len()
    }

    /// `threshold` ms: frame gaps longer than this (tab switches, debugger
    /// pauses) are treated as `adjusted` ms so animations do not jump.
    /// Pass `threshold <= 0` to disable.
    pub fn set_lag_smoothing(&mut self, threshold: f64, adjusted: f64) {
        self.lag_threshold = if threshold <= 0.0 { f64::INFINITY } else { threshold };
        self.lag_adjusted = adjusted.max(0.0);
    }

    // ------------------------------------------------------------------
    // The frame
    // ------------------------------------------------------------------

    /// Advance the world by `dt` milliseconds.
    pub fn tick(&mut self, dt: f64) {
        self.updated.clear();
        self.events.clear();
        let mut dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        if dt > self.lag_threshold {
            dt = self.lag_adjusted;
        }
        let dt = dt * self.rate;
        self.now += dt;

        let mut k = 0;
        while k < self.active.len() {
            let i = self.active[k] as usize;
            let (nt, backward, end) = {
                let n = &self.nodes[i];
                (n.time + dt * n.rate, n.rate < 0.0, n.timing.end_time())
            };
            let done = if backward { nt <= 0.0 } else { nt >= end };
            let t = if done {
                if backward {
                    0.0
                } else {
                    end
                }
            } else {
                nt
            };
            self.render(i, t, backward);
            if done {
                self.finish(i);
                // `finish` swap-removed index k; re-examine the same slot.
            } else {
                k += 1;
            }
        }
        if !self.active_sims.is_empty() {
            self.step_sims(dt / 1000.0);
        }
        if !self.released_scratch.is_empty() {
            let list = core::mem::take(&mut self.released_scratch);
            for &i in &list {
                let h = self.handle_of(i as usize);
                let _ = self.release(h);
            }
            self.released_scratch = list;
            self.released_scratch.clear();
        }
    }

    fn finish(&mut self, i: usize) {
        self.deactivate(i);
        self.nodes[i].state = PlayState::Finished;
        let h = self.handle_of(i);
        self.events.push(h);
        self.events.push(event::FINISH);
        if self.nodes[i].flags & flags::AUTO_RELEASE != 0 {
            self.released_scratch.push(i as u32);
        }
    }

    /// Phase-transition events (cold path).
    #[cold]
    #[inline(never)]
    fn emit_transition(&mut self, h: Handle, prev: u8, phase: Phase, changed: bool) {
        if !changed {
            self.events.extend_from_slice(&[h, event::REPEAT]);
            return;
        }
        let from_before = prev == Phase::Before as u8 || prev == PHASE_UNKNOWN;
        match phase {
            Phase::Active => {
                if from_before {
                    self.events.extend_from_slice(&[h, event::START]);
                }
            }
            Phase::After => {
                if from_before {
                    self.events.extend_from_slice(&[h, event::START]);
                }
                self.events.extend_from_slice(&[h, event::COMPLETE]);
            }
            Phase::Before => {
                if prev != PHASE_UNKNOWN {
                    self.events.extend_from_slice(&[h, event::REVERSE_COMPLETE]);
                }
            }
        }
    }

    /// Render node `i` at local time `t`.
    #[inline]
    fn render(&mut self, i: usize, t: f64, backward: bool) {
        let n = &mut self.nodes[i];
        n.time = t;
        let mut st = n.timing.eval(t);
        // Direction-aware boundary: moving backward, the exact start belongs to "before".
        if backward && st.phase == Phase::Active && t <= n.timing.delay && n.timing.active_duration() > 0.0 {
            st.phase = Phase::Before;
        }
        let phase = st.phase as u8;
        let prev = n.last_phase;
        let changed = phase != prev;
        // Steady before/after the active phase: nothing to render or report.
        // (This is what makes long timelines cheap: idle children cost one eval.)
        if !changed && st.phase != Phase::Active {
            return;
        }
        let h = (n.gen << INDEX_BITS) | i as u32;
        let repeat = !changed && st.iteration != n.last_iter;
        n.last_phase = phase;
        n.last_iter = st.iteration;
        let fl = n.flags;
        let te = n.timing.easing;
        let is_cb = matches!(n.kind, Kind::Callback);
        if (changed || repeat) && !is_cb {
            self.emit_transition(h, prev, st.phase, changed);
        }
        let should_render = match st.phase {
            Phase::Active => true,
            Phase::After => fl & flags::FILL_FORWARDS != 0,
            Phase::Before => fl & flags::FILL_BACKWARDS != 0,
        };
        match &mut self.nodes[i].kind {
            Kind::Anim(a) => {
                if should_render {
                    let p = if te == LINEAR { st.progress } else { ease_by(&self.easings, te, st.progress) };
                    a.eval(&self.easings, p);
                    self.updated.push(h);
                }
            }
            Kind::Group(_) => {
                let (duration, delay) = {
                    let n = &self.nodes[i];
                    (n.timing.duration, n.timing.delay)
                };
                let inner = if duration == f64::INFINITY {
                    (t - delay).max(0.0)
                } else {
                    ease_by(&self.easings, te, st.progress) * duration
                };
                let (children, child_backward) = match &mut self.nodes[i].kind {
                    Kind::Group(g) => {
                        let back = inner < g.last_inner || (inner == g.last_inner && backward);
                        g.last_inner = inner;
                        (core::mem::take(&mut g.children), back)
                    }
                    _ => return,
                };
                if child_backward {
                    for &c in children.iter().rev() {
                        let cn = &self.nodes[c as usize];
                        let ct = (inner - cn.start) * cn.rate;
                        self.render(c as usize, ct, true);
                    }
                } else {
                    for &c in children.iter() {
                        let cn = &self.nodes[c as usize];
                        let ct = (inner - cn.start) * cn.rate;
                        self.render(c as usize, ct, false);
                    }
                }
                if let Kind::Group(g) = &mut self.nodes[i].kind {
                    g.children = children;
                }
            }
            Kind::Callback => {
                if changed && prev != PHASE_UNKNOWN {
                    self.events.extend_from_slice(&[h, event::CALLBACK]);
                }
            }
            Kind::Batch(b) => {
                if should_render {
                    b.eval(&self.easings, t);
                    self.updated.push(h);
                }
            }
            Kind::Sim(_) | Kind::Free => {}
        }
    }

    fn step_sims(&mut self, dt: f64) {
        if dt <= 0.0 {
            return;
        }
        // One matrix per spring configuration per frame.
        for c in self.springs.iter_mut() {
            if c.dt != dt {
                c.matrix = c.params.matrix(dt);
                c.dt = dt;
            }
        }
        let mut k = 0;
        while k < self.active_sims.len() {
            let i = self.active_sims[k] as usize;
            let h = self.handle_of(i);
            let at_rest = match &mut self.nodes[i].kind {
                Kind::Sim(s) => step_sim(s, &self.springs[s.cfg as usize].matrix, dt),
                _ => true,
            };
            self.updated.push(h);
            if at_rest {
                self.deactivate(i);
                self.nodes[i].state = PlayState::Finished;
                self.events.extend_from_slice(&[h, event::FINISH]);
                if self.nodes[i].flags & flags::AUTO_RELEASE != 0 {
                    self.released_scratch.push(i as u32);
                }
            } else {
                k += 1;
            }
        }
    }
}

/// Advance one simulation; returns true when every channel is at rest.
#[inline]
fn step_sim(s: &mut Sim, m: &[f64; 4], dt: f64) -> bool {
    let ch = s.ch;
    let (pos, rest) = s.buf.split_at_mut(ch);
    let (vel, rest) = rest.split_at_mut(ch);
    let (target, bounds) = rest.split_at_mut(ch);
    let (min, max) = bounds.split_at(ch);
    let mut all_rest = true;
    let decay_k = if s.kind == SimKind::Decay { exp(-dt / s.time_constant) } else { 1.0 };
    for k in 0..ch {
        match s.mode[k] {
            0 => {
                // Exponential decay towards target, exact for any dt.
                let d = (pos[k] - target[k]) * decay_k;
                let np = target[k] + d;
                vel[k] = -d / s.time_constant;
                if np < min[k] || np > max[k] {
                    // Crossed a bound: hand over to the bounce spring.
                    let b = if np < min[k] { min[k] } else { max[k] };
                    pos[k] = np;
                    target[k] = b;
                    s.mode[k] = 1;
                    all_rest = false;
                } else if abs(d) < s.rest_delta {
                    pos[k] = target[k];
                    vel[k] = 0.0;
                    s.mode[k] = 2;
                } else {
                    pos[k] = np;
                    all_rest = false;
                }
            }
            1 => {
                let x = pos[k] - target[k];
                let v = vel[k];
                let nx = m[0] * x + m[1] * v;
                let nv = m[2] * x + m[3] * v;
                if abs(nx) < s.rest_delta && abs(nv) < s.rest_speed {
                    pos[k] = target[k];
                    vel[k] = 0.0;
                    s.mode[k] = 2;
                } else {
                    pos[k] = target[k] + nx;
                    vel[k] = nv;
                    all_rest = false;
                }
            }
            _ => {}
        }
    }
    all_rest
}

fn sanitize(mut t: Timing) -> Timing {
    let fix = |v: f64, d: f64| if v.is_nan() { d } else { v };
    t.delay = fix(t.delay, 0.0);
    t.end_delay = fix(t.end_delay, 0.0);
    t.duration = fix(t.duration, 0.0).max(0.0);
    t.iterations = fix(t.iterations, 1.0).max(0.0);
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn tween(e: &mut Engine, from: f64, to: f64, dur: f64) -> Handle {
        e.create_anim(Timing { duration: dur, ..Default::default() }, &[TrackSpec::number(vec![from, to], 1)], flags::DEFAULT)
            .unwrap()
    }

    fn has_event(e: &Engine, h: Handle, kind: u32) -> bool {
        e.events().chunks(2).any(|c| c[0] == h && c[1] == kind)
    }

    #[test]
    fn basic_tween_and_events() {
        let mut e = Engine::new();
        let h = tween(&mut e, 0.0, 100.0, 1000.0);
        e.play(h).unwrap();
        e.tick(250.0);
        assert!((e.values(h).unwrap()[0] - 25.0).abs() < 1e-9);
        assert!(has_event(&e, h, event::START));
        assert_eq!(e.updated(), &[h]);
        e.tick(400.0);
        e.tick(400.0);
        assert_eq!(e.values(h).unwrap()[0], 100.0);
        assert!(has_event(&e, h, event::COMPLETE));
        assert!(has_event(&e, h, event::FINISH));
        assert_eq!(e.state(h).unwrap(), PlayState::Finished);
        assert_eq!(e.active_count(), 0);
        // Finished nodes cost nothing.
        e.tick(16.0);
        assert!(e.updated().is_empty());
    }

    #[test]
    fn keyframes_offsets_easings() {
        let mut e = Engine::new();
        let ein = e.easing("inQuad").unwrap();
        let spec = TrackSpec::number(vec![0.0, 10.0, 0.0], 1)
            .with_offsets(vec![0.0, 0.25, 1.0])
            .with_easings(vec![ein, LINEAR]);
        let h = e.create_anim(Timing { duration: 100.0, ..Default::default() }, &[spec], flags::DEFAULT).unwrap();
        e.play(h).unwrap();
        e.tick(12.5);
        assert!((e.values(h).unwrap()[0] - 2.5).abs() < 1e-9); // inQuad(0.5)=0.25 -> 2.5
        e.tick(50.0); // t = 62.5 → second segment halfway
        assert!((e.values(h).unwrap()[0] - 5.0).abs() < 1e-9);
        // Backwards seek uses the cursor correctly.
        e.seek(h, 10.0).unwrap();
        assert!((e.values(h).unwrap()[0] - 0.4 * 0.4 * 10.0).abs() < 1e-9);
    }

    #[test]
    fn colors_paths_discrete() {
        let mut e = Engine::new();
        let mut c = TrackSpec::number(vec![255.0, 0.0, 0.0, 1.0, 0.0, 0.0, 255.0, 1.0], 4);
        c.kind = TrackKind::Color(ColorSpace::Oklab);
        let p = crate::path::Path::parse("M0 0 L100 0");
        let mut pt = TrackSpec::number(vec![0.0, 1.0], 1);
        pt.kind = TrackKind::Path;
        pt.path = Some(PathLut::new(&p, 101));
        let mut d = TrackSpec::number(vec![0.0, 1.0], 1);
        d.kind = TrackKind::Discrete;
        let h = e.create_anim(Timing { duration: 100.0, ..Default::default() }, &[c, pt, d], flags::DEFAULT).unwrap();
        assert_eq!(e.track_offsets(h).unwrap(), vec![0, 4, 7]);
        assert_eq!(e.values(h).unwrap()[0], 255.0); // prefilled start value
        e.play(h).unwrap();
        e.tick(50.0);
        let v = e.values(h).unwrap();
        assert!(v[0] > 0.0 && v[2] > 0.0 && v[3] == 1.0);
        assert!((v[4] - 50.0).abs() < 1e-6 && v[5].abs() < 1e-9 && v[6].abs() < 1e-9);
        assert_eq!(v[7], 1.0);
    }

    #[test]
    fn timeline_sequencing_forward_and_backward() {
        let mut e = Engine::new();
        let g = e.create_group(Timing::default(), true, flags::DEFAULT).unwrap();
        // Two sequential tweens on the "same property" sharing a slot model.
        let a = tween(&mut e, 0.0, 100.0, 1000.0);
        let b = tween(&mut e, 100.0, 200.0, 1000.0);
        e.group_add(g, a, 0.0).unwrap();
        e.group_add(g, b, 1000.0).unwrap();
        assert_eq!(e.duration(g).unwrap(), 2000.0);
        // Simulate a shared property: last writer in render order wins.
        let read = |e: &Engine| {
            let mut v = None;
            for &u in e.updated() {
                v = Some(e.values(u).unwrap()[0]);
            }
            v
        };
        e.seek(g, 1500.0).unwrap();
        assert_eq!(read(&e), Some(150.0));
        e.clear_frame();
        e.seek(g, 500.0).unwrap();
        assert_eq!(read(&e), Some(50.0)); // b renders its start first, a wins
        e.clear_frame();
        e.seek(g, 2000.0).unwrap();
        assert_eq!(read(&e), Some(200.0));
        e.clear_frame();
        e.seek(g, 0.0).unwrap();
        assert_eq!(read(&e), Some(0.0));
    }

    #[test]
    fn nested_repeat_yoyo_and_callbacks() {
        let mut e = Engine::new();
        let g = e
            .create_group(
                Timing { iterations: 2.0, direction: crate::timing::Direction::Alternate, ..Default::default() },
                true,
                flags::DEFAULT,
            )
            .unwrap();
        let a = tween(&mut e, 0.0, 10.0, 100.0);
        let cb = e.create_callback().unwrap();
        e.group_add(g, a, 0.0).unwrap();
        e.group_add(g, cb, 50.0).unwrap();
        e.play(g).unwrap();
        e.tick(60.0);
        assert!(has_event(&e, cb, event::CALLBACK));
        e.tick(80.0); // t=140 → second (reversed) iteration, inner = 60
        assert!(has_event(&e, g, event::REPEAT));
        assert!((e.values(a).unwrap()[0] - 6.0).abs() < 1e-9);
        e.tick(20.0); // inner 40 → crossed callback backwards
        assert!(has_event(&e, cb, event::CALLBACK));
        e.tick(100.0);
        assert_eq!(e.values(a).unwrap()[0], 0.0);
        assert!(has_event(&e, g, event::FINISH));
    }

    #[test]
    fn reverse_and_restart() {
        let mut e = Engine::new();
        let h = tween(&mut e, 0.0, 100.0, 100.0);
        e.play(h).unwrap();
        e.tick(200.0);
        assert_eq!(e.state(h).unwrap(), PlayState::Finished);
        e.reverse(h).unwrap();
        e.tick(25.0);
        assert!((e.values(h).unwrap()[0] - 75.0).abs() < 1e-9);
        e.tick(100.0);
        assert_eq!(e.values(h).unwrap()[0], 0.0);
        assert!(has_event(&e, h, event::REVERSE_COMPLETE));
        e.reverse(h).unwrap();
        e.restart(h).unwrap();
        e.tick(50.0);
        assert!((e.values(h).unwrap()[0] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn handles_are_generational() {
        let mut e = Engine::new();
        let h = tween(&mut e, 0.0, 1.0, 10.0);
        e.release(h).unwrap();
        assert_eq!(e.play(h), Err(Error::BadHandle));
        let h2 = tween(&mut e, 0.0, 1.0, 10.0);
        assert_eq!(Engine::handle_index(h), Engine::handle_index(h2));
        assert_ne!(h, h2);
        assert!(e.is_alive(h2) && !e.is_alive(h));
    }

    #[test]
    fn auto_release() {
        let mut e = Engine::new();
        let h = e
            .create_anim(
                Timing { duration: 10.0, ..Default::default() },
                &[TrackSpec::number(vec![0.0, 1.0], 1)],
                flags::DEFAULT | flags::AUTO_RELEASE,
            )
            .unwrap();
        e.play(h).unwrap();
        e.tick(20.0);
        assert!(has_event(&e, h, event::FINISH));
        assert!(!e.is_alive(h));
        assert_eq!(e.node_count(), 0);
    }

    #[test]
    fn springs_settle_and_retarget() {
        let mut e = Engine::new();
        let s = e.create_spring(SpringParams::new(170.0, 26.0, 1.0), &[0.0, 0.0], &[], &[100.0, -50.0]).unwrap();
        for _ in 0..30 {
            e.tick(16.0);
        }
        let v = e.values(s).unwrap();
        assert!(v[0] > 90.0 && v[1] < -45.0);
        e.sim_set_target(s, &[0.0, 0.0]).unwrap();
        let mut finished = false;
        for _ in 0..400 {
            e.tick(16.0);
            if has_event(&e, s, event::FINISH) {
                finished = true;
                break;
            }
        }
        assert!(finished);
        assert_eq!(e.values(s).unwrap(), &[0.0, 0.0]);
        // Large dt stays stable (closed form).
        e.sim_set_target(s, &[1000.0, 1000.0]).unwrap();
        e.tick(400.0);
        assert!(e.values(s).unwrap()[0].is_finite());
    }

    #[test]
    fn decay_with_bounds() {
        let mut e = Engine::new();
        let d = e
            .create_decay(&[0.0], &[1000.0], 0.8, 350.0, None, None, Some(&[100.0]), SpringParams::new(500.0, 40.0, 1.0))
            .unwrap();
        let mut maxv: f64 = 0.0;
        for _ in 0..300 {
            e.tick(16.0);
            maxv = maxv.max(e.values(d).unwrap()[0]);
        }
        assert!(maxv > 100.0 && maxv < 200.0, "{maxv}");
        assert_eq!(e.values(d).unwrap()[0], 100.0);
        assert_eq!(e.state(d).unwrap(), PlayState::Finished);
        // Without bounds it settles at pos + power * v * tc = 280.
        let d2 = e
            .create_decay(&[0.0], &[1000.0], 0.8, 350.0, None, None, None, SpringParams::default())
            .unwrap();
        for _ in 0..300 {
            e.tick(16.0);
        }
        assert_eq!(e.values(d2).unwrap()[0], 280.0);
    }

    #[test]
    fn lag_smoothing() {
        let mut e = Engine::new();
        let h = tween(&mut e, 0.0, 1000.0, 1000.0);
        e.play(h).unwrap();
        e.tick(5000.0);
        assert!((e.values(h).unwrap()[0] - 33.0).abs() < 1e-9);
        e.set_lag_smoothing(0.0, 0.0);
        e.tick(5000.0);
        assert_eq!(e.values(h).unwrap()[0], 1000.0);
    }

    #[test]
    fn delay_fill_and_immediate() {
        let mut e = Engine::new();
        let spec = [TrackSpec::number(vec![50.0, 100.0], 1)];
        let t = Timing { delay: 100.0, duration: 100.0, ..Default::default() };
        let lazy = e.create_anim(t, &spec, flags::DEFAULT).unwrap();
        let eager = e.create_anim(t, &spec, flags::DEFAULT | flags::IMMEDIATE).unwrap();
        e.play(lazy).unwrap();
        e.play(eager).unwrap();
        e.tick(10.0);
        assert_eq!(e.updated(), &[eager]);
        e.tick(140.0);
        assert!((e.values(lazy).unwrap()[0] - 75.0).abs() < 1e-9);
    }

    #[test]
    fn start_fires_again_on_replay() {
        let mut e = Engine::new();
        let g = e.create_group(Timing::default(), true, flags::DEFAULT).unwrap();
        let a = tween(&mut e, 0.0, 10.0, 100.0);
        e.group_add(g, a, 0.0).unwrap();
        e.play(g).unwrap();
        e.tick(50.0);
        assert!(has_event(&e, g, event::START) && has_event(&e, a, event::START));
        e.tick(100.0);
        assert!(has_event(&e, g, event::FINISH));
        e.play(g).unwrap(); // replay from the end
        assert!(!has_event(&e, g, event::REVERSE_COMPLETE));
        e.tick(10.0);
        assert!(has_event(&e, g, event::START) && has_event(&e, a, event::START));
        assert!((e.values(a).unwrap()[0] - 1.0).abs() < 1e-9);
        e.restart(g).unwrap();
        e.tick(10.0);
        assert!(has_event(&e, a, event::START));
    }

    #[test]
    fn batches() {
        let mut e = Engine::new();
        let ease = e.easing("outQuad").unwrap();
        let spec = BatchSpec {
            count: 3,
            channels: 2,
            offsets: vec![],
            easings: vec![ease],
            values: vec![0.0, 0.0, 10.0, 20.0, 0.0, 0.0, 10.0, 20.0, 0.0, 0.0, 10.0, 20.0],
            delays: vec![0.0, 50.0, 100.0],
        };
        let h = e.create_batch(Timing { duration: 100.0, ..Default::default() }, spec, flags::DEFAULT).unwrap();
        assert_eq!(e.duration(h).unwrap(), 200.0);
        e.play(h).unwrap();
        e.tick(50.0);
        let v = e.batch_values(h).unwrap();
        assert!((v[0] - 7.5).abs() < 1e-5 && (v[1] - 15.0).abs() < 1e-5);
        assert_eq!(&v[2..], &[0.0, 0.0, 0.0, 0.0]);
        // Keyframes + yoyo
        let spec = BatchSpec {
            count: 1,
            channels: 1,
            offsets: vec![0.0, 0.5, 1.0],
            easings: vec![],
            values: vec![0.0, 10.0, 0.0],
            delays: vec![],
        };
        let k = e.create_batch(Timing { duration: 100.0, ..Default::default() }, spec, flags::DEFAULT).unwrap();
        e.seek(k, 25.0).unwrap();
        assert!((e.batch_values(k).unwrap()[0] - 5.0).abs() < 1e-5);
        e.tick(200.0);
        assert!(e.events().chunks(2).any(|c| c[0] == h && c[1] == event::FINISH));
        assert_eq!(e.batch_values(h).unwrap()[4], 10.0);
    }

    #[test]
    fn many_anims_stress() {
        let mut e = Engine::new();
        let ease = e.easing("outCubic").unwrap();
        let mut hs = Vec::new();
        for i in 0..10_000 {
            let spec = TrackSpec::number(vec![0.0, 0.0, i as f64, 1.0], 2).with_easings(vec![ease]);
            let h = e
                .create_anim(Timing { duration: 500.0 + i as f64 % 300.0, ..Default::default() }, &[spec], flags::DEFAULT)
                .unwrap();
            e.play(h).unwrap();
            hs.push(h);
        }
        for _ in 0..100 {
            e.tick(16.0);
        }
        assert_eq!(e.active_count(), 0);
        for (i, h) in hs.iter().enumerate() {
            assert_eq!(e.values(*h).unwrap(), &[i as f64, 1.0]);
        }
    }
}
