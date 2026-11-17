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
    let md = [include_str!("../../../docs/video/spec.md"), include_str!("../../../docs/video/spec-reference.md")].concat();
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

#[test]
fn long_text_wraps_by_default() {
    let ev = build(r#"{ "size": "9:16", "scenes": [ { "layers": [ { "text": "A rather long sentence that would never fit across a vertical frame at title size", "style": "title" } ] } ] }"#);
    let dl = ev.eval(1.0);
    let ys: std::collections::BTreeSet<i32> = dl.ops.iter().filter_map(|o| match o { Op::Glyphs { glyphs, .. } => Some(glyphs.iter().map(|g| g.y as i32).collect::<Vec<_>>()), _ => None }).flatten().collect();
    assert!(ys.len() >= 3, "expected several lines, got {ys:?}");
}

#[test]
fn clip_shapes() {
    let ev = build(r#"{ "scenes": [ { "layers": [ { "shape": "rect", "width": 400, "height": 400, "clip": { "shape": "circle", "width": 200, "height": 200 } } ] } ] }"#);
    let dl = ev.eval(1.0);
    let clip = dl.ops.iter().find_map(|o| match o { Op::PushLayer { clip: Some((p, _)), .. } => Some(kurbo::Shape::bounding_box(p)), _ => None }).expect("clip");
    assert!((clip.x0 - 100.0).abs() < 0.5 && (clip.x1 - 300.0).abs() < 0.5, "{clip:?}");
}


#[test]
fn contextual_key_suggestions() {
    // `durration` on a scene → duration; `colr` on a layer → color; `ease` on a scene is not valid there.
    let e = errors(r##"{ "scenes": [ { "durration": 2, "ease": "x", "layers": [ { "text": "x", "colr": "#fff", "enter": { "preset": "fadeUp", "dellay": 1 } } ] } ] }"##);
    assert!(e.iter().any(|m| m == "warning: scenes[0].durration: unknown key (ignored), did you mean 'duration'?"), "{e:?}");
    assert!(e.iter().any(|m| m == "warning: scenes[0].layers[0].colr: unknown key (ignored), did you mean 'color'?"), "{e:?}");
    assert!(e.iter().any(|m| m == "warning: scenes[0].layers[0].enter.dellay: unknown key (ignored), did you mean 'delay'?"), "{e:?}");
    assert!(e.iter().any(|m| m == "warning: scenes[0].ease: 'ease' is not valid here (ignored)"), "{e:?}");
}

#[test]
fn static_spans() {
    let ev = build(r#"{ "scenes": [ { "duration": 3, "layers": [ { "text": "x", "enter": { "preset": "fadeUp", "duration": 0.5 } } ] },
        { "duration": 3, "transition": { "type": "crossfade", "duration": 0.5 }, "layers": [ { "shape": "rect", "rotation": "=t*10" } ] } ] }"#);
    assert!(!ev.is_static(0.0, 0.4));
    assert!(ev.is_static(1.0, 2.0));
    assert!(!ev.is_static(2.4, 2.7)); // transition
    assert!(!ev.is_static(3.5, 4.0)); // expression of t
    // The static claim must agree with the display lists.
    assert_eq!(ev.eval(1.0), ev.eval(2.0));
}

#[test]
fn is_static_never_lies() {
    let ev = build(include_str!("../../../docs/video/examples/showcase.json"));
    let mut claims = 0;
    for i in 0..ev.frames() - 1 {
        let (a, b) = (i as f32 / ev.fps, (i + 1) as f32 / ev.fps);
        if ev.is_static(a, b) {
            claims += 1;
            assert_eq!(ev.eval(a), ev.eval(b), "frame {i}");
        }
    }
    eprintln!("{claims} static claims");
}

#[test]
fn caption_word_highlight() {
    let ev = build(r##"{ "scenes": [ { "duration": 3, "layers": [] } ],
        "captions": { "highlight": "#ff0000", "color": "#ffffff", "cues": [ { "text": "", "at": 0.5, "end": 2.5, "words": [["Hello", 0.5, 1.0], ["world", 1.0, 2.0]] } ] } }"##);
    let reds = |t: f32| -> usize {
        ev.eval(t).ops.iter().filter(|o| matches!(o, Op::Glyphs { paint: anikit_video::dl::Paint::Solid(c), .. } if c.0[0] > 0.95 && c.0[1] < 0.1)).count()
    };
    assert_eq!(ev.eval(0.2).ops.iter().filter(|o| matches!(o, Op::Glyphs { .. })).count(), 0, "nothing before the first word");
    assert!(reds(0.8) >= 1, "the spoken word is highlighted");
    assert!(reds(1.5) >= 1);
    assert!(!ev.is_static(0.7, 0.75));
}

#[test]
fn depth_of_field_and_parallax() {
    let ev = build(r##"{ "scenes": [ { "duration": 2,
        "camera": { "aperture": 10, "animate": [ { "to": { "focus": 2, "x": 100 }, "at": 0, "duration": 1, "ease": "linear" } ] },
        "layers": [ { "shape": "rect", "id": "near", "anchor": "topLeft", "x": 0, "y": 0, "width": 10, "height": 10 },
                    { "shape": "rect", "id": "far", "anchor": "topLeft", "x": 0, "y": 0, "width": 10, "height": 10, "depth": 2 } ] } ] }"##);
    let info = |t: f32| {
        let dl = ev.eval(t);
        let mut blurs = Vec::new();
        let mut xs = Vec::new();
        for o in &dl.ops {
            match o {
                Op::PushLayer { filters, .. } => blurs.push(filters.iter().find_map(|f| match f { anikit_video::dl::Filter::Blur(r) => Some(*r), _ => None }).unwrap_or(0.0)),
                Op::RoundRect { transform, .. } => xs.push(transform[4]),
                _ => {}
            }
        }
        (blurs, xs)
    };
    // t=0: focus 0 → near sharp (no layer), far blurred by |2-0|*10 = 20.
    let (b, _) = info(0.0);
    assert_eq!(b, vec![20.0]);
    // t=1: focus 2 → far sharp, near blurred 20; camera x=100: near moves -100, far moves -100/(1+2).
    let (b, xs) = info(1.0);
    assert_eq!(b, vec![20.0]);
    assert!((xs[0] + 100.0).abs() < 0.01 && (xs[1] + 33.333).abs() < 0.01, "{xs:?}");
}

