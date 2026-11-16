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
use crate::math::{abs, exp, sqrt};
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
    /// 3D transform: keyframes are 4×4 matrices (16 values, CSS `matrix3d`
    /// order); interpolated like browsers do (decompose → lerp + quaternion
    /// slerp → recompose); outputs 16 values.
    Matrix,
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
    /// Fast path: a numeric track of one channel with 3+ keyframes, or 2
    /// keyframes not at offsets 0 and 1.
    num1: bool,
    /// Offsets generated by the engine (`k / (n - 1)`, strictly increasing),
    /// not given by the caller.
    even: bool,
}

/// A keyframe animation. Everything it needs per frame lives in two
/// allocations: the track table and one `f64` buffer laid out as
/// `[offsets/values/LUTs/easing ids…, outputs…]`.
#[derive(Debug)]
struct Anim {
    tracks: Box<[Track]>,
    buf: Box<[f64]>,
    out_at: usize,
    /// When every track is an enabled single-segment number of one channel
    /// with the same easing (`{ x: 500, y: 300, opacity: 0 }`), the number of
    /// tracks, else 0. Their data then sits at fixed places (track `k`: values
    /// at `4k + 2`, output `k`), so [`eval`](Self::eval) skips the track table.
    simple: u32,
    /// The shared easing of a `simple` animation, or the segment easing of
    /// a `kf` one.
    simple_ease: u32,
    /// When the animation is one enabled keyframed number (`num1`) with at
    /// most one easing for all segments, its keyframe count, else 0. Its data
    /// then sits at fixed places (offsets at 0, values at `kf`, output 0), so
    /// [`eval`](Self::eval) skips the track table.
    kf: u32,
    /// The segment cursor of a `kf` animation (the track's own `cursor` is
    /// only current when `kf` is 0; see [`classify`](Self::classify)).
    kf_cursor: u32,
    /// A `kf` animation's offsets were generated by the engine (see
    /// [`EaseMemo`]'s keyframe segment entry).
    kf_even: bool,
}

impl Anim {
    #[inline]
    fn out(&self) -> &[f64] {
        &self.buf[self.out_at..]
    }
    #[inline(always)]
    fn eval(&mut self, easings: &[Easing], memo: &mut EaseMemo, p: f64) {
        let (data, out) = self.buf.split_at_mut(self.out_at);
        let n = self.simple as usize;
        if n > 0 {
            // The single-segment path of `eval_into` for every track, without
            // reading the tracks (same easing call, same lerp).
            let e = ease_memo(easings, memo, self.simple_ease, p);
            if let (Some(d), Some(o)) = (data.get(..4 * n), out.get_mut(..n)) {
                for (o, d) in o.iter_mut().zip(d.chunks_exact(4)) {
                    *o = d[2] + (d[3] - d[2]) * e;
                }
            }
            return;
        }
        let k = self.kf as usize;
        if k > 0 {
            // The `num1` path of `eval_into` for the only track, without
            // reading it (same search, same local progress, same easing call,
            // same lerp). Evenly spaced offsets make the segment and its eased
            // progress a function of `(p, k, easing)`, shared by every target
            // of one `animate()` call: the memo keeps the last one.
            if let (Some(off), Some(vals), Some(o)) = (data.get(..k), data.get(k..2 * k), out.first_mut()) {
                let id = self.simple_ease;
                let bits = p.to_bits();
                let (i, e) = if self.kf_even && memo.kf_n == self.kf && memo.kf_bits == bits && memo.kf_ease == id {
                    (memo.kf_i as usize, memo.kf_e)
                } else {
                    let mut i = (self.kf_cursor as usize).min(k - 2);
                    while i > 0 && p < off[i] {
                        i -= 1;
                    }
                    while i < k - 2 && p >= off[i + 1] {
                        i += 1;
                    }
                    let span = off[i + 1] - off[i];
                    let local = if span > 0.0 {
                        (p - off[i]) / span
                    } else if p >= off[i + 1] {
                        1.0
                    } else {
                        0.0
                    };
                    let e = ease_memo(easings, memo, id, local);
                    // NaN progress: the search stops wherever the cursor was,
                    // so the result depends on this target's cursor.
                    if self.kf_even && !p.is_nan() {
                        memo.kf_n = self.kf;
                        memo.kf_bits = bits;
                        memo.kf_ease = id;
                        memo.kf_i = i as u32;
                        memo.kf_e = e;
                    }
                    (i, e)
                };
                self.kf_cursor = i as u32;
                let (a, b) = (vals[i], vals[i + 1]);
                *o = a + (b - a) * e;
            }
            return;
        }
        Anim::eval_into(&mut self.tracks, data, easings, memo, p, out);
    }

    /// Recompute [`simple`](Self::simple) (after creation or a track toggle).
    fn classify(&mut self) {
        let t = &self.tracks;
        let ok = !t.is_empty()
            && t.iter().enumerate().all(|(k, tr)| {
                tr.enabled
                    && tr.single
                    && tr.ch == 1
                    && tr.off_at as usize == 4 * k
                    && tr.val_at as usize == 4 * k + 2
                    && tr.out_at as usize == k
                    && tr.ease0 == t[0].ease0
            })
            && self.out_at >= 4 * t.len();
        self.simple = if ok { t.len() as u32 } else { 0 };
        self.simple_ease = if ok { t[0].ease0 } else { LINEAR };
        // `kf`: hand the cursor back to the track, then reclassify.
        if self.kf > 0 {
            self.tracks[0].cursor = self.kf_cursor;
        }
        let t = &self.tracks;
        let kf = !ok
            && t.len() == 1
            && t[0].enabled
            && t[0].num1
            && t[0].ease_n <= 1
            && t[0].off_at == 0
            && t[0].val_at == t[0].nkf
            && t[0].out_at == 0
            && self.out_at >= 2 * t[0].nkf as usize;
        if kf {
            self.kf = t[0].nkf;
            self.kf_cursor = t[0].cursor;
            self.kf_even = t[0].even;
            self.simple_ease = t[0].ease0;
        } else {
            self.kf = 0;
        }
    }

