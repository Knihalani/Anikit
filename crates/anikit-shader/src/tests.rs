//! Per-feature tests: each shades a tiny canvas and checks exact values.

use super::*;

fn render(src: &str, w: u32, h: u32) -> Vec<[f32; 4]> {
    let s = Shader::compile(src).unwrap_or_else(|e| panic!("{e}"));
    s.render_f32(&Frame::new(w, h, 1.0))
}

/// Run `body` on a 4×2 canvas; returns all pixels (row-major, top row first).
fn all(body: &str) -> Vec<[f32; 4]> {
    render(&format!("void mainImage(out vec4 c, in vec2 p) {{ {body} }}"), 4, 2)
}

fn one(body: &str) -> [f32; 4] {
    all(body)[0]
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol * (1.0 + b.abs())
}

#[test]
fn constants_and_fragcoord() {
    assert_eq!(one("c = vec4(0.25, 0.5, 0.75, 1.0);"), [0.25, 0.5, 0.75, 1.0]);
    // fragCoord: pixel centres, y up (row 0 is the top → y = 1.5 on a 2-high canvas).
    let v = all("c = vec4(p, iResolution.xy);");
    assert_eq!(v[0], [0.5, 1.5, 4.0, 2.0]);
    assert_eq!(v[3], [3.5, 1.5, 4.0, 2.0]);
    assert_eq!(v[4], [0.5, 0.5, 4.0, 2.0]);
    assert_eq!(v[7], [3.5, 0.5, 4.0, 2.0]);
}

#[test]
fn vectors_and_swizzles() {
    assert_eq!(one("vec3 a = vec3(1, 2, 3); c = vec4(a.zyx, a.x + a.y);"), [3.0, 2.0, 1.0, 3.0]);
    assert_eq!(one("vec4 a = vec4(1, 2, 3, 4); a.xz = vec2(9, 8); c = a;"), [9.0, 2.0, 8.0, 4.0]);
    assert_eq!(one("vec2 a = vec2(3); a *= 2.0; a.y += 1.0; c = vec4(a, a * a);"), [6.0, 7.0, 36.0, 49.0]);
    assert_eq!(one("vec4 a = vec4(1); a.wzyx = vec4(1, 2, 3, 4); c = a;"), [4.0, 3.0, 2.0, 1.0]);
    assert_eq!(one("vec3 a = vec3(1, 2, 3); c = vec4(a[2], a.rgb.b, a.stp.s, 0);"), [3.0, 3.0, 1.0, 0.0]);
}

#[test]
fn matrices() {
    // Column-major: mat2(a, b, c, d) has columns (a, b) and (c, d).
    assert_eq!(one("mat2 m = mat2(1, 2, 3, 4); c = vec4(m * vec2(1, 1), vec2(1, 1) * m);"), [4.0, 6.0, 3.0, 7.0]);
    assert_eq!(one("mat2 m = mat2(1, 2, 3, 4); mat2 n = m * m; c = vec4(n[0], n[1]);"), [7.0, 10.0, 15.0, 22.0]);
    assert_eq!(one("mat3 m = mat3(2.0); c = vec4(m * vec3(1, 2, 3), m[1][1]);"), [2.0, 4.0, 6.0, 2.0]);
    let r = one("mat2 m = mat2(4, 7, 2, 6); mat2 i = inverse(m); c = vec4(i[0], i[1]);");
    let want = [0.6, -0.7, -0.2, 0.4];
    for k in 0..4 {
        assert!(close(r[k], want[k], 1e-6), "{r:?}");
    }
    assert_eq!(one("mat3 m = mat3(1,2,3, 0,1,4, 5,6,0); c = vec4(determinant(m), transpose(m)[0]);"), [1.0, 1.0, 0.0, 5.0]);
    let r = one("mat4 m = mat4(2,0,0,0, 0,4,0,0, 0,0,5,0, 1,2,3,1); mat4 i = inverse(m); c = (i * m)[3];");
    assert!(close(r[0], 0.0, 1e-6) && close(r[3], 1.0, 1e-6), "{r:?}");
    assert_eq!(one("mat2 r = mat2(0, 1, -1, 0); vec2 v = r * vec2(1, 0); c = vec4(v, 0, 0);"), [0.0, 1.0, 0.0, 0.0]);
}

