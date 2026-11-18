// Halde & Sons — pass 2: "It's coffee." A tea table on white linen in morning sun, from above.
// The sun comes through the roastery's lettered shop window: its letters land on the cloth as
// shadow type. iChannel0 = window lettering mask (white on transparent, screen space),
// iChannel1 = the bag's printed label (screen space).

uniform float cx;    // = 0.0
uniform float cy;    // = 0.0
uniform float zoom;  // = 1.0
uniform float rot;   // = 0.0
uniform float sun;   // = 1.0
uniform float drift; // = 0.0   sun patch drift (window light crawling across the cloth)
uniform float stir;  // = 0.0   ripples in the cup
uniform float bag;   // = 0.0
uniform float bx;    // = 0.0
uniform float by;    // = -0.3
uniform float spoon; // = 1.0
uniform float letters; // = 1.0

const vec3 SUN = vec3(1.0, 0.85, 0.66);
const vec3 SKY = vec3(0.55, 0.64, 0.80);
const vec2 CUP = vec2(0.0, 0.10);
const float RS = 0.215;   // saucer
const float RC = 0.135;   // cup rim
const float RL = 0.118;   // liquid

mat2 r2(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }

// woven linen, warm white, with soft fold ridges
vec3 linen(vec2 p, out float h) {
    vec2 q = p * 520.0;
    float wx = 0.5 + 0.5 * sin(q.x + 1.3 * ak_noise(p * 40.0));
    float wy = 0.5 + 0.5 * sin(q.y + 1.3 * ak_noise(p * 40.0 + 7.0));
    float weave = mix(wx, wy, step(0.5, fract(floor(q.x / 3.14159) * 0.5 + floor(q.y / 3.14159) * 0.5)));
    float slub = ak_noise(vec2(p.x * 900.0, p.y * 30.0)) * ak_noise(vec2(p.x * 25.0, p.y * 900.0));
    h = 0.6 * ak_fbm(p * 2.2 + 3.0, 4);          // folds
    vec3 c = vec3(0.90, 0.88, 0.84) * (0.93 + 0.06 * weave + 0.05 * slub);
    return c;
}

// blue floral transfer band for the porcelain (polar pattern)
float floral(vec2 rel, float r0, float r1) {
    float r = length(rel);
    float a = atan(rel.y, rel.x);
    float band = smoothstep(r0, r0 + 0.004, r) * (1.0 - smoothstep(r1 - 0.004, r1, r));
    float n = 18.0;
    float sa = fract(a / 6.28318 * n) - 0.5;
    float rm = (r - (r0 + r1) * 0.5) / (r1 - r0);
    // petals: five-lobed flowers on a vine
    vec2 fp = vec2(sa * 6.28318 / n * (r0 + r1) * 0.5, rm * (r1 - r0)) * 120.0;
    float pa = atan(fp.y, fp.x);
    float flower = 1.0 - smoothstep(0.9, 1.1, length(fp * 0.55) / (0.75 + 0.35 * cos(pa * 5.0)));
    float vine = 1.0 - smoothstep(0.03, 0.12, abs(rm - 0.18 * sin(a * n * 2.0)));
    float leaf = 1.0 - smoothstep(0.3, 0.5, length((fp - vec2(1.5, 0.6)) * vec2(1.0, 2.2)));
    return band * clamp(max(max(flower, vine * 0.8), leaf * 0.9), 0.0, 1.0);
}

