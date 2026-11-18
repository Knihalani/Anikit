#!/usr/bin/env python3
"""Offset (design-systems conference, Lisbon) opening titles, 15 s 16:9.
Device: offset printing. Plates misregister and land in register; the word becomes
a running-bond (offset) tile wall; the lockup is printed onto cobalt."""
import json, os
PAPER, INK, COBALT = "#efe9dc", "#16140f", "#1d3fb8"
CY, MA, YE = "#00a2df", "#e5007d", "#ffd200"
F = "Bricolage Grotesque"
TRAP = {"opsz": 12}
L = []
def add(**k): L.append(k); return k

# ---- registration marks (they stay all film: the printer's sheet)
def mark(x, y, start=0.0):
    for w, h in ((70, 2), (2, 70)):
        add(shape="rect", x=x, y=y, width=w, height=h, fill=INK, start=start, end=10.2,
            animate=[{"from": {"scaleX" if w > h else "scaleY": 0}, "duration": 0.5, "ease": "outExpo"}])
    add(shape="circle", x=x, y=y, width=34, height=34, fill="none", stroke=INK, strokeWidth=2, start=start, end=10.2,
        animate=[{"from": {"trimEnd": 0}, "duration": 0.6, "ease": "inOutQuart"}])


def label(text, start, end, x="92vw", y="8vh", anchor="right", color=INK, align="right"):
    add(text=text, font="IBM Plex Mono", size="1.9vh", weight=500, letterSpacing="0.14em", color=color, align=align,
        x=x, y=y, anchor=anchor, start=start, end=end, features="tnum",
        enter={"preset": "type", "duration": 0.5})

# ---- A (0–2.6): one plate. A giant cropped O, cobalt, width opening up; a cyan plate drifts under it.

for col, dx, blend, st in ((CY, 38, "multiply", 0.0), (COBALT, 0, "multiply", 0.0)):
    add(text="Of", font=F, size="118vh", weight=800, axes={"wdth": 75, "opsz": 12}, color=col, blend=blend, lineHeight=0.8,
        align="left", x=f"={-60 + dx} - 140*t", y="52vh", anchor="left", start=st, end=2.75,
        animate=[{"keyframes": {"axes.wdth": [75, 100], "axes.wght": [300, 800]}, "duration": 2.6, "ease": "inOutQuart"},
                 {"to": {"opacity": 0}, "at": 2.55, "duration": 0.2}])

# ---- B (2.6–6.2): CMY plates of the word, misregistered, land in register with anticipation/overshoot; key plate slams.

plates = [(YE, (-140, 90), -3), (MA, (120, -70), 2), (CY, (-60, -110), -1.5)]
for i, (col, (dx, dy), rot) in enumerate(plates):
    land = 4.75 + i * 0.1
    x0, y0 = 960, 540
    add(text="Offset", font=F, size="40vh", weight=800, axes={"wdth": 100, "opsz": 12}, letterSpacing="-0.035em", color=col, blend="multiply",
        start=2.6, end=6.45, x=x0, y=y0,
        animate=[{"from": {"x": x0 + dx * 4, "y": y0 + dy * 4, "rotation": rot * 3, "opacity": 0}, "to": {"x": x0 + dx, "y": y0 + dy, "rotation": rot, "opacity": 1}, "duration": 0.6, "ease": "outQuart"},
                 {"to": {"x": x0 + dx * 1.25, "y": y0 + dy * 1.25, "rotation": rot * 1.3}, "at": 0.6, "duration": land - 2.6 - 0.9, "ease": "inOutSine"},
                 {"to": {"x": x0, "y": y0, "rotation": 0}, "at": land - 2.6 - 0.3, "duration": 0.3, "ease": "outBack"}])
key = add(text="Offset", font=F, size="40vh", weight=800, axes={"wdth": 100, "opsz": 12}, letterSpacing="-0.035em", color=COBALT, blend="multiply",
          start=5.15, end=6.1, x=960, y=540,
          animate=[{"from": {"scale": 1.18, "opacity": 0}, "duration": 0.12, "ease": "inExpo"},
                   {"keyframes": {"scale": [1, 0.985, 1]}, "at": 0.12, "duration": 0.25, "ease": "outCubic"},
                   {"to": {"scale": 0.28, "y": 410}, "at": 0.6, "duration": 0.35, "ease": "inExpo"}])
