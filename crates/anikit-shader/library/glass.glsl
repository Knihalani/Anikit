// Glass: everything below seen through a pane of reeded, ribbed, rippled or
// pebbled glass. Each element bends the image by its slope, splits colour a
// little, catches a highlight and frosts if asked.
// A filter: {"shader": "glass", "filter": true}.
// Uniforms: pattern, width, strength, dispersion, frost, angle, highlight, tint.
uniform float pattern;    // = 0.0 0 fluted (reeded), 1 ribbed, 2 ripple, 3 pebbled
uniform float width;      // = 48.0 element size in pixels at 1080p
uniform float strength;   // = 1.0 how far the glass bends the image
uniform float dispersion; // = 1.0 colour fringing at slopes
uniform float frost;      // = 0.0 0 clear … 1 frosted
uniform float angle;      // = 0.0 rotation of the pattern in degrees
uniform float highlight;  // = 1.0 specular streaks on the glass
uniform vec3 tint;        // = vec3(0.97, 0.99, 1.0) glass colour

// Height (0..1) and slope (d/dx, d/dy in pattern space) of the glass at q (in elements).
vec3 glassField(vec2 q) {
    if (pattern < 0.5) {                      // fluted: half-cylinders side by side
        float f = fract(q.x) * 2.0 - 1.0;
        float h = sqrt(max(1.0 - f * f, 0.0));
        return vec3(h, -f / max(h, 0.08), 0.0);
    } else if (pattern < 1.5) {               // ribbed: rounded horizontal ribs
        float f = fract(q.y) * 2.0 - 1.0;
        float h = cos(f * 1.5707963);
        return vec3(h, 0.0, -1.5707963 * sin(f * 1.5707963) * 1.2);
    } else if (pattern < 2.5) {               // ripple: rings spreading from the centre
        float r = length(q);
        float w = sin(r * 6.2831853 - iTime * 2.0) * exp(-r * 0.08);
        vec2 d = q / max(r, 1e-3) * cos(r * 6.2831853 - iTime * 2.0) * exp(-r * 0.08);
        return vec3(w * 0.5 + 0.5, d * 1.2);
    }
    // pebbled: soft lumps on a hex-ish jittered grid
    vec2 i = floor(q), f = fract(q) - 0.5;
    vec2 o = vec2(fract(sin(dot(i, vec2(127.1, 311.7))) * 43758.5453), fract(sin(dot(i, vec2(269.5, 183.3))) * 43758.5453)) - 0.5;
    vec2 d = f - o * 0.4;
    float h = exp(-dot(d, d) * 6.0);
    return vec3(h, -12.0 * d * h);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float s = res.y / 1080.0;
    float a = radians(angle);
    mat2 r = mat2(cos(a), -sin(a), sin(a), cos(a));
    vec2 q = r * (fragCoord - 0.5 * res) / (width * s);
    vec3 g = glassField(q);
    vec2 slope = transpose(r) * g.yz;
    // Refraction: bend by the slope (pixels), split colour along the same direction.
    vec2 off = slope * width * s * 0.18 * strength;
    float fr = (fract(sin(dot(fragCoord, vec2(12.9898, 78.233))) * 43758.5453) - 0.5) * frost * 10.0 * s;
    off += vec2(fr, -fr * 0.7);
    vec2 uv = (fragCoord + off) / res;
    vec2 ch = off * 0.035 * dispersion / res;
    vec3 col;
    col.r = texture(iChannel0, uv + ch).r;
    col.g = texture(iChannel0, uv).g;
    col.b = texture(iChannel0, uv - ch).b;
    // The glass itself: slight tint, darker where it's thin, a highlight along the crest.
    col *= tint * (0.9 + 0.1 * g.x);
    float spec = pow(clamp(g.x, 0.0, 1.0), 24.0) * 0.18 + pow(clamp(1.0 - abs(slope.x + slope.y * 0.3 - 0.35), 0.0, 1.0), 40.0) * 0.12;
    col += spec * highlight;
    fragColor = vec4(col, 1.0);
}
