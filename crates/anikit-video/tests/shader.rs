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

#[test]
fn named_composition_channel() {
    let ev = build(
        r##"{ "size": [32, 32], "compositions": { "card": { "size": [16, 16], "duration": 1, "layers": [ { "shape": "rect", "fill": "#00ff00", "width": "100%", "height": "100%" } ] } },
            "scenes": [ { "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, p / iResolution.xy).gbra; }", "channels": [ "#card" ] } ] } ] }"##,
    );
    assert!(near(px(&ev, 0.1, 16, 16), [255, 0, 0, 255], 1), "{:?}", px(&ev, 0.1, 16, 16));
    let d = diags(r##"{ "compositions": { "card": { "duration": 1, "layers": [ "x" ] } }, "scenes": [ { "layers": [ { "shader": "aurora", "channels": [ "#cards" ] } ] } ] }"##);
    assert!(d.iter().any(|d| d.contains("card")), "{d:?}");
}

#[test]
fn uniform_names_and_misplaced_keys() {
    let d = diags(r#"{ "scenes": [ { "layers": [ { "shader": "aurora", "animate": [ { "to": { "intensty": 2 } } ] } ] } ] }"#);
    assert!(d.iter().any(|d| d.contains("intensty") && d.contains("intensity")), "{d:?}");
    let d = diags(r#"{ "scenes": [ { "layers": [ { "text": "hi", "uniforms": { "a": 1 } } ] } ] }"#);
    assert!(d.iter().any(|d| d.starts_with("warning") && d.contains("uniforms")), "{d:?}");
    let d = diags(r#"{ "scenes": [ { "layers": [ { "src": "missing.glsl" } ] } ] }"#);
    assert!(d.iter().all(|d| !d.contains("only applies")), "{d:?}");
}

/// Static spans never hide a change: animated uniforms and time-reading shaders aren't static.
#[test]
fn is_static_never_lies_for_shaders() {
    for src in [
        r#"{ "size": [16, 16], "scenes": [ { "duration": 2, "layers": [ { "shader": "uniform float k;\nvoid mainImage(out vec4 c, in vec2 p) { c = vec4(k); }", "animate": [ { "to": { "k": 1 }, "at": 0.5, "duration": 1 } ] } ] } ] }"#,
        r#"{ "size": [16, 16], "scenes": [ { "duration": 2, "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(k); }", "uniforms": { "k": "=t * 0.3" } } ] } ] }"#,
        r##"{ "size": [16, 16], "compositions": { "m": { "duration": 2, "layers": [ { "shape": "rect", "fill": "#fff", "width": 8, "height": 8, "animate": [ { "to": { "x": 12 }, "duration": 2 } ] } ] } },
            "scenes": [ { "duration": 2, "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, p / iResolution.xy); }", "channels": [ "#m" ] } ] } ] }"##,
    ] {
        let ev = build(src);
        let frames: Vec<_> = (0..20).map(|i| render_rgba(&ev, 0.1 + i as f32 * 0.09, 1.0).2).collect();
        for i in 1..frames.len() {
            let (a, b) = (0.1 + (i - 1) as f32 * 0.09, 0.1 + i as f32 * 0.09);
            if ev.is_static(a, b) {
                assert_eq!(frames[i - 1], frames[i], "claimed static {a}..{b}: {src}");
            }
        }
        assert!(frames.windows(2).any(|w| w[0] != w[1]), "nothing changed: {src}");
    }
}

