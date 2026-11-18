//! Separable image resampling (tent filter: bilinear when enlarging, area-like
//! when shrinking) of premultiplied RGBA8 into an exact device-pixel grid,
//! including fractional edge coverage. Used to pre-scale static images once.

/// Per-output-sample taps: first source index and fixed-point weights (sum = coverage · 2^14).
struct Taps {
    start: Vec<u32>,
    len: Vec<u16>,
    weights: Vec<i32>,
    offs: Vec<u32>,
}

/// Taps mapping `n_out` device samples onto `n_src` source samples, where the
/// image spans device `[off, off + extent)` (off in 0..1).
fn taps(n_src: usize, n_out: usize, off: f64, extent: f64) -> Taps {
    let scale = n_src as f64 / extent; // source px per device px
    let radius = scale.max(1.0);
    let mut t = Taps { start: Vec::with_capacity(n_out), len: Vec::with_capacity(n_out), weights: Vec::new(), offs: Vec::with_capacity(n_out) };
    let mut tmp: Vec<f64> = Vec::new();
    for i in 0..n_out {
        // Coverage of output pixel [i, i+1) by [off, off+extent).
        let cov = ((i as f64 + 1.0).min(off + extent) - (i as f64).max(off)).clamp(0.0, 1.0);
        let c = ((i as f64 + 0.5).clamp(off, off + extent) - off) * scale - 0.5;
        let lo = (c - radius).floor().max(0.0) as usize;
        let hi = ((c + radius).ceil() as usize).min(n_src - 1);
        tmp.clear();
        let mut sum = 0.0;
        for s in lo..=hi {
            let w = (1.0 - ((s as f64 - c) / radius).abs()).max(0.0);
            tmp.push(w);
            sum += w;
        }
        t.offs.push(t.weights.len() as u32);
        if sum <= 0.0 {
            // Degenerate: nearest sample.
            let s = (c.round().max(0.0) as usize).min(n_src - 1);
            t.start.push(s as u32);
            t.len.push(1);
            t.weights.push((cov * 16384.0).round() as i32);
            continue;
        }
        t.start.push(lo as u32);
        t.len.push(tmp.len() as u16);
        // Normalise with exact integer total (rounding error to the largest tap).
        let total = (cov * 16384.0).round() as i32;
        let base = t.weights.len();
        let mut acc = 0;
        for &w in &tmp {
            let q = (w / sum * cov * 16384.0).round() as i32;
            acc += q;
            t.weights.push(q);
        }
        let imax = (0..tmp.len()).max_by(|&a, &b| tmp[a].total_cmp(&tmp[b])).unwrap_or(0);
        t.weights[base + imax] += total - acc;
    }
    t
}

/// Resample `src` (`sw × sh` premultiplied RGBA8) so that it covers device
/// `[ox, ox + ew) × [oy, oy + eh)` within an output of `ow × oh` pixels.
#[allow(clippy::too_many_arguments)]
pub fn resample(src: &[u8], sw: usize, sh: usize, ox: f64, oy: f64, ew: f64, eh: f64, ow: usize, oh: usize, out: &mut [u8]) {
    debug_assert_eq!(out.len(), ow * oh * 4);
    let tx = taps(sw, ow, ox, ew);
    let ty = taps(sh, oh, oy, eh);
    // Horizontal pass on the source rows the vertical taps need.
    let r0 = *ty.start.iter().min().unwrap_or(&0) as usize;
    let r1 = ty.start.iter().zip(&ty.len).map(|(&s, &l)| s as usize + l as usize).max().unwrap_or(0);
    let mut mid = vec![0u16; (r1 - r0) * ow * 4];
    for y in r0..r1 {
        let row = &src[y * sw * 4..(y + 1) * sw * 4];
        let m = &mut mid[(y - r0) * ow * 4..(y - r0 + 1) * ow * 4];
        for i in 0..ow {
            let (s, l, o) = (tx.start[i] as usize, tx.len[i] as usize, tx.offs[i] as usize);
            let mut acc = [0i32; 4];
            for k in 0..l {
                let w = tx.weights[o + k];
                let p = &row[(s + k) * 4..(s + k) * 4 + 4];
                for c in 0..4 {
                    acc[c] += w * p[c] as i32;
                }
            }
            // Keep 4 extra bits of precision (value · 16).
            for c in 0..4 {
                m[i * 4 + c] = ((acc[c] + (1 << 9)) >> 10).clamp(0, 255 * 16) as u16;
            }
        }
    }
    let mut acc = vec![0i32; ow * 4];
    for j in 0..oh {
        let (s, l, o) = (ty.start[j] as usize, ty.len[j] as usize, ty.offs[j] as usize);
        acc.fill(0);
        for k in 0..l {
            let w = ty.weights[o + k];
            let m = &mid[(s + k - r0) * ow * 4..(s + k - r0 + 1) * ow * 4];
            for (a, &v) in acc.iter_mut().zip(m) {
                *a += w * v as i32;
            }
        }
        for (d, &a) in out[j * ow * 4..(j + 1) * ow * 4].iter_mut().zip(&acc) {
            *d = ((a + (1 << 17)) >> 18).clamp(0, 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_flat_colour() {
        let (w, h) = (5, 4);
        let src: Vec<u8> = (0..w * h * 4).map(|i| (i * 13 % 256) as u8).collect();
        let mut out = vec![0u8; w * h * 4];
        resample(&src, w, h, 0.0, 0.0, w as f64, h as f64, w, h, &mut out);
        assert_eq!(out, src);
        let flat = vec![200u8; 16 * 16 * 4];
        let mut out = vec![0u8; 8 * 7 * 4];
        resample(&flat, 16, 16, 0.25, 0.5, 6.5, 6.0, 8, 7, &mut out);
        assert_eq!(out[(3 * 8 + 3) * 4], 200); // interior
        assert_eq!(out[3 * 4], 100); // top row: half covered
        assert_eq!(out[(3 * 8) * 4], 150); // left column: 3/4 covered
    }
}
