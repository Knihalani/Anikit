// The sky: one fragment shader. Aurora curtains are ray-marched as stacked
// altitude slices of an emission field (no absorption), folded by curl noise;
// the lake is a cracked, glossy ice plane that reflects the same field.
// This module has no dependencies so the first frame paints before anything else loads.
// Every parameter lives in `sky.params`; main.js drives them with anikit.

const VERT = `attribute vec2 p; void main(){ gl_Position = vec4(p, 0., 1.); }`;

const FRAG = `
precision highp float;
uniform vec2 uRes;
uniform float uT;          // aurora time (s)
uniform vec3 uCam;         // camera position
uniform vec2 uLook;        // yaw, pitch (rad)
uniform float uFov;        // focal length
uniform float uStorm;      // 0 quiet → 1 storm
uniform float uRed;        // red 630 nm upper layer
uniform float uBandZ;      // distance of the main band
uniform float uCorona;     // 0 → 1 corona overhead
uniform float uExposure;
uniform float uNight;      // 0 dusk-ish → 1 deep night
uniform float uSteps;      // slices (quality)
uniform vec3 uAmb;         // aurora ambient light (CPU estimate)

#define TAU 6.2831853

float h12(vec2 p){ vec3 p3 = fract(vec3(p.xyx) * .1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
vec2 h22(vec2 p){ vec3 p3 = fract(vec3(p.xyx) * vec3(.1031, .1030, .0973)); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.xx + p3.yz) * p3.zy); }
float vnoise(vec2 p){
  vec2 i = floor(p), f = fract(p); vec2 u = f * f * (3. - 2. * f);
  return mix(mix(h12(i), h12(i + vec2(1, 0)), u.x), mix(h12(i + vec2(0, 1)), h12(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p){ float a = .5, s = 0.; for (int i = 0; i < 4; i++){ s += a * vnoise(p); p = p * 2.03 + 17.1; a *= .5; } return s; }
float n1(float x){ float i = floor(x), f = fract(x); f = f * f * (3. - 2. * f); return mix(h12(vec2(i, 1.7)), h12(vec2(i + 1., 1.7)), f); }

vec2 n1d(float x){ float i = floor(x), f = fract(x); float a = h12(vec2(i, 1.7)), b = h12(vec2(i + 1., 1.7)); float u = f * f * (3. - 2. * f); return vec2(mix(a, b, u), (b - a) * 6. * f * (1. - f)); }
// Curl of a scalar noise field: divergence-free folding of the curtains.
vec2 curl(vec2 p){
  const float e = .35;
  float a = vnoise(p + vec2(0, e)), b = vnoise(p - vec2(0, e));
  float c = vnoise(p + vec2(e, 0)), d = vnoise(p - vec2(e, 0));
  return vec2(a - b, d - c) / (2. * e);
}

const vec3 GREEN = vec3(.20, 1., .42);   // O 557.7 nm
const vec3 RED   = vec3(1., .10, .16);   // O 630.0 nm
const vec3 VIOLET= vec3(.55, .22, 1.);   // N2+ fringe

// Sheet k: signed horizontal distance from the curtain centre line (folded by curl noise).
float sheetD(vec2 s, float fk, float T, out float sx, out float slope){
  sx = s.x;
  float a = s.x * .0016 + T * .022 + fk * 7.;
  vec2 na = n1d(a);
  float zc = uCam.z + uBandZ + fk * 190. - 110. * fk * fk
           + 300. * (na.x - .5)
           + 45. * sin(s.x * .0062 + T * .28 + fk);
  slope = 300. * .0016 * na.y + 45. * .0062 * cos(s.x * .0062 + T * .28 + fk);
  return s.y - zc;
}

// Light emitted where a ray crosses a curtain: altitude profile × rays along the band.
vec3 curtainLight(float sx, float y, float fk, float T, float dist){
  float base = 105. + 28. * (n1(sx * .007 + T * .05 + fk * 3.) - .5) * 2.;
  float hgt = (y - base) / (120. + 190. * uStorm);
  if (hgt < -.1) return vec3(0.);
  float ray = vnoise(vec2(sx * .05 + T * .22, fk * 5.));
  ray = mix(.5, .12 + 1.1 * ray * ray * ray, clamp(1.6 - dist / 3000., 0., 1.));
  ray *= .55 + .45 * mix(.5, vnoise(vec2(sx * .19 - T * .7, 3. + fk)), clamp(1.5 - dist / 900., 0., 1.));
  float pulse = .7 + .3 * sin(sx * .004 - T * .9 + fk * 2.);
  float edge = smoothstep(-.1, .0, hgt);
  float g = edge * exp(-max(hgt, 0.) * 3.);
  float r = smoothstep(.2, .65, hgt) * exp(-max(hgt - .65, 0.) * 1.8) * uRed;
  float v = exp(-pow((hgt + .03) / .045, 2.)) * (.3 + .7 * uStorm);
  return ray * pulse * (GREEN * g + RED * r * .85 + VIOLET * v * .8);
}

vec3 aurora(vec3 ro, vec3 rd, float steps, float dither){
  if (rd.y < .003) return vec3(0.);
  float y0 = 75., y1 = 560.;
  float t0 = max((y0 - ro.y) / rd.y, 0.), t1 = min((y1 - ro.y) / rd.y, 7000.);
  if (t1 <= t0) return vec3(0.);
  vec3 acc = vec3(0.);
  float dt = (t1 - t0) / steps;
  float T = uT;
  float pd0 = 0., pd1 = 0., pd2 = 0., px0 = 0., px1 = 0., px2 = 0.;
  for (int i = 0; i <= 64; i++){
    if (float(i) > steps) break;
    float t = t0 + (float(i) + dither - .5) * dt;
    vec3 p = ro + rd * t;
    float fade = exp(-t / 3000.);
    vec2 sq = p.xz + curl(p.xz * .0026 + vec2(T * .03, -T * .018)) * (40. + 35. * uStorm);
    for (int k = 0; k < 3; k++){
      float fk = float(k);
      float amp = (k == 0 ? 1. : (k == 1 ? .3 + .8 * uStorm : uStorm * .85)) * (1. - uCorona);
      if (amp < .01) continue;
      float sx, sl;
      float d = sheetD(sq, fk, T, sx, sl);
      float pd = k == 0 ? pd0 : (k == 1 ? pd1 : pd2);
      float px = k == 0 ? px0 : (k == 1 ? px1 : px2);
      if (i > 0 && d * pd <= 0.){
        // crossed the sheet between the last sample and this one
        float f = pd / (pd - d);
        float tc = t - dt * (1. - f);
        vec3 pc = ro + rd * tc;
        float th = 7. + 6. * uStorm;
        float cosA = abs(rd.z - sl * rd.x) / sqrt(1. + sl * sl) / max(length(rd.xz), .05);
        float L = min(th / max(cosA, .06), th * 7.) + 22.;
        acc += curtainLight(mix(px, sx, f), pc.y, fk, T, tc) * amp * L * fade;
      }
      // a soft glow around each sheet
      if (k == 0) { pd0 = d; px0 = sx; } else if (k == 1) { pd1 = d; px1 = sx; } else { pd2 = d; px2 = sx; }
    }
    if (uCorona > .001){
      vec2 dd = p.xz - uCam.xz; float rr = length(dd), an = atan(dd.y, dd.x);
      float rays = vnoise(vec2(an * 9. + T * .12 + rr * .003, T * .15)); rays = pow(rays, 4.) * 3.;
      rays *= .5 + .5 * vnoise(vec2(an * 34. - T * .45, rr * .008));
      float ring = smoothstep(30., 140., rr) * exp(-rr / 380.);
      float hgt = (p.y - 105.) / 320.;
      float g = smoothstep(-.06, .03, hgt) * exp(-max(hgt, 0.) * 2.6);
      float r = smoothstep(.3, .8, hgt) * uRed;
      acc += uCorona * ring * rays * (GREEN * g * 1.6 + RED * r * .38 + VIOLET * exp(-pow(hgt / .12, 2.)) * .3) * dt * .019;
    }
  }
  // extinction: aurora low on the horizon is dimmed by a long path through the air
  return acc * .028 * (.6 + .8 * uStorm) * smoothstep(.0, .16, rd.y + .02 * uCorona);
}

vec3 stars(vec3 rd){
  vec3 col = vec3(0.);
  // octahedral-ish: three projections avoid the pole pinch
  vec2 uv = vec2(atan(rd.x, rd.z) / TAU, asin(clamp(rd.y, -1., 1.)) / 3.14159);
  for (int l = 0; l < 3; l++){
    float sc = 180. + float(l) * 160.;
    vec2 g = uv * vec2(sc * 2., sc);
    vec2 id = floor(g), f = fract(g);
    vec2 o = h22(id + float(l) * 31.);
    float m = h12(id * 1.3 + float(l));
    if (m > .62 - float(l) * .1){
      float b = pow(h12(id + 7.), 9.) * 6. + .06;
      float d = length(f - o) * sc / 180.;
      float s = exp(-d * d * 2600.) * b;
      float tw = .75 + .25 * sin(uT * (2. + 5. * m) + m * 40.);
      vec3 tint = mix(vec3(1., .78, .6), vec3(.7, .82, 1.), h12(id + 3.1));
      col += s * tw * tint;
    }
  }
  // the Milky Way: a faint dusty band
  vec3 axis = normalize(vec3(.4, .55, .7));
  float band = exp(-pow(dot(rd, axis) / .22, 2.));
  float dust = fbm(uv * vec2(24., 12.));
  col += vec3(.55, .6, .75) * band * dust * dust * .09;
  return col;
}

float ridge(float az, float scale, float seed){
  float s = 0., a = .5, f = scale;
  for (int i = 0; i < 5; i++){ float n = 1. - abs(2. * vnoise(vec2(az * f, seed)) - 1.); s += a * n * n; f *= 2.1; a *= .48; }
  return s;
}

// Sky without the aurora: gradient, airglow, stars, mountains. el: elevation sine.
vec3 skyBase(vec3 rd, out float mountain){
  float e = rd.y;
  vec3 top = mix(vec3(.010, .016, .036), vec3(.002, .004, .011), uNight);
  vec3 hor = mix(vec3(.018, .026, .04), vec3(.006, .01, .016), uNight);
  vec3 col = mix(hor, top, smoothstep(0., .5, e));
  col += uAmb * .04 * exp(-max(e, 0.) * 5.);           // aurora light scattered in the air
  col += vec3(.02, .05, .03) * exp(-max(e, 0.) * 30.) * .4; // airglow
  col += stars(rd) * smoothstep(-.01, .08, e) * (.55 + .45 * uNight);
  mountain = 0.;
  float az = atan(rd.x, rd.z);
  float far = .03 + .13 * pow(ridge(az * .7 + 1., 1.3, 4.), 1.5);
  float near = .004 + .2 * pow(ridge(az * .55 + 3., 1.0, 9.), 2.2) * smoothstep(.2, .6, vnoise(vec2(az * .9, 2.)));
  if (e < far){
    float lit = smoothstep(far - .012, far, e) * (.6 + .4 * vnoise(vec2(az * 12., 1.)));
    vec3 m = vec3(.005, .007, .011) + uAmb * .012 + (vec3(.02, .025, .03) + uAmb * .06) * lit;
    col = mix(col, m, .94); mountain = 1.;
  }
  if (e < near){
    float lit = smoothstep(near - .015, near, e) * (.5 + .5 * vnoise(vec2(az * 14., 3.)));
    vec3 m = vec3(.002, .003, .005) + uAmb * .008 + (vec3(.03, .036, .045) + uAmb * .1) * lit;
    col = m; mountain = 1.;
  }
  return col;
}

vec2 voronoi(vec2 p){
  vec2 n = floor(p), f = fract(p), mg, mr; float md = 8.;
  for (int j = -1; j <= 1; j++) for (int i = -1; i <= 1; i++){
    vec2 g = vec2(i, j), o = h22(n + g), r = g + o - f; float d = dot(r, r);
    if (d < md){ md = d; mr = r; mg = g; }
  }
  md = 8.;
  for (int j = -2; j <= 2; j++) for (int i = -2; i <= 2; i++){
    vec2 g = mg + vec2(i, j), o = h22(n + g), r = g + o - f;
    if (dot(mr - r, mr - r) > .00001) md = min(md, dot(.5 * (mr + r), normalize(r - mr)));
  }
  return vec2(md, h12(n + mg));
}

vec3 aces(vec3 x){ return clamp((x * (2.51 * x + .03)) / (x * (2.43 * x + .59) + .14), 0., 1.); }

void main(){
  vec2 uv = (gl_FragCoord.xy - .5 * uRes) / min(uRes.x, uRes.y * 1.25);
  float cy = cos(uLook.x), sy = sin(uLook.x), cp = cos(uLook.y), sp = sin(uLook.y);
  vec3 fw = vec3(sy * cp, sp, cy * cp);
  vec3 rt = normalize(vec3(cy, 0., -sy));
  vec3 up = cross(fw, rt);
  vec3 rd = normalize(fw * uFov + rt * uv.x + up * uv.y);
  vec3 ro = uCam;
  float dither = .5;
  vec3 col;
  if (rd.y > 0.){
    float mtn;
    col = skyBase(rd, mtn);
    if (mtn < .5) col += aurora(ro, rd, uSteps, dither);
    // ground fog near the horizon
    col = mix(col, vec3(.004, .006, .009) + uAmb * .035, exp(-rd.y * 40.) * .25);
  } else {
    float t = -ro.y / rd.y;
    vec3 p = ro + rd * t;
    vec2 g = p.xz;
    float lod = clamp(t / 70., 0., 1.);
    // cracks: two voronoi scales, sharp lines with frosted edges
    vec2 gw = g + (vec2(vnoise(g * .09), vnoise(g * .09 + 7.)) - .5) * 9.;
    vec2 v1 = voronoi(gw * .13);
    vec2 v2 = voronoi(gw * .6 + 7.);
    float aa = .02 + t * .0012;
    float c1 = 1. - smoothstep(0., aa * 2.2, v1.x);
    float c2 = (1. - smoothstep(0., aa * 3., v2.x)) * step(.45, v2.y) * (1. - lod);
    float crack = max(c1 * smoothstep(.35, .65, vnoise(g * .12 + 4.)), c2 * .6) * (1. - lod);
    float frost = smoothstep(.45, .8, fbm(g * .05 + 3.));
    float rough = .003 + .025 * frost + .02 * crack;
    // normal: gentle undulation + tilt into cracks; fades with distance
    vec2 grad = vec2(vnoise(g * .7) - .5, vnoise(g * .7 + 5.) - .5) * .012 * (1. - lod);
    grad += (vec2(vnoise(g * 3.1), vnoise(g * 3.1 + 9.)) - .5) * rough * 1.6 * (1. - lod);
    vec3 n = normalize(vec3(grad.x, 1., grad.y));
    vec3 rr = reflect(rd, n);
    rr.y = abs(rr.y) + .002;
    float mtn;
    vec3 refl = skyBase(rr, mtn);
    if (mtn < .5) refl += aurora(p, rr, max(10., uSteps * .45), dither);
    float cosT = clamp(-dot(rd, n), 0., 1.);
    float F = .02 + .98 * pow(1. - cosT, 5.);
    F = mix(F, .5, .0) ;
    vec3 base = vec3(.002, .004, .007) + uAmb * .012;
    col = base * .5 + refl * mix(.42, 1., F) * (1. - frost * .35);
    // frosted patches and crack walls scatter the aurora light
    vec3 lit = vec3(.05, .06, .075) * (.3 + .7 * uNight) + uAmb * .5;
    col += lit * (frost * .03 + crack * .2);
    // ground fog: breathes slowly over the ice
    float fogN = fbm(g * .03 + vec2(uT * .04, -uT * .03));
    float breath = .75 + .25 * sin(uT * .35 + fogN * 4.);
    float fog = (1. - exp(-t * .006)) * (.25 + .55 * fogN) * breath;
    col = mix(col, vec3(.004, .006, .009) + uAmb * .035, clamp(fog, 0., 1.) * .35);
  }
  col *= uExposure;
  col = max(col - .008, 0.) * 1.02;   // black point: night ice and rock stay black
  col = aces(col);
  col = pow(col, vec3(1. / 2.2));
  vec2 q = gl_FragCoord.xy / uRes;
  col *= .55 + .45 * pow(16. * q.x * q.y * (1. - q.x) * (1. - q.y), .22);
  col += (h12(gl_FragCoord.xy + uT) - .5) / 160.;
  gl_FragColor = vec4(col, 1.);
}`;

