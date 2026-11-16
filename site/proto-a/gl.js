// One WebGL2 context for the whole page: extruded letters, the rippling code
// block and the shattering finale all draw into the same fixed canvas.

const HEAD = `#version 300 es
precision highp float;
uniform vec3 uView; // viewport w, h (CSS px), camera distance D
vec4 project(vec3 p) {
  float w = uView.z - p.z;
  float zc = ((w - 1.0) / (uView.z * 3.0) * 2.0 - 1.0) * w;
  return vec4((p.x - uView.x * 0.5) * 2.0 * uView.z / uView.x, -(p.y - uView.y * 0.5) * 2.0 * uView.z / uView.y, zc, w);
}
`;

const LETTER_VS = HEAD + `
in vec3 aPos; in vec3 aNrm;
uniform mat3 uRot; uniform vec3 uT; uniform vec3 uS; uniform float uHmax; uniform vec2 uShadow;
out vec3 vN; out float vH;
void main() {
  vec3 p = vec3(aPos.x * uS.x, aPos.z * uS.y, aPos.y * uS.z);
  float sz = abs(uS.z) < 1e-3 ? sign(uS.z + 1e-9) * 1e-3 : uS.z;
  vN = uRot * vec3(aNrm.x / uS.x, aNrm.z / uS.y, aNrm.y / sz);
  vH = aPos.y / uHmax;
  vec3 w = uRot * p;
  if (uShadow.x > 0.0) { w.xy += (w.z + uShadow.y) * vec2(0.28, 0.42); w.z = -4.0; }
  gl_Position = project(w + uT);
}`;
const LETTER_FS = `#version 300 es
precision highp float;
in vec3 vN; in float vH;
uniform float uE; uniform vec3 uInk; uniform vec3 uSideA; uniform vec3 uSideB; uniform vec2 uShadow;
out vec4 o;
void main() {
  float cut = mix(0.5, 0.07, smoothstep(0.0, 0.25, uE));
  if (vH < cut) discard;
  if (uShadow.x > 0.0) { o = vec4(0.0, 0.0, 0.0, 0.1 * smoothstep(0.0, 0.6, uE)); return; }
  vec3 n = normalize(vN);
  if (!gl_FrontFacing) n = -n;
  vec3 L = normalize(vec3(-0.45, -0.6, 0.66));
  float d = max(dot(n, L), 0.0);
  float face = smoothstep(mix(0.0, 0.78, uE), mix(0.01, 0.93, uE), vH);
  vec3 side = mix(uSideA, uSideB, smoothstep(0.2, 0.9, vH));
  vec3 base = mix(side, uInk, face);
  float lit = mix(1.0, 0.62 + 0.55 * d, uE);
  vec3 h = normalize(L + vec3(0.0, 0.0, 1.0));
  float spec = pow(max(dot(n, h), 0.0), 40.0) * 0.35 * uE * (1.0 - face);
  o = vec4(base * lit + spec, 1.0);
}`;

const RIPPLE_VS = HEAD + `
in vec2 aQ; uniform vec4 uRect; out vec2 vL;
void main() { vL = aQ * uRect.zw; gl_Position = project(vec3(uRect.xy + vL, 0.0)); }`;
const RIPPLE_FS = `#version 300 es
precision highp float;
in vec2 vL;
uniform sampler2D uTex; uniform vec4 uRect; uniform vec4 uRip[12]; uniform int uN;
out vec4 o;
float hgt(vec2 p) {
  float h = 0.0;
  for (int i = 0; i < 12; i++) {
    if (i >= uN) break;
    vec4 r = uRip[i];
    if (r.w <= 0.0) continue;
    float d = length(p - r.xy);
    float k = d - r.z * 380.0;
    h += r.w * sin(k * 0.085) * exp(-k * k / 5200.0) * exp(-r.z * 1.3) / (1.0 + d * 0.004);
  }
  return h;
}
void main() {
  float h0 = hgt(vL), hx = hgt(vL + vec2(2.0, 0.0)), hy = hgt(vL + vec2(0.0, 2.0));
  vec2 g = vec2(hx - h0, hy - h0) * 0.5;
  vec2 uv = (vL - g * 70.0) / uRect.zw;
  vec4 t = texture(uTex, uv);
  vec3 n = normalize(vec3(-g * 5.0, 1.0));
  vec3 L = normalize(vec3(-0.5, -0.6, 0.62));
  float dl = dot(n, L) - L.z;
  vec3 sc = dl > 0.0 ? vec3(1.0, 0.99, 0.95) : vec3(0.22, 0.32, 0.85);
  float sa = clamp(abs(dl) * (dl > 0.0 ? 3.0 : 1.0), 0.0, 0.32);
  vec3 base = t.a > 0.0 ? t.rgb / t.a : vec3(0.0);
  o = vec4(mix(base, sc, sa), t.a);
}`;

