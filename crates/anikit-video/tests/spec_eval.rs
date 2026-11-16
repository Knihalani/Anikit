use anikit_video::dl::Op;
use anikit_video::eval::{Evaluator, Options};
use anikit_video::spec::{parse_json, parse_yaml, Severity};

fn build(src: &str) -> Evaluator {
    let p = parse_json(src).unwrap_or_else(|e| panic!("{e:?}"));
    Evaluator::new(&p.spec, Options::default()).unwrap_or_else(|e| panic!("{e}"))
}

fn errors(src: &str) -> Vec<String> {
    let p = match parse_json(src) {
        Ok(p) => p,
        Err(e) => return e.iter().map(|d| d.to_string()).collect(),
    };
    let (_, mut d) = Evaluator::build(&p.spec, &Options::default());
    d.extend(p.warnings);
    d.iter().map(|d| d.to_string()).collect()
}

#[test]
fn smallest_video() {
    let ev = build(r##"{ "scenes": [ { "layers": [ { "text": "Hello, world", "enter": "fadeUp" } ] } ] }"##);
    assert_eq!((ev.width, ev.height, ev.fps), (1920, 1080, 30.0));
    assert_eq!(ev.duration, 3.0);
    assert_eq!(ev.frames(), 90);
    let a = ev.eval(0.0);
    let b = ev.eval(2.0);
    // At t=0 the text is fully transparent (fade from 0), so nothing is drawn.
    assert!(a.ops.iter().all(|o| !matches!(o, Op::Glyphs { .. })));
    assert!(b.ops.iter().any(|o| matches!(o, Op::Glyphs { .. })));
}

#[test]
fn determinism() {
    let src = include_str!("../../../docs/video/examples/showcase.json");
    let ev = build(src);
    for &t in &[0.0, 0.7, 1.33, 3.2, 4.1, 6.0] {
        assert_eq!(ev.eval(t), ev.eval(t));
    }
    // eval_into reuse gives the same result.
    let mut dl = Default::default();
    ev.eval_into(3.2, &mut dl);
    assert_eq!(dl, ev.eval(3.2));
}

#[test]
fn messages() {
    let e = errors(r##"{ "scenes": [ { "layers": [ { "type": "txt", "text": "x" } ] } ] }"##);
    assert!(e.iter().any(|m| m == "error: scenes[0].layers[0].type: unknown layer type 'txt', did you mean 'text'?"), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "layers": [ { "text": "x", "animate": [ { "to": { "x": 5 }, "ease": "easeOutQuartic" } ] } ] } ] }"##);
    assert!(e.iter().any(|m| m == "error: scenes[0].layers[0].animate[0].ease: unknown easing 'easeOutQuartic', did you mean 'outQuart'?"), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "layers": [ { "text": "x", "colour": "#fff" } ] } ] }"##);
    assert!(e.iter().any(|m| m == "warning: scenes[0].layers[0].colour: unknown key (ignored), did you mean 'color'?"), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "duration": 2, "layers": [ { "text": "x", "start": 4.5 } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("after the scene ends")), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "layers": [ { "text": "x", "enter": "fadeup" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("did you mean 'fadeUp'")), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "layers": [ { "text": "x", "color": "$primay" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("did you mean '$primary'")), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "layers": [ { "shape": "rect", "x": "=sinn(t)" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("did you mean 'sin'")), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "layers": [ { "shape": "rect", "opacity": [1] } ] } ] }"##);
    assert!(e.iter().any(|m| m.starts_with("error: scenes[0].layers[0].opacity")), "{e:?}");
    let e = errors(r##"{ "scenes": [ { } ] }"##);
    assert!(e.iter().any(|m| m.contains("missing 'layers'")), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "transition": "slid", "layers": [] } , { "transition": {"type":"slid"}, "layers": [] } ] }"##);
    assert!(e.iter().any(|m| m.contains("unknown transition 'slid', did you mean 'slide'")), "{e:?}");
}

#[test]
fn yaml() {
    let p = parse_yaml("size: 9:16\nscenes:\n  - layers:\n      - text: Hi\n").unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    assert_eq!((ev.width, ev.height), (1080, 1920));
}