/// Token-efficiency metric (≈ chars / 4): the cheat sheet an agent reads, and
/// what common videos cost to write. Run with `--nocapture` to see the numbers.
#[test]
fn token_metric() {
    let sheet = include_str!("../../../docs/video/spec.md");
    assert!(sheet.lines().count() <= 150, "the cheat sheet must stay under 150 lines");
    let examples = [
        ("hello", r#"{"scenes":[{"layers":[{"text":"Hello, world","enter":"fadeUp"}]}]}"#),
        ("3 scenes with a chart", r#"{"scenes":["Launch day | Everything ships",{"layers":[{"text":"3× faster","style":"title","animate":"rise"},"Built in Rust"]},{"transition":"push","layers":[{"component":"bar-chart","props":{"values":[3,7,5,9]}}]}]}"#),
        ("vertical teaser + captions + sfx", r#"{"size":"9:16","scenes":[{"duration":4,"layers":[{"text":"Ship faster","style":"title","split":"words","enter":{"preset":"fadeUp","stagger":0.12}}],"audio":[{"sfx":"whoosh","at":0.1}]}],"captions":{"background":"$accent","cues":[{"text":"Ship faster with Anikit","at":0.5,"end":3.5}]}}"#),
    ];
    eprintln!("cheat sheet: {} lines, ≈{} tokens", sheet.lines().count(), sheet.len() / 4);
    for (name, src) in examples {
        let ev = build(src);
        assert!(ev.diagnostics.is_empty(), "{name}: {:?}", ev.diagnostics);
        eprintln!("{name:34} ≈{:4} tokens → {:.1} s, {} scene(s)", src.len() / 4, ev.duration, ev.scenes().len());
    }
}

#[test]
fn moving_transitions_keep_backdrops_in_place() {
    // Full-frame backdrop layers must not be translated by push/slide/zoom (their edges would show).
    for kind in ["push", "slide", "zoom", "morphCut"] {
        let ev = build(&format!(
            r##"{{ "scenes": [ {{ "duration": 2, "layers": [ {{ "shape": "rect", "width": "100%", "height": "100%", "fill": "#204080" }}, {{ "text": "A" }} ] }},
                {{ "duration": 2, "transition": {{ "type": "{kind}", "duration": 1 }}, "layers": [ {{ "shape": "rect", "width": "100%", "height": "100%", "fill": "#802040" }}, {{ "text": "B" }} ] }} ] }}"##
        ));
        let dl = ev.eval(1.5);
        for o in &dl.ops {
            if let Op::RoundRect { w, transform, .. } = o {
                if *w >= 1919.0 {
                    assert!(transform[4].abs() < 0.01 && transform[5].abs() < 0.01 && (transform[0] - 1.0).abs() < 1e-4, "{kind}: backdrop moved: {transform:?}");
                }
            }
        }
    }
}
