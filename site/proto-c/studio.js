// A small physically based studio renderer: one WebGL2 context, HDR target,
// analytic softbox environment, top-down contact shadows, half-res bloom.
// Stand-in for anikit's scene(): the parts, materials and passes map 1:1.

import { VERTEX_STRIDE } from './geometry.js';

const COMMON = `#version 300 es
precision highp float;
`;

// Studio environment: warm white sweep + three softboxes. `rough` widens and dims them.
const ENV = `
vec3 box(vec3 d, vec3 c, vec3 u, vec3 v, vec2 hs, float soft, float k) {
  float cd = dot(d, c);
  if (cd <= 0.0) return vec3(0.0);
  vec3 p = d / cd;
  float x = abs(dot(p, u)), y = abs(dot(p, v));
  float w = 0.015 + soft;
  float m = (1.0 - smoothstep(hs.x - w, hs.x + w, x)) * (1.0 - smoothstep(hs.y - w, hs.y + w, y));
  return vec3(k * m / (1.0 + soft * 6.0));
}
uniform vec3 uSky;   // wall colour
uniform vec3 uFloor; // floor colour
uniform float uEnvBoost;
vec3 env(vec3 d, float r) {
  float soft = r * r * 1.6;
  vec3 base = mix(uFloor * 0.75, uSky, smoothstep(-0.35 - soft, 0.25 + soft, d.y));
  base += uSky * 0.15 * smoothstep(0.3, 1.0, d.y);
  vec3 L = vec3(0.0);
  L += box(d, normalize(vec3(-0.55, 0.62, 0.56)), normalize(vec3(0.71, 0.0, 0.7)), normalize(vec3(0.3, 0.88, -0.36)), vec2(0.62, 0.42), soft, 9.0);
  L += box(d, normalize(vec3(0.75, 0.35, 0.56)), normalize(vec3(-0.6, 0.0, 0.8)), normalize(vec3(-0.2, 0.94, -0.26)), vec2(0.18, 0.75), soft, 5.0);
  L += box(d, vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), vec2(0.9, 0.22), soft, 4.0);
  L += box(d, normalize(vec3(0.1, 0.25, -1.0)), vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec2(0.5, 0.3), soft, 2.4);
  return (base + L * vec3(1.0, 0.985, 0.96)) * uEnvBoost;
}
`;

const OBJ_VS = COMMON + `
layout(location=0) in vec3 aPos;
layout(location=1) in vec3 aNrm;
layout(location=2) in vec4 aTan;
layout(location=3) in vec4 aInst; // x, z, delay, phase
uniform mat4 uVP;
uniform mat3 uRot;     // part rotation (explode spin)
uniform vec3 uPivot;   // part centre
uniform vec3 uOffset;  // explode offset
uniform float uTime, uAppear, uWave, uFloorY, uLift;
out vec3 vPos; out vec3 vNrm; out vec3 vTan; out float vBrush; out float vScale;
void main() {
  vec3 p = uRot * (aPos - uPivot) + uPivot + uOffset;
  vec3 n = uRot * aNrm;
  vec3 t = uRot * aTan.xyz;
  // Instance transform (identity for the hero).
  float d = length(aInst.xy);
  float s = clamp((uAppear - aInst.z) / 0.35, 0.0, 1.0);
  s = aInst.z <= 0.0 ? 1.0 : s * s * (3.0 - 2.0 * s) * (1.0 + 0.25 * sin(3.14159 * s));
  float ph = d * 0.055 - uTime * 2.1 + aInst.w;
  float h = uWave * (sin(ph) * 3.4 + 2.0) * step(0.001, d);
  float yaw = uWave * (0.6 * sin(ph * 0.5 + aInst.w) + d * 0.004) * step(0.001, d);
  float pitch = uWave * 0.32 * cos(ph) * step(0.001, d);
  float cy = cos(yaw), sy = sin(yaw), cp = cos(pitch), sp = sin(pitch);
  mat3 R = mat3(cy, 0.0, -sy, 0.0, 1.0, 0.0, sy, 0.0, cy) * mat3(1.0, 0.0, 0.0, 0.0, cp, sp, 0.0, -sp, cp);
  vec3 lift = vec3(0.0, uLift - uFloorY, 0.0);
  p = R * (p - lift) * s + lift + vec3(aInst.x, h, aInst.y);
  vPos = p; vNrm = R * n; vTan = R * t; vBrush = aTan.w; vScale = s;
  gl_Position = uVP * vec4(p, 1.0);
}`;

