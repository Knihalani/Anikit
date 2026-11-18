//! Render-time taste tips from the craft research (`docs/video/craft/`):
//! an empty opening frame, an end mark held too briefly to remember, and
//! eye-trace that jumps across the frame on fast cuts. Advice, never verdicts.

use crate::critique::Note;
use crate::review::{Samples, Timeline};

/// Luma (Rec.709 weights) of an RGB8 frame.
fn luma(f: &[u8]) -> Vec<f32> {
    f.chunks_exact(3).map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32).collect()
}

/// Standard deviation of luma: near zero is an empty or flat frame.
fn spread(f: &[u8]) -> f32 {
    let y = luma(f);
    let n = y.len().max(1) as f32;
    let m = y.iter().sum::<f32>() / n;
    (y.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / n).sqrt()
}

/// Where the eye goes: the centroid (0–1) of pixels that stand out from the
/// frame's mean luma, weighted by how much they stand out.
fn salient_x(f: &[u8], w: usize) -> Option<f32> {
    let y = luma(f);
    let n = y.len().max(1) as f32;
    let m = y.iter().sum::<f32>() / n;
    let (mut sx, mut sw) = (0.0f32, 0.0f32);
    for (i, v) in y.iter().enumerate() {
        let d = (v - m).abs();
        if d > 24.0 {
            sx += (i % w) as f32 / w.max(1) as f32 * d;
            sw += d;
        }
    }
    (sw > 0.0).then(|| sx / sw)
}

/// Edge-energy layout of a frame on a 24×14 grid, normalised to sum 1: where
/// the structure sits, independent of colour (a palette flip keeps the layout).
fn layout_sig(f: &[u8], w: usize, h: usize) -> Vec<f32> {
    let (gw, gh) = (24usize, 14usize);
    let y = luma(f);
    let mut g = vec![0f32; gw * gh];
    let mut n = vec![0f32; gw * gh];
    for py in 0..h {
        for px in 0..w {
            let c = (py * gh / h.max(1)) * gw + px * gw / w.max(1);
            g[c] += y[py * w + px];
            n[c] += 1.0;
        }
    }
    for (v, k) in g.iter_mut().zip(&n) {
        *v /= k.max(1.0);
    }
    let mut e = vec![0f32; (gw - 1) * (gh - 1)];
    for gy in 0..gh - 1 {
        for gx in 0..gw - 1 {
            let a = g[gy * gw + gx];
            e[gy * (gw - 1) + gx] = (g[gy * gw + gx + 1] - a).abs() + (g[(gy + 1) * gw + gx] - a).abs();
        }
    }
    let t: f32 = e.iter().sum::<f32>().max(1e-6);
    e.iter().map(|v| v / t).collect()
}

/// Six frames spaced evenly through the piece (what a viewer scrubbing it, or a jury, sees):
/// how many adjacent pairs share one composition. Calibrated on the REFS set, where no
/// professional piece had an adjacent pair above 0.8, against rejected pieces with 3–4.
pub fn same_layout_pairs(s: &Samples) -> usize {
    let n = s.frames.len();
    if n < 6 {
        return 0;
    }
    let (w, h) = (s.w as usize, s.h as usize);
    let sigs: Vec<Vec<f32>> = (0..6).map(|i| layout_sig(&s.frames[((i as f32 + 0.5) / 6.0 * n as f32) as usize], w, h)).collect();
    sigs.windows(2).filter(|p| p[0].iter().zip(&p[1]).map(|(a, b)| a.min(*b)).sum::<f32>() > 0.8).count()
}

