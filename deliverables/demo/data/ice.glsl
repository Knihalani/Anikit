// Arctic pack ice seen from straight above, lit by a sun that sinks over the film.
// `cover` is the share of the frame that is ice (calibrated: extent / 7.05 for the data beats).
// Floes are Voronoi cells; each has a random melt threshold biased toward the ice edge
// (bottom of frame = open Atlantic side), so the water opens from one side like a real pack.
uniform float cover;   // = 1.0
uniform float sun;     // = 0.0   0 high white daylight .. 1 low amber sun
uniform float scale;   // = 7.0   floes across the frame width
uniform float crack;   // = 0.0   leads widening through the pack
uniform float panx;    // = 0.0
uniform float pany;    // = 0.0
uniform float dusk;    // = 0.0   darkens everything toward night
uniform float chart;   // = 0.0   1: the pack is cut into 48 bars, one per September
uniform float grow;    // = 48.0  bars revealed (fractional)
uniform float hl;      // = -1.0  highlighted bar index
uniform float edge;    // = 0.0   direction the water opens from, degrees (0 bottom, -90 right)
uniform float yi;      // = -1.0  >= 0: cover follows the data, index into D (fractional = blend)
const float CF[11] = float[11](0.0, 0.015, 0.06, 0.13, 0.24, 0.34, 0.48, 0.62, 0.74, 0.85, 0.92);
float coverOf(float ext) {
  float f = ext / 8.0;
  float c = 1.0;
  for (int k = 0; k < 10; k++) {
    if (f >= CF[k] && f < CF[k + 1]) { c = (float(k) + (f - CF[k]) / (CF[k + 1] - CF[k])) * 0.1; }
  }
  return c;
}
const float D[48] = float[48](7.05,7.67,7.14,7.30,7.39,6.81,6.70,7.41,7.28,7.37,7.01,6.14,6.47,7.47,6.40,7.14,6.08,7.58,6.69,6.54,6.12,6.25,6.73,5.83,6.12,5.98,5.50,5.86,4.27,4.69,5.26,4.87,4.56,3.57,5.21,5.22,4.62,4.53,4.82,4.79,4.36,4.00,4.95,4.90,4.38,4.35,4.75,4.81);

