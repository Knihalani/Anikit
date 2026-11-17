// Offset 2027 — azulejo system. One module, one rule, offset rows, a system.
// All choreography is driven by iTime (video seconds). iChannel0 = title mask.

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float ease(float x) { x = clamp(x, 0.0, 1.0); return x < 0.5 ? 4.0*x*x*x : 1.0 - pow(-2.0*x + 2.0, 3.0) * 0.5; }
float expo(float x) { x = clamp(x, 0.0, 1.0); return x < 0.5 ? 0.5*pow(2.0, 20.0*x - 10.0) : 1.0 - 0.5*pow(2.0, -20.0*x + 10.0); }
mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }

// cell size in px over time
float cellSize(float t) {
    float a = mix(640.0, 560.0, ease(t / 2.5));                 // slow push on the single tile
    float b = mix(a, 150.0, expo((t - 2.45) / 1.5));             // pull out: copies appear
    float c = mix(b, 96.0, ease((t - 5.0) / 2.4));              // drift
    float d = mix(c, 36.0, expo((t - 7.4) / 0.9));
    d *= 1.0 + 0.06 * ease((t - 11.0) / 4.0);   // slow push on the end card              // the system: tiles become pixels of the title
    return d;
}

// row offset (in tiles) applied to odd rows: steps on the beat (96 bpm, 0.625 s)
float rowShift(float t) {
    float s = 0.0;
    s += 0.5 * expo((t - 5.00) / 0.45);
    s += 0.5 * expo((t - 5.625) / 0.45);
    s -= 0.5 * expo((t - 6.25) / 0.45);
    s += 0.5 * expo((t - 6.875) / 0.45);
    s -= 1.0 * expo((t - 7.45) / 0.6);
    return s;
}

