// SIGNAL: a living network in wet dark ground (mycelium, neurons) that
// actually transmits. One node fires; the signal runs out along the filaments
// at a finite speed; each node it reaches answers with its own front; the
// cascade lights the whole network, and the word the network has been
// growing around is what stays lit. Camera: a slow pull-back from macro.
// iChannel0: the word, white on black, framed as the final wide shot.

const float V = 115.0;               // signal speed, world px per second

// Camera zoom (screen px per world px): macro at the start, wide at the end.
float zoom(float t) { return mix(3.4, 1.0, smoothstep(0.5, 15.5, t)); }

// World position of a screen pixel at time t (world = final framing, 1080p px).
vec2 world(vec2 fc, float t) {
    vec2 c = vec2(960.0, 540.0);
    vec2 focus = mix(vec2(760.0, 600.0), c, smoothstep(2.0, 15.5, t));   // drifts to centre
    return focus + (ak_px(fc) - c) / zoom(t);
}

// Network: warped Voronoi edges at two scales. x = filament (0..1),
// y = node glow, z = edge distance (world px).
vec3 network(vec2 w) {
    vec2 q = w + 22.0 * (ak_warp(w * 0.003, 0.5) - 0.5);
    vec2 a = ak_voronoi_edge(q / 95.0);
    vec2 b = ak_voronoi_edge(q / 31.0 + 7.3);
    float ea = a.x * 95.0 * 2.0, eb = b.x * 31.0 * 2.0;
    float fa = exp(-ea * ea / 4.0);
    float fb = exp(-eb * eb / 1.6) * 0.45;
    float node = exp(-a.y * a.y * 95.0 * 95.0 / 14.0);
    return vec3(max(fa, fb), node, min(ea, eb * 1.6));
}

// Arrival time of the signal at w: fronts from the ignition points (bell times),
// travelling along slightly longer, wandering paths than straight lines.
float arrival(vec2 w) {
    float wander = 1.0 + 0.35 * ak_noise(w * 0.01);
    float best = 1e4;
    vec2 S[7] = vec2[7](vec2(760.0, 600.0), vec2(1030.0, 470.0), vec2(520.0, 380.0), vec2(1350.0, 720.0), vec2(300.0, 800.0), vec2(1620.0, 330.0), vec2(1100.0, 930.0));
    float T[7] = float[7](1.5, 4.0, 6.5, 8.0, 9.5, 10.5, 11.25);
    for (int i = 0; i < 7; i++) {
        best = min(best, T[i] + length(w - S[i]) * wander / V);
    }
    // From 11.5 s the whole field answers: a dense cascade of small fronts.
    vec2 g = floor(w / 220.0);
    vec2 h = ak_hash2(g);
    vec2 sp = (g + 0.2 + 0.6 * h) * 220.0;
    best = min(best, 11.5 + 2.4 * h.x + length(w - sp) * wander / (V * 0.8));
    return best;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float t = iTime;
    vec2 w = world(fragCoord, t);
    float z = zoom(t);
    vec3 net = network(w);
    // Word mask, in world space (the final framing), softened a little.
    vec2 wuv = vec2(w.x / 1920.0, w.y / 1080.0);
    float word = 0.0;
    if (wuv.x > 0.0 && wuv.x < 1.0 && wuv.y > 0.0 && wuv.y < 1.0) word = texture(iChannel0, wuv).r;
    float reveal = smoothstep(14.2, 17.0, t);

    // Signal: a bright front as it arrives, then a lingering, slowly fading charge.
    float ta = arrival(w);
    float since = t - ta;
    float front = since > 0.0 ? exp(-since * 5.0) : exp(-since * since * 900.0);
    float charge = since > 0.0 ? 0.25 + 0.75 * exp(-since * 0.9) : 0.0;
    // After the reveal the word holds its charge; the rest settles to embers.
    charge = mix(charge, mix(charge * 0.3, 1.6, word), reveal);
    // The word ignites as one last front sweeps along it, left to right.
    float sweep = t - (14.2 + w.x / 1920.0 * 1.2);
    front += word * (sweep > 0.0 ? exp(-sweep * 3.0) : 0.0) * 1.5;

    // Colour arc: cold blue-white early, amber in the cascade, white-gold at the end.
    float arc = smoothstep(3.0, 13.0, t);
    vec3 fc = mix(vec3(0.45, 0.8, 1.0), vec3(1.0, 0.55, 0.18), arc);
    fc = mix(fc, vec3(1.0, 0.86, 0.62), reveal * word);
    vec3 idle = vec3(0.006, 0.009, 0.022);

    // Ground: deep indigo, wet, with a fine fibrous texture.
    float tex = ak_fbm(w * 0.02, 4);
    vec3 ground = mix(vec3(0.0012, 0.0016, 0.0035), vec3(0.004, 0.005, 0.011), tex);
    // Wet sheen: a faint specular from the texture's slope toward a light at upper left.
    ground += vec3(0.004, 0.005, 0.009) * pow(ak_noise(w * 0.09), 6.0);
    vec3 col = ground;

    // Light the ground from nearby charged filaments (a soft halo).
    float halo = exp(-net.z * 0.08 * z) * (charge * 0.6 + front * 1.5);
    col += fc * halo * 0.22;

    // Filaments: cold and dim at rest, lit by the front and its charge.
    float fil = net.x;
    float lit = front * 2.4 + charge * 0.9;
    col += idle * fil * (1.0 - reveal * 0.5);
    col += fc * fil * lit;
    // Nodes flare as the front passes, then pulse with the charge.
    col += mix(fc, vec3(1.0), 0.5) * net.y * (front * 3.0 + charge * 0.7);

    // Depth: out-of-focus spores drifting in front, moving faster than the ground.
    vec2 sp = ak_px(fragCoord) / (z * 0.55) + vec2(t * 9.0, -t * 5.0);
    vec2 cell = floor(sp / 140.0);
    vec2 hh = ak_hash2(cell);
    vec2 cpos = (cell + 0.2 + 0.6 * hh) * 140.0;
    float bok = smoothstep(14.0, 9.0, length(sp - cpos)) * step(0.6, hh.y);
    col += fc * bok * 0.05 * (0.4 + charge);

    col = ak_tonemap(col);
    col = ak_vignette(col, ak_uv(fragCoord), 0.8);
    fragColor = vec4(ak_grain(col, fragCoord, 0.3), 1.0);
}
