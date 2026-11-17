//! `critique --render`: pixel-level QA on sampled frames, so agents get
//! "unreadable text" and "text collides" findings without looking at images.
//!
//! For each sampled frame (each scene at 40% and 75% of its length, plus
//! transition midpoints), text is found in the display list (glyph runs merged
//! into blocks), the frame is rendered once without text to get the real
//! background, and each block is checked for:
//! - **contrast**: WCAG ratio of its colour against the pixels behind it (the
//!   darkest/lightest 10%, not the average);
//! - **overlap**: text blocks covering each other;
//! - **frame / safe area**: text cut by the frame edge or outside the title-safe margins;
//! - **empty frame**: no text and almost no visual variation.
//!
//! Frames render at half resolution, so a 30 s video takes well under a second.

use crate::critique::Note;
use crate::dl::{Affine, DisplayList, Op, Paint};
use crate::eval::Evaluator;
use crate::pipeline;
use crate::templates::{color, Format};

/// A block of text found in a display list, in output pixels.
#[derive(Clone, Debug)]
struct Block {
    rect: [f32; 4],
    size: f32,
    color: color::C,
    /// The spec path of the layer that drew it.
    layer: String,
}

fn bbox(t: &Affine, x0: f32, y0: f32, x1: f32, y1: f32) -> [f32; 4] {
    let pts = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].map(|(x, y)| (t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5]));
    let xs = pts.map(|p| p.0);
    let ys = pts.map(|p| p.1);
    [xs.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MAX, f32::min), xs.iter().cloned().fold(f32::MIN, f32::max), ys.iter().cloned().fold(f32::MIN, f32::max)]
}

/// For each op, the spec path of the user text layer that drew it, or `None`
/// for anything inside a component (device UI, chips, cards: their own design).
fn user_text(ev: &Evaluator, n: usize, tags: &[crate::eval::OpTag]) -> Vec<Option<String>> {
    let mut best: Vec<Option<(usize, u32)>> = vec![None; n];
    let mut in_component = vec![false; n];
    for tag in tags {
        let (_, kind) = ev.layer_info(tag.layer);
        let comp = kind.starts_with("component:");
        for k in tag.ops.clone().take_while(|&k| k < n) {
            in_component[k] |= comp;
            if best[k].is_none_or(|(len, _)| tag.ops.len() < len) {
                best[k] = Some((tag.ops.len(), tag.layer));
            }
        }
    }
    best.into_iter()
        .zip(in_component)
        .map(|(b, comp)| {
            let (_, layer) = b?;
            let (path, kind) = ev.layer_info(layer);
            (!comp && (kind == "text" || kind == "caption")).then(|| path.to_string())
        })
        .collect()
}

/// Text blocks with their effective opacity ≥ 0.6 (settled, not mid-fade).
/// Only glyphs of user text layers count (`who`, from [`user_text`]).
fn text_blocks(dl: &DisplayList, who: &[Option<String>]) -> Vec<Block> {
    text_blocks_min(dl, who, 0.6)
}

fn text_blocks_min(dl: &DisplayList, who: &[Option<String>], min_alpha: f32) -> Vec<Block> {
    let mut opacity = vec![1.0f32];
    // Out-of-focus text (depth of field, blurIn) is decoration, not something to read.
    let mut blurred = vec![false];
    let mut runs: Vec<Block> = Vec::new();
    for (k, op) in dl.ops.iter().enumerate() {
        let layer = match who.get(k) {
            Some(Some(p)) => p.clone(),
            _ if matches!(op, Op::Glyphs { .. }) => continue,
            _ => String::new(),
        };
        match op {
            Op::PushLayer { opacity: o, filters, .. } => {
                opacity.push(opacity.last().unwrap() * o);
                let b = filters.iter().any(|f| matches!(f, crate::dl::Filter::Blur(r) if *r > 1.5));
                blurred.push(*blurred.last().unwrap() || b);
            }
            Op::PopLayer => {
                if opacity.len() > 1 {
                    opacity.pop();
                    blurred.pop();
                }
            }
            Op::Glyphs { size, glyphs, paint: Paint::Solid(c), transform, .. } if !glyphs.is_empty() => {
                let a = c.0[3] * opacity.last().unwrap();
                if a < min_alpha || *blurred.last().unwrap() {
                    continue;
                }
                let (mut x0, mut x1) = (f32::MAX, f32::MIN);
                let (mut y0, mut y1) = (f32::MAX, f32::MIN);
                // The last glyph's width: the run's average advance (narrow letters end runs often).
                let adv = if glyphs.len() > 1 {
                    let (a, b) = glyphs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), g| (lo.min(g.x), hi.max(g.x)));
                    ((b - a) / (glyphs.len() - 1) as f32).clamp(size * 0.25, size * 0.55)
                } else {
                    size * 0.5
                };
                for g in glyphs {
                    x0 = x0.min(g.x);
                    x1 = x1.max(g.x + adv);
                    y0 = y0.min(g.y - size * 0.72);
                    y1 = y1.max(g.y + size * 0.1);
                }
                let r = bbox(transform, x0, y0, x1, y1);
                let scale = (transform[0] * transform[3] - transform[1] * transform[2]).abs().sqrt();
                let c = c.0;
                runs.push(Block { rect: r, size: size * scale, color: [c[0] as f64, c[1] as f64, c[2] as f64, 1.0], layer });
            }
            _ => {}
        }
    }
    // Merge runs of the same colour and size that touch (split words, lines of a paragraph).
    let mut blocks: Vec<Block> = Vec::new();
    for r in runs {
        let near = |b: &Block| {
            let gap = r.size.max(b.size) * 0.9;
            b.layer == r.layer
                && (b.size - r.size).abs() < r.size * 0.15
                && color::contrast(b.color, r.color) < 1.1
                && r.rect[0] <= b.rect[2] + gap
                && b.rect[0] <= r.rect[2] + gap
                && r.rect[1] <= b.rect[3] + gap * 0.6
                && b.rect[1] <= r.rect[3] + gap * 0.6
        };
        match blocks.iter_mut().find(|b| near(b)) {
            Some(b) => {
                b.rect = [b.rect[0].min(r.rect[0]), b.rect[1].min(r.rect[1]), b.rect[2].max(r.rect[2]), b.rect[3].max(r.rect[3])];
            }
            None => blocks.push(r),
        }
    }
    blocks
}

