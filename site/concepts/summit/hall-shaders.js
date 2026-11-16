// hall-shaders.js — GLSL for the hall.
// Materials are procedural (world-space noise, no textures to load). Lighting: warm key
// through the left windows, cool fill from the right, a light pool over each station,
// analytic AO at wall/floor/column junctions, filmic (ACES) tone map. The scene renders
// into an offscreen target; a final pass adds bloom, FXAA, a gentle grade and vignette.
//
// Instance material ids ride in iScale.w as  mat * 2 + gloss  (gloss in 0..1).
// 0 plain · 1 plaster wall · 2 fluted marble · 3 metal · 4 coffer · 5 walnut · 6 lacquer

const NOISE = /* glsl */ `
float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vn(vec2 p) { vec2 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(h21(i), h21(i + vec2(1, 0)), f.x), mix(h21(i + vec2(0, 1)), h21(i + vec2(1, 1)), f.x), f.y); }
float fbm(vec2 p) { return 0.55 * vn(p) + 0.3 * vn(p * 2.03 + 7.1) + 0.15 * vn(p * 4.1 + 3.7); }
`;

const LIGHT = /* glsl */ `
uniform vec3 uEye;
uniform vec3 uSun;     // direction the key light travels
uniform vec3 uFog;
uniform float uTime;
uniform vec4 uPool[6]; // x, z, radius, intensity
uniform vec4 uGlassL;  // backlight behind the glass: position, intensity

float winMask(vec3 w) {
  float t = (w.x + 8.0) / max(uSun.x, 0.05);
  vec3 h = w - uSun * t;
  float y = smoothstep(6.3, 6.9, h.y) * (1.0 - smoothstep(10.3, 10.9, h.y));
  float cell = fract((h.z - 8.5) / 5.0 + 0.5) - 0.5;
  float x = (1.0 - smoothstep(0.17, 0.25, abs(cell))) * smoothstep(0.004, 0.02, abs(cell));
  return y * x * step(-27.0, h.z) * step(h.z, 15.0);
}

vec3 aces(vec3 c) {
  c *= 0.9;
  c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14);
  return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2));
}

float aoAt(vec3 w) {
  float ao = mix(0.45, 1.0, smoothstep(0.0, 1.2, w.y));
  ao *= mix(0.6, 1.0, clamp(smoothstep(8.0, 6.9, abs(w.x)) + smoothstep(1.0, 3.0, w.y) * 0.4, 0.0, 1.0));
  ao *= mix(0.7, 1.0, smoothstep(12.0, 10.6, w.y));
  ao *= mix(0.7, 1.0, smoothstep(-28.0, -25.5, w.z) * smoothstep(16.0, 13.5, w.z));
  float zc = floor((w.z - 11.0) / 5.0 + 0.5) * 5.0 + 11.0;
  float dc = length(vec2(abs(w.x) - 7.0, w.z - zc));
  ao *= mix(mix(0.5, 1.0, smoothstep(0.42, 1.4, dc)), 1.0, smoothstep(0.3, 1.6, w.y));
  return clamp(ao, 0.0, 1.0);
}

vec3 env(vec3 r) {
  return mix(vec3(0.32, 0.26, 0.2), vec3(1.1, 1.05, 1.0), smoothstep(-0.3, 0.6, r.y));
}

vec3 shade(vec3 w, vec3 n, vec3 alb, float gloss, float metal, float emis, float fogDist) {
  vec3 V = normalize(uEye - w);
  float hemi = 0.5 + 0.5 * n.y;
  vec3 amb = mix(vec3(0.22, 0.19, 0.17), vec3(0.53, 0.53, 0.57), hemi);
  float ao = aoAt(w);
  float wm = winMask(w);
  vec3 keyC = vec3(1.0, 0.78, 0.52) * 3.5;
  float nd = max(dot(n, -uSun), 0.0);
  vec3 fillC = vec3(0.55, 0.66, 0.9) * 0.42;
  float fd = max(dot(n, normalize(vec3(-1.0, 0.35, 0.15))), 0.0);
  vec3 lit = amb * ao + keyC * wm * nd + fillC * fd * ao;
  for (int i = 0; i < 6; i++) {
    vec2 d = w.xz - uPool[i].xy;
    float l = length(d), R = uPool[i].z;
    float f = (smoothstep(R, R * 0.6, l) * 0.75 + exp(-l * l / (R * R)) * 0.35) * uPool[i].w;
    lit += vec3(1.0, 0.84, 0.62) * f * (0.3 + 0.7 * max(n.y, 0.0)) * smoothstep(9.0, 2.0, w.y);
  }
  vec3 H = normalize(V - uSun);
  float spec = pow(max(dot(n, H), 0.0), 20.0 + 140.0 * gloss) * wm * (0.2 + 2.0 * gloss);
  float fr = pow(1.0 - max(dot(n, V), 0.0), 5.0);
  vec3 R = reflect(-V, n);
  vec3 col;
  if (metal > 1.5) {
    // glass: mostly reflection, tinted, with a hot specular glint
    float spec2 = pow(max(dot(n, H), 0.0), 220.0) * (0.4 + 3.0 * wm);
    vec3 sky = mix(vec3(0.45, 0.5, 0.58), vec3(1.05, 1.04, 1.0), smoothstep(-0.5, 0.7, R.y));
    col = alb * lit * 0.12 + sky * (0.15 + 0.85 * fr) * alb + vec3(1.0, 0.92, 0.8) * spec2 * 2.5;
    // the warm backlight: light seen through the glass, and glints off rotating faces
    vec3 Lg = uGlassL.xyz - w;
    float lg = length(Lg); Lg /= lg;
    float through = pow(max(dot(-V, Lg), 0.0), 5.0);
    float glint = pow(max(dot(reflect(-Lg, n), V), 0.0), 48.0) + pow(max(dot(reflect(Lg, n), V), 0.0), 48.0);
    col += vec3(1.0, 0.8, 0.52) * uGlassL.w * (through * 0.25 + glint * 3.5) / (1.0 + 0.05 * lg * lg);
  } else if (metal > 0.5) {
    col = alb * (lit * 0.22 + env(R) * (0.55 + 0.45 * gloss)) + alb * keyC * spec * 0.6;
  } else {
    col = alb * lit + keyC * spec * 0.25;
    col += env(R) * (0.04 + 0.5 * fr) * gloss;
  }
  col += alb * emis;
  float f = (1.0 - exp(-fogDist * 0.014)) * 0.8;
  return mix(col, uFog, f);
}
`;

