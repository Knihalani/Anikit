#!/usr/bin/env python3
"""Burrow launch film: builds spec.json.

The world is one illo illustration (4800 x 3000 design px): a night hill with a
house on top, and under it a warren of story chambers lit like shadow-puppet
theatres. A single camera move (the "world" group) sinks from the bedroom window
down a freshly dug tunnel into tonight's chamber, then pulls back to reveal every
story the family has made before.
"""
import json, math, random, os

random.seed(7)
HERE = os.path.dirname(os.path.abspath(__file__))

W, H = 4800, 3000
INK = "#170c18"
PUPPET = "#3b1713"

def surface(x):
    return 820 + 430 * ((x - 2400) / 2400) ** 2

shapes = []
def add(s, group="world"):
    s.setdefault("group", group)
    shapes.append(s)
    return s

# ---------------------------------------------------------------- sky
# stars
for i in range(70):
    x = random.uniform(-600, W + 600)
    y = random.uniform(-500, 760)
    if y > surface(min(max(x, 0), W)) - 60:
        continue
    r = random.choice([6, 7, 9, 12, 16]) if i % 9 else 22
    add({"shape": "star", "cx": round(x), "cy": round(y), "r": r, "n": 4, "inner": 0.32,
         "fill": "#f6e6b4", "outline": False, "wobble": 0.3,
         "sway": {"scale": 0.35, "period": round(random.uniform(1.6, 3.4), 2), "phase": round(random.random(), 2)}}, "sky")
# moon (crescent)
add({"shape": "circle", "name": "moonhalo", "cx": 3260, "cy": 300, "r": 260, "fill": "#f6e6b4", "opacity": 0.06, "outline": False}, "sky")
add({"shape": "circle", "name": "mooncut", "cx": 3330, "cy": 250, "r": 150, "visible": False}, "sky")
add({"shape": "circle", "name": "moon", "cx": 3260, "cy": 300, "r": 160, "fill": "#f7e7b9", "cut": ["mooncut"],
     "shade": {"amount": 0.35, "offset": 0.08}, "texture": {"style": "paper", "amount": 0.5}, "outline": True}, "sky")
# soft clouds drifting
for (cx, cy, rx, ry, ph) in [(1300, 380, 420, 70, 0), (3900, 560, 380, 60, 0.4), (2600, 120, 300, 46, 0.7)]:
    add({"shape": "blob", "cx": cx, "cy": cy, "rx": rx, "ry": ry, "lumpy": 0.3, "n": 9, "seed": int(cx),
         "fill": "#3a4680", "opacity": 0.75, "outline": False, "texture": "paper",
         "sway": {"x": 60, "period": 14, "phase": ph}}, "sky")

# ---------------------------------------------------------------- earth
# earth strata as stripes between curves, cut into 600 px tiles so the
# per-window culling below can drop what the camera can't see
OFFS = [0, 70, 520, 1150, 1750, H + 600]
COLS = ["#24374a", "#4b2a33", "#3f2330", "#331b29", "#271421"]
TEX = [None] * 5
def curve(off, x):
    if off >= H:
        return off
    wob = 14 * math.sin(x / 80 * 1.7 / 1.0 + off) if off else 0
    return round(surface(x) + off + wob)
X0, X1, TW = -1400, W + 1400, 600
for bi in range(5):
    for tx in range(X0, X1, TW):
        xs = list(range(tx, tx + TW + 1, 40))
        top = [[x, curve(OFFS[bi], x)] for x in xs]
        bot = [[x, curve(OFFS[bi + 1], x) + 8] for x in reversed(xs)]
        top[0][0] -= 6; top[-1][0] += 6; bot[0][0] += 6; bot[-1][0] -= 6
        sh = {"shape": "path", "points": top + bot, "closed": True, "fill": COLS[bi], "grain": 0, "outline": False,
              "wobble": 0, "rough": 0, "corners": "sharp"}
        if TEX[bi]:
            sh["texture"] = TEX[bi]
        add(sh)

# grass tufts along the surface
for i in range(46):
    x = 60 + i * 104 + random.uniform(-30, 30)
    y = surface(x) + 6
    h = random.uniform(28, 60)
    add({"shape": "path", "points": [[round(x - 12), round(y)], [round(x - 4), round(y - h)], [round(x + 3), round(y - 4)], [round(x + 14), round(y - h * 0.8)], [round(x + 18), round(y)]],
         "closed": True, "fill": "#2c4558", "outline": False,
         "sway": {"rotate": 4, "period": 2.6, "phase": round(i * 0.13 % 1, 2)}, "origin": [round(x), round(y)]})
