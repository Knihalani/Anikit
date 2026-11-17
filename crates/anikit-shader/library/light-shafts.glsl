// Light shafts: a hard light behind a slatted window (or foliage) throws
// volumetric beams through hazy air, with dust motes drifting through them.
// Analytic: the occluder is a function of the angle around the light, so
// beams cost the same as a gradient. Fast (~20–40 ms at 1080p).
// Uniforms: light (position in uv 0..1), color, intensity, slats (beam count),
// haze, dust, speed.
uniform vec2 light;      // = vec2(0.82, 0.92)  light position, uv 0..1 (keep it near the top edge)
uniform vec3 color;      // = vec3(1.0, 0.78, 0.5)  light colour
uniform float intensity; // = 1.0  overall brightness
uniform float slats;     // = 9.0  beam count
uniform float haze;      // = 1.0  how much the air glows
uniform float dust;      // = 1.0  dust mote brightness
uniform float speed;     // = 1.0  drift speed

float h11(float x) { return fract(sin(x * 127.1) * 43758.5453); }
float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise1(float x) { float i = floor(x), f = fract(x); return mix(h11(i), h11(i + 1.0), f * f * (3.0 - 2.0 * f)); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float asp = res.x / res.y;
    vec2 uv = fragCoord / res;
    float t = iTime * speed;
    vec2 d = (uv - light) * vec2(asp, 1.0);
    float r = length(d);
    // Angle measured so its ±π seam points straight up, off screen for a light near the top.
    float a = atan(-d.x, -d.y);
    // The occluder: slats with soft edges, drifting leaves/branches breaking them up.
    float slat = smoothstep(0.15, 0.45, abs(fract(a * slats / 3.14159265 + 0.13 * sin(t * 0.21)) - 0.5) * 2.0);
    float leaves = smoothstep(0.35, 0.75, vnoise1(a * 23.0 + t * 0.35) * 0.6 + vnoise1(a * 61.0 - t * 0.5) * 0.4);
    // Beams differ in strength (a few carry most of the light) and breathe slowly.
    float strength = smoothstep(0.25, 0.85, vnoise1(a * 4.0 + 1.7 + t * 0.05) * 0.7 + vnoise1(a * 11.0 - t * 0.08) * 0.3);
    float open = slat * mix(1.0, leaves, 0.55) * (0.15 + 0.85 * strength);
    // Beams fade with distance into the haze.
    float beam = open * (exp(-r * 2.4) + 0.12 * exp(-r * 0.7)) * smoothstep(0.0, 0.06, r);
    float air = exp(-r * 7.0) * 0.8 + exp(-r * 2.0) * 0.06;  // bloom around the source
    vec3 bg = mix(vec3(0.004, 0.004, 0.006), vec3(0.016, 0.014, 0.016), uv.y);
    vec3 col = bg + color * (beam * 0.75 + air) * intensity * haze;
    // Dust: three depth layers of motes, bright only inside beams.
    float motes = 0.0;
    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float sc = 70.0 - fi * 22.0;
        vec2 q = vec2(uv.x * asp, uv.y) * sc + vec2(t * (0.15 + 0.1 * fi), -t * (0.25 + 0.08 * fi));
        vec2 c = floor(q);
        float h = h21(c + fi * 17.0);
        vec2 o = vec2(h21(c + 3.1), h21(c + 7.7)) * 0.6 + 0.2 + 0.15 * vec2(sin(t + h * 30.0), cos(t * 0.8 + h * 20.0));
        float m = smoothstep(0.09 + 0.04 * fi, 0.0, length(fract(q) - o)) * step(0.72, h);
        motes += m * (0.6 + 0.4 * fi);
    }
    col += color * motes * beam * 2.2 * dust * intensity;
    col = 1.0 - exp(-col * 1.25);
    fragColor = vec4(pow(col, vec3(0.4545)), 1.0);
}
