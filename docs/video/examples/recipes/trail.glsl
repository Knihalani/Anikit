// Buffer A: last frame, zoomed and turned a little and faded, plus a bright orbiting dot.
void mainImage(out vec4 c, in vec2 p) {
    vec2 uv = p / iResolution.xy, ctr = uv - 0.5;
    float a = 0.01;
    ctr = mat2(cos(a), -sin(a), sin(a), cos(a)) * ctr * 0.985;
    vec3 prev = texture(iChannel0, ctr + 0.5).rgb * 0.965;
    vec2 q = (p - 0.5 * iResolution.xy) / iResolution.y;
    vec2 dot_ = 0.3 * vec2(cos(iTime * 1.7), sin(iTime * 2.3));
    vec3 hue = 0.5 + 0.5 * cos(iTime * 0.6 + vec3(0.0, 2.1, 4.2));
    vec3 ink = hue * exp(-pow(length(q - dot_) / 0.018, 2.0)) * 2.0;
    c = vec4(prev + ink, 1.0);
}
