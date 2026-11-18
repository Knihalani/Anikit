// Paint: turns the layers below into brushwork. Every stroke takes its colour
// from the image at its centre and lies across the local gradient, so strokes
// follow form (around a face, along a horizon); a second, finer pass of
// strokes adds detail only where the image has edges. Strokes carry bristle
// streaks and the canvas weave shows in thin paint.
// A filter: {"shader": "paint", "filter": true}.
// Uniforms: size, detail, stroke, bristles, canvas, jitter.
uniform float size;     // = 18.0 stroke width in pixels at 1080p
uniform float detail;   // = 1.0 fine strokes at edges (0 = only broad strokes)
uniform float stroke;   // = 2.4 stroke length in widths
uniform float bristles; // = 1.0 bristle streaks in each stroke
uniform float canvas;   // = 0.6 canvas weave in thin paint
uniform float jitter;   // = 0.25 random turn of each stroke (radians)

float lum(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }

// The topmost stroke of one grid layer over pixel p (1080p pixels):
// rgb = its colour, a = coverage.
vec4 strokes(vec2 p, float s, float seed, float gate) {
    vec2 q = p / s, i = floor(q);
    vec2 res = iResolution.xy * (1080.0 / iResolution.y);
    vec4 best = vec4(0.0);
    float top = -1.0;
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec2 cell = i + vec2(float(x), float(y));
            vec2 h = ak_hash2(cell + seed);
            vec2 c = (cell + 0.15 + 0.7 * h) * s;          // stroke centre, pixels
            vec2 uv = c / res;
            vec3 col = texture(iChannel0, uv).rgb;
            float l0 = lum(col);
            float gx = lum(texture(iChannel0, uv + vec2(0.5 * s / res.x, 0.0)).rgb) - l0;
            float gy = lum(texture(iChannel0, uv + vec2(0.0, 0.5 * s / res.y)).rgb) - l0;
            float g = length(vec2(gx, gy));
            // Fine strokes only where there is an edge to describe.
            float keep = gate < 0.0 ? 1.0 : smoothstep(gate, gate + 0.06, g);
            if (keep <= 0.0) continue;
            // Along the edge (across the gradient); flat areas keep a random lean.
            float a = g > 0.004 ? atan(gy, gx) + 1.5708 : h.x * 6.2832;
            a += (h.y - 0.5) * 2.0 * jitter;
            vec2 dir = vec2(cos(a), sin(a));
            vec2 d = p - c;
            float al = dot(d, dir) / s, ac = dot(d, vec2(-dir.y, dir.x)) / s;
            float hl = 0.5 * stroke * (0.8 + 0.4 * h.x);
            float w = 0.5 * (1.0 - 0.35 * smoothstep(0.3 * hl, hl, abs(al)));
            float dist = length(vec2(max(abs(al) - hl, 0.0), ac)) - w;
            float px = 1.0 / s;
            float cov = clamp(0.5 - dist / px, 0.0, 1.0) * keep;
            if (cov <= 0.0) continue;
            // Bristles: streaks along the stroke, lighter and darker paint.
            float br = ak_noise(vec2(ac * 9.0 + h.y * 31.0, al * 0.7)) - 0.5;
            vec3 sc = col * (1.0 + br * 0.22 * bristles) + br * 0.03 * bristles;
            float pr = ak_hash(cell + seed + 7.7);         // painting order
            if (pr > top) {
                top = pr;
                best = vec4(sc, cov);
            } else if (best.a < 1.0) {
                best = vec4(mix(sc, best.rgb, best.a), max(best.a, cov));
            }
        }
    }
    return best;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 p = ak_px(fragCoord);
    vec3 under = texture(iChannel0, fragCoord / iResolution.xy).rgb;
    vec4 a = strokes(p, size, 0.0, -1.0);
    vec3 col = mix(under, a.rgb, a.a);
    if (detail > 0.0) {
        vec4 b = strokes(p, size * 0.45, 19.0, 0.05 / detail);
        col = mix(col, b.rgb, b.a);
    }
    // Canvas weave shows where the paint is thin (light, low-contrast strokes).
    float weave = (sin(p.x * 1.9) * sin(p.y * 1.9)) * 0.5 + (ak_noise(p * 0.7) - 0.5);
    col *= 1.0 + weave * 0.035 * canvas;
    fragColor = vec4(col, 1.0);
}
