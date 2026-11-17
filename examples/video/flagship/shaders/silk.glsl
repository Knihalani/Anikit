// Silk: oxblood satin falling in slow folds. A height field of a few drifting folds,
// warped by low-frequency noise, lit by a soft key with a long anisotropic sheen.
uniform float speed; // = 1.0
uniform float light; // = 0.0  0 = soft daylight, 1 = a single hard slit of light
uniform float slit;  // = 0.5  where the slit is, 0..1 across

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}

float height(vec2 p, float t) {
    vec2 q = p + 0.35 * vec2(noise(p * 1.3 + t * 0.1), noise(p * 1.3 - t * 0.12 + 7.0));
    float h = 0.0;
    h += 0.55 * sin(q.x * 2.1 + q.y * 0.6 + t * 0.35);
    h += 0.30 * sin(q.x * 3.7 - q.y * 1.1 - t * 0.27 + 1.3);
    h += 0.18 * sin(q.x * 6.3 + q.y * 1.9 + t * 0.5 + 2.1);
    h += 0.08 * sin(q.x * 11.0 - q.y * 3.0 + t * 0.8);
    return h;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.y;
    float t = iTime * speed;
    vec2 p = uv * vec2(2.2, 1.6);
    float e = 0.003;
    float h = height(p, t);
    vec3 n = normalize(vec3(-(height(p + vec2(e, 0), t) - h) / e, -(height(p + vec2(0, e), t) - h) / e, 6.0));
    vec3 v = vec3(0.0, 0.0, 1.0);
    vec3 l = normalize(vec3(-0.5, 0.6, 0.65));
    vec3 hv = normalize(l + v);
    // satin: a broad diffuse plus a stretched highlight across the fold direction
    vec3 tng = normalize(vec3(1.0, 0.25, 0.0));
    float th = dot(tng, hv), nh = dot(n, hv);
    float aniso = pow(max(0.0, sqrt(max(0.0, 1.0 - th * th)) * nh * 1.0), 60.0);
    float dif = max(dot(n, l), 0.0);
    vec3 base = vec3(0.30, 0.03, 0.06);
    vec3 col = base * (0.25 + 0.95 * dif) + vec3(1.0, 0.55, 0.6) * aniso * 0.9;
    col += vec3(0.6, 0.1, 0.15) * pow(1.0 - n.z, 2.0) * 0.6;   // sheen at grazing folds
    // the slit of light (for "Shadow."): everything dark except a vertical band
    float band = exp(-pow((uv.x / (iResolution.x / iResolution.y) - slit) * 11.0, 2.0));
    col = mix(col, col * (0.08 + 2.2 * band), light);
    col = col / (1.0 + col);
    col = pow(col * 1.6, vec3(0.4545));
    fragColor = vec4(col, 1.0);
}
