// The billet on the anvil: raymarched steel, a white-hot bar lighting everything
// around it (line light + glossy reflections), a hammer that lands on `lift` = 0.
uniform float heat;  // = 1.0
uniform float lift;  // = 1.0
uniform float flash; // = 0.0
uniform float camA;  // = 0.5
uniform float camH;  // = 0.6
uniform float camD;  // = 3.2
uniform float camX;  // = 0.0
uniform float lens;  // = 1.6
uniform float hammer; // = 1.0

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) { float s = 0.0; float a = 0.5; for (int i = 0; i < 4; i++) { s += a * vnoise(p); p = p * 2.07 + vec2(1.7, 9.2); a *= 0.5; } return s; }
vec3 bb(float h) { h = max(h, 0.0); return vec3(1.7 * h, 1.1 * h * h, 0.7 * h * h * h * h); }

float sdRBox(vec3 p, vec3 b, float r) { vec3 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - r; }
float sdCap(vec3 p, vec3 a, vec3 b, float r) { vec3 pa = p - a; vec3 ba = b - a; float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0); return length(pa - ba * h) - r; }

const vec3 BA = vec3(-1.15, 0.085, 0.0);
const vec3 BB = vec3(1.15, 0.085, 0.0);

vec3 headPos() { return vec3(0.25, 0.17 + 0.2 + lift * 1.5, 0.0); }

vec2 map(vec3 p) {
  // anvil: face, waist, horn
  float face = sdRBox(p - vec3(0.0, -0.22, 0.0), vec3(1.55, 0.22, 0.5), 0.03);
  float waist = sdRBox(p - vec3(0.0, -0.9, 0.0), vec3(0.75 - 0.25 * clamp(p.y + 0.9, -0.5, 0.5), 0.5, 0.36), 0.06);
  vec3 hp = p - vec3(-1.55, -0.2, 0.0);
  float horn = sdCap(hp, vec3(0.0), vec3(-1.0, 0.05, 0.0), 0.0) - mix(0.24, 0.02, clamp(-hp.x, 0.0, 1.0));
  float anvil = min(min(face, waist), horn);
  vec2 res = vec2(anvil, 1.0);
  // bar
  float bar = sdRBox(p - vec3(0.0, 0.085, 0.0), vec3(1.15, 0.075, 0.11), 0.035);
  if (bar < res.x) res = vec2(bar, 2.0);
  // hammer
  if (hammer > 0.5) {
    vec3 h = headPos();
    float head = sdRBox(p - h, vec3(0.2, 0.2, 0.34), 0.04);
    float handle = sdCap(p, h + vec3(0.0, 0.05, 0.0), h + vec3(1.7, 0.55, 0.9), 0.055);
    if (head < res.x) res = vec2(head, 3.0);
    if (handle < res.x) res = vec2(handle, 5.0);
  }
  // floor far below
  return res;
}

vec3 calcN(vec3 p) {
  vec2 e = vec2(0.0015, -0.0015);
  return normalize(e.xyy * map(p + e.xyy).x + e.yyx * map(p + e.yyx).x + e.yxy * map(p + e.yxy).x + e.xxx * map(p + e.xxx).x);
}

float barTemp(vec3 p) {
  float along = clamp(abs(p.x) / 1.15, 0.0, 1.0);
  float crust = smoothstep(0.5, 0.68, 0.45 * fbm(p.xz * vec2(13.0, 17.0)) + 0.35 * fbm(p.xy * vec2(11.0, 23.0) + 4.0) + 0.3 * vnoise(p.xz * 70.0 + p.y * 40.0));
  float t = heat * (0.98 - 0.5 * along * along) * mix(1.0, 0.45, crust);
  t += flash * 0.6 * exp(-8.0 * length(p.xz - vec2(0.25, 0.0)));
  return t;
}

vec3 glowAt(vec3 p, vec3 n) {
  vec3 ba = BB - BA; float h = clamp(dot(p - BA, ba) / dot(ba, ba), 0.0, 1.0);
  vec3 c = BA + ba * h; vec3 l = c - p; float d = length(l); l /= d;
  float w = max(dot(n, l), 0.0) / (1.0 + 10.0 * d * d);
  return bb(heat * 1.05) * w * 1.6 + vec3(1.0, 0.75, 0.45) * flash * 4.0 * max(dot(n, normalize(vec3(0.25, 0.1, 0.0) - p)), 0.0) / (1.0 + 18.0 * dot(p - vec3(0.25, 0.1, 0.0), p - vec3(0.25, 0.1, 0.0)));
}

// glossy reflection of the hot bar: distance from reflected ray to the bar segment
vec3 reflGlow(vec3 p, vec3 r, float rough) {
  vec3 u = BB - BA; vec3 w0 = p - BA;
  float a = dot(u, u); float b = dot(u, r); float c = 1.0; float d = dot(u, w0); float e = dot(r, w0);
  float den = a * c - b * b;
  float sc = clamp((a * e - b * d) / max(den, 1e-4), 0.0, 20.0);
  float tc = clamp((b * e - c * d) / max(den, 1e-4) * 1.0, 0.0, a) / a;
  vec3 q1 = p + r * sc; vec3 q2 = BA + u * tc;
  float dist = length(q1 - q2);
  float s = smoothstep(0.12 + rough * sc * 0.4, 0.0, dist) * step(0.001, sc);
  return bb(heat * 1.1) * s * 1.4;
}

