// Tern — the rehearsal room. One world for every shot: lime plaster wall,
// oak boards, a low sun through a four-pane window off-frame left, dust in
// the shaft. Sounds pass through the dust as rings; after `hold` they stop
// and hang (the recorder is listening). Type comes in through iChannel0:
// `ink` = letters on the window glass cast as shadow, `name` = letters cast
// as light.
uniform float zoom;   // = 1.0
uniform float camx;   // = 0.0
uniform float camy;   // = 0.0
uniform float sx;     // = -0.3  sound source
uniform float sy;     // = 0.1
uniform float w0;     // = -100.0 wave start times (scene s)
uniform float w1;     // = -100.0
uniform float w2;     // = -100.0
uniform float amp;    // = 1.0
uniform float decay;  // = 0.9
uniform float hold;   // = 1000.0 time the air stops
uniform float devx;   // = 0.0
uniform float devy;   // = -2.0
uniform float devs;   // = 0.0
uniform float devsun; // = 1.0
uniform float pip;    // = 0.0
uniform float ink;    // = 0.0
uniform float name;   // = 0.0
uniform float tscale; // = 1.0
uniform float warm;   // = 0.0
uniform float haze;   // = 1.0
uniform float sun;    // = 1.0
uniform float bird;   // = -100.0

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

const vec2 L = vec2(0.62, -0.40);   // screen shift of the sun per unit of depth
const vec2 C = vec2(0.16, 0.10);    // centre of the sun patch on the wall

// Window coordinates: the patch is a skewed parallelogram.
vec2 winUV(vec2 q) {
    vec2 w = q - C;
    w.x -= 0.42 * w.y;
    return w;
}
float textAt(vec2 w) {
    vec2 uv = vec2(0.5 + w.x / (tscale * iResolution.x / iResolution.y), 0.5 + w.y / tscale);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) return 0.0;
    vec4 c = texture(iChannel0, uv);
    return clamp(max(c.a * dot(c.rgb, vec3(0.333)), 0.0), 0.0, 1.0);
}
// Light arriving at wall point q (0..1): window panes, mullions, leaves, type.
float patchAt2(vec2 q, float soft, float useT) {
    vec2 w = winUV(q);
    vec2 hs = vec2(0.40, 0.34);
    float inWin = smoothstep(-soft, soft, hs.x - abs(w.x)) * smoothstep(-soft, soft, hs.y - abs(w.y - 0.0));
    float mull = smoothstep(0.012 - soft, 0.012 + soft, abs(w.x)) * smoothstep(0.012 - soft, 0.012 + soft, abs(w.y - 0.03));
    float tm = useT > 0.5 ? textAt(w) : 0.0;
    float win = inWin * mull * (1.0 - 0.92 * ink * tm);
    float lightType = tm;
    float open = useT > 0.5 ? mix(win, lightType, name) : win * (1.0 - 0.6 * name);
    // leaves outside break the light, drifting with real time
    float lv = vn(vec2(w.x * 7.0 + iTime * 0.10, w.y * 5.0 + 1.3 + iTime * 0.04)) * 0.6 + vn(vec2(w.x * 19.0 - iTime * 0.15, w.y * 13.0 + 4.0)) * 0.4;
    lv = smoothstep(0.30, 0.62, lv);
    open *= mix(1.0, 0.25 + 0.75 * lv, 0.55 * smoothstep(0.1, 0.4, -w.x + w.y * 0.6 + 0.35));
    // a bird crossing outside the window
    if (bird > -50.0) {
        float bt = iTime - bird;
        vec2 bp = vec2(-0.6 + bt * 0.9, 0.25 - bt * 0.12 + 0.02 * sin(bt * 30.0));
        vec2 d = w - bp;
        float flap = 0.5 + 0.5 * sin(bt * 28.0);
        float wing = abs(d.y + 0.03 * flap - 0.6 * abs(d.x) * (0.4 + flap * 0.6));
        float b = smoothstep(0.012, 0.004, wing) * smoothstep(0.08, 0.05, abs(d.x)) + smoothstep(0.018, 0.008, length(d * vec2(1.0, 1.6)));
        open *= 1.0 - clamp(b, 0.0, 1.0);
    }
    return open;
}

