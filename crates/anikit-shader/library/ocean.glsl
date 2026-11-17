// Ocean: open water to the horizon under a low sun. Directional swell plus
// wind chop, intersected with a few secant steps from the plane (no long
// march), analytic normals, Fresnel sky reflection, a sun glitter path,
// light scattering through wave crests and a little foam on the peaks.
// Uniforms: speed, sun (0 high … 1 at the horizon), swell, chop, color, sky.
uniform float speed;  // = 1.0 wave speed
uniform float sun;    // = 0.7 0 high afternoon … 1 sun on the horizon
uniform float swell;  // = 1.0 height of the long waves
uniform float chop;   // = 1.0 roughness of the short waves
uniform vec3 color;   // = vec3(0.01, 0.07, 0.1) deep water colour
uniform vec3 sky;     // = vec3(0.32, 0.45, 0.62) sky colour overhead

float T;

vec3 sunDir() { return normalize(vec3(0.0, mix(0.32, 0.035, sun), -1.0)); }

// Height and slope (dh/dx, dh/dz) of the sea at (x, z). Peaked waves
// (exp of a sine) in golden-angle directions so no grid forms; each wave drags
// the point the next one sees, which leans and sharpens the crests.
vec3 waves(vec2 p, int n) {
    float h = 0.0;
    vec2 d = vec2(0.0);
    float amp = 0.26 * swell, freq = 0.2, ang = 0.4;
    for (int i = 0; i < 9; i++) {
        if (i >= n) break;
        vec2 dir = vec2(cos(ang), sin(ang));
        float x = dot(dir, p) * freq + T * sqrt(9.81 * freq) * 0.55 + float(i) * 1.37;
        float e = exp(sin(x) - 1.0);
        float de = e * cos(x);
        h += amp * e;
        d += amp * de * freq * dir;
        p -= dir * de * amp * 0.8 * chop;
        amp *= 0.58;
        freq *= 1.72;
        ang += 2.39996;
    }
    return vec3(h, d);
}

vec3 skyCol(vec3 rd) {
    vec3 sd = sunDir();
    float y = max(rd.y, 0.0);
    float s = dot(rd, sd);
    vec3 zen = mix(sky * vec3(0.2, 0.32, 0.5), sky * vec3(0.16, 0.14, 0.3), sun);
    vec3 hor = mix(vec3(0.58, 0.64, 0.7), vec3(0.98, 0.46, 0.2), sun);
    vec3 c = mix(hor, zen, smoothstep(0.0, 0.32, y));
    // Sun lobes: exp(k·(cos − 1)) ≈ cosᵏ near the sun, one exp each.
    c += vec3(1.0, 0.42, 0.15) * exp(14.0 * (s - 1.0)) * 0.35 * sun;
    c += vec3(1.0, 0.68, 0.38) * exp(90.0 * (s - 1.0)) * 0.5;
    c += vec3(1.0, 0.9, 0.75) * exp(2500.0 * (s - 1.0)) * 14.0;
    return c;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    T = iTime * speed;
    vec3 ro = vec3(0.0, 2.2, T * 1.2);
    vec3 rd = normalize(vec3(uv.x, uv.y - 0.12, -1.7));
    vec3 col;
    if (rd.y >= -0.002) {
        col = skyCol(rd);
    } else {
        // Secant search between the planes just above and below the waves.
        float top = 0.65 * swell + 0.05, bot = 0.0;
        float t0 = (top - ro.y) / rd.y, t1 = (bot - ro.y) / rd.y;
        float h0 = ro.y + rd.y * t0 - waves((ro + rd * t0).xz, 3).x;
        float h1 = ro.y + rd.y * t1 - waves((ro + rd * t1).xz, 3).x;
        for (int i = 0; i < 3; i++) {
            float tm = t0 + (t1 - t0) * h0 / (h0 - h1);
            float hm = ro.y + rd.y * tm - waves((ro + rd * tm).xz, 3).x;
            if (hm > 0.0) { t0 = tm; h0 = hm; } else { t1 = tm; h1 = hm; }
        }
        float t = t0 + (t1 - t0) * h0 / (h0 - h1);
        vec3 p = ro + rd * t;
        // Fewer octaves with distance: the far sea is too small to resolve them.
        int oct = t < 14.0 ? 9 : (t < 30.0 ? 7 : (t < 100.0 ? 5 : 4));
        vec3 w = waves(p.xz, oct);
        vec3 n = normalize(vec3(-w.y, 1.0, -w.z));
        float fade = exp(-t * 0.004);
        n = normalize(mix(vec3(0.0, 1.0, 0.0), n, fade));
        vec3 r = reflect(rd, n);
        r.y = abs(r.y);
        float fre = 0.02 + 0.98 * pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
        vec3 sd = sunDir();
        // Light through the crests: thinner water glows green-blue facing the sun.
        // Only the thin tops of crests glow, and only looking toward the sun.
        float sss = pow(max(dot(rd, sd), 0.0), 4.0) * smoothstep(0.38, 0.62, w.x / max(swell, 1e-3)) * 0.8;
        vec3 water = color * (0.25 + 0.5 * max(dot(n, sd), 0.0)) + vec3(0.03, 0.2, 0.17) * sss;
        col = mix(water, skyCol(r), fre);
        // The sun's glitter path.
        col += vec3(1.0, 0.8, 0.55) * exp(900.0 * (dot(r, sd) - 1.0)) * 18.0 * (0.4 + sun);
        // Foam where the crests peak.
        float foam = smoothstep(0.5, 0.62, w.x / max(swell, 1e-3)) * fade * chop;
        col = mix(col, vec3(0.85, 0.88, 0.9) * (0.6 + 0.4 * max(dot(n, sd), 0.0)), foam * 0.5);
        // Haze toward the horizon.
        col = mix(col, skyCol(normalize(vec3(rd.x, 0.0, rd.z))), 1.0 - exp(-t * 0.004));
    }
    col = 1.0 - exp(-col * 0.85);
    fragColor = vec4(pow(col, vec3(0.4545)), 1.0);
}
