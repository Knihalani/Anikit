// First-person arms, hands and the spark portal.
// The hands are built from tapered capsules ("round cones") drawn as one instanced mesh:
// spheres at the joints and tangent cones between them give one smooth surface per finger.
// An Anikit timeline, scrubbed by the gesture progress g, poses every joint.
// The portal is thousands of GPU sparks generated in the vertex shader (no CPU work per spark).
import { timeline, spring } from '../../../packages/anikit/dist/anikit.lean.js';

// ---------------------------------------------------------------------------
// Shaders
// ---------------------------------------------------------------------------
const meshVert = `#version 300 es
precision highp float;
layout(location = 0) in vec3 aGeo;     // round cone: (latitude, angle, t) / torus: (major angle, minor angle, -1)
layout(location = 1) in vec4 iA;       // end A xyz, radius A    | torus: centre, major radius
layout(location = 2) in vec4 iB;       // end B xyz, radius B    | torus: axis, minor radius
layout(location = 3) in vec4 iRef;     // dorsal reference xyz, material
uniform mat4 uProj;
out vec3 vPos;
out vec3 vN;
out vec3 vAxis;
out vec3 vRef;
out vec2 vLocal;   // t along the segment, angle from the dorsal side
flat out float vMat;
void main() {
  vec3 p, n, Z;
  vec3 X, Y;
  if (aGeo.z < -0.5) {
    Z = normalize(iB.xyz);
    X = normalize(iRef.xyz - Z * dot(iRef.xyz, Z));
    Y = cross(Z, X);
    vec3 radial = cos(aGeo.x) * X + sin(aGeo.x) * Y;
    n = cos(aGeo.y) * radial + sin(aGeo.y) * Z;
    p = iA.xyz + radial * iA.w + n * iB.w;
    vLocal = vec2(0.5, aGeo.x);
  } else {
    vec3 ab = iB.xyz - iA.xyz;
    float L = max(length(ab), 1e-5);
    Z = ab / L;
    X = normalize(iRef.xyz - Z * dot(iRef.xyz, Z) + vec3(1e-5, 0.0, 0.0));
    Y = cross(Z, X);
    float lat = aGeo.x, th = aGeo.y, t = aGeo.z;
    vec3 radial = cos(th) * X + sin(th) * Y;
    float slope = (iB.w - iA.w) / L;
    // Tangent latitude: the cone touches both spheres smoothly.
    float s = clamp(slope, -0.95, 0.95), c0 = sqrt(1.0 - s * s);
    vec3 dir;
    vec3 C; float r;
    if (t <= 0.0) { C = iA.xyz; r = iA.w; float l = mix(-1.5707963, asin(-s), lat / 1.5707963); dir = cos(l) * radial + sin(l) * Z; }
    else if (t >= 1.0) { C = iB.xyz; r = iB.w; float l = mix(asin(-s), 1.5707963, lat / 1.5707963); dir = cos(l) * radial + sin(l) * Z; }
    else { dir = c0 * radial - s * Z; C = mix(iA.xyz - Z * iA.w * s, iB.xyz - Z * iB.w * s, t); r = mix(iA.w, iB.w, t) * c0; }
    if (t > 0.0 && t < 1.0) p = C + radial * r; else p = C + dir * r;
    n = dir;
    vLocal = vec2(t, th);
  }
  vPos = p; vN = n; vAxis = Z; vRef = X; vMat = iRef.w;
  gl_Position = uProj * vec4(p, 1.0);
}`;

