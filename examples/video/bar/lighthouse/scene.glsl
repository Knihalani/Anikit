// The Lighthouse Keeper: a lighthouse on a rock at night, a sweeping beam in sea fog.
uniform float push;   // = 0.0   camera dolly in (0..1)
uniform float lamp;   // = 1.0   lamp power (flickers out at the end)
uniform float sweep;  // = 0.0   beam angle offset (radians)

const vec3 LAMP = vec3(2.1, 3.55, 6.0);

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}
float smin(float a, float b, float k) { float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0); return mix(b, a, h) - k * h * (1.0 - h); }

float lanternSdf(vec3 p) { vec3 q = p - vec3(2.1, 0.0, 6.0); return max(length(q.xz) - 0.2, abs(q.y - 3.55) - 0.25); }

// Tapered tower, gallery, lantern and cap, on a lumpy rock.
float tower(vec3 p) {
    vec3 q = p - vec3(2.1, 0.0, 6.0);
    float h = clamp(q.y, 0.0, 3.2);
    float r = mix(0.42, 0.27, h / 3.2);
    float body = max(length(q.xz) - r, max(-q.y + 0.2, q.y - 3.2));
    float gallery = max(length(q.xz) - 0.4, abs(q.y - 3.25) - 0.05);
    float lantern = max(length(q.xz) - 0.2, abs(q.y - 3.55) - 0.25);
    float cap = max(length(q.xz) - 0.28 * clamp((4.05 - q.y) / 0.25, 0.0, 1.0), abs(q.y - 3.92) - 0.13);
    return min(min(body, gallery), min(lantern, cap));
}
float rock(vec3 p) {
    vec3 q = p - vec3(2.1, -0.2, 6.0);
    float d = length(q * vec3(0.55, 1.4, 0.7)) - 0.9;
    d = smin(d, length((q - vec3(-1.1, -0.1, 0.4)) * vec3(0.8, 1.6, 0.9)) - 0.6, 0.5);
    d = smin(d, length((q - vec3(0.9, -0.2, -0.3)) * vec3(0.9, 1.8, 0.9)) - 0.55, 0.4);
    return d + 0.16 * vnoise(p.xz * 2.5 + p.y * 3.0) + 0.07 * vnoise(p.xz * 7.0 - p.y * 5.0);
}
float map(vec3 p, out int id) {
    float t = tower(p), r = rock(p);
    id = t < r ? 1 : 2;
    return min(t, r);
}
vec3 normal(vec3 p) {
    int id; vec2 e = vec2(0.002, 0.0);
    return normalize(vec3(map(p + e.xyy, id) - map(p - e.xyy, id), map(p + e.yxy, id) - map(p - e.yxy, id), map(p + e.yyx, id) - map(p - e.yyx, id)));
}

// The beam: a half-line from the lamp, turning about y. Glow from the ray's closest approach.
vec3 beamDir() { float a = -0.62 * iTime + sweep + 3.47; return normalize(vec3(cos(a), -0.22, -sin(a))); }
float beam(vec3 ro, vec3 rd, float tmax) {
    vec3 b = beamDir();
    vec3 w = ro - LAMP;
    float bb = dot(rd, b);
    float den = max(1.0 - bb * bb, 1e-4);
    float t = clamp((bb * dot(w, b) - dot(w, rd)) / den, 0.0, tmax);
    vec3 p = ro + rd * t;
    float s = dot(p - LAMP, b);
    float d = length(p - LAMP - b * s);
    float width = 0.12 + 0.07 * max(s, 0.0);
    float g = exp(-d * d / (width * width)) * smoothstep(-1.6, 1.2, s) * exp(-0.045 * max(s, 0.0));
    float core = exp(-dot(p - LAMP, p - LAMP) * 6.0);
    return g * 0.9 * lamp;
}

