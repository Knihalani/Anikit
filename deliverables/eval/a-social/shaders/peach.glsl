float hash(vec2 p){ p=fract(p*vec2(123.34,456.21)); p+=dot(p,p+45.32); return fract(p.x*p.y); }
float noise(vec2 p){ vec2 i=floor(p), f=fract(p); vec2 u=f*f*(3.0-2.0*f);
  return mix(mix(hash(i),hash(i+vec2(1.0,0.0)),u.x), mix(hash(i+vec2(0.0,1.0)),hash(i+vec2(1.0,1.0)),u.x),u.y); }
float fbm(vec2 p){ float a=0.5, s=0.0; for(int i=0;i<5;i++){ s+=a*noise(p); p=p*2.03+vec2(1.7,9.2); a*=0.5; } return s; }
mat2 rot(float a){ float c=cos(a), s=sin(a); return mat2(c,-s,s,c); }
float vor(vec2 x){ vec2 n=floor(x), f=fract(x); float md=8.0;
  for(int j=-1;j<=1;j++) for(int i=-1;i<=1;i++){ vec2 g=vec2(float(i),float(j));
    vec2 o=vec2(hash(n+g),hash(n+g+17.0)); vec2 r=g+o-f; md=min(md,dot(r,r)); }
  return sqrt(md); }
vec3 hexc(float r,float g,float b){ return vec3(r,g,b)/255.0; }
// flat colour field lit softly from the top-left, with paper tooth
vec3 field(vec2 f, vec3 c){
  vec2 uv=f/iResolution.xy;
  float l=0.80+0.32*smoothstep(1.5,0.0,length(uv-vec2(0.15,0.95)));
  float g=(hash(floor(f*0.5)+floor(iTime*12.0))-0.5)*0.035 + (fbm(f*0.012)-0.5)*0.06;
  return c*l + g;
}
// spring entrance: returns (angle, scale)
vec2 enterSpin(float t, float spin){
  float e=exp(-6.5*t);
  return vec2(spin*e, 1.0-0.10*e*cos(13.0*t));
}
// Shot 3: a ripe peach halved, the stone still in it.
const vec2 C = vec2(540.0, 1100.0);
const float R = 410.0;
void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  vec2 es = enterSpin(t, 1.0);
  vec2 p = (f - C) / R;
  vec3 col = field(f, hexc(92.0, 22.0, 52.0));
  float sh = smoothstep(1.25, 0.85, length(p - vec2(0.09, -0.12)));
  col *= 1.0 - 0.55 * sh;
  vec2 q = rot(es.x - 0.03 * t) * p / es.y;
  q.x *= 0.97;
  float r = length(q);
  float a = atan(q.y, q.x);
  float aa = 1.6 / R;
  float edgeR = 1.0 + 0.03 * sin(a * 2.0 + 0.7) + 0.012 * sin(a * 5.0 + 2.0);
  float inside = smoothstep(edgeR + aa, edgeR - aa, r);
  float rn = r / edgeR;
  // stone cavity
  vec2 s = rot(0.35) * (q - vec2(0.02, 0.03));
  float sd = length(s / vec2(0.46, 0.34)) + 0.06 * fbm(s * 6.0) - 1.0;
  float bleed = smoothstep(0.75, 0.0, sd + 0.25 * fbm(q * 5.0 + 3.0));
  float fib = noise(vec2(a * 46.0, rn * 3.0)) * 0.5 + noise(vec2(a * 120.0, rn * 6.0)) * 0.5;
  vec3 flesh = mix(hexc(255.0, 186.0, 74.0), hexc(252.0, 150.0, 60.0), smoothstep(0.2, 0.9, rn));
  flesh = mix(flesh, hexc(232.0, 76.0, 46.0), bleed * 0.9);
  flesh *= 0.92 + 0.12 * fib;
  // skin rim
  vec3 skin = mix(hexc(200.0, 40.0, 40.0), hexc(240.0, 120.0, 50.0), noise(vec2(a * 4.0, 1.0)));
  flesh = mix(flesh, skin, smoothstep(0.955, 0.985, rn));
  // stone: wrinkled, deep red-brown
  float w = fbm(s * 7.0);
  float ridges = abs(sin(w * 14.0 + s.y * 6.0));
  vec3 stone = mix(hexc(96.0, 30.0, 18.0), hexc(166.0, 70.0, 40.0), smoothstep(0.2, 0.9, ridges));
  stone *= 0.75 + 0.35 * smoothstep(0.3, -0.6, sd);
  float inStone = smoothstep(0.012, -0.012, sd);
  // the flesh dips into a red, glistening lip around the stone
  float lip = smoothstep(0.12, 0.0, sd) * (1.0 - inStone);
  flesh = mix(flesh, hexc(196.0, 46.0, 34.0), lip * 0.8);
  vec3 c = mix(flesh, stone, inStone);
  // wet sheen
  vec2 L = normalize(vec2(-0.55, 0.83));
  float sheen = smoothstep(0.55, 1.0, fbm(q * 4.0 + 7.0)) * smoothstep(-0.2, 0.8, dot(q, L)) * (1.0 - inStone);
  c += sheen * 0.22;
  c *= 0.9 + 0.12 * dot(q, L);
  col = mix(col, c, inside);
  o = vec4(col, 1.0);
}
