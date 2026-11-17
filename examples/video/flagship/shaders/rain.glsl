// Rain, Swiss style: thin ultramarine streaks falling at a slight angle on off-white,
// in depth layers (far = short, faint, slow), with small ripples where drops land.
uniform float density; // = 1.0
uniform float ground;  // = 0.86  y (0 top .. 1 bottom) where ripples appear; > 1 hides them

float hash(float n) { return fract(sin(n * 91.345) * 47453.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    uv.y = 1.0 - uv.y;                                  // 0 at the top
    float aspect = iResolution.x / iResolution.y;
    vec3 paper = vec3(0.98, 0.98, 0.968);
    vec3 blue = vec3(0.122, 0.231, 1.0);
    float ink = 0.0;
    for (int layer = 0; layer < 3; layer++) {
        float fl = float(layer);
        float cols = 70.0 + 50.0 * fl;
        float speed = 0.9 + 0.6 * fl;
        float len = 0.035 + 0.03 * fl;
        float wgt = 0.25 + 0.3 * fl;
        vec2 p = uv;
        p.x += p.y * 0.08;                               // slant
        float c = floor(p.x * cols);
        float r = hash(c + fl * 17.0);
        if (r > 0.55 * density) continue;
        float y = fract(p.y * (0.8 + 0.4 * r) - iTime * speed * (0.8 + 0.4 * r) + r * 13.0);
        float x = fract(p.x * cols) - 0.5;
        float streak = smoothstep(len, 0.0, y) * smoothstep(0.0, 0.006, y);
        float w = smoothstep(0.09 + 0.04 * fl, 0.0, abs(x));
        ink += streak * w * wgt;
    }
    // ripples on the ground line
    float rip = 0.0;
    for (int k = 0; k < 9; k++) {
        float fk = float(k);
        float period = 1.1 + hash(fk) * 0.9;
        float ph = fract(iTime / period + hash(fk + 3.0));
        vec2 c = vec2(0.08 + 0.84 * hash(fk + floor(iTime / period) * 7.1), ground + 0.02 * hash(fk + 9.0));
        vec2 d = (uv - c) * vec2(aspect, aspect * 3.2);
        float ring = abs(length(d) - ph * 0.12);
        rip += smoothstep(0.004, 0.0, ring) * (1.0 - ph) * 0.8;
    }
    ink = clamp(ink + rip, 0.0, 1.0);
    vec3 col = mix(paper, blue, ink * 0.9);
    fragColor = vec4(col, 1.0);
}
