#!/usr/bin/env python3
"""Arctic sea ice, September minimum 1979-2026 (NSIDC Sea Ice Index v4). Builds spec.json.
The frame is 9 million km² of ocean; the white is the ice: cover is calibrated so the share
of white pixels equals extent / 9 (measured from the shader, see CAL)."""
import csv, json, sys
from pathlib import Path

HERE = Path(__file__).parent
CSV = HERE / "N_09_extent_v4.0.csv"
if not CSV.exists(): CSV = HERE.parent / "N_09_extent_v4.0.csv"
rows = [r for r in csv.reader(open(CSV)) if r and r[0].strip().isdigit()]
EXT = {int(r[0]): float(r[4]) for r in rows}
YEARS = sorted(EXT)
assert YEARS[0] == 1979 and len(YEARS) == 48, YEARS

FRAME_KM2 = 9.0
# measured white fraction -> cover uniform (two scales averaged)
CAL = [(0, 0.0), (0.013, 0.1), (0.052, 0.2), (0.123, 0.3), (0.232, 0.4), (0.327, 0.5), (0.456, 0.6), (0.606, 0.7), (0.729, 0.8), (0.842, 0.9), (0.913, 1.0)]
def cover(ext, frame=None):
    f = ext / (frame or FRAME_KM2)
    for (a, ca), (b, cb) in zip(CAL, CAL[1:]):
        if f <= b:
            return round(ca + (cb - ca) * (f - a) / (b - a), 4)
    return 1.0

WATER = "#071520"; ICE_INK = "#0b1d2a"; WHITE = "#f2f6f8"; AMBER = "#ffae5c"
WIDE = {"wght": 900, "opsz": 72}
F_WIDE = "Big Shoulders Stencil"; F_MONO = "Big Shoulders"
PAINT = "#ff5a1f"
PAINTED = dict(blend="multiply", inkTexture={"amount": 0.32, "scale": 38, "seed": 3})

def shader(**u):
    return {"shader": "ice.glsl", "uniforms": u, "supersample": False}

def water(layer):
    """the same ice, water only, laid over paint so leads cut through it"""
    d = json.loads(json.dumps(layer)); d["uniforms"]["hl"] = -2; d["alpha"] = True
    return d

def text(s, x, y, size, color, font=F_WIDE, axes=None, anchor="topLeft", **kw):
    d = {"text": s, "x": x, "y": y, "size": size, "color": color, "font": font, "anchor": anchor}
    if axes: d["axes"] = axes
    d.update(kw)
    return d

def mono(s, x, y, color, size=24, **kw):
    return text(s, x, y, int(size * 1.25), color, font=F_MONO, axes={"wght": 600, "opsz": 24}, lineHeight=1.12, **kw)

def plate(x, y, w, h, **kw):
    d = {"shape": "rect", "x": x + w / 2, "y": y + h / 2, "width": w, "height": h, "fill": "#e8eef2", "opacity": 0.92,
         "cutout": {"edge": "torn", "amount": 3, "rim": 0, "shadow": False, "seed": int(x + y) % 97}}
    d.update(kw)
    return d

scenes = []

# 1 — 1979: close on the pack, all but the bottom edge is ice. The year is cut into it.
c79 = cover(EXT[1979])
sh1 = shader(cover=c79, sun=0.0, scale="=4.2 + t * 0.25", panx=0.0, pany=0.0)
scenes.append({"id": "1979", "duration": 4.5, "camera": {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.06}, "duration": 4.5, "ease": "inOutSine"}]}, "layers": [
    sh1,
    text("1979", 110, 330, 600, PAINT, axes=WIDE, anchor="topLeft", **PAINTED, enter={"preset": "fadeIn", "duration": 0.5, "delay": 0.3}),
    water(sh1),
    mono("September 1979: 7.05 million km² of ice.", 104, 100, ICE_INK, 38, weight=700, anchor="topLeft", shadow={"x": 0, "y": 0, "blur": 5, "color": "#f4f8fa"},
         enter={"preset": "fadeIn", "duration": 0.5, "delay": 1.1}),
]})