#[test]
fn loops_and_divergence() {
    // Uniform loop.
    assert_eq!(one("float s = 0.0; for (int i = 0; i < 10; i++) s += float(i); c = vec4(s);")[0], 45.0);
    // Per-pixel trip counts: pixel x runs floor(p.x) + 1 iterations.
    let v = all("float s = 0.0; for (int i = 0; i < 100; i++) { if (float(i) > p.x) break; s += 1.0; } c = vec4(s);");
    assert_eq!([v[0][0], v[1][0], v[2][0], v[3][0]], [1.0, 2.0, 3.0, 4.0]);
    // continue skips odd iterations only in the right half.
    let v = all("float s = 0.0; for (int i = 0; i < 8; i++) { if (p.x > 2.0 && i % 2 == 1) continue; s += 1.0; } c = vec4(s);");
    assert_eq!([v[0][0], v[1][0], v[2][0], v[3][0]], [8.0, 8.0, 4.0, 4.0]);
    // while and do-while.
    assert_eq!(one("int i = 0; while (i < 5) i++; int j = 0; do { j += 2; } while (j < 7); c = vec4(i, j, 0, 0);"), [5.0, 8.0, 0.0, 0.0]);
    // Nested loops with an inner break.
    let v = one("float s = 0.0; for (int i = 0; i < 4; i++) for (int j = 0; j < 4; j++) { if (j > i) break; s += 1.0; } c = vec4(s);");
    assert_eq!(v[0], 10.0);
}

#[test]
fn functions_inout_and_early_return() {
    let src = "
float sq(float x) { return x * x; }
void bump(inout vec2 v, float k) { v += k; }
float pick(float x) { if (x < 2.0) return 1.0; if (x < 3.0) return 2.0; return 3.0; }
void mainImage(out vec4 c, in vec2 p) {
    vec2 v = vec2(1, 2);
    bump(v, 3.0);
    c = vec4(sq(v.x), v.y, pick(p.x), 0);
}";
    let v = render(src, 4, 1);
    assert_eq!(v[0], [16.0, 5.0, 1.0, 0.0]);
    assert_eq!(v[1][2], 1.0);
    assert_eq!(v[2][2], 2.0);
    assert_eq!(v[3][2], 3.0);
    // Early return from mainImage itself, divergent.
    let v = render("void mainImage(out vec4 c, in vec2 p) { c = vec4(1); if (p.x < 2.0) { c = vec4(0.5); return; } c.x = 0.25; }", 4, 1);
    assert_eq!(v[0], [0.5; 4]);
    assert_eq!(v[3], [0.25, 1.0, 1.0, 1.0]);
}

#[test]
fn ints_bools_switch() {
    assert_eq!(one("int a = 7; int b = 2; c = vec4(a / b, a % b, -a / b, a * b);"), [3.0, 1.0, -3.0, 14.0]);
    assert_eq!(one("uint h = 0x9e3779b9u; h ^= h >> 16; h *= 0x85ebca6bu; c = vec4(float(h & 255u), float(h >> 24), 0, 0);")[0], {
        let mut h: u32 = 0x9e37_79b9;
        h ^= h >> 16;
        h = h.wrapping_mul(0x85eb_ca6b);
        (h & 255) as f32
    });
    assert_eq!(one("int s = 3; int i = 1; s <<= i; c = vec4(s, 5 >> 1, ~0, -7 >> 1);"), [6.0, 2.0, -1.0, -4.0]);
    assert_eq!(one("bool a = true; bool b = !a; c = vec4(a ? 1.0 : 0.0, b ? 1.0 : 2.0, float(a && b), float(a || b));"), [1.0, 2.0, 0.0, 1.0]);
    let v = all("int k = int(p.x); float r = 0.0; switch (k) { case 0: r = 10.0; break; case 1: r = 20.0; case 2: r += 5.0; break; default: r = 99.0; } c = vec4(r);");
    assert_eq!([v[0][0], v[1][0], v[2][0], v[3][0]], [10.0, 25.0, 5.0, 99.0]);
    assert_eq!(one("c = vec4(int(-2.7), int(2.7), float(uint(3.9)), float(int(true)));"), [-2.0, 2.0, 3.0, 1.0]);
    assert_eq!(one("c = vec4(intBitsToFloat(floatBitsToInt(1.5)), uintBitsToFloat(0x3f800000u), 0, 0);"), [1.5, 1.0, 0.0, 0.0]);
}

#[test]
fn arrays_and_structs() {
    let src = "
struct Hit { float d; vec3 n; };
const vec3 pal[3] = vec3[](vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1));
Hit mk(float d) { Hit h; h.d = d; h.n = vec3(0, d, 0); return h; }
void mainImage(out vec4 c, in vec2 p) {
    int i = int(p.x) % 3;
    float a[4];
    for (int k = 0; k < 4; k++) a[k] = float(k * k);
    a[i] += 100.0;
    Hit h = mk(2.0);
    c = vec4(pal[i].x + pal[i].y * 2.0 + pal[i].z * 3.0, a[i], h.n.y, a[3]);
}";
    let v = render(src, 4, 1);
    assert_eq!(v[0], [1.0, 100.0, 2.0, 9.0]);
    assert_eq!(v[1], [2.0, 101.0, 2.0, 9.0]);
    assert_eq!(v[2], [3.0, 104.0, 2.0, 9.0]);
    assert_eq!(v[3], [1.0, 100.0, 2.0, 9.0]);
}

