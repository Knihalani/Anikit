// The water: one full-screen fragment shader. It knows nothing about Anikit;
// main.js feeds it the camera depth (0 at the surface, 1 at the bottom of the
// page) and the descent speed every frame.
//
// Light model, in "metres" of a toy ocean:
//  - sunlight enters through a wavy surface; each channel is absorbed at its own
//    rate (red first, then green), so colour drains with depth and distance;
//  - god rays: a short march along the view ray, each sample lit by the wave
//    pattern projected up the sun direction (volumetric shafts);
//  - caustics on the sand, faded by the light that reaches the floor;
//  - marine snow: three parallax layers of hashed points, streaking with speed.

const VERT = `attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}`;

const FRAG = `precision highp float;
uniform vec2 uRes;
uniform float uT, uDepth, uSpeed;
uniform vec2 uPtr;

const vec3 ABS = vec3(0.42, 0.075, 0.032);   // absorption per metre: red, green, blue
const vec3 SCAT = vec3(0.10, 0.42, 0.62);    // in-scattering colour of the water

float h21(vec2 p){ p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(vec2 p){
  vec2 i = floor(p), f = fract(p); f = f * f * (3. - 2. * f);
  return mix(mix(h21(i), h21(i + vec2(1, 0)), f.x), mix(h21(i + vec2(0, 1)), h21(i + vec2(1, 1)), f.x), f.y);
}
// Wave light pattern on the surface: bright where the waves focus light.
float waves(vec2 p, float t){
  float a = vnoise(p * 0.55 + vec2(t * 0.11, t * 0.07));
  float b = vnoise(p * 1.3 - vec2(t * 0.09, -t * 0.13) + a * 1.7);
  float c = 1. - abs(b * 2. - 1.);
  return pow(c, 5.);
}
// Caustics: an iterated, self-warping sine field (sharp bright ridges).
float caustic(vec2 p, float t){
  vec2 i = p; float c = 0.;
  for (int n = 0; n < 4; n++) {
    float tt = t * (1. - 3.0 / float(n + 2));
    i = p + vec2(cos(tt - i.x) + sin(tt + i.y), sin(tt - i.y) + cos(tt + i.x));
    c += 1. / length(vec2(p.x / (sin(i.x + tt) * 180.), p.y / (cos(i.y + tt) * 180.)));
  }
  c = 1.2 - pow(c / 4., 1.35);
  return pow(clamp(abs(c), 0., 1.), 7.);
}

void main(){
  vec2 uv = (gl_FragCoord.xy * 2. - uRes) / uRes.y;
  float t = uT;
  float D = uDepth;
  // Camera: looks up a little at the surface in the hero, then levels out.
  float pitch = mix(0.06, -0.04, smoothstep(0.0, 0.16, D));
  vec3 rd = normalize(vec3(uv.x + uPtr.x * 0.04, uv.y + uPtr.y * 0.03, 1.55));
  float cp = cos(pitch), sp = sin(pitch);
  rd = vec3(rd.x, rd.y * cp + rd.z * sp, -rd.y * sp + rd.z * cp);

  float depthM = 2.2 + D * D * 120.;          // camera depth in metres
  vec3 sunDir = normalize(vec3(0.18, 1., 0.32)); // towards the sun (inside the water)
  // Light left at the camera; past the twilight zone nothing gets through.
  vec3 sunAtCam = exp(-ABS * depthM / sunDir.y) * (1. - smoothstep(0.28, 0.66, D));
  float lightLeft = dot(sunAtCam, vec3(0.2, 0.5, 0.3));

  vec3 col = vec3(0.);
  if (lightLeft > 0.0005) {
    // Ambient water colour: brighter looking up.
    float up = rd.y * 0.5 + 0.5;
    vec3 amb = SCAT * (0.10 + 0.9 * pow(up, 2.2)) * sunAtCam;
    col = amb;

    // Surface (Snell's window) when looking up.
    if (rd.y > 0.) {
      float ts = depthM / rd.y;
      vec2 sp2 = rd.xz * ts;
      float win = smoothstep(0.62, 0.86, rd.y);
      float w = waves(sp2 * 0.6, t * 1.6);
      vec3 surf = vec3(0.75, 0.95, 1.0) * (0.35 + 1.4 * w) * win;
      col += surf * exp(-ABS * ts) * 1.2;
    }

    // Sea floor: a plane below the camera, falling away as we descend.
    float floorY = 2.6 + D * 70.;
    float tf = rd.y < 0. ? floorY / -rd.y : 1e5;

    // God rays: shafts parallel to the sun, so seen from inside they fan out
    // from the sun's point. Their pattern is the wave focus pattern carried
    // around that point; they're stronger looking towards the sun and over
    // longer paths, and fade with the light left.
    vec3 ax = normalize(cross(sunDir, vec3(0., 0., 1.)));
    vec3 ay = cross(sunDir, ax);
    vec2 dir2 = normalize(vec2(dot(rd, ax), dot(rd, ay)) + 1e-5);
    float toward = dot(rd, sunDir) * 0.5 + 0.5;
    float s1 = vnoise(dir2 * 7. + vec2(t * 0.16, -t * 0.11));
    float s2 = vnoise(dir2 * 19. + vec2(-t * 0.23, t * 0.2) + 7.);
    float s3 = vnoise(dir2 * 41. + vec2(t * 0.31, t * 0.17) + 3.);
    float sh = pow(clamp(s1 * 0.55 + s2 * 0.3 + s3 * 0.15, 0., 1.), 3.2) * 2.6;
    float pathLen = min(tf, 30.);
    float vol = 1. - exp(-pathLen * 0.12);
    col += sh * vol * pow(toward, 2.) * vec3(0.55, 0.9, 1.0) * sunAtCam * 1.1 * (1. - 0.5 * smoothstep(0.86, 0.98, toward));

    // Sand with caustics.
    if (tf < 1e4) {
      vec3 hp = rd * tf;
      vec2 sp3 = hp.xz;
      float sand = 0.6 + 0.25 * vnoise(sp3 * 2.0) + 0.15 * vnoise(sp3 * 9.);
      float ripple = 0.85 + 0.15 * sin(sp3.x * 3.1 + vnoise(sp3 * 0.7) * 6.);
      float c = caustic(sp3 * 1.6, t * 0.55);
      vec3 sandCol = vec3(0.94, 0.86, 0.68) * sand * ripple;
      float toS = (depthM + floorY) / sunDir.y;
      vec3 lit = exp(-ABS * toS) * (0.3 + 4.5 * c);
      vec3 fl = sandCol * lit * exp(-ABS * tf);
      float fog = 1. - exp(-tf * 0.065);
      col = mix(col + fl, col, fog);
    }
  }

  // Marine snow: three layers; they rise past the camera as it descends.
  vec3 snow = vec3(0.);
  float glow = smoothstep(0.35, 0.75, D);
  for (int L = 0; L < 3; L++) {
    float fl = float(L);
    float sc = 5. + fl * 4.5;
    float drift = t * (0.015 + fl * 0.01) - D * (6. + fl * 5.) ;
    vec2 g = uv * sc + vec2(t * 0.05 * (fl - 1.), drift);
    vec2 id = floor(g), f = fract(g) - 0.5;
    float r = h21(id + fl * 31.);
    if (r > 0.87) {
      vec2 o = vec2(h21(id * 1.7 + 3.) - 0.5, h21(id * 2.3 + 9.) - 0.5) * 0.6;
      vec2 d = f - o;
      float stretch = 1. + min(abs(uSpeed) * (0.4 + fl * 0.3), 6.);
      d.y /= stretch;
      float sz = 0.045 + 0.035 * fl * fl;
      float q2 = length(d) / sz; float m = exp(-q2 * q2 * 3.) * step(q2, 1.6) / sqrt(stretch);
      float tw = 0.6 + 0.4 * sin(t * (1. + r * 3.) + r * 40.);
      vec3 lit = vec3(0.7, 0.9, 1.) * (0.04 + 1.5 * lightLeft) * (0.3 + fl * 0.22);
      // In the dark a few flakes are alive: faint blue-green flickers.
      vec3 bio = vec3(0.2, 0.9, 0.85) * glow * step(0.985, r) * tw * 0.6;
      snow += m * (lit * (1. - glow * 0.8) + bio);
    }
  }
  col += snow;

  // Tone map, vignette, dither.
  col = 1. - exp(-col * 1.25);
  float vig = 1. - 0.45 * dot(uv * 0.55, uv * 0.55);
  col *= vig;
  col += (h21(gl_FragCoord.xy + t) - 0.5) / 255.;
  gl_FragColor = vec4(pow(max(col, 0.), vec3(0.4545)), 1.);
}`;

