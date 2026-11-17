// Glitch / datamosh (filter): block displacement, smeared rows that hold
// stale pixels, RGB channel split and scanlines, in bursts.
// Uniforms: amount (0..1), rate (bursts per second).
uniform float amount; // = 0.6  0 off … 1 wrecked
uniform float rate;   // = 2.0  bursts per second

float h1(float n) { return fract(sin(n * 12.9898) * 43758.5453); }
float h2(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float tick = floor(iTime * rate * 6.0);
    float burst = step(0.55, h1(floor(iTime * rate))) * amount;
    // Macroblocks (16×16 like a codec) slide sideways.
    vec2 blk = floor(fragCoord / 16.0);
    float bh = h2(blk + tick);
    vec2 duv = uv;
    if (bh < 0.25 * burst) duv.x += (h2(blk.yy + tick) - 0.5) * 0.3 * burst;
    // Datamosh: some rows smear, reusing the pixels of an earlier column.
    float row = floor(fragCoord.y / 4.0);
    if (h2(vec2(row, tick)) < 0.08 * burst) duv.x = floor(duv.x * 12.0) / 12.0;
    // Channel split.
    float split = 0.006 * amount + 0.03 * burst * h1(tick);
    float r = texture(iChannel0, duv + vec2(split, 0.0)).r;
    float g = texture(iChannel0, duv).g;
    float b = texture(iChannel0, duv - vec2(split, 0.0)).b;
    vec3 col = vec3(r, g, b);
    // Quantised colour in hit blocks, scanlines everywhere.
    if (bh > 1.0 - 0.15 * burst) col = floor(col * 4.0) / 4.0;
    col *= 0.94 + 0.06 * sin(fragCoord.y * 3.14159);
    fragColor = vec4(col, 1.0);
}
