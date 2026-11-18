#!/usr/bin/env python3
"""Builds spec.json for the Halde & Sons — Ethiopia Guji Reel.
One world shader (counter.glsl) renders every shot; text enters through its channels:
iChannel0 = words written in the caustic light, iChannel1 = the bag's printed label."""
import json, math, os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "spec.json")
SH = "src/counter.glsl"
FONT = "Bricolage Grotesque"
W, H = 1080, 1920
DIR = (-0.36 / math.hypot(0.36, 1), -1 / math.hypot(0.36, 1))


def screen(wx, wy, cx, cy, zoom, rot):
    dx, dy = wx - cx, wy - cy
    c, s = math.cos(rot), math.sin(rot)
    ux, uy = (c * dx - s * dy) * zoom, (s * dx + c * dy) * zoom
    return W / 2 + ux * H, H / 2 - uy * H


def world_shader(dur, u, anim=None, words=None, ink=None):
    lay = {"type": "shader", "src": SH, "uniforms": u,
           "channels": [{"layers": words or []}, {"layers": ink or []}]}
    if anim:
        lay["animate"] = anim
    return lay


def light_word(text, x, y, size, rot_deg=0.0):
    return {"text": text, "font": FONT, "axes": {"wght": 720, "opsz": 96}, "size": size,
            "x": round(x), "y": round(y), "color": "#ffffff", "rotation": rot_deg}


G = (0.0, 0.25)  # glass for shots 1-6
L0 = 0.78
CC = (G[0] + DIR[0] * L0 * 0.6, G[1] + DIR[1] * L0 * 0.6)  # caustic centre
WORD = (G[0] + DIR[0] * 0.215, G[1] + DIR[1] * 0.215)       # between glass and caustic
BASE = {"gx": G[0], "gy": G[1], "mat": 2, "slen": L0, "cscale": 0.85, "bar": 0.0}

scenes = []

# ---- 1 · hook + pull-back (0 – 3.75) -----------------------------------
d = 3.75
u = dict(BASE, cx=CC[0] + 0.02, cy=CC[1] - 0.02, zoom=7.0, rot=-0.10, morph=0.0, slosh=1.0, words=0.0, bar=0.55, barx=0.42)
S = "smoothstep(0.85,2.75,t)"
u["zoom"] = f"=exp({math.log(6.6):.4f}+({math.log(0.78):.4f}-{math.log(6.6):.4f})*{S})*(1-0.02*t)"
u["cx"] = f"={CC[0]+0.02:.4f}+({-0.06-CC[0]-0.02:.4f})*{S}"
u["cy"] = f"={CC[1]-0.02:.4f}+({0.02-CC[1]+0.02:.4f})*{S}"
scenes.append({"duration": d, "layers": [world_shader(d, u, anim=[
    {"from": {"slosh": 1.0}, "to": {"slosh": 0.15}, "duration": 3.2, "ease": "outQuad"},
])]})

# ---- shots 3-5 · the light draws the notes --------------------------------
def note_shot(d, morph0, morph1, word, cam, z0, z1, rot, wsize, wpos=None, extra=None, wy_px=None):
    cx, cy = cam
    wx, wy = wpos or WORD
    if wy_px is not None:  # on the shadow's axis, at screen height wy_px (first frame)
        best = None
        for i in range(2000):
            a = i / 2000 * 0.78
            px_, py_ = G[0] + DIR[0] * a, G[1] + DIR[1] * a
            _, sy_ = screen(px_, py_, cx, cy, z0, rot)
            if best is None or abs(sy_ - wy_px) < best[0]:
                best = (abs(sy_ - wy_px), px_, py_)
        wx, wy = best[1], best[2]
    zx = f"({z0}+({z1}-{z0})*t/{d})"
    u = dict(BASE, cx=cx, cy=cy, zoom=f"={zx}", rot=rot, morph=morph0, slosh=f"=0.9*exp(-t*2.6)", words=0.0)
    if extra:
        u.update(extra)
    c, s = math.cos(rot), math.sin(rot)
    dx, dy = wx - cx, wy - cy
    ox, oy = (c * dx - s * dy), (s * dx + c * dy)
    tw = light_word(word, 0, 0, wsize, -math.degrees(rot))
    tw["x"] = f"={W/2}+{ox:.5f}*{zx}*{H}"
    tw["y"] = f"={H/2}-{oy:.5f}*{zx}*{H}"
    tw["scale"] = f"={zx}/{z0}"
    return {"duration": d, "layers": [world_shader(d, u, anim=[
        {"from": {"morph": morph0}, "to": {"morph": morph1}, "at": 0.12, "duration": 0.75, "ease": "inOutCubic"},
        {"from": {"words": 0.0}, "to": {"words": 1.0}, "at": 0.55, "duration": 0.45, "ease": "outCubic"},
    ], words=[tw])]}

mid = ((WORD[0] + CC[0]) / 2, (WORD[1] + CC[1]) / 2)
scenes.append(note_shot(1.875, 0.0, 1.0, "peach", (mid[0] + 0.01, mid[1] - 0.01), 1.45, 1.55, 0.0, 112))
scenes.append(note_shot(1.875, 1.0, 2.0, "bergamot", (CC[0] - 0.03, CC[1] + 0.06), 1.75, 1.85, 0.24, 120, wy_px=300))
scenes.append(note_shot(1.25, 2.0, 3.0, "black tea", (CC[0] + 0.05, CC[1] + 0.07), 1.85, 1.95, 0.30, 104, wy_px=330,
                        extra={"bar": 0.5, "barx": 0.05}))

