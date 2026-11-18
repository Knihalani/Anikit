#!/usr/bin/env python3
"""Pass 2 spec: "It's coffee." — one shader world (teatable.glsl), shadow type through the window
(iChannel0), the bag's label (iChannel1). Writes ../spec.json."""
import json, os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "spec.json")
SH = "src/teatable.glsl"
SERIF = "Libre Caslon Text"
SANS = "Bricolage Grotesque"
W, H = 1080, 1920


def zexpr(z0, z1, d):
    return f"({z0}+({z1}-{z0})*t/{d})"


def sign(text, x, y, size, z0, z1, d, rot=0):
    """Window lettering in screen space; follows a push centred on the frame."""
    zx = zexpr(z0, z1, d)
    return {"text": text, "font": SERIF, "axes": {"wght": 700}, "size": size, "color": "#ffffff", "rotation": rot,
            "x": f"={W/2}+({x}-{W/2})*{zx}/{z0}", "y": f"={H/2}+({y}-{H/2})*{zx}/{z0}", "scale": f"={zx}/{z0}"}


def shot(d, u, z0, z1, words=None, ink=None, anim=None):
    u = dict(u)
    u["zoom"] = "=" + zexpr(z0, z1, d)
    lay = {"type": "shader", "src": SH, "uniforms": u, "channels": [{"layers": words or []}, {"layers": ink or []}]}
    if anim:
        lay["animate"] = anim
    return {"duration": d, "layers": [lay]}


def fade_letters(at=0.25, dur=0.5):
    return {"from": {"letters": 0.0}, "to": {"letters": 1.0}, "at": at, "duration": dur, "ease": "outCubic"}


scenes = []
# 1 · hook: macro on the tea, still rocking from the spoon (0 – 1.25)
scenes.append(shot(1.25, {"cx": 0.07, "cy": 0.05, "rot": 0.3, "stir": "=1.0*exp(-t*0.8)", "drift": 0.02, "letters": 0}, 3.6, 3.3))
# 2 · wide: the tea table; "bergamot" falls across the cloth (1.25 – 3.75)
scenes.append(shot(2.5, {"cx": 0.0, "cy": -0.10, "rot": 0.0, "stir": "=0.5*exp(-t*0.7)", "drift": "=0.02-0.01*t", "letters": 0},
                   0.95, 1.0, words=[sign("bergamot", 450, 1350, 150, 0.95, 1.0, 2.5)], anim=[fade_letters(0.35, 0.6)]))
# 3 · rolled close on the saucer and spoon: "peach" (3.75 – 5.625)
scenes.append(shot(1.875, {"cx": 0.06, "cy": -0.12, "rot": -0.42, "stir": 0.1, "drift": 0.0, "letters": 0},
                   1.55, 1.62, words=[sign("peach", 430, 1270, 210, 1.55, 1.62, 1.875, 0)], anim=[fade_letters(0.2, 0.45)]))
# 4 · the tea, closer, rolled the other way: "black tea" (5.625 – 8.75)
scenes.append(shot(3.125, {"cx": -0.02, "cy": 0.02, "rot": 0.35, "stir": 0.05, "drift": "=0.12-0.01*t", "letters": 0},
                   1.95, 2.1, words=[sign("black tea", 560, 1560, 190, 1.95, 2.1, 3.125, 0)], anim=[fade_letters(0.2, 0.45)]))
# 5 · the cloud: hard stop (8.75 – 10)
scenes.append(shot(1.25, {"cx": 0.0, "cy": 0.0, "rot": 0.12, "stir": 0, "drift": -0.03, "letters": 1, "sun": 1.0},
                   1.3, 1.28, words=[sign("black tea", 560, 1560, 190 * 1.3 / 2.1, 1.3, 1.28, 1.25, 0)],
                   anim=[{"from": {"sun": 1.0}, "to": {"sun": 0.08}, "duration": 0.6, "ease": "inOutSine"}]))

# 6 · payoff: the sun comes back on the bag: "It's coffee." (10 – 12.5), then 7 · bag close, "Out Friday" (12.5 – 15)
BAG = (0.04, -0.71)
CAM6 = (0.0, -0.37, 0.86)


def bag_screen(lx, ly, cam):
    import math
    c, s = math.cos(0.06), math.sin(0.06)
    wx, wy = BAG[0] + c * lx + s * ly, BAG[1] - s * lx + c * ly   # inverse of r2(0.06)
    cx, cy, z = cam
    return W / 2 + (wx - cx) * z * H, H / 2 - (wy - cy) * z * H


