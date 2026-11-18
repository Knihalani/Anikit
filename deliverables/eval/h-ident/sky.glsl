// Dusk sky over a hayfield, with wind. dusk 0..1 drains the light; grass 0/1 draws the field.
uniform float dusk;  // = 0.0
uniform float grass; // = 1.0
uniform float ox;    // = 0.0
uniform float oy;    // = 0.0
float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float h1(float x){ return fract(sin(x * 91.345) * 47453.21); }
float vnoise(vec2 p){ vec2 i = floor(p), f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(hash(i), hash(i+vec2(1.0,0.0)), f.x), mix(hash(i+vec2(0.0,1.0)), hash(i+vec2(1.0,1.0)), f.x), f.y); }
float fbm(vec2 p){ float v = 0.0, a = 0.5; for (int k = 0; k < 5; k++){ v += a*vnoise(p); p = p*2.03 + vec2(1.7, 9.2); a *= 0.5; } return v; }
float blades(vec2 f, float dens, float hmax, float seed, float t){
  // thin tapered blades bending with the gusts
  float x = f.x / dens;
  float id = floor(x);
  float best = 0.0;
  for (int k = -1; k <= 1; k++){
    float c = id + float(k);
    float r = h1(c + seed);
    float h = hmax * (0.45 + 0.55*h1(c*1.7 + seed));
    float base = (c + 0.2 + 0.6*r) * dens;
    float gust = vnoise(vec2(c*0.05 - t*1.6, seed)) ;
    float y = f.y;
    float s = clamp(y / h, 0.0, 1.0);
    float bend = -(18.0 + 40.0*gust) * s*s * (0.6 + 0.4*sin(t*7.0 + c*1.3));
    float w = dens * 0.22 * (1.0 - s);
    float d = abs(f.x - (base + bend));
    best = max(best, step(y, h) * smoothstep(w + 1.0, w - 0.5, d));
  }
  return best;
}
void mainImage(out vec4 o, in vec2 f){
  vec2 R = iResolution.xy;
  f = f - vec2(ox, oy) * iResolution.y;
  vec2 uv = f / R;
  float ar = R.x / R.y;
  vec2 p = vec2(uv.x*ar, uv.y);
  float t = iTime;
  // sky
  vec3 low = vec3(1.00, 0.62, 0.30), mid = vec3(0.78, 0.38, 0.26), top = vec3(0.13, 0.15, 0.26);
  vec3 col = mix(low, mid, smoothstep(0.12, 0.5, uv.y));
  col = mix(col, top, smoothstep(0.42, 1.05, uv.y));
  // sun, low and to the left, just above the field
  vec2 sp = vec2(0.5*ar, 0.425);
  float d = length(p - sp);
  col += vec3(1.0, 0.70, 0.38) * exp(-d*3.5) * 0.6;
  col = mix(col, vec3(1.0, 0.95, 0.82), smoothstep(0.092, 0.086, d));
  // cloud streamers driven right-to-left by the wind
  vec2 cp = vec2(p.x*1.4 + t*0.42, p.y*5.0);
  float warp = fbm(cp*0.6 + vec2(t*0.15, 0.0));
  float c = fbm(cp + vec2(warp*1.2, 0.0));
  float band = smoothstep(0.22, 0.48, uv.y) * (1.0 - smoothstep(0.92, 1.1, uv.y));
  float cl = smoothstep(0.48, 0.78, c) * band;
  vec3 cloudCol = mix(vec3(0.98, 0.58, 0.36), vec3(0.30, 0.20, 0.27), smoothstep(0.35, 0.9, uv.y));
  cloudCol += vec3(1.0, 0.62, 0.32) * 0.55 * exp(-d*2.2) * smoothstep(0.75, 0.5, c);
  col = mix(col, cloudCol, cl * 0.9);
  // fine high wisps, faster
  float wisp = fbm(vec2(p.x*3.0 + t*0.9, p.y*22.0));
  col = mix(col, vec3(1.0, 0.8, 0.6), smoothstep(0.62, 0.8, wisp) * smoothstep(0.55, 0.8, uv.y) * 0.25);
  // wind-borne seeds streaking across
  vec2 sq = vec2(f.x/R.y*14.0 + t*9.0, f.y/R.y*14.0);
  vec2 cell = floor(sq); vec2 fr = fract(sq);
  float rr = hash(cell);
  vec2 sc = vec2(0.5 + 0.4*(hash(cell+3.1)-0.5), hash(cell+7.7));
  float mote = step(0.86, rr) * smoothstep(0.035, 0.0, abs(fr.y - sc.y)) * smoothstep(0.35, 0.0, abs(fr.x - sc.x));
  col = mix(col, vec3(1.0, 0.85, 0.65), mote * 0.5 * smoothstep(0.6, 0.15, uv.y) * grass);
  // the field: three ranks of grass, nearest darkest
  if (grass > 0.5) {
    float horizon = R.y * (0.115 + 0.012*vnoise(vec2(uv.x*6.0, 1.0)));
    float field = step(f.y, horizon);
    vec3 far = mix(col, vec3(0.32, 0.17, 0.12), 0.75);
    col = mix(col, far, field);
    float g1 = blades(f - vec2(0.0, horizon*0.5), 6.0, R.y*0.11, 1.0, t);
    col = mix(col, vec3(0.16, 0.08, 0.06), g1);
    float g2 = blades(f, 9.0, R.y*0.2, 5.0, t*1.1);
    col = mix(col, vec3(0.07, 0.035, 0.03), max(g2, step(f.y, R.y*0.03)));
    float g3 = blades(f + vec2(0.0, R.y*0.03), 16.0, R.y*0.3, 9.0, t*1.25);
    col = mix(col, vec3(0.025, 0.015, 0.012), g3);
  }
  col = mix(col, vec3(0.018, 0.014, 0.013), dusk);
  o = vec4(col, 1.0);
}