export const MESH_VS = /* glsl */ `#version 300 es
layout(location=0) in vec3 aPos;
layout(location=1) in vec3 aNor;
layout(location=2) in vec4 iPosRot;
layout(location=3) in vec4 iScale;
layout(location=4) in vec4 iCol;
layout(location=5) in vec4 iRot2; // rotX, rotZ (applied Z, then X, then Y)
uniform mat4 uVP;
uniform float uMirror;
out vec3 vW; out vec3 vN; out vec4 vCol; out float vGloss; out vec3 vL; flat out int vMat;
vec3 rot(vec3 p) {
  float cz = cos(iRot2.y), sz = sin(iRot2.y);
  p = vec3(cz * p.x - sz * p.y, sz * p.x + cz * p.y, p.z);
  float cx = cos(iRot2.x), sx = sin(iRot2.x);
  p = vec3(p.x, cx * p.y - sx * p.z, sx * p.y + cx * p.z);
  float c = cos(iPosRot.w), s = sin(iPosRot.w);
  return vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}
void main() {
  vec3 p = rot(aPos * iScale.xyz);
  vec3 n = rot(aNor / iScale.xyz);
  vec3 w = p + iPosRot.xyz;
  float m = floor(iScale.w * 0.5);
  vW = w; vN = n; vCol = iCol; vGloss = iScale.w - m * 2.0; vL = aPos; vMat = int(m);
  gl_Position = uVP * vec4(w.x, w.y * uMirror, w.z, 1.0);
}`;

