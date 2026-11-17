// Film: the look of a photochemical stock. A filmic tone curve with lifted
// blacks and rolled-off highlights, a warm/cool split, red halation around
// bright edges, a gentle gate weave and grain that is strongest in the mids.
// A filter: {"shader": "film", "filter": true}.
// Uniforms: stock, halation, grain, weave, fade.
uniform float stock;    // = 0.0 0 warm negative, 1 cool slide film
uniform float halation; // = 1.0 red glow around highlights
uniform float grain;    // = 1.0 grain amount
uniform float weave;    // = 1.0 gate weave (frame wobble)
uniform float fade;     // = 0.3 lifted, milky blacks

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float s = res.y / 1080.0;
    float f = float(iFrame);
    // Gate weave: a slow sub-pixel wander, different every frame but smooth.
    vec2 wv = vec2(sin(f * 0.37) + 0.5 * sin(f * 1.13), cos(f * 0.29) + 0.4 * sin(f * 0.91)) * 0.6 * weave * s;
    vec2 uv = (fragCoord + wv) / res;
    vec3 c = texture(iChannel0, uv).rgb;
    // Halation: highlights bleed red through the base, sampled to the sides.
    vec2 o = vec2(6.0, 4.0) * s / res;
    vec3 hb = texture(iChannel0, uv + o).rgb;
    float hl = max(dot(hb, vec3(0.3, 0.6, 0.1)) - 0.7, 0.0);
    c += vec3(1.0, 0.25, 0.08) * hl * 0.8 * halation;
    // Stock: split toning and a filmic S-curve.
    vec3 warm = vec3(1.04, 1.0, 0.92), cool = vec3(0.94, 1.0, 1.06);
    float l = dot(c, vec3(0.299, 0.587, 0.114));
    c = mix(c * mix(warm, cool, stock), vec3(l), 0.08);
    c = c * c * (3.0 - 2.0 * c) * 0.65 + c * 0.35;
    c = mix(c, vec3(0.06, 0.055, 0.07) + c * 0.94, fade);
    // Grain: zero-mean, strongest in the midtones, finer than a pixel block.
    float n = (h21(fragCoord + fract(f * 0.6180339) * 113.0) - 0.5) * 1.4;
    c += n * 0.06 * grain * (l * (1.0 - l) * 3.0 + 0.1);
    fragColor = vec4(clamp(c, 0.0, 1.0), 1.0);
}
