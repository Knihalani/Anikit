// SIGNAL — one material: 73 luminous threads. A single line hears a pulse,
// multiplies into static, locks into coherent waves, then lifts into the word.
#define NL 73
#define CJ 36.0

float hash(float n){ return fract(sin(n*127.1)*43758.5453); }
float vnoise(vec2 p){
  vec2 i = floor(p); vec2 f = fract(p); f = f*f*(3.0-2.0*f);
  float n = i.x + i.y*57.0;
  return mix(mix(hash(n), hash(n+1.0), f.x), mix(hash(n+57.0), hash(n+58.0), f.x), f.y)*2.0 - 1.0;
}
float eo(float a, float b, float t){ float x = clamp((t-a)/(b-a), 0.0, 1.0); return 1.0 - pow(1.0-x, 4.0); }
float es(float a, float b, float t){ return smoothstep(a, b, t); }

// one pulse: an ECG-like blip born at the centre, running out both ways
float pulse(float x, float t, float tk, float xc){
  float a = t - tk;
  if (a < 0.0) return 0.0;
  float dx = abs(x - xc) - 0.75*a;
  float g = exp(-dx*dx/0.0016);
  return g * sin(dx*75.0) * exp(-a*0.55) * smoothstep(0.0, 0.05, a);
}

float pulses(float x, float t, float xc){
  return pulse(x,t,1.875,xc) + pulse(x,t,3.125,xc) + 0.8*pulse(x,t,4.375,xc) + 0.7*pulse(x,t,4.6875,xc)
       + 0.5*pulse(x,t,6.25,xc) + 0.5*pulse(x,t,7.5,xc) + 0.6*pulse(x,t,8.75,xc) + 0.7*pulse(x,t,9.375,xc);
}

float S;      // spacing
float Y0;     // first line
float ASP;

// displacement of line j at x; returns (disp, crest, inLetter)
vec3 lineDisp(float j, float x, float t){
  float r = (j - CJ)/CJ;
  float yb = Y0 + j*S;
  float xc = ASP*0.5;
  float d = 0.0;
  // pulses: centre line big, field lines fade with distance
  float pa = mix(0.085, 0.06*S/0.0116*0.25, es(5.0, 6.0, t));
  d += pulses(x, t, xc) * pa * exp(-abs(r)*mix(60.0, 3.0, es(5.0, 6.0, t)));
  // static
  float chaos = eo(5.0, 6.2, t) * (1.0 - es(9.9, 10.6, t));
  float fl = floor(t*24.0);
  d += chaos * S * (1.3*vnoise(vec2(x*7.0 + t*2.5, j*0.61 + t*1.3)) + 0.55*vnoise(vec2(x*70.0, j*3.1 + fl*1.7)));
  // coherence: circular waves from a source, then a second one -> interference
  float coh = eo(10.0, 11.4, t) * (1.0 - 0.8*es(15.0, 16.0, t));
  vec2 s1 = vec2(ASP*0.63, 0.46);
  vec2 s2 = vec2(ASP*0.30, 0.58);
  float d1 = length(vec2(x, yb) - s1);
  float d2 = length(vec2(x, yb) - s2);
  float w1 = sin(48.0*d1 - 7.5*t) * exp(-d1*1.1);
  float k2 = eo(12.5, 13.6, t);
  float w2 = sin(48.0*d2 - 7.5*t + 1.3) * exp(-d2*1.1) * k2;
  float w = (w1 + w2) / (1.0 + 0.6*k2);
  d += coh * S * 1.55 * w;
  float crest = coh * smoothstep(0.25, 0.95, w);
  // the word: lines lift inside the letters
  float txt = eo(15.0, 16.3, t) * (1.0 - es(18.7, 19.3, t));
  float m = texture(iChannel0, vec2(x/ASP, yb)).r;
  float bt = es(15.0, 16.0, t);
  float m2 = m * txt;
  d += m2 * S * 0.42;
  return vec3(d, crest, m2);
}