const SHARD_VS = HEAD + `
in vec2 aP; in vec2 aC; in vec3 aR; in vec3 aB;
uniform vec4 uRect; uniform float uE; uniform vec2 uO;
out vec2 vUV; out vec3 vB; out float vShade; out float vE;
vec3 rot(vec3 v, vec3 k, float a) { float c = cos(a), s = sin(a); return v * c + cross(k, v) * s + k * dot(k, v) * (1.0 - c); }
void main() {
  vec2 c = aC * uRect.zw, p = aP * uRect.zw;
  float dist = length(c - uO) / length(uRect.zw);
  float e = clamp(uE * 1.3 - dist * 0.4, 0.0, 1.0);
  e = e * e * (3.0 - 2.0 * e);
  vec3 axis = normalize(vec3(aR.x - 0.5, aR.y - 0.5, 0.25));
  float ang = e * (aR.z * 2.0 - 1.0) * 1.2;
  vec3 rel = rot(vec3(p - c, 0.0), axis, ang);
  vec2 dir = normalize(c - uO + vec2(0.001));
  vec3 off = vec3(dir * (24.0 + 130.0 * aR.z) * e, (aR.x * 300.0 - 50.0) * e);
  off.y += 90.0 * e * e * (aR.y - 0.3);
  vec3 nrm = rot(vec3(0.0, 0.0, 1.0), axis, ang);
  vShade = 0.84 + 0.3 * max(dot(nrm, normalize(vec3(-0.4, -0.5, 0.75))), 0.0);
  vShade = mix(1.0, vShade, min(e * 3.0, 1.0));
  vUV = aP; vB = aB; vE = e;
  gl_Position = project(vec3(uRect.xy + c, 0.0) + rel + off);
}`;
const SHARD_FS = `#version 300 es
precision highp float;
in vec2 vUV; in vec3 vB; in float vShade; in float vE;
uniform sampler2D uTex; uniform float uE;
out vec4 o;
void main() {
  vec4 t = texture(uTex, vUV);
  t.rgb = t.a > 0.0 ? t.rgb / t.a : t.rgb;
  float edge = min(vB.x, min(vB.y, vB.z));
  float crack = (1.0 - smoothstep(0.0, 0.035, edge)) * min(uE * 10.0, 1.0);
  vec3 col = t.rgb * vShade;
  float rim = 1.0 - smoothstep(0.0, 0.05, edge);
  col = mix(col, col * 0.6 + vec3(0.1, 0.07, 0.03), crack * 0.22 * (1.0 - min(vE * 4.0, 1.0)));
  col += vec3(1.0, 0.97, 0.9) * rim * min(vE * 4.0, 1.0) * 0.28;
  o = vec4(col, 1.0);
}`;

function compile(gl, vs, fs) {
  const p = gl.createProgram();
  for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    gl.attachShader(p, s);
  }
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) { const info = gl.getActiveUniform(p, i); const name = info.name.replace(/\[0\]$/, ''); u[name] = gl.getUniformLocation(p, info.name); }
  return { p, u };
}

const hex = (h) => [parseInt(h.slice(1, 3), 16) / 255, parseInt(h.slice(3, 5), 16) / 255, parseInt(h.slice(5, 7), 16) / 255];

