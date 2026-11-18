import json, math
BPM=132; b=60/BPM; BAR=4*b
W,H=1080,1920
FACE="Big Shoulders"
T=[BAR+0*3*b, BAR+1*3*b, BAR+2*3*b, BAR+3*3*b]; T+= [T[-1]+3*b+k*2*b for k in range(4)]
T_PULL=6*BAR; PULL=1.1; T_LOCK=7*BAR; LEN=15.0
SOD="#FFB347"; SODG="#FF8C1A"; ARC="#BFD8FF"; COPPER="#C9803F"
WHIP=0.30
def E(x): return f"smoothstep(0,1,smoothstep(0,1,clamp({x},0,1)))"
cam="+".join(f"1920*{E(f'(t-{tk-WHIP:.4f})/{WHIP}')}" for tk in T)
p=E(f"(t-{T_PULL:.4f})/{PULL}")
C_FIN=(9*1920)/2-(1000-960)/0.075
C=f"mix(960+({cam}),{C_FIN:.1f},{p})"; S=f"pow(0.075,{p})"
comps={}
# ---------- shaders ----------
HASH="float h1(float x){return fract(sin(x*91.7)*43758.5453);} float h(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);} float n1(float x){float i=floor(x),f=fract(x);f=f*f*(3.0-2.0*f);return mix(h1(i),h1(i+1.0),f);} float n(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.0-2.0*f);return mix(mix(h(i),h(i+vec2(1,0)),f.x),mix(h(i+vec2(0,1)),h(i+vec2(1,1)),f.x),f.y);}"
def arc(x0,y0,x1,y1,t_on,amp=60,t_off=99.0,width=None):
    """an electric arc between two points (comp px), as an alpha shader layer over the whole comp"""
    src=HASH+f"""
float strand(vec2 f, vec2 a, vec2 b, float seed, float amp){{ vec2 d=b-a; float L=length(d); vec2 u=d/L; vec2 nn=vec2(-u.y,u.x);
  float s=clamp(dot(f-a,u)/L,0.0,1.0); float tt=floor(iTime*24.0);
  float off=(n1(s*7.0+tt*3.1+seed)-0.5)*amp*1.0+(n1(s*23.0+tt*5.7+seed*3.0)-0.5)*amp*0.35; off*=sin(3.14159*s);
  float dd=abs(dot(f-a,nn)-off) + max(0.0,abs(dot(f-a,u)/L-0.5)-0.5)*L;
  return exp(-dd*dd/6.0)*1.2 + exp(-dd/14.0)*0.55 + exp(-dd/70.0)*0.25; }}
void mainImage(out vec4 o, in vec2 f){{ f.y=iResolution.y-f.y; float on=step({t_on:.4f},iTime)*step(iTime,{t_off:.4f});
  float fl=0.6+0.4*h1(floor(iTime*30.0)); vec2 a=vec2({x0:.1f},{y0:.1f}), b=vec2({x1:.1f},{y1:.1f});
  float v=strand(f,a,b,1.0,{amp:.1f})+0.6*strand(f,a,b,7.0,{amp*0.7:.1f});
  v*=on*fl; vec3 c=mix(vec3(0.45,0.62,1.0),vec3(1.0),clamp(v-0.8,0.0,1.0))*v; o=vec4(c,clamp(v,0.0,1.0)); }}"""
    return {"shader":src,"alpha":True,"blend":"add"}
PLASMA=HASH+"""
float fbm(vec2 p){float a=0.5,s=0.0;for(int i=0;i<5;i++){s+=a*n(p);p*=2.1;a*=0.5;}return s;}
void mainImage(out vec4 o, in vec2 f){ vec2 uv=f/iResolution.xy; float m=texture(iChannel0,uv).a; float t=iTime;
  float r=abs(fbm(f*0.006+vec2(t*0.7,-t*0.4))-0.5); float r2=abs(fbm(f*0.013+vec2(-t*1.1,t*0.6)+5.0)-0.5);
  float vein=exp(-r*40.0)+0.7*exp(-r2*55.0); vec3 base=vec3(0.06,0.12,0.42); vec3 c=base+vec3(0.55,0.75,1.0)*vein*1.6+vec3(1.0)*pow(vein,3.0)*0.8;
  o=vec4(c*m,m); }"""
