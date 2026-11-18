// Ink: a drop of pigment blooming in still water, backlit. Billows unfold and
// tear into fine tendrils; dense cores absorb the light, thin wisps glow with
// the colour; a second ink can curl through the first. Three depths of warped
// noise give it volume; the bloom grows from the drop over time.
// anikit:aa off (soft volumes)
// Uniforms: ink, ink2, mixing, water, speed, size, center, turbulence.
uniform vec3 ink;         // = vec3(0.05, 0.12, 0.42) main pigment (what it absorbs leaves this colour)
uniform vec3 ink2;        // = vec3(0.72, 0.1, 0.16) second pigment
uniform float mixing;     // = 0.35 how much of the second ink (0 none)
uniform vec3 water;       // = vec3(0.96, 0.95, 0.92) backlight through the water
uniform float tt;         // = 0.0 clock
uniform float speed;      // = 1.0 bloom speed
uniform float size;       // = 1.0 bloom size
uniform vec2 center;      // = vec2(0.0, 0.35) where the drop entered (centred coords, y up)
uniform float turbulence; // = 1.0 how torn and tendrilled the billows are

// Pigment density of one depth layer at p (centred coords), bloom radius r.
float layer(vec2 p, float r, float t, float seed) {
    // Sinking: the cloud falls slowly as it blooms, and hangs taller than wide.
    vec2 q = (p - center - vec2(0.0, -0.035 * t)) / max(r, 0.05);
    q.y *= 0.8;
    vec2 w = ak_warp(q * 1.3 + seed * 3.1 + vec2(0.0, t * 0.04), 0.55 * turbulence);
    float body = ak_fbm(w * 1.1 + t * 0.02, 4);
    // Tendrils: ridged noise along the warped flow, strongest at the edges.
    float tend = ak_ridged(w * 2.6 + seed, 4);
    float fall = exp(-dot(q * vec2(0.9, 0.7), q * vec2(0.9, 0.7)) * 1.6);
    float edge = smoothstep(0.05, 0.6, fall) * (1.0 - smoothstep(0.6, 1.0, fall));
    return max(fall * smoothstep(0.38, 0.75, body) * 1.4 + edge * smoothstep(0.55, 0.85, tend) * turbulence * 0.9, 0.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = ak_uv(fragCoord);
    float t = tt * speed;
    // The bloom grows fast at first, then keeps creeping outward.
    float r = size * (0.25 + 0.55 * (1.0 - exp(-t * 0.35)) + 0.02 * t);
    vec3 light = ak_linear(water);
    // Absorption through three depths (Beer–Lambert): what the ink lets through.
    // Absorption per channel from the pigment's colour, so a blue ink stays blue.
    vec3 kInk = -log(max(ak_linear(ink), 0.004)) * 0.42;
    vec3 kInk2 = -log(max(ak_linear(ink2), 0.004)) * 0.42;
    vec3 T = vec3(1.0);
    for (int i = 0; i < 2; i++) {
        float fi = float(i);
        vec2 pp = uv + vec2(0.04, -0.03) * fi;                 // slight parallax per depth
        float d = layer(pp, r * (1.0 - 0.12 * fi), t + fi * 7.0, fi * 1.7);
        float m2 = 0.0;
        if (mixing > 0.0) m2 = smoothstep(0.45, 0.8, ak_fbm(pp * 2.2 + fi * 5.0 + t * 0.03, 3)) * mixing;
        vec3 k = mix(kInk, kInk2, m2);
        T *= exp(-k * d * (1.0 - 0.3 * fi));
    }
    // Light scattered inside the cloud glows a little with its own colour.
    vec3 col = light * T + ak_linear(ink) * (1.0 - T.b) * 0.08;
    // The water: a soft backlight falloff and the faintest caustic shimmer.
    col *= 1.0 - 0.25 * dot(uv * 0.55, uv * 0.55);
    col *= 1.0 + 0.02 * (ak_gnoise(uv * 3.0 + t * 0.05) - 0.5);
    fragColor = vec4(ak_grain(ak_srgb(col), fragCoord, 0.4), 1.0);
}
