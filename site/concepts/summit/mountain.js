// The summit: first-person view from a snowy peak at golden hour.
// Sky, distant ranges, a sea of clouds, the snowy rock under your feet, spindrift,
// and "anikit" written in the sky in cloud puffs. All geometry is built once at load;
// per frame we only set uniforms and issue a handful of (mostly instanced) draws.
// Anikit drives the motion: a looping timeline for the wind that drifts the word,
// and springs that smooth head-turn and the forward dolly.
import { timeline, spring } from '../../../packages/anikit/dist/anikit.lean.js';

// ---------------------------------------------------------------------------
// Small deterministic noise for building geometry (JS, load time only)
// ---------------------------------------------------------------------------
function hash2(x, y, s) {
  let h = (x * 374761393 + y * 668265263 + s * 2147483647) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}
function vnoise(x, y, s = 0) {
  const xi = Math.floor(x), yi = Math.floor(y);
  const xf = x - xi, yf = y - yi;
  const u = xf * xf * (3 - 2 * xf), v = yf * yf * (3 - 2 * yf);
  const a = hash2(xi, yi, s), b = hash2(xi + 1, yi, s), c = hash2(xi, yi + 1, s), d = hash2(xi + 1, yi + 1, s);
  return (a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v) * 2 - 1;
}
function fbm(x, y, oct, s = 0) {
  let sum = 0, amp = 0.5, f = 1;
  for (let i = 0; i < oct; i++) { sum += amp * vnoise(x * f, y * f, s + i * 17); f *= 2.03; amp *= 0.5; }
  return sum;
}
function ridged(x, y, oct, s = 0) {
  let sum = 0, amp = 0.55, f = 1;
  for (let i = 0; i < oct; i++) { const n = 1 - Math.abs(vnoise(x * f, y * f, s + i * 31)); sum += amp * n * n; f *= 2.1; amp *= 0.5; }
  return sum;
}
let seed = 1234567;
const rnd = () => { seed = (Math.imul(seed, 1664525) + 1013904223) | 0; return (seed >>> 0) / 4294967296; };

// ---------------------------------------------------------------------------
// Shared GLSL: sky model, haze, tone mapping
// ---------------------------------------------------------------------------
const COMMON = `
uniform vec3 uSun;        // direction to the sun (world)
uniform float uWarm;      // 0..1 golden-ness
uniform float uExposure;
vec3 skyCol(vec3 d) {
  float e = d.y;
  float s = max(dot(d, uSun), 0.0);
  float az = max(dot(normalize(vec3(d.x, 0.0, d.z) + 1e-5), normalize(vec3(uSun.x, 0.0, uSun.z) + 1e-5)), 0.0);
  vec3 zen = vec3(0.06, 0.20, 0.52);
  vec3 mid = vec3(0.22, 0.45, 0.82);
  vec3 hzSun = mix(vec3(1.0, 0.78, 0.52), vec3(1.1, 0.68, 0.38), uWarm);
  vec3 hzAway = vec3(0.62, 0.70, 0.86);
  vec3 hz = mix(hzAway, hzSun, pow(az, 3.0));
  float eu = max(e, 0.0);
  vec3 c = mix(hz, mid, smoothstep(0.0, 0.16, eu));
  c = mix(c, zen, smoothstep(0.12, 0.7, eu));
  // below the horizon: warm haze over the cloud sea
  c = mix(c, hz * 0.95, smoothstep(0.0, -0.08, e));
  // sun glow (Mie-like): broad warm halo + tighter core
  c += vec3(1.0, 0.70, 0.40) * (0.35 * pow(s, 6.0) + 0.55 * pow(s, 48.0));
  return c;
}
vec3 haze(vec3 col, vec3 dir, float dist, float k) {
  float f = 1.0 - exp(-dist * k);
  vec3 h = skyCol(vec3(dir.x, max(dir.y, 0.0) + 0.015, dir.z));
  h = mix(vec3(0.50, 0.62, 0.84), h, 0.55 + 0.45 * pow(max(dot(dir, uSun), 0.0), 4.0));
  return mix(col, h, f);
}
vec3 tonemap(vec3 c) {
  c *= uExposure;
  c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14);
  return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2));
}
float hash12(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
float vnoise(vec2 p) {
  vec2 i = floor(p), f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash12(i), hash12(i + vec2(1, 0)), u.x), mix(hash12(i + vec2(0, 1)), hash12(i + vec2(1, 1)), u.x), u.y);
}
// value noise with analytic derivatives: (value, d/dx, d/dy)
vec3 vnoised(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f), du = 6.0 * f * (1.0 - f);
  float a = hash12(i), b = hash12(i + vec2(1, 0)), c = hash12(i + vec2(0, 1)), d = hash12(i + vec2(1, 1));
  return vec3(a + (b - a) * u.x + (c - a) * u.y + (a - b - c + d) * u.x * u.y,
              du * (vec2(b - a, c - a) + (a - b - c + d) * u.yx));
}
`;

const HEAD = `#version 300 es
precision highp float;
`;

