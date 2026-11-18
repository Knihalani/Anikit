// Arctic September sea-ice minimum, 1979-2026 (NSIDC Sea Ice Index v4, N_09_extent_v4.0.csv)
// Ice area on screen is proportional to extent; every past year leaves a contour.
uniform float glow;  // = 0.0

const float M[48] = float[48](0.1048,0.0176,0.0918,0.0690,0.0563,0.1400,0.1565,0.0535,0.0718,0.0591,0.1106,0.2438,0.1917,0.0452,0.2025,0.0918,0.2535,0.0300,0.1580,0.1809,0.2470,0.2262,0.1520,0.2949,0.2470,0.2699,0.3518,0.2899,0.5958,0.5056,0.3951,0.4694,0.5326,0.7712,0.4043,0.4025,0.5200,0.5389,0.4793,0.4853,0.5757,0.6590,0.4537,0.4635,0.5713,0.5779,0.4934,0.4813);
float retreat(int k) { return M[clamp(k, 0, 47)]; }

const float E[48] = float[48](7.05,7.67,7.14,7.30,7.39,6.81,6.70,7.41,7.28,7.37,7.01,6.14,6.47,7.47,6.40,7.14,6.08,7.58,6.69,6.54,6.12,6.25,6.73,5.83,6.12,5.98,5.50,5.86,4.27,4.69,5.26,4.87,4.56,3.57,5.21,5.22,4.62,4.53,4.82,4.79,4.36,4.00,4.95,4.90,4.38,4.35,4.75,4.81);
float ext(int k) { return E[clamp(k, 0, 47)]; }

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) {
    float s = 0.0, a = 0.5;
    for (int i = 0; i < 4; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
    return s;
}

// radius of the ice edge at angle a for retreat m (solved so drawn area is proportional to extent)
float edge(float a, float m) {
    float shape = 1.0 + 0.10 * sin(2.0 * a + 0.7) + 0.07 * sin(3.0 * a + 2.1) + 0.045 * sin(5.0 * a + 1.3) + 0.025 * sin(9.0 * a + 0.4) + 0.012 * sin(17.0 * a + 2.2);
    float w = 0.12 + 0.88 * pow(0.5 + 0.5 * cos(a - 2.3), 1.6);
    return 0.37 * shape * (1.0 - m * w);
}

// crack network: distance to the nearest cell border
float cracks(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    float d1 = 8.0, d2 = 8.0;
    for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
        vec2 g = vec2(float(x), float(y));
        vec2 o = vec2(hash(i + g), hash(i + g + 17.3));
        float d = length(g + o - f);
        if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) { d2 = d; }
    }
    return d2 - d1;
}

// 1979-2006 one year per eighth note, 2007-2026 per dotted eighth, 2012 held one beat (84 bpm)
float yearAt(float t) {
    float B = 60.0 / 84.0;
    float t28 = 4.0 * B + 14.0 * B;
    if (t < t28) return clamp((t - 4.0 * B) / (0.5 * B), 0.0, 28.0);
    float t33 = t28 + 5.0 * 0.75 * B;
    if (t < t33) return 28.0 + (t - t28) / (0.75 * B);
    if (t < t33 + B) return 33.0;
    return min(33.0 + (t - t33 - B) / (0.75 * B), 47.0);
}