export const MESH_FS = /* glsl */ `#version 300 es
precision highp float;
${NOISE}
${LIGHT}
uniform float uMirror;
in vec3 vW; in vec3 vN; in vec4 vCol; in float vGloss; in vec3 vL; flat in int vMat;
out vec4 o;
void main() {
  vec3 n = normalize(vN);
  vec3 alb = vCol.rgb * vCol.rgb;
  float emis = vCol.a, gloss = vGloss, metal = 0.0;
  if (emis > 2.0) {
    vec3 sky = mix(vec3(1.5, 1.42, 1.3), vec3(0.85, 1.0, 1.35), smoothstep(-0.3, 0.5, vL.y));
    float q = abs(vL.z) > 0.499 ? vL.x : vL.z;
    float mull = step(0.47, abs(q)) + step(abs(q), 0.012) + step(abs(vL.y - 0.1), 0.01) + step(0.48, abs(vL.y));
    vec3 c = mix(sky * 1.3, vec3(0.08, 0.07, 0.06), clamp(mull, 0.0, 1.0));
    o = vec4(aces(c), 1.0);
    return;
  }
  if (vMat == 1) {
    float pn = fbm(vec2(vW.z + vW.x, vW.y) * 1.7);
    alb *= 0.86 + 0.22 * pn;
    float wain = 1.0 - smoothstep(1.32, 1.36, vW.y);
    float rail = smoothstep(1.3, 1.33, vW.y) * (1.0 - smoothstep(1.4, 1.43, vW.y));
    alb = mix(alb, vec3(0.16, 0.1, 0.07) * (0.85 + 0.3 * vn(vec2(vW.z + vW.x, vW.y * 8.0) * vec2(0.6, 1.0))), wain);
    gloss = mix(gloss, 0.45, wain);
    if (rail > 0.0) { alb = mix(alb, vec3(0.5, 0.33, 0.16), rail); metal = rail; gloss = 0.6; }
  } else if (vMat == 2) {
    float a = atan(vL.z, vL.x);
    float fl = sin(a * 24.0);
    vec3 t = vec3(-sin(a), 0.0, cos(a));
    n = normalize(n + t * fl * 0.22 * step(abs(n.y), 0.5));
    alb *= 0.92 + 0.1 * fbm(vec2(a * 3.0, vW.y * 1.4));
  } else if (vMat == 3) {
    metal = 1.0;
    alb *= 0.9 + 0.15 * vn(vW.xz * 9.0 + vW.y * 3.0);
  } else if (vMat == 4) {
    alb *= 0.85 + 0.2 * fbm(vW.xz * 0.9);
  } else if (vMat == 5) {
    float g = vn(vec2((vW.x + vW.z) * 0.5, vW.y * 14.0 + vn(vW.xz * 2.0) * 3.0));
    alb *= 0.75 + 0.45 * g;
  } else if (vMat == 7) {
    metal = 2.0;
  }
  vec3 wm = vec3(vW.x, vW.y * uMirror, vW.z);
  vec3 col = shade(vW, n, alb, gloss, metal, emis, length(uEye - wm));
  float alpha = 1.0;
  if (vMat == 7) {
    // translucent glass with bright cut edges
    vec2 e = abs(vL.xy);
    float edge = smoothstep(0.42, 0.5, max(e.x, e.y));
    float fr = pow(1.0 - abs(dot(n, normalize(uEye - vW))), 3.0);
    col += vec3(0.8, 0.92, 1.0) * edge * 0.55;
    alpha = clamp(0.12 + 0.55 * fr + 0.55 * edge, 0.0, 0.9);
  }
  o = vec4(aces(col), alpha);
}`;

export const FLOOR_VS = /* glsl */ `#version 300 es
layout(location=0) in vec2 aXZ;
uniform mat4 uVP;
out vec3 vW;
void main() { vW = vec3(aXZ.x, 0.0, aXZ.y); gl_Position = uVP * vec4(vW, 1.0); }`;

