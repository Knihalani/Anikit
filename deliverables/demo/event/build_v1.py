import json, math
BPM=132; b=60/BPM; BAR=4*b
W,H=1080,1920
FACE="Big Shoulders"; MONO="JetBrains Mono"
NAMES=["ANCIENT\nMETHODS","PAULA\nTEMPLE","DVS1","HÉCTOR\nOAKS","RROSE","PHASE\nFATALE","KIA","IDA\nENGBERG"]
T=[BAR+0*3*b, BAR+1*3*b, BAR+2*3*b, BAR+3*3*b]            # bars 2–4: one circuit every 3 beats
T+= [T[-1]+3*b+k*2*b for k in range(4)]                     # then every 2 beats: the cascade
T_PULL=6*BAR; PULL=1.1; T_LOCK=7*BAR; LEN=15.0
ENAMEL="#16201C"; DIM="#2A3530"; LIT="#FFF0D2"; GLOW="#FF8A2A"; COPPER="#C9803F"
WHIP=0.30
def E(x): return f"smoothstep(0,1,smoothstep(0,1,clamp({x},0,1)))"
# camera: tower coordinate at screen centre, and scale
cam="+".join(f"1920*{E(f'(t-{tk-WHIP:.4f})/{WHIP}')}" for tk in T)
p=E(f"(t-{T_PULL:.4f})/{PULL}")
C_END=8*1920+960; C_FIN=(9*1920)/2-(1000-960)/0.075
C=f"mix(960+({cam}),{C_FIN:.1f},{p})"
S=f"pow(0.075,{p})"
comps={}
def panel(k, name, t_on):
    L=[]
    L.append({"shape":"rect","width":W,"height":H,"fill":f"linear-gradient(180deg, #1B2622, {ENAMEL} 60%, #111815)","x":W/2,"y":H/2})
    L.append({"shape":"rect","width":W-60,"height":H-60,"fill":"none","stroke":"#2E3B35","strokeWidth":5,"radius":22,"x":W/2,"y":H/2})
    for (rx,ry_) in [(70,70),(W-70,70),(70,H-70),(W-70,H-70)]:
        L.append({"shape":"circle","width":18,"height":18,"fill":"radial-gradient(#7C8A84, #2A3430)","x":rx,"y":ry_})
    # busbar (continuous across panels) + current bead
    L.append({"shape":"rect","width":30,"height":H,"fill":f"linear-gradient(90deg, #6E3E1C, {COPPER} 45%, #F0B077 55%, #7A4420)","x":120,"y":H/2})
    if t_on is not None:
        L.append({"shape":"rect","width":30,"height":H,"fill":"#FFD49A","x":120,"y":H/2,"blend":"add","opacity":f"=t<{t_on:.4f}?0:0.35+0.15*sin(t*40)","glow":"#FF8A2A"})
        L.append({"shape":"circle","width":46,"height":46,"fill":"#FFFFFF","glow":"#FFB060","x":120,
                  "y":f"=-60+({H*0.78+60})*clamp((t-{t_on-WHIP:.4f})/{WHIP},0,1)","opacity":f"=(t>{t_on-WHIP:.4f} && t<{t_on+0.05:.4f})?1:0",
                  "echo":{"count":8,"delay":0.012,"decay":0.7,"blend":"add"}})
    # plate
    L.append({"shape":"rect","width":360,"height":64,"fill":"linear-gradient(180deg, #B89A5A, #8A6E36)","radius":6,"x":W/2,"y":150})
    L.append({"text":f"CIRCUIT {k:02d}" if k else "INCOMER","font":MONO,"weight":700,"size":34,"color":"#1C1608","letterSpacing":"0.12em","x":W/2,"y":150})
    L.append({"text":"11 kV","font":MONO,"weight":700,"size":30,"color":"#7F928A","x":W-110,"y":150,"anchor":"right"})
    # the name: unlit engraving, then lit filament
    nm={"text":name,"font":FACE,"weight":900,"fit":True,"width":860,"height":820 if "\n" in name else 560,"size":900,"lineHeight":0.86,"align":"center","x":W/2+40,"y":820,"letterSpacing":"0.01em"}
    L.append(dict(nm,color=DIM))
    if t_on is not None:
        fl=f"(t<{t_on:.4f}?0:(t<{t_on+0.034:.4f}?1:(t<{t_on+0.08:.4f}?0.2:(t<{t_on+0.12:.4f}?1:(t<{t_on+0.15:.4f}?0.5:1)))))"
        L.append(dict(nm,color=GLOW,blur=36,blend="add",opacity="="+fl+"*0.9"))
        L.append(dict(nm,color=LIT,glow=GLOW,opacity="="+fl))
        # arc flash
        L.append({"shape":"rect","width":W,"height":H,"fill":"#CFE2FF","x":W/2,"y":H/2,"blend":"add","opacity":f"=t<{t_on:.4f}?0:0.55*exp(-(t-{t_on:.4f})*18)"})
    # breaker lever
    bx,by=300,1520
    L.append({"shape":"circle","width":200,"height":200,"fill":"radial-gradient(#3B4843, #121916)","stroke":"#4C5A54","strokeWidth":4,"x":bx,"y":by})
    rot=f"=t<{t_on:.4f}?-48:-48+96*(1-exp(-(t-{t_on:.4f})*28)*cos((t-{t_on:.4f})*30))" if t_on is not None else -48
    L.append({"shape":"rect","width":36,"height":150,"radius":12,"fill":"linear-gradient(180deg, #E8E2D4, #9A958A)","x":bx,"y":by,"anchor":[0.5,0.9],"rotation":rot})
    L.append({"text":"OFF","font":MONO,"size":26,"weight":700,"color":"#6C7C74","x":bx-120,"y":by-130})
    L.append({"text":"ON","font":MONO,"size":26,"weight":700,"color":"#FF8A2A" if t_on is not None else "#6C7C74","x":bx+120,"y":by-130,
              "opacity":f"=t<{t_on:.4f}?0.4:1" if t_on is not None else 0.4})
    # meter
    mx,my,mr=760,1520,130
    L.append({"shape":"circle","width":2*mr,"height":2*mr,"fill":"#E9E3D2","stroke":"#2C3632","strokeWidth":10,"x":mx,"y":my})
    for i in range(11):
        a=math.radians(-60+i*12); L.append({"shape":"rect","width":4,"height":26 if i%5==0 else 14,"fill":"#1A1A1A","x":mx+math.sin(a)*(mr-30),"y":my-math.cos(a)*(mr-30),"rotation":-60+i*12})
    L.append({"text":"kA","font":MONO,"size":24,"weight":700,"color":"#1A1A1A","x":mx,"y":my+50})
    nd=f"=t<{t_on:.4f}?-58:42-100*exp(-(t-{t_on:.4f})*7)*cos((t-{t_on:.4f})*22)+2.5*sin(t*31)" if t_on is not None else -58
    L.append({"shape":"rect","width":6,"height":mr-24,"fill":"#D2381E","x":mx,"y":my,"anchor":[0.5,1.0],"rotation":nd})
    L.append({"shape":"circle","width":22,"height":22,"fill":"#1A1A1A","x":mx,"y":my})
    comps[f"p{k}"]={"size":[W,H],"duration":LEN,"background":ENAMEL,"layers":L}
