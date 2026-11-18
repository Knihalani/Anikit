#!/usr/bin/env python3
"""Builds spec.json for the Northbridge City v Harbour Athletic opener (data: bridge truss, bulb clock)."""
import json, math

CREAM = "#F3EEE2"; CRIMSON = "#B0102A"; TEAL = "#07535F"; NIGHT = "#03060B"
FONT = "Big Shoulders"

def slam(delay=0.0, scale=1.35, blur=14, dur=0.18):
    return [{"from": {"scale": scale, "opacity": 0, "blur": blur}, "duration": dur, "ease": "outExpo", "at": delay}]

def push(z0, z1, dur, rot0=0, rot1=0):
    return {"animate": [{"from": {"zoom": z0, "rotation": rot0}, "to": {"zoom": z1, "rotation": rot1}, "duration": dur, "ease": "outExpo"}]}

# ---------- pitch composition (top-down, ball on centre spot, four floodlight shadows) ----------
W, H = 1920, 1080
CX, CY = 960, 540
stripes = [{"shape": "rect", "width": 170, "height": 2400, "x": -900 + i * 170 + 85, "y": CY,
            "fill": "#1E5A2F" if i % 2 else "#24693A"} for i in range(22)]
ball = {"layers": [
    {"shape": "circle", "width": 66, "height": 66, "fill": "radial-gradient(#FFFFFF, #C9CDD2)"},
    {"draw": [
        {"points": [[CX-8, CY-14], [CX+8, CY-14], [CX+13, CY+2], [CX, CY+12], [CX-13, CY+2]], "closed": True, "fill": "#1A1C22"},
        {"points": [[CX-33, CY-6], [CX-24, CY-20], [CX-20, CY-4], [CX-28, CY+10]], "closed": True, "fill": "#2A2C33"},
        {"points": [[CX+33, CY-6], [CX+24, CY-20], [CX+20, CY-4], [CX+28, CY+10]], "closed": True, "fill": "#2A2C33"},
        {"points": [[CX-10, CY+33], [CX+10, CY+33], [CX+6, CY+22], [CX-6, CY+22]], "closed": True, "fill": "#2A2C33"},
    ]}
]}
shadows = []
for k, ang in enumerate([35, 145, 215, 325]):
    a = math.radians(ang); L = 330
    shadows.append({"shape": "rect", "width": L, "height": 58, "radius": 29, "fill": "linear-gradient(90deg, #000000CC, #00000000)",
                    "x": CX + math.cos(a) * L / 2, "y": CY + math.sin(a) * L / 2, "rotation": ang, "blur": 6, "blend": "multiply",
                    "start": "=$shadow0 + %f" % (k * 0.12) if False else None,
                    "animate": [{"from": {"opacity": 0}, "to": {"opacity": 1}, "duration": 0.06, "at": "SH%d" % k}]})
for s in shadows:
    s.pop("start")
