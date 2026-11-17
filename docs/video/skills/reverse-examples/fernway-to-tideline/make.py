#!/usr/bin/env python3
"""Tideline (fictional): a recreation of the Fernway reference's structure and style (same shot list, beat timing, camera language,
serif/sans/mono type system, amber glowing route motif, chart-paper inserts, grade) for a different product: offline sea charts."""
import json
B = 0.46875  # one beat at 128 BPM
MOSS, LICHEN, AMBER, FOG = "#0B1622", "#DCE6EC", "#FFB23F", "#5E6E78"
SERIF, SANS, MONO = "Instrument Serif", "Inter Display", "JetBrains Mono"

TERRAIN = {"type": "heightfield", "expr": "=0.22*sin(x*0.8 + z*0.3) + 0.12*sin(x*1.9 - z*1.3) + 0.05*sin(x*6 + z*4)",
           "size": [14, 14], "res": 160, "material": {"base": "#7F9CB3", "roughness": 0.22}}
# The motif: an amber trail of glowing beads that draws itself across every shot.
def trail(n=220, speed=1.0, lead=0.0):
    x = "(-5 + 10*i/n)"
    z = "(1.6*sin(i/n*7.5))"
    y = f"(0.22*sin({x}*0.8 + {z}*0.3) + 0.12*sin({x}*1.9 - {z}*1.3) + 0.05*sin({x}*6 + {z}*4) + 0.06)"
    return {"type": "sphere", "radius": 0.035, "count": n, "pos": f"=[{x}, {y}, {z}]",
            "scale": "=[" + ", ".join([f"(i/n < {lead} + t*{speed} ? 1 : 0)"] * 3) + "]", "material": {"base": AMBER, "emissive": AMBER, "emissiveStrength": 6}}

def s3d(props):
    base = {"quality": "draft", "resolution": 0.5, "bloom": 0.45, "exposure": 1.3,
            "env": {"preset": "night"}, "fog": {"density": 0.04, "color": "#18222A"}, "floor": {"y": -0.9, "color": "#121C26", "roughness": 0.9},
            "lights": [{"dir": [-0.85, 0.2, -0.45], "size": [14, 5], "color": "#FFC27A", "intensity": 22, "key": True}, {"dir": [0.6, 0.5, 0.6], "size": [40, 20], "color": "#6F8BB0", "intensity": 2.5}]}
    base.update(props)
    return {"component": "scene3d", "props": base}

def cam(keys):
    return {"keys": [{"t": t, "pos": p, "target": g, "focal": f, "fstop": 4} for t, p, g, f in keys]}

def slam(text, bg, fg, dur, rot=-3, font=SANS, size="24vw", y="50%"):
    t = {"text": text, "font": font, "size": size, "color": fg, "x": "50%", "y": y, "anchor": "center", "letterSpacing": "-0.04em",
         "animate": [{"from": {"scale": 1.9, "opacity": 0, "blur": 14}, "duration": 0.12, "ease": "outExpo"}]}
    if font == SANS:
        t["weight"] = 900
    return {"duration": dur, "transition": "cut", "background": bg,
            "camera": {"animate": [{"from": {"zoom": 1.18, "rotation": rot}, "to": {"zoom": 1, "rotation": 0}, "duration": 0.2, "ease": "outExpo"}],
                       "shake": {"amount": 7, "at": 0, "duration": 0.15}}, "layers": [t]}

def word3d(word, mat, pale, cam_keys, extra=(), rot_from=-25, layers=()):
    """Type inside the world: an extruded word standing on the ridge, snapped in on the beat."""
    props = {"camera": cam(cam_keys), "objects": [TERRAIN, {"text": word, "height": 0.5, "depth": 0.16, "bevel": 0.015, "font": SERIF,
             "material": mat, "pos": [0, 0.62, 0.9], "keys": [{"t": 0, "rot": [rot_from, 0, 0], "scale": 1.3, "ease": "outExpo"}, {"t": 0.22, "rot": [0, 0, 0], "scale": 1}]},
             *extra]}
    props["resolution"] = 1
    if pale:
        props["fog"] = {"density": 0.09, "color": "#A7B6C2"}
    return {"duration": 2 * B, "transition": "cut", "camera": {"shake": {"amount": 7, "at": 0, "duration": 0.15}}, "layers": [s3d(props), *layers]}
