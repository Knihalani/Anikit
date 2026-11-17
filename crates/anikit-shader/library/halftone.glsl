// Halftone: the frame as printed dots. CMYK screens at classic angles (or one
// black screen), dot size per ink from the colour under each cell, on paper.
// A filter: {"shader": "halftone", "filter": true}.
// Uniforms: size, mono, angle, paper, contrast.
uniform float size;     // = 9.0 dot cell size in pixels at 1080p
uniform float mono;     // = 0.0 1 for a single black screen
uniform float angle;    // = 15.0 screen angle in degrees (CMYK use it plus 60/30/75)
uniform vec3 paper;     // = vec3(0.96, 0.94, 0.89) paper colour
uniform float contrast; // = 1.1 ink contrast

// Ink coverage of one screen at pixel p: rotate, find the cell, sample there.
float screen(vec2 p, float a, int ch) {
    float s = size * iResolution.y / 1080.0;
    mat2 r = mat2(cos(a), -sin(a), sin(a), cos(a));
    vec2 q = r * p / s;
    vec2 c = floor(q) + 0.5;
    // The colour at the cell's centre: one exact texel read (cheaper than a
    // filtered lookup; a dot's colour is constant over its cell anyway).
    vec2 cp = transpose(r) * (c * s);
    vec3 col = texelFetch(iChannel0, ivec2(clamp(cp, vec2(0.0), iResolution.xy - 1.0)), 0).rgb;
    float k = 1.0 - max(col.r, max(col.g, col.b));
    vec3 cmy = (1.0 - col - k) / max(1.0 - k, 1e-3);
    float v = ch == 0 ? cmy.x : (ch == 1 ? cmy.y : (ch == 2 ? cmy.z : k));
    if (mono > 0.5) v = 1.0 - dot(col, vec3(0.299, 0.587, 0.114));
    v = clamp((v - 0.5) * contrast + 0.5, 0.0, 1.0);
    float d = length(q - c);
    float r0 = sqrt(v) * 0.62;
    float aa = 0.75 / s;                       // about one pixel, in cell units
    return smoothstep(r0 + aa, r0 - aa, d);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float a = radians(angle);
    vec3 col;
    if (mono > 0.5) {
        col = paper * (1.0 - 0.92 * screen(fragCoord, a + radians(30.0), 3));
    } else {
        float c = screen(fragCoord, a, 0), m = screen(fragCoord, a + radians(60.0), 1);
        float y = screen(fragCoord, a + radians(75.0), 2), k = screen(fragCoord, a + radians(30.0), 3);
        // Inks multiply on paper.
        col = paper * mix(vec3(1.0), vec3(0.0, 0.66, 0.92), c) * mix(vec3(1.0), vec3(0.93, 0.0, 0.55), m)
            * mix(vec3(1.0), vec3(1.0, 0.93, 0.0), y) * mix(vec3(1.0), vec3(0.1), k);
    }
    fragColor = vec4(col, 1.0);
}
