#!/usr/bin/env python3
"""Generates railyard.json — a 15 s launch film for Railyard (real-time payments API).

Concept: a signal-box mimic panel. A payment is an amber car held at a red signal;
one API call throws the lever, the signal clears, and the car runs through to the
other bank, lighting a lamp (a webhook) at every state change.
"""
import json, os

BPM = 128
BEAT = 60 / BPM          # 0.46875 s
BAR = 4 * BEAT           # 1.875 s

PANEL = "#12201C"
PANEL_HI = "#1B2E29"
CREAM = "#EDE6D3"
DIM = "#5E7069"
AMBER = "#FFB21A"
GREEN = "#3CE08A"
RED = "#FF4433"
MONO = "JetBrains Mono"
SANS = "Inter Display"

W, H = 1920, 1080
HERE = os.path.dirname(os.path.abspath(__file__))


def panel(seed=0):
    """Enamel panel (shader), engraved grid, rivets."""
    layers = [{"shader": open(os.path.join(HERE, "enamel.glsl")).read()}]
    for k in range(1, 18):
        layers.append({"shape": "rect", "width": W, "height": 1, "x": W / 2, "y": k * 60,
                       "fill": "#FFFFFF", "opacity": 0.03})
    for k in range(1, 32):
        layers.append({"shape": "rect", "width": 1, "height": H, "x": k * 60, "y": H / 2,
                       "fill": "#FFFFFF", "opacity": 0.03})
    for (x, y) in [(40, 40), (W - 40, 40), (40, H - 40), (W - 40, H - 40)]:
        layers.append({"shape": "circle", "width": 18, "height": 18, "x": x, "y": y,
                       "fill": "radial-gradient(#8C9A93, #2A3833)"})
    return layers


def track(x1, y1, x2, y2, w=10, color=CREAM, draw=None, opacity=1):
    """A straight track segment drawn as a path; optional draw-on (at, dur)."""
    minx, miny = min(x1, x2), min(y1, y2)
    d = f"M{x1 - minx} {y1 - miny} L{x2 - minx} {y2 - miny}"
    lay = {"shape": "path", "d": d, "stroke": color, "strokeWidth": w, "fill": "none",
           "x": minx, "y": miny, "anchor": "topLeft", "opacity": opacity}
    if draw:
        at, dur = draw
        lay["animate"] = [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "at": at,
                           "duration": dur, "ease": "inOutExpo"}]
    return lay


def label(text, x, y, size=34, color=CREAM, font=MONO, anchor="left", **kw):
    d = {"text": text, "font": font, "size": size, "color": color, "x": x, "y": y,
         "anchor": anchor, "letterSpacing": "0.08em"}
    d.update(kw)
    return d


def lamp(x, y, on, off="#22302B", lit="1", r=34):
    """A signal lamp: bezel, unlit lens, lit lens with glow whose opacity is `lit` (expr)."""
    return [{"shape": "circle", "width": r * 2 + 18, "height": r * 2 + 18, "x": x, "y": y,
             "fill": "#0A1210", "stroke": "#3A4A44", "strokeWidth": 3},
            {"shape": "circle", "width": r * 2, "height": r * 2, "x": x, "y": y, "fill": off},
            {"shape": "circle", "width": r * 2, "height": r * 2, "x": x, "y": y, "fill": on,
             "glow": on, "opacity": lit if lit.startswith("=") else float(lit)}]


def car(x, y, extra=None, w=150, h=44):
    d = {"shape": "rect", "width": w, "height": h, "radius": h / 2, "x": x, "y": y,
         "fill": AMBER, "glow": AMBER, "stroke": "#FFE2A0", "strokeWidth": 3}
    if extra:
        d.update(extra)
    return d


def station(x, y, name, align="left"):
    """Platform block + engraved name plate."""
    return [
        {"shape": "rect", "width": 260, "height": 18, "x": x, "y": y + 52, "fill": DIM},
        label(name, x, y + 100, size=40, color=CREAM, font=SANS, anchor="center",
              weight=700, letterSpacing="0.18em"),
    ]


