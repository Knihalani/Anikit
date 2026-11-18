// Calibre 9 — one engine-turned dial under three days of window light.
// Dial units: radius 1 = dial edge. Camera: zoom = half frame height in dial units.
uniform float zoom; // = 1.3
uniform float cx;   // = 0.0
uniform float cy;   // = 0.0
uniform float rot;  // = 0.0
uniform float T;    // = 0.0   clock hours since Friday 18:00
uniform float rate; // = 0.0   clock hours per film second (hand blur)
uniform float expo; // = 1.0
uniform float ink;  // = 1.0   dial printing visibility
uniform float tone; // = 0.0   0 silver, 1 salmon, 2 slate
uniform float sreal; // = 0.0  real-time seconds added to the seconds hand

#define PI 3.14159265
#define TAU 6.2831853

float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1,311.7)))*43758.5453); }
float vnoise(vec2 p){ vec2 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f);
  return mix(mix(hash(i),hash(i+vec2(1.0,0.0)),f.x), mix(hash(i+vec2(0.0,1.0)),hash(i+vec2(1.0,1.0)),f.x), f.y); }
vec2 rot2(vec2 p, float a){ float c=cos(a), s=sin(a); return vec2(c*p.x - s*p.y, s*p.x + c*p.y); }

// engine-turned relief, normalised to about [-1,1]
float relief(vec2 p, float px){
  float r = length(p); float a = atan(p.y, p.x);
  float h = 0.0;
  if (r < 0.30) {
    // centre: tight radiating wave (sunburst rose)
    float f = 34.0;
    float ph = a*48.0/TAU*TAU + 0.6*sin(r*f*TAU*0.25);
    float aa = clamp(1.6 - 48.0/TAU*px/max(r,0.02)*TAU*1.4, 0.0, 1.0);
    h = cos(a*48.0 + 9.0*r) * aa * smoothstep(0.02, 0.06, r);
  } else if (r < 0.325) {
    h = 0.0; // polished step
  } else if (r < 0.81) {
    // barleycorn: two counter-phased rose curves crossing
    float f = 58.0;
    float A = 0.055;
    float aa = clamp(1.6 - f*px*1.6, 0.0, 1.0);
    float h1 = cos(TAU*(r*f + f*A*0.3*sin(a*30.0)));
    float h2 = cos(TAU*(r*f - f*A*0.3*sin(a*30.0)));
    h = 0.5*(h1 + h2) * aa;
  } else {
    // chapter ring: fine circular satin
    float f = 260.0;
    float aa = clamp(1.4 - f*px*1.6, 0.0, 1.0);
    h = 0.25*cos(TAU*r*f) * aa + 0.1*(vnoise(vec2(a*400.0, r*30.0))-0.5);
  }
  return h;
}

// sun / moon from clock time
void sky(out vec3 L, out vec3 col, out float az, out float day){
  float s = fract((17.0 + T)/24.0);          // 0 midnight .5 noon
  float sunEl = sin(TAU*(s - 0.25));
  day = smoothstep(-0.12, 0.12, sunEl);
  az = (s - 0.25)*TAU;                        // dawn from the right, dusk from the left
  float el = 0.16 + 0.55*max(sunEl, 0.0);
  float moonAz = (s - 0.75)*TAU*0.5 + 2.2;
  float A = mix(moonAz, az, day);
  float E = mix(0.45, el, day);
  L = normalize(vec3(cos(A)*cos(E), sin(A)*cos(E), sin(E)));
  vec3 sunC = mix(vec3(1.0,0.50,0.20), vec3(1.0,0.90,0.78), smoothstep(0.05, 0.6, sunEl));
  float horizon = smoothstep(-0.12, 0.05, sunEl);
  col = mix(vec3(0.30,0.42,0.80)*0.45, sunC*3.2*horizon, day);
  az = A;
}

