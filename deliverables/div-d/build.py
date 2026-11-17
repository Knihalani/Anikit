#!/usr/bin/env python3
"""Generates burrow.json (the Anikit Video spec) for the Burrow 20 s film.

Concept: a picture-book page that listens. A child's reading voice moves word by
word along the page; each word it reaches wakes the illustration (fox lands, digs,
the tunnel draws, the moon rises). Then three quick pages fire on one word each,
and the fox peeks through the "o" of the logo.
Run: python3 build.py && anikit-video preview burrow.json
"""
import json, math, random, re
from PIL import ImageFont

W, H = 1920, 1080
INK = "#1D1A3A"
UNREAD = "#7D7894"
MARK = "#FFD23F"
FOX = "#F26B2A"
FOX_DK = "#C9481A"
CREAM = "#FFF1DC"
DARK = "#24120A"
WHITE = "#FFFFFF"
BODY = "Bricolage Grotesque"
LOGO = "Boldonse"

# ---------------------------------------------------------------- helpers

def bbox(d):
    """Bounding box of an absolute M/L/C/Q/Z path (curves sampled)."""
    toks = re.findall(r"[MLCQZmlcqz]|-?\d+\.?\d*", d)
    pts, cur, i, cmd = [], (0, 0), 0, None
    while i < len(toks):
        if toks[i] in "MLCQZ":
            cmd = toks[i]; i += 1
            if cmd == "Z":
                continue
        n = {"M": 1, "L": 1, "C": 3, "Q": 2}[cmd]
        ps = [(float(toks[i + 2 * k]), float(toks[i + 2 * k + 1])) for k in range(n)]
        i += 2 * n
        if n == 1:
            pts.append(ps[0])
        else:
            ctrl = [cur] + ps
            for s in range(21):
                u = s / 20
                c = ctrl[:]
                while len(c) > 1:
                    c = [((1 - u) * a[0] + u * b[0], (1 - u) * a[1] + u * b[1]) for a, b in zip(c, c[1:])]
                pts.append(c[0])
        cur = ps[-1]
        if cmd == "M":
            cmd = "L"
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return min(xs), min(ys)


def path(d, fill, **kw):
    bx, by = bbox(d)
    lay = {"shape": "path", "d": d, "fill": fill, "x": round(bx, 1), "y": round(by, 1), "anchor": "topLeft"}
    lay.update(kw)
    return lay


def circ(cx, cy, w, h, fill, **kw):
    lay = {"shape": "circle", "width": w, "height": h, "fill": fill, "x": cx, "y": cy}
    lay.update(kw)
    return lay


def rect(cx, cy, w, h, fill, **kw):
    lay = {"shape": "rect", "width": w, "height": h, "fill": fill, "x": cx, "y": cy}
    lay.update(kw)
    return lay


def text(s, x, y, size, color=WHITE, font=BODY, **kw):
    lay = {"text": s, "font": font, "size": size, "color": color, "x": x, "y": y}
    lay.update(kw)
    return lay


def group(layers, x, y, w, h, **kw):
    lay = {"layers": layers, "x": x, "y": y, "width": w, "height": h}
    lay.update(kw)
    return lay


def pop_in(at, dur=0.45, ease="outBack", start_scale=0):
    return {"from": {"scale": start_scale}, "to": {"scale": 1}, "at": at, "duration": dur, "ease": ease}

# ---------------------------------------------------------------- the fox (local 330 x 260, facing right)

