// Aurora: curtains of light traced through stacked height layers, over a
// starfield and mirrored in a dark lake. Uniforms: speed, intensity, lake
// (1 = trace the lake here; 0 = sky only, then add the `lake-mirror` filter
// on top, which reflects the rendered frame: about a third faster, and it
// reflects any layer in between too).
uniform float speed;     // = 1.0
uniform float intensity; // = 1.0
uniform float lake;      // = 1.0

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
    // The sheet's path folds at two scales; where it turns edge-on it brightens.
    float fold = fbm(vec2(q.x * 0.22 + t * 0.12, t * 0.05)) * 7.0 - 3.5 + 1.1 * sin(q.x * 0.85 + t * 0.45);
    float d = abs(q.y - 6.0 - fold - 1.5 * sin(q.x * 0.27 + t * 0.3));
    // Far from the sheet the envelope bounds the contribution below 1e-4
    // (< 0.2/255 summed over every layer): skip the ray noise there.
    float env = exp(-d * 7.5) * 2.0 + exp(-d * 1.4) * 0.2;
    if (env < 1e-4) return 0.0;
    // Rays: coarse bands plus fine fast striations, dragged along by the folds.
    float coarse = vnoise(vec2(q.x * 5.0 + fold * 3.0 + t * 0.8, t * 0.2));
    float fine = vnoise(vec2(q.x * 21.0 + fold * 9.0 + t * 2.2, t * 0.6));
    float rays = 0.12 + 0.88 * (0.55 * coarse + 0.45 * fine * fine * 1.6);
    return env * rays;
}

vec2 pix;

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
    // Jitter the layer heights per pixel so 22 layers read as a continuous volume.
    // Interleaved gradient noise: spatially even, far less visible than white noise.
    float jit = fract(52.9829189 * fract(dot(pix, vec2(0.06711056, 0.00583715))));
    for (int i = 0; i < 22; i++) {
        float fi = (float(i) + jit) / 22.0;
        float y = 0.8 + fi * fi * 6.0;
        float dist = y / max(rd.y, 0.02);
        vec2 q = vec2(rd.x, -rd.z) * dist;
        float c = curtain(q, t);
        vec3 tint = mix(vec3(0.1, 1.0, 0.45), vec3(0.55, 0.2, 0.95), smoothstep(0.25, 0.9, fi));
        tint = mix(tint, vec3(1.0, 0.25, 0.35), smoothstep(0.85, 1.0, fi) * 0.6);
        acc += tint * c * (1.0 - fi) * (1.0 - fi) * exp(-dist * 0.02) * (0.4 + 3.0 * fi);
    }
    col += acc * 0.16 * intensity * smoothstep(0.0, 0.15, rd.y);
    return col;
}

// Far mountain ridge: elevation of its silhouette for a horizontal direction.
float ridge(float x) {
    float r = 0.018 + 0.05 * fbm(vec2(x * 0.9 + 3.0, 1.7));
    r += 0.022 * smoothstep(0.35, 0.9, fbm(vec2(x * 3.1, 5.3)));   // peaks
    r += 0.004 * vnoise(vec2(x * 40.0, 9.0));                       // forest fringe
    return r;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    pix = fragCoord;
    float t = iTime * speed;
    vec3 rd = normalize(vec3(uv.x, uv.y + 0.35, -1.6));
    float rx = rd.x / -rd.z;
    float rg = ridge(rx);
    vec3 col;
    if (rd.y > 0.0) {
        col = sky(rd, t);
        // Silhouette, with a faint aurora rim light on its crest.
        float e = rd.y - rg;
        col = mix(vec3(0.004, 0.007, 0.009), col, smoothstep(-0.0008, 0.0008, e));
        col += vec3(0.05, 0.25, 0.14) * 0.15 * exp(-max(e, 0.0) * 260.0) * step(0.0, e) * intensity;
    } else if (lake < 0.5) {
        col = vec3(0.0);   // left for the lake-mirror filter
    } else {
        // Lake: the mirrored sky and ridge, rippled and darkened, brightest at grazing angles.
        vec3 r = vec3(rd.x, -rd.y, rd.z);
        float rip = vnoise(vec2(rd.x / rd.y * 8.0, 1.0 / rd.y * 2.0 + t * 0.5)) - 0.5;
        r.x += rip * 0.08 * -rd.y;
        r.y += rip * 0.02 * -rd.y;
        r = normalize(r);
        vec3 sk = sky(r, t);
        float e = r.y - ridge(r.x / -r.z);
        sk = mix(vec3(0.003, 0.005, 0.007), sk, smoothstep(-0.002, 0.002, e));
        float fres = 0.25 + 0.5 * exp(rd.y * 12.0);
        col = sk * fres + vec3(0.0, 0.003, 0.006);
    }
    col = pow(col, vec3(0.4545));
    fragColor = vec4(col, 1.0);
}
