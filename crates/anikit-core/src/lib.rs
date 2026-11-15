//! # anikit-core
//!
//! The engine behind Anikit: a `no_std`, allocation-light animation runtime
//! shared by the WebAssembly (TypeScript) and native (Python) bindings.
//!
//! * [`easing`] — every common easing + CSS/GSAP/Penner syntaxes + springs
//! * [`spring`] — exact closed-form damped oscillators
//! * [`color`] — CSS colour parsing and OKLab/OKLCH interpolation
//! * [`path`] / [`morph`] — SVG paths, motion paths, shape morphing
//! * [`timing`] — WAAPI-compatible timing model
//! * [`engine`] — timelines, keyframes, physics simulations, events
//! * [`stagger`] — 1D/2D stagger distributions
//! * [`css`] — export easings to native CSS (`linear()`)
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod bezier;
pub mod cloud;
pub mod color;
pub mod css;
pub mod easing;
pub mod engine;
pub mod gif;
pub mod math;
pub mod morph;
pub mod noise;
pub mod path;
pub mod space;
pub mod splat;
pub mod spring;
pub mod stagger;
pub mod timing;
pub mod util;
pub mod world;

pub use easing::Easing;
pub use engine::{BatchSpec, Engine, Handle, TrackKind, TrackSpec};
pub use spring::SpringParams;
pub use timing::{Direction, Timing};