vec3 sky(vec3 rd, float withMoon) {
    float y = max(rd.y, 0.0);
    vec3 c = mix(vec3(0.16, 0.165, 0.17), vec3(0.025, 0.028, 0.034), pow(y, 0.5));
    vec2 md = rd.xy / max(rd.z, 0.1) - vec2(-0.12, 0.25);
    float moon = withMoon * smoothstep(0.024, 0.021, length(md)) * (0.85 + 0.15 * vnoise(md * 60.0));
    float cl = vnoise(rd.xy * vec2(2.5, 9.0) + vec2(iTime * 0.015, 0.0)) * vnoise(rd.xy * vec2(5.0, 15.0) - vec2(iTime * 0.01, 0.0));
    c += vec3(0.07, 0.072, 0.075) * cl * smoothstep(0.0, 0.25, y);
    float halo = exp(-length(md) * 5.0);
    return c + vec3(1.0, 0.98, 0.92) * moon * 0.0 + vec3(0.34, 0.35, 0.37) * halo * 0.9 + vec3(0.2) * exp(-length(md) * 18.0) * withMoon;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    vec3 ro = vec3(-1.4, 0.75, -4.6 + 1.4 * push);
    vec3 ta = vec3(0.9, 1.55, 6.0);
    vec3 fw = normalize(ta - ro), rt = normalize(cross(vec3(0, 1, 0), fw)), up = cross(fw, rt);
    vec3 rd = normalize(fw * 1.55 + uv.x * rt + uv.y * up);

    vec3 moonDir = normalize(vec3(-0.12, 0.25, 1.0));
    vec3 col = sky(rd, 1.0);
    float tmax = 40.0;
    // Sea plane with small swells.
    float tSea = rd.y < 0.0 ? (-ro.y) / rd.y : 1e9;
    float t = 0.0; int id = 0; bool hit = false;
    for (int i = 0; i < 90; i++) {
        vec3 p = ro + rd * t;
        float d = map(p, id);
        if (d < 0.0015 * t) { hit = true; break; }
        t += d * 0.9;
        if (t > min(tmax, tSea)) break;
    }
    float tHit = hit ? t : (tSea < 1e8 ? tSea : tmax);
    if (hit) {
        vec3 p = ro + rd * t, n = normal(p);
        vec3 alb = id == 1 ? vec3(0.62, 0.6, 0.56) : vec3(0.08, 0.075, 0.07);
        if (id == 1) {
            // Painted bands and the lit lantern glass.
            float y = p.y;
            alb *= mix(1.0, 0.18, step(0.5, fract(y * 0.9 + 0.25)) * step(y, 3.1));
        }
        float dif = max(dot(n, moonDir), 0.0) * 0.9;
        vec3 l = LAMP - p; float ll = length(l);
        float lampLit = max(dot(n, l / ll), 0.0) / (1.0 + ll * ll * 1.5) * 2.2 * lamp;
        float rim = pow(1.0 - max(dot(n, -rd), 0.0), 3.0) * 0.25;
        col = alb * (vec3(0.6, 0.65, 0.75) * dif + vec3(1.0, 0.82, 0.5) * lampLit) + rim * vec3(0.5, 0.52, 0.55) * 0.3;
        if (id == 1 && lanternSdf(p) < 0.04) {
            // Glass with mullions, lit from inside.
            float frame = smoothstep(0.012, 0.02, abs(p.y - 3.55));
            col = vec3(1.0, 0.86, 0.55) * (1.6 * lamp + 0.04) * mix(0.45, 1.0, frame);
        }
    } else if (tSea < 1e8) {
        vec3 p = ro + rd * tSea;
        vec2 q = p.xz;
        float h = 0.0;
        vec3 n = normalize(vec3(0.06 * sin(q.x * 3.1 + iTime * 1.3) + 0.04 * sin(q.x * 7.0 - q.y * 4.0 + iTime * 2.0) + 0.09 * (vnoise(q * 4.0 + iTime * 0.3) - 0.5) + 0.05 * (vnoise(q * 11.0 - iTime * 0.5) - 0.5),
                                1.0,
                                0.05 * sin(q.y * 2.3 - iTime) + 0.05 * (vnoise(q * 2.7 - iTime * 0.2) - 0.5)));
        vec3 r = reflect(rd, n);
        float fres = 0.02 + 0.98 * pow(1.0 - max(dot(n, -rd), 0.0), 5.0);
        vec3 refl = sky(r, 0.0) * 0.75 + vec3(1.0, 0.85, 0.55) * beam(p, r, 30.0) * 0.6;
        float moonGlint = pow(max(dot(r, normalize(vec3(-0.55, 0.32, 1.0))), 0.0), 120.0) * 2.5;
        refl += vec3(0.85, 0.87, 0.9) * moonGlint;
        // The lamp's glitter path on the water.
        vec3 lv = normalize(LAMP - p);
        float spec = pow(max(dot(r, lv), 0.0), 220.0) * 6.0 * lamp;
        float near = exp(-tSea * 0.12);
        float sparkle = pow(vnoise(q * vec2(2.5, 26.0) + vec2(iTime * 0.3, iTime * 1.6)) * vnoise(q * vec2(6.0, 41.0) - vec2(0.0, iTime * 1.1)), 3.0) * 4.0;
        float glitter = pow(max(dot(r, lv), 0.0), 30.0) * sparkle * 3.0 * lamp * near;
        vec2 mq = rd.xy / max(rd.z, 0.1) - vec2(-0.12, 0.0);
        glitter += exp(-abs(mq.x) * 30.0) * sparkle * 1.6 * near;
        col = vec3(0.004, 0.005, 0.006) + refl * fres + vec3(1.0, 0.82, 0.5) * (spec + glitter);
        col *= 1.0 + h;
    }
    // Sea fog: thicker low and far; lit by the beam.
    float bank = 0.35 + 1.3 * vnoise(vec2(rd.x * 2.2 + iTime * 0.07, rd.y * 7.0)) * vnoise(vec2(rd.x * 5.0 - iTime * 0.04, rd.y * 14.0 + 3.0));
    float fogAmt = (1.0 - exp(-0.06 * tHit)) * clamp(bank, 0.0, 1.2);
    vec3 fogCol = vec3(0.13, 0.135, 0.14);
    col = mix(col, fogCol, fogAmt * 0.85);
    col += vec3(1.0, 0.86, 0.58) * beam(ro, rd, tHit) * 0.85;
    // Glare when the beam swings across the lens.
    vec3 toL = normalize(LAMP - ro);
    float ang = 1.0 - max(dot(rd, toL), 0.0);
    float align = pow(max(dot(beamDir(), -toL), 0.0), 24.0) * lamp;
    col += vec3(1.0, 0.9, 0.7) * align * (3.0 * exp(-ang * 3000.0) + 0.5 * exp(-ang * 150.0) + 0.12 * exp(-ang * 12.0));
    // Filmic tone map.
    col = col / (1.0 + col);
    col = pow(col, vec3(0.88));
    fragColor = vec4(col, 1.0);
}
