"""Loam — 15 s 9:16 launch film.

Two passes:
  python3 src/film.py plates   -> src/plates.json (one illo scene per plate; bake each with `anikit-video still`)
  python3 src/film.py film     -> spec.json       (the film: baked plates + animated layers, mattes, type, music, sound)

The illustrated world is drawn once per shot with `illo` (world.py) and baked to PNG plates, because
re-rasterising the illustration every frame costs ~1.3 s a frame. Motion in the film comes from
mattes (water soaking down = the wet plate revealed under a growing matte), plant layers that sway,
the sensor and drops as their own layers, and camera moves.

128 bpm: beat = 0.46875 s, bar = 1.875 s, 8 bars = 15 s.
"""
import json, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from world import *

B = 60 / 128
FONT = "Fraunces"
AX = {"wght": 860, "SOFT": 100, "opsz": 144}
PLATES = {}  # name -> (illo shapes, transparent)


def soil_y(top, h, s, frac):
    it = top + 40 * s
    ib = top + h * s - 34 * s
    return it, ib, it + (ib - it) * frac


def soil_x(cx, w, s):
    w = w * s
    return cx - w / 2 + 34 * s, cx + w / 2 - 34 * s


def sensor_shapes(sx, top, s=1.0):
    return [
        {"shape": "rect", "x": sx - 9 * s, "y": top - 40 * s, "w": 18 * s, "h": 300 * s, "radius": 8 * s, "fill": "$sensorLo", "shade": 0.4},
        {"shape": "rect", "x": sx - 46 * s, "y": top - 120 * s, "w": 92 * s, "h": 110 * s, "radius": 34 * s, "fill": "$sensor",
         "shade": {"amount": 0.7, "offset": 0.18}, "shadow": {"dx": 18 * s, "dy": 10 * s, "opacity": 0.45}},
        {"shape": "circle", "cx": sx, "cy": top - 74 * s, "r": 12 * s, "fill": "$water", "shade": 0},
    ]


def stream(x0, x1, y1, wdt=40):
    return {"shape": "path", "d": f"M{x0} -40 C {x0+40} 300, {x1-30} 600, {x1} {y1} L {x1+wdt} {y1} C {x1+10} 600, {x0+80} 300, {x0+wdt+10} -40 Z",
            "fill": {"stops": [[0, "$waterHi"], [1, "$water"]], "angle": 0, "steps": 3}, "opacity": 0.92, "shade": 0}


def plate(name, shapes, transparent=False):
    PLATES[name] = (shapes, transparent)


def img(name, **kw):
    L = {"src": f"plates/{name}.png", "x": 540, "y": 960, "width": W, "height": H}
    L.update(kw)
    return L


def plant_layer(name, kind, cx, base, s=1.0, period=3.2, amp=1.4, phase=0.0, seed=1):
    shapes = plant_shapes(kind, cx, base, s, 0.0, name, seed)
    plate(name, shapes, transparent=True)
    return img(name, anchor=[cx / W, base / H], x=cx, y=base,
               rotation=f"={amp}*sin(t*{6.283 / period:.3f}+{phase})")


def matte_rect(mid, x, y, w, h, grow, at, dur, ease="outCubic", soft=26):
    """A hidden rect that grows from its top edge (grow='down') or its bottom edge (grow='up')."""
    anchor = [0.5, 0] if grow == "down" else [0.5, 1]
    yy = y if grow == "down" else y + h
    return {"id": mid, "shape": "rect", "width": w, "height": h, "x": x + w / 2, "y": yy, "anchor": anchor, "fill": "#ffffff",
            "blur": soft, "visible": False,
            "animate": [{"from": {"scaleY": 0.001}, "to": {"scaleY": 1}, "at": at, "duration": dur, "ease": ease}]}


def word(text, x, y, size, color, enter="fadeUp", anchor="left", start=0, extra=None, axes=AX):
    t = {"text": text, "font": FONT, "axes": axes, "size": size, "x": x, "y": y, "anchor": anchor, "color": color, "lineHeight": 0.92}
    if enter:
        t["enter"] = {"preset": enter, "delay": start}
    if extra:
        t.update(extra)
    return t


def scene(dur, layers, camera=None):
    s = {"duration": round(dur, 5), "background": "#1E1614", "layers": layers}
    if camera:
        s["camera"] = camera
    return s


def cam(frm, to, dur, ease="inOutSine"):
    return {"animate": [{"from": frm, "to": to, "duration": dur, "ease": ease}]}


scenes = []

