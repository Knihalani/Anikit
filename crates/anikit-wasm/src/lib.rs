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
    clouds: Vec<Option<anikit_core::cloud::Cloud>>,
    splats: Vec<Option<anikit_core::splat::Splat>>,
    videos: Vec<Option<VideoSlot>>,
    emitters: Vec<Option<anikit_core::emitter::Emitter>>,
    audio_in: Vec<f32>,
    audio: anikit_core::audio::Analysis,
    meshes: Vec<Option<anikit_core::raster::Mesh>>,
    rasters: Vec<Option<(anikit_core::raster::Raster, Vec<f32>)>>,
    textures: Vec<Option<anikit_core::raster::Texture>>,
}

struct VideoSlot {
    post: anikit_core::video::PostFx,
    input: Vec<u8>,
    mixer: anikit_core::video::FrameMixer,
    yuv: Vec<u8>,
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
    clouds: Vec::new(),
    splats: Vec::new(),
    videos: Vec::new(),
    emitters: Vec::new(),
    audio_in: Vec::new(),
    audio: anikit_core::audio::Analysis { hop: 0.0, envelope: Vec::new(), flux: Vec::new(), onsets: Vec::new(), bpm: 0.0, beats: Vec::new() },
    meshes: Vec::new(),
    rasters: Vec::new(),
    textures: Vec::new(),
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
/// Project points. `source`: 0 = batch `handle`, 1 = the projector's own points, 2 = cloud `handle`.
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
    } else if source == 2 {
        match s.clouds.get(handle as usize) {
            Some(Some(c)) => &c.pos,
            _ => return core::ptr::null(),
        }
    } else if source == 3 {
        match s.emitters.get(handle as usize) {
            Some(Some(e)) => &e.pos,
            _ => return core::ptr::null(),
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

// ---------------------------------------------------------------------------
// GIF encoding (screenshots / recordings)
// ---------------------------------------------------------------------------

struct GifSlot {
    enc: Option<anikit_core::gif::GifEncoder>,
    frame: Vec<u8>,
    done: Vec<u8>,
}

struct Gifs(UnsafeCell<Vec<GifSlot>>);
// SAFETY: single-threaded wasm, as above.
unsafe impl Sync for Gifs {}
static GIFS: Gifs = Gifs(UnsafeCell::new(Vec::new()));
#[allow(clippy::mut_from_ref)]
fn gifs() -> &'static mut Vec<GifSlot> {
    // SAFETY: single-threaded; no re-entrancy while held.
    unsafe { &mut *GIFS.0.get() }
}

/// Start a GIF of `w × h` pixels (`loop_count` 0 = forever). Returns an id.
#[no_mangle]
pub extern "C" fn ak_gif_new(w: u32, h: u32, loop_count: u32) -> u32 {
    let g = gifs();
    let slot = GifSlot {
        enc: Some(anikit_core::gif::GifEncoder::new(w as u16, h as u16, loop_count as u16)),
        frame: alloc::vec![0; (w * h * 4) as usize],
        done: Vec::new(),
    };
    if let Some(i) = g.iter().position(|s| s.enc.is_none() && s.done.is_empty()) {
        g[i] = slot;
        i as u32
    } else {
        g.push(slot);
        (g.len() - 1) as u32
    }
}
/// RGBA buffer to fill with the next frame (`w * h * 4` bytes).
#[no_mangle]
pub extern "C" fn ak_gif_frame_ptr(id: u32) -> *mut u8 {
    gifs().get_mut(id as usize).map(|s| s.frame.as_mut_ptr()).unwrap_or(core::ptr::null_mut())
}
/// Encode the frame buffer with a delay in hundredths of a second.
#[no_mangle]
pub extern "C" fn ak_gif_add(id: u32, delay_cs: u32) {
    if let Some(s) = gifs().get_mut(id as usize) {
        if let Some(e) = s.enc.as_mut() {
            e.add_frame(&s.frame, delay_cs.min(65535) as u16);
        }
    }
}
/// Finish; returns the byte length (read from `ak_gif_ptr`).
#[no_mangle]
pub extern "C" fn ak_gif_finish(id: u32) -> u32 {
    match gifs().get_mut(id as usize) {
        Some(s) => {
            if let Some(e) = s.enc.take() {
                s.done = e.finish();
                s.frame = Vec::new();
            }
            s.done.len() as u32
        }
        None => 0,
    }
}
#[no_mangle]
pub extern "C" fn ak_gif_ptr(id: u32) -> *const u8 {
    gifs().get(id as usize).map(|s| s.done.as_ptr()).unwrap_or(core::ptr::null())
}
/// Free a finished GIF's memory.
#[no_mangle]
pub extern "C" fn ak_gif_free(id: u32) {
    if let Some(s) = gifs().get_mut(id as usize) {
        s.enc = None;
        s.frame = Vec::new();
        s.done = Vec::new();
    }
}

// ---------------------------------------------------------------------------
// Point clouds + additive splat rendering
// ---------------------------------------------------------------------------

fn cloud(id: u32) -> Option<&'static mut anikit_core::cloud::Cloud> {
    st().clouds.get_mut(id as usize).and_then(|c| c.as_mut())
}

