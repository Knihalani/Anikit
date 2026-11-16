// Ray-marched liquid metal on a dark glossy table, seen from above.
// World units: 1 = viewport height. x grows right, y grows up, z points at the viewer.
// The table is the plane z = 0; everything sits on it.
export const MAX_DROPS = 20;

export const vert = `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`;

export const frag = `#version 300 es
precision highp float;
#define ND ${MAX_DROPS}
uniform vec2 uRes;        // backing pixels
uniform float uTime;
uniform vec4 uD[ND];      // drops: x, y, radius, unused (world)
uniform sampler2D uTex;   // wordmark distance field (texels, signed)
uniform vec4 uTR;         // wordmark centre x, y, half width, half height
uniform vec3 uTA;         // thickness, erosion, texel → world scale
uniform float uMelt;
uniform vec4 uRimA; uniform vec4 uRimAP;   // centre, half size; corner, thickness, plate
uniform vec4 uRimB; uniform vec4 uRimBP;
uniform vec4 uNeck; uniform float uNeckR;
uniform vec4 uPool;       // centre x, y, radius x, height
uniform vec2 uLight;      // pointer parallax, -1..1
uniform float uK;         // how readily drops merge
uniform vec4 uBar[3];     // liquid bars: x0, y, x1, radius
uniform sampler2D uTex2;  // the split-text word, as a distance field
uniform vec4 uWR;         // word centre x, y, half width, half height
uniform vec3 uWA;         // thickness, texel → world scale, on
uniform vec3 uWP;         // word pool: centre x, baseline y, half width
uniform float uRise[8];   // how far each letter has risen out of the pool
uniform float uEdge[9];   // letter boundaries across the word (texture u)
out vec4 outColor;

float smin(float a, float b, float k) { float h = max(k - abs(a - b), 0.0) / k; return min(a, b) - h * h * k * 0.25; }
float sdRR(vec2 p, vec2 b, float r) { vec2 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r; }
float sdBox(vec2 p, vec2 b) { vec2 q = abs(p) - b; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0); }
float hash(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }

// ---- Per-pixel culling: only what can reach this pixel (or its shadow) is evaluated.
uint gMask;
bool gText, gWord, gRimA, gRimB, gNeck, gBars, gPool;
const float REACH = 0.15;
void cull(vec2 p) {
  gMask = 0u;
  for (int i = 0; i < ND; i++) {
    vec4 D = uD[i];
    if (D.z > 0.0 && length(p - D.xy) - D.z * 1.6 < REACH) gMask |= (1u << uint(i));
  }
  gText = uTA.x > 0.0008 && sdBox(p - uTR.xy, uTR.zw + vec2(0.0, uMelt * uTR.w * 1.2)) < REACH;
  gWord = uWA.z > 0.001 && sdBox(p - uWR.xy, uWR.zw) < REACH;
  gRimA = uRimAP.y > 0.0006 && abs(sdRR(p - uRimA.xy, uRimA.zw, uRimAP.x)) < REACH;
  gRimB = uRimBP.y > 0.0006 && abs(sdRR(p - uRimB.xy, uRimB.zw, uRimBP.x)) < REACH;
  gNeck = uNeckR > 0.0005;
  gBars = uBar[0].w + uBar[1].w + uBar[2].w > 0.0;
  gPool = uPool.z > 0.001 && length((p - uPool.xy) / vec2(uPool.z, uPool.z * 0.42)) < 1.0 + REACH / uPool.z * 2.0;
}
bool anything() { return gMask != 0u || gText || gWord || gRimA || gRimB || gNeck || gBars || gPool; }

float textD(vec2 p) {
  // Melting: each column stretches downward by its own amount and the letters thin out.
  float top = uTR.y + uTR.w;
  float col = 0.5 + 0.5 * sin(p.x * 13.0 + 1.7) * sin(p.x * 5.0 - 0.4);
  float a = uMelt * (0.12 + 0.9 * col * col);
  if (p.y > top) a = 0.0;
  p.y = top - (top - p.y) / (1.0 + a);
  vec2 uv = (p - uTR.xy) / (2.0 * uTR.zw) + 0.5;
  float d = texture(uTex, clamp(uv, 0.0, 1.0)).r * uTA.z;
  float outside = sdBox(p - uTR.xy, uTR.zw);
  return max(d, outside) / (1.0 + a);
}

// Each letter rises out of a pool along the baseline, and later sinks back into it.
float wordD(vec2 p) {
  vec2 uv = (p - uWR.xy) / (2.0 * uWR.zw) + 0.5;
  int k = 0;
  for (int i = 1; i < 8; i++) if (uv.x > uEdge[i]) k = i;
  float sink = 1.0 - uRise[k];
  // A sinking letter slides down into the pool and thins away.
  vec2 q = p + vec2(0.0, sink * uWR.w * 1.1);
  uv = (q - uWR.xy) / (2.0 * uWR.zw) + 0.5;
  float d = texture(uTex2, clamp(uv, 0.0, 1.0)).r * uWA.y;
  float outside = sdBox(q - uWR.xy, uWR.zw);
  // Hidden below the pool surface.
  float below = uWP.y - p.y;
  return max(max(d, outside) + sink * uWA.x * 3.0, below);
}

float rim(vec3 q, vec4 R, vec4 P) {
  float b = sdRR(q.xy - R.xy, R.zw, P.x);
  // Two slugs of metal run round the ring, so the frame looks poured rather than drawn.
  vec2 rp = (q.xy - R.xy) / R.zw;
  float ang = atan(rp.y, rp.x) / 6.2831853;
  float w1 = fract(ang - uTime * 0.07) - 0.5, w2 = fract(ang + 0.5 - uTime * 0.07) - 0.5;
  float slug = exp(-w1 * w1 * 160.0) + exp(-w2 * w2 * 160.0);
  float th = P.y * (1.0 + 0.07 * sin(q.x * 29.0 + uTime * 1.1) * sin(q.y * 23.0 - uTime * 0.9) + 0.7 * slug);
  return length(vec2(b, q.z - th * 0.15)) - th;
}

float map(vec3 q) {
  float d = 1e5;
  if (gText) {
    float d2 = textD(q.xy) + uTA.y;
    float T = uTA.x;
    d = length(vec2(max(d2 + T, 0.0), q.z)) - T;
  }
  if (gWord) {
    float T = uWA.x;
    float dw = length(vec2(max(wordD(q.xy) + T, 0.0), q.z)) - T;
    // The pool: a shallow puddle along the baseline.
    float pb = sdRR(q.xy - vec2(uWP.x, uWP.y), vec2(uWP.z, T * 0.9), T * 0.9);
    float dp = length(vec2(max(pb + T * 0.5, 0.0), q.z)) - T * 0.5;
    d = min(d, smin(dw, dp, T * 1.4));
  }
  if (gMask != 0u) for (int i = 0; i < ND; i++) {
    if ((gMask & (1u << uint(i))) == 0u) continue;
    vec4 D = uD[i];
    vec3 c = vec3(D.xy, D.z * 0.3);
    float e = (length(vec3(q.xy - c.xy, (q.z - c.z) * 1.45)) - D.z) / 1.45;
    d = smin(d, e, uK);
  }
  if (gRimA) d = smin(d, rim(q, uRimA, uRimAP), 0.03);
  if (gRimB) d = smin(d, rim(q, uRimB, uRimBP), 0.03);
  if (gNeck) {
    vec3 a = vec3(uNeck.xy, uNeckR * 0.5), b = vec3(uNeck.zw, uNeckR * 0.5);
    vec3 pa = q - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    float r = uNeckR * (1.0 - 0.45 * sin(h * 3.14159));
    d = smin(d, length(pa - ba * h) - r, 0.04);
  }
  if (gBars) for (int i = 0; i < 3; i++) {
    vec4 B = uBar[i];
    if (B.w <= 0.0) continue;
    vec3 a = vec3(B.x, B.y, B.w * 0.55), b = vec3(max(B.z, B.x), B.y, B.w * 0.55);
    vec3 pa = q - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    vec3 e = pa - ba * h;
    d = min(d, (length(vec3(e.xy, e.z * 1.3)) - B.w) / 1.3);
  }
  if (gPool) {
    vec3 r = vec3(uPool.z, uPool.z * 0.42, uPool.w);
    vec3 p = q - vec3(uPool.xy, 0.0);
    float k0 = length(p / r), k1 = length(p / (r * r));
    d = smin(d, k0 * (k0 - 1.0) / k1, 0.05);
  }
  return d;
}

float softBox(vec2 p, vec2 b, float s) { return 1.0 - smoothstep(-s, s, sdBox(p, b)); }
float spot(vec2 p, float r, float s) { return 1.0 - smoothstep(r - s, r + s, length(p)); }

// The studio, in linear HDR: large soft boxes with crisp edges, a few small hot lights,
// warm and cool rim lights on the horizon, a dark room and a dark floor below.
vec3 env(vec3 d, float rough) {
  float up = d.z;
  vec3 c = vec3(0.006, 0.0065, 0.008) + vec3(0.035, 0.038, 0.046) * smoothstep(-0.1, 1.0, up);
  if (up > 0.03) {
    vec2 pp = d.xy / up - uLight * 0.22;
    float s = 0.008 + rough;
    c += vec3(1.0, 0.98, 0.95) * 6.0 * softBox(pp - vec2(0.05, 0.42), vec2(0.62, 0.075), s);
    c += vec3(1.0) * 0.45 * softBox(pp - vec2(-0.12, -0.05), vec2(0.30, 0.20), s + 0.1);
    c += vec3(0.86, 0.92, 1.0) * 3.5 * softBox(pp - vec2(-0.82, 0.0), vec2(0.06, 0.75), s);
    c += vec3(1.0, 0.95, 0.88) * 2.6 * softBox(pp - vec2(0.95, -0.2), vec2(0.045, 0.6), s);
    c += vec3(0.95, 0.97, 1.0) * 3.0 * softBox(pp - vec2(0.1, -0.62), vec2(0.5, 0.03), s);
    // Small hot lights: tiny, blown-out highlights are what make chrome read as chrome.
    if (rough < 0.05) {
      c += vec3(1.0, 0.96, 0.9) * 40.0 * spot(pp - vec2(0.55, 0.25), 0.035, s);
      c += vec3(0.9, 0.95, 1.0) * 30.0 * spot(pp - vec2(-0.45, 0.7), 0.025, s);
      c += vec3(1.0) * 25.0 * spot(pp - vec2(-0.3, -0.38), 0.02, s);
    }
    // A cool-to-warm gradient across the studio walls.
    c += mix(vec3(0.05, 0.07, 0.11), vec3(0.11, 0.08, 0.05), smoothstep(-1.5, 1.5, pp.x)) * smoothstep(0.03, 0.5, up);
  }
  float az = atan(d.y, d.x);
  float hor = exp(-pow((up - 0.1) / (0.12 + rough * 0.3), 2.0));
  c += vec3(1.0, 0.70, 0.42) * 1.6 * hor * smoothstep(0.55, 0.0, abs(az - 2.5));
  c += vec3(0.42, 0.62, 1.0) * 1.3 * hor * smoothstep(0.6, 0.0, abs(az + 0.7));
  if (up < 0.0) c = vec3(0.004, 0.0045, 0.0055) + c * 0.1 * (1.0 + up);
  return c;
}

float gEps = 0.001;
vec3 calcNormal(vec3 p) {
  const vec2 k = vec2(1.0, -1.0);
  float e = gEps;
  return normalize(k.xyy * map(p + k.xyy * e) + k.yyx * map(p + k.yyx * e) + k.yxy * map(p + k.yxy * e) + k.xxx * map(p + k.xxx * e));
}

vec3 metal(vec3 p, vec3 n, vec3 rd) {
  // Surface tension ripples: a faint, slow perturbation of the normal.
  vec2 w = p.xy * 160.0;
  n = normalize(n + 0.005 * vec3(sin(w.x + uTime * 1.7) * cos(w.y * 0.8 - uTime * 1.3), cos(w.x * 0.9 - uTime) * sin(w.y + uTime * 1.1), 0.0));
  vec3 v = -rd;
  float ndv = max(dot(n, v), 0.0);
  vec3 F0 = vec3(0.77, 0.78, 0.81);
  vec3 F = F0 + (1.0 - F0) * pow(1.0 - ndv, 5.0);
  float edge = pow(1.0 - ndv, 2.5);
  vec3 side = normalize(vec3(n.xy, 0.0001));
  vec3 rr = reflect(rd, normalize(n + side * 0.035 * edge));
  vec3 rg = reflect(rd, n);
  vec3 rb = reflect(rd, normalize(n - side * 0.035 * edge));
  vec3 c = vec3(env(rr, 0.0).r, env(rg, 0.0).g, env(rb, 0.0).b) * F;
  // Occlusion near the table and in creases (reflections there see the metal, not the studio).
  float ao = clamp(map(p + n * 0.02) / 0.02, 0.0, 1.0);
  ao = 0.3 + 0.7 * ao;
  ao *= mix(0.2, 1.0, smoothstep(0.0, 0.01, p.z));
  return c * ao;
}

float shadow(vec3 p, vec3 L) {
  float res = 1.0, t = 0.006;
  for (int i = 0; i < 14; i++) {
    float h = map(p + L * t);
    res = min(res, 9.0 * h / t);
    t += clamp(h, 0.008, 0.06);
    if (res < 0.01 || t > 0.4) break;
  }
  return clamp(res, 0.0, 1.0);
}

float plate(vec2 p, vec4 R, vec4 P) {
  if (P.z <= 0.001) return 0.0;
  float b = sdRR(p - R.xy, R.zw, P.x);
  return P.z * (1.0 - smoothstep(-0.03, 0.0, b));
}

vec3 tableColor(vec2 p, vec2 cam, vec3 rd, vec3 L, bool near) {
  vec3 P = vec3(p, 0.0);
  float dF = near ? map(P) : 1.0;
  vec3 rf = reflect(rd, vec3(0.0, 0.0, 1.0));
  float pl = max(plate(p, uRimA, uRimAP), plate(p, uRimB, uRimBP));
  // Black lacquer under a soft key light, so shadows and contact read clearly.
  vec2 kd = (p - cam - vec2(0.0, 0.06) - uLight * 0.05) / vec2(0.9 * cam.x + 0.3, 0.55);
  float pool = exp(-dot(kd, kd) * 1.6);
  vec3 base = vec3(0.0022, 0.0023, 0.0028) + vec3(0.011, 0.0105, 0.0098) * pool;
  // Glossy: the soft boxes overhead appear in the lacquer as broad, blurred panels.
  vec3 fr = normalize(vec3(rf.xy * 2.4, rf.z));
  vec3 gloss = env(fr, 0.1) * 0.011;
  float sh = 1.0, ao = 1.0;
  vec3 mirror = vec3(0.0);
  if (dF < 0.14) {
    sh = shadow(P + vec3(0.0, 0.0, 0.002), L);
    ao = 0.3 + 0.7 * smoothstep(0.0, 0.04, map(P + vec3(0.0, 0.0, 0.012)));
    // The metal mirrored in the lacquer: soft coverage, so thin shapes don't reflect as dashes.
    float tt = 0.004, hmin = 1.0, tmin = 0.0;
    for (int i = 0; i < 16; i++) {
      float h = map(P + rf * tt);
      if (h < hmin) { hmin = h; tmin = tt; }
      if (h < 0.001) break;
      tt += max(h * 0.9, 0.002);
      if (tt > 0.18) break;
    }
    float cover = 1.0 - smoothstep(0.0, 0.006, hmin);
    if (cover > 0.0) {
      vec3 Q = P + rf * tmin;
      mirror = env(reflect(rf, calcNormal(Q)), 0.15) * 0.16 * cover * exp(-tmin * 14.0);
    }
    // Light bounced off the metal onto the table.
    base += vec3(0.9, 0.86, 0.82) * 0.012 * exp(-dF * 45.0);
  }
  vec3 col = (base * (0.45 + 0.55 * sh) + gloss * sh) * ao + mirror;
  return col * (1.0 - pl * 0.55);
}

void main() {
  float H = uRes.y;
  float px = 1.0 / H;                         // one pixel, in world units
  vec2 p = gl_FragCoord.xy / H;               // the table point under this pixel
  vec2 cam = vec2(0.5 * uRes.x / H, 0.5);
  vec3 rd = normalize(vec3((p - cam) * 0.32, -1.0));
  float z0 = 0.16;
  float tFloor = z0 / -rd.z;
  vec3 ro = vec3(p, 0.0) - rd * tFloor;
  gEps = max(0.0004, px * 0.75);
  cull(p);
  bool near = anything();

  float t = 0.0, dmin = 1e5, tmin = 0.0;
  bool hit = false;
  if (near) {
    float hitEps = px * 0.25;
    for (int i = 0; i < 96; i++) {
      vec3 q = ro + rd * t;
      float d = map(q);
      if (d < dmin) { dmin = d; tmin = t; }
      if (d < hitEps) { hit = true; break; }
      t += d * 0.9;
      if (t > tFloor) break;
    }
  }

  vec3 L = normalize(vec3(0.25 + uLight.x * 0.15, 0.55 + uLight.y * 0.1, 1.0));
  vec3 col;
  if (hit) {
    vec3 P = ro + rd * t;
    col = metal(P, calcNormal(P), rd);
  } else {
    col = tableColor(p, cam, rd, L, near);
    // Analytic edge antialiasing: a ray that grazed the surface takes a share of the metal.
    float cover = 1.0 - smoothstep(0.0, px * 1.25, dmin);
    if (cover > 0.0) {
      vec3 P = ro + rd * tmin;
      col = mix(col, metal(P, calcNormal(P), rd), cover);
    }
  }

  // Grade: soft vignette, ACES filmic curve, static dither.
  vec2 uv = gl_FragCoord.xy / uRes;
  col *= 1.0 - 0.35 * pow(length(uv - 0.5) * 1.25, 2.4);
  col = col * (2.51 * col + 0.03) / (col * (2.43 * col + 0.59) + 0.14);
  col = pow(clamp(col, 0.0, 1.0), vec3(1.0 / 2.2));
  // Static triangular dither: removes banding and never crawls.
  col += (hash(gl_FragCoord.xy) + hash(gl_FragCoord.xy + 17.0) - 1.0) / 255.0;
  outColor = vec4(col, 1.0);
}`;