# ---- 1 · HOOK: too much. Water is already pouring in; the pool rises over the roots. -------------------------
d1 = 4 * B
P1 = dict(cx=560, top=760, w=820, h=800, seed=3)
base1 = wall() + shelf(1640)
plate("s1_dry", base1 + pot(**P1, plant=None) + [stream(330, 500, 760)])
plate("s1_wet", base1 + pot(**P1, plant=None, flood=(0, 0.01)) + [stream(330, 500, 760)])
it, ib, _ = soil_y(760, 800, 1, 0)
l, r = soil_x(560, 820, 1)
scenes.append(scene(d1, [
    img("s1_dry"),
    matte_rect("m1", l - 40, it - 30, r - l + 80, ib - it + 60, "up", 0.0, 1.5, "outQuad"),
    img("s1_wet", matte={"layer": "m1"}),
    plant_layer("s1_plant", "monstera", 560, 730, 1.0, period=2.6, amp=1.8, seed=3),
    word("Too\nmuch.", 110, 1180, 190, "#F7F1E6", start=0.25, extra={"shadow": {"x": 0, "y": 8, "blur": 18, "color": "#0B1E33"}}),
], camera=cam({"zoom": 1.22, "y": -160, "x": 60}, {"zoom": 1.0, "y": 0, "x": 0}, d1, "outSine")))

# ---- 2 · too little. Cracked pale earth, a different plant. -----------------------------------------------------
d2 = 4 * B
plate("s2", wall(sun_pts=[[-40, 120], [760, 220], [700, 1420], [-40, 1380]]) + shelf(1700)
      + pot(cx=600, top=900, w=760, h=760, dry_cracks=True, plant=None, seed=8))
scenes.append(scene(d2, [
    img("s2"),
    plant_layer("s2_plant", "snake", 600, 870, 1.0, period=4, amp=0.6, seed=8),
    word("Too\nlittle.", 140, 1300, 190, "#1E120B", start=0.2),
], camera=cam({"zoom": 1.0, "x": 80, "y": 60}, {"zoom": 1.18, "x": -40, "y": -120}, d2)))

# ---- 3 · the sensor clips in and reads the soil (macro). --------------------------------------------------------
d3 = 4 * B
plate("s3", wall(sun_pts=[[-200, -100], [900, 0], [860, 1100], [-200, 1060]]) + shelf(1880)
      + pot(cx=250, top=640, w=1500, h=1500, plant=None, seed=5))
sx3 = 250 + 750 - 110
plate("s3_sensor", sensor_shapes(sx3, 640, 1.0), transparent=True)
rings = []
for k in range(4):
    rings.append({"shape": "ellipse", "width": 120, "height": 44, "x": sx3, "y": 760, "fill": "none", "stroke": "#57B6E8", "strokeWidth": 5,
                  "opacity": 0,
                  "animate": [{"to": {"opacity": 1}, "at": 0.6 + k * 0.24, "duration": 0.05},
                              {"to": {"y": 1350, "scale": 3.4, "opacity": 0}, "at": 0.65 + k * 0.24, "duration": 0.9, "ease": "outQuad"}]})
scenes.append(scene(d3, [
    img("s3"),
    img("s3_sensor", sfx={"land": "plastic"}, animate=[{"from": {"y": 960 - 300}, "to": {"y": 960}, "at": 0.2, "duration": 0.35, "ease": "outBack"}]),
    word("It reads\nthe soil.", 100, 1150, 150, "#1E120B", start=0.3),
], camera=cam({"zoom": 1.0, "y": -80}, {"zoom": 1.06, "y": 60}, d3)))

# ---- 4 · the app: when and how much. Phone held into the window light. ----------------------------------------
d4 = 3 * B
phone = [
    {"shape": "rect", "cx": 560, "cy": 980, "w": 640, "h": 1260, "radius": 90, "fill": "#17110F",
     "shade": {"amount": 0.5, "offset": 0.04}, "shadow": {"dx": 60, "dy": 40, "opacity": 0.45}},
    {"shape": "rect", "name": "screen", "cx": 560, "cy": 980, "w": 590, "h": 1206, "radius": 70, "fill": "#F3EBDD", "shade": 0,
     "grain": 0.05, "texture": {"style": "paper", "amount": 0.4}},
]
hand = [
    {"shape": "path", "d": "M120 1980 C 140 1600, 220 1420, 300 1300 C 360 1220, 440 1260, 420 1360 C 520 1340, 600 1400, 640 1500 L 760 1980 Z",
     "fill": "#C98E6A", "shade": {"amount": 0.7, "offset": 0.12}, "outline": True},
    {"shape": "path", "d": "M820 1980 C 860 1700, 900 1500, 880 1340 C 870 1280, 930 1260, 950 1320 C 990 1460, 980 1700, 1000 1980 Z",
     "fill": "#B97D5B", "shade": {"amount": 0.7, "offset": 0.12}, "outline": True},
]


