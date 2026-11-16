// hall-shaders.js — GLSL for the hall. Bright gallery: hemisphere light, window light
// projected through the left wall's high windows, warm fog, filmic tone map.

const LIGHT = /* glsl */ `
uniform vec3 uEye;
uniform vec3 uSun;     // direction the light travels
uniform vec3 uFog;
uniform float uTime;
uniform vec2 uRes;

float winMask(vec3 w) {
  // trace back along the sun to the left wall (x = -8) and test the window grid
  float t = (w.x + 8.0) / max(uSun.x, 0.05);
  vec3 h = w - uSun * t;
  float y = smoothstep(6.3, 6.9, h.y) * (1.0 - smoothstep(10.3, 10.9, h.y));
  float zz = h.z - 8.5;
  float cell = fract(zz / 5.0 + 0.5) - 0.5;          // window centred in each 5 m bay
  float x = 1.0 - smoothstep(0.17, 0.25, abs(cell));
  // mullion
  x *= smoothstep(0.004, 0.02, abs(cell));
  float inside = step(-27.0, h.z) * step(h.z, 15.0);
  return y * x * inside;
}

vec3 tone(vec3 c) {
  c *= 1.05;
  c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14);
  return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2));
}

vec3 shade(vec3 w, vec3 n, vec3 alb, float gloss, float emis, float fogDist) {
  vec3 V = normalize(uEye - w);
  float hemi = 0.5 + 0.5 * n.y;
  vec3 amb = mix(vec3(0.42, 0.36, 0.32), vec3(0.95, 0.97, 1.05), hemi);
  // soft ambient occlusion near floor and walls
  float ao = mix(0.5, 1.0, smoothstep(0.0, 1.6, w.y));
  ao *= mix(0.7, 1.0, smoothstep(8.0, 6.4, abs(w.x)));
  ao *= mix(0.75, 1.0, smoothstep(-28.0, -24.0, w.z) * smoothstep(16.0, 13.0, w.z));
  ao *= mix(0.85, 1.0, smoothstep(12.0, 9.5, w.y));
  float wm = winMask(w);
  float nd = max(dot(n, -uSun), 0.0);
  vec3 sunC = vec3(1.0, 0.78, 0.52) * 4.2;
  vec3 col = alb * (amb * ao + sunC * wm * nd);
  // bounce from lit floor patches
  col += alb * vec3(0.12, 0.08, 0.05) * (1.0 - hemi);
  vec3 H = normalize(V - uSun);
  col += gloss * (pow(max(dot(n, H), 0.0), 60.0) * wm * 2.0 + pow(max(dot(n, V), 0.0), 3.0) * 0.0) * sunC;
  // rim from the sky-lit hall
  float fr = pow(1.0 - max(dot(n, V), 0.0), 4.0);
  col += gloss * fr * vec3(0.55, 0.55, 0.6);
  col += alb * emis;
  float f = (1.0 - exp(-fogDist * 0.016)) * 0.85;
  col = mix(col, uFog, f);
  return col;
}
`;

export const MESH_VS = /* glsl */ `#version 300 es
layout(location=0) in vec3 aPos;
layout(location=1) in vec3 aNor;
layout(location=2) in vec4 iPosRot;
layout(location=3) in vec4 iScale;
layout(location=4) in vec4 iCol;
uniform mat4 uVP;
uniform float uMirror;
out vec3 vW; out vec3 vN; out vec4 vCol; out float vGloss; out vec3 vL;
void main() {
  float c = cos(iPosRot.w), s = sin(iPosRot.w);
  vec3 p = aPos * iScale.xyz;
  p = vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
  vec3 n = aNor / iScale.xyz;
  n = vec3(c * n.x + s * n.z, n.y, -s * n.x + c * n.z);
  vec3 w = p + iPosRot.xyz;
  vW = w; vN = n; vCol = iCol; vGloss = iScale.w; vL = aPos;
  gl_Position = uVP * vec4(w.x, w.y * uMirror, w.z, 1.0);
}`;

