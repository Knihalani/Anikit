import json, sys
sys.path.insert(0,'.')
from fx import *
B = 60/128          # beat
FONTS = {"Anton":"../fonts/Anton.ttf","Monoton":"../fonts/Monoton.ttf","Bebas Neue":"../fonts/Bebas_Neue.ttf",
 "UnifrakturMaguntia":"../fonts/UnifrakturMaguntia.ttf","Six Caps":"../fonts/Six_Caps.ttf","Rubik Mono One":"../fonts/Rubik_Mono_One.ttf",
 "Bodoni Moda Black":"../fonts/Bodoni_Moda_ital_wght_1_900.ttf","Instrument Sans":"../fonts/Instrument_Sans_wght_700.ttf",
 "Bungee Shade":"../fonts/Bungee_Shade.ttf","Archivo Black":"../fonts/Archivo_Black.ttf","Big Shoulders Display Black":"../fonts/Big_Shoulders_Display_wght_900.ttf",
 "Major Mono Display":"../fonts/Major_Mono_Display.ttf"}
def v3(h):
    h=h.lstrip('#'); return "vec3(%.3f,%.3f,%.3f)"%tuple(int(h[i:i+2],16)/255 for i in (0,2,4))
def sub(body, subs):
    for k,v in sorted(subs.items(), key=lambda kv:-len(kv[0])): body=body.replace(k,str(v))
    return body
def chan(text, font, w=1760, h=820, size=None, lh=0.88, ls="0em", y="50%", x="50%", extra=None):
    t={"text":text,"font":font,"color":"#fff","align":"center","lineHeight":lh,"letterSpacing":ls,"x":x,"y":y}
    if size: t["size"]=size; t["width"]=w
    else: t.update({"fit":True,"width":w,"height":h,"size":900})
    L=[t]+(extra or [])
    return {"layers":L,"mipmaps":True}
def fxlayer(body, subs, ch, **kw):
    d={"shader":shader(sub(body,subs)),"alpha":True,"channels":[ch]}; d.update(kw); return d
def rect(c, **kw):
    d={"shape":"rect","width":1920,"height":1080,"fill":c}; d.update(kw); return d
def label(t, y, color="#fff", size=40, font="Instrument Sans", ls="0.18em", **kw):
    d={"text":t,"font":font,"size":size,"color":color,"letterSpacing":ls,"y":y,"uppercase":True}; d.update(kw); return d
SNAP=[{"from":{"scale":1.18},"duration":0.22,"ease":"outExpo"}]
scenes=[]
def card(dur, layers, cam=None, post=None):
    s={"duration":round(dur,5),"layers":layers}
    if cam: s["camera"]=cam
    if post: s["post"]=post
    scenes.append(s)

# 1 · A HALCYON PICTURE — neon tube flickers on (2 beats)
flick = "=t<0.06?0:(t<0.10?1:(t<0.16?0.15:(t<0.2?1:(t<0.24?0.3:1))))"
NE={"text":"A HALCYON\nPICTURE","font":"Monoton","size":250,"lineHeight":1.02,"align":"center","letterSpacing":"0.03em","y":"44%"}
card(2*B, [rect("radial-gradient(#140A10, #020102)"),
  dict(NE, color="#FF2D7A", blur=28, opacity=flick, blend="add"),
  dict(NE, color="#FF2D7A", blur=8, opacity=flick, blend="add"),
  dict(NE, color="#FFD2E4", glow="#FF2D7A", opacity=flick),
  dict(NE, color="#FF2D7A", y="112%", scaleY=-0.55, blur=10, opacity="=0.28*("+flick[1:]+")")],
  cam={"animate":[{"from":{"zoom":1.0},"to":{"zoom":1.07},"duration":2*B,"ease":"linear"}]})
# 2 · MIRA OYELARAN — moiré
MO_BG = """ float m=0.0; float r2=length(f-R*vec2(0.22,0.8)); float b=smoothstep(0.4,0.6,0.5+0.5*sin(r2*0.5+iTime*6.0)); o=vec4(mix(vec3(0.03,0.0,0.03),vec3(0.93,0.07,0.36),b),1.0); """
card(2*B, [ {"shader":shader(MO_BG)},
  fxlayer(MOIRE, {"TINT":v3("#00D9B5")}, chan("MIRA\nOYELARAN","Anton",h=900), animate=SNAP)])
