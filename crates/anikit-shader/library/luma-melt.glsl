// Transition: the old scene melts away darkest-first (a luma key), so the
// new scene appears through the shadows before the highlights.
// Uniforms: softness, invert (1 = brightest first).
uniform float softness; // = 0.15  width of the melt edge
uniform float invert;   // = 0.0  1 melts brightest-first

vec4 transition(vec2 uv) {
    vec4 a = getFromColor(uv);
    float l = dot(a.rgb, vec3(0.2126, 0.7152, 0.0722));
    l = mix(l, 1.0 - l, invert);
    float p = progress * (1.0 + softness);
    float m = smoothstep(p - softness, p, l);
    return mix(getToColor(uv), a, m);
}
