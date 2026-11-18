// Air as a field of molecules. A noise wave shoves them into bands;
// inside the ear cup, the speaker's flipped wave holds them still.
uniform float front;   // = -300
uniform float cupOn;   // = 0
uniform float micOn;   // = 0
uniform float trace;   // = 0
uniform float graphOn; // = 0
uniform float antiOn;  // = 0
uniform float antiAmp; // = 1
uniform float merge;   // = 0
uniform float cancelR; // = 0
uniform float spkOn;   // = 0
uniform float scrim;   // = 1

const float PI = 3.14159265;
const vec2 SRC = vec2(-1500.0, 640.0);
const vec2 CUP = vec2(1575.0, 540.0);
const vec2 CUPR = vec2(272.0, 262.0);
const vec2 HEAD = vec2(2080.0, 540.0);
const vec2 HEADR = vec2(525.0, 660.0);
const vec2 SPK = vec2(1478.0, 540.0);
const vec2 MIC = vec2(1300.0, 540.0);
const vec2 CHIP = vec2(1392.0, 540.0);

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float N(float x, float t) {
  float k = 2.0 * PI / 330.0;
  float ph = k * x - k * 240.0 * t;
  return (sin(ph) + 0.45 * sin(2.0 * ph + 1.3) + 0.22 * sin(3.0 * ph + 0.4) + 0.12 * sin(5.0 * ph + 2.1)) / 1.45;
}

float sdEll(vec2 p, vec2 c, vec2 r) { return (length((p - c) / r) - 1.0) * min(r.x, r.y); }

float sdSeg(vec2 p, vec2 a, vec2 b) {
  vec2 pa = p - a, ba = b - a;
  float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return length(pa - ba * h);
}

float sdBox(vec2 p, vec2 c, vec2 b, float r) {
  vec2 d = abs(p - c) - b + r;
  return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - r;
}

