// Transition: a soft diagonal wipe that smears pixels along its direction
// as it passes. Uniforms: direction (degrees), smear.
uniform float direction; // = 20.0
uniform float smear;     // = 0.12

vec4 transition(vec2 uv) {
    float a = radians(direction);
    vec2 dir = vec2(cos(a), sin(a));
    vec2 c = uv - 0.5;
    float s = dot(c * vec2(ratio, 1.0), dir) / (abs(dir.x) * ratio * 0.5 + abs(dir.y) * 0.5) * 0.5 + 0.5;
    float band = 0.2;
    float p = progress * (1.0 + 2.0 * band) - band;
    float m = smoothstep(p - band, p + band, s);   // 1 = still old scene
    float k = (1.0 - abs(m * 2.0 - 1.0));          // strongest mid-band
    vec2 off = dir * vec2(1.0 / ratio, 1.0) * smear * k;
    vec4 from = getFromColor(uv - off * (1.0 - m));
    vec4 to = getToColor(uv + off * m);
    return mix(to, from, m);
}
