// Volumetric cumulus at golden hour, ray marched in a WebGL fragment shader.
//
// Pass 1 (adaptive resolution, into a texture): march the cloud slab with fbm density,
// a short light march towards the sun (Beer + powder), a dual-lobe Henyey–Greenstein
// phase for the silver lining, a multiple-scattering approximation, aerial haze and
// per-pixel jitter against banding. Alpha keeps the transmittance to the sky.
// Pass 2 (full resolution): bilinear upscale, god rays (a radial blur of the unoccluded
// sky towards the sun), ACES tone map and dither.
//
//   const sky = createSky(canvas);            // null without WebGL
//   sky.render({ eye, target, roll, time, clear, sun, scale });

const VS = `attribute vec2 p; varying vec2 v; void main(){ v = p * 0.5 + 0.5; gl_Position = vec4(p, 0.0, 1.0); }`;

const CLOUDS = `precision highp float;
varying vec2 v;
uniform vec2 uRes; uniform vec3 uEye; uniform mat3 uBasis; uniform float uTan, uTime, uClear, uFrame, uBank;
uniform vec3 uSun;
const float BOT = 1300.0, TOP = 3300.0;
float hash(vec3 p){ p = fract(p * 0.3183099 + 0.1); p *= 17.0; return fract(p.x * p.y * p.z * (p.x + p.y + p.z)); }
float noise(vec3 x){
  vec3 i = floor(x), f = fract(x); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(hash(i), hash(i + vec3(1,0,0)), f.x), mix(hash(i + vec3(0,1,0)), hash(i + vec3(1,1,0)), f.x), f.y),
             mix(mix(hash(i + vec3(0,0,1)), hash(i + vec3(1,0,1)), f.x), mix(hash(i + vec3(0,1,1)), hash(i + vec3(1,1,1)), f.x), f.y), f.z);
}
vec3 skyCol(vec3 d){
  float sh = uSun.y;
  float h = d.y;
  vec3 zen = mix(vec3(0.05, 0.09, 0.24), vec3(0.11, 0.22, 0.52), clamp(sh * 4.0, 0.0, 1.0));
  vec3 hor = mix(vec3(1.15, 0.46, 0.18), vec3(1.05, 0.66, 0.40), clamp(sh * 3.0, 0.0, 1.0));
  float k = pow(1.0 - clamp(h, 0.0, 1.0), 6.0);
  vec3 c = mix(zen, hor, k);
  float mu = max(dot(d, uSun), 0.0);
  c += vec3(1.0, 0.62, 0.3) * pow(mu, 8.0) * 0.55 + vec3(1.0, 0.8, 0.55) * pow(mu, 64.0) * 0.8;
  if (h < 0.0) c = mix(hor * 0.8, vec3(0.16, 0.17, 0.26), sqrt(clamp(-h * 2.5, 0.0, 1.0))) + vec3(1.0, 0.55, 0.25) * pow(mu, 30.0) * 0.4; // haze over the sea, and the sun's glint
  return c;
}
float density(vec3 p, bool fine){
  float h = (p.y - BOT) / (TOP - BOT);
  if (h < 0.0 || h > 1.0) return 0.0;
  vec3 q = p * 0.00042 + vec3(uTime * 0.006, 0.0, uTime * 0.012);
  float base = noise(q * 0.9) * 0.58 + noise(q * 2.0) * 0.28 + noise(q * 4.1) * 0.14;
  float prof = smoothstep(0.0, 0.14, h) * smoothstep(1.0, 0.35, h);
  float d = clamp((base * (0.5 + 0.5 * prof) - 0.45) * 3.6, 0.0, 1.0) * prof;
  if (fine && d > 0.0) {
    float e = noise(q * 9.0) * 0.6 + noise(q * 19.0) * 0.4;
    d = clamp(d - e * 0.32 * (1.0 - d * 0.7), 0.0, 1.0);
  }
  // The pocket of clear sky the camera flies into, and the bank that closes around it between pockets.
  float r = length(p - uEye);
  d *= mix(1.0, smoothstep(1100.0, 2800.0, r), uClear);
  if (uBank > 0.0) d = max(d, uBank * prof * smoothstep(2600.0, 500.0, r) * clamp(0.35 + base * 0.9, 0.0, 1.0));
  return d;
}
float hg(float c, float g){ float g2 = g * g; return (1.0 - g2) / (12.566 * pow(1.0 + g2 - 2.0 * g * c, 1.5)); }
void main(){
  vec2 ndc = v * 2.0 - 1.0;
  vec3 dir = normalize(uBasis * vec3(ndc.x * uTan * uRes.x / uRes.y, ndc.y * uTan, -1.0));
  vec3 sky = skyCol(dir);
  // Slab intersection.
  float t0 = 0.0, t1 = 0.0;
  if (abs(dir.y) > 1e-4) {
    float a = (BOT - uEye.y) / dir.y, b = (TOP - uEye.y) / dir.y;
    t0 = max(min(a, b), 0.0); t1 = max(a, b);
  } else if (uEye.y > BOT && uEye.y < TOP) { t1 = 1e9; }
  t1 = min(t1, 16000.0);
  vec3 col = vec3(0.0); float T = 1.0;
  if (t1 > t0) {
    float jit = fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))) + uFrame * 0.618034);
    float cosT = dot(dir, uSun);
    float ph = mix(hg(cosT, 0.6), hg(cosT, -0.25), 0.3);
    float t = t0 + jit * 60.0;
    for (int i = 0; i < 72; i++) {
      if (t > t1 || T < 0.02) break;
      float stp = 55.0 + t * 0.018;
      vec3 p = uEye + dir * t;
      float d = density(p, true);
      if (d > 0.002) {
        // Light march towards the sun.
        float ld = 0.0;
        for (int j = 1; j <= 5; j++) ld += density(p + uSun * (float(j) * float(j) * 45.0), false) * float(j) * 45.0 * 0.4;
        float x = ld * 0.012;
        // Multiple scattering approximation (3 octaves) and the powder term.
        float sc = 0.0, a = 1.0, b = 1.0, c = 1.0;
        for (int o = 0; o < 3; o++) { sc += a * mix(hg(cosT, 0.6 * c), hg(cosT, -0.25 * c), 0.3) * exp(-x * b); a *= 0.5; b *= 0.5; c *= 0.5; }
        float powder = 1.0 - exp(-d * 6.0);
        float h = (p.y - BOT) / (TOP - BOT);
        vec3 sunL = vec3(1.7, 0.98, 0.52) * (0.35 + 0.65 * clamp(uSun.y * 5.0 + 0.2, 0.0, 1.0));
        vec3 amb = mix(vec3(0.05, 0.06, 0.11), vec3(0.2, 0.22, 0.32), h);
        vec3 L = sunL * sc * 12.0 * mix(1.0, powder, 0.75) + amb;
        float sig = d * 0.022;
        float tr = exp(-sig * stp);
        // Energy-conserving integration.
        col += T * L * (1.0 - tr);
        T *= tr;
        // Aerial haze on far cloud.
        }
      t += stp;
    }
    float fog = 1.0 - exp(-t0 * 0.00011);
    col = mix(col, sky * (1.0 - T), fog * 0.85);
  }
  gl_FragColor = vec4(col + sky * T, T);
}`;