float patchAt(vec2 q, float soft) { return patchAt2(q, soft, 1.0); }

// Sound rings: field strength and outward direction at q, at (held) time te.
float ringsAt(vec2 q, float te, out vec2 dir) {
    vec2 d = q - vec2(sx, sy);
    float r = length(d);
    dir = d / max(r, 1e-4);
    float g = 0.0;
    for (int k = 0; k < 3; k++) {
        float tk = k == 0 ? w0 : (k == 1 ? w1 : w2);
        float dt = te - tk;
        if (dt <= 0.0) continue;
        float R = 0.62 * dt;
        float wid = 0.010 + 0.012 * dt;
        float env = exp(-dt * decay);
        for (int j = 0; j < 4; j++) {
            float x = (r - R + float(j) * 0.055) / wid;
            g += exp(-x * x) * env * (1.0 - float(j) * 0.22) * smoothstep(0.0, 0.05, R - float(j) * 0.055);
        }
    }
    return g * amp;
}

vec3 motes(vec2 q, float te, float zlo, float zhi, int i0, int i1, vec3 sunc) {
    vec3 acc = vec3(0.0);
    for (int i = i0; i < i1; i++) {
        float fi = float(i);
        float z = 0.2 + 0.32 * fi;
        vec2 qz = q + vec2(camx, camy) * z * 0.35;
        vec2 dir;
        float g = ringsAt(qz, te, dir);
        vec2 qs = qz - dir * g * 0.018;
        float sc = 95.0 - fi * 26.0;
        vec2 u = qs * sc + vec2(te * (0.25 + 0.1 * fi), -te * (0.35 + 0.12 * fi)) + fi * 13.7;
        vec2 c = floor(u);
        float h = h21(c + fi * 17.0);
        vec2 o = vec2(h21(c + 3.1), h21(c + 7.7)) * 0.6 + 0.2 + 0.12 * vec2(sin(te * 0.9 + h * 30.0), cos(te * 0.7 + h * 20.0));
        float rad = 0.07 + 0.05 * fi;
        float m = smoothstep(rad, rad * 0.25, length(fract(u) - o)) * step(0.55, h);
        float tw = 0.75 + 0.25 * sin(iTime * (3.0 + 4.0 * h) + h * 60.0);
        float lit = patchAt2(qz + L * z * 0.9, 0.02, 0.0) * sun;
        acc += m * tw * (sunc * lit * 1.6 + vec3(0.05, 0.06, 0.08)) * (1.0 + 7.0 * g) * (0.6 + 0.3 * fi);
    }
    return acc;
}

float sdRBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    vec2 p = (fragCoord - 0.5 * res) / res.y;
    vec2 q = p / zoom + vec2(camx, camy);
    float te = min(iTime, hold);
    vec3 sunc = mix(vec3(1.0, 0.80, 0.58), vec3(1.0, 0.66, 0.38), warm);
    vec3 fill = vec3(0.075, 0.088, 0.115);
    float floorY = -0.40;
    vec3 col;
    float wallSun = 0.0;
    if (q.y > floorY) {
        // plaster: trowel texture, bumped toward the sun
        float hgt = fbm(q * 9.0) * 0.6 + fbm(q * 31.0 + 3.0) * 0.4;
        float e = 0.004;
        vec2 grad = vec2(fbm((q + vec2(e, 0)) * 9.0) - fbm((q - vec2(e, 0)) * 9.0), fbm((q + vec2(0, e)) * 9.0) - fbm((q - vec2(0, e)) * 9.0));
        float bump = clamp(0.5 - dot(grad, normalize(L)) * 6.0, 0.0, 1.5);
        vec3 alb = vec3(0.80, 0.74, 0.65) * (0.86 + 0.18 * hgt);
        float soft = 0.004 + 0.010 * max(0.0, -winUV(q).x + 0.4);
        wallSun = patchAt(q, soft) * sun;
        // device shadow on the wall
        if (devs > 0.0) {
            float ds = sdRBox((q - L * 0.32 - vec2(devx, devy)) / devs, vec2(0.075, 0.2), 0.06) * devs;
            wallSun *= smoothstep(-0.004, 0.03, ds);
        }
        float amb = 0.55 + 0.45 * smoothstep(-0.9, 0.5, q.y) - 0.25 * smoothstep(0.5, 1.4, abs(q.x - 0.1));
        col = alb * (fill * amb + sunc * wallSun * 3.4 * mix(1.0, bump, 0.5));
        // warm bounce near the patch
        float near = exp(-length(winUV(q) * vec2(1.4, 1.8)) * 2.2);
        col += alb * sunc * near * 0.10 * sun;
        // skirting board
        float sk = smoothstep(floorY + 0.055, floorY + 0.05, q.y);
        col = mix(col, vec3(0.62, 0.58, 0.52) * (fill * 0.8 + sunc * 0.08) + vec3(0.06) * smoothstep(floorY + 0.045, floorY + 0.05, q.y), sk);
    } else {
        // oak boards in perspective, running toward the wall
        float dz = floorY - q.y;
        float persp = 1.0 / (dz + 0.12);
        float bx = q.x * persp * 0.11 + 0.37;
        float board = floor(bx * 3.0);
        float bf = fract(bx * 3.0);
        float seam = smoothstep(0.0, 0.025, bf) * smoothstep(1.0, 0.975, bf);
        float bz = persp * 0.4 + h21(vec2(board, 1.0)) * 7.0;
        float grain = fbm(vec2(bf * 3.0 + board * 5.0, bz * 9.0)) * 0.6 + 0.4 * vn(vec2(bf * 40.0, bz * 2.0 + board));
        vec3 alb = vec3(0.40, 0.25, 0.14) * (0.65 + 0.5 * grain) * (0.85 + 0.3 * h21(vec2(board, 3.0))) * seam;
        // sun patch on the floor: the window again, foreshortened
        vec2 fp = vec2(q.x - dz * 1.1 - 0.05, 0.30 - dz * 2.4);
        float fs = patchAt2(fp + C, 0.01 + dz * 0.03, 0.0) * sun * smoothstep(0.0, 0.03, dz) * (1.0 - name);
        float spec = pow(clamp(1.0 - abs(q.x - 0.1) * 1.2, 0.0, 1.0), 3.0) * 0.12;
        col = alb * (fill * 0.75 * (0.6 + 0.6 * smoothstep(0.6, 0.0, dz)) + sunc * fs * 2.2) + sunc * spec * fs;
        wallSun = fs;
    }
    // the shaft: light through the air, sampled along the ray toward the wall
    vec2 dir;
    float vol = 0.0;
    for (int k = 1; k <= 3; k++) {
        float z = float(k) / 3.0 - 0.12;
        vol += patchAt2(q + L * z * 0.9, 0.03 + z * 0.03, 0.0);
    }
    vol /= 3.0;
    float air = 0.6 + 0.4 * fbm(q * vec2(3.0, 2.0) + vec2(iTime * 0.03, -iTime * 0.05));
    float g = ringsAt(q, te, dir);
    col += sunc * vol * air * 0.12 * haze * sun * (1.0 + 2.5 * g);
    col += sunc * g * 0.010 * haze;

    col += motes(q, te, 0.0, 1.0, 0, 2, sunc) * haze;
    {
        vec2 u = q * 260.0 + vec2(te * 0.6, -te * 0.9);
        vec2 c = floor(u);
        float h = h21(c + 91.0);
        vec2 o = vec2(h21(c + 5.3), h21(c + 9.1)) * 0.6 + 0.2;
        float m = smoothstep(0.30, 0.05, length(fract(u) - o)) * step(1.0 - clamp(g * 0.9, 0.0, 0.85), h);
        float tw = 0.7 + 0.3 * sin(iTime * (4.0 + 5.0 * h) + h * 70.0);
        col += m * tw * (sunc * (0.35 + 2.2 * vol + 1.4 * wallSun) + vec3(0.10, 0.11, 0.13)) * 1.8 * haze;
    }

    // the recorder
    if (devs > 0.0) {
        vec2 b = (q - vec2(devx, devy)) / devs;
        float d = sdRBox(b, vec2(0.075, 0.2), 0.06);
        float aa = 1.5 / (res.y * zoom * devs);
        float cover = smoothstep(aa, -aa, d);
        if (cover > 0.0) {
            float e = clamp(-d / 0.05, 0.0, 1.0);
            float eps = 0.002;
            vec2 gr = vec2(sdRBox(b + vec2(eps, 0), vec2(0.075, 0.2), 0.06) - sdRBox(b - vec2(eps, 0), vec2(0.075, 0.2), 0.06),
                           sdRBox(b + vec2(0, eps), vec2(0.075, 0.2), 0.06) - sdRBox(b - vec2(0, eps), vec2(0.075, 0.2), 0.06)) / (2.0 * eps);
            float ez = sqrt(max(1.0 - (1.0 - e) * (1.0 - e), 0.0));
            vec3 n = normalize(vec3(gr * (1.0 - e) * 1.2, ez + 0.05));
            vec3 ls = normalize(vec3(-0.62, 0.40, 0.65));
            float dif = max(dot(n, ls), 0.0);
            vec3 hv = normalize(ls + vec3(0, 0, 1));
            float spc = pow(max(dot(n, hv), 0.0), 40.0);
            // grille on the top half: perforated metal
            float gz = smoothstep(0.035, 0.03, b.y);
            vec2 hp = b * vec2(1.0 / 0.016, 1.0 / 0.016);
            hp.x += 0.5 * mod(floor(hp.y), 2.0);
            float hole = smoothstep(0.30, 0.22, length(fract(hp) - 0.5)) * (1.0 - gz) * smoothstep(-0.003, -0.02, d);
            vec3 alb = mix(vec3(0.22, 0.22, 0.23), vec3(0.075, 0.075, 0.08), gz);
            alb *= 1.0 - 0.85 * hole;
            // a thin seam where grille meets body
            alb *= 1.0 - 0.6 * smoothstep(0.004, 0.0, abs(b.y - 0.035));
            float lit = devsun * sun;
            vec3 dc = alb * (fill * (0.5 + 0.5 * n.z) + sunc * dif * 2.2 * lit) + sunc * spc * (0.35 + 0.5 * (1.0 - gz)) * lit;
            // the record pip
            float pd = length(b - vec2(0.0, -0.11));
            vec3 red = vec3(0.90, 0.25, 0.16);
            dc = mix(dc, mix(vec3(0.12, 0.03, 0.02), red * 1.6, pip), smoothstep(0.013, 0.010, pd));
            col = mix(col, dc, cover);
            col += red * pip * 0.15 * exp(-pd * 40.0) * (1.0 - cover * 0.6);
        }
        float pd2 = length(b - vec2(0.0, -0.11));
        col += vec3(0.9, 0.25, 0.16) * pip * 0.05 * exp(-pd2 * 12.0);
    }
    col += motes(q, te, 0.0, 1.0, 2, 3, sunc) * haze;

    // grade: filmic shoulder, slight warm print, vignette, grain
    col *= 1.0 - 0.35 * smoothstep(0.45, 1.1, length(p * vec2(0.8, 1.0)));
    col = 1.0 - exp(-col * 1.25);
    col = pow(col, vec3(0.4545));
    col = mix(col, col * vec3(1.03, 1.0, 0.95), 0.6);
    col += (h21(fragCoord + float(iFrame) * 1.7) - 0.5) * 0.035;
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
