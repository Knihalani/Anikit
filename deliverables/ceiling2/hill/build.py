#!/usr/bin/env python3
"""Assemble the shot shaders (common.glsl + shot) and write hill.json.

The engine has no #include for GLSL, so the shared toolkit is prepended here.
"""
import json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
G = os.path.join(HERE, "glsl")
B = os.path.join(HERE, "build")
os.makedirs(B, exist_ok=True)
common = open(os.path.join(G, "common.glsl")).read()

def shot(name):
    src = common + "\n" + open(os.path.join(G, name + ".glsl")).read()
    out = os.path.join(B, name + ".glsl")
    open(out, "w").write(src)
    return "build/" + name + ".glsl"

ONLY = sys.argv[1] if len(sys.argv) > 1 else None

scenes = []

def add(name, dur, uniforms, animate, extra_layers=(), transition=None):
    sc = {"duration": dur, "layers": [{"src": shot(name), "uniforms": uniforms, "animate": animate}] + list(extra_layers)}
    if transition:
        sc["transition"] = transition
    scenes.append((name, sc))

add("s1_valley", 3.0,
    {"camx": -0.10, "zoom": 1.0, "walk": 0.0},
    [{"to": {"camx": 0.05, "zoom": 1.06}, "duration": 3.0, "ease": "inOutSine"},
     {"to": {"walk": 3.6}, "duration": 3.0, "ease": "linear"}])

add("s2_lantern", 2.6,
    {"camx": 0.0, "walk": 0.0},
    [{"to": {"camx": 0.52}, "duration": 2.6, "ease": "linear"},
     {"to": {"walk": 3.12}, "duration": 2.6, "ease": "linear"}])

add("s3_eye", 2.9,
    {"camx": 0.05, "camy": -0.02, "zoom": 1.0, "walk": 0.0, "eye": 0.0},
    [{"to": {"zoom": 1.12, "camx": 0.0, "camy": 0.02}, "duration": 2.9, "ease": "inOutSine"},
     {"keyframes": {"eye": [[0, 0], [0.10, 0.07, "inOutSine"], [0.15, 0.0, "inSine"], [0.30, 0.0], [0.42, 1.15, "outCubic"], [0.52, 1.0, "inOutSine"], [1, 1.0]]}, "duration": 2.9, "ease": "linear"}])

TITLE = [
    # each word lands on its own beat after the head settles, the face softening from sharp to round as it arrives
    {"text": w, "font": "Fraunces", "size": "10vh", "color": "#ECE8FF", "letterSpacing": "-0.03em",
     "axes": {"wght": 560, "SOFT": 0, "WONK": 1, "opsz": 144}, "italic": True,
     "x": x, "y": "15%", "anchor": "left", "start": 1.55 + 0.22*i,
     "animate": [{"from": {"opacity": 0, "y": "+40", "axes.SOFT": 0, "axes.wght": 300}, "to": {"axes.SOFT": 100, "axes.wght": 560},
                  "duration": 0.9, "ease": "outCubic"},
                 {"to": {"letterSpacing": 2}, "at": 0.9, "duration": 1.0, "ease": "linear"}]}
    for i, (w, x) in enumerate([("The", "6%"), ("Hill", "15.2%"), ("That", "25.2%"), ("Woke", "36.0%")])
] + [
    {"text": "a story told by the snow", "font": "Caveat", "size": "3.6vh", "color": "#B9BDE8", "x": "6.3%", "y": "25.5%",
     "anchor": "left", "start": 2.5, "animate": [{"from": {"opacity": 0, "x": "-20"}, "duration": 0.7, "ease": "outCubic"}]}
]
add("s4_wake", 3.4,
    {"camx": 0.10, "camy": 0.0, "zoom": 1.18, "walk": 0.0, "wake": 0.0},
    [{"to": {"zoom": 0.98, "camx": 0.0}, "duration": 3.4, "ease": "outCubic"},
     {"keyframes": {"wake": [[0, 0], [0.10, 0.0], [0.44, 1.0, "inOutCubic"], [1, 1.0]]}, "duration": 3.4, "ease": "linear"}],
    extra_layers=TITLE)

