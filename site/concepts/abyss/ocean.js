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
// Wave height of the surface.
float hgt(vec2 p, float t){
  return vnoise(p * 0.7 + vec2(t * 0.3, t * 0.2)) * 0.6 + vnoise(p * 1.9 - vec2(t * 0.45, -t * 0.3)) * 0.3 + vnoise(p * 4.3 + vec2(t * 0.6, -t * 0.5)) * 0.12;
}
// Caustics: an iterated, self-warping sine field (sharp bright ridges).
float caustic(vec2 p, float t){
  p = mod(p, 6.2831853) - 250.;
  vec2 i = p; float c = 1.;
  for (int n = 0; n < 5; n++) {
    float tt = t * (1. - 3.5 / float(n + 1));
    i = p + vec2(cos(tt - i.x) + sin(tt + i.y), sin(tt - i.y) + cos(tt + i.x));
    c += 1. / length(vec2(p.x / (sin(i.x + tt) / 0.005), p.y / (cos(i.y + tt) / 0.005)));
  }
  c = 1.17 - pow(c / 5., 1.4);
  return pow(abs(c), 8.);
}

void main(){
  vec2 uv = (gl_FragCoord.xy * 2. - uRes) / uRes.y;
  float t = uT;
  float D = uDepth;
  // Camera: looks up at the surface in the hero, then levels out.
  float pitch = mix(0.30, -0.04, smoothstep(0.0, 0.16, D));
  vec3 rd = normalize(vec3(uv.x + uPtr.x * 0.04, uv.y + uPtr.y * 0.03, 1.55));
  float cp = cos(pitch), sp = sin(pitch);
  rd = vec3(rd.x, rd.y * cp + rd.z * sp, -rd.y * sp + rd.z * cp);

  float depthM = 2.2 + D * D * 120.;          // camera depth in metres
  vec3 sunDir = normalize(vec3(0.18, 1., 0.32)); // towards the sun (inside the water)
  // Light left at the camera; past the twilight zone nothing gets through.
  vec3 sunAtCam = exp(-ABS * depthM / sunDir.y) * (1. - smoothstep(0.28, 0.66, D));
  float lightLeft = dot(sunAtCam, vec3(0.2, 0.5, 0.3));

  vec3 col = vec3(0.);
  float sh = 0.;
  if (lightLeft > 0.0005) {
    // Water colour: bright overhead, deeper and bluer towards the horizon and
    // below, because that light has travelled further through the water.
    float up = rd.y * 0.5 + 0.5;
    vec3 amb = SCAT * (0.05 + 0.95 * pow(up, 2.4)) * sunAtCam * exp(-ABS * 7. * (1. - up));
    col = amb;

    // The surface from below: inside Snell's window (about 48 degrees from
    // straight up) the sky shows through, refracted by the waves; outside it
    // the surface is a mirror of the dark water, with rippling highlights.
    if (rd.y > 0.05) {
      float ts = depthM / rd.y;
      vec2 P = rd.xz * ts * 1.7;
      float e = 0.08;
      float h0 = hgt(P, t), hx = hgt(P + vec2(e, 0.), t), hz = hgt(P + vec2(0., e), t);
      vec3 n = normalize(vec3(-(hx - h0) / e * 0.45, 1., -(hz - h0) / e * 0.45));
      // Transmission rises smoothly from the critical angle, so the window's
      // edge is a soft, wave-torn ring rather than a cut-out.
      float cI = dot(rd, n);
      float k = 1. - 1.769 * (1. - cI * cI);
      float T = smoothstep(0.0, 0.12, k);
      vec3 r = refract(rd, -n, 1.33);
      float ry = k > 0. ? max(r.y, 0.) : 0.;
      vec3 sky = mix(vec3(0.5, 0.78, 0.95), vec3(0.95, 1.0, 1.0), pow(ry, 2.));
      float glint = k > 0. ? pow(max(dot(normalize(r), normalize(vec3(0.24, 1., 0.42))), 0.), 220.) * 10. : 0.;
      float waveLight = 0.6 + 0.9 * pow(h0, 2.);
      float net = caustic(P * 0.9, t * 0.8);
      vec3 inside = sky * (0.45 + 0.8 * ry) * waveLight * (0.8 + 1.2 * net) + glint + vec3(0.7, 0.92, 1.) * smoothstep(0.45, 0.0, k) * step(0., k) * 0.6;
      vec3 outside = SCAT * (0.08 + 0.5 * pow(smoothstep(0.35, 0.95, h0), 3.));
      vec3 surf = mix(outside, inside, T);
      surf *= exp(-ABS * ts);
      col = mix(col, surf + amb * 0.3, smoothstep(0.05, 0.3, rd.y) * (1. - smoothstep(0.1, 0.25, D)));
    }

    // Sea floor: a plane below the camera, falling away as we descend.
    float floorY = 1.3 + D * D * 90.;
    float tf = rd.y < 0. ? floorY / -rd.y : 1e5;

    // God rays: shafts parallel to the sun, so seen from inside they fan out
    // from the sun's point; the pattern is the waves' focus carried around it.
    vec3 ax = normalize(cross(sunDir, vec3(0., 0., 1.)));
    vec3 ay = cross(sunDir, ax);
    vec2 dir2 = normalize(vec2(dot(rd, ax), dot(rd, ay)) + 1e-5);
    float toward = dot(rd, sunDir) * 0.5 + 0.5;
    float s1 = vnoise(dir2 * 7. + vec2(t * 0.16, -t * 0.11));
    float s2 = vnoise(dir2 * 19. + vec2(-t * 0.23, t * 0.2) + 7.);
    float s3 = vnoise(dir2 * 41. + vec2(t * 0.31, t * 0.17) + 3.);
    sh = pow(clamp(s1 * 0.55 + s2 * 0.3 + s3 * 0.15, 0., 1.), 3.2) * 2.6;
    float pathLen = min(tf, 30.);
    float vol = 1. - exp(-pathLen * 0.12);
    col += sh * vol * pow(toward, 2.) * vec3(0.55, 0.9, 1.0) * sunAtCam * 0.9 * (1. - 0.5 * smoothstep(0.86, 0.98, toward));

    // Sand: rippled, speckled, lit by caustics that split slightly into colour,
    // then fading into the blue with distance.
    if (tf < 1e4) {
      vec2 sp3 = (rd * tf).xz;
      float dune = sin(sp3.x * 2.3 + sp3.y * 0.6 + vnoise(sp3 * 0.5) * 5.);
      float sand = 0.62 + 0.18 * vnoise(sp3 * 2.0) + 0.12 * vnoise(sp3 * 11.) + 0.12 * dune;
      sand *= 1. - 0.3 * step(0.95, vnoise(sp3 * 7. + 13.));
      vec2 cq = sp3 * 2.2;
      float ct = t * 0.5;
      vec3 c = vec3(caustic(cq, ct), caustic(cq + vec2(0.035, 0.02), ct), caustic(cq + vec2(0.07, 0.04), ct));
      vec3 sandCol = vec3(0.95, 0.87, 0.7) * sand;
      float toS = (depthM + floorY) / sunDir.y;
      vec3 lit = exp(-ABS * toS) * (0.3 + 3.2 * c * exp(-tf * 0.05) * (1. - smoothstep(0.05, 0.3, D)));
      vec3 fl = sandCol * lit * exp(-ABS * tf);
      float fog = 1. - exp(-tf * 0.075);
      col = mix(col * 0.6 + fl, col, fog);
    }
  }

  // Particles: five layers, far and fine to near and soft (out of focus).
  // They drift, sway with the pointer, rise past the camera as it descends
  // and catch the light where a shaft passes through them.
  vec3 snow = vec3(0.);
  float glow = smoothstep(0.35, 0.75, D);
  for (int L = 0; L < 5; L++) {
    float fl = float(L);
    float sc = 13. - fl * 2.2;
    float drift = t * (0.012 + fl * 0.008) - D * (5. + fl * 4.);
    vec2 g = (uv + uPtr * fl * 0.012) * sc + vec2(t * 0.03 * (fl - 2.) + sin(t * 0.2 + fl) * 0.2, drift);
    vec2 id = floor(g), f = fract(g) - 0.5;
    float r = h21(id + fl * 31.);
    if (r > mix(0.82, 0.95, fl / 4.)) {
      float r2 = h21(id * 3.1 + fl);
      vec2 o = vec2(h21(id * 1.7 + 3.) - 0.5, h21(id * 2.3 + 9.) - 0.5) * 0.45;
      vec2 d = f - o;
      // Some flakes are fibres, at an angle.
      float ang = r2 * 6.283, ca = cos(ang), sa = sin(ang);
      d = vec2(ca * d.x - sa * d.y, sa * d.x + ca * d.y);
      d.x /= 1. + step(0.88, r2) * step(fl, 2.5) * 1.2;
      float stretch = 1. + min(abs(uSpeed) * (0.3 + fl * 0.25), 2.5);
      d.y /= stretch;
      float sz = min((0.04 + 0.022 * fl * fl) * (0.6 + 0.8 * r2), 0.2);
      float q = length(d) / sz;
      float m = (fl > 3.5 ? smoothstep(1.0, 0.55, q) * 0.16 : exp(-q * q * 3.) * step(q, 1.6)) / sqrt(stretch);
      float tw = 0.6 + 0.4 * sin(t * (1. + r * 3.) + r * 40.);
      vec3 lit = vec3(0.75, 0.92, 1.) * (0.03 + 1.6 * lightLeft * (0.5 + 0.8 * sh)) * (0.28 + fl * 0.14);
      vec3 bio = vec3(0.2, 0.9, 0.85) * glow * step(0.985, r) * tw * 0.6;
      snow += m * (lit * (1. - glow * 0.8) + bio);
    }
  }
  col += snow;

  // Tone map, vignette, dither.
  col = 1. - exp(-col * 1.1);
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