def headline(text, x, y, size="9vh", at=0.0, **kw):
    d = {"text": text, "font": SANS, "size": size, "weight": 700, "color": CREAM,
         "x": x, "y": y, "anchor": "topLeft", "lineHeight": 1.02, "align": "left", "letterSpacing": "-0.02em",
         "enter": {"preset": "mask by line up", "delay": at, "duration": 0.5}}
    d.update(kw)
    return d


# ---------------------------------------------------------------- S1a: the yard (no words)
S1A = BAR - BEAT  # 1.406 (cut on beat 4)
s1a = panel()
TY = 640
SX = 1340
s1a += [
    track(-20, TY, 1940, TY, draw=(0.0, 0.7)),
    track(300, TY, 420, TY + 140, draw=(0.2, 0.35), opacity=0.6),
    track(420, TY + 140, 1500, TY + 140, draw=(0.3, 0.6), opacity=0.6),
    track(1500, TY + 140, 1620, TY, draw=(0.55, 0.35), opacity=0.6),
    track(520, TY, 660, TY - 140, draw=(0.25, 0.35), opacity=0.6),
    track(660, TY - 140, 1260, TY - 140, draw=(0.35, 0.6), opacity=0.6),
    track(200, TY + 140, 300, TY + 280, draw=(0.4, 0.3), opacity=0.35),
    track(300, TY + 280, 1100, TY + 280, draw=(0.45, 0.6), opacity=0.35),
]
for k, (x, y) in enumerate([(700, TY + 140), (880, TY + 140), (1060, TY + 140), (1240, TY + 140),
                            (800, TY - 140), (980, TY - 140), (1160, TY - 140),
                            (520, TY + 280), (700, TY + 280)]):
    s1a.append(car(x, y, {"fill": "#5E4A1E", "glow": None, "stroke": "#8C7030",
                          "enter": {"preset": "pop", "delay": 0.55 + 0.05 * k, "duration": 0.25}}))
s1a += [{"shape": "rect", "width": 10, "height": 150, "x": SX, "y": TY - 95, "fill": DIM}]
s1a += lamp(SX, TY - 190, RED, "#4A1A14", "=fract(t*2.13) < 0.6 ? 1 : 0")
s1a.append(car(0, TY, {"animate": [{"from": {"x": -200}, "to": {"x": SX - 110}, "at": 0.15,
                                    "duration": 0.9, "ease": "outExpo"}]}))
s1a += station(220, TY, "BANK A")
s1a += station(1700, TY, "BANK B")
s1a.append(headline("Payments wait\nin the yard.", 150, 150, at=0.55, size="11vh"))

# ---------------------------------------------------------------- S1b: held (close-up)
S1B = BAR + BEAT  # 2.344
s1b = panel()
s1b += [track(-20, 820, 1940, 820, w=16)]
s1b += [{"shape": "rect", "width": 22, "height": 420, "x": 1460, "y": 610, "fill": DIM}]
s1b += lamp(1460, 330, RED, "#4A1A14", "=fract(t*2.13) < 0.6 ? 1 : 0", r=120)
s1b.append(car(1130, 820, {"animate": [{"from": {"x": 1080}, "to": {"x": 1130}, "duration": 0.6,
                                        "ease": "outExpo"}]}, w=460, h=110))
s1b.append(headline("Payments wait\nin the yard.", 150, 150, at=0.0, size="11vh", enter=None))
s1b.append(label("HELD · DAY {value}", 154, 470, size=46, color=RED,
                 value=f"=1 + floor(clamp(t / {BEAT:.5f} - 0.5, 0, 2))",
                 anchor="topLeft", letterSpacing="0.14em",
                 animate=[{"from": {"opacity": 0}, "to": {"opacity": 1}, "duration": 0.1}]))

# ---------------------------------------------------------------- S2: one API call (lever frame)
S2 = BAR
THROW = BEAT  # beat 10
s2 = panel()
LX0, LSTEP, LPY = 820, 130, 740
ACTIVE = 6
s2.append({"shape": "rect", "width": 1000, "height": 120, "radius": 12, "x": LX0 + 3 * LSTEP,
           "y": LPY + 20, "fill": "linear-gradient(180deg, #2C3C37, #121D19)",
           "stroke": "#3E514A", "strokeWidth": 3})
