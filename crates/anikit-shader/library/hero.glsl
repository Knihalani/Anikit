// Hero object: one product-shot object on a glossy studio floor, turning
// slowly under big soft lights. Pick a shape, a colour and a finish.
// Raymarched inside a bounding sphere; analytic floor with a soft contact
// shadow and a blurred reflection of the object.
// Uniforms: shape, color, metal, rough, speed, size, background, floor.
uniform float shape;      // = 0.0 0 torus, 1 rounded cube, 2 pill, 3 gem, 4 sphere, 5 twisted bar
uniform vec3 color;       // = vec3(0.9, 0.35, 0.18) base colour
uniform float metal;      // = 0.0 0 plastic/ceramic … 1 metal
uniform float rough;      // = 0.25 0 mirror … 1 matte
uniform float speed;      // = 1.0 turntable speed
uniform float size;       // = 1.0 object scale
uniform vec3 background;  // = vec3(0.07, 0.07, 0.08) backdrop colour
uniform float floor_;     // = 1.0 1 glossy floor, 0 object floats on the backdrop

const float R = 1.25;     // bounding radius of every shape (before size)

mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }

float sdObj(vec3 p) {
    if (shape < 0.5) {                       // torus, tilted to show its hole
        p.yz *= rot(0.85);
        vec2 q = vec2(length(p.xz) - 0.78, p.y);
        return length(q) - 0.3;
    } else if (shape < 1.5) {                // rounded cube
        vec3 q = abs(p) - vec3(0.62);
        return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - 0.14;
    } else if (shape < 2.5) {                // pill
        vec3 q = p; q.y -= clamp(q.y, -0.55, 0.55);
        return length(q) - 0.45;
    } else if (shape < 3.5) {                // gem: octahedron with a table
        vec3 q = abs(p);
        float o = (q.x + q.y + q.z - 1.05) * 0.57735;
        return max(o, p.y - 0.55);
    } else if (shape < 4.5) {                // sphere
        return length(p) - 0.85;
    }
    vec3 q = p;                              // twisted bar
    q.xz *= rot(q.y * 1.4);
    vec3 b = abs(q) - vec3(0.3, 0.95, 0.3);
    return (length(max(b, 0.0)) + min(max(b.x, max(b.y, b.z)), 0.0) - 0.06) * 0.7;
}

float t_;
float map(vec3 p) {
    p /= size;
    p.xz *= rot(t_ * 0.45);
    p.yz *= rot(0.35);
    return sdObj(p) * size;
}

vec3 calcNormal(vec3 p) {
    const vec2 e = vec2(1.0, -1.0) * 0.0012;
    return normalize(e.xyy * map(p + e.xyy) + e.yyx * map(p + e.yyx) + e.yxy * map(p + e.yxy) + e.xxx * map(p + e.xxx));
}

// A rectangular softbox around `axis`: half-size `sz` (tangent units), soft edge.
float box(vec3 d, vec3 axis, vec2 sz, float soft) {
    float facing = dot(d, axis);
    vec3 u = normalize(cross(axis, abs(axis.y) > 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0)));
    vec3 v = cross(axis, u);
    vec2 q = abs(vec2(dot(d, u), dot(d, v))) / max(facing, 1e-3);
    vec2 s = smoothstep(sz + soft, sz - soft, q);
    return s.x * s.y * step(0.0, facing);
}

// Studio: a backdrop sweep with a bright horizon, a big overhead softbox, a warm
// key at left, a tall cool strip at right, a low fill card. Flat metal faces
// always find a light or a clean gradient to reflect.
vec3 studio(vec3 d) {
    vec3 c = background * (0.5 + 0.9 * smoothstep(-0.4, 0.6, d.y));
    c += background * 1.6 * exp(-abs(d.y + 0.02) * 9.0);                                        // horizon glow
    c += vec3(4.5) * box(d, vec3(0.0, 1.0, 0.0), vec2(0.7, 0.45), 0.2);                       // overhead
    c += vec3(3.4, 3.0, 2.5) * box(d, normalize(vec3(-0.9, 0.35, 0.5)), vec2(0.45, 0.6), 0.15);  // key
    c += vec3(1.8, 2.4, 3.2) * box(d, normalize(vec3(1.0, 0.1, -0.35)), vec2(0.08, 0.9), 0.05);  // rim strip
    c += vec3(0.9, 0.85, 0.8) * box(d, normalize(vec3(0.3, -0.15, 1.0)), vec2(0.8, 0.12), 0.1);  // fill card
    c += vec3(0.75, 0.73, 0.7) * smoothstep(0.1, 1.0, d.z) * smoothstep(-0.7, 0.5, d.y);      // big bounce behind the camera
    return c;
}
// The same, pre-blurred: what a rough surface reflects, and diffuse light.
vec3 studioBlur(vec3 d) {
    vec3 c = background * (0.5 + 0.9 * smoothstep(-0.4, 0.6, d.y));
    c += vec3(1.5) * smoothstep(0.1, 1.0, d.y);
    c += vec3(1.3, 1.15, 0.95) * smoothstep(0.3, 1.0, dot(d, normalize(vec3(-0.9, 0.35, 0.5))));
    c += vec3(0.5, 0.65, 0.9) * smoothstep(0.5, 1.0, dot(d, normalize(vec3(1.0, 0.1, -0.35))));
    c += vec3(0.25) * smoothstep(0.4, 1.0, dot(d, normalize(vec3(0.3, -0.15, 1.0))));
    c += vec3(0.6, 0.58, 0.56) * smoothstep(0.1, 1.0, d.z) * smoothstep(-0.7, 0.5, d.y);
    return c;
}