void mainImage(out vec4 o, in vec2 f) {
    float t = iTime;
    vec2 R = iResolution.xy;
    float cs = cellSize(t) * R.x / 1920.0;
    vec2 drift = vec2(0.0);
    drift.x = 0.35 * ease((t - 5.0) / 2.4) * (1.0 - ease((t - 7.4) / 0.8));
    vec2 p = (f - 0.5 * R) / cs + 0.5 + drift;
    // the whole field turns a little while it is a pattern, settles square for the title
    float tilt = 0.06 * sin(clamp(t - 2.5, 0.0, 5.0) * 0.63) * (1.0 - ease((t - 7.4) / 0.8));
    p = rot(tilt) * (p - 0.5) + 0.5;

    float row = floor(p.y);
    float odd = mod(row, 2.0);
    float sh = rowShift(t) * (odd > 0.5 ? 1.0 : -1.0);
    p.x += sh;
    vec2 cell = floor(p);
    vec2 q = fract(p) - 0.5;
    float px = 1.0 / cs;
    float h = hash(cell);
    float h2 = hash(cell + 17.3);

    // ---- distance from the first tile, for reveals that grow outward
    float dist = length(cell);

    // ---- title mask: sample the mask at this cell's centre (in pixels)
    vec2 cc = cell + 0.5; cc.x -= sh;
    cc = rot(-tilt) * (cc - 0.5) + 0.5;
    vec2 cpx = (cc - 0.5 - drift) * cs + 0.5 * R;
    float inTitle = texture(iChannel0, cpx / R).r;
    inTitle = step(0.5, inTitle) * step(0.0, cpx.x) * step(cpx.x, R.x) * step(0.0, cpx.y) * step(cpx.y, R.y);
    float band = step(cpx.y, 0.215 * R.y);   // bottom info strip on the end card

    // ---- rules: quarter-turn per cell
    float rA = 0.0;
    float rB = mod(cell.x + cell.y, 2.0);               // checker: circles
    float rC = floor(h * 4.0);                          // random: meander (truchet)
    float tB = 3.55 + 0.05 * dist + 0.15 * h2;
    float tC = 6.25 + 0.035 * abs(cell.x) + 0.2 * h2;
    float tD = 7.45 + 0.02 * dist + 0.2 * h;
    float turns = rA + ease((t - tB) / 0.4) * (rB - rA) + ease((t - tC) / 0.4) * (rC - rB);
    turns += ease((t - tD) / 0.4) * (rB + 4.0 - rC) ;
    vec2 m = rot(turns * 1.5707963) * q;

    // ---- tile flip into the title (and into the end-card strip)
    float tf = 8.15 + 0.018 * (cell.x + 30.0) + 0.12 * h;
    float fp = ease((t - tf) / 0.32) * inTitle;
    float tb = 11.35 + 0.012 * (cell.x + 30.0) + 0.08 * h;
    float bp = ease((t - tb) / 0.32) * band * (1.0 - inTitle);
    float flip = max(fp, bp);
    float squash = abs(cos(flip * 3.14159265));
    float inverted = step(0.5, flip);
    float plain = step(0.5, bp);

    // ---- reveal (draw-on) per cell
    float rv = clamp((t + 0.35) / 1.9, 0.0, 1.0);
    if (dist > 0.1) rv = clamp((t - 2.25 - 0.16 * dist + 0.1 * h) / 0.6, 0.0, 1.0);
    rv = ease(rv);

    // ---- motif in m (rotated cell space)
    vec2 c1 = vec2(-0.5, -0.5), c2 = vec2(0.5, 0.5);
    vec2 c3 = vec2(0.5, -0.5), c4 = vec2(-0.5, 0.5);
    float l1 = length(m - c1), l2 = length(m - c2);
    float a1 = atan(m.y - c1.y, m.x - c1.x) / 1.5707963;     // 0..1 along arc
    float a2 = atan(c2.y - m.y, c2.x - m.x) / 1.5707963;
    float sweep1 = step(a1, rv * 1.02), sweep2 = step(a2, rv * 1.02);
    float w = 0.115;
    float arc = max(smoothstep(px, -px, abs(l1 - 0.5) - w) * sweep1,
                    smoothstep(px, -px, abs(l2 - 0.5) - w) * sweep2);
    float lw = 0.014 + 0.6 * px;
    float fine = 0.0;
    fine = max(fine, smoothstep(px, -px, abs(abs(l1 - 0.5) - 0.185) - lw) * sweep1);
    fine = max(fine, smoothstep(px, -px, abs(abs(l2 - 0.5) - 0.185) - lw) * sweep2);
    float l3 = length(m - c3), l4 = length(m - c4);
    float dr = 0.17 * rv;
    float disc = max(smoothstep(px, -px, l3 - dr), smoothstep(px, -px, l4 - dr));
    fine = max(fine, max(smoothstep(px, -px, abs(l3 - 0.255) - lw), smoothstep(px, -px, abs(l4 - 0.255) - lw)) * step(0.6, rv));
    float dia = smoothstep(px, -px, abs(m.x) + abs(m.y) - 0.085 * rv);
    float motif = max(max(arc, disc), max(dia, fine));
    // ink pools darker in the middle of the wide bands
    float pool = max(1.0 - abs(l1 - 0.5) / w, 1.0 - abs(l2 - 0.5) / w);
    pool = clamp(pool, 0.0, 1.0) * arc;

    // ---- colours
    vec3 white = vec3(0.945, 0.952, 0.962) * (0.975 + 0.035 * h2);
    vec3 cobalt = mix(vec3(0.13, 0.26, 0.66), vec3(0.06, 0.14, 0.46), 0.25 * pool + 0.2 * h);
    // once the title forms, the field steps back to a pale echo so the letters read
    float back = ease((t - 7.8) / 0.9) * (1.0 - inverted);
    vec3 ink = mix(cobalt, vec3(0.76, 0.81, 0.93), back);

    vec3 ground = white, fig = ink;
    if (inverted > 0.5) {
        ground = mix(vec3(0.08, 0.17, 0.52), vec3(0.11, 0.22, 0.60), h2);
        fig = vec3(0.30, 0.44, 0.86);
        if (plain > 0.5) fig = mix(ground, vec3(0.94, 0.95, 0.96), 0.08);
    }
    vec3 col = mix(ground, fig, motif);

    // ---- glaze: soft per-tile highlight and a bevel toward the grout
    vec2 qs = vec2(q.x, q.y / max(squash, 0.001));
    float e = 0.5 - max(abs(qs.x), abs(qs.y));
    float bevel = smoothstep(0.0, 0.07, e);
    col *= mix(0.86, 1.0, bevel);
    float spec = exp(-12.0 * dot(qs - vec2(-0.18, 0.2), qs - vec2(-0.18, 0.2)));
    col += (0.05 + 0.03 * h) * spec * (1.0 - 0.6 * flip * (1.0 - flip) * 4.0);
    // crazing: hairline cracks in the glaze, very faint
    float cr = abs(sin(dot(q, vec2(31.0, 17.0) + h * 40.0)) * sin(dot(q, vec2(-13.0, 29.0) + h2 * 30.0)));
    col *= 1.0 - 0.025 * smoothstep(0.02, 0.0, cr) * step(80.0, cs);

    // ---- grout and the flip gap
    float gw = max(1.0 * px, 0.008);
    float tileMask = smoothstep(-px, px, 0.5 * squash - abs(q.y)) * smoothstep(-px, px, 0.5 - abs(q.x));
    float groutM = smoothstep(gw + px, gw - px, 0.5 - max(abs(q.x), abs(q.y)) );
    vec3 grout = mix(vec3(0.74, 0.75, 0.74), vec3(0.86, 0.87, 0.88), back) * (0.94 + 0.06 * h);
    if (plain > 0.5 || (inverted > 0.5)) grout = vec3(0.05, 0.10, 0.32);
    if (flip > 0.001 && flip < 0.999) grout = vec3(0.10, 0.15, 0.36);
    col = mix(grout, col, tileMask * (1.0 - groutM));
    // flipping tiles darken edge-on
    col *= mix(1.0, 0.55 + 0.45 * squash, step(0.001, flip) * step(flip, 0.999));

    // ---- a glint that travels across the glaze (end card)
    vec2 uvg = f / R;
    float gx = uvg.x * 1.0 + (1.0 - uvg.y) * 0.45 - mix(-0.4, 1.8, clamp((t - 12.2) / 2.4, 0.0, 1.0));
    col += 0.10 * exp(-gx * gx * 60.0) * tileMask * bevel * step(12.1, t);

    // ---- light: a broad window light from the upper left
    vec2 uv = f / R;
    float light = 0.92 + 0.12 * smoothstep(1.3, 0.0, length(uv - vec2(0.2, 0.95)));
    col *= light;
    col *= 1.0 - 0.18 * pow(length(uv - 0.5) * 1.25, 2.5);
    o = vec4(clamp(col, 0.0, 1.0), 1.0);
}