void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  ASP = iResolution.x / iResolution.y;
  vec2 p = f / iResolution.y;
  // camera: slow push, a small punch on the downbeats
  float zoom = 1.0 + 0.05*t/20.0 + 0.025*exp(-max(t-5.0,0.0)*6.0)*step(5.0,t) + 0.02*exp(-max(t-10.0,0.0)*5.0)*step(10.0,t);
  vec2 c = vec2(ASP*0.5, 0.5);
  p = (p - c)/zoom + c;
  float px = 1.0/iResolution.y/zoom;

  S = 0.0116;
  Y0 = 0.5 - CJ*S;

  // how many lines are alive, and how much of the centre line is drawn
  float spread = eo(5.0, 7.6, t) * (1.0 - es(18.8, 19.5, t));
  float half_ = eo(0.35, 2.1, t) * (ASP*0.5 + 0.05) * (1.0 - es(19.4, 19.95, t));

  float jf = (p.y - Y0)/S;
  float j0 = floor(jf + 0.5);
  vec3 col = vec3(0.0);
  vec3 white = vec3(0.93, 0.91, 0.87);
  vec3 orange = vec3(1.0, 0.30, 0.08);
  float flash = 0.5*exp(-max(t-10.0,0.0)*3.0)*step(10.0,t);

  for (int k = 0; k < 8; k++){
    float j = (k < 7) ? j0 + float(k) - 3.0 : CJ;
    if (k < 7 && abs(j - CJ) < 0.5) continue;
    if (j < 0.0 || j > 72.0) continue;
    float r = abs(j - CJ)/CJ;
    float vis = (abs(j-CJ) < 0.5) ? 1.0 : smoothstep(spread + 0.03, spread - 0.03, r);
    vis *= step(0.0001, spread) + ((abs(j-CJ) < 0.5) ? 1.0 : 0.0);
    vis = min(vis, 1.0);
    if (vis <= 0.0) continue;
    vec3 a = lineDisp(j, p.x, t);
    float e = 0.0025;
    float b = lineDisp(j, p.x + e, t).x;
    float slope = (b - a.x)/e;
    float yl = Y0 + j*S + a.x;
    float dd = abs(p.y - yl) / sqrt(1.0 + slope*slope);
    float txt = eo(15.0, 16.3, t) * (1.0 - es(18.7, 19.3, t));
    float w = 0.00085 + a.z*0.0024;
    float core = smoothstep(w + px, w - px*0.5, dd);
    float glow = exp(-dd/0.0045)*0.22 + exp(-dd/0.02)*0.05;
    // extent along x
    float ext = (abs(j-CJ) < 0.5) ? smoothstep(half_ + 0.004, half_ - 0.004, abs(p.x - ASP*0.5)) : 1.0;
    // static flicker
    float chaos = eo(5.0, 6.2, t) * (1.0 - es(9.9, 10.6, t));
    float flick = 1.0 - chaos*0.55*hash(j*1.37 + floor(t*20.0));
    float dim = mix(1.0, 0.22, txt*(1.0 - a.z)*(1.0 - step(0.5, a.z)) );
    dim = mix(dim, 1.0, a.z);
    float base = mix(1.0, 0.55, smoothstep(0.0, 1.0, r)) ;
    vec3 lc = mix(white, orange, max(a.y, a.z));
    // the centre line carries the signal colour while it is alone
    float alone = 1.0 - es(5.0, 6.0, t);
    if (abs(j-CJ) < 0.5) lc = mix(lc, mix(white, orange, 0.0), alone);
    float I = vis * ext * flick * dim * base * (1.0 + flash);
    col += lc * (core + glow) * I;
  }
  // a soft point of light where the signal is born
  float bornA = es(0.0, 0.5, t) * (1.0 - eo(0.4, 1.2, t));
  float dc = length(p - c);
  col += white * (exp(-dc/0.004) * 1.2 + exp(-dc/0.03)*0.12) * bornA;
  // the source glows during coherence
  float coh = eo(10.0, 11.4, t) * (1.0 - es(15.0, 16.0, t));
  col += orange * exp(-length(p - vec2(ASP*0.63, 0.46))/0.06) * 0.18 * coh;
  // edge falloff
  float ex = smoothstep(0.0, 0.25, p.x) * smoothstep(0.0, 0.25, ASP - p.x);
  col *= mix(0.35, 1.0, ex);
  col += vec3(0.018, 0.016, 0.02);
  o = vec4(col, 1.0);
}
