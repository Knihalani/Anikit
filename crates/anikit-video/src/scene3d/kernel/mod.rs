//! Render kernels for `scene3d`: the low-level, performance-critical parts
//! (image post-processing and denoising, parallel helpers). Shading
//! semantics live in [`super::trace`]; these kernels only reorganise the work
//! (structure-of-arrays buffers, row-parallel passes, loops written so LLVM
//! vectorises them) without changing the image model.

/// Define a function compiled twice, for AVX2 + FMA and for the baseline,
/// dispatched at run time. The body must not rely on fused multiply-adds
/// (none are formed implicitly), so both versions compute the same bits.
macro_rules! simd_fn {
    ($(#[$m:meta])* $vis:vis fn $name:ident($($arg:ident : $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
        $(#[$m])*
        #[inline]
        $vis fn $name($($arg: $ty),*) $(-> $ret)? {
            #[inline(always)]
            fn imp($($arg: $ty),*) $(-> $ret)? $body
            #[cfg(target_arch = "x86_64")]
            #[target_feature(enable = "avx2,fma")]
            unsafe fn avx($($arg: $ty),*) $(-> $ret)? {
                imp($($arg),*)
            }
            #[cfg(target_arch = "x86_64")]
            if $crate::scene3d::kernel::wide::avx2() {
                // SAFETY: the CPU has AVX2 and FMA.
                return unsafe { avx($($arg),*) };
            }
            imp($($arg),*)
        }
    };
}

pub mod par;
pub mod pool;
pub mod post;
pub mod wide;
