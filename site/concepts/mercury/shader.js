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
out vec4 outColor;

float smin(float a, float b, float k) { float h = max(k - abs(a - b), 0.0) / k; return min(a, b) - h * h * k * 0.25; }
float sdRR(vec2 p, vec2 b, float r) { vec2 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r; }
float sdBox(vec2 p, vec2 b) { vec2 q = abs(p) - b; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0); }
float hash(vec2 p) { return fract(sin(dot(p, vec2(41.3, 289.1))) * 43758.5453); }

float textD(vec2 p) {
  // Melting: each column stretches downward by its own amount and the letters thin out.
  float top = uTR.y + uTR.w;
  float col = 0.5 + 0.5 * sin(p.x * 23.0 + 1.7) * sin(p.x * 9.0 - 0.4);
  float a = uMelt * (0.15 + 1.0 * col * col);
  p.y = top - (top - p.y) / (1.0 + a);
  vec2 uv = (p - uTR.xy) / (2.0 * uTR.zw) + 0.5;
  vec2 cuv = clamp(uv, 0.0, 1.0);
  float d = texture(uTex, cuv).r * uTA.z;
  float outside = sdBox(p - uTR.xy, uTR.zw);
  return max(d, outside) / (1.0 + a);
}

float rim(vec3 q, vec4 R, vec4 P) {
  if (P.y < 0.0006) return 1e5;
  float b = sdRR(q.xy - R.xy, R.zw, P.x);
  float th = P.y * (1.0 + 0.07 * sin(q.x * 29.0 + uTime * 1.1) * sin(q.y * 23.0 - uTime * 0.9));
  return length(vec2(b, q.z - th * 0.15)) - th;
}

float map(vec3 q) {
  float d = 1e5;
  if (uTA.x > 0.0008) {
    float d2 = textD(q.xy) + uTA.y;
    float T = uTA.x;
    d = length(vec2(max(d2 + T, 0.0), q.z)) - T;
  }
  for (int i = 0; i < ND; i++) {
    vec4 D = uD[i];
    if (D.z <= 0.0) continue;
    vec3 c = vec3(D.xy, D.z * 0.3);
    float e = (length(vec3(q.xy - c.xy, (q.z - c.z) * 1.45)) - D.z) / 1.45;
    d = smin(d, e, uK);
  }
  d = smin(d, rim(q, uRimA, uRimAP), 0.03);
  d = smin(d, rim(q, uRimB, uRimBP), 0.03);
  if (uNeckR > 0.0005) {
    vec3 a = vec3(uNeck.xy, uNeckR * 0.5), b = vec3(uNeck.zw, uNeckR * 0.5);
    vec3 pa = q - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    float r = uNeckR * (1.0 - 0.45 * sin(h * 3.14159));
    d = smin(d, length(pa - ba * h) - r, 0.04);
  }
  if (uPool.z > 0.001) {
    vec3 r = vec3(uPool.z, uPool.z * 0.62, uPool.w);
    vec3 p = q - vec3(uPool.xy, 0.0);
    float k0 = length(p / r), k1 = length(p / (r * r));
    d = smin(d, k0 * (k0 - 1.0) / k1, 0.05);
  }
  return d;
}

float softBox(vec2 p, vec2 b, float s) { return 1.0 - smoothstep(-s, s, sdBox(p, b)); }

// The studio: soft boxes overhead, warm and cool rim lights low on the horizon, a dark room.
vec3 env(vec3 d, float rough) {
  float up = d.z;
  vec3 c = vec3(0.010, 0.011, 0.014) + vec3(0.05, 0.052, 0.06) * smoothstep(-0.1, 1.0, up);
  if (up > 0.03) {
    vec2 pp = d.xy / up - uLight * 0.22;
    float s = 0.03 + rough;
    c += vec3(1.0, 0.98, 0.95) * 5.0 * softBox(pp - vec2(0.05, 0.42), vec2(0.62, 0.075), s);
    c += vec3(1.0) * 1.4 * softBox(pp - vec2(-0.12, -0.05), vec2(0.30, 0.20), s + 0.08);
    c += vec3(0.86, 0.92, 1.0) * 3.0 * softBox(pp - vec2(-0.82, 0.0), vec2(0.06, 0.75), s);
    c += vec3(1.0, 0.95, 0.88) * 2.2 * softBox(pp - vec2(0.95, -0.2), vec2(0.045, 0.6), s);
  }
  float az = atan(d.y, d.x);
  float hor = exp(-pow((up - 0.1) / (0.14 + rough * 0.3), 2.0));
  c += vec3(1.0, 0.70, 0.42) * 1.5 * hor * smoothstep(0.55, 0.0, abs(az - 2.5));
  c += vec3(0.42, 0.62, 1.0) * 1.2 * hor * smoothstep(0.6, 0.0, abs(az + 0.7));
  if (up < 0.0) c = vec3(0.006, 0.0065, 0.008) + c * 0.12 * (1.0 + up);
  return c;
}