#[test]
fn audio_and_beat_uniforms_follow_the_music() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/video/pulse");
    let src = r#"{ "size": [16, 8], "audio": [ { "src": "pulse.flac" } ], "scenes": [ { "duration": 6, "layers": [
        { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(iAudio.x, iAudio.y, iBeat.x, 1.0) * (p.x < 8.0 ? 1.0 : 0.0) + vec4(iBeat.z / 255.0, iBeat.y, iBeat.w, 1.0) * (p.x < 8.0 ? 0.0 : 1.0); }" } ] } ] }"#;
    let p = parse_json(src).unwrap();
    let opts = Options { base_dir: root, ..Options::default() };
    let ev = Evaluator::new(&p.spec, opts).unwrap_or_else(|e| panic!("{e}"));
    let mut levels = Vec::new();
    let mut beats = Vec::new();
    for i in 0..60 {
        let t = i as f32 * 0.1;
        levels.push(px(&ev, t, 0, 0)[0]);
        beats.push(px(&ev, t, 12, 0)[0]);
    }
    assert!(levels.iter().max().unwrap() - levels.iter().min().unwrap() > 60, "level should move with the music: {levels:?}");
    assert!(beats.windows(2).all(|w| w[1] >= w[0]) && beats[59] >= 4, "beat number counts up: {beats:?}");
    // Same input, same output.
    assert_eq!(px(&ev, 2.3, 0, 0), px(&ev, 2.3, 0, 0));
}

#[test]
fn audio_and_beat_uniforms_follow_generated_music() {
    // Generated music: iBeat uses its exact grid (120 bpm: beat n starts at n / 2 s)
    // and iAudio its rendered mix.
    let src = r#"{ "size": [16, 8], "music": { "style": "house", "bpm": 120, "seed": 2 }, "scenes": [ { "duration": 6, "layers": [
        { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(iAudio.x, iBeat.z / 255.0, iBeat.y, 1.0); }" } ] } ] }"#;
    assert!(diags(src).iter().all(|m| !m.contains("need music")), "{:?}", diags(src));
    let ev = build(src);
    let mut levels = Vec::new();
    for i in 0..48 {
        let t = 0.05 + i as f32 * 0.125;
        let p = px(&ev, t, 0, 0);
        levels.push(p[0]);
        // Beat number: beats at 0, 0.5, 1.0… so n = floor(2t) + 1 beats have started.
        assert_eq!(p[1] as i32, (2.0 * t).floor() as i32 + 1, "beat number at {t}");
    }
    assert!(levels.iter().max().unwrap() - levels.iter().min().unwrap() > 40, "level should move with the music: {levels:?}");
}

#[test]
fn audio_uniforms_without_music_warn() {
    let d = diags(r#"{ "size": [16, 8], "scenes": [ { "duration": 1, "layers": [ { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(iBeat.x); }" } ] } ] }"#);
    assert!(d.iter().any(|m| m.contains("need music")), "{d:?}");
}

#[test]
fn buffer_feedback_counts_frames_and_seeks_deterministically() {
    // Buffer A adds 1 per frame to its previous value (float precision, unclamped);
    // B reads A this frame; the image shows B / 255.
    let src = r#"{ "size": [8, 4], "fps": 10, "scenes": [ { "duration": 5, "layers": [
        { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(texelFetch(iChannel0, ivec2(p), 0).r / 255.0, 0.0, 0.0, 1.0); }",
          "channels": ["B"],
          "buffers": {
            "A": { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(texelFetch(iChannel0, ivec2(p), 0).r + 1.0, 0.0, 0.0, 1.0); }", "channels": ["A"] },
            "B": { "shader": "void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, p / iResolution.xy); }", "channels": ["A"] } } } ] } ] }"#;
    // Cold seek straight to frame 30 (t = 3.0): A has run 31 times.
    let ev = build(src);
    assert_eq!(px(&ev, 3.0, 2, 1)[0], 31);
    // Sequential frames from a fresh evaluator give the same values.
    let ev2 = build(src);
    for f in 0..=30 {
        let v = px(&ev2, f as f32 / 10.0, 2, 1)[0];
        assert_eq!(v, f + 1, "frame {f}");
    }
    // Going back works too.
    assert_eq!(px(&ev2, 1.0, 2, 1)[0], 11);
}

