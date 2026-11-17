// Transition: a ring wave spreads from the centre; the new scene rides in
// behind it. Uniforms: amplitude, waves.
uniform float amplitude; // = 0.03
uniform float waves;     // = 30.0

vec4 transition(vec2 uv) {
    vec2 c = uv - 0.5;
    c.x *= ratio;
    float r = length(c);
    float front = progress * 1.3;
    float k = exp(-pow((r - front) * 6.0, 2.0));
    vec2 dir = c / max(r, 1e-4);
    vec2 off = dir * sin(r * waves - progress * 20.0) * amplitude * k;
    off.x /= ratio;
    vec4 a = getFromColor(uv + off), b = getToColor(uv + off);
    float m = smoothstep(front + 0.05, front - 0.05, r);
    return mix(a, b, m);
}
