// Flow-field ink: dye advected through a curl-noise velocity field, folding
// into marbled filaments. Two inks on paper. Uniforms: speed, ink, ink2, paper.
uniform float speed; // = 1.0
uniform vec3 ink;    // = vec3(0.05, 0.18, 0.45)
uniform vec3 ink2;   // = vec3(0.85, 0.32, 0.18)
uniform vec3 paper;  // = vec3(0.95, 0.93, 0.88)

vec2 hash22(vec2 p) {
    uvec2 q = uvec2(ivec2(floor(p)) + 32768);
    q = q * uvec2(1597334673u, 3812015801u);
    q = (q.x ^ q.y) * uvec2(1597334673u, 3812015801u);
    return vec2(q) * (2.0 / 4294967295.0) - 1.0;
}

// Gradient noise.
float gnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float a = dot(hash22(i), f);
    float b = dot(hash22(i + vec2(1, 0)), f - vec2(1, 0));
    float c = dot(hash22(i + vec2(0, 1)), f - vec2(0, 1));
    float d = dot(hash22(i + vec2(1, 1)), f - vec2(1, 1));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Divergence-free velocity: the curl of a scalar potential.
vec2 curl(vec2 p, float t) {
    const float e = 0.02;
    float n1 = gnoise(p + vec2(0, e) + t * 0.1) + 0.5 * gnoise(p * 2.1 + vec2(0, e) - t * 0.13);
    float n2 = gnoise(p - vec2(0, e) + t * 0.1) + 0.5 * gnoise(p * 2.1 - vec2(0, e) - t * 0.13);
    float n3 = gnoise(p + vec2(e, 0) + t * 0.1) + 0.5 * gnoise(p * 2.1 + vec2(e, 0) - t * 0.13);
    float n4 = gnoise(p - vec2(e, 0) + t * 0.1) + 0.5 * gnoise(p * 2.1 - vec2(e, 0) - t * 0.13);
    return vec2(n1 - n2, n4 - n3) / (2.0 * e);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float t = iTime * speed;
    // Trace the dye backwards along the flow.
    vec2 p = uv * 2.2;
    for (int i = 0; i < 24; i++) {
        p -= curl(p, t) * 0.012;
    }
    // Initial dye: stripes that the flow then marbles.
    float s = sin(p.x * 7.0 + 1.3 * sin(p.y * 2.0));
    float band = smoothstep(-0.12, 0.12, s);
    float edge = 1.0 - smoothstep(0.0, 0.18, abs(s));
    float fil = smoothstep(0.55, 0.98, sin(p.y * 23.0 + p.x * 3.0) * 0.5 + 0.5);
    vec3 col = mix(paper, ink, band * 0.85);
    col = mix(col, ink2, (1.0 - band) * fil * 0.75);
    col = mix(col, paper * 0.98, edge * 0.5);
    // Paper grain.
    col *= 0.97 + 0.03 * gnoise(fragCoord * 0.6);
    fragColor = vec4(col, 1.0);
}
