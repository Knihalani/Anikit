import json, math, random
random.seed(5)
B=60/128
SIGN_W, SIGN_H = 3.2, 1.8
# credit cards (pre-rendered from v1) and the beat each lands on
cards=[("c00",0.0),("c01",2*B),("c02",4*B),("c03",5*B),("c04",6*B),("c05",7*B),("c06",8*B),("c07",9*B),("c08",10*B),("c09",11*B),("c10",12*B),("c11",13*B)]
strobe=[("c12",14*B),("c13",14.5*B),("c14",15*B),("c15",15.5*B)]
T_SWELL=16*B; T_SLAM=18*B; T_OFF=24*B; T_END=26*B+0.5
comps={}
def vs(e): return f"=[{e},{e},{e}]"
def tex(name):
    comps[name]={"size":[1920,1080],"duration":1,"layers":[{"src":f"cards/{name}.png","width":1920,"height":1080,"x":960,"y":540}]}
    return {"composition":name}
for n,_ in cards+strobe: tex(n)
tex("c16")
O=[]
GLOW={"base":"#ffffff","glow":1.0,"roughness":0.35,"transmission":0}
# --- the street: facades, windows, lamps
for side in (-1,1):
    O.append({"type":"box","size":[1.0,16,140],"pos":[side*5.6,0,-50],"material":{"base":"#141210" if side<0 else "#111317","roughness":0.9}})
    for z in range(6,-96,-4):
        for fl in range(2,5):
            if random.random()<0.45:
                c=random.choice(["#FFB866","#FFD9A0","#9CC4FF","#FF9A5A","#2A2A30","#2A2A30"])
                O.append({"type":"plane","size":[1.3,1.6],"pos":[side*5.08,fl*2.6+0.4,z+random.uniform(-0.6,0.6)],"rot":[0,-side*90,0],
                          "material":{"base":c,"emissive":c,"emissiveStrength":0.5 if c!="#2A2A30" else 0.0,"roughness":0.2}})
# textured surfaces: brick facades (panes, since box sides take no texture) and wet asphalt
comps['brick']={"size":[2048,2048],"duration":1,"layers":[{"src":"tex/brick_dark.png","width":2048,"height":2048,"x":1024,"y":1024}]}
comps['asphalt']={"size":[2048,2048],"duration":1,"layers":[{"src":"tex/asphalt.png","width":2048,"height":2048,"x":1024,"y":1024}]}
for side in (-1,1):
    for z in range(8,-112,-4):
        for yy in (3.0,9.0):
            O.append({"type":"pane","size":[4.02,6.02],"pos":[side*5.095,yy,z-2],"rot":[0,-side*90,0],"thin":True,
                      "texture":{"composition":"brick","face":"both"},"material":{"base":"#ffffff","roughness":0.85}})
# sidewalk curbs
for side in (-1,1):
    O.append({"type":"box","size":[2.2,0.16,140],"pos":[side*4.0,0,-50],"material":{"base":"#24221F","roughness":0.75}})
# --- credit signs: projecting blade signs, alternating sides, every 7 m
from PIL import Image
def avg(n):
    im=Image.open(f'cards/{n}.png').convert('RGB').resize((32,18)); px=list(im.getdata());
    r=sum(p[0] for p in px)/len(px); g=sum(p[1] for p in px)/len(px); b=sum(p[2] for p in px)/len(px); m=max(r,g,b,1)
    return '#%02x%02x%02x'%(int(255*r/m),int(255*g/m),int(255*b/m))
signs=[]
signlights=[]
for k,(n,t) in enumerate(cards):
    side=-1 if k%2==0 else 1
    z=-7.0*k
    y=3.0 + (0.4 if k%3==0 else 0.0)
    x=side*(5.1-SIGN_W/2-0.25)
    O.append({"type":"pane","size":[SIGN_W,SIGN_H],"pos":[x,y,z],"rot":[0,side*12,0],"texture":tex(n) if False else {"composition":n,"face":"both"},
              "material":GLOW,"thin":True,"name":n,
              "scale":vs(f"(t<{T_OFF+0.05+k*0.11:.3f}?1:0)") if k else vs("(t<0.06?0:(t<0.10?1:(t<0.16?0:(t<0.20?1:(t<0.24?0:1)))))")})
    # bracket arm
    O.append({"type":"box","size":[0.5,0.06,0.06],"pos":[side*4.9,y+SIGN_H/2+0.08,z],"material":"black"})
    signs.append((x,y,z,side,t))
    signlights.append({"pos":[x-side*0.4,y-0.4,z+0.9],"color":avg(n),"intensity":40,"shadow":False})
# --- the LED screen for the 8th-note strobe, high on the right
Zs=-7.0*len(cards)-2
for j,(n,t) in enumerate(strobe):
    t1=strobe[j+1][1] if j+1<len(strobe) else T_SWELL
    O.append({"type":"pane","size":[5.6,3.15],"pos":[1.2,5.2,Zs],"rot":[0,-8,0],"texture":{"composition":n,"face":"both"},"material":GLOW,
              "scale":vs(f"((t>={t:.3f} && t<{t1:.3f})?1:0)")})
