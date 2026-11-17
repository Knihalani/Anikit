#!/usr/bin/env python3
"""The Planet Breathes: NOAA Mauna Loa monthly CO2 (1958-03 .. 2026-09) as a breath, a spiral, then the Keeling curve.

Geometry that depends on data and time (the spiral drawing on, the morphs) is baked per frame here:
the engine has no data-driven path interpolation, so each frame gets its own `draw` layer.
"""
import csv, json, math, os

HERE = os.path.dirname(os.path.abspath(__file__))
FPS = 30
W, H = 1920, 1080
rows = [r for r in csv.reader(l for l in open(os.path.join(HERE, "data/co2_mm_mlo.csv")) if not l.startswith("#"))][1:]
D = [(int(r[0]), int(r[1]), float(r[2]), float(r[3]), float(r[4])) for r in rows]
N = len(D)
T0, T1 = D[0][2], D[-1][2]

BG = "#0A0F17"
INK = "#E8ECEF"
MUTED = "#7D8794"
HAIR = "#26303C"

def lerp(a, b, t): return a + (b - a) * t
def ease(t): t = max(0.0, min(1.0, t)); return t * t * (3 - 2 * t)
def ease_io(t): t = max(0.0, min(1.0, t)); return 0.5 - 0.5 * math.cos(math.pi * t)

STOPS = [(1958, (58, 134, 168)), (1990, (150, 166, 170)), (2010, (232, 160, 90)), (2027, (255, 84, 52))]
def ycol(t):
    for (a, ca), (b, cb) in zip(STOPS, STOPS[1:]):
        if t <= b:
            u = (t - a) / (b - a)
            return "#%02x%02x%02x" % tuple(int(lerp(x, y, u)) for x, y in zip(ca, cb))
    return "#ff5434"
COLS = [ycol(d[2]) for d in D]

CX, CY = 1270, 540
def ang(t): return math.pi / 2 - 2 * math.pi * (t - math.floor(t))       # Jan at top, clockwise
def polar(t, r): a = ang(t); return (CX + r * math.cos(a), CY - r * math.sin(a))
def pA(d): return polar(d[2], 330 + (d[3] - 428.5) * 28)                 # one year, magnified
def pB(d): return polar(d[2], 64 + (d[3] - 312) * 3.6)                    # the spiral
def pC(d): return (180 + (d[2] - 1958) / (2027 - 1958) * 1600, 960 - (d[3] - 312) * 6.6)  # the curve

SUB = 6
def cr(p0, p1, p2, p3, u):
    return 0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u * u + (-p0 + 3 * p1 - 3 * p2 + p3) * u ** 3)
def fine(ix):
    """(t, ppm, colour) samples through the months in ix, smoothed."""
    out = []
    ix = list(ix)
    for j in range(len(ix) - 1):
        a = D[ix[max(j - 1, 0)]]; b = D[ix[j]]; c = D[ix[j + 1]]; d = D[ix[min(j + 2, len(ix) - 1)]]
        for k in range(SUB):
            u = k / SUB
            out.append((cr(a[2], b[2], c[2], d[2], u), cr(a[3], b[3], c[3], d[3], u), COLS[ix[j]]))
    e = D[ix[-1]]; out.append((e[2], e[3], COLS[ix[-1]]))
    return out
def rA(t, v): return polar(t, 330 + (v - 428.5) * 28)
def rB(t, v): return polar(t, 64 + (v - 312) * 3.6)
def rC(t, v): return (180 + (t - 1958) / (2027 - 1958) * 1600, 960 - (v - 312) * 6.6)
def P2(samples, f): return [[round(f(t, v)[0], 1), round(f(t, v)[1], 1)] for t, v, _ in samples]
FALL = fine(range(N))

LAST = [i for i, d in enumerate(D) if (d[0] == 2025 and d[1] >= 10) or d[0] == 2026]

def pts(ix, f):
    return [[round(f(i)[0], 1), round(f(i)[1], 1)] for i in ix]

layers = []
def frame_layer(t0, items, extra=None):
    L = {"draw": items, "start": round(t0 - 0.25 / FPS, 5), "end": round(t0 + 0.75 / FPS, 5)}
    if extra: L.update(extra)
    layers.append(L)

