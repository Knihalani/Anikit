// Floodlight tower switching on, row by row, in match-night drizzle.
uniform float tOff; // = 0.0
float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float hash1(float x){ return fract(sin(x * 91.7) * 43758.5453); }
float noise(vec2 p){ vec2 i = floor(p), f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(hash(i), hash(i+vec2(1,0)), f.x), mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), f.x), f.y); }
float fbm(vec2 p){ float v = 0.0, a = 0.5; for (int i = 0; i < 5; i++){ v += a*noise(p); p *= 2.03; a *= 0.5; } return v; }
float seg(vec2 p, vec2 a, vec2 b){ vec2 pa = p-a, ba = b-a; float h = clamp(dot(pa,ba)/dot(ba,ba), 0.0, 1.0); return length(pa - ba*h); }
float rowOn(float r, float t){
  float t0 = 0.04 + 0.4*r;
  float s = smoothstep(t0, t0+0.03, t);
  float flick = 1.0 - 0.6*step(t0+0.04, t)*step(t, t0+0.08);
  float warm = 0.55 + 0.45*smoothstep(t0, t0+0.9, t);
  return s*flick*warm;
}
void mainImage(out vec4 o, in vec2 f){
  vec2 uv = (f - 0.5*iResolution.xy) / iResolution.y;
  float t = iTime + tOff;
  uv /= (1.0 + 0.05*t);
  uv += vec2(-0.02*t, 0.0);
  vec3 sky = mix(vec3(0.006,0.010,0.022), vec3(0.020,0.034,0.070), smoothstep(-0.6, 0.6, uv.y));
  vec3 col = sky;
  vec2 c = vec2(0.26, 0.17);
  float ang = -0.10; mat2 R = mat2(cos(ang), -sin(ang), sin(ang), cos(ang));
  vec2 q = R*(uv - c);
  float B = 0.0; for (int r = 0; r < 4; r++) B += rowOn(float(r), t); B *= 0.25;
  vec3 lampC = vec3(0.86, 0.93, 1.0);
  // beam with haze and rain
  vec2 dir = normalize(vec2(-0.78, -0.62));
  vec2 v = uv - c;
  float along = dot(v, dir);
  float perp = dot(v, vec2(-dir.y, dir.x));
  float w = 0.16 + 0.50*max(along, 0.0);
  float beam = step(0.0, along) * exp(-pow(perp/w, 2.0)*2.2) * exp(-along*0.9);
  float haze = fbm(uv*2.6 + vec2(t*0.22, -t*0.06));
  float haze2 = fbm(uv*7.0 + vec2(-t*0.4, t*0.1));
  float vol = beam * (0.35 + 0.9*haze*haze2*1.6);
  // rain streaks, lit only inside the beam
  float rain = 0.0;
  for (int k = 0; k < 2; k++){
    float sc = (k == 0) ? 90.0 : 150.0;
    vec2 rp = vec2(uv.x + uv.y*0.18, uv.y);
    float colI = floor(rp.x*sc);
    float rnd = hash1(colI + float(k)*13.0);
    float y = fract(rp.y*(k == 0 ? 1.6 : 2.4) + t*(k == 0 ? 3.2 : 4.4) + rnd*9.0);
    float xin = abs(fract(rp.x*sc) - 0.5);
    rain += step(y, 0.07) * smoothstep(0.18, 0.0, xin) * step(0.55, rnd) * (k == 0 ? 1.0 : 0.6);
  }
  col += lampC * B * (vol*1.15 + rain*beam*0.8);
  // general haze glow around the head
  col += lampC * B * 0.22 * exp(-length(q)*3.2) * (0.6 + 0.6*haze);
  // tower: lattice legs and braces
  float tw = 1e3;
  vec2 a1 = c + vec2(-0.07, -0.16), b1 = c + vec2(-0.16, -1.2);
  vec2 a2 = c + vec2( 0.07, -0.16), b2 = c + vec2( 0.18, -1.2);
  tw = min(tw, seg(uv, a1, b1)); tw = min(tw, seg(uv, a2, b2));
  for (int i = 0; i < 6; i++){
    float u0 = float(i)/6.0, u1 = float(i+1)/6.0;
    tw = min(tw, seg(uv, mix(a1,b1,u0), mix(a2,b2,u1)));
    tw = min(tw, seg(uv, mix(a2,b2,u0), mix(a1,b1,u1)));
  }
  float towerM = smoothstep(0.004, 0.0015, tw);
  col = mix(col, vec3(0.004,0.006,0.012) + lampC*B*0.05, towerM*0.95);
  // panel frame
  vec2 pq = abs(q) - vec2(0.245, 0.155);
  float box = max(pq.x, pq.y);
  float frame = step(box, 0.0);
  col = mix(col, vec3(0.012,0.014,0.020) + lampC*B*0.02, frame);
  // lamps 7 x 4
  float sp = 0.068;
  vec2 cell = q/sp + vec2(3.0, 1.5);
  vec2 id = clamp(floor(cell + 0.5), vec2(0.0), vec2(6.0, 3.0));
  vec2 lc = (id - vec2(3.0, 1.5))*sp;
  float d = length(q - lc);
  float on = rowOn(3.0 - id.y, t) * (0.9 + 0.2*hash(id));
  float rim = smoothstep(0.003, 0.0, abs(d - 0.027));
  col = mix(col, vec3(0.10,0.11,0.13), rim*frame);
  float disc = smoothstep(0.026, 0.022, d);
  vec3 off = vec3(0.03,0.035,0.045) + 0.04*smoothstep(0.02, 0.0, length(q - lc - vec2(-0.008, 0.008)));
  vec3 lit = mix(lampC*1.0, vec3(1.25, 1.22, 1.15), smoothstep(0.02, 0.0, d));
  col = mix(col, mix(off, lit, on), disc);
  col += lampC * on * 0.55 * exp(-d*55.0);
  // anamorphic streak
  col += vec3(0.55, 0.75, 1.0) * B * 0.22 * exp(-abs(q.y)*70.0) * exp(-abs(q.x)*1.6);
  // grain
  col += (hash(f + fract(t)*100.0) - 0.5) * 0.02;
  o = vec4(col, 1.0);
}
