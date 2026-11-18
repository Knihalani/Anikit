//! Motion blur: a camera shutter, by sub-frame accumulation in linear light.
//!
//! The pipeline hands the renderer an `eval(t)` closure. From the lists at
//! the shutter's open, centre and close, every op that changes is classified:
//! how far it moves on screen (bounds edges and transformed corners, so
//! rotation and scale count), whether it only translates, or whether it is a
//! discrete change (a glyph swapped, an op appearing). Changes are clustered
//! into a few macroblock boxes; outside them the frame is the centre render.
//!
//! - Each box takes as many samples as its fastest content needs (each step
//!   ≤ 1.25 px, 2–32; 16 for discrete changes). Streaks over 40 px (1080p) are
//!   clamped by shortening the effective shutter, so very fast moves never strobe.
//! - A box whose moving content translates together over a smooth, still
//!   backdrop (a slide) takes samples δ ≤ 8 px apart, then a δ box filter
//!   along the motion: the continuous streak, at a fraction of the samples.
//! - Blurred layers (glows, shadows) sample only every σ/2, and sub-pixel
//!   drift in scale or opacity is held at the centre value, so cached glyph
//!   runs and filtered layers are reused across samples.
//! - Image content (shader layers, 3D scenes, video frames) is made once per
//!   frame: samples reuse the centre frame's pixels, blurred by the layer's
//!   own movement only, so expensive layers never cost per sample.
//!
//! [`Renderer::set_brute_force_motion`] (and `Quality::Reference`) renders full
//! full frames instead: the ground truth `examples/mb_bench.rs` measures against.

use super::incremental::{hash_op, op_rects};
use super::{rgba_to_yuv420, FrameRgba, Renderer, YuvFrame};
use crate::dl::{DisplayList, Op};
use kurbo::{Affine, Rect, Shape, Vec2};

/// Most samples per frame.
pub const MAX_SAMPLES: usize = 32;

/// Most samples per frame at [`super::Quality::Reference`].
pub const REFERENCE_SAMPLES: usize = 96;
/// Longest streak (px at 1080p): faster motion gets a shorter effective
/// shutter, as motion-graphics renderers clamp blur, so samples stay ≤ 1.25 px apart.
pub const MAX_STREAK: f64 = 40.0;

/// Sub-frame samples for a discrete change inside the shutter (a glyph
/// swapped, an op appearing): enough that the time-weighted mix is within a
/// few levels of the true shutter average.
const EVENT_SAMPLES: usize = 16;

/// Something that changes while the shutter is open: where, how far it
/// moves on screen (px), and whether it is a discrete change.
struct Item {
    rect: Rect,
    moved: f64,
    event: bool,
    /// Screen velocity (px per half shutter) when the op only translates.
    vel: Option<Vec2>,
    /// Exactly the 16×16 blocks that change, when `rect` is much looser
    /// (a clip growing or sweeping: only the band between its edges changes).
    blocks: Option<Vec<(usize, usize)>>,
    /// For a clip that changes: its layer's push op (index, aligned lists).
    clip_of: Option<usize>,
}

fn op_transform(op: &Op) -> Option<&crate::dl::Affine> {
    match op {
        Op::Fill { transform, .. }
        | Op::Stroke { transform, .. }
        | Op::RoundRect { transform, .. }
        | Op::Image { transform, .. }
        | Op::Glyphs { transform, .. } => Some(transform),
        _ => None,
    }
}

/// Largest distance a corner of `local` travels between transforms `ta` and `tb`.
fn corner_motion(local: Rect, ta: Affine, tb: Affine) -> f64 {
    [(local.x0, local.y0), (local.x1, local.y0), (local.x0, local.y1), (local.x1, local.y1)]
        .iter()
        .map(|&(x, y)| (ta * kurbo::Point::new(x, y) - tb * kurbo::Point::new(x, y)).hypot())
        .fold(0.0, f64::max)
}

fn edge_motion(x: Rect, y: Rect) -> f64 {
    (x.x0 - y.x0).abs().max((x.x1 - y.x1).abs()).max((x.y0 - y.y0).abs()).max((x.y1 - y.y1).abs())
}

/// How far op `x` (bounds `rx`) moves on screen to become `y` (bounds `ry`):
/// its edges, and the corners of its local bounds under both transforms.
fn op_motion(x: &Op, y: &Op, rx: Rect, ry: Rect) -> f64 {
    let mut moved = edge_motion(rx, ry);
    if let (Some(ta), Some(tb)) = (op_transform(x), op_transform(y)) {
        let (ta, tb) = (super::affine(ta), super::affine(tb));
        if let Some(local) = super::ops_bounds(std::slice::from_ref(x), ta.inverse()) {
            moved = moved.max(corner_motion(local, ta, tb));
        }
    }
    moved
}

/// How far op `x` (bounds `r`) moves to become `y`, if it only translates.
fn translation(x: &Op, y: &Op, r: Rect) -> Option<Vec2> {
    let (ta, tb) = (op_transform(x)?, op_transform(y)?);
    let reach = r.width().max(r.height());
    let lin = (0..4).map(|j| (ta[j] - tb[j]).abs()).fold(0.0f32, f32::max) as f64;
    // Same shape, paint and linear part: only the offset differs.
    let mut y2 = y.clone();
    if let Some(t) = op_transform_mut(&mut y2) {
        *t = *ta;
    }
    // A slight fade (solid paint drifting < 2%) is fine too.
    if let (Some(crate::dl::Paint::Solid(pa)), Some(crate::dl::Paint::Solid(c))) = (op_paint(x), op_paint_mut(&mut y2)) {
        if (0..4).all(|k| (c.0[k] - pa.0[k]).abs() < 0.02) {
            *c = *pa;
        }
    }
    (lin * reach < 0.25 && hash_op(&y2) == hash_op(x)).then(|| Vec2::new((tb[4] - ta[4]) as f64, (tb[5] - ta[5]) as f64))
}

