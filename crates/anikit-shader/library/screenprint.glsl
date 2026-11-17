// Screenprint: flat poster colour. Tones collapse into a few ink layers,
// each laid slightly off register, with ink texture and a paper margin feel.
// A filter: {"shader": "screenprint", "filter": true}.
// Uniforms: inks, ink1, ink2, ink3, paper, offset, texture_.
uniform float inks;     // = 3.0 ink layers (1–3)
uniform vec3 ink1;      // = vec3(0.95, 0.75, 0.3) lightest layer
uniform vec3 ink2;      // = vec3(0.86, 0.28, 0.24) middle layer
uniform vec3 ink3;      // = vec3(0.12, 0.13, 0.22) darkest layer
uniform vec3 paper;     // = vec3(0.95, 0.93, 0.87) paper colour
uniform float offset;   // = 2.5 misregistration in pixels at 1080p
uniform float texture_; // = 1.0 ink texture amount

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float lum(vec2 uv) { return dot(texture(iChannel0, uv).rgb, vec3(0.299, 0.587, 0.114)); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float s = res.y / 1080.0;
    vec2 uv = fragCoord / res;
    vec2 o = vec2(offset * s) / res;
    float n = (h21(floor(fragCoord / (2.0 * s))) - 0.5) * 0.06 * texture_;
    // Each layer is a threshold of the tone, printed from its own offset screen.
    float l1 = lum(uv + o * vec2(1.0, 0.3));
    float l2 = lum(uv - o * vec2(0.4, 1.0));
    float l3 = l1;  // the key plate shares the first screen's registration
    float aa = 0.02;
    float a1 = smoothstep(0.82 + aa, 0.82 - aa, l1 + n);
    float a2 = inks > 1.5 ? smoothstep(0.55 + aa, 0.55 - aa, l2 + n) : 0.0;
    float a3 = inks > 2.5 ? smoothstep(0.28 + aa, 0.28 - aa, l3 + n) : 0.0;
    vec3 col = paper;
    col = mix(col, ink1 * (0.96 + n), a1);
    col = mix(col, ink2 * (0.96 + n), a2);
    col = mix(col, ink3 * (0.96 + n), a3);
    fragColor = vec4(col, 1.0);
}