const OBJ_FS = COMMON + ENV + `
in vec3 vPos; in vec3 vNrm; in vec3 vTan; in float vBrush; in float vScale;
uniform vec3 uEye;
uniform vec3 uBase;
uniform vec4 uMat; // metal, rough, brushed, glass
uniform float uHi;  // hover/selection highlight
out vec4 o;
float hash(float x) { return fract(sin(x * 91.3458) * 47453.5453); }
vec3 envBRDF(vec3 F0, float r, float nv) {
  // Karis' analytic approximation of the split-sum BRDF.
  vec4 c0 = vec4(-1.0, -0.0275, -0.572, 0.022), c1 = vec4(1.0, 0.0425, 1.04, -0.04);
  vec4 q = r * c0 + c1;
  float a = min(q.x * q.x, exp2(-9.28 * nv)) * q.x + q.y;
  vec2 ab = vec2(-1.04, 1.04) * a + q.zw;
  return F0 * ab.x + ab.y;
}
void main() {
  vec3 N = normalize(vNrm);
  vec3 V = normalize(uEye - vPos);
  if (dot(N, V) < 0.0) N = -N;
  float metal = uMat.x, rough = uMat.y, brushed = uMat.z, glass = uMat.w;
  vec3 base = uBase;
  // Brushed metal: fine lines along the brush coordinate and stretched highlights.
  if (brushed > 0.0) {
    float line = hash(floor(vBrush * 260.0)) * 0.5 + hash(floor(vBrush * 61.0)) * 0.5;
    base *= 1.0 - brushed * 0.07 * line;
    vec3 T = normalize(vTan - N * dot(vTan, N));
    vec3 B = cross(N, T);
    vec3 an = normalize(cross(cross(B, V), B));
    N = normalize(mix(N, an, brushed * 0.55 * rough * 2.0));
    rough = mix(rough, rough * (0.8 + 0.4 * line), brushed);
  }
  float nv = max(dot(N, V), 1e-3);
  vec3 R = reflect(-V, N);
  vec3 F0 = mix(vec3(0.04), base, metal);
  vec3 spec = env(R, rough) * envBRDF(F0, rough, nv);
  // Diffuse irradiance: the environment seen through a very rough lobe around N.
  vec3 irr = env(N, 0.95) * 0.55 + env(normalize(N + vec3(0.0, 0.6, 0.0)), 1.0) * 0.25;
  vec3 diff = base * (1.0 - metal) * irr;
  // Soft occlusion towards the floor and inside folds.
  float ao = 0.72 + 0.28 * smoothstep(-1.0, 0.6, N.y);
  vec3 col = diff * ao + spec * mix(0.85, 1.0, ao);
  if (glass > 0.0) {
    // Frosted glass: blurred light through it, tinted, plus a sharp clear coat.
    vec3 thru = env(normalize(-V + N * 0.35), 0.75) * base * 0.92;
    float fr = 0.04 + 0.96 * pow(1.0 - nv, 5.0);
    vec3 coat = env(R, 0.04) * fr;
    vec3 inner = base * 0.35 * smoothstep(0.2, 1.0, nv);
    col = mix(col, thru * (1.0 - fr) + coat + inner, glass);
  }
  col += uHi * vec3(0.06, 0.05, 0.04) * (1.0 - nv);
  o = vec4(col, 1.0);
}`;

// Full-screen studio sweep: floor with contact shadow fading into the wall.
const BG_VS = COMMON + `
out vec2 vUv;
void main() { vec2 p = vec2(gl_VertexID == 1 ? 3.0 : -1.0, gl_VertexID == 2 ? 3.0 : -1.0); vUv = p * 0.5 + 0.5; gl_Position = vec4(p, 0.0, 1.0); }`;