fn without_text(dl: &DisplayList) -> DisplayList {
    let mut d = dl.clone();
    d.ops.retain(|o| !matches!(o, Op::Glyphs { .. }));
    d
}

fn lum_at(px: &[u8], w: u32, x: u32, y: u32) -> f64 {
    let i = ((y * w + x) * 4) as usize;
    color::luminance([px[i] as f64 / 255.0, px[i + 1] as f64 / 255.0, px[i + 2] as f64 / 255.0, 1.0])
}

/// Worst-case contrast of `fg` against the background pixels in `rect` (10th percentile).
fn worst_contrast(px: &[u8], w: u32, h: u32, rect: [f32; 4], fg: color::C) -> Option<f64> {
    let (x0, y0) = (rect[0].max(0.0) as u32, rect[1].max(0.0) as u32);
    let (x1, y1) = ((rect[2].min(w as f32 - 1.0)) as u32, (rect[3].min(h as f32 - 1.0)) as u32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let fl = color::luminance(fg);
    let step = ((x1 - x0) * (y1 - y0) / 600).max(1) as f64;
    let step = step.sqrt().ceil() as u32;
    let mut ratios = Vec::new();
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            let bl = lum_at(px, w, x, y);
            ratios.push((fl.max(bl) + 0.05) / (fl.min(bl) + 0.05));
            x += step;
        }
        y += step;
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ratios.get(ratios.len() / 10).copied()
}

fn area(r: [f32; 4]) -> f32 {
    (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0)
}

fn inter(a: [f32; 4], b: [f32; 4]) -> f32 {
    area([a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])])
}

/// Sampled times: each scene at 40% and 75%, plus transition midpoints.
fn sample_times(ev: &Evaluator) -> Vec<(f32, String)> {
    let scenes = ev.scenes();
    let mut v = Vec::new();
    for (i, s) in scenes.iter().enumerate() {
        let d = s.end - s.start;
        v.push((s.start + d * 0.4, s.id.clone()));
        v.push((s.start + d * 0.75, s.id.clone()));
        if let Some(n) = scenes.get(i + 1) {
            if n.start < s.end - 0.05 {
                v.push(((n.start + s.end) / 2.0, format!("{}→{}", s.id, n.id)));
            }
        }
    }
    v
}

/// [`check`], with paths in the author's own spec (a look's backdrop layers removed).
pub fn check_spec(raw: &serde_json::Value, ev: &Evaluator) -> Vec<Note> {
    let off = crate::looks::layer_offsets(raw);
    let mut n = check(ev);
    for x in &mut n {
        x.path = crate::looks::remap_path(&off, &x.path);
    }
    n
}

