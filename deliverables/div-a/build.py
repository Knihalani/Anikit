#!/usr/bin/env python3
"""Generates halde.json: Halde & Sons — washed Ethiopian Guji release (1080x1080, 20 s, calm).

Concept: the kraft coffee bag as the canvas. One ink hand draws the release onto it:
the Guji highlands as topographic contours, a wash of water, the three tasting notes,
a small batch of beans, and finally the contours return to frame the roaster's name.
"""
import json, math, os

INK = "#24170F"
KRAFT = "#C79E70"
PEACH = "#E07A4A"
BERG = "#B9A93A"
TEA = "#5C2A1A"
WATER = "#4E6E78"
BEAN = "#6A3D22"

MONO = "JetBrains Mono"
SANS = "Inter Display"


def paper():
    return {"component": "paper", "props": {"stock": "kraft", "color": KRAFT, "tooth": 0.55, "fibers": 0.45, "vignette": 0.35, "seed": 7}}


def ring(cx, cy, r, k, wob=0.12, n=72):
    pts = []
    for j in range(n + 1):
        th = 2 * math.pi * j / n
        rr = r * (1 + wob * math.sin(3 * th + k * 0.45) + 0.6 * wob * math.sin(5 * th + 1.3 + k * 0.2)
                  + 0.35 * wob * math.cos(2 * th - k * 0.3))
        pts.append([round(cx + rr * math.cos(th) + k * 3.0, 1), round(cy + rr * math.sin(th) * 0.86 - k * 2.0, 1)])
    return pts


def contours(cx, cy, n, r0, step, color=INK, size=2.6, seed=1, wob=0.12):
    return [{"points": ring(cx, cy, r0 + step * k, k + seed, wob), "color": color, "size": size,
             "pressure": "even", "seed": seed + k} for k in range(n)]


def text(t, x, y, size, font=SANS, weight=700, color=INK, anchor="topLeft", start=0, enter="mask by line up", **kw):
    d = {"text": t, "font": font, "size": size, "weight": weight, "color": color, "x": x, "y": y, "anchor": anchor}
    if start:
        d["start"] = start
    if enter:
        d["enter"] = enter
    d.update(kw)
    return d


def sketch(paths, draw, stagger, delay=0.0, boil=8, texture="ink", **kw):
    return {"component": "sketch", "props": {"paths": paths, "draw": draw, "stagger": stagger, "delay": delay,
                                              "boil": boil, "boilAmount": 1.1, "texture": texture, "color": INK, **kw}}


def drift(dur, z0=1.045, z1=1.0, x0=0, x1=0):
    return {"animate": [{"from": {"zoom": z0, "x": x0}, "to": {"zoom": z1, "x": x1}, "duration": dur, "ease": "outCubic"}]}


def disc(cx, cy, r):
    # a ring of radius r/2 drawn r wide fills a disc
    h = r / 2
    return f"M{cx + h:.1f} {cy:.1f} A {h:.1f} {h:.1f} 0 1 1 {cx + h - 0.5:.1f} {cy - 1.5:.1f}"


def circle_d(cx, cy, r):
    return f"M{cx + r:.1f} {cy:.1f} A {r:.1f} {r:.1f} 0 1 1 {cx + r - 0.6:.1f} {cy - 2:.1f}"


# ---------- Scene 1: Guji highlands as contours ----------
s1 = [paper(),
      sketch(contours(640, 430, 10, 34, 34, seed=1) + contours(165, 640, 4, 22, 26, seed=20, wob=0.16),
             draw=1.0, stagger=0.09, delay=0.0, size=2.6),
      # peach pin on the summit
      {"component": "brush", "props": {"d": disc(652, 418, 30), "texture": "marker", "color": PEACH, "size": 30,
                                       "pressure": "even", "draw": 0.4, "delay": 1.1}},
      sketch([{"d": "M664 432 C 720 580, 780 700, 838 788"}], draw=0.6, stagger=0, delay=1.4, size=3, pressure="taper"),
      text("GUJI\nETHIOPIA", 820, 800, 30, font=MONO, weight=700, start=1.8, enter="type", align="left", lineHeight=1.15),
      text("HALDE & SONS — NEW RELEASE", 90, 90, 28, font=MONO, weight=600, enter="type"),
      text("A new\nsingle origin.", 90, 880, 88, anchor="left", start=0.15, align="left", lineHeight=1.0, letterSpacing="-0.02em"),
      ]