// --- sky (full-screen triangle at the far plane) ---
const skyVS = HEAD + `
layout(location = 0) in vec2 aP;
uniform vec3 uRight, uUp, uFwd;
uniform vec2 uTan;
out vec3 vDir;
void main() {
  vDir = uFwd + aP.x * uTan.x * uRight + aP.y * uTan.y * uUp;
  gl_Position = vec4(aP, 1.0, 1.0);
}`;
const skyFS = HEAD + COMMON + `
in vec3 vDir;
uniform float uT;
out vec4 o;
void main() {
  vec3 d = normalize(vDir);
  vec3 c = skyCol(d);
  // sun disc
  float s = dot(d, uSun);
  c += vec3(1.6, 1.25, 0.9) * smoothstep(0.99955, 0.99975, s) * 3.0;
  // high cirrus: stretched noise on a plane
  if (d.y > 0.02) {
    vec2 uv = d.xz / (d.y + 0.08) * 1.2 + vec2(uT * 0.004, 0.0);
    vec2 q = uv * vec2(1.0, 4.5);
    float n = vnoise(q * 1.3) * 0.55 + vnoise(q * 2.9 + 3.1) * 0.28 + vnoise(q * 6.1 + 7.7) * 0.17;
    float band = vnoise(uv * 0.35 + 11.0);
    float cir = smoothstep(0.52, 0.85, n) * smoothstep(0.35, 0.75, band) * smoothstep(0.02, 0.2, d.y) * (1.0 - smoothstep(0.55, 0.9, d.y));
    vec3 cc = mix(vec3(1.0, 0.96, 0.95), vec3(1.25, 0.9, 0.62), pow(max(s, 0.0), 4.0) * uWarm + 0.25 * uWarm);
    c = mix(c, cc, cir * 0.55);
  }
  o = vec4(tonemap(c), 1.0);
}`;

// --- distant ranges: vertical strips with ridged tops ---
const rangeVS = HEAD + `
layout(location = 0) in vec3 aP;
layout(location = 1) in vec3 aA;   // (lateral slope, top height, layer)
uniform mat4 uVP;
uniform vec3 uEye;
out vec3 vW;
out vec3 vA;
void main() { vW = aP; vA = aA; gl_Position = uVP * vec4(aP, 1.0); }`;
const rangeFS = HEAD + COMMON + `
in vec3 vW;
in vec3 vA;
uniform vec3 uEye;
out vec4 o;
void main() {
  vec3 v = vW - uEye; float dist = length(v); vec3 dir = v / dist;
  float below = vA.y - vW.y;                       // metres below the ridge line
  vec2 st = vec2(atan(vW.x, -vW.z) * 900.0, vW.y);
  float n = vnoise(st * vec2(0.03, 0.012)) * 0.6 + vnoise(st * vec2(0.09, 0.03)) * 0.4;
  // snow on the upper part, streaky gullies further down
  float snow = 1.0 - smoothstep(120.0 + n * 260.0, 360.0 + n * 420.0, below);
  snow = max(snow, smoothstep(0.66, 0.8, vnoise(st * vec2(0.08, 0.02))) * 0.7 * (1.0 - smoothstep(300.0, 900.0, below)));
  // lateral facing: slope sign tells whether this face turns toward the sun
  vec3 tang = normalize(vec3(-dir.z, 0.0, dir.x));
  vec3 N = normalize(vec3(0.0, 1.0, 0.0) * 0.55 - tang * vA.x * 1.6 - dir * 0.6);
  float lit = clamp(dot(N, uSun) * 1.2 + 0.25, 0.0, 1.0);
  vec3 rock = vec3(0.30, 0.27, 0.27);
  vec3 alb = mix(rock, vec3(0.95), snow);
  vec3 sunC = mix(vec3(1.5, 1.3, 1.05), vec3(1.75, 1.2, 0.75), uWarm);
  vec3 col = alb * (sunC * lit + vec3(0.40, 0.50, 0.75) * 0.65);
  col = haze(col, dir, dist, 0.00009);
  o = vec4(tonemap(col), 1.0);
}`;

// --- cloud deck: big plane under the sea of puffs ---
const deckVS = HEAD + `
layout(location = 0) in vec2 aP;
uniform mat4 uVP;
uniform float uDeckY;
out vec3 vW;
void main() { vW = vec3(aP.x, uDeckY, aP.y); gl_Position = uVP * vec4(vW, 1.0); }`;
const deckFS = HEAD + COMMON + `
in vec3 vW;
uniform vec3 uEye;
uniform float uT;
uniform vec2 uDrift;
out vec4 o;
void main() {
  vec3 v = vW - uEye; float dist = length(v); vec3 dir = v / dist;
  vec2 q = vW.xz / 900.0 + uDrift;
  vec3 a = vnoised(q), b = vnoised(q * 2.3 + 5.0), c = vnoised(q * 5.1 + 9.0);
  float h = a.x * 0.55 + b.x * 0.3 + c.x * 0.15;
  vec2 g = a.yz * 0.55 + b.yz * 0.3 * 2.3 + c.yz * 0.15 * 5.1;
  vec3 N = normalize(vec3(-g.x * 0.6, 1.0, -g.y * 0.6));
  float lit = clamp(dot(N, uSun) * 2.2 + 0.35, 0.0, 1.3);
  vec3 sunC = mix(vec3(1.35, 1.2, 1.0), vec3(1.55, 1.08, 0.7), uWarm);
  vec3 shade = vec3(0.62, 0.66, 0.86);
  vec3 col = mix(shade, sunC, lit * (0.6 + 0.4 * h));
  float fwd = pow(max(dot(dir, uSun), 0.0), 8.0);
  col += vec3(0.5, 0.32, 0.15) * fwd;
  col = haze(col, dir, dist, 0.00011);
  o = vec4(tonemap(col), 1.0);
}`;

