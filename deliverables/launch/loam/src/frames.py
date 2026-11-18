"""Step 4: three style frames of the key moment (the pour stops exactly at the roots), varying the light."""
import json, sys
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from world import *

def hero(variant):
    sh = wall(sun=variant != "night") + shelf(1600)
    sh += pot(cx=540, top=860, w=780, h=760, wet=(0, 0.01, 0.62), line=0.62, sensor=(-1,), plant="monstera")
    light = dict(LIGHTS["window"])
    pal = dict(PAL)
    if variant == "overcast":
        light = {"angle": 300, "rim": 150, "color": "#E4F0F2", "shadow": "#3A4650", "strength": 0.8}
        pal.update(wall="#8E9A8C", wallLo="#5F6B62", sun="#C9D3C7")
    if variant == "night":
        light = {"angle": 330, "rim": 200, "color": "#FFC77A", "shadow": "#120F2E", "strength": 1.1}
        pal.update(wall="#1B2033", wallLo="#0D0F18")
    layer = illo(sh, light)
    layer["props"]["palette"] = pal
    if variant == "night":
        layer["props"]["shapes"].insert(1, {"shape": "circle", "cx": 1000, "cy": 380, "r": 520, "fill": {"stops": [[0, "#FFB861"], [1, "#1B2033"]], "gradient": "radial", "steps": 5, "dither": 0.2}, "opacity": 0.55, "shade": 0})
    txt = {"text": "this much.", "font": "Fraunces", "axes": {"wght": 760, "SOFT": 100, "opsz": 144}, "size": 118,
           "x": 540, "y": 1210, "anchor": "center", "color": "#57B6E8"}
    return {"duration": 1, "background": "#1E1614", "layers": [layer, txt]}

spec = {"size": "9:16", "fps": 30, "scenes": [hero(v) for v in ("window", "overcast", "night")]}
json.dump(spec, open(sys.argv[1], "w"), indent=1)
