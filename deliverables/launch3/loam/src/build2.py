#!/usr/bin/env python3
"""Pass 2 of the Loam launch film: indoor weather. Run: python3 src/build2.py spec.json"""
import json, sys, copy
B = 0.625  # one beat at 96 bpm
WALL = {"texture": "polyhaven:texture/beige_wall_001", "color": "#8FB5AA"}
SILL = {"color": "#EEEAE2", "roughness": 0.5}
FONT = "Bricolage Grotesque"
ARROW_POS = [-0.05, 0, -0.08]
SOIL_Y = 0.165  # top of the arrowhead pot's soil (scale 0.55)

def plants():
    return [
        {"type": "box", "name": "wall", "size": [6, 3, 0.05], "pos": [0, 1.5, -0.4], "material": WALL},
        {"type": "model", "name": "succ", "src": "polyhaven:model/potted_plant_04", "pos": [0.42, 0, -0.05]},
        {"type": "model", "name": "arrow", "src": "polyhaven:model/potted_plant_02", "pos": ARROW_POS, "scale": 0.55},
        {"type": "model", "name": "ped", "src": "polyhaven:model/potted_plant_01", "pos": [-0.6, 0, -0.1], "scale": 0.5},
    ]

SENSOR = [  # a slim white stake with a smooth pebble head, pushed into the arrowhead's soil near the rim
    {"type": "cylinder", "name": "stem", "radius": 0.004, "height": 0.06, "pos": [-0.07, SOIL_Y - 0.005, 0.035],
     "material": {"extends": "plastic", "base": "#F2F0EA", "roughness": 0.3}},
    {"type": "lathe", "name": "sensor", "segments": 64,
     "points": [[0, 0], [0.012, 0.0005], [0.0175, 0.004], [0.019, 0.009], [0.0165, 0.0145], [0.009, 0.0175], [0, 0.018]],
     "pos": [-0.07, SOIL_Y + 0.025, 0.035], "material": {"extends": "plastic", "base": "#F5F3EE", "roughness": 0.22, "imperfections": 0.15}},
]

def rain(center, radius, start, stop, top=1.0, count=2200, size=0.0011, speed=2.2, floor_y=SOIL_Y, name="rain", opacity=0.75):
    """Rain over one pot: born only between `start` and `stop` (scene time), hidden below the soil line."""
    life = 0.6
    born = f"(t - age*{life})"
    return {"type": "particles", "name": name, "count": count, "life": life, "seed": 3, "start": start - life,
            "emit": {"shape": "disc", "pos": [center[0], top, center[1]], "radius": radius},
            "velocity": [0, -speed, 0], "gravity": -1.0, "drag": 0.0, "size": size, "stretch": 3.5,
            "color": "#E4F0F5", "blend": "over", "lit": True, "glint": {"amount": 2, "sharpness": 80},
            "opacity": f"=({born} > {start} && {born} < {stop} && y > {floor_y}) ? {opacity} : 0"}

def rig(key=1.6):
    return {"preset": "window", "angle": 60, "height": 24, "warmth": 0.4, "ratio": 8, "key": key, "softness": 0.01}

def shot(dur, keys, objs, lens="portrait85", key=1.6, extra_props=None, angle=60):
    r = rig(key); r["angle"] = angle
    props = {"resolution": 0.7, "env": {"preset": "interior"}, "rig": r, "floor": SILL,
             "camera": {"lens": lens, "shake": "breath", "keys": keys}, "objects": plants() + objs}
    if extra_props: props.update(extra_props)
    return {"duration": dur, "background": "#6F8F86", "layers": [{"component": "scene3d", "props": props}]}