# --- title billboard at the end of the street
Zt=Zs-16
O.append({"type":"pane","size":[14,7.875],"pos":[0,6.2,Zt],"texture":{"composition":"c16","face":"both"},"material":GLOW,
          "scale":vs(f"((t<{T_SLAM:.3f})?0:(t<{T_OFF+0.47:.3f}?1:(t<{T_OFF+0.50:.3f}?0:(t<{T_OFF+0.56:.3f}?1:(t<{T_OFF+0.6:.3f}?0:(t<{T_OFF+0.66:.3f}?1:0))))))")})
O.append({"type":"box","size":[14.6,8.4,0.3],"pos":[0,2.2,Zt-0.3],"material":{"base":"#0b0b0c","roughness":0.6}})
# street lamps (sodium)
lights=[]
for fx in (-4.5,0,4.5):
    O.append({'type':'box','size':[0.5,0.25,0.4],'pos':[fx,2.0,Zt+1.6],'rot':[-35,0,0],'material':'black'})
    lights.append({'pos':[fx,2.2,Zt+2.0],'target':[fx,6.2,Zt],'angle':40,'penumbra':0.5,'intensity':60,'color':'#FFE2B0'})
for i,z in enumerate(range(2,-110,-14)):
    side=1 if i%2 else -1
    O.append({"type":"cylinder","radius":0.07,"height":5.0,"pos":[side*3.4,2.5,z],"material":"black"})
    lights.append({"pos":[side*3.0,4.9,z],"color":"#FFB25A","intensity":90})
lights.append({"dir":[0.2,0.6,0.7],"size":[80,40],"color":"#2B3A66","intensity":0.35})
lights+=signlights
# rain streaks
O.append({"type":"particles","count":6000,"emit":{"shape":"box","size":[10,12,60],"pos":[0,8,-30]},"gravity":9.0,"drag":0.0,"life":1.4,
          "size":0.006,"color":["#C8D4FF"],"blend":"add","emissive":0.6,"opacity":0.5,"stretch":4})
# --- camera: open wide down the street, then dolly through it, turning toward each sign on its beat
keys=[{"t":0.0,"pos":[0,1.4,9.5],"target":[0,2.4,-30],"focal":24,"fstop":8},
      {"t":round(2*B-0.02,4),"pos":[0,1.45,8.2],"target":[-0.3,2.5,-30],"focal":24,"fstop":8}]
for k,(x,y,z,side,t) in enumerate(signs):
    if k==0: continue
    cam=[-side*0.5, 1.5, z+6.2]
    tgt=[x*0.8, y-0.2, z]
    keys.append({"t":round(t+0.10,4),"pos":cam,"target":tgt,"focal":28,"fstop":5.6})
    nxt=signs[k+1][4] if k+1<len(signs) else strobe[0][1]
    keys.append({"t":round(nxt-0.03,4),"pos":[cam[0]*0.8,cam[1],cam[2]-0.9],"target":[tgt[0]*0.97,tgt[1],tgt[2]-0.2],"focal":28,"fstop":5.6})
# strobe: look up at the LED screen
keys.append({"t":round(strobe[0][1]+0.08,4),"pos":[-1.0,1.6,Zs+9],"target":[1.2,5.0,Zs],"focal":35,"fstop":4})
keys.append({"t":round(T_SWELL-0.02,4),"pos":[-0.9,1.6,Zs+7.6],"target":[1.2,5.0,Zs],"focal":35,"fstop":4})
# swell: settle into the dark end of the street (billboard off)
keys.append({"t":round(T_SWELL+0.25,4),"pos":[0,1.2,Zs+2],"target":[0,4.2,Zt],"focal":26,"fstop":5.6})
keys.append({"t":round(T_SLAM,4),"pos":[0,1.15,Zs+1.2],"target":[0,4.4,Zt],"focal":26,"fstop":5.6})
# slam + hold: slow push toward the title billboard
keys.append({"t":round(T_SLAM+0.25,4),"pos":[0,1.15,Zs-0.2],"target":[0,4.6,Zt],"focal":26,"fstop":5.6})
keys.append({"t":round(T_END,4),"pos":[0,1.25,Zs-3.0],"target":[0,4.8,Zt],"focal":26,"fstop":5.6})
props={"env":"city","exposure":1.35,"quality":"standard","objects":O,"lights":lights,
       "camera":{"interp":"linear","keys":keys},"shutter":0.3,
       "fog":{"density":0.018,"color":"#0b0d16"},"bloom":0.12,"vignette":0.3}
score=json.load(open('score_v3.json'))
spec={"size":[1920,1080],"fps":24,"background":"#000","transition":"cut","intent":{"pace":"frantic"},
 "compositions":comps,"music":dict(score, followCuts=False),"post":{"grain":0.03,"halation":0.25},
 "scenes":[{"duration":round(T_END,4),"layers":[{"component":"scene3d","props":props}]}]}
json.dump(spec,open('street.json','w'),indent=1)
print("objects",len(O),"keys",len(keys),"dur",round(T_END,2))