def concrete(L, light=None):
    L.append({"src":"concrete.png","width":W,"height":H,"x":W/2,"y":H/2})
    L.append({"shape":"rect","width":W,"height":H,"fill":"#000","opacity":0.35,"x":W/2,"y":H/2})
def pool(L,x,y,r,color,t_on,base=0.0,amp=0.85):
    L.append({"shape":"circle","width":2*r,"height":2*r,"fill":f"radial-gradient({color}AA, {color}00)","x":x,"y":y,"blend":"add","blur":r*0.25,
              "opacity":f"=t<{t_on:.4f}?{base}:{amp}*(0.75+0.25*sin(t*53)*sin(t*31))"})
def busbar(L,t_on):
    L.append({"shape":"rect","width":26,"height":H,"fill":f"linear-gradient(90deg, #6E3E1C, {COPPER} 45%, #F0B077 55%, #7A4420)","x":64,"y":H/2})
    if t_on is None: return
    L.append({"shape":"rect","width":26,"height":H,"fill":"#FFD49A","x":64,"y":H/2,"blend":"add","opacity":f"=t<{t_on:.4f}?0:0.3+0.12*sin(t*40)"})
    L.append({"shape":"circle","width":40,"height":40,"fill":"#FFFFFF","glow":"#9CC2FF","x":64,
              "y":f"=-60+{H+120}*clamp((t-{t_on-WHIP:.4f})/{WHIP+0.06},0,1)","opacity":f"=(t>{t_on-WHIP:.4f} && t<{t_on+0.08:.4f})?1:0",
              "echo":{"count":8,"delay":0.012,"decay":0.7,"blend":"add"}})
def flick(t):
    return f"(t<{t:.4f}?0:(t<{t+0.034:.4f}?1:(t<{t+0.08:.4f}?0.15:(t<{t+0.12:.4f}?1:(t<{t+0.15:.4f}?0.45:1)))))"
def lit_text(L, base, t_on, unlit="#2B2B2A", color="#FFF3DE", glow=SODG, blurc=SODG):
    L.append(dict(base,color=unlit))
    fl=flick(t_on)
    L.append(dict(base,color=blurc,blur=40,blend="add",opacity="="+fl+"*0.85"))
    L.append(dict(base,color=color,glow=glow,opacity="="+fl))
def flash(L,t_on,color="#CFE2FF",k=18,a=0.6):
    L.append({"shape":"rect","width":W,"height":H,"fill":color,"x":W/2,"y":H/2,"blend":"add","opacity":f"=t<{t_on:.4f}?0:{a}*exp(-(t-{t_on:.4f})*{k})"})
def comp(k,L): comps[f"p{k}"]={"size":[W,H],"duration":LEN,"background":"#0A0A0A","layers":L}

