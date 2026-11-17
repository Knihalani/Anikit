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
