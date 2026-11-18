// Tern pass 2 — the inside of a waxed-cotton pocket, seen from the recorder.
// The pocket mouth is a lens-shaped opening; inside it is transparent, so the
// world layer below (the sea) shows through. The fabric is very close to the
// lens: soft, lit only by the daylight coming through the mouth. A woven
// label (text from iChannel0) is sewn to the lining.
uniform float open;   // = 0.2  mouth half-height (frame heights)
uniform float mw;     // = 0.9  mouth half-width
uniform float cx;     // = 0.0  mouth centre
uniform float cy;     // = 0.1
uniform float ang;    // = 0.0  mouth tilt, degrees
uniform float sway;   // = 1.0  walking sway amount
uniform float lab;    // = 0.0  label visibility
uniform float labx;   // = 0.0
uniform float laby;   // = -0.18
uniform float labs;   // = 1.0
uniform float warm;   // = 0.0

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vn(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(h21(i), h21(i + vec2(1, 0)), u.x), mix(h21(i + vec2(0, 1)), h21(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p) {
    float s = 0.0, a = 0.5;
    for (int i = 0; i < 4; i++) { s += a * vn(p); p = p * 2.03 + 7.1; a *= 0.5; }
    return s;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    vec2 p = (fragCoord - 0.5 * res) / res.y;
    float t = iTime;
    // walking sway: the whole pocket rocks a little
    vec2 sw = sway * vec2(0.012 * sin(t * 3.4), 0.018 * sin(t * 6.8 + 0.6));
    float a = radians(ang + sway * 2.0 * sin(t * 3.4));
    vec2 d = p - vec2(cx, cy) - sw;
    d = mat2(cos(a), sin(a), -sin(a), cos(a)) * d;
    // the mouth: a lens, pinched at both ends; edges wobble like cloth
    float xn = d.x / mw;
    float wob = 0.012 * (vn(vec2(d.x * 6.0, t * 0.7)) - 0.5) + 0.006 * sin(d.x * 23.0 + t * 2.0);
    float hh = open * max(1.0 - xn * xn, 0.0) + wob * min(open * 8.0, 1.0);
    float edge = abs(d.y) - hh;                   // < 0 inside the mouth
    float blurW = 0.012 + 0.03 * smoothstep(0.0, 0.6, open);
    float inside = smoothstep(blurW, -blurW, edge);
    // light reaching the fabric: from the mouth, falling off with distance
    float dist = max(edge, 0.0);
    float lightAmt = min(open * 4.0, 1.0) * (exp(-dist * 9.0) * 0.9 + 0.08 * exp(-dist * 2.0));
    vec3 day = mix(vec3(1.0, 0.86, 0.70), vec3(1.0, 0.70, 0.45), warm);
    // fabric: olive waxed cotton twill with soft folds, out of focus near the lens
    vec2 fp = d * vec2(1.0, 1.0);
    float folds = fbm(fp * vec2(0.9, 1.6) + 4.0);
    float fold2 = fbm(fp * vec2(2.0, 1.2) + 9.0 + t * 0.03);
    float hgt = folds * 0.7 + fold2 * 0.3;
    float e = 0.01;
    vec2 g = vec2(fbm((fp + vec2(e, 0)) * vec2(0.9, 1.6) + 4.0) - fbm((fp - vec2(e, 0)) * vec2(0.9, 1.6) + 4.0),
                  fbm((fp + vec2(0, e)) * vec2(0.9, 1.6) + 4.0) - fbm((fp - vec2(0, e)) * vec2(0.9, 1.6) + 4.0)) / (2.0 * e);
    vec2 toMouth = normalize(-d * vec2(1.0, 3.0) + vec2(0.0, 0.0001));
    if (edge > 0.0) toMouth = normalize(vec2(0.0, -sign(d.y)) + 0.3 * vec2(-sign(d.x) * smoothstep(0.5, 1.0, abs(xn)), 0.0));
    float shade = clamp(0.6 + 0.5 * dot(g, toMouth), 0.2, 1.3);
    // twill: fine diagonal ribs, only resolved away from the soft edge
    vec2 tw = fragCoord / res.y * 420.0;
    float rib = 0.5 + 0.5 * sin((tw.x + tw.y * 1.8) * 1.3 + vn(tw * 0.05) * 3.0);
    float sharp = smoothstep(0.02, 0.25, dist);
    vec3 alb = vec3(0.16, 0.15, 0.095) * (0.88 + 0.18 * hgt) * mix(1.0, 0.75 + 0.5 * rib, 0.6 * sharp);
    // the hem around the mouth: a folded edge that catches the light, with stitches
    float hem = smoothstep(0.05, 0.0, dist) * step(0.0, edge + 0.002) * min(open * 6.0, 1.0);
    float st = smoothstep(0.35, 0.5, fract(d.x * 30.0)) * smoothstep(0.65, 0.5, fract(d.x * 30.0)) * smoothstep(0.006, 0.002, abs(dist - 0.032));
    vec3 col = alb * (day * lightAmt * 1.6 * shade + vec3(0.010, 0.011, 0.013));
    col += day * hem * 0.20 * lightAmt * (0.6 + 0.4 * folds);
    col += vec3(0.45, 0.40, 0.28) * st * lightAmt * 0.18;
    // the woven label
    if (lab > 0.0) {
        vec2 lp = (p - vec2(labx, laby) - sw * 0.6) / labs;
        lp.y += 0.01 * sin(lp.x * 5.0);           // cloth curl
        vec2 lh = vec2(0.42, 0.22);
        float ld = max(abs(lp.x) - lh.x, abs(lp.y) - lh.y);
        float cover = smoothstep(0.004, -0.004, ld) * lab;
        vec2 uv = vec2(0.5 + lp.x / (2.0 * lh.x), 0.5 + lp.y / (2.0 * lh.y));
        vec4 tc = texture(iChannel0, clamp(uv, 0.0, 1.0));
        float ink = clamp(tc.a * dot(tc.rgb, vec3(0.333)), 0.0, 1.0);
        // woven threads: warp/weft
        vec2 wv = fragCoord / res.y * 520.0 / labs;
        float weave = 0.75 + 0.25 * sin(wv.x * 3.1) * sin(wv.y * 3.1 + 1.0) + 0.1 * (h21(floor(wv)) - 0.5);
        vec3 cloth = vec3(0.86, 0.82, 0.72);
        vec3 thread = vec3(0.10, 0.12, 0.20);
        vec3 lc = mix(cloth, thread, ink) * weave;
        float bord = smoothstep(0.03, 0.02, -ld) * 0.25;
        lc *= 1.0 - bord;
        float lLight = lightAmt * 1.6 + 0.06;
        float labShade = 0.85 + 0.15 * sin(lp.x * 5.0 + 1.0);
        col = mix(col, lc * day * lLight * labShade, cover);
        col *= 1.0 - 0.5 * smoothstep(0.03, 0.0, ld) * step(0.0, ld) * lab; // small shadow under the label
    }
    col = 1.0 - exp(-col * 1.4);
    col = pow(col, vec3(0.4545));
    // the world through the mouth (already graded), with a touch of edge vignette
    vec3 world = texture(iChannel1, fragCoord / res).rgb;
    world *= 1.0 - 0.35 * smoothstep(-0.12, 0.0, edge) * (1.0 - smoothstep(0.6, 1.2, open));
    col = mix(col, world, inside);
    col += (h21(fragCoord + float(iFrame) * 1.3) - 0.5) * 0.03;
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
