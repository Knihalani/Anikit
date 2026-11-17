// Sky: a golden-hour sky with two cloud decks seen from the ground, low
// cumulus-like billows and high cirrus streaks, sliding past in parallax. The
// clouds are lit from the sun's side (bright, silvered edges facing it),
// darker and warmer at their thick bases, and fade into the horizon haze.
// Each deck is a warped noise field projected onto a plane, so it costs
// a few noise lookups per pixel.
// anikit:aa off (soft clouds and gradients)
// Uniforms: sun, coverage, speed, warmth, cirrus, horizon.
uniform float sun;      // = 0.8 0 high afternoon … 1 sun on the horizon
uniform float coverage; // = 0.55 how much of the sky the low clouds cover
uniform float speed;    // = 1.0 drift speed
uniform float warmth;   // = 1.0 0 neutral daylight … 1 golden hour
uniform float cirrus;   // = 0.7 high wispy cloud amount
uniform float horizon;  // = -0.7 where the horizon sits (centred coords, y up)

vec3 sunDir() { return normalize(vec3(0.4, mix(0.42, 0.04, sun), -1.0)); }

float deck(vec2 q, float t, float cov) {
    vec2 w = ak_warp(q + vec2(t, 0.0), 0.35);
    float n = ak_fbm(w, 5);
    float th = 0.64 - cov * 0.24;
    return smoothstep(th, th + 0.09, n);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = ak_uv(fragCoord);
    float t = iTime * speed;
    vec3 sd = sunDir();
    vec3 rd = normalize(vec3(uv.x, uv.y - horizon - 0.02, -1.6));
    float up = max(rd.y, 0.0);
    float s = max(dot(rd, sd), 0.0);
    float gold = warmth * sun;
    // The clear sky: deep overhead, warm and bright at the horizon toward the sun.
    vec3 zen = mix(vec3(0.06, 0.16, 0.42), vec3(0.09, 0.12, 0.3), gold);
    vec3 hor = mix(vec3(0.6, 0.72, 0.86), vec3(1.0, 0.48, 0.2), gold);
    vec3 col = mix(hor, zen, pow(smoothstep(0.0, 0.7, up), 0.45));
    col += vec3(1.0, 0.55, 0.28) * pow(s, 8.0) * (0.15 + 0.4 * gold);
    vec3 sunCol = mix(vec3(1.2, 1.15, 1.05), vec3(1.5, 0.9, 0.5), gold);
    if (rd.y > 0.003) {
        // High cirrus: stretched streaks on a high plane, lit almost evenly.
        vec2 qc = rd.xz / rd.y * 0.18;
        float ci = ak_fbm(vec2(qc.x * 0.6, qc.y * 2.2) + vec2(t * 0.01, 0.0), 4);
        ci = smoothstep(0.5, 0.72, ci) * cirrus * smoothstep(0.0, 0.25, up);
        col = mix(col, mix(hor, sunCol, 0.45 + 0.4 * s), ci * 0.55);
        // Low deck: billows on a low plane; the sun's side is lit, the far side shadowed.
        vec2 q = rd.xz / rd.y * 0.5;
        float d = deck(q, t * 0.03, coverage);
        vec2 toSun = normalize(sd.xz) * 0.06;
        float dl = deck(q + toSun, t * 0.03, coverage);
        float lit = clamp(0.35 + (d - dl) * 3.0, 0.0, 1.4);       // facing the sun: brighter
        vec3 base = mix(vec3(0.22, 0.24, 0.32), vec3(0.26, 0.14, 0.2), gold);   // thick, shadowed bases
        vec3 cloud = mix(base, sunCol * 1.05, lit * 0.75);
        cloud += sunCol * pow(s, 6.0) * (1.0 - d) * 0.9;           // silver lining toward the sun
        // Far clouds sink into the horizon haze.
        float far = smoothstep(0.25, 0.0, up);
        cloud = mix(cloud, hor, far * 0.75);
        col = mix(col, cloud, d * (1.0 - far * 0.4));
    }
    // The sun itself and its glare.
    col += sunCol * smoothstep(0.99965, 0.99985, s) * 6.0 + vec3(1.0, 0.75, 0.5) * pow(s, 60.0) * 0.4;
    // Ground: a dark silhouette strip below the horizon.
    float g = smoothstep(0.004, -0.004, rd.y);
    col = mix(col, mix(vec3(0.03, 0.03, 0.035), hor * 0.2, exp(rd.y * 20.0) * 0.5), g);
    col = ak_tonemap(col * 0.8);
    col = ak_vignette(col, uv, 0.6);
    fragColor = vec4(ak_grain(col, fragCoord, 0.4), 1.0);
}
