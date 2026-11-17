// Soap film: a macro view of a draining soap film. Its thickness swirls with
// the flow and drains downward under gravity, and each wavelength reflects by
// real thin-film interference, so colours come in the true order: silver-white
// where thick, then the magenta–blue–green bands, then gold, white-grey and,
// where it is thinnest (at the top), black film: Newton's colour sequence.
// anikit:aa off (smooth fields; the swirls are wider than a pixel)
// Uniforms: speed, swirl, drain, thickness, background.
uniform float speed;      // = 1.0 flow speed
uniform float swirl;      // = 1.0 how strongly the film turns and folds
uniform float drain;      // = 1.0 how much thinner the film is at the top
uniform float thickness;  // = 1.0 overall film thickness (more bands when thicker)
uniform vec3 background;  // = vec3(0.01, 0.01, 0.012) what's behind the film

// Reflectance of a thin film of thickness d (nm), refractive index 1.33, at
// near-normal incidence, for wavelength l (nm): two-beam interference with
// the half-wave shift at the front face, so it goes to 0 as d → 0 (black film).
float film(float d, float l) {
    float ph = 6.2831853 * 2.0 * 1.33 * d / l;
    return 0.5 - 0.5 * cos(ph);
}

// Six wavelengths weighted by rough colour-matching curves: the eye's mix of
// the spectrum, not three pure channels (which would look like a rainbow).
vec3 filmColour(float d) {
    vec3 c = film(d, 440.0) * vec3(0.18, 0.02, 0.95)
           + film(d, 480.0) * vec3(0.02, 0.25, 0.75)
           + film(d, 520.0) * vec3(0.05, 0.85, 0.2)
           + film(d, 560.0) * vec3(0.45, 0.85, 0.02)
           + film(d, 600.0) * vec3(0.95, 0.45, 0.0)
           + film(d, 650.0) * vec3(0.75, 0.08, 0.0);
    return c / vec3(2.4, 2.5, 1.92);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = ak_uv(fragCoord);
    float t = iTime * speed;
    // Swirling flow: two levels of warp drifting at different rates.
    vec2 p = uv * 1.2;
    p += (vec2(ak_fbm(p * 0.8 + vec2(t * 0.05, 0.0), 3), ak_fbm(p * 0.8 + vec2(3.1, -t * 0.04), 3)) - 0.5) * 1.6 * swirl;
    p += (vec2(ak_gnoise(p * 2.2 - t * 0.07), ak_gnoise(p * 2.2 + 7.7 + t * 0.06)) - 0.5) * 0.5 * swirl;
    float n = ak_fbm(p * 1.4, 4);
    // Thickness (nm): drains to the top (thin, black film) from the bottom (thick, silver).
    float grav = clamp(0.5 - uv.y * 0.42 * drain, 0.0, 1.0);
    float d = (60.0 + 1100.0 * grav * grav + 380.0 * (n - 0.5)) * thickness;
    d = max(d, 0.0);
    vec3 refl = filmColour(d);
    // Thick films blend the bands toward silver (the eye integrates many orders).
    refl = mix(refl, vec3(0.55), smoothstep(900.0, 1600.0, d));
    // Light: a soft studio key reflected in the film (brighter toward the top left).
    float key = 0.55 + 0.45 * smoothstep(1.4, -0.6, length(uv - vec2(-0.6, 0.5)));
    vec3 col = mix(background, refl * key * 1.25, 0.92);
    col = ak_vignette(col, uv, 0.8);
    fragColor = vec4(ak_grain(col, fragCoord, 0.4), 1.0);
}
