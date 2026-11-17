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
    map: MbMap,
    spare: YuvFrame,
    rects: Vec<Option<Rect>>,
}

/// What [`Renderer::render_yuv_incremental`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    /// Nothing changed: frame and YUV untouched.
    Unchanged,
    /// Only `regions` rectangles (bounding box `bbox`, `[x0, y0, x1, y1)`) were re-rendered.
    Partial { regions: usize, bbox: [u32; 4] },
    /// The whole frame was rendered.
    Full,
}

/// Small, fast non-cryptographic hasher (Fx-style multiply-rotate).
#[derive(Default)]
pub struct Fx(u64);

impl Hasher for Fx {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for c in bytes.chunks(8) {
            let mut b = [0u8; 8];
            b[..c.len()].copy_from_slice(c);
            self.write_u64(u64::from_le_bytes(b));
        }
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    #[inline]
    fn write_u32(&mut self, v: u32) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u16(&mut self, v: u16) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u8(&mut self, v: u8) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
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

/// Content hash of one op. Images are identified by id; a changed image in the
/// shared store must come with [`Renderer::invalidate`] (per-frame images set
/// with [`Renderer::set_frame_image`] are tracked automatically).
pub fn hash_op(op: &Op) -> u64 {
    hash_op_with(op, &|_| 0)
}

fn hash_op_with(op: &Op, image_gen: &dyn Fn(crate::dl::ImageId) -> u64) -> u64 {
    let mut h = Fx::default();
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
            h.write_u64(image_gen(*image));
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
    let paint = match op {
        Op::Fill { paint, .. } | Op::Stroke { paint, .. } | Op::RoundRect { paint, .. } | Op::Glyphs { paint, .. } => Some(paint),
        _ => None,
    };
    if let Some(Paint::Image { image, .. }) = paint {
        h.write_u64(image_gen(*image));
    }
    h.finish()
}

/// Automatic cache key for a filtered layer: its content, filters and
/// frame-space transform. `None` when the layer draws images (their pixels can
/// change under the same id).
pub(crate) fn auto_layer_key(inner: &[Op], filters: &[Filter], frame_base: &kurbo::Affine) -> Option<u64> {
    let mut h = Fx::default();
    h.write_u64(0x6175_746f_6b65_7900); // "autokey"
    for c in frame_base.as_coeffs() {
        h.write_u64(c.to_bits());
    }
    for fl in filters {
        hash_filter(&mut h, fl);
    }
    for op in inner {
        let image = match op {
            Op::Image { .. } => true,
            Op::Fill { paint, .. } | Op::Stroke { paint, .. } | Op::RoundRect { paint, .. } | Op::Glyphs { paint, .. } => {
                matches!(paint, Paint::Image { .. })
            }
            _ => false,
        };
        if image {
            return None;
        }
        h.write_u64(hash_op(op));
    }
    Some(h.finish() | 1 << 63)
}

fn fingerprint(list: &DisplayList, out: &mut Vec<Print>, image_gen: &dyn Fn(crate::dl::ImageId) -> u64) {
    out.clear();
    let mut stack: Vec<[f32; 4]> = Vec::new();
    let mut opened: Vec<usize> = Vec::new();
    let mut m = [0f32; 4];
    for op in &list.ops {
        let hash = hash_op_with(op, image_gen);
        match op {
            Op::PushLayer { filters, .. } => {
                stack.push(m);
                for fl in filters {
                    let fm = filter::margin(fl);
                    for k in 0..4 {
                        m[k] += fm[k];
                    }
                }
                opened.push(out.len());
                out.push(Print { hash, rect: None, layer: true });
            }
            Op::PopLayer => {
                m = stack.pop().unwrap_or([0.0; 4]);
                // A layer only affects pixels its contents can reach: the union of the
                // contents' rects (which already include this layer's filter margins).
                // Unknown bounds inside leave the layer unbounded (a change means Full).
                let rect = opened.pop().and_then(|start| {
                    let mut u: Option<Rect> = None;
                    for p in &out[start + 1..] {
                        match p.rect {
                            Some(r) => u = Some(u.map_or(r, |x| x.union(r))),
                            None if p.layer => return None,
                            None => {}
                        }
                    }
                    let r = u?;
                    out[start].rect = Some(r);
                    Some(r)
                });
                out.push(Print { hash, rect, layer: true });
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
        let frame_images = self.frame_images.clone();
        let (dynamic, shared) = (self.dynamic.clone(), self.assets.clone());
        let image_gen = |id| -> u64 {
            if let Some(p) = frame_images.get(&id) {
                return std::sync::Arc::as_ptr(p) as u64;
            }
            if shared.images.get(id).is_some() {
                return 0;
            }
            dynamic.as_ref().and_then(|d| d.get(id)).map_or(1, |p| std::sync::Arc::as_ptr(&p) as u64)
        };
        fingerprint(list, &mut inc.cur, &image_gen);
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
                // A changed layer op with known bounds dirties those bounds (prev and cur);
                // only an unbounded one forces a full frame.
                if a.is_some_and(|p| p.layer && p.rect.is_none()) || b.is_some_and(|p| p.layer && p.rect.is_none()) {
                    dirty = None;
                    break;
                }
                for r in [a, b].into_iter().flatten().filter_map(|p| p.rect) {
                    *acc = Some(acc.map_or(r, |d: Rect| d.union(r)));
                }
            }
        }
        let regions: Option<Vec<[u32; 4]>> = match dirty {
            None => None,
            Some(None) => Some(Vec::new()),
            Some(Some(_)) => {
                // Cluster the dirty macroblocks into a few rectangles.
                let mut map = std::mem::take(&mut inc.map);
                map.reset(mbw, mbh, false);
                for i in 0..inc.prev.len().max(inc.cur.len()) {
                    let (a, b) = (inc.prev.get(i), inc.cur.get(i));
                    if let (Some(a), Some(b)) = (a, b) {
                        if a.hash == b.hash {
                            continue;
                        }
                    }
                    for r in [a, b].into_iter().flatten().filter_map(|p| p.rect) {
                        map.mark_rect(r.inflate(-1.0, -1.0));
                    }
                }
                let boxes = cluster(&map, 8);
                inc.map = map;
                let area: u64 = boxes.iter().map(|b| (b[2] - b[0]) as u64 * (b[3] - b[1]) as u64 * 256).sum();
                (area * 10 < w as u64 * h as u64 * 6)
                    .then(|| boxes.iter().map(|b| [b[0] * 16, b[1] * 16, (b[2] * 16).min(w), (b[3] * 16).min(h)]).collect())
            }
        };
        if regions.as_ref().is_some_and(|r| r.is_empty()) {
            changed.reset(mbw, mbh, false);
            self.inc = inc;
            return Update::Unchanged;
        }
        let had_yuv = usable;
        match &regions {
            Some(rs) => {
                inc.rects.clear();
                inc.rects.extend(inc.cur.iter().map(|p| p.rect));
                for (k, r) in rs.iter().enumerate() {
                    self.render_region(list, Some(*r), k == 0, Some(&inc.rects), None);
                }
            }
            None => {
                self.render_region(list, None, true, None, None);
            }
        }
        if !had_yuv {
            yuv.resize(w, h);
            convert_region(self.frame.data_as_u8_slice(), w as usize, h as usize, [0, 0, w as usize, h as usize], yuv);
            changed.reset(mbw, mbh, true);
            self.inc = inc;
            return Update::Full;
        }
        let Some(rs) = regions.as_deref() else {
            // Full frame: convert into the spare buffer, diff, swap.
            inc.spare.resize(w, h);
            convert_region(self.frame.data_as_u8_slice(), w as usize, h as usize, [0, 0, w as usize, h as usize], &mut inc.spare);
            super::diff::diff_mb_into(yuv, &inc.spare, changed);
            std::mem::swap(yuv, &mut inc.spare);
            self.inc = inc;
            return Update::Full;
        };
        changed.reset(mbw, mbh, false);
        for &r in rs {
            refresh_yuv(&mut inc.snap, self.frame.data_as_u8_slice(), w, h, r, yuv, changed);
        }
        let n = rs.len();
        let bbox = rs.iter().fold([u32::MAX, u32::MAX, 0, 0], |a, r| [a[0].min(r[0]), a[1].min(r[1]), a[2].max(r[2]), a[3].max(r[3])]);
        self.inc = inc;
        match regions {
            Some(_) => Update::Partial { regions: n, bbox },
            None => Update::Full,
        }
    }
}

/// Convert region `r` of the frame into `yuv`, marking macroblocks whose YUV changed.
fn refresh_yuv(snap: &mut Vec<u8>, frame: &[u8], w: u32, h: u32, [x0, y0, x1, y1]: [u32; 4], yuv: &mut YuvFrame, changed: &mut MbMap) {
    let (mx0, my0) = (x0 as usize / 16, y0 as usize / 16);
    let (mx1, my1) = ((x1 as usize).div_ceil(16), (y1 as usize).div_ceil(16));
    let (ys, cs) = (yuv.y_stride, yuv.c_stride);
    let (bw, cbw) = ((mx1 - mx0) * 16, (mx1 - mx0) * 8);
    snap.clear();
    for y in my0 * 16..my1 * 16 {
        snap.extend_from_slice(&yuv.y[y * ys + mx0 * 16..y * ys + mx0 * 16 + bw]);
    }
    for plane in [&yuv.u, &yuv.v] {
        for y in my0 * 8..my1 * 8 {
            snap.extend_from_slice(&plane[y * cs + mx0 * 8..y * cs + mx0 * 8 + cbw]);
        }
    }
    convert_region(frame, w as usize, h as usize, [x0 as usize, y0 as usize, x1 as usize, y1 as usize], yuv);
    let rows = (my1 - my0) * 16;
    let crow = (my1 - my0) * 8;
    let (sy, su) = (&snap[..rows * bw], &snap[rows * bw..rows * bw + crow * cbw]);
    let sv = &snap[rows * bw + crow * cbw..];
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
}

/// Bounding boxes (in macroblocks, `[x0, y0, x1, y1)`) of the 8-connected
/// components of `map`, merged where they overlap and down to `max` boxes.
fn cluster(map: &MbMap, max: usize) -> Vec<[u32; 4]> {
    let (w, h) = (map.mb_width, map.mb_height);
    let mut seen = vec![false; w * h];
    let mut boxes: Vec<[u32; 4]> = Vec::new();
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if seen[y * w + x] || !map.get(x, y) {
                continue;
            }
            let mut b = [x as u32, y as u32, x as u32 + 1, y as u32 + 1];
            seen[y * w + x] = true;
            stack.push((x, y));
            while let Some((cx, cy)) = stack.pop() {
                b = [b[0].min(cx as u32), b[1].min(cy as u32), b[2].max(cx as u32 + 1), b[3].max(cy as u32 + 1)];
                for (dx, dy) in [(-1i32, -1i32), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                    let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let i = ny as usize * w + nx as usize;
                    if !seen[i] && map.get(nx as usize, ny as usize) {
                        seen[i] = true;
                        stack.push((nx as usize, ny as usize));
                    }
                }
            }
            boxes.push(b);
        }
    }
    let area = |b: &[u32; 4]| (b[2] - b[0]) as u64 * (b[3] - b[1]) as u64;
    let union = |a: &[u32; 4], b: &[u32; 4]| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])];
    if boxes.len() > 4 * max {
        // Scattered changes: pairwise merging would cost O(n³); one box it is.
        let u = boxes.iter().skip(1).fold(boxes[0], |a, b| union(&a, b));
        return vec![u];
    }
    loop {
        // Merge overlapping boxes; otherwise, if too many, the pair whose union wastes least.
        let mut best: Option<(usize, usize, i64)> = None;
        for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                let (a, b) = (&boxes[i], &boxes[j]);
                let overlap = a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
                let waste = area(&union(a, b)) as i64 - area(a) as i64 - area(b) as i64;
                let score = if overlap { i64::MIN } else { waste };
                if best.is_none_or(|(_, _, s)| score < s) {
                    best = Some((i, j, score));
                }
            }
        }
        match best {
            Some((i, j, s)) if s == i64::MIN || boxes.len() > max => {
                let u = union(&boxes[i], &boxes[j]);
                boxes[i] = u;
                boxes.swap_remove(j);
            }
            _ => break,
        }
    }
    boxes
}