#[test]
fn builtins_exact() {
    assert_eq!(one("c = vec4(mod(-1.5, 1.0), fract(-0.25), floor(-0.5), ceil(0.2));"), [0.5, 0.75, -1.0, 1.0]);
    assert_eq!(one("c = vec4(clamp(2.0, 0.0, 1.0), mix(2.0, 4.0, 0.25), step(0.5, 0.4), smoothstep(0.0, 1.0, 0.5));"), [1.0, 2.5, 0.0, 0.5]);
    assert_eq!(one("c = vec4(min(1.0, 2.0), max(vec2(1, 5), vec2(3, 2)), sign(-3.0));"), [1.0, 3.0, 5.0, -1.0]);
    assert_eq!(one("c = vec4(length(vec2(3, 4)), distance(vec3(1), vec3(1, 1, 3)), dot(vec2(1, 2), vec2(3, 4)), abs(-2.0));"), [5.0, 2.0, 11.0, 2.0]);
    assert_eq!(one("c = vec4(cross(vec3(1, 0, 0), vec3(0, 1, 0)), sqrt(16.0));"), [0.0, 0.0, 1.0, 4.0]);
    assert_eq!(one("c = vec4(normalize(vec2(3, 4)), reflect(vec2(1, -1), vec2(0, 1)));"), [0.6, 0.8, 1.0, 1.0]);
    assert_eq!(one("c = vec4(round(2.5), trunc(-2.7), inversesqrt(4.0), radians(180.0) / 3.14159265);")[..3], [2.0, -2.0, 0.5]);
    assert_eq!(one("bvec2 b = lessThan(vec2(1, 3), vec2(2, 2)); c = vec4(float(b.x), float(b.y), float(any(b)), float(all(b)));"), [1.0, 0.0, 1.0, 0.0]);
    assert_eq!(one("float z = iTime - 1.0; c = vec4(isnan(z / z) ? 1.0 : 0.0, isinf(1.0 / z) ? 1.0 : 0.0, 0, 0);"), [1.0, 1.0, 0.0, 0.0]);
}

