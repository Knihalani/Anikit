// One WebGL2 context: instanced Gaussian splats, a floor shadow pass and a
// half-resolution glow pass. Stand-in until the library's splats() lands.
// Nothing here allocates per frame.

const SPLAT_VS = `#version 300 es
layout(location=0) in vec3 aPos;
layout(location=1) in float aSeed;
uniform mat4 uViewProj;
uniform mat3 uModel;
uniform vec2 uPx;          // 2 / viewport (clip units per pixel)
uniform float uFocal;      // proj[1][1] * viewportH / 2
uniform vec3 uEye;
uniform float uSize, uAlpha, uMode, uCut, uTime, uFloor, uWave, uMaxPx, uRadial;
uniform vec3 uA, uB, uC;
out vec2 vUv;
out vec4 vCol;
void main() {
  int id = gl_VertexID;
  vec2 c = vec2(float(id & 1), float(id >> 1)) * 2.0 - 1.0;
  vec3 m = aPos;
  // timeline-driven swell (wave stage)
  m.y += uWave * sin(m.x * 0.011 + uTime * 1.7) * cos(m.z * 0.009 - uTime * 1.1);
  vec3 p = uModel * m;
  float idx = float(gl_InstanceID);
  float fade = 1.0 - smoothstep(uCut - 2000.0, uCut, idx);
  float h = fract(aSeed * 13.71);
  float t = clamp(m.y / 700.0 + 0.5 + (h - 0.5) * 0.25, 0.0, 1.0);
  t = mix(t, clamp(length(m.xz) / 520.0 + (h - 0.5) * 0.3, 0.0, 1.0), uRadial);
  vec3 col = mix(uA, uB, smoothstep(0.1, 0.9, t));
  col = mix(col, uC, smoothstep(0.82, 1.0, fract(aSeed * 7.31)) * 0.85);
  float a = uAlpha;
  float size = uSize * (0.55 + 0.9 * fract(aSeed * 3.17));
  if (uMode > 0.5) {
    // shadow: flatten onto the floor, spread with height
    float hgt = max(p.y - uFloor, 0.0);
    p = vec3(p.x + hgt * 0.18, uFloor + 0.5, p.z - hgt * 0.35);
    a *= 1.0 / (1.0 + hgt * 0.004);
    size *= 1.0 + hgt * 0.012;
    col = vec3(0.42, 0.42, 0.52);
  }
  vec4 clip = uViewProj * vec4(p, 1.0);
  float dist = max(clip.w, 1.0);
  float r = size * uFocal / dist;
  // keep sub-pixel splats from shimmering: grow to 0.8px and fade instead
  float rc = max(r, 0.8);
  a *= min(1.0, (r * r) / (rc * rc));
  rc = min(rc, uMaxPx);
  if (uMode < 0.5) {
    // nearer is brighter, deeper is hazier (aerial perspective)
    float depth = clamp((dist - 1200.0) / 1400.0, 0.0, 1.0);
    col = mix(col * 1.06, vec3(0.86, 0.9, 0.97), depth * 0.45);
  }
  clip.xy += c * rc * uPx * clip.w;
  gl_Position = clip;
  vUv = c;
  vCol = vec4(col, a * fade);
}`;

const SPLAT_FS = `#version 300 es
precision mediump float;
in vec2 vUv;
in vec4 vCol;
out vec4 o;
void main() {
  float d = dot(vUv, vUv);
  float g = exp(-d * 4.0) * vCol.a;
  if (g < 0.004) discard;
  o = vec4(vCol.rgb * g, g);
}`;

