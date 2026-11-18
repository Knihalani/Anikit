import json
BPM=132; b=60/BPM; BAR=4*b
T=[BAR+0*3*b, BAR+1*3*b, BAR+2*3*b, BAR+3*3*b]; T+=[T[-1]+3*b+k*2*b for k in range(4)]
T_PULL=6*BAR; T_LOCK=7*BAR
music={"bpm":BPM,"key":"Gm","length":"15s","seed":9,"end":"ring",
 "define":{
  "hum":{"synth":{"osc":[{"wave":"sine","level":0.8},{"wave":"saw","semis":12,"level":0.25}],"sub":0.4,"filter":{"type":"lp","cutoff":260,"res":0.2},"lfo":{"rate":6,"trem":0.08},"amp":[0.3,0.2,1.0,0.8]}},
  "arc":{"synth":{"osc":[{"wave":"square","pw":0.2,"level":0.3}],"noise":1.0,"filter":{"type":"bp","cutoff":3200,"res":0.4},"amp":[0.0005,0.09,0.0,0.06]}},
  "clunk":{"synth":{"osc":[{"wave":"sine","level":1.0}],"noise":0.5,"filter":{"type":"lp","cutoff":900,"res":0.3},"pitch":[24,0.02],"amp":[0.0005,0.05,0.0,0.03]}},
  "acid":{"synth":{"osc":[{"wave":"saw"}],"filter":{"type":"ladder","cutoff":300,"res":0.7,"drive":2.5,"env":3.0},"fenv":[0.001,0.12,0.0,0.1],"amp":[0.001,0.18,0.6,0.05],"mono":True,"glide":0.03}}},
 "tracks":[
  {"name":"hum","inst":"hum","clips":[{"at":"@0","notes":"G1:33"}],"gain":-8,
   "automate":{"lp":[[0,200],[T_PULL/b,200],[(T_PULL+1.0)/b,1600],[T_LOCK/b,500]]}},
  {"name":"kick","inst":"drums:techno","clips":[{"at":f"@{BAR}","bars":5,"drums":{"kick":"X...x...x...x...","hat":"..x...x...x...x."}},
                                                {"at":f"@{T_PULL}","bars":1,"drums":{"kick":"X...............","crash":"X..............."}},
                                                {"at":f"@{T_LOCK}","bars":2,"drums":{"kick":"X...x...x...x...","hat":"x.xxx.xxx.xxx.xx","clap":"....x.......x..."}}],
   "fx":[{"type":"comp","threshold":-12,"ratio":4},{"type":"saturate","drive":0.5}]},
  {"name":"acid","inst":"acid","clips":[{"at":f"@{4*b*3}","bars":3,"notes":"o2 l16 (G1 G1 G2 G1 r G1 Bb1 G1 C2 G1 r G1 F2 G1 D2 G1)x3"},
                                        {"at":f"@{T_LOCK}","bars":2,"notes":"o2 l16 (G1 G1 G2 G1 r G1 Bb1 G1 C2 G1 r G1 F2 G1 D2 G1)x2"}],"gain":-9,
   "automate":{"synth.cutoff":[[0,250],[T_PULL/b,1800],[T_LOCK/b,600],[(T_LOCK+2*BAR)/b,2400]]},"duck":5},
  {"name":"arcs","inst":"arc","clips":[{"at":f"@{t-0.02:.4f}","notes":"C5/16 C5/32"} for t in T],"gain":-6},
  {"name":"clunks","inst":"clunk","clips":[{"at":f"@{t:.4f}","notes":"C3/16"} for t in T],"gain":-4}
 ],
 "sfx":[{"sfx":"boom","at":f"@{T_PULL:.4f}"},{"sfx":"riser","at":f"@{T_PULL:.4f}","gain":-6},{"sfx":"impact","at":f"@{T_LOCK:.4f}"}],
 "master":{"reverb":"hall"}}
json.dump(music,open('score.json','w'),indent=1)