# pebbles
for i in range(60):
    x = random.uniform(0, W)
    y = random.uniform(surface(x) + 140, H - 40)
    add({"shape": "blob", "cx": round(x), "cy": round(y), "rx": random.randint(10, 26), "ry": random.randint(8, 18),
         "seed": i, "fill": random.choice(["#5d3a3e", "#6b4442", "#4f2f37"]), "outline": False, "shade": 0.5})

# tree on the left shoulder of the hill, roots reaching down
TX = 3560
ty = surface(TX)
add({"shape": "path", "name": "trunk", "points": [[TX - 34, ty + 10], [TX - 22, ty - 240], [TX - 60, ty - 330], [TX - 10, ty - 300], [TX + 4, ty - 420], [TX + 20, ty - 300], [TX + 70, ty - 340], [TX + 28, ty - 240], [TX + 38, ty + 10]],
     "closed": True, "fill": "#1b1426", "outline": False})
add({"shape": "blob", "cx": TX, "cy": round(ty - 470), "rx": 230, "ry": 190, "lumpy": 0.35, "n": 11, "seed": 3,
     "fill": "#1c2c3e", "shade": {"amount": 0.6, "offset": 0.1}, "texture": {"style": "hatch", "amount": 0.4}, "outline": True,
     "sway": {"rotate": 1.2, "period": 5}, "origin": [TX, round(ty - 300)]})
for k, (dx, dy) in enumerate([(-260, 300), (-90, 420), (120, 380), (300, 250), (-380, 160)]):
    add({"shape": "path", "points": [[TX, round(ty + 10)], [TX + dx * 0.4, round(ty + dy * 0.5)], [TX + dx, round(ty + dy)], [TX + dx * 1.2, round(ty + dy * 1.25)]],
         "smooth": True, "fill": "none", "stroke": "#5f3a2c", "weight": [2, 9], "outline": True})

# ---------------------------------------------------------------- house on the hilltop
HX, HY = 2400, round(surface(2400))  # base
add({"shape": "rect", "name": "house", "x": HX - 150, "y": HY - 200, "w": 300, "h": 205, "fill": "#2a2f55",
     "shade": {"amount": 0.6, "offset": 0.12}, "texture": {"style": "hatch", "amount": 0.35}, "outline": True, "corners": 4})
add({"shape": "polygon", "points": [[HX - 190, HY - 190], [HX, HY - 360], [HX + 190, HY - 190]], "fill": "#3a2032",
     "shade": 0.5, "outline": True})
add({"shape": "rect", "x": HX + 80, "y": HY - 370, "w": 44, "h": 110, "fill": "#3a2032", "outline": True})
# window glow spilling onto the grass
add({"shape": "ellipse", "cx": HX - 40, "cy": HY + 18, "rx": 230, "ry": 30, "fill": "#ffb85c", "opacity": 0.4, "blend": "screen", "outline": False})
add({"shape": "rect", "name": "window", "x": HX - 110, "y": HY - 160, "w": 140, "h": 104, "fill": {"stops": [[0, "#ffe2a0"], [1, "#f39a4c"]], "gradient": "radial"},
     "outline": True, "corners": 6})
# parent and child, silhouetted in the window, leaning over one story
add({"shape": "circle", "cx": HX - 72, "cy": HY - 112, "r": 19, "fill": PUPPET, "clipTo": "window", "outline": False})
add({"shape": "ellipse", "cx": HX - 70, "cy": HY - 62, "rx": 38, "ry": 34, "fill": PUPPET, "clipTo": "window", "outline": False})
add({"shape": "circle", "cx": HX - 14, "cy": HY - 94, "r": 14, "fill": PUPPET, "clipTo": "window", "outline": False,
     "sway": {"rotate": 3, "period": 3.2}, "origin": [HX - 14, HY - 80]})
add({"shape": "ellipse", "cx": HX - 14, "cy": HY - 60, "rx": 24, "ry": 24, "fill": PUPPET, "clipTo": "window", "outline": False})
add({"shape": "rect", "x": HX - 40, "y": HY - 76, "w": 34, "h": 22, "fill": "#fff1c8", "clipTo": "window", "outline": False, "rotate": -8})  # the book/tablet glow
add({"shape": "rect", "x": HX - 112, "y": HY - 112, "w": 144, "h": 6, "fill": "#2a2f55", "outline": False})
add({"shape": "rect", "x": HX - 44, "y": HY - 160, "w": 6, "h": 104, "fill": "#2a2f55", "outline": False})
# door
add({"shape": "rect", "x": HX + 60, "y": HY - 120, "w": 56, "h": 120, "fill": "#3a2032", "outline": True, "corners": 20})

