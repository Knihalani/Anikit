// Tern launch — "the land is the recording".
// An estuary at dawn: four ridges over still water. The nearest ridge's top
// edge is the recording line (vermilion ink); sounds raise it. Its mirror in
// the water makes the land read as a waveform. One key light: a low sun.
uniform float zoom;   // = 1.0   world height visible (horizon at y = 0)
uniform float camX;   // = 0.0
uniform float camY;   // = 0.15
uniform float sun;    // = 0.3   dawn progress 0 (blue hour) .. 1 (gold)
uniform float ev;     // = 0.0   event loudness (height of the sound on the near ridge)
uniform float evx;    // = 0.0   event centre x (world)
uniform float evw;    // = 0.4   event width (world)
uniform float evf;    // = 40.0  event detail frequency
uniform float hills;  // = 1.0   overall relief of the land (0 = silent, flat)
uniform float drift;  // = 0.05  pan speed (world/s)
uniform float drop;   // = 0.0   ripple strength on the water (a drip)
uniform float dropx;  // = 0.0
uniform float dropt;  // = 0.0   time since drip
uniform float line;   // = 1.0   vermilion line visibility
uniform float lineEnd;// = 99.0  the line is drawn only for x < lineEnd (world)
uniform float part;   // = 0.0   0 all, 1 back (sky, far ridges, water), 2 near ridge + its water only

float h1(float n) { return fract(sin(n * 127.1) * 43758.5453); }
float vn(float x) { float i = floor(x); float f = fract(x); f = f * f * (3.0 - 2.0 * f); return mix(h1(i), h1(i + 1.0), f) * 2.0 - 1.0; }
float fb(float x) { float a = 0.5; float s = 0.0; for (int k = 0; k < 4; k++) { s += a * vn(x); x = x * 2.03 + 7.1; a *= 0.5; } return s; }
float hh(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }

// ridge height of range k (0 far .. 3 near) at world x
float ridge(int k, float x) {
    float fk = float(k);
    float base = 0.30 - 0.075 * fk;
    float fr = 1.4 + 0.9 * fk;
    // the land only wakes behind the travelling line (lineEnd)
    float wk = 0.06 + 0.94 * smoothstep(lineEnd + 0.02, lineEnd - 0.6, x);
    float amp = (0.10 + 0.03 * fk) * hills * wk;
    float px = x + drift * iTime * (0.3 + 0.5 * fk) + fk * 13.7;
    float y = base * mix(0.25, 1.0, hills * wk) + amp * (0.5 + fb(px * fr));
    if (k == 3) {
        float d = (x - evx) / evw;
        float env = exp(-d * d);
        // transients: sharp, irregular peaks, like a waveform's envelope
        float spikes = 0.45 + 0.55 * pow(abs(sin(x * evf + 2.0 * fb(x * 6.0 + 3.0))), 0.6) * (0.6 + 0.4 * vn(x * evf * 0.37));
        y += ev * env * spikes;
        y = max(y, 0.0);
    }
    return y;
}

vec2 sunPos() { return vec2(camX + 0.50 * zoom, max(0.16, camY - 0.22 * zoom) + sun * 0.05 * zoom); }

vec3 skyCol(vec2 p, float s, float disc) {
    vec3 top = mix(vec3(0.075, 0.080, 0.110), vec3(0.150, 0.150, 0.190), s);
    vec3 hor = mix(vec3(0.330, 0.300, 0.320), vec3(0.900, 0.620, 0.400), s);
    // screen-space gradient: the same dusk in every shot, whatever the camera
    float sy = (p.y - camY) / zoom + 0.5;
    float t = clamp(sy * 1.05, 0.0, 1.0);
    vec3 c = mix(hor, top, pow(t, 0.7));
    // the sun: the key light's source, low and right of centre
    vec2 sp = sunPos();
    float dsun = length((p - sp) * vec2(0.5, 1.0)) / zoom;
    c += vec3(1.0, 0.60, 0.30) * (0.30 * s + 0.12) * exp(-dsun * 3.0);
    c = mix(c, vec3(1.0, 0.88, 0.70), smoothstep(0.022, 0.018, length(p - sp) / zoom) * disc);
    return c;
}

