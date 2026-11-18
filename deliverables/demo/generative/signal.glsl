// SIGNAL: the wood-wide web. Under a night forest floor, trees signal each
// other through a mycelial network. A tree fires; the signal runs out along
// the veins at a finite speed; each tree it reaches answers; the cascade
// lights the whole floor, and the veins converge into the season's name.
// Four shots: macro on the first root node; low across the floor among the
// trunks; a crane up over the cascade; top-down on the word.
// iChannel0: the word, white on black, framed as the final wide shot.

const float V = 120.0;    // signal speed, world px per second

// The trees: they are the senders, at the bell times of the score.
const vec2 TREE[7] = vec2[7](vec2(760.0, 600.0), vec2(1030.0, 470.0), vec2(520.0, 380.0), vec2(1350.0, 720.0), vec2(300.0, 800.0), vec2(1620.0, 330.0), vec2(1100.0, 930.0));
const float FIRE[7] = float[7](1.5, 4.0, 6.5, 8.0, 9.5, 10.5, 11.25);

// One family of veins: the 0.5 isoline of gradient noise at `scale`, as a
// true distance (value / gradient), drawn with width wpx and footprint fp
// (lines keep their energy and never alias). x = light, y = distance.
vec2 veinLine(vec2 q, float scale, float wpx, float fp) {
    float n = ak_gnoise(q / scale);
    vec2 g = vec2(ak_gnoise((q + vec2(2.0, 0.0)) / scale) - n, ak_gnoise((q + vec2(0.0, 2.0)) / scale) - n) * 0.5;
    float d = abs(n - 0.5) / max(length(g), 1e-6);
    float s = sqrt(wpx * wpx + fp * fp);
    return vec2(exp(-d * d / (2.0 * s * s)) * wpx / s, d);
}

// Mycelial veins at three scales (strands, veins, hairs). x = vein light,
// y = spore sparkle, z = distance to the nearest strand or vein.
vec3 veins(vec2 w, float fp) {
    vec2 q = w + 50.0 * (ak_warp(w * 0.002, 0.6) - 0.5);
    vec2 a = veinLine(q, 230.0, 1.7, fp);
    vec2 b = veinLine(q + 91.0, 80.0, 0.9, fp);
    vec2 c = veinLine(q + 37.0, 29.0, 0.45, fp);
    float hairs = c.x * 0.5 * smoothstep(0.3, 0.7, ak_noise(q * 0.008));
    float spore = pow(ak_hash(floor(q / 9.0)), 60.0) * exp(-b.y * 0.2) / (1.0 + fp);
    return vec3(min(a.x * 1.3 + b.x * 0.8 + hairs, 1.6), spore, min(a.y, b.y * 1.5));
}

// Arrival time of the signal at w: from the trees along wandering paths, then
// (from 11.5 s) a dense cascade of small answering fronts.
float FIRST;   // 1 when the first tree's signal arrived first, else 0 (set by arrival)
float arrival(vec2 w) {
    float wander = 1.0 + 0.35 * ak_noise(w * 0.01);
    float best = FIRE[0] + length(w - TREE[0]) * wander / V;
    FIRST = 1.0;
    for (int i = 1; i < 7; i++) {
        float a = FIRE[i] + length(w - TREE[i]) * wander / V;
        if (a < best) { best = a; FIRST = 0.0; }
    }
    vec2 g0 = floor(w / 220.0);
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec2 g = g0 + vec2(float(x), float(y));
            vec2 h = ak_hash2(g);
            float a = 11.5 + 2.4 * h.x + length(w - (g + 0.2 + 0.6 * h) * 220.0) * wander / (V * 1.6);
            if (a < best) { best = a; FIRST = 0.0; }
        }
    }
    return best;
}

// Signal state at w and time t: (front, charge).
vec2 signal(vec2 w, float t, float word, float reveal) {
    float since = t - arrival(w);
    float front = since > 0.0 ? exp(-since * 4.0) : exp(-since * since * 900.0);
    float charge = since > 0.0 ? 0.35 + 0.65 * exp(-since * 0.8) : 0.0;
    charge = mix(charge, mix(charge * 0.25, 1.4, word), reveal);
    return vec2(front, charge);
}