# ---------- Scene 2: washed ----------
s2 = [paper(),
      {"component": "brush", "props": {"d": "M-60 660 C 240 600, 520 710, 780 640 S 1040 600, 1160 650",
                                       "texture": "dry", "color": WATER, "size": 300, "pressure": "swell",
                                       "draw": 1.1, "opacity": 0.9, "seed": 4}},
      ]
for i, (cx, cy, d0) in enumerate([(760, 650, 0.9), (330, 680, 1.5), (900, 620, 2.1)]):
    s2.append({"shape": "circle", "width": 60, "height": 60, "fill": "none", "stroke": "#EADFCB", "strokeWidth": 3,
               "x": cx, "y": cy, "scaleY": 0.35, "start": d0,
               "animate": [{"from": {"scale": 0.2, "opacity": 0.9}, "to": {"scale": 6, "opacity": 0}, "duration": 2.2, "ease": "outCubic"}]})
s2 += [text("Washed.", 90, 170, 190, letterSpacing="-0.04em", lineHeight=1.0),
       text("Ethiopian Guji, washed process", 92, 400, 40, weight=500, start=0.6),
       text("02 / PROCESS", 90, 90, 28, font=MONO, weight=600, enter="type")]

# ---------- Scene 3: tasting notes ----------
cols = [200, 540, 880]
cy = 560
# bergamot
bx = cols[0]
berg_fill = {"component": "brush", "props": {"d": disc(bx, cy, 118), "texture": "marker", "color": BERG, "size": 118,
                                              "pressure": "even", "draw": 0.6, "delay": 0.2, "seed": 3}}
seg = [{"d": f"M{bx:.1f} {cy:.1f} L {bx + 104 * math.cos(a):.1f} {cy + 104 * math.sin(a):.1f}", "size": 2.2}
       for a in [i * 2 * math.pi / 9 + 0.2 for i in range(9)]]
berg_ink = sketch([{"d": circle_d(bx, cy, 118), "size": 4.5, "pressure": "even"},
                   {"d": circle_d(bx, cy, 106), "size": 2, "pressure": "even"}] + seg +
                  [{"d": f"M{bx + 10} {cy - 118} C {bx + 20} {cy - 150}, {bx + 40} {cy - 160}, {bx + 60} {cy - 170}", "size": 4},
                   {"d": f"M{bx + 30} {cy - 150} C {bx + 70} {cy - 200}, {bx + 120} {cy - 180}, {bx + 140} {cy - 150} C {bx + 100} {cy - 130}, {bx + 60} {cy - 130}, {bx + 30} {cy - 150}", "size": 3}],
                  draw=0.6, stagger=0.06, delay=0.5)
# peach
px = cols[1]
peach_shape = (f"M{px} {cy - 95} C {px - 40} {cy - 130}, {px - 125} {cy - 100}, {px - 125} {cy - 5} "
               f"C {px - 125} {cy + 85}, {px - 60} {cy + 125}, {px} {cy + 125} "
               f"C {px + 60} {cy + 125}, {px + 125} {cy + 85}, {px + 125} {cy - 5} "
               f"C {px + 125} {cy - 100}, {px + 40} {cy - 130}, {px} {cy - 95}")
peach_fill = {"component": "brush", "props": {"d": disc(px, cy + 10, 120),
                                               "texture": "marker", "color": PEACH, "size": 120, "pressure": "even",
                                               "draw": 0.6, "delay": 1.8, "seed": 5}}
peach_hatch = {"component": "hatch", "props": {"d": peach_shape, "style": "hatch", "toneFrom": 0.0, "toneTo": 0.75,
                                                "light": 225, "spacing": 8, "width": 1.4, "color": INK, "draw": 1.0, "boil": 8}, "start": 2.4}