export const FLOOR_FS = /* glsl */ `#version 300 es
precision highp float;
${NOISE}
${LIGHT}
uniform vec4 uBlob[6];
uniform vec4 uRing;
uniform float uRefl;
uniform sampler2D uReflTex;
uniform vec2 uRes;
in vec3 vW;
out vec4 o;
void main() {
  vec2 p = vW.xz;
  vec2 tile = floor(p / 2.0);
  float th = h21(tile);
  float chk = mod(tile.x + tile.y, 2.0);
  vec3 a = mix(vec3(0.86, 0.79, 0.68), vec3(0.5, 0.42, 0.35), chk);
  a *= 0.93 + 0.12 * th;
  float run = 1.0 - smoothstep(2.9, 2.95, abs(p.x));
  a = mix(a, vec3(0.3, 0.38, 0.33), run * 0.85);
  float brass = smoothstep(2.9, 2.93, abs(p.x)) * (1.0 - smoothstep(3.02, 3.05, abs(p.x)));
  vec2 q = p * 0.55 + th * 13.0;
  float v = fbm(q + fbm(q * 1.7) * 1.6);
  float vein = 1.0 - smoothstep(0.0, 0.035, abs(v - 0.52));
  a = mix(a, a * (chk > 0.5 ? 1.35 : 0.62), vein * 0.75);
  a *= 0.94 + 0.12 * fbm(p * 3.1);
  vec2 g = abs(fract(p / 2.0) - 0.5);
  float grout = smoothstep(0.488, 0.494, max(g.x, g.y));
  a = mix(a, vec3(0.16, 0.13, 0.11), grout);
  a = mix(a, vec3(0.62, 0.45, 0.22), brass);
  float sh = 1.0;
  for (int i = 0; i < 6; i++) {
    float d = length(p - uBlob[i].xy);
    sh *= mix(1.0 - uBlob[i].w, 1.0, smoothstep(uBlob[i].z * 0.7, uBlob[i].z * 1.35, d));
  }
  vec3 alb = a * a * sh;
  vec3 V = normalize(uEye - vW);
  float d = length(uEye - vW);
  vec3 col = shade(vW, vec3(0, 1, 0), alb, 0.0, 0.0, 0.0, d);
  float dr = abs(length(p - uRing.xy) - uRing.z);
  col += vec3(1.0, 0.62, 0.28) * uRing.w * (exp(-dr * dr * 900.0) * 1.1 + exp(-dr * 7.0) * 0.12);
  vec3 base = aces(col);
  float fres = pow(1.0 - clamp(V.y, 0.0, 1.0), 5.0);
  float k = (0.2 + 0.5 * fres) * (1.0 - grout) * mix(1.0, 0.7, vein) * mix(1.0, 1.25, run) * uRefl;
  vec2 uv = gl_FragCoord.xy / uRes;
  vec2 nOff = (vec2(vn(p * 6.0), vn(p * 6.0 + 9.0)) - 0.5) * 0.006 + (vec2(fbm(p * 1.3), fbm(p * 1.3 + 5.0)) - 0.5) * 0.006;
  vec2 px = 1.5 / uRes * (1.0 + d * 0.04);
  vec3 r = texture(uReflTex, uv + nOff + vec2(px.x, px.y)).rgb + texture(uReflTex, uv + nOff + vec2(-px.x, px.y)).rgb
         + texture(uReflTex, uv + nOff + vec2(px.x, -px.y)).rgb + texture(uReflTex, uv + nOff + vec2(-px.x, -px.y)).rgb;
  r *= 0.25 * vec3(0.92, 0.88, 0.84);
  o = vec4(mix(base, r, k), 1.0);
}`;

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
  float a = edge * mix(0.15, 1.0, 1.0 - vUV.y) * smoothstep(0.0, 0.12, vUV.y);
  float d = length(uEye - vW);
  a *= smoothstep(1.0, 6.0, d);
  vec3 N = normalize(cross(dFdx(vW), dFdy(vW)));
  a *= pow(abs(dot(N, normalize(uEye - vW))), 2.0);
  a *= (0.85 + 0.15 * sin(uTime * 0.7 + vW.z * 0.9 + vW.y * 0.4)) * uBeam;
  o = vec4(vec3(1.0, 0.84, 0.6) * a, 0.0);
}`;

export const PTS_VS = /* glsl */ `#version 300 es
layout(location=0) in vec3 aA;
layout(location=1) in vec3 aB;
layout(location=2) in vec4 aS;
uniform mat4 uVP; uniform float uMirror; uniform float uMix; uniform float uTime;
uniform vec3 uCenter; uniform float uPx; uniform vec3 uEye; uniform float uSpin; uniform float uScale; uniform float uGlow; uniform float uBlast;
out vec3 vC; out float vFog;
vec3 pal(float t) {
  t = clamp(t, 0.0, 1.0);
  vec3 a = vec3(0.12, 0.3, 1.0), b = vec3(0.6, 0.25, 1.0), c = vec3(1.0, 0.35, 0.3), d = vec3(1.0, 0.8, 0.32);
  return t < 0.33 ? mix(a, b, t / 0.33) : t < 0.66 ? mix(b, c, (t - 0.33) / 0.33) : mix(c, d, (t - 0.66) / 0.34);
}
void main() {
  float m = clamp(uMix * 1.7 - aS.x * 0.7, 0.0, 1.0);
  m = m * m * (3.0 - 2.0 * m);
  vec3 p = mix(aA, aB, m);
  float mid = sin(3.14159 * m);
  vec3 sw = vec3(sin(p.y * 3.1 + uTime * 1.3 + aS.w * 6.0), cos(p.x * 2.7 + uTime), sin(p.z * 3.3 - uTime * 1.1));
  p += sw * mid * 0.35;
  // explosion: fling outward along the point's direction plus a turbulent kick
  vec3 dir = normalize(p + vec3(0.001, 0.002, 0.003));
  p += (dir * (0.9 + 1.6 * aS.w) + sw * 0.6) * uBlast * (0.6 + aS.x);
  p += 0.012 * vec3(sin(uTime * 1.7 + aS.w * 40.0), cos(uTime * 1.3 + aS.w * 31.0), sin(uTime * 1.1 + aS.w * 23.0));
  float c = cos(uSpin), s = sin(uSpin);
  vec3 col = pal(0.48 + p.y * 0.3 + aS.z * 0.25 - 0.05 + mid * 0.25);
  p = vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z) * uScale;
  vec3 w = uCenter + p;
  vec4 cp = uVP * vec4(w.x, w.y * uMirror, w.z, 1.0);
  gl_Position = cp;
  gl_PointSize = clamp(uPx * aS.y * uScale * 0.6 / cp.w, 1.0, 10.0);
  vC = col * (0.95 + 0.7 * aS.w * aS.w + 0.8 * mid + 1.2 * uBlast) * uGlow;
  vFog = (1.0 - exp(-length(uEye - w) * 0.014)) * 0.6;
}`;

export const PTS_FS = /* glsl */ `#version 300 es
precision highp float;
uniform vec3 uFog;
in vec3 vC; in float vFog;
out vec4 o;
vec3 aces(vec3 c) { c = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14); return pow(clamp(c, 0.0, 1.0), vec3(1.0 / 2.2)); }
void main() {
  vec2 q = gl_PointCoord * 2.0 - 1.0;
  float r = dot(q, q);
  if (r > 1.0) discard;
  vec3 c = vC * (1.25 - 0.6 * r);
  o = vec4(aces(mix(c, uFog, vFog)), 1.0);
}`;

export const DUST_VS = /* glsl */ `#version 300 es
layout(location=0) in vec4 aP;
uniform mat4 uVP; uniform float uTime; uniform float uPx;
out float vA;
void main() {
  vec3 p = aP.xyz + vec3(sin(uTime * 0.21 + aP.w * 9.0), sin(uTime * 0.17 + aP.w * 5.0) * 0.6, cos(uTime * 0.19 + aP.w * 7.0)) * 0.6;
  vec4 cp = uVP * vec4(p, 1.0);
  gl_Position = cp;
  gl_PointSize = clamp(uPx * 0.016 / cp.w, 1.0, 5.0);
  vA = 0.3 + 0.3 * sin(uTime * 0.9 + aP.w * 30.0);
}`;
export const DUST_FS = /* glsl */ `#version 300 es
precision highp float;
in float vA; out vec4 o;
void main() { vec2 q = gl_PointCoord * 2.0 - 1.0; float r = 1.0 - dot(q, q); if (r <= 0.0) discard;
  o = vec4(vec3(1.0, 0.9, 0.7) * r * vA, 0.0); }`;

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
  vec3 c = t.rgb * (0.9 + 0.2 * uGlow);
  float f = (1.0 - exp(-length(uEye - vW) * 0.014)) * 0.8;
  o = vec4(mix(c, pow(uFog, vec3(1.0 / 2.2)) * 0.97, f), 1.0);
}`;