#[test]
fn timeline_and_transitions() {
    let ev = build(r##"{ "scenes": [ { "duration": 2, "layers": [] }, { "duration": 3, "transition": { "type": "slide", "duration": 0.5 }, "layers": [] }, { "duration": 1, "transition": "cut", "layers": [] } ] }"##);
    let s = ev.scenes();
    assert_eq!((s[1].start, s[1].end), (1.5, 4.5));
    assert_eq!((s[2].start, s[2].end), (4.5, 5.5));
    assert_eq!(ev.duration, 5.5);
}

#[test]
fn tweens_hold_and_relative() {
    let ev = build(r##"{ "scenes": [ { "layers": [ { "shape": "rect", "x": 100, "y": 100, "width": 10, "height": 10, "anchor": "topLeft",
        "animate": [ { "from": { "x": "+50" }, "at": 1, "duration": 1, "ease": "linear" }, { "to": { "x": "*2" }, "at": 2, "duration": 0.5 } ] } ] } ] }"##);
    let x = |t: f32| match &ev.eval(t).ops[0] {
        Op::RoundRect { transform, .. } => transform[4],
        o => panic!("{o:?}"),
    };
    assert_eq!(x(0.0), 150.0); // holds the from value before the tween
    assert_eq!(x(1.5), 125.0);
    assert_eq!(x(2.0), 100.0);
    assert_eq!(x(2.9), 200.0);
}

#[test]
fn box_layout() {
    let ev = build(r##"{ "scenes": [ { "layers": [ { "type": "box", "x": 0, "y": 0, "anchor": "topLeft", "style": { "gap": 10, "padding": 5 },
        "children": [ { "shape": "rect", "width": 100, "height": 50 }, { "shape": "rect", "width": 30, "height": 20 } ] } ] } ] }"##);
    let rects: Vec<[f32; 6]> = ev.eval(1.0).ops.iter().filter_map(|o| match o { Op::RoundRect { transform, .. } => Some(*transform), _ => None }).collect();
    assert_eq!(rects.len(), 2);
    assert_eq!((rects[0][4], rects[0][5]), (5.0, 5.0));
    assert_eq!((rects[1][4], rects[1][5]), (115.0, 20.0)); // centred on the cross axis
}

#[test]
fn schema_is_valid_json() {
    let s = anikit_video::spec::schema_json();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert!(v["properties"]["scenes"].is_object());
}

#[test]
fn all_doc_examples_parse() {
    // Every ```json block in spec.md that is a whole spec must compile without errors.
    let md = include_str!("../../../docs/video/spec.md");
    let mut n = 0;
    for block in md.split("```json").skip(1) {
        let src = block.split("```").next().unwrap().trim();
        let is_spec = serde_json::from_str::<serde_json::Value>(src).map(|v| v.get("scenes").is_some()).unwrap_or(false);
        if !is_spec {
            continue;
        }
        let p = parse_json(src).unwrap_or_else(|e| panic!("{src}\n{e:?}"));
        let (_, d) = Evaluator::build(&p.spec, &Options { load_assets: false, ..Default::default() });
        let errs: Vec<_> = d.iter().filter(|d| d.severity == Severity::Error && !d.message.contains("not found")).collect();
        assert!(errs.is_empty(), "{src}\n{errs:?}");
        assert!(p.warnings.is_empty(), "{src}\n{:?}", p.warnings);
        n += 1;
    }
    assert!(n >= 3);
}


#[test]
fn shorthand() {
    let ev = build(r##"{ "scenes": [ "Launch day | Everything ships", { "layers": [ "plain text", { "text": "x", "animate": "rise" }, { "shape": "rect", "animate": { "to": { "x": 10 } } } ] } ] }"##);
    assert_eq!(ev.scenes().len(), 2);
    assert!(ev.diagnostics.is_empty(), "{:?}", ev.diagnostics);
    let e = errors(r#"{ "scenes": [ { "layers": [ { "text": "x", "animate": "rize" } ] } ] }"#);
    assert!(e.iter().any(|m| m.contains("did you mean 'rise'") || m.contains("did you mean")), "{e:?}");
}