# ---- score: celesta lullaby, held breath, timpani on the eye, D major bloom when it wakes
MUSIC = {
  "bpm": 72, "key": "Dm", "length": "13.4s", "seed": 3, "end": "ring",
  "master": {"reverb": "hall"},
  "define": {
    "wind": {"synth": {"osc": [{"wave": "sine", "level": 0.0}], "noise": 1.0,
                        "filter": {"type": "bp", "cutoff": 700, "res": 0.35}, "amp": [1.5, 0.5, 1.0, 2.0],
                        "lfo": {"rate": 0.23, "cutoff": 0.6}}},
    "crunch": {"synth": {"osc": [{"wave": "sine", "level": 0.0}], "noise": 1.0,
                          "filter": {"type": "bp", "cutoff": 2600, "res": 0.2}, "amp": [0.002, 0.09, 0.0, 0.06]}},
    "growl": {"synth": {"osc": [{"wave": "saw", "voices": 3, "detune": 18}], "noise": 0.25,
                         "filter": {"type": "ladder", "cutoff": 160, "res": 0.3, "drive": 2.0},
                         "amp": [0.6, 0.6, 0.9, 1.8], "lfo": {"rate": 3.5, "trem": 0.25}}}
  },
  "tracks": [
    {"inst": "wind", "notes": "o3 l1 C3 _ _ _", "gain": -17, "pan": -0.2,
     "automate": {"lp": [[0, 900], [8, 600], [12, 2400]]}},
    {"inst": "crunch", "notes": "l8 v38 (C4)x7 v96 (C4)x6 r", "gain": -9, "pan": 0.1},
    {"inst": "celesta", "notes": "o5 l8 r/4 A5 D6 E6/4. F6 E6 D6/4 r/8 | A5/4 C6 D6 A5/2 r/4 | r/1", "gain": -5, "reverb": 0.55, "pan": 0.15},
    {"inst": "harp", "notes": "o3 l16 (D3 A3 D4 F4 A4 F4 D4 A3)x2 (Bb2 F3 Bb3 D4 F4 D4 Bb3 F3)x2 | r/1", "gain": -10, "reverb": 0.5, "pan": -0.25},
    {"inst": "strings", "notes": "o3 l1 [D3 A3 E4 F4] | [Bb2 F3 D4 A4] | [D2 A2 D3]/2 r/2 | [D3 A3 D4 F#4 A4 D5] _ |", "gain": -9, "reverb": 0.5,
     "automate": {"volume": [[0, -6], [4, -3], [7.5, -14], [8, 0], [10, -2], [12, 3]]}},
    {"inst": "cello", "notes": "r/1 | r/1 | D2/1 | D2/1 _ |", "gain": -6, "reverb": 0.4},
    {"inst": "timpani", "notes": "r/1 | r/2 r/4 D2/16? D2/16? D2/16? D2/16? | D2/2! r/2 | r/2 D2/8 D2/8 D2/8 D2/8 | D2/1! |", "gain": -4},
    {"inst": "growl", "notes": "r/1 | r/1 | o1 D1/1 | D1/2 r/2 |", "gain": -12},
    {"inst": "choir", "notes": "r/1 | r/1 | r/1 | [D4 F#4 A4 D5]/1 _ |", "gain": -6, "reverb": 0.6,
     "automate": {"volume": [[12, -14], [13.5, 0]]}},
    {"inst": "french-horn", "notes": "r/1 | r/1 | r/1 | o4 r/8 A4/8 D5/4. E5/8 F#5/2 |", "gain": -5, "reverb": 0.5}
  ]
}

spec = {
    "size": [1920, 1080],
    "fps": 24,
    "transition": "cut",
    "background": "#0e112e",
    "intent": {"pace": "steady", "feeling": "hushed wonder, then awe"},
    "music": MUSIC,
    "scenes": [s for n, s in scenes if (ONLY is None or n == ONLY)],
}
json.dump(spec, open(os.path.join(HERE, "hill.json"), "w"), indent=1)
print("wrote hill.json with", len(spec["scenes"]), "scenes")
