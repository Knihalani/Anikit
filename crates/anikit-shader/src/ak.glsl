// The ak_ standard library: building blocks a shader can call without
// defining them. Included automatically when the source mentions `ak_`.
// Docs: docs/video/shader.md, "Building blocks".

// ---- coordinates
// Centred, aspect-correct coordinates: y from -1 (bottom) to 1 (top).
vec2 ak_uv(vec2 fragCoord) { return (2.0 * fragCoord - iResolution.xy) / iResolution.y; }
// 0..1 across the frame.
vec2 ak_uv01(vec2 fragCoord) { return fragCoord / iResolution.xy; }
mat2 ak_rot(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }

// ---- hashes (arithmetic, no sin: fast and well distributed)
float ak_hash(float p) { p = fract(p * 0.1031); p *= p + 33.33; p *= p + p; return fract(p); }
float ak_hash(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
float ak_hash(vec3 p3) { p3 = fract(p3 * 0.1031); p3 += dot(p3, p3.zyx + 31.32); return fract((p3.x + p3.y) * p3.z); }
vec2 ak_hash2(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973)); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.xx + p3.yz) * p3.zy); }
vec3 ak_hash3(vec3 p3) { p3 = fract(p3 * vec3(0.1031, 0.1030, 0.0973)); p3 += dot(p3, p3.yxz + 33.33); return fract((p3.xxy + p3.yxx) * p3.zyx); }

// ---- noise (all roughly in [0, 1])
// Value noise.
float ak_noise(vec2 p) {
    vec2 i = floor(p), f = fract(p), u = f * f * (3.0 - 2.0 * f);
    return mix(mix(ak_hash(i), ak_hash(i + vec2(1, 0)), u.x), mix(ak_hash(i + vec2(0, 1)), ak_hash(i + vec2(1, 1)), u.x), u.y);
}
float ak_noise(vec3 p) {
    vec3 i = floor(p), f = fract(p), u = f * f * (3.0 - 2.0 * f);
    float a = mix(mix(ak_hash(i), ak_hash(i + vec3(1, 0, 0)), u.x), mix(ak_hash(i + vec3(0, 1, 0)), ak_hash(i + vec3(1, 1, 0)), u.x), u.y);
    float b = mix(mix(ak_hash(i + vec3(0, 0, 1)), ak_hash(i + vec3(1, 0, 1)), u.x), mix(ak_hash(i + vec3(0, 1, 1)), ak_hash(i + vec3(1, 1, 1)), u.x), u.y);
    return mix(a, b, u.z);
}
// Gradient (Perlin-style) noise: smoother, no grid look.
float ak_gnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p), u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    vec2 g00 = ak_hash2(i) * 2.0 - 1.0, g10 = ak_hash2(i + vec2(1, 0)) * 2.0 - 1.0;
    vec2 g01 = ak_hash2(i + vec2(0, 1)) * 2.0 - 1.0, g11 = ak_hash2(i + vec2(1, 1)) * 2.0 - 1.0;
    float n = mix(mix(dot(g00, f), dot(g10, f - vec2(1, 0)), u.x), mix(dot(g01, f - vec2(0, 1)), dot(g11, f - vec2(1, 1)), u.x), u.y);
    return n * 0.7 + 0.5;
}
// Fractal sum of `oct` octaves (1..8) of gradient noise, rotated per octave.
float ak_fbm(vec2 p, int oct) {
    float s = 0.0, a = 0.5, w = 0.0;
    mat2 r = mat2(0.8, -0.6, 0.6, 0.8);
    for (int i = 0; i < 8; i++) {
        if (i >= oct) break;
        s += a * ak_gnoise(p);
        w += a;
        p = r * p * 2.02 + 17.3;
        a *= 0.5;
    }
    return s / w;
}
float ak_fbm(vec3 p, int oct) {
    float s = 0.0, a = 0.5, w = 0.0;
    for (int i = 0; i < 8; i++) {
        if (i >= oct) break;
        s += a * ak_noise(p);
        w += a;
        p = p * 2.03 + vec3(17.3, -9.1, 4.7);
        a *= 0.5;
    }
    return s / w;
}
// Ridged fbm: sharp creases (mountains, veins, lightning).
float ak_ridged(vec2 p, int oct) {
    float s = 0.0, a = 0.5, w = 0.0;
    for (int i = 0; i < 8; i++) {
        if (i >= oct) break;
        float n = 1.0 - abs(ak_gnoise(p) * 2.0 - 1.0);
        s += a * n * n;
        w += a;
        p = p * 2.07 + 11.1;
        a *= 0.5;
    }
    return s / w;
}
// Domain warp: p pushed along two fbm fields (marbling, smoke, ink).
vec2 ak_warp(vec2 p, float amount) {
    vec2 q = vec2(ak_fbm(p + vec2(0.0, 0.0), 4), ak_fbm(p + vec2(5.2, 1.3), 4));
    return p + (q - 0.5) * 4.0 * amount;
}
// Divergence-free 2D flow (curl of a noise field): advect particles or ink.
vec2 ak_curl(vec2 p) {
    float e = 0.01;
    float a = ak_gnoise(p + vec2(0, e)), b = ak_gnoise(p - vec2(0, e));
    float c = ak_gnoise(p + vec2(e, 0)), d = ak_gnoise(p - vec2(e, 0));
    return vec2(a - b, d - c) / (2.0 * e);
}
// Voronoi: x = distance to the nearest cell point, y = to the second nearest,
// z = the nearest cell's id hash. Cells, cracks, stone, foam.
vec3 ak_voronoi(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    float d1 = 8.0, d2 = 8.0, id = 0.0;
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec2 g = vec2(float(x), float(y));
            vec2 o = ak_hash2(i + g);
            float d = length(g + o - f);
            if (d < d1) { d2 = d1; d1 = d; id = ak_hash(i + g); } else if (d < d2) { d2 = d; }
        }
    }
    return vec3(d1, d2, id);
}