def fox(eyes_closed=False, blink_phase=0.0):
    eye = ({"shape": "path", "d": "M236 80 Q 244 90 252 80", "fill": "none", "stroke": {"color": DARK, "width": 5, "cap": "round"}}
           if eyes_closed else
           circ(244, 80, 13, 15, DARK, scaleY=f"=fract(gt*0.45+{blink_phase}) < 0.05 ? 0.15 : 1"))
    if eyes_closed:
        x, y = bbox(eye["d"]); eye.update({"x": x, "y": y - 3, "anchor": "topLeft"})
    return [
        path("M95 205 C 30 215, -5 150, 25 95 C 45 70, 70 80, 70 100 C 70 140, 95 175, 130 190 Z", FOX),
        path("M25 95 C 35 70, 62 72, 70 100 C 52 108, 35 108, 25 95 Z", CREAM),
        circ(165, 180, 190, 120, FOX),
        circ(200, 195, 90, 80, CREAM),
        rect(150, 240, 22, 44, FOX_DK, radius=11),
        rect(215, 240, 22, 44, FOX_DK, radius=11),
        path("M200 30 L 214 -30 L 242 40 Z", FOX),
        path("M240 36 L 268 -26 L 274 58 Z", FOX),
        path("M210 20 L 216 -8 L 230 26 Z", DARK),
        path("M246 30 L 262 0 L 264 40 Z", DARK),
        path("M180 80 C 190 30, 260 20, 275 60 L 325 95 C 310 118, 278 125, 252 122 C 215 125, 182 112, 180 80 Z", FOX),
        path("M252 122 L 325 95 C 312 124, 280 132, 252 122 Z", CREAM),
        circ(323, 95, 20, 18, DARK),
        eye,
    ]

# ---------------------------------------------------------------- reading line (the product: words light up as read)

def reading_line(words, times, ends, size=68, y=950, clock="gt", band=True, band_color=WHITE):
    kids = []
    for w_, t0, t1 in zip(words, times, ends):
        bg = f"={clock} >= {t0} && {clock} < {t1} ? {MARK} : #FFD23F00"
        col = f"={clock} < {t0} ? {UNREAD} : {INK}"
        kids.append({"children": [text(w_, 0, 0, size, col, weight=700)],
                     "style": {"padding": [2, 14], "radius": 16, "background": bg},
                     "animate": [{"keyframes": {"scale": [1, 1.16, 1]}, "at": t0 if clock == "gt" else t0, "duration": 0.35, "ease": "outCubic"}]})
    layers = []
    if band:
        layers.append(rect(W / 2, (y - 105 + H) / 2, W, H - (y - 105), band_color))
        # page edge shadow line
        layers.append(rect(W / 2, y - 105, W, 6, "#00000022"))
    layers.append({"children": kids, "style": {"direction": "row", "gap": 6}, "x": W / 2, "y": y})
    return layers


def rec_pill(t_on, t_off, clock="gt"):
    """Small recording widget: the child's voice, live."""
    bars = []
    for i in range(14):
        amp = f"=({clock} > {t_on} && {clock} < {t_off}) ? 10 + 34*abs(noise({clock}*7 + {i*1.7})) + 14*abs(sin({clock}*11+{i})) : 6"
        bars.append(rect(250 + i * 15, 48, 8, 6, INK, radius=4, height=amp))
    return [group([
        rect(240, 48, 440, 84, WHITE, radius=42, shadow={"x": 0, "y": 8, "blur": 24, "color": "#0005"}),
        circ(66, 48, 26, 26, "#FF3B3B", scale=f"=1 + 0.18*abs(sin({clock}*4))"),
        text("Reading", 98, 48, 34, INK, weight=700, anchor="left"),
        *bars,
    ], 60, 54, 480, 96, anchor="topLeft", enter="pop")]

# ---------------------------------------------------------------- scene 1: the fox page (0 - 10 s)

random.seed(7)
S1_WORDS = ["A", "little", "fox", "dug", "a", "burrow", "under", "the", "big", "moon."]
S1_T = [1.0, 1.4, 2.0, 3.0, 3.75, 4.0, 5.0, 5.4, 5.9, 6.5]
S1_E = S1_T[1:] + [8.2]

GRASS_TOP = "M-20 700 C 400 640, 800 660, 1100 682 C 1400 704, 1700 650, 1940 676 L 1940 1100 L -20 1100 Z"
EARTH = "M-20 740 C 400 680, 800 700, 1100 722 C 1400 744, 1700 690, 1940 716 L 1940 1100 L -20 1100 Z"
HOLE = (880, 676)
CHAMBER = (1250, 820)