def text(s, x, y, size, color=INK, font="Libre Franklin", weight=300, start=0, end=None, anchor="topLeft", fade=0.35, extra=None, **kw):
    L = {"text": s, "font": font, "size": size, "weight": weight, "color": color, "x": x, "y": y, "anchor": anchor,
         "start": start, "lineHeight": 1.18, "align": "left"}
    if end is not None: L["end"] = end
    if fade: L["animate"] = [{"from": {"opacity": 0, "y": "+14"}, "duration": fade, "ease": "outCubic"}]
    if extra: L.update(extra)
    L.update(kw)
    layers.append(L)

# ------------------------------------------------------------------ timeline
A0, A1 = 0.0, 3.4          # the breath (one year)
B0, B1 = 3.4, 4.0          # the year shrinks into its place on the spiral
S0, S1 = 3.5, 7.4          # 68 years spiral out
M0, M1 = 7.6, 9.2          # the spiral unrolls into the curve
END = 12.4

# ---- static furniture: source line
text("Source: NOAA Global Monitoring Laboratory, Mauna Loa Observatory, monthly mean CO₂ (ppm), Mar 1958 – Sep 2026",
     120, 1020, 19, MUTED, font="IBM Plex Mono", weight=400, start=0.2, fade=0.6)

# ---- beat 1: one breath -------------------------------------------------
# month ring: hairline circle, 12 ticks, initials
MON = "JFMAMJJASOND"
ring = [{"circle": [CX, CY, 330], "stroke": HAIR, "width": 1.2}]
for m in range(12):
    a = math.pi / 2 - 2 * math.pi * m / 12
    x0, y0 = CX + 432 * math.cos(a), CY - 432 * math.sin(a)
    x1, y1 = CX + 446 * math.cos(a), CY - 446 * math.sin(a)
    ring.append({"line": [round(x0, 1), round(y0, 1), round(x1, 1), round(y1, 1)], "stroke": MUTED, "width": 1.5})
layers.append({"draw": ring, "start": 0.0, "end": B1, "animate": [{"from": {"opacity": 0.4}, "duration": 0.4},
               {"to": {"opacity": 0}, "at": B0 - 0.0, "duration": 0.4}]})
for m in range(12):
    a = math.pi / 2 - 2 * math.pi * (m + 0.5) / 12
    text(MON[m], CX + 462 * math.cos(a), CY - 462 * math.sin(a), 20, MUTED, font="IBM Plex Mono", weight=400,
         anchor="center", start=0.15 + m * 0.03, end=B0 + 0.3, fade=0.3)
# baked draw-on of the last 12 months (A scale), with a breathing head dot
nA = len(LAST)
for f in range(int(A0 * FPS), int(B1 * FPS)):
    t = f / FPS
    if t < B0:
        u = ease_io((t + 0.15) / 2.6)
        FL = fine(LAST)
        k = max(2, int(round(u * (len(FL) - 1))) + 1)
        P = P2(FL[:k], rA)
        items = [{"points": P, "smooth": True, "stroke": "#FF6A45", "width": 4.5, "cap": "round"}]
        # soft glow under the line
        items.insert(0, {"points": P, "smooth": True, "stroke": "#FF6A4533", "width": 16, "cap": "round"})
        hx, hy = P[-1]
        pulse = 1 + 0.25 * math.sin(t * 6)
        items.append({"circle": [hx, hy, 9 * pulse], "fill": "#FFE2D6"})
        frame_layer(t, items)
    else:
        # B: the year slides to its place on the spiral (outermost ring)
        u = ease_io((t - B0) / (B1 - B0)) * 0.85
        FL = fine(LAST)
        P = [[round(lerp(rA(tt, v)[0], rB(tt, v)[0], u), 1), round(lerp(rA(tt, v)[1], rB(tt, v)[1], u), 1)] for tt, v, _ in FL]
        frame_layer(t, [{"points": P, "smooth": True, "stroke": "#FF6A45", "width": lerp(4.5, 2.2, u), "cap": "round", "opacity": round(1 - u, 3)}])

