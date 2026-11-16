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
uniform vec3 uSun;        // direction to the sun: low, in front of you
uniform vec4 uPortal;     // portal centre (view space), glow strength
uniform float uTime;
out vec4 outColor;
// Backlit silhouettes: the sun is in front, so the hands are near-black shapes
// with a thin warm rim, red light through the thin edges and the portal's glow on them.
void main() {
  vec3 n = normalize(vN);
  vec3 V = normalize(-vPos);
  vec3 L = normalize(uSun);
  float ndv = max(dot(n, V), 0.0);
  float edge = 1.0 - ndv;
  float back = pow(max(dot(-V, L), 0.0), 1.5);
  vec3 Lp = normalize(uPortal.xyz - vPos);
  float pl = max(dot(n, Lp), 0.0);
  float m = vMat;
  bool skin = m < 0.5 || (m > 2.5 && m < 3.5) || m > 4.5;
  bool metal = m > 3.5 && m < 4.5;
  vec3 col = (skin ? vec3(0.03, 0.022, 0.022) : vec3(0.016, 0.017, 0.021)) * (0.55 + 0.45 * max(n.y, 0.0));
  // A thin rim where the low sun grazes the edge.
  float rim = pow(edge, 5.0) * smoothstep(-0.4, 0.5, dot(n, L) + 0.35);
  col += vec3(1.0, 0.66, 0.36) * rim * (0.5 + 0.9 * back) * (skin ? 1.0 : 0.6);
  // Light through the skin at the silhouette.
  if (skin) col += vec3(0.85, 0.16, 0.05) * pow(edge, 2.2) * back * 0.3;
  // The portal's fire on the edges that face it.
  col += uPortal.w * vec3(1.0, 0.42, 0.1) * (pow(edge, 2.0) * pl * 1.3 + pl * 0.06);
  if (metal) {
    vec3 r = reflect(-V, n);
    col = vec3(0.05, 0.04, 0.03) + vec3(1.0, 0.8, 0.5) * 3.0 * pow(max(dot(r, L), 0.0), 40.0) + uPortal.w * vec3(1.0, 0.5, 0.15) * 1.5 * pow(max(dot(r, Lp), 0.0), 6.0);
  }
  col = col / (1.0 + col);
  col = pow(col, vec3(1.0 / 2.2));
  outColor = vec4(col, 1.0);
}`;

// Sparks: one tiny instanced streak per spark, in screen pixels around the portal.
// A thin band of countless bright sparks spinning fast like a grinder, a ragged crackling edge,
// and sparks shed tangentially (mostly off the lower half) that fall and bounce on the snow.
const sparkVert = `#version 300 es
precision highp float;
uniform vec2 uRes;          // device px
uniform vec4 uRing;         // centre px (y down), radius px, arc fraction 0..1
uniform vec4 uP;            // time, accumulated spin angle, intensity, ember fraction
uniform vec2 uA;            // start angle of the drawn arc, closed (0..1)
uniform float uFloor;       // snow line, device px from the top
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
  float h1 = h(f + 0.13), h2 = h(f * 1.7 + 3.1), h3 = h(f * 2.3 + 7.7), h4 = h(f * 0.37 + 1.9), h5 = h(f * 3.1 + 0.7);
  float t = uP.x, R = uRing.z, arc = uRing.w, closed = uA.y;
  vec2 C = uRing.xy;
  float TAU = 6.2831853;
  bool ember = h4 < uP.w;
  // Spin is clockwise on screen (decreasing angle); the band crackles: radius re-rolls ~25 times a second.
  float tick = floor(t * 25.0 + h5 * 25.0);
  float band = 3.0 * uScale + R * 0.055;
  vec2 pos, dir; float len, wid, bright;
  vec3 col;
  if (!ember) {
    float speed = (0.85 + 0.3 * h2);
    // Sparks race round the whole circle; only those inside the part drawn so far are shown.
    float a = h1 * TAU - uP.y * speed;
    float relA = mod(a - uA.x, TAU) / TAU;
    float inArc = arc >= 0.999 ? 1.0 : step(relA, arc);
    float jr = (h(f + tick) - 0.5) * 2.0;                     // -1..1, re-rolled: a ragged edge
    float r = R + jr * sqrt(abs(jr)) * band * (1.0 + 1.2 * step(0.94, h3));
    pos = C + vec2(cos(a), -sin(a)) * r;
    dir = vec2(sin(a), cos(a));                               // direction of travel (clockwise)
    float hot = 1.0 - min(1.0, abs(jr) * 1.4);
    len = (4.0 + 22.0 * h2 * h2) * uScale * (0.4 + 0.6 * closed) * (0.5 + 0.5 * min(1.0, R / (200.0 * uScale)));
    wid = (1.0 + 1.0 * h3) * uScale;
    float flick = 0.35 + 0.65 * h(f * 0.7 + tick);
    bright = (0.8 + 1.4 * h1 * h1) * flick * (0.5 + 0.5 * hot) * (1.0 + 0.8 * min(1.0, R / (500.0 * uScale)));
    col = mix(vec3(1.0, 0.3, 0.06), mix(vec3(1.0, 0.6, 0.18), vec3(1.0, 0.92, 0.7), hot * hot), 0.35 + 0.65 * hot);
    // While the arc is being drawn its leading end flares.
    // The growing tip, at the hand's phase, flares and crackles.
    if (arc < 0.999) bright *= 0.75 + 1.8 * (1.0 - smoothstep(0.0, 0.05, relA)) * (0.5 + h(f + tick * 1.3));
    bright *= inArc; wid *= inArc;
  } else {
    // Shed sparks: thrown off along the spin, mostly from the lower half, falling and bouncing on the snow.
    float rate = 0.6 + 0.6 * h2;
    float life = fract(t * rate + h1 * 7.0);
    float born = t - life / rate;
    float side = h3 < 0.75 ? -0.25 * TAU + (h5 - 0.5) * 0.55 * TAU : h5 * TAU;  // lower half preferred
    float a0 = arc < 0.999 ? uA.x + h5 * arc * TAU : side;
    vec2 p0 = C + vec2(cos(a0), -sin(a0)) * (R + (h1 - 0.5) * band);
    vec2 tang = vec2(sin(a0), cos(a0));
    vec2 out0 = vec2(cos(a0), -sin(a0));
    float v = (0.5 + 0.9 * h2) * min(R, 260.0 * uScale) + 70.0 * uScale;
    vec2 vel = (tang * (0.9 + 0.3 * h1) + out0 * (h1 - 0.3) * 0.35) * v * (0.35 + 0.65 * closed);
    float T = life * 1.3;
    float drag = (1.0 - exp(-T * 1.2)) / 1.2;
    vec2 g = vec2(0.0, 900.0 * uScale);
    pos = p0 + vel * drag + g * T * T * 0.5;
    vec2 vnow = vel * exp(-T * 1.2) + g * T;
    if (pos.y > uFloor) { pos.y = uFloor - (pos.y - uFloor) * 0.25; vnow.y = -vnow.y * 0.25; }
    dir = normalize(vnow + 1e-4);
    len = clamp(length(vnow) * 0.022, 2.0 * uScale, 26.0 * uScale);
    wid = (0.8 + 0.7 * h3) * uScale;
    bright = (1.0 - life) * (1.0 - life) * (0.9 + 0.9 * h2);
    col = mix(vec3(1.0, 0.22, 0.03), vec3(1.0, 0.7, 0.3), (1.0 - life) * (1.0 - life));
  }
  vec2 side2 = vec2(-dir.y, dir.x);
  vec2 q = pos - dir * len * corner.x + side2 * wid * (corner.y - 0.5) * 2.0;
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
  float along = 1.0 - vUv.x;
  float a = across * along * along * vI;
  outColor = vec4(vC * a, min(1.0, a * 0.8));
}`;

// Glow: a tight glow hugging the band, drawn on a ring-shaped strip only (no fill inside or out).
const glowVert = `#version 300 es
precision highp float;
uniform vec2 uRes;
uniform vec4 uRing;
uniform float uDepth;
uniform float uScale;
out vec2 vPx;
const float SEG = 160.0;
void main() {
  float seg = float(gl_VertexID >> 1), outer = float(gl_VertexID & 1);
  float a = seg / SEG * 6.2831853;
  float w = 16.0 * uScale + uRing.z * 0.09;
  float r = max(0.0, uRing.z + (outer * 2.0 - 1.0) * w);
  vec2 q = uRing.xy + vec2(cos(a), -sin(a)) * r;
  vPx = q;
  gl_Position = vec4(q.x / uRes.x * 2.0 - 1.0, 1.0 - q.y / uRes.y * 2.0, uDepth, 1.0);
}`;
const glowFrag = `#version 300 es
precision highp float;
uniform vec4 uRing;
uniform vec4 uP;        // time, spin, intensity, open
uniform vec2 uA;        // start angle, closed
uniform float uScale;
in vec2 vPx;
out vec4 outColor;
void main() {
  vec2 d = vPx - uRing.xy;
  float r = length(d), R = max(uRing.z, 1.0);
  float x = (r - R) / uScale;
  float ang = atan(-d.y, d.x);
  float TAU = 6.2831853;
  float rel = mod(ang - uA.x, TAU) / TAU;
  float arc = uRing.w;
  float drawn = arc >= 0.999 ? 1.0 : 1.0 - smoothstep(arc - 0.01, arc + 0.003, rel);
  float t = uP.x;
  float flick = 0.75 + 0.25 * sin(ang * 23.0 - t * 31.0) * sin(ang * 9.0 + t * 17.0);
  float w = 3.0 + R / uScale * 0.03;
  float core = exp(-x * x / (w * w));
  float halo = exp(-abs(x) / (5.0 + R / uScale * 0.02));
  vec3 c = (vec3(1.0, 0.55, 0.16) * core * 0.35 + vec3(1.0, 0.38, 0.06) * halo * 0.16) * flick * drawn * uP.z;
  outColor = vec4(c, clamp(core * 0.25 * drawn * uP.z, 0.0, 0.6));
}`;

// Light: the portal lights the scene — a warm wash in the air, a pool of glow on the snow, and a flash as it opens.
const lightFrag = `#version 300 es
precision highp float;
uniform vec2 uRes;
uniform vec4 uRing;     // centre px, radius px, intensity
uniform vec2 uL;        // flash, snow line (device px from the top)
out vec4 outColor;
void main() {
  vec2 px = vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y);
  vec2 d = px - uRing.xy;
  float R = max(uRing.z, 1.0);
  float r = length(d);
  float air = exp(-max(r - R, 0.0) / (R * 0.6)) * step(R, r) * 0.07;
  vec2 g = vec2(d.x / (R * 2.2), (px.y - uL.y) / (R * 0.6));
  float snow = exp(-dot(g, g)) * smoothstep(uL.y - R * 0.45, uL.y + R * 0.05, px.y) * 0.4;
  float flash = uL.x * (0.6 * exp(-r / R) + 0.1);
  vec3 c = vec3(1.0, 0.45, 0.12) * (air + snow) * uRing.w + vec3(1.0, 0.8, 0.55) * flash;
  // Fade to nothing well inside the scissor box main draws this in, so its edges never show.
  c *= (1.0 - smoothstep(3.0 * R, 4.3 * R, abs(d.x))) * smoothstep(uRing.y - 3.1 * R, uRing.y - 2.3 * R, px.y);
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
  { u: 0.031, w: 0.092, len: [0.043, 0.025, 0.021], r: [0.0118, 0.0100, 0.0084, 0.0066], spread: 0.16 },
  { u: 0.010, w: 0.097, len: [0.047, 0.029, 0.022], r: [0.0122, 0.0103, 0.0087, 0.0069], spread: 0.03 },
  { u: -0.010, w: 0.093, len: [0.044, 0.028, 0.022], r: [0.0116, 0.0098, 0.0083, 0.0065], spread: -0.11 },
  { u: -0.029, w: 0.083, len: [0.034, 0.021, 0.019], r: [0.0102, 0.0086, 0.0074, 0.0059], spread: -0.25 },
];

