#!/usr/bin/env python3
"""Arctic sea ice, September minimum 1979-2026 (NSIDC Sea Ice Index v4). Builds spec.json.
The frame is 8 million km² of ocean; the white is the ice: cover is calibrated so the share
of white pixels equals extent / 8 (measured from the shader, see CAL)."""
import csv, json, sys
from pathlib import Path

HERE = Path(__file__).parent
CSV = HERE / "N_09_extent_v4.0.csv"
if not CSV.exists(): CSV = HERE.parent / "N_09_extent_v4.0.csv"
rows = [r for r in csv.reader(open(CSV)) if r and r[0].strip().isdigit()]
EXT = {int(r[0]): float(r[4]) for r in rows}
YEARS = sorted(EXT)
assert YEARS[0] == 1979 and len(YEARS) == 48, YEARS

FRAME_KM2 = 8.0
# measured white fraction -> cover uniform (two scales averaged)
CAL = [(0.0, 0.0), (0.015, 0.1), (0.06, 0.2), (0.13, 0.3), (0.24, 0.4), (0.34, 0.5), (0.48, 0.6), (0.62, 0.7), (0.74, 0.8), (0.85, 0.9), (0.92, 1.0)]
def cover(ext):
    f = ext / FRAME_KM2
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
scenes.append({"id": "1979", "duration": 4.5, "layers": [
    sh1,
    text("1979", 110, 400, 600, PAINT, axes=WIDE, anchor="topLeft", **PAINTED, enter={"preset": "fadeIn", "duration": 0.5, "delay": 0.3}),
    water(sh1),
    plate(84, 92, 880, 118, enter={"preset": "fadeIn", "duration": 0.5, "delay": 1.1}),
    mono("September, the month the Arctic Ocean’s ice is smallest.\n7.05 million km² of it in 1979.", 104, 110, ICE_INK, 36,
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
    L2.append(text(str(y), 110, 400, 600, PAINT, axes=WIDE, anchor="topLeft", start=round(st, 3), end=round(en, 3), sfx=False, **PAINTED))
    if i == 0: L2.append(plate(84, 96, 620, 66, sfx=False))
    L2.append(mono(f"September {y}: {EXT[y]:.2f} million km² of ice", 104, 110, ICE_INK, 36, anchor="topLeft", sfx=False, start=round(st, 3), end=round(en, 3)))
L2.append(mono("White is ice. The frame holds\n8 million km² of ocean.", 1830, 110, ICE_INK, 32, anchor="topRight", align="right", enter={"preset": "fadeIn", "duration": 0.5, "delay": 0.3}, exit={"preset": "fadeOut", "duration": 0.3}, end=4.0))
L2.insert(1 + 2 * len([1979] + yrs) + 1, water(L2[0]))
scenes.append({"id": "melt", "duration": 8.5, "layers": L2})

# 3 — 2012: hard cut, close and cracked; the record number fills the water.
c12 = cover(EXT[2012])
sh3 = shader(cover=c12, sun=0.45, scale="=3.0 + t * 0.12", pany=0.25, panx=2.0, edge=0, crack="=clamp(t * 0.5, 0, 0.35)")
scenes.append({"id": "2012", "duration": 4.5, "transition": "cut", "layers": [
    sh3,
    text("3.57", 900, 300, 600, PAINT, axes=WIDE, anchor="center", **PAINTED),
    water(sh3),
    text("2012", 1830, 860, 150, WHITE, axes=WIDE, anchor="bottomRight", enter={"preset": "fadeIn", "duration": 0.3, "delay": 0.25}),
    mono("million km² in September 2012, the smallest on record:\nabout half the ice of 1979.", 1836, 1030, WHITE, 34, anchor="bottomRight", align="right",
         enter={"preset": "fadeIn", "duration": 0.4, "delay": 0.7}),
]})

# 4 — the pack cut into 48 bars, one per September; 2012 burns amber.
base_y, top_y = 1080 * (1 - 0.14), 1080 * (1 - 0.80)
lvl79 = base_y - EXT[1979] / 7.67 * (base_y - top_y)
x0, x1 = 1920 * 0.075, 1920 * 0.925
colw = (x1 - x0) / 48
def bx(i): return x0 + (i + 0.5) * colw
L4 = [shader(chart=1, grow="=clamp(t * 20 + 30, 0, 48)", hl="=t > 1.3 ? 33 : -1", scale=26, sun="=0.55 + t * 0.03"),
      {"shape": "rect", "x": (x0 + x1) / 2, "y": lvl79, "width": x1 - x0, "height": 2, "fill": WHITE, "opacity": 0.6,
       "start": 1.2, "enter": {"preset": "wipe", "duration": 0.6}},
      mono("1979 level", x1, lvl79 - 12, WHITE, 28, anchor="bottomRight", start=1.4, enter={"preset": "fadeIn", "duration": 0.3}),
      text("48 Septembers", 144, 64, 96, WHITE, axes=WIDE, anchor="topLeft", letterSpacing="-0.02em",
           enter={"preset": "fadeIn", "duration": 0.3}),
      mono("2012\n3.57", bx(33), base_y - EXT[2013] / 7.67 * (base_y - top_y) - 16, PAINT, 30, anchor="bottom", align="center", start=1.4,
           enter={"preset": "fadeIn", "duration": 0.3})]
for y in (1979, 1990, 2000, 2010, 2026):
    i = y - 1979
    L4.append(mono(str(y), bx(i), base_y + 18, WHITE, 28, anchor="top", align="center", opacity=0.8, start=round(max(0, (i - 30) / 20), 2),
                   enter={"preset": "fadeIn", "duration": 0.25}))
scenes.append({"id": "chart", "duration": 7.0, "transition": "cut", "camera": {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.07}, "duration": 7.0, "ease": "inOutSine"}]}, "layers": L4})

# 5 — now: wide, low sun, the pack at its 2026 size; the line in the water.
c26 = cover(EXT[2026])
scenes.append({"id": "now", "duration": 5.5, "transition": {"type": "fade", "duration": 0.6}, "layers": [
    shader(cover=0.66, sun=0.75, scale="=19 - t * 0.5", dusk="=0.35 + t * 0.06", edge=-70),
    text("Missing\nsince 1979:\na Greenland.", 1830, 110, 88, WHITE, axes=WIDE, anchor="topRight", align="right", lineHeight=0.92, letterSpacing="-0.02em",
         kinetic="mask by line up 0.6s 80ms outExpo", start=0.4),
    {"shape": "path", "d": "M18 12 L30 5 L45 1 L62 0 L78 2 L92 6 L100 12 L94 17 L98 24 L92 31 L86 38 L80 46 L76 52 L70 58 L64 66 L58 74 L52 82 L46 90 L41 100 L36 96 L32 90 L28 82 L24 74 L20 66 L15 59 L10 53 L7 47 L10 42 L5 36 L3 30 L8 25 L4 19 L10 14 Z", "x": 1620, "y": 740, "width": 290, "height": 390, "fill": "#ff5a1f30", "stroke": {"color": PAINT, "width": 4, "dash": [12, 7]},
     "trimEnd": 0, "start": 1.0, "animate": [{"to": {"trimEnd": 1}, "duration": 2.2, "ease": "inOutCubic"}]},
    mono("2.24 million km² of September\nice lost since 1979.\nGreenland: 2.17.", 1836, 420, WHITE, 30, anchor="topRight", align="right", start=1.2,
         enter={"preset": "fadeIn", "duration": 0.4}),
    mono("Source: NSIDC Sea Ice Index v4 (G02135),\nSeptember mean extent, 1979–2026.", 1836, 1030, WHITE, 24, anchor="bottomRight", align="right",
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
print("covers", c79, c12, c26, "total", sum(s["duration"] for s in scenes))
