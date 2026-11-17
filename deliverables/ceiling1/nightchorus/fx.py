# GLSL treatments: each is a body that sees  f (frag px), uv, R (res), m (mask 0..1), and helper mb(lod)
HEAD = """
float mb(vec2 uv, float l){ return textureLod(iChannel0, uv, l).a; }
float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1,311.7)))*43758.5453); }
"""
def shader(body, bg="o = vec4(0.0);"):
    return HEAD + "void mainImage(out vec4 o, in vec2 f){ vec2 R=iResolution.xy; vec2 uv=f/R; " + ("" if "float m=0.0" in body else "float m=texture(iChannel0,uv).a; ") + body + " }"

NEON = """
  float b1=mb(uv,1.5), b4=mb(uv,4.0), b6=mb(uv,6.0);
  float tube=exp(-pow((b1-0.5)/0.17,2.0));
  float core=exp(-pow((b1-0.5)/0.07,2.0));
  float fl = 0.85+0.15*step(0.2,hash(vec2(floor(iTime*24.0),3.0)));
  vec3 c = TINT;
  vec3 col = (c*tube*1.2 + vec3(1.0,0.95,0.9)*core*0.9)*fl + c*(b4*0.35+b6*0.35)*fl;
  float a = clamp(max(max(tube, core), (b4*0.35+b6*0.45)), 0.0, 1.0);
  o = vec4(col, a);
"""
BULBS = """
  float cs=CELL; vec2 cell=floor(f/cs); vec2 c=(cell+0.5)*cs; float mm=texture(iChannel0,c/R).a;
  float r=length(f-c)/cs; float on=step(0.5,mm);
  float chase = 0.75+0.25*step(0.5,fract((cell.x+cell.y)*0.25 - iTime*4.0));
  float bulb=smoothstep(0.36,0.30,r); float halo=exp(-r*r*7.0);
  vec3 w=TINT;
  vec3 col = on*(w*bulb*1.3*chase + w*halo*0.45) + (1.0-on)*w*0.05*bulb;
  o = vec4(col, max(on*max(bulb,halo*0.8), BGA*bulb));
"""
MOIRE = """
  float r=length(f-R*vec2(0.5,0.5)); float ring=smoothstep(0.35,0.65,0.5+0.5*sin(r*0.55 - iTime*12.0));
  vec3 col = mix(TINT, vec3(1.0,0.98,0.92), ring);
  o = vec4(col*m, m);
"""
HALFTONE = """
  float ang=0.785; mat2 rot=mat2(cos(ang),-sin(ang),sin(ang),cos(ang));
  vec2 q=rot*f; float cs=CELL*(1.0+2.0*exp(-iTime*10.0)); vec2 cc=(floor(q/cs)+0.5)*cs; vec2 back=transpose(rot)*cc;
  float v=textureLod(iChannel0, back/R, 2.5).a;
  float shade = mix(1.0, 0.45, uv.y);
  float rad = sqrt(v*shade)*0.62*cs;
  float d = length(q-cc);
  float dot_ = smoothstep(rad+0.8, rad-0.8, d);
  o = vec4(TINT*dot_, dot_);
"""
STRIPES = """
  float s = step(0.5, fract((f.x + f.y*0.0)/14.0 + iTime*0.0));
  float h = step(0.5, fract((f.y+iTime*90.0)/18.0));
  vec3 col = mix(TINT, TINT2, h);
  o = vec4(col*m, m);
"""
LED = """
  float cs=CELL; vec2 cell=floor(f/cs); vec2 c=(cell+0.5)*cs; float mm=texture(iChannel0,c/R).a;
  vec2 d=abs(f-c)/cs; float px=step(d.x,0.40)*step(d.y,0.40);
  float sub = fract(f.x/cs*3.0);
  vec3 rgb = vec3(step(sub,0.333), step(0.333,sub)*step(sub,0.666), step(0.666,sub));
  vec3 base = TINT;
  vec3 col = (base*0.55 + rgb*base*0.9)*px*mm*1.2 + base*0.04*px;
  o = vec4(col, px*max(mm, BGA));
"""
EXTRUDE = """
  float acc=0.0; vec3 col=vec3(0.0); float a=0.0;
  for(int k=0;k<28;k++){ float fk=float(k); vec2 off=vec2(-1.0,1.0)*fk*2.2/R; float s=texture(iChannel0,uv+off).a;
     vec3 c = mix(TINT2, TINT2*0.25, fk/28.0); if(s>0.5 && a<0.5){ col=c; a=1.0; } }
  float face=m; col = mix(col, TINT, face); a=max(a,face);
  float edge = clamp(abs(m-mb(uv,1.0))*3.0,0.0,1.0); col = mix(col, vec3(0.05), edge*face*0.0);
  o = vec4(col*a, a);
"""
CHROMA = """
  float oo=OFF*(1.0+5.0*exp(-iTime*9.0)); float r=texture(iChannel0, uv+vec2(oo,0.0)/R).a, g=m, b=texture(iChannel0, uv-vec2(oo,0.0)/R).a;
  vec3 col=vec3(r, g, b); col = col*TINT;
  o = vec4(col, max(max(r,g),b));
"""
NEON = """
  float b1=mb(uv,1.0), b3=mb(uv,3.0), b5=mb(uv,5.0);
  float tube=exp(-pow((b1-0.5)/0.16,2.0));
  float core=exp(-pow((b1-0.5)/0.06,2.0));
  float glow=exp(-pow((b3-0.5)/0.32,2.0))*0.55 + exp(-pow((b5-0.45)/0.4,2.0))*0.35;
  float fl = 0.88+0.12*step(0.25,hash(vec2(floor(iTime*24.0),SEED)));
  vec3 c = TINT;
  vec3 col = (c*tube*1.3 + vec3(1.0,0.96,0.92)*core*0.8 + c*glow)*fl*ON;
  float a = clamp(max(max(tube, core), glow)*ON, 0.0, 1.0);
  o = vec4(col, a);
"""
EXTRUDE = """
  vec3 col=vec3(0.0); float a=0.0;
  for(int k=1;k<15;k++){ float fk=float(k); vec2 off=DIR*fk*2.0*min(1.0,0.25+iTime*5.0)/R; float s=texture(iChannel0,uv+off).a;
     float hit = step(0.5,s)*(1.0-a); col += hit*mix(TINT2, TINT2*0.18, fk/15.0); a=max(a,step(0.5,s)); }
  col = mix(col, TINT, m); a=max(a,m);
  o = vec4(col*a, a);
"""