// colour of land at world point p in range k
vec3 landCol(int k, vec2 p, float s, float y0) {
    float fk = float(k);
    // faces: gullies run diagonally down from the ridge, so sample the ridge's
    // slope along a line that leans with depth (no vertical columns)
    float dd = max(y0 - p.y, 0.0);
    float xs = p.x + dd * (0.55 + 0.25 * vn(p.x * 7.0 + fk * 3.0));
    float slope = 0.0;
    if (k >= 2) {
        float e = 0.012 + dd * 0.08;
        slope = (ridge(k, xs + e) - ridge(k, xs - e)) / (2.0 * e);
        slope = mix(slope, 0.0, smoothstep(0.0, 0.35, dd));
    }
    // key light from the right: faces whose slope falls to the right catch it
    float key = clamp(-slope * 0.7 + 0.3 + 0.12 * vn(xs * 22.0 + p.y * 9.0), 0.0, 1.0);
    float sx = sunPos().x;
    float fall = exp(-abs(p.x - sx) * 0.9 / max(zoom, 0.3));
    vec3 shadow = mix(vec3(0.060, 0.075, 0.105), vec3(0.090, 0.085, 0.100), s);
    vec3 lit = mix(vec3(0.200, 0.215, 0.260), vec3(0.740, 0.460, 0.270), s);
    float depthHaze = (3.0 - fk) / 3.0;
    vec3 c = mix(shadow, lit, key * fall * (0.35 + 0.65 * smoothstep(0.08, 0.0, y0 - p.y)));
    // atmospheric perspective: far ranges take the sky
    vec3 haze = skyCol(vec2(p.x, y0 * 0.6), s, 0.0);
    c = mix(c, haze, depthHaze * 0.72);
    // mist pooled at the foot
    float mist = smoothstep(0.10, 0.0, p.y) * (0.25 + 0.15 * vn(p.x * 3.0 + iTime * 0.2));
    c = mix(c, haze * 1.05, mist);
    // grain locked to the land (boiled every 3 frames)
    float g = hh(floor(p * 900.0 / zoom) + floor(iTime * 8.0)) - 0.5;
    c += g * 0.035;
    return c;
}

void mainImage(out vec4 o, in vec2 f) {
    vec2 uv = (f - 0.5 * iResolution.xy) / iResolution.y;
    vec2 p = uv * zoom + vec2(camX, camY);
    float px = zoom / iResolution.y;
    float s = sun;
    vec3 c = vec3(0.0);
    float a = 1.0;
    bool water = p.y < 0.0;
    vec2 q = p;
    float dripGlint = 0.0;
    if (water) {
        // mirror with ripples; a drip sends rings out from dropx
        float r = length(vec2((p.x - dropx) * 0.6, p.y * 3.5));
        float ring = drop * sin(r * 90.0 - dropt * 14.0) * exp(-abs(r - dropt * 0.16) * 18.0) * smoothstep(0.0, 0.1, dropt);
        float wob = 0.004 * vn(p.x * 30.0 + iTime * 1.5) * smoothstep(0.0, -0.05, p.y);
        dripGlint = max(ring, 0.0) * smoothstep(-0.25, 0.0, p.y);
        q = vec2(p.x + ring * 0.02 + wob, -p.y + ring * 0.03 * (1.0 + 4.0 * smoothstep(0.0, -0.2, p.y)));
    }
    float y3 = ridge(3, q.x);
    if (part > 1.5 && q.y > y3 + 6.0 * px) { o = vec4(0.0); return; }
    // which range covers q?
    int hit = -1;
    float hk = 0.0;
    if (q.y < y3) { hit = 3; hk = y3; }
    else if (part < 1.5) {
        float y2 = ridge(2, q.x);
        if (q.y < y2) { hit = 2; hk = y2; }
        else {
            float y1 = ridge(1, q.x);
            if (q.y < y1) { hit = 1; hk = y1; }
            else {
                float y0 = ridge(0, q.x);
                if (q.y < y0) { hit = 0; hk = y0; }
            }
        }
    }
    if (part > 1.5 && hit != 3) { a = 0.0; }
    if (hit < 0) c = skyCol(q, s, 1.0);
    else c = landCol(hit, q, s, hk);

    // the vermilion line: top edge of the near ridge
    float e = 0.002;
    float d3 = (ridge(3, q.x + e) - y3) / e;
    float dist = abs(q.y - y3) / sqrt(1.0 + d3 * d3);
    float wpx = (water ? 1.6 : 2.1) * px;
    float on = smoothstep(wpx, wpx * 0.35, dist) * line * smoothstep(lineEnd, lineEnd - 0.01, q.x);
    vec3 ink = vec3(1.0, 0.33, 0.14);
    if (water) on *= 0.75;
    if (part > 1.5 && on > 0.0) a = max(a, on);
    c = mix(c, ink, on);

    if (water) {
        // the water darkens with depth and takes a little sky sheen
        float dep = smoothstep(0.0, -0.5, p.y);
        c *= mix(0.78, 0.45, dep);
        c += vec3(0.9, 0.55, 0.3) * s * 0.06 * exp(-abs(p.x - sunPos().x) * 3.0 / zoom) * (1.0 - dep);
        c += vec3(0.95, 0.85, 0.72) * dripGlint * 0.35;
        float shore = smoothstep(px * 2.5, 0.0, abs(p.y));
        c = mix(c, c * 1.3 + 0.02, shore * 0.5);
    }
    // lens: slight vignette toward the unlit left
    float v = 1.0 - 0.35 * pow(length(uv * vec2(0.8, 1.1)), 2.2) - 0.08 * smoothstep(0.2, -0.9, uv.x);
    c *= v;
    if (part > 0.5 && part < 1.5) a = 1.0;
    o = vec4(max(c, 0.0) * a, a);
}
