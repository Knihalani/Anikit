//! Frame-sized buffers that are reused across frames.
//!
//! A 1080p frame needs a few hundred MB of planes; fresh allocations of that
//! size come straight from the kernel (mmap), and first-touch page faults
//! cost about 0.4 ms per MB, more than the passes that fill them. [`Buf`]
//! returns its storage to a process-wide pool on drop, so the next frame
//! (on any thread) reuses warm pages.
//!
//! Storage is always initialised memory (`u64`s), so a reused buffer holds
//! stale values from an earlier frame, never undefined bytes: callers that
//! don't overwrite every element use [`Buf::filled`].

use std::marker::PhantomData;
use std::sync::Mutex;

/// Pooled storage, by capacity in words.
static POOL: Mutex<Vec<Vec<u64>>> = Mutex::new(Vec::new());
/// Keep at most this many words pooled (2 GiB).
const MAX_WORDS: usize = 1 << 28;

/// Plain data that any bit pattern is valid for (f32, u32, u8 and arrays or
/// `repr(C)` structs of them).
///
/// # Safety
/// Implementors must be `Copy`, have alignment ≤ 8 and no invalid bit patterns.
pub unsafe trait Pod: Copy + Send + Sync + 'static {}
unsafe impl Pod for f32 {}
unsafe impl Pod for u32 {}
unsafe impl Pod for u16 {}
unsafe impl Pod for u8 {}
unsafe impl<T: Pod, const N: usize> Pod for [T; N] {}

pub struct Buf<T: Pod> {
    raw: Vec<u64>,
    len: usize,
    _t: PhantomData<T>,
}

impl<T: Pod> Buf<T> {
    /// `len` elements with unspecified (stale but valid) values.
    pub fn uninit(len: usize) -> Buf<T> {
        const { assert!(std::mem::align_of::<T>() <= 8) };
        let words = (len * std::mem::size_of::<T>()).div_ceil(8).max(1);
        let raw = {
            let mut pool = POOL.lock().unwrap();
            // The smallest pooled buffer that fits (and isn't wastefully large).
            let pick = pool.iter().enumerate().filter(|(_, v)| v.capacity() >= words && v.capacity() <= words * 2 + 4096).min_by_key(|(_, v)| v.capacity()).map(|(i, _)| i);
            pick.map(|i| pool.swap_remove(i))
        };
        let mut raw = raw.unwrap_or_else(|| Vec::with_capacity(words));
        // Grow the initialised length (zeros only for never-used words).
        if raw.len() < words {
            raw.resize(words, 0);
        }
        Buf { raw, len, _t: PhantomData }
    }

    /// `len` copies of `v`.
    pub fn filled(len: usize, v: T) -> Buf<T> {
        let mut b = Buf::uninit(len);
        b.fill(v);
        b
    }
}

impl<T: Pod> Drop for Buf<T> {
    fn drop(&mut self) {
        let raw = std::mem::take(&mut self.raw);
        if raw.capacity() < 4096 {
            return;
        }
        let mut pool = POOL.lock().unwrap();
        let total: usize = pool.iter().map(|v| v.capacity()).sum();
        if total + raw.capacity() <= MAX_WORDS {
            pool.push(raw);
        }
    }
}

impl<T: Pod> std::ops::Deref for Buf<T> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        // SAFETY: `raw` holds at least `len * size_of::<T>()` initialised bytes,
        // aligned to 8 ≥ align_of::<T>(), and every bit pattern is a valid `T`.
        unsafe { std::slice::from_raw_parts(self.raw.as_ptr() as *const T, self.len) }
    }
}

impl<T: Pod> std::ops::DerefMut for Buf<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        // SAFETY: as in `deref`, and we hold the only reference.
        unsafe { std::slice::from_raw_parts_mut(self.raw.as_mut_ptr() as *mut T, self.len) }
    }
}

impl<T: Pod> Clone for Buf<T> {
    fn clone(&self) -> Buf<T> {
        let mut b = Buf::uninit(self.len);
        b.copy_from_slice(self);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_and_keeps_values_valid() {
        let mut a: Buf<[f32; 3]> = Buf::filled(100_000, [1.0, 2.0, 3.0]);
        a[5] = [4.0, 5.0, 6.0];
        assert_eq!(a.len(), 100_000);
        drop(a);
        let b: Buf<u32> = Buf::filled(70_000, 7);
        assert!(b.iter().all(|&v| v == 7));
        let c = b.clone();
        assert_eq!(&c[..10], &[7; 10]);
    }
}
