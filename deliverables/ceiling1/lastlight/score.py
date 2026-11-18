import json, random
random.seed(11)
# vinyl crackle: sparse 64th-note noise clicks, from the stylus touch (6.3 s) to the end
cr=[]
for k in range(64*8):
    r=random.random()
    cr.append("C5/64" if r<0.05 else ("C5/64?" if r<0.16 else "r/64"))
crackle=" ".join(cr)
music={"bpm":60,"key":"Db","length":"10.6s","seed":3,"end":"ring",
 "define":{
   "crackle":{"synth":{"osc":[{"wave":"sine","level":0.0}],"noise":1.0,"filter":{"type":"bp","cutoff":2600,"res":0.15},"amp":[0.0005,0.006,0.0,0.004]}},
   "hiss":{"synth":{"osc":[{"wave":"sine","level":0.0}],"noise":1.0,"filter":{"type":"lp","cutoff":5200,"res":0.05},"amp":[0.3,0.2,1.0,1.0]}}
 },
 "tracks":[
  {"name":"clock","inst":"woodblock","clips":[{"at":"@0.25","notes":"o5 l4 C5? C5? C5? C5?"}],"gain":-20,"reverb":0.25,"pan":-0.35},
  {"name":"room","inst":"synth:pad-dark","clips":[{"at":"@0","notes":"o2 [Db2 Ab2]/1 [Db2 Ab2]/2"}],"gain":-30},
  {"name":"crackle","inst":"crackle","clips":[{"at":"@6.3","notes":crackle}],"gain":-16,"pan":0.1},
  {"name":"hiss","inst":"hiss","clips":[{"at":"@6.3","notes":"C4/1 C4/4"}],"gain":-38},
  {"name":"piano","inst":"salamander-piano|piano","clips":[
      {"at":"@6.45","notes":"o3 [Db2 Ab2 C4 Eb4 F4]/2.~ r/8 F5/8 Eb5/4 C5/4 [Bb1 F2 Db4 Ab4 C5]/1"}],"gain":-3,"reverb":0.35},
  {"name":"strings","inst":"strings","clips":[{"at":"@6.45","notes":"o3 [Db3 Ab3 F4]/1 [Bb2 F3 Db4]/1"}],"gain":-12,"reverb":0.4}
 ],
 "sfx":[{"sfx":"click","at":"@2.55","gain":-10},{"sfx":"tick","at":"@3.95","gain":-14}],
 "master":{"reverb":"room"}
}
json.dump(music,open('score.json','w'),indent=1)