/// Tips from the sampled frames and the measured timeline.
pub fn checks(s: &Samples, tl: &Timeline) -> Vec<Note> {
    let mut notes = Vec::new();
    let w = s.w as usize;
    let mut tip = |check: &'static str, path: String, message: String, fix: &str| notes.push(Note { severity: "tip", check, path, message, fix: fix.into() });
    // Opening frame: viewers judge within ~0.25 s, so an empty or black first frame spends that moment on nothing.
    if let Some(f0) = s.frames.first() {
        if spread(f0) < 3.0 {
            tip("hook", "t=0.0s".into(), "the first frame is flat or empty (a fade from black or a blank ground)".into(),
                "feeds recall content within ~0.25 s; consider opening mid-motion on the most interesting image (docs/video/craft/storytelling.md §2.1)");
        }
    }
    // End mark: memory encoding needs ~1.5–2.5 s of attention on the key image.
    if let Some(last) = tl.shots.last() {
        if tl.shots.len() >= 3 && *last < 1.2 {
            tip("ending", "timeline".into(), format!("the last shot holds {last:.1} s; a final image or mark tends to need ~1.5–2.5 s to be remembered"),
                "consider holding the payoff or mark longer, with a small button after it (docs/video/craft/storytelling.md §2.4)");
        }
    }
    // Eye-trace on fast cuts: in fast runs, editors keep the point of interest near where the eye already is.
    let mut fast = 0;
    let mut jumps = 0;
    for (k, &c) in tl.cuts.iter().enumerate() {
        let next = tl.shots.get(k + 1).copied().unwrap_or(f32::MAX);
        let prev = tl.shots.get(k).copied().unwrap_or(f32::MAX);
        if next.min(prev) > 0.8 {
            continue;
        }
        let i = (c / s.step).round() as usize;
        if i == 0 || i + 1 >= s.frames.len() {
            continue;
        }
        if let (Some(a), Some(b)) = (salient_x(&s.frames[i - 1], w), salient_x(&s.frames[i + 1], w)) {
            fast += 1;
            if (a - b).abs() > 0.35 {
                jumps += 1;
            }
        }
    }
    if fast >= 4 && jumps * 2 > fast {
        tip("eye-trace", "timeline".into(), format!("{jumps} of {fast} fast cuts move the point of interest across more than a third of the frame"),
            "in fast runs the eye can't re-find the subject in time; consider keeping the focal point near where the last shot left it, and saving the jump for one deliberate jolt (docs/video/craft/editing.md)");
    }
    let same = same_layout_pairs(s);
    if same >= 2 {
        notes.push(Note { severity: "tip", check: "layout", path: "timeline".into(),
            message: format!("{same} of 5 pairs of evenly spaced frames share one composition: scrubbed through, it reads as one layout with the words swapped"),
            fix: "plan a sequence of different frames: change scale, angle, crop and what fills the frame from beat to beat; repeat a layout only as a deliberate rhyme (docs/video/craft/judge-r1-review.md)".into() });
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: usize, h: usize, blob_x: Option<usize>) -> Vec<u8> {
        let mut f = vec![20u8; w * h * 3];
        if let Some(bx) = blob_x {
            for y in h / 3..2 * h / 3 {
                for x in bx..(bx + w / 8).min(w) {
                    let i = (y * w + x) * 3;
                    f[i..i + 3].copy_from_slice(&[240, 240, 240]);
                }
            }
        }
        f
    }

    fn tl(shots: Vec<f32>) -> Timeline {
        let mut cuts = Vec::new();
        let mut t = 0.0;
        for d in &shots[..shots.len() - 1] {
            t += d;
            cuts.push(t);
        }
        Timeline { median: 0.5, longest: 2.0, same: 0.0, energy: vec![], on_beat: None, shots, cuts }
    }

    #[test]
    fn flags_empty_open_short_end_and_jumping_eye() {
        let (w, h) = (64, 36);
        // 6 fps samples over 4 s: the subject alternates sides on every cut.
        let mut frames = vec![frame(w, h, None)];
        for i in 1..25 {
            frames.push(frame(w, h, Some(if (i / 3) % 2 == 0 { 2 } else { w - 10 })));
        }
        let s = Samples { step: 1.0 / 6.0, w: w as u32, h: h as u32, coarse: frames.clone(), frames };
        let t = tl(vec![0.5; 8].into_iter().chain([0.5]).collect());
        let n = checks(&s, &t);
        let has = |c: &str| n.iter().any(|x| x.check == c);
        assert!(has("hook") && has("ending") && has("eye-trace"), "{:?}", n.iter().map(|x| x.check).collect::<Vec<_>>());
        // A steady subject, a strong first frame and a held end: nothing to say.
        let frames: Vec<Vec<u8>> = (0..25).map(|_| frame(w, h, Some(w / 2))).collect();
        let s = Samples { step: 1.0 / 6.0, w: w as u32, h: h as u32, coarse: frames.clone(), frames };
        let t = tl(vec![0.5, 0.5, 0.5, 0.5, 2.0]);
        // (one composition held throughout is the `layout` tip's business)
        assert!(checks(&s, &t).iter().all(|n| n.check == "layout"));
    }

    #[test]
    fn same_layout_through_the_piece() {
        let (w, h) = (96, 54);
        // A headline bar in the same place in every frame, with the colour flipping.
        let frame = |bg: u8, fg: u8| {
            let mut f = vec![bg; w * h * 3];
            for y in 10..20 {
                for x in 8..60 {
                    let i = (y * w + x) * 3;
                    f[i..i + 3].copy_from_slice(&[fg, fg, fg]);
                }
            }
            f
        };
        let frames: Vec<Vec<u8>> = (0..30).map(|i| if i % 2 == 0 { frame(20, 230) } else { frame(230, 20) }).collect();
        let s = Samples { step: 0.2, w: w as u32, h: h as u32, coarse: frames.clone(), frames };
        assert!(same_layout_pairs(&s) >= 4);
        // The subject moves around the frame: different compositions.
        let frames: Vec<Vec<u8>> = (0..30)
            .map(|i| {
                let mut f = vec![20u8; w * h * 3];
                let (x0, y0) = ((i * 17) % (w - 20), (i * 7) % (h - 12));
                for y in y0..y0 + 10 {
                    for x in x0..x0 + 18 {
                        let k = (y * w + x) * 3;
                        f[k..k + 3].copy_from_slice(&[230, 230, 230]);
                    }
                }
                f
            })
            .collect();
        let s = Samples { step: 0.2, w: w as u32, h: h as u32, coarse: frames.clone(), frames };
        assert!(same_layout_pairs(&s) <= 1, "{}", same_layout_pairs(&s));
    }
}