// --- soft lit cloud puffs (instanced billboards): the sea, and the word ---
const puffVS = HEAD + `
layout(location = 0) in vec2 aC;          // corner -1..1
layout(location = 1) in vec4 iP;          // centre (world or word space), radius
layout(location = 2) in vec4 iS;          // seed, alpha, brightness, kind
uniform mat4 uVP;
uniform vec3 uRight, uUp, uFwd, uEye;
uniform float uT;
uniform vec2 uWind;                       // anikit-driven gust
uniform float uWordMode;                  // 1: iP is in word space
uniform vec3 uWordC;                      // word centre (world)
uniform float uWordS;                     // word width (world units)
uniform vec2 uDrift;
out vec2 vUV;
out vec4 vS;
out vec3 vDir;
out float vDist;
void main() {
  vec3 c;
  float r = iP.w;
  if (uWordMode > 0.5) {
    // drift in the wind: each puff sways with its own phase, wisps (kind > 0.5) stream more
    float ph = iS.x * 6.2831;
    float loose = 0.35 + iS.w * 1.4;
    vec2 sway = vec2(sin(uT * 0.31 + ph), cos(uT * 0.23 + ph * 1.7)) * 0.0022 * loose;
    vec2 w = uWind * (0.004 + 0.012 * iS.w) * (0.6 + 0.4 * sin(ph));
    vec2 q = iP.xy + sway + w;
    r *= 1.0 + 0.06 * sin(uT * 0.4 + ph * 3.0);
    c = uWordC + (q.x * uRight + q.y * uUp - iP.z * uFwd) * uWordS;
    r *= uWordS;
  } else {
    c = iP.xyz + vec3(uDrift.x * 900.0, 0.0, uDrift.y * 900.0) * (0.6 + 0.4 * iS.x);
  }
  vec3 p = c + (aC.x * uRight + aC.y * uUp) * r;
  vUV = aC; vS = iS;
  vec3 v = c - uEye; vDist = length(v); vDir = v / vDist;
  gl_Position = uVP * vec4(p, 1.0);
}`;
const puffFS = HEAD + COMMON + `
in vec2 vUV;
in vec4 vS;
in vec3 vDir;
in float vDist;
uniform vec3 uSunV;    // sun in billboard space (right, up, toward-camera)
uniform float uWordMode;
out vec4 o;
void main() {
  float r2 = dot(vUV, vUV);
  if (r2 > 1.0) discard;
  // fluffy edge: a few octaves of noise warp the falloff
  vec2 q = vUV * 1.7 + vS.x * 37.0;
  float n = vnoise(q) * 0.6 + vnoise(q * 2.3 + 4.0) * 0.4;
  float rr = sqrt(r2) + (n - 0.5) * 0.35;
  float a = smoothstep(1.0, 0.35, rr) * vS.y;
  if (a < 0.004) discard;
  vec3 N = vec3(vUV, sqrt(max(1.0 - r2, 0.0)));
  float ndl = dot(N, uSunV);
  float lit = clamp(ndl * 0.6 + 0.55, 0.0, 1.0);
  // silver lining: thin edges facing the sun glow
  float rim = pow(clamp(1.0 - N.z, 0.0, 1.0), 2.0) * clamp(dot(normalize(vUV + 1e-4), normalize(uSunV.xy + 1e-4)), 0.0, 1.0);
  vec3 sunC = mix(vec3(1.45, 1.3, 1.1), vec3(1.7, 1.18, 0.72), uWarm);
  vec3 shade = uWordMode > 0.5 ? vec3(0.78, 0.82, 0.98) : vec3(0.6, 0.64, 0.85);
  vec3 col = mix(shade, sunC, lit) * vS.z + vec3(1.0, 0.75, 0.45) * rim * 0.7;
  col = haze(col, vDir, vDist, uWordMode > 0.5 ? 0.00004 : 0.00011);
  o = vec4(tonemap(col) * a, a);
}`;

// --- foreground snow and rock ---
const groundVS = HEAD + `
layout(location = 0) in vec3 aP;
layout(location = 1) in vec3 aN;
uniform mat4 uVP;
out vec3 vW;
out vec3 vN;
void main() { vW = aP; vN = aN; gl_Position = uVP * vec4(aP, 1.0); }`;
const groundFS = HEAD + COMMON + `
in vec3 vW;
in vec3 vN;
uniform vec3 uEye;
uniform float uT;
out vec4 o;
void main() {
  vec3 v = vW - uEye; float dist = length(v); vec3 dir = v / dist;
  float fade = 1.0 - smoothstep(10.0, 80.0, dist);
  // detail normals: two octaves with analytic derivatives
  vec3 d1 = vnoised(vW.xz * 1.1), d2 = vnoised(vW.xz * 4.7 + 3.0), d3 = vnoised(vW.xz * 0.27 + 9.0);
  vec2 g = d1.yz * 0.22 + d2.yz * 0.07 * fade + d3.yz * 0.35;
  vec3 N = normalize(vN + vec3(-g.x, 0.0, -g.y) * (0.35 + 0.65 * fade));
  // snow on gentle slopes, rock on steep ones; noise breaks the boundary
  float rn = d3.x * 0.6 + d1.x * 0.4;
  float snow = smoothstep(0.72, 0.86, normalize(vN).y + (rn - 0.5) * 0.45);
  vec3 L = uSun;
  vec3 sunC = mix(vec3(1.6, 1.42, 1.18), vec3(1.85, 1.3, 0.85), uWarm);
  vec3 sky = vec3(0.45, 0.58, 0.88);
  // snow: wrapped diffuse (light scatters through) + cool sky fill + sparkle
  float wrap = clamp((dot(N, L) + 0.35) / 1.35, 0.0, 1.0);
  vec3 sn = vec3(0.92, 0.94, 1.0) * (sunC * wrap * 0.85 + sky * (0.35 + 0.3 * N.y));
  vec3 H = normalize(L - dir);
  sn += sunC * 0.25 * pow(max(dot(N, H), 0.0), 24.0);
  vec2 cell = floor(vW.xz * 38.0);
  float hsh = hash12(cell);
  float tw = hash12(cell + floor(uT * 3.0 + hsh * 7.0));
  float glint = step(0.985, hsh) * step(0.4, tw) * pow(max(dot(N, H), 0.0), 3.0) * (1.0 - smoothstep(4.0, 22.0, dist));
  sn += vec3(3.5, 3.2, 2.8) * glint;
  // rock: warm granite with lichen-ish variation
  vec3 ra = mix(vec3(0.24, 0.20, 0.18), vec3(0.42, 0.36, 0.30), d2.x) * (0.8 + 0.4 * d1.x);
  vec3 rk = ra * (sunC * max(dot(N, L), 0.0) * 1.1 + sky * 0.5);
  vec3 col = mix(rk, sn, snow);
  col = haze(col, dir, dist, 0.0009);
  o = vec4(tonemap(col), 1.0);
}`;