float hash1(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
vec2 hash2(vec2 p) { return vec2(hash1(p), hash1(p + 17.31)); }
float vnoise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash1(i), hash1(i + vec2(1.0, 0.0)), u.x), mix(hash1(i + vec2(0.0, 1.0)), hash1(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) {
  float s = 0.0; float a = 0.5;
  for (int i = 0; i < 4; i++) { s += a * vnoise(p); p = p * 2.07 + vec2(1.7, 9.2); a *= 0.5; }
  return s;
}

// returns: x = distance to the cell edge, y = cell random, z = melt threshold of the cell
vec3 floe(vec2 p, float edgeBias) {
  vec2 ip = floor(p); vec2 fp = fract(p);
  float d1 = 9.0; vec2 c1 = vec2(0.0); vec2 o1 = vec2(0.0);
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 o = g + 0.15 + 0.7 * hash2(ip + g);
      float d = dot(o - fp, o - fp);
      if (d < d1) { d1 = d; c1 = ip + g; o1 = o; }
    }
  }
  float e = 9.0;
  for (int j = -2; j <= 2; j++) {
    for (int i = -2; i <= 2; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 o = g + 0.15 + 0.7 * hash2(ip + g);
      vec2 dd = o - o1;
      float l = dot(dd, dd);
      if (l > 0.0001) { e = min(e, dot(0.5 * (o1 + o) - fp, dd / sqrt(l))); }
    }
  }
  float r = hash1(c1 + 3.7);
  return vec3(e, r, r);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  float cov = cover;
  if (yi >= 0.0) {
    float k0 = floor(yi); int i0 = int(min(k0, 47.0)); int i1 = int(min(k0 + 1.0, 47.0));
    float fr = yi - k0; fr = fr * fr * (3.0 - 2.0 * fr);
    cov = coverOf(mix(D[i0], D[i1], fr));
  }
  vec2 uv = fragCoord / iResolution.xy;           // 0..1, y up
  float asp = iResolution.x / iResolution.y;
  vec2 p = vec2(uv.x * asp, uv.y) * (scale / asp) + vec2(panx, pany);
  // gentle drift of the whole pack
  p += vec2(0.035, 0.012) * iTime;

  // light
  vec3 L = normalize(mix(vec3(-0.35, 0.45, 0.82), vec3(-0.85, 0.42, 0.18), sun));
  vec3 sunCol = mix(vec3(1.0, 0.99, 0.97), vec3(1.0, 0.84, 0.66), sun);
  vec3 skyCol = mix(vec3(0.62, 0.74, 0.86), vec3(0.42, 0.50, 0.68), sun);

  // ---- floes: bigger cells plus a finer breakup at the ice edge
  vec2 warp = vec2(fbm(p * 0.9 + 3.1), fbm(p * 0.9 + 8.4)) - 0.5;
  vec2 q = p + warp * 0.55;
  vec3 f = floe(q, 0.0);
  float edgeDist = f.x;                     // cell units
  // ice-edge bias: lower frame (uv.y small) melts first
  vec2 ed = vec2(sin(radians(edge)), cos(radians(edge)));
  float ev = dot(vec2(uv.x * asp, uv.y) - vec2(0.5 * asp, 0.5), ed) / (abs(ed.x) * asp + abs(ed.y)) + 0.5;
  float grad = clamp(1.0 - ev * 1.05 + 0.25 * (fbm(p * 0.35 + 2.0) - 0.5), 0.0, 1.0);
  float thr = mix(f.y, grad, 0.72);         // 0..1, low = most robust
  // melt: floes whose threshold sits just under cov are partly eaten from their edges
  float w = 0.16;
  float margin = (cov - thr) / w;         // >1 solid, 0..1 shrinking, <0 gone
  float erode = 0.012 + 0.5 * clamp(1.0 - margin, 0.0, 1.0) + crack * 0.12;
  float rough = 0.05 * (fbm(q * 9.0) - 0.5);
  float sd = edgeDist - erode + rough;      // >0 inside ice
  float alive = step(0.0, margin + 0.999) * step(-0.999, -0.0); // margin > -1
  if (margin < 0.0) sd = -1.0;
  float isHl = 0.0;
  if (chart > 0.5) {
    float x0 = 0.075; float x1 = 0.925; float base = 0.14; float top = 0.80;
    float fx = (uv.x - x0) / (x1 - x0) * 48.0;
    float fi = clamp(floor(fx), 0.0, 47.0);
    int i = int(fi);
    float lx = fx - fi;
    float gi = clamp((grow - fi) * 1.0, 0.0, 1.0); gi = gi * gi * (3.0 - 2.0 * gi);
    float hgt = D[i] / 7.67 * (top - base) * gi;
    float colw = (x1 - x0) / 48.0 * asp;
    float dx = min(lx - 0.10, 0.90 - lx) * colw;
    if (fx < 0.0 || fx >= 48.0) dx = -1.0;
    float dy = min(uv.y - base, base + hgt - uv.y);
    float sdBar = min(dx, dy) * scale + (0.006 * (fbm(q * 9.0 + fi * 3.1) - 0.5) + 0.002 * (fbm(q * 40.0) - 0.5)) * scale;
    vec3 fb = floe(q + vec2(fi * 5.3, fi * 2.1), 0.0); edgeDist = fb.x;
    if (hgt < 0.001) sdBar = -1.0;
    sd = min(sdBar, edgeDist - 0.006 + rough * 0.3);
    margin = 2.0;
    isHl = step(abs(fi - hl), 0.5);
  }
  // brash: small fragments in the gaps near the edge
  vec3 f2 = floe(q * 4.3 + 11.0, 0.0);
  float brashAmt = clamp(1.0 - abs(margin - 0.2) * 1.6, 0.0, 1.0) * step(f2.y, 0.33) * step(0.05, cov);
  float sdB = (f2.x - 0.14 - 0.2 * fbm(q * 11.0)) / 4.3;
  sdB = (brashAmt > 0.0 && chart < 0.5) ? sdB : -1.0;
  float px = scale / iResolution.x * 1.3;
  float ice = smoothstep(-px, px, sd);
  float iceB = smoothstep(-px, px, sdB) * (1.0 - ice);

  // ---- ice surface: snow height field
  float h = fbm(q * 5.0) * 0.6 + fbm(q * 23.0) * 0.25 + 0.15 * smoothstep(0.08, 0.0, abs(fbm(q * 2.3 + 5.0) - 0.5)); // ridges
  float e = 0.002 * scale;
  float hx = fbm((q + vec2(e, 0.0)) * 5.0) * 0.6 + fbm((q + vec2(e, 0.0)) * 23.0) * 0.25;
  float hy = fbm((q + vec2(0.0, e)) * 5.0) * 0.6 + fbm((q + vec2(0.0, e)) * 23.0) * 0.25;
  vec3 n = normalize(vec3(-(hx - (fbm(q * 5.0) * 0.6 + fbm(q * 23.0) * 0.25)) / e * 0.016, -(hy - (fbm(q * 5.0) * 0.6 + fbm(q * 23.0) * 0.25)) / e * 0.016, 1.0));
  float diff = clamp(0.62 + 2.2 * (dot(n, L) - L.z), 0.0, 1.0);
  vec3 snow = vec3(0.93, 0.96, 0.99);
  vec3 iceCol = snow * (sunCol * (0.34 + 0.62 * diff) + skyCol * 0.22);
  // melt ponds: turquoise patches on floes in summer
  float pond = smoothstep(0.66, 0.70, fbm(q * 3.1 + 21.0)) * (0.5 + 0.5 * clamp(1.0 - margin, 0.0, 1.0));
  iceCol = mix(iceCol, vec3(0.30, 0.62, 0.74) * (0.6 + 0.5 * sunCol), pond * 0.75);
  // thin, blue rim at the floe edge (the ice is translucent where it is thin)
  float rim = smoothstep(0.07, 0.0, sd);
  iceCol = mix(iceCol, vec3(0.55, 0.80, 0.90) * (0.7 + 0.3 * sunCol), rim * 0.55);

  // ---- water: dark, with sky reflection and wind ripples
  float rip = fbm(p * vec2(26.0, 34.0) + vec2(iTime * 0.5, iTime * 0.2));
  vec3 deep = mix(vec3(0.020, 0.055, 0.085), vec3(0.025, 0.030, 0.060), sun);
  vec3 water = deep + skyCol * (0.05 + 0.03 * rip);
  // sun glint, strongest with a low sun
  float glint = pow(clamp(rip, 0.0, 1.0), 9.0) * (0.15 + 1.2 * sun);
  water += sunCol * glint * 0.5;
  // submerged ice foot: turquoise halo under the water around each floe
  float foot = smoothstep(-0.10, 0.0, sd) * (1.0 - ice);
  water = mix(water, vec3(0.10, 0.36, 0.44), foot * 0.6);
  // shadow cast on the water by the floe (offset away from the sun)
  vec3 fs = floe(q + L.xy * 0.045 / max(L.z, 0.2), 0.0);
  float thrS = mix(fs.y, grad, 0.72);
  float sdS = fs.x - (0.012 + 0.5 * clamp(1.0 - (cov - thrS) / w, 0.0, 1.0));
  float shadow = (cov - thrS) > 0.0 ? smoothstep(-0.01, 0.02, sdS) : 0.0;
  water *= 1.0 - 0.55 * shadow * (1.0 - ice) * (1.0 - chart);

  vec3 brashCol = mix(iceCol, vec3(0.55, 0.78, 0.88), 0.35) * 0.9;
  vec3 col = mix(water, brashCol, iceB);
  col = mix(col, iceCol, ice);
  col = mix(col, vec3(1.0, 0.353, 0.122) * (0.55 + 0.45 * diff), isHl * ice * 0.9);
  col *= mix(vec3(1.0), vec3(0.42, 0.47, 0.70), dusk);
  float wa = clamp(1.0 - ice - iceB, 0.0, 1.0);
  fragColor = hl < -1.5 ?  // hl = -2: water only (alpha), laid over paint so leads cut it
     vec4(col * wa, wa) : vec4(col, 1.0);
}
