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

#[test]
fn tagged_eval_identifies_layers() {
    let ev = build(r##"{ "scenes": [ { "layers": [ { "text": "Hello" }, { "component": "button", "props": { "label": "Go" }, "width": 300, "height": 100, "y": "80%" } ] } ],
        "captions": [ { "text": "Words here", "at": 0, "end": 3 } ] }"##);
    let (dl, tags) = ev.eval_tagged(1.0);
    assert_eq!(dl, ev.eval(1.0), "tagging must not change the output");
    let kinds: Vec<(&str, &str)> = tags.iter().map(|t| ev.layer_info(t.layer)).collect();
    assert!(kinds.contains(&("scenes[0].layers[0]", "text")), "{kinds:?}");
    assert!(kinds.iter().any(|k| k.1 == "component:button"), "{kinds:?}");
    assert!(kinds.contains(&("captions", "caption")), "{kinds:?}");
    let text = tags.iter().find(|t| ev.layer_info(t.layer).1 == "text").unwrap();
    assert!(dl.ops[text.ops.clone()].iter().any(|o| matches!(o, Op::Glyphs { .. })));
    // A normal eval doesn't collect tags.
    assert_eq!(ev.eval_tagged(1.0).1.len(), tags.len());
}

// ---------- compositions ----------

const COMP: &str = r##"{ "size": [400, 200], "background": "#000000", "motionBlur": false,
  "compositions": { "badge": { "size": [100, 50], "duration": 2, "background": "#ff0000",
      "layers": [ { "shape": "rect", "width": 50, "height": 50, "x": 75, "fill": "#00ff00",
                    "animate": [ { "from": { "opacity": 0 }, "at": 1, "duration": 0.5 } ] } ] } },
  "scenes": [ { "duration": 4, "layers": [ { "composition": "badge", "x": 0, "y": 0, "anchor": "topLeft", "width": 200, "start": 1 } ] } ] }"##;

fn px(ev: &Evaluator, t: f32, x: u32, y: u32) -> [u8; 3] {
    let (w, _, p) = anikit_video::pipeline::render_rgba(ev, t, 1.0);
    let i = ((y * w + x) * 4) as usize;
    [p[i], p[i + 1], p[i + 2]]
}

#[test]
fn composition_layer_scales_clips_and_maps_time() {
    let ev = build(COMP);
    let c = ev.composition("badge").unwrap();
    assert_eq!((c.width, c.height, c.duration), (100, 50, 2.0));
    // Before the layer starts: nothing.
    assert_eq!(px(&ev, 0.5, 20, 20), [0, 0, 0]);
    // Layer local 0.5 s = composition 0.5 s: red background (scaled 2×, so 200×100), green not yet in.
    assert_eq!(px(&ev, 1.5, 20, 20), [255, 0, 0]);
    assert_eq!(px(&ev, 1.5, 175, 50), [255, 0, 0]);
    // Outside the composition's box: clipped.
    assert_eq!(px(&ev, 1.5, 300, 50), [0, 0, 0]);
    assert_eq!(px(&ev, 1.5, 100, 150), [0, 0, 0]);
    // Composition 1.6 s+ : the green square is in (x 100..200 after scaling).
    assert_eq!(px(&ev, 3.0, 150, 50), [0, 255, 0]);
    // Static spans follow the composition's clock: its fade runs at layer 1..1.5 s = video 2..2.5 s.
    assert!(ev.is_static(1.1, 1.9));
    assert!(!ev.is_static(1.9, 2.6));
    assert!(ev.is_static(2.6, 3.9));
    // Tagged eval names the composition layer for all of its ops.
    let (dl, tags) = ev.eval_tagged(3.0);
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].ops, 0..dl.ops.len());
    assert_eq!(ev.layer_info(tags[0].layer).1, "composition");
}

