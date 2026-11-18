// Halde & Sons — Ethiopia Guji. One world: a counter in low morning sun, seen from above.
// A tumbler of clear, amber brew throws a caustic into its long shadow; the caustic draws
// the tasting notes. iChannel0 = light words (mask, white on transparent), iChannel1 = label ink.

uniform float cx;    // = 0.0   camera centre (world units: frame height = 1 at zoom 1)
uniform float cy;    // = 0.0
uniform float zoom;  // = 1.0
uniform float rot;   // = 0.0   camera roll (radians)
uniform float morph; // = 0.0   0 blob · 1 peach · 2 bergamot · 3 tea leaf · 4 broad oval (on the bag)
uniform float slosh; // = 0.0   liquid unrest 0..1 (scrambles the caustic)
uniform float sun;   // = 1.0   sunlight (cloud = 0.15)
uniform float mat;   // = 0.0   counter: 0 terrazzo · 1 ash wood · 2 honed slate
uniform float bag;   // = 0.0   bag on the counter
uniform float gx;    // = 0.0   glass position
uniform float gy;    // = 0.25
uniform float cscale; // = 1.0  caustic size
uniform float words; // = 1.0   light-word strength
uniform float bar;   // = 0.0   mullion shadow strength
uniform float barx;  // = 0.3   mullion offset
uniform float slen;  // = 0.78  shadow length (low sun; shorter as it climbs)
uniform float bx;    // = 0.0   bag centre
uniform float by;    // = -0.3

const float R = 0.115;           // glass radius
const vec3 SUN = vec3(1.0, 0.84, 0.64);
const vec3 SKY = vec3(0.50, 0.60, 0.78);
const vec3 AMB1 = vec3(1.0, 0.60, 0.16);
const vec3 AMB2 = vec3(0.95, 0.20, 0.02);

float h21(vec2 p) { return ak_hash(p); }

mat2 r2(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }

// ---------- counter albedo ----------
vec3 terrazzo(vec2 p) {
    vec3 base = vec3(0.86, 0.84, 0.80);
    float n = ak_fbm(p * 9.0, 4);
    base *= 0.95 + 0.07 * n;
    // chips: three voronoi scales
    for (int k = 0; k < 3; k++) {
        float s = 22.0 + float(k) * 30.0;
        vec3 v = ak_voronoi(p * s + float(k) * 13.1);
        float id = v.z;
        float sel = h21(vec2(id, float(k)));
        float sz = 0.10 + 0.22 * h21(vec2(id * 1.7, 3.0 + float(k)));
        float chip = 1.0 - smoothstep(sz - 0.06, sz + 0.02, v.x + 0.08 * ak_noise(p * s * 3.0));
        vec3 cc = sel < 0.35 ? vec3(0.55, 0.53, 0.50) : (sel < 0.6 ? vec3(0.30, 0.29, 0.28) : (sel < 0.8 ? vec3(0.78, 0.55, 0.42) : vec3(0.93, 0.92, 0.89)));
        cc *= 0.9 + 0.2 * h21(vec2(id, 9.0));
        float keep = step(0.55, h21(vec2(id, 21.0 + float(k))));
        base = mix(base, cc, chip * keep);
    }
    float speck = step(0.985, h21(floor(p * 900.0)));
    base *= 1.0 - 0.25 * speck;
    return base;
}

vec3 ashwood(vec2 p) {
    vec2 q = p * vec2(1.0, 0.12);
    float w = ak_fbm(vec2(q.x * 3.0, q.y * 4.0) + ak_fbm(p * 2.0, 3) * 0.6, 4);
    float rings = sin((p.x + 0.25 * w) * 70.0);
    vec3 a = vec3(0.80, 0.70, 0.56), b = vec3(0.66, 0.54, 0.40);
    vec3 c = mix(a, b, smoothstep(0.2, 1.0, rings) * 0.6 + 0.3 * w);
    c *= 0.96 + 0.08 * ak_noise(vec2(p.x * 400.0, p.y * 8.0));
    // plank seams
    float seam = abs(fract(p.x * 3.2 + 0.37) - 0.5);
    c *= 1.0 - 0.45 * (1.0 - smoothstep(0.0, 0.004, seam));
    return c;
}

