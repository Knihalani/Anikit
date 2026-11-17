// Transition: punch-in zoom blur with chromatic fringes; the cut happens at
// peak blur. Uniforms: strength.
uniform float strength; // = 0.35  zoom amount at the cut

// One scene per frame: the branch is uniform, so only one texture is read.
vec4 tap(vec2 q) {
    if (progress < 0.5) return getFromColor(q);
    return getToColor(q);
}

vec4 transition(vec2 uv) {
    float d = sin(progress * 3.14159265);
    // At rest (no blur) every tap lands on the same pixel: sample once.
    if (d < 1e-4) return tap(uv);
    vec2 c = uv - 0.5;
    // Thirteen taps along the zoom ray. Each tap's green/blue pairs with the
    // next tap's red: the fringe for free, one texture read per tap.
    const int N = 12;
    float step_ = strength * d / float(N - 1);
    vec4 next = tap(0.5 + c);
    vec3 acc = vec3(0.0);
    float wsum = 0.0;
    for (int i = 0; i < N; i++) {
        vec4 cur = next;
        next = tap(0.5 + c * (1.0 - step_ * float(i + 1)));
        float w = 1.0 - 0.5 * float(i) / float(N - 1);
        acc += vec3(next.r, cur.g, cur.b) * w;
        wsum += w;
    }
    return vec4(acc / wsum, 1.0);
}