// ---- colour
// Cosine palette: a + b·cos(2π(c·t + d)) (four vec3s pick the look).
vec3 ak_palette(float t, vec3 a, vec3 b, vec3 c, vec3 d) { return a + b * cos(6.2831853 * (c * t + d)); }
// sRGB ↔ linear (exact curves).
vec3 ak_linear(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c)); }
vec3 ak_srgb(vec3 c) { c = max(c, 0.0); return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c)); }
// Film-like tone map (ACES fit) for linear HDR colour, then display gamma.
vec3 ak_tonemap(vec3 c) { c = max(c, 0.0); c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14); return ak_srgb(clamp(c, 0.0, 1.0)); }
// Perceptual mix (OKLab): no muddy midpoints between saturated colours.
vec3 ak_oklab(vec3 c) {
    vec3 lms = mat3(0.4122214708, 0.2119034982, 0.0883024619, 0.5363325363, 0.6806995451, 0.2817188376, 0.0514459929, 0.1073969566, 0.6299787005) * c;
    lms = sign(lms) * pow(abs(lms), vec3(1.0 / 3.0));
    return mat3(0.2104542553, 1.9779984951, 0.0259040371, 0.7936177850, -2.4285922050, 0.7827717662, -0.0040720468, 0.4505937099, -0.8086757660) * lms;
}
vec3 ak_oklab_inv(vec3 c) {
    vec3 lms = mat3(1.0, 1.0, 1.0, 0.3963377774, -0.1055613458, -0.0894841775, 0.2158037573, -0.0638541728, -1.2914855480) * c;
    lms = lms * lms * lms;
    return mat3(4.0767416621, -1.2684380046, -0.0041960863, -3.3077115913, 2.6097574011, -0.7034186147, 0.2309699292, -0.3413193965, 1.7076147010) * lms;
}
vec3 ak_mix(vec3 a, vec3 b, float t) { return ak_oklab_inv(mix(ak_oklab(a), ak_oklab(b), t)); }
// Thin-film interference tint (soap bubbles, oil, anodised metal) for a view cosine.
vec3 ak_iridescence(float cosTheta, float thickness) {
    float x = thickness * 2.4 * (1.0 - cosTheta) + thickness * 0.6;
    return 0.5 + 0.5 * cos(6.2831853 * (x * vec3(1.0, 1.12, 1.27) + vec3(0.0, 0.33, 0.67)));
}

// ---- 2D signed distances (negative inside)
float ak_circle(vec2 p, float r) { return length(p) - r; }
float ak_box(vec2 p, vec2 b) { vec2 d = abs(p) - b; return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0); }
float ak_segment(vec2 p, vec2 a, vec2 b) { vec2 pa = p - a, ba = b - a; return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0)); }
// Anti-aliased fill of a distance: 1 inside, 0 outside, one pixel of edge.
float ak_fill(float d) { float w = fwidth(d); return clamp(0.5 - d / max(w, 1e-6), 0.0, 1.0); }

// ---- 3D signed distances and operators
float ak_sphere(vec3 p, float r) { return length(p) - r; }
float ak_box(vec3 p, vec3 b) { vec3 d = abs(p) - b; return length(max(d, 0.0)) + min(max(d.x, max(d.y, d.z)), 0.0); }
float ak_torus(vec3 p, vec2 t) { vec2 q = vec2(length(p.xz) - t.x, p.y); return length(q) - t.y; }
float ak_capsule(vec3 p, vec3 a, vec3 b, float r) { vec3 pa = p - a, ba = b - a; return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0)) - r; }
// Smooth union / subtraction with blend radius k.
float ak_smin(float a, float b, float k) { float h = max(k - abs(a - b), 0.0) / k; return min(a, b) - h * h * k * 0.25; }
float ak_smax(float a, float b, float k) { return -ak_smin(-a, -b, k); }
// A camera ray: from ro looking at ta, for centred coords uv (ak_uv), lens zoom.
vec3 ak_ray(vec3 ro, vec3 ta, vec2 uv, float zoom) {
    vec3 w = normalize(ta - ro), u = normalize(cross(w, vec3(0, 1, 0))), v = cross(u, w);
    return normalize(uv.x * u + uv.y * v + zoom * w);
}

// ---- finishing
// Film grain: zero-mean, animated, strongest in the midtones.
vec3 ak_grain(vec3 c, vec2 fragCoord, float amount) {
    float n = ak_hash(fragCoord + fract(float(iFrame) * 0.6180339) * 113.0) - 0.5;
    float l = dot(c, vec3(0.299, 0.587, 0.114));
    return c + n * amount * 0.12 * (l * (1.0 - l) * 3.0 + 0.1);
}
// Vignette for centred coords: 0 none … 1 strong.
vec3 ak_vignette(vec3 c, vec2 uv, float amount) { return c * (1.0 - amount * 0.35 * dot(uv * 0.6, uv * 0.6)); }
