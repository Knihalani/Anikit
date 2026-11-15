//! WebAssembly bindings for `anikit-core`.
//!
//! This deliberately avoids `wasm-bindgen`: every export is a plain C-ABI
//! function taking numbers. Bulk inputs are written by the host into a scratch
//! buffer inside linear memory (`ak_f64_buf` / `ak_u8_buf`) and outputs are
//! read directly from linear memory (`ak_values_ptr`, `ak_updated_ptr`), so a
//! frame costs exactly one host → wasm call (`ak_tick`) regardless of how many
//! animations are running.
//!
//! WebAssembly is single-threaded here, so global state lives in one static
//! cell accessed through [`st`].
#![allow(clippy::missing_safety_doc)]
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

// On wasm: no std runtime, a compact malloc, and a panic handler that traps
// without pulling in any formatting machinery.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

use anikit_core::color::{self, ColorSpace};
use anikit_core::css;
use anikit_core::easing::Easing;
use anikit_core::engine::{Engine, Error, Handle, TrackKind, TrackSpec, LINEAR};
use anikit_core::morph;
use anikit_core::path::{Path, PathLut};
use anikit_core::spring::SpringParams;
use anikit_core::stagger::{stagger, Axis, Stagger, StaggerFrom};
use anikit_core::timing::{Direction, Timing};
use core::cell::UnsafeCell;

struct State {
    engine: Engine,
    f64_buf: Vec<f64>,
    u8_buf: Vec<u8>,
    out_str: String,
    luts: Vec<Option<PathLut>>,
    free_luts: Vec<u32>,
    noise: Option<anikit_core::noise::Noise>,
    worlds: Vec<Option<anikit_core::world::World>>,
    projectors: Vec<Option<anikit_core::space::Projector>>,
    points: Vec<Vec<f32>>,
}

struct Global(UnsafeCell<State>);
// SAFETY: wasm32-unknown-unknown without threads is single-threaded.
unsafe impl Sync for Global {}

static STATE: Global = Global(UnsafeCell::new(State {
    engine: Engine::new(),
    f64_buf: Vec::new(),
    u8_buf: Vec::new(),
    out_str: String::new(),
    luts: Vec::new(),
    free_luts: Vec::new(),
    noise: None,
    worlds: Vec::new(),
    projectors: Vec::new(),
    points: Vec::new(),
}));

#[inline(always)]
#[allow(clippy::mut_from_ref)]
fn st() -> &'static mut State {
    // SAFETY: single-threaded; each export obtains the reference once and
    // never re-enters (no callbacks into the host while it is held).
    unsafe { &mut *STATE.0.get() }
}

const ERR_EASING: u32 = 0xFFFF_FFFE;

fn code(r: Result<(), Error>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(Error::BadHandle) => 1,
        Err(Error::WrongKind) => 2,
        Err(Error::InvalidInput) => 3,
        Err(Error::Capacity) => 4,
        Err(Error::NotRoot) => 5,
    }
}

fn handle(r: Result<Handle, Error>) -> u32 {
    r.unwrap_or(0)
}

fn input_str(len: u32) -> &'static str {
    let s = st();
    let len = (len as usize).min(s.u8_buf.len());
    // The host writes UTF-8; fall back to empty on invalid input.
    core::str::from_utf8(&s.u8_buf[..len]).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Scratch memory
// ---------------------------------------------------------------------------

