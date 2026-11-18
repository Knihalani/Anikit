// Arctic sea ice from above. The pack is built from Voronoi floes; the edge is a
// noisy contour whose radius follows the September extent (area ∝ extent, so
// radius ∝ sqrt(extent)). A dashed contour keeps the 1979 edge.
uniform float yr;    // = 1979.0
uniform float fixedExt; // = 0.0
const float D[48] = float[48](7.05,7.67,7.14,7.30,7.39,6.81,6.70,7.41,7.28,7.37,7.01,6.14,6.47,7.47,6.40,7.14,6.08,7.58,6.69,6.54,6.12,6.25,6.73,5.83,6.12,5.98,5.50,5.86,4.27,4.69,5.26,4.87,4.56,3.57,5.21,5.22,4.62,4.53,4.82,4.79,4.36,4.00,4.95,4.90,4.38,4.35,4.75,4.81);
uniform float ghost; // = 0.0
uniform float dim;   // = 0.0

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) {
  float s = 0.0; float a = 0.5;
  for (int i = 0; i < 5; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
  return s;
}
// edge field: distance-like value, compared against a radius
float edgeF(vec2 p, float shrink) {
  vec2 q = p - vec2(-0.05, 0.045) * shrink;          // the pack retreats toward Greenland / the Canadian archipelago
  float r = length(q * vec2(1.0, 1.08));
  return r + 0.10 * (fbm(p * 2.2 + vec2(4.0, 1.0)) - 0.5) + 0.035 * (fbm(p * 7.0 + 11.0) - 0.5);
}
float landF(vec2 p) {
  float r = length(p);
  float ring = r + 0.09 * (fbm(p * 2.6 + vec2(8.0, 3.0)) - 0.5) - 0.475;
  // straits: Fram (towards Greenland's east) and Bering
  float a = atan(p.y, p.x);
  ring -= 0.05 * smoothstep(0.92, 1.0, cos(a - 0.35)) + 0.06 * smoothstep(0.95, 1.0, cos(a + 1.75));
  vec2 g = (p - vec2(-0.40, 0.42)) * vec2(1.0, 0.8);
  float green = length(g) + 0.06 * (fbm(p * 5.0 + 2.0) - 0.5) - 0.15;
  float isl = length((p - vec2(-0.40, 0.16)) * vec2(0.8, 1.3)) + 0.05 * (fbm(p * 6.0 + 5.0) - 0.5) - 0.11;
  float sv = length(p - vec2(0.10, 0.50)) + 0.03 * (fbm(p * 9.0) - 0.5) - 0.06;
  return min(min(-ring, green), min(isl, sv)); // < 0 means land
}

void mainImage(out vec4 o, in vec2 fc) {
  vec2 R = iResolution.xy;
  vec2 c = vec2(0.355 * R.x, 0.5 * R.y);
  vec2 p = (fc - c) / R.y;
  p.y = -p.y;
  float yc = clamp(yr - 1979.0, 0.0, 47.0);
  int k0 = int(floor(yc)); int k1 = min(k0 + 1, 47);
  float ext = mix(D[k0], D[k1], smoothstep(0.0, 1.0, fract(yc)));
  if (fixedExt > 0.0) { ext = fixedExt; }
  float rad = 0.355 * sqrt(ext / 7.05);
  float shrink = clamp((7.3 - ext) / 3.7, 0.0, 1.0);

  // ocean: deep cold blue, slow swell texture
  float sw = fbm(p * 18.0 + vec2(iTime * 0.05, 0.0));
  vec3 col = mix(vec3(0.020, 0.050, 0.085), vec3(0.035, 0.085, 0.130), smoothstep(0.75, 0.0, length(p)));
  col *= 0.85 + 0.3 * sw;

  // floes: Voronoi cells, gently drifting clockwise (Beaufort gyre)
  float ang = -iTime * 0.006;
  vec2 pr = vec2(cos(ang) * p.x - sin(ang) * p.y, sin(ang) * p.x + cos(ang) * p.y);
  vec2 cp = pr * 64.0;
  vec2 n = floor(cp); vec2 f = fract(cp);
  float d1 = 8.0; float d2 = 8.0; vec2 cell = vec2(0.0); vec2 cen = vec2(0.0);
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 off = vec2(hash(n + g), hash(n + g + 19.1));
      vec2 rr = g + off - f;
      float d = dot(rr, rr);
      if (d < d1) { d2 = d1; d1 = d; cell = n + g; cen = rr; } else if (d < d2) { d2 = d; }
    }
  }
  float edgeDist = sqrt(d2) - sqrt(d1);
  vec2 cellP = (cp + cen) / 64.0;   // cell centre, rotated frame
  vec2 cellW = vec2(cos(-ang) * cellP.x - sin(-ang) * cellP.y, sin(-ang) * cellP.x + cos(-ang) * cellP.y);
  float rnd = hash(cell + 3.7);
  float eC = edgeF(cellW, shrink);
  float eP = edgeF(p, shrink);
  float margin = smoothstep(rad - 0.09, rad + 0.01, eC);       // 0 in the pack, 1 at the edge
  float keep = step(eC, rad + 0.03 * (rnd - 0.5) - 0.012 * margin * rnd);
  float crack = -0.08 + 0.42 * margin * (0.4 + rnd);
  float floe = keep * smoothstep(crack, crack + 0.05, edgeDist);
  // thin grease ice beyond the floes
  float grease = smoothstep(rad + 0.035, rad - 0.01, eP) * (1.0 - floe) * 0.22;

  // ice surface: snow texture, per-floe tone, raking light from the upper right
  float snow = fbm(p * 60.0 + cell * 0.3);
  float light = 0.86 + 0.16 * clamp(dot(normalize(vec2(0.7, 0.7)), p) * 2.5 + 0.5, 0.0, 1.0);
  float bevel = mix(0.97 + 0.03 * smoothstep(0.03, 0.15, edgeDist), 0.80 + 0.20 * smoothstep(0.03, 0.22, edgeDist), margin);
  vec3 ice = mix(vec3(0.80, 0.87, 0.92), vec3(0.97, 0.985, 1.0), snow);
  ice *= (0.92 + 0.08 * rnd) * light * bevel;
  ice = mix(ice, vec3(0.55, 0.70, 0.80), 0.35 * margin); // thinner, bluer ice at the margin
  ice *= 1.0 - 0.10 * (1.0 - smoothstep(0.0, 0.035, edgeDist)) * (1.0 - margin);
  col = mix(col, ice, floe);
  col = mix(col, vec3(0.30, 0.42, 0.52), grease);
  // floe shadow on water (sun from upper right)
  vec2 sp = p + vec2(0.004, 0.004);
  float shade = smoothstep(rad + 0.01, rad - 0.02, edgeF(sp, shrink));
  col *= 1.0 - 0.25 * shade * (1.0 - floe);

  // land: dark, matte, with a faint relief
  float lf = landF(p);
  float land = smoothstep(0.003, -0.003, lf);
  float rel = fbm(p * 24.0 + 40.0);
  vec3 landC = vec3(0.085, 0.080, 0.074) * (0.85 + 0.3 * rel);
  col = mix(col, landC, land);
  float coast = smoothstep(0.004, 0.0, abs(lf));
  col += vec3(0.10, 0.12, 0.13) * coast * 0.6;

  // the 1979 edge, dashed
  float rad79 = 0.355;
  float line = smoothstep(0.0022, 0.0006, abs(edgeF(p, 0.0) - rad79));
  float dash = step(0.45, fract(atan(p.y, p.x) * 34.0 / 6.28318));
  col = mix(col, vec3(1.0, 0.86, 0.70), line * dash * ghost * 0.85 * (1.0 - land));

  // grade: vignette, dim, grain
  float v = smoothstep(1.15, 0.2, length((fc / R - vec2(0.42, 0.5)) * vec2(1.4, 1.0)));
  col *= mix(0.55, 1.0, v);
  col *= 1.0 - dim;
  col += (hash(fc + fract(iTime * 7.0) * 100.0) - 0.5) * 0.025;
  o = vec4(col, 1.0);
}