def mini(wet):
    mp = pot(cx=560, top=720, w=360, h=360, s=0.85, name="m", plant=None, line=0.62,
             wet=(0, 0.01, 0.62) if wet else None, sway=0, seed=11)
    for m in mp:
        if "__group__" not in m:
            if not m.get("clipTo"):
                m["clipTo"] = "screen"
            m.pop("shadow", None)
    return mp


w4 = wall(sun_pts=[[-40, 200], [1100, 320], [1060, 1700], [-40, 1640]])
plate("s4_dry", w4 + phone + mini(False) + hand)
plate("s4_wet", w4 + phone + mini(True) + hand)
it4, ib4, ly4 = soil_y(720, 360, 0.85, 0.62)
l4, r4 = soil_x(560, 360, 0.85)
scenes.append(scene(d4, [
    img("s4_dry"),
    matte_rect("m4", l4 - 10, it4 - 4, r4 - l4 + 20, ly4 - it4 + 12, "down", 0.45, 0.6, soft=6),
    img("s4_wet", matte={"layer": "m4"}),
    word("Water now.", 300, 480, 92, "#17110F", start=0.1, enter="fadeUp"),
    word("up to\nhere", 560, 1170, 64, "#2C7FB8", anchor="center", start=0.7, enter="fadeUp", extra={"align": "center", "axes": {"wght": 760, "SOFT": 100, "opsz": 72}}),
], camera=cam({"y": 260, "zoom": 0.95}, {"y": 0, "zoom": 1.0}, 0.5, "outCubic")))

# ---- 5 · the pour: wet earth soaks down and stops exactly on the line. ---------------------------------------
d5 = 4 * B
P5 = dict(cx=540, top=860, w=780, h=760, line=0.62, seed=1)
base5 = wall() + shelf(1600)
plate("s5_dry", base5 + pot(**P5, plant=None, sensor=(-1,)))
plate("s5_wet", base5 + pot(**P5, plant=None, sensor=(-1,), wet=(0, 0.01, 0.62)))
it5, ib5, ly5 = soil_y(860, 760, 1, 0.62)
l5, r5 = soil_x(540, 780, 1)
pour = {"shape": "path", "d": "M0 0 C 50 340, 150 640, 180 900 L 212 900 C 190 640, 90 340, 50 0 Z", "x": 250, "y": -40,
        "anchor": [0.5, 0], "fill": "linear-gradient(90deg, #A9DDF5, #57B6E8)", "opacity": 0.92,
        "animate": [{"from": {"scaleY": 0.01}, "to": {"scaleY": 1}, "duration": 0.25, "ease": "outQuad"},
                    {"to": {"opacity": 0}, "at": 1.3, "duration": 0.15}]}
scenes.append(scene(d5, [
    img("s5_dry"),
    matte_rect("m5", l5 - 30, it5 - 20, r5 - l5 + 60, ly5 - it5 + 36, "down", 0.25, 1.15, "outCubic", soft=22),
    img("s5_wet", matte={"layer": "m5"}),
    pour,
    plant_layer("s5_plant", "monstera", 540, 830, 1.0, period=3.0, amp=1.2, seed=1),
], camera=cam({"zoom": 1.0, "y": -40}, {"zoom": 1.1, "y": 90}, d5)))

# ---- 6 · scale: the whole windowsill, each plant drinks to its own depth. -----------------------------------
d6 = 3 * B
plants6 = [("monstera", 0.60), ("snake", 0.32), ("fern", 0.78), ("cactus", 0.16)]
base6 = wall(sun_pts=[[-40, 120], [1100, 200], [1060, 1200], [-40, 1180]]) + shelf(1220, 760)
dry6, wet6 = list(base6), list(base6)
for k, (pl, dep) in enumerate(plants6):
    cx = 140 + k * 270
    dry6 += pot(cx=cx, top=920, w=520, h=600, s=0.48, name=f"s{k}", line=dep, plant=pl, seed=20 + k, sway=0)
    wet6 += pot(cx=cx, top=920, w=520, h=600, s=0.48, name=f"s{k}", line=dep, plant=pl, seed=20 + k, sway=0, wet=(0, 0.01, dep))
