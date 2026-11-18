#!/usr/bin/env python3
"""Builds spec.json for the Loam launch film (pass 1). Run: python3 src/build.py"""
import json, sys
B = 0.625          # one beat at 96 bpm
WALL = {"texture": "polyhaven:texture/beige_wall_001", "color": "#F2FAEC"}
SILL = {"color": "#ECE8DF", "roughness": 0.55}
FONT = "Archivo Black"
SUCC = "polyhaven:model/potted_plant_04"
ARROW = "polyhaven:model/potted_plant_02"
PED = "polyhaven:model/potted_plant_01"

def sill_objects(extra=()):
    o = [
        {"type": "box", "name": "wall", "size": [6, 3, 0.05], "pos": [0, 1.5, -0.4], "material": WALL},
        {"type": "model", "name": "succ", "src": SUCC, "pos": [0.5, 0, -0.12]},
        {"type": "model", "name": "arrow", "src": ARROW, "pos": [0.0, 0, -0.08], "scale": 0.55},
        {"type": "model", "name": "ped", "src": PED, "pos": [-0.62, 0, -0.1], "scale": 0.5},
    ]
    return o + list(extra)

SENSOR = [  # a small white clip-stake: stem in the soil, a rounded head above it
    {"type": "cylinder", "name": "stem", "radius": 0.0045, "height": 0.07, "pos": [0.548, 0.085, -0.075],
     "material": {"extends": "plastic", "base": "#F2F0EA", "roughness": 0.35}},
    {"type": "lathe", "name": "sensor", "points": [[0, 0], [0.016, 0.0], [0.019, 0.006], [0.017, 0.014], [0.008, 0.018], [0, 0.018]],
     "pos": [0.548, 0.12, -0.075], "material": {"extends": "plastic", "base": "#F4F2EC", "roughness": 0.3, "imperfections": 0.2}},
    {"type": "cylinder", "name": "dot", "radius": 0.0022, "height": 0.001, "pos": [0.548, 0.1385, -0.075],
     "material": {"base": "#6E8F5E", "roughness": 0.4}},
]

def shot(dur, cam, word, at, size, key=1.5, start=0.05, settle=0.5, frm=(0.25, 0), extra=(), fstop=4, warm=0.35, rig_extra=None, gobo=True):
    rig = {"preset": "window", "angle": 60, "height": 20, "warmth": warm, "ratio": 8, "key": key, "softness": 0.01}
    if rig_extra: rig.update(rig_extra)
    props = {"resolution": 0.7, "env": {"preset": "interior"}, "rig": rig, "floor": SILL, "camera": cam, "objects": sill_objects(extra)}
    if gobo and word:
        props["gobo"] = {"text": word, "font": FONT, "size": size, "at": at, "height": 0.6, "strength": 0.92,
                         "softness": 0.003, "from": list(frm), "start": start, "settle": settle}
    return {"duration": dur, "background": "#B4C4AC", "layers": [{"component": "scene3d", "props": props}]}

def cam(keys, lens="portrait85", shake="breath"):
    return {"lens": lens, "shake": shake, "keys": keys}

