// Shot 4: a cup of black tea from above, amber and clear, a leaf turning in it.
const vec2 C = vec2(540.0, 1100.0);
const float R = 305.0;
void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  vec2 es = enterSpin(t, -0.9);
  vec2 p0 = (f - C) / R;
  vec3 col = field(f, hexc(226.0, 160.0, 52.0));
  float sh = smoothstep(1.65, 1.2, length(p0 - vec2(0.13, -0.17)));
  col *= 1.0 - 0.45 * sh;
  vec2 p = rot(es.x) * p0 / es.y;
  float r = length(p);
  float aa = 1.6 / R;
  vec2 L = normalize(vec2(-0.55, 0.83));
  // saucer
  float sau = smoothstep(1.46 + aa, 1.46 - aa, r);
  vec3 sc = hexc(246.0, 240.0, 228.0) * (0.86 + 0.1 * dot(p / max(r,1e-3), L));
  sc *= 1.0 - 0.3 * smoothstep(1.35, 1.12, length(p - vec2(0.07, -0.09)));
  sc *= 1.0 - 0.10 * smoothstep(1.22, 1.30, r) * smoothstep(1.40, 1.30, r);
  // a thin blue band on the saucer and cup: porcelain
  float band = smoothstep(0.012, 0.0, abs(r - 1.40));
  sc = mix(sc, hexc(40.0, 70.0, 130.0), band * 0.85);
  col = mix(col, sc, sau);
  float cupR = 1.17;
  float x = clamp((r - 1.0) / 0.17, 0.0, 1.0);
  float side = dot(p / max(r, 1e-3), L);
  float lipc = smoothstep(0.0, 0.35, x) * (1.0 - smoothstep(0.75, 1.0, x));
  vec3 cer = hexc(246.0, 240.0, 228.0) * (0.80 + 0.2 * lipc + 0.1 * side * (x - 0.5) - 0.18 * (1.0 - smoothstep(0.0, 0.25, x)) * max(side, 0.0));
  cer += smoothstep(0.92, 1.0, side) * lipc * 0.5;
  cer = mix(cer, hexc(40.0, 70.0, 130.0), smoothstep(0.012, 0.0, abs(r - 1.12)) * 0.85);
  col = mix(col, cer, smoothstep(cupR + aa, cupR - aa, r));
  // tea: deep red-amber, lighter toward the wall where light bounces off the porcelain
  float depth = smoothstep(1.0, 0.0, r);
  vec3 tea = mix(hexc(196.0, 92.0, 24.0), hexc(92.0, 26.0, 8.0), pow(depth, 0.6));
  // caustic shimmer
  float cs = fbm(rot(0.2 * t) * p * 6.0 + vec2(0.0, t * 0.3));
  tea += hexc(255.0, 170.0, 60.0) * smoothstep(0.62, 0.8, cs) * 0.18 * (1.0 - depth);
  // one leaf turning slowly
  vec2 lp = rot(0.6 + 0.25 * t) * (p - vec2(0.22 + 0.05 * sin(t), -0.18));
  float leaf = length(lp / vec2(0.26, 0.09)) - 1.0 + 0.3 * lp.x * lp.x;
  float vein = smoothstep(0.006, 0.0, abs(lp.y + 0.02 * sin(lp.x * 8.0))) * step(leaf, 0.0);
  vec3 lc = mix(hexc(70.0, 40.0, 14.0), hexc(110.0, 70.0, 26.0), noise(lp * 30.0));
  tea = mix(tea * 0.8, tea, smoothstep(-0.02, 0.15, length((lp + vec2(0.03, 0.03)) / vec2(0.26, 0.09)) - 1.0)); // its shadow
  tea = mix(tea, lc, smoothstep(0.03, -0.03, leaf));
  tea = mix(tea, lc * 1.5, vein * 0.6);
  // window reflection
  vec2 wp = rot(0.5) * (p - vec2(-0.38, 0.40));
  float win = smoothstep(0.10, -0.02, max(abs(wp.x) - 0.24, abs(wp.y) - 0.15));
  float mull = clamp(smoothstep(0.02, 0.0, abs(wp.x)) + smoothstep(0.02, 0.0, abs(wp.y)), 0.0, 1.0);
  tea += win * (1.0 - mull) * 0.20 * hexc(255.0, 240.0, 215.0);
  tea *= 0.8 + 0.2 * smoothstep(1.0, 0.94, r);
  col = mix(col, tea, smoothstep(1.0 + aa, 1.0 - aa, r));
  o = vec4(col, 1.0);
}