const QUAD_VS = `#version 300 es
out vec2 vUv;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  vUv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const COMP_FS = `#version 300 es
precision mediump float;
in vec2 vUv;
uniform sampler2D uTex;
uniform float uGain;
uniform vec2 uTexel;
out vec4 o;
void main() {
  // 5-tap tent on the half-res buffer: soft and cheap
  vec4 s = texture(uTex, vUv) * 0.4;
  s += texture(uTex, vUv + vec2(uTexel.x, 0.0)) * 0.15;
  s += texture(uTex, vUv - vec2(uTexel.x, 0.0)) * 0.15;
  s += texture(uTex, vUv + vec2(0.0, uTexel.y)) * 0.15;
  s += texture(uTex, vUv - vec2(0.0, uTexel.y)) * 0.15;
  o = s * uGain;
}`;

// Captions as splats: one splat per cell of the caption's texture (rasterised
// by htmlCanvas into this context), drifting apart as uP goes 0 → 1.
const CAP_VS = `#version 300 es
uniform sampler2D uTex;
uniform vec4 uRect;       // x, y, w, h in device pixels (top-left origin)
uniform vec2 uView;       // viewport in device pixels
uniform vec2 uGrid;       // columns, rows
uniform float uP, uTime, uCell;
uniform vec3 uTint;
out vec2 vUv;
out vec4 vCol;
float hash(float n) { return fract(sin(n) * 43758.5453); }
void main() {
  int id = gl_VertexID;
  vec2 c = vec2(float(id & 1), float(id >> 1)) * 2.0 - 1.0;
  float i = float(gl_InstanceID);
  vec2 cell = vec2(mod(i, uGrid.x), floor(i / uGrid.x));
  vec2 uv = (cell + 0.5) / uGrid;
  vec4 t = textureLod(uTex, uv, 0.0);
  float h1 = hash(i * 1.37 + 0.1), h2 = hash(i * 2.11 + 3.7), h3 = hash(i * 0.73 + 9.1);
  // each splat leaves at its own moment, left to right with jitter
  float local = clamp(uP * 1.6 - (uv.x * 0.45 + h1 * 0.15), 0.0, 1.0);
  float e = local * local * (3.0 - 2.0 * local);
  vec2 px = uRect.xy + uv * uRect.zw;
  float a = 6.2831 * h2;
  vec2 drift = vec2(cos(a), sin(a)) * (70.0 + 260.0 * h3) + vec2(320.0, -220.0) * (0.3 + h1);
  drift += vec2(sin(uTime * 1.3 + h1 * 9.0), cos(uTime * 1.1 + h2 * 9.0)) * 18.0 * e;
  px += drift * e * uView.y / 900.0;
  float alpha = t.a * (1.0 - smoothstep(0.15, 0.85, e));
  vec3 col = t.a > 0.0 ? t.rgb / t.a : vec3(0.0);
  col = mix(col, mix(uTint, vec3(1.0), 0.25), smoothstep(0.0, 0.22, e));
  float r = uCell * (1.25 + e * 1.4);
  vec2 ndc = vec2(px.x / uView.x * 2.0 - 1.0, 1.0 - px.y / uView.y * 2.0);
  ndc += c * r * 2.0 / uView;
  gl_Position = alpha < 0.01 ? vec4(2.0, 2.0, 2.0, 1.0) : vec4(ndc, 0.0, 1.0);
  vUv = c;
  vCol = vec4(col, alpha);
}`;
const CAP_FS = `#version 300 es
precision mediump float;
in vec2 vUv;
in vec4 vCol;
out vec4 o;
void main() {
  float g = exp(-dot(vUv, vUv) * 3.0) * vCol.a;
  o = vec4(vCol.rgb * g, g);
}`;

function compile(gl, vs, fs) {
  const p = gl.createProgram();
  for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    gl.attachShader(p, s);
  }
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const name = gl.getActiveUniform(p, i).name;
    u[name] = gl.getUniformLocation(p, name);
  }
  return { p, u };
}

export class SplatRenderer {
  constructor(canvas, count) {
    const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false, depth: false, stencil: false, powerPreference: 'high-performance', desynchronized: false });
    if (!gl) throw new Error('WebGL2 unavailable');
    this.gl = gl;
    this.canvas = canvas;
    this.count = count;
    this.splat = compile(gl, SPLAT_VS, SPLAT_FS);
    this.comp = compile(gl, QUAD_VS, COMP_FS);
    this.cap = compile(gl, CAP_VS, CAP_FS);

    this.vao = gl.createVertexArray();
    gl.bindVertexArray(this.vao);
    this.posBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.posBuf);
    gl.bufferData(gl.ARRAY_BUFFER, count * 12, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    gl.vertexAttribDivisor(0, 1);
    const seeds = new Float32Array(count);
    let s = 99;
    for (let i = 0; i < count; i++) seeds[i] = (s = (s * 1664525 + 1013904223) >>> 0) / 4294967296;
    const sb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, sb);
    gl.bufferData(gl.ARRAY_BUFFER, seeds, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 1, gl.FLOAT, false, 0, 0);
    gl.vertexAttribDivisor(1, 1);
    gl.bindVertexArray(null);
    this.emptyVao = gl.createVertexArray();

    this.glowTex = gl.createTexture();
    this.glowFbo = gl.createFramebuffer();
    this.w = this.h = this.gw = this.gh = 0;
    this.viewProj = new Float32Array(16);
    this.model = new Float32Array(9);
  }

  /** Draw a caption texture as dissolving splats. `c` is a reused object. */
  drawCaption(c) {
    const gl = this.gl, u = this.cap.u;
    gl.useProgram(this.cap.p);
    gl.bindVertexArray(this.emptyVao);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, c.tex);
    gl.uniform1i(u.uTex, 0);
    gl.uniform4f(u.uRect, c.x, c.y, c.w, c.h);
    gl.uniform2f(u.uView, this.w, this.h);
    gl.uniform2f(u.uGrid, c.cols, c.rows);
    gl.uniform1f(u.uP, c.p);
    gl.uniform1f(u.uTime, c.time);
    gl.uniform1f(u.uCell, c.cell);
    gl.uniform3fv(u.uTint, c.tint);
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, c.cols * c.rows);
    gl.bindVertexArray(null);
  }

  resize(w, h) {
    const gl = this.gl;
    if (w === this.w && h === this.h) return;
    this.w = this.canvas.width = w;
    this.h = this.canvas.height = h;
    this.gw = Math.max(1, w >> 1);
    this.gh = Math.max(1, h >> 1);
    gl.bindTexture(gl.TEXTURE_2D, this.glowTex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, this.gw, this.gh, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.glowFbo);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, this.glowTex, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  upload(positions) {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.posBuf);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, positions);
  }

  /**
   * f: frame params (reused object): viewProj, model, focal, eye, time, floor,
   * wave, palette a/b/c, size, cut (instances), glow (0..1), shadow (0..1).
   */
  render(f) {
    const gl = this.gl, u = this.splat.u;
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.useProgram(this.splat.p);
    gl.bindVertexArray(this.vao);
    gl.uniformMatrix4fv(u.uViewProj, false, f.viewProj);
    gl.uniformMatrix3fv(u.uModel, false, f.model);
    gl.uniform1f(u.uTime, f.time);
    gl.uniform1f(u.uFloor, f.floor);
    gl.uniform1f(u.uWave, f.wave);
    gl.uniform3fv(u.uA, f.a);
    gl.uniform3fv(u.uB, f.b);
    gl.uniform3fv(u.uC, f.c);
    gl.uniform1f(u.uCut, f.cut);
    gl.uniform1f(u.uRadial, f.radial);
    const n = Math.min(this.count, Math.ceil(f.cut));

    // 1. half-resolution layer: soft floor shadow, then a faint glow
    const fx = f.glow > 0.01 || f.shadow > 0.01;
    if (fx) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.glowFbo);
      gl.viewport(0, 0, this.gw, this.gh);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.uniform2f(u.uPx, 2 / this.gw, 2 / this.gh);
      gl.uniform1f(u.uFocal, f.focal * 0.5);
      if (f.shadow > 0.01) {
        gl.uniform1f(u.uMode, 1);
        gl.uniform1f(u.uSize, f.size * 7);
        gl.uniform1f(u.uMaxPx, 9);
        gl.uniform1f(u.uAlpha, 0.045 * f.shadow);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, n >> 3);
      }
      if (f.glow > 0.01) {
        gl.uniform1f(u.uMode, 0);
        gl.uniform1f(u.uSize, f.size * 6);
        gl.uniform1f(u.uMaxPx, 10);
        gl.uniform1f(u.uAlpha, 0.05 * f.glow);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, n >> 2);
      }
    }

    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, this.w, this.h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.uniform2f(u.uPx, 2 / this.w, 2 / this.h);
    gl.uniform1f(u.uFocal, f.focal);

    // 2. composite the half-res layer (premultiplied, 5-tap soften)
    if (fx) {
      gl.useProgram(this.comp.p);
      gl.bindVertexArray(this.emptyVao);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.glowTex);
      gl.uniform1i(this.comp.u.uTex, 0);
      gl.uniform1f(this.comp.u.uGain, 1);
      gl.uniform2f(this.comp.u.uTexel, 1.5 / this.gw, 1.5 / this.gh);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      gl.useProgram(this.splat.p);
      gl.bindVertexArray(this.vao);
    }

    // 3. the splats
    gl.uniform1f(u.uMode, 0);
    gl.uniform1f(u.uSize, f.size);
    gl.uniform1f(u.uMaxPx, 4.5 * f.dpr);
    gl.uniform1f(u.uAlpha, f.alpha);
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, n);
    gl.bindVertexArray(null);
  }
}