void mainImage(out vec4 o, in vec2 f) {
    float t = iTime;
    vec2 uv = (f - 0.5 * iResolution.xy) / iResolution.y;
    vec2 p = vec2(cx, cy) + r2(rot) * uv / zoom;
    float px = 1.0 / (iResolution.y * zoom);
    vec2 dir = normalize(vec2(-0.42, -1.0));   // sun travel, down-left
    vec2 nrm = vec2(-dir.y, dir.x);

    float fh;
    vec3 alb = linen(p, fh);
    vec3 nshade = vec3(1.0);

    // ---------- bag ----------
    vec2 bq = r2(0.06) * (p - vec2(bx, by));
    float bagD = ak_box(bq, vec2(0.235, 0.335)) - 0.01;
    float inBag = bag * (1.0 - smoothstep(-px, px, bagD));
    if (bag > 0.0) {
        vec3 paper = vec3(0.040, 0.036, 0.034) * (0.90 + 0.2 * ak_fbm(bq * 70.0, 3));
        float hA = ak_fbm(bq * 6.0 + 2.0, 4), hB = ak_fbm(bq * 6.0 + vec2(2.01, 2.0), 4), hC = ak_fbm(bq * 6.0 + vec2(2.0, 2.01), 4);
        paper *= 1.0 + 0.12 * dot(vec2(hB - hA, hC - hA) / 0.01, -dir);
        paper *= 1.0 - 0.35 * smoothstep(0.20, 0.235, abs(bq.x));
        float fold = smoothstep(0.255, 0.26, bq.y) * (1.0 - smoothstep(0.33, 0.335, bq.y));
        paper *= 1.0 - 0.3 * fold * (0.5 + 0.5 * sin(bq.x * 420.0));
        paper += 0.04 * (1.0 - smoothstep(0.0, 0.004, abs(bq.y - 0.255)));
        float vr = length(bq - vec2(0.16, 0.218));
        paper = mix(paper, vec3(0.07, 0.065, 0.06), 1.0 - smoothstep(0.020, 0.022, vr));
        paper += 0.06 * exp(-pow((vr - 0.013) / 0.002, 2.0));
        vec4 ink = texture(iChannel1, f / iResolution.xy);
        paper = mix(paper, ink.rgb / max(ink.a, 1e-3), ink.a * 0.96);
        alb = mix(alb, paper, inBag);
    }

    // ---------- sun through the shop window ----------
    // panes: a big patch whose edges and mullions are projected long and soft
    vec2 wp = p - vec2(drift, 0.0);
    vec2 wl = vec2(dot(wp, nrm), dot(wp, -dir));          // window space (x across, y along the light)
    float patch = smoothstep(-0.30, -0.27, wl.x) * (1.0 - smoothstep(0.25, 0.28, wl.x))
                * smoothstep(-1.30, -1.25, wl.y) * (1.0 - smoothstep(0.80, 0.85, wl.y));
    float mull = (1.0 - smoothstep(0.010, 0.018, abs(wl.x + 0.01))) + (1.0 - smoothstep(0.010, 0.018, abs(wl.y + 0.30)));
    float glazing = 0.96 + 0.04 * ak_noise(wl * vec2(3.0, 9.0));  // old glass waviness
    float sunlit = sun * patch * (1.0 - 0.92 * clamp(mull, 0.0, 1.0)) * glazing;
    // lettering on the window: shadow type
    vec2 lu = f / iResolution.xy, lo = vec2(2.0) / iResolution.xy;
    float let = (texture(iChannel0, lu).a * 2.0 + texture(iChannel0, lu + lo).a + texture(iChannel0, lu - lo).a
               + texture(iChannel0, lu + vec2(lo.x, -lo.y)).a + texture(iChannel0, lu + vec2(-lo.x, lo.y)).a) / 6.0 * letters;
    sunlit *= 1.0 - 0.95 * smoothstep(0.0, 0.8, let);

    // ---------- objects: shadows ----------
    vec2 rel = p - CUP;
    float tall = 0.16;  // cup height in shadow length units at this sun
    float cupSh = 1.0 - smoothstep(-0.006, 0.010, ak_segment(p, CUP, CUP + dir * tall) - RC * 0.95);
    float sauSh = 1.0 - smoothstep(-0.004, 0.008, length(p - CUP - dir * 0.012) - RS);
    float handleSh = 1.0 - smoothstep(-0.004, 0.008, length((p - CUP - vec2(0.17, -0.02) - dir * 0.10) * vec2(1.0, 1.4)) - 0.035);
    float spoonC = 0.0;
    vec2 sp0 = CUP + vec2(-0.05, -0.17), sp1 = CUP + vec2(0.17, -0.26);
    float spD = min(ak_segment(p, sp0, sp1) - 0.007, length((p - sp0 + normalize(sp1 - sp0) * 0.03) * vec2(1.0, 1.0)) - 0.024);
    float spSh = spoon * (1.0 - smoothstep(-0.003, 0.006, min(ak_segment(p - dir * 0.012, sp0, sp1) - 0.007, length(p - dir * 0.012 - sp0 + normalize(sp1 - sp0) * 0.03) - 0.024)));
    float bagSh = 0.0;
    if (bag > 0.0) bagSh = bag * (1.0 - smoothstep(-0.004, 0.008, ak_box(r2(0.06) * (p - vec2(bx, by) - dir * 0.02), vec2(0.235, 0.335)) - 0.01)) * (1.0 - inBag);
    float occl = max(max(cupSh, sauSh * (1.0 - step(length(rel), RS))), max(spSh, bagSh));
    float lit = sunlit * (1.0 - 0.9 * occl);

    vec3 light = SUN * 2.6 * lit + SKY * 0.30 * (0.6 + 0.4 * sun);
    light *= 0.9 + 0.2 * fh * (1.0 - inBag);
    vec3 col = alb * light;

    // ---------- the saucer, cup, tea ----------
    float r = length(rel);
    vec3 porc = vec3(0.95, 0.94, 0.92);
    vec3 blue = vec3(0.16, 0.26, 0.52);
    vec3 gold = vec3(0.86, 0.66, 0.30);
    if (r < RS + 0.004) {
        float sunHere = sunlit;
        // saucer: rim highlight, well, floral band, gold edge
        float well = smoothstep(0.155, 0.185, r);
        vec3 s = porc * (0.86 + 0.14 * well);
        s = mix(s, blue, 0.9 * floral(rel, 0.165, 0.201));
        s = mix(s, gold, 1.0 - smoothstep(0.0025, 0.004, abs(r - 0.207)));
        float spec = exp(-pow((r - 0.195) / 0.006, 2.0)) * smoothstep(0.0, 1.0, dot(normalize(rel), -dir));
        vec3 sl = SUN * 2.4 * sunHere * (1.0 - 0.85 * max(cupSh, spSh)) + SKY * 0.36;
        vec3 sc = s * sl + SUN * spec * 1.4 * sunHere;
        float cov = 1.0 - smoothstep(RS - px, RS + px, r);
        col = mix(col, sc, cov);
    }
    // handle (seen from above: a loop)
    vec2 hq = (rel - vec2(0.152, -0.018)) * vec2(1.0, 1.25);
    float hD = abs(length(hq) - 0.03) - 0.009;
    if (hD < 0.004) {
        vec3 hc = porc * (SUN * 2.0 * sunlit + SKY * 0.5);
        hc = mix(hc, gold * (SUN * 2.0 * sunlit + SKY * 0.5), 1.0 - smoothstep(0.0, 0.004, abs(abs(length(hq) - 0.03) - 0.006)));
        col = mix(col, hc, 1.0 - smoothstep(-px, px, hD));
    }
    if (r < RC + 0.004) {
        // inside wall of the cup, then the tea
        vec3 wall = porc * (0.75 + 0.25 * smoothstep(RL, RC, r));
        wall = mix(wall, blue, 0.85 * floral(rel, 0.121, 0.131) * 0.0);
        float wallSun = smoothstep(0.0, 0.6, dot(normalize(rel + 1e-5), -dir));   // far wall catches sun
        vec3 wc = wall * (SUN * 2.0 * sunlit * (0.35 + 0.65 * wallSun) + SKY * 0.45);
        float lipG = 1.0 - smoothstep(0.002, 0.0035, abs(r - (RC - 0.003)));
        wc = mix(wc, gold * (SUN * 2.0 * sunlit + SKY * 0.5), lipG);
        // tea: deep red-amber, lighter at the edge, light passing through toward the sun-far side
        float rr = r / RL;
        vec3 deep = vec3(0.30, 0.075, 0.018), edge = vec3(0.70, 0.30, 0.08);
        vec3 tea = mix(deep, edge, smoothstep(0.6, 1.0, rr));
        float through = smoothstep(0.0, 1.0, dot(rel / RL, dir)) * smoothstep(1.0, 0.7, rr);
        tea += vec3(0.85, 0.38, 0.09) * 0.5 * through * sun;
        float rip = sin(rr * 34.0 - t * 9.0) * stir * (0.3 + 0.7 * rr);
        tea *= 1.0 + 0.10 * rip;
        vec2 wr = rel / RL - vec2(0.33, 0.42) + 0.05 * rip * normalize(rel + 1e-5);
        wr = r2(0.42) * wr;
        float win = (1.0 - smoothstep(-0.02, 0.05, ak_box(wr, vec2(0.14, 0.17)))) * (1.0 - 0.8 * (1.0 - smoothstep(0.006, 0.02, abs(wr.x - 0.03))));
        tea += vec3(1.0, 0.96, 0.9) * win * 0.85 * (0.25 + 0.75 * sun) + SKY * (0.06 + 0.03 * rip) * smoothstep(0.5, 1.0, rr);
        float meni = exp(-pow((rr - 0.985) / 0.02, 2.0));
        tea += vec3(0.9, 0.6, 0.3) * 0.3 * meni;
        vec3 cc = mix(wc, tea, 1.0 - smoothstep(RL - px, RL + px, r));
        // outer rim
        float rimO = 1.0 - smoothstep(RC - px, RC + px, r);
        col = mix(col, cc, rimO);
    }
    // teaspoon (silver) on the saucer
    if (spoon > 0.0 && spD < 0.004) {
        vec2 sd = normalize(sp1 - sp0);
        float along = dot(p - sp0, sd);
        float shine = 0.5 + 0.5 * sin(along * 60.0 + dot(p - sp0, vec2(-sd.y, sd.x)) * 300.0);
        vec3 silver = vec3(0.30, 0.31, 0.33) * (0.4 + 0.9 * shine);
        vec3 sc = silver * (SUN * 1.8 * sunlit + SKY * 0.6) + SUN * 2.5 * sunlit * pow(shine, 12.0) + SKY * 0.3 * (1.0 - shine);
        col = mix(col, sc, spoon * (1.0 - smoothstep(-px, px, spD)));
    }

    vec2 sv = f / iResolution.xy - 0.5;
    col *= 1.0 - 0.6 * dot(sv, sv);
    col = ak_tonemap(col * 0.62);
    col = ak_grain(col, f, 0.6);
    o = vec4(col, 1.0);
}