// --- spindrift: wind-blown snow, procedural in the vertex shader ---
const driftVS = HEAD + `
layout(location = 0) in vec2 aC;
layout(location = 1) in vec4 iA;   // x0, z0, speed, seed
uniform mat4 uVP;
uniform vec3 uRight, uUp;
uniform float uT;
uniform vec2 uWind;
out vec2 vUV;
out float vA;
void main() {
  float span = 34.0;
  float life = fract(uT * iA.z * 0.09 + iA.w);
  float x = iA.x + (life - 0.5) * span;
  float z = iA.y + sin(uT * 0.7 + iA.w * 20.0) * 0.8;
  float d = length(vec2(x, z));
  float gy = -0.17 * d - 0.00624 * d * d;
  float y = gy + 0.15 + life * 1.6 * fract(iA.w * 7.3) + 0.3 * sin(uT * 2.0 + iA.w * 40.0);
  vec3 c = vec3(x, y, z);
  float r = 0.025 + 0.05 * fract(iA.w * 13.1);
  gl_Position = uVP * vec4(c + (aC.x * uRight + aC.y * uUp) * r, 1.0);
  vUV = aC;
  vA = sin(life * 3.14159) * (0.2 + 0.25 * fract(iA.w * 3.7)) * (0.7 + 0.3 * uWind.x);
}`;
const driftFS = HEAD + `
in vec2 vUV;
in float vA;
out vec4 o;
void main() {
  float a = exp(-dot(vUV, vUV) * 3.0) * vA;
  o = vec4(vec3(1.0, 0.95, 0.88) * a, a);
}`;

// --- lens bloom around the sun, and faint ghosts (screen space, additive) ---
const glareVS = HEAD + `
layout(location = 0) in vec2 aC;
layout(location = 1) in vec4 iG;   // t along sun->centre axis, size, intensity, hue
uniform vec2 uSunS;   // sun position in NDC
uniform float uAspect;
uniform float uVis;
out vec2 vUV;
out vec4 vG;
void main() {
  vec2 c = mix(uSunS, -uSunS, iG.x);
  gl_Position = vec4(c + aC * vec2(iG.y / uAspect, iG.y), 0.0, 1.0);
  vUV = aC; vG = iG; vG.z *= uVis;
}`;
const glareFS = HEAD + `
in vec2 vUV;
in vec4 vG;
out vec4 o;
void main() {
  float r = length(vUV);
  float a = vG.w < 0.5 ? exp(-r * r * 4.0) * 0.55 + exp(-r * 14.0) * 0.45 : smoothstep(1.0, 0.82, r) * (0.4 + 0.6 * r);
  vec3 c = vG.w < 0.5 ? vec3(1.0, 0.72, 0.42) : mix(vec3(0.55, 0.8, 1.0), vec3(1.0, 0.7, 0.4), fract(vG.w * 3.7));
  o = vec4(c * a * vG.z, 0.0);
}`;

// ---------------------------------------------------------------------------
// GL helpers
// ---------------------------------------------------------------------------
function compile(gl, vs, fs) {
  const mk = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    return s;
  };
  const p = gl.createProgram();
  const v = mk(gl.VERTEX_SHADER, vs), f = mk(gl.FRAGMENT_SHADER, fs);
  gl.attachShader(p, v); gl.attachShader(p, f); gl.linkProgram(p);
  return { p, v, f, u: null };
}
function finish(gl, prog, name) {
  if (!gl.getProgramParameter(prog.p, gl.LINK_STATUS)) {
    console.error('mountain: ' + name, gl.getShaderInfoLog(prog.v), gl.getShaderInfoLog(prog.f), gl.getProgramInfoLog(prog.p));
  }
  const u = {};
  const n = gl.getProgramParameter(prog.p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) { const info = gl.getActiveUniform(prog.p, i); u[info.name] = gl.getUniformLocation(prog.p, info.name); }
  prog.u = u;
  return prog;
}
function vbo(gl, data, target = gl.ARRAY_BUFFER) {
  const b = gl.createBuffer(); gl.bindBuffer(target, b); gl.bufferData(target, data, gl.STATIC_DRAW); return b;
}
function attrib(gl, loc, buf, size, stride = 0, offset = 0, divisor = 0) {
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, size, gl.FLOAT, false, stride, offset);
  gl.vertexAttribDivisor(loc, divisor);
}

// ---------------------------------------------------------------------------
// Scene constants
// ---------------------------------------------------------------------------
const EYE_Y = 1.7;
const DECK_Y = -720;
const groundH = (x, z) => {
  const d = Math.hypot(x, z);
  const az = Math.atan2(x, -z);
  // the summit ridge runs forward-left; it falls away more steeply to the right
  const spine = Math.exp(-Math.pow((az + 0.55) / 0.35, 2));
  const a = 0.17 - 0.04 * spine + 0.03 * Math.max(0, Math.sin(az));
  const b = 0.00624 * (1 - 0.35 * spine);
  let h = -a * d - b * d * d;
  h += fbm(x * 0.045, z * 0.045, 4, 3) * Math.min(1, d / 6) * (1.2 + d * 0.12);
  h += ridged(x * 0.11, z * 0.11, 3, 8) * Math.min(1, d / 10) * 1.6;
  h += fbm(x * 0.6, z * 0.6, 3, 5) * 0.12;
  return h;
};