AC = [ARROW_POS[0], ARROW_POS[2]]
scenes = [
    # 1 hook: rain indoors, on one pot, in window sun
    shot(3 * B, [{"t": 0, "pos": [0.05, 0.5, 1.55], "target": [-0.05, 0.22, 0], "focus": "arrow", "fstop": 5.6},
                 {"t": 3 * B, "pos": [0.03, 0.46, 1.38], "target": [-0.05, 0.22, 0], "focus": "arrow", "fstop": 5.6}],
         [rain(AC, 0.1, -2, 99)] + SENSOR),
    # 2 the succulent next door: dry, in the sun; the rain a blur behind it
    shot(3 * B, [{"t": 0, "pos": [0.95, 0.2, 0.75], "target": [0.25, 0.16, -0.1], "focus": "succ", "fstop": 3.2},
                 {"t": 3 * B, "pos": [0.9, 0.19, 0.7], "target": [0.25, 0.16, -0.1], "focus": "succ", "fstop": 3.2}],
         [rain(AC, 0.1, -2, 99)] + SENSOR, angle=25),
    # 3 the cause: the sensor in the wet soil; the shower thins and stops, exactly
    shot(4 * B, [{"t": 0, "pos": [0.02, 0.30, 0.42], "target": [-0.07, SOIL_Y + 0.03, 0.035], "focus": "sensor", "fstop": 5.6},
                 {"t": 4 * B, "pos": [0.01, 0.28, 0.37], "target": [-0.07, SOIL_Y + 0.03, 0.035], "focus": "sensor", "fstop": 5.6}],
         [rain(AC, 0.1, -2, 1.3)] + SENSOR, lens="macro100"),
    # 4 a different weather: a short fine drizzle on the pedestal plant
    shot(2 * B, [{"t": 0, "pos": [-0.3, 0.55, 1.35], "target": [-0.6, 0.3, -0.1], "focus": "ped", "fstop": 4},
                 {"t": 2 * B, "pos": [-0.44, 0.55, 1.35], "target": [-0.62, 0.3, -0.1], "focus": "ped", "fstop": 4}],
         [rain([-0.6, -0.1], 0.13, -1, 99, top=1.2, count=5000, size=0.0007, speed=1.4, floor_y=0.3, name="drizzle", opacity=0.6)] + SENSOR),
    # 5 the whole sill from above: one pot drizzling, the others in the sun
    shot(2 * B, [{"t": 0, "pos": [0.35, 0.95, 1.9], "target": [-0.1, 0.18, -0.05], "fstop": 8},
                 {"t": 2 * B, "pos": [0.25, 0.92, 1.85], "target": [-0.1, 0.18, -0.05], "fstop": 8}],
         [rain([-0.6, -0.1], 0.13, -2, 99, top=1.2, count=5000, size=0.0009, speed=1.4, floor_y=0.3, name="drizzle", opacity=0.7)] + SENSOR, lens="normal50"),
    # 6 hard stop: the rain is gone; leaves, wet, still; silence
    shot(2 * B, [{"t": 0, "pos": [-0.2, 0.2, 0.42], "target": [-0.07, 0.19, 0.035], "focus": "sensor", "fstop": 4},
                 {"t": 2 * B, "pos": [-0.195, 0.2, 0.40], "target": [-0.07, 0.19, 0.035], "focus": "sensor", "fstop": 4}],
         SENSOR, lens="macro100"),
    # 7 payoff: drops gather in the sun above the sill into the name, then the line
    shot(8 * B, [{"t": 0, "pos": [-0.05, 0.42, 1.75], "target": [-0.08, 0.38, 0], "fstop": 8},
                 {"t": 8 * B, "pos": [-0.05, 0.42, 1.55], "target": [-0.08, 0.38, 0], "fstop": 8}],
         SENSOR + [
             {"type": "particles", "name": "name", "count": 60000, "seed": 7,
              "emit": {"shape": "box", "pos": [-0.08, 0.6, 0.15], "size": [1.0, 0.9, 0.9]},
              "velocity": [0, -0.05, 0], "spread": 0.02, "drag": 0.6,
              "flow": {"strength": 0.02, "scale": 0.05, "evolve": 0.3},
              "target": {"text": "loam", "font": FONT, "weight": 800, "height": 0.16, "depth": 0.01, "pos": [-0.08, 0.6, 0.12], "from": 0.15, "to": 1.6},
              "size": 0.0011, "color": "#EAF4F8", "blend": "over", "lit": True, "glint": {"amount": 1.5, "sharpness": 60}, "opacity": "=0.06 + 0.89*clamp((t - 1.35)/0.35, 0, 1)"},
             {"type": "particles", "name": "line", "count": 18000, "seed": 9,
              "emit": {"shape": "box", "pos": [-0.08, 0.5, 0.05], "size": [0.4, 0.2, 0.2]},
              "velocity": [0, -0.05, 0], "spread": 0.02, "drag": 0.6, "start": 1.6,
              "target": {"text": "Out now.", "font": FONT, "weight": 700, "height": 0.035, "depth": 0.004, "pos": [-0.08, 0.48, 0.12], "from": 1.7, "to": 2.3},
              "size": 0.0006, "color": "#EAF4F8", "blend": "over", "lit": True, "opacity": "=0.95*clamp((t - 2.3)/0.3, 0, 1)"},
         ]),
]
spec = {
    "size": "9:16", "fps": 30,
    "music": {"style": "acoustic", "bpm": 96, "key": "D", "mood": "warm bright",
              "sections": [{"name": "a", "bars": 3, "energy": 0.55}, {"name": "b", "bars": 1, "energy": 0.8}, {"name": "c", "bars": 2, "energy": 0.9, "stop": 2}],
              "hits": ["@bar5"], "end": "ring"},
    "sound": "auto",
    "scenes": scenes,
}
json.dump(spec, open(sys.argv[1] if len(sys.argv) > 1 else "spec.json", "w"), indent=1)
