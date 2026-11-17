import json, math
# ---- groove texture: a record seen from above (composition), alpha outside the disc
def grooves():
    # polar unwrap: x = angle (0..1 → 4096 px), y = radius (0 centre → 1024 rim)
    W,H=4096,1024
    L=[{"shape":"rect","width":W,"height":H,"fill":"#0B0B0D","x":W/2,"y":H/2}]
    lab=int(0.40*H)
    # grooves: fine lines from the label to the rim, with 3 track gaps
    gaps=[0.55,0.68,0.82]
    y=lab+40
    while y<H-14:
        g=y/H
        if not any(abs(g-q)<0.012 for q in gaps):
            L.append({"shape":"rect","width":W,"height":1.4,"fill":"#24242A","x":W/2,"y":y})
        y+=3.2
    L.append({"shape":"rect","width":W,"height":12,"fill":"#151518","x":W/2,"y":H-6})   # rim
    L.append({"shape":"rect","width":W,"height":lab,"fill":"#B8361E","x":W/2,"y":lab/2})  # label
    L.append({"shape":"rect","width":W,"height":3,"fill":"#E9D9B8","x":W/2,"y":lab-26})
    for k in range(2):
        L.append({"text":"ORRERY","font":"Bodoni Moda","weight":900,"size":120,"color":"#F1E6CF","x":W*(0.25+0.5*k),"y":lab-110,"letterSpacing":"0.25em"})
        L.append({"text":"SIDE A  ·  33⅓ RPM  ·  LAST LIGHT","font":"Instrument Sans","size":46,"color":"#F1E6CF","x":W*(0.5*k),"y":lab-210,"letterSpacing":"0.3em"})
    L.append({"shape":"rect","width":W,"height":40,"fill":"#0B0B0D","x":W/2,"y":lab+20})
    return {"size":[W,H],"duration":1,"layers":L}
WAL={"base":"#4A2A16","roughness":0.42,"clearcoat":0.5}
objects=[
 {"type":"box","size":[4,0.04,3],"pos":[0,-0.02,0],"material":{"base":"#1c140e","roughness":0.6}},       # table
 {"type":"box","size":[0.47,0.075,0.37],"pos":[0,0.0375,0],"material":WAL,"name":"plinth"},
 {"type":"cylinder","radius":0.152,"height":0.018,"pos":[-0.04,0.075,0],"material":"aluminium-bead"},
 {"type":"cylinder","radius":0.150,"height":0.002,"pos":[-0.04,0.093,0],"material":{"base":"#0a0a0c","roughness":0.2}},
 {"type":"surface","x":"=1.5*v*cos(u*6.28318)","y":"=1.5*v*sin(u*6.28318)","z":"=0","u":[0,1,256],"v":[0.02,1,48],"pos":[-0.04,0.0945,0],"rot":"=[-90, 0, t*200]","texture":{"composition":"grooves","face":"both"},"material":{"base":"#ffffff","roughness":0.16,"clearcoat":1},"name":"record"},
 {"type":"cylinder","radius":0.004,"height":0.02,"pos":[-0.04,0.093,0],"material":"chrome"},
 # tonearm
 {"type":"cylinder","radius":0.018,"height":0.03,"pos":[0.17,0.075,-0.12],"material":"aluminium"},
 {"type":"tube","points":[[0.17,0.112,-0.12],[0.15,0.112,-0.02],[0.10,0.106,0.06],[0.07,0.102,0.085]],"radius":0.0045,"material":"chrome","name":"arm"},
 {"type":"box","size":[0.022,0.012,0.03],"pos":[0.065,0.097,0.088],"material":{"base":"#111","roughness":0.35}},
 {"type":"cylinder","radius":0.016,"height":0.026,"pos":[0.175,0.099,-0.165],"rot":[90,0,0],"material":"aluminium"},
]
lights=[{"pos":[-2.6,0.55,-1.4],"target":[0,0.1,0],"angle":16,"penumbra":0.15,"intensity":4000,"color":"#FFB46A"}]
# blinds: slats between sun and subject, not seen by camera
for k in range(9):
    objects.append({"type":"box","size":[0.02,0.018,0.9],"pos":[-1.3,0.08+k*0.06,-0.7],"rot":[0,-28,25],"material":"black","camera":False})
K=10.0
def sc(o):
    for k in ("size","pos","points"):
        if k in o:
            v=o[k]
            if k=="points": o[k]=[[c*K for c in p] for p in v]
            else: o[k]=[c*K for c in v]
    for k in ("radius","height"):
        if k in o: o[k]*=K
    return o
objects=[sc(o) for o in objects]
for l in lights: l["pos"]=[c*K for c in l["pos"]]; l["target"]=[c*K for c in l["target"]]
scene={"env":{"preset":"night"},"floor":False,"exposure":1.0,
 "camera":{"keys":[{"t":0,"pos":[5.5,3.2,7.5],"target":[0,0.8,0],"focal":50,"fstop":2.8},{"t":3,"pos":[4.2,2.8,8.0],"target":[0,0.8,0],"focal":50,"fstop":2.8}]},
 "objects":objects,"lights":lights,"fog":{"density":0.005,"color":"#0a0806","shafts":True}}
spec={"size":[1920,1080],"fps":30,"transition":"cut","compositions":{"grooves":grooves()},
 "scenes":[{"duration":3,"layers":[{"component":"scene3d","props":scene}]}]}
json.dump(spec,open('test.json','w'),indent=1)
