// Portals at golden hour: a colonnade of thick concrete frames recedes down a
// shallow reflecting pool; a low sun rakes through them, laying long soft
// shadows across the stone, with warm haze toward the light. A slow dolly.
// Raymarched SDF with soft shadows, ambient occlusion, a mirror pool with
// Fresnel, aerial perspective and a filmic tone curve.
// Uniforms: speed (dolly), sun (0 noon-ish … 1 sunset), haze.
uniform float speed; // = 1.0
uniform float sun;   // = 0.75
uniform float haze;  // = 1.0

const float PERIOD = 3.2;    // spacing of the frames along z
const float POOL = 1.35;     // half-width of the pool

float sdBox(vec3 p, vec3 b) {
    vec3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}

// x: distance, y: material (1 stone floor, 2 frames, 3 pool floor)
vec2 map(vec3 p) {
    // Stone deck with the pool cut into it.
    float deck = max(p.y, -sdBox(p - vec3(0.0, 0.0, 0.0), vec3(POOL, 0.25, 1e3)));
    vec2 res = vec2(deck, 1.0);
    float poolFloor = p.y + 0.25;
    if (poolFloor < res.x) res = vec2(poolFloor, 3.0);
    // Frames: two piers and a lintel, repeated along z.
    vec3 q = p;
    float cell = floor(q.z / PERIOD + 0.5);
    q.z -= PERIOD * cell;
    float w = 2.25, h = 3.4, t = 0.38, depth = 0.55;
    float piers = sdBox(vec3(abs(q.x) - w, q.y - h * 0.5, q.z), vec3(t, h * 0.5, depth));
    float lintel = sdBox(vec3(q.x, q.y - h, q.z), vec3(w + t, t * 0.9, depth));
    float frame = min(piers, lintel);
    // A chamfer groove on every frame face reads at grazing light.
    frame = max(frame, -sdBox(vec3(q.x, q.y - h, q.z - depth), vec3(w + t + 0.1, 0.035, 0.03)));
    if (frame < res.x) res = vec2(frame, 2.0);
    return res;
}

vec3 calcNormal(vec3 p) {
    const vec2 e = vec2(1.0, -1.0) * 0.0006;
    return normalize(e.xyy * map(p + e.xyy).x + e.yyx * map(p + e.yyx).x + e.yxy * map(p + e.yxy).x + e.xxx * map(p + e.xxx).x);
}

float softShadow(vec3 ro, vec3 rd) {
    float res = 1.0, t = 0.03;
    for (int i = 0; i < 56; i++) {
        float h = map(ro + rd * t).x;
        res = min(res, 9.0 * h / t);
        t += clamp(h, 0.03, 0.4);
        if (res < 0.002 || t > 30.0) break;
    }
    res = clamp(res, 0.0, 1.0);
    return res * res * (3.0 - 2.0 * res);
}

float ao(vec3 p, vec3 n) {
    float occ = 0.0, sca = 1.0;
    for (int i = 0; i < 5; i++) {
        float hr = 0.03 + 0.14 * float(i);
        occ += (hr - map(p + n * hr).x) * sca;
        sca *= 0.8;
    }
    return clamp(1.0 - 1.6 * occ, 0.0, 1.0);
}

vec3 sunDir() {
    // Low, ahead and to the left: the frames are backlit, and each throws a
    // long shadow band back toward the camera.
    float el = mix(0.45, 0.13, sun);
    return normalize(vec3(-0.42, el, -1.0));
}

vec3 sky(vec3 rd) {
    vec3 sd = sunDir();
    float s = max(dot(rd, sd), 0.0);
    vec3 top = mix(vec3(0.22, 0.38, 0.68), vec3(0.14, 0.2, 0.42), sun);
    vec3 hor = mix(vec3(0.62, 0.6, 0.58), vec3(0.8, 0.42, 0.22), sun);
    vec3 c = mix(hor, top, pow(clamp(rd.y * 1.6, 0.0, 1.0), 0.55));
    c += vec3(1.0, 0.62, 0.3) * (pow(s, 6.0) * 0.9 + pow(s, 40.0) * 1.5) * sun;
    c += vec3(1.0, 0.9, 0.7) * pow(s, 900.0) * 12.0;              // the disc
    // Thin high cirrus, lit warm toward the sun.
    if (rd.y > 0.0) {
        vec2 cp = rd.xz / (rd.y + 0.08) * vec2(0.35, 1.6);
        float cl = vnoise(cp * 1.3 + vec2(iTime * 0.01, 0.0)) * 0.6 + vnoise(cp * 3.7) * 0.3 + vnoise(cp * 9.1) * 0.1;
        cl = smoothstep(0.52, 0.85, cl) * smoothstep(0.0, 0.12, rd.y) * smoothstep(0.7, 0.2, rd.y);
        c = mix(c, mix(vec3(0.85, 0.82, 0.8), vec3(1.0, 0.7, 0.45), sun) * (0.75 + 0.6 * pow(s, 4.0)), cl * 0.35);
    }
    return c;
}