    /// Run `f` on the track table with every track's `cursor` current
    /// (a `kf` animation keeps its cursor outside the table).
    fn with_tracks<R>(&mut self, f: impl FnOnce(&mut [Track], &[f64]) -> R) -> R {
        if self.kf > 0 {
            self.tracks[0].cursor = self.kf_cursor;
        }
        let r = f(&mut self.tracks, &self.buf[..self.out_at]);
        if self.kf > 0 {
            self.kf_cursor = self.tracks[0].cursor;
        }
        r
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
    /// Every delay is zero (set at creation; delays never change).
    no_delays: bool,
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
        let single = nkf == 2 && self.offsets[0] == 0.0 && self.offsets[1] == 1.0;
        if single && self.no_delays {
            // No stagger: every instance has the same progress, so time and
            // ease it once (the per-instance results would all be equal),
            // then lerp all channels.
            let mut e = [tm.eval(t).progress];
            if tm.easing != LINEAR {
                if let Some(ez) = easings.get(tm.easing as usize) {
                    ez.ease_slice(&mut e);
                }
            }
            if let Some(&id) = self.eases.first() {
                if let Some(ez) = easings.get(id as usize) {
                    ez.ease_slice(&mut e);
                }
            }
            let e = e[0];
            for (v, o) in self.values.chunks_exact(2 * ch).zip(self.out.chunks_exact_mut(ch)) {
                let (a, b) = v.split_at(ch);
                for ((o, &a), &b) in o.iter_mut().zip(a).zip(b) {
                    *o = (a + (b - a) * e) as f32;
                }
            }
            return;
        }
        // 1. Per-instance progress (shared timing, own delay).
        if tm.iterations == 1.0 && tm.direction == crate::timing::Direction::Normal && tm.delay == 0.0 && tm.duration > 0.0 {
            // What `Timing::eval` computes for one forward iteration with no
            // delay (before: 0, active: at / duration, after: 1), as selects
            // so the loop vectorises.
            let dur = tm.duration;
            for (p, &d) in self.scratch.iter_mut().zip(self.delays.iter()) {
                let at = t - d;
                *p = if at < 0.0 {
                    0.0
                } else if at < dur {
                    at / dur
                } else {
                    1.0
                };
            }
        } else {
            for (p, &d) in self.scratch.iter_mut().zip(self.delays.iter()) {
                *p = tm.eval(t - d).progress;
            }
        }
        // 2. Timing easing over the whole slice (one dispatch).
        if tm.easing != LINEAR {
            if let Some(e) = easings.get(tm.easing as usize) {
                e.ease_slice(&mut self.scratch);
            }
        }
        if single {
            // 3a. Single segment: ease the slice, then lerp all channels.
            if let Some(&id) = self.eases.first() {
                if let Some(e) = easings.get(id as usize) {
                    e.ease_slice(&mut self.scratch);
                }
            }
            let it = self.values.chunks_exact(2 * ch).zip(self.out.chunks_exact_mut(ch)).zip(self.scratch.iter());
            for ((v, o), &e) in it {
                let (a, b) = v.split_at(ch);
                for ((o, &a), &b) in o.iter_mut().zip(a).zip(b) {
                    *o = (a + (b - a) * e) as f32;
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
    /// Ballistic motion: velocity, constant acceleration (gravity), friction,
    /// bouncing bounds.
    Physics,
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
    /// Physics: friction coefficient (1/s), bounce restitution, remaining life (s).
    friction: f64,
    restitution: f64,
    life: f64,
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

/// See [`Engine::render_children`].
const LANE_MAX_MISSES: u32 = 8;

/// Bit-for-bit timing equality (`==` would equate `0.0` and `-0.0`).
#[inline(always)]
fn same_timing(a: &Timing, b: &Timing) -> bool {
    a.delay.to_bits() == b.delay.to_bits()
        && a.duration.to_bits() == b.duration.to_bits()
        && a.iterations.to_bits() == b.iterations.to_bits()
        && a.end_delay.to_bits() == b.end_delay.to_bits()
        && a.direction == b.direction
        && a.easing == b.easing
}

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
    /// The first output of every animation node, by node index (see
    /// [`first_outputs`](Self::first_outputs)).
    out0: Vec<f64>,
    /// Bumped whenever `out0` moves (see [`out0_epoch_ptr`](Self::out0_epoch_ptr)).
    out0_epoch: u32,
    /// Easing memo for the frame loop (see [`EaseMemo`]).
    memo: EaseMemo,
    /// Tests: render every group child in full (reference for the lane).
    #[cfg(test)]
    lane_off: bool,
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

/// One-entry memo of the last easing evaluated (`id`, bits of `t`, result).
///
/// The children of one `animate()` call usually share their timing, so the
/// same easing is evaluated at the same progress for every child and track of
/// a frame. `Easing::ease` is a pure function of `(id, t)` and the registry is
/// append-only, so reusing the result is exact. Keyed on the bits of `t`, so
/// `-0.0` and `0.0` (or two NaNs) never alias.
///
/// The same goes for colour tracks: targets animated between the same
/// colours convert the same interpolated colour back to RGBA, so the last
/// conversion (space and bits of the four channels in, RGBA out) is kept too.
#[derive(Clone, Copy, Debug)]
struct EaseMemo {
    id: u32,
    bits: u64,
    v: f64,
    col_space: Option<ColorSpace>,
    col_in: [u64; 4],
    col_out: [f64; 4],
    /// Keyframe segment of a `kf` animation with engine-generated offsets:
    /// keyframe count (0: none yet), bits of the progress, segment easing →
    /// segment index and eased local progress. Generated offsets depend only
    /// on the count and are strictly increasing, so the segment search finds
    /// the same segment from any cursor (NaN progress excluded) and the
    /// local progress is the same arithmetic on the same numbers: exact.
    kf_n: u32,
    kf_bits: u64,
    kf_ease: u32,
    kf_i: u32,
    kf_e: f64,
}

impl EaseMemo {
    /// Never matches: `LINEAR` is answered before the memo is consulted, and
    /// no colour has been converted.
    const EMPTY: EaseMemo = EaseMemo { id: LINEAR, bits: 0, v: 0.0, col_space: None, col_in: [0; 4], col_out: [0.0; 4], kf_n: 0, kf_bits: 0, kf_ease: LINEAR, kf_i: 0, kf_e: 0.0 };

    /// `color::from_space(c, space)` as an array, reusing the last result.
    #[inline]
    fn color(&mut self, c: &[f64; 4], space: ColorSpace) -> [f64; 4] {
        let key = [c[0].to_bits(), c[1].to_bits(), c[2].to_bits(), c[3].to_bits()];
        if self.col_space == Some(space) && self.col_in == key {
            return self.col_out;
        }
        let out = color::from_space(c, space).to_array();
        self.col_space = Some(space);
        self.col_in = key;
        self.col_out = out;
        out
    }
}

#[inline(always)]
fn ease_memo(easings: &[Easing], memo: &mut EaseMemo, id: u32, t: f64) -> f64 {
    if id == LINEAR {
        return t;
    }
    let bits = t.to_bits();
    if memo.id == id && memo.bits == bits {
        return memo.v;
    }
    let v = match easings.get(id as usize) {
        Some(e) => e.ease(t),
        None => t,
    };
    memo.id = id;
    memo.bits = bits;
    memo.v = v;
    v
}

impl Anim {
    /// Evaluate every enabled track at (eased) iteration progress `p` into `out`.
    #[inline(always)]
    fn eval_into(tracks: &mut [Track], data: &[f64], easings: &[Easing], memo: &mut EaseMemo, p: f64, out: &mut [f64]) {
        for tr in tracks.iter_mut() {
            if !tr.enabled {
                continue;
            }
            if tr.single {
                // Hot path: from → to tween. No segment search, no kind dispatch.
                let e = ease_memo(easings, memo, tr.ease0, p);
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
            if tr.num1 {
                // Keyframed single number: the segment search of `eval_track`
                // without the slicing and kind dispatch (same arithmetic).
                let n = tr.nkf as usize;
                let oa = tr.out_at as usize;
                if let (Some(off), Some(vals), Some(o)) =
                    (data.get(tr.off_at as usize..tr.off_at as usize + n), data.get(tr.val_at as usize..tr.val_at as usize + n), out.get_mut(oa))
                {
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
                    let e = ease_memo(easings, memo, id, local);
                    let (a, b) = (vals[i], vals[i + 1]);
                    *o = a + (b - a) * e;
                }
                continue;
            }
            Anim::eval_track(tr, data, easings, memo, p, out);
        }
    }

    /// The general track evaluation (keyframe segments, colour, path,
    /// matrix, discrete): out of line so the single-segment fast path in
    /// [`eval_into`](Self::eval_into) can be inlined into the frame loop
    /// without duplicating all of this.
    #[inline(never)]
    fn eval_track(tr: &mut Track, data: &[f64], easings: &[Easing], memo: &mut EaseMemo, p: f64, out: &mut [f64]) {
        let n = tr.nkf as usize;
        let ch = tr.ch as usize;
        let off = &data[tr.off_at as usize..tr.off_at as usize + n];
        let vals = &data[tr.val_at as usize..tr.val_at as usize + n * ch];
        let o = &mut out[tr.out_at as usize..tr.out_at as usize + tr.out_ch as usize];
        // Segment search with a temporal-coherence cursor: amortised O(1).
        let (i, e) = if tr.seg1 {
            (0usize, ease_memo(easings, memo, tr.ease0, p))
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
            (i, ease_memo(easings, memo, id, local))
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
                TrackKind::Matrix => {
                    let mut d = [0.0; 17];
                    d.copy_from_slice(&vals[..17]);
                    o.copy_from_slice(&crate::space::recompose(&d));
                }
                _ => o.copy_from_slice(&vals[..ch.min(o.len())]),
            }
            return;
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
                o.copy_from_slice(&memo.color(&c, space));
            }
            TrackKind::Discrete => {
                o[0] = if e < 0.5 { a[0] } else { b[0] };
            }
            TrackKind::Matrix => {
                let mut da = [0.0; 17];
                let mut db = [0.0; 17];
                da.copy_from_slice(a);
                db.copy_from_slice(b);
                o.copy_from_slice(&crate::space::recompose(&crate::space::mix_decomposed(&da, &db, e)));
            }
            TrackKind::Path => {
                let lut = &data[tr.lut_at as usize..(tr.lut_at + tr.lut_len) as usize];
                o.copy_from_slice(&sample_lut(lut, a[0] + (b[0] - a[0]) * e));
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
            out0: Vec::new(),
            out0_epoch: 0,
            memo: EaseMemo::EMPTY,
            #[cfg(test)]
            lane_off: false,
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
            let cap = self.out0.capacity();
            self.out0.push(0.0);
            if self.out0.capacity() != cap {
                self.out0_epoch = self.out0_epoch.wrapping_add(1);
            }
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
                TrackKind::Matrix => {
                    if ch != 16 {
                        return Err(Error::InvalidInput);
                    }
                    (17, 16)
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
                TrackKind::Matrix => {
                    for k in 0..nkf {
                        let mut m = [0.0; 16];
                        m.copy_from_slice(&spec.values[k * 16..k * 16 + 16]);
                        // Singular matrices (e.g. scale(0)) fall back to a near-zero scale.
                        let d = crate::space::decompose(&m).unwrap_or([
                            m[12], m[13], m[14], 1e-9, 1e-9, 1e-9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0,
                        ]);
                        data.extend_from_slice(&d);
                    }
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
                num1: spec.kind == TrackKind::Number
                    && ch == 1
                    && nkf >= 2
                    && !(nkf == 2 && data[off_at as usize] == 0.0 && data[off_at as usize + 1] == 1.0),
                even: spec.offsets.is_empty() && nkf >= 2,
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
        let mut anim = Anim { tracks: tracks.into_boxed_slice(), buf: data.into_boxed_slice(), out_at, simple: 0, simple_ease: LINEAR, kf: 0, kf_cursor: 0, kf_even: false };
        anim.classify();
        // Pre-fill outputs with the start values so readers never see zeros.
        anim.eval(&self.easings, &mut EaseMemo::EMPTY.clone(), 0.0);
        let first = anim.out().first().copied().unwrap_or(0.0);
        let idx = self.alloc(Kind::Anim(anim), sanitize(timing), fl)?;
        self.out0[idx] = first;
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
            no_delays: delays.iter().all(|&d| d == 0.0),
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
            friction: 0.0,
            restitution: 0.0,
            life: f64::INFINITY,
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
            friction: 0.0,
            restitution: 0.0,
            life: f64::INFINITY,
        };
        let idx = self.alloc(Kind::Sim(Box::new(sim)), Timing::default(), flags::DEFAULT)?;
        self.activate_sim(idx);
        Ok(self.handle_of(idx))
    }

    /// Ballistic physics simulation (projectiles, confetti, falling objects).
    ///
    /// Per channel: position, velocity (units/s) and constant acceleration
    /// (units/s², e.g. gravity). `friction` (1/s) applies exponential drag.
    /// Optional `min`/`max` bounds reflect the velocity scaled by
    /// `restitution` (0 = stop, 1 = perfectly elastic). The simulation
    /// finishes after `life_ms` (if finite) or when everything is at rest.
    /// All updates are exact for constant acceleration + linear drag.
    #[allow(clippy::too_many_arguments)]
    pub fn create_physics(
        &mut self,
        pos: &[f64],
        vel: &[f64],
        acc: &[f64],
        friction: f64,
        min: Option<&[f64]>,
        max: Option<&[f64]>,
        restitution: f64,
        life_ms: f64,
    ) -> Result<Handle, Error> {
        let ch = pos.len();
        if ch == 0 || vel.len() != ch || (acc.len() != ch && !acc.is_empty()) {
            return Err(Error::InvalidInput);
        }
        let mut buf = alloc::vec![0.0; ch * 5];
        buf[..ch].copy_from_slice(pos);
        buf[ch..ch * 2].copy_from_slice(vel);
        if !acc.is_empty() {
            buf[ch * 2..ch * 3].copy_from_slice(acc);
        }
        for k in 0..ch {
            let lo = min.and_then(|m| m.get(k).copied()).unwrap_or(f64::NEG_INFINITY);
            let hi = max.and_then(|m| m.get(k).copied()).unwrap_or(f64::INFINITY);
            if lo > hi {
                return Err(Error::InvalidInput);
            }
            buf[ch * 3 + k] = lo;
            buf[ch * 4 + k] = hi;
            // Start inside the bounds.
            buf[k] = buf[k].clamp(lo, hi);
        }
        let sim = Sim {
            kind: SimKind::Physics,
            ch,
            cfg: 0,
            buf,
            mode: alloc::vec![0; ch],
            time_constant: 0.0,
            rest_delta: 0.0,
            rest_speed: 0.05,
            friction: friction.max(0.0),
            restitution: restitution.clamp(0.0, 1.0),
            life: if life_ms.is_finite() && life_ms > 0.0 { life_ms / 1000.0 } else { f64::INFINITY },
        };
        let idx = self.alloc(Kind::Sim(Box::new(sim)), Timing::default(), flags::DEFAULT)?;
        self.activate_sim(idx);
        Ok(self.handle_of(idx))
    }

    /// Set the acceleration of a physics simulation (e.g. change gravity / wind).
    pub fn sim_set_acceleration(&mut self, h: Handle, acc: &[f64]) -> Result<(), Error> {
        let (i, s) = self.sim_mut(h)?;
        if acc.len() != s.ch || s.kind != SimKind::Physics {
            return Err(Error::InvalidInput);
        }
        let ch = s.ch;
        s.buf[ch * 2..ch * 3].copy_from_slice(acc);
        self.activate_sim(i);
        Ok(())
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
        if s.kind == SimKind::Physics {
            return Err(Error::WrongKind);
        }
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
                    a.classify();
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

    /// The first output of every animation, by node index (`handle &
    /// INDEX_MASK`): always equal to `values(h)[0]` of a live animation with
    /// outputs. Children of one `animate()` call are usually allocated in
    /// order, so their values are contiguous here (one sequential read per
    /// target for single-property writers) where each animation's own
    /// outputs sit in its own buffer.
    pub fn first_outputs(&self) -> &[f64] {
        &self.out0
    }
    /// Pointer to a counter bumped whenever [`first_outputs`](Self::first_outputs)
    /// moves (its address is stable for the engine's lifetime).
    pub fn out0_epoch_ptr(&self) -> *const u32 {
        &self.out0_epoch
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
                let easings = &self.easings;
                a.with_tracks(|tracks, data| Anim::eval_into(tracks, data, easings, &mut EaseMemo::EMPTY.clone(), pe, &mut out[..n]));
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

    /// Render node `i` at local time `t`. Returns the eased progress when an
    /// animation was evaluated in its active phase, else NaN (see
    /// [`render_children`](Self::render_children); a scalar, not an
    /// `Option`, so wasm returns it in a register).
    #[inline]
    fn render(&mut self, i: usize, t: f64, backward: bool) -> f64 {
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
            return f64::NAN;
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
                    let p = ease_memo(&self.easings, &mut self.memo, te, st.progress);
                    a.eval(&self.easings, &mut self.memo, p);
                    if let Some(&v) = a.out().first() {
                        self.out0[i] = v;
                    }
                    self.updated.push(h);
                    if st.phase == Phase::Active {
                        return p;
                    }
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
                    _ => return f64::NAN,
                };
                if child_backward {
                    self.render_children(children.iter().rev(), inner, true);
                } else {
                    self.render_children(children.iter(), inner, false);
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
        f64::NAN
    }

    /// Render a group's children at group time `inner`, in iteration order.
    ///
    /// Children of one `animate()` call usually share start, rate and timing,
    /// so they all reach the same timing state. After a child is rendered in
    /// full, the next ones with bit-identical local time and timing that are
    /// already steady in that active iteration take [`render_lane`]: no timing
    /// evaluation, no phase bookkeeping, no events (there would be none).
    ///
    /// [`render_lane`]: Self::render_lane
    #[inline(never)]
    fn render_children<'a>(&mut self, children: impl Iterator<Item = &'a u32>, inner: f64, backward: bool) {
        // (index of the last child rendered in full in its active phase, its eased progress)
        let mut lane: Option<(usize, f64)> = None;
        // Consecutive lane misses: staggered children never match, so after a
        // few misses stop checking (the checks alone cost ~8% of their tick).
        let mut misses = 0u32;
        for &c in children {
            let ci = c as usize;
            let cn = &self.nodes[ci];
            let ct = (inner - cn.start) * cn.rate;
            #[cfg(test)]
            if self.lane_off {
                lane = None;
            }
            if let Some((li, p)) = lane {
                if self.render_lane(ci, ct, li, p) {
                    misses = 0;
                    continue;
                }
                misses += 1;
            }
            let p = self.render(ci, ct, backward);
            // NaN: not an active animation (or a NaN progress): keep the old lane.
            if !p.is_nan() && misses < LANE_MAX_MISSES {
                lane = Some((ci, p));
            } else if misses >= LANE_MAX_MISSES {
                lane = None;
            }
        }
    }

    /// The fast path of [`render`](Self::render) for animation `i` at time
    /// `t` when node `li` was just rendered in full, in its active phase, at
    /// eased progress `p` (its `time`, `timing` and `last_iter` are what that
    /// render used and produced). `false` if it doesn't apply.
    #[inline(always)]
    fn render_lane(&mut self, i: usize, t: f64, li: usize, p: f64) -> bool {
        let (l, n) = (&self.nodes[li], &self.nodes[i]);
        if t.to_bits() != l.time.to_bits()
            || n.last_phase != Phase::Active as u8
            || n.last_iter != l.last_iter
            || !matches!(n.kind, Kind::Anim(_))
            || !same_timing(&n.timing, &l.timing)
        {
            return false;
        }
        let iteration = l.last_iter;
        let n = &mut self.nodes[i];
        n.time = t;
        n.last_iter = iteration;
        let h = (n.gen << INDEX_BITS) | i as u32;
        if let Kind::Anim(a) = &mut n.kind {
            a.eval(&self.easings, &mut self.memo, p);
            if let Some(&v) = a.out().first() {
                self.out0[i] = v;
            }
        }
        self.updated.push(h);
        true
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
                Kind::Sim(s) => {
                    let m = self.springs.get(s.cfg as usize).map(|c| c.matrix).unwrap_or([1.0, 0.0, 0.0, 1.0]);
                    step_sim(s, &m, dt)
                }
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
    if s.kind == SimKind::Physics {
        return step_physics(s, dt);
    }
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

/// Exact state after time `t` under constant acceleration `a` and linear drag `k`.
#[inline]
fn ballistic(x: f64, v: f64, a: f64, k: f64, t: f64) -> (f64, f64) {
    if k > 0.0 {
        let e = exp(-k * t);
        let f1 = (1.0 - e) / k;
        (x + (v - a / k) * f1 + a * t / k, v * e + a * f1)
    } else {
        (x + v * t + 0.5 * a * t * t, v + a * t)
    }
}

/// Exact integration of `x'' = a - k·x'` per channel. Bounds are resolved at
/// the moment of impact (not at the end of the frame), so bouncing loses
/// energy correctly and comes to rest regardless of frame rate.
fn step_physics(s: &mut Sim, dt: f64) -> bool {
    let ch = s.ch;
    let k = s.friction;
    let (pos, rest) = s.buf.split_at_mut(ch);
    let (vel, rest) = rest.split_at_mut(ch);
    let (acc, bounds) = rest.split_at_mut(ch);
    let (min, max) = bounds.split_at(ch);
    let mut moving = false;
    for c in 0..ch {
        let a = acc[c];
        let (lo, hi) = (min[c], max[c]);
        let (mut x, mut v) = (pos[c], vel[c]);
        let mut remaining = dt;
        // A ball resting on a bound, pushed into it, stays put.
        let resting = (x <= lo && a <= 0.0 && v <= 0.0) || (x >= hi && a >= 0.0 && v >= 0.0);
        if resting && (x == lo || x == hi) && abs(v) < 1e-9 {
            vel[c] = 0.0;
            continue;
        }
        // Up to a few bounces per frame.
        for _ in 0..8 {
            let (nx, nv) = ballistic(x, v, a, k, remaining);
            if nx >= lo && nx <= hi {
                x = nx;
                v = nv;
                break;
            }
            let b = if nx < lo { lo } else { hi };
            // Bisection for the impact time within `remaining`.
            let (mut t0, mut t1) = (0.0, remaining);
            for _ in 0..40 {
                let tm = 0.5 * (t0 + t1);
                let (xm, _) = ballistic(x, v, a, k, tm);
                if (b == lo && xm < lo) || (b == hi && xm > hi) {
                    t1 = tm;
                } else {
                    t0 = tm;
                }
            }
            let (_, vi) = ballistic(x, v, a, k, t0);
            x = b;
            v = -vi * s.restitution;
            remaining -= t0;
            // Too slow to leave the bound again (rises less than ~0.5 units): rest.
            let pushed_in = (b == lo && a < 0.0) || (b == hi && a > 0.0);
            let lift = if pushed_in { sqrt(2.0 * abs(a) * 0.5) } else { 0.0 };
            if abs(v) <= lift.max(s.rest_speed) {
                v = 0.0;
                if pushed_in || abs(a) == 0.0 {
                    break;
                }
            }
            if remaining <= 1e-12 {
                break;
            }
        }
        pos[c] = x;
        vel[c] = v;
        let on_bound_rest = v == 0.0 && ((x == lo && a <= 0.0) || (x == hi && a >= 0.0));
        if !(on_bound_rest || (abs(v) < s.rest_speed && a == 0.0)) {
            moving = true;
        }
    }
    s.life -= dt;
    !moving || s.life <= 0.0
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
    fn physics_projectile() {
        let mut e = Engine::new();
        // Free fall from rest: x = ½ g t².
        let h = e.create_physics(&[0.0], &[0.0], &[100.0], 0.0, None, None, 0.0, 0.0).unwrap();
        e.tick(500.0);
        assert!((e.values(h).unwrap()[0] - 12.5).abs() < 1e-9);
        // Drag: terminal velocity a/k.
        let d = e.create_physics(&[0.0], &[0.0], &[100.0], 5.0, None, None, 0.0, 0.0).unwrap();
        for _ in 0..200 {
            e.tick(16.0);
        }
        assert!((e.sim_velocity(d).unwrap()[0] - 20.0).abs() < 1e-4);
        // Bouncing ball comes to rest on the floor and finishes.
        let b = e
            .create_physics(&[0.0, 0.0], &[50.0, 0.0], &[0.0, 980.0], 0.5, None, Some(&[f64::INFINITY, 300.0]), 0.6, 0.0)
            .unwrap();
        let mut finished = false;
        for _ in 0..2000 {
            e.tick(16.0);
            if e.events().chunks(2).any(|c| c[0] == b && c[1] == event::FINISH) {
                finished = true;
                break;
            }
        }
        assert!(finished);
        assert_eq!(e.values(b).unwrap()[1], 300.0);
        // High restitution still comes to rest (impact-time resolution), at any frame rate.
        for dt in [16.0, 8.0, 33.0] {
            let b = e
                .create_physics(&[0.0], &[0.0], &[980.0], 0.0, None, Some(&[300.0]), 0.8, 0.0)
                .unwrap();
            let mut done = false;
            for _ in 0..(60_000.0 / dt) as usize {
                e.tick(dt);
                if e.events().chunks(2).any(|c| c[0] == b && c[1] == event::FINISH) {
                    done = true;
                    break;
                }
            }
            assert!(done, "dt {dt}");
            assert_eq!(e.values(b).unwrap()[0], 300.0);
        }
        assert_eq!(e.sim_set_target(b, &[0.0, 0.0]), Err(Error::WrongKind));
        assert!(e.create_physics(&[0.0], &[0.0], &[], 0.0, Some(&[5.0]), Some(&[1.0]), 0.5, 0.0).is_err());
        // Life limit.
        let l = e.create_physics(&[0.0], &[10.0], &[], 0.0, None, None, 0.0, 100.0).unwrap();
        e.tick(60.0);
        assert_eq!(e.state(l).unwrap(), PlayState::Playing);
        e.tick(60.0);
        assert_eq!(e.state(l).unwrap(), PlayState::Finished);
    }

    #[test]
    fn matrix_tracks() {
        let mut e = Engine::new();
        let a = crate::space::parse_transform("rotateY(0deg)").unwrap();
        let b = crate::space::parse_transform("rotateY(90deg) translateZ(100px)").unwrap();
        let mut v = a.to_vec();
        v.extend_from_slice(&b);
        let mut spec = TrackSpec::number(v, 16);
        spec.kind = TrackKind::Matrix;
        let h = e.create_anim(Timing { duration: 100.0, ..Default::default() }, &[spec], flags::DEFAULT).unwrap();
        e.play(h).unwrap();
        e.tick(100.0);
        let out = e.values(h).unwrap();
        for k in 0..16 {
            assert!((out[k] - b[k]).abs() < 1e-9);
        }
        e.seek(h, 50.0).unwrap();
        let mid = e.values(h).unwrap();
        let expect = crate::space::mix_matrix(&a, &b, 0.5);
        for k in 0..16 {
            assert!((mid[k] - expect[k]).abs() < 1e-9);
        }
    }

    #[test]
    fn uniform_batches_match_per_instance_eval_bit_for_bit() {
        // The no-delay fast path times and eases once; it must equal the
        // per-instance evaluation (timing easing, segment easing, repeats,
        // alternate direction, before/after phases).
        let mut e = Engine::new();
        let te = e.easing("inOutSine").unwrap();
        let se = e.easing("outBack").unwrap();
        let (n, ch) = (257, 3);
        let values: Vec<f64> = (0..n * ch * 2).map(|i| ((i * 7919) % 1000) as f64 * 0.37 - 100.0).collect();
        let timing = Timing { duration: 300.0, iterations: 2.5, direction: crate::timing::Direction::Alternate, easing: te, ..Default::default() };
        let mk = |e: &mut Engine, delays: Vec<f64>| {
            let spec = BatchSpec { count: n, channels: ch, offsets: vec![], easings: vec![se], values: values.clone(), delays };
            let h = e.create_batch(timing, spec, flags::DEFAULT).unwrap();
            e.play(h).unwrap();
            h
        };
        let h = mk(&mut e, vec![]);
        // The previous per-instance evaluation, verbatim (single segment).
        let reference = |e: &Engine, t: f64| -> Vec<f32> {
            let Kind::Batch(b) = &e.nodes[Engine::handle_index(h) as usize].kind else { unreachable!() };
            let tm = b.timing;
            let mut scratch: Vec<f64> = b.delays.iter().map(|&d| tm.eval(t - d).progress).collect();
            e.easings[te as usize].ease_slice(&mut scratch);
            e.easings[se as usize].ease_slice(&mut scratch);
            let mut out = vec![0.0f32; n * ch];
            for i in 0..n {
                let v = &b.values[i * 2 * ch..i * 2 * ch + 2 * ch];
                for k in 0..ch {
                    out[i * ch + k] = (v[k] + (v[ch + k] - v[k]) * scratch[i]) as f32;
                }
            }
            out
        };
        let mut t = 0.0;
        while t < 900.0 {
            let want = reference(&e, e.time(h).unwrap());
            let got = e.batch_values(h).unwrap();
            assert_eq!(got.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), want.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "t = {t}");
            e.tick(17.0);
            t += 17.0;
        }
    }

    #[test]
    fn memoised_easing_matches_direct_evaluation_bit_for_bit() {
        // Children of one group: staggered starts and delays, timing easings
        // on some, several easing ids interleaved across children and tracks,
        // single-segment and keyframe tracks. Every rendered value must equal
        // a direct evaluation that never reuses an easing result.
        let mut e = Engine::new();
        let ids = [
            e.easing("outCubic").unwrap(),
            e.easing("inOutSine").unwrap(),
            e.easing("outBack").unwrap(),
            e.easing("spring(200, 12)").unwrap(),
            LINEAR,
        ];
        let te = e.easing("inOutQuad").unwrap();
        let g = e.create_group(Timing::default(), true, flags::DEFAULT).unwrap();
        struct Child {
            h: Handle,
            tracks: Vec<(Vec<f64>, Vec<u32>)>,
        }
        let mut kids = Vec::new();
        for i in 0..90usize {
            let fi = i as f64;
            let tracks = vec![
                (vec![fi, -2.0 * fi], vec![ids[i % 5]]),
                (vec![0.0, 50.0 + fi, 10.0, 80.0], vec![ids[(i + 1) % 5], ids[(i + 2) % 5], ids[(i + 3) % 5]]),
                (vec![1.0, 2.0], vec![ids[i % 5]]),
                (vec![0.0, 10.0, 20.0, 5.0, 1.0], vec![ids[(i + 2) % 5]]),
            ];
            let specs: Vec<TrackSpec> = tracks.iter().map(|(v, es)| TrackSpec::number(v.clone(), 1).with_easings(es.clone())).collect();
            let timing = Timing {
                delay: (i % 7) as f64 * 20.0,
                duration: 400.0 + (i % 3) as f64 * 100.0,
                iterations: if i % 4 == 0 { 2.5 } else { 1.0 },
                direction: crate::timing::Direction::from_u8(if i % 5 == 0 { 2 } else { 0 }),
                easing: if i % 2 == 0 { te } else { LINEAR },
                ..Default::default()
            };
            let h = e.create_anim(timing, &specs, flags::DEFAULT).unwrap();
            e.group_add(g, h, (i % 3) as f64 * 30.0).unwrap();
            kids.push(Child { h, tracks });
        }
        e.play(g).unwrap();
        let direct = |e: &Engine, vals: &[f64], es: &[u32], p: f64| -> f64 {
            let n = vals.len();
            let off: Vec<f64> = (0..n).map(|k| k as f64 / (n - 1) as f64).collect();
            let mut i = 0;
            while i < n - 2 && p >= off[i + 1] {
                i += 1;
            }
            let local = if n == 2 {
                p
            } else {
                let span = off[i + 1] - off[i];
                if span > 0.0 {
                    (p - off[i]) / span
                } else if p >= off[i + 1] {
                    1.0
                } else {
                    0.0
                }
            };
            let id = if es.len() == 1 { es[0] } else { es[i] };
            let k = ease_by(&e.easings, id, local);
            vals[i] + (vals[i + 1] - vals[i]) * k
        };
        let mut checked = 0;
        for _ in 0..110 {
            e.tick(17.0);
            for c in &kids {
                if !e.updated().contains(&c.h) {
                    continue;
                }
                let n = &e.nodes[Engine::handle_index(c.h) as usize];
                let p = ease_by(&e.easings, n.timing.easing, n.timing.eval(n.time).progress);
                let got = e.values(c.h).unwrap();
                for (k, (vals, es)) in c.tracks.iter().enumerate() {
                    assert_eq!(got[k].to_bits(), direct(&e, vals, es, p).to_bits(), "child {} track {k}", c.h);
                }
                checked += 1;
            }
        }
        assert!(checked > 3000, "{checked}");
    }

    #[test]
    fn group_lane_matches_full_render() {
        // Same scenario on two engines, one rendering every group child in
        // full: values, updated lists, events and node state must agree.
        fn build(lane_off: bool) -> (Engine, Handle, Vec<Handle>) {
            let mut e = Engine::new();
            e.lane_off = lane_off;
            let ease = e.easing("outCubic").unwrap();
            let sine = e.easing("inOutSine").unwrap();
            let g = e.create_group(Timing { iterations: 2.0, direction: crate::timing::Direction::Alternate, ..Default::default() }, true, flags::DEFAULT)
                .unwrap();
            let mut kids = Vec::new();
            for i in 0..64usize {
                let timing = Timing {
                    delay: if i % 16 == 15 { 50.0 } else { 0.0 },
                    duration: if i % 11 == 10 { 300.0 } else { 500.0 },
                    iterations: if i % 13 == 12 { 3.0 } else { 1.0 },
                    easing: if i % 9 == 8 { sine } else { LINEAR },
                    ..Default::default()
                };
                let specs = [
                    TrackSpec::number(vec![0.0, 500.0 + i as f64], 1).with_easings(vec![ease]),
                    TrackSpec::number(vec![0.0, 50.0, 10.0, 80.0, i as f64], 1).with_easings(vec![sine]),
                ];
                let fl = if i % 7 == 6 { flags::DEFAULT & !flags::FILL_FORWARDS } else { flags::DEFAULT };
                let h = e.create_anim(timing, &specs, fl).unwrap();
                e.group_add(g, h, if i % 10 == 9 { 40.0 } else { 0.0 }).unwrap();
                kids.push(h);
            }
            // A callback and a nested group among the children.
            let cb = e.create_callback().unwrap();
            e.group_add(g, cb, 120.0).unwrap();
            e.play(g).unwrap();
            (e, g, kids)
        }
        let (mut a, g, kids) = build(false);
        let (mut b, _, _) = build(true);
        let snap = |e: &Engine| {
            let mut v: Vec<u64> = Vec::new();
            for &h in &kids {
                v.extend(e.values(h).unwrap().iter().map(|x| x.to_bits()));
                let n = &e.nodes[Engine::handle_index(h) as usize];
                v.extend([n.time.to_bits(), n.last_phase as u64, n.last_iter.to_bits(), n.state as u64]);
            }
            (v, e.updated().to_vec(), e.events().to_vec())
        };
        for f in 0..140 {
            match f {
                30 => {
                    a.seek(g, 200.0).unwrap();
                    b.seek(g, 200.0).unwrap();
                }
                50 => {
                    a.reverse(g).unwrap();
                    b.reverse(g).unwrap();
                }
                70 => {
                    a.set_rate(g, 2.5).unwrap();
                    b.set_rate(g, 2.5).unwrap();
                }
                90 => {
                    a.seek(g, 0.0).unwrap();
                    b.seek(g, 0.0).unwrap();
                    a.set_rate(g, 1.0).unwrap();
                    b.set_rate(g, 1.0).unwrap();
                }
                _ => {}
            }
            a.tick(17.0);
            b.tick(17.0);
            assert_eq!(snap(&a), snap(&b), "frame {f}");
        }
    }

    #[test]
    fn keyframed_number_fast_path_matches_eval_track() {
        let mut e = Engine::new();
        let ids = [e.easing("outBack").unwrap(), e.easing("inOutSine").unwrap(), e.easing("steps(3)").unwrap()];
        let specs = [
            TrackSpec::number(vec![0.0, 50.0, 10.0, 80.0, 20.0, 90.0, 30.0, 100.0], 1).with_easings(vec![ids[0]]),
            TrackSpec::number(vec![5.0, -5.0, 7.0, 1.0], 1).with_easings(vec![ids[0], ids[1], ids[2]]),
            TrackSpec::number(vec![1.0, 2.0, 3.0, 4.0], 1).with_offsets(vec![0.0, 0.5, 0.5, 1.0]),
            TrackSpec::number(vec![1.0, 9.0], 1).with_offsets(vec![0.2, 0.7]).with_easings(vec![ids[1]]),
            TrackSpec::number(vec![3.0, 4.0, 6.0], 1).with_offsets(vec![0.3, 0.1, 0.9]),
        ];
        let h = e.create_anim(Timing::default(), &specs, flags::DEFAULT).unwrap();
        let Kind::Anim(anim) = &mut e.nodes[Engine::handle_index(h) as usize].kind else { unreachable!() };
        assert!(anim.tracks.iter().all(|t| t.num1));
        let mut reference = anim.tracks.to_vec();
        let mut rng = crate::math::Rng(7);
        let mut ps: Vec<f64> = (0..200).map(|k| k as f64 / 150.0 - 0.2).collect();
        ps.extend((0..200).map(|k| 1.1 - k as f64 / 160.0));
        ps.extend((0..400).map(|_| rng.next_f64() * 1.6 - 0.3));
        ps.extend([0.0, -0.0, 0.5, 1.0, 0.2, 0.7, f64::NAN, 0.4]);
        let (data, out) = anim.buf.split_at_mut(anim.out_at);
        let mut want = vec![0.0; out.len()];
        for &p in &ps {
            Anim::eval_into(&mut anim.tracks, data, &e.easings, &mut EaseMemo::EMPTY.clone(), p, out);
            for tr in reference.iter_mut() {
                Anim::eval_track(tr, data, &e.easings, &mut EaseMemo::EMPTY.clone(), p, &mut want);
            }
            assert_eq!(out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), want.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "p = {p}");
            assert_eq!(anim.tracks.iter().map(|t| t.cursor).collect::<Vec<_>>(), reference.iter().map(|t| t.cursor).collect::<Vec<_>>());
        }
    }

    #[test]
    fn memoised_colour_conversion_matches_from_space() {
        // Interleaved spaces and colours (shared and per-target), alpha 0,
        // hue spaces: every output equals a fresh conversion.
        let mut e = Engine::new();
        let ease = e.easing("inOutQuad").unwrap();
        let spaces = [ColorSpace::Oklab, ColorSpace::Srgb, ColorSpace::Oklch, ColorSpace::Hsl, ColorSpace::LinearSrgb];
        let g = e.create_group(Timing::default(), true, flags::DEFAULT).unwrap();
        let mut kids = Vec::new();
        for i in 0..60usize {
            let to_b = if i % 3 == 0 { i as f64 * 4.0 } else { 255.0 };
            let alpha = if i % 11 == 0 { 0.0 } else { 1.0 };
            let mut c = TrackSpec::number(vec![255.0, 0.0, 0.0, 1.0, 0.0, 30.0, to_b, alpha], 4).with_easings(vec![ease]);
            let space = spaces[(i / 2) % spaces.len()];
            c.kind = TrackKind::Color(space);
            let mut c2 = TrackSpec::number(vec![10.0, 200.0, 30.0, 0.5, 250.0, 250.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], 4);
            c2.kind = TrackKind::Color(spaces[i % spaces.len()]);
            let h = e.create_anim(Timing { duration: 400.0 + (i % 2) as f64 * 100.0, ..Default::default() }, &[c, c2], flags::DEFAULT).unwrap();
            e.group_add(g, h, 0.0).unwrap();
            kids.push(h);
        }
        e.play(g).unwrap();
        let mut checked = 0;
        for _ in 0..40 {
            e.tick(16.0);
            for &h in &kids {
                if !e.updated().contains(&h) {
                    continue;
                }
                let Kind::Anim(a) = &e.nodes[Engine::handle_index(h) as usize].kind else { unreachable!() };
                let n = &e.nodes[Engine::handle_index(h) as usize];
                let p = n.timing.eval(n.time).progress;
                let mut want = vec![0.0; 8];
                let mut tracks = a.tracks.to_vec();
                let data = &a.buf[..a.out_at];
                for tr in tracks.iter_mut() {
                    // eval_track with a fresh memo per track: no reuse across targets.
                    Anim::eval_track(tr, data, &e.easings, &mut EaseMemo::EMPTY.clone(), p, &mut want);
                }
                let got = e.values(h).unwrap();
                assert_eq!(got.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), want.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
                checked += 1;
            }
        }
        assert!(checked > 1000, "{checked}");
    }

    #[test]
    fn simple_anims_match_track_evaluation() {
        // `simple` animations skip the track table; they must equal
        // `eval_into` exactly, before and after tracks are toggled.
        let mut e = Engine::new();
        let ease = e.easing("outCubic").unwrap();
        let other = e.easing("inOutSine").unwrap();
        let mk = |e: &mut Engine, specs: &[TrackSpec]| e.create_anim(Timing::default(), specs, flags::DEFAULT).unwrap();
        let n3 = |es: [u32; 3]| -> Vec<TrackSpec> {
            [(0.0, 500.0), (-3.0, 300.0), (1.0, 0.0)].iter().zip(es).map(|(&(a, b), x)| TrackSpec::number(vec![a, b], 1).with_easings(vec![x])).collect()
        };
        let hs = [
            mk(&mut e, &n3([ease; 3])),
            mk(&mut e, &n3([LINEAR; 3])),
            mk(&mut e, &n3([ease, other, ease])),
            mk(&mut e, &[TrackSpec::number(vec![0.0, 1.0, 2.0, 3.0], 2)]),
            mk(&mut e, &[TrackSpec::number(vec![5.0, 6.0], 1).with_easings(vec![ease])]),
        ];
        let simple = |e: &Engine, h: Handle| match &e.nodes[Engine::handle_index(h) as usize].kind {
            Kind::Anim(a) => a.simple,
            _ => unreachable!(),
        };
        assert_eq!(hs.iter().map(|&h| simple(&e, h)).collect::<Vec<_>>(), [3, 3, 0, 0, 1]);
        let check = |e: &mut Engine| {
            for &h in &hs {
                for k in 0..=40 {
                    let p = k as f64 / 32.0 - 0.1;
                    let Kind::Anim(a) = &mut e.nodes[Engine::handle_index(h) as usize].kind else { unreachable!() };
                    let mut want = a.out().to_vec();
                    let (data, _) = a.buf.split_at(a.out_at);
                    let mut tracks = a.tracks.to_vec();
                    Anim::eval_into(&mut tracks, data, &e.easings, &mut EaseMemo::EMPTY.clone(), p, &mut want);
                    a.eval(&e.easings, &mut EaseMemo::EMPTY.clone(), p);
                    assert_eq!(a.out().iter().map(|v| v.to_bits()).collect::<Vec<_>>(), want.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
                }
            }
        };
        check(&mut e);
        e.set_track_enabled(hs[0], 1, false).unwrap();
        assert_eq!(simple(&e, hs[0]), 0);
        check(&mut e);
        e.set_track_enabled(hs[0], 1, true).unwrap();
        assert_eq!(simple(&e, hs[0]), 3);
        check(&mut e);
    }

    #[test]
    fn first_outputs_track_values() {
        // `first_outputs()[index]` equals `values(h)[0]` for every live
        // animation after creation, ticks (full renders and the group lane),
        // seeks, reversal, and node reuse.
        let mut e = Engine::new();
        let ease = e.easing("outCubic").unwrap();
        let mut live: Vec<Handle> = Vec::new();
        let g = e.create_group(Timing::default(), true, flags::DEFAULT).unwrap();
        let mut mk = |e: &mut Engine, k: usize| -> Handle {
            // Group children (even k) share timing and start: the lane renders most of them.
            let tm = Timing { duration: if k % 2 == 0 { 300.0 } else { 300.0 + (k % 3) as f64 * 50.0 }, ..Default::default() };
            let spec = match k % 4 {
                0 => vec![TrackSpec::number(vec![0.0, 50.0, 10.0, 80.0, 20.0], 1).with_easings(vec![ease])],
                1 => vec![TrackSpec::number(vec![k as f64, 500.0], 1).with_easings(vec![ease]), TrackSpec::number(vec![1.0, 0.0], 1)],
                2 => vec![TrackSpec::number(vec![1.0, 2.0, 3.0, 4.0], 2)],
                _ => {
                    let mut c = TrackSpec::number(vec![255.0, 0.0, 0.0, 1.0, 0.0, 0.0, 255.0, 1.0], 4);
                    c.kind = TrackKind::Color(ColorSpace::Oklab);
                    vec![c]
                }
            };
            e.create_anim(tm, &spec, flags::DEFAULT).unwrap()
        };
        for k in 0..40 {
            let h = mk(&mut e, k);
            if k % 2 == 0 {
                e.group_add(g, h, if k < 30 { 0.0 } else { 10.0 }).unwrap();
            } else {
                e.play(h).unwrap();
            }
            live.push(h);
        }
        e.play(g).unwrap();
        let check = |e: &Engine, live: &[Handle]| {
            for &h in live {
                if let Ok(v) = e.values(h) {
                    assert_eq!(e.first_outputs()[Engine::handle_index(h) as usize].to_bits(), v[0].to_bits());
                }
            }
        };
        check(&e, &live);
        for f in 0..60 {
            match f {
                10 => {
                    e.seek(g, 120.0).unwrap();
                }
                20 => {
                    e.reverse(g).unwrap();
                }
                30 => {
                    // Release some, create more (reused indices).
                    for h in live.drain(..10) {
                        e.release(h).unwrap();
                    }
                    for k in 0..10 {
                        let h = mk(&mut e, k);
                        e.play(h).unwrap();
                        live.push(h);
                    }
                }
                _ => {}
            }
            e.tick(17.0);
            check(&e, &live);
            e.clear_frame();
        }
    }

    #[test]
    fn kf_anims_match_track_evaluation() {
        // `kf` animations skip the track table and share the segment of
        // evenly spaced keyframes through the frame memo; outputs and cursors
        // must equal `eval_into` on a private copy of the tracks, bit for bit,
        // with one memo shared by all of them (as in the frame loop), across
        // `sample()` calls and track toggles.
        let mut e = Engine::new();
        let ids = [e.easing("inOutSine").unwrap(), e.easing("outBack").unwrap(), e.easing("steps(3)").unwrap()];
        let kf8 = |s: f64| (0..8).map(|k| [0.0, 50.0, 10.0, 80.0, 20.0, 90.0, 30.0, 100.0][k] * s).collect::<Vec<f64>>();
        let specs: Vec<TrackSpec> = vec![
            TrackSpec::number(kf8(1.0), 1).with_easings(vec![ids[0]]),
            TrackSpec::number(kf8(-2.0), 1).with_easings(vec![ids[0]]),
            // Same easing and progress, another keyframe count: no sharing.
            TrackSpec::number(vec![1.0, 2.0, 3.0, 4.0, 5.0], 1).with_easings(vec![ids[0]]),
            TrackSpec::number(kf8(3.0), 1),
            TrackSpec::number(kf8(1.0), 1).with_easings(vec![ids[2]]),
            TrackSpec::number(vec![1.0, 2.0, 3.0, 4.0], 1).with_offsets(vec![0.0, 0.5, 0.5, 1.0]),
            TrackSpec::number(vec![3.0, 4.0, 6.0], 1).with_offsets(vec![0.3, 0.1, 0.9]).with_easings(vec![ids[1]]),
            TrackSpec::number(vec![1.0, 9.0], 1).with_offsets(vec![0.2, 0.7]).with_easings(vec![ids[0]]),
            // Not `kf`: per-segment easings, two tracks, two channels.
            TrackSpec::number(vec![5.0, -5.0, 7.0, 1.0], 1).with_easings(vec![ids[0], ids[1], ids[2]]),
        ];
        let mut hs: Vec<Handle> = specs.iter().map(|s| e.create_anim(Timing::default(), core::slice::from_ref(s), flags::DEFAULT).unwrap()).collect();
        hs.push(e.create_anim(Timing::default(), &specs[..2], flags::DEFAULT).unwrap());
        hs.push(e.create_anim(Timing::default(), &[TrackSpec::number(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0], 2)], flags::DEFAULT).unwrap());
        let kf = |e: &Engine, h: Handle| match &e.nodes[Engine::handle_index(h) as usize].kind {
            Kind::Anim(a) => (a.kf, a.kf_even),
            _ => unreachable!(),
        };
        assert_eq!(
            hs.iter().map(|&h| kf(&e, h)).collect::<Vec<_>>(),
            [(8, true), (8, true), (5, true), (8, true), (8, true), (4, false), (3, false), (2, false), (0, false), (0, false), (0, false)]
        );
        // Private reference copies of every track table (cursors included).
        let mut reference: Vec<Vec<Track>> = hs
            .iter()
            .map(|&h| match &e.nodes[Engine::handle_index(h) as usize].kind {
                Kind::Anim(a) => a.tracks.to_vec(),
                _ => unreachable!(),
            })
            .collect();
        let mut rng = crate::math::Rng(11);
        let mut ps: Vec<f64> = (0..160).map(|k| k as f64 / 120.0 - 0.2).collect();
        ps.extend((0..160).map(|k| 1.1 - k as f64 / 130.0));
        ps.extend((0..300).map(|_| rng.next_f64() * 1.6 - 0.3));
        ps.extend([0.0, -0.0, 0.5, 1.0, 2.0 / 7.0, 0.2, 0.7, f64::NAN, 0.4, f64::NAN, 0.95, f64::NAN, 0.05]);
        let mut memo = EaseMemo::EMPTY;
        for (step, &p) in ps.iter().enumerate() {
            // Every few steps, give each animation its own progress (memo misses).
            // Before a NaN progress, too: the cursors then differ, and a NaN
            // search stops wherever each cursor is.
            let own = step % 7 == 3 || ps.get(step + 1).is_some_and(|q| q.is_nan());
            if step == 200 {
                for &h in &hs[..8] {
                    e.set_track_enabled(h, 0, false).unwrap();
                    assert_eq!(kf(&e, h).0, 0);
                }
            }
            if step == 230 {
                for &h in &hs[..8] {
                    e.set_track_enabled(h, 0, true).unwrap();
                }
            }
            for (k, &h) in hs.iter().enumerate() {
                let p = if own { p + k as f64 * 0.11 } else { p };
                if step % 11 == 5 {
                    // `sample()` moves the same cursor.
                    let mut got = [0.0; 4];
                    e.sample(h, p, &mut got).unwrap();
                    let (data, mut want) = {
                        let Kind::Anim(a) = &e.nodes[Engine::handle_index(h) as usize].kind else { unreachable!() };
                        (a.buf[..a.out_at].to_vec(), vec![0.0; a.out().len()])
                    };
                    Anim::eval_into(&mut reference[k], &data, &e.easings, &mut EaseMemo::EMPTY.clone(), p, &mut want);
                    assert_eq!(got[..want.len()].iter().map(|v| v.to_bits()).collect::<Vec<_>>(), want.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
                    continue;
                }
                let easings = e.easings.clone();
                let Kind::Anim(a) = &mut e.nodes[Engine::handle_index(h) as usize].kind else { unreachable!() };
                for (tr, r) in a.tracks.iter().zip(reference[k].iter_mut()) {
                    r.enabled = tr.enabled;
                }
                let mut want = a.out().to_vec();
                let data = a.buf[..a.out_at].to_vec();
                Anim::eval_into(&mut reference[k], &data, &easings, &mut EaseMemo::EMPTY.clone(), p, &mut want);
                a.eval(&easings, &mut memo, p);
                assert_eq!(
                    a.out().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    want.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    "step {step}, anim {k}, p = {p}"
                );
                let cursors = a.with_tracks(|t, _| t.iter().map(|t| t.cursor).collect::<Vec<_>>());
                assert_eq!(cursors, reference[k].iter().map(|t| t.cursor).collect::<Vec<_>>(), "step {step}, anim {k}");
            }
        }
    }

    #[test]
    fn staggered_batch_progress_matches_timing_eval() {
        let mut rng = crate::math::Rng(3);
        for &(duration, iterations, dir) in &[(300.0, 1.0, 0u8), (300.0, 2.5, 2), (0.0, 1.0, 0), (f64::INFINITY, 1.0, 0), (250.0, 1.0, 1)] {
            let tm = Timing { duration, iterations, direction: crate::timing::Direction::from_u8(dir), ..Default::default() };
            let n = 300;
            let delays: Vec<f64> = (0..n).map(|i| if i % 9 == 0 { 0.0 } else { rng.next_f64() * 400.0 - 50.0 }).collect();
            let mut b = Batch {
                n,
                ch: 1,
                nkf: 3,
                timing: tm,
                offsets: vec![0.0, 0.5, 1.0].into_boxed_slice(),
                eases: Box::new([]),
                values: (0..n * 3).map(|i| i as f64).collect(),
                delays: delays.clone().into_boxed_slice(),
                no_delays: false,
                scratch: vec![0.0; n].into_boxed_slice(),
                seg: vec![0; n].into_boxed_slice(),
                out: vec![0.0; n].into_boxed_slice(),
            };
            for k in -5..60 {
                let t = k as f64 * 13.7;
                b.eval(&[], t);
                for (i, &d) in delays.iter().enumerate() {
                    assert_eq!(b.scratch[i].to_bits(), tm.eval(t - d).progress.to_bits(), "{duration} {iterations} {dir} t={t} d={d}");
                }
            }
        }
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
