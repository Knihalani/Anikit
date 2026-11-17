//! Sample interpolation kernels.
//!
//! Positions are 32.32 fixed point in frames of the padded sample buffer, so
//! phase never drifts and results are identical on every platform.
//!
//! - [`Interp::Cubic`]: 4-point, 3rd-order Hermite (Catmull-Rom).
//! - [`Interp::Sinc`]: 16-tap Kaiser-windowed sinc, 256 phases with linear
//!   interpolation between phases. When a sample is played faster than its
//!   rate (pitched up), the cut-off follows the output Nyquist in
//!   quarter-octave steps up to 4× so transposition does not alias.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// Interpolation quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interp {
    /// 4-point cubic Hermite.
    Cubic,
    /// 16-tap windowed-sinc polyphase with anti-aliasing for transposition.
    Sinc,
}

static INTERP: AtomicU8 = AtomicU8::new(1);

/// Set the interpolation used by voices started from now on (default [`Interp::Sinc`]).
pub fn set_interp(i: Interp) {
    INTERP.store(i as u8, Ordering::Relaxed);
}

/// The interpolation new voices use.
pub fn interp() -> Interp {
    if INTERP.load(Ordering::Relaxed) == 0 {
        Interp::Cubic
    } else {
        Interp::Sinc
    }
}

/// Fixed-point one.
pub const ONE: u64 = 1 << 32;
const INV32: f32 = 1.0 / 4_294_967_296.0;

/// Sinc taps.
pub const TAPS: usize = 16;
const PBITS: u32 = 8;
const PHASES: usize = 1 << PBITS;
/// Number of anti-alias cut-off steps (quarter octaves, 1× … 4×).
pub const BUCKETS: usize = 9;
/// Base cut-off relative to the input Nyquist frequency.
const CUTOFF: f64 = 0.92;
const BETA: f64 = 8.0;

/// Frames left of the read index a kernel touches.
pub const fn left(i: Interp) -> usize {
    match i {
        Interp::Cubic => 1,
        Interp::Sinc => TAPS / 2 - 1,
    }
}
/// Frames right of the read index a kernel touches.
pub const fn right(i: Interp) -> usize {
    match i {
        Interp::Cubic => 2,
        Interp::Sinc => TAPS / 2,
    }
}

/// One cut-off's polyphase table: rows of coefficients and the difference to
/// the next row (for linear interpolation between phases).
pub struct SincTable {
    c: Vec<[f32; TAPS]>,
    d: Vec<[f32; TAPS]>,
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..64 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

fn build(scale: f64) -> SincTable {
    let half = (TAPS / 2) as f64;
    let i0b = bessel_i0(BETA);
    let mut rows = Vec::with_capacity(PHASES + 1);
    for p in 0..=PHASES {
        let x = p as f64 / PHASES as f64;
        let mut row = [0f64; TAPS];
        for (t, r) in row.iter_mut().enumerate() {
            // tap t sits at offset j = t − (TAPS/2 − 1) from the read index
            let u = (t as f64 - (half - 1.0)) - x;
            let w = if u.abs() >= half { 0.0 } else { bessel_i0(BETA * (1.0 - (u / half).powi(2)).sqrt()) / i0b };
            let a = std::f64::consts::PI * u * scale;
            let s = if a.abs() < 1e-12 { 1.0 } else { a.sin() / a };
            *r = s * w;
        }
        let sum: f64 = row.iter().sum();
        rows.push(row.map(|v| v / sum));
    }
    let c: Vec<[f32; TAPS]> = rows.iter().map(|r| r.map(|v| v as f32)).collect();
    let mut d = Vec::with_capacity(PHASES);
    for p in 0..PHASES {
        let mut row = [0f32; TAPS];
        for t in 0..TAPS {
            row[t] = (rows[p + 1][t] - rows[p][t]) as f32;
        }
        d.push(row);
    }
    SincTable { c: c[..PHASES].to_vec(), d }
}

/// The polyphase tables, one per anti-alias bucket.
pub fn tables() -> &'static [SincTable] {
    static T: OnceLock<Vec<SincTable>> = OnceLock::new();
    T.get_or_init(|| (0..BUCKETS).map(|b| build(CUTOFF / (b as f64 / 4.0).exp2())).collect())
}

/// Anti-alias bucket for a phase increment (frames of input per output frame).
#[inline]
pub fn bucket(inc: f64) -> usize {
    if inc <= 1.0 {
        0
    } else {
        ((inc.log2() * 4.0).ceil() as usize).min(BUCKETS - 1)
    }
}

/// Render `out.len()` frames by cubic Hermite interpolation; returns the new position.
#[inline]
pub fn cubic(d: &[f32], mut pos: u64, inc: u64, out: &mut [f32]) -> u64 {
    for o in out.iter_mut() {
        let i = (pos >> 32) as usize;
        let x = (pos as u32) as f32 * INV32;
        let s: &[f32; 4] = d[i - 1..i + 3].try_into().unwrap();
        let (ym1, y0, y1, y2) = (s[0], s[1], s[2], s[3]);
        let c1 = 0.5 * (y1 - ym1);
        let c2 = ym1 - 2.5 * y0 + 2.0 * y1 - 0.5 * y2;
        let c3 = 0.5 * (y2 - ym1) + 1.5 * (y0 - y1);
        *o = ((c3 * x + c2) * x + c1) * x + y0;
        pos = pos.wrapping_add(inc);
    }
    pos
}

/// Render `out.len()` frames with the windowed-sinc table; returns the new position.
#[inline]
pub fn sinc(d: &[f32], tab: &SincTable, mut pos: u64, inc: u64, out: &mut [f32]) -> u64 {
    for o in out.iter_mut() {
        let i = (pos >> 32) as usize;
        let ph = pos as u32;
        let p = (ph >> (32 - PBITS)) as usize;
        let f = (ph << PBITS >> 8) as f32 * (1.0 / 16_777_216.0);
        let s: &[f32; TAPS] = d[i + 1 - TAPS / 2..i + 1 + TAPS / 2].try_into().unwrap();
        let c = &tab.c[p];
        let dd = &tab.d[p];
        let mut a = [0f32; 8];
        for t in (0..TAPS).step_by(8) {
            for l in 0..8 {
                a[l] += s[t + l] * (c[t + l] + f * dd[t + l]);
            }
        }
        *o = ((a[0] + a[4]) + (a[1] + a[5])) + ((a[2] + a[6]) + (a[3] + a[7]));
        pos = pos.wrapping_add(inc);
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_interp_tables_unity_dc() {
        for t in tables() {
            for r in t.c.iter() {
                let s: f32 = r.iter().sum();
                assert!((s - 1.0).abs() < 1e-5);
            }
        }
        assert_eq!(bucket(0.5), 0);
        assert_eq!(bucket(1.0), 0);
        assert_eq!(bucket(1.1), 1);
        assert_eq!(bucket(2.0), 4);
        assert_eq!(bucket(100.0), BUCKETS - 1);
    }

    #[test]
    fn sf2_interp_identity_at_integer_positions() {
        let d: Vec<f32> = (0..64).map(|i| ((i * 7919) % 101) as f32 / 101.0).collect();
        let mut o = [0f32; 16];
        sinc(&d, &tables()[0], 20 * ONE, ONE, &mut o);
        // At integer phase the sinc row is not exactly a delta (cut-off < 1),
        // but cubic must reproduce samples exactly.
        cubic(&d, 20 * ONE, ONE, &mut o);
        for (k, v) in o.iter().enumerate() {
            assert!((v - d[20 + k]).abs() < 1e-6);
        }
    }
}