plate("s6_dry", dry6)
plate("s6_wet", wet6)
L6 = [img("s6_dry")]
for k, (pl, dep) in enumerate(plants6):
    cx = 140 + k * 270
    a, b, y = soil_y(920, 600, 0.48, dep)
    l, r = soil_x(cx, 520, 0.48)
    L6.append(matte_rect(f"m6{k}", l - 8, a - 8, r - l + 16, y - a + 14, "down", 0.05 + k * 0.17, 0.55, soft=6))
    L6.append(img("s6_wet", matte={"layer": f"m6{k}"}))
L6.append(word("Each one,\nits own.", 90, 1390, 128, "#F2E3C4", start=0.25))
scenes.append(scene(d6, L6, camera=cam({"x": 120, "zoom": 1.05}, {"x": -60, "zoom": 1.0}, d6, "outSine")))

# ---- 7 · hard stop: one drop falls in silence over pale earth. -----------------------------------------------
d7 = 2 * B
ground = [{"shape": "rect", "x": 0, "y": 0, "w": W, "h": H, "fill": {"stops": [[0, "$wall"], [1, "$wallLo"]], "angle": 90, "steps": 3}, "grain": 0.08, "shade": 0}]
drop = {"shape": "path", "d": "M40 0 C 20 40, 0 70, 0 96 A 40 40 0 0 0 80 96 C 80 70, 60 40, 40 0 Z", "x": 540, "y": 150,
        "fill": "linear-gradient(180deg, #A9DDF5, #2C7FB8)",
        "animate": [{"to": {"y": 1300}, "duration": d7, "ease": "inQuad"}]}

# ---- 8 · payoff: the drop lands and the wet earth spells the name. -------------------------------------------
d8 = 10 * B
T0 = 2 * B  # the drop lands on the bar-7 downbeat
plate("s8", ground + pot(cx=540, top=520, w=2400, h=1700, plant=None, roots=False, seed=31))
drop8 = {"shape": "path", "d": "M40 0 C 20 40, 0 70, 0 96 A 40 40 0 0 0 80 96 C 80 70, 60 40, 40 0 Z", "x": 540, "y": -120,
         "fill": "linear-gradient(180deg, #A9DDF5, #2C7FB8)",
         "animate": [{"to": {"y": 520}, "duration": T0, "ease": "inQuad"}, {"to": {"opacity": 0}, "at": T0, "duration": 0.05}]}
splash = [{"shape": "circle", "width": 22, "height": 22, "x": 540, "y": 590, "fill": "#A9DDF5", "opacity": 0,
           "animate": [{"to": {"opacity": 1}, "at": T0, "duration": 0.02},
                       {"to": {"x": 540 + (k - 2) * 110, "y": 430 + abs(k - 2) * 60, "opacity": 0}, "at": T0, "duration": 0.45, "ease": "outQuad"}]}
          for k in range(5)]
soak = {"id": "soak", "shape": "circle", "width": 60, "height": 60, "x": 540, "y": 640, "fill": "#ffffff", "blur": 30, "visible": False, "scale": 0.01,
        "animate": [{"from": {"scale": 0.01}, "to": {"scale": 24}, "duration": 2.2, "ease": "outCubic", "at": T0 + 0.05}]}
blot = {"shape": "circle", "width": 60, "height": 60, "x": 540, "y": 680, "fill": "#2E1C11", "opacity": 0.4, "blur": 50, "scale": 0.01,
        "animate": [{"from": {"scale": 0.01}, "to": {"scale": 20}, "duration": 3.0, "ease": "outCubic", "at": T0 + 0.05}]}
halo = {"text": "loam", "font": FONT, "axes": {"wght": 900, "SOFT": 100, "opsz": 144}, "size": 400, "x": 540, "y": 860, "anchor": "center",
        "color": "#5A3A24", "blur": 18, "opacity": 0,
        "animate": [{"from": {"opacity": 0, "scale": 0.6}, "to": {"opacity": 0.8, "scale": 1.04}, "at": T0 + 0.05, "duration": 1.6, "ease": "outCubic"}]}
name = {"text": "loam", "font": FONT, "axes": {"wght": 900, "SOFT": 100, "opsz": 144}, "size": 400, "x": 540, "y": 860, "anchor": "center",
        "color": "#1E120B", "opacity": 0, "sfx": {"in": "droplet"},
        "animate": [{"from": {"opacity": 0, "blur": 24, "scale": 0.94}, "to": {"opacity": 1, "blur": 0, "scale": 1}, "at": T0 + 0.1, "duration": 1.3, "ease": "outCubic"}]}