/// Transcendentals against f64, over the ranges shaders use.
#[test]
fn builtins_accuracy() {
    type Case = (&'static str, fn(f64) -> f64, f32, f32);
    let cases: &[Case] = &[
        ("sin", f64::sin, -100.0, 100.0),
        ("cos", f64::cos, -100.0, 100.0),
        ("tan", f64::tan, -1.5, 1.5),
        ("exp", f64::exp, -80.0, 80.0),
        ("exp2", f64::exp2, -120.0, 120.0),
        ("log", f64::ln, 1e-30, 1e30),
        ("log2", f64::log2, 1e-30, 1e30),
        ("atan", f64::atan, -50.0, 50.0),
        ("asin", f64::asin, -1.0, 1.0),
        ("acos", f64::acos, -1.0, 1.0),
        ("tanh", f64::tanh, -10.0, 10.0),
        ("sinh", f64::sinh, -10.0, 10.0),
        ("cosh", f64::cosh, -10.0, 10.0),
        ("sqrt", f64::sqrt, 0.0, 1e6),
    ];
    let (w, h) = (64u32, 64u32);
    for &(name, f, lo, hi) in cases {
        let log_range = lo > 0.0 && hi / lo > 1e3;
        let src = if log_range {
            format!("void mainImage(out vec4 c, in vec2 p) {{ float t = (p.x - 0.5 + (p.y - 0.5) * 64.0) / 4095.0; float x = exp2(mix(log2({lo:e}), log2({hi:e}), t)); c = vec4(x, {name}(x), 0, 1); }}")
        } else {
            format!("void mainImage(out vec4 c, in vec2 p) {{ float t = (p.x - 0.5 + (p.y - 0.5) * 64.0) / 4095.0; float x = mix({lo:?}, {hi:?}, t); c = vec4(x, {name}(x), 0, 1); }}")
        };
        let v = render(&src, w, h);
        let mut worst = 0.0f64;
        for px in &v {
            let x = px[0] as f64;
            let want = f(x);
            let got = px[1] as f64;
            // Error in units of f32 ulp of the true value (or absolute near zero).
            let ulp = (want.abs().max(1e-6) * f32::EPSILON as f64).max(f32::MIN_POSITIVE as f64);
            let e = (got - want).abs() / ulp;
            if e > worst {
                worst = e;
            }
        }
        assert!(worst < 4.0, "{name}: worst error {worst:.2} ulp");
    }
    // pow is exp2(y·log2 x), as on GPUs.
    let r = one("c = vec4(pow(2.0, 10.0), pow(9.0, 0.5), pow(0.0, 2.0), pow(5.0, 0.0));");
    assert!(close(r[0], 1024.0, 1e-6) && close(r[1], 3.0, 1e-6) && r[2] == 0.0 && r[3] == 1.0, "{r:?}");
    let r = one("float z = iTime - 4.0; c = vec4(pow(z, 2.0), pow(z, 3.0), pow(z, 0.5), 0);");
    assert!(close(r[0], 9.0, 1e-6) && close(r[1], -27.0, 1e-6) && r[2] == 0.0 /* NaN is written as 0 */, "{r:?}");
    let r = one("c = vec4(atan(1.0, -1.0), atan(-1.0, -1.0), atan(0.0, -1.0), atan(1.0, 0.0));");
    let want = [3.0 * std::f32::consts::FRAC_PI_4, -3.0 * std::f32::consts::FRAC_PI_4, std::f32::consts::PI, std::f32::consts::FRAC_PI_2];
    for k in 0..4 {
        assert!(close(r[k], want[k], 1e-6), "{r:?}");
    }
}

#[test]
fn derivatives() {
    let v = all("float f = p.x * 3.0 + p.y * 5.0; c = vec4(dFdx(f), dFdy(f), fwidth(f), dFdx(p.x * p.x));");
    assert_eq!(v[0][..3], [3.0, 5.0, 8.0]);
    assert_eq!(v[5][..3], [3.0, 5.0, 8.0]);
}

#[test]
fn texture_sampling() {
    // 2×2 texture: top row red, green; bottom row blue, white.
    let tex = Texture::from_f32(2, 2, vec![[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]]);
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, p / iResolution.xy); }").unwrap();
    let mut f = Frame::new(2, 2, 0.0);
    f.channels[0] = Some(&tex);
    let v = s.render_f32(&f);
    assert_eq!(v[0], [1.0, 0.0, 0.0, 1.0]); // top-left = red (upright)
    assert_eq!(v[3], [1.0, 1.0, 1.0, 1.0]);
    // Bilinear midpoint and texelFetch (y = 0 is the bottom row) and textureSize.
    let s = Shader::compile(
        "void mainImage(out vec4 c, in vec2 p) { vec4 m = texture(iChannel0, vec2(0.5)); vec4 t = texelFetch(iChannel0, ivec2(0, 0), 0); ivec2 sz = textureSize(iChannel0, 0); c = vec4(m.r, t.b, float(sz.x), iChannelResolution[0].y); }",
    )
    .unwrap();
    let v = s.render_f32(&f);
    assert_eq!(v[0], [0.5, 1.0, 2.0, 2.0]);
    // Repeat wrap.
    let tex2 = Texture::from_f32(2, 2, vec![[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]]).with_wrap(Wrap::Repeat).with_filter(Filter::Nearest);
    let mut f = Frame::new(2, 2, 0.0);
    f.channels[0] = Some(&tex2);
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, vec2(1.25, 1.75)); }").unwrap();
    assert_eq!(s.render_f32(&f)[0], [1.0, 0.0, 0.0, 1.0]);
    // Mip levels: level 1 of a 2×2 is the average.
    let tex3 = Texture::from_f32(2, 2, vec![[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]]).with_mipmaps();
    let mut f = Frame::new(2, 2, 0.0);
    f.channels[0] = Some(&tex3);
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = textureLod(iChannel0, vec2(0.3), 1.0); }").unwrap();
    // Texels are 8-bit: the mip average is 128/255.
    assert_eq!(s.render_f32(&f)[0], [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0]);
}

#[test]
fn discard_and_alpha() {
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { if (p.x < 1.0) discard; c = vec4(1, 0, 0, 0.5); }").unwrap();
    let mut f = Frame::new(2, 1, 0.0);
    f.alpha = true;
    let mut px = vec![0u8; 8];
    let opaque = s.render(&f, &mut px);
    assert!(!opaque);
    assert_eq!(px, [0, 0, 0, 0, 128, 0, 0, 128]);
    f.alpha = false;
    assert!(s.render(&f, &mut px));
    assert_eq!(&px[4..], [255, 0, 0, 255]);
}

#[test]
fn uniforms_and_defaults() {
    let s = Shader::compile("uniform float amp; // = 0.5\nuniform vec3 tint;\nvoid mainImage(out vec4 c, in vec2 p) { c = vec4(tint * amp, iTime); }").unwrap();
    let tint = [1.0f32, 2.0, 4.0];
    let unis = [("tint", &tint[..])];
    let mut f = Frame::new(2, 2, 3.0);
    f.uniforms = &unis;
    assert_eq!(s.render_f32(&f)[0], [0.5, 1.0, 2.0, 3.0]);
    // Undeclared spec uniforms can be declared by the caller.
    let s = Shader::compile_with("void mainImage(out vec4 c, in vec2 p) { c = vec4(speed, dir, 1); }", &[("speed", UType::Float), ("dir", UType::Vec(2))]).unwrap();
    let (sp, dir) = ([2.0f32], [3.0f32, 4.0]);
    let unis = [("speed", &sp[..]), ("dir", &dir[..])];
    f.uniforms = &unis;
    assert_eq!(s.render_f32(&f)[0], [2.0, 3.0, 4.0, 1.0]);
}

