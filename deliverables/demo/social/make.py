#!/usr/bin/env python3
"""Halde & Sons, Ethiopia Guji (washed): a 15 s 9:16 Reel.

Idea: the morning light spells it out. One cup on an oak table; the low sun
prints each tasting note across the wood as a shadow, and the light itself
takes the note's colour: bergamot (pale citrus), peach, black tea (deep
amber). Four shots match-cut on the cup; each word is bigger, the camera
further back, the light lower and warmer. The last shadow is the coffee's
name. Sound comes from the table: porcelain, a spoon, the pour."""
import json, math, os
HERE = os.path.dirname(os.path.abspath(__file__))
SERIF = "Fraunces"

# Light direction (towards the sun): low, from the back left.
def sun(az, el):
    a, e = math.radians(az), math.radians(el)
    return [-math.cos(e) * math.cos(a), math.sin(e), -math.cos(e) * math.sin(a)]

def stencil(text, h, shadow_xz, d, height, t0, slide, weight=650, extra=None):
    """Flat type floating above the table, hidden from the camera: its shadow
    lands at `shadow_xz`. It slides in along x so the word drifts into place and settles."""
    k = height / d[1]
    x, z = shadow_xz[0] + d[0] * k, shadow_xz[1] + d[2] * k
    o = {"text": text, "font": SERIF, "weight": weight, "height": h, "depth": 0.004, "bevel": 0.0005, "align": "center",
         "rot": [-90, 0, 0], "camera": False, "material": {"base": "#222222", "roughness": 0.9},
         "keys": [{"t": t0, "pos": [x - slide, height, z]}, {"t": t0 + 1.4, "pos": [x + slide * 0.04, height, z], "ease": "outCubic"},
                  {"t": t0 + 1.9, "pos": [x, height, z], "ease": "inOutSine"}]}
    if extra:
        o.update(extra)
    return o

CUP = [
    # Saucer and cup: turned profiles, glazed porcelain.
    {"type": "lathe", "points": [[0.0, 0.0], [0.05, 0.0], [0.075, 0.008], [0.08, 0.013], [0.074, 0.012], [0.05, 0.006], [0.0, 0.006]], "segments": 96,
     "pos": [0, 0, 0], "material": {"base": "#F4F1EA", "roughness": 0.12, "clearcoat": 1.0}},
    {"type": "lathe", "points": [[0.0, 0.0065], [0.026, 0.0065], [0.03, 0.012], [0.04, 0.03], [0.047, 0.07], [0.049, 0.079], [0.046, 0.08], [0.044, 0.07], [0.037, 0.03], [0.028, 0.016], [0.0, 0.016]],
     "segments": 96, "pos": [0, 0, 0], "material": {"base": "#F4F1EA", "roughness": 0.1, "clearcoat": 1.0}},
    {"type": "torus", "radius": 0.016, "tube": 0.0035, "pos": [0.053, 0.05, 0], "rot": [90, 0, 0], "material": {"base": "#F4F1EA", "roughness": 0.1, "clearcoat": 1.0}},
    # The coffee: a dark mirror with a crema ring.
    {"type": "cylinder", "radius": 0.0438, "height": 0.001, "pos": [0, 0.067, 0], "material": {"base": "#140904", "roughness": 0.03, "clearcoat": 1.0, "specular": 1.3}},
    {"type": "torus", "radius": 0.0425, "tube": 0.0016, "pos": [0, 0.0675, 0], "material": {"base": "#9A5A2C", "roughness": 0.6}},
    {"type": "model", "src": "polyhaven:model/wooden_spoon", "height": 0.012, "scale": 1, "pos": [0.0, 0.009, 0.06], "rot": [0, 70, 0]},
]

def steam(seed):
    return {"type": "particles", "count": 2500, "emit": {"shape": "disc", "radius": 0.035, "pos": [0, 0.07, 0]},
            "flow": {"strength": 0.05, "scale": 0.05, "evolve": 0.4}, "gravity": -0.03, "drag": 0.6, "life": 2.5,
            "size": 0.0035, "color": ["#FFFFFF"], "opacity": 0.05, "blend": "over", "scatter": 0.8, "seed": seed}

TABLE = {"type": "box", "size": [3.0, 0.04, 2.0], "pos": [0, -0.02, 0], "material": {"texture": "polyhaven:texture/oak_veneer_01", "scale": 1}}

def cam(t, az, dist, el, focal, fs=4.0, tgt=(0.0, 0.03, 0.06)):
    a, e = math.radians(az), math.radians(el)
    return {"t": t, "pos": [tgt[0] + dist * math.cos(e) * math.sin(a), tgt[1] + dist * math.sin(e), tgt[2] + dist * math.cos(e) * math.cos(a)],
            "target": list(tgt), "focal": focal, "fstop": fs}