# 3 · TOMAS VELD — bulb marquee
card(B, [rect("#000"), fxlayer(BULBS, {"TINT":v3("#FFC46B"),"CELL":"24.0","BGA":"1.0"}, chan("TOMAS VELD","Anton",w=1820,h=700))])
# 4 · JUNO ABARA — halftone black on acid yellow
card(B, [rect("#F2E600"), fxlayer(HALFTONE, {"TINT":"vec3(0.02)","CELL":"16.0"}, chan("JUNO ABARA","Anton",w=1860,h=900))], cam={"animate":[{"from":{"zoom":1.08},"to":{"zoom":1.0},"duration":B,"ease":"outCubic"}]})
# 5 · ISAK LINDQVIST — chroma split on cobalt
card(B, [rect("#1531FF"), fxlayer(CHROMA, {"TINT":"vec3(1.0)","OFF":"14.0"}, chan("ISAK\nLINDQVIST","Bebas Neue",h=980,lh=0.82))])
# 6 · RENATA SOLÉ — blackletter on bone with red
card(B, [rect("#EFE7DA"),
  {"text":"Renata Solé","font":"UnifrakturMaguntia","size":400,"width":1900,"scale":"=1+0.05*t","color":"#C8102E","stroke":{"color":"#111","width":6},"shadow":{"x":8,"y":8,"blur":0,"color":"#111"}},
  label("with", "24%", color="#111", size=40)])
# 7 · CASTING · KENJI ARAMAKI — extruded block
card(B, [rect("#0B0B0B"), fxlayer(EXTRUDE, {"TINT":v3("#F5EBDD"),"TINT2":v3("#E0261B"),"DIR":"vec2(-1.0,1.0)"}, chan("KENJI\nARAMAKI","Archivo Black",w=1600,h=860,lh=0.9)),
  label("casting", "8%", color="#F5EBDD", size=36)], cam={"animate":[{"from":{"zoom":1.0},"to":{"zoom":1.06},"duration":B}]})
# 8 · EDITOR · ODILE MARCHETTI — six caps giant scroll on red
card(B, [rect("#E3120B"),
  {"text":"ODILE MARCHETTI","font":"Six Caps","size":1250,"color":"#FFF4E8","x":"=1300-t*900","anchor":"center","width":4000,"lineHeight":0.8},
  {"shape":"rect","width":260,"height":64,"fill":"#111","x":"50%","y":"91%"}, label("editor", "91%", color="#FFF4E8", size=36)])
# 9 · PRODUCTION DESIGN · SAUL KIM — stripes
card(B, [rect("#0A0A0A"), fxlayer(STRIPES, {"TINT":v3("#FFFFFF"),"TINT2":v3("#FF5A00")}, chan("SAUL\nKIM","Rubik Mono One",w=1700,h=820,lh=0.95,y="54%")),
  label("production design", "7%", color="#FF5A00", size=36)], cam={"animate":[{"from":{"x":-60},"to":{"x":0},"duration":B,"ease":"outExpo"}]})
# 10 · CINEMATOGRAPHY · LEV ANSARI — Bodoni italic gold on green
card(B, [rect("radial-gradient(#0E4A33, #031A12)"),
  {"text":"*Lev Ansari*","font":"Bodoni Moda","weight":900,"size":300,"width":1800,"color":"linear-gradient(180deg, #FFF1B8, #D9A441 55%, #8A5A12)","shadow":{"x":0,"y":10,"blur":40,"color":"#000a"}},
  label("cinematography", "20%", color="#E8C979", size=36)], cam={"animate":[{"from":{"zoom":1.14},"to":{"zoom":1.0},"duration":0.3,"ease":"outExpo"}]})
# 11 · MUSIC · HAZE & CINDER — cyan neon
card(B, [rect("#020407"), fxlayer(NEON, {"TINT":v3("#26E0FF"),"SEED":"7.0","ON":"1.0"}, chan("HAZE &\nCINDER","Anton",w=1400,h=700,lh=0.92,y="56%")),
  label("music", "8%", color="#26E0FF", size=36)])
# 12 · LED jumbotron · WRITTEN BY
card(B, [rect("#000"), fxlayer(LED, {"TINT":v3("#FFFFFF"),"CELL":"9.0","BGA":"0.12"}, chan("WRITTEN &\nDIRECTED BY","Anton",w=1800,h=900,lh=0.9))])
# 13 · ADA — 8th note strobe (4 × B/2)
for i,(txt,bg,fg,font) in enumerate([("ADA","#FFFFFF","#000","Anton"),("ADA","#000","#FFFFFF","Anton"),("KOVÁČ","#FF2D7A","#000","Anton"),("KOVÁČ","#000","#FF2D7A","Anton")]):
    card(B/2, [rect(bg), {"text":txt,"font":font,"size":900 if len(txt)<4 else 640,"color":fg,"letterSpacing":"-0.01em"}],
        cam={"animate":[{"from":{"zoom":1.0+0.06*i},"to":{"zoom":1.04+0.06*i},"duration":B/2}]})