#[test]
fn transition_kind() {
    let src = "uniform float amount; // = 1.0\nvec4 transition(vec2 uv) { return mix(getFromColor(uv), getToColor(uv), progress * amount); }";
    let s = Shader::compile(src).unwrap();
    assert_eq!(s.kind, Kind::Transition);
    let a = Texture::from_f32(1, 1, vec![[1.0, 0.0, 0.0, 1.0]]);
    let b = Texture::from_f32(1, 1, vec![[0.0, 0.0, 1.0, 1.0]]);
    let mut f = Frame::new(2, 2, 0.0);
    f.channels = [Some(&a), Some(&b), None, None];
    f.progress = 0.25;
    assert_eq!(s.render_f32(&f)[0], [0.75, 0.0, 0.25, 1.0]);
}

#[test]
fn wgsl() {
    let s = Shader::compile("fn mainImage(fragCoord: vec2f) -> vec4f { let uv = fragCoord / iResolution.xy; return vec4f(uv, iTime, 1.0); }").unwrap();
    assert_eq!(s.lang, Lang::Wgsl);
    let v = s.render_f32(&Frame::new(2, 2, 0.5));
    assert_eq!(v[0], [0.25, 0.75, 0.5, 1.0]);
}

#[test]
fn supersampling_averages_four_lanes() {
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(fract(p), 0, 1); }").unwrap();
    let mut f = Frame::new(2, 2, 0.0);
    f.supersample = true;
    // Sub-samples at ±0.25 around the centre average back to 0.5.
    assert_eq!(s.render_f32(&f)[0], [0.5, 0.5, 0.0, 1.0]);
}

#[test]
fn errors_point_at_user_lines() {
    let e = Shader::compile("void mainImage(out vec4 c, in vec2 p) {\n  float x = 1.0;\n  c = vec4(smoothStep(0.0, 1.0, x));\n}").err().unwrap();
    assert_eq!(e.line, Some(3), "{e}");
    assert!(e.message.contains("smoothstep"), "{e}");
    let e = Shader::compile("void mainImage(out vec4 c, in vec2 p) {\n  c = vec4(lerp(0.0, 1.0, 0.5));\n}").err().unwrap();
    assert!(e.message.contains("mix"), "{e}");
    let e = Shader::compile("void main() {}").err().unwrap();
    assert!(e.message.contains("mainImage"), "{e}");
    let e = Shader::compile("void mainImage(out vec4 c, in vec2 p) {\n  c = vec4(1.0) // missing semicolon\n}").err().unwrap();
    assert!(e.line.is_some(), "{e}");
}

#[test]
fn deterministic_across_runs_and_threads() {
    let src = "void mainImage(out vec4 c, in vec2 p) { vec2 uv = p / iResolution.xy; float v = sin(uv.x * 40.0 + iTime) * cos(uv.y * 30.0) + exp(-uv.x) * log(1.0 + uv.y); c = vec4(v, atan(uv.y, uv.x - 0.5), pow(uv.x + 0.01, 2.2), 1); }";
    let s = Shader::compile(src).unwrap();
    let a = s.render_f32(&Frame::new(97, 61, 1.7));
    let b = s.render_f32(&Frame::new(97, 61, 1.7));
    assert_eq!(a, b);
}

#[test]
fn odd_sizes_cover_every_pixel() {
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(p, 0, 1); }").unwrap();
    let v = s.render_f32(&Frame::new(5, 3, 0.0));
    assert_eq!(v[4], [4.5, 2.5, 0.0, 1.0]);
    assert_eq!(v[14], [4.5, 0.5, 0.0, 1.0]);
}

#[test]
fn channel_usage() {
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel1, p); }").unwrap();
    assert_eq!(s.channels_used, [false, true, false, false]);
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(1); }").unwrap();
    assert_eq!(s.channels_used, [false; 4]);
}

