// Transition: a vortex twists the old scene and unwinds into the new one.
// Uniforms: turns, radius.
uniform float turns;  // = 1.5
uniform float radius; // = 0.8

vec4 transition(vec2 uv) {
    vec2 c = uv - 0.5;
    c.x *= ratio;
    float r = length(c);
    float d = sin(progress * 3.14159265);
    float a = turns * 6.2831853 * d * smoothstep(radius, 0.0, r);
    float s = sin(a), co = cos(a);
    vec2 q = mat2(co, s, -s, co) * c;
    q.x /= ratio;
    q += 0.5;
    return mix(getFromColor(q), getToColor(q), smoothstep(0.3, 0.7, progress));
}
