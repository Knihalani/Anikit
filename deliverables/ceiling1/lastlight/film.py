import json, math, copy
K=10.0                       # scene units = real metres × 10 (engine fog/light falloff assume a ~5 m set)
SPIN=200.0                   # 33⅓ rpm = 200 °/s
REC=(-0.4, 0.0)              # record centre (scaled x,z)
PIV=(1.7, -1.2)              # tonearm pivot (scaled)
Y_REC=0.95                  # record top
FPS=24

def grooves():
    W,H=4096,1024
    L=[{"shape":"rect","width":W,"height":H,"fill":"#0B0B0D","x":W/2,"y":H/2}]
    lab=int(0.40*H); gaps=[0.55,0.68,0.82]; y=lab+40
    while y<H-14:
        g=y/H
        if not any(abs(g-q)<0.012 for q in gaps):
            L.append({"shape":"rect","width":W,"height":1.4,"fill":"#26262C","x":W/2,"y":y})
        y+=3.2
    L.append({"shape":"rect","width":W,"height":12,"fill":"#151518","x":W/2,"y":H-6})
    L.append({"shape":"rect","width":W,"height":lab,"fill":"#B8361E","x":W/2,"y":lab/2})
    L.append({"shape":"rect","width":W,"height":3,"fill":"#E9D9B8","x":W/2,"y":lab-26})
    for k in range(2):
        L.append({"text":"ORRERY","font":"Bodoni Moda","weight":900,"size":120,"color":"#F1E6CF","x":W*(0.25+0.5*k),"y":lab-110,"letterSpacing":"0.25em","rotation":180})
        L.append({"text":"SIDE A  ·  33⅓  ·  LAST LIGHT","font":"Instrument Sans","size":46,"color":"#F1E6CF","x":W*(0.5*k),"y":lab-210,"letterSpacing":"0.3em","rotation":180})
    L.append({"shape":"rect","width":W,"height":40,"fill":"#0B0B0D","x":W/2,"y":lab+20})
    return {"size":[W,H],"duration":1,"layers":L}

def walnut():
    # end-grain-free walnut veneer for the plinth front: long wavy grain lines
    W,H=2048,320
    L=[{"shape":"rect","width":W,"height":H,"fill":"#3E2414","x":W/2,"y":H/2}]
    import random; random.seed(3)
    for k in range(70):
        y=random.uniform(0,H); a=random.uniform(2,9); ph=random.uniform(0,6)
        d="M0 %.1f "%y + " ".join("L%d %.1f"%(x, y+a*math.sin(x/random.uniform(90,140)+ph)) for x in range(0,W+64,64))
        c=random.choice(["#2A170C","#56331C","#341D10","#5E3A20"])
        L.append({"shape":"path","d":d,"stroke":c,"strokeWidth":random.uniform(1,5),"fill":"none","x":0,"y":0,"anchor":"topLeft"})
    return {"size":[W,H],"duration":1,"layers":L}

LACQ={"base":"#120C08","roughness":0.22,"clearcoat":1}
WAL={"base":"#ffffff","roughness":0.5,"clearcoat":0.4}

def arm_xz(a_deg, rel):
    # rotate rel (x,z) about the pivot by a (degrees, positive = toward the record)
    a=math.radians(a_deg); x,z=rel
    return (PIV[0]+x*math.cos(a)-z*math.sin(a), PIV[1]+x*math.sin(a)+z*math.cos(a))