# panel 0: the incomer = festival title
def title_panel():
    L=[]
    L.append({"shape":"rect","width":W,"height":H,"fill":f"linear-gradient(180deg, #1B2622, {ENAMEL} 60%, #111815)","x":W/2,"y":H/2})
    L.append({"shape":"rect","width":W-60,"height":H-60,"fill":"none","stroke":"#2E3B35","strokeWidth":5,"radius":22,"x":W/2,"y":H/2})
    L.append({"shape":"rect","width":30,"height":H*0.55,"fill":f"linear-gradient(90deg, #6E3E1C, {COPPER} 45%, #F0B077 55%, #7A4420)","x":120,"y":H*0.725})
    nm={"text":"SUB\nSTAT\nION","font":FACE,"weight":900,"fit":True,"width":900,"height":1250,"size":900,"lineHeight":0.82,"align":"center","x":W/2+30,"y":860}
    fl="(t<0.35?0:(t<0.39?1:(t<0.5?0.1:(t<0.55?1:(t<0.62?0.3:(t<0.7?1:(t<0.74?0.4:1)))))))"
    L.append(dict(nm,color=DIM))
    L.append(dict(nm,color="#FF3B1A",blur=40,blend="add",opacity="="+fl+"*0.9"))
    L.append(dict(nm,color="#FFE3D6",glow="#FF3B1A",opacity="="+fl))
    for i,(tx,y) in enumerate([("12—13 JUNE",1620),("TWO NIGHTS · THE OLD POWER STATION",1690)]):
        L.append({"text":tx,"font":MONO,"weight":700,"size":44 if i==0 else 30,"color":"#E9E3D2","letterSpacing":"0.08em","x":W/2,"y":y,"opacity":f"=smoothstep({0.9+i*0.15},{1.1+i*0.15},t)"})
    comps["p0"]={"size":[W,H],"duration":LEN,"background":ENAMEL,"layers":L}
