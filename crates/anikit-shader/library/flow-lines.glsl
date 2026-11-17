// Flow lines: curves grow from scattered seeds along a smooth, swirling flow
// field and draw themselves across the paper in a weighted palette (one
// dominant ink, one support, one rare accent), like a generative drawing being
// made. Each pixel traces the flow backwards to find the stroke it lies on, so
// any moment can be rendered directly (no simulation to replay).
// anikit:aa off (strokes are drawn with their own one-pixel soft edge)
// Uniforms: density, width, speed, scale, length, color1, color2, accent, paper.
uniform float density; // = 1.0 how many strokes
uniform float width;   // = 2.2 stroke half-width in pixels at 1080p
uniform float speed;   // = 1.0 how fast strokes grow
uniform float scale;   // = 1.0 size of the flow's swirls
uniform float length;  // = 1.0 longest stroke (1 ≈ a third of the frame)
uniform vec3 color1;   // = vec3(0.1, 0.12, 0.2) dominant ink (about 60%)
uniform vec3 color2;   // = vec3(0.86, 0.33, 0.22) support ink (about 30%)
uniform vec3 accent;   // = vec3(0.95, 0.77, 0.3) rare accent (about 10%)
uniform vec3 paper;    // = vec3(0.95, 0.93, 0.88) paper colour

// Parabolic sine: smooth (C1), within 6% of sin, a few multiplies instead of a
// full-precision sine; plenty for a flow field traced 60 times per pixel.
float qsin(float a) { float t = fract(a * 0.15915494) * 2.0 - 1.0; return -4.0 * t * (1.0 - abs(t)); }
float qcos(float a) { return qsin(a + 1.5707963); }

// The flow direction at p (pixels): analytic and smooth, so tracing is cheap.
vec2 flowDir(vec2 p, float s) {
    vec2 q = p / (300.0 * s * scale);
    float a = 1.9 * qsin(q.x * 1.3 + 1.4 * qsin(q.y * 0.9 + 0.3)) + 1.3 * qcos(q.y * 1.1 - q.x * 0.5) + 0.6 * qsin((q.x + q.y) * 2.3);
    return normalize(vec2(qcos(a), qsin(a)));
}

float segDist(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0));
}


void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float s = iResolution.y / 1080.0;
    float stepLen = 16.0 * s;
    float cell = 70.0 * s / sqrt(max(density, 0.05));
    float w = width * s;
    // How far strokes have grown (in steps) by now.
    float grown = iTime * speed * 3.75;
    float maxSteps = min(22.0 * length, grown);
    vec3 col = paper;
    vec2 p = fragCoord;
    // The nearest approach to a seed over the hit segment and the one after it
    // (taking only the first would notch the stroke at every step boundary).
    float best = 1e9, bestTaper = 1.0;
    vec3 bestInk = paper;
    int found = -1;
    for (int k = 0; k < 22; k++) {
        float fk = float(k);
        if (fk > maxSteps || (found >= 0 && k > found + 1)) break;
        // One midpoint (RK2) step upstream: stays on the curve.
        vec2 mid = p - flowDir(p, s) * stepLen * 0.5;
        vec2 q = p - flowDir(mid, s) * stepLen;
        // The seed of the cell this step passes through (one per cell, ~45% of cells).
        vec2 id = floor(mid / cell);
        vec2 seed = (id + 0.15 + 0.7 * ak_hash2(id + 7.1)) * cell;
        float has = step(ak_hash(id * 1.31 + 2.7), 0.45);
        // Each stroke has its own length (a fraction of the longest).
        float len = (0.35 + 0.65 * ak_hash(id + 19.7)) * 22.0 * length;
        float d = segDist(seed, p, q);
        if (has > 0.5 && fk <= len && d < w + 1.0 && d < best) {
            float pick = ak_hash(id + 3.3);
            bestInk = pick < 0.6 ? color1 : (pick < 0.9 ? color2 : accent);
            // Width tapers toward the stroke's growing tip.
            bestTaper = 1.0 - 0.6 * smoothstep(0.55, 1.0, fk / max(min(len, grown), 1.0));
            best = d;
            if (found < 0) found = k;
        }
        p = q;
    }
    col = mix(col, bestInk, clamp(w * bestTaper - best + 0.5, 0.0, 1.0));
    vec2 uv = ak_uv(fragCoord);
    float tooth = ak_noise(fragCoord * 0.7 / s) * 0.04 - 0.02;
    col *= 1.0 + tooth;
    col = ak_vignette(col, uv, 0.5);
    fragColor = vec4(ak_grain(col, fragCoord, 0.5), 1.0);
}
