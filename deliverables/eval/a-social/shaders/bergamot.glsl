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
// Shot 2: a bergamot cut across, wet, eleven segments.
const vec2 C = vec2(540.0, 1100.0);
const float R = 430.0;
void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  vec2 es = enterSpin(t, -1.1);
  vec2 p = (f - C) / R;
  vec3 col = field(f, hexc(24.0, 64.0, 40.0));
  float sh = smoothstep(1.25, 0.85, length(p - vec2(0.09, -0.12)));
  col *= 1.0 - 0.55 * sh;
  vec2 q = rot(es.x + 0.04 * t) * p / es.y;
  float r = length(q);
  float a = atan(q.y, q.x);
  float aa = 1.6 / R;
  float edgeR = 1.0 + 0.012 * sin(a * 3.0 + 1.0) + 0.008 * sin(a * 7.0);
  float inside = smoothstep(edgeR + aa, edgeR - aa, r);
  // rind with oil glands
  vec3 rind = mix(hexc(150.0, 170.0, 40.0), hexc(196.0, 200.0, 62.0), noise(q * 9.0));
  rind *= 0.85 + 0.15 * smoothstep(0.15, 0.6, vor(q * 55.0));
  rind *= 0.8 + 0.2 * smoothstep(edgeR, edgeR - 0.05, r);
  // pith
  vec3 pith = hexc(244.0, 238.0, 206.0) * (0.92 + 0.08 * fbm(q * 14.0));
  // segments
  const float N = 11.0;
  float an = a + 0.05 * fbm(q * 3.0);
  float seg = an / (6.28318 / N);
  float fs = fract(seg) - 0.5;
  float id = floor(seg);
  float arc = (0.5 - abs(fs)) * (6.28318 / N) * r;      // distance to membrane, in radius units
  float segOut = 0.80 + 0.02 * sin(id * 2.3);
  float memb = smoothstep(0.010, 0.004, arc) + smoothstep(segOut - 0.012, segOut, r);
  float core = smoothstep(0.13, 0.09, r + 0.02 * noise(q * 20.0));
  memb = clamp(memb + core, 0.0, 1.0);
  // juice vesicles: teardrops along the radius
  vec2 vc = vec2(an * r * 26.0, r * 11.0) + id * 3.1;
  float v = vor(vc);
  float vcell = smoothstep(0.75, 0.15, v);
  vec3 flesh = mix(hexc(214.0, 196.0, 52.0), hexc(250.0, 238.0, 140.0), vcell);
  flesh = mix(flesh, hexc(186.0, 196.0, 70.0), 0.35 * smoothstep(0.55, 0.8, r));   // greener toward the pith
  // light: wet sheen
  vec2 L = normalize(vec2(-0.55, 0.83));
  float spec = pow(vcell, 6.0) * smoothstep(0.2, 1.0, dot(normalize(q + 1e-3), L) * 0.5 + 0.6);
  flesh += spec * 0.35;
  flesh *= 0.88 + 0.12 * smoothstep(0.0, 0.03, arc);
  vec3 c = flesh;
  c = mix(c, pith * 0.97, memb);
  c = mix(c, pith, smoothstep(segOut, segOut + 0.01, r));
  c = mix(c, rind, smoothstep(0.915, 0.93, r));
  // droplets on the cut face
  float dr = smoothstep(0.035, 0.02, vor(q * 9.0 + 5.0)) * step(r, 0.78);
  c = mix(c, c * 1.15 + 0.12, dr * 0.6);
  // overall roundness light
  c *= 0.9 + 0.12 * dot(q, L);
  col = mix(col, c, inside);
  o = vec4(col, 1.0);
}
