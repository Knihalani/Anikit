// The quench, raymarched: a glowing Damascus blade drops into black oil. The oil
// mirrors it (with ripples), the submerged part boils, and a column of steam,
// lit from below by the blade, rises while the steel cools to black.
uniform float dip;   // = 0.0
uniform float cool;  // = 0.0
uniform float steam; // = 0.0
uniform float heat;  // = 1.0
uniform float camA;  // = 0.35
uniform float camH;  // = 0.25
uniform float camD;  // = 2.6
uniform float camY;  // = 0.25
uniform float lens;  // = 1.5

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) { float s = 0.0; float a = 0.5; for (int i = 0; i < 5; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; } return s; }
vec3 bb(float h) { h = max(h, 0.0); return vec3(1.7 * h, 1.1 * h * h, 0.7 * h * h * h * h); }

float tipY() { return mix(0.28, -0.62, dip); }

// blade: tip down, edge towards +x, length along +y
float sdBlade(vec3 p) {
  p.xz = mat2(0.94, 0.34, -0.34, 0.94) * p.xz;
  p.xy = mat2(0.985, -0.17, 0.17, 0.985) * p.xy;
  float y = p.y - tipY();
  float k = clamp(y / 0.42, 0.0, 1.0);
  float hw = 0.19 * mix(k, sqrt(k), 0.65);
  float x = p.x + 0.07 * (1.0 - k) * (1.0 - k);
  float d2 = max(abs(x) - hw, -y);
  d2 = max(d2, y - 2.4);
  float th = 0.024 * clamp((hw - x) / max(2.0 * hw, 1e-3), 0.0, 1.0) + 0.0015;
  return max(d2, abs(p.z) - th) * 0.8;
}

float tempAt(vec3 p) {
  float crust = smoothstep(0.52, 0.78, 0.75 * fbm(p.xy * vec2(14.0, 9.0)) + 0.25 * fbm(p.xy * 31.0 + 5.0));
  float tt = heat * (1.0 - 0.45 * crust) * (1.05 - 0.4 * smoothstep(-0.3, 1.0, p.y));
  // a cooling front climbs the blade from the oil line
  float front = cool * 1.5 - 0.15 + 0.05 * fbm(p.xy * 12.0);
  tt *= smoothstep(front - 0.02, front + 0.12, p.y) * (1.0 - smoothstep(0.85, 1.0, cool));
  return tt;
}

vec3 envC(vec3 r) {
  float box = smoothstep(0.35, 0.8, r.y) * smoothstep(0.7, 0.2, abs(r.x + 0.2));
  float strip = smoothstep(0.02, 0.06, r.y) * smoothstep(0.2, 0.12, r.y) * smoothstep(0.9, 0.3, abs(r.x - 0.1));
  return vec3(0.6, 0.65, 0.75) * box * 1.2 + vec3(0.55, 0.6, 0.7) * strip * 1.5 + vec3(0.03);
}

float march(vec3 ro, vec3 rd, float tmax) {
  float t = 0.0;
  for (int i = 0; i < 80; i++) {
    float d = sdBlade(ro + rd * t);
    if (d < 0.0006) return t;
    t += d;
    if (t > tmax) break;
  }
  return -1.0;
}

vec3 bladeN(vec3 p) {
  vec2 e = vec2(0.001, -0.001);
  return normalize(e.xyy * sdBlade(p + e.xyy) + e.yyx * sdBlade(p + e.yyx) + e.yxy * sdBlade(p + e.yxy) + e.xxx * sdBlade(p + e.xxx));
}