// ---- post ----
export const POST_VS = /* glsl */ `#version 300 es
out vec2 vUV;
void main() { vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2); vUV = p; gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0); }`;

export const BRIGHT_FS = /* glsl */ `#version 300 es
precision mediump float;
uniform sampler2D uTex; uniform vec2 uTexel;
in vec2 vUV; out vec4 o;
vec3 b(vec2 uv) { vec3 c = texture(uTex, uv).rgb; float l = max(c.r, max(c.g, c.b)); return c * smoothstep(0.86, 1.0, l); }
void main() {
  vec3 c = b(vUV + uTexel * vec2(-1, -1)) + b(vUV + uTexel * vec2(1, -1)) + b(vUV + uTexel * vec2(-1, 1)) + b(vUV + uTexel * vec2(1, 1));
  o = vec4(c * 0.25, 1.0);
}`;

export const BLUR_FS = /* glsl */ `#version 300 es
precision mediump float;
uniform sampler2D uTex; uniform vec2 uDir;
in vec2 vUV; out vec4 o;
void main() {
  vec3 c = texture(uTex, vUV).rgb * 0.227;
  c += (texture(uTex, vUV + uDir * 1.385).rgb + texture(uTex, vUV - uDir * 1.385).rgb) * 0.316;
  c += (texture(uTex, vUV + uDir * 3.231).rgb + texture(uTex, vUV - uDir * 3.231).rgb) * 0.070;
  o = vec4(c, 1.0);
}`;