def label(cam, z1, d):
    import math
    z0 = cam[2]
    zx = zexpr(z0, z1, d)
    rot = -math.degrees(0.06)
    out = []

    def ink(text, lx, ly, size, wght, extra=None):
        x, y = bag_screen(lx, ly, cam)
        t = {"text": text, "font": SANS, "axes": {"wght": wght, "opsz": 96 if size * z0 > 50 else 14}, "size": size * z0,
             "color": "#F1ECE2", "rotation": rot,
             "x": f"={W/2}+({x:.1f}-{W/2})*{zx}/{z0}", "y": f"={H/2}+({y:.1f}-{H/2})*{zx}/{z0}", "scale": f"={zx}/{z0}"}
        if extra:
            t.update(extra)
        out.append(t)
    k = 1920 / 1920
    ink("HALDE & SONS", 0.0, 0.205, 72, 800, {"letterSpacing": "0.06em"})
    ink("coffee roasters", 0.0, 0.168, 40, 400)
    ink("Ethiopia", 0.0, 0.065, 125, 500)
    ink("Guji", 0.0, -0.050, 290, 800)
    ink("washed coffee", 0.0, -0.160, 56, 500)
    ink("bergamot · peach · black tea", 0.0, -0.205, 44, 450)
    return out


d6 = 2.5
u6 = {"cx": CAM6[0], "cy": CAM6[1], "rot": 0.0, "stir": 0.0, "drift": "=0.14+0.01*t", "letters": 0, "sun": 0.08,
      "bag": 1, "bx": BAG[0], "by": BAG[1], "spoon": 1}
scenes.append(shot(d6, u6, CAM6[2], 0.9, words=[sign("It's coffee.", 540, 830, 128, CAM6[2], 0.9, d6, 0)], ink=label(CAM6, 0.9, d6),
                   anim=[{"from": {"sun": 0.08}, "to": {"sun": 1.0}, "duration": 0.35, "ease": "outCubic"}, fade_letters(0.25, 0.3)]))

CAM7 = (0.04, -0.69, 1.22)
d7 = 2.5
u7 = {"cx": CAM7[0], "cy": CAM7[1], "rot": 0.0, "stir": 0.0, "drift": "=0.27+0.012*t", "letters": 0, "sun": 1.0,
      "bag": 1, "bx": BAG[0], "by": BAG[1], "spoon": 1}
lab7 = label(CAM7, 1.26, d7)
sx, sy = bag_screen(0.085, -0.262, CAM7)
zx7 = zexpr(CAM7[2], 1.26, d7)
for lay in ({"shape": "rect", "width": 380, "height": 104, "fill": "none", "stroke": "#E0502C", "strokeWidth": 7, "radius": 10},
            {"text": "OUT FRIDAY", "font": SANS, "axes": {"wght": 800, "opsz": 96}, "size": 62, "letterSpacing": "0.03em", "color": "#E0502C"}):
    lay.update({"rotation": -8, "x": f"={W/2}+({sx:.1f}-{W/2})*{zx7}/{CAM7[2]}", "y": f"={H/2}+({sy:.1f}-{H/2})*{zx7}/{CAM7[2]}",
                "animate": [{"from": {"scale": 1.4, "opacity": 0}, "to": {"scale": 1.0, "opacity": 1}, "at": 0.35, "duration": 0.1, "ease": "outCubic"}]})
    lab7.append(lay)
scenes.append(shot(d7, u7, CAM7[2], 1.26, ink=lab7))

spec = {
    "size": "9:16", "fps": 30,
    "music": {"style": "acoustic", "bpm": 96, "key": "D", "mood": "warm bright morning",
              "instruments": {"lead": "kalimba"},
              "sections": [{"name": "intro", "bars": 1, "energy": 0.4}, {"name": "verse", "bars": 3, "energy": 0.6},
                           {"name": "chorus", "bars": 2, "energy": 0.85, "stop": 2}],
              "hits": ["@bar5"], "end": "ring", "followCuts": False},
    "sound": {"palette": "organic", "ambience": "room", "density": 0.35,
              "cues": [{"sfx": "ceramic", "at": 0.0, "gain": -3}, {"sfx": "metal", "at": 0.08, "gain": -12, "pitch": 7},
                       {"sfx": "paper", "at": 10.0, "gain": -6}, {"sfx": "thump", "at": 12.85, "gain": -4}]},
    "scenes": scenes,
}
json.dump(spec, open(OUT, "w"), indent=1)
print("wrote", OUT, sum(s["duration"] for s in scenes))
