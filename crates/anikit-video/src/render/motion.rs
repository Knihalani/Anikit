//! Motion blur: a camera shutter, by sub-frame accumulation.
//!
//! The pipeline hands the renderer an `eval(t)` closure. The renderer looks
//! at the shutter's open and close: if nothing differs, it renders once.
//! Otherwise it estimates how far things move on screen (matched op bounds)
//! and takes enough evenly spaced sub-frame samples (2–24) that each step
//! moves about 1.5 px. The samples are averaged in linear light, then post
//! runs once on the result.

use super::incremental::{hash_op, op_rects};
use super::{rgba_to_yuv420, FrameRgba, Renderer, YuvFrame};
use crate::dl::DisplayList;

/// Most samples per frame.
pub const MAX_SAMPLES: usize = 24;

/// Most samples per frame at [`super::Quality::Reference`].
pub const REFERENCE_SAMPLES: usize = 96;

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

/// Macroblock-aligned boxes (`[x0, y0, x1, y1)` pixels) outside which every
/// sample is identical to `center`, or `None` when that can't be shown cheaply
/// (layer structure or a layer's own parameters change, the background
/// changes, or the moving area is most of the frame).
fn motion_boxes(center: &DisplayList, samples: &[DisplayList], w: usize, h: usize) -> Option<Vec<[usize; 4]>> {
    use crate::dl::Op;
    let mut rc = Vec::new();
    op_rects(&center.ops, &mut rc);
    let hc: Vec<u64> = center.ops.iter().map(hash_op).collect();
    let mut map = super::MbMap::new(w.div_ceil(16), h.div_ceil(16));
    let mut rs = Vec::new();
    for s in samples {
        if s.ops.len() != center.ops.len() || s.width != center.width || s.height != center.height || s.background != center.background {
            return None;
        }
        op_rects(&s.ops, &mut rs);
        for (i, op) in s.ops.iter().enumerate() {
            if hash_op(op) == hc[i] {
                continue;
            }
            if matches!(op, Op::PushLayer { .. } | Op::PopLayer) {
                return None;
            }
            match (rc[i], rs[i]) {
                (Some(a), Some(b)) => {
                    map.mark_rect(a);
                    map.mark_rect(b);
                }
                (None, None) => {}
                _ => return None,
            }
        }
    }
    let boxes: Vec<[usize; 4]> = super::incremental::cluster(&map, 6)
        .into_iter()
        .map(|b| [b[0] as usize * 16, b[1] as usize * 16, (b[2] as usize * 16).min(w), (b[3] as usize * 16).min(h)])
        .filter(|b| b[2] > b[0] && b[3] > b[1])
        .collect();
    let area: usize = boxes.iter().map(|b| (b[2] - b[0]) * (b[3] - b[1])).sum();
    (area as f64 <= 0.6 * (w * h) as f64).then_some(boxes)
}

impl Renderer {
    /// Render the frame at `t` with a shutter open for `shutter` seconds
    /// (centred on `t`; 180° at 30 fps = 1/60 s). `eval(t)` must return the
    /// display list at time `t`. Static frames render once.
    ///
    /// Only the parts of the frame that move during the shutter are rendered
    /// per sample (clustered macroblock boxes around the ops that differ from
    /// the centre); everything else is the centre frame, exactly. At
    /// [`super::Quality::Reference`] every sample is a whole frame.
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
        let d = displacement(&a, &b, &mut ra, &mut rb);
        let reference = self.quality() == super::Quality::Reference;
        let n = if reference {
            // Ground truth: ~0.5 px steps, at least 8 samples.
            ((d / 0.5).ceil() as usize).clamp(8, REFERENCE_SAMPLES)
        } else {
            ((d / 1.5).ceil() as usize).clamp(2, MAX_SAMPLES)
        };
        let (w, h) = (center.width.max(1) as usize, center.height.max(1) as usize);
        // Sample times spread evenly over the open shutter (centres of n slots).
        let samples: Vec<DisplayList> = (0..n).map(|i| eval(t - shutter * 0.5 + shutter * (i as f32 + 0.5) / n as f32)).collect();
        let boxes = if reference { None } else { motion_boxes(&center, &samples, w, h) };
        let boxes = boxes.unwrap_or_else(|| vec![[0, 0, w, h]]);
        let whole = boxes.len() == 1 && boxes[0] == [0, 0, w, h];
        // Post runs once on the average: switch it off for the samples.
        let post = self.post.take();
        let lin = super::post::to_linear_lut();
        let mut acc = std::mem::take(&mut self.motion_acc);
        acc.clear();
        let len: usize = boxes.iter().map(|b| (b[2] - b[0]) * (b[3] - b[1]) * 3).sum();
        acc.resize(len, 0.0);
        if !whole {
            // Outside the boxes the frame is the centre's.
            self.render(&center);
        }
        for l in &samples {
            if whole {
                self.render(l);
            } else {
                for (k, bx) in boxes.iter().enumerate() {
                    self.render_region(l, Some([bx[0] as u32, bx[1] as u32, bx[2] as u32, bx[3] as u32]), k == 0, None, None);
                }
            }
            let px = self.frame.data_as_u8_slice();
            let mut o = 0;
            for bx in &boxes {
                let bw = bx[2] - bx[0];
                for y in bx[1]..bx[3] {
                    let row = &px[(y * w + bx[0]) * 4..(y * w + bx[2]) * 4];
                    for (a, p) in acc[o..o + bw * 3].chunks_exact_mut(3).zip(row.chunks_exact(4)) {
                        a[0] += lin[p[0] as usize];
                        a[1] += lin[p[1] as usize];
                        a[2] += lin[p[2] as usize];
                    }
                    o += bw * 3;
                }
            }
        }
        // Back to sRGB with dither (averaging creates smooth ramps that would band).
        let inv = 1.0 / n as f32;
        let frame = self.frame.data_as_u8_slice_mut();
        let mut o = 0;
        for bx in &boxes {
            for y in bx[1]..bx[3] {
                for x in bx[0]..bx[2] {
                    let (p, a) = (&mut frame[(y * w + x) * 4..(y * w + x) * 4 + 4], &acc[o..o + 3]);
                    let d = super::post::tri_dither(x as u32, y as u32);
                    for k in 0..3 {
                        let v = super::post::encode_srgb((a[k] * inv).clamp(0.0, 1.0)) + d;
                        p[k] = (v + 0.5).clamp(0.0, 255.0) as u8;
                    }
                    p[3] = 255;
                    o += 3;
                }
            }
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