# ---------------------------------------------------------------- chambers
# (cx, cy, rx, ry, motif)
MAIN = (2500, 1760, 330, 220)
CH = [
    (1700, 1480, 170, 118, "whale"), (3250, 1500, 185, 125, "balloon"), (1180, 1880, 190, 130, "castle"),
    (3720, 1860, 200, 135, "moonbed"), (2050, 2220, 180, 124, "rocket"), (3000, 2260, 205, 140, "lighthouse"),
    (820, 2330, 165, 115, "tree"), (1640, 2580, 190, 130, "dragon"), (2470, 2620, 215, 142, "whale"),
    (3420, 2640, 190, 130, "castle"), (4180, 2350, 175, 120, "balloon"), (620, 1640, 140, 98, "moonbed"),
    (4300, 1640, 150, 104, "tree"), (900, 2800, 150, 100, "rocket"), (3950, 2880, 150, 96, "lighthouse"),
    (2950, 2900, 130, 86, "tree"), (1950, 2900, 130, 86, "balloon"),
]

def tunnel(p0, p1, r=34, bend=0.0):
    (x0, y0), (x1, y1) = p0, p1
    mx, my = (x0 + x1) / 2 + bend, (y0 + y1) / 2
    n = max(3, int(math.hypot(x1 - x0, y1 - y0) / (r * 0.9)))
    out = []
    for i in range(n + 1):
        t = i / n
        x = (1 - t) ** 2 * x0 + 2 * (1 - t) * t * mx + t * t * x1
        y = (1 - t) ** 2 * y0 + 2 * (1 - t) * t * my + t * t * y1
        out.append((round(x), round(y)))
    return out

def tunnel_path(pts, w=46):
    # one thick, rough-edged tunnel: a brush stroke along the route
    return {"shape": "path", "points": [list(p) for p in pts], "smooth": True, "fill": "none",
            "stroke": "#1a0d16", "weight": [w * 0.7, w], "outline": True, "taper": 0}

# old tunnels joining the warren (previous nights)
links = [(0, 2), (0, 11), (1, 3), (1, 12), (2, 6), (2, 7), (3, 10), (4, 7), (4, 8), (5, 8), (5, 9), (6, 13), (9, 14),
         (8, 16), (9, 15), (3, 9), (7, 16), (10, 14)]
for a, b in links:
    A, B = CH[a], CH[b]
    add(tunnel_path(tunnel((A[0], A[1]), (B[0], B[1]), bend=random.choice([-1, 1]) * random.uniform(120, 260))))
for k in (0, 1, 4, 5):
    A = CH[k]
    add(tunnel_path(tunnel((A[0], A[1]), (MAIN[0], MAIN[1]), bend=random.uniform(-80, 80))))
# short entrances from the surface on the far shoulders
for x in ():
    add(tunnel_path(tunnel((x, surface(x) + 20), (CH[11][0] if x < 2400 else CH[12][0], CH[11][1] if x < 2400 else CH[12][1]))))

# tonight's tunnel: dug live, house -> main chamber, one bite at a time
DIG0, DIG1 = 3.5, 6.6
dig = tunnel((HX + 88, HY - 4), (MAIN[0] - 40, MAIN[1] - MAIN[3] + 40), r=22, bend=-260)
for i, (x, y) in enumerate(dig):
    t = DIG0 + (DIG1 - DIG0) * i / (len(dig) - 1)
    add({"shape": "blob", "cx": x, "cy": y, "r": 40, "lumpy": 0.08, "n": 9, "seed": i, "fill": "#120910", "outline": False,
         "animate": {"keys": [{"t": 0, "opacity": 0, "scale": 0.2}, {"t": round(t, 3), "opacity": 0, "scale": 0.2},
                              {"t": round(t + 0.12, 3), "opacity": 1, "scale": 1.15, "ease": "out"}, {"t": round(t + 0.3, 3), "scale": 1, "ease": "inOut"}]}})
    # crumbs kicked up by the digging
    if i % 2 == 0:
        for j in range(2):
            ang = random.uniform(0, 2 * math.pi)
            add({"shape": "blob", "cx": x, "cy": y, "r": random.randint(6, 11), "seed": i * 7 + j, "fill": "#7a4c45", "outline": False,
                 "animate": {"keys": [{"t": 0, "opacity": 0}, {"t": round(t, 3), "opacity": 0, "x": 0, "y": 0},
                                      {"t": round(t + 0.05, 3), "opacity": 1},
                                      {"t": round(t + 0.6, 3), "opacity": 0, "x": round(70 * math.cos(ang)), "y": round(70 * math.sin(ang) - 30), "ease": "out"}]}})

