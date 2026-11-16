// Shaders for the liquid-metal world.
// World units: y up, the mirror floor is y = 0, the wordmark stands at the origin facing +z.
export const ND = 48;      // droplets
export const MAXC = 24;    // droplets a single ray may consider

export const quadVert = `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`;

const common = `
uniform vec2 uRes;
uniform float uTime;
uniform mat3 uCam;      // columns: right, up, forward
uniform vec3 uEye;
uniform float uFov;     // tan(fov / 2)
`;

export const sceneFrag = `#version 300 es
precision highp float;
#define ND ${ND}
#define MAXC ${MAXC}
${common}
uniform vec4 uD[ND];      // droplet centre xyz, radius
uniform float uSq[ND];    // vertical squash (1 = round)
uniform int uN;
uniform sampler2D uTex;   // wordmark distance field (world units, signed)
uniform vec4 uTR;         // wordmark centre x, y; half width, half height
uniform vec4 uTA;         // tube radius, morph, breathing, visible
uniform vec4 uSph;        // sphere centre, radius
uniform vec4 uPool;       // radius, height, portal ripple, visible
uniform vec4 uRip;        // pointer ripple x, y, age, strength
uniform vec2 uLight;      // pointer light tilt
uniform float uK;         // droplet merge radius
uniform float uWipe;      // portal ring radius (screen units), 0 = none
uniform vec2 uEyeY;       // eye height above, below the surface
uniform vec3 uCloudC;     // centre of the under-surface world
uniform float uOut;       // 1, or 0.25 when the buffer is 8-bit
out vec4 outColor;

int gIdx[MAXC];
int gCnt;
bool gText, gSph, gPool;

float smin(float a, float b, float k) { float h = max(k - abs(a - b), 0.0) / k; return min(a, b) - h * h * k * 0.25; }
float sdBox2(vec2 p, vec2 b) { vec2 q = abs(p) - b; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0); }
float hash(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }

// Ray vs sphere: does the ray (o, d) pass within r of c before tmax?
bool hitSphere(vec3 o, vec3 d, vec3 c, float r, float tmax) {
  vec3 oc = o - c;
  float b = dot(oc, d), cc = dot(oc, oc) - r * r;
  if (cc < 0.0) return true;
  float h = b * b - cc;
  return h > 0.0 && -b - sqrt(h) < tmax && -b + sqrt(h) > 0.0;
}
bool hitBox(vec3 o, vec3 d, vec3 bmin, vec3 bmax, float tmax) {
  vec3 inv = 1.0 / d;
  vec3 t0 = (bmin - o) * inv, t1 = (bmax - o) * inv;
  vec3 lo = min(t0, t1), hi = max(t1, t0);
  float a = max(max(lo.x, lo.y), lo.z), b = min(min(hi.x, hi.y), hi.z);
  return b >= max(a, 0.0) && a < tmax;
}

// Bounding volumes: decide once per ray which objects can be hit at all.
void setup(vec3 o, vec3 d, float tmax) {
  float k = uK;
  gText = uTA.w > 0.5 && hitBox(o, d, vec3(uTR.xy - uTR.zw - 0.25, -0.4), vec3(uTR.xy + uTR.zw + 0.25, 0.4), tmax);
  gSph = uSph.w > 0.002 && hitSphere(o, d, uSph.xyz, uSph.w * 1.15 + 0.3, tmax);
  gPool = uPool.w > 0.5 && hitBox(o, d, vec3(-uPool.x - 0.3, -0.1, -uPool.x - 0.3), vec3(uPool.x + 0.3, uPool.y + 0.25, uPool.x + 0.3), tmax);
  gCnt = 0;
  for (int i = 0; i < ND; i++) {
    if (i >= uN || gCnt >= MAXC) break;
    vec4 D = uD[i];
    if (D.w > 0.001 && hitSphere(o, d, D.xyz, D.w + k, tmax)) { gIdx[gCnt] = i; gCnt++; }
  }
}
bool anything() { return gText || gSph || gPool || gCnt > 0; }

float sdText(vec3 p) {
  float m = uTA.y;
  float sx = 1.0 - 0.8 * m;
  vec3 q = p - vec3(uTR.xy, 0.0);
  q.x /= sx;
  // Melting: the letters sag towards their middle line and pull together.
  q.y = q.y * (1.0 + 0.9 * m) + 0.25 * m * sin(q.x * 3.0 + 1.0);
  vec2 uv = q.xy / (2.0 * uTR.zw) + 0.5;
  float d2 = texture(uTex, clamp(uv, 0.0, 1.0)).r;
  d2 = max(d2, sdBox2(q.xy, uTR.zw));
  float R = uTA.x * (1.0 + 0.35 * m);
  d2 += uTA.z - 0.35 * R * m;
  float d = length(vec2(max(d2 + R, 0.0), q.z)) - R + min(d2 + R, 0.0) * 0.0;
  return d * sx / (1.0 + 0.9 * m);
}

float map(vec3 p) {
  float d = 1e5;
  if (gText) {
    d = sdText(p);
    if (uRip.w > 0.0) {
      float r = length(p.xy - uRip.xy);
      d -= uRip.w * sin(r * 26.0 - uRip.z * 10.0) * exp(-r * 3.0) * smoothstep(0.25, 0.0, abs(p.z) - 0.05);
    }
  }
  if (gSph) {
    float ds = length(p - uSph.xyz) - uSph.w;
    // Slow surface ripples, stronger while the shape is in flux.
    ds += 0.006 * sin(p.x * 9.0 + uTime * 1.3) * sin(p.y * 8.0 - uTime * 1.1) * sin(p.z * 7.0 + uTime * 0.9);
    d = gText ? smin(d, ds, 0.35 * uTA.y + 0.02) : ds;
    if (gText) d = mix(d, ds, smoothstep(0.7, 1.0, uTA.y));
  }
  for (int j = 0; j < MAXC; j++) {
    if (j >= gCnt) break;
    int i = gIdx[j];
    vec4 D = uD[i];
    float s = uSq[i];
    vec3 e = p - D.xyz;
    e.y /= s;
    d = smin(d, (length(e) - D.w) * s, uK);
  }
  if (gPool) {
    vec3 r = vec3(uPool.x, uPool.y, uPool.x);
    vec3 q = p;
    float k0 = length(q / r), k1 = length(q / (r * r));
    float dp = k0 * (k0 - 1.0) / k1;
    float rr = length(p.xz);
    // The portal: rings run out from the middle as the camera comes down.
    dp -= uPool.z * sin(rr * 22.0 - uTime * 7.0) * exp(-rr * 1.2) * smoothstep(-0.02, 0.06, p.y);
    dp -= 0.0025 * sin(p.x * 6.0 + uTime * 0.8) * sin(p.z * 5.0 - uTime * 0.7);
    d = smin(d, dp, 0.18);
  }
  return d;
}

vec3 calcNormal(vec3 p, float t) {
  const vec2 k = vec2(1.0, -1.0);
  float e = 0.0006 + t * 0.0004;
  return normalize(k.xyy * map(p + k.xyy * e) + k.yyx * map(p + k.yyx * e) + k.yxy * map(p + k.yxy * e) + k.xxx * map(p + k.xxx * e));
}

float rect(vec2 p, vec2 c, vec2 b, float s) { return 1.0 - smoothstep(-s, s, sdBox2(p - c, b)); }

// The studio: overhead soft boxes, tall strip lights, a warm horizon, a near-black room.
vec3 env(vec3 d) {
  float y = d.y;
  float az = atan(d.x, d.z);
  vec3 c = mix(vec3(0.0035, 0.004, 0.0055), vec3(0.016, 0.018, 0.024), smoothstep(-0.1, 0.9, y));
  // Horizon glow: warm one way, cool the other.
  float hz = exp(-pow((y - 0.02) / 0.075, 2.0));
  c += hz * (vec3(1.0, 0.62, 0.32) * 0.9 * (0.5 + 0.5 * cos(az - 2.3)) + vec3(0.35, 0.55, 1.0) * 0.55 * (0.5 + 0.5 * cos(az + 1.2)));
  if (y > 0.015) {
    vec2 p = d.xz / y + uLight * vec2(0.35, 0.25);
    // A big overhead soft box with a hot centre line, two long strips, a small spot.
    c += vec3(1.0, 0.98, 0.95) * 3.2 * rect(p, vec2(0.0, -0.15), vec2(0.85, 0.42), 0.02);
    c += vec3(1.0) * 9.0 * rect(p, vec2(0.0, -0.15), vec2(0.7, 0.025), 0.01);
    c += vec3(0.85, 0.92, 1.0) * 5.0 * rect(p, vec2(-1.55, 0.2), vec2(0.06, 1.6), 0.015);
    c += vec3(1.0, 0.93, 0.85) * 5.0 * rect(p, vec2(1.55, -0.3), vec2(0.06, 1.6), 0.015);
    c += vec3(1.0, 0.97, 0.92) * 40.0 * (1.0 - smoothstep(0.05, 0.065, length(p - vec2(0.75, 0.9))));
  }
  // Tall strip lights standing around the set: these draw the crisp vertical lines on curves.
  float yb = smoothstep(-0.08, -0.02, y) * smoothstep(0.75, 0.6, y);
  c += vec3(0.95, 0.97, 1.0) * 4.5 * yb * (1.0 - smoothstep(0.035, 0.045, abs(az - 0.55)));
  c += vec3(1.0, 0.9, 0.8) * 3.5 * yb * (1.0 - smoothstep(0.035, 0.045, abs(az + 0.75)));
  c += vec3(0.8, 0.88, 1.0) * 3.0 * yb * (1.0 - smoothstep(0.05, 0.06, abs(abs(az) - 2.6)));
  // A wide, dim panel behind the default camera for broad gradients on the front faces.
  c += vec3(0.5, 0.52, 0.55) * smoothstep(0.55, 0.2, abs(az)) * smoothstep(0.0, 0.12, y) * smoothstep(0.55, 0.25, y);
  return c;
}

// The floor as seen in a reflection: a black mirror showing the room upside down.
vec3 floorSeen(vec3 r) {
  float c = -r.y;
  float F = 0.05 + 0.6 * pow(1.0 - c, 5.0);
  return env(vec3(r.x, -r.y, r.z)) * F * 0.6;
}

vec3 metal(vec3 p, vec3 n, vec3 rd) {
  vec3 v = -rd;
  float ndv = clamp(dot(n, v), 0.0, 1.0);
  vec3 F0 = vec3(0.80, 0.805, 0.82);
  vec3 F = F0 + (1.0 - F0) * pow(1.0 - ndv, 5.0);
  vec3 r = reflect(rd, n);
  vec3 c = r.y >= 0.0 ? env(r) : floorSeen(r);
  float ao = clamp(map(p + n * 0.05) / 0.05, 0.0, 1.0);
  ao = 0.35 + 0.65 * ao;
  // Contact with the floor darkens the underside a little.
  ao *= mix(0.55, 1.0, smoothstep(0.0, 0.08, p.y));
  return c * F * ao;
}

float march(vec3 ro, vec3 rd, float tmax, int steps, out float t) {
  t = 0.0;
  for (int i = 0; i < 160; i++) {
    if (i >= steps) break;
    vec3 q = ro + rd * t;
    float d = map(q);
    if (d < 0.0004 + t * 0.0004) return 1.0;
    t += d * 0.9;
    if (t > tmax) break;
  }
  return 0.0;
}

vec3 background(vec3 rd) {
  float hz = exp(-pow((rd.y - 0.01) / 0.05, 2.0));
  float az = atan(rd.x, rd.z);
  vec3 c = vec3(0.004, 0.0045, 0.006) + vec3(0.012, 0.013, 0.017) * smoothstep(0.0, 0.6, rd.y);
  c += hz * (vec3(1.0, 0.62, 0.32) * 0.07 * (0.5 + 0.5 * cos(az - 2.3)) + vec3(0.35, 0.55, 1.0) * 0.05 * (0.5 + 0.5 * cos(az + 1.2)));
  return c;
}

vec3 world(vec3 ro, vec3 rd) {
  float tFloor = rd.y < -1e-4 ? -ro.y / rd.y : 1e9;
  float tmax = min(tFloor + 0.01, 60.0);
  setup(ro, rd, tmax);
  float t;
  if (anything() && march(ro, rd, tmax, 120, t) > 0.5) {
    vec3 P = ro + rd * t;
    return metal(P, calcNormal(P, t), rd);
  }
  if (rd.y >= -1e-4) return background(rd);
  // The black mirror floor.
  vec3 P = ro + rd * tFloor;
  vec3 r = vec3(rd.x, -rd.y, rd.z);
  float c = -rd.y;
  float F = 0.045 + 0.5 * pow(1.0 - c, 5.0);
  vec3 refl = env(r) * 0.5;
  setup(P, r, 14.0);
  float ao = 1.0;
  if (anything()) {
    float tr;
    if (march(P + r * 0.002, r, 14.0, 72, tr) > 0.5) {
      vec3 Q = P + r * tr;
      // Real reflection of the metal, fading as it gets further from the floor.
      refl = mix(refl, metal(Q, calcNormal(Q, tr), r) * 1.6, exp(-tr * 0.35));
    }
    ao = 0.45 + 0.55 * smoothstep(0.0, 0.18, map(P + vec3(0.0, 0.12, 0.0)));
  }
  vec3 col = refl * F * ao;
  // Faint grain in the lacquer, and a slow falloff into the dark room.
  col += vec3(0.0016, 0.0017, 0.002) * (1.0 + 0.4 * sin(P.x * 0.7) * sin(P.z * 0.5));
  float fog = 1.0 - exp(-max(0.0, length(P.xz) - 4.0) * 0.08);
  return mix(col, background(vec3(rd.x, 0.0, rd.z)), fog);
}

// Under the surface: a dark chamber with the metal as a rippling ceiling.
vec3 under(vec3 ro, vec3 rd) {
  vec3 c = mix(vec3(0.0025, 0.003, 0.0045), vec3(0.008, 0.009, 0.013), smoothstep(-1.0, 1.0, rd.y));
  vec3 oc = ro - uCloudC;
  float b = dot(oc, rd), h = dot(oc, oc) - b * b;
  c += vec3(0.25, 0.3, 0.42) * 0.02 * exp(-h * 0.25);
  if (rd.y > 0.0) {
    float t = -ro.y / rd.y;
    vec3 P = ro + rd * t;
    float a = sin(P.x * 2.3 + uTime * 0.6) + sin(P.z * 2.9 - uTime * 0.5) + 0.6 * sin((P.x + P.z) * 4.1 + uTime * 0.9);
    vec3 n = normalize(vec3(0.35 * cos(P.x * 2.3 + uTime * 0.6), -1.0, 0.35 * cos(P.z * 2.9 - uTime * 0.5)));
    vec3 r = reflect(rd, n);
    float spec = pow(max(0.0, 1.0 - abs(r.y + 0.35)), 18.0);
    float fall = exp(-t * 0.18);
    c += (vec3(0.5, 0.55, 0.65) * 0.05 * (0.5 + 0.5 * a / 2.6) + vec3(0.8, 0.85, 1.0) * spec * 0.9) * fall;
  }
  return c;
}

void main() {
  vec2 uv = (gl_FragCoord.xy * 2.0 - uRes) / uRes.y;
  float rr = length(uv);
  // Portal ring: the view refracts round its edge as the camera passes the surface.
  float band = 0.0;
  if (uWipe > 0.0) {
    float x = (rr - uWipe) / 0.09;
    band = exp(-x * x);
    uv *= 1.0 - 0.06 * band * sign(x) * (1.0 - exp(-abs(x)));
  }
  vec3 rd = normalize(uCam * vec3(uv * uFov, 1.0));
  vec3 col;
  if (rr < uWipe) col = under(vec3(uEye.x, uEyeY.y, uEye.z), rd);
  else col = world(vec3(uEye.x, uEyeY.x, uEye.z), rd);
  col += vec3(0.85, 0.9, 1.0) * band * 0.9 * step(0.001, uWipe) * (1.0 - smoothstep(1.4, 1.9, uWipe));
  outColor = vec4(col * uOut, 1.0);
}`;

