// The planet: physically based single scattering (Rayleigh + Mie + ozone) over a
// procedural world, in one WebGL fragment shader. No anikit here, so the first
// frame can be drawn before the engine has loaded; main.js drives the camera.

const VERT = `attribute vec2 p; void main(){ gl_Position = vec4(p, 0.0, 1.0); }`;

const FRAG = `
precision highp float;
uniform vec2 uRes;
uniform vec3 uCam;
uniform mat3 uRot;
uniform float uFov;
uniform vec3 uSun;
uniform vec3 uSunScr;      // xy: sun in view space (uv units), z: visibility 0..1
uniform float uTime;
uniform float uExposure;
uniform vec3 uPole, uPoleA, uPoleB;
uniform mat3 uSpin;
uniform float uNight;      // 1 at midnight, 0 after sunrise
uniform float uSteps;

#define PI 3.14159265
const float R = 1.0;
const float RA = 1.032;
const vec3 BR = vec3(12.3, 28.6, 70.2);
const float BM = 44.5;
const vec3 BO = vec3(1.38, 3.99, 0.18);
const float HR = 0.00377;
const float HM = 0.00085;
const float SUN = 22.0;

float hash(vec3 p) { p = fract(p * 0.3183099 + 0.1); p *= 17.0; return fract(p.x * p.y * p.z * (p.x + p.y + p.z)); }
float noise(vec3 x) {
  vec3 i = floor(x); vec3 f = fract(x); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(hash(i), hash(i + vec3(1,0,0)), f.x), mix(hash(i + vec3(0,1,0)), hash(i + vec3(1,1,0)), f.x), f.y),
             mix(mix(hash(i + vec3(0,0,1)), hash(i + vec3(1,0,1)), f.x), mix(hash(i + vec3(0,1,1)), hash(i + vec3(1,1,1)), f.x), f.y), f.z);
}
const mat3 M3 = mat3(0.00, 0.80, 0.60, -0.80, 0.36, -0.48, -0.60, -0.48, 0.64);
float fbm4(vec3 p) { float a = 0.5, s = 0.0; for (int i = 0; i < 4; i++) { s += a * noise(p); p = M3 * p * 2.03; a *= 0.5; } return s / 0.9375; }
float fbm6(vec3 p) { float a = 0.5, s = 0.0; for (int i = 0; i < 6; i++) { s += a * noise(p); p = M3 * p * 2.01; a *= 0.5; } return s / 0.984; }

vec2 sphere(vec3 ro, vec3 rd, float r) {
  float b = dot(ro, rd), c = dot(ro, ro) - r * r, h = b * b - c;
  if (h < 0.0) return vec2(1e9, -1e9);
  h = sqrt(h); return vec2(-b - h, -b + h);
}
vec3 density(vec3 p) {
  float h = max(length(p) - R, 0.0);
  return vec3(exp(-h / HR), exp(-h / HM), max(0.0, 1.0 - abs(h - 0.0118) / 0.0071));
}
// Optical depth from p towards the sun; a planet in the way blocks it (with a soft edge).
vec3 sunDepth(vec3 p) {
  float b = dot(p, uSun);
  float closest = sqrt(max(dot(p, p) - b * b, 0.0));
  float block = (b < 0.0) ? smoothstep(R - 0.0015, R + 0.0015, closest) : 1.0;
  if (block <= 0.0) return vec3(1e4);
  float t1 = sphere(p, uSun, RA).y;
  float ds = t1 / 5.0; vec3 od = vec3(0.0);
  for (int i = 0; i < 5; i++) od += density(p + uSun * ds * (float(i) + 0.5));
  return od * ds + (1.0 - block) * 1e3;
}
vec3 extinction(vec3 od) { return exp(-(BR * od.x + BM * 1.11 * od.y + BO * od.z)); }

vec3 aurora(vec3 ro, vec3 rd, float jit) {
  vec3 col = vec3(0.0);
  vec2 tO = sphere(ro, rd, R + 0.042);
  if (tO.y <= 0.0 || tO.y > 1e8) return col;
  float tIn = max(tO.x, 0.0);
  vec2 tI = sphere(ro, rd, R + 0.004);
  float tOut = (tI.x > 0.0 && tI.x < 1e8) ? tI.x : tO.y;
  float ds = (tOut - tIn) / 20.0;
  for (int i = 0; i < 20; i++) {
    vec3 p = ro + rd * (tIn + ds * (float(i) + jit));
    float l = length(p), h = l - R;
    vec3 n = p / l;
    float lon = atan(dot(n, uPoleB), dot(n, uPoleA));
    float ang = acos(clamp(dot(n, uPole), -1.0, 1.0));
    float w = 0.30 + 0.03 * sin(lon * 4.0 + uTime * 0.11) + 0.014 * sin(lon * 11.0 - uTime * 0.23) + 0.004 * sin(lon * 31.0 + uTime * 0.5);
    float band = exp(-pow((ang - w) / 0.01, 2.0)) + 0.45 * exp(-pow((ang - w - 0.05) / 0.016, 2.0));
    float prof = smoothstep(0.004, 0.009, h) * exp(-(h - 0.009) / 0.013);
    float rays = 0.35 + 0.65 * pow(0.5 + 0.5 * sin(lon * 140.0 + 6.0 * sin(lon * 9.0 + uTime * 0.25)), 2.0);
    vec3 c = mix(vec3(0.12, 1.0, 0.42), vec3(0.9, 0.18, 0.5), smoothstep(0.016, 0.036, h));
    col += c * band * prof * rays;
  }
  col *= ds * 14.0;
  // Airglow: a faint green shell, seen edge-on at the limb.
  vec2 ta = sphere(ro, rd, R + 0.016);
  float tg = ta.x > 0.0 ? ta.x : ta.y;
  if (tg > 0.0 && tg < 1e8) { vec3 n = normalize(ro + rd * tg); col += vec3(0.3, 1.0, 0.45) * 0.01 * pow(1.0 - abs(dot(n, rd)), 12.0); }
  return col;
}

vec3 stars(vec3 d) {
  vec3 g = d * 210.0; vec3 id = floor(g);
  float h = hash(id);
  vec3 c = vec3(0.0);
  if (h > 0.985) {
    vec3 o = vec3(hash(id + 11.3), hash(id + 27.1), hash(id + 5.7));
    float dist = length(g - id - o);
    float b = pow((h - 0.985) / 0.015, 6.0) * 2.5 + 0.04;
    c = mix(vec3(1.0, 0.75, 0.55), vec3(0.65, 0.8, 1.0), hash(id + 3.1)) * b * exp(-dist * dist * 40.0);
  }
  vec3 G = normalize(vec3(0.42, 0.62, -0.66));
  float lat = dot(d, G);
  float band = exp(-lat * lat * 60.0) * (0.55 + 0.45 * fbm4(d * 4.0)) + exp(-lat * lat * 400.0) * 0.6;
  band *= 1.0 - 0.8 * smoothstep(0.5, 0.68, fbm4(d * 14.0 + 3.0)) * exp(-lat * lat * 300.0);
  return c + vec3(0.6, 0.62, 0.75) * band * 0.004;
}

vec3 aces(vec3 x) { return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }

void main() {
  vec2 uv = (gl_FragCoord.xy * 2.0 - uRes) / uRes.y;
  vec3 rd = normalize(uRot * vec3(uv * uFov, -1.0));
  vec3 ro = uCam;
  float jitter = hash(vec3(gl_FragCoord.xy, fract(uTime) * 7.0));

  float tP = sphere(ro, rd, R).x;
  bool ground = tP > 0.0 && tP < 1e8;
  vec2 tA = sphere(ro, rd, RA);
  float t0 = max(tA.x, 0.0), t1 = ground ? tP : tA.y;
  vec3 col = vec3(0.0);
  vec3 T = vec3(1.0);
  float mu = dot(rd, uSun);
  if (t1 > t0) {
    float L = t1 - t0;
    vec3 od = vec3(0.0), sR = vec3(0.0), sM = vec3(0.0);
    for (int i = 0; i < 24; i++) {
      if (float(i) >= uSteps) break;
      // Towards the ground, samples crowd where the air is densest.
      float sv = (float(i) + jitter) / uSteps;
      float tt = ground ? 1.0 - (1.0 - sv) * (1.0 - sv) : sv;
      float ds = ground ? L * 2.0 * (1.0 - sv) / uSteps : L / uSteps;
      vec3 p = ro + rd * (t0 + L * tt);
      vec3 d = density(p) * ds;
      od += d;
      vec3 att = extinction(od + sunDepth(p));
      sR += d.x * att; sM += d.y * att;
    }
    T = extinction(od);
    float pR = 3.0 / (16.0 * PI) * (1.0 + mu * mu);
    float g = 0.8;
    float pM = 3.0 / (8.0 * PI) * ((1.0 - g * g) * (1.0 + mu * mu)) / ((2.0 + g * g) * pow(1.0 + g * g - 2.0 * g * mu, 1.5));
    col = SUN * (sR * BR * pR + sM * BM * pM) * (ground ? 0.45 : 1.0);
    col += aurora(ro, rd, jitter) * uNight * (ground ? 1.0 : 0.6) * (0.6 + 0.4 * T);
  }

  if (ground) {
    vec3 p = ro + rd * tP;
    vec3 n = normalize(p);
    vec3 q = uSpin * n;
    vec3 sq = uSpin * uSun;
    float c = fbm6(q * 1.7 + 0.3);
    float land = smoothstep(0.555, 0.57, c);
    float coast = smoothstep(0.53, 0.555, c) * (1.0 - land);
    float m = fbm4(q * 5.0);
    vec3 alb = mix(vec3(0.035, 0.06, 0.025), vec3(0.2, 0.15, 0.09), smoothstep(0.45, 0.7, m));
    alb = mix(alb, vec3(0.09, 0.08, 0.065), smoothstep(0.66, 0.78, c));
    float ice = smoothstep(0.82, 0.9, abs(q.y) + (m - 0.5) * 0.15);
    alb = mix(alb, vec3(0.75, 0.8, 0.85), ice);
    vec3 ocean = mix(vec3(0.004, 0.012, 0.03), vec3(0.01, 0.05, 0.07), coast);
    alb = mix(ocean, alb, max(land, ice));
    vec3 drift = vec3(uTime * 0.004, 0.0, uTime * 0.002);
    float cl = smoothstep(0.5, 0.78, fbm4(q * 3.4 + drift + fbm4(q * 7.0) * 0.6));
    float clS = smoothstep(0.5, 0.78, fbm4((q + sq * 0.012) * 3.4 + drift + fbm4((q + sq * 0.012) * 7.0) * 0.6));

    float ndl = dot(n, uSun);
    vec3 sunT = extinction(sunDepth(p + n * 0.0004));
    vec3 Ls = SUN * sunT * max(ndl, 0.0);
    // Faint moonlight on the night side, so the planet reads as a sphere.
    vec3 moon = normalize(vec3(-0.6, 0.7, 0.4));
    vec3 Lm = vec3(0.03, 0.04, 0.06) * max(dot(n, moon), 0.0) * uNight;
    vec3 sky = SUN * vec3(0.006, 0.012, 0.025) * smoothstep(-0.12, 0.25, ndl);
    vec3 surf = alb * (Ls * (1.0 - 0.7 * clS) + sky + Lm) / PI;
    // Ocean glint: GGX-ish lobe with Fresnel, coloured by the sunlight that reaches it.
    vec3 v = -rd; vec3 hv = normalize(uSun + v);
    float nh = max(dot(n, hv), 0.0);
    float fres = 0.02 + 0.98 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    float rough = 0.06;
    float a2 = rough * rough;
    float dd = nh * nh * (a2 - 1.0) + 1.0;
    float spec = a2 / (PI * dd * dd);
    surf += (1.0 - max(land, ice)) * (1.0 - cl) * Ls * fres * spec * 0.25;
    // City lights: clustered along coasts and lowlands, only in the dark.
    float dark = smoothstep(0.08, -0.12, ndl);
    float cluster = smoothstep(0.42, 0.7, fbm4(q * 9.0)) * land * (1.0 - smoothstep(0.6, 0.72, c)) * (1.0 - ice);
    float pts = pow(noise(q * 140.0), 7.0) * 9.0 + pow(noise(q * 42.0), 4.0) * 1.6;
    vec3 city = vec3(1.0, 0.6, 0.26) * cluster * pts * dark * 0.09 * (1.0 - 0.85 * cl);
    // Clouds on top: lit by the sun (with the same transmittance) and by the moon.
    vec3 cloudCol = vec3(0.92) * (Ls + sky * 1.6 + Lm * 2.0) / PI;
    surf = mix(surf, cloudCol, cl * 0.95);
    col += T * (surf * 1.8 + city);
  } else {
    float cosS = 0.99993;
    float disk = smoothstep(cosS - 0.00003, cosS + 0.00001, mu);
    col += T * (stars(rd) * (1.0 - 0.9 * (1.0 - uNight)) + disk * vec3(1.0, 0.97, 0.92) * 2400.0);
  }

  // Starburst and halo in view space, where the sun is.
  vec2 dv = uv - uSunScr.xy;
  float r = length(dv);
  float a = atan(dv.y, dv.x);
  float spikes = pow(abs(cos(a * 3.0)), 220.0) + pow(abs(cos(a * 3.0 + 1.0472 * 0.5)), 600.0) * 0.6 + pow(abs(cos(a * 9.0 + 0.3)), 80.0) * 0.15;
  vec3 burst = vec3(1.0, 0.86, 0.68) * (spikes * 0.06 / (r * 6.0 + 0.04) + 0.08 * exp(-r * 9.0) + 0.02 / (r * 30.0 + 0.2));
  col += burst * uSunScr.z;

  col = aces(col * uExposure);
  col = pow(col, vec3(1.0 / 2.2));
  col += (jitter - 0.5) / 255.0;
  gl_FragColor = vec4(col, 1.0);
}`;

