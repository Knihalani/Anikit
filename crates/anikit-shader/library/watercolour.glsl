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

// Two independent value noises at once: four 2-channel hashes instead of eight.
vec2 h22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}
vec2 vnoise2(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(h22(i), h22(i + vec2(1, 0)), u.x), mix(h22(i + vec2(0, 1)), h22(i + vec2(1, 1)), u.x), u.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float s = res.y / 1080.0;
    vec2 p = fragCoord / s;
    // Bleed: sample through a slow, smooth warp (sines: no noise lookups), twice.
    vec2 w = vnoise2(p * 0.009 + 3.1) - 0.5;          // the slow, wide wander
    vec2 w2 = vnoise2(p * 0.045) - 0.5;
    vec2 uv = fragCoord / res;
    // Exact texel reads at the warped positions: the bleed itself softens them.
    ivec2 hi = ivec2(res) - 1;
    vec3 a = texelFetch(iChannel0, clamp(ivec2(fragCoord + (w * 6.0 + w2 * 7.0) * bleed * s), ivec2(0), hi), 0).rgb;
    vec3 b = texelFetch(iChannel0, clamp(ivec2(fragCoord - (w * 3.0 - w2 * 6.0) * bleed * s), ivec2(0), hi), 0).rgb;
    vec3 c = (a + b) * 0.5;
    // Pigment: how much colour sits over the paper, slightly simplified.
    vec3 pig = 1.0 - c;
    pig = floor(pig * 6.0 + 0.5 + w2.x * 0.5) / 6.0 * 0.4 + pig * 0.6;
    // Edge darkening where the wash changes.
    float e = length(vec3(dFdx(c.r), dFdx(c.g), dFdx(c.b))) + length(vec3(dFdy(c.r), dFdy(c.g), dFdy(c.b)));
    pig *= 1.0 + edges * clamp(e * 8.0 / s, 0.0, 1.2);
    // Granulation: pigment settles in the paper's tooth.
    float gr = h21(floor(p * 0.6));                // per-cell grain: the paper's tooth
    pig *= 1.0 + granulation * (gr - 0.5) * 0.28;
    vec3 col = paper * (1.0 - clamp(pig * wash, 0.0, 1.0) * 0.98);
    col *= 0.985 + 0.015 * w2.y;
    fragColor = vec4(col, 1.0);
}
