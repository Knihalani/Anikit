//! Shader layers end to end: spec → evaluator → renderer pixels.

use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::render_rgba;
use anikit_video::spec::parse_json;

fn build(src: &str) -> Evaluator {
    let p = parse_json(src).unwrap_or_else(|e| panic!("{e:?}"));
    Evaluator::new(&p.spec, Options::default()).unwrap_or_else(|e| panic!("{e}"))
}

fn diags(src: &str) -> Vec<String> {
    let p = parse_json(src).unwrap();
    let (_, d) = Evaluator::build(&p.spec, &Options::default());
    d.iter().map(|d| d.to_string()).collect()
}

/// Pixel (x, y) of the frame at `t`, straight RGBA8.
fn px(ev: &Evaluator, t: f32, x: u32, y: u32) -> [u8; 4] {
    let (w, _, rgba) = render_rgba(ev, t, 1.0);
    let i = ((y * w + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

#[test]
fn generator_layer_draws_and_uniforms_animate() {
    let ev = build(
        r#"{ "size": [64, 32], "scenes": [ { "duration": 2, "layers": [
            { "shader": "uniform float k; // = 0.0\nvoid mainImage(out vec4 c, in vec2 p) { c = vec4(k, p.x / iResolution.x, 0.25, 1.0); }",
              "animate": [ { "to": { "k": 1.0 }, "duration": 1, "ease": "linear" } ] } ] } ] }"#,
    );
    let a = px(&ev, 0.0, 0, 0);
    let b = px(&ev, 1.0, 63, 0);
    assert!(near(a, [0, 2, 64, 255], 1), "{a:?}");
    assert!(near(b, [255, 253, 64, 255], 1), "{b:?}");
    let mid = px(&ev, 0.5, 32, 10);
    assert!(near(mid, [128, 130, 64, 255], 2), "{mid:?}");
}

#[test]
fn spec_uniforms_numbers_vectors_colours() {
    let ev = build(
        r##"{ "size": [16, 16], "scenes": [ { "layers": [
            { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(tint * amount, 1.0); }",
              "uniforms": { "tint": "#ff8000", "amount": 0.5 } } ] } ] }"##,
    );
    assert!(near(px(&ev, 0.1, 8, 8), [128, 64, 0, 255], 1));
}

#[test]
fn filter_reads_the_layers_below() {
    let ev = build(
        r##"{ "size": [32, 32], "background": "#ff0000", "scenes": [ { "layers": [
            { "shape": "rect", "fill": "#0000ff", "x": 8, "y": 16, "width": 16, "height": 32 },
            { "filter": true, "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(1.0 - texture(iChannel0, p / iResolution.xy).rgb, 1.0); }" } ] } ] }"##,
    );
    // Left: blue rect inverted → yellow. Right: red background inverted → cyan.
    assert!(near(px(&ev, 0.1, 4, 16), [255, 255, 0, 255], 1), "{:?}", px(&ev, 0.1, 4, 16));
    assert!(near(px(&ev, 0.1, 28, 16), [0, 255, 255, 255], 1), "{:?}", px(&ev, 0.1, 28, 16));
}

#[test]
fn shader_transition_and_builtin_name() {
    let src = r##"{ "size": [32, 32], "scenes": [
        { "duration": 1, "background": "#ff0000", "layers": [] },
        { "duration": 1, "background": "#0000ff", "layers": [],
          "transition": { "type": "shader", "duration": 1, "ease": "linear",
            "shader": "vec4 transition(vec2 uv) { return mix(getFromColor(uv), getToColor(uv), progress); }" } } ] }"##;
    let ev = build(src);
    // Scenes overlap by the transition (here clamped to 0.5 s): the midpoint is start + 0.25.
    let start = ev.scenes()[1].start;
    let m = px(&ev, start + 0.25, 16, 16);
    assert!(near(m, [128, 0, 128, 255], 2), "{m:?}");
    for name in ["burn", "ripple", "pixelate", "swirl", "ink-bleed", "warp-wipe", "luma-melt", "zoom-blur"] {
        let s = src.replace(r#""type": "shader""#, &format!(r#""type": "{name}""#)).replace("\"shader\": \"vec4 transition(vec2 uv) { return mix(getFromColor(uv), getToColor(uv), progress); }\"", "\"amount\": 0");
        let d = diags(&s);
        assert!(d.iter().all(|d| !d.starts_with("error")), "{name}: {d:?}");
        let ev = build(&s);
        // Fully transitioned at the end.
        let end = ev.duration - 0.01;
        assert!(near(px(&ev, end, 16, 16), [0, 0, 255, 255], 8), "{name}: {:?}", px(&ev, end, 16, 16));
        let mid = px(&ev, ev.scenes()[1].start + 0.25, 16, 16);
        assert_eq!(mid[3], 255, "{name}");
    }
}

#[test]
fn channels_from_layers() {
    let ev = build(
        r##"{ "size": [32, 32], "scenes": [ { "layers": [
            { "shader": "void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, p / iResolution.xy).bgra; }",
              "channels": [ { "layers": [ { "shape": "rect", "fill": "#ff0000", "width": "100%", "height": "100%" } ] } ] } ] } ] }"##,
    );
    assert!(near(px(&ev, 0.1, 16, 16), [0, 0, 255, 255], 1));
}

#[test]
fn builtins_by_name() {
    for e in anikit_shader::library::ALL.iter().filter(|e| e.use_ != anikit_shader::library::Use::Transition) {
        let filter = e.use_ == anikit_shader::library::Use::Filter;
        let s = format!(r#"{{ "size": [48, 27], "scenes": [ {{ "layers": [ {{ "text": "Hi" }}, {{ "shader": "{}", "filter": {filter} }} ] }} ] }}"#, e.name);
        let ev = build(&s);
        let p = px(&ev, 0.5, 24, 13);
        assert_eq!(p[3], 255, "{}", e.name);
    }
}

#[test]
fn errors_are_located_and_suggest() {
    let d = diags(r#"{ "scenes": [ { "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) {\n  c = vec4(smoothStep(0.0, 1.0, p.x));\n}" } ] } ] }"#);
    assert!(d.iter().any(|d| d.contains("scenes[0].layers[0].shader") && d.contains("line 2") && d.contains("smoothstep")), "{d:?}");
    let d = diags(r#"{ "scenes": [ { "layers": [ { "shader": "aurorra" } ] } ] }"#);
    assert!(d.iter().any(|d| d.contains("aurora")), "{d:?}");
    let d = diags(r#"{ "scenes": [ { "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel1, p); }" } ] } ] }"#);
    assert!(d.iter().any(|d| d.contains("iChannel1")), "{d:?}");
}

#[test]
fn static_shader_renders_once() {
    let ev = build(r#"{ "size": [16, 16], "scenes": [ { "duration": 2, "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(0.5); }" } ] } ] }"#);
    assert!(ev.is_static(0.1, 1.9));
    assert_eq!(ev.eval(0.1), ev.eval(1.5));
    let ev = build(r#"{ "size": [16, 16], "scenes": [ { "duration": 2, "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(fract(iTime)); }" } ] } ] }"#);
    assert!(!ev.is_static(0.1, 1.9));
    assert_ne!(ev.eval(0.1), ev.eval(1.5));
}