const TAU = Math.PI * 2;
const norm = (v) => { const l = Math.hypot(v[0], v[1], v[2]); return [v[0] / l, v[1] / l, v[2] / l]; };
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const mix = (a, b, t) => a + (b - a) * t;
const add = (a, b, k = 1) => [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k];
const scale = (a, k) => [a[0] * k, a[1] * k, a[2] * k];
const smooth = (x) => x * x * (3 - 2 * x);

// The orbit. u = 0 just after midnight, u = 1 the sun has cleared the limb.
const PHI0 = -1.5, PHI1 = -0.565, PHI2 = -0.4, URISE = 0.84;
const phiAt = (u) => (u < URISE ? mix(PHI0, PHI1, u / URISE) : mix(PHI1, PHI2, (u - URISE) / (1 - URISE)));
const SUN = norm([0.3, 0, 1]);
const frameAt = (phi) => {
  const U = [0, Math.cos(phi), Math.sin(phi)];
  const Tg = [0, -Math.sin(phi), Math.cos(phi)];
  const Rt = cross(Tg, U);
  return { U, Tg, Rt };
};
// The yaw that puts the sun in the middle of the view at the end.
const YAW1 = (() => { const f = frameAt(PHI2); return Math.atan2(dot(SUN, f.Rt), dot(SUN, f.Tg)); })();
// The aurora oval: around a magnetic pole off to the left of the night-time view.
const POLE = (() => {
  // The oval's near edge passes below the middle of the view early in the orbit.
  const c = camera(0.18);
  const fov = Math.tan((25 * Math.PI) / 180);
  const rd = norm(add(add(c.F, c.Rv, -0.25 * fov), c.Up, -0.4 * fov));
  const b = dot(c.pos, rd), h = b * b - (dot(c.pos, c.pos) - 1);
  const p = add(c.pos, rd, -b - Math.sqrt(Math.max(h, 0)));
  const away = norm(add(c.F, p, -dot(c.F, p)));
  return norm(add(scale(p, Math.cos(0.33)), away, Math.sin(0.33)));
})();
const POLE_A = norm(cross(POLE, [0, 1, 0]));
const POLE_B = cross(POLE, POLE_A);

