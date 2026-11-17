// Light shafts: sun through a window into a dark, hazy room. Near-parallel
// beams (one per pane, split by mullions and broken by leaves outside) cross
// the frame, drifting haze gives them volume, and dust motes glint only where
// the light catches them. Analytic, one pass, fast.
// Uniforms: angle, spread, panes, color, intensity, haze, dust, speed.
// anikit:aa off (measured within SSIM 0.995 / ΔE p99.9 2.3 of 2×2 supersampling at 1 sample)
uniform float angle;     // = 35.0 beam direction in degrees from straight down (positive leans right)
uniform float spread;    // = 0.25 0 parallel sunlight … 1 fanning from a near source
uniform float panes;     // = 4.0 window panes across the beam
uniform vec3 color;      // = vec3(1.0, 0.8, 0.55) light colour
uniform float intensity; // = 1.0 overall brightness
uniform float haze;      // = 1.0 how thick the air is (beam visibility and texture)
uniform float dust;      // = 1.0 dust mote brightness
uniform float speed;     // = 1.0 drift speed

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(h21(i), h21(i + vec2(1, 0)), u.x), mix(h21(i + vec2(0, 1)), h21(i + vec2(1, 1)), u.x), u.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    vec2 p = (fragCoord - 0.5 * res) / res.y;          // y up, −0.5..0.5 vertically
    float t = iTime * speed;
    float a = radians(angle);
    vec2 L = vec2(sin(a), -cos(a));                    // light travels along L (down the frame)
    vec2 N = vec2(-L.y, L.x);                          // across the beams
    // Window: behind the top edge, so beams enter from above. Along = distance travelled.
    vec2 win = -L * 0.95 - N * 0.12;
    vec2 d = p - win;
    float along = dot(d, L);
    float across = dot(d, N) / (1.0 + spread * along);  // fan out away from the window
    // Panes: bright rectangles across the beam with soft mullion shadows; the
    // whole window is ~0.6 frame heights wide. Edges soften with distance (penumbra).
    float soft = 0.006 + 0.03 * along;
    float wx = across / 0.6 + 0.5;
    float inWin = smoothstep(-soft, soft, wx) * smoothstep(-soft, soft, 1.0 - wx);
    float pane = abs(fract(wx * panes) - 0.5) * 2.0;     // 0 at the middle of a pane, 1 at a mullion
    float mull = smoothstep(0.8 - soft * panes * 3.0, 0.8 + soft * panes * 3.0, pane);
    // Leaves outside the window drift across it and break up the panes.
    float leaves = vnoise(vec2(wx * 9.0 + t * 0.12, 3.1 + t * 0.05)) * 0.65 + vnoise(vec2(wx * 23.0 - t * 0.2, 7.7)) * 0.35;
    leaves = smoothstep(0.32, 0.7, leaves);
    float open = inWin * (1.0 - mull) * mix(1.0, leaves, 0.6);
    // Volume: beams fade as they travel and scatter in drifting haze.
    vec2 hz = vec2(across * 3.0, along * 0.7 - t * 0.04);
    float air = 0.55 + 0.45 * (vnoise(hz * 2.0 + vec2(t * 0.05, 0.0)) * 0.65 + vnoise(hz * 5.3 - vec2(0.0, t * 0.08)) * 0.35);
    float fall = exp(-along * 0.9) * smoothstep(0.0, 0.25, along);
    float beam = open * fall * mix(1.0, air, 0.8 * min(haze, 1.0)) * haze;
    // Room: near-black, with a faint bounce where the light lands low in frame.
    vec3 room = vec3(0.006, 0.0055, 0.006) + color * 0.012 * smoothstep(0.6, -0.5, p.y) * haze;
    vec3 col = room + color * beam * 0.42 * intensity;
    // Dust: three depths of motes, visible only inside beams.
    float motes = 0.0;
    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float sc = 60.0 - fi * 18.0;
        vec2 q = p * sc + vec2(t * (0.08 + 0.05 * fi), -t * (0.18 + 0.06 * fi));
        vec2 c = floor(q);
        float h = h21(c + fi * 17.0);
        vec2 o = vec2(h21(c + 3.1), h21(c + 7.7)) * 0.6 + 0.2 + 0.15 * vec2(sin(t + h * 30.0), cos(t * 0.8 + h * 20.0));
        float m = smoothstep(0.07 + 0.03 * fi, 0.0, length(fract(q) - o)) * step(0.86, h);
        motes += m * (0.5 + 0.5 * fi);
    }
    col += color * motes * open * fall * 1.4 * dust * intensity;
    col = 1.0 - exp(-col * 1.3);
    fragColor = vec4(pow(col, vec3(0.4545)), 1.0);
}