# the lantern at the tip of the dig: a warm spark leading the way down
lk = [{"t": 0, "x": 0, "y": 0, "opacity": 0}, {"t": DIG0 - 0.3, "x": 0, "y": 0, "opacity": 0}, {"t": DIG0, "x": 0, "y": 0, "opacity": 1}]
for i, (x, y) in enumerate(dig):
    t = DIG0 + (DIG1 - DIG0) * i / (len(dig) - 1)
    lk.append({"t": round(t + 0.1, 3), "x": x - dig[0][0], "y": y - dig[0][1]})
lk.append({"t": DIG1 + 0.35, "x": dig[-1][0] - dig[0][0], "y": dig[-1][1] - dig[0][1] + 60, "opacity": 0})
for r, op in [(120, 0.16), (60, 0.3), (16, 1.0)]:
    add({"shape": "circle", "cx": dig[0][0], "cy": dig[0][1], "r": r, "fill": "#ffd27a" if r < 20 else "#ff9a4a",
         "opacity": op if r > 20 else 1, "blend": "screen" if r > 20 else None, "outline": False,
         "animate": {"keys": [dict(k, opacity=(k["opacity"] * (op if r > 20 else 1)) if "opacity" in k else None) for k in lk]}})

def chamber(cx, cy, rx, ry, name, t_on, motif, main=False):
    seed = cx % 97
    add({"shape": "blob", "cx": cx, "cy": cy, "rx": rx + 26, "ry": ry + 22, "lumpy": 0.12, "n": 9, "seed": seed,
         "fill": "#120910", "outline": False})
    # warm light pooling on the soil around a lit room
    add({"shape": "blob", "cx": cx, "cy": cy, "rx": rx * 1.9, "ry": ry * 1.9, "lumpy": 0.1, "seed": seed, "fill": "#ff9a4a",
         "opacity": 0, "blend": "screen", "outline": False,
         "animate": {"keys": [{"t": 0, "opacity": 0}, {"t": t_on, "opacity": 0}, {"t": t_on + 0.5, "opacity": 0.13, "ease": "out"}]}})
    add({"shape": "blob", "name": name, "cx": cx, "cy": cy, "rx": rx, "ry": ry, "lumpy": 0.12, "n": 9, "seed": seed,
         "fill": {"stops": [[0, "#ffe7a6"], [0.55, "#f7ad57"], [1, "#c9572f"]], "gradient": "radial", "center": [0.5, 0.42], "steps": 5, "dither": 0.2},
         "texture": {"style": "paper", "amount": 0.5}, "grain": 0.06, "outline": True, "weight": [2, 7],
         "animate": {"keys": [{"t": 0, "opacity": 0.0}, {"t": t_on, "opacity": 0.0}, {"t": t_on + 0.08, "opacity": 1, "ease": "out"}]}})
    CUR_T[0] = t_on
    motif_fn[motif](cx, cy + (30 if main else 0), min(rx / 200, ry / 135) * (0.86 if main else 1), name, t_on)
    # the floor of the room, a little lip of soil
    add({"shape": "ellipse", "cx": cx, "cy": cy + ry * 0.93, "rx": rx * 0.95, "ry": ry * 0.2, "fill": "#120910", "clipTo": name, "outline": False})

CUR_T = [0.0]
def P(cl, **kw):
    if "animate" not in kw and kw.get("visible", True):
        t = CUR_T[0]
        kw["animate"] = {"keys": [{"t": 0, "opacity": 0}, {"t": t, "opacity": 0}, {"t": t + 0.1, "opacity": kw.pop("opacity", 1), "ease": "out"}]}
    kw.setdefault("fill", PUPPET)
    kw.setdefault("outline", False)
    kw["clipTo"] = cl
    return add(kw)

def pts(cx, cy, k, arr):
    return [[round(cx + x * k), round(cy + y * k)] for x, y in arr]

def m_whale(cx, cy, k, n, t):
    P(n, shape="ellipse", cx=round(cx - 10 * k), cy=round(cy + 30 * k), rx=round(110 * k), ry=round(52 * k))
    P(n, shape="path", points=pts(cx, cy, k, [(90, 20), (150, -20), (170, -50), (150, 0), (185, 10), (100, 60)]), closed=True, smooth=True)
    P(n, shape="path", points=pts(cx, cy, k, [(-60, -20), (-70, -70), (-55, -100), (-80, -110)]), fill="none", stroke=PUPPET, weight=[3, 6], smooth=True)
    P(n, shape="path", points=pts(cx, cy, k, [(-200, 90), (-120, 70), (-40, 92), (40, 70), (120, 90), (200, 72), (200, 140), (-200, 140)]), closed=True, smooth=True)
    P(n, shape="star", cx=round(cx + 110 * k), cy=round(cy - 75 * k), r=round(18 * k), n=5, inner=0.45, fill="#fff3cf")

