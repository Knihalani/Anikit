//! # ak264
//!
//! A fast, renderer-aware H.264 encoder in pure Rust.
//!
//! - Annex-B output, Constrained Baseline profile (CAVLC), decodable everywhere.
//! - Constant QP. Intra 16×16 / 4×4, P 16×16 with integer-pel motion, P_Skip.
//! - Renderer hints: a per-macroblock change map (unchanged macroblocks cost almost
//!   nothing) and known layer translations (exact motion vectors).
//! - Chunk-parallel encoding ([`encode_chunks`]) and slice-parallel streaming.
//! - A minimal MP4 muxer ([`mp4::mux`]).

mod bits;
mod cabac;
mod cabac_tables;
mod cavlc;
mod deblock;
mod diff;
mod dsp;
mod encoder;
mod frame;
mod headers;
mod intra;
mod mc;
pub mod mp4;
mod simd;
mod soa;
#[doc(hidden)]
pub mod synth;
mod transform;
mod vlc;

pub use encoder::{Encoder, EncoderConfig, FrameHints, FrameStats, MotionHint};
pub use frame::{YuvBuffer, YuvFrame};

/// Encode `frames` as independent GOP chunks (each starting with an IDR) on all
/// cores, and return one concatenated Annex-B stream. `hints` is either empty or
/// has one entry per frame.
pub fn encode_chunks(cfg: &EncoderConfig, frames: &[YuvFrame], hints: &[FrameHints]) -> Vec<u8> {
    use rayon::prelude::*;
    assert!(hints.is_empty() || hints.len() == frames.len());
    let gop = cfg.gop.max(1) as usize;
    let none = FrameHints::default();
    let parts: Vec<Vec<u8>> = frames
        .par_chunks(gop)
        .enumerate()
        .map(|(ci, chunk)| {
            let mut c = cfg.clone();
            c.threads = 1;
            let mut e = Encoder::with_idr_base(c, ci as u32);
            let mut out = Vec::new();
            for (k, f) in chunk.iter().enumerate() {
                let h = if hints.is_empty() { &none } else { &hints[ci * gop + k] };
                out.extend_from_slice(e.encode(f, h));
            }
            out
        })
        .collect();
    let mut out = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
    for p in parts {
        out.extend_from_slice(&p);
    }
    out
}