s1 = []
s1.append(rect(W / 2, H / 2, W * 1.3, H * 1.3, "linear-gradient(180deg, #0E2F44, #1F6470)"))
for i, (cx, cy, sc) in enumerate([(520, 230, 1.0), (1080, 150, 0.8), (1350, 380, 0.7), (760, 420, 0.6)]):
    s1.append(group([circ(90, 60, 180, 90, "#2E6E80"), circ(170, 40, 160, 110, "#2E6E80"), circ(250, 65, 150, 80, "#2E6E80")],
                    f"={cx} + 18*gt*{sc}", cy, 340, 130, scale=sc, opacity=0.8))
for i in range(14):
    s1.append({"shape": "circle", "width": 6, "height": 6, "fill": "#FFF3B0", "x": random.randint(600, 1880),
               "y": random.randint(60, 520), "opacity": f"=0.35 + 0.25*sin(gt*2 + {i})"})
# stars
for i in range(30):
    sx, sy = random.uniform(60, 1860), random.uniform(160, 560)
    if sx < 560 and sy < 200:
        sy += 120
    sz = random.choice([18, 22, 28, 34])
    s1.append({"shape": "star", "width": sz, "height": sz, "fill": "#FFF3B0", "x": round(sx), "y": round(sy),
               "opacity": f"=0.65 + 0.35*sin(gt*3 + {i})", "rotation": random.randint(0, 40),
               "scale": 0, "animate": [{"to": {"scale": 1}, "at": 6.6 + i * 0.025, "duration": 0.4, "ease": "outBack"}]})
# moon (rises on "moon")
moon = group([
    circ(150, 150, 260, 260, "#FFD23F"),
    circ(100, 115, 46, 46, "#F2B92E"), circ(190, 190, 64, 64, "#F2B92E"), circ(185, 95, 28, 28, "#F2B92E"),
], 1530, 860, 300, 300, glow={"radius": 60, "intensity": 0.8, "color": "#FFD23F"},
    animate=[{"to": {"y": 270}, "at": 6.5, "duration": 0.9, "ease": "outBack"}])
s1.append(moon)
# far hills
s1.append(path("M-20 650 C 250 520, 520 540, 760 610 C 1000 680, 1300 520, 1600 560 C 1750 580, 1850 600, 1940 590 L 1940 1100 L -20 1100 Z", "#2C6B57"))
# tree, left
s1.append(rect(250, 600, 34, 190, "#5A3420", radius=10))
s1.append(circ(250, 470, 230, 210, "#3E9A63", rotation="=3*sin(gt*1.3)"))
s1.append(circ(190, 520, 140, 120, "#358A57"))
s1.append(circ(320, 515, 130, 120, "#4CAF6C"))
# near ground: grass + earth cross-section
s1.append(path(GRASS_TOP, "#5DB55F"))
s1.append(path(EARTH, "#7A4A2A"))
for i in range(26):  # pebbles
    px, py = random.uniform(40, 1880), random.uniform(790, 880)
    s1.append(circ(round(px), round(py), random.choice([10, 14, 18]), random.choice([8, 10, 12]), "#5E3820"))
# grass tufts + flowers that sway
for i, fx in enumerate([120, 460, 640, 1080, 1460, 1720, 1860]):
    s1.append(path(f"M{fx} 700 L {fx+10} 660 L {fx+18} 700 L {fx+26} 668 L {fx+34} 700 Z", "#4AA352"))
for i, (fx, fc) in enumerate([(380, "#FF6FA3"), (1020, "#FFFFFF"), (1600, "#FF6FA3"), (1790, "#FFD23F")]):
    s1.append(group([rect(30, 70, 6, 70, "#3E8F45"), circ(30, 28, 34, 34, fc), circ(30, 28, 12, 12, "#FFB13B")],
                    fx, 640, 60, 110, rotation=f"=6*sin(gt*2 + {i})"))
