//! Incremental frames: re-render only what changed since this renderer's
//! previous frame.
//!
//! Each op gets a fingerprint (content hash + device bounds grown by the
//! filters around it). Ops are compared index by index with the previous
//! frame; the union of old and new bounds of every op that differs, aligned
//! out to 16×16 macroblocks, is re-rendered and re-converted to YUV. A pixel
//! outside that region is covered by the same ops as before, so it is
//! unchanged. Layer structure changes (a different `PushLayer`/`PopLayer` at
//! an index) fall back to a full frame. The changed-macroblock map handed to
//! the encoder is then exact: the region's macroblocks are compared with the
//! previous YUV.

use std::hash::{Hash, Hasher};

use kurbo::{Affine, PathEl, Rect};

use super::diff::MbMap;
use super::yuv::{convert_region, YuvFrame};
use super::{filter, ops_bounds, Renderer};
use crate::dl::{DisplayList, Filter, Op, Paint, Rgba, Stroke};

#[derive(Clone, Copy, Debug)]
struct Print {
    hash: u64,
    rect: Option<Rect>,
    layer: bool,
}

/// Previous-frame state for [`Renderer::render_yuv_incremental`].
#[derive(Default, Debug)]
pub struct Tracker {
    prev: Vec<Print>,
    cur: Vec<Print>,
    size: (u32, u32),
    bg: [u32; 4],
    valid: bool,
    snap: Vec<u8>,
}

/// What [`Renderer::render_yuv_incremental`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    /// Nothing changed: frame and YUV untouched.
    Unchanged,
    /// Only `[x0, y0, x1, y1)` was re-rendered.
    Partial([u32; 4]),
    /// The whole frame was rendered.
    Full,
}

fn f(h: &mut impl Hasher, v: f32) {
    h.write_u32(v.to_bits());
}

fn hash_rgba(h: &mut impl Hasher, c: &Rgba) {
    for v in c.0 {
        f(h, v);
    }
}

fn hash_affine(h: &mut impl Hasher, a: &[f32; 6]) {
    for &v in a {
        f(h, v);
    }
}

fn hash_path(h: &mut impl Hasher, p: &kurbo::BezPath) {
    h.write_usize(p.elements().len());
    for el in p.elements() {
        let pts: &[kurbo::Point] = match el {
            PathEl::MoveTo(a) => {
                h.write_u8(0);
                std::slice::from_ref(a)
            }
            PathEl::LineTo(a) => {
                h.write_u8(1);
                std::slice::from_ref(a)
            }
            PathEl::QuadTo(a, b) => {
                h.write_u8(2);
                h.write_u64(a.x.to_bits() ^ a.y.to_bits().rotate_left(17));
                std::slice::from_ref(b)
            }
            PathEl::CurveTo(a, b, c) => {
                h.write_u8(3);
                h.write_u64(a.x.to_bits() ^ a.y.to_bits().rotate_left(17));
                h.write_u64(b.x.to_bits() ^ b.y.to_bits().rotate_left(17));
                std::slice::from_ref(c)
            }
            PathEl::ClosePath => {
                h.write_u8(4);
                &[]
            }
        };
        for p in pts {
            h.write_u64(p.x.to_bits());
            h.write_u64(p.y.to_bits());
        }
    }
}

fn hash_paint(h: &mut impl Hasher, p: &Paint) {
    match p {
        Paint::Solid(c) => {
            h.write_u8(0);
            hash_rgba(h, c);
        }
        Paint::Linear { p0, p1, stops } => {
            h.write_u8(1);
            for v in p0.iter().chain(p1) {
                f(h, *v);
            }
            for (o, c) in stops {
                f(h, *o);
                hash_rgba(h, c);
            }
        }
        Paint::Radial { c, r, stops } => {
            h.write_u8(2);
            for v in c.iter().chain(std::iter::once(r)) {
                f(h, *v);
            }
            for (o, c) in stops {
                f(h, *o);
                hash_rgba(h, c);
            }
        }
        Paint::Image { image, transform, opacity } => {
            h.write_u8(3);
            h.write_u32(image.0);
            hash_affine(h, transform);
            f(h, *opacity);
        }
    }
}

fn hash_stroke(h: &mut impl Hasher, s: &Stroke) {
    f(h, s.width);
    h.write_u8(s.cap);
    h.write_u8(s.join);
    f(h, s.miter_limit);
    for &d in &s.dash {
        f(h, d);
    }
    f(h, s.dash_offset);
    f(h, s.trim[0]);
    f(h, s.trim[1]);
}