# roots growing live from the surface down through the name (trim-on strokes), still growing at the last frame
import random
rr = random.Random(7)
roots8 = []
for k in range(9):
    x0 = 540 + (k - 4) * 14
    ex = 540 + (k - 4) * 95 + rr.uniform(-40, 40)
    ey = 1350 + rr.uniform(0, 420)
    pts = [(x0, 560), (x0 + (ex - x0) * 0.2 + rr.uniform(-60, 60), 760), (x0 + (ex - x0) * 0.6 + rr.uniform(-50, 50), 1050), (ex, ey)]
    curve = [tuple((1 - u) ** 3 * pts[0][i] + 3 * (1 - u) ** 2 * u * pts[1][i] + 3 * (1 - u) * u * u * pts[2][i] + u ** 3 * pts[3][i] for i in (0, 1))
             for u in [j / 200 for j in range(201)]]
    mnx = min(c[0] for c in curve); mxx = max(c[0] for c in curve)
    mny = min(c[1] for c in curve); mxy = max(c[1] for c in curve)
    q = [(p[0] - mnx, p[1] - mny) for p in pts]
    d = f"M{q[0][0]:.0f} {q[0][1]:.0f} C {q[1][0]:.0f} {q[1][1]:.0f}, {q[2][0]:.0f} {q[2][1]:.0f}, {q[3][0]:.0f} {q[3][1]:.0f}"
    roots8.append({"shape": "path", "d": d, "x": (mnx + mxx) / 2, "y": (mny + mxy) / 2, "stroke": "#EBDDBF", "strokeWidth": 7 - k % 3 * 2, "fill": "none",
                   "trimEnd": 0, "opacity": 0.9,
                   "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "at": T0 + 0.1 + (k % 4) * 0.12, "duration": d8 - T0 - 0.2, "ease": "outSine"}]})
key8 = {"shape": "rect", "width": W, "height": H, "x": 540, "y": 960,
        "fill": "linear-gradient(140deg, rgba(255,214,150,0.38) 0%, rgba(255,214,150,0.0) 45%, rgba(10,4,2,0.45) 100%)", "blend": "softLight"}
promise = word("Exactly when.\nExactly how much.", 540, 300, 84, "#57B6E8", anchor="center", start=T0 + 1.0, extra={"align": "center"})
promise["exit"] = {"preset": "fadeOut", "delay": 0}
promise["end"] = T0 + 3.1
cta = word("Out now.", 540, 300, 96, "#EBDDBF", anchor="center", start=T0 + 3.15, enter="fadeUp")
scenes.append(scene(d8, [img("s8"), blot, *roots8, *splash, halo, name, drop8, key8, promise, cta],
                    camera={"shake": {"amount": 6, "at": T0, "duration": 0.25},
                            "animate": [{"from": {"zoom": 1.1, "y": -60}, "to": {"zoom": 1.0, "y": 0}, "duration": d8, "ease": "linear"}]}))

spec = {
    "size": "9:16", "fps": 30,
    "intent": {"pace": "fast"},
    "post": {"grain": 0.035, "vignette": 0.18},
    "music": {"style": "acoustic", "bpm": 128, "key": "D", "mood": "warm bright",
              "instruments": {"lead": "marimba"},
              "sections": [{"name": "intro", "bars": 2, "energy": 0.45}, {"name": "build", "bars": 4, "energy": 0.75},
                           {"name": "drop", "bars": 2, "energy": 1, "stop": 2}],
              "hits": ["@bar7"], "end": "ring"},
    "sound": {"palette": "organic", "ambience": "room", "density": 0.5,
              "cues": [
                  {"sfx": "droplet", "at": 0.15, "gain": -4},
                  {"sfx": "crunch", "at": 4 * B + 0.1, "gain": -6},
                  {"sfx": "click", "at": 8 * B + 0.5, "gain": -2},
                  {"sfx": "ping", "at": 12 * B + 0.5, "gain": -6},
                  {"sfx": "droplet", "at": 15 * B + 0.3, "gain": -3},
                  {"sfx": "droplet", "at": 24 * B - 0.02, "gain": 0},
                  {"sfx": "thump", "at": 24 * B, "gain": -4},
              ]},
    "scenes": scenes,
}

here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if sys.argv[1] == "plates":
    ps = {"size": "9:16", "fps": 30, "scenes": []}
    for n, (shapes, transparent) in PLATES.items():
        L = illo(shapes, "window")
        ps["scenes"].append({"duration": 1, "background": "transparent" if transparent else "#1E1614", "layers": [L]})
    json.dump(ps, open(os.path.join(here, "src/plates.json"), "w"))
    print(" ".join(PLATES))
else:
    json.dump(spec, open(os.path.join(here, "spec.json"), "w"), indent=1)
