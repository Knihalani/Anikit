// Growth: a living reaction–diffusion simulation (Gray–Scott), four steps per
// frame. Spots divide, labyrinths fold, coral branches, then it is lit as a
// relief: soft key light, ambient occlusion in the grooves, a glossy wet sheen.
// Multi-pass: buffers A–D each take one simulation step.
// anikit:aa off (the relief is smooth: no edges to anti-alias)
// Uniforms: feed, kill, scale, color, color2, light, speed.
uniform float feed;   // = 0.0545 feed rate: 0.030 spots … 0.062 coral (with kill)
uniform float kill;   // = 0.062 kill rate: 0.057 labyrinths … 0.065 spots
uniform float scale;  // = 2.0 pattern size (simulation cell in pixels at 1080p)
uniform vec3 color;   // = vec3(0.92, 0.42, 0.32) colour of the grown matter
uniform vec3 color2;  // = vec3(0.05, 0.06, 0.09) ground colour
uniform float light;  // = 1.0 relief lighting strength
uniform float speed;  // = 1.0 0 freezes the growth

// State: r = A (substrate), g = B (the grown chemical), on a grid of `scale` px cells.
vec2 cellOf(vec2 p) { float s = max(1.0, floor(scale * iResolution.y / 1080.0 + 0.5)); return floor(p / s) * s + 0.5 * s; }
vec2 at(vec2 c, vec2 d) {
    float s = max(1.0, floor(scale * iResolution.y / 1080.0 + 0.5));
    // Exact texel reads (one load each); the edges wrap so the pattern tiles.
    ivec2 q = ivec2(mod(c + d * s, iResolution.xy));
    return texelFetch(iChannel0, q, 0).rg;
}

vec4 simStep(vec2 fragCoord) {
    vec2 c = cellOf(fragCoord);
    vec2 v = at(c, vec2(0.0));
    if (iFrame == 0) {
        // Seed: substrate everywhere, the chemical in noisy patches.
        float n = fract(sin(dot(floor(c / 7.0), vec2(127.1, 311.7))) * 43758.5453);
        float m = fract(sin(dot(c, vec2(12.9898, 78.233))) * 43758.5453);
        return vec4(1.0, step(0.78, n) * (0.4 + 0.3 * m), 0.0, 1.0);
    }
    vec2 lap = 0.2 * (at(c, vec2(1, 0)) + at(c, vec2(-1, 0)) + at(c, vec2(0, 1)) + at(c, vec2(0, -1)))
             + 0.05 * (at(c, vec2(1, 1)) + at(c, vec2(-1, 1)) + at(c, vec2(1, -1)) + at(c, vec2(-1, -1))) - v;
    float a = v.x, b = v.y;
    float abb = a * b * b;
    float dt = min(speed, 1.0);
    a += dt * (1.0 * lap.x - abb + feed * (1.0 - a));
    b += dt * (0.5 * lap.y + abb - (kill + feed) * b);
    return vec4(clamp(a, 0.0, 1.0), clamp(b, 0.0, 1.0), 0.0, 1.0);
}

//--- buffer A: D
void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = simStep(fragCoord); }

//--- buffer B: A
void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = simStep(fragCoord); }

//--- buffer C: B
void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = simStep(fragCoord); }

//--- buffer D: C
void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = simStep(fragCoord); }

//--- image: D
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 px = 1.5 / iResolution.xy;
    // Height = the chemical, smoothed by the bilinear lookups.
    float h = texture(iChannel0, uv).g;
    float hx = texture(iChannel0, uv + vec2(px.x, 0.0)).g - texture(iChannel0, uv - vec2(px.x, 0.0)).g;
    float hy = texture(iChannel0, uv + vec2(0.0, px.y)).g - texture(iChannel0, uv - vec2(0.0, px.y)).g;
    vec3 n = normalize(vec3(-hx * 6.0, -hy * 6.0, 1.0));
    vec3 L = normalize(vec3(-0.5, 0.6, 0.65));
    float m = smoothstep(0.08, 0.32, h);
    float dif = max(dot(n, L), 0.0);
    float spec = pow(max(dot(reflect(-L, n), vec3(0, 0, 1)), 0.0), 40.0);
    vec3 base = mix(color2, color, m);
    vec3 col = base * mix(1.0, 0.35 + 0.9 * dif, light) + spec * 0.35 * light * m;
    col *= 0.75 + 0.25 * smoothstep(0.0, 0.25, h + 0.1);                // grooves read darker
    col *= 1.0 - 0.25 * dot(uv - 0.5, uv - 0.5);
    fragColor = vec4(pow(col, vec3(0.95)), 1.0);
}