// window light patch with mullions, in world (dial) coordinates
float windowMask(vec2 p, float az, float el){
  vec2 d = vec2(cos(az), sin(az));
  vec2 c = d*0.35;
  vec2 q = rot2(p - c, -az);                 // q.x along light travel
  float stretch = 1.0/max(sin(el), 0.2);
  q.x /= stretch*0.55;
  float soft = 0.05*stretch;
  float box = smoothstep(1.25+soft, 1.25-soft, abs(q.y)) * smoothstep(1.1+soft, 1.1-soft, abs(q.x));
  float mull = smoothstep(0.035, 0.035+soft, abs(q.y - 0.05)) * smoothstep(0.03, 0.03+soft, abs(q.x + 0.15));
  float pane = smoothstep(0.02, 0.02+soft, abs(q.y + 0.65)) * smoothstep(0.02, 0.02+soft, abs(q.y - 0.75));
  return box*mull*pane;
}

vec3 env(vec3 R, float az, vec3 lc, float day){
  vec3 w = normalize(vec3(cos(az)*0.75, sin(az)*0.75, 0.62));
  float win = smoothstep(0.80, 0.97, dot(R, w));
  vec3 room = mix(vec3(0.020,0.025,0.035), vec3(0.10,0.095,0.09), day) * (0.5 + 0.5*R.z);
  return room + lc*win*1.4;
}

// hand coverage, local frame: y along the hand
float dauphine(vec2 q, float L, float w, float px){
  float y0 = 0.16*L;
  float hw = q.y < y0 ? mix(0.55, 1.0, clamp(q.y/y0, 0.0, 1.0))*w : w*clamp((L - q.y)/(L - y0), 0.0, 1.0);
  if (q.y < 0.0) hw = w*0.55;
  if (q.y < -0.07) hw = 0.0;
  return smoothstep(px, -px, abs(q.x) - hw) * step(q.y, L);
}
float secondsHand(vec2 q, float px){
  float body = smoothstep(px, -px, abs(q.x) - mix(0.0075, 0.0035, clamp(q.y/0.9, 0.0, 1.0))) * step(-0.22, q.y) * step(q.y, 0.92);
  float cw = smoothstep(px, -px, length(q - vec2(0.0, -0.16)) - 0.032);
  float hub = smoothstep(px, -px, length(q) - 0.038);
  return max(max(body, cw), hub);
}
vec2 toLocal(vec2 p, float th){ return vec2(p.x*cos(th) - p.y*sin(th), p.x*sin(th) + p.y*cos(th)); }
float wrapA(float a){ return a - TAU*floor((a + PI)/TAU); }
// angle-blurred evaluation: kind 0 hour, 1 minute, 2 seconds
float handCov(vec2 p, float th, float dlt, int kind, float px, out vec2 qo){
  float r = length(p);
  float phi = atan(p.x, p.y);                 // clockwise from 12
  float d = wrapA(phi - th);
  float t2 = th + clamp(d, -dlt, dlt);
  vec2 q = toLocal(p, t2);
  qo = q;
  float c; float wmax;
  if (kind == 0) { c = dauphine(q, 0.52, 0.048, px); wmax = 0.048; }
  else if (kind == 1) { c = dauphine(q, 0.84, 0.036, px); wmax = 0.036; }
  else { c = secondsHand(q, px); wmax = 0.008; }
  float k = 2.0*wmax/max(r, 0.04);
  return c * k/(k + 2.0*dlt);
}