handle = ["#C9352B", "#C9352B", "#2D5FA8", "#EDE6D3", "#C9352B", "#2D5FA8", None]
for k in range(7):
    x = LX0 + k * LSTEP
    if k == ACTIVE:
        lever = {"shape": "rect", "width": 46, "height": 560, "radius": 23, "x": x, "y": LPY,
                 "anchor": "bottom", "fill": "linear-gradient(90deg, #B98B2E, #F3D27A, #A87A22)",
                 "rotation": -24,
                 "animate": [{"to": {"rotation": 26}, "at": THROW - 0.12, "duration": 0.2, "ease": "inExpo"},
                             {"keyframes": {"rotation": [26, 21, 26]}, "at": THROW + 0.08, "duration": 0.25}]}
    else:
        lever = {"shape": "rect", "width": 34, "height": 470, "radius": 17, "x": x, "y": LPY,
                 "anchor": "bottom", "rotation": -24,
                 "fill": f"linear-gradient(180deg, {handle[k]} 0%, {handle[k]} 22%, #8D979A 23%, #5D6669 100%)"}
    s2.append(lever)
    s2.append({"shape": "circle", "width": 70, "height": 70, "x": x, "y": LPY,
               "fill": "radial-gradient(#C2CBC6, #3A4743)" if k != ACTIVE else "radial-gradient(#F3D27A, #8A6418)"})
    s2.append(label(str(k + 1), x, LPY + 60, size=34, color=AMBER if k == ACTIVE else "#8C9A93",
                    anchor="center", letterSpacing="0"))
s2 += [
    {"shape": "rect", "width": 760, "height": 110, "radius": 10, "x": LX0 + 3 * LSTEP, "y": 925,
     "fill": "#0B1411", "stroke": AMBER, "strokeWidth": 2},
    label("POST /v1/transfers", LX0 + 3 * LSTEP, 925, size=46, color=AMBER, anchor="center",
          letterSpacing="0.02em", enter={"preset": "type", "duration": 0.3}),
]
s2 += [{"shape": "rect", "width": 14, "height": 300, "x": 330, "y": 830, "fill": DIM}]
s2 += lamp(330, 610, RED, "#22302B", f"=t < {THROW + 0.06:.3f} ? 1 : 0", r=70)
s2 += lamp(330, 610, GREEN, "#22302B", f"=t < {THROW + 0.06:.3f} ? 0 : 1", r=70)[2:]
s2.append(label("CLEAR", 450, 610, size=46, color=GREEN, anchor="left", font=SANS,
                weight=700, letterSpacing="0.3em",
                animate=[{"from": {"opacity": 0}, "to": {"opacity": 1}, "at": THROW + 0.06,
                          "duration": 0.05}]))
s2.append(headline("One API call.", 150, 140, at=0.0))

# ---------------------------------------------------------------- S3: run + webhooks
S3 = 2 * BAR
s3 = panel()
RY = 770
X0, X1 = 230, 1690
RUN_AT, RUN_DUR = 0.15, 1.6
s3 += [track(-20, RY, 1940, RY)]
s3 += station(X0, RY, "BANK A")
s3 += station(X1, RY, "BANK B")
events = [("transfer.created", 500), ("transfer.sent", 960), ("transfer.settled", 1420)]


def tl_of(x):
    return RUN_AT + (x - X0) / (X1 - X0) * RUN_DUR


