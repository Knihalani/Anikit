// Particles: thousands of glowing points drifting in depth with bokeh — in
// one pass, no simulation. Each depth layer hashes a grid of cells to one
// particle per cell; near layers are bigger and blurrier (depth of field).
// Fast (~60–90 ms at 1080p). Uniforms: color, color2, density (0..1),
// speed, size, focus (0 near … 1 far), direction (drift, uv/s).
uniform vec3 color;      // = vec3(1.0, 0.72, 0.38)
uniform vec3 color2;     // = vec3(0.35, 0.6, 1.0)
uniform float density;   // = 0.6
uniform float speed;     // = 1.0
uniform float size;      // = 1.0
uniform float focus;     // = 0.5
uniform vec2 direction;  // = vec2(0.02, 0.05)

vec3 h33(vec3 p) {
    uvec3 q = uvec3(ivec3(p) + 32768) * uvec3(1597334673u, 3812015801u, 2798796415u);
    q = (q.x ^ q.y ^ q.z) * uvec3(1597334673u, 3812015801u, 2798796415u);
    return vec3(q) * (1.0 / 4294967295.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float t = iTime * speed;
    vec3 col = mix(vec3(0.002, 0.003, 0.007), vec3(0.008, 0.007, 0.013), fragCoord.y / iResolution.y);
    for (int L = 0; L < 6; L++) {
        float z = float(L) / 5.0;                       // 0 near … 1 far
        float scale = mix(5.0, 26.0, z);                // cells per screen height
        float blur = abs(z - focus) * 1.6;              // circle of confusion
        vec2 p = uv * scale + direction * t * scale * mix(1.6, 0.5, z) + z * 31.7;
        vec2 cell = floor(p);
        vec2 f = fract(p);
        // Sharp particles stay inside their cell; big defocused discs can
        // spill over an edge, so those layers check the 2×2 nearest cells.
        // (The test is uniform across the frame, so it costs nothing.)
        float rmax = 0.065 * size * mix(1.0, 0.45, z) * (1.0 + 4.0 * blur);
        int cells = rmax > 0.16 ? 4 : 1;
        vec2 o = step(0.5, f) * 2.0 - 1.0;
        for (int k = 0; k < cells; k++) {
            vec2 c = cell + vec2(float(k & 1), float(k >> 1)) * o;
            vec3 h = h33(vec3(c, float(L)));
            if (h.z > density) continue;
            vec2 pos = c + 0.25 + 0.5 * h.xy + 0.08 * vec2(sin(t * (0.7 + h.x) + h.y * 40.0), cos(t * (0.6 + h.y) + h.x * 30.0));
            float r = (0.035 + 0.03 * h.x) * size * mix(1.0, 0.45, z) * (1.0 + 4.0 * blur);
            float d = length(p - pos) / r;
            if (d >= 1.0) continue;                       // most pixels miss: skip the shading
            // Bokeh: a soft disc with a brighter rim when defocused; a sharp glow in focus.
            float disc = smoothstep(1.0, 1.0 - 0.25 - 0.5 * (1.0 - blur), d);
            float rim = smoothstep(0.75, 1.0, d) * disc * blur;
            float core = exp(-d * d * 6.0) * (1.0 - blur);
            float energy = (disc * 0.35 + rim * 0.4 + core) / (1.0 + 6.0 * blur * blur);
            float tw = 0.7 + 0.3 * sin(t * (2.0 + 3.0 * h.y) + h.x * 50.0);
            col += mix(color, color2, smoothstep(0.35, 0.65, h.y)) * energy * tw * mix(1.1, 0.5, z);
        }
    }
    col = 1.0 - exp(-col * 1.3);
    fragColor = vec4(pow(col, vec3(0.4545)), 1.0);
}