# month value labels at the peak and the trough
pk = max(LAST, key=lambda i: D[i][3]); tr = min(LAST, key=lambda i: D[i][3])
for i, lab, dx, dy in ((pk, "May 2026  %.1f" % D[pk][3], 44, 30), (tr, "Oct 2025  %.1f" % D[tr][3], 30, -14)):
    x, y = pA(D[i])
    text(lab, x + dx, y + dy, 22, INK, font="IBM Plex Mono", weight=400, start=2.0 if i == pk else 0.6, end=B0, fade=0.3)

text("ONE YEAR AT MAUNA LOA", 120, 150, 22, "#FF8A6A", font="IBM Plex Mono", weight=400, start=0.0, end=B0 + 0.2,
     letterSpacing="0.16em")
text("Every year,\nthe planet\ntakes a breath.", 120, 200, 76, INK, weight=300, start=0.0, end=B0 + 0.2,
     letterSpacing="-0.02em")
text("Each spring the northern forests leaf out\nand pull carbon from the air. From May to\nSeptember, carbon dioxide falls by about %.0f ppm.\nIn winter, it all comes back." % (D[pk][3] - D[LAST[-1]][3]),
     120, 520, 27, "#B9C1CA", weight=300, start=0.9, end=B0 + 0.2, lineHeight=1.42)

# ---- beat 2: the spiral --------------------------------------------------
# faint ppm rings
grid = []
for ppm in range(320, 440, 20):
    r = 64 + (ppm - 312) * 3.6
    grid.append({"circle": [CX, CY, round(r, 1)], "stroke": HAIR, "width": 1})
layers.append({"draw": grid, "start": S0 - 0.2, "end": M0 + 0.5,
               "animate": [{"from": {"opacity": 0}, "duration": 0.5}, {"to": {"opacity": 0}, "at": M0 - S0 + 0.2, "duration": 0.4}]})
for ppm in (420,):
    r = 64 + (ppm - 312) * 3.6
    text(str(ppm) + " ppm", CX + 6, CY - r - 24, 17, MUTED, font="IBM Plex Mono", weight=400, start=S0 + 0.1, end=M0 + 0.3, fade=0.4)

def spiral_items(k, width=2.0, head=True):
    P = P2(FALL[:k], rB)
    items = [{"points": P, "colors": [c for _, _, c in FALL[:k]], "width": width, "cap": "round", "join": "round"}]
    if head:
        hx, hy = P[-1]
        items.append({"circle": [hx, hy, 6], "fill": "#FFFFFF"})
    return items

