// Riso-print oscilloscope: noise (pink plate), anti-noise (blue plate), what you hear (black plate).
uniform float uNY;   // = 0.5
uniform float uAY;   // = 0.5
uniform float uSY;   // = 0.5
uniform float uNA;   // = 1.0
uniform float uAA;   // = 0.0
uniform float uSA;   // = 0.0
uniform float uAmp;  // = 0.17
uniform float uFlip; // = 1.0
uniform float uSumK; // = 0.0
uniform float uX0;   // = -0.2
uniform float uX1;   // = 1.2
uniform float uSX0;  // = -0.2
uniform float uSX1;  // = 1.2
uniform float uCalm; // = 0.0
uniform float uSCalm; // = 0.0
uniform float uSpeed; // = 1.0
uniform float uT0;   // = 0.0

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
  vec2 i = floor(p), f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(hash(i), hash(i+vec2(1,0)), f.x), mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), f.x), f.y);
}
// the noise signal: engine drone + jagged partials; uCalm morphs it into a gentle melody line
float sigc(float x, float tt, float cm) {
  float rough = 0.52*sin(x*5.0 - tt*4.0) + 0.30*sin(x*12.7 - tt*7.3 + 1.3)
              + 0.20*sin(x*27.0 - tt*12.0 + 0.7) + 0.13*sin(x*58.0 - tt*19.0)
              + 0.09*sin(x*121.0 - tt*31.0 + 2.0);
  float calm = 0.35*sin(x*3.0 - tt*1.6) * (0.6 + 0.4*sin(x*1.1 + tt*0.7));
  return mix(rough, calm, cm);
}
float sig(float x, float tt) { return sigc(x, tt, uCalm); }
float env(float u, float a, float b) { return smoothstep(a, a + 0.035, u) * (1.0 - smoothstep(b - 0.035, b, u)); }
// distance (px) to curve y = c(x), with c given by value and slope
float lineCov(float py, float cy, float slope, float halfw) {
  float d = abs(py - cy) / sqrt(1.0 + slope*slope);
  return 1.0 - smoothstep(halfw - 0.9, halfw + 0.9, d);
}
float dots(vec2 p, float ang, float cell, float tone) {
  float c = cos(ang), s = sin(ang);
  vec2 q = mat2(c, -s, s, c) * p;
  vec2 g = fract(q / cell) - 0.5;
  float d = length(g) * cell;
  float r = sqrt(clamp(tone, 0.0, 1.0)) * cell * 0.56;
  return 1.0 - smoothstep(r - 0.8, r + 0.8, d);
}
void mainImage(out vec4 o, in vec2 f) {
  vec2 R = iResolution.xy;
  float H = R.y;
  vec2 p = vec2(f.x, R.y - f.y);           // top-down pixels
  float u = p.x / R.x;
  float tt = (iTime + uT0) * uSpeed;
  float boil = floor(iTime * 12.0);       // ink boils on twos
  float A = uAmp * H;
  // per plate misregistration
  vec2 pb = p + vec2(3.0, -2.0);
  float X = p.x / H, Xb = pb.x / H;
  float e  = env(u, uX0, uX1);
  float eb = env(pb.x / R.x, uX0, uX1);
  float es = env(u, uSX0, uSX1);
  float eps = 1.5 / H;
  // noise plate
  float n  = A * sig(X, tt) * e;
  float n2 = A * sig(X + eps, tt) * env((p.x + 1.5) / R.x, uX0, uX1);
  float nSlope = (n2 - n) / 1.5;
  // anti plate (on the blue plate's own registration)
  float nb  = A * sig(Xb, tt) * eb;
  float nb2 = A * sig(Xb + eps, tt) * env((pb.x + 1.5) / R.x, uX0, uX1);
  float flipK = 1.0 - 2.0 * uFlip;
  float a  = nb * flipK;
  float aSlope = (nb2 - nb) / 1.5 * flipK;
  // what you hear = noise + k * anti
  float sc = max(uCalm, uSCalm);
  float ns  = A * sigc(X, tt, sc);
  float ns2 = A * sigc(X + eps, tt, sc);
  float s  = (ns  + uSumK * ns  * flipK) * es;
  float sSlope = (ns2 - ns) / 1.5 * (1.0 + uSumK * flipK);

  float yN = uNY * H, yA = uAY * H, yS = uSY * H;
  // fills between baseline and curve, as halftone
  float inN = step(min(yN, yN - n), p.y) * step(p.y, max(yN, yN - n)) * step(0.5, abs(n));
  float inA = step(min(yA, yA - a), pb.y) * step(pb.y, max(yA, yA - a)) * step(0.5, abs(a));
  float tex = 0.82 + 0.18 * vnoise(p * 0.08 + boil * 7.3);
  float tex2 = 0.82 + 0.18 * vnoise(pb * 0.08 + boil * 3.1 + 11.0);
  float pinkCov = max(inN * dots(p, 0.26, 9.0, 0.42 * tex), lineCov(p.y, yN - n, nSlope, 3.4) * e) * uNA * tex;
  float blueCov = max(inA * dots(pb, 1.31, 9.0, 0.42 * tex2), lineCov(pb.y, yA - a, aSlope, 3.4) * eb) * uAA * tex2;
  // baselines as fine hairlines
  float baseN = (1.0 - smoothstep(0.4, 1.2, abs(p.y - yN))) * 0.35 * uNA * e;
  float baseA = (1.0 - smoothstep(0.4, 1.2, abs(pb.y - yA))) * 0.35 * uAA * eb;
  pinkCov = max(pinkCov, baseN);
  blueCov = max(blueCov, baseA);
  float blackCov = lineCov(p.y, yS - s, sSlope, 4.6) * uSA * es * (0.9 + 0.1 * vnoise(p * 0.15 + boil));

  // paper with fibres
  vec3 paper = vec3(0.945, 0.925, 0.878);
  float fib = vnoise(p * vec2(0.9, 0.05)) * 0.5 + vnoise(p * 0.02) * 0.5;
  paper *= 0.975 + 0.035 * fib;
  vec2 vc = (f / R) - 0.5;
  paper *= 1.0 - 0.18 * dot(vc, vc);
  vec3 pink = vec3(1.0, 0.30, 0.66);
  vec3 blue = vec3(0.17, 0.33, 0.66);
  vec3 ink  = vec3(0.11, 0.10, 0.11);
  vec3 col = paper;
  col *= mix(vec3(1.0), pink, clamp(pinkCov, 0.0, 1.0));
  col *= mix(vec3(1.0), blue, clamp(blueCov, 0.0, 1.0));
  col *= mix(vec3(1.0), ink, clamp(blackCov, 0.0, 1.0));
  o = vec4(col, 1.0);
}