# 2 — the years run: pull back, the water opens from the bottom, the year grows into the water it opened.
t0, t1 = 0.6, 8.2
yrs = list(range(1980, 2012))
dt = (t1 - t0) / len(yrs)
cov_keys = [[0, cover(EXT[1979])]] + [[round((t0 + i * dt) / 8.5, 4), cover(EXT[y])] for i, y in enumerate(yrs)] + [[1, cover(EXT[2011])]]
CV = ", ".join(str(cover(EXT[y])) for y in [1979] + yrs)
X = f"clamp((t - {t0}) / {dt:.5f} + 0.5, 0, {len(yrs)})"
L2 = [dict(**shader(yi=f"=clamp((t - {t0}) / {dt:.5f} + 0.5, 0, {len(yrs)})", sun="=t * 0.035", scale="=5.4 + t * 0.42"))]
for i, y in enumerate([1979] + yrs):
    st = 0 if i == 0 else t0 + (i - 1) * dt
    en = t0 if i == 0 else (st + dt if y != 2011 else 8.5)
    lost = 1 - EXT[y] / EXT[1979]
    size = int(150 + 210 * min(1.0, max(0.0, (i / len(yrs)) ** 0.8)))
    L2.append(text(str(y), 110, 330, 600, PAINT, axes=WIDE, anchor="topLeft", start=round(st, 3), end=round(en, 3), sfx=False, **PAINTED))
    L2.append(mono(f"September {y}: {EXT[y]:.2f} million km² of ice", 104, 100, ICE_INK, 38, anchor="topLeft", shadow={"x": 0, "y": 0, "blur": 5, "color": "#f4f8fa"}, sfx=False, start=round(st, 3), end=round(en, 3)))
L2.insert(1 + 2 * len([1979] + yrs) + 1, water(L2[0]))
scenes.append({"id": "melt", "duration": 8.5, "layers": L2})

# 3 — 2012: hard cut, close and cracked; the record number fills the water.
c12 = cover(EXT[2012])
sh3 = shader(cover=c12, sun=0.45, scale="=3.0 + t * 0.12", pany=0.25, panx=2.0, edge=0, crack="=clamp(t * 0.4, 0, 0.25)")
scenes.append({"id": "2012", "duration": 4.5, "transition": "cut", "camera": {"shake": {"amount": 12, "duration": 0.45, "at": 0}, "animate": [{"from": {"zoom": 1.03}, "to": {"zoom": 1.0}, "duration": 4.5, "ease": "outCubic"}]}, "layers": [
    sh3,
    text("3.57", 900, 275, 500, PAINT, axes=WIDE, anchor="center", **PAINTED),
    water(sh3),
    text("2012", 1830, 860, 150, WHITE, axes=WIDE, anchor="bottomRight", enter={"preset": "fadeIn", "duration": 0.3, "delay": 0.25}),
    mono("September 2012: 3.57 million km² of ice,\nthe record low, about half of 1979.", 1836, 1030, WHITE, 34, anchor="bottomRight", align="right",
         enter={"preset": "fadeIn", "duration": 0.4, "delay": 0.7}),
]})

# 4 — the pack cut into 48 bars, one per September; 2012 burns amber.
base_y, top_y = 1080 * (1 - 0.14), 1080 * (1 - 0.80)
lvl79 = base_y - EXT[1979] / 7.67 * (base_y - top_y)
x0, x1 = 1920 * 0.075, 1920 * 0.925
colw = (x1 - x0) / 48
def bx(i): return x0 + (i + 0.5) * colw
L4 = [shader(chart=1, grow="=clamp(t * 60 + 20, 0, 48)", hl="=t > 1.3 ? 33 : -1", scale=26, sun="=0.55 + t * 0.03"),
      {"shape": "rect", "x": (x0 + x1) / 2, "y": lvl79, "width": x1 - x0, "height": 2, "fill": WHITE, "opacity": 0.6,
       "start": 1.2, "enter": {"preset": "wipe", "duration": 0.6}},
      mono("1979 level", x1, lvl79 - 12, WHITE, 28, anchor="bottomRight", start=1.4, enter={"preset": "fadeIn", "duration": 0.3}),
      text("48 Septembers", 144, 64, 96, WHITE, axes=WIDE, anchor="topLeft", letterSpacing="-0.02em",
           enter={"preset": "fadeIn", "duration": 0.3}),
      mono("2012\n3.57", bx(33), base_y - EXT[2013] / 7.67 * (base_y - top_y) - 16, PAINT, 30, anchor="bottom", align="center", start=1.4,
           enter={"preset": "fadeIn", "duration": 0.3})]