// A pose: wrist position (view space), hand orientation, finger state.
const makePose = (side) => ({ x: side * 0.2, y: -0.5, z: -0.38, yaw: 0, pitch: 20, roll: 0, curl: 0.55, spread: 0.1, thumb: 0.2, tcurl: 0.4, ex: side * 0.26, ey: -0.55, ez: -0.12 });

// This module can draw into a WebGL2 context shared with the rest of the page (see CONTRACT.md).
export const SHARED_GL = true;

export function createHands(target) {
  // A shared context: draw over what is already there. A canvas: our own transparent overlay.
  const shared = typeof WebGL2RenderingContext !== 'undefined' && target instanceof WebGL2RenderingContext;
  const canvas = shared ? target.canvas : target;
  const gl = shared ? target : canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: (globalThis.devicePixelRatio || 1) < 2, depth: true, powerPreference: 'high-performance' });
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
  const LIGHT = compile(`#version 300 es
void main() { vec2 c = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)) * 4.0 - 1.0; gl_Position = vec4(c, 0.0, 1.0); }`, lightFrag);

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
  // The formation plays on its own clock, like a film: once the scroll triggers it, about 3.5 s of
  // hands rising, a fingertip tracing loops, a small ring that ignites, expands and opens onto the hall.
  // Scrolling back reverses it. Afterwards the scroll carries you through (a second, scrubbed timeline).
  // -------------------------------------------------------------------------
  const Lh = makePose(-1), Rh = makePose(1);
  Lh.x = -0.36; Lh.y = -0.62; Rh.x = 0.36; Rh.y = -0.62;
  const portal = { size: 0, glow: 0, grow: 0, drop: 0 };
  const io = 'inOutCubic', oo = 'outCubic';
  const FORM = 4.3;            // seconds
  const tlForm = timeline({ autoplay: false })
    .add(Lh, { x: [-0.36, -0.31], y: [-0.62, -0.31], z: [-0.48, -0.52], pitch: [30, 80], yaw: [25, 24], roll: [-20, -6], curl: [0.5, 0.12], spread: [0.1, 0.75], thumb: [0.2, 0.75], tcurl: [0.4, 0.12], ex: -0.42, ey: -0.68, ez: -0.3 }, { duration: 0.9, ease: oo }, 0)
    .add(Rh, { x: [0.36, 0.22], y: [-0.62, -0.3], z: [-0.48, -0.5], pitch: [30, 62], yaw: [-25, -14], roll: [20, 8], curl: [0.5, 0.2], spread: 0.35, thumb: 0.45, tcurl: 0.3, ex: 0.4, ey: -0.68, ez: -0.3 }, { duration: 0.85, ease: oo }, 0.05)
    .add(portal, { glow: [0, 1] }, { duration: 0.35, ease: 'outSine' }, 0.7)
    .add(Rh, { x: 0.27, y: -0.34, curl: 0.3 }, { duration: 0.7, ease: io }, 3.6);
  const tlStep = timeline({ autoplay: false })
    .add(portal, { grow: [0, 1] }, { duration: 1, ease: 'inCubic' }, 0)
    .add(portal, { drop: [0, 1] }, { duration: 0.6, ease: 'inCubic' }, 0.05);
  const TRIGGER = 0.06;        // g at which the formation starts
  const ARC_FROM = 0.6, ARC_LEN = 3.0;   // three calm circles of the hand draw the ring, a third each
  // Time-driven state (allocation-free).
  const draw$ = { f: 0, lastF: -1, phase: 0, vel: 0, amp: 0, flash: 0, step: 0, stepV: 0, lastStep: -1 };

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
    // Forearm sleeve, tapering from the elbow to the wrist.
    at(0.5, tmpA); at(0.1, tmpB); push(tmpA, 0.058, tmpB, 0.037, D, 1);
    at(0.105, tmpA); at(0.05, tmpB); push(tmpA, 0.037, tmpB, 0.03, D, 2);           // knit cuff, gathered at the wrist
    at(0.075, tmpA); toView(tmpB, 0, -0.002, 0.012); push(tmpA, 0.03, tmpB, 0.026, D, 0); // wrist
    // Palm: metacarpals side by side fuse into the back of the hand; a pad underneath.
    for (let f = 0; f < 4; f++) {
      const fi = FINGERS[f];
      toView(tmpA, fi.u * 0.55, 0.0, 0.016);
      toView(tmpB, fi.u, 0.0015, fi.w - 0.004);
      push(tmpA, 0.0155, tmpB, fi.r[0] * 1.1, D, 5);
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
  const out = { portal: { x: 0, y: 0, r: 0, power: 0, ring: 0, formed: 0, step: 0 } };
  const proj = new Float32Array(16);
  let cssW = 1, cssH = 1, dprK = 1, fov = 0, tanH = 0.6, aspect = 1;
  const NSPARK = 9000;
  const sun = [0.18, 0.2, -1];
  const off = (globalThis.__summitOff) || {};
  function resize(w, h, dpr) {
    cssW = w; cssH = h; dprK = Math.min(dpr || 1, 2);
    if (!shared) { canvas.width = Math.max(2, Math.round(w * dprK)); canvas.height = Math.max(2, Math.round(h * dprK)); }
    aspect = w / h;
    fov = 62 * Math.PI / 180 * (aspect < 1 ? 1.25 : 1);
    tanH = Math.tan(fov / 2);
    const f = 1 / tanH, near = 0.03, far = 20;
    proj.fill(0);
    proj[0] = f / aspect; proj[5] = f; proj[10] = (far + near) / (near - far); proj[11] = -1; proj[14] = (2 * far * near) / (near - far);
  }
  resize(canvas.clientWidth || innerWidth, canvas.clientHeight || innerHeight, devicePixelRatio);

  const PORTAL_Z = -1.6;
  const portalDepth = () => { const z = PORTAL_Z; return (proj[10] * z + proj[14]) / -z; };
  const scr = [0, 0, 0];
  function project(x, y, z) {
    const k = 1 / (-z * tanH);
    scr[0] = (x * k / aspect * 0.5 + 0.5) * cssW;
    scr[1] = (0.5 - y * k * 0.5) * cssH;
    scr[2] = k * 0.5 * cssH;
  }
  const LOOP_R = 0.06;         // metres, the drawing hand's loop
  let spinAcc = 0, drewLast = true, timeNow = 0;
  const sm = (a, b, x) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };

  function draw(sparkOn, cx, cy, ringR, arc, a0, closed, gr, power) {
    const Wd = canvas.width, Hd = canvas.height, k = dprK;
    gl.viewport(0, 0, Wd, Hd);
    if (shared) gl.clear(gl.DEPTH_BUFFER_BIT);
    else { gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT); }
    if (n > 0 && !off.hands) {
      gl.enable(gl.DEPTH_TEST); gl.depthMask(true); gl.disable(gl.BLEND);
      gl.enable(gl.CULL_FACE); gl.cullFace(gl.FRONT);
      gl.useProgram(MESH.p);
      gl.uniformMatrix4fv(MESH.u.uProj, false, proj);
      gl.uniform3f(MESH.u.uSun, sun[0], sun[1], sun[2]);
      const vx = ((cx / cssW) * 2 - 1) * tanH * aspect * -PORTAL_Z, vy = (1 - (cy / cssH) * 2) * tanH * -PORTAL_Z;
      gl.uniform4f(MESH.u.uPortal, vx, vy, PORTAL_Z, power * 1.3);
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
    if (sparkOn && !off.sparks) {
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.bindVertexArray(emptyVao);
      // Light cast on the snow, limited to a box round the portal and below it.
      gl.disable(gl.DEPTH_TEST); gl.depthMask(false);
      const LR = Math.max(ringR, cssH * 0.1);
      const bx = Math.max(0, Math.floor((cx - LR * 4.5) * k)), bw = Math.min(Wd, Math.ceil((cx + LR * 4.5) * k)) - bx;
      const by = Math.max(0, Math.floor((cy - LR * 3.2) * k));
      if (bw > 0 && by < Hd) {
        gl.enable(gl.SCISSOR_TEST);
        gl.scissor(bx, 0, bw, Hd - by);
        gl.useProgram(LIGHT.p);
        gl.uniform2f(LIGHT.u.uRes, Wd, Hd);
        gl.uniform4f(LIGHT.u.uRing, cx * k, cy * k, LR * k, power * (1 - gr));
        gl.uniform2f(LIGHT.u.uL, draw$.flash * 0.35 * (1 - gr), cssH * 0.86 * k);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
        gl.disable(gl.SCISSOR_TEST);
      }
      gl.enable(gl.DEPTH_TEST);
      const fade = 1 - gr * gr;
      gl.useProgram(GLOW.p);
      gl.uniform2f(GLOW.u.uRes, Wd, Hd);
      gl.uniform4f(GLOW.u.uRing, cx * k, cy * k, ringR * k, arc);
      gl.uniform4f(GLOW.u.uP, timeNow, spinAcc, power * fade, 1);
      gl.uniform2f(GLOW.u.uA, a0, closed);
      gl.uniform1f(GLOW.u.uDepth, portalDepth());
      gl.uniform1f(GLOW.u.uScale, k);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 322);
      // Ignition sends one shockwave ring outwards.
      const sh = 1 - draw$.flash;
      if (draw$.flash > 0.02) {
        gl.uniform4f(GLOW.u.uRing, cx * k, cy * k, ringR * (1 + 1.4 * sh) * k, 1);
        gl.uniform4f(GLOW.u.uP, timeNow, spinAcc, 1.4 * draw$.flash * draw$.flash * fade, 0);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 322);
      }
      gl.useProgram(SPARK.p);
      gl.uniform2f(SPARK.u.uRes, Wd, Hd);
      gl.uniform4f(SPARK.u.uRing, cx * k, cy * k, ringR * k, arc);
      gl.uniform4f(SPARK.u.uP, timeNow, spinAcc, power * fade, 0.14 + 0.16 * closed);
      gl.uniform2f(SPARK.u.uA, a0, closed);
      gl.uniform1f(SPARK.u.uFloor, cssH * 0.88 * k);
      gl.uniform1f(SPARK.u.uDepth, portalDepth());
      gl.uniform1f(SPARK.u.uScale, k);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, sparkCount);
      gl.depthMask(true);
      gl.disable(gl.BLEND);
    }
    // Leave the shared state as we found it.
    gl.disable(gl.DEPTH_TEST); gl.disable(gl.CULL_FACE); gl.disable(gl.SCISSOR_TEST); gl.disable(gl.BLEND);
    gl.depthMask(true);
    gl.bindVertexArray(null);
  }
  let sparkCount = NSPARK;
  // Quality governor hook: 0 full, 1 fewer embers' glow, 2 fewer sparks.
  function setQuality(level) { sparkCount = level >= 2 ? 5000 : level >= 1 ? 7000 : NSPARK; }

  const sparkOn0 = (D) => D.f * FORM > ARC_FROM && portal.glow > 0.001;
  function frame(t, dt, g, look) {
    timeNow = t;
    dt = Math.min(0.05, Math.max(0, dt || 0));
    g = Math.min(1, Math.max(0, g || 0));
    const D = draw$;
    // Formation clock: forwards at real speed once triggered, back a little faster when you scroll up.
    const trig = g > TRIGGER;
    D.f = trig ? Math.min(1, D.f + dt / FORM) : Math.max(0, D.f - dt / (FORM * 0.6));
    const F = D.f * FORM;
    if (D.f !== D.lastF) { tlForm.seek(F); D.lastF = D.f; }
    // Step through: only once formed; a critically damped spring follows the scroll.
    const stepT = D.f >= 1 ? Math.min(1, Math.max(0, (g - 0.75) / 0.25)) : 0;
    const K = 60, C = 2 * Math.sqrt(K);
    D.stepV += (K * (stepT - D.step) - C * D.stepV) * dt;
    D.step = Math.min(1, Math.max(0, D.step + D.stepV * dt));
    if (Math.abs(D.step - D.lastStep) > 1e-5) { tlStep.seek(D.step); D.lastStep = D.step; }

    // Ring: drawn by the fingertip, closes, ignites, expands.
    // The hand's phase and the arc are one thing: three eased loops, the ring growing a third per loop.
    const u = Math.min(1, Math.max(0, (F - ARC_FROM) / ARC_LEN));
    const ue = u * u * (3 - 2 * u) * 0.35 + u * 0.65;
    const arc = ue;
    D.phase = ue * Math.PI * 2 * 3;
    const closedNow = F >= ARC_FROM + ARC_LEN;
    if (closedNow && !D.wasClosed && trig) D.flash = 1;
    D.wasClosed = closedNow;
    D.flash *= Math.exp(-dt * 3.2);
    const closed = sm(ARC_FROM + ARC_LEN - 0.05, ARC_FROM + ARC_LEN + 0.25, F);
    // The hand circles at about a loop a second while it draws and holds the ring.
    D.amp = sm(ARC_FROM - 0.3, ARC_FROM + 0.25, F) * (1 - 0.7 * sm(ARC_FROM + ARC_LEN, ARC_FROM + ARC_LEN + 0.6, F));
    spinAcc += dt * (3.0 + 5.0 * closed);
    if (spinAcc > 1e4) spinAcc -= Math.PI * 2 * 1000;
    if (spinAcc > 1e4) spinAcc -= Math.PI * 2 * 1000;

    const lx = look ? look.x || 0 : 0, ly = look ? look.y || 0 : 0;
    const sway = Math.sin(t * 0.9) * 0.003, bob = Math.sin(t * 1.4) * 0.002;
    const ph = -D.phase + Math.PI / 2;
    const sizeEff = 0;
    const loopR = LOOP_R * (1 + 0.6 * sizeEff) * D.amp;
    const ca = Math.cos(ph), sa = Math.sin(ph);
    const xs = Math.min(1, Math.max(0.42, aspect / 1.6));
    const drop = portal.drop * 0.36;
    const rx = Rh.x, ry = Rh.y, lxp = Lh.x, lyp = Lh.y, rex = Rh.ex, lex = Lh.ex;
    Rh.x = Rh.x * xs + ca * loopR - lx * 0.01 + sway; Rh.y += sa * loopR - ly * 0.008 + bob - drop; Rh.ex *= xs;
    Lh.x = Lh.x * xs - lx * 0.01 - sway * 0.7; Lh.y += -ly * 0.008 + bob * 0.8 - drop; Lh.ex *= xs;
    n = 0; ringN = 0;
    if (Lh.y > -0.58 || Rh.y > -0.58) {
      buildHand(Lh, -1, true, t);
      buildHand(Rh, 1, false, t);
    }
    Rh.x = rx; Rh.y = ry; Lh.x = lxp; Lh.y = lyp; Rh.ex = rex; Lh.ex = lex;

    // Small ring on the fingertip's loop, growing out to the full portal in the middle.
    const minS = Math.min(cssW, cssH);
    const R0 = Math.min(cssW * 0.34, cssH * 0.4);
    const cover = Math.hypot(cssW, cssH) * 0.62;
    const gr = portal.grow;
    const fx = cssW * 0.5 + lx * minS * 0.02, fy = lerpR(cssH * 0.42, cssH * 0.5, gr) + ly * minS * 0.015;
    const cx = fx, cy = fy;
    const ringR = lerpR(R0, cover * 1.08, gr);
    const power = portal.glow * Math.min(1, 0.3 + arc) * (1 + D.flash * 0.7);
    const P = out.portal;
    P.x = cx; P.y = cy;
    // Only once the ring is complete does the inside tear open onto the hall, centre to rim in ~0.4 s.
    const tear = sm(ARC_FROM + ARC_LEN + 0.05, ARC_FROM + ARC_LEN + 0.45, F);
    P.r = tear < 0.005 ? 0 : Math.max(0, ringR * 0.955 - 2) * tear;
    P.power = Math.min(1, power);
    P.ring = sparkOn0(D) ? ringR : 0;
    P.formed = D.f;
    P.step = D.step;

    const sparkOn = sparkOn0(D) && arc > 0.001;
    if (n === 0 && !sparkOn) {
      if (drewLast && !shared) { gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT); }
      drewLast = false; pend.on = false;
      return out;
    }
    drewLast = true;
    pend.on = true; pend.sparkOn = sparkOn; pend.cx = cx; pend.cy = cy; pend.ringR = ringR; pend.arc = arc; pend.ph = Math.PI / 2 - arc * Math.PI * 2; pend.closed = closed; pend.gr = gr; pend.power = power;
    if (!deferred) render();
    return out;
  }
  // In shared-context mode main.js calls update() first (it needs the portal for the mountain and the hall)
  // and render() last, over everything else.
  const pend = { on: false, sparkOn: false, cx: 0, cy: 0, ringR: 0, arc: 0, ph: 0, closed: 0, gr: 0, power: 0 };
  let deferred = false;
  function render() {
    if (!pend.on) return;
    pend.on = false;
    draw(pend.sparkOn, pend.cx, pend.cy, pend.ringR, pend.arc, pend.ph, pend.closed, pend.gr, pend.power);
  }
  function update(t, dt, g, look) { deferred = true; const o = frame(t, dt, g, look); deferred = false; return o; }

  // Pre-warm: run every program once at load so the portal never stalls when it first opens.
  {
    const saveN = n; n = 0;
    buildHand(Lh, -1, true, 0); buildHand(Rh, 1, false, 0);
    draw(true, 0, 0, 1, 0.5, 0, 0, 0, 0);
    gl.flush();
    n = saveN; ringN = 0;
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  }
  return { resize, frame, update, render, setQuality };
}