fn hash_filter(h: &mut impl Hasher, fl: &Filter) {
    match fl {
        Filter::Blur(s) => {
            h.write_u8(0);
            f(h, *s);
        }
        Filter::Shadow { dx, dy, blur, color } => {
            h.write_u8(1);
            f(h, *dx);
            f(h, *dy);
            f(h, *blur);
            hash_rgba(h, color);
        }
        Filter::Glow { radius, intensity, color } => {
            h.write_u8(2);
            f(h, *radius);
            f(h, *intensity);
            hash_rgba(h, color);
        }
        Filter::ColorMatrix(m) => {
            h.write_u8(3);
            for &v in m {
                f(h, v);
            }
        }
    }
}

/// Content hash of one op (images are identified by id: a changed image under
/// the same id must come with [`Renderer::invalidate`]).
pub fn hash_op(op: &Op) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match op {
        Op::Fill { path, paint, rule, transform } => {
            h.write_u8(0);
            hash_path(&mut h, path);
            hash_paint(&mut h, paint);
            h.write_u8(*rule as u8);
            hash_affine(&mut h, transform);
        }
        Op::Stroke { path, paint, stroke, transform } => {
            h.write_u8(1);
            hash_path(&mut h, path);
            hash_paint(&mut h, paint);
            hash_stroke(&mut h, stroke);
            hash_affine(&mut h, transform);
        }
        Op::RoundRect { x, y, w, h: hh, radius, paint, transform } => {
            h.write_u8(2);
            for v in [x, y, w, hh, radius] {
                f(&mut h, *v);
            }
            hash_paint(&mut h, paint);
            hash_affine(&mut h, transform);
        }
        Op::Image { image, dst, opacity, transform } => {
            h.write_u8(3);
            h.write_u32(image.0);
            for v in dst {
                f(&mut h, *v);
            }
            f(&mut h, *opacity);
            hash_affine(&mut h, transform);
        }
        Op::Glyphs { font, size, glyphs, paint, transform } => {
            h.write_u8(4);
            h.write_u32(font.0);
            f(&mut h, *size);
            h.write_usize(glyphs.len());
            for g in glyphs {
                h.write_u16(g.id);
                f(&mut h, g.x);
                f(&mut h, g.y);
            }
            hash_paint(&mut h, paint);
            hash_affine(&mut h, transform);
        }
        Op::PushLayer { opacity, blend, filters, clip, cache_key } => {
            h.write_u8(5);
            f(&mut h, *opacity);
            (*blend as u8).hash(&mut h);
            for fl in filters {
                hash_filter(&mut h, fl);
            }
            if let Some((p, t)) = clip {
                hash_path(&mut h, p);
                hash_affine(&mut h, t);
            }
            cache_key.hash(&mut h);
        }
        Op::PopLayer => h.write_u8(6),
    }
    h.finish()
}

fn fingerprint(list: &DisplayList, out: &mut Vec<Print>) {
    out.clear();
    let mut stack: Vec<[f32; 4]> = Vec::new();
    let mut m = [0f32; 4];
    for op in &list.ops {
        let hash = hash_op(op);
        match op {
            Op::PushLayer { filters, .. } => {
                stack.push(m);
                for fl in filters {
                    let fm = filter::margin(fl);
                    for k in 0..4 {
                        m[k] += fm[k];
                    }
                }
                out.push(Print { hash, rect: None, layer: true });
            }
            Op::PopLayer => {
                m = stack.pop().unwrap_or([0.0; 4]);
                out.push(Print { hash, rect: None, layer: true });
            }
            _ => {
                let rect = ops_bounds(std::slice::from_ref(op), Affine::IDENTITY)
                    .map(|r| Rect::new(r.x0 - m[0] as f64 - 1.0, r.y0 - m[1] as f64 - 1.0, r.x1 + m[2] as f64 + 1.0, r.y1 + m[3] as f64 + 1.0));
                out.push(Print { hash, rect, layer: false });
            }
        }
    }
}

impl Renderer {
    /// Forget the previous frame: the next incremental render is a full one.
    /// Call after replacing image or font data under an existing id.
    pub fn invalidate(&mut self) {
        self.inc.valid = false;
    }

