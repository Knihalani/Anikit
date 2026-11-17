//! Graphic design: grid cells, squircle/blob/arc/pattern shapes, tone and halftone treatments.

use anikit_video::dl::{Filter, Op};
use anikit_video::eval::{Evaluator, Options};
use anikit_video::spec::parse_json;

fn ev(src: &str) -> Evaluator {
    let p = parse_json(src).unwrap_or_else(|e| panic!("{e:?}"));
    Evaluator::new(&p.spec, Options::default()).unwrap_or_else(|e| panic!("{e}"))
}

fn first_fill_xf(e: &Evaluator) -> [f64; 6] {
    e.eval(0.5).ops.iter().find_map(|o| match o {
        Op::RoundRect { transform, .. } | Op::Fill { transform, .. } => Some(*transform),
        _ => None,
    }).map(|t| t.map(|v| v as f64)).expect("a fill")
}

#[test]
fn cells_place_layers_on_the_grid() {
    // 12 columns over 1920 − 2×96 with 24 px gutters: (1728 − 11×24) / 12 = 122 px columns; 8 rows: (972 − 7×24) / 8 = 100.5 px rows.
    let e = ev(r##"{ "grid": { "columns": 12, "rows": 8, "margin": [54, 96], "gutter": 24, "areas": { "side": "9-12/2-8" } },
        "scenes": [ { "layers": [ { "cell": "side", "shape": "rect", "fill": "#f00" } ] } ] }"##);
    let ops = e.eval(0.5).ops;
    let (x, y, w, h) = ops.iter().find_map(|o| if let Op::RoundRect { x, y, w, h, transform, .. } = o { Some((transform[4] + x, transform[5] + y, *w, *h)) } else { None }).unwrap();
    assert!((x - (96.0 + 8.0 * 146.0)).abs() < 0.5, "x {x}");
    assert!((y - (54.0 + 100.5 + 24.0)).abs() < 0.5, "y {y}");
    assert!((w - (4.0 * 122.0 + 3.0 * 24.0)).abs() < 0.5, "w {w}");
    assert!((h - (7.0 * 100.5 + 6.0 * 24.0)).abs() < 0.5, "h {h}");
    let _ = first_fill_xf;
    // Errors name the problem and suggest areas.
    let bad = parse_json(r#"{ "grid": { "areas": { "head": "1-6/1-2" } }, "scenes": [ { "layers": [ { "cell": "haed", "shape": "rect" } ] } ] }"#).unwrap();
    let err = Evaluator::new(&bad.spec, Options::default()).err().expect("bad cell is an error").to_string();
    assert!(err.contains("head"), "{err}");
    let nogrid = parse_json(r#"{ "scenes": [ { "layers": [ { "cell": "1-2", "shape": "rect" } ] } ] }"#).unwrap();
    assert!(Evaluator::new(&nogrid.spec, Options::default()).err().unwrap().to_string().contains("grid"));
}

#[test]
fn shapes_and_patterns_compile_to_one_fill() {
    for s in [r#""shape": "squircle""#, r#""shape": "blob", "wobble": 0.5"#, r#""shape": "arc", "innerRadius": 0.5"#, r#""pattern": "truchet 30""#, r#""pattern": { "kind": "dots", "spacing": 12, "ramp": [0, 1] }"#] {
        let e = ev(&format!(r#"{{ "scenes": [ {{ "layers": [ {{ {s}, "width": 400, "height": 300 }} ] }} ] }}"#));
        let n = e.eval(0.5).ops.iter().filter(|o| matches!(o, Op::Fill { .. })).count();
        assert_eq!(n, 1, "{s}");
    }
    let bad = parse_json(r#"{ "scenes": [ { "layers": [ { "pattern": "dotz" } ] } ] }"#).unwrap();
    assert!(Evaluator::new(&bad.spec, Options::default()).err().unwrap().to_string().contains("dots"));
}

#[test]
fn treatments_become_layer_filters() {
    let e = ev(r##"{ "scenes": [ { "layers": [ { "shape": "circle", "width": 300, "height": 300,
        "duotone": ["#102040", "#ffd000"], "posterize": 3, "halftone": { "cell": 8, "angle": 15, "ink": "#111", "paper": "#fff" } } ] } ] }"##);
    let ops = e.eval(0.5).ops;
    let filters = ops.iter().find_map(|o| if let Op::PushLayer { filters, .. } = o { Some(filters.clone()) } else { None }).expect("a filtered layer");
    assert!(matches!(filters[0], Filter::Tone(_)) && matches!(filters[1], Filter::Halftone(_)), "{filters:?}");
    if let Filter::Tone(t) = &filters[0] {
        let m = t.map.as_ref().unwrap();
        // Posterised into 3 steps: the map holds exactly 3 colours, darkest first.
        let mut cs: Vec<[u8; 3]> = m.to_vec();
        cs.dedup();
        assert_eq!(cs.len(), 3, "{cs:?}");
        assert_eq!(cs[0], [0x10, 0x20, 0x40]);
    }
}

#[test]
fn stepped_time_and_boil() {
    let x_at = |e: &Evaluator, t: f32| e.eval(t).ops.iter().find_map(|o| if let Op::RoundRect { transform, .. } = o { Some(transform[4]) } else { None }).unwrap();
    let smooth = ev(r#"{ "scenes": [ { "duration": 2, "layers": [ { "shape": "rect", "width": 10, "height": 10, "x": "=100 + t * 600" } ] } ] }"#);
    let twos = ev(r#"{ "fps": 24, "scenes": [ { "duration": 2, "layers": [ { "shape": "rect", "width": 10, "height": 10, "x": "=100 + t * 600", "step": "twos" } ] } ] }"#);
    assert!(x_at(&smooth, 0.50) != x_at(&smooth, 0.54));
    // On twos at 24 fps: held for 1/12 s.
    assert_eq!(x_at(&twos, 0.50), x_at(&twos, 0.53));
    assert!(x_at(&twos, 0.50) != x_at(&twos, 0.59));
    let boil = ev(r#"{ "scenes": [ { "duration": 2, "layers": [ { "shape": "rect", "width": 10, "height": 10, "x": 500, "boil": { "amount": 4, "fps": 8, "seed": 3 } } ] } ] }"#);
    let (a, b, c) = (x_at(&boil, 0.01), x_at(&boil, 0.05), x_at(&boil, 0.30));
    assert_eq!(a, b, "held between redraws");
    assert!(a != c && (a - 495.0).abs() <= 4.01 && (c - 495.0).abs() <= 4.01, "{a} {c}");
    assert_eq!(x_at(&boil, 0.30), c, "deterministic");
}

#[test]
fn cutout_wraps_the_layer_in_a_rimmed_clipped_piece() {
    let e = ev(r##"{ "scenes": [ { "layers": [ { "shape": "rect", "width": 400, "height": 300, "fill": "#123", "rotation": 5,
        "cutout": { "edge": "torn", "rim": 8, "seed": 2 } } ] } ] }"##);
    let ops = e.eval(0.5).ops;
    // A rim (with its printed shadow) and the clipped content.
    let shadows = ops.iter().filter(|o| matches!(o, Op::PushLayer { filters, .. } if filters.iter().any(|f| matches!(f, Filter::Shadow { .. })))).count();
    let clips = ops.iter().filter(|o| matches!(o, Op::PushLayer { clip: Some(_), .. })).count();
    assert_eq!((shadows, clips), (1, 1), "{ops:?}");
    let bad = parse_json(r#"{ "scenes": [ { "layers": [ { "shape": "rect", "cutout": "ripped" } ] } ] }"#).unwrap();
    assert!(Evaluator::new(&bad.spec, Options::default()).err().unwrap().to_string().contains("cutout"));
}