# P0 the incomer: SUBSTATION in sodium tubes on concrete
L=[]; concrete(L); busbar(L,None)
pool(L,W/2,860,900,SOD,0.35,0.0,0.55)
nm={"text":"SUB\nSTAT\nION","font":FACE,"weight":900,"fit":True,"width":920,"height":1300,"size":900,"lineHeight":0.82,"align":"center","x":W/2+20,"y":860}
L.append(dict(nm,color="#2B2B2A"))
fl="(t<0.35?0:(t<0.39?1:(t<0.5?0.1:(t<0.55?1:(t<0.62?0.3:(t<0.7?1:(t<0.74?0.4:1)))))))"
L.append(dict(nm,color=SODG,blur=44,blend="add",opacity="="+fl+"*0.9")); L.append(dict(nm,color="#FFF0D6",glow=SODG,opacity="="+fl))
L.append({"text":"12—13 JUNE   THE OLD POWER STATION","font":FACE,"weight":800,"size":56,"color":"#E9E3D2","letterSpacing":"0.04em","x":W/2,"y":1700,"opacity":"=smoothstep(0.9,1.2,t)"})
comp(0,L)
# P1 ANCIENT METHODS: two copper contacts, an arc across, the name vertical up the wall
t=T[0]; L=[]; concrete(L); busbar(L,t)
pool(L,560,1180,820,"#9CC2FF",t)
L.append({"shape":"rect","width":300,"height":120,"fill":f"linear-gradient(180deg, #F0B077, {COPPER} 40%, #5E3416)","radius":14,"x":230,"y":1180})
L.append({"shape":"rect","width":300,"height":120,"fill":f"linear-gradient(180deg, #F0B077, {COPPER} 40%, #5E3416)","radius":14,"x":870,"y":1180})
L.append(arc(380,1180,720,1180,t,amp=90))
nm={"text":"ANCIENT METHODS","font":FACE,"weight":900,"size":330,"width":1800,"x":W/2,"y":560,"rotation":0,"letterSpacing":"-0.01em","lineHeight":0.8}
nm={"text":"ANCIENT\nMETHODS","font":FACE,"weight":900,"fit":True,"width":980,"height":760,"size":900,"lineHeight":0.82,"align":"left","x":W/2+20,"y":520}
lit_text(L,nm,t,color="#EAF2FF",glow="#6FA2FF",blurc="#5C8BFF")
flash(L,t); comp(1,L)
# P2 PAULA TEMPLE: an insulator stack, flashover crawling up it, name crossing behind and in front
t=T[1]; L=[]; concrete(L); busbar(L,t)
pool(L,700,960,760,"#9CC2FF",t)
behind={"text":"PAULA","font":FACE,"weight":900,"size":520,"width":2200,"x":W/2,"y":520,"letterSpacing":"-0.01em"}
lit_text(L,behind,t,color="#EAF2FF",glow="#6FA2FF",blurc="#5C8BFF")
L.append({"shape":"rect","width":36,"height":1500,"fill":"linear-gradient(90deg, #3A3A3A, #9A9A9A 50%, #2A2A2A)","x":W/2,"y":960})
for i in range(11):
    y=330+i*125
    L.append({"shape":"circle","width":420,"height":96,"fill":"radial-gradient(#C8743A, #6B2E10 70%, #2A1206)","x":W/2,"y":y})
    L.append({"shape":"circle","width":360,"height":40,"fill":"#FFFFFF22","x":W/2-20,"y":y-18})
L.append(arc(W/2+215,320,W/2+215,1600,t,amp=70))
front={"text":"TEMPLE","font":FACE,"weight":900,"size":395,"width":2200,"x":W/2,"y":1420,"letterSpacing":"-0.01em"}
lit_text(L,front,t+0.1,color="#EAF2FF",glow="#6FA2FF",blurc="#5C8BFF")
flash(L,t); comp(2,L)
# P3 DVS1: extreme scale, letters full of plasma
t=T[2]; L=[{"shape":"rect","width":W,"height":H,"fill":"#04060C","x":W/2,"y":H/2}]; busbar(L,t)
L.append({"shader":PLASMA,"alpha":True,"opacity":f"={flick(t)}",
          "channels":[{"layers":[{"text":"DVS\n1","font":FACE,"weight":900,"size":800,"lineHeight":0.78,"align":"center","x":W/2+30,"y":H/2,"width":1400,"color":"#fff"}]}]})
L.append({"text":"DVS\n1","font":FACE,"weight":900,"size":800,"lineHeight":0.78,"align":"center","x":W/2+30,"y":H/2,"width":1400,"color":"#1A2340","opacity":f"=t<{t:.4f}?1:0"})
flash(L,t,a=0.8); comp(3,L)
# P4 HÉCTOR OAKS: a bundle of cables sweeping through, the arc jumps between two cut ends, name set low and left
t=T[3]; L=[]; concrete(L); busbar(L,t)
pool(L,700,520,700,"#9CC2FF",t)
for i in range(9):
    y0=180+i*44; y1=900+i*60
    d=f"M-100 {y0} C 400 {y0+40}, 500 {y1-300}, 1180 {y1}"
    L.append({"shape":"path","d":d,"stroke":"#111","strokeWidth":40,"fill":"none","x":0,"y":0,"anchor":"topLeft"})
    L.append({"shape":"path","d":d,"stroke":"#3C3C3C","strokeWidth":8,"fill":"none","x":0,"y":-12,"anchor":"topLeft"})