export function camera(u, tiltX = 0, tiltY = 0, small = false) {
  const e = smooth(Math.min(1, Math.max(0, u)));
  const phi = phiAt(Math.min(1, Math.max(0, u)));
  const d = mix(small ? 1.75 : 1.6, 1.2, e);
  const { U, Tg, Rt } = frameAt(phi);
  const dip = Math.acos(1 / d);
  const yaw = mix(YAW1 - 0.75, YAW1, smooth(Math.min(1, u * 1.15))) + tiltX;
  const lift = mix(-0.04, small ? 0.06 : 0.14, e) + tiltY;
  const pitch = -dip + lift;
  const hz = add(scale(Tg, Math.cos(yaw)), Rt, Math.sin(yaw));
  const F = norm(add(scale(hz, Math.cos(pitch)), U, Math.sin(pitch)));
  const Rv = norm(cross(F, U));
  const Up = cross(Rv, F);
  return { pos: scale(U, d), F, Rv, Up, d };
}

export function createPlanet(canvas, { small = false } = {}) {
  const gl = canvas.getContext('webgl', { antialias: false, alpha: false, depth: false, stencil: false, powerPreference: 'high-performance', preserveDrawingBuffer: false });
  if (!gl) return null;
  const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
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
  const loc = {};
  for (const n of ['uRes', 'uCam', 'uRot', 'uFov', 'uSun', 'uSunScr', 'uTime', 'uExposure', 'uPole', 'uPoleA', 'uPoleB', 'uSpin', 'uNight', 'uSteps']) loc[n] = gl.getUniformLocation(prog, n);
  gl.uniform3fv(loc.uSun, SUN);
  gl.uniform3fv(loc.uPole, POLE);
  gl.uniform3fv(loc.uPoleA, POLE_A);
  gl.uniform3fv(loc.uPoleB, POLE_B);

  // Adaptive resolution: a pixel budget that follows the measured frame time.
  let quality = small ? 0.55 : 0.75;
  const maxQ = small ? 0.75 : 1;
  let W = 0, H = 0, avg = 16.7, last = 0, slow = 0, fast = 0;
  const size = () => {
    const dpr = Math.min(devicePixelRatio || 1, 2);
    const cap = small ? 520_000 : 1_400_000;
    let w = innerWidth * dpr * quality, h = innerHeight * dpr * quality;
    const k = Math.min(1, Math.sqrt(cap / (w * h)));
    w = Math.max(64, Math.round(w * k)); h = Math.max(64, Math.round(h * k));
    if (w !== W || h !== H) { W = canvas.width = w; H = canvas.height = h; gl.viewport(0, 0, W, H); }
  };
  size();
  addEventListener('resize', size);

  const state = { u: 0, tiltX: 0, tiltY: 0, time: 0 };
  const stats = { w: 0, h: 0, quality: 0, ms: 0 };
  const render = (s = state) => {
    const now = performance.now();
    if (last) {
      const dt = now - last;
      if (dt < 100) {
        avg = avg * 0.92 + dt * 0.08;
        if (avg > 19) { if (++slow > 20) { quality = Math.max(0.3, quality * 0.88); slow = 0; avg = 16.7; size(); } } else slow = 0;
        if (avg < 15 && quality < maxQ) { if (++fast > 120) { quality = Math.min(maxQ, quality * 1.08); fast = 0; size(); } } else fast = 0;
      }
    }
    last = now;
    const cam = camera(s.u, s.tiltX, s.tiltY, small);
    const fov = Math.tan(((small ? 30 : 25) * Math.PI) / 180);
    gl.uniform2f(loc.uRes, W, H);
    gl.uniform3fv(loc.uCam, cam.pos);
    gl.uniformMatrix3fv(loc.uRot, false, [...cam.Rv, ...cam.Up, ...scale(cam.F, -1)]);
    gl.uniform1f(loc.uFov, fov);
    // Where the sun is in the view, and how much of it clears the limb.
    const sx = dot(SUN, cam.Rv), sy = dot(SUN, cam.Up), sz = dot(SUN, cam.F);
    const nadir = norm(scale(cam.pos, -1));
    const limb = Math.asin(1 / cam.d);
    const sep = Math.acos(Math.max(-1, Math.min(1, dot(SUN, nadir))));
    const clear = sep - limb; // radians above the limb
    const vis = sz > 0 ? Math.max(0, Math.min(1, (clear + 0.004) / 0.012)) : 0;
    const burst = vis * (0.55 + 1.6 * Math.exp(-Math.max(0, clear) * 60));
    gl.uniform3f(loc.uSunScr, sz > 0 ? sx / sz / fov : 99, sz > 0 ? sy / sz / fov : 99, burst);
    gl.uniform1f(loc.uTime, s.time);
    const night = Math.max(0, Math.min(1, (0.98 - s.u) / 0.35));
    gl.uniform1f(loc.uNight, night);
    const day = Math.max(0, Math.min(1, (s.u - 0.84) / 0.16));
    gl.uniform1f(loc.uExposure, mix(mix(1.0, 2.4, night), 0.6, smooth(day)));
    const a = s.time * 0.006 + 2.6, c = Math.cos(a), sn = Math.sin(a);
    gl.uniformMatrix3fv(loc.uSpin, false, [c, 0, -sn, 0, 1, 0, sn, 0, c]);
    gl.uniform1f(loc.uSteps, quality < 0.45 ? 10 : small ? 12 : 16);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    stats.w = W; stats.h = H; stats.quality = quality; stats.ms = avg;
  };
  return { render, state, stats, resize: size, canvas };
}

// Draw the first frame now, before anything else has loaded.
const canvas = document.getElementById('planet');
const smallScreen = innerWidth < 760;
let planet = null;
try { planet = canvas && createPlanet(canvas, { small: smallScreen }); } catch (err) { console.warn('planet:', err); }
if (planet) { planet.render(); canvas.classList.add('on'); }
window.__planet = planet;
export { planet, TAU };
