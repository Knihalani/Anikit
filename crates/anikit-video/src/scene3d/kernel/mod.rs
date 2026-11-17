//! Render kernels for `scene3d`: the low-level, performance-critical parts
//! (image post-processing and denoising, parallel helpers). Shading
//! semantics live in [`super::trace`]; these kernels only reorganise the work
//! (structure-of-arrays buffers, row-parallel passes, loops written so LLVM
//! vectorises them) without changing the image model.

pub mod par;
pub mod pool;
pub mod post;
pub mod wide;
