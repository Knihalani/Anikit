// Air as a field of molecules shaken by a travelling pressure wave, plus waveform traces.
// Noise (coral), anti-noise (cyan) and their sum (white). `anti` cancels the field.
uniform float amp;    // = 10.0  molecule displacement, px
uniform float chaos;  // = 1.0   0 pure tone .. 1 rough noise
uniform float anti;   // = 0.0   0 noise .. 1 cancelled
uniform float fieldA; // = 1.0
uniform float x1N;    // = 1920.0
uniform float yN;     // = 540.0
uniform float yA;     // = 540.0
uniform float yS;     // = 540.0
uniform float flip;   // = 1.0   anti line: 1 inverted, -1 copy
uniform float aN;     // = 0.0
uniform float aA;     // = 0.0
uniform float aS;     // = 0.0
uniform float gateM;  // = 1.0   displacement multiplier for x > gateX
uniform float gateX;  // = 99999.0
uniform float lamp;   // = 120.0 line amplitude px
uniform float gN;     // = 1.0
uniform float gA;     // = 1.0
uniform float speed;  // = 260.0
uniform float x0N;    // = 0.0
uniform float x0A;    // = 0.0
uniform float x1A;    // = 1920.0
uniform float x0S;    // = 0.0
uniform float x1S;    // = 1920.0
uniform float zoomF;  // = 1.0  field spacing scale
uniform float bg;     // = 1.0  0: lines only, transparent
uniform float xoffN;  // = 0.0  trace phase shift, px

float W(float x) {
  float u = x - speed * iTime;
  float pure = sin(u * 0.0105);
  float rough = 0.55 * sin(u * 0.0083 + 0.3) + 0.32 * sin(u * 0.0217 + 1.7)
              + 0.22 * sin(u * 0.047 + iTime * 3.1) + 0.14 * sin(u * 0.093 - iTime * 5.3)
              + 0.08 * sin(u * 0.17 + iTime * 9.0);
  return mix(pure, rough, chaos);
}

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float lineMask(vec2 p, float y0, float g, float x0, float x1, out float glow) {
  float xs = p.x + xoffN;
  float v = g * W(xs);
  float dv = g * (W(xs + 1.0) - W(xs - 1.0)) * 0.5;
  float d = abs(p.y - (y0 + v)) / sqrt(1.0 + dv * dv);
  float ext = smoothstep(x0 - 2.0, x0 + 30.0, p.x) * (1.0 - smoothstep(x1 - 30.0, x1 + 2.0, p.x));
  glow = exp(-d / 9.0) * ext;
  return (1.0 - smoothstep(1.6, 3.2, d)) * ext;
}

void mainImage(out vec4 o, in vec2 f) {
  float s = iResolution.y / 1080.0;
  vec2 p = vec2(f.x, iResolution.y - f.y) / s;
  // background: deep ink with a soft lift
  vec2 q = (p - vec2(960.0, 520.0)) / 1100.0;
  vec3 col = mix(vec3(0.050, 0.075, 0.115), vec3(0.020, 0.030, 0.050), clamp(dot(q, q) * 1.6, 0.0, 1.0));

  float fa = fieldA * bg;
  // molecule field
  float g = 22.0 * zoomF;
  float jc = floor(p.y / g + 0.5);
  float ic = floor(p.x / g + 0.5);
  float best = 1e5; float comp = 0.0;
  for (int k = -2; k <= 2; k++) {
    float i = ic + float(k);
    float x0 = i * g;
    float m = (x0 > gateX) ? gateM : 1.0;
    float a = amp * m * (1.0 - anti) * zoomF;
    float w0 = W(x0);
    float h = hash(vec2(i, jc));
    float jit = a * 0.18 * sin(iTime * (7.0 + 5.0 * h) + h * 40.0);
    vec2 c = vec2(x0 + a * w0, jc * g + jit);
    float d = length(p - c);
    if (d < best) { best = d; comp = -(W(x0 + 6.0) - W(x0 - 6.0)) / 12.0 * a * 18.0; }
  }
  float r = 2.5 * zoomF;
  float dot_ = 1.0 - smoothstep(r - 0.8, r + 0.8, best);
  vec3 calm = vec3(0.55, 0.68, 0.78);
  vec3 dim = vec3(0.20, 0.28, 0.38);
  vec3 hot = vec3(1.00, 0.42, 0.26);
  float cmp = clamp(comp, -1.0, 1.0);
  vec3 dc = mix(dim, hot, clamp(cmp, 0.0, 1.0));
  dc = mix(dc, dim * 0.6, clamp(-cmp, 0.0, 1.0));
  dc = mix(dc, calm, anti);
  col = mix(col, dc, dot_ * fa * (0.75 + 0.25 * clamp(cmp, 0.0, 1.0)));

  // traces
  if (bg < 0.5) col = vec3(0.0);
  float al = 0.0;
  vec3 cN = vec3(1.00, 0.36, 0.21);
  vec3 cA = mix(cN, vec3(0.36, 0.88, 1.00), smoothstep(-1.0, 1.0, flip));
  vec3 cS = vec3(0.97, 0.95, 0.90);
  float gl;
  float mN = lineMask(p, yN, lamp * gN, x0N, x1N, gl);
  col += cN * gl * 0.35 * aN; col = mix(col, cN, mN * aN); al = max(al, max(mN, gl * 0.35) * aN);
  float mA = lineMask(p, yA, -lamp * gA * flip, x0A, x1A, gl);
  col += cA * gl * 0.35 * aA; col = mix(col, cA, mA * aA); al = max(al, max(mA, gl * 0.35) * aA);
  float mS = lineMask(p, yS, lamp * gN * (1.0 - anti), x0S, x1S, gl);
  col += cS * gl * 0.30 * aS; col = mix(col, cS, mS * aS); al = max(al, max(mS, gl * 0.3) * aS);

  if (bg < 0.5) { o = vec4(min(col, vec3(al)), al); } else { o = vec4(col, 1.0); }
}