#[test]
fn composition_loop_and_speed() {
    let src = COMP.replace(r#""start": 1"#, r#""start": 1, "loop": true, "speed": 2"#);
    let ev = build(&src);
    // Layer 1.0 s × 2 = 2.0 → wraps to 0: green hidden again.
    assert_eq!(px(&ev, 2.0, 150, 50), [255, 0, 0]);
    assert!(!ev.is_static(1.1, 3.9));
}

#[test]
fn composition_diagnostics() {
    let e = errors(r##"{ "compositions": { "ui": { "layers": [ { "type": "txt", "text": "a" } ] } },
        "scenes": [ { "layers": [ { "composition": "uii" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("compositions.ui.layers[0].type") && m.contains("'text'")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("scenes[0].layers[0].composition") && m.contains("did you mean 'ui'")), "{e:?}");
    let e = errors(r##"{ "compositions": { "a": { "layers": [ { "composition": "b" } ] }, "b": { "layers": [ { "composition": "a" } ] } },
        "scenes": [ "x" ] }"##);
    assert!(e.iter().any(|m| m.contains("cycle")), "{e:?}");
    // Nested compositions compile in dependency order regardless of name order.
    let ev = build(r##"{ "compositions": { "a": { "size": [10, 10], "layers": [ { "composition": "z" } ] }, "z": { "size": [5, 5], "layers": [ "hi" ] } },
        "scenes": [ { "layers": [ { "composition": "a", "repeat": 2 } ] } ] }"##);
    assert!(ev.composition("a").unwrap().composition("z").is_some());
    let e = errors(r##"{ "compositions": { "ui": { "layers": [ "a" ], "sizee": 3 } }, "scenes": [ "x" ] }"##);
    assert!(e.iter().any(|m| m.contains("compositions.ui.sizee") && m.contains("'size'")), "{e:?}");
}

#[test]
fn post_and_motion_blur_spec() {
    let e = errors(r##"{ "post": "cinematc", "scenes": [ { "post": { "preset": "noir", "bloomTreshold": 0.5 }, "layers": [ "a" ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("error: post:") && m.contains("did you mean 'cinematic'")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("scenes[0].post") && m.contains("did you mean 'bloomThreshold'")), "{e:?}");
    let ev = build(r##"{ "post": "cinematic", "motionBlur": { "shutter": 90 }, "scenes": [ { "layers": [ "a" ] }, { "post": "none", "layers": [ "b" ] } ] }"##);
    assert!(ev.post_at(1.0).is_some() && ev.post_at(5.0).is_none());
    assert!((ev.shutter - 0.25 / 30.0).abs() < 1e-6 && ev.shutter_explicit);
    let ev = build(r##"{ "scenes": [ { "layers": [ { "text": "a", "animate": [ { "to": { "x": 0 }, "duration": 1 } ] } ] } ] }"##);
    assert!(!ev.shutter_explicit && ev.needs_blur(0.5, true) && !ev.needs_blur(0.5, false) && !ev.needs_blur(2.0, true));
    // Stills apply the look.
    let plain = anikit_video::pipeline::render_rgba(&build(r##"{ "size": [64, 36], "scenes": [ "x" ] }"##), 1.0, 1.0).2;
    let graded = anikit_video::pipeline::render_rgba(&build(r##"{ "size": [64, 36], "post": "noir", "scenes": [ "x" ] }"##), 1.0, 1.0).2;
    assert_ne!(plain, graded);
}

#[test]
fn colour_and_logic_expressions() {
    let src = r##"{ "size": [64, 36], "motionBlur": false, "scenes": [ { "duration": 2, "background": "#000000", "layers": [
        { "shape": "rect", "width": 64, "height": 36, "fill": "=t < 1 ? hsl(0, 100, 50) : mix(#0000ff, #00ff00, t - 1)" } ] } ] }"##;
    let ev = build(src);
    let px = |t: f32| {
        let (w, _, p) = anikit_video::pipeline::render_rgba(&ev, t, 1.0);
        let i = ((18 * w + 32) * 4) as usize;
        [p[i], p[i + 1], p[i + 2]]
    };
    let red = px(0.5);
    assert!(red[0] > 240 && red[1] < 15 && red[2] < 15, "{red:?}");
    let blue = px(1.0);
    assert!(blue[2] > 240 && blue[0] < 15, "{blue:?}");
    let late = px(1.99);
    assert!(late[1] > 200 && late[2] < 40, "{late:?}");
    assert!(!ev.is_static(0.2, 0.4));
    let e = errors(r##"{ "scenes": [ { "layers": [ { "shape": "rect", "fill": "=hls(1, 2, 3)" }, { "text": "x", "x": "=level * 100" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("unknown colour function 'hls'")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("add an `audio` track")), "{e:?}");
}

#[test]
fn scripts_generate_layers_and_specs() {
    // An item script: a ring of 12 dots with per-dot colour and staggered entrances.
    let ev = build(r##"{ "scenes": [ { "layers": [
        { "script": [
            "let out = [];",
            "for k in 0..12 {",
            "  let a = k * 30.0;",
            "  out.push(#{ shape: \"circle\", width: 40, x: w / 2 + 300 * cos(a.to_radians()), y: h / 2 + 300 * sin(a.to_radians()),",
            "              fill: hsl(a, 80, 60), enter: #{ preset: \"popIn\", delay: k * 0.05 } });",
            "}",
            "out" ] },
        "Centre" ] } ] }"##);
    let dl = ev.eval(2.0);
    let fills = dl.ops.iter().filter(|o| matches!(o, Op::Fill { .. } | Op::RoundRect { .. })).count();
    assert!(fills >= 12, "{fills}");
    // A top-level script rewrites the spec (here: one scene per word).
    let ev = build(r##"{ "script": "spec.scenes = []; for w in [\"one\", \"two\", \"three\"] { spec.scenes.push(w) } spec", "scenes": [] }"##);
    assert_eq!(ev.scenes().len(), 3);
    // Deterministic helpers.
    let a = build(r##"{ "scenes": [ { "layers": [ { "script": "[#{ text: fmt(rand(7) * 100.0, 3) }]" } ] } ] }"##).eval(1.0);
    let b = build(r##"{ "scenes": [ { "layers": [ { "script": "[#{ text: fmt(rand(7) * 100.0, 3) }]" } ] } ] }"##).eval(1.0);
    assert_eq!(a, b);
    // Errors carry the path and the script's line; runaway loops stop.
    let e = errors(r##"{ "scenes": [ { "layers": [ { "script": "let x = ;" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("scenes[0].layers[0].script") && m.contains("line 1")), "{e:?}");
    let e = errors(r##"{ "scenes": [ { "layers": [ { "script": "loop { }" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("script")), "{e:?}");
}

#[test]
fn draw_layer_primitives_in_frame_pixels() {
    let ev = build(r##"{ "size": [64, 36], "motionBlur": false, "scenes": [ { "background": "#000000", "layers": [ { "draw": [
        { "circle": [16, 18, 8], "fill": "#ff0000" },
        { "rect": [40, 10, 16, 16], "fill": "#0000ff", "stroke": "#ffffff", "width": 1 },
        { "points": [[2, 34], [62, 34]], "colors": ["#00ff00", "#ff00ff"], "width": 3 },
        { "d": "M30 2 L34 2 L32 6 Z", "fill": "$accent" } ] } ] } ] }"##);
    let (w, _, p) = anikit_video::pipeline::render_rgba(&ev, 1.0, 1.0);
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [p[i], p[i + 1], p[i + 2]]
    };
    assert_eq!(px(16, 18), [255, 0, 0]);
    assert_eq!(px(48, 18), [0, 0, 255]);
    let left = px(4, 34);
    let right = px(60, 34);
    assert!(left[1] > 180 && left[0] < 80, "{left:?}");
    assert!(right[0] > 180 && right[2] > 180, "{right:?}");
    assert_eq!(px(2, 2), [0, 0, 0]);
    let e = errors(r##"{ "scenes": [ { "layers": [ { "draw": [ { "circle": [1, 2, 3], "line": [0, 0, 1, 1] }, { "points": [[0, 0], [1, 1]], "colors": ["#fff"] }, { "d": "M0 0 Q" } ] } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("draw[0]") && m.contains("exactly one")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("draw[1].colors") && m.contains("one per point")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("draw[2].d")), "{e:?}");
}

#[test]
fn expressions_follow_generated_music() {
    // Generated music gives the exact grid: at 120 bpm, beat = 2 * t.
    let ev = build(r##"{ "music": { "style": "lofi", "bpm": 120 }, "motionBlur": false, "scenes": [ { "duration": 4, "layers": [
        { "shape": "rect", "width": 10, "height": 10, "x": "=100 * beat", "y": "=100 + 100 * level" } ] } ] }"##);
    let x_at = |t: f32| {
        let dl = ev.eval(t);
        dl.ops.iter().find_map(|o| match o { Op::RoundRect { transform, .. } => Some(transform[4]), _ => None }).unwrap()
    };
    let (a, b) = (x_at(1.0), x_at(2.0));
    assert!((b - a - 200.0).abs() < 1.0, "{a} {b}");
}

#[test]
fn scripts_anywhere_helpers_and_update() {
    // A script inside draw items (any array) splices; helpers return SVG paths and palette colours.
    let ev = build(r##"{ "size": [200, 100], "motionBlur": false, "scenes": [ { "background": "#000000", "layers": [ { "draw": [
        { "script": "let o = []; for k in 0..5 { o.push(#{ d: path_star(20 + k * 40, 50, 15, 6, 5), fill: palette(\"viridis\", k / 4.0) }) } o" },
        { "circle": [100, 90, 4], "fill": "#fff" } ] } ] } ] }"##);
    let dl = ev.eval(1.0);
    assert_eq!(dl.ops.iter().filter(|o| matches!(o, Op::Fill { .. })).count(), 6);
    // Deterministic helpers.
    let s = r##"{ "scenes": [ { "layers": [ { "script": "[#{ text: fmt(fbm(1.5, 2.5, 4) + noise3(1, 2, 3) + ease(\"outBack\", 0.5), 6) + palette(\"sunset\", 0.3) + path_circle(0, 0, 10) }]" } ] } ] }"##;
    assert_eq!(build(s).eval(1.0), build(s).eval(1.0));
    // Per-frame update scripts set props each frame.
    let ev = build(r##"{ "motionBlur": false, "scenes": [ { "duration": 2, "layers": [
        { "shape": "rect", "width": 10, "height": 10, "update": "#{ x: 100 + 400 * t, fill: if t > 1 { \"#ff0000\" } else { \"#00ff00\" } }" } ] } ] }"##);
    let x_at = |t: f32| ev.eval(t).ops.iter().find_map(|o| match o { Op::RoundRect { transform, .. } => Some(transform[4]), _ => None }).unwrap();
    assert!((x_at(1.0) - x_at(0.5) - 200.0).abs() < 0.5);
    assert!(!ev.is_static(0.1, 0.2));
    let e = errors(r##"{ "scenes": [ { "layers": [ { "text": "a", "update": "#{ rotaton: t }" }, { "text": "b", "update": "42" } ] } ] }"##);
    assert!(e.iter().any(|m| m.contains("'rotaton'") && m.contains("did you mean 'rotation'")), "{e:?}");
    assert!(e.iter().any(|m| m.contains("layers[1].update") && m.contains("map")), "{e:?}");
}

