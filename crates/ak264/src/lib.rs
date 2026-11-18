//! # ak264
//!
//! A fast, renderer-aware H.264 encoder in pure Rust.
//!
//! - Annex-B output: High profile (CABAC with the 8×8 transform, default), Main
//!   (CABAC) or Constrained Baseline (CAVLC).
//! - Constant QP with an optional SSIM floor. Intra 16×16 / 4×4; P 16×16 (opt-in
//!   16×8 / 8×16 / 8×8) with quarter-pel motion and P_Skip; optional non-reference
//!   B pictures (L0 / L1 / bi / spatial direct / B_Skip).
//! - Renderer hints: a per-macroblock change map (unchanged macroblocks cost almost
//!   nothing) and known layer translations (exact motion vectors).
//! - Chunk-parallel encoding ([`encode_chunks`], [`encode_parallel`] fusing a
//!   renderer) and slice-parallel streaming ([`EncoderConfig::threads`]).
//! - A minimal MP4 muxer ([`mp4::mux`]).

#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

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
    let none = FrameHints::default();
    let parts: Vec<Vec<u8>> = chunk_ranges(frames.len(), cfg.gop)
        .into_par_iter()
        .enumerate()
        .map(|(ci, (a, b))| {
            let mut c = cfg.clone();
            c.threads = 1;
            let mut e = Encoder::with_idr_base(c, ci as u32);
            let mut out = Vec::new();
            for t in a..b {
                let h = if hints.is_empty() { &none } else { &hints[t] };
                out.extend_from_slice(e.encode(&frames[t], h));
            }
            out.extend_from_slice(e.flush());
            out
        })
        .collect();
    let mut out = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
    for p in parts {
        out.extend_from_slice(&p);
    }
    out
}

/// Render and encode in parallel: frames are split into GOP chunks, and each
/// worker renders its chunk's frames in order into a reused buffer (via
/// `render(frame_index, &mut buffer, &mut hints)`) and encodes them with its own
/// [`Encoder`]. Returns the concatenated Annex-B stream. This is the streaming
/// pipeline mode: no frame is buffered beyond the one being encoded per worker.
pub fn encode_parallel<F>(cfg: &EncoderConfig, frames: usize, render: F) -> Vec<u8>
where
    F: Fn(usize, &mut YuvBuffer, &mut FrameHints) + Sync,
{
    encode_ranges(cfg, chunk_ranges(frames, cfg.gop), render)
}

/// [`encode_parallel`] over explicit chunk ranges (each starts with an IDR). With
/// ranges that don't depend on the machine, the output is identical on any number
/// of threads.
pub fn encode_ranges<F>(cfg: &EncoderConfig, ranges: Vec<(usize, usize)>, render: F) -> Vec<u8>
where
    F: Fn(usize, &mut YuvBuffer, &mut FrameHints) + Sync,
{
    let skip = vec![false; ranges.len()];
    encode_ranges_select(cfg, ranges, &skip, render).into_iter().flatten().flatten().collect()
}

/// As [`encode_ranges`], but chunks with `skip[i]` are not rendered or encoded
/// (`None`); every other chunk is encoded exactly as [`encode_ranges`] would
/// (same IDR numbering by chunk index), so cached chunks can be spliced back in.
pub fn encode_ranges_select<F>(cfg: &EncoderConfig, ranges: Vec<(usize, usize)>, skip: &[bool], render: F) -> Vec<Option<Vec<u8>>>
where
    F: Fn(usize, &mut YuvBuffer, &mut FrameHints) + Sync,
{
    use rayon::prelude::*;
    ranges
        .into_par_iter()
        .enumerate()
        .map(|(ci, (a, b))| {
            if skip.get(ci).copied().unwrap_or(false) {
                return None;
            }
            let mut c = cfg.clone();
            c.threads = 1;
            let mut e = Encoder::with_idr_base(c, ci as u32);
            let mut buf = YuvBuffer::new(cfg.width, cfg.height);
            let mut hints = FrameHints::default();
            let mut out = Vec::new();
            for t in a..b {
                hints.changed = None;
                hints.motions.clear();
                render(t, &mut buf, &mut hints);
                if t == a {
                    hints.changed = None; // first frame of a chunk is an IDR
                }
                out.extend_from_slice(e.encode(&buf.frame(), &hints));
            }
            out.extend_from_slice(e.flush());
            Some(out)
        })
        .collect()
}

/// Frame ranges of the independently encoded chunks: at most `gop` frames each,
/// and (for short videos) a multiple of the core count so no core idles in the
/// last round. Each chunk starts with an IDR.
pub fn chunk_ranges(frames: usize, gop: u32) -> Vec<(usize, usize)> {
    let gop = gop.max(1) as usize;
    let mut n = frames.div_ceil(gop).max(1);
    let cores = rayon::current_num_threads().max(1);
    if !n.is_multiple_of(cores) && n < 4 * cores {
        n = n.div_ceil(cores) * cores;
    }
    let n = n.min(frames.max(1));
    (0..n).map(|i| (i * frames / n, (i + 1) * frames / n)).filter(|r| r.1 > r.0).collect()
}
