// Liquid chrome: a slow, heavy sculpture of merging metal drops, polished to
// a mirror and lit by a large studio (or a night sky, for compositing over a
// landscape). Smooth metaball field, analytic-quality normals, Fresnel metal
// with optional thin-film iridescence. With "alpha": true and background = 0
// the sculpture is cut out for compositing.
// Uniforms: speed, env (0 studio … 1 night sky), film (iridescence 0..1),
// background (1 draw the environment behind, 0 transparent), size.
uniform float speed;      // = 1.0
uniform float env;        // = 0.0
uniform float film;       // = 0.0
uniform float background; // = 1.0
uniform float size;       // = 1.0

// Smooth polynomial union: soft, wide necks between drops.
float smin(float a, float b, float k) {
    float h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * h * k * (1.0 / 6.0);
}

float map(vec3 p) {
    float t = iTime * speed * 0.35;
    p /= size;
    // A heavy liquid sphere whose surface carries slow, low-order waves…
    float w = 0.055 * sin(2.0 * p.x + 1.3 * t) * sin(2.0 * p.y - 0.9 * t + 1.0)
            + 0.04 * sin(3.0 * p.z + 1.1 * t + 2.0) * sin(2.5 * p.x - 0.7 * t);
    float d = length(p) - 0.95 - w;
    // …and three drops orbiting just above it, pulled in and released.
    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float a = t * (0.9 + 0.23 * fi) + fi * 2.094;
        // Tilted, equatorial orbits 120° apart: drops pass in front of and
        // behind the mass, and never pair up symmetrically.
        float r = 1.3 + 0.2 * sin(t * 1.1 + fi * 1.9);
        vec3 c = vec3(cos(a) * r, 0.0, sin(a) * r);
        c.y = 0.32 * c.x - 0.1 + 0.18 * sin(a * 2.0 + fi);
        d = smin(d, length(p - c) - (0.2 + 0.04 * fi), 0.42);
    }
    return d * size;
}

vec3 normal(vec3 p) {
    // Tetrahedral central differences: smooth, no axis bias.
    const vec2 e = vec2(1.0, -1.0) * 0.0015;
    return normalize(e.xyy * map(p + e.xyy) + e.yyx * map(p + e.yyx) + e.yxy * map(p + e.yxy) + e.xxx * map(p + e.xxx));
}

// Large, soft area lights: wide smooth falloffs, no hard-edged strips.
float box(vec3 d, vec3 axis, vec2 size, float soft) {
    float facing = dot(d, axis);
    vec3 u = normalize(cross(axis, abs(axis.y) > 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0)));
    vec3 v = cross(axis, u);
    vec2 q = abs(vec2(dot(d, u), dot(d, v))) / max(facing, 1e-3);
    vec2 s = smoothstep(size + soft, size - soft, q);
    return s.x * s.y * smoothstep(0.0, 0.2, facing);
}

vec3 studio(vec3 d) {
    // A white cyclorama sweep (warm floor, cool top) with black flags left and
    // right: chrome is all about the contrast edges between light and dark.
    vec3 c = mix(vec3(0.42, 0.39, 0.36), vec3(0.95, 0.97, 1.0), smoothstep(-0.5, 0.7, d.y));
    c *= 1.0 - 0.75 * exp(-abs(d.y - 0.05) * 7.0);                                                   // dark horizon seam
    c *= 1.0 - 0.95 * box(d, normalize(vec3(-1.0, 0.35, 0.3)), vec2(0.22, 0.5), 0.1);             // black flag, high left
    c *= 1.0 - 0.9 * box(d, normalize(vec3(0.7, -0.25, 0.75)), vec2(0.5, 0.18), 0.1);            // low black card, right
    c += vec3(7.0, 6.8, 6.4) * box(d, normalize(vec3(-0.3, 0.92, 0.2)), vec2(0.5, 0.28), 0.3);     // overhead key
    c += vec3(1.6, 2.4, 3.6) * box(d, normalize(vec3(1.0, 0.2, -0.3)), vec2(0.1, 0.8), 0.08);      // cool strip
    c += vec3(3.2, 2.1, 1.2) * box(d, normalize(vec3(-0.8, 0.05, -0.7)), vec2(0.14, 0.5), 0.12);   // warm kicker
    return c;
}