# burrow hole (on "dug")
s1.append(circ(*HOLE, 150, 52, DARK, scale=0, animate=[pop_in(3.05, 0.35)]))
# dirt flying out of the hole (on "dug"), three throws
dirt = []
for k, t0 in enumerate([3.05, 3.35, 3.65]):
    for i in range(9):
        vx = random.uniform(-520, -80) if i % 2 else random.uniform(80, 420)
        vy = random.uniform(700, 1050)
        sz = random.choice([14, 18, 24])
        u = f"max(gt-{t0},0)"
        dirt.append(circ(0, 0, sz, sz, random.choice(["#7A4A2A", "#5E3820", "#9A6236"]),
                         x=f"={HOLE[0]} + {vx:.0f}*{u}", y=f"={HOLE[1]} - {vy:.0f}*{u} + 1500*{u}*{u}",
                         opacity=f"=gt > {t0} && gt < {t0+0.9} ? 1 : 0"))
s1.extend(dirt)
# tunnel draws on "burrow"
s1.append({"component": "brush", "start": 4.0, "props": {
    "d": f"M{HOLE[0]} {HOLE[1]+10} C 900 790, 1020 840, {CHAMBER[0]-60} {CHAMBER[1]}",
    "size": 74, "texture": "ink", "color": DARK, "pressure": "even", "taper": [0.0, 0.0], "draw": 0.7, "ease": "inOut"}})
s1.append(circ(*CHAMBER, 250, 120, DARK, scale=0, animate=[pop_in(4.55, 0.4)]))
# the fox: lands on "fox", digs on "dug", dives on "burrow"
s1.append(group(fox(), 560, -300, 330, 300, animate=[
    {"to": {"y": 548}, "at": 2.0, "duration": 0.55, "ease": "bouncy"},
    {"to": {"x": 700}, "at": 3.0, "duration": 0.35, "ease": "outCubic"},
    {"keyframes": {"rotation": [0, -8, 6, -6, 0]}, "at": 3.0, "duration": 0.9},
    {"to": {"x": HOLE[0], "y": HOLE[1] + 10, "scale": 0.15, "rotation": 70}, "at": 4.0, "duration": 0.45, "ease": "inBack"},
    {"to": {"opacity": 0}, "at": 4.4, "duration": 0.05},
]))
# sleeping fox in the chamber
s1.append(group(fox(eyes_closed=True), CHAMBER[0], CHAMBER[1] - 12, 330, 300, scale=0,
                animate=[{"to": {"scale": 0.42}, "at": 4.75, "duration": 0.45, "ease": "outBack"}]))
for i, (zx, zy, zs) in enumerate([(1330, 760, 30), (1365, 725, 38), (1405, 682, 46)]):
    s1.append(text("z", zx, f"={zy} - 6*sin(gt*2+{i})", zs, CREAM, weight=700, opacity=0,
                   animate=[{"to": {"opacity": 1}, "at": 7.0 + i * 0.2, "duration": 0.3}]))
# paper tooth over the picture
s1.append({"component": "paper", "props": {"stock": "cotton", "vignette": 0.25}, "blend": "multiply", "opacity": 0.35})
s1 += reading_line(S1_WORDS, S1_T, S1_E)
s1 += rec_pill(0.8, 7.4)

scene1 = {"duration": 10, "layers": s1, "camera": {"animate": [
    {"from": {"zoom": 1.08, "y": -20}, "to": {"zoom": 1.0, "y": 0}, "duration": 1.2, "ease": "outCubic"},
    {"to": {"zoom": 1.04}, "at": 1.2, "duration": 7.2, "ease": "inOutSine"},
    {"to": {"zoom": 7.0, "x": CHAMBER[0] - 960, "y": CHAMBER[1] - 540}, "at": 8.6, "duration": 1.4, "ease": "inExpo"},
]}}

# ---------------------------------------------------------------- montage pages (one word wakes each)