/// The JIT's inline bilinear path matches `Texture::sample` exactly.
#[test]
fn inline_sampling_matches_reference() {
    let px: Vec<[f32; 4]> = (0..7 * 5).map(|i| [((i * 37) % 11) as f32 / 10.0, ((i * 13) % 7) as f32 / 6.0, (i % 5) as f32 / 4.0, 1.0]).collect();
    // Float buffers: values outside 0..1 must come through exactly.
    let fpx: Vec<[f32; 4]> = px.iter().enumerate().map(|(i, p)| [p[0] * 7.0 - 3.0, p[1] * -2.5, p[2] + i as f32 * 0.25, 1.0 + i as f32]).collect();
    for (wrap, filter, float) in [(Wrap::Clamp, Filter::Linear, false), (Wrap::Repeat, Filter::Linear, false), (Wrap::Clamp, Filter::Nearest, false), (Wrap::Repeat, Filter::Nearest, false), (Wrap::Clamp, Filter::Linear, true), (Wrap::Repeat, Filter::Linear, true), (Wrap::Repeat, Filter::Nearest, true)] {
        let t = if float { Texture::from_f32_exact(7, 5, fpx.clone()) } else { Texture::from_f32(7, 5, px.clone()) };
        let t = t.with_wrap(wrap).with_filter(filter);
        let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { vec2 uv = (p - vec2(20.0, 12.0)) * vec2(0.037, -0.029) + 0.013; c = texture(iChannel0, uv); }").unwrap();
        let mut f = Frame::new(41, 23, 0.0);
        f.channels[0] = Some(&t);
        let out = s.render_f32(&f);
        for (i, got) in out.iter().enumerate() {
            let (x, y) = ((i % 41) as f32 + 0.5, (23 - i / 41) as f32 - 0.5);
            let uv = [(x - 20.0) * 0.037 + 0.013, (y - 12.0) * -0.029 + 0.013];
            let want = t.sample(uv[0], uv[1], 0.0);
            for k in 0..4 {
                assert!((got[k] - want[k]).abs() <= 1e-6 * (1.0 + want[k].abs()), "{wrap:?} {filter:?} float={float} px {i}: {got:?} vs {want:?}");
            }
        }
    }
}

#[test]
fn dither_keeps_exact_values_and_breaks_bands() {
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(p.x < 32.0 ? 128.0 / 255.0 : 0.3017, 0.0, 1.0, 1.0); }").unwrap();
    let mut f = Frame::new(64, 64, 0.0);
    f.dither = true;
    let mut px = vec![0u8; 64 * 64 * 4];
    s.render(&f, &mut px);
    let left: Vec<u8> = (0..64 * 64).filter(|i| i % 64 < 32).map(|i| px[i * 4]).collect();
    assert!(left.iter().all(|&v| v == 128));
    let right: Vec<f32> = (0..64 * 64).filter(|i| i % 64 >= 32).map(|i| px[i * 4] as f32).collect();
    let mean = right.iter().sum::<f32>() / right.len() as f32;
    assert!((mean - 0.3017 * 255.0).abs() < 0.05, "{mean}");
    assert!(right.contains(&76.0) && right.contains(&77.0));
}

#[test]
fn adaptive_render_reuses_scratch_across_sizes() {
    let src = "void mainImage(out vec4 c, in vec2 p) {
        float a = step(0.5, fract(p.x * 0.137 + sin(p.y * 0.31) * 0.2));
        float b = step(0.5, fract(p.y * 0.173 + sin(p.x * 0.27) * 0.2));
        c = vec4(a, b, a * b, 0.2 + 0.8 * step(0.5, fract(p.x * 0.071 + p.y * 0.113)));
    }";
    let shader = Shader::compile(src).unwrap();
    for max_refine in [1.0, 0.2] {
        let mut f = Frame::new(97, 61, 0.0);
        f.supersample = true;
        f.adaptive = true;
        f.max_refine = max_refine;
        f.alpha = true;
        f.dither = true;
        for (w, h) in [(97, 61), (40, 30), (97, 61)] {
            f.width = w;
            f.height = h;
            let mut reused = vec![0u8; w as usize * h as usize * 4];
            shader.render(&f, &mut reused);
            let fresh = Shader::compile(src).unwrap();
            let mut expected = vec![0u8; reused.len()];
            fresh.render(&f, &mut expected);
            assert_eq!(reused, expected, "size {w}×{h}, max_refine {max_refine}");
        }
    }
}

#[test]
fn unrolled_loops_keep_glsl_semantics() {
    // Counter value after the loop, divergent ifs around it, nested and
    // decreasing loops, loops with a break (not unrolled), <= bounds.
    let src = r#"
float f(float x) {
    float s = 0.0;
    for (int i = 0; i < 6; i++) { float fi = float(i); s += sin(iTime + fi) * x + fi; }
    return s;
}
void mainImage(out vec4 c, in vec2 p) {
    int k = 0;
    if (p.x > 2.0) { for (k = 0; k < 3; k++) { } }       // lanes that skip keep k = 0
    float n = 0.0;
    for (int a = 0; a < 3; a++) { for (int b = 4; b > 0; b--) { n += float(a * 10 + b); } }
    float m = 0.0;
    for (int i = 1; i <= 4; i++) { if (p.x > 3.0 && i == 3) break; m += float(i); }
    float big = 0.0;
    for (int i = 0; i < 40; i++) { big += 1.0; }          // too many trips: stays a loop
    c = vec4(f(1.0), float(k) + n * 0.001, m, big);
}"#;
    let px = render(src, 8, 2);
    let expect_f: f32 = (0..6).map(|i| (1.0 + i as f32).sin() + i as f32).sum();
    for (x, p) in px.iter().take(8).enumerate() {
        let fx = x as f32 + 0.5;
        assert!(close(p[0], expect_f, 1e-5), "f: {} vs {expect_f}", p[0]);
        let k = if fx > 2.0 { 3.0 } else { 0.0 };
        let n: f32 = (0..3).flat_map(|a| (1..=4).map(move |b| (a * 10 + b) as f32)).sum();
        assert!(close(p[1], k + n * 0.001, 1e-5), "x={x}: k/n {} vs {}", p[1], k + n * 0.001);
        let m = if fx > 3.0 { 3.0 } else { 10.0 };
        assert_eq!(p[2], m, "x={x}: m");
        assert_eq!(p[3], 40.0);
    }
}