vec3 slate(vec2 p) {
    float n = ak_fbm(p * 5.0 + ak_fbm(p * 2.0, 3), 5);
    vec3 c = mix(vec3(0.46, 0.49, 0.51), vec3(0.58, 0.60, 0.61), n);
    float vein = 1.0 - smoothstep(0.0, 0.02, abs(ak_fbm(p * 3.0 + 7.0, 4) - 0.5));
    c = mix(c, vec3(0.62, 0.64, 0.64), vein * 0.18);
    float fl = step(0.992, ak_hash(floor(p * 700.0)));
    c = mix(c, vec3(0.75, 0.74, 0.70), fl * 0.6);
    c *= 0.90 + 0.16 * ak_fbm(p * 40.0, 3);
    c *= 1.0 - 0.18 * smoothstep(0.6, 0.8, ak_fbm(p * 12.0 + 3.0, 4));
    c *= 0.97 + 0.06 * ak_noise(p * 600.0);
    return c;
}

vec3 counter(vec2 p) {
    if (mat < 0.5) return terrazzo(p);
    if (mat < 1.5) return ashwood(p);
    return slate(p);
}

// ---------- caustic shapes (local units, size ~1) ----------
float sdCircle(vec2 p, float r) { return length(p) - r; }
float sdVesica(vec2 p, float r, float d) {
    p = abs(p);
    float b = sqrt(r * r - d * d);
    return ((p.y - b) * d > p.x * b) ? length(p - vec2(0.0, b)) : length(p - vec2(-d, 0.0)) - r;
}
float leafShape(vec2 p, float len, float wid) {
    // vesica along y, length len, half-width wid
    float d = (len * len / 4.0 - wid * wid) / (2.0 * wid);
    float r = d + wid;
    return sdVesica(p, r, d);
}
float sdPeach(vec2 q) {
    float a = sdCircle(q - vec2(-0.13, -0.02), 0.78);
    float b = sdCircle(q - vec2(0.15, 0.03), 0.74);
    float d = ak_smin(a, b, 0.25);
    d = max(d, -sdCircle(q - vec2(0.05, 0.93), 0.12)); // stem dimple
    d = ak_smin(d, sdCircle(q - vec2(0.10, -0.80), 0.08), 0.15); // blossom tip
    float lf = leafShape(r2(-0.9) * (q - vec2(0.42, 0.92)), 0.62, 0.17);
    float st = ak_segment(q, vec2(0.04, 0.70), vec2(0.10, 0.90)) - 0.04;
    return min(min(d, lf), st);
}
float sdBerg(vec2 q) {
    vec2 s = q * vec2(1.0, 1.06);
    float d = sdCircle(s, 0.80);
    d = ak_smin(d, sdCircle(q - vec2(0.0, -0.84), 0.11), 0.12); // blossom-end nub
    float st = ak_segment(q, vec2(0.0, 0.70), vec2(-0.06, 1.02)) - 0.035;
    float l1 = leafShape(r2(0.95) * (q - vec2(-0.42, 1.08)), 0.78, 0.2);
    float l2 = leafShape(r2(-1.15) * (q - vec2(0.36, 1.04)), 0.58, 0.15);
    return min(min(d, st), min(l1, l2));
}
float sdTea(vec2 q) {
    vec2 p = r2(0.35) * q;
    float d = leafShape(p - vec2(0.0, -0.05), 2.0, 0.52);
    // serration
    float ang = p.y * 26.0;
    d += 0.018 * (0.5 + 0.5 * sin(ang)) * smoothstep(0.9, 0.2, abs(p.y));
    float st = ak_segment(p, vec2(0.0, -1.0), vec2(0.03, -1.25)) - 0.03;
    return min(d, st);
}
float sdBlob(vec2 q, float t) {
    float a = atan(q.y, q.x);
    return length(q * vec2(1.0, 0.92)) - 0.8 - 0.06 * sin(a * 3.0 + t * 2.0);
}
float sdOval(vec2 q) { return length(q * vec2(0.62, 1.0)) - 0.9; }

float shapeAt(vec2 q, float m, float t) {
    float d0 = sdBlob(q, t), d1 = sdPeach(q), d2 = sdBerg(q), d3 = sdTea(q), d4 = sdOval(q);
    float i = clamp(floor(m), 0.0, 3.0);
    float f = smoothstep(0.0, 1.0, clamp(m - i, 0.0, 1.0));
    float a = i < 0.5 ? d0 : (i < 1.5 ? d1 : (i < 2.5 ? d2 : d3));
    float b = i < 0.5 ? d1 : (i < 1.5 ? d2 : (i < 2.5 ? d3 : d4));
    return mix(a, b, f);
}

