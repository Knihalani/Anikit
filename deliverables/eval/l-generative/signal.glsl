// SIGNAL — a raster of lines: static -> pulses -> interference -> one line -> the word.
// Everything is drawn by one material: horizontal lines whose weight and bend carry the field.

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = h21(i), b = h21(i + vec2(1.0, 0.0));
    float c = h21(i + vec2(0.0, 1.0)), d = h21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// one source: returns wave value, writes how "ordered" it makes this point
float src(vec2 q, vec2 s, float t0, float t, float k, inout float ord) {
    float age = t - t0;
    if (age <= 0.0) return 0.0;
    float r = length(q - s);
    float front = age * 0.42;
    float inside = smoothstep(front + 0.02, front - 0.10, r);
    float lead = exp(-pow((r - front) / 0.025, 2.0)) * exp(-age * 0.35);
    ord += inside;
    return (sin(k * r - age * 7.0) * inside * (0.55 + 0.45 * exp(-r * 1.2))) + lead * 1.6;
}

// the field at a point; heat = constructive peaks
float field(vec2 q, float t, float k, out float heat, out float ordOut) {
    float ord = 0.0;
    float w = 0.0;
    // sources drift slowly (orbit) so the moire keeps moving
    float d = t * 0.05;
    w += src(q, vec2(0.30 + 0.03 * sin(d * 7.0), 0.06), 1.5, t, k, ord);
    w += src(q, vec2(-0.46, -0.16 + 0.03 * cos(d * 5.0)), 4.0, t, k, ord);
    w += src(q, vec2(0.02 * cos(d * 6.0), -0.34), 6.5, t, k * 1.1, ord);
    w += src(q, vec2(-0.12, 0.33), 8.0, t, k * 0.9, ord);
    w += src(q, vec2(0.66, -0.22), 9.5, t, k * 1.2, ord);
    w += src(q, vec2(-0.74, 0.28), 10.5, t, k, ord);
    w += src(q, vec2(0.0, 0.0), 11.25, t, k * 1.35, ord);
    ord = clamp(ord, 0.0, 1.0);
    float n = clamp(ord + 0.0, 0.0, 6.0);
    float active = 1.0 + step(4.0, t) + step(6.5, t) + step(8.0, t) + step(9.5, t) + step(10.5, t) + step(11.25, t);
    w /= sqrt(active);
    heat = smoothstep(0.75, 1.7, w);
    ordOut = ord;
    return w;
}

void mainImage(out vec4 o, in vec2 f) {
    float t = iTime;
    vec2 R = iResolution.xy;
    float H = R.y;
    float P = H / 84.0;                   // line pitch
    vec3 ink   = vec3(0.035, 0.034, 0.038);
    vec3 paper = vec3(0.93, 0.91, 0.86);
    vec3 hot   = vec3(1.0, 0.30, 0.10);

    // stages
    float zoom = 1.0 + 0.22 * smoothstep(3.0, 13.4, t);
    float k = 34.0 + 10.0 * smoothstep(6.0, 13.0, t);
    float collapse = smoothstep(13.4, 13.6, t);         // everything -> one line
    float reveal = smoothstep(15.0, 16.7, t);             // lines open into the word
    float textOn = step(14.95, t);
    float noiseAmt = 1.0 - smoothstep(9.0, 12.5, t);

    float yRel = f.y - 0.5 * H;
    float jc = floor(yRel / P + 0.5);
    float cover = 0.0;
    vec3 col = ink;

    for (int dj = -2; dj <= 2; dj++) {
        float j = jc + float(dj);
        float yc = 0.5 * H + j * P;
        vec2 q = (vec2(f.x, yc) - 0.5 * R) / H / zoom;
        float heat, ord;
        float F = field(q, t, k, heat, ord);

        // static: boiling value noise along each line, stepped every 2 frames
        float dim = 1.0;
        float boil = floor(t * 12.0);
        float nz = vnoise(vec2(f.x / H * 46.0 + boil * 17.3, j * 3.7 + boil * 1.3));
        nz = nz * nz * 1.6 - 0.45;
        float sig = mix(nz * noiseAmt, F, max(ord, 1.0 - noiseAmt));
        sig *= smoothstep(0.0, 1.2, t);                      // out of black

        // weight and bend from the field
        float wgt = P * (0.06 + (0.40 + 0.22 * smoothstep(10.0, 13.4, t)) * smoothstep(-0.35, 1.25, sig));
        float bend = P * 0.55 * F * ord * (0.4 + 1.6 * smoothstep(7.0, 13.4, t));
        float c = mix(0.0, heat, smoothstep(7.5, 12.5, t));

        // collapse: only the centre line survives
        float isMid = 1.0 - step(0.5, abs(j));
        float pulse = sin(f.x / H * 9.0 - t * 5.0) * exp(-pow((f.x / R.x - fract((t - 13.8) * 0.45)) / 0.08, 2.0));
        float midW = P * 0.16;
        float midBend = P * 1.6 * pulse * (1.0 - reveal);
        wgt = mix(wgt, isMid * midW, collapse);
        bend = mix(bend, isMid * midBend, collapse);
        c = mix(c, isMid, collapse);

        // the word: lines sample the type at their centre line
        if (textOn > 0.5) {
            vec2 uv = vec2(f.x / R.x, yc / H);
            float m = texture(iChannel0, uv).r;
            float open = smoothstep(reveal * H * 0.30 + P, reveal * H * 0.30 - P, abs(yc - 0.5 * H));
            float tw = m * P * 0.70 * open;
            // after the word lands, the old field returns faintly in the background
            float rr = length((vec2(f.x, yc) - 0.5 * R) / H);
            float ring = 0.5 + 0.5 * sin(rr * 30.0 - (t - 16.0) * 3.0);
            float gfade = smoothstep(16.4, 18.6, t) * smoothstep(0.18, 0.6, rr);
            float ghost = P * 0.16 * ring * gfade * (1.0 - m);
            dim = mix(1.0, 0.42, step(tw + 0.001, ghost));
            float settle = 1.0 - smoothstep(15.6, 17.4, t);
            wgt = max(max(wgt, tw), ghost);
            bend += m * open * P * 0.9 * sin(f.x / H * 14.0 - t * 9.0) * settle;
            c = max(c * (1.0 - m * open), isMid * open * 0.0 + isMid * (1.0 - m) * 1.0);
        }

        c = clamp(c, 0.0, 1.0);
        float dist = abs(f.y - (yc + bend));
        float a = smoothstep(wgt * 0.5 + 0.8, wgt * 0.5 - 0.8, dist) * step(0.02, wgt);
        if (a > cover) {
            cover = a;
            col = mix(ink, mix(paper, hot, c) * dim, a);
        }
    }

    // the screen: slight falloff, a breath of light around crests
    vec2 v = f / R - 0.5;
    col *= 1.0 - 0.35 * dot(v, v);
    o = vec4(col, 1.0);
}