vec3 shadeBlade(vec3 p, vec3 rd) {
  vec3 n = bladeN(p);
  vec3 r = reflect(rd, n);
  float s = 0.5 + 0.5 * sin((p.y + 0.05 * fbm(p.xy * 9.0) + 0.3 * p.x) * 55.0);
  float fre = pow(1.0 - max(dot(-rd, n), 0.0), 4.0);
  vec3 steel = envC(r) * mix(0.35, 1.0, s) * (0.6 + 0.6 * fre) + vec3(0.05) * s + vec3(0.12, 0.13, 0.15) * max(dot(n, normalize(vec3(-0.5, 0.6, 0.6))), 0.0);
  float tp = tempAt(p);
  float edge = smoothstep(0.0, 1.0, fre);
  tp *= 0.85 + 0.3 * edge;
  vec3 c = steel * smoothstep(0.7, 0.1, tp) + bb(tp) * 1.2 + envC(r) * 0.25 * fre;
  return c;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
  float t = iTime;
  vec3 tgt = vec3(0.0, camY, 0.0);
  vec3 ro = tgt + vec3(sin(camA) * camD, camH * camD, cos(camA) * camD);
  vec3 fw = normalize(tgt - ro); vec3 rt = normalize(cross(fw, vec3(0.0, 1.0, 0.0))); vec3 up = cross(rt, fw);
  vec3 rd = normalize(uv.x * rt + uv.y * up + lens * fw);
  float glowT = heat * (1.0 - cool);
  vec3 glowC = bb(glowT * 0.95);

  // backdrop: dark shop, faint cool window haze
  vec3 col = vec3(0.016, 0.017, 0.02) + vec3(0.1, 0.11, 0.14) * 0.3 * fbm(uv * 2.5 + t * 0.04) * smoothstep(-0.1, 0.5, uv.y);

  float tw = rd.y < 0.0 ? -ro.y / rd.y : 1e9;     // oil surface at y = 0
  float tb = march(ro, rd, min(tw, 8.0));
  if (tb > 0.0) {
    col = shadeBlade(ro + rd * tb, rd);
  } else if (tw < 1e8) {
    vec3 p = ro + rd * tw;
    float rr = length(p.xz);
    float ring = steam * exp(-rr * 2.5) * sin(rr * 55.0 - t * 10.0);
    vec3 n = normalize(vec3(p.x / max(rr, 1e-3) * ring * 0.08, 1.0, p.z / max(rr, 1e-3) * ring * 0.08));
    n = normalize(n + vec3(0.02 * (fbm(p.xz * 8.0 + t * 0.3) - 0.5), 0.0, 0.02 * (fbm(p.xz * 8.0 + 3.0 - t * 0.3) - 0.5)));
    vec3 r = reflect(rd, n);
    float fre = 0.12 + 0.88 * pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
    vec3 refl = envC(r) * 0.9;
    float tr = march(p + n * 0.002, r, 6.0);
    if (tr > 0.0) refl = shadeBlade(p + r * tr, r);
    // below: submerged blade through dark oil
    vec3 rdr = normalize(rd + n * 0.15);
    vec3 below = vec3(0.01, 0.011, 0.012);
    float ts = march(p - n * 0.002, rdr, 3.0);
    if (ts > 0.0) below = shadeBlade(p + rdr * ts, rdr) * vec3(0.45, 0.42, 0.38) * exp(-ts * 2.5);
    below += glowC * 0.05 / (0.02 + rr * rr * 6.0) * 0.12;
    refl += glowC * 0.02 / (0.01 + rr * rr * 8.0) * (1.0 + 0.5 * ring);
    col = mix(below, refl, fre);
    // boiling at the entry line
    float boil = steam * (1.0 - cool * 0.7) * smoothstep(0.25, 0.0, rr);
    float b = smoothstep(0.55, 0.9, fbm(p.xz * 40.0 + vec2(t * 3.0, -t * 2.0)));
    col += (vec3(0.5, 0.52, 0.55) * b * 0.6 + glowC * 0.4 * b) * boil;
    col = mix(col, vec3(0.016, 0.017, 0.02), 1.0 - exp(-0.02 * tw * tw));
  }
  // steam column rising from the entry point (projected to the screen)
  if (steam > 0.0) {
    vec3 e = vec3(0.0, 0.0, 0.0) - ro;
    vec2 es = vec2(dot(e, rt), dot(e, up)) * lens / dot(e, fw);
    vec2 sv = uv - es;
    float hgt = max(sv.y, 0.0);
    float wid = 0.08 + hgt * 0.8;
    vec2 sp = vec2(sv.x / wid * 0.6, hgt * 2.2 - t * 1.6);
    float d = fbm(sp * 1.8 + 1.4 * vec2(fbm(sp * 2.4 + vec2(0.0, -t * 0.7)), fbm(sp * 2.4 + 7.0)));
    float prof = exp(-pow(sv.x / wid, 2.0) * 1.5);
    float dens = smoothstep(0.26, 0.72, d) * prof * steam * smoothstep(-0.01, 0.05, sv.y) * exp(-hgt * 0.7);
    vec3 sc = mix(vec3(0.66, 0.68, 0.72), glowC * 1.3 + vec3(0.1), exp(-hgt * 6.0) * (1.0 - cool * 0.5));
    sc *= 0.6 + 0.6 * smoothstep(0.3, 0.9, d);
    col = mix(col, sc, clamp(dens * 2.3, 0.0, 0.95));
  }
  col *= 1.0 - 0.4 * dot(uv, uv);
  fragColor = vec4(col, 1.0);
}