export const sky = {
  params: {
    time: 0, storm: 0.15, red: 0.25, bandZ: 1400, corona: 0, exposure: 1.6, night: 0.2,
    camX: 0, camY: 4, camZ: 0, yaw: 0, pitch: 0.16, fov: 1.15,
  },
  stats: { scale: 0, ms: 0, frames: 0, w: 0, h: 0 },
  running: true,
  ok: false,
};
window.__sky = sky;

const canvas = document.getElementById('sky');
const gl = canvas && canvas.getContext('webgl', { antialias: false, alpha: false, depth: false, powerPreference: 'high-performance', preserveDrawingBuffer: false });
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const small = Math.min(innerWidth, innerHeight) < 700;

if (gl) {
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
  const U = {};
  for (const n of ['uRes', 'uT', 'uCam', 'uLook', 'uFov', 'uStorm', 'uRed', 'uBandZ', 'uCorona', 'uExposure', 'uNight', 'uSteps', 'uAmb']) U[n] = gl.getUniformLocation(prog, n);

  // Adaptive resolution: a pixel budget that follows the measured frame interval.
  const dpr = Math.min(devicePixelRatio || 1, 2);
  const forced = +new URLSearchParams(location.search).get('scale') || 0;
  let scale = forced || (small ? 0.5 : 0.6);
  const maxScale = Math.min(1, dpr * 0.75);
  let steps = small ? 20 : 30;
  const resize = () => {
    const w = Math.max(64, Math.round(innerWidth * scale)), h = Math.max(64, Math.round(innerHeight * scale));
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; gl.viewport(0, 0, w, h); }
    sky.stats.w = w; sky.stats.h = h; sky.stats.scale = scale;
  };
  resize();
  addEventListener('resize', resize);

  const draw = () => {
    const P = sky.params;
    const s = P.storm, r = P.red;
    const amb = [(0.2 + r * 0.6) * (0.3 + s), (1 - r * 0.3) * (0.3 + s), (0.42 + 0.3 * s) * (0.3 + s)].map((v) => v * 0.35);
    gl.uniform2f(U.uRes, canvas.width, canvas.height);
    gl.uniform1f(U.uT, P.time);
    gl.uniform3f(U.uCam, P.camX, P.camY, P.camZ);
    gl.uniform2f(U.uLook, P.yaw, P.pitch);
    gl.uniform1f(U.uFov, P.fov);
    gl.uniform1f(U.uStorm, s);
    gl.uniform1f(U.uRed, r);
    gl.uniform1f(U.uBandZ, P.bandZ);
    gl.uniform1f(U.uCorona, P.corona);
    gl.uniform1f(U.uExposure, P.exposure);
    gl.uniform1f(U.uNight, P.night);
    gl.uniform1f(U.uSteps, steps);
    gl.uniform3f(U.uAmb, amb[0], amb[1], amb[2]);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    sky.stats.frames++;
  };
  sky.draw = draw;
  sky.ambient = () => { const P = sky.params; return [0.2 + P.red * 0.8, 1 - P.red * 0.55, 0.45 + 0.25 * P.storm]; };
  draw(); // the static first frame, before anything else has loaded
  sky.ok = true;

  let last = performance.now(), acc = 0, n = 0, lastKey = '';
  const loop = (now) => {
    requestAnimationFrame(loop);
    const dt = Math.min(now - last, 100);
    last = now;
    if (document.hidden || !sky.running) return;
    if (!reduced) sky.params.time += dt / 1000;
    // Under reduced motion only redraw when the scroll moved something.
    const P = sky.params;
    const key = reduced ? `${P.camZ.toFixed(1)}|${P.pitch.toFixed(3)}|${P.storm.toFixed(3)}|${P.yaw.toFixed(3)}|${canvas.width}` : '';
    if (reduced && key === lastKey) return;
    lastKey = key;
    draw();
    // Adapt every ~0.5 s: >19 ms frames → fewer pixels; <13 ms → more.
    acc += dt; n++;
    if (acc > 500 && !forced) {
      const ms = acc / n; sky.stats.ms = ms; acc = 0; n = 0;
      if (ms > 19 && scale > 0.3) { scale = Math.max(0.3, scale * 0.85); if (scale < 0.4) steps = Math.max(18, steps - 4); resize(); }
      else if (ms < 13 && scale < maxScale) { scale = Math.min(maxScale, scale * 1.08); resize(); }
    }
  };
  requestAnimationFrame(loop);
  document.addEventListener('visibilitychange', () => { last = performance.now(); });
} else {
  document.documentElement.classList.add('no-gl');
}