# Six 2.5 s shots, one note each, match-cut on the cup. Each word lands within
# 0.9 s and holds; the light takes the note's colour, the camera never repeats.
# (start, word, size, light colour, sun az/el, camera az0→az1, dist, elevation, focal, target, word centre)
SHOTS = [
    (0.0, "bergamot", 0.042, "#F7F08A", (200, 36), (-14, -6), 0.34, 60, 50, (0.0, 0.03, 0.07), (0.0, 0.12)),
    (2.5, "peach", 0.058, "#FF9C78", (206, 32), (8, 16), 0.26, 32, 50, (0.02, 0.01, 0.12), (0.03, 0.13)),
    (5.0, "black tea", 0.06, "#E2661E", (212, 30), (-20, -10), 0.5, 84, 45, (0.0, 0.0, 0.09), (0.0, 0.14)),
    (7.5, "washed", 0.05, "#BFD8FF", (196, 40), (26, 18), 0.36, 26, 55, (0.0, 0.02, 0.1), (0.0, 0.13)),
    (10.0, "Ethiopia", 0.06, "#FFD08A", (204, 30), (-8, 0), 0.42, 52, 50, (0.0, 0.02, 0.08), (0.0, 0.13)),
    (12.5, "Guji", 0.12, "#FFB45E", (214, 26), (4, 10), 0.62, 62, 45, (0.0, 0.02, 0.1), (0.0, 0.17)),
]
scenes = []
for i, (t0, word, h, col, (az, el), (c0, c1), dist, cel, focal, tgt, at) in enumerate(SHOTS):
    d = sun(az, el)
    dur = 2.5
    objs = [TABLE] + CUP
    props = {
        "env": {"hdri": "polyhaven:hdri/brown_photostudio_02", "rotate": 40, "exposure": -2.0, "background": "blur", "key": False},
        "lights": [{"dir": d, "size": 1.0, "color": col, "intensity": 20000, "key": True}],
        # The sun through a cut-out: the note is written in light across the table, sliding in and settling.
        "gobo": {"text": word, "font": SERIF, "weight": 600, "size": h, "at": list(at), "height": 0.3, "invert": True,
                 "strength": 0.9, "softness": 0.0016, "from": [-(0.25 + h * 2) * (1 if i % 2 == 0 else -1), 0.0], "start": 0.0, "settle": 0.9},
        "floor": False,
        "camera": {"keys": [cam(0, c0, dist * 1.03, cel, focal, 4.0, tgt), cam(dur, c1, dist, cel, focal, 4.0, tgt)]},
        "objects": objs,
    }
    scenes.append({"duration": dur, "transition": "cut", "layers": [{"component": "scene3d", "props": props}]})

# The brand, small and once, over the last shot's open wood.
scenes[-1]["layers"].append({"text": "Ethiopia Guji, washed. Out Friday.", "font": SERIF, "size": "4.4vw", "color": "#F3E7D3", "x": "50%", "y": "90%", "anchor": "center", "start": 0.9, "enter": "fadeUp"})
scenes[-1]["layers"].append({"text": "HALDE & SONS", "font": "Inter", "weight": 600, "size": "2vw", "letterSpacing": "0.3em", "color": "#F3E7D3B3", "x": "50%", "y": "8%", "anchor": "center", "start": 1.1, "enter": "fadeUp"})

spec = {
    "size": [1080, 1920], "fps": 30, "background": "#1A120C",
    "post": {"preset": "clean", "lut": {"base": "warm-negative", "fade": 0.02, "contrast": 0.25}, "lutMix": 0.45, "grain": 0.018, "vignette": 0.18},
    "sound": {"palette": "organic", "ambience": "cafe", "design": "off", "cues": [
        {"sfx": "ceramic", "at": 0.1}, {"sfx": "air", "at": 0.7}, {"sfx": "ceramic", "at": 2.5, "pitch": 2}, {"sfx": "air", "at": 3.2},
        {"sfx": "wood", "at": 5.0}, {"sfx": "air", "at": 5.7}, {"sfx": "cc0-foley/splash", "at": 7.5, "gain": -6}, {"sfx": "air", "at": 8.2},
        {"sfx": "ceramic", "at": 10.0, "pitch": -2}, {"sfx": "air", "at": 10.7}, {"sfx": "clink", "at": 12.5}, {"sfx": "swell", "at": 13.4, "gain": -6}]},
    "music": {"style": "acoustic", "bpm": 72, "key": "D"},
    "scenes": scenes,
}
json.dump(spec, open(os.path.join(HERE, "spec.json"), "w"), indent=1)
print("ok")