// Ray against the bounding sphere: (enter, exit), or exit < 0 on a miss.
vec2 bound(vec3 ro, vec3 rd) {
    float b = dot(ro, rd), c = dot(ro, ro) - R * R * size * size;
    float h = b * b - c;
    if (h < 0.0) return vec2(0.0, -1.0);
    h = sqrt(h);
    return vec2(max(-b - h, 0.0), -b - h < 0.0 && -b + h < 0.0 ? -1.0 : -b + h);
}

float march(vec3 ro, vec3 rd, int n) {
    vec2 bb = bound(ro, rd);
    if (bb.y < 0.0) return -1.0;
    float t = bb.x;
    for (int i = 0; i < n; i++) {
        float d = map(ro + rd * t);
        if (d < 0.0005 * (1.0 + t)) return t;
        t += d;
        if (t > bb.y) break;
    }
    return -1.0;
}

vec3 shadeObj(vec3 p, vec3 rd) {
    vec3 n = calcNormal(p);
    vec3 r = reflect(rd, n);
    float c = clamp(dot(-rd, n), 0.0, 1.0);
    vec3 f0 = mix(vec3(0.04), color, metal);
    vec3 fre = f0 + (1.0 - f0) * pow(1.0 - c, 5.0) * (1.0 - 0.7 * rough);
    vec3 spec = mix(studio(r), studioBlur(r), smoothstep(0.0, 0.6, rough));
    vec3 diff = studioBlur(n) * 0.55 * color * (1.0 - metal);
    // The floor darkens what faces down (a cheap, stable occlusion).
    float occ = mix(1.0, 0.45 + 0.55 * smoothstep(-1.0, 0.3, n.y), floor_);
    return (diff + spec * fre) * occ;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    t_ = iTime * speed;
    vec3 ro = vec3(0.0, 0.75, 4.6);
    vec3 ww = normalize(vec3(0.0, -0.05, 0.0) - ro), uu = normalize(cross(ww, vec3(0, 1, 0))), vv = cross(uu, ww);
    vec3 rd = normalize(uv.x * uu + uv.y * vv + 2.3 * ww);

    // Backdrop: a soft gradient sweep with a vignette.
    vec3 col = background * (1.15 - 0.35 * length(uv * vec2(0.6, 0.9)));
    float fy = -1.15 * size;                  // floor height under the object
    float t = march(ro, rd, 96);
    float tf = rd.y < 0.0 ? (fy - ro.y) / rd.y : 1e9;
    if (t > 0.0 && t < tf) {
        col = shadeObj(ro + rd * t, rd);
    } else if (floor_ > 0.5 && tf < 1e8) {
        vec3 p = ro + rd * tf;
        // Contact shadow: the object's distance seen from the floor, softened.
        float d = map(vec3(p.x, fy + 0.35 * size, p.z));
        float sh = smoothstep(0.0, 0.9 * size, d) * 0.75 + 0.25;
        float far = smoothstep(9.0, 3.0, length(p.xz));
        vec3 base = background * mix(0.55, 1.05, sh);
        // Glossy reflection of the object, fading with height above the floor.
        vec3 rr = reflect(rd, vec3(0.0, 1.0, 0.0));
        float tr = march(p + rr * 0.002, rr, 48);
        vec3 refl = tr > 0.0 ? shadeObj(p + rr * tr, rr) * exp(-tr * 1.6) : vec3(0.0);
        float fre = 0.06 + 0.5 * pow(1.0 - max(-rd.y, 0.0), 5.0);
        col = mix(col, base + refl * (0.35 + fre), far);
    }
    col = 1.0 - exp(-col * 1.15);
    col *= 1.0 - 0.15 * dot(uv * 0.5, uv * 0.5);
    fragColor = vec4(pow(col, vec3(0.4545)), 1.0);
}
