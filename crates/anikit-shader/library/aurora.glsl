// Aurora: curtains of light traced through stacked height layers, over a
// starfield and mirrored in a dark lake. Uniforms: speed, intensity.
uniform float speed;     // = 1.0
uniform float intensity; // = 1.0

uint hashu(uint x) { x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16; return x; }
float hash2(vec2 p) { return float(hashu(uint(int(p.x)) * 1973u + hashu(uint(int(p.y)) + 9277u))) / 4294967295.0; }

float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2(1, 0)), u.x), mix(hash2(i + vec2(0, 1)), hash2(i + vec2(1, 1)), u.x), u.y);
}

float fbm(vec2 p) {
    float s = 0.0, a = 0.5;
    for (int i = 0; i < 3; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(17.1, 3.7); a *= 0.5; }
    return s;
}

// Curtain brightness at ground-plane position q (x across, z away).
float curtain(vec2 q, float t) {
    float fold = fbm(vec2(q.x * 0.18 + t * 0.12, t * 0.05)) * 6.0 - 3.0;   // where the sheet runs
    float d = abs(q.y - 9.0 - fold - 1.2 * sin(q.x * 0.21 + t * 0.3));
    float rays = 0.45 + 0.55 * vnoise(vec2(q.x * 3.0 + t * 0.8, t * 0.2)); // vertical striations
    return exp(-d * 0.9) * rays;
}

vec3 sky(vec3 rd, float t) {
    vec3 col = mix(vec3(0.004, 0.01, 0.025), vec3(0.02, 0.04, 0.08), exp(-rd.y * 3.0));
    // Stars: one per cell of a direction grid, twinkling.
    vec2 sp = rd.xz / (rd.y + 0.35) * 160.0;
    vec2 cell = floor(sp);
    float h = hash2(cell);
    if (h > 0.985) {
        vec2 c = cell + 0.5 + 0.35 * (vec2(hash2(cell + 7.0), hash2(cell + 13.0)) - 0.5);
        float tw = 0.7 + 0.3 * sin(t * 3.0 + h * 80.0);
        col += vec3(0.8, 0.85, 1.0) * tw * smoothstep(0.08, 0.0, length(sp - c) / 6.0) * (h - 0.985) * 60.0;
    }
    // Aurora: march stacked layers between heights 1.0 and 3.0.
    vec3 acc = vec3(0.0);
    for (int i = 0; i < 28; i++) {
        float fi = float(i) / 28.0;
        float y = 1.0 + fi * 2.2;
        float dist = y / max(rd.y, 0.02);
        vec2 q = vec2(rd.x, -rd.z) * dist;
        float c = curtain(q, t);
        vec3 tint = mix(vec3(0.1, 1.0, 0.45), vec3(0.55, 0.2, 0.95), smoothstep(0.25, 0.9, fi));
        tint = mix(tint, vec3(1.0, 0.25, 0.35), smoothstep(0.85, 1.0, fi) * 0.6);
        acc += tint * c * (1.0 - fi) * exp(-dist * 0.025);
    }
    col += acc * 0.12 * intensity * smoothstep(0.0, 0.15, rd.y);
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    float t = iTime * speed;
    vec3 rd = normalize(vec3(uv.x, uv.y + 0.35, -1.6));
    vec3 col;
    if (rd.y > 0.0) {
        col = sky(rd, t);
    } else {
        // Lake: mirrored sky, slightly rippled and darkened.
        vec3 r = vec3(rd.x, -rd.y, rd.z);
        float rip = vnoise(vec2(rd.x / rd.y * 8.0, 1.0 / rd.y * 2.0 + t * 0.5));
        r.x += (rip - 0.5) * 0.02 * -rd.y;
        col = sky(normalize(r), t) * 0.55 + vec3(0.0, 0.004, 0.008);
    }
    // Conifer treeline on the far shore (y = horizon).
    float hz = -0.35 + 0.35 * 0.0;
    float x = uv.x * 14.0;
    float tree = 0.012 + 0.02 * fbm(vec2(uv.x * 2.0, 3.0));
    float cell = fract(x) - 0.5;
    float th = 0.03 + 0.05 * hash2(vec2(floor(x), 1.0));
    tree = max(tree, th * (1.0 - 2.2 * abs(cell)) * step(0.35, hash2(vec2(floor(x), 2.0))));
    float yy = uv.y - hz + 0.0;
    float hor = (vec3(uv.x, uv.y + 0.35, -1.6).y) / length(vec3(uv.x, uv.y + 0.35, -1.6));
    if (hor > -0.004 && hor < tree) col = vec3(0.003, 0.005, 0.008);
    col = pow(col, vec3(0.4545));
    fragColor = vec4(col, 1.0);
}