def page_words(words, last_at=1.0):
    n = len(words)
    ts = [round(0.15 + i * (last_at - 0.15) / (n - 1), 2) for i in range(n)]
    es = ts[1:] + [2.2]
    return reading_line(words, ts, es, size=68, clock="T")


def whale_page():
    L = [rect(W / 2, H / 2, W, H, "linear-gradient(180deg, #8FD3F4, #3B9AD9)")]
    L.append(circ(1620, 220, 170, 170, "#FFE27A", glow="#FFE27A"))
    for i, (cx, cy) in enumerate([(300, 220), (760, 160), (1180, 250)]):
        L.append(group([circ(60, 40, 120, 70, WHITE), circ(110, 30, 100, 80, WHITE), circ(150, 45, 90, 60, WHITE)],
                       f"={cx} + 20*sin(T*0.8+{i})", cy, 220, 100))
    # whale rises
    whale = group([
        path("M40 160 C 40 40, 300 10, 420 90 C 470 120, 520 110, 560 60 C 580 110, 570 170, 520 190 C 420 260, 120 270, 40 160 Z", "#2D3E9C"),
        path("M60 175 C 150 235, 380 235, 470 180 C 380 250, 140 255, 60 175 Z", "#C9D6FF"),
        circ(130, 120, 22, 26, DARK, scaleY="=fract(T*0.7) < 0.06 ? 0.15 : 1"),
        path("M150 160 Q 170 175 190 160", "none", stroke={"color": DARK, "width": 5, "cap": "round"}),
    ], 860, 980, 600, 280, rotation="=4*sin(T*3)",
        animate=[{"to": {"y": 640}, "at": 0.6, "duration": 0.5, "ease": "outBack"}])
    L.append(whale)
    # spout on SPLASH
    random.seed(3)
    for i in range(22):
        vx, vy, sz = random.uniform(-380, 380), random.uniform(900, 1400), random.choice([20, 28, 36])
        u = "max(T-1.0,0)"
        L.append(circ(0, 0, sz, sz, random.choice([WHITE, "#BDE8FF"]),
                      x=f"=780 + {vx:.0f}*{u}", y=f"=600 - {vy:.0f}*{u} + 1700*{u}*{u}",
                      opacity="=T > 1.0 ? 1 : 0"))
    # sea in front
    for k, (yy, col) in enumerate([(700, "#2C7FC4"), (760, "#1F6AAE")]):
        d = f"M-60 {yy} " + " ".join(f"Q {-60+120*j+60} {yy-30} {-60+120*(j+1)} {yy}" for j in range(18)) + " L 2100 1100 L -60 1100 Z"
        L.append(path(d, col, x=f"={-60 + 0} + {30 if k else -30}*sin(T*2)"))
    L.append({"component": "paper", "props": {"stock": "cotton", "vignette": 0.2}, "blend": "multiply", "opacity": 0.3})
    return L + page_words(["The", "whale", "went", "SPLASH!"])