const COMPOSE = `precision highp float;
varying vec2 v;
uniform sampler2D uTex; uniform vec2 uSunUv; uniform float uSunVis, uExposure, uRays; uniform vec2 uRes;
vec3 aces(vec3 x){ return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
void main(){
  vec4 c = texture2D(uTex, v);
  vec3 col = c.rgb;
  // God rays: march towards the sun, gathering unoccluded sky (alpha = transmittance).
  if (uRays > 0.0) {
    vec2 dv = (uSunUv - v) / 28.0;
    vec2 q = v; float acc = 0.0, w = 1.0;
    float n = fract(sin(dot(v * uRes, vec2(12.9898, 78.233))) * 43758.5453);
    q += dv * n;
    for (int i = 0; i < 28; i++) { q += dv; acc += texture2D(uTex, q).a * w; w *= 0.96; }
    float fall = exp(-length((v - uSunUv) * vec2(uRes.x / uRes.y, 1.0)) * 2.2);
    col += vec3(1.0, 0.7, 0.42) * acc / 28.0 * fall * uRays * uSunVis;
  }
  col = aces(col * uExposure);
  col = pow(col, vec3(1.0 / 2.2));
  col += (fract(sin(dot(gl_FragCoord.xy, vec2(12.9898, 78.233))) * 43758.5453) - 0.5) / 255.0;
  gl_FragColor = vec4(col, 1.0);
}`;

