// Drawn: a landscape made only of the ak_ drawing blocks: brush-stroke sky
// washes, a dry-brushed sun, a stippled beam of light, a hill hatched along
// its slope (crossing strokes in the shadows), an inked ridge, paper tooth,
// and line boil on twos. Read it as an example of the blocks, then copy it.
// Uniforms: paper, ink, sun, sky, sky2, boil, hatch.
uniform vec3 paper;  // = vec3(0.96, 0.93, 0.86) paper colour
uniform vec3 ink;    // = vec3(0.2, 0.28, 0.3) hill ink
uniform vec3 sun;    // = vec3(0.92, 0.36, 0.18) sun and beam colour
uniform vec3 sky;    // = vec3(0.55, 0.68, 0.82) top wash
uniform vec3 sky2;   // = vec3(0.93, 0.74, 0.55) lowest wash
uniform float boil;  // = 1.2 line boil in pixels (0 = still)
uniform float hatch; // = 9.0 hatch spacing in pixels at 1080p

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 p = ak_px(fragCoord);
    p += ak_boil(p, boil, 12.0);
    float pap = ak_paper(p);
    vec3 col = paper;
    for (int i = 0; i < 6; i++) {
        float fi = float(i);
        float y = 1000.0 - fi * 55.0;
        float c = ak_brush(p, vec2(-80.0 + fi * 37.0, y), vec2(2000.0 - fi * 61.0, y + 18.0 - fi * 6.0), 34.0, fi);
        col = mix(col, mix(sky, sky2, fi / 5.0), c * 0.8);
    }
    vec2 sp = vec2(1380.0, 690.0);
    float beam = ak_wedge(p, sp, vec2(-0.7, -0.7), 0.35, 900.0, 0.8);
    col = mix(col, mix(sun, paper, 0.25), ak_stipple(p, beam * 0.55, 9.0) * 0.8);
    col = mix(col, sun, ak_dry(length(p - sp) - 110.0, pap, 14.0));
    float h = 380.0 + 120.0 * sin(p.x * 0.004 + 0.6) + 40.0 * sin(p.x * 0.013);
    float slope = 0.48 * cos(p.x * 0.004 + 0.6) + 0.52 * cos(p.x * 0.013);
    float tone = clamp(0.25 + (h - p.y) / 500.0, 0.0, 1.0);
    col = mix(col, ink, ak_dry(p.y - h, pap, 8.0) * (0.35 + 0.65 * ak_hatch(p, vec2(1.0, slope), tone, hatch)));
    col = mix(col, ink * 0.4, ak_dry(abs(p.y - h) - 2.5, pap, 2.0));
    fragColor = vec4(ak_paper_shade(col, p, 0.8), 1.0);
}