for f in range(int(S0 * FPS), int(M0 * FPS)):
    t = f / FPS
    u = ease_io((t - S0) / (S1 - S0))
    kf = max(2, int(round(u * (len(FALL) - 1))) + 1)
    frame_layer(t, spiral_items(kf, head=t < S1 + 0.1))
    # live year + ppm readout (exact monthly values from the data)
    d = D[min(N - 1, (kf - 1) // SUB)]
    layers.append({"text": str(d[0]), "font": "IBM Plex Mono", "size": 132, "weight": 400, "color": ycol(d[2]),
                   "x": 120, "y": 430, "anchor": "topLeft", "start": round(t - 0.25 / FPS, 5), "end": round(t + 0.75 / FPS, 5),
                   "letterSpacing": "-0.03em"})
    layers.append({"text": "%.1f ppm" % d[3], "font": "IBM Plex Mono", "size": 40, "weight": 400, "color": INK,
                   "x": 126, "y": 600, "anchor": "topLeft", "start": round(t - 0.25 / FPS, 5), "end": round(t + 0.75 / FPS, 5)})

text("68 YEARS, 68 BREATHS", 120, 150, 22, "#FF8A6A", font="IBM Plex Mono", weight=400, start=S0, end=M0 + 0.1, letterSpacing="0.16em")
text("Each loop is a year.\nEach sits a little\nfurther out.", 120, 200, 56, INK, weight=300, start=S0 + 0.1, end=M0 + 0.1, letterSpacing="-0.015em")
text("Monthly mean, plotted as a clock:\nangle is the month, distance\nfrom the centre is carbon dioxide.", 126, 700, 24, "#9AA3AD", weight=300, start=S0 + 0.6, end=M0 + 0.1, lineHeight=1.42)

# ---- beat 3: unroll into the curve ----------------------------------------
for f in range(int(M0 * FPS), int(END * FPS)):
    t = f / FPS
    u = (t - M0) / (M1 - M0)
    P = []
    NF = len(FALL)
    for i, (tt, v, _) in enumerate(FALL):
        s = 0.55 * (1 - i / (NF - 1))           # outer (recent) years start first: the spiral peels off from its outer end
        w = ease_io((u - s) / 0.45)
        a, b = rB(tt, v), rC(tt, v)
        # bow the path through an arc so points swing rather than slide
        mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2 - 60 * math.sin(math.pi * w)
        x = (1 - w) ** 2 * a[0] + 2 * (1 - w) * w * mx + w * w * b[0]
        y = (1 - w) ** 2 * a[1] + 2 * (1 - w) * w * my + w * w * b[1]
        P.append([round(x, 1), round(y, 1)])
    items = [{"points": P, "colors": [c for _, _, c in FALL], "smooth": False, "width": 2.0 + 0.4 * min(1, max(0, u)), "cap": "round", "join": "round"}]
    if t > M1:
        # the "now" dot breathes at the end of the line
        hx, hy = P[-1]
        ph = (t - M1)
        items.append({"circle": [hx, hy, 18 + 6 * math.sin(ph * 2.6)], "fill": "#FF543422"})
        items.append({"circle": [hx, hy, 7], "fill": "#FFFFFF"})
    frame_layer(t, items)

# axes and decade rates for the curve
axis = [{"line": [180, 960, 1780, 960], "stroke": "#3A4552", "width": 1.2}]
for ppm in range(320, 440, 20):
    y = 960 - (ppm - 312) * 6.6
    axis.append({"line": [180 if y > 440 else 1020, round(y, 1), 1780, round(y, 1)], "stroke": HAIR, "width": 1})
layers.append({"draw": axis, "start": M1 - 0.4, "animate": [{"from": {"opacity": 0}, "duration": 0.6}]})
for ppm in range(320, 440, 20):
    y = 960 - (ppm - 312) * 6.6
    text(str(ppm), 1800, y - 12, 18, MUTED, font="IBM Plex Mono", weight=400, start=M1 - 0.2, fade=0.5)
for yr in range(1960, 2030, 10):
    x = 180 + (yr - 1958) / (2027 - 1958) * 1600
    text(str(yr), x, 975, 18, MUTED, font="IBM Plex Mono", weight=400, start=M1 - 0.2, fade=0.5, anchor="top")

RATES = []
for dec in range(1960, 2030, 10):
    s = [x for x in D if dec <= x[0] < dec + 10]
    n = len(s); mx = sum(x[2] for x in s) / n; my = sum(x[4] for x in s) / n
    b = sum((x[2] - mx) * (x[4] - my) for x in s) / sum((x[2] - mx) ** 2 for x in s)
    RATES.append((dec, b, s))
for j, (dec, b, s) in enumerate(RATES):
    mid = s[len(s) // 2]
    x, y = pC(mid)
    lab = ("+%.1f" % b) + ("/yr" if j in (0, len(RATES) - 1) else "")
    below = j == len(RATES) - 1
    text(lab, (x + 40) if below else (x - 10), (y + 130) if below else (y - 74 - 6 * j), 24 if j in (0, len(RATES) - 1) else 20, ycol(mid[2]), font="IBM Plex Mono", weight=400,
         start=M1 + 0.5 + j * 0.16, fade=0.35, anchor="bottom")
    ly0, ly1 = ((y + 24, y + 100) if below else (y - 64 - 6 * j, y - 14))
    layers.append({"draw": [{"line": [round(x, 1), round(ly0, 1), round(x, 1), round(ly1, 1)], "stroke": ycol(mid[2]) + "88", "width": 1.2}],
                   "start": M1 + 0.5 + j * 0.16, "animate": [{"from": {"opacity": 0}, "duration": 0.3}]})

text("THE KEELING CURVE", 180, 120, 22, "#FF8A6A", font="IBM Plex Mono", weight=400, start=M1 - 0.1, letterSpacing="0.16em")
text("The breath hasn’t changed.\nThe air has.", 180, 160, 64, INK, weight=300, start=M1 + 0.1, letterSpacing="-0.02em")
text("Carbon dioxide now rises %.1f ppm a year, %.1f× the pace of the 1960s.\nppm per year, by decade (trend of the deseasonalised series)." % (RATES[-1][1], RATES[-1][1] / RATES[0][1]),
     183, 330, 25, "#9AA3AD", weight=300, start=M1 + 1.4, lineHeight=1.42)
last = D[-1]
hx, hy = pC(last)
text("%.1f" % last[3], hx - 16, hy - 64, 64, "#FFFFFF", font="IBM Plex Mono", weight=400, start=M1 + 2.0, anchor="bottomRight", fade=0.4)
text("ppm, Sep 2026", hx - 16, hy - 40, 20, "#FF8A6A", font="IBM Plex Mono", weight=400, start=M1 + 2.1, anchor="bottomRight", fade=0.4)
layers.append({"draw": [{"line": [round(pC(D[0])[0], 1), round(pC(D[0])[1] - 150, 1), round(pC(D[0])[0], 1), round(pC(D[0])[1] - 12, 1)], "stroke": "#6FA9C488", "width": 1.2}],
               "start": M1 + 0.3, "animate": [{"from": {"opacity": 0}, "duration": 0.3}]})
text("%.1f ppm, Mar 1958" % D[0][3], pC(D[0])[0] + 2, pC(D[0])[1] - 180, 18, "#6FA9C4", font="IBM Plex Mono", weight=400, start=M1 + 0.3, fade=0.4)

MUSIC = {
  "bpm": 72, "key": "D", "length": "13.0s", "seed": 11, "end": "ring", "master": {"reverb": "hall"},
  "tracks": [
    {"inst": "piano", "notes": "o4 l8 r/4 F#4 A4 D5 A4 E5/4. A4 | r/8 F#4 A4 D5 C#5/2 | r/4 G4 B4 D5 B4 F#5/4. B4 | r/8 A4 C#5 E5 A5/2 | D5/2 C#5/4 B4/4 | A4/1", "gain": -4, "reverb": 0.45},
    {"inst": "strings", "notes": "o3 l1 [D3 A3 F#4] | [B2 F#3 D4] | [G2 D3 B3] | [A2 E3 C#4] | [D3 A3 D4 F#4]", "gain": -11, "reverb": 0.55,
     "automate": {"volume": [[0, -8], [3.5, -4], [4, -2], [9, 0], [11, -3], [16, 2]]}},
    {"inst": "cello", "notes": "o2 r/1 | r/2 B2/2 | G2/1 | A2/1 | D2/1 _", "gain": -8, "reverb": 0.45},
    {"inst": "celesta", "notes": "o6 r/1 | r/1 | r/1 | r/2 A6/8 F#6/8 E6/8 C#6/8 | D6/2", "gain": -12, "reverb": 0.6}
  ]
}

spec = {
  "size": [W, H], "fps": FPS, "transition": "cut", "background": BG,
  "fonts": {"Libre Franklin": "fonts/Libre_Franklin_wght_300.ttf", "Libre Franklin SemiBold": "fonts/Libre_Franklin_wght_600.ttf",
            "Libre Franklin Italic": "fonts/Libre_Franklin_ital_wght_1_300.ttf", "IBM Plex Mono": "fonts/IBM_Plex_Mono_wght_400.ttf"},
  "intent": {"pace": "steady", "feeling": "clear, quietly alarming"},
  "music": MUSIC,
  "scenes": [{"duration": END, "layers":
     [{"shader": "// anikit:aa off\nvoid mainImage(out vec4 o, in vec2 f){ vec2 uv = f/iResolution.xy; vec2 q = uv - vec2(0.62, 0.5); float v = 1.0 - 0.55*dot(q*vec2(1.6, 1.0), q*vec2(1.6, 1.0)); float g = fract(sin(dot(floor(f) + floor(iTime*24.0), vec2(12.9898, 78.233)))*43758.5453) - 0.5; vec3 c = vec3(0.039, 0.059, 0.090)*(0.75 + 0.45*v) + g*0.012; o = vec4(c, 1.0); }"}]
     + layers}],
}
json.dump(spec, open(os.path.join(HERE, "breath.json"), "w"))
print("layers:", len(layers), "json KB:", os.path.getsize(os.path.join(HERE, "breath.json")) // 1024)