function buildGround() {
  const NR = 150, NA = 240;
  const r0 = 0.5, r1 = 170;
  const a0 = -1.9, a1 = 1.9;
  const pos = new Float32Array(NR * NA * 3), nor = new Float32Array(NR * NA * 3);
  const e = 0.05;
  let k = 0;
  for (let i = 0; i < NR; i++) {
    const r = r0 * Math.pow(r1 / r0, i / (NR - 1));
    for (let j = 0; j < NA; j++) {
      const az = a0 + (a1 - a0) * j / (NA - 1);
      const x = Math.sin(az) * r, z = -Math.cos(az) * r;
      const h = groundH(x, z);
      const s = e * Math.max(1, r * 0.04);
      const hx = (groundH(x + s, z) - groundH(x - s, z)) / (2 * s);
      const hz = (groundH(x, z + s) - groundH(x, z - s)) / (2 * s);
      const l = Math.hypot(hx, 1, hz);
      pos[k] = x; pos[k + 1] = h; pos[k + 2] = z;
      nor[k] = -hx / l; nor[k + 1] = 1 / l; nor[k + 2] = -hz / l;
      k += 3;
    }
  }
  const idx = new Uint32Array((NR - 1) * (NA - 1) * 6);
  k = 0;
  for (let i = 0; i < NR - 1; i++) for (let j = 0; j < NA - 1; j++) {
    const a = i * NA + j, b = a + 1, c = a + NA, d = c + 1;
    idx[k++] = a; idx[k++] = c; idx[k++] = b; idx[k++] = b; idx[k++] = c; idx[k++] = d;
  }
  return { pos, nor, idx };
}

function buildRanges() {
  // layers: distance, base height of the ridge line, amplitude, frequency
  const layers = [
    [1500, -520, 420, 9, 0.45],
    [2600, -380, 560, 7, 0.55],
    [4200, -300, 650, 6, 0.7],
    [7000, -260, 900, 5, 0.85],
    [11500, -200, 1200, 4, 1.0],
    [17000, -150, 1300, 3.2, 1.0],
  ];
  const NA = 360, a0 = -2.1, a1 = 2.1;
  const verts = [];
  const idx = [];
  let base = 0;
  layers.forEach(([D, H0, A, F, cov], li) => {
    for (let j = 0; j < NA; j++) {
      const az = a0 + (a1 - a0) * j / (NA - 1);
      const top = (az) => {
        const u = az * F;
        let h = ridged(u, li * 7.3, 5, 40 + li) - 0.45;
        // sparse dominant peaks
        h += Math.max(0, vnoise(u * 0.35, li * 3.1, 90 + li)) * 0.9;
        // gaps where the cloud sea shows through
        const m = (vnoise(u * 0.5, li * 1.7, 70 + li) + 1) * 0.5;
        return H0 + A * h * (0.25 + 0.75 * Math.min(1, m / cov + 0.2));
      };
      const h = top(az);
      const dh = (top(az + 0.002) - top(az - 0.002)) / (0.004 * D);
      const dd = D * (1 + 0.08 * vnoise(az * 3, li, 5));
      const x = Math.sin(az) * dd, z = -Math.cos(az) * dd;
      verts.push(x, h, z, dh, h, li, x, -1600, z, dh, h, li);
    }
    for (let j = 0; j < NA - 1; j++) {
      const a = base + j * 2;
      idx.push(a, a + 1, a + 2, a + 2, a + 1, a + 3);
    }
    base += NA * 2;
  });
  // draw near layers first (early-z)
  return { verts: new Float32Array(verts), idx: new Uint32Array(idx) };
}

function buildSea() {
  const out = [];
  const N = 1100;
  for (let i = 0; i < N; i++) {
    const r = 500 * Math.pow(9000 / 500, Math.pow(rnd(), 0.8));
    const az = (rnd() * 2 - 1) * 2.0;
    const x = Math.sin(az) * r, z = -Math.cos(az) * r;
    const y = DECK_Y + 10 + rnd() * 70 + (fbm(x / 1500, z / 1500, 2, 11) + 0.5) * 60;
    const rad = (90 + rnd() * 140) * (0.5 + r / 3500);
    out.push([x, y, z, rad, rnd(), 0.75 + rnd() * 0.25, 0.9 + rnd() * 0.2, 0, r]);
  }
  out.sort((a, b) => b[8] - a[8]);
  const f = new Float32Array(N * 8);
  out.forEach((p, i) => f.set(p.slice(0, 8), i * 8));
  return f;
}

