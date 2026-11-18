import json, math, re
BPM=120; B=60/BPM; S16=B/4
COB="#1A2BD6"; VER="#FF4A1A"; CHALK="#F1EFE9"; INK="#0E0E10"; LIME="#D8FF3C"
SCALE=["D4","E4","F4","G4","A4","B4","C5","D5"]          # 8 rows = one octave
FONTS={"Archivo Expanded":"../fonts/Archivo_wdth_wght_125_900.ttf","Familjen Grotesk":"../fonts/Familjen_Grotesk_wght_700.ttf",
       "Space Mono":"../fonts/Space_Mono_wght_700.ttf"}
PAT=[0,2,4,1,5,3,7,4,2,6,3,5]                             # 12-step ostinato against a 16-step bar: it phases
M=135; SPEED=M/S16; PH_X=1920-M*2.5
def ry(row, band=0): return (7-row)*M+M/2 + band*1080
L=[]; notes1=[]; notes2=[]; notes3=[]
T_FLY=4.0; T_DATES=6.25; T_LOCK=8.25; LEN=11.0
# ================= 0–4 s: the score (cobalt), phasing; pull back to the canon =================
L.append({"shape":"rect","width":1920*3,"height":1080*3,"fill":COB,"x":960,"y":540,"end":T_DATES})
L.append({"text":"OCTAVA","font":"Archivo Expanded","size":900,"color":"#00000000","stroke":{"color":"#3C4CF0","width":4},"x":"=900-t*60","y":560,"width":5200,"letterSpacing":"-0.03em","end":T_FLY})
for r in range(9):
    for band in (-1,0,1):
        L.append({"shape":"rect","width":1920*3,"height":2,"fill":"#FFFFFF1F","x":960,"y":r*M+band*1080,"end":T_FLY})
def xat(t_on, t): return PH_X-(t-t_on)*SPEED
def note(t_on,row,color,inset,band=0,end=T_FLY):
    return {"shape":"rect","width":M-inset,"height":M-inset,"fill":color,"x":f"={PH_X}-t*{SPEED}","y":ry(row)+band*1080,
            "start":round(t_on,4),"end":end,"animate":[{"from":{"scale":1.5},"duration":0.14,"ease":"outExpo"}]}
for k in range(32):
    t=k*S16; r=PAT[k%12]; notes1.append((t,r)); L.append(note(t,r,CHALK,12))
for k in range(16):
    t=2.0+k*S16; r=PAT[(k+1)%12]; notes2.append((t,r)); L.append(note(t,r,VER,52))
    r3=PAT[(k+2)%12]; notes3.append((t,r3)); L.append(note(t,r3,LIME,12,band=-1)); L.append(note(t,PAT[(k+3)%12],INK,12,band=1))
L.append({"shape":"rect","width":6,"height":1080*3,"fill":LIME,"x":PH_X+M/2+8,"y":540,"end":T_FLY})
for r,n in enumerate(SCALE):
    L.append({"text":n[0],"font":"Space Mono","size":30,"color":"#FFFFFFAA","x":40,"y":ry(r),"end":T_FLY})
L.append({"text":"OCTAVA — Festival for New Music — Basel 2027","font":"Familjen Grotesk","size":40,"color":CHALK,"x":1920-70,"y":540,"rotation":90,"end":T_FLY})
# ================= custom modular lettering =================
MODS={"S":"M0 0 L1 0 L1 1 L0 1 Z",
      "a":"M1 0 L1 1 L0 1 C0 0.45 0.45 0 1 0 Z", "b":"M0 0 L0 1 L1 1 C1 0.45 0.55 0 0 0 Z",
      "c":"M0 1 L0 0 L1 0 C1 0.55 0.55 1 0 1 Z", "d":"M1 1 L1 0 L0 0 C0 0.55 0.45 1 1 1 Z",
      "u":"M0 0 L1 0 L1 1 Z", "l":"M0 0 L0 1 L1 1 Z", "v":"M0 0 L1 0 L0 1 Z", "w":"M1 0 L1 1 L0 1 Z"}
