// Transition: the old scene burns away along a noisy front with a glowing
// ember edge, revealing the new one. Uniforms: glow (edge width), color.
uniform float glow;  // = 0.06  width of the ember edge
uniform vec3 color;  // = vec3(1.0, 0.55, 0.12)  ember colour

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p) { float s = 0.0, a = 0.5; for (int i = 0; i < 5; i++) { s += a * vnoise(p); p *= 2.1; a *= 0.5; } return s; }

vec4 transition(vec2 uv) {
    float n = fbm(uv * vec2(ratio, 1.0) * 5.0) * 0.6 + 0.4 * (1.0 - uv.y);
    float p = progress * (1.0 + 2.0 * glow) - glow;
    float d = n - p;
    vec4 a = getFromColor(uv), b = getToColor(uv);
    float t = smoothstep(-0.005, 0.005, d);
    vec4 c = mix(b, a, t);
    float e = smoothstep(glow, 0.0, abs(d));
    c.rgb = mix(c.rgb, c.rgb * 0.2, smoothstep(glow * 1.6, 0.0, d) * t);   // charring ahead of the edge
    c.rgb += color * e * 1.8;
    return c;
}
