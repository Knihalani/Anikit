//! Motion-graphics layer effects: repeater, echo, motion paths, mattes, text animator.

use anikit_video::dl::{Blend, Op};
use anikit_video::eval::{Evaluator, Options};
use anikit_video::spec::parse_json;

fn ev(src: &str) -> Evaluator {
    let p = parse_json(src).unwrap_or_else(|e| panic!("{e:?}"));
    Evaluator::new(&p.spec, Options::default()).unwrap_or_else(|e| panic!("{e}"))
}

fn count(ops: &[Op], f: impl Fn(&Op) -> bool) -> usize {
    ops.iter().filter(|o| f(o)).count()
}

#[test]
fn repeater_and_echo_multiply_draws() {
    let one = ev(r#"{ "scenes": [ { "layers": [ { "shape": "rect", "width": 10, "height": 10 } ] } ] }"#);
    let rep = ev(r#"{ "scenes": [ { "layers": [ { "shape": "rect", "width": 10, "height": 10, "repeat": { "count": 12, "offset": { "rotate": 30 } } } ] } ] }"#);
    let echo = ev(r#"{ "scenes": [ { "layers": [ { "shape": "rect", "width": 10, "height": 10, "x": "=t*500", "echo": { "count": 4, "delay": 0.1 } } ] } ] }"#);
    let shapes = |e: &Evaluator| count(&e.eval(1.0).ops, |o| matches!(o, Op::RoundRect { .. } | Op::Fill { .. }));
    let base = shapes(&one);
    assert_eq!(shapes(&rep), 12 * base);
    assert_eq!(shapes(&echo), 5 * base);
    // Spec-level numeric repeat still expands copies.
    let n = ev(r#"{ "scenes": [ { "layers": [ { "shape": "rect", "width": 10, "height": 10, "repeat": 3, "x": "=$i*100" } ] } ] }"#);
    assert_eq!(shapes(&n), 3 * base);
}

#[test]
fn motion_path_moves_layer() {
    let e = ev(r#"{ "scenes": [ { "duration": 2, "layers": [ { "shape": "rect", "width": 10, "height": 10, "path": "M100 100 L1100 100" } ] } ] }"#);
    let x = |t: f32| match e.eval(t).ops.iter().find_map(|o| if let Op::RoundRect { transform, .. } = o { Some(transform[4]) } else { None }) {
        Some(x) => x,
        None => panic!("no rect"),
    };
    // Linear over the layer's 2 s: halfway at 1 s (minus half the 10 px box).
    assert!((x(1.0) - 595.0).abs() < 2.0, "{}", x(1.0));
    assert!(x(0.0) < x(0.5) && x(0.5) < x(1.5));
}

#[test]
fn track_matte_composes() {
    let e = ev(r##"{ "scenes": [ { "layers": [
        { "shape": "rect", "width": 400, "height": 400, "fill": "#f00", "matte": { "layer": "m", "mode": "alphaInverted" } },
        { "id": "m", "shape": "circle", "width": 100, "height": 100 } ] } ] }"##);
    let ops = e.eval(0.5).ops;
    assert_eq!(count(&ops, |o| matches!(o, Op::PushLayer { blend: Blend::DestOut, .. })), 1);
    // The matte layer is not drawn on its own: one fill for the content, one for the matte.
    assert_eq!(count(&ops, |o| matches!(o, Op::RoundRect { .. } | Op::Fill { .. })), 2);
    let bad = parse_json(r#"{ "scenes": [ { "layers": [ { "shape": "rect", "matte": "nope" }, { "id": "m", "shape": "circle" } ] } ] }"#).unwrap();
    let err = Evaluator::new(&bad.spec, Options::default()).err().expect("unknown matte id is an error");
    assert!(err.to_string().contains("nope"), "{err}");
}

#[test]
fn deterministic_frames() {
    let src = r#"{ "scenes": [ { "layers": [
        { "shape": "circle", "width": 200, "height": 200, "wigglePath": { "amount": 12 }, "zigzag": { "size": 6, "ridges": 12 } },
        { "text": "Hello motion", "animator": { "y": 40, "opacity": 0, "order": "random", "seed": 4 } } ] } ] }"#;
    let (a, b) = (ev(src), ev(src));
    assert_eq!(format!("{:?}", a.eval(0.7).ops), format!("{:?}", b.eval(0.7).ops));
}

