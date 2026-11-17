// Dither: the frame in a tiny palette with an ordered (Bayer) pattern, like
// an early Mac or a Game Boy. Pixel size and palette are yours.
// A filter: {"shader": "dither", "filter": true}.
// Uniforms: pixel, dark, light, mid, levels.
uniform float pixel;  // = 3.0 dither pixel size at 1080p
uniform vec3 dark;    // = vec3(0.06, 0.07, 0.12) darkest colour
uniform vec3 light;   // = vec3(0.93, 0.91, 0.82) lightest colour
uniform vec3 mid;     // = vec3(0.85, 0.35, 0.25) middle colour (with levels 3)
uniform float levels; // = 2.0 2 for 1-bit, 3 adds the middle colour

// 8×8 Bayer threshold from the bits of x and y.
float bayer(vec2 p) {
    ivec2 i = ivec2(p) & 7;
    int v = 0;
    for (int b = 0; b < 3; b++) {
        int xb = (i.x >> b) & 1, yb = (i.y >> b) & 1;
        v |= ((xb ^ yb) << (5 - 2 * b)) | (yb << (4 - 2 * b));
    }
    return (float(v) + 0.5) / 64.0;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float ps = max(1.0, floor(pixel * iResolution.y / 1080.0 + 0.5));
    vec2 cell = floor(fragCoord / ps);
    vec3 c = texture(iChannel0, (cell + 0.5) * ps / iResolution.xy).rgb;
    float l = dot(c, vec3(0.299, 0.587, 0.114));
    float t = bayer(cell);
    vec3 col;
    if (levels > 2.5) {
        float v = l * 2.0;
        float k = floor(v + t);
        col = k < 0.5 ? dark : (k < 1.5 ? mid : light);
    } else {
        col = l > t ? light : dark;
    }
    fragColor = vec4(col, 1.0);
}