// The first tree speaks in cold light; every answer is warm.
vec3 signalColour(float first, float word, float reveal) {
    vec3 c = mix(vec3(1.0, 0.5, 0.15), vec3(0.45, 0.85, 1.0), first);
    return mix(c, vec3(1.0, 0.88, 0.66), reveal * word);
}

// The floor at w as seen with footprint fp; dist = distance from the camera
// (for fog), t = time.
vec3 floorAt(vec2 w, float fp, float t, float word, float reveal, float flash) {
    vec3 v = veins(w, fp);
    vec2 s = signal(w, t, word, reveal);
    s.x += flash * word;
    vec3 fc = signalColour(FIRST, word, reveal);
    float tex = ak_fbm(w * 0.015, 3);
    vec3 col = mix(vec3(0.0015, 0.0018, 0.003), vec3(0.006, 0.005, 0.006), tex);   // damp soil
    col += fc * exp(-v.z * 0.05) * (s.y * 0.5 + s.x * 1.2) * 0.12;               // glow on the soil
    col += vec3(0.005, 0.008, 0.02) * v.x * (1.0 - reveal * 0.6);                 // dormant veins
    col += fc * v.x * (s.x * 2.2 + s.y * 0.85);
    col += mix(fc, vec3(1.0), 0.5) * v.y * (s.x * 3.0 + s.y * 0.8);
    return col;
}

// Camera per shot. Returns false for a top-down shot (ortho: centre, zoom).
// Perspective: position (x = across, y = height, z = along) and target.
struct Cam { bool persp; vec2 centre; float zoom; vec3 pos; vec3 at; float focus; };

