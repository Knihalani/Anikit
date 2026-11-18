// Floodlight banks igniting row by row over a wet night sky, beams through drizzle.
uniform float boot; // = 0.0

float hash(float n) { return fract(sin(n) * 43758.5453); }
float hash2(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash2(i), hash2(i + vec2(1.0, 0.0)), u.x),
             mix(hash2(i + vec2(0.0, 1.0)), hash2(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) {
  float v = 0.0; float a = 0.5;
  for (int i = 0; i < 4; i++) { v += a * noise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
  return v;
}
float rowOn(float t0, float seed) {
  float t = iTime - t0;
  float on = smoothstep(0.0, 0.04, t);
  float flick = mix(0.35 + 0.65 * step(0.45, hash(floor(iTime * 30.0) + seed)), 1.0, smoothstep(0.08, 0.2, t));
  return on * flick;
}
// one floodlight head: returns rgb contribution; p in frame units
vec3 head(vec2 p, vec2 c, float s, float t0, float ang, vec2 dir, out float lit) {
  float ca = cos(ang); float sa = sin(ang);
  vec2 q = p - c; q = vec2(ca * q.x + sa * q.y, -sa * q.x + ca * q.y) / s;
  vec3 col = vec3(0.0);
  // steel frame
  vec2 b = abs(q) - vec2(0.25, 0.165);
  float box = 1.0 - smoothstep(0.0, 0.004 / s, max(b.x, b.y));
  float rim = smoothstep(0.012, 0.0, abs(max(b.x, b.y)));
  lit = 0.0;
  vec3 lamps = vec3(0.0);
  float halo = 0.0;
  for (int r = 0; r < 4; r++) {
    float on = rowOn(t0 + float(r) * 0.25, float(r) * 7.0 + c.x * 13.0);
    lit += on * 0.25;
    for (int k = 0; k < 6; k++) {
      vec2 lc = vec2(-0.2 + float(k) * 0.08, 0.12 - float(r) * 0.08);
      float d = length(q - lc);
      float disk = smoothstep(0.034, 0.03, d);
      float core = smoothstep(0.03, 0.0, d);
      vec3 off = vec3(0.02, 0.025, 0.03) * disk + vec3(0.06, 0.07, 0.08) * smoothstep(0.034, 0.031, d) * smoothstep(0.026, 0.03, d);
      vec3 onc = mix(vec3(1.0, 0.86, 0.62), vec3(1.0, 0.99, 0.96), core) * disk * (1.4 + core * 2.0);
      lamps += mix(off, onc, on);
      halo += on * 0.0016 / (d * d * s * s + 0.0016) ;
    }
  }
  col = vec3(0.025, 0.03, 0.04) * box + vec3(0.25, 0.3, 0.38) * rim * (0.15 + 0.5 * lit) + lamps * box;
  col += vec3(1.0, 0.9, 0.75) * halo * 0.05;
  return col;
}
float beam(vec2 p, vec2 c, vec2 dir, float w0, float spread) {
  vec2 q = p - c;
  float along = dot(q, dir);
  float perp = abs(q.x * dir.y - q.y * dir.x);
  float w = w0 + along * spread;
  return smoothstep(w, w * 0.2, perp) * smoothstep(0.0, 0.1, along) * exp(-along * 0.9);
}
void mainImage(out vec4 o, in vec2 f) {
  vec2 p = (f - 0.5 * iResolution.xy) / iResolution.y;
  float t = iTime;
  // night sky
  vec3 col = mix(vec3(0.012, 0.02, 0.04), vec3(0.03, 0.05, 0.085), smoothstep(0.6, -0.5, p.y));
  float haze = fbm(p * 2.5 + vec2(t * 0.06, -t * 0.02));
  // heads
  float lit1; float lit2; float lit3;
  vec2 c1 = vec2(-0.42, 0.24); vec2 d1 = normalize(vec2(0.55, -1.0));
  vec2 c2 = vec2(0.5, 0.36);  vec2 d2 = normalize(vec2(-0.5, -1.0));
  vec2 c3 = vec2(0.12, 0.42); vec2 d3 = normalize(vec2(0.05, -1.0));
  vec3 h1 = head(p, c1, 1.0, 0.35, 0.12, d1, lit1);
  vec3 h2 = head(p, c2, 0.42, 1.25, -0.1, d2, lit2);
  vec3 h3 = head(p, c3, 0.22, 1.6, 0.0, d3, lit3);
  // volumetric beams through haze
  float b = beam(p, c1, d1, 0.24, 0.55) * lit1 + beam(p, c2, d2, 0.1, 0.5) * lit2 * 0.7 + beam(p, c3, d3, 0.05, 0.45) * lit3 * 0.5;
  float hz = 0.35 + 0.9 * haze;
  col += vec3(0.95, 0.88, 0.74) * b * hz * 0.85;
  // drizzle caught in the beams
  float colx = floor(f.x / 3.0);
  float sp = 2.2 + hash(colx) * 1.4;
  float y = fract(f.y / iResolution.y * 1.3 + t * sp + hash(colx * 3.1) * 7.0);
  float streak = smoothstep(0.0, 0.02, y) * smoothstep(0.09, 0.02, y) * step(0.9, hash(colx * 1.7 + floor(f.y / iResolution.y * 1.3 + t * sp + hash(colx * 3.1) * 7.0)));
  col += vec3(1.0, 0.95, 0.85) * streak * b * 0.9;
  // stadium glow on the horizon
  float lit = lit1 * 0.6 + lit2 * 0.25 + lit3 * 0.15;
  col += vec3(0.5, 0.55, 0.45) * exp(-(p.y + 0.62) * (p.y + 0.62) * 18.0) * lit * 0.35 * hz;
  // lattice masts
  float m1 = smoothstep(0.006, 0.002, abs(p.x - (-0.43) - (p.y - 0.1) * 0.05)) * step(p.y, 0.08);
  float m1b = smoothstep(0.006, 0.002, abs(p.x - (-0.36) + (p.y - 0.1) * 0.05)) * step(p.y, 0.08);
  float lat = smoothstep(0.004, 0.0, abs(fract((p.y + p.x * 0.9) * 14.0) - 0.5) * 0.07) * step(-0.44 - (p.y-0.1)*0.05, p.x) * step(p.x, -0.35 + (p.y-0.1)*0.05) * step(p.y, 0.08);
  col = mix(col, vec3(0.03, 0.035, 0.045) + vec3(0.2, 0.2, 0.18) * lit1 * 0.25, clamp(m1 + m1b + lat * 0.8, 0.0, 1.0));
  col += h1 + h2 + h3;
  // grain
  col += (hash2(f + fract(t) * 91.0) - 0.5) * 0.025;
  col = col / (1.0 + col * 0.35);
  o = vec4(pow(max(col, 0.0), vec3(0.95)), 1.0);
}