void mainImage(out vec4 o, in vec2 f){
  vec2 uv = (f - 0.5*iResolution.xy)/iResolution.y;
  float px = 2.0*zoom/iResolution.y;
  vec2 p = rot2(uv*2.0*zoom, rot) + vec2(cx, cy);
  float r = length(p);
  vec3 L; vec3 lc; float az; float day;
  sky(L, lc, az, day);
  float el = asin(L.z);
  vec2 sh = L.xy/max(L.z, 0.12);               // shadow offset per unit height
  float win = windowMask(p, az, el);
  vec3 V = vec3(0.0, 0.0, 1.0);
  vec3 amb = mix(vec3(0.030,0.040,0.070), vec3(0.09,0.09,0.095), day);
  vec3 col;

  // hand angles (clockwise from 12)
  float hrs = 17.0 + T;
  float thH = hrs/12.0*TAU;
  float thM = hrs*TAU;
  float secs = fract(hrs*60.0)*60.0 + sreal;
  if (rate < 0.01) secs = floor(secs*8.0 + 0.001)/8.0;   // 28,800 vph: eight steps a second
  float thS = secs/60.0*TAU;
  float fr = 1.0/30.0*0.5;
  float dH = min(rate/12.0*TAU*fr, PI);
  float dM = min(rate*TAU*fr, PI);
  float dS = min(rate*60.0*TAU*fr, PI);

  if (r < 1.0) {
    // ---------------- dial
    float e = 0.0012;
    vec2 tuv0 = vec2(960.0 + p.x*500.0, 540.0 + p.y*500.0)/vec2(1920.0, 1080.0);
    float halo = 0.0;
    for (int j = 0; j < 4; j++) {
      vec2 o2 = vec2(j == 0 ? 7.0 : (j == 1 ? -7.0 : 0.0), j == 2 ? 7.0 : (j == 3 ? -7.0 : 0.0))/vec2(1920.0, 1080.0);
      halo = max(halo, texture(iChannel0, tuv0 + o2).a);
    }
    halo = max(halo, texture(iChannel0, tuv0).a) * ink;
    float amp = 0.55*(1.0 - 0.9*halo);
    float h0 = relief(p, px), hx = relief(p + vec2(e, 0.0), px), hy = relief(p + vec2(0.0, e), px);
    float freq = r < 0.30 ? 48.0/TAU/max(r,0.03) : (r < 0.81 ? 58.0 : 260.0);
    float k = amp/(freq*e*TAU)*0.9;
    vec3 n = normalize(vec3(-(hx - h0)*k, -(hy - h0)*k, 1.0));
    vec3 alb = r < 0.81 ? vec3(0.80,0.79,0.76) : vec3(0.74,0.74,0.73);
    if (tone > 0.5 && tone < 1.5) alb *= vec3(1.0, 0.70, 0.55);
    if (tone > 1.5) alb *= vec3(0.40, 0.46, 0.52);
    if (r > 0.955) { n = normalize(vec3(-p/r*0.9, 1.0)); alb *= 0.8; } // rehaut
    // printing: minute track
    float ang = atan(p.x, p.y);
    float m60 = ang*60.0/TAU;
    float isFive = step(abs(fract(m60/5.0 + 0.5) - 0.5)*5.0, 0.5);
    float tickW = mix(0.0032, 0.006, isFive);
    float dt = abs(fract(m60 + 0.5) - 0.5)*TAU/60.0*r;
    float tick = smoothstep(tickW + px, tickW - px*0.5, dt) * step(0.885 - isFive*0.02, r) * step(r, 0.935);
    float rings = smoothstep(0.0022 + px, 0.0022, abs(r - 0.935)) + smoothstep(0.0016 + px, 0.0016, abs(r - 0.885));
    // printing: names (channel 0 holds the dial printing at 500 px per dial unit)
    vec2 tuv = vec2(960.0 + p.x*500.0, 540.0 + p.y*500.0)/vec2(1920.0, 1080.0);
    float txt = texture(iChannel0, tuv).a * ink;
    float inkA = clamp(max(tick, rings)*ink + txt, 0.0, 1.0);
    // applied indices (polished steel batons)
    float hi = floor(ang/(TAU/12.0) + 0.5);
    vec2 lq = toLocal(p, hi*TAU/12.0);
    float bw = hi == 0.0 || hi == 12.0 ? 0.016 : 0.022;
    float bx = (hi == 0.0 || hi == 12.0) ? abs(abs(lq.x) - 0.024) : abs(lq.x);
    float baton = smoothstep(px, -px, max(bx - bw, abs(lq.y - 0.735) - 0.075));
    // index shadow (height .02)
    vec2 ps = p + sh*0.02;
    float hi2 = floor(atan(ps.x, ps.y)/(TAU/12.0) + 0.5);
    vec2 lq2 = toLocal(ps, hi2*TAU/12.0);
    float bx2 = (hi2 == 0.0 || hi2 == 12.0) ? abs(abs(lq2.x) - 0.024) : abs(lq2.x);
    float bshadow = smoothstep(0.012, -0.004, max(bx2 - 0.022, abs(lq2.y - 0.735) - 0.075));
    // case wall shadow on the dial
    float caseSh = smoothstep(0.985, 1.02, length(p + sh*0.06));
    // hand shadows
    vec2 dummy;
    float hS = max(max(handCov(p + sh*0.025, thH, dH, 0, 0.006, dummy), handCov(p + sh*0.04, thM, dM, 1, 0.006, dummy)), handCov(p + sh*0.055, thS, dS, 2, 0.005, dummy));
    float shadow = (1.0 - 0.75*bshadow*(1.0 - baton)) * (1.0 - 0.85*caseSh) * (1.0 - 0.7*hS);
    float lit = win * shadow;
    vec3 H = normalize(L + V);
    float ndl = max(dot(n, L), 0.0);
    float spec = pow(max(dot(n, H), 0.0), 18.0)*1.1 + pow(max(dot(n, H), 0.0), 140.0)*3.0;
    vec3 R = reflect(-V, n);
    col = alb*(amb*(0.6 + 0.4*n.z) + lc*ndl*0.55*lit) + alb*lc*spec*lit*0.9 + alb*env(R, az, lc, day)*0.18*(0.4 + 0.6*win);
    col = mix(col, vec3(0.018,0.018,0.022)*(0.6 + lit), inkA);
    // baton: faceted roof, polished
    if (baton > 0.0) {
      float side = sign(lq.x - (hi == 0.0 || hi == 12.0 ? sign(lq.x)*0.024 : 0.0));
      vec2 nl = vec2(side*0.75, 0.0);
      vec2 nw = toLocal(nl, -hi*TAU/12.0);
      vec3 bn = normalize(vec3(nw, 1.0));
      vec3 bR = reflect(-V, bn);
      float bs = pow(max(dot(bn, normalize(L + V)), 0.0), 60.0);
      vec3 bc = vec3(0.86,0.87,0.88)*(env(bR, az, lc, day)*1.6 + lc*bs*2.5*win + amb*0.5);
      col = mix(col, bc, baton);
    }
    // hands: hour, minute (dauphine, heat-blued), seconds
    for (int k2 = 0; k2 < 3; k2++) {
      vec2 q;
      float th = k2 == 0 ? thH : (k2 == 1 ? thM : thS);
      float dl = k2 == 0 ? dH : (k2 == 1 ? dM : dS);
      float c = handCov(p, th, dl, k2, px, q);
      if (c > 0.0) {
        float side = q.x >= 0.0 ? 1.0 : -1.0;
        vec2 nw = toLocal(vec2(side*(k2 == 2 ? 0.3 : 0.85), 0.0), -th);
        vec3 hn = normalize(vec3(nw, 1.0));
        vec3 hR = reflect(-V, hn);
        float hs = pow(max(dot(hn, normalize(L + V)), 0.0), 50.0);
        vec3 blue = vec3(0.035,0.07,0.30);
        vec3 hc = blue*(amb*1.5 + lc*max(dot(hn, L), 0.0)*0.5*win) + vec3(0.25,0.45,1.0)*(env(hR, az, lc, day)*0.9 + lc*hs*2.2*win);
        col = mix(col, hc, c);
      }
    }
    // crystal: faint domed reflection of the window
    vec3 cn = normalize(vec3(p*0.22, 1.0));
    col += env(reflect(-V, cn), az, lc, day)*0.035;
  } else {
    // ---------------- case, crown, lugs, strap, bench
    float inCase = smoothstep(1.17 + px, 1.17 - px, r);
    // crown at 3 o'clock
    vec2 cq = p - vec2(1.235, 0.0);
    float crown = smoothstep(px, -px, max(abs(cq.x) - 0.075, abs(cq.y) - 0.115));
    // lugs
    vec2 lp = vec2(abs(p.x), abs(p.y));
    float lug = smoothstep(px, -px, max(abs(lp.x - 0.585) - 0.07 + (lp.y - 0.9)*0.06, max(lp.y - 1.30, 0.80 - lp.y)));
    // strap
    float strap = smoothstep(px, -px, abs(p.x) - 0.50) * step(1.08, abs(p.y));
    // bench: dark oiled walnut, grain along x
    vec2 g = p*vec2(1.0, 9.0);
    float grain = vnoise(g*2.0 + vec2(0.0, 3.0*vnoise(p*1.3))) * 0.6 + vnoise(g*7.0)*0.4;
    vec3 bench = mix(vec3(0.050,0.032,0.020), vec3(0.11,0.07,0.04), grain);
    vec3 bn = normalize(vec3(0.0, (grain - 0.5)*0.3, 1.0));
    // shadow of the watch on the bench
    vec2 q = p + sh*0.10;
    float wsil = max(smoothstep(1.25, 1.10, length(q)), max(smoothstep(0.62, 0.5, abs(q.x))*step(1.0, abs(q.y)), 0.0));
    float bl = win*(1.0 - 0.85*wsil);
    col = bench*(amb*0.9 + lc*max(dot(bn, L), 0.0)*0.55*bl) + bench*lc*pow(max(dot(bn, normalize(L + V)), 0.0), 30.0)*0.3*bl;
    // strap: dark brown calf with stitching
    if (strap > 0.0) {
      float lt = vnoise(p*60.0)*0.5 + vnoise(p*180.0)*0.5;
      vec3 sc = mix(vec3(0.07,0.035,0.02), vec3(0.13,0.07,0.04), lt);
      float stitch = smoothstep(0.006, 0.002, abs(abs(p.x) - 0.435)) * step(0.45, fract(p.y*28.0));
      sc = mix(sc, vec3(0.55,0.48,0.38), stitch*0.8);
      float edge = smoothstep(0.50, 0.45, abs(p.x));
      vec3 sn = normalize(vec3(sign(p.x)*(1.0 - edge)*0.8, 0.0, 1.0));
      float sl = win*(1.0 - 0.8*smoothstep(1.25, 1.10, length(p + sh*0.05)));
      vec3 s = sc*(amb + lc*max(dot(sn, L), 0.0)*0.6*sl) + lc*pow(max(dot(sn, normalize(L+V)),0.0), 20.0)*0.06*sl;
      col = mix(col, s, strap);
    }
    // steel parts share one shading
    float steel = max(max(inCase, crown), lug);
    if (steel > 0.0) {
      vec3 sn;
      if (inCase > 0.5) {
        float sl = (r - 1.085)/0.085;
        sn = normalize(vec3(p/r*sl*1.3, 1.0));
        if (r < 1.02) sn = normalize(vec3(-p/r*1.5, 1.0));
      } else if (crown > 0.5) {
        float fl = sin(cq.y*170.0);
        sn = normalize(vec3(0.5*cq.x/0.075, fl*0.6 + cq.y*4.0, 1.0));
      } else {
        sn = normalize(vec3(sign(p.x)*(lp.x - 0.585)/0.07*0.9, 0.0, 1.0));
      }
      vec3 sR = reflect(-V, sn);
      float ss = pow(max(dot(sn, normalize(L + V)), 0.0), 80.0);
      float sl = win;
      vec3 stc = vec3(0.80,0.81,0.83)*(env(sR, az, lc, day)*1.3 + lc*ss*3.0*sl + amb*0.4) ;
      col = mix(col, stc, steel);
    }
  }
  col *= expo;
  // filmic
  col = 1.0 - exp(-col*1.25);
  col = pow(col, vec3(0.92, 0.95, 1.0));
  vec2 vq = f/iResolution.xy - 0.5;
  col *= 1.0 - 0.35*dot(vq, vq)*1.6;
  col += (hash(f + fract(iTime*7.13)*100.0) - 0.5)*0.018;
  o = vec4(clamp(col, 0.0, 1.0), 1.0);
}