L.append({"shape":"rect","width":60,"height":60,"fill":COPPER,"radius":6,"x":620,"y":470})
L.append({"shape":"rect","width":60,"height":60,"fill":COPPER,"radius":6,"x":900,"y":560})
L.append(arc(650,470,870,560,t,amp=50))
nm={"text":"HÉCTOR\nOAKS","font":FACE,"weight":900,"fit":True,"width":900,"height":680,"size":900,"lineHeight":0.82,"align":"left","x":540,"y":1480}
lit_text(L,nm,t,color="#FFF3DE",glow=SODG,blurc=SODG)
pool(L,520,1480,700,SOD,t,0.0,0.4)
flash(L,t); comp(4,L)
# P5 RROSE: all plasma, black type cut out of it
t=T[4]; L=[{"shape":"rect","width":W,"height":H,"fill":"#04060C","x":W/2,"y":H/2}]; busbar(L,t)
L.append({"shader":PLASMA,"alpha":True,"opacity":f"={flick(t)}","channels":[{"layers":[{"shape":"rect","width":W,"height":H,"fill":"#fff","x":W/2,"y":H/2}]}]})
L.append({"text":"RROSE","font":FACE,"weight":900,"size":440,"width":1400,"rotation":-90,"x":W/2+20,"y":H/2,"color":"#05060A","letterSpacing":"-0.02em"})
flash(L,t,a=0.9); comp(5,L)
# P6 PHASE FATALE: the danger plate: enamel yellow, black type, a burnt arc mark
t=T[5]; L=[{"shape":"rect","width":W,"height":H,"fill":"#0A0A0A","x":W/2,"y":H/2}]; busbar(L,t)
L.append({"shape":"rect","width":940,"height":1500,"fill":"#F2C21B","radius":40,"x":W/2+20,"y":H/2,"opacity":f"={flick(t)}"})
L.append({"shape":"rect","width":880,"height":1440,"fill":"none","stroke":"#111","strokeWidth":14,"radius":30,"x":W/2+20,"y":H/2,"opacity":f"={flick(t)}"})
L.append({"text":"PHASE\nFATALE","font":FACE,"weight":900,"fit":True,"width":800,"height":1100,"size":900,"lineHeight":0.8,"align":"center","x":W/2+20,"y":H/2+60,"color":"#111","opacity":f"={flick(t)}"})
L.append({"text":"DANGER","font":FACE,"weight":900,"size":110,"color":"#111","letterSpacing":"0.08em","x":W/2+20,"y":360,"opacity":f"={flick(t)}"})
flash(L,t,"#FFF4C0",a=0.7); comp(6,L)
# P7 KIA: a vast dark hall, one caged lamp, the name small in its pool
t=T[6]; L=[]; concrete(L); busbar(L,t)
L.append({"shape":"rect","width":W,"height":H,"fill":"#000","opacity":0.5,"x":W/2,"y":H/2})
pool(L,W/2,1000,620,SOD,t,0.0,0.9)
L.append({"shape":"line","x":W/2,"y":330,"width":4,"height":420,"fill":"#222"})
L.append({"shape":"circle","width":70,"height":70,"fill":"#FFE8B0","glow":SODG,"x":W/2,"y":560,"opacity":f"={flick(t)}"})
nm={"text":"KIA","font":FACE,"weight":900,"size":260,"x":W/2,"y":1160,"letterSpacing":"0.02em"}
lit_text(L,nm,t)
comp(7,L)
# P8 IDA ENGBERG: a row of lamps fires down the hall onto the name
t=T[7]; L=[]; concrete(L); busbar(L,t)
for i in range(6):
    tt=t+i*0.05; x=170+i*150
    pool(L,x,300,240,SOD,tt,0.0,0.7)
    L.append({"shape":"circle","width":44,"height":44,"fill":"#FFE8B0","glow":SODG,"x":x,"y":260,"opacity":f"={flick(tt)}"})
