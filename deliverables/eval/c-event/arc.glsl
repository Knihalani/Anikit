// A high-voltage arc between two contacts (y0 at top, y1 at bottom, in 0..1 of height, top = 0).
uniform float power;   // = 1.0
uniform float y0;      // = 0.0
uniform float y1;      // = 1.0
uniform float xc;      // = 0.5
uniform float spread;  // = 1.0
uniform float bg;      // = 1.0

float hash(float n) { return fract(sin(n) * 43758.5453); }
float n1(float x) { float i = floor(x); float f = fract(x); f = f * f * (3.0 - 2.0 * f); return mix(hash(i), hash(i + 1.0), f) * 2.0 - 1.0; }
float fbm(float x, float s) { float v = 0.0; float a = 0.55; for (int k = 0; k < 6; k++) { v += a * n1(x + s * 17.31); x *= 2.13; a *= 0.5; s += 3.7; } return v; }
float h2(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
float vn(vec2 p) { vec2 i = floor(p); vec2 f = fract(p); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(h2(i), h2(i + vec2(1.0, 0.0)), f.x), mix(h2(i + vec2(0.0, 1.0)), h2(i + vec2(1.0, 1.0)), f.x), f.y); }

void mainImage(out vec4 o, in vec2 f) {
  vec2 R = iResolution.xy;
  float y = 1.0 - f.y / R.y;          // 0 at top
  float seed = floor(iTime * 20.0);   // re-strike 20x a second
  float flick = 0.55 + 0.45 * hash(seed * 1.7);
  float span = max(y1 - y0, 0.001);
  float u = clamp((y - y0) / span, 0.0, 1.0);
  float inside = step(y0, y) * step(y, y1);
  float env = sin(3.14159 * u);
  float px = f.x;
  float cx = xc * R.x;
  // main channel
  float x1 = cx + spread * 230.0 * env * fbm(u * 2.4, seed);
  float d1 = abs(px - x1);
  // a second, fainter channel
  float x2 = cx + spread * 300.0 * env * fbm(u * 2.0 + 5.0, seed + 41.0);
  float d2 = abs(px - x2);
  // a short branch
  float bu = clamp((u - 0.3) / 0.35, 0.0, 1.0);
  float x3 = x1 + spread * 260.0 * bu * bu * (hash(seed + 9.0) > 0.5 ? 1.0 : -1.0) + 60.0 * fbm(u * 6.0, seed + 77.0) * bu;
  float d3 = abs(px - x3) + (1.0 - step(0.3, u)) * 1e4 + step(0.65, u) * 1e4;
  float core = exp(-d1 * d1 / 10.0) + 0.55 * exp(-d2 * d2 / 5.0) + 0.5 * exp(-d3 * d3 / 4.0);
  float halo = exp(-d1 / 26.0) * 0.55 + exp(-d1 / 140.0) * 0.22 + exp(-d2 / 30.0) * 0.18;
  float soft = smoothstep(-0.06, 0.0, y - y0) * smoothstep(-0.06, 0.0, y1 - y);
  float a = power * flick * inside;
  float ah = power * flick * soft;
  vec3 hot = vec3(1.0, 0.98, 0.95);
  vec3 ion = vec3(0.55, 0.62, 1.0);
  vec3 col = hot * core * a * 1.4 + ion * halo * ah;
  // contacts glow
  float c0 = exp(-length(vec2(px - cx, (y - y0) * R.y)) / 50.0);
  float c1 = exp(-length(vec2(px - cx, (y - y1) * R.y)) / 50.0);
  col += vec3(1.0, 0.55, 0.2) * (c0 + c1) * power * 0.8;
  // concrete: dark, mottled, lit by the arc
  float conc = 0.6 * vn(f / 9.0) + 0.3 * vn(f / 40.0) + 0.1 * vn(f / 160.0);
  float lit = exp(-abs(px - cx) / 420.0) * power * flick;
  vec3 base = vec3(0.045, 0.043, 0.041) * (0.7 + 0.6 * conc) * (1.0 + 2.2 * lit) * bg;
  base += ion * 0.05 * lit * conc * bg;
  o = vec4(base + col, 1.0);
}