def rocket_page():
    L = [rect(W / 2, H / 2, W * 1.2, H * 1.2, "linear-gradient(180deg, #2A1650, #E2577A)")]
    random.seed(11)
    for i in range(40):
        L.append(rect(random.randint(0, W), random.randint(0, 640), 4, 4, "#FFE9F2", radius=2,
                      height="=T > 1.0 ? 4 + 260*min(T-1.0, 0.5) : 4", opacity=0.8))
    L.append(path("M-20 820 C 400 760, 900 800, 1300 780 C 1600 770, 1800 790, 1940 780 L 1940 1100 L -20 1100 Z", "#3A2366"))
    rocket = group([
        path("M100 20 C 150 70, 160 160, 150 300 L 50 300 C 40 160, 50 70, 100 20 Z", WHITE),
        path("M100 20 C 125 45, 138 70, 145 100 L 55 100 C 62 70, 75 45, 100 20 Z", "#FF4F5E"),
        circ(100, 170, 56, 56, "#4FC3F7", stroke={"color": "#2B3A8C", "width": 8}),
        path("M50 230 L 0 320 L 52 300 Z", "#FF4F5E"),
        path("M150 230 L 200 320 L 148 300 Z", "#FF4F5E"),
        path("M70 300 L 130 300 L 100 380 Z", "#FFD23F", scaleY="=0.7 + 0.5*abs(sin(T*30))"),
    ], 960, 520, 200, 400,
        rotation="=T < 1.0 ? 2*sin(T*60) : 0",
        animate=[{"to": {"y": -700}, "at": 1.0, "duration": 0.8, "ease": "inQuart"}])
    random.seed(5)
    for i in range(16):  # smoke puffs
        side = -1 if i % 2 else 1
        vx = side * random.uniform(150, 520)
        sz = random.choice([110, 140, 170])
        u = "max(T-0.95,0)"
        L.append(circ(0, 0, sz, sz * 0.8, random.choice([WHITE, "#F6E3F0", "#FFD9E4"]),
                      x=f"=960 + {vx:.0f}*min({u}*2.5, 1)", y=f"=830 - {random.uniform(0, 90):.0f}*min({u}*2, 1)",
                      scale=f"=min({u}*5, 1)"))
    L.append(rocket)
    L.append({"component": "paper", "props": {"stock": "cotton", "vignette": 0.2}, "blend": "multiply", "opacity": 0.3})
    return L + page_words(["The", "rocket", "went", "ZOOM!"])


def balloon_page():
    L = [rect(W / 2, H / 2, W, H, "linear-gradient(180deg, #FFE07A, #FFB84C)")]
    cols = ["#FF4F5E", "#4FC3F7", "#7C4DFF", "#2BBF6A", "#FF8A3D", "#FF6FA3", "#3D5AFE"]
    random.seed(9)
    for i, c in enumerate(cols):
        bx = 250 + i * 240 + random.randint(-40, 40)
        by = random.randint(260, 460)
        L.append(group([
            path("M60 150 C 50 220, 80 260, 60 330", "none", stroke={"color": "#6B4A2A", "width": 3}),
            circ(60, 75, 130, 155, c),
            circ(35, 45, 26, 40, "#FFFFFF88", rotation=-25),
            path("M50 150 L 70 150 L 60 162 Z", c),
        ], bx, f"={by} + 14*sin(T*2.2 + {i})", 140, 340, rotation=f"=5*sin(T*1.7+{i})",
            animate=[{"keyframes": {"scale": [1, 1.25, 0]}, "at": 1.0 + i * 0.04, "duration": 0.18}]))
    L.append({"component": "confetti", "start": 1.0, "props": {"count": 260, "colors": cols}})
    L.append({"component": "paper", "props": {"stock": "cotton", "vignette": 0.2}, "blend": "multiply", "opacity": 0.3})
    return L + page_words(["Then", "every", "balloon", "went", "POP!"])

# ---------------------------------------------------------------- logo (16 - 20 s)

f = ImageFont.truetype("Boldonse-Regular.ttf", 190)
word = "Burrow"
total_w = f.getlength(word)
o_left = f.getlength("Burr")
o_w = f.getlength("o")
o_cx = W / 2 - total_w / 2 + o_left + o_w / 2
LOGO_Y = 430

logo = [rect(W / 2, H / 2, W, H, "linear-gradient(180deg, #0E2F44, #1F6470)")]
random.seed(21)
for i in range(22):
    sz = random.choice([16, 22, 28])
    logo.append({"shape": "star", "width": sz, "height": sz, "fill": "#FFF3B0", "x": random.randint(40, 1880),
                 "y": random.randint(60, 300), "opacity": f"=0.6 + 0.4*sin(gt*3 + {i})"})
