//! Native Node.js addon for anikit: the `ak_*` C-ABI of `anikit-wasm`,
//! compiled for the host CPU and exposed through N-API.
//!
//! The JavaScript layer reads and writes WebAssembly linear memory through
//! byte offsets (`new Float32Array(memory.buffer, ptr, n)`). To keep that
//! contract unchanged, every Rust allocation in this addon comes from one
//! fixed arena (reserved once, committed lazily by the OS), exposed to
//! JavaScript as a single external `ArrayBuffer`; pointers returned to
//! JavaScript are offsets into it, exactly like wasm addresses. The arena
//! never moves, so views into it are never detached.
//!
//! Load it with `process.dlopen` (see `packages/anikit/node/index.mjs`):
//! `exports` holds every `ak_*` function plus `memory: { buffer }`.
#![allow(clippy::missing_safety_doc)]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Arena + allocator
// ---------------------------------------------------------------------------

/// Arena size: the wasm32 address space (offsets fit in a u32), less 64 KiB.
const ARENA: usize = (4usize << 30) - (64 << 10);
/// Granularity of arena growth (the wasm page size).
const PAGE: usize = 64 << 10;

static BASE: AtomicUsize = AtomicUsize::new(0);
static NEXT: AtomicUsize = AtomicUsize::new(PAGE); // offset 0 stays unused: a null offset is never valid

fn base() -> usize {
    let b = BASE.load(Ordering::Acquire);
    if b != 0 {
        return b;
    }
    // SAFETY: an anonymous private mapping; NORESERVE so untouched pages cost nothing.
    let p = unsafe {
        libc::mmap(core::ptr::null_mut(), ARENA, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANON | libc::MAP_NORESERVE, -1, 0)
    };
    if p == libc::MAP_FAILED {
        std::process::abort();
    }
    match BASE.compare_exchange(0, p as usize, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => p as usize,
        Err(other) => {
            // SAFETY: we own `p` and nobody saw it.
            unsafe { libc::munmap(p, ARENA) };
            other
        }
    }
}

struct Arena;

// SAFETY: hands out disjoint, zeroed, page-aligned regions of the arena.
unsafe impl dlmalloc::Allocator for Arena {
    fn alloc(&self, size: usize) -> (*mut u8, usize, u32) {
        let size = (size + PAGE - 1) & !(PAGE - 1);
        let b = base();
        let off = NEXT.fetch_add(size, Ordering::AcqRel);
        if off + size > ARENA {
            return (core::ptr::null_mut(), 0, 0);
        }
        ((b + off) as *mut u8, size, 0)
    }
    fn remap(&self, _ptr: *mut u8, _oldsize: usize, _newsize: usize, _can_move: bool) -> *mut u8 {
        core::ptr::null_mut()
    }
    fn free_part(&self, _ptr: *mut u8, _oldsize: usize, _newsize: usize) -> bool {
        false
    }
    fn free(&self, _ptr: *mut u8, _size: usize) -> bool {
        false
    }
    fn can_release_part(&self, _flags: u32) -> bool {
        false
    }
    fn allocates_zeros(&self) -> bool {
        true
    }
    fn page_size(&self) -> usize {
        PAGE
    }
}

/// dlmalloc over the arena behind a spin lock (allocation is off the hot paths).
struct Heap {
    lock: AtomicBool,
    dl: UnsafeCell<dlmalloc::Dlmalloc<Arena>>,
}
// SAFETY: every access to `dl` holds `lock`.
unsafe impl Sync for Heap {}

impl Heap {
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut dlmalloc::Dlmalloc<Arena>) -> R) -> R {
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        // SAFETY: the lock is held.
        let r = f(unsafe { &mut *self.dl.get() });
        self.lock.store(false, Ordering::Release);
        r
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        self.with(|d| d.malloc(l.size(), l.align()))
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        self.with(|d| d.calloc(l.size(), l.align()))
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        self.with(|d| d.free(p, l.size(), l.align()))
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        self.with(|d| d.realloc(p, l.size(), l.align(), n))
    }
}

#[global_allocator]
static HEAP: Heap = Heap { lock: AtomicBool::new(false), dl: UnsafeCell::new(dlmalloc::Dlmalloc::new_with_allocator(Arena)) };

/// A pointer as an arena offset (what a wasm address would be).
#[inline]
fn offset<T>(p: *const T) -> u32 {
    let (p, b) = (p as usize, base());
    if p >= b && p < b + ARENA {
        (p - b) as u32
    } else {
        0 // dangling pointers of empty buffers: any address works for a zero-length view
    }
}