def m_balloon(cx, cy, k, n, t):
    P(n, shape="circle", cx=cx, cy=round(cy - 40 * k), r=round(70 * k))
    P(n, shape="path", points=pts(cx, cy, k, [(-50, 0), (-16, 62), (16, 62), (50, 0)]), closed=True)
    P(n, shape="rect", cx=cx, cy=round(cy + 76 * k), w=round(40 * k), h=round(30 * k))
    P(n, shape="blob", cx=round(cx - 120 * k), cy=round(cy + 20 * k), rx=round(50 * k), ry=round(20 * k), fill="#fff3cf", opacity=0.7)

def m_castle(cx, cy, k, n, t):
    P(n, shape="rect", cx=cx, cy=round(cy + 50 * k), w=round(200 * k), h=round(120 * k))
    for dx, h in [(-90, 150), (0, 200), (90, 150)]:
        P(n, shape="rect", cx=round(cx + dx * k), cy=round(cy + (110 - h / 2) * k), w=round(46 * k), h=round(h * k))
        P(n, shape="polygon", points=pts(cx, cy, k, [(dx - 32, 110 - h), (dx, 110 - h - 60), (dx + 32, 110 - h)]))
    P(n, shape="rect", cx=cx, cy=round(cy - 10 * k), w=round(14 * k), h=round(22 * k), fill="#fff3cf")

def m_moonbed(cx, cy, k, n, t):
    P(n, shape="circle", name=n + "c", cx=round(cx + 30 * k), cy=round(cy - 40 * k), r=round(60 * k), visible=False)
    P(n, shape="circle", cx=round(cx), cy=round(cy - 20 * k), r=round(70 * k), cut=[n + "c"])
    P(n, shape="polygon", points=pts(cx, cy, k, [(-50, -70), (-120, -150), (-20, -90)]))
    P(n, shape="circle", cx=round(cx - 122 * k), cy=round(cy - 152 * k), r=round(12 * k))
    P(n, shape="rect", cx=cx, cy=round(cy + 85 * k), w=round(260 * k), h=round(40 * k), radius=10)

def m_rocket(cx, cy, k, n, t):
    P(n, shape="path", points=pts(cx, cy, k, [(0, -130), (36, -60), (36, 50), (-36, 50), (-36, -60)]), closed=True, smooth=False)
    P(n, shape="polygon", points=pts(cx, cy, k, [(-36, 10), (-70, 70), (-36, 50)]))
    P(n, shape="polygon", points=pts(cx, cy, k, [(36, 10), (70, 70), (36, 50)]))
    P(n, shape="circle", cx=cx, cy=round(cy - 40 * k), r=round(14 * k), fill="#fff3cf")
    P(n, shape="blob", cx=cx, cy=round(cy + 90 * k), rx=round(26 * k), ry=round(40 * k), fill="#fff3cf", opacity=0.8,
      sway={"scale": 0.15, "period": 0.3})

def m_lighthouse(cx, cy, k, n, t):
    P(n, shape="polygon", points=pts(cx, cy, k, [(-34, 110), (-22, -60), (22, -60), (34, 110)]))
    P(n, shape="rect", cx=cx, cy=round(cy - 80 * k), w=round(56 * k), h=round(40 * k))
    P(n, shape="polygon", points=pts(cx, cy, k, [(0, -80), (220, -130), (220, -30)]), fill="#fff3cf", opacity=0.55,
      sway={"rotate": 10, "period": 3}, origin=[cx, round(cy - 80 * k)])
    P(n, shape="path", points=pts(cx, cy, k, [(-220, 110), (-120, 96), (-40, 112), (60, 96), (220, 110), (220, 160), (-220, 160)]), closed=True, smooth=True)

def m_tree(cx, cy, k, n, t):
    P(n, shape="rect", cx=cx, cy=round(cy + 60 * k), w=round(30 * k), h=round(110 * k))
    P(n, shape="blob", cx=cx, cy=round(cy - 30 * k), rx=round(100 * k), ry=round(85 * k), lumpy=0.3, seed=cx % 13)
    P(n, shape="circle", cx=round(cx + 20 * k), cy=round(cy - 40 * k), r=round(14 * k), fill="#fff3cf")  # owl eye
    P(n, shape="circle", cx=round(cx + 46 * k), cy=round(cy - 40 * k), r=round(14 * k), fill="#fff3cf")