function buildWord() {
  // Draw the word once, then sample puff centres inside the glyphs.
  const W = 1400, H = 420;
  const c = document.createElement('canvas');
  c.width = W; c.height = H;
  const g = c.getContext('2d', { willReadFrequently: true });
  g.fillStyle = '#fff'; g.strokeStyle = '#fff';
  g.textAlign = 'center'; g.textBaseline = 'middle';
  g.font = '700 300px "Nunito", "Varela Round", "Arial Rounded MT Bold", "Helvetica Neue", Arial, sans-serif';
  g.lineJoin = 'round'; g.lineWidth = 8;
  const m = g.measureText('anikit');
  const sc = Math.min(1, (W * 0.92) / m.width);
  g.setTransform(sc, 0, 0, sc, W / 2, H / 2 + 10);
  g.fillText('anikit', 0, 0); g.strokeText('anikit', 0, 0);
  const img = g.getImageData(0, 0, W, H).data;
  const inside = (x, y) => x >= 0 && y >= 0 && x < W && y < H && img[((y | 0) * W + (x | 0)) * 4 + 3] > 127;
  // bounding box of ink
  let x0 = W, x1 = 0, y0 = H, y1 = 0;
  for (let y = 0; y < H; y += 2) for (let x = 0; x < W; x += 2) if (inside(x, y)) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
  const bw = x1 - x0, cx = (x0 + x1) / 2, cy = (y0 + y1) / 2;
  // distance to the edge (coarse), so the cores are denser and brighter than the rims
  const edgeDist = (x, y) => {
    for (let r = 2; r < 24; r += 2) {
      if (!inside(x + r, y) || !inside(x - r, y) || !inside(x, y + r) || !inside(x, y - r)) return r;
    }
    return 24;
  };
  const pts = [];
  const step = 5;
  for (let y = y0 - 4; y <= y1 + 4; y += step) for (let x = x0 - 4; x <= x1 + 4; x += step) {
    const px = x + (rnd() - 0.5) * step, py = y + (rnd() - 0.5) * step;
    if (!inside(px, py)) continue;
    const ed = edgeDist(px, py);
    const rad = 5 + Math.min(ed, 14) * 0.4 + rnd() * 3;
    pts.push([px, py, rad, ed, 0]);
  }
  // wisps: thin streams trailing off downwind (to the left, away from the sun) from some glyph edges
  const wisps = [];
  for (let k = 0; k < 26; k++) {
    let sx = 0, sy = 0, tries = 0;
    do { sx = x0 + rnd() * bw; sy = y0 + rnd() * (y1 - y0); tries++; } while ((!inside(sx, sy) || inside(sx - 10, sy)) && tries < 400);
    if (tries >= 400) continue;
    const len = 40 + rnd() * 160;
    const n = Math.floor(len / 7);
    const curl = (rnd() - 0.5) * 0.6;
    for (let i = 1; i <= n; i++) {
      const t = i / n;
      wisps.push([sx - t * len, sy + Math.sin(t * 3 + curl * 5) * 8 * t + curl * t * len * 0.3, 7 * (1 - t * 0.6) + rnd() * 3, 1 - t, 1]);
    }
  }
  const all = pts.concat(wisps);
  const N = all.length;
  const f = new Float32Array(N * 8);
  // back-to-front by depth jitter
  const rows = all.map((q) => {
    const z = (rnd() - 0.5) * 0.02;
    return [(q[0] - cx) / bw, -(q[1] - cy) / bw, z, q[2] / bw, rnd(), q[4] ? 0.35 * q[3] + 0.08 : 0.55 + 0.4 * Math.min(1, q[3] / 14), q[4] ? 1.0 : 0.95 + 0.1 * Math.min(1, q[3] / 14), q[4]];
  });
  rows.sort((a, b) => a[2] - b[2]);
  rows.forEach((r, i) => f.set(r, i * 8));
  return { inst: f, count: N, aspect: (y1 - y0) / bw };
}