WCAM = [(0, [0, 0.95, 3.9], [0, 0.85, 0.9], 40), (2 * B, [0, 0.9, 3.7], [0, 0.85, 0.9], 41)]
EMIT = {"base": AMBER, "emissive": AMBER, "emissiveStrength": 4}
INKMAT = {"base": MOSS, "roughness": 0.4}

# The product's screen: a topographic map with the trail and live position.
def map_layers(progress_expr):
    L = [{"shape": "rect", "width": 720, "height": 1560, "fill": "#E6EEF2", "x": 0, "y": 0, "anchor": "topLeft"}]
    for k in range(14):  # contour lines
        L.append({"shape": "path", "d": f"M-40 {120 + k*105} C 200 {40 + k*105}, 420 {220 + k*105}, 780 {90 + k*105}",
                  "stroke": "#8FA9BC", "strokeWidth": 4, "fill": "none", "x": -40, "y": 100 + k*105, "anchor": "topLeft"})
    L += [{"shape": "path", "d": "M120 1400 C 80 1150, 520 1050, 430 820 S 160 520, 360 300 S 600 160, 560 110",
           "stroke": AMBER, "strokeWidth": 14, "fill": "none", "x": 110, "y": 110, "anchor": "topLeft",
           "trimEnd": progress_expr},
          *[{"text": w, "font": MONO, "size": 22, "color": "#4E6272", "x": x, "y": y, "anchor": "topLeft"} for w, x, y in
            [("12 m", 470, 300), ("BUOY", 300, 820), ("8 m", 80, 640), ("REEF", 520, 1080), ("4 m", 260, 1240), ("SHOAL", 90, 420)]],
          {"shape": "circle", "width": 44, "height": 44, "fill": AMBER, "stroke": "#FFFFFF", "strokeWidth": 6, "x": 430, "y": 820},
          {"shape": "rect", "width": 720, "height": 200, "fill": MOSS, "x": 0, "y": 0, "anchor": "topLeft"},
          {"text": "North Channel", "font": SERIF, "size": 64, "color": LICHEN, "x": 40, "y": 50, "anchor": "topLeft"},
          {"text": "OFFLINE · 6.4 NM · NEXT MARK 0.3 NM", "font": MONO, "size": 22, "color": AMBER, "x": 40, "y": 140, "anchor": "topLeft"},
          {"shape": "rect", "width": 640, "height": 150, "fill": MOSS, "radius": 28, "x": 40, "y": 1370, "anchor": "topLeft"},
          {"text": "Keep the red buoy to port", "font": SANS, "size": 40, "weight": 700, "color": LICHEN, "x": 80, "y": 1410, "anchor": "topLeft"},
          {"text": "Mark 3 of 7", "font": MONO, "size": 24, "color": AMBER, "x": 80, "y": 1468, "anchor": "topLeft"}]
    return L

phone = [{"type": "box", "size": [0.8, 1.66, 0.07], "pos": [0, 0.95, 0], "material": "aluminium"},
         {"type": "pane", "size": [0.72, 1.56, 0.01], "pos": [0, 0.95, 0.04], "texture": "#map", "material": {"base": "#FFFFFF", "glow": 0.3, "roughness": 0.3}}]

scenes = []
scenes.append({"id": "hook", "duration": 2 * B, "layers": [s3d({
    "camera": cam([(0, [-5.5, 1.2, 3.5], [-2, 0.3, 0], 30), (2 * B, [-4.6, 1.0, 3.0], [-1.5, 0.3, 0], 32)]),
    "objects": [TERRAIN, trail(speed=0.5)]})]})
scenes.append(word3d("No coverage.", INKMAT, True, WCAM))
scenes.append(word3d("No chart.", EMIT, False, [(0, [0.3, 0.8, 3.8], [0, 0.85, 0.9], 40), (2 * B, [0.15, 0.78, 3.6], [0, 0.85, 0.9], 41)], rot_from=25))
scenes.append({"id": "lost", "duration": 2 * B, "transition": "cut", "camera": {"shake": {"amount": 10, "at": 0, "duration": 0.9}}, "layers": [s3d({
    "fog": {"density": 0.09, "color": "#A7B6C2"},
    "camera": cam([(0, [-4.6, 2.4, 3.0], [-2, 0.3, 0], 24), (2 * B, [-4.0, 1.9, 2.4], [-1.8, 0.3, 0], 28)]),
    "objects": [TERRAIN, trail(lead=0.25, speed=-0.2)]}),
    {"text": "Which way?", "font": SERIF, "size": "15vw", "color": LICHEN, "x": "50%", "y": "28%", "anchor": "center", "enter": "mask by line up"}]})
