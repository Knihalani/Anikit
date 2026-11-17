// Tidewater: a phone on a white studio cove, sunlight through water dappling the floor.
// iChannel0 = the phone's screen (a 2D composition, landscape).
uniform float orbit;  // = 0.0  camera orbit (radians)
uniform float push;   // = 0.0  dolly (0..1)
uniform float lift;   // = 0.0  phone lift off the floor (0..1, settles to 0)

float sdRoundBox(vec3 p, vec3 b, float r) { vec3 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r; }
float smin(float a, float b, float k) { float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0); return mix(b, a, h) - k * h * (1.0 - h); }

const vec3 PH = vec3(0.0, 0.042, 0.0);       // phone centre (lies on the floor)
const vec3 HB = vec3(1.62, 0.042, 0.75);     // half extents: a 19.5:9 phone in landscape
const float CORNER = 0.24;                   // plan-view corner radius
const float EDGE = 0.036;                    // rounded side band
const vec2 SCR = vec2(1.565, 0.695);         // screen half extents (thin bezel)
const float YAW = -0.32;

vec3 toPhone(vec3 p) {
    p -= PH + vec3(0.0, lift * 0.35, 0.0);
    float c = cos(YAW), s = sin(YAW);
    return vec3(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
}
float sdRoundRect(vec2 p, vec2 b, float r) { vec2 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r; }
float phone(vec3 p) {
    vec3 q = toPhone(p);
    float d2 = sdRoundRect(q.xz, HB.xz, CORNER);
    vec2 w = vec2(d2 + EDGE, abs(q.y) - HB.y + EDGE);
    return min(max(w.x, w.y), 0.0) + length(max(w, 0.0)) - EDGE;
}
// The studio as a reflection: white cove above, the sunlit floor below, the sun itself.
vec3 env(vec3 r, vec3 sun) {
    vec3 c = r.y > 0.0 ? mix(vec3(0.86, 0.88, 0.91), vec3(1.02), smoothstep(0.0, 0.6, r.y)) : vec3(0.78, 0.79, 0.81);
    c += vec3(1.0, 0.97, 0.9) * pow(max(dot(r, sun), 0.0), 350.0) * 14.0;
    // A softbox strip high in the cove gives the long edge its highlight.
    c += vec3(0.6) * smoothstep(0.92, 0.99, dot(r, normalize(vec3(0.2, 0.75, 0.62))));
    return c;
}
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
        vec3 qn = normalize(toPhone(p + n) - q);
        vec3 r = reflect(rd, n);
        float ndv = max(dot(n, -rd), 0.0);
        bool top = qn.y > 0.97;
        if (top) {
            float sd = sdRoundRect(q.xz, SCR, CORNER - 0.05);
            // Screen content lives in the composition's middle band (keeps 16:9 type unstretched).
            vec2 suv = vec2(q.x / SCR.x, q.z / SCR.y) * 0.5 + 0.5;
            vec2 tuv = vec2(suv.x, 0.5 + (suv.y - 0.5) * (SCR.y / SCR.x) * (1920.0 / 1080.0));
            vec3 scr = texture(iChannel0, tuv).rgb;
            vec3 glass = mix(vec3(0.012, 0.013, 0.015), scr * 0.92, smoothstep(0.004, -0.004, sd));
            float fres = 0.04 + 0.96 * pow(1.0 - ndv, 5.0);
            // Sunlight through water lands on the glass too.
            float pool = 1.0 - smoothstep(1.2, 3.4, length((p.xz - vec2(0.5, 0.2)) * vec2(0.62, 1.0)));
            float sheen = pow(caustic(p.xz * 0.3 + vec2(0.0, 0.004)), 1.5) * 0.5 * pool * sh;
            col = glass + env(r, sun) * fres * 0.9 + sheen;
        } else {
            // Bead-blasted aluminium band: tinted reflection, a crisp sun glint, darker underneath.
            vec3 metal = vec3(0.78, 0.79, 0.81);
            float f = 0.65 + 0.35 * pow(1.0 - ndv, 5.0);
            vec3 h = normalize(sun - rd);
            float glint = pow(max(dot(n, h), 0.0), 140.0) * 5.0 * sh;
            col = metal * env(r, sun) * f * mix(0.35, 1.0, smoothstep(-0.6, 0.4, qn.y)) * occ + glint;
            // A thin antenna line, as on real phones.
            col *= 1.0 - 0.5 * (1.0 - smoothstep(0.004, 0.008, abs(abs(q.x) - 1.2)));
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