/// Cost of one `update` script per layer per frame (printed; run with --release for real numbers).
#[test]
fn update_script_cost() {
    let ev = build(r##"{ "scenes": [ { "duration": 2, "layers": [
        { "shape": "rect", "width": 10, "height": 10, "update": "#{ x: 960 + 300 * sin(t * 2.0), y: 540 + 100 * cos(t * 3.0), rotation: t * 45.0, fill: hsl(t * 90, 80, 60) }" } ] } ] }"##);
    let plain = build(r##"{ "scenes": [ { "duration": 2, "layers": [ { "shape": "rect", "width": 10, "height": 10 } ] } ] }"##);
    let mut dl = anikit_video::dl::DisplayList::default();
    let time = |e: &Evaluator, dl: &mut anikit_video::dl::DisplayList| {
        let t0 = std::time::Instant::now();
        for k in 0..2000 {
            e.eval_into((k % 60) as f32 / 30.0, dl);
        }
        t0.elapsed().as_secs_f64() / 2000.0 * 1e6
    };
    let (a, b) = (time(&ev, &mut dl), time(&plain, &mut dl));
    eprintln!("update script: {:.1} µs per layer per frame", a - b);
}

#[test]
fn text_expressions_render_numbers() {
    let ev = build(r##"{ "scenes": [ { "duration": 3, "layers": [ { "text": "=t < 1 ? 10 : floor(t * 100)", "style": "title" } ] } ] }"##);
    let glyphs = |t: f32| ev.eval(t).ops.iter().map(|o| match o { Op::Glyphs { glyphs, .. } => glyphs.len(), _ => 0 }).sum::<usize>();
    assert_eq!(glyphs(0.5), 2); // "10"
    assert_eq!(glyphs(2.5), 3); // "250"
    assert!(!ev.is_static(0.5, 2.5));
}

#[test]
fn cut_on_the_music_grid_and_stack() {
    let ev = build(r##"{ "music": { "style": "lofi", "bpm": 120 }, "cut": "2 bars", "transition": "cut", "scenes": [
        { "stack": true, "layers": [ { "text": "A", "style": "title" }, { "text": "B" } ] }, { "duration": "1 bar", "layers": [ "C" ] }, { "duration": "4b", "layers": [ "D" ] } ] }"##);
    let s = ev.scenes();
    assert_eq!(s.len(), 3);
    assert!((s[0].end - s[0].start - 4.0).abs() < 1e-3 && (s[1].end - s[1].start - 2.0).abs() < 1e-3 && (s[2].end - s[2].start - 2.0).abs() < 1e-3);
    // Stacked: the two text layers are at different heights (one column), not both centred.
    let ys: Vec<f32> = ev.eval(1.0).ops.iter().filter_map(|o| match o { Op::Glyphs { transform, .. } => Some(transform[5]), _ => None }).collect();
    assert!(ys.len() >= 2 && (ys[0] - ys[1]).abs() > 20.0, "{ys:?}");
}
