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