#[test]
fn buffer_names_are_checked() {
    let d = diags(r#"{ "size": [8, 4], "scenes": [ { "duration": 1, "layers": [
        { "shader": "void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, p / iResolution.xy); }", "channels": ["C"],
          "buffers": { "A": { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(1.0); }", "channels": ["Z"] }, "E": { "shader": "void mainImage(out vec4 c, in vec2 p) { c = vec4(1.0); }" } } } ] } ] }"#);
    let all = d.join("\n");
    assert!(all.contains("buffers.E") && all.contains("\"A\", \"B\", \"C\" or \"D\""), "{all}");
    assert!(all.contains("not 'Z'"), "{all}");
    assert!(all.contains("reads buffer C"), "{all}");
}

/// A shader inside a composition renders at the size it is shown (×2 for
/// filtering, capped at its declared size); shown more than once, it renders
/// once, at the largest of those sizes.
#[test]
fn composition_shaders_render_at_displayed_size() {
    let spec = |extra: &str| {
        format!(
            r#"{{ "size": [400, 200], "compositions": {{ "c": {{ "size": [400, 200], "duration": 1, "layers": [
                {{ "shader": "void mainImage(out vec4 o, in vec2 p) {{ o = vec4(p.x / iResolution.x, 0.5, 0.2, 1.0); }}" }} ] }} }},
               "scenes": [ {{ "duration": 1, "layers": [ {{ "composition": "c", "width": 400, "height": 200, "x": 100, "y": 50, "scale": 0.2 }}{extra} ] }} ] }}"#
        )
    };
    let small = build(&spec(""));
    render_rgba(&small, 0.5, 1.0);
    let s = &small.shader_stats()[0];
    // 0.2 × 2 = 0.4, rounded up to ⅛ steps: 0.5 of 400 × 200.
    assert_eq!((s.width, s.height, s.renders), (200, 100, 1), "{s:?}");
    let both = build(&spec(r#", { "composition": "c", "width": 400, "height": 200, "x": 200, "y": 100 }"#));
    render_rgba(&both, 0.5, 1.0);
    let s = &both.shader_stats()[0];
    assert_eq!((s.width, s.height, s.renders), (400, 200, 1), "{s:?}");
}

#[test]
fn many_static_uniforms_leave_slots_for_animated_ones() {
    // 16 static floats (more than the animation slots) and one animated uniform
    // declared last: no warning, the static values apply, and k animates.
    let mut decl = String::new();
    let mut sum = String::from("0.0");
    let mut given = Vec::new();
    for i in 0..16 {
        decl += &format!("uniform float s{i};\n");
        sum += &format!(" + s{i}");
        given.push(format!("\"s{i}\": 0.01"));
    }
    let src = format!(
        r#"{{ "size": [8, 8], "scenes": [ {{ "duration": 2, "layers": [ {{ "shader": "{decl}uniform float k;\nvoid mainImage(out vec4 c, in vec2 p) {{ c = vec4({sum}, k, 0.0, 1.0); }}", "uniforms": {{ {} }}, "animate": [ {{ "to": {{ "k": 1.0 }}, "duration": 2, "ease": "linear" }} ] }} ] }} ] }}"#,
        given.join(", ")
    )
    .replace('\n', "\\n");
    let d = diags(&src);
    assert!(d.iter().all(|m| !m.contains("at most 12")), "{d:?}");
    let ev = build(&src);
    let a = px(&ev, 0.0, 4, 4);
    let b = px(&ev, 1.5, 4, 4);
    assert!((a[0] as i32 - 41).abs() <= 1, "16 × 0.01 = 0.16 → 41: {a:?}");
    assert!(b[1] > a[1] + 100, "k animates: {a:?} → {b:?}");
}

#[test]
fn too_many_animated_uniform_floats_is_an_error() {
    // 13 animated floats: the 13th can't animate, which must stop the render, not pass as a warning.
    let mut decl = String::new();
    let mut anim = Vec::new();
    for i in 0..13 {
        decl += &format!("uniform float a{i};\n");
        anim.push(format!("\"a{i}\": 1.0"));
    }
    let src = format!(
        r#"{{ "size": [8, 8], "scenes": [ {{ "duration": 2, "layers": [ {{ "shader": "{decl}void mainImage(out vec4 c, in vec2 p) {{ c = vec4(a0 + a12); }}", "animate": [ {{ "to": {{ {} }}, "duration": 2 }} ] }} ] }} ] }}"#,
        anim.join(", ")
    )
    .replace('\n', "\\n");
    let d = diags(&src);
    assert!(d.iter().any(|m| m.starts_with("error") && m.contains("at most 12")), "{d:?}");
}
