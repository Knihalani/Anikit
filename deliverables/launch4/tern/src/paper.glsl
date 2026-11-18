// Carbonless stock: fibres, formation and tooth as a multiply layer over the page (paper, print and ink),
// so ink sits in the paper. It lives in page space and moves with the sheet.
float h(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float n(vec2 p) {
    vec2 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(h(i), h(i + vec2(1, 0)), f.x), mix(h(i + vec2(0, 1)), h(i + vec2(1, 1)), f.x), f.y);
}
void mainImage(out vec4 o, in vec2 f) {
    vec2 p = f / iResolution.x * 1920.0;
    float fib = smoothstep(0.72, 0.95, n(vec2(p.x * 0.6 + p.y * 0.05, p.y * 9.0)))
              + smoothstep(0.75, 0.97, n(vec2(p.x * 7.0, p.y * 0.5 - p.x * 0.1) + 17.0));
    float mott = n(p * 0.012) * 0.6 + n(p * 0.04) * 0.4;
    float tooth = h(floor(p * 1.3));
    float k = 1.0 - (0.05 * fib + 0.06 * (mott - 0.5) + 0.04 * (tooth - 0.5)) - 0.02;
    o = vec4(vec3(k), 1.0);
}
