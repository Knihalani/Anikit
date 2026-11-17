// Linocut: the frame carved into a block and printed: solid ink where it's
// dark, gouge marks following the tones, bold outlines, on rough paper.
// A filter: {"shader": "linocut", "filter": true}.
// Uniforms: ink, paper, threshold, lines, edge.
uniform vec3 ink;        // = vec3(0.08, 0.07, 0.09) ink colour
uniform vec3 paper;      // = vec3(0.93, 0.89, 0.8) paper colour
uniform float threshold; // = 0.5 tone where carving starts
uniform float lines;     // = 1.0 gouge line density
uniform float edge;      // = 1.0 outline strength

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float s = iResolution.y / 1080.0;
    float l = dot(texture(iChannel0, uv).rgb, vec3(0.299, 0.587, 0.114));
    // Outlines from the luma gradient (quad derivatives: no extra taps).
    float e = length(vec2(dFdx(l), dFdy(l))) * 6.0 / s;
    // Gouges: wavy parallel cuts whose width follows the tone.
    vec2 p = fragCoord / s;
    float wave = sin(p.y * 0.012 + sin(p.x * 0.004) * 2.0) * 18.0;
    float stripe = abs(fract((p.x * 0.35 + p.y + wave) * 0.06 * lines) - 0.5) * 2.0;
    float carve = smoothstep(threshold - 0.25, threshold + 0.35, l);
    float inked = step(carve, stripe * 0.95 + 0.02);
    inked = max(inked, smoothstep(0.25, 0.6, e * edge));
    // Paper tooth and uneven ink.
    float tooth = h21(floor(p * 0.9)) * 0.12;
    vec3 col = mix(paper * (0.97 + tooth * 0.3), ink + tooth * 0.08, inked * (0.9 + tooth));
    fragColor = vec4(col, 1.0);
}
