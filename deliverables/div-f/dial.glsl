// Atelier Vauclair Calibre 9 - the dial, the case and the light, in watch millimetres.
// One continuous camera (uK: 0 = macro on the grooves, 1 = the whole watch), a raking light (uLight)
// that the engine-turned grooves catch anisotropically, hands, a 72-hour ring and a 150-mark ring.
uniform float uK;       // = 0.0  camera: 0 macro .. 1 full watch (>1 pushes in a little)
uniform float uLight;   // = 0.6  light azimuth (rad)
uniform float uCut;     // = 0.0  progress of the single salmon groove being cut
uniform float uCutA;    // = 1.0  its opacity
uniform float uHours;   // = 10.15 watch time in hours
uniform float uSpin;    // = 0.0  hand smear (turns per frame of the minute hand)
uniform float uReserve; // = 0.0  72-hour ring fill 0..1
uniform float uResA;    // = 0.0  72-hour ring opacity
uniform float uRing;    // = 0.0  150-mark ring sweep 0..1
uniform float uRingA;   // = 0.0  150-mark ring opacity
uniform float uOne;     // = 0.0  0 all marks lit .. 1 only one lit
uniform float uWX;      // = 1280.0 watch centre x on screen at full view

const float PI = 3.14159265;
const float TAU = 6.28318531;

float lum(float x) { return clamp(x, 0.0, 1.0); }

// anisotropic sheen of a field of V-grooves whose in-plane gradient is g (unit), at phase v (0..1 across a groove),
// with aa = groove pitch in pixels (small = average both flanks)
float grooveSheen(vec2 g, vec2 L, float v, float aa) {
  float d = dot(g, L);
  float a = pow(max(d, 0.0), 6.0);
  float b = pow(max(-d, 0.0), 6.0);
  float side = smoothstep(0.5 - 0.5 / aa, 0.5 + 0.5 / aa, v); // 0 first flank, 1 second
  float sharp = mix(a, b, side);
  float avg = 0.5 * (a + b);
  float k = smoothstep(2.0, 6.0, aa);
  // groove bottoms and crests: thin dark/bright lines that only read in macro
  float edge = 1.0 - smoothstep(0.0, 1.5 / aa, min(v, 1.0 - v));
  return mix(avg, sharp - 0.18 * edge * k, k);
}

// the guilloche field: a twisted rosette (barleycorn-like). returns f (mm) ; grooves are level sets of f.
float guil(vec2 p) {
  float r = length(p);
  float th = atan(p.y, p.x);
  return r + 0.32 * sin(18.0 * th + 0.55 * r) * smoothstep(1.0, 4.0, r);
}

float handMask(vec2 p, float turns, float len, float wBase, float wTip, float tail, float smear, float px) {
  // angle 0 = 12 o'clock, clockwise
  float ang = (0.25 - turns) * TAU;
  vec2 dir = vec2(cos(ang), sin(ang));
  vec2 nrm = vec2(-dir.y, dir.x);
  float s = dot(p, dir);
  float n = dot(p, nrm);
  float u = clamp((s + tail) / (len + tail), 0.0, 1.0);
  float w = mix(wBase, wTip, u) * 0.5;
  // angular smear widens the hand and lowers its density
  float sm = min(smear, 0.05) * TAU * max(s, 0.5);
  float ww = w + sm;
  float m = (1.0 - smoothstep(ww - px, ww + px, abs(n)))
          * smoothstep(-tail - px, -tail + px, s) * (1.0 - smoothstep(len - px, len + px, s));
  return m * max(w / ww, 0.45);
}

float handFacet(vec2 p, float turns, vec2 L) {
  float ang = (0.25 - turns) * TAU;
  vec2 dir = vec2(cos(ang), sin(ang));
  vec2 nrm = vec2(-dir.y, dir.x);
  float side = sign(dot(p, nrm));
  return 0.25 + 0.75 * pow(max(dot(nrm * side, L), 0.0), 2.0);
}