logo.append(path(GRASS_TOP.replace("700", "860").replace("640", "800").replace("660", "820").replace("682", "842").replace("704", "864").replace("650", "810").replace("676", "836"), "#5DB55F"))
for i, (fx, fc) in enumerate([(240, "#FF6FA3"), (420, "#FFFFFF"), (1500, "#FF6FA3"), (1700, "#FFD23F")]):
    logo.append(group([rect(30, 70, 6, 70, "#3E8F45"), circ(30, 28, 34, 34, fc), circ(30, 28, 12, 12, "#FFB13B")],
                      fx, 800 + (i % 2) * 20, 60, 110, rotation=f"=6*sin(gt*2 + {i})"))
# fox peeks up behind the "o"
logo.append(group(fox(blink_phase=0.3), o_cx + 30, 900, 330, 300, scale=0.62,
                  animate=[{"to": {"y": LOGO_Y - 105}, "at": 0.7, "duration": 0.5, "ease": "outBack"},
                           {"keyframes": {"rotation": [0, -6, 4, 0]}, "at": 1.3, "duration": 0.6}]))
logo.append(text(word, W / 2, LOGO_Y, 190, WHITE, font=LOGO, enter="pop by char from center 0.5s 40ms outBack"))
logo.append({"component": "brush", "start": 0.6, "props": {
    "d": f"M{W/2 - total_w/2 + 20} {LOGO_Y + 175} C {W/2 - 200} {LOGO_Y + 205}, {W/2 + 200} {LOGO_Y + 150}, {W/2 + total_w/2 - 20} {LOGO_Y + 180}",
    "size": 22, "texture": "ink", "color": MARK, "pressure": "taper", "draw": 0.6}})
logo.append(text("Read aloud. Watch it come alive.", W / 2, 720, 64, WHITE, weight=700, start=0.7, enter="rise"))
logo.append({"children": [text("Picture books for ages 4–7", 0, 0, 38, INK, weight=700)],
             "style": {"padding": [12, 30], "radius": 40, "background": MARK},
             "x": W / 2, "y": 830, "start": 1.1, "enter": "pop"})
logo.append({"component": "paper", "props": {"stock": "cotton", "vignette": 0.25}, "blend": "multiply", "opacity": 0.3})

spec = {
    "size": "16:9",
    "fps": 30,
    "fonts": {BODY: "BricolageGrotesque-Bold.ttf", LOGO: "Boldonse-Regular.ttf"},
    "intent": {"pace": "steady", "genre": "launch"},
    "transition": "cut",
    "music": {"style": "acoustic", "bpm": 120, "key": "F", "mood": "playful bright",
              "instruments": {"lead": "glockenspiel"},
              "hits": ["@bar6", "@bar9"],
              "sfx": [{"sfx": "pop", "at": "@2.0"}, {"sfx": "pop", "at": "@4.55"}, {"sfx": "chime", "at": "@6.5"},
                      {"sfx": "whoosh", "at": "@9.6"}, {"sfx": "whoosh", "at": "@13.0"},
                      {"sfx": "pop", "at": "@15.0"}, {"sfx": "ding", "at": "@16.7"}],
              "end": "ring"},
    "scenes": [
        scene1,
        {"duration": 2, "layers": whale_page(),
         "camera": {"animate": [{"from": {"zoom": 1.15}, "to": {"zoom": 1}, "duration": 0.4, "ease": "outExpo"}],
                    "shake": {"amount": 10, "at": 1.0, "duration": 0.3}}},
        {"duration": 2, "layers": rocket_page(),
         "camera": {"animate": [{"from": {"zoom": 1.15}, "to": {"zoom": 1}, "duration": 0.4, "ease": "outExpo"}],
                    "shake": {"amount": 14, "at": 0.9, "duration": 0.4}}},
        {"duration": 2, "layers": balloon_page(),
         "camera": {"animate": [{"from": {"zoom": 1.15}, "to": {"zoom": 1}, "duration": 0.4, "ease": "outExpo"}],
                    "shake": {"amount": 8, "at": 1.0, "duration": 0.25}}},
        {"duration": 4, "layers": logo},
    ],
}

with open("burrow.json", "w") as fh:
    json.dump(spec, fh, indent=1)
print("wrote burrow.json")
