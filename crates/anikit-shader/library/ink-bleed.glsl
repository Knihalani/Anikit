// Transition: the new scene bleeds in like ink soaking into paper, with a
// dark wet edge. Uniforms: softness, edge.
uniform float softness; // = 0.04
uniform float edge;     // = 0.5

float hash(vec2 p) { return fract(sin(dot(p, vec2(269.5, 183.3))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}

vec4 transition(vec2 uv) {
    vec2 p = uv * vec2(ratio, 1.0);
    float n = 0.0, a = 0.5;
    vec2 q = p * 3.0;
    for (int i = 0; i < 6; i++) { n += a * vnoise(q); q = q * 2.03 + 1.7; a *= 0.5; }
    // Blots start from a few centres and spread.
    float blot = min(length(p - vec2(0.3 * ratio, 0.35)), length(p - vec2(0.75 * ratio, 0.7)));
    float field = blot * 0.8 + n * 0.55;
    float th = progress * 1.4;
    float m = smoothstep(th + softness, th - softness, field);
    float wet = smoothstep(softness * 3.0, 0.0, abs(field - th)) * edge;
    vec4 c = mix(getFromColor(uv), getToColor(uv), m);
    c.rgb *= 1.0 - wet * 0.6;
    return c;
}
