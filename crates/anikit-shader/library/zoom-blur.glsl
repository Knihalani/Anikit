// Transition: punch-in zoom blur with chromatic fringes; the cut happens at
// peak blur. Uniforms: strength.
uniform float strength; // = 0.35

vec4 transition(vec2 uv) {
    float d = sin(progress * 3.14159265);
    vec2 c = uv - 0.5;
    vec3 acc = vec3(0.0);
    float wsum = 0.0;
    for (int i = 0; i < 16; i++) {
        float t = float(i) / 15.0;
        float s = 1.0 - strength * d * t;
        vec2 q = 0.5 + c * s;
        vec4 col = progress < 0.5 ? getFromColor(q) : getToColor(q);
        vec4 cr = progress < 0.5 ? getFromColor(0.5 + c * (s - 0.01 * d)) : getToColor(0.5 + c * (s - 0.01 * d));
        float w = 1.0 - t * 0.5;
        acc += vec3(cr.r, col.g, col.b) * w;
        wsum += w;
    }
    return vec4(acc / wsum, 1.0);
}