/// Run the pixel checks.
pub fn check(ev: &Evaluator) -> Vec<Note> {
    const SCALE: f32 = 0.5;
    let fmt = Format { width: ev.width, height: ev.height, fps: 30 };
    let [top, right, bottom, left] = fmt.safe().map(|x| x as f32 * SCALE);
    let mut notes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (t, where_) in sample_times(ev) {
        let transition = where_.contains('→');
        let (mut dl, tags) = ev.eval_tagged(t);
        pipeline::scale_list(&mut dl, SCALE);
        let who = user_text(ev, dl.ops.len(), &tags);
        let blocks = text_blocks(&dl, &who);
        let (w, h, bg) = pipeline::render_list_rgba(ev, &without_text(&dl));
        let at = |b: &Block| format!("{} (t={t:.1}s {where_}, at {:.0}, {:.0})", b.layer, (b.rect[0] + b.rect[2]) / SCALE / 2.0, (b.rect[1] + b.rect[3]) / SCALE / 2.0);
        let mut push = |key: String, n: Note| {
            if seen.insert(key) {
                notes.push(n);
            }
        };
        for (i, b) in blocks.iter().enumerate() {
            let large = b.size / SCALE >= fmt.width.min(fmt.height) as f32 * 0.03;
            let need = if large { 3.0 } else { 4.5 };
            if let Some(c) = worst_contrast(&bg, w, h, b.rect, b.color) {
                if c + 0.05 < need {
                    push(format!("c{where_}{i}"), Note {
                        severity: if c < 2.0 { "error" } else { "warning" },
                        check: "render-contrast",
                        path: at(b),
                        message: format!("text colour {} has {c:.1}:1 contrast against the pixels behind it (needs {need}:1)", color::to_hex(b.color)),
                        fix: "give the text a backing (box, darker scene), change its colour, or move it off the busy/bright area".into(),
                    });
                }
            }
            if !transition {
                // Camera moves nudge type a little; flag only real overflows (> 1.5% of the frame).
                let (tx, ty) = (w as f32 * 0.015, h as f32 * 0.015);
                let outside_safe = b.rect[0] < left - tx || b.rect[2] > w as f32 - right + tx || b.rect[1] < top - ty || b.rect[3] > h as f32 - bottom + ty;
                if b.rect[0] < -1.0 || b.rect[1] < -1.0 || b.rect[2] > w as f32 + 1.0 || b.rect[3] > h as f32 + 1.0 {
                    push(format!("f{where_}{i}"), Note { severity: "error", check: "render-frame", path: at(b), message: "text is cut by the frame edge".into(), fix: "move it inside the frame or give it a narrower width".into() });
                } else if outside_safe {
                    push(format!("s{where_}{i}"), Note { severity: "warning", check: "render-safe", path: at(b), message: "text extends outside the title-safe area".into(), fix: match fmt.orientation() {
                        "portrait" => "keep it inside the safe margins (top 12%, bottom 20%, sides 8%)",
                        "square" => "keep it inside the safe margins (8% on every side)",
                        _ => "keep it inside the safe margins (top and bottom 9%, sides 7%)",
                    }.into() });
                }
            }
        }
        // Overlaps: in a crossfade both texts are half visible and still read as mush.
        let ov_blocks = if transition { text_blocks_min(&dl, &who, 0.3) } else { blocks.clone() };
        for (i, b) in ov_blocks.iter().enumerate() {
            for (j, o) in ov_blocks.iter().enumerate().skip(i + 1) {
                let ov = inter(b.rect, o.rect);
                if ov > 0.2 * area(b.rect).min(area(o.rect)) {
                    push(format!("o{where_}{i}{j}"), Note {
                        severity: "warning",
                        check: "render-overlap",
                        path: at(b),
                        message: format!("two pieces of text overlap{}", if transition { " during the transition" } else { "" }),
                        fix: if transition { "let the outgoing text exit before the new text enters (exit + a cut, a push or slide, or a fadeBlack), not a crossfade".into() } else { "space them apart, or stack them with the line-count spacing".into() },
                    });
                }
            }
        }
        if blocks.is_empty() && !transition {
            // Nearly empty: almost no luminance variation.
            let n = (w * h) as usize;
            let mut sum = 0.0;
            let mut sq = 0.0;
            for k in (0..n).step_by(97) {
                let l = lum_at(&bg, w, (k as u32) % w, (k as u32) / w);
                sum += l;
                sq += l * l;
            }
            let m = (n / 97).max(1) as f64;
            let var = sq / m - (sum / m).powi(2);
            if var < 0.0004 {
                push(format!("e{where_}"), Note { severity: "warning", check: "render-empty", path: format!("t={t:.1}s {where_}"), message: "the frame is nearly empty (no text, almost no visual variation)".into(), fix: "add a visual or text, or shorten the scene".into() });
            }
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Options;
    use serde_json::json;

    fn ev(spec: serde_json::Value) -> Evaluator {
        let p = crate::spec::parse::from_value(spec).unwrap();
        let opts = Options { fonts: Some(std::sync::Arc::new(crate::text::FontStore::new())), ..Options::default() };
        Evaluator::new(&p.spec, opts).unwrap()
    }

    #[test]
    fn finds_low_contrast_overlap_and_cut_text() {
        let e = ev(json!({ "size": [640, 360], "scenes": [ { "duration": 2, "background": "#C6FF3D", "layers": [
            { "text": "Hard to read", "color": "#FFFFFF", "size": 40 },
            { "text": "Overlapping", "color": "#000000", "size": 40, "y": "52%" },
            { "text": "Off the edge of the frame", "color": "#000000", "size": 40, "x": "95%" }
        ] } ] }));
        let n = check(&e);
        let checks: Vec<&str> = n.iter().map(|x| x.check).collect();
        assert!(checks.contains(&"render-contrast"), "{n:?}");
        assert!(checks.contains(&"render-overlap"), "{n:?}");
        assert!(checks.contains(&"render-frame"), "{n:?}");
    }

    #[test]
    fn clean_frame_has_no_findings() {
        let e = ev(json!({ "size": [640, 360], "scenes": [ { "duration": 2, "layers": [ { "text": "Readable", "size": 40 } ] } ] }));
        let n = check(&e);
        assert!(n.is_empty(), "{n:?}");
    }
}
