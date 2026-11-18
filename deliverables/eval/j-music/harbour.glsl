// Harbour at night: ink sky, sea fog, a stone quay and a row of sodium lamps.
// Below the horizon it leaves black: tide.glsl (a filter) builds the mudflat and reflections.
uniform float camx;    // = 0.0   world pan
uniform float zoom;    // = 1.0
uniform float horizon; // = 0.40  screen fraction from the bottom
uniform float kill;    // = 0.0   lamps go out in order as this climbs 0..7
uniform float relight; // = 0.0   the one lamp that comes back
uniform float moon;    // = 0.25
uniform float spacing; // = 0.20
uniform float hero;    // = 0.0   extra bloom on the lamp that stays

float h1(float n){ return fract(sin(n*12.9898)*43758.5453); }
float h2(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7)))*43758.5453); }
float vn(vec2 p){ vec2 i = floor(p); vec2 f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(h2(i), h2(i+vec2(1.0,0.0)), f.x), mix(h2(i+vec2(0.0,1.0)), h2(i+vec2(1.0,1.0)), f.x), f.y); }
float fbm(vec2 p){ float s = 0.0; float a = 0.5; for (int i = 0; i < 5; i++){ s += a*vn(p); p = p*2.03 + vec2(1.7, 9.2); a *= 0.5; } return s; }

float lampState(float k){
  float th = clamp(k + 3.0, 0.0, 6.0);
  float d = clamp(kill - th, 0.0, 1.0);
  float fl = step(0.42, h1(floor(iTime*22.0) + k*17.0));
  float dying = step(0.001, d)*step(d, 0.999);
  float b = (1.0 - smoothstep(0.0, 1.0, d))*mix(1.0, fl*0.8 + 0.2, dying);
  // the lamp that stays on (k = 1)
  float isR = 1.0 - step(0.5, abs(k - 1.0));
  float rf = step(0.3, h1(floor(iTime*18.0) + 3.0));
  float rl = clamp(relight, 0.0, 1.0);
  float r = rl*mix(rf, 1.0, smoothstep(0.55, 0.95, rl));
  return max(b, isR*r);
}