title_panel()
for k,(n,t) in enumerate(zip(NAMES,T)): panel(k+1,n,t)
L=[{"shape":"rect","width":W,"height":H,"fill":"#05080A","x":W/2,"y":H/2}]
for k in range(9):
    L.append({"composition":f"p{k}","width":W,"height":H,"x":W/2,"y":f"=960+({S})*({k*1920+960}-({C}))","scale":f"={S}"})
# power-up light: warm haze bloom behind the tower when it's all on
L.append({"shape":"rect","width":220,"height":1500,"fill":"radial-gradient(#FF8A2A55, #FF8A2A00)","x":W/2,"y":1000,"blend":"add","blur":60,"opacity":f"=smoothstep({T_PULL+0.6},{T_PULL+1.2},t)"})
# lockup
LK=T_LOCK
L.append({"text":"SUBSTATION","font":FACE,"weight":900,"size":205,"color":"#FFE3D6","glow":"#FF3B1A","letterSpacing":"0.0em","x":W/2,"y":200,
          "start":LK,"animate":[{"from":{"scale":1.25,"opacity":0},"duration":0.18,"ease":"outExpo"}]})
L.append({"text":"12—13 JUNE · THE OLD POWER STATION","font":MONO,"weight":700,"size":34,"color":"#E9E3D2","letterSpacing":"0.06em","x":W/2,"y":345,"start":LK+0.15,
          "animate":[{"from":{"opacity":0},"duration":0.2}]})
L.append({"shape":"rect","width":720,"height":150,"fill":"#FF3B1A","radius":8,"x":W/2,"y":1770,"start":LK+0.45,"animate":[{"from":{"scaleX":0},"duration":0.25,"ease":"outExpo"}]})
L.append({"text":"TICKETS FRIDAY","font":FACE,"weight":900,"size":120,"color":"#140805","x":W/2,"y":1772,"start":LK+0.6,"animate":[{"from":{"opacity":0,"y":"+30"},"duration":0.2,"ease":"outExpo"}]})
camera={"shake":{"amount":22,"at":T_PULL,"duration":0.5}}
score=json.load(open('score.json'))
spec={"size":[W,H],"fps":30,"background":"#05080A","transition":"cut","intent":{"pace":"fast"},
 "motionBlur":{"shutter":180},"compositions":comps,"music":dict(score,followCuts=False),
 "post":{"grain":0.03,"halation":0.35,"bloom":0.3,"vignette":0.3},
 "scenes":[{"duration":LEN,"layers":L,"camera":camera}]}
json.dump(spec,open('event.json','w'),indent=1)
print([round(x,2) for x in T], round(T_PULL,2), round(T_LOCK,2))
