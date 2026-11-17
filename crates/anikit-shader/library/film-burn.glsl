// Film burn (filter): the frame below scorches from hot spots — amber
// glow, a dark charred rim, a light leak. Use with "filter": true and
// animate `amount` 0 → 1. Uniforms: amount, seed.
uniform float amount; // = 0.5  0 clean … 1 fully burnt; animate it
uniform float seed;   // = 1.0  picks the hot spots

float hash(vec2 p) { return fract(sin(dot(p, vec2(41.3, 289.1)) + seed * 13.7) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p) {
    float s = 0.0, a = 0.5;
    for (int i = 0; i < 5; i++) { s += a * vnoise(p); p *= 2.07; a *= 0.5; }
    return s;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 q = fragCoord / iResolution.y;
    vec3 src = texture(iChannel0, uv).rgb;
    // Burn front: noisy distance from two hot spots, grown by `amount`.
    float d = min(length(q - vec2(0.25, 0.8)), length(q - vec2(1.5, 0.15)) * 1.2);
    float front = d * 0.9 + 0.35 * fbm(q * 4.0 + seed) - amount * 1.5 + 0.12;
    float burnt = smoothstep(0.02, -0.02, front);
    float rim = smoothstep(0.08, 0.0, abs(front)) * (1.0 - burnt);
    float heat = smoothstep(0.35, 0.0, front) * (1.0 - burnt);
    vec3 col = src;
    col = mix(col, col * vec3(1.25, 0.9, 0.55) + vec3(0.25, 0.08, 0.0), heat * 0.8);   // warming
    col = mix(col, vec3(1.0, 0.75, 0.3) * 1.6, smoothstep(0.03, 0.0, abs(front - 0.02)) * (1.0 - burnt)); // glowing edge
    col = mix(col, vec3(0.06, 0.03, 0.02), rim * 0.9);                                  // char
    col = mix(col, vec3(1.0, 0.95, 0.85), burnt);                                      // burnt through to white
    // Light leak along the right edge.
    col += vec3(1.0, 0.45, 0.15) * amount * 0.35 * smoothstep(0.6, 1.0, uv.x) * (0.6 + 0.4 * fbm(q * 2.0 + iTime * 0.3));
    fragColor = vec4(col, 1.0);
}