chalk = [
    {"shape": "path", "d": "M300 0 A300 300 0 1 1 299.9 0", "stroke": "#EDEFE8", "strokeWidth": 9, "fill": "none",
     "x": CX, "y": CY, "rotation": -90, "opacity": 0.92,
     "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.9, "ease": "inOutCubic", "at": "CH"}]},
    {"shape": "rect", "width": 9, "height": 2400, "fill": "#EDEFE8", "x": CX, "y": CY, "opacity": 0.92,
     "animate": [{"from": {"scaleY": 0}, "to": {"scaleY": 1}, "duration": 0.7, "ease": "outExpo", "at": "CH"}]},
    {"shape": "circle", "width": 22, "height": 22, "fill": "#EDEFE8", "x": CX, "y": CY},
]
light = [
    {"shape": "rect", "width": 3000, "height": 3000, "x": CX, "y": CY, "blend": "multiply",
     "fill": "radial-gradient(#FFFFFF, #8C9488 45%, #1A2018)"},
    {"shape": "circle", "width": 1600, "height": 1600, "x": CX, "y": CY, "blend": "screen", "opacity": 0.22,
     "fill": "radial-gradient(#FFF6DF, #00000000)"},
]
def pitch_layers(ch, sh):
    out = json.loads(json.dumps(stripes + light + chalk + shadows + [ball]))
    for l in out:
        for a in l.get("animate", []):
            if a.get("at") == "CH": a["at"] = ch
            elif isinstance(a.get("at"), str) and a["at"].startswith("SH"): a["at"] = sh + int(a["at"][2:]) * 0.125
    return out

# ---------- Northbridge: a suspension arch drawn on ----------
deck = 620
bridge = [{"d": "M-60 %d Q 960 -40 1980 %d" % (deck, deck), "stroke": CREAM, "width": 12, "cap": "round"},
          {"line": [-60, deck, 1980, deck], "stroke": CREAM, "width": 12},
          {"line": [-60, deck + 26, 1980, deck + 26], "stroke": CREAM, "width": 4}]
for i in range(1, 24):
    t = i / 24
    x = (1 - t) ** 2 * -60 + 2 * t * (1 - t) * 960 + t * t * 1980
    y = (1 - t) ** 2 * deck + 2 * t * (1 - t) * -40 + t * t * deck
    bridge.append({"line": [x, y, x, deck], "stroke": CREAM, "width": 4})
# ---------- Harbour: swell lines + a lighthouse sweep ----------
waves = []
for r in range(7):
    y0 = 650 + r * 38
    pts = [[x, y0 + 14 * math.sin(x / 95 + r * 1.3)] for x in range(-40, 1981, 40)]
    waves.append({"points": pts, "smooth": True, "stroke": CREAM, "width": 4 if r else 8, "opacity": 1 - r * 0.1})

# ---------- 19:45 as a board of floodlight bulbs ----------
GLYPH = {
    "1": ["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
    "9": ["01110", "10001", "10001", "01111", "00001", "00010", "01100"],
    ":": ["0", "0", "1", "0", "1", "0", "0"],
    "4": ["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
    "5": ["11111", "10000", "11110", "00001", "00001", "10001", "01110"],
}
pitch_px = 50; cols = []
for ch in "19:45":
    cols += [ch]
total_cols = sum(len(GLYPH[c][0]) for c in "19:45") + 4
x0 = CX - (total_cols - 1) * pitch_px / 2; y0 = CY - 3 * pitch_px + 10
on, off = [], []
cx = 0
for c in "19:45":
    g = GLYPH[c]
    for r, row in enumerate(g):
        for k, b in enumerate(row):
            p = [round(x0 + (cx + k) * pitch_px, 1), round(y0 + r * pitch_px, 1)]
            (on if b == "1" else off).append(p)
    cx += len(g[0]) + 1
# dim bulbs fill the board margins too
for r in range(-1, 8):
    for k in range(-1, total_cols + 1):
        if r in (-1, 7) or k in (-1, total_cols):
            off.append([round(x0 + k * pitch_px, 1), round(y0 + r * pitch_px, 1)])
on.sort(key=lambda p: (p[0], p[1]))
N = len(on)


def bulb_row(n, y, color, delay, spread=1.0):
    pts = [[round(160 + i * (1600 / (n - 1)), 1), y] for i in range(n)]
    return [{"dots": {"data": pts, "size": 22, "color": "#1C2230"}},
            {"dots": {"data": pts, "size": 26, "color": color, "appear": "=clamp((t-%.2f)*%d - i, 0, 1)" % (delay, int(n * 3 * spread))},
             "glow": {"radius": 22, "intensity": 0.9, "color": "#FFD99A"}}]

spec = {
    "size": "16:9", "fps": 30,
    "intent": {"pace": "fast", "genre": "sport"},
    "motionBlur": True,
    "post": {"bloom": 0.22, "grain": 0.04, "vignette": 0.25},
    "music": {"style": "cinematic-hybrid", "bpm": 120, "key": "Dm", "mood": "dark epic",
              "hits": ["@2.5", "@7.5"], "end": "ring",
              "sfx": [{"sfx": "impact", "at": 0.35}, {"sfx": "impact", "at": 0.6}, {"sfx": "impact", "at": 0.85},
                      {"sfx": "impact", "at": 1.1}, {"sfx": "riser", "at": 2.5}, {"sfx": "whoosh", "at": 4.0},
                      {"sfx": "whoosh", "at": 5.0}, {"sfx": "riser", "at": 7.5}, {"sfx": "braam", "at": 7.5}]},
    "scenes": [
        # 1. the lights come on
        {"duration": 2.5, "transition": "cut", "background": NIGHT,
         "camera": {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.08}, "duration": 2.5, "ease": "linear"}]},
         "layers": [{"type": "shader", "src": "floodlights.glsl"}]},
        # 2. ball on the centre spot, four shadows
        {"duration": 1.5, "transition": "cut", "background": "#0B1A10",
         "camera": push(1.9, 1.15, 1.5, -28, 0),
         "layers": pitch_layers(0.0, 0.05)},
        # 3. Northbridge City
        {"duration": 1.0, "transition": "cut", "background": CRIMSON,
         "camera": push(1.12, 1.0, 0.6),
         "layers": [
             {"enter": "draw", "draw": bridge, "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.55, "ease": "outCubic"}]},
             *bulb_row(8, 90, "#FFF1D2", 0.05),
             {"text": "NORTHBRIDGE", "font": FONT, "weight": 900, "size": "26vh", "color": CREAM, "letterSpacing": "-0.01em",
              "x": "7%", "y": "79%", "anchor": "left", "animate": slam(0.0)},
             {"text": "CITY", "font": FONT, "weight": 700, "size": "7vh", "color": CREAM, "letterSpacing": "0.06em",
              "x": "93%", "y": "20%", "anchor": "right", "animate": slam(0.12, 1.2, 8)}]},
        # 4. Harbour Athletic
        {"duration": 1.0, "transition": "cut", "background": TEAL,
         "camera": push(1.12, 1.0, 0.6),
         "layers": [
             {"shape": "polygon", "points": [[1700, 150], [-400, -300], [-400, 700]], "fill": "linear-gradient(270deg, #F3EEE255, #F3EEE200)",
              "blend": "screen", "x": 0, "y": 0, "anchor": "topLeft"} if False else
             {"draw": [{"points": [[300, 250], [2300, -350], [2300, 520]], "closed": True, "fill": "#F3EEE230"}],
              "blend": "screen", "rotation": "=-10 + t*16", "animate": [{"from": {"opacity": 0}, "duration": 0.15}]},
             {"draw": [{"points": [[258, 650], [342, 650], [324, 290], [276, 290]], "closed": True, "fill": CREAM},
                       {"rect": [268, 330, 64, 26], "fill": TEAL}, {"rect": [266, 420, 68, 26], "fill": TEAL},
                       {"rect": [272, 228, 56, 52], "fill": "#FFF6DC"}, {"points": [[264, 230], [336, 230], [300, 196]], "closed": True, "fill": CREAM}],
              "glow": {"radius": 30, "intensity": 0.5, "color": "#FFF1C8"}},
             {"draw": waves, "x": "=960-40*t", "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.35, "ease": "outCubic"}]},
             *bulb_row(12, 990, "#FFF1D2", 0.0, 1.3),
             {"text": "HARBOUR", "font": FONT, "weight": 900, "size": "30vh", "color": CREAM, "letterSpacing": "-0.01em",
              "x": "93%", "y": "33%", "anchor": "right", "animate": slam(0.0)},
             {"text": "ATHLETIC", "font": FONT, "weight": 700, "size": "11vh", "color": CREAM, "letterSpacing": "0.04em",
              "x": "93%", "y": "50%", "anchor": "right", "animate": slam(0.12, 1.2, 8)}]},
        # 5. 19:45 in bulbs
        {"duration": 1.5, "transition": "cut", "background": "#05080E", "timeRemap": "=t < 0.7 ? t : 0.7 + (t - 0.7) * 0.45",
         "camera": push(1.25, 1.0, 1.5),
         "layers": [
             {"shape": "rect", "width": 2400, "height": 1400, "fill": "radial-gradient(#1A2230, #05080E)"},
             {"dots": {"data": off, "size": 30, "color": "#1C2230"}},
             {"dots": {"data": on, "size": 34, "color": "#FFF1D2", "appear": "=clamp((t-0.1)*%d - i, 0, 1)" % int(N * 2.4)},
              "glow": {"radius": 26, "intensity": 0.9, "color": "#FFD99A"}},
             {"text": "KICK-OFF", "font": FONT, "weight": 800, "size": "8vh", "color": CREAM, "letterSpacing": "0.06em",
              "x": "50%", "y": "13%", "animate": slam(0.0, 1.1, 6)},
             {"text": "TONIGHT", "font": FONT, "weight": 800, "size": "8vh", "color": CREAM, "letterSpacing": "0.06em",
              "x": "50%", "y": "88%", "animate": slam(0.45, 1.1, 6)}]},
        # 6. lockup on the pitch
        {"duration": 1.5, "transition": "cut", "background": "#0B1A10",
         "camera": {"animate": [{"from": {"zoom": 1.25}, "to": {"zoom": 1.0}, "duration": 1.5, "ease": "outQuart"}]},
         "layers": pitch_layers(-5, -5) + [
             {"shape": "rect", "width": 1100, "height": 1600, "x": 410, "y": CY, "fill": CRIMSON, "blend": "multiply", "opacity": 0.9, "inertia": "tight", "smear": 1,
              "animate": [{"from": {"x": -700}, "duration": 0.3, "ease": "outExpo"}]},
             {"shape": "rect", "width": 1100, "height": 1600, "x": 1510, "y": CY, "fill": "#0B7C8A", "blend": "multiply", "opacity": 0.9, "inertia": "tight", "smear": 1,
              "animate": [{"from": {"x": 2620}, "duration": 0.3, "ease": "outExpo"}]},

             {"text": "NORTHBRIDGE\nCITY", "font": FONT, "weight": 900, "size": "10vh", "lineHeight": 0.88, "color": CREAM, "align": "right",
              "x": "33%", "y": "50%", "anchor": "right", "animate": slam(0.15)},
             {"text": "HARBOUR\nATHLETIC", "font": FONT, "weight": 900, "size": "10vh", "lineHeight": 0.88, "color": CREAM, "align": "left",
              "x": "67%", "y": "50%", "anchor": "left", "animate": slam(0.25)},
             {"text": "WOMEN'S SUPER LEAGUE", "font": FONT, "weight": 700, "size": "5vh", "color": CREAM, "letterSpacing": "0.08em",
              "x": "50%", "y": "10%", "animate": slam(0.5, 1.1, 6, 0.3)},
             {"text": "TONIGHT", "font": FONT, "weight": 800, "size": "7vh", "color": "#FFE7B8", "letterSpacing": "0.08em",
              "x": "50%", "y": "89%", "animate": slam(0.9, 1.1, 6, 0.3)}]},

        # 7. payoff: the floodlights ARE the clock, pushed in until the bulbs fill the frame
        {"duration": 1.0, "transition": "cut", "background": "#05080E",
         "camera": {"animate": [{"from": {"zoom": 2.1, "x": -120}, "to": {"zoom": 2.9, "x": -60}, "duration": 1.0, "ease": "outQuart"}]},
         "layers": [
             {"shape": "rect", "width": 2400, "height": 1400, "fill": "radial-gradient(#1A2230, #05080E)"},
             {"dots": {"data": off, "size": 30, "color": "#1C2230"}},
             {"dots": {"data": on, "size": 34, "color": "#FFF1D2"}, "glow": {"radius": 30, "intensity": 1.2, "color": "#FFD99A"}}]},
    ],
}
json.dump(spec, open("spec.json", "w"), indent=1)
print("on bulbs", N)