def m_dragon(cx, cy, k, n, t):
    P(n, shape="path", points=pts(cx, cy, k, [(-150, 60), (-90, 20), (-30, 40), (20, 0), (60, -50), (110, -60), (90, -30), (60, 10), (40, 60), (-40, 70)]), closed=True, smooth=True)
    P(n, shape="polygon", points=pts(cx, cy, k, [(-30, 30), (10, -90), (40, 0)]))
    P(n, shape="polygon", points=pts(cx, cy, k, [(-80, 30), (-60, -60), (-20, 20)]))
    P(n, shape="circle", cx=round(cx + 92 * k), cy=round(cy - 48 * k), r=round(6 * k), fill="#fff3cf")

def m_story(cx, cy, k, n, t):
    """Tonight's story, made up piece by piece: a rabbit sailing a teacup on paper waves."""
    pops = [t + 0.55 + 0.4167 * i for i in range(6)]
    def pop(i, base=1.0):
        return {"keys": [{"t": 0, "scale": 0, "opacity": 0}, {"t": round(pops[i], 3), "scale": 0, "opacity": 0},
                         {"t": round(pops[i] + 0.22, 3), "scale": 1.12 * base, "opacity": 1, "ease": "out"},
                         {"t": round(pops[i] + 0.42, 3), "scale": base, "ease": "inOut"}]}
    # moon + stars (piece 5)
    P(n, shape="circle", name=n + "mc", cx=round(cx + 175 * k), cy=round(cy - 95 * k), r=round(34 * k), visible=False)
    P(n, shape="circle", cx=round(cx + 160 * k), cy=round(cy - 85 * k), r=round(38 * k), cut=[n + "mc"], fill="#fff3cf", animate=pop(4))
    for i, (sx, sy, r) in enumerate([(-170, -100, 14), (-90, -140, 10), (40, -130, 12), (240, -10, 9)]):
        P(n, shape="star", cx=round(cx + sx * k), cy=round(cy + sy * k), r=round(r * k), n=5, inner=0.45, fill="#fff3cf",
          animate=pop(5), sway={"scale": 0.2, "period": 1.7 + i * 0.3})
    # boat group (pieces 1-3) rocks on the waves
    boat = {"rotate": 5, "y": 6, "period": 2.4}
    P(n, shape="path", points=pts(cx, cy, k, [(-120, 40), (120, 40), (95, 105), (-95, 105)]), closed=True, smooth=True,
      animate=pop(1), sway=boat, origin=[cx, round(cy + 110 * k)])
    P(n, shape="path", points=pts(cx, cy, k, [(110, 55), (160, 50), (165, 85), (105, 92)]), fill="none", stroke=PUPPET, weight=[5, 9], smooth=True,
      animate=pop(1), sway=boat, origin=[cx, round(cy + 110 * k)])
    # the rabbit (piece 2): one sitting silhouette, facing the moon
    P(n, shape="path", points=pts(cx, cy, k, [(-62, 46), (-72, 4), (-52, -30), (-26, -50), (-32, -80), (-4, -98), (24, -88), (44, -64), (26, -48), (12, -30), (24, 12), (34, 46)]),
      closed=True, smooth=True, animate=pop(2), sway=boat, origin=[cx, round(cy + 110 * k)])
    P(n, shape="ellipse", cx=round(cx - 18 * k), cy=round(cy - 126 * k), rx=round(11 * k), ry=round(40 * k), animate=pop(2), sway=boat, origin=[cx, round(cy + 110 * k)])
    P(n, shape="ellipse", cx=round(cx - 2 * k), cy=round(cy - 134 * k), rx=round(11 * k), ry=round(42 * k), animate=pop(2), sway=boat, origin=[cx, round(cy + 110 * k)])
    P(n, shape="circle", cx=round(cx - 72 * k), cy=round(cy + 26 * k), r=round(14 * k), animate=pop(2), sway=boat, origin=[cx, round(cy + 110 * k)])
    P(n, shape="circle", cx=round(cx + 20 * k), cy=round(cy - 74 * k), r=round(5 * k), fill="#fff3cf", animate=pop(2), sway=boat, origin=[cx, round(cy + 110 * k)])
    # sail: a page from a book (piece 3)
    P(n, shape="rect", cx=round(cx + 60 * k), cy=round(cy - 60 * k), w=round(6 * k), h=round(200 * k), animate=pop(3), sway=boat, origin=[cx, round(cy + 110 * k)])
    P(n, shape="path", points=pts(cx, cy, k, [(66, -150), (150, -110), (140, 10), (66, 20)]), closed=True, smooth=True, fill="#fff3cf",
      animate=pop(3), sway=boat, origin=[cx, round(cy + 110 * k)])
    for j in range(3):
        P(n, shape="path", points=pts(cx, cy, k, [(80, -110 + j * 34), (128, -92 + j * 34)]), fill="none", stroke="#c98a55", weight=[2, 3],
          animate=pop(3), sway=boat, origin=[cx, round(cy + 110 * k)])
    # paper waves (piece 0), two rows sliding against each other
    for row, (yy, dx, ph) in enumerate([(95, 0, 0), (125, 40, 0.5)]):
        w = [(-360 + dx + i * 60, yy + (-22 if i % 2 else 0)) for i in range(14)]
        P(n, shape="path", points=pts(cx, cy, k, w + [(480, 200), (-360, 200)]), closed=True, smooth=True,
          fill=PUPPET if row else "#5a2219", animate=pop(0), sway={"x": 22 * (1 if row else -1), "period": 3.0, "phase": ph})