/// Blur radius (px) of a layer's filters, if any blur.
fn layer_softness(op: &Op) -> f32 {
    match op {
        Op::PushLayer { filters, .. } => filters
            .iter()
            .map(|f| match f {
                crate::dl::Filter::Blur(r) => *r,
                crate::dl::Filter::Shadow { blur, .. } => *blur,
                crate::dl::Filter::Glow { radius, .. } => *radius,
                _ => 0.0,
            })
            .fold(0.0, f32::max),
        _ => 0.0,
    }
}

/// Outermost blurred layers `(push, pop + 1, softness)` in `ops`.
fn soft_layers(ops: &[Op]) -> Vec<(usize, usize, f32)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        let soft = layer_softness(&ops[i]);
        if soft >= 4.0 {
            let mut depth = 0usize;
            let mut j = i;
            while j < ops.len() {
                match ops[j] {
                    Op::PushLayer { .. } => depth += 1,
                    Op::PopLayer => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            out.push((i, (j + 1).min(ops.len()), soft));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// What changes between lists `a` and `b`: ops are matched in order by kind
/// from both ends; the unmatched middle (ops appearing or vanishing) is an event.
fn changes(a: &[Op], b: &[Op], ra: &[Option<Rect>], rb: &[Option<Rect>], cut: f64, out: &mut Vec<Item>) {
    use std::mem::discriminant;
    let same_kind = |x: &Op, y: &Op| discriminant(x) == discriminant(y);
    let n = a.len().min(b.len());
    let pre = (0..n).find(|&i| !same_kind(&a[i], &b[i])).unwrap_or(n);
    let suf = (0..n - pre).find(|&k| !same_kind(&a[a.len() - 1 - k], &b[b.len() - 1 - k])).unwrap_or(n - pre);
    let pairs = (0..pre).map(|i| (i, i)).chain((0..suf).map(|k| (a.len() - 1 - k, b.len() - 1 - k)));
    for (i, j) in pairs {
        let (x, y) = (&a[i], &b[j]);
        if hash_op(x) == hash_op(y) {
            continue;
        }
        match (x, y) {
            (Op::PushLayer { clip: Some((pa, ta)), .. }, Op::PushLayer { clip: Some((pb, tb)), .. }) => {
                let (ta, tb) = (super::affine(ta), super::affine(tb));
                let (ba, bb) = (ta.transform_rect_bbox(pa.bounding_box()), tb.transform_rect_bbox(pb.bounding_box()));
                let moved = edge_motion(ba, bb).max(corner_motion(pa.bounding_box(), ta, tb));
                if moved < cut {
                    let rect = ba.union(bb);
                    let blocks = clip_blocks(pa, ta, pb, tb, rect);
                    out.push(Item { rect, moved, event: false, vel: None, blocks, clip_of: Some(i) });
                }
            }
            (Op::PushLayer { .. } | Op::PopLayer, _) => {}
            _ => {
                let (Some(xa), Some(yb)) = (ra[i], rb[j]) else { continue };
                let rect = xa.union(yb);
                if let (Op::Glyphs { glyphs: ga, .. }, Op::Glyphs { glyphs: gb, .. }) = (x, y) {
                    if ga.len() != gb.len() || ga.iter().zip(gb).any(|(p, q)| p.id != q.id) {
                        out.push(Item {
                            rect,
                            moved: 0.0,
                            event: true,
                            vel: None,
                            blocks: None,
                            clip_of: None,
                        });
                        continue;
                    }
                }
                let moved = op_motion(x, y, xa, yb);
                // Absurd jumps (an op swapped for an unrelated one) cut, not move.
                if moved >= cut {
                    out.push(Item {
                        rect,
                        moved: 0.0,
                        event: true,
                        vel: None,
                        blocks: None,
                        clip_of: None,
                    });
                } else {
                    out.push(Item {
                        rect,
                        moved,
                        event: false,
                        vel: translation(x, y, xa),
                        blocks: None,
                        clip_of: None,
                    });
                }
            }
        }
    }
    let mid = |r: &[Option<Rect>], lo: usize, hi: usize| -> Option<Rect> { r[lo..hi].iter().flatten().copied().reduce(|p, q| p.union(q)) };
    let ea = mid(ra, pre, a.len() - suf);
    let eb = mid(rb, pre, b.len() - suf);
    if let Some(rect) = match (ea, eb) {
        (Some(p), Some(q)) => Some(p.union(q)),
        (p, q) => p.or(q),
    } {
        out.push(Item {
            rect,
            moved: 0.0,
            event: true,
            vel: None,
            blocks: None,
            clip_of: None,
        });
    }
}

/// Image content (shader frames, 3D scenes, video) is costly to make: a
/// sample shows the centre frame's pixels for each image op (matched in
/// order), which still blurs by its own movement, so the content is never
/// re-rendered per sample.
fn freeze_images(ops: &mut [Op], center: &[Op]) {
    let theirs = center.iter().filter_map(|o| if let Op::Image { image, .. } = o { Some(*image) } else { None });
    let n = center.iter().filter(|o| matches!(o, Op::Image { .. })).count();
    if ops.iter().filter(|o| matches!(o, Op::Image { .. })).count() != n {
        return;
    }
    let mine = ops.iter_mut().filter_map(|o| if let Op::Image { image, .. } = o { Some(image) } else { None });
    for (a, b) in mine.zip(theirs) {
        *a = b;
    }
}

/// The 16×16 blocks (within `rect`, dilated by one) where clip `a` and clip
/// `b` differ or either one's edge passes: tested at block corners and
/// centres. `None` when that isn't much smaller than `rect`.
fn clip_blocks(pa: &kurbo::BezPath, ta: Affine, pb: &kurbo::BezPath, tb: Affine, rect: Rect) -> Option<Vec<(usize, usize)>> {
    let (ia, ib) = (ta.inverse(), tb.inverse());
    let (bx0, by0) = ((rect.x0.max(0.0) / 16.0).floor() as usize, (rect.y0.max(0.0) / 16.0).floor() as usize);
    let (bx1, by1) = ((rect.x1.max(0.0) / 16.0).ceil() as usize, (rect.y1.max(0.0) / 16.0).ceil() as usize);
    if bx1 <= bx0 || by1 <= by0 || (bx1 - bx0) * (by1 - by0) < 64 || (bx1 - bx0) * (by1 - by0) > 1 << 16 {
        return None;
    }
    let (gw, gh) = (bx1 - bx0 + 1, by1 - by0 + 1);
    // Containment at grid corners (2 bits: in a, in b).
    let at = |x: f64, y: f64| -> u8 {
        let p = kurbo::Point::new(x, y);
        (pa.winding(ia * p) != 0) as u8 | ((pb.winding(ib * p) != 0) as u8) << 1
    };
    let corner: Vec<u8> = (0..gh).flat_map(|j| (0..gw).map(move |i| (i, j))).map(|(i, j)| at(((bx0 + i) * 16) as f64, ((by0 + j) * 16) as f64)).collect();
    let mut marked = vec![false; (gw - 1) * (gh - 1)];
    for j in 0..gh - 1 {
        for i in 0..gw - 1 {
            let c = [corner[j * gw + i], corner[j * gw + i + 1], corner[(j + 1) * gw + i], corner[(j + 1) * gw + i + 1]];
            let m = at(((bx0 + i) * 16 + 8) as f64, ((by0 + j) * 16 + 8) as f64);
            // Same state everywhere, inside both or outside both: unchanged.
            if !(c.iter().all(|&v| v == m) && (m == 0 || m == 3)) {
                marked[j * (gw - 1) + i] = true;
            }
        }
    }
    let mut out = Vec::new();
    let (mw, mh) = (gw - 1, gh - 1);
    for j in 0..mh {
        for i in 0..mw {
            let near = (j.saturating_sub(1)..(j + 2).min(mh)).any(|y| (i.saturating_sub(1)..(i + 2).min(mw)).any(|x| marked[y * mw + x]));
            if near {
                out.push((bx0 + i, by0 + j));
            }
        }
    }
    (out.len() * 2 < mw * mh).then_some(out)
}

/// Row bands of 4 blocks: runs of marked columns (gaps ≤ 1 merged), as
/// block boxes. Tighter than connected clusters for rings and sweeps.
fn band_boxes(map: &super::diff::MbMap) -> Vec<[u32; 4]> {
    let mut out = Vec::new();
    for y0 in (0..map.mb_height).step_by(4) {
        let y1 = (y0 + 4).min(map.mb_height);
        let col = |x: usize| (y0..y1).any(|y| map.get(x, y));
        let mut x = 0;
        while x < map.mb_width {
            if !col(x) {
                x += 1;
                continue;
            }
            let s = x;
            let mut e = x + 1;
            while e < map.mb_width && (col(e) || (e + 1 < map.mb_width && col(e + 1))) {
                e += 1;
            }
            let rows: Vec<usize> = (y0..y1).filter(|&y| (s..e).any(|x| map.get(x, y))).collect();
            out.push([s as u32, rows[0] as u32, e as u32, *rows.last().unwrap() as u32 + 1]);
            x = e;
        }
    }
    out
}

/// Hold sub-pixel drift at the centre frame's values in a sample list:
/// a linear part (scale, rotation) that moves content < 0.05 px over the
/// shutter, and layer opacity that changes < 1%. The average is unchanged to
/// well under a level, and cached glyph runs and layers stay valid across samples.
fn snap(ops: &mut [Op], center: &[Op], rects: &[Option<Rect>]) {
    if ops.len() != center.len() {
        return;
    }
    for i in 0..ops.len() {
        let (o, c) = (&mut ops[i], &center[i]);
        if let (Op::PushLayer { opacity: a, .. }, Op::PushLayer { opacity: b, .. }) = (&mut *o, c) {
            if (*a - *b).abs() < 0.01 {
                *a = *b;
            }
            continue;
        }
        let (Some(r), Some(tc)) = (rects.get(i).copied().flatten(), op_transform(c)) else {
            continue;
        };
        let reach = r.width().max(r.height());
        if let Some(t) = op_transform_mut(o) {
            let d = (0..4).map(|j| (t[j] - tc[j]).abs()).fold(0.0f32, f32::max) as f64;
            if d > 0.0 && d * reach < 0.05 {
                t[..4].copy_from_slice(&tc[..4]);
            }
        }
    }
}

fn op_paint(op: &Op) -> Option<&crate::dl::Paint> {
    match op {
        Op::Fill { paint, .. } | Op::Stroke { paint, .. } | Op::RoundRect { paint, .. } | Op::Glyphs { paint, .. } => Some(paint),
        _ => None,
    }
}

fn op_paint_mut(op: &mut Op) -> Option<&mut crate::dl::Paint> {
    match op {
        Op::Fill { paint, .. } | Op::Stroke { paint, .. } | Op::RoundRect { paint, .. } | Op::Glyphs { paint, .. } => Some(paint),
        _ => None,
    }
}

fn op_transform_mut(op: &mut Op) -> Option<&mut crate::dl::Affine> {
    match op {
        Op::Fill { transform, .. }
        | Op::Stroke { transform, .. }
        | Op::RoundRect { transform, .. }
        | Op::Image { transform, .. }
        | Op::Glyphs { transform, .. } => Some(transform),
        _ => None,
    }
}

/// The `PopLayer` closing the layer pushed at `p`.
fn layer_end(ops: &[Op], p: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (j, op) in ops.iter().enumerate().skip(p) {
        match op {
            Op::PushLayer { .. } => depth += 1,
            Op::PopLayer => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// Box-filter a `bw × bh` RGB accumulator along `slot` (its length is the
/// filter width), into `out`. Taps every ½ px, bilinear, clamped to the box.
fn smear_box(acc: &[f32], bw: usize, bh: usize, slot: Vec2, out: &mut Vec<f32>, cols: &mut Vec<f32>) {
    out.clear();
    out.resize(acc.len(), 0.0);
    // Axis-aligned (nearly every slide): an exact box over pixel cells by prefix sums.
    if slot.y.abs() * 64.0 < slot.x.abs() || slot.x.abs() * 64.0 < slot.y.abs() {
        let horizontal = slot.y.abs() < slot.x.abs();
        let len = slot.hypot().max(1e-3);
        let n = if horizontal { bw } else { bh };
        // Element k of line l.
        let at = move |l: usize, k: usize| if horizontal { (l * bw + k) * 3 } else { (k * bw + l) * 3 };
        let line = |pre: &mut Vec<[f64; 3]>, l: usize, dst: &mut [f32], stride: usize, base: usize| {
            // Cumulative integral, cells [k - ½, k + ½], edges extended.
            pre.clear();
            pre.resize(n + 1, [0.0; 3]);
            for k in 0..n {
                let p = at(l, k);
                for c in 0..3 {
                    pre[k + 1][c] = pre[k][c] + acc[p + c] as f64;
                }
            }
            let integral = |u: f64, c: usize| -> f64 {
                // ∫ from -½ to u.
                if u <= -0.5 {
                    return (u + 0.5) * acc[at(l, 0) + c] as f64;
                }
                if u >= n as f64 - 0.5 {
                    return pre[n][c] + (u - (n as f64 - 0.5)) * acc[at(l, n - 1) + c] as f64;
                }
                let k = (u + 0.5).floor() as usize;
                pre[k][c] + (u + 0.5 - k as f64) * acc[at(l, k) + c] as f64
            };
            for k in 0..n {
                let (lo, hi) = (k as f64 - 0.5 * len, k as f64 + 0.5 * len);
                for c in 0..3 {
                    dst[base + k * stride + c] = ((integral(hi, c) - integral(lo, c)) / len) as f32;
                }
            }
        };
        use rayon::prelude::*;
        if horizontal {
            out.par_chunks_mut(bw * 3).enumerate().for_each_init(|| Vec::with_capacity(n + 1), |pre, (l, row)| line(pre, l, row, 3, 0));
        } else {
            cols.clear();
            cols.resize(bw * bh * 3, 0.0);
            cols.par_chunks_mut(bh * 3).enumerate().for_each_init(|| Vec::with_capacity(n + 1), |pre, (l, col)| line(pre, l, col, 3, 0));
            for x in 0..bw {
                for y in 0..bh {
                    out[(y * bw + x) * 3..(y * bw + x) * 3 + 3].copy_from_slice(&cols[(x * bh + y) * 3..(x * bh + y) * 3 + 3]);
                }
            }
        }
        return;
    }
    let m = ((slot.hypot() * 2.0).ceil() as usize).max(2);
    let taps: Vec<(f32, f32)> = (0..m)
        .map(|j| {
            let s = (j as f64 + 0.5) / m as f64 - 0.5;
            ((slot.x * s) as f32, (slot.y * s) as f32)
        })
        .collect();
    let inv = 1.0 / m as f32;
    let (xmax, ymax) = ((bw - 1) as f32, (bh - 1) as f32);
    let row = |(y, orow): (usize, &mut [f32])| {
        for x in 0..bw {
            let mut c = [0f32; 3];
            for &(dx, dy) in &taps {
                let (sx, sy) = ((x as f32 + dx).clamp(0.0, xmax), (y as f32 + dy).clamp(0.0, ymax));
                let (x0, y0) = (sx as usize, sy as usize);
                let (x1, y1) = ((x0 + 1).min(bw - 1), (y0 + 1).min(bh - 1));
                let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                let (p00, p10, p01, p11) = ((y0 * bw + x0) * 3, (y0 * bw + x1) * 3, (y1 * bw + x0) * 3, (y1 * bw + x1) * 3);
                for k in 0..3 {
                    let top = acc[p00 + k] + (acc[p10 + k] - acc[p00 + k]) * fx;
                    let bot = acc[p01 + k] + (acc[p11 + k] - acc[p01 + k]) * fx;
                    c[k] += top + (bot - top) * fy;
                }
            }
            orow[x * 3..x * 3 + 3].copy_from_slice(&[c[0] * inv, c[1] * inv, c[2] * inv]);
        }
    };
    use rayon::prelude::*;
    out.par_chunks_mut(bw * 3).enumerate().for_each(row);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smear_box_ref(acc: &[f32], bw: usize, bh: usize, slot: Vec2, out: &mut Vec<f32>) {
        out.clear();
        out.resize(acc.len(), 0.0);
        // Axis-aligned (nearly every slide): an exact box over pixel cells by prefix sums.
        if slot.y.abs() * 64.0 < slot.x.abs() || slot.x.abs() * 64.0 < slot.y.abs() {
            let horizontal = slot.y.abs() < slot.x.abs();
            let len = slot.hypot().max(1e-3);
            let n = if horizontal { bw } else { bh };
            // Element k of line l.
            let at = move |l: usize, k: usize| if horizontal { (l * bw + k) * 3 } else { (k * bw + l) * 3 };
            let line = |l: usize, dst: &mut [f32], stride: usize, base: usize| {
                // Cumulative integral, cells [k - ½, k + ½], edges extended.
                let mut pre = vec![[0f64; 3]; n + 1];
                for k in 0..n {
                    let p = at(l, k);
                    for c in 0..3 {
                        pre[k + 1][c] = pre[k][c] + acc[p + c] as f64;
                    }
                }
                let integral = |u: f64, c: usize| -> f64 {
                    // ∫ from -½ to u.
                    if u <= -0.5 {
                        return (u + 0.5) * acc[at(l, 0) + c] as f64;
                    }
                    if u >= n as f64 - 0.5 {
                        return pre[n][c] + (u - (n as f64 - 0.5)) * acc[at(l, n - 1) + c] as f64;
                    }
                    let k = (u + 0.5).floor() as usize;
                    pre[k][c] + (u + 0.5 - k as f64) * acc[at(l, k) + c] as f64
                };
                for k in 0..n {
                    let (lo, hi) = (k as f64 - 0.5 * len, k as f64 + 0.5 * len);
                    for c in 0..3 {
                        dst[base + k * stride + c] = ((integral(hi, c) - integral(lo, c)) / len) as f32;
                    }
                }
            };
            use rayon::prelude::*;
            if horizontal {
                out.par_chunks_mut(bw * 3).enumerate().for_each(|(l, row)| line(l, row, 3, 0));
            } else {
                let mut cols = vec![0f32; bw * bh * 3];
                cols.par_chunks_mut(bh * 3).enumerate().for_each(|(l, col)| line(l, col, 3, 0));
                for x in 0..bw {
                    for y in 0..bh {
                        out[(y * bw + x) * 3..(y * bw + x) * 3 + 3].copy_from_slice(&cols[(x * bh + y) * 3..(x * bh + y) * 3 + 3]);
                    }
                }
            }
            return;
        }
        let m = ((slot.hypot() * 2.0).ceil() as usize).max(2);
        let taps: Vec<(f32, f32)> = (0..m)
            .map(|j| {
                let s = (j as f64 + 0.5) / m as f64 - 0.5;
                ((slot.x * s) as f32, (slot.y * s) as f32)
            })
            .collect();
        let inv = 1.0 / m as f32;
        let (xmax, ymax) = ((bw - 1) as f32, (bh - 1) as f32);
        let row = |(y, orow): (usize, &mut [f32])| {
            for x in 0..bw {
                let mut c = [0f32; 3];
                for &(dx, dy) in &taps {
                    let (sx, sy) = ((x as f32 + dx).clamp(0.0, xmax), (y as f32 + dy).clamp(0.0, ymax));
                    let (x0, y0) = (sx as usize, sy as usize);
                    let (x1, y1) = ((x0 + 1).min(bw - 1), (y0 + 1).min(bh - 1));
                    let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                    let (p00, p10, p01, p11) = ((y0 * bw + x0) * 3, (y0 * bw + x1) * 3, (y1 * bw + x0) * 3, (y1 * bw + x1) * 3);
                    for k in 0..3 {
                        let top = acc[p00 + k] + (acc[p10 + k] - acc[p00 + k]) * fx;
                        let bot = acc[p01 + k] + (acc[p11 + k] - acc[p01 + k]) * fx;
                        c[k] += top + (bot - top) * fy;
                    }
                }
                orow[x * 3..x * 3 + 3].copy_from_slice(&[c[0] * inv, c[1] * inv, c[2] * inv]);
            }
        };
        use rayon::prelude::*;
        out.par_chunks_mut(bw * 3).enumerate().for_each(row);
    }

    #[test]
    fn smear_box_matches_reference_and_reuses_buffers() {
        let mut state = 0x1234_5678u32;
        let sizes = [(64, 200), (37, 23), (64, 200), (37, 23)];
        let slots = [
            Vec2::new(7.3, 0.01),
            Vec2::new(0.02, -5.6),
            Vec2::new(0.3, 0.2),
            Vec2::new(4.7, -3.1),
        ];
        let mut out = Vec::new();
        let mut cols = Vec::new();

        for (bw, bh) in sizes {
            let mut acc = vec![0.0; bw * bh * 3];
            for value in &mut acc {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *value = (state >> 8) as f32 / (1 << 24) as f32;
            }

            for slot in slots {
                let mut expected = Vec::new();
                smear_box_ref(&acc, bw, bh, slot, &mut expected);
                smear_box(&acc, bw, bh, slot, &mut out, &mut cols);
                assert_eq!(out, expected, "size {bw}x{bh}, slot {slot:?}");
                assert_eq!(
                    out.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
                    expected.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
                    "bitwise mismatch at size {bw}x{bh}, slot {slot:?}"
                );
            }
        }
    }
}

impl Renderer {
    /// Add the coverage (0..1) of clip `path` under `aff` to `out`, over box `r`.
    fn clip_coverage(&mut self, path: &kurbo::BezPath, aff: &crate::dl::Affine, r: [usize; 4], out: &mut [f32]) {
        let (bw, bh) = ((r[2] - r[0]) as u16, (r[3] - r[1]) as u16);
        let ctx = self.mask_ctx.get_or_insert_with(|| vello_cpu::RenderContext::new(bw, bh));
        ctx.reset_and_resize(bw, bh);
        ctx.set_paint(super::color(crate::dl::Rgba([1.0, 1.0, 1.0, 1.0])));
        ctx.set_fill_rule(vello_cpu::peniko::Fill::NonZero);
        ctx.set_transform(Affine::translate((-(r[0] as f64), -(r[1] as f64))) * super::affine(aff));
        ctx.fill_path(path);
        ctx.flush();
        self.mask_pix.resize(bw, bh);
        ctx.render_with(&mut self.mask_pix, &mut self.st.res, vello_cpu::RasterizerSettings::default());
        ctx.reset();
        for (o, p) in out.iter_mut().zip(self.mask_pix.data_as_u8_slice().chunks_exact(4)) {
            *o += p[3] as f32 / 255.0;
        }
    }

    /// Render the frame at `t` with a shutter open for `shutter` seconds
    /// (centred on `t`; 180° at 30 fps = 1/60 s). `eval(t)` must return the
    /// display list at time `t`. Static frames render once.
    pub fn render_motion_blur(&mut self, eval: &mut dyn FnMut(f32) -> DisplayList, t: f32, shutter: f32) -> &FrameRgba {
        let center = eval(t);
        if shutter <= 0.0 {
            return self.render(&center);
        }
        let (a, b) = (eval(t - shutter * 0.5), eval(t + shutter * 0.5));
        // A smooth, edge-free backdrop (gradients, glows) looks the same blurred
        // or not: judge motion on what's drawn over it only.
        let (wf, hf) = (center.width as f64, center.height as f64);
        let skip = |l: &DisplayList| super::smooth::prefix(&l.ops, wf, hf, &|_| None);
        let k = skip(&center).min(skip(&a)).min(skip(&b));
        let top = |l: &DisplayList| DisplayList {
            ops: l.ops[k.min(l.ops.len())..].to_vec(),
            ..l.clone()
        };
        let (ta, tc, tb) = (top(&a), top(&center), top(&b));
        let same = ta.ops.len() == tc.ops.len()
            && tb.ops.len() == tc.ops.len()
            && ta.ops.iter().zip(&tc.ops).zip(&tb.ops).all(|((x, c), y)| {
                let hc = hash_op(c);
                hash_op(x) == hc && hash_op(y) == hc
            });
        if same {
            return self.render(&center);
        }
        let (w, h) = (center.width.max(1) as usize, center.height.max(1) as usize);
        // What changes over each half of the shutter (both halves, so motion
        // that turns back mid-shutter still counts).
        let cut = 0.5 * w.max(h) as f64;
        let mut rects = [Vec::new(), Vec::new(), Vec::new()];
        for (r, l) in rects.iter_mut().zip([&ta, &tc, &tb]) {
            op_rects(&l.ops, r);
        }
        let mut items = Vec::new();
        changes(&ta.ops, &tc.ops, &rects[0], &rects[1], cut, &mut items);
        changes(&tc.ops, &tb.ops, &rects[1], &rects[2], cut, &mut items);
        // Each half's motion is half the streak.
        let d = items.iter().map(|i| 2.0 * i.moved).fold(0.0, f64::max);
        if d < 0.5 && !items.iter().any(|i| i.event) {
            return self.render(&center);
        }
        let reference = self.motion_brute || self.quality == super::Quality::Reference;
        let max_streak = (MAX_STREAK * h as f64 / 1080.0).max(16.0);
        let (mut shutter, mut scale) = (shutter, 1.0);
        if d > max_streak {
            scale = max_streak / d;
            shutter *= scale as f32;
        }
        let samples = |i: &Item| -> usize {
            if i.event {
                EVENT_SAMPLES
            } else if 2.0 * i.moved * scale < 0.5 {
                1
            } else {
                ((2.0 * i.moved * scale / 1.25).ceil() as usize).clamp(2, MAX_SAMPLES)
            }
        };
        // Cluster what changes into a few macroblock-aligned boxes, each with
        // as many samples as its fastest content needs.
        let mut map = super::diff::MbMap::new(w.div_ceil(16), h.div_ceil(16));
        for i in items.iter().filter(|i| samples(i) > 1) {
            match &i.blocks {
                Some(b) => {
                    let (mw, mh) = (map.mb_width, map.mb_height);
                    b.iter().filter(|(x, y)| *x < mw && *y < mh).for_each(|&(x, y)| map.set(x, y));
                }
                None => map.mark_rect(i.rect.inflate(2.0, 2.0)),
            }
        }
        // Ops that don't change at all over the shutter.
        let aligned = ta.ops.len() == tc.ops.len() && tb.ops.len() == tc.ops.len();
        let still: Vec<bool> = if aligned {
            (0..tc.ops.len())
                .map(|j| {
                    let hc = hash_op(&tc.ops[j]);
                    hash_op(&ta.ops[j]) == hc && hash_op(&tb.ops[j]) == hc
                })
                .collect()
        } else {
            Vec::new()
        };
        // Samples for box `br`, and the slot vector of a post filter when
        // everything moving in it translates together over a smooth, still
        // backdrop: n samples δ apart, then a δ box filter along the motion,
        // is the continuous streak exactly, at a fraction of the samples.
        let plan = |br: Rect| -> (usize, Option<Vec2>) {
            let inside: Vec<&Item> = items.iter().filter(|i| i.rect.inflate(3.0, 3.0).overlaps(br)).collect();
            let n = inside.iter().map(|i| samples(i)).max().unwrap_or(1);
            if reference || !aligned || n <= 3 {
                return (n, None);
            }
            let Some(v0) = inside.iter().find(|i| i.moved > 0.0).and_then(|i| i.vel) else {
                return (n, None);
            };
            // Velocities may differ a little (acceleration over the shutter):
            // slots then misalign by δ·Δv/|v|, kept ≤ ¼ px by a shorter δ.
            let dv = inside.iter().filter_map(|i| i.vel).map(|v| (v - v0).hypot()).fold(0.0, f64::max);
            let rel = dv / v0.hypot().max(1e-9);
            if rel > 0.2 || inside.iter().any(|i| i.event || i.vel.is_none()) {
                return (n, None);
            }
            let smooth_still = (0..tc.ops.len()).filter(|&j| still[j]).all(|j| match &tc.ops[j] {
                Op::PushLayer { clip: None, .. } | Op::PopLayer => true,
                op => match rects[1][j] {
                    None => true,
                    Some(r) if !r.overlaps(br) => true,
                    Some(r) => {
                        matches!(
                            op,
                            Op::RoundRect {
                                paint: crate::dl::Paint::Solid(_),
                                ..
                            } | Op::Fill {
                                paint: crate::dl::Paint::Solid(_),
                                ..
                            }
                        ) && r.inflate(-1.0, -1.0).contains_rect(br.inflate(1.0, 1.0))
                    }
                },
            });
            if !smooth_still {
                return (n, None);
            }
            let streak = v0 * (2.0 * scale);
            let delta = (0.25 / rel.max(1e-9)).clamp(1.25, 8.0);
            let m = ((streak.hypot() / delta).ceil() as usize).max(2);
            if m >= n {
                return (n, None);
            }
            (m, Some(streak / m as f64))
        };
        // Few loose boxes, or many tight bands when that covers far less.
        let area = |bs: &[[u32; 4]]| bs.iter().map(|b| ((b[2] - b[0]) * (b[3] - b[1])) as usize).sum::<usize>();
        let mut chosen = super::incremental::cluster(&map, 8);
        if items.iter().any(|i| i.blocks.is_some()) {
            let bands = band_boxes(&map);
            if bands.len() <= 48 && area(&bands) * 2 < area(&chosen) {
                chosen = bands;
            }
        }
        let mut boxes: Vec<([usize; 4], usize, Option<Vec2>)> = chosen
            .into_iter()
            .map(|b| [b[0] as usize * 16, b[1] as usize * 16, (b[2] as usize * 16).min(w), (b[3] as usize * 16).min(h)])
            .filter(|b| b[2] > b[0] && b[3] > b[1])
            .map(|b| {
                let (n, smear) = plan(Rect::new(b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64));
                (b, n, smear)
            })
            .filter(|b| b.1 > 1)
            .collect();
        if boxes.is_empty() {
            return self.render(&center);
        }
        let boxed_area: usize = boxes.iter().map(|(b, _, _)| (b[2] - b[0]) * (b[3] - b[1])).sum();
        if reference || boxed_area * 10 >= w * h * 6 {
            let (n, smear) = if reference {
                (((d / 0.5).ceil() as usize).clamp(8, REFERENCE_SAMPLES), None)
            } else {
                plan(Rect::new(0.0, 0.0, w as f64, h as f64))
            };
            boxes = vec![([0, 0, w, h], n.max(2), smear)];
        }
        let full = boxes.len() == 1 && boxes[0].0 == [0, 0, w, h];
        // Clip wipes (an iris, a wipe): in a box where only one layer's clip
        // changes and everything else is still, the shutter average is the
        // layer shown in proportion to the time each pixel spends inside the
        // clip. Two renders and a clip mask per sample, not a render per sample.
        let wipes: Vec<Option<(usize, usize)>> = boxes
            .iter()
            .map(|(b, _, _)| {
                if reference || full || !aligned {
                    return None;
                }
                let br = Rect::new(b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64);
                let inside: Vec<&Item> = items.iter().filter(|i| i.rect.inflate(3.0, 3.0).overlaps(br)).collect();
                let p = inside.first()?.clip_of?;
                if inside.iter().any(|i| i.event || i.clip_of != Some(p)) {
                    return None;
                }
                let q = layer_end(&tc.ops, p)?;
                let calm = (0..tc.ops.len()).all(|j| {
                    j == p
                        || j == q
                        || still[j]
                        || (!matches!(tc.ops[j], Op::PushLayer { .. } | Op::PopLayer)
                            && [&rects[0], &rects[1], &rects[2]].iter().all(|r| r[j].is_none_or(|r| !r.overlaps(br))))
                });
                calm.then_some((p, q))
            })
            .collect();
        // Post runs once on the average: switch it off for the samples.
        let post = self.post.take();
        let lin: Vec<f32> = (0..256).map(|i| super::post::srgb_to_linear(i as f32 / 255.0)).collect();
        // One linear-light accumulator per box, packed.
        let offs: Vec<usize> = boxes
            .iter()
            .scan(0, |o, (b, _, _)| {
                let at = *o;
                *o += (b[2] - b[0]) * (b[3] - b[1]) * 3;
                Some(at)
            })
            .collect();
        let total: usize = boxes.iter().map(|(b, _, _)| (b[2] - b[0]) * (b[3] - b[1]) * 3).sum();
        let mut acc = std::mem::take(&mut self.motion_acc);
        acc.clear();
        acc.resize(total, 0.0);
        if !full {
            // Everything outside the boxes is the centre frame.
            self.render(&center);
        }
        // Sub-frame samples per blurred layer: spaced ≥ σ/2 apart (a box streak
        // that short widens the blur by well under a pixel).
        let soft: Vec<(usize, usize, usize)> = if reference || ta.ops.len() != tc.ops.len() || tb.ops.len() != tc.ops.len() {
            Vec::new()
        } else {
            soft_layers(&tc.ops)
                .into_iter()
                .map(|(s0, s1, sigma)| {
                    let moved = (s0..s1)
                        .filter_map(|j| Some(op_motion(&ta.ops[j], &tb.ops[j], rects[0][j]?, rects[2][j]?)))
                        .filter(|m| *m < cut)
                        .fold(0.0, f64::max);
                    let m = (moved * scale / (0.5 * sigma as f64).max(1.25)).ceil() as usize;
                    (s0, s1, m.max(1))
                })
                .collect()
        };
        let mut slots: Vec<(u32, DisplayList)> = Vec::new();
        let mut sample_rects = Vec::new();
        // Boxes with the same sample count share their sub-frame lists.
        // Wipe boxes: two renders and the clip's coverage over the shutter.
        for ((r, n, _), (&o, wipe)) in boxes.iter().zip(offs.iter().zip(&wipes)) {
            let Some((p, q)) = *wipe else { continue };
            let (pk, qk) = (k + p, k + q);
            let mut outside = center.clone();
            outside.ops.drain(pk..=qk);
            let mut unclipped = center.clone();
            if let Op::PushLayer { clip, .. } = &mut unclipped.ops[pk] {
                *clip = None;
            }
            let region = Some([r[0] as u32, r[1] as u32, r[2] as u32, r[3] as u32]);
            let (bw, bh) = (r[2] - r[0], r[3] - r[1]);
            let read = |me: &mut Self, l: &DisplayList| -> Vec<f32> {
                me.render_region(l, region, true, None, None);
                let px = me.frame.data_as_u8_slice();
                (r[1]..r[3]).flat_map(|y| px[(y * w + r[0]) * 4..(y * w + r[2]) * 4].chunks_exact(4).flat_map(|p| [lin[p[0] as usize], lin[p[1] as usize], lin[p[2] as usize]])).collect()
            };
            let (lo, hi) = (read(self, &outside), read(self, &unclipped));
            let mut cover = vec![0f32; bw * bh];
            for i in 0..*n {
                let ts = t - shutter * 0.5 + shutter * (i as f32 + 0.5) / *n as f32;
                let l = eval(ts);
                match l.ops.get(pk) {
                    Some(Op::PushLayer { clip: Some((path, aff)), .. }) if l.ops.len() == center.ops.len() => self.clip_coverage(path, aff, *r, &mut cover),
                    _ => cover.iter_mut().for_each(|c| *c += 1.0),
                }
            }
            let nf = *n as f32;
            for (j, c) in cover.iter().enumerate() {
                for ch in 0..3 {
                    let (a, b) = (lo[j * 3 + ch], hi[j * 3 + ch]);
                    acc[o + j * 3 + ch] = (nf - c) * a + c * b;
                }
            }
        }
        let mut counts: Vec<usize> = boxes.iter().zip(&wipes).filter(|(_, w)| w.is_none()).map(|(b, _)| b.1).collect();
        counts.sort_unstable();
        counts.dedup();
        for &n in &counts {
            let regions: Vec<([usize; 4], usize)> = boxes.iter().zip(offs.iter().zip(&wipes)).filter(|(b, (_, w))| b.1 == n && w.is_none()).map(|(b, (&o, _))| (b.0, o)).collect();
            for i in 0..n {
                // Sample times spread evenly over the open shutter (centres of n slots).
                let ts = t - shutter * 0.5 + shutter * (i as f32 + 0.5) / n as f32;
                let mut l = eval(ts);
                // The smooth backdrop isn't blurred (see above): keep the centre
                // frame's, so it renders once rather than per sample.
                if k > 0 && l.ops.len() >= k && skip(&l) >= k {
                    l.ops.splice(..k, center.ops[..k].iter().cloned());
                }
                // Blurred layers need samples only every σ/2 or so: their ops
                // come from a few coarser slots (and their cached blur is reused).
                for &(s0, s1, m) in &soft {
                    if m >= n {
                        continue;
                    }
                    let slot = ((i as f32 + 0.5) / n as f32 * m as f32) as usize;
                    let tq = t - shutter * 0.5 + shutter * (slot as f32 + 0.5) / m as f32;
                    let at = match slots.iter().position(|(x, _)| *x == tq.to_bits()) {
                        Some(p) => p,
                        None => {
                            slots.push((tq.to_bits(), eval(tq)));
                            slots.len() - 1
                        }
                    };
                    let src = &slots[at].1.ops;
                    if src.len() == l.ops.len() && l.ops.len() == center.ops.len() {
                        l.ops[k + s0..k + s1].clone_from_slice(&src[k + s0..k + s1]);
                    }
                }
                snap(&mut l.ops[k..], &center.ops[k.min(center.ops.len())..], &rects[1]);
                if !reference {
                    freeze_images(&mut l.ops, &center.ops);
                }
                if full {
                    self.render(&l);
                } else {
                    // Op bounds once per sample, for culling in every box.
                    op_rects(&l.ops, &mut sample_rects);
                    for (k, (r, _)) in regions.iter().enumerate() {
                        self.render_region(&l, Some([r[0] as u32, r[1] as u32, r[2] as u32, r[3] as u32]), k == 0, Some(&sample_rects), None);
                    }
                }
                let px = self.frame.data_as_u8_slice();
                for &(r, o) in &regions {
                    let bw = r[2] - r[0];
                    let rows = &mut acc[o..o + bw * (r[3] - r[1]) * 3];
                    let add = |(y, arow): (usize, &mut [f32])| {
                        let prow = &px[((r[1] + y) * w + r[0]) * 4..((r[1] + y) * w + r[2]) * 4];
                        for (a, p) in arow.chunks_exact_mut(3).zip(prow.chunks_exact(4)) {
                            a[0] += lin[p[0] as usize];
                            a[1] += lin[p[1] as usize];
                            a[2] += lin[p[2] as usize];
                        }
                    };
                    if bw * (r[3] - r[1]) > 1 << 16 {
                        use rayon::prelude::*;
                        rows.par_chunks_mut(bw * 3).enumerate().for_each(add);
                    } else {
                        rows.chunks_mut(bw * 3).enumerate().for_each(add);
                    }
                }
            }
        }
        let frame = self.frame.data_as_u8_slice_mut();
        let mut smeared = std::mem::take(&mut self.motion_tmp);
        let mut cols = std::mem::take(&mut self.motion_cols);
        for ((r, n, smear), &o) in boxes.iter().zip(&offs) {
            let (bw, inv) = (r[2] - r[0], 1.0 / *n as f32);
            let mut acc = &acc[o..o + bw * (r[3] - r[1]) * 3];
            if let Some(slot) = smear {
                smear_box(acc, bw, r[3] - r[1], *slot, &mut smeared, &mut cols);
                acc = &smeared;
            }
            let put = |(y, frow): (usize, &mut [u8])| {
                if y < r[1] || y >= r[3] {
                    return;
                }
                let arow = &acc[(y - r[1]) * bw * 3..(y - r[1] + 1) * bw * 3];
                for (x, (a, f)) in (r[0]..r[2]).zip(arow.chunks_exact(3).zip(frow[r[0] * 4..r[2] * 4].chunks_exact_mut(4))) {
                    // Back to sRGB with dither (averaging creates smooth ramps that would band).
                    let d = super::post::tri_dither(x as u32, y as u32);
                    for k in 0..3 {
                        f[k] = (super::post::encode_srgb255(a[k] * inv) + d + 0.5).clamp(0.0, 255.0) as u8;
                    }
                    f[3] = 255;
                }
            };
            let rows = &mut frame[r[1] * w * 4..r[3] * w * 4];
            if bw * (r[3] - r[1]) > 1 << 16 {
                use rayon::prelude::*;
                rows.par_chunks_mut(w * 4).enumerate().for_each(|(y, f)| put((y + r[1], f)));
            } else {
                rows.chunks_mut(w * 4).enumerate().for_each(|(y, f)| put((y + r[1], f)));
            }
        }
        self.motion_acc = acc;
        self.motion_tmp = smeared;
        self.motion_cols = cols;
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
