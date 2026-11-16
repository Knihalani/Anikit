// hall-gl.js — small WebGL2 helpers for the hall: programs, meshes, instanced groups,
// camera maths. No allocation in the per-frame paths.

export function program(gl, vs, fs) {
  const sh = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(s);
      throw new Error('hall shader: ' + log + '\n' + src.split('\n').map((l, i) => i + 1 + ': ' + l).join('\n'));
    }
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error('hall link: ' + gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const info = gl.getActiveUniform(p, i);
    u[info.name.replace(/\[0\]$/, '')] = gl.getUniformLocation(p, info.name);
  }
  return { p, u };
}

// ---- meshes: interleaved [px,py,pz,nx,ny,nz], indexed ----

export function cubeMesh() {
  const v = [], idx = [];
  const faces = [
    [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1],
  ];
  for (const [nx, ny, nz] of faces) {
    // any a ⟂ n; with b = n × a, a × b = n, so the corners below wind CCW from outside
    const a = nx !== 0 ? [0, 0, 1] : [1, 0, 0];
    const b = [ny * a[2] - nz * a[1], nz * a[0] - nx * a[2], nx * a[1] - ny * a[0]];
    const base = v.length / 6;
    for (const [s, t] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
      v.push(0.5 * (nx + s * a[0] + t * b[0]), 0.5 * (ny + s * a[1] + t * b[1]), 0.5 * (nz + s * a[2] + t * b[2]), nx, ny, nz);
    }
    idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
  }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}

export function cylinderMesh(seg = 20) {
  const v = [], idx = [];
  for (let k = 0; k <= seg; k++) {
    const a = (k / seg) * Math.PI * 2, c = Math.cos(a), s = Math.sin(a);
    v.push(0.5 * c, -0.5, 0.5 * s, c, 0, s, 0.5 * c, 0.5, 0.5 * s, c, 0, s);
  }
  for (let k = 0; k < seg; k++) {
    const a = k * 2;
    idx.push(a, a + 1, a + 3, a, a + 3, a + 2);
  }
  // caps
  for (const y of [-0.5, 0.5]) {
    const c0 = v.length / 6;
    v.push(0, y, 0, 0, Math.sign(y), 0);
    for (let k = 0; k <= seg; k++) {
      const a = (k / seg) * Math.PI * 2;
      v.push(0.5 * Math.cos(a), y, 0.5 * Math.sin(a), 0, Math.sign(y), 0);
    }
    for (let k = 0; k < seg; k++) {
      if (y > 0) idx.push(c0, c0 + 2 + k, c0 + 1 + k);
      else idx.push(c0, c0 + 1 + k, c0 + 2 + k);
    }
  }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}

export function sphereMesh(lon = 14, lat = 9) {
  const v = [], idx = [];
  for (let j = 0; j <= lat; j++) {
    const th = (j / lat) * Math.PI, st = Math.sin(th), ct = Math.cos(th);
    for (let k = 0; k <= lon; k++) {
      const ph = (k / lon) * Math.PI * 2;
      const x = st * Math.cos(ph), y = ct, z = st * Math.sin(ph);
      v.push(0.5 * x, 0.5 * y, 0.5 * z, x, y, z);
    }
  }
  for (let j = 0; j < lat; j++) {
    for (let k = 0; k < lon; k++) {
      const a = j * (lon + 1) + k, b = a + lon + 1;
      idx.push(a, a + 1, b, a + 1, b + 1, b);
    }
  }
  return { v: new Float32Array(v), i: new Uint16Array(idx) };
}

// ---- instanced group: 16 floats per instance: posRotY(4) scaleGloss(4) colorEmis(4) rotXZ(4) ----

export const STRIDE = 16;

