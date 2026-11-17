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