fn rect_x(e: &Evaluator, t: f32) -> f32 {
    e.eval(t).ops.iter().find_map(|o| if let Op::RoundRect { transform, .. } = o { Some(transform[4]) } else { None }).expect("rect")
}

#[test]
fn inertia_overshoots_and_settles() {
    let src = |extra: &str| format!(r#"{{ "scenes": [ {{ "duration": 3, "layers": [ {{ "shape": "rect", "x": 100, "width": 10, "height": 10, {extra}
        "animate": {{ "to": {{ "x": 1100 }}, "duration": 0.3, "ease": "linear" }} }} ] }} ] }}"#);
    let plain = ev(&src(""));
    let sprung = ev(&src(r#""inertia": "bouncy","#));
    // Lags behind while moving, overshoots after, and comes to rest on the target.
    assert!(rect_x(&sprung, 0.15) < rect_x(&plain, 0.15));
    let peak = (30..90).map(|k| rect_x(&sprung, k as f32 / 60.0)).fold(f32::MIN, f32::max);
    assert!(peak > rect_x(&plain, 1.0) + 20.0, "peak {peak}");
    assert!((rect_x(&sprung, 2.9) - rect_x(&plain, 2.9)).abs() < 1.0);
    // Deterministic: a fresh evaluator gives the same frame, in any order.
    let again = ev(&src(r#""inertia": "bouncy","#));
    assert_eq!(rect_x(&again, 0.7), rect_x(&sprung, 0.7));
}

#[test]
fn smear_stretches_fast_moves_only() {
    let e = ev(r#"{ "scenes": [ { "duration": 2, "layers": [ { "shape": "rect", "x": 100, "width": 40, "height": 40, "smear": true,
        "animate": { "to": { "x": 1800 }, "duration": 0.3, "ease": "linear" } } ] } ] }"#);
    let sx = |t: f32| e.eval(t).ops.iter().find_map(|o| if let Op::RoundRect { transform, .. } = o { Some(transform[0]) } else { None }).unwrap();
    assert!(sx(0.15) > 1.5, "{}", sx(0.15));
    assert!((sx(1.5) - 1.0).abs() < 1e-3);
}

#[test]
fn idea_transitions_render_both_scenes() {
    for tr in [
        r#"{ "type": "zoomThrough", "layer": "w" }"#,
        r#"{ "type": "reveal", "shape": "star", "amount": 90 }"#,
        r#"{ "type": "reveal", "shape": "blob" }"#,
        r#"{ "type": "blinds", "count": 6 }"#,
        r#"{ "type": "split", "direction": "up" }"#,
        r#"{ "type": "whip" }"#,
    ] {
        let src = format!(
            r##"{{ "scenes": [ {{ "duration": 1, "background": "#123456", "layers": [ {{ "id": "w", "text": "ONE", "size": 200 }} ] }},
                {{ "duration": 1, "background": "#654321", "transition": {tr}, "layers": [ {{ "text": "TWO", "size": 200 }} ] }} ] }}"##
        );
        let e = ev(&src);
        // Mid-transition: glyphs from both scenes are in the frame.
        let ops = e.eval(0.85).ops;
        let glyphs = count(&ops, |o| matches!(o, Op::Glyphs { .. }));
        assert!(glyphs >= 2, "{tr}: {glyphs} glyph runs");
        let a = format!("{:?}", e.eval(0.85).ops);
        assert_eq!(a, format!("{:?}", ev(&src).eval(0.85).ops), "{tr}: deterministic");
    }
    let bad = parse_json(r#"{ "scenes": [ { "layers": ["a"] }, { "transition": "zoomThrough", "layers": ["b"] } ] }"#).unwrap();
    assert!(Evaluator::new(&bad.spec, Options::default()).is_err(), "zoomThrough without a layer is an error");
}