for y in (1979, 1990, 2000, 2010, 2026):
    i = y - 1979
    L4.append(mono(str(y), bx(i), base_y + 18, WHITE, 28, anchor="top", align="center", opacity=0.8, start=round(max(0, (i - 20) / 60), 2),
                   enter={"preset": "fadeIn", "duration": 0.25}))
scenes.append({"id": "chart", "duration": 7.0, "transition": "cut", "camera": {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.07}, "duration": 7.0, "ease": "inOutSine"}]}, "layers": L4})

# 5 — now: the frame is 18 million km²; the 2026 pack on the left, and an ice floe the size and shape of
# Greenland (Natural Earth 1:50m, Lambert equal-area, to scale) that melts away, leaving its outline.
GL = json.load(open(HERE / "greenland.json"))
c26w = round(cover(EXT[2026], 18.0) * 0.94, 4)  # measured: 0.27 of the frame white at dusk-free light
sky5 = dict(sun=0.75, scale="=9 + 6 * smoothstep(0, 1.2, t) - t * 0.3", dusk="=0.35 + t * 0.06")
gl_shape = {"shape": "path", "d": GL["d"], "x": GL["cx"], "y": GL["cy"], "width": GL["w"], "height": GL["h"]}
scenes.append({"id": "now", "duration": 5.5, "transition": {"type": "fade", "duration": 0.6}, "layers": [
    shader(cover=c26w, edge=-90, **sky5),
    dict(shader(cover="=clamp(1 - (t - 1.9) / 2.4, 0, 1)", edge=0, **sky5), mask=dict(gl_shape, fill=WHITE), start=1.2, enter={"preset": "fadeIn", "duration": 0.4}),
    dict(gl_shape, fill="none", stroke={"color": PAINT, "width": 3}, start=1.6, enter={"preset": "fadeIn", "duration": 0.5}),
    text("Missing\nsince 1979:\na Greenland.", 1840, 100, 104, WHITE, axes=WIDE, anchor="topRight", align="right", lineHeight=0.92,
         kinetic="mask by line up 0.6s 80ms outExpo", start=0.3),
    mono("Greenland, to scale: 2.17 million km².\nSeptember ice lost since 1979: 2.24.", 1840, 470, WHITE, 32, anchor="topRight", align="right", start=1.0,
         enter={"preset": "fadeIn", "duration": 0.4}),
    mono("Data: NSIDC Sea Ice Index v4. Coastline: Natural Earth.", 1840, 1040, WHITE, 22, anchor="bottomRight", align="right",
         opacity=0.8, start=1.6, enter={"preset": "fadeIn", "duration": 0.4}),
]})


spec = {
    "size": [1920, 1080], "fps": 30, "intent": {"pace": "slow"},
    "background": WATER,
    "music": {"style": "ambient", "bpm": 64, "key": "Dm", "mood": "cold sparse"},
    "sound": {"palette": "organic", "ambience": "wind", "density": 0.35, "cues": [
        {"sfx": "crunch", "at": 4.5}, {"sfx": "shatter", "at": 13.0, "gain": -2}, {"sfx": "crunch", "at": 17.5, "gain": -4},
        {"sfx": "droplet", "at": 24.6, "gain": -6}]},
    "post": {"grain": 0.03, "vignette": 0.18},
    "transition": "cut",
    "scenes": scenes,
}
json.dump(spec, open(HERE / "spec.json", "w"), indent=1, ensure_ascii=False)
print("covers", c79, c12, c26w, "total", sum(s["duration"] for s in scenes))