#[test]
fn texel_fetch_inline_matches_reference() {
    let px: Vec<[f32; 4]> = (0..6 * 4).map(|i| [(i % 6) as f32 / 5.0, (i / 6) as f32 / 3.0, ((i * 7) % 10) as f32 / 9.0, 1.0]).collect();
    let fpx: Vec<[f32; 4]> = px.iter().enumerate().map(|(i, p)| [p[0] * 9.0 - 4.0, i as f32, -p[2], 2.5]).collect();
    // Coordinates run from -2 to 8 / -2 to 6: out of range on every side.
    let src = "void mainImage(out vec4 c, in vec2 p) { c = texelFetch(iChannel0, ivec2(p) - ivec2(2), 0); }";
    for t in [Texture::from_f32(6, 4, px.clone()), Texture::from_f32_exact(6, 4, fpx.clone()), Texture::from_f32(6, 4, px.clone()).with_mipmaps()] {
        let s = Shader::compile(src).unwrap();
        let mut f = Frame::new(10, 8, 0.0);
        f.channels[0] = Some(&t);
        let out = s.render_f32(&f);
        for (i, got) in out.iter().enumerate() {
            let (x, y) = ((i % 10) as i32 - 2, (7 - i / 10) as i32 - 2);
            let want = t.fetch(x, y, 0);
            assert_eq!(*got, want, "px {i} ({x}, {y})");
        }
    }
    // Another level goes through the sampler.
    let t = Texture::from_f32(6, 4, px).with_mipmaps();
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = texelFetch(iChannel0, ivec2(p), 1); }").unwrap();
    let mut f = Frame::new(4, 2, 0.0);
    f.channels[0] = Some(&t);
    let out = s.render_f32(&f);
    assert_eq!(out[4], t.fetch(0, 0, 1));
}

#[test]
fn disk_cache_round_trip_renders_identically() {
    let dir = std::env::temp_dir().join(format!("anikit-shader-cache-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::env::set_var("ANIKIT_SHADER_CACHE", &dir);
    // Samples a texture too, so the patched call addresses are exercised.
    let src = "uniform float k; // = 0.25\nvoid mainImage(out vec4 c, in vec2 p) { vec2 uv = p / iResolution.xy; c = texture(iChannel0, uv * 1.7) * k + vec4(sin(uv.x * 9.0 + iTime), 0.0, 0.0, 1.0); for (int i = 0; i < 3; i++) c.g += texelFetch(iChannel0, ivec2(p) + ivec2(i), 0).r * 0.1; }";
    let tex = Texture::from_f32(5, 3, (0..15).map(|i| [i as f32 / 15.0, 0.5, 1.0 - i as f32 / 15.0, 1.0]).collect());
    let render = |s: &Shader| {
        let mut f = Frame::new(33, 17, 0.7);
        f.channels[0] = Some(&tex);
        s.render_f32(&f)
    };
    let a = Shader::compile(src).unwrap();
    let files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.flatten().collect()).unwrap_or_default();
    assert!(!files.is_empty(), "the first compile writes the cache");
    let b = Shader::compile(src).unwrap(); // loaded from disk
    assert_eq!(a.insts, b.insts);
    assert_eq!(render(&a), render(&b));
    // A corrupt entry is ignored (compiled afresh), never trusted.
    for f in &files {
        std::fs::write(f.path(), b"AKSHJIT1garbage").unwrap();
    }
    let c = Shader::compile(src).unwrap();
    assert_eq!(render(&a), render(&c));
    std::env::remove_var("ANIKIT_SHADER_CACHE");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ak_library_is_included_on_use_and_correct() {
    // Round trips and known values, written into the four channels.
    let src = "void mainImage(out vec4 c, in vec2 p) {
        vec3 x = vec3(0.02, 0.4, 0.9);
        float a = length(ak_srgb(ak_linear(x)) - x);
        float b = length(ak_oklab_inv(ak_oklab(x)) - x);
        float m = ak_mix(vec3(1, 0, 0), vec3(0, 0, 1), 0.0).r;
        vec3 v = ak_voronoi(vec2(3.3, 7.1));
        float n = ak_fbm(vec2(1.7, 2.3), 5);
        c = vec4(a + b, m, step(v.x, v.y) * step(0.0, n) * step(n, 1.0), ak_palette(0.25, vec3(0.5), vec3(0.5), vec3(1.0), vec3(0.0)).r);
    }";
    let s = Shader::compile(src).unwrap();
    let px = s.render_f32(&Frame::new(2, 2, 0.0))[0];
    assert!(px[0] < 2e-4, "round trips: {}", px[0]);
    assert!((px[1] - 1.0).abs() < 1e-3, "oklab mix endpoint: {}", px[1]);
    assert_eq!(px[2], 1.0, "voronoi ordering and fbm range");
    assert!((px[3] - 0.5).abs() < 1e-5, "palette: {}", px[3]);
    // A shader that doesn't mention ak_ doesn't get it (and an unknown ak_ name is an error).
    assert!(Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(ak_nonsense(p), 0, 1); }").is_err());
}

#[test]
fn nonfinite_pixels_are_reported() {
    // NaN only at fragCoord (5.5, 2.5): 0/0 there.
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { float z = (p.x == 5.5 && p.y == 2.5) ? 0.0 : 1.0; c = vec4(z / z, 0.0, 0.0, 1.0); }").unwrap();
    let mut out = vec![0u8; 16 * 8 * 4];
    let f = Frame::new(16, 8, 0.0);
    s.render(&f, &mut out);
    assert_eq!(s.take_nonfinite(), Some((5, 2)));
    assert_eq!(s.take_nonfinite(), None, "cleared once taken");
    let ok = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(p.x / 16.0); }").unwrap();
    ok.render(&f, &mut out);
    assert_eq!(ok.take_nonfinite(), None);
}