/// Ensure the f64 scratch buffer holds at least `n` values; returns its address.
#[no_mangle]
pub extern "C" fn ak_f64_buf(n: u32) -> *mut f64 {
    let s = st();
    if s.f64_buf.len() < n as usize {
        s.f64_buf.resize(n as usize, 0.0);
    }
    s.f64_buf.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn ak_u8_buf(n: u32) -> *mut u8 {
    let s = st();
    if s.u8_buf.len() < n as usize {
        s.u8_buf.resize(n as usize, 0);
    }
    s.u8_buf.as_mut_ptr()
}

/// Address of the last string result (length returned by the producing call).
#[no_mangle]
pub extern "C" fn ak_str_ptr() -> *const u8 {
    st().out_str.as_ptr()
}

fn set_out(s: String) -> u32 {
    let len = s.len() as u32;
    st().out_str = s;
    len
}

fn write_f64(vals: &[f64]) -> *mut f64 {
    let p = ak_f64_buf(vals.len() as u32);
    st().f64_buf[..vals.len()].copy_from_slice(vals);
    p
}

// ---------------------------------------------------------------------------
// Easings
// ---------------------------------------------------------------------------

/// Parse an easing string from the u8 buffer. Returns its id,
/// `0xFFFFFFFF` for linear or `0xFFFFFFFE` on error.
#[no_mangle]
pub extern "C" fn ak_easing_parse(len: u32) -> u32 {
    let src = input_str(len);
    st().engine.easing(src).unwrap_or(ERR_EASING)
}

/// Register a lookup-table easing from `n` samples in the f64 buffer.
#[no_mangle]
pub extern "C" fn ak_easing_table(n: u32) -> u32 {
    let s = st();
    let n = (n as usize).min(s.f64_buf.len());
    let samples = s.f64_buf[..n].to_vec().into_boxed_slice();
    s.engine.add_easing(Easing::Table(samples))
}

#[no_mangle]
pub extern "C" fn ak_easing_bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> u32 {
    st().engine.add_easing(Easing::bezier(x1, y1, x2, y2))
}

#[no_mangle]
pub extern "C" fn ak_easing_spring(stiffness: f64, damping: f64, mass: f64, velocity: f64) -> u32 {
    st().engine.add_easing(Easing::spring(SpringParams::new(stiffness, damping, mass), velocity))
}

/// Perceptual spring easing: visual duration (s) + bounce (0..1).
#[no_mangle]
pub extern "C" fn ak_easing_spring_db(duration: f64, bounce: f64, velocity: f64) -> u32 {
    st().engine.add_easing(Easing::spring(SpringParams::from_duration_bounce(duration, bounce), velocity))
}

#[no_mangle]
pub extern "C" fn ak_ease(id: u32, t: f64) -> f64 {
    st().engine.ease(id, t)
}

/// Natural duration (seconds) of a spring easing, or -1.
#[no_mangle]
pub extern "C" fn ak_easing_duration(id: u32) -> f64 {
    st().engine.easing_ref(id).and_then(|e| e.natural_duration()).unwrap_or(-1.0)
}

/// Native CSS easing string (into the string result). Returns its length.
#[no_mangle]
pub extern "C" fn ak_easing_css(id: u32, tolerance: f64) -> u32 {
    let s = if id == LINEAR {
        String::from("linear")
    } else {
        match st().engine.easing_ref(id) {
            Some(e) => css::to_css(e, tolerance),
            None => String::from("linear"),
        }
    };
    set_out(s)
}

/// Write `n` uniform samples of an easing to the f64 buffer.
#[no_mangle]
pub extern "C" fn ak_easing_sample(id: u32, n: u32) -> *mut f64 {
    let p = ak_f64_buf(n);
    let s = st();
    let n = n as usize;
    for i in 0..n {
        let t = if n > 1 { i as f64 / (n - 1) as f64 } else { 1.0 };
        s.f64_buf[i] = s.engine.ease(id, t);
    }
    p
}

// ---------------------------------------------------------------------------
// Creation
// ---------------------------------------------------------------------------

fn read_timing(b: &[f64]) -> Timing {
    Timing {
        delay: b[0],
        duration: b[1],
        iterations: b[2],
        direction: Direction::from_u8(b[3] as u8),
        end_delay: b[4],
        easing: if b[5] < 0.0 { LINEAR } else { b[5] as u32 },
    }
}

/// Create an animation from an encoded description in the f64 buffer:
///
/// ```text
/// delay, duration, iterations, direction, endDelay, timingEasing(-1 = linear), flags, nTracks,
///   per track: kind(0 num, 1 color, 2 discrete, 3 path), arg(color space | path lut id),
///              channels, nKeyframes, hasOffsets(0/1), [offsets], values..., nEasings, easings...
/// ```
///
/// Returns the handle, or 0 on error.
#[no_mangle]
pub extern "C" fn ak_anim_create(len: u32) -> u32 {
    let s = st();
    let b = &s.f64_buf[..(len as usize).min(s.f64_buf.len())];
    if b.len() < 8 {
        return 0;
    }
    let timing = read_timing(b);
    let fl = b[6] as u8;
    let ntracks = b[7] as usize;
    let mut at = 8;
    let mut specs: Vec<TrackSpec> = Vec::with_capacity(ntracks);
    let take = |n: usize, at: &mut usize| -> Option<Vec<f64>> {
        if *at + n > b.len() {
            return None;
        }
        let v = b[*at..*at + n].to_vec();
        *at += n;
        Some(v)
    };
    for _ in 0..ntracks {
        let Some(h) = take(5, &mut at) else { return 0 };
        let (kind_code, arg, ch, nkf, has_off) = (h[0] as u32, h[1], h[2] as usize, h[3] as usize, h[4] != 0.0);
        let offsets = if has_off {
            match take(nkf, &mut at) {
                Some(v) => v,
                None => return 0,
            }
        } else {
            Vec::new()
        };
        let Some(values) = take(nkf * ch.max(1), &mut at) else { return 0 };
        let Some(ne) = take(1, &mut at) else { return 0 };
        let Some(eases) = take(ne[0] as usize, &mut at) else { return 0 };
        let easings: Vec<u32> = eases.iter().map(|&e| if e < 0.0 { LINEAR } else { e as u32 }).collect();
        let (kind, path) = match kind_code {
            1 => (TrackKind::Color(ColorSpace::from_u8(arg as u8)), None),
            2 => (TrackKind::Discrete, None),
            4 => (TrackKind::Matrix, None),
            3 => {
                let lut = s.luts.get(arg as usize).and_then(|l| l.clone());
                if lut.is_none() {
                    return 0;
                }
                (TrackKind::Path, lut)
            }
            _ => (TrackKind::Number, None),
        };
        specs.push(TrackSpec { kind, channels: ch.max(1), offsets, values, easings, path });
    }
    handle(s.engine.create_anim(timing, &specs, fl))
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ak_group_create(
    delay: f64,
    duration: f64,
    iterations: f64,
    direction: u32,
    end_delay: f64,
    easing: f64,
    flags: u32,
    auto_duration: u32,
) -> u32 {
    let t = read_timing(&[delay, duration, iterations, direction as f64, end_delay, easing]);
    handle(st().engine.create_group(t, auto_duration != 0, flags as u8))
}

#[no_mangle]
pub extern "C" fn ak_callback_create() -> u32 {
    handle(st().engine.create_callback())
}

#[no_mangle]
pub extern "C" fn ak_group_add(group: u32, child: u32, start: f64) -> i32 {
    code(st().engine.group_add(group, child, start))
}

#[no_mangle]
pub extern "C" fn ak_group_remove(child: u32) -> i32 {
    code(st().engine.group_remove(child))
}

/// Spring simulation. f64 buffer: `[pos * ch, vel * ch, target * ch]`.
#[no_mangle]
pub extern "C" fn ak_spring_create(ch: u32, stiffness: f64, damping: f64, mass: f64) -> u32 {
    let s = st();
    let ch = ch as usize;
    if s.f64_buf.len() < ch * 3 {
        return 0;
    }
    let (pos, rest) = s.f64_buf.split_at(ch);
    let (vel, rest) = rest.split_at(ch);
    let target = &rest[..ch];
    handle(s.engine.create_spring(SpringParams::new(stiffness, damping, mass), pos, vel, target))
}

/// Inertia simulation. f64 buffer: `[pos * ch, vel * ch, (target * ch)?, (min * ch)?, (max * ch)?]`.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ak_decay_create(
    ch: u32,
    power: f64,
    time_constant: f64,
    stiffness: f64,
    damping: f64,
    mass: f64,
    has_target: u32,
    has_min: u32,
    has_max: u32,
) -> u32 {
    let s = st();
    let ch = ch as usize;
    let need = ch * (2 + has_target.min(1) + has_min.min(1) + has_max.min(1)) as usize;
    if s.f64_buf.len() < need {
        return 0;
    }
    let b = &s.f64_buf;
    let mut at = ch * 2;
    let mut opt = |flag: u32| {
        if flag != 0 {
            let r = &b[at..at + ch];
            at += ch;
            Some(r)
        } else {
            None
        }
    };
    let target = opt(has_target);
    let min = opt(has_min);
    let max = opt(has_max);
    handle(s.engine.create_decay(
        &b[..ch],
        &b[ch..ch * 2],
        power,
        time_constant,
        target,
        min,
        max,
        SpringParams::new(stiffness, damping, mass),
    ))
}

/// Retarget a simulation; targets in the f64 buffer.
#[no_mangle]
pub extern "C" fn ak_sim_target(h: u32, ch: u32) -> i32 {
    let s = st();
    let ch = (ch as usize).min(s.f64_buf.len());
    let t = s.f64_buf[..ch].to_vec();
    code(s.engine.sim_set_target(h, &t))
}

/// Jump a simulation; f64 buffer `[pos * ch, (vel * ch)?]`.
#[no_mangle]
pub extern "C" fn ak_sim_position(h: u32, ch: u32, has_vel: u32) -> i32 {
    let s = st();
    let ch = ch as usize;
    if s.f64_buf.len() < ch * if has_vel != 0 { 2 } else { 1 } {
        return 3;
    }
    let pos = s.f64_buf[..ch].to_vec();
    let vel = if has_vel != 0 { Some(s.f64_buf[ch..ch * 2].to_vec()) } else { None };
    code(s.engine.sim_set_position(h, &pos, vel.as_deref()))
}

#[no_mangle]
pub extern "C" fn ak_sim_params(h: u32, stiffness: f64, damping: f64, mass: f64) -> i32 {
    code(st().engine.sim_set_params(h, SpringParams::new(stiffness, damping, mass)))
}

#[no_mangle]
pub extern "C" fn ak_sim_rest(h: u32, rest_delta: f64, rest_speed: f64) -> i32 {
    code(st().engine.sim_set_rest(h, rest_delta, rest_speed))
}

#[no_mangle]
pub extern "C" fn ak_sim_velocity_ptr(h: u32) -> *const f64 {
    st().engine.sim_velocity(h).map(|v| v.as_ptr()).unwrap_or(core::ptr::null())
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn ak_play(h: u32) -> i32 {
    code(st().engine.play(h))
}
#[no_mangle]
pub extern "C" fn ak_pause(h: u32) -> i32 {
    code(st().engine.pause(h))
}
#[no_mangle]
pub extern "C" fn ak_seek(h: u32, t: f64) -> i32 {
    code(st().engine.seek(h, t))
}
#[no_mangle]
pub extern "C" fn ak_set_progress(h: u32, p: f64) -> i32 {
    code(st().engine.set_progress(h, p))
}
#[no_mangle]
pub extern "C" fn ak_set_rate(h: u32, r: f64) -> i32 {
    code(st().engine.set_rate(h, r))
}
#[no_mangle]
pub extern "C" fn ak_reverse(h: u32) -> i32 {
    code(st().engine.reverse(h))
}
#[no_mangle]
pub extern "C" fn ak_restart(h: u32) -> i32 {
    code(st().engine.restart(h))
}
#[no_mangle]
pub extern "C" fn ak_release(h: u32) -> i32 {
    code(st().engine.release(h))
}
#[no_mangle]
pub extern "C" fn ak_set_start(h: u32, start: f64) -> i32 {
    code(st().engine.set_start(h, start))
}
#[no_mangle]
pub extern "C" fn ak_set_timing(
    h: u32,
    delay: f64,
    duration: f64,
    iterations: f64,
    direction: u32,
    end_delay: f64,
    easing: f64,
) -> i32 {
    let t = read_timing(&[delay, duration, iterations, direction as f64, end_delay, easing]);
    code(st().engine.set_timing(h, t))
}
#[no_mangle]
pub extern "C" fn ak_track_enabled(h: u32, track: u32, enabled: u32) -> i32 {
    code(st().engine.set_track_enabled(h, track as usize, enabled != 0))
}
#[no_mangle]
pub extern "C" fn ak_clear() {
    st().engine.clear();
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn ak_values_ptr(h: u32) -> *const f64 {
    st().engine.values_ptr(h)
}
#[no_mangle]
pub extern "C" fn ak_values_len(h: u32) -> u32 {
    st().engine.values(h).map(|v| v.len() as u32).unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn ak_alive(h: u32) -> u32 {
    st().engine.is_alive(h) as u32
}
#[no_mangle]
pub extern "C" fn ak_time(h: u32) -> f64 {
    st().engine.time(h).unwrap_or(f64::NAN)
}
#[no_mangle]
pub extern "C" fn ak_duration(h: u32) -> f64 {
    st().engine.duration(h).unwrap_or(f64::NAN)
}
#[no_mangle]
pub extern "C" fn ak_progress(h: u32) -> f64 {
    st().engine.progress(h).unwrap_or(f64::NAN)
}
#[no_mangle]
pub extern "C" fn ak_state(h: u32) -> u32 {
    st().engine.state(h).map(|s| s as u32).unwrap_or(0xFF)
}
#[no_mangle]
pub extern "C" fn ak_iteration(h: u32) -> f64 {
    st().engine.iteration(h).unwrap_or(f64::NAN)
}
#[no_mangle]
pub extern "C" fn ak_rate(h: u32) -> f64 {
    st().engine.rate_of(h).unwrap_or(f64::NAN)
}

// ---------------------------------------------------------------------------
// Frame
// ---------------------------------------------------------------------------

/// Advance by `dt` ms. Returns the number of updated handles.
#[no_mangle]
pub extern "C" fn ak_tick(dt: f64) -> u32 {
    let e = &mut st().engine;
    e.tick(dt);
    e.updated().len() as u32
}
#[no_mangle]
pub extern "C" fn ak_updated_ptr() -> *const u32 {
    st().engine.updated().as_ptr()
}
#[no_mangle]
pub extern "C" fn ak_updated_len() -> u32 {
    st().engine.updated().len() as u32
}
#[no_mangle]
pub extern "C" fn ak_events_ptr() -> *const u32 {
    st().engine.events().as_ptr()
}
/// Number of u32 words in the event list (2 per event).
#[no_mangle]
pub extern "C" fn ak_events_len() -> u32 {
    st().engine.events().len() as u32
}
#[no_mangle]
pub extern "C" fn ak_clear_frame() {
    st().engine.clear_frame();
}
#[no_mangle]
pub extern "C" fn ak_set_lag_smoothing(threshold: f64, adjusted: f64) {
    st().engine.set_lag_smoothing(threshold, adjusted);
}
#[no_mangle]
pub extern "C" fn ak_set_global_rate(r: f64) {
    st().engine.rate = if r.is_finite() { r.max(0.0) } else { 1.0 };
}
#[no_mangle]
pub extern "C" fn ak_active_count() -> u32 {
    st().engine.active_count() as u32
}
#[no_mangle]
pub extern "C" fn ak_node_count() -> u32 {
    st().engine.node_count() as u32
}

// ---------------------------------------------------------------------------
// Colours, paths, morphing, stagger
// ---------------------------------------------------------------------------

/// Parse a CSS colour from the u8 buffer into f64 buffer `[r, g, b, a]`.
/// Returns 1 on success.
#[no_mangle]
pub extern "C" fn ak_color_parse(len: u32) -> u32 {
    match color::parse(input_str(len)) {
        Some(c) => {
            write_f64(&c.to_array());
            1
        }
        None => 0,
    }
}

/// Register a motion-path lookup table built from path data in the u8 buffer.
/// `samples == 0` chooses automatically. Returns the LUT id.
#[no_mangle]
pub extern "C" fn ak_path_lut(len: u32, samples: u32) -> u32 {
    let p = Path::parse(input_str(len));
    let lut = if samples == 0 { PathLut::auto(&p) } else { PathLut::new(&p, samples as usize) };
    let s = st();
    if let Some(i) = s.free_luts.pop() {
        s.luts[i as usize] = Some(lut);
        i
    } else {
        s.luts.push(Some(lut));
        (s.luts.len() - 1) as u32
    }
}

#[no_mangle]
pub extern "C" fn ak_path_lut_free(id: u32) {
    let s = st();
    if let Some(slot) = s.luts.get_mut(id as usize) {
        if slot.take().is_some() {
            s.free_luts.push(id);
        }
    }
}

#[no_mangle]
pub extern "C" fn ak_path_lut_length(id: u32) -> f64 {
    st().luts.get(id as usize).and_then(|l| l.as_ref()).map(|l| l.length).unwrap_or(0.0)
}

/// Sample a LUT at progress `t`; writes `[x, y, angle]` to the f64 buffer.
#[no_mangle]
pub extern "C" fn ak_path_lut_sample(id: u32, t: f64) -> *mut f64 {
    let v = st().luts.get(id as usize).and_then(|l| l.as_ref()).map(|l| l.sample(t)).unwrap_or([0.0; 3]);
    write_f64(&v)
}

/// Total length of the path in the u8 buffer.
#[no_mangle]
pub extern "C" fn ak_path_length(len: u32) -> f64 {
    Path::parse(input_str(len)).length()
}

/// Bounds `[minX, minY, maxX, maxY]` of the path in the u8 buffer (f64 buffer).
#[no_mangle]
pub extern "C" fn ak_path_bounds(len: u32) -> *mut f64 {
    let b = Path::parse(input_str(len)).bounds();
    write_f64(&b)
}

/// Normalise a path to absolute cubic form (string result). Returns length.
#[no_mangle]
pub extern "C" fn ak_path_normalize(len: u32, decimals: u32) -> u32 {
    let p = Path::parse(input_str(len));
    set_out(p.to_svg(decimals))
}

/// Prepare a morph between two paths stored back-to-back in the u8 buffer
/// (`len_a` bytes then `len_b` bytes). Writes to the f64 buffer:
/// `[channels, templateLen, template..., from..., to...]`; returns total length.
#[no_mangle]
pub extern "C" fn ak_morph(len_a: u32, len_b: u32) -> u32 {
    let (a, b) = {
        let s = st();
        let total = (len_a as usize + len_b as usize).min(s.u8_buf.len());
        let la = (len_a as usize).min(total);
        let sa = core::str::from_utf8(&s.u8_buf[..la]).unwrap_or("");
        let sb = core::str::from_utf8(&s.u8_buf[la..total]).unwrap_or("");
        (Path::parse(sa), Path::parse(sb))
    };
    let plan = morph::prepare(&a, &b);
    let tmpl = plan.template_flat();
    let mut out = Vec::with_capacity(2 + tmpl.len() + plan.from.len() * 2);
    out.push(plan.channels() as f64);
    out.push(tmpl.len() as f64);
    out.extend_from_slice(&tmpl);
    out.extend_from_slice(&plan.from);
    out.extend_from_slice(&plan.to);
    write_f64(&out);
    out.len() as u32
}

/// Compute stagger values (written to the f64 buffer).
/// `from`: 0 first, 1 last, 2 center, 3 edges, 4 random(seed = from_value), 5 index(from_value).
/// `total`: NaN to use `each`. `axis`: 0 none, 1 x, 2 y. `cols == 0`: no grid.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ak_stagger(
    count: u32,
    each: f64,
    total: f64,
    start: f64,
    from: u32,
    from_value: f64,
    cols: u32,
    rows: u32,
    axis: u32,
    ease: u32,
) -> *mut f64 {
    let ease_e = if ease == LINEAR {
        Easing::Linear
    } else {
        st().engine.easing_ref(ease).cloned().unwrap_or(Easing::Linear)
    };
    let opts = Stagger {
        each,
        total: if total.is_nan() { None } else { Some(total) },
        start,
        from: match from {
            1 => StaggerFrom::Last,
            2 => StaggerFrom::Center,
            3 => StaggerFrom::Edges,
            4 => StaggerFrom::Random(from_value as u32),
            5 => StaggerFrom::Index(from_value),
            _ => StaggerFrom::First,
        },
        grid: if cols > 0 { Some((cols as usize, rows as usize)) } else { None },
        axis: match axis {
            1 => Some(Axis::X),
            2 => Some(Axis::Y),
            _ => None,
        },
        ease: ease_e,
    };
    let v = stagger(count as usize, &opts);
    write_f64(&v)
}

/// Mix two colours: f64 buffer `[r, g, b, a, r, g, b, a]` → `[r, g, b, a]`.
#[no_mangle]
pub extern "C" fn ak_color_mix(t: f64, space: u32) -> *mut f64 {
    let s = st();
    if s.f64_buf.len() < 8 {
        return ak_f64_buf(8);
    }
    let b = &s.f64_buf;
    let a = color::Rgba::new(b[0], b[1], b[2], b[3]);
    let c = color::Rgba::new(b[4], b[5], b[6], b[7]);
    let m = color::mix(a, c, t, ColorSpace::from_u8(space as u8));
    write_f64(&m.to_array())
}

/// Create an instanced batch from the f64 buffer:
/// `duration, iterations, direction, endDelay, timingEasing(-1), flags, count, channels, nKeyframes,
///  hasOffsets, [offsets], nEasings, easings..., hasDelays, [delays * count], values (count*nkf*ch)`.
/// Returns the handle or 0.
#[no_mangle]
pub extern "C" fn ak_batch_create(len: u32) -> u32 {
    let s = st();
    let b = &s.f64_buf[..(len as usize).min(s.f64_buf.len())];
    if b.len() < 10 {
        return 0;
    }
    let timing = read_timing(&[0.0, b[0], b[1], b[2], b[3], b[4]]);
    let fl = b[5] as u8;
    let (count, ch, nkf) = (b[6] as usize, b[7] as usize, b[8] as usize);
    let mut at = 9;
    let mut take = |n: usize| -> Option<Vec<f64>> {
        if at + n > b.len() {
            return None;
        }
        let v = b[at..at + n].to_vec();
        at += n;
        Some(v)
    };
    let Some(h) = take(1) else { return 0 };
    let offsets = if h[0] != 0.0 { take(nkf) } else { Some(Vec::new()) };
    let Some(offsets) = offsets else { return 0 };
    let Some(ne) = take(1) else { return 0 };
    let Some(eases) = take(ne[0] as usize) else { return 0 };
    let Some(hd) = take(1) else { return 0 };
    let delays = if hd[0] != 0.0 { take(count) } else { Some(Vec::new()) };
    let Some(delays) = delays else { return 0 };
    let Some(values) = take(count * nkf * ch) else { return 0 };
    let spec = anikit_core::BatchSpec {
        count,
        channels: ch,
        offsets,
        easings: eases.iter().map(|&e| if e < 0.0 { LINEAR } else { e as u32 }).collect(),
        values,
        delays,
    };
    handle(s.engine.create_batch(timing, spec, fl))
}

/// Address of a batch's `f32` output buffer.
#[no_mangle]
pub extern "C" fn ak_batch_ptr(h: u32) -> *const f32 {
    st().engine.batch_values(h).map(|v| v.as_ptr()).unwrap_or(core::ptr::null())
}


// ---------------------------------------------------------------------------
// Noise
// ---------------------------------------------------------------------------

fn noise(seed: u32) -> &'static anikit_core::noise::Noise {
    let s = st();
    if s.noise.as_ref().map(|n| n.seed() != seed).unwrap_or(true) {
        s.noise = Some(anikit_core::noise::Noise::new(seed));
    }
    s.noise.as_ref().unwrap_or_else(|| unreachable!())
}

#[no_mangle]
pub extern "C" fn ak_noise2(seed: u32, x: f64, y: f64) -> f64 {
    noise(seed).noise2(x, y)
}
#[no_mangle]
pub extern "C" fn ak_noise3(seed: u32, x: f64, y: f64, z: f64) -> f64 {
    noise(seed).noise3(x, y, z)
}
#[no_mangle]
pub extern "C" fn ak_fbm2(seed: u32, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    noise(seed).fbm2(x, y, octaves, lacunarity, gain)
}
#[no_mangle]
pub extern "C" fn ak_fbm3(seed: u32, x: f64, y: f64, z: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    noise(seed).fbm3(x, y, z, octaves, lacunarity, gain)
}
/// Bulk 3D noise: f64 buffer `[x, y, z] * n` → results written to `[0..n]`.
#[no_mangle]
pub extern "C" fn ak_noise3_bulk(seed: u32, n: u32) -> *mut f64 {
    let nz = noise(seed).clone();
    let s = st();
    let n = (n as usize).min(s.f64_buf.len() / 3);
    for i in 0..n {
        let (x, y, z) = (s.f64_buf[i * 3], s.f64_buf[i * 3 + 1], s.f64_buf[i * 3 + 2]);
        s.f64_buf[i] = nz.noise3(x, y, z);
    }
    s.f64_buf.as_mut_ptr()
}

// ---------------------------------------------------------------------------
// Path operations
// ---------------------------------------------------------------------------

/// Trim the path in the u8 buffer to `[start, end]` (length fractions). String result.
#[no_mangle]
pub extern "C" fn ak_path_trim(len: u32, start: f64, end: f64, decimals: u32) -> u32 {
    let p = Path::parse(input_str(len)).trim(start, end);
    set_out(p.to_svg(decimals))
}

/// Smooth path through `n` points (`[x, y] * n` in the f64 buffer). String result.
#[no_mangle]
pub extern "C" fn ak_path_through(n: u32, curviness: f64, closed: u32, decimals: u32) -> u32 {
    let s = st();
    let n = (n as usize * 2).min(s.f64_buf.len());
    let p = Path::through_points(&s.f64_buf[..n], curviness, closed != 0);
    set_out(p.to_svg(decimals))
}

#[no_mangle]
pub extern "C" fn ak_path_simplify(len: u32, tolerance: f64, smooth: u32, decimals: u32) -> u32 {
    let p = Path::parse(input_str(len)).simplify(tolerance, smooth != 0);
    set_out(p.to_svg(decimals))
}

#[no_mangle]
pub extern "C" fn ak_path_area(len: u32) -> f64 {
    Path::parse(input_str(len)).area()
}

/// Flatten to polylines: f64 buffer `[nSub, (nPts, x, y, ...) * nSub]`; returns its length.
#[no_mangle]
pub extern "C" fn ak_path_flatten(len: u32, tolerance: f64) -> u32 {
    let polys = Path::parse(input_str(len)).flatten(tolerance);
    let mut out = Vec::new();
    out.push(polys.len() as f64);
    for poly in &polys {
        out.push(poly.len() as f64);
        for p in poly {
            out.push(p[0]);
            out.push(p[1]);
        }
    }
    write_f64(&out);
    out.len() as u32
}

/// Closest point on a LUT: `[progress, x, y, distance]` in the f64 buffer.
#[no_mangle]
pub extern "C" fn ak_path_lut_closest(id: u32, x: f64, y: f64) -> *mut f64 {
    let v = st().luts.get(id as usize).and_then(|l| l.as_ref()).map(|l| l.closest(x, y)).unwrap_or([0.0; 4]);
    write_f64(&v)
}

/// `n` evenly spaced points `[x, y] * n` in the f64 buffer.
#[no_mangle]
pub extern "C" fn ak_path_lut_points(id: u32, n: u32) -> *mut f64 {
    let v = st().luts.get(id as usize).and_then(|l| l.as_ref()).map(|l| l.points(n as usize)).unwrap_or_default();
    write_f64(&v)
}

/// Morph through `count` shapes. Byte lengths of each path in the f64 buffer,
/// path data back-to-back in the u8 buffer. Writes
/// `[channels, templateLen, template..., frame0..., frame1..., ...]`; returns length.
#[no_mangle]
pub extern "C" fn ak_morph_many(count: u32) -> u32 {
    let paths: Vec<Path> = {
        let s = st();
        let n = (count as usize).min(s.f64_buf.len());
        let mut at = 0usize;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let l = s.f64_buf[i] as usize;
            let end = (at + l).min(s.u8_buf.len());
            v.push(Path::parse(core::str::from_utf8(&s.u8_buf[at..end]).unwrap_or("")));
            at = end;
        }
        v
    };
    let seq = morph::prepare_many(&paths);
    let tmpl = seq.template_flat();
    let mut out = Vec::with_capacity(2 + tmpl.len() + seq.channels() * seq.frames.len());
    out.push(seq.channels() as f64);
    out.push(tmpl.len() as f64);
    out.extend_from_slice(&tmpl);
    for f in &seq.frames {
        out.extend_from_slice(f);
    }
    write_f64(&out);
    out.len() as u32
}

// ---------------------------------------------------------------------------
// Physics & sampling
// ---------------------------------------------------------------------------

/// Ballistic simulation. f64 buffer `[pos * ch, vel * ch, acc * ch, (min * ch)?, (max * ch)?]`.
#[no_mangle]
pub extern "C" fn ak_physics_create(ch: u32, friction: f64, restitution: f64, life_ms: f64, has_min: u32, has_max: u32) -> u32 {
    let s = st();
    let ch = ch as usize;
    let need = ch * (3 + has_min.min(1) + has_max.min(1)) as usize;
    if s.f64_buf.len() < need {
        return 0;
    }
    let b = &s.f64_buf;
    let mut at = ch * 3;
    let mut opt = |f: u32| {
        if f != 0 {
            let r = &b[at..at + ch];
            at += ch;
            Some(r)
        } else {
            None
        }
    };
    let min = opt(has_min);
    let max = opt(has_max);
    handle(s.engine.create_physics(&b[..ch], &b[ch..ch * 2], &b[ch * 2..ch * 3], friction, min, max, restitution, life_ms))
}

#[no_mangle]
pub extern "C" fn ak_sim_acceleration(h: u32, ch: u32) -> i32 {
    let s = st();
    let ch = (ch as usize).min(s.f64_buf.len());
    let a = s.f64_buf[..ch].to_vec();
    code(s.engine.sim_set_acceleration(h, &a))
}

/// Evaluate an animation at raw iteration progress `p` (timing easing applied)
/// into the f64 buffer without changing its state. Returns the buffer address.
#[no_mangle]
pub extern "C" fn ak_sample(h: u32, p: f64) -> *mut f64 {
    let n = ak_values_len(h);
    let ptr = ak_f64_buf(n);
    let s = st();
    let mut tmp = alloc::vec![0.0; n as usize];
    if s.engine.sample(h, p, &mut tmp).is_ok() {
        s.f64_buf[..n as usize].copy_from_slice(&tmp);
    }
    ptr
}


// ---------------------------------------------------------------------------
// Physics world
// ---------------------------------------------------------------------------

use anikit_core::world::{Attractor, World, WorldConfig};

fn world(id: u32) -> Option<&'static mut World> {
    st().worlds.get_mut(id as usize).and_then(|w| w.as_mut())
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ak_world_create(
    gx: f64, gy: f64, damping: f64, substeps: u32, iterations: u32, restitution: f64, wall_restitution: f64, friction: f64,
) -> u32 {
    let w = World::new(WorldConfig {
        gravity: [gx as f32, gy as f32],
        damping: damping as f32,
        substeps,
        iterations,
        restitution: restitution as f32,
        wall_restitution: wall_restitution as f32,
        friction: friction as f32,
        ..Default::default()
    });
    let s = st();
    if let Some(i) = s.worlds.iter().position(|w| w.is_none()) {
        s.worlds[i] = Some(w);
        i as u32
    } else {
        s.worlds.push(Some(w));
        (s.worlds.len() - 1) as u32
    }
}
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ak_world_config(
    id: u32, gx: f64, gy: f64, damping: f64, substeps: u32, iterations: u32, restitution: f64, wall_restitution: f64, friction: f64,
) {
    if let Some(w) = world(id) {
        let c = &mut w.config;
        c.gravity = [gx as f32, gy as f32];
        c.damping = damping as f32;
        c.substeps = substeps;
        c.iterations = iterations;
        c.restitution = restitution as f32;
        c.wall_restitution = wall_restitution as f32;
        c.friction = friction as f32;
    }
}
#[no_mangle]
pub extern "C" fn ak_world_free(id: u32) {
    if let Some(slot) = st().worlds.get_mut(id as usize) {
        *slot = None;
    }
}
/// Bounds (pass non-finite values to disable).
#[no_mangle]
pub extern "C" fn ak_world_bounds(id: u32, x0: f64, y0: f64, x1: f64, y1: f64) {
    if let Some(w) = world(id) {
        let f = |v: f64, d: f32| if v.is_finite() { v as f32 } else { d };
        w.config.bounds = [f(x0, f32::NEG_INFINITY), f(y0, f32::NEG_INFINITY), f(x1, f32::INFINITY), f(y1, f32::INFINITY)];
    }
}
/// Add `n` bodies from the f64 buffer `[x, y, r, mass] * n`; returns the first index.
#[no_mangle]
pub extern "C" fn ak_world_add(id: u32, n: u32) -> u32 {
    let s = st();
    let n = (n as usize * 4).min(s.f64_buf.len());
    let data: Vec<f32> = s.f64_buf[..n].iter().map(|&v| v as f32).collect();
    world(id).map(|w| w.add_bodies(&data)).unwrap_or(u32::MAX)
}
#[no_mangle]
pub extern "C" fn ak_world_box(id: u32, x0: f64, y0: f64, x1: f64, y1: f64) -> u32 {
    world(id).map(|w| w.add_box(x0 as f32, y0 as f32, x1 as f32, y1 as f32)).unwrap_or(u32::MAX)
}
#[no_mangle]
pub extern "C" fn ak_world_clear_boxes(id: u32) {
    if let Some(w) = world(id) {
        w.clear_boxes();
    }
}
#[no_mangle]
pub extern "C" fn ak_world_constraint(id: u32, a: u32, b: u32, length: f64, stiffness: f64, tear: f64) -> u32 {
    world(id).map(|w| w.add_constraint(a, b, length as f32, stiffness as f32, tear as f32)).unwrap_or(u32::MAX)
}
/// Constraint states into the f64 buffer `[a, b, alive] * n`; returns n.
#[no_mangle]
pub extern "C" fn ak_world_constraints(id: u32) -> u32 {
    let v: Vec<f64> = match world(id) {
        Some(w) => w.constraints().iter().flat_map(|c| [c.a as f64, c.b as f64, c.alive as u8 as f64]).collect(),
        None => return 0,
    };
    write_f64(&v);
    (v.len() / 3) as u32
}
/// Attractors from the f64 buffer `[x, y, strength, radius] * n`.
#[no_mangle]
pub extern "C" fn ak_world_attractors(id: u32, n: u32) {
    let s = st();
    let n = (n as usize).min(s.f64_buf.len() / 4);
    let a: Vec<Attractor> = (0..n)
        .map(|i| {
            let b = &s.f64_buf[i * 4..i * 4 + 4];
            Attractor { x: b[0] as f32, y: b[1] as f32, strength: b[2] as f32, radius: b[3] as f32 }
        })
        .collect();
    if let Some(w) = world(id) {
        w.set_attractors(&a);
    }
}
#[no_mangle]
pub extern "C" fn ak_world_step(id: u32, dt: f64) {
    if let Some(w) = world(id) {
        w.step(dt as f32);
    }
}
/// Address of `[x, y, angle] * n` (f32).
#[no_mangle]
pub extern "C" fn ak_world_ptr(id: u32) -> *const f32 {
    world(id).map(|w| w.output().as_ptr()).unwrap_or(core::ptr::null())
}
#[no_mangle]
pub extern "C" fn ak_world_len(id: u32) -> u32 {
    world(id).map(|w| w.len() as u32).unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn ak_world_set_position(id: u32, i: u32, x: f64, y: f64, keep_velocity: u32) {
    if let Some(w) = world(id) {
        w.set_position(i as usize, x as f32, y as f32, keep_velocity != 0);
    }
}
#[no_mangle]
pub extern "C" fn ak_world_set_velocity(id: u32, i: u32, vx: f64, vy: f64) {
    if let Some(w) = world(id) {
        w.set_velocity(i as usize, vx as f32, vy as f32);
    }
}
#[no_mangle]
pub extern "C" fn ak_world_velocity(id: u32, i: u32) -> *mut f64 {
    let v = world(id).map(|w| w.velocity(i as usize)).unwrap_or([0.0; 2]);
    write_f64(&[v[0] as f64, v[1] as f64])
}
#[no_mangle]
pub extern "C" fn ak_world_set_pinned(id: u32, i: u32, pinned: u32) {
    if let Some(w) = world(id) {
        w.set_pinned(i as usize, pinned != 0);
    }
}
#[no_mangle]
pub extern "C" fn ak_world_set_active(id: u32, i: u32, on: u32) {
    if let Some(w) = world(id) {
        w.set_active(i as usize, on != 0);
    }
}
#[no_mangle]
pub extern "C" fn ak_world_set_radius(id: u32, i: u32, r: f64) {
    if let Some(w) = world(id) {
        w.set_radius(i as usize, r as f32);
    }
}
#[no_mangle]
pub extern "C" fn ak_world_impulse(id: u32, x: f64, y: f64, radius: f64, strength: f64) {
    if let Some(w) = world(id) {
        w.impulse(x as f32, y as f32, radius as f32, strength as f32);
    }
}
#[no_mangle]
pub extern "C" fn ak_world_pick(id: u32, x: f64, y: f64) -> u32 {
    world(id).map(|w| w.pick(x as f32, y as f32)).unwrap_or(u32::MAX)
}
#[no_mangle]
pub extern "C" fn ak_world_energy(id: u32) -> f64 {
    world(id).map(|w| w.kinetic_energy() as f64).unwrap_or(0.0)
}
#[no_mangle]
pub extern "C" fn ak_world_pair_tests(id: u32) -> f64 {
    world(id).map(|w| w.pair_tests as f64).unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// 3D
// ---------------------------------------------------------------------------

use anikit_core::space;

/// Parse a CSS transform list from the u8 buffer into 16 values (f64 buffer). Returns 1 on success.
#[no_mangle]
pub extern "C" fn ak_transform_parse(len: u32) -> u32 {
    match space::parse_transform(input_str(len)) {
        Some(m) => {
            write_f64(&m);
            1
        }
        None => 0,
    }
}
/// Interpolate two matrices (f64 buffer `[a * 16, b * 16]`) like browsers do.
#[no_mangle]
pub extern "C" fn ak_matrix_mix(t: f64) -> *mut f64 {
    let s = st();
    if s.f64_buf.len() < 32 {
        return ak_f64_buf(32);
    }
    let mut a = [0.0; 16];
    let mut b = [0.0; 16];
    a.copy_from_slice(&s.f64_buf[..16]);
    b.copy_from_slice(&s.f64_buf[16..32]);
    write_f64(&space::mix_matrix(&a, &b, t))
}
/// View-projection matrix from a camera → f64 buffer (16 values).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ak_camera(ex: f64, ey: f64, ez: f64, tx: f64, ty: f64, tz: f64, fov: f64, aspect: f64, near: f64, far: f64) -> *mut f64 {
    let view = space::look_at([ex, ey, ez], [tx, ty, tz], [0.0, 1.0, 0.0]);
    let proj = space::perspective(fov, aspect, near, far);
    write_f64(&space::mul(&proj, &view))
}
/// Rotation matrix from Euler angles (degrees, X then Y then Z) → f64 buffer.
#[no_mangle]
pub extern "C" fn ak_rotation(x: f64, y: f64, z: f64) -> *mut f64 {
    write_f64(&space::to_matrix(&space::quat_euler(x, y, z)))
}
#[no_mangle]
pub extern "C" fn ak_projector_create() -> u32 {
    let s = st();
    s.projectors.push(Some(space::Projector::default()));
    s.points.push(Vec::new());
    (s.projectors.len() - 1) as u32
}
#[no_mangle]
pub extern "C" fn ak_projector_free(id: u32) {
    let s = st();
    if let Some(p) = s.projectors.get_mut(id as usize) {
        *p = None;
    }
    if let Some(v) = s.points.get_mut(id as usize) {
        *v = Vec::new();
    }
}
/// A projector-owned f32 buffer for `n * stride` point values the host fills.
#[no_mangle]
pub extern "C" fn ak_projector_points(id: u32, len: u32) -> *mut f32 {
    match st().points.get_mut(id as usize) {
        Some(v) => {
            v.resize(len as usize, 0.0);
            v.as_mut_ptr()
        }
        None => core::ptr::null_mut(),
    }
}
/// Project points. `source`: 0 = batch `handle`, 1 = the projector's own points.
/// f64 buffer: `[model * 16, viewProj * 16]`. Returns the `[sx, sy, depth, scale] * n` f32 address.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ak_project(id: u32, source: u32, handle: u32, stride: u32, ox: u32, oy: u32, oz: u32, n: u32, w: f64, h: f64, sort: u32) -> *const f32 {
    let s = st();
    if s.f64_buf.len() < 32 {
        return core::ptr::null();
    }
    let mut model = [0.0; 16];
    let mut vp = [0.0; 16];
    model.copy_from_slice(&s.f64_buf[..16]);
    vp.copy_from_slice(&s.f64_buf[16..32]);
    let src: &[f32] = if source == 0 {
        match s.engine.batch_values(handle) {
            Ok(v) => v,
            Err(_) => return core::ptr::null(),
        }
    } else {
        match s.points.get(id as usize) {
            Some(v) => v,
            None => return core::ptr::null(),
        }
    };
    let Some(Some(p)) = s.projectors.get_mut(id as usize) else { return core::ptr::null() };
    p.project(src, stride as usize, [ox as usize, oy as usize, oz as usize], n as usize, &model, &vp, w, h, sort != 0);
    p.out.as_ptr()
}
#[no_mangle]
pub extern "C" fn ak_projector_order(id: u32) -> *const u32 {
    st().projectors.get(id as usize).and_then(|p| p.as_ref()).map(|p| p.order.as_ptr()).unwrap_or(core::ptr::null())
}