def objects(arm="0", lift="0", spin=True, motes=True):
    """arm, lift: expressions of t (degrees toward record; scaled units of lift)."""
    sp=f"t*{SPIN}" if spin else "0"
    O=[
     {"type":"box","size":[40,0.4,30],"pos":[0,-0.2,0],"material":{"base":"#1A120C","roughness":0.55}},
     {"type":"box","size":[4.7,0.75,3.7],"radius":0.12,"bevel":0.1,"pos":[0,0.375,0],"material":LACQ,"name":"plinth","texture":{"composition":"walnut","face":"front"}},
     {"type":"cylinder","radius":1.52,"height":0.18,"pos":[REC[0],0.84,REC[1]],"material":"aluminium-bead"},
     {"type":"cylinder","radius":1.53,"height":0.012,"pos":[REC[0],0.932,REC[1]],"material":{"base":"#26221F","roughness":1.0}},
     {"type":"cylinder","radius":1.50,"height":0.012,"pos":[REC[0],0.943,REC[1]],"material":{"base":"#0a0a0c","roughness":0.25}},
     {"type":"surface","x":"=1.5*v*cos(u*6.28318)","y":"=1.5*v*sin(u*6.28318)","z":"=0","u":[0,1,256],"v":[0.02,1,48],
      "pos":[REC[0],Y_REC,REC[1]],"rot":f"=[-90, 0, {sp}]","texture":{"composition":"grooves","face":"both"},
      "material":{"base":"#ffffff","roughness":0.14,"clearcoat":1},"name":"record"},
     {"type":"cylinder","radius":0.04,"height":0.22,"pos":[REC[0],1.0,REC[1]],"material":"chrome"},
     # strobe dots on the platter rim (they rotate with it)
     {"type":"box","count":90,"size":[0.05,0.05,0.02],"pos":f"=[{REC[0]}+1.525*cos((i*4+{sp})*0.0174533), 0.84, {REC[1]}-1.525*sin((i*4+{sp})*0.0174533)]",
      "rot":f"=[0, i*4+{sp}+90, 0]","material":{"base":"#C9C6BE","roughness":0.3,"metallic":0.6}},
     # pivot, arm rest, counterweight
     {"type":"cylinder","radius":0.2,"height":0.3,"pos":[PIV[0],0.9,PIV[1]],"material":"aluminium"},
     {"type":"cylinder","radius":0.05,"height":0.35,"pos":[2.05,0.92,0.75],"material":"aluminium"},
    ]
    # tonearm: tube in pivot space, rotated by arm(t) and lifted by lift(t)
    A=f"(({arm})*0.0174533)"
    rel=[(0,0),(-0.2,1.0),(-0.7,1.8),(-1.0,2.05)]
    pts=[]
    path="=[" + f"{PIV[0]}+x0*cos({A})-z0*sin({A})" + "," + f"(1.12+({lift}))+yy" + "," + f"{PIV[1]}+x0*sin({A})+z0*cos({A})" + "]"
    # piecewise-linear arm path in s (three segments), as an expression
    def seg(c0,c1): return f"mix({c0},{c1},clamp(s*3-k,0,1))"
    xs=[p[0] for p in rel]; zs=[p[1] for p in rel]
    x0="(s<0.3333?mix(%g,%g,s*3):(s<0.6667?mix(%g,%g,s*3-1):mix(%g,%g,s*3-2)))"%(xs[0],xs[1],xs[1],xs[2],xs[2],xs[3])
    z0="(s<0.3333?mix(%g,%g,s*3):(s<0.6667?mix(%g,%g,s*3-1):mix(%g,%g,s*3-2)))"%(zs[0],zs[1],zs[1],zs[2],zs[2],zs[3])
    path=path.replace("x0",x0).replace("z0",z0).replace("yy","(-0.08*s)")
    O.append({"type":"tube","path":path,"radius":0.045,"segments":120,"sides":16,"material":"chrome"})
    # counterweight behind the pivot
    cx,cz=-0.0,-0.5
    O.append({"type":"cylinder","radius":0.17,"height":0.3,"rot":f"=[90,0,0]",
              "pos":f"=[{PIV[0]}+({cx})*cos({A})-({cz})*sin({A}), 1.12+({lift}), {PIV[1]}+({cx})*sin({A})+({cz})*cos({A})]","material":"aluminium"})
    # headshell + cartridge + stylus at the arm's end (rel -1.0, 2.05 → extend a little)
    O.append({"type":"extrude","d":"M0 12 L44 12 L50 6 L62 0 L64 3 L54 9 L50 18 L46 22 L8 22 L0 17 Z","height":0.075,"depth":0.2,"bevel":0.008,
              "rot":f"=[0, -({arm})-90+0, 0]","material":{"base":"#141414","roughness":0.3,"clearcoat":0.6},
              "pos":f"=[{PIV[0]}+(-1.05)*cos({A})-(1.98)*sin({A}), 1.03+({lift}), {PIV[1]}+(-1.05)*sin({A})+(2.02)*cos({A})]"})
    for (rx,rz,sz,yo,mat) in [
                              (-1.05,2.27,[0.14,0.12,0.18],0.97,{"base":"#7A1E12","roughness":0.4}),
                              (-1.05,2.33,[0.008,0.07,0.008],0.93,"chrome")]:
        O.append({"type":"box","size":sz,"radius":min(sz)*0.3,"rot":f"=[0, -({arm}), 0]",
                  "pos":f"=[{PIV[0]}+({rx})*cos({A})-({rz})*sin({A}), {yo}+({lift}), {PIV[1]}+({rx})*sin({A})+({rz})*cos({A})]","material":mat})
    if motes:
        O.append({"type":"particles","count":3000,"emit":{"shape":"box","size":[6,4,6],"pos":[-1.0,2.6,-0.8]},
                  "flow":{"strength":0.05,"scale":1.5,"evolve":0.1},"gravity":-0.002,"drag":0.9,"life":6,
                  "size":"=0.008+0.012*rand(i)","color":["#FFD7A0"],"blend":"add","emissive":1.5,"opacity":"=0.8*smoothstep(0,0.15,age)*(1-smoothstep(0.8,1,age))*(1-smoothstep((36.25-((x+0.4)*-0.5407+(y-0.9)*0.32+(z-0.2)*-0.778))*0.06,(36.25-((x+0.4)*-0.5407+(y-0.9)*0.32+(z-0.2)*-0.778))*0.085,sqrt(max(0,(x+0.4)*(x+0.4)+(y-0.9)*(y-0.9)+(z-0.2)*(z-0.2)-((x+0.4)*-0.5407+(y-0.9)*0.32+(z-0.2)*-0.778)*((x+0.4)*-0.5407+(y-0.9)*0.32+(z-0.2)*-0.778)))))"})
    return O

