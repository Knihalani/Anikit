// Risograph: two (or three) spot inks laid separately, slightly out of
// register, with the grainy, uneven coverage of a stencil duplicator.
// A filter: {"shader": "risograph", "filter": true}.
// Uniforms: ink1, ink2, ink3, use3, offset, grain, paper.
uniform vec3 ink1;    // = vec3(1.0, 0.28, 0.45) first ink (fluorescent pink)
uniform vec3 ink2;    // = vec3(0.0, 0.47, 0.75) second ink (medium blue)
uniform vec3 ink3;    // = vec3(1.0, 0.85, 0.0) third ink (yellow), with use3
uniform float use3;   // = 0.0 1 adds the third ink
uniform float offset; // = 3.0 misregistration in pixels at 1080p
uniform float grain;  // = 1.0 stencil grain amount
uniform vec3 paper;   // = vec3(0.97, 0.95, 0.9) paper colour

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

// How much of an ink a colour needs: its darkness along the ink's opposite.
float need(vec3 c, vec3 ink) {
    vec3 a = 1.0 - ink;                     // what the ink absorbs
    return clamp(dot(1.0 - c, a) / max(dot(a, a), 1e-3), 0.0, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 res = iResolution.xy;
    float px = offset * res.y / 1080.0;
    vec2 uv = fragCoord / res;
    vec2 o1 = vec2(px, -0.6 * px) / res, o2 = vec2(-0.7 * px, px * 0.4) / res;
    vec3 c1 = texture(iChannel0, uv + o1).rgb;
    vec3 c2 = texture(iChannel0, uv + o2).rgb;
    float n = h21(floor(fragCoord * 0.75)) - 0.5;
    float g = grain * 0.22;
    float a1 = smoothstep(0.0, 1.0, need(c1, ink1) + n * g);
    float a2 = smoothstep(0.0, 1.0, need(c2, ink2) - n * g);
    vec3 col = paper * mix(vec3(1.0), ink1, a1) * mix(vec3(1.0), ink2, a2);
    if (use3 > 0.5) {
        vec3 c3 = texture(iChannel0, uv).rgb;
        float a3 = smoothstep(0.0, 1.0, need(c3, ink3) + h21(fragCoord.yx) * g - g * 0.5);
        col *= mix(vec3(1.0), ink3, a3);
    }
    // Uneven drum pressure: faint large-scale density drift.
    col = mix(col, paper, 0.06 * (0.5 + 0.5 * sin(uv.y * 7.0 + uv.x * 3.0)));
    fragColor = vec4(col, 1.0);
}
