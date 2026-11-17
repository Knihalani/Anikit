// Inside the forge: domain-warped flames over a cracked coal bed, blackbody colour.
uniform float heat;  // = 1.0
uniform float zoom;  // = 1.0
uniform float flare; // = 0.0
uniform float rise;  // = 1.0

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) { float s = 0.0; float a = 0.5; for (int i = 0; i < 5; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; } return s; }
vec3 bb(float h) { h = max(h, 0.0); return vec3(1.7 * h, 1.15 * h * h, 0.75 * h * h * h * h); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y / zoom;
  float t = iTime * rise;
  // flames
  vec2 q = uv * vec2(2.4, 1.5);
  vec2 w = vec2(fbm(q * 1.3 + vec2(0.0, -t * 0.9)), fbm(q * 1.3 + vec2(5.2, -t * 1.1)));
  float n = fbm(q + 1.6 * w + vec2(0.0, -t * 1.8));
  float base = clamp(1.05 - (uv.y + 0.32) * 0.95, 0.0, 1.5);
  float f = n * base * 1.55 * heat + flare * 0.35;
  f = smoothstep(0.32, 1.25, f);
  vec3 col = bb(min(f * 1.45, 1.12));
  // coal bed with glowing cracks
  float bed = smoothstep(-0.22, -0.34, uv.y + 0.03 * sin(uv.x * 7.0));
  vec2 cq = uv * vec2(7.0, 11.0);
  float c = fbm(cq + vec2(t * 0.05, 0.0));
  float crack = 1.0 - abs(2.0 * fbm(cq * 0.8 + 3.1) - 1.0);
  crack = pow(crack, 7.0);
  float pulse = 0.75 + 0.25 * sin(t * 2.0 + c * 9.0);
  vec3 coal = vec3(0.05, 0.035, 0.03) * c + bb(min(crack * 1.15 + 0.35 * smoothstep(0.55, 0.8, c), 1.05) * pulse * heat);
  col = mix(col, coal + col * 0.3, bed);
  // embers
  vec2 eu = uv * vec2(14.0, 9.0) + vec2(0.0, -t * 3.0);
  vec2 ei = floor(eu);
  float er = hash(ei);
  vec2 ep = fract(eu) - vec2(0.3 + 0.4 * hash(ei + 7.0), 0.5 + 0.3 * sin(t * 3.0 + er * 6.0));
  float emb = step(0.9, er) * smoothstep(0.035 + 0.03 * hash(ei + 3.0), 0.0, length(ep * vec2(1.0, 0.35)));
  col += bb(0.95 + 0.3 * hash(ei + 9.0)) * emb * 1.6 * (1.0 - smoothstep(0.1, 0.5, uv.y));
  // vignette, flicker
  col *= 1.0 - 0.55 * dot(uv * 0.9, uv * 0.9);
  col *= 0.92 + 0.08 * vnoise(vec2(t * 9.0, 1.0));
  fragColor = vec4(col, 1.0);
}
