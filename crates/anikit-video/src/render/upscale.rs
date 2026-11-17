//! Fast bilinear upscale of a (low-res) premultiplied image straight into a
//! frame buffer: used for full-frame backgrounds (mesh / aurora fields,
//! photos) so vello neither clears nor samples them.
//!
//! Separable: each needed source row is interpolated horizontally once into a
//! 16-bit row (value · 256), then every output row is a vertical lerp of two
//! such rows — a straight, vectorised loop over bytes.

use kurbo::Rect;

#[derive(Default, Debug)]
pub struct Upscaler {
    xtab: Vec<(u32, u32, u32)>,
    rows: [Vec<u8>; 2],
    row_ids: [usize; 2],
}

impl Upscaler {
    /// Write `src` (`sw × sh` premultiplied RGBA8), placed at device rect `dev`,
    /// into `out` (`ow × oh` pixels, tightly packed), whose pixel (0,0) is
    /// frame pixel (`ox`, `oy`). Samples are clamped at the image edges.
    #[allow(clippy::too_many_arguments)]
    pub fn run(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], ow: usize, oh: usize, ox: i64, oy: i64) {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: feature checked above.
            return unsafe { self.run_avx2(src, sw, sh, dev, out, ow, oh, ox, oy) };
        }
        self.run_inner(src, sw, sh, dev, out, ow, oh, ox, oy)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    #[allow(clippy::too_many_arguments)]
    unsafe fn run_avx2(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], ow: usize, oh: usize, ox: i64, oy: i64) {
        self.run_inner(src, sw, sh, dev, out, ow, oh, ox, oy)
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn run_inner(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], ow: usize, oh: usize, ox: i64, oy: i64) {
        let (sx, sy) = (sw as f64 / dev.width(), sh as f64 / dev.height());
        let lerp_tab = |o: f64, s: f64, n: usize| {
            let u = (o + 0.5 - 0.0) * s - 0.5;
            let u = u.clamp(0.0, (n - 1) as f64);
            let i0 = u.floor() as u32;
            let i1 = (i0 + 1).min(n as u32 - 1);
            let w = ((u - i0 as f64) * 256.0).round() as u32;
            (i0, i1, w)
        };
        self.xtab.clear();
        self.xtab.extend((0..ow).map(|x| lerp_tab((ox + x as i64) as f64 - dev.x0, sx, sw)));
        for r in &mut self.rows {
            r.resize(ow * 4, 0);
        }
        self.row_ids = [usize::MAX; 2];
        let srow = sw * 4;
        for y in 0..oh {
            let (j0, j1, wy) = lerp_tab((oy + y as i64) as f64 - dev.y0, sy, sh);
            let (j0, j1) = (j0 as usize, j1 as usize);
            // Moving down one source row: the old second row becomes the first.
            if self.row_ids[0] != j0 && self.row_ids[1] == j0 {
                self.rows.swap(0, 1);
                self.row_ids.swap(0, 1);
            }
            for (slot, j) in [(0usize, j0), (1, j1)] {
                if self.row_ids[slot] == j {
                    continue;
                }
                let s = &src[j * srow..(j + 1) * srow];
                let r = &mut self.rows[slot];
                debug_assert!(r.len() >= self.xtab.len() * 4);
                let (sp, rp) = (s.as_ptr(), r.as_mut_ptr());
                for (x, &(i0, i1, w)) in self.xtab.iter().enumerate() {
                    // SAFETY: i0, i1 < sw (clamped in `lerp_tab`); x < ow and r.len() == ow * 4.
                    unsafe {
                        let a = std::ptr::read_unaligned(sp.add(i0 as usize * 4) as *const [u8; 4]);
                        let b = std::ptr::read_unaligned(sp.add(i1 as usize * 4) as *const [u8; 4]);
                        let o: [u8; 4] = std::array::from_fn(|c| ((a[c] as u32 * (256 - w) + b[c] as u32 * w + 128) >> 8) as u8);
                        std::ptr::write_unaligned(rp.add(x * 4) as *mut [u8; 4], o);
                    }
                }
                self.row_ids[slot] = j;
            }
            let (a, b) = (&self.rows[0], &self.rows[1]);
            let d = &mut out[y * ow * 4..(y + 1) * ow * 4];
            let iw = 256 - wy;
            if wy == 0 {
                d.copy_from_slice(a);
            } else {
                let (iw, wy) = (iw as u16, wy as u16);
                for ((d, &a), &b) in d.iter_mut().zip(a.iter()).zip(b.iter()) {
                    *d = ((a as u16 * iw + b as u16 * wy + 128) >> 8) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upscale_flat_and_ramp() {
        // 2×1 image black→white stretched to 8 px: a monotone ramp with clamped ends.
        let src = [0, 0, 0, 255, 255, 255, 255, 255];
        let mut out = vec![0u8; 8 * 2 * 4];
        Upscaler::default().run(&src, 2, 1, Rect::new(0.0, 0.0, 8.0, 2.0), &mut out, 8, 2, 0, 0);
        let r: Vec<u8> = out[..32].chunks(4).map(|p| p[0]).collect();
        assert_eq!(r[0], 0);
        assert_eq!(r[1], 0);
        assert_eq!(r[7], 255);
        assert!(r.windows(2).all(|w| w[0] <= w[1]), "{r:?}");
        assert!(out.chunks(4).all(|p| p[3] == 255));
        assert_eq!(&out[..32], &out[32..]);
    }
}

#[cfg(test)]
mod perf {
    use super::*;
    #[test]
    #[ignore]
    fn upscale_timing() {
        let (sw, sh) = (481usize, 271usize);
        let src: Vec<u8> = (0..sw * sh * 4).map(|i| (i % 251) as u8).collect();
        let mut out = vec![0u8; 1920 * 1080 * 4];
        let mut u = Upscaler::default();
        for _ in 0..3 {
            let t = std::time::Instant::now();
            u.run(&src, sw, sh, Rect::new(0.0, 0.0, 1924.0, 1084.0), &mut out, 1920, 1080, 0, 0);
            eprintln!("upscale {:?}", t.elapsed());
        }
    }
}
