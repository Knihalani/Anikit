// Transition: the frame breaks into growing pixel blocks, cross-fades,
// then resolves into the new scene. Uniforms: blocks (smallest count across).
uniform float blocks; // = 12.0

vec4 transition(vec2 uv) {
    float d = min(progress, 1.0 - progress) * 2.0;            // 0 → 1 → 0
    float n = mix(iResolution.x, blocks, pow(d, 0.35));
    vec2 cells = vec2(n, n / ratio);
    vec2 q = d > 0.001 ? (floor(uv * cells) + 0.5) / cells : uv;
    return mix(getFromColor(q), getToColor(q), smoothstep(0.35, 0.65, progress));
}
