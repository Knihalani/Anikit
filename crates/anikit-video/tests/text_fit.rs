//! `fit: true` on text shrinks (never grows) the size so the widest line fits.

use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline;
use anikit_video::spec::parse_json;

/// Horizontal ink extent (first and last column with a bright pixel).
fn ink_x(spec: &str) -> (u32, u32) {
    let ev = Evaluator::new(&parse_json(spec).unwrap().spec, Options::default()).unwrap();
    let (w, h, px) = pipeline::render_rgba(&ev, 0.5, 1.0);
    let (mut lo, mut hi) = (w, 0);
    for y in 0..h {
        for x in 0..w {
            if px[((y * w + x) * 4) as usize] > 128 {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
    }
    (lo, hi)
}

fn spec(layer: &str) -> String {
    format!(r##"{{ "size": [540, 960], "scenes": [ {{ "duration": 1, "background": "#000", "layers": [ {layer} ] }} ] }}"##)
}

#[test]
fn fit_true_shrinks_to_width_and_keeps_breaks() {
    let big = r##"{ "text": "FREQUENCY", "size": 160, "weight": 900, "color": "#fff" }"##;
    let (a, b) = ink_x(&spec(big));
    assert!(a == 0 || b >= 539, "without fit the word overflows: {a}..{b}");

    let fit = r##"{ "text": "FREQUENCY", "size": 160, "weight": 900, "color": "#fff", "fit": true, "width": 440 }"##;
    let (a, b) = ink_x(&spec(fit));
    assert!(b - a <= 440 && b - a > 380, "fits 440 px: {a}..{b}");

    // Default target: 85% of the parent (459 px); two lines stay two lines.
    let two = r##"{ "text": "ALL\nNIGHTLONG", "size": 160, "weight": 900, "color": "#fff", "fit": true }"##;
    let (a, b) = ink_x(&spec(two));
    assert!(b - a <= 460 && b - a > 400, "{a}..{b}");

    // Never grows.
    let small = r##"{ "text": "HI", "size": 40, "color": "#fff", "fit": true, "width": 500 }"##;
    let (a, b) = ink_x(&spec(small));
    assert!(b - a < 80, "{a}..{b}");

    // Images keep their modes; text rejects other words.
    assert!(Evaluator::new(&parse_json(&spec(r##"{ "text": "X", "fit": "cover" }"##)).unwrap().spec, Options::default()).is_err());
}
