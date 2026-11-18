// Turbine hall at night: tall window shafts falling through haze onto concrete,
// with an oscilloscope trace of the 50 Hz mains in the opening bar.
uniform float power; // = 0.0
uniform float scope; // = 0.0
uniform float amp;   // = 0.0

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) {
  float s = 0.0, a = 0.5;
  for (int k = 0; k < 4; k++) { s += a * vnoise(p); p *= 2.03; a *= 0.5; }
  return s;
}

void mainImage(out vec4 o, in vec2 f) {
  vec2 uv = f / iResolution.xy;           // y up
  float asp = iResolution.x / iResolution.y;

  // concrete: mottled, low contrast
  float conc = fbm(f * 0.012) * 0.6 + fbm(f * 0.08) * 0.4;
  vec3 col = vec3(0.030, 0.029, 0.027) * (0.75 + 0.5 * conc);

  // tall window shafts slanting down from top-right
  float sx = uv.x * asp + (1.0 - uv.y) * 0.32 - iTime * 0.004;
  float cell = fract(sx * 2.4);
  float shaft = smoothstep(0.30, 0.22, abs(cell - 0.5)) ;
  float mull = smoothstep(0.012, 0.0, abs(fract(sx * 2.4 * 4.0) - 0.5) - 0.47);
  shaft *= 1.0 - 0.6 * mull;
  float haze = fbm(vec2(uv.x * 3.0 - iTime * 0.05, uv.y * 1.5 + iTime * 0.03));
  float fall = pow(uv.y, 1.3);
  vec3 sodium = vec3(1.0, 0.56, 0.24);
  col += sodium * shaft * fall * (0.10 + 0.22 * haze) * power;

  // dust motes caught in the light
  vec2 g = f / 9.0 + vec2(0.0, iTime * 1.2);
  float d = step(0.996, hash(floor(g))) * shaft * power;
  col += sodium * d * 0.5;

  // oscilloscope trace of the mains, centred
  float y = (f.y - 0.5 * iResolution.y);
  float ph = uv.x * 6.2831 * 3.0 - iTime * 18.0;
  float wave = amp * (sin(ph) + 0.25 * sin(ph * 3.0 + 1.0) * amp / 98.0);
  float jitter = (hash(vec2(floor(iTime * 30.0), floor(f.x / 6.0))) - 0.5) * 0.5 * amp * pow(amp / 98.0, 4.0);
  float dist = abs(y - wave - jitter);
  float line = exp(-dist * dist / 5.0) + 0.35 * exp(-dist / 28.0);
  col += vec3(1.0, 0.93, 0.80) * line * scope;

  // floor haze glow at bottom
  col += sodium * 0.035 * power * smoothstep(0.35, 0.0, uv.y);

  o = vec4(col, 1.0);
}
