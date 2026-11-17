// Swell as a ridgeline plot: 56 stacked lines, front to back, each lifted by
// peaked travelling waves; nearer lines hide farther ones (filled below).
uniform float amp;    // = 1.0 swell height
uniform float order;  // = 1.0 0 chaotic wind chop … 1 organised swell
uniform float steep;  // = 0.0 sharpen crests (breaking)
float hgt(float x, float k, float t) {
    float ph = k * 0.42;
    float w1 = exp(sin(x * 7.0 - t * 2.2 + ph) - 1.0);
    w1 = pow(w1, 1.0 + 3.0 * steep);
    float w2 = exp(sin(x * 11.0 + t * 1.3 - ph * 1.7 + 2.0) - 1.0);
    float n = sin(x * 31.0 + k * 3.1 + t * 4.0) * sin(x * 17.0 - k * 1.3 - t * 2.7);
    float env = exp(-pow((x - 0.5) * 2.3, 2.0));
    return env * (mix(0.5 * w2 + 0.35 * n, w1, order)) * 0.085 * amp;
}
void mainImage(out vec4 o, in vec2 f) {
    vec2 uv = f / iResolution.xy;
    float t = iTime;
    float aspect = iResolution.x / iResolution.y;
    float px = 1.0 / iResolution.y;
    vec3 bg = mix(vec3(0.018, 0.035, 0.065), vec3(0.03, 0.06, 0.1), uv.y);
    vec3 col = bg;
    float done = 0.0;
    for (int i = 0; i < 56; i++) {
        float k = float(i);
        float base = 0.09 + k * 0.0135;
        float x = (uv.x - 0.5) * (1.0 - k * 0.006) + 0.5;
        float y = base + hgt(x, k, t) * (1.0 - k * 0.008);
        float d = uv.y - y;
        float line = smoothstep(1.6 * px, 0.4 * px, abs(d));
        float fade = 1.0 - k / 62.0;
        vec3 lc = mix(vec3(0.55, 0.66, 0.76), vec3(0.93, 0.95, 0.97), fade);
        float crest = smoothstep(0.03, 0.08, y - base);
        lc = mix(lc, vec3(1.0, 0.45, 0.26), crest * order);
        if (done < 0.5) {
            col = mix(col, lc * (0.35 + 0.65 * fade), line);
            if (d < 0.0) { done = 1.0; }
        }
    }
    float vig = 1.0 - 0.35 * pow(length((uv - 0.5) * vec2(aspect * 0.6, 1.0)), 2.0);
    o = vec4(col * vig, 1.0);
}
