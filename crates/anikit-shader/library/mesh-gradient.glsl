// Mesh gradient: four colour fields drifting and folding into each other,
// the soft animated background of product and title shots. Colours mix in
// linear light with smooth inverse-distance weights over a gently warped
// plane, plus fine film grain so the gradients never band.
// Uniforms: color1..color4, speed, warp, softness, grain.
uniform vec3 color1;    // = vec3(0.98, 0.42, 0.33) first colour
uniform vec3 color2;    // = vec3(0.36, 0.25, 0.95) second colour
uniform vec3 color3;    // = vec3(0.07, 0.75, 0.82) third colour
uniform vec3 color4;    // = vec3(0.99, 0.82, 0.55) fourth colour
uniform float speed;    // = 1.0 drift speed
uniform float warp;     // = 1.0 how much the fields fold and swirl
uniform float softness; // = 1.0 0.5 tighter blobs … 2 broad washes
uniform float grain;    // = 1.0 film grain amount

vec3 lin(vec3 c) { return c * c; }                  // cheap sRGB → linear
float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(h21(i), h21(i + vec2(1, 0)), u.x), mix(h21(i + vec2(0, 1)), h21(i + vec2(1, 1)), u.x), u.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    vec2 p = (fragCoord - 0.5 * res) / res.y;
    float t = iTime * speed * 0.15;
    // Warp the plane with two octaves of slow noise: organic, folded edges.
    vec2 w = vec2(vnoise(p * 1.3 + vec2(t, 1.7)), vnoise(p * 1.3 + vec2(5.2, -t))) - 0.5;
    w += 0.5 * (vec2(vnoise(p * 2.9 - vec2(t * 1.3, 0.0)), vnoise(p * 2.9 + vec2(0.0, t * 1.1) + 9.1)) - 0.5);
    vec2 q = p + w * 0.55 * warp;
    // Four field centres on slow Lissajous paths.
    vec2 c1 = vec2(-0.55 + 0.25 * sin(t * 1.1), 0.25 + 0.15 * cos(t * 1.3));
    vec2 c2 = vec2(0.6 + 0.2 * cos(t * 0.9 + 1.0), 0.3 + 0.2 * sin(t * 1.2 + 2.0));
    vec2 c3 = vec2(-0.3 + 0.3 * cos(t * 0.7 + 4.0), -0.35 + 0.15 * sin(t * 1.5));
    vec2 c4 = vec2(0.45 + 0.25 * sin(t * 1.3 + 3.0), -0.3 + 0.2 * cos(t * 0.8 + 1.0));
    float s = 0.12 * softness * softness;
    float w1 = 1.0 / (dot(q - c1, q - c1) + s);
    float w2 = 1.0 / (dot(q - c2, q - c2) + s);
    float w3 = 1.0 / (dot(q - c3, q - c3) + s);
    float w4 = 1.0 / (dot(q - c4, q - c4) + s);
    // Sharpen the weights a little so each colour keeps its own territory.
    w1 *= w1; w2 *= w2; w3 *= w3; w4 *= w4;
    vec3 col = (lin(color1) * w1 + lin(color2) * w2 + lin(color3) * w3 + lin(color4) * w4) / (w1 + w2 + w3 + w4);
    // A soft light falloff toward the corners.
    col *= 1.0 - 0.18 * dot(p, p);
    col = sqrt(max(col, 0.0));                      // back to display space
    // Grain: fine, animated, zero-mean.
    float g = h21(fragCoord + fract(iTime * 7.13) * 91.7) - 0.5;
    col += g * 0.035 * grain;
    fragColor = vec4(col, 1.0);
}
