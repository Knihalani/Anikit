//! Row-parallel helpers. Inside a rayon worker (the video pipeline renders
//! several frames at once) they run serially, so frames don't fight over
//! the pool; called from outside they spread rows over every core.

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

/// Whether to split work across the pool (not when already on a worker).
#[inline]
pub fn parallel() -> bool {
    rayon::current_thread_index().is_none()
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
