import json, sys
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from world import *

def hang_objects():
    o = pan_parts()
    o.append(dict(pancake([0.0, 0.30, 0.02]), rot=[18, 0, -24]))
    for p in egg([0.09, 0.42, -0.06]):
        p["rot"] = [-30, 20, 15]; o.append(p)
    o.append(dict(tomato([-0.10, 0.22, 0.05]), rot=[40, 0, 20]))
    o.append(dict(mushroom([0.05, 0.16, 0.1]), rot=[0, 0, 70]))
    o.append(dict(toast([-0.06, 0.52, -0.08]), rot=[25, 30, -35]))
    return o

CAM = {"keys": [{"t": 0, "pos": [0.62, 0.42, 0.78], "target": [0, 0.24, 0], "focal": 40, "fstop": 5.6}]}
variants = [
    {"floor": {"texture": "ambientcg:Concrete034", "scale": 0.8}, "env": {"preset": "studio"},
     "rig": {"preset": "window", "angle": 60, "height": 35, "warmth": 0.15, "ratio": 6, "key": 1.5, "gobo": "window"}},
    {"set": {"preset": "sweep", "color": "#C4532E"}, "env": {"preset": "cyc"},
     "rig": {"preset": "product", "angle": 50, "height": 55, "warmth": 0.35, "ratio": 5, "key": 1.6}},
    {"floor": {"texture": "ambientcg:Wood095", "scale": 0.7}, "env": {"preset": "studio"},
     "rig": {"preset": "golden", "angle": 70, "height": 20, "warmth": 0.8, "ratio": 8, "gobo": "leaves"}},
]
scenes = []
for v in variants:
    s3 = dict(v, camera=CAM, objects=hang_objects())
    scenes.append({"duration": 1, "layers": [{"component": "scene3d", "props": s3}]})
json.dump({"size": "1:1", "fps": 30, "scenes": scenes}, open(sys.argv[1], "w"), indent=1)