export function createGL(canvas, shared) {
  const gl = shared || canvas.getContext('webgl2', { antialias: true, alpha: true, premultipliedAlpha: false, powerPreference: 'high-performance' });
  if (!gl) return null;
  const letter = compile(gl, LETTER_VS, LETTER_FS);
  const ripple = compile(gl, RIPPLE_VS, RIPPLE_FS);
  const shard = compile(gl, SHARD_VS, SHARD_FS);
  const rot = new Float32Array(9);
  const ripData = new Float32Array(48);
  const view = [1, 1, 1];
  let dpr = 1, W = 0, H = 0;

  const quad = gl.createVertexArray();
  gl.bindVertexArray(quad);
  const qb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, qb);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 1, 1]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(gl.getAttribLocation(ripple.p, 'aQ'));
  gl.vertexAttribPointer(gl.getAttribLocation(ripple.p, 'aQ'), 2, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  function texture() {
    const t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(4));
    return t;
  }
  let codeTex = texture(), panelTex = texture();

  return {
    gl,
    get dpr() { return dpr; },
    resize(vw, vh, scale) {
      dpr = scale;
      const w = Math.round(vw * dpr), h = Math.round(vh * dpr);
      if (!shared && (w !== W || h !== H)) { W = canvas.width = w; H = canvas.height = h; }
      view[0] = vw; view[1] = vh; view[2] = (vh / 2) / Math.tan((22 * Math.PI) / 180);
    },
    /** Upload a mesh from anikit (`Mesh3D.arrays()`). */
    mesh(arrays) {
      const vao = gl.createVertexArray();
      gl.bindVertexArray(vao);
      const pb = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, pb);
      gl.bufferData(gl.ARRAY_BUFFER, arrays.positions, gl.STATIC_DRAW);
      let loc = gl.getAttribLocation(letter.p, 'aPos');
      gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0);
      const nb = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, nb);
      gl.bufferData(gl.ARRAY_BUFFER, arrays.normals, gl.STATIC_DRAW);
      loc = gl.getAttribLocation(letter.p, 'aNrm');
      gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0);
      const ib = gl.createBuffer();
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, arrays.indices, gl.STATIC_DRAW);
      gl.bindVertexArray(null);
      let hmax = 1e-6;
      for (let i = 1; i < arrays.positions.length; i += 3) if (arrays.positions[i] > hmax) hmax = arrays.positions[i];
      return { vao, count: arrays.indices.length, hmax };
    },
    shards(cols, rows) {
      const pts = [];
      let seed = 7;
      const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
      for (let j = 0; j <= rows; j++) for (let i = 0; i <= cols; i++) {
        const ex = i === 0 || i === cols, ey = j === 0 || j === rows;
        pts.push((i + (ex ? 0 : (rnd() - 0.5) * 0.7)) / cols, (j + (ey ? 0 : (rnd() - 0.5) * 0.7)) / rows);
      }
      const P = (i, j) => [pts[(j * (cols + 1) + i) * 2], pts[(j * (cols + 1) + i) * 2 + 1]];
      const data = [];
      const tri = (a, b, c) => {
        const cx = (a[0] + b[0] + c[0]) / 3, cy = (a[1] + b[1] + c[1]) / 3;
        const r = [rnd(), rnd(), rnd()];
        const bars = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
        [a, b, c].forEach((v, k) => data.push(v[0], v[1], cx, cy, ...r, ...bars[k]));
      };
      for (let j = 0; j < rows; j++) for (let i = 0; i < cols; i++) {
        const a = P(i, j), b = P(i + 1, j), c = P(i, j + 1), d = P(i + 1, j + 1);
        if (rnd() < 0.5) { tri(a, b, d); tri(a, d, c); } else { tri(a, b, c); tri(b, d, c); }
      }
      const vao = gl.createVertexArray();
      gl.bindVertexArray(vao);
      const b = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW);
      const S = 10 * 4;
      [['aP', 2, 0], ['aC', 2, 2], ['aR', 3, 4], ['aB', 3, 7]].forEach(([n, k, off]) => {
        const l = gl.getAttribLocation(shard.p, n);
        gl.enableVertexAttribArray(l); gl.vertexAttribPointer(l, k, gl.FLOAT, false, S, off * 4);
      });
      gl.bindVertexArray(null);
      return { vao, count: data.length / 10 };
    },
    upload(which, source) {
      gl.bindTexture(gl.TEXTURE_2D, which === 'code' ? codeTex : panelTex);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    },
    /** Use a texture made elsewhere in this context (htmlCanvas). Premultiplied, row 0 at the top. */
    use(which, tex) { if (which === 'code') codeTex = tex; else panelTex = tex; },
    begin() {
      gl.viewport(0, 0, W, H);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    },
    colors: { ink: hex('#1c1a17'), a: hex('#2b2723'), b: hex('#8c8170') },
    /** One letter: rotation (rx, ry in radians), centre (px), size (px), extrusion in px. */
    letter(m, cx, cy, sx, sy, depth, rx, ry, e, mirror, shadow = 0) {
      const { p, u } = letter;
      gl.useProgram(p);
      gl.disable(gl.CULL_FACE);
      if (shadow) { gl.disable(gl.DEPTH_TEST); gl.enable(gl.BLEND); gl.blendEquation(gl.MAX); }
      else { gl.enable(gl.DEPTH_TEST); gl.disable(gl.BLEND); }
      gl.uniform2f(u.uShadow, shadow, depth);
      const cxr = Math.cos(rx), sxr = Math.sin(rx), cyr = Math.cos(ry), syr = Math.sin(ry);
      // R = Rx * Ry, column-major
      rot[0] = cyr; rot[1] = sxr * syr; rot[2] = -cxr * syr;
      rot[3] = 0; rot[4] = cxr; rot[5] = sxr;
      rot[6] = syr; rot[7] = -sxr * cyr; rot[8] = cxr * cyr;
      gl.uniform3f(u.uView, view[0], view[1], view[2]);
      gl.uniformMatrix3fv(u.uRot, false, rot);
      gl.uniform3f(u.uT, cx, cy, 0);
      gl.uniform3f(u.uS, sx, sy, (depth * mirror) / m.hmax);
      gl.uniform1f(u.uHmax, m.hmax);
      gl.uniform1f(u.uE, e);
      gl.uniform3fv(u.uInk, this.colors.ink); gl.uniform3fv(u.uSideA, this.colors.a); gl.uniform3fv(u.uSideB, this.colors.b);
      gl.bindVertexArray(m.vao);
      gl.drawElements(gl.TRIANGLES, m.count, gl.UNSIGNED_INT, 0);
      if (shadow) gl.blendEquation(gl.FUNC_ADD);
    },
    ripple(x, y, w, h, rips, n) {
      const { p, u } = ripple;
      gl.useProgram(p);
      gl.disable(gl.DEPTH_TEST); gl.enable(gl.BLEND);
      gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      ripData.set(rips);
      gl.uniform3f(u.uView, view[0], view[1], view[2]);
      gl.uniform4f(u.uRect, x, y, w, h);
      gl.uniform4fv(u.uRip, ripData);
      gl.uniform1i(u.uN, n);
      gl.activeTexture(gl.TEXTURE12); gl.bindTexture(gl.TEXTURE_2D, codeTex); gl.uniform1i(u.uTex, 12); gl.activeTexture(gl.TEXTURE0);
      gl.bindVertexArray(quad);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    },
    shatter(s, x, y, w, h, e, ox, oy) {
      const { p, u } = shard;
      gl.useProgram(p);
      gl.enable(gl.DEPTH_TEST); gl.disable(gl.BLEND); gl.disable(gl.CULL_FACE);
      gl.uniform3f(u.uView, view[0], view[1], view[2]);
      gl.uniform4f(u.uRect, x, y, w, h);
      gl.uniform1f(u.uE, e);
      gl.uniform2f(u.uO, ox, oy);
      gl.activeTexture(gl.TEXTURE12); gl.bindTexture(gl.TEXTURE_2D, panelTex); gl.uniform1i(u.uTex, 12); gl.activeTexture(gl.TEXTURE0);
      gl.bindVertexArray(s.vao);
      gl.drawArrays(gl.TRIANGLES, 0, s.count);
    },
    clearDepth() { gl.clear(gl.DEPTH_BUFFER_BIT); },
  };
}
