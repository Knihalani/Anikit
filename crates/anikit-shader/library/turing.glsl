// Turing patterns: the labyrinths and spots of reaction–diffusion. A
// reaction–diffusion system settles on one wavelength; summing many plane
// waves of that wavelength (random directions and phases) and thresholding
// gives the same look without a simulation, identical at any frame.
// Uniforms: scale, balance (−1 spots of colorA … 0 maze … 1 spots of colorB),
// speed (slow drift), colours.
uniform float scale;   // = 1.0  pattern scale
uniform float balance; // = 0.0  −1 spots … 0 maze … 1 holes
uniform float speed;   // = 1.0  drift speed
uniform vec3 colorA;   // = vec3(0.06, 0.07, 0.10)  background colour
uniform vec3 colorB;   // = vec3(0.98, 0.72, 0.30)  pattern colour

float h1(float n) { return fract(sin(n * 91.345) * 47453.5453); }

float field(vec2 p, float t) {
    float f = 0.0;
    for (int i = 0; i < 14; i++) {
        float fi = float(i);
        float a = 3.14159265 * (fi + 0.5 * h1(fi)) / 14.0;
        vec2 d = vec2(cos(a), sin(a));
        f += cos(dot(p, d) + 6.2831853 * h1(fi + 17.0) + t * (h1(fi + 31.0) - 0.5));
    }
    return f / sqrt(14.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float t = iTime * speed * 0.3;
    vec2 p = uv * 60.0 * scale;
    // Slow warp so the maze bends like a grown pattern, not a crystal.
    p += 4.0 * vec2(sin(uv.y * 3.1 + 1.0) + sin(uv.x * 1.7 + t * 0.2), cos(uv.x * 2.3) + sin(uv.y * 1.3 - t * 0.2));
    float f = field(p, t) - balance * 0.9;
    float w = fwidth(f);
    float m = smoothstep(-w, w, f);
    // Ridges darken toward stripe centres, like dye concentration.
    float depth = smoothstep(0.0, 1.2, abs(f));
    vec3 col = mix(colorA, colorB, m);
    col *= 0.82 + 0.18 * depth;
    fragColor = vec4(col, 1.0);
}
