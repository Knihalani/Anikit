// Raymarched SDF still life: smooth-blended forms on a studio floor,
// soft shadows, ambient occlusion, fog. Uniforms: speed, hue.
uniform float speed; // = 1.0
uniform vec3 tint;   // = vec3(1.0, 0.55, 0.25)

float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

float sdTorus(vec3 p, vec2 t) { vec2 q = vec2(length(p.xz) - t.x, p.y); return length(q) - t.y; }
float sdBox(vec3 p, vec3 b, float r) { vec3 q = abs(p) - b; return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r; }

mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, s, -s, c); }

// x: distance, y: material (0 floor, 1 blob, 2 ring, 3 box)
vec2 map(vec3 p) {
    float t = iTime * speed;
    vec2 res = vec2(p.y + 1.0, 0.0);
    vec3 q = p - vec3(0.0, 0.1 + 0.15 * sin(t * 1.3), 0.0);
    float blob = length(q) - 0.75;
    blob = smin(blob, length(q - vec3(0.55 * sin(t), 0.45, 0.55 * cos(t))) - 0.38, 0.35);
    blob = smin(blob, length(q - vec3(-0.6 * cos(t * 0.7), -0.25, 0.5 * sin(t * 0.7))) - 0.3, 0.3);
    if (blob < res.x) res = vec2(blob, 1.0);
    vec3 r = p - vec3(1.9, -0.35, 0.4);
    r.xy *= rot(0.6 + 0.3 * sin(t * 0.5));
    r.yz *= rot(t * 0.4);
    float ring = sdTorus(r, vec2(0.55, 0.12));
    if (ring < res.x) res = vec2(ring, 2.0);
    vec3 b = p - vec3(-1.9, -0.45, 0.6);
    b.xz *= rot(t * 0.3 + 0.5);
    float box = sdBox(b, vec3(0.45), 0.08);
    if (box < res.x) res = vec2(box, 3.0);
    return res;
}

vec3 normal(vec3 p) {
    const vec2 e = vec2(1.0, -1.0) * 0.0007;
    return normalize(e.xyy * map(p + e.xyy).x + e.yyx * map(p + e.yyx).x + e.yxy * map(p + e.yxy).x + e.xxx * map(p + e.xxx).x);
}

float softShadow(vec3 ro, vec3 rd) {
    float res = 1.0, t = 0.02;
    for (int i = 0; i < 48; i++) {
        float h = map(ro + rd * t).x;
        res = min(res, 10.0 * h / t);
        t += clamp(h, 0.02, 0.25);
        if (res < 0.002 || t > 8.0) break;
    }
    return clamp(res, 0.0, 1.0);
}

float ao(vec3 p, vec3 n) {
    float occ = 0.0, sca = 1.0;
    for (int i = 0; i < 5; i++) {
        float h = 0.02 + 0.12 * float(i);
        occ += (h - map(p + n * h).x) * sca;
        sca *= 0.9;
    }
    return clamp(1.0 - 2.5 * occ, 0.0, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    float a = 0.4 + 0.15 * iTime * speed;
    vec3 ro = vec3(5.2 * sin(a), 1.4, 5.2 * cos(a));
    vec3 ta = vec3(0.0, -0.1, 0.0);
    vec3 ww = normalize(ta - ro), uu = normalize(cross(ww, vec3(0, 1, 0))), vv = cross(uu, ww);
    vec3 rd = normalize(uv.x * uu + uv.y * vv + 1.8 * ww);

    vec3 sky = mix(vec3(0.62, 0.68, 0.78), vec3(0.18, 0.22, 0.32), clamp(rd.y * 1.6 + 0.3, 0.0, 1.0));
    vec3 col = sky;
    float t = 0.0;
    vec2 h = vec2(0.0);
    bool hit = false;
    for (int i = 0; i < 128; i++) {
        h = map(ro + rd * t);
        if (h.x < 0.0004 * t) { hit = true; break; }
        t += h.x;
        if (t > 30.0) break;
    }
    if (hit) {
        vec3 p = ro + rd * t;
        vec3 n = normal(p);
        vec3 alb = h.y < 0.5 ? vec3(0.55) * (0.85 + 0.15 * mod(floor(p.x) + floor(p.z), 2.0))
                 : h.y < 1.5 ? tint
                 : h.y < 2.5 ? vec3(0.9, 0.92, 0.95) : vec3(0.2, 0.45, 0.85);
        vec3 l = normalize(vec3(0.6, 0.8, 0.4));
        float dif = max(dot(n, l), 0.0) * softShadow(p + n * 0.002, l);
        float occ = ao(p, n);
        float spe = pow(max(dot(reflect(rd, n), l), 0.0), h.y > 1.5 && h.y < 2.5 ? 64.0 : 16.0);
        float fre = pow(clamp(1.0 + dot(n, rd), 0.0, 1.0), 3.0);
        col = alb * (dif * vec3(1.25, 1.1, 0.95) + occ * (0.25 + 0.15 * n.y) * vec3(0.6, 0.7, 0.9));
        col += spe * dif * 0.6 + fre * occ * sky * 0.25;
        col = mix(col, sky, 1.0 - exp(-0.002 * t * t * t * 0.4));
    }
    col = pow(col, vec3(0.4545));
    col *= 1.0 - 0.25 * dot(uv * 0.5, uv * 0.5);
    fragColor = vec4(col, 1.0);
}
