import json, sys
sys.path.insert(0,'..')
from fx import *
def v3(h):
    h=h.lstrip('#'); return "vec3(%.3f,%.3f,%.3f)"%tuple(int(h[i:i+2],16)/255 for i in (0,2,4))
def sub(body, subs):
    for k,v in sorted(subs.items(), key=lambda kv:-len(kv[0])): body=body.replace(k,str(v))
    return body
FONTS={"Anton":"../../fonts/Anton.ttf","Monoton":"../../fonts/Monoton.ttf"}
marquee={"size":[1600,500],"duration":4,"background":"#00000000","layers":[
  {"shader":shader(sub(BULBS,{"TINT":v3("#FFC46B"),"CELL":"22.0","BGA":"0.6"})),"alpha":True,
   "channels":[{"layers":[{"text":"TOMAS VELD","font":"Anton","fit":True,"width":1500,"height":420,"size":600,"color":"#fff"}],"mipmaps":True}]}]}
neon={"size":[1400,700],"duration":4,"background":"#00000000","layers":[
  {"text":"A HALCYON\nPICTURE","font":"Monoton","size":210,"lineHeight":1.02,"color":"#FFD2E4","glow":"#FF2D7A","align":"center"}]}
objs=[
 # street: two facades of dark brick boxes
 {"type":"box","size":[0.6,9,40],"pos":[-4.2,0,-10],"material":{"base":"#1a1614","roughness":0.85}},
 {"type":"box","size":[0.6,9,40],"pos":[4.2,0,-10],"material":{"base":"#16181c","roughness":0.85}},
 {"type":"pane","size":[3.2,1.0],"pos":[-3.85,2.8,-3],"rot":[0,90,0],"texture":"#marquee","material":{"base":"#ffffff","glow":1.0,"transmission":0,"roughness":0.4}},
 {"type":"pane","size":[2.6,1.3],"pos":[3.85,3.6,-7],"rot":[0,-90,0],"texture":"#neon","material":{"base":"#ffffff","glow":1.0,"transmission":0,"roughness":0.4}},
]
scene={"env":"city","exposure":1.0,"quality":"standard","objects":objs,
 "camera":{"keys":[{"t":0,"pos":[0.6,1.4,4.5],"target":[-1.5,2.4,-4],"focal":28,"fstop":2.8},{"t":2,"pos":[0.4,1.4,2.5],"target":[-1.5,2.4,-6],"focal":28,"fstop":2.8}]},
 "fog":{"density":0.02,"color":"#0a0c14"}}
spec={"size":[1920,1080],"fps":24,"fonts":FONTS,"compositions":{"marquee":marquee,"neon":neon},
 "scenes":[{"duration":2,"layers":[{"component":"scene3d","props":scene}]}]}
json.dump(spec,open('spike.json','w'),indent=1)