for k, (ev, ex) in enumerate(events):
    tl = tl_of(ex)
    s3 += [{"shape": "rect", "width": 8, "height": 110, "x": ex, "y": RY - 60, "fill": DIM}]
    s3 += lamp(ex, RY - 130, GREEN, "#22302B", f"=t < {tl:.3f} ? 0 : 1", r=26)
    s3.append({"shape": "rect", "width": 4, "height": 120, "x": ex, "y": RY - 230,
               "fill": GREEN, "start": tl,
               "animate": [{"from": {"scaleY": 0}, "to": {"scaleY": 1}, "duration": 0.15, "ease": "outExpo"}]})
    rise = [{"from": {"y": "+40", "opacity": 0}, "to": {"y": "-40", "opacity": 1}, "duration": 0.25, "ease": "outExpo"}]
    s3.append({"shape": "rect", "width": 392, "height": 108, "radius": 10, "x": ex, "y": RY - 342 + 40,
               "fill": "#0B1411", "stroke": GREEN, "strokeWidth": 2, "start": tl, "animate": rise})
    s3.append(label("WEBHOOK", ex - 172, RY - 364 + 40, size=26, color=GREEN, letterSpacing="0.25em",
                    start=tl, animate=rise))
    s3.append(label(ev, ex - 172, RY - 322 + 40, size=36, color=CREAM, letterSpacing="0",
                    start=tl, animate=rise))
s3.append(car(0, RY, {"x": f"={X0} + clamp((t - {RUN_AT}) / {RUN_DUR}, 0, 1) * {X1 - X0}"}))
s3.append(headline("A webhook at\nevery signal.", 150, 120, at=0.2))
s3.append(label("SETTLED", X1, RY - 110, size=46, color=GREEN, anchor="center", font=SANS,
                weight=800, letterSpacing="0.3em", start=RUN_AT + RUN_DUR,
                enter={"preset": "pop", "duration": 0.25}))

# ---------------------------------------------------------------- S4: the whole yard
S4 = 1.5 * BAR
s4 = panel()
NT = 12
ys = [150 + k * 70 for k in range(NT)]
for y in ys:
    s4.append(track(-20, y, 1940, y, w=6, opacity=0.7))
xo = [260, 700, 1180, 1580, 480, 940, 1400, 820, 1700, 360, 1080]
for k in range(NT - 1):
    s4.append(track(xo[k], ys[k], xo[k] + 70, ys[k + 1], w=6, opacity=0.7))
for k in range(26):
    tr = k % NT
    speed = 900 + (k * 137) % 700
    off = (k * 389) % 2200
    s4.append(car(0, ys[tr], {"x": f"=mod({off} + t*{speed}, 2300) - 190"}, w=90, h=26))
for k in range(30):
    s4.append({"shape": "circle", "width": 18, "height": 18,
               "x": 120 + (k * 613) % 1700, "y": ys[(k * 5) % NT] - 26,
               "fill": GREEN, "glow": GREEN, "opacity": f"=fract(t*1.7 + {k * 0.137:.3f}) < 0.5 ? 1 : 0.15"})
pop = {"preset": "pop", "delay": 0.45, "duration": 0.3}
s4.append({"shape": "rect", "width": 1180, "height": 290, "radius": 16, "x": 740, "y": 545,
           "fill": "#0B1411", "stroke": "#3E514A", "strokeWidth": 3, "enter": pop})
s4.append({"text": "Instant settlement\nbetween bank accounts.", "font": SANS, "size": "9vh",
           "weight": 700, "color": CREAM, "x": 200, "y": 545, "anchor": "left", "align": "left",
           "lineHeight": 1.04, "letterSpacing": "-0.02em", "enter": pop})

# ---------------------------------------------------------------- S5: wordmark
S5 = 15.0 - (S1A + S1B + S2 + S3 + S4)
s5 = panel()
LY = 700
for k, y0 in enumerate([260, 420, 560, 860, 1000]):
    top = min(y0, LY)
    d = f"M0 {y0 - top} C 320 {y0 - top}, 420 {LY - top}, 760 {LY - top}"
    s5.append({"shape": "path", "d": d, "stroke": CREAM, "strokeWidth": 8, "fill": "none",
               "x": -20, "y": top, "anchor": "topLeft", "opacity": 0.5,
               "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "at": 0.05 * k,
                            "duration": 0.6, "ease": "outExpo"}]})