    /// Render `list` and update `yuv`, re-rendering only what changed since
    /// this renderer's previous incremental frame. `yuv` must be the frame
    /// passed to the previous call (anything else forces a full frame).
    /// `changed` receives the exact changed-macroblock map versus that frame.
    pub fn render_yuv_incremental(&mut self, list: &DisplayList, yuv: &mut YuvFrame, changed: &mut MbMap) -> Update {
        let (w, h) = (list.width.max(1), list.height.max(1));
        let bg = list.background.0.map(f32::to_bits);
        let mut inc = std::mem::take(&mut self.inc);
        std::mem::swap(&mut inc.prev, &mut inc.cur);
        fingerprint(list, &mut inc.cur);
        let usable = inc.valid && inc.size == (w, h) && inc.bg == bg && yuv.width == w && yuv.height == h;
        inc.valid = true;
        inc.size = (w, h);
        inc.bg = bg;
        let (mbw, mbh) = (w.div_ceil(16) as usize, h.div_ceil(16) as usize);
        // Dirty rect in pixels, or None for "full".
        let mut dirty: Option<Option<Rect>> = if usable { Some(None) } else { None };
        if let Some(acc) = dirty.as_mut() {
            for i in 0..inc.prev.len().max(inc.cur.len()) {
                let (a, b) = (inc.prev.get(i), inc.cur.get(i));
                if let (Some(a), Some(b)) = (a, b) {
                    if a.hash == b.hash {
                        continue;
                    }
                }
                if a.is_some_and(|p| p.layer) || b.is_some_and(|p| p.layer) {
                    dirty = None;
                    break;
                }
                for r in [a, b].into_iter().flatten().filter_map(|p| p.rect) {
                    *acc = Some(acc.map_or(r, |d: Rect| d.union(r)));
                }
            }
        }
        let region = match dirty {
            Some(None) => {
                changed.reset(mbw, mbh, false);
                self.inc = inc;
                return Update::Unchanged;
            }
            Some(Some(r)) => {
                let x0 = ((r.x0.max(0.0) / 16.0).floor() * 16.0) as u32;
                let y0 = ((r.y0.max(0.0) / 16.0).floor() * 16.0) as u32;
                let x1 = (((r.x1.max(0.0) / 16.0).ceil() * 16.0) as u32).min(w);
                let y1 = (((r.y1.max(0.0) / 16.0).ceil() * 16.0) as u32).min(h);
                if x1 <= x0 || y1 <= y0 {
                    changed.reset(mbw, mbh, false);
                    self.inc = inc;
                    return Update::Unchanged;
                }
                let area = (x1 - x0) as u64 * (y1 - y0) as u64;
                (area * 10 < w as u64 * h as u64 * 6).then_some([x0, y0, x1, y1])
            }
            None => None,
        };
        let had_yuv = usable;
        match region {
            Some(r) => self.render_region(list, Some(r)),
            None => self.render_region(list, None),
        }
        if !had_yuv {
            yuv.resize(w, h);
            convert_region(self.frame.data_as_u8_slice(), w as usize, h as usize, [0, 0, w as usize, h as usize], yuv);
            changed.reset(mbw, mbh, true);
            self.inc = inc;
            return Update::Full;
        }
        // Macroblock range to refresh (padded planes).
        let [x0, y0, x1, y1] = region.unwrap_or([0, 0, w, h]);
        let (mx0, my0) = (x0 as usize / 16, y0 as usize / 16);
        let (mx1, my1) = ((x1 as usize).div_ceil(16), (y1 as usize).div_ceil(16));
        // Snapshot the old macroblocks, convert, then compare.
        let (ys, cs) = (yuv.y_stride, yuv.c_stride);
        let (bw, cbw) = ((mx1 - mx0) * 16, (mx1 - mx0) * 8);
        inc.snap.clear();
        for y in my0 * 16..my1 * 16 {
            inc.snap.extend_from_slice(&yuv.y[y * ys + mx0 * 16..y * ys + mx0 * 16 + bw]);
        }
        for plane in [&yuv.u, &yuv.v] {
            for y in my0 * 8..my1 * 8 {
                inc.snap.extend_from_slice(&plane[y * cs + mx0 * 8..y * cs + mx0 * 8 + cbw]);
            }
        }
        convert_region(self.frame.data_as_u8_slice(), w as usize, h as usize, [x0 as usize, y0 as usize, x1 as usize, y1 as usize], yuv);
        changed.reset(mbw, mbh, false);
        let rows = (my1 - my0) * 16;
        let crow = (my1 - my0) * 8;
        let (sy, su) = (&inc.snap[..rows * bw], &inc.snap[rows * bw..rows * bw + crow * cbw]);
        let sv = &inc.snap[rows * bw + crow * cbw..];
        for my in my0..my1 {
            for mx in mx0..mx1 {
                let ox = (mx - mx0) * 16;
                let mut diff = (0..16).any(|r| {
                    let y = my * 16 + r;
                    sy[(y - my0 * 16) * bw + ox..(y - my0 * 16) * bw + ox + 16] != yuv.y[y * ys + mx * 16..y * ys + mx * 16 + 16]
                });
                if !diff {
                    let oc = (mx - mx0) * 8;
                    diff = (0..8).any(|r| {
                        let y = my * 8 + r;
                        let (a, b) = ((y - my0 * 8) * cbw + oc, y * cs + mx * 8);
                        su[a..a + 8] != yuv.u[b..b + 8] || sv[a..a + 8] != yuv.v[b..b + 8]
                    });
                }
                if diff {
                    changed.set(mx, my);
                }
            }
        }
        self.inc = inc;
        match region {
            Some(r) => Update::Partial(r),
            None => Update::Full,
        }
    }
}
