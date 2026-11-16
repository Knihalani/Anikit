// hall-world-city.js — world 4: the kinetic city.
//
// A night city of 3600 instanced towers on a 288 m tile that wraps around you (the vertex
// shader wraps each tower's position into the tile centred on the camera; fog to the exact
// sky colour hides the seam). The city is 36 districts of 10×10 towers. As you walk, the
// district ahead of you rearranges itself: an anikit timeline() runs two batch()es over its
// towers, one with keyframed height/footprint/window-glow (sink, overshoot, settle) and one
// with a transient slide, both staggered across a 10×10 grid from the tower nearest you.
// A click sends a lift wave out from where you look: another batch, delayed by distance.
// All per-tower state lives in one Float32Array the batches are copied into, uploaded once
// per frame while anything moves.
//
// Rendering: scene into a full-res RGBA8 target (alpha = emissive), bright extract + blur at
// half resolution and again at quarter, then one composite into the caller's framebuffer.
//
//   createWorld(gl) → { name, tint, spawn, resize, enter, exit, preview, frame, setQuality }

import { batch, timeline, stagger } from '../../../packages/anikit/dist/anikit.lean.js';

export const SHARED_GL = true;

const LOT = 4;                 // m per lot
const NL = 72;                 // lots per tile side
const TILE = NL * LOT;         // 288 m
const DPL = 12;                // lots per district side (two blocks of 5 towers + roads)
const ND = NL / DPL;           // 6 districts per side
const FOG_END = 136;           // fully sky-coloured before the wrap seam at 144 m

// ---------------------------------------------------------------- shaders

const SKY_GLSL = /* glsl */ `
uniform vec3 uMoon;
vec3 skyCol(vec3 d) {
  float y = max(d.y, 0.0);
  vec3 zen = vec3(0.02, 0.03, 0.085), hz = vec3(0.17, 0.14, 0.25);
  vec3 c = mix(hz, zen, pow(smoothstep(0.0, 0.6, y), 0.7));
  c += vec3(0.32, 0.15, 0.07) * exp(-y * 14.0) * 0.55;          // sodium glow of the city on the haze
  float mu = max(dot(d, uMoon), 0.0);
  c += vec3(0.35, 0.42, 0.6) * (pow(mu, 18.0) * 0.18 + pow(mu, 300.0) * 0.4);
  return c;
}`;

const SKY_VS = /* glsl */ `#version 300 es
uniform vec3 uF, uR, uU;
uniform vec2 uTan;
out vec3 vDir;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)) * 2.0 - 1.0;
  vDir = uF + p.x * uTan.x * uR + p.y * uTan.y * uU;
  gl_Position = vec4(p, 1.0, 1.0);   // at the far plane: drawn after geometry, LEQUAL
}`;

const SKY_FS = /* glsl */ `#version 300 es
precision highp float;
in vec3 vDir;
uniform float uT;
out vec4 o;
${SKY_GLSL}
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
void main() {
  vec3 d = normalize(vDir);
  vec3 c = skyCol(d);
  float mu = dot(d, uMoon);
  float disc = smoothstep(0.99935, 0.9996, mu);
  c += vec3(0.95, 0.93, 0.85) * disc;
  float e = disc;
  if (d.y > 0.1) {
    vec2 g = vec2(atan(d.z, d.x) * 140.0, d.y * 180.0);
    vec2 cell = floor(g), f = fract(g) - 0.5;
    float h = hash(cell);
    float s = smoothstep(0.13, 0.0, length(f)) * step(0.988, h) * smoothstep(0.1, 0.5, d.y);
    c += vec3(0.85, 0.9, 1.0) * s * (0.5 + 0.5 * sin(uT * (0.8 + h * 2.0) + h * 40.0));
  }
  o = vec4(c, e * 0.8);
}`;

const GROUND_VS = /* glsl */ `#version 300 es
uniform mat4 uVP;
uniform vec3 uEye;
out vec3 vW;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)) * 2.0 - 1.0;  // a big triangle
  vec3 w = vec3(uEye.x + p.x * 300.0, 0.0, uEye.z + p.y * 300.0);
  vW = w;
  gl_Position = uVP * vec4(w, 1.0);
}`;