// The spark ring on its own, for other portals (the hall's): the same shaders and look.
// draw(cx, cy, r, t, power) in device px of the bound framebuffer (top-left origin); additive over what is there.
// Leaves the default GL state (no blend, no depth test, no VAO).
export function createSparkRing(gl, count = 6000) {
  const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
  const prog = (vs, fs) => {
    const p = gl.createProgram();
    gl.attachShader(p, sh(gl.VERTEX_SHADER, vs)); gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
    const u = {};
    for (const n of ['uRes', 'uRing', 'uP', 'uA', 'uFloor', 'uDepth', 'uScale']) u[n] = gl.getUniformLocation(p, n);
    return { p, u };
  };
  const S = prog(sparkVert, sparkFrag), G = prog(glowVert, glowFrag);
  const vao = gl.createVertexArray();
  let last = -1, spin = 0;
  function draw(cx, cy, r, t, power = 1, scale = 1) {
    const dt = last < 0 ? 0 : Math.min(0.05, Math.max(0, t - last));
    last = t; spin += dt * 8;
    if (spin > 1e4) spin -= Math.PI * 2 * 1000;
    const W = gl.drawingBufferWidth, H = gl.drawingBufferHeight;
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.bindVertexArray(vao);
    for (const P of [G, S]) {
      gl.useProgram(P.p);
      gl.uniform2f(P.u.uRes, W, H);
      gl.uniform4f(P.u.uRing, cx, cy, r, 1);
      gl.uniform4f(P.u.uP, t, spin, power, P === S ? 0.3 : 1);
      gl.uniform2f(P.u.uA, Math.PI / 2, 1);
      gl.uniform1f(P.u.uFloor, H * 4);
      gl.uniform1f(P.u.uDepth, 0);
      gl.uniform1f(P.u.uScale, scale);
      if (P === G) gl.drawArrays(gl.TRIANGLE_STRIP, 0, 322);
      else gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, count);
    }
    gl.disable(gl.BLEND);
    gl.bindVertexArray(null);
  }
  return { draw };
}