const meshFrag = `#version 300 es
precision highp float;
in vec3 vPos;
in vec3 vN;
in vec3 vAxis;
in vec3 vRef;
in vec2 vLocal;
flat in float vMat;
uniform vec3 uSun;        // direction to the sun: low, in front, over the golden horizon
uniform vec4 uPortal;     // portal centre (view space), glow strength
uniform float uTime;
out vec4 outColor;
float hash(vec3 p) { p = fract(p * 0.3183099 + 0.1); p *= 17.0; return fract(p.x * p.y * p.z * (p.x + p.y + p.z)); }
vec3 aces(vec3 x) { return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
void main() {
  vec3 n = normalize(vN);
  vec3 V = normalize(-vPos);
  vec3 L = normalize(uSun);
  vec3 sunC = vec3(1.0, 0.76, 0.5) * 2.6;
  vec3 skyC = vec3(0.50, 0.62, 0.92);
  vec3 snowC = vec3(0.92, 0.94, 1.0);
  float ndv = max(dot(n, V), 0.0);
  float edge = 1.0 - ndv;
  // Light passing the silhouette towards us from the sun in front.
  float back = pow(max(dot(-V, L), 0.0), 2.0);
  vec3 Lp = uPortal.xyz - vPos;
  float dp = length(Lp); Lp /= dp;
  vec3 portC = vec3(1.0, 0.48, 0.12) * uPortal.w * 1.6;
  float m = vMat;
  vec3 col;
  if (m > 3.5) {
    // Polished gold: the sky above, the bright horizon, a hard sun glint.
    vec3 r = reflect(-V, n);
    float up = r.y;
    vec3 env = mix(vec3(0.95, 0.93, 0.95), vec3(0.45, 0.58, 0.9), smoothstep(0.0, 0.8, up));
    env = mix(vec3(0.35, 0.36, 0.4), env, smoothstep(-0.25, 0.0, up));
    env += vec3(1.0, 0.75, 0.45) * 2.0 * exp(-pow(up / 0.08, 2.0));
    env += vec3(6.0, 5.2, 4.0) * pow(max(dot(r, L), 0.0), 140.0);
    env += portC * pow(max(dot(r, Lp), 0.0), 6.0) * 2.0;
    float f = pow(edge, 5.0);
    col = env * mix(vec3(1.0, 0.76, 0.42), vec3(1.0), f);
  } else if (m > 0.5 && m < 2.5) {
    // Technical shell jacket: dark, faint ripstop grid, a stitched seam; knit cuff ribs.
    vec3 Z = normalize(vAxis);
    float t = vLocal.x, th = vLocal.y;
    vec3 base;
    float tex = 1.0;
    if (m < 1.5) {
      float fold = sin(t * 22.0 + 1.6 * sin(th * 2.0 + t * 3.0)) * 0.5 + 0.5;
      fold = pow(fold, 2.0);
      n = normalize(n + Z * (fold - 0.4) * 0.45 + cross(Z, n) * 0.12 * sin(th * 5.0 + t * 9.0));
      vec2 q = vec2(t * 260.0, th * 42.0);
      vec2 gq = abs(fract(q) - 0.5);
      tex = 1.0 + 0.07 * step(0.44, max(gq.x, gq.y));
      base = vec3(0.034, 0.038, 0.048);
      float seam = smoothstep(0.992, 0.999, cos(th - 2.4));
      float stitch = step(0.45, fract(t * 140.0));
      base += vec3(0.08, 0.085, 0.09) * seam * stitch;
      base *= 1.0 - 0.45 * smoothstep(0.96, 0.99, cos(th - 2.35)) * (1.0 - seam);
    } else {
      vec3 X = normalize(cross(Z, n) + 1e-5);
      n = normalize(n + X * 0.3 * sin(th * 34.0) * step(t, 0.999) * step(0.001, t));
      base = vec3(0.028, 0.03, 0.036);
    }
    float d = max(dot(n, L) * 0.5 + 0.5, 0.0);
    col = base * tex * (sunC * d * d * 0.8 + skyC * (0.55 + 0.35 * n.y) + snowC * 0.3 * max(-n.y, 0.0));
    col += vec3(1.0, 0.7, 0.45) * pow(edge, 3.0) * back * 0.22;
    col += vec3(0.55, 0.6, 0.7) * pow(edge, 2.5) * 0.05;
    col += base * portC * max(dot(n, Lp), 0.0) * 2.0;
    col *= 0.93 + 0.07 * hash(floor(vPos * 1400.0));
  } else {
    // Skin: sun behind the hands, so a cool sky fill on the faces we see, warm light through the edges.
    vec3 alb = vec3(0.68, 0.42, 0.30);
    float joint = 1.0 - smoothstep(0.0, 0.2, min(vLocal.x, 1.0 - vLocal.x));
    float tipK = m > 2.5 ? smoothstep(0.4, 1.0, vLocal.x) : 0.0;
    alb = mix(alb, vec3(0.70, 0.34, 0.26), joint * 0.4 + tipK * 0.45);
    float nail = 0.0;
    if (m > 2.5) {
      float c = cos(vLocal.y);
      nail = smoothstep(0.5, 0.72, c) * smoothstep(0.18, 0.38, vLocal.x) * (1.0 - smoothstep(1.02, 1.2, vLocal.x));
      alb = mix(alb, vec3(0.9, 0.74, 0.7), nail);
    }
    float ndl = dot(n, L);
    float wrap = max((ndl + 0.4) / 1.4, 0.0);
    vec3 diff = sunC * wrap * wrap * 0.75 + skyC * (0.55 + 0.3 * n.y) + snowC * 0.35 * max(-n.y, 0.0) + vec3(0.2, 0.18, 0.17);
    col = alb * diff * 0.6;
    // Knuckle creases: a touch darker and redder where segments meet.
    col *= 1.0 - 0.16 * joint * (1.0 - nail);
    // Subsurface: red light through the thin edges and the fingertips, against the sun.
    float sss = pow(edge, 1.6) * (0.35 + 1.1 * back) + tipK * back * 0.35;
    col += vec3(1.0, 0.36, 0.18) * sss * 0.42;
    // A crisp warm rim where the sun grazes the edge.
    col += sunC * 0.35 * pow(edge, 4.0) * smoothstep(-0.2, 0.4, ndl + 0.3);
    vec3 H = normalize(L + V);
    float spec = pow(max(dot(n, H), 0.0), mix(16.0, 120.0, nail)) * mix(0.05, 0.8, nail);
    col += sunC * spec;
    // Sky glints on the nails.
    col += skyC * nail * pow(max(reflect(-V, n).y, 0.0), 4.0) * 0.35;
    float pl = max(dot(n, Lp), 0.0);
    col += alb * portC * (pl * 0.4 + 0.3 * edge) + portC * pow(edge, 3.0) * 0.12;
  }
  col = aces(col * 0.95);
  col = pow(col, vec3(1.0 / 2.2));
  outColor = vec4(col, 1.0);
}`;