const GROUND_FS = /* glsl */ `#version 300 es
precision highp float;
in vec3 vW;
uniform vec3 uEye;
uniform float uT;
out vec4 o;
${SKY_GLSL}
float hash1(float n) { return fract(sin(n * 12.9898) * 43758.5453); }
// distance to the centre line of the nearest road, and which road
vec2 road(float x) { float m = mod(x + 12.0, 24.0) - 12.0; return vec2(abs(m), floor((x + 12.0) / 24.0)); }
// moving car lights along a road: lane offset, travel direction
float cars(float across, float along, float idx, float lane, float dir) {
  float sp = 9.0 + 5.0 * hash1(idx * 3.1 + lane);
  float gap = 31.0 + 17.0 * hash1(idx * 7.7 + lane);
  float s = mod(along - dir * sp * uT + hash1(idx + lane) * gap, gap);
  float l = abs(across - lane);
  float body = smoothstep(0.32, 0.0, l) * smoothstep(1.2, 0.0, abs(s - 0.8));
  return body;
}
void main() {
  vec2 rx = road(vW.x), rz = road(vW.z);
  float onX = step(rx.x, 2.0), onZ = step(rz.x, 2.0);       // on a road running along z / along x
  float onRoad = max(onX, onZ);
  vec3 c = mix(vec3(0.03, 0.032, 0.042), vec3(0.018, 0.019, 0.026), onRoad);
  // kerbs and lane dashes, faded with distance to stay alias-free
  vec2 fw = fwidth(vW.xz);
  float aa = 1.0 - smoothstep(0.08, 0.35, max(fw.x, fw.y));
  float dashZ = onX * (1.0 - onZ) * smoothstep(0.07, 0.0, rx.x) * step(0.5, fract(vW.z / 3.0));
  float dashX = onZ * (1.0 - onX) * smoothstep(0.07, 0.0, rz.x) * step(0.5, fract(vW.x / 3.0));
  float kerb = (smoothstep(0.08, 0.0, abs(rx.x - 2.0)) + smoothstep(0.08, 0.0, abs(rz.x - 2.0)));
  c += vec3(0.55, 0.5, 0.4) * (dashZ + dashX) * 0.35 * aa + vec3(0.08, 0.07, 0.07) * kerb * aa;
  // street-light pools at the corners of each crossing
  vec2 q = abs(mod(vW.xz + 12.0, 24.0) - 12.0) - 2.4;
  float pool = exp(-dot(q, q) * 0.22);
  c += vec3(1.0, 0.55, 0.22) * pool * 0.2;
  // traffic: white heads one way, red tails the other
  float e = pool * 0.25;
  float hw = 0.0, tl = 0.0;
  if (onX * (1.0 - onZ) > 0.5) { float m = mod(vW.x + 12.0, 24.0) - 12.0; hw += cars(m, vW.z, rx.y, -0.9, 1.0); tl += cars(m, vW.z, rx.y, 0.9, -1.0); }
  if (onZ * (1.0 - onX) > 0.5) { float m = mod(vW.z + 12.0, 24.0) - 12.0; hw += cars(m, vW.x, rz.y + 50.0, 0.9, 1.0); tl += cars(m, vW.x, rz.y + 50.0, -0.9, -1.0); }
  c += vec3(1.0, 0.92, 0.8) * hw * 1.4 + vec3(1.0, 0.12, 0.08) * tl * 1.1;
  e += (hw + tl) * 0.9;
  // fog into the sky at the same direction
  vec3 tv = vW - uEye;
  float dist = length(tv);
  float fog = max(1.0 - exp(-dist * 0.012), smoothstep(${(FOG_END - 40).toFixed(1)}, ${FOG_END.toFixed(1)}, dist));
  c = mix(c, skyCol(tv / dist), fog);
  o = vec4(c, e * (1.0 - fog));
}`;

const TOWER_VS = /* glsl */ `#version 300 es
in vec3 aPos;
in vec3 aNrm;
in vec4 aStat;    // x, z, seed, accent
in vec4 aA;       // height, width, depth, glow
in vec4 aB;       // slide x, slide z, lift, -
uniform mat4 uVP;
uniform vec3 uEye;
out vec3 vW;
flat out vec3 vN;
out vec4 vLoc;    // face/roof coords in metres: u, v (height), half-width of the face, roof z
flat out vec2 vHalf;  // half footprint
flat out vec4 vInfo;  // seed, height, glow, lift
flat out float vAcc;
void main() {
  vec2 c = aStat.xy + aB.xy;
  vec2 rel = mod(c - uEye.xz + ${(TILE / 2).toFixed(1)}, ${TILE.toFixed(1)}) - ${(TILE / 2).toFixed(1)};
  c = uEye.xz + rel;
  float dist = length(rel);
  float lift = aB.z;
  // the city makes way for the walker
  float h = aA.x * smoothstep(3.5, 12.0, dist) + lift * (5.0 + 0.3 * aA.x);
  h = max(h, 0.05);
  vec3 sz = vec3(aA.y, h, aA.z);
  vec3 p = vec3(c.x, 0.0, c.y) + aPos * sz;
  vW = p;
  vN = aNrm;
  bool xFace = abs(aNrm.x) > 0.5;
  vLoc = vec4(xFace ? aPos.z * sz.z : aPos.x * sz.x, aPos.y * h, (xFace ? sz.z : sz.x) * 0.5, aPos.z * sz.z);
  vHalf = sz.xz * 0.5;
  vInfo = vec4(aStat.z, h, aA.w, lift);
  vAcc = aStat.w;
  gl_Position = uVP * vec4(p, 1.0);
}`;

const DEPTH_FS = /* glsl */ `#version 300 es
precision lowp float;
out vec4 o;
void main() { o = vec4(0.0); }`;