void mainImage(out vec4 o, in vec2 fc) {
  vec2 res = iResolution.xy;
  vec2 f = fc * (1080.0 / res.y);              // work in 1080p pixels
  // camera: log-interpolated zoom, focus point in mm sits at screen centre
  float k = clamp(uK, 0.0, 1.0);
  float zoom = exp(mix(log(250.0), log(19.0), k)) * (1.0 + 0.12 * max(uK - 1.0, 0.0));
  vec2 F0 = vec2(7.2, -3.6);
  vec2 F1 = vec2((960.0 - uWX) / 19.0, 0.0);
  vec2 Fm = mix(F0, F1, smoothstep(0.0, 1.0, k));
  vec2 p = (f - vec2(960.0, 540.0)) / zoom + Fm;   // mm, y up
  float px = 1.0 / zoom;                            // one pixel in mm
  float r = length(p);
  float th = atan(p.y, p.x);
  vec2 L = vec2(cos(uLight), sin(uLight));

  // background: graphite with a soft pool of light behind the watch and a faint drafting grid
  vec2 sp = f / 1080.0;
  vec3 col = vec3(0.043, 0.047, 0.055) + vec3(0.05, 0.052, 0.058) * exp(-r * r / 900.0);
  vec2 gq = abs(fract(p / 5.0 + 0.5) - 0.5) * 5.0;
  float grid = (1.0 - smoothstep(0.0, 1.2 * px, min(gq.x, gq.y))) * smoothstep(19.5, 21.0, r);
  col += vec3(0.022, 0.024, 0.028) * grid * k;

  // lugs (12 and 6 o'clock), brushed
  float lx = abs(p.x), ly = abs(p.y);
  float lug = (1.0 - smoothstep(9.6 - px, 9.6 + px, lx)) * smoothstep(5.4 - px, 5.4 + px, lx)
            * (1.0 - smoothstep(21.6 - px, 21.6 + px, ly)) * smoothstep(14.0, 14.1, ly);
  // polished lug tops: a bevel highlight on the side facing the light
  float lugEdge = smoothstep(0.0, 0.9, min(lx - 5.4, 9.6 - lx));
  vec3 lugc = vec3(0.30, 0.31, 0.33) + vec3(0.75) * pow(max(dot(vec2(sign(p.x) * (lx < 7.5 ? -1.0 : 1.0), 0.0), L), 0.0), 4.0) * (1.0 - lugEdge)
            + vec3(0.18) * lugEdge * (0.5 + 0.5 * dot(vec2(0.0, sign(p.y)), L));
  // shadow of the case/lugs on the ground
  float sh = (1.0 - smoothstep(19.0, 21.5, length(p + vec2(0.5, 0.9))));
  col *= 1.0 - 0.45 * sh;
  col = mix(col, lugc, lug);

  // crown at 3 o'clock
  float crown = (1.0 - smoothstep(20.6 - px, 20.6 + px, p.x)) * smoothstep(18.0, 18.1, p.x)
              * (1.0 - smoothstep(2.3 - px, 2.3 + px, ly));
  vec3 crc = vec3(0.35, 0.36, 0.38) * (0.7 + 0.3 * step(0.5, fract(p.y * 3.2))) + 0.25 * max(L.x, 0.0);
  col = mix(col, crc, crown);

  // case: polished outer edge, circular-brushed flat, polished bevel
  float caseM = 1.0 - smoothstep(19.0 - px, 19.0 + px, r);
  vec2 rad = p / max(r, 1e-4);
  float brushV = fract(r * 9.0 + 0.3 * sin(th * 3.0) + 0.4 * sin(th * 211.0));
  float brushed = grooveSheen(rad, L, brushV, 0.111 * zoom);
  vec3 casec = vec3(0.17, 0.18, 0.20) + vec3(1.25, 1.25, 1.28) * brushed + 0.10 * (0.5 + 0.5 * dot(rad, L));
  float bevel = smoothstep(15.2, 15.3, r) * (1.0 - smoothstep(16.2, 16.3, r));
  float pol = pow(max(dot(-rad, L), 0.0), 18.0) * 1.2 + pow(max(dot(rad, L), 0.0), 30.0) * 0.4;
  casec = mix(casec, vec3(0.16, 0.17, 0.19) + vec3(0.95) * pol, bevel);
  float rim = smoothstep(18.55, 18.75, r);
  casec = mix(casec, vec3(0.2, 0.21, 0.23) + vec3(0.9) * pow(max(dot(rad, L), 0.0), 12.0), rim);
  col = mix(col, casec, caseM);

  // dial
  float dialR = 15.2;
  float dialM = 1.0 - smoothstep(dialR - px, dialR + px, r);
  float fg = guil(p);
  float pitch = 0.17;
  float fv = fg / pitch;
  vec2 gg = vec2(guil(p + vec2(0.01, 0.0)) - guil(p - vec2(0.01, 0.0)),
                 guil(p + vec2(0.0, 0.01)) - guil(p - vec2(0.0, 0.01)));
  vec2 g = gg / max(length(gg), 1e-5);
  float sheen = grooveSheen(g, L, fract(fv), pitch * zoom);
  vec3 dialLo = vec3(0.13, 0.17, 0.23);
  vec3 dialHi = vec3(0.80, 0.85, 0.92);
  vec3 dialc = mix(dialLo, dialHi, lum(0.08 + 1.05 * sheen));
  // chapter ring (flat, frosted) with 60 minute marks
  float chap = smoothstep(13.3, 13.3 + px, r);
  float mcell = th / TAU * 60.0;
  float md = abs(fract(mcell) - 0.5) * TAU * r / 60.0;
  float mk = (1.0 - smoothstep(0.06 - px, 0.06 + px, md)) * smoothstep(14.0, 14.0 + px, r) * (1.0 - smoothstep(14.9, 14.9 + px, r));
  vec3 chapc = vec3(0.2, 0.23, 0.28) + 0.12 * max(dot(rad, L), 0.0);
  chapc = mix(chapc, vec3(0.86, 0.88, 0.9), mk);
  dialc = mix(dialc, chapc, chap);
  // applied baton indices (polished, faceted)
  float icell = th / TAU * 12.0;
  float id = abs(fract(icell + 0.5) - 0.5) * TAU * r / 12.0;
  float idx = (1.0 - smoothstep(0.42 - px, 0.42 + px, id)) * smoothstep(10.6, 10.6 + px, r) * (1.0 - smoothstep(13.0, 13.0 + px, r));
  vec2 tng = vec2(-rad.y, rad.x);
  float fside = sign(fract(icell + 0.5) - 0.5);
  float ib = 0.25 + 0.9 * pow(max(dot(tng * fside, L), 0.0), 3.0);
  vec3 idxc = vec3(0.92, 0.93, 0.95) * ib;
  // index shadow
  float ids = abs(fract((atan(p.y + 0.15, p.x - 0.12)) / TAU * 12.0 + 0.5) - 0.5) * TAU * r / 12.0;
  float idsh = (1.0 - smoothstep(0.42, 0.62, ids)) * smoothstep(10.4, 10.8, r) * (1.0 - smoothstep(13.0, 13.4, r));
  dialc *= 1.0 - 0.45 * idsh;
  dialc = mix(dialc, idxc, idx);

  // the cut: a single groove engraved in salmon, with a hot stylus tip
  float n0 = floor(guil(F0) / pitch) + 0.0;
  float inG = step(n0, floor(fv)) * step(floor(fv), n0) ;
  float th0 = atan(F0.y, F0.x) + 0.62;
  float dth = th0 - th;                         // runs clockwise across screen
  float prog = uCut * 1.25;
  float along = step(0.0, dth) * (1.0 - smoothstep(prog - 0.002, prog, dth));
  float gv = fract(fv);
  float core = 1.0 - smoothstep(0.30, 0.42, abs(gv - 0.5));
  vec3 salmon = vec3(0.94, 0.60, 0.45);
  float cutm = inG * along * core * uCutA;
  vec3 cutc = salmon * (0.75 + 0.6 * pow(max(dot(g * sign(gv - 0.5), L), 0.0), 2.0));
  dialc = mix(dialc, cutc, cutm);
  float tip = exp(-pow((dth - prog) * r / 0.18, 2.0) - pow((fv - n0 - 0.5) / 0.9, 2.0)) * uCutA * step(0.001, uCut) * step(uCut, 0.999);
  dialc += vec3(1.0, 0.75, 0.55) * tip * 1.4;

  // hands (with shadows), hour, minute, seconds
  float hT = uHours / 12.0;
  float mT = fract(uHours);
  float sT = floor(iTime * 6.0) / 360.0 + 0.1 + uHours * 60.0;
  vec2 so = vec2(0.22, -0.32);
  float hs = handMask(p - so, hT, 8.6, 1.25, 0.55, 2.0, uSpin / 12.0, px * 4.0);
  float ms = handMask(p - so, mT, 13.0, 0.95, 0.35, 2.4, uSpin, px * 4.0);
  dialc *= 1.0 - 0.5 * max(hs, ms) * dialM;
  float hm = handMask(p, hT, 8.6, 1.25, 0.55, 2.0, uSpin / 12.0, px);
  float mm = handMask(p, mT, 13.0, 0.95, 0.35, 2.4, uSpin, px);
  float smk = handMask(p, sT, 13.8, 0.2, 0.14, 3.2, uSpin * 60.0, px) * (1.0 - smoothstep(0.02, 0.2, uSpin));
  vec3 handc = vec3(0.95, 0.96, 0.98);
  dialc = mix(dialc, handc * handFacet(p, hT, L), hm);
  dialc = mix(dialc, handc * handFacet(p, mT, L), mm);
  dialc = mix(dialc, salmon, smk);
  float hub = 1.0 - smoothstep(0.75 - px, 0.75 + px, r);
  dialc = mix(dialc, vec3(0.75, 0.77, 0.8) * (0.6 + 0.6 * max(dot(rad, L), 0.0)), hub);
  float hub2 = 1.0 - smoothstep(0.32 - px, 0.32 + px, r);
  dialc = mix(dialc, salmon * 0.9, hub2);

  // crystal: a broad soft reflection sweeping with the light
  float glare = exp(-pow(dot(p, vec2(-L.y, L.x)) / 5.0 - 0.0, 2.0)) * 0.05;
  dialc += vec3(glare);
  col = mix(col, dialc, dialM);

  // 72-hour ring: 72 segments outside the case
  float seg = th / TAU;
  float cw = fract(0.25 - seg);                 // 0 at 12 o'clock, clockwise
  float rr = 23.6;
  float sc = cw * 72.0;
  float sd = abs(fract(sc) - 0.5) * TAU * rr / 72.0;
  float segm = (1.0 - smoothstep(0.62 - px, 0.62 + px, sd)) * smoothstep(23.0 - px, 23.0 + px, r) * (1.0 - smoothstep(24.6 - px, 24.6 + px, r));
  float filled = step(cw, uReserve);
  vec3 rc = mix(vec3(0.16, 0.17, 0.19), vec3(0.88, 0.9, 0.93), filled);
  float lead = filled * (1.0 - smoothstep(0.0, 0.04, uReserve - cw)) * step(0.001, uReserve);
  rc = mix(rc, salmon, lead);
  col = mix(col, rc, segm * uResA);

  // 150-mark ring: one piece in a run of 150
  float c150 = cw * 150.0;
  float d150 = abs(fract(c150) - 0.5) * TAU * rr / 150.0;
  float m150 = (1.0 - smoothstep(0.075 - px, 0.075 + px, d150)) * smoothstep(22.8 - px, 22.8 + px, r) * (1.0 - smoothstep(25.2 - px, 25.2 + px, r));
  float lit = step(cw, uRing);
  float isOne = 1.0 - step(1.0, c150);          // the first mark, at 12 o'clock
  float b150 = lit * mix(1.0, isOne, uOne);
  vec3 c150c = mix(vec3(0.15, 0.16, 0.18), vec3(0.9, 0.92, 0.95), b150);
  c150c = mix(c150c, salmon * 1.1, isOne * lit);
  float m150b = (1.0 - smoothstep(0.16 - px, 0.16 + px, d150)) * isOne * smoothstep(22.2 - px, 22.2 + px, r) * (1.0 - smoothstep(25.8 - px, 25.8 + px, r));
  col = mix(col, c150c, max(m150, m150b) * uRingA);

  // gentle vignette
  vec2 vq = sp - vec2(0.889, 0.5);
  col *= 1.0 - 0.25 * dot(vq, vq);
  o = vec4(pow(col, vec3(1.0)), 1.0);
}
