//! Row-parallel helpers. They always split rows over the rayon pool, also
//! when called from inside a rayon task (the video pipeline renders each
//! chunk's frames in one task, often with fewer chunks than cores): nested
//! work is stolen by idle threads, so a frame never runs on one core while
//! the others wait. Results don't depend on the split (rows are independent).

use super::pool::{Buf, Pod};
use rayon::prelude::*;

/// A raw pointer that tasks may write through at disjoint indices.
#[derive(Clone, Copy)]
pub struct Shared<T>(pub *mut T);
// SAFETY: users write disjoint elements only (see `at`).
unsafe impl<T> Send for Shared<T> {}
unsafe impl<T> Sync for Shared<T> {}
impl<T> Shared<T> {
    /// Pointer to element `i`; the caller guarantees no other task touches it.
    #[inline(always)]
    pub fn at(self, i: usize) -> *mut T {
        self.0.wrapping_add(i)
    }
}

/// Whether to split work across the pool (always; kept as a switch for
/// single-threaded debugging via `ANIKIT_SCENE3D_SERIAL=1`).
#[inline]
pub fn parallel() -> bool {
    static S: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    !*S.get_or_init(|| std::env::var_os("ANIKIT_SCENE3D_SERIAL").is_some())
}

/// `f(y, row)` for every `w`-wide row of `buf`. On CPUs with AVX2 the row
/// function is compiled for AVX2 too (same IEEE results: no fused
/// multiply-adds are formed), so its loops vectorise 8 wide.
pub fn rows<T: Send, F: Fn(usize, &mut [T]) + Sync + Send>(buf: &mut [T], w: usize, f: F) {
    if w == 0 {
        return;
    }
    let avx = super::wide::avx2();
    if parallel() {
        // A few rows per task: enough to amortise scheduling, small enough to balance.
        let per = (8192 / w).clamp(1, 16);
        buf.par_chunks_mut(w * per).enumerate().for_each(|(k, chunk)| {
            for (j, row) in chunk.chunks_mut(w).enumerate() {
                call(&f, k * per + j, row, avx);
            }
        });
    } else {
        buf.chunks_mut(w).enumerate().for_each(|(y, r)| call(&f, y, r, avx));
    }
}

#[inline(always)]
fn call<T, F: Fn(usize, &mut [T])>(f: &F, y: usize, row: &mut [T], avx: bool) {
    #[cfg(target_arch = "x86_64")]
    if avx {
        // SAFETY: `avx` says the CPU has AVX2 and FMA.
        return unsafe { call_avx2(f, y, row) };
    }
    let _ = avx;
    f(y, row)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn call_avx2<T, F: Fn(usize, &mut [T])>(f: &F, y: usize, row: &mut [T]) {
    f(y, row)
}

/// A new (pooled) `w × h` buffer with every row produced by `f(y, row)`;
/// rows start out as `T::default()`.
pub fn image<T: Pod + Default>(w: usize, h: usize, f: impl Fn(usize, &mut [T]) + Sync + Send) -> Buf<T> {
    let mut v = Buf::uninit(w * h);
    rows(&mut v, w, |y, row| {
        row.fill(T::default());
        f(y, row)
    });
    v
}

/// Run `f` (which may use rayon) on scene3d's own render pool, from a scoped
/// thread the caller joins. The caller is usually a pipeline worker in the
/// middle of evaluating a frame (frame-scoped thread-locals set, maybe a
/// cache cell being initialised): waiting on its own pool it would steal other
/// frames' tasks and run them on top of that state. Blocked on a plain join it
/// can't. Render-pool threads only ever run scene3d's trace and post work,
/// which touches no evaluator state, so their stealing is harmless. Same
/// thread count as the global pool; results don't depend on scheduling.
pub fn isolated<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    if !parallel() {
        return f();
    }
    static P: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    let pool = P.get_or_init(|| rayon::ThreadPoolBuilder::new().num_threads(rayon::current_num_threads()).thread_name(|i| format!("anikit-render-{i}")).build().expect("scene3d: can't start the render thread pool"));
    std::thread::scope(|s| s.spawn(|| pool.install(f)).join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
}

/// `f(y)` for every row `0..h` (a few rows per task).
pub fn each_row(h: usize, f: impl Fn(usize) + Sync + Send) {
    if parallel() {
        (0..h).into_par_iter().with_min_len(2).for_each(f);
    } else {
        (0..h).for_each(f);
    }
}

/// Map `f` over `0..n` in parallel chunks, keeping order.
pub fn map<T: Pod>(n: usize, f: impl Fn(usize) -> T + Sync + Send) -> Buf<T> {
    let mut v = Buf::uninit(n);
    let chunk = 16384;
    if parallel() {
        v.par_chunks_mut(chunk).enumerate().for_each(|(k, c)| {
            for (j, o) in c.iter_mut().enumerate() {
                *o = f(k * chunk + j);
            }
        });
    } else {
        for (i, o) in v.iter_mut().enumerate() {
            *o = f(i);
        }
    }
    v
}