# ---- 6 · the cloud: hard stop (8.75 – 10) -------------------------------
d = 1.25
u = dict(BASE, cx=-0.10, cy=0.04, zoom=1.05, rot=0.06, morph=3.0, slosh=0.0, words=0.0, sun=1.0)
scenes.append({"duration": d, "layers": [world_shader(d, u, anim=[
    {"from": {"sun": 1.0}, "to": {"sun": 0.10}, "at": 0.0, "duration": 0.55, "ease": "inOutSine"},
    {"from": {"zoom": 1.05}, "to": {"zoom": 1.0}, "duration": 1.25, "ease": "linear"},
])]})

# ---- 7 · payoff: the light climbs the label (10 – 15) ---------------------
d = 5.0
G7 = (0.10, 0.42)
BAG = (-0.03, -0.13)
BROT = 0.035
CAM7 = (0.0, 0.035, 0.95, 0.0)


def bag_pt(lx, ly):
    c, s = math.cos(BROT), math.sin(BROT)
    wx, wy = BAG[0] + c * lx - s * ly, BAG[1] + s * lx + c * ly
    return screen(wx, wy, *CAM7)


def ink(text, lx, ly, size, wght, color="#F1ECE2", spacing=None, extra=None):
    x, y = bag_pt(lx, ly)
    t = {"text": text, "font": FONT, "axes": {"wght": wght, "opsz": 96 if size > 60 else 14}, "size": size,
         "x": round(x), "y": round(y), "color": color, "rotation": -math.degrees(BROT)}
    if spacing:
        t["letterSpacing"] = spacing
    if extra:
        t.update(extra)
    return t

sx, sy = bag_pt(0.095, -0.268)
label = [
    ink("HALDE & SONS", 0.0, 0.205, 64, 800, spacing="0.06em"),
    ink("coffee roasters", 0.0, 0.165, 36, 400),
    ink("Ethiopia", -0.0, 0.065, 112, 500),
    ink("Guji", 0.0, -0.045, 250, 800),
    ink("washed", 0.0, -0.150, 48, 500),
    ink("bergamot · peach · black tea", 0.0, -0.192, 38, 450),
    # the stamp (vermilion, same red-amber family), lands on @bar6 = scene time 2.5
    {"shape": "rect", "width": 400, "height": 108, "x": round(sx), "y": round(sy), "fill": "none",
     "stroke": "#E0502C", "strokeWidth": 7, "radius": 10, "rotation": -9,
     "animate": [{"from": {"scale": 1.35, "opacity": 0}, "to": {"scale": 1.0, "opacity": 1}, "at": 2.5, "duration": 0.12, "ease": "outCubic"}]},
    {"text": "OUT FRIDAY", "font": FONT, "axes": {"wght": 800, "opsz": 96}, "size": 60, "letterSpacing": "0.03em",
     "x": round(sx), "y": round(sy), "color": "#E0502C", "rotation": -9,
     "animate": [{"from": {"scale": 1.35, "opacity": 0}, "to": {"scale": 1.0, "opacity": 1}, "at": 2.5, "duration": 0.12, "ease": "outCubic"}]},
]
u = {"gx": G7[0], "gy": G7[1], "bx": BAG[0], "by": BAG[1], "bag": 1.0, "mat": 2, "cscale": 1.0,
     "cx": CAM7[0], "cy": CAM7[1], "zoom": CAM7[2], "rot": CAM7[3], "morph": 4.0, "slosh": "=0.5*exp(-t*1.5)",
     "words": 0.0, "sun": 0.1, "slen": 1.25, "bar": 0.0}
scenes.append({"duration": d, "layers": [world_shader(d, u, anim=[
    {"from": {"sun": 0.10}, "to": {"sun": 1.0}, "at": 0.0, "duration": 0.45, "ease": "outCubic"},
    {"from": {"slen": 1.25}, "to": {"slen": 0.62}, "at": 0.0, "duration": 3.4, "ease": "inOutSine"},
    {"from": {"zoom": 0.95}, "to": {"zoom": 0.95}, "duration": 0.1},
], ink=label)]})

spec = {
    "size": "9:16", "fps": 30,
    "fonts": {},
    "music": {"style": "acoustic", "bpm": 96, "key": "D", "mood": "warm bright morning",
              "instruments": {"lead": "kalimba"},
              "sections": [{"name": "intro", "bars": 1, "energy": 0.4}, {"name": "verse", "bars": 3, "energy": 0.6},
                           {"name": "chorus", "bars": 2, "energy": 0.85, "stop": 2}],
              "hits": ["@bar5"], "end": "ring", "followCuts": False},
    "sound": {"palette": "organic", "ambience": "room", "density": 0.35,
              "cues": [{"sfx": "glass", "at": 0.0, "gain": -2},
                       {"sfx": "droplet", "at": 3.9, "gain": -8}, {"sfx": "droplet", "at": 5.75, "gain": -9},
                       {"sfx": "droplet", "at": 7.6, "gain": -9},
                       {"sfx": "thump", "at": 12.5, "gain": -3}]},
    "scenes": scenes,
}
json.dump(spec, open(OUT, "w"), indent=1)
print("wrote", OUT, "length", sum(s["duration"] for s in scenes))