void mainImage(out vec4 o, in vec2 f) {
    vec2 c = vec2(0.62 * iResolution.x, 0.5 * iResolution.y);
    vec2 p = (f - c) / iResolution.y;
    float px = 1.0 / iResolution.y;
    float r = length(p), a = atan(p.y, p.x);
    float SH = 0.37 * (1.0 + 0.10 * sin(2.0 * a + 0.7) + 0.07 * sin(3.0 * a + 2.1) + 0.045 * sin(5.0 * a + 1.3) + 0.025 * sin(9.0 * a + 0.4) + 0.012 * sin(17.0 * a + 2.2));
    float WW = 0.12 + 0.88 * pow(0.5 + 0.5 * cos(a - 2.3), 1.6);

    float Y = yearAt(iTime);
    int k = int(floor(Y));
    float fr = Y - float(k);
    float ee = clamp(fr / 0.55, 0.0, 1.0);
    ee = 1.0 - (1.0 - ee) * (1.0 - ee) * (1.0 - ee);
    float e = mix(retreat(k), retreat(k + 1), ee);

    // ocean
    float n = vnoise(p * 5.0 + vec2(0.0, iTime * 0.02));
    vec3 col = mix(vec3(0.016, 0.040, 0.066), vec3(0.030, 0.070, 0.105), smoothstep(0.75, 0.0, r)) ;
    col += 0.018 * (n - 0.5);
    // graticule: latitude rings and meridians
    float lat = abs(fract(r / 0.12 + 0.5) - 0.5) * 0.12;
    float g = (1.0 - smoothstep(0.4 * px, 1.4 * px, lat)) * step(0.06, r) * smoothstep(0.62, 0.40, r);
    float mer = abs(fract(a / 0.5236 + 0.5) - 0.5) * 0.5236 * r;
    g += (1.0 - smoothstep(0.4 * px, 1.2 * px, mer)) * smoothstep(0.03, 0.08, r) * smoothstep(0.62, 0.40, r) * 0.7;
    col += vec3(0.20, 0.32, 0.40) * 0.22 * g;

    // surrounding land, stylised coastlines of the Arctic rim
    if (r > 0.36) {
    float landN = fbm(vec2(cos(a), sin(a)) * 1.7 + vec2(3.1, 1.3));
    float coast = 0.50 + 0.10 * (landN - 0.5) * 2.0 + 0.025 * (fbm(p * 18.0) - 0.5) + 0.06 * sin(3.0 * a + 1.0);
    float landD = coast - r;
    float land = 1.0 - smoothstep(-0.8 * px, 0.8 * px, landD);
    float hs = fbm(p * 22.0) - fbm(p * 22.0 + vec2(0.04, 0.04));
    vec3 landCol = vec3(0.105, 0.125, 0.135) + 0.035 * (fbm(p * 8.0) - 0.5) + 0.25 * hs;
    landCol *= smoothstep(0.95, 0.55, r) * 0.6 + 0.4;
    col = mix(col, landCol, land);
    col += vec3(0.30, 0.42, 0.50) * 0.35 * (1.0 - smoothstep(0.4 * px, 1.4 * px, abs(landD)));
    }
    // lost ice: hatch between the 1979 edge and today's
    float r79 = SH * (1.0 - retreat(0) * WW);

    // past contours (one per year already passed)
    float rec = 0.0;
    vec3 rings = vec3(0.0);
    for (int i = 0; i < 48; i++) {
        if (float(i) > Y) break;
        float d = abs(r - SH * (1.0 - retreat(i) * WW));
        float w = 1.0 - smoothstep(0.5 * px, 1.6 * px, d);
        float age = float(i) / 47.0;
        float on = clamp((Y - float(i)) * 2.0, 0.0, 1.0);
        vec3 rc = mix(vec3(0.80, 0.90, 0.95), vec3(0.25, 0.55, 0.70), age);
        float alpha = mix(0.55, 0.35, age);
        if (i == 0) alpha = 0.95;
        if (i == 33) { rc = vec3(1.0, 0.42, 0.29); alpha = 1.0; }
        rings = max(rings, rc * w * alpha * on);
    }
    col = max(col, rings + col * (1.0 - max(rings.r, max(rings.g, rings.b))));

    // the ice itself
    float R = SH * (1.0 - e * WW);
    float lost = step(R, r) * step(r, r79) * clamp((Y - 0.5) * 0.5, 0.0, 1.0);
    float hatch = 1.0 - smoothstep(0.6, 1.4, abs(mod(f.x + f.y, 9.0) - 4.5));
    col += vec3(0.25, 0.45, 0.58) * 0.10 * hatch * lost;
    float pay = smoothstep(23.9, 25.4, iTime);
    col = mix(col, vec3(0.86, 0.36, 0.24), lost * pay * (0.30 + 0.25 * hatch));
    float jag = 0.012 * (fbm(vec2(a * 9.0, r * 30.0)) - 0.5) + 0.006 * (vnoise(p * 120.0) - 0.5);
    float d = r - R + jag;
    float body = 1.0 - smoothstep(-0.6 * px, 0.8 * px, d);
    // broken floes just beyond the edge
    float floeN = vnoise(p * 70.0) * 0.6 + vnoise(p * 160.0) * 0.4;
    float floes = step(0.0, d) * (1.0 - smoothstep(0.0, 0.035, d)) * smoothstep(0.62 + 9.0 * d, 0.70 + 9.0 * d, floeN);
    float ice = max(body, floes * 0.85);
    vec3 iceCol = vec3(0.6, 0.78, 0.88);
    if (d < 0.04) {
    // surface: snow-dusted floes split by leads
    float tex = fbm(p * 9.0);
    iceCol = mix(vec3(0.80, 0.87, 0.92), vec3(0.96, 0.975, 0.985), smoothstep(0.3, 0.7, tex));
    float ck = cracks(p * 40.0 + 0.3 * vec2(vnoise(p * 20.0), vnoise(p * 20.0 + 5.0)));
    float lead = 1.0 - smoothstep(0.015, 0.05, ck);
    iceCol = mix(iceCol, vec3(0.62, 0.75, 0.83), lead * 0.30);
    float ck2 = cracks(p * 70.0 + 3.0);
    iceCol -= 0.05 * (1.0 - smoothstep(0.02, 0.06, ck2));
    }
    // thinner, bluer ice toward the edge
    iceCol = mix(iceCol, vec3(0.60, 0.78, 0.88), smoothstep(-0.07, 0.0, d) * 0.6);
    col = mix(col, iceCol, ice);
    // the 1979 edge and the 2012 record low stay on top
    float d79 = abs(r - r79);
    col = mix(col, vec3(0.92, 0.96, 0.98), (1.0 - smoothstep(0.6 * px, 1.8 * px, d79)) * 0.9);
    float d12 = abs(r - SH * (1.0 - retreat(33) * WW));
    col = mix(col, vec3(1.0, 0.42, 0.29), (1.0 - smoothstep(0.8 * px, 2.2 * px, d12)) * clamp((Y - 33.0) * 2.0, 0.0, 1.0));
    // pole marker
    col = mix(col, vec3(0.05, 0.10, 0.14), (1.0 - smoothstep(2.0 * px, 3.5 * px, r)) * 0.9);

    // fade in / vignette
    col *= smoothstep(0.0, 1.6, iTime);
    vec2 q = f / iResolution.xy - 0.5;
    col *= 1.0 - 0.35 * dot(q, q);
    o = vec4(col, 1.0);
}
