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
uniform vec3 uSun;        // direction to the sun (behind the viewer)
uniform vec4 uPortal;     // portal centre (view space), glow strength
uniform float uTime;
out vec4 outColor;
float hash(vec3 p) { p = fract(p * 0.3183099 + 0.1); p *= 17.0; return fract(p.x * p.y * p.z * (p.x + p.y + p.z)); }
vec3 aces(vec3 x) { return clamp(x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
void main() {
  vec3 n = normalize(vN);
  vec3 V = normalize(-vPos);
  vec3 L = normalize(uSun);
  vec3 sunC = vec3(1.0, 0.80, 0.58) * 2.3;
  vec3 skyC = vec3(0.55, 0.66, 0.9);
  vec3 Lp = uPortal.xyz - vPos;
  float dp = length(Lp); Lp /= dp;
  vec3 portC = vec3(1.0, 0.5, 0.14) * uPortal.w * 2.2;
  float m = vMat;
  vec3 col;
  if (m > 3.5) {
    // Metal ring: polished, warm reflections of the sky and the sparks.
    vec3 r = reflect(-V, n);
    float up = r.y * 0.5 + 0.5;
    vec3 env = mix(vec3(0.25, 0.22, 0.2), vec3(1.0, 0.86, 0.66), smoothstep(0.3, 0.9, up)) + vec3(1.2) * pow(max(dot(r, L), 0.0), 60.0);
    env += portC * pow(max(dot(r, Lp), 0.0), 8.0) * 1.5;
    float f = pow(1.0 - max(dot(n, V), 0.0), 5.0);
    col = env * mix(vec3(0.92, 0.78, 0.55), vec3(1.0), f);
  } else if (m > 0.5 && m < 2.5) {
    // Sleeve and cuff: dark wool with folds and a soft sheen.
    vec3 Z = normalize(vAxis);
    float t = vLocal.x, th = vLocal.y;
    if (m < 1.5) {
      float fold = sin(t * 26.0 + 1.6 * sin(th * 2.0 + t * 3.0)) * 0.5 + 0.5;
      fold = pow(fold, 2.0);
      n = normalize(n + Z * (fold - 0.4) * 0.55 + cross(Z, n) * 0.15 * sin(th * 5.0 + t * 9.0));
    } else {
      vec3 X = normalize(cross(Z, n) + 1e-5);
      n = normalize(n + X * 0.3 * sin(th * 34.0) * step(t, 0.999) * step(0.001, t));
    }
    vec3 base = m < 1.5 ? vec3(0.05, 0.056, 0.072) : vec3(0.036, 0.04, 0.05);
    float d = max(dot(n, L) * 0.6 + 0.4, 0.0);
    float sheen = pow(1.0 - max(dot(n, V), 0.0), 2.5);
    col = base * (sunC * d * 0.9 + skyC * (0.35 + 0.25 * n.y));
    col += vec3(0.6, 0.55, 0.5) * sheen * 0.16 * (0.4 + 0.6 * d);
    col += base * portC * max(dot(n, Lp), 0.0) * 1.2;
    col *= 0.92 + 0.08 * hash(floor(vPos * 900.0));
  } else {
    // Skin: wrapped diffuse with a warm red terminator, soft sheen, rim light.
    vec3 alb = vec3(0.66, 0.39, 0.27);
    // Blood shows through at the joints and fingertips.
    float joint = 1.0 - smoothstep(0.0, 0.22, min(vLocal.x, 1.0 - vLocal.x));
    alb = mix(alb, vec3(0.70, 0.33, 0.25), joint * 0.35 + (m > 2.5 ? smoothstep(0.5, 1.0, vLocal.x) * 0.45 : 0.0));
    // Nails on the dorsal side of the fingertips.
    float nail = 0.0;
    if (m > 2.5) {
      float c = cos(vLocal.y);
      nail = smoothstep(0.55, 0.75, c) * smoothstep(0.15, 0.35, vLocal.x) * (1.0 - smoothstep(1.05, 1.2, vLocal.x));
      alb = mix(alb, vec3(0.86, 0.66, 0.6), nail);
    }
    float ndl = dot(n, L);
    float wrap = max((ndl + 0.45) / 1.45, 0.0);
    float sss = smoothstep(-0.45, 0.15, ndl) * (1.0 - smoothstep(0.15, 0.7, ndl));
    vec3 diff = sunC * wrap * wrap * 1.15 + skyC * (0.2 + 0.2 * n.y);
    col = alb * diff * 0.62;
    col += alb * vec3(1.0, 0.25, 0.12) * sss * 0.75;
    vec3 H = normalize(L + V);
    float spec = pow(max(dot(n, H), 0.0), mix(18.0, 80.0, nail)) * mix(0.08, 0.6, nail);
    col += sunC * spec;
    // Rim from the bright sky ahead.
    float rim = pow(1.0 - max(dot(n, V), 0.0), 3.0);
    col += vec3(1.0, 0.6, 0.42) * rim * 0.3 * (0.5 + 0.5 * n.y);
    // The portal lights the hands from in front.
    float pl = max(dot(n, Lp), 0.0);
    col += alb * portC * (pl * 0.35 + 0.35 * rim) + portC * rim * 0.12;
    // Crease shading near joints (vLocal.x at the sphere caps).
  }
  col = aces(col * 0.82);
  col = pow(col, vec3(1.0 / 2.2));
  outColor = vec4(col, 1.0);
}`;

// Sparks: one instanced quad per spark, laid out in screen pixels around the portal.
const sparkVert = `#version 300 es
precision highp float;
uniform vec2 uRes;          // device px
uniform vec4 uRing;         // centre px (y down), radius px, arc fraction 0..1
uniform vec4 uP;            // time, spin, intensity, ember amount
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
  vec3 col = mix(vec3(1.0, 0.42, 0.08), vec3(1.0, 0.85, 0.45), h2 * h2);
  if (!ember) {
    // Ring sparks: run round the circle, each at its own speed, trailing a streak.
    float speed = 1.6 + 1.6 * h2;
    float a = (h1 * arc) * TAU + t * uP.y * speed * 0.35;
    float jr = h3 - 0.5;
    float jitter = jr * abs(jr) * 0.16 + (h(f + floor(t * 12.0)) - 0.5) * 0.01;
    float r = R * (1.0 + jitter);
    pos = C + vec2(cos(a), -sin(a)) * r;
    dir = vec2(sin(a), cos(a));                       // tangent, direction of travel
    len = (6.0 + 40.0 * h2 * h2) * uScale * (0.4 + 0.6 * min(1.0, R / 300.0 / uScale));
    wid = (0.8 + 1.3 * h1 * h1) * uScale;
    bright = (0.45 + 1.4 * h1 * h1 * h1) * (1.0 - abs(jr) * 1.2);
    col = mix(vec3(1.0, 0.36, 0.05), vec3(1.0, 0.78, 0.4), h2 * h2 * (1.0 - abs(jr) * 2.0));
    // Only the drawn part of the arc shows; the leading edge is brightest.
    float head = mod(h1 * arc + 1.0, 1.0);
    bright *= arc >= 0.999 ? 1.0 : smoothstep(0.0, 0.02, arc - h1 * arc) * (0.6 + 0.8 * smoothstep(0.8, 1.0, h1));
  } else {
    // Embers: thrown off tangentially, slowing, falling, fading.
    float rate = 0.5 + 0.7 * h2;
    float life = fract(t * rate + h1 * 7.0);
    float a0 = h3 * arc * TAU + (t - life / rate) * uP.y * 0.9;
    vec2 p0 = C + vec2(cos(a0), -sin(a0)) * R;
    vec2 tang = vec2(sin(a0), cos(a0));
    vec2 out0 = vec2(cos(a0), -sin(a0));
    float v = (120.0 + 260.0 * h2) * uScale;
    vec2 vel = (tang * (0.8 + 0.4 * h1) + out0 * (h1 - 0.35) * 0.9) * v;
    float drag = (1.0 - exp(-life * 2.2)) / 2.2;
    vec2 g = vec2(0.0, 380.0 * uScale);
    pos = p0 + vel * drag + g * life * life * 0.5;
    vec2 vnow = vel * exp(-life * 2.2) + g * life;
    dir = normalize(vnow + 1e-4);
    len = clamp(length(vnow) * 0.035, 2.0 * uScale, 26.0 * uScale);
    wid = (0.9 + 1.4 * h3) * uScale * (1.0 - life * 0.6);
    bright = (1.0 - life) * (1.0 - life) * (1.0 + 1.4 * h2);
    col = mix(vec3(1.0, 0.32, 0.05), vec3(1.0, 0.7, 0.3), (1.0 - life) * h2);
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
  vec3 c = vC * a * 1.25 + vec3(1.0, 0.8, 0.5) * a * a * 0.18;
  outColor = vec4(c, 0.0);                  // pure added light over whatever is below
}`;

// Glow: a soft halo around the ring and a heat-shimmer band, transparent inside.
const glowVert = `#version 300 es
precision highp float;
uniform vec2 uRes;
uniform vec4 uRing;
uniform float uDepth;
out vec2 vPx;
void main() {
  vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)) * 2.0 - 1.0;
  float ext = uRing.z * 1.45 + 60.0;
  vec2 q = uRing.xy + corner * ext;
  vPx = q;
  gl_Position = vec4(q.x / uRes.x * 2.0 - 1.0, 1.0 - q.y / uRes.y * 2.0, uDepth, 1.0);
}`;
const glowFrag = `#version 300 es
precision highp float;
uniform vec4 uRing;
uniform vec4 uP;        // time, spin, intensity, open
uniform float uScale;
uniform float uHole;    // radius of the opening (device px)
in vec2 vPx;
out vec4 outColor;
void main() {
  vec2 d = vPx - uRing.xy;
  float r = length(d), R = max(uRing.z, 1.0);
  float ang = atan(d.y, d.x);
  float x = (r - R) / uScale;
  float shimmer = 0.5 + 0.5 * sin(ang * 23.0 + r * 0.09 / uScale - uP.x * 7.0) * sin(ang * 9.0 - uP.x * 3.1);
  float edge = exp(-x * x / 60.0);
  float halo = exp(-abs(x) / (26.0 + R / uScale * 0.06));
  float outer = exp(-max(x, 0.0) / (90.0 + R / uScale * 0.2)) * step(0.0, x);
  vec3 c = vec3(1.0, 0.42, 0.08) * (halo * 0.3 + outer * 0.1) + vec3(1.0, 0.58, 0.2) * edge * (0.12 + 0.2 * shimmer);
  // Inside, a faint warm wash so the edge of the opening reads as hot.
  float inside = (1.0 - smoothstep(-0.4, 0.0, x / (R / uScale))) * exp(-abs(x) / 50.0);
  c += vec3(1.0, 0.45, 0.1) * inside * 0.06;
  // The opening tears outwards with a hot, flickering lip.
  if (uHole > 0.5) {
    float hx = (r - uHole) / (uScale * (2.5 + 2.0 * shimmer));
    float lip = exp(-hx * hx) * (1.0 - smoothstep(0.85, 1.0, uHole / R));
    c += vec3(1.0, 0.6, 0.22) * lip * 0.9;
  }
  float ext = uRing.z * 1.45 + 60.0;
  vec2 e = abs(d) / ext;
  c *= uP.z * (1.0 - smoothstep(0.6, 1.0, max(e.x, e.y)));
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
  const portal = { arc: 0, size: 0, open: 0, grow: 0, spin: 0, glow: 0 };
  const circle = { phase: 0, amp: 0 };
  const io = 'inOutCubic', oo = 'outCubic';
  const tl = timeline({ autoplay: false })
    // Arms rise into view (0 → 2.5).
    .add(Lh, { x: [-0.2, -0.125], y: [-0.5, -0.115], z: [-0.38, -0.42], pitch: [20, 35], yaw: [10, 18], roll: [-30, -12], curl: [0.55, 0.32], ex: [-0.26, -0.22], ey: [-0.55, -0.5] }, { duration: 2.5, ease: oo }, 0)
    .add(Rh, { x: [0.2, 0.13], y: [-0.5, -0.125], z: [-0.38, -0.42], pitch: [20, 30], yaw: [-10, -16], roll: [30, 14], curl: [0.55, 0.36], ex: [0.26, 0.22], ey: [-0.55, -0.5] }, { duration: 2.6, ease: oo }, 0.1)
    // The ring hand reaches forward, fingers spread, palm out (2.5 → 4).
    .add(Lh, { x: -0.075, y: -0.055, z: -0.6, pitch: 78, yaw: 14, roll: -6, curl: [0.32, 0.06], spread: [0.1, 1], thumb: [0.2, 0.9], tcurl: [0.4, 0.05], ex: -0.17, ey: -0.42, ez: -0.25 }, { duration: 1.6, ease: io }, 2.5)
    // The other hand comes up to draw (3.2 → 5).
    .add(Rh, { x: 0.1, y: -0.07, z: -0.52, pitch: 62, yaw: -22, roll: 8, curl: 0.18, spread: 0.35, thumb: 0.45, tcurl: 0.3, ex: 0.18, ey: -0.42, ez: -0.2 }, { duration: 1.8, ease: io }, 3.2)
    // Circles (5 → 7.4): two and a half turns, sparks following.
    .add(circle, { amp: [0, 1, 1, 0.4] }, { duration: 2.6, ease: 'inOutSine' }, 4.9)
    .add(circle, { phase: [0, Math.PI * 2 * 2.5] }, { duration: 2.6, ease: 'inOutSine' }, 4.9)
    .add(portal, { arc: [0, 1] }, { duration: 1.9, ease: 'inOutSine' }, 5.3)
    .add(portal, { glow: [0, 0.6, 1] }, { duration: 2.4, ease: 'linear' }, 5.2)
    .add(portal, { size: [0.18, 1] }, { duration: 2.3, ease: 'inOutCubic' }, 5.2)
    .add(portal, { open: [0, 1] }, { duration: 0.7, ease: 'inOutSine' }, 6.8)
    // Step through (7.5 → 10): the portal grows past the screen, the arms drop away.
    .add(portal, { grow: [0, 1] }, { duration: 2.5, ease: 'inCubic' }, 7.5)
    .add(Lh, { x: -0.2, y: -0.5, z: -0.45, pitch: 30, curl: 0.4, spread: 0.3, ey: -0.6 }, { duration: 1.6, ease: 'inCubic' }, 8.0)
    .add(Rh, { x: 0.2, y: -0.5, z: -0.45, pitch: 30, curl: 0.4, ey: -0.6 }, { duration: 1.5, ease: 'inCubic' }, 7.9);
  const TL = 10;
  // The circling hand's phase is spring-smoothed so fast scrolling never makes it jerk.
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
        if (ring && s === 0 && (f === 1 || f === 2)) {
          // The ring: a band on the middle and ring fingers' first segment.
          const k = ringN * 12, tt = 0.42;
          ringInst[k] = tmpA[0] + (tmpB[0] - tmpA[0]) * tt; ringInst[k + 1] = tmpA[1] + (tmpB[1] - tmpA[1]) * tt; ringInst[k + 2] = tmpA[2] + (tmpB[2] - tmpA[2]) * tt;
          ringInst[k + 3] = lerpR(fi.r[0], fi.r[1], tt) + 0.0012;
          ringInst[k + 4] = dirL[0]; ringInst[k + 5] = dirL[1]; ringInst[k + 6] = dirL[2]; ringInst[k + 7] = 0.0026;
          ringInst[k + 8] = dorL[0]; ringInst[k + 9] = dorL[1]; ringInst[k + 10] = dorL[2]; ringInst[k + 11] = 4;
          ringN++;
          if (f === 2) {
            // A bridge plate joining the two bands across the back of the fingers.
            const a0 = ringN - 2;
            for (let q = 0; q < 3; q++) RING[q] = ringInst[a0 * 12 + q] + ringInst[a0 * 12 + 8 + q] * (ringInst[a0 * 12 + 3] + 0.0012);
            for (let q = 0; q < 3; q++) RING[3 + q] = ringInst[(a0 + 1) * 12 + q] + ringInst[(a0 + 1) * 12 + 8 + q] * (ringInst[(a0 + 1) * 12 + 3] + 0.0012);
            tmpRingA[0] = RING[0]; tmpRingA[1] = RING[1]; tmpRingA[2] = RING[2];
            tmpRingB[0] = RING[3]; tmpRingB[1] = RING[4]; tmpRingB[2] = RING[5];
            push(tmpRingA, 0.0042, tmpRingB, 0.0042, dirL, 4);
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
  const tmpRingA = [0, 0, 0], tmpRingB = [0, 0, 0];
  const lerpR = (a, b, t) => a + (b - a) * t;

  // -------------------------------------------------------------------------
  // Frame
  // -------------------------------------------------------------------------
  const out = { portal: { x: 0, y: 0, r: 0 } };
  const proj = new Float32Array(16);
  let cssW = 1, cssH = 1, dprK = 1, fov = 0;
  const NSPARK = 4200;
  const sun = [-0.45, 0.55, 0.7];
  function resize(w, h, dpr) {
    cssW = w; cssH = h; dprK = Math.min(dpr || 1, 1.5);
    canvas.width = Math.max(2, Math.round(w * dprK)); canvas.height = Math.max(2, Math.round(h * dprK));
    // Vertical field of view: a natural first-person 62°, wider on portrait screens.
    const aspect = w / h;
    fov = 62 * Math.PI / 180 * (aspect < 1 ? 1.25 : 1);
    const f = 1 / Math.tan(fov / 2), near = 0.03, far = 20;
    proj.fill(0);
    proj[0] = f / aspect; proj[5] = f; proj[10] = (far + near) / (near - far); proj[11] = -1; proj[14] = (2 * far * near) / (near - far);
  }
  resize(canvas.clientWidth || innerWidth, canvas.clientHeight || innerHeight, devicePixelRatio);

  const PORTAL_Z = -1.6;
  const portalDepth = () => { const z = PORTAL_Z; return (proj[10] * z + proj[14]) / -z; };

  function frame(t, dt, g, look) {
    g = Math.min(1, Math.max(0, g || 0));
    // Spring-smooth the gesture a touch (only retarget when it actually moves).
    if (Math.abs(g - gTarget) > 0.0005) { gTarget = g; spring(gS, { v: g }, { stiffness: 90, damping: 19 }); }
    const gv = Math.min(1, Math.max(0, gS.v));
    if (Math.abs(gv - lastSeek) > 1e-5) { tl.seek(gv * TL); lastSeek = gv; }

    // Circling: the drawing hand's wrist runs round a small circle in the screen plane.
    const lx = look ? look.x || 0 : 0, ly = look ? look.y || 0 : 0;
    const sway = Math.sin(t * 0.9) * 0.004, bob = Math.sin(t * 1.4) * 0.003;
    const cx = Math.cos(circle.phase - 0.6) * 0.06 * circle.amp, cy = Math.sin(circle.phase - 0.6) * 0.05 * circle.amp;
    const rx = Rh.x, ry = Rh.y, lxp = Lh.x, lyp = Lh.y;
    Rh.x += cx - lx * 0.012 + sway; Rh.y += cy - ly * 0.008 + bob;
    Lh.x += -lx * 0.012 - sway * 0.7; Lh.y += -ly * 0.008 + bob * 0.8;
    n = 0; ringN = 0;
    if (Lh.y > -0.48 || Rh.y > -0.48) {
      buildHand(Lh, -1, true, t);
      buildHand(Rh, 1, false, t);
    }
    Rh.x = rx; Rh.y = ry; Lh.x = lxp; Lh.y = lyp;

    // Portal geometry in CSS px.
    const minS = Math.min(cssW, cssH);
    const R0 = Math.min(cssW * 0.4, cssH * 0.36);
    const cover = Math.hypot(cssW, cssH) * 0.62;
    const gr = portal.grow;
    const px = cssW * 0.5 + lx * minS * 0.02, py = lerpR(cssH * 0.44, cssH * 0.5, gr) + ly * minS * 0.015;
    const ringR = lerpR(R0 * portal.size, cover * 1.08, gr);
    const P = out.portal;
    P.x = px; P.y = py;
    P.r = portal.open <= 0.001 ? 0 : ringR * 0.985 * portal.open;

    // Draw.
    const Wd = canvas.width, Hd = canvas.height;
    gl.viewport(0, 0, Wd, Hd);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    const sparkOn = portal.arc > 0.001 && portal.glow > 0.001;
    if (n > 0) {
      gl.enable(gl.DEPTH_TEST); gl.depthMask(true); gl.disable(gl.BLEND);
      gl.enable(gl.CULL_FACE); gl.cullFace(gl.FRONT);
      gl.useProgram(MESH.p);
      gl.uniformMatrix4fv(MESH.u.uProj, false, proj);
      gl.uniform3f(MESH.u.uSun, sun[0], sun[1], sun[2]);
      // Portal light position in view space: unproject its centre to the portal plane.
      const tanH = Math.tan(fov / 2);
      const vx = ((px / cssW) * 2 - 1) * tanH * (cssW / cssH) * -PORTAL_Z, vy = (1 - (py / cssH) * 2) * tanH * -PORTAL_Z;
      gl.uniform4f(MESH.u.uPortal, vx, vy, PORTAL_Z, sparkOn ? portal.glow * (1 - gr * 0.5) : 0);
      gl.uniform1f(MESH.u.uTime, t);
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
      gl.uniform4f(GLOW.u.uRing, px * k, py * k, ringR * k, portal.arc);
      gl.uniform4f(GLOW.u.uP, t, 1, portal.glow * portal.arc * fade, portal.open);
      gl.uniform1f(GLOW.u.uDepth, portalDepth());
      gl.uniform1f(GLOW.u.uScale, k);
      gl.uniform1f(GLOW.u.uHole, P.r * k);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.useProgram(SPARK.p);
      gl.uniform2f(SPARK.u.uRes, Wd, Hd);
      gl.uniform4f(SPARK.u.uRing, px * k, py * k, ringR * k, portal.arc);
      gl.uniform4f(SPARK.u.uP, t, 1, portal.glow * fade, 0.4 * portal.glow);
      gl.uniform1f(SPARK.u.uDepth, portalDepth());
      gl.uniform1f(SPARK.u.uScale, k);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, NSPARK);
      gl.depthMask(true);
      gl.disable(gl.BLEND);
    }
    return out;
  }
  return { resize, frame };
}
