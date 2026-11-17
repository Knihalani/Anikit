// Lake mirror (filter): everything drawn above a horizon line is reflected in
// still dark water below it, rippled and darkened, brightest at grazing
// angles. Pairs with `aurora` (lake = 0) for a faster lake that also
// reflects any layer composited in between. The defaults match aurora's
// camera; animate `horizon`/`focal` with the backdrop's scale.
// Uniforms: horizon (in (2·fragCoord − res) / res.y units), focal, speed,
// ripple, gain (reflected brightness).
uniform float horizon; // = -0.35  waterline, in uv −1 (bottom) … 1 (top)
uniform float focal;   // = 1.6  perspective of the water plane
uniform float speed;   // = 1.0  ripple speed
uniform float ripple;  // = 1.0  ripple strength
uniform float gain;    // = 1.0  reflection brightness

uint hashu(uint x) { x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16; return x; }
float hash2(vec2 p) { return float(hashu(uint(int(p.x)) * 1973u + hashu(uint(int(p.y)) + 9277u))) / 4294967295.0; }
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2(1, 0)), u.x), mix(hash2(i + vec2(0, 1)), hash2(i + vec2(1, 1)), u.x), u.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    vec2 uv = (2.0 * fragCoord - res) / res.y;
    vec4 here = texture(iChannel0, fragCoord / res);
    vec3 rd = normalize(vec3(uv.x, uv.y - horizon, -focal));
    if (rd.y > 0.0) {
        fragColor = here;
        return;
    }
    float t = iTime * speed;
    // The same rippled mirror direction aurora traces…
    vec3 r = vec3(rd.x, -rd.y, rd.z);
    float rip = (vnoise(vec2(rd.x / rd.y * 8.0, 1.0 / rd.y * 2.0 + t * 0.5)) - 0.5) * ripple;
    r.x += rip * 0.08 * -rd.y;
    r.y += rip * 0.02 * -rd.y;
    r = normalize(r);
    // …looked up in the rendered frame instead of traced again.
    vec2 suv = vec2(r.x / -r.z, r.y / -r.z) * focal + vec2(0.0, horizon);
    vec3 sk = pow(texture(iChannel0, (suv * res.y + res) * 0.5 / res).rgb, vec3(2.2));
    float fres = (0.25 + 0.5 * exp(rd.y * 12.0)) * gain;
    vec3 col = sk * fres + vec3(0.0, 0.003, 0.006);
    fragColor = vec4(pow(col, vec3(0.4545)), 1.0);
}