// ---------------------------------------------------------------------------
// Minimal N-API (symbols resolved from the host process at load time)
// ---------------------------------------------------------------------------

type Env = *mut c_void;
type Value = *mut c_void;
type CbInfo = *mut c_void;
type Status = i32;
type Callback = unsafe extern "C" fn(Env, CbInfo) -> Value;
type Finalize = unsafe extern "C" fn(Env, *mut c_void, *mut c_void);

extern "C" {
    fn napi_get_cb_info(env: Env, info: CbInfo, argc: *mut usize, argv: *mut Value, this: *mut Value, data: *mut *mut c_void) -> Status;
    fn napi_get_value_double(env: Env, v: Value, out: *mut f64) -> Status;
    fn napi_coerce_to_number(env: Env, v: Value, out: *mut Value) -> Status;
    fn napi_create_double(env: Env, v: f64, out: *mut Value) -> Status;
    fn napi_create_int32(env: Env, v: i32, out: *mut Value) -> Status;
    fn napi_create_uint32(env: Env, v: u32, out: *mut Value) -> Status;
    fn napi_get_undefined(env: Env, out: *mut Value) -> Status;
    fn napi_create_object(env: Env, out: *mut Value) -> Status;
    fn napi_create_function(env: Env, name: *const c_char, len: usize, cb: Callback, data: *mut c_void, out: *mut Value) -> Status;
    fn napi_set_named_property(env: Env, obj: Value, name: *const c_char, v: Value) -> Status;
    fn napi_create_external_arraybuffer(env: Env, data: *mut c_void, len: usize, fin: Option<Finalize>, hint: *mut c_void, out: *mut Value) -> Status;
}

/// A number argument, coerced like a wasm parameter (`undefined` → NaN, `true` → 1).
#[inline]
unsafe fn arg(env: Env, v: Value) -> f64 {
    let mut x = f64::NAN;
    if napi_get_value_double(env, v, &mut x) != 0 {
        let mut n: Value = core::ptr::null_mut();
        if napi_coerce_to_number(env, v, &mut n) == 0 {
            napi_get_value_double(env, n, &mut x);
        }
    }
    x
}

/// ECMAScript ToUint32 (what a wasm i32 parameter receives).
#[inline]
fn to_u32(x: f64) -> u32 {
    if !x.is_finite() {
        return 0;
    }
    let t = x.trunc();
    if t.abs() < 9.0e15 {
        (t as i64) as u32
    } else {
        t.rem_euclid(4294967296.0) as u32
    }
}

#[inline]
unsafe fn ret_f64(env: Env, x: f64) -> Value {
    let mut v: Value = core::ptr::null_mut();
    napi_create_double(env, x, &mut v);
    v
}

#[inline]
unsafe fn ret_i32(env: Env, x: i32) -> Value {
    let mut v: Value = core::ptr::null_mut();
    napi_create_int32(env, x, &mut v);
    v
}

/// Pointers and sizes: unsigned (the arena may pass 2 GiB, which wasm memory never did).
#[inline]
unsafe fn ret_u32(env: Env, x: u32) -> Value {
    let mut v: Value = core::ptr::null_mut();
    napi_create_uint32(env, x, &mut v);
    v
}

#[inline]
unsafe fn undefined(env: Env) -> Value {
    let mut v: Value = core::ptr::null_mut();
    napi_get_undefined(env, &mut v);
    v
}

include!(concat!(env!("OUT_DIR"), "/wrappers.rs"));

/// Module entry point (Node looks this symbol up when `process.dlopen` loads the file).
#[no_mangle]
pub unsafe extern "C" fn napi_register_module_v1(env: Env, exports: Value) -> Value {
    // Addresses the core stores in buffers for the host become arena offsets.
    anikit_wasm::HOST_BASE.store(base(), Ordering::Relaxed);
    for (name, cb) in EXPORTS {
        let mut f: Value = core::ptr::null_mut();
        napi_create_function(env, name.as_ptr() as *const c_char, name.len() - 1, *cb, core::ptr::null_mut(), &mut f);
        napi_set_named_property(env, exports, name.as_ptr() as *const c_char, f);
    }
    let mut buf: Value = core::ptr::null_mut();
    napi_create_external_arraybuffer(env, base() as *mut c_void, ARENA, None, core::ptr::null_mut(), &mut buf);
    let mut mem: Value = core::ptr::null_mut();
    napi_create_object(env, &mut mem);
    napi_set_named_property(env, mem, c"buffer".as_ptr(), buf);
    napi_set_named_property(env, exports, c"memory".as_ptr(), mem);
    exports
}