# B fades out its colour plates into the wall
# ---- C (6.2–10.4): the word becomes a running-bond wall (rows offset by half a word), breathing in width.
rows = 9
rh = 1080 / (rows - 1)
wall_start = 5.85
for r in range(rows):
    y = r * rh
    band = r % 2 == 1
    if band:
        add(shape="rect", x="50vw", y=y, width="104vw", height=rh, fill=COBALT, start=wall_start, end=10.9,
            animate=[{"from": {"scaleX": 0}, "duration": 0.45, "at": abs(r - 4) * 0.04, "ease": "outExpo"},
                     {"to": {"scaleY": 0}, "at": 4.3 + r * 0.03, "duration": 0.3, "ease": "inExpo"}])
    shift = 0 if r % 2 == 0 else 190
    speed = (40 + 25 * (r % 4)) * (1 if r % 2 == 0 else -1)
    wd = [75, 88, 100][r % 3]
    add(text="  ".join(["OFFSET"] * 9), font=F, size=f"{rh * 0.74:.0f}", weight=900, axes={"wdth": wd, "opsz": 12},
        typeset=False, width=7000, color=PAPER if band else COBALT, lineHeight=0.9, letterSpacing="0.01em",
        x=f"={960 + shift} + {speed}*t*t*0.25", y=y + rh * 0.04, start=wall_start, end=10.9,
        animate=[{"from": {"opacity": 0, "y": f"+{40 if r % 2 else -40}"}, "at": abs(r - 4) * 0.05, "duration": 0.5, "ease": "outQuart"},
                 {"keyframes": {"axes.wdth": [wd, 175 - wd, wd]}, "at": 0.5, "duration": 3.6, "ease": "inOutSine"},
                 {"to": {"color": YE if band else INK}, "at": 1.9 + r * 0.07, "duration": 0.12, "ease": "linear"},
                 {"to": {"x": f"+{2400 if r % 2 else -2400}"}, "at": 4.0 + abs(r - 4) * 0.05, "duration": 0.55, "ease": "inExpo"}])


# ---- D (10.4–15): printed onto cobalt. Lockup flush left on a grid; plates settle in; one plate slips at the end.
add(shape="rect", x="50vw", y="50vh", width="100vw", height="100vh", fill=COBALT, start=10.23, end=15)
RES = {"by": "char", "axes": {"wdth": 75, "wght": 250}, "order": "random", "seed": 4, "smooth": 0.12, "duration": 2.4, "delay": 0.15, "ease": "outExpo"}
add(text="Offset", font=F, size="42vh", weight=800, axes={"wdth": 100, "opsz": 12}, letterSpacing="-0.035em", color=YE,
    align="left", x="4.5vw", y="64vh", anchor="left", start=10.22, end=15, animator=RES,
    animate=[{"from": {"opacity": 0}, "duration": 0.12},
             {"from": {"x": "+0", "y": "+0"}, "to": {"x": "+14", "y": "+10"}, "at": 1.6, "duration": 0.5, "ease": "outBack"},
             {"to": {"x": "+64", "y": "+38"}, "at": 3.7, "duration": 0.45, "ease": "outBack"}])
add(text="Offset", font=F, size="42vh", weight=800, axes={"wdth": 100, "opsz": 12}, letterSpacing="-0.035em", color=PAPER,
    align="left", x="4.5vw", y="64vh", anchor="left", start=10.22, end=15, animator=RES,
    animate=[{"from": {"opacity": 0}, "duration": 0.12}])
add(shape="rect", x="6.2vw", y="27vh", anchor="left", width="88vw", height=2, fill=PAPER, start=11.3, end=15,
    animate=[{"from": {"scaleX": 0}, "duration": 0.8, "ease": "inOutExpo"}])
def info(text, x, start, size="3.8vh", font=F, weight=600, ls="-0.005em", upper=False):
    add(text=text, font=font, size=size, weight=weight, letterSpacing=ls, color=PAPER, align="left", x=x, y="12vh",
        anchor="topLeft", start=start, end=15, features="tnum",
        animate=[{"from": {"y": "+18", "opacity": 0}, "duration": 0.5, "ease": "outQuart"}])
info("Design Systems\nConference", "6.2vw", 11.55)
info("Lisboa\nPortugal", "38vw", 11.7)
info("14–15 May\n2027", "62vw", 11.85)
add(text="One system, every offset.", font="Newsreader", italic=True, size="3.6vh", weight=400, color=PAPER, align="right",
    x="94vw", y="92vh", anchor="right", start=12.3, end=15, enter={"preset": "type", "duration": 0.4})
L.append({"shader": "print-offset", "filter": True, "width": "100%", "height": "100%",
          "uniforms": {"amount": 2.2, "angle": 25, "fibre": 1.2, "paper": [0.94, 0.91, 0.86]}})

spec = {
    "size": "1920x1080" if False else "1080p", "fps": 30,
    "post": {"grain": 0.035, "grainSize": 1.3, "vignette": 0.12},
    "music": {"style": "tech", "bpm": 120, "key": "Dm", "mood": "precise mechanical", "seed": 7,
              "sections": ["intro:1", "build:2", "drop:3", "outro:2"], "hits": ["@5.05", "@10.55"], "end": "ring"},
    "scenes": [{"duration": 15, "background": PAPER,
                "camera": {"shake": {"amount": 9, "at": 5.15, "duration": 0.25}, "animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.035}, "duration": 6.2, "ease": "linear"},
                                       {"to": {"zoom": 1.55}, "at": 5.85, "duration": 0.01},
                                       {"to": {"zoom": 1.0}, "at": 5.9, "duration": 4.3, "ease": "outCubic"},
                                       {"to": {"zoom": 1.03}, "at": 10.45, "duration": 4.6, "ease": "outSine"}]},
                "layers": L}],
}
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "spec.json")
json.dump(spec, open(out, "w"), indent=1, ensure_ascii=False)
print("wrote", out, len(L), "layers")