// Particles: GPU points whose positions come from Anikit's pointCloud every frame.
export const pointsVert = `#version 300 es
precision highp float;
${common}
in vec3 aPos;
uniform vec3 uOff;
uniform float uScale;
uniform float uSpin;
uniform float uSize;
out float vI;
void main() {
  vec3 p = aPos * uScale;
  float c = cos(uSpin), s = sin(uSpin);
  p.xz = mat2(c, -s, s, c) * p.xz;
  p += uOff;
  vec3 v = p - uEye;
  vec3 q = v * uCam;        // view space: x right, y up, z forward
  float z = max(q.z, 0.01);
  vec2 ndc = q.xy / (z * uFov);
  ndc.x *= uRes.y / uRes.x;
  gl_Position = vec4(ndc, 0.0, q.z > 0.05 ? 1.0 : -1.0);
  float px = uSize * uRes.y / z;
  gl_PointSize = clamp(px, 1.0, 24.0);
  // Points smaller than a pixel keep their energy as dimmer one-pixel points.
  vI = min(1.0, px * px) * (0.6 + 0.4 * fract(sin(float(gl_VertexID) * 12.9898) * 43758.5453));
}`;

export const pointsFrag = `#version 300 es
precision highp float;
uniform vec2 uRes;
uniform float uWipe;
uniform float uGain;
in float vI;
out vec4 outColor;
void main() {
  vec2 c = gl_PointCoord * 2.0 - 1.0;
  float r = dot(c, c);
  if (r > 1.0) discard;
  vec2 uv = (gl_FragCoord.xy * 2.0 - uRes) / uRes.y;
  float m = 1.0 - smoothstep(uWipe - 0.08, uWipe, length(uv));
  float a = (1.0 - r) * (1.0 - r) * vI * uGain * m;
  outColor = vec4(vec3(0.82, 0.86, 0.95) * a, 1.0);
}`;

