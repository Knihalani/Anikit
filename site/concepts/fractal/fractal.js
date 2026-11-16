// Ray-marched Mandelbulb in a WebGL2 fragment shader. No anikit dependency, so the
// first frame is drawn as soon as this module runs; main.js steers the camera.
//
//   const f = createFractal(canvas); f.set({ eye, forward, up, power, fade }); f.start();

const VERT = `#version 300 es
in vec2 p; void main(){ gl_Position = vec4(p, 0.0, 1.0); }`;

const FRAG = `#version 300 es
precision highp float;
out vec4 o;
uniform vec2 uRes;
uniform vec3 uEye, uRight, uUp, uFwd;
uniform float uFov, uScale, uPower, uTime, uDepth, uAperture, uFocus, uFade, uSeed, uSteps;
uniform vec2 uJitter;

float hash(vec2 p){ p = fract(p * vec2(443.897, 441.423)); p += dot(p, p.yx + 19.19); return fract((p.x + p.y) * p.x); }

// Distance estimate + orbit trap (min |z|², min |x|, |y|, |z| over the orbit).
float de(vec3 p, out vec4 trap){
  vec3 z = p; float dr = 1.0, r = length(z);
  trap = vec4(abs(z), dot(z, z));
  for (int i = 0; i < 9; i++) {
    if (r > 2.0) break;
    float th = acos(clamp(z.z / r, -1.0, 1.0)) * uPower;
    float ph = atan(z.y, z.x) * uPower;
    float rp = pow(r, uPower - 1.0);
    dr = rp * uPower * dr + 1.0;
    z = rp * r * vec3(sin(th) * cos(ph), sin(th) * sin(ph), cos(th)) + p;
    trap = min(trap, vec4(abs(z), dot(z, z)));
    r = length(z);
  }
  return 0.5 * log(r) * r / dr;
}
float de(vec3 p){ vec4 t; return de(p, t); }

vec3 normalAt(vec3 p, float h){
  const vec2 k = vec2(1.0, -1.0);
  return normalize(k.xyy * de(p + k.xyy * h) + k.yyx * de(p + k.yyx * h) + k.yxy * de(p + k.yxy * h) + k.xxx * de(p + k.xxx * h));
}

float shadow(vec3 ro, vec3 rd, float mint, float maxt){
  float res = 1.0, t = mint;
  for (int i = 0; i < 28; i++) {
    float h = de(ro + rd * t);
    res = min(res, 10.0 * h / t);
    t += clamp(h, mint * 0.5, maxt * 0.12);
    if (res < 0.002 || t > maxt) break;
  }
  return clamp(res, 0.0, 1.0);
}

// Cosine palette; its phase drifts with depth.
vec3 pal(float t){
  return 0.5 + 0.5 * cos(6.2831853 * (vec3(1.0, 0.9, 0.8) * t + vec3(0.02, 0.18, 0.36) + uDepth * vec3(0.10, 0.17, 0.23)));
}

vec3 aces(vec3 x){ return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }

void main(){
  vec2 frag = gl_FragCoord.xy + uJitter;
  float side = min(uRes.y, uRes.x * 0.8);   // portrait screens see wider, not narrower
  vec2 uv = (frag - 0.5 * uRes) / side;
  float n0 = hash(gl_FragCoord.xy + uSeed * 17.0);
  vec3 rd = normalize(uFwd + (uv.x * uRight + uv.y * uUp) * uFov);
  vec3 ro = uEye;
  // Thin-lens depth of field: jitter the origin on the aperture, keep the focal plane.
  if (uAperture > 0.0) {
    float a = 6.2831853 * hash(gl_FragCoord.yx * 1.7 + uSeed * 3.1), rr = sqrt(n0);
    vec3 fp = ro + rd * (uFocus / dot(rd, uFwd));
    ro += (uRight * cos(a) + uUp * sin(a)) * rr * uAperture;
    rd = normalize(fp - ro);
  }
  float S = uScale;
  vec3 fogCol = mix(vec3(0.006, 0.007, 0.012), pal(0.62) * 0.05, 0.5);
  vec3 bg = fogCol * (0.7 + 0.5 * pow(max(0.0, 1.0 - length(uv) * 0.9), 2.0));

  // Skip empty space outside the bounding sphere.
  float t = 0.0, tmax = 6.0;
  float b = dot(ro, rd), c = dot(ro, ro) - 1.44, disc = b * b - c;
  bool hit = false; float glow = 0.0; vec4 trap = vec4(0.0); int steps = 0;
  float pix = uFov / side;
  if (disc > 0.0) {
    t = max(0.0, -b - sqrt(disc)); tmax = -b + sqrt(disc);
    t += n0 * 0.02 * S; // dithered start hides banding
    for (int i = 0; i < 160; i++) {
      if (float(i) >= uSteps) break;
      vec3 p = ro + rd * t;
      float d = de(p, trap);
      float eps = max(pix * t * 0.6, S * 0.0006);
      glow += exp(-d / (S * 0.05 + 0.002)) * 0.012;
      if (d < eps) { hit = true; break; }
      t += d * 0.92;
      steps = i;
      if (t > tmax) break;
    }
  }
  vec3 col = bg;
  vec3 sun = normalize(uRight * 0.55 + uUp * 0.8 - uFwd * 0.35 + vec3(0.0, 0.25, 0.0));
  if (hit) {
    vec3 p = ro + rd * t;
    float h = max(pix * t * 0.6, S * 0.0006);
    vec3 n = normalAt(p, h);
    // Ambient occlusion from the orbit trap, refined by step count.
    float occ = clamp(pow(trap.w, 0.35) * 1.15 - 0.12, 0.0, 1.0);
    occ *= clamp(1.0 - float(steps) / uSteps * 0.6, 0.0, 1.0);
    vec3 alb = pal(0.42 + trap.x * 0.6 + trap.y * 0.25 - trap.z * 0.2 + sqrt(trap.w) * 0.25);
    alb = mix(alb, vec3(0.85, 0.8, 0.74), 0.12) * 0.5;
    float dif = max(dot(n, sun), 0.0);
    float sha = dif > 0.0 ? shadow(p + n * h * 2.0, sun, h * 4.0, max(S * 2.5, 0.02)) : 0.0;
    float sky = 0.5 + 0.5 * dot(n, uUp);
    float rim = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
    vec3 lin = vec3(0.0);
    lin += vec3(2.1, 1.6, 1.15) * dif * sha;
    lin += pal(0.12) * 0.13 * sky * occ * occ;
    lin += pal(0.75) * 0.3 * rim * occ;
    lin += vec3(0.015) * occ;
    col = alb * lin * (0.2 + 0.8 * occ);
    float spe = pow(max(dot(reflect(rd, n), sun), 0.0), 24.0) * sha * occ;
    col += vec3(1.0, 0.85, 0.65) * spe * 0.5;
  }
  // Fog measured in the camera's own scale, so every depth reads the same.
  float fogT = hit ? t : tmax;
  float fog = 1.0 - exp(-fogT / (S * 9.0 + 0.05));
  col = mix(col, bg, hit ? fog * 0.85 : 1.0);
  col += pal(0.68 + 0.1 * sin(uTime * 0.05)) * glow * 0.4;
  col = mix(col, vec3(0.012, 0.013, 0.02), uFade);
  col = aces(col * 1.05);
  col = pow(col, vec3(0.4545));
  vec2 q = gl_FragCoord.xy / uRes;
  col *= 0.55 + 0.45 * pow(16.0 * q.x * q.y * (1.0 - q.x) * (1.0 - q.y), 0.22);
  col += (hash(gl_FragCoord.xy + uSeed) - 0.5) / 255.0;
  o = vec4(col, 1.0);
}`;

