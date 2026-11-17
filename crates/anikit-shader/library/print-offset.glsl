// Print offset: a cheap print or a magazine scan. The cyan, magenta and
// yellow plates drift apart, colours lose a little saturation, and the paper
// shows a faint fibre.
// A filter: {"shader": "print-offset", "filter": true}.
// Uniforms: amount, angle, paper, fibre.
uniform float amount; // = 4.0 plate drift in pixels at 1080p
uniform float angle;  // = 30.0 drift direction in degrees
uniform vec3 paper;   // = vec3(0.97, 0.95, 0.9) paper colour
uniform float fibre;  // = 1.0 paper fibre amount

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float s = res.y / 1080.0;
    vec2 uv = fragCoord / res;
    float a = radians(angle);
    vec2 d = vec2(cos(a), sin(a)) * amount * s / res;
    vec2 e = vec2(-d.y, d.x) * 0.6;
    float c = 1.0 - texture(iChannel0, uv + d).r;
    float m = 1.0 - texture(iChannel0, uv - d * 0.7 + e).g;
    float y = 1.0 - texture(iChannel0, uv - e).b;
    vec3 col = paper * (1.0 - vec3(c, m, y) * 0.95);
    float f = h21(floor(fragCoord / s * vec2(0.4, 3.0))) - 0.5;
    col *= 1.0 + f * 0.035 * fibre;
    fragColor = vec4(col, 1.0);
}