// Sparks: one instanced quad per spark, laid out in screen pixels around the portal.
const sparkVert = `#version 300 es
precision highp float;
uniform vec2 uRes;          // device px
uniform vec4 uRing;         // centre px (y down), radius px, arc fraction 0..1
uniform vec4 uP;            // time, accumulated spin, intensity, ember amount
uniform vec2 uA;            // start angle of the drawn arc, spin on (0..1)
uniform float uDepth;       // NDC depth of the portal plane
uniform float uScale;       // device px per CSS px
out vec2 vUv;
out float vI;
out vec3 vC;
float h(float n) { return fract(sin(n * 12.9898) * 43758.5453); }
void main() {
  int id = gl_InstanceID;
  float f = float(id);
  vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
  float h1 = h(f + 0.13), h2 = h(f * 1.7 + 3.1), h3 = h(f * 2.3 + 7.7), h4 = h(f * 0.37 + 1.9);
  float t = uP.x, R = uRing.z, arc = uRing.w;
  vec2 C = uRing.xy;
  float TAU = 6.2831853;
  bool ember = h4 < uP.w;
  vec2 pos, dir; float len, wid, bright;
  vec3 col;
  float band = 3.0 * uScale + R * 0.035;
  if (!ember) {
    // Ring sparks: a dense band running round the circle, each at its own speed, trailing a streak.
    float speed = 0.7 + 0.9 * h2;
    float a = uA.x + h1 * arc * TAU + uP.y * speed * uA.y;
    float jr = h3 - 0.5;
    float r = R + jr * abs(jr) * 4.0 * band + (h(f + floor(t * 14.0)) - 0.5) * 1.5 * uScale;
    pos = C + vec2(cos(a), -sin(a)) * r;
    dir = vec2(-sin(a), -cos(a));
    len = (8.0 + 46.0 * h2 * h2) * uScale * (0.45 + 0.55 * min(1.0, R / (260.0 * uScale)));
    wid = (0.9 + 1.4 * h1 * h1) * uScale;
    float core = 1.0 - min(1.0, abs(jr) * 2.4);
    bright = (0.5 + 1.3 * h1 * h1 * h1) * (0.35 + 0.65 * core);
    col = mix(vec3(1.0, 0.33, 0.04), vec3(1.0, 0.72, 0.3), h2 * h2 * core);
    // While the arc is being drawn its head, at the fingertip, burns brightest.
    if (arc < 0.999) bright *= 0.55 + 1.6 * smoothstep(0.82, 1.0, h1);
  } else {
    // Embers: thrown off tangentially from the rim, slowing, then falling under gravity.
    float rate = 0.55 + 0.8 * h2;
    float life = fract(t * rate + h1 * 7.0);
    float a0 = uA.x + h3 * arc * TAU + uP.y * 0.9 * uA.y;
    vec2 p0 = C + vec2(cos(a0), -sin(a0)) * (R + (h1 - 0.5) * band);
    vec2 tang = vec2(-sin(a0), -cos(a0));
    vec2 out0 = vec2(cos(a0), -sin(a0));
    float v = (110.0 + 250.0 * h2) * uScale;
    vec2 vel = (tang * (0.75 + 0.5 * h1) * uA.y + tang * 0.25 + out0 * (h1 - 0.3) * 0.7) * v;
    float drag = (1.0 - exp(-life * 2.4)) / 2.4;
    vec2 g = vec2(0.0, 420.0 * uScale);
    pos = p0 + vel * drag + g * life * life * 0.5;
    vec2 vnow = vel * exp(-life * 2.4) + g * life;
    dir = normalize(vnow + 1e-4);
    len = clamp(length(vnow) * 0.03, 2.0 * uScale, 22.0 * uScale);
    wid = (0.9 + 1.2 * h3) * uScale * (1.0 - life * 0.6);
    bright = (1.0 - life) * (1.0 - life) * (0.9 + 1.4 * h2);
    col = mix(vec3(1.0, 0.28, 0.03), vec3(1.0, 0.62, 0.22), (1.0 - life) * h2);
  }
  vec2 side = vec2(-dir.y, dir.x);
  vec2 q = pos - dir * len * corner.x + side * wid * (corner.y - 0.5) * 2.0;
  vec2 ndc = vec2(q.x / uRes.x * 2.0 - 1.0, 1.0 - q.y / uRes.y * 2.0);
  gl_Position = vec4(ndc, uDepth, 1.0);
  vUv = corner;
  vI = bright * uP.z;
  vC = col;
}`;

const sparkFrag = `#version 300 es
precision highp float;
in vec2 vUv;
in float vI;
in vec3 vC;
out vec4 outColor;
void main() {
  float across = 1.0 - abs(vUv.y * 2.0 - 1.0);
  float along = (1.0 - vUv.x);              // bright head, fading tail
  float a = across * across * along * along * vI;
  vec3 c = vC * a * 1.15 + vec3(1.0, 0.82, 0.5) * a * a * 0.22;
  outColor = vec4(c, 0.0);                  // pure added light over whatever is below
}`;

// Glow: a tight orange halo with a hot core line around the drawn arc, transparent inside.
const glowVert = `#version 300 es
precision highp float;
uniform vec2 uRes;
uniform vec4 uRing;
uniform float uDepth;
uniform float uScale;
out vec2 vPx;
void main() {
  vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)) * 2.0 - 1.0;
  float ext = uRing.z * 1.12 + 70.0 * uScale;
  vec2 q = uRing.xy + corner * ext;
  vPx = q;
  gl_Position = vec4(q.x / uRes.x * 2.0 - 1.0, 1.0 - q.y / uRes.y * 2.0, uDepth, 1.0);
}`;
const glowFrag = `#version 300 es
precision highp float;
uniform vec4 uRing;
uniform vec4 uP;        // time, spin, intensity, open
uniform vec2 uA;        // start angle, spin on
uniform float uScale;
uniform float uHole;    // radius of the opening (device px)
in vec2 vPx;
out vec4 outColor;
void main() {
  vec2 d = vPx - uRing.xy;
  float r = length(d), R = max(uRing.z, 1.0);
  float x = (r - R) / uScale;
  if (abs(x) > 70.0 && uHole < 0.5) discard;
  float ang = atan(-d.y, d.x);
  float TAU = 6.2831853;
  // Only the part of the circle drawn so far glows; the head is brightest.
  float rel = mod(ang - uA.x, TAU) / TAU;
  float arc = uRing.w;
  float drawn = arc >= 0.999 ? 1.0 : (1.0 - smoothstep(arc - 0.004, arc + 0.004, rel)) * (0.5 + 0.8 * smoothstep(arc - 0.12, arc, rel));
  float shimmer = 0.5 + 0.5 * sin(ang * 23.0 + r * 0.09 / uScale - uP.x * 7.0) * sin(ang * 9.0 - uP.x * 3.1);
  float band = 3.0 + R / uScale * 0.035;
  float core = exp(-x * x / (1.6 + band * 0.3));
  float halo = exp(-abs(x) / (6.0 + band * 1.2));
  float heat = exp(-pow(max(x, 0.0) / (10.0 + band * 2.0), 2.0)) * step(0.0, x) * (0.6 + 0.4 * shimmer);
  vec3 c = vec3(1.0, 0.85, 0.55) * core * 0.65 + vec3(1.0, 0.4, 0.07) * halo * 0.28 + vec3(1.0, 0.36, 0.06) * heat * 0.08;
  c *= drawn;
  // The opening tears outwards with a hot, flickering lip.
  if (uHole > 0.5) {
    float hx = (r - uHole) / (uScale * (2.0 + 1.5 * shimmer));
    float lip = exp(-hx * hx) * (1.0 - smoothstep(0.85, 1.0, uHole / R));
    c += vec3(1.0, 0.6, 0.22) * lip * 0.9;
  }
  c *= uP.z;
  outColor = vec4(c, 0.0);
}`;

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------
function coneMesh(seg = 22, capRings = 7, bodyRings = 3) {
  const v = [];
  const rows = [];
  for (let i = 0; i <= capRings; i++) rows.push([(i / capRings) * Math.PI / 2, 0]);           // cap A: lat 0..pi/2 param
  for (let i = 1; i < bodyRings; i++) rows.push([0, i / bodyRings]);                           // body
  for (let i = 0; i <= capRings; i++) rows.push([(i / capRings) * Math.PI / 2, 1]);           // cap B
  for (const [lat, t] of rows) for (let j = 0; j <= seg; j++) v.push(lat, (j / seg) * Math.PI * 2, t);
  const idx = [];
  const W = seg + 1;
  for (let r = 0; r < rows.length - 1; r++) for (let j = 0; j < seg; j++) {
    const a = r * W + j, b = a + 1, c = a + W, d = c + 1;
    idx.push(a, c, b, b, c, d);
  }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}
