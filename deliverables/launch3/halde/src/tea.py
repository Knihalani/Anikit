#!/usr/bin/env python3
"""Pass 2 — 'It's not tea.' A Victorian tea table in morning sun; the light writes the tasting
notes (bergamot, peach, black tea) across it like a description of Earl Grey, then lands on the
roaster's bag: it's coffee. One ray-traced scene3d world for every shot.

    python3 src/tea.py            -> spec.json
    python3 src/tea.py frames     -> launch/frames2/frames.json (three light treatments of the reveal)
"""
import json, math, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..")
FONT = "Bricolage Grotesque"

# model is recentred on its bounds: part positions in world metres
CUP1 = (0.134, 0.0, 0.102)
CUP2 = (0.384, 0.0, 0.102)
SUGAR = (-0.116, 0.0, 0.102)
POT = (-0.366, 0.0, 0.102)
BAG = (0.10, 0.0, 0.33)
BAG_ROT = -18
LIQ_Y = 0.059

FLOOR = {"texture": "ambientcg:Fabric081C", "scale": 3, "color": "#F2EDE4"}

LIGHTS = {
    "sun": {"env": {"hdri": "polyhaven:hdri/cayley_interior", "background": "blur", "exposure": -0.6, "rotate": 200},
            "rig": {"preset": "golden", "warmth": 0.6, "key": 6, "ratio": 6}},
    "sunwin": {"env": {"hdri": "polyhaven:hdri/cayley_interior", "background": "blur", "exposure": -0.6, "rotate": 200},
               "rig": {"preset": "window", "warmth": 0.6, "gobo": "window", "goboScale": 0.6, "key": 6, "ratio": 6, "softness": 0.05}},
    "soft": {"env": {"hdri": "polyhaven:hdri/cayley_interior", "background": "blur", "exposure": 1.6, "rotate": 140}},
}


def liquid(pos, t_ripple=True):
    x, _, z = pos
    r = 0.0468
    y = (f"={LIQ_Y}+0.0012*sin(u*46-t*11)*exp(-t*0.9)*u" if t_ripple else f"={LIQ_Y}")
    return {"type": "surface", "name": "brew",
            "x": f"={x}+{r}*u*cos(v*6.28318)", "y": y, "z": f"={z}+{r}*u*sin(v*6.28318)",
            "u": [0, 1, 40], "v": [0, 1, 96],
            "material": {"base": "#6A2608", "roughness": 0.02, "specular": 0.8, "clearcoat": 1.0}}


def bag_objects():
    bx, _, bz = BAG
    return [
        {"type": "box", "name": "bag", "size": [0.15, 0.034, 0.225], "radius": 0.012, "pos": [bx, 0.017, bz], "rot": [0, BAG_ROT, 0],
         "material": {"extends": "matte", "base": "#121010", "roughness": 0.78, "imperfections": 0.5}},
        {"type": "plane", "size": [0.15, 0.225], "pos": [bx, 0.0345, bz], "rot": [-90, BAG_ROT, 0],
         "texture": "#label", "material": "decal", "thin": True, "shadow": False},
    ]


LABEL = {"size": [800, 1200], "duration": 15, "layers": [
    {"shape": "rect", "width": 800, "height": 70, "x": 400, "y": 35, "fill": "#0B0A0A"},
    {"text": "HALDE & SONS", "font": FONT, "axes": {"wght": 800, "opsz": 96}, "size": 64, "letterSpacing": "0.06em", "x": 400, "y": 200, "color": "#EFE9DF"},
    {"text": "coffee roasters", "font": FONT, "axes": {"wght": 400, "opsz": 14}, "size": 36, "x": 400, "y": 262, "color": "#EFE9DF"},
    {"text": "Ethiopia", "font": FONT, "axes": {"wght": 500, "opsz": 96}, "size": 108, "x": 400, "y": 520, "color": "#EFE9DF"},
    {"text": "Guji", "font": FONT, "axes": {"wght": 800, "opsz": 96}, "size": 250, "x": 400, "y": 700, "color": "#EFE9DF"},
    {"text": "washed · coffee", "font": FONT, "axes": {"wght": 500, "opsz": 14}, "size": 46, "x": 400, "y": 880, "color": "#EFE9DF"},
    {"text": "bergamot · peach · black tea", "font": FONT, "axes": {"wght": 450, "opsz": 14}, "size": 40, "x": 400, "y": 950, "color": "#EFE9DF"},
    {"shape": "circle", "radius": 30, "x": 650, "y": 120, "fill": "none", "stroke": "#3A3634", "strokeWidth": 5},
]}


def world(light, cam, objects_extra=(), gobo=None, key_anim=None, with_bag=False, ripple=True):
    props = {"objects": [{"type": "model", "name": "set", "src": "polyhaven:model/tea_set_01@2k"},
                         liquid(CUP1, ripple),
                         {"type": "cylinder", "radius": 0.0468, "height": 0.001, "pos": [CUP2[0], LIQ_Y, CUP2[2]],
                          "material": {"base": "#6A2608", "roughness": 0.02, "specular": 0.8, "clearcoat": 1.0}}],
             "floor": FLOOR,
             "camera": cam, "quality": "standard", "exposure": 1.0, "vignette": 0.25}
    props.update(json.loads(json.dumps(LIGHTS[light])))
    if key_anim and "rig" in props:
        props["rig"]["key"] = key_anim
    if with_bag:
        props["objects"] += bag_objects()
    props["objects"] += list(objects_extra)
    if gobo:
        props["gobo"] = gobo
    return {"component": "scene3d", "props": props}


def cam(keys, shake="breath"):
    return {"shake": shake, "keys": keys}


def gobo(text, at, size=0.04, frm=None, start=0.2, settle=0.9):
    g = {"text": text, "font": FONT, "weight": 700, "size": size, "at": list(at), "invert": True, "strength": 0.9,
         "softness": 0.03, "start": start, "settle": settle}
    if frm:
        g["from"] = list(frm)
    return g


REVEAL_CAM = cam([{"t": 0, "pos": [0.42, 0.42, 0.78], "target": [0.12, 0.02, 0.26], "focal": 42, "fstop": 5.6, "focus": "bag"},
                  {"t": 5, "pos": [0.38, 0.40, 0.74], "target": [0.12, 0.02, 0.26], "focal": 42, "fstop": 5.6, "focus": "bag"}])


def frames():
    sc = []
    for name in ("sun", "sunwin", "soft"):
        sc.append({"duration": 1, "layers": [world(name, REVEAL_CAM, gobo=gobo("It's coffee.", (0.13, 0.50), 0.035, start=0, settle=0.01), with_bag=True)]})
    spec = {"size": "9:16", "fps": 30, "compositions": {"label": LABEL}, "scenes": sc}
    json.dump(spec, open(os.path.join(ROOT, "launch/frames2/frames.json"), "w"), indent=1)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "frames":
        frames()