#[no_mangle]
pub extern "C" fn ak_cloud_create(n: u32, shapes: u32, seed: u32) -> u32 {
    let s = st();
    let c = anikit_core::cloud::Cloud::new(n as usize, shapes as usize, seed);
    if let Some(i) = s.clouds.iter().position(|c| c.is_none()) {
        s.clouds[i] = Some(c);
        return i as u32;
    }
    s.clouds.push(Some(c));
    (s.clouds.len() - 1) as u32
}
#[no_mangle]
pub extern "C" fn ak_cloud_free(id: u32) {
    if let Some(c) = st().clouds.get_mut(id as usize) {
        *c = None;
    }
}
/// Writable `[x, y, z] * n` storage for shape `k`.
#[no_mangle]
pub extern "C" fn ak_cloud_shape(id: u32, k: u32) -> *mut f32 {
    cloud(id).map(|c| c.shape_mut(k as usize).as_mut_ptr()).unwrap_or(core::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn ak_cloud_snap(id: u32, k: u32) {
    if let Some(c) = cloud(id) {
        c.snap(k as usize);
    }
}
/// f64 buffer: `[stiffness, damping, spread, noise, swirl, frequency]`.
#[no_mangle]
pub extern "C" fn ak_cloud_params(id: u32) {
    let b = &st().f64_buf;
    if b.len() < 6 {
        return;
    }
    let p = anikit_core::cloud::CloudParams {
        stiffness: b[0] as f32,
        damping: b[1] as f32,
        spread: b[2] as f32,
        noise: b[3] as f32,
        swirl: b[4] as f32,
        frequency: b[5] as f32,
    };
    if let Some(c) = cloud(id) {
        c.params = p;
    }
}
#[no_mangle]
pub extern "C" fn ak_cloud_step(id: u32, dt: f64, from: u32, to: u32, mix: f64) -> *const f32 {
    match cloud(id) {
        Some(c) => {
            c.step(dt as f32, from as usize, to as usize, mix as f32);
            c.pos.as_ptr()
        }
        None => core::ptr::null(),
    }
}
#[no_mangle]
pub extern "C" fn ak_cloud_ptr(id: u32) -> *const f32 {
    cloud(id).map(|c| c.pos.as_ptr()).unwrap_or(core::ptr::null())
}
#[no_mangle]
pub extern "C" fn ak_cloud_explode(id: u32, x: f64, y: f64, z: f64, strength: f64) {
    if let Some(c) = cloud(id) {
        c.explode(x as f32, y as f32, z as f32, strength as f32);
    }
}
#[no_mangle]
pub extern "C" fn ak_cloud_speed(id: u32) -> f64 {
    cloud(id).map(|c| c.mean_speed() as f64).unwrap_or(0.0)
}

fn splat(id: u32) -> Option<&'static mut anikit_core::splat::Splat> {
    st().splats.get_mut(id as usize).and_then(|c| c.as_mut())
}
#[no_mangle]
pub extern "C" fn ak_splat_create(w: u32, h: u32) -> u32 {
    let s = st();
    let sp = anikit_core::splat::Splat::new(w as usize, h as usize);
    if let Some(i) = s.splats.iter().position(|c| c.is_none()) {
        s.splats[i] = Some(sp);
        return i as u32;
    }
    s.splats.push(Some(sp));
    (s.splats.len() - 1) as u32
}
#[no_mangle]
pub extern "C" fn ak_splat_free(id: u32) {
    if let Some(c) = st().splats.get_mut(id as usize) {
        *c = None;
    }
}
#[no_mangle]
pub extern "C" fn ak_splat_resize(id: u32, w: u32, h: u32) {
    if let Some(sp) = splat(id) {
        sp.resize(w as usize, h as usize);
    }
}
/// Writable per-point linear RGB storage (`n * 3` f32).
#[no_mangle]
pub extern "C" fn ak_splat_colors(id: u32, n: u32) -> *mut f32 {
    match splat(id) {
        Some(sp) => {
            sp.colors.resize(n as usize * 3, 1.0);
            sp.colors.as_mut_ptr()
        }
        None => core::ptr::null_mut(),
    }
}
#[no_mangle]
pub extern "C" fn ak_splat_fade(id: u32, k: f64) {
    if let Some(sp) = splat(id) {
        sp.fade(k as f32);
    }
}
/// Splat the last projection of projector `pid`.
/// f64 buffer: `[size, intensity, fogNear, fogFar, scale, lensX, lensY, lensR, lensStrength, maxRadius, tintR?, tintG?, tintB?]`.
#[no_mangle]
pub extern "C" fn ak_splat_draw(id: u32, pid: u32, n: u32) {
    let s = st();
    if s.f64_buf.len() < 10 {
        return;
    }
    let b = &s.f64_buf;
    let p = anikit_core::splat::SplatParams {
        size: b[0] as f32,
        intensity: b[1] as f32,
        fog_near: b[2] as f32,
        fog_far: if b[3] > 0.0 { b[3] as f32 } else { f32::INFINITY },
        scale: b[4] as f32,
        lens: [b[5] as f32, b[6] as f32, b[7] as f32, b[8] as f32],
        max_radius: b[9] as f32,
        tint: if b.len() >= 13 { [b[10] as f32, b[11] as f32, b[12] as f32] } else { [1.0; 3] },
    };
    let Some(Some(proj)) = s.projectors.get(pid as usize) else { return };
    let Some(Some(sp)) = s.splats.get_mut(id as usize) else { return };
    sp.draw(&proj.out, n as usize, &p);
}
/// Tone-map, then keep `keep` of the light for the next frame (trails; 0
/// clears). Returns the RGBA pixel address (`w * h * 4` bytes).
#[no_mangle]
pub extern "C" fn ak_splat_resolve(id: u32, exposure: f64, bg: u32, keep: f64) -> *const u8 {
    match splat(id) {
        Some(sp) => {
            sp.resolve_fade(exposure as f32, bg, keep as f32);
            sp.rgba.as_ptr() as *const u8
        }
        None => core::ptr::null(),
    }
}
/// Rendering mode: 0 = additive light, 1 = subtractive ink.
#[no_mangle]
pub extern "C" fn ak_splat_mode(id: u32, ink: u32) {
    if let Some(sp) = splat(id) {
        sp.ink = ink != 0;
    }
}

// ---------------------------------------------------------------------------
// Video frames: motion-blur accumulation and RGBA → I420 conversion
// ---------------------------------------------------------------------------

fn video(id: u32) -> Option<&'static mut VideoSlot> {
    st().videos.get_mut(id as usize).and_then(|v| v.as_mut())
}
#[no_mangle]
pub extern "C" fn ak_video_create(w: u32, h: u32) -> u32 {
    let (w, h) = (w as usize, h as usize);
    let slot = VideoSlot {
        post: anikit_core::video::PostFx::default(),
        input: alloc::vec![0; w * h * 4],
        mixer: anikit_core::video::FrameMixer::new(w, h),
        yuv: alloc::vec![0; anikit_core::video::i420_len(w, h)],
    };
    let s = st();
    if let Some(i) = s.videos.iter().position(|v| v.is_none()) {
        s.videos[i] = Some(slot);
        return i as u32;
    }
    s.videos.push(Some(slot));
    (s.videos.len() - 1) as u32
}
#[no_mangle]
pub extern "C" fn ak_video_free(id: u32) {
    if let Some(v) = st().videos.get_mut(id as usize) {
        *v = None;
    }
}
/// Writable RGBA input frame (`w * h * 4` bytes).
#[no_mangle]
pub extern "C" fn ak_video_input(id: u32) -> *mut u8 {
    video(id).map(|v| v.input.as_mut_ptr()).unwrap_or(core::ptr::null_mut())
}
/// Accumulate the input frame for motion blur; returns the sample count.
#[no_mangle]
pub extern "C" fn ak_video_add(id: u32) -> u32 {
    match video(id) {
        Some(v) => {
            v.mixer.add(&v.input);
            v.mixer.count
        }
        None => 0,
    }
}
/// Average the accumulated samples into the input frame and reset.
#[no_mangle]
pub extern "C" fn ak_video_resolve(id: u32) {
    if let Some(v) = video(id) {
        if v.mixer.count > 0 {
            let out = v.mixer.resolve();
            v.input.copy_from_slice(out);
            v.mixer.reset();
        }
    }
}
/// Film-look post pass on the input frame. f64 buffer:
/// `[bloomThreshold, bloom, bloomRadius, aberration, vignette, grain]`.
#[no_mangle]
pub extern "C" fn ak_video_post(id: u32, seed: u32, nf: u32) -> u32 {
    // f64 buffer: bloomThreshold, bloom, bloomRadius, aberration, vignette, grain,
    // [6..23) streaks, streakLength, streakTint(3), zoomBlur, zoomCenter(2), exposure,
    // contrast, saturation, shadows(3), highlights(3), [23..27) rays, raysCenter(2), raysLength, [27] aa.
    let b = &st().f64_buf;
    let nf = (nf as usize).min(b.len());
    if nf < 6 {
        return 0;
    }
    let g = |i: usize, d: f32| if i < nf { b[i] as f32 } else { d };
    let d = anikit_core::video::Post::default();
    let p = anikit_core::video::Post {
        bloom_threshold: g(0, d.bloom_threshold),
        bloom: g(1, 0.0),
        bloom_radius: g(2, d.bloom_radius),
        aberration: g(3, 0.0),
        vignette: g(4, 0.0),
        grain: g(5, 0.0),
        seed,
        streaks: g(6, 0.0),
        streak_length: g(7, d.streak_length),
        streak_tint: [g(8, d.streak_tint[0]), g(9, d.streak_tint[1]), g(10, d.streak_tint[2])],
        zoom_blur: g(11, 0.0),
        zoom_center: [g(12, 0.5), g(13, 0.5)],
        exposure: g(14, 1.0),
        contrast: g(15, 1.0),
        saturation: g(16, 1.0),
        shadows: [g(17, 0.0), g(18, 0.0), g(19, 0.0)],
        highlights: [g(20, 0.0), g(21, 0.0), g(22, 0.0)],
        rays: g(23, 0.0),
        rays_center: [g(24, d.rays_center[0]), g(25, d.rays_center[1])],
        rays_length: g(26, d.rays_length),
        aa: g(27, 0.0),
    };
    if let Some(v) = video(id) {
        let (w, h) = (v.mixer.width, v.mixer.height);
        v.post.apply(&mut v.input, w, h, &p);
        return 1;
    }
    0
}
/// Convert the input frame to I420 (`order`: 0 RGBA, 1 BGRA); returns the address.
#[no_mangle]
pub extern "C" fn ak_video_i420(id: u32, order: u32) -> *const u8 {
    match video(id) {
        Some(v) => {
            let o = if order == 1 { anikit_core::video::Order::Bgra } else { anikit_core::video::Order::Rgba };
            anikit_core::video::rgba_to_i420(&v.input, v.mixer.width, v.mixer.height, o, &mut v.yuv);
            v.yuv.as_ptr()
        }
        None => core::ptr::null(),
    }
}