function torusMesh(seg = 28, sides = 10) {
  const v = [];
  for (let i = 0; i <= seg; i++) for (let j = 0; j <= sides; j++) v.push((i / seg) * Math.PI * 2, (j / sides) * Math.PI * 2, -1);
  const idx = [];
  const W = sides + 1;
  for (let i = 0; i < seg; i++) for (let j = 0; j < sides; j++) { const a = i * W + j, b = a + 1, c = a + W, d = c + 1; idx.push(a, c, b, b, c, d); }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}

// ---------------------------------------------------------------------------
// Hand anatomy (metres), local frame: u = towards the thumb, v = back of the hand, w = towards the fingers.
// ---------------------------------------------------------------------------
const FINGERS = [
  // knuckle u, w; segment lengths; radii at knuckle, PIP, DIP, tip; spread angle
  { u: 0.031, w: 0.092, len: [0.043, 0.025, 0.021], r: [0.0121, 0.0107, 0.0094, 0.0079], spread: 0.16 },
  { u: 0.010, w: 0.097, len: [0.047, 0.029, 0.022], r: [0.0125, 0.0110, 0.0096, 0.0082], spread: 0.03 },
  { u: -0.010, w: 0.093, len: [0.044, 0.028, 0.022], r: [0.0119, 0.0104, 0.0091, 0.0077], spread: -0.11 },
  { u: -0.029, w: 0.083, len: [0.034, 0.021, 0.019], r: [0.0106, 0.0092, 0.0082, 0.0070], spread: -0.25 },
];

// A pose: wrist position (view space), hand orientation, finger state.
const makePose = (side) => ({ x: side * 0.2, y: -0.5, z: -0.38, yaw: 0, pitch: 20, roll: 0, curl: 0.55, spread: 0.1, thumb: 0.2, tcurl: 0.4, ex: side * 0.26, ey: -0.55, ez: -0.12 });

