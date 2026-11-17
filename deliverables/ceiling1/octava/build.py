import json, math
BPM=120; B=60/BPM; S16=B/4
COB="#1A2BD6"; VER="#FF4A1A"; CHALK="#F1EFE9"; INK="#0E0E10"; LIME="#D8FF3C"
SCALE=["D4","E4","F4","G4","A4","B4","C5","D5"]          # 8 rows = one octave
FONTS={"Archivo Expanded":"../fonts/Archivo_wdth_wght_125_900.ttf","Familjen Grotesk":"../fonts/Familjen_Grotesk_wght_700.ttf",
       "Space Mono":"../fonts/Space_Mono_wght_700.ttf"}
PAT=[0,2,4,1,5,3,7,4,2,6,3,5]                             # 12-step ostinato against a 16-step bar (it phases)
M=135                                                     # 8 rows × 135 = 1080: the octave fills the frame
SPEED=M/S16                                               # one column per 16th
PH_X=1920-M*2.5                                           # playhead column
def ry(row, band=0, bands_h=1080): return (7-row)*M+M/2 + band*bands_h
L=[]; notes1=[]; notes2=[]; notes3=[]
# ---------------- bars 1–2: the score, full bleed, three canon voices stacked (the camera reveals them) ----------------
L.append({"shape":"rect","width":1920*3,"height":1080*3,"fill":COB,"x":960,"y":540,"end":4.0})
for r in range(9):
    for band in (-1,0,1):
        L.append({"shape":"rect","width":1920*3,"height":2,"fill":"#FFFFFF1F","x":960,"y":r*M+band*1080,"end":4.0})
def note(t_on,row,color,inset,band=0):
    return {"shape":"rect","width":M-inset,"height":M-inset,"fill":color,"x":f"={PH_X}-t*{SPEED}","y":ry(row)+band*1080,
            "start":round(t_on,4),"end":4.0,"animate":[{"from":{"scale":1.5},"duration":0.14,"ease":"outExpo"}]}
for k in range(32):
    t=k*S16; r=PAT[k%12]; notes1.append((t,r)); L.append(note(t,r,CHALK,12))
for k in range(16):
    t=2.0+k*S16; r=PAT[(k+1)%12]; notes2.append((t,r)); L.append(note(t,r,VER,52))
    r3=PAT[(k+2)%12]; notes3.append((t,r3)); L.append(note(t,r3,LIME,12,band=-1)); L.append(note(t,PAT[(k+3)%12],INK,12,band=1))
L.append({"shape":"rect","width":6,"height":1080*3,"fill":LIME,"x":PH_X+M/2+8,"y":540,"end":4.0})
for r,n in enumerate(SCALE):
    L.append({"text":n[0],"font":"Space Mono","size":30,"color":"#FFFFFFAA","x":40,"y":ry(r),"end":4.0})
L.append({"text":"OCTAVA — Festival for New Music — Basel 2027","font":"Familjen Grotesk","size":40,"color":CHALK,"x":1920-70,"y":540,"rotation":90,"end":4.0})
# ---------------- bar 3: dates, full bleed, alternate directions, on the beat ----------------
L.append({"shape":"rect","width":1920,"height":1080,"fill":VER,"x":960,"y":540,"start":4.0,"end":6.0})
words=[("14—29",400,4.0),("MAY",640,4.5),("2027",560,5.0),("BASEL",440,5.5)]
for i,(w,sz,t0) in enumerate(words):
    step=sz*0.8
    # the stack: echoes of the word in outline, above and below, arriving a frame apart
    for e in (-3,-2,-1,1,2,3):
        L.append({"text":w,"font":"Archivo Expanded","size":sz,"color":"#00000000","stroke":{"color":INK,"width":3},"x":960,"y":540+e*step,
                  "start":round(t0+abs(e)*0.033,3),"end":t0+0.5,"letterSpacing":"-0.02em","width":3000,"opacity":max(0.25,1-abs(e)*0.22),
                  "animate":[{"from":{"x":960+(700 if i%2==0 else -700)*(1+abs(e)*0.15)},"duration":0.2,"ease":"outExpo"},{"to":{"x":960+(-90 if i%2==0 else 90)*(1+abs(e)*0.4)},"at":0.2,"duration":0.3,"ease":"linear"}]})
    L.append({"text":w,"font":"Archivo Expanded","size":sz,"color":INK,"x":960,"y":540,"start":t0,"end":t0+0.5,"letterSpacing":"-0.02em","width":3000,
              "animate":[{"from":{"x":960+(700 if i%2==0 else -700)},"duration":0.2,"ease":"outExpo"},{"to":{"x":960+(-90 if i%2==0 else 90)},"at":0.2,"duration":0.3,"ease":"linear"}]})
for k in range(16):
    t=4.0+k*S16; r=PAT[(k+3)%12]; notes1.append((t,r))
    L.append({"shape":"rect","width":56,"height":56,"fill":CHALK if k%4==0 else INK,"x":60+k*120,"y":1080-60-r*0,"start":round(t,4),"end":6.0})
# ---------------- bars 4–5: the melody spells the wordmark, over a ghost of the score ----------------
GLY={"O":["01110","10001","10001","10001","10001","10001","01110"],
     "C":["01111","10000","10000","10000","10000","10000","01111"],
     "T":["11111","00100","00100","00100","00100","00100","00100"],
     "A":["01110","10001","10001","11111","10001","10001","10001"],
     "V":["10001","10001","10001","10001","01010","01010","00100"]}
