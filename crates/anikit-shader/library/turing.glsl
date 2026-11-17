// Turing patterns: the spots-and-stripes look of reaction–diffusion, made
// with a band-pass (difference of blurred noises) and a sharp threshold, so
// it needs no simulation and is the same at any frame. Uniforms: scale,
// balance (spots ↔ maze), speed, colours.
uniform float scale;   // = 1.0
uniform float balance; // = 0.0
uniform float speed;   // = 1.0
uniform vec3 colorA;   // = vec3(0.06, 0.07, 0.10)
uniform vec3 colorB;   // = vec3(0.98, 0.72, 0.30)

vec2 hash22(vec2 p) {
    uvec2 q = uvec2(ivec2(floor(p)) + 32768);
    q = q * uvec2(1597334673u, 3812015801u);
    q = (q.x ^ q.y) * uvec2(1597334673u, 3812015801u);
    return vec2(q) * (2.0 / 4294967295.0) - 1.0;
}

float gnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(mix(dot(hash22(i), f), dot(hash22(i + vec2(1, 0)), f - vec2(1, 0)), u.x),
               mix(dot(hash22(i + vec2(0, 1)), f - vec2(0, 1)), dot(hash22(i + vec2(1, 1)), f - vec2(1, 1)), u.x), u.y);
}

// Activator minus inhibitor: two octaves of noise at a fixed wavelength ratio.
float field(vec2 p, float t) {
    mat2 r = mat2(0.8, 0.6, -0.6, 0.8);
    float a = 0.0, w = 1.0;
    vec2 q = p;
    for (int i = 0; i < 4; i++) {
        a += w * gnoise(q + t * 0.05 * float(i + 1));
        q = r * q * 1.9;
        w *= 0.45;
    }
    float inh = gnoise(p * 0.5 - t * 0.03) * 0.8;
    return a - inh * 0.35;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float t = iTime * speed;
    vec2 p = uv * 9.0 * scale;
    // Warp so stripes meander like a real labyrinth pattern.
    p += 0.6 * vec2(gnoise(p * 0.35 + t * 0.02), gnoise(p * 0.35 + 7.1 - t * 0.02));
    float f = field(p, t) + balance * 0.15;
    float w = fwidth(f) * 1.2;
    float m = smoothstep(-w, w, f);
    // Soft shading at the boundaries, like dye pooling at edges.
    float rim = 1.0 - smoothstep(0.0, 0.08, abs(f));
    vec3 col = mix(colorA, colorB, m);
    col *= 1.0 - 0.25 * rim;
    col += colorB * 0.12 * smoothstep(0.08, 0.3, f);
    fragColor = vec4(col, 1.0);
}
