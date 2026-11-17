// Macro of etched Damascus steel. `layers` doubles as the billet is folded:
// the flowing strata multiply, lit by a raking light that slides across.
uniform float layers; // = 2.0
uniform float light;  // = 0.0
uniform float zoom;   // = 1.0
uniform float glow;   // = 0.0

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) { float s = 0.0; float a = 0.5; for (int i = 0; i < 4; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; } return s; }
vec3 bb(float h) { h = max(h, 0.0); return vec3(1.7 * h, 1.1 * h * h, 0.7 * h * h * h * h); }

float strata(vec2 p) {
  // warped coordinate: twisted ladder pattern
  float w = p.y + 0.22 * fbm(p * 1.6) + 0.07 * sin(p.x * 5.0 + 2.0 * fbm(p * 3.0)) + 0.04 * sin(p.x * 13.0 + p.y * 4.0);
  return w * layers * 0.5;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
  vec2 p = uv / zoom + vec2(iTime * 0.05, 0.0);
  float w = strata(p);
  float fw = fwidth(w);
  float s = 0.5 + 0.5 * sin(w * 6.2831853);
  // antialias: fade contrast when stripes approach the pixel size
  s = mix(0.5, s, clamp(1.0 - fw * 1.6, 0.0, 1.0));
  // relief: etched dark layers sit lower
  float hgt = s * 0.6 + 0.05 * fbm(p * 40.0);
  vec2 g = vec2(dFdx(hgt), dFdy(hgt)) * 60.0;
  vec3 n = normalize(vec3(-g, 1.0));
  vec3 L = normalize(vec3(light * 1.6 - uv.x, 0.6 - uv.y, 0.55));
  vec3 V = vec3(0.0, 0.0, 1.0);
  vec3 H = normalize(L + V);
  float spec = pow(max(dot(n, H), 0.0), mix(18.0, 90.0, s));
  vec3 bright = vec3(0.78, 0.8, 0.84);
  vec3 dark = vec3(0.07, 0.075, 0.085);
  vec3 alb = mix(dark, bright, smoothstep(0.25, 0.85, s));
  float diff = max(dot(n, L), 0.0);
  // anisotropic brushed sheen along the blade
  float brush = vnoise(vec2(p.x * 6.0, p.y * 420.0));
  vec3 col = alb * (0.18 + 0.55 * diff) + vec3(1.0, 0.96, 0.9) * spec * (0.5 + 0.9 * s) * (0.8 + 0.4 * brush);
  // broad raking highlight band
  col += vec3(0.85, 0.88, 0.95) * 0.35 * exp(-pow((uv.x - light * 1.2 + uv.y * 0.6) * 2.2, 2.0)) * s;
  // forge heat still in the steel
  col += bb(glow * (0.7 + 0.3 * s)) * glow;
  col *= 1.0 - 0.45 * dot(uv, uv);
  fragColor = vec4(col, 1.0);
}