// ---------------------------------------------------------------------------
// createMountain
// ---------------------------------------------------------------------------
export function createMountain(canvas) {
  const gl = canvas.getContext('webgl2', { antialias: true, alpha: false, depth: true, powerPreference: 'high-performance', preserveDrawingBuffer: false });
  if (!gl) throw new Error('mountain: WebGL2 unavailable');

  // compile everything up front (parallel compile where supported)
  const P = {
    sky: compile(gl, skyVS, skyFS),
    range: compile(gl, rangeVS, rangeFS),
    deck: compile(gl, deckVS, deckFS),
    puff: compile(gl, puffVS, puffFS),
    ground: compile(gl, groundVS, groundFS),
    drift: compile(gl, driftVS, driftFS),
    glare: compile(gl, glareVS, glareFS),
  };
  for (const k in P) finish(gl, P[k], k);

  // --- geometry ---
  const quad = vbo(gl, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]));
  const tri = vbo(gl, new Float32Array([-1, -1, 3, -1, -1, 3]));

  const vSky = gl.createVertexArray(); gl.bindVertexArray(vSky); attrib(gl, 0, tri, 2);

  const G = buildGround();
  const vGround = gl.createVertexArray(); gl.bindVertexArray(vGround);
  attrib(gl, 0, vbo(gl, G.pos), 3); attrib(gl, 1, vbo(gl, G.nor), 3);
  vbo(gl, G.idx, gl.ELEMENT_ARRAY_BUFFER);
  const groundCount = G.idx.length;

  const R = buildRanges();
  const vRange = gl.createVertexArray(); gl.bindVertexArray(vRange);
  const rb = vbo(gl, R.verts);
  attrib(gl, 0, rb, 3, 24, 0); attrib(gl, 1, rb, 3, 24, 12);
  vbo(gl, R.idx, gl.ELEMENT_ARRAY_BUFFER);
  const rangeCount = R.idx.length;

  const vDeck = gl.createVertexArray(); gl.bindVertexArray(vDeck);
  attrib(gl, 0, vbo(gl, new Float32Array([-40000, -40000, 40000, -40000, -40000, 400, 40000, 400])), 2);

  const sea = buildSea();
  const vSea = gl.createVertexArray(); gl.bindVertexArray(vSea);
  attrib(gl, 0, quad, 2);
  const sb = vbo(gl, sea);
  attrib(gl, 1, sb, 4, 32, 0, 1); attrib(gl, 2, sb, 4, 32, 16, 1);
  const seaCount = sea.length / 8;

  const word = buildWord();
  const vWord = gl.createVertexArray(); gl.bindVertexArray(vWord);
  attrib(gl, 0, quad, 2);
  const wb = vbo(gl, word.inst);
  attrib(gl, 1, wb, 4, 32, 0, 1); attrib(gl, 2, wb, 4, 32, 16, 1);

  const ND = 700;
  const dr = new Float32Array(ND * 4);
  for (let i = 0; i < ND; i++) {
    const z = -(2 + Math.pow(rnd(), 1.4) * 26);
    dr[i * 4] = (rnd() - 0.5) * 20; dr[i * 4 + 1] = z; dr[i * 4 + 2] = 0.6 + rnd() * 0.9; dr[i * 4 + 3] = rnd();
  }
  const vDrift = gl.createVertexArray(); gl.bindVertexArray(vDrift);
  attrib(gl, 0, quad, 2);
  attrib(gl, 1, vbo(gl, dr), 4, 0, 0, 1);

  // glare: [t along axis, size (NDC height), intensity, kind/hue]
  const glare = new Float32Array([
    0.0, 0.55, 0.32, 0.0,
    0.0, 0.16, 0.45, 0.0,
    0.55, 0.07, 0.05, 1.1,
    0.8, 0.13, 0.035, 1.5,
    1.15, 0.05, 0.05, 1.8,
    1.45, 0.2, 0.025, 1.3,
  ]);
  const vGlare = gl.createVertexArray(); gl.bindVertexArray(vGlare);
  attrib(gl, 0, quad, 2);
  attrib(gl, 1, vbo(gl, glare), 4, 0, 0, 1);
  gl.bindVertexArray(null);

  // --- camera state (all preallocated) ---
  const VP = new Float32Array(16);
  const cam = { yaw: 0, pitch: 0, x: 0, y: 0, push: 0 };
  const camTarget = { yaw: 0, pitch: 0, x: 0, y: 0 };
  const pushTarget = { push: 0 };
  let lastLookX = 0, lastLookY = 0, lastPush = 0;
  const wind = { x: 0, y: 0 };
  // a slow looping gust cycle, seeked by time (main.js owns the frame loop)
  const WIND_T = 36;
  const windTl = timeline({ autoplay: false })
    .add(wind, { x: [0, 1, 0.3, -0.6, 0.5, 0], y: [0, 0.4, -0.5, 0.2, -0.3, 0] }, { duration: WIND_T, ease: 'linear' });
  let cssW = 1, cssH = 1, aspect = 1;
  const BASE_FOV = 58 * Math.PI / 180;
  let tanY = Math.tan(BASE_FOV / 2), tanX = tanY;
  let wordFrac = 0.6, wordElev = 0.2;
  const right = [1, 0, 0], up = [0, 1, 0], fwd = [0, 0, -1], eye = [0, EYE_Y, 0], sun = [0, 0, -1], wordC = [0, 0, -1];

  function resize(w, h, dpr) {
    cssW = Math.max(1, w); cssH = Math.max(1, h);
    const r = Math.min(dpr || 1, 2);
    canvas.width = Math.round(cssW * r); canvas.height = Math.round(cssH * r);
    aspect = cssW / cssH;
    // portrait screens: widen the vertical FOV a little so the scene still reads
    const fov = aspect < 1 ? BASE_FOV * 1.25 : BASE_FOV;
    tanY = Math.tan(fov / 2); tanX = tanY * aspect;
    wordFrac = aspect < 1 ? 0.86 : 0.6;
  }

  function buildCamera(push) {
    const cy = Math.cos(cam.yaw), sy = Math.sin(cam.yaw), cp = Math.cos(cam.pitch), sp = Math.sin(cam.pitch);
    fwd[0] = sy * cp; fwd[1] = sp; fwd[2] = -cy * cp;
    right[0] = cy; right[1] = 0; right[2] = sy;
    // up = right x fwd
    up[0] = right[1] * fwd[2] - right[2] * fwd[1];
    up[1] = right[2] * fwd[0] - right[0] * fwd[2];
    up[2] = right[0] * fwd[1] - right[1] * fwd[0];
    const dolly = push * push * 9;
    eye[0] = cam.x + fwd[0] * dolly; eye[1] = EYE_Y + cam.y + fwd[1] * dolly; eye[2] = fwd[2] * dolly;
    const zoom = 1 - 0.12 * push;
    const ty = tanY * zoom, tx = tanX * zoom;
    const n = 0.1, f = 60000;
    const A = (f + n) / (n - f), B = (2 * f * n) / (n - f);
    // VP = Proj * View, written out directly
    const ex = -(right[0] * eye[0] + right[1] * eye[1] + right[2] * eye[2]);
    const ey = -(up[0] * eye[0] + up[1] * eye[1] + up[2] * eye[2]);
    const ez = (fwd[0] * eye[0] + fwd[1] * eye[1] + fwd[2] * eye[2]);
    const sx = 1 / tx, syy = 1 / ty;
    // column-major
    VP[0] = sx * right[0]; VP[4] = sx * right[1]; VP[8] = sx * right[2]; VP[12] = sx * ex;
    VP[1] = syy * up[0]; VP[5] = syy * up[1]; VP[9] = syy * up[2]; VP[13] = syy * ey;
    // view-space z = -fwd·(p - eye)
    VP[2] = -A * fwd[0]; VP[6] = -A * fwd[1]; VP[10] = -A * fwd[2]; VP[14] = A * ez + B;
    VP[3] = fwd[0]; VP[7] = fwd[1]; VP[11] = fwd[2]; VP[15] = -ez;
    camOut.tx = tx; camOut.ty = ty;
  }
  const camOut = { tx: 1, ty: 1 };

  function setCommon(prog) {
    const u = prog.u;
    if (u.uSun) gl.uniform3f(u.uSun, sun[0], sun[1], sun[2]);
    if (u.uWarm) gl.uniform1f(u.uWarm, curWarm);
    if (u.uExposure) gl.uniform1f(u.uExposure, 0.82);
    if (u.uVP) gl.uniformMatrix4fv(u.uVP, false, VP);
    if (u.uEye) gl.uniform3f(u.uEye, eye[0], eye[1], eye[2]);
    if (u.uRight) gl.uniform3f(u.uRight, right[0], right[1], right[2]);
    if (u.uUp) gl.uniform3f(u.uUp, up[0], up[1], up[2]);
    if (u.uFwd) gl.uniform3f(u.uFwd, fwd[0], fwd[1], fwd[2]);
    if (u.uT) gl.uniform1f(u.uT, curT);
    if (u.uWind) gl.uniform2f(u.uWind, wind.x, wind.y);
    if (u.uDrift) gl.uniform2f(u.uDrift, driftX, driftY);
  }
  let curWarm = 0.6, curT = 0, driftX = 0, driftY = 0;

  function frame(t, dt, p, look, push) {
    curT = t;
    const lx = look ? look.x || 0 : 0, ly = look ? look.y || 0 : 0;
    // springs smooth the head turn and the dolly; retarget only when the input moves
    if (Math.abs(lx - lastLookX) > 0.002 || Math.abs(ly - lastLookY) > 0.002) {
      lastLookX = lx; lastLookY = ly;
      camTarget.yaw = lx * 0.075; camTarget.pitch = ly * 0.04; camTarget.x = lx * 0.25; camTarget.y = ly * 0.08;
      spring(cam, camTarget, { stiffness: 60, damping: 16 });
    }
    const pu = push || 0;
    if (Math.abs(pu - lastPush) > 0.001) { lastPush = pu; pushTarget.push = pu; spring(cam, pushTarget, { stiffness: 120, damping: 22 }); }
    windTl.seek(t % WIND_T);

    // time of day: the sun sinks a touch and warms as you scroll
    const pp = Math.min(1, Math.max(0, p || 0));
    const el = (5.0 - 1.8 * pp) * Math.PI / 180, az = 0.66;
    sun[0] = Math.sin(az) * Math.cos(el); sun[1] = Math.sin(el); sun[2] = -Math.cos(az) * Math.cos(el);
    curWarm = 0.55 + 0.35 * pp;
    driftX = t * 0.0025 + pp * 0.15; driftY = -t * 0.001;

    const pitchBase = -0.02;
    const savedPitch = cam.pitch; cam.pitch = savedPitch + pitchBase;
    buildCamera(cam.push);
    cam.pitch = savedPitch;

    // word placement: centred, ~60% of the width, high in the sky
    const dist = 2600;
    const wW = wordFrac * 2 * dist * tanX;
    const wH = wW * word.aspect;
    const maxH = 0.42 * 2 * dist * tanY; // never taller than ~42% of the height
    const wordS = wH > maxH ? wW * maxH / wH : wW;
    const elev = Math.atan(0.58 * tanY) + pitchBase;
    wordC[0] = 0; wordC[1] = EYE_Y + Math.sin(elev) * dist; wordC[2] = -Math.cos(elev) * dist;

    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0.8, 0.85, 0.95, 1);
    gl.clearDepth(1);
    gl.depthMask(true);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST); gl.depthFunc(gl.LEQUAL);
    gl.disable(gl.BLEND);
    gl.enable(gl.CULL_FACE); gl.cullFace(gl.BACK); gl.frontFace(gl.CCW);
    gl.disable(gl.CULL_FACE);

    // 1. foreground (largest occluder first)
    gl.useProgram(P.ground.p); setCommon(P.ground);
    gl.bindVertexArray(vGround); gl.drawElements(gl.TRIANGLES, groundCount, gl.UNSIGNED_INT, 0);
    // 2. ranges
    gl.useProgram(P.range.p); setCommon(P.range);
    gl.bindVertexArray(vRange); gl.drawElements(gl.TRIANGLES, rangeCount, gl.UNSIGNED_INT, 0);
    // 3. cloud deck
    gl.useProgram(P.deck.p); setCommon(P.deck); gl.uniform1f(P.deck.u.uDeckY, DECK_Y);
    gl.bindVertexArray(vDeck); gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    // 4. sky fills what's left
    gl.depthMask(false);
    gl.useProgram(P.sky.p); setCommon(P.sky); gl.uniform2f(P.sky.u.uTan, camOut.tx, camOut.ty);
    gl.bindVertexArray(vSky); gl.drawArrays(gl.TRIANGLES, 0, 3);

    // sun direction in billboard space for puff lighting
    const sR = sun[0] * right[0] + sun[1] * right[1] + sun[2] * right[2];
    const sU = sun[0] * up[0] + sun[1] * up[1] + sun[2] * up[2];
    const sF = -(sun[0] * fwd[0] + sun[1] * fwd[1] + sun[2] * fwd[2]);

    // 5. sea of clouds and the word (premultiplied alpha, depth-tested, no depth writes)
    gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.useProgram(P.puff.p); setCommon(P.puff);
    const pu2 = P.puff.u;
    gl.uniform3f(pu2.uSunV, sR, sU, sF);
    gl.uniform1f(pu2.uWordMode, 0);
    gl.bindVertexArray(vSea); gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, seaCount);
    // the word: lit from the sun's side in screen space (it sits low and to the right)
    const wl = Math.hypot(sR, sU) || 1;
    gl.uniform3f(pu2.uSunV, 0.82 * sR / wl, 0.82 * (sU / wl) * 0.6 - 0.1, 0.45);
    gl.uniform1f(pu2.uWordMode, 1);
    gl.uniform3f(pu2.uWordC, wordC[0], wordC[1], wordC[2]);
    gl.uniform1f(pu2.uWordS, wordS);
    gl.bindVertexArray(vWord); gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, word.count);

    // 6. spindrift
    gl.useProgram(P.drift.p); setCommon(P.drift);
    gl.bindVertexArray(vDrift); gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, ND);

    // 7. lens bloom and ghosts (screen space, additive)
    const sx = sR / (sF < 0 ? -sF : 1e-3), sy = sU / (sF < 0 ? -sF : 1e-3);
    // (sF is the component toward the camera, so the sun is in front when sF < 0)
    const nx = sx / camOut.tx, ny = sy / camOut.ty;
    const vis = sF < 0 ? Math.max(0, 1 - Math.max(Math.abs(nx), Math.abs(ny)) * 0.6) : 0;
    gl.disable(gl.DEPTH_TEST);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.useProgram(P.glare.p);
    gl.uniform2f(P.glare.u.uSunS, nx, ny);
    gl.uniform1f(P.glare.u.uAspect, aspect);
    gl.uniform1f(P.glare.u.uVis, vis);
    gl.bindVertexArray(vGlare); gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, 6);

    gl.bindVertexArray(null);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
  }

  return { resize, frame };
}