// interior detail lines (crease, pores, veins) — darker filaments inside the light
float detailAt(vec2 q, float m) {
    float w1 = clamp(1.0 - abs(m - 1.0), 0.0, 1.0);
    float w2 = clamp(1.0 - abs(m - 2.0), 0.0, 1.0);
    float w3 = clamp(1.0 - abs(m - 3.0), 0.0, 1.0);
    // peach crease
    float crease = 1.0 - smoothstep(0.02, 0.09, abs(q.x - 0.06 - 0.16 * q.y * q.y + 0.1 * q.y));
    crease *= smoothstep(0.85, 0.2, abs(q.y + 0.05));
    // bergamot pores
    vec3 v = ak_voronoi(q * 9.0);
    float pores = (1.0 - smoothstep(0.08, 0.2, v.x)) * smoothstep(0.85, 0.5, length(q));
    // tea veins
    vec2 p = r2(0.35) * q;
    float mid = 1.0 - smoothstep(0.0, 0.03, abs(p.x));
    float vs = abs(fract(p.y * 3.2 - abs(p.x) * 2.4) - 0.5);
    float vein = (1.0 - smoothstep(0.0, 0.06, vs)) * smoothstep(0.02, 0.06, abs(p.x)) * smoothstep(0.42, 0.1, abs(p.x));
    vein *= smoothstep(0.95, 0.4, abs(p.y));
    return 0.75 * w1 * crease + 0.45 * w2 * pores + 0.6 * w3 * max(mid, vein * 0.8);
}

// caustic network: strongly warped voronoi borders (curved bright filaments, darker cells)
float caus(vec2 p, float t) {
    vec2 w = p + 0.9 * vec2(ak_noise(p * 0.55 + vec2(t * 0.3, 0.0)) - 0.5, ak_noise(p * 0.55 + vec2(4.0, -t * 0.27)) - 0.5);
    w += 0.25 * vec2(ak_noise(w * 1.7 + 9.0), ak_noise(w * 1.7 - 3.0));
    float a = exp(-ak_voronoi_edge(w * 1.1 + vec2(0.0, t * 0.08)).x * 11.0);
    float b = exp(-ak_voronoi_edge(w * 2.0 + vec2(3.7, -t * 0.11)).x * 18.0);
    return clamp(a * 0.85 + b * 0.5, 0.0, 1.6);
}

vec2 toScreen(vec2 w) {
    vec2 uv = r2(-rot) * ((w - vec2(cx, cy)) * zoom);
    return uv * iResolution.y + 0.5 * iResolution.xy;
}