motif_fn = {"whale": m_whale, "balloon": m_balloon, "castle": m_castle, "moonbed": m_moonbed, "rocket": m_rocket,
            "lighthouse": m_lighthouse, "tree": m_tree, "dragon": m_dragon, "story": m_story}

T_MAIN = 6.95
chamber(*MAIN, "main", T_MAIN, "story", main=True)
# the cascade: every earlier story lights up, nearest first, as the camera pulls away
order = sorted(range(len(CH)), key=lambda i: math.hypot(CH[i][0] - MAIN[0], CH[i][1] - MAIN[1]))
T_CASC = 11.25
for rank, i in enumerate(order):
    cx, cy, rx, ry, motif = CH[i]
    chamber(cx, cy, rx, ry, f"c{i}", round(T_CASC + rank * 0.208, 3), motif)

# fireflies over the grass near the house (the evening is alive)
for i in range(14):
    x = HX + random.uniform(-900, 900)
    y = surface(x) - random.uniform(40, 260)
    add({"shape": "circle", "cx": round(x), "cy": round(y), "r": 5, "fill": "#ffe9a0", "outline": False,
         "sway": {"x": random.randint(15, 40), "y": random.randint(10, 30), "scale": 0.4, "period": round(random.uniform(2, 4), 2), "phase": round(random.random(), 2)}})

# ---------------------------------------------------------------- camera (the world group)
def view(cx, cy, s):
    """Keys that put world point (cx, cy) at the frame centre at scale s."""
    return {"x": round(960 - s * cx, 1), "y": round(540 - s * cy, 1), "scale": s}

cam = [
    (0.0, view(2470, 600, 1.0), None),
    (3.3, view(2480, 690, 1.0), "inOut"),
    (6.6, view(2470, 1730, 1.0), "inOut"),
    (10.9, view(2480, 1760, 1.12), "inOut"),
    (14.2, view(2400, 1500, 0.355), "inOut"),
    (20.0, view(2400, 1440, 0.33), "out"),
]
# sky moves at half speed on the way down (multiplane), then joins the pull-back
sky = [
    (0.0, view(2470, 600, 1.0), None),
    (3.3, view(2480, 645, 1.0), "inOut"),
    (6.6, view(2470, 1170, 1.0), "inOut"),
    (10.9, view(2480, 1190, 1.04), "inOut"),
    (14.2, view(2400, 1500, 0.355), "inOut"),
    (20.0, view(2400, 1440, 0.33), "out"),
]
def keys(c):
    out = []
    for t, v, e in c:
        k = {"t": t, **v}
        if e:
            k["ease"] = e
        out.append(k)
    return {"keys": out}

illo = {"component": "illo", "props": {
    "view": [1920, 1080],
    "light": {"angle": 315, "rim": 135, "color": "#ffe3a8", "shadow": "#2a1846", "strength": 0.9},
    "hand": {"weight": [0.8, 4.5], "wobble": 1.2, "rough": 0.6, "boil": 8, "boilAmount": 0.6, "color": INK, "gaps": 0.18},
    "corners": "soft", "grain": 0.06, "misregister": [1.5, 1],
    "groups": [{"name": "sky", "pivot": [0, 0], "animate": keys(sky)},
               {"name": "world", "pivot": [0, 0], "animate": keys(cam)}],
    "shapes": shapes}}

# ---------------------------------------------------------------- culling
def ease(e, u):
    if e == "inOut":
        return u * u * (3 - 2 * u)
    if e == "out":
        return 1 - (1 - u) ** 2
    return u