export const MESH_FS = /* glsl */ `#version 300 es
precision highp float;
${LIGHT}
uniform float uMirror;
in vec3 vW; in vec3 vN; in vec4 vCol; in float vGloss; in vec3 vL;
out vec4 o;
void main() {
  vec3 n = normalize(vN);
  vec3 alb = vCol.rgb * vCol.rgb;
  float emis = vCol.a;
  // emissive > 2 marks windows: sky gradient
  if (emis > 2.0) {
    vec3 sky = mix(vec3(1.25, 1.22, 1.15), vec3(0.75, 0.9, 1.25), smoothstep(-0.3, 0.5, vL.y));
    float q = abs(vL.z) > 0.499 ? vL.x : vL.z;
    float mull = step(0.47, abs(q)) + step(abs(q), 0.012) + step(abs(vL.y - 0.1), 0.01) + step(0.48, abs(vL.y));
    o = vec4(tone(mix(sky * 1.25, vec3(0.9, 0.82, 0.7), clamp(mull, 0.0, 1.0))), 1.0);
    return;
  }
  vec3 wm = vec3(vW.x, vW.y * uMirror, vW.z);
  vec3 col = shade(vW, n, alb, vGloss, emis, length(uEye - wm));
  if (uMirror < 0.0) col *= vec3(0.8, 0.76, 0.72);
  o = vec4(tone(col), 1.0);
}`;

// polished stone floor, blended over the mirrored scene
export const FLOOR_VS = /* glsl */ `#version 300 es
layout(location=0) in vec2 aXZ;
uniform mat4 uVP;
out vec3 vW;
void main() { vW = vec3(aXZ.x, 0.0, aXZ.y); gl_Position = uVP * vec4(vW, 1.0); }`;

export const FLOOR_FS = /* glsl */ `#version 300 es
precision highp float;
${LIGHT}
in vec3 vW;
out vec4 o;
float h(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vn(vec2 p) { vec2 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(h(i), h(i + vec2(1, 0)), f.x), mix(h(i + vec2(0, 1)), h(i + vec2(1, 1)), f.x), f.y); }
void main() {
  vec2 p = vW.xz;
  vec2 tile = floor(p / 2.0);
  float chk = mod(tile.x + tile.y, 2.0);
  vec3 a = mix(vec3(0.84, 0.78, 0.70), vec3(0.42, 0.47, 0.44), chk);
  // central runner in warm stone
  float run = 1.0 - smoothstep(2.9, 3.0, abs(p.x));
  a = mix(a, vec3(0.78, 0.62, 0.46), run * 0.55);
  float border = smoothstep(2.9, 2.95, abs(p.x)) * (1.0 - smoothstep(3.05, 3.1, abs(p.x)));
  a = mix(a, vec3(0.75, 0.6, 0.35), border);
  // veins
  float v = vn(p * 1.3 + vn(p * 0.7) * 2.0);
  a *= 0.92 + 0.12 * smoothstep(0.45, 0.55, v) * (1.0 - smoothstep(0.55, 0.65, v));
  // grout
  vec2 g = abs(fract(p / 2.0) - 0.5);
  a *= mix(0.82, 1.0, smoothstep(0.485, 0.47, max(g.x, g.y)));
  vec3 col = shade(vW, vec3(0, 1, 0), a * a, 0.0, 0.0, length(uEye - vW));
  vec3 V = normalize(uEye - vW);
  float fres = pow(1.0 - V.y, 5.0);
  float refl = clamp(0.14 + 0.32 * fres, 0.0, 0.42) * (1.0 - 0.35 * chk);
  // floor gets the scene reflection through (1 - alpha)
  o = vec4(tone(col), 1.0 - refl);
}`;

// light shafts: additive slabs from the windows
export const BEAM_VS = /* glsl */ `#version 300 es
layout(location=0) in vec3 aPos;
layout(location=1) in vec2 aUV;
uniform mat4 uVP;
out vec3 vW; out vec2 vUV;
void main() { vW = aPos; vUV = aUV; gl_Position = uVP * vec4(aPos, 1.0); }`;

export const BEAM_FS = /* glsl */ `#version 300 es
precision highp float;
uniform vec3 uEye; uniform float uTime; uniform float uBeam;
in vec3 vW; in vec2 vUV;
out vec4 o;
void main() {
  float edge = sin(3.14159 * vUV.x);
  edge *= edge;
  float along = (1.0 - vUV.y);
  float a = edge * mix(0.25, 1.0, along) * smoothstep(0.0, 0.12, vUV.y);
  float d = length(uEye - vW);
  a *= smoothstep(0.6, 4.0, d);
  vec3 N = normalize(cross(dFdx(vW), dFdy(vW)));
  a *= pow(abs(dot(N, normalize(uEye - vW))), 1.5);
  float flick = 0.85 + 0.15 * sin(uTime * 0.7 + vW.z * 0.9 + vW.y * 0.4);
  a *= flick * uBeam;
  o = vec4(vec3(1.0, 0.86, 0.62) * a, 0.0);
}`;