scenes.append({"id": "reveal", "duration": 4 * B, "transition": "cut", "camera": {"shake": {"amount": 9, "at": 0, "duration": 0.2}}, "layers": [s3d({
    "camera": cam([(0, [-0.9, 0.7, 2.6], [0, 1.0, 0], 40), (4 * B, [0.6, 1.3, 2.3], [0, 0.95, 0], 44)]),
    "objects": [TERRAIN, trail(lead=0.3, speed=0.2)] + phone}),
    ]})
# Feature 1: works offline — the map itself, the trail drawing on, then the same map on the phone
scenes.append({"id": "f1", "duration": 2 * B, "transition": "cut", "background": "#E6EEF2",
               "camera": {"animate": [{"from": {"zoom": 1.3}, "to": {"zoom": 1.05}, "duration": 2 * B, "ease": "outExpo"}]}, "layers": [
    {"composition": "map", "width": 1080, "height": 2340, "x": "50%", "y": "55%"},
    {"layers": [{"shape": "rect", "width": 760, "height": 190, "fill": MOSS, "x": 0, "y": 0, "anchor": "topLeft"},
                {"text": "Charts offline.", "font": SANS, "size": 84, "weight": 900, "color": LICHEN, "x": 44, "y": 44, "anchor": "topLeft"}],
     "x": "50%", "y": "82%", "anchor": "center", "animate": [{"from": {"scaleX": 0}, "duration": 0.18, "ease": "outExpo"}]}]})
scenes.append({"id": "f1b", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "camera": {"keys": [{"t": 0, "pos": [0.5, 1.3, 1.3], "target": [0, 1.1, 0], "focal": 50, "fstop": 1.8},
                        {"t": 2 * B, "pos": [0.35, 1.2, 1.1], "target": [0, 1.0, 0], "focal": 55, "fstop": 1.8}]},
    "objects": [TERRAIN] + phone}),
    {"text": "Every harbour *saved.*", "font": SERIF, "size": "9vw", "width": "84%", "color": LICHEN, "x": "50%", "y": "8%", "anchor": "center", "enter": "mask by line up"}]})
# Feature 2: turn-by-turn — fly along the trail
scenes.append({"id": "f2", "duration": 4 * B, "transition": "cut",
               "camera": {"animate": [{"from": {"x": 1200}, "to": {"x": 0}, "duration": 0.22, "ease": "outExpo"}]}, "layers": [s3d({
    "camera": cam([(0, [-4.5, 1.6, 2.5], [-2.5, 0.4, 0.8], 28), (2 * B, [-2.0, 1.3, 2.2], [0, 0.4, -0.6], 30), (4 * B, [0.5, 1.2, 2.0], [2.5, 0.4, 0.6], 32)]),
    "objects": [TERRAIN, trail(lead=0.0, speed=0.55)]}),
    {"text": "Every", "font": SANS, "size": "17vw", "weight": 900, "color": LICHEN, "x": "50%", "y": "14%", "anchor": "center", "letterSpacing": "-0.04em",
     "animate": [{"from": {"scale": 1.6, "opacity": 0}, "duration": 0.12, "ease": "outExpo"}]},
    {"text": "*channel.*", "font": SERIF, "size": "17vw", "color": AMBER, "x": "50%", "y": "24%", "anchor": "center", "start": 2 * B,
     "animate": [{"from": {"scale": 1.6, "opacity": 0}, "duration": 0.12, "ease": "outExpo"}]}]})