def at(c, t):
    for (t0, v0, _), (t1, v1, e) in zip(c, c[1:]):
        if t0 <= t <= t1:
            u = ease(e, (t - t0) / (t1 - t0))
            return {k: v0[k] + (v1[k] - v0[k]) * u for k in v0}
    return c[-1][1]

def vis_rect(c, t0, t1):
    xs, ys = [], []
    n = 12
    for i in range(n + 1):
        v = at(c, t0 + (t1 - t0) * i / n)
        s_ = v["scale"]
        mx, my = 300 / s_, 300 / s_
        xs += [(-v["x"]) / s_ - mx, (1920 - v["x"]) / s_ + mx]
        ys += [(-v["y"]) / s_ - my, (1080 - v["y"]) / s_ + my]
    return min(xs), min(ys), max(xs), max(ys)

def bbox(sh):
    k = sh["shape"]
    if "points" in sh:
        px = [p[0] for p in sh["points"]]; py = [p[1] for p in sh["points"]]
        b = [min(px), min(py), max(px), max(py)]
    elif k == "rect" and "x" in sh:
        b = [sh["x"], sh["y"], sh["x"] + sh["w"], sh["y"] + sh["h"]]
    elif k == "rect":
        b = [sh["cx"] - sh["w"] / 2, sh["cy"] - sh["h"] / 2, sh["cx"] + sh["w"] / 2, sh["cy"] + sh["h"] / 2]
    else:
        rx = sh.get("rx", sh.get("r", 0)); ry = sh.get("ry", sh.get("r", 0))
        b = [sh["cx"] - rx, sh["cy"] - ry, sh["cx"] + rx, sh["cy"] + ry]
    m = 160 if ("animate" in sh or "sway" in sh) else 20
    return [b[0] - m, b[1] - m, b[2] + m, b[3] + m]

BOX = [bbox(sh) for sh in shapes]
NAMED = {sh["name"]: i for i, sh in enumerate(shapes) if "name" in sh}
# children clipped to / cutting a shape share its box
for i, sh in enumerate(shapes):
    if "clipTo" in sh:
        BOX[i] = BOX[NAMED[sh["clipTo"]]]
for i, sh in enumerate(shapes):
    for c in sh.get("cut", []):
        BOX[NAMED[c]] = BOX[i]

def inter(b, r):
    return not (b[2] < r[0] or b[0] > r[2] or b[3] < r[1] or b[1] > r[3])

STEP = 0.5
windows = []
t = 0.0
while t < 20 - 1e-6:
    t1 = min(20.0, t + STEP)
    rects = {"world": vis_rect(cam, t, t1), "sky": vis_rect(sky, t, t1)}
    sub = []
    for i, sh in enumerate(shapes):
        if inter(BOX[i], rects[sh["group"]]):
            sub.append(sh)
        else:  # keep indices (and so every shape's hand-drawn seed) stable
            sub.append({"shape": "circle", "cx": -50000, "cy": -50000, "r": 1, "fill": "#000", "outline": False, "group": sh["group"]})
    windows.append((t, t1, sub))
    t = t1

def clean(o):
    if isinstance(o, dict):
        return {k: clean(v) for k, v in o.items() if v is not None}
    if isinstance(o, list):
        return [clean(v) for v in o]
    return o
spec = json.load(open(os.path.join(HERE, "film.json")))
layers = []
for (t0, t1, sub) in windows:
    L = json.loads(json.dumps(illo))
    L["props"]["shapes"] = sub
    end = "" if t1 >= 20 else f" && gt < {t1}"
    L["opacity"] = f"=gt >= {t0}{end} ? 1 : 0" if t0 > 0 else f"=gt < {t1} ? 1 : 0"
    layers.append(clean(L))
    print(f"{t0:4.1f}-{t1:4.1f}: {sum(1 for x in sub if x.get('cx') != -50000)} shapes")
sky_layer = {"component": "illo", "props": {"view": [1920, 1080], "grain": 0.05, "shapes": [
    {"shape": "rect", "x": -20, "y": -20, "w": 1960, "h": 1120, "fill": {"stops": [[0, "#0b1030"], [0.6, "#1a2352"], [1, "#2e3a6e"]], "angle": 90, "steps": 6, "dither": 0.2},
     "wobble": 0, "rough": 0, "outline": False, "corners": "sharp"}]}}
spec["scenes"][0]["layers"][0:0] = [sky_layer] + layers
json.dump(spec, open(os.path.join(HERE, "spec.json"), "w"), indent=1)
print(len(shapes), "shapes")
