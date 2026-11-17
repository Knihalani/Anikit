//! Mixing and mastering DSP. Audio is planar stereo `f32` (`l`, `r`) at any
//! rate (the engine runs at [`crate::RATE`]). Every processor is a struct
//! built with `new(rate, &params)` that implements [`Processor`]: `process`
//! works in place on blocks of any length and never allocates; `reset`
//! clears state. Params are serde structs (camelCase, unknown keys rejected,
//! every field defaulted) so a mixer can build them from compact JSON.
//!
//! - [`biquad`]: RBJ biquads, [`Eq`], [`Svf`] (zero-delay-feedback, for sweeps)
//! - [`comp`]: [`Compressor`] (sidechain, parallel mix), [`Ducker`] (beat-triggered
//!   pumping), [`KeyDucker`] / [`key_duck_gains`] (voice-over ducking)
//! - [`reverb`]: 16-line modulated FDN [`Reverb`] with presets;
//!   [`convolution`]: partitioned FFT [`Convolver`], [`Ir`] (synthetic or from a file)
//! - [`delay`], [`chorus`], [`saturate`] (oversampled), [`widener`], [`transient`]
//! - [`loudness`]: BS.1770-4 / EBU R128 meter with true peak;
//!   [`master`]: the master chain and true-peak [`Limiter`];
//!   [`analysis`]: QA metrics ([`Report`])

// Index loops read more clearly than iterator chains in multi-buffer DSP code.
#![allow(clippy::needless_range_loop)]

pub mod analysis;
pub mod biquad;
pub mod chorus;
pub mod comp;
pub mod convolution;
pub mod delay;
pub mod loudness;
pub mod master;
pub mod oversample;
pub mod reverb;
pub mod saturate;
pub mod transient;
pub mod util;
pub mod widener;

pub use analysis::{analyze, Bands, Report};
pub use biquad::{butterworth_qs, Biquad, Coefs, Eq, EqParams, FilterKind, Svf, SvfMode};
pub use chorus::{Chorus, ChorusMode, ChorusParams};
pub use comp::{key_duck_gains, Compressor, CompressorParams, Detector, Ducker, DuckerParams, KeyDuckParams, KeyDucker};
pub use convolution::{convolve, Convolver, Ir, SynthIrParams};
pub use delay::{note_seconds, Delay, DelayParams};
pub use loudness::{measure, Loudness, LoudnessMeter, TruePeak};
pub use master::{master, master_with, Limiter, LimiterParams, MasterParams, MasterReport};
pub use oversample::{Downsampler2, Upsampler2};
pub use reverb::{Reverb, ReverbParams, REVERB_PRESETS};
pub use saturate::{SatMode, Saturator, SaturatorParams};
pub use transient::{TransientParams, TransientShaper};
pub use util::{db_to_gain, gain_to_db, resample};
pub use widener::{Widener, WidenerParams};

/// A stereo in-place audio processor.
pub trait Processor: Send {
    /// Process `l` and `r` in place (equal lengths, any length). Never allocates.
    fn process(&mut self, l: &mut [f32], r: &mut [f32]);
    /// Clear all internal state (as if freshly constructed).
    fn reset(&mut self);
    /// Processing latency in samples (look-ahead, oversampling filters).
    fn latency(&self) -> usize {
        0
    }
}

/// Offline helper: process whole buffers and remove the processor's latency,
/// so the output lines up sample-for-sample with the input (the tail is
/// flushed through with silence). Allocation-free.
pub fn process_aligned(p: &mut dyn Processor, l: &mut [f32], r: &mut [f32]) {
    let n = l.len().min(r.len());
    let (l, r) = (&mut l[..n], &mut r[..n]);
    let d = p.latency();
    p.process(l, r);
    if d == 0 {
        return;
    }
    let keep = n.saturating_sub(d);
    l.copy_within(n - keep.., 0);
    r.copy_within(n - keep.., 0);
    // Flush `d` samples of silence; the first `d − (n − keep)` outputs belong to
    // before the buffer start when d > n (discard them).
    let mut skip = d.saturating_sub(n);
    let mut w = keep;
    let mut zl = [0.0f32; 256];
    let mut zr = [0.0f32; 256];
    let mut left = d;
    while left > 0 {
        let m = left.min(256);
        zl[..m].iter_mut().for_each(|x| *x = 0.0);
        zr[..m].iter_mut().for_each(|x| *x = 0.0);
        p.process(&mut zl[..m], &mut zr[..m]);
        let s = skip.min(m);
        skip -= s;
        let take = (m - s).min(n - w);
        l[w..w + take].copy_from_slice(&zl[s..s + take]);
        r[w..w + take].copy_from_slice(&zr[s..s + take]);
        w += take;
        left -= m;
    }
}

/// Mix `src` into `dst` with gain `g`.
#[inline]
pub fn mix_into(dst: &mut [f32], src: &[f32], g: f32) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d += s * g;
    }
}