pool(L,W/2,1050,900,SOD,t+0.2,0.0,0.45)
nm={"text":"IDA\nENGBERG","font":FACE,"weight":900,"fit":True,"width":960,"height":1100,"size":900,"lineHeight":0.82,"align":"center","x":W/2+20,"y":1100}
lit_text(L,nm,t+0.1)
comp(8,L)
# ---------- master ----------
L=[{"shape":"rect","width":W,"height":H,"fill":"#05080A","x":W/2,"y":H/2}]
CAMY="960+("+cam+")"
for k in range(9):
    L.append({"composition":f"p{k}","width":W,"height":H,"x":W/2,"y":f"=960+{k*1920+960}-({CAMY})","end":T_PULL})
# ---------- the payoff: the full line-up poster, lit by the station ----------
P=T_PULL
L.append({"src":"concrete.png","width":W,"height":H,"x":W/2,"y":H/2,"start":P})
L.append({"shape":"rect","width":W,"height":H,"fill":"#000","opacity":0.45,"x":W/2,"y":H/2,"start":P})
L.append({"shape":"circle","width":1800,"height":1800,"fill":"radial-gradient(#FFB34766, #FFB34700)","x":W/2,"y":700,"blend":"add","blur":200,"start":P})
L.append({"shape":"rect","width":W,"height":H,"fill":"#CFE2FF","x":W/2,"y":H/2,"blend":"add","start":P,"opacity":"=0.9*exp(-t*9)"})
# SUBSTATION filled with live plasma
L.append({"shader":PLASMA,"alpha":True,"start":P,"channels":[{"layers":[{"text":"SUBSTATION","font":FACE,"weight":900,"fit":True,"width":1000,"height":280,"size":400,"x":W/2,"y":230,"color":"#fff"}]}]})
L.append({"text":"12—13 JUNE · THE OLD POWER STATION","font":FACE,"weight":800,"size":50,"color":"#E9E3D2","letterSpacing":"0.03em","x":W/2,"y":420,"start":P+0.1,"animate":[{"from":{"opacity":0},"duration":0.15}]})
L.append({"shape":"rect","width":980,"height":4,"fill":SOD,"x":W/2,"y":475,"start":P+0.05,"animate":[{"from":{"scaleX":0},"duration":0.3,"ease":"outExpo"}]})
LINES=[("ANCIENT METHODS",205),("PAULA TEMPLE",215),("DVS1 · HÉCTOR OAKS",165),("RROSE · PHASE FATALE",150),("KIA · IDA ENGBERG",150)]
y=560
for i,(tx,hh) in enumerate(LINES):
    t0=P+0.12+i*2*b/4
    col="#FFF3DE" if i<2 else "#E9E3D2"
    tx=tx.replace(" ","\u00a0")
    L.append({"text":tx,"font":FACE,"weight":900,"fit":True,"width":990,"height":hh,"size":400,"color":col,"x":W/2,"y":y+hh/2,"lineHeight":0.9,"start":round(t0,4),
              "animate":[{"from":{"x":W/2+(600 if i%2==0 else -600)},"duration":0.22,"ease":"outExpo"}]})
    y+=hh+28
L.append({"shape":"rect","width":980,"height":150,"fill":"#F2C21B","x":W/2,"y":1760,"start":T_LOCK,"animate":[{"from":{"scaleY":0},"duration":0.2,"ease":"outExpo"}]})
L.append({"text":"TICKETS FRIDAY","font":FACE,"weight":900,"size":130,"color":"#111","x":W/2,"y":1762,"start":T_LOCK+0.1,"animate":[{"from":{"opacity":0,"y":"+30"},"duration":0.2,"ease":"outExpo"}]})
score=json.load(open('score.json'))
spec={"size":[W,H],"fps":30,"background":"#05080A","transition":"cut","intent":{"pace":"fast"},"motionBlur":{"shutter":180},"compositions":comps,
 "music":dict(score,followCuts=False),"post":{"grain":0.035,"halation":0.4,"bloom":0.35,"vignette":0.3},
 "scenes":[{"duration":LEN,"layers":L,"camera":{"shake":{"amount":26,"at":T_PULL,"duration":0.45},"animate":[{"from":{"zoom":1.12},"at":T_PULL,"duration":0.5,"ease":"outExpo"},{"to":{"zoom":1.04},"at":T_PULL+0.5,"duration":LEN-T_PULL-0.5}]}}]}
json.dump(spec,open('event.json','w'),indent=1)