#[test]
fn posterize_holds_poses_and_named_curves_parse() {
    let e = ev(r#"{ "fps": 30, "scenes": [ { "duration": 2, "layers": [ { "shape": "rect", "x": 100, "width": 10, "height": 10, "posterizeTime": 10,
        "animate": { "to": { "x": 1100 }, "duration": 1, "ease": "glide" } } ] } ] }"#);
    // 10 poses per second: frames 0.30 and 0.3667 share a pose, 0.4 is the next one.
    assert_eq!(rect_x(&e, 0.30), rect_x(&e, 0.3667));
    assert!(rect_x(&e, 0.40) > rect_x(&e, 0.3667));
    for name in ["snap", "land", "launch", "material", "emphasizedDecelerate", "productive", "expressive", "expressiveExit"] {
        ev(&format!(r#"{{ "scenes": [ {{ "layers": [ {{ "text": "x", "animate": {{ "from": {{ "x": 0 }}, "ease": "{name}" }} }} ] }} ] }}"#));
    }
}

#[test]
fn group_drag_trails_children() {
    let e = ev(r#"{ "scenes": [ { "duration": 2, "layers": [ { "type": "group", "x": 960, "y": 540, "width": 1920, "height": 1080, "drag": 0.1,
        "animate": { "to": { "x": 1460 }, "duration": 0.5, "ease": "linear" },
        "layers": [ { "shape": "rect", "x": 100, "y": 100, "width": 10, "height": 10 }, { "shape": "rect", "x": 100, "y": 300, "width": 10, "height": 10 } ] } ] } ] }"#);
    let xs = |t: f32| -> Vec<f32> { e.eval(t).ops.iter().filter_map(|o| if let Op::RoundRect { transform, .. } = o { Some(transform[4]) } else { None }).collect() };
    let mid = xs(0.25);
    assert!(mid[1] < mid[0] - 50.0, "second child lags: {mid:?}");
    let end = xs(1.5);
    assert!((end[1] - end[0]).abs() < 1e-3, "and catches up: {end:?}");
}

#[test]
fn stroke_gradient_along_follows_trim() {
    let src = |trim: f32| format!(r##"{{ "scenes": [ {{ "layers": [ {{ "shape": "path", "d": "M0 0 L1000 0", "fill": "none", "trimEnd": {trim},
        "stroke": {{ "width": 10, "along": ["#f00", "#00f"] }} }} ] }} ] }}"##);
    let strokes = |t: f32| count(&ev(&src(t)).eval(0.5).ops, |o| matches!(o, Op::Stroke { paint: anikit_video::dl::Paint::Linear { .. }, .. }));
    let full = strokes(1.0);
    assert!(full > 50, "{full}");
    let half = strokes(0.5);
    assert!(half < full * 2 / 3 && half > full / 3, "{half} of {full}");
}

#[test]
fn carried_layer_travels_between_layouts() {
    let e = ev(r##"{ "scenes": [
        { "duration": 1, "layers": [ { "id": "m", "shape": "rect", "x": 100, "y": 100, "width": 100, "height": 100, "fill": "#f00" } ] },
        { "duration": 1, "transition": { "type": "crossfade", "duration": 0.5, "ease": "linear", "carry": ["m"] },
          "layers": [ { "id": "m", "shape": "rect", "x": 1100, "y": 100, "width": 300, "height": 300, "fill": "#f00" } ] } ] }"##);
    // Halfway through the transition (0.5..1.0), copies sit halfway between the two layouts at the mean size.
    let rects: Vec<[f32; 6]> = e.eval(0.75).ops.iter().filter_map(|o| if let Op::RoundRect { transform, w, .. } = o { if *w == 100.0 || *w == 300.0 { Some(*transform) } else { None } } else { None }).collect();
    assert!(!rects.is_empty());
    for t in &rects {
        // Centre of the drawn box in frame space.
        let w = if (t[0] - 2.0).abs() < 0.05 { 100.0 } else { 300.0 };
        let cx = t[0] * w * 0.5 + t[4];
        assert!((cx - 600.0).abs() < 5.0, "centre {cx} ({t:?})");
    }
}