WM=52; word="OCTAVA"; NC=len(word)*6-1; WX=(1920-NC*WM)/2+WM/2; WY=200
L.append({"shape":"rect","width":1920,"height":1080,"fill":CHALK,"x":960,"y":540,"start":6.0})
# ghost score band under the wordmark (continues the ostinato, quietly)
for k in range(40):
    t=6.0+k*S16; r=PAT[k%12]
    L.append({"shape":"rect","width":WM-8,"height":WM-8,"fill":"#1A2BD614","x":f"={1920-WM*2}-t*{WM/S16}","y":WY+8*WM+40+ (7-r)*WM*0.5,"start":round(t,4)})
mods=[]
for li,ch in enumerate(word):
    for r,row in enumerate(GLY[ch]):
        for c,v in enumerate(row):
            if v=="1": mods.append((li*6+c, r))
cols=sorted(set(m[0] for m in mods)); chords=[]
for j,cidx in enumerate(cols):
    t=6.0+(j//2)*S16; rows=[r for (c,r) in mods if c==cidx]; chords.append((t,[6-r for r in rows]))
    for r in rows:
        L.append({"shape":"rect","width":WM-4,"height":WM-4,"fill":COB,"x":WX+cidx*WM,"y":WY+r*WM,"start":round(t,4),
                  "animate":[{"from":{"scale":0,"fill":VER},"duration":0.16,"ease":"outBack"},{"to":{"fill":COB},"at":0.16,"duration":0.3}]})
T_END=6.0+((len(cols)+1)//2)*S16
GL=WX-WM/2                                                       # grid left edge
info=[("Festival for New Music",64,INK,0.0),("14—29 May 2027",64,INK,0.25),("Basel",64,VER,0.5)]
for i,(tx,sz,c,dt) in enumerate(info):
    L.append({"text":tx,"font":"Familjen Grotesk","size":sz,"color":c,"x":GL,"y":WY+7*WM+150+i*78,"anchor":"left","start":round(T_END-0.6+dt,3),
              "animate":[{"from":{"y":"+40","opacity":0},"duration":0.35,"ease":"outExpo"}]})
L.append({"text":"Reich\nSaariaho\nAndriessen\nLang\nAbrahamsen","font":"Familjen Grotesk","size":34,"color":"#55545A","lineHeight":1.2,"align":"left",
          "x":GL+18*WM,"y":WY+7*WM+150-24,"anchor":"topLeft","start":round(T_END-0.2,3),"animate":[{"from":{"opacity":0},"duration":0.4}]})
# button: one vermilion module drops onto the last grid cell on the final beat
L.append({"shape":"rect","width":WM*3-4,"height":WM*3-4,"fill":VER,"x":WX+(NC-2)*WM,"y":WY+7*WM+150+70,"start":9.5,
          "animate":[{"from":{"y":"-700"},"duration":0.2,"ease":"inQuad"}]})
# ---------------- score ----------------
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
LEN=10.8
music={"bpm":BPM,"key":"D dorian","length":f"{LEN}s","seed":5,"end":"ring",
 "tracks":[
  {"name":"marimba","inst":"phys:marimba","clips":[{"at":"@0","notes":notes_string([(t,[r]) for t,r in notes1]+chords,9.5)}],"gain":-2,"pan":-0.2},
  {"name":"vibes","inst":"vibraphone","clips":[{"at":"@0","notes":notes_string([(t,[r]) for t,r in notes2],4.0)}],"gain":-8,"pan":0.35},
  {"name":"glock","inst":"glockenspiel","clips":[{"at":"@0","transpose":12,"notes":notes_string([(t,[r]) for t,r in notes3],4.0)}],"gain":-16,"pan":-0.4},
  {"name":"pizz","inst":"pizzicato","clips":[{"at":"@2","notes":"o2 l4 D2 D2 A1 D2 D2 D2 C2 A1"},{"at":"@6","notes":"o2 l4 D2 r A1 r D2 r r r G1 r A1 r D2/1"}],"gain":-4},
  {"name":"hits","inst":"drums:orchestra","clips":[{"at":"@4","drums":{"kick":"X...X...X...X...","crash":"X..............."}},
                                                  {"at":"@9.5","drums":{"kick":"X...............","crash":"X..............."}}],"gain":-6},
  {"name":"claves","inst":"woodblock","clips":[{"at":"@0","notes":"o5 l8 (C5 r r C5 r C5 r r)x4"}],"gain":-14}
 ]}
spec={"size":[1920,1080],"fps":30,"background":CHALK,"transition":"cut","intent":{"pace":"steady"},"fonts":FONTS,
 "music":dict(music, followCuts=False),"post":{"grain":0.012},
 "scenes":[{"duration":LEN,"layers":L,
   "camera":{"animate":[{"from":{"zoom":1.0},"to":{"zoom":0.42},"at":2.75,"duration":1.2,"ease":"inOutExpo"},
                        {"to":{"zoom":1.0},"at":3.999,"duration":0.001},{"to":{"zoom":0.9},"at":5.999,"duration":0.001},
                        {"to":{"zoom":1.0},"at":6.0,"duration":4.8,"ease":"outCubic"}]}}]}
json.dump(spec,open('octava.json','w'),indent=1)
print(len(L), "layers; wordmark ends", round(T_END,2))
