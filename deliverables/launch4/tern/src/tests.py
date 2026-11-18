import json, sys
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from form import *

def page(stock, extra):
    return {"size": [1920, 1080], "duration": 6, "background": STOCK[stock], "layers": printed(stock) + extra}

rows = [
    hand("dawn chorus", 96, row_y(0), 0.0, 0.5), hand("1", 548, row_y(0), 0.0, 0.1),
    hand("FS — missed the top", 1070, row_y(0), 0.6, 0.9),
    hand("kick, live room", 96, row_y(1), 0.0, 0.5), hand("3", 548, row_y(1), 0.0, 0.1),
    hand("NG — clipped", 1070, row_y(1), 1.6, 0.6),
    hand("blackbird 5:12am", 96, row_y(2), 0.0, 0.5), hand("1", 548, row_y(2), 0.0, 0.1),
    hand("got the top", 1070, row_y(2), 2.4, 0.6),
    circle(1250, row_y(2), 210, 38, 3.2, seed=3),
]
caveat = [dict(r, font="Caveat", size=48) if "text" in r else r for r in rows]
light = {"shape": "rect", "width": 1920, "height": 1080, "x": 960, "y": 540, "blend": "multiply",
         "fill": {"type": "radial", "center": [0.3, 0.2], "radius": 1.1, "stops": [[0, "#ffffff"], [0.55, "#d9cfd3"], [1, "#8d8288"]]}}

spec = {"size": "16:9", "fps": 30,
  "compositions": {"pinkA": page("pink", rows), "pinkB": page("pink", caveat), "yellow": page("yellow", rows)},
  "scenes": [
    {"duration": 4, "background": "#000", "layers": [{"composition": "pinkA", "width": 1920, "height": 1080, "x": 960, "y": 540}, light],
     "camera": {"animate": [{"from": {"zoom": 1.7, "x": 300, "y": -60}, "to": {"zoom": 1.5, "x": 300, "y": -60}, "duration": 4}]}},
    {"duration": 4, "background": "#000", "layers": [{"composition": "pinkB", "width": 1920, "height": 1080, "x": 960, "y": 540}, light],
     "camera": {"zoom": 2.6, "x": 380, "y": -10}},
    {"duration": 2, "background": "#000", "layers": [{"component": "scene3d", "props": {
        "env": {"preset": "studio", "color": "#3a3336"}, "rig": {"preset": "window", "gobo": "blinds", "warmth": 0.2},
        "camera": {"lens": "macro100", "keys": [{"t": 0, "pos": [0.25, 0.55, 0.95], "target": [0.12, 0, 0.05], "fstop": 4},
                                               {"t": 2, "pos": [0.22, 0.52, 0.9], "target": [0.12, 0, 0.05], "fstop": 4}]},
        "objects": [{"type": "plane", "name": "page", "size": [1.92, 1.08], "pos": [0, 0, 0],
                     "rot": [0, -90, 0], "texture": "#pinkA", "material": {"base": "#ffffff", "roughness": 0.85}}]}}]},
    {"duration": 3, "background": STOCK["yellow"], "layers": [
        {"text": "NG", "font": FORM, "size": "70vh", "weight": 900, "axes": {"wdth": 62}, "color": INK, "x": "50%", "y": "52%", "inkTexture": 0.2},
        {"shape": "path", "d": "M300 620 C 700 560, 1200 640, 1650 470", "x": 300, "y": 470, "stroke": BIC, "strokeWidth": 26, "brush": {"pressure": 0.6}, "blend": "multiply",
         "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.4, "at": 0.8, "ease": "outCubic"}]}, light]}
  ]}
json.dump(spec, open(sys.argv[1], "w"), indent=1)