# 14 · swell: black, letters flicker awake one by one (bar5 beat1 → beat3)
TITLE="NIGHT CHORUS"
def masked(i):
    return "".join("\n" if ch==" " else (f"[{ch}]{{#FFFFFF}}" if j==i else f"[{ch}]{{#FFFFFF00}}") for j,ch in enumerate(TITLE))
TREAT=[(NEON,{"TINT":v3("#FF2D7A"),"SEED":"1.0","ON":"1.0"}),(BULBS,{"TINT":v3("#FFC46B"),"CELL":"20.0","BGA":"0.0"}),(MOIRE,{"TINT":v3("#00D9B5")}),
 (HALFTONE,{"TINT":v3("#F2E600"),"CELL":"14.0"}),(CHROMA,{"TINT":"vec3(1.0)","OFF":"9.0"}),None,
 (EXTRUDE,{"TINT":v3("#F5EBDD"),"TINT2":v3("#E0261B"),"DIR":"vec2(-0.6,1.0)"}),(LED,{"TINT":v3("#5AA0FF"),"CELL":"12.0","BGA":"0.0"}),
 (STRIPES,{"TINT":v3("#FFFFFF"),"TINT2":v3("#FF5A00")}),(NEON,{"TINT":v3("#26E0FF"),"SEED":"2.0","ON":"1.0"}),
 (BULBS,{"TINT":v3("#EAF2FF"),"CELL":"20.0","BGA":"0.0"}),(HALFTONE,{"TINT":v3("#FF2D7A"),"CELL":"14.0"})]
import random
random.seed(4)
def title_layers(phase):
    L=[]
    order=[i for i,t in enumerate(TREAT) if t]
    offs=[0.06+k*(B/4)*0.66 for k in range(len(order))]; random.shuffle(offs)
    for i in order:
        body,subs=TREAT[i]
        lay=fxlayer(body,subs,chan(masked(i),"Anton",size=500,w=1900,lh=0.86))
        if phase=="swell":
            # each letter wakes with a 2-frame flicker at its own random time across the swell
            t0=random.uniform(0.02, 0.85*B*2)
            lay["opacity"]=f"=t<{t0:.3f}?0:(t<{t0+0.033:.3f}?0.7:(t<{t0+0.1:.3f}?0:(t<{t0+0.133:.3f}?0.4:0.1)))"
        if phase=="off":
            t0=offs.pop()
            lay["opacity"]=f"=t<{t0:.3f}?1:(t<{t0+0.035:.3f}?0.2:(t<{t0+0.07:.3f}?1:0))"
        L.append(lay)
    return L
card(2*B, [rect("#000")]+title_layers("swell"))
# 15 · TITLE SLAM + hold (bar5 beat3 → bar6 end)  = 6 beats
card(6*B, [rect("#000")]+title_layers("on")+[label("a film by ada kováč", "94%", color="#B8B2A8", size=34, opacity="=t<1.2?0:1")],
  cam={"animate":[{"from":{"zoom":1.12},"to":{"zoom":1.0},"duration":0.25,"ease":"outExpo"},{"to":{"zoom":1.05},"at":0.25,"duration":6*B-0.25,"ease":"linear"}],
       "shake":{"amount":14,"at":0,"duration":0.3}})
# 16 · lights out (bar7, 2 beats) then black
card(2*B, [rect("#000")]+title_layers("off"), cam={"animate":[{"from":{"zoom":1.05},"to":{"zoom":1.07},"duration":2*B}]})
card(0.5, [rect("#000")])

spec={"size":[1920,1080],"fps":30,"background":"#000","fonts":FONTS,"transition":"cut","intent":{"pace":"frantic"},
 "music":dict(json.load(open("score.json")), followCuts=False),
 "post":{"grain":0.035,"halation":0.25,"bloom":0.25,"vignette":0.25,"aberration":0.05},
 "scenes":scenes}
json.dump(spec,open('nightchorus.json','w'),indent=1)
print(sum(s["duration"] for s in scenes))
