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

// Bounding radius of everything `map` can produce (before `size`).
const float BOUND = 2.15;

float map(vec3 p) {
    float t = iTime * speed * 0.35;
    p /= size;
    // A heavy liquid mass whose surface wobbles in a few low modes…
    float d = length(p) - 0.95;
    // The wobble is bounded by 0.127: only evaluate it near the surface.
    if (d < 0.25) {
        d -= 0.06 * sin(2.1 * p.x + 1.3 * t) * sin(1.9 * p.y - 0.9 * t + 1.0)
           + 0.045 * sin(3.0 * p.z + 1.1 * t + 2.0) * sin(2.6 * p.x - 0.7 * t)
           + 0.022 * sin(5.3 * p.y + 2.0 * t) * sin(4.7 * p.z - 1.7 * t);
    }
    // …and drops that fall into it, neck, merge and tear away again: each one's
    // orbit radius swings from inside the mass to well outside it.
    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float a = t * (0.55 + 0.17 * fi) + fi * 2.094;
        float r = 1.18 + 0.5 * sin(t * 0.62 + fi * 2.3);
        vec3 c = vec3(cos(a) * r, 0.0, sin(a) * r);
        c.y = 0.3 * c.x - 0.08 + 0.2 * sin(a * 2.0 + fi);
        // Stretch each drop along its path into a teardrop while it moves.
        vec3 q = p - c;
        vec3 tang = vec3(-sin(a), 0.0, cos(a));
        q -= tang * dot(q, tang) * 0.28;
        float rad = 0.25 + 0.04 * sin(t * 1.7 + fi * 2.0);
        d = smin(d, length(q) - rad, 0.55);
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
    // What the metal sees outdoors: near-black sky (stars would shimmer once
    // shrunk to sub-pixel size in the reflection), the aurora as a
    // sharp bright core over a soft glow (a crisp key highlight in the metal),
    // a dark ridge, and the lake mirroring the core.
    float az = atan(d.x, -d.z);
    float y = abs(d.y);
    vec3 c = mix(vec3(0.004, 0.008, 0.016), vec3(0.012, 0.022, 0.045), smoothstep(0.0, 0.9, y));
    float curtains = 0.55 + 0.45 * sin(az * 7.0 + 1.3) * sin(az * 3.0 - 0.4);   // integer frequencies: no seam at the azimuth wrap
    float lift = 0.24 + 0.04 * sin(az * 5.0) + 0.02 * sin(az * 13.0);
    // Below the horizon (the lake) the core is softer: the water isn't a perfect mirror.
    float sharp = d.y >= 0.0 ? 30.0 : 11.0;
    float core = exp(-pow((y - lift) * sharp, 2.0)) * curtains;
    float glow = exp(-pow((y - lift - 0.03) * 7.0, 2.0)) * (0.7 + 0.3 * curtains);
    vec3 green = vec3(0.12, 1.0, 0.55);
    c += green * (5.0 * core + 0.8 * glow);
    c += vec3(0.55, 0.2, 1.0) * 0.18 * exp(-pow((y - 0.5) * 4.0, 2.0));
    float ridge = 0.025 + 0.012 * sin(az * 4.0 + 1.0) + 0.006 * sin(az * 11.0);
    c = mix(vec3(0.003, 0.005, 0.007), c, smoothstep(ridge - 0.008, ridge + 0.008, y));
    // Below the horizon the lake: same sky, mirrored and darkened.
    return d.y >= 0.0 ? c : c * 0.22 + vec3(0.0, 0.002, 0.004);
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
    // Rays that miss the bounding sphere skip the march entirely.
    float bR = BOUND * size;
    float bb = dot(ro, rd);
    float disc = bb * bb - dot(ro, ro) + bR * bR;
    int steps = disc > 0.0 ? 110 : 0;
    tt = disc > 0.0 ? max(-bb - sqrt(disc), 0.0) : 0.0;
    float tExit = disc > 0.0 ? -bb + sqrt(disc) : 0.0;   // nothing to hit beyond the bound
    for (int i = 0; i < steps; i++) {
        float d = map(ro + rd * tt);
        if (d < 0.0004 * tt) { hit = true; break; }
        tt += d;
        if (tt > tExit) break;
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