void mainImage(out vec4 o, in vec2 f) {
    float t = iTime;
    float L = slen;
    vec2 uv = (f - 0.5 * iResolution.xy) / iResolution.y;
    vec2 p = vec2(cx, cy) + r2(rot) * uv / zoom;
    float px = 1.0 / (iResolution.y * zoom);

    vec2 dir = normalize(vec2(-0.36, -1.0));   // light travel (down-left)
    vec2 nrm = vec2(-dir.y, dir.x);
    vec2 g = vec2(gx, gy);

    // ---------------- albedo ----------------
    vec3 alb = counter(p);
    float height = 0.0;

    // bag (matte black-tea paper), world rect
    vec2 bc = vec2(bx, by);
    vec2 bq = r2(0.035) * (p - bc);
    float bagD = ak_box(bq, vec2(0.235, 0.335));
    float inBag = bag * (1.0 - smoothstep(-px, px, bagD));
    if (bag > 0.0) {
        vec3 paper = vec3(0.040, 0.036, 0.034) * (0.90 + 0.2 * ak_fbm(bq * 70.0, 3));
        // soft crumple: shading from a low-frequency height field, lit from the sun side
        float hA = ak_fbm(bq * 6.0 + 2.0, 4), hB = ak_fbm(bq * 6.0 + 2.0 + vec2(0.01, 0.0), 4), hC = ak_fbm(bq * 6.0 + 2.0 + vec2(0.0, 0.01), 4);
        vec2 grad = vec2(hB - hA, hC - hA) / 0.01;
        paper *= 1.0 + 0.10 * dot(grad, -dir);
        // gusset edges: the bag's sides fold under
        float side = smoothstep(0.20, 0.235, abs(bq.x));
        paper *= 1.0 - 0.35 * side;
        paper += 0.05 * exp(-pow((abs(bq.x) - 0.205) / 0.004, 2.0)) * step(bq.y, 0.255);
        // crimp/fold at the top
        float fold = smoothstep(0.255, 0.26, bq.y) * (1.0 - smoothstep(0.33, 0.335, bq.y));
        paper *= 1.0 - 0.3 * fold * (0.5 + 0.5 * sin(bq.x * 420.0));
        paper += 0.04 * (1.0 - smoothstep(0.0, 0.004, abs(bq.y - 0.255)));
        // one-way degassing valve
        float vr = length(bq - vec2(0.16, 0.218));
        paper = mix(paper, vec3(0.07, 0.065, 0.06), 1.0 - smoothstep(0.020, 0.022, vr));
        paper += 0.06 * exp(-pow((vr - 0.013) / 0.002, 2.0));
        // label ink from channel (screen space)
        vec4 ink = texture(iChannel1, f / iResolution.xy);
        paper = mix(paper, ink.rgb / max(ink.a, 1e-3), ink.a * 0.96);
        alb = mix(alb, paper, inBag);
    }

    // ---------------- light ----------------
    // window: soft overall falloff of the sun patch + optional mullion bar
    float patch = smoothstep(1.6, 0.4, length((p - g) * vec2(0.8, 0.5)));
    float sl = sun * mix(0.55, 1.0, patch);
    float mull = abs(dot(p - vec2(barx, 0.0), nrm));
    sl *= 1.0 - bar * 0.9 * (1.0 - smoothstep(0.03, 0.07, mull));
    // leafy dapple at the far top
    float dap = ak_fbm(p * 7.0 + vec2(t * 0.05, 0.0), 4);
    sl *= 1.0 - 0.35 * smoothstep(0.55, 0.75, dap) * smoothstep(0.15, 0.6, p.y - g.y);

    // glass shadow: capsule along dir
    vec2 rel = p - g;
    float along = dot(rel, dir);
    float across = dot(rel, nrm);
    float wid = R * (1.0 + 0.10 * clamp(along / L, 0.0, 1.0));
    float capD = ak_segment(p, g, g + dir * L) - wid;
    float soft = 0.006 + 0.010 * clamp(along / L, 0.0, 1.0);
    float shad = 1.0 - smoothstep(-soft, soft, capD);
    // walls: darker edge bands, lighter empty top
    float wall = smoothstep(wid - 0.02, wid - 0.004, abs(across)) * step(0.0, along);
    float topLight = smoothstep(L * 0.82, L * 0.95, along);
    float shadow = shad * (1.0 - 0.25 * topLight) * (0.82 + 0.18 * (1.0 - wall));
    float bagShad = 0.0;
    if (bag > 0.0) {
        float bs = ak_box(r2(0.035) * (p - bc + dir * 0.012), vec2(0.235, 0.335));
        bagShad = bag * (1.0 - smoothstep(-0.004, 0.006, bs)) * (1.0 - inBag);
    }
    float lit = sl * (1.0 - 0.86 * max(shadow, bagShad * 0.9));

    // caustic
    vec2 cc = g + dir * (L * 0.60);
    vec2 jitter = slosh * 0.025 * vec2(sin(t * 6.1), cos(t * 5.3));
    vec2 q = p - cc - jitter;
    // into a frame aligned with the shadow, with projection stretch along dir
    float ca = atan(dir.y, dir.x) + 1.5708;          // so local +y points back to the glass
    vec2 lq = r2(-ca) * q;
    lq = r2(ca * 0.85) * lq;                         // mostly keep shapes upright on screen
    lq /= (0.165 * cscale);
    lq.y /= 1.08;
    // liquid unrest scrambles the shape
    vec2 wob = slosh * 0.35 * vec2(ak_noise(lq * 2.2 + t * 3.0) - 0.5, ak_noise(lq * 2.2 - t * 3.1 + 5.0) - 0.5);
    float sd = shapeAt(lq + wob, morph, t);
    float edge = 0.012 + 0.08 * slosh;
    float inside = 1.0 - smoothstep(-edge, edge, sd);
    float rim = exp(-abs(sd) * 14.0) * inside;
    float fil = caus(lq * 2.6 + vec2(0.0, t * 0.12), t * 0.55 + 23.0);
    float det = detailAt(lq, morph) * (1.0 - slosh);
    float cI = inside * (0.22 + 1.5 * fil + 0.8 * rim) * (1.0 - det);
    // the glass's own bright crescent at its foot
    float cres = exp(-pow((length(rel - dir * R * 0.65) - R * 0.95) / 0.007, 2.0)) * smoothstep(0.0, R, along);
    

    // light words (from channel 0), written by the same caustic
    float wm = texture(iChannel0, f / iResolution.xy).a * words;
    float wfil = caus(p * 30.0 + vec2(0.0, t * 0.2), t * 0.5 + 3.0);
    cI = max(cI, wm * (0.9 + 0.9 * wfil));

    cI *= sun * shad;                                  // caustic only exists in the shadow
    if (bag > 0.0) cI *= 1.0 + 0.6 * inBag;           // brighter on the label

    vec3 cCol = mix(AMB2, AMB1, clamp(0.0 + 0.7 * fil + 0.3 * rim, 0.0, 1.0));
    vec3 light = SUN * 2.0 * lit + SKY * 0.40 * (0.6 + 0.4 * sun) + cCol * 1.9 * cI;

    // contact occlusion around glass base and bag
    float gd = length(rel) - R;
    light *= 1.0 - 0.45 * exp(-max(gd, 0.0) * 120.0) * step(0.0, gd);

    vec3 col = alb * light;

    // ---------------- the glass, seen from above ----------------
    if (gd < 0.012) {
        float r = length(rel) / R;
        // liquid: deep red-brown filter coffee, clearer toward the wall
        vec3 brewDeep = vec3(0.16, 0.045, 0.012);
        vec3 brewEdge = vec3(0.55, 0.22, 0.06);
        vec3 liq = mix(brewDeep, brewEdge, smoothstep(0.55, 0.9, r));
        // heavy glass base seen through the brew: an inner ring
        float baseRing = exp(-pow((r - 0.80) / 0.03, 2.0));
        liq += vec3(0.5, 0.2, 0.05) * 0.35 * baseRing * sun;
        // sun shining through: warm glow on the far (down-left) side
        float through = smoothstep(0.1, 1.0, dot(rel / R, dir)) * smoothstep(0.98, 0.7, r);
        liq += vec3(0.85, 0.38, 0.08) * 0.45 * through * sun;
        // ripples
        float rip = sin(r * 42.0 - t * 10.0) * slosh * (0.4 + 0.6 * r);
        // reflection of the window (four panes) near the up-right side
        vec2 wp = rel / R - vec2(0.36, 0.40) + 0.04 * rip;
        wp = r2(0.35) * wp;
        float win = (1.0 - smoothstep(-0.03, 0.05, ak_box(wp, vec2(0.10, 0.13)))) * (0.6 + 0.4 * ak_noise(wp * 9.0 + t));
        float mullion = 0.7 * (1.0 - smoothstep(0.005, 0.03, abs(wp.x)));
        win *= 1.0 - clamp(mullion, 0.0, 1.0);
        float skyr = 0.10 * smoothstep(0.5, 1.0, r) + 0.04 * rip;
        liq += vec3(1.0, 0.96, 0.9) * win * 0.9 * (0.3 + 0.7 * sun) + SKY * skyr;
        // glass rim: bright lip, refractive band, wall thickness
        float lip = exp(-pow((r - 0.975) / 0.018, 2.0));
        float band = smoothstep(0.88, 0.94, r);
        vec3 wallC = counter(g + rel * 1.02) * (SUN * 1.2 + SKY * 0.3) * 0.75;
        vec3 gcol = mix(liq, wallC, band * 0.55) + vec3(1.0, 0.95, 0.88) * lip * (0.5 + 1.3 * smoothstep(-0.2, 0.9, dot(rel / R, -dir))) * (0.4 + 0.6 * sun);
        float cov = 1.0 - smoothstep(1.0 - 1.5 * px / R, 1.0 + 1.5 * px / R, r);
        col = mix(col, gcol, cov);
    }

    // grade
    vec2 sv = (f / iResolution.xy - 0.5);
    col *= 1.0 - 0.35 * dot(sv, sv) * 2.0;
    col = ak_tonemap(col * 0.62);
    col = ak_grain(col, f, 0.6);
    o = vec4(col, 1.0);
}
