// Tidewater: a phone on a white studio cove, sunlight through water dappling the floor.
// iChannel0 = the phone's screen (a 2D composition, landscape).
uniform float orbit;  // = 0.0  camera orbit (radians)
uniform float push;   // = 0.0  dolly (0..1)
uniform float lift;   // = 0.0  phone lift off the floor (0..1, settles to 0)

float sdRoundBox(vec3 p, vec3 b, float r) { vec3 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r; }
float smin(float a, float b, float k) { float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0); return mix(b, a, h) - k * h * (1.0 - h); }

const vec3 PH = vec3(0.0, 0.055, 0.0);       // phone centre (half height 0.055)
const vec3 HB = vec3(1.62, 0.05, 0.86);      // half extents: landscape phone
const float YAW = -0.32;

vec3 toPhone(vec3 p) {
    p -= PH + vec3(0.0, lift * 0.35, 0.0);
    float c = cos(YAW), s = sin(YAW);
    return vec3(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
}
float phone(vec3 p) { return sdRoundBox(toPhone(p), HB, 0.045); }
float room(vec3 p) { return smin(p.y, 9.0 - p.z, 2.5); }
float map(vec3 p, out int id) {
    float a = phone(p), b = room(p);
    id = a < b ? 1 : 0;
    return min(a, b);
}
vec3 normal(vec3 p) {
    int id; vec2 e = vec2(0.0008, 0.0);
    return normalize(vec3(map(p + e.xyy, id) - map(p - e.xyy, id), map(p + e.yxy, id) - map(p - e.yxy, id), map(p + e.yyx, id) - map(p - e.yyx, id)));
}
float shadow(vec3 ro, vec3 rd) {
    float res = 1.0, t = 0.02;
    for (int i = 0; i < 40; i++) {
        float h = phone(ro + rd * t);
        res = min(res, 22.0 * h / t);
        t += clamp(h, 0.02, 0.3);
        if (res < 0.002 || t > 6.0) break;
    }
    return clamp(res, 0.0, 1.0);
}
float ao(vec3 p, vec3 n) {
    float o = 0.0, s = 1.0; int id;
    for (int i = 1; i < 5; i++) { float h = 0.04 * float(i); o += (h - map(p + n * h, id)) * s; s *= 0.6; }
    return clamp(1.0 - 2.2 * o, 0.0, 1.0);
}
// Light through a rippling water surface.
float caustic(vec2 uv) {
    const float TAU = 6.28318530718;
    vec2 p = mod(uv * TAU, TAU) - 250.0;
    vec2 i = p; float c = 1.0; float inten = 0.005;
    for (int n = 0; n < 5; n++) {
        float t = iTime * 0.3 * (1.0 - 3.5 / float(n + 1));
        i = p + vec2(cos(t - i.x) + sin(t + i.y), sin(t - i.y) + cos(t + i.x));
        c += 1.0 / length(vec2(p.x / (sin(i.x + t) / inten), p.y / (cos(i.y + t) / inten)));
    }
    c /= 5.0;
    c = 1.17 - pow(c, 1.4);
    return clamp(pow(abs(c), 8.0), 0.0, 1.5);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float a = 0.35 + orbit;
    float dist = 9.6 - 1.1 * push;
    vec3 ta = vec3(-1.15, -0.35, 0.9);
    vec3 ro = ta + vec3(sin(a) * dist * 0.62, dist * 0.62, -cos(a) * dist * 0.78);
    vec3 fw = normalize(ta - ro), rt = normalize(cross(vec3(0, 1, 0), fw)), up = cross(fw, rt);
    vec3 rd = normalize(fw * 2.1 + uv.x * rt + uv.y * up);

    vec3 sun = normalize(vec3(-0.8, 0.48, 0.38));
    float t = 0.0; int id = 0;
    for (int i = 0; i < 110; i++) {
        float d = map(ro + rd * t, id);
        if (d < 0.0004 * t) break;
        t += d;
        if (t > 30.0) break;
    }
    vec3 p = ro + rd * t, n = normal(p);
    float sh = shadow(p + n * 0.003, sun);
    float occ = ao(p, n);
    float dif = max(dot(n, sun), 0.0);
    vec3 col;
    if (id == 1) {
        vec3 q = toPhone(p);
        vec3 qn = toPhone(p + n) - q;
        bool top = qn.y > 0.9;
        vec2 suv = vec2(q.x / (HB.x - 0.07), q.z / (HB.z - 0.07)) * 0.5 + 0.5;
        bool onScreen = top && all(greaterThan(suv, vec2(0.0))) && all(lessThan(suv, vec2(1.0)));
        vec3 body = vec3(0.17, 0.18, 0.2);
        float fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, -rd), 0.0), 5.0);
        vec3 env = mix(vec3(0.9, 0.91, 0.93), vec3(1.0), clamp(reflect(rd, n).y, 0.0, 1.0));
        if (onScreen) {
            vec3 scr = texture(iChannel0, vec2(suv.x, suv.y)).rgb;
            float sheen = caustic(p.xz * 0.3 + 0.37) * 0.18;
            col = scr * 0.93 + env * fres * 0.22 + sheen;
        } else {
            vec3 h = normalize(sun - rd);
            float spec = pow(max(dot(n, h), 0.0), 60.0) * sh;
            col = body * (0.25 + 0.75 * dif * sh) * occ + env * fres * 0.5 + spec * 0.6;
            if (top) col = mix(col, vec3(0.03, 0.035, 0.04) + env * fres * 0.4, 0.85);
        }
    } else {
        vec3 alb = vec3(0.94, 0.945, 0.95);
        float cs = pow(caustic(p.xz * 0.3 + vec2(0.0, p.y * 0.1)), 1.5) * 1.6;
        // A pool of sunlight through water, falling around the phone; the rest of the cove is in soft shade.
        float pool = 1.0 - smoothstep(1.2, 3.4, length((p.xz - vec2(0.5, 0.2)) * vec2(0.62, 1.0)));
        float sunLit = dif * sh * pool;
        vec3 light = vec3(1.0, 0.975, 0.94) * sunLit * (0.45 + 1.4 * cs) + vec3(0.9, 0.92, 0.95) * (0.9 + 0.1 * n.z) * occ;
        col = alb * light;
        // Contact darkening right under the phone.
        col *= mix(0.55, 1.0, smoothstep(0.0, 0.35, phone(p)));
    }
    // Distance haze into the white cove.
    col = mix(col, vec3(0.93, 0.94, 0.95), 1.0 - exp(-0.0009 * t * t));
    col *= 1.1;
    col = col / (1.0 + 0.12 * col);
    fragColor = vec4(pow(col, vec3(0.94)), 1.0);
}