export function ocean(canvas) {
  const gl = canvas.getContext('webgl', { antialias: false, alpha: false, depth: false, stencil: false, preserveDrawingBuffer: false, powerPreference: 'high-performance' });
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
  const u = {};
  for (const n of ['uRes', 'uT', 'uDepth', 'uSpeed', 'uPtr']) u[n] = gl.getUniformLocation(prog, n);

  // Adaptive resolution: a scale on top of devicePixelRatio, nudged by frame time.
  const small = Math.min(innerWidth, innerHeight) < 700;
  let scale = small ? 0.5 : 0.6;
  const maxScale = Math.min(devicePixelRatio || 1, 1.25) * (small ? 0.7 : 0.85);
  let w = 0, h = 0;
  const size = () => {
    const nw = Math.max(1, Math.round(innerWidth * scale)), nh = Math.max(1, Math.round(innerHeight * scale));
    if (nw !== w || nh !== h) { w = canvas.width = nw; h = canvas.height = nh; gl.viewport(0, 0, w, h); }
  };
  let ema = 16, frames = 0;
  const adapt = (dtMs) => {
    if (dtMs > 100) return;
    ema += (dtMs - ema) * 0.05;
    if (++frames < 40) return;
    if (ema > 19.5 && scale > 0.32) { scale = Math.max(0.32, scale * 0.85); frames = 0; }
    else if (ema < 15 && scale < maxScale) { scale = Math.min(maxScale, scale * 1.08); frames = 0; }
  };
  const state = { depth: 0, speed: 0, t: 3.0, ptr: [0, 0] };
  const draw = () => {
    size();
    gl.uniform2f(u.uRes, w, h);
    gl.uniform1f(u.uT, state.t);
    gl.uniform1f(u.uDepth, state.depth);
    gl.uniform1f(u.uSpeed, state.speed);
        gl.uniform2f(u.uPtr, state.ptr[0], state.ptr[1]);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };
  draw();   // first frame now, before anything else loads
  canvas.classList.add('on');
  return { state, draw, adapt, get scale() { return scale; } };
}