SUN=[-20.0,12.5,-28.0]
def lights():
    L=[{"pos":SUN,"target":[-0.4,0.9,0.2],"angle":7.0,"penumbra":0.08,"intensity":14000,"color":"#FF9E4F"},
       {"dir":[0.6,0.5,0.8],"size":[60,40],"color":"#4C6A99","intensity":0.18}]
    return L
def blinds():
    mid=[c*0.62 for c in SUN]; O=[]
    for k in range(-9,10):
        O.append({"type":"box","size":[12,0.14,0.04],"pos":[mid[0],mid[1]+k*0.45,mid[2]],"rot":[0,36,0],"material":"black"})
    return O

def shot(dur, cam, arm="0", lift="0", spin=True, fog=0.0035, motes=True, extra=None, post=None, layers=None, xlights=None):
    props={"env":{"preset":"night"},"floor":False,"exposure":1.0,"quality":"high","camera":cam,
           "objects":objects(arm,lift,spin,motes)+blinds()+(extra or []),"lights":lights()+(xlights or []),"fog":{"density":fog,"color":"#0a0806","shafts":True},
           "bloom":0.08,"vignette":0.35}
    s={"duration":dur,"layers":[{"component":"scene3d","props":props}]+(layers or [])}
    if post: s["post"]=post
    return s

def keys(*kk):
    return {"interp":"smooth","keys":[dict(t=k[0],pos=k[1],target=k[2],focal=k[3],fstop=k[4],**({"focus":k[5]} if len(k)>5 else {})) for k in kk]}