peach_ink = sketch([{"d": peach_shape, "size": 4.5, "pressure": "even"},
                    {"d": f"M{px} {cy - 92} C {px - 18} {cy - 20}, {px - 14} {cy + 60}, {px + 6} {cy + 118}", "size": 2.6, "pressure": "taper"},
                    {"d": f"M{px} {cy - 100} C {px + 4} {cy - 125}, {px + 10} {cy - 140}, {px + 16} {cy - 150}", "size": 5},
                    {"d": f"M{px + 10} {cy - 140} C {px + 50} {cy - 185}, {px + 110} {cy - 170}, {px + 125} {cy - 135} C {px + 85} {cy - 115}, {px + 40} {cy - 115}, {px + 10} {cy - 140}", "size": 3}],
                   draw=0.7, stagger=0.15, delay=2.0)
# black tea: three leaves
tx = cols[2]


def leaf(x, y, L, W, ang):
    c, s = math.cos(ang), math.sin(ang)
    def P(u, v):
        return f"{x + u * c - v * s:.1f} {y + u * s + v * c:.1f}"
    outline = f"M{P(0, 0)} C {P(L * 0.3, -W)}, {P(L * 0.75, -W * 0.8)}, {P(L, 0)} C {P(L * 0.75, W * 0.8)}, {P(L * 0.3, W)}, {P(0, 0)}"
    rib = f"M{P(-L * 0.12, 0)} L {P(L * 0.95, 0)}"
    veins = [f"M{P(L * t, 0)} Q {P(L * (t + 0.08), side * W * 0.35)} {P(L * (t + 0.16), side * W * 0.62)}"
             for t in (0.2, 0.38, 0.56, 0.72) for side in (-1, 1)]
    return outline, rib, veins


tea_paths, tea_fills = [], []
for (lx, ly, L, W, ang, d) in [(tx - 20, cy + 110, 210, 62, -1.25, 3.5), (tx - 10, cy + 100, 170, 50, -0.55, 3.75), (tx - 30, cy + 105, 150, 46, -2.0, 4.0)]:
    o, r, v = leaf(lx, ly, L, W, ang)
    tea_fills.append({"component": "brush", "props": {"d": o, "texture": "marker", "color": TEA, "size": 46, "pressure": "even",
                                                       "draw": 0.5, "delay": d, "opacity": 0.85, "seed": 9}})
    tea_paths += [{"d": o, "size": 3.5, "pressure": "even", "delay": d - 3.6}, {"d": r, "size": 2.4, "delay": d - 3.6}] + \
                 [{"d": vv, "size": 1.6, "delay": d - 3.6} for vv in v]
tea_ink = sketch(tea_paths, draw=0.6, stagger=0.02, delay=3.7)

s3 = [paper(), {"layers": [berg_fill, berg_ink, peach_fill, peach_ink] + tea_fills + [tea_ink], },
      text("In the cup", 90, 150, 92, letterSpacing="-0.02em"),
      text("03 / TASTING NOTES", 90, 90, 28, font=MONO, weight=600, enter="type"),
      text("BERGAMOT", cols[0], 790, 36, font=MONO, weight=700, anchor="top", start=0.9, enter="rise"),
      text("PEACH", cols[1], 790, 36, font=MONO, weight=700, anchor="top", start=2.6, enter="rise"),
      text("BLACK TEA", cols[2], 790, 36, font=MONO, weight=700, anchor="top", start=4.3, enter="rise"),
      ]