vec3 env(vec3 r) {
  float top = smoothstep(0.55, 0.95, r.y) * smoothstep(-0.2, 0.6, r.x);
  return vec3(0.45, 0.52, 0.65) * top * 1.4 + vec3(0.6, 0.22, 0.05) * smoothstep(0.2, -0.6, r.x) * 0.25;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
  vec3 tgt = vec3(camX, 0.05, 0.0);
  vec3 ro = tgt + vec3(sin(camA) * camD, camH * camD * 0.6, cos(camA) * camD);
  ro += flash * 0.015 * vec3(sin(iTime * 90.0), cos(iTime * 77.0), 0.0);
  vec3 fw = normalize(tgt - ro); vec3 rt = normalize(cross(fw, vec3(0.0, 1.0, 0.0))); vec3 up = cross(rt, fw);
  vec3 rd = normalize(uv.x * rt + uv.y * up + lens * fw);

  // background: smoke lit from below by the forge
  vec2 su = uv * vec2(2.0, 2.4) + vec2(iTime * 0.06, -iTime * 0.12);
  float sm = fbm(su + fbm(su * 1.7));
  vec3 col = vec3(0.6, 0.2, 0.04) * 0.35 * exp(-3.0 * dot(uv - vec2(-0.75, 0.05), uv - vec2(-0.75, 0.05))) * heat;
  col += vec3(0.022, 0.016, 0.014) + vec3(0.5, 0.17, 0.04) * sm * sm * (0.35 + 0.4 * heat) * smoothstep(0.7, -0.4, uv.y);

  float t = 0.0; vec2 h = vec2(0.0);
  bool hit = false;
  for (int i = 0; i < 110; i++) {
    vec3 p = ro + rd * t;
    h = map(p);
    if (h.x < 0.0008 * t) { hit = true; break; }
    t += h.x * 0.9;
    if (t > 14.0) break;
  }
  if (hit) {
    vec3 p = ro + rd * t; vec3 n = calcN(p); vec3 r = reflect(rd, n);
    float fre = pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
    vec3 key = normalize(vec3(0.6, 1.0, 0.3));
    if (h.y < 1.5) {
      // anvil: dark forged steel, scuffed face
      float scuff = fbm(p.xz * 18.0) * 0.6 + fbm(p.xy * 6.0) * 0.4;
      float scratch = vnoise(vec2(p.x * 3.0 + p.z * 1.0, p.z * 260.0 - p.x * 40.0));
      float rough = clamp(0.12 + 0.55 * scuff - 0.12 * scratch, 0.05, 0.9);
      vec3 alb = vec3(0.03, 0.03, 0.034) * (0.5 + 0.9 * scuff);
      col = alb * (glowAt(p, n) * 7.0 + vec3(0.3, 0.36, 0.46) * max(dot(n, key), 0.0) * 1.4);
      col += vec3(1.0, 0.4, 0.12) * pow(max(dot(r, normalize(vec3(-1.0, 0.25, -0.6))), 0.0), 12.0) * 0.5 * (1.0 - rough);
      col += reflGlow(p, r, rough) * (0.35 + 0.65 * fre) * (1.1 - 0.7 * scuff) * (0.8 + 0.4 * scratch);
      col += env(r) * (0.08 + 0.5 * fre) * (1.0 - rough);
      col += glowAt(p, n) * 0.25 * (1.0 - rough);
    } else if (h.y < 2.5) {
      float tp = barTemp(p);
      col = bb(tp) * 1.25 + vec3(0.02) * max(dot(n, key), 0.0);
      col += vec3(1.0, 0.9, 0.7) * flash * 2.0 * exp(-10.0 * length(p.xz - vec2(0.25, 0.0)));
    } else if (h.y < 3.5) {
      vec3 alb = vec3(0.09, 0.09, 0.1);
      col = alb * (glowAt(p, n) * 8.0 + vec3(0.3, 0.35, 0.45) * max(dot(n, key), 0.0) * 1.6);
      col += vec3(1.0, 0.4, 0.12) * pow(max(dot(r, normalize(vec3(-1.0, 0.25, -0.6))), 0.0), 8.0) * 0.6;
      col += reflGlow(p, r, 0.15) * (0.35 + 0.65 * fre);
      col += env(r) * (0.5 + 0.8 * fre) + vec3(0.5, 0.55, 0.65) * pow(max(dot(r, key), 0.0), 30.0) * 0.6;
    } else if (h.y > 4.5) {
      float grain = vnoise(vec2(length(p.xz) * 6.0, (p.x + p.y) * 90.0));
      vec3 alb = mix(vec3(0.05, 0.025, 0.012), vec3(0.11, 0.06, 0.03), grain);
      col = alb * (glowAt(p, n) * 6.0 + vec3(0.3, 0.35, 0.45) * max(dot(n, key), 0.0)) + env(r) * 0.08 * fre;
    } else {
      col = vec3(0.02, 0.016, 0.014) * (0.3 + glowAt(p, n).r * 4.0);
    }
    float fog = 1.0 - exp(-0.03 * t * t);
    col = mix(col, vec3(0.03, 0.018, 0.012), fog);
  }
  // heat haze glow around the bar in screen space (volumetric-ish)
  vec3 ba = BB - BA; vec3 w0 = ro - BA;
  float b2 = dot(ba, rd); float dd = dot(ba, w0); float e2 = dot(rd, w0); float aa = dot(ba, ba);
  float den = aa - b2 * b2; float sc = (aa * e2 - b2 * dd) / max(den, 1e-4); float tc = clamp((b2 * e2 - dd) / max(den, 1e-4), 0.0, aa) / aa;
  float dist = length(ro + rd * max(sc, 0.0) - (BA + ba * tc));
  if (!hit || t > sc) col += bb(heat) * 0.05 / (0.02 + dist * dist * 6.0) * 0.15;
  col += vec3(1.0, 0.8, 0.5) * flash * 0.25;
  col *= 1.0 - 0.35 * dot(uv, uv);
  fragColor = vec4(col, 1.0);
}