ARM_PLAY=0.0      # playing angle (arm geometry is drawn in the playing position)
ARM_REST=-24.0    # parked on its rest
scenes=[]
# S1 macro along the outer grooves, low, into the light; label soft in the distance
scenes.append(shot(2.4, keys((0,[3.6,1.05,3.9],[-1.0,1.9,-2.0],35,2.8,5.6),(2.4,[3.3,1.02,3.6],[-1.0,2.05,-2.0],35,2.8,5.4)), arm=str(ARM_REST)))
# S2 the arm swings out, in silhouette against the shaft: low, far side, looking toward the sun
scenes.append(shot(1.8, keys((0,[2.6,1.05,3.2],[0.6,1.25,-1.0],50,2.8),(1.8,[2.3,1.05,3.0],[0.4,1.25,-1.0],50,2.8)),
    arm=f"{ARM_REST}+({ARM_PLAY-ARM_REST})*smoothstep(0.2,1.5,t)", lift="0.15*smoothstep(0.0,0.3,t)"))
# S3 the descent: record level, under the cartridge; the gap between stylus and groove closes
sx,sz=arm_xz(ARM_PLAY,(-1.05,2.33))
scenes.append(shot(2.2, keys((0,[sx+2.1,0.99,sz+1.25],[sx-0.3,0.99,sz-0.2],85,2.8),(2.2,[sx+2.1,0.99,sz+1.25],[sx-0.3,0.99,sz-0.2],85,2.8)),
    arm=str(ARM_PLAY), lift="0.32*pow(1-smoothstep(0.0,2.1,t),1.6)"))
# S4 wide reveal as the music blooms: camera cranes up and back
scenes.append(shot(2.0, keys((0,[5.5,1.25,6.5],[-0.3,0.9,0],75,3.5),(2.0,[6.0,1.85,7.2],[-0.5,1.1,0],48,4)), arm=str(ARM_PLAY)))
# S5: the last light writes the name on the wall: a gobo beam through the wordmark (shadow letters in a pool of sun)
import math as _m
GL=[-10.0,6.0,-4.0]; GW=[5.0,3.4,-1.0]
_dx,_dy,_dz=[GW[i]-GL[i] for i in range(3)]
_yaw=_m.degrees(_m.atan2(-_dx,-_dz)); _pitch=_m.degrees(_m.atan2(_dy,_m.hypot(_dx,_dz)))
_C=[GL[i]+0.45*(GW[i]-GL[i]) for i in range(3)]
wall={"type":"box","size":[0.3,9,14],"pos":[5.15,4.5,-1],"material":{"base":"#4a443c","roughness":0.9}}
gobo={"type":"text","text":"ORRERY","weight":900,"height":0.22,"depth":0.03,"align":"center","pos":[_C[0],_C[1]-0.11,_C[2]],"rot":[_pitch,_yaw,0],"material":"black"}
scenes.append(shot(2.4, keys((0,[-3.5,1.7,7.5],[4.5,2.8,-1.0],30,5.6),(2.4,[-2.6,1.8,6.2],[4.6,3.0,-1.0],32,5.6)), arm=str(ARM_PLAY),
    extra=[wall,gobo], xlights=[{"pos":GL,"target":GW,"angle":15,"penumbra":0.3,"intensity":5000,"color":"#FFB070"}],
    ))

spec={"size":[1920,1080],"fps":FPS,"transition":"cut","background":"#000","intent":{"pace":"calm"},
 "fonts":{"Bodoni Moda":"../fonts/Bodoni_Moda_ital_wght_1_900.ttf","Instrument Sans":"../fonts/Instrument_Sans_wght_700.ttf"},
 "compositions":{"grooves":grooves(),"walnut":walnut()},
 "post":{"grain":0.03,"halation":0.3,"vignette":0.2},
 "music":dict(json.load(open("score.json")), followCuts=False),
 "scenes":scenes}
json.dump(spec,open('lastlight.json','w'),indent=1)
print(sum(s["duration"] for s in scenes))