s5.append(track(735, LY, 1560, LY, w=10, draw=(0.3, 0.45)))
s5.append({"shape": "rect", "width": 18, "height": 70, "x": 1570, "y": LY, "fill": RED})
s5.append(car(0, LY, {"animate": [{"from": {"x": -200}, "to": {"x": 1470}, "at": 0.25,
                                   "duration": 1.0, "ease": "outExpo"}]}))
s5 += [{"shape": "rect", "width": 8, "height": 70, "x": 1660, "y": LY - 35, "fill": DIM}]
s5 += lamp(1660, LY - 100, GREEN, "#22302B", "=t < 1.0 ? 0 : 0.8 + 0.2*cos(beat*6.2832)", r=24)
s5.append({"text": "Railyard", "font": SANS, "size": "19vh", "weight": 800, "color": CREAM,
           "x": 760, "y": LY - 34, "anchor": "bottomLeft", "letterSpacing": "-0.035em",
           "enter": {"preset": "mask by line up", "delay": 0.4, "duration": 0.55}})
s5.append(label("REAL-TIME PAYMENTS API", 766, LY + 60, size=40, color=AMBER,
                anchor="topLeft", letterSpacing="0.22em",
                enter={"preset": "rise", "delay": 0.8, "duration": 0.4}))

T_S3 = S1A + S1B + S2
spec = {
    "size": "16:9",
    "fps": 30,
    "background": PANEL,
    "intent": {"pace": "steady"},
    "music": {
        "style": "acoustic", "instrumentation": "band", "bpm": BPM, "key": "E",
        "hits": ["@beat10"], "end": "ring",
        "sfx": [
            {"sfx": "click", "at": "@beat10"},
            {"sfx": "whoosh", "at": round(T_S3 + RUN_AT + 0.5, 3)},
            {"sfx": "tick", "at": round(T_S3 + tl_of(500), 3)},
            {"sfx": "tick", "at": round(T_S3 + tl_of(960), 3)},
            {"sfx": "tick", "at": round(T_S3 + tl_of(1420), 3)},
            {"sfx": "chime", "at": round(T_S3 + RUN_AT + RUN_DUR, 3)},
            {"sfx": "impact", "at": round(T_S3 + S3, 3)},
            {"sfx": "chime", "at": round(15.0 - S5 + 1.0, 3)},
        ],
    },
    "motionBlur": True,
    "post": {"bloom": 0.25, "bloomThreshold": 0.85, "vignette": 0.3},
    "scenes": [
        {"duration": S1A, "layers": s1a,
         "camera": {"animate": [{"from": {"zoom": 1.35, "x": -260}, "to": {"zoom": 1.0, "x": 0},
                                 "duration": 0.75, "ease": "outExpo"}]}},
        {"duration": S1B, "transition": "cut", "layers": s1b,
         "camera": {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.05}, "duration": S1B,
                                 "ease": "linear"}]}},
        {"duration": S2, "transition": "cut", "layers": s2,
         "camera": {"animate": [{"from": {"zoom": 1.12}, "to": {"zoom": 1.0}, "duration": 0.3, "ease": "outExpo"}],
                    "shake": {"amount": 10, "at": THROW + 0.08, "duration": 0.25}}},
        {"duration": S3, "transition": "cut", "layers": s3,
         "camera": {"animate": [{"from": {"x": 500}, "to": {"x": 0}, "duration": 0.3, "ease": "outExpo"},
                                {"to": {"zoom": 1.06}, "at": 0.3, "duration": S3 - 0.3, "ease": "linear"}]}},
        {"duration": S4, "transition": "cut", "layers": s4,
         "camera": {"animate": [{"from": {"zoom": 1.8}, "to": {"zoom": 1.0}, "duration": 0.9, "ease": "outExpo"}],
                    "shake": {"amount": 8, "at": 0, "duration": 0.15}}},
        {"duration": round(S5, 4), "transition": "cut", "layers": s5,
         "camera": {"animate": [{"from": {"zoom": 1.06}, "to": {"zoom": 1.0}, "duration": S5, "ease": "linear"}]}},
    ],
}

out = os.path.join(HERE, "railyard.json")
with open(out, "w") as f:
    json.dump(spec, f, indent=1)
print("wrote", out)