// Bloom: bright pass at quarter resolution, then a separable blur.
export const brightFrag = `#version 300 es
precision highp float;
uniform sampler2D uSrc;
uniform vec2 uTexel;
out vec4 outColor;
void main() {
  vec2 uv = gl_FragCoord.xy * uTexel;
  vec2 o = uTexel * 0.5;
  vec3 c = texture(uSrc, uv + vec2(-o.x, -o.y)).rgb + texture(uSrc, uv + vec2(o.x, -o.y)).rgb
         + texture(uSrc, uv + vec2(-o.x, o.y)).rgb + texture(uSrc, uv + vec2(o.x, o.y)).rgb;
  c *= 0.25;
  float l = max(c.r, max(c.g, c.b));
  outColor = vec4(c * smoothstep(0.9, 3.5, l), 1.0);
}`;

export const blurFrag = `#version 300 es
precision highp float;
uniform sampler2D uSrc;
uniform vec2 uTexel;
uniform vec2 uDir;
out vec4 outColor;
void main() {
  vec2 uv = gl_FragCoord.xy * uTexel;
  vec3 c = texture(uSrc, uv).rgb * 0.2270270;
  c += (texture(uSrc, uv + uDir * 1.3846154).rgb + texture(uSrc, uv - uDir * 1.3846154).rgb) * 0.3162162;
  c += (texture(uSrc, uv + uDir * 3.2307692).rgb + texture(uSrc, uv - uDir * 3.2307692).rgb) * 0.0702703;
  outColor = vec4(c, 1.0);
}`;

export const compositeFrag = `#version 300 es
precision highp float;
uniform sampler2D uSrc;
uniform sampler2D uBloom;
uniform vec2 uRes;
uniform float uFade;
uniform float uScaleIn;   // 1 when the scene buffer holds HDR, 4 when it was stored / 4
out vec4 outColor;
float hash(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
vec3 aces(vec3 x) { return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
void main() {
  vec2 uv = gl_FragCoord.xy / uRes;
  vec3 c = texture(uSrc, uv).rgb * uScaleIn + texture(uBloom, uv).rgb * 0.22 * uScaleIn;
  c *= 1.15;
  c = aces(c);
  vec2 q = uv - 0.5;
  q.x *= uRes.x / uRes.y;
  c *= 1.0 - 0.28 * smoothstep(0.35, 1.15, length(q));
  c = pow(c, vec3(1.0 / 2.2));
  c += (hash(gl_FragCoord.xy) - 0.5) / 255.0;
  outColor = vec4(c * uFade, 1.0);
}`;