const BG_FS = COMMON + `
in vec2 vUv;
uniform mat4 uInvVP;
uniform vec3 uEye, uSky, uFloor;
uniform float uFloorY, uEnvBoost;
uniform sampler2D uShadow;
uniform vec4 uShadowRect; // x0, z0, 1/w, 1/d
out vec4 o;
void main() {
  vec4 a = uInvVP * vec4(vUv * 2.0 - 1.0, 1.0, 1.0);
  vec3 d = normalize(a.xyz / a.w - uEye);
  // Wall gradient: lighter towards the top, a gentle vignette sideways.
  vec3 wall = mix(uSky * 0.93, uSky * 1.03, smoothstep(-0.2, 0.6, d.y));
  vec3 col = wall;
  if (d.y < 0.0) {
    float t = (uFloorY - uEye.y) / d.y;
    vec3 p = uEye + d * t;
    vec2 suv = (p.xz - uShadowRect.xy) * uShadowRect.zw;
    float sh = 0.0;
    if (all(greaterThan(suv, vec2(0.0))) && all(lessThan(suv, vec2(1.0)))) sh = texture(uShadow, suv).r;
    float pool = exp(-dot(p.xz, p.xz) * 0.00045);
    vec3 fl = uFloor * (0.9 + 0.12 * pool) * (1.0 - 0.62 * sh);
    // The floor bends into the wall: blend by distance, no horizon line.
    float k = smoothstep(40.0, 260.0, t) ;
    col = mix(fl, wall, k);
  }
  o = vec4(col * uEnvBoost, 1.0);
}`;

// Top-down occupancy for contact shadows: darker where the part is close to the floor.
const SH_FS = COMMON + `
in vec3 vPos; in vec3 vNrm; in vec3 vTan; in float vBrush; in float vScale;
uniform float uFloorY;
out vec4 o;
void main() { float h = max(vPos.y - uFloorY, 0.0); o = vec4(exp(-h * 0.16) * vScale, 0.0, 0.0, 1.0); }`;

const PICK_FS = COMMON + `
in vec3 vPos; in vec3 vNrm; in vec3 vTan; in float vBrush; in float vScale;
uniform float uId;
out vec4 o;
void main() { o = vec4(uId / 255.0, 0.0, 0.0, 1.0); }`;

const BLUR_FS = COMMON + `
in vec2 vUv;
uniform sampler2D uTex;
uniform vec2 uDir;
out vec4 o;
void main() {
  vec4 s = texture(uTex, vUv) * 0.2270270270;
  s += (texture(uTex, vUv + uDir * 1.3846153846) + texture(uTex, vUv - uDir * 1.3846153846)) * 0.3162162162;
  s += (texture(uTex, vUv + uDir * 3.2307692308) + texture(uTex, vUv - uDir * 3.2307692308)) * 0.0702702703;
  o = s;
}`;

const BRIGHT_FS = COMMON + `
in vec2 vUv;
uniform sampler2D uTex;
uniform vec2 uTexel;
out vec4 o;
void main() {
  vec3 c = texture(uTex, vUv + uTexel * vec2(-0.5, -0.5)).rgb + texture(uTex, vUv + uTexel * vec2(0.5, -0.5)).rgb
         + texture(uTex, vUv + uTexel * vec2(-0.5, 0.5)).rgb + texture(uTex, vUv + uTexel * vec2(0.5, 0.5)).rgb;
  c *= 0.25;
  float l = max(c.r, max(c.g, c.b));
  o = vec4(c * smoothstep(1.25, 3.0, l) * 0.5, 1.0);
}`;

const COMP_FS = COMMON + `
in vec2 vUv;
uniform sampler2D uTex, uBloom;
uniform float uBloomK, uFade;
uniform vec3 uPaper;
out vec4 o;
// Khronos PBR Neutral: keeps product colours faithful while rolling off highlights.
vec3 neutral(vec3 c) {
  float x = min(c.r, min(c.g, c.b));
  float off = x < 0.08 ? x - 6.25 * x * x : 0.04;
  c -= off;
  float peak = max(c.r, max(c.g, c.b));
  if (peak < 0.8) return c;
  float d = 0.2, np = 1.0 - d * d / (peak + d - 0.8);
  c *= np / peak;
  float g = 1.0 - 1.0 / (0.15 * (peak - np) + 1.0);
  return mix(c, np * vec3(1.0), g);
}
void main() {
  vec3 c = texture(uTex, vUv).rgb + texture(uBloom, vUv).rgb * uBloomK;
  c = neutral(c);
  vec2 q = vUv - 0.5;
  c *= 1.0 - 0.06 * dot(q, q) * 2.0;
  c = pow(c, vec3(1.0 / 2.2));
  c += (fract(sin(dot(gl_FragCoord.xy, vec2(12.9898, 78.233))) * 43758.5453) - 0.5) / 255.0;
  o = vec4(mix(uPaper, c, uFade), 1.0);
}`;

