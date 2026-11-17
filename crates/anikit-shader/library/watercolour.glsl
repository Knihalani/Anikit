// Watercolour: colours soften and bleed, pigment pools dark at the edges of
// each wash, and the paper's grain shows through.
// A filter: {"shader": "watercolour", "filter": true}.
// Uniforms: bleed, edges, granulation, paper, wash.
uniform float bleed;       // = 1.0 how far colours wander
uniform float edges;       // = 1.0 dark pigment rims at colour boundaries
uniform float granulation; // = 1.0 paper grain in the washes
uniform vec3 paper;        // = vec3(0.98, 0.97, 0.93) paper colour
uniform float wash;        // = 0.85 colour strength (lower is paler)

// Arithmetic hash (no sin): cheap enough for three noise lookups per pixel.
float h21(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(h21(i), h21(i + vec2(1, 0)), u.x), mix(h21(i + vec2(0, 1)), h21(i + vec2(1, 1)), u.x), u.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float s = res.y / 1080.0;
    vec2 p = fragCoord / s;
    // Bleed: sample through a slow, smooth warp (sines: no noise lookups), twice.
    vec2 w = vec2(sin(p.y * 0.011 + sin(p.x * 0.007) * 1.3), cos(p.x * 0.009 + sin(p.y * 0.008) * 1.3)) * 0.5;
    vec2 w2 = vec2(vnoise(p * 0.045), vnoise(p * 0.045 + 9.7)) - 0.5;
    vec2 uv = fragCoord / res;
    vec3 a = texture(iChannel0, uv + (w * 6.0 + w2 * 7.0) * bleed * s / res).rgb;
    vec3 b = texture(iChannel0, uv - (w * 3.0 - w2 * 6.0) * bleed * s / res).rgb;
    vec3 c = (a + b) * 0.5;
    // Pigment: how much colour sits over the paper, slightly simplified.
    vec3 pig = 1.0 - c;
    pig = floor(pig * 6.0 + 0.5 + w2.x * 0.5) / 6.0 * 0.4 + pig * 0.6;
    // Edge darkening where the wash changes.
    float e = length(vec3(dFdx(c.r), dFdx(c.g), dFdx(c.b))) + length(vec3(dFdy(c.r), dFdy(c.g), dFdy(c.b)));
    pig *= 1.0 + edges * clamp(e * 8.0 / s, 0.0, 1.2);
    // Granulation: pigment settles in the paper's tooth.
    float gr = vnoise(p * 0.6);
    pig *= 1.0 + granulation * (gr - 0.5) * 0.28;
    vec3 col = paper * (1.0 - clamp(pig * wash, 0.0, 1.0) * 0.98);
    col *= 0.985 + 0.015 * w2.y;
    fragColor = vec4(col, 1.0);
}