# Feature 3: share where you are without bars — two beacons linked by an arc
scenes.append({"id": "f3", "duration": 4 * B, "transition": "cut", "background": LICHEN,
               "camera": {"animate": [{"from": {"zoom": 1.2}, "to": {"zoom": 1}, "duration": 0.25, "ease": "outExpo"},
                                      {"to": {"zoom": 1.15, "y": -120}, "at": 2 * B, "duration": 0.2, "ease": "outExpo"}],
                          "shake": {"amount": 6, "at": 2 * B, "duration": 0.12}}, "layers": [
    *[{"shape": "path", "d": f"M-60 {300 + k*130} C 300 {200 + k*130}, 700 {420 + k*130}, 1140 {260 + k*130}", "stroke": "#A7BDCB", "strokeWidth": 4, "fill": "none",
       "x": -60, "y": 250 + k*130, "anchor": "topLeft"} for k in range(12)],
    {"shape": "path", "d": "M220 1300 C 260 700, 820 900, 860 520", "stroke": AMBER, "strokeWidth": 10, "fill": "none", "x": 220, "y": 520, "anchor": "topLeft",
     "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.7, "ease": "inOutExpo"}]},
    {"shape": "circle", "width": 70, "height": 70, "fill": AMBER, "x": 220, "y": 1300, "glow": AMBER, "scale": "=1 + 0.25*(1 - fract(beat))"},
    {"shape": "circle", "width": 70, "height": 70, "fill": MOSS, "x": 860, "y": 520, "start": 0.6},
    {"text": "Share your fix", "font": SANS, "size": "13vw", "weight": 900, "color": MOSS, "x": "50%", "y": "76%", "anchor": "center", "letterSpacing": "-0.04em"},
    {"text": "*offshore.*", "font": SERIF, "size": "14vw", "color": "#B0570A", "x": "50%", "y": "76%", "anchor": "center", "start": 2 * B,
     "animate": [{"from": {"scale": 1.6, "opacity": 0, "blur": 10}, "duration": 0.12, "ease": "outExpo"}]}]})
# Proof: a demonstration, the signal meter at zero while navigation keeps going
scenes.append(word3d("0", EMIT, False, [(0, [0, 1.0, 3.0], [0, 0.95, 0.9], 40), (2 * B, [0, 0.95, 2.7], [0, 0.95, 0.9], 42)],
                     layers=[{"text": "BARS NEEDED", "font": MONO, "size": "6vw", "color": LICHEN, "x": "50%", "y": "74%", "anchor": "center"}]))
scenes[-1]["layers"][0]["props"]["objects"][1]["height"] = 1.0
scenes.append(word3d("Tideline", INKMAT, True, WCAM, extra=[trail(lead=0.3, speed=0.8)]))
scenes.append({"id": "end", "duration": 4 * B, "transition": "cut", "layers": [s3d({
    "camera": cam([(0, [0, 6.5, 3.2], [0, 0, -0.2], 24), (4 * B, [0, 5.8, 2.6], [0, 0, -0.2], 26)]),
    "objects": [TERRAIN, trail(lead=0.6, speed=0.3)]}),
    {"text": "Tideline", "font": SERIF, "size": "22vw", "color": LICHEN, "x": "50%", "y": "30%", "anchor": "center",
     "animate": [{"from": {"scale": 1.4, "opacity": 0, "blur": 12}, "duration": 0.2, "ease": "outExpo"}]},
    {"text": "Sea charts that never drop.", "font": SANS, "size": "6vw", "weight": 600, "color": LICHEN, "x": "50%", "y": "72%", "anchor": "center",
     "enter": {"preset": "fadeUp", "delay": 0.4}},
    {"text": "tideline.app", "font": MONO, "size": "5.5vw", "color": AMBER, "x": "50%", "y": "79%", "anchor": "center", "enter": {"preset": "fadeUp", "delay": 0.7}}]})

spec = {"size": "9:16", "fps": 30, "intent": {"pace": "fast", "genre": "launch"},
        "music": {"style": "cinematic-hybrid", "bpm": 128, "key": "Dm", "mood": "driving", "hits": ["@bar3", "@bar7"], "end": "ring"},
        "audio": [{"sfx": "whoosh", "at": 2 * B}, {"sfx": "riser", "at": 8 * B}, {"sfx": "impact", "at": 8 * B},
                  {"sfx": "whoosh", "at": 16 * B}, {"sfx": "pop", "at": 22 * B + 0.6}, {"sfx": "impact", "at": 26 * B}, {"sfx": "chime", "at": 26.5 * B}],
        "post": {"preset": "cinematic", "grain": 0.05},
        "compositions": {"map": {"size": [720, 1560], "duration": 15, "background": "#E6EEF2", "layers": map_layers("=min(1, 0.25 + t*0.12)")}},
        "scenes": scenes}
json.dump(spec, open(__file__.replace("make.py", "tideline.json"), "w"), indent=1)