export function createHands(canvas) {
  const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: true, depth: true, powerPreference: 'high-performance' });
  if (!gl) return { resize() {}, frame() { return NOOP; } };
  const NOOP = { portal: { x: 0, y: 0, r: 0 } };

  const compile = (vs, fs) => {
    const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
    const p = gl.createProgram();
    gl.attachShader(p, sh(gl.VERTEX_SHADER, vs)); gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
    const u = new Proxy({}, { get: (o, k) => (k in o ? o[k] : (o[k] = gl.getUniformLocation(p, k))) });
    return { p, u };
  };
  const MESH = compile(meshVert, meshFrag);
  const SPARK = compile(sparkVert, sparkFrag);
  const GLOW = compile(glowVert, glowFrag);

  // Instance buffer: 12 floats per capsule.
  const MAXI = 64;
  const inst = new Float32Array(MAXI * 12);
  const instBuf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, instBuf);
  gl.bufferData(gl.ARRAY_BUFFER, inst.byteLength, gl.DYNAMIC_DRAW);
  const ringInst = new Float32Array(4 * 12);
  const ringBuf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, ringBuf);
  gl.bufferData(gl.ARRAY_BUFFER, ringInst.byteLength, gl.DYNAMIC_DRAW);

  function vao(mesh, ib) {
    const va = gl.createVertexArray();
    gl.bindVertexArray(va);
    const vb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vb);
    gl.bufferData(gl.ARRAY_BUFFER, mesh.v, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    const eb = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, eb);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.i, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, ib);
    for (let k = 0; k < 3; k++) {
      gl.enableVertexAttribArray(1 + k);
      gl.vertexAttribPointer(1 + k, 4, gl.FLOAT, false, 48, k * 16);
      gl.vertexAttribDivisor(1 + k, 1);
    }
    gl.bindVertexArray(null);
    return { va, count: mesh.i.length };
  }
  const cone = vao(coneMesh(), instBuf);
  const torus = vao(torusMesh(), ringBuf);
  const emptyVao = gl.createVertexArray();

  // -------------------------------------------------------------------------
  // The gesture: one Anikit timeline over two poses and the portal, scrubbed by g (0..1 → 0..10 s).
  // -------------------------------------------------------------------------
  const Lh = makePose(-1), Rh = makePose(1);
  const portal = { arc: 0, size: 0, open: 0, grow: 0, glow: 0 };
  const circle = { phase: 0, amp: 0 };
  const io = 'inOutCubic', oo = 'outCubic';
  const TURNS = 2.5;
  const tl = timeline({ autoplay: false })
    // Arms rise into view (0 → 2.5).
    .add(Lh, { x: [-0.2, -0.13], y: [-0.5, -0.11], z: [-0.38, -0.43], pitch: [20, 38], yaw: [10, 16], roll: [-30, -12], curl: [0.55, 0.34], ex: [-0.26, -0.22], ey: [-0.55, -0.5] }, { duration: 2.5, ease: oo }, 0)
    .add(Rh, { x: [0.2, 0.13], y: [-0.5, -0.13], z: [-0.38, -0.42], pitch: [20, 30], yaw: [-10, -16], roll: [30, 14], curl: [0.55, 0.38], ex: [0.26, 0.22], ey: [-0.55, -0.5] }, { duration: 2.7, ease: oo }, 0.15)
    // The ring hand reaches forward and a little left, palm out, fingers relaxed and spread: the anchor (2.4 → 4).
    .add(Lh, { x: -0.13, y: -0.035, z: -0.6, pitch: 80, yaw: 20, roll: -4, curl: [0.34, 0.12], spread: [0.1, 0.75], thumb: [0.2, 0.8], tcurl: [0.4, 0.12], ex: -0.21, ey: -0.42, ez: -0.27 }, { duration: 1.6, ease: io }, 2.4)
    // The drawing hand comes up beside it, index leading (3.0 → 4.1).
    .add(Rh, { x: 0.075, y: -0.075, z: -0.5, pitch: 58, yaw: -10, roll: 12, curl: 0.24, spread: 0.3, thumb: 0.45, tcurl: 0.35, ex: 0.2, ey: -0.44, ez: -0.18 }, { duration: 1.1, ease: io }, 3.0)
    // Two and a half big circles (4.0 → 7.0); the first one draws the ring, the rest spin it up.
    .add(circle, { amp: [0, 1] }, { duration: 0.45, ease: 'inOutSine' }, 3.95)
    .add(circle, { phase: [0, Math.PI * 2 * TURNS] }, { duration: 3.0, ease: 'inOutSine' }, 4.0)
    .add(portal, { glow: [0, 1] }, { duration: 0.5, ease: 'outSine' }, 4.0)
    // Both hands pull back and apart as the ring expands to full size (7.0 → 7.6).
    .add(circle, { amp: 0 }, { duration: 0.5, ease: 'inOutSine' }, 7.0)
    .add(portal, { size: [0, 1] }, { duration: 0.65, ease: io }, 6.95)
    .add(Lh, { x: -0.18, y: -0.07, z: -0.5, yaw: 26, curl: 0.2 }, { duration: 0.65, ease: io }, 7.0)
    .add(Rh, { x: 0.17, y: -0.1, z: -0.45, yaw: -20, curl: 0.3 }, { duration: 0.65, ease: io }, 7.0)
    .add(portal, { open: [0, 1] }, { duration: 0.55, ease: 'inOutSine' }, 7.0)
    // Step through (7.6 → 10): the portal grows past the screen, the arms drop away.
    .add(portal, { grow: [0, 1] }, { duration: 2.4, ease: 'inCubic' }, 7.6)
    .add(Lh, { x: -0.22, y: -0.52, z: -0.45, pitch: 30, curl: 0.4, spread: 0.3, ey: -0.62 }, { duration: 1.6, ease: 'inCubic' }, 8.0)
    .add(Rh, { x: 0.22, y: -0.52, z: -0.45, pitch: 30, curl: 0.4, ey: -0.62 }, { duration: 1.5, ease: 'inCubic' }, 7.9);
  const TL = 10;
  // g is spring-smoothed so fast scrolling never makes the hands jerk.
  const gS = { v: 0 };
  let gTarget = -1, lastSeek = -1;

  // -------------------------------------------------------------------------
  // Skeleton → instances (no allocation per frame).
  // -------------------------------------------------------------------------
  let n = 0;
  const F = [0, 0, 0], D = [0, 0, 0], S = [0, 0, 0], T = [0, 0, 0], tmpA = [0, 0, 0], tmpB = [0, 0, 0], dirL = [0, 0, 0], dorL = [0, 0, 0], W = [0, 0, 0], E = [0, 0, 0], AR = [0, 0, 0];
  const R = (k, x, y, z) => { k[0] = x; k[1] = y; k[2] = z; };
  function push(a, ra, b, rb, ref, mat) {
    const o = n * 12;
    inst[o] = a[0]; inst[o + 1] = a[1]; inst[o + 2] = a[2]; inst[o + 3] = ra;
    inst[o + 4] = b[0]; inst[o + 5] = b[1]; inst[o + 6] = b[2]; inst[o + 7] = rb;
    inst[o + 8] = ref[0]; inst[o + 9] = ref[1]; inst[o + 10] = ref[2]; inst[o + 11] = mat;
    n++;
  }
  // Local hand coordinates → view space.
  function toView(out, u, v, w) {
    out[0] = W[0] + T[0] * u + D[0] * v + F[0] * w;
    out[1] = W[1] + T[1] * u + D[1] * v + F[1] * w;
    out[2] = W[2] + T[2] * u + D[2] * v + F[2] * w;
  }
  function dirView(out, u, v, w) {
    out[0] = T[0] * u + D[0] * v + F[0] * w; out[1] = T[1] * u + D[1] * v + F[1] * w; out[2] = T[2] * u + D[2] * v + F[2] * w;
  }
  const RING = [0, 0, 0, 0, 0, 0];
  let ringN = 0;
  function buildHand(P, side, ring, t) {
    const d2r = Math.PI / 180;
    const yaw = P.yaw * d2r, pitch = P.pitch * d2r, roll = P.roll * d2r;
    // Fingers' direction and the back of the hand.
    R(F, Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch));
    R(D, -Math.sin(yaw) * Math.sin(pitch), Math.cos(pitch), Math.cos(yaw) * Math.sin(pitch));
    // S = F × D, then roll about F.
    R(S, F[1] * D[2] - F[2] * D[1], F[2] * D[0] - F[0] * D[2], F[0] * D[1] - F[1] * D[0]);
    const cr = Math.cos(roll), sr = Math.sin(roll);
    for (let k = 0; k < 3; k++) { const d = D[k], s = S[k]; D[k] = d * cr + s * sr; S[k] = s * cr - d * sr; }
    // Thumb side: +S for the left hand, -S for the right.
    const m = side < 0 ? 1 : -1;
    R(T, S[0] * m, S[1] * m, S[2] * m);
    R(W, P.x, P.y, P.z);
    R(E, P.ex, P.ey, P.ez);
    // Forearm, sleeve and cuff.
    let ax = W[0] - E[0], ay = W[1] - E[1], az = W[2] - E[2];
    const al = Math.hypot(ax, ay, az); ax /= al; ay /= al; az /= al;
    R(AR, ax, ay, az);
    const at = (k, out) => R(out, W[0] - ax * k, W[1] - ay * k, W[2] - az * k);
    at(0.62, tmpA); at(0.1, tmpB); push(tmpA, 0.062, tmpB, 0.05, D, 1);            // sleeve
    at(0.105, tmpA); at(0.05, tmpB); push(tmpA, 0.049, tmpB, 0.033, D, 2);          // knit cuff, gathered at the wrist
    at(0.075, tmpA); toView(tmpB, 0, -0.002, 0.012); push(tmpA, 0.03, tmpB, 0.026, D, 0); // wrist
    // Palm: metacarpals side by side fuse into the back of the hand; a pad underneath.
    for (let f = 0; f < 4; f++) {
      const fi = FINGERS[f];
      toView(tmpA, fi.u * 0.55, 0.0, 0.016);
      toView(tmpB, fi.u, 0.0015, fi.w - 0.004);
      push(tmpA, 0.0155, tmpB, fi.r[0] * 1.1, D, 0);
    }
    toView(tmpA, 0.004, -0.006, 0.03); toView(tmpB, -0.002, -0.006, 0.072); push(tmpA, 0.026, tmpB, 0.024, D, 0);
    toView(tmpA, -0.018, -0.004, 0.035); toView(tmpB, -0.022, -0.004, 0.07); push(tmpA, 0.018, tmpB, 0.016, D, 0);
    toView(tmpA, 0.022, -0.009, 0.025); toView(tmpB, 0.03, -0.01, 0.05); push(tmpA, 0.017, tmpB, 0.014, D, 0);   // thenar mound
    // Fingers.
    const breathe = Math.sin(t * 1.3 + side) * 0.03;
    for (let f = 0; f < 4; f++) {
      const fi = FINGERS[f];
      const sp = fi.spread * (0.35 + P.spread * 1.15);
      const c = Math.max(0, P.curl + breathe + (f - 1.5) * 0.04 * P.curl);
      const bends = [c * 1.15, c * 1.55, c * 1.05];
      toView(tmpA, fi.u, 0.0015, fi.w - 0.004);
      let a = -0.06;
      for (let s = 0; s < 3; s++) {
        a += bends[s];
        // Bend towards the palm about the axis across the finger.
        dirView(dirL, Math.sin(sp) * Math.cos(a), -Math.sin(a), Math.cos(sp) * Math.cos(a));
        dirView(dorL, Math.sin(sp) * Math.sin(a), Math.cos(a), Math.cos(sp) * Math.sin(a));
        const L = fi.len[s];
        R(tmpB, tmpA[0] + dirL[0] * L, tmpA[1] + dirL[1] * L, tmpA[2] + dirL[2] * L);
        const rb = s === 2 ? fi.r[3] : fi.r[s + 1];
        // The fingertip is a little shorter so the round cap ends at the true length.
        if (s === 2) { tmpB[0] -= dirL[0] * rb * 0.6; tmpB[1] -= dirL[1] * rb * 0.6; tmpB[2] -= dirL[2] * rb * 0.6; }
        push(tmpA, fi.r[s], tmpB, rb, dorL, s === 2 ? 3 : 0);
        if (side > 0 && f === 0 && s === 2) R(TIP, tmpB[0] + dirL[0] * rb, tmpB[1] + dirL[1] * rb, tmpB[2] + dirL[2] * rb);
        if (ring && s === 0 && (f === 1 || f === 2)) {
          // The ring: a band on the middle and ring fingers' first segment.
          const k = ringN * 12, tt = 0.2;
          ringInst[k] = tmpA[0] + (tmpB[0] - tmpA[0]) * tt; ringInst[k + 1] = tmpA[1] + (tmpB[1] - tmpA[1]) * tt; ringInst[k + 2] = tmpA[2] + (tmpB[2] - tmpA[2]) * tt;
          ringInst[k + 3] = lerpR(fi.r[0], fi.r[1], tt) + 0.0024;
          ringInst[k + 4] = dirL[0]; ringInst[k + 5] = dirL[1]; ringInst[k + 6] = dirL[2]; ringInst[k + 7] = 0.0036;
          ringInst[k + 8] = dorL[0]; ringInst[k + 9] = dorL[1]; ringInst[k + 10] = dorL[2]; ringInst[k + 11] = 4;
          ringN++;
          if (f === 2) {
            // A bridge plate joining the two bands across the back of the fingers.
            const a0 = ringN - 2;
            for (let q = 0; q < 3; q++) RING[q] = ringInst[a0 * 12 + q] + ringInst[a0 * 12 + 8 + q] * (ringInst[a0 * 12 + 3] + 0.0012);
            for (let q = 0; q < 3; q++) RING[3 + q] = ringInst[(a0 + 1) * 12 + q] + ringInst[(a0 + 1) * 12 + 8 + q] * (ringInst[(a0 + 1) * 12 + 3] + 0.0012);
            tmpRingA[0] = RING[0]; tmpRingA[1] = RING[1]; tmpRingA[2] = RING[2];
            tmpRingB[0] = RING[3]; tmpRingB[1] = RING[4]; tmpRingB[2] = RING[5];
            push(tmpRingA, 0.006, tmpRingB, 0.006, dirL, 4);
          }
        }
        R(tmpA, tmpB[0], tmpB[1], tmpB[2]);
      }
    }
    // Thumb: metacarpal, proximal, distal; abducts and curls.
    const ab = P.thumb, tc = P.tcurl;
    let du = lerpR(0.5, 0.86, ab), dv = lerpR(-0.5, -0.12, ab), dw = lerpR(0.7, 0.5, ab);
    toView(tmpA, 0.024, -0.008, 0.022);
    const TL3 = [0.04, 0.032, 0.026], TR = [0.0135, 0.0115, 0.0098, 0.0085];
    for (let s = 0; s < 3; s++) {
      const bend = s === 0 ? 0 : tc * (s === 1 ? 0.6 : 0.9);
      du -= bend * 0.9; dv -= bend * 0.35; dw += bend * 0.25;
      const l = Math.hypot(du, dv, dw); du /= l; dv /= l; dw /= l;
      dirView(dirL, du, dv, dw);
      let L = TL3[s];
      if (s === 2) L -= TR[3] * 0.6;
      R(tmpB, tmpA[0] + dirL[0] * L, tmpA[1] + dirL[1] * L, tmpA[2] + dirL[2] * L);
      // Dorsal side of the thumb faces outwards-up.
      dirView(dorL, -dv * 0.3 + 0.2, 1, -0.2);
      push(tmpA, TR[s], tmpB, TR[s + 1], dorL, s === 2 ? 3 : 0);
      R(tmpA, tmpB[0], tmpB[1], tmpB[2]);
    }
  }
  const tmpRingA = [0, 0, 0], tmpRingB = [0, 0, 0], TIP = [0, 0, -0.5];
  const lerpR = (a, b, t) => a + (b - a) * t;

  // -------------------------------------------------------------------------
  // Frame
  // -------------------------------------------------------------------------
  const out = { portal: { x: 0, y: 0, r: 0 } };
  const proj = new Float32Array(16);
  let cssW = 1, cssH = 1, dprK = 1, fov = 0, tanH = 0.6;
  const NSPARK = 4800;
  // The sun is low over the horizon in front of you.
  const sun = [0.18, 0.2, -1];
  function resize(w, h, dpr) {
    cssW = w; cssH = h; dprK = Math.min(dpr || 1, 2);
    canvas.width = Math.max(2, Math.round(w * dprK)); canvas.height = Math.max(2, Math.round(h * dprK));
    // Vertical field of view: a natural first-person 62°, wider on portrait screens.
    const aspect = w / h;
    fov = 62 * Math.PI / 180 * (aspect < 1 ? 1.25 : 1);
    tanH = Math.tan(fov / 2);
    const f = 1 / tanH, near = 0.03, far = 20;
    proj.fill(0);
    proj[0] = f / aspect; proj[5] = f; proj[10] = (far + near) / (near - far); proj[11] = -1; proj[14] = (2 * far * near) / (near - far);
  }
  resize(canvas.clientWidth || innerWidth, canvas.clientHeight || innerHeight, devicePixelRatio);

  const PORTAL_Z = -1.6;
  const portalDepth = () => { const z = PORTAL_Z; return (proj[10] * z + proj[14]) / -z; };
  // View space → CSS px.
  const scr = [0, 0, 0];
  function project(x, y, z) {
    const k = 1 / (-z * tanH);
    scr[0] = (x * k / (cssW / cssH) * 0.5 + 0.5) * cssW;
    scr[1] = (0.5 - y * k * 0.5) * cssH;
    scr[2] = k * 0.5 * cssH;   // px per metre at this depth
  }
  const CIRCLE_R = 0.085;      // metres, the drawing fingertip's circle
  let spinAcc = 0, drewLast = true;

  function draw(sparkOn, cx, cy, ringR, arc, a0, spinOn, gr, hole) {
    const Wd = canvas.width, Hd = canvas.height;
    gl.viewport(0, 0, Wd, Hd);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (n > 0) {
      gl.enable(gl.DEPTH_TEST); gl.depthMask(true); gl.disable(gl.BLEND);
      gl.enable(gl.CULL_FACE); gl.cullFace(gl.FRONT);
      gl.useProgram(MESH.p);
      gl.uniformMatrix4fv(MESH.u.uProj, false, proj);
      gl.uniform3f(MESH.u.uSun, sun[0], sun[1], sun[2]);
      // The portal lights the hands: its centre unprojected onto the portal plane.
      const vx = ((cx / cssW) * 2 - 1) * tanH * (cssW / cssH) * -PORTAL_Z, vy = (1 - (cy / cssH) * 2) * tanH * -PORTAL_Z;
      gl.uniform4f(MESH.u.uPortal, vx, vy, PORTAL_Z, sparkOn ? portal.glow * (1 - gr * 0.5) * (0.4 + 0.6 * Math.min(1, arc * 1.5)) : 0);
      gl.uniform1f(MESH.u.uTime, 0);
      gl.bindBuffer(gl.ARRAY_BUFFER, instBuf);
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, inst, 0, n * 12);
      gl.bindVertexArray(cone.va);
      gl.drawElementsInstanced(gl.TRIANGLES, cone.count, gl.UNSIGNED_SHORT, 0, n);
      if (ringN > 0) {
        gl.bindBuffer(gl.ARRAY_BUFFER, ringBuf);
        gl.bufferSubData(gl.ARRAY_BUFFER, 0, ringInst, 0, ringN * 12);
        gl.bindVertexArray(torus.va);
        gl.drawElementsInstanced(gl.TRIANGLES, torus.count, gl.UNSIGNED_SHORT, 0, ringN);
      }
      gl.disable(gl.CULL_FACE);
    }
    if (sparkOn) {
      gl.enable(gl.DEPTH_TEST); gl.depthMask(false);
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.bindVertexArray(emptyVao);
      const fade = 1 - gr * gr;
      const k = dprK;
      gl.useProgram(GLOW.p);
      gl.uniform2f(GLOW.u.uRes, Wd, Hd);
      gl.uniform4f(GLOW.u.uRing, cx * k, cy * k, ringR * k, arc);
      gl.uniform4f(GLOW.u.uP, timeNow, spinAcc, portal.glow * fade, portal.open);
      gl.uniform2f(GLOW.u.uA, a0, spinOn);
      gl.uniform1f(GLOW.u.uDepth, portalDepth());
      gl.uniform1f(GLOW.u.uScale, k);
      gl.uniform1f(GLOW.u.uHole, hole * k);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.useProgram(SPARK.p);
      gl.uniform2f(SPARK.u.uRes, Wd, Hd);
      gl.uniform4f(SPARK.u.uRing, cx * k, cy * k, ringR * k, arc);
      gl.uniform4f(SPARK.u.uP, timeNow, spinAcc, portal.glow * fade, 0.3 + 0.12 * spinOn);
      gl.uniform2f(SPARK.u.uA, a0, spinOn);
      gl.uniform1f(SPARK.u.uDepth, portalDepth());
      gl.uniform1f(SPARK.u.uScale, k);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, NSPARK);
      gl.depthMask(true);
      gl.disable(gl.BLEND);
    }
  }
  let timeNow = 0;

  function frame(t, dt, g, look) {
    timeNow = t;
    g = Math.min(1, Math.max(0, g || 0));
    // Spring-smooth the gesture (retarget only when g actually moves).
    if (Math.abs(g - gTarget) > 0.0005) { gTarget = g; spring(gS, { v: g }, { stiffness: 90, damping: 19 }); }
    const gv = Math.min(1, Math.max(0, gS.v));
    if (Math.abs(gv - lastSeek) > 1e-5) { tl.seek(gv * TL); lastSeek = gv; }

    // The drawing hand runs round a big circle in the screen plane.
    const lx = look ? look.x || 0 : 0, ly = look ? look.y || 0 : 0;
    const sway = Math.sin(t * 0.9) * 0.004, bob = Math.sin(t * 1.4) * 0.003;
    const ph = circle.phase - Math.PI / 2;   // start at the bottom of the circle
    const ca = Math.cos(ph), sa = Math.sin(ph);
    const cx3 = ca * CIRCLE_R * circle.amp, cy3 = (sa + 1) * CIRCLE_R * circle.amp;
    const rx = Rh.x, ry = Rh.y, lxp = Lh.x, lyp = Lh.y;
    Rh.x += cx3 - lx * 0.012 + sway; Rh.y += cy3 - ly * 0.008 + bob;
    Lh.x += -lx * 0.012 - sway * 0.7; Lh.y += -ly * 0.008 + bob * 0.8;
    n = 0; ringN = 0;
    if (Lh.y > -0.49 || Rh.y > -0.49) {
      buildHand(Lh, -1, true, t);
      buildHand(Rh, 1, false, t);
    }
    Rh.x = rx; Rh.y = ry; Lh.x = lxp; Lh.y = lyp;

    // The ring being drawn: the circle the index fingertip traces, in screen px.
    const arc = Math.min(1, circle.phase / (Math.PI * 2));
    project(TIP[0] - ca * CIRCLE_R * circle.amp, TIP[1] - sa * CIRCLE_R * circle.amp, TIP[2]);
    const dX = scr[0], dY = scr[1], dR = CIRCLE_R * scr[2];
    // The finished portal, and beyond it the screen-filling step through.
    const minS = Math.min(cssW, cssH);
    const R0 = Math.min(cssW * 0.4, cssH * 0.36);
    const cover = Math.hypot(cssW, cssH) * 0.62;
    const s = portal.size, gr = portal.grow;
    const fx = cssW * 0.5 + lx * minS * 0.02, fy = lerpR(cssH * 0.44, cssH * 0.5, gr) + ly * minS * 0.015;
    const cx = lerpR(dX, fx, s), cy = lerpR(dY, fy, s);
    const ringR = lerpR(lerpR(dR, R0, s), cover * 1.08, gr);
    // Head of the arc sits at the fingertip; once closed, the ring spins up.
    const a0 = ph - arc * Math.PI * 2;
    const spinOn = arc < 0.85 ? 0 : arc >= 1 ? 1 : (arc - 0.85) / 0.15;
    spinAcc += dt * (1.2 + 2.0 * s) * spinOn;
    const P = out.portal;
    P.x = cx; P.y = cy;
    P.r = portal.open <= 0.001 ? 0 : ringR * 0.985 * portal.open;

    const sparkOn = arc > 0.001 && portal.glow > 0.001;
    // Nothing to show: clear once, then skip drawing entirely.
    if (n === 0 && !sparkOn) {
      if (drewLast) { gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT); drewLast = false; }
      return out;
    }
    drewLast = true;
    draw(sparkOn, cx, cy, ringR, arc, a0, spinOn, gr, P.r);
    return out;
  }

  // Pre-warm: run every program once at load so the portal never stalls when it first opens.
  {
    const saveN = n; n = 0;
    buildHand(Lh, -1, true, 0); buildHand(Rh, 1, false, 0);
    draw(true, 0, 0, 1, 0.5, 0, 0, 0, 0);
    gl.flush();
    n = saveN; ringN = 0;
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  }
  return { resize, frame };
}
