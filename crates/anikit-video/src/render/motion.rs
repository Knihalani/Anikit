//! Motion blur: a camera shutter, by sub-frame accumulation.
//!
//! The pipeline hands the renderer an `eval(t)` closure. The renderer looks
//! at the shutter's open and close: if nothing differs, it renders once.
//! Otherwise it estimates how far things move on screen (matched op bounds)
//! and takes enough evenly spaced sub-frame samples (2–32) that each step
//! moves at most 1.25 px; streaks longer than 40 px (at 1080p) are clamped
//! by shortening the effective shutter, so very fast moves never strobe. The samples are averaged in linear light, then post
//! runs once on the result.

use super::incremental::{hash_op, op_rects};
use super::{rgba_to_yuv420, FrameRgba, Renderer, YuvFrame};
use crate::dl::DisplayList;

/// Most samples per frame.
pub const MAX_SAMPLES: usize = 32;
/// Longest streak (px at 1080p): faster motion gets a shorter effective
/// shutter, as motion-graphics renderers clamp blur, so samples stay ≤ 1.25 px apart.
pub const MAX_STREAK: f64 = 40.0;

/// Largest on-screen displacement (px) between two lists, from ops matched by index and kind.
fn displacement(a: &DisplayList, b: &DisplayList, ra: &mut Vec<Option<kurbo::Rect>>, rb: &mut Vec<Option<kurbo::Rect>>) -> f64 {
    op_rects(&a.ops, ra);
    op_rects(&b.ops, rb);
    let mut d: f64 = 0.0;
    for (x, y) in ra.iter().zip(rb.iter()) {
        if let (Some(x), Some(y)) = (x, y) {
            // Edge movement: covers translation, scaling and growth.
            let m = (x.x0 - y.x0).abs().max((x.x1 - y.x1).abs()).max((x.y0 - y.y0).abs()).max((x.y1 - y.y1).abs());
            // Ignore absurd jumps (an op swapped for an unrelated one): they cut, not move.
            if m < 0.5 * (a.width.max(a.height)) as f64 {
                d = d.max(m);
            }
        }
    }
    d
}

impl Renderer {
    /// Render the frame at `t` with a shutter open for `shutter` seconds
    /// (centred on `t`; 180° at 30 fps = 1/60 s). `eval(t)` must return the
    /// display list at time `t`. Static frames render once.
    pub fn render_motion_blur(&mut self, eval: &mut dyn FnMut(f32) -> DisplayList, t: f32, shutter: f32) -> &FrameRgba {
        let center = eval(t);
        if shutter <= 0.0 {
            return self.render(&center);
        }
        let (a, b) = (eval(t - shutter * 0.5), eval(t + shutter * 0.5));
        let same = a.ops.len() == center.ops.len()
            && b.ops.len() == center.ops.len()
            && a.ops.iter().zip(&center.ops).zip(&b.ops).all(|((x, c), y)| {
                let hc = hash_op(c);
                hash_op(x) == hc && hash_op(y) == hc
            });
        if same {
            return self.render(&center);
        }
        let (mut ra, mut rb) = (Vec::new(), Vec::new());
        let mut d = displacement(&a, &b, &mut ra, &mut rb);
        let max_streak = (MAX_STREAK * center.height.max(1) as f64 / 1080.0).max(16.0);
        let mut shutter = shutter;
        if d > max_streak {
            shutter *= (max_streak / d) as f32;
            d = max_streak;
        }
        let n = ((d / 1.25).ceil() as usize).clamp(2, MAX_SAMPLES);
        let (w, h) = (center.width.max(1) as usize, center.height.max(1) as usize);
        // Post runs once on the average: switch it off for the samples.
        let post = self.post.take();
        let lin: Vec<f32> = (0..256).map(|i| super::post::srgb_to_linear(i as f32 / 255.0)).collect();
        let mut acc = std::mem::take(&mut self.motion_acc);
        acc.clear();
        acc.resize(w * h * 3, 0.0);
        for i in 0..n {
            // Sample times spread evenly over the open shutter (centres of n slots).
            let ts = t - shutter * 0.5 + shutter * (i as f32 + 0.5) / n as f32;
            let l = eval(ts);
            let px = self.render(&l).data_as_u8_slice();
            for (a, p) in acc.chunks_exact_mut(3).zip(px.chunks_exact(4)) {
                a[0] += lin[p[0] as usize];
                a[1] += lin[p[1] as usize];
                a[2] += lin[p[2] as usize];
            }
        }
        // Back to sRGB with dither (averaging creates smooth ramps that would band).
        let inv = 1.0 / n as f32;
        let frame = self.frame.data_as_u8_slice_mut();
        for (i, (p, a)) in frame.chunks_exact_mut(4).zip(acc.chunks_exact(3)).enumerate() {
            let (x, y) = ((i % w) as u32, (i / w) as u32);
            let d = super::post::tri_dither(x, y);
            for k in 0..3 {
                let v = super::post::linear_to_srgb((a[k] * inv).clamp(0.0, 1.0)) * 255.0 + d;
                p[k] = (v + 0.5).clamp(0.0, 255.0) as u8;
            }
            p[3] = 255;
        }
        self.motion_acc = acc;
        self.post = post;
        if let Some(p) = &self.post {
            self.pp.apply(self.frame.data_as_u8_slice_mut(), w, h, p, self.post_frame);
        }
        self.invalidate();
        &self.frame
    }

    /// [`Self::render_motion_blur`], converted to YUV 4:2:0.
    pub fn render_yuv_motion_blur(&mut self, eval: &mut dyn FnMut(f32) -> DisplayList, t: f32, shutter: f32, out: &mut YuvFrame) {
        self.render_motion_blur(eval, t, shutter);
        rgba_to_yuv420(&self.frame, out);
    }
}