vec3 shade(vec3 ro, vec3 rd, float t, float m) {
    vec3 p = ro + rd * t;
    vec3 n = calcNormal(p);
    vec3 sd = sunDir();
    if (m < 1.5 && n.y > 0.9) {
        // Wind-rippled sand beyond the deck; fine wear on the deck itself.
        float out_ = smoothstep(2.9, 3.4, abs(p.x));
        float hx = vnoise(vec2(p.x * 1.1 + p.z * 0.4, p.z * 3.5)) - vnoise(vec2(p.x * 1.1 + p.z * 0.4 + 0.05, p.z * 3.5));
        float hz = vnoise(vec2(p.x * 1.1 + p.z * 0.4, p.z * 3.5)) - vnoise(vec2(p.x * 1.1 + p.z * 0.4, p.z * 3.5 + 0.05));
        n = normalize(n + vec3(hx, 0.0, hz) * mix(0.6, 9.0, out_));
    }
    vec3 sunCol = mix(vec3(1.25, 1.15, 1.0), vec3(1.75, 0.98, 0.5), sun) * 2.6;
    vec3 skyCol = mix(vec3(0.3, 0.4, 0.6), vec3(0.22, 0.3, 0.55), sun);
    // Concrete: warm grey with board-form streaks and mottling.
    float mott = vnoise(p.xz * 3.0 + p.y * 1.7) * 0.5 + vnoise(p.xy * 9.0 + p.z * 4.0) * 0.25 + vnoise(vec2(p.y * 40.0, p.x * 2.0 + p.z * 2.0)) * 0.12;
    vec3 alb = vec3(0.56, 0.52, 0.47) * (0.82 + 0.3 * mott);
    if (m < 1.5) alb = mix(vec3(0.62, 0.57, 0.5) * (0.85 + 0.25 * vnoise(p.xz * 2.3)), vec3(0.7, 0.52, 0.36) * (0.9 + 0.2 * vnoise(p.xz * 0.7)), smoothstep(2.9, 3.4, abs(p.x)));   // limestone deck, then sand
    if (m > 2.5) alb = vec3(0.10, 0.16, 0.17);                                           // pool floor
    float dif = max(dot(n, sd), 0.0) * softShadow(p + n * 0.004, sd);
    float occ = ao(p, n);
    float bounce = clamp(0.5 - 0.5 * n.y, 0.0, 1.0);
    vec3 col = alb * (dif * sunCol + occ * (0.09 + 0.1 * n.y) * skyCol + occ * bounce * vec3(0.6, 0.34, 0.16) * 0.35);
    // A faint sheen on the stone at grazing sun.
    vec3 hv = normalize(sd - rd);
    col += sunCol * 0.04 * pow(max(dot(n, hv), 0.0), 24.0) * dif;
    return col;
}

vec3 aerial(vec3 col, vec3 rd, float t) {
    vec3 sd = sunDir();
    float s = max(dot(rd, sd), 0.0);
    vec3 fogCol = mix(vec3(0.5, 0.52, 0.56), vec3(0.75, 0.48, 0.3), sun) * (0.35 + 1.4 * pow(s, 8.0));
    float f = 1.0 - exp(-t * 0.011 * haze);
    return mix(col, fogCol, f);
}

vec2 march(vec3 ro, vec3 rd, float tmax) {
    float t = 0.02;
    for (int i = 0; i < 140; i++) {
        vec2 h = map(ro + rd * t);
        if (abs(h.x) < 0.0003 * t) return vec2(t, h.y);
        t += h.x;
        if (t > tmax) break;
        // Nothing stands above the lintels (y 3.74): a rising ray past them is sky.
        if (rd.y > 0.0 && ro.y + rd.y * t > 3.8) break;
    }
    return vec2(-1.0, 0.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    float t = iTime * speed;
    // Low camera, slightly off-axis, dollying toward the light.
    vec3 ro = vec3(0.6, 0.95, 4.8 - t * 0.55);
    vec3 ta = ro + vec3(-0.22, 0.1, -1.0);
    vec3 ww = normalize(ta - ro), uu = normalize(cross(ww, vec3(0, 1, 0))), vv = cross(uu, ww);
    vec3 rd = normalize(uv.x * uu + uv.y * vv + 1.6 * ww);

    vec3 col;
    // The pool's water surface sits just below the deck. Rays that reach it
    // need only march as far as the surface: anything beyond is under water.
    float tw = rd.y < 0.0 ? (-0.06 - ro.y) / rd.y : 1e9;
    vec3 pw = ro + rd * tw;
    bool pool = tw > 0.0 && abs(pw.x) < POOL;
    vec2 h = march(ro, rd, pool ? min(tw, 60.0) : 60.0);
    bool water = pool && (h.x < 0.0 || tw < h.x);
    if (water) {
        // Mirror with Fresnel, a whisper of ripple, and the dark pool beneath.
        vec3 n = normalize(vec3(0.004 * sin(pw.z * 9.0 + t * 1.3) + 0.003 * sin(pw.x * 13.0 - t), 1.0, 0.003 * sin(pw.z * 17.0 - t * 2.1)));
        vec3 rr = reflect(rd, n);
        float fre = 0.02 + 0.98 * pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
        vec2 hr = march(pw + n * 0.01, rr, 60.0);
        vec3 refl = hr.x > 0.0 ? aerial(shade(pw + n * 0.01, rr, hr.x, hr.y), rr, hr.x) : sky(rr);
        vec3 under = vec3(0.03, 0.06, 0.065);
        col = mix(under, refl, clamp(fre * 1.15 + 0.3, 0.0, 1.0));
        col = aerial(col, rd, tw);
    } else if (h.x > 0.0) {
        col = aerial(shade(ro, rd, h.x, h.y), rd, h.x);
    } else {
        col = sky(rd);
    }
    // Filmic tone curve (ACES fit), gentle vignette, gamma.
    col *= 0.9;
    col = (col * (2.51 * col + 0.03)) / (col * (2.43 * col + 0.59) + 0.14);
    col *= 1.0 - 0.22 * dot(uv * 0.55, uv * 0.55);
    fragColor = vec4(pow(clamp(col, 0.0, 1.0), vec3(0.4545)), 1.0);
}