Cam camera(float t) {
    Cam c;
    c.persp = false; c.centre = vec2(760.0, 600.0); c.zoom = 1.0; c.pos = vec3(0.0); c.at = vec3(0.0); c.focus = 400.0;
    if (t < 4.0) {                                    // 1: macro on the first node, a slow push in
        c.zoom = mix(3.0, 3.8, smoothstep(0.0, 4.0, t));
        c.centre = vec2(760.0, 600.0) + vec2(-30.0, 10.0) * t;
    } else if (t < 9.5) {                             // 2: low across the floor, dollying forward
        float u = (t - 4.0) / 5.5;
        c.persp = true;
        c.pos = vec3(mix(860.0, 900.0, u), mix(55.0, 70.0, u), mix(80.0, 260.0, u));
        c.at = vec3(980.0, 0.0, 700.0);
        c.focus = mix(420.0, 380.0, u);
    } else if (t < 14.6) {                            // 3: crane up over the cascade
        float u = smoothstep(9.5, 14.6, t);
        c.persp = true;
        c.pos = vec3(mix(820.0, 960.0, u), mix(420.0, 1350.0, u), mix(160.0, 330.0, u));
        c.at = vec3(960.0, 0.0, mix(640.0, 560.0, u));
        c.focus = mix(600.0, 1400.0, u);
    } else if (t < 17.0) {                            // 4: top-down on the word as it ignites
        float u = smoothstep(14.6, 17.0, t);
        c.centre = vec2(960.0, 540.0);
        c.zoom = mix(0.97, 1.0, u);
    } else {                                          // 5: back down on the floor: the word lies among the trees
        float u = (t - 17.0) / 3.0;
        c.persp = true;
        c.pos = vec3(mix(1020.0, 930.0, u), mix(760.0, 720.0, u), mix(-320.0, -270.0, u));
        c.at = vec3(960.0, 0.0, 520.0);
        c.focus = 1050.0;
    }
    return c;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float t = iTime;
    vec2 px = ak_px(fragCoord);
    vec2 uv = ak_uv(fragCoord);
    Cam cam = camera(t);
    float reveal = smoothstep(14.8, 15.1, t);
    float flash = t > 15.0 ? exp(-(t - 15.0) * 2.5) * 2.0 : 0.0;    // the 15 s hit
    vec3 col;
    if (!cam.persp) {
        vec2 w = cam.centre + (px - vec2(960.0, 540.0)) / cam.zoom;
        vec2 wuv = w / vec2(1920.0, 1080.0);
        float word = (wuv.x > 0.0 && wuv.x < 1.0 && wuv.y > 0.0 && wuv.y < 1.0) ? smoothstep(0.35, 0.65, texture(iChannel0, wuv).r) : 0.0;
        col = floorAt(w, 1.0 / cam.zoom, t, word, reveal, flash);
        // Anticipation: an ember gathers at the first tree's node before it fires.
        float pre = t < 1.5 ? 0.25 + 0.75 * t / 1.5 : exp(-(t - 1.5) * 2.5);
        float r = length(w - TREE[0]);
        col += vec3(0.45, 0.85, 1.0) * pre * (exp(-r * 0.08) * 1.2 + exp(-r * 0.015) * 0.06 * (0.6 + 0.4 * sin(r * 0.15 - t * 9.0)));
    } else {
        vec3 fw = normalize(cam.at - cam.pos), rt = normalize(cross(fw, vec3(0.0, 1.0, 0.0))), up = cross(rt, fw);
        vec3 rd = normalize(fw * 1.7 - uv.x * rt + uv.y * up);
        vec3 fc = signalColour(t < 4.5 ? 1.0 : 0.0, 0.0, 0.0);
        // Sky: dark, with the floor's glow in the low mist.
        float horizonGlow = exp(-max(rd.y, 0.0) * 14.0);
        col = vec3(0.004, 0.005, 0.008) + fc * horizonGlow * 0.05 * smoothstep(4.0, 9.0, t);
        float tg = rd.y < -1e-4 ? -cam.pos.y / rd.y : 1e9;
        if (tg < 1e8) {
            vec3 p = cam.pos + rd * tg;
            float fp = tg * (2.0 / 1080.0) / 1.7 / max(-rd.y, 0.04);
            float defocus = abs(tg - cam.focus) * 0.012;
            vec2 wuv = p.xz / vec2(1920.0, 1080.0);
            float word = (t > 14.0 && wuv.x > 0.0 && wuv.x < 1.0 && wuv.y > 0.0 && wuv.y < 1.0) ? smoothstep(0.35, 0.65, texture(iChannel0, wuv).r) : 0.0;
            vec3 g = floorAt(p.xz, fp + defocus, t, word, reveal, flash) * (1.0 + 0.6 * smoothstep(300.0, 1300.0, cam.pos.y));
            float fog = exp(-tg * (t > 17.0 ? 0.0004 : 0.0011));
            col = mix(col, g, fog);
        }
        // Trunks: vertical cylinders at the trees, lit from below when their signal arrives.
        float near = tg;
        for (int i = 0; i < 19; i++) {
            if (cam.pos.y > 200.0 || (i < 7 && t > 17.0 && false)) break;
            vec2 tp = i < 7 ? TREE[i] : vec2(150.0 + 120.0 * float(i - 7) + 60.0 * ak_hash(float(i)), 900.0 + 260.0 * ak_hash(float(i) + 3.0));
            vec2 o = cam.pos.xz - tp;
            vec2 d = rd.xz;
            float r = i < 7 ? 16.0 + 6.0 * float(i % 3) : 9.0 + 6.0 * ak_hash(float(i) + 9.0);
            float A = dot(d, d), B = dot(o, d), C = dot(o, o) - r * r;
            float disc = B * B - A * C;
            if (disc > 0.0 && A > 1e-6) {
                float s = (-B - sqrt(disc)) / A;
                float hy = cam.pos.y + s * rd.y;
                if (s > 0.0 && s < near && hy > 0.0) {
                    near = s;
                    vec2 hit = cam.pos.xz + s * d;
                    vec2 nrm = normalize(hit - tp);
                    float since = i < 7 ? t - FIRE[i] : t - arrival(tp);
                    float lit = since > 0.0 ? 0.25 + exp(-since * 0.6) : 0.0;
                    float bark = 0.6 + 0.4 * ak_noise(vec2(atan(nrm.y, nrm.x) * 6.0, hy * 0.05));
                    vec3 trunk = vec3(0.0012, 0.001, 0.0009) * bark;
                    float facing = max(dot(nrm, -normalize(d)), 0.0);
                    trunk += fc * lit * exp(-hy * 0.06) * (0.2 + 0.8 * facing * facing) * bark * 0.3;
                    col = mix(col, trunk, exp(-s * 0.0011));
                }
            }
        }
    }
    col = ak_tonemap(col);
    col = ak_vignette(col, uv, 0.9);
    fragColor = vec4(ak_grain(col, fragCoord, 0.3), 1.0);
}