void mainImage(out vec4 o, in vec2 f){
  vec2 uv = f/iResolution.xy;
  float asp = iResolution.x/iResolution.y;
  vec2 q = vec2((uv.x - 0.5)*asp, uv.y - horizon);
  vec2 w = q/zoom + vec2(camx, 0.0);
  if (q.y < 0.0) { o = vec4(0.0, 0.0, 0.0, 1.0); return; }

  // total light still burning, drives the fog glow
  float total = 0.0;
  for (int j = -6; j <= 8; j++){ total += lampState(float(j)); }
  float glowAmt = total/15.0;

  // sky: deep ink teal, a sodium bloom sitting in the sea fog
  float y = w.y;
  vec3 top = vec3(0.012, 0.030, 0.045);
  vec3 low = vec3(0.040, 0.085, 0.100);
  vec3 col = mix(low, top, smoothstep(0.0, 0.55, y));
  // clouds, faintly moonlit
  float cl = fbm(vec2(w.x*1.6 + iTime*0.012 + camx*0.6, y*4.0));
  col += vec3(0.04, 0.06, 0.07)*smoothstep(0.45, 0.85, cl)*(0.4 + moon)*smoothstep(0.05, 0.3, y);
  // stars
  vec2 sq = vec2(w.x*0.6 + camx*0.4, y)*220.0;
  vec2 sg = floor(sq);
  float st = step(0.996, h2(sg))*smoothstep(0.22, 0.05, length(fract(sq) - 0.5))*smoothstep(0.12, 0.4, y)*(0.5 + 0.5*sin(iTime*3.0 + h2(sg)*40.0));
  col += vec3(0.5, 0.6, 0.7)*st*0.35*(1.0 - smoothstep(0.55, 0.85, cl));
  // moon
  vec2 mp = vec2(0.62, 0.40);
  float md = length(vec2((uv.x - 0.5)*asp, uv.y - horizon) - mp);
  col += vec3(0.75, 0.82, 0.86)*moon*(smoothstep(0.034, 0.030, md)*1.4 + 0.012/(md*md*60.0 + 0.4));
  // sodium fog bank on the horizon
  float fogN = 0.6 + 0.4*fbm(vec2(w.x*3.0 + iTime*0.05, y*14.0));
  col += vec3(1.0, 0.42, 0.10)*(glowAmt + hero*relight*0.25)*exp(-y*14.0)*0.30*fogN;
  col += vec3(0.45, 0.62, 0.72)*moon*exp(-y*10.0)*0.06;

  // far headland: two ridges with a few town lights
  float hx = w.x*0.35 + camx*0.15;
  float ridge1 = 0.060 + 0.035*fbm(vec2(hx*2.2, 3.1));
  float ridge2 = 0.032 + 0.020*fbm(vec2(hx*5.0, 7.7));
  vec3 hill1 = mix(col, vec3(0.020, 0.035, 0.042), 0.85);
  col = mix(col, hill1, smoothstep(ridge1 + 0.002, ridge1 - 0.002, y));
  vec2 tq = vec2(hx*300.0, y*300.0);
  vec2 tg = floor(tq);
  float town = step(0.975, h2(tg))*smoothstep(0.30, 0.12, length(fract(tq) - 0.5))*step(y, ridge1 - 0.006)*step(ridge2, y);
  col += vec3(1.0, 0.55, 0.22)*town*0.9*clamp(1.6 - kill*0.3, 0.0, 1.0);
  col = mix(col, vec3(0.010, 0.018, 0.022), smoothstep(ridge2 + 0.002, ridge2 - 0.002, y));

  // the quay wall, lit from above
  float wallTop = 0.020;
  if (y < wallTop) {
    float stone = fbm(vec2(w.x*60.0, y*90.0));
    float course = smoothstep(0.0, 0.0015, abs(fract(y*180.0) - 0.5) - 0.42);
    vec3 wc = vec3(0.025, 0.030, 0.030)*(0.6 + 0.8*stone)*(1.0 - 0.4*course);
    col = wc;
  }

  // lamps
  float sp = spacing;
  float kc = floor(w.x/sp + 0.5);
  for (int j = -4; j <= 4; j++){
    float k = kc + float(j);
    float lx = k*sp + (h1(k*3.1) - 0.5)*sp*0.12;
    float ly = 0.105 + (h1(k*5.7) - 0.5)*0.006;
    float b = lampState(k);
    // post: thin iron column and an arm
    float dx = w.x - lx;
    float post = smoothstep(0.0024, 0.0012, abs(dx))*step(wallTop, y)*step(y, ly);
    float arm = smoothstep(0.0018, 0.0008, abs(y - ly - 0.004))*step(-0.012, dx)*step(dx, 0.002);
    vec3 iron = vec3(0.012, 0.012, 0.012) + vec3(1.0, 0.45, 0.15)*b*0.10*smoothstep(ly - 0.05, ly, y);
    col = mix(col, iron, clamp(post + arm, 0.0, 1.0));
    // lantern head and its light
    vec2 hp = vec2(lx - 0.010, ly - 0.002);
    vec2 dv = (w - hp)*vec2(1.0, 1.25);
    float d2 = dot(dv, dv);
    vec3 warm = mix(vec3(0.85, 0.16, 0.03), vec3(1.0, 0.62, 0.26), b);
    float core = exp(-d2/(0.000018));
    float halo = 0.00016/(d2 + 0.00016);
    float wide = exp(-sqrt(d2)*9.0);
    float hb = 1.0 + hero*(1.0 - step(0.5, abs(k - 1.0)))*2.5;
    col += warm*b*(core*3.2 + halo*0.55*hb + wide*0.10*hb*hb);
    // pool of light on the quay top
    col += vec3(1.0, 0.5, 0.18)*b*0.25*exp(-abs(w.x - hp.x)*30.0)*smoothstep(0.0, wallTop, y)*step(y, wallTop);
    // dying filament afterglow
    float dead = 1.0 - b;
    col += vec3(0.6, 0.08, 0.02)*core*dead*clamp(kill - clamp(k + 3.0, 0.0, 6.0), 0.0, 1.0)*0.4*step(kill, clamp(k+3.0,0.0,6.0) + 1.6);
  }
  o = vec4(col, 1.0);
}