void mainImage(out vec4 o, in vec2 f) {
  float S = 1920.0 / iResolution.x;
  vec2 P = vec2(f.x, iResolution.y - f.y) * S;
  float aa = 1.2 * S;
  float t = iTime;

  // ---- regions
  float dHead = sdEll(P, HEAD, HEADR);
  float dCup = sdEll(P, CUP, CUPR);
  float inner = smoothstep(2.0, -6.0, dCup + 30.0) * smoothstep(-2.0, 6.0, dHead) * step(P.x, CUP.x);
  float interior = inner * cupOn;

  // ---- the air field
  vec2 dir = normalize(P - SRC);
  float xs = length(P - SRC) + SRC.x;
  float env = smoothstep(front, front - 300.0, xs);
  float A = 15.0;
  float n0 = N(xs, t);
  float dn = (N(xs + 1.0, t) - N(xs - 1.0, t)) * 0.5;
  float reach = smoothstep(cancelR, cancelR - 140.0, length(P - SPK));
  float c = interior * reach;
  float passive = 1.0 - 0.35 * interior;
  float amp = A * env * passive * (1.0 - c);
  vec2 q = P - dir * amp * n0;
  float press = clamp(-dn * A * env * passive * (1.0 - c) * 1.6, -1.0, 1.0);

  float s = 22.0;
  vec2 id = floor(q / s);
  vec3 col = vec3(0.027, 0.034, 0.046);
  // vignette + faint warm floor light from the noise side
  col += vec3(0.05, 0.028, 0.016) * smoothstep(1400.0, 0.0, length(P - vec2(-100.0, 900.0)));
  float dots = 0.0;
  vec3 dotCol = vec3(0.0);
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 cid = id + vec2(float(i), float(j));
      float h1 = hash(cid), h2 = hash(cid + 17.3);
      vec2 cen = (cid + 0.5) * s + (vec2(h1, h2) - 0.5) * 7.0;
      float r = 1.6 + 1.0 * h2 + 1.4 * max(press, 0.0) - 0.5 * max(-press, 0.0);
      float d = length(q - cen) - r;
      float a = smoothstep(aa, -aa, d);
      if (a > dots) { dots = a; }
    }
  }
  vec3 calm = vec3(0.50, 0.54, 0.60);
  vec3 hot = vec3(1.0, 0.52, 0.24);
  vec3 still = vec3(0.78, 0.92, 1.0);
  dotCol = mix(calm * (0.5 + 0.38 * press), hot * (1.0 + 0.35 * press), smoothstep(0.1, 0.8, press));
  dotCol = mix(dotCol, still * 0.95, c);
  float hidden = smoothstep(-1.0, 1.0, dHead); // no air inside the head
  float fieldDim = 1.0 - 0.8 * scrim * smoothstep(360.0, 140.0, P.y) * smoothstep(1350.0, 1000.0, P.x);
  // panel behind graph
  float panel = sdBox(P, vec2(600.0, 838.0), vec2(500.0, 128.0), 18.0);
  float inPanel = smoothstep(2.0, -2.0, panel) * graphOn;
  fieldDim *= 1.0 - 0.82 * inPanel;
  col += dotCol * dots * hidden * fieldDim;

  // the speaker's anti-wave front, a cool ring inside the cup
  float ring = exp(-pow((length(P - SPK) - cancelR) / 10.0, 2.0)) * interior * spkOn * smoothstep(380.0, 200.0, cancelR);
  col += vec3(0.36, 0.78, 1.0) * ring * 0.55;
  col += vec3(0.30, 0.55, 0.75) * c * 0.06 * interior;

  // ---- head (top view): warm dark skin, lit by a soft key from upper-left
  float headA = smoothstep(aa, -aa, dHead);
  vec2 hn = (P - HEAD) / HEADR;
  float hl = clamp(0.55 - 0.45 * hn.x - 0.25 * hn.y, 0.0, 1.0);
  vec3 headCol = mix(vec3(0.035, 0.03, 0.03), vec3(0.15, 0.11, 0.09), hl * hl);
  headCol += vec3(0.55, 0.40, 0.30) * smoothstep(-14.0, -1.0, dHead) * 0.5 * (0.4 + 0.6 * smoothstep(400.0, 0.0, abs(P.y - 400.0)));
  col = mix(col, headCol, headA);

  // ---- cup shell (drawn on by angle)
  float ang = atan(P.y - CUP.y, -(P.x - CUP.x));
  float sweep = smoothstep(0.0, 0.02, cupOn * 1.04 - (ang + PI * 0.5) / PI);
  float shell = smoothstep(aa, -aa, abs(dCup + 15.0) - 15.0) * step(P.x, CUP.x - 6.0) * sweep;
  vec2 sn = (P - CUP) / CUPR;
  float sl = clamp(0.5 - 0.6 * sn.y - 0.3 * sn.x, 0.0, 1.0);
  vec3 shellCol = mix(vec3(0.09, 0.095, 0.105), vec3(0.42, 0.43, 0.45), sl * sl);
  shellCol += vec3(0.9) * smoothstep(1.6 * S, 0.0, abs(dCup)) * 0.5 * sl;
  shellCol *= 0.85 + 0.15 * hash(floor(P * 0.5));
  col = mix(col, shellCol, shell);
  // cushions where the shell meets the head
  for (int k = 0; k < 2; k++) {
    float sy = k == 0 ? -1.0 : 1.0;
    float cd = sdBox(P, vec2(1546.0, 540.0 + sy * 240.0), vec2(16.0, 42.0), 14.0);
    float ca = smoothstep(aa, -aa, cd) * smoothstep(0.85, 1.0, cupOn);
    vec3 cc = vec3(0.22, 0.21, 0.2) * (0.75 + 0.5 * hash(floor(P / 3.0)));
    col = mix(col, cc, ca);
  }

  // ---- driver: magnet, plate, cone
  float drv = smoothstep(0.7, 1.0, cupOn);
  float wob = spkOn * 3.0 * N(t * 240.0, t);
  float mag = sdBox(P, vec2(1452.0, 540.0), vec2(12.0, 34.0), 3.0);
  float plate = sdBox(P, vec2(1470.0 + wob * 0.3, 540.0), vec2(4.0, 86.0), 2.0);
  float cone = min(min(sdSeg(P, vec2(1474.0, 456.0), vec2(1500.0 + wob, 486.0)), sdSeg(P, vec2(1474.0, 624.0), vec2(1500.0 + wob, 594.0))),
                   sdSeg(P, vec2(1500.0 + wob, 486.0), vec2(1500.0 + wob, 594.0)));
  vec3 metal = vec3(0.62, 0.64, 0.67);
  col = mix(col, vec3(0.20, 0.21, 0.23), smoothstep(aa, -aa, mag) * drv);
  col = mix(col, metal, smoothstep(aa, -aa, plate) * drv);
  col = mix(col, metal * mix(1.0, 1.4, spkOn), smoothstep(aa + 1.6, -aa + 1.6, cone) * drv);

  // ---- mic, chip and the trace between them
  float micD = length(P - MIC) - 11.0;
  col = mix(col, vec3(0.85, 0.86, 0.88), smoothstep(aa, -aa, micD) * drv);
  col = mix(col, vec3(0.15), smoothstep(aa, -aa, length(P - MIC) - 5.0) * drv * (0.6 + 0.4 * step(0.5, fract((P.x + P.y) / 3.0))));
  // listening rings converge on the mic
  for (int k = 0; k < 3; k++) {
    float ph = fract(t * 0.9 + float(k) / 3.0);
    float rr = mix(70.0, 14.0, ph);
    float rg = smoothstep(1.5 * S, 0.0, abs(length(P - MIC) - rr)) * ph * (1.0 - ph) * 4.0;
    col += vec3(1.0, 0.6, 0.35) * rg * micOn * 0.7 * step(P.x, MIC.x + 4.0);
  }
  float tr = min(sdSeg(P, MIC + vec2(11.0, 0.0), CHIP - vec2(20.0, 0.0)), sdSeg(P, CHIP + vec2(20.0, 0.0), vec2(1440.0, 540.0)));
  float along = clamp((P.x - MIC.x) / (1440.0 - MIC.x), 0.0, 1.0);
  float lit = smoothstep(trace + 0.02, trace - 0.02, along);
  float pulse = exp(-pow((along - trace) * 14.0, 2.0)) * step(0.01, trace) * step(trace, 0.99);
  col = mix(col, mix(vec3(0.3), vec3(0.36, 0.78, 1.0), lit), smoothstep(1.4 + aa, 1.4 - aa, tr) * drv);
  col += vec3(0.6, 0.9, 1.0) * pulse * smoothstep(6.0, 0.0, tr) * drv;
  float chip = sdBox(P, CHIP, vec2(19.0, 19.0), 3.0);
  float pins = sdBox(vec2(P.x, mod(P.y - CHIP.y + 4.0, 8.0) + CHIP.y - 4.0), CHIP, vec2(25.0, 1.5), 0.5) ;
  pins = max(pins, abs(P.y - CHIP.y) - 15.0);
  col = mix(col, vec3(0.5), smoothstep(aa, -aa, pins) * drv);
  col = mix(col, mix(vec3(0.10), vec3(0.12, 0.2, 0.26), lit), smoothstep(aa, -aa, chip) * drv);
  col += vec3(0.36, 0.78, 1.0) * smoothstep(1.5 * S, 0.0, abs(chip)) * lit * drv * 0.8;

  // ---- graph: noise, its flipped copy, and their sum
  if (graphOn > 0.0) {
    float x0 = 160.0, x1 = 1040.0, yc = 846.0, ga = 62.0;
    float u = (P.x - x0) / (x1 - x0);
    float inX = step(0.0, u) * step(u, 1.0);
    float gx = P.x * 0.9;
    float tt = t * 0.9;
    float nv = N(gx, tt), nd = (N(gx + 1.0, tt) - N(gx - 1.0, tt)) * 0.5;
    float m = 1.0 - merge;
    // noise
    float fy = yc - ga * nv * m;
    float dN1 = abs(P.y - fy) / sqrt(1.0 + pow(ga * nd * m, 2.0));
    float rev1 = smoothstep(graphOn + 0.01, graphOn - 0.01, u);
    float a1 = smoothstep(2.4 + aa, 2.4 - aa, dN1) * inX * rev1;
    // anti
    float ay = yc - ga * nv * antiAmp * m;
    float dA = abs(P.y - ay) / sqrt(1.0 + pow(ga * nd * antiAmp * m, 2.0));
    float rev2 = smoothstep(antiOn + 0.01, antiOn - 0.01, u);
    float a2 = smoothstep(2.4 + aa, 2.4 - aa, dA) * inX * rev2;
    // baseline
    float base = smoothstep(1.0 + aa, 1.0 - aa, abs(P.y - yc)) * inX * step(0.5, fract(P.x / 14.0)) * graphOn;
    col += vec3(0.25) * base * (1.0 - merge);
    vec3 hotC = mix(vec3(1.0, 0.47, 0.22), vec3(1.0), merge);
    vec3 coolC = mix(vec3(0.36, 0.78, 1.0), vec3(1.0), merge);
    col = mix(col, hotC, a1);
    col = mix(col, coolC, a2 * 0.95);
    col += vec3(0.9) * smoothstep(9.0, 0.0, abs(P.y - yc)) * inX * merge * merge * 0.12;
    // panel border
    col += vec3(0.16) * smoothstep(1.2 * S, 0.0, abs(panel)) * graphOn;
  }

  // grain
  col += (hash(f + fract(t * 7.3) * 91.0) - 0.5) * 0.022;
  o = vec4(pow(max(col, 0.0), vec3(0.95)), 1.0);
}
