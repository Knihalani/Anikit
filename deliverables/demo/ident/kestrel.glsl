// Kestrel: the still point. A kestrel hovers into a gale: wings, tail, body, grass, cloud and
// light all move; the head (and the eye) never does. Then it stoops, and the eye stays where it
// was, cooling into the full stop of the wordmark.
// Screen space: y-up, units of frame height. The eye is pinned at E for the whole film.

uniform float pass; // = 0.0   0: world + bird + dot, 1: near seeds only (above the type)

const vec2 E = vec2(0.4296, -0.0833);     // the full stop's centre (matches the type layer)
const float TS = 3.30;                    // the stoop
const vec2 EYE = vec2(-0.012, 0.008);     // eye in bird coordinates
const vec2 SUN = vec2(-0.74, 0.67);       // direction towards the low sun (off frame, upper left)

float h1(float n) { return fract(sin(n) * 43758.5453); }
float h2(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
vec2 h22(vec2 p) { return fract(sin(vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(h2(i), h2(i + vec2(1, 0)), u.x), mix(h2(i + vec2(0, 1)), h2(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p) {
    float s = 0.0, a = 0.5;
    for (int i = 0; i < 5; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
    return s;
}
mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }
float smin(float a, float b, float k) { float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0); return mix(b, a, h) - k * h * (1.0 - h); }
// distance to segment a-b, and the position along it
vec2 seg(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return vec2(length(pa - ba * h), h);
}
float sdTri(vec2 p, vec2 a, vec2 b, vec2 c) {
    vec2 e0 = b - a, e1 = c - b, e2 = a - c, v0 = p - a, v1 = p - b, v2 = p - c;
    vec2 pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    vec2 pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    vec2 pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    float s = sign(e0.x * e2.y - e0.y * e2.x);
    vec2 d = min(min(vec2(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)), vec2(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))), vec2(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

// ---- timing -------------------------------------------------------------------------------
float gust(float t) { return 0.55 + 0.45 * smoothstep(0.0, 3.2, t) + 0.18 * sin(t * 3.1) * sin(t * 1.3 + 1.0); }
float zoomAt(float t) {
    float e = smoothstep(0.25, 3.05, t);
    e = e * e * (3.0 - 2.0 * e);
    return exp(mix(log(6.5), log(1.25), e));
}

// ---- the bird ------------------------------------------------------------------------------
// Seen side-on, facing left into the wind, backlit by the low sun: a dark profile whose feather
// fringe and wing feathers glow where the light comes through.
float wingSd(vec2 q, vec2 S, float ang, float L, out float u, out float v) {
    vec2 dir = vec2(cos(ang), sin(ang));
    vec2 n = vec2(-dir.y, dir.x);
    vec2 r = q - S;
    float x = dot(r, dir);
    v = dot(r, n);
    u = x / L;
    float uc = clamp(u, 0.0, 1.0);
    float lead = 0.010 * (1.0 - uc) + 0.002;
    float chord = 0.070 * pow(1.0 - uc, 0.6) * (1.0 - 0.25 * uc) + 0.002;
    chord += 0.0022 * smoothstep(0.55, 0.8, uc) * abs(sin(uc * 52.0));    // separated primary tips
    float dv = max(v - lead, -chord - v);
    float du = max(-x - 0.01, x - L * (1.0 - 0.08 * smoothstep(-0.07, 0.0, v)));
    return max(dv, du);
}

float bodySd(vec2 q, float tt, float G, float s, out float dHead, out float dBody, out float dTail, out float tsy) {
    float ph = 6.2830 * 6.1 * tt;
    float calm = 1.0 - smoothstep(0.0, 0.12, s);
    vec2 b = vec2(0.003 * sin(ph + 0.6), 0.006 * sin(ph + 1.2)) * calm;
    float sway = (0.08 * sin(tt * 1.9) + 0.04 * sin(tt * 4.3)) * G * calm;
    // head: flat crown, slightly longer than tall
    vec2 hq = q - vec2(0.002, 0.0);
    dHead = length(hq / vec2(1.08, 0.92)) - 0.030;
    dHead = max(dHead, q.y - 0.024 - 0.006 * smoothstep(-0.02, 0.02, q.x));
    // body: deep chest, inclined about 35 degrees, top forward
    vec2 C = vec2(0.034, -0.082) + b;
    vec2 ax = rot(-sway) * normalize(vec2(-0.48, 1.0));
    vec2 bq = C - q;
    vec2 lq = vec2(dot(-bq, vec2(ax.y, -ax.x)), dot(-bq, ax));
    float chest = smoothstep(0.06, -0.02, lq.y);
    dBody = length(lq / vec2(0.040 + 0.006 * chest, 0.080)) - 1.0;
    dBody *= 0.042;
    // tail: long, fanned and pressed down against the wind
    vec2 T0 = C - ax * 0.055;
    vec2 tdir = rot(0.25 * sin(tt * 2.3) * G * calm - 0.6 * sway) * normalize(vec2(0.35, -1.0));
    vec2 ts = seg(q, T0, T0 + tdir * 0.135);
    tsy = ts.y;
    dTail = ts.x - mix(0.013, 0.036 + 0.006 * sin(tt * 3.7) * calm, ts.y);
    dTail = max(dTail, dot(q - (T0 + tdir * 0.125), tdir));
    float d = smin(dHead, dBody, 0.016);
    d = smin(d, dTail, 0.010);
    return d;
}

// premultiplied colour + coverage of the bird at local time tt
vec4 bird(vec2 q0, vec3 behind, float tt, float px, float G) {
    float s = max(tt - TS, 0.0);
    // stoop: tip nose-down and fall away (weight: a slow first frame, then gravity)
    float a = 1.15 * smoothstep(0.0, 0.16, s);
    vec2 drop = vec2(-0.10 * s, -(0.25 * s + 4.6 * s * s));
    vec2 P = vec2(0.03, -0.07);
    vec2 q = rot(a) * (q0 - drop - P) + P;
    float ph = 6.2830 * 6.1 * tt;
    float ruff = (fbm(q * 300.0 + vec2(-tt * 46.0 * G, tt * 4.0)) - 0.5) * 0.0028 * G;
    float dHead, dBody, dTail, tsy;
    float d = bodySd(q, tt, G, s, dHead, dBody, dTail, tsy) + ruff;
    // hooked beak: the upper mandible curves over and down to the tip
    float dBeak = max(length(q - vec2(-0.025, -0.019)) - 0.024, -(q.y + 0.009 - 0.30 * (q.x + 0.022)));
    dBeak = max(dBeak, q.x + 0.018);
    d = min(d, dBeak);

    // plumage, seen dimly on the shadow side
    vec3 slate = vec3(0.42, 0.46, 0.52), cream = vec3(0.82, 0.74, 0.60), rufous = vec3(0.62, 0.28, 0.12);
    vec3 ink = vec3(0.06, 0.045, 0.035), yellow = vec3(0.93, 0.68, 0.16);
    float barb = vnoise(rot(0.45) * q * vec2(80.0, 900.0));
    vec3 col = slate * (0.8 + 0.3 * barb);
    float cheek = smoothstep(0.004, -0.004, length((q - vec2(-0.006, -0.016)) / vec2(1.2, 1.0)) - 0.016);
    col = mix(col, cream * (0.85 + 0.2 * barb), cheek);
    vec2 ms = seg(q, EYE + vec2(0.001, -0.007), EYE + vec2(0.004, -0.030));
    col = mix(col, ink, 0.55 * smoothstep(0.0015, -0.0015, ms.x - mix(0.0040, 0.0020, ms.y)));
    float onBody = smoothstep(-0.004, 0.006, dHead - dBody);
    float spot = smoothstep(0.62, 0.74, fbm(q * vec2(140.0, 220.0)));       // irregular dark bars
    vec3 back = mix(rufous, ink, spot * 0.8) * (0.8 + 0.3 * barb);
    col = mix(col, back, onBody);
    float onTail = smoothstep(-0.003, 0.003, min(dHead, dBody) - dTail) * step(0.25, tsy);
    vec3 tailc = mix(slate * 0.55, ink, 0.7 * smoothstep(0.74, 0.80, tsy));
    col = mix(col, tailc, onTail);
    float onBeak = smoothstep(0.001, -0.001, dBeak);
    col = mix(col, mix(vec3(0.24, 0.27, 0.32), yellow, smoothstep(-0.030, -0.025, q.x)), onBeak);

    // light: the sun is behind, so the near side gets only sky fill and a warm bounce from the field
    vec3 fill = vec3(0.12, 0.135, 0.17) + vec3(0.07, 0.045, 0.02) * smoothstep(0.0, -0.2, q.y);
    vec3 c = col * fill;
    // rim: the silhouette edge facing the sun, a fringe of lit feather tips
    float probe = bodySd(q + SUN * 0.007, tt, G, s, dHead, dBody, dTail, tsy) + ruff;
    probe = min(probe, dBeak + 0.0);
    float rim = smoothstep(-0.0005, 0.0055, probe) * smoothstep(-0.009, -0.0004, d);
    float fringe = 0.5 + 0.9 * vnoise(rot(-0.6) * q * vec2(150.0, 1600.0));
    c += vec3(1.00, 0.70, 0.36) * rim * fringe * 2.2;
    float cov = clamp(0.5 - d / px, 0.0, 1.0);
    vec4 res = vec4(c * cov, cov);

    // eye: large and dark, a yellow orbital ring, the sun caught as a point of light
    float de = length(q - EYE);
    vec3 eyec = vec3(0.035, 0.022, 0.015);
    eyec += vec3(0.30, 0.38, 0.48) * 0.18 * smoothstep(0.008, 0.001, length(q - EYE - vec2(0.002, -0.004)));
    float catchl = smoothstep(0.0020, 0.0010, length(q - EYE - vec2(-0.0026, 0.0027)));
    eyec = mix(eyec, vec3(1.0, 0.93, 0.80), catchl);
    float ring = smoothstep(0.0097, 0.0091, de) * smoothstep(0.0076, 0.0081, de);
    float eyeIn = clamp(0.5 - (de - 0.0078) / px, 0.0, 1.0);
    res.rgb = mix(res.rgb, yellow * 0.32 * cov, ring * cov * 0.8);
    res.rgb = mix(res.rgb, eyec, eyeIn * cov);

    // wings: far wing behind, near wing in front; light comes through the flight feathers
    float fold = smoothstep(0.0, 0.14, s);
    float beat = sin(ph);
    vec2 S = vec2(0.040, -0.050);
    float ang = mix(2.25 + 1.20 * beat, 0.35, fold) - a * 0.0;
    float L = mix(0.26 + 0.02 * cos(ph), 0.17, fold);
    for (int k = 0; k < 2; k++) {
        float u, v;
        float dw = k == 0 ? wingSd(q, S + vec2(0.006, 0.008), ang - 0.20, L * 0.88, u, v)
                          : wingSd(q, S, ang, L, u, v);
        dw += ruff * 0.4;
        float wc = clamp(0.5 - dw / px, 0.0, 1.0);
        if (wc <= 0.0) continue;
        float uc = clamp(u, 0.0, 1.0);
        float prim = smoothstep(0.50, 0.58, uc);
        vec3 wcol = mix(rufous, vec3(0.10, 0.07, 0.05), prim);
        float coverts = smoothstep(0.30, 0.18, length(fract(vec2(uc * 16.0, v * 120.0)) - 0.5)) * (1.0 - prim);
        wcol = mix(wcol, ink, coverts * 0.6);
        // feathers: shafts radiating along the chord
        float fe = abs(fract(uc * 20.0 + v * 6.0) - 0.5);
        float thin = clamp(-v / 0.06, 0.0, 1.0);
        float trans = (0.25 + 0.75 * thin) * (0.6 + 0.4 * smoothstep(0.0, 0.5, fe));
        vec3 through = behind * vec3(1.0, 0.62, 0.30) * (0.55 + 0.6 * prim * 0.0);
        vec3 wl = mix(wcol * fill * (k == 0 ? 0.6 : 1.0), through, trans * 0.55);
        float lead = 0.010 * (1.0 - uc) + 0.002;
        float edge = smoothstep(0.004, 0.0, abs(v - lead + 0.002)) * step(0.0, dot(vec2(-sin(ang), cos(ang)), SUN));
        wl += vec3(1.0, 0.72, 0.38) * edge * (k == 0 ? 0.5 : 1.1);
        float alpha = wc * (0.97 - 0.30 * thin * thin);
        if (k == 0) res = res + vec4(wl * alpha, alpha) * (1.0 - res.a);
        else res = vec4(wl * alpha, alpha) + res * (1.0 - alpha);
    }
    return res;
}

// ---- the world -------------------------------------------------------------------------------
vec3 world(vec2 p, float t, float Z, float G) {
    float soft = smoothstep(1.5, 9.0, Z);                         // closer = shallower focus
    float settle = smoothstep(3.3, 4.3, t);
    float roll = mix(-0.34 + 0.55 * smoothstep(0.0, 1.9, t) - 0.40 * smoothstep(1.9, 3.3, t) + 0.03 * sin(t * 2.7) * G, 0.0, settle);
    float bob = mix(0.15 - 0.30 * smoothstep(0.2, 2.2, t) + 0.15 * smoothstep(2.2, 3.3, t) + 0.012 * sin(t * 4.1), -0.30, settle);
    float bz = 1.0 + (Z - 1.0) * 0.05;
    vec2 w = rot(roll) * p / bz + vec2(0.0, bob);
    float hz = -0.25 + 0.025 * (vnoise(vec2(w.x * 2.5, 3.0)) - 0.5);
    // sky
    float ty = clamp((w.y - hz) / 0.85, 0.0, 1.0);
    vec3 sky = mix(vec3(0.97, 0.80, 0.55), vec3(0.42, 0.53, 0.66), pow(ty, 0.65));
    float sunD = length(w - vec2(-1.25, 0.32));
    sky += vec3(1.0, 0.70, 0.35) * exp(-sunD * 1.7) * 0.85;
    // clouds, racing right on the wind, edges lit from the sun side
    vec2 cw = w * vec2(1.1, 3.2) + vec2(-t * 0.35 * G - 2.0, 0.0);
    float lf = 1.0 - 0.6 * soft;
    float cd = fbm(cw * lf);
    float cl = fbm((cw + vec2(0.06, -0.04)) * lf);
    float cloud = smoothstep(0.46, 0.72, cd) * smoothstep(-0.02, 0.15, w.y - hz);
    vec3 cc = mix(vec3(0.33, 0.37, 0.45), vec3(0.55, 0.52, 0.52), ty);
    cc += vec3(1.0, 0.62, 0.30) * clamp((cd - cl) * 6.0, 0.0, 1.0) * 0.9;
    sky = mix(sky, cc, cloud * 0.85);
    // field: dry grass under cloud shadow, hazy toward the horizon, gust waves running right
    float gy = clamp((hz - w.y) / 0.3, 0.0, 1.0);
    float shadow = smoothstep(0.42, 0.62, fbm(vec2(w.x * 1.2 - t * 0.35 * G, w.y * 4.0)));
    vec3 field = mix(vec3(0.80, 0.62, 0.38), vec3(0.50, 0.40, 0.22), gy);
    float wave = 0.5 + 0.5 * sin(w.x * 9.0 - t * 7.0 * G + fbm(w * 3.0) * 4.0);
    field *= mix(1.0, 0.62, shadow) * (0.88 + 0.22 * wave * gy);
    float blades = vnoise(vec2((w.x + (hz - w.y) * 0.4 * sin(t * 3.0 + w.x * 2.0) * G) * mix(180.0, 40.0, soft), w.y * 6.0));
    field *= 0.85 + 0.3 * blades * gy;
    field += vec3(1.0, 0.68, 0.32) * pow(blades, 6.0) * gy * 0.35;   // backlit seed heads
    field = mix(field, vec3(0.93, 0.78, 0.56), exp(-(hz - w.y) * 14.0) * 0.7);
    vec3 col = mix(field, sky, smoothstep(-0.004, 0.004, w.y - hz));
    return col;
}

// windborne seeds in a depth layer: soft, streaked along the wind, lit by the sun behind them
float seeds(vec2 p, float t, float cellSz, float speed, float size, float G, float seed, float dens) {
    vec2 q = p / cellSz + vec2(-t * speed * G / cellSz, 0.0);
    vec2 c = floor(q);
    float acc = 0.0;
    for (int dx = -1; dx <= 1; dx++) {
        vec2 cc = c + vec2(float(dx), 0.0);
        vec2 r = h22(cc + seed);
        if (r.x > dens) continue;
        vec2 pos = cc + vec2(r.y, 0.5 + 0.35 * sin(t * (2.0 + r.x * 3.0) + r.y * 6.28));
        vec2 d = q - pos;
        d.x /= 1.0 + speed * G * 1.6;                             // motion streak
        acc += smoothstep(size, size * 0.15, length(d));
    }
    return acc;
}

void mainImage(out vec4 o, in vec2 f) {
    vec2 p0 = (f - 0.5 * iResolution.xy) / iResolution.y;
    float t = iTime;
    float push = 1.0 + 0.09 * smoothstep(4.1, 6.0, t);
    vec2 p = E + (p0 - E) / push;
    float G = gust(t);
    float Z = zoomAt(t);
    if (pass > 0.5) {
        // nearest seeds pass in front of everything, defocused
        float n = seeds(p + vec2(0.0, 0.03 * sin(t * 1.3)), t + 3.0, 0.55, 1.9, 0.07, G, 7.0, 0.3);
        o = vec4(vec3(1.0, 0.86, 0.62) * n * 0.12, 1.0);
        return;
    }
    vec3 col = world(p, t, Z, G);
    float far = seeds(p, t, 0.07, 0.55, 0.040, G, 1.0, 0.10);
    float mid = seeds(p + vec2(0.0, 0.2), t, 0.21, 1.1, 0.030, G, 3.0, 0.22);
    col += vec3(1.0, 0.80, 0.52) * (far * 0.16 + mid * 0.30);

    // the bird, motion-blurred over a 180-degree shutter (wings move ~40 degrees per frame)
    float px = 1.0 / (iResolution.y * Z);
    vec2 q = (p - E) / Z + EYE;
    if (t < TS + 0.5 && length(q - vec2(0.05, -0.08)) < 0.42 + max(t - TS, 0.0) * 4.0) {
        vec4 acc = vec4(0.0);
        const int N = 10;
        float jit = h2(f + fract(t * 7.13));
        for (int k = 0; k < N; k++) {
            float tt = t + ((float(k) + jit) / float(N) - 0.5) * (1.0 / 90.0);
            acc += bird(q, col, tt, px, G);
        }
        acc /= float(N);
        col = col * (1.0 - acc.a) + acc.rgb;
    }

    // the eye that stays: after the stoop the iris holds its place, then cools and swells into the full stop
    float s = t - TS;
    if (s > 0.0 && s < 0.98) {
        float grow = smoothstep(0.10, 0.75, s);
        float r = mix(0.0085 * Z, 25.5 / 1080.0, grow * grow * (3.0 - 2.0 * grow));
        float de = length(p - E);
        float ringOn = (1.0 - smoothstep(0.05, 0.45, s));
        float ringR = r * 1.24;
        float cov = clamp(0.5 - (de - r) * iResolution.y, 0.0, 1.0);
        float rc = clamp(0.5 - (de - ringR) * iResolution.y, 0.0, 1.0) * ringOn;
        col = mix(col, vec3(0.93, 0.68, 0.16) * 0.9, rc);
        vec3 dot = mix(vec3(0.02, 0.015, 0.01), vec3(0.090, 0.067, 0.047), grow);
        float catchl = smoothstep(r * 0.30, r * 0.14, length(p - E - vec2(-r * 0.33, r * 0.35))) * (1.0 - smoothstep(0.1, 0.6, s));
        dot = mix(dot, vec3(1.0, 0.93, 0.80), catchl);
        col = mix(col, dot, cov);
    }

    // the wordmark stands in the field: lit from behind by the same sun, its long shadows thrown
    // towards us across the grass, the near grass in front of its feet
    vec2 uvT = (p * iResolution.y + 0.5 * iResolution.xy) / iResolution.xy;
    vec2 tpx = 1.0 / iResolution.xy;
    float yb = E.y - 0.0197;                                         // baseline
    if (p.y < yb) {
        float dd = yb - p.y;
        vec2 src = vec2(p.x + 0.55 * dd, yb + dd * 2.4);
        vec2 suv = (src * iResolution.y + 0.5 * iResolution.xy) / iResolution.xy;
        float bl = 2.0 + dd * 260.0;
        float sh = 0.0;
        for (int k = 0; k < 5; k++) {
            vec2 o2 = vec2(float(k) - 2.0, 0.5 * (float(k) - 2.0)) * bl * tpx;
            sh += texture(iChannel0, suv + o2).a;
        }
        col *= 1.0 - 0.7 * sh / 5.0 * smoothstep(0.40, 0.0, dd);
    }
    float ta = texture(iChannel0, uvT).a;
    if (ta > 0.0) {
        float lit = 1.0 - texture(iChannel0, uvT + SUN * tpx * 2.0).a;
        vec3 inkc = vec3(0.075, 0.057, 0.042) + vec3(0.10, 0.06, 0.03) * smoothstep(yb + 0.05, yb, p.y);
        inkc += vec3(1.0, 0.70, 0.38) * lit * 0.6;
        col = mix(col, inkc, ta);
    }
    // near grass, in front of the letters' feet, swaying with the gusts
    float gx = p.x * 260.0;
    float colm = floor(gx);
    float hb = h1(colm * 1.37);
    float sway = 0.006 * sin(t * 2.4 + colm * 0.07) * G;
    float bladeTop = yb + 0.004 + 0.032 * hb * hb;
    float bx = abs(fract(gx + sway * 260.0 * smoothstep(yb - 0.05, bladeTop, p.y)) - 0.5);
    float blade = step(p.y, bladeTop) * smoothstep(0.32, 0.15, bx) * step(0.55, h1(colm * 7.1)) * smoothstep(yb - 0.08, yb - 0.02, p.y);
    vec3 gcol = vec3(0.28, 0.21, 0.11) + vec3(1.0, 0.68, 0.32) * 0.55 * smoothstep(bladeTop - 0.012, bladeTop, p.y);
    col = mix(col, gcol, blade * 0.9 * smoothstep(3.8, 4.4, t));
    o = vec4(col, 1.0);
}
