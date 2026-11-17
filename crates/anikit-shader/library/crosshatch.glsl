// Crosshatch: pen or pencil hatching. Darker tones add more directions of
// lines; lines wobble like a hand drew them; outlines from the edges.
// A filter: {"shader": "crosshatch", "filter": true}.
// Uniforms: ink, paper, spacing, pencil, edge.
uniform vec3 ink;      // = vec3(0.12, 0.11, 0.14) line colour
uniform vec3 paper;    // = vec3(0.96, 0.94, 0.9) paper colour
uniform float spacing; // = 7.0 line spacing in pixels at 1080p
uniform float pencil;  // = 0.0 1 for soft grey pencil, 0 for ink
uniform float edge;    // = 1.0 outline strength

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

// A set of parallel lines at angle a: 1 on a line.
float hatch(vec2 p, float a, float sp, float w) {
    vec2 d = vec2(cos(a), sin(a));
    float t = dot(p, vec2(-d.y, d.x)) / sp + 0.15 * sin(dot(p, d) * 0.05 + a * 7.0);
    float f = abs(fract(t) - 0.5) * 2.0;
    return smoothstep(w + 0.18, w, f);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float s = iResolution.y / 1080.0;
    vec2 p = fragCoord / s;
    vec3 c = texture(iChannel0, fragCoord / iResolution.xy).rgb;
    float l = dot(c, vec3(0.299, 0.587, 0.114));
    float sp = spacing;
    float m = 0.0;
    m = max(m, hatch(p, 0.79, sp, 0.22) * step(l, 0.82));
    m = max(m, hatch(p, -0.79, sp, 0.22) * step(l, 0.6));
    m = max(m, hatch(p, 0.0, sp * 0.9, 0.24) * step(l, 0.4));
    m = max(m, hatch(p + 3.0, 1.57, sp * 0.8, 0.3) * step(l, 0.22));
    float e = length(vec2(dFdx(l), dFdy(l))) * 6.0 / s;
    m = max(m, smoothstep(0.3, 0.7, e * edge));
    float tex = h21(floor(p * 1.3));
    float strength = mix(1.0, 0.55 + 0.25 * tex, pencil);
    vec3 col = mix(paper, ink, m * strength);
    fragColor = vec4(col, 1.0);
}