// ---------------------------------------------------------------------------
// Particle emitters
// ---------------------------------------------------------------------------

fn emitter(id: u32) -> Option<&'static mut anikit_core::emitter::Emitter> {
    st().emitters.get_mut(id as usize).and_then(|e| e.as_mut())
}
#[no_mangle]
pub extern "C" fn ak_emitter_create(capacity: u32, seed: u32) -> u32 {
    let e = anikit_core::emitter::Emitter::new(capacity as usize, seed);
    let s = st();
    if let Some(i) = s.emitters.iter().position(|e| e.is_none()) {
        s.emitters[i] = Some(e);
        return i as u32;
    }
    s.emitters.push(Some(e));
    (s.emitters.len() - 1) as u32
}
#[no_mangle]
pub extern "C" fn ak_emitter_free(id: u32) {
    if let Some(e) = st().emitters.get_mut(id as usize) {
        *e = None;
    }
}
/// f64 buffer: `[rate, life, lifeVar, ox, oy, oz, shape, sx, sy, sz, dx, dy, dz, speed, speedVar, spread,
/// gx, gy, gz, drag, turbulence, frequency, ax, ay, az, attract, sizeStart, sizeEnd]`.
#[no_mangle]
pub extern "C" fn ak_emitter_params(id: u32) {
    use anikit_core::emitter::{EmitterParams, SpawnShape};
    let b = &st().f64_buf;
    if b.len() < 28 {
        return;
    }
    let f = |i: usize| b[i] as f32;
    let p = EmitterParams {
        rate: f(0),
        life: f(1),
        life_var: f(2),
        origin: [f(3), f(4), f(5)],
        shape: match b[6] as u32 {
            1 => SpawnShape::Sphere,
            2 => SpawnShape::Box,
            3 => SpawnShape::Ring,
            _ => SpawnShape::Point,
        },
        size: [f(7), f(8), f(9)],
        direction: [f(10), f(11), f(12)],
        speed: f(13),
        speed_var: f(14),
        spread: f(15),
        gravity: [f(16), f(17), f(18)],
        drag: f(19),
        turbulence: f(20),
        frequency: f(21),
        attractor: [f(22), f(23), f(24)],
        attract: f(25),
        size_start: f(26),
        size_end: f(27),
    };
    if let Some(e) = emitter(id) {
        e.params = p;
    }
}
#[no_mangle]
pub extern "C" fn ak_emitter_burst(id: u32, n: u32) {
    if let Some(e) = emitter(id) {
        e.burst(n as usize);
    }
}
/// Step and return the live count.
#[no_mangle]
pub extern "C" fn ak_emitter_step(id: u32, dt: f64) -> u32 {
    match emitter(id) {
        Some(e) => {
            e.step(dt as f32);
            e.len() as u32
        }
        None => 0,
    }
}
/// Colour/size over life; f64 buffer: 4 RGB stops (12 values).
#[no_mangle]
pub extern "C" fn ak_emitter_shade(id: u32) {
    let b = &st().f64_buf;
    if b.len() < 12 {
        return;
    }
    let g: anikit_core::emitter::Gradient = core::array::from_fn(|k| [b[k * 3] as f32, b[k * 3 + 1] as f32, b[k * 3 + 2] as f32]);
    if let Some(e) = emitter(id) {
        e.shade(&g);
    }
}
#[no_mangle]
pub extern "C" fn ak_emitter_len(id: u32) -> u32 {
    emitter(id).map(|e| e.len() as u32).unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn ak_emitter_ptr(id: u32) -> *const f32 {
    emitter(id).map(|e| e.pos.as_ptr()).unwrap_or(core::ptr::null())
}
#[no_mangle]
pub extern "C" fn ak_emitter_colors(id: u32) -> *const f32 {
    emitter(id).map(|e| e.colors.as_ptr()).unwrap_or(core::ptr::null())
}
#[no_mangle]
pub extern "C" fn ak_emitter_sizes(id: u32) -> *const f32 {
    emitter(id).map(|e| e.sizes.as_ptr()).unwrap_or(core::ptr::null())
}
#[no_mangle]
pub extern "C" fn ak_emitter_clear(id: u32) {
    if let Some(e) = emitter(id) {
        e.clear();
    }
}

// ---------------------------------------------------------------------------
// Audio analysis (beats, onsets, tempo, envelope)
// ---------------------------------------------------------------------------

/// Writable mono sample buffer of `n` f32 values.
#[no_mangle]
pub extern "C" fn ak_audio_input(n: u32) -> *mut f32 {
    let s = st();
    s.audio_in.resize(n as usize, 0.0);
    s.audio_in.as_mut_ptr()
}
/// Analyse the input buffer at `rate` Hz; returns the tempo in BPM.
#[no_mangle]
pub extern "C" fn ak_audio_analyze(rate: f64) -> f64 {
    let s = st();
    s.audio = anikit_core::audio::analyze(&s.audio_in, rate as f32);
    s.audio_in = Vec::new();
    s.audio.bpm as f64
}
#[no_mangle]
pub extern "C" fn ak_audio_hop() -> f64 {
    st().audio.hop as f64
}
/// Result series: 0 envelope, 1 flux, 2 onsets, 3 beats.
#[no_mangle]
pub extern "C" fn ak_audio_result(kind: u32) -> *const f32 {
    let a = &st().audio;
    match kind {
        0 => a.envelope.as_ptr(),
        1 => a.flux.as_ptr(),
        2 => a.onsets.as_ptr(),
        _ => a.beats.as_ptr(),
    }
}
#[no_mangle]
pub extern "C" fn ak_audio_result_len(kind: u32) -> u32 {
    let a = &st().audio;
    (match kind {
        0 => a.envelope.len(),
        1 => a.flux.len(),
        2 => a.onsets.len(),
        _ => a.beats.len(),
    }) as u32
}

// ---------------------------------------------------------------------------
// 3D meshes + software rasterizer
// ---------------------------------------------------------------------------

fn push_slot<T>(v: &mut Vec<Option<T>>, x: T) -> u32 {
    if let Some(i) = v.iter().position(|e| e.is_none()) {
        v[i] = Some(x);
        return i as u32;
    }
    v.push(Some(x));
    (v.len() - 1) as u32
}

/// Built-in mesh. 0 cube; 1 sphere [rings, segments]; 2 torus knot tube
/// [p, q, R, r, tubeRadius, segments, sides]; 3 torus [R, r, segments, sides].
#[no_mangle]
pub extern "C" fn ak_mesh_builtin(kind: u32) -> u32 {
    use anikit_core::raster::*;
    let s = st();
    let b = |i: usize| s.f64_buf.get(i).copied().unwrap_or(0.0) as f32;
    let m = match kind {
        1 => sphere((b(0) as usize).max(3), (b(1) as usize).max(3)),
        2 => torus_knot(b(0), b(1), b(2), b(3), b(4), (b(5) as usize).max(8), (b(6) as usize).max(3)),
        3 => torus(b(0), b(1), (b(2) as usize).max(8), (b(3) as usize).max(3)),
        4 => plane((b(0) as usize).max(1), (b(1) as usize).max(1)),
        5 => cylinder((b(0) as usize).max(3), b(1) != 0.0),
        7 => {
            // Open tube: n points, radius, sides, has radii; then xyz * n; then radii * n.
            let n = b(0) as usize;
            let rad = b(3) != 0.0;
            let pts: Vec<f32> = s.f64_buf.get(4..4 + n * 3).map(|v| v.iter().map(|x| *x as f32).collect()).unwrap_or_default();
            let radii: Vec<f32> = if rad { s.f64_buf.get(4 + n * 3..4 + n * 4).map(|v| v.iter().map(|x| *x as f32).collect()).unwrap_or_default() } else { Vec::new() };
            tube_path(&pts, &radii, b(1), (b(2) as usize).max(3))
        }
        6 => {
            // Heightfield: w, h, skip, then w*h heights.
            let (w, h) = ((b(0) as usize).max(2), (b(1) as usize).max(2));
            let hs: Vec<f32> = s.f64_buf.get(3..3 + w * h).map(|v| v.iter().map(|x| *x as f32).collect()).unwrap_or_default();
            heightfield(w, h, &hs, b(2))
        }
        _ => cube(),
    };
    push_slot(&mut s.meshes, m)
}
/// Custom mesh storage: write positions/normals (xyz) and indices, then draw.
#[no_mangle]
pub extern "C" fn ak_mesh_custom(nverts: u32, ntris: u32) -> u32 {
    // Positions, normals and UVs are zeroed: write them through `ak_mesh_ptr` (0, 1, 5) and indices (2).
    let m = anikit_core::raster::Mesh { pos: alloc::vec![0.0; nverts as usize * 3], nrm: alloc::vec![0.0; nverts as usize * 3], uv: alloc::vec![0.0; nverts as usize * 2], idx: alloc::vec![0; ntris as usize * 3] };
    push_slot(&mut st().meshes, m)
}
/// Pointer into a mesh: 0 positions, 1 normals (f32), 2 indices (u32), 5 uvs (f32); 3/4 = vertex/triangle counts.
#[no_mangle]
pub extern "C" fn ak_mesh_ptr(id: u32, which: u32) -> usize {
    match st().meshes.get_mut(id as usize).and_then(|m| m.as_mut()) {
        Some(m) => match which {
            0 => m.pos.as_mut_ptr() as usize,
            1 => m.nrm.as_mut_ptr() as usize,
            2 => m.idx.as_mut_ptr() as usize,
            3 => m.pos.len() / 3,
            4 => m.idx.len() / 3,
            5 => m.uv.as_mut_ptr() as usize,
            _ => 0,
        },
        None => 0,
    }
}
#[no_mangle]
pub extern "C" fn ak_mesh_free(id: u32) {
    if let Some(m) = st().meshes.get_mut(id as usize) {
        *m = None;
    }
}
#[no_mangle]
pub extern "C" fn ak_raster_create(w: u32, h: u32) -> u32 {
    push_slot(&mut st().rasters, (anikit_core::raster::Raster::new(w as usize, h as usize), Vec::new()))
}
#[no_mangle]
pub extern "C" fn ak_raster_free(id: u32) {
    if let Some(r) = st().rasters.get_mut(id as usize) {
        *r = None;
    }
}
#[no_mangle]
pub extern "C" fn ak_raster_resize(id: u32, w: u32, h: u32) {
    if let Some(Some((r, _))) = st().rasters.get_mut(id as usize) {
        r.resize(w as usize, h as usize);
    }
}
#[no_mangle]
pub extern "C" fn ak_raster_clear(id: u32, rgb: u32, alpha: u32) {
    if let Some(Some((r, _))) = st().rasters.get_mut(id as usize) {
        r.clear(rgb, alpha.min(255) as u8);
    }
}
/// Writable instance buffer: `n * 15` f32 (3×4 affine + rgb).
#[no_mangle]
pub extern "C" fn ak_raster_instances(id: u32, n: u32) -> *mut f32 {
    match st().rasters.get_mut(id as usize) {
        Some(Some((_, inst))) => {
            inst.resize(n as usize * anikit_core::raster::INSTANCE_STRIDE, 0.0);
            inst.as_mut_ptr()
        }
        _ => core::ptr::null_mut(),
    }
}
/// Draw `n` instances of `mesh`. The f64 buffer holds `nf` floats:
/// [0..44) viewProj(16), eye(3), material(ambient, diffuse, specular, shininess, rim), key(3),
/// keyColor(3), fill(3), fillColor(3), rimColor(3), fog(2), fogColor(3);
/// optional [44..57) emissive, metal, iridescence, hue, sky(3), horizon(3), ground(3);
/// optional [57..73) lines, lineColor(3), grid(2), lineWidth, pulse, pulseCount, pulsePhase,
/// pulseDir(3), strips(3); optional [73..78) texture id + 1 (0 = none), mode, repeat u, v, gain.
/// Returns triangles rasterised.
#[no_mangle]
pub extern "C" fn ak_raster_draw(id: u32, mesh: u32, n: u32, cull_back: u32, nf: u32) -> u32 {
    use anikit_core::raster::*;
    let s = st();
    let nf = (nf as usize).min(s.f64_buf.len());
    if nf < 44 {
        return 0;
    }
    const DEFAULTS: [f32; 34] = [
        0.0, 0.0, 0.0, 0.0, 0.25, 0.3, 0.55, 1.0, 0.85, 0.7, 0.05, 0.05, 0.07, // emissive … ground
        0.0, 1.0, 1.0, 1.0, 8.0, 8.0, 0.04, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // lines … strips
        0.0, 0.0, 1.0, 1.0, 1.0, // texture id + 1, mode, repeat u, v, gain
    ];
    let nb = if nf >= 78 { 78 } else if nf >= 73 { 73 } else if nf >= 57 { 57 } else { 44 };
    let mut b: Vec<f32> = s.f64_buf[..nb].iter().map(|v| *v as f32).collect();
    b.extend_from_slice(&DEFAULTS[nb - 44..]);
    let vp: M4 = core::array::from_fn(|i| b[i]);
    let eye = [b[16], b[17], b[18]];
    let v3 = |i: usize| [b[i], b[i + 1], b[i + 2]];
    let mat = Material {
        ambient: b[19],
        diffuse: b[20],
        specular: b[21],
        shininess: b[22],
        rim: b[23],
        emissive: b[44],
        metal: b[45],
        iridescence: b[46],
        hue: b[47],
        lines: b[57],
        line_color: v3(58),
        grid: [b[61], b[62]],
        line_width: b[63],
        pulse: b[64],
        pulse_count: b[65],
        pulse_phase: b[66],
        pulse_dir: v3(67),
        tex_mode: b[74] as u8,
        tex_repeat: [b[75], b[76]],
        tex_gain: b[77],
    };
    let light = Lighting { key: v3(24), key_color: v3(27), fill: v3(30), fill_color: v3(33), rim_color: v3(36), fog: [b[39], if b[40] > 0.0 { b[40] } else { f32::INFINITY }], fog_color: v3(41), sky: v3(48), horizon: v3(51), ground: v3(54), strips: v3(70), ground_fog: [0.0; 3] };
    let Some(Some(m)) = s.meshes.get(mesh as usize) else { return 0 };
    let tex = (b[73] as usize).checked_sub(1).and_then(|t| s.textures.get(t)).and_then(|t| t.as_ref());
    let gloss = if nf > 78 { s.f64_buf[78] as f32 } else { 0.5 };
    let ground_fog = if nf > 81 { [s.f64_buf[79] as f32, s.f64_buf[80] as f32, s.f64_buf[81] as f32] } else { [0.0; 3] };
    let light = Lighting { ground_fog, ..light };
    let Some(Some((r, inst))) = s.rasters.get_mut(id as usize) else { return 0 };
    let k = (n as usize * INSTANCE_STRIDE).min(inst.len());
    // Flags: bit 0 back-face culling, bit 1 additive, bit 2 gloss (strength f64[78]), bit 3 flip winding.
    let blend = if cull_back & 2 != 0 {
        Blend::Additive
    } else if cull_back & 4 != 0 {
        Blend::Gloss
    } else {
        Blend::Opaque
    };
    r.gloss = gloss;
    r.flip_winding = cull_back & 8 != 0;
    r.draw_with(m, &inst[..k], &vp, eye, &mat, &light, cull_back & 1 != 0, blend, tex);
    r.drawn as u32
}
/// A texture from `w * h * 4` RGBA bytes in the u8 scratch buffer (mipmapped); returns its id.
#[no_mangle]
pub extern "C" fn ak_texture_create(w: u32, h: u32) -> u32 {
    let s = st();
    let n = (w as usize * h as usize * 4).min(s.u8_buf.len());
    let t = anikit_core::raster::Texture::from_rgba(w as usize, h as usize, &s.u8_buf[..n]);
    push_slot(&mut s.textures, t)
}
#[no_mangle]
pub extern "C" fn ak_texture_free(id: u32) {
    if let Some(t) = st().textures.get_mut(id as usize) {
        *t = None;
    }
}
/// Camera motion blur (see `Raster::motion_blur`). f64 buffer: viewProj(16), prevViewProj(16).
#[no_mangle]
pub extern "C" fn ak_raster_motion_blur(id: u32, shutter: f64, max_px: f64) {
    let s = st();
    if s.f64_buf.len() < 32 {
        return;
    }
    let vp: anikit_core::raster::M4 = core::array::from_fn(|i| s.f64_buf[i] as f32);
    let prev: anikit_core::raster::M4 = core::array::from_fn(|i| s.f64_buf[16 + i] as f32);
    if let Some(Some((r, _))) = s.rasters.get_mut(id as usize) {
        r.motion_blur(&vp, &prev, shutter as f32, max_px as f32);
    }
}
/// Fill the background with the environment (see `Raster::sky`). f64 buffer: viewProj(16),
/// key(3), keyColor(3), sky(3), horizon(3), ground(3), fogColor(3), fog(2).
#[no_mangle]
pub extern "C" fn ak_raster_sky(id: u32, sun: f64) {
    use anikit_core::raster::*;
    let s = st();
    if s.f64_buf.len() < 36 {
        return;
    }
    let b: Vec<f32> = s.f64_buf[..36].iter().map(|v| *v as f32).collect();
    let vp: M4 = core::array::from_fn(|i| b[i]);
    let v3 = |i: usize| [b[i], b[i + 1], b[i + 2]];
    let light = Lighting { key: v3(16), key_color: v3(19), sky: v3(22), horizon: v3(25), ground: v3(28), fog_color: v3(31), fog: [b[34], if b[35] > 0.0 { b[35] } else { f32::INFINITY }], ..Lighting::default() };
    if let Some(Some((r, _))) = s.rasters.get_mut(id as usize) {
        r.sky(&vp, &light, sun as f32);
    }
}
/// Reset the depth buffer only (mirror passes).
#[no_mangle]
pub extern "C" fn ak_raster_clear_depth(id: u32) {
    if let Some(Some((r, _))) = st().rasters.get_mut(id as usize) {
        r.clear_depth();
    }
}
/// Writable point buffer for [`ak_raster_points`]: `n * 6` f32 (xyz, rgb).
#[no_mangle]
pub extern "C" fn ak_raster_point_buf(id: u32, n: u32) -> *mut f32 {
    ak_raster_instances(id, n.saturating_mul(6).div_ceil(anikit_core::raster::INSTANCE_STRIDE as u32).max(1))
}
/// Draw `n` light points from the point buffer. f64 buffer: viewProj(16),
/// prevViewProj(16, ignored unless `has_prev`), eye(3), size, intensity,
/// streak, fogNear, fogFar (0 = no fog).
#[no_mangle]
pub extern "C" fn ak_raster_points(id: u32, n: u32, has_prev: u32) {
    use anikit_core::raster::*;
    let s = st();
    if s.f64_buf.len() < 40 {
        return;
    }
    let b: Vec<f32> = s.f64_buf[..40].iter().map(|v| *v as f32).collect();
    let vp: M4 = core::array::from_fn(|i| b[i]);
    let prev: M4 = core::array::from_fn(|i| b[16 + i]);
    let style = PointStyle { size: b[35], intensity: b[36], streak: b[37], fog: [b[38], if b[39] > 0.0 { b[39] } else { f32::INFINITY }] };
    let Some(Some((r, buf))) = s.rasters.get_mut(id as usize) else { return };
    let n = (n as usize).min(buf.len() / 6);
    let (pos, rgb) = buf[..n * 6].split_at(n * 3);
    r.points(pos, rgb, &vp, if has_prev != 0 { Some(&prev) } else { None }, [b[32], b[33], b[34]], &style);
}
#[no_mangle]
pub extern "C" fn ak_raster_ptr(id: u32) -> *const u32 {
    match st().rasters.get(id as usize) {
        Some(Some((r, _))) => r.color.as_ptr(),
        _ => core::ptr::null(),
    }
}