export function group(gl, mesh, max) {
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const vb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vb);
  gl.bufferData(gl.ARRAY_BUFFER, mesh.v, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);
  const ib = gl.createBuffer();
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
  gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.i, gl.STATIC_DRAW);
  const inst = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, inst);
  const data = new Float32Array(max * STRIDE);
  gl.bufferData(gl.ARRAY_BUFFER, data.byteLength, gl.DYNAMIC_DRAW);
  for (let a = 0; a < 4; a++) {
    gl.enableVertexAttribArray(2 + a);
    gl.vertexAttribPointer(2 + a, 4, gl.FLOAT, false, STRIDE * 4, a * 16);
    gl.vertexAttribDivisor(2 + a, 1);
  }
  gl.bindVertexArray(null);
  const g = {
    data, count: 0, max, n: mesh.i.length, visible: true,
    // write instance i
    put(i, x, y, z, ry, sx, sy, sz, gloss, r, gg, b, emis, rx = 0, rz = 0) {
      const o = i * STRIDE;
      data[o + 12] = rx; data[o + 13] = rz;
      data[o] = x; data[o + 1] = y; data[o + 2] = z; data[o + 3] = ry;
      data[o + 4] = sx; data[o + 5] = sy; data[o + 6] = sz; data[o + 7] = gloss;
      data[o + 8] = r; data[o + 9] = gg; data[o + 10] = b; data[o + 11] = emis;
    },
    push(x, y, z, ry, sx, sy, sz, gloss, r, gg, b, emis, rx = 0, rz = 0) {
      g.put(g.count++, x, y, z, ry, sx, sy, sz, gloss, r, gg, b, emis, rx, rz);
    },
    upload(n = g.count) {
      gl.bindBuffer(gl.ARRAY_BUFFER, inst);
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, data, 0, n * STRIDE);
    },
    draw() {
      if (!g.visible || g.count === 0) return;
      gl.bindVertexArray(vao);
      gl.drawElementsInstanced(gl.TRIANGLES, g.n, gl.UNSIGNED_SHORT, 0, g.count);
    },
  };
  return g;
}

// ---- camera ----

export function camera() {
  return {
    eye: new Float32Array(3), yaw: 0, pitch: 0, fov: 1.15, aspect: 1, near: 0.08, far: 120,
    view: new Float32Array(16), proj: new Float32Array(16), vp: new Float32Array(16),
    f: new Float32Array(3), r: new Float32Array(3), u: new Float32Array(3), tanH: 1,
  };
}

export function updateCamera(c) {
  const cp = Math.cos(c.pitch), sp = Math.sin(c.pitch), cy = Math.cos(c.yaw), sy = Math.sin(c.yaw);
  const f = c.f, r = c.r, u = c.u, e = c.eye;
  f[0] = -sy * cp; f[1] = sp; f[2] = -cy * cp;
  r[0] = cy; r[1] = 0; r[2] = -sy;
  u[0] = r[1] * f[2] - r[2] * f[1]; u[1] = r[2] * f[0] - r[0] * f[2]; u[2] = r[0] * f[1] - r[1] * f[0];
  const m = c.view;
  m[0] = r[0]; m[4] = r[1]; m[8] = r[2]; m[12] = -(r[0] * e[0] + r[1] * e[1] + r[2] * e[2]);
  m[1] = u[0]; m[5] = u[1]; m[9] = u[2]; m[13] = -(u[0] * e[0] + u[1] * e[1] + u[2] * e[2]);
  m[2] = -f[0]; m[6] = -f[1]; m[10] = -f[2]; m[14] = f[0] * e[0] + f[1] * e[1] + f[2] * e[2];
  m[3] = 0; m[7] = 0; m[11] = 0; m[15] = 1;
  const t = 1 / Math.tan(c.fov / 2);
  c.tanH = 1 / t;
  const p = c.proj;
  p.fill(0);
  p[0] = t / c.aspect; p[5] = t; p[10] = (c.far + c.near) / (c.near - c.far); p[11] = -1;
  p[14] = (2 * c.far * c.near) / (c.near - c.far);
  // vp = proj * view (column-major)
  const o = c.vp;
  for (let col = 0; col < 4; col++) {
    for (let row = 0; row < 4; row++) {
      o[col * 4 + row] = p[row] * m[col * 4] + p[4 + row] * m[col * 4 + 1] + p[8 + row] * m[col * 4 + 2] + p[12 + row] * m[col * 4 + 3];
    }
  }
}

// sphere vs view frustum (cheap, conservative)
export function sees(c, x, y, z, rad) {
  const dx = x - c.eye[0], dy = y - c.eye[1], dz = z - c.eye[2];
  const zf = dx * c.f[0] + dy * c.f[1] + dz * c.f[2];
  if (zf < -rad) return false;
  const zz = Math.max(zf, 0.001);
  const sx = Math.abs(dx * c.r[0] + dy * c.r[1] + dz * c.r[2]);
  const sy = Math.abs(dx * c.u[0] + dy * c.u[1] + dz * c.u[2]);
  return sx < zz * c.tanH * c.aspect + rad * 1.5 && sy < zz * c.tanH + rad * 1.5;
}
