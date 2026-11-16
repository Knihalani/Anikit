// A liquid-metal world over a black mirror floor, ray marched in one pass.
// y is up; the floor is the plane y = 0. Units are roughly metres.
export const MAX_DROPS = 48;

export const vert = `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`;

export const frag = `#version 300 es
precision highp float;
#define ND ${MAX_DROPS}
uniform vec2 uRes;
uniform float uTime;
uniform vec3 uCam;          // camera position
uniform mat3 uCamM;         // camera basis: right, up, forward
uniform float uFov;         // tan(fov / 2)
uniform vec4 uD[ND];        // drops: x, y, z, radius
uniform float uK;           // smooth-min blend between drops
uniform sampler2D uTex;     // wordmark distance field (texels, signed)
uniform vec4 uTC;           // wordmark centre xyz, world units per texel
uniform vec4 uTB;           // wordmark half extents (x, y), thickness, on
uniform vec4 uSph;          // the morph sphere: centre xyz, radius
uniform float uMorph;       // 0 = letters, 1 = sphere
uniform vec4 uRip;          // pointer ripple on the wordmark: x, y, z, strength
uniform vec2 uLight;        // pointer parallax for the lights
out vec4 outColor;

float smin(float a, float b, float k) { float h = max(k - abs(a - b), 0.0) / k; return min(a, b) - h * h * k * 0.25; }
float sdBox2(vec2 p, vec2 b) { vec2 q = abs(p) - b; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0); }
float hash(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }

// ---- Culling: which drops a ray can come near, and whether it nears the wordmark/sphere.
uvec2 gMask;
bool gText, gSph;
float rayDist(vec3 ro, vec3 rd, vec3 c) { vec3 oc = c - ro; float t = max(dot(oc, rd), 0.0); return length(oc - rd * t); }
void cull(vec3 ro, vec3 rd, float margin) {
  gMask = uvec2(0u);
  for (int i = 0; i < ND; i++) {
    vec4 D = uD[i];
    if (D.w <= 0.0) continue;
    if (rayDist(ro, rd, D.xyz) < D.w + margin) {
      if (i < 32) gMask.x |= (1u << uint(i)); else gMask.y |= (1u << uint(i - 32));
    }
  }
  float tb = length(uTB.xy) + uTB.z + margin;
  gText = uTB.w > 0.0 && uMorph < 0.999 && rayDist(ro, rd, uTC.xyz) < tb;
  gSph = uSph.w > 0.0 && uMorph > 0.001 && rayDist(ro, rd, uSph.xyz) < uSph.w + margin + 0.3;
}

float textD(vec3 q) {
  vec3 l = q - uTC.xyz;
  // The pointer pushes slow ripples across the letters.
  float rd = length(l.xy - (uRip.xy - uTC.xy));
  float rip = uRip.w * exp(-rd * rd * 3.0) * sin(rd * 18.0 - uTime * 5.0) * 0.012;
  vec2 uv = l.xy / (2.0 * uTB.xy) + 0.5;
  float d2 = texture(uTex, clamp(uv, 0.0, 1.0)).r * uTC.w;
  d2 = max(d2, sdBox2(l.xy, uTB.xy));
  float T = uTB.z;
  return length(vec2(max(d2 + T, 0.0), l.z)) - T + rip;
}

float map(vec3 q) {
  float d = 1e5;
  if (gText || gSph) {
    float dt = gText ? textD(q) : 1e5;
    // Surface tension: the sphere wobbles slowly, as a big drop does.
    vec3 sq = q - uSph.xyz;
    float ds = length(sq) - uSph.w * (1.0 + 0.025 * sin(sq.x * 5.0 + uTime * 1.3) * sin(sq.y * 4.0 - uTime * 1.1) * sin(sq.z * 4.5 + uTime * 0.9));
    // Melting letters into a sphere: blend the two distance fields, with a little sag.
    d = gText && gSph ? mix(dt, ds, uMorph) - sin(uMorph * 3.14159) * 0.06 : (gText ? dt : ds);
  }
  if (gMask.x != 0u || gMask.y != 0u) for (int i = 0; i < ND; i++) {
    bool on = i < 32 ? (gMask.x & (1u << uint(i))) != 0u : (gMask.y & (1u << uint(i - 32))) != 0u;
    if (!on) continue;
    vec4 D = uD[i];
    d = smin(d, length(q - D.xyz) - D.w, uK);
  }
  return d;
}

float softBox(vec2 p, vec2 b, float s) { return 1.0 - smoothstep(-s, s, sdBox2(p, b)); }
float spot(vec2 p, float r, float s) { return 1.0 - smoothstep(r - s, r + s, length(p)); }

// The studio, linear HDR: big soft boxes with crisp edges, strip lights, a few small hot
// lights, a warm/cool horizon glow, a dark room. Below the horizon: the black floor.
vec3 env(vec3 d, float rough) {
  float up = d.y;
  vec3 c = vec3(0.006, 0.0065, 0.008) + vec3(0.03, 0.033, 0.04) * smoothstep(-0.05, 1.0, up);
  float hor = exp(-pow((up - 0.04) / (0.07 + rough), 2.0));
  float az = atan(d.z, d.x);
  c += hor * (vec3(1.0, 0.72, 0.45) * 1.4 * smoothstep(1.2, 0.0, abs(az - 1.1)) + vec3(0.45, 0.62, 1.0) * 1.2 * smoothstep(1.2, 0.0, abs(az + 1.9)) + vec3(0.12));
  if (up > 0.02) {
    vec2 pp = d.xz / up - uLight * 0.3;
    float s = 0.01 + rough;
    c += vec3(1.0, 0.98, 0.95) * 6.0 * softBox(pp - vec2(0.0, -0.1), vec2(0.9, 0.22), s);
    c += vec3(0.86, 0.92, 1.0) * 4.0 * softBox(pp - vec2(-1.35, 0.3), vec2(0.07, 1.3), s);
    c += vec3(1.0, 0.94, 0.86) * 3.5 * softBox(pp - vec2(1.5, -0.2), vec2(0.06, 1.2), s);
    c += vec3(1.0) * 2.5 * softBox(pp - vec2(0.2, 1.05), vec2(1.1, 0.04), s);
    if (rough < 0.05) {
      c += vec3(1.0, 0.95, 0.88) * 45.0 * spot(pp - vec2(0.75, 0.55), 0.05, s);
      c += vec3(0.9, 0.95, 1.0) * 35.0 * spot(pp - vec2(-0.7, -0.75), 0.04, s);
    }
  }
  if (up < 0.0) c = vec3(0.003, 0.0033, 0.004) + c * 0.05;
  return c;
}

float gEps = 0.001;
vec3 normalAt(vec3 p) {
  const vec2 k = vec2(1.0, -1.0);
  float e = gEps;
  return normalize(k.xyy * map(p + k.xyy * e) + k.yyx * map(p + k.yyx * e) + k.yxy * map(p + k.yxy * e) + k.xxx * map(p + k.xxx * e));
}

vec3 metal(vec3 p, vec3 n, vec3 rd) {
  vec3 v = -rd;
  float ndv = max(dot(n, v), 0.0);
  vec3 F0 = vec3(0.77, 0.78, 0.81);
  vec3 F = F0 + (1.0 - F0) * pow(1.0 - ndv, 5.0);
  float edge = pow(1.0 - ndv, 2.5);
  vec3 side = normalize(cross(n, cross(v, n)) + 1e-4);
  vec3 rr = reflect(rd, normalize(n + side * 0.03 * edge));
  vec3 rg = reflect(rd, n);
  vec3 rb = reflect(rd, normalize(n - side * 0.03 * edge));
  vec3 c = vec3(env(rr, 0.0).r, env(rg, 0.0).g, env(rb, 0.0).b) * F;
  // Reflections that point down see the black floor; creases see the metal itself.
  float ao = clamp(map(p + n * 0.06) / 0.06, 0.0, 1.0);
  ao = 0.35 + 0.65 * ao;
  ao *= mix(0.3, 1.0, smoothstep(0.0, 0.06, p.y));
  return c * ao;
}

// Soft shadow of a sphere (analytic), for drops on the floor.
float sphShadow(vec3 ro, vec3 rd, vec4 s) {
  vec3 oc = ro - s.xyz;
  float b = dot(oc, rd), c = dot(oc, oc) - s.w * s.w, h = b * b - c;
  float dd = sqrt(max(0.0, s.w * s.w - h)) - s.w;
  float t = -b - sqrt(max(h, 0.0));
  return (b > 0.0) ? 1.0 : smoothstep(0.0, 1.0, 2.5 * dd / max(t, 1e-3) + 1.0);
}

struct Hit { bool hit; float t; float dmin; float tmin; };
Hit march(vec3 ro, vec3 rd, float tMax, float px) {
  Hit h = Hit(false, 0.0, 1e5, 0.0);
  float t = 0.0;
  for (int i = 0; i < 90; i++) {
    float d = map(ro + rd * t);
    float tol = px * t;
    if (d < tol * 0.5) { h.hit = true; h.t = t; return h; }
    if (d / max(t, 0.05) < h.dmin) { h.dmin = d / max(t, 0.05); h.tmin = t; }
    t += d * 0.9;
    if (t > tMax) break;
  }
  return h;
}

vec3 floorColor(vec3 P, vec3 rd, float px) {
  vec3 L = normalize(vec3(0.35 + uLight.x * 0.2, 1.0, 0.25));
  // A soft key light pool on the black lacquer.
  float pool = exp(-dot(P.xz, P.xz) * 0.06);
  vec3 base = vec3(0.0015) + vec3(0.010, 0.0098, 0.0094) * pool;
  float sh = 1.0, ao = 1.0;
  for (int i = 0; i < ND; i++) {
    vec4 D = uD[i];
    if (D.w <= 0.0) continue;
    // Only drops whose shadow or occlusion can reach this point.
    vec2 foot = D.xz - L.xz / L.y * D.y;
    if (length(P.xz - foot) < D.w * 3.0 + D.y * 0.35) sh *= sphShadow(P, L, D);
    float dist = length(P - D.xyz);
    if (dist < D.w * 6.0) ao *= 1.0 - D.w * D.w / (dist * dist) * 0.9;
  }
  if (uMorph > 0.001 && uSph.w > 0.0) { sh *= sphShadow(P, L, uSph); float dist = length(P - uSph.xyz); ao *= 1.0 - uSph.w * uSph.w / (dist * dist) * 0.8; }
  // The mirror: march the reflected ray; the studio shows dimly, the metal brightly.
  vec3 rr = reflect(rd, vec3(0.0, 1.0, 0.0));
  float fres = 0.04 + 0.96 * pow(1.0 - max(-rd.y, 0.0), 5.0);
  // Black lacquer: a dielectric, so it barely reflects straight down and mirrors at grazing angles.
  float refl = mix(0.035, 0.85, fres);
  cull(P, rr, 0.08);
  vec3 r;
  Hit h = march(P + rr * 0.002, rr, 12.0, px);
  if (h.hit) {
    vec3 Q = P + rr * h.t;
    r = metal(Q, normalAt(Q), rr) * exp(-h.t * 0.25);
  } else {
    // The studio in the lacquer: blurred, and without the small hot lights.
    r = env(rr, 0.08) * 0.35;
    float cover = 1.0 - smoothstep(0.0, px * 1.5, h.dmin);
    if (cover > 0.0) { vec3 Q = P + rr * h.tmin; r = mix(r, metal(Q, normalAt(Q), rr) * exp(-h.tmin * 0.25), cover); }
  }
  return base * clamp(sh * ao, 0.0, 1.0) * 1.0 + r * refl * mix(0.35, 1.0, clamp(ao, 0.0, 1.0));
}

void main() {
  vec2 sp = (gl_FragCoord.xy * 2.0 - uRes) / uRes.y;
  vec3 rd = normalize(uCamM * vec3(sp * uFov, 1.0));
  vec3 ro = uCam;
  float px = 1.5 * uFov / uRes.y;          // angular size of a pixel
  gEps = 0.0008;
  float tFloor = rd.y < 0.0 ? -ro.y / rd.y : 1e4;
  float tMax = min(tFloor, 40.0);
  cull(ro, rd, 0.08);
  vec3 col;
  bool any = gText || gSph || gMask.x != 0u || gMask.y != 0u;
  Hit h = Hit(false, 0.0, 1e5, 0.0);
  if (any) h = march(ro, rd, tMax, px);
  if (h.hit) {
    vec3 P = ro + rd * h.t;
    col = metal(P, normalAt(P), rd);
  } else {
    if (tFloor < 1e3) {
      vec3 P = ro + rd * tFloor;
      col = floorColor(P, rd, px);
      // The floor fades into the dark studio far away.
      col = mix(col, env(rd, 0.3) * 0.4, smoothstep(10.0, 30.0, tFloor));
    } else col = env(rd, 0.0) * 0.5;
    // Edge antialiasing from the closest approach.
    float cover = 1.0 - smoothstep(0.0, px * 1.5, h.dmin);
    if (any && cover > 0.0) {
      cull(ro, rd, 0.08);
      vec3 P = ro + rd * h.tmin;
      col = mix(col, metal(P, normalAt(P), rd), cover);
    }
  }
  vec2 uv = gl_FragCoord.xy / uRes;
  col *= 1.0 - 0.3 * pow(length(uv - 0.5) * 1.3, 2.4);
  col = col * (2.51 * col + 0.03) / (col * (2.43 * col + 0.59) + 0.14);
  col = pow(clamp(col, 0.0, 1.0), vec3(1.0 / 2.2));
  col += (hash(gl_FragCoord.xy) + hash(gl_FragCoord.xy + 17.0) - 1.0) / 255.0;
  outColor = vec4(col, 1.0);
}`;