vec3 night(vec3 d) {
    // What the metal sees outdoors at night: dark teal sky with tall aurora
    // curtains (strongest behind the camera's left), a dark ridge, a black lake
    // carrying a soft copy of the light.
    vec3 c = mix(vec3(0.01, 0.02, 0.035), vec3(0.03, 0.05, 0.08), smoothstep(0.0, 0.8, d.y));
    float az = atan(d.x, -d.z);
    float curtains = 0.6 + 0.4 * sin(az * 7.0 + 1.3) * sin(az * 3.1 - 0.4);
    float band = smoothstep(0.04, 0.1, d.y) * smoothstep(0.42, 0.14, d.y) * curtains;
    c += vec3(0.12, 1.9, 0.85) * band * (0.55 + 0.45 * sin(az + 0.6));
    c += vec3(0.6, 0.22, 1.0) * 0.3 * smoothstep(0.3, 0.42, d.y) * smoothstep(0.65, 0.42, d.y) * curtains;
    c = mix(vec3(0.004, 0.007, 0.009), c, smoothstep(0.0, 0.03, d.y - 0.02 * sin(az * 5.0)));   // ridge
    vec3 lake = vec3(0.003, 0.008, 0.01) + vec3(0.06, 0.8, 0.4) * 0.25 * exp(d.y * 9.0) * curtains;
    return mix(lake, c, smoothstep(-0.01, 0.0, d.y));
}

vec3 environment(vec3 d) {
    return mix(studio(d), night(d), env);
}

// Thin-film interference tint for a view cosine.
vec3 thinfilm(float c) {
    float x = 2.4 * (1.0 - c) + 0.35;
    return 0.6 + 0.4 * cos(6.2831853 * (x * vec3(1.0, 1.12, 1.26) + vec3(0.0, 0.33, 0.67)));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    float t = iTime * speed;
    float ca = 0.3 * sin(t * 0.11);
    vec3 ro = vec3(4.4 * sin(ca), 0.35, 4.4 * cos(ca));
    vec3 ww = normalize(-ro), uu = normalize(cross(ww, vec3(0, 1, 0))), vv = cross(uu, ww);
    vec3 rd = normalize(uv.x * uu + uv.y * vv + 2.4 * ww);

    vec3 bgc = mix(vec3(0.16, 0.155, 0.15), vec3(0.36, 0.37, 0.39), smoothstep(-1.0, 1.0, uv.y));
    bgc *= 1.0 - 0.35 * dot(uv * 0.5, uv * 0.5);
    bgc = mix(bgc, night(rd) * 0.6, env);
    vec3 col = bgc;
    float alpha = background;
    float tt = 0.0;
    bool hit = false;
    for (int i = 0; i < 110; i++) {
        float d = map(ro + rd * tt);
        if (d < 0.0004 * tt) { hit = true; break; }
        tt += d;
        if (tt > 9.0) break;
    }
    if (hit) {
        vec3 p = ro + rd * tt;
        vec3 n = normal(p);
        vec3 r = reflect(rd, n);
        float c = clamp(dot(-rd, n), 0.0, 1.0);
        // Polished silver-chrome: high base reflectance, Schlick Fresnel to white.
        vec3 f0 = vec3(0.86, 0.87, 0.9);
        vec3 fre = f0 + (1.0 - f0) * pow(1.0 - c, 5.0);
        vec3 tint = mix(vec3(1.0), thinfilm(c), film);
        // One bounce: occlude reflections that point back into the sculpture.
        float occ = clamp(map(p + r * 0.25) / 0.25, 0.0, 1.0);
        col = environment(r) * fre * tint * (0.35 + 0.65 * occ);
        alpha = 1.0;
    } else if (background < 0.5) {
        col = vec3(0.0);   // cut out (use "supersample": true for smooth edges)
    }
    col = 1.0 - exp(-col * 1.1);     // filmic shoulder
    col = pow(col, vec3(0.4545));
    fragColor = vec4(col, alpha);
}
