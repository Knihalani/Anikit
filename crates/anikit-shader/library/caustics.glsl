// Caustics: sunlight focused by a moving water surface onto a pool floor.
// The floor brightness is the density of refracted rays (the Jacobian of
// the refraction map, computed with derivatives). Uniforms: speed, depth, tint.
uniform float speed; // = 1.0
uniform float depth; // = 1.0
uniform vec3 tint;   // = vec3(0.35, 0.75, 0.95)

// Height of the water surface: a few travelling waves.
float wave(vec2 p, float t) {
    float h = 0.0;
    h += 0.50 * sin(dot(p, vec2(1.0, 0.3)) * 3.1 + t * 1.3);
    h += 0.35 * sin(dot(p, vec2(-0.6, 1.0)) * 4.3 - t * 1.7);
    h += 0.25 * sin(dot(p, vec2(0.8, -0.9)) * 6.7 + t * 2.3);
    h += 0.15 * sin(dot(p, vec2(-1.0, -0.2)) * 9.1 - t * 2.9);
    return h;
}

vec2 grad(vec2 p, float t) {
    const float e = 0.01;
    return vec2(wave(p + vec2(e, 0.0), t) - wave(p - vec2(e, 0.0), t), wave(p + vec2(0.0, e), t) - wave(p - vec2(0.0, e), t)) / (2.0 * e);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.y;
    float t = iTime * speed;
    vec2 p = uv * 3.0;
    // Where light through this surface point lands on the floor.
    vec2 land = p + grad(p, t) * 0.07 * depth;
    // Ray density = 1 / |det J| of the surface→floor map.
    float area = abs(dFdx(land.x) * dFdy(land.y) - dFdx(land.y) * dFdy(land.x));
    float px = abs(dFdx(p.x) * dFdy(p.y));
    float light = clamp(px / max(area, 1e-7), 0.0, 6.0);
    light = pow(light, 1.6);
    // Floor tiles under the light.
    vec2 g = abs(fract(uv * 6.0) - 0.5);
    float grout = smoothstep(0.47, 0.5, max(g.x, g.y));
    vec3 floorc = mix(vec3(0.75, 0.86, 0.9), vec3(0.55, 0.68, 0.74), grout);
    vec3 col = floorc * tint * (0.35 + 0.65 * light);
    col += vec3(1.0, 0.98, 0.9) * 0.12 * max(light - 1.5, 0.0);
    col *= 1.0 - 0.3 * length(uv - vec2(0.5 * iResolution.x / iResolution.y, 0.5));
    fragColor = vec4(pow(col, vec3(0.9)), 1.0);
}