const TOWER_FS = /* glsl */ `#version 300 es
precision highp float;
in vec3 vW;
flat in vec3 vN;
in vec4 vLoc;
flat in vec2 vHalf;
flat in vec4 vInfo;
flat in float vAcc;
uniform vec3 uEye;
uniform float uT;
uniform float uDetail;
out vec4 o;
${SKY_GLSL}
float hash(vec3 p) { p = fract(p * vec3(0.1031, 0.1030, 0.0973)); p += dot(p, p.yxz + 33.33); return fract((p.x + p.y) * p.z); }
void main() {
  vec3 n = vN;
  float seed = vInfo.x, h = vInfo.y, glow = vInfo.z, lift = vInfo.w;
  float moon = max(dot(n, normalize(uMoon * vec3(1.0, 0.6, 1.0))), 0.0);
  vec3 c = vec3(0.06, 0.07, 0.11) * (0.6 + 1.1 * moon) + vec3(0.05, 0.07, 0.12) * n.y;
  c += vec3(0.42, 0.22, 0.09) * exp(-vW.y * 0.5) * 0.1;      // street light on the lower floors
  float e = 0.0;
  vec3 accent = mix(vec3(0.2, 0.85, 1.0), vec3(1.0, 0.3, 0.75), step(0.5, fract(seed * 5.31)));
  float acc = vAcc;
  if (n.y < 0.5) {
    // windows: 0.62 m × 0.95 m cells (the towers read as many-storey buildings), lit by hash,
    // warm with a few cool ones; faded to their average where they'd alias
    vec2 g = vec2(vLoc.x + 50.0, vLoc.y - 0.5) / vec2(0.62, 0.95);
    vec2 id = floor(g), f = fract(g);
    float face = dot(n, vec3(1.0, 0.0, 3.0));
    float r = hash(vec3(id, seed * 97.0 + face * 13.0));
    float density = 0.3 + 0.5 * fract(seed * 7.13);
    float lit = step(1.0 - density, r) * step(0.0, g.y) * step(vLoc.y, h - 0.6) * step(abs(vLoc.x), vLoc.z - 0.18);
    float win = step(0.18, f.x) * step(f.x, 0.82) * step(0.25, f.y) * step(f.y, 0.85);
    vec2 fw = fwidth(g);
    float far = smoothstep(0.3, 0.75, max(fw.x, fw.y) * uDetail);
    float pat = mix(win * lit, density * 0.38 * step(vLoc.y, h - 0.6), far);
    vec3 warm = mix(vec3(1.0, 0.55, 0.22), vec3(1.0, 0.8, 0.5), fract(r * 13.0));
    warm = mix(warm, vec3(0.6, 0.8, 1.0), step(0.9, fract(r * 29.0)));
    float k = pat * glow * (0.45 + 0.55 * fract(r * 7.0));
    c += warm * k * 0.9;
    e += k * 0.7;
    // neon corner lines on accent towers
    float edge = smoothstep(0.1, 0.0, vLoc.z - abs(vLoc.x)) * acc;
    c += accent * edge * 0.9; e += edge * 0.9;
  } else {
    // roofs: an outline on accent towers, a blinking beacon on the tall ones
    vec2 q = vec2(vLoc.x, vLoc.w);
    float edge = smoothstep(0.12, 0.0, min(vHalf.x - abs(q.x), vHalf.y - abs(q.y))) * acc;
    c += accent * edge; e += edge;
    float beacon = step(48.0, h) * smoothstep(0.35, 0.0, length(q)) * pow(0.5 + 0.5 * sin(uT * 2.4 + seed * 30.0), 6.0);
    c += vec3(1.0, 0.1, 0.06) * beacon * 2.0; e += beacon;
  }
  // the click wave: whole towers flare cool-white as they rise
  float lf = max(lift, 0.0);
  c += vec3(0.5, 0.8, 1.0) * lf * (0.1 + 1.4 * e);
  e += lf * (0.3 + e);
  vec3 tv = vW - uEye;
  float dist = length(tv);
  float fog = max(1.0 - exp(-dist * 0.011), smoothstep(${(FOG_END - 40).toFixed(1)}, ${FOG_END.toFixed(1)}, dist));
  c = mix(c, skyCol(tv / dist), fog);
  o = vec4(c, min(e, 1.0) * (1.0 - fog));
}`;