#[test]
fn coord_scale_gives_final_size_coordinates() {
    // A half-size preview (8×4 for a 16×8 final) sees the final's fragCoord and iResolution.
    let s = Shader::compile("void mainImage(out vec4 c, in vec2 p) { c = vec4(p.x / 32.0, p.y / 32.0, iResolution.x / 32.0, iResolution.y / 32.0); }").unwrap();
    let mut f = Frame::new(8, 4, 0.0);
    f.coord_scale = 2.0;
    let px = s.render_f32(&f);
    // Preview pixel (0, 0) is the top-left block of the final: fragCoord (1, 7).
    assert_eq!(px[0], [1.0 / 32.0, 7.0 / 32.0, 0.5, 0.25]);
    // Preview pixel (3, 3) (bottom row): fragCoord (7, 1).
    assert_eq!(px[3 * 8 + 3], [7.0 / 32.0, 1.0 / 32.0, 0.5, 0.25]);
    f.supersample = true;
    let ss = s.render_f32(&f);
    assert!((ss[0][0] - 1.0 / 32.0).abs() < 1e-6 && (ss[0][1] - 7.0 / 32.0).abs() < 1e-6);
}

#[test]
fn ak_drawn_blocks_track_tone() {
    // Mean coverage of stipple and hatch rises with tone; brush, dry and
    // wedge are inside 0..1 and cover their shapes.
    let src = "uniform float tone;\nvoid mainImage(out vec4 c, in vec2 p) {\n\
        c = vec4(ak_stipple(p, tone, 9.0), ak_hatch(p, vec2(1.0, 0.3), tone, 8.0),\n\
                 ak_brush(p, vec2(20.0, 64.0), vec2(108.0, 64.0), 14.0, 1.0) * ak_dry(length(p - 64.0) - 30.0, ak_paper(p), 4.0),\n\
                 ak_wedge(p, vec2(0.0), vec2(1.0), 0.4, 200.0, 0.5)); }";
    let s = Shader::compile(src).unwrap();
    let mean = |tone: f32| {
        let t = [tone];
        let u = [("tone", &t[..])];
        let mut f = Frame::new(128, 128, 0.0);
        f.uniforms = &u;
        let px = s.render_f32(&f);
        let m = |k: usize| px.iter().map(|p| p[k]).sum::<f32>() / px.len() as f32;
        (m(0), m(1), m(2), m(3), px.iter().all(|p| p.iter().all(|v| (0.0..=1.0).contains(v))))
    };
    let (s0, h0, ..) = mean(0.1);
    let (s1, h1, ..) = mean(0.4);
    let (s2, h2, b, w, ok) = mean(0.8);
    assert!(ok);
    assert!(s0 < s1 && s1 < s2 && s2 > 0.4, "stipple {s0} {s1} {s2}");
    assert!(h0 < h1 && h1 < h2 && h2 > 0.4, "hatch {h0} {h1} {h2}");
    assert!(b > 0.05 && w > 0.05, "brush × dry {b}, wedge {w}");
}