GLY={"O":["aSSSb","S...S","S...S","S...S","S...S","S...S","dSSSc"],
     "C":["aSSSS","S....","S....","S....","S....","S....","dSSSS"],
     "T":["SSSSS","..S..","..S..","..S..","..S..","..S..","..S.."],
     "A":["aSSSb","S...S","S...S","SSSSS","S...S","S...S","S...S"],
     "V":["S...S","S...S","S...S","S...S","ul.wv",".uSv.","..S.."]}
def mpath(kind, s):
    toks=re.findall(r"[MLCZ]|[-0-9.]+", MODS[kind]); out=[]; nums=[]
    for t in toks:
        if t in "MLCZ": out.append(t)
        else:
            nums.append(float(t))
            if len(nums)==2: out.append("%.2f %.2f"%(nums[0]*s, nums[1]*s)); nums=[]
    return " ".join(out)
WM=50; GAP=WM; word="OCTAVA"; NC=len(word)*6-1
WX=(1920-NC*WM)/2; WY=170                                       # top-left of the wordmark grid (ink lockup position)
mods=[]
for li,ch in enumerate(word):
    for r,row in enumerate(GLY[ch]):
        for c,k in enumerate(row):
            if k!=".": mods.append((li*6+c, r, k))
# ================= 4–6.25 s: the transition IS the idea: notes fly from the stave into the letterforms =================
# where each visible note is at the moment the score freezes
src=[(xat(t,T_FLY), ry(r)) for (t,r) in notes1+notes2 if -M < xat(t,T_FLY) < 1920+M and t>=2.0-1e-6]
src+= [(xat(t,T_FLY), ry(r)) for (t,r) in notes1 if -M < xat(t,T_FLY) < 1920+M and t<2.0]
cols=sorted(set(m[0] for m in mods)); chords=[]
order={c:j for j,c in enumerate(cols)}
# the flight lands two module-columns per 16th, from T_FLY+0.25
for q,(cidx,r,k) in enumerate(sorted(mods, key=lambda m:(m[0]*7+m[1]*13)%len(mods))):
    j=order[cidx]; land=T_FLY+0.25+(j//2)*S16
    sx,sy=src[q%len(src)]
    dup = q>=len(src)
    tx=WX+cidx*WM; ty=WY+r*WM                     # final top-left of the module
    L.append({"shape":"path","d":mpath(k,WM-4),"fill":CHALK,"x":tx,"y":ty,"anchor":"topLeft","end":T_DATES,
              "start":round(land-0.30,3),
              "animate":[{"from":{"x":sx-(M-12)/2,"y":sy-(M-12)/2,"scale":(M-12)/(WM-4)},"duration":0.30,"ease":"inOutExpo"}]
                        +([{"from":{"opacity":0},"duration":0.12}] if dup else [])})
    if not dup:
        L.append({"shape":"rect","width":M-12,"height":M-12,"fill":CHALK,"x":sx,"y":sy,"start":T_FLY,"end":round(land-0.30,3)})
for j,cidx in enumerate(cols):
    if j%2==0:
        rows=sorted(set(r for (c,r,k) in mods if order[c]//2==j//2))
        chords.append((T_FLY+0.25+(j//2)*S16, [6-r for r in rows][:4]))
# a hairline stave remains under the word while it forms
L.append({"shape":"rect","width":NC*WM,"height":2,"fill":"#FFFFFF55","x":960,"y":WY+7*WM+30,"start":T_FLY+0.3,"end":T_DATES,
          "animate":[{"from":{"scaleX":0},"duration":0.6,"ease":"outExpo"}]})
L.append({"text":"Festival for New Music","font":"Space Mono","size":30,"color":CHALK,"letterSpacing":"0.06em","x":WX,"y":WY+7*WM+70,"anchor":"topLeft","start":5.4,"end":T_DATES,
          "animate":[{"from":{"opacity":0,"x":"-30"},"duration":0.3,"ease":"outExpo"}]})
# ================= 6.25–8.25 s: dates, vermilion, outline stacks (one word per beat) =================
L.append({"shape":"rect","width":1920,"height":1080,"fill":VER,"x":960,"y":540,"start":T_DATES,"end":T_LOCK})
words=[("14—29",400,T_DATES),("MAY",640,T_DATES+0.5),("2027",560,T_DATES+1.0),("BASEL",440,T_DATES+1.5)]
for i,(w,sz,t0) in enumerate(words):
    step=sz*0.8
    for e in (-3,-2,-1,1,2,3):
        L.append({"text":w,"font":"Archivo Expanded","size":sz,"color":"#00000000","stroke":{"color":INK,"width":3},"x":960,"y":540+e*step,
                  "start":round(t0+abs(e)*0.033,3),"end":t0+0.5,"letterSpacing":"-0.02em","width":3000,"opacity":max(0.25,1-abs(e)*0.22),
                  "animate":[{"from":{"x":960+(700 if i%2==0 else -700)*(1+abs(e)*0.15)},"duration":0.2,"ease":"outExpo"},{"to":{"x":960+(-90 if i%2==0 else 90)*(1+abs(e)*0.4)},"at":0.2,"duration":0.3,"ease":"linear"}]})
    L.append({"text":w,"font":"Archivo Expanded","size":sz,"color":INK,"x":960,"y":540,"start":t0,"end":t0+0.5,"letterSpacing":"-0.02em","width":3000,
              "animate":[{"from":{"x":960+(700 if i%2==0 else -700)},"duration":0.2,"ease":"outExpo"},{"to":{"x":960+(-90 if i%2==0 else 90)},"at":0.2,"duration":0.3,"ease":"linear"}]})
for k in range(16):
    t=T_DATES+k*S16; r=PAT[(k+3)%12]; notes1.append((t,r))
    L.append({"shape":"rect","width":56,"height":56,"fill":CHALK if k%4==0 else INK,"x":60+k*120,"y":1020,"start":round(t,4),"end":T_LOCK})
# ================= 8.25–11 s: the night lockup (ink): the wordmark returns, chalk, with a vermilion stave =================
L.append({"shape":"rect","width":1920,"height":1080,"fill":INK,"x":960,"y":540,"start":T_LOCK})
for q,(cidx,r,k) in enumerate(sorted(mods)):
    tx=WX+cidx*WM; ty=WY+r*WM
    L.append({"shape":"path","d":mpath(k,WM-4),"fill":CHALK,"x":tx,"y":ty,"anchor":"topLeft","start":T_LOCK,
              "animate":[{"from":{"opacity":0},"at":round((cidx%6)*0.012+r*0.01,3),"duration":0.08}]})
GL=WX; BY=WY+7*WM+110
for r in range(8):   # the octave stave under the date line, vermilion hairlines
    L.append({"shape":"rect","width":NC*WM,"height":1.5,"fill":"#FF4A1A88","x":960,"y":BY-40+r*7,"start":T_LOCK+0.1,
              "animate":[{"from":{"scaleX":0},"duration":0.5,"at":r*0.02,"ease":"outExpo"}]})
lock=[("14—29.05","Archivo Expanded",150,CHALK,GL,BY,0.25),("2027","Archivo Expanded",150,VER,GL+21*WM,BY,0.37),
      ("FESTIVAL FOR NEW MUSIC","Space Mono",30,CHALK,GL,BY+190,0.55),("BASEL · 8 STAGES · 41 PREMIERES","Space Mono",30,CHALK,GL+21*WM,BY+190,0.65)]
for (tx,f,sz,c,x,y,dt) in lock:
    L.append({"text":tx,"font":f,"size":sz,"color":c,"x":x,"y":y,"anchor":"topLeft","letterSpacing":"-0.02em" if sz>100 else "0.06em",
              "start":round(T_LOCK+dt,3),"animate":[{"from":{"x":"-60","opacity":0},"duration":0.3,"ease":"outExpo"}]})
T_BTN=10.0
L.append({"shape":"rect","width":WM*2-4,"height":WM*2-4,"fill":VER,"x":WX+(NC-1)*WM,"y":BY+78,"start":T_BTN,
          "animate":[{"from":{"y":"-700"},"duration":0.2,"ease":"inQuad"}]})
L.append({"shader":'float h(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}\nfloat n(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.0-2.0*f);return mix(mix(h(i),h(i+vec2(1,0)),f.x),mix(h(i+vec2(0,1)),h(i+vec2(1,1)),f.x),f.y);}\nvoid mainImage(out vec4 o, in vec2 f){\n  vec2 R=iResolution.xy; vec2 jit=vec2(n(f*0.08+iTime*0.0), n(f*0.08+7.3))-0.5;\n  vec2 uv=(f+jit*1.6)/R;\n  vec3 c=texture(iChannel0,uv).rgb;\n  float lum=dot(c,vec3(0.299,0.587,0.114));\n  float ink=1.0-lum;                                   // how much ink is on this spot\n  float speck=n(f*0.9)*0.6+n(f*0.23)*0.4;              // uneven ink density\n  vec3 inked=mix(c, c+ (1.0-c)*0.18, smoothstep(0.35,0.95,speck)*ink);\n  float fibre=n(vec2(f.x*0.05,f.y*0.9))*0.5+n(f*0.6)*0.5;\n  vec3 paper=inked*(0.94+0.06*fibre);\n  float grain=(h(f)-0.5)*0.03;\n  o=vec4(paper+grain,1.0);\n}',"filter":True})
# ================= score: generated from the same note lists =================
def notes_string(events, length):
    steps=int(round(length/S16)); grid=[[] for _ in range(steps)]
    for t,rows in events:
        k=int(round(t/S16))
        if 0<=k<steps: grid[k]+=rows
    out=[]
    for g in grid:
        if not g: out.append("r/16")
        elif len(set(g))==1: out.append(f"{SCALE[g[0]]}/16")
        else: out.append("["+" ".join(SCALE[x] for x in sorted(set(g)))+"]/16")
    return " ".join(out)
music={"bpm":BPM,"key":"D dorian","length":f"{LEN}s","seed":5,"end":"ring",
 "tracks":[
  {"name":"marimba","inst":"phys:marimba","clips":[{"at":"@0","notes":notes_string([(t,[r]) for t,r in notes1]+chords,T_BTN)}],"gain":-2,"pan":-0.2},
  {"name":"vibes","inst":"vibraphone","clips":[{"at":"@0","notes":notes_string([(t,[r]) for t,r in notes2],4.0)}],"gain":-8,"pan":0.35},
  {"name":"glock","inst":"glockenspiel","clips":[{"at":"@0","transpose":12,"notes":notes_string([(t,[r]) for t,r in notes3],4.0)}],"gain":-16,"pan":-0.4},
  {"name":"pizz","inst":"pizzicato","clips":[{"at":"@2","notes":"o2 l4 D2 D2 A1 D2 D2 D2 C2 A1"},{"at":f"@{T_DATES}","notes":"o2 l4 D2 D2 A1 A1"},{"at":f"@{T_LOCK}","notes":"o2 l4 D2 r A1 r G1 r A1/2 D2/1"}],"gain":-4},
  {"name":"hits","inst":"drums:orchestra","clips":[{"at":f"@{T_DATES}","drums":{"kick":"X...X...X...X...","crash":"X..............."}},
                                                  {"at":f"@{T_BTN}","drums":{"kick":"X...............","crash":"X..............."}}],"gain":-6},
  {"name":"claves","inst":"woodblock","clips":[{"at":"@0","notes":"o5 l8 (C5 r r C5 r C5 r r)x4"}],"gain":-14}
 ]}
spec={"size":[1920,1080],"fps":30,"background":CHALK,"transition":"cut","intent":{"pace":"steady"},"fonts":FONTS,
 "music":dict(music, followCuts=False),"post":{"grain":0.012},
 "scenes":[{"duration":LEN,"layers":L,
   "camera":{"animate":[{"from":{"zoom":1.0},"to":{"zoom":0.42},"at":2.75,"duration":1.2,"ease":"inOutExpo"},
                        {"to":{"zoom":1.0},"at":3.999,"duration":0.001},
                        {"to":{"zoom":0.94},"at":T_LOCK-0.001,"duration":0.001},{"to":{"zoom":1.0},"at":T_LOCK,"duration":LEN-T_LOCK,"ease":"outCubic"}]}}]}
json.dump(spec,open('octava.json','w'),indent=1)
print(len(L),"layers; modules",len(mods),"sources",len(src))