scenes = [
    # 1 hook: the succulent, eye level, a sentence of shadow slides in beside it
    shot(3 * B, cam([{"t": 0, "pos": [0.78, 0.30, 0.95], "target": [0.55, 0.06, -0.02], "focus": "succ", "fstop": 4},
                     {"t": 3 * B, "pos": [0.74, 0.28, 0.85], "target": [0.55, 0.06, -0.02], "focus": "succ", "fstop": 4}]),
         "Not yet.", [0.6, 0.1], 0.05, start=0.0, settle=0.9, frm=(0.6, 0.0)),
    # 2 the thirsty arrowhead plant, low: "Now." bigger than the pot
    shot(3 * B, cam([{"t": 0, "pos": [-0.05, 0.22, 1.15], "target": [0.0, 0.16, 0], "focus": "arrow", "fstop": 3.2},
                     {"t": 3 * B, "pos": [-0.02, 0.24, 1.05], "target": [0.0, 0.16, 0], "focus": "arrow", "fstop": 3.2}]),
         "Now.", [-0.03, 0.22], 0.08, start=0.0, settle=0.35, frm=(0.0, 0.0)),
    # 3 the cause: the sensor clipped into the succulent's gravel, macro from above; the amount
    shot(4 * B, cam([{"t": 0, "pos": [0.70, 0.40, 0.42], "target": [0.56, 0.09, -0.02], "focus": "sensor", "fstop": 4},
                     {"t": 4 * B, "pos": [0.68, 0.36, 0.36], "target": [0.56, 0.09, -0.02], "focus": "sensor", "fstop": 4}], lens="macro100"),
         "Half a cup.", [0.565, 0.035], 0.026, start=0.5, settle=0.6, frm=(0.3, 0.0), extra=SENSOR),
    # 4 the pedestal plant, a lateral track: "Sunday."
    shot(2 * B, cam([{"t": 0, "pos": [-0.35, 0.45, 1.3], "target": [-0.6, 0.25, -0.1], "focus": "ped", "fstop": 4},
                     {"t": 2 * B, "pos": [-0.5, 0.45, 1.3], "target": [-0.62, 0.25, -0.1], "focus": "ped", "fstop": 4}]),
         "Sunday.", [-0.55, 0.22], 0.07, start=0.0, settle=0.3, frm=(0.0, 0.0), extra=SENSOR),
    # 5 the arrowhead from above: "A splash."
    shot(2 * B, cam([{"t": 0, "pos": [0.05, 1.05, 0.55], "target": [0.0, 0.0, 0.08], "focus": "arrow", "fstop": 5.6},
                     {"t": 2 * B, "pos": [0.05, 0.98, 0.5], "target": [0.0, 0.0, 0.08], "focus": "arrow", "fstop": 5.6}], lens="normal50"),
         "A splash.", [0.05, 0.27], 0.09, start=0.0, settle=0.3, frm=(0.0, 0.0), extra=SENSOR),
    # 6 hard stop: a cloud; the sun goes, the words go, silence
    shot(2 * B, cam([{"t": 0, "pos": [0.0, 1.5, 1.6], "target": [-0.05, 0.0, 0.1], "fstop": 8},
                     {"t": 2 * B, "pos": [0.0, 1.46, 1.55], "target": [-0.05, 0.0, 0.1], "fstop": 8}], lens="normal50"),
         None, None, None, key=0.04, extra=SENSOR, gobo=False),
    # 7 payoff: the sun returns over the whole sill and writes the name
    shot(8 * B, cam([{"t": 0, "pos": [0.0, 1.5, 1.6], "target": [-0.05, 0.0, 0.1], "fstop": 8},
                     {"t": 8 * B, "pos": [0.0, 1.32, 1.4], "target": [-0.05, 0.0, 0.12], "fstop": 8}], lens="normal50"),
         "loam", [-0.04, 0.36], 0.26, start=0.9, settle=1.0, frm=(0.9, 0.0), extra=SENSOR),
]
scenes[-1]["layers"].append({"text": "Out now.", "font": FONT, "size": "3.4vh", "x": "50%", "y": "78%", "anchor": "center",
                             "color": "#4F5A49", "start": 3.0, "enter": "fadeUp"})
spec = {
    "size": "9:16", "fps": 30,
    "music": {"style": "acoustic", "bpm": 96, "key": "D", "mood": "warm bright",
              "sections": [{"name": "a", "bars": 3, "energy": 0.55}, {"name": "b", "bars": 1, "energy": 0.8}, {"name": "c", "bars": 2, "energy": 0.9, "stop": 2}],
              "hits": ["@bar5"], "end": "ring"},
    "sound": "auto",
    "scenes": scenes,
}
json.dump(spec, open(sys.argv[1] if len(sys.argv) > 1 else "spec.json", "w"), indent=1)