export function createSky(canvas) {
  const gl = canvas.getContext('webgl', { antialias: false, alpha: false, depth: false, preserveDrawingBuffer: false, powerPreference: 'high-performance' });
  if (!gl) return null;
  const prog = (fs) => {
    const p = gl.createProgram();
    for (const [type, src] of [[gl.VERTEX_SHADER, VS], [gl.FRAGMENT_SHADER, fs]]) {
      const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
      gl.attachShader(p, s);
    }
    gl.bindAttribLocation(p, 0, 'p'); gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
    const u = {}; const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
    for (let i = 0; i < n; i++) { const a = gl.getActiveUniform(p, i); u[a.name] = gl.getUniformLocation(p, a.name); }
    return { p, u };
  };
  const P1 = prog(CLOUDS), P2 = prog(COMPOSE);
  const buf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  const half = gl.getExtension('OES_texture_half_float'), halfLin = gl.getExtension('OES_texture_half_float_linear');
  const type = half && halfLin ? half.HALF_FLOAT_OES : gl.UNSIGNED_BYTE;
  const tex = gl.createTexture(), fb = gl.createFramebuffer();
  let tw = 0, th = 0;
  const target = (w, h) => {
    if (w === tw && h === th) return;
    tw = w; th = h;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, type, null);
    for (const [k, val] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, val);
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  };
  const basis = new Float32Array(9);
  let frame = 0;
  return {
    /** Fraction of full resolution the clouds are marched at (0.2–1). */
    scale: 0.5,
    render(s) {
      const W = canvas.width, H = canvas.height;
      const w = Math.max(16, Math.round(W * this.scale)), h = Math.max(16, Math.round(H * this.scale));
      target(w, h);
      // Camera basis (right, up, back) with roll.
      const [ex, ey, ez] = s.eye, [tx, ty, tz] = s.target;
      let fx = tx - ex, fy = ty - ey, fz = tz - ez; const fl = Math.hypot(fx, fy, fz); fx /= fl; fy /= fl; fz /= fl;
      let rx = -fz, rz = fx; const rl = Math.hypot(rx, rz) || 1; rx /= rl; rz /= rl; // right = forward × up(0,1,0)
      let ux = -rz * fy, uy = rz * fx - rx * fz, uz = rx * fy; // up = right × forward
      const ca = Math.cos((s.roll || 0) * Math.PI / 180), sa = Math.sin((s.roll || 0) * Math.PI / 180);
      const Rx = rx * ca + ux * sa, Ry = uy * sa, Rz = rz * ca + uz * sa;
      const Ux = ux * ca - rx * sa, Uy = uy * ca, Uz = uz * ca - rz * sa;
      basis.set([Rx, Ry, Rz, Ux, Uy, Uz, -fx, -fy, -fz]);
      const tan = Math.tan((s.fov ?? 60) * Math.PI / 360);
      const sun = s.sun;
      gl.bindFramebuffer(gl.FRAMEBUFFER, fb); gl.viewport(0, 0, w, h);
      gl.useProgram(P1.p);
      gl.uniform2f(P1.u.uRes, w, h); gl.uniform3f(P1.u.uEye, ex, ey, ez); gl.uniformMatrix3fv(P1.u.uBasis, false, basis);
      gl.uniform1f(P1.u.uTan, tan); gl.uniform1f(P1.u.uTime, s.time); gl.uniform1f(P1.u.uClear, s.clear); gl.uniform1f(P1.u.uBank, s.bank || 0); gl.uniform1f(P1.u.uFrame, frame++ % 64);
      gl.uniform3f(P1.u.uSun, sun[0], sun[1], sun[2]);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      // Sun on screen.
      const dx = sun[0] * Rx + sun[1] * Ry + sun[2] * Rz, dy = sun[0] * Ux + sun[1] * Uy + sun[2] * Uz, dz = -(sun[0] * -fx + sun[1] * -fy + sun[2] * -fz);
      const vis = dz > 0 ? Math.min(1, dz * 3) : 0;
      const su = 0.5 + (dz > 0 ? dx / dz / (tan * W / H) : 0) * 0.5, sv = 0.5 + (dz > 0 ? dy / dz / tan : 0) * 0.5;
      gl.bindFramebuffer(gl.FRAMEBUFFER, null); gl.viewport(0, 0, W, H);
      gl.useProgram(P2.p);
      gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, tex); gl.uniform1i(P2.u.uTex, 0);
      gl.uniform2f(P2.u.uSunUv, su, sv); gl.uniform1f(P2.u.uSunVis, vis); gl.uniform1f(P2.u.uExposure, s.exposure ?? 1.1);
      gl.uniform1f(P2.u.uRays, s.rays ?? 0.6); gl.uniform2f(P2.u.uRes, W, H);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    },
    size: () => [tw, th],
  };
}