// Same estimate in JS (double precision), used for the camera's local scale.
export function de(x, y, z, power = 8) {
  let zx = x, zy = y, zz = z, dr = 1, r = Math.hypot(zx, zy, zz);
  for (let i = 0; i < 9 && r <= 2; i++) {
    const th = Math.acos(Math.max(-1, Math.min(1, zz / r))) * power, ph = Math.atan2(zy, zx) * power;
    const rp = Math.pow(r, power - 1);
    dr = rp * power * dr + 1;
    const rr = rp * r;
    zx = rr * Math.sin(th) * Math.cos(ph) + x; zy = rr * Math.sin(th) * Math.sin(ph) + y; zz = rr * Math.cos(th) + z;
    r = Math.hypot(zx, zy, zz);
  }
  return (0.5 * Math.log(r) * r) / dr;
}

export function createFractal(canvas, { quality, initial } = {}) {
  const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, preserveDrawingBuffer: true, powerPreference: 'high-performance' });
  if (!gl) { canvas.classList.add('nogl'); return null; }
  const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) console.warn(gl.getShaderInfoLog(s)); return s; };
  const prog = gl.createProgram();
  gl.attachShader(prog, sh(gl.VERTEX_SHADER, VERT));
  gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FRAG));
  gl.linkProgram(prog);
  gl.useProgram(prog);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  const U = {};
  for (const n of ['uRes', 'uEye', 'uRight', 'uUp', 'uFwd', 'uFov', 'uScale', 'uPower', 'uTime', 'uDepth', 'uAperture', 'uFocus', 'uFade', 'uSeed', 'uSteps', 'uJitter']) U[n] = gl.getUniformLocation(prog, n);

  const mobile = matchMedia('(pointer: coarse)').matches || Math.min(innerWidth, innerHeight) < 600;
  const fixed = quality != null;                       // ?q=… pins the resolution (for captures)
  const maxScale = fixed ? quality : mobile ? 0.6 : 0.8;
  let scale = fixed ? quality : mobile ? 0.4 : 0.55;     // start low: first frame fast
  const steps = mobile ? 90 : 130;
  const state = { eye: [0.4, 0.5, 3.1], forward: [0, 0, -1], up: [0, 1, 0], power: 8, fade: 0, focus: 2, time: 0, depth: 0 };
  Object.assign(state, initial);
  let prev = null, n = 0, frame = 0, running = false, raf = 0, w = 0, h = 0, last = 0, slow = 0, fast = 0, cool = 0, idle = false;

  const resize = () => {
    const dpr = Math.min(devicePixelRatio || 1, 1.25);
    const cw = Math.round(innerWidth * dpr * scale), ch = Math.round(innerHeight * dpr * scale);
    if (cw !== w || ch !== h) { w = canvas.width = cw; h = canvas.height = ch; n = 0; }
  };
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const norm = (a) => { const l = Math.hypot(a[0], a[1], a[2]) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];

  function draw() {
    if (state.fade >= 0.999 && idle) return;            // calm void: nothing left to draw
    resize();
    const f = norm(state.forward), r = norm(cross(f, state.up)), u = cross(r, f);
    const S = Math.max(1e-5, de(state.eye[0], state.eye[1], state.eye[2], state.power));
    // Motion relative to the camera's own scale decides how much history to keep.
    const cur = [...state.eye, ...f, state.power, state.fade];
    let m = 1;
    if (prev) {
      m = Math.hypot(...sub(state.eye, prev)) / S + Math.hypot(...sub(f, prev.slice(3, 6))) * 40 + Math.abs(state.power - prev[6]) * 400 + Math.abs(state.fade - prev[7]) * 4;
    }
    prev = cur;
    if (m > 0.02) n = 0; else if (m > 0.002) n = Math.min(n, 2);
    const still = Math.min(1, Math.max(0, (n - 3) / 14));
    const alpha = Math.max(1 / (n + 1), 0.07);
    gl.viewport(0, 0, w, h);
    gl.uniform2f(U.uRes, w, h);
    gl.uniform3fv(U.uEye, state.eye);
    gl.uniform3fv(U.uRight, r);
    gl.uniform3fv(U.uUp, u);
    gl.uniform3fv(U.uFwd, f);
    gl.uniform1f(U.uFov, 0.72);
    gl.uniform1f(U.uScale, S);
    gl.uniform1f(U.uPower, state.power);
    gl.uniform1f(U.uTime, state.time);
    gl.uniform1f(U.uDepth, state.depth);
    gl.uniform1f(U.uAperture, S * 0.035 * still);
    gl.uniform1f(U.uFocus, state.focus);
    gl.uniform1f(U.uFade, state.fade);
    gl.uniform1f(U.uSeed, (frame % 64) + 1);
    gl.uniform1f(U.uSteps, steps);
    const jit = n > 0 ? [Math.random() - 0.5, Math.random() - 0.5] : [0, 0];
    gl.uniform2f(U.uJitter, jit[0], jit[1]);
    if (alpha < 1) { gl.enable(gl.BLEND); gl.blendColor(0, 0, 0, alpha); gl.blendFunc(gl.CONSTANT_ALPHA, gl.ONE_MINUS_CONSTANT_ALPHA); } else gl.disable(gl.BLEND);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    n++; frame++;
    idle = state.fade >= 0.999;
  }

  function loop(now) {
    raf = requestAnimationFrame(loop);
    const dt = last ? now - last : 16.7;
    last = now;
    // Adaptive resolution: step down quickly when frames run long, creep back up slowly.
    if (!fixed && dt < 100) {
      if (dt > 19.5) { slow++; fast = 0; } else if (dt < 17.6) { fast++; slow = Math.max(0, slow - 1); }
      if (cool > 0) cool--;
      else if (slow > 12 && scale > 0.22) { scale = Math.max(0.22, scale * 0.84); slow = 0; cool = 40; }
      else if (fast > 150 && scale < maxScale) { scale = Math.min(maxScale, scale * 1.08); fast = 0; cool = 90; }
    }
    draw();
  }

  const api = {
    state,
    get scale() { return scale; },
    set(o) { Object.assign(state, o); },
    draw,
    start() { if (running) return; running = true; last = 0; raf = requestAnimationFrame(loop); },
    stop() { running = false; cancelAnimationFrame(raf); },
  };
  document.addEventListener('visibilitychange', () => { if (document.hidden) { cancelAnimationFrame(raf); } else if (running) { last = 0; raf = requestAnimationFrame(loop); } });
  addEventListener('resize', () => { prev = null; });
  draw();                                               // static first frame, right away
  return api;
}