vec3 calcNormal(vec3 p) {
  const vec2 k = vec2(1.0, -1.0);
  const float e = 0.0012;
  return normalize(k.xyy * map(p + k.xyy * e) + k.yyx * map(p + k.yyx * e) + k.yxy * map(p + k.yxy * e) + k.xxx * map(p + k.xxx * e));
}

vec3 metal(vec3 p, vec3 n, vec3 rd) {
  vec3 v = -rd;
  float ndv = max(dot(n, v), 0.0);
  vec3 F0 = vec3(0.80, 0.80, 0.82);
  vec3 F = F0 + (1.0 - F0) * pow(1.0 - ndv, 5.0);
  float edge = pow(1.0 - ndv, 2.5);
  vec3 side = normalize(vec3(n.xy, 0.0001));
  vec3 rr = reflect(rd, normalize(n + side * 0.05 * edge));
  vec3 rg = reflect(rd, n);
  vec3 rb = reflect(rd, normalize(n - side * 0.05 * edge));
  vec3 c = vec3(env(rr, 0.0).r, env(rg, 0.0).g, env(rb, 0.0).b) * F;
  // Occlusion near the table and in creases.
  float ao = clamp(map(p + n * 0.025) / 0.025, 0.0, 1.0);
  ao = 0.35 + 0.65 * ao;
  ao *= mix(0.25, 1.0, smoothstep(0.0, 0.012, p.z));
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

void main() {
  float H = uRes.y;
  vec2 p = gl_FragCoord.xy / H;              // the table point under this pixel
  vec2 cam = vec2(0.5 * uRes.x / H, 0.5);
  vec3 rd = normalize(vec3((p - cam) * 0.32, -1.0));
  float z0 = 0.16;
  float tFloor = z0 / -rd.z;
  vec3 ro = vec3(p, 0.0) - rd * tFloor;

  float t = 0.0;
  bool hit = false;
  // Cheap bound: if nothing is near this column, go straight to the table.
  float near = map(vec3(p, 0.02));
  if (near < 0.16) {
    for (int i = 0; i < 64; i++) {
      vec3 q = ro + rd * t;
      float d = map(q);
      if (d < 0.0006) { hit = true; break; }
      t += d * 0.85;
      if (t > tFloor) break;
    }
  }

  vec3 col;
  vec3 L = normalize(vec3(0.25 + uLight.x * 0.15, 0.55 + uLight.y * 0.1, 1.0));
  if (hit) {
    vec3 P = ro + rd * t;
    col = metal(P, calcNormal(P), rd);
  } else {
    vec3 P = vec3(p, 0.0);
    float dF = near < 0.16 ? map(P) : 1.0;
    vec3 rf = reflect(rd, vec3(0.0, 0.0, 1.0));
    float pl = max(plate(p, uRimA, uRimAP), plate(p, uRimB, uRimBP));
    // Dark lacquer: a blurred reflection of the soft boxes, almost black elsewhere.
    vec3 base = vec3(0.0035, 0.0036, 0.0042);
    vec3 gloss = env(rf, 0.3) * 0.006;
    float sh = 1.0, ao = 1.0;
    vec3 mirror = vec3(0.0);
    if (dF < 0.14) {
      sh = shadow(P + vec3(0.0, 0.0, 0.002), L);
      ao = 0.25 + 0.75 * smoothstep(0.0, 0.07, map(P + vec3(0.0, 0.0, 0.015)));
      // The metal mirrored in the lacquer, softened with distance.
      float tt = 0.004;
      for (int i = 0; i < 18; i++) {
        float h = map(P + rf * tt);
        if (h < 0.0015) {
          vec3 Q = P + rf * tt;
          vec3 n = calcNormal(Q);
          mirror = env(reflect(rf, n), 0.3) * 0.08 * exp(-tt * 20.0);
          break;
        }
        tt += max(h, 0.003);
        if (tt > 0.25) break;
      }
      // Light bounced off the metal onto the table.
      base += vec3(0.9, 0.86, 0.82) * 0.012 * exp(-dF * 45.0);
    }
    col = (base * (0.45 + 0.55 * sh) + gloss * sh) * ao + mirror;
    col *= 1.0 - pl * 0.55;
  }

  // Grade: soft vignette, filmic curve, dithering.
  vec2 uv = gl_FragCoord.xy / uRes;
  col *= 1.0 - 0.35 * pow(length(uv - 0.5) * 1.25, 2.4);
  col = col * (2.51 * col + 0.03) / (col * (2.43 * col + 0.59) + 0.14);
  col = pow(clamp(col, 0.0, 1.0), vec3(1.0 / 2.2));
  col += (hash(gl_FragCoord.xy + fract(uTime)) - 0.5) / 255.0;
  outColor = vec4(col, 1.0);
}`;
