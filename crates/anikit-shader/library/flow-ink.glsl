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

// Gradient noise with its analytic gradient: (value, d/dx, d/dy).
vec3 gnoised(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    vec2 du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    vec2 ga = hash22(i), gb = hash22(i + vec2(1, 0)), gc = hash22(i + vec2(0, 1)), gd = hash22(i + vec2(1, 1));
    float a = dot(ga, f), b = dot(gb, f - vec2(1, 0)), c = dot(gc, f - vec2(0, 1)), d = dot(gd, f - vec2(1, 1));
    float k = a - b - c + d;
    float n = a + (b - a) * u.x + (c - a) * u.y + k * u.x * u.y;
    vec2 g = ga + (gb - ga) * u.x + (gc - ga) * u.y + (ga - gb - gc + gd) * u.x * u.y
           + du * vec2(b - a + k * u.y, c - a + k * u.x);
    return vec3(n, g);
}

float gnoise(vec2 p) { return gnoised(p).x; }

// Divergence-free velocity: the curl of a two-octave potential, from its
// analytic gradient (one noise evaluation per octave).
vec2 curl(vec2 p, float t) {
    vec3 a = gnoised(p + t * 0.1);
    vec3 b = gnoised(p * 2.1 - t * 0.13);
    vec2 g = a.yz + 0.5 * 2.1 * b.yz;
    return vec2(g.y, -g.x);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float t = iTime * speed;
    // Trace the dye backwards along the flow.
    vec2 p = uv * 2.2;
    for (int i = 0; i < 12; i++) {
        p -= curl(p, t) * 0.024;
    }
    // Initial dye: stripes that the flow then marbles.
    float s = sin(p.x * 7.0 + 1.3 * sin(p.y * 2.0));
    float aa = fwidth(s) + 1e-4;
    float band = smoothstep(-aa, aa, s);
    float edge = 1.0 - smoothstep(0.0, 0.12 + aa, abs(s));
    float f2 = sin(p.y * 23.0 + p.x * 3.0);
    float fil = smoothstep(0.55 - fwidth(f2), 0.55 + fwidth(f2), f2);
    vec3 col = mix(paper, ink, band * 0.85);
    col = mix(col, ink2, (1.0 - band) * fil * 0.75);
    col = mix(col, paper * 0.98, edge * 0.5);
    // Paper grain.
    col *= 0.97 + 0.03 * gnoise(fragCoord * 0.6);
    fragColor = vec4(col, 1.0);
}
