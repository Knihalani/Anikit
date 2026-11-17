// FOLD: a glass sculpture folding through itself.
// A kaleidoscopic fold of a rounded slab: each iteration mirrors space and turns it by
// an angle that drifts with time, so the form refolds continuously. Shaded as thin glass:
// fresnel reflection of a soft studio, thin-film interference for the iridescence, a
// cheap two-sided transmission tint, deep green-black backdrop.
uniform float fold;    // = 1.0   overall fold amount (0 = a plain slab)
uniform float spin;    // = 1.0
uniform float film;    // = 1.0   thin-film strength

mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, s, -s, c); }

float sdBox(vec3 p, vec3 b, float r) {
    vec3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

float map(vec3 p) {
    float t = iTime;
    p.xz *= rot(0.35 * t * spin);
    p.xy *= rot(0.21 * t * spin + 0.4);
    float s = 1.0;
    for (int i = 0; i < 4; i++) {
        p = abs(p);
        float a = fold * (0.55 + 0.35 * sin(t * 0.45 + float(i) * 1.7));
        p.xy *= rot(a);
        p.yz *= rot(a * 0.7 + 0.3);
        p -= vec3(0.36, 0.14, 0.06) * fold;
        p *= 1.18;
        s *= 1.18;
    }
    float d = sdBox(p, vec3(0.9, 0.06, 0.62), 0.04) / s;
    return d;
}

vec3 normal(vec3 p) {
    const vec2 e = vec2(1.0, -1.0) * 0.0009;
    return normalize(e.xyy * map(p + e.xyy) + e.yyx * map(p + e.yyx) + e.yxy * map(p + e.yxy) + e.xxx * map(p + e.xxx));
}

// soft studio for reflections: a big overhead softbox, two side strips, dark floor
vec3 studio(vec3 rd) {
    vec3 c = mix(vec3(0.006, 0.02, 0.016), vec3(0.03, 0.09, 0.07), clamp(rd.y * 0.5 + 0.5, 0.0, 1.0));
    c += vec3(2.4, 2.5, 2.4) * smoothstep(0.86, 0.9, rd.y);
    c += vec3(1.6, 1.8, 1.7) * smoothstep(0.9, 0.93, abs(rd.x)) * smoothstep(-0.2, 0.3, rd.y);
    c += vec3(1.2) * smoothstep(0.985, 0.995, abs(rd.z)) * smoothstep(0.1, 0.2, rd.y) * step(abs(rd.x), 0.15);
    c += vec3(0.4, 0.9, 0.8) * 0.4 * smoothstep(0.6, 1.0, -rd.z) * smoothstep(0.0, 0.4, rd.y);
    return c;
}

// thin-film interference: thickness varies over the surface and with time
vec3 thinfilm(float cosT, float thick) {
    vec3 w = vec3(1.0, 1.18, 1.38);
    vec3 c = 0.5 + 0.5 * cos(6.2831 * (thick * cosT * w * 2.2 + vec3(0.0, 0.33, 0.67)));
    return 0.55 + 0.6 * c * c;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    float t = iTime;
    vec3 ro = vec3(0.0, 0.25, 4.6 - 0.3 * sin(t * 0.2));
    vec3 ta = vec3(0.0, 0.0, 0.0);
    vec3 ww = normalize(ta - ro), uu = normalize(cross(ww, vec3(0, 1, 0))), vv = cross(uu, ww);
    vec3 rd = normalize(uv.x * uu + uv.y * vv + 2.1 * ww);

    vec3 bg = mix(vec3(0.004, 0.016, 0.012), vec3(0.02, 0.07, 0.055), smoothstep(1.3, 0.0, length(uv)));
    vec3 col = bg;
    float d = 0.0;
    bool hit = false;
    for (int i = 0; i < 96; i++) {
        float h = map(ro + rd * d);
        if (h < 0.0008 * d) { hit = true; break; }
        d += h;
        if (d > 8.0) break;
    }
    if (hit) {
        vec3 p = ro + rd * d;
        vec3 n = normal(p);
        float cosT = clamp(dot(-rd, n), 0.0, 1.0);
        float fre = 0.04 + 0.96 * pow(1.0 - cosT, 5.0);
        vec3 refl = studio(reflect(rd, n));
        float thick = 0.6 + 0.4 * sin(p.x * 3.1 + p.y * 2.3 + t * 0.6) + 0.25 * sin(p.z * 7.0 - t);
        vec3 irid = mix(vec3(1.0), thinfilm(cosT, thick), film);
        // transmission: what's behind, tinted by the glass and its thickness along the view
        vec3 trans = studio(refract(rd, n, 0.67)) * vec3(0.35, 0.7, 0.6) * 0.18;
        col = mix(trans, refl * irid * 1.6, clamp(fre * 1.4 + 0.08, 0.0, 1.0));
        col += irid * irid * 0.05 * (0.5 + 0.5 * n.y);
        // a crisp rim
        col += vec3(0.7, 1.0, 0.9) * pow(1.0 - cosT, 8.0) * 0.6;
    }
    // vignette, gentle tone map, gamma
    col *= 1.0 - 0.35 * dot(uv * 0.55, uv * 0.55);
    col = col / (1.0 + col * 0.6);
    col = pow(max(col, 0.0), vec3(0.4545));
    fragColor = vec4(col, 1.0);
}
