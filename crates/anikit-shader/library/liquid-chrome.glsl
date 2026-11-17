// Liquid chrome: metaballs raymarched as polished metal reflecting a
// procedural studio (softboxes, horizon, coloured bounce). Uniforms: speed, warmth.
uniform float speed;  // = 1.0
uniform float warmth; // = 0.5

float smin(float a, float b, float k) {
    float h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

float map(vec3 p) {
    float t = iTime * speed * 0.6;
    float d = 1e9;
    for (int i = 0; i < 6; i++) {
        float fi = float(i);
        vec3 c = vec3(sin(t * (0.7 + 0.13 * fi) + fi * 1.7) * 1.3,
                      cos(t * (0.9 + 0.11 * fi) + fi * 2.3) * 0.75,
                      sin(t * (0.5 + 0.07 * fi) + fi * 0.9) * 0.6);
        d = smin(d, length(p - c) - (0.55 + 0.12 * sin(fi * 3.1)), 0.7);
    }
    return d;
}

vec3 normal(vec3 p) {
    const vec2 e = vec2(0.0008, 0.0);
    return normalize(vec3(map(p + e.xyy) - map(p - e.xyy), map(p + e.yxy) - map(p - e.yxy), map(p + e.yyx) - map(p - e.yyx)));
}

// The studio the metal reflects: black void, a bright horizon band, sharp
// softboxes. Chrome is all contrast, so the environment is too.
vec3 env(vec3 d) {
    vec3 warm = mix(vec3(0.95, 0.97, 1.0), vec3(1.0, 0.78, 0.52), warmth);
    float hor = exp(-abs(d.y) * 9.0);
    vec3 col = mix(vec3(0.015), vec3(0.22, 0.23, 0.26), smoothstep(-0.2, 0.9, d.y));
    col += warm * 1.4 * hor;                                                                       // horizon
    col += warm * 5.0 * smoothstep(0.02, 0.0, abs(d.y - 0.62) - 0.07) * smoothstep(0.75, 0.6, abs(d.x)); // overhead strip
    col += vec3(0.45, 0.7, 1.0) * 3.5 * smoothstep(0.015, 0.0, abs(d.x - 0.78) - 0.04) * smoothstep(-0.3, 0.1, d.y); // cool side box
    col += warm * 3.0 * smoothstep(0.02, 0.0, abs(d.x + 0.7) - 0.02) * smoothstep(0.0, 0.3, d.y) * smoothstep(0.9, 0.5, d.y);
    col *= 0.25 + 0.75 * smoothstep(-0.7, -0.05, d.y);                                             // dark floor
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    vec3 ro = vec3(0.0, 0.0, 4.2);
    vec3 rd = normalize(vec3(uv, -1.7));
    vec3 bg = mix(vec3(0.02, 0.02, 0.03), vec3(0.09, 0.09, 0.12), uv.y * 0.5 + 0.5);
    vec3 col = bg;
    float t = 0.0;
    for (int i = 0; i < 96; i++) {
        float d = map(ro + rd * t);
        if (d < 0.0005) {
            vec3 p = ro + rd * t;
            vec3 n = normal(p);
            vec3 r = reflect(rd, n);
            float fre = pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
            vec3 metal = vec3(0.9, 0.9, 0.92);
            col = env(r) * mix(metal, vec3(1.0), fre);
            break;
        }
        t += d;
        if (t > 10.0) break;
    }
    col = 1.0 - exp(-col * 1.4);          // tone map
    col = pow(col, vec3(0.4545));
    fragColor = vec4(col, 1.0);
}
