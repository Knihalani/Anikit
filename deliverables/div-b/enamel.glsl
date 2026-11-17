// Enamelled signal-box panel: dark green enamel, soft key light from upper left,
// low-contrast mottling and fine horizontal brushing. Static (no iTime) so it reads as a surface.
float h(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vn(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(h(i), h(i + vec2(1.0, 0.0)), f.x), mix(h(i + vec2(0.0, 1.0)), h(i + vec2(1.0, 1.0)), f.x), f.y);
}
void mainImage(out vec4 o, in vec2 f) {
  vec2 uv = f / iResolution.xy;
  float n = 0.5 * vn(f / 180.0) + 0.3 * vn(f / 60.0) + 0.2 * vn(f / 14.0);
  float brush = vn(vec2(f.x / 400.0, f.y / 1.6)) * 0.5;
  float key = exp(-2.2 * length((uv - vec2(0.22, 0.12)) * vec2(1.0, 0.75)));
  vec3 base = vec3(0.055, 0.105, 0.09);
  vec3 c = base * (0.75 + 0.55 * key) + vec3(0.012, 0.02, 0.017) * (n - 0.5) * 2.0 + vec3(0.006) * brush;
  o = vec4(c, 1.0);
}
