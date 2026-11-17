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
        assert!(checks(&s, &t).is_empty());
    }
}