# ---------- Scene 4: small batches ----------
beans_ink, beans_fill = [], []
k = 0
for row in range(3):
    for col in range(5):
        bx_ = 150 + col * 195 + (row % 2) * 40
        by_ = 300 + row * 175
        ang = math.radians(((col * 37 + row * 53) % 50) - 25)
        rx, ry = 52, 72
        pts = []
        for j in range(49):
            th = 2 * math.pi * j / 48
            u, v = rx * math.cos(th), ry * math.sin(th)
            pts.append([round(bx_ + u * math.cos(ang) - v * math.sin(ang), 1), round(by_ + u * math.sin(ang) + v * math.cos(ang), 1)])
        def Q(u, v):
            return f"{bx_ + u * math.cos(ang) - v * math.sin(ang):.1f} {by_ + u * math.sin(ang) + v * math.cos(ang):.1f}"
        crease = f"M{Q(0, -ry * 0.82)} C {Q(18, -ry * 0.3)}, {Q(-18, ry * 0.3)}, {Q(0, ry * 0.82)}"
        delay = k * 0.11
        beans_fill.append({"component": "brush", "props": {"points": [[float(c) for c in Q(rx*0.42*math.cos(2*math.pi*j/24), ry*0.48*math.sin(2*math.pi*j/24)).split()] for j in range(25)], "texture": "marker",
                                                            "color": BEAN, "size": 50, "pressure": "even", "draw": 0.3,
                                                            "delay": delay + 0.25, "seed": 11 + k, "opacity": 0.9}})
        beans_ink.append({"points": pts, "size": 3.4, "pressure": "even", "delay": 0})
        beans_ink.append({"d": crease, "size": 3, "pressure": "taper", "delay": -0.11 * 0 + 0.0})
        k += 1
# sketch stagger is per path index; two paths per bean -> stagger 0.055 gives 0.11 per bean
s4 = [paper()] + beans_fill + [sketch(beans_ink, draw=0.45, stagger=0.055, delay=0.0),
      text("Roasted in\nsmall batches.", 90, 900, 84, anchor="left", start=0.9, align="left", lineHeight=1.0, letterSpacing="-0.02em"),
      text("04 / ROAST", 90, 90, 28, font=MONO, weight=600, enter="type")]

# ---------- Scene 5: end card, the contours return ----------
s5 = [paper(),
      sketch(contours(540, 530, 7, 330, 22, color=PEACH, size=3.2, seed=3, wob=0.07), draw=1.2, stagger=0.08, delay=0.0, boil=6),
      text("Halde & Sons", 540, 470, 118, anchor="center", letterSpacing="-0.03em", start=0.3, enter="rise"),
      text("WASHED ETHIOPIAN GUJI", 540, 580, 34, font=MONO, weight=700, anchor="center", start=0.7, enter="type"),
      text("Bergamot · Peach · Black tea", 540, 640, 34, weight=500, anchor="center", start=1.0, enter="rise"),
      text("Now roasting.", 540, 980, 40, weight=600, anchor="center", start=1.3, enter="rise"),
      ]

T = 0.6
scenes = [
    {"id": "origin", "duration": 4.3, "layers": s1, "camera": drift(4.3, 1.06, 1.0)},
    {"id": "washed", "duration": 3.9, "layers": s2, "transition": {"type": "crossfade", "duration": T}, "camera": drift(3.9, 1.0, 1.04)},
    {"id": "notes", "duration": 7.0, "layers": s3, "transition": {"type": "crossfade", "duration": T}, "camera": drift(7.0, 1.03, 1.0)},
    {"id": "batch", "duration": 3.6, "layers": s4, "transition": {"type": "crossfade", "duration": T}, "camera": drift(3.6, 1.0, 1.035)},
    {"id": "end", "duration": 3.6, "layers": s5, "transition": {"type": "crossfade", "duration": T}, "camera": drift(3.6, 1.03, 1.0)},
]

spec = {
    "size": [1080, 1080],
    "fps": 30,
    "background": KRAFT,
    "intent": {"pace": "calm", "genre": "launch"},
    "music": {"style": "acoustic", "bpm": 72, "key": "D", "mood": "warm calm", "instrumentation": "acoustic",
              "instruments": {"lead": "nylon-guitar"}, "end": "ring"},
    "post": {"vignette": 0.25, "contrast": 1.03},
    "scenes": scenes,
}
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "halde.json")
with open(out, "w") as f:
    json.dump(spec, f, indent=1)
print(out, sum(s["duration"] for s in scenes) - T * 4)
