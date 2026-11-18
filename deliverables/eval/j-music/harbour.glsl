// Low Tide — a harbour at night, the sea gone out.
// Wet mudflats mirror the harbour lights; the lights go out one by one,
// the stars come through, and the two lanterns on a grounded boat stay on.

uniform float drift;   // = 1.0 camera push-in amount

const float H = 0.42;  // horizon height (uv.y)

float hash1(float n) { return fract(sin(n * 127.13 + 3.7) * 43758.5453); }
float hash2(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2(1.0, 0.0)), f.x),
               mix(hash2(i + vec2(0.0, 1.0)), hash2(i + vec2(1.0, 1.0)), f.x), f.y);
}
float fbm(vec2 p) {
    float s = 0.0, a = 0.5;
    for (int i = 0; i < 4; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
    return s;
}

// Sky colour at height y (uv), with the town's sodium glow and the end's blue hour.
vec3 skyCol(vec2 uv, float t, float townOn) {
    float k = clamp((uv.y - H) / (1.0 - H), 0.0, 1.0);
    vec3 top = vec3(0.004, 0.006, 0.020);
    vec3 mid = vec3(0.014, 0.021, 0.055);
    vec3 hor = vec3(0.060, 0.042, 0.075);
    float dawn = smoothstep(13.5, 20.0, t);
    hor = mix(hor, vec3(0.16, 0.085, 0.09), dawn * 0.6);
    mid = mix(mid, vec3(0.030, 0.042, 0.095), dawn * 0.5);
    vec3 c = mix(hor, mid, smoothstep(0.0, 0.28, k));
    c = mix(c, top, smoothstep(0.25, 1.0, k));
    // sodium light pollution over the town, fading as the lights go
    float glow = exp(-max(uv.y - H, 0.0) / 0.10) * smoothstep(0.18, 0.55, uv.x) * smoothstep(1.15, 0.8, uv.x);
    c += vec3(0.30, 0.13, 0.04) * glow * 0.35 * townOn;
    return c;
}

// Skyline height above the horizon at x (breakwater + lighthouse on the left, town right).
float skyline(float x) {
    float h = 0.0;
    // breakwater
    h = max(h, 0.0045 * step(x, 0.24));
    // lighthouse tower
    h = max(h, (0.052 - 120.0 * (x - 0.115) * (x - 0.115)) * step(abs(x - 0.115), 0.0075));
    // town: blocks
    float cell = floor(x * 70.0);
    float b = hash1(cell);
    float town = smoothstep(0.30, 0.36, x) * smoothstep(1.05, 0.9, x);
    h = max(h, town * (0.006 + 0.022 * b * b + 0.010 * step(0.86, b)));
    // quay
    h = max(h, 0.003 * step(0.28, x));
    // two cranes
    h = max(h, 0.075 * step(abs(x - 0.52), 0.0016));
    h = max(h, 0.068 * step(abs(x - 0.86), 0.0016));
    return h;
}

float skylineMask(vec2 uv) {
    float x = uv.x, y = uv.y - H;
    float h = skyline(x);
    // crane jibs are thin: carve them
    float jib1 = step(0.47, x) * step(x, 0.585) * step(abs(y - 0.072), 0.0014);
    float jib2 = step(0.80, x) * step(x, 0.90) * step(abs(y - 0.066), 0.0014);
    float core = step(y, h);
    return max(core, max(jib1, jib2)) * step(0.0, y);
}

// Fixed lights: lighthouse, harbour-mouth red and green, two crane tops.
void specialLight(int i, float t, out vec2 p, out vec3 c, out float b, out float size) {
    if (i == 0) {             // lighthouse lamp: last to go, on "gone"
        p = vec2(0.115, H + 0.054); c = vec3(1.0, 0.86, 0.62); size = 1.6;
        b = (1.0 - smoothstep(9.25, 9.85, t)) * (0.78 + 0.22 * sin(t * 2.2));
    } else if (i == 1) { p = vec2(0.205, H + 0.012); c = vec3(1.0, 0.16, 0.10); size = 0.9; b = 1.0 - smoothstep(8.75, 9.0, t); }
    else if (i == 2) { p = vec2(0.315, H + 0.010); c = vec3(0.25, 1.0, 0.45); size = 0.9; b = 1.0 - smoothstep(7.95, 8.2, t); }
    else if (i == 3) { p = vec2(0.52, H + 0.077); c = vec3(1.0, 0.25, 0.15); size = 0.7; b = 1.0 - smoothstep(8.05, 8.25, t); }
    else { p = vec2(0.86, H + 0.071); c = vec3(1.0, 0.25, 0.15); size = 0.7; b = 1.0 - smoothstep(7.0, 7.2, t); }
}

// Town lights live one per x cell, so a pixel only visits its neighbours.
const float X0 = 0.33;
const float CW = 0.0125;
const int NC = 52;
void cellLight(int k, float t, out vec2 p, out vec3 c, out float b, out float size) {
    float fk = float(k);
    p = vec2(X0 + (fk + 0.5 + (hash1(fk * 3.1) - 0.5) * 0.7) * CW, H + 0.006 + 0.026 * pow(hash1(fk * 7.7), 2.0));
    float warm = step(0.25, hash1(fk * 1.3));
    c = mix(vec3(0.95, 0.88, 0.80), vec3(1.0, 0.55, 0.22), warm);
    size = 0.55 + 0.6 * hash1(fk * 5.9);
    float r = (p.x - X0) / (CW * float(NC));
    // they go out right to left across "when the harbour lights are gone"
    float off = 6.75 + 2.3 * (1.0 - r) + 0.45 * hash1(fk * 9.3);
    b = 1.0 - smoothstep(off, off + 0.22, t);
    float flick = step(0.45, hash1(floor(t * 24.0) + fk * 13.0));
    b *= mix(1.0, flick, step(off - 0.35, t) * step(t, off));
    b *= step(0.2, hash1(fk * 5.3)) * step(0.0, fk) * step(fk, float(NC) - 1.0);
}

vec2 rot(vec2 p, float a) { float c = cos(a), s = sin(a); return vec2(c * p.x - s * p.y, s * p.x + c * p.y); }

// The grounded boat: returns coverage, writes lantern positions.
const vec2 BOAT = vec2(0.705, 0.285);
const float TILT = -0.13;
float boatMask(vec2 uv, float asp) {
    vec2 p = (uv - BOAT) * vec2(asp, 1.0);
    p = rot(p, -TILT);
    float L = 0.11;
    float u = p.x / L;
    float hull = step(abs(u), 1.0) * step(-0.026 * (1.0 - u * u * u * u) - 0.002, p.y) * step(p.y, 0.004 + 0.010 * max(u, 0.0) * max(u, 0.0));
    float cabin = step(-0.045, p.x) * step(p.x, 0.005) * step(0.0, p.y) * step(p.y, 0.020 - 0.25 * max(p.x - -0.005, 0.0));
    float mast = step(abs(p.x + 0.012), 0.0013) * step(0.0, p.y) * step(p.y, 0.19);
    float boom = step(-0.09, p.x) * step(p.x, -0.012) * step(abs(p.y - 0.035 - 0.05 * (p.x + 0.012)), 0.0011);
    float stay = step(0.0, p.x) * step(p.x, 0.11) * step(abs(p.y - (0.19 - 1.62 * (p.x + 0.012))), 0.0008) * step(0.004, p.y);
    return max(max(hull, cabin), max(mast, max(boom, stay)));
}
vec2 boatPoint(vec2 local, float asp) {
    vec2 w = rot(local, TILT);
    return BOAT + w / vec2(asp, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float t = iTime;
    float asp = iResolution.x / iResolution.y;
    vec2 uv0 = fragCoord / iResolution.xy;
    // slow push-in toward the boat, a breath of lateral drift
    float push = 1.0 + 0.055 * drift * smoothstep(0.0, 20.0, t);
    vec2 uv = (uv0 - vec2(0.64, 0.36)) / push + vec2(0.64, 0.36);
    uv.x += 0.006 * sin(t * 0.21);

    float townOn = 1.0 - smoothstep(6.9, 9.4, t);
    // the boat's two lanterns: they stay on (and swell on "stayed on")
    float swell = 1.0 + 0.55 * smoothstep(16.5, 18.6, t);
    vec2 lan1 = boatPoint(vec2(-0.012, 0.192), asp);
    vec2 lan2 = boatPoint(vec2(-0.028, 0.010), asp);
    vec3 lanC = vec3(1.0, 0.62, 0.30);

    vec3 col;
    float below = 0.0, sBroken = 0.0, sAlongK = 1.0, sWk = 1.0, sAmp = 0.0, sA = 0.0, sB = 0.0, sDy = 0.0;
    float stars = 0.25 + 0.75 * smoothstep(8.0, 12.0, t);

    if (uv.y >= H) {
        col = skyCol(uv, t, townOn);
        // stars: brighter when the lights are gone
        vec2 sg = uv * vec2(asp, 1.0) * 160.0;
        vec2 si = floor(sg);
        float sh = hash2(si);
        vec2 so = vec2(hash2(si + 3.1), hash2(si + 7.7));
        float sd = length(fract(sg) - so);
        float star = step(0.965, sh) * smoothstep(0.16, 0.0, sd) * (0.5 + 0.5 * sin(t * (1.0 + 3.0 * sh) + sh * 40.0));
        col += vec3(0.75, 0.82, 1.0) * star * stars * 0.55 * smoothstep(H + 0.03, H + 0.25, uv.y);
        // silhouette
        float m = skylineMask(uv);
        col = mix(col, vec3(0.008, 0.009, 0.016) + vec3(0.05, 0.025, 0.012) * townOn * 0.4, m);
        // lit windows in the town, going with the lights
        vec2 wg = vec2(uv.x * 420.0, (uv.y - H) * 900.0);
        float win = step(0.9, hash2(floor(wg))) * step(abs(fract(wg.x) - 0.5), 0.3) * step(abs(fract(wg.y) - 0.5), 0.25);
        float wOff = 6.8 + 2.4 * (1.0 - (uv.x - 0.33) / 0.64) + 0.6 * hash2(floor(wg) + 1.3);
        win *= 1.0 - smoothstep(wOff, wOff + 0.2, t);
        col += vec3(1.0, 0.6, 0.28) * win * m * step(uv.y, H + skyline(uv.x) - 0.002) * 0.35 * step(0.33, uv.x);
    } else {
        // ---- the flats ----
        float dy = H - uv.y;                    // distance below horizon
        float z = 0.06 / (dy + 0.004);          // ground depth
        vec2 g = vec2((uv.x - 0.5) * z * 3.0, z);
        // channels of left-behind water, meandering towards us
        float ch = fbm(vec2(g.x * 0.55 + 0.6 * sin(g.y * 0.35), g.y * 0.12));
        float water = smoothstep(0.50, 0.535, ch);
        water = max(water, smoothstep(0.012, 0.004, dy));      // the far sea, a line at the horizon
        // tide still running out: the far water edge creeps
        float wet = smoothstep(0.25, 0.75, fbm(g * vec2(0.6, 0.25) + 4.0));
        // sand ripples
        float rip = sin(g.y * 22.0 + fbm(g * vec2(1.2, 0.8)) * 9.0 + g.x * 1.4);
        // puddles left in the ripple troughs
        float puddle = smoothstep(-0.55, -0.85, rip) * smoothstep(0.35, 0.6, wet);
        water = max(water, puddle * 0.85);
        float crest = smoothstep(0.55, 1.0, rip) * (1.0 - water);
        vec3 sand = vec3(0.0065, 0.0055, 0.0060)
                  + crest * (vec3(0.030, 0.016, 0.008) * townOn * smoothstep(0.25, 0.6, uv.x) + vec3(0.008, 0.010, 0.020)) * exp(-dy * 3.0);
        // mirrored sky, rougher on sand
        vec2 mir = vec2(uv.x, H + dy);
        float fres = 0.25 + 0.75 * exp(-dy * 5.0);
        float refl = mix(0.06 + 0.10 * wet, 0.95, water) * fres;
        vec3 sky = skyCol(mir, t, townOn);
        col = mix(sand, sky, refl);
        // skyline reflected (darker band)
        float sm = skylineMask(vec2(uv.x + 0.002 * sin(g.y * 4.0 + t), mir.y));
        col = mix(col, sand * 0.6, sm * refl);
        // light streaks
        float rough = mix(1.0, 0.25, water);
        float wobA = vnoise(vec2(g.y * 2.6, t * 0.6)) - 0.5;
        float wobB = vnoise(vec2(g.y * 3.3 + 17.0, t * 0.5)) - 0.5;
        float brk = 0.55 + 0.45 * vnoise(vec2(g.y * 6.0, uv.x * 40.0));
        float wobAmp = (0.003 + 0.012 * dy) * (1.0 + 1.5 * rough);
        float broken = mix(brk, 1.0, water);
        float alongK = 1.0 / (0.05 + 0.25 * (1.0 - rough));
        float wk = 0.0018 + 0.010 * dy * rough;
        below = 1.0;
        sBroken = broken * refl; sAlongK = alongK; sWk = wk; sAmp = wobAmp; sA = wobA; sB = wobB; sDy = dy;
        // lantern reflections under the boat
        for (int j = 0; j < 2; j++) {
            vec2 lpp = j == 0 ? lan1 : lan2;
            float contact = BOAT.y - 0.02;
            float my = 2.0 * contact - lpp.y;          // mirrored about the boat's footing
            float dx = (uv.x - lpp.x) * asp + (vnoise(vec2(uv.y * 90.0, t * 0.5 + float(j))) - 0.5) * 0.01;
            float w = 0.006;
            float a = exp(-dx * dx / (w * w)) * exp(-abs(uv.y - my) / 0.05) * step(uv.y, contact);
            col += lanC * a * refl * 0.9 * swell * (j == 0 ? 0.5 : 0.8);
        }
        // mist lying on the flats
        float mist = fbm(vec2(uv.x * 3.0 + t * 0.02, dy * 14.0)) * exp(-dy / 0.07);
        col += vec3(0.035, 0.028, 0.045) * mist * (0.5 + 0.5 * townOn);
    }

    // the lights: glow, and their streaks across the wet flats
    vec3 acc = vec3(0.0);
    int kc = int(floor((uv.x - X0) / CW));
    for (int n = 0; n < 10; n++) {
        vec2 p; vec3 c; float b; float sz;
        if (n < 5) specialLight(n, t, p, c, b, sz);
        else cellLight(kc + n - 7, t, p, c, b, sz);
        if (b < 0.003) continue;
        vec2 d = (uv - p) * vec2(asp, 1.0);
        float r2 = dot(d, d);
        float core = exp(-r2 / (0.0000035 * sz * sz));
        float halo = 0.0000045 * sz / (r2 + 0.000012 * sz);
        acc += c * b * (core * 2.5 + halo * 0.9);
        // reflection
        float dx = d.x + mix(sA, sB, hash1(p.x * 91.0)) * sAmp;
        float w = sWk * sz;
        float st = exp(-dx * dx / (w * w)) * exp(-max(sDy - (p.y - H), 0.0) * sAlongK);
        acc += c * (b * 1.5 * st * sBroken * below);
    }
    col += acc;

    // the boat, settled into the mud
    vec2 cs = (uv - vec2(BOAT.x, BOAT.y - 0.022)) * vec2(asp / 0.13, 1.0 / 0.012);
    col *= 1.0 - 0.75 * exp(-dot(cs, cs));
    float bm = boatMask(uv, asp);
    vec3 boatC = vec3(0.0015, 0.0015, 0.0030);
    // warm rim from the cabin lantern
    float near = exp(-length((uv - lan2) * vec2(asp, 1.0)) / 0.03);
    boatC += lanC * near * 0.035 * swell;
    col = mix(col, boatC, bm);
    // lanterns
    for (int j = 0; j < 2; j++) {
        vec2 lpp = j == 0 ? lan1 : lan2;
        vec2 d = (uv - lpp) * vec2(asp, 1.0);
        float r2 = dot(d, d);
        float k = j == 0 ? 0.8 : 1.15;
        col += lanC * swell * k * (exp(-r2 / 0.0000045) * 2.2 + 0.0000075 / (r2 + 0.00002));
    }
    // horizon haze
    col += vec3(0.06, 0.04, 0.06) * exp(-abs(uv.y - H) / 0.014) * (0.2 + 0.4 * townOn);

    // grade: filmic, a little lift in the shadows towards ink blue
    col = 1.0 - exp(-col * 1.25);
    col = pow(col, vec3(0.4545));
    col = col * 0.98 + vec3(0.006, 0.008, 0.018);
    // vignette
    vec2 v = uv0 - 0.5;
    col *= 1.0 - 0.75 * dot(v, v);
    fragColor = vec4(col, 1.0);
}