// morphing point sculpture
export const PTS_VS = /* glsl */ `#version 300 es
layout(location=0) in vec3 aA;
layout(location=1) in vec3 aB;
layout(location=2) in vec4 aS; // delay, size, hue, rand
uniform mat4 uVP; uniform float uMirror; uniform float uMix; uniform float uTime;
uniform vec3 uCenter; uniform float uPx; uniform vec3 uEye; uniform float uSpin;
out vec3 vC; out float vFog;
vec3 pal(float t) {
  t = clamp(t, 0.0, 1.0);
  vec3 a = vec3(0.10, 0.22, 0.95), b = vec3(0.55, 0.22, 0.95), c = vec3(1.0, 0.38, 0.32), d = vec3(1.0, 0.78, 0.30);
  return t < 0.33 ? mix(a, b, t / 0.33) : t < 0.66 ? mix(b, c, (t - 0.33) / 0.33) : mix(c, d, (t - 0.66) / 0.34);
}
void main() {
  float m = clamp(uMix * 1.7 - aS.x * 0.7, 0.0, 1.0);
  m = m * m * (3.0 - 2.0 * m);
  vec3 p = mix(aA, aB, m);
  float mid = sin(3.14159 * m);
  vec3 sw = vec3(sin(p.y * 3.1 + uTime * 1.3 + aS.w * 6.0), cos(p.x * 2.7 + uTime), sin(p.z * 3.3 - uTime * 1.1));
  p += sw * mid * 0.35;
  float a = uSpin;
  float c = cos(a), s = sin(a);
  p = vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
  vec3 w = uCenter + p;
  vec4 cp = uVP * vec4(w.x, w.y * uMirror, w.z, 1.0);
  gl_Position = cp;
  gl_PointSize = clamp(uPx * aS.y / cp.w, 1.0, 9.0);
  vC = pal(0.48 + p.y * 0.3 + aS.z * 0.25 - 0.05 + mid * 0.25) * (0.8 + 0.5 * aS.w + 0.6 * mid);
  vFog = (1.0 - exp(-length(uEye - w) * 0.016)) * 0.85;
}`;

export const PTS_FS = /* glsl */ `#version 300 es
precision highp float;
uniform vec3 uFog;
in vec3 vC; in float vFog;
out vec4 o;
vec3 tone(vec3 c) { c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14); return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2)); }
void main() {
  vec2 q = gl_PointCoord * 2.0 - 1.0;
  float r = dot(q, q);
  if (r > 1.0) discard;
  vec3 c = vC * (0.75 - 0.35 * r);
  o = vec4(tone(mix(c, uFog, vFog)), 1.0);
}`;

// dust motes in the light
export const DUST_VS = /* glsl */ `#version 300 es
layout(location=0) in vec4 aP;
uniform mat4 uVP; uniform float uTime; uniform float uPx; uniform vec3 uSun;
out float vA;
void main() {
  vec3 p = aP.xyz;
  p += vec3(sin(uTime * 0.21 + aP.w * 9.0), sin(uTime * 0.17 + aP.w * 5.0) * 0.6, cos(uTime * 0.19 + aP.w * 7.0)) * 0.6;
  // keep motes inside the shaft they were born in
  vec4 cp = uVP * vec4(p, 1.0);
  gl_Position = cp;
  gl_PointSize = clamp(uPx * 0.018 / cp.w, 1.0, 4.0);
  vA = 0.35 + 0.35 * sin(uTime * 0.9 + aP.w * 30.0);
}`;
export const DUST_FS = /* glsl */ `#version 300 es
precision highp float;
in float vA; out vec4 o;
void main() { vec2 q = gl_PointCoord * 2.0 - 1.0; float r = 1.0 - dot(q, q); if (r <= 0.0) discard;
  o = vec4(vec3(1.0, 0.9, 0.7) * r * vA, 0.0); }`;

// textured panel (benchmarks)
export const TEX_VS = /* glsl */ `#version 300 es
layout(location=0) in vec3 aPos;
layout(location=1) in vec2 aUV;
uniform mat4 uVP; uniform float uMirror;
out vec2 vUV; out vec3 vW;
void main() { vUV = aUV; vW = aPos; gl_Position = uVP * vec4(aPos.x, aPos.y * uMirror, aPos.z, 1.0); }`;
export const TEX_FS = /* glsl */ `#version 300 es
precision highp float;
uniform sampler2D uTex; uniform vec3 uEye; uniform vec3 uFog; uniform float uGlow;
in vec2 vUV; in vec3 vW; out vec4 o;
void main() {
  vec4 t = texture(uTex, vUV);
  vec3 c = t.rgb * (0.95 + 0.25 * uGlow);
  float f = (1.0 - exp(-length(uEye - vW) * 0.016)) * 0.85;
  o = vec4(mix(c, pow(uFog, vec3(1.0/2.2)) * 0.97, f), 1.0);
}`;