const POST_VS = /* glsl */ `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

// bright extract: emissive (alpha) weighted colour, one bilinear tap = a 2×2 average
const EXTRACT_FS = /* glsl */ `#version 300 es
precision mediump float;
uniform sampler2D uSrc;
uniform vec2 uDst;
out vec4 o;
void main() {
  vec4 s = texture(uSrc, gl_FragCoord.xy / uDst);
  o = vec4(s.rgb * s.a, 1.0);
}`;

// 9-tap gaussian with 5 bilinear fetches
const BLUR_FS = /* glsl */ `#version 300 es
precision mediump float;
uniform sampler2D uSrc;
uniform vec2 uDst;
uniform vec2 uDir;   // texel step in source uv
out vec4 o;
void main() {
  vec2 uv = gl_FragCoord.xy / uDst;
  vec3 c = texture(uSrc, uv).rgb * 0.2270270;
  c += (texture(uSrc, uv + uDir * 1.3846154).rgb + texture(uSrc, uv - uDir * 1.3846154).rgb) * 0.3162162;
  c += (texture(uSrc, uv + uDir * 3.2307692).rgb + texture(uSrc, uv - uDir * 3.2307692).rgb) * 0.0702703;
  o = vec4(c, 1.0);
}`;

const COMPOSITE_FS = /* glsl */ `#version 300 es
precision mediump float;
uniform sampler2D uScene, uB1, uB2;
uniform vec2 uDst;
uniform vec3 uClip;
uniform vec4 uRect;
uniform float uB2On;
out vec4 o;
float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
void main() {
  if (uClip.z > 0.0 && distance(gl_FragCoord.xy, uClip.xy) > uClip.z) discard;
  vec2 uv = gl_FragCoord.xy / uDst;
  vec3 c = texture(uScene, uv).rgb;
  c += texture(uB1, uv).rgb * 1.25 + texture(uB2, uv).rgb * 1.9 * uB2On;
  c = c / (1.0 + max(c - 0.8, 0.0) * 0.6);                 // soft shoulder on the bright bits
  vec2 r = (gl_FragCoord.xy - uRect.xy) / uRect.zw - 0.5;
  c *= 1.0 - dot(r, r) * 0.45;                            // vignette
  c += (hash(gl_FragCoord.xy) - 0.5) / 255.0;
  o = vec4(c, 1.0);
}`;

// ---------------------------------------------------------------- GL helpers

function compile(gl, vs, fs, attribs) {
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error('city shader: ' + gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
  if (attribs) attribs.forEach((a, i) => gl.bindAttribLocation(p, i, a));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error('city link: ' + gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const info = gl.getActiveUniform(p, i);
    u[info.name.replace(/\[0\]$/, '')] = gl.getUniformLocation(p, info.name);
  }
  return { p, u };
}

function target(gl, w, h, depth) {
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  const fb = gl.createFramebuffer();
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  let rb = null;
  if (depth) {
    rb = gl.createRenderbuffer();
    gl.bindRenderbuffer(gl.RENDERBUFFER, rb);
    gl.renderbufferStorage(gl.RENDERBUFFER, gl.DEPTH_COMPONENT24, w, h);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, rb);
    gl.bindRenderbuffer(gl.RENDERBUFFER, null);
  }
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.bindTexture(gl.TEXTURE_2D, null);
  return { tex, fb, rb, w, h };
}
function freeTarget(gl, t) {
  if (!t) return;
  gl.deleteTexture(t.tex); gl.deleteFramebuffer(t.fb);
  if (t.rb) gl.deleteRenderbuffer(t.rb);
}

// unit box: x, z in [-0.5, 0.5], y in [0, 1], no bottom face; interleaved position + normal
function boxMesh() {
  const v = [], idx = [];
  const faces = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, 0, 1], [0, 0, -1]];
  for (const [nx, ny, nz] of faces) {
    const a = nx !== 0 ? [0, 0, 1] : [1, 0, 0];
    const b = [ny * a[2] - nz * a[1], nz * a[0] - nx * a[2], nx * a[1] - ny * a[0]];
    const base = v.length / 6;
    for (const [s, t] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
      v.push(0.5 * (nx + s * a[0] + t * b[0]), 0.5 * (ny + s * a[1] + t * b[1]) + 0.5, 0.5 * (nz + s * a[2] + t * b[2]), nx, ny, nz);
    }
    idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
  }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}

// ---------------------------------------------------------------- city layout

let rs = 0x9e3779b9;
function rnd() { // xorshift, deterministic so every visit sees the same city
  rs ^= rs << 13; rs >>>= 0; rs ^= rs >>> 17; rs ^= rs << 5; rs >>>= 0;
  return rs / 4294967296;
}

// Arrangements a district can take; each returns [height, width, depth] for local cell (i, j) of 10×10
const PATTERNS = [
  (i, j, r, s) => [5 + 62 * s * Math.pow(r(), 2.4), 2.3 + r() * 1.3, 2.3 + r() * 1.3],                // skyline
  (i, j) => { const d = Math.max(Math.abs(i - 4.5), Math.abs(j - 4.5)); return [6 + 70 * Math.pow(1 - d / 5, 1.6), 3.3, 3.3]; }, // ziggurat
  (i, j) => { const d = Math.hypot(i - 4.5, j - 4.5); return [Math.abs(d - 3.6) < 1 ? 46 : 7, 3.2, 3.2]; },   // crater ring
  (i, j, r) => [i % 2 ? 9 + r() * 4 : 34 + r() * 8, i % 2 ? 2.4 : 3.4, 3.4],                            // stripes
  (i, j) => [24 + 18 * Math.sin(i * 0.75 + j * 0.45), 3.1, 3.1],                                       // swell
  (i, j, r) => { const c = Math.abs(i - 4.5) < 1 && Math.abs(j - 4.5) < 1; return [c ? 96 : 5 + r() * 9, c ? 3.6 : 2.8, c ? 3.6 : 2.8]; }, // spire
  (i, j) => [6 + i * 5.5, 3.2, 2.6],                                                                     // ramp
  (i, j) => [((i + j) & 1) ? 6 : 28, 3.3, 3.3],                                                          // chequer
];

export function createWorld(gl) {
  // ---- programs
  const sky = compile(gl, SKY_VS, SKY_FS);
  const ground = compile(gl, GROUND_VS, GROUND_FS);
  const tower = compile(gl, TOWER_VS, TOWER_FS, ['aPos', 'aNrm', 'aStat', 'aA', 'aB']);
  const towerZ = compile(gl, TOWER_VS, DEPTH_FS, ['aPos', 'aNrm', 'aStat', 'aA', 'aB']);
  const extract = compile(gl, POST_VS, EXTRACT_FS);
  const blur = compile(gl, POST_VS, BLUR_FS);
  const comp = compile(gl, POST_VS, COMPOSITE_FS);

  // ---- towers: static per-instance data and the animated state
  const towers = [];
  for (let j = 0; j < NL; j++) for (let i = 0; i < NL; i++) if (i % 6 && j % 6) towers.push([i, j]);
  const n = towers.length;                        // 3600
  const stat = new Float32Array(n * 4);
  const dyn = new Float32Array(n * 8);            // h, w, d, glow | slideX, slideZ, lift, -
  const dIdx = [];                                 // per district: tower indices, row-major 10×10
  const dCx = new Float32Array(ND * ND), dCz = new Float32Array(ND * ND), dScale = new Float32Array(ND * ND);
  for (let k = 0; k < ND * ND; k++) dIdx.push(new Int32Array(100));
  const dFill = new Int32Array(ND * ND);
  for (let k = 0; k < ND * ND; k++) dScale[k] = 0.35 + rnd() * 0.75;
  // place the tile so that a crossing sits at the origin: lot i spans [(i-36)·4 − 2, (i-36)·4 + 2]
  const tmp = [];
  for (let k = 0; k < n; k++) tmp.push(k);
  tmp.sort((a, b) => { // row-major within each district
    const [ia, ja] = towers[a], [ib, jb] = towers[b];
    const da = Math.floor(ja / DPL) * ND + Math.floor(ia / DPL), db = Math.floor(jb / DPL) * ND + Math.floor(ib / DPL);
    return da - db || ja - jb || ia - ib;
  });
  for (const k of tmp) {
    const [i, j] = towers[k];
    const d = Math.floor(j / DPL) * ND + Math.floor(i / DPL);
    dIdx[d][dFill[d]++] = k;
    stat[k * 4] = (i - 36) * LOT;
    stat[k * 4 + 1] = (j - 36) * LOT;
    stat[k * 4 + 2] = rnd();
    stat[k * 4 + 3] = rnd() < 0.16 ? 1 : 0;
  }
  for (let d = 0; d < ND * ND; d++) {
    let sx = 0, sz = 0;
    for (const k of dIdx[d]) { sx += stat[k * 4]; sz += stat[k * 4 + 1]; }
    dCx[d] = sx / 100; dCz[d] = sz / 100;
  }
  // first arrangement: mostly skylines, a few graphic districts
  const out = [0, 0, 0];
  function arrange(d, pat) {
    const ids = dIdx[d], f = PATTERNS[pat], s = dScale[d];
    for (let c = 0; c < 100; c++) {
      const r = f(c % 10, (c / 10) | 0, rnd, s);
      out[0] = r[0]; out[1] = r[1]; out[2] = r[2];
      const k = ids[c];
      dyn[k * 8] = out[0]; dyn[k * 8 + 1] = out[1]; dyn[k * 8 + 2] = out[2];
    }
  }
  for (let d = 0; d < ND * ND; d++) {
    arrange(d, rnd() < 0.7 ? 0 : 1 + Math.floor(rnd() * (PATTERNS.length - 1)));
    for (const k of dIdx[d]) dyn[k * 8 + 3] = 1;
  }

  // ---- GPU buffers
  const box = boxMesh();
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const vb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vb);
  gl.bufferData(gl.ARRAY_BUFFER, box.v, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
  gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);
  const ib = gl.createBuffer();
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
  gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, box.i, gl.STATIC_DRAW);
  const sb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, sb);
  gl.bufferData(gl.ARRAY_BUFFER, stat, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(2); gl.vertexAttribPointer(2, 4, gl.FLOAT, false, 16, 0); gl.vertexAttribDivisor(2, 1);
  const db = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, db);
  gl.bufferData(gl.ARRAY_BUFFER, dyn, gl.DYNAMIC_DRAW);
  gl.enableVertexAttribArray(3); gl.vertexAttribPointer(3, 4, gl.FLOAT, false, 32, 0); gl.vertexAttribDivisor(3, 1);
  gl.enableVertexAttribArray(4); gl.vertexAttribPointer(4, 4, gl.FLOAT, false, 32, 16); gl.vertexAttribDivisor(4, 1);
  gl.bindVertexArray(null);
  gl.bindBuffer(gl.ARRAY_BUFFER, null);
  const emptyVao = gl.createVertexArray();
  let dirty = true;

  // ---- choreography: a timeline of two staggered batches per district
  const busy = new Array(ND * ND).fill(null);    // { tl, a, b, ids } while a district moves
  const active = [];                             // districts currently moving
  const waves = [];                              // { b, ids }
  let lastPat = 0;

  function rearrange(d, fromX, fromZ) {
    if (busy[d]) return;
    const ids = dIdx[d];
    // where the wave starts: the tower nearest (fromX, fromZ), in wrapped coordinates
    let best = 0, bd = 1e9;
    for (let c = 0; c < 100; c++) {
      const k = ids[c];
      const dx = wrap(stat[k * 4] - fromX), dz = wrap(stat[k * 4 + 1] - fromZ), dd = dx * dx + dz * dz;
      if (dd < bd) { bd = dd; best = c; }
    }
    let pat = 1 + Math.floor(rnd() * (PATTERNS.length - 1));
    if (pat === lastPat || rnd() < 0.3) pat = 0;
    lastPat = pat;
    const f = PATTERNS[pat], s = dScale[d];
    // resolve every target first (user code before batch(): see AGENTS.md "scratch buffer")
    const h0 = new Float32Array(100), w0 = new Float32Array(100), d0 = new Float32Array(100), g0 = new Float32Array(100);
    const h1 = new Float32Array(100), w1 = new Float32Array(100), d1 = new Float32Array(100);
    for (let c = 0; c < 100; c++) {
      const k = ids[c];
      h0[c] = dyn[k * 8]; w0[c] = dyn[k * 8 + 1]; d0[c] = dyn[k * 8 + 2]; g0[c] = dyn[k * 8 + 3];
      const r = f(c % 10, (c / 10) | 0, rnd, s);
      h1[c] = r[0]; w1[c] = r[1]; d1[c] = r[2];
    }
    const slideX = rnd() < 0.5, amt = (rnd() < 0.5 ? 1 : -1) * LOT * 0.55;
    const delay = stagger(0.055, { grid: [10, 10], from: best });
    const a = batch(100, {
      h: (c) => [h0[c], h0[c] * 0.3 + 1, h1[c] * 1.1 + 2, h1[c]],
      w: (c) => [w0[c], w0[c] * 0.8, w1[c], w1[c]],
      d: (c) => [d0[c], d0[c] * 0.8, d1[c], d1[c]],
      glow: (c) => [g0[c], 0.2, 1.7, 1],
    }, { duration: 2.4, times: [0, 0.3, 0.72, 1], ease: ['inOutCubic', 'outCubic', 'inOutSine'], delay, autoplay: false });
    const b = batch(100, {
      sx: (c) => (slideX ? [0, (((c / 10) | 0) % 2 ? amt : -amt), 0] : [0, 0, 0]),
      sz: (c) => (slideX ? [0, 0, 0] : [0, (c % 2 ? amt : -amt), 0]),
    }, { duration: 1.5, ease: 'inOutSine', delay, autoplay: false });
    const tl = timeline().add(a, 0).add(b, 0.25);
    busy[d] = { tl, a, b, ids };
    active.push(d);
  }

  function wave(px, pz) {
    // every tower within 75 m of the point (wrapped), delayed by its distance
    const R = 75;
    let m = 0;
    for (let k = 0; k < n; k++) {
      const dx = wrap(stat[k * 4] - px), dz = wrap(stat[k * 4 + 1] - pz);
      if (dx * dx + dz * dz < R * R) m++;
    }
    const ids = new Int32Array(m), dist = new Float32Array(m);
    m = 0;
    for (let k = 0; k < n; k++) {
      const dx = wrap(stat[k * 4] - px), dz = wrap(stat[k * 4 + 1] - pz), dd = Math.sqrt(dx * dx + dz * dz);
      if (dd < R) { ids[m] = k; dist[m++] = dd; }
    }
    const b = batch(m, { lift: (c) => [0, 1 - dist[c] / (R * 1.3), -0.12, 0] }, {
      duration: 1.7, times: [0, 0.28, 0.62, 1], ease: ['outCubic', 'inOutSine', 'inOutSine'],
      delay: (_, c) => dist[c] * 0.02,
    });
    waves.push({ b, ids });
  }

  function wrap(v) { return v - Math.round(v / TILE) * TILE; }

  // copy running batches into dyn; retire finished ones
  function pump() {
    for (let q = active.length - 1; q >= 0; q--) {
      const d = active[q], s = busy[d];
      const A = s.a.data, B = s.b.data, ids = s.ids;
      if (A.length === 400 && B.length === 200) {
        for (let c = 0; c < 100; c++) {
          const k = ids[c] * 8;
          dyn[k] = A[c * 4]; dyn[k + 1] = A[c * 4 + 1]; dyn[k + 2] = A[c * 4 + 2]; dyn[k + 3] = A[c * 4 + 3];
          dyn[k + 4] = B[c * 2]; dyn[k + 5] = B[c * 2 + 1];
        }
      }
      dirty = true;
      if (s.tl.state === 'finished') {
        s.tl.kill();
        busy[d] = null;
        active.splice(q, 1);
      }
    }
    if (waves.length || liftLive) {
      for (let k = 0; k < n; k++) dyn[k * 8 + 6] = 0;
      liftLive = waves.length > 0;
      for (let q = waves.length - 1; q >= 0; q--) {
        const w = waves[q], L = w.b.data, ids = w.ids;
        if (L.length === ids.length) for (let c = 0; c < ids.length; c++) dyn[ids[c] * 8 + 6] += L[c];
        if (w.b.state === 'finished') { w.b.kill(); waves.splice(q, 1); }
      }
      dirty = true;
    }
  }
  let liftLive = false;

  // ---- camera
  const FOV = 1.15, NEAR = 0.1, FAR = 420;
  const eye = new Float32Array(3), f = new Float32Array(3), r = new Float32Array(3), u = new Float32Array(3);
  const vp = new Float32Array(16);
  const moon = new Float32Array([0.42, 0.2, -0.88]);
  { const l = Math.hypot(moon[0], moon[1], moon[2]); moon[0] /= l; moon[1] /= l; moon[2] /= l; }
  function camera(px, py, pz, yaw, pitch, aspect) {
    eye[0] = px; eye[1] = py; eye[2] = pz;
    const cp = Math.cos(pitch), sp = Math.sin(pitch), cy = Math.cos(yaw), sy = Math.sin(yaw);
    f[0] = -sy * cp; f[1] = sp; f[2] = -cy * cp;
    r[0] = cy; r[1] = 0; r[2] = -sy;
    u[0] = r[1] * f[2] - r[2] * f[1]; u[1] = r[2] * f[0] - r[0] * f[2]; u[2] = r[0] * f[1] - r[1] * f[0];
    matrices(aspect, Math.tan(FOV / 2));
  }
  let tanNow = Math.tan(FOV / 2), aspNow = 1;
  function matrices(aspect, tanY) {
    tanNow = tanY; aspNow = aspect;
    const px = eye[0], py = eye[1], pz = eye[2];
    const t = 1 / tanY;
    const a = t / aspect, b = t, c = (FAR + NEAR) / (NEAR - FAR), dd = (2 * FAR * NEAR) / (NEAR - FAR);
    for (let i = 0; i < 3; i++) {
      vp[i * 4] = a * r[i]; vp[i * 4 + 1] = b * u[i]; vp[i * 4 + 2] = -c * f[i]; vp[i * 4 + 3] = f[i];
    }
    const ex = -(r[0] * px + r[1] * py + r[2] * pz), ey = -(u[0] * px + u[1] * py + u[2] * pz), ez = f[0] * px + f[1] * py + f[2] * pz;
    vp[12] = a * ex; vp[13] = b * ey; vp[14] = c * ez + dd; vp[15] = -ez;
  }

  // ---- triggers: the district ahead of you moves as you walk; others move now and then
  let walked = 0, lastX = 0, lastZ = 0, ambient = 0.5;
  function pickAhead(minA, maxA) {
    let best = -1, bs = 1e9;
    const fx = -Math.sin(yawNow), fz = -Math.cos(yawNow);
    for (let d = 0; d < ND * ND; d++) {
      if (busy[d]) continue;
      const rx = wrap(dCx[d] - eye[0]), rz = wrap(dCz[d] - eye[2]);
      const along = rx * fx + rz * fz, lat = Math.abs(rx * fz - rz * fx);
      if (along < minA || along > maxA || lat > along * 0.8 + 10) continue;
      const sc = Math.abs(along - (minA + maxA) * 0.45) + lat * 0.6 + rnd() * 12;
      if (sc < bs) { bs = sc; best = d; }
    }
    return best;
  }
  let yawNow = 0;
  function triggers(dt, moved) {
    walked += moved;
    if (walked > 10) {
      walked = 0;
      const d = pickAhead(14, 80);
      if (d >= 0) rearrange(d, eye[0], eye[2]);
    }
    ambient -= dt;
    if (ambient <= 0) {
      ambient = 1.6 + rnd() * 1.4;
      if (active.length < 4) {
        const d = pickAhead(20, 120);
        if (d >= 0) rearrange(d, eye[0], eye[2]);
      }
    }
  }

  // ---- render targets (allocated in resize)
  let W = 2, H = 2, dpr = 1, quality = 0;
  let scene = null, h1 = null, h2 = null, q1 = null, q2 = null;
  function alloc(w, h) {
    [scene, h1, h2, q1, q2].forEach((t) => freeTarget(gl, t));
    scene = target(gl, w, h, true);
    const hw = Math.max(1, Math.ceil(w / 2)), hh = Math.max(1, Math.ceil(h / 2));
    const qw = Math.max(1, Math.ceil(w / 4)), qh = Math.max(1, Math.ceil(h / 4));
    h1 = target(gl, hw, hh, false); h2 = target(gl, hw, hh, false);
    q1 = target(gl, qw, qh, false); q2 = target(gl, qw, qh, false);
  }
  alloc(W, H);

  const clip = new Float32Array(3), rect = new Float32Array(4);

  // scissor a target to the rect (scaled to its size), with a margin so blurs don't read stale texels
  function region(t, x, y, w, h, margin) {
    const sx = t.w / W, sy = t.h / H;
    const x0 = Math.max(0, Math.floor(x * sx) - margin), y0 = Math.max(0, Math.floor(y * sy) - margin);
    const x1 = Math.min(t.w, Math.ceil((x + w) * sx) + margin), y1 = Math.min(t.h, Math.ceil((y + h) * sy) + margin);
    gl.bindFramebuffer(gl.FRAMEBUFFER, t.fb);
    gl.viewport(0, 0, t.w, t.h);
    gl.scissor(x0, y0, x1 - x0, y1 - y0);
  }
  function post(prog, src, dst, x, y, w, h, margin, dx, dy) {
    region(dst, x, y, w, h, margin);
    gl.useProgram(prog.p);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, src.tex);
    gl.uniform1i(prog.u.uSrc, 0);
    gl.uniform2f(prog.u.uDst, dst.w, dst.h);
    if (prog.u.uDir) gl.uniform2f(prog.u.uDir, dx, dy);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  // towers (depth pre-pass, then shaded), ground, sky; into whatever is bound, at its viewport
  function scenePasses(t) {
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LESS);
    gl.enable(gl.CULL_FACE);
    gl.bindVertexArray(vao);
    // depth pre-pass: at street level towers hide each other, so shade each pixel once
    gl.colorMask(false, false, false, false);
    gl.useProgram(towerZ.p);
    gl.uniformMatrix4fv(towerZ.u.uVP, false, vp);
    gl.uniform3fv(towerZ.u.uEye, eye);
    gl.drawElementsInstanced(gl.TRIANGLES, box.i.length, gl.UNSIGNED_SHORT, 0, n);
    gl.colorMask(true, true, true, true);
    gl.depthFunc(gl.LEQUAL);
    gl.depthMask(false);
    gl.useProgram(tower.p);
    gl.uniformMatrix4fv(tower.u.uVP, false, vp);
    gl.uniform3fv(tower.u.uEye, eye);
    gl.uniform1f(tower.u.uT, t);
    gl.uniform3fv(tower.u.uMoon, moon);
    gl.uniform1f(tower.u.uDetail, quality > 1 ? 1.6 : 1);
    gl.drawElementsInstanced(gl.TRIANGLES, box.i.length, gl.UNSIGNED_SHORT, 0, n);
    gl.disable(gl.CULL_FACE);
    gl.depthMask(true);
    gl.depthFunc(gl.LESS);

    gl.bindVertexArray(emptyVao);
    gl.useProgram(ground.p);
    gl.uniformMatrix4fv(ground.u.uVP, false, vp);
    gl.uniform3fv(ground.u.uEye, eye);
    gl.uniform1f(ground.u.uT, t);
    gl.uniform3fv(ground.u.uMoon, moon);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    gl.depthFunc(gl.LEQUAL);
    gl.depthMask(false);
    gl.useProgram(sky.p);
    gl.uniform3fv(sky.u.uF, f); gl.uniform3fv(sky.u.uR, r); gl.uniform3fv(sky.u.uU, u);
    gl.uniform2f(sky.u.uTan, tanNow * aspNow, tanNow);
    gl.uniform1f(sky.u.uT, t);
    gl.uniform3fv(sky.u.uMoon, moon);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.depthMask(true);
    gl.depthFunc(gl.LESS);
  }

  function draw(t, x, y, w, h, clipOn) {
    if (dirty) {
      gl.bindBuffer(gl.ARRAY_BUFFER, db);
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, dyn);
      gl.bindBuffer(gl.ARRAY_BUFFER, null);
      dirty = false;
    }
    gl.enable(gl.SCISSOR_TEST);
    // 1. scene → full-res target (alpha = emissive)
    const M = 12;   // margin in full-res px, cleared so blurs read black
    region(scene, x, y, w, h, M);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.viewport(x, y, w, h);
    gl.scissor(x, y, w, h);
    scenePasses(t);

    gl.disable(gl.DEPTH_TEST);

    // 2. bloom: extract + blur at half res, then again at quarter res for the wide glow
    post(extract, scene, h1, x, y, w, h, 6, 0, 0);
    post(blur, h1, h2, x, y, w, h, 6, 1 / h1.w, 0);
    post(blur, h2, h1, x, y, w, h, 6, 0, 1 / h2.h);
    const wide = quality === 0;
    if (wide) {   // quarter res: downsample while blurring horizontally, then vertically
      post(blur, h1, q1, x, y, w, h, 4, 2 / h1.w, 0);
      post(blur, q1, q2, x, y, w, h, 4, 0, 1 / q1.h);
    }

    // 3. composite into the caller's framebuffer
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, W, H);
    gl.scissor(x, y, w, h);
    gl.useProgram(comp.p);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, scene.tex);
    gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, h1.tex);
    gl.activeTexture(gl.TEXTURE2); gl.bindTexture(gl.TEXTURE_2D, q2.tex);
    gl.uniform1i(comp.u.uScene, 0); gl.uniform1i(comp.u.uB1, 1); gl.uniform1i(comp.u.uB2, 2);
    gl.uniform2f(comp.u.uDst, W, H);
    if (!clipOn) clip[2] = 0;
    gl.uniform3fv(comp.u.uClip, clip);
    rect[0] = x; rect[1] = y; rect[2] = w; rect[3] = h;
    gl.uniform4fv(comp.u.uRect, rect);
    gl.uniform1f(comp.u.uB2On, wide ? 1 : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    // restore the contract defaults
    gl.activeTexture(gl.TEXTURE2); gl.bindTexture(gl.TEXTURE_2D, null);
    gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, null);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, null);
    gl.disable(gl.SCISSOR_TEST);
    gl.bindVertexArray(null);
    gl.useProgram(null);
  }

  // ---- the hall's world interface (hall.js): the hall owns the camera, stencil and post
  let hx = NaN, hz = 0;
  function update(t, dt, pos, active) {
    const moved = active && !Number.isNaN(hx) ? Math.min(5, Math.hypot(pos[0] - hx, pos[2] - hz)) : 0;
    hx = active ? pos[0] : NaN; hz = pos[2];
    eye[0] = pos[0]; eye[1] = pos[1]; eye[2] = pos[2];
    pump();
    triggers(dt, moved);
  }
  function use(kind, pos, fw) {
    let dd = 60;
    if (fw[1] < -0.01) dd = Math.min(110, pos[1] / -fw[1]);
    wave(pos[0] + fw[0] * dd, pos[2] + fw[2] * dd);
  }
  function hallDraw(cam) {
    for (let i = 0; i < 3; i++) { eye[i] = cam.eye[i]; f[i] = cam.f[i]; r[i] = cam.r[i]; u[i] = cam.u[i]; }
    yawNow = Math.atan2(-f[0], -f[2]);
    matrices(cam.aspect, cam.tanH);
    if (dirty) { gl.bindBuffer(gl.ARRAY_BUFFER, db); gl.bufferSubData(gl.ARRAY_BUFFER, 0, dyn); gl.bindBuffer(gl.ARRAY_BUFFER, null); dirty = false; }
    const cull = gl.isEnabled(gl.CULL_FACE), depth = gl.isEnabled(gl.DEPTH_TEST);
    scenePasses(cam.time);
    if (!cull) gl.disable(gl.CULL_FACE);
    if (!depth) gl.disable(gl.DEPTH_TEST);
    gl.bindVertexArray(null);
    gl.useProgram(null);
  }

  const world = {
    id: 'city',
    name: 'Kinetic city',
    feature: 'timeline() · stagger() · 3600 keyframed towers',
    hint: 'Walk the avenues · click sends a wave',
    fly: false, walk: true, radius: 600,
    fog: [0.17, 0.14, 0.25],
    spawnYaw: 0,
    start() {}, update, use, draw: hallDraw,
    tint: [1.0, 0.66, 0.32],
    spawn: [0, 4.3, 0, 0],    // eye a little above street level

    resize(cssW, cssH, d) {
      dpr = Math.min(d || 1, 2);
      const w = gl.drawingBufferWidth || Math.round(cssW * dpr), h = gl.drawingBufferHeight || Math.round(cssH * dpr);
      if (w !== W || h !== H) { W = w; H = h; alloc(W, H); }
    },

    enter() { walked = 0; lastX = NaN; },
    exit() {},

    // rect and portal: device px, top-left origin (CONTRACT.md)
    preview(t, dt, rc, portal) {
      // glide down the avenue above the street, swaying a little
      const z = -((t * 2.2) % TILE);
      yawNow = Math.sin(t * 0.09) * 0.25;
      const x = Math.floor(rc.x), w = Math.ceil(rc.w), h = Math.ceil(rc.h), y = H - Math.floor(rc.y) - h;
      camera(Math.sin(t * 0.05) * 1.2, 7.5, z, yawNow, -0.1, w / h);
      pump();
      triggers(dt, 0);
      clip[0] = portal.x; clip[1] = H - portal.y; clip[2] = portal.r;
      draw(t, x, y, w, h, true);
    },

    frame(t, dt, input) {
      const p = input.pos;
      yawNow = input.yaw;
      camera(p[0], p[1], p[2], input.yaw, input.pitch, W / H);
      const moved = Number.isNaN(lastX) ? 0 : Math.min(5, Math.hypot(p[0] - lastX, p[2] - lastZ));
      lastX = p[0]; lastZ = p[2];
      if (input.click) {
        // where you look on the ground, or 60 m out along the horizon
        let dd = 60;
        if (f[1] < -0.01) dd = Math.min(110, eye[1] / -f[1]);
        wave(eye[0] + f[0] * dd, eye[2] + f[2] * dd);
      }
      pump();
      triggers(dt, moved);
      draw(t, 0, 0, W, H, false);
    },

    setQuality(level) { quality = level | 0; },
    get _debug() { return { moving: active.length, waves: waves.length }; },
  };

  // pre-warm: run every pass once into a 1×1 scissor, and one choreography so batch() is warm
  camera(0, 1.7, 0, 0, 0, 1);
  rearrange(14, 0, 0);
  pump();
  draw(0, 0, 0, 1, 1, false);
  return world;
}