function compile(gl, vs, fs) {
  const p = gl.createProgram();
  for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) + '\n' + src.split('\n').map((l, i) => i + 1 + ': ' + l).join('\n'));
    gl.attachShader(p, s);
  }
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) { const info = gl.getActiveUniform(p, i); u[info.name] = gl.getUniformLocation(p, info.name); }
  return { p, u };
}

export class Studio {
  constructor(canvas) {
    this.canvas = canvas;
    const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, powerPreference: 'high-performance', preserveDrawingBuffer: false });
    if (!gl) throw new Error('WebGL2 unavailable');
    this.gl = gl;
    this.float = !!gl.getExtension('EXT_color_buffer_float') || !!gl.getExtension('EXT_color_buffer_half_float');
    gl.getExtension('OES_texture_float_linear');
    this.progs = {
      obj: compile(gl, OBJ_VS, OBJ_FS),
      shadow: compile(gl, OBJ_VS, SH_FS),
      pick: compile(gl, OBJ_VS, PICK_FS),
      bg: compile(gl, BG_VS, BG_FS),
      blur: compile(gl, BG_VS, BLUR_FS),
      bright: compile(gl, BG_VS, BRIGHT_FS),
      comp: compile(gl, BG_VS, COMP_FS),
    };
    this.vao = gl.createVertexArray(); // empty VAO for full-screen triangles
    this.meshes = [];
    this.instBuf = null;
    this.quality = 3; // 3: bloom + MSAA, 2: bloom, 1: none, 0: lower resolution
    this.resScale = 1;
    this.w = this.h = 0;
    this.shadowRes = 256;
    this.shadow = [this.target(this.shadowRes, this.shadowRes, gl.R8, gl.RED, gl.UNSIGNED_BYTE), this.target(this.shadowRes, this.shadowRes, gl.R8, gl.RED, gl.UNSIGNED_BYTE)];
  }

  /** Upload a part mesh; `inst` is a Float32Array of (x, z, delay, phase) per instance. */
  addMesh(m, inst) {
    const gl = this.gl;
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const vb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vb);
    gl.bufferData(gl.ARRAY_BUFFER, m.v.subarray(0, m.nv * VERTEX_STRIDE), gl.STATIC_DRAW);
    const S = VERTEX_STRIDE * 4;
    gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, S, 0);
    gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 3, gl.FLOAT, false, S, 12);
    gl.enableVertexAttribArray(2); gl.vertexAttribPointer(2, 4, gl.FLOAT, false, S, 24);
    if (!this.instBuf) {
      this.instBuf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, this.instBuf);
      gl.bufferData(gl.ARRAY_BUFFER, inst, gl.STATIC_DRAW);
    }
    gl.bindBuffer(gl.ARRAY_BUFFER, this.instBuf);
    gl.enableVertexAttribArray(3); gl.vertexAttribPointer(3, 4, gl.FLOAT, false, 16, 0);
    gl.vertexAttribDivisor(3, 1);
    const ib = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, m.i.subarray(0, m.ni), gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    const h = { vao, count: m.ni };
    this.meshes.push(h);
    return h;
  }

  target(w, h, internal, format, type, depth = false) {
    const gl = this.gl;
    const tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, internal, w, h, 0, format, type, null);
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
    }
    return { tex, fb, rb, w, h };
  }

  free(t) {
    if (!t) return;
    const gl = this.gl;
    gl.deleteTexture(t.tex); gl.deleteFramebuffer(t.fb); if (t.rb) gl.deleteRenderbuffer(t.rb);
  }

  /** (Re)allocate the HDR, MSAA, bloom and pick targets for this canvas size. */
  resize(cssW, cssH) {
    const gl = this.gl;
    const dpr = Math.min(2, window.devicePixelRatio || 1) * this.resScale;
    const w = Math.max(1, Math.round(cssW * dpr)), h = Math.max(1, Math.round(cssH * dpr));
    const msaa = this.quality >= 3;
    if (w === this.w && h === this.h && msaa === this.msaaOn) return;
    this.w = w; this.h = h; this.msaaOn = msaa;
    this.canvas.width = w; this.canvas.height = h;
    const [ifmt, type] = this.float ? [gl.RGBA16F, gl.HALF_FLOAT] : [gl.RGBA8, gl.UNSIGNED_BYTE];
    this.free(this.hdr); this.free(this.bloomA); this.free(this.bloomB); this.free(this.pickT);
    this.hdr = this.target(w, h, ifmt, gl.RGBA, type, !msaa);
    const bw = Math.max(1, w >> 1), bh = Math.max(1, h >> 1);
    this.bloomA = this.target(bw, bh, ifmt, gl.RGBA, type);
    this.bloomB = this.target(bw, bh, ifmt, gl.RGBA, type);
    this.pickT = this.target(Math.max(1, w >> 2), Math.max(1, h >> 2), gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE, true);
    if (this.msFb) { gl.deleteFramebuffer(this.msFb); gl.deleteRenderbuffer(this.msColor); gl.deleteRenderbuffer(this.msDepth); this.msFb = null; }
    if (msaa) {
      const samples = Math.min(4, gl.getParameter(gl.MAX_SAMPLES));
      this.msFb = gl.createFramebuffer();
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.msFb);
      this.msColor = gl.createRenderbuffer();
      gl.bindRenderbuffer(gl.RENDERBUFFER, this.msColor);
      gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, ifmt, w, h);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, this.msColor);
      this.msDepth = gl.createRenderbuffer();
      gl.bindRenderbuffer(gl.RENDERBUFFER, this.msDepth);
      gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, gl.DEPTH_COMPONENT24, w, h);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, this.msDepth);
    }
  }

  setPartUniforms(u, part) {
    const gl = this.gl;
    gl.uniformMatrix3fv(u.uRot, false, part.rot);
    gl.uniform3fv(u.uPivot, part.centre);
    gl.uniform3fv(u.uOffset, part.offset);
  }

  setFrameUniforms(u, f) {
    const gl = this.gl;
    gl.uniformMatrix4fv(u.uVP, false, f.vp);
    gl.uniform1f(u.uTime, f.time);
    gl.uniform1f(u.uAppear, f.appear);
    gl.uniform1f(u.uWave, f.wave);
    gl.uniform1f(u.uFloorY, f.floorY);
    if (u.uLift) gl.uniform1f(u.uLift, f.floorY);
  }

  drawParts(prog, f, parts, isPick) {
    const gl = this.gl, u = prog.u;
    this.setFrameUniforms(u, f);
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      this.setPartUniforms(u, part);
      if (prog === this.progs.obj) {
        gl.uniform3fv(u.uBase, part.mat.base);
        gl.uniform4fv(u.uMat, part.mat.params);
        gl.uniform1f(u.uHi, part.hi);
      }
      if (isPick) gl.uniform1f(u.uId, i + 1);
      const mesh = f.instances > 1 ? part.lod : part.gpu;
      gl.bindVertexArray(mesh.vao);
      gl.drawElementsInstanced(gl.TRIANGLES, mesh.count, gl.UNSIGNED_INT, 0, f.instances);
    }
    gl.bindVertexArray(null);
  }

  fullscreen() {
    this.gl.bindVertexArray(this.vao);
    this.gl.drawArrays(this.gl.TRIANGLES, 0, 3);
  }

  /** Render one frame. `f` is a reused frame-state object (no allocation here). */
  render(f, parts) {
    const gl = this.gl, P = this.progs;
    gl.disable(gl.BLEND);
    gl.disable(gl.CULL_FACE);

    // 1. Contact shadow: top-down occupancy, max-blended, then blurred twice.
    const S = this.shadow;
    gl.bindFramebuffer(gl.FRAMEBUFFER, S[0].fb);
    gl.viewport(0, 0, this.shadowRes, this.shadowRes);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendEquation(gl.MAX);
    gl.useProgram(P.shadow.p);
    gl.uniform1f(P.shadow.u.uFloorY, f.floorY);
    const vpSave = f.vp;
    f.vp = f.shadowVP;
    this.drawParts(P.shadow, f, parts, false);
    f.vp = vpSave;
    gl.disable(gl.BLEND);
    gl.blendEquation(gl.FUNC_ADD);
    gl.useProgram(P.blur.p);
    gl.uniform1i(P.blur.u.uTex, 0);
    gl.activeTexture(gl.TEXTURE0);
    for (let k = 0; k < 2; k++) {
      const r = k === 0 ? 2.2 : 1.1;
      gl.bindFramebuffer(gl.FRAMEBUFFER, S[1].fb);
      gl.bindTexture(gl.TEXTURE_2D, S[0].tex);
      gl.uniform2f(P.blur.u.uDir, r / this.shadowRes, 0);
      this.fullscreen();
      gl.bindFramebuffer(gl.FRAMEBUFFER, S[0].fb);
      gl.bindTexture(gl.TEXTURE_2D, S[1].tex);
      gl.uniform2f(P.blur.u.uDir, 0, r / this.shadowRes);
      this.fullscreen();
    }

    // 2. Scene into the HDR target (multisampled when quality allows).
    const ms = this.msaaOn && this.msFb;
    gl.bindFramebuffer(gl.FRAMEBUFFER, ms ? this.msFb : this.hdr.fb);
    gl.viewport(0, 0, this.w, this.h);
    gl.clear(gl.DEPTH_BUFFER_BIT);
    gl.useProgram(P.bg.p);
    const bu = P.bg.u;
    gl.uniformMatrix4fv(bu.uInvVP, false, f.invVP);
    gl.uniform3fv(bu.uEye, f.eye);
    gl.uniform3fv(bu.uSky, f.sky);
    gl.uniform3fv(bu.uFloor, f.floor);
    gl.uniform1f(bu.uFloorY, f.floorY);
    gl.uniform1f(bu.uEnvBoost, f.envBoost);
    gl.uniform4fv(bu.uShadowRect, f.shadowRect);
    gl.uniform1i(bu.uShadow, 0);
    gl.bindTexture(gl.TEXTURE_2D, S[0].tex);
    this.fullscreen();
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LESS);
    gl.useProgram(P.obj.p);
    const ou = P.obj.u;
    gl.uniform3fv(ou.uEye, f.eye);
    gl.uniform3fv(ou.uSky, f.sky);
    gl.uniform3fv(ou.uFloor, f.floor);
    gl.uniform1f(ou.uEnvBoost, f.envBoost);
    this.drawParts(P.obj, f, parts, false);
    gl.disable(gl.DEPTH_TEST);
    if (ms) {
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.msFb);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this.hdr.fb);
      gl.blitFramebuffer(0, 0, this.w, this.h, 0, 0, this.w, this.h, gl.COLOR_BUFFER_BIT, gl.NEAREST);
    }

    // 3. Bloom at half resolution.
    const bloom = this.quality >= 2;
    if (bloom) {
      const A = this.bloomA, B = this.bloomB;
      gl.viewport(0, 0, A.w, A.h);
      gl.bindFramebuffer(gl.FRAMEBUFFER, A.fb);
      gl.useProgram(P.bright.p);
      gl.uniform1i(P.bright.u.uTex, 0);
      gl.uniform2f(P.bright.u.uTexel, 1 / this.w, 1 / this.h);
      gl.bindTexture(gl.TEXTURE_2D, this.hdr.tex);
      this.fullscreen();
      gl.useProgram(P.blur.p);
      for (let k = 0; k < 2; k++) {
        const r = k === 0 ? 1.6 : 3.2;
        gl.bindFramebuffer(gl.FRAMEBUFFER, B.fb);
        gl.bindTexture(gl.TEXTURE_2D, A.tex);
        gl.uniform2f(P.blur.u.uDir, r / A.w, 0);
        this.fullscreen();
        gl.bindFramebuffer(gl.FRAMEBUFFER, A.fb);
        gl.bindTexture(gl.TEXTURE_2D, B.tex);
        gl.uniform2f(P.blur.u.uDir, 0, r / A.h);
        this.fullscreen();
      }
    }

    // 4. Tone map to the screen.
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, this.w, this.h);
    gl.useProgram(P.comp.p);
    const cu = P.comp.u;
    gl.uniform1i(cu.uTex, 0);
    gl.uniform1i(cu.uBloom, 1);
    gl.uniform1f(cu.uBloomK, bloom ? 0.9 : 0);
    gl.uniform1f(cu.uFade, f.fade);
    gl.uniform3fv(cu.uPaper, f.paper);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.bloomA.tex);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.hdr.tex);
    this.fullscreen();
    gl.bindVertexArray(null);
  }

  /** Which part is under the pointer (1-based index, 0 for none). Runs on click only. */
  pick(f, parts, x, y) {
    const gl = this.gl, T = this.pickT;
    gl.bindFramebuffer(gl.FRAMEBUFFER, T.fb);
    gl.viewport(0, 0, T.w, T.h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    gl.useProgram(this.progs.pick.p);
    this.drawParts(this.progs.pick, f, parts, true);
    gl.disable(gl.DEPTH_TEST);
    const px = new Uint8Array(4);
    gl.readPixels(Math.floor(x * T.w), Math.floor((1 - y) * T.h), 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return px[0];
  }
}