export const FINAL_FS = /* glsl */ `#version 300 es
precision highp float;
uniform sampler2D uTex; uniform sampler2D uBloom; uniform vec2 uTexel; uniform float uBloomK; uniform float uFxaa;
in vec2 vUV; out vec4 o;
float lum(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }
void main() {
  vec3 cM = texture(uTex, vUV).rgb;
  float lM = lum(cM);
  float lNW = lum(texture(uTex, vUV + uTexel * vec2(-1, -1)).rgb);
  float lNE = lum(texture(uTex, vUV + uTexel * vec2(1, -1)).rgb);
  float lSW = lum(texture(uTex, vUV + uTexel * vec2(-1, 1)).rgb);
  float lSE = lum(texture(uTex, vUV + uTexel * vec2(1, 1)).rgb);
  float lMin = min(lM, min(min(lNW, lNE), min(lSW, lSE)));
  float lMax = max(lM, max(max(lNW, lNE), max(lSW, lSE)));
  vec3 c = cM;
  if (uFxaa > 0.5 && lMax - lMin > max(0.04, lMax * 0.12)) {
    vec2 dir = vec2(-((lNW + lNE) - (lSW + lSE)), (lNW + lSW) - (lNE + lSE));
    float rcp = 1.0 / (min(abs(dir.x), abs(dir.y)) + max((lNW + lNE + lSW + lSE) * 0.03125, 0.0078));
    dir = clamp(dir * rcp, -8.0, 8.0) * uTexel;
    vec3 a = 0.5 * (texture(uTex, vUV + dir * (1.0 / 3.0 - 0.5)).rgb + texture(uTex, vUV + dir * (2.0 / 3.0 - 0.5)).rgb);
    vec3 b = a * 0.5 + 0.25 * (texture(uTex, vUV - dir * 0.5).rgb + texture(uTex, vUV + dir * 0.5).rgb);
    float lb = lum(b);
    c = (lb < lMin || lb > lMax) ? a : b;
  }
  if (uBloomK > 0.0) c += texture(uBloom, vUV).rgb * uBloomK;
  float l = lum(c);
  c *= mix(vec3(0.95, 0.98, 1.05), vec3(1.04, 1.0, 0.94), smoothstep(0.2, 0.8, l));
  vec2 q = vUV - 0.5;
  c *= 1.0 - 0.5 * dot(q, q);
  o = vec4(c, 1.0);
}`;
