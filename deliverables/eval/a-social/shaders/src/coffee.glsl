// Shot 1: a cup of black coffee from above. A drop lands, rings spread, the crema starts to spin.
const vec2 C = vec2(540.0, 1100.0);
const float R = 305.0;
vec3 ceramic(vec2 p, float r){
  vec2 n = p / max(r, 1e-3);
  float side = dot(n, normalize(vec2(-0.55, 0.83)));
  float x = clamp((r - 1.0) / 0.17, 0.0, 1.0);           // 0 inner lip -> 1 outer edge
  vec3 c = hexc(240.0, 234.0, 223.0);
  float lip = smoothstep(0.0, 0.35, x) * (1.0 - smoothstep(0.75, 1.0, x));
  float shade = 0.80 + 0.20 * lip + 0.10 * side * (x - 0.5);
  // inner wall catches shadow on the lit side
  shade -= 0.18 * (1.0 - smoothstep(0.0, 0.25, x)) * max(side, 0.0);
  float spec = smoothstep(0.92, 1.0, side) * lip * 0.5;
  return c * shade + spec;
}
void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  vec2 p = (f - C) / R;
  vec3 col = field(f, hexc(58.0, 22.0, 14.0));
  float r = length(p);
  float aa = 1.6 / R;
  // soft cast shadow, light from top-left
  float sh = smoothstep(1.65, 1.2, length(p - vec2(0.13, -0.17)));
  col *= 1.0 - 0.6 * sh;
  // saucer ring
  float sau = smoothstep(1.46 + aa, 1.46 - aa, r);
  vec3 sc = hexc(232.0, 225.0, 212.0) * (0.86 + 0.1 * dot(p / max(r,1e-3), normalize(vec2(-0.55,0.83))));
  sc *= 1.0 - 0.35 * smoothstep(1.35, 1.12, length(p - vec2(0.07, -0.09))); // cup shadow on saucer
  sc *= 1.0 - 0.10 * smoothstep(1.22, 1.30, r) * smoothstep(1.40, 1.30, r);  // well of the saucer
  col = mix(col, sc, sau);
  // cup
  float cupR = 1.17;
  float inCup = smoothstep(cupR + aa, cupR - aa, r);
  col = mix(col, ceramic(p, r), inCup);
  // liquid
  float drop = 1.35;
  float td = max(t - drop, 0.0);
  float ring = 0.0;
  if (t > drop) {
    float d = r - td * 0.85;
    ring = sin(d * 46.0) * exp(-abs(d) * 9.0) * exp(-td * 1.6) * (1.0 - smoothstep(0.95, 1.0, r));
  }
  float spin = 0.35 * t + 3.2 * smoothstep(1.75, 2.5, t) * (t - 1.75);
  float twist = (1.0 - r) * (1.4 + 4.0 * smoothstep(1.8, 2.5, t));
  vec2 q = rot(spin + twist) * p + ring * 0.03 * p / max(r, 1e-3);
  float cr = fbm(q * 3.2 + 4.0);
  float cr2 = fbm(q * 9.0 - 2.0);
  vec3 dark = hexc(30.0, 13.0, 6.0);
  vec3 crema = hexc(168.0, 98.0, 48.0);
  vec3 tiger = hexc(214.0, 150.0, 86.0);
  float edge = smoothstep(0.55, 0.98, r);
  float cm = clamp(edge * 0.9 + 0.45 * smoothstep(0.45, 0.75, cr) - 0.15, 0.0, 1.0);
  vec3 liq = mix(dark, crema, cm * 0.85);
  liq = mix(liq, tiger, smoothstep(0.62, 0.8, cr2) * cm * 0.6);
  // tiny bubbles at the rim
  float bub = smoothstep(0.18, 0.05, vor(q * 70.0)) * smoothstep(0.86, 0.97, r);
  liq = mix(liq, tiger * 1.1, bub * 0.5);
  // window reflection, bent by the ripples
  vec2 wp = p + ring * 0.05 - vec2(-0.38, 0.40);
  wp = rot(0.5) * wp;
  float win = smoothstep(0.10, -0.02, max(abs(wp.x) - 0.24, abs(wp.y) - 0.15));
  float mull = smoothstep(0.02, 0.0, abs(wp.x)) + smoothstep(0.02, 0.0, abs(wp.y));
  liq += (win * (1.0 - clamp(mull, 0.0, 1.0)) * 0.16) * hexc(255.0, 236.0, 210.0);
  liq += ring * 0.10;
  // meniscus darkening against the wall
  liq *= 0.75 + 0.25 * smoothstep(1.0, 0.94, r);
  float inLiq = smoothstep(1.0 + aa, 1.0 - aa, r);
  col = mix(col, liq, inLiq);
  // the falling drop (a bright bead) just before impact
  float db = smoothstep(0.045, 0.035, length(p - vec2(0.0, 0.0))) * step(drop - 0.2, t) * step(t, drop);
  col = mix(col, hexc(60.0, 30.0, 14.0) + 0.25, db);
  o = vec4(col, 1.0);
}
